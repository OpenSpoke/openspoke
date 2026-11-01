//  kernel への中継。
//
//  画面から kernel を直に叩かせない。browser からは届かない場所に
//  あるのと、認証の情報を外に出さないため。
//
//  使い方: POST /api/kernel  { path, method, body, timeout }
//    path は kernel のパス (例 "/core/kubectl/execute")。
//    許す先は下の一覧に書いたものだけ。

import { readBody, createError } from "h3";

//  中継を許す先。前方一致で見る。
//  ここに無いパスは 403 で断る (画面から任意の場所を叩けないように)。
const ALLOW = [
  "/core/kubectl/",
  "/core/kubeconfig",
  "/core/clusters",
  "/core/k8s/",
  "/core/fleet/",
  "/core/ledger",
  "/core/module",
  "/core/bundle",
  "/core/nodes",
  "/core/spoke",
  "/core/handoff/",
  "/core/knowledge/",
  "/core/embedding/",
  "/core/git_credentials/",
  "/core/project/",
  "/core/spawn",
  "/core/chat/",
  "/core/usage/",
  "/core/user_acl",
  "/core/security/",
  "/core/jev/",
  "/core/orchestrate",
  "/core/claude/",
  "/core/graph/",
];

//  kernel には /core 以外にも口がある。使用量はそちら。
const ALLOW_EXTRA = ["/api/usage/"];

export default defineEventHandler(async (event) => {
  const cfg = useRuntimeConfig();
  const body = await readBody<{
    path?: string;
    method?: string;
    body?: unknown;
    query?: Record<string, string>;
    timeout?: number;
  }>(event);

  const path = (body?.path || "").trim();
  if (!path.startsWith("/")) {
    throw createError({ statusCode: 400, statusMessage: "path を /core/... の形で渡してください" });
  }
  if (!ALLOW.some((p) => path.startsWith(p)) && !ALLOW_EXTRA.some((p) => path.startsWith(p))) {
    throw createError({ statusCode: 403, statusMessage: `この先への中継は許していません: ${path}` });
  }

  const method = (body?.method || "GET").toUpperCase();
  const timeout = Math.min(Math.max(body?.timeout ?? 60, 5), 600);

  let url = `${cfg.kernelUrl.replace(/\/$/, "")}${path}`;
  if (body?.query && Object.keys(body.query).length > 0) {
    url += `?${new URLSearchParams(body.query).toString()}`;
  }

  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), (timeout + 15) * 1000);

  try {
    const res = await fetch(url, {
      method,
      headers: {
        "Content-Type": "application/json",
        //  誰の操作かを kernel に伝える (記録と権限の判定に使う)
        "X-Forwarded-User":
          String(event.node.req.headers["x-forwarded-preferred-username"] || ""),
        "X-Forwarded-Email":
          String(event.node.req.headers["x-auth-request-email"] || ""),
        //  ロールも渡す。kernel 側の管理者判定 (git_credentials など) は
        //  X-Forwarded-Groups に role:openspoke_admin があるかだけを見る。
        //  これを落とすと一覧が空になり、登録・削除が 403 になる。
        "X-Forwarded-Groups": String(
          event.node.req.headers["x-forwarded-groups"] ||
            event.node.req.headers["x-auth-request-groups"] ||
            "",
        ),
      },
      body: method === "GET" || method === "HEAD" ? undefined : JSON.stringify(body?.body ?? {}),
      signal: ctrl.signal,
    });

    const text = await res.text();
    let parsed: unknown = text;
    try {
      parsed = JSON.parse(text);
    } catch {
      //  JSON でなければそのまま返す
    }

    if (!res.ok) {
      return { ok: false, status: res.status, detail: parsed };
    }
    return { ok: true, status: res.status, data: parsed };
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, status: 0, detail: message };
  } finally {
    clearTimeout(timer);
  }
});
