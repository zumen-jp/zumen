# 適合テスト

言語に依存しない、API のテストです（[API のスキーマ](../../schema/README.md)）。同じテストの組を、API のどの形態・どの言語のバインディングでも流し、次のことを確かめます。

- 状態と、応答の値（JSON Pointer で指す）が、期待どおりであること。期待は仕様から書きます
- 応答とバイナリの結果のハッシュ（SHA-256）の一覧が、`hashes.txt` と一致すること。どこでも同じ結果になることを確かめます（[決定性の規則](../../docs/architecture.md#決定性)）

## 流し方

| 形態 | 流すもの | コマンド |
| --- | --- | --- |
| Rust から直接 | `tests/conformance.rs` | `cargo test -p zumen-conformance` |
| C ABI（プロセスの中） | `tests/conformance.rs` | 同上 |
| Rust の利用者向けの crate（`zumen`） | `tests/conformance.rs`（`src/wrapper.rs`） | 同上。シーンの blob を crate のリーダーで読み、応答の数と比べる（[Rust の API](../../crates/zumen/src/lib.rs)） |
| wasm | `wasm.mjs`（Node.js 22 以降） | `cargo build -p zumen-abi --target wasm32-unknown-unknown --release` のあと、`node bindings/conformance/wasm.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm` |
| TypeScript・JavaScript のラッパー（wasm） | `../js/test/conformance.mjs`（Node.js 22 以降） | 同じく作ったあと、`node bindings/js/test/conformance.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm`（[JavaScript の使い方](../js/README.md)） |
| TypeScript・JavaScript のラッパー（ブラウザー） | `../js/test/browser/run.mjs`（Playwright） | 同じく作ったあと、`node bindings/js/test/browser/run.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm`（手順は `../js/README.md`） |
| Python のラッパー（C ABI） | `../python/tests/conformance.py`（Python 3.9 以降） | `cargo build -p zumen-abi` のあと、`python3 bindings/python/tests/conformance.py target/debug/libzumen_abi1.so`（macOS は `.dylib`、Windows は `zumen_abi1.dll`。[Python の使い方](../python/README.md)） |
| .NET のラッパー（C ABI） | `../dotnet/tests/Conformance`（.NET 10。Windows では .NET Framework 4.8 も） | `cargo build -p zumen-abi` のあと、`dotnet run --project bindings/dotnet/tests/Conformance --framework net10.0 -- target/debug/libzumen_abi1.so`（[.NET の使い方](../dotnet/README.md)） |
| Java のラッパー（C ABI、JNA） | `../java/src/test/java/dev/zumen/Conformance.java`（Java 8 以降。Android 7.0 以降のエミュレーターでも） | `cargo build -p zumen-abi` と `sh bindings/java/fetch-jna.sh target/jna` のあと、`../java/README.md` の手順で作って流す（[Java の使い方](../java/README.md)）。毎晩 |
| Swift のラッパー（C ABI の静的ライブラリ） | `../swift/Sources/ZumenConformance`（macOS 12・iOS 15 以降） | `sh bindings/swift/build-xcframework.sh debug aarch64-apple-darwin aarch64-apple-ios-sim` のあと、`bindings/swift` で `swift run ZumenConformance ../..`。iOS のシミュレーターは `sh bindings/swift/test-simulator.sh`（[Swift の使い方](../swift/README.md)） |
| Go のラッパー（wasm、wazero） | `../go/conformance_test.go`（Go 1.26 以降） | `cargo build -p zumen-abi --target wasm32-unknown-unknown --release` のあと、モジュールを `bindings/go` に写し、`bindings/go` で `go test ./...`（[Go の使い方](../go/README.md)）。毎晩 |
| Dart のラッパー（C ABI、dart:ffi） | `../dart/tool/conformance.dart`（Dart 3.0 以降） | `cargo build -p zumen-abi` のあと、`bindings/dart` で `dart pub get` と `dart run tool/conformance.dart ../../target/debug/libzumen_abi1.so`（[Dart の使い方](../dart/README.md)）。毎晩 |
| VBA7 のラッパー（Windows、C ABI） | `../vba/ZumenConformance.bas` | `python bindings/vba/prepare.py target/vba-conformance.tsv` のあと、Office のマクロで全手順とハッシュを確認する。[VBA の手順](../vba/README.md)。Office 実機は [#800](https://github.com/zumen-jp/zumen/issues/800) で未確認。CI は Windows x86/x64 の DLL と手順の対応を検査する |
| C のヘッダーと C++ のラッパー（C ABI） | `../c/tests/conformance.rs` | `cargo test -p zumen-c`。組の手順は Rust のテストのツールが読み、API の関数は C と C++ の側から呼ぶ（[C・C++ の使い方](../c/README.md)） |
| CLI（`zumen serve --stdio` と `zumen mcp`） | `../cli/tests/conformance.rs` | `cargo test -p zumen-cli`。MCP はツールで表せる組だけを流す（[CLI の使い方](../cli/README.md)） |
| 言語に依らない形態（`zumen call`・`serve --stdio`・`serve --http`・WASI） | `cli.mjs`（Node.js 22 以降） | `cargo build -p zumen-cli --release` のあと、`node bindings/conformance/cli.mjs <call・stdio・http> target/release/zumen`。WASI は `cargo build -p zumen-cli --release --target wasm32-wasip1` のあと、`node bindings/conformance/cli.mjs wasi target/wasm32-wasip1/release/zumen.wasm`（ランタイムは `wasi.mjs` の node:wasi）。`call` は 1 回が 1 つのセッションなので、セッションの最初の call と直後の take だけを流す。Tier 3 の言語の代わり（[対応する環境](../../docs/support.md)）。毎晩 |

JavaScript で流すものは、手順の読み方と応答の確かめ方を `cases.mjs` で共有します。ブラウザーでも読み込むため、ファイルを読まない部分は `steps.mjs` に置き、`cases.mjs` から出し直します。各言語のバインディングを足すときは、その言語で同じ組を流すものを足し、CI に入れます（[対応する環境](../../docs/support.md) の「適合テストに通る」）。

## ハッシュの一覧を作り直す

応答を変えたときは、一覧を作り直し、変わった理由をコミットか PR に書きます。一覧は退行を見つけるためのもので、正しさの根拠にはしません。

```bash
cargo run -p zumen-conformance -- --write
```

`capabilities` の応答はパッケージのバージョンとスキーマのマイナーバージョンを含むので、バージョンを上げたときにも作り直します。

## 形式の入力

[#2](https://github.com/zumen-jp/zumen/issues/2) の出力の一致と [#8](https://github.com/zumen-jp/zumen/issues/8) の停止位置の一致は、DXF・DWG・SXF（SFC・P21）・JWW・IFC・IDS・BCF などの形式を読む組で確かめます。

- 呼び出し方: `zm_push` で入力を渡し、`open` の要求を送ります。コアは、コーデックが持つ判定の規則で形式を決めて読み、文書の名前と形式を返します。`capabilities` の `formats` に、読める形式が出ます。続けて `drawing` の要求を送ると、文書を図面モデル（L2）に変換し、空間と用紙ごとの単位・要素の数・範囲と、移さなかった図形の診断を返します。`entities` の要求を送ると、要素の一覧（ID・種類・レイヤ・出所）を、`offset` と `limit` で区切って返します（[API のスキーマ](../../schema/README.md)）。`fields` で範囲（`bbox`）を含む返すフィールドを選び、`region` で範囲の重なる要素に絞ります（[API のスキーマ](../../schema/README.md)）。`apply` の要求を送ると、`entities` が返す ID とハッシュで要素を指す変更を DXF・DWG に当てて書き戻し、書き戻したファイルを `take` で取り出せます（[図面モデルとの対応](../../docs/formats/drawing2d-mapping.md)）。変更は、スタイルの変更（`restyle`）・変換（`transform`）・除去（`remove`）と、形の値（`geometry`）での置き換え（`replace`）と追加（`add`）です。`entities` の `fields` に `geometry` を入れると、線分・円・円弧・楕円・折れ線・文字（2 点の間に合わせる文字を含む）・書式つきの文字・記号・スプライン（フィット点で決まるものを含む）・点・引出線・引出線つきの注記・寸法・表の形の値を返します。`tables` の要求を送ると、レイヤ・線種・寸法スタイルの表の項目を ID・名前・ハッシュで返します（[ID と編集の前提条件](../../schema/README.md#id-と編集の前提条件)）。`lift` の要求を送ると、図面モデルのバイナリを blob で返します。それを `zm_push` で渡して `open` すると、元の文書の形式のまま、読むだけの文書になります（[API のスキーマ](../../schema/README.md)）
- 対象の入力: `cases/open.json` の、テストの中で作る ASCII の DXF（SECTION・ENTITIES・EOF の 3 つのタグ）と、`cases/drawing.json` の、ヘッダーの単位と用紙の範囲、モデル空間とペーパー空間の図形を持つ DXF と、`cases/entities.json` の、`drawing.json` と同じ DXF と、`cases/entities_e9.json` の、対応表の E9 の輪になる定義・定義のない配置・有限でない尺度・展開しない格子の配置を持つ DXF と、`cases/open_dwg.json` の、`zumen-acad` のテストと同じ合成の DWG と、`cases/open_sxf.json` の、テストの中で作るレイヤ・線分・用紙を持つ SFC と、同じ図面の P21 と、FILE_SCHEMA が SXF の図面のスキーマでない入力と、`cases/open_jww.json` の、テストの中で作る縮尺 1/100 のレイヤグループに線分を持つ JWW と、ヘッダーが途中で切れた入力と、`cases/apply.json` の、ハンドルを持つ線と円の DXF と、`cases/convert.json` の、`open_sxf.json` と同じ SFC と `open_jww.json` と同じ JWW と、`cases/convert_playground.json` の、`open_dwg.json` と同じ DWG と `open_jww.json` と同じ JWW と `open_sxf.json` と同じ SFC・P21 と、公開サンプルの DWG（R2000・R2004・R2018）と、`cases/samples_dxf.json` の、公開サンプルの `samples/dxf/text_alignments.dxf` と、`cases/scene.json` の、シーンを作る合成の DXF と同じ公開サンプルと、`cases/scene_fonts.json` の、テストの中で組み立てた合成の TrueType で文字を字形にする DXF と、`cases/check.json` の、プロファイルで照合する合成の DXF と、`cases/validate.json` の、テストの中で書いた小さな EXPRESS と、それに照らす合成の IFC と、`cases/review.json` の、`check.json` と同じ DXF と、テストの中で書いたレビューの文書と、`cases/building.json` の、テストの中で書いた階と扉を持つ小さな IFC4 と、扉の属性を求める IDS と、IFC4 の一部を写した EXPRESS と、`cases/building_drawing.json` の、`building.json` と同じ IFC4 に図面の op を呼ぶ組と、`cases/bcf.json` の、`building.json` と同じ IFC4 と、テストの中で書いた BCF 3.0 と 2.1 の bcfzip と、レビューの文書と、`cases/lift.json` の、テストの中で作る線と円の DXF と、それを lift で書いた図面モデルのバイナリと、それを切ったもの・メジャーバージョンを変えたもの・新しいマイナーバージョンのフィールドとセクションを足したものです
- 止まる所: 入力の大きさは読む前に、作業は読むときにタグごと、表を作るときにレコードごと、変換で ENTITIES の図形ごとに、レコードはグループ・コード 0 のタグごとに数えます。止まった診断の `requested` は、その単位でそれまでに使った量です。同じ入力と予算なら、どの形態でも同じ値になることを、`hashes.txt` で確かめます

いまの制限です。それぞれ、要るものと一緒に足します。

- `entities` の形の値は、一部の種類だけを返します。範囲は形の定義から求め、文字の字形と記号の中身を含みません（[API のスキーマ](../../schema/README.md)）。空間か用紙を選ぶ `view` は、まだありません
- 停止の診断は、DXF のタグ・DXF の表・DWG のオブジェクトの表・図面モデルへの変換・`entities` で止めたとき、扱っていたレコードかバイト範囲を場所（`at`）に持ちます（[予算と中断](../../docs/architecture.md#予算と中断)）。DWG のヘッダー・クラス・オブジェクトの位置の表・R2004 のコンテナーで止めたときは、まだ持ちません
- 合成でない入力は、`cases/samples_dxf.json` の DXF と `cases/convert_playground.json` などの DWG（`samples/dwg/`）の公開サンプルです。ほかの形式の公開サンプルも、再配布できるものを確かめてから `samples/` に足します（#66）

## テストの組の書き方

`render.json` は、空間と用紙からの SVG、範囲と余白、未描画の元の ID、不正な設定と予算による停止・失敗後の再利用を確かめます。`render_fonts.json` は `scene_fonts.json` と同じ合成の TrueType を使い、フォントと高さの指定から字形の輪郭を描く出力・診断を照合します。どちらも図面のモジュールで、画像は `image` blob として同じハッシュになります。API の中断は zumen-abi、描画部品の中断と予算は zumen-render の契約テストでも確かめています（[シーンの描画](../../docs/render.md)）。

`render_png.json` と `render_fonts_png.json` は同じ図面・フォントを PNG で描き、画像の全バイト列と診断を照らします。大きすぎる寸法、描画と符号化の予算停止、停止後の再利用も含みます（[シーンの描画](../../docs/render.md)）。

`apply_hatch.json` は、合成 DXF と公開 DWG（R2000・R2004・R2013・R2018）に、塗りつぶしと線を定義する模様の HATCH を足します。膨らみを持つ外周・円の穴・模様・種点、既存の ID とハッシュ、無効な境界を含む変更と予算停止の原子性を確かめます。形式の正しさは、この一致検査と分けて [ezdxf・ACadSharp で照合](../../xtask/verify/README.md)します。

`apply_dwg_table.json` は、[自作の表](../../crates/zumen-acad/tests/fixtures/README.md)を持つ R2010・R2013・R2018 の DWG で、位置だけの `replace` と `transform` を確かめます。ID・セル・表示用ブロックの参照と未変更要素のハッシュを保ち、セルの変更・回転・予算停止で変更が残らないことを比べます。形式の照合は、検証キットで ACadSharp と別に行います。

### 図面と記号定義の詳細

`drawing_metadata.json` は全 14 種類の件数・未配置の定義・属性の既定値・出所が重なる公開 ID・ビューの回転と反転とせん断・単位不明・予算停止を照らします。`drawing_metadata_formats.json` は自作 DXF・DWG・JWW・SFC・P21 で、一覧の名前から記号を配置し、再読した全応答を比べます。`cargo run -p zumen-abi --example drawing_metadata_case` と `cargo run -p zumen-abi --example drawing_metadata_formats` で作り、`-- --check` を付けると生成元と照らします。数値と参照の正しさは ABI の固定した期待値と別に照合します。

### 要素の絞り込み

`entity_query.json` は、同名で出所も重なるレイヤ、同じ出所の要素、出所を持たない要素、モデル・用紙・記号定義を持つ合成図面です。`ids`・`layers`・`kinds`・`source`・`region`・`place` の組み合わせ、空や重複した指定、選択後のページ位置、半開区間の重なり、不正な条件と予算停止後の再利用を確かめます。`cargo run -p zumen-abi --example entity_query_case` で生成し、`-- --check` を付けると生成元と照らせます。ABI のテストでは、手で定めた集合の積との比較、選ばない要素の属性を複製しないこと、条件の複製・並べ替え・検索の予算の境界も確認します。

### 要素のスタイルと拡張

`entity_details.json` は、要素自身のスタイル、重複する出所と線種の公開 ID、拡張の全型・順序・有効性、ページ分け、記号定義の中の要素、予算停止後の再利用を確かめます。64 bit 整数・ハンドルの上限、空のバイト列も含みます。`cargo run -p zumen-abi --example entity_details_case` で合成の図面モデルから生成します。

`entity_details_formats.json` は、DXF・DWG・JWW・SXF で `restyle` した色を `style` で読み、書き戻したファイルを新しいセッションで読み直します。`cargo run -p zumen-abi --example entity_details_formats` で既存の合成入力から生成します。生成時と ABI のテストで、編集後と再読後のスタイル・拡張・ハッシュの全応答が一致することも確かめます。両方の生成コマンドに `-- --check` を付けると、生成元との一致を検査できます。

### 色表と非表示

`palette.json` は、ACI・JWW・SXF の符号と図面内 RGB を持つ合成の図面モデルで、組み込み色表を使う `scene`・`render`・`check` の応答を照らします。SVG・PNG での非表示、未知のレイヤ・色表、予算停止後の再利用、元のハッシュの保持も含みます。`cargo run -p zumen-abi --example palette_case` で生成し、末尾に `-- --check` を足すと生成元との一致を確かめられます。数値の正しさは、ABI の色と画素の期待値、および[外部実装による全符号の照合](../../xtask/verify/README.md#既定の色表を照らす)で別に確かめます。

### 計測

`measure.json` は、線分・円・円弧・膨らみのない折れ線・楕円・制御点とフィット点のスプライン・クロソイド・点を持つ合成の図面モデルで、`measure` の値と全応答のハッシュを照らします。同じ出所が重なる公開 ID、単位不明、未対応の形、閉じていない面積、予算停止と再利用も含みます。生成元は `crates/zumen-abi/examples/measurement_case.rs` と隣の JSON です。

```bash
cargo run -p zumen-abi --example measurement_case
cargo run -p zumen-abi --example measurement_case -- --check
```

環境間の一致とは別に、円と円弧・閉じた経路の解析式、楕円の完全楕円積分による周長、2 次 Bézier の速度の積分を、ABI の計測テストの期待値に使います。幾何のテストは、離れた円・内包する円・接線・交差・角度の範囲・膨らみを持つ辺・大きな座標での面積も確かめます。スプラインの `work`・`decoded_bytes`・`segments` の停止境界と中断は、ABI のテストで確認します。

### 合成フォント

`cases/scene_fonts_cff.json` は、CFF・CFF2 と、両方を含むフォントのコレクション（TTC）の合成の入力です（#564）。実在のフォントは使わず、次のコマンドで組み立てます。`--check` は、作り直した組が記録した JSON と同じかを確かめます。

```bash
node bindings/conformance/generate-font-cases.mjs
node bindings/conformance/generate-font-cases.mjs --check
```

`H` は長方形で、CFF の幅は 400、CFF2 の幅は 700 です。`g` は局所と全体の副ルーチンを呼び、3 次 Bézier 曲線を描きます。`font_outlines.rs` はシーンの頂点を調べ、長方形の幅と曲線の最大値が合うこと、単独のフォントと TTC の番号 0・1 の結果がそれぞれ一致することを確かめます。ネイティブと wasm は、応答とシーンのハッシュを比較します。

入力のバイト列は組の `push.hex` に置くので、`cargo xtask fuzz-seeds` で全 target の種になります。組を変えたときは、上の手順でハッシュの一覧も作り直してください。

### 手順と期待値

`cases/<名前>.json` に 1 件ずつ置きます。名前の順に流し、組ごとに新しい形（セッション）で始めます。入力はテストの中で作る合成のものか、`samples/` の公開サンプル（[公開サンプルの条件](../../samples/README.md)）にし、受け取った図面は使いません。

```json
{
  "description": "何を確かめる組か",
  "steps": [
    { "push": { "name": "a.dxf", "text": "0\nSECTION\n" } },
    {
      "call": { "v": 1, "op": "open", "input": "a.dxf", "budget": { "input_bytes": 9 } },
      "status": "failed",
      "expect": { "/diagnostics/0/code": "budget.input-bytes.exceeded", "/diagnostics/0/params/requested": 10 },
      "absent": ["/result"]
    }
  ]
}
```

組は `"module"` のフィールドを持てます（[API のスキーマ](../../schema/README.md)）。値はモジュール名か `all` で、`all` の組は、すべてのモジュールを入れた wasm でだけ流します。フィールドがなければ、分けたモジュールで流すとき、手順の入力の形式と op から、それを持つモジュールを選びます。

手順は、次のどれか 1 つのフィールドを持ちます。最初の手順が `session` でなければ、既定の設定でセッションを作ってから始めます。

| フィールド | 中身 |
| --- | --- |
| `session` | セッションを作り直す。`options`（JSON の値）か `options_text`（文字列）で設定を渡す。どちらもなければ既定の設定 |
| `push` | 入力を足す。名前は `name`（文字列）か `name_hex`、中身は `text`（UTF-8）か `hex` か `file`。`file` は、リポジトリの根から書いた `samples/` の中の公開サンプルの道筋（[公開サンプルの条件](../../samples/README.md)）。どれもなければ空 |
| `call` | 要求（JSON の値）を渡す |
| `call_text` | 要求を、文字列のまま渡す。読めない JSON を試すのに使う |
| `call_hex` | 要求を、16 進で書いたバイト列で渡す。UTF-8 でないものを試すのに使う |
| `take` | バイナリの結果を、番号で取り出す |

どの手順にも、次のフィールドを置けます。

| フィールド | 中身 |
| --- | --- |
| `status` | 期待する状態（`ok`・`failed`・`invalid`・`busy`・`panicked`）。書かなければ `ok` |
| `expect` | JSON Pointer と、期待する値。数は値で、オブジェクトはキーの順によらずに比べる |
| `absent` | 応答に、あってはいけない場所の JSON Pointer |

ハッシュの一覧の行は `<組の名前> <手順の番号> <response か blob> <ハッシュ>` です。応答とバイナリの結果を受け取った手順だけが、行を持ちます。

`call` の要求は、流すものがそれぞれの言語で JSON に直して渡します。キーの順や数の書き方が違っても、コアは同じ値として読むので、応答は同じバイト列になります。バイト列のまま渡したいときは、`call_text` か `call_hex` を使います。
