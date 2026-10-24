# 0010: 予算を決まった作業量で数え、どの環境でも同じ所で止める

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-24 |
| 状態 | 採用 |

## 背景

- 旧版の上限（`zumen_core::limits`、229 行）は、バイト数・図形数・時間を数えた（[旧版の構成](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/architecture.en.md#limits-for-untrusted-drawings)）。
  - 上限を thread_local に持ち、`admit` と `tick` で数え、`tick` の 256 回ごとに時計を見た。並列・入れ子・非同期と相性が悪い。
  - 上限に達すると、途中までのシーンではなく、上限の名前を付けた失敗を返した。
  - WebAssembly には時計がないので、時間の上限を Rust に渡さず、`ReaderWorker` が Worker ごと止めた。
  - P2Z・SFZ は、上限のバイト数（なければ 2 GiB）まで展開した。
- 時間で止めると、止まる所が機械の速さで変わる。[0007](0007-single-rust-implementation.md) は、どの環境でも同じ所で止まることを求める。
- wasm32 では、`catch_unwind` でパニックを捕まえられない。Rust 1.99 の wasm32-unknown-unknown で試すと、パニックは `unreachable` の trap になった（2026-10-24）。
- [0008](0008-model-layers-and-codecs.md) で、予算・診断・中断の合図を、文脈 `Cx` で明示的に渡すと決めた。

## 選択肢

1. **時間で止める**（旧版）。設定は分かりやすいが、止まる所が環境で変わり、wasm では使えない。
2. **ホストの手段だけで止める**（プロセス・Worker・wasm の fuel）。核に手を入れずに済むが、止まる所が決まらず、途中までの結果も診断も返せない。
3. **決まった作業量で数える。** 作業の単位を決め、コードの決まった所で数える。どの環境でも同じ所で止まる。時計とホストの手段は補助にする。

## 決定

3 を選ぶ。

### 作業量の単位

| 種類 | 数えるもの | 例 |
| --- | --- | --- |
| `input-bytes` | 受け取ったバイト数 | 読み始める前に確かめる |
| `decoded-bytes` | 展開・復号で作ったバイト数。入れ子の分も足す | ZIP、DWG の圧縮、PDF のストリーム、LAZ |
| `records` | L0・L1 で読んだレコードの数 | DXF のエンティティ、SXF のインスタンス、JWW のレコード、XML の要素 |
| `elements` | L2・L3 で作った要素の数 | 図形、建物の要素、表示用の要素 |
| `depth` | 入れ子と再帰の深さ | ブロックの中のブロック、XML の深さ、アーカイブの中のアーカイブ |
| `segments` | 幾何の分割で作った点・線・面の数 | 曲線の分割、ハッチの線、メッシュの面 |
| `work` | 上のどれにも入らない繰り返しの回数 | 探索、解読のループ |

- 種類ごとに、有限の既定値を持つ。既定値はデータにし、実図面で測って決め、文書に残す。
- 呼び出し側は、種類ごとに上限を変えられる。上限をなくすときは、明示的に指定する。
- 予算は、呼び出しごとに 1 つ持つ。入れ子の読み取り（アーカイブの中のファイル、外部参照）も、同じ予算から使う。
- 数える所は、コードの中で決めておく。どの環境でも同じ所で止まる。
- 並列にしても、順に処理したときと同じ所で止まる。仕事ごとに使った量を入力の順に足し、超えた所で切る。
- ファイルに書かれた個数を信用して、先にメモリを確保しない。実際に読んだ量に合わせて増やし、残りの入力と予算で抑える。

### 上限に達したとき

- その所で止め、`budget.<種類>.exceeded` の診断（[0009](0009-diagnostics-and-catalog.md)）を返す。値には、種類・上限・止まった場所を入れる。
- 読み取り（L1・L2・L3 を作る処理）は、既定では失敗として返す。呼び出し側が求めたときだけ、止まった所までの結果を返す。
  - 結果には、欠けていることの印と、上の診断を付ける。
  - 止まる所が決まっているので、この結果も環境によらない。
- 書き出し（`apply`・`lower`・コーデックの書き出し）は、途中までの結果を返さず、何も書かない（[0008](0008-model-layers-and-codecs.md)）。

### 時計と中断

- 時間の上限は、ネイティブでの補助にする。
  - 作業量を数えるのと同じ所で時計を見て、超えたら `budget.time.exceeded` を返す。
  - 止まる所は環境で変わるので、[0007](0007-single-rust-implementation.md) の約束の外になる。
- 中断の合図（`CancelToken`）は `Cx` で渡し、同じ所で確かめる。中断したら `call.request.cancelled` を返す。
- wasm では、モジュールの中に時計がなく、呼び出しの途中に合図を書き込めない。ホストの手段で止める。
  - ブラウザー: Worker を止める
  - wasm のランタイム: wasmtime の fuel や epoch、wazero の context など
- wasm のインスタンスを止めたときと、trap したときは、ホストがインスタンスを捨てて作り直す。メモリの中の状態が壊れているかもしれないため。
- 依存の中の処理で、呼び出しの間に数えられないもの（PDF のストリームの解読など）は、入力の大きさで先に制限する。fuzz で見つけた問題は、上流に報告する（[旧版の決定 0020](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0020-pdf-drawings-in-rust.md)）。

### パニックしない

- 核の crate では、試験を除き、clippy で次を禁じる。どうしても要る所は、理由をコメントに書いて許す。
  - `unwrap_used`・`expect_used`・`panic`・`todo`・`unimplemented`・`unreachable`
  - `indexing_slicing`・`string_slice`
- ネイティブの境界では、`catch_unwind` で止め、`internal.panic.caught` の診断を返す。パニックは不具合なので、直す。
- wasm では `catch_unwind` が効かない。パニックしないことを lint と fuzz で守り、trap したらホストがインスタンスを作り直す。
- スタックのあふれは、どの環境でも捕まえられない。入力で深さが決まる処理は、`depth` で制限するか、明示的なスタックを使うループにする。

### fuzz

- L0 の部品、コーデック、lift のそれぞれに、fuzz の対象を作る（cargo-fuzz）。
- PR ごとに短く、定期的に長く回す。
- 種には、テストキットで作る合成のファイルだけを使う。実図面は Git に入れない。
- 見つけた入力は小さくして、試験に足す。
- 確かめること: パニックしない、予算の中で止まる、2 回読んで同じ結果になる。

### 入れ物と XML

- **ZIP**
  - 展開したバイト数・項目の数・入れ子の深さを、予算で数える。
  - 中身の範囲が重なる項目（重ねて作る ZIP 爆弾）は、読まずに診断にする。
  - 項目の名前は、そのままパスとして使わない。`..`・絶対パス・ドライブ名・NUL を含む名前は、`zip.path.unsafe` の診断にする。核はファイルシステムに書かない（[0007](0007-single-rust-implementation.md)）。
- **XML**
  - DTD を処理しない。外部の実体と外部の DTD は解決しない。
  - 内部の実体の展開は、予算で数えるか、読まずに診断にする。

## 影響

- コーデックを書くときは、入力で回数が決まるループごとに、どの単位で数えるかを決める。レビューで確かめる。
- 既定値を決めるには、実図面の目録（[0004](0004-repositories.md) の案の `corpus`）での計測が要る。
- 旧版の `zumen_core::limits` は移さない。
- 予算の設定を受け渡す形は、[#13](https://github.com/zumen-jp/zumen/issues/13) で決める。
- [#8](https://github.com/zumen-jp/zumen/issues/8) の完了の条件のうち、試作と、ネイティブと wasm32 で同じ所で止まることの試験は、AutoCAD の系統の試作（[#3](https://github.com/zumen-jp/zumen/issues/3)）と一緒に入れる。
- AGENTS.md の「受け取ったファイルを信用しない」から、この記録を参照する。

## 参照

- [#1](https://github.com/zumen-jp/zumen/issues/1) の「5. 層をまたぐ仕組み」の「予算と、信用しない入力」、[#8](https://github.com/zumen-jp/zumen/issues/8)
- 旧版の構成の [Limits for untrusted drawings](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/architecture.en.md#limits-for-untrusted-drawings)
- [旧版の決定 0020](https://github.com/zumen-jp/zumen-legacy/blob/main/docs/decisions/0020-pdf-drawings-in-rust.md)
- [0007](0007-single-rust-implementation.md)、[0008](0008-model-layers-and-codecs.md)、[0009](0009-diagnostics-and-catalog.md)

## 訂正

- 2026-10-24: 中断とパニックの code の名前を、[0009](0009-diagnostics-and-catalog.md) の `<範囲>.<対象>.<出来事>` の規則に合わせた（`call.cancelled` → `call.request.cancelled`、`internal.panic` → `internal.panic.caught`）。判断は変えていない。
