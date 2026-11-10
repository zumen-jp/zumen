# XML の原本と文字コード

`zumen-xml` は L0 の XML リーダーです。形式の意味を解釈せず、要素・属性・文字・名前空間を読み、原本のバイト範囲を返します。外部の DTD と実体を取得しません。

## UTF-16 の受理条件

UTF-8・Shift_JIS に加え、UTF-16LE・UTF-16BE を読みます。UTF-16 のバイト順と XML 宣言は、[XML 1.0 4.3.3](https://www.w3.org/TR/xml/#charencoding)・[付録 F](https://www.w3.org/TR/xml/#sec-guessing) と [RFC 2781 3.3](https://www.rfc-editor.org/rfc/rfc2781.html#section-3.3) に従って、次の組を受け付けます。MIME の文字コード指定は入力に取りません。

| バイト列 | XML 宣言の encoding | 結果 |
| --- | --- | --- |
| LE・BE の BOM がある | 宣言なし、encoding なし、`UTF-16` | BOM の順で読む |
| BOM がない | 同じバイト順の `UTF-16LE`・`UTF-16BE` | 宣言の順で読む |
| BOM がある | `UTF-16LE`・`UTF-16BE` | `xml.document.invalid` |
| BOM がない | `UTF-16`、encoding なし | 読まない |
| UTF-16 と検出したバイト列 | 逆のバイト順、`UTF-8` など | `xml.document.invalid` |

BOM のない UTF-16 は、先頭の XML 宣言 `<?xml` から検出します。宣言のない BOM なしのバイト列は UTF-16 と判定しません。UTF-32 の BOM と先頭の並びは `xml.encoding.unsupported` で断ります。奇数バイト、孤立したサロゲート、XML 1.0 の Char に含まれない文字、不正な QName を UTF-16 の入力で受け取った場合は、置換せず `xml.document.invalid` で止めます。既存の UTF-8・Shift_JIS の受理条件は変更しません。

既存の Shift_JIS は、日本語の名前の 2 バイト目を区切りと取り違える場合があります。ボーリングの根の判定と合わせて [#824](https://github.com/zumen-jp/zumen/issues/824) で修正します。UTF-16 の追加で、この問題も解決したとはしません。

## 原本と解析用の名前

- `Document::src`・`bytes` と、名前の `raw`・`local`・`prefix`、要素・属性・文字・CDATA・DTD・診断の範囲は、すべて原本のバイト位置です。ファイルモデルの `RecordInfo.kind` も原本の名前を保ちます。
- `parsed_local_name` は構造の比較に使います。UTF-16 は解析用の UTF-8、UTF-8・Shift_JIS は従来の名前のバイト列を返します。ASCII の要素名は、このメソッドで比べます。
- `local_name_text` は文字列の名前を返します。文字・属性値は `text`・`value`、CDATA は `cdata` で復号します。原本の UTF-16 の名前を `from_utf8_lossy` へ渡しません。

UTF-16 の全文を厳密に UTF-8 にし、既存のパーサーで解析します。UTF-8 と UTF-16 の文字幅の組が続く範囲ごとに位置の対応表を作り、公開する範囲を原本の位置へ戻します。ASCII の文字ごとに表は作りません。

確保前に、復号後の UTF-8 のバイト数と、対応表の区間数 × 32 を `DecodedBytes` で予約します。2 回の走査はそれぞれ UTF-16 のコード単位ごと、名前の検査は解析用のバイト数、範囲の戻しは要素・属性・子・DTD ごとに `Work` を数えます。32 は予算の固定値で、実行環境の構造体のサイズに依存しません。元からある読み取りと領域モデルへの変換の予算も使います。

## Rust API の移行

`Encoding` に `Utf16Le`・`Utf16Be` と `#[non_exhaustive]` を追加します。既存の網羅的な `match` は、次のように残りの値を扱う必要があります。この変更を含む次の公開版は 0.3.0 とします。API の関数・メッセージのスキーマ、ABI、既存の ID の規則は変えません。

```rust
match document.encoding() {
    zumen_xml::Encoding::Utf8 => { /* UTF-8 の原本 */ }
    zumen_xml::Encoding::ShiftJis => { /* Shift_JIS の原本 */ }
    _ => { /* UTF-16 と、今後追加する文字コード */ }
}
```

## 確かめた範囲

[独立検査の手順](../../xtask/verify/xml/README.md)に、入力・比較する値・予算の境界と停止の検査を記載しています。自作の 39 入力について、XML と J-LandXML の公開値・原本の位置・診断・予算の記録を、ネイティブと WASI の各 2 回でバイト単位で比較します。既存の UTF-8・Shift_JIS は、変更前の main の固定値とも比べます。

Expat との照合は、受理済みの自作 10 入力の名前・属性・文字・CDATA と要素の原本位置に限ります。Expat は文字コードの宣言と BOM の組を広く受け付けるため、受理条件の比較先にはしません。J-LandXML の領域モデルの照合や、各形式の外部検証の段を上げる根拠には含めません。

配布する wasm と各言語の API 応答は、[UTF-16 の適合テスト](../../bindings/conformance/cases/xml_utf16.json)で別に比較します。共通の XML を使う各形式の自作の回帰テストも、それぞれの形式の文書に記載します。UTF-16 の読み取りを足しても、その形式のすべての文字コードで編集できるとはしません。[BCF](bcf.md) では、変更しない topic は原本を保ち、UTF-8 以外の markup の変更保存は診断して止めます。
