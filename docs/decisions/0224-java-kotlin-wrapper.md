# 0224: Java・Kotlin の包みは JNA で C ABI を呼び、型は JSON の表を包む見方として生成し、毎晩 Java 8 と Android 7.0 のエミュレーターで流す

| 項目 | 内容 |
| --- | --- |
| 日付 | 2026-10-31 |
| 状態 | 採用 |

## 背景

- Java と Kotlin（Android）は、階層 2 の言語である。生成のテンプレートと境界の関数の包みを持ち、適合試験を毎晩流す（[0006](0006-language-tiers.md)・[0011](0011-bindings-through-ffi.md)）。
- 下限は Java 8 と Android 7.0（API 24）にする（[docs/support.md](../support.md)）。Java 8 は業務システムに多く、JNA 5 の下限でもある。
- 手本は Python と .NET の包み（[0195](0195-python-wrapper.md)・[0196](0196-dotnet-wrapper.md)）。境界は足すだけで、包みは知らない欄と列挙の値も受け取る（[0015](0015-add-only-boundary-and-version-floors.md)）。
- Java 8 と Android には、標準の JSON の道具がない。
- Maven Central に出すことは、#435 で扱う。

## 選択肢

### ネイティブの呼び方

1. **JNI のグルーを C で書く。** 速いが、対象ごとにグルーのライブラリを作って配る。
2. **JNA。** Java 8 と Android（aar）で動き、関数の形を Java のインターフェースで宣言する。呼び出しは JNI より遅いが、境界の呼び出しは粗い（要求ごとに 1 回）。
3. **FFM（`java.lang.foreign`）。** 依存がないが、Java 22 からで、Android にない。

### 型の書き方

1. **クラスに読み直す（Jackson・Gson などで対応づける）。** 依存が増え、知らない欄を落とす。
2. **読んだ JSON の表（`Map`）を包む見方のクラスを生成する。** 欄ごとに読む関数と書く関数を持つ。写さず、知らない欄も残る。

### Kotlin

1. Kotlin の包みを別に生成する。
2. Java の包みを Kotlin からそのまま使う。

## 決定

ネイティブの呼び方は 2、型の書き方は 2、Kotlin は 2 を選ぶ。

- **置き場所。** 包みを `bindings/java` に置き、パッケージを `dev.zumen` にする。`pom.xml` の版はコアと同じにし、xtask が確かめる（[0015](0015-add-only-boundary-and-version-floors.md)）。
- **依存。** 依存は JNA 5.19.1 だけにする。JNA は Apache-2.0 と LGPL-2.1 以降の二重のライセンスで、Apache-2.0 で使う。JSON の小さな読み書き（`Json`）を、包みの中に持つ。
- **形**
  - `Zumen.load(道筋)` で、ライブラリを読み込む。関数がそろうことと ABI の大版を確かめる。32 bit の Windows では、JNA の呼び出しの規約を stdcall にする。
  - `Zumen.open` でセッションを作る。`Session` は `push`・`call`・`callBytes`・`callText`・`take`・`cancel`・`close` を持ち、境界の関数（[0012](0012-boundary-functions-and-messages.md)）に 1 つずつ当たる。`AutoCloseable` にする。
  - コアが返したバッファーは、写したあとに包みの中で `zm_free` で返す。長さ 0 の入力にも NULL でない番地を渡す。
  - 状態が `ok` でなければ `ZumenException` を投げる。`failed` の応答も、例外の `getResponse()` で読める。閉じたセッションへの呼び出しは NULL のまま境界に渡し、境界が `invalid` を返す。`cancel` は、ほかのスレッドから呼べる。
  - シーンの blob は `Scene.read` で読む。区画は `u8`・`u32`・`f32`・`f64`（写す）と `buffer`（写さない、リトルエンディアンの `ByteBuffer`）で引く。知らない区画は読み飛ばし、ない区画は空の列にし、大版が違えば読まない（[0137](0137-columnar-scene-transfer.md)）。
  - Android 7.0 で使えない API（`String.join`・`Integer.toUnsignedLong` など、API 26 から）は使わない。
- **生成するもの**（`cargo xtask codegen`）
  - `Messages.java`: `schema/messages.json` の定義ごとの見方のクラス。欄は `getXxx`（ない欄と形の違う値は `null`）と `withXxx`（`null` なら欄を除く）で読み書きする。目印の欄（`op` など）は、作るときに入れる。列挙は文字列の定数、共用体は形ごとの型に移す `asXxx`、op の名前は `Operations` に置く。
  - `Constants.java`: 版、状態コード（`Status` と名前）、シーンの区画の定義と値の組。
- **Kotlin。** Kotlin からは、`dev.zumen` のクラスをそのまま使う。Android では、JNA の aar を依存に加え、ネイティブのライブラリを APK の `jniLibs` に入れて、その道筋を `Zumen.load` に渡す。
- **適合試験**
  - `dev.zumen.Conformance` が、`bindings/conformance` の組を包みのメソッドで流す。状態・応答の値・ハッシュの一覧が `hashes.txt` と一致することと、シーンの blob を読み手で読んで応答の数と合うことを確かめる。
  - 毎晩（`nightly.yml`）、Java 8（Zulu）で Linux x64・macOS arm64・Windows x64・Windows x86 を流す。
  - Android は、NDK で API 24 に向けて作ったライブラリを使い、API 24 の x86_64 のエミュレーターで流す。包みと試験と JNA の aar のクラスを d8 で dex にし、`app_process` で流す。シェルの利用者は dalvik-cache に書けないので、`dalvikvm` は使わない。
  - JNA は Maven Central から取り、SHA-256 を確かめる（`fetch-jna.sh`）。

## 影響

- 階層 2 の 3 つの条件（生成のテンプレート・境界の関数の包み・毎晩の適合試験）を、この PR で満たす。
- `Messages.java` は大きい（約 1.2 万行）。新しい op・欄を足すと、その差分が PR に並ぶ。
- `zm_take_arrow`（Arrow の C Stream Interface）は、包みのメソッドにしない。
- 32 bit の Android（armeabi-v7a）と arm64 の実機、Android の APK の形（aar に包むこと）は、試験していない。配る形は #435 で決める。
- 包みの API と生成した型は、公開するまで変えてよい。公開してからは、境界と同じく足すだけにする。

## 参照

- [0006](0006-language-tiers.md)、[0011](0011-bindings-through-ffi.md)、[0012](0012-boundary-functions-and-messages.md)、[0014](0014-platforms-signing-and-distribution.md)、[0015](0015-add-only-boundary-and-version-floors.md)、[0137](0137-columnar-scene-transfer.md)、[0195](0195-python-wrapper.md)、[0196](0196-dotnet-wrapper.md)
- [docs/support.md](../support.md)、[bindings/java](../../bindings/java/README.md)
- [JNA](https://github.com/java-native-access/jna)
- #430、#435
