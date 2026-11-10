# IFC

IFC-SPF と ifcZIP を、スキーマを知らないインスタンスのグラフ（L1）として読み書きする規則を記録します。実装は `crates/zumen-ifc` です。仕様の本文とスキーマは写しません（`formats/ledger.toml`）。

## 対象

- IFC-SPF（ISO 10303-21 の交換構造）。バージョン（IFC2X3・IFC4・IFC4X3 など）は問わず、知らない実体も読む。交換構造の読み取りは、SXF と共有する crate `zumen-step` が行う。
- ifcZIP。ZIP のアーカイブ（`crates/zumen-zip`、[ZIP の読み取り](../../crates/zumen-zip/src/lib.rs)）の中の 1 つの IFC-SPF を読む。
- ifcXML と ifcJSON は、まだ読まない。建物モデルへの移し方とスキーマでの検証は、別の issue で決める。

## 判定

- 先頭の行が `ISO-10303-21;` で、ヘッダーの `FILE_SCHEMA` の最初のスキーマの名前が、大文字と小文字を区別せずに `IFC` で始まれば IFC とする（`Signature::StepSchema`）。ヘッダーの文字列とコメントの中の `FILE_SCHEMA` は読まない。
- 同じ始まりの行の SXF は `ASSOCIATIVE_DRAUGHTING` のスキーマで、`FILE_SCHEMA` のない古い SXF もあるので、IFC の判定を先に行う。
- 読むときも、スキーマの名前が `IFC` で始まらないか `FILE_SCHEMA` がなければ、`ifc.schema.unsupported` で読まない。
- ifcZIP は ZIP のアーカイブとして読み、名前が `.ifc` で終わる項目（大文字と小文字を区別しない）がちょうど 1 つあるものとする。なければ `ifc.entry.missing`、2 つ以上なら `ifc.entry.unsupported` で読まない。ほかの項目は読まない。

## インスタンスのグラフ

- レコードは `zumen-step` の文で、節の語・ヘッダーの文・インスタンス・複合のインスタンスを、書いた順に持つ。レコードの間の空白とコメント、読めなかった文は、そのまま持つ。
- インスタンスの番号（`#n`）から、レコードを引く表を作る。同じ番号のインスタンスが複数あれば、初めのものを引き、`ifc.duplicate.dropped` を報告する。後のものもレコードとして残し、書くときは元のまま書く。
- 値の中の参照は、並びと型を付けた値の中も含めて、書いた順に引く。ファイルにない番号への参照は、読むときに `ifc.reference.missing` で報告する。
- 逆向きの参照（あるインスタンスを参照するインスタンス）は、求められたときに作り、インスタンスと参照の数を `work` で数える。

## 書き出したソフトごとの違い

#5 で、公開サンプルの 12 件（`samples/ifc`）と、手元の国内のソフトの書き出し 9 件（#513 の B01〜B09。公開の許諾がないので Git に入れない）を読んで見つけた違いです。国内の 9 件については、件数と種類（実体・属性の有無・書き方・値が 0 か）と、#513 に書いた書き出したソフトの名前とバージョンだけを書きます。林野庁と GNI の公開サンプルは公開元が保存し直したもので、書き出したソフトがそのまま書いたファイルとしては扱いません（`samples/README.md`）。

- **文字列。** 古い国内の書き出し（CADEWA Real2011・ARCHICAD 20 日本語版・ArchiCAD 15 日本語版）は、Shift_JIS のバイト列を `\X\hh`・`\S\c`・`\X2\00hh\X0\` で書く。ISO 10303-21 のとおりに読むと、ISO 8859-1 の文字に化ける。0x80〜0x9F の `\X\hh` は ISO 8859-1 では印字しない制御文字で、Shift_JIS では 2 バイトの文字の 1 バイト目になるので、ファイルが Shift_JIS で書かれた手がかりにする（#540）。手がかりのあるファイルにも、手がかりを持たない 8 ビットの文字列がある。半角の片仮名だけの文字列（ArchiCAD 15 日本語版で 2 件）と、CP932 の利用者定義の文字（1 バイト目が 0xF0〜0xF9）を含む文字列（CADEWA Real2011 で 66 件）である。新しい書き出しと公開サンプルは `\X2\` で書く。FILDER CeeD は、ISO 8859-1 の 8 ビットの文字を、0xA0 以上の `\X\hh` で書く。
- **精度。** 表現の文脈の `Precision` は、Revit・Archicad 28 のサンプルが 1e-5 m（mm で 0.01 か、m で 1E-05）、Civil 3D 2024 日本語版が 2.54E-8 m、Vectorworks Architect 2024 が 1E-5 mm だった。国内の 9 件は、FILDER CeeD（`$`）を除いて精度を書く。
- **開口の形。** GNI の Archicad 28 のサンプルは、開口（`IfcOpeningElement`）の形を `Body` でなく `Reference` に書く。
- **面。** 国内の書き出しの 214 要素（多くは設備の継手）に、面積のない面（点が 1 直線に並ぶ面など）がある。GNI の Archicad 28 のサンプルには、幅 100 mm の切り込みを、辺を重ねて書いた多角形の面がある。
- **室の形。** 室（`IfcSpace`）の床の輪郭は、Revit のサンプル（BIM Whale の Revit 2021、林野庁の Revit 2024 日本語版、GNI の Revit 2024・2025）が内法、GLOOBE Architect 2024 が壁芯だった。ほかのサンプルには、室がなかった。
- **建具。** どのサンプルの建具にも、2D の表現（`Plan`・`FootPrint`・`Annotation` の文脈の形）はない。開き方（`OperationType`・`PartitioningType`）は、BIM Whale・林野庁・GNI・Text2BIM（Vectorworks Architect 2024）の扉の型か扉に書かれ、国内の書き出し（CADEWA Real2011・ArchiCAD 15 日本語版・GLOOBE Architect 2024）では、ないか `NOTDEFINED` だった。
- **階と標高。** 階（`IfcBuildingStorey`）は、中野区の 5 件を除く 16 件にある。床でない基準（林野庁の `設計GL`・`最高高さ` など）も、階として書く。種類を示すプロパティはない。敷地の `RefElevation` と建物の `ElevationOfRefHeight`・`ElevationOfTerrain` は、書いてあっても多くは 0 で、0 でないのは GLOOBE Architect 2024 の建物（基準の高さと地盤）だけだった。
- **配置。** 配置はすべて `IfcLocalPlacement` で、`IfcGridPlacement`・`IfcLinearPlacement` はない。

## 建物モデルへ移すとき

- L1 はスキーマを仮定せずに元の文と参照を保持する。L2 は要素に意味ごとのコンポーネントを結び、持つコンポーネントだけを種類の名前とともにハッシュに入れる。正しい GlobalId の初出ではその値を ID に使う。GlobalId がない場合は `i<インスタンス番号>` にし、不正な値では `building.global-id.invalid`、後出の重複値では `building.global-id.exists` を返して、どちらも同じインスタンス番号の規則で ID を作る。値と参照の具体例は [建物の適合テスト](../../bindings/conformance/cases/building.json) にある。
- プロパティや IDS の照合で不要な形状は、必要な要素について初めて取り出す。試作で写せる型は [写す形の項目](../../bench/kernels/README.md#写す形の項目) にある。領域モデルにはこれに加え、断面の `IfcIShapeProfileDef`・`IfcRectangleHollowProfileDef`、Axis・FootPrint の `IfcGeometricCurveSet`・`IfcGeometricSet`、回転の `IfcRevolvedAreaSolid`、円断面の掃引の `IfcSweptDiskSolid` を持つ。試作の表にも上記の追加対象にもない形は、種類と出所を残し、評価時に診断する。形状とスタイルの実装条件は [#419](https://github.com/zumen-jp/zumen/issues/419)、階・区画・分類は [#418](https://github.com/zumen-jp/zumen/issues/418) にある。開口を引くときの入力条件・許容差・カーネルの選択は [3D カーネルの検証](../../bench/kernels/README.md#サンプルで要った手当て) に従う。
- 建具は書かれた 2D の表現を優先し、次に開き方とプロファイルの記号、どちらもなければ切断線と診断を返す。室の輪郭は書かれたまま残す。面積を求めるときは利用者が指定した内法・壁芯を優先し、推定に使った基準を出所に残す。室の面積の作業は [#424](https://github.com/zumen-jp/zumen/issues/424) で追う。
- スキーマを同梱せず、構文の検査は利用者の [EXPRESS](express.md)、要件の照合は [IDS](ids.md) を受け取る。

## 書き出し

- 変えずに書くと、IFC-SPF は元のバイト列と一致する。ifcZIP は、元のアーカイブのバイト列をそのまま書く。
- `building_apply` の op は、変更を当てたインスタンスだけを `#n=名前(引数);` の形で書き直す。足すインスタンスには今の最大の番号の次から振り、データのセクションの終わり（`ENDSEC;`）の前に、ファイルの改行で 1 行ずつ書く。ほかのレコードとレコードの間は、元のバイト列のまま書く。ifcZIP で開いたものは、書き直した IFC-SPF を元の項目の名前で入れ直す。
- 書く文字列は、`'` と `\` を重ね、印字できる ASCII の外の文字を `\X2\…\X0\`（基本多言語面の外を含めば `\X4\…\X0\`）にする。
- 当てられるのは、`IfcPropertySingleValue` の値と、`IfcRoot` の Name・Description。ほかの種類のプロパティ（列挙・表など）と、GlobalId を持たない要素（材料・通り芯の軸）は、`change.edit.unsupported` で当てない。

## 壊れた所と上限

- 始まりの行・`DATA` の節・`END-ISO-10303-21;` のどれかがなければ、`ifc.file.invalid` で読まない。形の読めない文は次の `;` まで飛ばし、`ifc.record.invalid` にその範囲を付ける。
- 入力のバイト数、レコードの数、値の数と参照の数（`work`）、並びの入れ子の深さ（`depth`）を予算で数える。ifcZIP は、展開した大きさを `decoded-bytes` で、展開する前に数える。
- 建物モデルを作るとき、値を写す大きさを `decoded-bytes` で数える。1 つのセットや材料を多くの要素に結んだり、同じ項目を繰り返し参照したりすると、入力の大きさに比例しないメモリを使うためである（#733）。
  - 数える写し: プロパティと数量のセットと、材料を、覚えるときと、要素に写すとき。セットのプロパティと数量、層の集合の層を、足すとき。単位を、プロパティ・数量・長さの値に写すとき。関係の GlobalId を、要素の出所に写すとき。
  - どれも、1 つのレコードから作った値を、覚える前と足す前に数える。
  - 大きさは、文字のバイトの数に、項目ごとの 32 を足した目安にする。プロパティの多い大きな IFC では、既定の予算（`decoded-bytes` の 2 GiB。ifcZIP は展開した大きさと分け合う）に当たることがある。
  - 要素と、要素に結ぶ定義の参照の組ごとに、`work` を 1 数える。
  - 止まったら写さず、実体を読むループで止まったことを返す。中断の合図と診断の上限で止まったときも同じにする。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `ifc` | 読み | `external` |
| `ifc` | 書き | `external` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-ifc)にあります。

- 読み: 公開サンプルの IFC 12 件（`samples/ifc`）を建物モデルに写し、毎晩の CI で、IfcOpenShell 0.9.0.post1 が同じファイルから読んだ値と照らす（`crates/zumen-ifc/tests/ifcopenshell/samples.py`、#412・#512）。照らすのは、実体のクラス・GlobalId・名前・所属の先と型・プロジェクトの空間への変換・開口・プロパティと数量の値。手元の照合では、12 件とも食い違いは 0。許諾の要る国内の BIM ソフトの書き出しは #513 で足す。
- 書き: `building_apply` でプロパティ・名前・説明を変えて書いた IFC4 と IFC2X3 を、毎晩の CI で IfcOpenShell 0.9.0.post1 で開く。変更が見え、スキーマと EXPRESS の規則の検査の誤りが増えないことを確かめる（`crates/zumen-ifc/tests/ifcopenshell/check.py`、#482）。BIM ソフトでは開いていない。BIM ソフトで開く確認は #526 で行う。
- 判定・インスタンスのグラフ・壊れた所の扱いと、建物モデルに写さない値は、外部の実装と照らしていない（IfcOpenShell と照らしたのは、上の値だけ）。公開サンプル 12 件は、自前のテストでも L1 で読み、変えずに書くと元のバイト列に戻ることと、写さない値の診断の code・実体のクラス・件数を確かめる（`crates/zumen-ifc/tests/samples.rs`）。壊れた所の扱いと ifcZIP は、仕様とテストの中で作ったファイルだけで確かめた。
