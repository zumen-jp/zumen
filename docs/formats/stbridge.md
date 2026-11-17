# ST-Bridge

ST-Bridge（構造の設計データを交換する XML、`.stb`、台帳の `stbridge`）を読み、建物モデルへ移す規則を記録します。実装は `crates/zumen-stbridge`、建物モデルは `crates/zumen-building` です。仕様書と XML Schema は buildingSMART Japan の公開のページ（構造設計小委員会）にあり、リポジトリには含めません。仕様の文は写さず、ここに書くのは zumen の解釈です。権利の根拠は台帳にあります（#400）。

## API

- `open` は、根の要素が `ST_BRIDGE` の XML を、名前空間を問わず見分ける。判定は `bim` のモジュールで、CityGML・基盤地図情報の後、ifcZIP の前に試す。
- `building`・`building_elements`・`scene3d` で、IFC と同じ建物モデルとして返す。
- 書き戻さない。`building_apply` は `call.document.unsupported` で断る。

## 読む版

- 根の要素が `ST_BRIDGE` の XML を、名前空間を問わず判定する。
- `version` が 2.0 系（2.0.0〜2.0.2）と 2.1 系（2.1.0・2.1.1）を読む。1.x などほかの版は、`stbridge.version.unsupported` で知らせて読まない。2.1.1 の XML Schema は、`version` を `2.1.0` に固定している。
- 文字の符号化は、XML の宣言に従う（UTF-8 と Shift_JIS をサンプルで確かめる）。

## 空間の構造

| ST-Bridge | 建物モデル |
| --- | --- |
| `StbCommon` | `IfcProject`。`project_name` を文書の名前に、`app_name`・`app_version` を作ったアプリにする |
| `StbModel` | `IfcBuilding`。プロジェクトに集約する |
| `StbStory` | `IfcBuildingStorey`。`height` を階の高さ（mm）にし、建物に集約する |
| `StbAxes` の下の軸の組（`StbParallelAxes` など） | `IfcGrid`。組の原点と角度を配置にし、建物に含める |
| 軸 | `IfcGridAxis`。名前を軸の札にする |

単位は長さを mm、角度を度にする。

## 部材

| ST-Bridge | 種類 | 決まった型 | 配置を決める節点 |
| --- | --- | --- | --- |
| `StbColumn`・`StbPost` | `IfcColumn` | `COLUMN` | 下と上 |
| `StbGirder`・`StbBeam` | `IfcBeam` | `BEAM` | 始端と終端 |
| `StbBrace` | `IfcMember` | `BRACE` | 始端と終端（階は高いほう） |
| `StbSlab` | `IfcSlab` | | 節点の並び（階は最初の節点） |
| `StbWall` | `IfcWall` | | 節点の並び（階は最後の節点） |
| `StbFooting` | `IfcFooting` | `PAD_FOOTING` | 1 つの節点 |
| `StbStripFooting` | `IfcFooting` | `STRIP_FOOTING` | 始端と終端 |
| `StbPile` | `IfcPile` | | 1 つの節点 |
| `StbFoundationColumn` | `IfcColumn` | | 1 つの節点 |
| `StbParapet` | `IfcWall` | `PARAPET` | 始端と終端 |

- 元の要素の名前を、種類の `original` に残す。`name` を名前に、`id` を札にする。
- 要素の ID は `guid` から作る。22 文字の GlobalId はそのまま、UUID（32 桁の 16 進数か、ハイフンのある形）は GlobalId に直す。`guid` がなければ、XML の要素の番号で作る。
- 部材のすべての属性を、要素の名前のプロパティのセットに、文字列のまま持つ。`id_section` が指す断面があれば、断面の要素の属性を、断面の要素の名前のセットに足す。
- 部材は、節点の階に含める。節点の階は、階の `StbNodeIdList` で決め、なければ高さのいちばん近い階にする。階がなければ建物に含める。

### 配置

- 線の部材（柱・梁・ブレースなど）は、始めの節点を原点にし、材軸を局所の Z にする。材軸が鉛直なら局所の X を全体の X に、そうでなければ全体の Z と材軸の外積にする。`rotate` があれば、局所の Z のまわりに右手の向きで回す。
- 面の部材（スラブ・壁）は、1 つ目の節点を原点に、2 つ目の節点への向きを局所の X に、多角形の法線を局所の Z にする。
- 1 つの節点の部材（基礎・杭など）は、節点に平行移動し、`rotate` だけ Z のまわりに回す。
- 節点がない・重なるなどで決められなければ、配置を持たせず `stbridge.placement.unresolved` で知らせる。参照する節点・断面・部材がなければ、`stbridge.reference.missing` で知らせる。

### 開口

- 2.1 系は `StbOpenArrangement` の `kind_member`・`id_member` で、開口を壁かスラブに結ぶ。2.0 系は、壁・スラブの `StbOpenIdList` が `StbOpen` を指す。
- 開口は `IfcOpeningElement` にし、部材に穴をあける（`Voids`）。配置は部材の配置に対する位置（`position_X`・`position_Y`）と回転にする。

## 移さないもの

- 断面の形（押し出しの形）と配筋。建物モデルの形の項目の型が決まるまで持たない。断面の要素の属性はプロパティとして持ち、形・配筋を書く子の要素（`StbSecFigureBeam_RC`・`StbSecBarArrangementBeam_RC` など）は、要素の名前ごとに `stbridge.element.unsupported` で知らせる。
- 鋼材の規格の一覧（`StbSecSteel`）。断面の属性に残る規格の名前から、寸法は引かない。`stbridge.element.unsupported` で知らせる。
- 根と `StbModel` の下の、上の表にない要素（拡張の `StbExtensions` など）と、上の表にない部材。要素の名前ごとに `stbridge.element.unsupported` で知らせる。

## サンプル

- `crates/zumen-stbridge/tests/fixtures` のサンプルは、XML Schema に合わせて書いた合成のもので、実在の建物ではない。`frame-2.1.stb` は 2.1 の架構、`wall-2.0.stb` は Shift_JIS の 2.0 の壁と開口。
- `samples/stbridge` には、MIT で公開された実装の作者が自分のツールで作ったサンプルを置く（[公開サンプルの台帳](../../samples/ledger.toml)）。どれも実在の建物ではない。商用ソフトの出力は、権利が分からないので置かない。

| サンプル | 出どころ | 版 | 確かめること |
| --- | --- | --- | --- |
| `hoaryfox-sample-building.stb` | hrntsm/HoaryFox の `Samples` | 2.0.2 | 鉄骨の架構（柱 105・梁 192・ブレース 10）を移す。`StbCommon` に `app_version` がなく XML Schema に合わないが読む |
| `stb-diffchecker-mini210-a.stb`・`-b.stb` | NS-NS/STB-DiffChecker の `TestData` | 2.1.0 | 断面の子の要素を知らせて移す。2 つの違い（断面の寸法と節点の位置）は、建物モデルに残らない |

## まだしないこと

- 断面の形（押し出しの形）と配筋、鋼材の規格の寸法。
- 書き出しと、IFC への変換。
- 外部の実装（HoaryFox・STB-DiffChecker）が読んだ値との照合と、XML Schema でのサンプルの検査の CI。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `stbridge` | 読み | `synthetic` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-stbridge)にあります。

- 合成のサンプル 2 件と MIT の公開サンプル 3 件を読み、部材の数・種類・配置・開口・診断を試験で確かめています。
- 公開サンプルの値を、外部の実装が読んだ値とは照らしていません。照らすまで `synthetic` にとどめます。
