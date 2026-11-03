# 0260: 試料の IFC で測り、3D のカーネルを manifold-rust にして、建物モデルの残りの 7 項目を決める

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-11-03 |
| 状態 | 採用 |

## 背景

[決定 0207](0207-building-model-components.md) は、建物モデルの 7 項目を、日本の BIM ソフトが書き出した IFC を得てから決めるとした（「試料を得てから決めること」）。[決定 0222](0222-geom3d-types-and-kernel-trait.md) は、3D のカーネルの第一の候補を manifold-rust にし、試料での失敗の割合・時間・メモリ・環境による違いを測ってから選ぶとした。

#412 で、公開サンプルの IFC 12 件を `samples/ifc/` に入れた。Revit 2024 日本語版（林野庁）・Civil 3D 2024 日本語版（中野区）・Archicad 28（GNI）・Revit（BIM Whale）・Text2BIM の書き出しで、IFC2X3・IFC4・IFC4X3 を含む。国内のソフトの書き出し 9 件（#513 の B01〜B09）は、公開の許諾がないので Git に入れず、手元で測った件数だけを使う。CADEWA Real2011・Archicad 20 日本語版・ArchiCAD 15 日本語版・GLOOBE Architect 2024・Rebro2025・CADWe'll Linx V6・CADEWA Smart V5.0・FILDER CeeD の書き出しで、どれも IFC2X3、0.7〜245 MB である。

測り方と結果の数は、[bench/kernels/README.md](../../bench/kernels/README.md) の「公開サンプルの IFC で測る」にある。

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

形の項目をいつ写すかは、次から選ぶ。

1. lift で、すべての要素の形の項目を写す
2. 要るときに、要素ごとに写す

## 決定

### 1. 意味を手で書く実体と、形の項目の一覧

形の項目は、次を `zumen-building` に持つ（#419）。21 件の `Body` と開口の形に使われた項目で、試作（`bench/kernels/src/ifc.rs`）は、公開サンプルの 12 件の `Body` と開口の形をすべて写せた。

| 種類 | 項目 |
| --- | --- |
| 立体・面 | `IfcExtrudedAreaSolid`・`IfcFacetedBrep`・`IfcShellBasedSurfaceModel`・`IfcFaceBasedSurfaceModel`・`IfcPolygonalFaceSet`・`IfcTriangulatedFaceSet` |
| ブーリアン | `IfcBooleanResult`・`IfcBooleanClippingResult`・`IfcHalfSpaceSolid`・`IfcBoxedHalfSpace`・`IfcPolygonalBoundedHalfSpace` |
| 写像 | `IfcMappedItem`・`IfcCartesianTransformationOperator3D`・`IfcCartesianTransformationOperator3DnonUniform` |
| 断面 | `IfcRectangleProfileDef`・`IfcCircleProfileDef`・`IfcArbitraryClosedProfileDef`・`IfcArbitraryProfileDefWithVoids`・`IfcIShapeProfileDef`・`IfcRectangleHollowProfileDef` |
| 曲線 | `IfcPolyline`・`IfcIndexedPolyCurve`・`IfcCompositeCurve`・`IfcTrimmedCurve`・`IfcLine`・`IfcCircle` |
| 2D の集まり（`Axis`・`FootPrint` など） | `IfcGeometricCurveSet`・`IfcGeometricSet` |

- 国内の書き出しの形は、面の集合（`IfcShellBasedSurfaceModel` が 13.6 万、`IfcFacetedBrep` が 1.4 万）と、写像・押し出しが中心だった。`IfcIShapeProfileDef` と `IfcRectangleHollowProfileDef` は、GLOOBE 2024 の柱（59 要素）だけが使い、試作は写していない。
- 回転（`IfcRevolvedAreaSolid`）と円の断面の掃引（`IfcSweptDiskSolid`）は、21 件のどれにもなかったが、カーネルが評価する種類なので、型を持つ（試験は最小の IFC で作る）。B スプライン・`IfcAdvancedBrep` もなかった。一覧にない項目は、種類と出所だけを持ち、評価するときに診断を返す（#419）。

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

- 公開サンプルの 12 件で、開口のある 171 要素のすべてで、開口を引いた結果が閉じた形になった。パニックと失敗はなかった。IfcOpenShell 0.9 の形と体積を照らすと、体積のある 1,414 要素のうち 1,278 が相対の差 1e-6 以下で、1e-3 を超えるのは 3 要素だった。3 要素は GNI の床で、重なる部品の体積を、IfcOpenShell は和にしてから求め、試作は部品ごとに足す。
- 国内の 9 件でも、パニックと止まりはなかった。開口のある 227 要素のうち、形を写せた 168 要素の 165 で、開口を引いた結果が閉じた形になった。残る 3 要素は、開口の形が閉じていないので manifold-rust が断った。59 要素は、柱の断面（`IfcIShapeProfileDef`・`IfcRectangleHollowProfileDef`）を試作が写していない。
- ネイティブ（Windows x64）で 2 度流した結果と、wasm32（Node.js の WASI）の結果は、manifold-rust のすべての行（1,874 行）で、座標のビットまで同じだった。macOS（arm64・x86_64）は、合成の形で同じだった（決定 0222）。
- 公開サンプルで開口を引く時間は、1 件でネイティブ 0.24 秒以下、wasm32 で 0.33 秒以下だった。
- csgrs は、開口を引いた 171 要素のうち 4 要素しか閉じず、5 要素でパニックした。wasm ではパニックで試料の全体が止まる。依存の 1 つ（`core2`）は、crates.io で取り下げられている。
- 自前の 2D のブーリアンは、開口のある 171 要素のどれも評価できなかった。壁の開口は押し出しの向きが母体と違い、半空間での切断もある。manifold-rust で十分に速いので、速い道は足さない。

#420 で workspace に入れるとき、次を条件にする。どれも試料で要った手当てで、試作に入れて測った。

1. ブーリアンの前に、点を `Tolerance::length` の格子にそろえる。BIM Whale の TallBuilding の壁は、面が開口の面から 1.6e-13 mm ずれていて、3 つの開口を順に引くと、manifold-rust が 1 つの開口のうち 915 × 200 × 305 mm を引かなかった。そろえると、引く順と、先に開口の和を取るかに関わらず、正しい体積になる。csgrs のパニックも 104 から 5 に減った。
2. 断面と面を三角形にするとき、T 字の継ぎ目を残さない。耳の切り取り（earcut）は、同じ直線の上の点を落とし、穴の頂点を通る辺を作るので、逆向きの辺がない辺を、その上の点で分ける。面積のない面は、要素を断らずに飛ばし、件数を診断で返す。国内の書き出しの 214 要素（多くは設備の継手）にあった。
3. 開口は、母体の部品ごとに引く。層ごとの壁など、閉じた部品の組を 1 つのメッシュにして引くと、閉じた形にならない。開口に `Body` がなければ、`Reference` を使う（Archicad 28 の書き出しは、開口の形を `Reference` に書く）。
4. 閉じていない形（面の集合で書いた家具・設備・地形など）は、ブーリアンに渡さず、元の形と診断を返す（決定 0018）。
5. 閉じた部分どうしが辺で接する結果（点を 1 つにすると、4 枚の面が 1 本の辺を共有する）は、閉じた形として扱う。`Mesh::is_closed` は 2 様体だけを閉じたとするので、向きつきの辺と逆向きの辺の数が等しいかで確かめる。切り口が、接する辺でも輪になることを試験で確かめる。
6. ブーリアンの道筋の std の `powi`（1 か所）と、`HashMap`・`HashSet`（5 か所）は、`libm` と決まった順の集まりに直すか、反復の順に結果が依存しないことを確かめる（決定 0007）。直した所は記録する。Linux での一致は、#420 の試験を CI の 4 つの環境と wasm32 で流して確かめる。
7. パニックしうる所（約 650）は、入力を確かめてから渡すこと（有限・点の番号・閉じた 2 様体）で避ける。試料でパニックが出たら、上流に直しを出すか、直した写しを持つ。

3D の許容差の既定値は、弦の誤差を 1 mm、同じ点とみなす距離（格子）を 1e-9 m にし、ファイルの長さの単位に換算する（決定 0018・0222）。この値で、IfcOpenShell との体積の差は、上のとおり 9 割の要素で 1e-6 以下だった。細い管の断面が粗いなど、ほかの値が要るときは、利用者が許容差を渡す。

### 3. `level`・`zone`・`classification`

- `level` は、階（`IfcBuildingStorey`）の名前と `Elevation` から作る。階は、中野区の 5 件を除く 16 件にあった。床でない基準（`設計GL`・`GL`・`最高高さ`・`基礎`・`PIT`・`PHR`・`屋根`）も、階として書かれていた。建築の書き出しの階の名前は、`1FL`〜`5FL`・`RFL`・`設計GL`・`GL`・`最高高さ`・`基礎`（Revit 2024 日本語版・Archicad 20）、`1階`〜`3階`・`屋根`（CADEWA Real2011・Archicad 20）、`1F`〜`3F`・`RF`・`PHF`・`PHR`・`PIT`（GLOOBE 2024・ArchiCAD 15）だった。種類（GL・FL など）を示すプロパティはなかった。種類は、階の名前をプロファイル（決定 0138）の語彙に当てて決め、当たらない名前の `level` は種類を持たない。名前から推定した種類は、出所に推定と残す。
- `zone` は、`IfcZone`・`IfcSpatialZone` から作る（決定 0207）。21 件のどれにもなかったので、プロパティからは作らず、種類の語彙は IFC データ作成基準を待つ。
- `classification` は、`IfcRelAssociatesClassification` と `IfcClassificationReference` から作る（決定 0207）。体系は、Uniformat（Revit）・Archicad Classification（GNI）・Uniclass と OmniClass（Text2BIM）で、国内のソフトの書き出しには分類がなかった。日本の分類の扱いは、使う書き出しを得てから決める。

### 4. 建具の 2D の表現と、室の形

- 建具の 2D の表現（`Plan`・`FootPrint`・`Annotation` の文脈の形）は、21 件のどの建具にもなかった。開き方（`OperationType`）は、Revit の書き出しにはあったが、国内のソフトの書き出し（CADEWA Real2011・ArchiCAD 15・GLOOBE 2024）では、ないか `NOTDEFINED` だった。決定 0207 のとおり、2D の表現があればそれを描き、なければ開き方とプロファイルの記号で描き、どちらもなければ形を切った線を描いて診断を返す。開き方を形から推定しない。
- 室（`IfcSpace`）の形は、書き出しで違った。Revit の書き出し（林野庁・BIM Whale・GNI）は内法で、床の輪郭の頂点が、壁の芯（`Axis`）から壁の厚さの半分ほど離れていた。GLOOBE 2024 は壁芯で、316 頂点のうち 258 が、壁の芯から 1 mm 以内にあった。室の形は書かれたまま持ち、どちらかに直さない。面積を求めるとき（#424）は、室の形の頂点と壁の `Axis` の距離で、室の形が内法か壁芯かを判定し、引数の測り方と違えば、決定 0207 のとおりに換算する。判定できないとき（壁に `Axis` がない書き出し）は、推定せず診断を返す。

### 5. 文字の符号化

- エスケープでない 0x80 以上のバイトは、21 件のどれにもなかった。決定 0207 のとおり、UTF-8 として読む。
- 国内の書き出しのうち 4 件（CADEWA Real2011・Archicad 20 日本語版の 2 件・ArchiCAD 15 日本語版）は、Shift_JIS のバイト列を、`\X\hh`・`\S\c`・`\X2\00hh\X0\` のエスケープで書いていた。ISO 10303-21 のとおりに読むと、仮名と漢字が ISO 8859-1 の文字に化ける。新しい書き出し（GLOOBE 2024・Rebro2025・CADWe'll Linx V6・CADEWA Smart V5.0・FILDER CeeD）と、公開サンプルの 12 件は、`\X2\` で正しく書いていた。
- 選択肢 3 を選ぶ。手がかりは、`\X\hh` の hh が 0x80〜0x9F であること。ISO 8859-1 では印字しない制御文字で、Shift_JIS では 2 バイトの文字の 1 バイト目になる。手がかりのある文字列は、4 件で 1,530 あり、エスケープをバイトに戻すと、すべて Shift_JIS として読めた。手がかりのない 17 件には、そのような文字列が 1 つもなかった。FILDER CeeD は「°」「×」を `\X\B0`・`\X\D7` で正しく書いていて、手がかりに当たらない。
- 手がかりのある文字列が 1 つでもあるファイルでは、8 ビットのエスケープ（`\X\hh`・`\S\c`、`\X2\` の 0xFF 以下の単位）をバイトに戻し、Shift_JIS（CP932）として読む。読めない文字列は、規格のとおりに読む。どちらも、Shift_JIS で読んだ件数を診断で返す。元のバイトは L1 に残るので、書き戻しは変わらない。
- ソフトの名前では分けない。見出しの名前は書き方がまちまちで、同じソフトでも版で書き方が変わる（CADEWA Real2011 と CADEWA Smart V5.0）。

### 6. メモリ・時間・wasm のモジュールの大きさと、形の項目をいつ写すか

- 時間（ネイティブ）: 公開サンプル（4 MB 以下）は、読む・写すが 1 件で 0.2 秒以下だった。国内の書き出しは、36〜63 MB で読むのに 0.9〜1.5 秒・写すのに 1.3〜2.4 秒、245 MB で 5.6 秒・7.7 秒だった。wasm32 は、読む・写すがネイティブの約 2.2 倍、形の評価が約 1.5 倍だった。
- メモリ: 読み終えたときの頂点は、ファイルの 9〜13 倍だった（245 MB で 2.9 GB）。建物モデルが足すのは 4〜30 % で、多くは L1 が占める。wasm32 では、頂点がネイティブの約 0.7 倍になる（ポインターが 32 ビット）が、4 GB を超えられない。245 MB のファイルは約 2 GB になると推定し、端末によっては開けない。L1 のメモリは #541 で減らす。
- 形の項目は、要るときに、要素ごとに写す（選択肢 2）。36 MB 以上の国内の書き出し 5 件で、すべての要素の形を写すと、写す時間の 3〜6 割の時間と、40〜116 MB が足された。プロパティ・数量・IDS の照合は、形を使わない。
- BIM の wasm のモジュールは、manifold-rust の分だけ大きくなる（brotli で約 244 KB、決定 0222）。図面のモジュールには入れない。

### 7. `IfcGridPlacement`・`IfcLinearPlacement`

21 件のどれにもなく、配置はすべて `IfcLocalPlacement` だった（国内の 9 件は、`IfcLinearPlacement` のない IFC2X3）。今の、解かずに診断を返す扱いを保つ。試料を得たら、`IfcGridPlacement` は通り芯の交点と向き（`grid` の成分、#419）から、`IfcLinearPlacement` は線形（`IfcAlignment`）の側で解く。

## 影響

- #419 は、この一覧の形の項目を持てば始められる。形の項目は、要素ごとに要るときに写す。
- #420 は、manifold-rust を workspace の依存に入れ、上の条件を満たす。試作の手当て（格子・T 字の継ぎ目・部品ごとの開口・面積のない面）は、`bench/kernels` のコードを元に書き直す（AGENTS.md）。
- lift で捨てた実体は #537（1 の 1・2）と #539（1 の 3〜7）、Shift_JIS をエスケープで書いた文字列は #540、L1 のメモリは #541 で進める（段階 2、#303）。
- `level` の種類の語彙と、建具の記号は、作図の慣習のプロファイル（#423）に置く。面積の測り方の判定は #424 に置く。
- 国内の書き出しを公開サンプルに足せたら（#513）、同じ手順で測り直す。

## 参照

- [0007](0007-single-rust-implementation.md)・[0018](0018-geometry-layers-and-tolerances.md)・[0138](0138-profiles-as-versioned-data.md)・[0171](0171-public-samples-in-git.md)・[0207](0207-building-model-components.md)・[0222](0222-geom3d-types-and-kernel-trait.md)
- [#5](https://github.com/zumen-jp/zumen/issues/5)・[#303](https://github.com/zumen-jp/zumen/issues/303)・[#412](https://github.com/zumen-jp/zumen/issues/412)・[#418](https://github.com/zumen-jp/zumen/issues/418)・[#419](https://github.com/zumen-jp/zumen/issues/419)・[#420](https://github.com/zumen-jp/zumen/issues/420)・[#423](https://github.com/zumen-jp/zumen/issues/423)・[#424](https://github.com/zumen-jp/zumen/issues/424)・[#513](https://github.com/zumen-jp/zumen/issues/513)・[#537](https://github.com/zumen-jp/zumen/issues/537)・[#539](https://github.com/zumen-jp/zumen/issues/539)・[#540](https://github.com/zumen-jp/zumen/issues/540)・[#541](https://github.com/zumen-jp/zumen/issues/541)
- [bench/kernels/README.md](../../bench/kernels/README.md)
- ISO 10303-21（交換構造の文字列のエスケープ）、IFC 4.3.2 の文書（`formats/ledger.toml` の `ifc_4_3_2`）
- IfcOpenShell 0.9.0.post1（試験の道具として、体積を照らすのに使った。依存にしない）
