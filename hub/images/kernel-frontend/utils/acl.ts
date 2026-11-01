//  見ている人に何を見せてよいかの判定。
//
//  旧 UI (user_scope.py の is_cluster_allowed / is_module_allowed) と
//  同じ規則にしてある。判定を変えるときは両方を直すこと。

import type { Me } from "~/composables/useMe";

export interface ClusterEntry {
  kind?: string;
  current_cluster?: string;
}

/*  クラスターを見せてよいか。
 *
 *  kind=application のアプリケーションスポークは、それ自身の名前ではなく
 *  載っているクラスター (current_cluster) で判定する。
 */
export function isClusterAllowed(
  me: Me,
  clusterName: string,
  entry?: ClusterEntry | null,
): boolean {
  if (me.isAdmin) return true;
  const allowed = me.allowedClusters || [];
  if (allowed.includes("*")) return true;
  if (entry && entry.kind === "application") {
    return allowed.includes(String(entry.current_cluster || ""));
  }
  return allowed.includes(clusterName);
}

/*  モジュールを見せてよいか。key は "Core/Hub" のような形。
 *
 *  許可の側に "Hub" のようにモジュール名だけが入っていることもあるので、
 *  後ろの部分だけでも照合する (旧 UI と同じ)。
 */
export function isModuleAllowed(me: Me, key: string): boolean {
  if (me.isAdmin) return true;
  const allowed = me.allowedModules || [];
  if (allowed.includes("*")) return true;
  if (allowed.includes(key)) return true;
  if (key.includes("/")) {
    const last = key.slice(key.lastIndexOf("/") + 1);
    if (allowed.includes(last)) return true;
  }
  return false;
}

/*  権限が読めなかったときに画面へ出す断り書き。
 *  読めないときは許可を増やさないので、見えるものが減る。
 */
export function aclNote(me: Me): string {
  if (me.isAdmin) return "";
  if (!me.aclKnown) {
    return "権限を読めませんでした。見えていない対象があります。";
  }
  if ((me.allowedClusters || []).length === 0) {
    return "見られるクラスターが登録されていません。管理者に問い合わせてください。";
  }
  return "";
}
