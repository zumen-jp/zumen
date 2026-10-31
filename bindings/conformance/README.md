# 適合試験

言語に依存しない、境界の試験です（[決定 0012](../../docs/decisions/0012-boundary-functions-and-messages.md)）。同じ試験の組を、境界のどの形・どの言語の結合でも流し、次のことを確かめます。

- 状態と、応答の値（JSON Pointer で指す）が、期待どおりであること。期待は仕様から書きます
- 応答とバイナリの結果のハッシュ（SHA-256）の一覧が、`hashes.txt` と一致すること。どこでも同じ結果になることを確かめます（[決定 0007](../../docs/decisions/0007-single-rust-implementation.md)）

## 流し方

| 形 | 流すもの | コマンド |
| --- | --- | --- |
| Rust から直接 | `tests/conformance.rs` | `cargo test -p zumen-conformance` |
| C ABI（プロセスの中） | `tests/conformance.rs` | 同上 |
| Rust の利用者向けの crate（`zumen`） | `tests/conformance.rs`（`src/wrapper.rs`） | 同上。シーンの blob を crate の読み手で読み、応答の数と比べる（[決定 0203](../../docs/decisions/0203-rust-user-crate.md)） |
| wasm | `wasm.mjs`（Node.js 22 以降） | `cargo build -p zumen-abi --target wasm32-unknown-unknown --release` のあと、`node bindings/conformance/wasm.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm` |
| TypeScript・JavaScript の包み（wasm） | `../js/test/conformance.mjs`（Node.js 22 以降） | 同じく作ったあと、`node bindings/js/test/conformance.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm`（[決定 0194](../../docs/decisions/0194-typescript-javascript-wrapper.md)） |
| TypeScript・JavaScript の包み（ブラウザー） | `../js/test/browser/run.mjs`（Playwright） | 同じく作ったあと、`node bindings/js/test/browser/run.mjs chromium target/wasm32-unknown-unknown/release/zumen_abi1.wasm`（[決定 0199](../../docs/decisions/0199-browser-conformance-with-playwright.md)、手順は `../js/README.md`） |
| Python の包み（C ABI） | `../python/tests/conformance.py`（Python 3.9 以降） | `cargo build -p zumen-abi` のあと、`python3 bindings/python/tests/conformance.py target/debug/libzumen_abi1.so`（macOS は `.dylib`、Windows は `zumen_abi1.dll`。[決定 0195](../../docs/decisions/0195-python-wrapper.md)） |
| .NET の包み（C ABI） | `../dotnet/tests/Conformance`（.NET 10。Windows では .NET Framework 4.8 も） | `cargo build -p zumen-abi` のあと、`dotnet run --project bindings/dotnet/tests/Conformance --framework net10.0 -- target/debug/libzumen_abi1.so`（[決定 0196](../../docs/decisions/0196-dotnet-wrapper.md)） |
| Java の包み（C ABI、JNA） | `../java/src/test/java/dev/zumen/Conformance.java`（Java 8 以降。Android 7.0 以降のエミュレーターでも） | `cargo build -p zumen-abi` と `sh bindings/java/fetch-jna.sh target/jna` のあと、`../java/README.md` の手順で作って流す（[決定 0224](../../docs/decisions/0224-java-kotlin-wrapper.md)）。毎晩 |
| Swift の包み（C ABI の静的ライブラリ） | `../swift/Sources/ZumenConformance`（macOS 12・iOS 15 以降） | `sh bindings/swift/build-xcframework.sh debug aarch64-apple-darwin aarch64-apple-ios-sim` のあと、`bindings/swift` で `swift run ZumenConformance ../..`。iOS のシミュレーターは `sh bindings/swift/test-simulator.sh`（[決定 0225](../../docs/decisions/0225-swift-wrapper.md)） |
| Go の包み（wasm、wazero） | `../go/conformance_test.go`（Go 1.26 以降） | `cargo build -p zumen-abi --target wasm32-unknown-unknown --release` のあと、モジュールを `bindings/go` に写し、`bindings/go` で `go test ./...`（[決定 0226](../../docs/decisions/0226-go-wrapper.md)）。毎晩 |
| Dart の包み（C ABI、dart:ffi） | `../dart/tool/conformance.dart`（Dart 3.0 以降） | `cargo build -p zumen-abi` のあと、`bindings/dart` で `dart pub get` と `dart run tool/conformance.dart ../../target/debug/libzumen_abi1.so`（[決定 0227](../../docs/decisions/0227-dart-wrapper.md)）。毎晩 |
| C のヘッダーと C++ の包み（C ABI） | `../c/tests/conformance.rs` | `cargo test -p zumen-c`。組の手順は Rust の試験の道具が読み、境界の関数は C と C++ の側から呼ぶ（[決定 0198](../../docs/decisions/0198-c-and-cpp-wrapper.md)） |
| CLI（`zumen serve --stdio` と `zumen mcp`） | `../cli/tests/conformance.rs` | `cargo test -p zumen-cli`。MCP は道具で表せる組だけを流す（[決定 0197](../../docs/decisions/0197-cli-and-mcp-server.md)） |

JavaScript で流すものは、手順の読み方と応答の確かめ方を `cases.mjs` で共有します。ブラウザーでも読み込むため、ファイルを読まない部分は `steps.mjs` に置き、`cases.mjs` から出し直します。各言語の結合を足すときは、その言語で同じ組を流すものを足し、CI に入れます（[決定 0006](../../docs/decisions/0006-language-tiers.md) の「適合試験に通る」）。

## ハッシュの一覧を作り直す

応答を変えたときは、一覧を作り直し、変わった理由を PR に書きます。一覧は退行を見つけるためのもので、正しさの根拠にはしません。

```bash
cargo run -p zumen-conformance -- --write
```

`capabilities` の応答はパッケージの版とスキーマの小版を含むので、版を上げたときにも作り直します。

## 形式の入力

[#2](https://github.com/zumen-jp/zumen/issues/2) の出力の一致と [#8](https://github.com/zumen-jp/zumen/issues/8) の停止位置の一致は、形式を読む組で確かめます。いまの対象は DXF と DWG（R2000）と SXF の SFC・P21 と JWW です（[#3](https://github.com/zumen-jp/zumen/issues/3)・[#64](https://github.com/zumen-jp/zumen/issues/64)・[#171](https://github.com/zumen-jp/zumen/issues/171)・[#172](https://github.com/zumen-jp/zumen/issues/172)）。

- 呼び出し方: `zm_push` で入力を渡し、`open` の要求を送ります。コアは、コーデックが持つ判定の規則で形式を決めて読み、文書の名前と形式を返します。`capabilities` の `formats` に、読める形式が出ます。続けて `drawing` の要求を送ると、文書を図面モデル（L2）に変換し、空間と用紙ごとの単位・要素の数・範囲と、移さなかった図形の診断を返します。`entities` の要求を送ると、要素の一覧（ID・種類・レイヤ・出所）を、`offset` と `limit` で区切って返します（決定 0117）。`fields` で範囲（`bbox`）を含む返す欄を選び、`region` で範囲の重なる要素に絞ります（決定 0118）。`apply` の要求を送ると、`entities` が返す ID とハッシュで要素を指す変更を DXF・DWG に当てて書き戻し、書き戻したファイルを `take` で取り出せます（決定 0139）。変更は、スタイルの変更（`restyle`）・変換（`transform`）・除去（`remove`）と、形の値（`geometry`）での置き換え（`replace`）と追加（`add`）です。`entities` の `fields` に `geometry` を入れると、線分・円・円弧・楕円・折れ線・文字（2 点の間に合わせる文字を含む）・書式つきの文字・記号・スプライン（当てはめる点で決まるものを含む）の形の値を返します。`tables` の要求を送ると、レイヤ・線種・寸法スタイルの表の項目を ID・名前・ハッシュで返します（決定 0156）
- 対象の入力: `cases/open.json` の、試験の中で作る ASCII の DXF（SECTION・ENTITIES・EOF の 3 つのタグ）と、`cases/drawing.json` の、見出しの単位と用紙の範囲、モデル空間とペーパー空間の図形を持つ DXF と、`cases/entities.json` の、`drawing.json` と同じ DXF と、`cases/entities_e9.json` の、対応表の E9 の輪になる定義・定義のない配置・有限でない尺度・展開しない格子の配置を持つ DXF と、`cases/open_dwg.json` の、`zumen-acad` の試験と同じ合成の DWG と、`cases/open_sxf.json` の、試験の中で作るレイヤ・線分・用紙を持つ SFC と、同じ図面の P21 と、FILE_SCHEMA が SXF の図面のスキーマでない入力と、`cases/open_jww.json` の、試験の中で作る縮尺 1/100 のレイヤグループに線分を持つ JWW と、見出しが途中で切れた入力と、`cases/apply.json` の、ハンドルを持つ線と円の DXF と、`cases/convert.json` の、`open_sxf.json` と同じ SFC と `open_jww.json` と同じ JWW と、`cases/samples_dxf.json` の、公開サンプルの `samples/dxf/text_alignments.dxf` と、`cases/scene.json` の、シーンを作る合成の DXF と同じ公開サンプルと、`cases/scene_fonts.json` の、試験の中で組み立てた合成の TrueType で文字を字形にする DXF と、`cases/check.json` の、プロファイルで照合する合成の DXF と、`cases/validate.json` の、試験の中で書いた小さな EXPRESS と、それに照らす合成の IFC と、`cases/review.json` の、`check.json` と同じ DXF と、試験の中で書いたレビューの文書と、`cases/building.json` の、試験の中で書いた階と扉を持つ小さな IFC4 と、扉の属性を求める IDS と、IFC4 の一部を写した EXPRESS と、`cases/building_drawing.json` の、`building.json` と同じ IFC4 に図面の op を呼ぶ組と、`cases/bcf.json` の、`building.json` と同じ IFC4 と、試験の中で書いた BCF 3.0 と 2.1 の bcfzip と、レビューの文書です
- 止まる所: 入力の大きさは読む前に、作業は読むときにタグごと、表を作るときにレコードごと、変換で ENTITIES の図形ごとに、レコードはグループ・コード 0 のタグごとに数えます。止まった診断の `requested` は、その単位でそれまでに使った量です。同じ入力と予算なら、どの形でも同じ値になることを、`hashes.txt` で確かめます

いまの制限です。それぞれ、要るものと一緒に足します。

- `entities` の形の値は、一部の種類だけを返します。範囲は形の定義から求め、文字の字形と記号の中身を含みません（決定 0118）。空間か用紙を選ぶ `view` は、まだありません
- 停止の診断は、DXF のタグ・DXF の表・DWG のオブジェクトの表・図面モデルへの変換・`entities` で止めたとき、扱っていたレコードかバイト範囲を場所（`at`）に持ちます（決定 0119）。DWG の見出し・クラス・オブジェクトの位置の表・R2004 の入れ物で止めたときは、まだ持ちません
- 合成でない入力は、`cases/samples_dxf.json` の公開サンプル（ezdxf が書き出した DXF）だけです。AutoCAD・Jw_cad などで作った図面と、DXF のほかの形式の公開サンプルは、再配布できるものを確かめてから `samples/` に足します（決定 0171、#66）

## 試験の組の書き方

`cases/<名前>.json` に 1 件ずつ置きます。名前の順に流し、組ごとに新しい形（セッション）で始めます。入力は試験の中で作る合成のものか、`samples/` の公開サンプル（[決定 0171](../../docs/decisions/0171-public-samples-in-git.md)）にし、受け取った図面は使いません。

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

組は `"module"` の欄を持てます（[決定 0221](../../docs/decisions/0221-wasm-modules.md)）。値はモジュール名か `all` で、`all` の組は、すべてのモジュールを入れた wasm でだけ流します。欄がなければ、分けたモジュールで流すとき、手順の入力の形式と op から、それを持つモジュールを選びます。

手順は、次のどれか 1 つの欄を持ちます。最初の手順が `session` でなければ、既定の設定でセッションを作ってから始めます。

| 欄 | 中身 |
| --- | --- |
| `session` | セッションを作り直す。`options`（JSON の値）か `options_text`（文字列）で設定を渡す。どちらもなければ既定の設定 |
| `push` | 入力を足す。名前は `name`（文字列）か `name_hex`、中身は `text`（UTF-8）か `hex` か `file`。`file` は、リポジトリの根から書いた `samples/` の中の公開サンプルの道筋（決定 0171）。どれもなければ空 |
| `call` | 要求（JSON の値）を渡す |
| `call_text` | 要求を、文字列のまま渡す。読めない JSON を試すのに使う |
| `call_hex` | 要求を、16 進で書いたバイト列で渡す。UTF-8 でないものを試すのに使う |
| `take` | バイナリの結果を、番号で取り出す |

どの手順にも、次の欄を置けます。

| 欄 | 中身 |
| --- | --- |
| `status` | 期待する状態（`ok`・`failed`・`invalid`・`busy`・`panicked`）。書かなければ `ok` |
| `expect` | JSON Pointer と、期待する値。数は値で、オブジェクトはキーの順によらずに比べる |
| `absent` | 応答に、あってはいけない場所の JSON Pointer |

ハッシュの一覧の行は `<組の名前> <手順の番号> <response か blob> <ハッシュ>` です。応答とバイナリの結果を受け取った手順だけが、行を持ちます。

`call` の要求は、流すものがそれぞれの言語で JSON に直して渡します。キーの順や数の書き方が違っても、コアは同じ値として読むので、応答は同じバイト列になります。バイト列のまま渡したいときは、`call_text` か `call_hex` を使います。
