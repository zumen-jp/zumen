# 電子納品の DTD による照合

電子納品の管理ファイルとボーリング交換用データを、国土交通省が要領とともに配る DTD で [lxml](https://lxml.de/) 6.1.3 に検証させ、lxml が読んだ要素の木とコアの読みを照らします（#505）。管理ファイルの書き戻しは、値を置き換えた XML と記録を足した XML を、同じ DTD で検証させます。外部ツールのソースとバイナリはリポジトリに置かず、Cargo の依存にも加えません。

## 作成例と DTD を用意する

国土交通省の電子納品の頁は、要領・DTD・作成例の転載と改変を禁じ、HTML 以外のファイルへ直接リンクしないよう求めています（[リンク・著作権](https://www.cals-ed.go.jp/copyright/)）。そのため、作成例と DTD はリポジトリに入れず、テストと CI も取得しません。照合ツールは毎晩の CI で流すのが原則ですが（[依存と照合ツール](../../../CONTRIBUTING.md#依存と照合ツール)）、入力を用意できないので、この照合は手元で流します。

[電子納品に関する要領・基準の DTD・XML 出力例の頁](https://www.cals-ed.go.jp/cri_dtdxml/)から、令和 8 年 2 月の版の工事・業務・地質の 3 つの ZIP と、令和 4 年 3 月の版の工事の ZIP（`const10.zip`）を入手し、1 つのフォルダーの下に展開します。`const10.zip` からは、写真管理ファイルと図面管理ファイルの作成例と DTD を使います。同じ ZIP の工事の管理ファイルは DTD だけで作成例がないので、展開しても照合に影響しません。展開したフォルダーの名前と並びは問いません。テストはフォルダーの下を探し、次のファイルを、同じフォルダーにある DTD で検証させます。

| 要領 | ファイル | DTD |
| --- | --- | --- |
| 工事 | `INDEX_C.XML`・`REGISTER.XML`・`MEET.XML`・`PLAN.XML`・`OTHRS.XML` | `INDE_C09.DTD`・`REGIST06.DTD`・`MEET05.DTD`・`PLAN05.DTD`・`OTHRS05.DTD` |
| 業務 | `INDEX_D.XML`・`REGISTER.XML`・`REPORT.XML`・`OPENREP.XML` | `INDE_D08.DTD`・`REGIST07.DTD`・`REP05.DTD`・`OPREP01.DTD` |
| 地質 | `BORING.XML`・`COREPIC.XML`・`GRNDTST.XML`・`OTHRFLS.XML` | `BRG0200.DTD`・`CPIC0200.DTD`・`GTST0300.DTD`・`OTHR0110.DTD` |
| 地質 | `BED*.XML`（ボーリング交換用データ） | `BED0500.DTD` |
| デジタル写真管理情報基準（令和 2 年 3 月） | `PHOTO.XML` | `PHOTO05.DTD` |
| CAD 製図基準（平成 29 年 3 月） | `DRAWINGF.XML`（完成図）・`DRAWINGS.XML`（発注図） | `DRAW04.DTD` |

作成例は使い終えたら消してかまいません。テストは作成例を読むだけで、写しを作りません。

## 実行

Python は、すでに入っているものを使います。Windows では Python Install Manager の Python 3.14 と uv で、リポジトリの `target/` 内に環境を作れます。uv に Python 本体をダウンロードさせません。

```powershell
uv venv --python 3.14 --no-python-downloads target/dtd-venv
uv pip install --python target/dtd-venv/Scripts/python.exe --only-binary :all: lxml==6.1.3
$env:ZUMEN_DTD_PYTHON = (Resolve-Path target/dtd-venv/Scripts/python.exe).Path
$env:ZUMEN_CALS_DTD = 'D:\path\to\cals-dtd'
cargo test --locked -p zumen-delivery -p zumen-geology --test dtd -- --nocapture
```

ほかの環境では、既存の Python に付く venv も使えます。

```bash
python3 -m venv target/dtd-venv
target/dtd-venv/bin/python -m pip install --only-binary :all: lxml==6.1.3
ZUMEN_DTD_PYTHON="$PWD/target/dtd-venv/bin/python" ZUMEN_CALS_DTD=/path/to/cals-dtd \
  cargo test --locked -p zumen-delivery -p zumen-geology --test dtd -- --nocapture
```

`ZUMEN_CALS_DTD` がなければ照合を飛ばし、理由をテストの出力に書きます。`ZUMEN_DTD_REQUIRED` を指定したときは、不在も失敗にします。`ZUMEN_CALS_DTD` を指定したときは、lxml がない・バージョンが違う・作成例が足りない・検証に失敗する・値が食い違う場合は、どれも失敗です。`ZUMEN_DTD_PYTHON` がなければ `python` を使います。

## 比較する値

`check.py` は標準入力の XML を、引数のフォルダーから DTD を読んで lxml に検証させます（`dtd_validation`、ネットワークの取得なし、実体を展開しない）。検証に通れば、文書型宣言の根の名前とシステム識別子、要素ごとの深さ・名前・DTD の内容の種類・親の内容モデルで単独で繰り返せるか・値を TSV で返します。値は `zumen_delivery::Record` と同じく、直下の文字をつなげて前後の空白を除いたものです。

- 管理ファイル（`crates/zumen-delivery/tests/dtd.rs`）: 16 件の作成例を `zumen_delivery::read` で読み、文書型宣言のシステム識別子、要素の順・深さ・名前・値がすべて lxml と一致することを確かめます。
- 書き戻し: 作成例ごとに、子の要素を持たず、DTD が文字を許す（`#PCDATA`）項目の値を、文字参照にする記号（`<`・`&`・`>`・`"`・`'`）と日本語を含む値にすべて置き換えます（`write_xml`）。書いた XML が同じ DTD の検証に通り、lxml が読んだ値が置き換えた値と一致し、コアの再読とも一致することを確かめます。
- 記録の追加: 作成例ごとに、親の最後の子の要素で、DTD が親の中で単独の繰り返しを許すもの（要素に `*`・`+` が付くか、`*`・`+` の付いた選択の中にある）を、すべて写して親の末尾に足します（`Edit::Append`）。写した木の、DTD が文字を許す子のない項目は、新しい値にします。書いた XML が同じ DTD の検証に通り、lxml が読んだ木が、足した後に期待する木とコアの再読の両方と一致することを確かめます。
- ボーリング交換用データ（`crates/zumen-geology/tests/dtd.rs`）: `zumen_geology::read` の適用規格・公開フラグ・調査基本情報・位置（度・分・秒から度に換算）・測地系・住所・期間・孔口標高・総削孔長・角度・方位と、地層・色調・観察記事・標準貫入試験・孔内水位の各記録を、lxml が読んだ要素の値と照らします。値の誤りの診断（`boring.value.invalid`）がないことも確かめます。

libxml2 は Shift_JIS を JIS X 0201・JIS X 0208 の対応で復号し、zumen は [WHATWG Encoding](https://encoding.spec.whatwg.org/#shift_jis-decoder) の対応（Windows の符号）で復号します。作成例の `～`（0x8160）と、観察記事の改行の印に使う `\`（0x5C）は、この違いで別の文字になります。テストは対応の違う 8 字（0x5C・0x7E・0x8160・0x8161・0x817C・0x8191・0x8192・0x81CA）を WHATWG の側にそろえて比べます。

## 検証の範囲

- 照らしたのは国土交通省の作成例だけです。受け取った成果品・公開サンプル・電子納品チェックシステムとは、まだ照らしていません。
- 記録の追加で足したのは、DTD が繰り返しを許す記録を親の末尾に足すものだけです。コアは DTD を処理しないので、繰り返せない記録を足したり、DTD が子の要素だけを許す項目に文字を書いたりすると、検証に通らない XML になります（作成例の `MEET.XML` の空の `その他` がこれに当たります）。
- ボーリング交換用データで照らすのは、コアが読む項目だけです。コアが読まない要素は `boring.element.unsupported` で知らせます。
- 電気通信設備・機械設備の要領の管理ファイルなど、照らした ZIP にない管理ファイルと、データシート交換用データ・土質試験結果一覧表データは照らしていません。写真管理ファイルと図面管理ファイルの作成例は令和 4 年 3 月の版の束のものです。

## ツールと資料の権利

lxml 6.1.3 のライセンスは BSD-3-Clause です。wheel は libxml2 2.11.9・libxslt（MIT）、zlib、iconv（LGPL 2.1）を同梱します（2026-11-18 に wheel の `LICENSES.txt` で確認）。Cargo の外で動き、成果物に何も残さない、別に導入するテストのツールで（[依存と照合ツール](../../../CONTRIBUTING.md#依存と照合ツール)）、リポジトリには同梱しません。ソースも読みません。

作成例と DTD は、国土交通省の電子納品の頁から誰でも無償で入手できる公開の資料です。転載と改変が禁じられているので、リポジトリ・CI・issue に写しを置かず、値も書きません。
