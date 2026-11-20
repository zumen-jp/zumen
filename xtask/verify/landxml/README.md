# J-LandXML の保持値と公開 TIN の検証

片勾配と IP・基準点・地盤縦断・横断の属性・高さ基準について、Rust のネイティブと `wasm32-wasip1` で同じ合成入力を読み、土木の領域モデルへ移します（#468）。

## 実行

リポジトリが指定する Rust と Node.js 24 以降を使います。`wasm32-wasip1` がなければ、同じ Rust toolchain に追加します。Node.js の [WASI](https://nodejs.org/docs/latest-v24.x/api/wasi.html) だけで実行し、追加の npm パッケージは使いません。

```powershell
rustup target add wasm32-wasip1
$env:CARGO_TARGET_DIR = 'target/landxml-portability-468'
cargo test --locked -p zumen-landxml --test superelevation_contract
cargo test --locked -p zumen-landxml --test values_contract
node xtask/verify/landxml/compare_superelevation.mjs
```

`compare_superelevation.mjs` は、`rustc -vV` が返すネイティブの対象と `wasm32-wasip1` の `superelevation_probe`・`values_probe` をビルドし、それぞれを 2 回実行します。`CARGO_TARGET_DIR` を指定しなければ、`target/landxml-superelevation-contract` を使います。ビルド・起動・probe 内の検査・出力の比較が失敗したら、コマンドも失敗します。

## 入力と比較するもの

[superelevation_contract.rs](../../../crates/zumen-landxml/tests/superelevation_contract.rs) が、13 種類の小さな入力をテスト内で作ります。[superelevation_probe.rs](../../../crates/zumen-landxml/examples/superelevation_probe.rs) も同じ内容を検査します。公開サンプルや受け取った図面は使いません。

- 子の測点を属性より優先する一定区間、属性だけの区間、CDATA と文字参照を持つ勾配について、追加距離・長さ・左右の勾配（%）と元の要素のバイト範囲を期待値で確かめます。
- 直線部の勾配、基本型のすりつけ、S 型の反転位置と待ち勾配、任意横断勾配について、変化点と左右の値を既知値に照らします。
- 未対応の子、複数の同種値、後ろにある `SlopeList`、`useSlopeList=true`、測点の欠落、長さ 0、不正な勾配、負の長さについて、移す区間の数と診断の code を確かめます。
- `read` と `lift` の一連の処理と、`lift` 単体を分けます。全 `Kind` の使用量を記録し、成功する上限ちょうどでも結果が変わらないことを確かめます。使用量が正なら上限を 1 減らし、該当する `Exceeded` で停止してモデルを返さないことを確かめます。使用しない単位は上限 0 で成功することを確かめます。
- `Depth` の使用量は最終的な深さなので、成功する最小の上限を別に探し、その 1 未満で停止することを確かめます。
- `cancel` と `expire` を処理の前に通知し、読み取り前と `lift` 前のどちらもモデルを返さず、予算を使わずに停止することを確かめます。実行中に別スレッドから通知する時刻は比較しません。

出力は、片勾配の全浮動小数を `f64::to_bits` にした値、線形の識別子、出所、診断の順・code・件数・値・場所・仕様参照、全 `Kind` の使用量、停止理由と上限です。この出力全体をバイト単位で比較します。テスト内でも繰り返しの結果を比較します。

## 確認の範囲

`values_contract.rs` と `values_probe.rs` は、IP・基準点・地盤縦断・横断の属性・高さ基準を別の自作入力で検査します。元の値・参照する縦断・省略された高さ・元の位置と、全予算の境界・中断を記録し、全バイトを環境間で照らします。相対高さの計画高への評価、壊れた点列をつながないこと、重複や欠落した参照を推測しないことも検査します。

この検査は、コアを直接呼ぶネイティブと 32 bit の wasm の比較です。配布する `wasm32-unknown-unknown` の API・バインディングの適合テストや、外部の実装との値の照合とは分けます。基準になるアプリとの一致と、書き出しは検査しません。合成入力の補間と `SlopeList` の値の解釈は、仕様から定めた期待値との検査です。

## 公開原本の TIN

農林水産省の 2 原本を別々に読み、[公開 Rust API の `Tin`](../../../crates/zumen-civil/src/lib.rs) の頂点と面を検査します。Python の ElementTree で地表面ごとに XML の点 ID を頂点の並び順に対応させます。地表面の名前・順序、頂点の北・東・標高の 64 ビット、可視面が指す 3 頂点とその順序を比較します。

| 原本 | SHA-256 | 地表面ごとの頂点数 / 面数 |
| --- | --- | --- |
| [ほ場整備整地工](https://www.maff.go.jp/j/nousin/sekkei/01_hojo-seichi.xml) | `dd5f47f8aa1724cd93fae451322967c437b7c8b1da955f3f1e8aea6a68e2e97c` | 8 / 6 |
| [道路工・畦畔](https://www.maff.go.jp/j/nousin/sekkei/02_douro-keihan.xml) | `bec7804e5394bf348b42546e6889bd2a7752ccd4ce92cec423a9b238f77c2c9a` | 258 / 430・25 / 32・306 / 510・311 / 521 |

原本はリポジトリ外に保存し、リポジトリのルートから次を実行します。Windows では実行ファイルの末尾に `.exe` を付けます。照合に成功すると原本の SHA-256 と件数が表示されます。

```sh
cargo build --locked -p zumen-landxml --example tin_probe
python xtask/verify/landxml/compare_public_tin.py target/debug/examples/tin_probe /path/to/01_hojo-seichi.xml /path/to/02_douro-keihan.xml
```
