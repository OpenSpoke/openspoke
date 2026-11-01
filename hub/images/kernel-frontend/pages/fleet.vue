<template>
  <div>
    <div class="page-head">
      <div>
        <h1>Fleet</h1>
        <div class="sub">
          GitOps の配信状態 (fleetCluster の GitRepo / Bundle)
          <template v-if="lastAt"> / 取得 {{ fmtJst(lastAt) }}</template>
        </div>
      </div>
      <div class="row">
        <label class="switch">
          <input v-model="auto" type="checkbox" />
          <span>自動更新 (30 秒)</span>
        </label>
        <button class="quiet" :disabled="loading" @click="load">
          {{ loading ? "取得中" : "再読み込み" }}
        </button>
      </div>
    </div>

    <div v-if="notice" class="note" :class="notice.kind">{{ notice.text }}</div>
    <div v-if="error" class="note ng">{{ error }}</div>
    <div v-if="aclMessage" class="note">{{ aclMessage }}</div>

    <div class="metrics">
      <div class="metric">
        <div class="label">GitRepo</div>
        <div class="value">{{ repos.length }}</div>
      </div>
      <div class="metric">
        <div class="label">正常</div>
        <div class="value">{{ repos.filter((r) => r.ready).length }}</div>
      </div>
      <div class="metric">
        <div class="label">反映待ち・失敗</div>
        <div class="value">{{ repos.filter((r) => !r.ready).length }}</div>
      </div>
      <div class="metric">
        <div class="label">取得に失敗</div>
        <div class="value">{{ repos.filter((r) => r.stalled).length }}</div>
      </div>
    </div>

    <section v-if="repos.filter((r) => !r.ready).length" class="section">
      <h2>手当てが必要なもの</h2>
      <p class="section-note">
        反映が済んでいない、または取得に失敗している GitRepo を先に出します。
      </p>

      <details
        v-for="r in repos.filter((x) => !x.ready)"
        :key="'ng-' + r.name"
        class="fold"
        open
      >
        <summary>
          <span class="tag" :class="r.stalled ? 'ng' : 'run'">
            {{ r.stalled ? "取得失敗" : "反映中" }}
          </span>
          <strong>{{ r.name }}</strong>
          <span class="muted mono small">{{ r.commit.slice(0, 7) || "-" }}</span>
          <span class="spacer" />
          <span class="muted small">{{ r.paths[0] || "" }}</span>
        </summary>
        <div class="body">
          <FleetRepoDetail :repo="r" :can-write="canWrite" @sync="forceSync" />
        </div>
      </details>
    </section>

    <section class="section">
      <h2>すべての GitRepo</h2>

      <div class="card" style="margin-bottom: 12px">
        <div class="row">
          <div class="field" style="flex: 2; min-width: 200px">
            <label>名前で絞り込み</label>
            <input v-model="fText" placeholder="GitRepo 名の一部" />
          </div>
          <div class="field" style="flex: 1; min-width: 160px">
            <label>状態</label>
            <select v-model="fState">
              <option value="">すべて</option>
              <option value="ok">正常のみ</option>
              <option value="ng">異常のみ</option>
            </select>
          </div>
        </div>
      </div>

      <details v-for="r in filtered" :key="r.name" class="fold">
        <summary>
          <span class="tag" :class="r.ready ? 'ok' : r.stalled ? 'ng' : 'run'">
            {{ r.ready ? "正常" : r.stalled ? "取得失敗" : "反映中" }}
          </span>
          <strong>{{ r.name }}</strong>
          <span class="muted mono small">{{ r.commit.slice(0, 7) || "-" }}</span>
          <span class="spacer" />
          <span class="muted small">{{ r.paths[0] || "" }}</span>
        </summary>
        <div class="body">
          <FleetRepoDetail :repo="r" :can-write="canWrite" @sync="forceSync" />
        </div>
      </details>

      <div v-if="filtered.length === 0 && !loading" class="card muted">
        該当する GitRepo がありません。
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { kubectlJson, kubectl } from "~/utils/kernel";
import { osSearch } from "~/utils/opensearch";
import { fmtJst, parseUtc } from "~/utils/format";
import { isModuleAllowed, aclNote } from "~/utils/acl";

export interface Repo {
  name: string;
  repo: string;
  branch: string;
  paths: string[];
  commit: string;
  polling: string;
  ready: boolean;
  stalled: boolean;
  conditions: { type: string; status: string; message: string }[];
  summary: { desiredReady?: number; ready?: number } | null;
  forceSync: number;
}

const CLUSTER = "fleetCluster";
const NS = "fleet-default";

const me = await useMe();
const canWrite = computed(() => !me.value.rolesKnown || me.value.isAdmin);

const repos = ref<Repo[]>([]);
const error = ref("");
const loading = ref(false);
const lastAt = ref<Date | null>(null);
const auto = ref(false);
const notice = ref<{ kind: "ok" | "ng"; text: string } | null>(null);

const fText = ref("");
const fState = ref("");

const aclMessage = computed(() => aclNote(me.value));

//  manifest_repo が空のモジュールはこのリポジトリーとみなす (旧 UI と同じ)。
const DEFAULT_MANIFEST_REPO = "https://github.com/your-org/your-repo.git";

function normalizeRepo(url: string): string {
  let u = (url || "").trim();
  while (u.endsWith("/")) u = u.slice(0, -1);
  if (u.endsWith(".git")) u = u.slice(0, -4);
  return u;
}

/*  GitRepo を台帳のモジュールに紐づける。
 *
 *  旧 UI の derive_module_bundles と同じ規則:
 *    1. リポジトリー URL が一致すること (別リポジトリーの同名 path での誤判定を避ける)
 *    2. path が <module.path>/manifests 以下、または module.path 自身かその下
 *  module.path の長い順に見て最長一致とする。
 */
async function loadModuleKeys(): Promise<Map<string, string>> {
  const map = new Map<string, string>();
  const got = await osSearch<any>(
    "fleet_state",
    { size: 1, query: { ids: { values: ["ledger"] } } },
    40,
  );
  if (!got.ok || got.hits.length === 0) return map;

  const index: { path: string; repo: string; key: string }[] = [];
  const cats = (got.hits[0] as any).categories || {};
  for (const [catName, cat] of Object.entries<any>(cats)) {
    for (const [modName, modRaw] of Object.entries<any>(cat || {})) {
      const mod = modRaw || {};
      const p = String(mod.path || "").replace(/\/+$/, "");
      if (!p) continue;
      index.push({
        path: p,
        repo: normalizeRepo(String(mod.manifest_repo || DEFAULT_MANIFEST_REPO)),
        key: `${catName}/${modName}`,
      });
    }
  }
  index.sort((a, b) => b.path.length - a.path.length);

  for (const r of repos.value) {
    const grRepo = normalizeRepo(r.repo || DEFAULT_MANIFEST_REPO);
    for (const p of r.paths) {
      const pn = String(p || "").replace(/\/+$/, "");
      const hit = index.find((m) => {
        if (m.repo !== grRepo) return false;
        const std = `${m.path}/manifests`;
        return (
          pn === std ||
          pn.startsWith(`${std}/`) ||
          pn === m.path ||
          pn.startsWith(`${m.path}/`)
        );
      });
      if (hit) {
        map.set(r.name, hit.key);
        break;
      }
    }
  }
  return map;
}

async function load(): Promise<void> {
  loading.value = true;
  const got = await kubectlJson<{ items: any[] }>(
    CLUSTER,
    ["get", "gitrepo", "-n", NS],
    60,
  );
  loading.value = false;
  lastAt.value = new Date();

  if (!got.ok || !got.data) {
    error.value = `GitRepo 一覧を取得できません: ${got.error}`;
    return;
  }
  error.value = "";

  repos.value = (got.data.items || [])
    .map((item: any) => {
      const spec = item.spec || {};
      const status = item.status || {};
      const conds = (status.conditions || []).map((c: any) => ({
        type: c.type || "",
        status: c.status || "",
        message: c.message || "",
      }));
      const find = (t: string) => conds.find((c: any) => c.type === t);

      return {
        name: item.metadata?.name || "-",
        repo: spec.repo || "",
        branch: spec.branch || "",
        paths: spec.paths || [],
        commit: status.commit || "",
        polling: spec.pollingInterval || "",
        ready: find("Ready")?.status === "True",
        stalled: find("Stalled")?.status === "True",
        conditions: conds,
        summary: status.summary || null,
        forceSync: spec.forceSyncGeneration || 0,
      } as Repo;
    })
    .sort((a, b) => {
      //  手当てが必要なものを上に
      if (a.ready !== b.ready) return a.ready ? 1 : -1;
      return a.name.localeCompare(b.name);
    });

  //  管理者以外には、許されているモジュールの GitRepo だけを残す。
  //  台帳に紐づかないもの (fleet-agent-* など) も出さない。
  if (!me.value.isAdmin) {
    const keys = await loadModuleKeys();
    repos.value = repos.value.filter((r) => {
      const key = keys.get(r.name);
      return key ? isModuleAllowed(me.value, key) : false;
    });
  }
}

const filtered = computed(() =>
  repos.value.filter((r) => {
    if (fText.value && !r.name.toLowerCase().includes(fText.value.toLowerCase()))
      return false;
    if (fState.value === "ok" && !r.ready) return false;
    if (fState.value === "ng" && r.ready) return false;
    return true;
  }),
);

/*  取得が詰まった GitRepo に再取得を促す。
 *  forceSyncGeneration は同じ値だと何も起きないので必ず +1 する。 */
async function forceSync(name: string): Promise<void> {
  const target = repos.value.find((r) => r.name === name);
  const next = (target?.forceSync || 0) + 1;
  const res = await kubectl(
    CLUSTER,
    [
      "patch", "gitrepo", name, "-n", NS,
      "--type", "merge",
      "-p", JSON.stringify({ spec: { forceSyncGeneration: next } }),
    ],
    40,
  );
  notice.value = res.ok
    ? { kind: "ok", text: `${name} に再取得を促しました (forceSyncGeneration ${next})。` }
    : { kind: "ng", text: res.stderr || "再取得を促せません" };
  await load();
}

let timer: ReturnType<typeof setInterval> | null = null;

watch(auto, () => {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
  if (auto.value) timer = setInterval(load, 30_000);
});

onMounted(load);
onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>
