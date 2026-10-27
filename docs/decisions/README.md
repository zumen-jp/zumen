# 設計の判断の記録

選択肢、選んだ理由、その影響を 1 件ずつ記録します。

- ファイル名は `NNNN-<主題>.md` にし、番号は 0001 からの連番にします。主題は英語の小文字とハイフンで書きます。
- 決めたあとは内容を書き換えません。判断を変えるときは新しい記録を書き、元の記録からリンクします。表記の基準（[docs/style.md](../style.md)）に合わせる直しは、意味を変えないので例外にします（[0021](0021-notation-and-terms.md)）。
- 「提案」は、まだ採用していないことを表します。
- 旧版（[zumen-jp/zumen-legacy](https://github.com/zumen-jp/zumen-legacy)）の記録は、「旧版の決定 0013」のように書いてリンクします。

## テンプレート

```markdown
# NNNN: 決定を表す短い文

| 項目 | 内容 |
| --- | --- |
| 日付 | YYYY-MM-DD |
| 状態 | 提案 / 採用 / NNNN に置き換え |

## 背景

## 選択肢

## 決定

## 影響

## 参照
```

## 一覧

- [0001: 新しい org zumen-jp で、コンセプトだけを引き継いで作り直す](0001-rebuild-in-zumen-jp.md)（2026-10-24、採用）
- [0002: ライセンスを MIT OR Apache-2.0 にする](0002-dual-license.md)（2026-10-24、採用）
- [0003: パッケージの名前を org から切り離し、識別子にドメイン zumen.dev を使う](0003-names-and-domain.md)（2026-10-24、採用）
- [0004: 中心のリポジトリを 1 つにし、配布・権利・非公開の都合があるものだけを分ける](0004-repositories.md)（2026-10-24、提案）
- [0005: 日本語を正にする](0005-japanese-first.md)（2026-10-24、採用）
- [0006: 各言語への対応を 3 つの段に分ける](0006-language-tiers.md)（2026-10-24、採用）
- [0007: 読み書きと解析を Rust の 1 つの実装にし、どの環境でも同じ出力を返す](0007-single-rust-implementation.md)（2026-10-24、採用）
- [0008: モデルをファイル・領域・派生の 3 層に分け、ファイルモデルを形式の系統ごとに 1 つにする](0008-model-layers-and-codecs.md)（2026-10-24、採用）
- [0009: 診断を code・場所・忠実度・型付きの値で返し、文言はカタログから引く](0009-diagnostics-and-catalog.md)（2026-10-24、採用）
- [0010: 予算を決まった作業量で数え、どの環境でも同じ所で止める](0010-budget-by-work.md)（2026-10-24、採用）
- [0011: 各言語からは C ABI と wasm を FFI で呼び、型は自前の生成器で作る](0011-bindings-through-ffi.md)（2026-10-24、採用）
- [0012: 各言語への境界を、数個の関数と、スキーマで定めたメッセージにする](0012-boundary-functions-and-messages.md)（2026-10-24、採用）
- [0013: 同じ境界を、ネイティブ・wasm・プロセスの外・HTTP の 4 つの形で出し、セッションごとに選ぶ](0013-boundary-in-four-forms.md)（2026-10-24、採用）
- [0014: 対象のプラットフォームを決め、成果物に署名し、言語の段に合わせて配る](0014-platforms-signing-and-distribution.md)（2026-10-24、採用）
- [0015: 境界は足すだけにし、4 つの版を分け、古い環境への下限を表で約束する](0015-add-only-boundary-and-version-floors.md)（2026-10-24、採用）
- [0016: 境界のメッセージの JSON を自前で読み書きし、スキーマの書き方と適合試験の比べ方を足す](0016-boundary-message-details.md)（2026-10-24、採用）
- [0017: 形式の権利を台帳で管理し、許諾の要る形式は同じ SDK で外部のプラグインにする](0017-format-rights-ledger-and-plugin-sdk.md)（2026-10-24、採用）
- [0018: 幾何は L2 で厳密な形を持ち、位相は許容差つきの折れ線で求め、許容差は用紙と実寸の型で持つ](0018-geometry-layers-and-tolerances.md)（2026-10-24、採用）
- [0019: 座標を空間ごとの型に分け、平面直角座標の換算を自前で持ち、国土地理院のデータは同梱せずに利用者が渡す](0019-coordinate-systems-and-gsi-data.md)（2026-10-24、採用）
- [0020: 作業の入口と優先順をリポジトリに置き、範囲と完了条件を issue で追う](0020-work-selection-and-handoff.md)（2026-10-24、採用）
- [0021: 表記と用語の基準を決め、textlint と prh で確かめる](0021-notation-and-terms.md)（2026-10-24、提案）
- [0022: 図面を空間・配置・意味のある要素で持ち、形式ごとの違いを長さの型と拡張に残す](0022-drawing2d-semantics-and-format-mapping.md)（2026-10-25、採用）
- [0023: AutoCAD の系統は、DXF と DWG が共有するオブジェクトの表を持ち、元のバイトは符号化ごとに持つ](0023-acad-shared-object-table.md)（2026-10-26、採用）
- [0032: DWG（R2000）への書き戻しは、TEXT・INSERT・MINSERT の型ごとの欄を、印を値から選んで符号化する](0032-dwg-write-back-text-insert.md)（2026-10-26、採用）
- [0033: DWG（R2000）への書き戻しは、ATTRIB・ATTDEF の型ごとの欄を、TEXT と同じ欄に続けて符号化する](0033-dwg-write-back-attributes.md)（2026-10-26、採用）
- [0034: DWG（R2000）への書き戻しは、MTEXT の表にない欄を元のビットから写して符号化する](0034-dwg-write-back-mtext.md)（2026-10-26、採用）
- [0035: DWG（R2000）への書き戻しは、LWPOLYLINE の続く欄の印を、値があるかから選んで符号化する](0035-dwg-write-back-lwpolyline.md)（2026-10-26、採用）
- [0036: DWG（R2000）への書き戻しは、SPLINE の場合を元のまま保ち、それに合う欄を符号化する](0036-dwg-write-back-spline.md)（2026-10-26、採用）
- [0037: DWG（R2000）への書き戻しは、HATCH の欄を表の順に読み直して符号化し、境界の図形の数の変更は受け付けない](0037-dwg-write-back-hatch.md)（2026-10-27、採用）
- [0052: DWG（R2000）への書き戻しは、表の終わりに足した単純な図形を一から符号化し、ブロックの並びの終わりに繋いで HANDSEED を進める](0052-dwg-write-back-entity-addition.md)（2026-10-27、採用）
- [0053: DWG（R2000）への書き戻しで足す TEXT・ATTDEF・INSERT は、指す文字の体裁とブロックを名前で探してハンドルを書く](0053-dwg-write-back-pointing-entity-addition.md)（2026-10-27、採用）
- [0054: DWG（R2000）への書き戻しで足す MTEXT は、表にない文字の範囲の大きさと行の間隔の後の B を 0 にする](0054-dwg-write-back-mtext-addition.md)（2026-10-27、採用）
- [0055: DWG（R2000）への書き戻しで足す属性を持つ INSERT は、すぐ後の ATTRIB と SEQEND を INSERT に持たせる](0055-dwg-write-back-attributed-insert-addition.md)（2026-10-27、採用）
- [0056: DWG（R2000）への書き戻しで足す LWPOLYLINE は、表にない印のビットと一定の幅を 0 にする](0056-dwg-write-back-lwpolyline-addition.md)（2026-10-27、採用）
- [0057: DWG（R2000）への書き戻しで足す SPLINE は、節点と制御点を持ち重みを持たないものだけを、表にない印を 0 にして書く](0057-dwg-write-back-spline-addition.md)（2026-10-27、採用）
- [0058: DWG（R2000）への書き戻しでは、足した SHAPE を受け付けない](0058-dwg-write-back-shape-addition.md)（2026-10-27、採用）
- [0059: DWG（R2000）への書き戻しで足す SPLINE は、重みを持てば有理の印を立て、当てはめる点を持てば場合を 2 にする](0059-dwg-write-back-weighted-and-fit-spline-addition.md)（2026-10-27、採用）
- [0060: DWG（R2000）への書き戻しで足す INSERT は、列と行の数か間隔を持てば MINSERT として書く](0060-dwg-write-back-minsert-addition.md)（2026-10-27、採用）
- [0061: DWG（R2000）への書き戻しで足す画層は、一から符号化して画層の表のオブジェクトの項目の終わりに加える](0061-dwg-write-back-layer-addition.md)（2026-10-27、採用）
- [0062: DWG（R2000）への書き戻しで足す文字の体裁は、画層と同じく文字の体裁の表のオブジェクトの項目の終わりに加える](0062-dwg-write-back-style-addition.md)（2026-10-27、採用）
- [0063: DWG（R2000）への書き戻しで足す線種は、形も文字もない要素だけのものを線種の表のオブジェクトの項目の終わりに加える](0063-dwg-write-back-ltype-addition.md)（2026-10-27、採用）
- [0064: DWG（R2000）への書き戻しで足すアプリケーションの名前は、使い道の分からない RC を 0 として表のオブジェクトの項目の終わりに加える](0064-dwg-write-back-appid-addition.md)（2026-10-27、採用）
- [0065: DWG（R2000）への書き戻しで足す寸法の体裁は、表にない欄を既定の値で書き、表のオブジェクトの項目の終わりに加える](0065-dwg-write-back-dimstyle-addition.md)（2026-10-27、採用）
- [0066: DWG（R2000）への書き戻しで足すビューは、型ごとの欄をみな持ち、UCS を伴うなら UCS を指すものを表のオブジェクトの項目の終わりに加える](0066-dwg-write-back-view-addition.md)（2026-10-27、採用）
- [0067: DWG（R2000）への書き戻しで足すビューポートの設定は、型ごとの欄をみな持ち、使い道の分からないビットを偽として表のオブジェクトの項目の終わりに加える](0067-dwg-write-back-vport-addition.md)（2026-10-27、採用）
- [0068: DWG（R2000）への書き戻しで足すブロックは、レコードと BLOCK・ENDBLK と図形を揃えて加え、レコードを表のオブジェクトの項目の終わりに足す](0068-dwg-write-back-block-record-addition.md)（2026-10-27、採用）
- [0069: DWG（R2000）への書き戻しで変えた INSUNITS・PLIMMIN・PLIMMAX は、見出しの変数の区切りの長さを変えずにその場で書き直す](0069-dwg-write-back-header-variables.md)（2026-10-27、採用）
- [0070: R2004 の入れ物は、置き換えた区切りだけを圧縮し直し、ページを並べ直して地図と見出しを作り直して書く](0070-dwg-r2004-container-write.md)（2026-10-27、採用）
- [0071: R2004 の DWG の変更は、オブジェクトの区切りの後ろに足し、地図の区切りを作り直して、入れ物の区切りを置き換える](0071-dwg-r2004-object-edit.md)（2026-10-27、採用）
- [0072: 共有の表の変更は、R2004 の DWG には図形の共通の欄と単純な種類の型ごとの欄から書き戻す](0072-dwg-r2004-write-back.md)（2026-10-27、採用）
- [0073: R2004 の DWG への書き戻しは、R2000 と同じ形の種類を広げ、後ろに増えた欄は元のビットを写す](0073-dwg-r2004-write-back-kinds.md)（2026-10-27、採用）
- [0074: R2004 の DWG への HATCH の書き戻しは、初めのグラデーションの欄を元のビットのまま写す](0074-dwg-r2004-write-back-hatch.md)（2026-10-27、採用）
- [0075: R2004 の DWG への表のレコードの書き戻しは、色を CMC の形で書く](0075-dwg-r2004-write-back-records.md)（2026-10-27、採用）
- [0076: R2004 の DWG への図形でないオブジェクトの書き戻しは、LAYOUT を除いて R2000 と同じに符号化する](0076-dwg-r2004-write-back-objects.md)（2026-10-27、採用）
- [0077: R2004 の DWG への図形の除きは、ブロックのレコードの持つ図形の数とハンドルを直して書き戻す](0077-dwg-r2004-write-back-entity-removal.md)（2026-10-27、採用）
- [0078: R2004 の DWG の見出しの変数は、展開した見出しの区切りの中で書き直し、区切りを置き換える](0078-dwg-r2004-header-variables.md)（2026-10-27、採用）
- [0079: R2004 の DWG への図形の足しは、R2004 の共通の欄で符号化し、ブロックのレコードの持つ図形に加える](0079-dwg-r2004-write-back-entity-addition.md)（2026-10-27、採用）
- [0080: R2004 の DWG に足す属性を持つ INSERT は、ATTRIB の数とすべての ATTRIB のハンドルを持つ](0080-dwg-r2004-write-back-attributed-insert-addition.md)（2026-10-27、採用）
- [0081: R2004 の DWG に足す表のレコードは、R2004 の共通の欄と CMC の色で符号化し、ブロックのレコードは持つ図形をみな指す](0081-dwg-r2004-write-back-record-addition.md)（2026-10-27、採用）
- [0082: R2004 の LAYOUT は、印刷するビューをハンドルで持ち、陰影の印刷の欄とビューポートの数を持つ形で読み書きする](0082-dwg-r2004-layout.md)（2026-10-27、採用）
- [0083: R2010 から後の DWG のオブジェクトは、ハンドルの流れの大きさ（MC）から始まるデータで置き換え、除き、足す](0083-dwg-r2010-object-edit.md)（2026-10-27、採用）
- [0084: R2010 から後の DWG の見出しの変数は、主のデータの中で、ビットの数を変えずに書き換える](0084-dwg-r2010-header-variables.md)（2026-10-27、採用）
- [0085: R2010 から後の DWG への書き戻しは、文字列を持たない単純な図形の共通の欄と型ごとの欄から始める](0085-dwg-r2010-write-back.md)（2026-10-27、採用）
- [0086: R2010 から後の DWG の文字列を持つ図形は、型ごとの欄と一緒に文字列の流れを書き直す](0086-dwg-r2010-write-back-strings.md)（2026-10-27、採用）
- [0087: R2010 から後の DWG の MTEXT と VERTEX は、R2010 で増えた欄を表と元のビットから書く](0087-dwg-r2010-write-back-mtext-vertex.md)（2026-10-27、採用）
- [0088: R2010 から後の DWG の DIMENSION・HATCH・TOLERANCE・VIEWPORT・SHAPE は、版ごとの欄を読み分けて書き戻す](0088-dwg-r2010-write-back-more-entities.md)（2026-10-27、採用）
- [0089: R2010 から後の DWG の表のレコードは、名前の文字列を写し、版ごとの欄を読み分けて書き戻す](0089-dwg-r2010-write-back-records.md)（2026-10-27、採用）
- [0090: R2010 から後の DWG の図形でないオブジェクトは、R2004 と同じ欄の符号化で、文字列を文字列の流れに書いて書き戻す](0090-dwg-r2010-write-back-objects.md)（2026-10-27、採用）
- [0091: R2010 から後の DWG から除いた図形は、R2004 と同じく、ブロックのレコードが持つ図形の並びから除いて書き戻す](0091-dwg-r2010-write-back-entity-removal.md)（2026-10-27、採用）
- [0092: R2010 から後の DWG に足した図形は、文字列の流れの形が R2000 と同じ種類だけを、ブロックのレコードの持つ図形に加えて書き戻す](0092-dwg-r2010-write-back-entity-addition.md)（2026-10-27、採用）
- [0093: R2010 から後の DWG に足した MTEXT・属性を持つ INSERT・SPLINE は、欄の形を確かめた版だけで書き戻す](0093-dwg-r2010-write-back-more-entity-addition.md)（2026-10-27、採用）
- [0094: R2010 から後の DWG に足した表のレコードは、LAYER・STYLE・LTYPE・APPID だけを、名前を文字列の流れに書いて書き戻す](0094-dwg-r2010-write-back-table-record-addition.md)（2026-10-27、採用）
- [0038: DWG（R2000）への書き戻しは、POLYLINE と VERTEX の欄を型ごとに符号化し、DXF の決まった値は変更を受け付けない](0038-dwg-write-back-polyline.md)（2026-10-27、採用）
- [0039: DWG（R2000）への書き戻しは、DIMENSION の表にない欄と印のビットを元のまま写す](0039-dwg-write-back-dimension.md)（2026-10-27、採用）
- [0040: DWG（R2000）への書き戻しは、TOLERANCE・LEADER・MLINE の表にない欄を元のまま写し、MLINE の数は表の値に合わせる](0040-dwg-write-back-annotations.md)（2026-10-27、採用）
- [0041: DWG（R2000）への書き戻しは、VIEWPORT の表の値を DWG の並びで書き、凍結した画層の数は元のまま写す](0041-dwg-write-back-viewport.md)（2026-10-27、採用）
- [0042: DWG（R2000）への書き戻しは、SHAPE の既定の値の欄を表になければ既定の値で書き、形の番号は元のまま写す](0042-dwg-write-back-shape.md)（2026-10-27、採用）
- [0043: DWG（R2000）への書き戻しは、LAYER と STYLE の表のレコードの欄を符号化し直し、名前とハンドルは変えない](0043-dwg-write-back-layer-style.md)（2026-10-27、採用）
- [0044: DWG（R2000）への書き戻しは、LTYPE の要素の数と形と文字の印を変えず、変わった文字列は領域に詰め直す](0044-dwg-write-back-ltype.md)（2026-10-27、採用）
- [0045: DWG（R2000）への書き戻しは、VIEW・UCS・VPORT の欄を DWG の並びで書き、UCS を伴う印は変えない](0045-dwg-write-back-view-ucs-vport.md)（2026-10-27、採用）
- [0046: DWG（R2000）への書き戻しは、DIMSTYLE の欄を DWG の並びで書き、表にない欄は既定の値で書く](0046-dwg-write-back-dimstyle.md)（2026-10-27、採用）
- [0047: DWG（R2000）への書き戻しは、OBJECTS のうち GROUP と DICTIONARYVAR の欄を書き直し、図形の数は変えない](0047-dwg-write-back-group-dictionaryvar.md)（2026-10-27、採用）
- [0048: DWG（R2000）への書き戻しは、DICTIONARY と MLINESTYLE の欄を書き直し、項目と線の数と体裁の名前は変えない](0048-dwg-write-back-dictionary-mlinestyle.md)（2026-10-27、採用）
- [0049: DWG（R2000）への書き戻しは、LAYOUT の欄を DWG の並びのまま書き直し、レイアウトの名前は変えない](0049-dwg-write-back-layout.md)（2026-10-27、採用）
- [0050: DWG（R2000）への書き戻しは、XRECORD のデータを元の並びのまま書き直し、文字列のコード・ページは元の値のものを使う](0050-dwg-write-back-xrecord.md)（2026-10-27、採用）
- [0051: DWG（R2000）への書き戻しは、表から除いた図形を地図から除き、前後の図形とブロックのレコードの並びのハンドルを直す](0051-dwg-write-back-entity-removal.md)（2026-10-27、採用）
- [0024: AutoCAD の系統の表の変更は、元の表と比べて違うレコードだけを符号化に書き戻す](0024-acad-table-write-back.md)（2026-10-26、採用）
- [0025: AutoCAD の系統の表に足したオブジェクトは、同じ所の次の残したオブジェクトの前に書き戻す](0025-acad-object-add-remove-write-back.md)（2026-10-26、採用）
- [0026: DWG の R2004 以降は、入れ物の区切りを展開したバイト列にしてから読む](0026-dwg-r2004-container.md)（2026-10-26、採用）
- [0027: DWG の R2010 から後の版を、R2004 の入れ物の読み方で読む](0027-dwg-r2010-reading.md)（2026-10-26、採用）
- [0028: DWG（R2000）の変更は、変えたオブジェクトと地図をファイルの後ろに足して書く](0028-dwg-object-data-edit.md)（2026-10-26、採用）
- [0029: 共有の表の変更は、DWG（R2000）の型ごとの欄を符号化し直して書き戻す](0029-dwg-table-write-back.md)（2026-10-26、採用）
- [0030: DWG（R2000）への書き戻しは、表にない値を読む側の既定の値として符号化する](0030-dwg-write-back-more-entities.md)（2026-10-26、採用）
- [0031: DWG（R2000）への書き戻しは、図形の共通の欄の色・線種の尺度・非表示を、どの種類の図形でも符号化する](0031-dwg-write-back-entity-common.md)（2026-10-26、採用）
