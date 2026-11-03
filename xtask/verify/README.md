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
python -m pip install --only-binary :all: ezdxf==1.4.5 ezjww==0.7.6 ezsxf==0.3.7
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

`make.py` は、書いたファイルを zumen で読み直し、診断の件数が code と kind ごとに入力の読みより増えたら、そのファイルを `manifest.json` の `reread` に残して失敗にします。

照らす台本は、どれもファイルごとに次を確かめます。

- 外部の実装が読める。書き戻したファイルで、外部の実装の監査・警告・通知が、入力より増えない。
- zumen の要素の出所（ハンドル・レコード・インスタンス）の図形が外部の実装にあり、種類と画層が同じ。外部の実装の読みに画層があれば、zumen の要素も画層を持つ。形の値（端点・中心・半径・角度・位置・文字・尺度など）も同じ。形の欄は両向きに比べ、どちらかの読みにだけある欄も食い違いにする。形を比べる種類の図形は、zumen の要素も形を持つ。数は、相対と絶対の誤差 1e-9 まで同じとみなす。角度は、360° の違いを同じとみなす。形と図形の種類の対応がなければ、表に足すよう失敗にする。
- 図形（DXF・DWG はモデル空間の図形、JWW は図形の一覧の図形、SFC は置く所のフィーチャ、P21 は用紙の並びの要素）は、どれも zumen の要素になるか、図形を移さなかったことを言う zumen の診断（`compare.py` の `LIFTED_AWAY`。`*.lift.*`・`dxf.non-planar.dropped` など）がある（黙って捨てない）。図形の一部だけを落としたことを言う診断（`PARTLY`）と、ほかの診断は数えない。`catalog/codes.toml` で落としたことを言う code（fidelity が dropped）のうち、どちらにも分けていないものが出たら、分けるよう失敗にする。診断の場所は、数を限らずに求める。
- 書き戻したファイルで、変更で決めた形と画層（`manifest.json` の `expect`）が、外部の実装で読んだ値と同じ。期待する値は、zumen の読みでなく、変更そのものと変換の行列から決める。期待する形は、変更に書いた欄だけを比べる。当てる候補のない段は、`skipped` として束に残す。
- 1 つのファイルで台本が止まっても、そのファイルの食い違いにして、ほかのファイルを照らす。

形式ごとに、次を足します。

- DXF（`check_ezdxf.py`）: 読む中で流れる ezdxf の監査（`recover.readfile`）で、書き戻したファイルの誤りと直した誤りが、code ごとの件数で入力より増えない。変換したファイルには、誤りも直した誤りもない。画層の名前が、ファイルの画層の表と同じ（ezdxf が読むときに足す Defpoints は除く）。比べられる種類（LINE・CIRCLE・ARC・ELLIPSE・LWPOLYLINE・POINT・TEXT・MTEXT・INSERT）のモデル空間の要素は、形を持つ。要素にならない図形は、移さなかったことを言う診断が、その図形の種類とレコードを指す（kind のない code は、レコードだけで結ぶ）。足した画層の色、色を変えた図形の色、点の表し方（$PDMODE・$PDSIZE）も比べる。
- DWG（`check_acadsharp.py`）: 画層の名前、比べる形と色は DXF と同じ。ACadSharp は図形のバイトの位置を返さないので、要素にならない図形の数が、種類ごとに、移さなかったことを言う診断（kind は図形の種類。kind のない code は、その code が言いうる種類（`compare.py` の `KINDLESS`）に足す）の件数を超えないことを確かめる。そのため、ブロックの定義の中など、ほかの所の同じ種類の図形を言う診断と区別しない。束の DWG には zumen が移さない図形がないので、この照合は受け取った図面（`--inputs`）でだけ働く（zumen は要素にしない図形を DWG に足せず、照らすための DWG を作れない）。通知は文で比べる。
- JWW（`check_ezjww.py`）: zumen のレコードの番号は、見出しを 0 とし、図形の一覧の i 番目（0 から）を i + 1 とする（決定 0133）。要素にならない図形は、移さなかったことを言う診断が、その図形のレコードを指す（kind はクラスの名前なので比べない）。ezjww の座標は用紙の mm なので、レイヤグループの縮尺の分母を掛けて比べる。形は、線と点を比べ、線と点の要素は形を持つ。足した図形のペンの番号も比べる。
- SFC・P21（`check_ezsxf.py`）: zumen のレコードの番号は、`docs/formats/sxf.md` の「判定」と「レコードの区切り」で数え（AP202 と書いた P21 ではフィーチャの印を読まず、zumen が読めないと言った範囲は数えない）、要素の出所の番号と同じことも確かめる。要素の種類と形が、出所のフィーチャ（SFC）か実体（P21）に合う。P21 の曲線の要素は、出所の曲線の実体（トリムした曲線は基の曲線）で決まる種類と形を持つ。
  - 黙って捨てない照合は、決定 0121・0129・0130 と `docs/formats/sxf.md` の「P21 の組み直し」の置き方で、置く所（用紙と部分図）の図形をたどる。作図グループは置いた所に広げ、表示しない複合曲線の曲線と、作図部品の定義の中身は数えない（zumen の要素の一覧にないため）。SFC は診断の kind がフィーチャの名前と同じことも確かめ、P21 はレコードだけで結ぶ（kind は組み直したフィーチャの実体の名前）。
  - SFC は、レイヤ・用紙の中にあることと、線・円・点マーカーの形と、足した図形の色（既定義色の番号）を比べる。ezsxf は複合曲線を用紙に置かず、ハッチングの境界の定義として持つので、複合曲線の曲線の要素は、ezsxf の読みで表示する複合曲線（表示のフラグが 1）の曲線であることを比べる。ezsxf がフィーチャを型で読まない SFC（印が `/*SXF3` など）は、要素の種類だけを比べる。ezsxf が複合曲線を読めない SFC は、用紙の中にあることを比べない。
  - P21 は、複合のインスタンスも読み、レイヤ（PRESENTATION_LAYER_ASSIGNMENT）と、用紙（最初の DRAWING_SHEET_REVISION の並びと、そこに置いた作図グループと複合曲線）の中にあることと、参照をたどって、点の位置・回転・倍率と、線分の端点を比べる（印の種類と色、寸法と引出線のレイヤは比べない）。
  - ezsxf の警告は、code の種類で比べる。

`make.py` は、照らすための入力を作ります。どれも、役目を果たさなくなったら失敗にします（ezsxf が SFC を型で読まない、移さなかったことを言う診断の場所の数が変わる）。

- ezsxf がフィーチャを型で読む SFC（`input/made-features.sfc`）。
- zumen が要素にしない図形を 20 持つ DXF（`input/made-dropped.dxf`）。診断の場所が既定の数（16）を超えても、黙って捨てない照合が働くことを確かめる。平面の図面モデルに移さない図形（`dxf.non-planar.dropped`）も持つ。
- 置き方と黙って捨てない照合の枝を通す SFC と P21（`input/made-structure.sfc`・`input/made-structure.p21`）。作図部品・作図グループ・表示する複合曲線と表示しない複合曲線・ハッチング・文字・バルーン（P21）と、zumen が移さない半径 0 の円を持つ。書き戻しと変換は当てない。

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
- ODA File Converter・DWG TrueView など、使うのに規約への同意が要る道具。入れること自体が、利用者を待つもの（決定 0254）。
