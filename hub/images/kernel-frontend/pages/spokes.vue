<template>
  <div>
    <div class="page-head">
      <div>
        <h1>スポーク</h1>
        <div class="sub">
          spoke_clusters に登録されているスポークと、その接続状態 (spoke_health)
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

    <div v-if="error" class="note ng">{{ error }}</div>
    <div v-if="aclMessage" class="note">{{ aclMessage }}</div>

    <div class="metrics">
      <div class="metric">
        <div class="label">稼働中</div>
        <div class="value">{{ counts.active }}</div>
      </div>
      <div class="metric">
        <div class="label">未接続</div>
        <div class="value">{{ counts.unreachable }}</div>
      </div>
      <div class="metric">
        <div class="label">停止</div>
        <div class="value">{{ counts.stopped }}</div>
      </div>
      <div class="metric">
        <div class="label">未確認</div>
        <div class="value">{{ counts.unknown }}</div>
      </div>
    </div>

    <p v-if="hubChannels" class="section-note">
      ハブ (rag) のデータチャンネル: {{ hubChannels }}
    </p>

    <section class="section">
      <h2>つながり</h2>
      <p class="section-note">
        ノードを押すとそのスポークの詳細が開きます。矢印の向きは実際の接続の向きです。
      </p>
      <SpokeGraph v-if="rows.length" :rows="rows" />
    </section>

    <section class="section">
      <h2>ハブとの接続</h2>
      <p class="section-note">
        各スポークは自分からハブへ逆向きにトンネルを張ります。ここに出るのはハブ側が受けている状態です。
        spoke_clusters に無いスポークの古い記録は出しません。
      </p>

      <div class="card" style="overflow-x: auto">
        <table>
          <thead>
            <tr>
              <th>スポーク</th>
              <th>状態</th>
              <th>種類</th>
              <th>用途</th>
              <th class="num">応答</th>
              <th>最終確認</th>
              <th>データチャンネル</th>
              <th>備考</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in rows" :key="r.name">
              <td class="mono">
                {{ r.name }}
                <div v-if="r.url" class="muted mono" style="font-size: 10px">
                  {{ r.url }}
                </div>
              </td>
              <td>
                <span class="tag" :class="tagOf(r.state)">{{ stateLabel(r.state) }}</span>
                <div v-if="r.registered && r.registered !== 'active'" class="muted" style="font-size: 10px">
                  登録: {{ r.registered }}
                </div>
              </td>
              <td class="small">
                {{ r.env || "-" }}
                <div v-if="r.kind" class="muted" style="font-size: 10px">{{ r.kind }}</div>
              </td>
              <td class="small">{{ r.purpose || "-" }}</td>
              <td class="num">{{ r.latency ? `${r.latency} ms` : "-" }}</td>
              <td class="small">
                {{ r.checkedAt ? fmtJst(parseUtc(r.checkedAt)) : "未確認" }}
              </td>
              <td class="small mono">{{ r.channels || "-" }}</td>
              <td class="small muted">{{ r.detail || "" }}</td>
            </tr>
            <tr v-if="rows.length === 0 && !loading">
              <td colspan="8" class="muted">
                見られるスポークがありません。
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <section v-if="orphans.length && me.isAdmin" class="section">
      <h2>spoke_clusters に無い古い記録</h2>
      <p class="section-note">
        spoke_health にだけ残っているものです。管理下から外したあとも probe の記録が消えないため残ります。
        上の表には出していません。
      </p>
      <div class="card">
        <span v-for="o in orphans" :key="o" class="tag plain" style="margin-right: 6px">
          {{ o }}
        </span>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { osSearch } from "~/utils/opensearch";
import { fmtJst, parseUtc } from "~/utils/format";
import { isClusterAllowed, aclNote } from "~/utils/acl";

const me = await useMe();

//  ハブ自身は spoke_clusters に載っているが、スポークの表には出さない。
const HUB = "rag";

const CHANNELS = ["knowledge", "opensearch", "memgraph"];

interface Row {
  name: string;
  state: string;
  registered: string;
  env: string;
  kind: string;
  parent: string;
  purpose: string;
  url: string;
  latency: string;
  checkedAt: string;
  channels: string;
  detail: string;
}

const rows = ref<Row[]>([]);
const orphans = ref<string[]>([]);
const hubChannels = ref("");
const error = ref("");
const loading = ref(false);
const lastAt = ref<Date | null>(null);
const auto = ref(false);

const aclMessage = computed(() => aclNote(me.value));

const counts = computed(() => {
  const c = { active: 0, unreachable: 0, stopped: 0, unknown: 0 } as Record<string, number>;
  for (const r of rows.value) {
    c[r.state] = (c[r.state] ?? 0) + 1;
  }
  return c;
});

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}

function stateLabel(state: string): string {
  if (state === "active") return "稼働中";
  if (state === "unreachable") return "未接続";
  if (state === "stopped") return "停止";
  return "未確認";
}

function tagOf(state: string): string {
  if (state === "active") return "ok";
  if (state === "unreachable") return "ng";
  if (state === "stopped") return "idle";
  return "idle";
}

function channelText(dc: Record<string, string> | undefined): string {
  if (!dc) return "";
  return CHANNELS.map((ch) => `${ch}=${dc[ch] || "unknown"}`).join(" ");
}

async function load(): Promise<void> {
  loading.value = true;
  const [reg, health] = await Promise.all([
    osSearch<any>("spoke_clusters", { size: 100, sort: [{ _id: "asc" }] }, 30),
    //  論理削除された記録 (deleted_at が付いたもの) は除く。
    //  撤去したスポークの古い記録がここに残るため。
    osSearch<any>(
      "spoke_health",
      {
        size: 100,
        query: { bool: { must_not: [{ exists: { field: "deleted_at" } }] } },
      },
      30,
    ),
  ]);
  loading.value = false;
  lastAt.value = new Date();

  if (!reg.ok) {
    error.value = `スポークの一覧を取得できません: ${reg.error}`;
    return;
  }
  if (!health.ok) {
    error.value = `接続状態を取得できません: ${health.error}`;
    return;
  }
  error.value = "";

  //  接続状態をスポーク名で引けるようにする
  const byName = new Map<string, any>();
  for (const h of health.hits) {
    const name = str(h.cluster_name) || str(h._id);
    if (name) byName.set(name, h);
  }

  hubChannels.value = channelText(byName.get(HUB)?.data_channels);

  //  表に出すのは spoke_clusters にあるものだけ。ハブ自身と、
  //  見ている人に許されていないクラスターは除く。
  rows.value = reg.hits
    .map((s: any) => {
      const name = str(s.cluster_name) || str(s._id);
      const h = byName.get(name) || {};
      return {
        name,
        state: str(h.status) || "unknown",
        registered: str(s.status),
        env: str(s.env) || str(s.k8s_distribution),
        kind: str(s.kind),
        parent: str(s.current_cluster),
        purpose: str(s.purpose) || str(s.description),
        url: str(s.url),
        latency: str(h.probe_duration_ms),
        checkedAt: str(h.last_probe_at),
        channels: channelText(h.data_channels),
        detail: str(h.error),
      };
    })
    .filter((r: Row) => r.name && r.name !== HUB)
    .filter((r: Row) =>
      isClusterAllowed(me.value, r.name, {
        kind: r.kind,
        current_cluster: r.parent,
      }),
    )
    .sort((a: Row, b: Row) => a.name.localeCompare(b.name));

  //  spoke_clusters に無い古い記録は、別枠で名前だけ出す (管理者だけ)
  const registered = new Set(
    reg.hits.map((s: any) => str(s.cluster_name) || str(s._id)),
  );
  orphans.value = [...byName.keys()]
    .filter((n) => !registered.has(n))
    .sort((a, b) => a.localeCompare(b));
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
