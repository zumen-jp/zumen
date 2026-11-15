# E57の公開サンプルをpye57と照らす

[#504](https://github.com/zumen-jp/zumen/issues/504)の点群の読みを、公開の libE57Format サンプルと別実装で確かめます。`e57` crate が読むこと自体は、外部照合に数えません。

```powershell
uv pip install --python <検証用venvのPython> --only-binary :all: pye57==0.4.19 numpy==2.5.4 pyquaternion==0.9.9
$env:ZUMEN_PYE57_PYTHON = '<検証用venvのPython>'
$env:ZUMEN_PYE57_REQUIRED = '1'
cargo test --locked -p zumen-e57 --test external
```

既存の Python を uv で指定しても実行できます。`ZUMEN_PYE57_PYTHON` に外部ツールを入れた Python の実行ファイルを指定すると、その環境を使います。指定も必須指定もなく未導入なら外部照合を飛ばします。指定した環境で未導入・版違い・起動失敗があれば失敗にします。公開サンプルの点数・属性・診断は、外部ツールの有無によらず検査します。

## 入力と期待値

| 入力 | 座標の型 | 点数 | 座標の範囲 | 色 |
| --- | --- | ---: | --- | --- |
| `samples/e57/libe57-coloured-cube-double.e57` | float64 | 7,680 | X/Y/Zそれぞれ−0.5〜0.5 | RGB各0〜255 |
| `samples/e57/libe57-coloured-cube-float.e57` | float32 | 7,680 | X/Y/Zそれぞれ−0.5〜0.5 | RGB各0〜255 |

両方とも E57 1.0、スキャン 1 件・画像 0 件です。姿勢・座標系は指定されていません。コアは各ファイルで `e57.crs.missing` を 1 件返し、水平・鉛直の座標系を不明のままにします。強度・分類・反射・時刻は持ちません。

`read.py` は SHA-256 で固定したこの 2 件だけを受け付けます。pye57 の生の全点をファイルの順で読み、姿勢・フィールド・色の幅を確認してから、RGB を 257 倍してコアの 16 ビット値に換算します。座標は float64 のリトルエンディアンのビット列、色は 16 ビット整数で比較します。座標の許容差や間引きは使いません。点数・順序・全座標・全色・範囲が違えば Rust のテストが失敗します。

`public_cubes_keep_all_points_colors_and_missing_crs` は、生の読みから点群へ移す間に全座標と色を保つこと、属性と欠落診断も検査します。`bindings/conformance/cases/pointcloud_e57_samples.json` は、同じ公開サンプルを公開 API の `open`・`pointcloud` で開き、全点の応答・診断をネイティブと wasm で照らします。外部実装との照合と環境間の一致は別の証拠です。

## 権利と範囲

2026-11-16 に、[libE57Format-test-dataの固定版のREADME](https://github.com/asmaloney/libE57Format-test-data/blob/24201531715945ba7d49af6dfcb8e07b6b9b6b7f/README.md)を確認しました。`self/` は libE57Format 自身が生成したサンプルで、`reference/` 以外には CC0 1.0 が適用されます。この 2 件は `self/` から取得したままのバイト列で保存し、出所・指紋・大きさ・利用条件を `samples/ledger.toml` に記録しています。メタデータは合成立方体の名前・説明とライブラリ版で、個人の氏名・住所・連絡先・端末パスを含みません。

[pye57 0.4.19](https://github.com/davidcaron/pye57/tree/v0.4.19)は libE57Format の Python ラッパーで、[MIT](https://github.com/davidcaron/pye57/blob/v0.4.19/LICENSE)です。[pyquaternion 0.9.9](https://pypi.org/project/pyquaternion/0.9.9/)は MIT、NumPy 2.5.4 の条件は[LASの照合](../laspy/README.md#ツールと公開サンプルの権利)と同じです。固定 wheel を別のテスト環境へ導入し、製品へリンク・同梱しません。ASTM の規格本文・製品付属資源・GPL の実装ソースは読みません。

この公開サンプルは他者が作った合成の立方体です。実測点群、球座標、姿勢の回転・移動、複数スキャン、座標系、強度・反射・時刻・無効点・画像、CloudCompare での表示は確認していません。毎晩 CI は外部照合を必須にしますが、GUI アプリは起動しません。

## 公開 API の検証キット

上で外部ツールを入れた Python で次を実行します。Windows の venv は、既存の Python 3.13 を `uv venv --python 3.13 --no-python-downloads` でリポジトリ外に作れます。

```powershell
cargo build --locked -p zumen-cli
& $env:ZUMEN_PYE57_PYTHON -I xtask/verify/e57/check_api.py --cli <zumenの実行ファイル> --out <Git外の新しいフォルダー>
& $env:ZUMEN_PYE57_PYTHON -I xtask/verify/e57/test_check.py <作ったキット>
```

公開サンプル、出典と利用条件、全点の外部観測バイト、CLI の応答、比較結果とコードの指紋を保存します。既存の出力先と Git 内の出力先は拒否します。API は八分木の浅い段から点を選ぶため、全点を重複数と座標・色の組で比較します。ファイル順は Rust の外部テストで別に比較します。改変検査は座標・色・重複・欠落・診断・CRS の不一致を検出します。

両ファイルは格納する座標の型が違いますが、点の値は同じです。float64 に格納されていることを、float32 を超える精度を確認した証拠にはしません。色も赤・緑・青の 3 色で、各成分の中間値は未確認です。毎晩の CI はキットを `verify-e57` に残し、GUI 表示のフィールドは未実行に保ちます。
