<template>
  <div>
    <div class="page-head">
      <div>
        <h1>ナレッジベース</h1>
        <div class="sub">Memgraph に貯めた知識 (全体概要・知識・事物・分類)</div>
      </div>
      <button class="quiet" :disabled="loading" @click="load">
        {{ loading ? "取得中" : "再読み込み" }}
      </button>
    </div>

    <div v-if="error" class="note ng">{{ error }}</div>

    <div class="metrics">
      <div class="metric">
        <div class="label">知識</div>
        <div class="value">{{ counts.Topic || 0 }}</div>
      </div>
      <div class="metric">
        <div class="label">事物</div>
        <div class="value">{{ counts.Entity || 0 }}</div>
      </div>
      <div class="metric">
        <div class="label">分類</div>
        <div class="value">{{ counts.Tag || 0 }}</div>
      </div>
      <div class="metric">
        <div class="label">全体概要</div>
        <div class="value">{{ counts.Brief || 0 }}</div>
      </div>
    </div>

    <section class="section">
      <h2>
        全体図
        <span class="muted small" style="font-weight: 400">
          点は知識と分類、線は「この知識にこの分類が付いている」
        </span>
      </h2>

      <div v-if="gError" class="note ng">{{ gError }}</div>
      <div v-else-if="gLoading" class="card muted">全体図のもとを取り出しています。</div>

      <KnowledgeGraph
        v-if="gLinks.length"
        :topics="gTopics"
        :tags="gTags"
        :links="gLinks"
        @pick="onPick"
      />

      <div v-if="picked" class="card" style="margin-top: 12px">
        <div class="row" style="margin-bottom: 10px; align-items: center">
          <span class="tag plain">
            {{ picked.kind === "tag" ? "分類" : "知識" }}
          </span>
          <strong>{{ picked.label }}</strong>
          <span class="spacer" />
          <button class="quiet small" @click="clearPick">閉じる</button>
        </div>

        <div v-if="pickedBusy" class="muted small">取り出しています。</div>

        <div
          v-else-if="picked.kind === 'tag' && pickedTopics.length === 0"
          class="muted small"
        >
          この分類が付いた知識はありません。
        </div>

        <div v-else-if="picked.kind === 'tag'" class="muted small" style="margin-bottom: 8px">
          この分類が付いた知識 {{ pickedTopics.length }} 件
        </div>

        <details v-for="t in pickedTopics" :key="`p-${t.id}`" class="fold">
          <summary>
            <strong>{{ t.title || t.id }}</strong>
            <span class="spacer" />
            <span class="muted small">{{ t.updatedAt ? fmtJst(parseUtc(t.updatedAt)) : "" }}</span>
          </summary>

          <div class="body">
            <div class="mono small muted" style="margin-bottom: 8px">{{ t.id }}</div>

            <p v-if="t.summary" style="margin: 0 0 12px">{{ t.summary }}</p>

            <div v-if="t.tags.length" class="row" style="margin-bottom: 12px; gap: 4px">
              <span v-for="g in t.tags" :key="g" class="tag plain">{{ g }}</span>
            </div>

            <button v-if="!bodies[t.id]" class="sm quiet" @click="loadBody(t.id)">
              本文を読む
            </button>
            <pre v-else class="log" style="max-height: 520px">{{ bodies[t.id] }}</pre>
          </div>
        </details>
      </div>
    </section>

    <section class="section">
      <h2>知識をさがす</h2>

      <div class="card">
        <div class="row">
          <div class="field" style="flex: 3; min-width: 240px">
            <label>言葉で絞り込み (名前・要約・本文)</label>
            <input
              v-model="query"
              placeholder="例: fleet, 認証, スポーン"
              @keyup.enter="search"
            />
          </div>
          <div class="field" style="width: 120px">
            <label>件数</label>
            <input v-model.number="limit" type="number" min="10" max="300" />
          </div>
          <button class="primary" style="margin-top: 18px" :disabled="searching" @click="search">
            {{ searching ? "検索中" : "さがす" }}
          </button>
        </div>
      </div>
    </section>

    <section class="section">
      <h2>
        結果
        <span v-if="topics.length" class="muted small" style="font-weight: 400">
          {{ topics.length }} 件
        </span>
      </h2>

      <div v-if="topics.length === 0 && !searching" class="card muted">
        {{ query ? "見つかりませんでした。" : "言葉を入れて「さがす」を押してください。" }}
      </div>

      <details v-for="t in topics" :key="t.id" class="fold">
        <summary>
          <strong>{{ t.title || t.id }}</strong>
          <span class="spacer" />
          <span class="muted small">{{ t.updatedAt ? fmtJst(parseUtc(t.updatedAt)) : "" }}</span>
        </summary>

        <div class="body">
          <div class="mono small muted" style="margin-bottom: 8px">{{ t.id }}</div>

          <p v-if="t.summary" style="margin: 0 0 12px">{{ t.summary }}</p>

          <div v-if="t.tags.length" class="row" style="margin-bottom: 12px; gap: 4px">
            <span v-for="g in t.tags" :key="g" class="tag plain">{{ g }}</span>
          </div>

          <button
            v-if="!bodies[t.id]"
            class="sm quiet"
            @click="loadBody(t.id)"
          >
            本文を読む
          </button>
          <pre v-else class="log" style="max-height: 520px">{{ bodies[t.id] }}</pre>
        </div>
      </details>
    </section>
  </div>
</template>

<script setup lang="ts">
import { callKernel, describe } from "~/utils/kernel";
import { fmtJst, parseUtc } from "~/utils/format";
//  全体図の部品が渡してくる、押された点の形。
//  部品側は script setup なので export できない。同じものを置いてある。
interface PickedNode {
  kind: "topic" | "tag";
  key: string;
  label: string;
}

interface Topic {
  id: string;
  title: string;
  summary: string;
  updatedAt: string;
  tags: string[];
}

const counts = ref<Record<string, number>>({});
const topics = ref<Topic[]>([]);
const bodies = ref<Record<string, string>>({});
const error = ref("");
const loading = ref(false);
const searching = ref(false);

const query = ref("");
const limit = ref(50);

//  全体図のもと。知識と分類の点、HAS_TAG の線。
const gTopics = shallowRef<{ id: string; title: string }[]>([]);
const gTags = shallowRef<string[]>([]);
const gLinks = shallowRef<[string, string][]>([]);
const gLoading = ref(false);
const gError = ref("");

//  全体図で押された点
const picked = ref<PickedNode | null>(null);
const pickedTopics = ref<Topic[]>([]);
const pickedBusy = ref(false);

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}

/*  kernel 経由で Memgraph に問い合わせる。 */
async function graph<T = any>(
  cypher: string,
  params: Record<string, unknown> = {},
): Promise<{ ok: boolean; rows: T[]; error: string }> {
  const reply = await callKernel<any>("/core/graph/run", {
    method: "POST",
    body: { cypher, params },
    timeout: 60,
  });
  if (!reply.ok) {
    return { ok: false, rows: [], error: describe(reply.detail) };
  }
  const d = reply.data || {};
  const rows = d.rows || d.records || d.result || d.data || [];
  return { ok: true, rows: Array.isArray(rows) ? rows : [], error: "" };
}

function toTopic(r: any): Topic {
  return {
    id: str(r.id),
    title: str(r.title),
    summary: str(r.summary),
    updatedAt: str(r.updated_at),
    tags: Array.isArray(r.tags) ? r.tags.map(String) : [],
  };
}

async function load(): Promise<void> {
  loading.value = true;
  const got = await graph<any>(
    "MATCH (n) RETURN labels(n)[0] AS label, count(n) AS count",
  );
  loading.value = false;

  if (!got.ok) {
    error.value = `知識ベースに繋がりません: ${got.error}`;
    return;
  }
  error.value = "";

  const out: Record<string, number> = {};
  for (const r of got.rows) {
    const label = str(r.label ?? r[0]);
    const n = Number(r.count ?? r[1]) || 0;
    if (label) out[label] = n;
  }
  counts.value = out;
}

/*  全体図のもとを取り出す。
 *
 *  知識ベースとして扱うのは 知識 (Topic) と 分類 (Tag)。
 *  事物 (Entity) と契約書 (Contract) は別のシステムのもので、
 *  この画面の対象外 (topic knowledgebase_view_schema_kb_only_20260828)。
 *  今ある繋がりは HAS_TAG だけなので、線はこれ 1 種類になる。
 */
async function loadGraph(): Promise<void> {
  gLoading.value = true;
  const [a, b, c] = await Promise.all([
    graph<any>("MATCH (t:Topic) RETURN t.id AS id, coalesce(t.title, t.id) AS title"),
    graph<any>("MATCH (g:Tag) RETURN g.name AS name"),
    graph<any>("MATCH (t:Topic)-[:HAS_TAG]->(g:Tag) RETURN t.id AS src, g.name AS dst"),
  ]);
  gLoading.value = false;

  const bad = [a, b, c].find((r) => !r.ok);
  if (bad) {
    gError.value = `全体図を作れません: ${bad.error}`;
    return;
  }
  gError.value = "";

  gTopics.value = a.rows
    .map((r: any) => ({ id: str(r.id ?? r[0]), title: str(r.title ?? r[1]) }))
    .filter((r) => r.id);
  gTags.value = b.rows.map((r: any) => str(r.name ?? r[0])).filter(Boolean);
  gLinks.value = c.rows
    .map((r: any) => [str(r.src ?? r[0]), str(r.dst ?? r[1])] as [string, string])
    .filter((p) => p[0] && p[1]);
}

function clearPick(): void {
  picked.value = null;
  pickedTopics.value = [];
}

/*  全体図の点を押したとき。
 *  分類ならその分類が付いた知識を並べ、知識ならその 1 件を出す。
 */
async function onPick(v: PickedNode | null): Promise<void> {
  if (!v) {
    clearPick();
    return;
  }
  picked.value = v;
  pickedTopics.value = [];
  pickedBusy.value = true;

  const got =
    v.kind === "tag"
      ? await graph<any>(
          `MATCH (t:Topic)-[:HAS_TAG]->(:Tag {name: $name})
           RETURN t.id AS id, t.title AS title, t.summary AS summary,
                  t.updated_at AS updated_at, t.tags AS tags
           ORDER BY coalesce(t.updated_at, '') DESC`,
          { name: v.key },
        )
      : await graph<any>(
          `MATCH (t:Topic {id: $id})
           RETURN t.id AS id, t.title AS title, t.summary AS summary,
                  t.updated_at AS updated_at, t.tags AS tags`,
          { id: v.key },
        );

  pickedBusy.value = false;
  if (got.ok) pickedTopics.value = got.rows.map(toTopic);
}

async function search(): Promise<void> {
  searching.value = true;
  const q = query.value.trim();

  const cypher = q
    ? `MATCH (t:Topic)
       WHERE toLower(t.id) CONTAINS toLower($q)
          OR toLower(coalesce(t.title, '')) CONTAINS toLower($q)
          OR toLower(coalesce(t.summary, '')) CONTAINS toLower($q)
          OR toLower(coalesce(t.content, '')) CONTAINS toLower($q)
       RETURN t.id AS id, t.title AS title, t.summary AS summary,
              t.updated_at AS updated_at, t.tags AS tags
       ORDER BY t.updated_at DESC
       LIMIT $limit`
    : `MATCH (t:Topic)
       RETURN t.id AS id, t.title AS title, t.summary AS summary,
              t.updated_at AS updated_at, t.tags AS tags
       ORDER BY t.updated_at DESC
       LIMIT $limit`;

  const got = await graph<any>(cypher, { q, limit: limit.value });
  searching.value = false;

  if (!got.ok) {
    error.value = `さがせません: ${got.error}`;
    return;
  }
  error.value = "";

  topics.value = got.rows.map(toTopic);
}

async function loadBody(id: string): Promise<void> {
  const got = await graph<any>(
    "MATCH (t:Topic {id: $id}) RETURN t.content AS content",
    { id },
  );
  const text = got.ok ? str(got.rows[0]?.content) : "";
  bodies.value = { ...bodies.value, [id]: text || "(本文がありません)" };
}

onMounted(async () => {
  await load();
  await search();
  await loadGraph();
});
</script>
