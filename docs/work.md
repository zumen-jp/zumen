# 次の作業を選ぶ

利用者から作業を任されたエージェントと開発者の入口です。確認先と更新の役割は [決定 0020](decisions/0020-work-selection-and-handoff.md) で定めています。

## 何を確認するか

| 知りたいこと | 確認先 |
| --- | --- |
| 何を優先するか、何がそろえば始められるか | 最新の main の、この一覧 |
| 何を作るか、どこまでで完了か | 対象 issue の本文とコメント |
| すでに決まったこと | `docs/decisions/` の採用済みの記録 |
| 誰かが進めていないか | issue の着手・引き継ぎの記録、開いた PR、手元の変更と worktree |
| 実際にどこまでできているか | マージ済みの PR、main のコードと試験 |

この一覧には、issue の開閉や進行中の状態を写しません。[全体計画 #1](https://github.com/zumen-jp/zumen/issues/1) は目的と段階を確認するために読みます。チェック欄だけで実装の有無を判断しません。

## 選び方

1. 利用者の依頼と、この作業場所に適用される `AGENTS.md` を読む。
2. 手元の変更を確認し、`origin/main` を取得して、この一覧の最新版を読む。進行中の変更を上書きしない。
3. 一覧を上から見て、候補 issue の本文・コメント・関連 PR と開始条件を確認する。依存する issue 全体が閉じる必要はなく、書かれた成果物が main に入っていればよい。
4. 作業中の範囲を重ねない。同じ issue でも、分担が明記され、変更範囲が独立しているなら着手できる。明記されていなければ次の候補を見る。
5. 最初に「対象 issue」「今回作るもの」「検査方法」を示す。大きな issue は、完了条件の一部を満たす PR に分ける。

着手したら、対象 issue の本文に作業ブランチ・担当する範囲・引き継ぎ先を記録する。PR を作ったらそのリンクに置き換える。古い着手記録は、関連 PR とブランチの状態を確かめてから扱う。経過時間だけで担当が空いたとは判断しない。

GitHub を確認できない場合は、候補と未確認の点を報告する。指定済みの作業の調査やローカルの検証は進められるが、未着手と断定して別の作業を選ばない。

## 優先順と次の一手

順番は、図面を読み書きする最初の経路を通すことを優先しています。前の行が作業中か、開始条件を満たさないときは、次の行を確認します。利用者が指定した作業は、この順番より優先します。

| 順 | 作業 | 開始条件 | 次の一手 |
| --- | --- | --- | --- |
| 1 | [#64 DXF・DWG の編集と書き戻し](https://github.com/zumen-jp/zumen/issues/64) | #3 の共有のオブジェクトの表と、DXF・DWG（R2000）のコーデックが main にある | #3 で、DXF と DWG（R2000）を [決定 0023](decisions/0023-acad-shared-object-table.md) の共有の表に読み、型付きの見方と `lift` が同じ結果になることを確かめた（[DXF の扱い](formats/dxf.md)・[DWG の扱い](formats/dwg.md)）。境界の `open` から DWG（R2000）を読めるようにした。DXF のレコードとタグの追加・削除と、`$HANDSEED` からのハンドルの払い出しをトランザクションで扱えるようにした。共有の表の値と見出しの変数の変更を、DXF に書き戻せるようにした（[決定 0024](decisions/0024-acad-table-write-back.md)）。表のオブジェクトの追加と削除も書き戻せるようにし（[決定 0025](decisions/0025-acad-object-add-remove-write-back.md)）、書き戻した表のハンドルに合わせて `$HANDSEED` を進め、変えた図形に合わせて図面の範囲を広げ、除いたオブジェクトへの辞書の項目と 102 の組の参照も除くようにした。R2004 の DWG は、入れ物の区切りを展開し（決定 0026）、見出し・クラス・オブジェクトを展開した区切りから読むようにした。R2010・R2013・R2018 の DWG も、同じ入れ物から読むようにした（[決定 0027](decisions/0027-dwg-r2010-reading.md)）。R2000 の DWG のオブジェクトのデータを置き換え、地図を作り直して書けるようにした（[決定 0028](decisions/0028-dwg-object-data-edit.md)）。共有の表の LINE・CIRCLE・ARC の型ごとの欄の変更を、R2000 の DWG に書き戻せるようにした（[決定 0029](decisions/0029-dwg-table-write-back.md)）。POINT・ELLIPSE・RAY・XLINE・SOLID・TRACE・3DFACE も書き戻せるようにした（[決定 0030](decisions/0030-dwg-write-back-more-entities.md)）。どの種類の図形も、共通の欄の色・線種の尺度・非表示を書き戻せるようにした（[決定 0031](decisions/0031-dwg-write-back-entity-common.md)）。TEXT・INSERT・MINSERT の型ごとの欄も書き戻せるようにした（[決定 0032](decisions/0032-dwg-write-back-text-insert.md)）。ATTRIB・ATTDEF も書き戻せるようにした（[決定 0033](decisions/0033-dwg-write-back-attributes.md)）。MTEXT も書き戻せるようにした（[決定 0034](decisions/0034-dwg-write-back-mtext.md)）。LWPOLYLINE も書き戻せるようにした（[決定 0035](decisions/0035-dwg-write-back-lwpolyline.md)）。SPLINE も書き戻せるようにした（[決定 0036](decisions/0036-dwg-write-back-spline.md)）。HATCH も書き戻せるようにした（[決定 0037](decisions/0037-dwg-write-back-hatch.md)）。POLYLINE・VERTEX も書き戻せるようにした（[決定 0038](decisions/0038-dwg-write-back-polyline.md)）。DIMENSION も書き戻せるようにした（[決定 0039](decisions/0039-dwg-write-back-dimension.md)）。TOLERANCE・LEADER・MLINE も書き戻せるようにした（[決定 0040](decisions/0040-dwg-write-back-annotations.md)）。VIEWPORT も書き戻せるようにした（[決定 0041](decisions/0041-dwg-write-back-viewport.md)）。SHAPE も書き戻せるようにし、表に型ごとの欄を置く図形はどれも書き戻せる（[決定 0042](decisions/0042-dwg-write-back-shape.md)）。表のレコードは LAYER と STYLE の欄を書き戻せるようにした（[決定 0043](decisions/0043-dwg-write-back-layer-style.md)）。LTYPE も書き戻せるようにした（[決定 0044](decisions/0044-dwg-write-back-ltype.md)）。VIEW・UCS・VPORT（[決定 0045](decisions/0045-dwg-write-back-view-ucs-vport.md)）と DIMSTYLE（[決定 0046](decisions/0046-dwg-write-back-dimstyle.md)）、OBJECTS の GROUP と DICTIONARYVAR（[決定 0047](decisions/0047-dwg-write-back-group-dictionaryvar.md)）、DICTIONARY と MLINESTYLE（[決定 0048](decisions/0048-dwg-write-back-dictionary-mlinestyle.md)）、LAYOUT（[決定 0049](decisions/0049-dwg-write-back-layout.md)）、XRECORD（[決定 0050](decisions/0050-dwg-write-back-xrecord.md)）も書き戻せるようにした。表から除いた図形も書き戻せるようにした（[決定 0051](decisions/0051-dwg-write-back-entity-removal.md)）。表の終わりに足した単純な図形も書き戻せるようにした（[決定 0052](decisions/0052-dwg-write-back-entity-addition.md)）。足した TEXT・ATTDEF・INSERT も書き戻せるようにした（[決定 0053](decisions/0053-dwg-write-back-pointing-entity-addition.md)）。足した MTEXT も書き戻せるようにした（[決定 0054](decisions/0054-dwg-write-back-mtext-addition.md)）。属性を持つ INSERT の追加も書き戻せるようにした（[決定 0055](decisions/0055-dwg-write-back-attributed-insert-addition.md)）。足した LWPOLYLINE も書き戻せるようにした（[決定 0056](decisions/0056-dwg-write-back-lwpolyline-addition.md)）。足した SPLINE も書き戻せるようにした（[決定 0057](decisions/0057-dwg-write-back-spline-addition.md)）。足した SHAPE は、形を表で示せないので受け付けないと決めた（[決定 0058](decisions/0058-dwg-write-back-shape-addition.md)）。足した重みを持つ SPLINE と当てはめる点の SPLINE も書き戻せるようにした（[決定 0059](decisions/0059-dwg-write-back-weighted-and-fit-spline-addition.md)）。足した MINSERT（属性を持つものも）も書き戻せるようにした（[決定 0060](decisions/0060-dwg-write-back-minsert-addition.md)）。足した画層も書き戻せるようにした（[決定 0061](decisions/0061-dwg-write-back-layer-addition.md)）。次は、DWG（R2000）の、画層のほかの表のレコードの追加を書き戻す |
| 2 | [#28 図面モデルの要素と試験](https://github.com/zumen-jp/zumen/issues/28) | 決定 0022 と対応表が main にあり、対象部分の共通の型を #3 から使える | 最小のモデルを寸法・記号・ハッチング・下敷きなどへ広げる。対応表の E1〜E10 を対象ごとに試験にする。SXF・JWW のコーデック全体は別 issue に分ける |
| 3 | [#66 適合試験で比べる出力と入力](https://github.com/zumen-jp/zumen/issues/66) | `drawing` の op と DXF の組が main にある。要素の一覧の op は #28 の要素と合わせる | 要素の一覧を返す `entities` の op（決定 0012）を足して比べ、再配布できる実際の図面に近い入力を確かめて足す。停止の診断に位置を持たせるかを決める |
| 4 | [#6 バイナリの受け渡し](https://github.com/zumen-jp/zumen/issues/6) | 試作は合成データで開始できる。最終判断には issue が指定する測定用データが必要 | 転送時間・ページ側の時間・メモリ・wasm の増分を測る。図面モデル全体の完成を待たずに試す |
| 5 | [#9 プロファイル](https://github.com/zumen-jp/zumen/issues/9) | 旧版の `mlit-cad-2025` の項目と出典を確認できる | データの形式と版を決め、移行例を作る。図面要素の対応は決定 0022 と照らす |
| 6 | [#5 建物モデル](https://github.com/zumen-jp/zumen/issues/5) | 成分の整理と開口処理の試作は開始できる。平面図への変換には決定 0022 に沿った #3・#28 のモデルと切断処理が必要 | IFC の成分を整理し、開口の処理を小さく試す。成分への読み込みと階の平面図の試作へ進む |

#6 の測定用の図面は Git の外の `ZUMEN_SAMPLES` で扱い、入手できない測定結果を推測で埋めません。

#4 の設計後の実装は #3 と #28 に分けました。#28 は最初の読み書きと環境間の一致を確かめる作業の後に置き、モデル全体を作り終える前に経路を検証します。#3 と独立した部分を進める場合は、担当する型と要素を両方の issue に記します。

## 完了したとき・途中で引き継ぐとき

- PR の「対象と残る作業」に、対象 issue・満たした条件・残る条件を書く。issue の条件をすべて満たすときだけ `Closes #番号` を使う。
- 完了した行は、この一覧から同じ PR で取り除く。途中なら次の一手を残りの作業に直す。ほかの行の開始条件が変わる場合も更新する。
- 設計の issue を閉じても実装が残るなら、既存の実装 issue に関連づけるか、新しい issue を作ってこの一覧へ加える。閉じた issue の本文だけに残件を置かない。
- マージを確認してから、issue の着手記録・完了条件・親 issue の進捗を更新する。PR が閉じただけの場合は、完了とは扱わない。
- 中断時は、issue の本文に「できたことと PR／ブランチ」「残る条件」「検査結果」「待っているもの」「次の一手」を残す。待っているものには、必要な成果物か利用者の判断を具体的に書く。
- 新しく見つけた仕事は、目的・範囲・開始条件・完了条件・参照を issue に書き、優先順を変える理由と一緒に PR でこの一覧へ加える。

一覧に着手できるものがなければ、開いた issue と採用済みの決定の残件を確認します。追跡漏れなら一覧を補い、判断や外部の成果物が必要なら、その点を利用者に伝えます。
