# fuzz

受け取ったバイト列を読む所を、cargo-fuzz で試します（#398・#501・#517、[決定 0010](../docs/decisions/0010-budget-by-work.md)・[決定 0017](../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)・[決定 0258](../docs/decisions/0258-fuzz-seeds-and-runs.md)・[決定 0262](../docs/decisions/0262-boundary-fuzz-targets.md)）。登録したすべての形式と、L0 の ZIP・XML・ISO 10303-21 に target があります。API（`zumen` の `Session`）を通して、形式の判定から応答を作るまでと、形式でない入力を流す target もあります。どの target も、テストキットの fuzz の関数（`survives`・`survives_lift`・`survives_with`・`survives_calls`）を呼びます。小さい予算の中で止まり、2 回で同じ結果（読めたか・止まった理由・診断・使った量、API では応答のバイト列）になることを確かめます。パニック・止まらない繰り返し・メモリの使い過ぎは、libFuzzer が見つけます。

## target

| target | 試すこと |
| --- | --- |
| `zip` | ZIP を読み、項目を展開し、`.zip` の項目はアーカイブとして辿る。展開した量（ZIP 爆弾）と入れ子が予算で止まり、読めた名前が展開先の外を指さない |
| `xml` | XML を読み、文字と属性の値を戻す。入れ子が予算で止まり、文書型宣言の実体を戻さない |
| `step` | ISO 10303-21 を、フィーチャのコメントを読まない書き方と読む書き方で読み、文字列を戻す。入れ子が予算で止まる |
| `dxf`・`dwg`・`sxf`・`jww`・`pdf` | 読んで図面モデルに移す。`pdf` は、移した図面も 2 回で同じになることを確かめる（#506） |
| `saf`・`drawing` | SXF の属性ファイルと、図面モデルのバイナリを読む |
| `png` | シーンのバイナリを検査し、64×64 の PNG を書く。走査・合成・符号化を予算で止め、2 回の出力も一致することを確かめる |
| `svg` | シーンのバイナリを検査して SVG を書く。SVG 自体は読まず、2 回の出力も一致することを確かめる |
| `ifc` | IFC-SPF か ifcZIP を読み、建物モデルと土木の領域モデルに移す |
| `express`・`ids`・`bcf` | EXPRESS のスキーマ・IDS・BCF を読む |
| `landxml` | 読んで土木の領域モデルに移す |
| `citygml`・`fgd`・`extdm` | 読んで地理の領域モデルに移す |
| `isg`・`par` | ジオイド高の格子（ISG）と、座標の補正量の格子（TKY2JGD・PatchJGD の形）を読む |
| `las`・`e57`・`ply`・`xyz` | 読んで点群の領域モデルに移す。LAZ は、laz の中のパニックを直すまで飛ばす（#510） |
| `delivery` | 電子納品の成果品のアーカイブを読み、照合する |
| `jpeg`・`boring`・`xlsx`・`csv` | 工事写真・ボーリング交換用データ・Excel の表・CSV を読む |
| `open` | API の `open` で形式を判定して開き、図面・建物・土木・地理・点群の op で読む。図面はシーンにし、図面モデルのバイナリと DXF に書いて読み直す。線形からは図（`civil_drawing`）を作ってシーンにする。LAS は範囲でも読む。開けない入力は、名指せる XYZ・拡張 DM と、固定の SXF の図面の属性ファイル（SAF、`open` の `saf`）と、写真・表・ボーリング交換用データ・成果品の op に渡す。P2Z・SFZ の中の SAF は、アーカイブの項目として結ぶ。LAZ は、`las` と同じく飛ばす（#510） |
| `font` | シーンの文字のフォント（TrueType・OpenType）として渡し、固定の図面をシーンにする |
| `profile` | プロファイルとして渡し、固定の図面の照合（`check`）・線形の図（`civil_drawing`）・点群と設計面の差（`deviation`）を呼ぶ。線形の図はシーンにする |
| `review` | レビューの文書として渡し、固定の図面の版へ移し（`review_carry`）、照合の指摘を取り込む（`review_adopt`） |
| `json` | 色の表として渡して固定の図面をシーンにし、要求として呼ぶ。配列なら、先頭の 16 の要素を順に要求として呼ぶ |

API を通す 5 つの target（`open`・`font`・`profile`・`review`・`json`）は、`fuzz/src/lib.rs` の部品で、`zumen` の `Session` を呼びます（決定 0262）。

- どの要求の予算も、テストキットの API の呼び出しの fuzz の上限（`fuzz_call_limits`）にします。`fuzz_limits` より、要素・幾何の分割・ほかの繰り返しの上限が小さく、1 つの入力で多くの op を呼んでも、ASan と計装を入れた build で時間切れにならないようにします。`json` の target が受け取った要求も、`budget` を替え、時計で止める `time_limit_ms` を除きます。
- 2 回呼んで、応答の JSON と、取り出したバイナリの結果のバイト列を呼んだ順に並べたものが、同じになることを確かめます（`survives_calls`）。
- 応答ごとに、JSON として読め、成功したときだけ `result` を持ち、失敗した応答は理由の診断を持つことを確かめます。`blobs` の結果は、どれも取り出せ、記録した大きさと同じで、シーンの blob は `zumen` のリーダーで読めることを確かめます。
- 文書の中身に依らない入力（図面・照合のプロファイル・色の表・線形・設計面・点群・SXF の図面）は、適合テストの組の合成の入力を写した、`fuzz/src/lib.rs` の定数にします。図面（DXF）だけは、写したものに図形と文字を足しました。

## 名前の付け方

target の名前は、`xtask/src/fuzz.rs` の検査が求める名前にします。crate が `[package.metadata.zumen]` の `formats` で名乗る形式、`schema/boundary.toml` の `[[detection]]` と `[[modules]]` の形式、L0 の `zip`・`xml`・`step` です。どれかに target がないと、`cargo xtask codegen --check` が落ちます。形式を足したら、`fuzz/fuzz_targets/<名前>.rs` を作り、`fuzz/Cargo.toml` の `[[bin]]` に足してください。

検査が求めない読み取りにも、target を置けます。名前は、読むものの短い英語にします（EXPRESS のスキーマの `express`、補正量の格子の `par`、API を通す `open`・`font`・`profile`・`review`・`json`）。

## 動かし方

cargo-fuzz は nightly の Rust を使います。Linux か macOS で動かします。Windows の MSVC では、libFuzzer が使う計測の記号がリンクで見つからず、作れません。CI と同じバージョンは、`.github/workflows/fuzz.yml` の `NIGHTLY` と `CARGO_FUZZ_VERSION` から読みます。

```bash
NIGHTLY=$(sed -n 's/^  NIGHTLY: //p' .github/workflows/fuzz.yml)
CARGO_FUZZ_VERSION=$(sed -n 's/^  CARGO_FUZZ_VERSION: //p' .github/workflows/fuzz.yml)
```

```bash
rustup toolchain install "$NIGHTLY" --profile minimal
```

```bash
cargo install cargo-fuzz --version "$CARGO_FUZZ_VERSION" --locked
```

```bash
cargo xtask fuzz-seeds
```

```bash
cp Cargo.lock fuzz/Cargo.lock
```

```bash
cargo "+$NIGHTLY" fuzz run -O --debug-assertions zip
```

- `cargo xtask fuzz-seeds` は、どの target にも `fuzz/corpus/<target>/` に種を書きます。種にするのは、次の 5 つです（決定 0258）。受け取った図面と、国土地理院のデータは置きません（AGENTS.md、決定 0019）。
  - 公開サンプル（`samples/`）
  - 適合テストの組の合成の入力と、`call` で渡す要求（1 つずつと、組ごとに並べた配列。決定 0262）。`call_text`・`call_hex` で渡す要求のバイト列（読めない要求と、BOM を付けた要求）も、1 つずつ種にします
  - L0 の攻撃の形（ZIP 爆弾・膨らむ deflate・アーカイブの入れ子・パスの走査・XML の外部実体と深い入れ子・ISO 10303-21 の深い入れ子）
  - 合成の格子（ISG・TKY2JGD・PatchJGD の形）
  - 線と列の参照を持つ合成シーン（ZMSC、SVG を書く target 用）
- `cp Cargo.lock fuzz/Cargo.lock` で、fuzz の crate の依存を、ワークスペースと同じバージョンから解きます。`fuzz/Cargo.lock` はリポジトリに入れません。
- `cargo "+$NIGHTLY" fuzz list` で、target の一覧を出せます。
- 見つけた入力は `fuzz/artifacts/` に出ます。直したら、その入力を最小にして、合成のテストに移します。`corpus` と `artifacts` はリポジトリに入れません。
- 見つけた入力は種か種を変えたもので、表示を求めるライセンス（CC BY・MIT・公共データ利用規約）の公開サンプルから来ることがあります。入力を報告に添えるときは、最小にした入力にし、元の公開サンプルの出所とライセンス（`samples/ledger.toml`）を添えます。元が分からなければ、種を作ったコミットに固定した URL（`https://github.com/zumen-jp/zumen/blob/<コミット>/samples/ledger.toml`）で、台帳の全体を指します。サンプルを外した後も、指す先から表示が消えないようにするためです（決定 0258）。種を作ったコミットは、artifact の `NOTICE.txt` と、log の checkout に出ます。
  - 毎晩の実行なら、その夜の main のコミットです。
  - PR の実行なら、PR と main を合わせた一時のコミットです。このコミットは、後でどの ref からも指されなくなり、残るとは限らないので、件名の「Merge <PR の head> into <main のコミット>」の 2 つのコミットの台帳を、並べて指します。

## CI

以下は初回リリース前の公開 CI の動作です。初回リリース以降は、入力と詳細が公開のログ・summary・artifact・キャッシュに出ないようにします。CI 側の切り替えと確認は [#647](https://github.com/zumen-jp/zumen/issues/647) で追います。

`.github/workflows/fuzz.yml` が、毎晩すべての target を 1 つ 180 秒ずつ、4 つずつ並べて流します（決定 0258）。

- 前の夜までに育てた種を actions/cache で引き継ぎます。キャッシュのキーに公開サンプルの台帳（`samples/ledger.toml`）のハッシュを入れ、台帳が変わったら引き継ぎません。
- 残す前に、target ごとに、通る辺を変えずに種を縮めます（libFuzzer の `-merge=1`）。libFuzzer は起動からの時間で止めますが、起動時に種をすべて流し終えるまでは止まりません。縮めないと、種が増えるほど新しい入力を試す時間が減り、種を流すだけで決まった時間を超える target も出ます。縮める前と後の種の数も、summary と log に書きます。流す手順で通らなかった target は、見つけた入力の元を残すため、縮めません。
- target ごとの結果の表を、実行の summary と log に書きます。表には、試した入力・通った辺・もっとも遅い入力（1 秒から）・使ったメモリ・流した後の種の数を出します。
- パニック、30 秒を超える入力、3 GB を超えるメモリを見つけたら失敗します。種を縮めるときに見つけたときと、1 つの target を縮めるのに 20 分を超えたときも、縮める前の種を残して失敗します。
- 見つけた入力と、1 秒以上かかった入力のうち target ごとにそれまでの最長を超えたものを、artifact の `fuzz-artifacts` に 14 日残します。公開サンプルの出所とライセンス（`samples/README.md`・`samples/ledger.toml`・`licenses/`）と、種を作ったコミット（`NOTICE.txt`）を添えます。
- 公開サンプルを台帳から外すと、台帳のハッシュが変わるので、外したサンプルとそれを変えた入力を含む前の種は戻りません。前の種のキャッシュを早く消す手順は、[`samples/README.md`](../samples/README.md) の「外し方」にあります。
- crate・種の元（`samples/`・`bindings/conformance/cases/`）・fuzz の target・このワークフローを変えた PR では、1 つ 20 秒ずつ流します。Markdown（`*.md`）だけの変更では流しません（[決定 0263](../docs/decisions/0263-skip-documentation-only-fuzz.md)）。種や実装に使う Markdown を足すときは、パスの除外条件の後ろにそのパスを足し、実行の対象に戻します。
- fuzz の crate はワークスペースの外にあるので、PR の fuzz で組み立ても確かめます。台帳が同じなら main の育てた種も戻すので、20 秒のうち新しい入力を試す時間が減ります。種を流すのに 20 秒より長くかかる target は、その分長くかかります。
- 落ちたときの扱いは、下の「落ちたとき」にあります（決定 0261）。

## 落ちたとき

fuzz の job は、main の必須の検査ではありません。CI を軽く保つためです（決定 0261）。PR で流れたら、結果も通ってからマージします。見つけた入力のある失敗は、直すまで、通ったとしません。流し直しや、新しいコミットの実行が通っても、手元で流して落ちなくても、同じです。直したら、その入力を最小にして合成のテストに移し、必須の検査で確かめます。入力のない一時の失敗（runner が取れないなど）は、流し直します。

通らなかった失敗は、下の「失敗を分ける」で、関係がある・関係がない・確かめられないの 3 つに分けます。PR の変更と関係のない失敗では、マージを止めません。

### 初回リリース後の記録先

入力による不具合を見つけたら、[Security タブの非公開の窓口](https://github.com/zumen-jp/zumen/security/advisories/new) で報告します。担当者は次のように記録します（[決定 0283](../docs/decisions/0283-fuzz-reporting-after-release.md)）。

- 公開の issue に書くのは対象の target と失敗の種類（パニック、時間切れ、メモリの使い過ぎなど）だけです。親 issue や PR からは、その issue を参照します。
- 非公開のアドバイザリに公開 issue の番号と実行の ID を対応させ、入力、出所とライセンス、誤りの文、ログ、比較結果を記録します。最小にできなかった理由や artifact 内の名前もここに書きます。同じ失敗が再び出たときは、非公開の記録で照合し、その記録に新しい実行の ID を足します。
- 直せずに利用者の承認を求める場合は、非公開の記録を利用者と照合し、実行の ID・target・誤りを特定してから、その PR について会話で認めてもらいます。公開の PR には日付・head・対象の公開 issue 番号・詳細を含まない利用者の承認の言葉を残します。非公開の記録にはその承認へのリンクと、認めた実行・誤り・比較結果を残します。別の失敗を含めるときはあらためて認めてもらいます。
- 修正と回帰テストは非公開で確かめます。修正版を公開したあとに、公開できる詳細と合成のテストを出し、公開 issue から修正版と修正内容を案内します。

### 見つけた入力を流す

初回リリース後は、担当者が非公開の記録から入力を取得します。以下は初回リリース前の取得手順です。見つけた入力は、落ちた実行の artifact `fuzz-artifacts` のうち、落ちた target のフォルダーにある `crash-*`・`timeout-*`・`oom-*`・`leak-*` です。artifact には、通った実行でも、1 秒以上かかった入力（`slow-unit-*`）が入ります。取り出すと、`<置き場所>/<target>/crash-…` のように並びます。PR の実行は、PR のブランチの名前で引けます。

```bash
gh run list --workflow fuzz.yml --branch <PR のブランチ> --limit 5
```

```bash
gh run download <実行の ID> --name fuzz-artifacts --dir <置き場所>
```

PR の実行は、PR の head と main を合わせた一時のコミットで流れます。そのコミットは、log の checkout と `NOTICE.txt` に出ます。手元の target は、次のコミットで作ります。

- PR の target: そのコミットか、そのコミットの main を取り込んだ PR の head
- main の target: そのコミットの main か、それより新しい main

入力は、CI と同じ条件（`-timeout=30 -rss_limit_mb=3072`）で流します。上の「動かし方」と同じく、Linux・macOS か WSL で動かします。最小にするときは、`cargo fuzz tmin` を使います。手元で流すのは、調べる・比べる・最小にするためで、落ちなくても、通ったとはしません。

```bash
cargo "+$NIGHTLY" fuzz run -O --debug-assertions <target> <置き場所>/<target>/<入力> -- -timeout=30 -rss_limit_mb=3072
```

```bash
cargo "+$NIGHTLY" fuzz tmin -O --debug-assertions <target> <置き場所>/<target>/<入力> -- -timeout=30 -rss_limit_mb=3072
```

### 失敗を分ける

次の順に見ます。誤りは、種類と、パニックならその関数と文で比べます。行の番号は、PR の変更でずれるので比べません。

1. PR で足した種（`cargo xtask fuzz-seeds` が書く種）そのものが落ちるなら、関係があります。マージすると、その後の実行が、その種で落ち続けるからです。種のファイルの名前は、中身の SHA-256 です。空の `fuzz/corpus` で、PR の側と main の側のそれぞれで `cargo xtask fuzz-seeds` を流し、見つけた入力の SHA-256 の名前の種が、PR の側にだけあるかを見ます。macOS では、`sha256sum` の代わりに `shasum -a 256` を使います。
2. 時間切れとメモリの使い過ぎは、確かめられないとします。時間切れは機械の速さで、メモリの使い過ぎは OS とメモリの割り当て方で変わるので、main の target と比べても決まりません。メモリは、それまでに流した入力で積もった分も数えるので、入力を 1 つだけ流しても再現しないこともあります。
3. ほかの、見つけた入力のある失敗は、その入力を手元で PR の target と main の target に流します。どちらの target でも落ちなければ、確かめられないとします。main の target が、PR の実行と同じ誤りで落ちれば、関係がありません。ほかは、関係があります。
4. 入力のない失敗（組み立てやツールの失敗）は、main の実行と比べます。同じ手順が同じ誤り（落ちた target と誤りの文）で落ちれば、関係がありません。PR の実行の log に、main の実行の log にない誤りがあれば、関係があります。比べる main の実行は、PR の実行が合わせた main のコミットか、それより新しいものにします。main の最新の毎晩の実行がそれより古いときだけ、手で流します。

```bash
sha256sum <置き場所>/<target>/<入力>
```

```bash
gh run list --workflow fuzz.yml --branch main --limit 5
```

main の fuzz を手で流すときと流し直すときは、走っている実行がないことを確かめ、毎晩の実行（日本時間 4:00 から）の前後を避けます。手で流した実行と毎晩の実行は同じ組（concurrency）で、後から始まったほうが、前のほうを取り消すからです。

```bash
gh run list --workflow fuzz.yml --branch main --status in_progress
```

```bash
gh workflow run fuzz.yml --ref main -f seconds=20
```

分けたあとは、次のようにします。

- 関係がある失敗は、直してからマージします。
- 関係のない失敗では、マージを止めません。下の「issue にする」のとおり、issue にします。
- 確かめられない失敗は、直してからマージします。直せないときは、その PR でマージの条件を緩めることになるので、人を待つ PR の手順で扱います（AGENTS.md の「並行して進める」）。PR に「待ち: 利用者」を付けます。初回リリース後の入力と詳細は上の「初回リリース後の記録先」に従い、利用者に PR の番号・head・失敗した実行の ID と、分けた結果（手元でどちらの target が落ちたか）を示して、会話で認めてもらいます。認める範囲は、その失敗（実行の ID・target・誤り）です。認めた日・head・範囲・言葉を上の記録先に合わせて残し、認めた会話のエージェントか利用者がマージします。あとの実行で、範囲の外の確かめられない失敗が出たら、認め直してもらいます。この失敗も、下の「issue にする」のとおり、issue にします。

### issue にする

関係のない失敗と、利用者に認めてもらってマージする失敗は、次のように issue にします。

- マージの前に issue にし、PR の本文にその issue を書きます。同じ失敗の issue があれば、それを使います。落ちた target と誤りの文で同じ失敗かを確かめ、実行の ID を足します。初回リリース後の入力による失敗は、この照合と追記を非公開のアドバイザリで行います。
- 初回リリース後の入力による失敗は、上の「初回リリース後の記録先」に従います。それ以外は、issue に落ちた target か手順、誤りの文、実行の ID を書きます。見つけた入力があれば、最小にし、元の公開サンプルの出所とライセンスを添えて貼ります（上の「動かし方」）。artifact は 14 日で消えるので、直すときに再現できるようにするためです。手元で再現せず、最小にできない入力は、貼らずに、artifact の中の名前と、最小にできなかったわけを書きます。
- issue は、いちばん前の開いた段階の issue の子にし、開いた子 issue のいちばん上に並べます。早めに直すためです。並べた理由は、親の issue のコメントに、決定 0261 を指して書きます（決定 0174）。
- main の組み立てかツールが落ちている間は、PR の fuzz は何も確かめないので、パニックを足す変更も止まりません。早めに直します。

### 毎晩の実行

毎晩の実行で見つけた失敗も、同じく issue にして早めに直します。fuzz は入力を乱数で作るので、ある夜に落ちても、次の夜は通ることがあります。初回リリース後の入力による失敗は、実行の ID と公開 issue の対応を非公開のアドバイザリで調べます。エージェントは、作業を選ぶ前に、main の毎晩の fuzz の終わった実行を引き、結論が `success` でない実行に、その実行に対応する issue がなければ、issue にします（`docs/work.md` の「選び方」）。入力のない一時の失敗（runner が取れない、ほかの実行に取り消された、など）は、流し直して通れば、issue にしません。毎晩の実行は 1 日 1 回なので、新しいほうから 14 回分見れば、artifact が残る 14 日の分はすべて入ります。main で手で流した実行が通らなかったときは、流した側が、同じく issue にします。

```bash
gh run list --workflow fuzz.yml --branch main --event schedule --status completed --limit 14
```

次の検索は、実行の ID を公開している場合に使います。

```bash
gh issue list --state all --search "<実行の ID>"
```

```bash
gh run rerun <実行の ID>
```

## ライセンス

libfuzzer-sys は LLVM の libFuzzer を同梱し、そのライセンスは NCSA です。fuzz のツールにだけ使い、配る成果物には入りません。`deny.toml` の許可リストに理由を書いてあります。

## 普段のテスト

fuzz のツールなしでも、`crates/zumen-abi/tests/contracts.rs` が、サンプルと合成の入力を途中で切ったものに `survives` と `survives_lift`（図面モデルまで）を流します。
