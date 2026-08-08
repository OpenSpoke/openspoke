# native-spoke

OpenSpoke の **ネイティブスポーク** 実装。 Kubernetes やコンテナを介さず OS 上で直接動く Go 製単一バイナリのスポーク。

対比される **Kubernetes スポーク** (rke2 / EKS 系、 `rag-spoke` namespace) と異なり、 Windows / Linux / macOS の host OS 上でサービス (Windows Service / systemd unit / launchd plist) として常駐する。

## 全体構成

ネイティブスポークは以下のサービス群で構成される:

| Service | プロセス | 役割 |
|---|---|---|
| `native-spoke` | Go 単一バイナリ、 LocalSystem / root で常駐 | MCP server、 heartbeat 送信 |
| `qdrant` | Qdrant 公式バイナリ | vector DB |
| `fastembed-server` | Python + FastAPI + FastEmbed | text → vector 変換 |
| `opensearch` | OpenSearch 公式 | OpenSearch 直接操作系の裏側 |
| `cloudflared` | Cloudflare 公式 | `spoke-XXXX.example.com` 公開 |

本リポジトリで管理するのは `native-spoke` 本体 (Go バイナリ) のみ。 qdrant / fastembed-server / opensearch / cloudflared は別途 host にインストールする。

## ビルド

```sh
# 現在の OS / arch 向け
go build -o native-spoke ./cmd/native-spoke

# クロスコンパイル
GOOS=windows GOARCH=amd64 go build -o dist/native-spoke.exe         ./cmd/native-spoke
GOOS=linux   GOARCH=amd64 go build -o dist/native-spoke-linux-amd64 ./cmd/native-spoke
GOOS=linux   GOARCH=arm64 go build -o dist/native-spoke-linux-arm64 ./cmd/native-spoke
GOOS=darwin  GOARCH=arm64 go build -o dist/native-spoke-darwin-arm64 ./cmd/native-spoke
```

## 起動

```sh
# config を明示指定
native-spoke --config /etc/openspoke/native-spoke/config.yaml

# デフォルト場所 (OS 慣習) から読む
native-spoke

# version 表示
native-spoke --version
```

## デフォルトの config 配置 (OS 慣習)

- Windows: `C:\ProgramData\OpenSpoke\native-spoke\config.yaml`
- Linux: `/etc/openspoke/native-spoke/config.yaml`
- macOS: `/usr/local/etc/openspoke/native-spoke/config.yaml`

## config 例

```yaml
spoke_id: spoke-example
hub_url: https://hub.example.com
auth_token: <pre-shared bearer token>
listen_port: 18080

qdrant:
  host: 127.0.0.1
  port: 6334

fastembed:
  host: 127.0.0.1
  port: 8000

opensearch:
  host: 127.0.0.1
  port: 9200

log:
  level: info
```

## 状態

v0 開発中。 現在のスコープ:

- [x] CLI / config loader scaffold
- [ ] heartbeat sender (hub の `/native-spoke/heartbeat` に 30 秒ごと POST、 services up/down 含む)
- [ ] OS サービス登録 (Windows Service / systemd / launchd) — `--install-service` / `--uninstall-service` flag
- [ ] MCP server (HTTP)
- [ ] MCP tools 実装 (node_exec, opensearch_*, vector_search, search_rag, 等)

詳細設計は別途記録あり (ネイティブスポーク v0 設計)。
