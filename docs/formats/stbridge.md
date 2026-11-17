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
- 部材のすべての属性を、要素の名前のプロパティのセットに、文字列のまま持つ。`id_section` が指す断面があれば、断面の要素の属性を、断面の要素の名前のセットに足す。断面の形が鋼材の規格を引くなら、規格の要素の属性も、規格の要素の名前（`StbSecRoll-H` など）のセットに足す。
- 部材は、節点の階に含める。節点の階は、階の `StbNodeIdList` で決め、なければ高さのいちばん近い階にする。階がなければ建物に含める。

### 配置

- 線の部材（柱・梁・ブレースなど）は、始めの節点を原点にし、材軸を局所の Z にする。材軸が鉛直なら局所の X を全体の X に、そうでなければ全体の Z と材軸の外積にする。`rotate` があれば、局所の Z のまわりに右手の向きで回す。
- 面の部材（スラブ・壁）は、1 つ目の節点を原点に、2 つ目の節点への向きを局所の X に、多角形の法線を局所の Z にする。
- 1 つの節点の部材（基礎・杭など）は、節点に平行移動し、`rotate` だけ Z のまわりに回す。
- 節点がない・重なるなどで決められなければ、配置を持たせず `stbridge.placement.dropped` で知らせる。参照する節点・断面・部材がなければ、`stbridge.reference.missing` で知らせる。

### 開口

- 2.1 系は `StbOpenArrangement` の `kind_member`・`id_member` で、開口を壁かスラブに結ぶ。2.0 系は、壁・スラブの `StbOpenIdList` が `StbOpen` を指す。
- 開口は `IfcOpeningElement` にし、部材に穴をあける（`Voids`）。配置は部材の配置に対する位置（`position_X`・`position_Y`）と回転にする。

## 断面の形

柱・梁・ブレース・スラブ・壁は、断面の形を押し出した立体（表現は `Body`・`SweptSolid`、単位は mm）を、部材の配置の局所の座標で持つ。断面の要素の、形を書く子（鉄骨と CFT は `StbSecSteelFigure` で始まる子、ほかは `StbSecFigure` で始まる子と 2.1 の `StbSecSlab_RC_Conventional`）から、1 つだけの葉の要素を探す。`StbSecSteelBeam_S_Shape` はラッパーとして中に入る。子が複数ある（ハンチ・テーパー・端と中央で違う断面など）か、葉が下の表にないときは形にしない。

| 葉の要素 | 断面 | 寸法 |
| --- | --- | --- |
| `StbSecColumn_RC_Rect`・`_SRC_Rect`、2.0 の `StbSecColumnRect` | 矩形 | 局所の X が `width_X`、Y が `width_Y` |
| `StbSecColumn_RC_Circle`・`_SRC_Circle`、2.0 の `StbSecColumnCircle` | 円 | 直径 `D` |
| `StbSecBeam_RC_Straight`・`_SRC_Straight`、2.0 の `StbSecBeamStraight` | 矩形 | 局所の X が `width`、Y が `depth` |
| `StbSecSteelColumn_S_Same`・`_CFT_Same`、`StbSecSteelBeam_S_Straight`、2.0 の `StbSecSteelBeamStraight`、`StbSecSteelBrace_S_Same` | `shape` が名前で引く鋼材の規格 | 下の表 |
| `StbSecWall_RC_Straight` | 板 | 厚さ `t` |
| `StbSecSlab_RC_Straight`・`StbSecSlab_RC_ConventionalStraight` | 板 | 厚さ `depth` |

| 鋼材の規格 | 断面 | 寸法 |
| --- | --- | --- |
| `StbSecRoll-H`・`StbSecBuild-H` | I 形 | 局所の X がフランジの幅 `B`、Y がせい `A`、ウェブ `t1`、フランジ `t2` |
| `StbSecRoll-BOX` | 角形鋼管 | 局所の X が `B`、Y が `A`、厚さ `t` |
| `StbSecBuild-BOX` | 角形鋼管 | 局所の X が `B`、Y が `A`、縦の板 `t1`、横の板 `t2`（違えば外形と穴の多角形） |
| `StbSecPipe` | 円形鋼管 | 直径 `D`、厚さ `t` |

- 線の部材は、材軸に沿って節点の間の長さだけ押し出す。柱・ブレースは断面の中心を、梁は断面の上端の中心を節点の線に合わせる（梁は節点から下へ垂れる）。
- スラブは節点の面を上端とし、下へ厚さだけ押し出す。壁は節点の面を厚さの中心にする。節点が 1 つの平面に 1 mm を超えて乗らないときは、形を移さず `stbridge.shape.dropped`（`non_planar`）で知らせる。部材の種類と断面の形が合わない（梁に板の断面など）ときは `figure_mismatch` で知らせる。
- 建物モデルは丸みのある断面を評価しないので、鋼材の規格の丸み（`r`）は形に入れない。部材の名前ごとに `stbridge.fillet.dropped` で知らせ、丸みの値は規格のプロパティに残す。
- 寸法が欠ける・0 以下・板厚が外形に収まらない規格は形にしない。`shape` が指す規格がなければ `stbridge.reference.missing` で知らせる。形にしなかった断面の子は、`stbridge.element.unsupported` で知らせる。

## 移さないもの

- 配筋と、形にしない断面の子の要素（`StbSecBarArrangementBeam_RC`・杭や基礎の形の子・ハンチの梁など）。要素の名前ごとに `stbridge.element.unsupported` で知らせる。
- 寸法を引かない鋼材の規格（L 形・溝形・T 形・平鋼・丸鋼・製品など）。規格の要素の名前ごとに `stbridge.element.unsupported` で知らせる。
- 根と `StbModel` の下の、上の表にない要素（拡張の `StbExtensions` など）と、上の表にない部材。要素の名前ごとに `stbridge.element.unsupported` で知らせる。

## サンプル

- `crates/zumen-stbridge/tests/fixtures` のサンプルは、XML Schema に合わせて書いた合成のもので、実在の建物ではない。`frame-2.1.stb` は 2.1 の架構、`wall-2.0.stb` は Shift_JIS の 2.0 の壁と開口。
- `samples/stbridge` には、MIT で公開された実装の作者が自分のツールで作ったサンプルを置く（[公開サンプルの台帳](../../samples/ledger.toml)）。どれも実在の建物ではない。商用ソフトの出力は、権利が分からないので置かない。

| サンプル | 出どころ | 版 | 確かめること |
| --- | --- | --- | --- |
| `hoaryfox-sample-building.stb` | hrntsm/HoaryFox の `Samples` | 2.0.2 | 鉄骨の架構（柱 105・梁 192・ブレース 10）を移す。`StbCommon` に `app_version` がなく XML Schema に合わないが読む |
| `stb-diffchecker-mini210-a.stb`・`-b.stb` | NS-NS/STB-DiffChecker の `TestData` | 2.1.0 | 配筋の子の要素を知らせて移す。2 つの違い（断面の寸法と節点の位置）は、梁の形に表れる |

## まだしないこと

- 開口の形。開口の要素は形を持たないので、評価では壁・スラブに穴をあけられず、`building.boolean.failed`（`opening_missing`）で知らせて部材を欠けたものとする。
- 部材の端のずれ（`offset_start_X` など）、ハンチ・テーパー、端と中央で違う断面、杭・基礎・パラペットの形、配筋。
- 鋼材の丸みと、L 形・溝形などの規格の形。
- 書き出しと、IFC への変換。
- 2.1 系の公開サンプルを外部の実装と照らすことと、XML Schema でのサンプルの検査の CI。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `stbridge` | 読み | `external` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-stbridge)にあります。

- 合成のサンプル 2 件と MIT の公開サンプル 3 件を読み、部材の数・種類・配置・開口・診断と、断面の形の評価を試験で確かめています。合成の 2.1 の架構では、柱・梁・スラブの形の外接箱を確かめています。
- 2.0.2 の公開サンプル（HoaryFox）を STBDotNet 0.2.3 でも読み、部材・階の数と ID・名前、部材・階・断面・鋼材の規格の属性の値と参照、部材の配置の原点と材軸の向きを照らし、食い違いは 0 でした（[xtask/verify/stbridge](../../xtask/verify/stbridge/README.md)、毎晩の CI）。この 1 件を `external` の根拠とします。
- 断面の形の寸法と位置、局所の X の向き、階への所属は外部の実装と照らしていません。STBDotNet は 2.1 系を読まないので、2.1.0 の公開サンプルは `synthetic` の範囲です。
