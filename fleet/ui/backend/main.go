// Fleet UI - minimal backend serving REST API for fleet.cattle.io resources
// and static frontend files.
//
// Design:
//   - dynamic client (unstructured) instead of typed client, so we are not
//     pinned to a specific Fleet version
//   - basic auth from env vars (BASIC_AUTH_USERNAME / BASIC_AUTH_PASSWORD)
//   - in-cluster config (uses the fleet-ui ServiceAccount)
//   - single binary serves both /api/* and / (static SPA from ./web)
package main

import (
	"crypto/subtle"
	"encoding/json"
	"fmt"
	"io/fs"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime/schema"
	"k8s.io/client-go/dynamic"
	"k8s.io/client-go/kubernetes"
	"k8s.io/client-go/rest"

	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
)

const (
	fleetGroup   = "fleet.cattle.io"
	fleetVersion = "v1alpha1"
)

var (
	gvrGitRepo  = schema.GroupVersionResource{Group: fleetGroup, Version: fleetVersion, Resource: "gitrepos"}
	gvrBundle   = schema.GroupVersionResource{Group: fleetGroup, Version: fleetVersion, Resource: "bundles"}
	gvrCluster  = schema.GroupVersionResource{Group: fleetGroup, Version: fleetVersion, Resource: "clusters"}
	gvrBundleDp = schema.GroupVersionResource{Group: fleetGroup, Version: fleetVersion, Resource: "bundledeployments"}
)

type server struct {
	dyn       dynamic.Interface
	kube      kubernetes.Interface
	authUser  string
	authPass  string
	staticDir string
}

func main() {
	cfg, err := rest.InClusterConfig()
	if err != nil {
		log.Fatalf("failed to load in-cluster config: %v", err)
	}
	dyn, err := dynamic.NewForConfig(cfg)
	if err != nil {
		log.Fatalf("failed to create dynamic client: %v", err)
	}
	kube, err := kubernetes.NewForConfig(cfg)
	if err != nil {
		log.Fatalf("failed to create kube client: %v", err)
	}

	s := &server{
		dyn:       dyn,
		kube:      kube,
		// default を空文字にしておくと requireAuth が認証スキップ。
		// 単体運用したい時は env で明示指定する。
		authUser:  os.Getenv("BASIC_AUTH_USERNAME"),
		authPass:  os.Getenv("BASIC_AUTH_PASSWORD"),
		staticDir: envOr("STATIC_DIR", "/app/web"),
	}

	mux := http.NewServeMux()

	// Health (no auth)
	mux.HandleFunc("/healthz", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})

	// API (auth required)
	mux.Handle("/api/", s.requireAuth(http.HandlerFunc(s.apiRouter)))

	// Static (auth required)
	mux.Handle("/", s.requireAuth(http.HandlerFunc(s.serveStatic)))

	srv := &http.Server{
		Addr:         ":8080",
		Handler:      mux,
		ReadTimeout:  30 * time.Second,
		WriteTimeout: 30 * time.Second,
	}
	log.Printf("fleet-ui listening on :8080 (static=%s)", s.staticDir)
	log.Fatal(srv.ListenAndServe())
}

func envOr(k, def string) string {
	if v := os.Getenv(k); v != "" {
		return v
	}
	return def
}

// requireAuth applies HTTP Basic Auth.
// If both authUser and authPass are empty, authentication is skipped
// (use case: oauth2-proxy in front of fleet-ui handles authentication).
func (s *server) requireAuth(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if s.authUser == "" && s.authPass == "" {
			next.ServeHTTP(w, r)
			return
		}
		u, p, ok := r.BasicAuth()
		if !ok ||
			subtle.ConstantTimeCompare([]byte(u), []byte(s.authUser)) != 1 ||
			subtle.ConstantTimeCompare([]byte(p), []byte(s.authPass)) != 1 {
			w.Header().Set("WWW-Authenticate", `Basic realm="fleet-ui"`)
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		next.ServeHTTP(w, r)
	})
}

// apiRouter dispatches /api/* requests.
func (s *server) apiRouter(w http.ResponseWriter, r *http.Request) {
	path := strings.TrimPrefix(r.URL.Path, "/api/")
	parts := strings.Split(path, "/")

	switch {
	case path == "config":
		s.handleConfig(w, r)
	case path == "workspaces":
		s.handleWorkspaces(w, r)
	case len(parts) >= 1 && parts[0] == "gitrepos":
		s.handleGitRepos(w, r, parts[1:])
	case len(parts) >= 1 && parts[0] == "bundles":
		s.handleBundles(w, r, parts[1:])
	case len(parts) >= 1 && parts[0] == "clusters":
		s.handleClusters(w, r, parts[1:])
	case len(parts) >= 1 && parts[0] == "bundledeployments":
		s.handleBundleDeployments(w, r, parts[1:])
	default:
		http.NotFound(w, r)
	}
}

func (s *server) handleConfig(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, map[string]any{
		"title":            envOr("UI_TITLE", "Fleet UI"),
		"brandColor":       envOr("UI_BRAND_COLOR", "#3d98d3"),
		"defaultWorkspace": envOr("FLEET_DEFAULT_WORKSPACE", "fleet-default"),
	})
}

// handleWorkspaces returns namespaces that look like Fleet workspaces.
// A Fleet workspace is just a namespace that contains GitRepo CRs or that
// is labelled as such. We list namespaces and let the UI filter.
func (s *server) handleWorkspaces(w http.ResponseWriter, r *http.Request) {
	nss, err := s.kube.CoreV1().Namespaces().List(r.Context(), metav1.ListOptions{})
	if err != nil {
		httpError(w, err)
		return
	}
	out := make([]string, 0, len(nss.Items))
	for _, ns := range nss.Items {
		// Fleet workspaces are: fleet-local, fleet-default, or any ns with GitRepo CRs.
		// For simplicity we include all that start with "fleet-" plus any explicitly
		// labelled with fleet.cattle.io/workspace=true.
		if strings.HasPrefix(ns.Name, "fleet-") {
			out = append(out, ns.Name)
			continue
		}
		if ns.Labels["fleet.cattle.io/workspace"] == "true" {
			out = append(out, ns.Name)
		}
	}
	writeJSON(w, out)
}

// handleGitRepos: /api/gitrepos              -> list across all namespaces
//                 /api/gitrepos/{ns}         -> list in namespace
//                 /api/gitrepos/{ns}/{name}  -> get / patch / delete
//                 /api/gitrepos/{ns}/{name}/force-sync -> patch annotation
func (s *server) handleGitRepos(w http.ResponseWriter, r *http.Request, parts []string) {
	s.handleCR(w, r, parts, gvrGitRepo)
}

func (s *server) handleBundles(w http.ResponseWriter, r *http.Request, parts []string) {
	s.handleCR(w, r, parts, gvrBundle)
}

func (s *server) handleClusters(w http.ResponseWriter, r *http.Request, parts []string) {
	s.handleCR(w, r, parts, gvrCluster)
}

func (s *server) handleBundleDeployments(w http.ResponseWriter, r *http.Request, parts []string) {
	s.handleCR(w, r, parts, gvrBundleDp)
}

// handleCR is the shared CRUD handler for Fleet CRs.
func (s *server) handleCR(w http.ResponseWriter, r *http.Request, parts []string, gvr schema.GroupVersionResource) {
	ctx := r.Context()

	// list across all namespaces
	if len(parts) == 0 {
		switch r.Method {
		case http.MethodGet:
			list, err := s.dyn.Resource(gvr).List(ctx, metav1.ListOptions{})
			if err != nil {
				httpError(w, err)
				return
			}
			writeJSON(w, list)
		default:
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		}
		return
	}

	ns := parts[0]

	// list within namespace
	if len(parts) == 1 {
		switch r.Method {
		case http.MethodGet:
			list, err := s.dyn.Resource(gvr).Namespace(ns).List(ctx, metav1.ListOptions{})
			if err != nil {
				httpError(w, err)
				return
			}
			writeJSON(w, list)
		case http.MethodPost:
			obj := &unstructured.Unstructured{}
			if err := json.NewDecoder(r.Body).Decode(obj); err != nil {
				http.Error(w, err.Error(), http.StatusBadRequest)
				return
			}
			created, err := s.dyn.Resource(gvr).Namespace(ns).Create(ctx, obj, metav1.CreateOptions{})
			if err != nil {
				httpError(w, err)
				return
			}
			writeJSON(w, created)
		default:
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		}
		return
	}

	name := parts[1]

	// /{ns}/{name}/force-sync
	if len(parts) >= 3 && parts[2] == "force-sync" {
		if r.Method != http.MethodPost {
			http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
			return
		}
		patch := []byte(fmt.Sprintf(
			`{"metadata":{"annotations":{"fleet.cattle.io/force-sync-generation":"%d"}}}`,
			time.Now().Unix(),
		))
		_, err := s.dyn.Resource(gvr).Namespace(ns).Patch(
			ctx, name, types.MergePatchType, patch, metav1.PatchOptions{},
		)
		if err != nil {
			httpError(w, err)
			return
		}
		writeJSON(w, map[string]string{"status": "ok"})
		return
	}

	// /{ns}/{name}
	switch r.Method {
	case http.MethodGet:
		obj, err := s.dyn.Resource(gvr).Namespace(ns).Get(ctx, name, metav1.GetOptions{})
		if err != nil {
			httpError(w, err)
			return
		}
		writeJSON(w, obj)
	case http.MethodPatch:
		body, _ := readAll(r)
		obj, err := s.dyn.Resource(gvr).Namespace(ns).Patch(
			ctx, name, types.MergePatchType, body, metav1.PatchOptions{},
		)
		if err != nil {
			httpError(w, err)
			return
		}
		writeJSON(w, obj)
	case http.MethodDelete:
		err := s.dyn.Resource(gvr).Namespace(ns).Delete(ctx, name, metav1.DeleteOptions{})
		if err != nil {
			httpError(w, err)
			return
		}
		writeJSON(w, map[string]string{"status": "deleted"})
	default:
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
	}
}

// serveStatic serves the SPA. Anything not /api/* falls here and gets
// either the requested file or index.html (so the SPA router works).
func (s *server) serveStatic(w http.ResponseWriter, r *http.Request) {
	p := filepath.Clean(r.URL.Path)
	if p == "/" {
		p = "/index.html"
	}
	full := filepath.Join(s.staticDir, p)
	if _, err := os.Stat(full); err != nil {
		// fallback: serve index.html for SPA routing
		full = filepath.Join(s.staticDir, "index.html")
	}
	http.ServeFile(w, r, full)
}

func writeJSON(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json")
	if err := json.NewEncoder(w).Encode(v); err != nil {
		log.Printf("writeJSON: %v", err)
	}
}

func httpError(w http.ResponseWriter, err error) {
	log.Printf("api error: %v", err)
	http.Error(w, err.Error(), http.StatusInternalServerError)
}

func readAll(r *http.Request) ([]byte, error) {
	buf := make([]byte, 0, 4096)
	tmp := make([]byte, 4096)
	for {
		n, err := r.Body.Read(tmp)
		if n > 0 {
			buf = append(buf, tmp[:n]...)
		}
		if err != nil {
			if err.Error() == "EOF" {
				return buf, nil
			}
			return buf, nil
		}
	}
}

// unused but kept for future namespace-by-label workspace detection
var _ corev1.Namespace
var _ fs.FS
