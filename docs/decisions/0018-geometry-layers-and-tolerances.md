# 0018: 幾何は L2 で厳密な形を持ち、位相は許容差つきの折れ線で求め、許容差は用紙と実寸の型で持つ

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- [#1](https://github.com/zumen-jp/zumen/issues/1) は、幾何を横断の部品（geom2d・geom3d）にした（[0008](0008-model-layers-and-codecs.md)）。
  - 2D: 頑健な判定（向き・交差）、ブーリアン、閉領域、オフセット、文字の配置
  - 3D: 押し出し・掃引・回転、面の集合、インスタンス、半空間での切断、開口のブーリアン、平面での切断
  - 3D のカーネルは trait の裏に置き、BIM ソフトの書き出し（[zumen-jp/zumen-legacy#128](https://github.com/zumen-jp/zumen-legacy/issues/128)）で測って選ぶ（[zumen-jp/zumen-legacy#129](https://github.com/zumen-jp/zumen-legacy/issues/129)）
  - 曲線（クロソイドを含む）は厳密な形で持ち、折れ線にするのは L3 で許容差を付けて行う
- 旧版は、曲線を折れ線にしてから、表示と解析をした。
  - 表示用のシーンは曲線を折れ線で持った。DXF の円弧は、用紙の 0.02 mm（半径の 1/10000 のほうが大きければそれ）を弦の誤差にして分けた。
  - 閉領域・罫線の表・版の比較は、シーンの折れ線の上で、用紙の 0.01 mm を許容差にして求めた（[旧版の決定 0015](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0015-analysis-in-zumen-core.md)）。丸い部屋や柱の面積は、折れ線の分だけ厳密な値と違った（試験の図面の円で 0.1 % 未満）。
  - 許容差は、操作ごとの定数か引数だった。文書の精度は持たなかった。
- [0007](0007-single-rust-implementation.md) は、どこでも同じ結果を求め、std の三角関数などを使わないと決めた。幾何カーネルは、wasm32 と native で同じ結果になることを選ぶ条件にした。
- [0009](0009-diagnostics-and-catalog.md) の忠実度 `approximated` は、許容差を値に持つ。[0010](0010-budget-by-work.md) の予算の単位 `segments` は、幾何の分割で作った点・線・面の数を数える。
- IFC は、表現の文脈（`IfcGeometricRepresentationContext`）に、精度（`Precision`）を書ける。
- 部品の状況（2026-10-24 に、crates.io とソースで確かめた）

| crate | 版 | ライセンス | 用途 | 決定性に関わること |
| --- | --- | --- | --- | --- |
| robust | 1.2.0 | MIT OR Apache-2.0 | 向き・円の内外の厳密な判定（Shewchuk の適応精度の述語） | 四則演算だけで計算する |
| i_overlay | 9.1.0 | MIT OR Apache-2.0（依存の i_float・i_shape は MIT） | 多角形のブーリアン・オフセット・線の切り取り。入力の辺の出所を結果に残せる | 浮動小数の座標を整数の格子に写して計算する。浮動小数の三角関数は、i_float が libm で計算する。rayon は feature を入れたときだけ使う。範囲を外れると panic する API と、失敗を返す API がある |
| cavalier_contours | 0.9.0 | MIT OR Apache-2.0 | 円弧を含む折れ線のオフセット | std の三角関数を使う |
| manifold-rust | 0.18.0 | Apache-2.0 | 3D のブーリアン（Manifold の Rust への移植、2026-07 から） | std の三角関数と HashMap（rustc-hash）を使う。rayon は feature を入れたときだけ使う |
| manifold-csg | 0.4.1 | Apache-2.0 OR MIT | 3D のブーリアン（C++ の Manifold を呼ぶ） | 作るのに cmake と C++ のコンパイラが要る |
| csgrs | 0.20.1 | MIT | 3D のブーリアン（BSP） | std の三角関数と HashMap（hashbrown）を使う。rayon は feature を入れたときだけ使う |
| boolmesh | 0.2.1 | MPL-2.0 | 3D のブーリアン | 寛容でないので使えない |

## 選択肢

### 解析をどの形の上で行うか

1. **折れ線の上**（旧版）。位相は頑健に求められる。ただし曲線の長さと面積が、分ける細かさで変わる。
2. **厳密な曲線の上。** 量は正しい。ただしスプラインやクロソイドどうしの交わりは数値の解き方に頼り、頑健さと、どこでも同じ結果を保つのが難しい。
3. **位相は許容差つきの折れ線で求め、量は元の形で求める。**

### 許容差の持ち方

1. **ライブラリ全体で 1 つの値**（旧版の 0.01 mm など）。用紙と実寸の区別がない。
2. **文書・操作・プロファイルで持ち、長さの型で用紙と実寸を分ける。**

### 3D のカーネル

1. **1 つのカーネルに決めて、直接呼ぶ。**
2. **trait の裏に置き、測って選ぶ。** あとから入れ替えられる。

## 決定

解析は 3、許容差は 2、3D のカーネルは 2 を選ぶ。

### 幾何の部品

| crate | 中身 |
| --- | --- |
| `zumen-geom2d` | 2D の点・ベクトル・変換、曲線（線分・円弧・楕円の弧・NURBS・クロソイド）、折れ線と領域、厳密な判定、曲線を折れ線にする処理、ブーリアン・オフセット・閉領域 |
| `zumen-geom3d` | 3D の点・ベクトル・配置、メッシュ、形状表現を評価する trait（カーネル）、平面での切断 |

- どちらも横断の部品で、L1 から L3 のどこからでも使う。形式も領域も知らない（[0008](0008-model-layers-and-codecs.md)）。
- 幾何の型は、座標の空間（用紙・実寸・プロジェクト・地図、[0019](0019-coordinate-systems-and-gsi-data.md)）を型の引数に持つ。空間の違う点を、混ぜて計算できないようにする。

### L2 は厳密な形を持つ

- L2 は、形式が書いた曲線を、その種類のまま持つ。折れ線にしない。
- 曲線を折れ線にするのは、次の所だけにする。
  - L3（表示・描画・解析）
  - 出力先が曲線を持てない書き出し（`lower`）。忠実度は `approximated` にし、弦の誤差を記録する（[0009](0009-diagnostics-and-catalog.md)）
- 折れ線にして作った点の数は、予算の `segments` で数える（[0010](0010-budget-by-work.md)）。

### 解析は、位相を折れ線で、量を元の形で求める

- **位相。** 交わり・閉領域・ブーリアン・オフセットは、許容差で折れ線にした形を、整数の格子に写して求める。
  - 格子の間隔は、許容差より細かくし、結果に記録する。
  - 格子は、操作の範囲の隅を原点にした相対の座標で作る。地図の座標（10^5 m 級）でも、桁を失わない。
  - ブーリアンとオフセットは、i_overlay を第一の候補にする。失敗を返す API だけを使い（[0010](0010-budget-by-work.md)）、rayon の feature は入れない。
- **判定。** 向き・交差・円の内外は、厳密な述語（robust）で判定する。浮動小数の比較に、場当たりの ε を置かない。
- **出所。** 結果の要素は、元の曲線と、その上の区間を出所に持つ（[0008](0008-model-layers-and-codecs.md)）。
- **量。** 長さ・面積は、閉じた式で求められる区間（線分・円弧）は元の形から求める。ほかの曲線（楕円の弧・NURBS・クロソイド）の区間は折れ線から求め、使った許容差を結果に付ける。
- **オフセット。** 最初は、折れ線にしてから行う。線分と円弧の形を円弧のまま返すオフセットは、要る場面が出たら足す。

### 文字の配置

- 字形の寸法は、呼び出し側が渡したフォントから、`text` の crate が読む（[0007](0007-single-rust-implementation.md)）。
- 配置（文字の枠・基準点・回転・縦書き・行送り）は、`text` が geom2d の型で返す。字形の輪郭を描くのは L3。

### 3D

- 形状表現は、L2 では評価せずに持つ。表示・切断・数量で要るときに評価する。
- 評価は、geom3d のカーネルの trait を通して呼ぶ。押し出し・掃引・回転、面の集合、インスタンス、半空間での切断、開口のブーリアン、平面での切断（→ 2D）。
- カーネルは、[zumen-jp/zumen-legacy#129](https://github.com/zumen-jp/zumen-legacy/issues/129) で、日本の BIM ソフトの書き出しで測って選ぶ。選ぶ条件は次のとおり。
  - wasm32 と native で同じ結果になる（[0007](0007-single-rust-implementation.md)）
  - 寛容なライセンス（[0011](0011-bindings-through-ffi.md)）で、作るのに C・C++ の道具が要らない。wasm32-unknown-unknown と、すべての対象（[0014](0014-platforms-signing-and-distribution.md)）で同じように作るため
  - 失敗しても panic しない（[0010](0010-budget-by-work.md)）
  - 失敗の割合・時間・メモリ・BIM の wasm のモジュールの大きさ（[0013](0013-boundary-in-four-forms.md)）
- 今の候補は、どれもそのままでは条件に合わない（背景の表）。feature で外せるか、上流が直すか、写して直すかも測る。
- 開口のブーリアンなどに失敗した要素は、失敗する前の形を返し、診断を付ける。黙って落とさない。

### 許容差

- 許容差は、長さの型を持つ。用紙の長さ（用紙の mm）と実寸の長さ（文書の長さの単位）を型で分け、混ぜて比べられないようにする（図面の長さの型、[#4](https://github.com/zumen-jp/zumen/issues/4)）。角度の許容差は別の型にする。
- 3 つの所で持つ。

  | 所 | 中身 |
  | --- | --- |
  | 文書 | 形式が書いた精度（IFC の `Precision` など）。書いていなければ持たない |
  | 操作 | 解析・変換・書き出しの引数。書かなければ既定値を使う |
  | プロファイル | 出力先の基準が決める値（[#9](https://github.com/zumen-jp/zumen/issues/9)） |

- 最初の既定値は、旧版から続ける。

  | 操作 | 既定値 |
  | --- | --- |
  | 閉領域・罫線の表・版の比較 | 用紙の 0.01 mm（旧版の決定 0015） |
  | 表示の弦の誤差（L3） | 用紙の 0.02 mm（旧版の DXF のシーン） |
  | 配置の中の実寸での解析 | 用紙の許容差を、その配置の縮尺で実寸に直した値 |

- 3D の既定値は、文書の精度があればそれを使う。なければ、カーネルを選ぶときに書き出しで測って決める。
- 使った許容差は、結果か診断の値（`approximated` の `tolerance`、[0009](0009-diagnostics-and-catalog.md)）に入れる。
- 許容差は、結果を決める入力として扱う。同じ入力と同じ許容差なら、どこでも同じ結果を返す（[0007](0007-single-rust-implementation.md)）。既定値を変えると結果が変わるので、リリースノートに書く。

### 決定性

- 幾何の部品と依存は、[0007](0007-single-rust-implementation.md) の規則に従う。浮動小数は四則演算・`sqrt` と libm だけを使い、HashMap の順を使わず、並列の結果は入力の順に集める。
- 判定は厳密な述語で、位相は整数の格子で求める。浮動小数の丸めの違いが、位相の違いにならない。
- 幾何の試験の出力も、native と wasm32 で同じハッシュになることを確かめる。
- 依存を足すときは、std の三角関数・HashMap・rayon の使い方を確かめ、記録に書く。

## 影響

- geom2d は、図面の系統（[#1](https://github.com/zumen-jp/zumen/issues/1) の段階 1）より前に作る。3D の開口のブーリアンと、字形の寸法による文字の配置は、段階 0 で先に試作する（#1）。
- robust と i_overlay を入れる PR で、依存を足す理由を記録する（AGENTS.md）。
- 旧版の閉領域・罫線の表・版の比較は、geom2d の上に書き直す。許容差の既定値は変えない。
- 楕円の弧・NURBS・クロソイドの長さと面積は、許容差の分だけ厳密な値と違う。結果に、そのことを書く。
- L3 の表示用のシーンの点の数は、弦の誤差で決まる。バイナリの受け渡し（[#6](https://github.com/zumen-jp/zumen/issues/6)）を測るときは、使った許容差を書く。
- [#11](https://github.com/zumen-jp/zumen/issues/11) は、この記録で完了する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「4. 中立モデルの中身」と「5. 層をまたぐ仕組み」の「幾何」、[#11](https://github.com/zumen-jp/zumen/issues/11)
- [0007](0007-single-rust-implementation.md)、[0008](0008-model-layers-and-codecs.md)、[0009](0009-diagnostics-and-catalog.md)、[0010](0010-budget-by-work.md)、[0011](0011-bindings-through-ffi.md)、[0013](0013-boundary-in-four-forms.md)、[0014](0014-platforms-signing-and-distribution.md)、[0019](0019-coordinate-systems-and-gsi-data.md)
- [旧版の決定 0015](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0015-analysis-in-zumen-core.md)、旧版の構成の [Texts, entities and images for servers](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/architecture.en.md#texts-entities-and-images-for-servers)（閉領域・罫線の表・版の比較の許容差）
- [zumen-jp/zumen-legacy#126](https://github.com/zumen-jp/zumen-legacy/issues/126)・[zumen-jp/zumen-legacy#128](https://github.com/zumen-jp/zumen-legacy/issues/128)・[zumen-jp/zumen-legacy#129](https://github.com/zumen-jp/zumen-legacy/issues/129)
- IFC 4.3 の [IfcGeometricRepresentationContext](https://ifc43-docs.standards.buildingsmart.org/IFC/RELEASE/IFC4x3/HTML/lexical/IfcGeometricRepresentationContext.htm)
- 2026-10-24 に確かめた crate: [robust](https://crates.io/crates/robust)、[i_overlay](https://crates.io/crates/i_overlay)（と [i_float](https://crates.io/crates/i_float)・[i_shape](https://crates.io/crates/i_shape)）、[cavalier_contours](https://crates.io/crates/cavalier_contours)、[manifold-rust](https://crates.io/crates/manifold-rust)、[manifold-csg](https://crates.io/crates/manifold-csg)、[csgrs](https://crates.io/crates/csgrs)、[boolmesh](https://crates.io/crates/boolmesh)
