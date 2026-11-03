# 基盤地図情報（GML）

国土地理院の基盤地図情報ダウンロードデータ（台帳の `fgd_dl_5_3`・`fgd_dl_5_2`）の GML を読み、地理の領域モデル（[決定 0233](../decisions/0233-geo-model-features.md)）に移す規則を記録します。実装は `crates/zumen-fgd`、モデルは `crates/zumen-geo` です。境界では、`open` が XML の根の `Dataset` と名前空間で見分けて開き、`geo` の op に `doc`（と、中身を見る地物の `id`）を渡して、地物の型ごとの数と地物の属性・形の要約を返します。仕様書の本文とスキーマは写しません（`formats/ledger.toml`）。節の番号は、ファイル仕様書の第 5.3 版のものです。

## 対象

- 基本項目（測量の基準点・標高点・等高線・行政区画・水系・構造物など）と、数値標高モデル（DEM 区画）の GML。どちらも JPGIS 2014 の符号化規則（4.2.2 節）で、GML 3.2 の名前空間を使う。
- 文字コードは UTF-8（4.2.2 節）。Shift_JIS の古いデータも `zumen-xml` で読む。
- 配布の書庫（ZIP、4.1 節）も開く（下の「書庫」）。
- ジオイド・モデル（DGHM 区画）と基準面補正パラメータ（HRCV 区画）は、地物と属性だけを移し、被覆は移さない（下の「被覆」）。
- 書き戻しはしない。書き出すと、読んだバイト列をそのまま書く。

## 判定

- 最初の要素が基盤地図情報の名前空間（`http://fgd.gsi.go.jp/spec/2008/FGD_GMLSchema`、4.2.4 節の表 4-2）の `Dataset` なら、基盤地図情報とする（`Signature::XmlRoot`）。
- ZIP の書庫で、フォルダーを除いた名前が `FG-GML-` で始まり `.xml` か `.zip` で終わる項目を持てば、基盤地図情報の書庫とする（`Signature::ZipName`、[決定 0251](../decisions/0251-fgd-archives.md)）。open は、SFZ・P2Z の後、ifcZIP の前に試す。

## 座標系

- 文書で初めに `srsName` を持つ要素の値を、文書の座標系にする（3.1 節）。
  - `fguuid:jgd2024.bl` は JGD2024 の経緯度、`fguuid:jgd2011.bl` は JGD2011 の経緯度にする（`Crs::Geographic`）。2025-07-31 からのデータは JGD2024（第 5.2 版の更新履歴）。
  - 高さの基準は、経緯度と同じ測地成果の標高にする（`fguuid:jgd2024.bl` なら JGD2024、`fguuid:jgd2011.bl` なら JGD2011）。仕様書は標高の基準を書いていないので、これは推定。
  - ほかの値は `zumen-crs` の `Crs::from_name` で読む。読めなければ（JGD2000 の `fguuid:jgd2000.bl` など）`fgd.crs.unsupported` を返し、座標系を不明にして形を移さない。地物と属性は移す。
- 形の要素の `srsName` が文書の座標系と違えば、`fgd.crs.unsupported` を返し、その形を移さない。
- 座標は（緯度・経度）の順に読む（2.2.3 節、DirectPosition は 2 次元）。`srsDimension` が 3 なら 3 つめを標高にする。

## 地物

- 根の `Dataset` の子で、基盤地図情報の名前空間の要素を地物にする（4.2.1 節）。子の `description`（データセットの説明）と、GML の `gml:name`・`gml:description`・`gml:identifier`・`gml:boundedBy` は地物にしない。ほかの名前空間の要素は `fgd.element.unsupported` を返す。
- 型は `fgd:` と要素の名前（`fgd:RdEdg`・`fgd:BldA`・`fgd:DEM` など、4.2.6 節の表 4-3）。
- ID は `gml:id`。なければ、型と文書の中の順の番号（`fgd:RdEdg/0`）から作る。基盤地図情報レコード ID（`fid`、4.2.5 節）は属性 `fgd:fid` にする。
- 性質 `type`（種別）を分類コード、`name`（名称・点名称）を名前にする。
- 関連（`repPt`・`admArea`・`surfA`・`compL`）は、`xlink:href` の文字を属性にする。関連の中に地物を書いたものは、子の地物にする。

## 属性

型は、応用スキーマ（2.2.2 節）の属性の型と、GML のスキーマ（Ver5.1）の型から決める。

| 性質 | 値 |
| --- | --- |
| `lfSpanFr`・`lfSpanTo`・`devDate` | `gml:timePosition` の文字。`YYYY-MM-DD` なら日付、ほかは文字 |
| `B`・`L`・`alti`・`gh`・`hrc` | 実数。数でなければ `fgd.value.invalid` |
| `altiAcc` | 整数。整数でなければ `fgd.value.invalid` |
| `orgGILvl`・`vis`・`admOffice` | 列挙型なので、コード（一覧の名前なし） |
| ほか（`fid`・`orgMDId`・`orgName`・`gcpClass`・`gcpCode`・`admCode`・`sbaNo`・`mesh` など） | 文字。行政コードやメッシュ番号のような数字の並びも、文字のままにする |

- 名前は `fgd:` と性質の名前にする（`fgd:admCode`）。

## 形

空間属性の適用パターン（2.2.3 節）を読む。どの形も LOD を持たない（`None`）。

- 点要素 P パターン（`gml:Point` の `gml:pos`）は点にする。
- 線要素 L パターン（`gml:Curve` の `gml:segments` の `gml:LineStringSegment` の `gml:posList`）は線にする。区間が複数あれば、前の終わりと同じ初めの点を除いてつなぐ。`gml:LineStringSegment` でない区間は `fgd.element.unsupported` を返し、その線を移さない。
- 面要素 A パターン（`gml:Surface` の `gml:patches` の `gml:PolygonPatch`）と、面要素 B パターン（`gml:PolyhedralSurface` の `gml:polygonPatches`）、`gml:Polygon` は面（外周と穴）にする。
  - 輪は、`gml:Ring` の `gml:curveMember` の曲線を順につなぐ。`gml:LinearRing` なら座標の列を読む。
  - 境界の曲線は、中に書いたものか、`xlink:href` で同じ文書の `gml:id` を引いたもの。`gml:OrientableCurve` の `orientation="-"`（マイナス方向の参照）は、基の曲線を逆に辿る。
  - 輪の閉じる点（初めと同じ終わりの点）は除く。点が 3 つに満たないか、曲線が読めない輪は `fgd.value.invalid` を返し、外周がなければ面を移さない。
- ほかの GML の形は `fgd.element.unsupported` を返す。
- 同じ文書にない参照と、ない ID は `fgd.reference.unsupported` を返す。参照は 32 段まで引く。
- 地物が標高（`alti`）を持ち、点が標高を持たなければ、点の標高を `alti` にする（標高点・等高線・測量の基準点）。

## 被覆（DEM 区画）

DEM 区画の被覆（`coverage`、2.2.2 節の `CV_DiscreteGridPointCoverage` と `CV_GridValuesMatrix`）を、セルの中心の点を結ぶ三角形の集まり（`zumen-geom3d` の `Shape::Triangles`、閉じていない）にする。

- 範囲は `gml:boundedBy` の `gml:Envelope` の `gml:lowerCorner`（南・西）と `gml:upperCorner`（北・東）。格子の列と行の数は、`gml:GridEnvelope` の `gml:low` と `gml:high` から求める。
- 値は `gml:tupleList` の「種別,標高値」の並び。先頭のセルは北西の端で、西から東へ並び、東の端で 1 行南へ進む（並びの規則 `+x -y` の `Linear`）。値の列は `gml:startPoint` のセルから始まり、足りない末尾のセルは値がない。
- 点は、セルの中心（北の端から行の番号と 0.5 のセルの高さ、西の端から列の番号と 0.5 のセルの幅）に置く。仕様書は点とセルの対応を書いていないので、これは推定。
- 種別が「データなし」か、標高値が -9999 以下の点は置かない。
- 隣り合う 4 つのセルの四角形を、北西・北東・南西と、北東・南東・南西の 2 つの三角形にする。どちらかの 3 点のうち 1 つでもなければ、その三角形を作らない。三角形は、（緯度・経度・標高）の空間で上から見て反時計回り。
- DEM 構成点の種別（地表面・表層面・海水面など）は、点ごとには移さず、種別ごとの点の数を属性 `fgd:coverage/<種別>`（整数）にする。データなし以外の種別が 2 つ以上あれば `fgd.point-kind.dropped` を返す。
- 並びの規則が `+x -y` の `Linear` でなければ `fgd.element.unsupported` を、範囲・格子・値の列が読めないか、値が格子より多ければ `fgd.value.invalid` を返す。
- DGHM 区画・HRCV 区画の被覆は、値が標高でないので `fgd.element.unsupported` を返して移さない。ジオイド・モデルは、利用者が `zumen-crs` に渡して使う（[決定 0019](../decisions/0019-coordinate-systems-and-gsi-data.md)）。

## 書庫

配布の書庫（[決定 0251](../decisions/0251-fgd-archives.md)、`crates/zumen-fgd/src/archive.rs`）の中の GML を、1 つの地理の領域モデルにまとめる。名前の規則は「基盤地図情報データのファイル名について」（2025-07-31）による。

- `FG-GML-` で始まる `.xml` の項目を、書庫の順に GML として読む。`FG-GML-` で始まる `.zip` の項目（全項目の書庫の中の、提供クラスごとの書庫）は、1 段だけ開く。全項目の書庫が中に書庫を入れるかは確かめていない（推定）。ほかの項目は `fgd.entry.unsupported` を返す。
- 地物の ID は、項目の名前と `#` と、GML を 1 つで開いたときの ID（`FG-GML-533945-RdEdg-20250801-0001.xml#K1_1`）。入れ子の書庫の項目は、外の項目の名前と `/` でつなぐ。
- 出所のバイトの範囲は、項目を展開したバイト列の中の範囲。
- 文書の座標系と高さの基準は、初めの形を持つ項目のもの。違うものの項目は `fgd.crs.unsupported`（値は項目の名前）を返し、その項目の形を移さない。
- 読めない GML の項目は、XML の読み取りの診断を返し、ほかの項目を読み進める。展開の大きさと入れ子の深さは `zumen-zip` の予算で止める。

## 上限

- 要素の数は、地物と形の要素ごとに予算（`Elements`）で数え、座標の点の数と DEM 区画のセルの数は `Segments` で数える。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `fgd` | 読み | `synthetic` |

段の意味は[決定 0254](../decisions/0254-parallel-agents-and-cross-review.md) に、根拠は[形式と権利の一覧](README.md#format-fgd)にあります。この文書の規則は、仕様と、試験の中で作ったファイルだけで確かめたものです。公開サンプル・受け取った図面・外部の実装・基準になるソフトとは、まだ照らしていません。実際のファイルでは、ここに書いた扱いと違うことがあります。

- 試験は、試験の中で作った小さな GML と書庫（`crates/zumen-fgd/tests/lift.rs`・`archive.rs`）と、適合試験の組（`bindings/conformance/cases/fgd.json`・`fgd_zip.json`）。国土地理院のデータは、Git・パッケージ・試験に入れない。
- 基盤地図情報ビューアーと QGIS で開いたデータとの照合は、まだしていない（#504）。
