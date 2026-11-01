<template>
  <div class="shell">
    <aside class="sidebar">
      <div class="brand">
        <span class="dot" />
        <span>OpenSpoke</span>
      </div>

      <div class="who">
        <template v-if="me.user || me.email">
          <div class="name">{{ me.user || me.email }}</div>
          <div v-if="me.email && me.email !== me.user" class="mail">
            {{ me.email }}
          </div>
        </template>
        <div v-else class="mail">(利用者を取得できていません)</div>
      </div>

      <nav class="nav">
        <template v-for="group in GROUPS" :key="group">
          <div v-if="inGroup(group).length" class="nav-group">{{ group }}</div>
          <NuxtLink
            v-for="item in inGroup(group)"
            :key="item.path"
            :to="item.path"
            :class="{ on: isOn(item.path) }"
          >
            {{ item.label }}
          </NuxtLink>
        </template>
      </nav>

      <div class="sidebar-foot">
        <a :href="logoutUrl">ログアウト</a>
      </div>
    </aside>

    <main class="main">
      <slot />
    </main>
  </div>
</template>

<script setup lang="ts">
import { GROUPS, visibleMenu, type MenuItem } from "~/utils/menu";

const route = useRoute();
const me = await useMe();

const menu = computed<MenuItem[]>(() =>
  visibleMenu(me.value.isAdmin, me.value.rolesKnown),
);

function inGroup(group: string): MenuItem[] {
  return menu.value.filter((item) => item.group === group);
}

function isOn(path: string): boolean {
  if (path === "/") return route.path === "/";
  return route.path === path || route.path.startsWith(`${path}/`);
}

//  ログアウトは oauth2-proxy と Keycloak の両方を続けて落とす。
//  片方だけだと、すぐに自動で入り直してしまう。
const logoutUrl = computed(() => {
  const back = encodeURIComponent("https://llm.example.com/");
  const kc =
    "https://keycloak.example.com/realms/rag/protocol/openid-connect/logout" +
    `?client_id=rag-frontend1&post_logout_redirect_uri=${back}`;
  return `/oauth2/sign_out?rd=${encodeURIComponent(kc)}`;
});
</script>
