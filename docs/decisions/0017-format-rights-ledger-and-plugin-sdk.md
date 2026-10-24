# 0017: 形式の権利を台帳で管理し、許諾の要る形式は同じ SDK で外部のプラグインにする

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 旧版は、公開に第三者の許諾が要る形式と、寛容でないライセンスの依存を入れないと決め、ST-Bridge を非公開のリポジトリへ移した（[旧版の決定 0013](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0013-st-bridge-out-of-zumen.md)）。IFC は、文書のライセンスが実装に触れていないことなどから、許諾は要らないと扱った（[旧版の決定 0028](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0028-ifc-in-rust.md)）。
- この規則を守る手段は、AGENTS.md・リリースのチェックリスト・レビューだけだった。形式ごとの仕様の入手先・権利・実装の根拠は、形式の文書と決定の記録に散っていた。
- 依存のライセンスは、cargo-deny で検査している（[0011](0011-bindings-through-ffi.md)、`deny.toml`）。成果物に何も残さないビルド時の道具の扱いも、0011 で決めた（[#15](https://github.com/zumen-jp/zumen/issues/15)）。
- 形式を読み書きする部品は、ほかの記録で決めた。
  - `Codec`・`FileModel`・`Lift`・`Lower` と、文脈 `Cx`（[0008](0008-model-layers-and-codecs.md)）
  - 診断と code の台帳（[0009](0009-diagnostics-and-catalog.md)）、予算・パニックしない約束・fuzz（[0010](0010-budget-by-work.md)）
  - どこでも同じ結果の規則（[0007](0007-single-rust-implementation.md)）、境界と適合試験（[0012](0012-boundary-functions-and-messages.md)）、4 つの形と wasm のモジュールの分け方（[0013](0013-boundary-in-four-forms.md)）、版（[0015](0015-add-only-boundary-and-version-floors.md)）
- 2026-10-24 に、権利を確かめていた形式（[zumen-jp/zumen-legacy#140](https://github.com/zumen-jp/zumen-legacy/issues/140)・[zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)）の公式の頁を確かめた。

| 形式 | 持ち主 | 仕様の入手 | 実装の公開についての記述 |
| --- | --- | --- | --- |
| ST-Bridge（構造） | buildingSMART Japan | XML ファイル仕様書 ver.2.1.1 と XML Schema を公開している（2026-07-29） | 仕様書にはない。公開された実装の STB-KIT（MPL-2.0）は、ST-Bridge の権利は buildingSMART Japan にあり、同会との利用許諾契約に基づいて実装を公開している、と書く |
| CEDXM（木造のプレカット） | 特定非営利活動法人シーデクセマ評議会 | 会員にだけ配る | 評議会の Q&A は、形式を会員にだけ開示し、会員だけが使う、とする。会員は、読み書きするソフトを自由に作って売れる |
| BE-Bridge（設備の部材） | C-CADEC が作り、2015 年 4 月に建築保全センターが承継した | Ver.6.1（2014 年 4 月）の仕様書が、建設業振興基金のサイトにある | 仕様書にはない。承継した側の条件と、最新の版の入手先は見つからなかった |
| Stem（設備の機器） | BE-Bridge と同じ | 見つからなかった | 見つからなかった |

- crates.io には、ST-Bridge を読む crate（`stb-parser`、MIT）がある。ライセンスが寛容なので、cargo-deny のライセンスの検査では止まらない。

## 選択肢

### 台帳の形

1. **文書の表にする。** 人には読みやすいが、CI で検査できない。
2. **機械で読める TOML にし、検査と文書をそこから作る。** 診断の code の台帳（`catalog/codes.toml`、[0009](0009-diagnostics-and-catalog.md)）と同じ作り方。

### 台帳の置き場所

1. **`schema/`**（[#1](https://github.com/zumen-jp/zumen/issues/1) の素案）。今は、境界のスキーマの置き場所になっている（[0012](0012-boundary-functions-and-messages.md)）。
2. **`catalog/`。** 診断の code の台帳と文言で、核と一緒に配る（[0009](0009-diagnostics-and-catalog.md)）。配らない台帳が混ざる。
3. **新しい `formats/`。** 形式の台帳だけを置く。

### 外部のプラグインの束ね方

1. **実行時に読み込む。** ライブラリや wasm のモジュールを、あとから足す。
2. **ビルドの時に束ねる。** Cargo の依存と feature で、1 つの成果物にする。
3. **別のモジュールにして、L2 のバイナリでやり取りする。**

## 決定

台帳の形は 2、置き場所は 3 を選ぶ。束ね方は 1 を採らない。Rust とネイティブで呼ぶ言語では 2、wasm では 3 にする。

### 台帳

- `formats/ledger.toml` に置き、書き方を `formats/README.md` に書く。
- 仕様と形式を、それぞれ ID で持つ。
  - **仕様**: 名前・出している者・版・入手の条件・入手先・確かめた日。診断の `cite`（[0009](0009-diagnostics-and-catalog.md)）は、この ID で仕様を指す。
  - **形式**: 次の項目を持つ。

  | 項目 | 中身 |
  | --- | --- |
  | 系統・領域 | L1 の系統と L2 の領域（[0008](0008-model-layers-and-codecs.md)） |
  | 仕様 | 仕様の ID の並び。公式の仕様がなければ空 |
  | 権利 | `public`・`permission`・`unknown` と、その根拠と、根拠にした文書 |
  | 置き場所 | `core`・`plugin`・`none`。`plugin` ならリポジトリ |
  | 実装の根拠 | `spec`（仕様）・`inferred`（推定）・`reference`（参照した実装）と、参照した実装の名前とライセンス |
  | 照合 | 照合に使うソフト |
  | サンプル | サンプルの出どころ |
  | 既知の crate | crates.io にある、その形式を読み書きする crate |
  | 記録 | 関わる決定の記録と issue |

- **権利**
  - `public`: 実装を公開するのに、第三者の許諾が要らない。持ち主が仕様を公開して実装を制限していないもの、または持ち主の仕様がなく、公開された資料と推定で読むもの
  - `permission`: 持ち主が、仕様の入手か実装の公開に、契約・会員・許諾を求めている
  - `unknown`: 持ち主か、その条件を確かめられていない
- **置き場所**
  - `core`: このリポジトリの核のワークスペース。`public` の形式だけを置ける
  - `plugin`: 外部のリポジトリのプラグイン
  - `none`: 実装しない
- 形式のキーは、英小文字と数字だけにする（`sxf`・`stbridge` など）。境界の列挙の値（[0012](0012-boundary-functions-and-messages.md)）にも、診断の code の範囲（[0009](0009-diagnostics-and-catalog.md)）にも、そのまま使える。仕様の ID は、英小文字・数字・`_` にする。
- 事実（仕様の版・入手先・権利の根拠）は、公式の文書で確かめ、確かめた日を書く。仕様の本文は写さない。
- 台帳から、形式の一覧と権利の表の文書（`docs/formats/README.md`）を生成する。

最初の版は、今の形式と、権利を確かめていた形式を載せる。

| 形式 | 権利 | 置き場所 |
| --- | --- | --- |
| SXF・JWW・JWC・JWS・DXF・DWG・PDF・IFC | `public` | `core` |
| ST-Bridge | `permission` | `plugin`（非公開の `stbridge`、[0004](0004-repositories.md)） |
| CEDXM | `permission` | `none` |
| BE-Bridge・Stem | `unknown` | `none` |

### CI の検査

- crate は、実装する形式を `Cargo.toml` の `[package.metadata.zumen]` の `formats` で名乗る。
- `cargo xtask` の検査で、次を確かめる。
  - 台帳の形: 要る項目、値の語、キーの書き方、仕様の入手先と確かめた日、決定の記録のファイルがあること
  - `place` が `core` の形式は、`rights` が `public` であること
  - ワークスペースと、依存のすべての crate（`cargo metadata`）が名乗る形式が、台帳にあり、`place` が `core` であること。許諾の要る形式や、権利が不明の形式を名乗る crate が入ったら、失敗にする
  - 権利が `public` でない形式の既知の crate が、`deny.toml` の `[bans]` で禁じられていること
- 名乗らない crate は、cargo-deny で止める。
  - 権利が `public` でない形式の既知の crate は、`deny.toml` の `[bans]` で禁じる。最初は `stb-parser`。
  - 依存のライセンスは、今のとおり許可リストで確かめる（[0011](0011-bindings-through-ffi.md)）。
- 名乗らずに形式を自分で書くことは、検査では見つけられない。形式を足す PR は台帳を一緒に変え、レビューで確かめる。

### プラグインの SDK

- crate `zumen-sdk` を作り、crates.io に出す（[0003](0003-names-and-domain.md) の `zumen-*`）。

  | 部品 | 中身 |
  | --- | --- |
  | trait | `Codec`・`FileModel`・`Lift`・`Lower`、文脈 `Cx`・予算・診断・中断の合図、形式の判定の規則（[0008](0008-model-layers-and-codecs.md)・[0009](0009-diagnostics-and-catalog.md)・[0010](0010-budget-by-work.md)） |
  | 登録 | プラグインの形式・コーデック・lift・lower を、窓口に知らせる記述。台帳と同じキーと権利を持つ |
  | テストキット | 合成のファイルを作る道具と、試験の枠組み。往復（L1 は読んで書くとバイト列が一致、L2 は lift → lower → lift で等価）、決定性（2 回読んで同じハッシュ、native と wasm32 で同じハッシュの一覧）、予算（同じ所で止まる） |
  | ID の契約試験 | 同じバイト列から、記録した一覧と同じ ID が出ることを確かめる。ID を変えるには、先に決定の記録が要る（AGENTS.md） |
  | fuzz | バイト列を受けて、パニックしない・予算の中で止まる・2 回読んで同じ結果になる、を確かめる関数。cargo-fuzz の target から呼ぶ（[0010](0010-budget-by-work.md)） |

- **核の形式も、同じ SDK で書く。** 核のコーデックと外部のプラグインで、書き方と試験を分けない。SDK は、核の形式を書きながら育てる。
- プラグインも、どこでも同じ結果の規則（[0007](0007-single-rust-implementation.md)）と、予算とパニックしない約束（[0010](0010-budget-by-work.md)）に従う。
- プラグインは、自分の診断の code の台帳と文言を持つ。code の範囲は形式のキーにし、核の code とぶつからないようにする（[0009](0009-diagnostics-and-catalog.md)）。
- SDK の版は、核と同じにする（[0015](0015-add-only-boundary-and-version-floors.md) のパッケージの版）。trait を変えることは Rust の API を壊すことなので、1.0 からは大版を上げる。

### 束ね方

| 使う所 | 束ね方 |
| --- | --- |
| Rust | プラグインの crate を依存に足し、窓口に登録する（ビルドの時に束ねる） |
| wasm（ブラウザ・Node.js・Go） | プラグインの形式を持つ、別のモジュールにする。同じ境界の関数を出し（[0012](0012-boundary-functions-and-messages.md)）、公式のモジュールとは L2 のバイナリ（[#6](https://github.com/zumen-jp/zumen/issues/6)）でやり取りする |
| ネイティブで呼ぶ言語 | プラグインのリポジトリが、核とプラグインを束ねた CLI を作る。各言語からは、その CLI を `process` の形（[0013](0013-boundary-in-four-forms.md)）で使う |

- 公式の成果物（crates.io の `zumen`、C ABI のライブラリ、npm の wasm のモジュール、CLI）は、核のワークスペースの形式だけを持つ。プラグインを実行時に読み込む仕組みは持たない。
  - Rust には安定した ABI がない。
  - 実行時に読み込むと、どこでも同じ結果と、受け取ったファイルを信用しない約束を、読み込んだコードに預けることになる。
  - wasm のモジュールは import を持たない（[0011](0011-bindings-through-ffi.md)）ので、ほかのモジュールを読み込めない。
- JavaScript の包みは、アプリが渡したプラグインのモジュールを、その判定の規則と op の表（データ）と一緒に受け付ける。形式を判定してから、要るモジュールだけを読み込む（[0013](0013-boundary-in-four-forms.md)）。
- 核とプラグインを束ねたネイティブのライブラリを、ホストのプロセスに読み込む形は、当面は作らない。公式のライブラリとファイル名と版がぶつかる（[0015](0015-add-only-boundary-and-version-floors.md) の「同じプロセスに並べる」）。要るときは記録を書く。

### 外部のリポジトリに置くもの

| 対象 | 扱い |
| --- | --- |
| ST-Bridge | 非公開の `stbridge` に置く。許諾を得たら、記録を書いて公開する |
| CEDXM | 作らない。作るなら、評議会に入って、実装を公開できるかを確かめてからにする |
| BE-Bridge・Stem | 権利が分かるまで作らない |
| 非公開の SDK や、寛容でないライセンスのライブラリを使う連携（商用の DWG の SDK、ホストアプリの API など） | 使う人が、同じ SDK で自分のリポジトリに作れる。zumen-jp の成果物には入れない |

## 影響

- 新しい形式を足す PR は、台帳の項目と形式の文書を一緒に入れる。判断を伴うなら、決定の記録も書く。
- 台帳の検査と文書の生成は、xtask に入れる。xtask は境界の crate の PR（[#23](https://github.com/zumen-jp/zumen/pull/23)）が変えているので、#23 のあとの PR で入れる。それまでは、レビューで台帳を確かめる。`deny.toml` の禁止は、この記録と同じ PR で入れる。
- `zumen-sdk` は、AutoCAD の系統の試作（[#3](https://github.com/zumen-jp/zumen/issues/3)）で、核のコーデックと一緒に作る。素案の trait の形（[0008](0008-model-layers-and-codecs.md)）も、そこで決まる。
- 旧版の `docs/formats/` を移すときに、台帳の各形式に文書のパスを足す。
- fuzz の crate が使う libfuzzer-sys のライセンスは `(MIT OR Apache-2.0) AND NCSA`。NCSA は寛容なライセンスだが、`deny.toml` の許可リストにない。fuzz を入れる PR で、理由を記録して許可リストに足す（[0011](0011-bindings-through-ffi.md)）。
- ST-Bridge・CEDXM・BE-Bridge・Stem の権利は、持ち主に問い合わせて確かめる（[zumen-jp/zumen-legacy#140](https://github.com/zumen-jp/zumen-legacy/issues/140)・[zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141) を続ける）。答えが出たら台帳を直し、扱いを変えるなら記録を書く。
- AGENTS.md の「範囲と権利」から、この記録と台帳を参照する。
- [#10](https://github.com/zumen-jp/zumen/issues/10) は、この記録と台帳の最初の版で完了する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「骨格」の 6 と「5. 層をまたぐ仕組み」の「権利の台帳とプラグイン」、[#10](https://github.com/zumen-jp/zumen/issues/10)
- [0003](0003-names-and-domain.md)、[0004](0004-repositories.md)、[0007](0007-single-rust-implementation.md)、[0008](0008-model-layers-and-codecs.md)、[0009](0009-diagnostics-and-catalog.md)、[0010](0010-budget-by-work.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0013](0013-boundary-in-four-forms.md)、[0015](0015-add-only-boundary-and-version-floors.md)
- [旧版の決定 0001](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0001-st-bridge-in-zumen.md)・[旧版の決定 0013](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0013-st-bridge-out-of-zumen.md)・[旧版の決定 0028](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0028-ifc-in-rust.md)
- [zumen-jp/zumen-legacy#140](https://github.com/zumen-jp/zumen-legacy/issues/140)・[zumen-jp/zumen-legacy#141](https://github.com/zumen-jp/zumen-legacy/issues/141)
- 2026-10-24 に確かめたもの
  - buildingSMART Japan の[構造設計小委員会の頁](https://www.building-smart.or.jp/meeting/buildall/structural-design/)（ST-Bridge XML ファイル仕様書 ver.2.1.1）、[STB-KIT](https://github.com/taisei-oss/stb-kit) の README の「ST-Bridge および IFC との関係」
  - シーデクセマ評議会の [Q&A](https://www.cedxm.com/about/faq/)
  - [BE-Bridge Ver.6.1 の仕様書](https://www.kensetsu-kikin.or.jp/assets/img/database/database27.pdf)、BIM ライブラリーコンソーシアムの[報告書](https://blcj.or.jp/wp/wp-content/uploads/2020/03/blcj_bim_standard_v1.pdf)（2018-10-04）の「1.4 Stem、BE-Bridge 等の承継」
  - [stb-parser](https://crates.io/crates/stb-parser)・[libfuzzer-sys](https://crates.io/crates/libfuzzer-sys) の crates.io の頁
  - [cargo-deny の bans の設定](https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html)
