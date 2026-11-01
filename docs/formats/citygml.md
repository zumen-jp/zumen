# CityGML

CityGML 2.0（台帳の `citygml_2_0`）の 3D 都市モデル（PLATEAU の製品仕様書と i-UR の拡張を含む、台帳の `plateau_5_1`）を読み、地理の領域モデル（[決定 0233](../decisions/0233-geo-model-features.md)）に移す規則を記録します。実装は `crates/zumen-citygml`、モデルは `crates/zumen-geo` です。境界では、`open` が XML の根の `CityModel` と名前空間で見分けて開き、`geo` の op に `doc`（と、中身を見る地物の `id`）を渡して、地物の型ごとの数と地物の属性・形の要約を返します。仕様の本文とスキーマは写しません（`formats/ledger.toml`）。

## 対象

- CityGML 2.0 の名前空間（`http://www.opengis.net/citygml/2.0`）の文書。PLATEAU の 3D 都市モデルは、この版で書く。CityGML 3.0 は、まだ読まない。
- 文字コードは UTF-8 と Shift_JIS を読む（`zumen-xml`）。
- 配布の書庫（ZIP）は、まだ開かない。中の `.gml` を 1 つずつ開く。
- 書き戻しはしない。書き出すと、読んだバイト列をそのまま書く。

## 判定

- 最初の要素が CityGML 2.0 の名前空間の `CityModel` なら CityGML とする（`Signature::XmlRoot`）。

## 座標系

- 文書で初めに `srsName` を持つ要素（ふつうは `gml:boundedBy` の `gml:Envelope`）の値を、文書の座標系にする。`srsDimension` も、その要素の値を既定にする。
- PLATEAU の EPSG:6697（JGD2011 の経緯度と JGD2011 の標高の複合）は、経緯度の座標系と JGD2011 の標高にする。ほかは `zumen-crs` の `Crs::from_name` で読む（JGD2011 の経緯度・平面直角座標系の系など）。
- 座標系がないか読めなければ、`citygml.crs.unsupported` を返し、座標系を不明にして形を移さない。地物と属性は移す。
- 座標は `srsName` の座標系の軸の順のまま読む。経緯度は（緯度・経度・標高）、平面直角座標系は（X = 北・Y = 東・標高）。3D の形の点は、地図の空間なら（東・北・標高）の順に直す（決定 0233）。

## 地物

- `core:cityObjectMember`（と `gml:featureMember`）の子を地物にする。型は、名前空間の接頭辞をつけた元の名前（`bldg:Building`・`tran:Road` など）。接頭辞は、名前空間ごとに決めた名前（`core`・`bldg`・`tran`・`luse`・`dem`・`brid`・`tun`・`frn`・`veg`・`wtr`・`grp`・`gen`・`app`・`gml`）にし、知らない名前空間（i-UR の `uro`・`urf` など）は文書の接頭辞を使う。
- ID は `gml:id`。なければ、親の地物の ID と型と、親の中の順の番号（`bldg_1/bldg:BuildingPart/0`）から作る。親のない地物は、型と文書の中の順の番号から作る。
- 地物の中の性質の子の要素で、`gml:id` を持つか LOD の形の性質を持つもの（建築物の部分・境界の面・付属物・交通の領域など）は、子の地物にし、親の地物の番号を持つ。子の地物は親の後に並ぶ。
- `gml:name` を名前にし、`gml:description` は属性にする。`gml:boundedBy` は読まない。
- 性質 `class`（`bldg:class` など）の値を、分類コードにする。

## 属性

- 文字だけを持つ性質は、接頭辞をつけた名前の属性にする。値の型は、次の順に決める。
  - `codeSpace` があればコード（一覧の名前と値の文字）。コードの一覧は読まない。
  - `uom` があれば単位つきの量。数でなければ `citygml.value.invalid` を返す。
  - `true`・`false` は真偽、`YYYY-MM-DD` は日付、先頭が 0 でない整数は整数、小数点のある数は実数。どれでもなければ文字。先頭が 0 の数字の並び（番号やコード）は、文字のままにする。
- 文字がなく `xlink:href` だけを持つ性質は、参照の文字を属性にする。
- 子の要素を持つが地物でないもの（i-UR のデータ型 `uro:BuildingDetailAttribute` など）は、葉の性質を、外の性質から `/` でつないだ名前の属性にする（`uro:buildingDetailAttribute/uro:surveyYear`）。データ型の要素の名前は入れない。
- 汎用の属性（`gen:stringAttribute`・`intAttribute`・`doubleAttribute`・`dateAttribute`・`uriAttribute`・`measureAttribute`）は、`gen:` と `name` の名前の属性にし、型は要素の種類から決める。属性の組（`gen:genericAttributeSet`）は、組の名前を `/` でつなぐ。`name` か `gen:value` がないか、値が型に合わなければ `citygml.value.invalid` を返す。

## 形

- `lod` と数字（0〜4）で始まる性質を、その LOD の形にする。中身がなく `xlink:href` だけなら、引いた先の形にする。
- `gml:Point`・`gml:MultiPoint` は点、`gml:LineString`・`gml:MultiCurve`・`gml:CompositeCurve` は線、`gml:Polygon` は面（外周と穴）にする。
- `gml:Solid`・`gml:MultiSurface`・`gml:CompositeSurface`・`gml:Shell`・`gml:Surface`・`gml:TriangulatedSurface`・`gml:Tin`・`gml:OrientableSurface` は、面の集まり（`zumen-geom3d` の `Shape::FaceSet`）にし、評価しない。`gml:Solid` だけを閉じた面の集まりにする。`gml:MultiSolid`・`gml:CompositeSolid` は、立体ごとの面の集まりにする。
- `gml:OrientableSurface` の `orientation="-"` は、輪を逆に回す。`gml:Solid` の内側の殻（`gml:interior`）は、面の集まりで表せないので `citygml.element.unsupported` を返す。
- 輪の閉じる点（初めと同じ終わりの点）は除く。点が 3 つに満たない面は `citygml.value.invalid` を返す。
- 座標は `gml:posList` と `gml:pos` を読む。点の次元は要素の `srsDimension`、なければ文書の既定（3）。値の数が次元で割り切れないか数でなければ、`citygml.value.invalid` を返す。2 次元の点は標高を持たない。
- 形の中の `xlink:href` は、同じ文書の `gml:id`（`#` のあと）を引く。ほかの文書を指すものと、ない ID は `citygml.reference.unsupported` を返す。参照は 32 段まで引く。
- 暗黙の形（`lodXImplicitRepresentation` の `core:ImplicitGeometry`）は、`relativeGMLGeometry` の面の集まりを、`transformationMatrix`（4 行 4 列、行の順）の上の 3 行と `referencePoint` の点で置く `Shape::Mapped` にする。行列の平行移動に基準点の座標を足し、座標系の単位の違いは直さない。
- ほかの GML の形（`gml:Curve` の区間など）は、`citygml.element.unsupported` を返す。

## 移さない要素

- `CityModel` の子の、地物の性質でないもの（`app:appearanceMember` など）は、`citygml.element.unsupported` を返す。見た目（テクスチャ・色）は読まない。

## 上限

- 要素の数は、地物と形の要素ごとに予算（`Elements`）で数え、座標の点の数は `Segments` で数える。
