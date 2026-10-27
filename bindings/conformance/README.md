# 適合試験

言語に依存しない、境界の試験です（[決定 0012](../../docs/decisions/0012-boundary-functions-and-messages.md)）。同じ試験の組を、境界のどの形・どの言語の結合でも流し、次のことを確かめます。

- 状態と、応答の値（JSON Pointer で指す）が、期待どおりであること。期待は仕様から書きます
- 応答とバイナリの結果のハッシュ（SHA-256）の一覧が、`hashes.txt` と一致すること。どこでも同じ結果になることを確かめます（[決定 0007](../../docs/decisions/0007-single-rust-implementation.md)）

## 流し方

| 形 | 流すもの | コマンド |
| --- | --- | --- |
| Rust から直接 | `tests/conformance.rs` | `cargo test -p zumen-conformance` |
| C ABI（プロセスの中） | `tests/conformance.rs` | 同上 |
| wasm | `wasm.mjs`（Node.js 22 以降） | `cargo build -p zumen-abi --target wasm32-unknown-unknown --release` のあと、`node bindings/conformance/wasm.mjs target/wasm32-unknown-unknown/release/zumen_abi1.wasm` |

各言語の結合を足すときは、その言語で同じ組を流すものを足し、CI に入れます（[決定 0006](../../docs/decisions/0006-language-tiers.md) の「適合試験に通る」）。

## ハッシュの一覧を作り直す

応答を変えたときは、一覧を作り直し、変わった理由を PR に書きます。一覧は退行を見つけるためのもので、正しさの根拠にはしません。

```bash
cargo run -p zumen-conformance -- --write
```

`capabilities` の応答はパッケージの版とスキーマの小版を含むので、版を上げたときにも作り直します。

## 形式の入力

[#2](https://github.com/zumen-jp/zumen/issues/2) の出力の一致と [#8](https://github.com/zumen-jp/zumen/issues/8) の停止位置の一致は、形式を読む組で確かめます。いまの対象は DXF と DWG（R2000）です（[#3](https://github.com/zumen-jp/zumen/issues/3)・[#64](https://github.com/zumen-jp/zumen/issues/64)）。

- 呼び出し方: `zm_push` で入力を渡し、`open` の要求を送ります。核は、コーデックが持つ判定の規則で形式を決めて読み、文書の名前と形式を返します。`capabilities` の `formats` に、読める形式が出ます。続けて `drawing` の要求を送ると、文書を図面モデル（L2）に変換し、空間と用紙ごとの単位・要素の数・範囲と、移さなかった図形の診断を返します。`entities` の要求を送ると、要素の一覧（ID・種類・レイヤ・出所）を、`offset` と `limit` で区切って返します（決定 0117）。`fields` で範囲（`bbox`）を含む返す欄を選び、`region` で範囲の重なる要素に絞ります（決定 0118）
- 対象の入力: `cases/open.json` の、試験の中で作る ASCII の DXF（SECTION・ENTITIES・EOF の 3 つのタグ）と、`cases/drawing.json` の、見出しの単位と用紙の範囲、モデル空間とペーパー空間の図形を持つ DXF と、`cases/entities.json` の、`drawing.json` と同じ DXF と、`cases/open_dwg.json` の、`zumen-acad` の試験と同じ合成の DWG です
- 止まる所: 入力の大きさは読む前に、作業は読むときにタグごと、表を作るときにレコードごと、変換で ENTITIES の図形ごとに、レコードはグループ・コード 0 のタグごとに数えます。止まった診断の `requested` は、その単位でそれまでに使った量です。同じ入力と予算なら、どの形でも同じ値になることを、`hashes.txt` で確かめます

いまの制限です。それぞれ、要るものと一緒に足します。

- `entities` の応答は、形の中身をまだ返しません。範囲は形の定義から求め、文字の字形と記号の中身を含みません（決定 0118）。空間か用紙を選ぶ `view` は、まだありません
- 停止の診断は、止まったレコードやバイトの位置を持ちません。位置は `requested` の量から読み取ります
- 入力は合成の小さなものだけです。実際の図面に近い入力は、再配布できるものを確かめてから足します（#66）

## 試験の組の書き方

`cases/<名前>.json` に 1 件ずつ置きます。名前の順に流し、組ごとに新しい形（セッション）で始めます。入力は試験の中で作る合成のものにし、受け取った図面は使いません。

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

手順は、次のどれか 1 つの欄を持ちます。最初の手順が `session` でなければ、既定の設定でセッションを作ってから始めます。

| 欄 | 中身 |
| --- | --- |
| `session` | セッションを作り直す。`options`（JSON の値）か `options_text`（文字列）で設定を渡す。どちらもなければ既定の設定 |
| `push` | 入力を足す。名前は `name`（文字列）か `name_hex`、中身は `text`（UTF-8）か `hex`。どちらもなければ空 |
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

`call` の要求は、流すものがそれぞれの言語で JSON に直して渡します。キーの順や数の書き方が違っても、核は同じ値として読むので、応答は同じバイト列になります。バイト列のまま渡したいときは、`call_text` か `call_hex` を使います。
