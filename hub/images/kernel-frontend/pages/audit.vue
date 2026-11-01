<template>
  <div>
    <div class="page-head">
      <div>
        <h1>監査</h1>
        <div class="sub">
          AI に出した依頼を外から測った記録 (監査ベース)
          <template v-if="lastAt"> / 取得 {{ fmtJst(lastAt) }}</template>
        </div>
      </div>
      <div class="row">
        <button
          v-if="canWrite"
          class="primary"
          :disabled="starting || runActive"
          @click="runAudit"
        >
          {{ starting ? "開始中" : runActive ? "監査中" : "監査の実行" }}
        </button>
        <button class="quiet" :disabled="loading" @click="load">
          {{ loading ? "取得中" : "再読み込み" }}
        </button>
      </div>
    </div>

    <div v-if="runNotice" class="note" :class="runNotice.kind">{{ runNotice.text }}</div>
    <div v-if="error" class="note ng">{{ error }}</div>

    <section v-if="run" class="section">
      <h2>起こした監査</h2>
      <div class="card">
        <div class="row">
          <span class="tag" :class="runStateClass">{{ runStateLabel }}</span>
          <strong>{{ run.alias || run.id }}</strong>
          <span class="muted mono small">{{ run.id }}</span>
        </div>
        <p class="section-note">
          測るのは spoke-example。判定と記録は監査の手続きの側で行う。
          経過はスポーンの画面でも追える。
        </p>

        <details class="fold">
          <summary>頼んだ内容</summary>
          <div class="body">
            <pre class="log" style="max-height: 360px">{{ AUDIT_PROMPT }}</pre>
          </div>
        </details>

        <template v-if="run.result">
          <h3 style="margin: 16px 0 8px">結果</h3>
          <pre class="log" style="max-height: 360px">{{ run.result }}</pre>
        </template>

        <template v-if="run.errorBody">
          <h3 style="margin: 16px 0 8px">エラー</h3>
          <pre class="log" style="max-height: 240px">{{ run.errorBody }}</pre>
        </template>
      </div>
    </section>

    <div class="metrics">
      <div class="metric">
        <div class="label">記録</div>
        <div class="value">{{ records.length }}</div>
      </div>
      <div class="metric">
        <div class="label">対象</div>
        <div class="value">{{ subjects.length }}</div>
      </div>
      <div class="metric">
        <div class="label">不合格</div>
        <div class="value">{{ countOf("不合格") }}</div>
      </div>
      <div class="metric">
        <div class="label">最後の記録</div>
        <div class="value small">
          {{ records.length ? fmtJst(parseUtc(records[0].recordedAt)) : "-" }}
        </div>
      </div>
    </div>

    <section class="section">
      <h2>対象ごと</h2>
      <div class="card">
        <table>
          <thead>
            <tr>
              <th>対象</th>
              <th>記録</th>
              <th>最後の判定</th>
              <th>最後の記録</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="s in bySubject" :key="s.subject">
              <td><strong>{{ s.subject }}</strong></td>
              <td class="mono">{{ s.count }}</td>
              <td>
                <span class="tag" :class="verdictClass(s.lastVerdict)">
                  {{ s.lastVerdict }}
                </span>
              </td>
              <td class="muted small">{{ fmtJst(parseUtc(s.lastAt)) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <section class="section">
      <h2>
        監査内容
        <span class="muted small" style="font-weight: 400">
          <template v-if="specMeta.updatedAt">
            最終更新 {{ fmtJst(parseUtc(specMeta.updatedAt)) }}
            <template v-if="specMeta.updatedBy"> / {{ specMeta.updatedBy }}</template>
          </template>
          <template v-else>まだ入っていません</template>
        </span>
      </h2>

      <div class="card">
        <div class="muted small" style="margin-bottom: 8px">
          何をどう測るかの定め。画面から直せる。CI の監査もこれを読む。
          監査されるのは AI なので、AI の道具 (opensearch_*) からは
          この索引 (audit_specs) を書けないようにしてある。
        </div>

        <textarea
          v-model="specText"
          spellcheck="false"
          style="width: 100%; min-height: 320px; font-family: monospace; font-size: 12px"
        />

        <div class="row" style="margin-top: 8px; align-items: center">
          <button class="primary" :disabled="specSaving" @click="saveSpec">
            {{ specSaving ? "保存中" : "保存" }}
          </button>
          <button class="quiet" :disabled="specSaving" @click="loadSpec">読み直す</button>
          <button v-if="!specMeta.updatedAt" class="quiet" @click="specText = DEFAULT_SPEC">
            初版を入れる
          </button>
          <span class="spacer" />
          <span v-if="specNote" class="muted small">{{ specNote }}</span>
        </div>

        <div v-if="specError" class="note ng" style="margin-top: 8px">{{ specError }}</div>
      </div>
    </section>

    <section class="section">
      <h2>記録</h2>

      <div class="card">
        <div class="row">
          <div class="field" style="flex: 2; min-width: 200px">
            <label>対象</label>
            <select v-model="pickSubject">
              <option value="">すべて</option>
              <option v-for="s in subjects" :key="s" :value="s">{{ s }}</option>
            </select>
          </div>
          <div class="field" style="flex: 1; min-width: 160px">
            <label>判定</label>
            <select v-model="pickVerdict">
              <option value="">すべて</option>
              <option v-for="v in verdicts" :key="v" :value="v">{{ v }}</option>
            </select>
          </div>
        </div>
      </div>

      <div v-if="shown.length === 0" class="card muted">
        あてはまる記録はありません。
      </div>

      <details v-for="(r, idx) in shown" :key="r.id" class="fold" :open="idx === 0">
        <summary>
          <span class="tag" :class="verdictClass(r.verdict)">{{ r.verdict }}</span>
          <strong>{{ r.subject }}</strong>
          <span class="spacer" />
          <span class="muted small">{{ fmtJst(parseUtc(r.recordedAt)) }}</span>
        </summary>

        <div class="body">
          <table>
            <tbody>
              <tr>
                <th>依頼</th>
                <td>{{ r.task || "-" }}</td>
              </tr>
              <tr>
                <th>記録者</th>
                <td>{{ r.recordedBy || "-" }}</td>
              </tr>
              <tr>
                <th>根拠</th>
                <td>{{ r.evidence || "-" }}</td>
              </tr>
            </tbody>
          </table>

          <div v-if="r.metricRows.length" style="margin-top: 12px">
            <div class="muted small" style="margin-bottom: 4px">測った値</div>
            <table>
              <thead>
                <tr>
                  <th>項目</th>
                  <th>値</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="m in r.metricRows" :key="m.key">
                  <td>{{ m.key }}</td>
                  <td class="mono">{{ m.value }}</td>
                </tr>
              </tbody>
            </table>
          </div>

          <div v-if="r.note" style="margin-top: 12px">
            <div class="muted small" style="margin-bottom: 4px">所見</div>
            <pre class="log" style="max-height: 420px">{{ r.note }}</pre>
          </div>

          <div class="mono small muted" style="margin-top: 12px">{{ r.id }}</div>
        </div>
      </details>
    </section>

    <div class="note" style="margin-top: 16px">
      監査ベースは OpenSearch の索引 audit_records。ナレッジベース (Memgraph) とは
      別の器で、書けるのは auditor ロールを持つアカウントの audit_record だけ。
      この画面は読むだけで、書き込みの口は持たない。
    </div>
  </div>
</template>

<script setup lang="ts">
import { osSearch } from "~/utils/opensearch";
import { callKernel, describe } from "~/utils/kernel";
import { fmtJst, parseUtc } from "~/utils/format";

const INDEX = "audit_records";

//  監査の手続き。ボタンは「監査してよ」と頼むだけで、測るのも判定するのも
//  記録するのもこの手続きの側。手順と測り方は引き継ぎ
//  audit_watch_handoff_20261020 の内容をそのまま写している。
//  ここを書き換えると監査の中身が変わるので、変えるときは引き継ぎ側も直す。
const AUDIT_PROMPT = `OpenSpoke の監査人として、正本化の見回りを 1 回おこなう。

0. 下ごしらえ
   ToolSearch で道具を取る。select: の形だけを使う。キーワード検索はしない:
     select:mcp__IntelligentMCP__audit_record,mcp__IntelligentMCP__audit_search,mcp__IntelligentMCP__audit_get,mcp__IntelligentMCP__cluster_nsm_sh_exec,mcp__IntelligentMCP__github_apply_str_replace_batch
   前回の値は会話の記憶ではなく audit_search から取る。subject と limit を指定する:
     audit_search(subject="shoppingapp-rust", limit=3)
     audit_search(subject="connectapp-rust", limit=3)

   探索はしない。必要なことはこの依頼文に全部書いてある。
   とくに system_brief と system_topics と search_rag は呼ばない。
   system_topics は 1 回で 50 万字を超えて返ることがあり、それだけで
   持ち分 (30 万) を使い切って打ち切られる。2026-10-20 に実際に起きた。
   文脈が要るときは system_lookup(topic="audit_watch_handoff_20261020") だけを引く。

1. spoke-example で測る (spoke-alt は 10-19 から繋がらない)
   ~/Library/Application Support/NativeSpokeMacTool/work/repos/ で
     python3 _census/census3.py your-org-your-app-1 .m,.mm
     python3 _census/census3.py your-org-your-app-2 .dart
   出力はそれぞれ 25 行ほど。そのまま読んでよい。
   ファイルの一覧を出したり、原本や report.md を丸こと読んだりはしない。

2. 見るのは 5 つ。ファイル数と report.md の % は当てにしない。
   1 実コード行が増えたか          最重視。注釈では増えない
   2 実コード 10 行未満の本数が減ったか
   3 原本の実数が増えたか
   4 原本の記録なしが減ったか
   5 HEAD が進んだか

3. audit_record に repository ごと 1 件入れる。
   subject   shoppingapp-rust / connectapp-rust
   task      薄い入れ物に中身を追記する (正本化)
   verdict   合格 / 不合格 / 変化なし / 基準値 から選ぶ
   metrics   先頭の項目に 測定時刻 を入れる (格納時刻とずれるため)
   evidence  見た commit の範囲と件数、表題の傾向
   note      数字だけでは分からないこと。手抜きがあれば中身を出す

4. your-org/reference/audit.md の両方の表に 1 行足す。
   github_apply_str_replace_batch を使う。GitOps の経路だけを通る。

5. 守ること
   削除してやりなおさない。既存に追記する。
   測り方は 2 つ以上持つ。的にされたら足す。
   作業側に自分の成果を数えさせない。報告文も外から取る。
   監査ベースは追記のみ。書き換えと削除はしない。`;

interface MetricRow {
  key: string;
  value: string;
}

interface AuditRecord {
  id: string;
  recordedAt: string;
  recordedBy: string;
  subject: string;
  task: string;
  verdict: string;
  evidence: string;
  note: string;
  metricRows: MetricRow[];
}

const records = ref<AuditRecord[]>([]);
const error = ref("");
const loading = ref(false);
const lastAt = ref<Date | null>(null);

const pickSubject = ref("");
const pickVerdict = ref("");

//  監査内容 (audit_specs の current 1 件)
const specText = ref("");
const specMeta = ref<{ updatedAt: string; updatedBy: string }>({ updatedAt: "", updatedBy: "" });
const specError = ref("");
const specNote = ref("");
const specSaving = ref(false);

//  まだ 1 件も無いときに「初版を入れる」で差し込む中身。
//  your-org/reference の audit/audit.json と同じもの。
const DEFAULT_SPEC = JSON.stringify(
  {
    版: 1,
    更新: "2026-10-20",
    題: "正本化の監査内容",
    対象: [
      {
        名前: "shoppingapp-rust",
        リポジトリー: "your-org/your-app-1",
        原本の拡張子: ".m,.mm",
        依頼: "薄い入れ物に中身を追記する (正本化)",
      },
      {
        名前: "connectapp-rust",
        リポジトリー: "your-org/your-app-2",
        原本の拡張子: ".dart",
        依頼: "薄い入れ物に中身を追記する (正本化)",
      },
    ],
    測り方: [
      {
        番号: 1,
        名前: "バイト数",
        見るもの: "原本バイトに対する Rust バイトの割合",
        単独で使わない: true,
        理由: "注釈で水増しできる",
      },
      {
        番号: 2,
        名前: "実コード行",
        見るもの: "注釈と空行を除いた行数が増えたか",
        しきい値: { 不合格: "増分が 0 以下", 要注意: "1 以上 100 未満", 合格: "100 以上" },
      },
      {
        番号: 3,
        名前: "差分と原本の照合",
        見るもの: "その回のコミットから 1 本抜き、原本に実在するか",
        しきい値: { 不合格: "原本に無いもの (妄想) が 1 件でもある" },
      },
      {
        番号: 4,
        名前: "原本から読む",
        見るもの: "原本の定義を先に全部読み、Rust 側に落ちているものが無いか",
      },
      {
        番号: 5,
        名前: "ビルドが通るか",
        見るもの: "cargo build --release が通るか。エラー数と警告数",
        しきい値: { 不合格: "エラーが 1 件でもある", 要注意: "警告が前回より増えた" },
      },
    ],
    判定: {
      合格: "測り方 2 が合格で、測り方 3 と 5 に不合格が無い",
      不合格: "測り方 2 / 3 / 5 のいずれかが不合格",
      変化なし: "コミットが 0 件",
      基準値: "最初の計測",
    },
    手抔きの印: [
      "実コード行が増えないのにコミットだけ増える",
      "注釈だけ増える",
      "原本の実数が増えずにファイル数だけ増える",
      "重複が増える",
      "割合が下がる",
      "report.md の % だけが進む",
      "コミットの表題が記す行数や「妄想 0」が実コードや原本と合わない",
      "足して引く (同じ回の中で追加と復元が打ち消し合う)",
    ],
    守ること: [
      "削除してやりなおさない。既存に追記する",
      "測り方は 2 つ以上持つ。的にされたら足す",
      "作業側に自分の成果を数えさせない。報告文も外から取る",
      "監査ベースは追記のみ。書き換えと削除の道具を作らない",
      "監査人自身の誤りも、消さずに訂正を足す",
    ],
  },
  null,
  2,
);

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}

function verdictClass(v: string): string {
  if (v === "合格") return "ok";
  if (v === "不合格") return "ng";
  return "plain";
}

function countOf(v: string): number {
  return records.value.filter((r) => r.verdict === v).length;
}

const subjects = computed(() => {
  const seen: string[] = [];
  for (const r of records.value) {
    if (r.subject && !seen.includes(r.subject)) seen.push(r.subject);
  }
  return seen.sort();
});

const verdicts = computed(() => {
  const order = ["不合格", "変化なし", "基準値", "合格"];
  const seen: string[] = [];
  for (const r of records.value) {
    if (r.verdict && !seen.includes(r.verdict)) seen.push(r.verdict);
  }
  return seen.sort((a, b) => order.indexOf(a) - order.indexOf(b));
});

const bySubject = computed(() => {
  const out: { subject: string; count: number; lastVerdict: string; lastAt: string }[] = [];
  for (const r of records.value) {
    const found = out.find((o) => o.subject === r.subject);
    if (found) {
      found.count += 1;
    } else {
      out.push({
        subject: r.subject,
        count: 1,
        lastVerdict: r.verdict,
        lastAt: r.recordedAt,
      });
    }
  }
  return out;
});

const shown = computed(() =>
  records.value.filter(
    (r) =>
      (!pickSubject.value || r.subject === pickSubject.value) &&
      (!pickVerdict.value || r.verdict === pickVerdict.value),
  ),
);

function toRecord(h: any): AuditRecord {
  const metrics = h?.metrics && typeof h.metrics === "object" ? h.metrics : {};
  const rows: MetricRow[] = Object.keys(metrics).map((k) => ({
    key: k,
    value: str(metrics[k]),
  }));
  return {
    id: str(h?._id),
    recordedAt: str(h?.recorded_at),
    recordedBy: str(h?.recorded_by),
    subject: str(h?.subject),
    task: str(h?.task),
    verdict: str(h?.verdict),
    evidence: str(h?.evidence),
    note: str(h?.note),
    metricRows: rows,
  };
}

async function load(): Promise<void> {
  loading.value = true;

  //  recorded_at で並べられない索引でも落ちないように、並べ替え無しでもう一度試す。
  let got = await osSearch<any>(INDEX, {
    size: 500,
    sort: [{ recorded_at: "desc" }],
    query: { match_all: {} },
  });
  if (!got.ok) {
    got = await osSearch<any>(INDEX, { size: 500, query: { match_all: {} } });
  }
  loading.value = false;

  if (!got.ok) {
    error.value =
      `監査ベースを読めません: ${got.error}` +
      " (記録が 1 件も無いうちは索引 audit_records が作られない)";
    return;
  }
  error.value = "";

  const rows = got.hits.map(toRecord);
  rows.sort((a, b) => (a.recordedAt < b.recordedAt ? 1 : -1));
  records.value = rows;
  lastAt.value = new Date();
}

// ------------------------------------------------------------------
//  監査の実行
//
//  この画面は監査ベースに書かない。audit_record は auditor ロールに
//  閉じてあり、画面から kernel を叩く経路にそのロールは乗らない。
//  ここで書けるようにすると、塞いだ裏口を開けることになる。
//  ボタンがするのは手続きを起こすことだけ。
// ------------------------------------------------------------------

const me = await useMe();
const canWrite = computed(() => !me.value.rolesKnown || me.value.isAdmin);

interface RunState {
  id: string;
  alias: string;
  state: string;
  result: string;
  errorBody: string;
}

const run = ref<RunState | null>(null);
const starting = ref(false);
const runNotice = ref<{ kind: "ok" | "ng"; text: string } | null>(null);
let runTimer: ReturnType<typeof setInterval> | null = null;

const RUN_LABEL: Record<string, string> = {
  queued: "待ち",
  running: "実行中",
  waiting_user: "返答待ち",
  done: "完了",
  error: "失敗",
  failed: "失敗",
  cancelled: "中止",
};

const runActive = computed(
  () =>
    !!run.value &&
    ["queued", "running", "waiting_user"].includes(run.value.state),
);

const runStateLabel = computed(() =>
  run.value ? RUN_LABEL[run.value.state] || run.value.state : "",
);

const runStateClass = computed(() => {
  const s = run.value?.state || "";
  if (s === "queued" || s === "running" || s === "waiting_user") return "run";
  if (s === "done") return "ok";
  if (s === "error" || s === "failed") return "ng";
  return "idle";
});

function stopRunTimer(): void {
  if (runTimer) {
    clearInterval(runTimer);
    runTimer = null;
  }
}

async function pollRun(): Promise<void> {
  if (!run.value) return;

  //  引数の名前は kernel に合わせて spawn_ref。
  const reply = await callKernel<any>("/core/spawn/get", {
    method: "POST",
    body: { spawn_ref: run.value.id },
    timeout: 30,
  });
  if (!reply.ok || !reply.data?.spawn) return;

  const s = reply.data.spawn;
  run.value = {
    id: run.value.id,
    alias: str(s.alias) || run.value.alias,
    state: str(s.state) || run.value.state,
    result: str(s.result),
    errorBody: str(s.error_body),
  };

  if (!runActive.value) {
    stopRunTimer();
    runNotice.value =
      run.value.state === "done"
        ? { kind: "ok", text: "監査が終わりました。記録を読み直します。" }
        : { kind: "ng", text: `監査は ${runStateLabel.value} で終わりました。` };
    await load();
  }
}

async function runAudit(): Promise<void> {
  const ok = confirm(
    "監査を実行します。spoke-example で測って監査ベースに記録します。よろしいですか。",
  );
  if (!ok) return;

  starting.value = true;
  runNotice.value = null;

  const reply = await callKernel<any>("/core/spawn/submit", {
    method: "POST",
    body: {
      prompt: AUDIT_PROMPT,
      alias: "監査の実行",
      label: "audit",
      priority: "high",
      timeout_sec: 3600,
      user_id: me.value.user || "",
    },
    timeout: 40,
  });
  starting.value = false;

  const fullId = str(reply.data?.full_id);
  if (!reply.ok || reply.data?.ok === false || !fullId) {
    const why = str(reply.data?.error) || describe(reply.detail) || "原因不明";
    runNotice.value = { kind: "ng", text: `監査を起こせません: ${why}` };
    return;
  }

  run.value = {
    id: fullId,
    alias: "監査の実行",
    state: "queued",
    result: "",
    errorBody: "",
  };
  runNotice.value = {
    kind: "ok",
    text: `監査を起こしました (${fullId})。終わると記録が増えます。`,
  };

  stopRunTimer();
  runTimer = setInterval(pollRun, 10_000);
}

async function loadSpec(): Promise<void> {
  specError.value = "";
  specNote.value = "";
  const got = await osSearch<any>("audit_specs", {
    size: 1,
    query: { ids: { values: ["current"] } },
  });
  if (!got.ok) {
    specError.value =
      `監査内容を読めません: ${got.error}` +
      " (まだ 1 件も保存していないときもこの表示になる)";
    return;
  }
  const hit = got.hits[0];
  if (!hit) {
    specMeta.value = { updatedAt: "", updatedBy: "" };
    specText.value = "";
    specNote.value = "まだ入っていません。「初版を入れる」を押してから保存してください。";
    return;
  }
  specMeta.value = {
    updatedAt: str(hit.updated_at),
    updatedBy: str(hit.updated_by),
  };
  specText.value = JSON.stringify(hit.spec ?? {}, null, 2);
}

async function saveSpec(): Promise<void> {
  specError.value = "";
  specNote.value = "";

  let parsed: unknown;
  try {
    parsed = JSON.parse(specText.value);
  } catch (err: unknown) {
    specError.value = `形が壊れています: ${err instanceof Error ? err.message : String(err)}`;
    return;
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    specError.value = "全体を 1 つの中身 ({ ... }) にしてください。";
    return;
  }

  specSaving.value = true;
  try {
    const reply = await $fetch<any>("/api/auditspec", {
      method: "POST",
      body: { spec: parsed },
    });
    if (!reply?.ok) {
      specError.value = `保存できませんでした: ${JSON.stringify(reply?.detail ?? "").slice(0, 300)}`;
      return;
    }
    specNote.value = "保存しました。";
    await loadSpec();
  } catch (err: unknown) {
    specError.value = err instanceof Error ? err.message : String(err);
  } finally {
    specSaving.value = false;
  }
}

onMounted(async () => {
  await load();
  await loadSpec();
});
onUnmounted(stopRunTimer);
</script>
