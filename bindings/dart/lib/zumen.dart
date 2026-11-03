/// zumen のネイティブのライブラリの C ABI を、dart:ffi で呼ぶラッパー（決定 0011・0227）。
///
/// [Zumen.open] でライブラリを読み込み、[Zumen.session] でセッションを作る。[Session] のメソッドは、API の関数
/// （schema/boundary.toml）に 1 つずつ当たる。要求と応答の型は `messages.dart` にある。
library;

export 'src/constants.dart';
export 'src/message.dart' show Message;
export 'src/messages.dart';
export 'src/scene.dart';
export 'src/session.dart';
