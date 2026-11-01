//  見ている人を一度だけ取り、画面のあいだで使い回す。

export interface Me {
  user: string;
  email: string;
  roles: string[];
  isAdmin: boolean;
  rolesKnown: boolean;
  //  見てよいクラスターとモジュール。["*"] は全部。
  allowedClusters: string[];
  allowedModules: string[];
  //  権限を読めたかどうか。読めないときは許可を増やさない。
  aclKnown: boolean;
}

const EMPTY: Me = {
  user: "",
  email: "",
  roles: [],
  isAdmin: false,
  rolesKnown: false,
  allowedClusters: [],
  allowedModules: [],
  aclKnown: false,
};

export async function useMe() {
  const me = useState<Me>("me", () => ({ ...EMPTY }));

  if (!me.value.user && !me.value.email) {
    try {
      const got = await $fetch<Me>("/api/whoami");
      me.value = { ...EMPTY, ...got };
    } catch {
      //  取れなくても画面は出す。権限の要る操作は kernel が断る。
      me.value = { ...EMPTY };
    }
  }

  return me;
}
