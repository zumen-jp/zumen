# zumen

日本の建設のデータ（図面・BIM・土木・点群・納品）を読み書きする、寛容なライセンスのライブラリ群です。Rust の 1 つのコアを、ブラウザー・サーバー・各言語から同じ結果で使います。ファイルは外へ出さずに読みます。

> 状態: 作り直しを始めたところで、まだ図面は読めません。設計は [#1](https://github.com/zumen-jp/zumen/issues/1) にあり、決めることを子 issue に分けています。SXF・JWW・DXF・DWG・PDF を今すぐ読み書きしたい場合は、旧版の [zumen-jp/zumen-legacy](https://github.com/zumen-jp/zumen-legacy) を使ってください。

## 方針

- **どこでも同じ結果。** ブラウザー・サーバー・Python・.NET などで、同じファイルから同じ結果を返します。
- **黙って捨てない。** 読めなかったもの、形を変えて書いたものは、種類と件数と場所を報告します。
- **ID は契約。** 同じバイト列からは、どの版でも同じ ID を返します。
- **受け取ったファイルを信用しない。** 大きさ・数・作業量に上限を付けて読みます。
- **日本の実務を優先する。** 電子納品の SXF、Jw_cad、日本語の文字、国土交通省の基準を先に扱います。

## 文書

- 次の作業の選び方: [docs/work.md](docs/work.md)（計画は [#1](https://github.com/zumen-jp/zumen/issues/1) の下の issue）
- 設計: [#1](https://github.com/zumen-jp/zumen/issues/1)
- 決定の記録: [docs/decisions](docs/decisions/README.md)
- 形式と権利の一覧: [docs/formats](docs/formats/README.md)（台帳から生成）
- 各言語への境界のスキーマ: [schema](schema/README.md)（C のヘッダーは [bindings/c](bindings/c/README.md)、TypeScript・JavaScript の包みは [bindings/js](bindings/js/README.md)、適合試験は [bindings/conformance](bindings/conformance/README.md)）
- 対応する環境と下限: [docs/support.md](docs/support.md)
- 表記と用語の基準: [docs/style.md](docs/style.md)
- 開発への参加: [CONTRIBUTING.md](CONTRIBUTING.md)（エージェント向けの規則は [AGENTS.md](AGENTS.md)）
- 旧版: [zumen-jp/zumen-legacy](https://github.com/zumen-jp/zumen-legacy)
- サイト: <https://zumen.dev>（準備中。元は [site](site/README.md)）

## ライセンス

次のどちらかを選んで使えます。

- MIT License（[LICENSE-MIT](LICENSE-MIT)）
- Apache License 2.0（[LICENSE-APACHE](LICENSE-APACHE)）

明示しない限り、このリポジトリへの寄稿は、Apache License 2.0 の定める「寄稿」として、追加の条件なしに上の 2 つのライセンスで提供されたものとします。
