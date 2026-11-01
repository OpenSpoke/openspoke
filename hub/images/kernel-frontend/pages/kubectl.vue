<template>
  <div>
    <div class="page-head">
      <div>
        <h1>Kubectl</h1>
        <div class="sub">各クラスターへコマンドを投げる</div>
      </div>
      <button v-if="canWrite" class="quiet" :disabled="loadingConfigs" @click="loadConfigs">
        {{ loadingConfigs ? "取得中" : "再読み込み" }}
      </button>
    </div>

    <div v-if="!canWrite" class="note info">
      読み取りのコマンドだけ実行できます。変更する操作は管理者のみです。
    </div>

    <div v-if="notice" class="note" :class="notice.kind">{{ notice.text }}</div>

    <section class="section">
      <div class="card">
        <div class="row">
          <div class="field" style="flex: 1; min-width: 200px">
            <label>クラスター</label>
            <select v-model="cluster">
              <option v-for="c in clusters" :key="c" :value="c">{{ c }}</option>
            </select>
          </div>
          <div class="field" style="flex: 1; min-width: 160px">
            <label>ネームスペース</label>
            <input v-model="ns" placeholder="省略可" />
          </div>
          <div class="field" style="width: 120px">
            <label>制限時間 (秒)</label>
            <input v-model.number="timeout" type="number" min="5" max="600" />
          </div>
        </div>

        <div class="field">
          <label>コマンド (kubectl より後ろを書く)</label>
          <input
            v-model="argLine"
            class="mono"
            placeholder="get pods -o wide"
            @keyup.enter="run"
          />
        </div>

        <div class="row" style="margin-top: 12px">
          <button class="primary" :disabled="running || !argLine.trim()" @click="run">
            {{ running ? "実行中" : "実行" }}
          </button>
          <span v-if="isMutating" class="tag ng">変更をともなう操作</span>
          <span class="spacer" />
          <span class="muted small">よく使うもの:</span>
          <button
            v-for="p in PRESETS"
            :key="p.label"
            class="sm quiet"
            @click="argLine = p.args"
          >
            {{ p.label }}
          </button>
        </div>

        <p v-if="isMutating && canWrite" class="section-note" style="margin: 12px 0 0">
          Fleet が管理しているものを直接書き換えると、GitOps と食い違います。
          原則として GitHub 経由で直してください。
        </p>
      </div>
    </section>

    <section v-if="result" class="section">
      <h2>
        結果
        <span class="tag" :class="result.ok ? 'ok' : 'ng'" style="margin-left: 8px">
          {{ result.ok ? "成功" : "失敗" }}
        </span>
      </h2>
      <pre class="log" style="max-height: 560px">{{ result.text || "(出力なし)" }}</pre>
    </section>

    <section class="section">
      <h2>履歴</h2>
      <div v-if="history.length === 0" class="card muted">
        まだ実行していません。
      </div>
      <div v-for="(h, i) in history" :key="i" class="card">
        <div class="row">
          <span class="tag" :class="h.ok ? 'ok' : 'ng'">{{ h.ok ? "成功" : "失敗" }}</span>
          <span class="mono small">{{ h.cluster }}</span>
          <span class="mono small">kubectl {{ h.args }}</span>
          <span class="spacer" />
          <span class="muted small">{{ fmtJst(h.at) }}</span>
          <button class="sm quiet" @click="argLine = h.args; cluster = h.cluster">
            もう一度
          </button>
        </div>
      </div>
    </section>

    <!--  ここから下は kubeconfig の置き場。
          kubectl はクラスターごとの kubeconfig が入っていないと動かないので、
          実行する画面と同じ場所に置いてある。 -->
    <section v-if="canWrite" class="section">
      <h2>
        登録済みの kubeconfig
        <span class="muted small" style="margin-left: 8px">{{ configs.length }} 件</span>
      </h2>

      <div v-if="configError" class="note ng">{{ configError }}</div>

      <div v-if="configs.length === 0 && !loadingConfigs" class="card muted">
        まだ登録されていません。下の欄から入れてください。
      </div>

      <div v-else class="card" style="overflow-x: auto">
        <table>
          <thead>
            <tr>
              <th>クラスター</th>
              <th>バージョン</th>
              <th>server の上書き</th>
              <th>由来</th>
              <th>覚え書き</th>
              <th>更新</th>
              <th style="width: 150px">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="c in configs" :key="c.clusterName">
              <td class="mono small">{{ c.clusterName }}</td>
              <td class="small">{{ c.k8sVersion || "-" }}</td>
              <td class="mono small">{{ c.serverUrlOverride || "(そのまま)" }}</td>
              <td class="small">{{ c.source === "auto" ? "自動取得" : "手動" }}</td>
              <td class="small">{{ c.notes || "-" }}</td>
              <td class="small">{{ c.updatedAt ? fmtJst(parseUtc(c.updatedAt)) : "-" }}</td>
              <td>
                <div class="row" style="gap: 6px">
                  <button class="sm quiet" :disabled="filling" @click="startEdit(c)">
                    差し替え
                  </button>
                  <button class="sm danger" @click="removeConfig(c)">削除</button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="row" style="margin-top: 12px">
        <button class="quiet" :disabled="fetching" @click="autofetch">
          {{ fetching ? "取得中" : "ノードから自動取得" }}
        </button>
        <span class="muted small">
          ノードに入って kubeconfig を取ってきます。手動で入れたものは上書きしません。
        </span>
      </div>
    </section>

    <section v-if="canWrite" class="section">
      <h2>kubeconfig の登録・差し替え</h2>

      <div class="card">
        <p class="section-note" style="margin-top: 0">
          同じクラスター名で入れ直すと差し替わります。中身は kernel の保管先に入り、
          Git には残りません。一覧の「差し替え」を押すと、今入っている中身が
          この欄に読み込まれます。
        </p>

        <div v-if="editing" class="note">
          <strong class="mono">{{ editing }}</strong> の kubeconfig を差し替えます。
          <button class="sm quiet" style="margin-left: 8px" @click="clearForm">
            やめる
          </button>
        </div>

        <div class="row">
          <div class="field" style="flex: 2; min-width: 220px">
            <label>クラスター名</label>
            <input v-model="formCluster" class="mono" />
          </div>
          <div class="field" style="flex: 1; min-width: 170px">
            <label>Kubernetes バージョン</label>
            <input v-model="formVersion" class="mono" />
          </div>
        </div>

        <div v-if="clusters.length" class="row" style="margin-top: 4px">
          <span class="muted small">登録済みのクラスター:</span>
          <button
            v-for="c in clusters"
            :key="c"
            class="sm quiet"
            @click="formCluster = c"
          >
            {{ c }}
          </button>
        </div>

        <div class="field">
          <label>kubeconfig 本体</label>
          <textarea v-model="formYaml" rows="12" class="mono" />
        </div>

        <div class="field">
          <label>server URL の上書き</label>
          <input v-model="formServerUrl" class="mono" placeholder="省略可" />
        </div>
        <p class="section-note" style="margin-top: 0">
          空のままなら kubeconfig に書かれた server をそのまま使います。
          ハブからその口に直接届かないクラスターのときだけ、
          トンネル経由の URL を入れてください。
        </p>

        <div class="field">
          <label>覚え書き</label>
          <input v-model="formNotes" placeholder="省略可" />
        </div>

        <div class="row" style="margin-top: 12px">
          <button
            class="primary"
            :disabled="!!saveBlocked || savingConfig"
            @click="saveConfig"
          >
            {{ savingConfig ? "登録中" : "登録する" }}
          </button>
          <span v-if="saveBlocked" class="muted small">{{ saveBlocked }}</span>
          <span v-else class="muted small">登録のあと kubectl が通るか確かめます。</span>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { kubectl, callKernel, describe } from "~/utils/kernel";
import { fmtJst, parseUtc } from "~/utils/format";

const PRESETS = [
  { label: "ノード", args: "get nodes -o wide" },
  { label: "異常な Pod", args: "get pods -A --field-selector=status.phase!=Running" },
  { label: "直近の出来事", args: "get events -A --sort-by=.lastTimestamp" },
  { label: "ネームスペース", args: "get ns" },
];

//  変更をともなう操作。実行は止めないが、はっきり見せる。
const MUTATING = [
  "apply", "delete", "edit", "patch", "replace", "scale",
  "rollout", "create", "annotate", "label", "set", "drain",
  "cordon", "uncordon", "taint", "exec",
];

//  kernel の _RE_K8S_VERSION に合わせる。外れた形は 400 で返される。
const VERSION_RE = /^v\d+\.\d+(\.\d+)?$/;

const me = await useMe();
const canWrite = computed(() => !me.value.rolesKnown || me.value.isAdmin);

const clusters = ref<string[]>(["rag"]);
const cluster = ref("rag");
const ns = ref("");
const argLine = ref("get nodes -o wide");
const timeout = ref(60);
const running = ref(false);
const result = ref<{ ok: boolean; text: string } | null>(null);
const history = ref<{ cluster: string; args: string; ok: boolean; at: Date }[]>([]);

const isMutating = computed(() => {
  const first = argLine.value.trim().split(/\s+/)[0] || "";
  return MUTATING.includes(first);
});

interface ConfigRow {
  clusterName: string;
  k8sVersion: string;
  serverUrlOverride: string;
  source: string;
  notes: string;
  updatedAt: string;
}

const configs = ref<ConfigRow[]>([]);
const loadingConfigs = ref(false);
const configError = ref("");
const savingConfig = ref(false);
const fetching = ref(false);
const filling = ref(false);
const notice = ref<{ kind: "ok" | "ng"; text: string } | null>(null);

const formCluster = ref("");
const formVersion = ref("");
const formYaml = ref("");
const formServerUrl = ref("");
const formNotes = ref("");
//  一覧から「差し替え」を押したときの対象の表示用。
const editing = ref("");

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}

/*  押せないときは理由を出す。
 *
 *  バージョンは空でも kernel は受け取るが、空のまま入れると
 *  kubectl を実行するときに「中身が足りない」と判定されて動かない。
 *  ここで必須にしておく。
 */
const saveBlocked = computed(() => {
  if (savingConfig.value) return "";
  const missing: string[] = [];
  if (!formCluster.value.trim()) missing.push("クラスター名");
  if (!formYaml.value.trim()) missing.push("kubeconfig 本体");
  if (!formVersion.value.trim()) missing.push("Kubernetes バージョン");
  if (missing.length > 0) return `${missing.join(" と ")} を入れてください。`;
  if (!VERSION_RE.test(formVersion.value.trim())) {
    return "バージョンは v1.32 や v1.32.5 の形で入れてください。";
  }
  return "";
});

async function loadClusters(): Promise<void> {
  const reply = await callKernel<{ clusters?: any[] }>("/core/clusters");
  if (!reply.ok) return;
  const list = (reply.data?.clusters || [])
    .map((c: any) => (typeof c === "string" ? c : c.cluster_name || c.name))
    .filter(Boolean)
    .sort();
  if (list.length) {
    clusters.value = list;
    if (!list.includes(cluster.value)) cluster.value = list[0];
  }
}

async function run(): Promise<void> {
  const line = argLine.value.trim();
  if (!line) return;

  const args = line.split(/\s+/);
  if (ns.value.trim()) args.push("-n", ns.value.trim());

  running.value = true;
  const res = await kubectl(cluster.value, args, timeout.value);
  running.value = false;

  result.value = {
    ok: res.ok,
    text: res.stdout || res.stderr || "",
  };
  history.value.unshift({
    cluster: cluster.value,
    args: line,
    ok: res.ok,
    at: new Date(),
  });
  if (history.value.length > 20) history.value.pop();
}

async function loadConfigs(): Promise<void> {
  loadingConfigs.value = true;
  const reply = await callKernel<{ entries?: any[] }>("/core/kubeconfig", {
    timeout: 30,
  });
  loadingConfigs.value = false;

  if (!reply.ok) {
    configError.value = `一覧を取得できません: ${describe(reply.detail)}`;
    configs.value = [];
    return;
  }
  configError.value = "";
  configs.value = (reply.data?.entries || []).map((e: any) => ({
    clusterName: str(e.cluster_name),
    k8sVersion: str(e.k8s_version),
    serverUrlOverride: str(e.server_url_override),
    source: str(e.source),
    notes: str(e.notes),
    updatedAt: str(e.updated_at),
  }));
}

/*  一覧の行から差し替えを始める。
 *  一覧には本体が入っていないので、ここだけ取り直す。
 */
async function startEdit(c: ConfigRow): Promise<void> {
  filling.value = true;
  const reply = await callKernel<any>(`/core/kubeconfig/${c.clusterName}`, {
    query: { include_yaml: "true" },
    timeout: 30,
  });
  filling.value = false;

  if (!reply.ok) {
    notice.value = {
      kind: "ng",
      text: `${c.clusterName} の中身を取れません: ${describe(reply.detail)}`,
    };
    return;
  }
  const src = reply.data || {};
  formCluster.value = c.clusterName;
  formVersion.value = str(src.k8s_version) || c.k8sVersion;
  formYaml.value = str(src.kubeconfig_yaml);
  formServerUrl.value = str(src.server_url_override);
  formNotes.value = str(src.notes);
  editing.value = c.clusterName;
  notice.value = null;
}

function clearForm(): void {
  formCluster.value = "";
  formVersion.value = "";
  formYaml.value = "";
  formServerUrl.value = "";
  formNotes.value = "";
  editing.value = "";
}

async function saveConfig(): Promise<void> {
  if (saveBlocked.value) return;
  const name = formCluster.value.trim();

  savingConfig.value = true;
  const reply = await callKernel("/core/kubeconfig", {
    method: "POST",
    body: {
      cluster_name: name,
      k8s_version: formVersion.value.trim(),
      kubeconfig_yaml: formYaml.value,
      server_url_override: formServerUrl.value.trim(),
      notes: formNotes.value.trim(),
      //  画面から入れるものは必ず手動あつかい。
      //  これで自動取得に上書きされなくなる。
      source: "manual",
    },
    timeout: 30,
  });

  if (!reply.ok) {
    savingConfig.value = false;
    notice.value = { kind: "ng", text: describe(reply.detail) || "登録できません" };
    return;
  }

  /*  入れたらその場で叩いて確かめる。通らない kubeconfig を
   *  気づかずに置いたままにしないため。
   *
   *  /version を直に引く。出力が小さく、server まで届かないときは
   *  はっきり失敗で返るので、疎通の判定に使える。
   */
  const probe = await kubectl(name, ["get", "--raw", "/version"], 30);
  savingConfig.value = false;

  notice.value = probe.ok
    ? { kind: "ok", text: `${name} を登録し、kubectl も通りました。` }
    : {
        kind: "ng",
        text: `登録はできましたが kubectl が通りません: ${probe.stderr || "原因不明"}`,
      };

  clearForm();
  await loadConfigs();
}

async function removeConfig(c: ConfigRow): Promise<void> {
  //  消すと戻せない。差し替えと押し違えたときのために必ず聞く。
  const sure = confirm(
    `${c.clusterName} の kubeconfig を削除します。戻せません。\n\n` +
      "入れ替えたいだけなら、削除ではなく「差し替え」を使ってください。",
  );
  if (!sure) return;

  const reply = await callKernel(`/core/kubeconfig/${c.clusterName}`, {
    method: "DELETE",
    timeout: 30,
  });
  notice.value = reply.ok
    ? { kind: "ok", text: `${c.clusterName} の kubeconfig を削除しました。` }
    : { kind: "ng", text: describe(reply.detail) || "削除できません" };
  await loadConfigs();
}

async function autofetch(): Promise<void> {
  fetching.value = true;
  const reply = await callKernel<any>("/core/kubectl/autofetch", {
    method: "POST",
    timeout: 120,
  });
  fetching.value = false;

  notice.value = reply.ok
    ? { kind: "ok", text: "ノードからの自動取得を実行しました。" }
    : { kind: "ng", text: describe(reply.detail) || "自動取得できません" };
  await loadConfigs();
}

onMounted(async () => {
  await loadClusters();
  if (canWrite.value) await loadConfigs();
});
</script>
