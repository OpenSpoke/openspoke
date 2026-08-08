"""
RAG MCP Server
テナント単位でデプロイ。環境変数 TENANT_NAME / RAG_BACKEND_URL で動作を切り替える。
"""

import os
import json
import httpx
from mcp.server.fastmcp import FastMCP

# ── 環境変数 ────────────────────────────────────────────────
TENANT      = os.environ["TENANT_NAME"]       # e.g. "company1"
RAG_URL     = os.environ["RAG_BACKEND_URL"]   # e.g. http://rag-backend.rag-company1.svc.cluster.local:8000
MCP_HOST    = os.environ.get("MCP_HOST", "0.0.0.0")
MCP_PORT    = int(os.environ.get("MCP_PORT", "8000"))

# ── MCP サーバー初期化 ──────────────────────────────────────
mcp = FastMCP(f"RAG-{TENANT}")

_client = httpx.AsyncClient(timeout=120.0)


# ── ツール定義 ──────────────────────────────────────────────

@mcp.tool()
async def search_rag(
    question: str,
    user_id: str,
    top_k: int = 5,
    search_mode: str = "vector_first",
    use_graph: bool = True,
) -> str:
    """
    社内ナレッジベースを検索してLLMが生成した回答を返す。

    Args:
        question:    自然言語での質問（日本語・英語可）
        user_id:     検索対象ユーザーのID（例: user_example_com）
        top_k:       取得するチャンク数。デフォルト5、最大20
        search_mode: "vector_first"（ベクトル優先）または "graph_first"（グラフ優先）
        use_graph:   vector_first 時にグラフ補強を使うか（graph_first 時は無視）
    """
    payload = {
        "question":    question,
        "user_id":     user_id,
        "top_k":       top_k,
        "search_mode": search_mode,
        "use_graph":   use_graph,
    }
    resp = await _client.post(f"{RAG_URL}/query", json=payload)

    if resp.status_code == 404:
        return f"ユーザー '{user_id}' のデータが存在しません。先にドキュメントをインデックス化してください。"

    resp.raise_for_status()
    data = resp.json()

    # ── 回答の組み立て ──────────────────────────────────────
    lines = []
    lines.append(f"## 回答\n{data['answer']}")

    # リランキング情報
    ri = data.get("rerank_info")
    if ri and ri.get("reranked"):
        lines.append(
            f"\n_[リランキング] {ri['candidates_count']}件 → {len(ri['selected_indices'])}件選択 "
            f"/ 処理時間: {data.get('processing_time_sec', 0):.2f}秒_"
        )

    # グラフエンティティ
    entities = data.get("graph_entities", [])
    if entities:
        lines.append("\n## 関連エンティティ")
        for e in entities[:8]:
            related = ", ".join(r["name"] for r in e.get("related", [])[:3] if r.get("name"))
            desc = e.get("description", "")[:60]
            lines.append(f"- **{e['entity']}** ({e['type']}){': ' + desc if desc else ''}"
                         + (f" → {related}" if related else ""))

    # 参照ドキュメント
    docs = data.get("retrieved_documents", [])
    if docs:
        lines.append("\n## 参照ドキュメント")
        src_map = {"vector": "ベクトル", "graph": "グラフ", "vector_fallback": "フォールバック"}
        for i, doc in enumerate(docs, 1):
            src = src_map.get(doc.get("source", "vector"), doc.get("source", ""))
            lines.append(f"\n### [{i}] スコア: {doc['score']:.3f}  ({src})")
            lines.append(doc["text"][:300] + ("…" if len(doc["text"]) > 300 else ""))

    return "\n".join(lines)


@mcp.tool()
async def search_code(
    query: str,
    user_id: str,
    top_k: int = 3,
) -> str:
    """
    インデックス済みの GitHub コードを意味検索する。

    Args:
        query:   検索クエリ（日本語・英語可。例: 認証処理、database connection）
        user_id: 検索対象ユーザーのID
        top_k:   取得件数。デフォルト3、最大10
    """
    payload = {"query": query, "user_id": user_id, "top_k": top_k}
    resp = await _client.post(f"{RAG_URL}/code/search", json=payload)

    if resp.status_code == 404:
        return "コードがインデックス化されていません。GitHub リポジトリを登録してください。"

    resp.raise_for_status()
    data = resp.json()
    results = data.get("results", [])

    if not results:
        return "該当するコードが見つかりませんでした。"

    lines = [f"## コード検索結果 ({len(results)}件)\n"]
    for r in results:
        lines.append(f"### {r['filepath']}  L{r['start_line']}-{r['end_line']}  スコア: {r['score']:.3f}")
        lines.append(f"_Repository: {r['repository']} / Branch: {r['branch']}_")
        lines.append(f"```\n{r['text']}\n```\n")

    return "\n".join(lines)


@mcp.tool()
async def list_users() -> str:
    """
    このテナントに登録されているユーザー一覧とドキュメント件数を返す。
    """
    resp = await _client.get(f"{RAG_URL}/users")
    resp.raise_for_status()
    data = resp.json()

    org  = data.get("organization", TENANT)
    users = data.get("users", [])

    if not users:
        return f"組織 '{org}' にユーザーはまだ登録されていません。"

    lines = [f"## 組織: {org}  ({len(users)}名)\n"]
    for u in users:
        lines.append(f"- **{u['user_id']}**: {u['num_entities']}件")

    return "\n".join(lines)


@mcp.tool()
async def get_user_info(user_id: str) -> str:
    """
    指定ユーザーのコレクション情報（ドキュメント数・グラフエンティティ数）を返す。

    Args:
        user_id: 情報を取得したいユーザーのID
    """
    resp = await _client.get(f"{RAG_URL}/collection/info", params={"user_id": user_id})
    resp.raise_for_status()
    info = resp.json()

    if not info.get("exists"):
        return f"ユーザー '{user_id}' のデータは存在しません。"

    lines = [
        f"## ユーザー: {user_id}",
        f"- ドキュメント数: {info['num_entities']}件",
    ]
    if info.get("graph_entities", 0) > 0:
        lines.append(f"- グラフエンティティ数: {info['graph_entities']}件")

    return "\n".join(lines)


# ── エントリーポイント ────────────────────────────────────────
if __name__ == "__main__":
    mcp.run(transport="sse", host=MCP_HOST, port=MCP_PORT)
