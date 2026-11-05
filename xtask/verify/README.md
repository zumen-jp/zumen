# 外部の実装との照合と検証キット

zumen が書き戻した（apply）・変換した（convert）図面を、zumen でない実装で読んで照らし、AutoCAD・Jw_cad・SXF ブラウザで開いて確かめる検証キットを作ります（[決定 0254](../../docs/decisions/0254-parallel-agents-and-cross-review.md)、#503）。外部の実装と照らした範囲は、形式の台帳（`formats/ledger.toml`）の確かめた度合いと、形式の文書の「確かめた度合い」の節に書きます（[決定 0255](../../docs/decisions/0255-verification-levels-in-the-ledger.md)）。自前の往復のテストは、書く側と同じ読みで確かめるので、仕様の読み違いを見つけません（#345）。

| ファイル | 中身 |
| --- | --- |
| `make.py` | zumen の CLI（`zumen serve --stdio`）で、公開サンプル（`samples/`）と適合テストの中で作った図面を開き、書き戻しと変換を当てて、キットのフォルダーに書く。zumen が読んだ値・入力の出所とライセンス・当てた変更・期待する形を `manifest.json` に、issue ごとの手順と期待する結果を `README.md` に、ライセンスの本文を `licenses/` に書く |
| `compare.py` | 照らすスクリプトが共有する比べ方。形の値の比べ方、出所のハンドルで結んだ図形の照らし方、結果の書き方 |
| `check_ezdxf.py` | キットの DXF を ezdxf で読み、zumen の読みと照らす |
| `acadsharp/` | キットの DWG を ACadSharp で読み、値をキットの `acadsharp-dump.json` に書き出す .NET のツール |
| `check_acadsharp.py` | `acadsharp-dump.json` の値を、zumen の読みと照らす |
| `check_ezjww.py` | キットの JWW を ezjww で読み、zumen の読みと照らす |
| `check_ezsxf.py` | キットの SFC・P21 を ezsxf で読み、zumen の読みと照らす |
| `pdfjs/` | PDF の寸法・文字・経路と画像の数を pdf.js と照らす。手順は [README](pdfjs/README.md) |
| `dump.lsp` | AutoCAD で開いた図面の図形を、ハンドルごとに 1 行で書き出す。キットの README の手順で使う |

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

ACadSharp のツールは、.NET 10 の SDK で動かします。ACadSharp のバージョンは、`acadsharp/packages.lock.json` で固定します。

キットのフォルダーは、リポジトリの外に作ります。`make.py` は、前のキット（`manifest.json` がある）か空のフォルダーだけを消して作り直し、ほかのフォルダーは消さずに止まります。

毎晩の CI（`.github/workflows/nightly.yml` の `external`）が同じことを流し、キットを成果物の `verify-bundle` に置きます。検証キットの手順と期待する結果は、キットの `README.md` にあります。

受け取った図面で確かめるときは、`make.py` に `--inputs <フォルダー>` を足します。キットをリポジトリの中に作ろうとすると止まります。そのキットは、Git にも CI の成果物にも入れず、issue には件数と種類だけを書き、ファイルの名前と中身を書きません（決定 0254）。

## 照らすこと

`make.py` は、書いたファイルを zumen で読み直し、診断の件数が code と kind ごとに入力の読みより増えたら、そのファイルを `manifest.json` の `reread` に残して失敗にします。

照らすスクリプトは、どれもファイルごとに次を確かめます。

- 外部の実装が読める。書き戻したファイルで、外部の実装の監査・警告・通知が、入力より増えない。
- zumen の要素の出所（ハンドル・レコード・インスタンス）の図形が外部の実装にあり、種類と画層が同じ。DXF・DWG・JWW は種類を要素の形で比べるので、zumen の entities の応答が形を返さない要素（ハッチング・画像の範囲・表など）は、種類を比べない（#558）。外部の実装の読みに画層があれば、zumen の要素も画層を持つ。形の値（端点・中心・半径・角度・位置・文字・尺度など）も同じ。形のフィールドは両向きに比べ、どちらかの読みにだけあるフィールドも食い違いにする。形を比べる種類の図形は、zumen の要素も形を持つ。数は、相対と絶対の誤差 1e-9 まで同じとみなす。zumen の読みの null（zumen の JSON は NaN と無限大を null にする）は、外部の実装の読みも null のときだけ同じとみなす。角度は、360° の違いを同じとみなす。形と図形の種類の対応がなければ、表に足すよう失敗にする。
- 図形（DXF・DWG はモデル空間の図形、JWW は図形の一覧の図形、SFC は置く所のフィーチャ、P21 は用紙の並びの要素）は、どれも zumen の要素になるか、図形を移さなかったことを言う zumen の診断（`compare.py` の `LIFTED_AWAY`。`*.lift.*`・`dxf.non-planar.dropped` など）がある（黙って捨てない）。図形の一部だけを落としたことを言う診断（`PARTLY`）と、ほかの診断は数えない。`catalog/codes.toml` で落としたことを言う code（fidelity が dropped）のうち、どちらにも分けていないものが出たら、分けるよう失敗にする。診断の場所は、数を限らずに求める。
- 書き戻したファイルで、変更で決めた形と画層（`manifest.json` の `expect`）が、外部の実装で読んだ値と同じ。期待する値は、zumen の読みでなく、変更そのものと変換の行列から決める。期待する形は、変更に書いたフィールドだけを比べる。当てる候補のない段は、`skipped` としてキットに残す。
- 1 つのファイルでスクリプトが止まっても、そのファイルの食い違いにして、ほかのファイルを照らす。

形式ごとに、次を足します。

- DXF（`check_ezdxf.py`）: 読む中で流れる ezdxf の監査（`recover.readfile`）で、書き戻したファイルの誤りと直した誤りが、code ごとの件数で入力より増えない。変換したファイルには、誤りも直した誤りもなく、各 TABLE の宣言件数（70）が実件数と同じ。画層の名前は、R2007 以降を UTF-8、それより前をファイルのコード・ページで読み、ファイルの画層の表と同じ（ezdxf が読むときに足す Defpoints は除く）。比べられる種類（LINE・CIRCLE・ARC・ELLIPSE・LWPOLYLINE・POINT・TEXT・MTEXT・INSERT）のモデル空間の要素は、形を持つ。要素にならない図形は、移さなかったことを言う診断が、その図形の種類とレコードを指す（kind のない code は、レコードだけで結ぶ）。足した画層の色、色を変えた図形の色、点の表し方（$PDMODE・$PDSIZE）も比べる。
- DWG（`check_acadsharp.py`）: 画層の名前、比べる形と色は DXF と同じ。ACadSharp は図形のバイトの位置を返さないので、要素にならない図形の数が、種類ごとに、移さなかったことを言う診断（kind は図形の種類。kind のない code は、その code が言いうる種類（`compare.py` の `KINDLESS`）に足す）の件数を超えないことを確かめる。そのため、ブロックの定義の中など、ほかの所の同じ種類の図形を言う診断と区別しない。キットの DWG には zumen が移さない図形がないので、この照合は受け取った図面（`--inputs`）でだけ働く（zumen は要素にしない図形を DWG に足せず、照らすための DWG を作れない）。通知は文で比べる。
- JWW（`check_ezjww.py`）: zumen のレコードの番号は、ヘッダーを 0 とし、図形の一覧の i 番目（0 から）を i + 1 とする（決定 0133）。要素にならない図形は、移さなかったことを言う診断が、その図形のレコードを指す（kind はクラスの名前なので比べない）。zumen が描かずに拡張に持つ文字（(0, -1000) の `名前 = 値` の設定と、`^@BM` でない `^@` で始まる文字。`docs/formats/jww.md`）は数えず、要素の出所にあれば食い違いにする。ezjww の座標は用紙の mm なので、レイヤグループの縮尺の分母を掛けて比べる。形は、線と点を比べ、線と点の要素は形を持つ。足した図形のペンの番号も比べる。キットには適合テストの線の終点 Y を NaN にした JWW も入れ、`jww.lift.invalid` の場所が 1 件あることを `PLACES` で確かめる。これで、診断とレコードを結ぶ枝を毎回通す（#548）。描かない文字を持つ合成の入力は #558 で足す。
- SFC・P21（`check_ezsxf.py`）: zumen のレコードの番号は、`docs/formats/sxf.md` の「判定」と「レコードの区切り」で数え（AP202 と書いた P21 ではフィーチャのマーカーを読まず、zumen が読めないと言った範囲は数えない）、要素の出所の番号と同じことも確かめる。要素の種類と形が、出所のフィーチャ（SFC）か実体（P21）に合う。P21 の曲線の要素は、出所の曲線の実体（トリムした曲線は基の曲線）で決まる種類と形を持つ。
  - 黙って捨てない照合は、決定 0121・0129・0130 と `docs/formats/sxf.md` の「P21 の組み直し」の置き方で、置く所（用紙と部分図）の図形をたどる。作図グループは置いた所に広げ、表示しない複合曲線の曲線と、作図部品の定義の中身は数えない（zumen の要素の一覧にないため）。表示しない複合曲線がまとめた、曲線でないフィーチャ（文字・点マーカーなど）は、zumen が元のレコードを診断で報告するので数える（#549）。SFC は診断の kind がフィーチャの名前と同じことも確かめ、P21 はレコードだけで結ぶ（kind は組み直したフィーチャの実体の名前）。
  - SFC は、レイヤ・用紙の中にあることと、線・円・点マーカーの形と、足した図形の色（既定義色の番号）を比べる。ezsxf は複合曲線を用紙に置かず、ハッチングの境界の定義として持つので、複合曲線の曲線の要素は、ezsxf の読みで表示する複合曲線（表示のフラグが 1）の曲線であることを比べる。ezsxf がフィーチャを型で読まない SFC（印が `/*SXF3` など）は、要素の種類だけを比べる。ezsxf が複合曲線を読めないか飛ばした SFC と、ハッチングの指す複合曲線がないと言った SFC は、用紙の中にあることを比べない。
  - P21 は、複合のインスタンスも読み、レイヤ（PRESENTATION_LAYER_ASSIGNMENT。2 つのレイヤに入れた要素は先のもの。複合曲線の曲線の要素は、複合曲線を指す曲線の表記の実体のもの）と、用紙（番号がもっとも小さい DRAWING_SHEET_REVISION の並びと、そこに置いた作図グループと表示する複合曲線）の中にあることと、参照をたどって、点の位置・回転・倍率と、線分の端点（点か媒介変数のトリム）を比べる（点マーカーの種類と色、寸法と引出線のレイヤは比べない）。同じ番号の 2 つ目のインスタンスは使わない（zumen と同じ）。画像の範囲の要素は、画像の作図グループ（属性の `$$ATRU$$番号$$名称$$画像$$` か、名前の `$$RASTER$$` の後に、ファイルの名前があるもの）の中の 4 点以上の折れ線から作ったことを比べる。逆の向き（画像の作図グループの折れ線を、zumen が画像の範囲にしたこと）は比べない（#558）。
  - ezsxf は P21 の文字列のエスケープを戻さず、8 ビットのバイトを UTF-8 として読むので、レイヤと作図グループの名前は、ファイルのバイトから zumen と同じく戻す（エスケープを戻し、8 ビットのバイトは Shift_JIS として読む。`docs/formats/sxf.md`）。
  - ezsxf 0.3.7 は、引数の中にコメントを持つ P21 の文を読み飛ばす（警告 `salvage-record`）。zumen はその文も読むので、その文を指す要素と、その文から置いた図形は、食い違いになる（#558）。
  - ezsxf の警告は、code の種類で比べる。

`make.py` は、照らすための入力を作ります。どれも、役目を果たさなくなったら失敗にします（ezsxf が SFC を型で読まない、移さなかったことを言う診断の場所の数が変わる）。

- ezsxf がフィーチャを型で読む SFC（`input/made-features.sfc`）。
- zumen が要素にしない図形を 20 持つ DXF（`input/made-dropped.dxf`）。診断の場所が既定の数（16）を超えても、黙って捨てない照合が働くことを確かめる。平面の図面モデルに移さない図形（`dxf.non-planar.dropped`）も持つ。
- 日本語の画層とその上の線を持つ R2004（ANSI_932）と R2007（UTF-8）の DXF（`input/made-japanese-r2004.dxf`・`input/made-japanese-r2007.dxf`）。変換で足す線種・画層・文字スタイルも持ち、TABLE の件数と、文字コードごとの画層の照合を確かめる。
- 置き方と黙って捨てない照合の枝を通す SFC と P21（`input/made-structure.sfc`・`input/made-structure.p21`）。作図部品・作図グループ・表示する複合曲線と表示しない複合曲線・ハッチング・文字・バルーン（P21）と、zumen が移さない半径 0 の円を持つ。SFC は、非表示の複合曲線の前にも文字・点マーカー・ハッチング・作図部品と作図グループの配置を持つ（#549）。書き戻しと変換は当てない。

既知の食い違いは、スクリプトごとの `KNOWN` に、issue と当たるファイルの数と一緒に置き、失敗にしません。当たるファイルの数が変わったら（直ったか、同じ形の食い違いが増えた）失敗にして、`KNOWN` を直すことを知らせます。

## 外部の実装

外部の実装は、テストで別のプログラムとして動かし、成果物にも依存にも入れません。バージョンを決めて入れます。

| 形式 | 実装 | ライセンス | 扱い |
| --- | --- | --- | --- |
| DXF | [ezdxf](https://github.com/mozman/ezdxf) 1.4.5 | MIT | 照らす（`check_ezdxf.py`） |
| DWG | [ACadSharp](https://github.com/DomCR/ACadSharp) 3.8.0 | MIT | 照らす（`acadsharp/`・`check_acadsharp.py`）。.NET のツールで読み、値を JSON に書き出す |
| JWW | [ezjww](https://github.com/monozukuri-ai/ezjww) 0.7.6 | MIT | 照らす（`check_ezjww.py`） |
| SFC・P21 | [ezsxf](https://github.com/monozukuri-ai/ezsxf) 0.3.7 | MIT | 照らす（`check_ezsxf.py`） |
| PDF | [pdf.js](https://github.com/mozilla/pdf.js) 6.5.438 | Apache-2.0 | 別のプログラムとして呼び、読みを照らす（`pdfjs/`）。Node.js 22.13.0 以降 |

次のものは使いません。

- LibreDWG などの GPL の実装。ソースも読みません（`AGENTS.md`）。
- ODA File Converter・DWG TrueView など、使うのに規約への同意が要るツール。入れること自体が、利用者を待つもの（決定 0254）。
