# SIMA

日本測量機器工業会（JSIMA）の SIMA 共通フォーマット（カンマ区切り `.sim` と、無償公開の JPGIS 版 XML Schema `jsima_jpgis_1_0_2`・`jpgis_2_1`）を読み書きし、図面（`zumen-drawing`）・土木（`zumen-civil`）・地理（`zumen-geo`）の 3 つの領域モデルと変換する規則を記録します。実装は `crates/zumen-sima` です。有償の冊子（`jsima_201_2026`）は入手・参照せず、JPGIS 版スキーマと寛容なライセンスの公開実装（`sima2geojson.rb`、MIT）・慣用の区切り形式の推定で実装しています（`formats/ledger.toml`、`#374`）。スキーマは同梱しません。

## L1 と見分け・予算

- 先頭が `G00,`（UTF-8 BOM の直後を含む）のカンマ区切りテキストと、根要素が `GI`（名前空間 `http://www.jsima.or.jp/JSIMASchema/201206` または `http://www.jsima.or.jp/JSIMASchema/201004`）の XML を `SIGNATURES` で見分ける。
- カンマ区切り `.sim`:
  - 文字コードは UTF-8（BOM あり・なし）であれば UTF-8、そうでなければ Shift_JIS（WHATWG の定義、`encoding_rs`）として行ごとに戻す。
  - 最初の非空行のデータ区分が `G00` でなければ `sima.file.invalid` を報告して失敗する。
  - L1 のレコード（`FileModel`）は各非空行とし、元のバイト範囲を重ならない昇順で持つ。
- JPGIS 版 XML:
  - `zumen-xml`（UTF-8・Shift_JIS・UTF-16LE・UTF-16BE 対応、BOM の有無を問わない）で読む。根要素が上記の `GI` でなければ `sima.file.invalid` を報告して失敗する。
  - L1 のレコードは、`GI` 直下の `exchangeMetadata`、`dataset` 直下の `GenbaJoho`、および `object` 要素直下の各オブジェクト要素（空のときは外側の要素）とし、互いに重ならない昇順のバイト範囲を持つ。
- 読み取り・領域モデルへの変換・書き出しはすべて `Cx` の予算の下で行い、入力長を `InputBytes`、戻した文字列や中間構造・出力バイト列の確保を `DecodedBytes`、L1 のレコードを `Records`、ブロックや XML 参照・入れ子要素を `Depth`、折れ線近似の点数を `Segments`、走査と積分計算を `Work`、移した要素を `Elements` で数える。予算超過や中断で止まるときは、扱っていた場所を付けた診断を報告して止まる。
- L1 モデルを変えずに `Codec::write` すると、元のバイト列をそのまま返す。

## 座標の軸と型（決定 0019）

- 測量の平面直角座標は **第 1 座標（X）が北（`northing`）、第 2 座標（Y）が東（`easting`）** であり、単位はメートルとする。
- 読み取り時は必ず `Axes::NorthEast.map_point([x, y], 1.0)` を通して地図空間の `MapPoint` に変換し、`-0.0` は `0.0` に正規化する。有限でない実数は `sima.value.invalid` を報告して退ける。
- 図面の領域モデル（`Drawing2D`）へ移すときは、実寸空間（`Unit::Meter`、`Axes::EastNorth`）の点 `ModelPoint { x: easting, y: northing }` に写すことで、画面や SVG・PNG 描画で東が右・北が上になるようにする。書き出し時は空間の軸（`Axes::EastNorth`・`Axes::NorthEast`）と円弧の向きを考慮して北（X）・東（Y）へ戻す。

## カンマ区切り `.sim` の読み方（推定・公開実装参照）

各行を `,` で区切り、先頭の空白を除いた第 1 列をデータ区分とする。`sima2geojson.rb`（MIT）から裏付けられる基本列（`G00` の版番号・現場名、`Z00`、`A00`/`A01`/`A99`、`D00`/`B01`/`D99`）以外の詳細規則は推定であり、項目ごとに `（推定）` を付記する。

- `G00`: ファイル属性。第 2 列を版番号、第 3 列を現場名とする。第 4 列以降に平面直角座標系（I〜XIX 系）の名前・系番号または対応する `EPSG:<code>` が書かれていれば `Crs` として読み、平面直角座標以外の座標系名は `sima.crs.unsupported` を報告する（推定。通常の `.sim` は座標系を持たないため `None` になる）。
- `Z00`: コメント行。そのまま読み飛ばす。
- `A00`〜`A99`: 座標ブロック。
  - `A01,<点番号>,<点名>,<X(北)>,<Y(東)>,<Z(標高)>,`: 測点として登録する。標高が空欄のときは `None` にする。同じ `(点番号, 点名)` が再び現れたときは既存の測点を後の定義で更新する（推定）。
- `D00`〜`D99`: 画地（区画・折れ線）ブロック。
  - `D00,<画地番号>,<画地名>,<閉合区分>,`: 閉合区分が `"2"` のときは開放折れ線（`KaihouKakuchi`）、`"1"` または省略のときは閉じた区画（`Kakuchi`）とする（推定）。
  - ブロック内の `B01,<点番号>,<点名>,` で構成点を順に引く。まず `(点番号, 点名)` の完全一致を探し、見つからなければ点番号のみ（点番号が空なら点名のみ）で引く（フォールバック検索は推定）。見つからない参照は `sima.point.missing` を報告して飛ばす。閉じた区画で 3 点未満、開放折れ線で 2 点未満のときは `sima.geometry.dropped` を報告して省く。
- `F00`〜`F99`・`C00`〜`C99`: 路線・要素区間ブロック（推定）。
  - `F00,<路線番号>,<路線名>,<開始測点>,`（または第 2 列のみの名前）で路線を始め、`F01`〜`F03` の路線属性を保持し、`F99` で閉じる。`F00` の外に単独で置かれた `C00`〜`C99` も 1 つの路線区間として扱う（推定）。
  - `B01`・`B03` で構成点（始点・終点・通過点）を引き、`C01,<半径 R>,` で単曲線の半径（進行方向に対して右カーブが正、左カーブが負、推定）、`C02,<パラメーター A>,<曲線長>,<始端 R>,<終端 R>,`（第 3〜5 列は任意、推定）でクロソイドのパラメーターと始端接線角を復元する。
- `C03`（緩和曲線種別拡張など）を含む上記以外のデータ区分は `sima.record.unsupported` を報告して読み飛ばす。

## JPGIS 版 XML（`jsima102.xsd`）の読み方

- `GenbaJoho`: `Name`（現場名）、`CoordinateSystem`（`"1"`〜`"19"` → 平面直角座標系 I〜XIX 系）、`Crs`（測地系コード。省略または `3` は JGD2011、`4` は JGD2024、`1` は JGD2000、`2` は日本測地系とする。`Crs` の番号対応は推定）、および `Worker`・`Client`・`Engineer`・`Start`・`End`・`Comment` を読む。扱えない系番号や測地系コードは `sima.crs.unsupported` を報告する。
- 空間オブジェクト（`http://www.gsi.go.jp/GIS/jpgis/standardSchemas2.1_2009-05`）:
  - `GM_Point`: `position`・`coordinate` の空白区切り数値列から `X Y`（および任意の `Z`）を読む。
  - `GM_Curve`・`GM_OrientableCurve`・`GM_CompositeCurve`: `orientation`（`+`・`-`）、`primitive` 参照、`generator` 参照、および `segment`（`GM_LineString`・`GM_LineSegment`・`GM_Arc`・`GM_ArcString`・`GM_Circle` の `controlPoint`・`column` の `direct` と `indirect`）を再帰深さ上限のもとで解決する。3 点円弧（`GM_Arc`）は始点・通過点・終点から中心と符号付き半径を求め、`GM_Circle` は 3 点を通る外接円の全周（半円 2 区間）に展開する。未対応の曲線セグメント要素は `sima.record.unsupported` を報告する。
  - `GM_Surface`: `patch`（`GM_Polygon`・`GM_Triangle`）・`boundary` の `exterior` と `interior`（`GM_Ring`）を解決し、外周と穴の輪を得る。輪の中に円弧区間が含まれるときは許容差で折れ線に近似し、`sima.curve.approximated` を報告する。
  - オブジェクト参照は `idref` 属性と `xlink:href`（`#id` または `id`）の両方を受け付ける。参照先がないときは `sima.point.missing` を報告する。
- 地物オブジェクト:
  - `Sokuten`（測点）: `Number`・`Name`・`Altitude`・`RefPoint`。
  - `Kijyunten`（基準点）・`Suijyunten`（水準点）: `RefSokuten`（参照または入れ子の `Sokuten`）の位置を複製した独立の要素として保持し、等級（`Class`）・経緯度（`B`・`L`）・ジオイド高（`geoidH`）・平均標高（`AveAltitude`）・観測日（`obsDay`）などを属性として付ける。
  - `Kakuchi`（画地）: `Number`・`Name`・`RefSurface`（閉じた区画）。
  - `KaihouKakuchi`（開放画地）: `Number`・`Name`・`RefCurve`（開いた折れ線・曲線）。
  - `Chiban`（地番）: `RefKakuchi`（参照または入れ子の `Kakuchi`）の区画を複製した独立の要素として保持し、地目（`Chimoku`・`KChimoku`）・大字（`OAza`）・字（`Aza`）・面積（`Area`・`KArea`）・所有者（`Owner`）・不動産番号（`Regist`）・備考（`Comment`）・回り方向（`Rotation`）を属性として付ける。

## 3 つの領域モデルへの変換と書き出し

- 図面（`Drawing2D`）:
  - レイヤ `測点`・`画地`・`路線` を作り、測点を十字＋丸の点要素（`Point`）と点名ラベル（`Text`）、画地・地番・開放画地を折れ線（`Polyline`、穴があれば穴の閉じた折れ線も追加）、路線を線分・円弧（`Edge::Arc`）・クロソイド（`Edge::Clothoid`、曲率 0 の変曲点原点）として配置する。
  - 測点番号・属性・現場情報は `sima.attribute` 名前空間の `Extension` として要素と文書に結び付ける。
- 土木（`CivilModel`）:
  - 路線（および曲率区間を持つ開放曲線）を `Alignment`（`HorizontalKind::Line`・`Arc`・`Transition`）に移し、既知の座標系があれば `crs` と `realization` に設定する。座標系が未指定なら `sima.crs.missing` を報告する。
- 地理（`GeoModel`）:
  - 測点・基準点・水準点・画地・開放画地・地番・路線を `Feature` に移す。円弧・クロソイドを含む路線や開放画地は許容差で折れ線に標本化し、`sima.curve.approximated` を報告する。
  - `GeoModel` の不変条件に従い、座標系が判明しているとき（JPGIS 版の `GenbaJoho` など）だけ `Form::Point`・`Form::Line`・`Form::Area` の幾何を持たせ、座標系が不明なとき（通常の `.sim` など）は `sima.crs.missing` を報告して属性のみを持つ地物にする。
- 書き出し（`lower_*_to_sima`）:
  - `SimaFile`・`Drawing2D`・`CivilModel`・`GeoModel` から UTF-8（CRLF 改行）のカンマ区切り `.sim`（`G00`、`A00`/`A01`/`A99`、`D00`/`B01`/`D99`、`F00`/`B01`/`C00`/`C01`/`C02`/`C99`/`F99`、`G00` の座標系列と `F00`/`C00` 出力形式は推定）を決定的に書き出す。中心角が 180° を超える優弧は複数の `C01` 区間に分割して出力し、膨らみ（`bulge`）を持つ折れ線は折れ線近似して `sima.curve.approximated` を報告する。座標系が経緯度（WGS 84・JGD2011 経緯度）など平面直角座標でないときは `sima.write-crs.unsupported` を報告して失敗する。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `sima` | 読み | `synthetic` |
| `sima` | 書き | `synthetic` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-sima)にあります。

- 合成テストの範囲（読み・書きとも `synthetic`）: テストの中で組み立てたカンマ区切り `.sim`（Shift_JIS・UTF-8・UTF-8 BOM）と JPGIS 版 XML（`jsima102.xsd` の全地物種別と空間参照）を `crates/zumen-sima/tests/sima.rs` と `bindings/conformance/cases/sima.json` で読み書きし、北（X）・東（Y）の座標軸変換、3 領域モデルへの変換と書き出し往復、診断、レコード範囲、決定性、6 種の予算と中断を確かめた。再配布できる公開サンプルの収集と外部ソフトでの照合は `#504` で扱う。
