//  oauth2-proxy が差し込むヘッダーから、見ている人と権限を読む。
//
//  既存の画面 (Streamlit 版の user_scope.py) と同じ読み方をする。
//  ロールの値は `role:openspoke_admin,role:default-roles-rag,...` のように
//  Keycloak のロール名に `role:` の接頭辞が付く。外してから比べる。
//
//  管理者でない人には、OpenSearch の user_acl から
//  allowed_clusters / allowed_modules を読んで返す。
//  画面側で任意のインデックスを引けるようにしたくないので、
//  ここでサーバー側だけが読む。

import { getRequestHeaders } from "h3";

const OS_URL = "http://opensearch.opensearch.svc.cluster.local:9200";
const ACL_INDEX = "user_acl";

const ADMIN_ROLE = "openspoke_admin";

//  ロールがヘッダーから取れない場合の予備。既存の画面と同じ。
const ADMIN_EMAILS = new Set(["admin@example.com"]);

function asStrings(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map((v) => String(v)).filter(Boolean);
}

export default defineEventHandler(async (event) => {
  const headers = getRequestHeaders(event);

  const pick = (...names: string[]): string => {
    for (const name of names) {
      const value = headers[name.toLowerCase()];
      if (value) return String(value);
    }
    return "";
  };

  const email = pick(
    "x-auth-request-email",
    "x-forwarded-email",
    "x-auth-email",
  );

  const user = pick(
    "x-forwarded-preferred-username",
    "x-auth-request-preferred-username",
    "x-forwarded-email",
    "x-auth-request-email",
    "x-forwarded-user",
    "x-auth-request-user",
    "x-auth-user",
  );

  //  既存の画面と同じ順でヘッダーを見る。
  const rawRoles = pick("x-forwarded-groups", "x-auth-request-groups");

  //  `role:` の接頭辞を外す。これを忘れると管理者と判定できない。
  const roles = rawRoles
    ? rawRoles
        .split(",")
        .map((r) => r.trim().replace(/^role:/, ""))
        .filter(Boolean)
    : [];

  const isAdmin =
    roles.includes(ADMIN_ROLE) || ADMIN_EMAILS.has(email.toLowerCase());

  const base = {
    user,
    email,
    roles,
    isAdmin,
    //  ヘッダーからロールが取れないときは、画面側で管理者向けの操作を
    //  隠さずに出し、実際の可否は kernel の応答に任せる。
    rolesKnown: roles.length > 0,
  };

  if (isAdmin) {
    return {
      ...base,
      allowedClusters: ["*"],
      allowedModules: ["*"],
      aclKnown: true,
    };
  }

  //  管理者でなければ user_acl を引く。
  //  doc が無い人は何も許されていない (既存の画面と同じ扱い)。
  let allowedClusters: string[] = [];
  let allowedModules: string[] = [];
  let aclKnown = false;

  if (user) {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 5000);
    try {
      const res = await fetch(
        `${OS_URL}/${ACL_INDEX}/_doc/${encodeURIComponent(user)}`,
        { signal: ctrl.signal },
      );
      if (res.ok) {
        const doc = (await res.json()) as { _source?: Record<string, unknown> };
        const src = doc?._source || {};
        allowedClusters = asStrings(src.allowed_clusters);
        allowedModules = asStrings(src.allowed_modules);
        aclKnown = true;
      } else if (res.status === 404) {
        //  登録が無い人。許可なしとして扱う。
        aclKnown = true;
      }
    } catch {
      //  OpenSearch に届かないとき。許可は増やさず、
      //  読めなかったことを画面に伝えて断り書きを出させる。
      aclKnown = false;
    } finally {
      clearTimeout(timer);
    }
  } else {
    //  利用者が分からない。許可なし。
    aclKnown = true;
  }

  return { ...base, allowedClusters, allowedModules, aclKnown };
});
