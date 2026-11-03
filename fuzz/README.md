# fuzz

受け取ったバイト列を読む所を、cargo-fuzz で試します（#398・#501、[決定 0010](../docs/decisions/0010-budget-by-work.md)・[決定 0017](../docs/decisions/0017-format-rights-ledger-and-plugin-sdk.md)・[決定 0258](../docs/decisions/0258-fuzz-seeds-and-runs.md)）。登録したすべての形式と、L0 の ZIP・XML・ISO 10303-21 に target があります。どの target も、テストキットの fuzz の関数（`survives`・`survives_lift`・`survives_with`）を呼びます。小さい予算の中で止まり、2 回で同じ結果（読めたか・止まった理由・診断・使った量）になることを確かめます。パニック・止まらない繰り返し・メモリの使い過ぎは、libFuzzer が見つけます。

## target

| target | 試すこと |
| --- | --- |
| `zip` | ZIP を読み、項目を展開し、`.zip` の項目は書庫として辿る。展開した量（ZIP 爆弾）と入れ子が予算で止まり、読めた名前が展開先の外を指さない |
| `xml` | XML を読み、文字と属性の値を戻す。入れ子が予算で止まり、文書型宣言の実体を戻さない |
| `step` | ISO 10303-21 を、フィーチャのコメントを読まない書き方と読む書き方で読み、文字列を戻す。入れ子が予算で止まる |
| `dxf`・`dwg`・`sxf`・`jww` | 読んで図面モデルに移す |
| `saf`・`drawing` | SXF の属性ファイルと、図面モデルのバイナリを読む |
| `ifc` | IFC-SPF か ifcZIP を読み、建物モデルと土木の領域モデルに移す |
| `express`・`ids`・`bcf` | EXPRESS のスキーマ・IDS・BCF を読む |
| `landxml` | 読んで土木の領域モデルに移す |
| `citygml`・`fgd`・`extdm` | 読んで地理の領域モデルに移す |
| `isg`・`par` | ジオイド高の格子（ISG）と、座標の補正量の格子（TKY2JGD・PatchJGD の形）を読む |
| `las`・`e57`・`ply`・`xyz` | 読んで点群の領域モデルに移す。LAZ は、laz の中のパニックを直すまで飛ばす（#510） |
| `delivery` | 電子納品の成果品の書庫を読み、照合する |
| `jpeg`・`boring`・`xlsx`・`csv` | 工事写真・ボーリング交換用データ・Excel の表・CSV を読む |

フォント・プロファイル・レビューの文書・要求の JSON と、領域モデルから図や応答を作る所には、まだ target がありません。#517 で足します。

## 名前の付け方

target の名前は、`xtask/src/fuzz.rs` の検査が求める名前にします。crate が `[package.metadata.zumen]` の `formats` で名乗る形式、`schema/boundary.toml` の `[[detection]]` と `[[modules]]` の形式、L0 の `zip`・`xml`・`step` です。どれかに target がないと、`cargo xtask codegen --check` が落ちます。形式を足したら、`fuzz/fuzz_targets/<名前>.rs` を作り、`fuzz/Cargo.toml` の `[[bin]]` に足してください。

検査が求めない読み取りにも、target を置けます。名前は、読むものの短い英語にします（EXPRESS のスキーマの `express`、補正量の格子の `par`）。

## 動かし方

cargo-fuzz は nightly の Rust を使います。Linux か macOS で動かします。Windows の MSVC では、libFuzzer が使う計測の記号がリンクで見つからず、作れません。CI と同じ版は、`.github/workflows/fuzz.yml` の `NIGHTLY` と `CARGO_FUZZ_VERSION` から読みます。

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

- `cargo xtask fuzz-seeds` は、どの target にも `fuzz/corpus/<target>/` に種を書きます。種にするのは、次の 4 つです（決定 0258）。受け取った図面と、国土地理院のデータは置きません（AGENTS.md、決定 0019）。
  - 公開サンプル（`samples/`）
  - 適合試験の組の合成の入力
  - L0 の攻撃の形（ZIP 爆弾・膨らむ deflate・書庫の入れ子・パスの走査・XML の外部実体と深い入れ子・ISO 10303-21 の深い入れ子）
  - 合成の格子（ISG・TKY2JGD・PatchJGD の形）
- `cp Cargo.lock fuzz/Cargo.lock` で、fuzz の crate の依存を、ワークスペースと同じ版から解きます。`fuzz/Cargo.lock` はリポジトリに入れません。
- `cargo "+$NIGHTLY" fuzz list` で、target の一覧を出せます。
- 見つけた入力は `fuzz/artifacts/` に出ます。直したら、その入力を最小にして、合成の試験に移します。`corpus` と `artifacts` はリポジトリに入れません。
- 見つけた入力は種か種を変えたもので、表示を求めるライセンス（CC BY・MIT・公共データ利用規約）の公開サンプルから来ることがあります。issue などに貼るときは、最小にした入力にし、元の公開サンプルの出所とライセンス（`samples/ledger.toml`）を添えます。元が分からなければ、種を作ったコミットに固定した URL（`https://github.com/zumen-jp/zumen/blob/<コミット>/samples/ledger.toml`）で、台帳の全体を指します。サンプルを外した後も、指す先から表示が消えないようにするためです（決定 0258）。種を作ったコミットは、artifact の `NOTICE.txt` と、log の checkout に出ます。
  - 毎晩の実行なら、その夜の main のコミットです。
  - PR の実行なら、PR と main を合わせた一時のコミットです。このコミットは、後でどの ref からも指されなくなり、残るとは限らないので、件名の「Merge <PR の head> into <main のコミット>」の 2 つのコミットの台帳を、並べて指します。

## CI

`.github/workflows/fuzz.yml` が、毎晩すべての target を 1 つ 180 秒ずつ、4 つずつ並べて流します（決定 0258）。

- 前の夜までに育てた種を actions/cache で引き継ぎます。キャッシュのキーに公開サンプルの台帳（`samples/ledger.toml`）のハッシュを入れ、台帳が変わったら引き継ぎません。
- 残す前に、target ごとに、通る辺を変えずに種を縮めます（libFuzzer の `-merge=1`）。libFuzzer は起動からの時間で止めますが、起動時に種をすべて流し終えるまでは止まりません。縮めないと、種が増えるほど新しい入力を試す時間が減り、種を流すだけで決まった時間を超える target も出ます。縮める前と後の種の数も、summary と log に書きます。流す手順で通らなかった target は、見つけた入力の元を残すため、縮めません。
- target ごとの結果の表を、実行の summary と log に書きます。表には、試した入力・通った辺・もっとも遅い入力（1 秒から）・使ったメモリ・流した後の種の数を出します。
- パニック、30 秒を超える入力、3 GB を超えるメモリを見つけたら失敗します。種を縮めるときに見つけたときと、1 つの target を縮めるのに 20 分を超えたときも、縮める前の種を残して失敗します。
- 見つけた入力と、1 秒以上かかった入力のうち target ごとにそれまでの最長を超えたものを、artifact の `fuzz-artifacts` に 14 日残します。公開サンプルの出所とライセンス（`samples/README.md`・`samples/ledger.toml`・`licenses/`）と、種を作ったコミット（`NOTICE.txt`）を添えます。
- 公開サンプルを台帳から外すと、台帳のハッシュが変わるので、外したサンプルとそれを変えた入力を含む前の種は戻りません。前の種のキャッシュを早く消す手順は、[`samples/README.md`](../samples/README.md) の「外し方」にあります。
- crate・種の元（`samples/`・`bindings/conformance/cases/`）・fuzz の target・この workflow を変えた PR では、1 つ 20 秒ずつ流します。fuzz の crate はワークスペースの外にあるので、ここで組み立てを確かめます。台帳が同じなら main の育てた種も戻すので、20 秒のうち新しい入力を試す時間が減ります。種を流すのに 20 秒より長くかかる target は、その分長くかかります。
- 落ちたときの扱いは、下の「落ちたとき」にあります（決定 0261）。

## 落ちたとき

fuzz の job は、main の必須の検査ではありません。CI を軽く保つためです（決定 0261）。PR で流れたら、結果も通ってからマージします。見つけた入力のある失敗は、直すか、関係のない失敗と確かめるまで、通ったとしません。流し直しや、新しいコミットの実行が通っても、手元で流して落ちなくても、同じです。直したら、その入力を最小にして合成の試験に移し、必須の検査で確かめます。入力のない一時の失敗（runner が取れないなど）は、流し直します。

### 見つけた入力を流す

見つけた入力は、落ちた実行の artifact `fuzz-artifacts` のうち、落ちた target のフォルダーにある `crash-*`・`timeout-*`・`oom-*`・`leak-*` です。artifact には、通った実行でも、1 秒以上かかった入力（`slow-unit-*`）が入ります。取り出すと、`<置き場所>/<target>/crash-…` のように並びます。PR の実行は、PR のブランチの名前で引けます。

```bash
gh run list --workflow fuzz.yml --branch <PR のブランチ> --limit 5
```

```bash
gh run download <実行の ID> --name fuzz-artifacts --dir <置き場所>
```

入力は、CI と同じ条件（`-timeout=30 -rss_limit_mb=3072`）で流します。上の「動かし方」と同じく、Linux・macOS か WSL で動かします。main を取り出した作業場所で流せば main の target を、PR の head を取り出した作業場所で流せば PR の target を試せます。最小にするときは、`cargo fuzz tmin` を使います。手元で流すのは、調べる・比べる・最小にするためで、落ちなくても、通ったとはしません。

```bash
cargo "+$NIGHTLY" fuzz run -O --debug-assertions <target> <置き場所>/<target>/<入力> -- -timeout=30 -rss_limit_mb=3072
```

```bash
cargo "+$NIGHTLY" fuzz tmin -O --debug-assertions <target> <置き場所>/<target>/<入力> -- -timeout=30 -rss_limit_mb=3072
```

### 関係がないかを確かめる

fuzz の失敗のうち、PR の変更と関係のないもの（main の上でも同じように落ちるもの）では、マージを止めません。関係がないかは、次のように確かめます。

- 見つけた入力があれば、main の target でも同じ誤りで落ちるかを見ます。誤りは、種類と、パニックならその関数と文で比べます。行の番号は、PR の変更でずれるので比べません。落ちれば、関係がありません。落ちなければ、PR の変更で落ちたので、直してからマージします。
- 時間切れは機械の速さで変わるので、同じ機械で、PR の target と main の target の時間を比べます。PR の target が遅くなっていなければ、関係がありません。
- PR で足した種（`cargo xtask fuzz-seeds` が書く種）そのものが落ちるなら、main の target でも落ちても、関係があります。マージすると、その後の実行が、その種で落ち続けるからです。種のファイルの名前は、中身の SHA-256 です。空の `fuzz/corpus` で、PR の head と main のそれぞれで `cargo xtask fuzz-seeds` を流し、見つけた入力の SHA-256 の名前の種が、PR の側にだけあるかを見ます。
- 入力がなければ（組み立てや道具の失敗）、main の実行が、同じ手順で、同じ誤り（落ちた target と誤りの文）で落ちるかを見ます。PR の実行の log に、main の実行の log にない誤りがあれば、関係があります。比べる相手は、まず main の最新の毎晩の実行にし、手で流すのは、それで比べられないときだけにします。手で流した実行と毎晩の実行は同じ組（concurrency）で、後から始まったほうが前のほうを取り消します。走っている実行がないことを確かめ、毎晩の実行（日本時間 4:00 から）の前後を避けて流します。

```bash
sha256sum <置き場所>/<target>/<入力>
```

```bash
gh run list --workflow fuzz.yml --branch main --limit 5
```

```bash
gh run list --workflow fuzz.yml --branch main --status in_progress
```

```bash
gh workflow run fuzz.yml --ref main -f seconds=20
```

macOS では、`sha256sum` の代わりに `shasum -a 256` を使います。

### 関係のない失敗の issue

- マージの前に issue にし、PR の本文にその issue を書きます。同じ失敗の issue があれば、それを使い、実行の ID をコメントに足します。同じ失敗かは、落ちた target と誤りの文で見分けます。
- issue には、落ちた target か手順、誤りの文、実行の ID を書きます。見つけた入力があれば、最小にし、元の公開サンプルの出所とライセンスを添えて貼ります（上の「動かし方」）。artifact は 14 日で消えるので、直すときに再現できるようにするためです。
- issue は、いちばん前の開いた段階の issue の子にし、開いた子 issue のいちばん上に並べます。早めに直すためです。並べた理由は、親の issue のコメントに、決定 0261 を指して書きます（決定 0174）。
- main の組み立てか道具が落ちている間は、PR の fuzz は何も確かめないので、パニックを足す変更も止まりません。早めに直します。
- 版を出したあとに、落ちる入力を公開の issue に貼るかは、#557 で決めます（`SECURITY.md`）。

毎晩の実行で見つけた失敗も、同じく issue にして早めに直します。fuzz は入力を乱数で作るので、ある夜に落ちても、次の夜は通ることがあります。エージェントは、作業を選ぶ前に、main の毎晩の fuzz で落ちた実行を引き、その実行の ID を書いた issue がなければ、issue にします（`docs/work.md` の「選び方」）。毎晩の実行は 1 日 1 回なので、落ちた実行を新しいほうから 14 回分見れば、artifact が残る 14 日の分はすべて入ります。

```bash
gh run list --workflow fuzz.yml --branch main --event schedule --status failure --limit 14
```

```bash
gh issue list --state all --search "<実行の ID>"
```

## ライセンス

libfuzzer-sys は LLVM の libFuzzer を同梱し、そのライセンスは NCSA です。fuzz の道具にだけ使い、配る成果物には入りません。`deny.toml` の許可リストに理由を書いてあります。

## 普段の試験

fuzz の道具なしでも、`crates/zumen-abi/tests/contracts.rs` が、試料と合成の入力を途中で切ったものに `survives` と `survives_lift`（図面モデルまで）を流します。
