<template>
  <div>
    <div class="page-head">
      <div>
        <h1>コスト</h1>
        <div class="sub">Claude API の使用量と料金</div>
      </div>
      <div class="row">
        <div class="segmented">
          <button
            v-for="p in PERIODS"
            :key="p.key"
            :class="{ on: period === p.key }"
            @click="period = p.key"
          >
            {{ p.label }}
          </button>
        </div>
        <button class="quiet" :disabled="loading" @click="load">
          {{ loading ? "取得中" : "再読み込み" }}
        </button>
      </div>
    </div>

    <div v-if="error" class="note ng">{{ error }}</div>

    <div class="metrics">
      <div class="metric">
        <div class="label">料金 ({{ periodLabel }})</div>
        <div class="value">{{ usd(total.cost) }}</div>
      </div>
      <div class="metric">
        <div class="label">呼び出し回数</div>
        <div class="value">{{ total.calls.toLocaleString() }}</div>
      </div>
      <div class="metric">
        <div class="label">入力トークン</div>
        <div class="value">{{ compact(total.input) }}</div>
      </div>
      <div class="metric">
        <div class="label">出力トークン</div>
        <div class="value">{{ compact(total.output) }}</div>
      </div>
      <div class="metric">
        <div class="label">キャッシュ読み</div>
        <div class="value">{{ compact(total.cacheRead) }}</div>
      </div>
    </div>

    <section class="section">
      <h2>操作ごと</h2>
      <p class="section-note">料金の多い順。横棒の長さが比率です。</p>

      <div v-if="byOp.length === 0 && !loading" class="card muted">
        この期間の記録がありません。
      </div>

      <div v-else class="card">
        <div v-for="row in byOp" :key="row.op" style="margin-bottom: 10px">
          <div class="row" style="gap: 8px; margin-bottom: 3px">
            <span class="mono small" style="flex: 1">{{ row.op }}</span>
            <span class="mono small muted">{{ row.calls.toLocaleString() }} 回</span>
            <span class="mono small" style="width: 92px; text-align: right">
              {{ usd(row.cost) }}
            </span>
          </div>
          <div class="tl-track" style="height: 8px">
            <div
              class="tl-bar"
              style="background: var(--accent)"
              :style="{ width: barWidth(row.cost) + '%' }"
            />
          </div>
        </div>
      </div>
    </section>

    <section class="section">
      <h2>日ごとの推移 (過去 {{ days }} 日)</h2>

      <div v-if="daily.length === 0 && !loading" class="card muted">
        日ごとの記録がありません。
      </div>

      <div v-else class="card">
        <div
          v-for="d in daily"
          :key="d.date"
          class="row"
          style="gap: 8px; margin-bottom: 4px"
        >
          <span class="mono small" style="width: 90px">{{ d.date }}</span>
          <div class="tl-track" style="height: 14px">
            <div
              class="tl-bar"
              style="background: var(--accent)"
              :style="{ width: dailyWidth(d.cost) + '%' }"
            />
          </div>
          <span class="mono small" style="width: 92px; text-align: right">
            {{ usd(d.cost) }}
          </span>
        </div>
      </div>
    </section>

    <section class="section">
      <h2>明細</h2>
      <div class="card" style="overflow-x: auto">
        <table>
          <thead>
            <tr>
              <th>操作</th>
              <th class="num">回数</th>
              <th class="num">入力</th>
              <th class="num">出力</th>
              <th class="num">キャッシュ書き</th>
              <th class="num">キャッシュ読み</th>
              <th class="num">料金</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in byOp" :key="'t-' + row.op">
              <td class="mono small">{{ row.op }}</td>
              <td class="num">{{ row.calls.toLocaleString() }}</td>
              <td class="num">{{ compact(row.input) }}</td>
              <td class="num">{{ compact(row.output) }}</td>
              <td class="num">{{ compact(row.cacheWrite) }}</td>
              <td class="num">{{ compact(row.cacheRead) }}</td>
              <td class="num">{{ usd(row.cost) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { callKernel, describe } from "~/utils/kernel";

const PERIODS = [
  { key: "today", label: "今日", days: 7 },
  { key: "7d", label: "7 日", days: 7 },
  { key: "30d", label: "30 日", days: 30 },
  { key: "all", label: "全期間", days: 90 },
];

interface OpRow {
  op: string;
  calls: number;
  input: number;
  output: number;
  cacheWrite: number;
  cacheRead: number;
  cost: number;
}

const period = ref("7d");
const byOp = ref<OpRow[]>([]);
const daily = ref<{ date: string; cost: number }[]>([]);
const error = ref("");
const loading = ref(false);

const days = computed(
  () => PERIODS.find((p) => p.key === period.value)?.days || 7,
);
const periodLabel = computed(
  () => PERIODS.find((p) => p.key === period.value)?.label || "",
);

function num(v: unknown): number {
  const n = Number(v);
  return Number.isFinite(n) ? n : 0;
}

async function load(): Promise<void> {
  loading.value = true;
  error.value = "";

  const [opReply, dayReply] = await Promise.all([
    callKernel<any>("/api/usage/by_op", {
      query: { period: period.value },
      timeout: 40,
    }),
    callKernel<any>("/api/usage/daily", {
      query: { days: String(days.value) },
      timeout: 40,
    }),
  ]);

  loading.value = false;

  if (!opReply.ok) {
    error.value = `使用量を取得できません: ${describe(opReply.detail)}`;
  } else {
    const list = opReply.data?.by_op || [];
    byOp.value = (Array.isArray(list) ? list : [])
      .map((r: any) => ({
        op: String(r.op || "-"),
        calls: num(r.calls ?? r.count),
        input: num(r.input_tokens),
        output: num(r.output_tokens),
        cacheWrite: num(r.cache_creation_tokens ?? r.cache_w_tokens),
        cacheRead: num(r.cache_read_tokens ?? r.cache_r_tokens),
        cost: num(r.cost_usd),
      }))
      .sort((a, b) => b.cost - a.cost);
  }

  if (dayReply.ok) {
    const list = dayReply.data?.daily || [];
    daily.value = (Array.isArray(list) ? list : [])
      .map((d: any) => ({ date: String(d.date || ""), cost: num(d.cost_usd) }))
      .sort((a, b) => a.date.localeCompare(b.date));
  }
}

const total = computed(() => ({
  cost: byOp.value.reduce((s, r) => s + r.cost, 0),
  calls: byOp.value.reduce((s, r) => s + r.calls, 0),
  input: byOp.value.reduce((s, r) => s + r.input, 0),
  output: byOp.value.reduce((s, r) => s + r.output, 0),
  cacheRead: byOp.value.reduce((s, r) => s + r.cacheRead, 0),
}));

function barWidth(cost: number): number {
  const max = Math.max(...byOp.value.map((r) => r.cost), 0.0001);
  return Math.max((cost / max) * 100, 0.5);
}

function dailyWidth(cost: number): number {
  const max = Math.max(...daily.value.map((d) => d.cost), 0.0001);
  return Math.max((cost / max) * 100, 0.5);
}

function usd(v: number): string {
  if (v === 0) return "$0";
  if (v < 0.01) return `$${v.toFixed(4)}`;
  if (v < 100) return `$${v.toFixed(2)}`;
  return `$${Math.round(v).toLocaleString()}`;
}

function compact(v: number): string {
  if (v >= 1_000_000_000) return `${(v / 1_000_000_000).toFixed(1)}G`;
  if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(1)}M`;
  if (v >= 1_000) return `${(v / 1_000).toFixed(1)}K`;
  return String(v);
}

watch(period, load);
onMounted(load);
</script>
