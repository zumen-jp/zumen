# 開発エージェント向けの規則

zumen は、日本の建設のデータを読み書きする Rust のコアと、各言語のバインディングです。できることと使い方は [README.md](README.md) に、計画は [#1](https://github.com/zumen-jp/zumen/issues/1) の下の issue に、決まったことは [docs/decisions](docs/decisions/README.md) にあります。旧版は [zumen-jp/zumen-legacy](https://github.com/zumen-jp/zumen-legacy) にあります。旧版から知見やコードを移すときも、この規則と層の構成（[決定 0008](docs/decisions/0008-model-layers-and-codecs.md)）に合わせて書き直します。

## リポジトリの地図

| 場所 | 中身 |
| --- | --- |
| `crates/` | コア。下の表 |
| `bindings/` | 各言語のバインディング、CLI と MCP のサーバー（`cli`）、適合テスト（`conformance`） |
| `schema/` | API の関数（`boundary.toml`）とメッセージ（`messages.json`）、シーンと図面モデルのバイナリの配置 |
| `catalog/` | 診断の code の台帳（`codes.toml`）と、日本語・英語の文 |
| `formats/` | 形式の権利の台帳（`ledger.toml`） |
| `profiles/` | 要領・基準の規則と語彙（プロファイル） |
| `samples/` | 再配布できる公開サンプルと、その台帳（`ledger.toml`） |
| `xtask/` | 生成と検査（`cargo xtask codegen`）、外部の実装と照らす束（`verify/`）、AutoCAD でサンプルを作る手順（`autocad/`） |
| `templates/plugin/` | 外部のプラグインのリポジトリのひな形 |
| `examples/`・`bench/`・`site/` | Tier 3 の言語のサンプル、ベンチマーク、<https://zumen.dev> のサイト |
| `docs/` | 決定の記録（`decisions/`）、形式の知見（`formats/`）、作業の選び方（`work.md`）、表記の基準（`style.md`）、対応する環境（`support.md`） |

crate は、層の構成（[決定 0008](docs/decisions/0008-model-layers-and-codecs.md)）に沿って分けている。

| 層 | crate |
| --- | --- |
| 横断の部品 | `zumen-base`（診断・予算・中断の合図）・`zumen-json`・`zumen-crs`（座標系と標高）・`zumen-geom2d`・`zumen-geom3d` |
| L0（交換の構造） | `zumen-step`（ISO 10303-21）・`zumen-xml`・`zumen-zip` |
| L1（ファイルモデルとコーデック） | `zumen-acad`（DXF・DWG）・`zumen-sxf`・`zumen-jww`・`zumen-ifc` |
| L2（領域モデル） | `zumen-drawing`（図面）・`zumen-building`（建物）・`zumen-civil`（土木）・`zumen-geo`（地理）・`zumen-pointcloud`（点群） |
| 領域モデルに読む形式 | `zumen-landxml`・`zumen-citygml`・`zumen-fgd`・`zumen-extdm`・`zumen-las`・`zumen-ply`・`zumen-xyz`・`zumen-e57` |
| 納品の読み取り | `zumen-delivery`（成果品）・`zumen-photo`（工事写真）・`zumen-sheet`（表）・`zumen-geology`（ボーリング交換用データ） |
| L3（派生）と照合 | `zumen-scene`（2D のシーン）・`zumen-civil-drawing`（土木の図面）・`zumen-profile`（プロファイルの照合）・`zumen-review`（レビューの文書）・`zumen-ids`・`zumen-bcf`・`zumen-asbuilt`（出来形） |
| API と入口 | `zumen-abi`（C ABI と wasm の関数）・`zumen`（Rust の入口）・`zumen-sdk`（形式を書くための部品とプラグインの SDK） |

- 生成したファイル（`messages.rs`・`zumen.h`・各言語のメッセージの型・`docs/formats/README.md` など）は手で直さない。元（`schema/`・`catalog/`・`formats/`）を直し、`cargo xtask codegen` で作り直す。
- 決定の記録は 250 を超える。関わる決定は、[一覧](docs/decisions/README.md)と、op・形式・crate の名前で `rg` して探す。

## 最初に確認すること

- 作業を選ぶときは、[#1](https://github.com/zumen-jp/zumen/issues/1) の下の段階の issue を開き、子 issue を上から見る。閉じていない葉の issue のうち、待ちと作業中のラベルがなく、依存で止められていないものを選ぶ。手順は [作業の選び方](docs/work.md) にある（[決定 0174](docs/decisions/0174-plan-in-issues.md)）。利用者が対象を指定したときは、その依頼を優先する。
- 対象 issue の本文・コメント、関連する開いた PR、手元の変更と worktree を確認する。issue が開いているだけで、未着手とは判断しない。
- issue の完了条件と、採用済みの決定・main の実装を照らす。設計済みのことを決め直さず、残っている成果物を特定する。
- 着手時に「対象 issue・今回作るもの・完了を確かめる方法」を短く示す。着手したら、issue にラベル「作業中」を付ける。条件がそろわない場合の扱いと、引き継ぎの手順は [作業の選び方](docs/work.md) に従う。
- マージを確かめてから、issue の次の一手と完了の条件を直す。できたことの経過は PR に書き、issue の本文に積まない。設計だけで完了する issue を閉じるときも、残る実装には追跡先の issue を用意する。

## 日本語を正とする

- README・文書・決定の記録・issue・PR・コミットの説明は、分かりやすく簡潔な日本語で書く。
- 表記と用語は [docs/style.md](docs/style.md) に従う。語は和語に言い換えず、要領・基準・製品の語と、開発で定着した語で書く。辞書（`prh.yml`）にない揺れを見つけたら、辞書と用語集に足す（[決定 0021](docs/decisions/0021-notation-and-terms.md)・[決定 0257](docs/decisions/0257-established-terms-over-coined-words.md)）。
- 識別子は英語にする。関数名・型名・パッケージ名・コマンド・JSON のキー・診断の code などが対象。Conventional Commits の種別とスコープも英語のままにする（例: `feat(dxf): 外部参照を読む`）。
- 第三者のライセンスの本文と、仕様の引用は原文を保つ。図面に記録された名前・文字・単位は翻訳しない。
- コア（Rust の crate）は人向けの文を返さない。日本語・英語の文言はカタログに置き、診断の code から引く。code は一度使ったら別の意味に使わない（[決定 0009](docs/decisions/0009-diagnostics-and-catalog.md)）。

## 範囲と権利

- 公開仕様に従って実装し、コメントに仕様の節を書く。推定した挙動は `docs/formats/` に記録する。
- 公開に第三者の許諾が要る形式（ST-Bridge など）は、このリポジトリに入れない。外部のプラグインとして作る（旧版の決定 0013）。
- 形式を足す前に、`formats/ledger.toml` に仕様の入手先と権利を記録する。権利が `public` でない形式は、コアに入れない（[決定 0017](docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)、`formats/README.md`）。
- 国土地理院のジオイド・モデルや座標変換のパラメーターを、Git・パッケージ・テストに入れない。利用者が入手して渡す（[決定 0019](docs/decisions/0019-coordinate-systems-and-gsi-data.md)）。
- 実行時の依存とリンクする依存は、寛容なライセンス（MIT・Apache-2.0・BSD・ISC・Zlib など）に限る。GPL・AGPL・LGPL・MPL は入れない。Cargo の依存（build・dev を含む）と、生成したコードを成果物に入れるツールも、寛容なライセンスに限る（[決定 0011](docs/decisions/0011-bindings-through-ffi.md)）。
- 依存を足す前に、ライセンス、WebAssembly で動くこと、決定性の規則（[決定 0007](docs/decisions/0007-single-rust-implementation.md)）を満たすことを確かめ、理由を記録する。
- IFC のスキーマと、そこから生成したコードを同梱しない（旧版の決定 0028）。GPL の実装（LibreDWG など）のソースを読まない。

## 図面と秘密

- 受け取った図面を Git に入れない。リポジトリの外のフォルダーを `ZUMEN_SAMPLES` に指定して開く。
- 公開サンプルは、再配布を認めるライセンスのものだけを `samples/` に入れ、出所とライセンスを `samples/ledger.toml` に書く。ライセンスがはっきりしなければ入れずに利用者に尋ねる（[決定 0171](docs/decisions/0171-public-samples-in-git.md)）。
- テストは、規則を確かめる最小のファイルを、テストの中で作る。
- 顧客の名前・図面と、非公開のリポジトリの情報を書かない。

## 守る約束

- **どこでも同じ結果。** 同じ入力から、ネイティブ・wasm・各言語のバインディングでビット単位で同じ出力を返す。三角関数・指数・対数・`powi`・`hypot` などは std ではなく `libm` を使い、`HashMap` の順やロケールに依存しない。規則の全体は [決定 0007](docs/decisions/0007-single-rust-implementation.md) にある。
- **黙って捨てない。** 描かないもの・形を変えて書いたものは、診断として種類・件数・場所を返す。
- **ID は契約。** 同じバイト列は、どのバージョンでも同じ ID を返す。ID を変える変更には、先に決定の記録が要る。
- **受け取ったファイルを信用しない。** パニックしない。再帰とループには上限を付ける。予算は、明示的な文脈で渡し、時間ではなく決まった作業量で数える（[決定 0010](docs/decisions/0010-budget-by-work.md)）。
- **API は足すだけ。** 各言語へは、数個の関数と、スキーマで定めたメッセージだけで届ける（[決定 0012](docs/decisions/0012-boundary-functions-and-messages.md)）。関数・op・フィールド・列挙の値は足すだけにし、消さず、名前と意味を変えない（[決定 0015](docs/decisions/0015-add-only-boundary-and-version-floors.md)）。

## 開発の手順

- 最新の `origin/main` からブランチを作り、すべての変更を PR にする。`main` へ直接 push しない。
- PR を出す前に、CI と同じ検査を手元で通す: `cargo fmt --all --check`・`cargo clippy --workspace --all-targets -- -D warnings`・`cargo test --workspace`・`cargo xtask codegen --check`。
  - API の crate を変えたときは、wasm32 の検査も通す: `cargo clippy -p zumen-abi --target wasm32-unknown-unknown -- -D warnings` と、wasm32 の適合テスト（`bindings/conformance/README.md`、Node.js 22 以降）。
  - 文書・コメント・カタログ・スキーマの説明を変えたときは、表記の検査も通す: `npm ci` と `npm run lint:text`（Node.js 22 以降）。
- CI が通ったら squash merge し、ブランチを消す。レビューで止めるよう頼まれた場合は、そこで止める。
- Rust のバージョンは `rust-toolchain.toml` で固定する。上げるときは、CI の cargo-deny のバージョンも合わせる。
- 診断の code を足すときは、`catalog/` の台帳と日本語・英語の文を直し、`cargo xtask codegen` で生成したファイルも一緒にコミットする（`catalog/README.md`）。
- API の関数・op・フィールドを変えるときは、`schema/` を直し、`cargo xtask codegen` で生成したファイルも一緒にコミットする（`schema/README.md`）。メッセージの JSON は `zumen-json` で読み書きし、規則は [決定 0016](docs/decisions/0016-boundary-message-details.md) にある。
- 応答が変わったら、適合テストのハッシュの一覧を `cargo run -p zumen-conformance -- --write` で作り直し、変わった理由を PR に書く（`bindings/conformance/README.md`）。
- コミットと PR のタイトルは Conventional Commits にする。
- 選択肢と理由のある設計判断は、`docs/decisions/README.md` の形で記録する。
  - 番号は 0001 から振る。番号を振る前に、開いている PR が同じ番号を使っていないか確かめる。
  - 旧版の記録は「旧版の決定 0013」と書いて、リンクする。
- issue で旧版の issue を指すときは、`zumen-jp/zumen-legacy#番号` と書く。
- 開発用のポートは 24100〜24199 を使う。
- 端末に固有のファイル（`.claude/launch.json` など）は、`.git/info/exclude` で Git から外す。
