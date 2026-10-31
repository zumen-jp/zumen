import CZumen
import Foundation

/// 境界の関数の状態が ok でなかった（決定 0012）
public struct ZumenError: Error, CustomStringConvertible {
    /// 呼んだ境界の関数の名前
    public let operation: String
    /// 状態コード（`Constants.Status`）
    public let code: Int32
    /// コアが書いたバイト列（failed・panicked の応答）。書かなかったときは nil
    public let data: Data?

    /// 状態の名前（`failed`・`invalid` など）
    public var status: String { Constants.Status.name(code) }

    /// 書かれた応答。応答の JSON でなければ nil
    public var response: Response? {
        data.flatMap { try? JSON.parse($0) }.flatMap(Response.init(jsonValue:))
    }

    public var description: String {
        var text = "\(operation) の状態が \(status)"
        if let first = response?.diagnostics?.first, let code = first.code {
            text += "（\(code)）"
        }
        return text
    }
}

/// コアのセッション（`zm_session`）。境界の関数に 1 つずつ当たるメソッドを持つ（決定 0012・0225）
///
/// 1 つのセッションは、一度に 1 つのスレッドだけが使う。`cancel` だけは、どのスレッドからも呼べる。閉じたセッションへの呼び出しは
/// NULL のまま境界に渡し、境界が invalid を返す。
public final class Session: @unchecked Sendable {
    /// cancel と close を、ほかのスレッドとの間で順に並べる
    private let lock = NSLock()
    private var handle: OpaquePointer?

    /// 既定の設定でセッションを作る
    public convenience init() throws {
        try self.init(optionsData: Data())
    }

    /// 設定（session_options）を値で渡して、セッションを作る
    public convenience init(options: SessionOptions) throws {
        try self.init(optionsData: try options.jsonValue.data())
    }

    /// 設定を JSON の UTF-8 のバイト列で渡して、セッションを作る。長さが 0 なら既定の設定
    public init(optionsData: Data) throws {
        let version = zm_abi_version()
        guard version == Constants.abiVersion else {
            throw ZumenError(operation: "zm_abi_version", code: Constants.Status.invalid, data: nil)
        }
        var handle: OpaquePointer?
        let code = borrow(optionsData) { zm_session_new($0, $1, &handle) }
        guard code == Constants.Status.ok else {
            throw ZumenError(operation: "zm_session_new", code: code, data: nil)
        }
        self.handle = handle
    }

    deinit {
        close()
    }

    /// ABI の大版（`zm_abi_version`）
    public static var abiVersion: UInt32 { zm_abi_version() }

    /// 閉じたか
    public var isClosed: Bool {
        lock.lock()
        defer { lock.unlock() }
        return handle == nil
    }

    /// 入力を名前で渡す（`zm_push`）。同じ名前なら置き換える
    public func push(_ name: String, _ data: Data) throws {
        try push(nameData: Data(name.utf8), data)
    }

    /// 入力を、名前のバイト列で渡す
    public func push(nameData: Data, _ data: Data) throws {
        let code = borrow(nameData) { name, nameLength in
            borrow(data) { bytes, length in
                zm_push(handle, name, nameLength, bytes, length)
            }
        }
        guard code == Constants.Status.ok else {
            throw ZumenError(operation: "zm_push", code: code, data: nil)
        }
    }

    /// 要求の JSON の UTF-8 を渡し、応答のバイト列を受け取る（`zm_call`）
    public func callData(_ request: Data) throws -> Data {
        let (code, data) = withBuf { buf in
            borrow(request) { zm_call(handle, $0, $1, buf) }
        }
        guard code == Constants.Status.ok, let data else {
            throw ZumenError(operation: "zm_call", code: code, data: data)
        }
        return data
    }

    /// 要求を JSON の文字列で渡し、応答の文字列を受け取る
    public func callText(_ request: String) throws -> String {
        String(decoding: try callData(Data(request.utf8)), as: UTF8.self)
    }

    /// 要求を値で渡し、応答を読む
    public func call(_ request: JSON) throws -> Response {
        let data = try callData(try request.data())
        guard let response = Response(jsonValue: try JSON.parse(data)) else {
            throw JSONError("応答が JSON のオブジェクトでない")
        }
        return response
    }

    /// 要求の型を渡し、応答を読む
    public func call<T: Message>(_ request: T) throws -> Response {
        try call(request.jsonValue)
    }

    /// 直前の call が作ったバイナリの結果を、番号で取り出す（`zm_take`）
    public func take(_ blob: UInt32) throws -> Data {
        let (code, data) = withBuf { zm_take(handle, blob, $0) }
        guard code == Constants.Status.ok, let data else {
            throw ZumenError(operation: "zm_take", code: code, data: data)
        }
        return data
    }

    /// 動いている呼び出しを止める（`zm_cancel`）。ほかのスレッドから呼べる。動いていなければ何もしない
    public func cancel() {
        lock.lock()
        defer { lock.unlock() }
        if let handle {
            zm_cancel(handle)
        }
    }

    /// セッションを閉じる（`zm_session_close`）。2 度目からは何もしない
    public func close() {
        lock.lock()
        defer { lock.unlock() }
        if let handle {
            zm_session_close(handle)
            self.handle = nil
        }
    }

    /// zm_buf を用意して呼び、書かれたバイト列を写してから zm_free で返す。書かれなかったときは nil
    private func withBuf(_ call: (UnsafeMutablePointer<zm_buf>) -> zm_status) -> (Int32, Data?) {
        var buf = zm_buf(ptr: nil, len: 0)
        let code = call(&buf)
        defer { zm_free(&buf) }
        if let ptr = buf.ptr {
            return (code, Data(bytes: ptr, count: buf.len))
        }
        return (code, code == Constants.Status.ok ? Data() : nil)
    }
}

/// 入力のバイト列を、呼び出しの間だけ借りる。長さ 0 の入力にも、NULL でない番地を渡す
private func borrow<R>(_ data: Data, _ body: (UnsafePointer<UInt8>, Int) -> R) -> R {
    if data.isEmpty {
        var zero: UInt8 = 0
        return withUnsafePointer(to: &zero) { body($0, 0) }
    }
    return data.withUnsafeBytes { raw in
        body(raw.bindMemory(to: UInt8.self).baseAddress!, raw.count)
    }
}
