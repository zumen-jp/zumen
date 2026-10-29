# 0148: 図形の拡張辞書のうち、アプリケーションの XRECORD を形式固有の情報にする

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-29 |
| 状態 | 提案 |

## 背景

[決定 0115](0115-drawing-format-extensions.md) は、図形の XDATA を形式固有の情報（`Extension`）にし、拡張辞書（102 の `{ACAD_XDICTIONARY`）は「まだ移さない」とした（#174 の「拡張辞書」）。拡張辞書は図形が持つ DICTIONARY で、項目の名前ごとに、アプリケーションが XRECORD や独自のオブジェクトを置く。

手元の試料の DXF・DWG の図形の拡張辞書には、次の項目があった。

- 表（ACAD_TABLE）と MTEXT の、版の往復のための XRECORD（`ACAD_XREC_ROUNDTRIP`・`ACAD_MTEXT_RT` など）
- 文字とハッチングの注釈の尺度の辞書（`AcDbContextDataManager`）
- 寸法の関連づけ（`ACAD_DIMASSOC` の DIMASSOC）
- 記号（INSERT）の動的ブロックの表し方の辞書（`AcDbBlockRepresentation`）と、切り抜き（`ACAD_FILTER` の辞書の SPATIAL_FILTER）

また、DWG の読み手は図形の拡張辞書のハンドルを読み捨てていた。R2010 からの DWG の XRECORD の文字列は UTF-16LE なのに、R2000 の形（長さ・コード・ページ・バイト）で読んで、読み切れずに値が空になっていた。

この決定の最初の案は、すべての XRECORD を `dxf.xrecord` にし、値の前に `code` の欄を置いて並べた。[決定 0157](0157-drawing-model-fields-and-extensions.md)（提案）は、AutoCAD が自分の版の往復のために持つ値を L1 だけに残し、表示や意味を変える値をモデルの欄にし、`Field` にグループ・コードを足すとした。

## 選択肢

1. アプリケーションの XRECORD を形式固有の情報にし、AutoCAD の版の往復の XRECORD は L1 に残す。値は、グループ・コードを持つ `Field` で並べる。意味の分かる項目（注釈の尺度・寸法の関連づけなど）は、モデルの欄を足して移し、それまでは診断する。
2. すべての XRECORD を形式固有の情報にし、値の前に `code` の欄を置いて並べる（この決定の最初の案）。
3. 項目の種類ごとに、すぐに意味の分かる値にする（DIMASSOC を寸法の関連づけに、など）。
4. 拡張辞書を移さないまま、診断だけを返す。

2 は、AutoCAD の内部の控えを使い手に見せる。往復の XRECORD は図形の値の写しなので、使い手が読んでも新しいことが分からず、書き戻しは L1 の値を残す（[決定 0139](0139-drawing-apply.md)）。3 は、欄を決めないまま意味を写すことになる。欄は、0157 の線引きで決めてから足す。4 では、XRECORD に入ったアプリケーションの値を使い手が読めない。

## 決定

選択肢 1 を提案する。XRECORD の値の意味はアプリケーションが決めるので、XDATA と同じく、意味を推定せずに元の値を失わずに持つのがよい。XDATA と違い、XRECORD のグループ・コードは決まった意味を持たないので、コードも値として持つ。

- **アプリケーションの XRECORD。** 図形（記号の定義の中身も含む）の拡張辞書の XRECORD の項目を、名前空間 `dxf.xrecord`、項目の名前を名前とする形式固有の情報にする。値の型の契約の版は 1 とする。版・付く先の ID・読んだときの付く先のハッシュ（[決定 0156](0156-drawing-stable-ids-and-preconditions.md)）・出所は、XDATA の形式固有の情報と同じに持つ。
- **値。** XRECORD の複製の扱い（280）の後のデータを、記録の順に、元のグループ・コードを持つ `Field`（0157）で並べる。型付きの値は、文字列・整数（真偽は 0 と 1）・実数・ハンドル・バイナリ・点とする。点は、XDATA と同じく X・Y・Z のコード（10・20・30 など）を 1 つの値に合わせる。
- **AutoCAD の版の往復の XRECORD。** AutoCAD が自分の版の往復のために置く項目（`ACAD_XREC_ROUNDTRIP`・`ACAD_MTEXT_RT` など。名前の一覧は docs/formats/dxf.md に書く）は、図面モデルへ写さず、診断もしない。L1 に残り、書き戻しで保たれる。中身のうち、図形の値で表せない新しい版の内容は、0157 のとおり意味を読んでモデルの欄にし、読めない間は診断する。
- **意味の分かる項目。** 注釈の尺度（`AcDbContextDataManager`）・寸法の関連づけ（`ACAD_DIMASSOC` の DIMASSOC）・動的ブロックの表し方（`AcDbBlockRepresentation`）は、表示や寸法の意味を変えるので、図面モデルの欄にする（0157）。欄を足すまでは移さず、`dxf.extension-dictionary.dropped` を返す。記号の切り抜き（`ACAD_FILTER`）は、[決定 0149](0149-drawing-symbol-clips.md) が記号の切り抜きにする。
- **ほかの項目。** XRECORD でない項目は移さず、`dxf.extension-dictionary.dropped`（項目の名前と種類ごと）を返す。項目の指すオブジェクトがないものは、移すものがないので診断しない。
- **DWG の読み手。** 図形の拡張辞書のハンドルを、DXF と同じく所有者の前に `102 {ACAD_XDICTIONARY`・360・`102 }` の組で置く。R2010 からの DWG の XRECORD の文字列は、文字の数（RS）と UTF-16LE で読む。
- **作業量。** 拡張辞書の項目と XRECORD の値は、移すたびに作業量として数える。同じ辞書を複数の図形が指す場合も、複製ごとに数え、上限を超える前に止める。

## 影響

- 試料の `sample_AC10*` の図面（R12 の AC1009 を除く）の図形の XRECORD は、表と MTEXT の往復のためのものなので、図面モデルへ写らない。DWG の読み手を直したので、L1 の XRECORD の値は、同じ版の DWG と DXF で合う（ハンドルを除く）。
- 今の main は、往復の XRECORD も `dxf.xrecord` にし、値の前に `code` の欄を置いている。この決定を採ったら、往復の XRECORD を外し、`Field` のコードに移す。
- 図形でないオブジェクト（画層・ブロックのレコード・表のスタイルなど）の拡張辞書と、図面全体の名前付きの辞書の中身は、まだ移さない。
  - [決定 0152](0152-drawing-layer-line-type-xrecords.md) が、画層と線種の拡張辞書の XRECORD を移す。
- 書き戻しは、XDATA と同じく L1 の値を残し、形式固有の情報からは書かない（0139）。
- AutoCAD は DXF を書くとき、拡張辞書を別のハンドルで作り直すことがある（試料の R2000 の DXF のハッチングなど）。DWG と DXF の比べ方は、拡張辞書の組を比べない。

## 参照

- [決定 0115](0115-drawing-format-extensions.md)、[決定 0022](0022-drawing2d-semantics-and-format-mapping.md)、[決定 0139](0139-drawing-apply.md)、[決定 0149](0149-drawing-symbol-clips.md)、[決定 0152](0152-drawing-layer-line-type-xrecords.md)、[決定 0156](0156-drawing-stable-ids-and-preconditions.md)、[決定 0157](0157-drawing-model-fields-and-extensions.md)
- DXF Reference「Extension Dictionary and Persistent Reactors」「XRECORD」「Common Group Codes for Entities」
- ODS 5.4.1 の 20.4.89（XRECORD）
- #174
