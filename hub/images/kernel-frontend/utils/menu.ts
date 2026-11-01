//  画面の一覧。サイドバーとルーティングはここだけを見て組み立てる。
//
//  用語の方針 (topic claude_japanese_wording_no_invented_kanji):
//    1. 製品名・固有名詞はそのまま        CI / Fleet / Kubectl / Claude
//    2. 適切な日本語があればそれを使う    利用者 / 認証情報 / 脆弱性
//    3. 無ければカタカナ                  スポーン / スポーク / クラスター
//    4. 無理に漢字へ訳さない              スポーンを「差配」などとしない
//
//  adminOnly は管理者だけに見せるもの。
//  隠す範囲は旧 UI の URL_TO_VIEW_FULL の絞り込みに合わせてある
//  (利用者 / ナレッジベース / チャット / コスト / 脆弱性)。

export interface MenuItem {
  path: string;
  label: string;
  group: string;
  adminOnly?: boolean;
}

export const MENU: MenuItem[] = [
  //  ビルドと配信
  { path: "/ci", label: "CI", group: "ビルドと配信" },
  { path: "/fleet", label: "Fleet", group: "ビルドと配信" },
  { path: "/modules", label: "モジュール", group: "ビルドと配信" },

  //  インフラ
  { path: "/spokes", label: "スポーク", group: "インフラ" },
  { path: "/clusters", label: "クラスター", group: "インフラ" },
  { path: "/nodes", label: "ノード", group: "インフラ" },
  { path: "/kubectl", label: "Kubectl", group: "インフラ" },

  //  タスク管理
  { path: "/projects", label: "プロジェクト", group: "タスク管理" },
  { path: "/spawns", label: "スポーン", group: "タスク管理" },

  //  Claude
  { path: "/chat", label: "チャット", group: "Claude", adminOnly: true },
  { path: "/knowledgebase", label: "ナレッジベース", group: "Claude", adminOnly: true },
  { path: "/cost", label: "コスト", group: "Claude", adminOnly: true },

  //  監視
  { path: "/security", label: "脆弱性", group: "監視", adminOnly: true },
  { path: "/audit", label: "監査", group: "監視" },

  //  アカウント管理
  { path: "/users", label: "利用者", group: "アカウント管理", adminOnly: true },
  { path: "/credentials", label: "認証情報", group: "アカウント管理" },
];

export const GROUPS = [
  "ビルドと配信",
  "インフラ",
  "タスク管理",
  "Claude",
  "監視",
  "アカウント管理",
];

export function visibleMenu(isAdmin: boolean, rolesKnown: boolean): MenuItem[] {
  //  ロールが読めないときは隠さない。実際の可否は kernel が判定する。
  if (!rolesKnown) return MENU;
  return MENU.filter((item) => !item.adminOnly || isAdmin);
}
