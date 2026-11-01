<template>
  <div>
    <div class="page-head">
      <div>
        <h1>プロジェクト</h1>
        <div class="sub">スポーンをまとめる単位</div>
      </div>
      <button class="quiet" :disabled="loading" @click="load">
        {{ loading ? "取得中" : "再読み込み" }}
      </button>
    </div>

    <div v-if="notice" class="note" :class="notice.kind">{{ notice.text }}</div>
    <div v-if="error" class="note ng">{{ error }}</div>

    <section v-if="canWrite" class="section">
      <details class="fold">
        <summary>新しいプロジェクトを作る</summary>
        <div class="body">
          <div class="field">
            <label>名前</label>
            <input v-model="newName" placeholder="例: ショッピングアプリの移植" />
          </div>
          <div class="field">
            <label>説明</label>
            <textarea v-model="newDesc" rows="2" placeholder="省略可" />
          </div>
          <div class="row" style="margin-top: 12px">
            <button class="primary" :disabled="!newName.trim() || creating" @click="create">
              {{ creating ? "作成中" : "作る" }}
            </button>
          </div>
        </div>
      </details>
    </section>

    <section class="section">
      <h2>一覧</h2>

      <details v-for="(p, idx) in rows" :key="p.id" class="fold" :open="idx === 0">
        <summary>
          <span class="tag" :class="p.status === 'archived' ? 'idle' : 'ok'">
            {{ p.status === "archived" ? "保管" : "進行中" }}
          </span>
          <strong>{{ p.name || p.id }}</strong>
          <span class="muted mono small">{{ p.id }}</span>
          <span class="spacer" />
          <span class="muted small">{{ fmtJst(parseUtc(p.createdAt)) }}</span>
        </summary>

        <div class="body">
          <table>
            <tbody>
              <tr v-if="p.description">
                <th style="width: 140px">説明</th>
                <td>{{ p.description }}</td>
              </tr>
              <tr>
                <th>所有者</th>
                <td>{{ p.owners.join(", ") || "-" }}</td>
              </tr>
              <tr v-if="p.members.length">
                <th>参加者</th>
                <td>{{ p.members.join(", ") }}</td>
              </tr>
              <tr>
                <th>作成</th>
                <td>{{ fmtJst(parseUtc(p.createdAt)) }}</td>
              </tr>
              <tr v-if="p.updatedAt">
                <th>更新</th>
                <td>{{ fmtJst(parseUtc(p.updatedAt)) }}</td>
              </tr>
            </tbody>
          </table>

          <div v-if="canWrite && p.status !== 'archived'" class="row" style="margin-top: 16px">
            <button class="sm" @click="archive(p.id)">保管する</button>
          </div>
        </div>
      </details>

      <div v-if="rows.length === 0 && !loading" class="card muted">
        プロジェクトがありません。
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { callKernel, describe } from "~/utils/kernel";
import { fmtJst, parseUtc } from "~/utils/format";

interface Project {
  id: string;
  name: string;
  description: string;
  status: string;
  owners: string[];
  members: string[];
  createdAt: string;
  updatedAt: string;
}

const me = await useMe();
const canWrite = computed(() => !me.value.rolesKnown || me.value.isAdmin);

const rows = ref<Project[]>([]);
const error = ref("");
const loading = ref(false);
const creating = ref(false);
const notice = ref<{ kind: "ok" | "ng"; text: string } | null>(null);

const newName = ref("");
const newDesc = ref("");

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}

function arr(v: unknown): string[] {
  return Array.isArray(v) ? v.map(String) : [];
}

async function load(): Promise<void> {
  loading.value = true;
  const reply = await callKernel<any>("/core/project/list", {
    method: "POST",
    body: {},
    timeout: 30,
  });
  loading.value = false;

  if (!reply.ok) {
    error.value = `プロジェクト一覧を取得できません: ${describe(reply.detail)}`;
    return;
  }
  error.value = "";

  const list = reply.data?.projects || reply.data?.items || reply.data || [];
  rows.value = (Array.isArray(list) ? list : [])
    .map((p: any) => ({
      id: str(p.project_id || p.id || p._id),
      name: str(p.name),
      description: str(p.description),
      status: str(p.status) || "active",
      owners: arr(p.owners),
      members: arr(p.members),
      createdAt: str(p.created_at),
      updatedAt: str(p.updated_at),
    }))
    .sort((a, b) => (b.createdAt || "").localeCompare(a.createdAt || ""));
}

async function create(): Promise<void> {
  creating.value = true;
  const reply = await callKernel("/core/project/create", {
    method: "POST",
    body: { name: newName.value.trim(), description: newDesc.value.trim() },
    timeout: 30,
  });
  creating.value = false;

  notice.value = reply.ok
    ? { kind: "ok", text: `${newName.value} を作りました。` }
    : { kind: "ng", text: describe(reply.detail) || "作れません" };
  if (reply.ok) {
    newName.value = "";
    newDesc.value = "";
  }
  await load();
}

async function archive(id: string): Promise<void> {
  const reply = await callKernel("/core/project/archive", {
    method: "POST",
    body: { project_id: id },
    timeout: 30,
  });
  notice.value = reply.ok
    ? { kind: "ok", text: `${id} を保管しました。` }
    : { kind: "ng", text: describe(reply.detail) || "保管できません" };
  await load();
}

onMounted(load);
</script>
