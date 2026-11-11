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

- **文字列。** 古い国内の書き出し（CADEWA Real2011・ARCHICAD 20 日本語版・ArchiCAD 15 日本語版）は、Shift_JIS のバイト列を `\X\hh`・`\S\c`・`\X2\00hh\X0\` で書く。ISO 10303-21 のとおりに読むと、ISO 8859-1 の文字に化ける。0x80〜0x9F の `\X\hh` は ISO 8859-1 では印字しない制御文字で、Shift_JIS では 2 バイトの文字の 1 バイト目になるので、ファイルが Shift_JIS で書かれた手がかりにする（#540）。手がかりのあるファイルにも、手がかりを持たない 8 ビットの文字列がある。半角の片仮名だけの文字列（ArchiCAD 15 日本語版で 2 件）と、CP932 の利用者定義の文字（1 バイト目が 0xF0〜0xF9）を含む文字列（CADEWA Real2011 で 66 件）です。新しい書き出しと公開サンプルは `\X2\` で書く。FILDER CeeD は、ISO 8859-1 の 8 ビットの文字を、0xA0 以上の `\X\hh` で書く。
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

## 共有する SI 単位の採用条件

建物モデルへの `Lift<Building>` と、既存の `curve()`・`extruded_area()` は同じ SI の読み取りを使います。文書の単位の割り当て、プロパティ・数量に明示した単位、一段の換算の基の SI を対象に、長さ以外も検査します。値は元の単位のまま保持し、単位換算は適用しません。`extrusion()` の長さ専用の採用前検査は引き続き別に行います。

- 単独は IfcSIUnit の 4 属性、複合は IfcNamedUnit の 2 属性と IfcSIUnit の 2 属性を読みます。複合の Part の順は問いません。重複・余分な Part・未知の属性配置は採用しません。派生する Dimensions は `*` を要求し、参照をたどって代用しません。
- Prefix は正規の 16 列挙値か省略の `$`、Name は正規の 30 列挙値を要求します。未知値・数値・異型・小文字の列挙値を、接頭辞なしや倍率 1 として扱いません。[ISO 10303-21 第 3 版 6.4.5・12.1.7 と Table 2](https://steptools.com/stds/step/IS_final_p21e3.html) の交換構造の列挙は大文字です。L0 の `Value::Enum` と既存の `enumeration()` は寛容な字句読みで元の綴りを保持します。この契約を変えず、SI の採用時にだけ正規の列挙と照合し、入力の綴りや係数を黙って正規化しません。
- Name から導かれる 7 次元と UnitType の次元を照合します。名前と種類の独自の一対一対応は課さず、RADIAN/STERADIAN、HERTZ/BECQUEREL、CANDELA/LUMEN、GRAY/SIEVERT、KELVIN/DEGREE_CELSIUS の同次元の組み合わせは元値を保持します。正規の `USERDEFINED` は次元照合の結果が UNKNOWN になります。FALSE の次元不整合とは区別し、配置・Name・Prefix が正規なら元値を保持します。名前や使用文脈が示す単位の種類まで検証したとは扱わず、既存の保持範囲を狭めません。
- IFC2X3 TC1 の電気容量は、公式の CorrectDimensions が質量指数 +1、DimensionsForSIUnit(FARAD) が -1 で、電流指数はともに +1 と記す不整合があります。IFC4 と IFC4.3 の両関数は（-2、-1、4、2、0、0、0）で一致します。2X3 でも FARAD の自然言語定義「electric capacitance」と整合する後者の次元で読みます。これは読み取りの判断であり、2X3 の形式的検証の実装ではありません。HENRY/INDUCTANCEUNIT は 3 版とも同じ次元です。

採用できない元の SI には `ifc.unit.invalid` と理由・元レコードのバイト範囲を返します。同じ SI を文書・複数の値・換算基から共有しても、1 回の取得内では一度だけ診断します。不正な SI の cache と成功した単位の cache を分け、成功 cache の再取得は従来どおり写す値を予算に数えます。プロパティ・数量の値は残し、単位だけを `None` にします。単位が解けない値の `ifc.unit.missing` も従来どおり返します。予算や中断で採用前に止まったことを、不正や欠落として診断しません。

正規の最大 2 Part・4 属性は固定個数の借用値の照合で、正しい既存入力の値・出所・予算使用量を保ちます。非正規の多 Part は走査前に Part 数を Work に数え、不正 SI の記録前は Work 1 と DecodedBytes 512 を確保します。追加検査の停止位置は参照する値でなく SI の元レコードです。文書の単位の検査で止まった場合は、要素の走査・確保へ進みません。

根拠は [2X3 NamedUnit.WR1](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcnamedunit.htm)・[CorrectDimensions](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifccorrectdimensions.htm)・[DimensionsForSIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcdimensionsforsiunit.htm)・[SIUnitName](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcsiunitname.htm)、[IFC4 CorrectDimensions](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcmeasureresource/lexical/ifccorrectdimensions.htm)・[DimensionsForSIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcmeasureresource/lexical/ifcdimensionsforsiunit.htm)、[IFC4.3 CorrectDimensions](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCorrectDimensions.htm)・[DimensionsForSIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcDimensionsForSIUnit.htm) です。Prefix と属性の定義は[断面の仕様参照](#断面を共通の経路で取得する)にあります。スキーマ・生成コード・第三者の実装ソースは同梱しません。

換算単位自身の Dimensions、ValueComponent の意味・種類との整合、複数段の換算、文書の単位集合の一意性、派生単位や文脈依存の単位全般は今回の検査に含めず、#419・#539 に残します。ID と Hash の規則、公開 API、Building 全面統合や形の評価は変えません。

### 共有 SI の確かめ方

`tests/units.rs` は合成 3 版で、全 SI 名・同次元の別名・全接頭辞と省略・単独/複合・換算基・成功/不正 cache・診断の元位置と件数・停止前の確保・中断を検査します。公開の `gni/model_9.ifc` を基に単位だけを自作の不正値へ変え、旧押し出しと曲線の取得でも幾何の定義を保って単位を拒否することを確認します。正しい旧入力は変更前後の値・出所・診断・全 Kind の予算と ID/Hash を比較します。

```powershell
$env:ZUMEN_IFC_UNITS_OUT = Join-Path (Get-Location) 'target/unit-comparison'
cargo test -p zumen-ifc --locked --test units
python -I crates/zumen-ifc/tests/ifcopenshell/units.py target/unit-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-units.mjs'
cargo test -p zumen-ifc --locked --test units --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-units.mjs --compare target/unit-comparison target/unit-comparison-wasm
```

WASI は JSON 4 件、正例の値・出所の記録、不正値・全 Kind の予算境界・中断の契約記録をネイティブとバイト単位で比較します。IfcOpenShell 0.9.0.post1 とは、合成 105 件と公開 2 件の元の単位名・種類・接頭辞を照合します。合成は `synthetic`、公開は `external` の範囲です。次元関数・単位換算・ファイル全体の適合性を外部照合したとは扱いません。既存の IFC fuzz が共有 Lifter と各 getter を呼び、毎晩の既存の読み取り専用 job でも export と元属性の照合を流します。

## 要素から形状表現をたどる

Rust の `IfcFile::product_shape(instance, cx)` は、建物の要素の L1 の番号から形状表現をたどり、`zumen_ifc::ProductShape` を返します。`building` の feature が必要です。`Building::lift` が知る要素・空間・通り芯の実体が対象で、型の要素（`IfcTypeProduct`）は対象外です。ifcZIP は `file.ifc().product_shape(...)` から呼べます。

既存の `instance()`・`Record::parts()` でも元の属性を読めますが、要素から共有する写像へ進むには参照の型と循環・予算の検査が必要です。この取得は L1 の Rust の経路として追加し、既存の `curve()` と組み合わせます。各言語への新しい op や Building の成分は追加しません。

```rust,ignore
use zumen_ifc::ShapeLinks;

let shape = file.product_shape(element_instance, cx)?;
for node in &shape.nodes {
    if matches!(node.links, ShapeLinks::Curve) {
        let curve = file.curve(node.source.id().unwrap(), cx)?;
        // curve は元の座標空間の定義。shape の配置や写像はまだ適用していない
    }
}
```

- `product`・`placement`・各 node の `source` は、同じ `IfcFile` の元の `Record` を借用します。元のバイト範囲・属性・STEP の参照を保ちます。`ShapeRef` はこのビュー内の `nodes` だけの参照です。STEP 番号とグラフの並びは source 内の参照であり、L2 の要素の ID や幾何の意味値には使いません。ビュー全体には意味値の等価性やハッシュを定義しません。
- `IfcProductDefinitionShape` の表現の列、`IfcShapeRepresentation` の文脈・識別子・種類・項目、`IfcMappedItem` の写像元と写像先、`IfcRepresentationMap` の原点と元の表現、`IfcGeometricSet`・`IfcGeometricCurveSet` の要素を保持します。元の列の並びを保ち、同じインスタンスの表現・写像・項目は一度だけ持ちます。
- 配置、表現の文脈、写像の原点と写像先はレコードへの参照です。元の軸の比・原点・一様または非一様な尺度・省略した属性もそのまま読めます。変換・正規化・単位換算は行いません。配置の親や補助レコードの子はこの取得で展開せず、数値や次元の妥当性、幾何の一致も検査しません。曲線の値と単位は `curve()` を呼んだときに取得します。
- 型を持つ曲線の項目は `ShapeLinks::Curve` と元のレコードを返し、曲線の値は読みません。ほかの形、点・面、立体・断面、トポロジやスタイルの表現は `Unsupported` と `ifc.shape.unsupported` を返し、元の値と参照を source に残します。スタイルやレイヤの逆参照はこのビューに集めません。
- 配置・形状表現・表現の識別子と種類が `$` なら、`None` と `ifc.shape.missing` を返します。必要な参照・種類・列・文字列の構造の誤りと循環は `ifc.shape.invalid` と `Error::Invalid` にします。`reason` は `product`・`reference`・`type`・`count`・`list`・`text`・`cycle` です。EXPRESS の全規則や SET の重複・内容の幾何的な制約を検査した結果ではありません。
- 探索と所有権は再帰しません。表現と共有写像をたどる深さを `depth`、参照・列・文字列の読み取りを `work`、項目・索引・参照・探索・文字列の確保前の予約量を `decoded-bytes` で数えます。予約量は 64 bit と wasm32 で共通です。循環・不正・予算・中断では部分結果を返さず、入った深さを戻します。

### 形状表現の確かめ方

`crates/zumen-ifc/tests/product_shapes.rs` の合成の 3 バージョンで、要素・共有写像・文脈・未適用の非一様な尺度と配置・曲線への受け渡しを確かめます。未知と省略の診断、参照と種類の誤り、循環・深さ・確保前の予算・中断・変異した入力も検査します。必要時の取得の前後で、既存の建物の ID と要素ハッシュは変わりません。

公開サンプル `gni/model_9.ifc` の柱の GlobalId `2q2CBrfQTCKPY0zxCdZygb`（`#131`）から、`#127` → `#126` → 写像 `#125` → 写像元 `#121` → 表現 `#119` → 押し出し `#114` まで、このビューでたどります。`#114` は未対応の葉として診断します。その先の断面 `#112` → 輪郭 `#111` は、既存の L1 の属性をたどり `curve()` へ渡す利用例です。立体・断面を型に写した結果や、写像を適用した輪郭の確認には数えません。

IfcOpenShell 0.9.0.post1 との照合は、合成の各版の 9 項目と補助レコード各 8 件、公開の 6 項目と補助レコード 16 件の参照・元の属性、および曲線各 1 件の点を対象にします。公開サンプルの表現と写像の照合が `external`、合成の 3 件は `synthetic` の範囲で外部の実装とも照合したものです。配置・写像の適用、単位・診断・予算、幾何の評価はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_SHAPES_OUT = Join-Path (Get-Location) 'target/shape-comparison'
cargo test -p zumen-ifc --locked --test product_shapes
python -I crates/zumen-ifc/tests/ifcopenshell/product_shapes.py target/shape-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-shapes.mjs'
cargo test -p zumen-ifc --locked --test product_shapes --target wasm32-wasip1 -- --test-threads 1
```

WASI の出力は `target/shape-comparison-wasm` に書きます。同じ 4 件の JSON で、参照と保持した属性、曲線の点、各 Kind の予算使用量と診断がネイティブと一致することを比べます。1 MiB のスタックで 3,000 項目を超える写像の read・clone・drop を行う検査だけはネイティブで流します。毎晩の CI でも上の手順で export して外部の実装と照らします。fuzz の `ifc` target は 16 要素までこの取得へ渡して共有の予算で探索します。

Building の representation・style・grid への統合、立体・断面の型、形の意味値を含む要素ハッシュは [#419](https://github.com/zumen-jp/zumen/issues/419) に残ります。

## 形の葉から押し出しと断面を取得する

`IfcFile::extruded_area(&ShapeNode, cx)` は、既存の `ProductShape` の葉から `IfcExtrudedAreaSolid`・`IfcArbitraryClosedProfileDef`・外輪の `CurveGraph` をまとめて取得します。L1 の引数の位置を利用側で読む必要はありません。同じ `IfcFile` の node を渡し、別のファイルや clone したファイルの node は `ifc.extrusion.invalid` にします。ifcZIP でも `file.ifc()` の同じ取得経路を使います。

```rust,ignore
let shape = file.product_shape(element_instance, cx)?;
for node in &shape.nodes {
    if file.kind(node.source).eq_ignore_ascii_case(b"IFCEXTRUDEDAREASOLID") {
        let area = file.extruded_area(node, cx)?;
        let profile = &area.definition.profile;
        let curve = area.boundary.item(profile.boundary);
        // 深さ・方向・配置・断面名は definition、外輪と単位は boundary、出所は source
    }
}
```

既存の `product_shape()` の結果と `ShapeLinks` は変えません。その L1 ビューの `Unsupported` と診断は、そのビューでは値を展開しないことを表します。同じ葉をこの操作で型に取得しても、先の診断を消しません。新しい境界 op、Building の成分、既存の ID・Hash v1・公開型の変更はありません。

- 値は `zumen_building::Extrusion`・`ClosedProfile`・`AxisPlacement3`、出所は `zumen_ifc::ExtrusionSource` に分けます。`ExtrudedArea` 全体には意味値の等価性やハッシュを定義しません。比較するときは定義・曲線の `kind`・単位を使い、元の Record・STEP 番号・CurveSource を含めません。`CurveRef` は同じ取得結果の `boundary` 内だけの参照です。
- 配置の原点・軸と参照方向の比、押し出す局所方向の比・深さ、断面名を元の値で保持します。軸や方向は正規化せず、要素の配置・写像・単位換算を適用しません。深さと立体の原点の単位も `boundary.length_unit` です。単位が解けない場合は `None` と `ifc.unit.missing` を返し、メートルなどを仮定しません。
- 立体の Position は IFC2X3 では必須、IFC4 以降で省略されていれば `None` にします。配置の位置は 3D の有限な数とし、軸と参照方向は同時指定か同時省略、指定時は非零・非平行であることを検査します。省略した軸や配置を既定の値で補いません。
- 断面は AREA の `IfcArbitraryClosedProfileDef` だけです。外輪の取得は既存 `curve()` に任せ、点・区間・共有・出所・単位を保持します。2D の Polyline は 4 点以上と始点・終点の一致、IndexedPolyCurve は添字の構造・接続と端点の座標の一致、完全な Circle は 2D の円の定義を検査します。点の追加や反転・近似は行いません。
- IndexedPolyCurve の区間がない場合は 4 点以上、区間がある場合は点の列が 3 点以上必要です。使わない点や重複した点も元の列のまま保持します。自己交差・面積・弧の非共線性などを評価して有効な面と認めた結果ではありません。自己交差の申告値も `CurveGraph` にそのまま残します。
- 深さは正の有限な数、押し出す方向は 3D の有限な比とし、局所 z 成分が 0 の方向を拒みます。元の配置の軸へ方向を変換してから検査することはしません。閉じていない輪、次元・参照・方向・配置・深さなどの不正は `ifc.extrusion.invalid` と `Error::Invalid` にします。`reason` は `source`・`type`・`reference`・`number`・`list`・`dimension`・`direction`・`placement`・`text`・`profile-type`・`count`・`closure`・`depth` です。曲線内部の不正は既存の曲線の診断も返します。
- ほかの立体・断面、CompositeCurve・TrimmedCurve など、この取得で閉鎖を検証しない境界は `ifc.extrusion.unsupported` と `Error::Unsupported` にします。`reason` は `solid`・`profile`・`boundary` です。未検証を閉じた面と仮定して値を返しません。元のレコードは L1 のビューから引けます。
- 立体・断面・曲線の深さ、参照・数・文字列・閉鎖の検査の手間、曲線・出所・文字列の確保前の予約量を共有の `Cx` で数えます。予約量はネイティブと wasm32 で共通です。中断・予算・不正・未対応では部分結果を返さず、入った深さを戻します。

### 押し出しの確かめ方

`crates/zumen-ifc/tests/extrusions.rs` は合成の IFC2X3・IFC4・IFC4X3 から既存の要素・共有写像の経路を使い、押し出し・閉じた断面・曲線を取得します。閉鎖・次元・方向・深さ・位置の不正、別のファイルの node、未知・循環・深さ・確保前の予算・中断と変異した入力を検査します。元の STEP 番号を変えたときの定義と出所の分離、取得前後の既存 Building の ID/hash も確認します。

公開の柱は `#131` → 表現/写像 → `#114` の葉を既存の `product_shape()` から引き、この取得で断面 `#112` と外輪 `#111` へ進みます。利用側で STEP の引数を読む箇所はありません。深さ 8.8・方向・未適用の配置・断面名・外輪の 5 点を型付きで持ちます。全立体・断面の対応、配置や写像を適用した形、メッシュや体積の検証には数えません。

IfcOpenShell 0.9.0.post1 との照合は、合成各版 2 件と公開 1 件の計 7 件です。押し出しの深さ・方向・未適用の配置、断面名と参照、外輪の定義、SI 単位の名前・接頭辞を比べます。数値は有限であることと等値を許容差なしで比較し、参照と列も厳密に比較します。公開の 1 件が `external`、合成 6 件は `synthetic` の範囲で外部の実装とも照合したものです。診断・予算、単位や配置・写像の適用、自己交差・面積・形の評価はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_EXTRUSIONS_OUT = Join-Path (Get-Location) 'target/extrusion-comparison'
cargo test -p zumen-ifc --locked --test extrusions
python -I crates/zumen-ifc/tests/ifcopenshell/extrusions.py target/extrusion-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-extrusions.mjs'
cargo test -p zumen-ifc --locked --test extrusions --target wasm32-wasip1 -- --test-threads 1
```

WASI の出力先は `target/extrusion-comparison-wasm` です。4 件の JSON で、定義と元の単位・出所・曲線・Kind 別の予算使用量がネイティブと一致します。1 MiB のスタックで 1,024 段の曲線の停止とエラーの破棄を検査するテストだけはネイティブで流します。fuzz の `ifc` target も、既存の形の葉から 16 件までこの取得へ渡します。毎晩の CI でも export と外部照合を流します。

## 断面を共通の経路で取得する

`IfcFile::extrusion(&ShapeNode, cx)` は既存の `product_shape()` の葉から、押し出しと `AreaProfileKind::ArbitraryClosed`・`Rectangle`・`Circle` の断面をまとめて取得します。種類ごとの独立した getter を増やさず、代表要素から同じ操作で値と出所へ進めます。

```rust,ignore
let shape = file.product_shape(element_instance, cx)?;
let result = file.extrusion(extruded_solid_node, cx)?;
match &result.definition.profile.kind {
    AreaProfileKind::Rectangle { x_dim, y_dim } => { /* 元の全幅・全高 */ }
    AreaProfileKind::Circle { radius } => { /* 元の半径 */ }
    AreaProfileKind::ArbitraryClosed { boundary } => {
        let curve = result.boundary.as_ref().and_then(|g| g.item(*boundary));
    }
}
```

既存の `extruded_area()`・`ExtrudedArea`・`Extrusion`・`ClosedProfile` を保つ追加を選びました。これらは v0.2.0 の公開タグには含まれず、その後の main へ追加された API ですが、現 main の利用側も書き換えずに使えるようにします。モデルを置き換える案は既存のフィールド・戻り値・予算の互換性を壊すため採用しません。旧 getter の値・診断・予算・JSON を変えず、新 getter の予算を別に確かめます。スキーマ・ABI・ID・Hash v1・公開版数は変えません。今後の公開版への追加として扱い、リリース準備の #799 は書き換えません。

- `ProfileExtrusion` は深さ・押し出す方向の比・3D の局所配置、`AreaProfile` は元の名前・2D の局所配置・断面の定義を持ちます。矩形は中心からの全幅 `XDim`・全高 `YDim`、円は中心からの半径です。元値は有限とし、寸法・半径・深さは正を要求します。頂点や近似輪郭は生成しません。
- `ArbitraryClosed` は既存の閉じた外輪の読み取りを共有し、`boundary` に `CurveGraph` を持ちます。矩形と円の `boundary` は `None` です。曲線の共有は各グラフ内に限り、複数の取得結果を横断して統合しません。
- `length_unit` は文書の元の長さの単位です。最初の IfcProject の単位の割り当てから、単独の SI 単位と一段の正の換算を既存の単位の読み取りで取得し、寸法・深さ・配置へ適用しません。SI 単位は派生する Dimensions の記号 `*`、正規の `LENGTHUNIT` と `METRE` の組み合わせ、`$` または正規の 16 種類の Prefix を採用前に検査します。換算の基の SI 単位も同じ条件です。換算単位自身の必須の Dimensions 参照を読み、7 個の整数の指数が長さの（1、0、0、0、0、0、0）であることを確認し、元のレコードも出所へ残します。これらの不正は `ifc.extrusion.invalid` と場所を返します。長さ以外の単位はこの操作の対象にせず、割り当てとそのレコードを出所に残します。長さの単位を扱えない場合は拒否し、割り当てがなければ単位なしと `ifc.unit.missing` を返します。
- `source` は立体・断面・配置・点・方向・読んだ単位の元のレコードを借用し、曲線内の参照は `CurveSource` が持ちます。STEP 番号・元バイト列を意味の値や要素ハッシュへ入れません。全体に `PartialEq`/`Hash` は定義せず、意味の比較には定義・曲線の値・単位を使います。
- 断面は `.AREA.` だけです。I 形・中空・穴付きなどを親型として読まず、種類と場所を `ifc.extrusion.unsupported` で返します。複合インスタンスや、対象の断面・配置・長さの単位に未解釈の追加属性がある場合も成功扱いにしません。元の L1 レコードは保持します。
- IFC2X3 では断面と立体の Position が必須、IFC4 以降では省略可能です。省略は `None` とし、既定の原点や軸を補いません。2D の Location・RefDirection は 2 成分を要求し、方向比を正規化しません。IFC2X3 の `IfcDirection` 自体と継承元に非ゼロ比の WHERE はなく、新しい断面の 2D 配置では `[0, 0]` も元比として保持します。IFC4 以降の `MagnitudeGreaterZero` では零比を拒否します。この版差で旧曲線・旧押し出しの判定は変更しません。派生する軸の `IfcBuild2Axes`/`IfcNormalise` は評価しません。
- 固定の参照経路と曲線の深さ、単位探索の全レコード・追加する次元の参照と 7 指数・属性の列・数・文字の Work、文字・出所・単位・結果の確保前の DecodedBytes を共有の `Cx` で数えます。走査の停止診断は走査中のレコードを指します。停止・不正・未対応では部分結果を返さず、呼び出し側の深さを戻します。

配置・共有写像・単位換算・輪郭や立体の評価、Building の全面統合と形の意味値を含む要素ハッシュは #419・#420 の残件です。

共有する既存の Lifter と旧 `extruded_area()` の SI の意味検査は[共有 SI の採用条件](#共有する-si-単位の採用条件)に従います。本取得の長さ専用の採用前検査と、共有経路の正しい旧入力の値・出所・予算の互換性の確認を分けて扱います。

公式仕様は [IFC2X3 ParameterizedProfileDef](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcprofileresource/lexical/ifcparameterizedprofiledef.htm)、[Rectangle](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcprofileresource/lexical/ifcrectangleprofiledef.htm)、[Circle](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcprofileresource/lexical/ifccircleprofiledef.htm)、[IFC4 ParameterizedProfileDef](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcprofileresource/lexical/ifcparameterizedprofiledef.htm)、[IFC4.3 ParameterizedProfileDef](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcParameterizedProfileDef.htm)です。方向の版差は [IFC2X3 Direction](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometryresource/lexical/ifcdirection.htm)・[Axis2Placement2D](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometryresource/lexical/ifcaxis2placement2d.htm)、[IFC4 Direction](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometryresource/lexical/ifcdirection.htm)、[IFC4.3 Direction](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcDirection.htm)です。単位は [IFC2X3 SIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcsiunit.htm)・[Prefix](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcsiprefix.htm)、[IFC4 SIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcmeasureresource/lexical/ifcsiunit.htm)・[Prefix](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcmeasureresource/lexical/ifcsiprefix.htm)、[IFC4.3 SIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSIUnit.htm)・[Prefix](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSIPrefix.htm) と [NamedUnit.WR1](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcNamedUnit.htm)・[CorrectDimensions](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCorrectDimensions.htm) に従います。仕様のスキーマ・生成コードは同梱しません。

### 断面の共通の取得の確かめ方

`tests/profiles.rs` は合成 3 版の要素・共有写像から各 3 断面を取得し、版差・元値・参照・未知/追加属性・不正・循環・確保前予算・中断・停止位置・呼び出し側の深さ・既存 ID/Hash を検査します。公開サンプルは `gni/model_9.ifc` の床の GlobalId `1soz90sYzCp8a2Do0djRBF`（`#1163` → `#1137` → 矩形 `#1134`）、`gni/model_18.ifc` の家具 `0pTNkIYt5EyfHBAi6yiASg`（`#21873` → `#21764` → 円 `#21761`）と既存の柱（`#131` → `#114` → 任意断面 `#112`）です。利用側の STEP 引数読みなしで元寸法・2D 配置・名前・単位と出所へ到達します。同梱の 12 サンプルには I 形・矩形中空・穴付き断面の実例を見つけていないため、今回は矩形と円を優先しました。

IfcOpenShell 0.9.0.post1 との属性比較は合成 9 件と公開 3 件です。要素の表現からの到達、断面の種類・名前・元寸法・元配置、押し出しの方向・深さ・元配置、外輪の定義、SI 単位と一段の換算の係数・接頭辞を許容差なしで比べます。公開 3 件は `external`、合成 9 件は `synthetic` の範囲です。予算・診断と出所の列、単位換算・配置や写像を適用した形はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_PROFILES_OUT = Join-Path (Get-Location) 'target/profile-comparison'
cargo test -p zumen-ifc --locked --test profiles
python -I crates/zumen-ifc/tests/ifcopenshell/profiles.py target/profile-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-profiles.mjs'
cargo test -p zumen-ifc --locked --test profiles --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-profiles.mjs --compare target/profile-comparison target/profile-comparison-wasm
```

WASI の出力先は `target/profile-comparison-wasm` です。6 件の JSON と、全 Kind の予算・中断・版差・走査の各停止位置の診断を記録する `contract.txt` をバイト単位で比べます。既存の fuzz の `ifc` target も形の葉からこの取得を呼び、既存の読み取り専用の毎晩の job で export と IfcOpenShell 比較を流します。

## 形の項目から面の表示を取得する

`IfcFile::surface_styles(&ShapeNode, cx)` は、既存の `ProductShape` の項目に直接結びつく `IfcStyledItem` から、面の色・透明度・反射の元の値を取得します。`building` の feature が必要です。ifcZIP では `file.ifc()` から同じ操作を呼びます。同じファイルの node だけを受け付け、別のファイルや clone したファイルの node は `ifc.style.invalid` にします。

```rust,ignore
let shape = file.product_shape(element_instance, cx)?;
for node in &shape.nodes {
    let appearance = file.surface_styles(node, cx)?;
    for binding in &appearance.styles {
        let shading = &binding.definition.shading;
        // RGB と透明度、任意の Rendering の指定を元の値で読む
        // 元の SurfaceStyle / Shading / Colour のレコードは binding.source
    }
}
```

公開サンプル `gni/model_9.ifc` の柱 `#131` から、既存の表現・写像の経路で得た葉 `#114` を渡すと、StyledItem `#118` → SurfaceStyle `#117` → Rendering `#116` → Colour `#115` を取得できます。スタイル名は `StB - Ortbeton`、Side は BOTH、RGB は各成分 0.968627450980392、透明度は明示した 0、鏡面反射の係数は 0.5、指数は 128、反射モデルは NOTDEFINED です。利用側で STEP の引数の位置を読む必要はありません。

- 意味の値は L1 に依存しない `zumen_building::SurfaceStyle`・`SurfaceShading`・`SurfaceRendering` に、借用する元の Record は `ItemStyleSource`・`SurfaceStyleSource` に分けます。StyledItem と SurfaceStyle の名前、面の側、色の名前も保持します。色の出所は属性を読んだ順で、重複も残します。比較は `definition` に対して行い、出所を含む全体には等価性・ハッシュを定義しません。
- Rendering の 9 属性をすべて保持します。SurfaceColour・Transparency に加え、DiffuseColour・TransmissionColour・DiffuseTransmissionColour・ReflectionColour・SpecularColour は色か係数、SpecularHighlight は指数か粗さ、ReflectanceMethod は列挙の元値を持ちます。省略は `None` のままとし、既定値・係数の掛け算・透明度から不透明度への変換・反射の式を適用しません。
- IFC2X3 の StyledItem は PresentationStyleAssignment を 1 件経由します。IFC4 ADD2 では直接指定と旧式の Assignment を受け付け、IFC4.3 ADD2 では削除された Assignment を拒みます。Shading は IFC2X3 では色だけ、IFC4 ADD2 以降では任意の透明度を持ちます。Rendering の引数の位置は 3 版とも共通です。PHYSICAL は IFC4.3 だけで受け付けます。
- RGB・透明度・色の係数・粗さは有限な 0〜1、指数は有限な実数として読みます。集合の空・重複、Shading と Rendering の併記、複数の直接の StyledItem、不正な参照・型・列挙・次元は `ifc.style.invalid` と `Error::Invalid` にします。診断に種類または理由と元の場所を持たせます。
- CurveStyle・FillAreaStyle・TextStyle、NullStyle、SurfaceStyleLighting・Textures・Refraction・ExternallyDefinedSurfaceStyle など、合法でも保持しない種類は `ifc.style.unsupported` と `Error::Unsupported` にします。ほかの値だけを返す部分成功にはしません。元のレコードは既存の L1 で参照できます。
- 直接の StyledItem がなければ空の成功です。写像元・材質・レイヤからの継承や優先順位は調べません。写像元の葉に付く指定は、その葉を別に渡して取得します。表示色・材質・面への適用・法線・形を評価した結果には数えません。
- ファイルの全レコードの走査、参照・数・文字列・集合の検査を共有の `Cx` の Work で数えます。確保前の DecodedBytes は、文字列を元の長さの 12 倍と 32、集合を 256 と項目ごと 64、結果の容量を項目ごと 1024 とし、ネイティブと wasm32 で共通にします。結果の容量には 2,097,151 項目の共通上限も置きます。入った深さは成功・不正・未対応・停止のすべてで戻します。

Building の既存の `Style` 成分への統合、形の意味値の Hash は未実装です。既存の Hash v1・ID・公開 0.2 の API・境界 op・スキーマは変えません。

根拠は buildingSMART の [StyledItem](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcStyledItem.htm)、[SurfaceStyle](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSurfaceStyle.htm)、[Shading](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSurfaceStyleShading.htm)、[Rendering](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSurfaceStyleRendering.htm)、[IFC4 の PresentationStyleAssignment](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2/HTML/schema/ifcpresentationappearanceresource/lexical/ifcpresentationstyleassignment.htm)、[IFC2X3 の StyledItem](https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/ifcpresentationappearanceresource/lexical/ifcstyleditem.htm) です。スキーマや第三者の実装は同梱しません。

### 面の表示の確かめ方

`crates/zumen-ifc/tests/styles.rs` は合成の IFC2X3・IFC4・IFC4X3 と公開の柱で、代表要素からの取得、全 Rendering 属性、出所・意味値の分離、取得前後の Building の ID・Hash v1 を確認します。文字列・符号付きの 0・省略・不正・未対応・共有・循環・別のファイルの node、確保前の予算、全 Kind の境界・中断・深さの復元と変異した入力も検査します。

IfcOpenShell 0.9.0.post1 との照合は、合成各版 2 件と公開 1 件の計 7 件です。保持する全属性と参照を、許容差なしの有限な数値の等値と列の厳密な一致で比較します。公開 1 件が `external`、合成 6 件は `synthetic` の範囲で外部の実装とも照合したものです。診断・予算・表示の評価・継承はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_STYLES_OUT = Join-Path (Get-Location) 'target/style-comparison'
cargo test -p zumen-ifc --locked --test styles
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/styles.py target/style-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-styles.mjs'
cargo test -p zumen-ifc --locked --test styles --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-styles.mjs --compare target/style-comparison target/style-comparison-wasm
```

同じテストを WASI でも流し、4 件の JSON と診断・全 Kind の予算使用量を記録する `contract.txt` を、ネイティブとバイト列で比較します。WASI の出力先は `target/style-comparison-wasm` です。fuzz の `ifc` target は既存の形の項目から 16 件までこの取得へ渡し、全レコードの走査を含めて予算を共有します。毎晩の CI でも export と外部照合を流します。

## 通り芯から軸と曲線を取得する

`IfcFile::grid_axes(grid_instance, cx)` は、通り芯の L1 番号から U/V/W の軸と既存の `CurveGraph` をまとめて取得します。`building` の feature が必要です。ifcZIP では `file.ifc().grid_axes(...)` を使います。Building の通り芯の要素が持つ `source().instance` からも呼べます。利用側で STEP の引数の位置を読む必要はありません。

```rust,ignore
let grid = file.grid_axes(grid_instance, cx)?;
for axis in &grid.axes {
    let designation = &axis.definition; // set・tag・same_sense
    let curve = axis.curve.item(axis.curve.root);
    // 曲線の元の値と単位を取得。配置と向きは未適用
}
```

公開の `gni/model_9.ifc` では、GlobalId `2vVYk8VWENmwp2otf5sJ1q` の Grid `#5535` から U 10 軸・V 10 軸・W 1 軸を取得できます。先頭の U 軸は `#5450`、タグは `E`、曲線は `#5449`、元の 2D の点の列は `#5448` です。既存の `product_shape()` の FootPrint の中にも同じ曲線への出所があり、この経路と一致することを検査します。

- 所属・タグ・SameSense は L1 に依存しない `GridAxisDefinition`、任意の PredefinedType は `GridLayout` に保持します。省略したタグと空のタグ、向きの真偽、レイアウトの指定と省略を区別します。U・V・W の順に、各 LIST の記録順を保ちます。
- 借用する元の Grid・軸・ObjectPlacement・Representation は `GridSource` と各軸の `source` に分けます。配置は参照先の種類を確認するだけで、属性や親の連鎖を展開せず保持します。配置・反転・既定値を適用せず、レイアウトに対する平行・直交・交点の適合も評価しません。
- 曲線の共有は各軸の `CurveGraph` の中です。同じ曲線を指す複数の軸は、同じ出所を持つ別のグラフとして取得します。グラフを横断する `CurveRef` や重複排除を仮定しません。意味を比べるときは `definition` と曲線の `kind`・単位を使い、CurveSource や元の番号を含めません。出所付きの全体には PartialEq/Hash を定義しません。
- U/V は必須で空でない unique LIST、W は任意で、指定する場合は空でない unique LIST です。同じ軸の重複・U/V/W 間の重複と、選んだ軸を別の Grid が参照する二重所属を拒みます。ファイル全体を走査して選んだ軸の所属を確認しますが、関係のない Grid 全体の妥当性を検査した結果ではありません。
- 別の Grid インスタンスの所属確認では、最初の Part だけに頼らず全 Part の IFCGRID を探します。継承した属性を含む 10/11 属性と、外部マッピングのローカルな 3/4 属性の配置から U/V/W を読みます。配置やリストの形を確認できない場合は `ifc.grid.invalid` の `owner-layout`・`owner-list` で拒み、所属なしと仮定しません。ヘッダーの同名の記録はインスタンスとして数えません。取得対象そのものの複合 Grid を型へ写す操作は、この経路には追加していません。
- AxisCurve は既存の曲線の取得を通し、全体が 2D であることを確認します。未知の曲線を含み次元を確認できない場合は、種類・出所を持つ `ifc.grid.unsupported` と `Error::Unsupported` にし、部分結果を返しません。元の曲線の未対応の診断も保持します。不正な参照・型・属性数・列・所属・次元・真偽は `ifc.grid.invalid` と `Error::Invalid` にします。

| 版 | Grid の属性数 | PredefinedType | ObjectPlacement の省略 |
| --- | ---: | --- | --- |
| IFC2X3 TC1 | 10 | 属性なし | 不可（WR41） |
| IFC4 ADD2 TC1 | 11 | 任意の 6 列挙 | 不可（HasPlacement） |
| IFC4.3 ADD2 | 11 | 同じ 6 列挙 | ShapeRepresentation を含まなければ可 |

列挙は RECTANGULAR・RADIAL・TRIANGULAR・IRREGULAR・USERDEFINED・NOTDEFINED です。IFC4.3 では Grid の HasPlacement が削除され、Product の PlacementForShapeRepresentation に従います。Representation が省略された場合と、参照先の Representations に ShapeRepresentation がない場合は、配置を `None` として保持します。配置の具体型は LocalPlacement・GridPlacement、4.3 では LinearPlacement も受け付けます。

全走査とほかの Grid のリスト、選んだ軸のリスト、各曲線と追加の次元検査、文字列を共有の `Cx` で数えます。1 レコードの Work に先頭 Part を含め、複合のインスタンスでは後続 Part の数も走査前に予約します。停止診断には処理中のレコードの範囲を残します。確保前の DecodedBytes は、選んだリストごとに 256 と項目ごと 64、結果の容量を軸ごと 1024、文字列を元の長さの 12 倍と 32、未対応の種類の文字列を 6 倍として予約します。結果の容量の共通上限は 2,097,151 軸です。曲線の予約量も同じ予算で数え、入った深さはすべての経路で戻します。

Building の Grid 成分の曲線、形の意味値の Hash、ほかの立体と断面は [#419](https://github.com/zumen-jp/zumen/issues/419) の残件です。公開タグ v0.2.0 と現在の main に追加された未公開 API を区別し、既存 API・ID・Hash v1・境界 op・版数は変えません。

根拠は buildingSMART の [IFC2X3 Grid](https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/ifcproductextension/lexical/ifcgrid.htm)、[IFC4 Grid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2/HTML/schema/ifcproductextension/lexical/ifcgrid.htm)、[IFC4.3 Grid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcGrid.htm)、[GridAxis](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcGridAxis.htm)、[Product の配置の条件](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcProduct.htm) です。公式ページが取得できない場合は buildingSMART の [公式 HTML スナップショット](https://github.com/buildingSMART/Standards-builds/tree/main/IFC/RELEASE) で版を確認できます。スキーマや第三者の実装は同梱しません。

### 通り芯の確かめ方

`crates/zumen-ifc/tests/grids.rs` は合成の 3 版と公開の通り芯から、値・出所・順序・共有の範囲、配置の版差、反転の未適用、取得前後の既存の ID/Hash v1 を検査します。集合・所属・次元・真偽・循環・未対応、長いリストの確保前の予算、全 Kind の境界・中断・深さの復元、複数の走査位置と変異した入力も確認します。曲線と通り芯の外部照合用の JSON の書き方は `tests/support/curve_json.rs` で共有します。

ほかの Grid が複合レコードである場合も、自作の 3 版で IFCGRID の先頭・後続、全属性・ローカル属性、二重所属・無関係の軸を確認します。未知の属性配置とリストの拒否、Part の走査前の予算と停止位置をネイティブと WASI の契約記録に含めます。この複合レコードの所属確認は `synthetic` の範囲です。

IfcOpenShell 0.9.0.post1 との照合は、合成 3 Grid の 20 軸・26 曲線と、公開 1 Grid の 21 軸・21 曲線です。Grid の配置・表現の参照とレイアウト、U/V/W の順序と軸の参照・タグ・向き、曲線の全保持属性を、有限な数値の等値を許容差なしで比較します。共有を含む曲線の探索も外部の実装側で別にたどります。公開の 1 Grid が `external`、合成の 3 Grid は `synthetic` です。単位・曲線の補助レコード・診断・予算、配置・反転・幾何の評価はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_GRIDS_OUT = Join-Path (Get-Location) 'target/grid-comparison'
cargo test -p zumen-ifc --locked --test grids
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/grids.py target/grid-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-grids.mjs'
cargo test -p zumen-ifc --locked --test grids --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-grids.mjs --compare target/grid-comparison target/grid-comparison-wasm
```

WASI の出力先は `target/grid-comparison-wasm` です。4 JSON と、診断・全 Kind の予算境界・中断・複数の走査位置を含む `contract.txt` のバイト列をネイティブと比較します。fuzz の `ifc` target は 16 Grid までこの経路へ渡し、形状表現の取得と予算を共有します。毎晩の既存 IfcOpenShell job でも export と外部照合を流します。

## 曲線を必要なときに取り出す

Rust の `IfcFile::curve(instance, cx)` は、曲線のインスタンスを指定し、`zumen_building::CurveGraph` を返します。`building` の feature（既定で有効）が必要です。`IfcZipFile` では `file.ifc().curve(instance, cx)` と呼びます。要素の番号や `IfcGridAxis` の番号ではなく、それらから参照する曲線の番号を渡します。

```rust,ignore
// 公開サンプル samples/ifc/gni/model_9.ifc の柱の断面の輪郭
let curve = file.curve(111, cx)?;
let item = curve.item(curve.root);
```

- `CurveKind` は `IfcPolyline`・`IfcLine`・`IfcCircle`・`IfcIndexedPolyCurve`（IFC4 以降）・`IfcCompositeCurve`・`IfcTrimmedCurve` を持ちます。円や弧を折れ線にせず、点や区間を逆に並べず、閉じる点も補いません。折れ線は最初と最後の点が同じなら閉じます。添字の曲線は、区間がなければ最初と最後の点、区間があれば最後と最初の添字で閉じるかを表します。合成の曲線は区間の順・`SameSense`・`Transition`、切った曲線は `SenseAgreement` と優先する表現を保持します。
- 座標は 2 次元と 3 次元を区別し、曲線が書かれた座標空間のまま持ちます。円は中心と軸の比、直線は基点・方向の比・ベクトルの大きさを持ち、軸を正規化しません。3 次元の円の軸と参照方向は、ともに指定するか、ともに省略する必要があり、平行・反平行な指定は診断します。要素の配置・写像・表現の文脈の変換は適用しません。座標・半径・大きさの単位は `length_unit`、円の媒介変数の単位は `angle_unit` に持ち、度かラジアンに換算しません。直線の媒介変数は方向の大きさに掛ける無次元の値です。解けない単位は `None` にし、円の平面角の単位がなければ `ifc.unit.missing` を返します。
- 切る位置に点と媒介変数の両方が書かれていれば、どちらも元の順で残します。点が基の曲線の上にあること、点と媒介変数の一致、区間の接線と曲率の連続、添字の弧の非共線性や自己交差は評価しません。`self_intersect` は元の申告値です。
- 共有する基の曲線は 1 度だけ写し、`CurveRef` で参照します。参照はこのグラフ内だけで有効で、L1 の番号・要素の ID・ハッシュではありません。`CurveSource.instance` は曲線の L1 の番号、`records` は直接読んだ点・配置・方向・区間などの L1 の番号を読んだ順で持ちます。合成の曲線の区間の出所は、意味の値から外し、`records` に `segments` と同じ順で持ちます。子の曲線の出所は参照先から引きます。出所を意味の値から分け、STEP の番号を付け替えても同じ定義を返します。
- 型を持たない種類は `Unsupported` に種類と出所を残し、`ifc.curve.unsupported` に種類ごとの件数を返します。既知の子曲線の次元を参照元へ伝え、合成の区間同士と、基の曲線と切る点の次元を照合します。必要な参照の欠落、型・次元・有限な数・添字・区間の接続・切る位置の構成の誤りと循環は、`ifc.curve.invalid` と `Error::Invalid` にし、グラフの一部を返しません。`reason` は `reference`・`type`・`dimension`・`number`・`direction`・`radius`・`count`・`list`・`enumeration`・`boolean`・`indices`・`connectivity`・`trim`・`placement`・`unit`・`cycle` です。
- 探索と所有権は平らな列にし、深い曲線も Rust の再帰を使いません。探索の入れ子は `depth`、参照・数・列の読み取りは `work`、曲線・点・区間・出所・探索の領域は確保前に `decoded-bytes` で数えます。予約量は 64 bit と wasm32 で共通です。中断・予算では `Error::Stopped` にし、入った深さを戻します。近似で点を作らないので `segments` は使いません。

この getter は、`Building::lift` のコンポーネントを変更しません。既存の `ShapeItem`、要素の ID と Hash v1、各言語への API も変えません。L1 の形状表現・共有写像から曲線へは、上の `product_shape()` を使えます。Building の形状表現・style・通り芯への統合、要素ハッシュへの意味の値の取り込み、立体と断面の型は [#419](https://github.com/zumen-jp/zumen/issues/419) に残ります。形を評価するカーネルへの変換は [#420](https://github.com/zumen-jp/zumen/issues/420) の対象です。

### 曲線の確かめ方

`crates/zumen-ifc/tests/curves.rs` の合成の 3 バージョンで、定義・単位・向き・共有・出所、循環・予算・中断、取得順と STEP の番号の付け替え、取得前後の既存の要素の ID とハッシュを確かめます。公開サンプル `gni/model_9.ifc` の柱の GlobalId `2q2CBrfQTCKPY0zxCdZygb`（`#131`）から、`#127`・`#126`・写像 `#125`・写像元 `#121`・表現 `#119`・押し出し `#114`・断面 `#112`・曲線 `#111`・点の列 `#110` をたどり、輪郭の 5 点を比較します。写像や立体を型に写した結果の確認ではありません。

外部の実装との照合では、合成の IFC2X3 の 9 曲線・IFC4 と IFC4X3 の各 10 曲線と、公開サンプルの `#111` を、IfcOpenShell 0.9.0.post1 の元の属性値と比べます。合成の数には共有する曲線の再取得を含み、29 件は `synthetic` の範囲で外部の実装とも照合したものです。公開サンプルの 1 件（IndexedPolyCurve）だけを、読みの `external` に数えます。方向の比・座標の次元・半径・大きさ・添字・区間の向き・切る位置の値と順に食い違いはありません。単位・診断・予算と、曲線の評価は、この外部の照合には含めません。毎晩の CI でも同じ手順を流します。

```powershell
$env:ZUMEN_IFC_CURVES_OUT = 'target/curve-comparison'
cargo test -p zumen-ifc --locked --test curves writes_curves_for_other_readers
python -I crates/zumen-ifc/tests/ifcopenshell/curves.py target/curve-comparison
```

同じ Rust のテストを `wasm32-wasip1` に作り、Node.js の WASI で流せます。1 MiB のスタックを指定するスレッドのテストだけはネイティブで流します。WASI の runner はリポジトリだけを開き、ホストの環境変数を渡しません。`target/curve-comparison-wasm` の JSON に、同じ曲線の定義と各予算の使用量を書きます。ネイティブの JSON とバイト列で比較できます。

```powershell
rustup target add wasm32-wasip1
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-curves.mjs'
cargo test -p zumen-ifc --locked --test curves --target wasm32-wasip1 -- --test-threads 1
```

`fuzz/fuzz_targets/ifc.rs` も、読んだ IFC の曲線を 16 件まで必要時の getter に渡し、建物・土木のモデルと予算を共有します。型を持つ曲線と切った入力は、Rust のテストキットでも、2 度の結果・診断・使用量を確かめます。

## 階・区域・分類の成分

次の値を建物モデルに写す。属性の位置は、台帳の IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の各実体の「属性」の節による。

- `level`: `IfcBuildingStorey` の `Name` と `Elevation` を、文書の長さの単位で持つ。所属の親をたどって建物が見つかり、その `ElevationOfRefHeight` があれば、`datum` にその建物の ID を持つ。基準の値と単位は、その建物の `spatial.elevation` から引ける。敷地と建物の標高から `level` は作らない。高さがないか有限の数でなければ、成分を作らず `ifc.attribute.invalid` を返す。
- 階の種類の語彙は #423 のプロファイルで足すため、今は `level.kind` を持たない。`1FL` のような名前も、そのまま持ち、コードで種類を推定しない。語彙から推定する処理と、その推定の出所は #423 に残る。
- `zone`: `IfcZone` と、IFC4 以降の `IfcSpatialZone` に付ける。`IfcZone` は `IfcRelAssignsToGroup`、`IfcSpatialZone` は `IfcRelReferencedInSpatialStructure` の、室と区域の直接の参照を、関係と並びの順で持つ。区域の中の区域を展開しない。物理の要素への空間の参照は、従来の `parent.referenced_in` に持つ。語彙がないので `zone.kind` は持たず、IFC の `PredefinedType` と `ObjectType` は `class` に残す。境界の形は #419 で扱う。
- `classification`: `IfcRelAssociatesClassification` の対象に、`IfcClassificationReference` のコード（IFC2X3 の `ItemReference`、IFC4 以降の `Identification`）と名前を付ける。`ReferencedSource` から `IfcClassification` の名前と版を引く。IFC4 以降は参照の階層をたどり、体系そのものの関連づけも持つ。体系のない軽い分類は、コードと名前だけを持つ。循環、欠けた参照、対応していない種類は、成分を付けず `ifc.attribute.invalid` を返す。
- 出所は、階と区域の元の要素のインスタンス、所属と分類の関係のインスタンス・GlobalId・並びの位置を持つ。区域の成分を付けた関係は区域の側にも残すので、L1 の値までたどれる。参照の連鎖と親の探索は深さと手間を数え、成分と出所を写す前に展開のバイトを数える。
- 区域と分類の構成員の集合は、参照でない値と欠けた参照を `ifc.attribute.invalid` で診断する。有効な参照は元の位置を保って写す。L2 の要素にできない対象の実体自体を落とす場合は `ifc.entity.dropped` だけで数える。プロパティ定義など別の成分に使った実体に関係だけを付けられない場合は、`ifc.relation-target.unsupported` に関係・対象の種類と元の集合内の位置（0 から）を返す。ほかの関係にも有効な構成員がない区域は、成分を付けない。型の分類を IDS で照合するときは、要素自身が持たない体系について、その型にあるすべての参照を引き継ぐ。

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

階・区域・分類の成分は、3 つの版の最小の合成 IFC で値・出所・診断・予算を確かめた `synthetic` の範囲です。公開サンプルの階と分類も読みますが、この 3 つの成分を外部の実装とは照らしていません。適合テストは、成分の応答と IDS の分類の照合をネイティブと wasm で比べます。

| 形式 | 向き | 段 |
| --- | --- | --- |
| `ifc` | 読み | `external` |
| `ifc` | 書き | `external` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-ifc)にあります。

- 読み: 公開サンプルの IFC 12 件（`samples/ifc`）を建物モデルに写し、毎晩の CI で、IfcOpenShell 0.9.0.post1 が同じファイルから読んだ値と照らす（`crates/zumen-ifc/tests/ifcopenshell/samples.py`、#412・#512）。照らすのは、実体のクラス・GlobalId・名前・所属の先と型・プロジェクトの空間への変換・開口・プロパティと数量の値。手元の照合では、12 件とも食い違いは 0。許諾の要る国内の BIM ソフトの書き出しは #513 で足す。
- 曲線の必要時の取得: [曲線の確かめ方](#曲線の確かめ方)のうち、公開サンプルの IndexedPolyCurve 1 件が `external` の範囲です。自作した 29 件は `synthetic` とし、IfcOpenShell との元の属性値の照合も行いました。ネイティブと wasm の JSON は、曲線の定義と予算の使用量まで一致しました。幾何の評価と BIM ソフトで開く確認は含みません。
- 要素から形状表現をたどる L1 の取得: [形状表現の確かめ方](#形状表現の確かめ方)に従い、公開の柱の表現・写像と元の配置の参照・属性は `external`、合成の 3 版は `synthetic` の範囲で IfcOpenShell とも照合しました。曲線の取得へ渡す経路を確認しています。配置や写像を適用した形、立体・断面の型への変換は含みません。
- 押し出しと閉じた外輪の断面: [押し出しの確かめ方](#押し出しの確かめ方)の公開 1 件が `external` です。合成の 3 版の 6 件は `synthetic` の範囲で IfcOpenShell とも照合しました。局所の値と SI 単位の保持の範囲で、配置・写像・単位の適用や形の評価は含みません。ほかの立体・断面、未検証の境界と Building への統合は残っています。
- 共通の断面の必要時の取得: [断面の共通の取得の確かめ方](#断面の共通の取得の確かめ方)の公開 3 要素が `external`、合成 3 版の 9 件が `synthetic` です。矩形・円・任意断面と押し出しの元の値と参照を比べ、配置・形の評価と Building/Hash への統合は含めません。
- 面の表示の必要時の取得: [面の表示の確かめ方](#面の表示の確かめ方)の公開 1 件が `external` です。合成の 3 版の 6 件は `synthetic` の範囲で IfcOpenShell とも照合しました。色・透明度・Rendering の元の値と参照だけを比較し、表示・材質・レイヤ・写像による継承と反射の式は評価しません。Building の Style 成分と形の意味値の Hash は未実装です。
- 通り芯の必要時の取得: [通り芯の確かめ方](#通り芯の確かめ方)の公開 1 Grid・21 軸・21 曲線が `external` です。合成 3 Grid・20 軸・26 曲線は `synthetic` の範囲で IfcOpenShell とも照合しました。元の属性と参照、順序・共有を比較し、配置・反転・幾何の評価と Building/Hash の統合は含みません。
- 書き: `building_apply` でプロパティ・名前・説明を変えて書いた IFC4 と IFC2X3 を、毎晩の CI で IfcOpenShell 0.9.0.post1 で開く。変更が見え、スキーマと EXPRESS の規則の検査の誤りが増えないことを確かめる（`crates/zumen-ifc/tests/ifcopenshell/check.py`、#482）。BIM ソフトでは開いていない。BIM ソフトで開く確認は #526 で行う。[検証キット](../../xtask/verify/ifc/README.md)は公開 IFC4・IFC2X3 を CLI で編集し、期待値・対照の壁・元の形の行の保持を比較する。キットの生成と再読み込みは `synthetic` で、製品での `application` の確認には数えない。
- 判定・インスタンスのグラフ・壊れた所の扱いと、上に記した属性以外の値は、外部の実装と照らしていない（IfcOpenShell と照らしたのは、上の値だけ）。公開サンプル 12 件は、自前のテストでも L1 で読み、変えずに書くと元のバイト列に戻ることと、写さない値の診断の code・実体のクラス・件数を確かめる（`crates/zumen-ifc/tests/samples.rs`）。壊れた所の扱いと ifcZIP は、仕様とテストの中で作ったファイルだけで確かめた。
