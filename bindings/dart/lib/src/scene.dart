import 'dart:convert';
import 'dart:typed_data';

import 'constants.dart';

const _header = 16;
const _entry = 8;

/// シーンの blob（決定 0137）のリーダー。
///
/// ヘッダー（16 バイト）とセクションの表（セクションごとに 8 バイト）を読み、セクションを名前で引く。値はリトルエンディアン。知らないセクションは
/// 読み飛ばし、blob にないセクションは空の列にする。メジャーバージョンが違えば読まない。
final class Scene {
  Scene._(this.major, this.minor, this.chunks, this.sections, this._columns);

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
  factory Scene.read(Uint8List blob) {
    if (blob.length < _header || ascii.decode(blob.sublist(0, 4), allowInvalid: true) != 'ZMSC') {
      throw const FormatException('シーンの blob でない');
    }
    final view = ByteData.sublistView(blob);
    final major = view.getUint16(4, Endian.little);
    final minor = view.getUint16(6, Endian.little);
    final chunks = view.getUint32(8, Endian.little);
    final sections = view.getUint32(12, Endian.little);
    if (major != Constants.sceneMajor) {
      throw FormatException('シーンのメジャーバージョンが $major（このラッパーは ${Constants.sceneMajor}）');
    }
    if (_header + _entry * sections > blob.length) {
      throw const FormatException('セクションの表が blob に収まらない');
    }
    final columns = <String, Uint8List>{};
    for (var index = 0; index < Constants.sceneSections.length; index++) {
      final section = Constants.sceneSections[index];
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
    return Scene._(major, minor, chunks, sections, columns);
  }

  static SceneSection _section(String name) {
    for (final section in Constants.sceneSections) {
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

  /// f64 のセクションの値（写す）
  Float64List f64(String name) {
    final data = ByteData.sublistView(_column(name, ColumnType.f64));
    return Float64List.fromList([for (var i = 0; i < data.lengthInBytes; i += 8) data.getFloat64(i, Endian.little)]);
  }

  /// strings のセクションから、文字列を引く（UTF-8）
  String text(int start, int length) {
    final strings = _column('strings', ColumnType.u8);
    if (start + length > strings.length) {
      throw RangeError('strings のセクションの外を指す');
    }
    return utf8.decode(Uint8List.sublistView(strings, start, start + length));
  }
}
