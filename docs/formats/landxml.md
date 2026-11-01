# J-LandXML

J-LandXML（LandXML 1.2 に準じた 3 次元設計データ交換標準、台帳の `jlandxml_1_8`）を読み、土木の領域モデル（[決定 0230](../decisions/0230-civil-model-alignments.md)）に移す規則を記録します。実装は `crates/zumen-landxml` です。境界では、`open` が XML の根の `LandXML` と名前空間で見分けて開き、`civil` の op に `doc`（線形が 2 つ以上なら `id` も）を渡して線形の上の所を求めます。仕様の本文とスキーマは写しません（`formats/ledger.toml`）。

## 対象

- J-LandXML Ver.1.8（令和 8 年 7 月）。LandXML 1.2 の名前空間の文書で、交換標準の版は `Project` の `Feature` の `applicationCriterion`（`MlitLandXmlVer.1.8` など）に書く。版の違いでは読み方を変えない。
- 文字コードは UTF-8 と Shift_JIS を読む（`zumen-xml`）。交換標準が認める UTF-16 は、まだ読まずに `xml.encoding.unsupported` を返す。
- 書き戻しはしない。書き出すと、読んだバイト列をそのまま書く。

## 判定

- 最初の要素が LandXML 1.2 の名前空間（`http://www.landxml.org/schema/LandXML-1.2`）の `LandXML` なら J-LandXML とする（`Signature::XmlRoot`）。交換標準の版の印がない LandXML 1.2 も、同じ規則で読む。

## 座標と高さ

- 点の文字は「X Y Z」の順で、X を北（`northing`）、Y を東（`easting`）、Z を標高として読む。Z はなくてもよい。
- `CoordinateSystem` の `horizontalCoordinateSystemName`（`1(X,Y)`〜`19(X,Y)`）を平面直角座標系の系に、`horizontalDatum` の `JGD2011`・`JGD2024` を測地系と標高の成果（測地成果2011・2024）にする。座標系を書いていないか、ほかの測地系（`JGD2000`・`TD`）なら、CRS を不明にして `landxml.crs.unsupported` を返す。
- `verticalDatum` の `T.P` を東京湾平均海面にする。ほかの基準（`K.P`・`O.P` など）は、`differTP` の差を足さずに、高さの基準を不明にして `landxml.height-datum.unsupported` を返す。

## 線形

- `Alignment` を線形にする。ID は `name` で作り、名前のないものと重なるものは `alignment/<順の番号>` にする（決定 0230）。
- 測点の値は、線形の初めからの連続した内部の値（メートル）なので、追加距離は測点の値から `staStart` を引いて求める。`StaEquation` は、`staInternal` の追加距離での不連続にし、`staBack` と `staAhead` を戻りと進みの測点の値にする。
- 測点の間隔は、線形の `Feature`（`Interval`）の `main` を読む。表記（`No.12+5.000`）は境界で作る。
- `CoordGeom` の `Line`・`Curve`・`Spiral` を、書いた順に平面線形の区間にする。区間の初めの点は `Start` の点にし、初めの向きは要素の点から求める。
  - `Line`: `Start` から `End` への向き。長さは `length`、なければ 2 点の距離。
  - `Curve`: `Start` から `Center` への向きを、`rot` の向きに 90° 回したもの。半径は `radius`、なければ `Start` と `Center` の距離。長さは `length`、なければ中心から見た角度から求める。`rot` が `ccw`（左へ曲がる）なら、半径を負にする。
  - `Spiral`: `Start` から `PI`（2 つの接線の交点）への向き。`PI` がなければ、前の区間の終わりの向き。`radiusStart`・`radiusEnd` の `INF` は直線の側（半径なし）にする。`spiType` の `clothoid` をクロソイドにし、ほかは対応する種類（`cubicParabola`・`sineHalfWave`・`cosine`・`bloss`・`weinerBogen`）か、元の名前のまま持つ。
- 前の区間の終わりと次の区間の初めが合わなくても直さない。確かめるのは `zumen_civil::check` で行う。
- 読めない点・`rot`・長さ・半径の区間は、移さずに `landxml.value.invalid` を返す。

## 縦断

- 線形の最初の `Profile` の最初の `ProfAlign` を縦断線形にする。`PVI`（縦断曲線のない変化点）と `ParaCurve`（縦断曲線のある変化点。`length` が縦断曲線の長さ）の並びを、変化点の間の一定の勾配の区間と、変化点を中心にした 2 次放物線の区間に直す。区間の出所は、区間の初めの変化点の要素にする。
- 2 つ目からの `ProfAlign`、縦断地盤線（`ProfSurf`）、円の縦断曲線（`CircCurve`）と対称でない放物線は、`landxml.element.unsupported` を返す。追加距離が増えない変化点と、重なる縦断曲線は、縦断線形を移さずに `landxml.value.invalid` を返す。

## 横断

- `CrossSects` の `CrossSect` を、`sta` の追加距離の横断にし、`name`（`No.10+0.0000` など）を名前にする。
- `DesignCrossSectSurf` を計画の横断の線にし、`name`（`Carriageway` など）を名前にする。点は `CrossSectPnt` の「離れと標高」の 2 つの数（`dataFormat` が `OffsetElevation`、書いていないか `Offset Elevation`）で、離れは中心線から右を正とする。
- 高さを計画高からの差（`heightType` が `fhOffset`）か、勾配と距離（`SlopeDistance`）で書いた線は、`landxml.cross-section-line.unsupported` を返して移さない。
- `CrossSectSurf` は、`PntList2D` ごとに横断の線にし、`ExistingGround` を現況の線、ほか（`SoilLine-n` など）を元の名前の役割にする。
- `desc`（`道路面` などの面の種類）・`side`・`Feature` の情報（幅員中心・数量など）は、まだ移さない。

## 地表面

- `Surfaces` の `Surface` を TIN にする。ID は `name` で作る。`desc` の `ExistingGround` を現況、`FinishedGrade` を計画、ほかを元の値の役割にする。
- `Definition`（`surfType` が `TIN`。交換標準の本文の `SurfType` も読む）の `Pnts` の `P`（`id` と「X Y Z」）を点に、`Faces` の `F`（3 つの点の `id`）を三角形にする。見えない面（`i` が `1`）は移さず、数を `landxml.face.dropped` で返す。
- `Feature`（`AreaPnts`）の `areaList` を境界線にする。
- 知らない点を指す面、重なる `id` の点があれば、その地表面を移さずに `landxml.value.invalid` を返す。

## 移さない要素

- 片勾配すりつけ（`Superelevation`）、IP 点のリスト（`AlignPIs`）、座標点セット（`CgPoints`）、`Roadways` は、まだ移さず、`landxml.element.unsupported` を返す。
- 単位（`Units`）は、交換標準がメートルと決めているので読まない。方向角の属性は交換標準にないので、角の単位は使わない。

## 上限

- 入力のバイト数と、移す要素（区間・変化点・横断・点・面）の数を `elements` で予算で数える。
