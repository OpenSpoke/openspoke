// tunnel-client: OpenSpoke spoke -> hub reverse tunnel, spoke side (Rust).
//
// - Opens a gRPC connection to the hub URL and runs Tunnel.Connect(stream).
// - Answers HandshakeChallenge with SPOKE_ID + TOKEN.
// - For each OpenStream from the hub, dials the matching host:port from
//   TARGETS and forwards bytes in both directions by stream_id.
// - Reconnects automatically with exponential backoff when the stream
//   goes down.
//
// Phase 1 authenticates with a pre-shared token; Phase 3 replaces the
// token with an Ed25519 signature over (nonce || spoke_id || issued_at).
//
// This is the Rust rewrite of the original Go tunnel-client. The wire
// protocol, environment variables, and log format match the Go version.

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::time::{sleep, timeout};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::{ClientTlsConfig, Endpoint};

pub mod tunnelpb {
    tonic::include_proto!("openspoke.tunnel.v1");
}

use tunnelpb::{
    client_frame, server_frame, tunnel_client::TunnelClient, ClientFrame, CloseStream, Data,
    HandshakeResponse, HttpRequest as PbHttpRequest, HttpResponse as PbHttpResponse, Pong,
};

const CLIENT_VERSION: &str = "tc-rust/0.1.0";

// ---------------------------------------------------------------------------
// config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ClientConfig {
    hub_url: String,
    spoke_id: String,
    token: String,
    targets: HashMap<String, String>,
    insecure: bool,
    backoff_min: Duration,
    backoff_max: Duration,
    // Some only when KERNEL_PROXY_LISTEN is set (e.g. "0.0.0.0:8080").
    // Used by the application-spoke HTTP proxy mode.
    kernel_proxy_listen: Option<String>,
}

// Shared state for the HTTP proxy mode.
//   - hub_tx: send-side of the current session (Some while connected,
//     None while reconnecting).
//   - pending: HttpRequests waiting for a response, keyed by stream_id.
//   - stream_id_counter: allocates ids from 2^63 upward so they never
//     collide with the hub-side TCP stream ids that start at 1.
type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<PbHttpResponse>>>>;

struct SharedProxyState {
    hub_tx: RwLock<Option<mpsc::Sender<ClientFrame>>>,
    pending: PendingMap,
    stream_id_counter: AtomicU64,
}

impl SharedProxyState {
    fn new() -> Self {
        Self {
            hub_tx: RwLock::new(None),
            pending: Arc::new(Mutex::new(HashMap::new())),
            // 2^63 = 9223372036854775808. Ids run from here up to u64::MAX.
            stream_id_counter: AtomicU64::new(1u64 << 63),
        }
    }

    fn next_stream_id(&self) -> u64 {
        self.stream_id_counter.fetch_add(1, Ordering::SeqCst)
    }

    // Called at the start of a session with the outbound sender.
    async fn attach_hub(&self, tx: mpsc::Sender<ClientFrame>) {
        *self.hub_tx.write().await = Some(tx);
    }

    // Called at the end of a session. Drops the outbound sender and
    // releases every pending request with 503, so callers do not
    // wait through the reconnect backoff for nothing.
    async fn detach_hub_and_flush(&self) {
        *self.hub_tx.write().await = None;
        let mut map = self.pending.lock().await;
        let drained: Vec<(u64, oneshot::Sender<PbHttpResponse>)> = map.drain().collect();
        drop(map);
        for (_sid, tx) in drained {
            let _ = tx.send(PbHttpResponse {
                status: 503,
                headers: HashMap::new(),
                body: b"tunnel session ended".to_vec(),
                error: "tunnel session ended".to_string(),
            });
        }
    }
}

fn load_config() -> Result<ClientConfig, String> {
    let hub_url = env::var("HUB_URL").unwrap_or_default();
    let spoke_id = env::var("SPOKE_ID").unwrap_or_default();
    let token = env::var("TUNNEL_TOKEN").unwrap_or_default();
    let target_single = env::var("TARGET").unwrap_or_default();
    let targets_raw = env::var("TARGETS").unwrap_or_default();
    let insecure = env::var("TUNNEL_INSECURE")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let kernel_proxy_listen = env::var("KERNEL_PROXY_LISTEN")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let mut targets: HashMap<String, String> = HashMap::new();
    // TARGETS format:
    //   "default=mcp-company1.rag-spoke.svc.cluster.local:8000,\
    //    apiserver=kubernetes.default.svc:443"
    // Empty is allowed (the legacy TARGET on its own also works).
    let raw = targets_raw.trim();
    if !raw.is_empty() {
        for entry in raw.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let eq = match entry.find('=') {
                Some(i) => i,
                None => {
                    return Err(format!(
                        "bad TARGETS entry (expected name=host:port): {:?}",
                        entry
                    ))
                }
            };
            let name = entry[..eq].trim().to_string();
            let host_port = entry[eq + 1..].trim().to_string();
            if name.is_empty() || host_port.is_empty() {
                return Err(format!(
                    "empty name or host:port in TARGETS entry: {:?}",
                    entry
                ));
            }
            targets.insert(name, host_port);
        }
    }
    // Legacy TARGET env maps to "default" (TARGETS wins if it also
    // sets "default").
    if !target_single.is_empty() {
        targets
            .entry("default".to_string())
            .or_insert(target_single);
    }

    if hub_url.is_empty() || spoke_id.is_empty() || token.is_empty() {
        return Err("HUB_URL / SPOKE_ID / TUNNEL_TOKEN are required".to_string());
    }
    // Application spokes (SPOKE_ID starts with "app-") do not open
    // TCP tunnels and therefore need no TARGET / TARGETS. Every
    // other spoke must declare at least one.
    if targets.is_empty() && !spoke_id.starts_with("app-") {
        return Err("TARGET or TARGETS is required".to_string());
    }

    Ok(ClientConfig {
        hub_url,
        spoke_id,
        token,
        targets,
        insecure,
        backoff_min: Duration::from_secs(1),
        backoff_max: Duration::from_secs(30),
        kernel_proxy_listen,
    })
}

fn client_http_request(sid: u64, req: PbHttpRequest) -> ClientFrame {
    ClientFrame {
        stream_id: sid,
        kind: Some(client_frame::Kind::HttpRequest(req)),
    }
}

// ---------------------------------------------------------------------------
// Frame construction helpers
// ---------------------------------------------------------------------------

type StreamMap = Arc<Mutex<HashMap<u64, mpsc::Sender<Vec<u8>>>>>;

fn client_close(sid: u64, half: bool, reason: impl Into<String>) -> ClientFrame {
    ClientFrame {
        stream_id: sid,
        kind: Some(client_frame::Kind::Close(CloseStream {
            half,
            reason: reason.into(),
        })),
    }
}

fn client_data(sid: u64, payload: Vec<u8>) -> ClientFrame {
    ClientFrame {
        stream_id: sid,
        kind: Some(client_frame::Kind::Data(Data { payload })),
    }
}

fn client_handshake(spoke_id: &str, token: &str) -> ClientFrame {
    ClientFrame {
        stream_id: 0,
        kind: Some(client_frame::Kind::Handshake(HandshakeResponse {
            spoke_id: spoke_id.to_string(),
            // Phase 1: signature = pre-shared token bytes
            // Phase 3: signature = Ed25519(nonce || spoke_id || issued_at)
            signature: token.as_bytes().to_vec(),
            tunnel_client_ver: CLIENT_VERSION.to_string(),
        })),
    }
}

fn client_pong(sent_at: i64) -> ClientFrame {
    ClientFrame {
        stream_id: 0,
        kind: Some(client_frame::Kind::Pong(Pong { sent_at })),
    }
}

// ---------------------------------------------------------------------------
// Per-stream pump: hub <-> target TCP, both directions.
// ---------------------------------------------------------------------------

async fn run_stream(
    sid: u64,
    target_name: String,
    target: String,
    hub_tx: mpsc::Sender<ClientFrame>,
    mut inbound: mpsc::Receiver<Vec<u8>>,
    streams: StreamMap,
) {
    tracing::info!(sid, target_name = %target_name, target = %target, "open dial start");
    let conn = match timeout(Duration::from_secs(5), TcpStream::connect(&target)).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            tracing::warn!(sid, target = %target, err = %e, "dial target failed");
            let _ = hub_tx.send(client_close(sid, false, e.to_string())).await;
            streams.lock().await.remove(&sid);
            return;
        }
        Err(_) => {
            tracing::warn!(sid, target = %target, "dial target failed (timeout)");
            let _ = hub_tx
                .send(client_close(sid, false, "dial timeout"))
                .await;
            streams.lock().await.remove(&sid);
            return;
        }
    };
    let local = conn
        .local_addr()
        .map(|a| a.to_string())
        .unwrap_or_else(|_| "?".to_string());
    tracing::info!(sid, local = %local, "open dial ok");

    let (mut rd, mut wr) = conn.into_split();
    let mut buf = vec![0u8; 32 * 1024];

    loop {
        tokio::select! {
            // TCP -> hub
            n = rd.read(&mut buf) => {
                match n {
                    Ok(0) => {
                        tracing::info!(sid, "pump read err: eof");
                        let _ = hub_tx.send(client_close(sid, true, "eof")).await;
                        break;
                    }
                    Ok(n) => {
                        tracing::info!(sid, n, "pump read");
                        let payload = buf[..n].to_vec();
                        if hub_tx.send(client_data(sid, payload)).await.is_err() {
                            tracing::warn!(sid, "send data failed (hub_tx closed)");
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::info!(sid, err = %e, "pump read err");
                        let _ = hub_tx.send(client_close(sid, true, "eof")).await;
                        break;
                    }
                }
            }
            // hub -> TCP
            payload = inbound.recv() => {
                match payload {
                    Some(p) => {
                        if let Err(e) = wr.write_all(&p).await {
                            tracing::warn!(sid, err = %e, "write to target failed");
                            break;
                        }
                    }
                    None => {
                        // The hub sent Close and the map dropped the
                        // channel. Shut the TCP side down quietly.
                        break;
                    }
                }
            }
        }
    }

    tracing::info!(sid, "pump end");
    streams.lock().await.remove(&sid);
    // rd / wr drop here, sending TCP FIN.
}

// ---------------------------------------------------------------------------
// HTTP proxy mode (only when KERNEL_PROXY_LISTEN is set).
// ---------------------------------------------------------------------------

async fn handle_http_proxy(
    axum::extract::State(shared): axum::extract::State<Arc<SharedProxyState>>,
    req: axum::extract::Request,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let method = req.method().to_string();
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    // Copy headers. The proto's map<string,string> only takes UTF-8
    // values, so non-ASCII values are skipped. In practice the fields
    // that matter (Content-Type, Accept, ...) are all ASCII.
    let mut headers: HashMap<String, String> = HashMap::new();
    for (name, value) in req.headers().iter() {
        if let Ok(v) = value.to_str() {
            headers.insert(name.to_string(), v.to_string());
        }
    }

    // Body cap: 32 MiB. Applications that need to upload larger
    // objects should hit their object store directly rather than
    // routing them through the tunnel.
    let body_bytes = match axum::body::to_bytes(req.into_body(), 32 * 1024 * 1024).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(err = %e, "failed to read request body");
            return (StatusCode::BAD_REQUEST, format!("body read failed: {e}")).into_response();
        }
    };
    let body = body_bytes.to_vec();

    // Grab the current hub sender. If we are between sessions, fail
    // fast with 503 so the caller does not sit through the backoff.
    let hub_tx_opt = shared.hub_tx.read().await.clone();
    let hub_tx = match hub_tx_opt {
        Some(t) => t,
        None => {
            return (StatusCode::SERVICE_UNAVAILABLE, "tunnel not connected").into_response();
        }
    };

    // Allocate a stream id from the >= 2^63 range. This never
    // collides with the hub-side TCP ids that start at 1.
    let sid = shared.next_stream_id();

    // Register the oneshot before we send, so a very fast response
    // can never race the insert.
    let (resp_tx, resp_rx) = oneshot::channel::<PbHttpResponse>();
    shared.pending.lock().await.insert(sid, resp_tx);

    let http_req = PbHttpRequest {
        method: method.clone(),
        path: path_and_query.clone(),
        headers,
        body,
    };
    tracing::info!(sid, method = %method, path = %path_and_query, "send http_request");

    if hub_tx.send(client_http_request(sid, http_req)).await.is_err() {
        shared.pending.lock().await.remove(&sid);
        return (StatusCode::SERVICE_UNAVAILABLE, "tunnel send failed").into_response();
    }

    // Wait for the response. 60s is shorter than the tunnel-server's
    // upstream reqwest timeout (300s), which is fine: the hub kernel
    // paths that application spokes are allowed to hit (triage,
    // embedding, small generation) all finish well within a minute.
    let pb_resp = match timeout(Duration::from_secs(60), resp_rx).await {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => {
            // Not reachable in practice: detach_hub_and_flush would
            // have replied 503 before dropping the sender. Kept as
            // a safety net.
            return (StatusCode::BAD_GATEWAY, "response channel closed").into_response();
        }
        Err(_) => {
            shared.pending.lock().await.remove(&sid);
            return (StatusCode::GATEWAY_TIMEOUT, "hub response timeout").into_response();
        }
    };

    // The proto sends status as int32 and adds an error field. A
    // non-empty error paired with status=0 (upstream unreachable /
    // body read failure) surfaces as 502 Bad Gateway with the
    // error string in the body; other statuses go through as-is.
    let (status, headers_out, body_out) = if pb_resp.error.is_empty() {
        (
            axum::http::StatusCode::from_u16(pb_resp.status as u16)
                .unwrap_or(StatusCode::BAD_GATEWAY),
            pb_resp.headers,
            pb_resp.body,
        )
    } else if pb_resp.status > 0 {
        // Hub reached the server but the server chose to reject
        // (e.g. rejected_path, rejected_kind). Pass the status and
        // include the error text as the body if the server left it
        // empty.
        let body = if pb_resp.body.is_empty() {
            pb_resp.error.into_bytes()
        } else {
            pb_resp.body
        };
        (
            axum::http::StatusCode::from_u16(pb_resp.status as u16)
                .unwrap_or(StatusCode::BAD_GATEWAY),
            pb_resp.headers,
            body,
        )
    } else {
        // Upstream unreachable. Turn it into 502 with the error text.
        (
            StatusCode::BAD_GATEWAY,
            HashMap::new(),
            pb_resp.error.into_bytes(),
        )
    };

    let mut builder = axum::http::Response::builder().status(status);
    for (name, value) in &headers_out {
        if let (Ok(n), Ok(v)) = (
            axum::http::HeaderName::try_from(name.as_str()),
            axum::http::HeaderValue::try_from(value.as_str()),
        ) {
            builder = builder.header(n, v);
        }
    }
    builder
        .body(axum::body::Body::from(body_out))
        .unwrap_or_else(|e| {
            tracing::warn!(err = %e, "failed to build response");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("build failed: {e}"),
            )
                .into_response()
        })
}

async fn run_http_proxy(
    listen: String,
    shared: Arc<SharedProxyState>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use axum::routing::any;
    use axum::Router;

    let app: Router = Router::new()
        .fallback(any(handle_http_proxy))
        .with_state(shared);

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    tracing::info!(listen = %listen, "kernel-proxy HTTP server listening");
    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Session (one gRPC stream).
// ---------------------------------------------------------------------------

async fn run_session(
    cfg: &ClientConfig,
    shared: &Arc<SharedProxyState>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // detach_hub_and_flush must always run, even on an early ? return
    // from the impl. Wrapping it here means the pending map is never
    // left holding stale senders.
    let result = run_session_impl(cfg, shared).await;
    shared.detach_hub_and_flush().await;
    result
}

async fn run_session_impl(
    cfg: &ClientConfig,
    shared: &Arc<SharedProxyState>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let scheme = if cfg.insecure { "http" } else { "https" };
    let uri_string = format!("{}://{}", scheme, cfg.hub_url);

    // gRPC transport tuning:
    //   InitialWindowSize     = 4 MiB (per stream)
    //   InitialConnWindowSize = 16 MiB (per connection)
    //   Keepalive: interval=30s, timeout=20s, while_idle=true
    let mut endpoint = Endpoint::from_shared(uri_string.clone())?
        .initial_stream_window_size(4 * 1024 * 1024)
        .initial_connection_window_size(16 * 1024 * 1024)
        .keep_alive_while_idle(true)
        .http2_keep_alive_interval(Duration::from_secs(30))
        .keep_alive_timeout(Duration::from_secs(20));

    if !cfg.insecure {
        // The hub TLS is terminated by whatever fronts hub_url (an
        // ingress controller, a CDN edge, etc.). A plain TLS client
        // with the webpki-roots trust store is enough.
        let host = cfg
            .hub_url
            .rsplit_once(':')
            .map(|(h, _)| h.to_string())
            .unwrap_or_else(|| cfg.hub_url.clone());
        let tls = ClientTlsConfig::new()
            .with_webpki_roots()
            .domain_name(host);
        endpoint = endpoint.tls_config(tls)?;
    }

    let channel = endpoint.connect().await?;
    let mut client = TunnelClient::new(channel);

    // Outbound: an mpsc receiver, wrapped as a Stream and handed to
    // the gRPC call. Every send goes through hub_tx.send(frame).await,
    // so no per-frame lock is needed.
    let (hub_tx, hub_rx) = mpsc::channel::<ClientFrame>(256);
    let outbound = ReceiverStream::new(hub_rx);

    let mut inbound = client.connect(outbound).await?.into_inner();

    // Publish hub_tx to the HTTP proxy. The handshake is sent
    // internally on the first challenge, so attaching here is fine.
    // The paired detach_hub_and_flush() runs in the wrapper above.
    shared.attach_hub(hub_tx.clone()).await;

    let target_names: Vec<&String> = cfg.targets.keys().collect();
    tracing::info!(
        hub = %cfg.hub_url,
        spoke_id = %cfg.spoke_id,
        targets = ?target_names,
        ver = CLIENT_VERSION,
        "tunnel-client connected"
    );

    let streams: StreamMap = Arc::new(Mutex::new(HashMap::new()));

    while let Some(frame) = inbound.message().await? {
        let sid = frame.stream_id;
        match frame.kind {
            Some(server_frame::Kind::Challenge(_)) => {
                tracing::info!(sid, "recv challenge");
                if hub_tx
                    .send(client_handshake(&cfg.spoke_id, &cfg.token))
                    .await
                    .is_err()
                {
                    return Err("failed to send handshake (hub_tx closed)".into());
                }
            }
            Some(server_frame::Kind::Open(open)) => {
                tracing::info!(sid, target = %open.target, "recv open");
                let name = if open.target.is_empty() {
                    "default".to_string()
                } else {
                    open.target.clone()
                };
                let target = match cfg.targets.get(&name) {
                    Some(t) => t.clone(),
                    None => {
                        let available: Vec<&String> = cfg.targets.keys().collect();
                        tracing::warn!(
                            sid,
                            target_name = %name,
                            available = ?available,
                            "unknown target from hub"
                        );
                        let _ = hub_tx
                            .send(client_close(
                                sid,
                                false,
                                format!("unknown target {:?}", name),
                            ))
                            .await;
                        continue;
                    }
                };

                // Insert into streams[sid] synchronously, then spawn.
                // If Data arrives before the insert the pump would
                // drop it with "unknown stream".
                let (tx, rx) = mpsc::channel::<Vec<u8>>(64);
                streams.lock().await.insert(sid, tx);
                let hub_tx_c = hub_tx.clone();
                let streams_c = streams.clone();
                tokio::spawn(async move {
                    run_stream(sid, name, target, hub_tx_c, rx, streams_c).await;
                });
            }
            Some(server_frame::Kind::Data(d)) => {
                let n = d.payload.len();
                let tx_opt = {
                    let map = streams.lock().await;
                    map.get(&sid).cloned()
                };
                if let Some(tx) = tx_opt {
                    tracing::info!(sid, n, "recv data");
                    if tx.send(d.payload).await.is_err() {
                        tracing::warn!(sid, "deliver to stream failed");
                    }
                } else {
                    tracing::warn!(sid, n, "recv data for unknown stream");
                }
            }
            Some(server_frame::Kind::Close(_)) => {
                tracing::info!(sid, "recv close");
                // Dropping the entry closes the per-stream inbound
                // receiver, which lets the pump wind itself down.
                streams.lock().await.remove(&sid);
            }
            Some(server_frame::Kind::Ping(p)) => {
                let _ = hub_tx.send(client_pong(p.sent_at)).await;
            }
            Some(server_frame::Kind::HttpResponse(r)) => {
                // Application-spoke HTTP mode: hand the response off
                // to the waiting oneshot::Sender.
                let tx_opt = shared.pending.lock().await.remove(&sid);
                if let Some(tx) = tx_opt {
                    let n = r.body.len();
                    tracing::info!(sid, status = r.status, n, "recv http_response");
                    // If the receiver already dropped (e.g. the
                    // caller timed out), send() returns Err. That is
                    // harmless: we already removed the entry from
                    // the pending map.
                    let _ = tx.send(r);
                } else {
                    tracing::warn!(sid, status = r.status, "recv http_response for unknown stream_id");
                }
            }
            None => {
                tracing::warn!(sid, "recv frame with no kind");
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// main: reconnect loop with exponential backoff.
// ---------------------------------------------------------------------------

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_current_span(false)
        .with_span_list(false)
        .init();
}

#[tokio::main]
async fn main() {
    init_tracing();

    let cfg = match load_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(err = %e, "config");
            std::process::exit(1);
        }
    };

    let shared = Arc::new(SharedProxyState::new());

    // If KERNEL_PROXY_LISTEN is set, start the HTTP proxy server in
    // the background. Otherwise the process just maintains the gRPC
    // session (matches the pre-HTTP-proxy behaviour).
    if let Some(listen) = cfg.kernel_proxy_listen.clone() {
        let shared_c = shared.clone();
        let listen_c = listen.clone();
        tokio::spawn(async move {
            if let Err(e) = run_http_proxy(listen_c.clone(), shared_c).await {
                tracing::error!(err = %e, listen = %listen_c, "kernel-proxy HTTP server exited");
            }
        });
        tracing::info!(listen = %listen, "kernel-proxy mode enabled");
    }

    let mut backoff = cfg.backoff_min;
    loop {
        let session_started = std::time::Instant::now();
        match run_session(&cfg, &shared).await {
            Ok(()) => {
                tracing::info!("session ended cleanly");
                backoff = cfg.backoff_min;
            }
            Err(e) => {
                // If the session ran long enough to be considered a
                // successful connect (>= 10 s), reset the backoff so
                // routine reconnects (idle timeouts on the transport
                // in front of the hub, for instance) do not park us
                // at the maximum. Only chains of quick failures
                // grow the delay.
                if session_started.elapsed() >= Duration::from_secs(10) {
                    backoff = cfg.backoff_min;
                }
                tracing::warn!(err = %e, backoff_sec = backoff.as_secs(), "session error");
                sleep(backoff).await;
                backoff = std::cmp::min(cfg.backoff_max, backoff * 2);
            }
        }
    }
}
