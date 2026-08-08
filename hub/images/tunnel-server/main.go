// tunnel-server is the hub-side implementation of the OpenSpoke
// spoke -> hub reverse tunnel.
//
//   - Each spoke opens a bidirectional gRPC stream via Tunnel.Connect(stream).
//   - Anything inside the hub that dials
//       tunnel-server.mcp.svc.cluster.local:<PORT>
//     is transparently forwarded through the existing bidi stream to the
//     spoke-side target (default: mcp-company1:8000 on that spoke).
//   - Phase 1 (this release) authenticates each spoke with a pre-shared
//     token; the per-spoke listen port is statically mapped from the
//     TUNNEL_SPOKES env var.
//   - Phase 3 will replace the token with Ed25519 signature verification
//     and drive the mapping from a spoke registry.

package main

import (
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net"
	"net/http"
	"os"
	"strconv"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/keepalive"
	"google.golang.org/grpc/status"

	tunnelpb "github.com/openspoke/openspoke-tunnel-server/proto"
)

var (
	logger = slog.New(slog.NewJSONHandler(os.Stdout, nil))

	metricConnected = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "openspoke_tunnel_connected_spokes",
		Help: "Number of currently connected spokes.",
	})
	metricStreamsOpen = promauto.NewGaugeVec(prometheus.GaugeOpts{
		Name: "openspoke_tunnel_streams_open",
		Help: "Open virtual TCP streams per spoke.",
	}, []string{"spoke_id"})
	metricBytesSent = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "openspoke_tunnel_bytes_sent_total",
		Help: "Bytes sent hub->spoke via tunnel.",
	}, []string{"spoke_id"})
	metricBytesRecv = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "openspoke_tunnel_bytes_recv_total",
		Help: "Bytes received spoke->hub via tunnel.",
	}, []string{"spoke_id"})
	metricHandshakeFail = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "openspoke_tunnel_handshake_failures_total",
		Help: "Handshake failures.",
	}, []string{"reason"})
)

// ---------------------------------------------------------------------------
// Config (env-driven)
// ---------------------------------------------------------------------------

type spokeConfig struct {
	spokeID string
	port    int    // per-spoke listen port (hub side)
	token   string // Phase 1 pre-shared token
}

type serverConfig struct {
	grpcListen    string
	metricsListen string
	spokes        map[string]spokeConfig // key = spoke_id
	nonceTTL      time.Duration
}

func loadConfig() (serverConfig, error) {
	c := serverConfig{
		grpcListen:    envDefault("TUNNEL_GRPC_LISTEN", ":8080"),
		metricsListen: envDefault("TUNNEL_METRICS_LISTEN", ":9090"),
		spokes:        make(map[string]spokeConfig),
		nonceTTL:      60 * time.Second,
	}

	// TUNNEL_SPOKES="spoke-1:10001:tokenA,spoke-2:10002:tokenB"
	raw := strings.TrimSpace(os.Getenv("TUNNEL_SPOKES"))
	if raw == "" {
		return c, errors.New("TUNNEL_SPOKES env is required (format: spoke-id:port:token,...)")
	}
	for _, entry := range strings.Split(raw, ",") {
		entry = strings.TrimSpace(entry)
		if entry == "" {
			continue
		}
		parts := strings.SplitN(entry, ":", 3)
		if len(parts) != 3 {
			return c, fmt.Errorf("bad TUNNEL_SPOKES entry: %q", entry)
		}
		port, err := strconv.Atoi(parts[1])
		if err != nil {
			return c, fmt.Errorf("bad port in %q: %w", entry, err)
		}
		c.spokes[parts[0]] = spokeConfig{
			spokeID: parts[0],
			port:    port,
			token:   parts[2],
		}
	}
	return c, nil
}

func envDefault(k, d string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return d
}

// ---------------------------------------------------------------------------
// server
// ---------------------------------------------------------------------------

type tunnelServer struct {
	tunnelpb.UnimplementedTunnelServer

	cfg serverConfig

	mu      sync.Mutex
	spokes  map[string]*spokeSession // key = spoke_id, live session
	nextSID uint64
}

func newTunnelServer(cfg serverConfig) *tunnelServer {
	return &tunnelServer{
		cfg:    cfg,
		spokes: make(map[string]*spokeSession),
	}
}

// spokeSession represents one spoke's active tunnel.
type spokeSession struct {
	spokeID string
	stream  tunnelpb.Tunnel_ConnectServer
	sendMu  sync.Mutex // stream.Send is not goroutine-safe

	// virtual TCP connections keyed by stream_id
	streamsMu sync.Mutex
	streams   map[uint64]*virtualConn

	listener net.Listener
	closeCh  chan struct{}
	closed   atomic.Bool
}

type virtualConn struct {
	streamID uint64
	spoke    *spokeSession
	tcp      net.Conn
}

func (s *tunnelServer) Connect(stream tunnelpb.Tunnel_ConnectServer) error {
	// 1. Send the challenge.
	nonce := make([]byte, 32)
	if _, err := io.ReadFull(cryptoRand(), nonce); err != nil {
		metricHandshakeFail.WithLabelValues("nonce_read").Inc()
		return status.Error(codes.Internal, "nonce read failed")
	}
	issuedAt := time.Now().UnixMilli()
	if err := stream.Send(&tunnelpb.ServerFrame{
		Kind: &tunnelpb.ServerFrame_Challenge{
			Challenge: &tunnelpb.HandshakeChallenge{
				Nonce:    nonce,
				IssuedAt: issuedAt,
			},
		},
	}); err != nil {
		return err
	}

	// 2. Await the HandshakeResponse (must be the client's first frame).
	first, err := stream.Recv()
	if err != nil {
		metricHandshakeFail.WithLabelValues("recv_first").Inc()
		return err
	}
	hs := first.GetHandshake()
	if hs == nil {
		metricHandshakeFail.WithLabelValues("no_handshake").Inc()
		return status.Error(codes.Unauthenticated, "expected HandshakeResponse")
	}

	// 3. Look up spoke_id and verify token (Phase 1).
	sc, ok := s.cfg.spokes[hs.SpokeId]
	if !ok {
		metricHandshakeFail.WithLabelValues("unknown_spoke").Inc()
		return status.Error(codes.Unauthenticated, "unknown spoke_id")
	}
	if string(hs.Signature) != sc.token {
		metricHandshakeFail.WithLabelValues("bad_token").Inc()
		return status.Error(codes.Unauthenticated, "bad token")
	}
	// Phase 3 will add nonce replay-check and issued_at drift verification.

	// 4. Register the session. A stale duplicate session is displaced.
	session := &spokeSession{
		spokeID: hs.SpokeId,
		stream:  stream,
		streams: make(map[uint64]*virtualConn),
		closeCh: make(chan struct{}),
	}
	s.mu.Lock()
	if old, ok := s.spokes[hs.SpokeId]; ok {
		logger.Warn("displacing existing session", "spoke_id", hs.SpokeId)
		old.close()
	}
	s.spokes[hs.SpokeId] = session
	s.mu.Unlock()
	metricConnected.Inc()
	defer func() {
		metricConnected.Dec()
		s.mu.Lock()
		if s.spokes[hs.SpokeId] == session {
			delete(s.spokes, hs.SpokeId)
		}
		s.mu.Unlock()
		session.close()
	}()

	// 5. Start the per-spoke TCP listener (lifetime = this session).
	ln, err := net.Listen("tcp", fmt.Sprintf(":%d", sc.port))
	if err != nil {
		return status.Errorf(codes.Internal, "listen %d: %v", sc.port, err)
	}
	session.listener = ln
	defer ln.Close()

	logger.Info("spoke connected",
		"spoke_id", hs.SpokeId,
		"port", sc.port,
		"ver", hs.TunnelClientVer)

	// 6. Accept loop: each new TCP conn becomes a new virtual stream to the spoke.
	go s.acceptLoop(session)

	// 7. Keep receiving spoke frames until the stream ends.
	return s.recvLoop(session)
}

func (s *tunnelServer) acceptLoop(session *spokeSession) {
	for {
		conn, err := session.listener.Accept()
		if err != nil {
			if session.closed.Load() {
				return
			}
			logger.Warn("accept failed", "spoke_id", session.spokeID, "err", err)
			return
		}
		sid := atomic.AddUint64(&s.nextSID, 1)
		logger.Info("accept new conn", "spoke_id", session.spokeID, "sid", sid, "remote", conn.RemoteAddr().String())
		vc := &virtualConn{streamID: sid, spoke: session, tcp: conn}
		session.streamsMu.Lock()
		session.streams[sid] = vc
		session.streamsMu.Unlock()
		metricStreamsOpen.WithLabelValues(session.spokeID).Inc()

		if err := session.send(&tunnelpb.ServerFrame{
			StreamId: sid,
			Kind: &tunnelpb.ServerFrame_Open{
				Open: &tunnelpb.OpenStream{
					Protocol: "tcp",
					Target:   "default",
				},
			},
		}); err != nil {
			logger.Warn("send open failed", "sid", sid, "err", err)
			vc.close()
			continue
		}
		logger.Info("open frame sent", "spoke_id", session.spokeID, "sid", sid)

		go s.pumpUpstream(vc)
	}
}

// pumpUpstream: TCP -> spoke (Data frame)
func (s *tunnelServer) pumpUpstream(vc *virtualConn) {
	defer vc.close()
	buf := make([]byte, 32*1024)
	for {
		n, err := vc.tcp.Read(buf)
		if n > 0 {
			metricBytesSent.WithLabelValues(vc.spoke.spokeID).Add(float64(n))
			if err := vc.spoke.send(&tunnelpb.ServerFrame{
				StreamId: vc.streamID,
				Kind: &tunnelpb.ServerFrame_Data{
					Data: &tunnelpb.Data{Payload: append([]byte(nil), buf[:n]...)},
				},
			}); err != nil {
				logger.Warn("send data failed", "err", err)
				return
			}
		}
		if err != nil {
			if err != io.EOF {
				logger.Debug("tcp read err", "err", err)
			}
			// Signal half-close.
			_ = vc.spoke.send(&tunnelpb.ServerFrame{
				StreamId: vc.streamID,
				Kind: &tunnelpb.ServerFrame_Close{
					Close: &tunnelpb.CloseStream{Half: true, Reason: "eof"},
				},
			})
			return
		}
	}
}

func (s *tunnelServer) recvLoop(session *spokeSession) error {
	for {
		f, err := session.stream.Recv()
		if err != nil {
			logger.Info("recv end", "spoke_id", session.spokeID, "err", err)
			return err
		}
		switch k := f.Kind.(type) {
		case *tunnelpb.ClientFrame_Data:
			session.streamsMu.Lock()
			vc, ok := session.streams[f.StreamId]
			session.streamsMu.Unlock()
			if !ok {
				logger.Warn("data for unknown stream", "spoke_id", session.spokeID, "sid", f.StreamId, "n", len(k.Data.Payload))
				continue
			}
			payload := k.Data.Payload
			logger.Info("recv data from spoke", "spoke_id", session.spokeID, "sid", f.StreamId, "n", len(payload))
			metricBytesRecv.WithLabelValues(session.spokeID).Add(float64(len(payload)))
			if _, err := vc.tcp.Write(payload); err != nil {
				logger.Warn("tcp write failed", "sid", f.StreamId, "err", err)
				vc.close()
			}
		case *tunnelpb.ClientFrame_Close:
			logger.Info("recv close from spoke", "spoke_id", session.spokeID, "sid", f.StreamId)
			session.streamsMu.Lock()
			vc, ok := session.streams[f.StreamId]
			session.streamsMu.Unlock()
			if ok {
				vc.close()
			}
		case *tunnelpb.ClientFrame_Pong:
			// TODO: RTT metric
		}
	}
}

func (session *spokeSession) send(f *tunnelpb.ServerFrame) error {
	session.sendMu.Lock()
	defer session.sendMu.Unlock()
	return session.stream.Send(f)
}

func (session *spokeSession) close() {
	if session.closed.Swap(true) {
		return
	}
	close(session.closeCh)
	if session.listener != nil {
		session.listener.Close()
	}
	session.streamsMu.Lock()
	for _, vc := range session.streams {
		vc.tcp.Close()
	}
	session.streamsMu.Unlock()
}

func (vc *virtualConn) close() {
	vc.tcp.Close()
	vc.spoke.streamsMu.Lock()
	if _, ok := vc.spoke.streams[vc.streamID]; ok {
		delete(vc.spoke.streams, vc.streamID)
		metricStreamsOpen.WithLabelValues(vc.spoke.spokeID).Dec()
	}
	vc.spoke.streamsMu.Unlock()
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

func main() {
	cfg, err := loadConfig()
	if err != nil {
		logger.Error("config", "err", err)
		os.Exit(1)
	}

	go func() {
		mux := http.NewServeMux()
		mux.Handle("/metrics", promhttp.Handler())
		mux.HandleFunc("/healthz", func(w http.ResponseWriter, r *http.Request) {
			w.WriteHeader(http.StatusOK)
		})
		logger.Info("metrics listening", "addr", cfg.metricsListen)
		_ = http.ListenAndServe(cfg.metricsListen, mux)
	}()

	lis, err := net.Listen("tcp", cfg.grpcListen)
	if err != nil {
		logger.Error("grpc listen", "err", err)
		os.Exit(1)
	}
	grpcSrv := grpc.NewServer(
		grpc.InitialWindowSize(4<<20),
		grpc.InitialConnWindowSize(16<<20),
		grpc.KeepaliveParams(keepalive.ServerParameters{
			Time:    30 * time.Second,
			Timeout: 20 * time.Second,
		}),
		grpc.KeepaliveEnforcementPolicy(keepalive.EnforcementPolicy{
			MinTime:             10 * time.Second,
			PermitWithoutStream: true,
		}),
	)
	srv := newTunnelServer(cfg)
	tunnelpb.RegisterTunnelServer(grpcSrv, srv)
	logger.Info("tunnel-server ready",
		"grpc", cfg.grpcListen,
		"spokes", len(cfg.spokes))
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	_ = ctx
	if err := grpcSrv.Serve(lis); err != nil {
		logger.Error("grpc serve", "err", err)
		os.Exit(1)
	}
}

// cryptoRand returns crypto/rand.Reader (wrapped so the import graph is minimal).
func cryptoRand() io.Reader { return cryptoRandReader{} }

type cryptoRandReader struct{}

func (cryptoRandReader) Read(p []byte) (int, error) {
	return cryptoRandRead(p)
}
