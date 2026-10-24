# 0016: 境界のメッセージの JSON を自前で読み書きし、スキーマの書き方と適合試験の比べ方を足す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- [0012](0012-boundary-functions-and-messages.md) で、境界を 9 つの関数と、JSON Schema で定めたメッセージにした。メッセージは決まったバイト列にし、要求の知らない欄は失敗にし、整数は ±(2^53 − 1) の中に限り、浮動小数は「読み直すと同じ値に戻る最短の表記」で書く。スキーマは、型に素直に写せる部分だけを使う。
- 境界の crate の最初の版（[#13](https://github.com/zumen-jp/zumen/issues/13) の完了の条件）を作るには、0012 が決めていない細部を決める必要があった。
  - JSON を何で読み書きするか
  - 「最短の表記」が、具体的にどの表記か
  - 0012 の部分集合では書けないもの: すべての op に共通の欄、予算の `"unlimited"`（整数か文字列）、診断の値（名前から値への表）、カタログの文（code から文への表）
  - 要求が JSON として読めないときや、知らない欄があるときの状態
  - `describe` に渡すカタログのファイルの形
  - 環境の間で、ハッシュの一覧をどう比べるか
- 2026-10-24 に確かめたこと
  - serde_json は、NaN を `null` と書く。`#[serde(flatten)]` と `deny_unknown_fields` は一緒に使えない。失敗の理由は英語の文で、どの欄かは別の crate（serde_path_to_error）がないと分からない。
  - Rust の `{:e}` は最短の桁を返すが、同じ桁数の候補の真ん中にある値で、ECMAScript と違う桁を選ぶ。無作為の浮動小数 20 万件のうち 183 件で、Node.js 24 の `String(x)` と違った。偶数の桁を選ぶようにすると、100 万件のすべてで一致した。

## 選択肢

### JSON の読み書き

1. **serde_json と serde の derive。** 広く使われている。ただし、上の振る舞いを補う手当てが、生成するコードのあちこちに要る。
2. **自前の小さな読み書き（`zumen-json`）。** 厳しさ・失敗の場所・数の書き方を、すべて自分で決められる。読み書きと試験を自分で保守する。

### 浮動小数の表記

1. **Rust の `{}`。** 指数を使わないので、1e300 は 301 桁になる。
2. **Rust の `{:e}` の桁を、そのまま並べる。** 決まった結果にはなるが、表記を説明できる公開の規則がない。
3. **ECMAScript の `Number::toString`。** 公開の規則で、JavaScript の `JSON.stringify` と同じ表記になる。

### スキーマで書けないもの

1. **0012 の部分集合の中で書く。** 共通の欄は op ごとに繰り返し、`"unlimited"` や診断の値は、タグつきのオブジェクトにする。メッセージが長くなり、`"unlimited"` は 0012 の書き方から外れる。
2. **型に写せる書き方を 3 つ足す。** `allOf` で共通の欄を取り込む書き方、名前から値への表、JSON の値で見分けられるスカラーの共用体。

### 読めない要求の状態

1. **`ZM_INVALID`。** 0012 の表の「引数が使えない」に入れる。応答を書かないので、理由が分からない。
2. **`ZM_FAILED` と、理由の診断。** `ZM_INVALID` は、NULL や長さのような関数の引数と、`zm_push` の名前のように応答のない関数にだけ使う。

### ハッシュの一覧の比べ方

1. **環境ごとに一覧を作り、最後の job で比べる**（[0007](0007-single-rust-implementation.md) の書き方）。CI の job の間で、一覧を受け渡す仕組みが要る。
2. **一覧をコミットし、どの環境・どの形も、それと比べる。** すべてが同じ一覧と一致すれば、互いにも一致する。応答が変わると、PR の差分に現れる。

## 決定

JSON の読み書きは 2、浮動小数の表記は 3、スキーマは 2、状態は 2、一覧の比べ方は 2 を選ぶ。

### JSON を読む

- UTF-8 の JSON（RFC 8259）の値を 1 つだけ読む。先頭の BOM は読み飛ばす（RFC 8259 の 8.1）。
- 次のものは `call.json.invalid` にし、問題を見つけたバイトの位置を返す。
  - UTF-8 でないもの、文法に合わないもの、値のあとに空白以外が続くもの
  - オブジェクトのキーが重なるもの
  - 配列とオブジェクトの入れ子が 64 より深いもの
  - 対になっていないサロゲートの `\u` エスケープ
- 版 `v` を先に確かめ、違えば `call.version.unsupported` にする。版が違えば、ほかの欄の形も違いうるため。
- そのあと、スキーマで読む。合わない所は JSON Pointer で返す。
  - 知らない op: `call.op.unsupported`
  - 知らない欄: `call.field.unsupported`
  - 必要な欄がない: `call.field.missing`
  - 型か範囲が合わない: `call.value.invalid`。書かなくてよい欄の `null` も、これにする
- 整数の欄は、`1.0` のように小数部が 0 だけの表記も、整数として読む（JSON Schema の `integer` と同じ）。指数のある表記は、整数として読まない。

### JSON を書く

- キーは、スキーマの順に並べる。名前から値への表は、名前の順に並べる。空白は入れない。
- 文字列は、`"`・`\`・制御文字だけをエスケープし、ほかの文字は UTF-8 のまま書く。
- 浮動小数は、ECMAScript の `Number::toString`（基数 10）と同じ表記で書く。負の 0 は `0` になる。
- 診断の値のうち、2^53 − 1 を超える整数と、有限でない浮動小数（`NaN`・`Infinity`・`-Infinity`）は、文字列にする。

### スキーマに足す書き方

| 書き方 | 使う所 |
| --- | --- |
| `allOf: [{"$ref": …}]` と `unevaluatedProperties: false`。取り込まれる定義は `additionalProperties` を書かない | すべての op の要求に共通の欄（`request_common`） |
| `type: object` と、値の型を書いた `additionalProperties` | 診断の値（`params`）、カタログの文（`codes`） |
| `oneOf` の中が `integer`・`number`・`string`・文字列の `const` で、JSON の値から 1 つに決まるもの | 予算の上限（`limit`）、診断の値（`param_value`） |

- オブジェクト・列挙・共用体は、欄に直接書かず、`$defs` に名前をつけて指す。生成する型の名前を、定義の名前から決めるため。
- 定義と欄には、日本語の `description` を必ず書く。
- 生成器は、ここと 0012 にない書き方と、知らないキーワードを受け付けない。書き方の一覧は `schema/README.md` に置く。

### カタログのファイル

- `catalog/<言語>.toml` から、`catalog/<言語>.json` を生成してコミットする。形は `schema/messages.json` の `catalog`。核はすでに JSON を読むので、TOML を読むものを核に入れずに済む。
- 数の書き方（3 桁ごとの区切りと小数点の文字）を、カタログの `[number]` に置く（[0009](0009-diagnostics-and-catalog.md) の「カタログの規則」）。
- カタログがないか、カタログにない code は、`code (名前=値, …, count=件数)` と書く。

### 入力の名前

- 0012 の規則（`..`・絶対パス・ドライブ名・NUL を受け付けない）を、次のように決める。`zumen_base::name::is_safe` に置き、ZIP の項目の名前と共有する。
  - 空の部分（空の名前、先頭・末尾の `/`、続いた `/`）を受け付けない
  - `.` と `..` の部分を受け付けない
  - `\` と `:` を受け付けない

### 適合試験

- 試験の組は `bindings/conformance/cases/*.json`、ハッシュの一覧は `bindings/conformance/hashes.txt` に置く。書き方は、そこの `README.md` に置く。
- Rust から直接・C ABI・wasm で流し、どれも `hashes.txt` と一致することを、PR ごとに確かめる。
- 一覧は、退行を見つけるためのものとして扱う（[0007](0007-single-rust-implementation.md)）。作り直す PR には、応答が変わった理由を書く。

## 影響

- JSON の読み書き（`zumen-json`）を自分で保守する。読み書きの試験と、Node.js との数の表記の突き合わせを、変えるたびに流す。
- 浮動小数の表記で、同じ桁数の候補があるときに、10 進で書き切って確かめる。1 つの数に 1 µs ほどかかる。大きなデータは JSON ではなくバイナリで渡すので（[#6](https://github.com/zumen-jp/zumen/issues/6)）、問題にしない。
- 各言語の生成器は、足した 3 つの書き方を扱う必要がある。どれも、構造体の取り込み、表、列挙にそのまま写せる。
- `capabilities` の応答はパッケージの版を含むので、版を上げるたびに `hashes.txt` を作り直す。
- [0007](0007-single-rust-implementation.md) の「最後の job で、すべての一覧が一致することを確かめる」は、コミットした一覧との比較に置き換えた。
- この版の境界には、まだ次のものがない。それぞれ、要るものと一緒に足す。
  - 読める形式。`open` は、入力と予算を確かめてから `call.format.unsupported` を返す
  - 表を返す op。`zm_take_arrow` は、どの番号にも `ZM_INVALID` を返す
  - 実行の形の `process`。CLI と一緒に作る
- [#13](https://github.com/zumen-jp/zumen/issues/13) は、この記録と、境界のヘッダーとスキーマの最初の版で完了する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1)、[#13](https://github.com/zumen-jp/zumen/issues/13)
- [0007](0007-single-rust-implementation.md)、[0009](0009-diagnostics-and-catalog.md)、[0012](0012-boundary-functions-and-messages.md)、[0015](0015-add-only-boundary-and-version-floors.md)
- [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259)、[RFC 6901（JSON Pointer）](https://www.rfc-editor.org/rfc/rfc6901)、[JSON Schema 2020-12](https://json-schema.org/draft/2020-12)
- [ECMAScript の Number::toString](https://tc39.es/ecma262/#sec-numeric-types-number-tostring)
