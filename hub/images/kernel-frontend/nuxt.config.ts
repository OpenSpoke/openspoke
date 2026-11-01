//  OpenSpoke 管理ダッシュボードの設定。
//
//  見た目の部品 (Vuetify / Tailwind 等) は入れない。
//  配色と間隔は assets/css/main.css の変数だけで決める。

export default defineNuxtConfig({
  compatibilityDate: "2026-10-01",

  //  画面は認証の内側だけで動く。先に組み立てておく意味がないので
  //  描画は browser 側に寄せ、server は中継に専念させる。
  ssr: false,

  devtools: { enabled: false },

  css: ["~/assets/css/main.css"],

  app: {
    head: {
      title: "OpenSpoke",
      meta: [
        { charset: "utf-8" },
        { name: "viewport", content: "width=device-width, initial-scale=1" },
        { name: "color-scheme", content: "dark light" },
      ],
    },
  },

  runtimeConfig: {
    //  server 側だけが見る。外には出さない。
    kernelUrl:
      process.env.KERNEL_URL ||
      "http://rag-backend-kernel.rag-company1.svc.cluster.local:8000",
    keycloakUrl: process.env.KEYCLOAK_URL || "https://keycloak.example.com",
    keycloakRealm: process.env.KEYCLOAK_REALM || "rag",
    keycloakAdminClientId: process.env.KEYCLOAK_ADMIN_CLIENT_ID || "",
    keycloakAdminClientSecret: process.env.KEYCLOAK_ADMIN_CLIENT_SECRET || "",

    public: {
      //  画面に出す行先。秘密は含めない。
      appName: "OpenSpoke",
    },
  },

  nitro: {
    //  ファイル名に [ ] を使うと Fleet の post render が蹴るため、
    //  受け皿は自分で登録する (topic fleet_kustomize_configmap_source_pitfalls)。
    //  今は容器を焼く形なのでその制約は無いが、素性を変えない。
    compressPublicAssets: true,
  },

  typescript: {
    //  建てる時に型で止めない (画面の数が多く、順に移すため)
    typeCheck: false,
  },
});
