# 外部の実装との照合と検証キット

zumen が書き戻した（apply）・変換した（convert）図面を、zumen でない実装で読んで照らし、AutoCAD・Jw_cad・SXF ブラウザで開いて確かめる検証キットを作ります（#503）。照合した範囲は[形式台帳の手順](../../formats/README.md#確かめた度合い)に従って記録します。自前の往復のテストは、書く側と同じ読みで確かめるので、仕様の読み違いを見つけません（#345）。

IFC4・IFC2X3 の名前・説明・プロパティを書き戻し、BIM ソフトで確かめるキットは [ifc/README.md](ifc/README.md) にあります（#526）。公開モデルだけを使い、入力と編集後のファイル・既存 CLI の要求・期待値・報告様式を作ります。実機の操作は、利用者の環境と許可を待ちます。

| ファイル | 中身 |
| --- | --- |
| `make.py` | zumen の CLI（`zumen serve --stdio`）で、公開サンプル（`samples/`）と適合テストの中で作った図面を開き、書き戻しと変換を当てて、キットのフォルダーに書く。zumen が読んだ値・入力の出所とライセンス・当てた変更・期待する形を `manifest.json` に、issue ごとの手順と期待する結果を `README.md` に、ライセンスの本文を `licenses/` に書く |
| `compare.py` | 照らすスクリプトが共有する比べ方。形の値の比べ方、出所のハンドルで結んだ図形の照らし方、結果の書き方 |
| `check_ezdxf.py` | キットの DXF を ezdxf で読み、zumen の読みと照らす |
| `test_dxf_layer_fields.py` | ezdxf で作った画層 0 と追加画層の説明・透過性を、公開 API で変換した DXF と照らす（#635） |
| `acadsharp/` | キットの DWG を ACadSharp で読み、値をキットの `acadsharp-dump.json` に書き出す .NET のツール |
| `check_acadsharp.py` | `acadsharp-dump.json` の値を、zumen の読みと照らす |
| `check_ezjww.py` | キットの JWW を ezjww で読み、zumen の読みと照らす |
| `check_ezsxf.py` | キットの SFC・P21 を ezsxf で読み、zumen の読みと照らす |
| `test_ezsxf.py` | 合成 P21 の追加点のレイヤ所属と、同じ番号を持つ合成 SFC の各要素の形・種類・レイヤ・出所・欠落を個別に変え、`check_ezsxf.py` が食い違いを検出することを確かめる。合成 P21 の画像枠と折れ線の取り違え、枠のない画像の作図グループの診断の欠落、引数の中にコメントを持つ文の実体のレイヤの違いも検出することを確かめる |
| `test_source_kinds.py` | キットの読みの要素の種類を変え、`check_ezdxf.py`・`check_acadsharp.py`・`check_ezjww.py` が、形を返さない要素の種類の違いを検出することを確かめる。JWW では、文字と画像の範囲の取り違え、描かない文字を要素にした読み、診断の欠落も検出することを確かめる。DWG は `acadsharp-dump.json` があるときだけ確かめる |
| `pdfjs/` | `@zumen-jp/view2d` の PDF アンダーレイのブラウザーテストが使う固定版の pdf.js。手順は [README](pdfjs/README.md) |
| `geoid/` | ジオイドの格子の補間を、国土地理院のジオイド高計算サービスか内挿計算プログラムの結果と照らす（#438）。利用者が入手したモデルで、`cargo xtask geoid` を使う。手順は [README](geoid/README.md) |
| `acc/` | 公開 IFC と BCF 2.1 のオフライン検証キットを作り、Autodesk Build の取り込み・書き出しを利用者が確認する（#686・#787）。手順と期待値は [README](acc/README.md) |
| `check_autocad.py` | キットの DXF・DWG を、画面のない AutoCAD（`accoreconsole.exe`）で 1 つずつ開き、監査の誤りの数と `dump.lsp` の書き出しを、zumen の読みと変更で決めた値に照らす（#683）。AutoCAD のある端末の手元でだけ流す |
| `test_check_autocad.py` | `check_autocad.py` の、流すスクリプトの組み立て、コンソールの出力と書き出しの読み、形の値への直し、寸法の部品の比べ方を、合成の入力で確かめる。AutoCAD がなくても流せる |
| `dump.lsp` | AutoCAD で開いた図面の画層・線種・文字スタイルの表と図形を、ハンドルごとに 1 行で書き出す。`check_autocad.py` がスクリプトに埋めて流す。キットの README の手順でも使う |

## 使い方

```bash
cargo build --release -p zumen-cli
python -m pip install --only-binary :all: ezdxf==1.4.5 ezjww==0.7.6 ezsxf==0.3.7
python -I xtask/verify/make.py target/release/zumen ../zumen-verify
python -I -m unittest discover -s xtask/autocad/check316 -p 'test_*.py'
python -I xtask/autocad/check316/make.py ../zumen-verify/316/controls
python -I xtask/verify/check_ezdxf.py ../zumen-verify
python -I xtask/verify/test_dxf_layer_fields.py target/release/zumen
python -I xtask/verify/check_ezjww.py ../zumen-verify
python -I xtask/verify/check_ezsxf.py ../zumen-verify
python -I xtask/verify/test_ezsxf.py ../zumen-verify
dotnet run --project xtask/verify/acadsharp -c Release -- ../zumen-verify
python -I xtask/verify/check_acadsharp.py ../zumen-verify
python -I xtask/verify/test_source_kinds.py ../zumen-verify
```

ACadSharp のツールは、.NET 10 の SDK で動かします。ACadSharp のバージョンは、`acadsharp/packages.lock.json` で固定します。`test_source_kinds.py` は、DWG を ACadSharp のツールが書く `acadsharp-dump.json` で確かめるので、そのツールの後に流します。このファイルがないときは、DWG だけを飛ばしてそう表示します。

キットのフォルダーは、リポジトリの外に作ります。`make.py` は、前のキット（`manifest.json` がある）か空のフォルダーだけを消して作り直し、ほかのフォルダーは消さずに止まります。

毎晩の CI（`.github/workflows/nightly.yml` の `external`）が同じことを流し、キットを成果物の `verify-bundle` に置きます。検証キットの手順と期待する結果は、キットの `README.md` にあります。

受け取った図面で確かめるときは、`make.py` に `--inputs <フォルダー>` を足します。キットをリポジトリの中に作ろうとすると止まります。そのキットは、Git にも CI の成果物にも入れず、issue には件数と種類だけを書き、ファイルの名前と中身を書きません（[開発の進め方](../../docs/work.md)）。

## AutoCAD で開いて照らす

AutoCAD のある Windows の端末では、キットの DXF・DWG を、画面のない AutoCAD（`accoreconsole.exe`）で 1 つずつ開いて照らせます（#683）。CI では流しません。

```bash
python -I xtask/verify/check_autocad.py ../zumen-verify
python -I xtask/verify/check_autocad.py ../zumen-verify --issue 683
python -I -m unittest discover -s xtask/verify -p 'test_check_autocad.py'
```

- `accoreconsole.exe` は、`--exe`、環境変数 `ZUMEN_ACCORECONSOLE`、既定の導入先（`Program Files` の `Autodesk\AutoCAD *`）の順に探す。`--issue` は、その issue のファイルと入力だけを流す（繰り返せる）。1 つの図面は 3 秒ほどで、キットの全体は 7 分ほど。
- 図面は一時フォルダーの写しを開き、`dump.lsp` を埋めたスクリプトで、書き出しと監査（`AUDIT`、直さない）を流して、保存せずに閉じる。AutoCAD は保存のたびに Windows のログイン名を図面に書く（[公開サンプルの条件](../../samples/README.md)）ので、AutoCAD が保存したファイルを作らない。
- AutoCAD のコンソールの出力は、道筋にログイン名を含むので、一時フォルダーに置いて終わりに消す。結果（キットの `autocad.json`）には、状態・数と、`dump.lsp` の書き出しから読んだ値との食い違いだけを書く。`--keep <フォルダー>` で作業のフォルダーを残せる（リポジトリの外に限る）。
- ファイルごとの状態は、開けて書き出しと監査が終わった `ok`、AutoCAD が図面を開けなかった `unopened`、`--timeout` の秒数（既定 300）で止めた `timeout`、開けたが書き出しか監査が終わらなかった `error` に分ける。`timeout` と `error` は、どこまで進んだかも残す。
- 監査の誤りの数は、AutoCAD の文の言語によらず、数だけを読む。読み違えて 0 にしないよう、初めに、監査の誤りが出る自作の図面と、出ない自作の図面を流し、0 より大きい数と 0 を読めなければ止まる。
- 終了コードは、0 が食い違いなし、1 が食い違いあり、2 が流せない（`accoreconsole.exe` がない、対照の図面で数を読めないなど）。

## 照らすこと

`make.py` は、書いたファイルを zumen で読み直し、診断の件数が code と kind ごとに入力の読みより増えたら、そのファイルを `manifest.json` の `reread` に残して失敗にします。

照らすスクリプトは、どれもファイルごとに次を確かめます。

- 外部の実装が読める。書き戻したファイルで、外部の実装の監査・警告・通知が、入力より増えない。
- zumen の要素の出所（ハンドル・レコード・インスタンス）の図形が外部の実装にあり、種類と画層が同じ。DXF・DWG・JWW は、要素の種類（`kind`）と、あれば要素の形を、出所の図形の種類と比べる（`compare.py`・`check_ezjww.py` の `KINDS` と `TYPES`）。zumen の entities の応答が形を返さない要素（ハッチング・画像の範囲・表など）も、要素の種類で比べる（#558）。外部の実装の読みに画層があれば、zumen の要素も画層を持つ。形の値（端点・中心・半径・角度・位置・文字・尺度など）も同じ。形のフィールドは両向きに比べ、どちらかの読みにだけあるフィールドも食い違いにする。形を比べる種類の図形は、zumen の要素も形を持つ。数は、相対と絶対の誤差 1e-9 まで同じとみなす。zumen の読みの null（zumen の JSON は NaN と無限大を null にする）は、外部の実装の読みも null のときだけ同じとみなす。角度は、360° の違いを同じとみなす。要素の種類か形と、図形の種類の対応がなければ、表に足すよう失敗にする。
- 図形（DXF・DWG はモデル空間の図形、JWW は図形の一覧の図形、SFC は置く所のフィーチャ、P21 は用紙の並びの要素）は、どれも zumen の要素になるか、図形を移さなかったことを言う zumen の診断（`compare.py` の `LIFTED_AWAY`。`*.lift.*`・`dxf.non-planar.dropped` など）がある（黙って捨てない）。図形の一部だけを落としたことを言う診断（`PARTLY`）と、ほかの診断は数えない。`catalog/codes.toml` で落としたことを言う code（fidelity が dropped）のうち、どちらにも分けていないものが出たら、分けるよう失敗にする。診断の場所は、数を限らずに求める。
- 書き戻したファイルで、変更で決めた形と画層（`manifest.json` の `expect`）が、外部の実装で読んだ値と同じ。期待する値は、zumen の読みでなく、変更そのものと変換の行列から決める。期待する形は、変更に書いたフィールドだけを比べる。当てる候補のない段は、`skipped` としてキットに残す。
- 1 つのファイルでスクリプトが止まっても、そのファイルの食い違いにして、ほかのファイルを照らす。

形式ごとに、次を足します。

- DXF（`check_ezdxf.py`）: 読む中で流れる ezdxf の監査（`recover.readfile`）で、書き戻したファイルの誤りと直した誤りが、code ごとの件数で入力より増えない。変換したファイルには、誤りも直した誤りもなく、各 TABLE の宣言件数（70）が実件数と同じ。画層の名前は、R2007 以降を UTF-8、それより前をファイルのコード・ページで読み、ファイルの画層の表と同じ（ezdxf が読むときに足す Defpoints は除く）。比べられる種類（LINE・CIRCLE・ARC・ELLIPSE・LWPOLYLINE・POINT・TEXT・MTEXT・INSERT）のモデル空間の要素は、形を持つ。要素にならない図形は、移さなかったことを言う診断が、その図形の種類とレコードを指す（kind のない code は、レコードだけで結ぶ）。足した画層の色、色を変えた図形の色、点の表し方（$PDMODE・$PDSIZE）も比べる。
- R2004 DWG の弧長寸法: 公開 DWG に画層を足す場合（`Walls`・`外壁`、ACI 3）と元のままの場合を DXF に変換する。ezdxf で追加した画層の名前・色を読み、AutoCAD が同じ図面から保存した公開 DXF と、モデル空間の寸法 6 件の定義点・寸法値・所有者、寸法スタイルと描画ブロックの参照、ブロック内の図形 67 件の順序・値・文字、クラスの宣言、リアクター、プロキシ画像の全バイトを照合する。省いたリアクターの診断は、元の DWG にある図形のバイト範囲と照らす（#634・#641）。
- DWG（`check_acadsharp.py`）: 画層の名前、比べる形と色は DXF と同じ。ACadSharp は図形のバイトの位置を返さないので、要素にならない図形の数が、種類ごとに、移さなかったことを言う診断（kind は図形の種類。kind のない code は、その code が言いうる種類（`compare.py` の `KINDLESS`）に足す）の件数を超えないことを確かめる。そのため、ブロックの定義の中など、ほかの所の同じ種類の図形を言う診断と区別しない。キットの DWG には zumen が移さない図形がないので、この照合は受け取った図面（`--inputs`）でだけ働く（zumen は要素にしない図形を DWG に足せず、照らすための DWG を作れない）。通知は文で比べる。
- DXF・DWG を AutoCAD で開く（`check_autocad.py`）: 外部の実装の読みの代わりに、AutoCAD で開いた図面から `dump.lsp` が書き出した値を使う。監査の誤りの数が、書き戻したファイルでは入力より増えず、変換したファイルでは 0。書き戻したファイルと変換したファイルは、AutoCAD が開けなければ食い違いにし、入力は記録だけにする（AutoCAD が読み込まない公開サンプルと合成の入力がある）。AutoCAD は、図形が指すだけで表にない画層も作るので、画層の名前は、zumen が読んだ画層のすべて（定義のないものも）と比べる。形の値は、DXF・DWG のほかの照合と同じ種類を比べ、INSERT は格子と属性の値も比べる。書き戻したファイルでは、記号の定義の中に足した図形の形・画層・色・線種と、足した画層の色・線種・状態、足した線種の説明と列、足した文字スタイルのフォント・高さ・幅の倍率・傾き・向きも比べる。
  - AutoCAD が読み込むときに決め直す値は比べず、比べなかった数を結果に残す。DXF の、2 点の間に置く文字（TEXT の 72 が 3・5）の、両端揃えの高さと、フィットの幅の倍率は、AutoCAD が文字の幅から決め直す（ezdxf が書いた公開サンプルの `text_alignments.dxf` でも変わる。DWG では、ファイルの値のまま読む）。見出しの `$ACADVER` が AC1012（R13）より前か、見出しのない DXF は、AutoCAD がハンドルを振り直すので、開けるかと監査の誤りの数だけを見る。
  - 要素にならない図形を黙って捨てていないかは、ezdxf と ACadSharp の照合で確かめ、ここでは確かめない。属性の文字の位置と、画面での見え方も確かめない（accoreconsole は画面を持たない）。
  - #314 の実験のファイルは、食い違いにせず、監査を「直す」でも流して、寸法の部品（匿名のブロック）を AutoCAD が作り直したかを記録する。
  - 今の既知の食い違いは、convert で書いた DXF を AutoCAD が読み込めないこと（#879）と、拡張データでつないだ R2007 までの段組みの MTEXT を AutoCAD が 1 つにまとめて読むこと（#174）。
- JWW（`check_ezjww.py`）: zumen のレコードの番号は、ヘッダーを 0 とし、図形の一覧の i 番目（0 から）を i + 1 とする（[JWW の読み書き](../../docs/formats/jww.md)）。要素にならない図形は、移さなかったことを言う診断が、その図形のレコードを指す（kind はクラスの名前なので比べない）。zumen が描かずに拡張に持つ文字（(0, -1000) の `名前 = 値` の設定と、`^@BM` でない `^@` で始まる文字。`docs/formats/jww.md`）は数えず、要素の出所にあれば食い違いにする。ezjww の座標は用紙の mm なので、レイヤグループの縮尺の分母を掛けて比べる。形は、線と点を比べ、線と点の要素は形を持つ。足した図形のペンの番号も比べる。キットには適合テストの線の終点 Y を NaN にした JWW も入れ、`jww.lift.invalid` の場所が 1 件あることを `PLACES` で確かめる。これで、診断とレコードを結ぶ枝を毎回通す（#548）。同じ JWW の図形の一覧を置き換えて、設定と制御の文字・描く文字・ソリッド・寸法・`^@BM` の画像を持つ JWW（`input/made-classes.jww`）も入れ、元の線の `jww.lift.invalid` の場所が 1 件あることを `PLACES` で確かめる。zumen はソリッドをハッチングの要素、`^@BM` の文字を画像の範囲の要素にし、ezjww はそれぞれ `SOLID`・`TEXT` で読む。文字と画像の範囲は、どちらも出所が `TEXT` で形を返さないので、ezjww が読んだ文字が `^@BM` で始まるときだけ画像の範囲（`underlay`）、ほかは文字（`text`）であることも比べる。`test_source_kinds.py` は、このファイルの要素の並びが変わったら失敗にし、描かない文字を要素にした読み、診断を消した読み、文字と画像の範囲の種類を入れ替えた読みを食い違いにすることを確かめる（#558）。
- SFC・P21（`check_ezsxf.py`）: zumen のレコードの番号は、`docs/formats/sxf.md` の「判定」と「レコードの区切り」で数え（AP202 と書いた P21 ではフィーチャのマーカーを読まず、zumen が読めないと言った範囲は数えない）、要素の出所の番号と同じことも確かめる。要素の種類と形が、出所のフィーチャ（SFC）か実体（P21）に合う。P21 の曲線の要素は、出所の曲線の実体（トリムした曲線は基の曲線）で決まる種類と形を持つ。
  - 黙って捨てない照合は、[SXF の読み書き](../../docs/formats/sxf.md) と `docs/formats/sxf.md` の「P21 の組み直し」の置き方で、置く所（用紙と部分図）の図形をたどる。作図グループは置いた所に広げ、表示しない複合曲線の曲線と、作図部品の定義の中身は数えない（zumen の要素の一覧にないため）。表示しない複合曲線がまとめた、曲線でないフィーチャ（文字・点マーカーなど）は、zumen が元のレコードを診断で報告するので数える（#549）。SFC は診断の kind がフィーチャの名前と同じことも確かめ、P21 はレコードだけで結ぶ（kind は組み直したフィーチャの実体の名前）。
  - SFC は、レイヤ・用紙の中にあることと、線・円・点マーカーの形と、足した図形の色（既定義色の番号）を比べる。ezsxf は複合曲線を用紙に置かず、ハッチングの境界の定義として持つので、複合曲線の曲線の要素は、ezsxf の読みで表示する複合曲線（表示のフラグが 1）の曲線であることを比べる。ezsxf がフィーチャを型で読まない SFC（印が `/*SXF3` など）は、要素の種類だけを比べる。ezsxf が複合曲線を読めないか飛ばした SFC と、ハッチングの指す複合曲線がないと言った SFC は、用紙の中にあることを比べない。
  - SFC の同じ番号のフィーチャは、要素の `source.record` でそれぞれの生のフィーチャと型で読めた値に結ぶ。番号が重なる場合や、宣言以外にもマーカーがある場合は、メモリ上のコピーで DATA 節の宣言番号を一意にして ezsxf で読む。zumen が読み飛ばした範囲がある場合や、ezsxf が警告を返した場合も同じ。元の番号に戻した値を元ファイルの読みと照らして、同じ位置のフィーチャに結ぶ。ヘッダーから回復した同番号のフィーチャしか読めないときは、対応する出所がないことを報告する。型で読めなかったレコードに後続の値を当てず、要素の欠落と診断の種類・場所もレコードで比べる。用紙と作図グループについて ezsxf が返す所属はフィーチャ番号で、同じ番号では番号の単位までの照合になる（#553）。
  - P21 は、複合のインスタンスも読み、レイヤ（PRESENTATION_LAYER_ASSIGNMENT。2 つのレイヤに入れた要素は先のもの。複合曲線の曲線の要素は、複合曲線を指す曲線の表記の実体のもの）と、用紙（番号がもっとも小さい DRAWING_SHEET_REVISION の並びと、そこに置いた作図グループと表示する複合曲線）の中にあることと、参照をたどって、点の位置・回転・倍率と、線分の端点（点か媒介変数のトリム）を比べる（点マーカーの種類と色、寸法と引出線のレイヤは比べない）。同じ番号の 2 つ目のインスタンスは使わない（zumen と同じ）。画像の範囲の要素は、画像の作図グループ（属性の `$$ATRU$$番号$$名称$$画像$$` か、名前の `$$RASTER$$` の後に、ファイルの名前があるもの）の中で、最初の 4 点以上の折れ線（[SXF の読み書き](../../docs/formats/sxf.md)）から作ったことを比べる。座標を読めない点を持つ折れ線を飛ばすのは zumen の `raster` と同じ選び方で、[SXF の読み書き](../../docs/formats/sxf.md) にはない。照合側も同じ選び方をするので、この所は仕様の解釈を独立に確かめるものではない。逆の向きも比べる。画像の作図グループを用紙に置いたら、その折れ線が配置の数だけ画像の範囲の要素になる。同じ実体を別の配置では通常の折れ線にできるので、配置の数と、退化した枠を落とす `sxf.lift.invalid` の数で比べる。その折れ線がない作図グループは、定義と名前を指す `sxf.raster-frame.missing` があることを確かめる（#558）。
  - ezsxf は P21 の文字列のエスケープを戻さず、8 ビットのバイトを UTF-8 として読むので、レイヤと作図グループの名前は、ファイルのバイトから zumen と同じく戻す（エスケープを戻し、8 ビットのバイトは Shift_JIS として読む。`docs/formats/sxf.md`）。
  - Shift_JIS のレイヤ名を持つ合成 P21（`apply_p21_raw.json`）にも点を 2 つ足す。書き戻したファイルで、各点の形と、変更で決めたレイヤ「図枠」に属すことを照らす。一時コピーでレイヤの所属一覧だけから 2 点を外すと、用紙の並びと形を保っていても、各点の所属を食い違いにする（#552）。
  - ezsxf 0.3.7 は、引数の中にコメントを持つ P21 の文を読み飛ばす（警告 `salvage-record`）。zumen はその文も読むので、この警告があれば、コメントを空白にしたメモリ上のコピーを ezsxf で読み直す。元の読みの実体が値も順序も変わらずに残るときだけ、増えた文を照合に使い、その番号を結果の注記に書く。これで、その文を指す要素と、その文から置いた図形も、食い違いにせずに照らす（#558）。
  - ezsxf の警告は、code の種類で比べる。

`make.py` は、照らすための入力を作ります。どれも、役目を果たさなくなったら失敗にします（ezsxf が SFC を型で読まない、移さなかったことを言う診断の場所の数が変わる）。

- ezsxf がフィーチャを型で読む SFC（`input/made-features.sfc`）。
- zumen が要素にしない図形を 20 持つ DXF（`input/made-dropped.dxf`）。診断の場所が既定の数（16）を超えても、黙って捨てない照合が働くことを確かめる。平面の図面モデルに移さない図形（`dxf.non-planar.dropped`）も持つ。
- zumen の entities の応答が形を返さない要素になる図形を持つ R2010 の DXF（`input/made-shapeless.dxf`）。HATCH・SOLID・TRACE はハッチング、IMAGE・PDFUNDERLAY は画像の範囲、ACAD_TABLE は表の要素になる。要素の種類を出所の図形の種類と比べる照合を、毎晩のキットで通し、`test_source_kinds.py` は、その種類を入れ替えた読みを食い違いにすることを確かめる（#558）。キットの DWG（公開サンプル）はこれらの図形を持たないので、DWG は `test_source_kinds.py` が寸法の要素から形を外した読みで確かめる。
- 日本語の画層とその上の線を持つ R2004（ANSI_932）と R2007（UTF-8）の DXF（`input/made-japanese-r2004.dxf`・`input/made-japanese-r2007.dxf`）。変換で足す線種・画層・文字スタイルも持ち、TABLE の件数と、文字コードごとの画層の照合を確かめる。
- 置き方と黙って捨てない照合の枝を通す SFC と P21（`input/made-structure.sfc`・`input/made-structure.p21`）。作図部品・作図グループ・表示する複合曲線と表示しない複合曲線・ハッチング・文字・バルーン（P21）と、zumen が移さない半径 0 の円を持つ。P21 は、枠の折れ線がある画像の作図グループと、ない画像の作図グループ、引数の中にコメントを持つ文も持つ（#558）。SFC は、非表示の複合曲線の前にも文字・点マーカー・ハッチング・作図部品と作図グループの配置を持つ（#549）。書き戻しと変換は当てない。
  - P21 は、番号が大きい用紙とレイヤを先に書き、同じ番号の点も後に書く。採用する形・レイヤ・用紙内の所属が変わらず、採用しない実体・用紙・レイヤへの所属を診断することを確かめる（#550）。照合側も zumen と同じ選び方をするので、仕様の解釈を独立に確かめるものではない。

既知の食い違いは、スクリプトごとの `KNOWN` に、issue と当たるファイルの数と一緒に置き、失敗にしません。当たるファイルの数が変わったら（直ったか、同じ形の食い違いが増えた）失敗にして、`KNOWN` を直すことを知らせます。

## Rust と API の DXF の lower を照らす

#643 の共用部分は、合成の土木図面を Rust の `lower_to_dxf` と、`convert`・`civil_drawing` が呼ぶ API の経路で書き、ezdxf 1.4.5 で照合します。`ZUMEN_DXF_LOWER_OUT` に出力先のフォルダーを指定して、`cargo test -p zumen-abi --lib the_api_writes_what_the_rust_lower_writes` を流します。そのフォルダーを `python -I xtask/verify/test_dxf_lower.py <出力先>` に渡します。検査後は環境変数を外します。

毎晩の CI でも、同じ合成ファイルを作り、このスクリプトで照合します。

出力はこのテストが作る小さい合成図面だけです。画層・線種・用紙とモデル空間の図形・LAYOUT の値と参照、監査の誤りと修復がないことを照らします。API の形の値が持たない TEXT の位置合わせだけを比較から除き、その差が実際にあることも確かめます（#763）。API の応答と診断、ネイティブと wasm の一致は、適合テストと Rust のテストで別に確かめます。

## 外部の実装

外部の実装は、テストで別のプログラムとして動かし、成果物にも依存にも入れません。バージョンを決めて入れます。

| 形式 | 実装 | ライセンス | 扱い |
| --- | --- | --- | --- |
| DXF | [ezdxf](https://github.com/mozman/ezdxf) 1.4.5 | MIT | 照らす（`check_ezdxf.py`） |
| DWG | [ACadSharp](https://github.com/DomCR/ACadSharp) 3.8.0 | MIT | 照らす（`acadsharp/`・`check_acadsharp.py`）。.NET のツールで読み、値を JSON に書き出す |
| JWW | [ezjww](https://github.com/monozukuri-ai/ezjww) 0.7.6 | MIT | 照らす（`check_ezjww.py`） |
| SFC・P21 | [ezsxf](https://github.com/monozukuri-ai/ezsxf) 0.3.7 | MIT | 照らす（`check_ezsxf.py`） |
| PDF アンダーレイ | [pdf.js](https://github.com/mozilla/pdf.js) 6.5.438 | Apache-2.0 | `@zumen-jp/view2d` のブラウザーテストで使う（`pdfjs/`） |
| LAS | [laspy](https://github.com/laspy/laspy) 2.7.0 | BSD-2-Clause | 非圧縮 LAS のヘッダーと全点を照らす（[`laspy/`](laspy/README.md)）。NumPy 2.5.4 を別に導入 |

次のものは使いません。

- LibreDWG などの GPL の実装。ソースも読みません（`AGENTS.md`）。
- ODA File Converter・DWG TrueView など、使うのに規約への同意が要るツール。入れること自体が、利用者を待つもの（[開発の進め方](../../docs/work.md)）。
