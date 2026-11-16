import 'dart:convert';
import 'dart:typed_data';

import 'constants.dart';
import 'scene3d_layout.dart';

const _header = 16;
const _entry = 8;

/// シーンの blob（決定 0213）のリーダー。
///
/// ヘッダー（16 バイト）とセクションの表（セクションごとに 8 バイト）を読み、セクションを名前で引く。値はリトルエンディアン。知らないセクションは
/// 読み飛ばし、基本の 20 セクションを必要とする。メジャーバージョンが違えば読まない。
final class Scene3D {
  Scene3D._(this.major, this.minor, this.chunks, this.sections, this._columns);

  /// メジャーバージョン
  final int major;

  /// マイナーバージョン
  final int minor;

  /// 塊の数
  final int chunks;

  /// blob にあるセクションの数
  final int sections;

  final Map<String, Uint8List> _columns;

  /// blob を読む。セクションは blob を写さずに指す
  factory Scene3D.read(Uint8List blob) {
    if (blob.length > 536870912) throw const FormatException('3D シーンのバイト数の上限');
    if (blob.length < _header || ascii.decode(blob.sublist(0, 4), allowInvalid: true) != 'ZMS3') {
      throw const FormatException('シーンの blob でない');
    }
    final view = ByteData.sublistView(blob);
    final major = view.getUint16(4, Endian.little);
    final minor = view.getUint16(6, Endian.little);
    final chunks = view.getUint32(8, Endian.little);
    final sections = view.getUint32(12, Endian.little);
    if (major != Scene3DLayout.major) {
      throw FormatException('シーンのメジャーバージョンが $major（このラッパーは ${Scene3DLayout.major}）');
    }
    if (_header + _entry * sections > blob.length) {
      throw const FormatException('セクションの表が blob に収まらない');
    }
    if (sections < 20) throw const FormatException('3D シーンの基本列がない');
    var previous = _header + _entry * sections;
    for (var i = 0; i < sections; i++) {
      final start = view.getUint32(_header + _entry*i, Endian.little);
      final length = view.getUint32(_header + _entry*i + 4, Endian.little);
      if (start % 8 != 0 || start < previous || start + length > blob.length) {
        throw const FormatException('3D シーンのセクションの範囲');
      }
      previous = start + length;
    }
    final columns = <String, Uint8List>{};
    for (var index = 0; index < Scene3DLayout.sections.length; index++) {
      final section = Scene3DLayout.sections[index];
      if (index >= sections) {
        columns[section.name] = Uint8List(0);
        continue;
      }
      final at = _header + _entry * index;
      final start = view.getUint32(at, Endian.little);
      final length = view.getUint32(at + 4, Endian.little);
      if (start + length > blob.length || length % (section.type.size * section.width) != 0) {
        throw FormatException('セクション ${section.name} の始まりか長さが合わない');
      }
      columns[section.name] = Uint8List.sublistView(blob, start, start + length);
    }
    return Scene3D._(major, minor, chunks, sections, columns);
  }

  int _reference(String name, int row, [int component = 0]) {
    final width = _section(name).width;
    if (row < 0 || row >= rows(name) || component < 0 || component >= width) throw RangeError('3D シーンの行の範囲');
    return ByteData.sublistView(_columns[name]!).getUint32((row*width+component)*4, Endian.little);
  }
  int _instanceReference(String name, String target, int row) {
    final value = _reference(name, row);
    if (value >= rows(target)) throw RangeError('3D シーンの参照');
    return value;
  }
  /// 元の建物の要素 ID
  String elementId(int row) => text(_reference('element_ids', row), _reference('element_ids', row, 1));
  /// インスタンスが指すメッシュの行
  int instanceMesh(int row) => _instanceReference('instance_meshes', 'mesh_vertices', row);
  /// インスタンスが指す要素の行
  int instanceElement(int row) => _instanceReference('instance_elements', 'element_ids', row);
  /// インスタンスが指す色の行
  int instanceColor(int row) => _instanceReference('instance_colors', 'color_rgba', row);

  static SceneSection _section(String name) {
    for (final section in Scene3DLayout.sections) {
      if (section.name == name) {
        return section;
      }
    }
    throw ArgumentError.value(name, 'name', '知らないセクション');
  }

  Uint8List _column(String name, ColumnType type) {
    if (_section(name).type != type) {
      throw ArgumentError.value(name, 'name', 'セクションの値の型が違う');
    }
    return _columns[name]!;
  }

  /// セクションのバイト列（写さない）。値はリトルエンディアン
  Uint8List buffer(String name) => _columns[_section(name).name]!;

  /// セクションの行の数
  int rows(String name) {
    final section = _section(name);
    return _columns[name]!.length ~/ (section.type.size * section.width);
  }

  /// u8 のセクションの値（写さない）
  Uint8List u8(String name) => _column(name, ColumnType.u8);

  /// u32 のセクションの値（写す）
  Uint32List u32(String name) {
    final data = ByteData.sublistView(_column(name, ColumnType.u32));
    return Uint32List.fromList([for (var i = 0; i < data.lengthInBytes; i += 4) data.getUint32(i, Endian.little)]);
  }

  /// f32 のセクションの値（写す）
  Float32List f32(String name) {
    final data = ByteData.sublistView(_column(name, ColumnType.f32));
    return Float32List.fromList([for (var i = 0; i < data.lengthInBytes; i += 4) data.getFloat32(i, Endian.little)]);
  }

  /// u16 のセクションの値（写す）
  Uint16List u16(String name) {
    final data = ByteData.sublistView(_column(name, ColumnType.u16));
    return Uint16List.fromList(List<int>.generate(data.lengthInBytes ~/ 2, (i) => data.getUint16(i * 2, Endian.little)));
  }

  /// f64 のセクションの値（写す）
  Float64List f64(String name) {
    final data = ByteData.sublistView(_column(name, ColumnType.f64));
    return Float64List.fromList([for (var i = 0; i < data.lengthInBytes; i += 8) data.getFloat64(i, Endian.little)]);
  }

  /// strings のセクションから、文字列を引く（UTF-8）
  String text(int start, int length) {
    final strings = _column('strings', ColumnType.u8);
    if (start < 0 || length < 0 || start + length > strings.length) {
      throw RangeError('strings のセクションの外を指す');
    }
    final bytes = Uint8List.sublistView(strings, start, start + length);
    final value = utf8.decode(bytes);
    // Utf8Decoder.convert は先頭の BOM を除く。列の文字列では内容として保つ。
    // https://api.dart.dev/dart-convert/Utf8Decoder/convert.html
    final bom = bytes.length >= 3 && bytes[0] == 0xef && bytes[1] == 0xbb && bytes[2] == 0xbf;
    return bom ? '\uFEFF$value' : value;
  }
}
