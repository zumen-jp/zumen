# 0148: 図形の拡張辞書の XRECORD を、形式固有の情報にする

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

## 選択肢

1. XRECORD の項目を、XDATA と同じく形式固有の情報にする。値は、グループ・コードと型付きの値の組を順に並べる。XRECORD でない項目は移さず、診断する。
2. 項目の種類ごとに、意味の分かる値にする（DIMASSOC を寸法の関連づけに、SPATIAL_FILTER を記号の切り抜きにする、など）。
3. 拡張辞書を移さないまま、診断だけを返す。

## 決定

選択肢 1 を採る。

- XRECORD の値の意味はアプリケーションが決めるので、XDATA と同じく、意味を推定せずに元の値を失わずに持つのがよい。XDATA と違い、XRECORD のグループ・コードは決まった意味を持たないので、コードも値として持つ。
- 種類ごとの意味（選択肢 2）は、図面モデルに要る欄を決めてから、種類ごとに足す。
- 診断だけ（選択肢 3）では、XRECORD に入った値を使い手が読めない。

内容は次のとおり。

- 図形（記号の定義の中身も含む）の拡張辞書の XRECORD の項目を、名前空間 `dxf.xrecord`、項目の名前を名前とする形式固有の情報にする。値の型の契約の版は 1 とする。版・付く先・読んだときの付く先の値・出所は、XDATA の形式固有の情報と同じに持つ。
- 値は、XRECORD の複製の扱い（280）の後のデータを、記録の順に、グループ・コード（`code` の整数）と型付きの値の組で並べる。型付きの値の名前は、文字列 `text`・整数 `integer`（真偽は 0 と 1）・実数 `real`・ハンドル `handle`・バイナリ `binary` とする。点は X・Y・Z のコードごとに並べ、合わせない。
- XRECORD でない項目は移さず、`dxf.extension-dictionary.dropped`（項目の名前と種類ごと）を返す。項目の指すオブジェクトがないものは、移すものがないので診断しない。
- DWG の読み手は、図形の拡張辞書のハンドルを、DXF と同じく所有者の前に `102 {ACAD_XDICTIONARY`・360・`102 }` の組で置く。R2010 からの DWG の XRECORD の文字列は、文字の数（RS）と UTF-16LE で読む。

## 影響

- 試料の `sample_AC10*` の図面（R12 の AC1009 を除く）では、表と MTEXT の往復のための XRECORD が移り、同じ版の DWG と DXF で値が合う（ハンドルを除く）。
- 注釈の尺度・寸法の関連づけ・動的ブロックの表し方・記号の切り抜きは、まだ移さず、診断で分かる。とくに記号の切り抜き（`ACAD_FILTER`）は見た目を変えるので、図面モデルに記号の切り抜きを足すときに移す。
- 図形でないオブジェクト（画層・ブロックのレコード・表のスタイルなど）の拡張辞書と、図面全体の名前付きの辞書の中身は、まだ移さない。
  - [決定 0152](0152-drawing-layer-line-type-xrecords.md) が、画層と線種の拡張辞書の XRECORD を移す。
- 書き戻しは、XDATA の形式固有の情報と同じく、まだない。
- AutoCAD は DXF を書くとき、拡張辞書を別のハンドルで作り直すことがある（試料の R2000 の DXF のハッチングなど）。DWG と DXF の比べ方は、拡張辞書の組を比べない。

## 参照

- [決定 0115](0115-drawing-format-extensions.md)、[決定 0022](0022-drawing2d-semantics-and-format-mapping.md)
- DXF Reference「Extension Dictionary and Persistent Reactors」「XRECORD」「Common Group Codes for Entities」
- ODS 5.4.1 の 20.4.89（XRECORD）
- #174
