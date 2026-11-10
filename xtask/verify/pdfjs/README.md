# PDF の読みを pdf.js と照らす

PDF のページ寸法・文字の内容と位置・経路と画像の数を、外部の実装である pdf.js 6.5.438 と照らします（#519）。pdf.js は Apache-2.0 の別のプログラムとして呼び、zumen の依存や成果物には入れません。Node.js は 22.13.0 以降を使います。

```bash
npm ci --ignore-scripts --no-audit --no-fund --prefix xtask/verify/pdfjs
cargo test -p zumen-pdf --test external
```

`read.mjs` は標準入力の PDF を読み、結果を JSON で返します。外部のフォントや URL は取得せず、標準フォント・CMap・wasm は入れたパッケージから読みます。`--check` は起動を確かめ、pdf.js がなければ code 77 で終わります。Rust のテストは Node.js または pdf.js がなければ外部の照合だけを飛ばします。毎晩の CI では `ZUMEN_PDFJS_REQUIRED=1` を指定し、入っていなければ失敗にします。公開サンプルを読むテストは、外部のツールがなくても流れます。

## 比べること

- `getViewport` の寸法と、図面モデルの用紙の寸法。回転・クロップ・`UserUnit` を含みます。
- `getTextContent` の文字列と原点。pdf.js の表示の座標を左下原点・Y が上の pt に直し、zumen の mm と比べます。数の誤差は 0.0001 mm までとします。テストの文字は、両方が同じ数の文字列に分けるものです。
- `getOperatorList` の経路と画像の数。線は部分経路ごと、塗りは輪をまとめて数えます。表示しない OCG と OCMD は、pdf.js の `getOptionalContentConfig` に従って除きます。Type 3 の字形の中身は文字として扱い、注釈は数えません。

合成の入力は、線・曲線・塗りと線の両方・穴のある塗り・文字・画像・フォームと、回転した用紙の 2 ページです。公開サンプルは、PDFium の Type 3・OCG・ソフトマスク・LZW と JPX の 4 ファイルです。出所・変換内容・ライセンスは `samples/ledger.toml` にあります。

次の差は、両方の値と診断の件数を固定して確認します。差が増えても、そのまま通しません。

| 入力 | 差と根拠 | 追跡先 |
| --- | --- | --- |
| `pdfium-type3.pdf` | `ToUnicode` がなく、字形名は `C1`。pdf.js はコード 65 を `A` にし、zumen は U+FFFD と `pdf.glyph.dropped` 3 件を返す（PDF 32000-2 の 9.10.2）。原点は比べる | #520 |
| `pdfium-smask-blend.pdf` | pdf.js の演算の一覧はソフトマスク用の 2 経路も含むので 4、zumen は描画の 2 経路と `pdf.transparency.approximated` を返す（11.6.5） | #520 |

この比較はページを画像にして比べるものではありません。文字列の分け方が違う一般の PDF、切り抜き後の形、色・透明度・Type 3 の字形の見え方は、まだ比べていません。
