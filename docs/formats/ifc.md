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
- 書き: `building_apply` でプロパティ・名前・説明を変えて書いた IFC4 と IFC2X3 を、毎晩の CI で IfcOpenShell 0.9.0.post1 で開く。変更が見え、スキーマと EXPRESS の規則の検査の誤りが増えないことを確かめる（`crates/zumen-ifc/tests/ifcopenshell/check.py`、#482）。BIM ソフトでは開いていない。BIM ソフトで開く確認は #526 で行う。[検証キット](../../xtask/verify/ifc/README.md)は公開 IFC4・IFC2X3 を CLI で編集し、期待値・対照の壁・元の形の行の保持を比較する。キットの生成と再読み込みは `synthetic` で、製品での `application` の確認には数えない。
- 判定・インスタンスのグラフ・壊れた所の扱いと、上に記した属性以外の値は、外部の実装と照らしていない（IfcOpenShell と照らしたのは、上の値だけ）。公開サンプル 12 件は、自前のテストでも L1 で読み、変えずに書くと元のバイト列に戻ることと、写さない値の診断の code・実体のクラス・件数を確かめる（`crates/zumen-ifc/tests/samples.rs`）。壊れた所の扱いと ifcZIP は、仕様とテストの中で作ったファイルだけで確かめた。
