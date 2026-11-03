# 外部の実装との照合と確認の束

zumen が書き戻した（apply）・変換した（convert）図面を、zumen でない実装で読んで照らし、AutoCAD・Jw_cad・SXF ブラウザで開いて確かめるファイルの束を作ります（決定 0254（PR #500）、#503）。自前の往復の試験は、書く側と同じ読みで確かめるので、仕様の読み違いを見つけません（#345）。

| ファイル | 中身 |
| --- | --- |
| `make.py` | zumen の CLI（`zumen serve --stdio`）で、公開サンプル（`samples/`）と適合試験の中で作った図面を開き、書き戻しと変換を当てて、束のフォルダーに書く。zumen が読んだ値・入力の出所とライセンス・当てた変更・期待する形を `manifest.json` に、issue ごとの手順と期待する結果を `README.md` に、ライセンスの本文を `licenses/` に書く |
| `compare.py` | 照らす台本が共有する比べ方。形の値の比べ方、出所のハンドルで結んだ図形の照らし方、結果の書き方 |
| `check_ezdxf.py` | 束の DXF を ezdxf で読み、zumen の読みと照らす |
| `acadsharp/` | 束の DWG を ACadSharp で読み、値を束の `acadsharp-dump.json` に書き出す .NET の道具 |
| `check_acadsharp.py` | `acadsharp-dump.json` の値を、zumen の読みと照らす |
| `check_ezjww.py` | 束の JWW を ezjww で読み、zumen の読みと照らす |
| `check_ezsxf.py` | 束の SFC・P21 を ezsxf で読み、zumen の読みと照らす |
| `dump.lsp` | AutoCAD で開いた図面の図形を、ハンドルごとに 1 行で書き出す。束の README の手順で使う |

## 使い方

```bash
cargo build --release -p zumen-cli
python -m pip install ezdxf==1.4.5 ezjww==0.7.6 ezsxf==0.3.7
python -I xtask/verify/make.py target/release/zumen ../zumen-verify
python -I xtask/verify/check_ezdxf.py ../zumen-verify
python -I xtask/verify/check_ezjww.py ../zumen-verify
python -I xtask/verify/check_ezsxf.py ../zumen-verify
dotnet run --project xtask/verify/acadsharp -c Release -- ../zumen-verify
python -I xtask/verify/check_acadsharp.py ../zumen-verify
```

ACadSharp の道具は、.NET 10 の SDK で動かします。ACadSharp の版は、`acadsharp/packages.lock.json` で固定します。

束のフォルダーは、リポジトリの外に作ります。`make.py` は、前の束（`manifest.json` がある）か空のフォルダーだけを消して作り直し、ほかのフォルダーは消さずに止まります。

毎晩の CI（`.github/workflows/nightly.yml` の `external`）が同じことを流し、束を成果物の `verify-bundle` に置きます。確認の束の手順と期待する結果は、束の `README.md` にあります。

受け取った図面で確かめるときは、`make.py` に `--inputs <フォルダー>` を足します。束をリポジトリの中に作ろうとすると止まります。その束は、Git にも CI の成果物にも入れず、issue には件数と種類だけを書き、ファイルの名前と中身を書きません（決定 0254）。

## 照らすこと

`make.py` は、書いたファイルを zumen で読み直し、入力の読みにない診断の code が出たら、そのファイルを `manifest.json` の `reread` に残して失敗にします。

照らす台本は、どれもファイルごとに次を確かめます。

- 外部の実装が読める。書き戻したファイルで、外部の実装の監査・警告・通知が、入力より増えない。
- zumen の要素の出所（ハンドル・レコード・インスタンス）の図形が外部の実装にあり、種類と画層が同じ。形の値（端点・中心・半径・角度・位置・文字・尺度など）も同じ。数は、相対と絶対の誤差 1e-9 まで同じとみなす。角度は、360° の違いを同じとみなす。
- 図形は、どれも zumen の要素になるか、zumen が診断を返している（黙って捨てない）。診断の場所は、数を限らずに求める。
- 書き戻したファイルで、変更で決めた形と画層（`manifest.json` の `expect`）が、外部の実装で読んだ値と同じ。期待する値は、zumen の読みでなく、変更そのものと変換の行列から決める。当てる候補のない段は、`skipped` として束に残す。

形式ごとに、次を足します。

- DXF（`check_ezdxf.py`）: 読む中で流れる ezdxf の監査（`recover.readfile`）で、書き戻したファイルの誤りと直した誤りが、code ごとの件数で入力より増えない。変換したファイルには、誤りも直した誤りもない。画層の名前が、ファイルの画層の表と同じ（ezdxf が読むときに足す Defpoints は除く）。比べられる種類（LINE・CIRCLE・ARC・ELLIPSE・LWPOLYLINE・POINT・TEXT・MTEXT・INSERT）のモデル空間の要素は、形を持つ。要素にならない図形は、診断がその図形のレコードを指す。足した画層の色、色を変えた図形の色、点の表し方も比べる。
- DWG（`check_acadsharp.py`）: 画層の名前、比べる形と色は DXF と同じ。ACadSharp は図形のバイトの位置を返さないので、要素にならない図形があれば、zumen が診断を返していることだけを確かめる。通知は文で比べる。
- JWW（`check_ezjww.py`）: zumen のレコードの番号は、見出しを 0 とし、図形の一覧の i 番目（0 から）を i + 1 とする（決定 0133）。要素にならない図形は、診断がその図形のレコードを指す。ezjww の座標は用紙の mm なので、レイヤグループの縮尺の分母を掛けて比べる。形は、線と点を比べる。
- SFC・P21（`check_ezsxf.py`）: SFC は、フィーチャの種類・レイヤ・用紙の中にあることと、線・円・点マーカーの形を比べる。ezsxf がフィーチャを型で読まない SFC（印が `/*SXF3` など）は、要素を照らさない。P21 は、参照をたどって、点の位置・回転・倍率と、線の端点を比べる。レイヤはたどらない。ezsxf の警告は、code の種類で比べる。

`make.py` は、照らすための入力を 2 つ作ります。ezsxf がフィーチャを型で読む SFC（`input/made-features.sfc`）と、zumen が要素にしない図形を 20 持つ DXF（`input/made-dropped.dxf`）です。後者で、診断の場所が既定の数（16）を超えても、黙って捨てない照合が働くことを確かめます。

既知の食い違いは、台本ごとの `KNOWN` に、issue と当たるファイルの数と一緒に置き、失敗にしません。当たるファイルの数が変わったら（直ったか、同じ形の食い違いが増えた）失敗にして、`KNOWN` を直すことを知らせます。

## 外部の実装

外部の実装は、試験で別のプログラムとして動かし、成果物にも依存にも入れません。版を決めて入れます。

| 形式 | 実装 | ライセンス | 扱い |
| --- | --- | --- | --- |
| DXF | [ezdxf](https://github.com/mozman/ezdxf) 1.4.5 | MIT | 照らす（`check_ezdxf.py`） |
| DWG | [ACadSharp](https://github.com/DomCR/ACadSharp) 3.8.0 | MIT | 照らす（`acadsharp/`・`check_acadsharp.py`）。.NET の道具で読み、値を JSON に書き出す |
| JWW | [ezjww](https://github.com/monozukuri-ai/ezjww) 0.7.6 | MIT | 照らす（`check_ezjww.py`） |
| SFC・P21 | [ezsxf](https://github.com/monozukuri-ai/ezsxf) 0.3.7 | MIT | 照らす（`check_ezsxf.py`） |

次のものは使いません。

- LibreDWG などの GPL の実装。ソースも読みません（`AGENTS.md`）。
- ODA File Converter。使うには利用の条件への同意が要るため（決定 0254 の人を待つもの）。
