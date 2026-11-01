<template>
  <div>
    <div class="page-head">
      <div>
        <h1>スポーン</h1>
        <div class="sub">
          Claude に投げた非同期の仕事
          <template v-if="lastAt"> / 取得 {{ fmtJst(lastAt) }}</template>
        </div>
      </div>
      <div class="row">
        <label class="switch">
          <input v-model="auto" type="checkbox" />
          <span>自動更新 (15 秒)</span>
        </label>
        <button class="quiet" :disabled="loading" @click="load">
          {{ loading ? "取得中" : "再読み込み" }}
        </button>
      </div>
    </div>

    <div v-if="notice" class="note" :class="notice.kind">{{ notice.text }}</div>
    <div v-if="error" class="note ng">{{ error }}</div>

    <div class="metrics">
      <div class="metric">
        <div class="label">待ち</div>
        <div class="value">{{ byState.queued }}</div>
      </div>
      <div class="metric">
        <div class="label">実行中</div>
        <div class="value">{{ byState.running }}</div>
      </div>
      <div class="metric">
        <div class="label">返答待ち</div>
        <div class="value">{{ byState.waiting_user }}</div>
      </div>
      <div class="metric">
        <div class="label">完了</div>
        <div class="value">{{ byState.done }}</div>
      </div>
      <div class="metric">
        <div class="label">失敗</div>
        <div class="value">{{ byState.error }}</div>
      </div>
    </div>

    <section v-if="waiting.length" class="section">
      <h2>返答を待っているもの</h2>
      <p class="section-note">
        実行中の Claude から質問が来ています。答えると続きが動きます。
      </p>

      <div v-for="s in waiting" :key="s.id" class="card">
        <div class="row">
          <span class="tag run">返答待ち</span>
          <strong>{{ s.alias || s.id }}</strong>
          <span class="muted mono small">{{ s.id }}</span>
        </div>
        <pre class="log" style="margin-top: 12px; max-height: 240px">{{ s.question }}</pre>
        <div v-if="canWrite" class="field" style="margin-top: 12px">
          <label>返答</label>
          <textarea v-model="answers[s.id]" rows="3" placeholder="ここに書いて送る" />
        </div>
        <div v-if="canWrite" class="row" style="margin-top: 8px">
          <button
            class="primary sm"
            :disabled="!answers[s.id]?.trim()"
            @click="answer(s.id)"
          >
            返答を送る
          </button>
        </div>
      </div>
    </section>

    <section class="section">
      <h2>一覧</h2>

      <div class="card" style="margin-bottom: 12px">
        <div class="row">
          <div class="field" style="flex: 1; min-width: 160px">
            <label>状態</label>
            <select v-model="fState">
              <option value="">すべて</option>
              <option v-for="s in STATES" :key="s" :value="s">{{ stateLabel(s) }}</option>
            </select>
          </div>
          <div class="field" style="flex: 2; min-width: 200px">
            <label>名前や依頼文で絞り込み</label>
            <input v-model="fText" placeholder="一部で可" />
          </div>
        </div>
      </div>

      <details v-for="(s, idx) in filtered" :key="s.id" class="fold" :open="idx === 0">
        <summary>
          <span class="tag" :class="stateClass(s.state)">{{ stateLabel(s.state) }}</span>
          <strong>{{ s.alias || s.id }}</strong>
          <span class="muted mono small">{{ s.serial }}</span>
          <span class="spacer" />
          <span class="muted small">{{ fmtJst(parseUtc(s.submittedAt)) }}</span>
          <span class="muted small mono">
            {{ fmtDuration(parseUtc(s.startedAt), parseUtc(s.finishedAt)) }}
          </span>
        </summary>

        <div class="body">
          <table>
            <tbody>
              <tr>
                <th style="width: 140px">ID</th>
                <td class="mono small">{{ s.id }}</td>
              </tr>
              <tr>
                <th>優先度</th>
                <td>{{ prioLabel(s.priority) }}</td>
              </tr>
              <tr v-if="s.projectId">
                <th>プロジェクト</th>
                <td class="mono small">{{ s.projectId }}</td>
              </tr>
              <tr v-if="s.workerId">
                <th>実行した worker</th>
                <td class="mono small">{{ s.workerId }}</td>
              </tr>
              <tr>
                <th>投入</th>
                <td>{{ fmtJst(parseUtc(s.submittedAt)) }}</td>
              </tr>
              <tr v-if="s.startedAt">
                <th>開始</th>
                <td>{{ fmtJst(parseUtc(s.startedAt)) }}</td>
              </tr>
              <tr v-if="s.finishedAt">
                <th>終了</th>
                <td>{{ fmtJst(parseUtc(s.finishedAt)) }}</td>
              </tr>
            </tbody>
          </table>

          <h3 style="margin: 16px 0 8px">依頼した内容</h3>
          <pre class="log" style="max-height: 240px">{{ s.prompt || "-" }}</pre>

          <template v-if="s.result">
            <h3 style="margin: 16px 0 8px">結果</h3>
            <pre class="log" style="max-height: 400px">{{ s.result }}</pre>
          </template>

          <template v-if="s.errorBody">
            <h3 style="margin: 16px 0 8px">エラー</h3>
            <pre class="log" style="max-height: 300px">{{ s.errorBody }}</pre>
          </template>

          <div
            v-if="canWrite && ['queued', 'running', 'waiting_user'].includes(s.state)"
            class="row"
            style="margin-top: 16px"
          >
            <button class="sm danger" @click="cancel(s.id)">中止</button>
          </div>
        </div>
      </details>

      <div v-if="filtered.length === 0 && !loading" class="card muted">
        該当するスポーンがありません。
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { callKernel, describe } from "~/utils/kernel";
import { fmtJst, fmtDuration, parseUtc } from "~/utils/format";

interface Spawn {
  id: string;
  serial: string;
  alias: string;
  state: string;
  priority: string;
  prompt: string;
  result: string;
  errorBody: string;
  question: string;
  projectId: string;
  workerId: string;
  submittedAt: string;
  startedAt: string;
  finishedAt: string;
}

const STATES = ["queued", "running", "waiting_user", "done", "error", "cancelled"];

const me = await useMe();
const canWrite = computed(() => !me.value.rolesKnown || me.value.isAdmin);

const spawns = ref<Spawn[]>([]);
const answers = ref<Record<string, string>>({});
const error = ref("");
const loading = ref(false);
const auto = ref(false);
const lastAt = ref<Date | null>(null);
const notice = ref<{ kind: "ok" | "ng"; text: string } | null>(null);

const fState = ref("");
const fText = ref("");

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}

async function load(): Promise<void> {
  loading.value = true;
  const reply = await callKernel<any>("/core/spawn/list", {
    method: "POST",
    body: { limit: 200 },
    timeout: 40,
  });
  loading.value = false;
  lastAt.value = new Date();

  if (!reply.ok) {
    error.value = `スポーン一覧を取得できません: ${describe(reply.detail)}`;
    return;
  }
  error.value = "";

  const list = reply.data?.spawns || reply.data?.items || reply.data || [];
  spawns.value = (Array.isArray(list) ? list : [])
    .map((s: any) => ({
      id: str(s.full_id || s.id || s._id),
      serial: str(s.serial),
      alias: str(s.alias),
      state: str(s.state) || "unknown",
      priority: str(s.priority) || "normal",
      prompt: str(s.prompt),
      result: str(s.result),
      errorBody: str(s.error_body),
      question: str(s.question),
      projectId: str(s.project_id),
      workerId: str(s.worker_id),
      submittedAt: str(s.submitted_at),
      startedAt: str(s.started_at),
      finishedAt: str(s.finished_at),
    }))
    .sort((a, b) => (b.submittedAt || "").localeCompare(a.submittedAt || ""));
}

const byState = computed(() => {
  const out: Record<string, number> = {
    queued: 0, running: 0, waiting_user: 0, done: 0, error: 0, cancelled: 0,
  };
  for (const s of spawns.value) {
    if (out[s.state] !== undefined) out[s.state] += 1;
  }
  return out;
});

const waiting = computed(() => spawns.value.filter((s) => s.state === "waiting_user"));

const filtered = computed(() =>
  spawns.value.filter((s) => {
    if (fState.value && s.state !== fState.value) return false;
    if (fText.value) {
      const q = fText.value.toLowerCase();
      const hay = `${s.alias} ${s.id} ${s.prompt}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  }),
);

function stateLabel(state: string): string {
  return (
    {
      queued: "待ち",
      running: "実行中",
      waiting_user: "返答待ち",
      done: "完了",
      error: "失敗",
      cancelled: "中止",
    }[state] || state
  );
}

function stateClass(state: string): string {
  if (state === "running" || state === "waiting_user") return "run";
  if (state === "done") return "ok";
  if (state === "error") return "ng";
  return "idle";
}

function prioLabel(p: string): string {
  return { high: "高", normal: "通常", low: "低" }[p] || p;
}

async function answer(id: string): Promise<void> {
  const text = (answers.value[id] || "").trim();
  if (!text) return;

  const reply = await callKernel("/core/spawn/answer", {
    method: "POST",
    body: { full_id: id, answer: text },
    timeout: 30,
  });
  notice.value = reply.ok
    ? { kind: "ok", text: "返答を送りました。続きが動きます。" }
    : { kind: "ng", text: describe(reply.detail) || "返答を送れません" };
  if (reply.ok) answers.value = { ...answers.value, [id]: "" };
  await load();
}

async function cancel(id: string): Promise<void> {
  const reply = await callKernel("/core/spawn/cancel", {
    method: "POST",
    body: { full_id: id },
    timeout: 30,
  });
  notice.value = reply.ok
    ? { kind: "ok", text: `${id} を中止しました。` }
    : { kind: "ng", text: describe(reply.detail) || "中止できません" };
  await load();
}

let timer: ReturnType<typeof setInterval> | null = null;

watch(auto, () => {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
  if (auto.value) timer = setInterval(load, 15_000);
});

onMounted(load);
onUnmounted(() => {
  if (timer) clearInterval(timer);
});
</script>
