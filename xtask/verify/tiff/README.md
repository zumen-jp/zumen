# TIFF・GeoTIFF の外部照合

`zumen-tiff` の読み取り・PNG 復号・領域モデル変換を [Pillow](https://github.com/python-pillow/Pillow) 13.0.0（libtiff・libpng）および [tifffile](https://github.com/cgohlke/tifffile) 2026.11.12 と照らします（`#877`）。`crates/zumen-tiff/tests/external.rs` が、別の Python プロセスで動く `verify.py` に 10 種の合成 TIFF・GeoTIFF を出力させてコアで読み、復号した PNG の全画素と、GeoTIFF の外周座標・DEM 点群の座標と標高を検査します。外部ツールのソースとバイナリはリポジトリに置かず、Cargo の依存にも加えません。

## 実行

Python は、すでに入っているものを使います。

```powershell
uv venv --no-python-downloads target/tiff-venv
uv pip install --python target/tiff-venv/Scripts/python.exe --only-binary :all: pillow==13.0.0 tifffile==2026.11.12 numpy==2.5.4
$env:ZUMEN_TIFF_PYTHON = (Resolve-Path target/tiff-venv/Scripts/python.exe).Path
cargo test --locked -p zumen-tiff --test external -- --nocapture
```

ほかの環境では、既存の Python に付く venv も使えます。

```bash
python3 -m venv target/tiff-venv
target/tiff-venv/bin/python -m pip install --only-binary :all: pillow==13.0.0 tifffile==2026.11.12 numpy==2.5.4
ZUMEN_TIFF_PYTHON="$PWD/target/tiff-venv/bin/python" cargo test --locked -p zumen-tiff --test external -- --nocapture
```

`ZUMEN_TIFF_PYTHON` がなければ `python` を使います。Python または Pillow・tifffile がないときは外部照合だけを飛ばし、理由をテストの出力に書きます。`ZUMEN_TIFF_PYTHON` または `ZUMEN_TIFF_REQUIRED` を指定したときは、不在も失敗にします。バージョン違い・起動の失敗・画素や座標の食い違いは、指定の有無によらず失敗です。

## 入力と比較する値

読み取りの補助照合は次のとおり（合成入力のため台帳の読みの段は `synthetic` のまま）。

- 画像・圧縮の照合:
  - Pillow（libtiff）が出力した LZW RGB・PackBits グレースケール・CCITT Group 4（ITU-T T.6）2 値・Adobe Deflate RGBA・8-bit パレット LZW、ならびに tifffile が出力したタイル分割 `Predictor = 2` RGB・16-bit 面分離（`PlanarConfiguration = 2`）RGB・2 ページ構成 TIFF を `Tiff::read` と `encode_raster_png` で PNG に復号し、`verify.py --verify-png` が Pillow（libpng）で開いて全画素の RGBA8 値が一致することを確かめる。
- GeoTIFF・DEM の照合:
  - tifffile が出力した JGD2011 平面直角 IX 系（`EPSG:6677`）のオルソ画像（`PixelIsArea`）と Float32 単バンド DEM（`EPSG:6677`・標高基準 `EPSG:6695`・`PixelIsPoint`・`GDAL_NODATA = "-9999.0"`・`NaN` セル混在）を `Lift::<GeoModel>::lift` と `Lift::<PointCloud>::lift` で読み、外周座標・欠損除外後の全点数・各点の `(northing, easting, height)` が一致することを確かめる。

## ツールの権利

2026-11-13 に、[Pillow 13.0.0 のライセンス](https://github.com/python-pillow/Pillow/blob/13.0.0/LICENSE)（MIT-CMU）と [tifffile 2026.11.12 のライセンス](https://github.com/cgohlke/tifffile/blob/v2026.11.12/LICENSE)（BSD-3-Clause）を確認しました。別に導入するテストのツールであり、リポジトリには同梱しません。
