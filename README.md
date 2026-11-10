# zumen

日本の建設のデータ（図面・BIM・土木・点群・納品）を読み書きする、寛容なライセンスのライブラリ群です。Rust の 1 つのコアを、ブラウザー・サーバー・各言語から同じ結果で使います。ファイルは外へ出さずに読みます。

> 状態: 開発中で、最初の公開（0.2.0）を準備しています。パッケージはまだ公開していません（[#317](https://github.com/zumen-jp/zumen/issues/317)）。このリポジトリからビルドすると、CLI・各言語のバインディング・MCP のサーバーで試せます。1.0 までは、壊す変更をリリースノートに書いたうえで、マイナーバージョンで API を変えることがあります。PDF を読みたい場合や、パッケージで今すぐ使いたい場合は、旧版の [zumen-jp/zumen-legacy](https://github.com/zumen-jp/zumen-legacy) を使ってください。

## できること

`main` で動くものです。確かめた度合いは形式ごとに違います。実物のファイルとほかの実装で照らしたものも、テストの中で作ったファイルだけで確かめたものもあります。

| 領域 | 形式 | できること |
| --- | --- | --- |
| 図面 | SXF（SFC・P21・SFZ・P2Z）、JWW、DXF、DWG | 要素・レイヤ・線種・用紙を読む。変更を当てて、同じ形式に書き戻す。DXF に変換する。表示のための 2D のシーンを作る。CAD製図基準などのプロファイルの規則でチェックする |
| BIM | IFC（IFC-SPF・ifcZIP）、IDS、BCF | IFC を建物モデルとして読み、変更を当てて書き戻す。IDS で照合する。BCF を読み書きする |
| 土木・地理 | J-LandXML、CityGML（PLATEAU）、基盤地図情報、拡張 DM | 線形の上の座標・高さ・勾配を求める。平面図・縦断図・横断図を作る。地物を返す |
| 点群 | LAS、PLY、XYZ、E57 | 読む。設計の面との標高の差を、規格値と比べて集計する |
| 納品 | 電子納品の成果品、工事写真（JPEG）、表（xlsx・CSV）、ボーリング交換用データ | 管理ファイルを読み、プロファイルの規則でチェックする。Exif と XMP、セルの値、地層と標準貫入試験の値を返す |

- PDF・JWC・JWS・LAZ は、まだ読めません。LAZ は、展開を任せる laz の直した版が出るまで、展開を止めています（[#510](https://github.com/zumen-jp/zumen/issues/510)）。
- 読めなかったもの・形を変えて書いたものは、診断（code・件数・場所）で返します。
- 形式ごとの権利と、実装を置く場所の方針は [形式と権利の一覧](docs/formats/README.md) にあります。公開に第三者の許諾が要る形式（ST-Bridge など）は、外部のプラグインにします。

## 試す

Rust を [rustup](https://rustup.rs/) で入れ（バージョンは `rust-toolchain.toml` で決めてあります）、CLI をビルドして、公開サンプルを開きます。

```bash
cargo build -p zumen-cli
target/debug/zumen capabilities
target/debug/zumen entities samples/dwg/zumen-layout-r2000.dwg --limit 2
```

応答は JSON です。読めなかったものは `diagnostics` に入ります。要求と応答の形は [schema/messages.json](schema/messages.json) に、CLI の使い方は [bindings/cli](bindings/cli/README.md) にあります。

## 使える言語

どの言語も、同じ API（数個の関数と、[スキーマ](schema/README.md)で定めた JSON のメッセージ）を通ります。同じ入力から、ビット単位で同じ出力を返すことを、[適合テスト](bindings/conformance/README.md)で確かめます。

| Tier | 言語 | 使い方 |
| --- | --- | --- |
| 1 | Rust | [crates/zumen](crates/zumen) |
| 1 | TypeScript・JavaScript（ブラウザー・Node.js） | [bindings/js](bindings/js/README.md) |
| 1 | Python | [bindings/python](bindings/python/README.md) |
| 1 | .NET | [bindings/dotnet](bindings/dotnet/README.md) |
| 1 | C・C++ | [bindings/c](bindings/c/README.md) |
| 2 | Java・Kotlin | [bindings/java](bindings/java/README.md) |
| 2 | Swift | [bindings/swift](bindings/swift/README.md) |
| 2 | Go | [bindings/go](bindings/go/README.md) |
| 2 | Dart | [bindings/dart](bindings/dart/README.md) |
| 2 | VBA | C の DLL を `Declare` で呼ぶ（[bindings/c](bindings/c/README.md)） |
| 3 | Ruby・PHP・PowerShell など | C ABI・CLI・HTTP で呼ぶ（[docs/tier3.md](docs/tier3.md)） |

- Tier は、言語ごとの対応の約束です（[決定 0006](docs/decisions/0006-language-tiers.md)）。下限と、確かめる頻度は [docs/support.md](docs/support.md) にあります。
- CLI の `zumen` は、同じ要求を標準入出力と HTTP で受け、MCP のサーバーにもなります。AI のアプリからも使えます。

## 方針

- **どこでも同じ結果。** ブラウザー・サーバー・Python・.NET などで、同じファイルから同じ結果を返します。
- **黙って捨てない。** 読めなかったもの、形を変えて書いたものは、種類と件数と場所を報告します。
- **ID は契約。** 同じバイト列からは、どのバージョンでも同じ ID を返します。
- **受け取ったファイルを信用しない。** 大きさ・数・作業量に上限を付けて読みます。
- **日本の実務を優先する。** 電子納品の SXF、Jw_cad、日本語の文字、国土交通省の基準を先に扱います。

## 文書

| 文書 | 中身 |
| --- | --- |
| [docs/formats](docs/formats/README.md) | 形式と権利の一覧（台帳から生成）と、形式ごとの知見 |
| [docs/support.md](docs/support.md) | 対応する環境と、言語の下限 |
| [docs/tier3.md](docs/tier3.md) | 公式のラッパーのない言語から使う手順 |
| [docs/view2d.md](docs/view2d.md) | 2D の表示部品の方針と、旧版からの置き換えの対応表 |
| [docs/render.md](docs/render.md) | 図面から SVG・PNG を作る API と、Node.js の例 |
| [schema](schema/README.md) | API の関数とメッセージのスキーマ |
| [catalog](catalog/README.md) | 診断の code と、日本語・英語の文 |
| [docs/decisions](docs/decisions/README.md) | 決定の記録 |
| [#1](https://github.com/zumen-jp/zumen/issues/1) | 設計と計画（子 issue に分けている） |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 開発への参加。エージェント向けの規則は [AGENTS.md](AGENTS.md) |
| [docs/style.md](docs/style.md) | 表記と用語の基準 |
| [SECURITY.md](SECURITY.md) | 脆弱性の知らせ方 |

サイトは <https://zumen.dev> に置きます（準備中。元は [site](site/README.md)）。

## ライセンス

次のどちらかを選んで使えます。

- MIT License（[LICENSE-MIT](LICENSE-MIT)）
- Apache License 2.0（[LICENSE-APACHE](LICENSE-APACHE)）

`samples/` の公開サンプルは、[samples/ledger.toml](samples/ledger.toml) に書いたそれぞれのライセンスに従います。

明示しない限り、このリポジトリへの貢献は、Apache License 2.0 の定める「貢献」（Contribution）として、追加の条件なしに上の 2 つのライセンスで提供されたものとします。
