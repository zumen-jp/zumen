# 0152: 画層と図形の透過性と画層の説明は図面モデルの欄にし、画層と線種のアプリケーションの値は、ID で付く形式固有の情報にする

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-29 |
| 状態 | 提案 |

## 背景

[決定 0148](0148-drawing-extension-dictionary-xrecords.md) は、図形の拡張辞書の XRECORD を形式固有の情報にし、図形でないオブジェクトの拡張辞書は「まだ移さない」とした（#174 の「拡張辞書」）。形式固有の情報の付く先（`Target`）は、図面全体と、空間と用紙の要素だけだった。

手元の試料の `sample_AC10*` の図面では、次の図形でないオブジェクトが拡張辞書を持つ。

- 画層: 注釈の尺度の画層（`0 @ 1` など）の `ASDK_XREC_ANNO_SCALE_INFO`。R2000 の DWG では、真の色と色見本帳の色を持つ画層の、版の往復のための `ACAD_XREC_ROUNDTRIP` も持つ。
- 線種: R2010 までの DXF の、文字を添える線種（GAS_LINE）の `ACAD_XREC_ROUNDTRIP`。
- ブロックのレコード: SORTENTSTABLE・動的ブロックの辞書など、XRECORD でない項目だけ。
- 図面モデルに移さないオブジェクト（ビュー・材質・表示スタイル・表のスタイル・レイアウトなど）。

画層は、XDATA も持つ。`sample_AC1032` の DXF の 17 の画層は、透過性（`AcCmTransparency` の 1071）と説明（`AcAecLayerStandard` の 2 つ目の 1000）を XDATA で持つ。図形は、透過性を 440 で持つ。透過性は表示を変える。

DWG の読み手は、図形でないオブジェクトの拡張辞書のハンドルを読み捨てていた。

この決定の最初の案は、画層と線種の拡張辞書の XRECORD を付け、読んだときの値に、レイヤと線種の名前・状態・定義・出所を持たせた。画層の XDATA は移さなかった。[決定 0156](0156-drawing-stable-ids-and-preconditions.md)（提案）は付く先を ID で指し、読んだときの値をハッシュにした。[決定 0157](0157-drawing-model-fields-and-extensions.md)（提案）は、表示を変える値をモデルの欄にし、AutoCAD の版の往復の値を L1 だけに残すとした。

## 選択肢

1. 透過性と説明はレイヤの欄に、図形の透過性はスタイルの欄にする。画層と線種のアプリケーションの XDATA・XRECORD は、レイヤと線種を ID で指す形式固有の情報にする。版の往復の XRECORD は L1 に残し、注釈の尺度は欄を足すまで診断する。
2. 画層と線種の拡張辞書の XRECORD だけを、名前・状態・定義・出所を読んだときの値にして付ける（この決定の最初の案）。
3. 図面全体（`Target::Document`）に付け、名前で画層と線種を指す。
4. 移さないまま、文書にだけ書く。

2 は、透過性という表示を変える値を移さない。読んだときの値も、レイヤと線種に欄が増えるたびに広げる必要がある。3 では、画層の名前や定義を変えても失効しない。4 では、値を使い手が読めない。

## 決定

選択肢 1 を提案する。

- **透過性。** `Style` に、透過性（`Inherit<Transparency>`）を足す。`Transparency` は、不透明の度合い（0〜255。255 が不透明）を持つ。`Layer` にも透過性を足す。
  - DXF・DWG の図形は、440 から読む。440 がなければ、レイヤから継承する。上位のバイトの 1 はブロックからの継承、2 は値で、値は下位のバイトとする（推定）。
  - 画層は、`AcCmTransparency` の XDATA の 1071 を、図形の 440 と同じ形で読む（推定）。値でなければ、不透明とする。
  - JWW・SXF は、透過性を持たないので、レイヤから継承し、レイヤは不透明とする。
- **説明。** `Layer.description: Option<String>` を足す。DXF・DWG は、`AcAecLayerStandard` の XDATA の 2 つ目の 1000 から読み、空なら `None` とする（推定）。
- **アプリケーションの値。** `Target` に、レイヤ（`Target::Layer`）と線種（`Target::LineType`）を足す。付く先は ID で指し、読んだときの値は付く先のハッシュとする（0156）。画層（LAYER）と線種（LTYPE）の表のレコードの拡張辞書の XRECORD を、[決定 0148](0148-drawing-extension-dictionary-xrecords.md) と同じ `dxf.xrecord` に、透過性と説明のほかの XDATA を、[決定 0115](0115-drawing-format-extensions.md) と同じ `dxf.xdata` にして付ける。
- **L1 だけの値と診断。** 版の往復の XRECORD（`ACAD_XREC_ROUNDTRIP`）は、0148 と同じく L1 に残す。注釈の尺度の画層の `ASDK_XREC_ANNO_SCALE_INFO` は、注釈の尺度の欄（0157）を足すまで移さず、`dxf.extension-dictionary.dropped` を返す。XRECORD でない項目も、移さずに同じ診断を返す。
- **DWG の読み手。** 図形でないオブジェクトの拡張辞書のハンドルも、DXF と同じく所有者の前に `102 {ACAD_XDICTIONARY`・360・`102 }` の組で置く。型ごとのハンドルを持たないオブジェクトで、ハンドルの流れが反応子と拡張辞書を読み切れなければ、拡張辞書を持たないとする。

## 影響

- 表示する使い手は、透過性を、色と同じく継承を解いて使う。解く関数は、色と同じく [決定 0143](0143-drawing-inherit-through-placements.md) と [決定 0159](0159-drawing-walk-placements.md) の形にする。
- 書式つきの文字の背景の透過性（441。今は元の値の `u32`）も、同じ `Transparency` にする。
- 試料の画層と線種の XRECORD は、注釈の尺度と版の往復のものなので、図面モデルへ写らない。L1 の値は、注釈の尺度の画層では同じ版の DWG と DXF で同じになる。版の往復の XRECORD は形式ごとに違う（DXF は線種、R2000 の DWG は画層に持つ）。
- 今の main は、画層と線種の XRECORD を、往復のものも含めて、名前・状態・定義・出所を読んだときの値にして付けている。画層の XDATA と図形の透過性は移していない。この決定を採ったら、ID とハッシュ、モデルの欄に移す。
- ブロックのレコード・寸法スタイル・文字スタイルと、図面モデルに移さないオブジェクトの拡張辞書と、図面全体の名前付きの辞書の中身は、まだ移さない。ブロックのレコードの項目は、試料では XRECORD でないものだけである。
- 書き戻しは、XDATA と同じく L1 の値を残し、形式固有の情報からは書かない（[決定 0139](0139-drawing-apply.md)）。
- DWG の書き戻しは、図形でないオブジェクトの拡張辞書の組を、読んだままのときだけ受け付ける。拡張辞書を足す・除く・差し替える変更は `change.edit.unsupported` を返す（0139）。足すレコードは、拡張辞書を持てない。
- 図面モデルの `Style`・`Layer`・`Target` に欄と場合が増えるので、それらを作るか網羅して分ける使い手は、欄と場合を足す必要がある。

## 参照

- [決定 0115](0115-drawing-format-extensions.md)、[決定 0139](0139-drawing-apply.md)、[決定 0143](0143-drawing-inherit-through-placements.md)、[決定 0148](0148-drawing-extension-dictionary-xrecords.md)、[決定 0156](0156-drawing-stable-ids-and-preconditions.md)、[決定 0157](0157-drawing-model-fields-and-extensions.md)、[決定 0159](0159-drawing-walk-placements.md)
- DXF Reference「Extension Dictionary and Persistent Reactors」「LAYER」「LTYPE」「Common Group Codes for Entities」（440）
- #174
