# 形式ごとの需要とコアで扱う範囲

2026-11-18 に、日本の建設でファイルを受け取る側と渡す側が必要とする操作を調べました。以下は次の投資を選ぶための調査と提案です。削除や非推奨化の決定はありません。現在の対応は[形式台帳](../formats/ledger.toml)、公開版の範囲は[リリース](releases/0.3.0.md)で確認できます。

## 判断に使う根拠

「形式・バージョン・操作・受け取り先」の組で考えます。たとえば DWG R2018 を開くことと、変更した R2007 を AutoCAD で開き直せるようにすることでは、相手も必要な検査も違います。

| 観点 | 確かめること |
| --- | --- |
| 需要 | 発注者の指定、実際に配られた成果、国内製品での利用例、仕様の存在を区別する。zumen を直接使う組織数や形式別の利用件数、保守に使った人日は今回の調査では不明 |
| zumen の役割 | 日本語の図面、電子納品、座標と標高、設計面と計測値の照合のどこに効くか。ブラウザーで原本を外に出さず、各言語で同じ結果を得る必要があるか |
| 代替 | 元のソフトや外部の変換で同じ仕事を終えられるか。変換の手間、文字・属性・座標・参照の欠落と、元のレコードを指せるかを比べる |
| 確かめた範囲 | 合成入力、第三者のファイル、受け取り側のソフトを分ける。[台帳の段](../formats/README.md#確かめた度合い)は一部の操作で得た最高段であり、ほかの版や操作に広げない |
| 将来の負担 | 本番とテストの依存、各言語の互換性、実機での照合、仕様の改定、配布サイズ、大きい入力のメモリ、削除時の移行を比べる。公開実装の権利は[台帳の手順](../formats/README.md#形式を足す)で別に確かめる |

## 業務別の判断

調査開始時の [`main`（17805e19）の台帳](https://github.com/zumen-jp/zumen/blob/17805e1936b1c035b1e1cc53449338d1f858ee34/docs/formats/README.md)は 37 項目でした。コアの 28 項目のうち、読み書きが 9、読みだけが 17、書きだけが 2 です。読む 26 項目のうち 16 項目は `synthetic` です。[v0.3.0 の台帳](https://github.com/zumen-jp/zumen/blob/v0.3.0/docs/formats/README.md)では ST-Bridge はコアに含まれません。

| 業務・形式 | 維持・投資する操作 | 根拠と次の検査 |
| --- | --- | --- |
| 国内図面の交換。SXF・SAF・JWW・DXF | SXF・JWW・DXF の読みと限定した編集・変換。SAF は図面に伴う属性を読む | 国は P21・P2Z、福島県は SFC・SFZ を指定。伊丹市には JWW と DXF の両方を求める案件がある。文字・用紙・レイヤ・添付を受け取り側で照合する。最初の候補は JWW の文字・レイヤを変更し、Jw_cad で開き直す検査。[国の CAD製図基準](https://www.cals-ed.go.jp/mg/wp-content/uploads/cad8.pdf)・[福島県のガイドライン](https://www.pref.fukushima.lg.jp/uploaded/attachment/736092.pdf)・[伊丹市の特記仕様書](https://www.city.itami.lg.jp/material/files/group/39/toltukishiyousyo_ih.pdf) |
| AutoCAD の原本。DWG | R2018 の読みと、版・操作を明示した書き戻し。R2007 の書きは要望待ち | AutoCAD 2027 の既定は R2018。書き戻しの本家での照合は 4 世代・20 ファイルの記録があり、毎晩の CI では外部の実装と比べる。[Autodesk の案内](https://blog.autodesk.io/autocad-2027-interoperability-for-customization/)・[照合した範囲](formats/dwg.md) |
| 建築・構造・指摘。IFC・EXPRESS・ST-Bridge・IDS・BCF | IFC-SPF・ifcZIP の読みと限定編集。EXPRESS は IFC の検証、ST-Bridge は構造の読み、IDS は要件の照合、BCF は指摘の読み書き | 2026 年の BIM 図面審査では IFC2x3 が指定され、実際の申請がある。IFC2x3 の属性と配置、ST-Bridge 2.1 系の構造部材、IDS の第三者の要件を優先する。[審査の指定](https://bimpermit.jp/qa/?type=3)・[申請例](https://www.obayashi.co.jp/news/detail/news20260417_1.html)・[構造ソフトの連携](https://www.building-smart.or.jp/meeting/buildall/structural-design/)・[積算の IDS の実証](https://github.com/buildingSMART-Japan/bSJ_IDS) |
| 成果の提出。電子納品管理 XML・工事写真 JPEG・ボーリング交換用データ（BED） | 管理 XML は事前のチェックと部分編集。JPEG は撮影日時などの読み、BED は位置と深さごとの記録の読み | 国のオンラインチェックは 2025 年から本運用。写真・図面・管理 XML 間の参照と版を公式のチェックと比較し、地質は既知の柱状図と比べる。[オンライン電子納品の運用](https://www.cals-ed.go.jp/notice-20250220/)・[地質を含む要領](https://www.cals-ed.go.jp/cri_point/) |
| 測量・出来形。J-LandXML・SIMA・LAS/LAZ・TIFF/GeoTIFF | 設計面と実測の座標・標高が合うことを優先。SIMA は区切り形式の限定書き出しも維持。JPGIS 版は読み、TIFF は図面の参照画像・オルソ・標高格子に絞る | 川崎市は境界の座標・画地に SIMA を指定。国土地理院が刊行する LAZ は経緯度を使い、調査時の zumen はその水平座標系を不明として扱う。測量 CAD での往復、大きい実測点群の位置とメモリを確かめる。[川崎市の要領](https://www.city.kawasaki.jp/templates/outline/cmsfiles/contents/0000029/29891/seikasakuseiryouryo20260401.pdf)・[J-LandXML 認証製品](https://ocf.or.jp/kentei/land_soft/)・[国土地理院の点群](https://www.gsi.go.jp/gazochosa/gazochosa41023.html)・[対象工事の成果品の例](https://www.maff.go.jp/j/nousin/sekkei/pdf/zidouunten3.pdf) |
| 地図。基盤地図情報・GeoJSON・CityGML・拡張 DM・ISG・PAR | 基盤地図情報の読みと GeoJSON への変換を優先。CityGML は PLATEAU の地物・属性、拡張 DM は公共測量の読みに絞る。ISG・PAR は利用者が渡す補正格子 | PLATEAU は 2025 年度末に 329 都市で整備。これは供給量の指標で、利用者数は表さない。地物の位置、測地系の世代、高さを公式の値と照らす。[PLATEAU の整備実績](https://www.mlit.go.jp/report/press/toshi03_hh_000223.html)・[都市計画の GIS データ](https://www.mlit.go.jp/toshi/tosiko/toshi_tosiko_tk_000087.html)・[基盤地図情報の更新](https://service.gsi.go.jp/kiban/app/spec_update_info/) |
| 点の取り込み。E57・PLY・XYZ・COPC | 既存の点の読みを基準とし、スキャン構造、面、列の自動推定は別に判断。COPC の範囲読みは Rust の入口と共通 API への追加を区別 | 維持と削除の比較は次の節。COPC は LAS/LAZ と処理を共有し、部分読みの共通 API には[担当 issue](https://github.com/zumen-jp/zumen/issues/899) がある。[LAS・LAZ の実装範囲](formats/las.md) |
| 表示。SVG・PNG | 複数形式から共通の 2D 表示や画像を作る出口を維持 | SVG は文字とブラウザーの表示差、PNG は出力にかかる時間と容量を確かめる。[描画の範囲](render.md) |

PDF・XLSX・CSV・JWC・JWS・電子小黒板の信憑性確認・CEDXM・BE-Bridge・Stem は調査時点でコアの対象外です。PDF のように提出需要のある形式にも、呼び出し側で扱う選択があります。[PDF と表をコアから外した変更](https://github.com/zumen-jp/zumen/commit/903a9ea0429e6b07be9c2c41525bd42282cc0412)

## 削除すると何が減るか

**製品の依存と専用の照合を減らすなら E57 が第一候補です。** 国内のスキャナー原本を直接読める価値と引き換えになります。国内 BIM/CIM 製品が LAS への事前変換を省くために E57 の直接入力を追加した例があります。点の座標・色・強度だけで終わる仕事には元のソフトなどで LAS/LAZ に変換する道があります。zumen は球座標とスキャンの姿勢も扱い、画像・格子は診断して落とします。PDAL の E57 リーダーは直交座標に限られるので、球座標を含む入力まで移行できるとは見積もれません。原本を受け取った人に事前変換を求めてよいかが、E57 を外すかどうかの分かれ目です。[国内製品が対応した理由](https://prtimes.jp/main/html/rd/p/000000043.000081365.html)・[PDAL の E57 リーダー](https://pdal.org/en/stable/stages/readers.e57.html)・[zumen の E57](formats/e57.md)

| 候補 | 実装削除で減る負担 | 失う用途と判断 |
| --- | --- | --- |
| E57 | 専用 crate の `src` 629 行、`tests` 331 行。下記の製品の依存グラフから外部 3 組が外れ、完全削除なら pye57 との夜間照合も終えられる | 受領した原本をローカルで直接開く入口を失う。スキャナー原本を扱う利用者の有無と、姿勢適用後の座標・水平と鉛直の座標系・色・強度を移行先で確かめて判断する |
| PLY | 専用 `src` 456 行、`tests` 330 行。製品専用の外部依存は 0 組。外部照合用の `ply-rs` が開発時の依存を増やす | 国内の測量アプリが LiDAR の点群を PLY で出す。頂点読みを維持し、テストの依存管理が問題なら照合だけを独立させる案を先に比べる。[mapry 測量の入出力](https://service.mapry.co.jp/surveying/)・[現在の外部照合](formats/ply.md) |
| 拡張 DM | 専用 `src` 1,384 行、`tests` 554 行と固定長レコード・旧測地系の照合。製品専用の外部依存は 0 組 | NEXCO の測量成果に用途があり、QGIS には GeoPackage に変換する手段がある。対象要素や測地系に限りがあり、元のレコードを指す機能も失う。公共測量の原本を直接読む必要が薄い場合の次点。[NEXCO の電子納品要領](https://www.ri-nexco.co.jp/Portals/0/%E9%9B%BB%E5%AD%90%E7%B4%8D%E5%93%81/250701/%E9%9B%BB%E5%AD%90%E7%B4%8D%E5%93%81_%E8%AA%BF%E6%9F%BB%E7%AD%89_0707.pdf)・[QGIS の変換プラグイン](https://plugins.qgis.org/plugins/dmconverter/) |
| XYZ・COPC | XYZ の専用 `src` は 171 行、`tests` は 39 行で、製品専用の外部依存は 0 組。COPC の範囲読みを除いても LAS/LAZ の復号は残る | XYZ は国の点群テキストの受け渡しと接点がある。zumen は形式を明示して開き、最初の 3 列を東・北・高さとして読む。COPC はブラウザーで大容量点群の一部を扱う用途と合う。追加投資は想定する入力・必要な範囲・転送量で判断する。[点群テキストの受け渡し例](https://www.mlit.go.jp/tec/content/001901915.pdf) |

### 測り方と限界

- 行数は `17805e19` の、形式を名乗る 23 crate の `src/**/*.rs` と `tests/**/*.rs` で別々に数えた物理行数です。空行・コメントを含み、同じファイルの単体テストは `src` に入ります。`src` 合計 161,638 行のうち DXF/DWG、IFC/EXPRESS、SXF/SAF、JWW の 4 crate が 125,651 行（77.7%）です。DXF/DWG の 71,986 行のうち DWG 専用は 22,533 行、DXF 専用は 4,830 行、残りは共有層などです。廃止すれば全行が減るという意味はありません。
- 依存数は [`7289f22f` のロックファイル](https://github.com/zumen-jp/zumen/blob/7289f22fa927a658635bee49a8f31cd50e59497c/Cargo.lock)に固定しました。製品では CLI・ABI を起点に通常・ビルド依存をたどり、対象の形式 crate を通る経路を除いた前後で、到達できる外部パッケージの（名前、バージョン）の集合を比較しました。再集計には、たとえば `cargo tree --locked -p zumen-cli --target x86_64-pc-windows-msvc -e normal,build --no-dedupe` を使います。開発時は `--workspace` を起点に `dev` の辺も含め、対象 crate を起点からも除きます。Windows・Linux の CLI と Windows・wasm の ABI で、E57 の差は `e57`・`roxmltree`・`memchr` の 3 組です。開発・テストを含むワークスペースでは `memchr` に別経路があり、E57 は 2 組、PLY は 30 組です。後者は照合用の依存です。対象ごとの集合差は、全環境の `Cargo.lock` から消える組数とは異なります。
- [v0.3.0 の CI](https://github.com/zumen-jp/zumen/actions/runs/44000544264/job/133763346243) で、未圧縮 wasm は全部入り 14,100,617 バイト、点群用 1,729,031 バイトでした。E57 単独の削減バイト数・CI 時間は未測定です。図面用 wasm だけを使う利用者への転送は減りません。利用時のメモリには [IFC の測定例](formats/ifc.md)があり、48,736,361 バイトの入力でピークのヒープ確保量は 199,203,082 バイトでした。

### 次に採否を見直すとき

対象形式と操作を指定した問い合わせ、採用案件、既存の変換で失う値が分かったら、この調査と照らします。削除案では、同じリリース設定の有無比較で減る容量・検査時間を測り、代表入力の変換前後を比べます。[現行のプラグイン](../templates/plugin/README.md)が受け取れるモデルには限りがあり、点群はそのまま移せません。公開済みの読みを廃止する判断が決まった段階で、形式台帳、公開 API・各言語の契約、診断 code の扱い、移行先とリリースノートをそろえます。
