// 適合試験を、Dart の包み（bindings/dart）を通して流す（決定 0006・0227）。
//
// 使い方: dart run tool/conformance.dart <ネイティブのライブラリ> [リポジトリの根]
//
// bindings/conformance/cases/*.json を、包みのセッションのメソッドで流し、状態と応答の値が期待どおりで、ハッシュの
// 一覧が hashes.txt と一致することを確かめる。シーンの blob は、包みの読み手で読み、応答の数と合うかを確かめる。
// 手順の読み方と応答の確かめ方は、Python・Java の側と同じ規則で書く。

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:zumen/zumen.dart';

/// 手順の種類。手順は、どれか 1 つの欄を持つ
const actions = ['session', 'push', 'call', 'call_text', 'call_hex', 'take'];

/// JSON Pointer で引けなかった印
const missing = Object();

final failures = <String>[];
final hashes = <String>[];
var scenes = 0;

Uint8List decodeHex(String text) {
  final digits = text.replaceAll(RegExp(r'\s'), '');
  if (digits.length.isOdd) {
    throw FormatException('16 進として読めない: $text');
  }
  return Uint8List.fromList([
    for (var i = 0; i < digits.length; i += 2) int.parse(digits.substring(i, i + 2), radix: 16),
  ]);
}

Uint8List utf8Bytes(String text) => Uint8List.fromList(utf8.encode(text));

/// JSON Pointer で値を引く。なければ missing
Object? pointer(Object? value, String path) {
  if (path.isEmpty) {
    return value;
  }
  var current = value;
  for (final raw in path.substring(1).split('/')) {
    final token = raw.replaceAll('~1', '/').replaceAll('~0', '~');
    if (current is List) {
      if (!RegExp(r'^(0|[1-9][0-9]*)$').hasMatch(token) || token.length > 9 || int.parse(token) >= current.length) {
        return missing;
      }
      current = current[int.parse(token)];
    } else if (current is Map && current.containsKey(token)) {
      current = current[token];
    } else {
      return missing;
    }
  }
  return current;
}

/// JSON の値として等しいか。真偽値と数は別にし、オブジェクトはキーの順によらない
bool same(Object? a, Object? b) {
  if (a is List || b is List) {
    return a is List &&
        b is List &&
        a.length == b.length &&
        [for (var i = 0; i < a.length; i++) i].every((i) => same(a[i], b[i]));
  }
  if (a is Map || b is Map) {
    return a is Map &&
        b is Map &&
        a.length == b.length &&
        a.keys.every((key) => b.containsKey(key) && same(a[key], b[key]));
  }
  return a == b;
}

/// session の設定か、call の要求のバイト列。値は JSON に直し、文字列はそのまま渡す
Uint8List bytesOf(Object? input, String valueKey, String textKey) {
  if (input is! Map) {
    return Uint8List(0);
  }
  if (input.containsKey(valueKey)) {
    return utf8Bytes(jsonEncode(input[valueKey]));
  }
  if (input[textKey] is String) {
    return utf8Bytes(input[textKey] as String);
  }
  return Uint8List(0);
}

/// 公開サンプル（決定 0171）を読む。道筋はリポジトリの根から書き、samples/ の中に限る
Uint8List readSample(String repository, String file) {
  final parts = file.split('/');
  final valid = parts.length >= 2 &&
      parts[0] == 'samples' &&
      parts.skip(1).every((part) => part.isNotEmpty && part != '.' && part != '..' && !part.contains(r'\'));
  if (!valid) {
    throw ArgumentError('push の file は samples/ の中のファイルにする: $file');
  }
  return File([repository, ...parts].join(Platform.pathSeparator)).readAsBytesSync();
}

/// call・call_text・call_hex の手順の、要求のバイト列
Uint8List requestOf(String action, Object? input) => switch (action) {
      'call_text' => utf8Bytes(input as String),
      'call_hex' => decodeHex(input as String),
      _ => utf8Bytes(jsonEncode(input)),
    };

/// 境界の関数を呼び、状態の名前と、書かれたバイト列を返す
(String, Uint8List?) attempt(Uint8List? Function() run) {
  try {
    return ('ok', run());
  } on ZumenException catch (error) {
    return (error.status, error.data);
  }
}

/// 応答の値を、手順の expect と absent で確かめる
void checkResponse(String at, Map step, Object? response) {
  final expect = step['expect'];
  if (expect is Map) {
    for (final entry in expect.entries) {
      final found = pointer(response, entry.key as String);
      if (identical(found, missing)) {
        failures.add('$at: ${entry.key} がない');
      } else if (!same(found, entry.value)) {
        failures.add('$at: ${entry.key} が ${jsonEncode(found)}（期待は ${jsonEncode(entry.value)}）');
      }
    }
  }
  final absent = step['absent'];
  if (absent is List) {
    for (final path in absent) {
      if (!identical(pointer(response, path as String), missing)) {
        failures.add('$at: $path があってはいけない');
      }
    }
  }
}

/// シーンの blob を読み手で読み、応答の数と合うかを確かめる
void checkScene(String at, Uint8List blob, Map result) {
  final Scene scene;
  try {
    scene = Scene.read(blob);
  } on FormatException catch (error) {
    failures.add('$at: シーンを読めない: ${error.message}');
    return;
  }
  final counts = result['counts'] as Map;
  final chunks = counts['chunks'];
  final checks = <(String, Object?, Object?)>[
    ('大版', scene.major, Constants.sceneMajor),
    ('区画の数', scene.sections, Constants.sceneSections.length),
    ('塊の数', scene.chunks, chunks),
    ('chunk_origins の行', scene.rows('chunk_origins'), chunks),
    ('text_xy の行', scene.rows('text_xy'), counts['texts']),
    ('point_xy の行', scene.rows('point_xy'), counts['points']),
    ('glyph_xy の行', scene.rows('glyph_xy'), counts['glyphs']),
    ('line_xy と face_xy の行', scene.rows('line_xy') + scene.rows('face_xy'), counts['vertices']),
  ];
  // 文字の文字列は、strings の区画から引ける
  final strings = scene.u32('text_strings');
  for (var row = 0; row + 1 < strings.length; row += 2) {
    try {
      scene.text(strings[row], strings[row + 1]);
    } on Object catch (error) {
      failures.add('$at: text_strings の行 ${row ~/ 2} を引けない: $error');
    }
  }
  for (final (name, got, expected) in checks) {
    if (got != expected) {
      failures.add('$at: シーンの$nameが $got（期待は $expected）');
    }
  }
}

void runCase(Zumen zumen, String repository, String name, List steps) {
  Session? session;
  // 直前の応答が返したシーンの blob の番号と結果
  Map? lastScene;
  var lastBlob = -1;
  for (var index = 0; index < steps.length; index++) {
    final at = '$name の手順 $index';
    final step = steps[index] as Map;
    final action = actions.where(step.containsKey).firstOrNull;
    if (action == null) {
      failures.add('$at: 手順の種類がない');
      continue;
    }
    final expected = step['status'] as String? ?? 'ok';
    final input = step[action];
    if (action != 'session') {
      session ??= zumen.session();
    }
    final current = session;
    late (String, Uint8List?) outcome;
    String? kind;
    switch (action) {
      case 'session':
        session?.close();
        Session? opened;
        outcome = attempt(() {
          opened = zumen.sessionBytes(bytesOf(input, 'options', 'options_text'));
          return null;
        });
        // 作れなかったら、閉じたセッションで続ける。境界は、空のセッションへの呼び出しを invalid にする
        session = opened ?? (zumen.session()..close());
      case 'push':
        final push = input as Map;
        final pushName =
            push.containsKey('name_hex') ? decodeHex(push['name_hex'] as String) : utf8Bytes(push['name'] as String);
        final data = push.containsKey('hex')
            ? decodeHex(push['hex'] as String)
            : push.containsKey('file')
                ? readSample(repository, push['file'] as String)
                : utf8Bytes(push['text'] as String? ?? '');
        outcome = attempt(() {
          current!.pushBytes(pushName, data);
          return null;
        });
      case 'take':
        final blob = input as int;
        outcome = attempt(() => current!.take(blob));
        kind = outcome.$2 != null ? 'blob' : null;
        if (outcome.$1 == 'ok' && lastScene != null && lastBlob == blob) {
          checkScene(at, outcome.$2!, lastScene);
          scenes++;
        }
      default:
        final request = requestOf(action, input);
        outcome = attempt(() => current!.callBytes(request));
        kind = outcome.$2 != null ? 'response' : null;
    }
    if (outcome.$1 != expected) {
      failures.add('$at: 状態が ${outcome.$1}（期待は $expected）');
    }
    if (kind == null) {
      continue;
    }
    final data = outcome.$2!;
    hashes.add('$name $index $kind ${sha256.convert(data)}');
    if (kind != 'response') {
      continue;
    }
    final response = jsonDecode(utf8.decode(data)) as Map;
    checkResponse(at, step, response);
    lastScene = null;
    lastBlob = -1;
    final result = response['result'];
    final blobs = response['blobs'];
    if (result is Map && blobs is List) {
      for (final blob in blobs) {
        if (blob is Map && blob['kind'] == 'scene') {
          lastScene = result;
          lastBlob = blob['id'] as int;
          break;
        }
      }
    }
  }
  session?.close();
}

/// ハッシュの一覧が hashes.txt と一致するかを確かめる
void compareHashes(String conformance) {
  final expected = File('$conformance/hashes.txt')
      .readAsStringSync()
      .split('\n')
      .where((line) => line.isNotEmpty && !line.startsWith('#'))
      .toList();
  if (expected.length == hashes.length &&
      [for (var i = 0; i < hashes.length; i++) i].every((i) => expected[i] == hashes[i])) {
    return;
  }
  final message = StringBuffer('ハッシュの一覧が hashes.txt と合わない:');
  for (final line in expected.where((line) => !hashes.contains(line))) {
    message.write('\n- $line');
  }
  for (final line in hashes.where((line) => !expected.contains(line))) {
    message.write('\n+ $line');
  }
  failures.add(message.toString());
}

/// 型を渡す call と、失敗した call と、別の isolate での call と取り消しと、閉じたあとの呼び出し
Future<void> checkTypedCalls(Zumen zumen) async {
  final session = zumen.session(SessionOptions()..v = Constants.schemaMajor);
  try {
    final response = session.call(CapabilitiesRequest()..v = Constants.schemaMajor);
    final result = response.resultAs(CapabilitiesResult.fromJson);
    if (response.v != Constants.schemaMajor || result == null || result.package == null) {
      failures.add('call の応答が capabilities の形でない: $response');
    }
    try {
      session.call(OpenRequest()
        ..v = Constants.schemaMajor
        ..input = 'none.dxf');
      failures.add('ない入力を開く call が失敗しない');
    } on ZumenException catch (error) {
      final failed = error.response;
      if (error.status != 'failed' || failed == null || (failed.diagnostics ?? []).isEmpty) {
        failures.add('ない入力を開く call の失敗が、診断のある failed でない: $error');
      }
    }
    // 別の isolate で呼んでも、同じ応答になる。動いている呼び出しがなければ、cancel は何もしない
    final request = utf8Bytes(jsonEncode(CapabilitiesRequest()..v = Constants.schemaMajor));
    final pending = session.callBytesAsync(request);
    session.close();
    if (!session.isClosed) {
      failures.add('動いている呼び出しがある間の close のあと、isClosed が false');
    }
    final asynchronous = await pending;
    if (!same(jsonDecode(utf8.decode(asynchronous)), response.json)) {
      failures.add('別の isolate の call の応答が違う');
    }
  } finally {
    session.close();
  }
  final (status, _) = attempt(() => session.callBytes(utf8Bytes('{"v":1,"op":"capabilities"}')));
  if (!session.isClosed || status != 'invalid') {
    failures.add('閉じたあとの call が $status（期待は invalid）');
  }
}

/// 渡されなければ、今の場所から上にたどって、bindings/conformance のあるフォルダーを根にする
String findRepository(List<String> args) {
  if (args.length >= 2) {
    return Directory(args[1]).absolute.path;
  }
  var current = Directory.current.absolute;
  while (true) {
    if (File('${current.path}/bindings/conformance/hashes.txt').existsSync()) {
      return current.path;
    }
    final parent = current.parent;
    if (parent.path == current.path) {
      throw StateError('リポジトリの根が見つからない。2 つめの引数で渡す');
    }
    current = parent;
  }
}

Future<void> main(List<String> args) async {
  if (args.isEmpty || args.length > 2) {
    stderr.writeln('使い方: dart run tool/conformance.dart <ネイティブのライブラリ> [リポジトリの根]');
    exit(2);
  }
  final zumen = Zumen.open(args[0]);
  final repository = findRepository(args);
  final conformance = '$repository/bindings/conformance';
  final files = Directory('$conformance/cases')
      .listSync()
      .whereType<File>()
      .where((file) => file.path.endsWith('.json'))
      .toList()
    ..sort((a, b) => a.path.compareTo(b.path));
  for (final file in files) {
    final name = file.uri.pathSegments.last.replaceAll(RegExp(r'\.json$'), '');
    final steps = (jsonDecode(file.readAsStringSync()) as Map)['steps'] as List;
    runCase(zumen, repository, name, steps);
  }
  compareHashes(conformance);
  await checkTypedCalls(zumen);
  final summary = '包みを通した試験の組 ${files.length} 件、ハッシュ ${hashes.length} 件、読んだシーン $scenes 件';
  stdout.writeln(summary);
  final stepSummary = Platform.environment['GITHUB_STEP_SUMMARY'];
  if (stepSummary != null && stepSummary.isNotEmpty) {
    File(stepSummary).writeAsStringSync(
      '### Dart の包みの適合試験（Dart ${Platform.version.split(' ').first}、${Platform.operatingSystem}）\n\n$summary\n',
      mode: FileMode.append,
    );
  }
  if (scenes == 0) {
    failures.add('シーンの blob を 1 つも読んでいない');
  }
  if (failures.isNotEmpty) {
    stderr.writeln('期待と違った:');
    failures.forEach(stderr.writeln);
    exit(1);
  }
  stdout.writeln('ネイティブと同じ一覧になった');
}
