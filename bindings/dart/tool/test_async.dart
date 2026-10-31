import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:zumen/zumen.dart';

Uint8List request(Object value) =>
    Uint8List.fromList(utf8.encode(jsonEncode(value)));

void expectStatus(String expected, void Function() action) {
  try {
    action();
  } on ZumenException catch (error) {
    if (error.status == expected) return;
    throw StateError('期待した状態は $expected、実際は ${error.status}');
  }
  throw StateError('期待した状態 $expected が返らなかった');
}

Future<void> expectCancelled(Future<Uint8List> pending) async {
  try {
    await pending;
  } on ZumenException catch (error) {
    if (error.status == 'failed' &&
        error.response?.diagnostics?.firstOrNull?.code ==
            'call.request.cancelled') {
      return;
    }
    rethrow;
  }
  throw StateError('取り消した呼び出しが成功した');
}

Future<void> main(List<String> args) async {
  final library = File(args.single).absolute;
  final originalDirectory = Directory.current;
  final Zumen zumen;
  final Zumen byName;
  try {
    Directory.current = library.parent;
    zumen = Zumen.open(
        '.${Platform.pathSeparator}${library.uri.pathSegments.last}');
    byName = Zumen.open(library.uri.pathSegments.last);
    Directory.current = Directory.systemTemp;
    final byNameSession = byName.session();
    try {
      await byNameSession.callBytesAsync(request(CapabilitiesRequest()..v = Constants.schemaMajor));
    } finally {
      byNameSession.close();
    }
    final session = zumen.session();
    try {
      // 非同期処理が、開いたときの相対パスを使って同じライブラリを開ける。
      final req = request(CapabilitiesRequest()..v = Constants.schemaMajor);
      final reply = await session.callBytesAsync(req);
      if ((jsonDecode(utf8.decode(reply)) as Map)['result'] == null) {
        throw StateError('相対パスで開いた非同期呼び出しに結果がない');
      }

      // 予約中は他の呼び出しを busy にし、即時 cancel は native の操作を始めない。
      session.push('a', Uint8List.fromList([1]));
      final close = request(CloseRequest()
        ..v = Constants.schemaMajor
        ..inputs = ['a']);
      final pending = session.callBytesAsync(close);
      expectStatus('busy', () => session.push('b', Uint8List(0)));
      expectStatus('busy', () => session.callBytes(req));
      expectStatus('busy', () => session.take(0));
      session.cancel();
      await expectCancelled(pending);
      // cancel された close は入力を消していない。
      final closeResult = session.call(CloseRequest()
        ..v = Constants.schemaMajor
        ..inputs = ['a']);
      if (closeResult.diagnostics?.isNotEmpty ?? false) {
        throw StateError('取り消した close が入力を消した: $closeResult');
      }

      // isolate 起動後に cancel しても、トークンの登録まで再送して止める。
      final dxf = StringBuffer('0\nSECTION\n2\nENTITIES\n');
      for (var i = 0; i < 100000; i++) {
        dxf.write('0\nLINE\n10\n0\n20\n0\n11\n1\n21\n1\n');
      }
      dxf.write('0\nENDSEC\n0\nEOF\n');
      session.push(
          'large.dxf', Uint8List.fromList(utf8.encode(dxf.toString())));
      final active = session.callBytesAsync(request(OpenRequest()
        ..v = Constants.schemaMajor
        ..input = 'large.dxf'));
      await Future<void>.value();
      session.cancel();
      await expectCancelled(active);

      // タイマーが次の call を取り消さない。
      final after =
          session.call(CapabilitiesRequest()..v = Constants.schemaMajor);
      if (after.resultAs(CapabilitiesResult.fromJson) == null) {
        throw StateError('取り消し後の呼び出しが失敗した');
      }
    } finally {
      session.close();
    }
  } finally {
    Directory.current = originalDirectory;
  }
  print('非同期の取り消しと相対パスを確認した');
}
