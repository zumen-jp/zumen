# API のスキーマ

各言語への API の定義です。ここを元に、`cargo xtask codegen` が各言語の型と C のヘッダーを作ります。

| ファイル | 中身 | 生成するもの |
| --- | --- | --- |
| `boundary.toml` | バージョン（ABI・メッセージのスキーマ）、状態コード、関数とその引数 | `bindings/c/include/zumen.h`、`crates/zumen-abi/src/abi.rs`、`bindings/js/src/constants.js`・`constants.d.ts`、`bindings/python/zumen/constants.py`、`bindings/dotnet/Zumen/Constants.g.cs` |
| `messages.json` | 要求・応答・op ごとのフィールド・診断・セッションの設定・カタログの形（JSON Schema 2020-12） | `crates/zumen-abi/src/messages.rs`、`bindings/js/src/messages.d.ts`、`bindings/python/zumen/messages.py`、`bindings/dotnet/Zumen/Messages.g.cs`、`bindings/cli/src/tools.json`（MCP のツール） |
| `scene.toml` | シーンの blob の配置（セクションと値の組） | `crates/zumen-scene/src/layout.rs`、`bindings/c/include/zumen_scene.h`、`bindings/js/src/constants.js`・`constants.d.ts`、`bindings/python/zumen/constants.py`、`bindings/dotnet/Zumen/Constants.g.cs` |
| `drawing.toml` | 図面モデル（L2）のバイナリの型の契約（型ごとのフィールドと値の並びと、バージョン）。生成器が図面モデルの型から書き足す | `crates/zumen-drawing/src/binary/generated.rs` |

生成したファイルもコミットします。CI の `cargo xtask codegen --check` が、元と合わないものを見つけます。

VBA7 向けには、`boundary.toml` と `scene.toml` から `bindings/vba/ZumenNative.bas` にネイティブの宣言・ABI の型・バージョン・状態コード・シーンの配置を生成します。メッセージは UTF-8 の JSON のまま受け渡します。所有権と型の対応は [VBA のバインディング](../bindings/vba/README.md) にあります。

## バージョン

- **ABI**（`versions.abi`）は、関数の引数か意味を変えるときに上げます。上げないことを目指します。
- **メッセージのスキーマ**は、メジャーバージョン（`schema-major`、要求と応答の `v`）とマイナーバージョン（`schema-minor`、`capabilities` が返す）に分けます。op・フィールド・列挙の値を足すたびに、マイナーバージョンを上げます。
- 関数・状態コード・op・フィールド・列挙の値は、足すだけにします。消さず、名前と意味を変えません。パッケージの 1.0 より前は、壊したことをリリースノートに書いたうえで、マイナーバージョンで壊すことがあります。

要求に足すフィールドは、省略すれば従来の動きになるようにします。意味を変えるには新しい名前を足し、廃止した名前も動かし続けます。ラッパーは未知の応答フィールドを無視し、未知の列挙値も受け取ります。新しい op を古いコアで使う前に `capabilities` を確認します。追加で表せずメジャーバージョンを上げる場合、要求の `v` で以前のメジャーバージョンも受け付け、いつまで受け付けるかをこの節に明記します。パッケージが 1.0 より前の破壊的変更では、上の例外に従い、理由・移行方法・バージョンをリリースノートと、コミットまたは PR に残します。

## ID と編集の前提条件

同じバイト列からは、どのバージョンでも同じ ID を返します。浮動小数の計算結果や並び替え後の添字から ID を作りません。図面の要素は出所のキーを使い、AutoCAD のハンドルは `h` と大文字の 16 進、SXF のインスタンス番号は `i` と 10 進で表します。キーがなければレコード・バイト位置・要素の位置を使い、同じ出所の 2 つ目以降は `/1`、`/2` と区別します。実寸の空間は `space/0` です。

編集の前提条件には ID と読んだときのハッシュを使います。文書のハッシュは元のバイト列の BLAKE3 の先頭 128 ビットです。要素のハッシュは出所の位置を含めず、決まった順で符号化した意味の値を使います。すべての変更を適用前に検査し、要素がなければ `change.element.missing`、値が変わっていれば `change.element.stale` を返します。ID の規則を変える場合は、実装に先立って契約と移行方法を文書にし、その差分を別の担当者が確認します。1.0 より前の API の例外も ID には適用しません。

建物の形を含む成分と Hash v1 の追加の符号化は、[IFC の所有する形とハッシュ](../docs/formats/ifc.md#所有する形とハッシュを建物に保持する)にあります。属性だけの取得と形を含む取得は明示的に選び、前提条件の検査でも同じ経路を使います。

## モデル空間の計測

`measure` は、`doc` と実寸の空間の ID `from` を選び、`quantity` に `length`・`perimeter`・`area`・`distance` を指定します。`first` は `{"kind":"element","id":"space/0/h10"}` のように公開 ID で要素を指します。距離のときだけ `second` が必要で、両方の対象に `{"kind":"point","position":{"x":0,"y":0}}` も使えます。用紙、記号定義の中、異なる空間をまたぐ計測は対象にしません。`expected_hash` があれば文書のハッシュと照合します。文書・要素・ID・ハッシュは変わりません。

結果の `value` は空間の単位、面積ならその 2 乗です。`unit` と `unit_power` に単位と次数を返し、単位不明なら `unknown` を保ちます。距離は経路の境界間で求め、距離を作る点を `nearest_first` と `nearest_second` に返します。閉じた経路の内部や線幅、表示変換は含めません。周長と面積には閉じた経路を求めます。面積は向き付き線積分の絶対値で、自己交差を塗り規則で統合した面積ではありません。

線分・折れ線の直線と円弧・円・円弧は元の定義を使います。楕円の全面積とクロソイドの長さも解析的に求めます。ほかの楕円、スプライン、クロソイドの距離は折れ線で近似し、`approximate: true` と、要素の ID・計測の種類・出所・件数を持つ `drawing.element-measurement.approximated` を返します。`tolerance` は正の有限値で、既定は空間の単位で `0.001` です。曲線を分割する弦の誤差の目標であり、計測値の誤差上限の保証ではありません。要求に使った値を結果にも返します。

折れ線と分割数の上限は 65,536、スプラインの制御点・フィット点は各 4,096、次数は 32 です。作業量と確保量を先に数え、作った点は `segments`、距離の組み合わせは `work` で数えます。近似の上限、未対応の形、閉じていない経路、数値の範囲外などは `drawing.element-measurement.failed` に理由と元の要素を残し、計測の部分結果を返しません。予算停止後も文書を再利用できます。期待値と環境間の応答の一致は[計測の適合テスト](../bindings/conformance/README.md#計測)で確認します。

## 3D のシーンの契約

建物のシーンは [#421](https://github.com/zumen-jp/zumen/issues/421)、点群のセクションは [#475](https://github.com/zumen-jp/zumen/issues/475) の実装対象です。次の配置を共用し、実装時に `schema/scene3d.toml` から各言語の定数と型を生成します。

- ヘッダーは `ZMS3`、メジャー・マイナーバージョン（各 u16）、塊の数・セクションの数（各 u32）。セクション表は開始位置と長さ（各 u32）を持つ。リトルエンディアンで、セクションは 8 バイト境界に置く。未知のセクションは読み飛ばす。開始位置の列と「なし」（u32 の最大値）の扱いは [2D の配置](scene.toml) と同じ。
- メッシュの頂点は局所座標 f32、三角形と稜線は頂点番号 u32。法線は面から求め、稜線には三角形の分け目を入れない。配置はメッシュ・要素・色の番号と、塊の原点からの変換（f32 の 3 行 4 列）を持つ。写像では同じメッシュを使い回す。
- 要素は ID・クラス・名前を UTF-8 の文字列の開始位置と長さで参照し、階を要素番号で持つ。色は f32 の RGBA と未解決のフラグ。各塊は f64 の原点、f32 の相対座標の範囲、インスタンスの開始位置を持つ。同じ要素を複数の塊に分けない。4096 インスタンスは計測までの仮の値。
- `scene3d` は文書に対して `storeys`・`classes`・`ids` のすべてに合う要素を選ぶ。断面の `box` は重なりで絞り、切り抜きは読み手が行う。座標はプロジェクト空間の文書単位。許容差の既定は対象範囲の対角線の 1/50000 とし、使った許容差は応答と `approximated` の診断に残す。応答には文書の単位と地図への変換、指定された断面の箱、範囲、メッシュ・インスタンス・要素の数、blob の番号を記録する。作った点は `segments`、評価した要素は `work` で数える。blob は `blobs` の `scene3d` で返し、`zm_take` で受け取る。
- シーンごとに細かさを決め、粗いシーンは許容差を変えて作る。評価できない要素も ID と診断を残す。建物から切り出した 2D の図面とは同じ建物要素の ID で照合する。塊の大きさと LOD の段はサンプルで計測して決める。

点群のチャンクは八分木のノードと対応し、点の開始位置、段と 3 軸の位置で表すノード番号を持ちます。点は f64 のチャンク原点からの f32 の位置と、持っている属性の列（色は u8 の RGBA、強度は u16、分類は u8）で返します。色がなければ読み手が強度か分類から選びます。点群の文書にも `scene3d` を使い、`box`・`tolerance` と件数の上限 `limit` で絞ります。

## op を足す

1. `messages.json` に、`<op>_request` と `<op>_result` を足します。
   - `<op>_request` は `allOf` で `request_common` を取り込み、`op` のフィールドを `const` にします。
   - `request` の `oneOf` に、`<op>_request` を足します。
2. `boundary.toml` の `schema-minor` を上げます。
3. `cargo xtask codegen` を実行し、`crates/zumen-abi` に op の処理を書きます。
4. `bindings/conformance/cases/` にテストの組を足し、ハッシュの一覧を作り直します（[bindings/conformance/README.md](../bindings/conformance/README.md)）。
5. 新しい診断の code を使うなら、`catalog/` も直します（[catalog/README.md](../catalog/README.md)）。

op とフィールドの説明（`description`）は日本語で書きます。この説明が、生成した型のコメントと、AI 向けのツールの説明になります。説明のない定義とフィールドは、生成器が受け付けません。

## 使える書き方

JSON Schema のうち、どの言語の型にも素直に写せる部分だけを使います。生成器は、これ以外の書き方と、知らないキーワードを受け付けません。

| 書き方 | 使う所 | Rust の型 |
| --- | --- | --- |
| `type: object`、`properties`、`required`、`additionalProperties: false` | `$defs` の定義 | 構造体。書かなくてよいフィールドは `Option` |
| 上と同じで `additionalProperties` を書かないもの | `allOf` で取り込まれるだけの定義（`request_common`） | 構造体。取り込む側のフィールドになる |
| `allOf: [{"$ref": …}]` と `unevaluatedProperties: false` | 共通のフィールドを取り込む定義（`<op>_request`） | 取り込んだ定義を、同じ名前のフィールドに持つ構造体 |
| `type: string` と `enum` | `$defs` の定義 | 列挙 |
| `oneOf` の中がすべて `{"$ref": …}` で、どれも同じ名前の `const` のフィールドを持つオブジェクト | タグつきの共用体（`request`） | 列挙。形ごとに 1 つ |
| `oneOf` の中が `integer`・`number`・`string`・文字列の `const` で、JSON の値から 1 つに決まるもの | スカラーの共用体（`limit`・`param_value`） | 列挙 |
| `type: string`・`boolean`・`number` | フィールド | `String`・`bool`・`f64` |
| `type: integer`、`minimum`・`maximum` | フィールド | 下限が 0 以上なら `u64`、そうでなければ `i64` |
| `type: array` と `items` | フィールド | `Vec` |
| `type: object` と、値の型を書いた `additionalProperties` | フィールド（名前から値への表） | `BTreeMap<String, …>` |
| `$ref: "#/$defs/<名前>"` | フィールド | 指した定義の型 |
| `const`（文字列） | 共用体の形のタグのフィールド | フィールドにはしない |

- 定義・フィールド・列挙の値の名前は、英小文字・数字・`_` にします。
- オブジェクト・列挙・共用体は、フィールドの中に直接書かず、`$defs` に名前をつけて `$ref` で指します。
- 整数は ±(2^53 − 1) の中に限ります。`maximum` を書かなければ、2^53 − 1 が上限になります。
- `response` は、コアが手で書きます（`result` の形が op ごとに違うため）。スキーマには、文書として書きます。

## メッセージの JSON

要求と応答の JSON は次の規則で読み書きします。

- **読む。** UTF-8 の JSON（RFC 8259）だけを読みます。先頭の BOM は読み飛ばします。キーが重なるもの、入れ子が 64 より深いもの、値のあとに空白以外が続くものは、`call.json.invalid` にします。
- **整数。** `1.0` のように小数部が 0 だけの表記も、整数として読みます。指数のある表記は、整数として読みません。
- **知らないフィールド。** 要求に知らないフィールドがあれば `call.field.unsupported`、必要なフィールドがなければ `call.field.missing`、型か範囲が合わなければ `call.value.invalid` にします。場所は JSON Pointer で返します。
- **書く。** キーはスキーマの順に並べ、空白を入れません。浮動小数は、ECMAScript の `Number::toString` と同じ表記で書きます。どの環境でも、同じバイト列になります。
