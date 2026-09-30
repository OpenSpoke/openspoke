// tunnel-server: OpenSpoke spoke -> hub reverse tunnel, hub side (Rust).
//
// How it works:
// - Each spoke opens a Tunnel.Connect(stream) gRPC bidi to the hub.
// - When something inside the hub cluster dials
//   `tunnel-server.mcp.svc.cluster.local:<PORT>` over TCP, that TCP
//   is tunnelled back through the spoke's existing gRPC bidi and
//   terminates at mcp-company1:8000 on the spoke.
// - Phase 1 authenticates with a pre-shared token; per-spoke port is
//   pinned statically via environment variables.
// - Phase 3 replaces the token check with Ed25519 signatures and
//   drives the port allocation from spoke_clusters.

use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{routing::get, Router};
use prometheus::{
    register_counter_vec_with_registry, register_gauge_vec_with_registry,
    register_gauge_with_registry, CounterVec, Encoder, Gauge, GaugeVec, Registry, TextEncoder,
};
use rand::RngCore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::transport::Server;
use tonic::{Request, Response, Status, Streaming};

pub mod tunnelpb {
    tonic::include_proto!("openspoke.tunnel.v1");
}

use tunnelpb::{
    client_frame, server_frame,
    tunnel_server::{Tunnel, TunnelServer},
    ClientFrame, CloseStream, Data, HandshakeChallenge, HttpRequest, HttpResponse,
    OpenStream, ServerFrame,
};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct SpokePortBinding {
    port: u16,
    target: String,
}

/// Spoke kind. Determined from the SPOKE_ID prefix ("app-" -> Application).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpokeKind {
    /// A conventional Kubernetes cluster spoke (e.g. spoke-example). TCP tunnel.
    Cluster,
    /// An application spoke (e.g. app-example). HTTP proxy to the hub kernel only.
    Application,
}

#[derive(Clone, Debug)]
struct SpokeConfig {
    spoke_id: String,
    kind: SpokeKind,
    ports: Vec<SpokePortBinding>,
    token: String,
}

#[derive(Debug)]
struct ServerConfig {
    grpc_listen: SocketAddr,
    metrics_listen: SocketAddr,
    spokes: HashMap<String, SpokeConfig>,
    /// Base URL of the hub kernel, used by application spokes.
    /// Defaults to the in-cluster rag-backend-kernel.
    kernel_url: String,
    /// Exact-match allowlist of paths that application spokes are
    /// permitted to POST to the hub kernel. Any other path is
    /// answered with 403.
    allowed_paths: std::collections::HashSet<String>,
}

fn env_default(k: &str, d: &str) -> String {
    env::var(k)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| d.to_string())
}

fn parse_listen(s: &str) -> Result<SocketAddr, String> {
    let s = if s.starts_with(':') {
        format!("0.0.0.0{}", s)
    } else {
        s.to_string()
    };
    s.parse()
        .map_err(|e: std::net::AddrParseError| format!("bad listen {}: {}", s, e))
}

fn load_config() -> Result<ServerConfig, String> {
    let grpc_listen = parse_listen(&env_default("TUNNEL_GRPC_LISTEN", ":8080"))?;
    let metrics_listen = parse_listen(&env_default("TUNNEL_METRICS_LISTEN", ":9090"))?;

    // TUNNEL_SPOKES format:
    //   legacy (1 spoke = 1 port): "spoke-example:10001:tokenA"
    //   new (1 spoke = multiple ports):
    //     "spoke-example:10001@default;10011@apiserver:tokenA"
    //     - Ports are separated by `;`.
    //     - `port@target` may omit the target; the default is "default".
    //   Multiple spokes are separated by `,` in either form.
    let raw = env::var("TUNNEL_SPOKES").unwrap_or_default();
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(
            "TUNNEL_SPOKES env is required (format: spoke-id:port[@target][;port[@target]...]:token,...)"
                .to_string(),
        );
    }

    let mut spokes: HashMap<String, SpokeConfig> = HashMap::new();
    for entry in raw.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        // Split into spoke-id : <port-spec> : token using splitn(3).
        let parts: Vec<&str> = entry.splitn(3, ':').collect();
        if parts.len() != 3 {
            return Err(format!("bad TUNNEL_SPOKES entry: {}", entry));
        }
        let spoke_id = parts[0].to_string();
        let port_spec = parts[1];
        let token = parts[2].to_string();

        // Application spokes are identified by the "app-" prefix on
        // the SPOKE_ID. This keeps the tunnel-server side free of
        // any OpenSearch lookup.
        let kind = if spoke_id.starts_with("app-") {
            SpokeKind::Application
        } else {
            SpokeKind::Cluster
        };

        let mut bindings: Vec<SpokePortBinding> = Vec::new();
        for pb in port_spec.split(';') {
            let pb = pb.trim();
            if pb.is_empty() {
                continue;
            }
            let (port_str, target) = if let Some(at) = pb.find('@') {
                let t = pb[at + 1..].trim();
                if t.is_empty() {
                    return Err(format!("empty target after '@' in {}", entry));
                }
                (&pb[..at], t)
            } else {
                (pb, "default")
            };
            let port: u16 = port_str
                .parse()
                .map_err(|e| format!("bad port {} in {}: {}", port_str, entry, e))?;
            bindings.push(SpokePortBinding {
                port,
                target: target.to_string(),
            });
        }
        // Application spokes may declare zero ports. Cluster spokes
        // must declare at least one.
        if kind == SpokeKind::Cluster && bindings.is_empty() {
            return Err(format!("no ports in {}", entry));
        }

        spokes.insert(
            spoke_id.clone(),
            SpokeConfig {
                spoke_id,
                kind,
                ports: bindings,
                token,
            },
        );
    }

    // Hub kernel URL and the path allowlist used by application spokes.
    let kernel_url = env_default(
        "KERNEL_URL",
        "http://rag-backend-kernel.rag-company1.svc.cluster.local:8000",
    );

    // Default allowlist (exact match) for application-spoke HTTP
    // requests. Streaming / vector-store / graph / handoff endpoints
    // are intentionally left out until per-application namespacing
    // is in place.
    let mut allowed_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    for p in [
        "/core/orchestrator/triage",
        "/core/orchestrator/answer",
        "/core/claude/generate-text",
        "/core/swallow/generate-text",
        "/core/jev/decide",
        "/core/embedding/single",
        "/core/embedding/batch",
        // App-spoke self-serve usage guidance. The kernel resolves
        // the per-spoke namespace from the x-openspoke-spoke-id
        // header that tunnel-server sets below.
        "/core/appspoke/usage-chat",
    ] {
        allowed_paths.insert(p.to_string());
    }
    // TUNNEL_ALLOWED_PATHS overrides the default list (comma-separated).
    if let Ok(raw) = env::var("TUNNEL_ALLOWED_PATHS") {
        let raw = raw.trim();
        if !raw.is_empty() {
            allowed_paths.clear();
            for p in raw.split(',') {
                let p = p.trim();
                if !p.is_empty() {
                    allowed_paths.insert(p.to_string());
                }
            }
        }
    }

    Ok(ServerConfig {
        grpc_listen,
        metrics_listen,
        spokes,
        kernel_url,
        allowed_paths,
    })
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

struct Metrics {
    registry: Registry,
    connected: Gauge,
    streams_open: GaugeVec,
    bytes_sent: CounterVec,
    bytes_recv: CounterVec,
    handshake_fail: CounterVec,
    /// Application spoke HTTP proxy count
    /// (labels: spoke_id, result = ok / rejected_path / rejected_kind / upstream_err).
    http_requests: CounterVec,
}

impl Metrics {
    fn new() -> Self {
        let registry = Registry::new();
        let connected = register_gauge_with_registry!(
            "openspoke_tunnel_connected_spokes",
            "Number of currently connected spokes.",
            registry
        )
        .expect("register connected");
        let streams_open = register_gauge_vec_with_registry!(
            "openspoke_tunnel_streams_open",
            "Open virtual TCP streams per spoke.",
            &["spoke_id"],
            registry
        )
        .expect("register streams_open");
        let bytes_sent = register_counter_vec_with_registry!(
            "openspoke_tunnel_bytes_sent_total",
            "Bytes sent hub->spoke via tunnel.",
            &["spoke_id"],
            registry
        )
        .expect("register bytes_sent");
        let bytes_recv = register_counter_vec_with_registry!(
            "openspoke_tunnel_bytes_recv_total",
            "Bytes received spoke->hub via tunnel.",
            &["spoke_id"],
            registry
        )
        .expect("register bytes_recv");
        let handshake_fail = register_counter_vec_with_registry!(
            "openspoke_tunnel_handshake_failures_total",
            "Handshake failures.",
            &["reason"],
            registry
        )
        .expect("register handshake_fail");
        let http_requests = register_counter_vec_with_registry!(
            "openspoke_tunnel_http_requests_total",
            "Application spoke HTTP requests proxied to hub kernel (labels: spoke_id, result).",
            &["spoke_id", "result"],
            registry
        )
        .expect("register http_requests");
        Self {
            registry,
            connected,
            streams_open,
            bytes_sent,
            bytes_recv,
            handshake_fail,
            http_requests,
        }
    }

    fn encode(&self) -> String {
        let mut buf = Vec::new();
        let encoder = TextEncoder::new();
        let mf = self.registry.gather();
        encoder.encode(&mf, &mut buf).ok();
        String::from_utf8(buf).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Session state
// ---------------------------------------------------------------------------

/// Active tunnel for one spoke.
struct SpokeSession {
    spoke_id: String,
    /// Sender used from this session to push frames back to the spoke.
    server_tx: mpsc::Sender<Result<ServerFrame, Status>>,
    /// Virtual TCP streams keyed by stream_id: sid -> writer-half tx.
    streams: Mutex<HashMap<u64, mpsc::Sender<Vec<u8>>>>,
    /// Stop flag for the TCP listeners tied to this session's lifetime.
    closed: AtomicBool,
}

impl SpokeSession {
    async fn send(&self, f: ServerFrame) -> Result<(), ()> {
        self.server_tx.send(Ok(f)).await.map_err(|_| ())
    }

    async fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        // Drop every per-stream writer channel.
        let mut streams = self.streams.lock().await;
        streams.clear();
    }
}

// ---------------------------------------------------------------------------
// gRPC service
// ---------------------------------------------------------------------------

struct TunnelSvc {
    cfg: Arc<ServerConfig>,
    sessions: Arc<Mutex<HashMap<String, Arc<SpokeSession>>>>,
    next_sid: Arc<AtomicU64>,
    metrics: Arc<Metrics>,
    /// reqwest client used to proxy application-spoke HTTP calls
    /// to the hub kernel.
    http_client: reqwest::Client,
}

#[tonic::async_trait]
impl Tunnel for TunnelSvc {
    type ConnectStream = ReceiverStream<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        // Slightly larger channel: accept bursts can pile up 32 KB
        // frames in parallel.
        let (server_tx, server_rx) = mpsc::channel::<Result<ServerFrame, Status>>(256);
        let client_stream = request.into_inner();

        // Send the Challenge.
        let mut nonce = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut nonce);
        let issued_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        if server_tx
            .send(Ok(ServerFrame {
                stream_id: 0,
                kind: Some(server_frame::Kind::Challenge(HandshakeChallenge {
                    nonce: nonce.to_vec(),
                    issued_at,
                })),
            }))
            .await
            .is_err()
        {
            self.metrics
                .handshake_fail
                .with_label_values(&["send_challenge"])
                .inc();
            return Err(Status::internal("send challenge failed"));
        }

        // Handshake and receive processing run in a spawned task so
        // this call can return the response stream. The Challenge is
        // already queued on the response stream, so tonic starts
        // delivering it to the client immediately, and the client's
        // HandshakeResponse arrives right after.
        let cfg = self.cfg.clone();
        let sessions = self.sessions.clone();
        let next_sid = self.next_sid.clone();
        let metrics = self.metrics.clone();
        let http_client = self.http_client.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(
                cfg,
                sessions,
                next_sid,
                metrics,
                http_client,
                server_tx,
                client_stream,
            )
            .await
            {
                tracing::info!(err = %e, "recv end");
            }
        });

        Ok(Response::new(ReceiverStream::new(server_rx)))
    }
}

async fn handle_connection(
    cfg: Arc<ServerConfig>,
    sessions: Arc<Mutex<HashMap<String, Arc<SpokeSession>>>>,
    next_sid: Arc<AtomicU64>,
    metrics: Arc<Metrics>,
    http_client: reqwest::Client,
    server_tx: mpsc::Sender<Result<ServerFrame, Status>>,
    mut client_stream: Streaming<ClientFrame>,
) -> Result<(), String> {
    // 1. Wait for HandshakeResponse (first client frame).
    let first = match client_stream.next().await {
        Some(Ok(f)) => f,
        Some(Err(e)) => {
            metrics
                .handshake_fail
                .with_label_values(&["recv_first"])
                .inc();
            return Err(format!("recv first: {}", e));
        }
        None => {
            metrics
                .handshake_fail
                .with_label_values(&["recv_first"])
                .inc();
            return Err("recv first: stream closed".to_string());
        }
    };
    let hs = match first.kind {
        Some(client_frame::Kind::Handshake(hs)) => hs,
        _ => {
            metrics
                .handshake_fail
                .with_label_values(&["no_handshake"])
                .inc();
            return Err("expected HandshakeResponse".to_string());
        }
    };

    // 2. Look up the spoke and verify its Phase 1 pre-shared token.
    let sc = match cfg.spokes.get(&hs.spoke_id) {
        Some(sc) => sc.clone(),
        None => {
            metrics
                .handshake_fail
                .with_label_values(&["unknown_spoke"])
                .inc();
            return Err(format!("unknown spoke_id: {}", hs.spoke_id));
        }
    };
    // signature is the raw pre-shared token in Phase 1.
    if hs.signature != sc.token.as_bytes() {
        metrics
            .handshake_fail
            .with_label_values(&["bad_token"])
            .inc();
        return Err(format!("bad token for {}", hs.spoke_id));
    }
    // Phase 3 will add a nonce replay check and an issued_at delta check.

    // 3. Register the session (an older connection with the same
    //    spoke_id is displaced).
    let session = Arc::new(SpokeSession {
        spoke_id: hs.spoke_id.clone(),
        server_tx: server_tx.clone(),
        streams: Mutex::new(HashMap::new()),
        closed: AtomicBool::new(false),
    });
    {
        let mut map = sessions.lock().await;
        if let Some(old) = map.get(&hs.spoke_id) {
            tracing::warn!(spoke_id = %hs.spoke_id, "displacing existing session");
            let old_clone = old.clone();
            drop(map);
            old_clone.close().await;
            let mut map = sessions.lock().await;
            map.insert(hs.spoke_id.clone(), session.clone());
        } else {
            map.insert(hs.spoke_id.clone(), session.clone());
        }
    }
    metrics.connected.inc();

    // 4. Start a TCP listener per configured spoke port, tied to
    //    this session's lifetime.
    let mut listener_handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();
    let mut listen_error: Option<String> = None;
    for binding in &sc.ports {
        let addr: SocketAddr = format!("0.0.0.0:{}", binding.port)
            .parse()
            .map_err(|e| format!("bad port {}: {}", binding.port, e))?;
        match TcpListener::bind(addr).await {
            Ok(ln) => {
                tracing::info!(
                    spoke_id = %hs.spoke_id,
                    port = binding.port,
                    target = %binding.target,
                    "spoke port listen"
                );
                let session_clone = session.clone();
                let target = binding.target.clone();
                let next_sid_clone = next_sid.clone();
                let metrics_clone = metrics.clone();
                let handle = tokio::spawn(accept_loop(
                    session_clone,
                    ln,
                    target,
                    next_sid_clone,
                    metrics_clone,
                ));
                listener_handles.push(handle);
            }
            Err(e) => {
                listen_error = Some(format!("listen {}: {}", binding.port, e));
                break;
            }
        }
    }

    if let Some(e) = listen_error {
        // Any listeners already opened will terminate on their own
        // once session.closed flips.
        session.close().await;
        cleanup_session(&sessions, &hs.spoke_id, &metrics).await;
        return Err(e);
    }

    tracing::info!(
        spoke_id = %hs.spoke_id,
        port_count = sc.ports.len(),
        ver = %hs.tunnel_client_ver,
        "spoke connected"
    );

    // 5. Drive the receive loop for this connection.
    let ret = recv_loop(
        session.clone(),
        &sc,
        &cfg,
        &http_client,
        &mut client_stream,
        &metrics,
    )
    .await;

    // Cleanup.
    session.close().await;
    cleanup_session(&sessions, &hs.spoke_id, &metrics).await;
    // accept_loop watches session.closed, so we abort them rather
    // than join.
    for h in listener_handles {
        h.abort();
    }
    ret
}

async fn cleanup_session(
    sessions: &Arc<Mutex<HashMap<String, Arc<SpokeSession>>>>,
    spoke_id: &str,
    metrics: &Arc<Metrics>,
) {
    let mut map = sessions.lock().await;
    if let Some(cur) = map.get(spoke_id) {
        // Only remove ourselves when we're still the registered
        // session (a displacing connection may have replaced us).
        if cur.closed.load(Ordering::SeqCst) {
            map.remove(spoke_id);
            metrics.connected.dec();
        }
    }
}

async fn accept_loop(
    session: Arc<SpokeSession>,
    ln: TcpListener,
    target: String,
    next_sid: Arc<AtomicU64>,
    metrics: Arc<Metrics>,
) {
    loop {
        if session.closed.load(Ordering::SeqCst) {
            return;
        }
        let (conn, peer) = match ln.accept().await {
            Ok(v) => v,
            Err(e) => {
                if session.closed.load(Ordering::SeqCst) {
                    return;
                }
                tracing::warn!(
                    spoke_id = %session.spoke_id,
                    target = %target,
                    err = %e,
                    "accept failed"
                );
                return;
            }
        };
        let sid = next_sid.fetch_add(1, Ordering::SeqCst) + 1;
        tracing::info!(
            spoke_id = %session.spoke_id,
            target = %target,
            sid,
            remote = %peer,
            "accept new conn"
        );

        // Create and register a per-stream writer channel.
        let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>(64);
        {
            let mut streams = session.streams.lock().await;
            streams.insert(sid, write_tx);
        }
        metrics
            .streams_open
            .with_label_values(&[&session.spoke_id])
            .inc();

        // Send the Open frame.
        if session
            .send(ServerFrame {
                stream_id: sid,
                kind: Some(server_frame::Kind::Open(OpenStream {
                    protocol: "tcp".to_string(),
                    target: target.clone(),
                })),
            })
            .await
            .is_err()
        {
            tracing::warn!(sid, "send open failed");
            // Roll back the registration.
            let mut streams = session.streams.lock().await;
            streams.remove(&sid);
            metrics
                .streams_open
                .with_label_values(&[&session.spoke_id])
                .dec();
            continue;
        }
        tracing::info!(
            spoke_id = %session.spoke_id,
            sid,
            target = %target,
            "open frame sent"
        );

        // Split TCP into read/write halves.
        let (mut r, mut w) = conn.into_split();

        // Upstream (TCP -> spoke Data frame).
        let session_up = session.clone();
        let metrics_up = metrics.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 32 * 1024];
            loop {
                let n = match r.read(&mut buf).await {
                    Ok(0) => 0,
                    Ok(n) => n,
                    Err(e) => {
                        tracing::debug!(err = %e, "tcp read err");
                        break;
                    }
                };
                if n == 0 {
                    // EOF -> half-close notification.
                    let _ = session_up
                        .send(ServerFrame {
                            stream_id: sid,
                            kind: Some(server_frame::Kind::Close(CloseStream {
                                half: true,
                                reason: "eof".to_string(),
                            })),
                        })
                        .await;
                    break;
                }
                metrics_up
                    .bytes_sent
                    .with_label_values(&[&session_up.spoke_id])
                    .inc_by(n as f64);
                if session_up
                    .send(ServerFrame {
                        stream_id: sid,
                        kind: Some(server_frame::Kind::Data(Data {
                            payload: buf[..n].to_vec(),
                        })),
                    })
                    .await
                    .is_err()
                {
                    tracing::warn!(sid, "send data failed");
                    break;
                }
            }
            // Upstream done. Drop the downstream writer channel.
            let mut streams = session_up.streams.lock().await;
            if streams.remove(&sid).is_some() {
                metrics_up
                    .streams_open
                    .with_label_values(&[&session_up.spoke_id])
                    .dec();
            }
        });

        // Downstream (spoke Data -> TCP).
        let session_dn = session.clone();
        tokio::spawn(async move {
            let mut write_rx = write_rx;
            while let Some(payload) = write_rx.recv().await {
                if w.write_all(&payload).await.is_err() {
                    break;
                }
            }
            // recv_loop drops the channel on close, which propagates
            // here so the TCP writer shuts down cleanly.
            let _ = w.shutdown().await;
            // Stream removal is handled by the upstream task or by
            // recv_loop.
            let _ = session_dn;
        });
    }
}

async fn recv_loop(
    session: Arc<SpokeSession>,
    sc: &SpokeConfig,
    cfg: &Arc<ServerConfig>,
    http_client: &reqwest::Client,
    client_stream: &mut Streaming<ClientFrame>,
    metrics: &Arc<Metrics>,
) -> Result<(), String> {
    loop {
        let f = match client_stream.next().await {
            Some(Ok(f)) => f,
            Some(Err(e)) => {
                tracing::info!(spoke_id = %session.spoke_id, err = %e, "recv end");
                return Ok(());
            }
            None => {
                tracing::info!(spoke_id = %session.spoke_id, "recv end (stream closed)");
                return Ok(());
            }
        };
        let sid = f.stream_id;
        match f.kind {
            Some(client_frame::Kind::Data(d)) => {
                let payload = d.payload;
                let writer = {
                    let streams = session.streams.lock().await;
                    streams.get(&sid).cloned()
                };
                match writer {
                    Some(w) => {
                        tracing::info!(
                            spoke_id = %session.spoke_id,
                            sid,
                            n = payload.len(),
                            "recv data from spoke"
                        );
                        metrics
                            .bytes_recv
                            .with_label_values(&[&session.spoke_id])
                            .inc_by(payload.len() as f64);
                        if w.send(payload).await.is_err() {
                            tracing::warn!(sid, "tcp write chan closed");
                            let mut streams = session.streams.lock().await;
                            if streams.remove(&sid).is_some() {
                                metrics
                                    .streams_open
                                    .with_label_values(&[&session.spoke_id])
                                    .dec();
                            }
                        }
                    }
                    None => {
                        tracing::warn!(
                            spoke_id = %session.spoke_id,
                            sid,
                            n = payload.len(),
                            "data for unknown stream"
                        );
                    }
                }
            }
            Some(client_frame::Kind::Close(_)) => {
                tracing::info!(
                    spoke_id = %session.spoke_id,
                    sid,
                    "recv close from spoke"
                );
                let mut streams = session.streams.lock().await;
                if streams.remove(&sid).is_some() {
                    metrics
                        .streams_open
                        .with_label_values(&[&session.spoke_id])
                        .dec();
                }
            }
            Some(client_frame::Kind::Pong(_)) => {
                // TODO: RTT metric
            }
            Some(client_frame::Kind::HttpRequest(req)) => {
                // Application-spoke HTTP proxy to the hub kernel.
                // Three-stage check: kind=Application, path allowlist,
                // and forwarding to the hub kernel.
                if sc.kind != SpokeKind::Application {
                    tracing::warn!(
                        spoke_id = %session.spoke_id,
                        sid,
                        path = %req.path,
                        "http_request from non-application spoke; rejecting"
                    );
                    metrics
                        .http_requests
                        .with_label_values(&[&session.spoke_id, "rejected_kind"])
                        .inc();
                    let _ = session
                        .send(ServerFrame {
                            stream_id: sid,
                            kind: Some(server_frame::Kind::HttpResponse(HttpResponse {
                                status: 403,
                                headers: Default::default(),
                                body: b"application spoke required".to_vec(),
                                error: "rejected_kind".to_string(),
                            })),
                        })
                        .await;
                    continue;
                }
                let session_c = session.clone();
                let cfg_c = cfg.clone();
                let client_c = http_client.clone();
                let metrics_c = metrics.clone();
                tokio::spawn(async move {
                    handle_http_request(sid, req, session_c, cfg_c, client_c, metrics_c).await;
                });
            }
            Some(client_frame::Kind::Handshake(_)) => {
                // A second Handshake mid-stream is a protocol
                // violation. We log it and continue.
                tracing::warn!(spoke_id = %session.spoke_id, "unexpected handshake mid-stream");
            }
            None => {
                // Empty frame.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Application-spoke HttpRequest handling
// ---------------------------------------------------------------------------

async fn handle_http_request(
    sid: u64,
    req: HttpRequest,
    session: Arc<SpokeSession>,
    cfg: Arc<ServerConfig>,
    http_client: reqwest::Client,
    metrics: Arc<Metrics>,
) {
    // 1. Exact-match path allowlist check.
    if !cfg.allowed_paths.contains(&req.path) {
        tracing::warn!(
            spoke_id = %session.spoke_id,
            sid,
            path = %req.path,
            "path not in allowed list; rejecting"
        );
        metrics
            .http_requests
            .with_label_values(&[&session.spoke_id, "rejected_path"])
            .inc();
        let _ = session
            .send(ServerFrame {
                stream_id: sid,
                kind: Some(server_frame::Kind::HttpResponse(HttpResponse {
                    status: 403,
                    headers: Default::default(),
                    body: format!("path not allowed: {}", req.path).into_bytes(),
                    error: "rejected_path".to_string(),
                })),
            })
            .await;
        return;
    }

    // 2. Forward to the hub kernel. The URL host/scheme are pinned
    //    here; the spoke cannot direct the request elsewhere.
    let url = format!("{}{}", cfg.kernel_url, req.path);
    let method = reqwest::Method::from_bytes(req.method.as_bytes())
        .unwrap_or(reqwest::Method::GET);
    let mut builder = http_client.request(method, &url);
    for (k, v) in req.headers.iter() {
        // Skip hop-by-hop "host" (reqwest derives it from the URL).
        if k.eq_ignore_ascii_case("host") {
            continue;
        }
        // x-openspoke-* are reserved headers set by the hub; drop
        // any that the spoke tries to send to avoid spoofing.
        if k.to_ascii_lowercase().starts_with("x-openspoke-") {
            tracing::warn!(
                spoke_id = %session.spoke_id,
                sid,
                header = %k,
                "reserved header from spoke dropped"
            );
            continue;
        }
        builder = builder.header(k, v);
    }
    // Pass the authenticated spoke identity through to the kernel
    // (the kernel uses it to resolve the per-spoke namespace).
    builder = builder.header("x-openspoke-spoke-id", session.spoke_id.as_str());
    let builder = builder.body(req.body);

    tracing::info!(
        spoke_id = %session.spoke_id,
        sid,
        path = %req.path,
        url = %url,
        "forwarding to hub kernel"
    );

    let resp = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                spoke_id = %session.spoke_id,
                sid,
                err = %e,
                "upstream error"
            );
            metrics
                .http_requests
                .with_label_values(&[&session.spoke_id, "upstream_err"])
                .inc();
            let _ = session
                .send(ServerFrame {
                    stream_id: sid,
                    kind: Some(server_frame::Kind::HttpResponse(HttpResponse {
                        status: 0,
                        headers: Default::default(),
                        body: Vec::new(),
                        error: format!("upstream error: {}", e),
                    })),
                })
                .await;
            return;
        }
    };

    let status = resp.status().as_u16() as i32;
    let mut headers: HashMap<String, String> = HashMap::new();
    for (k, v) in resp.headers().iter() {
        if let Ok(s) = v.to_str() {
            headers.insert(k.as_str().to_string(), s.to_string());
        }
    }
    let body = match resp.bytes().await {
        Ok(b) => b.to_vec(),
        Err(e) => {
            tracing::warn!(
                spoke_id = %session.spoke_id,
                sid,
                err = %e,
                "upstream body read error"
            );
            metrics
                .http_requests
                .with_label_values(&[&session.spoke_id, "upstream_err"])
                .inc();
            let _ = session
                .send(ServerFrame {
                    stream_id: sid,
                    kind: Some(server_frame::Kind::HttpResponse(HttpResponse {
                        status: 0,
                        headers: Default::default(),
                        body: Vec::new(),
                        error: format!("upstream body error: {}", e),
                    })),
                })
                .await;
            return;
        }
    };

    metrics
        .http_requests
        .with_label_values(&[&session.spoke_id, "ok"])
        .inc();

    let _ = session
        .send(ServerFrame {
            stream_id: sid,
            kind: Some(server_frame::Kind::HttpResponse(HttpResponse {
                status,
                headers,
                body,
                error: String::new(),
            })),
        })
        .await;
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // JSON on stdout, matching the previous Go implementation's
    // slog handler.
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = Arc::new(load_config().map_err(|e| {
        tracing::error!(err = %e, "config");
        e
    })?);
    let metrics = Arc::new(Metrics::new());

    // /metrics and /healthz endpoints.
    let metrics_for_axum = metrics.clone();
    let metrics_listen = cfg.metrics_listen;
    tokio::spawn(async move {
        let app = Router::new()
            .route(
                "/metrics",
                get({
                    let m = metrics_for_axum.clone();
                    move || {
                        let m = m.clone();
                        async move { m.encode() }
                    }
                }),
            )
            .route("/healthz", get(|| async { "ok" }));
        tracing::info!(addr = %metrics_listen, "metrics listening");
        match tokio::net::TcpListener::bind(metrics_listen).await {
            Ok(l) => {
                if let Err(e) = axum::serve(l, app.into_make_service()).await {
                    tracing::error!(err = %e, "metrics server");
                }
            }
            Err(e) => {
                tracing::error!(err = %e, "metrics bind");
            }
        }
    });

    tracing::info!(
        grpc = %cfg.grpc_listen,
        spokes = cfg.spokes.len(),
        "tunnel-server ready"
    );

    // Application-spoke HTTP client (5 min timeout is generous for
    // long kernel calls like /core/claude/generate-text).
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| {
            tracing::error!(err = %e, "build http client");
            e
        })?;

    let svc = TunnelSvc {
        cfg: cfg.clone(),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        next_sid: Arc::new(AtomicU64::new(0)),
        metrics: metrics.clone(),
        http_client,
    };

    Server::builder()
        .initial_stream_window_size(Some(4 * 1024 * 1024))
        .initial_connection_window_size(Some(16 * 1024 * 1024))
        .http2_keepalive_interval(Some(Duration::from_secs(30)))
        .http2_keepalive_timeout(Some(Duration::from_secs(20)))
        .add_service(TunnelServer::new(svc))
        .serve(cfg.grpc_listen)
        .await?;

    Ok(())
}
