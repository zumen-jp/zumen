# 電子納品の成果品

電子納品の成果品（フォルダー・管理ファイル・図面・写真の集まり）を読む規則を記録します。実装は `crates/zumen-delivery` です。API では、`delivery_check` の op に、成果品の起点（`root`）か ZIP の入力（`archive`）を渡し、管理ファイルの要約を返します。要領の本文・DTD・作成例は、転載と改変が禁じられているので写さず、同梱しません（`formats/ledger.toml` の `delivery`）。

文字のチェックが参照する JIS X 0208・JIS X 0201 と、本文の入手条件の確認状況は、[JIS の記録](jis.md)にあります。

## 対象

- 工事完成図書・土木設計業務などの電子納品要領の成果品。要領の版では読み方を変えない。
- 成果品の中の図面は、この読み取りでは開かない。JPEG（SOI で始まるファイル）は、写真の照合のために、`zumen-photo` で Exif の撮影の日時と画素数だけを読む（#388）。その読み取りの診断は足さない。

## 入力

- `root`: `zm_push` で渡した入力のうち、名前が `root`（`/` で終わる）で始まるもの。起点からの相対の名前を、成果品の中の名前にする。
- `archive`: ZIP の入力。ディレクトリの項目を除き、項目の名前を成果品の中の名前にする。展開した量は予算（`decoded-bytes`）で数える。名前は、UTF-8 のフラグがあれば UTF-8、なければ Shift_JIS として読む（`zumen-zip`）。
- 空の部分・`.`・`..`・`\` を含む名前と、`/` で始まる名前は、`delivery.name.unsafe` を返して除く。同じ名前は 1 つにする。
- 名前の大文字と小文字は、元のまま持つ。

## 管理ファイル

- 起点か、その直下のフォルダーにある拡張子 `.XML`（大文字と小文字を問わない）のファイルを、管理ファイルとする（`INDEX_C.XML`・`DRAWING/DRAWING.XML`・`PHOTO/PHOTO.XML` など）。
- 宣言の文字コード（UTF-8・Shift_JIS）で読む（`zumen-xml`）。XML として読めなければ、`zumen-xml` の診断に `delivery.file.invalid` を足し、管理ファイルの記録に移さない。
- DTD は処理しない。文書型宣言のシステム識別子（`INDE_C05.DTD` など）を、DTD の名前として持つ。管理ファイルはどれも文書型宣言を持つので、`xml.dtd.dropped` は返さない。DTD で宣言する実体の参照は、`xml.entity.unsupported` を返す。
- 要素ごとに、元の名前・文字の値（前後の空白を除く）・子の要素の記録を持つ。子の要素だけを持つ要素は、値を持たない。属性は、まだ持たない。

## 管理 XML の書き戻し

`delivery_apply` の操作と、Rust の `zumen_delivery::write_xml` で、既存の管理ファイルの文字を更新し、記録を追加します。XML を `zumen-xml` で読み、元の要素を名前と同名兄弟内の位置で選び、`zumen_xml::write` で対象のバイト範囲だけを変更します。変更後の XML は同じ予算で再読してから返します。[要求の形と例](../../schema/README.md#管理-xml-の変更)を参照してください。

- UTF-8・Shift_JIS・UTF-16LE・UTF-16BE の文字コード、BOM、XML 宣言、DTD 宣言、既存の属性、注記と処理命令、未変更の範囲を保ちます。
- 文字の置き換えは、子要素を持たない項目に限ります。文字と CDATA をまとめて新しい値にし、注記と処理命令は元の順に保ちます。空要素は元の名前と属性を保って開始・終了タグに展開します。
- 追加は親の末尾に行います。親の終了タグが独立した行にあれば、既存の子の字下げ（なければ親に空白 2 個を追加）と、文書で最初に現れる改行を使います。それ以外は改行を足しません。新しい子の木の内側は字下げしません。この書式の選び方は、本実装の方針です。
- 値の `&`・`<`・`>` を文字参照にし、CR は `&#13;` にして再読した値を保ちます。LF は元の改行にします。Shift_JIS で表せない文字と、復号すると別の文字になる値は置換せず拒否します。
- `path` は元の文書で解き、名前は接頭辞を含めて区別します。複数の追加を同じ親へ指定したときは要求の順にします。追加した木は親の名前空間を引き継ぎます。

存在しない経路は `delivery.edit-path.missing`、古いハッシュは `delivery.edit-hash.stale` で拒否します。書き手は更新の重複・範囲の重なりを `xml.edit-range.invalid`、子要素のある項目の更新を `xml.edit-children.unsupported`、不正な名前・文字を `xml.edit-name.invalid`・`xml.edit-character.invalid`、未宣言の接頭辞を `xml.edit-namespace.missing`、文字コードで保てない値を `xml.edit-encoding.failed` で返します。診断には元の親・項目の場所を付けます。書き出す配列を確保できない場合は `xml.write.failed` です。

経路の探索・符号化・複製・並べ替えと再読を `Work`・`DecodedBytes`、新しい要素と再読を `Records`、親を含めた入れ子を `Depth` で制限します。読み取りでは `InputBytes` も数えます。中断や停止の後は、この操作の深さを共有予算に残しません。元の入力・DTD の取得状態・ファイルシステムには変更を加えません。

属性・宣言の変更、要素の削除・移動、表のファイルへの書き出し、ZIP 全体の再梱包は対象外です。DTD の内容は取得・検証しません。成果品のファイル参照や要領との照合は、blob を取り出して `close.inputs` で元の登録を捨て、書き出した XML を `zm_push` で入力に戻し、`delivery_check` でプロファイルを指定して行います。

## ファイルの名前の解決

- 値がファイルの名前の形（空白を含まず、英字で始まる 1〜4 文字の英数字の拡張子を持つ）なら、管理ファイルのフォルダーにつないだ名前に解き、成果品の中にあるかを持つ。`4.0` のような数は、ファイルの名前にしない。
- そこになく、値がフォルダーを含まなければ、管理ファイルのフォルダーの下のフォルダー（`ORG`・`ORG001` など）で同じ名前のファイルを探し、1 つだけならそれに解く。
- `delivery_check` は、管理ファイルごとに、ファイルの名前の形の値の数と、成果品の中に見つからない名前を返す。

## 照合

プロファイル（`profiles`・`profile`）を渡すと、`delivery-` で始まる照合の種類の規則と照らします（#384）。ほかの種類の規則（図面の照合など）は飛ばし、知らない `delivery-` の種類と読めない規則は `profile.rule.unsupported` を返して照らしません。照合は `zumen-delivery` の `check` で、規則の値は `zumen-profile` の `rules` で解きます。

どの種類も、`folders`（照らす名前の頭。書かなければすべて）・`except`（照らさない名前の頭）・`when`（この頭の名前のファイルがあるときだけ照らす）を書けます。

| 種類 | フィールド | 指摘 |
| --- | --- | --- |
| `delivery-name` | `characters`・`stem`・`extension`・`folder`・`depth`・`prefix` | 名前の文字・拡張子を除いた長さ・拡張子の長さ・フォルダーの名前の長さ・部分の数が外れるか、ファイルの名前が `prefix` で始まらない（`profile.delivery-name.invalid`） |
| `delivery-required` | `names` | 名前のファイルがない（`profile.delivery-required.invalid`） |
| `delivery-link` | `elements` | 管理ファイルが指すファイルがない（`profile.delivery-link.invalid`）。`elements` を書けば、その名前の要素だけを照らす |
| `delivery-unreferenced` | なし | どの管理ファイルも指さないファイル。管理ファイルそのものは除く（`profile.delivery-unreferenced.invalid`） |
| `delivery-standard` | `element`・`values` | 要素の値が、どの値とも違う（`profile.delivery-standard.invalid`）。要素がなければ確認できない（`profile.delivery-standard.unsupported`、`info`） |
| `delivery-characters` | `fullwidth-alphanumerics` | 値に、JIS X 0201 のラテン文字と JIS X 0208 の外の文字がある（`profile.delivery-characters.invalid`）。半角の片仮名、13 区の特殊文字、IBM の拡張は外。`fullwidth-alphanumerics = false` なら、全角の英数字も外 |
| `delivery-doctype` | `values` | DTD の名前が、どの値とも違うか、文書型宣言がない（`profile.delivery-doctype.invalid`） |
| `delivery-photo-date` | `entry`・`file`・`date` | 写真 1 枚の記録（`entry`）の下の、撮影の日の要素（`date`）の値が、ファイルの名前の要素（`file`）が指す写真の Exif の撮影の日（日時の始めの `YYYY-MM-DD`）と違う（`profile.delivery-photo-date.invalid`）。日の要素か Exif の撮影の日時がなければ確認できない（`profile.delivery-photo-date.unsupported`、`info`）。ファイルが見つからない記録は飛ばす（`delivery-link` が指摘する） |
| `delivery-photo-pixels` | `min`・`max` | 写真の画素数（フレームのヘッダーの幅 × 高さ）が範囲の外にある（`profile.delivery-photo-pixels.invalid`）。JPEG でないか画素数を読めなければ確認できない（`profile.delivery-photo-pixels.unsupported`、`info`） |

プロファイルは `profiles/mlit-delivery-const`（工事完成図書の電子納品等要領、令和 8 年 2 月）と `profiles/mlit-delivery-design`（土木設計業務等の電子納品要領、令和 8 年 2 月）です。どちらも、`PHOTO` をデジタル写真管理情報基準（令和 5 年 3 月）の規則で照らします（#388）。管理ファイルの名前と DTD、写真と参考図のファイルの名前、管理ファイルが指すファイル、適用要領基準、使う文字、撮影年月日と Exif、有効画素数の目安です。Exif との突き合わせと、画素数の範囲（100〜300 万画素、範囲の外は `info`）は、基準の目安から選んだ照合です。写真の編集の有無は、ここでは確かめません（電子小黒板の #389）。適用要領基準の要素の名前は、表の項目の名前を選んだ値です（DTD を読まないので、実物で確かめていません）。

## 上限

- 成果品のファイルの数を `Records`、管理ファイルの要素の数を `Elements` で数える。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `delivery` | 読み | `external` |
| `delivery` | 書き | `external` |

段の意味は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、根拠は[形式と権利の一覧](README.md#format-delivery)にあります。

読みと書きの `external` の根拠は、国土交通省が DTD とともに配る管理ファイルの作成例 16 件です。工事・業務・地質の要領の 13 件と、写真管理ファイル（デジタル写真管理情報基準の `PHOTO05.DTD`）・図面管理ファイル（CAD 製図基準の `DRAW04.DTD`、完成図と発注図）の 3 件です。作成例を [lxml 6.1.3](../../xtask/verify/dtd/README.md) に要領の DTD で検証させ、lxml が読んだ文書型宣言と要素の木が、コアの読みとすべて合うことを確かめます。書き戻しは 2 通りです。DTD が文字を許す項目の値をすべて置き換えて書くものと、DTD が親の中で単独の繰り返しを許す最後の子の記録を写して親の末尾に足すもの（`Edit::Append`）です。どちらも同じ DTD の検証に通ることと、lxml の読んだ木が書いた木と合うことを確かめます。作成例は転載が禁じられているので同梱せず、利用者が入手したものを手元で読みます。CI では流しません。

- libxml2 は Shift_JIS を JIS の対応で、コアは [XML の規則](xml.md#shift_jis-の名前)のとおり WHATWG の対応で復号するので、`～` などの 8 字は別の文字になる。照合では WHATWG の側にそろえる。
- コアは DTD を処理しない。DTD が子の要素だけを許す項目（作成例の `MEET.XML` の空の `その他` など）にも文字を書けるが、その XML は検証に通らない。記録を足す位置と数も DTD で確かめないので、繰り返せない記録を足すと検証に通らない XML になる。照合で足したのは、DTD が繰り返しを許す記録だけ。
- 写真管理ファイルと図面管理ファイルの作成例は、令和 4 年 3 月の版の工事の束にあるもの（デジタル写真管理情報基準は令和 2 年 3 月、CAD 製図基準は平成 29 年 3 月の版）。作成例のない管理ファイル（電気通信設備・機械設備の要領の管理ファイルなど）は照らしていない。

自作の UTF-16LE・UTF-16BE の管理ファイルで、DTD の名前と適用する要領の判定を検査します。[XML の規則](xml.md)を使います。書き戻しは、自作の管理 XML で符号化と BOM の 7 通り、同名の項目、注記・CDATA・名前空間、参照とプロファイルの再照合、予算境界と中断を検査します。ネイティブと wasm の一致は `delivery_apply.json`、XML 自体のバイト列・名前・値・参照は [Python と Expat の独立検査](../../xtask/verify/README.md#管理-xml-の書き戻しを照らす)で別々に確認します。これらの確認は `synthetic` です。

受け取った成果品・電子納品チェックシステムとは、まだ照らしていません。再配布できる公開サンプルが見つかっていない状況は台帳に記録しています。実際のファイルでは、ここに書いた扱いと違うことがあります。XML の fuzz target は更新と追加・再読も通り、公開 API の要求は適合入力から fuzz の種へ渡します。

- 公開サンプルとの照合は #505 で行う。
