# 工事写真（JPEG の Exif・XMP）

工事写真の JPEG から、Exif と XMP の情報を読む規則を記録します（#387）。実装は `crates/zumen-photo` です。API では、`photo` の op に JPEG の入力の名前を渡し、画素数・向き・撮影の日時・位置・XMP の性質を返します。画像は復号しません。仕様の本文は写しません（`formats/ledger.toml` の `jpeg`）。

## 対象

- JPEG のセグメント（SOI から、画素のデータの始まりの SOS か、EOI まで）。画素数は、フレームのヘッダー（SOF。DHT・JPG・DAC のマーカーを除く `0xC0`〜`0xCF`）の高さと幅にする。
- APP1 の `Exif\0\0` の後の TIFF の構造（Exif）と、APP1 の `http://ns.adobe.com/xap/1.0/\0` の後の XML（XMP）。拡張の XMP（複数の APP1 に分けたもの）は、まだ読まない。
- 電子小黒板の情報と信憑性の確認は、この読み取りに入れない（`kokuban`）。

## 判定

- 先頭が SOI（`FF D8`）でなければ、`jpeg.file.invalid` を返して失敗する。
- セグメントの長さが 2 より小さいか、ファイルの外に出るか、マーカーの前に `0xFF` がなければ、`jpeg.segment.invalid`（バイトの位置）を返し、そこまでに読んだ値を返す。

## Exif

- バイト順（`II`・`MM`）を読み、0 番目の IFD と、そこから指す Exif の IFD・GPS の IFD だけを読む。縮小画像の IFD は読まない。同じ位置の IFD は二度読まない。IFD の項目は 1024 までとし、多ければその IFD を読まない。
- 知らない型の項目は飛ばす。値がファイルの外に出る項目は、その IFD を読まず `jpeg.exif.invalid`（項目の番号 0）を返す。
- 読む項目は、作り手・型（0 番目の IFD）、向き（1〜8 の外は `jpeg.exif.invalid`）、撮影の日時（DateTimeOriginal を `YYYY-MM-DDTHH:MM:SS` に直し、OffsetTimeOriginal の時差を足す。形が違えば `jpeg.exif.invalid`）。
- GPS は、緯度・経度（度・分・秒の 3 つの有理数を度にし、南と西を負にする）、高さ（海面より下のフラグなら負）、向きとその基準、測地系の名前。緯度か経度の片方だけか、範囲の外なら、位置を返さず `jpeg.exif.invalid`（項目の番号 2）を返す。
- 位置は `zumen-crs` の地理の点（度）にする。GPS の測地系（ふつうは WGS 84）から JGD2011 への換算はしない。測地系の名前は `datum` に返す。
- 文字の値は、終わりの NUL と前後の空白を除く。UTF-8 でなければ、その値を返さない。

## XMP

- `zumen-xml` で読む。XML として読めなければ、`jpeg.xmp.invalid` を返し、XMP の性質を返さない。
- `rdf:Description` の属性（`rdf` と `xmlns` の名前空間のものを除く）と、文字だけを持つ子の要素を、名前空間の URI・接頭辞を除いた名前・値の組にする。配列（`rdf:Alt`・`rdf:Seq`・`rdf:Bag`）は、最初の `rdf:li` の値にする。入れ子の構造は、まだ読まない。

## 上限

- 入力の大きさを `InputBytes`、セグメントと IFD の項目と `rdf:Description` の数を `Work` で数える。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `jpeg` | 読み | `external` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-jpeg)にあります。`external` の根拠は、Wikimedia Commons の CC0 の写真（[`samples/jpeg/`](../../samples/README.md#jpeg)）を Pillow 13.0.0 で読んだ値との照合です（[JPEG の外部照合](../../xtask/verify/photo/README.md)、#505）。

- 公開サンプルは iPhone 6s で撮り、GIMP 2.10 で保存し直した写真で、Exif（作り手・型・撮影の日時）と GPS（緯度・経度・高さ・向きと基準）、GIMP の XMP（属性・空の配列・属性だけの `rdf:li` の入れ子）を持つ。画素数・向き・撮影の日時・作り手と型・GPS の値は Pillow が読んだタグの値と、XMP の名前空間・名前・値は Pillow が APP1 から取り出したパケットを Python の Expat で読んだ値と照らした。
- テストの中で作る 4 つの JPEG も同じく照らす（バイト順の II と MM、南と西、海面より下、時差の有無、緯度だけ、UTF-8 でない値、配列・入れ子・前後の空白を持つ XMP）。これは `synthetic` の確認で、段の根拠に数えない。
- XMP の性質の選び方（属性、文字だけを持つ子の要素、配列の最初の項目）は zumen の規則で、Python の側に同じ規則を書いて照らす。XMP の取り出しと名前空間の解決は外部の実装で確かめているが、性質の選び方そのものは外部の実装で確かめていない。
- 自作の APP1 に UTF-16LE・UTF-16BE・Shift_JIS の XMP を置き、名前空間・名前・値を検査する（[XML の規則](xml.md)）。この確認も `synthetic`。

電子小黒板の写真、拡張の XMP、縮小画像の IFD、MakerNote、基準になるソフトとの照合はまだです。実際の工事写真では、ここに書いた扱いと違うことがあります。
