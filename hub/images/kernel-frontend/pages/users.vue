<template>
  <div>
    <div class="page-head">
      <div>
        <h1>利用者</h1>
        <div class="sub">Keycloak の realm {{ realm }} の利用者とロール</div>
      </div>
      <button class="quiet" :disabled="loading" @click="load">
        {{ loading ? "取得中" : "再読み込み" }}
      </button>
    </div>

    <div v-if="notice" class="note" :class="notice.kind">{{ notice.text }}</div>
    <div v-if="error" class="note ng">{{ error }}</div>

    <div class="metrics">
      <div class="metric">
        <div class="label">利用者数</div>
        <div class="value">{{ users.length }}</div>
      </div>
      <div class="metric">
        <div class="label">有効</div>
        <div class="value">{{ users.filter((u) => u.enabled).length }}</div>
      </div>
      <div class="metric">
        <div class="label">管理者</div>
        <div class="value">{{ users.filter((u) => u.roles.includes(ADMIN)).length }}</div>
      </div>
    </div>

    <section class="section">
      <div class="card">
        <div class="row">
          <div class="field" style="flex: 2; min-width: 220px">
            <label>名前やメールで絞り込み</label>
            <input v-model="fText" placeholder="一部で可" />
          </div>
          <div class="field" style="flex: 1; min-width: 160px">
            <label>ロール</label>
            <select v-model="fRole">
              <option value="">すべて</option>
              <option v-for="r in allRoles" :key="r" :value="r">{{ r }}</option>
            </select>
          </div>
          <label class="switch" style="margin-top: 18px">
            <input v-model="fEnabled" type="checkbox" />
            <span>有効な人のみ</span>
          </label>
        </div>
      </div>
    </section>

    <section class="section">
      <h2>一覧</h2>

      <details v-for="u in filtered" :key="u.id" class="fold">
        <summary>
          <span class="tag" :class="u.enabled ? 'ok' : 'idle'">
            {{ u.enabled ? "有効" : "無効" }}
          </span>
          <strong>{{ u.username }}</strong>
          <span class="muted small">{{ u.email || "" }}</span>
          <span class="spacer" />
          <span v-if="u.roles.includes(ADMIN)" class="tag ng">管理者</span>
        </summary>

        <div class="body">
          <table>
            <tbody>
              <tr>
                <th style="width: 140px">名前</th>
                <td>{{ [u.firstName, u.lastName].filter(Boolean).join(" ") || "-" }}</td>
              </tr>
              <tr>
                <th>メール</th>
                <td class="mono small">{{ u.email || "-" }}</td>
              </tr>
              <tr>
                <th>ID</th>
                <td class="mono small">{{ u.id }}</td>
              </tr>
              <tr>
                <th>作成</th>
                <td>{{ u.createdAt ? fmtJst(new Date(u.createdAt)) : "-" }}</td>
              </tr>
              <tr>
                <th>ロール</th>
                <td>
                  <span v-if="u.roles.length === 0" class="muted">なし</span>
                  <span
                    v-for="r in u.roles"
                    :key="r"
                    class="tag plain"
                    style="margin: 2px 4px 2px 0"
                  >
                    {{ r }}
                  </span>
                </td>
              </tr>
            </tbody>
          </table>

          <div v-if="canWrite" class="row" style="margin-top: 16px">
            <button
              v-if="!u.roles.includes(ADMIN)"
              class="sm"
              @click="setRole(u, ADMIN, true)"
            >
              管理者にする
            </button>
            <button v-else class="sm danger" @click="setRole(u, ADMIN, false)">
              管理者から外す
            </button>
            <span class="spacer" />
            <span class="muted small">
              無効化やパスワードの初期化は Keycloak の画面で行ってください。
            </span>
          </div>
        </div>
      </details>

      <div v-if="filtered.length === 0 && !loading" class="card muted">
        該当する利用者がいません。
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { fmtJst } from "~/utils/format";

const ADMIN = "openspoke_admin";

interface User {
  id: string;
  username: string;
  email: string;
  firstName: string;
  lastName: string;
  enabled: boolean;
  createdAt: number;
  roles: string[];
}

const me = await useMe();
const canWrite = computed(() => me.value.isAdmin);

const users = ref<User[]>([]);
const realm = ref("rag");
const error = ref("");
const loading = ref(false);
const notice = ref<{ kind: "ok" | "ng"; text: string } | null>(null);

const fText = ref("");
const fRole = ref("");
const fEnabled = ref(false);

interface KcReply<T = any> {
  ok: boolean;
  status: number;
  data?: T;
  detail?: unknown;
}

async function kc<T = any>(
  path: string,
  method = "GET",
  body?: unknown,
): Promise<KcReply<T>> {
  try {
    return await $fetch<KcReply<T>>("/api/keycloak", {
      method: "POST",
      body: { path, method, body },
    });
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return { ok: false, status: 0, detail: message };
  }
}

function detail(v: unknown): string {
  if (!v) return "";
  if (typeof v === "string") return v;
  const o = v as any;
  return o.errorMessage || o.error || JSON.stringify(v).slice(0, 200);
}

async function load(): Promise<void> {
  loading.value = true;
  const reply = await kc<any[]>("/users?max=500&briefRepresentation=false");

  if (!reply.ok) {
    loading.value = false;
    error.value = `利用者一覧を取得できません: ${detail(reply.detail)}`;
    return;
  }
  error.value = "";

  const list = Array.isArray(reply.data) ? reply.data : [];

  //  ロールは 1 人ずつ引く。多いときは時間がかかるので並行に投げる。
  const roleReplies = await Promise.all(
    list.map((u: any) => kc<any[]>(`/users/${u.id}/role-mappings/realm`)),
  );

  users.value = list
    .map((u: any, i: number) => ({
      id: String(u.id),
      username: String(u.username || ""),
      email: String(u.email || ""),
      firstName: String(u.firstName || ""),
      lastName: String(u.lastName || ""),
      enabled: Boolean(u.enabled),
      createdAt: Number(u.createdTimestamp || 0),
      roles: (roleReplies[i].ok && Array.isArray(roleReplies[i].data)
        ? roleReplies[i].data!
        : []
      )
        .map((r: any) => String(r.name))
        .sort(),
    }))
    .sort((a, b) => a.username.localeCompare(b.username));

  loading.value = false;
}

const allRoles = computed(() =>
  [...new Set(users.value.flatMap((u) => u.roles))].sort(),
);

const filtered = computed(() =>
  users.value.filter((u) => {
    if (fEnabled.value && !u.enabled) return false;
    if (fRole.value && !u.roles.includes(fRole.value)) return false;
    if (fText.value) {
      const q = fText.value.toLowerCase();
      const hay = `${u.username} ${u.email} ${u.firstName} ${u.lastName}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  }),
);

async function setRole(u: User, role: string, add: boolean): Promise<void> {
  //  ロールの実体 (id と name) を取ってから付け外しする。
  const roleReply = await kc<any>(`/roles/${role}`);
  if (!roleReply.ok) {
    notice.value = { kind: "ng", text: `ロール ${role} が見つかりません` };
    return;
  }

  const body = [{ id: roleReply.data.id, name: roleReply.data.name }];
  const reply = await kc(
    `/users/${u.id}/role-mappings/realm`,
    add ? "POST" : "DELETE",
    body,
  );

  notice.value = reply.ok
    ? {
        kind: "ok",
        text: `${u.username} を${add ? "管理者にしました" : "管理者から外しました"}。`,
      }
    : { kind: "ng", text: detail(reply.detail) || "変更できません" };

  await load();
}

onMounted(load);
</script>
