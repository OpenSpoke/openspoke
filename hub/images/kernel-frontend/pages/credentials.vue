<template>
  <div>
    <div class="page-head">
      <div>
        <h1>認証情報</h1>
        <div class="sub">
          GitHub / GitLab / Anthropic / TypeSafe / Cloudflare の鍵を kernel の保管庫で預かる
        </div>
      </div>
      <button class="quiet" :disabled="loading" @click="load">
        {{ loading ? "取得中" : "再読み込み" }}
      </button>
    </div>

    <div v-if="notice" class="note" :class="notice.kind">{{ notice.text }}</div>
    <div v-if="error" class="note ng">{{ error }}</div>

    <div v-if="health" class="note info">
      保管庫の状態: {{ health.mode || "不明" }}
      <template v-if="health.count !== undefined"> / 登録 {{ health.count }} 件</template>
    </div>

    <section class="section">
      <div class="segmented">
        <button
          v-for="p in PROVIDERS"
          :key="p.key"
          :class="{ on: provider === p.key }"
          @click="provider = p.key"
        >
          {{ p.label }}
        </button>
      </div>
    </section>

    <section class="section">
      <h2>登録済み</h2>

      <div v-if="rows.length === 0 && !loading" class="card muted">
        この種類の鍵はまだ登録されていません。
      </div>

      <div v-else class="card" style="overflow-x: auto">
        <table>
          <thead>
            <tr>
              <th>範囲</th>
              <th>対象</th>
              <th>覚え書き</th>
              <th>最後に使われた</th>
              <th>更新</th>
              <th v-if="canWrite" style="width: 160px">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in rows" :key="r.scopeType + '/' + r.scopeId">
              <td>{{ scopeLabel(r.scopeType) }}</td>
              <td class="mono small">{{ r.scopeId }}</td>
              <td class="small">{{ r.note || "-" }}</td>
              <td class="small">{{ r.lastUsedAt ? fmtJst(parseUtc(r.lastUsedAt)) : "-" }}</td>
              <td class="small">{{ r.updatedAt ? fmtJst(parseUtc(r.updatedAt)) : "-" }}</td>
              <td v-if="canWrite">
                <div class="row" style="gap: 6px">
                  <button class="sm quiet" @click="startReplace(r)">差し替え</button>
                  <button class="sm quiet" @click="verify(r)">確認</button>
                  <button class="sm danger" @click="remove(r)">削除</button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>

    <section v-if="canWrite" class="section">
      <h2>登録・差し替え</h2>

      <div class="card">
        <p class="section-note" style="margin-top: 0">
          同じ範囲と対象に入れ直すと差し替わります。鍵の値は保管庫に入り、
          この画面にも Git にも残りません。
          既にある鍵を替えるなら、上の一覧の「差し替え」を押すと
          範囲と対象が埋まります。
        </p>

        <div v-if="replacing" class="note">
          <strong class="mono">{{ replacing }}</strong> の鍵を差し替えます。
          <button class="sm quiet" style="margin-left: 8px" @click="clearForm">
            やめる
          </button>
        </div>

        <div class="row">
          <div class="field" style="flex: 1; min-width: 150px">
            <label>範囲</label>
            <select v-model="formScopeType">
              <option value="org">組織</option>
              <option value="user">利用者</option>
            </select>
          </div>
          <div class="field" style="flex: 2; min-width: 220px">
            <label>{{ scopeIdHint }}</label>
            <input v-model="formScopeId" class="mono" />
          </div>
        </div>


        <div class="field">
          <label>鍵</label>
          <input
            v-model="formToken"
            type="password"
            class="mono"
            autocomplete="new-password"
          />
        </div>

        <div class="field">
          <label>覚え書き</label>
          <input v-model="formNote" placeholder="省略可" />
        </div>

        <div class="row" style="margin-top: 12px">
          <button
            class="primary"
            :disabled="!formScopeId.trim() || !formToken || saving"
            @click="save"
          >
            {{ saving ? "登録中" : "登録する" }}
          </button>
          <span v-if="blockedReason" class="muted small">{{ blockedReason }}</span>
          <span v-else class="muted small">登録のあと自動で確認します。</span>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { callKernel, describe } from "~/utils/kernel";
import { fmtJst, parseUtc } from "~/utils/format";

const PROVIDERS = [
  { key: "github", label: "GitHub" },
  { key: "gitlab", label: "GitLab" },
  { key: "anthropic", label: "Anthropic" },
  { key: "typesafe", label: "TypeSafe" },
  { key: "cloudflare", label: "Cloudflare" },
];

interface Row {
  provider: string;
  scopeType: string;
  scopeId: string;
  note: string;
  lastUsedAt: string;
  updatedAt: string;
}

const me = await useMe();
const canWrite = computed(() => !me.value.rolesKnown || me.value.isAdmin);

const provider = ref("github");
const rows = ref<Row[]>([]);
const health = ref<{ mode?: string; count?: number } | null>(null);
const error = ref("");
const loading = ref(false);
const saving = ref(false);
const notice = ref<{ kind: "ok" | "ng"; text: string } | null>(null);

const formScopeType = ref("org");
const formScopeId = ref("");
const formToken = ref("");
//  一覧から「差し替え」を押したときの対象の表示用。
const replacing = ref("");
const formNote = ref("");

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}

/*  項目名に入れる形を書く。
 *
 *  薄い字で例を出すのはやめた。入力済みの値と見分けがつかず、
 *  「入っているのにボタンが押せない」という誤解を生む。
 */
const scopeIdHint = computed(() => {
  if (provider.value === "gitlab") return "対象 (ホスト:組織 の形)";
  if (provider.value === "cloudflare") return "Cloudflare アカウント ID";
  return formScopeType.value === "org" ? "組織名" : "利用者 ID";
});

/*  押せないときは理由を出す。
 *  例示を入力済みと見違えて止まることがあるため。
 */
const blockedReason = computed(() => {
  if (saving.value) return "";
  const missing: string[] = [];
  if (!formScopeId.value.trim()) missing.push(scopeIdHint.value);
  if (!formToken.value) missing.push("鍵");
  if (missing.length === 0) return "";
  return `${missing.join(" と ")} を入れてください。`;
});


async function load(): Promise<void> {
  loading.value = true;
  const [listReply, healthReply] = await Promise.all([
    callKernel<any>("/core/git_credentials/list", {
      query: { provider: provider.value },
      timeout: 30,
    }),
    callKernel<any>("/core/git_credentials/health", { timeout: 20 }),
  ]);
  loading.value = false;

  if (!listReply.ok) {
    error.value = `一覧を取得できません: ${describe(listReply.detail)}`;
    rows.value = [];
  } else {
    error.value = "";
    const list =
      listReply.data?.credentials || listReply.data?.items || listReply.data || [];
    rows.value = (Array.isArray(list) ? list : [])
      .map((c: any) => ({
        provider: str(c.provider),
        scopeType: str(c.scope_type),
        scopeId: str(c.scope_id),
        note: str(c.note),
        lastUsedAt: str(c.last_used_at),
        updatedAt: str(c.updated_at),
      }))
      .filter((c) => !c.provider || c.provider === provider.value)
      .sort((a, b) => a.scopeId.localeCompare(b.scopeId));
  }

  if (healthReply.ok) {
    const h = healthReply.data || {};
    health.value = {
      mode: str(h.bootstrap_mode || h.mode),
      count: typeof h.count === "number" ? h.count : undefined,
    };
  }
}

function base(r: { scopeType: string; scopeId: string }): string {
  //  対象の名前にスラッシュが入ることがあるので、そのままパスに入れる。
  return `/core/git_credentials/${provider.value}/${r.scopeType}/${r.scopeId}`;
}

/*  一覧の行から差し替えを始める。
 *  対象を手で打ち直さずに済むようにする。
 */
function startReplace(r: Row): void {
  provider.value = r.provider || provider.value;
  formScopeType.value = r.scopeType || "org";
  formScopeId.value = r.scopeId;
  formToken.value = "";
  formNote.value = "";
  replacing.value = `${r.provider} / ${r.scopeId}`;
  notice.value = null;
}

function clearForm(): void {
  formScopeId.value = "";
  formToken.value = "";
  formNote.value = "";
  replacing.value = "";
}

async function save(): Promise<void> {
  saving.value = true;
  //  kernel が受け取るのは token / note / account_name / account_type /
  //  host / api_base_url だけ。username という項目は無いので送っても
  //  捨てられる。Fleet が使う利用者名は Secret 側にある。
  const body: Record<string, unknown> = { token: formToken.value };
  if (formNote.value.trim()) body.note = formNote.value.trim();

  const target = { scopeType: formScopeType.value, scopeId: formScopeId.value.trim() };

  //  GitLab だけ host が必須 (kernel が _validate_host で弾く)。
  //  対象が "{host}:{組織}" の形なので、その前半を使う。
  if (provider.value === "gitlab") {
    const sep = target.scopeId.indexOf(":");
    const host = sep > 0 ? target.scopeId.slice(0, sep) : "";
    if (!host) {
      saving.value = false;
      notice.value = {
        kind: "ng",
        text:
          "対象は 「ホスト:組織」 の形で入れてください " +
          "(例 gitlab.example.com:your-org)。",
      };
      return;
    }
    body.host = host;
  }
  const reply = await callKernel(base(target), {
    method: "PUT",
    body,
    timeout: 30,
  });

  if (!reply.ok) {
    saving.value = false;
    notice.value = { kind: "ng", text: describe(reply.detail) || "登録できません" };
    return;
  }

  //  入れたらすぐ確認する。通らない鍵を気づかずに置かないため。
  const v = await callKernel(`${base(target)}/verify`, { method: "POST", timeout: 40 });
  saving.value = false;

  notice.value = v.ok
    ? { kind: "ok", text: `${target.scopeId} に登録し、確認も通りました。` }
    : {
        kind: "ng",
        text: `登録はできましたが確認が通りません: ${describe(v.detail)}`,
      };

  //  うまくいったら欄を空にする。対象が残ると、次に別のものを
  //  登録するときに前の値を消す手間がかかり、押し間違いの元にもなる。
  clearForm();
  await load();
}

async function verify(r: Row): Promise<void> {
  const reply = await callKernel(`${base(r)}/verify`, { method: "POST", timeout: 40 });
  notice.value = reply.ok
    ? { kind: "ok", text: `${r.scopeId} の鍵は有効です。` }
    : { kind: "ng", text: `${r.scopeId} の鍵が通りません: ${describe(reply.detail)}` };
  await load();
}

async function remove(r: Row): Promise<void> {
  //  鍵を消すのは戻せない。差し替えと押し違えたときのために必ず聞く。
  const sure = confirm(
    `${r.provider} / ${r.scopeId} の鍵を削除します。戻せません。\n\n` +
      "鍵を入れ替えたいだけなら、削除ではなく「差し替え」を使ってください。",
  );
  if (!sure) return;

  const reply = await callKernel(base(r), { method: "DELETE", timeout: 30 });
  notice.value = reply.ok
    ? { kind: "ok", text: `${r.scopeId} の鍵を削除しました。` }
    : { kind: "ng", text: describe(reply.detail) || "削除できません" };
  await load();
}

function scopeLabel(t: string): string {
  return { org: "組織", user: "利用者" }[t] || t;
}

watch(provider, load);
onMounted(load);
</script>
