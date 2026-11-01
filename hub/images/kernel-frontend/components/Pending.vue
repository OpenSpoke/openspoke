<template>
  <div>
    <div class="page-head">
      <div>
        <h1>{{ title }}</h1>
        <div class="sub">移植中</div>
      </div>
    </div>

    <div class="card">
      <p style="margin-top: 0">
        この画面は Streamlit 版から順に移しています。まだこちらには来ていません。
      </p>
      <p class="muted small" style="margin-bottom: 0">
        いまのところは
        <a :href="oldUrl">これまでの画面</a>
        をご利用ください。
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { MENU } from "~/utils/menu";

const route = useRoute();

const title = computed(() => {
  const found = MENU.find((m) => m.path === route.path);
  return found?.label || "準備中";
});

//  これまでの画面 (Streamlit 版) の経路。
//  oauth2-proxy は /new/ 以下をこの Nuxt に、それ以外をこれまでの画面に送る。
//  移し終えた画面はこの表から外す。
const OLD_VIEW: Record<string, string> = {
  "/spokes": "topology",
  "/fleet": "fleet",
  "/clusters": "clusters",
  "/nodes": "nodes",
  "/modules": "modules",
  "/users": "users",
  "/credentials": "git_credentials",
  "/kubectl": "kubectl",
  "/knowledgebase": "knowledgebase",
  "/chat": "chat",
  "/cost": "cost",
  "/projects": "projects",
  "/spawns": "spawns",
  "/security": "security",
};

const oldUrl = computed(() => {
  const view = OLD_VIEW[route.path] || "topology";
  //  /new/ の外に出るので、行先は URL の根から書く
  return `/?view=${view}`;
});
</script>
