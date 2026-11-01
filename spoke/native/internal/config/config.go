// Package config loads native-spoke's YAML configuration from a host-local
// path. The default path follows each operating system's convention
// (see DefaultPath).
package config

import (
	"fmt"
	"os"
	"runtime"

	"gopkg.in/yaml.v3"
)

// Config is the on-disk representation of native-spoke's settings.
type Config struct {
	SpokeID    string     `yaml:"spoke_id"`
	HubURL     string     `yaml:"hub_url"`
	AuthToken  string     `yaml:"auth_token"`
	ListenPort int        `yaml:"listen_port"`
	Qdrant     Endpoint   `yaml:"qdrant"`
	FastEmbed  Endpoint   `yaml:"fastembed"`
	OpenSearch Endpoint   `yaml:"opensearch"`
	Log        LogSection `yaml:"log"`
}

// Endpoint identifies a co-located service that native-spoke connects to.
type Endpoint struct {
	Host string `yaml:"host"`
	Port int    `yaml:"port"`
}

// LogSection collects log-related settings.
type LogSection struct {
	Level string `yaml:"level"`
}

// DefaultPath returns the OS-conventional config path for the current platform.
func DefaultPath() string {
	switch runtime.GOOS {
	case "windows":
		return `C:\ProgramData\OpenSpoke\native-spoke\config.yaml`
	case "darwin":
		return "/usr/local/etc/openspoke/native-spoke/config.yaml"
	default: // linux and others
		return "/etc/openspoke/native-spoke/config.yaml"
	}
}

// Load reads, parses, and validates the YAML file at path.
func Load(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read: %w", err)
	}
	cfg := &Config{}
	if err := yaml.Unmarshal(data, cfg); err != nil {
		return nil, fmt.Errorf("parse: %w", err)
	}
	if err := cfg.validate(); err != nil {
		return nil, fmt.Errorf("validate: %w", err)
	}
	return cfg, nil
}

func (c *Config) validate() error {
	if c.SpokeID == "" {
		return fmt.Errorf("spoke_id is required")
	}
	if c.HubURL == "" {
		return fmt.Errorf("hub_url is required")
	}
	if c.AuthToken == "" {
		return fmt.Errorf("auth_token is required")
	}
	if c.ListenPort == 0 {
		return fmt.Errorf("listen_port is required")
	}
	return nil
}
