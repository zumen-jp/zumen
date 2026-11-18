# DXF のコード・ページの照合

R2007 より前の DXF の、Windows の 1 バイトのコード・ページ（`ANSI_874`・`ANSI_1250`〜`ANSI_1258`）の文字を、ezdxf 1.4.5 と Python の codec に照らします（[#327](https://github.com/zumen-jp/zumen/issues/327)）。`ANSI_1252` のほかは公開サンプルがないので、入力は ezdxf で自作します。

コード・ページごとに、次を確かめます。

- 表に割り当てのある 0x80〜0xFF のすべてのバイトを 1 つの TEXT に持ち、表の文字を名前に含むレイヤの R2000 の DXF を、zumen が ezdxf と同じレイヤ名と文字列で読む。
- 同じ文字を逆の順で持つ TEXT を公開 API で足すと、出力がそのバイト列を持ち、ezdxf が足した文字列として読む。
- 表せない文字（日）を足す変更は、全体を拒む。
- 表に割り当てのないバイトを 1 つ持つ TEXT は、Python の codec が拒み、zumen が `dxf.lift.invalid` で落とす（`ANSI_1256` は割り当てのないバイトがない）。

## 実行

同じチェックアウトで CLI をビルドしてから流します。出力先には、リポジトリ外の空のフォルダーを指定してください。

```sh
cargo build -p zumen-cli
uv run --no-python-downloads --no-build --python 3.14 --with ezdxf==1.4.5 python -I xtask/verify/codepage/check.py --cli target/debug/zumen --out ../zumen-codepage-verify
```

Windows では CLI に `.exe` を付けます。終了コードは、全件一致なら 0、不一致があれば 1、実行できなければ 2 です。入力と出力の DXF、コード・ページごとの不一致を `results.json` に残します。

## 確認した範囲

2026-11-18、Windows x64 のネイティブ CLI と Python 3.14・ezdxf 1.4.5 で、10 のコード・ページすべてが一致しました。通常の CI の成功条件には追加していません。wasm との一致は、適合テストの `code_page_1251.json` と `code_page_1252.json` で確かめます。AutoCAD で開いた確認と、DWG の番号の文字の確認はしていません。
