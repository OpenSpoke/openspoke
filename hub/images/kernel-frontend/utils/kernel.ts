//  画面から kernel を呼ぶときの入り口。
//
//  画面は kernel のパスだけを書けばよく、中継のことを知らなくて済む。

export interface KernelReply<T = unknown> {
  ok: boolean;
  status: number;
  data?: T;
  detail?: unknown;
}

export async function callKernel<T = unknown>(
  path: string,
  options: {
    method?: string;
    body?: unknown;
    query?: Record<string, string>;
    timeout?: number;
  } = {},
): Promise<KernelReply<T>> {
  try {
    return await $fetch<KernelReply<T>>("/api/kernel", {
      method: "POST",
      body: {
        path,
        method: options.method || "GET",
        body: options.body,
        query: options.query,
        timeout: options.timeout,
      },
    });
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, status: 0, detail: message };
  }
}

/*  クラスターに kubectl を投げる。
 *
 *  読み取りだけに使う。変える操作 (apply / delete / patch 等) は
 *  画面の操作として明示されている場所からだけ呼ぶ。
 */
export async function kubectl(
  cluster: string,
  args: string[],
  timeout = 60,
): Promise<{ ok: boolean; stdout: string; stderr: string }> {
  const reply = await callKernel<{
    exit_code?: number;
    stdout?: string;
    stderr?: string;
  }>("/core/kubectl/execute", {
    method: "POST",
    //  引数の名前は kernel に合わせて cluster_name。
    //  stdin は渡さない (kernel 側で kubeconfig として解釈されるため)。
    body: { cluster_name: cluster, args, timeout },
    timeout,
  });

  if (!reply.ok) {
    return { ok: false, stdout: "", stderr: describe(reply.detail) };
  }
  const out = reply.data || {};
  return {
    ok: (out.exit_code ?? -1) === 0,
    stdout: out.stdout || "",
    stderr: out.stderr || "",
  };
}

/*  kubectl の結果を JSON として読む。 */
export async function kubectlJson<T = any>(
  cluster: string,
  args: string[],
  timeout = 60,
): Promise<{ ok: boolean; data: T | null; error: string }> {
  const res = await kubectl(cluster, [...args, "-o", "json"], timeout);
  if (!res.ok) {
    return { ok: false, data: null, error: res.stderr || "原因不明" };
  }
  try {
    return { ok: true, data: JSON.parse(res.stdout || "{}") as T, error: "" };
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, data: null, error: `読み取れない: ${message}` };
  }
}

export function describe(detail: unknown): string {
  if (!detail) return "";
  if (typeof detail === "string") return detail;
  if (typeof detail === "object") {
    const obj = detail as Record<string, unknown>;
    if (typeof obj.detail === "string") return obj.detail;
    if (typeof obj.message === "string") return obj.message;
    try {
      return JSON.stringify(detail);
    } catch {
      return String(detail);
    }
  }
  return String(detail);
}
