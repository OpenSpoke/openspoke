<template>
  <div>
    <table>
      <tbody>
        <tr>
          <th style="width: 160px">リポジトリー</th>
          <td class="mono small">{{ repo.repo || "-" }}</td>
        </tr>
        <tr>
          <th>ブランチ</th>
          <td>{{ repo.branch || "-" }}</td>
        </tr>
        <tr>
          <th>配信するパス</th>
          <td>
            <div v-for="p in repo.paths" :key="p" class="mono small">{{ p }}</div>
            <span v-if="repo.paths.length === 0" class="muted">-</span>
          </td>
        </tr>
        <tr>
          <th>最後に取得した版</th>
          <td class="mono small">{{ repo.commit || "(未取得)" }}</td>
        </tr>
        <tr>
          <th>取得間隔</th>
          <td>{{ repo.polling || "-" }}</td>
        </tr>
        <tr v-if="repo.summary">
          <th>配信先</th>
          <td>
            {{ repo.summary.ready ?? 0 }} / {{ repo.summary.desiredReady ?? 0 }}
            クラスターで完了
          </td>
        </tr>
      </tbody>
    </table>

    <h3 style="margin: 16px 0 8px">状態の内訳</h3>
    <table>
      <thead>
        <tr>
          <th style="width: 160px">種類</th>
          <th style="width: 60px">値</th>
          <th>内容</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="c in repo.conditions" :key="c.type">
          <td>{{ condLabel(c.type) }}</td>
          <td>
            <span class="tag" :class="condClass(c.type, c.status)">
              {{ c.status === "True" ? "はい" : "いいえ" }}
            </span>
          </td>
          <td class="small" style="word-break: break-all">{{ c.message || "-" }}</td>
        </tr>
      </tbody>
    </table>

    <div v-if="canWrite" class="row" style="margin-top: 16px">
      <button class="sm" @click="emit('sync', repo.name)">
        再取得を促す
      </button>
      <span class="muted small">
        取得が詰まったときに使います (forceSyncGeneration を 1 つ進めます)。
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Repo } from "~/pages/fleet.vue";

const props = defineProps<{ repo: Repo; canWrite: boolean }>();
const emit = defineEmits<{ sync: [name: string] }>();

//  Fleet の状態の種類を、何を指しているかが分かる言い方にする。
function condLabel(type: string): string {
  return (
    {
      Ready: "配信完了",
      GitPolling: "取得",
      Reconciling: "反映中",
      Stalled: "停止",
      Accepted: "受理",
    }[type] || type
  );
}

//  Stalled だけは「はい」が悪い状態。それ以外は「はい」が良い状態。
function condClass(type: string, status: string): string {
  const good = status === "True";
  if (type === "Stalled" || type === "Reconciling") {
    return good ? "run" : "plain";
  }
  return good ? "ok" : "ng";
}
</script>
