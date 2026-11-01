# sysml-validator

OpenSpoke の知識ベースに書く SysML v2 本文を調べる HTTP サービス。

2026-10-30 の方針 (知識ベースの topic `knowledge_base_notation_policy_sysml_v2_20261030`) により、
この日以降に記録する知見は本文を SysML v2 のテキスト記法で書く。
kernel の知識ベース書き込み (`backends.py` の `MemgraphHandoffStore.topic_upsert`) が
書き込みの直前にこのサービスを呼び、合格しない本文は書き込まない。
AI が任意に呼ぶ道具ではなく、書き込みの経路に組み込まれた仕組みとして動く。

## 2 段で調べる

| 段 | 部品 | 見るもの |
|---|---|---|
| 1 | 構文チェッカー `sysml2` 0.3.1 | 構文のみ (参照の解決は見ない) |
| 2 | バリデーター `mercurio-sysml` 0.86.0 | 構文と、型・再定義先・式中の名前の解決 |

片方だけでは取りこぼしがある (`sysml2` は未解決の参照を通し、`mercurio-sysml` は名前の無い属性を通す)。
両方に合格したものだけを合格とする。

## 口

- `GET /healthz` → `ok`
- `POST /validate` 本文 `{"content": "<SysML v2 テキスト>"}`
  - 返り `{"ok", "syntax": {"ok", "diagnostics": [...]}, "semantic": {...}, "elements", "elapsed_ms", "tools"}`
  - `diagnostics` の各要素は `{"line", "column", "code", "message"}`

## 組み立て

OpenSpoke の CI (`builder` 名前空間のディスパッチャー) が `.openspoke/build.json` に従って組み立て、
`your-registry.example.com/sysml-validator:<時刻>-<コミット>` に置き、
`hub/manifests/sysml-validator/Deployments/sysml-validator.yaml` の `image` を差し替える。

組み立てのたびに `cargo test` で `tests/cases` を流す。`ok_*` は合格、`ng_*` と、
実物のモデルの参照を 1 か所だけ壊したもの 4 通りは不合格でなければならない。
1 件でも外れたらイメージを作らない。
