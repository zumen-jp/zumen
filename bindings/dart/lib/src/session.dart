import 'dart:convert';
import 'dart:ffi';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'constants.dart';
import 'messages.dart';

/// zm_buf。コアが確保したバイト列
final class _Buf extends Struct {
  external Pointer<Uint8> ptr;

  @UintPtr()
  external int len;
}

typedef _AbiVersionC = Uint32 Function();
typedef _AbiVersion = int Function();
typedef _SessionNewC = Int32 Function(Pointer<Uint8>, UintPtr, Pointer<Pointer<Void>>);
typedef _SessionNew = int Function(Pointer<Uint8>, int, Pointer<Pointer<Void>>);
typedef _PushC = Int32 Function(Pointer<Void>, Pointer<Uint8>, UintPtr, Pointer<Uint8>, UintPtr);
typedef _Push = int Function(Pointer<Void>, Pointer<Uint8>, int, Pointer<Uint8>, int);
typedef _CallC = Int32 Function(Pointer<Void>, Pointer<Uint8>, UintPtr, Pointer<_Buf>);
typedef _Call = int Function(Pointer<Void>, Pointer<Uint8>, int, Pointer<_Buf>);
typedef _TakeC = Int32 Function(Pointer<Void>, Uint32, Pointer<_Buf>);
typedef _Take = int Function(Pointer<Void>, int, Pointer<_Buf>);
typedef _SessionC = Void Function(Pointer<Void>);
typedef _SessionF = void Function(Pointer<Void>);
typedef _FreeC = Void Function(Pointer<_Buf>);
typedef _Free = void Function(Pointer<_Buf>);

/// 境界の関数の状態が ok でなかった（決定 0012）
final class ZumenException implements Exception {
  ZumenException(this.operation, this.code, this.data);

  /// 呼んだ境界の関数の名前
  final String operation;

  /// 状態コード（[Status]）
  final int code;

  /// コアが書いたバイト列（failed の応答）。書かなかったときは null
  final Uint8List? data;

  /// 状態の名前（failed・invalid など）
  String get status => Status.name(code);

  /// 書かれた応答。応答の JSON でなければ null
  Response? get response {
    final data = this.data;
    if (data == null || data.isEmpty) {
      return null;
    }
    try {
      final value = jsonDecode(utf8.decode(data));
      return value is Map<String, Object?> ? Response.fromJson(value) : null;
    } on FormatException {
      return null;
    }
  }

  @override
  String toString() {
    final code = response?.diagnostics?.firstOrNull?.code;
    return 'ZumenException: $operation の状態が $status${code == null ? '' : '（$code）'}';
  }
}

/// 読み込んだネイティブのライブラリの関数
final class _Functions {
  _Functions(DynamicLibrary library)
      : abiVersion = library.lookupFunction<_AbiVersionC, _AbiVersion>('zm_abi_version'),
        sessionNew = library.lookupFunction<_SessionNewC, _SessionNew>('zm_session_new'),
        push = library.lookupFunction<_PushC, _Push>('zm_push'),
        call = library.lookupFunction<_CallC, _Call>('zm_call'),
        take = library.lookupFunction<_TakeC, _Take>('zm_take'),
        cancel = library.lookupFunction<_SessionC, _SessionF>('zm_cancel'),
        sessionClose = library.lookupFunction<_SessionC, _SessionF>('zm_session_close'),
        free = library.lookupFunction<_FreeC, _Free>('zm_free');

  final _AbiVersion abiVersion;
  final _SessionNew sessionNew;
  final _Push push;
  final _Call call;
  final _Take take;
  final _SessionF cancel;
  final _SessionF sessionClose;
  final _Free free;

  /// ライブラリの道筋から読み込む。null なら、プロセスに静的にリンクしたもの
  static _Functions load(String? path) =>
      _Functions(path == null ? DynamicLibrary.process() : DynamicLibrary.open(path));
}

/// data を、ネイティブのメモリに写して body に渡す。長さ 0 でも NULL でない番地を渡す
R _borrow<R>(Uint8List data, R Function(Pointer<Uint8> ptr, int length) body) {
  final ptr = malloc<Uint8>(data.isEmpty ? 1 : data.length);
  try {
    ptr.asTypedList(data.length).setAll(0, data);
    return body(ptr, data.length);
  } finally {
    malloc.free(ptr);
  }
}

/// zm_buf を用意して呼び、書かれたバイト列を写してから zm_free で返す。書かれなかったときは null
(int, Uint8List?) _withBuf(_Functions functions, int Function(Pointer<_Buf> buf) body) {
  final buf = calloc<_Buf>();
  try {
    final code = body(buf);
    Uint8List? data;
    if (buf.ref.ptr != nullptr) {
      data = Uint8List.fromList(buf.ref.ptr.asTypedList(buf.ref.len));
    } else if (code == Status.ok) {
      data = Uint8List(0);
    }
    functions.free(buf);
    return (code, data);
  } finally {
    calloc.free(buf);
  }
}

(int, Uint8List?) _call(_Functions functions, Pointer<Void> handle, Uint8List request) =>
    _withBuf(functions, (buf) => _borrow(request, (ptr, length) => functions.call(handle, ptr, length, buf)));

/// 別の isolate で zm_call を呼ぶ。isolate に送れる値（道筋・番地・バイト列）だけを、関数の外から受け取る
Future<(int, Uint8List?)> _callInIsolate(String? path, int address, Uint8List request) =>
    Isolate.run(() => _call(_Functions.load(path), Pointer<Void>.fromAddress(address), request));

/// 読み込んだネイティブのライブラリ（決定 0011）
final class Zumen {
  Zumen._(this._path, this._functions);

  final String? _path;
  final _Functions _functions;

  /// ライブラリを道筋から読み込み、ABI の大版を確かめる
  factory Zumen.open(String path) => Zumen._checked(path);

  /// プロセスに静的にリンクしたライブラリを使う（iOS のアプリなど）
  factory Zumen.process() => Zumen._checked(null);

  factory Zumen._checked(String? path) {
    final zumen = Zumen._(path, _Functions.load(path));
    final version = zumen.abiVersion;
    if (version != Constants.abiVersion) {
      throw StateError('ABI の大版が $version（この包みは ${Constants.abiVersion}）');
    }
    return zumen;
  }

  /// ABI の大版（zm_abi_version）
  int get abiVersion => _functions.abiVersion();

  /// 設定（session_options）を値で渡して、セッションを作る。null なら既定の設定
  Session session([SessionOptions? options]) =>
      sessionBytes(options == null ? Uint8List(0) : utf8.encode(jsonEncode(options)));

  /// 設定を JSON の UTF-8 のバイト列で渡して、セッションを作る。長さが 0 なら既定の設定
  Session sessionBytes(Uint8List options) {
    final out = calloc<Pointer<Void>>();
    try {
      final code = _borrow(options, (ptr, length) => _functions.sessionNew(ptr, length, out));
      if (code != Status.ok) {
        throw ZumenException('zm_session_new', code, null);
      }
      return Session._(this, out.value);
    } finally {
      calloc.free(out);
    }
  }
}

/// コアのセッション（zm_session）。境界の関数に 1 つずつ当たるメソッドを持つ（決定 0012・0227）。
///
/// dart:ffi の呼び出しは、終わるまで isolate を止める。取り消すときは [callBytesAsync] で別の isolate で呼び、
/// 待っている間に [cancel] を呼ぶ。動いている呼び出しがある間は、[close] は呼び出しが終わるまで待ってから閉じる。
final class Session {
  Session._(this._zumen, this._handle);

  final Zumen _zumen;
  Pointer<Void> _handle;
  int _running = 0;
  bool _closePending = false;

  _Functions get _functions => _zumen._functions;

  /// 閉じたか
  bool get isClosed => _handle == nullptr || _closePending;

  /// 入力を名前で渡す（zm_push）。同じ名前なら置き換える
  void push(String name, Uint8List data) => pushBytes(utf8.encode(name), data);

  /// 入力を、名前のバイト列で渡す
  void pushBytes(Uint8List name, Uint8List data) {
    final code = _borrow(
      name,
      (namePtr, nameLength) =>
          _borrow(data, (dataPtr, dataLength) => _functions.push(_live, namePtr, nameLength, dataPtr, dataLength)),
    );
    if (code != Status.ok) {
      throw ZumenException('zm_push', code, null);
    }
  }

  /// 要求の JSON の UTF-8 を渡し、応答のバイト列を受け取る（zm_call）
  Uint8List callBytes(Uint8List request) => _result('zm_call', _call(_functions, _live, request));

  /// [callBytes] を別の isolate で呼ぶ。待っている間に [cancel] で止められる
  Future<Uint8List> callBytesAsync(Uint8List request) async {
    final path = _zumen._path;
    final address = _live.address;
    _running++;
    try {
      final outcome = await _callInIsolate(path, address, request);
      return _result('zm_call', outcome);
    } finally {
      _running--;
      if (_running == 0 && _closePending) {
        _closeNow();
      }
    }
  }

  /// 要求を JSON の文字列で渡し、応答の文字列を受け取る
  String callText(String request) => utf8.decode(callBytes(utf8.encode(request)));

  /// 要求の値（要求の型か、JSON にできる値）を渡し、応答を読む
  Response call(Object request) {
    final value = jsonDecode(utf8.decode(callBytes(utf8.encode(jsonEncode(request)))));
    return Response.fromJson(value as Map<String, Object?>);
  }

  /// 直前の call が作ったバイナリの結果を、番号で取り出す（zm_take）
  Uint8List take(int blob) => _result('zm_take', _withBuf(_functions, (buf) => _functions.take(_live, blob, buf)));

  /// 動いている呼び出しを止める（zm_cancel）。動いていなければ何もしない
  void cancel() {
    if (_handle != nullptr) {
      _functions.cancel(_handle);
    }
  }

  /// セッションを閉じる（zm_session_close）。2 度目からは何もしない
  void close() {
    if (_running > 0) {
      _closePending = true;
    } else {
      _closeNow();
    }
  }

  void _closeNow() {
    if (_handle != nullptr) {
      _functions.sessionClose(_handle);
      _handle = nullptr;
    }
    _closePending = false;
  }

  /// 閉じたセッションは NULL のまま境界に渡し、境界が invalid を返す
  Pointer<Void> get _live => _closePending ? nullptr : _handle;

  static Uint8List _result(String operation, (int, Uint8List?) outcome) {
    final (code, data) = outcome;
    if (code != Status.ok || data == null) {
      throw ZumenException(operation, code, data);
    }
    return data;
  }
}
