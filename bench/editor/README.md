# エディターの設計のための測り

[#693](https://github.com/zumen-jp/zumen/issues/693) の測りです。エディター（[#692](https://github.com/zumen-jp/zumen/issues/692)）で、1 回の操作ごとに `apply` してシーンを作り直せるか、取り消しをどう持つかを決めるために、`open`・`drawing`・`scene`・`entities`・`apply` の時間と作業量を、ネイティブと wasm で測ります。結果と判断は[決定 0288](../../docs/decisions/0288-editor-placement-edit-flow-and-history.md) にあります。

コアのワークスペースには入れていません。依存は、このリポジトリの `zumen` の crate だけです。`Cargo.lock` は、ワークスペースの `Cargo.lock` から同じバージョンを引き継いでいます。

## 図面

公開サンプルと適合テストの合成の図面を土台にし、`apply` の `add` で要素を足して、大きさの違う図面を作ります。受け取った図面は使いません。作った図面は Git に入れず、`target/` の下に置きます。

| 土台 | 出所 | 足すもの | 大きな図面 |
| --- | --- | --- | --- |
| DWG | `samples/dwg/zumen-layout-r2018.dwg` | 線・折れ線・円・円弧・文字 | 6 万件 |
| DXF | `samples/dwg/zumen-layout-r2018_ascii.dxf` | 線・折れ線・円・円弧・文字 | 10 万件 |
| JWW | `bindings/conformance/cases/apply.json` の `d.jww` | 点 | 10 万件 |
| SFC | 同じく `b.sfc` | 線・折れ線・円・円弧 | 10 万件 |
| P21 | 同じく `c.p21` | 線・折れ線・円・円弧 | 3 万件 |

- 大きさは、土台のまま・1,000 件・10,000 件・大きな図面の 4 つです。要素は、土台の要素の範囲に格子に並べます。
- DWG は、足して `$HANDSEED` のバイト数が増える（0xFFFF を超える）と書き戻せない（[#701](https://github.com/zumen-jp/zumen/issues/701)）ので、6 万件までにしました。
- JWW は、API の `add` で線・円・円弧を足せない（[#702](https://github.com/zumen-jp/zumen/issues/702)）ので、点を足しました。
- SFC・P21 の `add` は、文字を足せず、線種と線幅をレイヤに従わせられないので、文字を除き、線種と線幅を明示しました。
- P21 は、`apply` の時間が図面の大きさより速く増え（[#709](https://github.com/zumen-jp/zumen/issues/709)）、足して作るのにも測るのにも時間がかかるので、3 万件までにしました。

## 測る場合

図面ごとに、次の 11 の場合を測ります。場合ごとに新しいセッション（wasm は新しいインスタンス）を作り、準備（入力の `push` と `open`）を流してから、測る要求を流します。測る時間には、応答の blob を取り出す時間を含めます。

| 場合 | 測る要求 |
| --- | --- |
| `open` | `open` |
| `drawing` | `drawing` |
| `scene` | 最初の空間か用紙からの `scene` |
| `entities_page` | `entities` の 1,000 件（`hash`） |
| `entities_all` | `entities` の全件（`kind`・`layer`・`hash`） |
| `apply_restyle_1` | 1 件の色を変える |
| `apply_transform_1` | 1 件を動かす |
| `apply_transform_100` | 100 件を動かす（土台の図面では、ある数だけ） |
| `apply_add_1` | 1 件を足す |
| `apply_remove_1` | 1 件を除く |
| `scene_after_apply` | 1 件を動かした `apply` のあとの `scene` |

作業量は、通る最小の予算です。土台のままの図面を除き、`open`・`scene`・`apply_transform_1` について、予算の単位（`work`・`records`・`elements`・`segments`）ごとに、その単位の上限だけを変えて、通るかどうかで二分探索します。5 万要素を超える図面では、時間がかかるので `work` だけを探します。予算は決まった作業量で数えるので（[決定 0010](../../docs/decisions/0010-budget-by-work.md)）、ネイティブと wasm で同じ値になります。

## 流し方

リポジトリのルートで、次を流します。Node.js 22 以降を使います。

```bash
cargo build -p zumen-abi --release --target wasm32-unknown-unknown --locked --no-default-features --features drawing
node bench/editor/prepare.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm target/bench-editor
cargo build --release --locked --manifest-path bench/editor/Cargo.toml --target-dir target
target/release/measure target/bench-editor/plan.txt time
target/release/measure target/bench-editor/plan.txt work
node bench/editor/run.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm target/bench-editor/plan.txt time
node bench/editor/run.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm target/bench-editor/plan.txt work -1k/
```

- `prepare.mjs` は、図面と、場合の一覧（`plan.txt`）・図面の大きさ（`inputs.tsv`）を書きます。前に作った図面があれば使うので、作り直すときは出力のフォルダーを消します。
- `measure` と `run.mjs` の `time` は、場合ごとに中央値・最小・最大（ミリ秒）、応答のバイト数、blob のバイト数を書きます。`run.mjs` は、測ったあとの wasm のメモリの大きさ（バイト）も書きます。回数は、5 万要素を超える図面で 3 回、ほかで 5 回です。
- `work` は、単位ごとに、通る最小の予算を書きます。wasm の `work` は時間がかかるので、上の例では 1,000 要素の図面だけを流し、ネイティブの値と比べます。
- 3 つ目の引数（`run.mjs` では 4 つ目）に文字を渡すと、名前にその文字を含む場合だけを流します。
