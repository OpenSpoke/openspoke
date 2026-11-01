//  監査内容 (audit_specs) の書き込み口。
//
//  監査されるのは AI であって人ではない。だから塞ぐべきなのは
//  「AI が監査内容を書き換えること」であって、人が画面から直すことではない。
//
//    AI の道具 (opensearch_*)  →  audit_ で始まる索引を拒む。書けない
//    人 (この画面)             →  ここを通して書ける
//
//  中央ハブの OpenSearch を直に見る。索引は audit_specs の 1 本だけ、
//  入れ物は current の 1 件だけ。それ以外は通さない。
//
//  使い方: POST /api/auditspec  { spec: <object> }

import { readBody, createError } from "h3";

const OS_URL = "http://opensearch.opensearch.svc.cluster.local:9200";
const INDEX = "audit_specs";
const DOC_ID = "current";

export default defineEventHandler(async (event) => {
  const req = await readBody<{ spec?: unknown; updatedBy?: string }>(event);

  if (!req?.spec || typeof req.spec !== "object" || Array.isArray(req.spec)) {
    throw createError({
      statusCode: 400,
      statusMessage: "spec は入れ子のある 1 つの中身で渡してください。",
    });
  }

  const doc = {
    updated_at: new Date().toISOString(),
    updated_by: typeof req.updatedBy === "string" ? req.updatedBy : "",
    spec: req.spec,
  };

  try {
    const res = await fetch(`${OS_URL}/${INDEX}/_doc/${DOC_ID}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(doc),
    });
    const text = await res.text();
    let parsed: unknown = text;
    try {
      parsed = JSON.parse(text);
    } catch {
      //  JSON でなければそのまま返す
    }
    if (!res.ok) return { ok: false, status: res.status, detail: parsed };
    return { ok: true, status: res.status, data: parsed };
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, status: 0, detail: message };
  }
});
