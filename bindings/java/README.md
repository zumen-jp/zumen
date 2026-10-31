# Java の包み

ネイティブのライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib`）の C ABI を、JNA で呼ぶ薄い包みです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0224](../../docs/decisions/0224-java-kotlin-wrapper.md)）。下限は Java 8 と Android 7.0（API 24）です。Kotlin からも、同じクラスをそのまま使います。

- パッケージは `dev.zumen` です。Maven Central にはまだ出していません（#435）。
- 依存は JNA 5.19.1（Apache-2.0 と LGPL-2.1 以降の二重のライセンス。Apache-2.0 で使う）だけです。JSON の読み書きは、包みの中の `Json` で行います。
- `Zumen.load(道筋)` で、ライブラリを読み込みます。32 bit の Windows では、関数を stdcall で呼びます。Android では、APK の `jniLibs` に入れたライブラリの道筋（`ApplicationInfo.nativeLibraryDir`）を渡し、JNA の aar を依存に加えます。
- `src/main/java/dev/zumen/Messages.java`・`Constants.java` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

```java
import dev.zumen.*;
import java.nio.file.Files;
import java.nio.file.Paths;

Zumen zumen = Zumen.load("target/release/libzumen_abi1.so");
try (Session session = zumen.open()) {
    session.push("a.dxf", Files.readAllBytes(Paths.get("a.dxf")));
    long v = Constants.SCHEMA_MAJOR;
    String doc = session.call(new Messages.OpenRequest().withV(v).withInput("a.dxf"))
            .result(Messages.OpenResult::of).getDoc();
    Messages.Response scene = session.call(new Messages.SceneRequest().withV(v).withDoc(doc).withFrom("space/0"));
    Scene columns = Scene.read(session.take(scene.getBlobs().get(0).getId().intValue()));
    double[] origins = columns.f64("chunk_origins");
} catch (ZumenException error) {
    // failed の応答は error.getResponse() にあり、診断を持ちます
    System.out.println(error.getStatus() + " " + error.getResponse());
}
```

| 名前 | 中身 |
| --- | --- |
| `Zumen.load` | ライブラリを読み込み、関数と ABI の大版を確かめる |
| `Zumen.open` | セッションを作る（`zm_session_new`）。try-with-resources で閉じる |
| `Session.push`・`call`・`callBytes`・`take`・`cancel`・`close` | 境界の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenException` を投げる |
| `Scene.read` | シーンの blob を読む。区画は `u8`・`u32`・`f32`・`f64`（写し）か `buffer`（写さない `ByteBuffer`）で引く（`schema/scene.toml`） |

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあります。`Messages` の型は、読んだ JSON のオブジェクト（`Map`）を包む見方で、知らない欄もそのまま持ちます。欄は `getXxx` で読み（ない欄と形の違う値は `null`）、`withXxx` で書きます。列挙は `String` で持ち、値を定数で出すので、知らない値も受け取れます。共用体は `asXxx` で形ごとの型に移します。`Messages.Operations` に op の名前があります。`cancel` は、ほかのスレッドから呼べます。

## 試験

```bash
cargo build -p zumen-abi
sh bindings/java/fetch-jna.sh target/jna
javac -encoding UTF-8 -cp target/jna/jna.jar -d target/java bindings/java/src/main/java/dev/zumen/*.java bindings/java/src/test/java/dev/zumen/*.java
java -cp target/java:target/jna/jna.jar dev.zumen.Conformance target/debug/libzumen_abi1.so
```

`dev.zumen.Conformance` は、[適合試験](../conformance/README.md)の組をこの包みで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。毎晩（`nightly.yml`）、Java 8 で Linux x64・macOS arm64・Windows x64・Windows x86 を流します。Android のエミュレーター（API 24、x86_64）でも、同じクラスを d8 で dex にし、`app_process` で流します（`android/run.sh`）。
