# JPEG の外部照合

工事写真（JPEG）の Exif と XMP の読みを、[Pillow](https://github.com/python-pillow/Pillow) 13.0.0 と照らします（#505）。`crates/zumen-photo/tests/external.rs` が、コアと、別の Python プロセスで動く `read.py` の読みを比較します。外部ツールのソースとバイナリはリポジトリに置かず、Cargo の依存にも加えません。

## 実行

Python は、すでに入っているものを使います。毎晩の CI は Python 3.14 を使います。

Windows では Python Install Manager の Python 3.14 と uv で、リポジトリの `target/` 内に環境を作れます。uv に Python 本体をダウンロードさせません。

```powershell
uv venv --python 3.14 --no-python-downloads target/photo-venv
uv pip install --python target/photo-venv/Scripts/python.exe --only-binary :all: pillow==13.0.0
$env:ZUMEN_PHOTO_PYTHON = (Resolve-Path target/photo-venv/Scripts/python.exe).Path
cargo test --locked -p zumen-photo --test external -- --nocapture
```

ほかの環境では、既存の Python に付く venv も使えます。

```bash
python3 -m venv target/photo-venv
target/photo-venv/bin/python -m pip install --only-binary :all: pillow==13.0.0
ZUMEN_PHOTO_PYTHON="$PWD/target/photo-venv/bin/python" cargo test --locked -p zumen-photo --test external -- --nocapture
```

`ZUMEN_PHOTO_PYTHON` がなければ `python` を使います。Python か Pillow がないときは外部照合だけを飛ばし、理由をテストの出力に書きます。コアの読みと診断は、そのときも確かめます。`ZUMEN_PHOTO_PYTHON` または `ZUMEN_PHOTO_REQUIRED` を指定したときは、不在も失敗にします。バージョン違い・起動の失敗・入力を読めない場合・値の食い違いは、指定の有無によらず失敗です。毎晩の CI は `ZUMEN_PHOTO_REQUIRED=1` で流します。

## 入力と比較する値

| 入力 | 中身 |
| --- | --- |
| `samples/jpeg/commons-aichi-ceramic-museum-2018-005.jpg` | Wikimedia Commons の CC0 の写真。iPhone 6s の Exif・GPS（高さ・向きを持つ）と、GIMP 2.10 が書いた XMP |
| テストの中で作る 4 つの JPEG | II と MM のバイト順、北東と南西、海面より下、時差の有無、磁北、緯度だけの GPS、UTF-8 でない型の名前、配列・入れ子・前後の空白を持つ XMP、Exif のない XMP だけの写真 |

`read.py` は標準入力の JPEG を Pillow で開き、1 行に 1 つの値を TSV で返します。

- 画素数は Pillow の `size`、Exif は `getexif()` の 0 番目の IFD と、Exif の IFD・GPS の IFD のタグの値です。緯度・経度は度・分・秒の有理数から度にし、南と西を負にします。高さは、海面より下のフラグなら負にします。どれも Exif の定義どおりの換算で、片方だけの緯度・経度は位置にしません。
- Exif の文字の値は、Pillow が Latin-1 で復号したものをバイト列に戻し、UTF-8 で読みます。UTF-8 でなければ値を返さず、終わりの NUL と前後の空白を除きます。
- XMP は、Pillow が APP1 から取り出したパケットを Python の ElementTree（Expat）で読みます。`rdf:Description` の属性（`rdf` と `xmlns` の名前空間のものを除く）、文字だけを持つ子の要素（前後の空白を除く）、配列（`rdf:Alt`・`rdf:Seq`・`rdf:Bag`）の子の要素を持たない最初の `rdf:li` の値を、名前空間・名前・値の組にします。
- 浮動小数は Python の `repr` で送り、Rust で 1e-12 の相対の差の中で比べます。

## 検証の範囲

- XMP の性質の選び方は zumen の規則（[JPEG の文書](../../../docs/formats/jpeg.md#xmp)）で、Python の側にも同じ規則を書いています。外部の実装で確かめているのは、APP1 からのパケットの取り出しと、XML と名前空間の読みです。
- 向きが 1〜8 の外の値・日時の形が違う値・壊れたセグメントなど、コアが診断を返す入力の多くは、`tests/read.rs` で確かめ、Pillow とは照らしていません。
- 電子小黒板の写真、拡張の XMP、縮小画像の IFD、MakerNote、基準になるソフトで開いた結果とは、まだ照らしていません。

## ツールと公開サンプルの権利

Pillow 13.0.0 のライセンス（MIT-CMU）は、[TIFF の外部照合](../tiff/README.md#ツールの権利)で 2026-11-13 に確認したものと同じ版です。別に導入するテストのツールで、リポジトリには同梱しません。

公開サンプルは、2026-11-18 に Wikimedia Commons の説明のページで、撮った利用者が自分の作品として CC0 1.0 で公開していることと、Exif と XMP に個人の氏名・住所・連絡先がないことを確かめました。取得したファイルの SHA-1 は、Commons の 2021-12-05 の版の値と同じです。出所と取得の日は `samples/ledger.toml` にあります。
