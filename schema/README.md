# 境界のスキーマ

各言語への境界（[決定 0012](../docs/decisions/0012-boundary-functions-and-messages.md)）の定義です。ここを元に、`cargo xtask codegen` が各言語の型と C のヘッダーを作ります。

| ファイル | 中身 | 生成するもの |
| --- | --- | --- |
| `boundary.toml` | 版（ABI・メッセージのスキーマ）、状態コード、関数とその引数 | `bindings/c/include/zumen.h`、`crates/zumen-abi/src/abi.rs` |
| `messages.json` | 要求・応答・op ごとの欄・診断・セッションの設定・カタログの形（JSON Schema 2020-12） | `crates/zumen-abi/src/messages.rs` |

生成したファイルもコミットします。CI の `cargo xtask codegen --check` が、元と合わないものを見つけます。

## 版

- **ABI**（`versions.abi`）は、関数の引数か意味を変えるときに上げます。上げないことを目指します（[決定 0015](../docs/decisions/0015-add-only-boundary-and-version-floors.md)）。
- **メッセージのスキーマ**は、大版（`schema-major`、要求と応答の `v`）と小版（`schema-minor`、`capabilities` が返す）に分けます。op・欄・列挙の値を足すたびに、小版を上げます。
- 関数・状態コード・op・欄・列挙の値は、足すだけにします。消さず、名前と意味を変えません。パッケージの 1.0 より前は、壊したことをリリースノートに書いたうえで、小版で壊すことがあります。

## op を足す

1. `messages.json` に、`<op>_request` と `<op>_result` を足します。
   - `<op>_request` は `allOf` で `request_common` を取り込み、`op` の欄を `const` にします。
   - `request` の `oneOf` に、`<op>_request` を足します。
2. `boundary.toml` の `schema-minor` を上げます。
3. `cargo xtask codegen` を実行し、`crates/zumen-abi` に op の処理を書きます。
4. `bindings/conformance/cases/` に試験の組を足し、ハッシュの一覧を作り直します（[bindings/conformance/README.md](../bindings/conformance/README.md)）。
5. 新しい診断の code を使うなら、`catalog/` も直します（[catalog/README.md](../catalog/README.md)）。

op と欄の説明（`description`）は日本語で書きます。この説明が、生成した型のコメントと、AI 向けの道具の説明になります。説明のない定義と欄は、生成器が受け付けません。

## 使える書き方

JSON Schema のうち、どの言語の型にも素直に写せる部分だけを使います（[決定 0012](../docs/decisions/0012-boundary-functions-and-messages.md) と [決定 0016](../docs/decisions/0016-boundary-message-details.md)）。生成器は、これ以外の書き方と、知らないキーワードを受け付けません。

| 書き方 | 使う所 | Rust の型 |
| --- | --- | --- |
| `type: object`、`properties`、`required`、`additionalProperties: false` | `$defs` の定義 | 構造体。書かなくてよい欄は `Option` |
| 上と同じで `additionalProperties` を書かないもの | `allOf` で取り込まれるだけの定義（`request_common`） | 構造体。取り込む側の欄になる |
| `allOf: [{"$ref": …}]` と `unevaluatedProperties: false` | 共通の欄を取り込む定義（`<op>_request`） | 取り込んだ定義を、同じ名前の欄に持つ構造体 |
| `type: string` と `enum` | `$defs` の定義 | 列挙 |
| `oneOf` の中がすべて `{"$ref": …}` で、どれも同じ名前の `const` の欄を持つオブジェクト | タグつきの共用体（`request`） | 列挙。形ごとに 1 つ |
| `oneOf` の中が `integer`・`number`・`string`・文字列の `const` で、JSON の値から 1 つに決まるもの | スカラーの共用体（`limit`・`param_value`） | 列挙 |
| `type: string`・`boolean`・`number` | 欄 | `String`・`bool`・`f64` |
| `type: integer`、`minimum`・`maximum` | 欄 | 下限が 0 以上なら `u64`、そうでなければ `i64` |
| `type: array` と `items` | 欄 | `Vec` |
| `type: object` と、値の型を書いた `additionalProperties` | 欄（名前から値への表） | `BTreeMap<String, …>` |
| `$ref: "#/$defs/<名前>"` | 欄 | 指した定義の型 |
| `const`（文字列） | 共用体の形の目印の欄 | 欄にはしない |

- 定義・欄・列挙の値の名前は、英小文字・数字・`_` にします。
- オブジェクト・列挙・共用体は、欄の中に直接書かず、`$defs` に名前をつけて `$ref` で指します。
- 整数は ±(2^53 − 1) の中に限ります。`maximum` を書かなければ、2^53 − 1 が上限になります。
- `response` は、核が手で書きます（`result` の形が op ごとに違うため）。スキーマには、文書として書きます。

## メッセージの JSON

要求と応答の JSON の読み方と書き方は、[決定 0016](../docs/decisions/0016-boundary-message-details.md) で決めています。要点は次のとおりです。

- **読む。** UTF-8 の JSON（RFC 8259）だけを読みます。先頭の BOM は読み飛ばします。キーが重なるもの、入れ子が 64 より深いもの、値のあとに空白以外が続くものは、`call.json.invalid` にします。
- **整数。** `1.0` のように小数部が 0 だけの表記も、整数として読みます。指数のある表記は、整数として読みません。
- **知らない欄。** 要求に知らない欄があれば `call.field.unsupported`、必要な欄がなければ `call.field.missing`、型か範囲が合わなければ `call.value.invalid` にします。場所は JSON Pointer で返します。
- **書く。** キーはスキーマの順に並べ、空白を入れません。浮動小数は、ECMAScript の `Number::toString` と同じ表記で書きます。どの環境でも、同じバイト列になります。
