# 公開 PLATEAU の地物と座標の照合

[PLATEAU GIS Converter のテスト用データ](https://github.com/Project-PLATEAU/PLATEAU-GIS-Converter/tree/e1c8feff7d8ff3e132230d6c21eaed4879e4a235/nusamai-plateau/tests/data)にある 4 原本を、[配布元の出典・利用条件](https://github.com/Project-PLATEAU/PLATEAU-GIS-Converter/blob/e1c8feff7d8ff3e132230d6c21eaed4879e4a235/nusamai-plateau/tests/data/LICENSE.txt)に従って参照します。リポジトリ外に取得するファイルの相対パスと SHA-256 は [plateau.sha256](plateau.sha256) にあります。原本を同梱しません。

比較先は、Python 標準の ElementTree が XML に記された値を読んだ結果と、GDAL 3.12.4 の GML ドライバーが返した地物と座標です。Rust 側は同じ原本を `CityGml::read` で開き、公開 API の `Lift<GeoModel>` を呼びます。Windows では実行ファイルに `.exe` を付けてください。GDAL と Python の外部パッケージは、成果物と分けて照合用のプロセスで使います。[GDAL のライセンス](https://gdal.org/en/stable/license.html)

```sh
cargo build --locked -p zumen-citygml --example plateau_probe
cargo build --locked -p zumen-citygml --example plateau_probe --target wasm32-wasip1
python -I xtask/verify/citygml/compare_plateau.py --wasm target/wasm32-wasip1/debug/examples/plateau_probe.wasm target/debug/examples/plateau_probe /path/to/*.gml
python -m pip install --only-binary :all: pyogrio==0.13.0 shapely==2.2.0 numpy==2.5.4
python -I xtask/verify/citygml/compare_plateau.py --gdal target/debug/examples/plateau_probe /path/to/523836_luse_6668_op.gml /path/to/52385721_fld_6697_l1_op.gml
```

XML の照合では地物を ID 順に並べ、各地物の形・面・外周と穴の順、頂点の順序と 3 成分の 64 ビットを比較します。LOD0〜3、EPSG:6697 の複合座標系、同一文書内の参照、モデルへ移さない短い外周の診断件数も照らします。ネイティブと WASI の照合では Rust の出力をそのまま比較します。

GDAL では土地利用と浸水の原本を対象とし、`gml_id` と面・輪の構成と順序を比較します。経緯度は各軸 10⁻¹² 度、標高は 10⁻¹⁰ m を許容差とし、最大の差を軸ごとに報告します。LOD と高さ基準の検証は XML 側で行います。[入力別の結果と適用範囲](../../../docs/formats/citygml.md#公開原本の座標と-lod)
