//  OpenSearch を引くときの入り口。
//
//  既存の画面と同じインデックスを見る。
//    spoke_clusters / spoke_health / nodes_aggregated / fleet_state
//    gitrepo_metadata / claude_usage / spawns / projects など

export interface OsReply<T = unknown> {
  ok: boolean;
  status: number;
  data?: T;
  detail?: unknown;
}

/*  インデックスを検索する。size の既定は 1000。 */
export async function osSearch<T = any>(
  index: string,
  body: Record<string, unknown> = {},
  timeout = 30,
): Promise<{ ok: boolean; hits: T[]; total: number; raw: any; error: string }> {
  const payload = { size: 1000, ...body };
  try {
    const reply = await $fetch<OsReply<any>>("/api/os", {
      method: "POST",
      body: { index, body: payload, timeout },
    });

    if (!reply.ok) {
      return { ok: false, hits: [], total: 0, raw: null, error: describeOs(reply.detail) };
    }

    const raw = reply.data || {};
    const hitList = raw?.hits?.hits || [];
    const total =
      typeof raw?.hits?.total === "object"
        ? raw.hits.total.value
        : raw?.hits?.total || hitList.length;

    return {
      ok: true,
      hits: hitList.map((h: any) => ({ ...(h._source || {}), _id: h._id })),
      total,
      raw,
      error: "",
    };
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, hits: [], total: 0, raw: null, error: message };
  }
}

/*  集計だけを取りたいとき (hits は要らない)。 */
export async function osAggregate(
  index: string,
  aggs: Record<string, unknown>,
  query?: Record<string, unknown>,
  timeout = 30,
): Promise<{ ok: boolean; aggs: any; error: string }> {
  const body: Record<string, unknown> = { size: 0, aggs };
  if (query) body.query = query;

  try {
    const reply = await $fetch<OsReply<any>>("/api/os", {
      method: "POST",
      body: { index, body, timeout },
    });
    if (!reply.ok) {
      return { ok: false, aggs: null, error: describeOs(reply.detail) };
    }
    return { ok: true, aggs: (reply.data as any)?.aggregations || {}, error: "" };
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, aggs: null, error: message };
  }
}

export function describeOs(detail: unknown): string {
  if (!detail) return "";
  if (typeof detail === "string") return detail;
  if (typeof detail === "object") {
    const obj = detail as Record<string, any>;
    if (obj.error?.reason) return String(obj.error.reason);
    if (typeof obj.detail === "string") return obj.detail;
    try {
      return JSON.stringify(detail).slice(0, 300);
    } catch {
      return String(detail);
    }
  }
  return String(detail);
}
