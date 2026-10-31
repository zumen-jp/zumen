# Dart の包み

ネイティブのライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib`）の C ABI を、dart:ffi で呼ぶ薄い包みです（[決定 0011](../../docs/decisions/0011-bindings-through-ffi.md)・[決定 0227](../../docs/decisions/0227-dart-wrapper.md)）。下限は Dart 3（Flutter 3）です。

- パッケージは `zumen` です。pub.dev にはまだ出していません。
- 依存は `ffi`（BSD-3-Clause）だけです。
- `Zumen.open(道筋)` でライブラリを読み込みます。アプリに静的にリンクしたとき（iOS など）は `Zumen.process()` を使います。
- `lib/src/messages.dart`・`constants.dart` は、`schema/` から `cargo xtask codegen` で生成します。手で直さないでください。

## 使い方

```dart
import 'dart:io';

import 'package:zumen/zumen.dart';

final zumen = Zumen.open('target/release/libzumen_abi1.so');
final session = zumen.session();
try {
  session.push('a.dxf', File('a.dxf').readAsBytesSync());
  const v = Constants.schemaMajor;
  final doc = session
      .call(OpenRequest()
        ..v = v
        ..input = 'a.dxf')
      .resultAs(OpenResult.fromJson)!
      .doc;
  final scene = session.call(SceneRequest()
    ..v = v
    ..doc = doc
    ..from = 'space/0');
  final columns = Scene.read(session.take(scene.blobs!.first.id!));
  final origins = columns.f64('chunk_origins');
} on ZumenException catch (error) {
  // failed の応答は error.response にあり、診断を持ちます
  print('${error.status} ${error.response}');
} finally {
  session.close();
}
```

| 名前 | 中身 |
| --- | --- |
| `Zumen.open`・`Zumen.process` | ライブラリを読み込み、ABI の大版を確かめる |
| `Zumen.session`・`sessionBytes` | セッションを作る（`zm_session_new`） |
| `Session.push`・`call`・`callBytes`・`callText`・`take`・`cancel`・`close` | 境界の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenException` を投げる |
| `Session.callBytesAsync` | `zm_call` を別の isolate で呼ぶ。待っている間に `cancel` で止められる |
| `Scene.read` | シーンの blob を読む。区画は `u8`・`buffer`（写さない）か `u32`・`f32`・`f64`（写す）で引く（`schema/scene.toml`） |

要求と応答の形は [schema/messages.json](../../schema/messages.json) にあります。`messages.dart` の型は、読んだ JSON のオブジェクト（`Map<String, Object?>`）を包む見方で、知らない欄もそのまま持ちます。欄はゲッターで読み（ない欄と形の違う値は `null`）、セッターで書きます。列挙は `String` で持ち、値を定数で出すので、知らない値も受け取れます。共用体は `asXxx` で形ごとの型に移します。`Operations` に op の名前があります。

dart:ffi の呼び出しは、終わるまで isolate を止めます。取り消したいときは `callBytesAsync` で呼び、待っている間に同じ isolate から `cancel` を呼びます。呼び出しが動いている間の `close` は、呼び出しが終わってから閉じます。

## 試験

```bash
cargo build -p zumen-abi
cd bindings/dart && dart pub get && dart run tool/conformance.dart ../../target/debug/libzumen_abi1.so
```

`tool/conformance.dart` は、[適合試験](../conformance/README.md)の組をこの包みで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。毎晩（`nightly.yml`）、Linux x64 で Dart 3.0 と安定版、macOS arm64 と Windows x64 で安定版を流します。
