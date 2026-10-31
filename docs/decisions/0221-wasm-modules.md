# 0221: wasm のモジュールを、zumen-abi の feature で図面と BIM に分け、判定の規則とモジュールの表を schema/boundary.toml に置く

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

[決定 0013](0013-boundary-in-four-forms.md) は、wasm では形式の群ごとにモジュールを分け、どのモジュールも同じ関数を出し、入っていない op は診断で返すとした。#416 の前は、wasm のモジュールは `zumen_abi1.wasm` の 1 つだけで、図面だけを使う利用者も、IFC・建物モデル・BCF の分を読み込んでいた。次のことが決まっていなかった。

- どう分けて作るか。
- どの入力にどのモジュールが要るかを、包みがどう知るか。
- 適合試験を、どのモジュールで流すか。

## 選択肢

分け方は 2 つある。

1. 依存の crate を optional にし、feature で外す。
2. 依存はそのままにし、`zumen-abi` の op の振り分けだけを feature で外す。呼ばれないコードは、リンカーが落とす。

選択肢 1 は、crate の間の依存（図面のモデルを BIM も使うなど）を feature ごとに切り直す必要があり、変更が大きい。選択肢 2 で落ちることを、大きさで確かめた。

## 決定

選択肢 2 を採る。

### モジュール

- `zumen-abi` に feature `drawing`・`bim` を置き、既定は両方を入れる。ネイティブ・CLI・Go の包みは、今までどおり両方を入れた 1 つを使う。
- 両方を入れた `zumen_abi1.wasm` も配り続ける。分けたモジュールは `zumen_abi1_drawing.wasm`（`--no-default-features --features drawing`）と `zumen_abi1_bim.wasm`（`--features bim`）として足す。
- どのモジュールも、`capabilities`・`open`・`close`・`describe`・`review_carry` を持つ。`drawing` は図面の op（`drawing`・`entities`・`apply`・`convert`・`tables`・`scene`・`check`・`review_adopt`）と DXF・DWG・SXF・JWW を、`bim` は IFC と建物の op（`validate`・`building`・`building_elements`・`ids_check`・`review_adopt_ids`・`bcf_read`・`bcf_write`）を持つ。
- 入っていない op は、要求の欄を読む前に `call.op.unsupported` で返す。`open` は、入っていない形式を `call.format.unsupported` で返す。`capabilities` は、そのモジュールにある op と形式だけを返す。

### 表と判定の規則

- `schema/boundary.toml` に、モジュールの表（名前・説明・形式・op）と、形式の判定の規則（`open` が試す順に、バイト列・行・STEP のスキーマの接頭辞）を置く。`cargo xtask codegen` が、Rust の `MODULES`・`DETECTION`・`has_op` と、JS の `constants.js` の `MODULES`・`DETECTION` を生成する。
- 判定の規則は、各コーデックの `SIGNATURES` と同じであることを Rust の試験で、`open` が選ぶ形式と同じになることを JS の試験で確かめる。表の op がスキーマの要求と合うことは `codegen` が確かめる。

### JS の包み

- `detect(bytes)` は形式を、`modulesWith(op)` は op を持つモジュールを返す。
- `loadModules({ drawing, bim })` は、モジュール名から読み込み元を引く表を受け、`get`・`forInput`・`forOp` で、要るモジュールだけを初めて要るときに読み込む。

### 適合試験

組に `"module"` の欄を置く。値はモジュール名か `all` で、`all` は両方を入れたモジュールでだけ流す（`capabilities`・形式をまたぐ組）。欄がなければ、手順の入力と op から、それを持つモジュールを選ぶ。CI は、Node.js と Chromium（毎晩は Firefox と WebKit も）で、両方を入れたモジュールの一覧と、組ごとに分けたモジュールで流した一覧が、どちらも `hashes.txt` と一致することを確かめる。

### 大きさ（2026-10-31 の main、release、brotli は品質 11）

| モジュール | そのまま | brotli |
| --- | ---: | ---: |
| 両方 | 6,269,784 | 1,183,396 |
| drawing | 5,595,850 | 1,054,447 |
| bim | 1,560,631 | 403,444 |

図面だけの利用者は、およそ 11% 小さいものを読む。BIM だけなら、およそ 1/3 になる。図面のモジュールの大きさの多くは DWG・SXF・JWW・シーンのコードで、さらに分けるかは、利用者の求めが分かってから決める。

## 影響

- 土木・地理と点群のモジュール（#380）は、feature と表の行を足して作る。
- 新しい op を足すときは、`schema/boundary.toml` のどれかのモジュールに書く。書かなければ `codegen` が失敗する。
- npm の包みに分けたモジュールを入れるかは、配り方（[決定 0205](0205-release-and-publishing.md)）に合わせて、公開のときに決める。リリースの成果物には、3 つの wasm を置く。
