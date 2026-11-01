<template>
  <div>
    <div class="page-head">
      <div>
        <h1>脆弱性</h1>
        <div class="sub">
          コンテナーイメージとモバイルアプリの検査結果
          <template v-if="lastAt"> / 取得 {{ fmtJst(lastAt) }}</template>
        </div>
      </div>
      <button class="quiet" :disabled="loading" @click="load">
        {{ loading ? "取得中" : "再読み込み" }}
      </button>
    </div>

    <div v-if="error" class="note ng">{{ error }}</div>

    <section class="section">
      <div class="segmented">
        <button :class="{ on: tab === 'image' }" @click="tab = 'image'">
          コンテナーイメージ
        </button>
        <button :class="{ on: tab === 'mobile' }" @click="tab = 'mobile'">
          モバイルアプリ
        </button>
      </div>
    </section>

    <template v-if="tab === 'image'">
      <div class="metrics">
        <div class="metric">
          <div class="label">検査回数</div>
          <div class="value">{{ scans.length }}</div>
        </div>
        <div class="metric" v-for="s in SEVERITIES" :key="s.key">
          <div class="label">{{ s.label }} (最新)</div>
          <div class="value">{{ latestSeverity[s.key] || 0 }}</div>
        </div>
      </div>

      <section class="section">
        <h2>検査の履歴</h2>

        <div v-if="scans.length === 0 && !loading" class="card muted">
          検査の記録がありません。
        </div>

        <details
          v-for="(s, idx) in scans"
          :key="s.scanId"
          class="fold"
          :open="idx === 0"
          @toggle="onOpenScan(s.scanId, $event)"
        >
          <summary>
            <span class="tag" :class="s.critical + s.high > 0 ? 'ng' : 'ok'">
              {{ s.critical + s.high > 0 ? "要対応" : "問題なし" }}
            </span>
            <strong>{{ s.target || s.scanId }}</strong>
            <span class="spacer" />
            <span class="muted small">{{ fmtJst(parseUtc(s.startedAt)) }}</span>
            <span class="muted small mono">
              致命 {{ s.critical }} / 高 {{ s.high }} / 中 {{ s.medium }}
            </span>
          </summary>

          <div class="body">
            <table>
              <tbody>
                <tr>
                  <th style="width: 150px">検査 ID</th>
                  <td class="mono small">{{ s.scanId }}</td>
                </tr>
                <tr v-if="s.target">
                  <th>対象</th>
                  <td class="mono small">{{ s.target }}</td>
                </tr>
                <tr v-if="s.module">
                  <th>モジュール</th>
                  <td>{{ s.module }}</td>
                </tr>
                <tr>
                  <th>開始</th>
                  <td>{{ fmtJst(parseUtc(s.startedAt)) }}</td>
                </tr>
                <tr v-if="s.status">
                  <th>状態</th>
                  <td>{{ s.status }}</td>
                </tr>
              </tbody>
            </table>

            <h3 style="margin: 16px 0 8px">指摘の内訳</h3>
            <div v-if="!findings[s.scanId]" class="muted small">読み込み中…</div>
            <div v-else-if="findings[s.scanId].length === 0" class="muted small">
              指摘はありません。
            </div>
            <div v-else class="card" style="overflow-x: auto">
              <table>
                <thead>
                  <tr>
                    <th>深刻度</th>
                    <th>部品</th>
                    <th>識別子</th>
                    <th>直す版</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="f in findings[s.scanId]" :key="f.id + f.pkg">
                    <td>
                      <span class="tag" :class="sevClass(f.severity)">
                        {{ sevLabel(f.severity) }}
                      </span>
                    </td>
                    <td class="mono small">{{ f.pkg }}</td>
                    <td class="mono small">{{ f.id }}</td>
                    <td class="mono small">{{ f.fixed || "-" }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </details>
      </section>
    </template>

    <template v-else>
      <section class="section">
        <h2>モバイルアプリの検査</h2>

        <div v-if="mobile.length === 0 && !loading" class="card muted">
          検査の記録がありません。
        </div>

        <details
          v-for="(e, idx) in mobile"
          :key="e.eventId"
          class="fold"
          :open="idx === 0"
          @toggle="onOpenMobile(e.eventId, $event)"
        >
          <summary>
            <span class="tag" :class="e.high > 0 ? 'ng' : 'ok'">
              {{ e.high > 0 ? "要対応" : "問題なし" }}
            </span>
            <strong>{{ e.app || e.eventId }}</strong>
            <span class="spacer" />
            <span class="muted small">{{ fmtJst(parseUtc(e.at)) }}</span>
          </summary>

          <div class="body">
            <table>
              <tbody>
                <tr>
                  <th style="width: 150px">記録 ID</th>
                  <td class="mono small">{{ e.eventId }}</td>
                </tr>
                <tr v-if="e.app">
                  <th>アプリ</th>
                  <td>{{ e.app }}</td>
                </tr>
                <tr v-if="e.platform">
                  <th>環境</th>
                  <td>{{ e.platform }}</td>
                </tr>
              </tbody>
            </table>

            <h3 style="margin: 16px 0 8px">指摘</h3>
            <div v-if="!mobileFindings[e.eventId]" class="muted small">読み込み中…</div>
            <div v-else-if="mobileFindings[e.eventId].length === 0" class="muted small">
              指摘はありません。
            </div>
            <div v-else>
              <div
                v-for="(f, i) in mobileFindings[e.eventId]"
                :key="i"
                class="card"
                style="margin-bottom: 8px"
              >
                <div class="row">
                  <span class="tag" :class="sevClass(f.severity)">
                    {{ sevLabel(f.severity) }}
                  </span>
                  <strong>{{ f.title }}</strong>
                </div>
                <p v-if="f.description" class="small muted" style="margin: 8px 0 0">
                  {{ f.description }}
                </p>
                <div v-if="f.file" class="mono small muted" style="margin-top: 6px">
                  {{ f.file }}
                </div>
              </div>
            </div>
          </div>
        </details>
      </section>
    </template>
  </div>
</template>

<script setup lang="ts">
import { osSearch } from "~/utils/opensearch";
import { fmtJst, parseUtc } from "~/utils/format";

const SEVERITIES = [
  { key: "critical", label: "致命" },
  { key: "high", label: "高" },
  { key: "medium", label: "中" },
  { key: "low", label: "低" },
];

interface Scan {
  scanId: string;
  target: string;
  module: string;
  status: string;
  startedAt: string;
  critical: number;
  high: number;
  medium: number;
  low: number;
}

interface Finding {
  severity: string;
  pkg: string;
  id: string;
  fixed: string;
}

interface MobileEvent {
  eventId: string;
  app: string;
  platform: string;
  at: string;
  high: number;
}

const tab = ref<"image" | "mobile">("image");
const scans = ref<Scan[]>([]);
const findings = ref<Record<string, Finding[]>>({});
const mobile = ref<MobileEvent[]>([]);
const mobileFindings = ref<Record<string, any[]>>({});
const error = ref("");
const loading = ref(false);
const lastAt = ref<Date | null>(null);

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}
function num(v: unknown): number {
  const n = Number(v);
  return Number.isFinite(n) ? n : 0;
}

async function load(): Promise<void> {
  loading.value = true;
  lastAt.value = new Date();

  const [scanGot, mobileGot] = await Promise.all([
    osSearch<any>(
      "vuln_scans",
      { size: 50, sort: [{ started_at: { order: "desc", unmapped_type: "date" } }] },
      40,
    ),
    osSearch<any>(
      "vuln_events",
      {
        size: 50,
        query: { term: { target_type: "mobile_app" } },
        sort: [{ created_at: { order: "desc", unmapped_type: "date" } }],
      },
      40,
    ),
  ]);

  loading.value = false;

  if (!scanGot.ok && !mobileGot.ok) {
    error.value = `検査結果を取得できません: ${scanGot.error || mobileGot.error}`;
    return;
  }
  error.value = "";

  if (scanGot.ok) {
    scans.value = scanGot.hits.map((s: any) => {
      const sum = s.summary || s.severity_counts || {};
      return {
        scanId: str(s.scan_id || s._id),
        target: str(s.target || s.image_ref || s.image),
        module: str(s.module),
        status: str(s.status),
        startedAt: str(s.started_at || s.created_at),
        critical: num(sum.CRITICAL ?? sum.critical),
        high: num(sum.HIGH ?? sum.high),
        medium: num(sum.MEDIUM ?? sum.medium),
        low: num(sum.LOW ?? sum.low),
      };
    });
  }

  if (mobileGot.ok) {
    mobile.value = mobileGot.hits.map((e: any) => {
      const sum = e.summary || e.severity_counts || {};
      return {
        eventId: str(e.event_id || e._id),
        app: str(e.app || e.app_name || e.target),
        platform: str(e.platform),
        at: str(e.created_at || e.started_at),
        high: num(sum.HIGH ?? sum.high) + num(sum.CRITICAL ?? sum.critical),
      };
    });
  }
}

const latestSeverity = computed(() => {
  const s = scans.value[0];
  if (!s) return {} as Record<string, number>;
  return { critical: s.critical, high: s.high, medium: s.medium, low: s.low };
});

async function onOpenScan(scanId: string, ev: Event): Promise<void> {
  const el = ev.target as HTMLDetailsElement;
  if (!el.open || findings.value[scanId]) return;

  const got = await osSearch<any>(
    "vuln_findings",
    {
      size: 300,
      query: { term: { scan_id: scanId } },
      sort: [{ severity: { order: "asc", unmapped_type: "keyword" } }],
    },
    40,
  );

  findings.value = {
    ...findings.value,
    [scanId]: got.ok
      ? got.hits.map((f: any) => ({
          severity: str(f.severity),
          pkg: str(f.pkg_name || f.package || f.pkg),
          id: str(f.vulnerability_id || f.cve || f.id),
          fixed: str(f.fixed_version),
        }))
      : [],
  };
}

async function onOpenMobile(eventId: string, ev: Event): Promise<void> {
  const el = ev.target as HTMLDetailsElement;
  if (!el.open || mobileFindings.value[eventId]) return;

  const got = await osSearch<any>(
    "vuln_findings_mobile",
    { size: 300, query: { term: { event_id: eventId } } },
    40,
  );

  mobileFindings.value = {
    ...mobileFindings.value,
    [eventId]: got.ok
      ? got.hits.map((f: any) => ({
          severity: str(f.severity),
          title: str(f.title || f.rule || f.check),
          description: str(f.description || f.detail),
          file: str(f.file || f.path),
        }))
      : [],
  };
}

function sevLabel(s: string): string {
  return (
    {
      CRITICAL: "致命", HIGH: "高", MEDIUM: "中", LOW: "低", UNKNOWN: "不明",
      critical: "致命", high: "高", medium: "中", low: "低",
    }[s] || s || "-"
  );
}

function sevClass(s: string): string {
  const up = (s || "").toUpperCase();
  if (up === "CRITICAL" || up === "HIGH") return "ng";
  if (up === "MEDIUM") return "run";
  return "idle";
}

onMounted(load);
</script>
