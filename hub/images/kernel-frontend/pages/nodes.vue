<template>
  <div>
    <div class="page-head">
      <div>
        <h1>ノード</h1>
        <div class="sub">
          集計元 nodes_aggregated (nodes-aggregator が 5 分間隔で更新)
          <template v-if="lastAt"> / 取得 {{ fmtJst(lastAt) }}</template>
        </div>
      </div>
      <button class="quiet" :disabled="loading" @click="load">
        {{ loading ? "取得中" : "再読み込み" }}
      </button>
    </div>

    <div v-if="error" class="note ng">ノード一覧を取得できません: {{ error }}</div>
    <div v-if="aclMessage" class="note">{{ aclMessage }}</div>

    <div class="metrics">
      <div class="metric">
        <div class="label">ノード数</div>
        <div class="value">{{ filtered.length }}</div>
      </div>
      <div class="metric">
        <div class="label">クラスター数</div>
        <div class="value">{{ uniq(filtered, "cluster").length }}</div>
      </div>
      <div class="metric">
        <div class="label">OS 種別</div>
        <div class="value">{{ uniq(filtered, "os").length }}</div>
      </div>
      <div class="metric">
        <div class="label">GPU ノード</div>
        <div class="value">{{ filtered.filter((r) => r.gpu).length }}</div>
      </div>
    </div>

    <section class="section">
      <div class="card">
        <div class="row">
          <div class="field" style="flex: 1; min-width: 180px">
            <label>クラスター</label>
            <select v-model="fCluster">
              <option value="">すべて</option>
              <option v-for="c in uniq(nodes, 'cluster')" :key="c" :value="c">
                {{ c }}
              </option>
            </select>
          </div>
          <div class="field" style="flex: 1; min-width: 180px">
            <label>OS</label>
            <select v-model="fOs">
              <option value="">すべて</option>
              <option v-for="o in uniq(nodes, 'os')" :key="o" :value="o">
                {{ o }}
              </option>
            </select>
          </div>
          <div class="field" style="flex: 2; min-width: 200px">
            <label>名前で絞り込み</label>
            <input v-model="fText" placeholder="ノード名の一部" />
          </div>
          <label class="switch" style="margin-top: 18px">
            <input v-model="fGpu" type="checkbox" />
            <span>GPU のみ</span>
          </label>
        </div>
      </div>
    </section>

    <section class="section">
      <h2>一覧</h2>
      <div class="card" style="overflow-x: auto">
        <table>
          <thead>
            <tr>
              <th>ノード</th>
              <th>クラスター</th>
              <th>ロール</th>
              <th>OS</th>
              <th>カーネル</th>
              <th class="num">CPU</th>
              <th class="num">メモリー</th>
              <th>GPU</th>
              <th>内部 IP</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in filtered" :key="r.cluster + '/' + r.node">
              <td class="mono">{{ r.node }}</td>
              <td>{{ r.cluster || "-" }}</td>
              <td>{{ r.role || "-" }}</td>
              <td>{{ r.os || "-" }}</td>
              <td class="mono small">{{ r.kernel || "-" }}</td>
              <td class="num">{{ r.cpu || "-" }}</td>
              <td class="num">{{ r.memory || "-" }}</td>
              <td class="mono small">{{ r.gpu || "-" }}</td>
              <td class="mono small">{{ r.ip || "-" }}</td>
            </tr>
            <tr v-if="filtered.length === 0">
              <td colspan="9" class="muted">該当するノードがありません。</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <section class="section">
      <h2>内訳</h2>
      <div class="grid grid-2">
        <div v-for="key in ['os', 'kernel', 'role', 'cluster']" :key="key" class="card">
          <h3 style="margin-bottom: 8px">{{ keyLabel(key) }}別</h3>
          <table>
            <tbody>
              <tr v-for="row in countBy(filtered, key)" :key="row.name">
                <td>{{ row.name }}</td>
                <td class="num">{{ row.count }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { osSearch } from "~/utils/opensearch";
import { fmtJst } from "~/utils/format";
import { isClusterAllowed, aclNote } from "~/utils/acl";

const me = await useMe();

interface NodeRow {
  node: string;
  cluster: string;
  role: string;
  os: string;
  kernel: string;
  cpu: string;
  memory: string;
  gpu: string;
  ip: string;
}

const nodes = ref<NodeRow[]>([]);
const error = ref("");
const loading = ref(false);
const lastAt = ref<Date | null>(null);

const fCluster = ref("");
const fOs = ref("");
const fText = ref("");
const fGpu = ref(false);

function pick(obj: any, ...names: string[]): string {
  for (const n of names) {
    const parts = n.split(".");
    let cur = obj;
    for (const p of parts) cur = cur?.[p];
    if (cur !== undefined && cur !== null && cur !== "") return String(cur);
  }
  return "";
}

async function load(): Promise<void> {
  loading.value = true;
  const got = await osSearch<any>("nodes_aggregated", { size: 2000 }, 40);
  loading.value = false;
  lastAt.value = new Date();

  if (!got.ok) {
    error.value = got.error;
    return;
  }
  error.value = "";

  //  見ている人に許されているクラスターのノードだけを持つ。
  //  絞り込みの選択肢もこれから作るので、ここで落とす。
  nodes.value = got.hits
    .filter((n: any) =>
      isClusterAllowed(me.value, String(n.cluster || n.cluster_name || "")),
    )
    .map((n: any) => ({
    node: pick(n, "node", "node_name", "name", "hostname"),
    cluster: pick(n, "cluster", "cluster_name"),
    role: pick(n, "role", "roles", "node_role"),
    os: pick(n, "os.name", "os_name", "os.pretty_name", "os"),
    kernel: pick(n, "os.kernel", "kernel", "kernel_version"),
    cpu: pick(n, "cpu", "cpu_count", "capacity.cpu", "cpu_cores"),
    memory: pick(n, "memory", "memory_total", "capacity.memory"),
    gpu: pick(n, "gpu", "gpu_model", "gpu_name"),
    ip: pick(n, "internal_ip", "ip", "address"),
  }));
}

const aclMessage = computed(() => aclNote(me.value));

const filtered = computed(() =>
  nodes.value.filter((r) => {
    if (fCluster.value && r.cluster !== fCluster.value) return false;
    if (fOs.value && r.os !== fOs.value) return false;
    if (fGpu.value && !r.gpu) return false;
    if (fText.value && !r.node.toLowerCase().includes(fText.value.toLowerCase()))
      return false;
    return true;
  }),
);

function uniq(rows: NodeRow[], key: keyof NodeRow): string[] {
  return [...new Set(rows.map((r) => r[key]).filter(Boolean))].sort();
}

function countBy(rows: NodeRow[], key: string): { name: string; count: number }[] {
  const map = new Map<string, number>();
  for (const r of rows) {
    const v = (r as any)[key];
    if (!v) continue;
    map.set(v, (map.get(v) || 0) + 1);
  }
  return [...map.entries()]
    .map(([name, count]) => ({ name, count }))
    .sort((a, b) => b.count - a.count);
}

function keyLabel(key: string): string {
  return { os: "OS", kernel: "カーネル", role: "ロール", cluster: "クラスター" }[key] || key;
}

onMounted(load);
</script>
