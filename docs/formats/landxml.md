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
- `verticalDatum` の `T.P`・`T.P.` を東京湾平均海面にする。元の基準名と `Feature/Property` の `differTP` を `CivilModel.height_reference` に残す。`tokyo_bay_offset` は「元の高さ + 差 = T.P. の高さ」となる符号付きのメートルで、座標の高さへ自動で足さない。T.P. は差を省略すると 0。それ以外の差の省略・非数・重複、T.P. に非零の差を指定した場合は差を不明にする。不正な指定は `landxml.value.invalid` で知らせる。
- ほかの基準は共通の `HeightDatum` では不明のままとし、従来の `landxml.height-datum.unsupported` を返す。基準名と明記された差は別に保持する。仕様の河川名から差を補わず、地理院の変換パラメーターも使わない。

## IP と座標点

Ver.1.8 の 4-3-9・10・22・23 に従う。

- `AlignPIs/AlignPI/PI` を `Alignment.intersection_points` に入力順で移す。名前・注記・北と東・任意の標高・元の位置を保持し、IP から平面線形を再計算しない。点の名前を新しい永続 ID にしない。
- `CgPoints` を `CivilModel.point_sets` に移し、セットと点の名前・注記・順序・座標・元の位置を保持する。標高の省略は `None` とし、0 にはしない。
- `CgPoint.featureRef` と同じセットの `Feature.name` で `class` を解き、基準点・水準点の種類を文字のまま保持する。参照先の欠落や重複は診断し、種類を推測しない。中間点の測点・接線方向角、幅杭の接合などの未対応の `Property` は、元の場所と種類を知らせる。

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

- 線形の `Profile` を入力順に調べ、最初の `ProfAlign` を縦断線形にする。`PVI`（縦断曲線のない変化点）と `ParaCurve`（縦断曲線のある変化点。`length` が縦断曲線の長さ）の並びを、変化点の間の一定の勾配の区間と、変化点を中心にした 2 次放物線の区間に直す。区間の出所は、区間の初めの変化点の要素にする。
- 最初に保持した `ProfAlign.name` を `Alignment.vertical_name` に残す。同名の定義が複数あれば `vertical_name_ambiguous` を立て、名前を保ったまま横断からの参照を評価不能にする。2 つ目からの `ProfAlign`、円の縦断曲線（`CircCurve`）と対称でない放物線は、`landxml.element.unsupported` を返す。追加距離が増えない変化点と、重なる縦断曲線は、縦断線形を移さずに `landxml.value.invalid` を返す。
- Ver.1.8 の 4-3-37・38 に従い、`ProfSurf` の各 `PntList2D` を別の `GroundProfile` として保持する。名前・注記・追加距離と標高・リストの元の位置を残し、別のリスト間をつながない。点列の数が奇数、非数・非有限の値、追加距離の減少があればそのリスト全体を診断して除く。

## 片勾配

- Ver.1.8 の 4-3-12・24〜32 に従い、最大区間・すりつけ・任意横断勾配を Rust の `Alignment.cant` へ移す。距離は線形の `staStart` からの追加距離、勾配は % のままとする。左右とも左下がりが負、右下がりが正で、左右の外側を正にする値へ変えない。
- 最大区間の初めは `FullSuperSta`、終わりは `RunoffSta` を優先し、子がないときだけ `staStart`・`staEnd` を使う。`FullSuperelev` を両側の初めと終わりに持ち、`transition` は指定しない。優先する子が不正でも属性へ戻らない。
- `SuperelevationConfig` の `singleLaneRoad` は省略時に多車線とし、`normalCrown` を直線部の勾配にする。多車線では非負の値を左に負・右に正として置き、一車線では明記された符号の値を両側へ置く。線形全体が直線部勾配だけのときは、正の `Alignment.length` の全区間へ移す。
- 基本型は `BeginRunoutSta`・`BeginRunoffSta`・`FullSuperSta` と、終わり側の `RunoffSta`・`StartofRunoutSta`・`EndofRunoutSta` で区切る。待ち勾配ありの多車線では、外側の勾配が 0 になる点を `BeginRunoffSta`・`StartofRunoutSta` に置く。内側は直線部勾配を保ち、両側が同じ勾配になってから最大値へすりつける。`ReverseCrown` があればその変化点を使い、なければ外側の直線のすりつけが直線部勾配に達する位置を求める。
- `AdverseSE=non-adverse` では、直線部と最大区間の間を両側とも線形に変える。一車線も左右を同じ値にする。必要な変化点がないものや、順序が逆のものから区間を補わない。読める最大区間は保ち、移せない子の種類・件数・元の場所を診断する。
- 直線部へ戻る終点・始点を持たない隣の最大区間は、S 型・卵型としてつなぐ。`ReverseCrown` の待ち勾配と `FlatSta` の 0% を変化点にし、その間を線形にする。S 型で待ち勾配の変化点を指定したのに `FlatSta` がない場合、重なる測点、範囲外や逆順の変化点は接続を作らない。隣の曲線がない `FlatSta` なども診断する。
- `SlopeList` がある場合は、文書の順によらず `Superelevation` を置き換える。`useSlopeList=true` または判別できない指定でも最大区間へ戻らない。`slopeValue` は一車線なら測点・勾配、多車線なら測点・左・右の順で読み、隣の変化点を線形の区間へ移す。測点は厳密な昇順とし、1 点だけ、読めない値、重複・逆順の点を除いて部分的につなぐことはしない。
- すりつけと任意横断勾配の `transition` は `linear`。出所は元の `Superelevation`、曲線間の接続は手前の `Superelevation`、任意横断勾配は区間の初めの `Property` とする。明記された区間の外へ延長しない。同じ `FullSuperSta`・`RunoffSta`・`FullSuperelev` が複数あるものは、対応を推測せず診断する。
- 値は `LandXmlFile::lift` の `Alignment.cant` と、共通 API の `civil` に `details: true` を渡して取得する。開始・終了の左右の値、距離・長さ・変わり方・元の位置を返す。

## 横断

- `CrossSects` の `CrossSect` を、`sta` の追加距離の横断にし、`name`（`No.10+0.0000` など）を名前にする。
- `DesignCrossSectSurf` を計画の横断の線にし、`name`（`Carriageway` など）を名前にする。点は `CrossSectPnt` の「離れと標高」の 2 つの数（`dataFormat` が `OffsetElevation`、書いていないか `Offset Elevation`）で、離れは中心線から右を正とする。
- `heightType` が `fhOffset` なら、高さを計画高からの差のまま `CrossSectionHeight::FormationOffset` として保持する。`elevation` または省略は文書の標高の `Elevation`。未知の値・複数指定は診断して線全体を除く。勾配と距離（`SlopeDistance`）は引き続き `landxml.cross-section-line.unsupported` で知らせて除く。
- `CrossSects` の `profAlignRefs` を横断の `profile` に保持する。作図では `zumen_civil::cross_section_height_offset` が参照先の名前と追加距離を確かめ、その所の計画高を相対高さに足す。参照の重複・欠落、範囲外、評価不能は `civil.cross-section-height.unsupported` で知らせて描かない。XML の子の順序には依存しない。文書の高さの基準自体は変えない。
- `CrossSectSurf` は、`PntList2D` ごとに横断の線にし、`ExistingGround` を現況の線、ほか（`SoilLine-n` など）を元の名前の役割にする。
- `desc`（`道路面` などの面の種類）・`side` を元の文字のまま保持する。`side` で点の離れの符号を変えない。幅員中心・数量など、それ以外の `Feature` は未対応として診断する。

## 地表面

- `Surfaces` の `Surface` を TIN にする。ID は `name` で作る。`desc` の `ExistingGround` を現況、`FinishedGrade` を計画、ほかを元の値の役割にする。
- `Definition`（`surfType` が `TIN`。交換標準の本文の `SurfType` も読む）の `Pnts` の `P`（`id` と「X Y Z」）を点に、`Faces` の `F`（3 つの点の `id`）を三角形にする。見えない面（`i` が `1`）は移さず、数を `landxml.face.dropped` で返す。
- `Feature`（`AreaPnts`）の `areaList` を記載順の境界線にする。読めない点番号があれば、その `Property` の入力位置と `areaList` を `landxml.value.invalid` で知らせ、境界線全体を作らない。未定義の点番号でも境界線を作らない。どちらの場合も、正しく読める地表面の点と三角形は保つ。
- 知らない点を指す面、重なる `id` の点があれば、その地表面を移さずに `landxml.value.invalid` を返す。

## 移さない要素

- `Roadways` は、まだ移さず、`landxml.element.unsupported` を返す。
- 単位（`Units`）は、交換標準がメートルと決めているので読まない。方向角の属性は交換標準にないので、角の単位は使わない。

## 上限

- 入力のバイト数と、移す要素（区間・変化点・横断・点・面）の数を `elements` で予算で数える。
- 座標参照系・座標点セット・IP リスト・地盤縦断・横断セットは、各対象の原本バイト数の 64 倍を `work`、32 倍に 1,024 バイトを足した量を `decoded-bytes` に先に数える。子の列・文字の復号・数値列・参照の索引・モデルの確保を含む。同じ種類名や参照名を複数の点・線へ複写するときは、その文字数も複写前に追加で数える。
- 片勾配の設定を読む前に、線形の子の数の 4 倍を `work`、64 倍を `decoded-bytes` に数える。子の `Feature` と `Superelevation` ごとに、入力のバイト数を作業量、その 32 倍に 1,024 バイトを足した量を確保量として先に数える。変化点・区間・文字の復号・隣の曲線の照合もこの論理量に含める。さらに、片勾配の置換の指定を調べる線形の子の数と、片勾配の子・数の文字の処理を `work` で数える。片勾配の一時の文字列と区間を確保する前に、元の要素のバイト数の 8 倍に 512 バイトを足した量を `decoded-bytes` に数える。置換の指定を調べる `Feature` は、元の要素のバイト数の 4 倍を先に数える。これらはネイティブと wasm32 で共通の量とする。上限で止まれば元の要素を指し、部分結果を返さない。
- `PI` のない `Spiral` は、前の区間の終わりの向きを、平面線形を初めからたどって求める。そのたどる区間の数を `work` で数える（#560）。区間の終わりの点を求める手間（緩和曲線の向きの式を積む刻みの数。1 つの区間で 65,538 まで）も、求める前に `work` で数え、上限で止まれば `Spiral` の所で予算の診断を返す（#727）。

## 確かめた度合い

UTF-16 は自作の XML と J-LandXML で、原本範囲・未変更の書き出し・線形と一定の片勾配・診断と予算を検査します。[独立検査](../../xtask/verify/xml/README.md)で、ネイティブと WASI の公開値を比較します。Expat との比較は XML の値と要素の位置だけで、J-LandXML の領域モデルの外部照合には含めません。

自作の Shift_JIS で、日本語の未知の要素名を診断に返し、原本・診断の範囲・既存の線形の ID を保つことを検査します。原本の `RecordInfo.kind` は復号しません。この確認は `synthetic` です。

| 形式 | 向き | 段 |
| --- | --- | --- |
| `landxml` | 読み | `synthetic` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-landxml)にあります。この文書の規則は、仕様と、テストの中で作ったファイルだけで確かめたものです。公開サンプル・受け取った図面・外部の実装・基準になるソフトとは、まだ照らしていません。実際のファイルでは、ここに書いた扱いと違うことがあります。

- 再配布を認める公開サンプルは、見つかっていない（#368）。公開サンプルと外部の実装での照合は #504 で行う。
- 片勾配の一定区間、基本型・S 型・卵型のすりつけ、一車線と多車線、任意横断勾配は自作の小さな入力で、優先順位・符号・単位・出所と、移さない要素の診断を確かめる。外部の実装や実際のファイルとの一致は、まだ確認していない。
- [実行環境間の比較](../../xtask/verify/landxml/README.md)では、自作の 13 入力について、片勾配の値のビット列・出所・診断・全 `Kind` の予算と停止結果を、ネイティブと `wasm32-wasip1` で比較する。API の適合テストは、開いたときの未対応要素の診断と `civil` の応答を別に比較する。
- IP・基準点・地盤縦断・横断属性・高さの基準は `values_contract` の自作入力で、保持値・出所・全予算の境界と中断を検査する。`civil_values.json` は各言語と配布用 wasm の応答を照合する。既存の作図と SIMA へまだ出せない保持値は `civil.value.dropped` で種類・件数・場所を知らせる。これらも外部実装との照合ではない。

## Rust のモデルの移行

0.3.0 では `CivilModel`・`Alignment`・`CrossSection`・`CrossSectionLine` にフィールドを追加する。構造体リテラルの利用側は、前の 2 型に `..Default::default()` を使える。`CrossSection.profile` は元の指定がなければ `None`、既存の標高を持つ `CrossSectionLine` は `height_reference: CrossSectionHeight::Elevation`、`description`・`side` は未指定なら `None`、`source` は出所を渡す。相対高さの線を標高として使わず、上記の評価関数を通す。線形と地表面の ID の規則は変えない。
