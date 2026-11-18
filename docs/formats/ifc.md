# IFC

IFC-SPF と ifcZIP を、スキーマを知らないインスタンスのグラフ（L1）として読み書きする規則を記録します。実装は `crates/zumen-ifc` です。仕様の本文とスキーマは写しません（`formats/ledger.toml`）。

## 対象

- IFC-SPF（ISO 10303-21 の交換構造）。バージョン（IFC2X3・IFC4・IFC4X3 など）は問わず、知らない実体も読む。交換構造の読み取りは、SXF と共有する crate `zumen-step` が行う。
- ifcZIP。ZIP のアーカイブ（`crates/zumen-zip`、[ZIP の読み取り](../../crates/zumen-zip/src/lib.rs)）の中の 1 つの IFC-SPF を読む。
- ifcXML と ifcJSON は、まだ読まない。建物モデルへの移し方とスキーマでの検証は、別の issue で決める。

## 判定

- 先頭の行が `ISO-10303-21;` で、ヘッダーの `FILE_SCHEMA` の最初のスキーマの名前が、大文字と小文字を区別せずに `IFC` で始まれば IFC とする（`Signature::StepSchema`）。ヘッダーの文字列とコメントの中の `FILE_SCHEMA` は読まない。
- 同じ始まりの行の SXF は `ASSOCIATIVE_DRAUGHTING` のスキーマで、`FILE_SCHEMA` のない古い SXF もあるので、IFC の判定を先に行う。
- 読むときも、スキーマの名前が `IFC` で始まらないか `FILE_SCHEMA` がなければ、`ifc.schema.unsupported` で読まない。
- ifcZIP は ZIP のアーカイブとして読み、名前が `.ifc` で終わる項目（大文字と小文字を区別しない）がちょうど 1 つあるものとする。なければ `ifc.entry.missing`、2 つ以上なら `ifc.entry.unsupported` で読まない。ほかの項目は読まない。

## インスタンスのグラフ

- レコードは `zumen-step` の文で、節の語・ヘッダーの文・インスタンス・複合のインスタンスを、書いた順に持つ。レコードの間の空白とコメント、読めなかった文は、そのまま持つ。
- インスタンスの番号（`#n`）から、レコードを引く表を作る。同じ番号のインスタンスが複数あれば、初めのものを引き、`ifc.duplicate.dropped` を報告する。後のものもレコードとして残し、書くときは元のまま書く。
- 値の中の参照は、並びと型を付けた値の中も含めて、書いた順に引く。ファイルにない番号への参照は、読むときに `ifc.reference.missing` で報告する。
- 逆向きの参照（あるインスタンスを参照するインスタンス）は、求められたときに作り、インスタンスと参照の数を `work` で数える。

IFC の読み取りは `zumen_step::read_shared` を使います。元のバイト列を不変な `Arc` で共有し、データの文の構文・値の作業量・入れ子の深さを先にすべて検査します。値の配列は `Record::parts()` を初めて呼んだときに同じパーサーで作り、そのレコードが保持します。名前と参照だけを引く `first_name()`・`references()` は、値の配列を作りません。ヘッダーと形式固有のフィーチャのコメントは従来どおり値を持ちます。SXF が使う `zumen_step::read` は値を読むときに持ちます。

値の作業量は、初回の構文検査と一度だけの復元を含む単位として、読み取りで予約します。復元の作業量と深さは、その文のバイト数以下です。検査済みの不変な範囲だけを再走査するため、診断を遅らせず、読み取りの予算の使用量と停止位置を保ちます。レコードの clone も元の入力を共有し、まだ値を作っていなければ必要時に復元します。部分・値の並び、番号、範囲、等価比較、Debug の項目と書き戻すバイト列は変えません。

## L1 のメモリを測る

`l1_memory_audit` の例は、`ZUMEN_SAMPLES` 内の相対パスを引数に受け、各入力を 3 回読みます。`cargo run -p zumen-ifc --release --example l1_memory_audit -- <相対パス>...` で流します。入力は 512 MiB 以下とし、指定フォルダーの外のファイルを開きません。出力は入力番号・大きさ・時間・確保量・型の大きさ・件数だけで、パス・名前・属性値を含めません。各回に無編集の書き戻しも照合します。

`read_ns` は、ファイルをメモリに読んだ後の `Ifc::read` の時間です。`peak` と `retained` は、呼び出し側の入力バッファーを含むヒープの確保量で、OS の常駐メモリやアロケーターの管理領域は含みません。`System` の確保・解放を数え、読み終わった時点で最大と残存量を記録します。その後に値の件数と書き戻しを検査するため、その検査の確保量は含みません。

値を取り出した後は、その配列も保持します。以下の 5 倍以下の目標は L1 の読み取りまでを対象とし、全値の参照・lift・clone を含む処理全体の最大確保量ではありません。`IfcLive` への引き渡しは、未参照の文も `Record::detach_source()` で復元して元バイト列の共有を解きます。これにより、文書エンジンが元バイト列を圧縮した後に、文が未圧縮の入力を保持しません。文書エンジンの常駐量はこの測定の対象外です。

2026-11-16 に、変更前の [95bfbd18](https://github.com/zumen-jp/zumen/commit/95bfbd1816a999cb541bd267a6a2e899dc3252fa) と、必要時の復元を使う実装（[#541](https://github.com/zumen-jp/zumen/issues/541)）を比較しました。Windows 11 x64・Ryzen 5 3600XT・Rust 1.99.1 MSVC の release です。公開 12 入力の番号は `samples/ifc` 内の相対パス順です。

各実行ファイルを 6 回起動し、前→後と後→前を交互にしました。1 回の起動で各入力を 3 回読み、表の時間はそれぞれ 18 回の中央値です。計測プロセスだけを論理 CPU 10 に固定（affinity mask `1024`）、優先度を `AboveNormal` にし、端末全体の設定は変えていません。各回の最大確保量、レコード・部分・値の件数は再現し、書き戻しはすべて元のバイト列と一致しました。

| 入力番号 | 入力（バイト） | 変更前の最大（バイト） | 変更後の最大（バイト） | 変更後の倍率 | 変更前の時間（ms） | 変更後の時間（ms） |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 1,292,497 | 13,644,562 | 5,470,258 | 4.232 | 23.241 | 16.092 |
| 2 | 616,411 | 6,612,838 | 2,676,294 | 4.342 | 8.962 | 6.774 |
| 3 | 3,480,656 | 36,539,424 | 12,730,160 | 3.657 | 48.173 | 33.910 |
| 4 | 1,707,831 | 18,327,742 | 7,350,582 | 4.304 | 28.860 | 20.077 |
| 5 | 601,356 | 5,675,888 | 2,008,320 | 3.340 | 7.003 | 5.626 |
| 6 | 407,149 | 4,244,250 | 1,698,058 | 4.171 | 5.508 | 4.498 |
| 7 | 349,768 | 3,908,176 | 1,546,184 | 4.421 | 5.118 | 4.061 |
| 8 | 128,848 | 1,518,240 | 619,824 | 4.811 | 2.433 | 1.540 |
| 9 | 584,460 | 6,665,608 | 2,612,392 | 4.470 | 8.611 | 6.869 |
| 10 | 94,524 | 1,038,136 | 415,656 | 4.397 | 1.339 | 1.140 |
| 11 | 1,847,319 | 18,695,142 | 7,538,086 | 4.081 | 27.062 | 20.778 |
| 12 | 258,353 | 3,373,506 | 1,239,282 | 4.797 | 4.185 | 3.254 |
| リポジトリ外の B06 | 48,736,361 | 506,816,114 | 199,203,082 | 4.087 | 787.719 | 561.693 |

最大確保量は入力の 3.34〜4.81 倍で、5 倍以下の目標を満たしました。この条件の時間はすべて短くなりました。CPU を固定しない 18 回の予備比較では、一部の中央値が逆転し、B06 は 856.682 → 928.481 ms でした。並行作業のある端末での経過時間は負荷に左右されるため、環境一般の速度保証とはしません。確保量は CPU を固定しない比較でも同じでした。

B06 は 831,767 レコード・3,222,320 個の値です。この環境の `Record` は 80 → 88 バイト、`Part` と `Value` はそれぞれ 40 バイトのままです。値の配列を参照時まで持たず、番号の索引を作る前にレコード配列の余分な容量を返すことで減らしています。STEP の前後の比較試験は全接頭部・フィーチャ・予算停止・中断と、共有元の変更・解放を含みます。IFC・SXF の既存試験と適合テストで ID・ハッシュ・診断・編集後の書き戻しを照合します。

## 書き出したソフトごとの違い

#5 で、公開サンプルの 12 件（`samples/ifc`）と、手元の国内のソフトの書き出し 9 件（#513 の B01〜B09。公開の許諾がないので Git に入れない）を読んで見つけた違いです。国内の 9 件については、件数と種類（実体・属性の有無・書き方・値が 0 か）と、#513 に書いた書き出したソフトの名前とバージョンだけを書きます。林野庁と GNI の公開サンプルは公開元が保存し直したもので、書き出したソフトがそのまま書いたファイルとしては扱いません（`samples/README.md`）。

- **文字列。** 古い国内の書き出し（CADEWA Real2011・ARCHICAD 20 日本語版・ArchiCAD 15 日本語版）は、Shift_JIS のバイト列を `\X\hh`・`\S\c`・`\X2\00hh\X0\` で書く。ISO 10303-21 のとおりに読むと、ISO 8859-1 の文字に化ける。0x80〜0x9F の `\X\hh` は ISO 8859-1 では印字しない制御文字で、Shift_JIS では 2 バイトの文字の 1 バイト目になるので、ファイルが Shift_JIS で書かれた手がかりにする（#540）。手がかりのあるファイルにも、手がかりを持たない 8 ビットの文字列がある。半角の片仮名だけの文字列（ArchiCAD 15 日本語版で 2 件）と、CP932 の利用者定義の文字（1 バイト目が 0xF0〜0xF9）を含む文字列（CADEWA Real2011 で 66 件）です。新しい書き出しと公開サンプルは `\X2\` で書く。FILDER CeeD は、ISO 8859-1 の 8 ビットの文字を、0xA0 以上の `\X\hh` で書く。
- **精度。** 表現の文脈の `Precision` は、Revit・Archicad 28 のサンプルが 1e-5 m（mm で 0.01 か、m で 1E-05）、Civil 3D 2024 日本語版が 2.54E-8 m、Vectorworks Architect 2024 が 1E-5 mm だった。国内の 9 件は、FILDER CeeD（`$`）を除いて精度を書く。
- **開口の形。** GNI の Archicad 28 のサンプルは、開口（`IfcOpeningElement`）の形を `Body` でなく `Reference` に書く。
- **面。** 国内の書き出しの 214 要素（多くは設備の継手）に、面積のない面（点が 1 直線に並ぶ面など）がある。GNI の Archicad 28 のサンプルには、幅 100 mm の切り込みを、辺を重ねて書いた多角形の面がある。
- **室の形。** 室（`IfcSpace`）の床の輪郭は、Revit のサンプル（BIM Whale の Revit 2021、林野庁の Revit 2024 日本語版、GNI の Revit 2024・2025）が内法、GLOOBE Architect 2024 が壁芯だった。ほかのサンプルには、室がなかった。
- **建具。** どのサンプルの建具にも、2D の表現（`Plan`・`FootPrint`・`Annotation` の文脈の形）はない。開き方（`OperationType`・`PartitioningType`）は、BIM Whale・林野庁・GNI・Text2BIM（Vectorworks Architect 2024）の扉の型か扉に書かれ、国内の書き出し（CADEWA Real2011・ArchiCAD 15 日本語版・GLOOBE Architect 2024）では、ないか `NOTDEFINED` だった。
- **階と標高。** 階（`IfcBuildingStorey`）は、中野区の 5 件を除く 16 件にある。床でない基準（林野庁の `設計GL`・`最高高さ` など）も、階として書く。種類を示すプロパティはない。敷地の `RefElevation` と建物の `ElevationOfRefHeight`・`ElevationOfTerrain` は、書いてあっても多くは 0 で、0 でないのは GLOOBE Architect 2024 の建物（基準の高さと地盤）だけだった。
- **配置。** 配置はすべて `IfcLocalPlacement` で、`IfcGridPlacement`・`IfcLinearPlacement` はない。

## エスケープに書かれた文字コード

`IfcFile` と `IfcZipFile` の `set_text_encoding()` で、`TextEncoding::Auto`（既定）・`Iso8859_1`・`ShiftJis` を選びます。指定は建物・土木への lift、形の getter、`IdsLookup` と `apply()` の名前の照合で共通です。すでに作った領域モデルは変えません。Rust で書き戻したバイト列を開き直すときも、同じ指定を渡します。`IfcLive` のレコード単位の再取得・再編集への接続は担当外の残件です。

各言語の API は、スキーマのマイナーバージョン 76 から `open.ifc_text_encoding` に `auto`（省略時）・`iso_8859_1`・`shift_jis` を受けます。IFC-SPF と ifcZIP の両方に使え、`building_apply` 後の再読み込みにも指定を保ちます。IFC 以外の入力に指定すると失敗します。指定を変えて読み直すには、同じ入力を別の文書名で `open` します。

互換性の変更として、これまで ISO 8859-1 として読んでいた規格外のエスケープは、既定で CP932 に復元します。以前の文字列と要素ハッシュを必要とする利用者は、`open` に `"ifc_text_encoding":"iso_8859_1"` を加えてください。文書ハッシュと ID は変わりません。

- L1 の読み取りで、読めた文の文字列に `\X\80`〜`\X\9F` があるか一度調べ、手がかりの有無だけを保持します。この段階では文字列を展開しません。コメントと、二重の逆斜線で書かれた文字は手がかりにしません。末尾の文字列にだけ手がかりがあっても、ファイル全体の読み方を決めます。
- `Auto` は手がかりのあるファイル、`ShiftJis` はすべてのファイルで、`\X\hh`・`\S\c`・`\X2\00hh\X0\` と ASCII をバイトに戻します。SXF・JWW と同じ `encoding_rs` 0.8.42 の CP932 の表で、置換を許さずに読みます。既存の寛容なライセンスの依存を共有し、OS の文字コードとロケールを使いません。
- CP932 として読めない列、0xFF を超える Unicode の単位、生の UTF-8、`\X4\`、不正なエスケープがある文字列は、全体を従来の読み方に戻します。`Iso8859_1` も従来の ISO 8859-1/Unicode の読み方を使います。ほかの ISO 8859 のページの対応を広げる変更ではありません。
- 手がかりがない場合の `\X\B0`・`\X\D7`・`\X2\00E4\X0\` は「°」「×」「ä」です。手がかりがあると「°」も CP932 の半角片仮名へ読み替わるため、ISO 8859-1 が意図なら明示指定してください。新しい文字列の書き手は従来どおり Unicode のエスケープを使います。8 ビットの範囲だけでは意図が区別できない文字列の曖昧さは残ります。
- lift ごとに、CP932 で読める元文字列を `ifc.text.repaired` にまとめ、件数と元バイトの場所を返します。まだ要素へ移さない実体の文字列も数えます。共有先へコピーする回数では増やしません。個別の getter はファイルの指定を使いますが、このファイル全体の件数を再報告しません。
- L1 と書き戻しの元バイト、要素の ID は変えません。復元した意味の値を含む要素ハッシュは変わります。走査を Work で数え、CP932 の読み取りに必要な確保の前に DecodedBytes を数えます。文字列の判定は追加の配列や再帰を使いません。

根拠は buildingSMART の [IFC String Encoding](https://technical.buildingsmart.org/resources/ifcimplementationguidance/string-encoding/) の公開説明です。コードは ISO 10303-21 の 6.4.3 を参照箇所として示しますが、ISO 出版物・プレビューは読んでいません。規格外の CP932 の読み分けは、上記の書き出しの観測に基づく互換処理です。

合成 3 版と 3 指定で、混在、手がかりのない半角片仮名・利用者定義文字、Unicode、不正な列、元バイトと ID、復元したプロパティ名での編集、予算停止を検査します。`tests/wasi-text.mjs` で値・ハッシュ・診断・予算の記録をネイティブと WASI で比較します。API の適合テスト `ifc_text_encoding` では、既定値・明示指定・手がかりのない入力・編集後の再取得・無効な指定を照合します。公開 12 件の既存の要約は変わりません。既存の適合テストは、スキーマのバージョンと、L1 の走査を Work に数えることで変わる `civil` の予算停止位置を更新します。

2026-11-16 に、利用者から検証対象の選定を任され、#513 の旧形式 4 件を公式配布元からリポジトリ外へ取得しました。原本の SHA-256 は 2026-11-02 の調査記録と一致しました。元のファイルと属性の値は公開せず、次の件数だけを記録します。

| 対象 | CP932 で読んだ元文字列 | 復元した要素名 | 要素名に C1 制御文字を含む件数（前 → 後） |
| --- | ---: | ---: | ---: |
| B01・CADEWA Real2011 | 463 | 174 | 174 → 0 |
| B02・ARCHICAD 20 日本語版 | 880 | 116 | 116 → 0 |
| B03・ARCHICAD 20 日本語版 | 234 | 13 | 13 → 0 |
| B04・ArchiCAD 15 日本語版 | 63 | 31 | 29 → 0 |

4 件とも要素の ID を保ち、書き戻しは元バイトと一致しました。CP932 で読んだすべての文字列の意味や、IFC 全体の適合性を保証する検査ではありません。

```powershell
$env:ZUMEN_SAMPLES = 'リポジトリ外の検証用フォルダー'
cargo run -p zumen-ifc --example local_metadata_audit -- B01.ifc B02.ifc B03.ifc B04.ifc B06.ifc
cargo test -p zumen-ifc --test text_encoding -- --nocapture
```

`local_metadata_audit` は入力の順番・大きさ・件数・種類と往復の成否だけを出します。入力の名前や属性値は出しません。B01〜B04 を直下に置いた場合のテストも、件数だけを出力します。

## 土木の領域モデルへ移すとき

IFC4.3 の線形・地表面・横断を、J-LandXML と同じ土木の領域モデルに移します（`crates/zumen-ifc/src/civil.rs`、#375・#477・#478）。ここでは横断と、線形に沿った位置の規則を記録します。

- `IfcSectionedSurface` の `IfcOpenCrossProfileDef` を、線形の横断の線にする。準線（Directrix）は、線形の表現の項目にある `IfcGradientCurve` に限る。ほかの準線と、`IfcSectionedSolidHorizontal`・閉じた断面は移さず、`ifc.civil-geometry.dropped` を返す（決定 0235）。
- 断面の位置は、`IfcAxis2PlacementLinear` の `IfcPointByDistanceExpression` の DistanceAlong を、線形の追加距離にする。勾配の曲線の DistanceAlong は平面の曲線の長さで測る。位置の横・鉛直・前後のずれは仕様が禁じる（`NoOffsets`）ので、書いてあれば `ifc.attribute.invalid` を返す。Axis は鉛直だけを受け付け、RefDirection を書いた位置は `ifc.placement.unsupported` を返す。
- 断面の点は、OffsetPoint（なければ断面の原点）から始め、幅と勾配の角でたどる。角は断面の x 軸から反時計回りで、平面角の単位で読む。HorizontalWidths が真なら幅は水平の長さ、偽なら勾配に沿った長さ。真で鉛直の勾配は `ifc.attribute.invalid` を返す。
- 断面の x 軸は、準線の進む向きに対して左を向き、y 軸は上を向く（`IfcOpenCrossProfileDef` の定義）。横断の線は右を正にするので、x の符号を変える。断面の原点は準線の上の計画高なので、高さは計画高からの差（`CrossSectionHeight::FormationOffset`）にする。`IfcMapConversion` の Scale を離れに、ScaleZ を高さに掛ける。
- 線の名前は ProfileName、なければ面を持つ要素の Name。役割は計画（`SurfaceRole::Design`）。同じ追加距離の断面は 1 つの横断にまとめる。Tags は読まない。
- 1 つの断面でも移せなければ、その面の断面をどれも移さない。使わなかった `IfcOpenCrossProfileDef` は `ifc.civil-geometry.dropped` を返す。

試験（`crates/zumen-ifc/tests/civil.rs`）では、拝み勾配と斜めの幅の片勾配の 2 断面を、メートル・ラジアンとミリメートル・度の IFC4.3 で作り、同じ横断の J-LandXML から移した離れと高さに一致することを確かめます。公開原本 [IfcSectionedSurface_Superelevation.ifc](https://github.com/RickBrice/IFC-Alignment-Geometry-Implementation-Guide/blob/fd8776fc8f0a76e63cd42963a904549ab3997b62/examples/IfcSectionedSurface_Superelevation.ifc)（フィート・ラジアン、SHA-256 `f807c485…75d5`）は配布元を参照し、同梱しません。手元で読むと 6 断面を診断なしで移し、左へ曲がる円弧の片勾配で右が高くなりました。断面の左右の向きは、これと仕様の文で確かめました。外部の実装との照合はまだです（#504）。

### 線形に沿った位置

`Lift<Vec<LinearPosition>>` は、`IfcLinearPlacement` で置いた要素を、線形の上の位置として返します（#376）。単位と `IfcMapConversion` は線形と同じく読みます。建物モデルの要素の形と位置は、ここでは扱いません。

- 6 ObjectPlacement が `IfcLinearPlacement` で、1 GlobalId を書いた要素を、ファイルの順に返す。`IfcReferent` は線形の測点として読むので返さない。
- 位置は、RelativePlacement の `IfcAxis2PlacementLinear` の Location の `IfcPointByDistanceExpression` から読む。DistanceAlong を追加距離、OffsetLateral の符号を変えた値を離れ（右を正）、OffsetLongitudinal を接線に沿ったずれにする。`IfcMapConversion` の Scale を掛ける。
- BasisCurve は、線形の表現の曲線か、`IfcGradientCurve`・`IfcSegmentedReferenceCurve` の BaseCurve をたどった曲線に限る。`IfcGradientCurve` なら OffsetVertical に ScaleZ を掛けて計画高からの差（`CrossSectionHeight::FormationOffset`）、平面の `IfcCompositeCurve` なら線形の配置の高さに足した標高（`CrossSectionHeight::Elevation`）にする。`IfcSegmentedReferenceCurve`（カントで傾けた曲線）は `ifc.placement.unsupported` を返し、要素を返さない。どの線形の曲線でもなければ `ifc.attribute.invalid` を返す。
- `IfcAxis2PlacementLinear` の Axis と RefDirection（要素の向き）は移さず、`ifc.civil-geometry.dropped` を返す。位置は返す。`IfcLinearPlacement` の PlacementRelTo と CartesianPosition は読まない。

試験では、勾配の曲線と平面の曲線に置いた 2 つの信号を、メートルとミリメートルの IFC4.3 で作り、追加距離・離れ・高さ・前後のずれを確かめます。buildingSMART の公開原本 [linear-placement-of-signal.ifc](https://github.com/buildingSMART/IFC4.x-specification-models/blob/7b06d489afbcff581bbbf4fdf158196412709baf/models/alignment-geometries-and-linear-positioning/linear-placement-of-signal/linear-placement-of-signal.ifc)（CC BY-ND 4.0、SHA-256 `3c6b97f7…4c97`）は配布元を参照し、同梱しません。手元で読むと、2 つの信号の位置を `zumen_civil::place` で平面と高さに直した値が、ファイルの CartesianPosition とミリメートルまで一致しました。

## 建物モデルへ移すとき

- L1 はスキーマを仮定せずに元の文と参照を保持する。L2 は要素に意味ごとのコンポーネントを結び、持つコンポーネントだけを種類の名前とともにハッシュに入れる。正しい GlobalId の初出ではその値を ID に使う。GlobalId がない場合は `i<インスタンス番号>` にし、不正な値では `building.global-id.invalid`、後出の重複値では `building.global-id.exists` を返して、どちらも同じインスタンス番号の規則で ID を作る。値と参照の具体例は [建物の適合テスト](../../bindings/conformance/cases/building.json) にある。
- プロパティや IDS の照合で不要な形状は、必要な要素について初めて取り出す。試作で写せる型は [写す形の項目](../../bench/kernels/README.md#写す形の項目) にある。領域モデルにはこれに加え、断面の `IfcIShapeProfileDef`・`IfcRectangleHollowProfileDef`、Axis・FootPrint の `IfcGeometricCurveSet`・`IfcGeometricSet`、回転の `IfcRevolvedAreaSolid`、円断面の掃引の `IfcSweptDiskSolid` を持つ。試作の表にも上記の追加対象にもない形は、種類と出所を残し、評価時に診断する。形状とスタイルの実装条件は [#419](https://github.com/zumen-jp/zumen/issues/419)、階・区画・分類は [#418](https://github.com/zumen-jp/zumen/issues/418) にある。開口を引くときの入力条件・許容差・カーネルの選択は [3D カーネルの検証](../../bench/kernels/README.md#サンプルで要った手当て) に従う。
- 建具は書かれた 2D の表現を優先し、次に開き方とプロファイルの記号、どちらもなければ切断線と診断を返す。室の輪郭は書かれたまま残す。面積を求めるときは利用者が指定した内法・壁芯を優先し、推定に使った基準を出所に残す。室の面積の作業は [#424](https://github.com/zumen-jp/zumen/issues/424) で追う。
- スキーマを同梱せず、構文の検査は利用者の [EXPRESS](express.md)、要件の照合は [IDS](ids.md) を受け取る。

## 設備の要素と型

`Building::lift` は設備の要素と型、建築の部品、搬送の要素を、版ごとの実体の表から読みます。追加した 161 種類は [実体の表](../../crates/zumen-ifc/src/building/entities.rs) にあります。IFC2X3 の空調・制御・電気・給排水と共通の設備の領域にある型、および IFC4・IFC4.3 の対応する要素と型が対象です。実体名の接頭辞から未知の派生型を推測して写しません。その版にない実体も `ifc.entity.dropped` で知らせます。

- 要素の ID・名前・種類・配置、型の ID・名前・種類・`HasPropertySets`、`IfcRelDefinesByType` による関係と出所を既存の成分に写します。型のプロパティは型に保持します。形と材料を取得する経路も同じ実体の表を使います。設備のポートの接続は対象外です。
- IFC2X3 の設備の具体的な種類は、`IfcFlowSegment` などの一般の要素と `IfcPipeSegmentType` などの型で表すものがあります。IFC4 以降の具体的な要素を旧版の一般の要素と同一視せず、書かれたクラスと型への参照を保ちます。
- 旧版の `IfcTransportElement.OperationType` と `IfcElectricDistributionPoint.DistributionPointFunction` は種類の値として保ちます。後者の利用者が定めた種類は `UserDefinedFunction` から読みます。旧版の `IfcDiscreteAccessoryType` は `PredefinedType` を持ちません。旧版だけの `IfcElectricHeaterType`・`IfcGasTerminalType` のクラス名は変更しません。

根拠は buildingSMART が公開する各実体の自然文の属性の節です。共通する位置は [IFC2X3 PipeSegmentType](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifchvacdomain/lexical/ifcpipesegmenttype.htm)、[IFC4 PipeSegmentType](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifchvacdomain/lexical/ifcpipesegmenttype.htm)、[IFC4.3 PipeSegmentType](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcPipeSegmentType.htm)、旧版の違いは [ElectricDistributionPoint](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcelectricaldomain/lexical/ifcelectricdistributionpoint.htm)、[TransportElement](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcproductextension/lexical/ifctransportelement.htm)、[IFC4 TransportElement の変更の説明](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcproductextension/lexical/ifctransportelement.htm) を参照します。

IFC2X3 の `IfcAirTerminalType`・`IfcElectricFlowStorageDeviceType`・`IfcGasTerminalType` の自然文には、固有の属性の説明がありません。この 3 種類の種類の位置は共通の型の配置から推定し、合成入力を IfcOpenShell の実行 API に渡して確認しています。スキーマの定義や外部の実装ソースから取得した位置ではありません。自然文で位置の明記がない旧版の `UserDefinedFunction` も、同じ方法で確認しています。

### 設備の検査

[合成の検査](../../crates/zumen-ifc/tests/services.rs)では 3 版の対応する実体、ID・種類・型のプロパティ・関係・配置・材料の継承と、版違い・未知の派生型・予算の境界・中断・レコードの並び替えを確かめます。IfcOpenShell 0.9.0.post1 と要素 483 件、型との関係 69 件、型に直接付くプロパティ 233 件が一致しました。これは `synthetic` の範囲の元属性と参照の照合です。幾何の評価やファイル全体の適合性は検証していません。

```sh
ZUMEN_IFC_SERVICES_OUT=target/services-comparison cargo test -p zumen-ifc --locked --test services
python -I crates/zumen-ifc/tests/ifcopenshell/services.py target/services-comparison
export CARGO_TARGET_WASM32_WASIP1_RUNNER='node tests/wasi-services.mjs'
cargo test -p zumen-ifc --locked --test services --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-services.mjs --compare target/services-comparison target/services-comparison-wasm
```

ネイティブと WASI は入力・値・ID・ハッシュ・診断・予算の 9 ファイル、519,020 バイトが一致しました。公開 12 件も IfcOpenShell と取得した値を照らして食い違いがありません。未取得の値は比較器が別に報告します。公開サンプルの要約は GNI の `model_125.ifc` で設備の要素 18 件と型 2 件、Text2BIM で部品 6 件が増えました。これに伴い配置、型との関係、型と要素のプロパティ、部品の材料を保持し、対応する欠落の件数と要素のハッシュが変わりました。ほかの 10 件の要約と文書の値のハッシュは同じです。2026-11-16 に、利用者から対象の選定を任されたため、#513 の B06（BLCJ 公開の Rebro2025 出力）をリポジトリ外で検査しました。同じ依存と検査器で IFC 実装を f627a605 と b75f7cd8 に替えて比較し、要素は 3,750 → 6,643、`ifc.entity.dropped` は 754,428 → 745,213（9,215 件減）、変化した種類は 27 でした。前後とも元バイトの往復が一致しました。保持する対象の件数であり、形の評価や描画の成功率ではありません（[#537](https://github.com/zumen-jp/zumen/issues/537)）。

## 材料・プロパティ・関係・建具の値

`Lift<Building>` は次の値を移します（[#539](https://github.com/zumen-jp/zumen/issues/539)）。値は入力の単位のままです。既存の `properties`・`material`・`connection` の名前と意味を保ち、複合の構造には `property-details` を加えます。公開操作は Rust の `Lift<Building>` と、材料の表示を取得する `owned_geometry()` です。元の IFC を保持する L1 の読み書きと、L2 から IFC を再構築する操作は区別します。この追加は L2 の再書き出しを提供しません。

- **材料。** 層とその使用法に加え、IFC4 以降の構成と断面のセットを読みます。材料への参照、構成する項目の元の並びと名前、層の厚さ・方向・オフセットを保ちます。断面の項目で省略された材料を、空の材料に置き換えません。材料のプロパティは材料の要素に直接付けます。IFC2X3 は `IfcExtendedMaterialProperties`、IFC4 以降は `IfcMaterialProperties` を読みます。構成比、断面の幾何と優先度、断面の使用法はこの成分の取得範囲に含めません。
- **値の列。** `IfcPropertyListValue` と `IfcPropertyEnumeratedValue` は、型と順序を持つ `Literal::List` にします。省略、空の不正な列、異なる型を混ぜた列を区別します。列挙の明示単位は選択肢の定義から取り、不正な明示参照を文書の既定単位で代用しません。
- **複合・上下限・選択肢。** `property-details` はセットごとに親の位置を持つ平らな列です。親は先行する複合の項目を指し、共有する子は各親の下へ写します。名前・説明・用途・値の型、上下限と IFC4 以降の設定値、選択肢の定義と単位を保ちます。深い所有権の入れ子を作りません。元レコード番号は出所として持ち、意味のハッシュには入れません。成分がない従来の要素のハッシュは、この成分の追加では変わりません。
- **関係。** `IfcRelConnectsElements` と `IfcRelCoversBldgElements` は両側に、`IfcRelAssignsToActor` は対象から担当者へ `connection` の参照を持ちます。関係の種類と出所、集合内の元の位置を保ちます。`IfcActor` と `IfcOccupant` は名前・種類を持つ要素です。担当先の人・組織の詳細、`ActingRole`、接続の幾何はこの成分に含めません。
- **建具。** Door/Window の Lining/PanelProperties を、要素か型に直接付くプロパティセットにします。長さ、正規化比率、列挙、負のオフセット、0、省略を区別します。形の参照を長さの値として扱いません。旧版にない追加属性を読みません。

共有する予算で作業量・確保・深さを数えます。複合の循環は元の親の属性で診断し、予算停止では深さを戻して結果を返しません。未知の項目は元の種類と出所を持つ項目にし、未取得の実体を `ifc.entity.dropped` で知らせます。取得範囲の不正値・必須参照には `ifc.attribute.invalid`、ほかの成分で使った資源への移せない関係には `ifc.relation.target.unsupported` を使います。これらは IFC 全体の形式検証ではありません。

根拠は buildingSMART の自然文の属性の節です。[MaterialConstituentSet](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcMaterialConstituentSet.htm)、[MaterialProfileSet](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcMaterialProfileSet.htm)、[ComplexProperty](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcComplexProperty.htm)、[PropertyBoundedValue](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcPropertyBoundedValue.htm)、[RelAssignsToActor](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRelAssignsToActor.htm)、[DoorLiningProperties](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcsharedbldgelements/lexical/ifcdoorliningproperties.htm) を参照します。位置の番号がない IFC2X3 の属性は共通する継承の配置から推定し、合成入力と外部の実行 API で名前を照らしています。

### 換算と派生の単位

`IfcConversionBasedUnit` の `IfcMeasureWithUnit` から複数段の換算を解き、`IfcDerivedUnit` の各名前付き単位に整数の指数を適用します。元のプロパティ値は換算せず、`Unit.scale` に SI 基本単位系での大きさを持ちます。質量の基準は kg です。派生単位の名前は IFC4.3 の明示名、利用者定義ならその種類名、それ以外は元の順の単位と指数の式です。元の構成の完全な再出力には L1 を使います。

参照グラフは再帰せずに読み、共有する単位を再利用します。循環、不正な参照・整数指数、有限でない倍率、0 以下の倍率、乗算のオーバーフローとアンダーフローは採用しません。SI の不正は元の単位で一度だけ診断し、その利用側に同じ不正の診断を重ねません。取得できない単位でも値は残し、必要な単位の欠落を知らせます。平行移動を要する換算、文脈依存の単位、Dimensions と量の型の完全な整合検証、単位集合の一意性はこの処理に含めません。

根拠は [ConversionBasedUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcConversionBasedUnit.htm)、[MeasureWithUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcMeasureWithUnit.htm)、[DerivedUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcDerivedUnit.htm)、[DerivedUnitElement](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcDerivedUnitElement.htm) の自然文です。文書の既定単位へ対応させる量は実装の `DERIVED_MEASURES` の表に限定します。`IfcThermalConductivityMeasure` は W/(m K) の定義と公開サンプルの構成から `THERMALCONDUCTANCEUNIT` へ対応させる読み取り判断です。存在しない `THERMALCONDUCTIVITYUNIT` を作りません。

### 5 種類の検査

[合成入力](../../crates/zumen-ifc/tests/metadata.rs)で IFC2X3・IFC4・IFC4X3 の 5 範囲、共有・循環・不正値・予算停止・中断を検査します。複合のプロパティと換算単位は 1,000 段も検査します。IfcOpenShell 0.9.0.post1 の実行 API と元属性・参照 205 件が一致しました。倍率の比較は相対許容差 1e-14 です。スキーマや実装ソースは参照・同梱しません。

```sh
ZUMEN_IFC_METADATA_OUT=target/metadata-comparison cargo test -p zumen-ifc --locked --test metadata
python -I crates/zumen-ifc/tests/ifcopenshell/metadata.py target/metadata-comparison
export CARGO_TARGET_WASM32_WASIP1_RUNNER='node tests/wasi-metadata.mjs'
cargo test -p zumen-ifc --locked --test metadata --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-metadata.mjs --compare target/metadata-comparison target/metadata-comparison-wasm
```

ネイティブと WASI は、値・ID・ハッシュ・診断・全 Kind の予算、停止と中断を含む 9 ファイル、135,354 バイトが一致しました。公開 12 件の比較対象の値にも食い違いはありません。公開用の比較器が出力しない `property-details` の上下限や選択肢は、上の合成照合で別に確認しています。

公開サンプルの要約では、6 件の文書で派生単位が合計 57 個増えました。GNI 2 件と林野庁で材料の成分が合計 246 要素増え、GNI の列挙値 291 要素と中野区 5 件の上下限値に `property-details` が付きました。Text2BIM は担当者 3 要素が増えました。建具の値と材料のプロパティも保持するため、12 件の要素のハッシュと、単位が増えた 6 件の文書の値のハッシュが変わっています。GNI の `model_125.ifc` の熱貫流率 195 値は、対応する単位が文書にないため欠落の診断を残します。

## 共有する SI 単位の採用条件

建物モデルへの `Lift<Building>` と、既存の `curve()`・`extruded_area()` は同じ SI の読み取りを使います。文書の単位の割り当て、プロパティ・数量に明示した単位、一段の換算の基の SI を対象に、長さ以外も検査します。値は元の単位のまま保持し、単位換算は適用しません。`extrusion()` の長さ専用の採用前検査は引き続き別に行います。

- 単独は IfcSIUnit の 4 属性、複合は IfcNamedUnit の 2 属性と IfcSIUnit の 2 属性を読みます。複合の Part の順は問いません。重複・余分な Part・未知の属性配置は採用しません。SI を含む不正な配置を拒否したあとは、同じレコードの換算単位の Part としても再採用しません。SI のない正しい単独の換算単位は引き続き読みます。派生する Dimensions は `*` を要求し、参照をたどって代用しません。
- Prefix は正規の 16 列挙値か省略の `$`、Name は正規の 30 列挙値を要求します。未知値・数値・異型・小文字の列挙値を、接頭辞なしや倍率 1 として扱いません。ISO 10303-21 第 3 版 6.4.5・12.1.7 と Table 2 の交換構造の列挙は大文字です（[資料を読む条件](../../formats/README.md#資料を読む条件)のとおり、第三者のサイトにある規格の写しは開きません）。L0 の `Value::Enum` と既存の `enumeration()` は寛容な字句読みで元の綴りを保持します。この契約を変えず、SI の採用時にだけ正規の列挙と照合し、入力の綴りや係数を黙って正規化しません。
- Name から導かれる 7 次元と UnitType の次元を照合します。名前と種類の独自の一対一対応は課さず、RADIAN/STERADIAN、HERTZ/BECQUEREL、CANDELA/LUMEN、GRAY/SIEVERT、KELVIN/DEGREE_CELSIUS の同次元の組み合わせは元値を保持します。正規の `USERDEFINED` は次元照合の結果が UNKNOWN になります。FALSE の次元不整合とは区別し、配置・Name・Prefix が正規なら元値を保持します。名前や使用文脈が示す単位の種類まで検証したとは扱わず、既存の保持範囲を狭めません。
- IFC2X3 TC1 の電気容量は、公式の CorrectDimensions が質量指数 +1、DimensionsForSIUnit(FARAD) が -1 で、電流指数はともに +1 と記す不整合があります。IFC4 と IFC4.3 の両関数は（-2、-1、4、2、0、0、0）で一致します。2X3 でも FARAD の自然言語定義「electric capacitance」と整合する後者の次元で読みます。これは読み取りの判断であり、2X3 の形式的検証の実装ではありません。HENRY/INDUCTANCEUNIT は 3 版とも同じ次元です。

採用できない元の SI には `ifc.unit.invalid` と理由・元レコードのバイト範囲を返します。同じ SI を文書・複数の値・換算基から共有しても、1 回の取得内では一度だけ診断します。不正な SI の cache と成功した単位の cache を分け、成功 cache の再取得は従来どおり写す値を予算に数えます。プロパティ・数量の値は残し、単位だけを `None` にします。単位が解けない値の `ifc.unit.missing` も従来どおり返します。予算や中断で採用前に止まったことを、不正や欠落として診断しません。

正規の最大 2 Part・4 属性は固定個数の借用値の照合で、正しい既存入力の値・出所・予算使用量を保ちます。非正規の多 Part は走査前に Part 数を Work に数え、不正 SI の記録前は Work 1 と DecodedBytes 512 を確保します。追加検査の停止位置は参照する値でなく SI の元レコードです。文書の単位の検査で止まった場合は、要素の走査・確保へ進みません。

根拠は [2X3 NamedUnit.WR1](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcnamedunit.htm)・[CorrectDimensions](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifccorrectdimensions.htm)・[DimensionsForSIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcdimensionsforsiunit.htm)・[SIUnitName](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcsiunitname.htm)、[IFC4 CorrectDimensions](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcmeasureresource/lexical/ifccorrectdimensions.htm)・[DimensionsForSIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcmeasureresource/lexical/ifcdimensionsforsiunit.htm)、[IFC4.3 CorrectDimensions](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCorrectDimensions.htm)・[DimensionsForSIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcDimensionsForSIUnit.htm) です。Prefix と属性の定義は[断面の仕様参照](#断面を共通の経路で取得する)にあります。スキーマ・生成コード・第三者の実装ソースは同梱しません。

換算と派生の単位は[追加の取得範囲](#換算と派生の単位)に従います。SI の共有検査だけで、換算単位自身の Dimensions、ValueComponent の意味・種類との整合、文書の単位集合の一意性まで検証したとは扱いません。

### 共有 SI の確かめ方

`tests/units.rs` は合成 3 版で、全 SI 名・同次元の別名・全接頭辞と省略・単独/複合・換算基・成功/不正 cache・診断の元位置と件数・停止前の確保・中断を検査します。公開の `gni/model_9.ifc` を基に単位だけを自作の不正値へ変え、旧押し出しと曲線の取得でも幾何の定義を保って単位を拒否することを確認します。正しい旧入力は変更前後の値・出所・診断・全 Kind の予算と ID/Hash を比較します。

```powershell
$env:ZUMEN_IFC_UNITS_OUT = Join-Path (Get-Location) 'target/unit-comparison'
cargo test -p zumen-ifc --locked --test units
python -I crates/zumen-ifc/tests/ifcopenshell/units.py target/unit-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-units.mjs'
cargo test -p zumen-ifc --locked --test units --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-units.mjs --compare target/unit-comparison target/unit-comparison-wasm
```

WASI は JSON 4 件、正例の値・出所の記録、不正値・全 Kind の予算境界・中断の契約記録をネイティブとバイト単位で比較します。IfcOpenShell 0.9.0.post1 とは、合成 105 件と公開 2 件の元の単位名・種類・接頭辞を照合します。合成は `synthetic`、公開は `external` の範囲です。次元関数・単位換算・ファイル全体の適合性を外部照合したとは扱いません。既存の IFC fuzz が共有 Lifter と各 getter を呼び、毎晩の既存の読み取り専用 job でも export と元属性の照合を流します。

## 要素から形状表現をたどる

Rust の `IfcFile::product_shape(instance, cx)` は、建物の要素の L1 の番号から形状表現をたどり、`zumen_ifc::ProductShape` を返します。`building` の feature が必要です。`Building::lift` が知る要素・空間・通り芯の実体が対象で、型の要素（`IfcTypeProduct`）は対象外です。ifcZIP は `file.ifc().product_shape(...)` から呼べます。

既存の `instance()`・`Record::parts()` でも元の属性を読めますが、要素から共有する写像へ進むには参照の型と循環・予算の検査が必要です。この取得は L1 の Rust の経路として追加し、既存の `curve()` と組み合わせます。各言語への新しい op や Building の成分は追加しません。

```rust,ignore
use zumen_ifc::ShapeLinks;

let shape = file.product_shape(element_instance, cx)?;
for node in &shape.nodes {
    if matches!(node.links, ShapeLinks::Curve) {
        let curve = file.curve(node.source.id().unwrap(), cx)?;
        // curve は元の座標空間の定義。shape の配置や写像はまだ適用していない
    }
}
```

- `product`・`placement`・各 node の `source` は、同じ `IfcFile` の元の `Record` を借用します。元のバイト範囲・属性・STEP の参照を保ちます。`ShapeRef` はこのビュー内の `nodes` だけの参照です。STEP 番号とグラフの並びは source 内の参照であり、L2 の要素の ID や幾何の意味値には使いません。ビュー全体には意味値の等価性やハッシュを定義しません。
- `IfcProductDefinitionShape` の表現の列、`IfcShapeRepresentation` の文脈・識別子・種類・項目、`IfcMappedItem` の写像元と写像先、`IfcRepresentationMap` の原点と元の表現、`IfcGeometricSet`・`IfcGeometricCurveSet` の要素を保持します。元の列の並びを保ち、同じインスタンスの表現・写像・項目は一度だけ持ちます。
- `IfcBooleanResult`・`IfcBooleanClippingResult` は `ShapeLinks::Boolean` に、和・積・差と第 1・第 2 の対象への参照を保持します。ClippingResult は差だけを許し、参照の順序・共有・循環を既存の探索で扱います。対象の型の組み合わせと立体の妥当性は未検証です。未知の葉はその出所と診断を保持します。
- 配置、表現の文脈、写像の原点と写像先はレコードへの参照です。元の軸の比・原点・一様または非一様な尺度・省略した属性もそのまま読めます。変換・正規化・単位換算は行いません。配置の親や補助レコードの子はこの取得で展開せず、数値や次元の妥当性、幾何の一致も検査しません。曲線の値と単位は `curve()` を呼んだときに取得します。
- 型を持つ曲線の項目は `ShapeLinks::Curve` と元のレコードを返し、曲線の値は読みません。ほかの形、点・面、立体・断面、トポロジやスタイルの表現は `Unsupported` と `ifc.shape.unsupported` を返し、元の値と参照を source に残します。スタイルやレイヤの逆参照はこのビューに集めません。
- 配置・形状表現・表現の識別子と種類が `$` なら、`None` と `ifc.shape.missing` を返します。必要な参照・種類・列・文字列の構造の誤りと循環は `ifc.shape.invalid` と `Error::Invalid` にします。`reason` は `product`・`reference`・`type`・`count`・`list`・`text`・`cycle`、ブーリアンでは `attributes`・`boolean-operator` も返します。EXPRESS の全規則や SET の重複・内容の幾何的な制約を検査した結果ではありません。
- 探索と所有権は再帰しません。表現と共有写像をたどる深さを `depth`、参照・列・文字列の読み取りを `work`、項目・索引・参照・探索・文字列の確保前の予約量を `decoded-bytes` で数えます。予約量は 64 bit と wasm32 で共通です。循環・不正・予算・中断では部分結果を返さず、入った深さを戻します。

### 形状表現の確かめ方

`crates/zumen-ifc/tests/product_shapes.rs` の合成の 3 バージョンで、要素・共有写像・文脈・未適用の非一様な尺度と配置・曲線への受け渡しを確かめます。未知と省略の診断、参照と種類の誤り、循環・深さ・確保前の予算・中断・変異した入力も検査します。必要時の取得の前後で、既存の建物の ID と要素ハッシュは変わりません。

公開サンプル `gni/model_9.ifc` の柱の GlobalId `2q2CBrfQTCKPY0zxCdZygb`（`#131`）から、`#127` → `#126` → 写像 `#125` → 写像元 `#121` → 表現 `#119` → 押し出し `#114` まで、このビューでたどります。`#114` は未対応の葉として診断します。その先の断面 `#112` → 輪郭 `#111` は、既存の L1 の属性をたどり `curve()` へ渡す利用例です。立体・断面を型に写した結果や、写像を適用した輪郭の確認には数えません。

IfcOpenShell 0.9.0.post1 との照合は、合成の各版の 9 項目と補助レコード各 8 件、公開の 6 項目と補助レコード 16 件の参照・元の属性、および曲線各 1 件の点を対象にします。公開サンプルの表現と写像の照合が `external`、合成の 3 件は `synthetic` の範囲で外部の実装とも照合したものです。配置・写像の適用、単位・診断・予算、幾何の評価はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_SHAPES_OUT = Join-Path (Get-Location) 'target/shape-comparison'
cargo test -p zumen-ifc --locked --test product_shapes
python -I crates/zumen-ifc/tests/ifcopenshell/product_shapes.py target/shape-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-shapes.mjs'
cargo test -p zumen-ifc --locked --test product_shapes --target wasm32-wasip1 -- --test-threads 1
```

WASI の出力は `target/shape-comparison-wasm` に書きます。同じ 4 件の JSON で、参照と保持した属性、曲線の点、各 Kind の予算使用量と診断がネイティブと一致することを比べます。1 MiB のスタックで 3,000 項目を超える写像の read・clone・drop を行う検査だけはネイティブで流します。毎晩の CI でも上の手順で export して外部の実装と照らします。fuzz の `ifc` target は 16 要素までこの取得へ渡して共有の予算で探索します。

### 形・写像・表示・通り芯を一度に取得する

`IfcFile::element_geometry(&Element, cx)` は、この IFC から lift した Building の要素を受け取り、形・写像・表示・通り芯を `ProductGeometry` にまとめます。出所のインスタンスと、保持している元の GlobalId を照合します。L1 の番号から呼ぶ入口は `product_geometry(instance, cx)` です。取得のために Building を変更しません。

- `shape` に元の参照関係、同じ位置の `values` に `GeometryValue` を持ちます。押し出し・回転体・掃引円盤・多角形の面集合・添字付き面集合・半空間・3D の写像・曲線を、以下の個別の getter で取得します。表現の項目では `GeometryValue::Context` に座標系・精度・表示尺度を取得します。共有項目の値は一度だけ持ち、構造の `ShapeRef` で引けます。曲線内部の共有はそれぞれの `CurveGraph` に閉じます。
- `styles` は項目と、それを直接含む表現の組ごとに `item_presentation()` の結果を持ちます。同じ形を別の表現で使う場合も、レイヤの所属を混ぜません。表現に直接含まれず、ブーリアンや集合からだけ参照される項目の表示も取得します。写像の階層を越える上書きは、この参照関係を保持したまま未評価です。
- Grid では、軸・曲線・配置の出所を `grid_axes()` から `grid` に取得します。通り芯ではない場合の `None` と、通り芯の軸が未対応な場合の `Some(Unsupported)` を区別します。
- 未知の形や未対応の断面は、元の項目の種類と出所を持つ `GeometryValue::Unsupported` です。表示と通り芯の未対応は `Retrieved::Unsupported` とし、空の値・省略と同一視しません。未対応の子の種類や場所も個別の診断に残します。これは欠落を明示する取得の契約で、不正・予算・中断では全体を返しません。
- 統合した経路では、型に取得できた葉に `ifc.shape.unsupported` を出しません。従来の `product_shape()` は構造だけの取得として、値・診断・予算を変えません。統合結果の `shape` の未展開という印と、対応する `values` の取得結果を区別してください。
- 共有 `Cx` で構造の循環・深さと個別の取得を検査します。形の項目数を Elements、値と参照済みの印を確保前に 1 項目 2056 DecodedBytes、表示を使う位置の容量を 1 件 1024 DecodedBytes として予約します。個別の曲線・文字列・レイヤなどの確保と走査も同じ予算を使います。配列は平らに持ち、Rust の再帰を増やしません。

取得は形の評価ではなく、要素の配置・写像・単位換算を適用しません。L1 の借用出所と意味の値を含む全体の等価性や Hash は定義しません。所有する成分とハッシュは[形を含む lift](#所有する形とハッシュを建物に保持する)、写像間の表示は[使用位置からの選択](#共有写像の使用位置から面の表示を選ぶ)で扱います。材質からの表示の継承は [#419](https://github.com/zumen-jp/zumen/issues/419) の残件です。

`tests/geometry.rs` で合成の 3 版、共有する押し出し・写像・ブーリアン、表示の参照元、通り芯、未知・不正・循環・全 Kind の予算・中断を確かめます。公開の GNI の柱 `#131` と TallBuilding の壁 `#615` も、同じ入口から取得します。外部の実装との照合は、共有グラフの項目の種類・参照、表示とレイヤの参照、深さ・回転角・半径・写像先・半空間の真偽の代表属性です。個別 getter の全属性の照合は、以下の各節で示す範囲です。表示・配置・体積の評価とは分けます。

```powershell
$env:ZUMEN_IFC_GEOMETRY_OUT = Join-Path (Get-Location) 'target/geometry-comparison'
cargo test -p zumen-ifc --locked --test geometry
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/geometry.py target/geometry-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-geometry.mjs'
cargo test -p zumen-ifc --locked --test geometry --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-geometry.mjs --compare target/geometry-comparison target/geometry-comparison-wasm
```

5 件の JSON と、元値・出所・診断・全 Kind の記録をネイティブと WASI で比較します。fuzz は最初の 16 要素まで、統合した取得と従来の個別の getter に同じ予算を渡します。

### 所有する形とハッシュを建物に保持する

`IfcFile::lift_geometry(cx)` は、属性・配置・関係に加えて、要素の形・表示・通り芯を所有する Building を作ります。`building.geometry(element)` から `Geometry`、その `data()` から形の根・共有する項目・表示・通り芯、`materials()` から材料の表示と継承元を引けます。個別の要素だけなら `IfcFile::owned_geometry(instance, cx)` を使います。どちらも L1 の借用を持たず、元の IfcFile を破棄した後も値を使えます。

既存の `Lift<Building>` は属性だけの経路として保ちます。形を含む経路でも ID・既存の成分・その符号化は変えず、新しい `geometry` の成分を足します。Hash v1 はこの成分を持つ要素にだけ形のハッシュを加えます。編集の前提条件に使う側は、読んだときと検査するときに同じ経路を使ってください。`Components::representation`・`style` の従来の型を、別の意味の値へ読み替えることはしません。

形の構造は `GeometryLinks`、元値は `GeometryDefinition`、参照は同じ取得結果の `GeometryRef` です。共有写像・集合演算・曲線・面の共有を平らな表に保ちます。表現の文脈、断面、配置、写像、単位、表示と通り芯の値を移動して保持し、取得した値をもう一度複製しません。出所は `GeometrySource` の番号と、値の列に対応する `points`・`faces`・`units` などの列に分けます。曲線内部の出所は既存の `CurveSource` に保ちます。要素をまたぐ表の共有は行わず、各要素の取得結果に参照が閉じます。

`Geometry::new(data, cx)` は参照・循環・根からの到達性を検査し、出所を含めないハッシュを確定します。形と曲線の内部番号は最初に参照する順で付け直し、面集合の各表も同じ規則で符号化します。STEP 番号と内部配列の置き場所を変えても同じです。演算の左右、曲線の区間、輪の点、面の並びと共有関係は保持し、同じ形を描く別の定義を等価にする幾何の評価は行いません。

値は `Geometry` の中では変更できず、取得の前後や順序でハッシュは変わりません。値を変更するときは再検査して作り直し、`Building::insert_geometry` で成分を置きます。深さ・確保前の容量・探索・符号化は同じ `Cx` を使い、共有する子を展開しないため項目と参照の数に応じた作業量に収まります。不正と停止では部分結果を返しません。

未対応の項目は種類と出所を持ち、表示と通り芯の未対応は `Acquired::Unsupported` です。形を含む lift は、実際に保持した出所を使用済みとしてから欠落を報告します。未対応の定義と未使用のレコードの診断は残します。未知の定義の属性は L1 にだけ残り、形のハッシュにも含めません。元のファイル全体の同一性には文書ハッシュを使います。写像と材料からの表示の選択は後述の問い合わせで行い、形の評価は行いません。

`tests/geometry.rs` では借用した各定義との値・単位・出所の一致、既存成分と ID の一致、Hash v1 の固定値、取得前後・STEP の付け替え・内部表の並べ替え、意味値を変えたときのハッシュの変化を検査します。公開の GNI の柱と TallBuilding の壁も所有する形へ移します。外部実装との元属性の比較は、前節と個別 getter の検証範囲を使い、所有する型への移動では元値との一致を確かめます。配置・体積の評価結果とは分けます。

```powershell
$env:ZUMEN_IFC_OWNED_OUT = Join-Path (Get-Location) 'target/owned-comparison'
cargo test -p zumen-ifc --locked --test geometry
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-owned.mjs'
cargo test -p zumen-ifc --locked --test geometry --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-owned.mjs --compare target/owned-comparison target/owned-comparison-wasm
```

合成の 3 版と公開 2 例について、所有する元値・出所・ハッシュ・診断・全 Kind の境界をネイティブと WASI で比較します。fuzz は最初の 16 要素の所有する取得と、形を含む lift を通します。

### 共有写像の使用位置から面の表示を選ぶ

所有する `Geometry::surface_appearance(path, cx)` は、同じ形を使う経路を明示して面の表示を引きます。`path` は根の Definition から Representation・Mapped・Map を通り、表示する項目で終わる `GeometryRef` の列です。同じ共有形でも写像の使用位置を区別し、集合演算や曲線の内部へは降りません。

対象の項目にもっとも近い指定を選び、指定がないときだけ外側の写像へたどります。同じ項目では直接の表示とレイヤの取得結果に従います。`SurfaceAppearance` は指定なし・値と出所・未対応・レイヤの競合を区別し、未対応と競合を外側の指定へ読み替えません。上書きされた元値を削除せず、取得順やハッシュも変えません。表現と項目の所属・経路・深さ・探索を共有 Cx で検査し、追加の確保は行いません。

直接の表示とレイヤの優先関係は [IfcRepresentationItem の説明](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRepresentationItem.htm)に従います。写像の階層間で近い指定を選ぶ部分は、この自然文だけでは優先関係を確定できないため、IfcOpenShell 0.9.0.post1 の実行結果から推定した選択規則です。色の係数・反射式・面への適用と材質からの継承は、ここでは評価しません。

`tests/inheritance.rs` は二重の写像の内側・外側・葉への指定とその組み合わせ、同じ共有形の別の使用位置、未対応と競合、直接とレイヤ、誤った経路、予算と中断を検査します。合成の IFC2X3・IFC4・IFC4.3 の計 18 例を IfcOpenShell の幾何エンジンで評価し、採用した面の色と照合します。この照合は表示の選択だけで、形や体積の検証、公開の実物での一致率ではありません。

```powershell
$env:ZUMEN_IFC_INHERITANCE_OUT = Join-Path (Get-Location) 'target/inheritance-comparison'
cargo test -p zumen-ifc --locked --test inheritance
uv run --no-project --python 3.13 --no-python-downloads --with ifcopenshell==0.9.0.post1 python -I crates/zumen-ifc/tests/ifcopenshell/inheritance.py target/inheritance-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-inheritance.mjs'
cargo test -p zumen-ifc --locked --test inheritance --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-inheritance.mjs --compare target/inheritance-comparison target/inheritance-comparison-wasm
```

ネイティブと WASI は選択・元値・出所・診断・全 Kind の境界を比較します。fuzz も固定長の経路から同じ選択を通します。

### 材料の表示を文脈ごとに取得する

`IfcFile::material_presentation(instance, cx)` は、1 つの `IfcMaterial` から表示の定義への逆参照をたどります。`MaterialPresentation` に定義の名前・説明、表現ごとの識別名・種類・文脈、複数の StyledItem と面の表示を元の並びで保持します。表示のない材料は空の成功で、定義の出所は `None` です。材料や表示の定義・表現・項目・旧版の割り当て・面の表示・色の出所を、意味の値と分けます。

StyledRepresentation の文脈は幾何に限りません。一般の `IfcRepresentationContext` は識別名と種類を保持し、幾何の文脈とその子は既存の取得に従って座標系・精度・真北・表示尺度・単位を保ちます。材料の StyledItem は Item の省略を要求し、形への直接の指定を読み替えません。この取得では複数の表現を 1 つの色へまとめず、材料の構成・要素や型への割り当て・面への適用も行いません。Building への保持と、要素の表示を補う選択は次節の経路で扱います。

根拠は buildingSMART の各版の自然文の属性と使用規則、[MaterialDefinitionRepresentation](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcMaterialDefinitionRepresentation.htm)・[StyledRepresentation](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcrepresentationresource/lexical/ifcstyledrepresentation.htm)・[StyledItem](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcStyledItem.htm) です。公式ページが取得できない場合は、同じ持ち主の公式 HTML スナップショットで確認しています。スキーマを取得・同梱しません。

複数の逆参照・参照先・表示のない項目・重複した項目・属性の数と版を検査します。逆参照の所属を判定できないレイアウトは読み飛ばさず、不正とします。未知の表示が含まれる場合は未対応の種類と場所を診断し、部分結果を返しません。深さ・走査・確保は同じ Cx を使い、表現の容量を 1 件 2048、項目を 1024、幾何の文脈の Box を 512 DecodedBytes として確保前に予約します。文字列と面の表示、文脈の内部も個別に数えます。

`tests/material_styles.rs` は合成の 3 版と公開 TallBuilding の材料の元参照を検査します。IfcOpenShell 0.9.0.post1 との比較は合成 9 文脈と公開 1 文脈の定義・表現・文脈・表示の参照、名前・説明・面の向き・色と透過です。一般の文脈の値と幾何の文脈の値も Rust で検査し、幾何の文脈の全属性の外部比較は[文脈の個別の検査](#表現の座標系と表示尺度を取得する)の範囲を使います。材料から要素への適用や描画の照合ではありません。

```powershell
$env:ZUMEN_IFC_MATERIAL_OUT = Join-Path (Get-Location) 'target/material-comparison'
cargo test -p zumen-ifc --locked --test material_styles
uv run --no-project --python 3.13 --no-python-downloads --with ifcopenshell==0.9.0.post1 python -I crates/zumen-ifc/tests/ifcopenshell/material_styles.py target/material-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-material-styles.mjs'
cargo test -p zumen-ifc --locked --test material_styles --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-material-styles.mjs --compare target/material-comparison target/material-comparison-wasm
```

ネイティブと WASI で元値・出所・診断・全 Kind の境界を照合します。fuzz では最初の 16 材料まで同じ予算で取得します。

### 要素と型の材料から表示を継承する

`IfcFile::material_assignments(instance, cx)` は、要素への材料の割り当てを取得します。要素に指定がなければ型を参照し、型と結ぶ関係・割り当ての所有者・材料の選択の出所を保持します。要素の材料に表示がない場合や未対応の場合も、型の既知の色に読み替えません。複数の材料の割り当ては元の並びを保ちます。複数の型、不正な参照や集合、対象に関係する複合の実体はエラーです。根拠は [IfcRelAssociatesMaterial](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRelAssociatesMaterial.htm) の要素と型の上書きの使用規則と、各版の関係の自然文の属性です。

`owned_geometry()` と `lift_geometry()` は、この割り当てから単一材料・MaterialList・MaterialLayerSet・MaterialLayerSetUsage の材料の表示を所有します。リスト・層は順序と重複を保ち、材料の省略は `None`、表示の未対応は `Some(Unsupported)` として区別します。型・構成・層から材料までの参照の経路も保持します。厚さ・向きなどの構成の属性は既存の Building の材料成分と L1 が持ち、その拡充は #539 です。プロファイル・構成材の集合など、ここで未対応の選択は種類・出所と診断を保持します。未知の属性や参照しか読んでいない型・構成を、属性まで移したと数えません。

`Geometry::resolved_surface(path, cx)` は、根から表現・写像を通る使用位置を受けます。項目・写像・レイヤの表示を先に選び、指定がないときに材料を使います。項目の未対応・競合は材料の色に置き換えません。材料の表示は最上位の形の表現の文脈に対して選び、同じ文脈を先に使い、なければその親の文脈を使います。同じ値を持つ別の文脈や兄弟の文脈へは適用しません。同順位の表現、複数の割り当てや材料は `Ambiguous` で、元の構成から面へ割り振る処理を待ちます。複数の表示の項目・正負の面・反射式を 1 つの色へ合成しません。

文脈ごとの表示は [MaterialDefinitionRepresentation](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcMaterialDefinitionRepresentation.htm) の自然文に従います。最上位の表現を基準に、同じ文脈・親の順に選ぶのは zumen の明示した選択の規則です。仕様の引用や、外部実装との完全な一致とは扱いません。未対応の表示が材料の別の文脈にあっても、現段階の材料の取得は全体を未対応にします。

`Geometry::new_with_materials()` は材料の表示と、形の表現への結び付けを検査し、ハッシュに含めます。結び付けは `GeometryRef` で、STEP 番号だけで意味を決めません。同じ文脈の値でも結び付く表現が変わればハッシュが変わり、出所を付け替えても変わりません。材料が `None` の場合は従来の `Geometry::new()` と同じハッシュです。既存の属性だけの lift と Hash v1 の符号化は変えません。逆参照の走査、構成の各成分の取得、確保と符号化は同じ `Cx` を使い、不正と停止では部分結果を返しません。表示の問い合わせではメモリを確保しません。

`tests/material_inheritance.rs` は合成の 3 版で、上書き・文脈・写像・省略・未対応・競合・循環する不正参照・予算・中断・ハッシュを検査します。公開 TallBuilding の壁 `#615` では、割り当て `#16398` から層の使用位置 `#656`、層の集合 `#653`、層 `#651`、材料 `#638` と面の表示 `#604` までを保持します。

IfcOpenShell 0.9.0.post1 とは割り当て 33 件と、要素・型・直接の項目・写像・親の文脈・単一の層を通る面の色 21 件が一致しました。別枠の 6 件では、別の文脈の色も適用する、複数の文脈では既定の灰色になるという差がありました。zumen は別の文脈の色を適用せず、同じ文脈の指定を選びます。これは形・体積の一致の検査ではありません。外部への入力からは、未対応を調べる未使用の自作の実体を除きます。

```powershell
$env:ZUMEN_IFC_MATERIAL_INHERITANCE_OUT = Join-Path (Get-Location) 'target/material-inheritance-comparison'
cargo test -p zumen-ifc --locked --test material_inheritance
uv run --no-project --python 3.13 --no-python-downloads --with ifcopenshell==0.9.0.post1 python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/material_inheritance.py target/material-inheritance-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-material-inheritance.mjs'
cargo test -p zumen-ifc --locked --test material_inheritance --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-material-inheritance.mjs --compare target/material-inheritance-comparison target/material-inheritance-comparison-wasm
```

42 ファイルで、元値・選択・出所・ハッシュ・診断・全 Kind の境界と使用量をネイティブと WASI で比較します。fuzz も同じ予算で所有する取得から材料の表示の選択まで進みます。

### 表現の座標系と表示尺度を取得する

`IfcFile::representation_context(instance, cx)` は、幾何の文脈とその子から `ContextGeometry` を取得します。`definition.geometric` に親または直接の文脈の名前・次元・精度・座標系・真北、`definition.view` に子の名前・表示尺度・投影の種類・利用側が定めた名前を持ちます。原点と精度の長さの単位は `length_unit`、元の文脈・親・配置・点・方向・単位のレコードは `source` に分けます。

座標系は 2D と 3D を区別し、座標値と方向比を保持します。正規化・既定の軸・真北の既定値・尺度の反転・地図への変換を適用しません。宣言した次元は整数の 1〜3、座標系の原点はその配置が指定する 2D・3D の成分を読みます。宣言と配置の次元の整合を含む幾何の評価は別の段階です。精度を指定している場合は有限な値を要求し、省略はそのまま保持します。子の尺度は指定時に正の有限な値を要求し、1:100 は 0.01 のままです。

子の派生属性は `*` を確かめて親から取得します。親に別の子は認めず、自己参照・相互参照・子の鎖を `ifc.shape.invalid` にします。真北は 2 成分、`USERDEFINED` には投影名を要求します。軸の成分・属性数・方向の規則は既存の配置の取得と共有します。不正と停止では部分結果を返しません。深さは親までの最大 2 段で、終了・エラー時とも呼び出し前の深さへ戻します。共有 `Cx` で元の文字列・参照・単位の走査と確保を数えます。

根拠は buildingSMART の各版の自然文の属性と制約の説明です。幾何の文脈は [IFC2X3 TC1](https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/ifcrepresentationresource/lexical/ifcgeometricrepresentationcontext.htm)、[IFC4 ADD2 TC1](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcrepresentationresource/lexical/ifcgeometricrepresentationcontext.htm)、[IFC4.3 ADD2](https://ifc43-docs.standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcGeometricRepresentationContext.htm)、子の文脈は同じ版の [2X3](https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/ifcrepresentationresource/lexical/ifcgeometricrepresentationsubcontext.htm)・[4](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcrepresentationresource/lexical/ifcgeometricrepresentationsubcontext.htm)・[4.3](https://ifc43-docs.standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcGeometricRepresentationSubContext.htm) を参照しています。

`tests/contexts.rs` は合成の 3 版で平面・空間・親と子・全投影・省略・不正・全 Kind の境界・中断・STEP の付け替えを確かめます。公開の GNI の柱の文脈 `#24`、TallBuilding の壁の文脈 `#118` も含め、14 件の座標系・精度・真北・尺度・親子の名前・単位を IfcOpenShell の実行 API と照らします。形・座標変換・表示の評価とは分けます。

```powershell
$env:ZUMEN_IFC_CONTEXT_OUT = Join-Path (Get-Location) 'target/context-comparison'
cargo test -p zumen-ifc --locked --test contexts
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/contexts.py target/context-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-contexts.mjs'
cargo test -p zumen-ifc --locked --test contexts --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-contexts.mjs --compare target/context-comparison target/context-comparison-wasm
```

ネイティブと WASI は 5 件の JSON と、元値・出所・診断・全 Kind の境界の記録を比較します。統合取得のテストも両環境で流します。fuzz の断面の種では、共有写像の表現に子の文脈を割り当て、統合した取得から通します。

## 形の葉から押し出しと断面を取得する

`IfcFile::extruded_area(&ShapeNode, cx)` は、既存の `ProductShape` の葉から `IfcExtrudedAreaSolid`・`IfcArbitraryClosedProfileDef`・外輪の `CurveGraph` をまとめて取得します。L1 の引数の位置を利用側で読む必要はありません。同じ `IfcFile` の node を渡し、別のファイルや clone したファイルの node は `ifc.extrusion.invalid` にします。ifcZIP でも `file.ifc()` の同じ取得経路を使います。

```rust,ignore
let shape = file.product_shape(element_instance, cx)?;
for node in &shape.nodes {
    if file.kind(node.source).eq_ignore_ascii_case(b"IFCEXTRUDEDAREASOLID") {
        let area = file.extruded_area(node, cx)?;
        let profile = &area.definition.profile;
        let curve = area.boundary.item(profile.boundary);
        // 深さ・方向・配置・断面名は definition、外輪と単位は boundary、出所は source
    }
}
```

既存の `product_shape()` の結果と `ShapeLinks` は変えません。その L1 ビューの `Unsupported` と診断は、そのビューでは値を展開しないことを表します。同じ葉をこの操作で型に取得しても、先の診断を消しません。新しい境界 op、Building の成分、既存の ID・Hash v1・公開型の変更はありません。

- 値は `zumen_building::Extrusion`・`ClosedProfile`・`AxisPlacement3`、出所は `zumen_ifc::ExtrusionSource` に分けます。`ExtrudedArea` 全体には意味値の等価性やハッシュを定義しません。比較するときは定義・曲線の `kind`・単位を使い、元の Record・STEP 番号・CurveSource を含めません。`CurveRef` は同じ取得結果の `boundary` 内だけの参照です。
- 配置の原点・軸と参照方向の比、押し出す局所方向の比・深さ、断面名を元の値で保持します。軸や方向は正規化せず、要素の配置・写像・単位換算を適用しません。深さと立体の原点の単位も `boundary.length_unit` です。単位が解けない場合は `None` と `ifc.unit.missing` を返し、メートルなどを仮定しません。
- 立体の Position は IFC2X3 では必須、IFC4 以降で省略されていれば `None` にします。配置の位置は 3D の有限な数とし、軸と参照方向は同時指定か同時省略、指定時は非零・非平行であることを検査します。省略した軸や配置を既定の値で補いません。
- 断面は AREA の `IfcArbitraryClosedProfileDef` だけです。外輪の取得は既存 `curve()` に任せ、点・区間・共有・出所・単位を保持します。2D の Polyline は 4 点以上と始点・終点の一致、IndexedPolyCurve は添字の構造・接続と端点の座標の一致、完全な Circle は 2D の円の定義を検査します。点の追加や反転・近似は行いません。
- IndexedPolyCurve の区間がない場合は 4 点以上、区間がある場合は点の列が 3 点以上必要です。使わない点や重複した点も元の列のまま保持します。自己交差・面積・弧の非共線性などを評価して有効な面と認めた結果ではありません。自己交差の申告値も `CurveGraph` にそのまま残します。
- 深さは正の有限な数、押し出す方向は 3D の有限な比とし、局所 z 成分が 0 の方向を拒みます。元の配置の軸へ方向を変換してから検査することはしません。閉じていない輪、次元・参照・方向・配置・深さなどの不正は `ifc.extrusion.invalid` と `Error::Invalid` にします。`reason` は `source`・`type`・`reference`・`number`・`list`・`dimension`・`direction`・`placement`・`text`・`profile-type`・`count`・`closure`・`depth` です。曲線内部の不正は既存の曲線の診断も返します。
- ほかの立体・断面、CompositeCurve・TrimmedCurve など、この取得で閉鎖を検証しない境界は `ifc.extrusion.unsupported` と `Error::Unsupported` にします。`reason` は `solid`・`profile`・`boundary` です。未検証を閉じた面と仮定して値を返しません。元のレコードは L1 のビューから引けます。
- 立体・断面・曲線の深さ、参照・数・文字列・閉鎖の検査の手間、曲線・出所・文字列の確保前の予約量を共有の `Cx` で数えます。予約量はネイティブと wasm32 で共通です。中断・予算・不正・未対応では部分結果を返さず、入った深さを戻します。

### 押し出しの確かめ方

`crates/zumen-ifc/tests/extrusions.rs` は合成の IFC2X3・IFC4・IFC4X3 から既存の要素・共有写像の経路を使い、押し出し・閉じた断面・曲線を取得します。閉鎖・次元・方向・深さ・位置の不正、別のファイルの node、未知・循環・深さ・確保前の予算・中断と変異した入力を検査します。元の STEP 番号を変えたときの定義と出所の分離、取得前後の既存 Building の ID/hash も確認します。

公開の柱は `#131` → 表現/写像 → `#114` の葉を既存の `product_shape()` から引き、この取得で断面 `#112` と外輪 `#111` へ進みます。利用側で STEP の引数を読む箇所はありません。深さ 8.8・方向・未適用の配置・断面名・外輪の 5 点を型付きで持ちます。全立体・断面の対応、配置や写像を適用した形、メッシュや体積の検証には数えません。

IfcOpenShell 0.9.0.post1 との照合は、合成各版 2 件と公開 1 件の計 7 件です。押し出しの深さ・方向・未適用の配置、断面名と参照、外輪の定義、SI 単位の名前・接頭辞を比べます。数値は有限であることと等値を許容差なしで比較し、参照と列も厳密に比較します。公開の 1 件が `external`、合成 6 件は `synthetic` の範囲で外部の実装とも照合したものです。診断・予算、単位や配置・写像の適用、自己交差・面積・形の評価はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_EXTRUSIONS_OUT = Join-Path (Get-Location) 'target/extrusion-comparison'
cargo test -p zumen-ifc --locked --test extrusions
python -I crates/zumen-ifc/tests/ifcopenshell/extrusions.py target/extrusion-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-extrusions.mjs'
cargo test -p zumen-ifc --locked --test extrusions --target wasm32-wasip1 -- --test-threads 1
```

WASI の出力先は `target/extrusion-comparison-wasm` です。4 件の JSON で、定義と元の単位・出所・曲線・Kind 別の予算使用量がネイティブと一致します。1 MiB のスタックで 1,024 段の曲線の停止とエラーの破棄を検査するテストだけはネイティブで流します。fuzz の `ifc` target も、既存の形の葉から 16 件までこの取得へ渡します。毎晩の CI でも export と外部照合を流します。

## 断面を共通の経路で取得する

`IfcFile::extrusion(&ShapeNode, cx)` は既存の `product_shape()` の葉から、押し出しと `AreaProfileKind::ArbitraryClosed`・`ArbitraryWithVoids`・`Rectangle`・`Circle`・`IShape`・`RectangleHollow` の断面をまとめて取得します。種類ごとの独立した getter を増やさず、代表要素から同じ操作で値と出所へ進めます。

```rust,ignore
let shape = file.product_shape(element_instance, cx)?;
let result = file.extrusion(extruded_solid_node, cx)?;
match &result.definition.profile.kind {
    AreaProfileKind::Rectangle { x_dim, y_dim } => { /* 元の全幅・全高 */ }
    AreaProfileKind::Circle { radius } => { /* 元の半径 */ }
    AreaProfileKind::IShape { overall_width, flange_slope, .. } => { /* 元の寸法と単位付きの角度 */ }
    AreaProfileKind::RectangleHollow { wall_thickness, .. } => { /* 元の壁厚 */ }
    AreaProfileKind::ArbitraryClosed { boundary } => {
        let curve = result.boundary.as_ref().and_then(|g| g.item(*boundary));
    }
    AreaProfileKind::ArbitraryWithVoids { boundary, holes } => {
        // 外輪と各内輪は、同じ result.boundary の CurveGraph を参照する
    }
}
```

既存の `extruded_area()`・`ExtrudedArea`・`Extrusion`・`ClosedProfile` を保つ追加を選びました。これらは v0.2.0 の公開タグには含まれず、その後の main へ追加された API ですが、現 main の利用側も書き換えずに使えるようにします。モデルを置き換える案は既存のフィールド・戻り値・予算の互換性を壊すため採用しません。旧 getter の値・診断・予算・JSON を変えず、新 getter の予算を別に確かめます。スキーマ・ABI・ID・Hash v1・公開版数は変えません。今後の公開版への追加として扱い、リリース準備の #799 は書き換えません。

- `ProfileExtrusion` は深さ・押し出す方向の比・3D の局所配置、`AreaProfile` は元の名前・2D の局所配置・断面の定義を持ちます。矩形は中心からの全幅 `XDim`・全高 `YDim`、円は中心からの半径です。元値は有限とし、寸法・半径・深さは正を要求します。頂点や近似輪郭は生成しません。
- `ArbitraryClosed` は既存の閉じた外輪の読み取りを共有し、`boundary` に `CurveGraph` を持ちます。`ArbitraryWithVoids` は外輪・内輪を同じグラフで保持します。パラメーター付き断面の `boundary` は `None` です。曲線の共有は各グラフ内に限り、複数の取得結果を横断して統合しません。
- `length_unit` は文書の元の長さの単位です。最初の IfcProject の単位の割り当てから、単独の SI 単位と一段の正の換算を既存の単位の読み取りで取得し、寸法・深さ・配置へ適用しません。SI 単位は派生する Dimensions の記号 `*`、正規の `LENGTHUNIT` と `METRE` の組み合わせ、`$` または正規の 16 種類の Prefix を採用前に検査します。換算の基の SI 単位も同じ条件です。換算単位自身の必須の Dimensions 参照を読み、7 個の整数の指数が長さの（1、0、0、0、0、0、0）であることを確認し、元のレコードも出所へ残します。これらの不正は `ifc.extrusion.invalid` と場所を返します。長さの取得時には、それ以外の単位を評価せず、割り当てとそのレコードを出所に残します。長さの単位を扱えない場合は拒否し、割り当てがなければ単位なしと `ifc.unit.missing` を返します。
- `source` は立体・断面・配置・点・方向・読んだ単位の元のレコードを借用し、曲線内の参照は `CurveSource` が持ちます。STEP 番号・元バイト列を意味の値や要素ハッシュへ入れません。全体に `PartialEq`/`Hash` は定義せず、意味の比較には定義・曲線の値・単位を使います。
- 断面は `.AREA.` だけです。非対称 I 形・中空円など、対象外の断面を親型として読まず、種類と場所を `ifc.extrusion.unsupported` で返します。複合インスタンスや、対象の断面・配置・単位に未解釈の追加属性がある場合も成功扱いにしません。元の L1 レコードは保持します。穴付きの任意断面の不正と未対応は、[穴付き断面の取得](#外輪と内輪を同じ曲線グラフで取得する)の診断に従います。
- IFC2X3 では断面と立体の Position が必須、IFC4 以降では省略可能です。省略は `None` とし、既定の原点や軸を補いません。2D の Location・RefDirection は 2 成分を要求し、方向比を正規化しません。IFC2X3 の `IfcDirection` 自体と継承元に非ゼロ比の WHERE はなく、新しい断面の 2D 配置では `[0, 0]` も元比として保持します。IFC4 以降の `MagnitudeGreaterZero` では零比を拒否します。この版差で旧曲線・旧押し出しの判定は変更しません。派生する軸の `IfcBuild2Axes`/`IfcNormalise` は評価しません。
- 固定の参照経路と曲線の深さ、単位探索の全レコード・追加する次元の参照と 7 指数・属性の列・数・文字の Work、文字・出所・単位・結果の確保前の DecodedBytes を共有の `Cx` で数えます。走査の停止診断は走査中のレコードを指します。停止・不正・未対応では部分結果を返さず、呼び出し側の深さを戻します。

配置・共有写像・単位換算・輪郭や立体の評価、Building の全面統合と形の意味値を含む要素ハッシュは #419・#420 の残件です。

共有する既存の Lifter と旧 `extruded_area()` の SI の意味検査は[共有 SI の採用条件](#共有する-si-単位の採用条件)に従います。本取得の長さ・平面角の採用前検査と、共有経路の正しい旧入力の値・出所・予算の互換性の確認を分けて扱います。

公式仕様は [IFC2X3 ParameterizedProfileDef](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcprofileresource/lexical/ifcparameterizedprofiledef.htm)、[Rectangle](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcprofileresource/lexical/ifcrectangleprofiledef.htm)、[Circle](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcprofileresource/lexical/ifccircleprofiledef.htm)、[IFC4 ParameterizedProfileDef](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcprofileresource/lexical/ifcparameterizedprofiledef.htm)、[IFC4.3 ParameterizedProfileDef](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcParameterizedProfileDef.htm)です。方向の版差は [IFC2X3 Direction](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometryresource/lexical/ifcdirection.htm)・[Axis2Placement2D](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometryresource/lexical/ifcaxis2placement2d.htm)、[IFC4 Direction](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometryresource/lexical/ifcdirection.htm)、[IFC4.3 Direction](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcDirection.htm)です。単位は [IFC2X3 SIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcsiunit.htm)・[Prefix](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcmeasureresource/lexical/ifcsiprefix.htm)、[IFC4 SIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcmeasureresource/lexical/ifcsiunit.htm)・[Prefix](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcmeasureresource/lexical/ifcsiprefix.htm)、[IFC4.3 SIUnit](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSIUnit.htm)・[Prefix](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSIPrefix.htm) と [NamedUnit.WR1](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcNamedUnit.htm)・[CorrectDimensions](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCorrectDimensions.htm) に従います。仕様のスキーマ・生成コードは同梱しません。

### 断面の共通の取得の確かめ方

`tests/profiles.rs` は合成 3 版の要素・共有写像から各 3 断面を取得し、版差・元値・参照・未知/追加属性・不正・循環・確保前予算・中断・停止位置・呼び出し側の深さ・既存 ID/Hash を検査します。公開サンプルは `gni/model_9.ifc` の床の GlobalId `1soz90sYzCp8a2Do0djRBF`（`#1163` → `#1137` → 矩形 `#1134`）、`gni/model_18.ifc` の家具 `0pTNkIYt5EyfHBAi6yiASg`（`#21873` → `#21764` → 円 `#21761`）と既存の柱（`#131` → `#114` → 任意断面 `#112`）です。利用側の STEP 引数読みなしで元寸法・2D 配置・名前・単位と出所へ到達します。同梱の 12 サンプルには I 形・中空矩形の実例を見つけていません。穴付き断面は TallBuilding の窓などにあり、[穴付き断面の取得](#外輪と内輪を同じ曲線グラフで取得する)で照合します。

IfcOpenShell 0.9.0.post1 との属性比較は合成 9 件と公開 3 件です。要素の表現からの到達、断面の種類・名前・元寸法・元配置、押し出しの方向・深さ・元配置、外輪の定義、SI 単位と一段の換算の係数・接頭辞を許容差なしで比べます。公開 3 件は `external`、合成 9 件は `synthetic` の範囲です。予算・診断と出所の列、単位換算・配置や写像を適用した形はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_PROFILES_OUT = Join-Path (Get-Location) 'target/profile-comparison'
cargo test -p zumen-ifc --locked --test profiles
python -I crates/zumen-ifc/tests/ifcopenshell/profiles.py target/profile-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-profiles.mjs'
cargo test -p zumen-ifc --locked --test profiles --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-profiles.mjs --compare target/profile-comparison target/profile-comparison-wasm
```

WASI の出力先は `target/profile-comparison-wasm` です。6 件の JSON と、全 Kind の予算・中断・版差・走査の各停止位置の診断を記録する `contract.txt` をバイト単位で比べます。既存の fuzz の `ifc` target も形の葉からこの取得を呼び、既存の読み取り専用の毎晩の job で export と IfcOpenShell 比較を流します。

### I 形・中空矩形の元属性

`IShape` は対称な I 形の全幅・全高・ウェブ厚・フランジ厚と、任意のフィレット半径を保持します。IFC4 以降はフランジ端の半径と傾きも持ちます。IFC2X3 の属性数は 8、IFC4 以降は 10 です。ウェブ厚は全幅未満、フランジ厚の 2 倍は全高未満を要求します。傾きは有限の元値と `Measured::unit` に保持し、ラジアンへ換算しません。傾きがあるときだけ文書の平面角の単位も読みます。SI では `PLANEANGLEUNIT`・`RADIAN`、一段の換算では 7 個の次元指数がすべて 0 であることも確かめます。単位がなければ値を保って `ifc.unit.missing`、壊れた換算や重複した割り当てには場所付きのエラーを返します。

`RectangleHollow` は外形の全幅・全高・壁厚と、内外の任意のフィレット半径を保持します。3 版とも属性数は 8 です。壁厚の 2 倍は全幅・全高未満を要求します。半幅・半高を先に丸めず、非正規化数の寸法でも厚さと比較します。両断面の寸法と厚さは正の有限値、フィレット半径は IFC2X3 では正、IFC4 以降では 0 以上の有限値とします。省略は `None` のまま保持し、既定値を補いません。丸みと傾斜を組み合わせた輪郭の収まり、自己交差、面積や立体は未評価です。

`AreaProfileKind` の 2 変種は公開タグ v0.2.0 より後の API への追加です。旧 3 断面の値・出所・予算を保ち、追加した形の確保前予約だけを別に数えます。新しい断面も共有写像を適用せず、既存の要素 ID・Hash v1 には統合しません。

参照した説明は [IFC2X3 IShape](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcprofileresource/lexical/ifcishapeprofiledef.htm)・[RectangleHollow](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcprofileresource/lexical/ifcrectanglehollowprofiledef.htm)、[IFC4 IShape](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcprofileresource/lexical/ifcishapeprofiledef.htm)・[RectangleHollow](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcprofileresource/lexical/ifcrectanglehollowprofiledef.htm)、[IFC4.3 IShape](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcIShapeProfileDef.htm)・[RectangleHollow](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRectangleHollowProfileDef.htm) の属性と寸法の制約です。公開された自然言語の説明だけを読み、EXPRESS 本体・スキーマ・外部実装のソースは読まず、同梱しません。

`tests/sections.rs` は自作の合成 3 版で共有写像から取得し、元値・版差・単位・参照・不正・予算・中断・深さ・旧 ID/Hash を検査します。切った入力と変異した入力も共有予算で処理し、fuzz の種に両断面と傾きの単位を加えます。IfcOpenShell 0.9.0.post1 と合計 6 件の元属性を比べ、ネイティブと WASI では値・診断・全 Kind の使用量をバイト列で比べます。この 2 断面は `synthetic` の検証範囲です。公開実例での形・面積・体積の照合とは分けます。

```powershell
$env:ZUMEN_IFC_SECTIONS_OUT = Join-Path (Get-Location) 'target/section-comparison'
cargo test -p zumen-ifc --locked --test sections
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/sections.py target/section-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-sections.mjs'
cargo test -p zumen-ifc --locked --test sections --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-sections.mjs --compare target/section-comparison target/section-comparison-wasm
```

### 外輪と内輪を同じ曲線グラフで取得する

`extrusion()` と `revolved_area()` は `IfcArbitraryProfileDefWithVoids` を `AreaProfileKind::ArbitraryWithVoids` として取得します。外輪と 1 つ以上の内輪を、同じ `CurveGraph` 内の `CurveRef` で参照します。元の内輪の並び・名前・向き・長さと角度の単位・出所を保持し、点の追加や反転、穴の削除、輪郭の近似はしません。

- 曲線の探索を、外輪の後に内輪を読む形へ拡張しました。1 つの索引で共有する基の曲線を一度だけ持ち、外輪から各内輪へ初めて訪れた順に並べます。所有権と探索は再帰しません。旧 `curve()` の 1 つの根の探索と予算は変えません。
- 断面は 4 属性と `.AREA.` を要求し、内輪の空列、同じ参照の重複、外輪と同じ参照を拒否します。曲線と補助レコードの単純な形式・2D を検査し、Polyline・IndexedPolyCurve の始終点、Circle の完全な円を既存の閉鎖の検査で確かめます。
- CompositeCurve は全区間の Transition に不連続がないことを確認します。最後の区間の接続の申告を閉鎖と扱い、Line/Circle を切った曲線などを元のまま保持します。端点・接線が宣言どおりに接続するかは未評価です。ほかの種類では、閉鎖を検査できなければ診断で知らせます。
- 外輪が内輪を囲むこと、輪同士が交差・接触しないこと、内輪が別の内輪を囲まないこと、自己交差と面積は未評価です。輪の向きと、内輪という役割を分けて保持します。形を評価した結果は #420 で検査します。
- 内輪の列と重複検査の索引を確保前に予約し、個数・参照・曲線・数・文字・単位・深さ・中断を共有 `Cx` で数えます。不正と未対応は `ifc.shape.invalid`・`ifc.shape.unsupported`、曲線内は既存の曲線の診断と場所を返し、部分結果を返しません。旧 `extruded_area()` の戻り値と対象は変えません。

根拠は buildingSMART の [IFC2X3 ArbitraryProfileDefWithVoids](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcprofileresource/lexical/ifcarbitraryprofiledefwithvoids.htm)、[IFC4](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcprofileresource/lexical/ifcarbitraryprofiledefwithvoids.htm)、[IFC4.3](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcArbitraryProfileDefWithVoids.htm) の本文・属性・Informal Propositions と、[CompositeCurve.ClosedCurve](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCompositeCurve.htm) の自然言語の説明です。EXPRESS 本体は読みません。

検査は `cargo test -p zumen-ifc --test void_profiles`。`ZUMEN_IFC_VOID_PROFILES_OUT` を絶対パスに指定すると、合成 3 版と公開例の JSON 4 件、自作の IFC、元値・出所・診断・全 Kind の記録を出します。合成例は、円と折れ線の内輪、離れた 2 内輪が同じ基の直線を共有する合成曲線を、押し出しと回転体で取得します。公開例は TallBuilding の窓 GlobalId `0ZBert7zf3GhThbO52NKQt` (#13257) から、押し出し #13207・断面 #13202 の外輪と 1 内輪を取得します。従来の「公開サンプルに穴付き断面を見つけていない」という記述は、この例に基づいて訂正しました。

2026-11-16 に追加 8 テストをネイティブと WASI で実行し、JSON と全 Kind の記録 230,947 バイトが一致しました。別に IfcOpenShell 0.9.0.post1 の実行ファイルだけで、13 立体・25 内輪・122 曲線レコードの元属性を照合し、差異は 0 でした。同じグラフを押し出しと回転体から取得した分も数えています。変換済みの形や体積の一致は、この照合には含めません。旧曲線経路を使う半空間・ブーリアンの元値・出所・診断・予算 234,501 バイトは変更前と一致しました。

## 形の葉から多角形の面集合を取得する

`IfcFile::faceted_faces(&ShapeNode, cx)` は `product_shape()` の葉から `IfcFacetedBrep`・`IfcShellBasedSurfaceModel`・`IfcFaceBasedSurfaceModel` を取得します。`FacetedFaces::definition` の `FaceGraph` は、面集合・面・境界・ループ・3D の点を別々の表に持ちます。同じインスタンスは各表に一度だけ写し、元の共有を保ちます。各表は最初に参照した順です。借用する `source` の同名の列から元のレコードへ戻れます。

- 点座標・点の順・境界の `Orientation`・`FaceOuterBound` の指定を保持します。点の列を反転せず、閉じる点も足しません。外輪の明示がない面でも先頭を外輪と決めません。開殻・閉殻・指定なしはそれぞれ `Some(false)`・`Some(true)`・`None` で区別します。これらは元の申告で、幾何の検証結果ではありません。
- 単純なインスタンスと既知の属性数、参照の種類、3 成分の有限な座標、3 点以上のループ、点の重複、空の集合、集合内の参照の重複、複数の外輪の指定を検査します。面の境界や殻の参照が循環する入力は、固定の型の経路に合わないものとして拒否します。未対応の曲面・辺のループ・穴付き B-rep・添字の面を親型として読みません。不正は `ifc.shape.invalid` に理由と場所を、未対応は `ifc.shape.unsupported` に種類と場所を残します。
- 長さの単位は押し出しと同じ採用条件で取得し、点へ適用しません。単位がなければ `None` と `ifc.unit.missing` を返します。配置・共有写像・三角形化・平面性・自己交差・穴の内包・面の接続・閉殻の有効性・体積は未評価です。
- 同じ `Cx` で新しい項目の Elements、参照・列・数・単位探索の Work、表・索引・出所の確保前の DecodedBytes、最大 6 段の型の経路の Depth、中断を検査します。停止・不正・未対応で部分結果を返さず、呼び出し側の深さを戻します。既存の Building・ID・Hash v1 は維持し、全面統合と形の意味値のハッシュは #419、形の評価は #420 に残します。

根拠は buildingSMART の公開の自然言語の説明と属性です。[IFC2X3 FacetedBrep](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometricmodelresource/lexical/ifcfacetedbrep.htm)・[Face](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifctopologyresource/lexical/ifcface.htm)・[PolyLoop](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifctopologyresource/lexical/ifcpolyloop.htm)、[IFC4 ShellBasedSurfaceModel](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometricmodelresource/lexical/ifcshellbasedsurfacemodel.htm)・[FaceBasedSurfaceModel](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometricmodelresource/lexical/ifcfacebasedsurfacemodel.htm)、[IFC4.3 Face](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcFace.htm)・[FaceBound](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcFaceBound.htm)・[PolyLoop](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcPolyLoop.htm) に従います。仕様のスキーマや生成物は同梱しません。

`tests/faces.rs` は合成 3 版の共有写像から 3 種類の面集合を取得します。公開例は TallBuilding の建具 `0ZBert7zf3GhThbO52NKFT` の `#10949` → `#10896` と、Text2BIM の建具 `13v5xDIG0HxwZGA6irHSbP` の `#263` → `#260` です。IfcOpenShell 0.9.0.post1 と計 11 個の形・67 面の元属性を照合します。変換後の形や体積の比較とは分けます。ネイティブと WASI では JSON 5 件と診断・全 Kind の使用量をバイト列で比べます。fuzz の `ifc` target は 16 件まで同じ取得へ渡し、合成の種でも到達を検査します。

```powershell
$env:ZUMEN_IFC_FACES_OUT = Join-Path (Get-Location) 'target/face-comparison'
cargo test -p zumen-ifc --locked --test faces
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/faces.py target/face-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-faces.mjs'
cargo test -p zumen-ifc --locked --test faces --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-faces.mjs --compare target/face-comparison target/face-comparison-wasm
```

## 形の葉から添字付きの面集合を取得する

`IfcFile::tessellated_faces(&ShapeNode, cx)` は IFC4 以降の `IfcTriangulatedFaceSet` と `IfcPolygonalFaceSet` を取得します。`IndexedFaceSet` に元の点・任意のタグ・点の対応表・閉殻の申告を、`IndexedFaces` に三角形の添字と法線、または共有する多角形の外輪と内輪を持ちます。三角形化や単位換算は行いません。

- 添字は元と同じ 1 から始まります。`point_index` は `PnIndex` の元値で、面の添字から座標の添字への対応です。省略時は座標を直接指します。法線は対応表を通さず、面の添字で直接引きます。双方を別に範囲検査し、対応表の展開や法線の正規化はしません。参照されていない点や法線も保持します。
- 多角形の面の共有を保ち、`IndexedFaces::Polygons::indices` は 0 から始まる `faces` の参照として元の並びを持ちます。`faces` は最初に参照した順です。`source.faces` はその順の元のレコード、`source.coordinates` は点とタグの元のレコードを借用します。外輪・内輪の添字の並びは変えません。
- IFC4 の点の列は 1 属性、IFC4.3 は任意の `TagList` を含む 2 属性として検査します。タグを指定した場合は点と同じ個数を要求します。タグは既存の文字列の読み取りを使い、置換があれば `ifc.text.invalid` を返して元のレコードも残します。`Closed` の省略は `None` とし、未知を閉殻と決めません。
- 3 成分の有限値、正の整数の添字とその範囲、三角形の 3 添字、多角形と内輪の 3 点以上、属性数、版と参照の種類を検査します。縮退・平面性・内輪の交差や内包・法線の向き・閉殻の有効性・体積は未評価です。対応外の派生型や複合インスタンスを親型として読まず、種類と場所を診断します。
- 長さの単位の採用条件はほかの面集合と同じです。共有 `Cx` で参照・数・タグ・列・単位探索の Work、確保前の DecodedBytes、レコードごとの Elements、最大 2 段の取得の Depth と中断を検査します。不正・未対応・停止で部分結果を返さず、呼び出し側の深さを戻します。Building・ID・Hash v1 と形の評価の範囲は変更しません。

根拠は buildingSMART の [IFC4 TriangulatedFaceSet](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometricmodelresource/lexical/ifctriangulatedfaceset.htm)・[PolygonalFaceSet](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometricmodelresource/lexical/ifcpolygonalfaceset.htm)、[IFC4.3 TriangulatedFaceSet](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcTriangulatedFaceSet.htm)・[CartesianPointList3D](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCartesianPointList3D.htm)・[IndexedPolygonalFace](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcIndexedPolygonalFace.htm)・[IndexedPolygonalFaceWithVoids](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcIndexedPolygonalFaceWithVoids.htm) の自然言語と属性の説明です。仕様のスキーマや生成物は同梱しません。

`tests/indexed_faces.rs` は合成 2 版と公開の GNI `model_9.ifc` の壁 `2q2CBrfQTCKPY0zxCdZwK$` の `#628` → `#623` を検査します。公開例は 28 点・14 面で、最初の面 `#608` に 2 個の内輪があります。IfcOpenShell 0.9.0.post1 と計 7 個の形・共有を除く 24 面の元属性を照合します。同梱の公開 IFC に三角形の面集合はなく、三角形・法線・タグの読みは `synthetic` の範囲です。ネイティブと WASI では JSON 3 件と診断・全 Kind の使用量をバイト列で比べます。既存の多角形の面集合も追加前の出力と比べます。fuzz は 16 件まで取得し、合成の種で三角形と穴付き多角形へ到達することを検査します。

```powershell
$env:ZUMEN_IFC_INDEXED_FACES_OUT = Join-Path (Get-Location) 'target/indexed-face-comparison'
cargo test -p zumen-ifc --locked --test indexed_faces
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/indexed_faces.py target/indexed-face-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-indexed-faces.mjs'
cargo test -p zumen-ifc --locked --test indexed_faces --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-indexed-faces.mjs --compare target/indexed-face-comparison target/indexed-face-comparison-wasm
```

## 共有写像の 3D の変換を取得する

`IfcFile::mapped_shape3(&ShapeNode, cx)` は `IfcMappedItem` から、写像元の基準位置と写像先の変換 `ShapeMapping3`・長さの単位・借用出所を取得します。IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の `IfcCartesianTransformationOperator3D` と `IfcCartesianTransformationOperator3DnonUniform` を扱います。2D の写像は元の種類と場所を示して未対応とします。

- 写像元の `AxisPlacement3` と、写像先の `LocalOrigin`・`Axis1`・`Axis2`・`Axis3`・尺度を別々に持ちます。元の軸の比・負の 0・省略を保ち、基底軸の構築・正規化・鏡映の判定・行列への変換をしません。
- 一様な尺度と 3 軸の非一様な尺度を分けます。どの尺度も指定時は正の有限値を要求し、省略は `None` のまま返します。x の尺度の省略時は 1、非一様な y と z の尺度の省略時は x の尺度が規定されますが、この取得では補いません。
- 写像先の点と方向比は 3 成分を要求します。IFC2X3 の `IfcDirection` の零比は元値として保持し、IFC4 以降は `MagnitudeGreaterZero` に従って拒否します。零比の基底軸への変換や、元の軸同士の整合性は未評価です。写像元の配置は既存の 3D 配置の検査を使います。
- 写像元の表現は元のレコードを借用し、形を複製しません。同じ写像元を使う複数の項目から、同じ表現の出所へ到達できます。入れ子の写像もそれぞれの配置・変換の元値を返します。写像の循環・探索の深さは `product_shape()` で検査し、この取得では写像元の表現の中身をたどりません。
- 単位は共通の長さの読み取りを使い、欠ければ `None` と `ifc.unit.missing` を返します。複合インスタンスは未対応、読み取る属性の数・参照・数・尺度の不正は `ifc.shape.invalid` と場所を返します。項目・写像元・変換を Elements、参照・数・単位・確保前の量と深さを共有 `Cx` で数え、中断・停止で部分結果を返しません。
- `MappingSource` は項目・写像元・元の表現・変換と、読んだ配置・点・方向・単位を借用します。出所・STEP 番号は意味の値に含めません。既存の Building・要素 ID・Hash v1 は変更しません。形との統合、変換の適用、意味値ハッシュは #419・#420 の残件です。

根拠は buildingSMART の [IFC2X3 TransformationOperator3D](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometryresource/lexical/ifccartesiantransformationoperator3d.htm)・[非一様な尺度](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometryresource/lexical/ifccartesiantransformationoperator3dnonuniform.htm)、[IFC4 TransformationOperator3D](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometryresource/lexical/ifccartesiantransformationoperator3d.htm)、[IFC4.3 TransformationOperator3D](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCartesianTransformationOperator3D.htm)・[非一様な尺度](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcCartesianTransformationOperator3DnonUniform.htm)・[MappedItem](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcMappedItem.htm)・[RepresentationMap](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRepresentationMap.htm) の自然言語の本文と属性の説明です。EXPRESS 本体は読みません。

検査は `cargo test -p zumen-ifc --test mappings`。`ZUMEN_IFC_MAPPINGS_OUT` を絶対パスに指定すると、合成 3 版と公開の GNI の柱の JSON 4 件、自作の IFC、元値・出所・診断・全 Kind の記録を出します。公開例は GlobalId `2q2CBrfQTCKPY0zxCdZygb` の柱 #131 の項目 #125 から、写像元 #121・表現 #119・変換 #124 を取得します。幾何の評価や体積の一致は、この元属性の照合と分けます。

2026-11-16 に追加 9 テストをネイティブと WASI で実行し、JSON と全 Kind の記録 200,305 バイトが一致しました。別に IfcOpenShell 0.9.0.post1 の実行ファイルだけで、合成と公開例の写像 16 個の元属性を比較し、差異は 0 でした。既存の半空間・ブーリアンの元値・出所・診断・予算 234,501 バイトは追加前と一致しました。

## ブーリアンをたどり半空間を取得する

`IfcFile::half_space(&ShapeNode, cx)` は `IfcHalfSpaceSolid`・`IfcBoxedHalfSpace`・`IfcPolygonalBoundedHalfSpace` から、平面の半空間 `PlaneHalfSpace`・境界の曲線・長さの単位・借用出所を返します。`product_shape()` のブーリアンの参照から、切り取る半空間の葉にも到達できます。IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 が対象です。

- 基準面の元の配置と `AgreementFlag` を保持します。true は法線と逆側、false は法線が向く側を内部とします。元の軸の比、負の 0、軸の省略を保持し、法線を正規化・反転しません。平面以外の基準面は、種類と場所を示して未対応とします。
- 箱は隅と正の有限な 3 方向の寸法、`EnclosureCoordinates` を持ちます。IFC2X3 では基準面の座標系、IFC4 以降では要素の座標系にあります。この版差を意味の値に残します。箱は無限の半空間を計算で扱うための囲みであり、この取得では半空間との積や有限な代用立体を生成しません。
- 多角形による制限は、基準面とは別の 3D 配置と 2D の境界の `CurveGraph` を持ちます。Polyline・CompositeCurve を扱い、IFC4.3 では IndexedPolyCurve も扱います。合成の区間は Polyline または Line/Circle を切った曲線です。曲線の元値・向き・区間の共有を保持し、閉じる点を補いません。
- Polyline と IndexedPolyCurve は始終点の一致を確認します。合成の区間同士の接続・閉鎖、自己交差、基準面と境界を押し出す向きの関係は未評価です。これらの幾何の条件をすべて検証した結果ではありません。
- 半空間・基準面・囲み箱・3D 配置の属性数、有限な 3D の点、非ゼロかつ平行でない配置の軸、Boolean の側の指定を検査します。複合インスタンスは未対応です。共通の長さの単位の取得を使い、単位がなければ元値と `None`・`ifc.unit.missing` を返します。配置・写像・単位換算は未適用です。
- 不正は `ifc.shape.invalid`、未対応は `ifc.shape.unsupported` と元の場所を返します。境界内の不正には既存の曲線の診断を使います。半空間・基準面・囲み箱を Elements、曲線とともに深さ、参照・属性・数・単位・確保前の量を共有 `Cx` で数えます。停止時に部分結果を返さず、深さを戻します。
- `HalfSpaceSource` は半空間・基準面と、読んだ配置・点・方向・囲み箱・単位を借用します。STEP 番号・元バイト列は意味の値に含めません。Building の成分、要素の ID と Hash v1 は変更しません。立体の評価・全面統合・意味値ハッシュは #419・#420 の残件です。

根拠は buildingSMART の公開の自然言語の説明です。[IFC2X3 HalfSpaceSolid](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometricmodelresource/lexical/ifchalfspacesolid.htm)・[BoxedHalfSpace](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometricmodelresource/lexical/ifcboxedhalfspace.htm)・[PolygonalBoundedHalfSpace](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometricmodelresource/lexical/ifcpolygonalboundedhalfspace.htm)、[IFC4 BoxedHalfSpace の版差](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometricmodelresource/lexical/ifcboxedhalfspace.htm)、[IFC4.3 HalfSpaceSolid](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcHalfSpaceSolid.htm)・[PolygonalBoundedHalfSpace](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcPolygonalBoundedHalfSpace.htm)・[BooleanResult](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcBooleanResult.htm)・[BooleanClippingResult](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcBooleanClippingResult.htm) の本文・属性の説明を使い、EXPRESS 本体は読みません。

検査は `cargo test -p zumen-ifc --test half_spaces`。`ZUMEN_IFC_HALF_SPACES_OUT` を絶対パスに指定すると、合成 3 版と公開の TallBuilding の壁の JSON 4 件、診断・予算・元値・参照の記録、自作の IFC を出します。公開例は GlobalId `0ZBert7zf3GhThdO12NKAr` の壁 #615 から、差の参照を通って #600 と #596 の半空間を取得します。

2026-11-16 に追加 9 テストをネイティブと WASI で実行し、JSON と全 Kind の記録 234,501 バイトが一致しました。別に IfcOpenShell 0.9.0.post1 の実行ファイルだけを使い、半空間 18 個・境界の曲線 17 レコードの座標と参照・ブーリアン 11 個の演算と参照を比較し、差異は 0 でした。囲み箱の版差は公開仕様と合成入力で検査し、外部実装による座標変換・体積の比較とは分けています。既存の添字付き面集合の元値・出所・診断・予算 78,711 バイトも追加前と一致しました。

## 形の葉から回転体を取得する

`IfcFile::revolved_area(&ShapeNode, cx)` は `IfcRevolvedAreaSolid` の葉から、`ProfileRevolution`・任意断面の外輪と内輪・長さの単位・借用した出所を返します。矩形・円・I 形・中空矩形・任意の閉じた外輪・穴付きの 6 断面を既存の押し出しと共有して読みます。パラメーター付き断面の輪郭は生成しません。

- 定義は元の断面・立体の局所配置・回転軸・単位付きの角度を持ちます。軸の原点は長さの単位のまま、方向は正規化しない 3 成分の比です。断面と同じ XY 平面に原点と非ゼロの方向があることを確認します。軸の方向を省略すると既定の z 軸になり XY 平面内にないため、この取得では拒否します。
- 回転角は非負の有限値を保持します。平面角の単位が解けたときだけ、1 回転以下であることを SI の接頭辞・一段の換算の係数を使って確認します。元の角度には換算を適用しません。0 と負の 0 を保ち、単位がなければ上限を推定せず、元値・`None` と `ifc.unit.missing` を返します。
- IFC2X3 の立体の Position は必須、IFC4 以降は省略可能です。配置・写像・既定の原点や軸を適用しません。軸が断面の内部と交差しないこと、丸みや傾斜を含む輪郭、面積・体積・メッシュは未評価です。
- `RevolutionSource` は回転体・断面と、読んだ配置・軸・点・方向・単位の元のレコードを借用します。任意断面の曲線内の出所は `CurveGraph` が持ちます。STEP 番号・出所の列を意味の値に混ぜず、既存の Building・要素 ID・Hash v1 は変更しません。Building への統合と意味値ハッシュは #419 の残件です。
- 回転体と共有する断面の不正には `ifc.shape.invalid`、未対応の種類には `ifc.shape.unsupported` と元の場所を返します。曲線内の不正は既存の曲線の診断を返します。テーパー付き回転体や未知の派生型を親型として読まず、属性数が異なる場合も部分結果を返しません。共有 `Cx` で参照・数・単位・文字・確保前の量・深さ・中断を検査し、終了時に呼び出し側の深さを戻します。

根拠は [IFC2X3 RevolvedAreaSolid](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometricmodelresource/lexical/ifcrevolvedareasolid.htm)・[Axis1Placement](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometryresource/lexical/ifcaxis1placement.htm)、[IFC4 RevolvedAreaSolid](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometricmodelresource/lexical/ifcrevolvedareasolid.htm)・[Axis1Placement](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometryresource/lexical/ifcaxis1placement.htm)、[IFC4.3 RevolvedAreaSolid](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRevolvedAreaSolid.htm)・[Axis1Placement](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcAxis1Placement.htm) の公開の自然言語の説明です。角度の範囲は Informal Propositions、軸の平面は本文と AxisStartInXY/AxisDirectionInXY の説明を使います。EXPRESS 本体を読み取って実装に使いません。

`tests/revolutions.rs` は合成 3 版の共有写像から各 5 断面の回転体を取得し、元の軸・角度・単位・配置・出所、版差、不正、予算境界、中断、変異入力、既存 ID/Hash を検査します。IfcOpenShell 0.9.0.post1 と回転体の元属性・断面への参照・任意断面の曲線を合計 15 件照合します。ネイティブと WASI では値・診断・全 Kind の使用量をバイト列で比べます。既存 5 断面の押し出しは、追加前と元値・出所・診断・予算を比べます。同梱の公開 IFC 12 件に回転体の実例はなく、この追加の読みは `synthetic` の範囲です。変換後の形や体積の一致を示すものではありません。

```powershell
$env:ZUMEN_IFC_REVOLUTIONS_OUT = Join-Path (Get-Location) 'target/revolution-comparison'
cargo test -p zumen-ifc --locked --test revolutions
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/revolutions.py target/revolution-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-revolutions.mjs'
cargo test -p zumen-ifc --locked --test revolutions --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-revolutions.mjs --compare target/revolution-comparison target/revolution-comparison-wasm
```

## 形の葉から掃引円盤を取得する

`IfcFile::swept_disk(&ShapeNode, cx)` は `product_shape()` の葉から `IfcSweptDiskSolid` を取得します。`SweptDisk::definition` の `DiskSweep` に外半径・省略可能な内半径・区間の始点と終点を、`directrix` に既存の `CurveGraph` を持ちます。定義の曲線の参照は同じグラフの中だけで有効です。元の掃引のレコードは借用する `source`、曲線と補助レコードの出所はグラフに分けます。

- 元の有限な値と省略を保ち、配置・共有写像・単位換算・曲線の近似を適用しません。半径はグラフの長さの単位、区間は曲線の媒介変数です。単位の取得と未対応の曲線の診断は既存の `curve()` と共有します。
- 3D の経路、正の外半径、外半径より小さい正の内半径を要求します。IFC2X3 の区間の始点と終点は必須です。IFC4 以降では有界か閉じた曲線の場合だけ省略を保ち、無限の直線では両端を要求します。区間を並べ替えず、幾何的に有効な範囲かどうかは評価しません。
- 掃引の属性の不正には `ifc.shape.invalid` と `sweep-` から始まる理由・出所を返します。複合の掃引、派生した別の種類、未対応の経路には診断と `Unsupported` を返します。停止・不正・未対応では部分結果を返しません。曲線と同じ `Cx` で手間・深さ・確保前の展開量を数え、終了時に呼び出し側の深さを戻します。
- 接合部の形、自己交差、経路の曲率・長さと半径の関係は未評価です。Building の成分・既存の ID と Hash v1 は変えません。形の評価と全面統合は #419・#420 の残件です。

根拠は buildingSMART の IfcSweptDiskSolid の属性と制約の説明（[IFC2X3 TC1](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC2x3/TC1/HTML/ifcgeometricmodelresource/lexical/ifcsweptdisksolid.htm)・[IFC4 ADD2 TC1](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4/ADD2_TC1/HTML/schema/ifcgeometricmodelresource/lexical/ifcsweptdisksolid.htm)・[IFC4.3 ADD2](https://raw.githubusercontent.com/buildingSMART/Standards-builds/main/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSweptDiskSolid.htm)）です。

`tests/sweeps.rs` は合成の 3 版で、共有写像を通した取得、値・出所・単位・版差・曲線の共有と循環・不正な値・全 Kind の予算・中断・深さの復元・既存 ID とハッシュを検査します。切った入力と変異した入力も共有予算で 2 回処理し、停止位置と診断の一致を確かめます。fuzz の `ifc` target も 16 個までこの取得を通します。同梱の公開 IFC 12 件には掃引円盤がないため、読みの検証は `synthetic` です。IfcOpenShell の元属性との照合と、ネイティブと WASI のバイト列の比較は次で実行できます。

```powershell
$env:ZUMEN_IFC_SWEEPS_OUT = Join-Path (Get-Location) 'target/sweep-comparison'
cargo test -p zumen-ifc --locked --test sweeps
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/sweeps.py target/sweep-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-sweeps.mjs'
cargo test -p zumen-ifc --locked --test sweeps --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-sweeps.mjs --compare target/sweep-comparison target/sweep-comparison-wasm
```

比較するのは合成 3 版の 9 個の掃引と経路の元属性、別に JSON・診断・全 Kind の予算と停止の記録です。配置や写像を適用した形・体積・接合部の評価は含めません。

## 形の項目から面の表示を取得する

`IfcFile::surface_styles(&ShapeNode, cx)` は、既存の `ProductShape` の項目に直接結びつく `IfcStyledItem` から、面の色・透明度・反射の元の値を取得します。`building` の feature が必要です。ifcZIP では `file.ifc()` から同じ操作を呼びます。同じファイルの node だけを受け付け、別のファイルや clone したファイルの node は `ifc.style.invalid` にします。

```rust,ignore
let shape = file.product_shape(element_instance, cx)?;
for node in &shape.nodes {
    let appearance = file.surface_styles(node, cx)?;
    for binding in &appearance.styles {
        let shading = &binding.definition.shading;
        // RGB と透明度、任意の Rendering の指定を元の値で読む
        // 元の SurfaceStyle / Shading / Colour のレコードは binding.source
    }
}
```

公開サンプル `gni/model_9.ifc` の柱 `#131` から、既存の表現・写像の経路で得た葉 `#114` を渡すと、StyledItem `#118` → SurfaceStyle `#117` → Rendering `#116` → Colour `#115` を取得できます。スタイル名は `StB - Ortbeton`、Side は BOTH、RGB は各成分 0.968627450980392、透明度は明示した 0、鏡面反射の係数は 0.5、指数は 128、反射モデルは NOTDEFINED です。利用側で STEP の引数の位置を読む必要はありません。

- 意味の値は L1 に依存しない `zumen_building::SurfaceStyle`・`SurfaceShading`・`SurfaceRendering` に、借用する元の Record は `ItemStyleSource`・`SurfaceStyleSource` に分けます。StyledItem と SurfaceStyle の名前、面の側、色の名前も保持します。色の出所は属性を読んだ順で、重複も残します。比較は `definition` に対して行い、出所を含む全体には等価性・ハッシュを定義しません。
- Rendering の 9 属性をすべて保持します。SurfaceColour・Transparency に加え、DiffuseColour・TransmissionColour・DiffuseTransmissionColour・ReflectionColour・SpecularColour は色か係数、SpecularHighlight は指数か粗さ、ReflectanceMethod は列挙の元値を持ちます。省略は `None` のままとし、既定値・係数の掛け算・透明度から不透明度への変換・反射の式を適用しません。
- IFC2X3 の StyledItem は PresentationStyleAssignment を 1 件経由します。IFC4 ADD2 では直接指定と旧式の Assignment を受け付け、IFC4.3 ADD2 では削除された Assignment を拒みます。Shading は IFC2X3 では色だけ、IFC4 ADD2 以降では任意の透明度を持ちます。Rendering の引数の位置は 3 版とも共通です。PHYSICAL は IFC4.3 だけで受け付けます。
- RGB・透明度・色の係数・粗さは有限な 0〜1、指数は有限な実数として読みます。集合の空・重複、Shading と Rendering の併記、複数の直接の StyledItem、不正な参照・型・列挙・次元は `ifc.style.invalid` と `Error::Invalid` にします。診断に種類または理由と元の場所を持たせます。
- CurveStyle・FillAreaStyle・TextStyle、NullStyle、SurfaceStyleLighting・Textures・Refraction・ExternallyDefinedSurfaceStyle など、合法でも保持しない種類は `ifc.style.unsupported` と `Error::Unsupported` にします。ほかの値だけを返す部分成功にはしません。元のレコードは既存の L1 で参照できます。
- 直接の StyledItem がなければ空の成功です。写像元・材質・レイヤからの継承や優先順位は調べません。写像元の葉に付く指定は、その葉を別に渡して取得します。表示色・材質・面への適用・法線・形を評価した結果には数えません。
- ファイルの全レコードの走査、参照・数・文字列・集合の検査を共有の `Cx` の Work で数えます。確保前の DecodedBytes は、文字列を元の長さの 12 倍と 32、集合を 256 と項目ごと 64、結果の容量を項目ごと 1024 とし、ネイティブと wasm32 で共通にします。結果の容量には 2,097,151 項目の共通上限も置きます。入った深さは成功・不正・未対応・停止のすべてで戻します。

`IfcFile::item_presentation(node, representation, cx)` は、直接の面の表示に加えてレイヤを取得します。`representation` は項目を直接 `Items` に含む ShapeRepresentation、または省略です。共有写像を使う場合も、写像の項目と写像元の葉をそれぞれの表現と組にして呼びます。

- レイヤの名前・説明・識別子、表示・固定・編集禁止の真偽と不明を `PresentationLayer`・`LayerState` に保持します。通常のレイヤと、状態がすべて不明の表示付きレイヤを区別します。表示のない空の `LayerStyles` も保持します。
- `layers` は項目と表現の双方への割り当てを元レコード順に保持します。`layer_source` は項目への割り当てがあれば項目、なければ表現です。表示が空でも項目の割り当てが優先し、表現の表示で補いません。上書きされたレイヤも元の値と借用する出所を残します。
- `surface_source` は直接の面の表示を優先し、なければ選ばれた位置のレイヤの面の表示を指します。IFC2X3 で同じ位置の複数レイヤに面の表示がある場合は `MultipleLayers` とし、ファイル順で勝者を選びません。面の側・透明度・反射式の合成は行いません。
- IFC2X3 の複数の項目レイヤを保持し、IFC4 以降では 1 項目への複数レイヤを拒みます。IFC4 以降の WithStyle は、指定した表現への割り当てを拒みます。項目と表現に同時に割り当てていても同じです。レイヤ内の別の割り当て先の幾何の種類までは検証しません。
- 取得対象の複合レコード、不正な名前・論理値・集合・参照・属性数と、指定した表現の Items にない項目を拒みます。複合レイヤの走査では基の Assignment も調べ、所属なしと誤認しません。所属を読めない配置・リストは `owner-layout`・`owner-list` の診断にします。未知の表示は上書きされる場合も `Unsupported` とし、元の値を失う部分成功にしません。
- 直接表示の検査と全レコード・割り当てリストの走査は同じ `Cx` を使います。該当するレイヤを Elements、結果の容量を確保前に 1 件 1024 DecodedBytes として数え、集合・文字列・面の表示は従来の予約量を使います。レイヤの容量上限は 2,097,151 件です。深さと停止位置も元のレコードに対応させます。

この取得が決めるのは、同じ項目とその直接の表現の優先関係です。Building への保持と形の Hash は[所有する形の成分](#所有する形とハッシュを建物に保持する)、写像の階層間の選択は[使用位置からの選択](#共有写像の使用位置から面の表示を選ぶ)で扱います。材質からの継承は未実装です。既存の `Style` 成分と Hash v1 の符号化・ID・公開 0.2 の境界 op・スキーマは変えません。

レイヤの規則は buildingSMART の [RepresentationItem](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcRepresentationItem.htm)、[PresentationLayerAssignment](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcPresentationLayerAssignment.htm)、[PresentationLayerWithStyle](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcPresentationLayerWithStyle.htm) の説明と、IFC2X3 TC1・IFC4 ADD2 TC1 の同じ項目の説明で確認しました。

根拠は buildingSMART の [StyledItem](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcStyledItem.htm)、[SurfaceStyle](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSurfaceStyle.htm)、[Shading](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSurfaceStyleShading.htm)、[Rendering](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcSurfaceStyleRendering.htm)、[IFC4 の PresentationStyleAssignment](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2/HTML/schema/ifcpresentationappearanceresource/lexical/ifcpresentationstyleassignment.htm)、[IFC2X3 の StyledItem](https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/ifcpresentationappearanceresource/lexical/ifcstyleditem.htm) です。スキーマや第三者の実装は同梱しません。

### 面の表示の確かめ方

`crates/zumen-ifc/tests/styles.rs` は合成の IFC2X3・IFC4・IFC4X3 と公開の柱で、代表要素からの取得、全 Rendering 属性、出所・意味値の分離、取得前後の Building の ID・Hash v1 を確認します。文字列・符号付きの 0・省略・不正・未対応・共有・循環・別のファイルの node、確保前の予算、全 Kind の境界・中断・深さの復元と変異した入力も検査します。

IfcOpenShell 0.9.0.post1 との照合は、合成各版 2 件と公開 1 件の計 7 件です。保持する全属性と参照を、許容差なしの有限な数値の等値と列の厳密な一致で比較します。公開 1 件が `external`、合成 6 件は `synthetic` の範囲で外部の実装とも照合したものです。診断・予算・表示の評価・継承はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_STYLES_OUT = Join-Path (Get-Location) 'target/style-comparison'
cargo test -p zumen-ifc --locked --test styles
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/styles.py target/style-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-styles.mjs'
cargo test -p zumen-ifc --locked --test styles --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-styles.mjs --compare target/style-comparison target/style-comparison-wasm
```

同じテストを WASI でも流し、4 件の JSON と診断・全 Kind の予算使用量を記録する `contract.txt` を、ネイティブとバイト列で比較します。WASI の出力先は `target/style-comparison-wasm` です。fuzz の `ifc` target は既存の形の項目から 16 件までこの取得へ渡し、全レコードの走査を含めて予算を共有します。毎晩の CI でも export と外部照合を流します。

レイヤの取得は `tests/layers.rs` で 3 版の元値・優先規則・不明・空・複数レイヤ・不正・未対応・出所・予算・中断を検査します。公開の柱 `#131` の押し出し `#114` と表現 `#119` は、表現のレイヤ `#5883`（`S-COLS-____-OTLN`）と直接の面の表示 `#117` を持ちます。IfcOpenShell の実行ファイルとは合成 3 版と公開例の 7 レイヤ・3 レイヤ表示の元属性と参照を照合します。優先順位・診断・予算・表示結果は、この外部照合には含めません。

```powershell
$env:ZUMEN_IFC_LAYERS_OUT = Join-Path (Get-Location) 'target/layer-comparison'
cargo test -p zumen-ifc --locked --test layers
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/layers.py target/layer-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-layers.mjs'
cargo test -p zumen-ifc --locked --test layers --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-layers.mjs --compare target/layer-comparison target/layer-comparison-wasm
```

同じテストを WASI で実行し、4 JSON と診断・全 Kind の記録をネイティブとバイト列で比較します。fuzz は表現から直接の項目へ 16 件まで進み、レイヤの走査と旧 getter とで予算を共有します。

## 通り芯から軸と曲線を取得する

`IfcFile::grid_axes(grid_instance, cx)` は、通り芯の L1 番号から U/V/W の軸と既存の `CurveGraph` をまとめて取得します。`building` の feature が必要です。ifcZIP では `file.ifc().grid_axes(...)` を使います。Building の通り芯の要素が持つ `source().instance` からも呼べます。利用側で STEP の引数の位置を読む必要はありません。

```rust,ignore
let grid = file.grid_axes(grid_instance, cx)?;
for axis in &grid.axes {
    let designation = &axis.definition; // set・tag・same_sense
    let curve = axis.curve.item(axis.curve.root);
    // 曲線の元の値と単位を取得。配置と向きは未適用
}
```

公開の `gni/model_9.ifc` では、GlobalId `2vVYk8VWENmwp2otf5sJ1q` の Grid `#5535` から U 10 軸・V 10 軸・W 1 軸を取得できます。先頭の U 軸は `#5450`、タグは `E`、曲線は `#5449`、元の 2D の点の列は `#5448` です。既存の `product_shape()` の FootPrint の中にも同じ曲線への出所があり、この経路と一致することを検査します。

- 所属・タグ・SameSense は L1 に依存しない `GridAxisDefinition`、任意の PredefinedType は `GridLayout` に保持します。省略したタグと空のタグ、向きの真偽、レイアウトの指定と省略を区別します。U・V・W の順に、各 LIST の記録順を保ちます。
- 借用する元の Grid・軸・ObjectPlacement・Representation は `GridSource` と各軸の `source` に分けます。配置は参照先の種類を確認するだけで、属性や親の連鎖を展開せず保持します。配置・反転・既定値を適用せず、レイアウトに対する平行・直交・交点の適合も評価しません。
- 曲線の共有は各軸の `CurveGraph` の中です。同じ曲線を指す複数の軸は、同じ出所を持つ別のグラフとして取得します。グラフを横断する `CurveRef` や重複排除を仮定しません。意味を比べるときは `definition` と曲線の `kind`・単位を使い、CurveSource や元の番号を含めません。出所付きの全体には PartialEq/Hash を定義しません。
- U/V は必須で空でない unique LIST、W は任意で、指定する場合は空でない unique LIST です。同じ軸の重複・U/V/W 間の重複と、選んだ軸を別の Grid が参照する二重所属を拒みます。ファイル全体を走査して選んだ軸の所属を確認しますが、関係のない Grid 全体の妥当性を検査した結果ではありません。
- 別の Grid インスタンスの所属確認では、最初の Part だけに頼らず全 Part の IFCGRID を探します。継承した属性を含む 10/11 属性と、外部マッピングのローカルな 3/4 属性の配置から U/V/W を読みます。配置やリストの形を確認できない場合は `ifc.grid.invalid` の `owner-layout`・`owner-list` で拒み、所属なしと仮定しません。ヘッダーの同名の記録はインスタンスとして数えません。取得対象そのものの複合 Grid を型へ写す操作は、この経路には追加していません。
- AxisCurve は既存の曲線の取得を通し、全体が 2D であることを確認します。未知の曲線を含み次元を確認できない場合は、種類・出所を持つ `ifc.grid.unsupported` と `Error::Unsupported` にし、部分結果を返しません。元の曲線の未対応の診断も保持します。不正な参照・型・属性数・列・所属・次元・真偽は `ifc.grid.invalid` と `Error::Invalid` にします。

| 版 | Grid の属性数 | PredefinedType | ObjectPlacement の省略 |
| --- | ---: | --- | --- |
| IFC2X3 TC1 | 10 | 属性なし | 不可（WR41） |
| IFC4 ADD2 TC1 | 11 | 任意の 6 列挙 | 不可（HasPlacement） |
| IFC4.3 ADD2 | 11 | 同じ 6 列挙 | ShapeRepresentation を含まなければ可 |

列挙は RECTANGULAR・RADIAL・TRIANGULAR・IRREGULAR・USERDEFINED・NOTDEFINED です。IFC4.3 では Grid の HasPlacement が削除され、Product の PlacementForShapeRepresentation に従います。Representation が省略された場合と、参照先の Representations に ShapeRepresentation がない場合は、配置を `None` として保持します。配置の具体型は LocalPlacement・GridPlacement、4.3 では LinearPlacement も受け付けます。

全走査とほかの Grid のリスト、選んだ軸のリスト、各曲線と追加の次元検査、文字列を共有の `Cx` で数えます。1 レコードの Work に先頭 Part を含め、複合のインスタンスでは後続 Part の数も走査前に予約します。停止診断には処理中のレコードの範囲を残します。確保前の DecodedBytes は、選んだリストごとに 256 と項目ごと 64、結果の容量を軸ごと 1024、文字列を元の長さの 12 倍と 32、未対応の種類の文字列を 6 倍として予約します。結果の容量の共通上限は 2,097,151 軸です。曲線の予約量も同じ予算で数え、入った深さはすべての経路で戻します。

Building の Grid 成分の曲線、形の意味値の Hash、ほかの立体と断面は [#419](https://github.com/zumen-jp/zumen/issues/419) の残件です。公開タグ v0.2.0 と現在の main に追加された未公開 API を区別し、既存 API・ID・Hash v1・境界 op・版数は変えません。

根拠は buildingSMART の [IFC2X3 Grid](https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/ifcproductextension/lexical/ifcgrid.htm)、[IFC4 Grid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2/HTML/schema/ifcproductextension/lexical/ifcgrid.htm)、[IFC4.3 Grid](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcGrid.htm)、[GridAxis](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcGridAxis.htm)、[Product の配置の条件](https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/IfcProduct.htm) です。公式ページが取得できない場合は buildingSMART の [公式 HTML スナップショット](https://github.com/buildingSMART/Standards-builds/tree/main/IFC/RELEASE) で版を確認できます。スキーマや第三者の実装は同梱しません。

### 通り芯の確かめ方

`crates/zumen-ifc/tests/grids.rs` は合成の 3 版と公開の通り芯から、値・出所・順序・共有の範囲、配置の版差、反転の未適用、取得前後の既存の ID/Hash v1 を検査します。集合・所属・次元・真偽・循環・未対応、長いリストの確保前の予算、全 Kind の境界・中断・深さの復元、複数の走査位置と変異した入力も確認します。曲線と通り芯の外部照合用の JSON の書き方は `tests/support/curve_json.rs` で共有します。

ほかの Grid が複合レコードである場合も、自作の 3 版で IFCGRID の先頭・後続、全属性・ローカル属性、二重所属・無関係の軸を確認します。未知の属性配置とリストの拒否、Part の走査前の予算と停止位置をネイティブと WASI の契約記録に含めます。この複合レコードの所属確認は `synthetic` の範囲です。

IfcOpenShell 0.9.0.post1 との照合は、合成 3 Grid の 20 軸・26 曲線と、公開 1 Grid の 21 軸・21 曲線です。Grid の配置・表現の参照とレイアウト、U/V/W の順序と軸の参照・タグ・向き、曲線の全保持属性を、有限な数値の等値を許容差なしで比較します。共有を含む曲線の探索も外部の実装側で別にたどります。公開の 1 Grid が `external`、合成の 3 Grid は `synthetic` です。単位・曲線の補助レコード・診断・予算、配置・反転・幾何の評価はこの外部照合に含めません。

```powershell
$env:ZUMEN_IFC_GRIDS_OUT = Join-Path (Get-Location) 'target/grid-comparison'
cargo test -p zumen-ifc --locked --test grids
python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/grids.py target/grid-comparison
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-grids.mjs'
cargo test -p zumen-ifc --locked --test grids --target wasm32-wasip1 -- --test-threads 1
node crates/zumen-ifc/tests/wasi-grids.mjs --compare target/grid-comparison target/grid-comparison-wasm
```

WASI の出力先は `target/grid-comparison-wasm` です。4 JSON と、診断・全 Kind の予算境界・中断・複数の走査位置を含む `contract.txt` のバイト列をネイティブと比較します。fuzz の `ifc` target は 16 Grid までこの経路へ渡し、形状表現の取得と予算を共有します。毎晩の既存 IfcOpenShell job でも export と外部照合を流します。

## 曲線を必要なときに取り出す

Rust の `IfcFile::curve(instance, cx)` は、曲線のインスタンスを指定し、`zumen_building::CurveGraph` を返します。`building` の feature（既定で有効）が必要です。`IfcZipFile` では `file.ifc().curve(instance, cx)` と呼びます。要素の番号や `IfcGridAxis` の番号ではなく、それらから参照する曲線の番号を渡します。

```rust,ignore
// 公開サンプル samples/ifc/gni/model_9.ifc の柱の断面の輪郭
let curve = file.curve(111, cx)?;
let item = curve.item(curve.root);
```

- `CurveKind` は `IfcPolyline`・`IfcLine`・`IfcCircle`・`IfcIndexedPolyCurve`（IFC4 以降）・`IfcCompositeCurve`・`IfcTrimmedCurve` を持ちます。円や弧を折れ線にせず、点や区間を逆に並べず、閉じる点も補いません。折れ線は最初と最後の点が同じなら閉じます。添字の曲線は、区間がなければ最初と最後の点、区間があれば最後と最初の添字で閉じるかを表します。合成の曲線は区間の順・`SameSense`・`Transition`、切った曲線は `SenseAgreement` と優先する表現を保持します。
- 座標は 2 次元と 3 次元を区別し、曲線が書かれた座標空間のまま持ちます。円は中心と軸の比、直線は基点・方向の比・ベクトルの大きさを持ち、軸を正規化しません。3 次元の円の軸と参照方向は、ともに指定するか、ともに省略する必要があり、平行・反平行な指定は診断します。要素の配置・写像・表現の文脈の変換は適用しません。座標・半径・大きさの単位は `length_unit`、円の媒介変数の単位は `angle_unit` に持ち、度かラジアンに換算しません。直線の媒介変数は方向の大きさに掛ける無次元の値です。解けない単位は `None` にし、円の平面角の単位がなければ `ifc.unit.missing` を返します。
- 切る位置に点と媒介変数の両方が書かれていれば、どちらも元の順で残します。点が基の曲線の上にあること、点と媒介変数の一致、区間の接線と曲率の連続、添字の弧の非共線性や自己交差は評価しません。`self_intersect` は元の申告値です。
- 共有する基の曲線は 1 度だけ写し、`CurveRef` で参照します。参照はこのグラフ内だけで有効で、L1 の番号・要素の ID・ハッシュではありません。`CurveSource.instance` は曲線の L1 の番号、`records` は直接読んだ点・配置・方向・区間などの L1 の番号を読んだ順で持ちます。合成の曲線の区間の出所は、意味の値から外し、`records` に `segments` と同じ順で持ちます。子の曲線の出所は参照先から引きます。出所を意味の値から分け、STEP の番号を付け替えても同じ定義を返します。
- 型を持たない種類は `Unsupported` に種類と出所を残し、`ifc.curve.unsupported` に種類ごとの件数を返します。既知の子曲線の次元を参照元へ伝え、合成の区間同士と、基の曲線と切る点の次元を照合します。必要な参照の欠落、型・次元・有限な数・添字・区間の接続・切る位置の構成の誤りと循環は、`ifc.curve.invalid` と `Error::Invalid` にし、グラフの一部を返しません。`reason` は `reference`・`type`・`dimension`・`number`・`direction`・`radius`・`count`・`list`・`enumeration`・`boolean`・`indices`・`connectivity`・`trim`・`placement`・`unit`・`cycle` です。
- 探索と所有権は平らな列にし、深い曲線も Rust の再帰を使いません。探索の入れ子は `depth`、参照・数・列の読み取りは `work`、曲線・点・区間・出所・探索の領域は確保前に `decoded-bytes` で数えます。予約量は 64 bit と wasm32 で共通です。中断・予算では `Error::Stopped` にし、入った深さを戻します。近似で点を作らないので `segments` は使いません。

この getter は、`Building::lift` のコンポーネントを変更しません。既存の `ShapeItem`、要素の ID と Hash v1、各言語への API も変えません。L1 の形状表現・共有写像から曲線へは、上の `product_shape()` を使えます。Building の形状表現・style・通り芯への統合、要素ハッシュへの意味の値の取り込み、立体と断面の型は [#419](https://github.com/zumen-jp/zumen/issues/419) に残ります。形を評価するカーネルへの変換は [#420](https://github.com/zumen-jp/zumen/issues/420) の対象です。

### 曲線の確かめ方

`crates/zumen-ifc/tests/curves.rs` の合成の 3 バージョンで、定義・単位・向き・共有・出所、循環・予算・中断、取得順と STEP の番号の付け替え、取得前後の既存の要素の ID とハッシュを確かめます。公開サンプル `gni/model_9.ifc` の柱の GlobalId `2q2CBrfQTCKPY0zxCdZygb`（`#131`）から、`#127`・`#126`・写像 `#125`・写像元 `#121`・表現 `#119`・押し出し `#114`・断面 `#112`・曲線 `#111`・点の列 `#110` をたどり、輪郭の 5 点を比較します。写像や立体を型に写した結果の確認ではありません。

外部の実装との照合では、合成の IFC2X3 の 9 曲線・IFC4 と IFC4X3 の各 10 曲線と、公開サンプルの `#111` を、IfcOpenShell 0.9.0.post1 の元の属性値と比べます。合成の数には共有する曲線の再取得を含み、29 件は `synthetic` の範囲で外部の実装とも照合したものです。公開サンプルの 1 件（IndexedPolyCurve）だけを、読みの `external` に数えます。方向の比・座標の次元・半径・大きさ・添字・区間の向き・切る位置の値と順に食い違いはありません。単位・診断・予算と、曲線の評価は、この外部の照合には含めません。毎晩の CI でも同じ手順を流します。

```powershell
$env:ZUMEN_IFC_CURVES_OUT = 'target/curve-comparison'
cargo test -p zumen-ifc --locked --test curves writes_curves_for_other_readers
python -I crates/zumen-ifc/tests/ifcopenshell/curves.py target/curve-comparison
```

同じ Rust のテストを `wasm32-wasip1` に作り、Node.js の WASI で流せます。1 MiB のスタックを指定するスレッドのテストだけはネイティブで流します。WASI の runner はリポジトリだけを開き、ホストの環境変数を渡しません。`target/curve-comparison-wasm` の JSON に、同じ曲線の定義と各予算の使用量を書きます。ネイティブの JSON とバイト列で比較できます。

```powershell
rustup target add wasm32-wasip1
$env:CARGO_TARGET_WASM32_WASIP1_RUNNER = 'node --no-warnings tests/wasi-curves.mjs'
cargo test -p zumen-ifc --locked --test curves --target wasm32-wasip1 -- --test-threads 1
```

`fuzz/fuzz_targets/ifc.rs` も、読んだ IFC の曲線を 16 件まで必要時の getter に渡し、建物・土木のモデルと予算を共有します。型を持つ曲線と切った入力は、Rust のテストキットでも、2 度の結果・診断・使用量を確かめます。

## 階・区域・分類の成分

次の値を建物モデルに写す。属性の位置は、台帳の IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の各実体の「属性」の節による。

- `level`: `IfcBuildingStorey` の `Name` と `Elevation` を、文書の長さの単位で持つ。所属の親をたどって建物が見つかり、その `ElevationOfRefHeight` があれば、`datum` にその建物の ID を持つ。基準の値と単位は、その建物の `spatial.elevation` から引ける。敷地と建物の標高から `level` は作らない。IFC4.3 ADD2 は `Elevation` を非推奨にし、高さは階の配置で決まるとする。IFC4.3 で `Elevation` がなければ、所属の親をたどって見つけた建物の配置の座標で、階の配置の原点の z を高さにする。高さがないか有限の数でなければ、成分を作らず `ifc.attribute.invalid` を返す。
- `1FL` のような階の名前をそのまま持ち、lift では `level.kind` や高さを補わない。種類への対応は、[作図の慣習](../../profiles/zumen-building-drawing/2026-11/README.md)の `level_kind` で明示した語彙から得る。元の名前・高さ・種類は変更しない。
- `zone`: `IfcZone` と、IFC4 以降の `IfcSpatialZone` に付ける。`IfcZone` は `IfcRelAssignsToGroup`、`IfcSpatialZone` は `IfcRelReferencedInSpatialStructure` の、室と区域の直接の参照を、関係と並びの順で持つ。区域の中の区域を展開しない。物理の要素への空間の参照は、従来の `parent.referenced_in` に持つ。語彙がないので `zone.kind` は持たず、IFC の `PredefinedType` と `ObjectType` は `class` に残す。境界の形は #419 で扱う。
- `classification`: `IfcRelAssociatesClassification` の対象に、`IfcClassificationReference` のコード（IFC2X3 の `ItemReference`、IFC4 以降の `Identification`）と名前を付ける。`ReferencedSource` から `IfcClassification` の名前と版を引く。IFC4 以降は参照の階層をたどり、体系そのものの関連づけも持つ。体系のない軽い分類は、コードと名前だけを持つ。循環、欠けた参照、対応していない種類は、成分を付けず `ifc.attribute.invalid` を返す。
- 出所は、階と区域の元の要素のインスタンス、所属と分類の関係のインスタンス・GlobalId・並びの位置を持つ。区域の成分を付けた関係は区域の側にも残すので、L1 の値までたどれる。参照の連鎖と親の探索は深さと手間を数え、成分と出所を写す前に展開のバイトを数える。
- 区域と分類の構成員の集合は、参照でない値と欠けた参照を `ifc.attribute.invalid` で診断する。有効な参照は元の位置を保って写す。L2 の要素にできない対象の実体自体を落とす場合は `ifc.entity.dropped` だけで数える。プロパティ定義など別の成分に使った実体に関係だけを付けられない場合は、`ifc.relation-target.unsupported` に関係・対象の種類と元の集合内の位置（0 から）を返す。ほかの関係にも有効な構成員がない区域は、成分を付けない。型の分類を IDS で照合するときは、要素自身が持たない体系について、その型にあるすべての参照を引き継ぐ。

## 建物を図面へ切り出す

IFC の `lift_geometry` で所有する建物を作り、`zumen-building::drawing::cut` へ渡します。Rust の `zumen-ifc` では `drawing` feature を有効にします。切断面・見る深さ・許容差・縮尺・派生 ID・出所・診断の契約は、[建物から図面への切り出し](../architecture.md#建物から図面への切り出し)にあります。共通 API の op とファイルへの lower は、この Rust の入口とは別です。

建具の Plan の文脈があれば、その曲線を優先します。記号を選ぶ `JoinerySymbol` は、要素、または直接の型が持つプロパティセット名・プロパティ名・列挙型名・値を照合します。要素の値を型の値で上書きしません。複数の記号が一致したら診断し、推測で選びません。現在の lift が持つ DoorPanelProperties の `PanelOperation`、WindowPanelProperties の `OperationType` などを使えます。扉・窓そのものの `OperationType`・`PartitioningType` をこのプロパティと同一視しません。幅・開く側・線幅などを切り出しのコアでは補いません。

形状付きの `lift_geometry` は、扉・窓の `OverallWidth`・`OverallHeight`、扉・扉の型・旧 Style の `OperationType`、窓・窓の型の `PartitioningType`、旧 WindowStyle の `OperationType` を、元の要素の `extension` に `ifc.attribute` の名前空間で保持します。名前と列挙型名は元の属性のまま、寸法は文書の長さの単位です。通常の `Lift<Building>` と常駐編集の成分には、この追加を混ぜません。

`zumen_profile::building_drawing::DrawingProfile::cut` は、この拡張属性を `AttributeSymbol` へ接続します。扉の型がある場合はその開閉形式を採り、不明でも要素の値へ戻りません。幅の省略を形から補わず、窓の分割形式を開閉形式へ読み替えません。既定の記号、出典、未知の材料・境界・軸の診断は[作図プロファイル](../../profiles/zumen-building-drawing/2026-11/README.md)に記録しています。

### 公開 IFC の代表階と出所

`tests/building_drawing.rs` は、林野庁の公開サンプルの 2FL = 4,300 mm、切る高さ 1,200 mm、見る深さ 2,000 mm、格子 0.01 mm、弦の誤差 1 mm で図面を作ります。壁の線から GlobalId `3hYgN0Ix5829GlT_ttwWPW`、壁 `#13256`、形 `#13253` へ戻ることを検査します。母体の参照から、開口の GlobalId `1ol1N$i7$nTs9PKPhumpBV`、開口 `#27507`、形 `#27501` もたどります。開口を独立した線として重ねず、母体から引いた結果にします。開口の関係をたどれることと、個別の線をどの開口の面が作ったかの特定は区別します。

`building_drawing` の例は `ZUMEN_SAMPLES` 内の相対パスを受け、形状評価・切断・投影・隠れ線の判定を含む時間を 3 回測ります。入力の上限は 512 MiB、論理確保量は 64 GiB、作業量は 1,000 億です。既定の出力は入力番号・設定・件数・時間・確保量・診断の code だけです。`--bounds` は評価した形の範囲をメートルで返します。`--json` は 1 入力の線・座標・ID・出所・開口の関係を返します。後者には図面の値が含まれるため、保存先はリポジトリ外とし、公開前に対象の権利を確かめます。

```powershell
$env:ZUMEN_SAMPLES = 'リポジトリ外へ公開 IFC をコピーしたフォルダー'
cargo run -p zumen-ifc --features drawing --release --example building_drawing -- --floor-m=4.3 --height-m=1.2 --depth-m=2 rinya/wood-office-architecture.ifc
cargo run -p zumen-ifc --features drawing --release --example building_drawing -- --floor-m=4.3 --height-m=1.2 --depth-m=2 --json rinya/wood-office-architecture.ifc
```

この入力と指定が #5 の図面の試作の再現条件です。2026-11-16 の Windows x64、release の実行では 6,683 本（切断 2,168、見え掛かり 1,840、隠れ 2,661、通り芯 14）を出しました。投影の失敗は 0、格子上で縮んだ辺や輪郭は 549 件です。同じ条件の 3 回は全件数が一致しました。総時間の中央値は 397.639 ms、確保量の最大は 14.11 MiB です。時計とアロケーターの測定はコアの出力に含めません。

### 切り出しの測定と平面図の比較

同じ環境で公開 12 件を測りました。格子は実寸 0.01 mm、弦の誤差は 1 mm、深さは 2 メートルです。下表の時間は隠れ線の判定を含む 3 回の中央値で、読み取りと lift は除きます。「形の失敗」は評価が不完全な要素数、「投影の失敗」は投影または切り口の位相の計算に失敗した部品数です。格子で縮んだ区間はこれらと分けて診断します。

| 入力 | 切る Z（メートル） | 時間（ms） | 投影した部品 | 形の失敗 | 投影の失敗 | 縮んだ区間 | 線 | 隠れ線 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 林野庁 | 1.2 | 359.194 | 1,600 | 0 | 0 | 435 | 7,373 | 2,927 |
| Text2BIM | 1.2 | 4.541 | 43 | 10 | 0 | 3 | 139 | 48 |
| 中野 Outside | 83.223 | 1.917 | 37 | 0 | 0 | 9 | 51 | 9 |
| 中野 ConcertHall | 84.589 | 3.550 | 80 | 0 | 0 | 19 | 120 | 25 |
| 中野 6F chapel | 33.874 | 16.397 | 208 | 0 | 0 | 184 | 779 | 149 |
| 中野 20F Restaurant | 71.895 | 54.813 | 62 | 0 | 0 | 507 | 2,494 | 1,029 |
| 中野 entrance | 47.486 | 12.677 | 188 | 0 | 0 | 32 | 195 | 37 |
| GNI model_9 | 1.2 | 249.310 | 640 | 0 | 0 | 496 | 6,444 | 1,846 |
| GNI model_18 | 1.2 | 512.924 | 968 | 5 | 0 | 1,441 | 15,869 | 7,731 |
| GNI model_125 | 1.2 | 848.843 | 5,290 | 0 | 0 | 582 | 14,899 | 8,658 |
| BIM Whale TallBuilding | 1.2 | 40.326 | 167 | 0 | 0 | 91 | 1,466 | 352 |
| BIM Whale LargeBuilding | 1.2 | 398.791 | 624 | 0 | 0 | 447 | 12,918 | 2,719 |

中野の 5 件は Z = 1.2 m に形がないため、評価した形の高さの中央付近を明示して測りました。実際の階の FL として推定した値ではありません。12 件の投影失敗率は 0/9,907 部品です。形の失敗 15 要素と格子で縮んだ区間があるため、無欠落の図面が 12 件できたという意味ではありません。切り口を整数の領域にする方法と、面の深さを使う区間判定で、これらの入力を処理できました。

比較先は、日本住宅・木材技術センターが公開する[令和 4 年度版の標準モデル、4 ページ](https://www.howtec.or.jp/files/libs/4644/202304180947116309.pdf)の 2 階平面図です。[林野庁の手引き](https://www.rinya.maff.go.jp/j/mokusan/attach/pdf/bim-19.pdf)は、この標準モデルを IFC の基として示しています。比較資料の PDF は再配布せず、公開元のページから閲覧します。

平面の長辺 49.14 m、短辺 17.29 m、左右と中央の事務室、片側の廊下とコアの配置を照らしました。IFC の壁の切断線の外端は X = -178〜49,318 mm、Y = -178〜17,468 mm で、基準軸の寸法と壁の外面を混同しません。PDF の扉の開閉記号・室名・寸法・通り番号はこの出力にはなく、扉は閉じた実体の切断になります。PDF の階段・衛生器具も同じ詳細で出力されません。IFC にない形や記号は補わず、記号と注記は #423 の範囲とします。

この PDF の比較は公開された設計図との照合です。IFC は公開元が保存し直したもので、元の Revit ファイルと同一設定で出した平面図は入手していません。書き出したソフトとの同条件の照合は、次の Revit との比較で行いました。環境間の決定性は、これと別に `drawing_contract` の 547,929 バイト（SHA-256 `147fe7fc8c0ca856c7f0bc6cdfa899b7dd8c945dc3d67844ecb907f3044521c7`）で照合します。

### Revit の平面図との比較

この IFC を書き出した Revit で、同じ条件の平面図を作って比べました。元の RVT は公開されていないため、Revit 2027 で同じ IFC を開いています。そのため Revit 側の結果には、Revit の IFC の取り込みでの解釈も入ります。2026-11-17 に、`6423ba7a` の `building_drawing` の例を `--floor-m=4.3 --height-m=1.2 --depth-m=2` で動かしました。結果は 6,683 本（切断 2,168・見える線 1,840・隠れ線 2,661・通り芯 14）でした。

Revit 側の条件は次のとおりです。

- 2FL（4,300 mm）の平面ビューを作る。
- 切断面は 1,200 mm、ビューの深さは -800 mm とする。上端も 1,200 mm にして、切断より上の要素を描かない。
- 詳細度は詳細、トリミングはなし。
- DXF R2018 へ、共有座標を使わずに書き出す。

取り込みでは、エラー 2 件と警告 52 件を自動で処理しました。比べ方は、線を 5 mm ごとに区切り、相手の線から 2 mm 以内にある長さの割合を求めるものです。Revit の線には、zumen の隠れ線以外の線を当てました。Revit の手順と照合のスクリプトはリポジトリの外に置き、Revit の出力も同梱しません。

| Revit の画層 | 長さ（メートル） | zumen の線から 2 mm 以内 |
| --- | ---: | ---: |
| 外壁の切断（A-WALL MCUT） | 195.65 | 99.8% |
| 窓の切断と投影（A-GLAZ） | 404.57 | 100% |
| 扉（A-DOOR） | 4.36 | 100% |
| 床の投影（A-FLOR） | 115.55 | 98.6% |
| 通り芯（S-GRID OTLN） | 423.55 | 100% |
| 内壁の切断（I-WALL MCUT） | 223.58 | 34.5% |
| 内壁の投影（I-WALL OTLN） | 36.74 | 38.7% |
| 外壁の投影（A-WALL OTLN） | 199.43 | 53.7% |
| 取り込んだ形（G-XREF） | 75.23 | 12.6% |
| 通り芯の記号と文字（S-GRID IDEN） | 99.17 | 0.1% |

床の投影の残り 1.4% は、zumen の隠れ線まで含めると 100% になります。外壁・窓・扉・通り芯の切断と投影は一致しました。差の内訳は次のとおりです。

- **内壁**: Revit の内壁の切断線を中点で見ると、長さの 97% は zumen の壁の面から、その壁の厚さの半分（75・118・132 mm）だけ離れています。たとえば厚さ 150 mm の壁 `3hYgN0Ix5829GlT_ttwWQF` の Body は、中心線の両側に 75 mm ずつ広がる矩形の押し出しです。zumen の切断線もこの両面に引かれ、室（IfcSpace）の境界とも一致します。この壁の材料は IfcMaterialLayerSetUsage を介さず、IfcMaterialLayerSet を直接持ちます。Revit は取り込みでこの壁を自身の壁に変換するとき、中心線を壁の片面として扱いました。差は zumen の切り出しの誤りではありません。
- **取り込んだ形**: Revit の取り込んだ形の線は、ずれた内壁に付く扉の周りに多く、扉の線から 10〜50 mm、または室の境界から 115〜140 mm 離れています。内壁と一緒にずれた扉の枠と見ていますが、要素ごとには確かめていません。
- **外壁の投影**: 一致しない 92 m は、窓の開口の下で見える外壁の中の線です（X = -16 mm、壁の厚さ -178〜147 mm の中）。Revit は複合壁の層の境界を投影に描きますが、zumen は材料の層を描きません。
- **zumen だけにある線**: 室の境界（147 m が Revit の線から 50 mm より遠い）と隠れ線は、Revit のこのビューの既定では線になりません。通り芯の記号と文字は #423 の範囲です。

上端を 2,300 mm にした最初の比較では、2FL の天井（IfcCovering の CEILING、Z = 6,500〜6,510 mm）10 件が、Revit の一般モデルとして描かれました。これは、切断面と上端の間の要素を投影するという表示の約束による差です。天井の外形は正しい壁の面に沿うので、そのときは内壁の一致が見かけ上 48.5% に上がりました。

## 建物の数量を照らす

`lift_geometry` の建物を `zumen-building::measure` へ渡します。Rust の `zumen-ifc` の例では `measure` feature を有効にします。境界・単位・丸め・出所・許容差・失敗の契約は、[建物の数量](../architecture.md#建物の数量)にあります。ファイルの `IfcElementQuantity` は計算へ足さず、比較先として別に読みます。

`building_measure` の例は、`ZUMEN_SAMPLES` 内の相対パス、GlobalId、文書単位の原点の XYZ、求める境界（`shape`・`center`・`inside`）、元の形の解釈（`auto`・`center`・`inside`）を受けます。メートルの許容差は 0.000001、弦の誤差は 0.001、面積の出力は平方メートル、丸めは合計の小数点以下 3 桁で四捨五入と明示しています。入力は 512 MiB、作業量は 1,000 億、論理確保量は 64 GiB までです。出力は計算値と件数だけで、入力のファイル名・要素名・記録された数量を混ぜません。

この例では属性を `Lift<Building>` で移し、計測する要素と壁、および部品の分解と開口でつながる形を `owned_geometry` で取得します。階などの空間構造の集約へは広げません。対象外の通り芯などの不正で、室の比較まで止めないためです。全体を取得する `lift_geometry` の検証規則と、失敗したときに部分結果を返さない契約は変えません。

公開サンプル 12 件を調べ、BIM Whale の 11 室に `BaseQuantities` の `GrossFloorArea`・`NetFloorArea` がありました。両者の値はこの入力では同じです。`tests/ifcopenshell/measure.py` は IfcOpenShell 0.9.0.post1 の実行だけで記録済みの値を読み、室の形の範囲から高さを選びます。その高さの形のままの断面積を、zumen の例から別に求めます。2026-11-16 の比較は次のとおりです。

| 公開入力 | 室 | 計算面積の合計（m²） | 記録済み面積の合計（m²） | 1 室の最大絶対差（m²） |
| --- | ---: | ---: | ---: | ---: |
| BIM Whale LargeBuilding | 8 | 299.48 | 299.4800000000007 | 2.99 × 10⁻¹³ |
| BIM Whale TallBuilding | 3 | 58.519999999999996 | 58.520000000000195 | 2.03 × 10⁻¹³ |

11/11 室が 0.00001 m² 以内で一致します。残った差は浮動小数点の計算と表現の範囲です。この比較は IFC の保存済みの数量との照合であり、書き出し元の画面や PDF の面積表との照合とは分けます。

```sh
cargo build -p zumen-ifc --features measure --example building_measure
python crates/zumen-ifc/tests/ifcopenshell/measure.py target/debug/examples/building_measure
```

作成元の面積表は、[BLCJ の公開サンプル](https://blcj.or.jp/archive_index/samplemodel/)の GLOOBE 標準モデルの IFC・PDF の組で別に照らしました。2026-11-16 に取得した ZIP の SHA-256 は `b0d8ca7799a041de777d754f392b1538b70d529f128da553007f5015503ee0a9`、IFC は `0f674223f596c483c5abea0b58fe6c05ef3c68692409c977782040ffa501cca4`、PDF は `cba13db9f83a5b0f90e6adc07b06888e1132f8f62c6e2c3b0b53dde319ad63c9` です。元のモデル・PDF・抽出した図や照合条件はリポジトリ外に置き、同梱しません。

PDF の 18 ページ（A-15）の 2 階の 4 室を、床から 1 m の水平面、形のままの境界で計測しました。高さで断面が変わらない室なので、IfcOpenShell の `get_footprint_area` の Z 方向の投影面積も独立に求めました。次の差は丸め前の計算値から PDF の合計値を引いた値です。

| PDF の求積符号 | 計算面積（m²） | PDF の合計（m²） | 差（m²） |
| --- | ---: | ---: | ---: |
| A1 | 96.96 | 96.96 | 0 |
| A2〜A4 | 77.770876 | 77.71 | +0.060876 |
| A5 | 8.559524 | 8.54 | +0.019524 |
| A6 | 8.6196 | 8.62 | −0.0004 |

4/4 室で外部実装の形から求めた面積との差は 0.00001 m² 未満です。PDF との最大差は 0.060876 m² で、四捨五入で小数点以下 2 桁にしても 2/4 室だけが一致します。差の大きい室では、IFC の元の寸法と PDF の式の寸法が違います。たとえば A2〜A4 に対応する形は `9.6 × 3.4 + 6.688 × 6.7 + 2.612 × 0.123` で計算値を再現でき、PDF の式では一部の辺が小数点以下 2 桁です。差をメッシュ化の誤差や浮動小数点の誤差として扱わず、求積表の寸法・集計条件との差として残します。

PDF の 2 ページには申請用の集計段階で小数点以下 2 桁へ切り捨てる説明があります。ただし、この説明だけで居室表の全行の丸める段を再現できるとは判断しません。PDF の表示式の積と表示合計が異なる行もあります。元のソフトの内部の集計順や非表示の桁は未確認であり、計測側で丸め方を推定して一致させません。

再照合には `tests/ifcopenshell/exporter_measure.py` を使います。比較条件の JSON に、`ifc`・`pdf`（`ZUMEN_SAMPLES` からの相対パス）、`ifc_sha256`・`pdf_sha256`、`cases` の各項目に `global_id`・`origin_doc`（文書単位の XYZ）・`table_m2` を外部で用意します。スクリプトはファイルのハッシュを確かめ、外部の形との一致を検査し、表との差は修正せず出力します。

```sh
python crates/zumen-ifc/tests/ifcopenshell/exporter_measure.py target/debug/examples/building_measure "$ZUMEN_SAMPLES/quantity-reference.json"
```

## 書き出し

- 変えずに書くと、IFC-SPF は元のバイト列と一致する。ifcZIP は、元のアーカイブのバイト列をそのまま書く。
- `building_apply` の op は、変更を当てたインスタンスだけを `#n=名前(引数);` の形で書き直す。足すインスタンスには今の最大の番号の次から振り、データのセクションの終わり（`ENDSEC;`）の前に、ファイルの改行で 1 行ずつ書く。ほかのレコードとレコードの間は、元のバイト列のまま書く。ifcZIP で開いたものは、書き直した IFC-SPF を元の項目の名前で入れ直す。
- 書く文字列は、`'` と `\` を重ね、印字できる ASCII の外の文字を `\X2\…\X0\`（基本多言語面の外を含めば `\X4\…\X0\`）にする。
- 当てられるのは、`IfcPropertySingleValue` の値と、`IfcRoot` の Name・Description。ほかの種類のプロパティ（列挙・表など）と、GlobalId を持たない要素（材料・通り芯の軸）は、`change.edit.unsupported` で当てない。
- 空間・要素・関係も足し、除き、置き直せる（[#897](https://github.com/zumen-jp/zumen/issues/897)）。Rust の `zumen_ifc::apply` の変更と、`building_apply` の `add_space`・`add_element`・`remove`・`place`・`contain`・`type`・`void`・`fill`（スキーマのマイナーバージョン 96 から）。文書エンジン（`IfcLive::apply`）は、今までの経路で当てて状態を作り直す（[依存](../live.md#依存)）。
  - 足す: 敷地・建物・階（`IfcRelAggregates` で親に結ぶ。IFC4.3 は建物の `ElevationOfRefHeight` と階の `Elevation` を非推奨にしたので書かない。階の高さは、配置がなければ親の配置から高さだけ上げて置き、配置があれば親からの z が高さと同じときだけ受ける。建物の標高は断る）と、書き手の表にある要素（壁・床・柱・梁・部材・板・被覆・手すり・屋根・階段・斜路・基礎・杭・扉・窓・開口・プロキシ。`IfcRelContainedInSpatialStructure` で空間に結ぶ）。同じ要求で足した要素は、Rust では足した順の番号、`building_apply` では足した変更の `edits` の位置（`added`）で指せる。`building_apply` の結果は、足した要素の ID（GlobalId）とハッシュを返す。
  - 結ぶ・直す: 配置（`IfcLocalPlacement` と `IfcAxis2Placement3D`）、所属する空間、型（`IfcRelDefinesByType`）、開口（`IfcRelVoidsElement`）と充填（`IfcRelFillsElement`）。関係は、同じ相手の関係があればその列へ足し、なければ足す。外して列が空になった関係は除く。置き直す配置がその要素と子の配置だけに使われていれば、その場で書き直し、子の配置は付いてくる。
  - 除く: 要素と、その要素の開口、要素だけが使う配置・表現・定義・関係。ほかから使われる配置と、`IfcRoot` の対象・所有履歴・表現の文脈・材料・単位は残す。子や所属する要素を持つ空間は除かない。除いたレコードは、前との間が空白だけなら、その空白も書かない。
  - 書く実体と属性の数は、lift と同じく手書きのバージョンの表で持つ。そのバージョンにない実体と、持たない定義された種類は書かない。必須の定義された種類（IFC2X3 の屋根・階段・斜路の ShapeType、基礎・杭）を渡されなければ `NOTDEFINED` を書く。
  - 足す `IfcRoot` の所有履歴は、プロジェクトの所有履歴を使う。IFC2X3 でなければ断り、IFC4 と IFC4X3 では `$` にする。GlobalId は、渡された値が文書とこの要求で重ならなければ使う。渡されなければ、Rust では今と同じく作り、`building_apply` では文書のハッシュと変更の位置から作って L1 に渡す。
  - 当てられない変更（表にない実体、所属できない空間、輪になる配置、剛体でない変換、重なる GlobalId、ないか除いた要素）は、`change.edit.unsupported` で断り、要求のどの変更も当てない。
- 意図の値で壁を足せる（[#898](https://github.com/zumen-jp/zumen/issues/898)）。Rust の `Edit::Wall` と、`building_apply` の `add_wall`（スキーマのマイナーバージョン 97 から）で、芯の 2 点（空間の配置の XY）・高さ・材料の層と芯からのずれを受ける。IFC2X3 は `IfcWallStandardCase`、IFC4 と IFC4X3 は `IfcWall` に書き、空間に包含で結ぶ。
  - 配置は、空間の配置からの相対で、芯の始点を原点、芯の向きを x 軸にする。
  - 表現は `Axis`（芯の 2 点の折れ線、`Curve2D`）と `Body`（層の厚さの合計の矩形を閉じた折れ線の断面にして、高さだけ押し出す `SweptSolid`）。文脈は、プロジェクトの `Model` の文脈と、その下の `Axis`・`Body` の副文脈を使い、なければ足す。
  - 材料は、層ごとの `IfcMaterialLayer` を `IfcMaterialLayerSetUsage`（`AXIS2`・`POSITIVE`）で使い、`IfcRelAssociatesMaterial` で結ぶ。層は芯の左へ並び、ずれ（`OffsetFromReferenceLine`）を渡されなければ厚さの合計の半分を負にして、芯を厚さの中央にする。`IfcMaterial` は壁ごとに書き、同じ壁で同じ名前の層は同じ材料を指す。
  - 芯の 2 点が重なる、高さや層の厚さが正でない、層がない、値が有限でない壁は、`change.edit.unsupported` で断る。
- 意図の値で床・柱・梁を足せる（[#898](https://github.com/zumen-jp/zumen/issues/898)）。Rust の `Edit::Slab`・`Edit::Column`・`Edit::Beam` と、`building_apply` の `add_slab`・`add_column`・`add_beam`（スキーマのマイナーバージョン 98 から）で受け、空間の配置からの相対に置き、空間に包含で結ぶ。表現は `Body`（閉じた折れ線の断面を押し出す `SweptSolid`）だけで、文脈は壁と同じく使い回す。
  - 床は、外形の頂点（空間の配置の XY、3 つ以上）・材料の層・上の面の高さで受ける。上の面を配置の原点にし、層の厚さの合計だけ下へ押し出す。`IfcSlab` の `FLOOR` に書き、材料は `IfcMaterialLayerSetUsage`（`AXIS3`・`NEGATIVE`・ずれ 0）で、層は上から下へ並ぶ。最後の頂点が最初と同じなら除く。
  - 柱は、断面の中心の位置・脚の高さ・高さ・矩形の断面（幅とせい）・x 軸の向きで受け、断面の中心を配置の原点にして上へ押し出す。
  - 梁は、天端の芯の 2 点（空間の配置の XYZ）と矩形の断面で受ける。始点を配置の原点、芯の向きを x 軸、水平で芯の左を y 軸にし、断面（幅は芯の左右、せいは天端から下）を芯に沿って押し出す。傾いた梁も書き、鉛直の芯は断る。
  - 柱と梁の材料は、名前を渡されれば `IfcMaterial` を `IfcRelAssociatesMaterial` で結ぶ。`IfcColumn` と `IfcBeam` の PredefinedType（IFC4 から）は書かない。
  - 頂点が足りない・並ぶ頂点が重なる・面積のない外形、正でない高さ・断面・層の厚さ、長さのない向き、有限でない値は、`change.edit.unsupported` で断る。
- 意図の値で壁に開口をあけ、扉や窓で埋められる（[#898](https://github.com/zumen-jp/zumen/issues/898)）。Rust の `Edit::Opening` と `Edit::Filling`、`building_apply` の `add_opening`・`add_door`・`add_window`（スキーマのマイナーバージョン 98 から）で受ける。
  - 開口は、壁・芯に沿う距離・下端の高さ・幅・高さで受ける。壁の配置からの相対で、距離と下端の点を原点にし、`IfcOpeningElement`（IFC4 と IFC4X3 は `OPENING`）を `IfcRelVoidsElement` で壁に結ぶ。空間には所属させない。
  - 開口の `Body` は、幅と高さの矩形を壁の XZ に置き、壁の材料の層の使い方（`AXIS2`）から求めた層の範囲の一方の面から他方の面まで押し出す。層の使い方を持たない要素と空間には、穴をあけない。
  - 建具は、開口・種類（扉か窓）・全体の幅と高さで受ける。開口の配置に置き、`IfcDoor` か `IfcWindow`（IFC4 と IFC4X3 は PredefinedType の `DOOR` か `WINDOW`）を `IfcRelFillsElement` で開口に結び、壁と同じ空間に包含で結ぶ。建具の形と型はまだ書かない。開口でない要素と、もう埋まった開口は断る。
- 意図の値で通り芯を足せる（[#898](https://github.com/zumen-jp/zumen/issues/898)）。Rust の `Edit::Grid` と、`building_apply` の `add_grid`（スキーマのマイナーバージョン 101 から）で受ける。空間・U と V の軸（名前・始点・終点）・高さで受け、空間の配置からの相対に置いて空間に包含で結ぶ。軸は 2 点の `IfcPolyline` を曲線にした `IfcGridAxis`（SameSense は真）に書く。U と V のどちらかが空・同じ名前の軸・始点と終点が同じ軸・有限でない値は断る。
- 意図の値で階の高さ、壁の芯・高さ・層、開口の位置と大きさを直せる（[#898](https://github.com/zumen-jp/zumen/issues/898)）。Rust の `Edit::Elevation`・`Edit::WallShape`・`Edit::OpeningShape` と、`building_apply` の `elevation`・`wall_shape`・`opening_shape`（スキーマのマイナーバージョン 103 から）で受ける。
  - 階の高さは 10 Elevation に書き、階の配置の z を高さの差だけ動かす。今の高さがなければ、配置の z を今の高さとみなす。IFC4.3 では、`Elevation` が書いてあるときだけ直し、なければ配置だけを動かす。階に属する要素は配置の相対で付いてくる。
  - 壁は、足すときと同じ値（芯の 2 点・高さ・層・芯からのずれ）で受け、配置・表現・材料の層の使い方を書き直す。芯の点は壁の配置の親の配置の XY。同じ名前の層は元の `IfcMaterial` を使う。開口と建具は壁の配置からの相対のまま付いてきて、開口の `Body` の押し出しを新しい層の範囲に合わせる。元の層の範囲の遠い面から近い面へ押し出す、ただ 1 つの `IfcExtrudedAreaSolid` でない開口があれば、何も変えずに断る。
  - 開口は、足すときと同じ値（芯に沿う距離・下端・幅・高さ）で受け、配置と `Body` を書き直す。埋める扉と窓の OverallHeight と OverallWidth は、書いてあれば新しい大きさにする。
  - 配置をほかの要素と共有していれば、新しい配置に付け替える。使われなくなった配置と表現は除く。
- 作る変更と直す変更の確かめ方（[#898](https://github.com/zumen-jp/zumen/issues/898)）。
  - `crates/zumen-ifc/tests/author.rs` は、IFC2X3・IFC4・IFC4X3 の文書に、建物・2 つの階・通り芯と各階の壁・床・柱・梁・開口・扉か窓を変更だけで足す。直した壁と開口は、平断面の切断線と開口を引いた体積が付いてくることを、直す前と後で照らす。
  - 適合テストの `building_storeys` は、同じ 2 階建てを 1 つの `building_apply` で作り、建物モデルの要約・要素と `scene3d` の場面を、各環境の全バイトで照らす。
  - `examples/author_contract.rs` は、作った文書と直した文書の IFC のバイト列・建物モデルのハッシュ・各階の平断面・体積と、変更を当てるときの予算停止と中断を記録する。`tests/check_author.py` で、各ネイティブ環境と wasm32-wasip1 で同じ SHA-256 になることを照らす。
  - 毎晩の IfcOpenShell の段は、`author.rs` の `writes_the_authored_files_for_other_readers` が書いた 3 版の作った IFC と直した IFC を、`tests/ifcopenshell/author.py` で IfcOpenShell 0.9.0.post1 の検査（スキーマと EXPRESS の規則）にかけ、種類ごとの数・階の高さと配置・所属・開口と建具の関係・壁の層・開口を引いた壁の体積を照らす。手元では次のように流す。

    ```powershell
    $env:ZUMEN_IFC_AUTHORED = Join-Path (Get-Location) 'target/authored'
    cargo test -p zumen-ifc --locked --test author writes_the_authored_files_for_other_readers
    uv run --no-project --python 3.13 --no-python-downloads --with ifcopenshell==0.9.0.post1 --with pytest==9.2.0 python -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/author.py target/authored
    ```

  - 同じ段で、buildingSMART の ifc-gherkin-rules（MIT、コミット `893f8275049b8960c3ce0c7379b4b57aefeaa8f9`）を別の環境に IfcOpenShell 0.8.5 と入れ、`tests/ifcopenshell/gherkin.py` で同じ 6 つのファイルを検証サービスと同じ規則（実装者の合意と非公式の命題）にかける。データベースを使わない試験の動かし方で、規則ごとの結果を読む。誤りがなく、警告は試験の入力に座標参照系がないことの GRF003 だけで、規則の手順が例外で止まらないことを確かめる。GEM004 は IFC2X3 の識別子の表を規則の側に持たないので、その例外は数えない。IFC4.3 で非推奨の階の `Elevation` を書かないことは、この照合の IFC102 で見つけて直した。手元では、ifc-gherkin-rules を取って入れた環境の Python で次のように流す。

    ```powershell
    <環境の python> -X utf8 -I crates/zumen-ifc/tests/ifcopenshell/gherkin.py <ifc-gherkin-rules のフォルダー> target/authored
    ```

### 新しい IFC を作る

建物モデルから新しい IFC を作るときは、バージョンごとの空のひな形に、`building_apply` と同じ書き手で空間・要素・関係・定義を足します。図面の[新しい DXF への変換](dxf.md#新しい-dxf-への変換)と同じ作りです。実装は [#395](https://github.com/zumen-jp/zumen/issues/395) と [#897](https://github.com/zumen-jp/zumen/issues/897) で進め、この節の規則に合わせます。

- `create` は、バージョン・長さの単位・ヘッダーと所有履歴の値を受けて、ひな形だけを作る。ひな形は、プロジェクト・長さとラジアンの単位・3 次元の `Model` の文脈とその `Body` の下位の文脈（ifc-gherkin-rules の GEM052）を持ち、書き手はこの文脈を使う。長さの単位はミリメートル・センチメートル・メートルと、換算する `inch`・`foot`。所有履歴の ChangeAction は、IFC2X3 では必須なので `ADDED` を書き、IFC4 からは規則 CorrectChangeAction に合わせて書かない。`convert` は、建物モデルへ lift できる文書（別のバージョンの IFC・ST-Bridge など）から、ひな形に足して書く。`convert_estimate` は、同じ書き手で最後まで書き、予定のハッシュ・要素数と、種類と忠実度ごとの件数を返す。どれも bim のモジュールの op にし、Rust の呼び出し元も `zumen-ifc` の建物の側にある同じ関数を呼ぶ。
- 今の `convert` は Rust の関数で、構造だけを書く。敷地・建物・階と、書き手の表にある要素（壁・床・柱・梁・部材・板・被覆・手すり・屋根・階段・斜路・基礎・杭・扉・窓・開口・プロキシ）を、名前・定義された種類・所属・配置・開口と建具の関係と、単一の値のプロパティ（`IfcPropertySingleValue`）とともに書く。書き手の型の表にある型（壁・床・柱・梁・部材・板・被覆・手すり・プロキシの型と、IFC4 からの屋根・階段・斜路・基礎・杭・扉・窓の型）も書き、要素と `IfcRelDefinesByType` で結ぶ。型の必須の PredefinedType は、元になければ `NOTDEFINED` にし、扉と窓の型の必須の OperationType・PartitioningType も `NOTDEFINED` にする。型のプロパティは `HasPropertySets` に書く。数量（`IfcElementQuantity`）も、プロパティと同じく書いた後に GlobalId で引いて書く。値の単位が書いた文書の単位のものだけを書き、IFC2X3 へは式を落とし、IFC4X3 では整数でない個数（`IfcCountMeasure` は整数）を落とす。元の文書の SI の単位（面積・体積・質量・時間など）は、種類ごとに 1 つ単位の割り当てに足し、換算する単位（度など）は落とす。定義された種類は書く前に列挙の綴りだけを確かめ、そのバージョンの列挙にあるかはまだ確かめない。プロパティは、要素を書いた後に GlobalId でインスタンスを引き、`building_apply` と同じ変更で足す。書かなかった祖先からの配置は、書いた祖先まで変換を畳む。階の高さは、IFC2X3 と IFC4 では元の `Elevation`（なければ階の高さ）を書き、IFC4.3 では元の値が親からの配置の z と違えば落とす。書けない要素（`IfcSpace`・表にない型・家具など）は `convert.element.unsupported` で落とし、書いた要素の落とした値（説明などの識別・単一の値でないプロパティ・書けない数量・材料・形・分類など、そのバージョンにない定義された種類）は近似として、値のキーごとに `convert.item.approximated` で数える。書き手が足す関係の GlobalId は、まだ名前空間と足した順から作る。長さの単位がひな形で書けなければ、`convert.element.unsupported` で書かない。
- 書くのは、建物モデルのコンポーネントと、[形を含む lift](#所有する形とハッシュを建物に保持する)が所有する形の値。元の L1 のレコードは写さない。建物モデルにない値（未対応の形の項目、知らない実体と属性）は書かず、`zumen-sdk::conversion` の忠実度と、種類・件数・出所の診断で返す。同じバージョンへの `convert` も同じで、元のバイト列を保つ書き出しは `building_apply` が受け持つ。
- バージョンは IFC2X3 TC1・IFC4 ADD2 TC1・IFC4X3 ADD2 の 3 つ（`FILE_SCHEMA` は `IFC2X3`・`IFC4`・`IFC4X3_ADD2`）で、`create` と `convert` の必須の引数にする。実体と属性の並びは、lift と同じ手書きのバージョンの表に足し、実体ごとにバージョンと節を引用する。スキーマは同梱せず、スキーマから生成もしない。表にない実体は書かない。
- L1 はスキーマを持たないので、IFC のバージョンの変更（IFC2X3 から IFC4 など）は L1 の中で書き分けず、この経路で行う。落ちる値は、ほかの形式への変換と同じく忠実度で示す。
- GlobalId は、建物モデルの ID が GlobalId ならその値を書く。ほかの要素と、書き手が足す関係・セットなどの GlobalId は、名前空間と元の安定したキー（元の要素の ID と、要素・関係・セットの役割）から BLAKE3 で導き、足した数には依らない。名前空間は呼び出し元の値、なければ元の IFC の `IfcProject` の GlobalId、それもなければ元の文書のハッシュにする。名前空間が文書のハッシュでなければ、元の文書の一部を直して書き直しても、直していない要素の GlobalId は変わらない。
- ヘッダーの時刻・作者・組織・作ったソフトと、所有履歴（`IfcOwnerHistory`）の値は、呼び出し元か出力先のプロファイルから受け取り、コアは時計を読まない。ヘッダーの値は、渡されなければ空にする。IFC2X3 は所有履歴が必須なので、値が渡されなければ書かずに診断する。IFC4 と IFC4X3 では、渡されたときだけ書く。
- 既定値・語彙・ビュー（MVD）は、出力先の[プロファイル](../../profiles/README.md)が決める。ヘッダーの `ViewDefinition` は、プロファイルか呼び出し元が渡したときだけ書き、書き手は MVD を推測しない。MVD に合わせて書く範囲を絞る規則も、プロファイルに置く。IFC データ作成基準のような公的な基準は、公表された版を[資料を読む条件](../../formats/README.md#資料を読む条件)に従って読み、既存のプロファイルを変えずに、別の ID のプロファイルとして足す。
- 書いた IFC を lift し直し、建物モデル（ID・クラス・空間の親子・配置・型・プロパティ・数量・所有する形のハッシュ）が元と一致すること、落ちた値が忠実度と診断の件数に一致することを、合成の 3 版と公開サンプルで試験する。ネイティブと wasm のバイト列の一致は、適合テストで照らす。
- 外部の読み手では、IfcOpenShell（LGPL-3.0）で既知の GlobalId・空間の親子・配置・単位を元の値と照らす。buildingSMART の `validate` と `ifc-gherkin-rules`（どちらも MIT）で、スキーマと規則の誤りを調べる。どれも Cargo の外でバージョンを固定して手元と毎晩の CI で動かし（[依存と照合ツール](../../CONTRIBUTING.md#依存と照合ツール)）、使うバージョンを台帳に書く。受け取った図面は外部のサービスに送らない。BIM ソフトで開く確認は [#526](https://github.com/zumen-jp/zumen/issues/526) で行う。
- 書き手が IFC の 1 つだけのうちは、`zumen-sdk` に lower の trait と往復試験のツールを置かず、集計の型だけを共有する。2 つ目の建物の書き手を作るときに、両方から形を決める。

### 建物を作る編集

建物を作る製品のモデルは、作成用の別の形式を作らず、IFC を正として持ちます。開いた IFC か `create` のひな形を文書エンジン（[`IfcLive`](../live.md#依存)）に持ち、`building_apply` で空間・要素・関係を足し・除き・直します。建物モデルは、この IFC から lift し直す読みの側です。実装は [#897](https://github.com/zumen-jp/zumen/issues/897) と [#898](https://github.com/zumen-jp/zumen/issues/898) で進めます。

- 保存するのは IFC そのもので、読んだ IFC と作った IFC を同じに扱う。知らない実体と属性は、`building_apply` が元のバイト列で保つ。作成用のバージョンつきの保存形式と、その移し方は持たない。IFC のバージョンを移すときは、[新しい IFC を作る](#新しい-ifc-を作る)の `convert` で、落ちる値を忠実度で示す。
- 配置・表現・型は、IFC のとおり分けて持つ。配置は親の配置からの相対（`IfcLocalPlacement`）、形は要素の表現と、型の表現を写す `IfcMappedItem`、型は `IfcRelDefinesByType` で結ぶ。建物モデルの `placement`・`representation`・`type` のコンポーネントがそれぞれを読む。
- 壁に付いた窓は、開口（`IfcOpeningElement`）を壁へ `IfcRelVoidsElement`、窓を開口へ `IfcRelFillsElement` で結び、開口と窓の配置を壁の配置からの相対にする。壁を動かせば、開口と窓は配置の相対で付いてくる。壁の厚さのように、付いてくる側の形を変える値を直すときは、文書エンジンの依存辺でたどり、同じ要求の中で開口と窓も書き直す。
- 足す変更は 2 つの段で受ける。建物モデルのコンポーネントの値（配置・型・表現・定義・関係）で足す変更は、新しい IFC の書き手も使う。意図の値（壁なら芯の 2 点・高さ・層か型）で足す変更では、IFC の実体の組（配置・`Axis` と `Body` の表現・材料の層の使い方・所属の関係）をコアがバージョンの表で書く。製品は STEP のインスタンスを組み立てない。
- IFC の実体で表せる意図（壁の芯、`IfcMaterialLayerSetUsage` の層と芯からのずれ、階の高さ、通り芯）は、その実体に書く。階の高さは、IFC2X3 と IFC4 では `Elevation` と配置に、`Elevation` を非推奨にした IFC4.3 では配置だけに書く。表せない意図（壁の上端を上の階に合わせる、など）は、`Pset_` と `Qto_` で始めない zumen の名前空間のプロパティセットに書き、セットの名前にバージョンを持たせる。ほかのソフトがこのセットを読まなくても、形と関係は IFC の実体だけで読める。今受ける意図の値（階・通り芯・壁・床・柱・梁・開口・建具と、階の高さ・壁・開口を直す値）は、どれも IFC の実体で表せるので、このセットはまだ書かない。表せない意図を初めて受けるときに、セットの名前・バージョン・値をここに書く。
- `building_apply` の 1 つの要求を、1 つのトランザクションと取り消しの単位にする。すべての変更を確かめてから反映し、失敗・中断・予算停止では何も変えない。取り消しとやり直しは文書エンジンの履歴で、要求ごとに戻す。製品の 1 つの操作を 1 回で取り消したいときは、1 つの要求にまとめる。
- 足す GlobalId は、今の `building_apply` と同じく文書のハッシュと足した数から作るか、呼び出し元が渡した値にする。一度書いた GlobalId は変えない。
- 形は IFC の表現（押し出し・写像・開口の関係）で持ち、評価には[建物の 3D の形状評価](../architecture.md#建物の-3d-の形状評価)と同じカーネルを使う。B-Rep のカーネルは、メッシュで足りない範囲を測ってから決める。
- 図面の切り出し・3D のシーン・数量は、lift し直した建物モデルから既存の経路で作り、作成用の別の経路を持たない。
- モデル・変更・取り消し・保存・出力は、このリポジトリのコアに置く。画面・入力・操作の途中の状態は、このリポジトリの外の製品が持つ。

## 壊れた所と上限

- 始まりの行・`DATA` の節・`END-ISO-10303-21;` のどれかがなければ、`ifc.file.invalid` で読まない。形の読めない文は次の `;` まで飛ばし、`ifc.record.invalid` にその範囲を付ける。
- 入力のバイト数、レコードの数、値の数と参照の数（`work`）、並びの入れ子の深さ（`depth`）を予算で数える。ifcZIP は、展開した大きさを `decoded-bytes` で、展開する前に数える。
- 建物モデルを作るとき、値を写す大きさを `decoded-bytes` で数える。1 つのセットや材料を多くの要素に結んだり、同じ項目を繰り返し参照したりすると、入力の大きさに比例しないメモリを使うためである（#733）。
  - 数える写し: プロパティと数量のセットと、材料を、覚えるときと、要素に写すとき。セットのプロパティと数量、層の集合の層を、足すとき。単位を、プロパティ・数量・長さの値に写すとき。関係の GlobalId を、要素の出所に写すとき。
  - どれも、1 つのレコードから作った値を、覚える前と足す前に数える。
  - 大きさは、文字のバイトの数に、項目ごとの 32 を足した目安にする。プロパティの多い大きな IFC では、既定の予算（`decoded-bytes` の 2 GiB。ifcZIP は展開した大きさと分け合う）に当たることがある。
  - 要素と、要素に結ぶ定義の参照の組ごとに、`work` を 1 数える。
  - 止まったら写さず、実体を読むループで止まったことを返す。中断の合図と診断の上限で止まったときも同じにする。

## 確かめた度合い

| 形式 | 向き | 段 |
| --- | --- | --- |
| `ifc` | 読み | `external` |
| `ifc` | 書き | `external` |

段の定義は[形式台帳の書き方](../../formats/README.md#確かめた度合い)に、入力と照合ツールのバージョンは[台帳の IFC](README.md#format-ifc)にあります。

- **読み**では、公開 IFC の建物モデルを IfcOpenShell と照らします。対象は実体のクラス・GlobalId・名前・所属先と型・空間変換・開口・プロパティ・数量です。国内の BIM ソフトの公開サンプルは [#513](https://github.com/zumen-jp/zumen/issues/513) で許諾を得て追加します。[公開入力の照合](../../crates/zumen-ifc/tests/ifcopenshell/samples.py)
- **必要時の取得**のうち、公開例の元の属性と参照を IfcOpenShell と照らした範囲が `external` です。[曲線](#曲線の確かめ方)・[形状表現と写像](#形状表現の確かめ方)・[押し出し](#押し出しの確かめ方)・[断面](#断面の共通の取得の確かめ方)・[面の表示](#面の表示の確かめ方)・[通り芯](#通り芯の確かめ方)・[SI 単位](#共有-si-の確かめ方)の各節に、比べた値と手順があります。合成例と IfcOpenShell の照合は `synthetic` です。配置や写像を適用した形、表示や継承の評価は外部照合の範囲外です。
- **自前の検査**では、公開 IFC の無編集の往復と、写さない値の診断を確かめます。判定・インスタンスのグラフ・壊れた所の扱い・ifcZIP は外部照合していません。壊れた入力と ifcZIP は合成例で検査します。階・区域・分類の値・出所・診断・予算も合成 3 版の `synthetic` です。公開例の階と分類も読みます。成分の応答と IDS の分類の照合はネイティブと wasm で比べます。[公開入力の往復](../../crates/zumen-ifc/tests/samples.rs)
- **書き**では、名前・説明・プロパティを `building_apply` で変えた IFC4・IFC2X3 を毎晩 IfcOpenShell で開き、変更後の値と、スキーマ・EXPRESS の規則の誤りが増えないことを確かめます。[検証キット](../../xtask/verify/ifc/README.md)の生成と再読み込みは `synthetic` です。BIM ソフトで開く確認は [#526](https://github.com/zumen-jp/zumen/issues/526) に残っています。[書き出しの照合](../../crates/zumen-ifc/tests/ifcopenshell/check.py)
