# TIFF・GeoTIFF の読み取り

スキャンした完成図、SXF や CAD のラスター参照画像、UAV 写真測量のオルソ画像、数値標高モデル（DEM）の GeoTIFF を `zumen-tiff` で読みます（#877）。書き出しは行いません。

## 典拠と範囲

- クラシック TIFF（32 ビットオフセット）は Adobe の TIFF Revision 6.0（1992-06-03）に従う。先頭 4 バイトがリトルエンディアンの `II*\0`（`49 49 2A 00`）またはビッグエンディアンの `MM\0*`（`4D 4D 00 2A`）の入力を判定し、BigTIFF（マジック `43`）はまだ扱わず `tiff.format.unsupported`（`reason = "bigtiff"`）を診断して拒む。
- CCITT Group 4 の 2 値符号化は TIFF 6.0 の Section 10・11 と ITU-T Recommendation T.6（11/1988）に従う。
- 地理座標の参照とアフィン変換は OGC GeoTIFF Standard 1.1（`19-008r4`、2019-09-14）に従う。欠損値のタグ `GDAL_NODATA`（42113）は GDAL・GeoTIFF の実運用に合わせて ASCII 文字列の数値を解釈する（推定）。

## L0・L1 のレコードと IFD

- レコード 0 は先頭 8 バイトのファイルヘッダー（`0..8`）、レコード `1..=N` は各 IFD のディレクトリ本体（件数 2 バイト＋タグ項目 `12 × count` バイト＋次 IFD オフセット 4 バイト）のバイト範囲とする。`FileModel` のレコード一覧は開始オフセット昇順で非重複になるよう並べる。
- 次の IFD オフセットをたどるときは訪問済みオフセットの集合で循環を検出し、循環や範囲外のオフセットは `tiff.file.invalid` にする。
- ストリップ配置（`StripOffsets` 273・`RowsPerStrip` 278・`StripByteCounts` 279）とタイル配置（`TileWidth` 322・`TileLength` 323・`TileOffsets` 324・`TileByteCounts` 325）の両方を読む。`PlanarConfiguration`（284）は `1`（Chunky）と `2`（Planar）の両方を扱う。`Orientation`（274）は `1`〜`8` を画素の向きと GeoTIFF の座標変換に反映する。

## 圧縮方式と展開の予算

各ストリップ・タイルの展開では、出力バッファーを伸ばす前に `Cx` の `DecodedBytes` と `Work` を検査します。小さな圧縮列が巨大な展開サイズを主張する入力（展開爆弾）は、メモリを確保する前に `budget.decoded-bytes.exceeded` で決定的に止まります。

- `1`: 無圧縮
- `4`: CCITT Group 4（`T6Options` 293 の非圧縮拡張モードは未対応として診断する。`FillOrder` 266 の `1` MSB 先頭と `2` LSB 先頭に対応）
- `5`: LZW（TIFF 6.0 Section 13 の MSB 先頭・early-change 9〜12 ビット符号）
- `8`・`32946`: Deflate（RFC 1950 の zlib ラッパーと Adler-32 検査。`32946` で生の RFC 1951 ストリームが置かれた入力も読む）
- `32773`: PackBits（TIFF 6.0 Section 9）

`Predictor`（317）は `1`（予測なし）と `2`（8 ビット・16 ビット整数の水平差分）に対応します。JPEG 圧縮（`6`・`7`）など未対応の圧縮方式は `tiff.compression.unsupported` を記録します。

## 画素表現と `raster` 操作

対応する画素表現は次のとおりです。

- 白黒 2 値（1 ビット、`PhotometricInterpretation` `0` WhiteIsZero・`1` BlackIsZero）
- グレースケール（4・8・16 ビット、アルファの `ExtraSamples` 338 を含む）
- パレットカラー（4・8 ビット、`PhotometricInterpretation = 3`、`ColorMap` 320）
- RGB・RGBA（各チャネル 8・16 ビット、`PhotometricInterpretation = 2`、`ExtraSamples` `1` 乗算済みアルファ・`2` 非乗算アルファ）
- 単バンドの数値標高格子（`SampleFormat` 339 の符号なし整数 8・16・32 ビット、符号付き整数 8・16・32 ビット、IEEE 754 浮動小数点 32・64 ビット）

公開 API の `raster` 操作は、開いた TIFF 文書の `page`（1 始まり、既定は 1）を RGBA8・非インターレースの PNG に符号化し、`BlobKind::Image` の blob として返します。`max_side` を指定したときは、元の長辺が `max_side` を超える場合に限り縦横比を保って縮小し、`MAX_PNG_PIXELS`（16,777,216 画素）以内に収めます。単バンドの数値標高格子を `raster` で画像化するときは、`GDAL_NODATA` と非有限値を透明にし、有効な最小値〜最大値をグレースケールに正規化して可視化します。

## GeoTIFF の座標参照系とアフィン変換

- `GeoKeyDirectoryTag`（34735）・`GeoDoubleParamsTag`（34736）・`GeoAsciiParamsTag`（34737）から次のキーを読む。
  - `GTModelTypeGeoKey`（1024）
  - `GTRasterTypeGeoKey`（1025）: `1` `RasterPixelIsArea`（既定）と `2` `RasterPixelIsPoint`
  - `GeographicTypeGeoKey`（2048）・`ProjectedCSTypeGeoKey`（3072）: `zumen_crs::Crs::from_epsg` で解決し、未対応コードは `tiff.crs.unsupported` を記録する
  - `VerticalCSTypeGeoKey`（4096）: `zumen_crs::VerticalCrs::from_epsg` で解決する
- 画素座標 `(col, row)` からモデル座標 `(X, Y)` への変換は、`ModelTransformationTag`（34264、4×4 行列）または `ModelPixelScaleTag`（33550）＋`ModelTiepointTag`（33922）から 2 次元アフィン変換を組む。GeoTIFF の慣例に従い、モデル座標の `X` を東方向（経緯度では経度）、`Y` を北方向（経緯度では緯度）とする。
- セルの代表点は `RasterPixelIsArea` では `(col + 0.5, row + 0.5)`、`RasterPixelIsPoint` では `(col, row)` とし、画像全体の外枠は `RasterPixelIsArea` では `(0, 0)`〜`(width, height)`、`RasterPixelIsPoint` では `(-0.5, -0.5)`〜`(width - 0.5, height - 0.5)` とする。

## 領域モデルへの持ち上げ

- **`Drawing2D`**: 各ページ（IFD）を `ModelSpace` に持ち上げる。投影座標系（平面直角座標系）の GeoTIFF は `Unit::Meter` で実座標の四隅に `Shape::Underlay`（`UnderlayKind::Image`、`path = "."`、`page = "<1始まりのページ番号>"`）と外枠の閉じた `Shape::Polyline`（レイヤ `"境界"`）を置く。非 GeoTIFF では `XResolution`（282）・`YResolution`（283）・`ResolutionUnit`（296、`2` インチ・`3` センチメートル）が正の有限値なら `Unit::Millimeter` に換算し、解像度が決まらなければ `Unit::Unknown` でピクセル寸法のまま配置する。これにより図面ビューの表示・距離計測と DXF への変換が動く。
- **`GeoModel`**: 先頭ページに有効な GeoTIFF の座標参照系と外枠変換があるとき、四隅を閉じた `Form::Area` の地物 1 件と画像のメタデータ属性（幅・高さ・バンド数・ビット深さ・圧縮方式・ページ数）を持つ `GeoModel` に持ち上げる。座標系がないときは `tiff.crs.missing` を記録する。これにより `geo` 操作と GeoJSON・SIMA への変換が動く。
- **`PointCloud`**: 単バンドの数値格子で、平面直角座標系の GeoTIFF 変換がある最初の IFD から、`GDAL_NODATA` と非有限値を除いた各セル代表点の `(northing, easting, height)` を集めて `Octree` を構築する。経緯度座標系の DEM は平面距離（メートル）を仮定せず `tiff.grid.unsupported`（`reason = "geographic-crs"`）を記録する。これにより `pointcloud` 操作と `deviation`（J-LandXML の設計 TIN との標高差検査）がそのまま動く。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `tiff` | 読み | `synthetic` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-tiff)にあります。

- 単体テスト（`crates/zumen-tiff/tests/tiff.rs`）で、両エンディアン・複数 IFD・循環 IFD・ストリップとタイル・Chunky と Planar・全対応圧縮方式と `Predictor=2`・全対応色空間とビット深さ・GeoTIFF の座標変換（`PixelIsArea`・`PixelIsPoint`）・`Drawing2D`・`GeoModel`・`PointCloud` への持ち上げ・`raster` の PNG 出力と縮小・展開爆弾に対する `DecodedBytes` 予算停止・決定性を確かめた。
- 外部実装との照合テスト（`crates/zumen-tiff/tests/external.rs`・`xtask/verify/tiff/verify.py`）で、Pillow 13.0.0 と tifffile 2026.11.12 が生成した各圧縮方式（無圧縮・PackBits・LZW・Deflate・CCITT Group 4）・タイル配置・16 ビット RGB・複数ページ・GeoTIFF オルソ・Float32 DEM（`GDAL_NODATA` 付き）を読み、`raster` が出力した PNG の全画素、外枠座標、DEM 点群の座標と標高を照合した。自前のスクリプトで生成した合成データのため、台帳の段は `synthetic` とする。第三者の公開サンプルと QGIS での照合は #504 で扱う。
