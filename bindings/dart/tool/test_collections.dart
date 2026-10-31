import 'dart:convert';

import 'package:zumen/zumen.dart';

void expect(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void expectUnsupported(void Function() change, String message) {
  try {
    change();
  } on UnsupportedError {
    return;
  }
  throw StateError(message);
}

void main() {
  final request = CloseRequest()..inputs = ['a.dxf'];
  expectUnsupported(() => request.inputs!.add('b.dxf'), '取得した配列を変更できてしまう');
  request.inputs = [...request.inputs!, 'b.dxf'];
  expect(jsonEncode(request).contains('"inputs":["a.dxf","b.dxf"]'), '配列をセッターで変更できない');

  final geometry = PolylineGeometry()
    ..points = [
      [1.0, 2.0],
      [3.0, 4.0],
    ];
  expectUnsupported(() => geometry.points![0][0] = 100.0, '入れ子の配列を変更できてしまう');
  geometry.points = [
    [100.0, geometry.points![0][1]],
    geometry.points![1],
  ];
  expect(geometry.points![0][0] == 100.0, '入れ子の配列をセッターで変更できない');

  final catalog = Catalog()..codes = {'a': '甲'};
  expectUnsupported(() => catalog.codes!['b'] = '乙', '取得した辞書を変更できてしまう');
  catalog.codes = {...catalog.codes!, 'b': '乙'};
  expect(catalog.codes!['b'] == '乙', '辞書をセッターで変更できない');

  final response = Response()
    ..result = {
      'unknown': [1, {'x': 2}],
    };
  final unknown = response.result!['unknown'] as List<Object?>;
  expectUnsupported(() => unknown.add(3), '型を定めない JSON の中の配列を変更できてしまう');
  final nested = unknown[1] as Map<String, Object?>;
  expectUnsupported(() => nested['x'] = 3, '型を定めない JSON の中の辞書を変更できてしまう');
  response.result = {...response.result!, 'unknown': [...unknown, 3]};
  expect((response.result!['unknown'] as List<Object?>).length == 3, '型を定めない JSON をセッターで変更できない');

  final scene = SceneRequest()..fonts = [SceneFont()..name = '古い名前'];
  scene.fonts!.first.name = '新しい名前';
  expect(scene.fonts!.first.name == '新しい名前', '配列の中のメッセージは元の JSON を通して変更できる');

  final bad = CloseRequest.fromJson({'inputs': ['a.dxf', 2], 'future': true});
  expect(bad.inputs == null, '形の違う要素を含む配列を読んでしまう');
  expect(bad.toJson()['future'] == true, '知らない欄が失われた');
  print('メッセージの配列・辞書を確認しました');
}
