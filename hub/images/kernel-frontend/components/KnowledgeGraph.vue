<template>
  <div class="graph-wrap">
    <div ref="box" class="graph-box">
      <canvas
        ref="cv"
        class="graph"
        @pointerdown="onDown"
        @pointermove="onMove"
        @pointerup="onUp"
        @pointerleave="onLeave"
        @wheel.prevent="onWheel"
      />
      <div v-if="busy" class="busy">{{ busy }}</div>
      <div
        v-if="hover"
        class="tip"
        :style="{ left: `${tipX}px`, top: `${tipY}px` }"
      >
        <span class="kind" :class="hover.kind">{{ hover.kind === "tag" ? "分類" : "知識" }}</span>
        {{ hover.label }}
      </div>
    </div>

    <div class="legend">
      <span class="key"><i class="dot topic" />知識 {{ topicCount }}</span>
      <span class="key"><i class="dot tag" />分類 {{ tagCount }}</span>
      <span class="key"><i class="bar" />線 {{ edgeCount }}</span>
      <span class="spacer" />
      <span class="muted small">
        ホイールで拡大縮小、引きずると移動、点を押すと中身が出る
      </span>
      <button class="quiet small" @click="restart">並べ直す</button>
      <button class="quiet small" @click="fit">全体を映す</button>
    </div>
  </div>
</template>

<script setup lang="ts">
/*  知識ベースの全体図。
 *
 *  点が 2800、線が 5800 ほどになるので、SpokeGraph のように
 *  1 要素 1 タグで組むと DOM が重い。ここは canvas に直接描く。
 *
 *  並べ方は Fruchterman-Reingold (斥力 + 線に沿ったバネ)。
 *  斥力の総当たりは点の数の二乗になるので、碁盤の目に振り分けて
 *  隣の升目だけを見る。これで 1 回の計算が数ミリ秒で済む。
 */

//  script setup の中では export を書けないので、押された点の形は
//  呼ぶ側 (pages/knowledgebase.vue) にも同じものを置いてある。
interface PickedNode {
  kind: "topic" | "tag";
  key: string;
  label: string;
}

interface GNode extends PickedNode {
  deg: number;
  x: number;
  y: number;
  dx: number;
  dy: number;
}

const props = defineProps<{
  topics: { id: string; title: string }[];
  tags: string[];
  links: [string, string][];
}>();

const emit = defineEmits<{ (e: "pick", v: PickedNode | null): void }>();

const TAU = Math.PI * 2;
const MAX_TICKS = 420;
const HEIGHT = 560;

const box = ref<HTMLDivElement | null>(null);
const cv = ref<HTMLCanvasElement | null>(null);

const busy = ref("");
const hover = shallowRef<GNode | null>(null);
const tipX = ref(0);
const tipY = ref(0);

const topicCount = ref(0);
const tagCount = ref(0);
const edgeCount = ref(0);

//  見る位置。ox / oy は画面の中央からのずれ (画面の点の単位)。
const view = reactive({ scale: 1, ox: 0, oy: 0 });

let ctx: CanvasRenderingContext2D | null = null;
let ro: ResizeObserver | null = null;
let raf = 0;
let dpr = 1;

let nodes: GNode[] = [];
let edges: [number, number][] = [];

let temp = 0;
let ticks = 0;
let K = 40;
let R0 = 400;

//  ---- 組み立て ----

function build(): void {
  const index = new Map<string, number>();
  const list: GNode[] = [];

  const add = (kind: "topic" | "tag", key: string, label: string): void => {
    const id = `${kind}\u0000${key}`;
    if (index.has(id)) return;
    index.set(id, list.length);
    list.push({ kind, key, label, deg: 0, x: 0, y: 0, dx: 0, dy: 0 });
  };

  for (const t of props.topics) {
    if (t.id) add("topic", t.id, t.title || t.id);
  }
  for (const g of props.tags) {
    if (g) add("tag", g, g);
  }

  const es: [number, number][] = [];
  for (const [t, g] of props.links) {
    const a = index.get(`topic\u0000${t}`);
    const b = index.get(`tag\u0000${g}`);
    if (a === undefined || b === undefined) continue;
    list[a].deg += 1;
    list[b].deg += 1;
    es.push([a, b]);
  }

  nodes = list;
  edges = es;
  topicCount.value = list.filter((n) => n.kind === "topic").length;
  tagCount.value = list.length - topicCount.value;
  edgeCount.value = es.length;

  seed();
  restart();
}

/*  初期位置。黄金角のらせんに置く。
 *  分類は線が集まるので内側、知識は外側から始めると落ち着きが早い。
 */
function seed(): void {
  const n = Math.max(nodes.length, 1);
  R0 = Math.sqrt(n) * 20;
  K = Math.sqrt((R0 * 2 * (R0 * 2)) / n);

  nodes.forEach((nd, i) => {
    const a = i * 2.399963;
    const r = (nd.kind === "tag" ? 0.5 : 1) * R0 * Math.sqrt((i + 1) / n);
    nd.x = r * Math.cos(a);
    nd.y = r * Math.sin(a);
    nd.dx = 0;
    nd.dy = 0;
  });
}

function restart(): void {
  if (!nodes.length) return;
  temp = R0 / 6;
  ticks = 0;
  if (!raf) raf = requestAnimationFrame(loop);
}

//  ---- 並べる ----

function tick(): void {
  const n = nodes;
  const m = n.length;
  const cell = K * 2;
  const grid = new Map<number, number[]>();

  //  升目に振り分ける。鍵は升目の座標を 1 つの数にまとめたもの。
  for (let i = 0; i < m; i += 1) {
    const nd = n[i];
    nd.dx = 0;
    nd.dy = 0;
    const gx = Math.floor(nd.x / cell);
    const gy = Math.floor(nd.y / cell);
    const key = (gx + 4096) * 16384 + (gy + 4096);
    const arr = grid.get(key);
    if (arr) arr.push(i);
    else grid.set(key, [i]);
  }

  //  斥力は隣の升目まで
  for (let i = 0; i < m; i += 1) {
    const a = n[i];
    const gx = Math.floor(a.x / cell);
    const gy = Math.floor(a.y / cell);
    for (let ux = gx - 1; ux <= gx + 1; ux += 1) {
      for (let uy = gy - 1; uy <= gy + 1; uy += 1) {
        const arr = grid.get((ux + 4096) * 16384 + (uy + 4096));
        if (!arr) continue;
        for (let p = 0; p < arr.length; p += 1) {
          const j = arr[p];
          if (j === i) continue;
          const b = n[j];
          let dx = a.x - b.x;
          let dy = a.y - b.y;
          let d2 = dx * dx + dy * dy;
          if (d2 < 0.01) {
            dx = Math.random() - 0.5;
            dy = Math.random() - 0.5;
            d2 = dx * dx + dy * dy + 0.01;
          }
          const d = Math.sqrt(d2);
          const f = (K * K) / d;
          a.dx += (dx / d) * f;
          a.dy += (dy / d) * f;
        }
      }
    }
  }

  //  線に沿ったバネ
  for (let e = 0; e < edges.length; e += 1) {
    const a = n[edges[e][0]];
    const b = n[edges[e][1]];
    const dx = a.x - b.x;
    const dy = a.y - b.y;
    const d = Math.sqrt(dx * dx + dy * dy) || 0.01;
    const f = (d * d) / K;
    const fx = (dx / d) * f;
    const fy = (dy / d) * f;
    a.dx -= fx;
    a.dy -= fy;
    b.dx += fx;
    b.dy += fy;
  }

  //  中央に寄せてから、1 回に動ける量を温度で抑えて進める
  for (let i = 0; i < m; i += 1) {
    const a = n[i];
    a.dx -= a.x * 0.013;
    a.dy -= a.y * 0.013;
    const d = Math.hypot(a.dx, a.dy) || 1;
    const s = Math.min(d, temp) / d;
    a.x += a.dx * s;
    a.y += a.dy * s;
  }

  temp *= 0.976;
}

function loop(): void {
  raf = 0;
  if (ticks < MAX_TICKS) {
    const start = performance.now();
    while (ticks < MAX_TICKS && performance.now() - start < 24) {
      tick();
      ticks += 1;
    }
    busy.value = `並べています ${Math.round((ticks / MAX_TICKS) * 100)} %`;
    if (ticks % 40 === 0 || ticks >= MAX_TICKS) fit();
    if (ticks >= MAX_TICKS) busy.value = "";
    draw();
    if (ticks < MAX_TICKS) raf = requestAnimationFrame(loop);
    return;
  }
  draw();
}

function redraw(): void {
  if (raf) return;
  raf = requestAnimationFrame(loop);
}

//  ---- 描く ----

function radiusOf(nd: GNode): number {
  const base = Math.min(Math.sqrt(nd.deg), 7);
  return nd.kind === "tag" ? 2 + base * 1.5 : 1.3 + base * 0.7;
}

function cssVar(name: string, fallback: string): string {
  const el = cv.value;
  if (!el) return fallback;
  const v = getComputedStyle(el).getPropertyValue(name).trim();
  return v || fallback;
}

function draw(): void {
  const c = cv.value;
  if (!c) return;
  if (!ctx) ctx = c.getContext("2d");
  if (!ctx) return;

  const w = c.width / dpr;
  const h = c.height / dpr;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);

  const colLine = cssVar("--line", "#283240");
  const colTopic = cssVar("--run", "#4c8dff");
  const colTag = cssVar("--accent", "#35c2b0");
  const colInk = cssVar("--ink", "#e6eaf0");

  ctx.save();
  ctx.translate(w / 2 + view.ox, h / 2 + view.oy);
  ctx.scale(view.scale, view.scale);

  //  線はまとめて 1 本の道にして一度で引く
  ctx.globalAlpha = 0.16;
  ctx.strokeStyle = colLine;
  ctx.lineWidth = 1 / view.scale;
  ctx.beginPath();
  for (let e = 0; e < edges.length; e += 1) {
    const a = nodes[edges[e][0]];
    const b = nodes[edges[e][1]];
    ctx.moveTo(a.x, a.y);
    ctx.lineTo(b.x, b.y);
  }
  ctx.stroke();
  ctx.globalAlpha = 1;

  //  点も種類ごとにまとめて塗る
  for (const kind of ["topic", "tag"] as const) {
    ctx.fillStyle = kind === "tag" ? colTag : colTopic;
    ctx.beginPath();
    for (let i = 0; i < nodes.length; i += 1) {
      const nd = nodes[i];
      if (nd.kind !== kind) continue;
      const r = radiusOf(nd);
      ctx.moveTo(nd.x + r, nd.y);
      ctx.arc(nd.x, nd.y, r, 0, TAU);
    }
    ctx.fill();
  }
  ctx.restore();

  //  指している点の輪は画面の座標で描く (拡大しても太さが変わらない)
  const on = hover.value;
  if (on) {
    const sx = on.x * view.scale + w / 2 + view.ox;
    const sy = on.y * view.scale + h / 2 + view.oy;
    ctx.beginPath();
    ctx.arc(sx, sy, radiusOf(on) * view.scale + 5, 0, TAU);
    ctx.strokeStyle = colInk;
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }
}

function fit(): void {
  const c = cv.value;
  if (!c || !nodes.length) return;
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const nd of nodes) {
    if (nd.x < minX) minX = nd.x;
    if (nd.x > maxX) maxX = nd.x;
    if (nd.y < minY) minY = nd.y;
    if (nd.y > maxY) maxY = nd.y;
  }
  const w = c.width / dpr;
  const h = c.height / dpr;
  const bw = Math.max(maxX - minX, 1);
  const bh = Math.max(maxY - minY, 1);
  view.scale = Math.min((w - 40) / bw, (h - 40) / bh);
  view.ox = -((minX + maxX) / 2) * view.scale;
  view.oy = -((minY + maxY) / 2) * view.scale;
  //  並べている最中は loop がすぐ描くので、ここでは頼まない
  //  (二重に頼むと次の 1 回分が余計に走る)。
  if (ticks >= MAX_TICKS) redraw();
}

//  ---- 触る ----

function at(ev: PointerEvent | WheelEvent): { mx: number; my: number } {
  const c = cv.value as HTMLCanvasElement;
  const rect = c.getBoundingClientRect();
  return { mx: ev.clientX - rect.left, my: ev.clientY - rect.top };
}

function nodeAt(mx: number, my: number): GNode | null {
  const c = cv.value;
  if (!c || !nodes.length) return null;
  const w = c.width / dpr;
  const h = c.height / dpr;
  const wx = (mx - w / 2 - view.ox) / view.scale;
  const wy = (my - h / 2 - view.oy) / view.scale;
  const lim = (14 / view.scale) ** 2;
  let best: GNode | null = null;
  let bd = Infinity;
  for (const nd of nodes) {
    const d = (nd.x - wx) ** 2 + (nd.y - wy) ** 2;
    if (d < bd) {
      bd = d;
      best = nd;
    }
  }
  return bd <= lim ? best : null;
}

let dragging = false;
let lastX = 0;
let lastY = 0;
let moved = 0;

function onDown(ev: PointerEvent): void {
  dragging = true;
  moved = 0;
  lastX = ev.clientX;
  lastY = ev.clientY;
  (ev.target as HTMLElement).setPointerCapture?.(ev.pointerId);
}

function onMove(ev: PointerEvent): void {
  const { mx, my } = at(ev);
  if (dragging) {
    const dx = ev.clientX - lastX;
    const dy = ev.clientY - lastY;
    moved += Math.abs(dx) + Math.abs(dy);
    lastX = ev.clientX;
    lastY = ev.clientY;
    view.ox += dx;
    view.oy += dy;
    redraw();
    return;
  }
  const found = nodeAt(mx, my);
  if (found !== hover.value) {
    hover.value = found;
    redraw();
  }
  if (found) {
    tipX.value = mx + 12;
    tipY.value = my + 14;
  }
}

function onUp(ev: PointerEvent): void {
  dragging = false;
  if (moved > 4) return;
  const { mx, my } = at(ev);
  const found = nodeAt(mx, my);
  hover.value = found;
  emit("pick", found ? { kind: found.kind, key: found.key, label: found.label } : null);
  redraw();
}

function onLeave(): void {
  dragging = false;
  if (hover.value) {
    hover.value = null;
    redraw();
  }
}

function onWheel(ev: WheelEvent): void {
  const c = cv.value;
  if (!c) return;
  const { mx, my } = at(ev);
  const w = c.width / dpr;
  const h = c.height / dpr;
  const wx = (mx - w / 2 - view.ox) / view.scale;
  const wy = (my - h / 2 - view.oy) / view.scale;
  const f = Math.exp(-ev.deltaY * 0.0015);
  view.scale = Math.min(Math.max(view.scale * f, 0.02), 12);
  view.ox = mx - w / 2 - wx * view.scale;
  view.oy = my - h / 2 - wy * view.scale;
  redraw();
}

//  ---- 大きさ ----

function resize(): void {
  const c = cv.value;
  const b = box.value;
  if (!c || !b) return;
  dpr = window.devicePixelRatio || 1;
  c.width = Math.max(Math.floor(b.clientWidth * dpr), 1);
  c.height = Math.floor(HEIGHT * dpr);
  ctx = c.getContext("2d");
  redraw();
}

onMounted(() => {
  resize();
  ro = new ResizeObserver(resize);
  if (box.value) ro.observe(box.value);
  if (props.links.length || props.topics.length) build();
});

onBeforeUnmount(() => {
  if (raf) cancelAnimationFrame(raf);
  raf = 0;
  ro?.disconnect();
  ro = null;
});

watch(
  () => [props.topics, props.tags, props.links],
  () => build(),
);
</script>

<style scoped>
.graph-wrap {
  margin-bottom: 4px;
}

.graph-box {
  position: relative;
  background: var(--surface);
  border: 1px solid var(--line);
  border-radius: 12px;
  overflow: hidden;
}

.graph {
  display: block;
  width: 100%;
  height: 560px;
  touch-action: none;
  cursor: crosshair;
}

.busy {
  position: absolute;
  left: 12px;
  top: 10px;
  font-size: 12px;
  color: var(--ink-3);
}

.tip {
  position: absolute;
  max-width: 360px;
  padding: 5px 8px;
  font-size: 12px;
  line-height: 1.4;
  color: var(--ink);
  background: var(--surface-2);
  border: 1px solid var(--line);
  border-radius: 6px;
  pointer-events: none;
  word-break: break-all;
}

.tip .kind {
  display: inline-block;
  margin-right: 6px;
  padding: 0 5px;
  border-radius: 4px;
  font-size: 11px;
  color: var(--accent-ink);
  background: var(--run);
}

.tip .kind.tag {
  background: var(--accent);
}

.legend {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 8px;
}

.key {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  color: var(--ink-2);
}

.dot {
  display: inline-block;
  width: 9px;
  height: 9px;
  border-radius: 50%;
}

.dot.topic {
  background: var(--run);
}

.dot.tag {
  background: var(--accent);
}

.bar {
  display: inline-block;
  width: 14px;
  height: 1px;
  background: var(--line);
}
</style>
