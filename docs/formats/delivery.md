# 電子納品の成果品

電子納品の成果品（フォルダー・管理ファイル・図面・写真の集まり）を読む規則を記録します（[決定 0241](../decisions/0241-delivery-package-check.md)）。実装は `crates/zumen-delivery` です。境界では、`delivery_check` の op に、成果品の起点（`root`）か ZIP の入力（`archive`）を渡し、管理ファイルの要約を返します。要領の本文・DTD・作成例は、転載と改変が禁じられているので写さず、同梱しません（`formats/ledger.toml` の `delivery`）。

## 対象

- 工事完成図書・土木設計業務などの電子納品要領の成果品。要領の版では読み方を変えない。
- 成果品の中の図面は、この読み取りでは開かない。JPEG（SOI で始まるファイル）は、写真の照合のために、`zumen-photo` で Exif の撮影の日時と画素数だけを読む（#388）。その読み取りの診断は足さない。

## 入力

- `root`: `zm_push` で渡した入力のうち、名前が `root`（`/` で終わる）で始まるもの。起点からの相対の名前を、成果品の中の名前にする。
- `archive`: ZIP の入力。ディレクトリの項目を除き、項目の名前を成果品の中の名前にする。展開した量は予算（`decoded-bytes`）で数える。名前は、UTF-8 の印があれば UTF-8、なければ Shift_JIS として読む（`zumen-zip`）。
- 空の部分・`.`・`..`・`\` を含む名前と、`/` で始まる名前は、`delivery.name.unsafe` を返して除く。同じ名前は 1 つにする。
- 名前の大文字と小文字は、元のまま持つ。

## 管理ファイル

- 起点か、その直下のフォルダーにある拡張子 `.XML`（大文字と小文字を問わない）のファイルを、管理ファイルとする（`INDEX_C.XML`・`DRAWING/DRAWING.XML`・`PHOTO/PHOTO.XML` など）。
- 宣言の文字コード（UTF-8・Shift_JIS）で読む（`zumen-xml`）。XML として読めなければ、`zumen-xml` の診断に `delivery.file.invalid` を足し、管理ファイルの記録に移さない。
- DTD は処理しない。文書型宣言のシステム識別子（`INDE_C05.DTD` など）を、DTD の名前として持つ。管理ファイルはどれも文書型宣言を持つので、`xml.dtd.dropped` は返さない。DTD で宣言する実体の参照は、`xml.entity.unsupported` を返す。
- 要素ごとに、元の名前・文字の値（前後の空白を除く）・子の要素の記録を持つ。子の要素だけを持つ要素は、値を持たない。属性は、まだ持たない。

## ファイルの名前の解決

- 値がファイルの名前の形（空白を含まず、英字で始まる 1〜4 文字の英数字の拡張子を持つ）なら、管理ファイルのフォルダーにつないだ名前に解き、成果品の中にあるかを持つ。`4.0` のような数は、ファイルの名前にしない。
- そこになく、値がフォルダーを含まなければ、管理ファイルのフォルダーの下のフォルダー（`ORG`・`ORG001` など）で同じ名前のファイルを探し、1 つだけならそれに解く。
- `delivery_check` は、管理ファイルごとに、ファイルの名前の形の値の数と、成果品の中に見つからない名前を返す。

## 照合

プロファイル（`profiles`・`profile`）を渡すと、`delivery-` で始まる照合の種類の規則と照らします（#384）。ほかの種類の規則（図面の照合など）は飛ばし、知らない `delivery-` の種類と読めない規則は `profile.rule.unsupported` を返して照らしません（決定 0179）。照合は `zumen-delivery` の `check` で、規則の値は `zumen-profile` の `rules` で解きます。

どの種類も、`folders`（照らす名前の頭。書かなければすべて）・`except`（照らさない名前の頭）・`when`（この頭の名前のファイルがあるときだけ照らす）を書けます。

| 種類 | 欄 | 指摘 |
| --- | --- | --- |
| `delivery-name` | `characters`・`stem`・`extension`・`folder`・`depth`・`prefix` | 名前の文字・拡張子を除いた長さ・拡張子の長さ・フォルダーの名前の長さ・部分の数が外れるか、ファイルの名前が `prefix` で始まらない（`profile.delivery-name.invalid`） |
| `delivery-required` | `names` | 名前のファイルがない（`profile.delivery-required.invalid`） |
| `delivery-link` | `elements` | 管理ファイルが指すファイルがない（`profile.delivery-link.invalid`）。`elements` を書けば、その名前の要素だけを照らす |
| `delivery-unreferenced` | なし | どの管理ファイルも指さないファイル。管理ファイルそのものは除く（`profile.delivery-unreferenced.invalid`） |
| `delivery-standard` | `element`・`values` | 要素の値が、どの値とも違う（`profile.delivery-standard.invalid`）。要素がなければ確認できない（`profile.delivery-standard.unsupported`、`info`） |
| `delivery-characters` | `fullwidth-alphanumerics` | 値に、JIS X 0201 のラテン文字と JIS X 0208 の外の文字がある（`profile.delivery-characters.invalid`）。半角の片仮名、13 区の特殊文字、IBM の拡張は外。`fullwidth-alphanumerics = false` なら、全角の英数字も外 |
| `delivery-doctype` | `values` | DTD の名前が、どの値とも違うか、文書型宣言がない（`profile.delivery-doctype.invalid`） |
| `delivery-photo-date` | `entry`・`file`・`date` | 写真 1 枚の記録（`entry`）の下の、撮影の日の要素（`date`）の値が、ファイルの名前の要素（`file`）が指す写真の Exif の撮影の日（日時の始めの `YYYY-MM-DD`）と違う（`profile.delivery-photo-date.invalid`）。日の要素か Exif の撮影の日時がなければ確認できない（`profile.delivery-photo-date.unsupported`、`info`）。ファイルが見つからない記録は飛ばす（`delivery-link` が指摘する） |
| `delivery-photo-pixels` | `min`・`max` | 写真の画素数（フレームの見出しの幅 × 高さ）が範囲の外にある（`profile.delivery-photo-pixels.invalid`）。JPEG でないか画素数を読めなければ確認できない（`profile.delivery-photo-pixels.unsupported`、`info`） |

プロファイルは `profiles/mlit-delivery-const`（工事完成図書の電子納品等要領、令和 8 年 2 月）と `profiles/mlit-delivery-design`（土木設計業務等の電子納品要領、令和 8 年 2 月）です。どちらも、`PHOTO` をデジタル写真管理情報基準（令和 5 年 3 月）の規則で照らします（#388）。管理ファイルの名前と DTD、写真と参考図のファイルの名前、管理ファイルが指すファイル、適用要領基準、使う文字、撮影年月日と Exif、有効画素数の目安です。Exif との突き合わせと、画素数の範囲（100〜300 万画素、範囲の外は `info`）は、基準の目安から選んだ照合です。写真の編集の有無は、ここでは確かめません（電子小黒板の #389）。適用要領基準の要素の名前は、表の項目の名前を選んだ値です（DTD を読まないので、実物で確かめていません）。

## 上限

- 成果品のファイルの数を `Records`、管理ファイルの要素の数を `Elements` で数える。
