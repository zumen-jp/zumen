# zumen

日本の建設のデータ（図面・BIM・土木・点群・納品）を読み書きする、寛容なライセンスのライブラリ群です。Rust の 1 つの核を、ブラウザ・サーバー・各言語から同じ結果で使います。ファイルは外へ出さずに読みます。

> 状態: 作り直しを始めたところで、まだ動くコードはありません。設計は [#1](https://github.com/zumen-jp/zumen/issues/1) にあり、決めることを子 issue に分けています。SXF・JWW・DXF・DWG・PDF を今すぐ読み書きしたい場合は、旧版の [zumen-jp/zumen-legacy](https://github.com/zumen-jp/zumen-legacy) を使ってください。

## 方針

- **どこでも同じ結果。** ブラウザ・サーバー・Python・.NET などで、同じファイルから同じ結果を返します。
- **黙って捨てない。** 読めなかったもの、形を変えて書いたものは、種類と件数と場所を報告します。
- **ID は契約。** 同じバイト列からは、どの版でも同じ ID を返します。
- **受け取ったファイルを信用しない。** 大きさ・数・作業量に上限を付けて読みます。
- **日本の実務を優先する。** 電子納品の SXF、Jw_cad、日本語の文字、国土交通省の基準を先に扱います。

## 文書

- 設計: [#1](https://github.com/zumen-jp/zumen/issues/1)
- 旧版: [zumen-jp/zumen-legacy](https://github.com/zumen-jp/zumen-legacy)
- サイト: https://zumen.dev （準備中）

## ライセンス

次のどちらかを選んで使えます。

- MIT License（[LICENSE-MIT](LICENSE-MIT)）
- Apache License 2.0（[LICENSE-APACHE](LICENSE-APACHE)）

明示しない限り、このリポジトリへの寄稿は、Apache License 2.0 の定める「寄稿」として、追加の条件なしに上の 2 つのライセンスで提供されたものとします。
