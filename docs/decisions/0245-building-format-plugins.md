# 0245: 建物の形式のプラグインを、図面の形式と同じ窓口に登録し、wasm ではコアの建物のモジュールに足す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-02 |
| 状態 | 採用 |

## 背景

[決定 0243](0243-plugin-bundling.md) のプラグインは、図面の形式（`Lift<Drawing2D>`）だけだった。[#402](https://github.com/zumen-jp/zumen/issues/402) の ST-Bridge は構造の設計データで、図面モデルではなく建物モデル（[決定 0207](0207-building-model-components.md)）へ移す。台帳では、権利が `permission` で、置き場が非公開のプラグインのリポジトリ（`zumen-jp/stbridge`）である。

決めることは、次のとおり。

- 建物の形式のプラグインを、窓口に知らせる口と、開いた文書の持ち方。
- wasm のモジュールの中身。
- 非公開のリポジトリの試料と、仕様への照らし方。

## 選択肢

口の選択肢は、次の 2 つ。

1. **建物の形式のための別の登録の一覧（`install_building` など）を足す。** 図面と建物で口が 2 つになり、束ね方の 3 つの形も 2 通りずつ要る。
2. **同じ `Plugin` に、読んでできるモデルの種類を持たせる。** 登録の一覧と `install` は 1 つのまま。開くときと capabilities で、種類を見て分ける。

開いた文書の持ち方の選択肢は、次の 2 つ。

1. **建物モデルを IFC の L1 に写してから持つ。** building_apply なども使えるが、写し方が形式ごとに要り、写す途中で落ちるものが増える。
2. **IFC の L1 を持たない建物の文書にする。** 建物モデルを読む op は使え、IFC の L1 に書き戻す op は使えない。

## 決定

口は 2、持ち方は 2 を選ぶ。

### 窓口

- `zumen_abi1::Plugin` の読む関数を、列挙 `Reader`（`Drawing` と、`bim` の feature の `Building`）にする。`Plugin::building::<C>()` で、`Codec` と `Lift<Building>` を持つコーデックから作る。`install` は決定 0243 のまま。
- open は、コアの建物の形式（IFC と、IFC を入れた ZIP）のどれでもない入力を、登録した順に建物のプラグインの判定の規則で試す。図面のプラグインは、決定 0243 のとおり図面の形式の後に試す。
- プラグインの建物の文書は、IFC の L1 を持たない。building・building_elements・ids_check など建物モデルを読む op は使え、スキーマは建物モデルの `header.schemas` の最初の値を返す。building_apply は `call.document.unsupported` を返す。IFC から作る土木のモデルは作らない。
- capabilities は、図面のプラグインを `drawing` の feature で、建物のプラグインを `bim` の feature で、形式の一覧に足す。

### 束ね方

- 3 つの形は決定 0243 と同じ。wasm のモジュールは、プラグインの crate を `zumen-abi` の `bim` の feature と一緒に作り、コアの建物のモジュールに `zm_plugin_init` を足したものになる。JavaScript の包みには、`plugin.json` の `module` を `bim` にして渡す。

### 非公開のプラグインの試料

- 公開のページに試料がない形式は、プラグインのリポジトリに、XML Schema に合わせて書いた合成の試料を置く。XML Schema と仕様書は取ってきて使い、リポジトリに含めない。CI で公開のページから XML Schema を取り、試料を照らす。
- 第三者の公開の試料は、使う条件を確かめてから足す。

## 影響

- ST-Bridge のプラグイン（`zumen-jp/stbridge`）は、この口で、2.0 系と 2.1 系の構造の要素を建物モデルへ移す。形（断面の押し出し）は、建物モデルの形の項目が決まるまで持たない。
- 建物の形式のプラグインの wasm のモジュールは、コアの建物のモジュールの分だけ大きい。L2 のバイナリでやり取りする形は、[#481](https://github.com/zumen-jp/zumen/issues/481) で決める。
- プラグインの建物の文書を IFC に書き出す口は、要るときに足す。

## 参照

- [0017: 形式の権利の台帳とプラグインの SDK](0017-format-rights-ledger-and-plugin-sdk.md)
- [0207: 建物モデルの成分](0207-building-model-components.md)
- [0243: プラグインの束ね方](0243-plugin-bundling.md)
