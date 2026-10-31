import 'dart:collection';
import 'dart:convert';

/// 欄の値の形が、型と違う印
final class _Mismatch implements Exception {
  const _Mismatch();
}

/// 境界のメッセージの型の土台。読んだ JSON のオブジェクトを包み、知らない欄もそのまま持つ（決定 0015）。
abstract class Message {
  Message(this.json);

  /// 包んでいる JSON のオブジェクト
  final Map<String, Object?> json;

  /// 欄を読む。ない欄と、形の違う値は null
  T? readField<T>(String key, T? Function(Object?) decode) {
    final value = json[key];
    if (value == null) {
      return null;
    }
    try {
      return decode(value);
    } on _Mismatch {
      return null;
    }
  }

  /// 欄を書く。null を書くと欄を除く
  void writeField(String key, Object? value) {
    if (value == null) {
      json.remove(key);
    } else {
      json[key] = encodeValue(value);
    }
  }

  /// 目印の欄が値と同じなら、その形の見方。違えば null
  T? taggedView<T>(String key, String value, T Function(Map<String, Object?>) of) =>
      json[key] == value ? of(json) : null;

  /// `jsonEncode` が使う
  Map<String, Object?> toJson() => json;

  @override
  String toString() => jsonEncode(json);
}

/// 値を JSON の値にする。メッセージは包んでいるオブジェクトにする
Object? encodeValue(Object? value) => switch (value) {
      Message() => value.json,
      List() => [for (final item in value) encodeValue(item)],
      Map() => {for (final entry in value.entries) entry.key as String: encodeValue(entry.value)},
      _ => value,
    };

String asString(Object? value) => value is String ? value : throw const _Mismatch();

bool asBool(Object? value) => value is bool ? value : throw const _Mismatch();

int asInt(Object? value) => value is int ? value : throw const _Mismatch();

double asDouble(Object? value) => value is num ? value.toDouble() : throw const _Mismatch();

/// 型を定めない JSON の配列と辞書は、要素に触れたときに包む。深い値を一度にたどらない
Object? asJson(Object? value) => switch (value) {
      List() => _JsonList(value),
      Map<String, Object?>() => _JsonMap(value),
      _ => value,
    };

final class _JsonList extends ListBase<Object?> {
  _JsonList(this._values);

  final List _values;

  @override
  int get length => _values.length;

  @override
  set length(int length) => throw UnsupportedError('変更できない配列');

  @override
  Object? operator [](int index) => asJson(_values[index]);

  @override
  void operator []=(int index, Object? value) => throw UnsupportedError('変更できない配列');
}

final class _JsonMap extends MapBase<String, Object?> {
  _JsonMap(this._values);

  final Map<String, Object?> _values;

  @override
  Object? operator [](Object? key) => asJson(_values[key]);

  @override
  void operator []=(String key, Object? value) => throw UnsupportedError('変更できない辞書');

  @override
  void clear() => throw UnsupportedError('変更できない辞書');

  @override
  Iterable<String> get keys => _values.keys;

  @override
  Object? remove(Object? key) => throw UnsupportedError('変更できない辞書');
}

/// 配列を、要素ごとに読み、変更できない列で返す。形の違う要素があれば、配列ごと読まない
List<T> Function(Object?) listOf<T>(T Function(Object?) decode) =>
    (value) => value is List ? List<T>.unmodifiable([for (final item in value) decode(item)]) : throw const _Mismatch();

/// オブジェクトを、値ごとに読み、変更できない表で返す。形の違う値があれば、オブジェクトごと読まない
Map<String, T> Function(Object?) mapOf<T>(T Function(Object?) decode) => (value) => value is Map<String, Object?>
    ? Map<String, T>.unmodifiable({for (final entry in value.entries) entry.key: decode(entry.value)})
    : throw const _Mismatch();

/// オブジェクトを、メッセージの型で包む
T Function(Object?) objectOf<T>(T Function(Map<String, Object?>) of) =>
    (value) => value is Map<String, Object?> ? of(value) : throw const _Mismatch();
