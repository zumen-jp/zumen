# 0211: IFC の L1 は zumen-step の文に番号の表を置き、IFC と SXF は FILE_SCHEMA の名前の判定で分ける

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

[決定 0207](0207-building-model-components.md) で、IFC の L1 をスキーマを知らないインスタンスのグラフにすると決めた。その形と、IFC-SPF・ifcZIP のコーデックの細部を決める（#407）。IFC-SPF と SXF（SFC・P21）は、どちらも `ISO-10303-21;` の行で始まり、SDK の判定の規則（`Bytes`・`Lines`）では分けられない。

## 選択肢

判定について。

1. 境界で SXF のコーデックに読ませ、`sxf.schema.unsupported` で失敗したら IFC として読み直す。
2. 判定の規則に、見出しの `FILE_SCHEMA` の名前で判定する種類を足す。

選択肢 1 は、判定がコーデックの読みに頼り、TypeScript の判定（決定 0007）をデータから作れない。

インスタンスのグラフについて。

1. 読むときに、すべての参照を前と逆の両方の向きで表にする。
2. 番号の表だけを読むときに作り、前向きの参照は値をたどって引き、逆向きの参照は求められたときに作る。

選択肢 1 は、逆向きの参照を使わない読み（書き戻しや、一部の要素だけの lift）でも、参照の数だけ表を作る。

## 決定

判定は選択肢 2、グラフは選択肢 2 を採る。

- **判定。** SDK の `Signature` に `StepSchema { prefix }` を足す。先頭の行が `ISO-10303-21;` で、見出しの節の `FILE_SCHEMA` の最初のスキーマの名前が、ASCII の大文字と小文字を区別せずに `prefix` で始まれば合う。見出しの文字列とコメントは飛ばし、`DATA;` が先に来るか、先頭のバイト列の中で名前が終わらなければ合わない。IFC は `IFC` で判定する。SXF は `FILE_SCHEMA` のないものも読むので `Lines` のままとし、判定は IFC を先に行う。
- **L1 の形。** crate `zumen-ifc` に置く。レコードは `zumen-step` の文をそのまま持ち、インスタンスの番号からレコードを引く表を作る。同じ番号は初めのものを引き、`ifc.duplicate.dropped` を報告する。値の中の参照は、並びと型を付けた値の中も含めて書いた順に引く。ファイルにない番号への参照は、読むときに `ifc.reference.missing` を報告する。逆向きの参照は `IfcFile::inverse` で作り、インスタンスと参照の数を `work` で数える。
- **書き出し。** 変えずに書くと元のバイト列と一致する（[決定 0008](0008-model-layers-and-codecs.md)）。
- **ifcZIP。** コーデック `IfcZip` は、ZIP の書庫（[決定 0210](0210-zip-l0-without-dependency.md)）の中の、名前が `.ifc` で終わる項目をちょうど 1 つ読む。なければ `ifc.entry.missing`、2 つ以上なら `ifc.entry.unsupported` で読まない。書くときは、元の書庫のバイト列をそのまま書く。インスタンスを変える操作（#393）を足すときに、IFC-SPF を同じ名前の項目に包み直す書き方を足す。
- **形式の台帳。** IFC-SPF も ifcZIP も、台帳の `ifc` の符号化とする。

## 影響

- 境界で IFC を開くこと（#415）は、判定の順を IFC・SXF の順にする。
- 建物モデルへの lift（#409）とスキーマでの検証（#410）は、この L1 の番号の表と参照の引き方を使う。
- その場の編集（#393）は、ifcZIP の包み直しを足す。

## 参照

- [決定 0007](0007-single-rust-implementation.md)、[決定 0008](0008-model-layers-and-codecs.md)、[決定 0120](0120-sxf-l1-exchange-structure.md)、[決定 0207](0207-building-model-components.md)、[決定 0210](0210-zip-l0-without-dependency.md)
- [docs/formats/ifc.md](../formats/ifc.md)
