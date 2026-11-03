# 0260: 試料の IFC で測り、3D のカーネルを manifold-rust にして、建物モデルの残りの 7 項目を決める

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-03 |
| 状態 | 採用 |

## 背景

[決定 0207](0207-building-model-components.md) は、建物モデルの 7 項目を、日本の BIM ソフトが書き出した IFC を得てから決めるとした（「試料を得てから決めること」）。[決定 0222](0222-geom3d-types-and-kernel-trait.md) は、3D のカーネルの第一の候補を manifold-rust にし、試料での失敗の割合・時間・メモリ・Linux と Windows での結果の一致を測ってから選ぶとした。[決定 0018](0018-geometry-layers-and-tolerances.md) は、3D の許容差の既定値を、文書の精度があればそれにし、なければカーネルを選ぶときに測って決めるとした。

#412 で、公開サンプルの IFC 12 件を `samples/ifc/` に入れた。Civil 3D 2024 日本語版（中野区）・Revit 2021（BIM Whale）・Vectorworks Architect 2024（Text2BIM）の書き出しと、公開元が保存し直した Revit 2024 日本語版（林野庁）・Revit 2024 と 2025・Archicad 28（GNI）の書き出しで、IFC2X3・IFC4・IFC4X3 を含む。保存し直したものは、書き出したソフトがそのまま書いたファイルとしては扱わない（`samples/README.md`）。

国内のソフトの書き出し 9 件（#513 の B01〜B09）は、公開の許諾がないので Git に入れず、手元で数えた件数と種類（実体・属性の有無・書き方）だけを書き、記録された値（名前・数値・文字列）は書かない。CADEWA Real2011・ARCHICAD 20 日本語版・ArchiCAD 15 日本語版・GLOOBE Architect 2024・Rebro2025・CADWe'll Linx V6・CADEWA Smart V5.0・FILDER CeeD の書き出しで、どれも IFC2X3、0.7〜245 MB である。

測り方と結果の数は [bench/kernels/README.md](../../bench/kernels/README.md) の「公開サンプルの IFC で測る」に、書き出したソフトごとの違いは [docs/formats/ifc.md](../formats/ifc.md) の「書き出したソフトごとの違い」にある。測った manifold-rust は 0.19.1 である（決定 0018 の表は、0.18.0 のときのもの）。形・時間・メモリは `bench/kernels` の `samples` で測り、この決定の 3〜5 と docs/formats/ifc.md の数（階・室の形・文字列・建具の開き方など）は、IfcOpenShell と使い捨ての道具で数えた（道具はこのリポジトリに入れていない）。

## 選択肢

カーネルは、次から選ぶ。

1. manifold-rust（Apache-2.0。C++ の Manifold の Rust への移植）
2. csgrs（MIT。BSP の木）
3. 自前の、断面の 2D のブーリアン（i_overlay）だけ
4. manifold-rust に、自前の 2D のブーリアンを速い道として足す

Shift_JIS をエスケープで書いた文字列の読み方は、次から選ぶ。

1. ISO 10303-21 のとおりに読む（`\X\` と `\S\` は ISO 8859-1）
2. 書き出したソフトの名前（見出しの `FILE_NAME`）で、Shift_JIS として読むかを分ける
3. ファイルの中の手がかりで、Shift_JIS として読むかを分ける
4. 利用者が引数で文字コードを指定する

室の形が内法か壁芯かは、次から選ぶ。

1. 書き出したソフトの名前で決める
2. 室の形の頂点と、壁の中心線の距離で判定する
3. 判定せず、利用者が引数で指定する

形の項目をいつ写すかは、次から選ぶ。

1. lift で、すべての要素の形の項目を写す
2. 要るときに、要素ごとに写す

## 決定

### 1. 意味を手で書く実体と、形の項目の一覧

形の項目は、次を `zumen-building` に持つ（#419）。21 件の `Body` と開口の形からたどれた項目で、試作（`bench/kernels/src/ifc.rs`）は、公開サンプルの 12 件の `Body` と開口の形をすべて写せた。

| 種類 | 項目 |
| --- | --- |
| 立体・面 | `IfcExtrudedAreaSolid`・`IfcFacetedBrep`・`IfcShellBasedSurfaceModel`・`IfcFaceBasedSurfaceModel`・`IfcPolygonalFaceSet` |
| ブーリアン | `IfcBooleanResult`・`IfcBooleanClippingResult`・`IfcHalfSpaceSolid`・`IfcPolygonalBoundedHalfSpace` |
| 写像 | `IfcMappedItem`・`IfcCartesianTransformationOperator3D`・`IfcCartesianTransformationOperator3DnonUniform`（GLOOBE 2024 だけ） |
| 断面 | `IfcRectangleProfileDef`・`IfcCircleProfileDef`・`IfcArbitraryClosedProfileDef`・`IfcArbitraryProfileDefWithVoids`・`IfcIShapeProfileDef`・`IfcRectangleHollowProfileDef` |
| 曲線 | `IfcPolyline`・`IfcIndexedPolyCurve`・`IfcCompositeCurve`・`IfcTrimmedCurve`・`IfcCircle` |

- `Axis`・`FootPrint` の 2D の形は、`IfcGeometricCurveSet`・`IfcGeometricSet` と、上の曲線（`IfcPolyline`・`IfcIndexedPolyCurve`）だった。壁の中心線（4）と、平面図の切り出し（#422）に使うので、これも持つ。
- 国内の書き出しの形は、面の集合（`IfcShellBasedSurfaceModel` が 13.6 万、`IfcFacetedBrep` が 1.4 万）と、写像・押し出しが中心だった。`IfcIShapeProfileDef` と `IfcRectangleHollowProfileDef` は、GLOOBE 2024 の柱（59 要素）だけが使い、試作は写していない。
- 21 件になかったが、型を持つもの: `IfcTriangulatedFaceSet`（IFC4 から）・`IfcBoxedHalfSpace`・`IfcLine`（試作は写せる）と、回転（`IfcRevolvedAreaSolid`）・円の断面の掃引（`IfcSweptDiskSolid`。カーネルが評価する種類）。試験は最小の IFC で作る。B スプライン・`IfcAdvancedBrep` もなかった。一覧にない項目は、種類と出所だけを持ち、評価するときに診断を返す（#419）。

意味を手で書く実体は、lift（#409）が試料で写さずに捨てた実体（診断 `ifc.entity.dropped`）のうち、次を足す。試料で多い順に並べる。

1. 型（`IfcTypeObject` を継ぐもの）と、型に付いたプロパティのセット（`HasPropertySets`）。国内の設備の書き出しのうち 3 件（Rebro2025・CADWe'll Linx V6・CADEWA Smart V5.0）は、`IfcPipeSegmentType`・`IfcDuctFittingType`・`IfcAirTerminalType` など 20 種類ほどの型を使い、`IfcRelDefinesByType` が 1 件に 1,500〜2,700 あった。型を写さないと、型に付いたプロパティのセットも一緒に消える。
2. 設備の要素。IFC2X3 の `IfcFlowController`・`IfcFlowMovingDevice`・`IfcFlowTreatmentDevice`・`IfcEnergyConversionDevice`・`IfcElectricDistributionPoint`・`IfcDiscreteAccessory` と、`IfcTransportElement`・`IfcBuildingElementPart`・`IfcDistributionFlowElement`。
3. 材料の構成（`IfcMaterialLayer`・`IfcMaterialLayerSet`・`IfcMaterialLayerSetUsage`・`IfcMaterialConstituentSet`・`IfcMaterialProfileSet`）と `IfcMaterialProperties`。
4. プロパティの値: `IfcPropertyListValue`（国内の設備の書き出しで 1 件に 86〜4,683）・`IfcComplexProperty`・`IfcPropertyEnumeratedValue`・`IfcPropertyBoundedValue`。
5. 単位: `IfcConversionBasedUnit` と `IfcMeasureWithUnit`（FILDER CeeD）、派生の単位（`ifc.unit.missing`）。
6. 関係: `IfcRelConnectsElements`・`IfcRelCoversBldgElements`・`IfcRelAssignsToActor`（GLOOBE 2024）。
7. 建具の枠と扉の値: `IfcDoorLiningProperties`・`IfcDoorPanelProperties`・`IfcWindowLiningProperties`（ArchiCAD 15）。

### 2. カーネル

manifold-rust を選ぶ。

- 公開サンプルの 12 件で、開口のある 171 要素のすべてで、開口を引いた結果が閉じた形になった。パニックと失敗はなかった。
- IfcOpenShell 0.9.0.post1 の形と体積を照らした。閉じた 1,414 要素のうち、体積が 1e-5 m³ 以上の 1,358 要素を比べた。相対の差は、1,255 要素が 1e-6 以下、99 要素が 1e-6 を超え 1e-4 以下、1 要素が 1e-4 を超え 1e-3 以下で、3 要素が 1e-2 を超えた（1e-3 を超え 1e-2 以下はなかった）。3 要素は GNI の床で、部品の 1 つの面が、幅 100 mm の切り込みを、辺を重ねて書いた多角形だった。面の座標から求めた体積は、試作の値と合った（IfcOpenShell がこの面をどう三角形にしたかは、確かめていない）。
- 国内の 9 件でも、パニックせず、途中で止まった試料もなかった。開口のある 227 要素のうち、形を写せた 168 要素の 165 で、開口を引いた結果が閉じた形になった。残る 3 要素は、開口の形が閉じていないので manifold-rust が断った。59 要素は、柱の断面（`IfcIShapeProfileDef`・`IfcRectangleHollowProfileDef`）を試作が写していない。
- 結果は環境で変わらなかった。公開サンプルは、ネイティブの Windows x64 で 2 度と、Linux x86_64（WSL 2 の Ubuntu）で流し、すべての行（候補ごとに 1,874 行）が、座標のビットまで同じだった。国内の 9 件も、Windows と Linux のすべての行（14,943 行）が同じだった。wasm32（Node.js の WASI）も、公開サンプルの manifold-rust のすべての行で同じだった。合成の形の組（78 行）も、この 3 つの環境で同じだった。macOS（arm64・x86_64）は、決定 0222 のときに合成の形で同じだったが、今の試作では流していない。
- 公開サンプルで開口を引く時間は、1 件でネイティブ 0.22 秒以下、wasm32 で 0.28 秒以下だった。
- 切断（決定 0207 の 2）: 閉じた結果を、高さの中ほどの水平の面で切った（`zumen_geom3d::MeshSection`。カーネルによらない）。公開サンプルの 1,414 要素と国内の 13,237 要素のすべてで、失敗とパニックはなかった。切り口がない要素（高さの中ほどに材料がない）は 12、点が 3 つ未満の輪を含む要素（中野区の点群の面の集合）は 18 だった。時間は、1 要素で公開サンプルが 1.6 ms 以下、国内が 26 ms 以下だった。
- 隠れ線（決定 0207 の 2）は、測っていない。決定 0207 の方法（面を見る向きに投影し、手前の面で隠れる所を geom2d のブーリアンで除く）は、カーネルが返したメッシュの上で動き、カーネルの選び方に関わらない。#422 で、平面図を切り出す試作と一緒に、試料で速さと失敗の割合を測る（#422 の完了の条件の 3 つめに足す）。
- csgrs は、開口を引いた 171 要素のうち 4 要素しか閉じず、7 要素でパニックした。wasm ではパニックで試料の全体が止まる。依存の 1 つ（`core2`）は、crates.io で取り下げられている。
- 自前の 2D のブーリアンは、開口のある 171 要素のどれも評価できなかった。壁の開口は押し出しの向きが母体と違い、半空間での切断もある。manifold-rust で十分に速いので、速い道は足さない。

3D の許容差の既定値は、弦の誤差を 1 mm にする。同じ点とみなす距離は、決定 0018 のとおり、文書の精度を使う。IFC では、要素の `Body` の表現の文脈が書いた `Precision`（ファイルの長さの単位）で、下位の文脈（`IfcGeometricRepresentationSubContext`）は親の文脈の精度を使う。精度を書いていなければ、1e-9 m にする（下位の文脈の導出の属性の既定値 1.E-5 は、文書が書いた精度ではないので使わない）。

- 精度は、FILDER CeeD を除く 20 件が書いていた。公開サンプルの 12 件は 1e-5 m〜1e-8 m だった。
- 文書の精度で測った結果は、すべての試料を 1e-9 m で測った結果と比べて、manifold-rust の状態がすべて同じで、体積の差は相対で 7.6e-6 以下だった。IfcOpenShell との体積の差が 1e-6 以下の要素は、1e-9 m では 1,278、文書の精度では 1,255 で、増えた差はどれも 1e-4 以下だった。
- 細い管の断面が粗いなど、ほかの値が要るときは、利用者が許容差を渡す（決定 0018 の「操作」）。

#420 で workspace に入れるとき、次を条件にする。1〜5 は、試料で要った手当てで、試作に入れて測った。6・7 は、コードを読んで足した条件で、試作にはエンジンの明示だけを入れた。

1. ブーリアンの前に、点を、同じ点とみなす距離の格子にそろえる。BIM Whale の TallBuilding の壁は、面が開口の面から 1.6e-13 mm ずれていて、3 つの開口を順に引くと、manifold-rust が 1 つの開口のうち 915 × 200 × 305 mm を引かなかった。そろえると、引く順と、先に開口の和を取るかに関わらず、正しい体積になる。csgrs のパニックも、1e-9 m の格子で 104 から 5 に減った（文書の精度の格子では 7）。
   - 格子は、要素の局所の座標の原点で作り、決定 0018 の 2D の格子のように操作の範囲の隅へ動かさない。同じ要素に開口を順に引いても、前の結果の点が格子から動かない。平面直角座標のまま書いた形（中野区。原点から約 3.6 万 m）でも、1e-8 m の格子の点は f64 で表せる（約 3.6e12 で、2 の 53 乗より十分に小さい）。
   - 格子にそろえるのは、失敗を減らす手当てで、確実に直す手段ではない。格子の境をまたぐ 2 点は 1 つにならない。残る失敗は、決定 0018 のとおり、失敗する前の形と診断で返す。
2. 断面と面を三角形にするとき、T 字の継ぎ目を残さない。耳の切り取り（earcut）は、同じ直線の上の点を落とし、穴の頂点を通る辺を作るので、逆向きの辺がない辺を、その上の点で分ける。面積のない面は、要素を断らずに飛ばし、件数を診断で返す。国内の書き出しの 214 要素（多くは設備の継手）にあった。
3. 開口は、母体の部品ごとに引く。層ごとの壁など、閉じた部品の組を 1 つのメッシュにして引くと、閉じた形にならない。開口に `Body` がなければ、`Reference` を使う（GNI の Archicad 28 の試料は、開口の形を `Reference` に書いていた。公開元が保存し直したファイルなので、Archicad がそう書くかは確かめていない）。
4. 閉じていない形（面の集合で書いた家具・設備・地形など）は、ブーリアンに渡さず、元の形と診断を返す（決定 0018）。
5. 閉じた部分どうしが辺で接する結果（点を 1 つにすると、4 枚の面が 1 本の辺を共有する）は、閉じた形として扱う。`Mesh::is_closed` は 2 様体だけを閉じたとするので、向きつきの辺と逆向きの辺の数が等しいかで確かめる。切り口が、接する辺でも輪になることを試験で確かめる。
6. 環境で結果を変えない（決定 0007）。
   - ブーリアンは、`_with_engine` で `Exact` のエンジンを明示する。既定のエンジンはプロセス全体の設定（`BooleanConfig::set_default_engine`）で、ほかから変えられる。`Robust` のエンジン（`Auto` も選ぶ）は rustc-hash の表を使い、ポインターの幅で反復の順が変わる。
   - 半空間での切断に、`trim_by_plane`・`split_by_plane` を使わない。どちらもエンジンを渡せず、既定のエンジンで積を取る。半空間の箱を作る所（`halfspace`）には、std の `powi` がある（試験を除くと、manifold-rust のソースでここだけ）。ほかの候補と同じく、半空間を大きな箱にして、`Exact` のブーリアンで取る。試作の manifold-rust は、既定が `Exact` であることを確かめてから `trim_by_plane` を呼んで測ったので、#420 で箱の方法に変えて、試料で測り直す。
   - ブーリアンの道筋の `HashMap`・`HashSet`（5 か所）は、決まった順の集まりに直すか、反復の順に結果が依存しないことを確かめる。直した所は記録する。
   - Linux・Windows・wasm32 での一致は、試作で確かめた。書き直した包みでの一致は、#420 の試験を CI の 4 つの環境（Linux x64・macOS arm64・Windows の x64 と x86）と wasm32 で流して確かめる。
7. パニックしうる所（約 650）は、入力を確かめてから渡すこと（有限・点の番号・閉じた 2 様体）で避ける。カーネルの包みに fuzz の対象（形を評価する）を足して確かめる。wasm はパニックで止まるので、包みで止められない。試料でパニックが出たら、直した写しを持つか、上流に直しを出す。上流への報告と直しは、このリポジトリの外への書き込みなので、文案を issue に書いて利用者に頼む。

条件 1〜5 を試料なしで確かめられるよう、#420 で、合成の形の組（`zumen_geom3d::cases`）に、T 字の継ぎ目・わずかにずれた面から 3 つの開口を順に引く形・辺で接する部品・面積のない面・辺を重ねて書いた多角形の面を足す。

### 3. `level`・`zone`・`classification`

- `level` は、階（`IfcBuildingStorey`）の名前と `Elevation` から作る。階は、中野区の 5 件を除く 16 件にあった。公開サンプルの階の名前は、`設計GL`・`1FL`〜`3FL`・`RFL`・`最高高さ`（林野庁）、`UG`・`EG`・`1. OG`（GNI の Revit 2025）、`Ground Floor`・`1. Story`（GNI の Archicad 28）、`Level 1`（BIM Whale）などで、床でない基準（`設計GL`・`最高高さ`）も階として書かれていた。国内の建築の書き出し 5 件（CADEWA Real2011・ARCHICAD 20 の 2 件・ArchiCAD 15・GLOOBE 2024）も、階の名前の書き方がそれぞれ違い、床でない基準を階として書くものがあった。種類（GL・FL など）を示すプロパティはなかった。種類は、階の名前をプロファイル（決定 0138）の語彙に当てて決め、当たらない名前の `level` は種類を持たない。名前から推定した種類は、出所に推定と残す。
- `level` の高さは、階の `Elevation`（建物の ±0 からの高さ）にする。高さの基準は、建物の `ElevationOfRefHeight`（±0 の標高。今の lift は `spatial` に写している）を書いていれば持ち、書いていなければ持たない。敷地の `RefElevation` は、今のとおり `spatial` に持ち、`level` を作らない。建物の `ElevationOfTerrain`（地盤の標高）は写さず、地盤の高さの `level`（GL）は、階として書かれたものを使う。21 件では、これらの標高は書いてあっても多くが 0 で、0 でないのは GLOOBE 2024 の建物（基準の高さと地盤）だけだった。
- `zone` は、`IfcZone`・`IfcSpatialZone` から作る（決定 0207）。21 件のどれにもなかったので、プロパティからは作らず、種類の語彙は IFC データ作成基準を待つ。
- `classification` は、`IfcRelAssociatesClassification` と `IfcClassificationReference` から作る（決定 0207）。体系は、Uniformat（Revit）・Archicad Classification（GNI）・Uniclass と OmniClass（Text2BIM）で、国内のソフトの書き出しには分類がなかった。日本の分類の扱いは、使う書き出しを得てから決める。

### 4. 建具の 2D の表現と、室の形

- 建具の 2D の表現（`Plan`・`FootPrint`・`Annotation` の文脈の形）は、21 件のどの建具にもなかった。開き方（`OperationType`・`PartitioningType`）は、BIM Whale（Revit 2021）・林野庁・GNI・Text2BIM の扉の型か扉に書かれていたが、国内のソフトの書き出し（CADEWA Real2011・ArchiCAD 15・GLOOBE 2024）では、ないか `NOTDEFINED` だった。決定 0207 のとおり、2D の表現があればそれを描き、なければ開き方とプロファイルの記号で描き、どちらもなければ形を切った線を描いて診断を返す。開き方を形から推定しない。
- 室（`IfcSpace`）の形は、書き出しで違った。Revit の試料（BIM Whale の Revit 2021 の 2 件、保存し直した林野庁の Revit 2024 日本語版と GNI の Revit 2024・2025）は内法で、床の輪郭の頂点の多くが、壁の芯（`Axis`）から壁の厚さの半分ほど離れていた。GLOOBE 2024 は壁芯で、316 頂点のうち 258 が、壁の芯から 1 mm 以内にあった。ほかの試料には、室がなかった。
- 室の形は、書かれたまま持ち、どちらかに直さない。面積を求めるとき（#424）は、選択肢 2 で、室の形が内法か壁芯かを判定する。ソフトの名前では決めない（選択肢 1）。室の形の書き方は、ソフトの設定と版で変わりうるうえ、Revit の 5 件のうち 3 件は保存し直したもので、名前と書き方の対応を試料で確かめられない。
  - 壁の中心線は、壁の `Axis` の表現から求め、なければ、決定 0207 のとおり、材料の層の厚さと向きから求める。どちらもなければ、判定せず診断を返す。
  - 室の形の頂点の多くが、中心線から 1 mm 以内なら壁芯、壁の厚さの半分ほど離れていれば内法とする。判定は推定なので、結果の出所に推定と残す。
  - 引数の測り方と違えば、決定 0207 のとおりに換算する。利用者が内法か壁芯かを引数で指定したときは、判定より指定を使う（選択肢 3 を上書きとして足す）。

### 5. 文字の符号化

- エスケープでない 0x80 以上のバイトは、21 件のどれにもなかった。決定 0207 のとおり、UTF-8 として読む。
- 国内の書き出しのうち 4 件（CADEWA Real2011・ARCHICAD 20 日本語版の 2 件・ArchiCAD 15 日本語版）は、Shift_JIS のバイト列を、`\X\hh`・`\S\c`・`\X2\00hh\X0\` のエスケープで書いていた。ISO 10303-21 のとおりに読むと、仮名と漢字が ISO 8859-1 の文字に化ける。新しい書き出し（GLOOBE 2024・Rebro2025・CADWe'll Linx V6・CADEWA Smart V5.0・FILDER CeeD）と、公開サンプルの 12 件は、`\X2\` で正しく書いていた。
- 選択肢 3 を選ぶ。手がかりは、`\X\hh` の hh が 0x80〜0x9F であること。ISO 8859-1 では印字しない制御文字で、Shift_JIS では 2 バイトの文字の 1 バイト目になる。手がかりのある文字列は、4 件で 1,530 あり、エスケープをバイトに戻すと、すべて Shift_JIS として読めた。手がかりのない 17 件には、そのような文字列が 1 つもなかった。FILDER CeeD は、ISO 8859-1 の 8 ビットの文字を、0xA0 以上の `\X\hh` で正しく書いていて、手がかりに当たらない。
- 手がかりのある文字列が 1 つでもあるファイルでは、8 ビットのエスケープ（`\X\hh`・`\S\c`、`\X2\` の 0xFF 以下の単位）をバイトに戻し、Shift_JIS（CP932）として読む。読めない文字列は、規格のとおりに読む。どちらも、Shift_JIS で読んだ件数を診断で返す。元のバイトは L1 に残るので、書き戻しは変わらない。
- ファイルごとに決め、文字列ごとには決めない。手がかりのある 4 件には、手がかりを持たない 8 ビットの文字列が 68 あり、半角の片仮名だけの文字列（ArchiCAD 15 で 2）と、CP932 の利用者定義の文字を含む文字列（CADEWA Real2011 で 66）だった。どれも Shift_JIS のつもりで書かれていて、文字列ごとに決めると、ISO 8859-1 の文字に化ける。
- 逆に、手がかりのあるファイルに ISO 8859-1 のつもりの 8 ビットの文字（「°」の `\X\B0` など）があると、CP932 の別の文字（半角の片仮名など）に読み替わる。4 件にはそのような文字列はなかったが、ありうる。公開サンプルにある 0xFF 以下の単位だけの `\X2\` の文字列（306）は、手がかりのないファイルにあるので、読み替わらない。
- 利用者が引数で文字コードを指定したときは、手がかりより指定を使う（選択肢 4 を上書きとして足す、#540）。選択肢 4 だけにしないのは、利用者がファイルの文字コードを知らないことが多く、既定のままでは古い国内の書き出しが化けるため。
- ソフトの名前では分けない。見出しの名前は書き方がまちまちで、同じソフトでも版で書き方が変わる（CADEWA Real2011 と CADEWA Smart V5.0）。

### 6. メモリ・時間・wasm のモジュールの大きさと、形の項目をいつ写すか

- 時間（ネイティブ）: 公開サンプル（4 MB 以下）は、読む・写すが 1 件で 0.2 秒以下だった。国内の書き出しは、36〜63 MB で読むのに 0.9〜1.4 秒・写すのに 1.2〜2.4 秒、245 MB で 5.2 秒・7.3 秒だった。wasm32 は、公開サンプルで、読む・写すがネイティブの約 1.8 倍、形の評価が約 1.4 倍だった。
- メモリ: 読み終えたときの最大は、ファイルの 9〜13 倍だった（245 MB で 2.9 GB）。建物モデルが足すのは 4〜31 % で、多くは L1 が占める。wasm32 では、最大がネイティブの約 0.7 倍になる（ポインターが 32 ビット）が、4 GB を超えられない。245 MB のファイルは約 2 GB になると推定し、端末によっては開けない。L1 のメモリは #541 で減らす。
- 形の項目は、要るときに、要素ごとに写す（選択肢 2）。36 MB 以上の国内の書き出し 5 件で、すべての要素の形を写すと、写す時間の 3〜6 割の時間と、40〜116 MB が足された。プロパティ・数量・IDS の照合は、形を使わない。
- 要素のハッシュは `representation` の成分を含む（決定 0156・0207）。要るときに写すと、lift の時点では形の項目がないので、ハッシュに入れる値を、形を写す前の L1 の値から作るかを #419 で決める。
- BIM の wasm のモジュール（決定 0253 の `bim`。今 brotli で 437,262 バイト）は、manifold-rust の分（brotli で約 244 KB、決定 0222）だけ大きくなり、約 1.6 倍になる。図面のモジュールには入れない。

### 7. `IfcGridPlacement`・`IfcLinearPlacement`

21 件のどれにもなく、配置はすべて `IfcLocalPlacement` だった（国内の 9 件は、`IfcLinearPlacement` のない IFC2X3）。今の、解かずに診断を返す扱いを保つ。試料を得たら、`IfcGridPlacement` は通り芯の交点と向き（`grid` の成分、#419）から、`IfcLinearPlacement` は線形（`IfcAlignment`）の側で解く。

## 影響

- #419 は、この一覧の形の項目を持てば始められる。形の項目は、要素ごとに要るときに写し、要素のハッシュとの関係を決める。`bench/kernels/src/ifc.rs` を元にするときは、形状表現をたどる再帰（写像・合成の曲線・ブーリアンの項）に上限を付け、`IfcTrimmedCurve` の媒介変数を平面角の単位で読む（試作は度と決めている）。
- #420 は、manifold-rust を workspace の依存に入れ、上の条件を満たす。試作の手当て（格子・T 字の継ぎ目・部品ごとの開口・面積のない面・エンジンの明示）は、`bench/kernels` のコードを元に書き直す（AGENTS.md）。
- #422 は、隠れ線の速さと失敗の割合を、試料で測る。
- lift で捨てた実体は #537（1 の 1・2）と #539（1 の 3〜7）、Shift_JIS をエスケープで書いた文字列は #540、L1 のメモリは #541 で進める（段階 2、#303）。#540 は、手がかりのあるファイルで ISO 8859-1 のつもりの文字が混ざる場合を、試験の中で作るファイルで確かめ、利用者が文字コードを指定する引数を足す。
- `level` の種類の語彙と、建具の記号は、作図の慣習のプロファイル（#423）に置く。室の形の判定は #424 に置く。
- 国内の書き出しを公開サンプルに足せたら（#513）、`samples` で測り直し、この決定の 3〜5 の数も数え直す。

## 参照

- [0007](0007-single-rust-implementation.md)・[0018](0018-geometry-layers-and-tolerances.md)・[0138](0138-profiles-as-versioned-data.md)・[0156](0156-drawing-stable-ids-and-preconditions.md)・[0171](0171-public-samples-in-git.md)・[0207](0207-building-model-components.md)・[0222](0222-geom3d-types-and-kernel-trait.md)・[0253](0253-wasm-civil-and-pointcloud-modules.md)
- [#5](https://github.com/zumen-jp/zumen/issues/5)・[#303](https://github.com/zumen-jp/zumen/issues/303)・[#412](https://github.com/zumen-jp/zumen/issues/412)・[#418](https://github.com/zumen-jp/zumen/issues/418)・[#419](https://github.com/zumen-jp/zumen/issues/419)・[#420](https://github.com/zumen-jp/zumen/issues/420)・[#422](https://github.com/zumen-jp/zumen/issues/422)・[#423](https://github.com/zumen-jp/zumen/issues/423)・[#424](https://github.com/zumen-jp/zumen/issues/424)・[#513](https://github.com/zumen-jp/zumen/issues/513)・[#537](https://github.com/zumen-jp/zumen/issues/537)・[#539](https://github.com/zumen-jp/zumen/issues/539)・[#540](https://github.com/zumen-jp/zumen/issues/540)・[#541](https://github.com/zumen-jp/zumen/issues/541)
- [bench/kernels/README.md](../../bench/kernels/README.md)・[docs/formats/ifc.md](../formats/ifc.md)
- ISO 10303-21（交換構造の文字列のエスケープ）、IFC の文書（`formats/ledger.toml` の `ifc_2x3_tc1`・`ifc_4_0_2_1`・`ifc_4_3_2`）
- IfcOpenShell 0.9.0.post1（試験の道具として、体積を照らすのに使った。依存にしない）
