//  Keycloak の管理 API への中継。
//
//  鍵 (クライアントシークレット) は server 側だけが持つ。
//  ブラウザーには渡さない。
//
//  使い方: POST /api/keycloak  { path, method, body }
//    path は realm より後ろ (例 "/users?max=200", "/roles")。

import { readBody, createError } from "h3";

//  中継を許す先。利用者とロールの管理に要るものだけ。
const ALLOW = [
  /^\/users(\?.*)?$/,
  /^\/users\/count(\?.*)?$/,
  /^\/users\/[\w-]+$/,
  /^\/users\/[\w-]+\/role-mappings\/realm$/,
  /^\/users\/[\w-]+\/reset-password$/,
  /^\/users\/[\w-]+\/execute-actions-email$/,
  /^\/roles(\?.*)?$/,
  /^\/roles\/[\w:-]+$/,
];

let cached: { token: string; until: number } | null = null;

async function getToken(cfg: ReturnType<typeof useRuntimeConfig>): Promise<string> {
  const now = Date.now();
  if (cached && cached.until > now + 10_000) return cached.token;

  const url =
    `${cfg.keycloakUrl.replace(/\/$/, "")}` +
    `/realms/${cfg.keycloakRealm}/protocol/openid-connect/token`;

  const form = new URLSearchParams({
    grant_type: "client_credentials",
    client_id: String(cfg.keycloakAdminClientId || ""),
    client_secret: String(cfg.keycloakAdminClientSecret || ""),
  });

  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: form.toString(),
  });

  if (!res.ok) {
    throw createError({
      statusCode: 502,
      statusMessage: `Keycloak の管理用の鍵を取得できません (${res.status})`,
    });
  }

  const data = (await res.json()) as { access_token?: string; expires_in?: number };
  if (!data.access_token) {
    throw createError({ statusCode: 502, statusMessage: "鍵が返ってきません" });
  }

  cached = {
    token: data.access_token,
    until: now + (data.expires_in || 60) * 1000,
  };
  return cached.token;
}

export default defineEventHandler(async (event) => {
  const cfg = useRuntimeConfig();

  if (!cfg.keycloakAdminClientSecret) {
    return { ok: false, status: 0, detail: "Keycloak の管理用の鍵が設定されていません" };
  }

  const req = await readBody<{ path?: string; method?: string; body?: unknown }>(event);
  const path = (req?.path || "").trim();

  if (!ALLOW.some((re) => re.test(path))) {
    throw createError({
      statusCode: 403,
      statusMessage: `この先への中継は許していません: ${path}`,
    });
  }

  const method = (req?.method || "GET").toUpperCase();

  try {
    const token = await getToken(cfg);
    const url =
      `${cfg.keycloakUrl.replace(/\/$/, "")}` +
      `/admin/realms/${cfg.keycloakRealm}${path}`;

    const res = await fetch(url, {
      method,
      headers: {
        Authorization: `Bearer ${token}`,
        "Content-Type": "application/json",
      },
      body:
        method === "GET" || method === "HEAD" || req?.body === undefined
          ? undefined
          : JSON.stringify(req.body),
    });

    const text = await res.text();
    let parsed: unknown = text;
    try {
      parsed = text ? JSON.parse(text) : null;
    } catch {
      //  本文が無い応答 (204 など) もある
    }

    if (!res.ok) return { ok: false, status: res.status, detail: parsed };
    return { ok: true, status: res.status, data: parsed };
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, status: 0, detail: message };
  }
});
