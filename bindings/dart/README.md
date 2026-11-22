# Dart のラッパー

ネイティブのライブラリ（`zumen_abi1.dll`・`libzumen_abi1.so`・`libzumen_abi1.dylib`）の C ABI を、dart:ffi で呼ぶ薄いラッパーです。下限は Dart 3.5（Flutter 3.24）です。

- アプリのフォルダーで `dart pub add zumen` を実行し、[pub.dev の zumen](https://pub.dev/packages/zumen) を追加します。ネイティブのライブラリは同じ版の [GitHub Releases](https://github.com/zumen-jp/zumen/releases) から入手します。
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
| `Zumen.open`・`Zumen.process` | ライブラリを読み込み、ABI のメジャーバージョンを確かめる |
| `Zumen.session`・`sessionBytes` | セッションを作る（`zm_session_new`） |
| `Session.push`・`call`・`callBytes`・`callText`・`take`・`cancel`・`close` | API の関数（`zm_push`・`zm_call`・`zm_take`・`zm_cancel`・`zm_session_close`）。状態が `ok` でなければ `ZumenException` を投げる |
| `Session.callBytesAsync` | `zm_call` を別の isolate で呼ぶ。待っている間に `cancel` で止められる |
| `Scene.read` | シーンの blob を読む。セクションは `u8`・`buffer`（写さない）か `u32`・`f32`・`f64`（写す）で引く（`schema/scene.toml`） |

要求と応答の形は [schema/messages.json](https://github.com/zumen-jp/zumen/blob/main/schema/messages.json) にあります。`messages.dart` の型は、読んだ JSON のオブジェクト（`Map<String, Object?>`）を包むビューで、知らないフィールドもそのまま持ちます。フィールドはゲッターで読み（ないフィールドと形の違う値は `null`）、セッターで書きます。列挙は `String` で持ち、値を定数で出すので、知らない値も受け取れます。共用体は `asXxx` で形ごとの型に移します。`Operations` に op の名前があります。

ゲッターが返す配列と辞書は変更できません。要素を変えるときは、新しい配列や辞書を作ってセッターに渡してください。入れ子の配列も同じです。配列・辞書の要素がメッセージの型なら、そのフィールドは元の JSON を通して変更できます。

```dart
final request = CloseRequest()..inputs = ['a.dxf'];
request.inputs = [...request.inputs!, 'b.dxf'];
```

dart:ffi の呼び出しは、終わるまで isolate を止めます。取り消したいときは `callBytesAsync` で呼び、待っている間に同じ isolate から `cancel` を呼びます。呼び出しが動いている間の `close` は、呼び出しが終わってから閉じます。

取り消しは、処理が途中で合図を確認して止まる方式です。短い処理が取り消し前に終わる場合や、待っている isolate が止まっている間は、取り消しが間に合わない場合があります。非同期の呼び出しが終わるまでは、同じセッションのほかの操作は `busy` を返します。

## pub.dev 向けの梱包

リポジトリのルートで配布物を作ります。

```sh
python xtask/release/dart_package.py --out target/release-packages
```

`zumen-dart-<版>.tar.gz` を空のフォルダーに展開し、展開先で `dart pub publish --dry-run` を実行します。ラッパーのソース、pubspec、README、変更履歴、ライセンス本文を確認できます。pub.dev へ送る配布物の `pubspec.yaml` では `publish_to` を省きます。初版の 0.4.0 は GitHub Release の配布物から `publish_to: https://pub.dev` の 1 行を除いて公開しました。[pubspec の公開先](https://dart.dev/tools/pub/pubspec#publish_to)・[初版の記録](https://github.com/zumen-jp/zumen/issues/435)

初版を公開したら、pub.dev の `zumen` の Admin で GitHub Actions からの公開を有効にし、リポジトリを `zumen-jp/zumen`、タグのパターンを `v{{version}}`、必須の環境を `release` にします。後続のタグでは、その環境の承認後に検査済みの配布物を展開し、GitHub が発行する短期のトークンで公開します。[pub.dev の自動公開](https://dart.dev/tools/pub/automated-publishing)

## テスト

```bash
cargo build -p zumen-abi
cd bindings/dart && dart pub get && dart run tool/conformance.dart ../../target/debug/libzumen_abi1.so
dart run tool/test_collections.dart
dart run tool/test_async.dart ../../target/debug/libzumen_abi1.so
```

`tool/conformance.dart` は、[適合テスト](https://github.com/zumen-jp/zumen/blob/main/bindings/conformance/README.md)の組をこのラッパーで流し、ハッシュの一覧が `hashes.txt` と一致することを確かめます。毎晩（`nightly.yml`）、Linux x64 で Dart 3.5 と安定版、macOS arm64 と Windows x64 で安定版を流します。
`tool/test_collections.dart` は、メッセージの配列・辞書の読み書きを確かめます。
`tool/test_async.dart` は、取り消しとライブラリの相対パスを確かめます。

## 3D のシーン

Scene3D.read(blob) で ZMS3 を読みます。instanceElement と elementId で建物の要素 ID を引けます。u8 は元のバッファーを共有し、u32・f32・f64 は配列へコピーします。

同じ blob の 2D 用の読み手との混用はできません。列の配置・座標・単位・色・選択と検査範囲は [3D のシーンの契約](https://github.com/zumen-jp/zumen/blob/main/schema/README.md#3d-のシーンの契約)に従います。元の blob を共有する読み手の利用中は、そのバイト列を変更しません。
