# ストラスブールの地形図 DWG

Eurométropole de Strasbourg が公開する「Référentiel topographique simplifié」を使いました。配布元は Licence Ouverte（Open Licence）を示しています。2026-11-21 に配布元から取得した DWG を AutoCAD 2027 で開き、R2000 として別名保存しました。以下は別名保存した 1 ファイルのモデル空間の測定です。[データの説明と利用条件](https://www.data.gouv.fr/datasets/referentiel-topographique-simplifie)・[発行元の DWG ZIP](https://sig.strasbourg.eu/datastrasbourg/doref/referentiel_topo_simplifie_dwg.zip)

| 対象 | バイト数 | SHA-256 |
| --- | ---: | --- |
| 配布 ZIP | 40,943,172 | `ead4baad0e9bcaa2af729b810fbae173ad75861c21889434f395ed4da9cc2b13` |
| 配布 DWG（AC1027） | 46,905,854 | `e11d352fe13191c02093ba727411513619183078f3b734ca285f893ab936ae37` |
| 計測に使った R2000（AC1015） | 63,522,232 | `f5d99013d3b051c3d2a188b748b413d3fb09bf12b949d87433ea11d5ed16b019` |

配布 DWG をそのまま公開操作 `open` に渡すと `dwg.file.invalid` で止まります。R2000 の `space/0` はメートル単位で、`drawing` が返す要素は直線 101,854、文字 57,182、ポリライン 162,248、記号配置 1,260 の計 322,544 件です。図面モデルへの変換で `dxf.lift.dropped` が 89,148 件、`dxf.extension-dictionary.unsupported` が 3 件あります。読み落とした図形、用紙、CAD での表示は塊の測定に含めません。

## シーンの監査

記号の中までたどると 449,804 件を訪問します。シーンの要素は 432,164 件、描画する線は 998,806 本（2,896,325 頂点）、文字は 159,242 件です。どの設定もシーン生成の追加診断は 0 件でした。元の実線 110,674 本の両端を復元し、元の座標との軸ごとの絶対差を調べました。ポリラインや文字の形の誤差はこの値に含めません。

| 塊の設定 | 塊数 | シーンのバイト数 | 最大誤差（mm） | ネイティブ（ms） | WASI（ms） |
| --- | ---: | ---: | ---: | ---: | ---: |
| 仮値 | 275,263 | 74,533,464 | 0.03043 | 3,540.8 | 2,644.7 |
| 件数 1,024 | 275,363 | 74,538,664 | 0.03043 | 2,972.3 | 1,513.8 |
| 件数 16,384 | 275,263 | 74,533,464 | 0.03043 | 3,252.6 | 1,914.8 |
| 範囲 16,384 | 285,299 | 75,055,336 | 0.02832 | 3,229.6 | 1,772.3 |
| 範囲 262,144 | 241,490 | 72,777,248 | 0.12203 | 3,069.8 | 1,740.6 |

ネイティブと WASI の全 5 組でシーンの全バイト、元図面とシーンの診断、ID・描く順と復元誤差の監査結果が一致しました。件数 16,384 と仮値のシーンも同一です。時間はほかの処理も動いている端末での実測です。[設定別の件数・時間・SHA-256](dwg-policies-2026-11-21.json)

範囲の倍率を 262,144 にすると、仮値より 33,773 塊・1,756,216 バイト減り、対象線分の最大誤差は約 0.030 mm から 0.122 mm になりました。各条件の監査で ID と描く順は同じでした。この測定の対象はシーン生成です。ブラウザーでの描画、範囲選択、GPU メモリは含めません。

## 再現手順

配布元の DWG をリポジトリの外に展開し、AutoCAD 2027 の `SAVEAS` で R2000 を選びます。上のハッシュは計測に使ったファイルの識別用です。図面、AutoCAD の出力、生成したシーンはリポジトリに置きません。次の例ではリポジトリの隣に `strasbourg` フォルダーを用意し、別名保存版を `r2000.dwg` としています。

```powershell
cargo build --manifest-path bench/chunks/Cargo.toml --locked --release --bin cad
cargo build --manifest-path bench/chunks/Cargo.toml --locked --release --bin cad --target wasm32-wasip1
bench/chunks/target/release/cad.exe ../strasbourg/r2000.dwg ../strasbourg/native > ../strasbourg/native.jsonl
node bench/chunks/compare-cad.mjs bench/chunks/target/wasm32-wasip1/release/cad.wasm ../strasbourg/r2000.dwg ../strasbourg/native ../strasbourg/native.jsonl ../strasbourg/wasi
```

モデル空間の全体を許容差 0.01 m、範囲とフォント指定なしでシーンにします。各設定を 1 回暖機し、シーンの生成を 3 回計時して中央値を取ります。読み取り・図面モデルへの変換・監査・ファイルへの保存を時間から除きます。実行環境は Windows 11 Pro（ビルド 26300）、AMD Ryzen 5 3600XT、メモリ 32 GiB、Rust 1.99.1（MSVC と wasm32-wasip1）、Node.js 24.22.2 です。
