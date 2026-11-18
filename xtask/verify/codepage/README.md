# DXF のコード・ページの照合

R2007 より前の DXF の、Windows の 1 バイトのコード・ページ（`ANSI_874`・`ANSI_1250`〜`ANSI_1258` と、DOS の 12 の名前 `DOS437`〜`DOS869`）と 2 バイトのコード・ページ（`ANSI_936`・`ANSI_949`・`ANSI_950`・`ANSI_1361`）の文字を、Windows の API・ezdxf 1.4.5・Python の codec に照らします（[#327](https://github.com/zumen-jp/zumen/issues/327)）。AutoCAD 2027 が Windows のコード・ページとして読んだ別名（`ISO8859-1`・`ISO8859-2`・`ISO8859-7`・`ISO8859-9`・`GB2312`・`KSC5601`・`BIG5`・`JOHAB`）も、`$DWGCODEPAGE` をその名前にして同じに照らします。MTEXT の書式の符号 `\M+<番号><16 進 4 桁>` の 2 バイトの文字も照らします。`ANSI_1252` のほかは公開サンプルがないので、入力は ezdxf で自作します。

割り当ての基準は、Windows では MultiByteToWideChar（Python の `codecs.code_page_decode`）が私用領域と C1 制御文字でない 1 文字に戻すバイト列、ほかの OS では ezdxf が選ぶ Python の codec が戻すバイト列です。Windows では、Python の codec との違いが既知の数（`ANSI_1255` の 0xCA、`ANSI_936` の 0x80、`ANSI_950` の 249 の 2 バイト、`ANSI_1361` の 111 の 2 バイト）と同じことも確かめます。Python の `johab` は、字母を互換の字母（U+3131〜）に戻し、Windows は結合用の字母（U+1100〜）に戻します。`ANSI_1361` の 2 バイト目は 0x31 から試します。

コード・ページごとに、次を確かめます。

- 割り当てのあるすべてのバイト列を 200 文字ずつの TEXT に持ち、表の文字を名前に含むレイヤの R2000 の DXF を、zumen が基準と同じレイヤ名と文字列で読む。Python の codec で戻せる TEXT は、ezdxf の読みとも照らす。ezdxf 1.4.5 は `ANSI_` でない名前を `cp1252` で読むので、DOS と別名は ezdxf と照らさない。
- 同じ文字を逆の順で持つ TEXT を公開 API で足すと、出力が WideCharToMultiByte の書くバイト列を持ち、開き直した zumen と ezdxf が足した文字列として読む。
- 表せない文字（ภ。`ANSI_874` は日）を足す変更は、全体を拒む。
- 割り当てのないすべてのバイト列（1 バイトと、先頭のバイトの範囲の 2 バイト）を 1 つずつ持つ TEXT は、zumen がどれも読まず、すべて `dxf.lift.invalid` で落とす（`ANSI_1256` は割り当てのないバイトがない）。

MTEXT の `\M+` は、番号 1（`ANSI_932`）・2（`ANSI_950`）・3（`ANSI_949`）・4（`ANSI_1361`）・5（`ANSI_936`）ごとに、次を確かめます。基準は上と同じで、`ANSI_932` は私用領域（外字）も含みます。基準を ezdxf の `decode_mif_to_unicode` とも比べ、違いが既知の数（`ANSI_950` の 249、`ANSI_1361` の 111）と同じことを確かめます。

- 割り当てのあるすべての 2 バイトを 200 ずつ MTEXT に持つ R2000 の DXF の `scene` が、MTEXT ごとに 1 つの文字を持ち、その文字列が基準と同じ。
- 割り当てのない残りのすべての 2 バイト（0000〜FFFF のうち）を持つ MTEXT からは、文字を描かない。

## 実行

同じチェックアウトで CLI をビルドしてから流します。出力先には、リポジトリ外の空のフォルダーを指定してください。

```sh
cargo build -p zumen-cli
uv run --no-python-downloads --no-build --python 3.14 --with ezdxf==1.4.5 python -I xtask/verify/codepage/check.py --cli target/debug/zumen --out ../zumen-codepage-verify
```

Windows では CLI に `.exe` を付けます。終了コードは、全件一致なら 0、不一致があれば 1、実行できなければ 2 です。入力と出力の DXF、基準、コード・ページごとの不一致を `results.json` に残します。

## 確認した範囲

2026-11-18、Windows 11 x64 のネイティブ CLI と Python 3.14・ezdxf 1.4.5 で、Windows の API を基準に 37 の名前（`ANSI_` の 14、DOS の 12、別名の 8）と `\M+` の 5 つの番号のすべてが一致しました。通常の CI の成功条件には追加していません。wasm との一致は、適合テストの `code_page_1251.json`・`code_page_1252.json`・`code_page_dbcs.json`・`code_page_mif.json`・`code_page_dos.json`・`code_page_johab.json` で確かめます。AutoCAD 2027 で DOS と別名の名前を開いた結果は [DXF の文書](../../../docs/formats/dxf.md) に推定として書きました。zumen が書いた DXF を AutoCAD で開いた確認と、DWG の番号の文字の確認はしていません。
