<!-- formats/ledger.toml から cargo xtask codegen で生成。直接編集しない。 -->

# 形式と権利の一覧

[権利の台帳](../../formats/ledger.toml)から生成しています。ここにあるのは実装を置く方針であり、読み書きできる形式の一覧ではありません。変更の手順は [台帳の書き方](../../formats/README.md)、図面の要素の対応は [Drawing2D の対応表](drawing2d-mapping.md)を参照してください。

- 権利: `public` は実装の公開に第三者の許諾が不要、`permission` は契約・会員・許諾が必要、`unknown` は条件が未確認です。
- 置き場所: `core` はコア、`plugin` は外部のプラグイン、`none` は実装しない方針です。
- 台帳は判断した時点の記録です。URL の到達や権利の変更は、生成時に再確認していません。

## 一覧

| キー | 名前 | 系統 | 領域 | 権利 | 置き場所 |
| --- | --- | --- | --- | --- | --- |
| [bcf](#format-bcf) | BCF | bcf | review | `public` | `core` |
| [bebridge](#format-bebridge) | BE-Bridge | bebridge | building | `unknown` | `none` |
| [boring](#format-boring) | ボーリング交換用データ（BED） | boring | delivery | `public` | `core` |
| [cedxm](#format-cedxm) | CEDXM | cedxm | building | `permission` | `none` |
| [citygml](#format-citygml) | CityGML（PLATEAU の 3D 都市モデルと i-UR を含む） | citygml | geo | `public` | `core` |
| [csv](#format-csv) | CSV | csv | delivery | `public` | `core` |
| [delivery](#format-delivery) | 電子納品の管理ファイル（INDEX\_C.XML・PHOTO.XML など） | delivery | delivery | `public` | `core` |
| [dwg](#format-dwg) | DWG | acad | drawing | `public` | `core` |
| [dxf](#format-dxf) | DXF | acad | drawing | `public` | `core` |
| [e57](#format-e57) | E57（ASTM E2807） | e57 | pointcloud | `public` | `core` |
| [extdm](#format-extdm) | 拡張 DM（公共測量標準図式数値地形図データファイル） | extdm | geo | `public` | `core` |
| [fgd](#format-fgd) | 基盤地図情報（GML） | fgd | geo | `public` | `core` |
| [ids](#format-ids) | IDS（Information Delivery Specification） | ids | building | `public` | `core` |
| [ifc](#format-ifc) | IFC | ifc | building・civil | `public` | `core` |
| [isg](#format-isg) | ISG 形式（ジオイド高と基準面補正量の格子） | isg | geo | `public` | `core` |
| [jpeg](#format-jpeg) | 工事写真（JPEG と Exif・XMP） | jpeg | delivery | `public` | `core` |
| [jwc](#format-jwc) | JWC（Jw\_cad for DOS の図面） | jwcad | drawing | `public` | `core` |
| [jws](#format-jws) | JWS（Jw\_cad の図形） | jwcad | drawing | `public` | `core` |
| [jww](#format-jww) | JWW（Jw\_cad for Windows の図面） | jwcad | drawing | `public` | `core` |
| [kokuban](#format-kokuban) | 電子小黒板の情報と信憑性確認（改ざん検知） | jpeg | delivery | `permission` | `none` |
| [landxml](#format-landxml) | J-LandXML（LandXML 1.2 に準じた 3 次元設計データ交換標準） | landxml | civil | `public` | `core` |
| [las](#format-las) | LAS・LAZ | las | pointcloud | `public` | `core` |
| [pdf](#format-pdf) | PDF | pdf | drawing | `public` | `core` |
| [ply](#format-ply) | PLY（点群） | ply | pointcloud | `public` | `core` |
| [saf](#format-saf) | SXF の属性ファイル（SAF） | sxf | drawing・delivery | `public` | `core` |
| [sima](#format-sima) | SIMA 共通フォーマット | sima | civil | `unknown` | `none` |
| [stbridge](#format-stbridge) | ST-Bridge | stbridge | building | `permission` | `plugin` |
| [stem](#format-stem) | Stem（設備機器ライブラリデータ交換仕様） | stem | building | `unknown` | `none` |
| [sxf](#format-sxf) | SXF（SFC・P21 と、それを ZIP に入れた SFZ・P2Z） | sxf | drawing | `public` | `core` |
| [xlsx](#format-xlsx) | Excel の表（xlsx） | xlsx | delivery | `public` | `core` |
| [xyz](#format-xyz) | XYZ（座標を並べた点群のテキスト） | xyz | pointcloud | `public` | `core` |

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
- 補足: ブランチ release\_2\_1 には、ライセンスの記述がない

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
- 補足: 最新の版ではない。2015-04 に建築保全センターが承継したあとの版の入手先は見つからなかった

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
- 補足: DTD は https://www.cals-ed.go.jp/cri\_dtdxml/ の boring\_R8.2.zip に入っている

### spec-ed_const_r8_2

- 名前: 工事完成図書の電子納品等要領
- 公開元: 国土交通省
- 版: 令和 8 年 2 月（令和 8 年 4 月 1 日から契約する工事に適用）
- 入手の条件: 無償公開
- 入手先: [工事完成図書の電子納品等要領](https://www.cals-ed.go.jp/cri_point/)
- 確認日: 2026-11-01
- 補足: 管理ファイルの DTD と作成例は https://www.cals-ed.go.jp/cri\_dtdxml/ に置く。頁の転載と改変を禁じるので、DTD は同梱しない

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

### spec-fgd_dl_4_1

- 名前: 基盤地図情報ダウンロードデータファイル仕様書
- 公開元: 国土地理院
- 版: Ver4.1（2023-03-31）
- 入手の条件: 無償公開
- 入手先: [基盤地図情報ダウンロードデータファイル仕様書](https://fgd.gsi.go.jp/otherdata/spec/FGD_DLFileSpecV4.1.pdf)
- 確認日: 2026-11-01
- 補足: GML のスキーマ（JPGIS2014）も同じ所に置く。データの入手には利用者の登録が要る

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
- 補足: 文書の著作権の注記は CC BY-NC-ND 4.0 で、ライセンスの外の使い方には事前の書面の同意が要るとする。実装についての記述はない。2026-10-31 はサイトが自動の取得を拒み、Internet Archive の 2026-03-27 の写しで確かめた

### spec-ifc_4_0_2_1

- 名前: Industry Foundation Classes Version 4.0 - Addendum 2 - Technical Corrigendum 1
- 公開元: buildingSMART International
- 版: IFC 4.0.2.1（IFC4 ADD2 TC1。ISO 16739-1:2018 と同じ内容）
- 入手の条件: 無償公開
- 入手先: [Industry Foundation Classes Version 4.0 - Addendum 2 - Technical Corrigendum 1](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/)
- 確認日: 2026-10-31
- 補足: 文書の著作権の注記は CC BY-NC-ND 4.0 で、ライセンスの外の使い方には事前の書面の同意が要るとする。実装についての記述はない。2026-10-31 はサイトが自動の取得を拒み、Internet Archive の 2026-10-12 の写しで確かめた

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

### spec-rfc_4180

- 名前: RFC 4180 Common Format and MIME Type for Comma-Separated Values (CSV) Files
- 公開元: IETF
- 版: 2005-10
- 入手の条件: 無償公開
- 入手先: [RFC 4180 Common Format and MIME Type for Comma-Separated Values (CSV) Files](https://www.rfc-editor.org/rfc/rfc4180)
- 確認日: 2026-11-01

### spec-stbridge_2_1_1

- 名前: ST-Bridge XML ファイル仕様書
- 公開元: buildingSMART Japan
- 版: ver.2.1.1（2026-04-15。公開は 2026-07-29）
- 入手の条件: 無償公開
- 入手先: [ST-Bridge XML ファイル仕様書](https://www.building-smart.or.jp/meeting/buildall/structural-design/)
- 確認日: 2026-10-24

### spec-sxf_3_1

- 名前: SXF Ver.3.1 仕様書
- 公開元: 国土交通省
- 版: Ver.3.1 第 2 版（2009-06）
- 入手の条件: 無償公開
- 入手先: [SXF Ver.3.1 仕様書](https://www.cals-ed.go.jp/sxf_ver3-1_specification_draft/)
- 確認日: 2026-10-24
- 補足: 2026-10-24 はサイトにつながらず、Internet Archive の 2026-02-04 の写しで確かめた

### spec-sxf_display_r7_12

- 名前: SXF 表示機能及び確認機能要件書（案）
- 公開元: 国土交通省
- 版: 令和 7 年 12 月版
- 入手の条件: 無償公開
- 入手先: [SXF 表示機能及び確認機能要件書（案）](https://www.cals-ed.go.jp/sxf_ver3-1_specification_draft/)
- 確認日: 2026-10-24
- 補足: 2026-10-24 はサイトにつながらず、Internet Archive の 2026-02-04 の写しで確かめた

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
- 権利の根拠: 3.0 の文書と XSD・JSON のスキーマは CC BY-ND 4.0 で、実装についての記述はない（IFC 4.3 の文書と同じ扱い）。スキーマは同梱しない。2.1 にはライセンスの記述がなく、zumen-jp/zumen#440 で確かめる。商標の注記（buildingSMART・openBIM・IFC）は BCF を挙げていない
- 仕様: [bcf\_xml\_2\_1](#spec-bcf_xml_2_1)、[bcf\_xml\_3\_0](#spec-bcf_xml_3_0)、[bcf\_api\_3\_0](#spec-bcf_api_3_0)
- 実装の根拠: spec
- 権利の根拠にした文書: [IFC4 ADD2 TC1 の文書の著作権の注記（商標の注記を含む）](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/copyright.htm)（確認日: 2026-10-31）
- issue: [zumen-jp/zumen#429](https://github.com/zumen-jp/zumen/issues/429)

### format-bebridge

- 名前: BE-Bridge
- 権利の根拠: Ver.6.1 の仕様書は公開されていて、条件の記述はない。2015-04 に建築保全センターが Stem・BE-Bridge を承継した。承継した側の条件と、最新の版の入手先は見つからなかった
- 仕様: [bebridge\_6\_1](#spec-bebridge_6_1)
- 権利の根拠にした文書: [BIM ライブラリーコンソーシアム「BLC-BIM オブジェクト標準（Version1.0）報告書」（2018-10-04）の「1.4 Stem、BE-Bridge 等の承継」](https://blcj.or.jp/wp/wp-content/uploads/2020/03/blcj_bim_standard_v1.pdf)（確認日: 2026-10-24）
- issue: [zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)

### format-boring

- 名前: ボーリング交換用データ（BED）
- 権利の根拠: 国土交通省が要領と DTD を無償で公開している。実装を制限する記述はない。DTD は転載が禁じられているので同梱しない
- 仕様: [ed\_boring\_r8\_2](#spec-ed_boring_r8_2)
- 実装の根拠: spec
- 権利の根拠にした文書: [KuniJiban の利用規約](https://www.kunijiban.pwri.go.jp/jp/terms.html)（確認日: 2026-11-01）
- サンプルの出どころ（Git に入れない）: 国土地盤情報検索サイト KuniJiban の柱状図（規約は複製と頒布を認め、出典の表示を求める）。XML で取れるかと個人情報の有無を #390 で確かめる
- issue: [zumen-jp/zumen#390](https://github.com/zumen-jp/zumen/issues/390)
- 形式の文書: [docs/formats/boring.md](../../docs/formats/boring.md)

### format-cedxm

- 名前: CEDXM
- 権利の根拠: 評議会の Q&amp;A は、形式を会員にだけ開示し、会員だけが使う、とする。会員は、読み書きするソフトを自由に作って売れる。実装を公開すると、会員でない人に形式を開示することになる
- 仕様: [cedxm](#spec-cedxm)
- 権利の根拠にした文書: [シーデクセマ評議会の Q&amp;A](https://www.cedxm.com/about/faq/)（確認日: 2026-10-24）
- issue: [zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)

### format-citygml

- 名前: CityGML（PLATEAU の 3D 都市モデルと i-UR を含む）
- 権利の根拠: OGC が CityGML を、国土交通省が PLATEAU の製品仕様書（サイトの内容は PDL 1.0）を無償で公開している。実装を制限する記述はない。3D 都市モデルのデータの権利は各自治体にあり、測量法に注意するよう書かれているので、データは同梱しない
- 仕様: [citygml\_2\_0](#spec-citygml_2_0)、[citygml\_3\_0\_gml](#spec-citygml_3_0_gml)、[plateau\_5\_1](#spec-plateau_5_1)
- 実装の根拠: spec
- 権利の根拠にした文書: [PLATEAU のサイトポリシー](https://www.mlit.go.jp/plateau/site-policy/)（確認日: 2026-11-01）
- 照合するソフト: PLATEAU VIEW・QGIS
- サンプルの出どころ（Git に入れない）: PLATEAU のデータ（G 空間情報センター。多くは政府標準利用規約 2.0）。大きいので ZUMEN\_SAMPLES に置く。再配布できる 5 MB 未満の公式の試料は見つかっていない
- 既知の crate: nusamai-citygml・ecitygml-io
- issue: [zumen-jp/zumen#379](https://github.com/zumen-jp/zumen/issues/379)
- 形式の文書: [docs/formats/citygml.md](../../docs/formats/citygml.md)

### format-csv

- 名前: CSV
- 権利の根拠: IETF の RFC で、誰でも読める。実装を制限する記述はない
- 仕様: [rfc\_4180](#spec-rfc_4180)
- 実装の根拠: spec
- サンプルの出どころ（Git に入れない）: 試験の中で作る
- issue: [zumen-jp/zumen#391](https://github.com/zumen-jp/zumen/issues/391)
- 形式の文書: [docs/formats/sheet.md](../../docs/formats/sheet.md)

### format-delivery

- 名前: 電子納品の管理ファイル（INDEX\_C.XML・PHOTO.XML など）
- 権利の根拠: 国土交通省が要領・基準と DTD を誰でも無償で入手できる形で公開している。実装を制限する記述はない。電子納品の頁は転載と改変を禁じるので、DTD・作成例・本文は写さず同梱しない
- 仕様: [ed\_const\_r8\_2](#spec-ed_const_r8_2)、[ed\_design\_r8\_2](#spec-ed_design_r8_2)、[ed\_eizen\_r3](#spec-ed_eizen_r3)、[ed\_photo\_r5\_3](#spec-ed_photo_r5_3)
- 実装の根拠: spec
- 権利の根拠にした文書: [国土交通省 電子納品に関する要領・基準の「リンク・著作権」](https://www.cals-ed.go.jp/copyright/)（確認日: 2026-11-01）
- 照合するソフト: 国土交通省の電子納品チェックシステム（手元でだけ使う）
- サンプルの出どころ（Git に入れない）: 再配布を認める公開の試料は見つかっていない。国土交通省の作成例は転載が禁じられているので、試験の中で作る
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
- 照合するソフト: ODA File Converter・DWG TrueView
- サンプルの出どころ（Git に入れない）: ACadSharp の試験の図面（MIT）。同じ図面を DXF にしたものと比べる・ezdwg の試験の図面（MIT）
- 記録: [決定 0008](../decisions/0008-model-layers-and-codecs.md)
- 旧版の記録: 旧版の決定 0003（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）
- 形式の文書: [docs/formats/dwg.md](../../docs/formats/dwg.md)

### format-dxf

- 名前: DXF
- 権利の根拠: Autodesk が DXF Reference を公開している。実装を制限する記述はない。DXF と AutoCAD は Autodesk の商標
- 仕様: [dxf\_2026](#spec-dxf_2026)
- 実装の根拠: spec・inferred・reference
- 参照した実装: [ezdxf](https://ezdxf.mozman.at/)（MIT）
- 照合するソフト: ezdxf・ODA File Converter・DWG TrueView
- サンプルの出どころ（Git に入れない）: ACadSharp の試験の図面（MIT）・ezdwg の試験の図面（MIT）・自治体などが公開する図面。再配布できるかは確かめていない
- 記録: [決定 0008](../decisions/0008-model-layers-and-codecs.md)
- 形式の文書: [docs/formats/dxf.md](../../docs/formats/dxf.md)

### format-e57

- 名前: E57（ASTM E2807）
- 権利の根拠: 仕様は ASTM が売り、実装を制限する記述はない。販売のページが規格を AI で扱うことを禁じるので、本文は読まず、許容のライセンスの公開の実装と試料から読む
- 仕様: [e57\_astm\_e2807](#spec-e57_astm_e2807)
- 実装の根拠: reference
- 権利の根拠にした文書: [libE57Format-test-data の README（ライセンス）](https://github.com/asmaloney/libE57Format-test-data)（確認日: 2026-11-01）
- 参照した実装: [libE57Format](https://github.com/asmaloney/libE57Format)（BSL-1.0）
- 参照した実装: [e57（Rust）](https://github.com/cry-inc/e57)（MIT）
- 照合するソフト: CloudCompare
- サンプルの出どころ（Git に入れない）: libE57Format-test-data（reference は libe57 の Test Data License、ほかは CC0 1.0）
- 既知の crate: e57
- issue: [zumen-jp/zumen#373](https://github.com/zumen-jp/zumen/issues/373)
- 形式の文書: [docs/formats/e57.md](../../docs/formats/e57.md)

### format-extdm

- 名前: 拡張 DM（公共測量標準図式数値地形図データファイル）
- 権利の根拠: 国土地理院が実装規約を技術資料として無償で公開している。技術資料は、特段の記載がなければ公共データ利用規約（PDL 1.0）を準用する。実装を制限する記述はない
- 仕様: [extdm\_a1\_291](#spec-extdm_a1_291)
- 実装の根拠: spec
- 権利の根拠にした文書: [国土地理院の技術資料の利用規約](https://www.gsi.go.jp/REPORT/TECHNICAL/riyoukiyaku_gijutsushiryou.html)（確認日: 2026-11-01）
- 照合するソフト: 基盤地図情報ビューアー（拡張 DM で書き出せる。手元でだけ使う）
- サンプルの出どころ（Git に入れない）: 再配布を認める公開の試料は見つかっていない。試験の中で作る
- issue: [zumen-jp/zumen#378](https://github.com/zumen-jp/zumen/issues/378)

### format-fgd

- 名前: 基盤地図情報（GML）
- 権利の根拠: 国土地理院がファイル仕様書と GML のスキーマを無償で公開している。実装を制限する記述はない。基盤地図情報は測量成果で、データの複製と使用には測量法の承認（基盤地図情報は無償で承認する）が要る場合があるので、データは同梱せず利用者が渡す（決定 0019 のジオイド・モデルと同じ扱い）
- 仕様: [fgd\_dl\_4\_1](#spec-fgd_dl_4_1)
- 実装の根拠: spec
- 権利の根拠にした文書: [国土地理院「測量成果の複製・使用承認の Q&amp;A」](https://www.gsi.go.jp/LAW/2930-qa.html)（確認日: 2026-11-01）
- 照合するソフト: 基盤地図情報ビューアー・QGIS
- サンプルの出どころ（Git に入れない）: 利用者が国土地理院から入手したデータ。Git・パッケージ・試験に入れない。試験の中で小さな GML を作る
- 記録: [決定 0019](../decisions/0019-coordinate-systems-and-gsi-data.md)
- issue: [zumen-jp/zumen#378](https://github.com/zumen-jp/zumen/issues/378)

### format-ids

- 名前: IDS（Information Delivery Specification）
- 権利の根拠: 文書と XSD は CC BY-ND 4.0 で、実装についての記述はない（IFC 4.3 の文書と同じ扱い）。XSD と、そこから生成したコードは同梱しない。照合は文書の記述から書く
- 仕様: [ids\_1\_0](#spec-ids_1_0)
- 実装の根拠: spec
- 権利の根拠にした文書: [IDS のリポジトリのライセンス（LICENSE）](https://github.com/buildingSMART/IDS/blob/development/LICENSE)（確認日: 2026-10-31）
- 記録: [決定 0207](../decisions/0207-building-model-components.md)
- 記録: [決定 0217](../decisions/0217-ids-check.md)
- issue: [zumen-jp/zumen#411](https://github.com/zumen-jp/zumen/issues/411)

### format-ifc

- 名前: IFC
- 権利の根拠: 文書は CC BY-ND 4.0 で、ライセンスの外の使い方には事前の書面の同意が要る。実装についての記述はない。buildingSMART International は、自らの検証の道具を MIT で公開している。スキーマと、そこから生成したコードは同梱しない。IFC2X3 TC1 と IFC4 ADD2 TC1 の文書は CC BY-NC-ND 4.0 で、営利の利用との関係を zumen-jp/zumen#440 で確かめる。IFC は buildingSMART International の商標
- 仕様: [ifc\_4\_3\_2](#spec-ifc_4_3_2)、[ifc\_2x3\_tc1](#spec-ifc_2x3_tc1)、[ifc\_4\_0\_2\_1](#spec-ifc_4_0_2_1)
- 実装の根拠: spec
- 権利の根拠にした文書: [IFC 4.3.x の文書の表紙（ライセンスと連絡の注意）](https://github.com/buildingSMART/IFC4.x-development/blob/master/content/cover.md)（確認日: 2026-10-24）
- 権利の根拠にした文書: [IFC4 ADD2 TC1 の文書の著作権の注記（IFC2X3 TC1 も同じ文面）](https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/copyright.htm)（確認日: 2026-10-31）
- 文書の補足: Internet Archive の写しで確かめた
- 権利の根拠にした文書: [buildingSMART International の Intellectual Property Policy（2021-06）](https://www.buildingsmart.org/wp-content/uploads/2021/06/bSI-Intellectual-Property-Policy.pdf)（確認日: 2026-10-31）
- 文書の補足: 実装の許諾の記述はない。Internet Archive の 2022-03-02 の写しで確かめた
- 権利の根拠にした文書: [buildingSMART International の検証の道具（validate）](https://github.com/buildingSMART/validate)（確認日: 2026-10-24）
- 照合するソフト: IfcOpenShell（手元でだけ使う）
- サンプルの出どころ（Git に入れない）: 日本で使う BIM ソフトの書き出し（zumen-jp/zumen-legacy#128）
- 記録: [決定 0008](../decisions/0008-model-layers-and-codecs.md)
- 記録: [決定 0207](../decisions/0207-building-model-components.md)
- 記録: [決定 0211](../decisions/0211-ifc-l1-instance-graph.md)
- 記録: [決定 0244](../decisions/0244-ifc-in-place-edit.md)
- 旧版の記録: 旧版の決定 0028（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）
- 形式の文書: [docs/formats/ifc.md](../../docs/formats/ifc.md)

### format-isg

- 名前: ISG 形式（ジオイド高と基準面補正量の格子）
- 権利の根拠: ISG が書式の文書を公開している。実装を制限する記述はない。国土地理院のジオイド・モデル（JPGEO2024 など）は基本測量の測量成果で、複製と使用に測量法の承認が要る場合があるので、同梱せず利用者が渡す（決定 0019）
- 仕様: [isg\_2\_0](#spec-isg_2_0)
- 実装の根拠: spec
- 権利の根拠にした文書: [「ジオイド2024日本とその周辺」と「基準面補正パラメータ」の説明書（2026 年 4 月）](https://www.gsi.go.jp/common/000277542.pdf)（確認日: 2026-10-31）
- 照合するソフト: 国土地理院の内挿計算プログラム（geoidcalc）
- サンプルの出どころ（Git に入れない）: 利用者が国土地理院から入手したジオイド・モデル。Git・パッケージ・試験に入れない
- 記録: [決定 0019](../decisions/0019-coordinate-systems-and-gsi-data.md)
- 形式の文書: [docs/formats/crs.md](../../docs/formats/crs.md)

### format-jpeg

- 名前: 工事写真（JPEG と Exif・XMP）
- 権利の根拠: Exif は CIPA が、XMP は Adobe が無償で公開している。JPEG の ITU-T T.81 は有償だが実装の制限はなく、画素の展開は許容のライセンスの crate に任せ、zumen は Exif と XMP の区画だけを読む
- 仕様: [jpeg\_t81](#spec-jpeg_t81)、[exif\_3\_1](#spec-exif_3_1)、[xmp\_2012](#spec-xmp_2012)
- 実装の根拠: spec
- サンプルの出どころ（Git に入れない）: 個人情報（位置・機種の番号など）を含まないよう、試験の中で作るか、zumen-jp が作った写真を CC0 で入れる
- issue: [zumen-jp/zumen#387](https://github.com/zumen-jp/zumen/issues/387)
- 形式の文書: [docs/formats/jpeg.md](../../docs/formats/jpeg.md)

### format-jwc

- 名前: JWC（Jw\_cad for DOS の図面）
- 権利の根拠: 公式の説明は見つからない。今の Jw\_cad も開いて保存できる形式を、保存したファイルどうしを比べて読む。実装を制限する記述はない
- 仕様: 登録なし
- 実装の根拠: inferred・reference
- 参照した実装: [ezjww](https://github.com/monozukuri-ai/ezjww)（MIT）
- 照合するソフト: Jw\_cad
- サンプルの出どころ（Git に入れない）: Jw\_cad 10 で保存したファイル
- 記録: [決定 0008](../decisions/0008-model-layers-and-codecs.md)

### format-jws

- 名前: JWS（Jw\_cad の図形）
- 権利の根拠: 公式の説明は見つからない。Jw\_cad の掲示板に投稿された説明（版 2.30）と、登録した図形と元の図面を比べた結果で読む。実装を制限する記述はない
- 仕様: 登録なし
- 実装の根拠: inferred・reference
- 参照した実装: [ezjww](https://github.com/monozukuri-ai/ezjww)（MIT）
- 照合するソフト: Jw\_cad
- サンプルの出どころ（Git に入れない）: Jw\_cad 10 で登録した図形
- 記録: [決定 0008](../decisions/0008-model-layers-and-codecs.md)
- 補足: 掲示板の説明: https://www.jwcad.net/log/consult2/consult2-0610-12/thread17752.htm

### format-jww

- 名前: JWW（Jw\_cad for Windows の図面）
- 権利の根拠: Jw\_cad が、データ形式の説明を公開している。実装を制限する記述はない
- 仕様: [jww\_7\_02](#spec-jww_7_02)
- 実装の根拠: spec・reference
- 参照した実装: [ezjww](https://github.com/monozukuri-ai/ezjww)（MIT）
- 照合するソフト: Jw\_cad
- サンプルの出どころ（Git に入れない）: Jw\_cad に付くサンプル・ezjww のリポジトリにある図面
- 記録: [決定 0008](../decisions/0008-model-layers-and-codecs.md)
- 記録: [決定 0133](../decisions/0133-jww-l1-records.md)
- 記録: [決定 0134](../decisions/0134-jww-lift-to-drawing.md)
- 記録: [決定 0135](../decisions/0135-jww-blocks-and-dimensions.md)
- 記録: [決定 0136](../decisions/0136-jww-images-and-edit-state.md)
- 形式の文書: [docs/formats/jww.md](../../docs/formats/jww.md)

### format-kokuban

- 名前: 電子小黒板の情報と信憑性確認（改ざん検知）
- 権利の根拠: J-COMSIA の実装規約は公開されておらず、協会の会員か検定を受けるソフトの作り手に出すものとみられる。改ざん検知は SHA-256 のハッシュを使うとだけ公開している。zumen は規約を持たない
- 仕様: [kokuban\_jcomsia](#spec-kokuban_jcomsia)
- 権利の根拠にした文書: [J-COMSIA の合格ソフトの一覧（信憑性確認の Ver.1 と Ver.1.1）](https://www.jcomsia.org/kokuban/software/?v=2)（確認日: 2026-11-01）
- 権利の根拠にした文書: [J-COMSIA の入会の案内](https://www.jcomsia.org/nyukai/)（確認日: 2026-11-01）
- 照合するソフト: J-COMSIA の信憑性チェックツール（利用の登録が要る）
- issue: [zumen-jp/zumen#389](https://github.com/zumen-jp/zumen/issues/389)

### format-landxml

- 名前: J-LandXML（LandXML 1.2 に準じた 3 次元設計データ交換標準）
- 権利の根拠: OCF と国土交通省が仕様の PDF を誰でも無償で入手できる形で公開している。実装を制限する記述はない。元の LandXML 1.2 のスキーマは同梱しない
- 仕様: [jlandxml\_1\_8](#spec-jlandxml_1_8)
- 実装の根拠: spec
- 照合するソフト: 国土交通省の基準に対応した 3 次元設計データの閲覧ソフト（iLEX など。手元でだけ使う）
- サンプルの出どころ（Git に入れない）: 再配布を認める公開の試料は見つかっていない。試験の中で小さな J-LandXML を作る
- issue: [zumen-jp/zumen#366](https://github.com/zumen-jp/zumen/issues/366)
- issue: [zumen-jp/zumen#368](https://github.com/zumen-jp/zumen/issues/368)
- 形式の文書: [docs/formats/landxml.md](../../docs/formats/landxml.md)

### format-las

- 名前: LAS・LAZ
- 権利の根拠: ASPRS が LAS の仕様を、OGC が LAZ の仕様を無償で公開している。OGC は LAZ を誰でも実装できるとする。LAZ を展開する LASzip と Rust の laz は、どちらも Apache-2.0
- 仕様: [las\_1\_4\_r15](#spec-las_1_4_r15)、[laz\_1\_4](#spec-laz_1_4)
- 実装の根拠: spec
- 権利の根拠にした文書: [OGC の LAZ 1.4 コミュニティ標準の公表](https://www.ogc.org/announcement/ogc-announces-publication-of-the-laz-1-4-community-standard/)（確認日: 2026-11-01）
- 権利の根拠にした文書: [LASzip のリポジトリ（ライセンス）](https://github.com/LASzip/LASzip)（確認日: 2026-11-01）
- 文書の補足: GitHub のライセンスの判定は Apache-2.0
- 照合するソフト: CloudCompare・PDAL・LAStools の lasinfo
- サンプルの出どころ（Git に入れない）: laz-rs の tests/data（Apache-2.0。数十 KB から数百 KB）
- 既知の crate: las・laz
- issue: [zumen-jp/zumen#371](https://github.com/zumen-jp/zumen/issues/371)
- 形式の文書: [docs/formats/las.md](../../docs/formats/las.md)

### format-pdf

- 名前: PDF
- 権利の根拠: ISO の規格で、PDF Association が無償で配っている。実装を制限する記述はない
- 仕様: [iso\_32000\_2\_2020](#spec-iso_32000_2_2020)
- 実装の根拠: spec
- 照合するソフト: pdf.js
- サンプルの出どころ（Git に入れない）: CAD が出力した図面の PDF（手元で作る）
- 記録: [決定 0013](../decisions/0013-boundary-in-four-forms.md)
- 旧版の記録: 旧版の決定 0020、旧版の決定 0030（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）
- 補足: ページの解釈は hayro-interpret（Apache-2.0 OR MIT）を使う予定（旧版の決定 0020）

### format-ply

- 名前: PLY（点群）
- 権利の根拠: 持ち主の公式の仕様はない。Greg Turk の説明と、それを写した公開の解説で読む。利用の制限の記述はない
- 仕様: 登録なし
- 実装の根拠: inferred
- 権利の根拠にした文書: [Paul Bourke「PLY - Polygon File Format」](https://paulbourke.net/dataformats/ply/)（確認日: 2026-11-01）
- 照合するソフト: CloudCompare・MeshLab
- サンプルの出どころ（Git に入れない）: 試験の中で作る
- issue: [zumen-jp/zumen#373](https://github.com/zumen-jp/zumen/issues/373)
- 形式の文書: [docs/formats/ply.md](../../docs/formats/ply.md)

### format-saf

- 名前: SXF の属性ファイル（SAF）
- 権利の根拠: SXF Ver.3.1 仕様書の附属書（属性付加機構）に含まれ、SXF と同じ扱い。実装を制限する記述はない。附属書の名前は解説の記事で確かめた
- 仕様: [sxf\_3\_1](#spec-sxf_3_1)
- 実装の根拠: spec
- 照合するソフト: SXF ブラウザ
- issue: [zumen-jp/zumen#386](https://github.com/zumen-jp/zumen/issues/386)

### format-sima

- 名前: SIMA 共通フォーマット
- 権利の根拠: 日本測量機器工業会（JSIMA）の公式のページの中身を確かめられなかった。形式の定義書を冊子で頒布したとする二次の資料と、国立国会図書館の JSIMA 203:2000（Ver.02）の記録がある。最新の版・入手の条件・実装の条件は確かめられない
- 仕様: 登録なし
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
- 記録: [決定 0017](../decisions/0017-format-rights-ledger-and-plugin-sdk.md)
- 記録: [決定 0243](../decisions/0243-plugin-bundling.md)
- 記録: [決定 0245](../decisions/0245-building-format-plugins.md)
- 旧版の記録: 旧版の決定 0001、旧版の決定 0013、旧版の決定 0028（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）
- issue: [zumen-jp/zumen-legacy#140](https://github.com/zumen-jp/zumen-legacy/issues/140)
- issue: [zumen-jp/zumen#402](https://github.com/zumen-jp/zumen/issues/402)

### format-stem

- 名前: Stem（設備機器ライブラリデータ交換仕様）
- 権利の根拠: C-CADEC が作り、2015-04 に建築保全センターが承継した。仕様書の入手先と条件は見つからなかった
- 仕様: 登録なし
- 権利の根拠にした文書: [BIM ライブラリーコンソーシアム「BLC-BIM オブジェクト標準（Version1.0）報告書」（2018-10-04）の「1.4 Stem、BE-Bridge 等の承継」](https://blcj.or.jp/wp/wp-content/uploads/2020/03/blcj_bim_standard_v1.pdf)（確認日: 2026-10-24）
- issue: [zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)

### format-sxf

- 名前: SXF（SFC・P21 と、それを ZIP に入れた SFZ・P2Z）
- 権利の根拠: 国土交通省が、開発者向けに仕様書を公開している。実装を制限する記述はない。頁の転載と改変は禁じているので、仕様の本文は写さない
- 仕様: [sxf\_3\_1](#spec-sxf_3_1)、[sxf\_display\_r7\_12](#spec-sxf_display_r7_12)
- 実装の根拠: spec・inferred・reference
- 権利の根拠にした文書: [国土交通省 電子納品に関する要領・基準の「リンク・著作権」](https://www.cals-ed.go.jp/copyright/)（確認日: 2026-10-24）
- 文書の補足: サイトにつながらず、Internet Archive の 2026-05-13 の写しで確かめた
- 参照した実装: [ezsxf](https://github.com/monozukuri-ai/ezsxf)（MIT）
- 照合するソフト: Jw\_cad・SXF ブラウザ
- サンプルの出どころ（Git に入れない）: ezsxf のリポジトリにある公開の図面
- 記録: [決定 0008](../decisions/0008-model-layers-and-codecs.md)
- 記録: [決定 0120](../decisions/0120-sxf-l1-exchange-structure.md)
- 記録: [決定 0121](../decisions/0121-sxf-sfc-lift-to-drawing.md)
- 記録: [決定 0122](../decisions/0122-sxf-text-to-formatted-text.md)
- 記録: [決定 0123](../decisions/0123-drawing-clothoid-edge.md)
- 記録: [決定 0124](../decisions/0124-drawing-title-blocks.md)
- 記録: [決定 0125](../decisions/0125-sxf-raster-underlays.md)
- 記録: [決定 0126](../decisions/0126-sxf-splines-and-point-markers.md)
- 記録: [決定 0127](../decisions/0127-sxf-dimensions.md)
- 記録: [決定 0128](../decisions/0128-sxf-labels-and-balloons.md)
- 記録: [決定 0129](../decisions/0129-sxf-hatches-and-composite-curves.md)
- 記録: [決定 0130](../decisions/0130-sxf-parts-symbols-and-nested-views.md)
- 記録: [決定 0131](../decisions/0131-sxf-attribute-mechanism.md)
- 記録: [決定 0132](../decisions/0132-sxf-p21-through-sfc-features.md)
- issue: [zumen-jp/zumen#385](https://github.com/zumen-jp/zumen/issues/385)
- 形式の文書: [docs/formats/sxf.md](../../docs/formats/sxf.md)

### format-xlsx

- 名前: Excel の表（xlsx）
- 権利の根拠: Ecma International が ECMA-376 を無償で公開している。実装を制限する記述はない
- 仕様: [ecma\_376\_5](#spec-ecma_376_5)
- 実装の根拠: spec
- サンプルの出どころ（Git に入れない）: 試験の中で作る
- issue: [zumen-jp/zumen#391](https://github.com/zumen-jp/zumen/issues/391)
- 形式の文書: [docs/formats/sheet.md](../../docs/formats/sheet.md)

### format-xyz

- 名前: XYZ（座標を並べた点群のテキスト）
- 権利の根拠: 決まった仕様も持ち主もない。座標と属性を区切って並べたテキストを、推定で読む
- 仕様: 登録なし
- 実装の根拠: inferred
- 照合するソフト: CloudCompare
- サンプルの出どころ（Git に入れない）: 試験の中で作る
- issue: [zumen-jp/zumen#373](https://github.com/zumen-jp/zumen/issues/373)
- 形式の文書: [docs/formats/xyz.md](../../docs/formats/xyz.md)
