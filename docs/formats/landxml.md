# J-LandXML

J-LandXML（LandXML 1.2 に準じた 3 次元設計データ交換標準、台帳の `jlandxml_1_8`）を読み、土木の領域モデルに移す規則を記録します。実装は `crates/zumen-landxml` です。API では、`open` が XML の根の `LandXML` と名前空間で見分けて開き、`civil` の op に `doc`（線形が 2 つ以上なら `id` も）を渡して線形の上の所を求めます。仕様の本文とスキーマは写しません（`formats/ledger.toml`）。

## 対象

- J-LandXML Ver.1.8（令和 8 年 7 月）。LandXML 1.2 の名前空間の文書で、交換標準のバージョンは `Project` の `Feature` の `applicationCriterion`（`MlitLandXmlVer.1.8` など）に書く。バージョンの違いでは読み方を変えない。
- 文字コードは UTF-8・Shift_JIS・UTF-16LE・UTF-16BE を読む。BOM と宣言の組、原本の範囲と予算は [XML の規則](xml.md)に従う。
- 書き戻しはしない。書き出すと、読んだバイト列をそのまま書く。

## 判定

- 最初の要素が LandXML 1.2 の名前空間（`http://www.landxml.org/schema/LandXML-1.2`）の `LandXML` なら J-LandXML とする（`Signature::XmlRoot`）。`applicationCriterion` に交換標準のバージョンを書いていない LandXML 1.2 も、同じ規則で読む。

## 座標と高さ

- 点の文字は「X Y Z」の順で、X を北（`northing`）、Y を東（`easting`）、Z を標高として読む。Z はなくてもよい。
- `CoordinateSystem` の `horizontalCoordinateSystemName`（`1(X,Y)`〜`19(X,Y)`）を平面直角座標系の系に、`horizontalDatum` の `JGD2011`・`JGD2024` を測地系と標高の成果（測地成果2011・2024）にする。座標系を書いていないか、ほかの測地系（`JGD2000`・`TD`）なら、CRS を不明にして `landxml.crs.unsupported` を返す。
- `verticalDatum` の `T.P` を東京湾平均海面にする。ほかの基準（`K.P`・`O.P` など）は、`differTP` の差を足さずに、高さの基準を不明にして `landxml.height-datum.unsupported` を返す。

## 線形

- `Alignment` を線形にする。ID は `name` で作り、名前のないものと重なるものは `alignment/<順の番号>` にする。
- 測点の値は、線形の初めからの連続した内部の値（メートル）なので、追加距離は測点の値から `staStart` を引いて求める。`StaEquation` は、`staInternal` の追加距離での不連続にし、`staBack` と `staAhead` を戻りと進みの測点の値にする。
  - 不連続は、追加距離の順に並べる。同じ追加距離の不連続は、書いた順に並べる。`staBack` がない不連続は、並べたあとで、手前の不連続の `staAhead`（なければ `staStart`）から数えた値を戻りの値にする。不連続の追加距離がどれも違えば、値は文書に書いた順によらない。`staBack` が数として読めなければ、`landxml.value.invalid` を返してから、ないときと同じく求める（#560）。
- 測点の間隔は、線形の `Feature`（`Interval`）の `main` を読む。表記（`No.12+5.000`）は API で作る。
- `CoordGeom` の `Line`・`Curve`・`Spiral` を、書いた順に平面線形の区間にする。区間の初めの点は `Start` の点にし、初めの向きは要素の点から求める。
  - `Line`: `Start` から `End` への向き。長さは `length`、なければ 2 点の距離。
  - `Curve`: `Start` から `Center` への向きを、`rot` の向きに 90° 回したもの。半径は `radius`、なければ `Start` と `Center` の距離。長さは `length`、なければ中心から見た角度から求める。`rot` が `ccw`（左へ曲がる）なら、半径を負にする。
  - `Spiral`: `Start` から `PI`（2 つの接線の交点）への向き。`PI` がなければ、前の区間の終わりの向き。`radiusStart`・`radiusEnd` の `INF` は直線の側（半径なし）にする。`spiType` の `clothoid` をクロソイドにし、ほかは対応する種類（`cubicParabola`・`sineHalfWave`・`cosine`・`bloss`・`weinerBogen`）か、元の名前のまま持つ。
- 前の区間の終わりと次の区間の初めが合わなくても直さない。確かめるのは `zumen_civil::check` で行う。
- 読めない点・`rot`・長さ・半径の区間は、移さずに `landxml.value.invalid` を返す。

## 縦断

- 線形の最初の `Profile` の最初の `ProfAlign` を縦断線形にする。`PVI`（縦断曲線のない変化点）と `ParaCurve`（縦断曲線のある変化点。`length` が縦断曲線の長さ）の並びを、変化点の間の一定の勾配の区間と、変化点を中心にした 2 次放物線の区間に直す。区間の出所は、区間の初めの変化点の要素にする。
- 2 つ目からの `ProfAlign`、縦断地盤線（`ProfSurf`）、円の縦断曲線（`CircCurve`）と対称でない放物線は、`landxml.element.unsupported` を返す。追加距離が増えない変化点と、重なる縦断曲線は、縦断線形を移さずに `landxml.value.invalid` を返す。

## 片勾配

- Ver.1.8 の 4-3-24・27〜29 に従い、`Superelevation` の最大片勾配が一定の区間だけを、Rust の `Alignment.cant` に移す。初めの累加距離標は `FullSuperSta`、終わりは `RunoffSta` を優先し、それぞれの子要素がないときだけ `staStart`・`staEnd` を使う。追加距離は線形の `staStart` を引いた値、区間長は終わりから初めを引いた値にする。
- `FullSuperelev` は % のまま、左下がりを負、右下がりを正として保持する。`CantKind::Superelevation` の初めと終わりの左右は、すべて同じ値にする。正規化や単位換算はしない。区間の出所は元の `Superelevation` のバイト範囲とし、`transition` は指定しない。
- すりつけの開始・終了、待ち勾配、`ReverseCrown`・`FlatSta` などの子の `Feature` は、要素ごとに `landxml.element.unsupported` で種類・件数・元の場所を返す。区間の外の勾配や、初めと終わりの間の変化を補わない。
- `FullSuperSta`・`RunoffSta`・`FullSuperelev` の同種の子要素が複数あるものは、対応を推定せず、親の `Superelevation` を `landxml.element.unsupported` にする。初めか終わりを省略したものと、長さ 0 の区間も同じく移さない。明記した数が不正・非有限、区間が逆順、追加距離や長さの計算が非有限なら、`landxml.value.invalid` を返す。優先する子要素が不正でも、属性へ戻らない。
- 4-3-12・24 の任意横断勾配リストは未対応とする。線形の `Feature` に `SlopeList` があれば、文書の順によらず、その線形の `Superelevation` を移さない。`SuperelevationConfig` の `useSlopeList` が `true`、または値を判別できない場合も同じとする。`false`・`0` 以外を保守的に置換の指定として扱う。これらの `Feature` と、移さない `Superelevation` を診断する。直線部のおがみ勾配や車線数もまだ移さない。
- API の `civil` の応答には片勾配の値をまだ含めない。今回の値の取得は `LandXmlFile::lift` が返す Rust の L2 モデルで行う。API への値の公開、すりつけ区間、任意横断勾配リストは #468 に残す。

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

- IP 点のリスト（`AlignPIs`）、座標点セット（`CgPoints`）、`Roadways` は、まだ移さず、`landxml.element.unsupported` を返す。
- 単位（`Units`）は、交換標準がメートルと決めているので読まない。方向角の属性は交換標準にないので、角の単位は使わない。

## 上限

- 入力のバイト数と、移す要素（区間・変化点・横断・点・面）の数を `elements` で予算で数える。
- 片勾配の置換の指定を調べる線形の子の数と、片勾配の子・数の文字の処理を `work` で数える。片勾配の一時の文字列と区間を確保する前に、元の要素のバイト数の 8 倍に 512 バイトを足した量を `decoded-bytes` に数える。置換の指定を調べる `Feature` は、元の要素のバイト数の 4 倍を先に数える。これらはネイティブと wasm32 で共通の量とする。上限で止まれば元の要素を指し、部分結果を返さない。
- `PI` のない `Spiral` は、前の区間の終わりの向きを、平面線形を初めからたどって求める。そのたどる区間の数を `work` で数える（#560）。区間の終わりの点を求める手間（緩和曲線の向きの式を積む刻みの数。1 つの区間で 65,538 まで）も、求める前に `work` で数え、上限で止まれば `Spiral` の所で予算の診断を返す（#727）。

## 確かめた度合い

UTF-16 は自作の XML と J-LandXML で、原本範囲・未変更の書き出し・線形と一定の片勾配・診断と予算を検査します。[独立検査](../../xtask/verify/xml/README.md)で、ネイティブと WASI の公開値を比較します。Expat との比較は XML の値と要素の位置だけで、J-LandXML の領域モデルの外部照合には含めません。

| 形式 | 向き | 段 |
| --- | --- | --- |
| `landxml` | 読み | `synthetic` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-landxml)にあります。この文書の規則は、仕様と、テストの中で作ったファイルだけで確かめたものです。公開サンプル・受け取った図面・外部の実装・基準になるソフトとは、まだ照らしていません。実際のファイルでは、ここに書いた扱いと違うことがあります。

- 再配布を認める公開サンプルは、見つかっていない（#368）。公開サンプルと外部の実装での照合は #504 で行う。
- 片勾配の一定区間は自作の小さな入力で、優先順位・符号・単位・出所と、移さない要素の診断を確かめる。外部の実装や実際のファイルとの一致は、まだ確認していない。
- [実行環境間の比較](../../xtask/verify/landxml/README.md)では、自作の 9 入力について、片勾配の値のビット列・出所・診断・全 `Kind` の予算と停止結果を、ネイティブと `wasm32-wasip1` で比較する。API の適合テストは、開いたときの未対応要素の診断と `civil` の応答を別に比較する。
