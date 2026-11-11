<!-- formats/ledger.toml から cargo xtask codegen で生成。直接編集しない。 -->

# 形式と権利の一覧

[権利の台帳](../../formats/ledger.toml)から生成しています。ここにあるのは実装を置く方針であり、読み書きできる形式の一覧ではありません。変更の手順は [台帳の書き方](../../formats/README.md)、図面の要素の対応は [Drawing2D の対応表](drawing2d-mapping.md)を参照してください。

- 権利: `public` は実装の公開に第三者の許諾が不要、`permission` は契約・会員・許諾が必要、`unknown` は条件が未確認です。
- 置き場所: `core` はコア、`plugin` は外部のプラグイン、`none` は実装しない方針です。
- 読み・書き: コアの実装を確かめた度合い（[台帳の書き方](../../formats/README.md#確かめた度合い)）です。`synthetic` は、仕様とテストの中で作ったファイル（書きは自前の読み取り）だけで確かめたものです。公開サンプルや受け取った図面を読んでも、値を照らしていなければ `synthetic` です。`external` は、自前で作っていないファイルを読み、外部の実装が読んだ値かファイルに添えられた値と照らしたものです（書きは、書いたファイルを外部の実装が読み、書いた値と合ったもの）。`application` は、その形式の基準になるソフトで開いて確かめたものです。確かめた方法のうちもっとも強いものを示すので、その方法で確かめたのは一部の規則だけのことがあります。範囲は形式ごとの記録と、形式の文書の「確かめた度合い」の節にあります。空欄は、その向きをコアで実装していないことを示します。
- 台帳は判断した時点の記録です。URL の到達や権利の変更は、生成時に再確認していません。

## 一覧

| キー | 名前 | 系統 | 領域 | 権利 | 置き場所 | 読み | 書き |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [bcf](#format-bcf) | BCF | bcf | review | `public` | `core` | `synthetic` | `synthetic` |
| [bebridge](#format-bebridge) | BE-Bridge | bebridge | building | `unknown` | `none` |  |  |
| [boring](#format-boring) | ボーリング交換用データ（BED） | boring | delivery | `public` | `core` | `synthetic` |  |
| [cedxm](#format-cedxm) | CEDXM | cedxm | building | `permission` | `none` |  |  |
| [citygml](#format-citygml) | CityGML（PLATEAU の 3D 都市モデルと i-UR を含む） | citygml | geo | `public` | `core` | `synthetic` |  |
| [csv](#format-csv) | CSV | csv | delivery | `public` | `core` | `synthetic` |  |
| [delivery](#format-delivery) | 電子納品の管理ファイル（INDEX\_C.XML・PHOTO.XML など） | delivery | delivery | `public` | `core` | `synthetic` |  |
| [dwg](#format-dwg) | DWG | acad | drawing | `public` | `core` | `application` | `application` |
| [dxf](#format-dxf) | DXF | acad | drawing | `public` | `core` | `application` | `application` |
| [e57](#format-e57) | E57（ASTM E2807） | e57 | pointcloud | `public` | `core` | `synthetic` |  |
| [express](#format-express) | EXPRESS のスキーマ（ISO 10303-11。IFC のスキーマなど） | express | building | `public` | `core` | `synthetic` |  |
| [extdm](#format-extdm) | 拡張 DM（公共測量標準図式数値地形図データファイル） | extdm | geo | `public` | `core` | `synthetic` |  |
| [fgd](#format-fgd) | 基盤地図情報（GML） | fgd | geo | `public` | `core` | `synthetic` |  |
| [ids](#format-ids) | IDS（Information Delivery Specification） | ids | building | `public` | `core` | `synthetic` |  |
| [ifc](#format-ifc) | IFC | ifc | building・civil | `public` | `core` | `external` | `external` |
| [isg](#format-isg) | ISG 形式（ジオイド高と基準面補正量の格子） | isg | geo | `public` | `core` | `synthetic` |  |
| [jpeg](#format-jpeg) | 工事写真（JPEG と Exif・XMP） | jpeg | delivery | `public` | `core` | `synthetic` |  |
| [jwc](#format-jwc) | JWC（Jw\_cad for DOS の図面） | jwcad | drawing | `public` | `core` |  |  |
| [jws](#format-jws) | JWS（Jw\_cad の図形） | jwcad | drawing | `public` | `core` |  |  |
| [jww](#format-jww) | JWW（Jw\_cad for Windows の図面） | jwcad | drawing | `public` | `core` | `synthetic` | `external` |
| [kokuban](#format-kokuban) | 電子小黒板の情報と信憑性確認（改ざん検知） | jpeg | delivery | `permission` | `none` |  |  |
| [landxml](#format-landxml) | J-LandXML（LandXML 1.2 に準じた 3 次元設計データ交換標準） | landxml | civil | `public` | `core` | `synthetic` |  |
| [las](#format-las) | LAS・LAZ | las | pointcloud | `public` | `core` | `external` |  |
| [par](#format-par) | TKY2JGD・PatchJGD のパラメーターのファイル（座標の補正量の格子） | par | geo | `public` | `core` | `synthetic` |  |
| [pdf](#format-pdf) | PDF | pdf | drawing | `public` | `core` | `external` | `synthetic` |
| [ply](#format-ply) | PLY（点群） | ply | pointcloud | `public` | `core` | `external` |  |
| [png](#format-png) | PNG | png | drawing | `public` | `core` |  | `external` |
| [saf](#format-saf) | SXF の属性ファイル（SAF） | sxf | drawing・delivery | `public` | `core` | `synthetic` |  |
| [sima](#format-sima) | SIMA 共通フォーマット | sima | civil | `unknown` | `none` |  |  |
| [stbridge](#format-stbridge) | ST-Bridge | stbridge | building | `permission` | `plugin` |  |  |
| [stem](#format-stem) | Stem（設備機器ライブラリデータ交換仕様） | stem | building | `unknown` | `none` |  |  |
| [svg](#format-svg) | SVG | svg | drawing | `public` | `core` |  | `external` |
| [sxf](#format-sxf) | SXF（SFC・P21 と、それを ZIP に入れた SFZ・P2Z） | sxf | drawing | `public` | `core` | `synthetic` | `external` |
| [xlsx](#format-xlsx) | Excel の表（xlsx） | xlsx | delivery | `public` | `core` | `synthetic` |  |
| [xyz](#format-xyz) | XYZ（座標を並べた点群のテキスト） | xyz | pointcloud | `public` | `core` | `synthetic` |  |

## 仕様の入手先

### spec-bcf_api_3_0

- 名前: BCF-API
- 公開元: buildingSMART International
- 版: 3.0（v3.0、2021-06-18）
- 入手の条件: 無償公開
- 入手先: [BCF-API](https://github.com/buildingSMART/BCF-API/tree/release_3_0)
- 確認日: 2026-10-31
- 補足: 文書と JSON のスキーマは CC BY-ND 4.0。実装についての記述はない。2.1 のブランチには、ライセンスの記述がない

### spec-bcf_xml_2_1

- 名前: BCF-XML
- 公開元: buildingSMART International
- 版: 2.1（v2.1、2017-01-16）
- 入手の条件: 無償公開
- 入手先: [BCF-XML](https://github.com/buildingSMART/BCF-XML/tree/release_2_1)
- 確認日: 2026-10-31
- 補足: ブランチ release\_2\_1 には、ライセンスの記述がない。buildingSMART の今の説明は BCF 全体を CC BY-ND 4.0 とし、BCF-XML の LICENSE は 2022-03 に CC BY-NC-ND 4.0 から CC BY-ND 4.0 へ直された。zumen-jp/zumen#440 で、3.0 と同じ扱いで実装してよいと判断した（2026-11-02）

### spec-bcf_xml_3_0

- 名前: BCF-XML
- 公開元: buildingSMART International
- 版: 3.0（v3.0、2021-06-18）
- 入手の条件: 無償公開
- 入手先: [BCF-XML](https://github.com/buildingSMART/BCF-XML/tree/release_3_0)
- 確認日: 2026-10-31
- 補足: 文書と XSD は CC BY-ND 4.0。実装についての記述はない

### spec-bebridge_6_1

- 名前: 設備 CAD データ交換仕様 BE-Bridge
- 公開元: C-CADEC（一般財団法人建設業振興基金）
- 版: Ver.6.1（2014-04）
- 入手の条件: 無償公開
- 入手先: [設備 CAD データ交換仕様 BE-Bridge](https://www.kensetsu-kikin.or.jp/assets/img/database/database27.pdf)
- 確認日: 2026-10-24
- 補足: 最新の版ではない。最終の版は BE-Bridge Ver.7・Stem Ver.10（どちらも 2015-04）。管理は C-CADEC から建築保全センター（2015-04）、BIM ライブラリーコンソーシアム（2015-10）、BIM ライブラリ技術研究組合（BLCJ、2019-08）へ移り、BLCJ は 2026-03-30 に解散を議決した。解散の後の権利の行き先と、Ver.7 の入手先は見つからなかった（2026-11-02）

### spec-cedxm

- 名前: CEDXM ファイルフォーマット
- 公開元: 特定非営利活動法人シーデクセマ評議会
- 入手の条件: 会員などに限定
- 入手先: [CEDXM ファイルフォーマット](https://www.cedxm.com/)
- 確認日: 2026-10-24
- 補足: 会員にだけ配る。版は確かめられない

### spec-citygml_2_0

- 名前: OGC City Geography Markup Language (CityGML) Encoding Standard（OGC 12-019）
- 公開元: Open Geospatial Consortium
- 版: 2.0（2012-04-04）
- 入手の条件: 無償公開
- 入手先: [OGC City Geography Markup Language (CityGML) Encoding Standard（OGC 12-019）](https://docs.ogc.org/is/12-019/12-019.pdf)
- 確認日: 2026-11-01

### spec-citygml_3_0_gml

- 名前: OGC City Geography Markup Language (CityGML) 3.0 Part 2: GML Encoding Standard（OGC 21-006r2）
- 公開元: Open Geospatial Consortium
- 版: 3.0（2023-06-20）
- 入手の条件: 無償公開
- 入手先: [OGC City Geography Markup Language (CityGML) 3.0 Part 2: GML Encoding Standard（OGC 21-006r2）](https://docs.ogc.org/is/21-006r2/21-006r2.html)
- 確認日: 2026-11-01
- 補足: 概念モデルは Part 1（OGC 20-010）

### spec-dxf_2026

- 名前: DXF Reference
- 公開元: Autodesk
- 版: AutoCAD 2026
- 入手の条件: 無償公開
- 入手先: [DXF Reference](https://help.autodesk.com/cloudhelp/2026/ENU/AutoCAD-DXF/files/GUID-235B22E0-A567-4CF6-92D3-38A2306D73F3.htm)
- 確認日: 2026-10-24

### spec-e57_astm_e2807

- 名前: ASTM E2807 Standard Specification for 3D Imaging Data Exchange, Version 1.0
- 公開元: ASTM International
- 版: E2807-11(2026)（2026-06-05）
- 入手の条件: 有償
- 入手先: [ASTM E2807 Standard Specification for 3D Imaging Data Exchange, Version 1.0](https://store.astm.org/e2807-11r26.html)
- 確認日: 2026-11-01
- 補足: 買う。販売のページは、規格を AI で扱うことを禁じる。本文は読まない

### spec-ecma_376_5

- 名前: ECMA-376 Office Open XML File Formats
- 公開元: Ecma International
- 版: 第 5 版（Part 1 は 2016-12、Part 2 は 2021-12）
- 入手の条件: 無償公開
- 入手先: [ECMA-376 Office Open XML File Formats](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- 確認日: 2026-11-01

### spec-ed_boring_r8_2

- 名前: 地質・土質調査成果電子納品要領（ボーリング交換用データ BED0500.DTD を含む）
- 公開元: 国土交通省
- 版: 令和 8 年 2 月
- 入手の条件: 無償公開
- 入手先: [地質・土質調査成果電子納品要領（ボーリング交換用データ BED0500.DTD を含む）](https://www.cals-ed.go.jp/cri_point/)
- 確認日: 2026-11-01
- 補足: DTD は <https://www.cals-ed.go.jp/cri_dtdxml/> の boring\_R8.2.zip に入っている

### spec-ed_const_r8_2

- 名前: 工事完成図書の電子納品等要領
- 公開元: 国土交通省
- 版: 令和 8 年 2 月（令和 8 年 4 月 1 日から契約する工事に適用）
- 入手の条件: 無償公開
- 入手先: [工事完成図書の電子納品等要領](https://www.cals-ed.go.jp/cri_point/)
- 確認日: 2026-11-01
- 補足: 管理ファイルの DTD と作成例は <https://www.cals-ed.go.jp/cri_dtdxml/> に置く。頁の転載と改変を禁じるので、DTD は同梱しない

### spec-ed_design_r8_2

- 名前: 土木設計業務等の電子納品要領
- 公開元: 国土交通省
- 版: 令和 8 年 2 月
- 入手の条件: 無償公開
- 入手先: [土木設計業務等の電子納品要領](https://www.cals-ed.go.jp/cri_point/)
- 確認日: 2026-11-01

### spec-ed_eizen_r3

- 名前: 営繕工事電子納品要領・建築設計業務等電子納品要領
- 公開元: 国土交通省大臣官房官庁営繕部
- 版: 令和 3 年改定
- 入手の条件: 無償公開
- 入手先: [営繕工事電子納品要領・建築設計業務等電子納品要領](https://www.mlit.go.jp/gobuild/gobuild_tk2_000017.html)
- 確認日: 2026-11-01
- 補足: 管理ファイルの DTD の置き場所は確かめられない

### spec-ed_photo_r5_3

- 名前: デジタル写真管理情報基準
- 公開元: 国土交通省
- 版: 令和 5 年 3 月（正誤の修正 2023-12-01）
- 入手の条件: 無償公開
- 入手先: [デジタル写真管理情報基準](https://www.cals-ed.go.jp/cri_point/)
- 確認日: 2026-11-01

### spec-exif_3_1

- 名前: CIPA DC-008 Exchangeable image file format for digital still cameras: Exif
- 公開元: 一般社団法人カメラ映像機器工業会（CIPA）
- 版: DC-008-2026（Exif 3.1、2026-01-30）
- 入手の条件: 無償公開
- 入手先: [CIPA DC-008 Exchangeable image file format for digital still cameras: Exif](https://www.cipa.jp/e/std/std-sec.html)
- 確認日: 2026-11-01
- 補足: XMP での Exif は CIPA DC-010-2026

### spec-extdm_a1_291

- 名前: 拡張ディジタルマッピング実装規約（案）改訂版（技術資料 A1-No.291）
- 公開元: 国土地理院
- 版: 平成 17 年 3 月
- 入手の条件: 無償公開
- 入手先: [拡張ディジタルマッピング実装規約（案）改訂版（技術資料 A1-No.291）](https://www.gsi.go.jp/common/000258819.pdf)
- 確認日: 2026-11-01
- 補足: 平成 20 年の作業規程の準則の改正で「公共測量標準図式数値地形図データファイル仕様」に改めた。国土地理院の技術資料は、特段の記載がなければ PDL 1.0 を準用する

### spec-extdm_furoku7

- 名前: 作業規程の準則の付録 7「公共測量標準図式」（数値地形図データファイル仕様を含む）
- 公開元: 国土地理院
- 版: 2026-02 の更新
- 入手の条件: 無償公開
- 入手先: [作業規程の準則の付録 7「公共測量標準図式」（数値地形図データファイル仕様を含む）](https://www.gsi.go.jp/gijyutukanri/gijyutukanri41018.html)
- 確認日: 2026-11-02
- 補足: 拡張 DM の今の正本。A1-No.291 の実装規約を改めたもの。技術資料は、特段の記載がなければ PDL 1.0 を準用する

### spec-fgd_dl_4_1

- 名前: 基盤地図情報ダウンロードデータファイル仕様書
- 公開元: 国土地理院
- 版: Ver4.1（2023-03-31）
- 入手の条件: 無償公開
- 入手先: [基盤地図情報ダウンロードデータファイル仕様書](https://fgd.gsi.go.jp/otherdata/spec/FGD_DLFileSpecV4.1.pdf)
- 確認日: 2026-11-01
- 補足: GML のスキーマ（JPGIS2014）も同じ所に置く。データの入手には利用者の登録が要る

### spec-fgd_dl_5_2

- 名前: 基盤地図情報ダウンロードデータファイル仕様書
- 公開元: 国土地理院
- 版: 第 5.2 版（2025-07）
- 入手の条件: 無償公開
- 入手先: [基盤地図情報ダウンロードデータファイル仕様書](https://service.gsi.go.jp/kiban/app/help/)
- 確認日: 2026-11-02
- 補足: 2025-04-01 に fgd.gsi.go.jp から service.gsi.go.jp/kiban へ移った。2025-07-31 からのデータが使う版で、座標参照系が JGD2011 から JGD2024 に変わる。GML のスキーマは Ver5.1（FGD\_GMLSchemaV5.1.zip）に対応する。service.gsi.go.jp はこの作業の環境に 403 を返すので、Internet Archive の 2025-08-14 の写しで確かめた。第 5.3 版もある

### spec-fgd_dl_5_3

- 名前: 基盤地図情報ダウンロードデータファイル仕様書
- 公開元: 国土地理院
- 版: 第 5.3 版（2026-07-31）
- 入手の条件: 無償公開
- 入手先: [基盤地図情報ダウンロードデータファイル仕様書](https://service.gsi.go.jp/kiban/app/help/)
- 確認日: 2026-11-03
- 補足: DEM 種別に 1m メッシュ（標高）を、DEM 構成点種別に海水底面・内水底面を足した版。GML のスキーマは Ver5.1（FGD\_GMLSchemaV5.1.zip）のまま。service.gsi.go.jp はこの作業の環境に 403 を返すので、Internet Archive の 2026-10-25 の写しで確かめた

### spec-ids_1_0

- 名前: Information Delivery Specification（IDS）
- 公開元: buildingSMART International
- 版: 1.0.0（2024-06-03）
- 入手の条件: 無償公開
- 入手先: [Information Delivery Specification（IDS）](https://github.com/buildingSMART/IDS)
- 確認日: 2026-10-31
- 補足: 文書と XSD は CC BY-ND 4.0。実装についての記述はない

### spec-ifc_2x3_tc1

- 名前: IFC2x Edition 3 Technical Corrigendum 1
- 公開元: buildingSMART International
- 版: IFC2x3 TC1（2007）
- 入手の条件: 無償公開
- 入手先: [IFC2x Edition 3 Technical Corrigendum 1](https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/)
- 確認日: 2026-10-31
- 補足: 文書の著作権の注記は CC BY-NC-ND 4.0 で、ライセンスの外の使い方には事前の書面の同意が要るとする。実装についての記述はない。2026-10-31 はサイトが自動の取得を拒み、Internet Archive の 2026-03-27 の写しで確かめた。zumen-jp/zumen#440 で、文書もスキーマも同梱・改変せず、実体と属性の並びなどの事実だけを実装に使うので、この注記の条件に触れないと判断した

### spec-ifc_4_0_2_1

- 名前: Industry Foundation Classes Version 4.0 - Addendum 2 - Technical Corrigendum 1
- 公開元: buildingSMART International
- 版: IFC 4.0.2.1（IFC4 ADD2 TC1。ISO 16739-1:2018 と同じ内容）
- 入手の条件: 無償公開
- 入手先: [Industry Foundation Classes Version 4.0 - Addendum 2 - Technical Corrigendum 1](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/)
- 確認日: 2026-10-31
- 補足: 文書の著作権の注記は CC BY-NC-ND 4.0 で、ライセンスの外の使い方には事前の書面の同意が要るとする。実装についての記述はない。2026-10-31 はサイトが自動の取得を拒み、Internet Archive の 2026-10-12 の写しで確かめた。zumen-jp/zumen#440 で、文書もスキーマも同梱・改変せず、実体と属性の並びなどの事実だけを実装に使うので、この注記の条件に触れないと判断した

### spec-ifc_4_3_2

- 名前: IFC 4.3.2 Documentation
- 公開元: buildingSMART International
- 版: IFC 4.3.2.0（ISO 16739-1:2024 と同じ構造と意味）
- 入手の条件: 無償公開
- 入手先: [IFC 4.3.2 Documentation](https://ifc43-docs.standards.buildingsmart.org/)
- 確認日: 2026-10-24

### spec-isg_2_0

- 名前: The ISG-format (version 2.0)
- 公開元: International Service for the Geoid
- 版: 2.0（2020-06-25）
- 入手の条件: 無償公開
- 入手先: [The ISG-format (version 2.0)](https://www.isgeoid.polimi.it/Geoid/ISG_format_v20_20200625.pdf)
- 確認日: 2026-10-31

### spec-iso_10303_11_2004

- 名前: ISO 10303-11:2004 Industrial automation systems and integration — Product data representation and exchange — Part 11: Description methods: The EXPRESS language reference manual
- 公開元: ISO
- 版: 第 2 版（2004-11）。2025 年の見直しで確認され、今も有効
- 入手の条件: 有償
- 入手先: [ISO 10303-11:2004 Industrial automation systems and integration — Product data representation and exchange — Part 11: Description methods: The EXPRESS language reference manual](https://www.iso.org/standard/38047.html)
- 確認日: 2026-11-09
- 補足: ISO が PDF と冊子で売る（CHF 227）。販売のページに、実装を制限する記述はない。ISO のサイトに共通の注記は、ISO Open data のものを除き、ISO の内容を機械学習・AI に使うこと（モデルの学習と、AI のツールに入れて応答を作らせること）を禁じ、出版物の使用と複製に書面の許諾が要るとする。本文は買わず、読まない（zumen-jp/zumen#529）。この項の書誌の事実と注記の中身は、2026-11-09 にエージェントが AI のツールで ISO のページを読んで得た（作った側が www.iso.org をブラウザーの拡張で、zumen-jp/zumen#666 の確かめる側が committee.iso.org を取得のツールで。www.iso.org は自動の取得を拒む）。エージェントが ISO のページを読むことの扱いは zumen-jp/zumen#669 で決め、読まないと決まれば、この項も #669 の判断に従って直す

### spec-iso_32000_2_2020

- 名前: ISO 32000-2:2020（PDF 2.0）
- 公開元: ISO
- 版: 2020
- 入手の条件: 無償公開
- 入手先: [ISO 32000-2:2020（PDF 2.0）](https://pdfa.org/sponsored-standards/)
- 確認日: 2026-10-24
- 補足: PDF Association が、後援の企業の費用で無償で配っている。ISO からは買う

### spec-jlandxml_1_8

- 名前: LandXML 1.2 に準じた 3 次元設計データ交換標準（案）（J-LandXML）
- 公開元: 一般社団法人 OCF（監修: 国土技術政策総合研究所）
- 版: Ver.1.8（令和 8 年 7 月）
- 入手の条件: 無償公開
- 入手先: [LandXML 1.2 に準じた 3 次元設計データ交換標準（案）（J-LandXML）](https://ocf.or.jp/j-landxml/standard/)
- 確認日: 2026-11-01
- 補足: PDF に XSD の本文を載せる。著作権と複製の記述は見当たらない。国土交通省も同じ PDF を載せる

### spec-jpeg_t81

- 名前: ITU-T T.81 Digital compression and coding of continuous-tone still images
- 公開元: ITU-T
- 版: 09/92
- 入手の条件: 有償
- 入手先: [ITU-T T.81 Digital compression and coding of continuous-tone still images](https://www.itu.int/rec/T-REC-T.81)
- 確認日: 2026-11-01
- 補足: ITU の頁は有償とする。JPEG の展開は許容のライセンスの crate に任せる

### spec-jww_7_02

- 名前: Jw\_cad のデータ形式（jwdatafmt.txt）
- 公開元: Jw\_cad
- 版: Jw\_cad 7.02（データの版 700）
- 入手の条件: 無償公開
- 入手先: [Jw\_cad のデータ形式（jwdatafmt.txt）](https://www.jwcad.net/jwdatafmt.txt)
- 確認日: 2026-10-24

### spec-kokuban_jcomsia

- 名前: デジタル工事写真の小黒板情報連携機能の実装規約と信憑性確認（改ざん検知機能）
- 公開元: 一般社団法人施工管理ソフトウェア産業協会（J-COMSIA）
- 版: Ver.1.1（版の日付は確かめられない）
- 入手の条件: 会員などに限定
- 入手先: [デジタル工事写真の小黒板情報連携機能の実装規約と信憑性確認（改ざん検知機能）](https://www.jcomsia.org/kokuban)
- 確認日: 2026-11-01
- 補足: 公開の文書は見つからない。協会が検定を受けるソフトの作り手に出すものとみられる（推定）

### spec-las_1_4_r15

- 名前: LAS Specification
- 公開元: ASPRS（American Society for Photogrammetry and Remote Sensing）
- 版: 1.4 R15（2019-07-09）
- 入手の条件: 無償公開
- 入手先: [LAS Specification](https://github.com/ASPRSorg/LAS)
- 確認日: 2026-11-01
- 補足: OGC のコミュニティ標準でもある。リポジトリにライセンスの記述はない

### spec-laz_1_4

- 名前: LAZ Specification 1.4（OGC 24-070r1）
- 公開元: Open Geospatial Consortium
- 版: 1.4（2026-09-03 公表）
- 入手の条件: 無償公開
- 入手先: [LAZ Specification 1.4（OGC 24-070r1）](https://docs.ogc.org/cs/24-070r1/24-070r1.pdf)
- 確認日: 2026-11-01
- 補足: OGC の公表は、誰でも入手して実装できるとする

### spec-ods_dwg_5_4_1

- 名前: Open Design Specification for .dwg files
- 公開元: Open Design Alliance
- 版: 5.4.1
- 入手の条件: 無償公開
- 入手先: [Open Design Specification for .dwg files](https://opendesign.com/files/guestdownloads/OpenDesign_Specification_for_.dwg_files.pdf)
- 確認日: 2026-10-24

### spec-plateau_5_1

- 名前: 3D 都市モデル標準製品仕様書（i-UR の拡張を含む）
- 公開元: 国土交通省都市局
- 版: 第 5.1 版（2026-03-31）
- 入手の条件: 無償公開
- 入手先: [3D 都市モデル標準製品仕様書（i-UR の拡張を含む）](https://www.mlit.go.jp/plateau/libraries/handbooks/)
- 確認日: 2026-11-01
- 補足: サイトの内容は PDL 1.0。CityGML 3.0 に合わせた応用スキーマの案も公開している

### spec-png_3

- 名前: Portable Network Graphics (PNG) Specification (Third Edition)
- 公開元: W3C
- 版: W3C Recommendation 24 June 2025
- 入手の条件: 無償公開
- 入手先: [Portable Network Graphics (PNG) Specification (Third Edition)](https://www.w3.org/TR/png-3/)
- 確認日: 2026-11-06
- 補足: W3C が公開する勧告。Abstract は patent-free と記し、W3C Patent Policy に従う。仕様と参照コードの本文は同梱しない

### spec-rfc_4180

- 名前: RFC 4180 Common Format and MIME Type for Comma-Separated Values (CSV) Files
- 公開元: IETF
- 版: 2005-10
- 入手の条件: 無償公開
- 入手先: [RFC 4180 Common Format and MIME Type for Comma-Separated Values (CSV) Files](https://www.rfc-editor.org/rfc/rfc4180)
- 確認日: 2026-11-01

### spec-seikakaitei_r7_4

- 名前: 公共測量成果改定マニュアル（国土地理院技術資料 A1-No.337）
- 公開元: 国土地理院
- 版: 令和 7 年 4 月
- 入手の条件: 無償公開
- 入手先: [公共測量成果改定マニュアル（国土地理院技術資料 A1-No.337）](https://www.gsi.go.jp/shidou/data/seikakaiteimanual.pdf)
- 確認日: 2026-11-09
- 補足: PatchJGD とその補正パラメーターで公共測量成果を補正する方法を定める。第 3 条の解説は、補正パラメーターの格子点を 3 次メッシュのコードの末位 2 桁が 00・05・50・55 のメッシュの南西の隅（緯度 150 秒・経度 225 秒の倍数）とし、補正量を秒の緯度差・経度差とメートルの標高差で表すとする。同じ地区に地震ごとのファイルが複数あれば、成果を得た時期より後のものを古い年代順に当てるとし、どのファイルを当てるかは利用者が選ぶとする。パラメーターのファイルの書式（ヘッダー・列・文字コード）は示さない（第 14 条などの入力フォーマットは、補正する点の一覧の書式）。2026-11-09 に確かめた国土地理院の技術資料の一覧は、この資料に当てはめる利用規約を区分 2 の国土地理院コンテンツ利用規約とする。一覧は <https://www.gsi.go.jp/REPORT/TECHNICAL/gsigijutsu1.htm> にある

### spec-stbridge_2_1_1

- 名前: ST-Bridge XML ファイル仕様書
- 公開元: buildingSMART Japan
- 版: ver.2.1.1（2026-04-15。公開は 2026-07-29）
- 入手の条件: 無償公開
- 入手先: [ST-Bridge XML ファイル仕様書](https://www.building-smart.or.jp/meeting/buildall/structural-design/)
- 確認日: 2026-10-24

### spec-svg_1_1

- 名前: Scalable Vector Graphics (SVG) 1.1 (Second Edition)
- 公開元: W3C
- 版: W3C Recommendation 16 August 2011
- 入手の条件: 無償公開
- 入手先: [Scalable Vector Graphics (SVG) 1.1 (Second Edition)](https://www.w3.org/TR/SVG11/)
- 確認日: 2026-11-06
- 補足: 公開された W3C 勧告。Status of This Document は W3C Patent Policy (5 February 2004) に従う作業と記す。リンク先の文書ライセンスは、実装のために一部をソフトウェアへ組み込むことを認める。仕様の本文・コードは同梱しない

### spec-sxf_3_1

- 名前: SXF Ver.3.1 仕様書
- 公開元: 国土交通省
- 版: Ver.3.1 第 2 版（2009-06）
- 入手の条件: 無償公開
- 入手先: [SXF Ver.3.1 仕様書](https://www.cals-ed.go.jp/sxf_ver3-1_specification_draft/)
- 確認日: 2026-11-02
- 補足: 本体は LZH のアーカイブ（sxf\_v31\_r2.lzh）で、概要編・STEP AP202 サブセット編・フィーチャ仕様編・共通既定義要素編・附属書（SFC 編・共通属性セット編・属性付加機構編）・実装規約の PDF を収める。属性ファイル（SAF）は属性付加機構編が定める。サイトの規約は無断の転載と改変を禁じ、HTML 以外のファイルへの直接のリンクを控えるよう求めるが、実装を制限する記述はない

### spec-sxf_display_r7_12

- 名前: SXF 表示機能及び確認機能要件書（案）
- 公開元: 国土交通省
- 版: 令和 7 年 12 月版
- 入手の条件: 無償公開
- 入手先: [SXF 表示機能及び確認機能要件書（案）](https://www.cals-ed.go.jp/sxf_ver3-1_specification_draft/)
- 確認日: 2026-10-24
- 補足: 2026-10-24 はサイトにつながらず、Internet Archive の 2026-02-04 の写しで確かめた

### spec-tky2jgd_2001

- 名前: 世界測地系移行のための座標変換ソフトウェア“TKY2JGD”
- 公開元: 国土地理院
- 版: 国土地理院時報 No.97（2001）、31〜57 ページ。著者は飛田幹男
- 入手の条件: 無償公開
- 入手先: [世界測地系移行のための座標変換ソフトウェア“TKY2JGD”](https://www.gsi.go.jp/common/000040231.pdf)
- 確認日: 2026-11-09
- 補足: パラメーターのファイル（TKY2JGD.par）の構造を表 2 に、例を表 1 に載せる。ヘッダーは 2 行。データの行は、3 次メッシュのコード（8 バイト）と緯度・経度の補正量（秒、9 バイトずつ）を半角の空白で区切る。格子の間隔は緯度 30 秒・経度 45 秒で、格子点は 3 次メッシュの南西の隅。補間は、点を囲む 4 つの格子点の双一次（3.3 節）。本文は写さない

### spec-xmp_2012

- 名前: XMP Specification Part 1
- 公開元: Adobe
- 版: 2012-04
- 入手の条件: 無償公開
- 入手先: [XMP Specification Part 1](https://github.com/adobe/XMP-Toolkit-SDK)
- 確認日: 2026-11-01
- 補足: XMP Specification Public Patent License で出している。ISO 16684-1:2019 は有償

## 形式ごとの根拠と記録

### format-bcf

- 名前: BCF
- 権利の根拠: 3.0 の文書と XSD・JSON のスキーマは CC BY-ND 4.0 で、実装についての記述はない（IFC 4.3 の文書と同じ扱い）。スキーマは同梱しない。2.1 にはライセンスの記述がないが、3.0 と同じ扱いにする（zumen-jp/zumen#440）。商標の注記（buildingSMART・openBIM・IFC）は BCF を挙げていない
- 仕様: [bcf\_xml\_2\_1](#spec-bcf_xml_2_1)、[bcf\_xml\_3\_0](#spec-bcf_xml_3_0)、[bcf\_api\_3\_0](#spec-bcf_api_3_0)
- 実装の根拠: spec
- 権利の根拠にした文書: [IFC4 ADD2 TC1 の文書の著作権の注記（商標の注記を含む）](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/copyright.htm)（確認日: 2026-10-31）
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: BCF-XML 2.1・3.0 の文書と、テストの中で作った bcfzip（crates/zumen-bcf/tests）で確かめた。公開サンプル・外部の実装・BCF を読むソフトでの照合はまだ（#522）
- 書きの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 書きの確かめた範囲: 書いた bcfzip を読み直して確かめた。書いたファイルを、外部の実装や BCF を読むソフトで読ませてはいない（#522）
- issue: [zumen-jp/zumen#429](https://github.com/zumen-jp/zumen/issues/429)
- issue: [zumen-jp/zumen#454](https://github.com/zumen-jp/zumen/issues/454)
- 形式の文書: [docs/formats/bcf.md](../../docs/formats/bcf.md)

### format-bebridge

- 名前: BE-Bridge
- 権利の根拠: Ver.6.1 の仕様書は公開されていて、条件の記述はない。2015-04 に建築保全センターが Stem・BE-Bridge を承継した。承継した側の条件と、最新の版の入手先は見つからなかった。管理を引き継いだ BLCJ は 2026-03-30 に解散を議決し、権利の行き先が分からない。清算人に問い合わせるまで作らない（zumen-jp/zumen#401）
- 仕様: [bebridge\_6\_1](#spec-bebridge_6_1)
- 権利の根拠にした文書: [BIM ライブラリーコンソーシアム「BLC-BIM オブジェクト標準（Version1.0）報告書」（2018-10-04）の「1.4 Stem、BE-Bridge 等の承継」](https://blcj.or.jp/wp/wp-content/uploads/2020/03/blcj_bim_standard_v1.pdf)（確認日: 2026-10-24）
- issue: [zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)
- issue: [zumen-jp/zumen#401](https://github.com/zumen-jp/zumen/issues/401)

### format-boring

- 名前: ボーリング交換用データ（BED）
- 権利の根拠: 国土交通省が要領と DTD を無償で公開している。実装を制限する記述はない。DTD は転載が禁じられているので同梱しない
- 仕様: [ed\_boring\_r8\_2](#spec-ed_boring_r8_2)
- 実装の根拠: spec
- 権利の根拠にした文書: [KuniJiban の利用規約](https://www.kunijiban.pwri.go.jp/jp/terms.html)（確認日: 2026-11-01）
- サンプルの出どころ: 国土地盤情報検索サイト KuniJiban の柱状図（規約は複製と頒布を認め、出典の表示を求める）。XML で取れるかと個人情報の有無を #505 で確かめる
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: 要領と、テストの中で作ったボーリング交換用データ（crates/zumen-geology/tests）で確かめた。DTD での検証と、KuniJiban の柱状図との照合はまだ（#505）
- issue: [zumen-jp/zumen#390](https://github.com/zumen-jp/zumen/issues/390)
- 形式の文書: [docs/formats/boring.md](../../docs/formats/boring.md)

### format-cedxm

- 名前: CEDXM
- 権利の根拠: 評議会の Q&amp;A は、形式を会員にだけ開示し、会員だけが使う、とする。会員は、読み書きするソフトを自由に作って売れる。実装を公開すると、会員でない人に形式を開示することになる。入会は法人が主で、入会金と年会費がそれぞれ 8 万円。オープンソースの実装は見つからない。評議会が公開の実装を認めない限り作らない（zumen-jp/zumen#401、2026-11-02）
- 仕様: [cedxm](#spec-cedxm)
- 権利の根拠にした文書: [シーデクセマ評議会の Q&amp;A](https://www.cedxm.com/about/faq/)（確認日: 2026-10-24）
- issue: [zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)
- issue: [zumen-jp/zumen#401](https://github.com/zumen-jp/zumen/issues/401)

### format-citygml

- 名前: CityGML（PLATEAU の 3D 都市モデルと i-UR を含む）
- 権利の根拠: OGC が CityGML を、国土交通省が PLATEAU の製品仕様書（サイトの内容は PDL 1.0）を無償で公開している。実装を制限する記述はない。3D 都市モデルのデータの権利は各自治体にあり、測量法に注意するよう書かれているので、データは同梱しない
- 仕様: [citygml\_2\_0](#spec-citygml_2_0)、[citygml\_3\_0\_gml](#spec-citygml_3_0_gml)、[plateau\_5\_1](#spec-plateau_5_1)
- 実装の根拠: spec
- 権利の根拠にした文書: [PLATEAU のサイトポリシー](https://www.mlit.go.jp/plateau/site-policy/)（確認日: 2026-11-01）
- 照合に使う候補: PLATEAU VIEW・QGIS
- サンプルの出どころ: PLATEAU のデータ（G 空間情報センター。多くは政府標準利用規約 2.0）。大きいので ZUMEN\_SAMPLES に置く。再配布できる 5 MB 未満の公式のサンプルは見つかっていない
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: CityGML 2.0 と PLATEAU の製品仕様書と、テストの中で作った CityGML（crates/zumen-citygml/tests）で確かめた。PLATEAU のデータでの照合はまだ（#379・#504）
- 既知の crate: nusamai-citygml・ecitygml-io
- issue: [zumen-jp/zumen#379](https://github.com/zumen-jp/zumen/issues/379)
- 形式の文書: [docs/formats/citygml.md](../../docs/formats/citygml.md)

### format-csv

- 名前: CSV
- 権利の根拠: IETF の RFC で、誰でも読める。実装を制限する記述はない
- 仕様: [rfc\_4180](#spec-rfc_4180)
- 実装の根拠: spec
- サンプルの出どころ: テストの中で作る
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: RFC 4180 と、テストの中で作った CSV（crates/zumen-sheet/tests）で確かめた。表計算のソフトが書いた CSV との照合はまだ（#505）
- issue: [zumen-jp/zumen#391](https://github.com/zumen-jp/zumen/issues/391)
- 形式の文書: [docs/formats/sheet.md](../../docs/formats/sheet.md)

### format-delivery

- 名前: 電子納品の管理ファイル（INDEX\_C.XML・PHOTO.XML など）
- 権利の根拠: 国土交通省が要領・基準と DTD を誰でも無償で入手できる形で公開している。実装を制限する記述はない。電子納品の頁は転載と改変を禁じるので、DTD・作成例・本文は写さず同梱しない
- 仕様: [ed\_const\_r8\_2](#spec-ed_const_r8_2)、[ed\_design\_r8\_2](#spec-ed_design_r8_2)、[ed\_eizen\_r3](#spec-ed_eizen_r3)、[ed\_photo\_r5\_3](#spec-ed_photo_r5_3)
- 実装の根拠: spec
- 権利の根拠にした文書: [国土交通省 電子納品に関する要領・基準の「リンク・著作権」](https://www.cals-ed.go.jp/copyright/)（確認日: 2026-11-01）
- 照合に使う候補: 国土交通省の電子納品チェックシステム（手元でだけ使う）
- サンプルの出どころ: 再配布を認める公開サンプルは見つかっていない。国土交通省の作成例は転載が禁じられているので、テストの中で作る
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: 要領・基準と、テストの中で作った成果品と管理ファイル（crates/zumen-delivery/tests）で確かめた。DTD での検証と、公開サンプルとの照合はまだ（#505）
- issue: [zumen-jp/zumen#383](https://github.com/zumen-jp/zumen/issues/383)
- issue: [zumen-jp/zumen#384](https://github.com/zumen-jp/zumen/issues/384)
- issue: [zumen-jp/zumen#388](https://github.com/zumen-jp/zumen/issues/388)
- 形式の文書: [docs/formats/delivery.md](../../docs/formats/delivery.md)

### format-dwg

- 名前: DWG
- 権利の根拠: Autodesk は仕様を出していない。Open Design Alliance が公開する仕様書は、自分の責任で使うものとし、実装を制限する記述はない。DWG は Autodesk の商標。GPL の実装（LibreDWG）のソースは読まない
- 仕様: [ods\_dwg\_5\_4\_1](#spec-ods_dwg_5_4_1)
- 実装の根拠: spec・inferred・reference
- 参照した実装: [ACadSharp](https://github.com/DomCR/ACadSharp)（MIT）
- 参照した実装: [ezdwg](https://github.com/monozukuri-ai/ezdwg)（MIT）
- 照合に使う候補: ODA File Converter・DWG TrueView
- サンプルの出どころ: 公開サンプルの DWG（samples/dwg）。zumen-jp が AutoCAD 2027 で作った図面で、CC0 1.0。同じ図面を AutoCAD で書いた DXF と比べる。出所とライセンスは samples/ledger.toml・ACadSharp のテストの図面（MIT）。同じ図面を DXF にしたものと比べる・ezdwg のテストの図面（MIT）
- 読みの確かめた度合い: `application`（確認日: 2026-11-03）
- 読みの確かめた範囲: AutoCAD 2027 で作った R2000・R2004・R2013・R2018 の DWG（samples/dwg）を、AutoCAD が同じ図面から書いた DXF と比べ、オブジェクトの種類・所有者・値が合うことをテストで確かめる。ほかの図面で確かめた所と、確かめていない所は docs/formats/dwg.md にある。毎晩の CI で、公開サンプルの DWG（samples/dwg）を ACadSharp 3.8.0 でも読み、画層・図形の種類と画層・モデル空間の形の値が zumen の読みと同じことと、要素にならない図形の数が、移さなかったことを言う診断の件数を超えないことを確かめる（xtask/verify、#528）
- 読みで使った公開サンプル: [samples/dwg/](../../samples/dwg/)
- 読みで照らした外部の実装: ACadSharp 3.8.0
- 読みで開いたソフト: AutoCAD 2027
- 書きの確かめた度合い: `application`（確認日: 2026-11-03）
- 書きの確かめた範囲: AutoCAD 2027（accoreconsole）で、AutoCAD で作った R2000・R2004・R2010・R2018 の DWG に、引出線の置き換え・変換・追加を当てて書き戻したものを開き、監査の誤りが 0 で、形・矢印・注記が当てた値と合うことを確かめた。R2004 から後は、組み直したコンテナーも開けた（#345）。ほかの規則は、この方法で確かめていない。点・寸法・表と、引出線のほかの変更を書き戻したファイルは開いていない（#342・#314・#503）。書き戻した DWG は、毎晩の CI で ACadSharp 3.8.0 で読み、通知が入力より増えないことと、変更で決めた形・画層・色が ACadSharp の読んだ値と合うことを確かめる（xtask/verify、#528）
- 書きで照らした外部の実装: ACadSharp 3.8.0
- 書きで開いたソフト: AutoCAD 2027
- 旧版の記録: 旧版の決定 0003（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）
- 形式の文書: [docs/formats/dwg.md](../../docs/formats/dwg.md)

### format-dxf

- 名前: DXF
- 権利の根拠: Autodesk が DXF Reference を公開している。実装を制限する記述はない。DXF と AutoCAD は Autodesk の商標
- 仕様: [dxf\_2026](#spec-dxf_2026)
- 実装の根拠: spec・inferred・reference
- 参照した実装: [ezdxf](https://ezdxf.mozman.at/)（MIT）
- 照合に使う候補: ezdxf・ODA File Converter・DWG TrueView
- サンプルの出どころ: 公開サンプルの DXF（samples/dxf・samples/dwg）。ezdxf の例は MIT、zumen-jp が AutoCAD 2027 で作った図面は CC0 1.0。出所とライセンスは samples/ledger.toml・ACadSharp のテストの図面（MIT）・ezdwg のテストの図面（MIT）・自治体などが公開する図面。再配布できるかは確かめていない
- 読みの確かめた度合い: `application`（確認日: 2026-11-03）
- 読みの確かめた範囲: AutoCAD 2027 で作った R2000〜R2018 の DXF（samples/dwg）で、レイアウトの用紙・弧長寸法・反転した切り抜きを、AutoCAD が書いた値と比べた（決定 0208・0228・0229）。TEXT の高さと位置合わせは、AutoCAD 2027 が計算した置く点と比べた（決定 0246）。ほかの規則は、AutoCAD では確かめていない（docs/formats/dxf.md）。毎晩の CI で、公開サンプルの DXF（samples/dxf・samples/dwg）を ezdxf でも読み、画層・図形の種類・モデル空間の形の値が zumen の読みと同じことと、図形を黙って捨てていないことを確かめる（xtask/verify、#511）
- 読みで使った公開サンプル: [samples/dxf/](../../samples/dxf/)・[samples/dwg/](../../samples/dwg/)
- 読みで照らした外部の実装: ezdxf 1.4.5
- 読みで開いたソフト: AutoCAD 2027
- 書きの確かめた度合い: `application`（確認日: 2026-11-03）
- 書きの確かめた範囲: AutoCAD 2027（accoreconsole）で、AutoCAD で作った R2000・R2004・R2010・R2018 の DXF に、引出線の置き換え・変換・追加と MULTILEADER の線の置き換えを当てて書き戻したものを開き、監査の誤りが 0 で、形・矢印・注記が当てた値と合うことを確かめた（#345）。ほかの規則は、AutoCAD では確かめていない。点・寸法・表と、引出線のほかの変更を書き戻したファイルと、convert（決定 0206）で書いた DXF は、AutoCAD で開いていない（#342・#314・#503）。書き戻した DXF は、毎晩の CI で ezdxf で読み、監査の誤りが入力より増えないことと、変更で決めた形・画層・色が ezdxf の読んだ値と合うことを確かめる（xtask/verify、#511）。convert で書いた DXF は、ezdxf で読めないものと監査が誤りを直すものがある（#509）ので、根拠にしない
- 書きで照らした外部の実装: ezdxf 1.4.5
- 書きで開いたソフト: AutoCAD 2027
- 形式の文書: [docs/formats/dxf.md](../../docs/formats/dxf.md)

### format-e57

- 名前: E57（ASTM E2807）
- 権利の根拠: 仕様は ASTM が売り、実装を制限する記述はない。販売のページが規格を AI で扱うことを禁じるので、本文は読まず、許容のライセンスの公開の実装とサンプルから読む
- 仕様: [e57\_astm\_e2807](#spec-e57_astm_e2807)
- 実装の根拠: reference
- 権利の根拠にした文書: [libE57Format-test-data の README（ライセンス）](https://github.com/asmaloney/libE57Format-test-data)（確認日: 2026-11-01）
- 参照した実装: [libE57Format](https://github.com/asmaloney/libE57Format)（BSL-1.0）
- 参照した実装: [e57（Rust）](https://github.com/cry-inc/e57)（MIT）
- 照合に使う候補: CloudCompare
- サンプルの出どころ: libE57Format-test-data（reference は libe57 の Test Data License、ほかは CC0 1.0）
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: 公開の実装の記述と、テストの中で作った E57（crates/zumen-e57/tests）で確かめた。ページ・XML・点の圧縮の読み取りは e57 の crate に任せる。公開サンプルと CloudCompare での照合はまだ（#504）
- 既知の crate: e57
- issue: [zumen-jp/zumen#373](https://github.com/zumen-jp/zumen/issues/373)
- 形式の文書: [docs/formats/e57.md](../../docs/formats/e57.md)

### format-express

- 名前: EXPRESS のスキーマ（ISO 10303-11。IFC のスキーマなど）
- 権利の根拠: ISO が規格を売っていて、販売のページに実装を制限する記述はない。実装を制限しないと示す ISO の公式の文書は見つからなかったので、利用者が権利を判断した（zumen-jp/zumen#529、2026-11-09）。ISO のサイトに共通の注記が、ISO の内容を AI で扱うこと（学習と、AI のツールに入れて応答を作らせること）を禁じるので、台帳の e57 と同じく、ISO の本文は買わず、読まない。公開された EXPRESS のスキーマ（IFC など）と、寛容なライセンスの実装から読む。EXPRESS のスキーマは同梱しない（旧版の決定 0028）。ISO のページと仕様の扱いは、zumen-jp/zumen#669 で利用者が決める
- 仕様: [iso\_10303\_11\_2004](#spec-iso_10303_11_2004)
- 実装の根拠: inferred
- 照合に使う候補: IfcOpenShell（依存にせず、別に入れて使う）
- サンプルの出どころ: buildingSMART International が公開する IFC の EXPRESS のスキーマ。Git・パッケージ・テストに入れない（旧版の決定 0028）。テストの中で小さなスキーマを書く
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-09）
- 読みの確かめた範囲: テストの中で書いた小さな EXPRESS（crates/zumen-ifc/tests/express.rs と、src/express.rs の単体テスト）だけで確かめた。型・実体・属性・集まりを読むこと、評価しない規則の数、読めない書き方・宣言のない名前・同じ名前の宣言・循環の失敗と指す名前、予算（バイトごとの work、集まりの入れ子の depth、長い鎖と循環を調べる手順の work）、既定の深さまでの入れ子を 1 MiB のスタックで読んでチェックできること、その 4 倍まで上限を引き上げても同じスタックで型を読んで写し・比較・表示・破棄できること、1 つの宣言に並べた属性の名前が型を共有すること、切ったり変えたりした EXPRESS で落ちないことを確かめる。buildingSMART が公開する IFC の EXPRESS のスキーマを読むことと、読んだ値とチェックの結果を外部の実装と照らすことはまだ（#663）
- 旧版の記録: 旧版の決定 0028（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）
- issue: [zumen-jp/zumen#410](https://github.com/zumen-jp/zumen/issues/410)
- issue: [zumen-jp/zumen#529](https://github.com/zumen-jp/zumen/issues/529)
- issue: [zumen-jp/zumen#663](https://github.com/zumen-jp/zumen/issues/663)
- issue: [zumen-jp/zumen#668](https://github.com/zumen-jp/zumen/issues/668)
- issue: [zumen-jp/zumen#669](https://github.com/zumen-jp/zumen/issues/669)
- 形式の文書: [docs/formats/express.md](../../docs/formats/express.md)
- 補足: 読み取りを足した PR（zumen-jp/zumen#450）は、参照した実装を記録していないので、実装の根拠は推定にする。寛容なライセンスの実装を参照したら、basis に reference を、references にその実装を足す

### format-extdm

- 名前: 拡張 DM（公共測量標準図式数値地形図データファイル）
- 権利の根拠: 国土地理院が実装規約を技術資料として無償で公開している。技術資料は、特段の記載がなければ公共データ利用規約（PDL 1.0）を準用する。実装を制限する記述はない
- 仕様: [extdm\_furoku7](#spec-extdm_furoku7)、[extdm\_a1\_291](#spec-extdm_a1_291)
- 実装の根拠: spec
- 権利の根拠にした文書: [国土地理院の技術資料の利用規約](https://www.gsi.go.jp/REPORT/TECHNICAL/riyoukiyaku_gijutsushiryou.html)（確認日: 2026-11-01）
- 照合に使う候補: 基盤地図情報ビューアー（拡張 DM で書き出せる。手元でだけ使う）
- サンプルの出どころ: 再配布を認める公開サンプルは見つかっていない。テストの中で作る
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: 実装規約と、テストの中で組み立てたレコード（crates/zumen-extdm/tests）で確かめた。再配布できる公開サンプルは見つかっていない。基盤地図情報ビューアーが書き出した拡張 DM との照合はまだ（#504）
- issue: [zumen-jp/zumen#378](https://github.com/zumen-jp/zumen/issues/378)
- 形式の文書: [docs/formats/extdm.md](../../docs/formats/extdm.md)

### format-fgd

- 名前: 基盤地図情報（GML）
- 権利の根拠: 国土地理院がファイル仕様書と GML のスキーマを無償で公開している。実装を制限する記述はない。基盤地図情報は測量成果で、データの複製と使用には測量法の承認（基盤地図情報は無償で承認する）が要る場合があるので、データは同梱せず利用者が渡す（決定 0019 のジオイド・モデルと同じ扱い）
- 仕様: [fgd\_dl\_5\_3](#spec-fgd_dl_5_3)、[fgd\_dl\_5\_2](#spec-fgd_dl_5_2)、[fgd\_dl\_4\_1](#spec-fgd_dl_4_1)
- 実装の根拠: spec
- 権利の根拠にした文書: [国土地理院「測量成果の複製・使用承認の Q&amp;A」](https://www.gsi.go.jp/LAW/2930-qa.html)（確認日: 2026-11-01）
- 照合に使う候補: 基盤地図情報ビューアー・QGIS
- サンプルの出どころ: 利用者が国土地理院から入手したデータ。Git・パッケージ・テストに入れない。テストの中で小さな GML を作る
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: ファイル仕様書と、テストの中で作った小さな GML とアーカイブ（crates/zumen-fgd/tests）で確かめた。データは Git に入れない。基盤地図情報ビューアーと QGIS での照合はまだ（#504）
- issue: [zumen-jp/zumen#378](https://github.com/zumen-jp/zumen/issues/378)
- 形式の文書: [docs/formats/fgd.md](../../docs/formats/fgd.md)

### format-ids

- 名前: IDS（Information Delivery Specification）
- 権利の根拠: 文書と XSD は CC BY-ND 4.0 で、実装についての記述はない（IFC 4.3 の文書と同じ扱い）。XSD と、そこから生成したコードは同梱しない。照合は文書の記述から書く
- 仕様: [ids\_1\_0](#spec-ids_1_0)
- 実装の根拠: spec
- 権利の根拠にした文書: [IDS のリポジトリのライセンス（LICENSE）](https://github.com/buildingSMART/IDS/blob/development/LICENSE)（確認日: 2026-10-31）
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: IDS 1.0 の文書と、テストの中で作った IDS（crates/zumen-ids/tests・crates/zumen-ifc/tests/ids.rs）で確かめた。公開サンプルと外部の実装との照合はまだ（#522）
- issue: [zumen-jp/zumen#411](https://github.com/zumen-jp/zumen/issues/411)
- 形式の文書: [docs/formats/ids.md](../../docs/formats/ids.md)

### format-ifc

- 名前: IFC
- 権利の根拠: 文書は CC BY-ND 4.0 で、ライセンスの外の使い方には事前の書面の同意が要る。実装についての記述はない。buildingSMART International は、自らの検証のツールを MIT で公開している。スキーマと、そこから生成したコードは同梱しない。IFC2X3 TC1 と IFC4 ADD2 TC1 の文書は CC BY-NC-ND 4.0 だが、文書を複製・改変・頒布せず事実だけを使う実装は許諾の対象の行為に当たらない（CC 4.0 の 8(a)）。IFC 4.3 の開発リポジトリの LICENSE.md は、文書をソフトウェアの開発に使ってよいと明記し、buildingSMART の今の説明は IFC 全体を CC BY-ND 4.0 とする（zumen-jp/zumen#440、2026-11-02）。IFC は buildingSMART International の商標
- 仕様: [ifc\_4\_3\_2](#spec-ifc_4_3_2)、[ifc\_2x3\_tc1](#spec-ifc_2x3_tc1)、[ifc\_4\_0\_2\_1](#spec-ifc_4_0_2_1)
- 実装の根拠: spec
- 権利の根拠にした文書: [IFC 4.3.x の文書の表紙（ライセンスと連絡の注意）](https://github.com/buildingSMART/IFC4.x-development/blob/master/content/cover.md)（確認日: 2026-10-24）
- 権利の根拠にした文書: [IFC4 ADD2 TC1 の文書の著作権の注記（IFC2X3 TC1 も同じ文面）](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/copyright.htm)（確認日: 2026-10-31）
- 文書の補足: Internet Archive の写しで確かめた
- 権利の根拠にした文書: [buildingSMART International の Intellectual Property Policy（2021-06）](https://www.buildingsmart.org/wp-content/uploads/2021/06/bSI-Intellectual-Property-Policy.pdf)（確認日: 2026-10-31）
- 文書の補足: 実装の許諾の記述はない。Internet Archive の 2022-03-02 の写しで確かめた
- 権利の根拠にした文書: [buildingSMART International の検証のツール（validate）](https://github.com/buildingSMART/validate)（確認日: 2026-10-24）
- 照合に使う候補: IfcOpenShell（依存にせず、毎晩の CI で別に入れて使う）
- サンプルの出どころ: 公開サンプルの IFC 12 件（samples/ifc。林野庁・中野区・GNI BIM Dataset・BIM Whale・Text2BIM。出所とライセンスは samples/ledger.toml、#412）・許諾の要る国内の BIM ソフトの書き出しは #513 で足す（zumen-jp/zumen-legacy#128）
- 読みの確かめた度合い: `external`（確認日: 2026-11-11）
- 読みの確かめた範囲: 公開サンプルの IFC 12 件（samples/ifc）を建物モデルに写し、毎晩の CI で IfcOpenShell が同じファイルから読んだ値と照らす。照らすのは、実体のクラス・GlobalId・名前・所属の先と型・プロジェクトの空間への変換・開口・プロパティと数量の値（crates/zumen-ifc/tests/ifcopenshell/samples.py、#412・#512）。手元の照合では、12 件とも食い違いは 0。必要時の曲線の getter は、合成の IFC2X3 の 9 曲線・IFC4 と IFC4X3 の各 10 曲線、公開サンプル gni/model\_9 の 1 曲線で、元の属性の座標・方向の比・半径・ベクトルの大きさ・添字・区間の向き・切る位置を IfcOpenShell と照らし、食い違いは 0（crates/zumen-ifc/tests/ifcopenshell/curves.py、#419）。公開サンプルの IndexedPolyCurve 1 件を読みの external とし、自作した 29 件は synthetic の範囲で外部の実装とも照合したものとする。単位・診断・予算・幾何の評価は、この外部の曲線の照合には含めない。要素から形状表現・共有写像元をたどる L1 の getter は、合成 3 版の各 9 項目と補助レコード各 8 件、公開サンプルの柱の 6 項目と補助レコード 16 件で、参照と元の配置・写像の属性、曲線の点を IfcOpenShell と照らした（product\_shapes.py、#419）。公開の表現と写像は external、合成は synthetic の範囲で外部の実装とも照合したものとする。配置・写像・単位の適用と立体・断面の型への変換はしていない。既存の形の葉から押し出し・閉じた断面・外輪を取得する経路は、合成各版 2 件と公開の柱 1 件、計 7 件で、未適用の深さ・方向・配置と断面の名前・参照・外輪、SI 単位の名前・接頭辞を IfcOpenShell と照らし、食い違いは 0（extrusions.py、#419）。公開 1 件は external、合成 6 件は synthetic とする。配置・写像・単位の適用や自己交差・面積・形の評価、ほかの立体・断面の対応を検証した結果ではない。形の項目に直接付く面の表示は、合成各版 2 件と公開の柱 1 件、計 7 件で、色・透明度・Rendering の全属性と出所の参照を IfcOpenShell と照らし、食い違いは 0（styles.py、#419）。公開 1 件は external、合成 6 件は synthetic とする。表示・材質・レイヤ・写像による継承と反射の式は評価しない。診断と全 Kind の予算使用量はネイティブと WASI のバイト列を比較する。Building の Style 成分と形の意味値の Hash は未実装。判定・インスタンスのグラフ・壊れた所の扱いと、上に記した属性以外の値は、外部の実装と照らしていない。公開サンプル 12 件は、自前のテストでも L1 で読み、変えずに書くと元のバイト列に戻ることと、写さない値の診断の code・実体のクラス・件数を確かめる（crates/zumen-ifc/tests/samples.rs）。壊れた所の扱いと ifcZIP は、仕様とテストの中で作った IFC-SPF と ifcZIP（crates/zumen-ifc/tests）だけで確かめた。許諾の要る国内の BIM ソフトの書き出しは #513 で足す
- 読みで使った公開サンプル: [samples/ifc/](../../samples/ifc/)
- 読みで照らした外部の実装: IfcOpenShell 0.9.0.post1
- 書きの確かめた度合い: `external`（確認日: 2026-11-02）
- 書きの確かめた範囲: building\_apply でプロパティ・名前・説明を変えて書いた IFC4 と IFC2X3 を IfcOpenShell で開き、変更が見え、スキーマと EXPRESS の規則の検査の誤りが増えないことを、毎晩の CI で確かめる（#482、決定 0244）。書いたファイルを BIM ソフトで開いてはいない（#526）
- 書きで照らした外部の実装: IfcOpenShell 0.9.0.post1
- 旧版の記録: 旧版の決定 0028（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）
- 形式の文書: [docs/formats/ifc.md](../../docs/formats/ifc.md)

### format-isg

- 名前: ISG 形式（ジオイド高と基準面補正量の格子）
- 権利の根拠: ISG が書式の文書を公開している。実装を制限する記述はない。国土地理院のジオイド・モデル（JPGEO2024 など）は基本測量の測量成果で、複製と使用に測量法の承認が要る場合があるので、同梱せず利用者が渡す（決定 0019）
- 仕様: [isg\_2\_0](#spec-isg_2_0)
- 実装の根拠: spec
- 権利の根拠にした文書: [「ジオイド2024日本とその周辺」と「基準面補正パラメータ」の説明書（2026 年 4 月）](https://www.gsi.go.jp/common/000277542.pdf)（確認日: 2026-10-31）
- 照合に使う候補: 国土地理院のジオイド高計算サービス（ジオイド2024日本とその周辺）・国土地理院の内挿計算プログラム（geoidcalc。ISG 形式のモデルと一緒に配られる）
- サンプルの出どころ: 利用者が国土地理院から入手したジオイド・モデル。Git・パッケージ・テストに入れない
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: ISG format 2.0 の例と同じ形の合成の格子（crates/zumen-crs/tests/grid.rs）で確かめた。国土地理院のジオイド・モデルで、ジオイド高計算サービスか内挿計算プログラムと比べる照合は、検証キット（xtask/verify/geoid）を使って #438 で行う
- 形式の文書: [docs/formats/crs.md](../../docs/formats/crs.md)

### format-jpeg

- 名前: 工事写真（JPEG と Exif・XMP）
- 権利の根拠: Exif は CIPA が、XMP は Adobe が無償で公開している。JPEG の ITU-T T.81 は有償だが実装の制限はなく、画素の展開は許容のライセンスの crate に任せ、zumen は Exif と XMP のセグメントだけを読む
- 仕様: [jpeg\_t81](#spec-jpeg_t81)、[exif\_3\_1](#spec-exif_3_1)、[xmp\_2012](#spec-xmp_2012)
- 実装の根拠: spec
- サンプルの出どころ: 個人情報（位置・機種の番号など）を含まないよう、テストの中で作るか、zumen-jp が作った写真を CC0 で入れる
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: Exif 3.1・XMP の仕様と、テストの中で作った JPEG（crates/zumen-photo/tests）で確かめた。外部の実装と公開サンプルとの照合はまだ（#505）
- issue: [zumen-jp/zumen#387](https://github.com/zumen-jp/zumen/issues/387)
- 形式の文書: [docs/formats/jpeg.md](../../docs/formats/jpeg.md)

### format-jwc

- 名前: JWC（Jw\_cad for DOS の図面）
- 権利の根拠: 公式の説明は見つからない。今の Jw\_cad も開いて保存できる形式を、保存したファイルどうしを比べて読む。実装を制限する記述はない
- 仕様: 登録なし
- 実装の根拠: inferred・reference
- 参照した実装: [ezjww](https://github.com/monozukuri-ai/ezjww)（MIT）
- 照合に使う候補: Jw\_cad
- サンプルの出どころ: Jw\_cad 10 で保存したファイル

### format-jws

- 名前: JWS（Jw\_cad の図形）
- 権利の根拠: 公式の説明は見つからない。Jw\_cad の掲示板に投稿された説明（版 2.30）と、登録した図形と元の図面を比べた結果で読む。実装を制限する記述はない
- 仕様: 登録なし
- 実装の根拠: inferred・reference
- 参照した実装: [ezjww](https://github.com/monozukuri-ai/ezjww)（MIT）
- 照合に使う候補: Jw\_cad
- サンプルの出どころ: Jw\_cad 10 で登録した図形
- 補足: 掲示板の説明: <https://www.jwcad.net/log/consult2/consult2-0610-12/thread17752.htm>

### format-jww

- 名前: JWW（Jw\_cad for Windows の図面）
- 権利の根拠: Jw\_cad が、データ形式の説明を公開している。実装を制限する記述はない
- 仕様: [jww\_7\_02](#spec-jww_7_02)
- 実装の根拠: spec・reference
- 参照した実装: [ezjww](https://github.com/monozukuri-ai/ezjww)（MIT）
- 照合に使う候補: Jw\_cad
- サンプルの出どころ: Jw\_cad に付くサンプル・ezjww のリポジトリにある図面
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: 受け取った図面（JWW 10 件）を読んで図面モデルに移し、書き戻して読み直せることをテスト（crates/zumen-jww/tests/samples.rs）で確かめた（#289）。読んだ値を外部の実装や Jw\_cad の表示と照らしていないので、段は synthetic。ほかの規則は、Jw\_cad のデータ形式の説明と、テストの中で作った JWW で確かめた。毎晩の CI で、テストの中で作った JWW を ezjww 0.7.6 でも読んで照らす（xtask/verify、#528）が、自前で作ったファイルなので読みの段に数えない。公開サンプルと受け取った図面を外部の実装と照らすことと、Jw\_cad での照合はまだ（#503）
- 読みで使った受け取った図面: JWW 10 件
- 書きの確かめた度合い: `external`（確認日: 2026-11-10）
- 書きの確かめた範囲: テストの中で作った JWW と受け取った図面に書き戻し、自前の読み取りで読み直して、変えた要素が求めた値になることを確かめた。毎晩の CI で、テストの中で作った JWW に点・線分・円・円弧を足して書き戻したファイルを ezjww 0.7.6 で読み、変更で決めた形・レイヤ・ペンの番号が、ezjww の読んだ値と合うことを確かめる（xtask/verify、#528・#702）。外部の実装と照らしたのは追加の書き戻しだけで、ほかの変更を書き戻したファイルは、自前の読み取りで読み直しただけ。書いたファイルを Jw\_cad で開いてはいない（#342・#503）
- 書きで照らした外部の実装: ezjww 0.7.6
- 形式の文書: [docs/formats/jww.md](../../docs/formats/jww.md)

### format-kokuban

- 名前: 電子小黒板の情報と信憑性確認（改ざん検知）
- 権利の根拠: J-COMSIA の実装規約は公開されておらず、協会の会員か検定を受けるソフトの作り手に出すものとみられる。改ざん検知は SHA-256 のハッシュを使うとだけ公開している。zumen は規約を持たない
- 仕様: [kokuban\_jcomsia](#spec-kokuban_jcomsia)
- 権利の根拠にした文書: [J-COMSIA の合格ソフトの一覧（信憑性確認の Ver.1 と Ver.1.1）](https://www.jcomsia.org/kokuban/software/?v=2)（確認日: 2026-11-01）
- 権利の根拠にした文書: [J-COMSIA の入会の案内](https://www.jcomsia.org/nyukai/)（確認日: 2026-11-01）
- 照合に使う候補: J-COMSIA の信憑性チェックツール（利用の登録が要る）
- issue: [zumen-jp/zumen#389](https://github.com/zumen-jp/zumen/issues/389)

### format-landxml

- 名前: J-LandXML（LandXML 1.2 に準じた 3 次元設計データ交換標準）
- 権利の根拠: OCF と国土交通省が仕様の PDF を誰でも無償で入手できる形で公開している。実装を制限する記述はない。元の LandXML 1.2 のスキーマは同梱しない
- 仕様: [jlandxml\_1\_8](#spec-jlandxml_1_8)
- 実装の根拠: spec
- 照合に使う候補: 国土交通省の基準に対応した 3 次元設計データの閲覧ソフト（iLEX など。手元でだけ使う）
- サンプルの出どころ: 再配布を認める公開サンプルは見つかっていない。テストの中で小さな J-LandXML を作る
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-11）
- 読みの確かめた範囲: J-LandXML Ver.1.8 の文書と、テストの中で作った J-LandXML（crates/zumen-landxml/tests）で確かめた。片勾配は最大片勾配が一定の区間だけを Rust の L2 モデルへ移す。再配布できる公開サンプルは見つかっていない（#368・#504）
- issue: [zumen-jp/zumen#366](https://github.com/zumen-jp/zumen/issues/366)
- issue: [zumen-jp/zumen#368](https://github.com/zumen-jp/zumen/issues/368)
- 形式の文書: [docs/formats/landxml.md](../../docs/formats/landxml.md)

### format-las

- 名前: LAS・LAZ
- 権利の根拠: ASPRS が LAS の仕様を、OGC が LAZ の仕様を無償で公開している。OGC は LAZ を誰でも実装できるとする。LAZ を展開する LASzip と Rust の laz は、どちらも Apache-2.0
- 仕様: [las\_1\_4\_r15](#spec-las_1_4_r15)、[laz\_1\_4](#spec-laz_1_4)
- 実装の根拠: spec・inferred・reference
- 権利の根拠にした文書: [OGC の LAZ 1.4 コミュニティ標準の公表](https://www.ogc.org/announcement/ogc-announces-publication-of-the-laz-1-4-community-standard/)（確認日: 2026-11-01）
- 権利の根拠にした文書: [LASzip のリポジトリ（ライセンス）](https://github.com/LASzip/LASzip)（確認日: 2026-11-01）
- 文書の補足: GitHub のライセンスの判定は Apache-2.0
- 参照した実装: [LASzip](https://github.com/LASzip/LASzip)（Apache-2.0）
- 参照した実装: [laz-rs](https://github.com/tmontaigu/laz-rs)（Apache-2.0）
- 照合に使う候補: CloudCompare・PDAL・LAStools の lasinfo・laspy
- サンプルの出どころ: PDAL の test/data/las/synthetic\_test.las（BSD-3-Clause、合成の 1 点。samples/las/pdal-synthetic.las）
- 読みの確かめた度合い: `external`（確認日: 2026-11-11）
- 読みの確かめた範囲: PDAL が作った公開サンプル 1 件（非圧縮 LAS 1.2、点形式 3、synthetic フラグを持つ 1 点）を laspy 2.7.0 と照合した。ヘッダーの点数・尺度・原点・範囲と、座標・強度・分類・反射・GPS 時刻・RGB を比較した。実測の点群は使っていない。自前の合成 LAS 1.2 の点形式 0〜3、1.3 の 4・5、1.4 の 6〜10 も同じ値を比較したが、external の根拠は PDAL のサンプルだけ。公開サンプルの応答・診断は適合テストでネイティブと wasm のハッシュを比較する。座標系・VLR/EVLR の内容・落とす属性・波形の外部データ・実データ・基準になるアプリとの照合はまだ（#371・#504）。LAZ は las.laz.unsupported で断る（#510）
- 読みで使った公開サンプル: [samples/las/](../../samples/las/)
- 読みで照らした外部の実装: laspy 2.7.0
- 既知の crate: las・laz
- issue: [zumen-jp/zumen#371](https://github.com/zumen-jp/zumen/issues/371)
- issue: [zumen-jp/zumen#504](https://github.com/zumen-jp/zumen/issues/504)
- issue: [zumen-jp/zumen#804](https://github.com/zumen-jp/zumen/issues/804)
- 形式の文書: [docs/formats/las.md](../../docs/formats/las.md)

### format-par

- 名前: TKY2JGD・PatchJGD のパラメーターのファイル（座標の補正量の格子）
- 権利の根拠: 持ち主の国土地理院が、パラメーターのファイルのデータの行の書式（TKY2JGD の論文の表 2）と、PatchJGD の補正パラメーターの格子点の置き方と単位（公共測量成果改定マニュアル）を公開している。PatchJGD のファイルのヘッダーの書式を定めた文書は見当たらず、国土地理院が載せる各ファイルのヘッダーの例から推定する（basis の inferred）。国土地理院コンテンツ利用規約は、特段の記載がなければ公共データ利用規約（第 1.0 版）を当てはめ、実装を制限する記述はない（zumen-jp/zumen#529、2026-11-09）。パラメーターのファイルは同梱せず、利用者が渡す（決定 0019・0237）
- 仕様: [tky2jgd\_2001](#spec-tky2jgd_2001)、[seikakaitei\_r7\_4](#spec-seikakaitei_r7_4)
- 実装の根拠: spec・inferred
- 権利の根拠にした文書: [国土地理院コンテンツ利用規約](https://www.gsi.go.jp/kikakuchousei/kikakuchousei40182.html)（確認日: 2026-11-09）
- 文書の補足: 令和 7 年 11 月 20 日の改正。特段の記載がなければ、公共データ利用規約（第 1.0 版）を当てはめる。測量法による基本測量成果・公共測量成果の複製と使用の承認は、別に要る
- 権利の根拠にした文書: [国土地理院の TKY2JGD のダウンロードのページ](https://www.gsi.go.jp/sokuchikijun/tky2jgd_download.html)（確認日: 2026-11-09）
- 文書の補足: パラメーターのファイル（TKY2JGD.par の Ver.2.1.1・2.1.2 など）と、プログラム開発用の TKY2JGD のソースコードを配る。ソースコードは国土地理院コンテンツ利用規約に従う。書式と、申請の要否の記述はない。プログラムの配布は 2017-06-05 に終え、測量計算サイトの TKY2JGD を案内する
- 照合に使う候補: 国土地理院の測量計算サイト（TKY2JGD・PatchJGD）・jgdtrans 0.3.1（dev の依存。テストの中で作った格子で、読んだ格子点・補間の値・変換した座標を照らす。決定 0286）
- サンプルの出どころ: 利用者が国土地理院から入手したパラメーターのファイル。Git・パッケージ・テストに入れない。テストの中で合成の格子を作る
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-09）
- 読みの確かめた範囲: TKY2JGD の論文の表 2 と同じ項目の並びの合成の格子（桁の幅と行の終わりが表 2 と違うものと、表 2 の桁の幅のもの）と、行の終わりが CR+LF で長いヘッダーに UTF-8 でない行を含む合成の格子（crates/zumen-crs/tests/datum.rs・jgdtrans.rs）で、ヘッダーの読み飛ばし・壊れた行（初めのデータの行の後の UTF-8 でない行を含む）・格子点を数える予算・隣り合う格子点からの間隔の見分け（3 次メッシュの間隔と、公共測量成果改定マニュアル（令和 7 年 4 月）の 5 倍の間隔。格子点が 1 つだけのものや 15 倍の間隔のものは読まない）・双一次の補間（5 倍の格子の南端を含む）・複数の PatchJGD のファイルを古い順に当てる変換と、その作業量の上限や中断の合図で止まることを確かめた（決定 0286）。テストの中で作った格子を jgdtrans 0.3.1 にも読ませ、読んだ格子点・補間の値・変換した座標を照らした（自前で作った格子なので、段は上げない）。国土地理院のパラメーターのファイルは読んでいない。国土地理院のファイルの間隔とヘッダーと、測量計算サイトの TKY2JGD・PatchJGD との照合は #664 で行う
- 既知の crate: jgdtrans
- issue: [zumen-jp/zumen#471](https://github.com/zumen-jp/zumen/issues/471)
- issue: [zumen-jp/zumen#529](https://github.com/zumen-jp/zumen/issues/529)
- issue: [zumen-jp/zumen#664](https://github.com/zumen-jp/zumen/issues/664)
- issue: [zumen-jp/zumen#670](https://github.com/zumen-jp/zumen/issues/670)
- issue: [zumen-jp/zumen#678](https://github.com/zumen-jp/zumen/issues/678)
- issue: [zumen-jp/zumen#679](https://github.com/zumen-jp/zumen/issues/679)
- 形式の文書: [docs/formats/crs.md](../../docs/formats/crs.md)
- 補足: PatchJGD のファイルのヘッダーの行数と文字コード（UTF-8・Shift\_JIS）は、ファイルのバージョンで違う。2026-11-09 に確かめた国土地理院の座標補正パラメータファイルの更新の履歴では、noto2024\_BL.par の Ver.1.0.1 がヘッダーを 14 行から 15 行に、noto2024\_02BL.par の Ver.1.0.1 が文字コードを UTF-8 から Shift\_JIS に直した。初めのデータの行の前をヘッダーとして読まないのは、これに合わせた推定。更新の履歴と、ファイルと各ファイルのヘッダーの一覧へのリンクは、docs/formats/crs.md の「座標の補正量の格子」にある

### format-pdf

- 名前: PDF
- 権利の根拠: ISO の規格で、PDF Association が無償で配っている。実装を制限する記述はない
- 仕様: [iso\_32000\_2\_2020](#spec-iso_32000_2_2020)
- 実装の根拠: spec
- 権利の根拠にした文書: [PDF Association の告知「ISO 32000-2（PDF 2.0）を無償で入手できる」（2023-04-05）](https://pdfa.org/announcing-no-cost-access-to-iso-32000-2-pdf-2-0/)（確認日: 2026-11-03）
- 文書の補足: Adobe・Apryse・Foxit の後援で、PDF Association が登録した人に無償で配る。ページは自動の取得では開けない（HTTP 403）ので、検索の結果の要約で確かめた。無償版の利用の条件までは確かめていない
- 照合に使う候補: pdf.js
- サンプルの出どころ: PDFium の公開サンプル（Type 3・OCG・ソフトマスク・LZW と JPX）。CAD が出力した図面はまだない
- 読みの確かめた度合い: `external`（確認日: 2026-11-06）
- 読みの確かめた範囲: PDFium の公開サンプル 4 個と合成の 2 ページを pdf.js 6.5.438 で読み、ページ寸法・文字の内容と位置・経路と画像の数を照らす（crates/zumen-pdf/tests/external.rs、#519）。回転・クロップ・単位、フォーム、表示しない OCG を含む。Type 3 の Unicode の補い方と、ソフトマスクの経路の数は、両方の値と zumen の診断を固定して確認する（#520）。色・透明度・字形の見え方、一般の PDF の文字列の分け方は照らしていない。シーンは、図面モデルから zumen-scene で作れることだけを確かめた
- 読みで使った公開サンプル: [samples/pdf/](../../samples/pdf/)
- 読みで照らした外部の実装: pdf.js 6.5.438
- 書きの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 書きの確かめた範囲: 読んだバイト列をそのまま書けることを、自前の読み取りだけで確かめた。書いたファイルを PDF のソフトで開いてはいない
- 旧版の記録: 旧版の決定 0020、旧版の決定 0030（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）
- 形式の文書: [docs/formats/pdf.md](../../docs/formats/pdf.md)
- 補足: ページの解釈は hayro-interpret 0.9.0（Apache-2.0 OR MIT）に任せ、バージョンを固定する（決定 0256）。zumen-pdf は crates.io に出さず、どのパッケージにも入れない（決定 0289）

### format-ply

- 名前: PLY（点群）
- 権利の根拠: 持ち主の公式の仕様はない。Greg Turk の説明と、それを写した公開の解説で読む。利用の制限の記述はない
- 仕様: 登録なし
- 実装の根拠: inferred
- 権利の根拠にした文書: [Paul Bourke「PLY - Polygon File Format」](https://paulbourke.net/dataformats/ply/)（確認日: 2026-11-01）
- 照合に使う候補: CloudCompare・MeshLab
- サンプルの出どころ: samples/ply/（PDAL の 4 頂点・2 面の公開サンプル）・テストの中で作る
- 読みの確かめた度合い: `external`（確認日: 2026-11-11）
- 読みの確かめた範囲: PDAL が作った公開サンプル 1 件（ASCII、float64 の 4 頂点・2 面）を ply-rs 0.1.3 と照合した。全頂点の順と座標をビットで比較し、面を移さない診断と座標系が不明なことを確かめた。自前の合成の ASCII・両バイト順の座標・色・強度・分類と float/float32 の丸めも照合したが、external の根拠は公開サンプルだけ。公開サンプルと float32 の応答・診断をネイティブと wasm で比較する。実測点群、公開サンプルのバイナリと属性、法線や面のモデル化、CloudCompare・MeshLab との照合はまだ（#504）
- 読みで使った公開サンプル: [samples/ply/](../../samples/ply/)
- 読みで照らした外部の実装: ply-rs 0.1.3
- issue: [zumen-jp/zumen#373](https://github.com/zumen-jp/zumen/issues/373)
- issue: [zumen-jp/zumen#809](https://github.com/zumen-jp/zumen/issues/809)
- 形式の文書: [docs/formats/ply.md](../../docs/formats/ply.md)

### format-png

- 名前: PNG
- 権利の根拠: W3C が仕様を公開し、実装を制限していない。勧告の Abstract と Status of This Document、RF の実装を可能にする特許方針と勧告の著作権表示のライセンスを 2026-11-06 に確認した
- 仕様: [png\_3](#spec-png_3)
- 実装の根拠: spec
- 権利の根拠にした文書: [W3C Patent Policy](https://www.w3.org/policies/patent-policy/)（確認日: 2026-11-06）
- 文書の補足: 勧告の実装を Royalty-Free で可能にする特許方針。PNG 勧告の Status of This Document が準拠を記す
- 権利の根拠にした文書: [W3C Software and Document License (2023)](https://www.w3.org/copyright/software-license-2023/)（確認日: 2026-11-06）
- 文書の補足: 勧告の著作権表示が指定するライセンス。複製にライセンス通知の全文と既存の権利・免責の表示を含め、改変した場合は変更を示す条件で、目的を問わず無償で複製・改変・配布を認める
- 書きの確かめた度合い: `external`（確認日: 2026-11-06）
- 書きの確かめた範囲: RGBA 8 bit、非インターレース、非圧縮 DEFLATE の静止画。Chromium と zlib で寸法・RGBA・CRC・Adler-32 を照らした。穴・字形・描画順・通常と反転のクリップ・余白・半透明を含む。Canvas の半透明の RGB の丸めだけ 1 まで認める。外部画像・閲覧側ごとの色管理・アンチエイリアスの一致は対象外。PNG を読む実装は持たない
- 書きで照らした外部の実装: Chromium 156.0.8078.4・zlib (Node.js) 1.3.2.1-motley-285e94b
- issue: [zumen-jp/zumen#604](https://github.com/zumen-jp/zumen/issues/604)
- 形式の文書: [docs/formats/png.md](../../docs/formats/png.md)

### format-saf

- 名前: SXF の属性ファイル（SAF）
- 権利の根拠: SXF Ver.3.1 仕様書の附属書（属性付加機構）に含まれ、SXF と同じ扱い。実装を制限する記述はない。附属書の名前は解説の記事で確かめた
- 仕様: [sxf\_3\_1](#spec-sxf_3_1)
- 実装の根拠: spec
- 照合に使う候補: SXF ブラウザ
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: 属性付加機構編と、テストの中で作った SAF とアーカイブ（crates/zumen-sxf/tests/saf.rs）で確かめた。SXF ブラウザでの照合は #494・#505 で行う
- issue: [zumen-jp/zumen#386](https://github.com/zumen-jp/zumen/issues/386)
- 形式の文書: [docs/formats/sxf.md](../../docs/formats/sxf.md)

### format-sima

- 名前: SIMA 共通フォーマット
- 権利の根拠: 日本測量機器工業会（JSIMA）は、JSIMA201:2026「SIMA 測量データ共通フォーマット」（Ver.04.2）などの規格を、会員でない人にも書籍として頒布する（申込書を FAX で送り、先払い）。JPGIS 版の XML Schema（jsima102.xsd）は自由にダウンロードできる。実装を制限する記述は見つからず、オープンソースの小さな実装（MIT・Unlicense）もある。書籍を正規に入手し、利用の条項がないことを確かめてから public にする（zumen-jp/zumen#374、2026-11-02）
- 仕様: 登録なし
- 権利の根拠にした文書: [JSIMA「規格・書籍紹介」](https://www.jsima.or.jp/jsima/index.html)（確認日: 2026-11-02）
- 権利の根拠にした文書: [国立国会図書館サーチの「SIMA 共通フォーマット」JSIMA 203:2000（Ver.02）](https://iss.ndl.go.jp/books/R100000002-I000009976754-00)（確認日: 2026-11-01）
- issue: [zumen-jp/zumen#374](https://github.com/zumen-jp/zumen/issues/374)

### format-stbridge

- 名前: ST-Bridge
- 権利の根拠: 仕様書と XML Schema は公開されているが、実装の公開の条件は書かれていない。公開された実装の STB-KIT（MPL-2.0）は、ST-Bridge の権利は buildingSMART Japan にあり、同会との利用許諾契約に基づいて実装を公開している、と書く。zumen は契約を持たない
- プラグインの置き場所: zumen-jp/stbridge（非公開）
- 仕様: [stbridge\_2\_1\_1](#spec-stbridge_2_1_1)
- 実装の根拠: spec
- 権利の根拠にした文書: [STB-KIT の README「ST-Bridge および IFC との関係」](https://github.com/taisei-oss/stb-kit)（確認日: 2026-10-24）
- 既知の crate: stb-parser
- 旧版の記録: 旧版の決定 0001、旧版の決定 0013、旧版の決定 0028（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）
- issue: [zumen-jp/zumen-legacy#140](https://github.com/zumen-jp/zumen-legacy/issues/140)
- issue: [zumen-jp/zumen#402](https://github.com/zumen-jp/zumen/issues/402)

### format-stem

- 名前: Stem（設備機器ライブラリデータ交換仕様）
- 権利の根拠: C-CADEC が作り、2015-04 に建築保全センターが承継した。最終の版は Ver.10（2015-04）。管理を引き継いだ BLCJ は 2026-03-30 に解散を議決した。仕様書の入手先と条件は見つからなかった。清算人に問い合わせるまで作らない（zumen-jp/zumen#401）
- 仕様: 登録なし
- 権利の根拠にした文書: [BIM ライブラリーコンソーシアム「BLC-BIM オブジェクト標準（Version1.0）報告書」（2018-10-04）の「1.4 Stem、BE-Bridge 等の承継」](https://blcj.or.jp/wp/wp-content/uploads/2020/03/blcj_bim_standard_v1.pdf)（確認日: 2026-10-24）
- issue: [zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)
- issue: [zumen-jp/zumen#401](https://github.com/zumen-jp/zumen/issues/401)

### format-svg

- 名前: SVG
- 権利の根拠: W3C が仕様を公開し、実装を制限していない。勧告の実装を RF で可能にする特許方針と、実装に利用できる文書ライセンスを 2026-11-06 に確認した
- 仕様: [svg\_1\_1](#spec-svg_1_1)
- 実装の根拠: spec
- 権利の根拠にした文書: [W3C Patent Policy, 5 February 2004](https://www.w3.org/Consortium/Patent-Policy-20040205/)（確認日: 2026-11-06）
- 文書の補足: SVG 1.1 の勧告が準拠を記す特許方針。勧告の実装を RF で可能にすることを目的に定める
- 権利の根拠にした文書: [W3C Document License](https://www.w3.org/copyright/document-license-2023/)（確認日: 2026-11-06）
- 文書の補足: SVG 1.1 の勧告の文書の利用条件からたどれるライセンス。仕様の実装を容易にするため、一部をソフトウェアへ組み込めるとする。本文とコードは同梱しない
- 書きの確かめた度合い: `external`（確認日: 2026-11-06）
- 書きの確かめた範囲: 合成シーンから書いた SVG を Chromium で読み、寸法・描画順・面の穴・字形・通常と反転の切り抜きを画素と照らした。点記号の細部と閲覧側ごとのアンチエイリアスは照合していない。API 接続は #603、PNG は #604。SVG を読む実装は持たない
- 書きで照らした外部の実装: Chromium 156.0.8078.4
- issue: [zumen-jp/zumen#602](https://github.com/zumen-jp/zumen/issues/602)
- issue: [zumen-jp/zumen#603](https://github.com/zumen-jp/zumen/issues/603)
- issue: [zumen-jp/zumen#604](https://github.com/zumen-jp/zumen/issues/604)
- 形式の文書: [docs/formats/svg.md](../../docs/formats/svg.md)

### format-sxf

- 名前: SXF（SFC・P21 と、それを ZIP に入れた SFZ・P2Z）
- 権利の根拠: 国土交通省が、開発者向けに仕様書を公開している。実装を制限する記述はない。頁の転載と改変は禁じているので、仕様の本文は写さない
- 仕様: [sxf\_3\_1](#spec-sxf_3_1)、[sxf\_display\_r7\_12](#spec-sxf_display_r7_12)
- 実装の根拠: spec・inferred・reference
- 権利の根拠にした文書: [国土交通省 電子納品に関する要領・基準の「リンク・著作権」](https://www.cals-ed.go.jp/copyright/)（確認日: 2026-10-24）
- 文書の補足: サイトにつながらず、Internet Archive の 2026-05-13 の写しで確かめた
- 参照した実装: [ezsxf](https://github.com/monozukuri-ai/ezsxf)（MIT）
- 照合に使う候補: Jw\_cad・SXF ブラウザ
- サンプルの出どころ: ezsxf のリポジトリにある公開の図面
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-06）
- 読みの確かめた範囲: 受け取った図面（SFC 1 件・P21 1 件）を読み、要素の ID がインスタンスの番号で重ならないことと、書き戻して読み直せることをテスト（crates/zumen-sxf/tests/samples.rs）で確かめた（#288・#290）。読んだ値を外部の実装や SXF ブラウザの表示と照らしていないので、段は synthetic。ほかの規則は、仕様と、テストの中で作った SFC・P21 とアーカイブで確かめた。毎晩の CI で、テストの中で作った SFC・P21 を ezsxf 0.3.7 でも読んで照らす（xtask/verify、#528）が、自前で作ったファイルなので読みの段に数えない。同じ番号の SFC フィーチャも出所のレコードで区別し、線・円の形とレイヤ、要素の欠落と診断を照らす。用紙への所属は元の番号で比べる（#553）。公開サンプルと受け取った図面を外部の実装と照らすことと、SXF ブラウザでの照合はまだ（#503）
- 読みで使った受け取った図面: SFC 1 件・P21 1 件
- 書きの確かめた度合い: `external`（確認日: 2026-11-06）
- 書きの確かめた範囲: テストの中で作った SFC・P21 と受け取った図面に書き戻し、自前の読み取りで読み直して、変えた要素が求めた値になることを確かめた。毎晩の CI で、テストの中で作った SFC・P21 に点を足して書き戻したファイルを ezsxf 0.3.7 で読み、変更で決めた形・レイヤと、SFC の色（既定義色の番号）が、ezsxf の読んだ値と合うことを確かめる（xtask/verify、#528）。P21 では、点マーカーの種類を比べない。ezsxf は P21 の文字列のエスケープを戻さないので、P21 のレイヤの名前は、照合がファイルのバイトから戻す。Shift\_JIS のレイヤ「図枠」へ追加した 2 点の所属を照らし、書き戻したファイルに元の名前のバイトが残ることを確かめた（#552）。外部の実装と照らしたのは点の書き戻しだけで、ほかの変更を書き戻したファイルは、自前の読み取りで読み直しただけ。SFZ・P2Z に書き戻したアーカイブは、外部の実装と照らしていない。書いたファイルを SXF ブラウザなどで開いてはいない（#342・#503）
- 書きで照らした外部の実装: ezsxf 0.3.7
- issue: [zumen-jp/zumen#385](https://github.com/zumen-jp/zumen/issues/385)
- 形式の文書: [docs/formats/sxf.md](../../docs/formats/sxf.md)

### format-xlsx

- 名前: Excel の表（xlsx）
- 権利の根拠: Ecma International が ECMA-376 を無償で公開している。実装を制限する記述はない
- 仕様: [ecma\_376\_5](#spec-ecma_376_5)
- 実装の根拠: spec
- サンプルの出どころ: テストの中で作る
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: ECMA-376 と、テストの中で作った xlsx（crates/zumen-sheet/tests）で確かめた。表計算のソフトが書いた xlsx と、外部の実装との照合はまだ（#505）
- issue: [zumen-jp/zumen#391](https://github.com/zumen-jp/zumen/issues/391)
- 形式の文書: [docs/formats/sheet.md](../../docs/formats/sheet.md)

### format-xyz

- 名前: XYZ（座標を並べた点群のテキスト）
- 権利の根拠: 決まった仕様も持ち主もない。座標と属性を区切って並べたテキストを、推定で読む
- 仕様: 登録なし
- 実装の根拠: inferred
- 照合に使う候補: CloudCompare
- サンプルの出どころ: テストの中で作る
- 読みの確かめた度合い: `synthetic`（確認日: 2026-11-03）
- 読みの確かめた範囲: テストの中で作った XYZ（crates/zumen-xyz/tests）で確かめた。決まった仕様はなく、列の読み方は推定。CloudCompare が書いた XYZ との照合はまだ（#504）
- issue: [zumen-jp/zumen#373](https://github.com/zumen-jp/zumen/issues/373)
- 形式の文書: [docs/formats/xyz.md](../../docs/formats/xyz.md)
