<template>
  <div>
    <div class="page-head">
      <div>
        <h1>クラスター</h1>
        <div class="sub">
          登録元 spoke_clusters / 死活 spoke_health
          <template v-if="lastAt"> / 取得 {{ fmtJst(lastAt) }}</template>
        </div>
      </div>
      <button class="quiet" :disabled="loading" @click="load">
        {{ loading ? "取得中" : "再読み込み" }}
      </button>
    </div>

    <div v-if="error" class="note ng">{{ error }}</div>
    <div v-if="aclMessage" class="note">{{ aclMessage }}</div>

    <div class="metrics">
      <div class="metric">
        <div class="label">クラスター数</div>
        <div class="value">{{ rows.length }}</div>
      </div>
      <div class="metric">
        <div class="label">稼働中</div>
        <div class="value">{{ rows.filter((r) => r.status === "active").length }}</div>
      </div>
      <div class="metric">
        <div class="label">Rancher 管理下</div>
        <div class="value">{{ rows.filter((r) => r.rancher).length }}</div>
      </div>
      <div class="metric">
        <div class="label">環境の種類</div>
        <div class="value">{{ new Set(rows.map((r) => r.env).filter(Boolean)).size }}</div>
      </div>
    </div>

    <section class="section">
      <h2>一覧</h2>

      <details
        v-for="(r, idx) in rows"
        :key="r.name"
        class="fold"
        :open="idx === 0"
      >
        <summary>
          <span class="tag" :class="r.status === 'active' ? 'ok' : 'idle'">
            {{ r.status || "不明" }}
          </span>
          <strong>{{ r.name }}</strong>
          <span class="muted small">{{ r.env || "-" }}</span>
          <span class="spacer" />
          <span class="muted small">{{ r.purpose || "" }}</span>
        </summary>

        <div class="body">
          <table>
            <tbody>
              <tr v-if="r.description">
                <th style="width: 160px">説明</th>
                <td>{{ r.description }}</td>
              </tr>
              <tr v-if="r.purpose">
                <th>用途</th>
                <td>{{ r.purpose }}</td>
              </tr>
              <tr>
                <th>環境</th>
                <td>{{ r.env || "-" }} / {{ r.distribution || "-" }}</td>
              </tr>
              <tr>
                <th>接続先</th>
                <td class="mono small">{{ r.endpoint || "-" }}</td>
              </tr>
              <tr v-if="r.tunnel">
                <th>Cloudflare Tunnel</th>
                <td class="mono small">{{ r.tunnel }}</td>
              </tr>
              <tr>
                <th>Rancher 管理下</th>
                <td>{{ r.rancher ? "はい" : "いいえ" }}</td>
              </tr>
              <tr>
                <th>所有者</th>
                <td>{{ r.owner || "-" }}</td>
              </tr>
              <tr v-if="r.note">
                <th>備考</th>
                <td>{{ r.note }}</td>
              </tr>
              <tr>
                <th>更新</th>
                <td>{{ fmtJst(parseUtc(r.updatedAt)) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </details>

      <div v-if="rows.length === 0 && !loading" class="card muted">
        見られるクラスターがありません。
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { osSearch } from "~/utils/opensearch";
import { fmtJst, parseUtc } from "~/utils/format";
import { isClusterAllowed, aclNote } from "~/utils/acl";

const me = await useMe();

interface Row {
  name: string;
  env: string;
  distribution: string;
  endpoint: string;
  tunnel: string;
  description: string;
  purpose: string;
  owner: string;
  status: string;
  rancher: boolean;
  note: string;
  updatedAt: string;
}

const rows = ref<Row[]>([]);
const error = ref("");
const loading = ref(false);
const lastAt = ref<Date | null>(null);

function pick(obj: any, ...names: string[]): string {
  for (const n of names) {
    const v = obj?.[n];
    if (v !== undefined && v !== null && v !== "") return String(v);
  }
  return "";
}

async function load(): Promise<void> {
  loading.value = true;
  const got = await osSearch<any>("spoke_clusters", { size: 200 }, 40);
  loading.value = false;
  lastAt.value = new Date();

  if (!got.ok) {
    error.value = `クラスター一覧を取得できません: ${got.error}`;
    return;
  }
  error.value = "";

  rows.value = got.hits
    //  見ている人に許されているクラスターだけを持つ。
    .filter((c: any) =>
      isClusterAllowed(me.value, String(c.cluster_name || c._id || ""), c),
    )
    .map((c: any) => ({
      name: pick(c, "cluster_name", "name", "_id"),
      env: pick(c, "env", "environment"),
      distribution: pick(c, "k8s_distribution", "distribution", "distro"),
      endpoint: pick(c, "url", "endpoint", "mcp_endpoint"),
      tunnel: pick(c, "cloudflare_tunnel_id", "tunnel_id"),
      description: pick(c, "description"),
      purpose: pick(c, "purpose"),
      owner: pick(c, "owner"),
      status: pick(c, "status"),
      rancher: Boolean(c.rancher_managed ?? c.rancher),
      note: pick(c, "note", "notes", "remarks"),
      updatedAt: pick(c, "updated_at", "updatedAt"),
    }))
    .sort((a, b) => a.name.localeCompare(b.name));
}

const aclMessage = computed(() => aclNote(me.value));

onMounted(load);
</script>
