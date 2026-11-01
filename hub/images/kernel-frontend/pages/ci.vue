<template>
  <div>
    <div class="page-head">
      <div>
        <h1>CI</h1>
        <div class="sub">
          日本標準時 / クラスター rag / 名前空間 builder
          <template v-if="lastAt"> / この表示 {{ fmtJst(lastAt) }}</template>
        </div>
      </div>
      <div class="row">
        <label class="switch">
          <input v-model="auto" type="checkbox" />
          <span>自動更新</span>
        </label>
        <div class="segmented">
          <button
            v-for="w in WINDOWS"
            :key="w"
            :class="{ on: windowMin === w }"
            @click="windowMin = w"
          >
            {{ shortWindow(w) }}
          </button>
        </div>
        <button class="quiet" :disabled="loading" @click="load">
          {{ loading ? "取得中" : "更新" }}
        </button>
      </div>
    </div>

    <div v-if="notice" class="note" :class="notice.kind">
      {{ notice.text }}
    </div>

    <div v-if="error" class="note ng">
      ビルドの一覧を取れません: {{ error }}
    </div>

    <div class="metrics">
      <div class="metric">
        <div class="label">実行中</div>
        <div class="value">{{ counts.run }}</div>
      </div>
      <div class="metric">
        <div class="label">待機</div>
        <div class="value">{{ counts.idle }}</div>
      </div>
      <div class="metric">
        <div class="label">同時に動いた最大</div>
        <div class="value">{{ peak }}</div>
      </div>
      <div class="metric">
        <div class="label">成功 ({{ windowLabel(windowMin) }})</div>
        <div class="value">{{ counts.ok }}</div>
      </div>
      <div class="metric">
        <div class="label">失敗 ({{ windowLabel(windowMin) }})</div>
        <div class="value">{{ counts.ng }}</div>
      </div>
    </div>

    <p v-if="dispatcher" class="section-note" style="margin-top: 12px">
      ディスパッチャー ci-dispatcher:
      {{ dispatcher.suspend ? "止めてある" : "動いている" }}
      / 間隔 {{ dispatcher.schedule }}
      / 最後の起動 {{ fmtJst(dispatcher.last) }}
    </p>

    <section class="section">
      <h2>並行して動いている様子</h2>
      <p class="section-note">
        横が時間、1 行が 1 本のビルド。同じ時刻に棒が並んでいれば、それらは同時に走っています。
      </p>

      <div v-if="inWindow.length === 0" class="card muted">
        この時間帯に動いたビルドはありません。
      </div>

      <div v-else class="card">
        <div class="tl-row" style="margin-bottom: 6px">
          <div class="tl-name" />
          <div class="tl-ticks" style="flex: 1">
            <span v-for="(t, i) in ticks" :key="i">{{ t }}</span>
          </div>
          <div class="tl-dur" />
        </div>

        <div v-for="row in timeline" :key="row.name" class="tl-row">
          <div class="tl-name">
            {{ row.target }} / {{ row.job }}
            <span class="muted mono">{{ row.sha7 }}</span>
          </div>
          <div class="tl-track">
            <div
              class="tl-bar"
              :class="stateClass(row.state)"
              :style="{ left: row.left + '%', width: row.width + '%' }"
              :title="`${row.state} ${fmtJst(row.start)}`"
            />
          </div>
          <div class="tl-dur">{{ fmtDuration(row.start, row.end) }}</div>
        </div>

        <div class="legend">
          <span><i style="background: var(--run)" />実行中</span>
          <span><i style="background: var(--ok)" />成功</span>
          <span><i style="background: var(--ng)" />失敗</span>
          <span><i style="background: var(--idle)" />待機</span>
        </div>
      </div>
    </section>

    <section class="section">
      <h2>いま動いているビルド</h2>

      <div v-if="running.length === 0" class="card muted">
        いま動いているビルドはありません。
      </div>

      <div v-else class="grid grid-3">
        <div v-for="row in running" :key="row.name" class="card">
          <div class="row" style="justify-content: space-between">
            <strong>{{ row.target }} / {{ row.job }}</strong>
            <span class="tag" :class="stateClass(row.state)">{{ row.state }}</span>
          </div>
          <div class="small muted" style="margin-top: 6px">
            経過 {{ fmtDuration(row.start, null) }} / コミット
            <span class="mono">{{ row.sha7 }}</span>
          </div>
          <div class="small muted">開始 {{ fmtJst(row.start) }}</div>

          <details class="fold" style="margin-top: 12px">
            <summary>途中のログ</summary>
            <div class="body">
              <pre class="log">{{ logs[row.name] || "(開くと取得します)" }}</pre>
              <button class="sm quiet" @click="loadLog(row.name, 200)">
                読み直す
              </button>
            </div>
          </details>

          <div v-if="canWrite" class="row" style="margin-top: 12px">
            <button class="sm danger" @click="stop(row.name)">止める</button>
          </div>
        </div>
      </div>
    </section>

    <section class="section">
      <h2>ビルドを開始</h2>

      <div v-if="targets.length === 0" class="card muted">
        対象の一覧が空です。ci-dispatcher-config の targets.json に対象を足してください。
      </div>

      <div v-else class="card">
        <div class="row">
          <div class="field" style="flex: 1; min-width: 240px">
            <label>対象</label>
            <select v-model="picked">
              <option v-for="t in targets" :key="t.name" :value="t.name">
                {{ t.name }}
              </option>
            </select>
          </div>
          <button
            class="primary"
            :disabled="!canWrite || starting"
            style="margin-top: 18px"
            @click="start(picked)"
          >
            {{ starting ? "依頼中" : "ビルドを開始" }}
          </button>
        </div>

        <p class="section-note" style="margin: 12px 0 0">
          <span class="mono">{{ pickedTarget?.repo }}</span>
          (枝: {{ pickedTarget?.branch || "main" }})
          <template v-if="pickedTarget?.path">
            <br />見る場所: <span class="mono">{{ pickedTarget.path }}</span>
          </template>
          <br />ビルドの中身は、このリポジトリーの
          <span class="mono">{{ pickedTarget?.spec || ".openspoke/build.json" }}</span>
          に書かれています。
        </p>

        <p v-if="!canWrite" class="section-note" style="margin: 8px 0 0">
          ビルドを開始できるのは管理者だけです。
        </p>
      </div>
    </section>

    <section class="section">
      <h2>終わったビルド</h2>

      <div v-if="done.length === 0" class="card muted">
        終わったビルドはまだありません。
      </div>

      <details
        v-for="(row, idx) in done"
        :key="row.name"
        class="fold"
        :open="idx === 0"
        @toggle="onFold(row.name, $event)"
      >
        <summary>
          <span class="tag" :class="stateClass(row.state)">{{ row.state }}</span>
          <span>{{ row.target }} / {{ row.job }}</span>
          <span class="muted mono">{{ row.sha7 }}</span>
          <span class="spacer" />
          <span class="muted small">{{ fmtJst(row.start) }}</span>
          <span class="muted small mono">{{ fmtDuration(row.start, row.end) }}</span>
        </summary>

        <div class="body">
          <table>
            <tbody>
              <tr>
                <th style="width: 140px">リポジトリー</th>
                <td class="mono">{{ row.repo || "-" }}</td>
              </tr>
              <tr>
                <th>枝</th>
                <td>{{ row.branch || "-" }}</td>
              </tr>
              <tr>
                <th>コミット</th>
                <td class="mono">{{ row.sha || row.sha7 }}</td>
              </tr>
              <tr>
                <th>{{ row.kind === "image" ? "ビルドしたイメージ" : "実行イメージ" }}</th>
                <td class="mono">{{ row.image || "-" }}</td>
              </tr>
              <tr>
                <th>起動元</th>
                <td>{{ row.trigger === "manual" ? "手動" : "自動" }}</td>
              </tr>
              <tr>
                <th>ジョブ名</th>
                <td class="mono">{{ row.name }}</td>
              </tr>
            </tbody>
          </table>

          <pre class="log" style="margin-top: 12px">{{
            logs[row.name] || "(読み込み中)"
          }}</pre>

          <div v-if="canWrite" class="row" style="margin-top: 12px">
            <button class="sm" @click="start(row.target)">
              この対象をもう一度ビルドする
            </button>
            <button class="sm quiet" @click="loadLog(row.name, 500)">
              ログを読み直す
            </button>
            <span class="spacer" />
            <button class="sm danger" @click="stop(row.name)">
              この記録を消す
            </button>
          </div>
        </div>
      </details>
    </section>
  </div>
</template>

<script setup lang="ts">
import { kubectl, kubectlJson } from "~/utils/kernel";
import {
  fmtJst,
  fmtClock,
  fmtDuration,
  parseUtc,
  windowLabel,
} from "~/utils/format";

interface Build {
  name: string;
  target: string;
  job: string;
  sha7: string;
  sha: string;
  repo: string;
  branch: string;
  image: string;
  trigger: string;
  kind: string;
  state: "実行中" | "成功" | "失敗" | "待機";
  start: Date | null;
  end: Date | null;
}

interface Target {
  name: string;
  repo: string;
  branch?: string;
  path?: string;
  spec?: string;
  enabled?: boolean;
}

const CLUSTER = "rag";
const NS = "builder";
const WINDOWS = [30, 60, 120, 360, 720, 1440];

const me = await useMe();
const canWrite = computed(() => !me.value.rolesKnown || me.value.isAdmin);

const builds = ref<Build[]>([]);
const targets = ref<Target[]>([]);
const dispatcher = ref<{
  schedule: string;
  suspend: boolean;
  last: Date | null;
} | null>(null);
const logs = ref<Record<string, string>>({});
const error = ref("");
const loading = ref(false);
const starting = ref(false);
const lastAt = ref<Date | null>(null);
const notice = ref<{ kind: "ok" | "ng"; text: string } | null>(null);

//  入り切りと時間幅は URL に残す。開き直しても同じ見え方になる。
const route = useRoute();
const router = useRouter();

const auto = ref(route.query.auto === "1");
const windowMin = ref(
  WINDOWS.includes(Number(route.query.win)) ? Number(route.query.win) : 120,
);
const picked = ref("");

watch([auto, windowMin], () => {
  router.replace({
    query: { ...route.query, auto: auto.value ? "1" : "0", win: String(windowMin.value) },
  });
  setTimer();
});

const pickedTarget = computed(() =>
  targets.value.find((t) => t.name === picked.value),
);

function shortWindow(minutes: number): string {
  if (minutes < 60) return `${minutes}分`;
  if (minutes % 1440 === 0) return `${minutes / 1440}日`;
  return `${minutes / 60}時間`;
}

function stateClass(state: string): string {
  if (state === "実行中") return "run";
  if (state === "成功") return "ok";
  if (state === "失敗") return "ng";
  return "idle";
}

// ------------------------------------------------------------
//  取得
// ------------------------------------------------------------

async function load(): Promise<void> {
  loading.value = true;
  const got = await kubectlJson<{ items: any[] }>(
    CLUSTER,
    ["get", "jobs", "-n", NS, "-l", "openspoke.io/ci=true"],
    40,
  );
  loading.value = false;
  lastAt.value = new Date();

  if (!got.ok || !got.data) {
    error.value = got.error;
    return;
  }
  error.value = "";

  const rows: Build[] = (got.data.items || []).map((item: any) => {
    const meta = item.metadata || {};
    const labels = meta.labels || {};
    const anns = meta.annotations || {};
    const status = item.status || {};

    let state: Build["state"] = "待機";
    if (status.succeeded) state = "成功";
    else if (status.failed) state = "失敗";
    else if (status.active) state = "実行中";

    let end = parseUtc(status.completionTime);
    if (!end && (state === "成功" || state === "失敗")) {
      for (const cond of status.conditions || []) {
        const cand = parseUtc(cond.lastTransitionTime);
        if (cand) end = cand;
      }
    }

    return {
      name: meta.name || "-",
      target: labels["openspoke.io/ci-target"] || "-",
      job: labels["openspoke.io/ci-job"] || "-",
      sha7: labels["openspoke.io/ci-sha"] || "-",
      sha: anns["openspoke.io/ci-sha"] || "",
      repo: anns["openspoke.io/ci-repo"] || "",
      branch: anns["openspoke.io/ci-branch"] || "",
      image: anns["openspoke.io/ci-image"] || "",
      trigger: labels["openspoke.io/ci-trigger"] || "auto",
      kind: labels["openspoke.io/ci-kind"] || "plain",
      state,
      start: parseUtc(status.startTime),
      end,
    };
  });

  rows.sort((a, b) => (b.start?.getTime() || 0) - (a.start?.getTime() || 0));
  builds.value = rows;

  //  先頭の 1 件は開いた状態にするので、ログも先に取っておく。
  const first = rows.find((r) => r.state === "成功" || r.state === "失敗");
  if (first && !logs.value[first.name]) loadLog(first.name, 500);
}

async function loadTargets(): Promise<void> {
  const got = await kubectlJson<{ data: Record<string, string> }>(
    CLUSTER,
    ["get", "configmap", "ci-dispatcher-config", "-n", NS],
    30,
  );
  if (!got.ok || !got.data) return;
  try {
    const parsed = JSON.parse(got.data.data?.["targets.json"] || "{}");
    targets.value = parsed.targets || [];
    if (!picked.value && targets.value.length) {
      picked.value = targets.value[0].name;
    }
  } catch {
    targets.value = [];
  }
}

async function loadDispatcher(): Promise<void> {
  const got = await kubectlJson<any>(
    CLUSTER,
    ["get", "cronjob", "ci-dispatcher", "-n", NS],
    30,
  );
  if (!got.ok || !got.data) {
    dispatcher.value = null;
    return;
  }
  dispatcher.value = {
    schedule: got.data.spec?.schedule || "-",
    suspend: Boolean(got.data.spec?.suspend),
    last: parseUtc(got.data.status?.lastScheduleTime),
  };
}

async function loadLog(name: string, tail: number): Promise<void> {
  const res = await kubectl(
    CLUSTER,
    ["logs", "-n", NS, `job/${name}`, `--tail=${tail}`],
    60,
  );
  logs.value = {
    ...logs.value,
    [name]: res.stdout || res.stderr || "(ログが残っていません)",
  };
}

function onFold(name: string, ev: Event): void {
  const el = ev.target as HTMLDetailsElement;
  if (el.open && !logs.value[name]) loadLog(name, 500);
}

// ------------------------------------------------------------
//  操作
// ------------------------------------------------------------

async function start(target: string): Promise<void> {
  if (!target) return;
  starting.value = true;
  notice.value = null;

  const stamp = new Date()
    .toISOString()
    .replace(/[-:TZ.]/g, "")
    .slice(0, 14);
  const patch = JSON.stringify({ data: { [`req-${stamp}`]: target } });

  let res = await kubectl(
    CLUSTER,
    ["patch", "configmap", "ci-requests", "-n", NS, "--type", "merge", "-p", patch],
    30,
  );
  if (!res.ok && res.stderr.includes("NotFound")) {
    await kubectl(CLUSTER, ["create", "configmap", "ci-requests", "-n", NS], 30);
    res = await kubectl(
      CLUSTER,
      ["patch", "configmap", "ci-requests", "-n", NS, "--type", "merge", "-p", patch],
      30,
    );
  }
  if (!res.ok) {
    starting.value = false;
    notice.value = { kind: "ng", text: res.stderr || "要求を書けません" };
    return;
  }

  const runName = `ci-dispatch-${stamp}`;
  const res2 = await kubectl(
    CLUSTER,
    ["create", "job", runName, "--from=cronjob/ci-dispatcher", "-n", NS],
    40,
  );
  starting.value = false;

  if (!res2.ok) {
    notice.value = { kind: "ng", text: res2.stderr || "ディスパッチャーを呼び出せません" };
    return;
  }

  notice.value = {
    kind: "ok",
    text:
      `${target} のビルドを開始しました (ディスパッチャー ${runName})。` +
      "数秒で「いま動いているビルド」に出ます。" +
      "短いものは十数秒で終わり「終わったビルド」の先頭に移ります。",
  };
  //  進んでいく様子が見えるよう自動更新を入れる
  auto.value = true;
  setTimeout(load, 3000);
}

async function stop(name: string): Promise<void> {
  const res = await kubectl(CLUSTER, ["delete", "job", name, "-n", NS], 40);
  notice.value = res.ok
    ? { kind: "ok", text: `${name} を片付けました。` }
    : { kind: "ng", text: res.stderr || "消せません" };
  await load();
}

// ------------------------------------------------------------
//  まとめ
// ------------------------------------------------------------

const now = ref(new Date());

const windowStart = computed(
  () => new Date(now.value.getTime() - windowMin.value * 60_000),
);

const inWindow = computed(() =>
  builds.value.filter(
    (b) => b.start && (b.end || now.value) >= windowStart.value,
  ),
);

const counts = computed(() => ({
  run: builds.value.filter((b) => b.state === "実行中").length,
  idle: builds.value.filter((b) => b.state === "待機").length,
  ok: inWindow.value.filter((b) => b.state === "成功").length,
  ng: inWindow.value.filter((b) => b.state === "失敗").length,
}));

const running = computed(() =>
  builds.value.filter((b) => b.state === "実行中" || b.state === "待機"),
);

const done = computed(() =>
  builds.value.filter((b) => b.state === "成功" || b.state === "失敗").slice(0, 50),
);

/*  同時にいくつ動いていたかの最大。開始で +1、終了で -1 して走らせる。 */
const peak = computed(() => {
  const events: [number, number][] = [];
  for (const b of builds.value) {
    if (!b.start) continue;
    const end = (b.end || now.value).getTime();
    if (end < windowStart.value.getTime()) continue;
    events.push([Math.max(b.start.getTime(), windowStart.value.getTime()), 1]);
    events.push([end, -1]);
  }
  events.sort((a, b) => a[0] - b[0] || b[1] - a[1]);
  let cur = 0;
  let best = 0;
  for (const [, delta] of events) {
    cur += delta;
    if (cur > best) best = cur;
  }
  return best;
});

const ticks = computed(() => {
  const total = now.value.getTime() - windowStart.value.getTime();
  return [0, 1, 2, 3, 4].map((i) =>
    fmtClock(new Date(windowStart.value.getTime() + (total * i) / 4)),
  );
});

const timeline = computed(() => {
  const from = windowStart.value.getTime();
  const total = Math.max(now.value.getTime() - from, 1);

  return inWindow.value
    .slice()
    .sort((a, b) => (a.start?.getTime() || 0) - (b.start?.getTime() || 0))
    .map((b) => {
      const start = Math.max(b.start!.getTime(), from);
      const end = Math.min((b.end || now.value).getTime(), now.value.getTime());
      const left = ((start - from) / total) * 100;
      let width = (Math.max(end - start, 0) / total) * 100;
      if (left + width > 100) width = Math.max(100 - left, 0);
      return { ...b, left, width };
    });
});

// ------------------------------------------------------------
//  見張り
// ------------------------------------------------------------

let timer: ReturnType<typeof setInterval> | null = null;

function setTimer(): void {
  if (timer) {
    clearInterval(timer);
    timer = null;
  }
  if (auto.value) {
    timer = setInterval(() => {
      now.value = new Date();
      load();
    }, 10_000);
  }
}

//  経過時間の表示だけは、取得を待たずに毎秒進める。
let tick: ReturnType<typeof setInterval> | null = null;

onMounted(async () => {
  await Promise.all([load(), loadTargets(), loadDispatcher()]);
  setTimer();
  tick = setInterval(() => {
    now.value = new Date();
  }, 1000);
});

onUnmounted(() => {
  if (timer) clearInterval(timer);
  if (tick) clearInterval(tick);
});
</script>
