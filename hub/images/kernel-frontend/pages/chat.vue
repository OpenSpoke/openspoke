<template>
  <div class="chat-wrap">
    <aside class="chat-side">
      <div class="row" style="margin-bottom: 10px">
        <strong style="font-size: 13px">スレッド</strong>
        <span class="spacer" />
        <button class="quiet small" @click="createThread()">新規</button>
      </div>

      <div v-if="threads.length === 0" class="muted small">
        まだありません。
      </div>

      <div v-for="g in grouped" :key="g.id" style="margin-bottom: 12px">
        <div class="muted" style="font-size: 11px; margin-bottom: 4px">
          {{ g.name }}
        </div>
        <button
          v-for="t in g.threads"
          :key="t.id"
          class="thread-btn"
          :class="{ on: t.id === threadId }"
          @click="openThread(t.id)"
        >
          <span class="thread-title">{{ t.title || "無題" }}</span>
          <span class="muted" style="font-size: 10px">
            {{ t.updatedAt ? fmtAgo(parseUtc(t.updatedAt)) : "" }}
          </span>
        </button>
      </div>
    </aside>

    <section class="chat-main">
      <div v-if="error" class="note ng">{{ error }}</div>

      <div v-if="!threadId" class="card muted">
        左の「新規」でスレッドを作ってください。
      </div>

      <template v-else>
        <div class="card chat-head">
          <input
            v-model="title"
            placeholder="スレッド名"
            @change="saveHead"
          />
          <select v-model="model" @change="saveHead">
            <option v-for="m in MODELS" :key="m" :value="m">{{ m }}</option>
          </select>
          <select v-model="projectId" @change="saveProject">
            <option value="">未分類</option>
            <option v-for="p in projects" :key="p.id" :value="p.id">
              {{ p.name }}
            </option>
          </select>
          <button class="danger small" @click="removeThread">削除</button>
        </div>

        <div class="chat-log">
          <div v-if="messages.length === 0 && !live" class="muted small">
            まだ発言がありません。下の欄から始めてください。
          </div>

          <div
            v-for="(m, i) in messages"
            :key="i"
            class="msg"
            :class="m.role === 'user' ? 'me' : 'ai'"
          >
            <div class="msg-head">
              <span class="tag plain">{{ m.role === "user" ? "あなた" : "Claude" }}</span>
              <span class="spacer" />
              <span v-if="m.model" class="muted mono" style="font-size: 10px">
                {{ m.model }}
              </span>
              <span v-if="m.cost" class="muted mono" style="font-size: 10px">
                ${{ m.cost.toFixed(4) }}
              </span>
            </div>
            <div class="msg-body">{{ m.content }}</div>
            <div
              v-if="m.topics && m.topics.length"
              class="muted"
              style="font-size: 10px; margin-top: 6px"
            >
              参照したトピック: {{ m.topics.join(", ") }}
            </div>
          </div>

          <div v-if="live" class="msg ai">
            <div class="msg-head">
              <span class="tag run">Claude</span>
              <span class="spacer" />
              <span class="muted" style="font-size: 10px">{{ liveNote }}</span>
            </div>
            <div class="msg-body">{{ liveText || "…" }}</div>
          </div>
        </div>

        <div class="card chat-input">
          <textarea
            v-model="input"
            rows="3"
            placeholder="ここに書く (Ctrl + Enter で送信)"
            :disabled="live"
            @keydown.ctrl.enter.prevent="send"
          />
          <div class="row" style="margin-top: 8px">
            <button class="primary" :disabled="!input.trim() || live" @click="send">
              {{ live ? "返答中" : "送る" }}
            </button>
            <span class="spacer" />
            <span v-if="totalCost > 0" class="muted mono small">
              このスレッド ${{ totalCost.toFixed(4) }}
            </span>
          </div>
        </div>
      </template>
    </section>
  </div>
</template>

<script setup lang="ts">
import { callKernel, describe } from "~/utils/kernel";
import { fmtAgo, parseUtc } from "~/utils/format";

const MODELS = [
  "claude-sonnet-4-6",
  "claude-opus-4-6",
  "claude-opus-5",
  "claude-haiku-4-5",
  "gemma2:9b",
  "jev-gemma2",
];
const DEFAULT_MODEL = "claude-sonnet-4-6";

interface Thread {
  id: string;
  title: string;
  model: string;
  projectId: string;
  updatedAt: string;
}

interface Message {
  role: string;
  content: string;
  model?: string;
  cost?: number;
  topics?: string[];
}

const threads = ref<Thread[]>([]);
const projects = ref<{ id: string; name: string }[]>([]);
const messages = ref<Message[]>([]);
const threadId = ref("");
const title = ref("");
const model = ref(DEFAULT_MODEL);
const projectId = ref("");
const input = ref("");
const error = ref("");

const live = ref(false);
const liveText = ref("");
const liveNote = ref("");

const owner = ref("");

function str(v: unknown): string {
  return v === undefined || v === null ? "" : String(v);
}

const totalCost = computed(() =>
  messages.value.reduce((s, m) => s + (m.cost || 0), 0),
);

const grouped = computed(() => {
  const byProject = new Map<string, Thread[]>();
  for (const t of threads.value) {
    const key = t.projectId || "";
    const list = byProject.get(key) || [];
    list.push(t);
    byProject.set(key, list);
  }
  const out: { id: string; name: string; threads: Thread[] }[] = [];
  for (const p of projects.value) {
    const list = byProject.get(p.id);
    if (list && list.length) out.push({ id: p.id, name: p.name, threads: list });
  }
  const rest = byProject.get("");
  if (rest && rest.length) out.push({ id: "", name: "未分類", threads: rest });
  return out;
});

/*  やりとりのコストは、記録に cost_usd があればそれを使う。
 *  古い発言には入っていないので、その場合は 0 のまま出さない。
 */
function msgCost(m: any): number | undefined {
  const c = Number(m.cost_usd);
  return c > 0 ? c : undefined;
}

async function loadProjects(): Promise<void> {
  const reply = await callKernel<any>("/core/project/list", {
    method: "POST",
    body: { owner: owner.value },
    timeout: 30,
  });
  if (!reply.ok) return;
  const list = reply.data?.projects || reply.data?.items || [];
  projects.value = (Array.isArray(list) ? list : []).map((p: any) => ({
    id: str(p.project_id || p.id),
    name: str(p.name || p.title) || "(名前なし)",
  }));
}

async function loadThreads(): Promise<void> {
  const reply = await callKernel<any>("/core/chat/threads/list", {
    method: "POST",
    body: { owner: owner.value },
    timeout: 30,
  });
  if (!reply.ok) {
    error.value = `スレッド一覧を取れません: ${describe(reply.detail)}`;
    return;
  }
  error.value = "";
  const list = reply.data?.threads || reply.data?.items || [];
  threads.value = (Array.isArray(list) ? list : [])
    .map((t: any) => ({
      id: str(t.thread_id || t.id),
      title: str(t.title),
      model: str(t.model) || DEFAULT_MODEL,
      projectId: str(t.project_id),
      updatedAt: str(t.updated_at || t.created_at),
    }))
    .sort((a, b) => (b.updatedAt || "").localeCompare(a.updatedAt || ""));
}

async function openThread(id: string): Promise<void> {
  threadId.value = id;
  liveText.value = "";
  liveNote.value = "";
  const t = threads.value.find((x) => x.id === id);
  title.value = t?.title || "";
  model.value = t?.model || DEFAULT_MODEL;
  projectId.value = t?.projectId || "";

  const reply = await callKernel<any>("/core/chat/messages/list", {
    method: "POST",
    body: { owner: owner.value, thread_id: id },
    timeout: 40,
  });
  if (!reply.ok) {
    error.value = `やりとりを取れません: ${describe(reply.detail)}`;
    return;
  }
  error.value = "";
  const list = reply.data?.messages || reply.data?.items || [];
  messages.value = (Array.isArray(list) ? list : []).map((m: any) => ({
    role: str(m.role) || "assistant",
    content: str(m.content),
    model: str(m.model),
    cost: msgCost(m),
    topics: Array.isArray(m.injected_topics)
      ? m.injected_topics.map((x: any) => str(x.title || x.name || x))
      : [],
  }));
}

async function createThread(): Promise<string> {
  const reply = await callKernel<any>("/core/chat/threads/create", {
    method: "POST",
    body: { owner: owner.value, title: "", project_id: projectId.value || null },
    timeout: 30,
  });
  if (!reply.ok) {
    error.value = `スレッドを作れません: ${describe(reply.detail)}`;
    return "";
  }
  const id = str(reply.data?.thread_id || reply.data?.id);
  await loadThreads();
  if (id) await openThread(id);
  return id;
}

async function saveHead(): Promise<void> {
  if (!threadId.value) return;
  const reply = await callKernel<any>("/core/chat/threads/rename", {
    method: "POST",
    body: {
      owner: owner.value,
      thread_id: threadId.value,
      title: title.value,
      model: model.value,
    },
    timeout: 30,
  });
  if (!reply.ok) {
    error.value = `保存できません: ${describe(reply.detail)}`;
    return;
  }
  error.value = "";
  await loadThreads();
}

async function saveProject(): Promise<void> {
  if (!threadId.value) return;
  const reply = await callKernel<any>("/core/chat/threads/update_project", {
    method: "POST",
    body: {
      owner: owner.value,
      thread_id: threadId.value,
      project_id: projectId.value || null,
    },
    timeout: 30,
  });
  if (!reply.ok) {
    error.value = `プロジェクトを変えられません: ${describe(reply.detail)}`;
    return;
  }
  error.value = "";
  await loadThreads();
}

async function removeThread(): Promise<void> {
  if (!threadId.value) return;
  if (!confirm("このスレッドを削除します。戻せません。")) return;
  const reply = await callKernel<any>("/core/chat/threads/delete", {
    method: "POST",
    body: { owner: owner.value, thread_id: threadId.value },
    timeout: 30,
  });
  if (!reply.ok) {
    error.value = `削除できません: ${describe(reply.detail)}`;
    return;
  }
  error.value = "";
  threadId.value = "";
  messages.value = [];
  await loadThreads();
}

/*  送信。返答は /api/chatstream から流れてくるものを読みながら足していく。
 *  中継が SSE をそのまま素通しするので、ここでは data: 行を拾って組み立てる。
 */
async function send(): Promise<void> {
  const text = input.value.trim();
  if (!text) return;
  if (!threadId.value) {
    const id = await createThread();
    if (!id) return;
  }

  messages.value = [...messages.value, { role: "user", content: text }];
  input.value = "";
  live.value = true;
  liveText.value = "";
  liveNote.value = "考えています";

  const base = (useRuntimeConfig().app.baseURL || "/").replace(/\/$/, "");
  let finalCost: number | undefined;
  let finalModel = model.value;
  let topics: string[] = [];

  try {
    const res = await fetch(`${base}/api/chatstream`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        thread_id: threadId.value,
        content: text,
        model: model.value,
      }),
    });

    if (!res.ok || !res.body) {
      error.value = `送れません: HTTP ${res.status}`;
      live.value = false;
      return;
    }

    const reader = res.body.getReader();
    const dec = new TextDecoder();
    let buf = "";

    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true });
      const lines = buf.split("\n");
      buf = lines.pop() || "";

      for (const raw of lines) {
        const line = raw.trim();
        if (!line.startsWith("data:")) continue;
        const payload = line.slice(5).trim();
        if (!payload) continue;

        let evt: any;
        try {
          evt = JSON.parse(payload);
        } catch {
          continue;
        }

        if (evt.type === "delta") {
          liveText.value += str(evt.text);
          liveNote.value = "";
        } else if (evt.type === "start") {
          finalModel = str(evt.model) || finalModel;
          topics = Array.isArray(evt.injected_topics)
            ? evt.injected_topics.map((x: any) => str(x.title || x.name || x))
            : [];
        } else if (evt.type === "tool_use") {
          liveNote.value = `検索: ${str(evt.name)}`;
        } else if (evt.type === "tool_result") {
          liveNote.value = "";
        } else if (evt.type === "done") {
          const c = Number(evt.cost_usd);
          if (c > 0) finalCost = c;
        } else if (evt.type === "error") {
          error.value = `返答できません: ${str(evt.error)}`;
        }
      }
    }
  } catch (err: unknown) {
    error.value = `途切れました: ${err instanceof Error ? err.message : String(err)}`;
  }

  messages.value = [
    ...messages.value,
    {
      role: "assistant",
      content: liveText.value,
      model: finalModel,
      cost: finalCost,
      topics,
    },
  ];
  liveText.value = "";
  liveNote.value = "";
  live.value = false;
  await loadThreads();
}

onMounted(async () => {
  const me = await useMe();
  owner.value = str(me.value.user || me.value.email) || "anonymous";
  await Promise.all([loadProjects(), loadThreads()]);
  if (threads.value.length) await openThread(threads.value[0].id);
});
</script>

<style scoped>
.chat-wrap {
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: 16px;
  align-items: start;
}

.chat-side {
  position: sticky;
  top: 12px;
}

.thread-btn {
  display: block;
  width: 100%;
  text-align: left;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 7px;
  padding: 6px 8px;
  margin-bottom: 2px;
  color: var(--fg);
  cursor: pointer;
}

.thread-btn:hover {
  background: var(--surface-2);
}

.thread-btn.on {
  background: var(--surface-2);
  border-color: var(--accent);
}

.thread-title {
  display: block;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chat-head {
  display: grid;
  grid-template-columns: 1fr 180px 160px auto;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}

.chat-log {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.msg {
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 12px 14px;
  background: var(--surface);
}

.msg.me {
  background: var(--surface-2);
}

.msg-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.msg-body {
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.7;
}

.chat-input {
  margin-top: 12px;
  position: sticky;
  bottom: 0;
}

@media (max-width: 900px) {
  .chat-wrap {
    grid-template-columns: 1fr;
  }
  .chat-side {
    position: static;
  }
  .chat-head {
    grid-template-columns: 1fr;
  }
}
</style>
