# RAGシステム - 完全オフライン対応版

## 概要

このバージョンでは、Dockerイメージに全パッケージとモデルを事前インストールすることで、
Kubernetes Pod起動時にインターネット接続を必要としない完全オフライン動作を実現します。

## 準備するファイル

```
.
├── Dockerfile.backend           # バックエンド用Dockerfile
├── Dockerfile.frontend          # フロントエンド用Dockerfile
├── build-images.sh              # イメージビルドスクリプト
├── rag-backend-offline-v2.yaml  # バックエンドDeployment
└── rag-frontend-offline.yaml    # フロントエンドDeployment
```

## セットアップ手順

### ステップ1: Dockerイメージのビルド（オンライン環境で実行）

```bash
# Dockerが利用可能な環境で実行

# 1. ビルドスクリプトに実行権限を付与
chmod +x build-images.sh

# 2. イメージをビルド・プッシュ
./build-images.sh
```

**処理時間:** 約10-20分（初回のみ）

**生成されるイメージ:**
- `registry.example.com/rag-backend-gpu-offline:latest` (約5-6GB)
- `registry.example.com/rag-frontend-offline:latest` (約500MB)

### ステップ2: Kubernetesクラスタ各ノードでイメージをpull

```bash
# 各ワーカーノード（example-node, example-node）で実行

# バックエンドイメージ
crictl pull registry.example.com/rag-backend-gpu-offline:latest

# フロントエンドイメージ
crictl pull registry.example.com/rag-frontend-offline:latest
```

### ステップ3: Deploymentの適用

```bash
# example-node（control plane）で実行

# バックエンドDeploymentを適用
kubectl apply -f rag-backend-offline-v2.yaml

# フロントエンドDeploymentを適用
kubectl apply -f rag-frontend-offline.yaml

# Podを再起動
kubectl delete pod -n rag-company1 -l app=rag-backend
kubectl delete pod -n rag-company1 -l app=rag-frontend
```

### ステップ4: 動作確認

```bash
# Pod起動を確認
kubectl get pods -n rag-company1

# バックエンドのログ確認
kubectl logs -n rag-company1 -l app=rag-backend -c backend

# フロントエンドのログ確認
kubectl logs -n rag-company1 -l app=rag-frontend
```

## オフライン起動の確認

### テスト方法

```bash
# 全Podを削除
kubectl delete pod --all -n rag-company1

# 自動的に再起動されることを確認（インターネット接続なし）
kubectl get pods -n rag-company1 -w
```

**期待される動作:**
```
NAME                            READY   STATUS    RESTARTS   AGE
rag-backend-xxx                 1/1     Running   0          30s
rag-frontend-xxx                1/1     Running   0          25s
```

**バックエンドの起動ログ:**
```
Preloading Level Zero libraries...
Packages pre-installed in Docker image
Starting backend
```

**フロントエンドの起動ログ:**
```
Packages pre-installed in Docker image
Streamlit running...
```

## 変更内容のまとめ

### バックエンド

**変更前:**
- 起動時に毎回pip installを実行（PyPI必須）
- initContainerでモデルとパッケージをダウンロード（Hugging Face、PyPI必須）

**変更後:**
- 全パッケージとモデルをDockerイメージに事前インストール
- 起動時のダウンロード処理を完全削除
- imagePullPolicy: IfNotPresent

### フロントエンド

**変更前:**
- 起動時に毎回pip installを実行（PyPI必須）

**変更後:**
- 全パッケージをDockerイメージに事前インストール
- 起動時のダウンロード処理を完全削除
- imagePullPolicy: IfNotPresent

## トラブルシューティング

### イメージがpullできない場合

```bash
# レジストリの認証情報を確認
kubectl get secret -n rag-company1

# 必要に応じてイメージをtarファイルで配布
docker save -o rag-backend-offline.tar registry.example.com/rag-backend-gpu-offline:latest
docker load -i rag-backend-offline.tar
```

### Podが起動しない場合

```bash
# イベントを確認
kubectl describe pod -n rag-company1 <pod-name>

# ログを確認
kubectl logs -n rag-company1 <pod-name>
```

## 注意事項

1. **初回セットアップはオンライン環境必須**
   - Dockerイメージのビルド時にPyPI、Hugging Faceからダウンロード

2. **イメージサイズが大きい**
   - バックエンド: 約5-6GB
   - レジストリの容量を確認してください

3. **モデルの更新**
   - モデルを更新する場合は、Dockerイメージを再ビルド

4. **他のコンポーネント**
   - Ollama、Milvus、Neo4jも同様に確認が必要
