# 工事写真（JPEG の Exif・XMP）

工事写真の JPEG から、Exif と XMP の情報を読む規則を記録します（#387）。実装は `crates/zumen-photo` です。境界では、`photo` の op に JPEG の入力の名前を渡し、画素数・向き・撮影の日時・位置・XMP の性質を返します。画像は復号しません。仕様の本文は写しません（`formats/ledger.toml` の `jpeg`）。

## 対象

- JPEG のセグメント（SOI から、画素のデータの始まりの SOS か、EOI まで）。画素数は、フレームの見出し（SOF。DHT・JPG・DAC の印を除く `0xC0`〜`0xCF`）の高さと幅にする。
- APP1 の `Exif\0\0` の後の TIFF の構造（Exif）と、APP1 の `http://ns.adobe.com/xap/1.0/\0` の後の XML（XMP）。拡張の XMP（複数の APP1 に分けたもの）は、まだ読まない。
- 電子小黒板の情報と信憑性の確認は、この読み取りに入れない（`kokuban`）。

## 判定

- 先頭が SOI（`FF D8`）でなければ、`jpeg.file.invalid` を返して失敗する。
- セグメントの長さが 2 より小さいか、ファイルの外に出るか、印の前に `0xFF` がなければ、`jpeg.segment.invalid`（バイトの位置）を返し、そこまでに読んだ値を返す。

## Exif

- バイト順（`II`・`MM`）を読み、0 番目の IFD と、そこから指す Exif の IFD・GPS の IFD だけを読む。縮小画像の IFD は読まない。同じ位置の IFD は二度読まない。IFD の項目は 1024 までとし、多ければその IFD を読まない。
- 知らない型の項目は飛ばす。値がファイルの外に出る項目は、その IFD を読まず `jpeg.exif.invalid`（項目の番号 0）を返す。
- 読む項目は、作り手・型（0 番目の IFD）、向き（1〜8 の外は `jpeg.exif.invalid`）、撮影の日時（DateTimeOriginal を `YYYY-MM-DDTHH:MM:SS` に直し、OffsetTimeOriginal の時差を足す。形が違えば `jpeg.exif.invalid`）。
- GPS は、緯度・経度（度・分・秒の 3 つの有理数を度にし、南と西を負にする）、高さ（海面より下の印なら負）、向きとその基準、測地系の名前。緯度か経度の片方だけか、範囲の外なら、位置を返さず `jpeg.exif.invalid`（項目の番号 2）を返す。
- 位置は `zumen-crs` の地理の点（度）にする。GPS の測地系（ふつうは WGS 84）から JGD2011 への換算はしない。測地系の名前は `datum` に返す。
- 文字の値は、終わりの NUL と前後の空白を除く。UTF-8 でなければ、その値を返さない。

## XMP

- `zumen-xml` で読む。XML として読めなければ、`jpeg.xmp.invalid` を返し、XMP の性質を返さない。
- `rdf:Description` の属性（`rdf` と `xmlns` の名前空間のものを除く）と、文字だけを持つ子の要素を、名前空間の URI・接頭辞を除いた名前・値の組にする。配列（`rdf:Alt`・`rdf:Seq`・`rdf:Bag`）は、最初の `rdf:li` の値にする。入れ子の構造は、まだ読まない。

## 上限

- 入力の大きさを `InputBytes`、セグメントと IFD の項目と `rdf:Description` の数を `Work` で数える。
