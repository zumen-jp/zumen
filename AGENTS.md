# 開発エージェント向けの規則

このリポジトリは zumen の作り直しです。設計は [#1](https://github.com/zumen-jp/zumen/issues/1) とその子 issue に、決まったことは `docs/decisions/` にあります。旧版は [zumen-jp/zumen-legacy](https://github.com/zumen-jp/zumen-legacy) にあります。形式の知見（`docs/formats/`）、検証の記録、試験ファイルの作り方は旧版から移します。旧版のコードを移すときも、この規則と新しい層の構成（[決定 0008](docs/decisions/0008-model-layers-and-codecs.md)）に合わせて書き直します。

## 最初に確認すること

- 作業を選ぶときは、[#1](https://github.com/zumen-jp/zumen/issues/1) の下の段階の issue を開き、子 issue を上から見る。閉じていない葉の issue のうち、「待ち: 利用者」と「作業中」のラベルがなく、依存で止められていないものを選ぶ。「待ち: 試料」「待ち: 検証環境」の issue では、まず待ちを減らす。手順は [作業の選び方](docs/work.md) にある（[決定 0174](docs/decisions/0174-plan-in-issues.md)・[0254](docs/decisions/0254-parallel-agents-and-cross-review.md)）。利用者が対象を指定したときは、その依頼を優先する。
- 対象 issue の本文・コメント、関連する開いた PR、手元の変更と worktree を確認する。issue が開いているだけで、未着手とは判断しない。
- issue の完了条件と、採用済みの決定・main の実装を照らす。設計済みのことを決め直さず、残っている成果物を特定する。
- 着手時に「対象 issue・今回作るもの・完了を確かめる方法」を短く示す。着手は issue のコメントで名乗り、名乗りが通ったらラベル「作業中」を付ける（下の「並行して進める」）。条件がそろわない場合の扱いと、引き継ぎの手順は [作業の選び方](docs/work.md) に従う。
- マージを確かめてから、issue の次の一手と完了の条件を直す。できたことの経過は PR に書き、issue の本文に積まない。設計だけで完了する issue を閉じるときも、残る実装には追跡先の issue を用意する。

## 並行して進める

複数のエージェントが同時に進める。人を待つのは、人にしかできないことだけにする。全体は [決定 0254](docs/decisions/0254-parallel-agents-and-cross-review.md) にある。

- **人を待つもの。** 次のことだけ。ほかの判断は、選択肢と理由を書いて決める。
  - 権利者への問い合わせ、契約・規約への同意（同意の要る道具を入れることを含む）、費用のかかること
  - 権利の判断（台帳の `rights` を `unknown`・`permission` から `public` に変えること、NC・ND や利用規約の読みで採否が決まるもの）
  - アカウント・署名の鍵・secrets、パッケージの名前の確保と公開（`v*` のタグの push・GitHub の Release・レジストリへの公開）
  - GitHub の設定（ルールセット・ブランチの保護・Actions の権限・secrets と変数・環境・Webhook・アプリ・公開の範囲）の変更
  - workflow の `permissions` を広げる・secrets を使う・権限つきで動く発火の条件（`pull_request_target`・`workflow_run` など）を足す・公開の手順を変える変更
  - このリポジトリの外への書き込みと送信（ほかのリポジトリへの issue・PR・コメント、メール、外部のサービスへのファイルの送信）
  - 利用者の手元にしかないものを用意すること（受け取った図面を `ZUMEN_SAMPLES` に置く、ライセンスの要るソフトを使えるようにする、アプリの利用を許す）
  - この一覧・互いに確かめる規則・この文書の「範囲と権利」「図面と秘密」「守る約束」を緩める変更
- **ラベル。** 人を待つものには「待ち: 利用者」を付ける。「待ち: 試料」「待ち: 検証環境」の issue は、飛ばす前に待ちを減らす（[作業の選び方](docs/work.md)）。
- **着手。** issue のコメントで名乗り（ブランチ・worktree・時刻）、読み直して、いちばん早い名乗りだけが進める。そのあとでラベル「作業中」を付ける。同じ issue か、同じ crate・同じ形式の文書を主に変える開いた PR があれば着手しない。worktree を分ける。
- **互いに確かめる。** PR は、作った側の会話を引き継がない別のエージェントか人が確かめる。依頼には PR の番号と観点だけを渡し、確かめる側は直さない。結果は、確かめた commit と確かめる側の種類を添えて、全文を PR のレビューのコメントに貼る。作った側は指摘のすべてに答え、直したあとの差分も確かめてもらう。CI の通過は、この確認の代わりにしない。
- **マージ。** 最新の `origin/main` を取り込み、生成物を作り直し、CI を通し直してからマージする。

## 日本語を正にする

- README・文書・決定の記録・issue・PR・コミットの説明は、分かりやすく簡潔な日本語で書く。
- 表記と用語は [docs/style.md](docs/style.md) に従う。辞書（`prh.yml`）にない揺れや、新しい言い換えを見つけたら、辞書と用語集に足す（[決定 0021](docs/decisions/0021-notation-and-terms.md)）。
- 識別子は英語にする。関数名・型名・パッケージ名・コマンド・JSON のキー・診断の code などが対象。Conventional Commits の種別とスコープも英語のままにする（例: `feat(dxf): 外部参照を読む`）。
- 第三者のライセンスの本文と、仕様の引用は原文を保つ。図面に記録された名前・文字・単位は翻訳しない。
- コア（Rust の crate）は人向けの文を返さない。日本語・英語の文言はカタログに置き、診断の code から引く。code は一度使ったら別の意味に使わない（[決定 0009](docs/decisions/0009-diagnostics-and-catalog.md)）。

## 範囲と権利

- 公開仕様に従って実装し、コメントに仕様の節を書く。推定した挙動は `docs/formats/` に記録する。
- 公開に第三者の許諾が要る形式（ST-Bridge など）は、このリポジトリに入れない。外部のプラグインとして作る（旧版の決定 0013）。
- 形式を足す前に、`formats/ledger.toml` に仕様の入手先と権利を記録する。権利が `public` でない形式は、コアに入れない（[決定 0017](docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)、`formats/README.md`）。
- 形式・版・向き（読む・書く）を足す issue は、次をそろえて閉じる。外部の実装があれば、照らす試験。決定 0171 の条件に合う公開サンプルがあれば、読む試験（なければ、探した所を形式の文書に書く）。fuzz の対象とテストキットの契約試験。確かめた度合い（`synthetic`・`external`・`application`）の節。ほかの変更は、変えた所の試験を足し、度合いの節を直す（[決定 0254](docs/decisions/0254-parallel-agents-and-cross-review.md)）。
- 照合に使う外部の実装は、Cargo の依存（dev を含む）にせず、ソースもバイナリもリポジトリに入れず、別に入れたプログラムとして呼ぶ（[決定 0011](docs/decisions/0011-bindings-through-ffi.md) の道具の表）。入っていない環境では試験を飛ばす。食い違いは、仕様と出力・診断を比べて調べ、外部の実装のソースは寛容なライセンスのものだけを読む。
- 国土地理院のジオイド・モデルや座標変換のパラメーターを、Git・パッケージ・試験に入れない。利用者が入手して渡す（[決定 0019](docs/decisions/0019-coordinate-systems-and-gsi-data.md)）。
- 実行時の依存とリンクする依存は、寛容なライセンス（MIT・Apache-2.0・BSD・ISC・Zlib など）に限る。GPL・AGPL・LGPL・MPL は入れない。Cargo の依存（build・dev を含む）と、生成したコードを成果物に入れる道具も、寛容なライセンスに限る（[決定 0011](docs/decisions/0011-bindings-through-ffi.md)）。
- 依存を足す前に、ライセンス、WebAssembly で動くこと、決定性の規則（[決定 0007](docs/decisions/0007-single-rust-implementation.md)）を満たすことを確かめ、理由を記録する。
- IFC のスキーマと、そこから生成したコードを同梱しない（旧版の決定 0028）。GPL の実装（LibreDWG など）のソースを読まない。

## 図面と秘密

- 受け取った図面を Git に入れない。リポジトリの外のフォルダーを `ZUMEN_SAMPLES` に指定して開く。`ZUMEN_SAMPLES` が指定されていれば、試験で読んでよい。結果（issue・PR・ログ）に、ファイル名・顧客の名前・図面の中身を書かない。
- 公開サンプルは、再配布を認めるライセンスのものだけを `samples/` に入れ、出所とライセンスを `samples/ledger.toml` に書く。ライセンスがはっきりしなければ入れず、理由を issue に書く。どうしても入れたいときは、権利者への問い合わせとして利用者に頼む（[決定 0171](docs/decisions/0171-public-samples-in-git.md)・[0254](docs/decisions/0254-parallel-agents-and-cross-review.md)）。
- 試験は、規則を確かめる最小のファイルを、試験の中で作る。
- 顧客の名前・図面と、非公開のリポジトリの情報を書かない。

## 守る約束

- **どこでも同じ結果。** 同じ入力から、ネイティブ・wasm・各言語の結合でビット単位で同じ出力を返す。三角関数・指数・対数・`powi`・`hypot` などは std ではなく `libm` を使い、`HashMap` の順やロケールに依存しない。規則の全体は [決定 0007](docs/decisions/0007-single-rust-implementation.md) にある。
- **黙って捨てない。** 描かないもの・形を変えて書いたものは、診断として種類・件数・場所を返す。
- **ID は契約。** 同じバイト列は、どの版でも同じ ID を返す。ID を変える変更には、先に決定の記録が要る。
- **受け取ったファイルを信用しない。** パニックしない。再帰とループには上限を付ける。繰り返しは回数で数え、浮動小数の足し算で終わりを決めない（#486）。予算は、明示的な文脈で渡し、時間ではなく決まった作業量で数える（[決定 0010](docs/decisions/0010-budget-by-work.md)）。
- **境界は足すだけ。** 各言語へは、数個の関数と、スキーマで定めたメッセージだけで届ける（[決定 0012](docs/decisions/0012-boundary-functions-and-messages.md)）。関数・op・欄・列挙の値は足すだけにし、消さず、名前と意味を変えない（[決定 0015](docs/decisions/0015-add-only-boundary-and-version-floors.md)）。ただし 1.0 より前は、整理のために壊してよい。壊す判断は決定の記録にし、PR のタイトルの種別に `!` を付けて（`feat(abi)!: …`）、本文に移り方を書く。ID の規則は、この例外に入らない。形式や用途ごとの op を足す前に、領域の op（`drawing`・`building`・`civil`・`geo`・`pointcloud` など）で表せないかを確かめる（[決定 0254](docs/decisions/0254-parallel-agents-and-cross-review.md)）。

## 開発の手順

- 最新の `origin/main` からブランチを作り、すべての変更を PR にする。`main` へ直接 push しない。
- PR を出す前に、CI と同じ検査を手元で通す: `cargo fmt --all --check`・`cargo clippy --workspace --all-targets -- -D warnings`・`cargo test --workspace`・`cargo xtask codegen --check`。
  - 境界の crate を変えたときは、wasm32 の検査も通す: `cargo clippy -p zumen-abi --target wasm32-unknown-unknown -- -D warnings` と、wasm32 の適合試験（`bindings/conformance/README.md`、Node.js 22 以降）。
  - 文書・コメント・カタログ・スキーマの説明を変えたときは、表記の検査も通す: `npm ci` と `npm run lint:text`（Node.js 22 以降）。
- 互いに確かめ（上の「並行して進める」）、最新の `origin/main` の上で CI が通ったら squash merge し、ブランチを消す。利用者にレビューで止めるよう頼まれた場合は、そこで止める。
- Rust の版は `rust-toolchain.toml` で固定する。上げるときは、CI の cargo-deny の版も合わせる。
- 診断の code を足すときは、`catalog/` の台帳と日本語・英語の文を直し、`cargo xtask codegen` で生成したファイルも一緒にコミットする（`catalog/README.md`）。
- 境界の関数・op・欄を変えるときは、`schema/` を直し、`cargo xtask codegen` で生成したファイルも一緒にコミットする（`schema/README.md`）。メッセージの JSON は `zumen-json` で読み書きし、規則は [決定 0016](docs/decisions/0016-boundary-message-details.md) にある。
- 応答が変わったら、適合試験のハッシュの一覧を `cargo run -p zumen-conformance -- --write` で作り直し、変わった理由を PR に書く（`bindings/conformance/README.md`）。
- コミットと PR のタイトルは Conventional Commits にする。
- 選択肢と理由のある設計判断は、`docs/decisions/README.md` の形で記録する。
  - 記録にするのは、境界・ID・層とモデル・依存・権利・形式をまたぐ振る舞い・作業の進め方に関わる判断に限る。形式ごとの欄や符号化の細部は、決定にせず `docs/formats/` に書く（[決定 0254](docs/decisions/0254-parallel-agents-and-cross-review.md)）。
  - 番号は 0001 から振る。番号を振る前に、開いている PR が同じ番号を使っていないか確かめる。
  - 旧版の記録は「旧版の決定 0013」と書いて、リンクする。
- issue で旧版の issue を指すときは、`zumen-jp/zumen-legacy#番号` と書く。
- 開発用のポートは 24100〜24199 を使う。
- 端末に固有のファイル（`.claude/launch.json` など）は、`.git/info/exclude` で Git から外す。
