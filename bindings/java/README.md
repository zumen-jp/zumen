# Java のラッパー

ネイティブのライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib`）の C ABI を、JNA で呼ぶ薄いラッパーです（[依存とツールの条件](https://github.com/zumen-jp/zumen/blob/main/AGENTS.md#範囲と権利)）。下限は Java 21 と Android 12（API 31）です。Kotlin からも、同じクラスをそのまま使います。

- Maven の座標は `dev.zumen:zumen` です。ネイティブのライブラリは同じ版の [GitHub Releases](https://github.com/zumen-jp/zumen/releases) から入手します。[初回公開の進み具合](https://github.com/zumen-jp/zumen/issues/435)
- 依存は JNA 5.19.1（Apache-2.0 と LGPL-2.1 以降の二重のライセンス。Apache-2.0 で使う）だけです。JSON の読み書きは、ラッパーの中の `Json` で行います。
- `Zumen.load(道筋)` で、ライブラリを読み込みます。Android では、APK の `jniLibs` に入れたライブラリの道筋（`ApplicationInfo.nativeLibraryDir`）を渡し、JNA の aar を依存に加えます。
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
| `Zumen.load` | ライブラリを読み込み、関数と ABI のメジャーバージョンを確かめる |
| `Zumen.open` | セッションを作る（`zm_session_new`）。try-with-resources で閉じる |
| `Session.push`・`call`・`callBytes`・`take`・`cancel`・`close` | API の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenException` を投げる |
| `Scene.read` | シーンの blob を読む。セクションは `u8`・`u32`・`f32`・`f64`（写し）か `buffer`（写さない `ByteBuffer`）で引く（`schema/scene.toml`） |

要求と応答の形は [schema/messages.json](https://github.com/zumen-jp/zumen/blob/main/schema/messages.json) にあります。`Messages` の型は、読んだ JSON のオブジェクト（`Map`）を包むビューで、知らないフィールドもそのまま持ちます。フィールドは `getXxx` で読み（ないフィールドと形の違う値は `null`）、`withXxx` で書きます。列挙は `String` で持ち、値を定数で出すので、知らない値も受け取れます。共用体は `asXxx` で形ごとの型に移します。`Messages.Operations` に op の名前があります。`cancel` は、ほかのスレッドから呼べます。

## Maven Central 向けの梱包

JDK 21 と Maven を使います。リポジトリのルートで次を実行すると、本体・ソース・文書の JAR、POM とチェックサムを `bindings/java/target/central-publishing/central-bundle.zip` にまとめます。文書 JAR（`javadoc`）にはこの使い方とライセンス本文が入ります。メソッドごとの Javadoc はソース JAR で読めます。この検査は署名と送信を省きます。

```sh
mvn -B -ntp -f bindings/java/pom.xml -Pcentral "-Dproject.build.outputTimestamp=$(git show -s --format=%ct HEAD)" clean deploy
python xtask/release/verify_java.py bindings/java/target/central-publishing/central-bundle.zip
```

手元で公開するときは、同じタグをチェックアウトし、GitHub Release の `zumen-maven-<版>.zip` を取得します。たとえば v0.4.0 では、次のように自分の GnuPG 鍵を選んで署名します。`signed.zip` を Central Portal にアップロードし、検査が通ったら公開します。[名前空間の登録](https://central.sonatype.org/register/namespace/)・[Portal でのアップロード](https://central.sonatype.org/publish/publish-portal-upload/)

```sh
gh release download v0.4.0 --pattern zumen-maven-0.4.0.zip
python xtask/release/sign_java.py zumen-maven-0.4.0.zip signed.zip --key 署名鍵のフィンガープリント
```

タグから自動で出すときは、リポジトリの変数 `MAVEN_CENTRAL_PUBLISH` を `true` にします。GitHub の `release` 環境の secrets に `MAVEN_CENTRAL_TOKEN`（Portal が発行したユーザー名とパスワードを `:` で結んだ値）、`MAVEN_GPG_PRIVATE_KEY`（ASCII armor の署名鍵）、保護した鍵には `MAVEN_GPG_PASSPHRASE` を設定します。署名者の公開鍵を Central が参照できるようにします。ワークフローは検査済みのバンドルに署名を加え、ポータルで公開を終えるまで待ちます。[署名鍵の用意](https://central.sonatype.org/publish/requirements/gpg/)・[Portal API](https://central.sonatype.org/publish/publish-portal-api/)

公開ジョブを再実行するときは、前回のジョブのサマリーにある公開 ID を Central Portal で確認します。通信が切れて ID を受け取れなかったときは、`zumen-v<版>` の受付を調べます。受付や公開の状態が確かめられるまでは再送を保留します。再実行は同名の受付の一覧を調べ、処理中なら同じ ID の完了を待ちます。公開済みなら、配布先から取った各 JAR と POM がタグの配布物と一致することを確かめます。[Central Publisher API](https://central.sonatype.com/api-doc)

## テスト

```bash
cargo build -p zumen-abi
sh bindings/java/fetch-jna.sh target/jna
javac -encoding UTF-8 -cp target/jna/jna.jar -d target/java bindings/java/src/main/java/dev/zumen/*.java bindings/java/src/test/java/dev/zumen/*.java
java -cp target/java:target/jna/jna.jar dev.zumen.Conformance target/debug/libzumen_abi1.so
```

`dev.zumen.Conformance` は、[適合テスト](https://github.com/zumen-jp/zumen/blob/main/bindings/conformance/README.md)の組をこのラッパーで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。毎晩（`nightly.yml`）、Java 21 で Linux x64・macOS arm64・Windows x64 を流します。Android のエミュレーター（API 31、x86_64）でも、同じクラスを d8 で dex にし、`app_process` で流します（`android/run.sh`）。

## 3D のシーン

Scene3D.read(blob) で ZMS3 を読みます。scene.elementId((int)scene.instanceElement(0)) から建物の要素 ID を引けます。列のビューは元の ByteBuffer を共有し、配列への取得はコピーします。

同じ blob の 2D 用の読み手との混用はできません。列の配置・座標・単位・色・選択と検査範囲は [3D のシーンの契約](https://github.com/zumen-jp/zumen/blob/main/schema/README.md#3d-のシーンの契約)に従います。元の blob を共有する読み手の利用中は、そのバイト列を変更しません。
