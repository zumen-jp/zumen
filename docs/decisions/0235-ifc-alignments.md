# 0235: IFC4.3 の線形は、入れ子の区間を書いた順に読み、配置と IfcMapConversion で地図の空間に直して土木の領域モデルに移す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-02 |
| 状態 | 採用 |

## 背景

段階 3（[#304](https://github.com/zumen-jp/zumen/issues/304)）は、J-LandXML と IFC4.3 の線形を、同じ土木の領域モデル（[決定 0230](0230-civil-model-alignments.md)）で受ける（#375）。J-LandXML は #368 で移した。IFC4.3 は、`IfcAlignment` の下に平面・縦断・カントの配置を入れ子（`IfcRelNests`）で置き、その下に区間を並べる。区間は、初めの点・向き・半径・長さで書く。IFC の L1（#407）は、スキーマを知らずにインスタンスを読むので、意味を持つ属性の位置は手で書く（[決定 0215](0215-ifc-building-lift.md) と同じ）。

決めることは、次のとおり。

- 半径・角・長さの単位と符号を、モデルの約束にどう合わせるか。
- 測点と、点を置く空間をどう読むか。
- 移せない区間と形を、どう返すか。

## 選択肢

1. **区間を書いた順に読み、配置と `IfcMapConversion` で直す。** 区間の値を、モデルの約束（右へ曲がる半径を正、方向角は北から時計回り、地図の空間のメートル）に直して持つ。
2. **区間の形を評価して、点の列にする。** 種類を問わずに受けられるが、曲線の種類を落とす（決定 0018 に反する）。

## 決定

1 にする。属性の位置は、IFC 4.3 ADD2 の実体の定義（「属性」の節）による。

- **版。** `FILE_SCHEMA` が IFC4X3 のファイルだけを移す。ほかの版は `ifc.version.unsupported` を返す。IFC4X1 の `IfcAlignmentCurve` は扱わない。
- **入れ子。** `IfcAlignment` の入れ子の子の `IfcAlignmentHorizontal`・`IfcAlignmentVertical`・`IfcAlignmentCant` の、入れ子の子の `IfcAlignmentSegment` の 8 DesignParameters を、書いた順に読む。長さ 0 の区間は、線形の終わりを示す印なので移さない。
- **平面。** `IfcAlignmentHorizontalSegment` の種類を、次のように対応させる。半径は、IFC では左へ曲がるものを正、0 を直線とするので、符号を反転し、0 を `None` にする。
  - `LINE` は直線、`CIRCULARARC` は円弧にする。
  - `CLOTHOID` はクロソイド、`CUBIC` は 3 次放物線にする。
  - `HELMERTCURVE` はヘルマート曲線、`BLOSSCURVE` はブロス曲線にする。
  - `COSINECURVE` はコサイン曲線、`SINECURVE` はサイン半波長逓減曲線、`VIENNESEBEND` はウィーン曲線にする。
  - ほかの種類は、`ifc.alignment-segment.unsupported` を返して移さない。
- **縦断。** `IfcAlignmentVerticalSegment` の `CONSTANTGRADIENT`・`PARABOLICARC`・`CIRCULARARC` を移す。円弧の半径の符号は、IFC の符号に頼らず、勾配が減るもの（上に凸）を正にする。縦断のクロソイドは、モデルの半径の持ち方と合うかを試料で確かめるまで、`ifc.alignment-segment.unsupported` を返す。
- **カント。** `IfcAlignmentCantSegment` を、鉄道のカント（mm）にする。軌間は `IfcAlignmentCant` の 8 RailHeadDistance、変わり方は種類の名前のまま持つ。
- **測点。** 線形の入れ子の子の `IfcReferent`（`STATION`）の `Pset_Stationing` を読む。`IncomingStation` のない初めのものが初めの測点を決め、`IncomingStation` のあるものを測点の不連続にする。距離は `IfcLinearPlacement` の `IfcPointByDistanceExpression` の 1 DistanceAlong から取る。測点がなければ、初めの測点を 0 にして `ifc.station.missing` を返す。
- **単位。** `IfcProject` の単位の割り当ての長さと平面角を読む。SI の単位と接頭辞、`IfcConversionBasedUnit`（度など）を扱う。解けなければ、メートルかラジアンとして `ifc.civil-unit.assumed` を返す。
- **空間。** 線形の配置（`IfcLocalPlacement` の連鎖）を、平面の移動と回転として解く。鉛直でない軸と、ほかの配置は解けないので、`ifc.placement.unsupported` を返して原点に置く。プロジェクトの空間の点は、初めの `IfcMapConversion` で地図の空間に直す（決定 0019）。
  - 座標系は `IfcProjectedCRS` の 1 Name、高さの基準は 4 VerticalDatum から読む。
  - `IfcMapConversion` がなければ、座標系を不明にして `ifc.map-crs.missing` を返す（決定 0230）。
- **ID。** 線形の ID は `GlobalId` にする。ないときは、インスタンスの番号から作り、`ifc.global-id.missing` を返す。
- **地表面。** `IfcTriangulatedIrregularNetwork` を TIN にする（[#477](https://github.com/zumen-jp/zumen/issues/477)）。
  - 点は 1 Coordinates の `IfcCartesianPointList3D`、三角形は 4 CoordIndex から読む。5 PnIndex があれば、その番号で点を並べ直す。
  - 形を持つ要素（`IfcShapeRepresentation` → `IfcProductDefinitionShape` → 要素）の配置で直し、`IfcMapConversion` で地図の空間に置く。要素が見つからなければ、`ifc.placement.unsupported` を返して、プロジェクトの空間の点のまま置く。
  - ID と名前は、その要素の `GlobalId` と名前にする。役割は、`IfcGeographicElement` の `TERRAIN` を現況にし、ほかは種類の名前のまま持つ。
  - 三角形ごとの印（6 Flags）は、意味を試料で確かめるまで読まない。0 でない印があれば `ifc.civil-geometry.dropped` を返し、三角形はすべて移す。
- **横断。** 横断の形（`IfcOpenCrossProfileDef`・`IfcSectionedSolidHorizontal`・`IfcSectionedSurface`）は、まだ移さず、`ifc.civil-geometry.dropped` を返す。
  - 開いた断面（`IfcOpenCrossProfileDef`）は、幅と勾配の列から横断の線（中心からの離れと高さ）を作れる。ただし、勾配の符号と、断面の位置（`IfcAxis2PlacementLinear`）を線形の追加距離に結ぶ書き方を、試料で確かめてから移す。
  - 閉じた断面（舗装の層などの立体）は、モデルの横断の線が開いた線なので移さない。形が要るときは、3D のシーンの側で扱う。
- **境界。** `open` は、IFC-SPF を建物モデルとして開き、`IfcAlignment` があれば土木の領域モデルにもする。`civil`・`deviation` の op は、その土木の領域モデルを使う。土木の op は描画のモジュールにあるので、BIM と描画の両方を持つビルドだけで使える。モジュールの分け方は #380 で決める。

## 影響

- 同じ線形の J-LandXML と IFC4.3 が、同じ区間・測点・座標系になることを、`zumen-ifc` の試験で確かめる。
- `zumen-ifc` に、土木の領域モデルへ移す機能（`civil`）を足す。`IfcFile` が建物モデルと土木の領域モデルの 2 つに移せるので、呼ぶ側は型を名指す。
- 試料の IFC4.3 の線形は、まだない。試料を得たら、半径の符号・縦断のクロソイド・測点の書き方・三角形の印・開いた断面の勾配の符号を確かめる。

## 参照

- [決定 0008](0008-model-layers-and-codecs.md)・[0019](0019-coordinate-systems-and-gsi-data.md)・[0215](0215-ifc-building-lift.md)・[0230](0230-civil-model-alignments.md)
- IFC 4.3.2（buildingSMART International）
