# 松浦市の SFC で測った用紙ビューと実寸の空間

松浦市建設課の地形図（縮尺 1:15,000、CC BY 4.0）の図郭 1（17,975,570 バイト）と図郭 2（57,227,256 バイト）を使いました。いずれも用紙が 1 枚、用紙の中のビューが 1 件、実寸の空間が 1 件あります。図面は配布元から取得してリポジトリの外に置きます。[松浦市のデータセット](https://data.bodik.jp/dataset/422088_15000_sfc)・[市の利用案内](https://www.city-matsuura.jp/top/soshikikarasagasu/kensetsuka/dorokasengakari/1/1/7590.html)

## シーンの生成

2026-11-21、Windows x64 のネイティブと wasm32 で公開 API の `open`・`drawing`・`scene` を実行しました。フォントの指定は省きました。実寸の空間と用紙について、それぞれ既定の許容差と 0.01 mm で計 8 組を照合しました。応答とシーンの全バイトが一致しました。

| 図郭 | 対象 | 許容差（mm） | 塊 | 頂点 | シーン（バイト） |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | 用紙 | 既定 0.02 | 11 | 519,844 | 6,663,872 |
| 1 | 用紙 | 0.01 | 62 | 532,932 | 6,771,208 |
| 1 | 実寸 | 既定 0.304926 | 14 | 526,524 | 6,797,088 |
| 1 | 実寸 | 0.01 | 5,977 | 665,870 | 8,221,952 |
| 2 | 用紙 | 既定 0.02 | 35 | 1,674,654 | 21,418,368 |
| 2 | 用紙 | 0.01 | 592 | 1,697,690 | 21,631,600 |
| 2 | 実寸 | 既定 0.36318 | 45 | 1,704,811 | 21,990,616 |
| 2 | 実寸 | 0.01 | 18,010 | 1,933,076 | 24,750,896 |

どちらの図郭も `open`・`drawing` の診断は 0 件でした。`scene` は 8 組それぞれで `scene.curve.approximated` を 1 件報告しました。シーンに含まれる文字要素は図郭 1 が 655 件、図郭 2 が 1,536 件です。

## 日本語フォントを指定した用紙

用紙の既定の許容差で、静的な Noto Sans JP Regular 2.004 を `fonts` に渡しました。指定を省くと両方のシーンの字形数は 0 です。指定するとすべての文字要素に字形が付き、2 図郭とも応答とシーンの全バイトがネイティブと wasm で一致しました。表の「日本語あり」は漢字・ひらがな・カタカナを含む文字要素の数です。[Noto CJK の日本語フォント](https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/SubsetOTF/JP/NotoSansJP-Regular.otf)・[SIL Open Font License 1.1](https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/LICENSE)

| 図郭 | 字形が付いた文字要素 | うち日本語あり | 配置した字形 | 字形の形の数 | 輪郭の輪 | 輪郭の頂点 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 655 | 462 | 1,202 | 187 | 753 | 9,888 |
| 2 | 1,536 | 1,123 | 2,765 | 332 | 1,300 | 17,692 |

フォントを指定した場合の各用紙では `scene.font.assumed` が 2 組あり、どちらの組の件数も文字要素数と一致しました。字形の欠落を示す `scene.glyph.missing` は 0 件でした。

## 再現手順

配布ページの SFC の図郭 1 と 2 を取得します。次の照合器は各入力の SHA-256 を照らし、シーンごとに許容差・件数・バイト数・SHA-256・診断を出力します。ビルド先や図面の保存先が異なる場合はコマンドの引数に指定します。

```sh
cargo build --locked -p zumen-cli
cargo build --locked -p zumen-abi --release --target wasm32-unknown-unknown --no-default-features --features drawing
node bench/chunks/compare-sfc.mjs target/debug/zumen target/wasm32-unknown-unknown/release/zumen_abi1.wasm ../sfc/1.SFC ../sfc/2.SFC
```

日本語フォントも照合するときは、同じコマンドに `--font` とフォントの保存先を指定します。実測で使った OTF（4,533,028 バイト）の SHA-256 は `dff723ba59d57d136764a04b9b2d03205544f7cd785a711442d6d2d085ac5073` です。
