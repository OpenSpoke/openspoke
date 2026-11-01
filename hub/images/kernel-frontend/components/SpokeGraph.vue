<template>
  <div class="graph-wrap">
    <svg
      class="graph"
      :viewBox="`0 0 ${W} ${H}`"
      preserveAspectRatio="xMidYMid meet"
      role="img"
      aria-label="ハブとスポークのつながり"
    >
      <!--  つながりの線。スポークからハブへ向かう  -->
      <g class="edges">
        <template v-for="e in edges" :key="`e-${e.id}`">
          <line
            :x1="e.x1"
            :y1="e.y1"
            :x2="e.x2"
            :y2="e.y2"
            :class="['edge', e.cls, { dim: selected && selected !== e.id }]"
          />
          <polygon :points="e.arrow" :class="['arrow', e.cls, { dim: selected && selected !== e.id }]" />
        </template>
      </g>

      <!--  中央のハブ  -->
      <g class="node hub" @click="selected = ''">
        <circle :cx="cx" :cy="cy" :r="HUB_R" class="hub-face" />
        <text :x="cx" :y="cy + 4" class="hub-label">rag</text>
        <text :x="cx" :y="cy + HUB_R + 16" class="sub-label">ハブ</text>
      </g>

      <!--  スポーク  -->
      <g
        v-for="n in nodes"
        :key="n.id"
        :class="['node', 'spoke', n.cls, { on: selected === n.id, dim: selected && selected !== n.id }]"
        @click="selected = selected === n.id ? '' : n.id"
      >
        <circle v-if="n.shape === 'circle'" :cx="n.x" :cy="n.y" :r="n.r" class="face" />
        <polygon v-else :points="n.points" class="face" />
        <text :x="n.x" :y="n.y + n.r + 15" class="node-label">{{ n.id }}</text>
      </g>
    </svg>

    <div class="legend">
      <span class="tag ok">稼働中</span>
      <span class="tag ng">未接続</span>
      <span class="tag idle">停止 / 未確認</span>
      <span class="spacer" />
      <span class="muted small">丸 クラスター / 六角 ネイティブ / 菱形 アプリケーション</span>
    </div>

    <div v-if="picked" class="card detail">
      <div class="row" style="margin-bottom: 8px">
        <strong class="mono">{{ picked.name }}</strong>
        <span class="tag" :class="tagOf(picked.state)">{{ stateLabel(picked.state) }}</span>
        <span class="spacer" />
        <button class="quiet small" @click="selected = ''">閉じる</button>
      </div>
      <dl class="kv">
        <template v-if="picked.purpose">
          <dt>用途</dt>
          <dd>{{ picked.purpose }}</dd>
        </template>
        <template v-if="picked.env || picked.kind">
          <dt>種類</dt>
          <dd>{{ [picked.env, picked.kind].filter(Boolean).join(" / ") }}</dd>
        </template>
        <template v-if="picked.parent">
          <dt>載っているクラスター</dt>
          <dd class="mono">{{ picked.parent }}</dd>
        </template>
        <template v-if="picked.url">
          <dt>接続先</dt>
          <dd class="mono small">{{ picked.url }}</dd>
        </template>
        <dt>応答</dt>
        <dd>{{ picked.latency ? `${picked.latency} ms` : "-" }}</dd>
        <dt>最終確認</dt>
        <dd>{{ picked.checkedAt ? fmtJst(parseUtc(picked.checkedAt)) : "未確認" }}</dd>
        <template v-if="picked.channels">
          <dt>データチャンネル</dt>
          <dd class="mono small">{{ picked.channels }}</dd>
        </template>
        <template v-if="picked.detail">
          <dt>備考</dt>
          <dd class="small">{{ picked.detail }}</dd>
        </template>
      </dl>
    </div>
  </div>
</template>

<script setup lang="ts">
import { fmtJst, parseUtc } from "~/utils/format";

export interface SpokeNode {
  name: string;
  state: string;
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

const props = defineProps<{ rows: SpokeNode[] }>();

//  描く枠。横幅は親に合わせて伸びるので、ここは比率だけを決める。
const W = 900;
const H = 620;
const cx = W / 2;
const cy = H / 2;
const HUB_R = 38;
//  クラスターを並べる輪の半径
const RING = 218;
//  アプリケーションスポークは、載っているクラスターのそばに置く
const ORBIT = 62;

const R_CLUSTER = 26;
const R_APP = 17;

const selected = ref("");

function stateLabel(state: string): string {
  if (state === "active") return "稼働中";
  if (state === "unreachable") return "未接続";
  if (state === "stopped") return "停止";
  return "未確認";
}

function tagOf(state: string): string {
  if (state === "active") return "ok";
  if (state === "unreachable") return "ng";
  return "idle";
}

function clsOf(state: string): string {
  if (state === "active") return "s-ok";
  if (state === "unreachable") return "s-ng";
  return "s-idle";
}

function hexPoints(x: number, y: number, r: number): string {
  const pts: string[] = [];
  for (let i = 0; i < 6; i += 1) {
    const a = ((-90 + 60 * i) * Math.PI) / 180;
    pts.push(`${(x + r * Math.cos(a)).toFixed(1)},${(y + r * Math.sin(a)).toFixed(1)}`);
  }
  return pts.join(" ");
}

function diamondPoints(x: number, y: number, r: number): string {
  return `${x},${y - r} ${x + r},${y} ${x},${y + r} ${x - r},${y}`;
}

interface Placed extends SpokeNode {
  id: string;
  x: number;
  y: number;
  r: number;
  shape: "circle" | "poly";
  points: string;
  cls: string;
}

const nodes = computed<Placed[]>(() => {
  const clusters = props.rows.filter((r) => r.kind !== "application");
  const apps = props.rows.filter((r) => r.kind === "application");
  const placed: Placed[] = [];
  const at = new Map<string, { x: number; y: number }>();

  //  クラスターを輪に等間隔で置く。真上から時計回り。
  clusters.forEach((r, i) => {
    const a = (-90 + (360 / Math.max(clusters.length, 1)) * i) * (Math.PI / 180);
    const x = cx + RING * Math.cos(a);
    const y = cy + RING * Math.sin(a);
    at.set(r.name, { x, y });

    const native = r.env.startsWith("native-");
    placed.push({
      ...r,
      id: r.name,
      x,
      y,
      r: R_CLUSTER,
      shape: native ? "poly" : "circle",
      points: native ? hexPoints(x, y, R_CLUSTER) : "",
      cls: clsOf(r.state),
    });
  });

  //  アプリケーションスポークは親のクラスターの外側に置く。
  //  親が分からないものは輪のさらに外に逃がす。
  const perParent = new Map<string, number>();
  apps.forEach((r, i) => {
    const base = at.get(r.parent);
    let x: number;
    let y: number;
    if (base) {
      const n = perParent.get(r.parent) || 0;
      perParent.set(r.parent, n + 1);
      //  親からハブの反対側に、少しずらして並べる
      const away = Math.atan2(base.y - cy, base.x - cx) + (n - 0.5) * 0.5;
      x = base.x + ORBIT * Math.cos(away);
      y = base.y + ORBIT * Math.sin(away);
    } else {
      const a = (-90 + (360 / Math.max(apps.length, 1)) * i) * (Math.PI / 180);
      x = cx + (RING + ORBIT) * Math.cos(a);
      y = cy + (RING + ORBIT) * Math.sin(a);
    }
    placed.push({
      ...r,
      id: r.name,
      x,
      y,
      r: R_APP,
      shape: "poly",
      points: diamondPoints(x, y, R_APP),
      cls: clsOf(r.state),
    });
  });

  return placed;
});

interface Edge {
  id: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  arrow: string;
  cls: string;
}

/*  線の端を図形の縁で止め、矢の先を作る。
 *
 *  矢印はスポークからハブへ向ける。トンネルはスポーク側から張るので、
 *  向きを逆にすると実態と食い違う (topic frontend_topology_arrow_reversal_20260807)。
 */
function makeEdge(
  id: string,
  from: { x: number; y: number; r: number },
  to: { x: number; y: number; r: number },
  cls: string,
): Edge {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const len = Math.hypot(dx, dy) || 1;
  const ux = dx / len;
  const uy = dy / len;

  const x1 = from.x + ux * (from.r + 2);
  const y1 = from.y + uy * (from.r + 2);
  const x2 = to.x - ux * (to.r + 10);
  const y2 = to.y - uy * (to.r + 10);

  //  矢の先は線の終点に置く
  const w = 5;
  const tipX = to.x - ux * (to.r + 2);
  const tipY = to.y - uy * (to.r + 2);
  const arrow = [
    `${tipX.toFixed(1)},${tipY.toFixed(1)}`,
    `${(x2 - uy * w).toFixed(1)},${(y2 + ux * w).toFixed(1)}`,
    `${(x2 + uy * w).toFixed(1)},${(y2 - ux * w).toFixed(1)}`,
  ].join(" ");

  return { id, x1, y1, x2, y2, arrow, cls };
}

const edges = computed<Edge[]>(() => {
  const byName = new Map(nodes.value.map((n) => [n.id, n]));
  const out: Edge[] = [];

  for (const n of nodes.value) {
    if (n.kind === "application") {
      //  アプリケーションスポークは、載っているクラスターにつなぐ
      const parent = byName.get(n.parent);
      if (parent) {
        out.push(makeEdge(n.id, n, parent, `${n.cls} soft`));
        continue;
      }
    }
    out.push(makeEdge(n.id, n, { x: cx, y: cy, r: HUB_R }, n.cls));
  }
  return out;
});

const picked = computed<SpokeNode | null>(
  () => props.rows.find((r) => r.name === selected.value) || null,
);
</script>

<style scoped>
.graph-wrap {
  margin-bottom: 16px;
}

.graph {
  display: block;
  width: 100%;
  height: auto;
  max-height: 560px;
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 12px;
}

.edge {
  stroke: var(--line);
  stroke-width: 1.5;
}

.arrow {
  fill: var(--line);
}

.edge.s-ok,
.arrow.s-ok {
  stroke: var(--ok);
  fill: var(--ok);
  opacity: 0.55;
}

.edge.s-ng,
.arrow.s-ng {
  stroke: var(--ng);
  fill: var(--ng);
  opacity: 0.65;
}

.edge.s-idle,
.arrow.s-idle {
  stroke: var(--idle);
  fill: var(--idle);
  opacity: 0.5;
}

.edge.soft {
  stroke-dasharray: 4 4;
}

.edge.dim,
.arrow.dim {
  opacity: 0.12;
}

.node {
  cursor: pointer;
}

.hub-face {
  fill: var(--accent);
  stroke: var(--accent);
  stroke-width: 2;
  opacity: 0.9;
}

.hub-label {
  fill: var(--accent-ink);
  font-size: 15px;
  font-weight: 700;
  text-anchor: middle;
}

.sub-label {
  fill: var(--ink-3);
  font-size: 11px;
  text-anchor: middle;
}

.face {
  fill: var(--surface-2);
  stroke-width: 2.5;
}

.s-ok .face {
  stroke: var(--ok);
}

.s-ng .face {
  stroke: var(--ng);
}

.s-idle .face {
  stroke: var(--idle);
}

.node-label {
  fill: var(--ink-2);
  font-size: 12px;
  text-anchor: middle;
}

.node.on .face {
  fill: var(--surface-3);
  stroke-width: 4;
}

.node.on .node-label {
  fill: var(--ink);
  font-weight: 700;
}

.node.dim {
  opacity: 0.3;
}

.node:hover .node-label {
  fill: var(--ink);
}

.legend {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.detail {
  margin-top: 10px;
}

.kv {
  display: grid;
  grid-template-columns: 160px 1fr;
  gap: 4px 12px;
  margin: 0;
}

.kv dt {
  color: var(--ink-3);
  font-size: 12px;
}

.kv dd {
  margin: 0;
  font-size: 13px;
  word-break: break-word;
}

@media (max-width: 700px) {
  .kv {
    grid-template-columns: 1fr;
    gap: 0 0;
  }
  .kv dd {
    margin-bottom: 8px;
  }
}
</style>
