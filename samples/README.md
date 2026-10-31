# 公開サンプル

再配布を認めるライセンスで公開された図面を、取得したままの形で置きます（[決定 0171](../docs/decisions/0171-public-samples-in-git.md)）。CI は、これらの図面で適合試験を流します。試料の試験での利用は #310 で進めます。

**ここのファイルは、リポジトリのライセンス（MIT OR Apache-2.0）に従いません。** それぞれ、[`ledger.toml`](ledger.toml) に書いたライセンスに従います。配る crate とパッケージには入れません。

受け取った図面と顧客の図面は、公開されていても入れません。Git に入れられない図面は、今までどおりリポジトリの外のフォルダーを `ZUMEN_SAMPLES` に指定して使います。

## 入れてよいもの

目的を問わず、再配布・改変・商用の利用を、条件なしか表示だけを条件に認めるライセンスのものです。台帳の `license` には、次のどれかを書きます。

| `license` | ライセンス |
| --- | --- |
| `パブリックドメイン` | パブリックドメイン |
| `CC0-1.0` | CC0 1.0 |
| `CC-BY-2.0`・`CC-BY-2.1-JP`・`CC-BY-2.5`・`CC-BY-3.0`・`CC-BY-4.0` | CC BY |
| `公共データ利用規約（第 1.0 版）` | デジタル庁の公共データ利用規約（PDL1.0） |
| `政府標準利用規約（第 2.0 版）` | 各府省の政府標準利用規約 |
| `MIT`・`BSD-2-Clause`・`BSD-3-Clause`・`Apache-2.0`・`ISC`・`Zlib` | 寛容なライセンス |

次のものは入れません。

- ライセンスや利用条件が書かれていないもの。ダウンロードできることは、再配布の許諾ではありません
- 非営利に限るもの（NC）、改変を禁じるもの（ND）、同じ条件での公開を求めるもの（SA）
- 利用規約で再配布を禁じるもの。ソフトウェアの製品に付く試料で、使用許諾が再配布を認めないものも含みます
- 個人の氏名・住所・連絡先が読めるもの
- 1 つで 5 MB を超えるもの。`samples/` の全体は 100 MB までです

ライセンスがはっきりしないものは、入れずに利用者に尋ねます。

## 足し方

1. 取得したままのファイルを `samples/<形式>/` に置きます（例: `samples/dxf/`・`samples/jww/`・`samples/sxf/`）。Git は、このフォルダーのファイルの改行を変えません（`.gitattributes`）。
2. `ledger.toml` に、ファイルごとに `[[samples]]` を足します。

   ```toml
   [[samples]]
   path = "dxf/example.dxf"              # samples/ からの道筋
   sha256 = "…"                          # SHA-256 の 16 進の小文字
   bytes = 12345
   title = "図面の題"
   publisher = "公開した者"
   url = "https://example.com/example.dxf"  # 出所
   retrieved = "2026-10-30"              # 取得した日
   license = "CC-BY-4.0"
   license-url = "https://creativecommons.org/licenses/by/4.0/legalcode.ja"
   attribution = "表示の文"
   changes = "変えた内容"                 # 変えていなければ書かない
   ```

3. `cargo xtask codegen --check` で、ファイルと台帳の一対一・ハッシュとバイト数・ライセンス・大きさを確かめます。
4. ライセンスが本文や著作権表示の同梱を求める場合は、その条件を満たすように本文を `licenses/` に置き、台帳の表示の文から参照します。
5. PR に、ライセンスの本文の URL と、再配布を認める箇所を書きます。

## AutoCAD で作った図面

`dwg/zumen-*` は、zumen-jp が AutoCAD 2027 で作った図面です（#316）。ライセンスは CC0 1.0 です。主題ごとに 1 つの図面を作り、版ごとに DWG と、同じ図面の DXF（`_ascii.dxf`）を対で置きます。試験は、この対の表と図面モデルを比べます。

作り直すときは、`xtask/autocad/make.sh` を使います。SSH でつなぐ Windows の AutoCAD の accoreconsole で `xtask/autocad/samples.lsp` を流し、できた図面を `dwg/` に置きます。そのあと、台帳のハッシュとバイト数を直します。AutoCAD は保存のたびに、Windows のログイン名を図面の DWGPROPS に書きます。
