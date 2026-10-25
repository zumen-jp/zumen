<!-- formats/ledger.toml から cargo xtask codegen で生成。直接編集しない。 -->

# 形式と権利の一覧

[権利の台帳](../../formats/ledger.toml)から生成しています。ここにあるのは実装を置く方針であり、読み書きできる形式の一覧ではありません。変更の手順は [台帳の書き方](../../formats/README.md)、図面の要素の対応は [Drawing2D の対応表](drawing2d-mapping.md)を参照してください。

- 権利: `public` は実装の公開に第三者の許諾が不要、`permission` は契約・会員・許諾が必要、`unknown` は条件が未確認です。
- 置き場所: `core` は核、`plugin` は外部のプラグイン、`none` は実装しない方針です。
- 台帳は判断した時点の記録です。URL の到達や権利の変更は、生成時に再確認していません。

## 一覧

| キー | 名前 | 系統 | 領域 | 権利 | 置き場所 |
| --- | --- | --- | --- | --- | --- |
| [bebridge](#format-bebridge) | BE-Bridge | bebridge | building | `unknown` | `none` |
| [cedxm](#format-cedxm) | CEDXM | cedxm | building | `permission` | `none` |
| [dwg](#format-dwg) | DWG | acad | drawing | `public` | `core` |
| [dxf](#format-dxf) | DXF | acad | drawing | `public` | `core` |
| [ifc](#format-ifc) | IFC | ifc | building・civil | `public` | `core` |
| [jwc](#format-jwc) | JWC（Jw\_cad for DOS の図面） | jwcad | drawing | `public` | `core` |
| [jws](#format-jws) | JWS（Jw\_cad の図形） | jwcad | drawing | `public` | `core` |
| [jww](#format-jww) | JWW（Jw\_cad for Windows の図面） | jwcad | drawing | `public` | `core` |
| [pdf](#format-pdf) | PDF | pdf | drawing | `public` | `core` |
| [stbridge](#format-stbridge) | ST-Bridge | stbridge | building | `permission` | `plugin` |
| [stem](#format-stem) | Stem（設備機器ライブラリデータ交換仕様） | stem | building | `unknown` | `none` |
| [sxf](#format-sxf) | SXF | sxf | drawing | `public` | `core` |

## 仕様の入手先

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

### spec-dxf_2026

- 名前: DXF Reference
- 公開元: Autodesk
- 版: AutoCAD 2026
- 入手の条件: 無償公開
- 入手先: [DXF Reference](https://help.autodesk.com/cloudhelp/2026/ENU/AutoCAD-DXF/files/GUID-235B22E0-A567-4CF6-92D3-38A2306D73F3.htm)
- 確認日: 2026-10-24

### spec-ifc_4_3_2

- 名前: IFC 4.3.2 Documentation
- 公開元: buildingSMART International
- 版: IFC 4.3.2.0（ISO 16739-1:2024 と同じ構造と意味）
- 入手の条件: 無償公開
- 入手先: [IFC 4.3.2 Documentation](https://ifc43-docs.standards.buildingsmart.org/)
- 確認日: 2026-10-24

### spec-iso_32000_2_2020

- 名前: ISO 32000-2:2020（PDF 2.0）
- 公開元: ISO
- 版: 2020
- 入手の条件: 無償公開
- 入手先: [ISO 32000-2:2020（PDF 2.0）](https://pdfa.org/sponsored-standards/)
- 確認日: 2026-10-24
- 補足: PDF Association が、後援の企業の費用で無償で配っている。ISO からは買う

### spec-jww_7_02

- 名前: Jw\_cad のデータ形式（jwdatafmt.txt）
- 公開元: Jw\_cad
- 版: Jw\_cad 7.02（データの版 700）
- 入手の条件: 無償公開
- 入手先: [Jw\_cad のデータ形式（jwdatafmt.txt）](https://www.jwcad.net/jwdatafmt.txt)
- 確認日: 2026-10-24

### spec-ods_dwg_5_4_1

- 名前: Open Design Specification for .dwg files
- 公開元: Open Design Alliance
- 版: 5.4.1
- 入手の条件: 無償公開
- 入手先: [Open Design Specification for .dwg files](https://opendesign.com/files/guestdownloads/OpenDesign_Specification_for_.dwg_files.pdf)
- 確認日: 2026-10-24

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

## 形式ごとの根拠と記録

### format-bebridge

- 名前: BE-Bridge
- 権利の根拠: Ver.6.1 の仕様書は公開されていて、条件の記述はない。2015-04 に建築保全センターが Stem・BE-Bridge を承継した。承継した側の条件と、最新の版の入手先は見つからなかった
- 仕様: [bebridge\_6\_1](#spec-bebridge_6_1)
- 権利の根拠にした文書: [BIM ライブラリーコンソーシアム「BLC-BIM オブジェクト標準（Version1.0）報告書」（2018-10-04）の「1.4 Stem、BE-Bridge 等の承継」](https://blcj.or.jp/wp/wp-content/uploads/2020/03/blcj_bim_standard_v1.pdf)（確認日: 2026-10-24）
- issue: [zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)

### format-cedxm

- 名前: CEDXM
- 権利の根拠: 評議会の Q&amp;A は、形式を会員にだけ開示し、会員だけが使う、とする。会員は、読み書きするソフトを自由に作って売れる。実装を公開すると、会員でない人に形式を開示することになる
- 仕様: [cedxm](#spec-cedxm)
- 権利の根拠にした文書: [シーデクセマ評議会の Q&amp;A](https://www.cedxm.com/about/faq/)（確認日: 2026-10-24）
- issue: [zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)

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

### format-ifc

- 名前: IFC
- 権利の根拠: 文書は CC BY-ND 4.0 で、ライセンスの外の使い方には事前の書面の同意が要る。実装についての記述はない。buildingSMART International は、自らの検証の道具を MIT で公開している。スキーマと、そこから生成したコードは同梱しない。IFC は buildingSMART International の商標
- 仕様: [ifc\_4\_3\_2](#spec-ifc_4_3_2)
- 実装の根拠: spec
- 権利の根拠にした文書: [IFC 4.3.x の文書の表紙（ライセンスと連絡の注意）](https://github.com/buildingSMART/IFC4.x-development/blob/master/content/cover.md)（確認日: 2026-10-24）
- 権利の根拠にした文書: [buildingSMART International の検証の道具（validate）](https://github.com/buildingSMART/validate)（確認日: 2026-10-24）
- 照合するソフト: IfcOpenShell（手元でだけ使う）
- サンプルの出どころ（Git に入れない）: 日本で使う BIM ソフトの書き出し（zumen-jp/zumen-legacy#128）
- 旧版の記録: 旧版の決定 0028（[旧版の決定の一覧](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/README.md)）

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

### format-stem

- 名前: Stem（設備機器ライブラリデータ交換仕様）
- 権利の根拠: C-CADEC が作り、2015-04 に建築保全センターが承継した。仕様書の入手先と条件は見つからなかった
- 仕様: 登録なし
- 権利の根拠にした文書: [BIM ライブラリーコンソーシアム「BLC-BIM オブジェクト標準（Version1.0）報告書」（2018-10-04）の「1.4 Stem、BE-Bridge 等の承継」](https://blcj.or.jp/wp/wp-content/uploads/2020/03/blcj_bim_standard_v1.pdf)（確認日: 2026-10-24）
- issue: [zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)

### format-sxf

- 名前: SXF
- 権利の根拠: 国土交通省が、開発者向けに仕様書を公開している。実装を制限する記述はない。頁の転載と改変は禁じているので、仕様の本文は写さない
- 仕様: [sxf\_3\_1](#spec-sxf_3_1)、[sxf\_display\_r7\_12](#spec-sxf_display_r7_12)
- 実装の根拠: spec・inferred・reference
- 権利の根拠にした文書: [国土交通省 電子納品に関する要領・基準の「リンク・著作権」](https://www.cals-ed.go.jp/copyright/)（確認日: 2026-10-24）
- 文書の補足: サイトにつながらず、Internet Archive の 2026-05-13 の写しで確かめた
- 参照した実装: [ezsxf](https://github.com/monozukuri-ai/ezsxf)（MIT）
- 照合するソフト: Jw\_cad・SXF ブラウザ
- サンプルの出どころ（Git に入れない）: ezsxf のリポジトリにある公開の図面
- 記録: [決定 0008](../decisions/0008-model-layers-and-codecs.md)
