<template>
  <div>
    <div class="page-head">
      <div>
        <h1>モジュール</h1>
        <div class="sub">
          運用台帳 (fleet_state の ledger)
          <template v-if="schema"> / 版 {{ schema }}</template>
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
        <div class="label">モジュール数</div>
        <div class="value">{{ modules.length }}</div>
      </div>
      <div class="metric">
        <div class="label">Core</div>
        <div class="value">{{ modules.filter((m) => m.category === "Core").length }}</div>
      </div>
      <div class="metric">
        <div class="label">Application</div>
        <div class="value">
          {{ modules.filter((m) => m.category === "Application").length }}
        </div>
      </div>
      <div class="metric">
        <div class="label">触るのに確認が必要</div>
        <div class="value">{{ modules.filter((m) => m.careful).length }}</div>
      </div>
    </div>

    <section class="section">
      <div class="card">
        <div class="row">
          <div class="field" style="flex: 1; min-width: 160px">
            <label>区分</label>
            <select v-model="fCategory">
              <option value="">すべて</option>
              <option v-for="c in categories" :key="c" :value="c">{{ c }}</option>
            </select>
          </div>
          <div class="field" style="flex: 1; min-width: 160px">
            <label>動いている場所</label>
            <select v-model="fCluster">
              <option value="">すべて</option>
              <option v-for="c in liveClusters" :key="c" :value="c">{{ c }}</option>
            </select>
          </div>
          <div class="field" style="flex: 2; min-width: 200px">
            <label>名前で絞り込み</label>
            <input v-model="fText" placeholder="モジュール名の一部" />
          </div>
        </div>
      </div>
    </section>

    <section class="section">
      <h2>一覧</h2>

      <details v-for="m in filtered" :key="m.key" class="fold">
        <summary>
          <span v-if="m.canonical" class="tag ng">本番</span>
          <span v-else-if="m.careful" class="tag run">要確認</span>
          <span v-else class="tag plain">{{ m.category }}</span>
          <strong>{{ m.name }}</strong>
          <span class="muted small">{{ m.live || "-" }}</span>
          <span class="spacer" />
          <span class="muted small">{{ m.role || "" }}</span>
        </summary>

        <div class="body">
          <table>
            <tbody>
              <tr v-if="m.role">
                <th style="width: 180px">役割</th>
                <td>{{ m.role }}</td>
              </tr>
              <tr>
                <th>マニフェストの置き場</th>
                <td class="mono small">{{ m.manifestRepo || "-" }}</td>
              </tr>
              <tr>
                <th>パス</th>
                <td class="mono small">{{ m.path || "-" }}</td>
              </tr>
              <tr>
                <th>動いている場所</th>
                <td>{{ m.live || "-" }}</td>
              </tr>
              <tr>
                <th>配信方式</th>
                <td>{{ m.type || "-" }}</td>
              </tr>
              <tr v-if="m.dependsOn">
                <th>依存先</th>
                <td class="mono small">{{ m.dependsOn }}</td>
              </tr>
              <tr>
                <th>再起動</th>
                <td>{{ policyLabel(m.policyRestart) }}</td>
              </tr>
              <tr>
                <th>編集</th>
                <td>{{ policyLabel(m.policyEdit) }}</td>
              </tr>
              <tr>
                <th>あるべき状態</th>
                <td>{{ m.policyDesired || "-" }}</td>
              </tr>
              <tr v-if="m.ciPattern">
                <th>配信の型</th>
                <td>
                  {{ m.ciPattern }}
                  <span v-if="m.ciImage" class="mono small"> / {{ m.ciImage }}</span>
                </td>
              </tr>
              <tr v-if="m.note">
                <th>備考</th>
                <td>{{ m.note }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </details>

      <div v-if="filtered.length === 0 && !loading" class="card muted">
        該当するモジュールがありません。
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { osSearch } from "~/utils/opensearch";
import { fmtJst } from "~/utils/format";
import { isModuleAllowed, aclNote } from "~/utils/acl";

const me = await useMe();

interface Module {
  key: string;
  category: string;
  name: string;
  role: string;
  manifestRepo: string;
  path: string;
  live: string;
  type: string;
  dependsOn: string;
  note: string;
  canonical: boolean;
  careful: boolean;
  policyRestart: string;
  policyEdit: string;
  policyDesired: string;
  ciPattern: string;
  ciImage: string;
}

const modules = ref<Module[]>([]);
const schema = ref("");
const error = ref("");
const loading = ref(false);
const lastAt = ref<Date | null>(null);

const fCategory = ref("");
const fCluster = ref("");
const fText = ref("");

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}

async function load(): Promise<void> {
  loading.value = true;
  const got = await osSearch<any>(
    "fleet_state",
    { size: 1, query: { ids: { values: ["ledger"] } } },
    40,
  );
  loading.value = false;
  lastAt.value = new Date();

  if (!got.ok) {
    error.value = `台帳を取得できません: ${got.error}`;
    return;
  }
  if (got.hits.length === 0) {
    error.value = "台帳 (fleet_state の ledger) が見つかりません。";
    return;
  }
  error.value = "";

  const ledger = got.hits[0] as any;
  schema.value = str(ledger.schema_version || ledger.version);

  const out: Module[] = [];
  const cats = ledger.categories || {};

  for (const [catName, cat] of Object.entries<any>(cats)) {
    for (const [modName, modRaw] of Object.entries<any>(cat || {})) {
      //  見ている人に許されていないモジュールは飛ばす。
      if (!isModuleAllowed(me.value, `${catName}/${modName}`)) continue;

      const mod = modRaw || {};
      const policy = mod.policy || {};
      const ci = mod.ci || {};
      const restart = str(policy.restart);
      const edit = str(policy.edit);

      out.push({
        key: `${catName}/${modName}`,
        category: catName,
        name: modName,
        role: str(mod.role),
        manifestRepo: str(mod.manifest_repo),
        path: str(mod.path),
        live: str(mod.live),
        type: str(mod.type),
        dependsOn: str(mod.depends_on),
        note: str(mod.note),
        canonical: Boolean(mod.canonical),
        careful:
          ["needs_confirmation", "do_not_edit", "do_not_restart"].includes(restart) ||
          ["needs_confirmation", "do_not_edit", "do_not_restart"].includes(edit),
        policyRestart: restart,
        policyEdit: edit,
        policyDesired: str(policy.desired_state),
        ciPattern: str(ci.pattern),
        ciImage: str(ci.image_ref),
      });
    }
  }

  modules.value = out.sort(
    (a, b) => a.category.localeCompare(b.category) || a.name.localeCompare(b.name),
  );
}

const aclMessage = computed(() => aclNote(me.value));

const categories = computed(() =>
  [...new Set(modules.value.map((m) => m.category))].sort(),
);

const liveClusters = computed(() =>
  [...new Set(modules.value.map((m) => m.live).filter(Boolean))].sort(),
);

const filtered = computed(() =>
  modules.value.filter((m) => {
    if (fCategory.value && m.category !== fCategory.value) return false;
    if (fCluster.value && m.live !== fCluster.value) return false;
    if (fText.value && !m.name.toLowerCase().includes(fText.value.toLowerCase()))
      return false;
    return true;
  }),
);

function policyLabel(v: string): string {
  return (
    {
      safe: "そのまま可",
      needs_confirmation: "確認が必要",
      do_not_restart: "再起動しない",
      do_not_edit: "編集しない",
    }[v] || v || "-"
  );
}

onMounted(load);
</script>
