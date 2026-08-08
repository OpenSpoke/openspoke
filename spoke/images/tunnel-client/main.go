// tunnel-client is the spoke-side implementation of the OpenSpoke
// spoke -> hub reverse tunnel.
//
//   - Dials the hub over gRPC and opens Tunnel.Connect(stream).
//   - Answers the hub's HandshakeChallenge with SPOKE_ID + TOKEN.
//   - For each OpenStream frame from the hub, dials TARGET (the local
//     MCP endpoint, e.g. mcp-company1:8000) and forwards bytes in both
//     directions, keyed by stream_id.
//   - Reconnects with exponential backoff on disconnect.
//
// Phase 1 (this release): pre-shared token auth.
// Phase 3 will replace the signature with Ed25519(nonce ‖ spoke_id ‖ issued_at).

package main

import (
	"context"
	"crypto/tls"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net"
	"os"
	"strings"
	"sync"
	"time"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/keepalive"

	tunnelpb "github.com/openspoke/openspoke-tunnel-client/proto"
)

var logger = slog.New(slog.NewJSONHandler(os.Stdout, nil))

const clientVersion = "v0.1.0"

// ---------------------------------------------------------------------------
// config
// ---------------------------------------------------------------------------

type clientConfig struct {
	hubURL     string        // e.g. hub.example.com:443 or localhost:8080
	spokeID    string        // e.g. spoke-1
	token      string        // Phase 1 pre-shared token
	target     string        // e.g. mcp-company1.rag-spoke.svc.cluster.local:8000
	insecure   bool          // localhost dev only
	backoffMin time.Duration
	backoffMax time.Duration
}

func loadConfig() (clientConfig, error) {
	c := clientConfig{
		hubURL:     os.Getenv("HUB_URL"),
		spokeID:    os.Getenv("SPOKE_ID"),
		token:      os.Getenv("TUNNEL_TOKEN"),
		target:     os.Getenv("TARGET"),
		insecure:   strings.EqualFold(os.Getenv("TUNNEL_INSECURE"), "true"),
		backoffMin: 1 * time.Second,
		backoffMax: 30 * time.Second,
	}
	if c.hubURL == "" || c.spokeID == "" || c.token == "" || c.target == "" {
		return c, errors.New("HUB_URL / SPOKE_ID / TUNNEL_TOKEN / TARGET are required")
	}
	return c, nil
}

// ---------------------------------------------------------------------------
// session
// ---------------------------------------------------------------------------

type session struct {
	cfg    clientConfig
	stream tunnelpb.Tunnel_ConnectClient
	sendMu sync.Mutex

	streamsMu sync.Mutex
	streams   map[uint64]net.Conn
}

func newSession(cfg clientConfig, stream tunnelpb.Tunnel_ConnectClient) *session {
	return &session{
		cfg:     cfg,
		stream:  stream,
		streams: make(map[uint64]net.Conn),
	}
}

func (s *session) send(f *tunnelpb.ClientFrame) error {
	s.sendMu.Lock()
	defer s.sendMu.Unlock()
	return s.stream.Send(f)
}

func (s *session) handshake(challenge *tunnelpb.HandshakeChallenge) error {
	// Phase 1: signature = raw pre-shared token bytes.
	// Phase 3: signature = Ed25519(nonce || spoke_id || issued_at).
	return s.send(&tunnelpb.ClientFrame{
		Kind: &tunnelpb.ClientFrame_Handshake{
			Handshake: &tunnelpb.HandshakeResponse{
				SpokeId:         s.cfg.spokeID,
				Signature:       []byte(s.cfg.token),
				TunnelClientVer: clientVersion,
			},
		},
	})
}

func (s *session) handleOpen(sid uint64, open *tunnelpb.OpenStream) {
	target := s.cfg.target // v0: always the TARGET env value
	logger.Info("open dial start", "sid", sid, "target", target)
	conn, err := net.DialTimeout("tcp", target, 5*time.Second)
	if err != nil {
		logger.Warn("dial target failed", "sid", sid, "target", target, "err", err)
		_ = s.send(&tunnelpb.ClientFrame{
			StreamId: sid,
			Kind: &tunnelpb.ClientFrame_Close{
				Close: &tunnelpb.CloseStream{Half: false, Reason: err.Error()},
			},
		})
		return
	}
	logger.Info("open dial ok", "sid", sid, "local", conn.LocalAddr().String())
	s.streamsMu.Lock()
	s.streams[sid] = conn
	s.streamsMu.Unlock()

	// Local target -> hub (Data frame)
	go s.pumpToHub(sid, conn)
}

func (s *session) pumpToHub(sid uint64, conn net.Conn) {
	defer func() {
		logger.Info("pump end", "sid", sid)
		s.closeStream(sid)
	}()
	buf := make([]byte, 32*1024)
	for {
		n, err := conn.Read(buf)
		if n > 0 {
			logger.Info("pump read", "sid", sid, "n", n)
			if serr := s.send(&tunnelpb.ClientFrame{
				StreamId: sid,
				Kind: &tunnelpb.ClientFrame_Data{
					Data: &tunnelpb.Data{Payload: append([]byte(nil), buf[:n]...)},
				},
			}); serr != nil {
				logger.Warn("send data failed", "sid", sid, "err", serr)
				return
			}
		}
		if err != nil {
			logger.Info("pump read err", "sid", sid, "err", err)
			_ = s.send(&tunnelpb.ClientFrame{
				StreamId: sid,
				Kind: &tunnelpb.ClientFrame_Close{
					Close: &tunnelpb.CloseStream{Half: true, Reason: "eof"},
				},
			})
			return
		}
	}
}

func (s *session) closeStream(sid uint64) {
	s.streamsMu.Lock()
	if c, ok := s.streams[sid]; ok {
		c.Close()
		delete(s.streams, sid)
	}
	s.streamsMu.Unlock()
}

func (s *session) recvLoop() error {
	for {
		f, err := s.stream.Recv()
		if err != nil {
			return err
		}
		switch k := f.Kind.(type) {
		case *tunnelpb.ServerFrame_Challenge:
			logger.Info("recv challenge", "sid", f.StreamId)
			if err := s.handshake(k.Challenge); err != nil {
				return err
			}
		case *tunnelpb.ServerFrame_Open:
			logger.Info("recv open", "sid", f.StreamId, "target", k.Open.Target)
			// Register streams[sid] synchronously so a Data frame that
			// arrives within the dial's few-millisecond window is not
			// dropped as "unknown stream".
			s.handleOpen(f.StreamId, k.Open)
		case *tunnelpb.ServerFrame_Data:
			s.streamsMu.Lock()
			conn := s.streams[f.StreamId]
			s.streamsMu.Unlock()
			if conn == nil {
				logger.Warn("recv data for unknown stream", "sid", f.StreamId, "n", len(k.Data.Payload))
				continue
			}
			logger.Info("recv data", "sid", f.StreamId, "n", len(k.Data.Payload))
			if _, err := conn.Write(k.Data.Payload); err != nil {
				logger.Warn("write to target failed", "sid", f.StreamId, "err", err)
				s.closeStream(f.StreamId)
			}
		case *tunnelpb.ServerFrame_Close:
			logger.Info("recv close", "sid", f.StreamId)
			s.closeStream(f.StreamId)
		case *tunnelpb.ServerFrame_Ping:
			_ = s.send(&tunnelpb.ClientFrame{
				Kind: &tunnelpb.ClientFrame_Pong{
					Pong: &tunnelpb.Pong{SentAt: k.Ping.SentAt},
				},
			})
		}
	}
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

func run(ctx context.Context, cfg clientConfig) error {
	dialOpts := []grpc.DialOption{
		grpc.WithInitialWindowSize(4 << 20),
		grpc.WithInitialConnWindowSize(16 << 20),
		grpc.WithKeepaliveParams(keepalive.ClientParameters{
			Time:                30 * time.Second,
			Timeout:             20 * time.Second,
			PermitWithoutStream: true,
		}),
	}
	if cfg.insecure {
		dialOpts = append(dialOpts, grpc.WithTransportCredentials(insecure.NewCredentials()))
	} else {
		// hub.example.com:443 terminates TLS at your ingress
		// (Cloudflare edge, nginx, etc.); tunnel-client just uses
		// standard TLS on the way in.
		host, _, err := net.SplitHostPort(cfg.hubURL)
		if err != nil {
			host = cfg.hubURL
		}
		dialOpts = append(dialOpts, grpc.WithTransportCredentials(
			credentials.NewTLS(&tls.Config{ServerName: host})))
	}

	conn, err := grpc.DialContext(ctx, cfg.hubURL, dialOpts...)
	if err != nil {
		return fmt.Errorf("dial %s: %w", cfg.hubURL, err)
	}
	defer conn.Close()

	client := tunnelpb.NewTunnelClient(conn)
	stream, err := client.Connect(ctx)
	if err != nil {
		return fmt.Errorf("open stream: %w", err)
	}

	logger.Info("tunnel-client connected",
		"hub", cfg.hubURL,
		"spoke_id", cfg.spokeID,
		"target", cfg.target,
		"ver", clientVersion)

	s := newSession(cfg, stream)
	return s.recvLoop()
}

func main() {
	cfg, err := loadConfig()
	if err != nil {
		logger.Error("config", "err", err)
		os.Exit(1)
	}

	ctx := context.Background()
	backoff := cfg.backoffMin
	for {
		err := run(ctx, cfg)
		if err == nil || err == io.EOF {
			logger.Info("session ended cleanly")
			backoff = cfg.backoffMin
		} else {
			logger.Warn("session error", "err", err, "backoff", backoff)
			time.Sleep(backoff)
			backoff *= 2
			if backoff > cfg.backoffMax {
				backoff = cfg.backoffMax
			}
		}
	}
}
