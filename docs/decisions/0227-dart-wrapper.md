# 0227: Dart の包みは dart:ffi で C ABI を呼び、型は JSON の表を包む見方として生成し、取り消しは別の isolate の呼び出しで行い、毎晩 Dart 3.0 と安定版で流す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- Dart は、階層 2 の言語である。生成のテンプレートと境界の関数の包みを持ち、適合試験を毎晩流す（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。Dart はネイティブのライブラリを dart:ffi で呼ぶ（[0014](0014-platforms-signing-and-distribution.md)）。
- 下限は Dart 3（Flutter 3）にする（[docs/support.md](../support.md)）。
- dart:ffi の呼び出しは、終わるまで isolate を止める。isolate はメモリを分けるので、ロックを分け合えない。
- Flutter のアプリは、Android では APK の `jniLibs` に、iOS ではアプリに静的にリンクするか署名したフレームワークに、デスクトップでは実行ファイルの隣に、ライブラリを入れる。
- 手本は Python・.NET・Java・Swift の包み（[0195](0195-python-wrapper.md)・[0196](0196-dotnet-wrapper.md)・[0224](0224-java-kotlin-wrapper.md)・[0225](0225-swift-wrapper.md)）。境界は足すだけで、包みは知らない欄と列挙の値も受け取る（[0015](0015-add-only-boundary-and-version-floors.md)）。
- pub.dev に出すことは、段階 2 の別の子 issue で扱う。

## 選択肢

### 型の書き方

1. **欄を持つクラスに読み直す。** 知らない欄を落とす。
2. **読んだ JSON のオブジェクト（`Map<String, Object?>`）を包むクラスを生成する。** 欄ごとにゲッターとセッターを持つ。知らない欄も残る。

### 取り消し

1. **`cancel` を、ほかの isolate から呼べるようにする。** isolate の間でロックを分け合えないので、`cancel` と `close` が重なると、閉じたセッションを指してしまう。
2. **呼び出しを別の isolate で行う `callBytesAsync` を足し、待っている isolate が `cancel` を呼ぶ。** 呼び出しが動いている間の `close` は、呼び出しが終わってから閉じる。`cancel` と `close` は、同じ isolate の中で順に並ぶ。

### Flutter のアプリでのライブラリの配り方

1. **Dart のビルドのフック（native assets）で、パッケージがライブラリを作るか取ってくる。** 新しい Dart と Flutter が要り、下限の Dart 3.0 では使えない。
2. **ライブラリの道筋を `Zumen.open` に渡し、静的にリンクしたものは `Zumen.process` で使う。** ライブラリを入れる場所は、アプリの側で決める。

## 決定

型の書き方は 2、取り消しは 2、ライブラリの配り方は 2 を選ぶ。

- **置き場所。** 包みを `bindings/dart` に置き、パッケージ `zumen` にする。依存は `ffi`（BSD-3-Clause）だけ。試験は `crypto`（BSD-3-Clause）を使う。
- **形**
  - `Zumen.open`（道筋）か `Zumen.process`（静的にリンクしたもの）で読み込み、ABI の大版を確かめる。`Zumen.session`（`SessionOptions` の値）か `sessionBytes`（JSON のバイト列）でセッションを作る。
  - `Session` が `push`・`pushBytes`・`call`・`callBytes`・`callText`・`take`・`cancel`・`close` を持ち、境界の関数（[0012](0012-boundary-functions-and-messages.md)）に 1 つずつ当たる。`callBytesAsync` は、`zm_call` を `Isolate.run` で別の isolate で呼ぶ。
  - コアが返したバッファーは、写したあとに包みの中で `zm_free` で返す。長さ 0 の入力にも NULL でない番地を渡す。
  - 状態が `ok` でなければ `ZumenException` を投げる。`failed` の応答も、`response` で読める。閉じたセッションへの呼び出しは NULL のまま境界に渡し、境界が `invalid` を返す。
  - シーンの blob は `Scene.read` で読む。区画は `u8`・`buffer`（写さない）か `u32`・`f32`・`f64`（写す）で引く。知らない区画は読み飛ばし、ない区画は空の列にし、大版が違えば読まない（[0137](0137-columnar-scene-transfer.md)）。
- **生成するもの**（`cargo xtask codegen`）
  - `lib/src/messages.dart`: `schema/messages.json` の定義ごとのクラス（`Message` を継ぐ）。欄はゲッターとセッターで読み書きし（ない欄と形の違う値は `null`、`null` を書くと欄を除く）、名前はキャメルケースにする。Dart の語と `Message` のメンバーと重なる名前には `Field` を付け、列挙の値と op の名前で語と重なるものには `Value` を付ける。目印の欄（`op` など）は、作るときに入れる。列挙は文字列の定数、共用体は形ごとの見方に移す `asXxx`、op の名前は `Operations` に置く。名前が重なれば、生成を止める。
  - `lib/src/constants.dart`: 版、状態コード（`Status` と名前）、シーンの区画の定義と値の組。
  - `pubspec.yaml` の `version` が、コアと同じかを確かめる。
- **適合試験**
  - `tool/conformance.dart` が、`bindings/conformance` の組を包みのメソッドで流す。状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することと、シーンの blob を読み手で読んで応答の数と合うことを確かめる。型を渡す `call` と、`callBytesAsync` と、呼び出しの間の `close` も確かめる。
  - 毎晩（`nightly.yml`）、Linux x64 で Dart 3.0 と安定版、macOS arm64 と Windows x64 で安定版を流す。

## 影響

- 階層 2 の 3 つの条件（生成のテンプレート・境界の関数の包み・毎晩の適合試験）を、この PR で満たす。
- `messages.dart` は大きい（約 5,000 行）。新しい op・欄を足すと、その差分が PR に並ぶ。生成したファイルは `dart format` にかけない。
- 取り消したいときは `callBytesAsync` を使う。別の isolate を起こすので、呼び出しごとに少し時間がかかる。
- Flutter のアプリ（Android・iOS）は、試験していない。ライブラリを入れる手順と、ビルドのフックを使うかは、pub.dev に出す issue で決める。32 bit の Windows は、Dart が出していないので扱わない。
- 包みの API と生成した型は、公開するまで変えてよい。公開してからは、境界と同じく足すだけにする。

## 参照

- [0006](0006-language-tiers.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)、[0195](0195-python-wrapper.md)、[0196](0196-dotnet-wrapper.md)、[0224](0224-java-kotlin-wrapper.md)、[0225](0225-swift-wrapper.md)
- [docs/support.md](../support.md)、[bindings/dart](../../bindings/dart/README.md)
- #433
