// Command native-spoke is the entry point for OpenSpoke's native spoke binary.
//
// native-spoke runs on Windows / Linux / macOS as a host-level service
// (Windows Service / systemd unit / launchd plist), without Kubernetes or
// containers. See ../../README.md for the overall architecture.
package main

import (
	"flag"
	"fmt"
	"os"
	"runtime"

	"github.com/OpenSpoke/openspoke/spoke/native/internal/config"
	"github.com/OpenSpoke/openspoke/spoke/native/internal/version"
)

func main() {
	var (
		showVersion = flag.Bool("version", false, "print version and exit")
		configPath  = flag.String("config", "", "path to config.yaml (default: OS-conventional location)")
	)
	flag.Parse()

	if *showVersion {
		fmt.Printf("native-spoke %s (%s/%s)\n", version.Version, runtime.GOOS, runtime.GOARCH)
		return
	}

	path := *configPath
	if path == "" {
		path = config.DefaultPath()
	}

	cfg, err := config.Load(path)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to load config from %s: %v\n", path, err)
		os.Exit(1)
	}

	fmt.Printf("native-spoke %s starting\n", version.Version)
	fmt.Printf("  spoke_id    = %s\n", cfg.SpokeID)
	fmt.Printf("  hub_url     = %s\n", cfg.HubURL)
	fmt.Printf("  listen_port = %d\n", cfg.ListenPort)
	fmt.Printf("  os/arch     = %s/%s\n", runtime.GOOS, runtime.GOARCH)

	// TODO(v0): start heartbeat sender (POST to {hub_url}/native-spoke/heartbeat every 30s)
	// TODO(v0): start MCP HTTP server (listening on listen_port)
	// TODO(v0): handle SIGTERM / SIGINT for graceful shutdown
}
