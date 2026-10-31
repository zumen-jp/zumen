# 0207: 建物モデルは IFC を意味の基準にした成分のグラフにし、IFC の L1 はスキーマを知らないインスタンスのグラフにする

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- [0008](0008-model-layers-and-codecs.md) は、L2 の建物モデルの中身を [#5](https://github.com/zumen-jp/zumen/issues/5) で決めるとした。[#1](https://github.com/zumen-jp/zumen/issues/1) の「4. 中立モデルの中身」は、その方向を挙げている。IFC を意味の基準にし、成分の集まりで持ち、IFC の L1 をスキーマを知らない STEP のインスタンスのグラフにし、建物から 2D を切り出し、面積の測り方を引数にする。
- [旧版の決定 0028](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0028-ifc-in-rust.md) は、IFC を自前の Rust で読み、ISO 10303-21 の構文を SXF と共有し、スキーマとそこから生成したコードを同梱しないとした。読む版は IFC2X3 TC1・IFC4 ADD2 TC1・IFC4X3 ADD2 とした。
- 次のことは決まっている。
  - 形状表現は L2 では評価せずに持ち、3D のカーネルは日本の BIM ソフトの書き出しで測って選ぶ（[0018](0018-geometry-layers-and-tolerances.md)）。
  - プロジェクトの空間は文書の長さの単位で Z が上にし、地図への変換は `IfcMapConversion` から型で作る（[0019](0019-coordinate-systems-and-gsi-data.md)）。
  - 図面モデル（[0022](0022-drawing2d-semantics-and-format-mapping.md)）、変更の一覧（[0139](0139-drawing-apply.md)）、ID とハッシュと前提条件（[0156](0156-drawing-stable-ids-and-preconditions.md)）、SXF の交換構造の L1（[0120](0120-sxf-l1-exchange-structure.md)）。
- 日本の BIM ソフトが書き出した IFC（zumen-jp/zumen-legacy#128）は、まだ手元にない。書き出しごとの癖（使う実体と形状表現、GL・FL や防火区画の書き方、文字の符号化）と、カーネルの測定（zumen-jp/zumen-legacy#129）は、試料がないと決められない。成分の器・ID・L1・検証・切り出しの手順・面積の引数は、公開された仕様（IFC 4.3・ISO 10303-21・IDS 1.0）から決められる。
- IFC5 は、buildingSMART International の開発のリポジトリで、alpha（`ifcx_alpha`）の例が公開されている（2026-10-31 に確かめた）。要素を UUID のパスで指し、`children`・`inherits` と、名前空間つきの属性（`bsi::ifc::class`・`bsi::ifc::prop::Height`・`usd::xformop` など）を持つ。別のファイルを層として重ね、同じパスに属性を足せる。
- [0138](0138-profiles-as-versioned-data.md) は、IDS の取り込み方を建物モデル（#5）と一緒に決めるとした。

## 選択肢

### 建物モデルの持ち方

1. **IFC のクラスごとの型（`IfcWall` など）を Rust の構造体で持つ。** 型で意味を確かめられる。ただし、スキーマから作れば生成したコードの同梱になり、手で書けば数百の実体と 3 つの版を追うことになる。IFC5 はクラスを属性の 1 つとして持つので、移るときに作り直しになる。
2. **独自の BIM スキーマを作り、IFC をそこへ写す。** 形式から離れられる。ただし、写すたびに意味が落ち、IFC へ書き戻すときに戻らない。旧版の図面で起きたことと同じになる（0008 の背景）。
3. **IFC を意味の基準にし、要素に成分を付けたグラフで持つ。** 要素は ID だけを持ち、分類・配置・形状表現・プロパティなどを成分として付ける。IFC のクラスは、分類の成分の値にする。IFC5 のパスと名前空間つきの属性に近く、ほかの形式（ST-Bridge の断面など）も成分を足せば受けられる。

### IFC の L1

1. **スキーマを知るモデル（実体ごとの型）。** 建物モデルの 1 と同じく、生成したコードか、手で書く量が問題になる。
2. **スキーマを知らないインスタンスのグラフ。** 0120 の SXF と同じく、文を元のバイト範囲のレコードにし、インスタンスの番号で引けるようにする。未知の実体も保つので、その場で編集できる。意味は lift で、要る実体だけを手で書いて読む。

### スキーマでの検証

1. 検証しない。
2. EXPRESS のスキーマを同梱する。旧版の決定 0028 に反する。
3. 利用者が手元の EXPRESS を渡し、実行時に読んで検証する。

## 決定

建物モデルは 3、IFC の L1 は 2、スキーマでの検証は 3 を採る。crate・成分・feature の名前は実装の識別子の案であり、境界のスキーマを足す決定ではない。

### crate と依存

| crate | 層 | 中身 |
| --- | --- | --- |
| `zumen-step` | L0 | ISO 10303-21 の交換構造（始まり・見出し・データの節・終わり）、文と値を元のバイト範囲で持つレコード、文字列のエスケープの戻し方。スキーマを知らない |
| `zumen-ifc` | L1 | IFC の系統のモデル（インスタンスのグラフ）と、IFC-SPF・ifcZIP のコーデック。feature `building` に、建物モデルとの lift・lower・apply |
| `zumen-building` | L2 | 建物モデル、ID とハッシュ、関係の索引、面積と数量。feature `drawing` に、図面モデルへの切り出し |

- `zumen-sxf` が持つ交換構造の読み取り（0120）を `zumen-step` に移し、SXF と IFC で共有する（0008 の L0 の共有）。SFC のフィーチャのコメントや囲まない値など、SXF に固有の書き方を設定で渡すか `zumen-sxf` に残すかは、移す PR で決める。どちらでも、SXF のレコード・ID・往復は変えない。
- `zumen-building` は `zumen-ifc` を知らない。`zumen-ifc` は `zumen-sxf` に依存しない（0008）。
- IFC・建物モデル・3D のカーネルは、wasm の BIM のモジュールに入る（[0013](0013-boundary-in-four-forms.md)）。`zumen-step` は、SXF のために図面のモジュールにも入る。

### 要素と成分

- 建物モデルは、要素の一覧と、成分の種類ごとの列で持つ。要素は ID と出所だけを持ち、意味はすべて成分で持つ。
- 要素にするのは、`IfcObjectDefinition` を継ぐもの（プロジェクト・建物の要素・空間・型・グループ・通り芯など）と、ID で指す資源（材料・通り芯の軸）にする。
- 関係（`IfcRelationship` を継ぐもの）とプロパティのセット（`IfcPropertyDefinition` を継ぐもの）は、要素にしない。関係の側（子・開口・建具など）とセットを持つ要素に成分として付け、元の GlobalId と並びの位置を出所に残す。
- 要素は、ファイルに書いた順に並べ、番号で引く。成分は種類ごとに、要素の番号の順の列で持つ。HashMap の順に依存しない（[0007](0007-single-rust-implementation.md)）。
- 逆向き（親から子、壁から開口など）の索引は、問い合わせるときに作り、手間を予算で数える。
- 成分の種類は足すだけにし、名前と意味を変えない（[0015](0015-add-only-boundary-and-version-floors.md) と同じ考え方）。知らない種類の成分は、読み飛ばせるようにする。

最初の成分は次のとおりとする。

| 成分 | 持つもの | IFC での主な出所 |
| --- | --- | --- |
| `identity` | 名前・説明・タグ・長い名前 | `Name`・`Description`・`Tag`・`LongName` |
| `class` | IFC のクラス（IFC4.3 の名前）、定義された種類、利用者が付けた種類、元のスキーマでのクラスの名前 | 実体の名前、`PredefinedType`、`ObjectType`・`ElementType` |
| `classification` | 分類体系の名前と版、項目のコードと名前（日本の分類を含む） | `IfcRelAssociatesClassification`・`IfcClassificationReference` |
| `spatial` | 空間の種類（敷地・建物・階・室・空間の区域）と、階の高さ・敷地の標高など | `IfcSite`・`IfcBuilding`・`IfcBuildingStorey`・`IfcSpace`・`IfcSpatialZone` |
| `parent` | 所属の先と種類（集約・入れ子・空間への包含）と入れ子の順、複数の階にわたる要素の空間への参照 | `IfcRelAggregates`・`IfcRelNests`・`IfcRelContainedInSpatialStructure`・`IfcRelReferencedInSpatialStructure` |
| `placement` | 親の配置を持つ要素と、局所の変換 | `ObjectPlacement`・`IfcLocalPlacement` |
| `representation` | 形状表現（文脈・表現の種類・形の項目）。評価しない | `Representation`・`IfcShapeRepresentation`、型の `RepresentationMaps` |
| `style` | 面の色と透明度、線の色、表示のレイヤの名前 | `IfcStyledItem`・`IfcPresentationLayerAssignment` |
| `type` | 型の要素 | `IfcRelDefinesByType` |
| `properties` | セットの名前と、プロパティの名前・値の型・値・単位 | `IfcRelDefinesByProperties`・`IfcPropertySet`、型の `HasPropertySets` |
| `quantities` | セットの名前と測り方の名前、数量の名前・種類（長さ・面積・体積・個数・重さ・時間）・値・単位・式 | `IfcElementQuantity` |
| `material` | 材料の構成（単一・層・断面・構成要素・並び）、層の厚さと向き、材料の要素 | `IfcRelAssociatesMaterial` と、`IfcMaterial` などの材料の定義 |
| `opening` | 開口の側では穴をあける要素、建具の側では埋める開口 | `IfcRelVoidsElement`・`IfcRelFillsElement` |
| `group` | 所属するグループ・系統・区域 | `IfcRelAssignsToGroup`（`IfcSystem`・`IfcZone` など） |
| `connection` | 接続する要素・ポートと、空間の境界 | `IfcRelConnectsPathElements`・`IfcRelConnectsPorts` など、`IfcRelSpaceBoundary` |
| `grid` | 通り芯の軸の名前（`X1` など）・曲線・組（U・V・W）・向き | `IfcGrid`・`IfcGridAxis` |
| `level` | レベルの名前・種類（GL・FL など）・高さと、高さの基準 | 階の `Elevation`、敷地と建物の標高、プロパティ（試料で決める） |
| `zone` | 区画の種類（防火区画など）と、構成する室か境界の形 | `IfcZone`・`IfcSpatialZone`、プロパティ（試料で決める） |
| `extension` | 形式に固有の、名前空間つきの型付きの値 | 0022 の拡張と同じ規則。プラグインの成分もここに置く |

- `class` は、IFC4.3 のクラスの名前で持つ。IFC2X3・IFC4 の実体は IFC4.3 の意味に寄せ（`IfcWallStandardCase` は `IfcWall` にする、など）、寄せられないものは元の名前のまま持つ。どちらでも、元のクラスの名前を残す。
- 型から継ぐ値（定義された種類・プロパティ・材料・形状表現の写像）は、型の要素に置いたまま、引くときに解く。同じ名前のセットの同じ名前のプロパティは、その要素自身の値を優先する。引いた値には、型から来たか、要素自身の値かを付ける。
- `level` と `zone` は、IFC に決まった書き方がない。どの実体とプロパティから作るかは、試料で書き出しごとに決める。種類の語彙（GL・FL、防火区画など）は、プロファイル（0138）に置く。

### 値・単位・文字

- 長さ・面積などの値は、文書の単位のまま、単位とともに持つ。単位を黙って換算しない。
- プロパティの値は、IFC の値の型の名前（`IfcLengthMeasure`・`IfcLabel` など）と値を持つ。単位は、プロパティに書いた単位を使い、なければ文書の単位の割り当て（`IfcUnitAssignment`）から値の型に合うものを使う。解けなければ単位なしにし、診断を返す。
- 文字列は、lift で ISO 10303-21 のエスケープを戻す。エスケープでない 0x80 以上のバイトは UTF-8 として読み、読めない所は置き換えの文字にして診断を返す。元のバイトは L1 に残る。Shift_JIS の生のバイトを書く書き出しがあるか、そのとき文字コードを推定するかは、試料で決める。

### 配置

- 配置は、親の配置と局所の変換（f64）の連鎖で持つ。親は、その配置を持つ要素で指す。どの要素も持たない中間の配置は、局所の変換に掛けて畳み、出所に残す。
- プロジェクトの空間への変換は、連鎖をたどって求める。たどる深さを予算で数え、循環は配置なしにして診断する。
- `IfcAxis2Placement3D` の軸は、IFC の規則で正規直交にする。軸を作れない配置（長さが 0 の向きなど）は、配置なしにして診断する。
- `IfcLocalPlacement` 以外の配置（`IfcGridPlacement`・`IfcLinearPlacement`）は、種類と出所だけを持ち、解けない配置として診断する。解き方は、試料で使われ方を見てから足す。

### 形状表現

- 0018 のとおり、L2 では評価しない。形状表現は、文脈（`Body`・`Axis`・`FootPrint` など）、表現の種類（`SweptSolid`・`Tessellation`・`MappedRepresentation` など）、形の項目の木で持つ。
- 形の項目は、L1 の型ではなく、建物モデルの型で持つ。曲線は種類のまま持ち、折れ線にしない（0018）。
- 最初に持つ項目は、カーネルの候補が評価する種類（押し出し・回転・円の断面の掃引・面の集合・三角形の集合・ブーリアン・半空間・写像）と、2D の曲線と断面の形にする。ほかの項目は、種類と出所だけを持ち、評価するときに診断を返す。項目を足すかは、試料で書き出しが使う種類を数えて決める。
- 写像（`IfcMappedItem`）は、写像の元と変換で持ち、同じ形を使い回す。L3 では、同じ形のインスタンスにつなぐ。
- 形の項目を lift で写すと、L1 と合わせてメモリを使う。試料で測り、大きければ、要るときに要素ごとに写す。

### 文書

- 文書は、プロジェクト（名前・段階）、単位の割り当て、表現の文脈と精度（0018 の文書の許容差）、真北、地図への変換と座標系の名前（0019）、書き出したアプリケーションの名前と版、見出しの値（`FILE_DESCRIPTION` のビューの定義・`FILE_NAME`・`FILE_SCHEMA`）を持つ。
- 書き出しごとに扱いを変えるときは、書き出したアプリケーションの名前と版で分ける。分けるかは、試料で決める。

### ID と変更

- 要素の ID は、GlobalId（22 文字）をそのまま使う。GlobalId を持たない要素（材料・通り芯の軸）は、`i` と 10 進のインスタンスの番号で作る（0156 の SXF と同じ書き方）。
- 同じ GlobalId が 2 回以上あれば、ファイルで最初の要素に GlobalId を使い、後の要素は `i` と番号で作って診断を返す。22 文字でないか、使えない文字を含む GlobalId も、`i` と番号で作って診断を返す。
- GlobalId は 128 ビットの値なので、IFC5 のパスの UUID と相互に直せる。
- 要素のハッシュは、0156 と同じ規則で作る。成分を決まった並びのバイト列にし、BLAKE3 の先頭の 128 ビットを `v1:` と 32 桁の 16 進で書く。出所の位置は入れない。文書のハッシュも 0156 と同じにする。
- 変更は、0139 と同じく、要素の ID と読んだときのハッシュを前提条件に持つ一覧にし、apply で IFC の L1 の触れたインスタンスにだけ書き戻す。共有されたプロパティのセット・材料・形状表現を 1 つの要素について変えるときは、写しを作って付け替え、ほかの要素の値を変えない。

### IFC5 への備え

- 要素を ID で、意味を名前の付いた成分で持つので、IFC5 のパスと属性に対応づけられる。`class` は `bsi::ifc::class`、`properties` は `bsi::ifc::prop::…`、`style` は `bsi::ifc::presentation::…`、`material` は `bsi::ifc::material`、`placement` は `usd::xformop` に近い。`parent` と `type` は、`children` と `inherits` に近い。
- 層を重ねることは、変更の一覧を順に当てることで表す。L2 に層の仕組みは持たない。
- IFC5 は alpha なので、読み書きはまだ作らない。仕様が固まったら、記録を書いて系統を足す。

### IFC の L1

- **レコード。** 0120 と同じく、文を元のバイト範囲のレコードにし、値は解釈せずに範囲で持つ。インスタンスの番号から位置を引く表を、番号の順に持つ。
- **何も失わない。** 知らない実体も保ち、変えずに書くと元のバイト列と一致する（0008）。
- **参照。** 値の中の参照（`#n`）は、番号で引ける。逆向きの参照は要るときに作り、手間を予算で数える。ない番号への参照は診断する。
- **変更。** トランザクションで、インスタンスを変える・足す・除く。足すインスタンスには、今の最大の番号の次から振る。ほかから参照されるインスタンスは除かずに失敗する。触れたインスタンスだけを書き直す（0008）。
- **判定。** 始まりの行 `ISO-10303-21;` と、見出しの `FILE_SCHEMA` の最初の名前で判定し、名前が `IFC` で始まるものを IFC にする。スキーマの名前の規則も、判定の規則のデータに足す（0008）。SXF は `ASSOCIATIVE_DRAUGHTING` 以外を読まない（0120）。
- **版。** L1 は版を問わずに読む。lift は IFC2X3・IFC4・IFC4X3 を読み、ほかの版は診断して建物モデルを作らない。
- **ifcZIP。** ZIP（L0。展開量に上限を付ける）から 1 つの IFC-SPF を取り出して読み、書くときは同じ形で包む。

### 意味づけは手で書く

- lift は、意味を持つ実体ごとに、版ごとの属性の位置と型を手で書いて読む。コメントに、仕様の版と節を書く。
- IFC の文書の文・図・表と、EXPRESS・XSD のスキーマ、そこから生成したコードは入れない（旧版の決定 0028、AGENTS.md）。
- 意味を持たない実体は L1 に残る。L2 には、種類ごとの件数を診断で返す（0022）。

### スキーマでの検証

- 利用者は、手元の EXPRESS のファイル（buildingSMART International が公開するもの）を、ほかの入力と同じく渡す（[0012](0012-boundary-functions-and-messages.md) の `zm_push`）。0019 の国土地理院のデータと同じく、zumen は同梱しない。
- 渡されたスキーマの名前とファイルのハッシュを、結果に残す。`FILE_SCHEMA` と名前が違えば、診断を返す。
- 確かめるのは、実体の名前、属性の数（継いだ属性と、導く属性の `*` を含む）、値の種類（参照の先が宣言の型かその下位の型か、列挙の値、選択の型、並びの大きさ、省略できない属性の `$`）にする。`WHERE` の規則・大域の `RULE`・関数は評価せず、評価しなかったことを結果に書く。
- 結果は指摘として返し、言い方は [docs/style.md](../style.md) の「検査の結果の言い方」に従う。

### 建物から 2D を切り出す

- **作るもの。** 階の平断面図・断面図・立面図を、図面モデル（0022）に切り出す。切り出した図面は、ほかの図面と同じく、シーン・照合・別の形式への変換に使える。
- **手順。**
  1. 切る面と見る向きを決める。平断面は、階の高さ（FL）からの切る高さと、見下ろす深さを引数で受ける。断面と立面は、通り芯か 2 点で決めた鉛直の面と、見る向きを受ける。既定値はプロファイルが持つ。
  2. 対象の要素の形を geom3d のカーネル（0018）で評価し、開口をブーリアンであける。失敗した要素は失敗する前の形を使い、診断を返す（0018）。
  3. 面で切った断面（切断）、切る面の向こうに見える稜線（見え掛かり）、隠れる稜線（隠れ線）に分けて、2D の形にする。
  4. 図面モデルの実寸の空間に置き、用紙とビュー（縮尺）を作る。プロジェクトの空間から実寸の空間への変換は、0019 の型で持つ。
- **見え掛かりと隠れ線。** 面を見る向きに投影し、手前の面で隠れる所を geom2d のブーリアン（0018 の整数の格子）で除く方法を、第一の候補にする。速さと失敗の割合は、カーネルを選ぶときに試料で一緒に測る。
- **線の役割。** 2D の要素は、元の要素の ID と、線の役割（切断・見え掛かり・隠れ・通り芯・注記）を持つ。線の太さ・線種・色・レイヤは、役割と元の要素の分類から、プロファイルで決める。
- **日本の作図の慣習。** 切断線の太さ、見え掛かりと隠れ線の表し方、材料ごとの断面の模様、建具の表し方（開き戸の円弧・引き戸など）、通り芯の記号は、作図の慣習のプロファイルに置く。出典（建築製図通則 JIS A 0150 など）は、プロファイルを作るときに決める。BIM 図面審査の入出力基準も、同じ形で足す。
- **建具。** 書き出しが平面の文脈（`Plan`）の 2D の表現を持てば、それを描く。なければ、建具の開き方（`OperationType` など）とプロファイルの記号で描く。どちらもなければ切断の形を描き、診断を返す。日本の書き出しが 2D の表現を書くかは、試料で確かめる。
- **ID と出所。** 切り出した要素の ID は、元の要素の ID・線の役割・順の番号で作り、同じ入力と同じ引数なら同じ ID にする。出所は、元の要素と形の項目までたどれるようにする。

### 面積と数量

- 面積・長さ・体積を求める op は、測り方を引数で受け、既定値を置かない。プロファイルから測り方を取るときは、プロファイルの識別を結果に残す。
  - 面積を測る線: 壁芯（壁の中心線）か、内法（壁の内側の面）か
  - 丸め: 切り捨て・切り上げ・四捨五入と、桁（例: 0.01 m²）と、丸める段（室ごと・階ごと・合計）
  - 単位
- 丸めは 10 進で行い、丸める前の値と、使った許容差（0018）も返す。
- 壁芯の面積は、室の形を壁の中心線まで広げて求める。中心線は、壁の `Axis` の表現か、材料の層の厚さと向きから求める。書き出しが室の形を内法と壁芯のどちらで書くかと、使える表現は、試料で確かめる。
- ファイルに書かれた数量（`IfcElementQuantity`）は、書かれたとおりに返し、求めた値と混ぜない。比べるのは別の照合にする。
- 整形（桁の区切り・単位の書き方）は、表示の層で行う。

### IDS

- IDS は、プロファイルに写さず、照合の入力としてそのまま読む。0138 が残した取り込み方を、これで決める。IDS の facet は、次の成分で確かめる。

  | facet | 確かめる成分 |
  | --- | --- |
  | entity | `class` |
  | attribute | 手で書いた属性（`identity`・`class` など）。ほかの属性は、EXPRESS が渡されたときに、実体の名前から位置を引いて確かめる |
  | classification | `classification` |
  | property | `properties`・`quantities`（型から継いだ値を含む） |
  | material | `material` |
  | partOf | `parent`・`group`・`opening` |

- 結果は、プロファイルの照合と同じ指摘の形（[0179](0179-profile-check-details.md)）で返し、確かめられなかった項目はそのことを返す。
- IDS の仕様の入手先と権利は、取り込むときに台帳（`formats/ledger.toml`）に記録する。

### 旧版の issue との分担

- **3D のシーン（zumen-jp/zumen-legacy#126）。** L3 にし、建物モデルからカーネルで作る。シーンの要素の ID は建物モデルの ID にし、2D の図面と 3D の選択を ID で連動させる。区画と op は、2D のシーン（[0137](0137-columnar-scene-transfer.md)・[0172](0172-scene-sections-and-op.md)）にならい、zumen-jp/zumen-legacy#126 を引き継ぐ issue で決める。
- **作成用の建物モデル（zumen-jp/zumen-legacy#139）。** zumen の建物モデルは、読む・チェックする・その場で編集する・切り出すためのものにする。壁に沿った開口の追従・拘束・形の作り直しなどの作成の機能は、この記録の範囲の外にし、作るときに zumen-jp/zumen-legacy#139 を引き継いで決める。そのときも、作った結果は建物モデルの成分と変更の一覧で表し、IFC へは lower で書く。
- **ST-Bridge（zumen-jp/zumen-legacy#140）と CEDXM・BE-Bridge・Stem（zumen-jp/zumen-legacy#141）。** 許諾が得られたら、プラグイン（[0017](0017-format-rights-ledger-and-plugin-sdk.md)）が、既存の成分と、名前空間を付けた `extension` の成分を足して受ける。

### 試料を得てから決めること

日本の BIM ソフトが書き出した IFC（zumen-jp/zumen-legacy#128）で、次を決める。

1. 意味を手で書く実体と、形の項目の一覧
2. カーネル（zumen-jp/zumen-legacy#129）と、開口のブーリアン・切断・隠れ線の速さと失敗の割合
3. `level`・`zone`・`classification` を、どの実体とプロパティから作るか
4. 建具の 2D の表現と、室の形が内法か壁芯か
5. 文字の符号化（Shift_JIS の生のバイトがあるか）と、書き出しごとに扱いを分けるか
6. メモリ・時間・BIM の wasm のモジュールの大きさと、形の項目を要るときに写すか
7. `IfcGridPlacement`・`IfcLinearPlacement` の解き方

## 影響

- 試料がなくても作れるのは、`zumen-step` の切り出し、IFC の L1 とコーデック、建物モデルの器と ID・ハッシュ、形を除く成分の lift、EXPRESS での検証、IDS の読み取り。どれも、試験の中で作る最小の IFC で確かめる（AGENTS.md）。
- #5 の完了の条件のうち、書き出した IFC を成分に読み、階の平面図を切り出す試作は、試料とカーネルを得るまで残る。
- 作図の慣習のプロファイルは、切り出しを作るときに、出典を決めて `profiles/` に足す。
- `level`・`zone` の種類の語彙は、IFC データ作成基準（2027 年度ごろに公表の予定）が出たら、それに合わせて直す。

## 参照

- [0007](0007-single-rust-implementation.md)・[0008](0008-model-layers-and-codecs.md)・[0012](0012-boundary-functions-and-messages.md)・[0013](0013-boundary-in-four-forms.md)・[0015](0015-add-only-boundary-and-version-floors.md)・[0017](0017-format-rights-ledger-and-plugin-sdk.md)・[0018](0018-geometry-layers-and-tolerances.md)・[0019](0019-coordinate-systems-and-gsi-data.md)・[0022](0022-drawing2d-semantics-and-format-mapping.md)・[0120](0120-sxf-l1-exchange-structure.md)・[0137](0137-columnar-scene-transfer.md)・[0138](0138-profiles-as-versioned-data.md)・[0139](0139-drawing-apply.md)・[0156](0156-drawing-stable-ids-and-preconditions.md)・[0172](0172-scene-sections-and-op.md)・[0179](0179-profile-check-details.md)
- [旧版の決定 0028](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0028-ifc-in-rust.md)
- [#1](https://github.com/zumen-jp/zumen/issues/1)・[#5](https://github.com/zumen-jp/zumen/issues/5)・[#303](https://github.com/zumen-jp/zumen/issues/303)、zumen-jp/zumen-legacy#126・zumen-jp/zumen-legacy#128・zumen-jp/zumen-legacy#129・zumen-jp/zumen-legacy#139・zumen-jp/zumen-legacy#140・zumen-jp/zumen-legacy#141
- IFC 4.3.2 の文書（`formats/ledger.toml` の `ifc_4_3_2`）、ISO 10303-21（交換構造）、ISO 10303-11（EXPRESS）
- buildingSMART International の [IFC5-development](https://github.com/buildingSMART/IFC5-development)（Hello Wall の例。2026-10-31 に読んだ）
- buildingSMART International の [IDS](https://www.buildingsmart.org/standards/bsi-standards/information-delivery-specification-ids/)（1.0。2024-06 に最終の標準として承認。2026-10-31 に読んだ）
