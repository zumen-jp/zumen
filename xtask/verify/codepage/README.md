# DXF のコード・ページの照合

R2007 より前の DXF の、Windows の 1 バイトのコード・ページ（`ANSI_874`・`ANSI_1250`〜`ANSI_1258`）と 2 バイトのコード・ページ（`ANSI_936`・`ANSI_949`・`ANSI_950`）の文字を、Windows の API・ezdxf 1.4.5・Python の codec に照らします（[#327](https://github.com/zumen-jp/zumen/issues/327)）。`ANSI_1252` のほかは公開サンプルがないので、入力は ezdxf で自作します。

割り当ての基準は、Windows では MultiByteToWideChar（Python の `codecs.code_page_decode`）が私用領域と C1 制御文字でない 1 文字に戻すバイト列、ほかの OS では ezdxf が選ぶ Python の codec が戻すバイト列です。Windows では、Python の codec との違いが既知の数（`ANSI_1255` の 0xCA、`ANSI_936` の 0x80、`ANSI_950` の 249 の 2 バイト）と同じことも確かめます。

コード・ページごとに、次を確かめます。

- 割り当てのあるすべてのバイト列を 200 文字ずつの TEXT に持ち、表の文字を名前に含むレイヤの R2000 の DXF を、zumen が基準と同じレイヤ名と文字列で読む。Python の codec で戻せる TEXT は、ezdxf の読みとも照らす。
- 同じ文字を逆の順で持つ TEXT を公開 API で足すと、出力が WideCharToMultiByte の書くバイト列を持ち、開き直した zumen と ezdxf が足した文字列として読む。
- 表せない文字（ภ。`ANSI_874` は日）を足す変更は、全体を拒む。
- 割り当てのないすべてのバイト列（1 バイトと、先頭のバイトの範囲の 2 バイト）を 1 つずつ持つ TEXT は、zumen がどれも読まず、すべて `dxf.lift.invalid` で落とす（`ANSI_1256` は割り当てのないバイトがない）。

## 実行

同じチェックアウトで CLI をビルドしてから流します。出力先には、リポジトリ外の空のフォルダーを指定してください。

```sh
cargo build -p zumen-cli
uv run --no-python-downloads --no-build --python 3.14 --with ezdxf==1.4.5 python -I xtask/verify/codepage/check.py --cli target/debug/zumen --out ../zumen-codepage-verify
```

Windows では CLI に `.exe` を付けます。終了コードは、全件一致なら 0、不一致があれば 1、実行できなければ 2 です。入力と出力の DXF、基準、コード・ページごとの不一致を `results.json` に残します。

## 確認した範囲

2026-11-18、Windows 11 x64 のネイティブ CLI と Python 3.14・ezdxf 1.4.5 で、Windows の API を基準に 13 のコード・ページすべてが一致しました。通常の CI の成功条件には追加していません。wasm との一致は、適合テストの `code_page_1251.json`・`code_page_1252.json`・`code_page_dbcs.json` で確かめます。AutoCAD で開いた確認と、DWG の番号の文字の確認はしていません。
