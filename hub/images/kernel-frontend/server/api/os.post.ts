//  ハブ中央の OpenSearch への中継。
//
//  既存の画面 (Streamlit 版の kernel_api.py) と同じ参照先を使う。
//    opensearch.opensearch.svc.cluster.local:9200
//  スポーク側 (opensearch.rag-spoke) は見ない。
//
//  読み取りだけを通す。書き換えは kernel の口を使う。
//
//  使い方: POST /api/os  { index, body, path }
//    index を渡せば  /{index}/_search  を叩く。
//    path を渡せば そのパスをそのまま叩く (_cluster/health など)。

import { readBody, createError } from "h3";

const OS_URL = "http://opensearch.opensearch.svc.cluster.local:9200";

//  読み取りだけ。_search / _count / _cluster/health / _cat 系に限る。
const ALLOW_PATH = /^\/(_cluster\/health|_cat\/[a-z]+|[a-z0-9_.*-]+\/(_search|_count))$/;

export default defineEventHandler(async (event) => {
  const req = await readBody<{
    index?: string;
    body?: unknown;
    path?: string;
    timeout?: number;
  }>(event);

  const path = req?.index ? `/${req.index}/_search` : (req?.path || "").trim();

  if (!ALLOW_PATH.test(path)) {
    throw createError({
      statusCode: 403,
      statusMessage: `このパスへの中継は許していません: ${path}`,
    });
  }

  const timeout = Math.min(Math.max(req?.timeout ?? 30, 5), 120);
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeout * 1000);

  try {
    const hasBody = req?.body !== undefined && req?.body !== null;
    const res = await fetch(`${OS_URL}${path}`, {
      method: hasBody ? "POST" : "GET",
      headers: { "Content-Type": "application/json" },
      body: hasBody ? JSON.stringify(req.body) : undefined,
      signal: ctrl.signal,
    });

    const text = await res.text();
    let parsed: unknown = text;
    try {
      parsed = JSON.parse(text);
    } catch {
      //  JSON でなければそのまま返す (_cat 系)
    }

    if (!res.ok) return { ok: false, status: res.status, detail: parsed };
    return { ok: true, status: res.status, data: parsed };
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, status: 0, detail: message };
  } finally {
    clearTimeout(timer);
  }
});
