import Foundation

/// JSON の値（RFC 8259）。境界の要求と応答を、この値で読み書きする（決定 0225）。
///
/// 整数は `Int64` に収まれば `integer`、収まらないか小数点か指数があれば `number` で読む。オブジェクトの欄の順は持たない。
public enum JSON: Sendable, Equatable {
    case null
    case bool(Bool)
    case integer(Int64)
    case number(Double)
    case string(String)
    case array([JSON])
    case object([String: JSON])

    /// 入れ子の深さの上限。読むときと書くときに確かめる
    static let maxDepth = 512

    /// 文字列なら、その値
    public var stringValue: String? {
        if case .string(let value) = self { return value }
        return nil
    }

    /// 真偽値なら、その値
    public var boolValue: Bool? {
        if case .bool(let value) = self { return value }
        return nil
    }

    /// 整数なら、その値。小数で読んだ値も、整数で 2^53 までなら返す
    public var integerValue: Int64? {
        switch self {
        case .integer(let value):
            return value
        case .number(let value) where value.rounded() == value && abs(value) <= 9_007_199_254_740_991:
            return Int64(value)
        default:
            return nil
        }
    }

    /// 数なら、その値
    public var numberValue: Double? {
        switch self {
        case .integer(let value): return Double(value)
        case .number(let value): return value
        default: return nil
        }
    }

    /// 配列なら、その要素
    public var arrayValue: [JSON]? {
        if case .array(let value) = self { return value }
        return nil
    }

    /// オブジェクトなら、その欄
    public var objectValue: [String: JSON]? {
        if case .object(let value) = self { return value }
        return nil
    }

    /// オブジェクトの欄。オブジェクトでないか、欄がなければ nil
    public subscript(key: String) -> JSON? {
        objectValue?[key]
    }

    /// UTF-8 の JSON を読む
    public static func parse(_ data: Data) throws -> JSON {
        var reader = Reader(bytes: [UInt8](data))
        reader.space()
        let value = try reader.value(depth: 0)
        reader.space()
        if reader.at != reader.bytes.count {
            throw reader.error("値のあとに文字がある")
        }
        return value
    }

    /// JSON の文字列を読む
    public static func parse(_ text: String) throws -> JSON {
        try parse(Data(text.utf8))
    }

    /// JSON の文字列にする。有限でない数は書けない
    public func text() throws -> String {
        var out = ""
        try write(&out, depth: 0)
        return out
    }

    /// UTF-8 の JSON にする
    public func data() throws -> Data {
        Data(try text().utf8)
    }

    private func write(_ out: inout String, depth: Int) throws {
        if depth > JSON.maxDepth {
            throw JSONError("入れ子が深すぎる")
        }
        switch self {
        case .null:
            out += "null"
        case .bool(let value):
            out += value ? "true" : "false"
        case .integer(let value):
            out += String(value)
        case .number(let value):
            guard value.isFinite else {
                throw JSONError("有限でない数は JSON に書けない")
            }
            out += "\(value)"
        case .string(let value):
            JSON.writeString(&out, value)
        case .array(let items):
            out += "["
            for (index, item) in items.enumerated() {
                if index > 0 { out += "," }
                try item.write(&out, depth: depth + 1)
            }
            out += "]"
        case .object(let fields):
            out += "{"
            // 欄の順は持たないので、名前の順に書いて、同じ値なら同じ文字列にする
            for (index, key) in fields.keys.sorted().enumerated() {
                if index > 0 { out += "," }
                JSON.writeString(&out, key)
                out += ":"
                try fields[key]!.write(&out, depth: depth + 1)
            }
            out += "}"
        }
    }

    private static func writeString(_ out: inout String, _ text: String) {
        out += "\""
        for scalar in text.unicodeScalars {
            switch scalar {
            case "\"": out += "\\\""
            case "\\": out += "\\\\"
            case "\u{8}": out += "\\b"
            case "\u{c}": out += "\\f"
            case "\n": out += "\\n"
            case "\r": out += "\\r"
            case "\t": out += "\\t"
            default:
                if scalar.value < 0x20 {
                    let hex = String(scalar.value, radix: 16)
                    out += "\\u" + String(repeating: "0", count: 4 - hex.count) + hex
                } else {
                    out.unicodeScalars.append(scalar)
                }
            }
        }
        out += "\""
    }

    /// バイト列から JSON を読む
    private struct Reader {
        let bytes: [UInt8]
        var at = 0

        func error(_ message: String) -> JSONError {
            JSONError("JSON を読めない（\(at) バイト目）: \(message)")
        }

        mutating func space() {
            while at < bytes.count, [0x20, 0x09, 0x0a, 0x0d].contains(bytes[at]) {
                at += 1
            }
        }

        mutating func literal(_ word: String) -> Bool {
            let word = Array(word.utf8)
            guard at + word.count <= bytes.count, Array(bytes[at..<at + word.count]) == word else {
                return false
            }
            at += word.count
            return true
        }

        mutating func value(depth: Int) throws -> JSON {
            if depth > JSON.maxDepth {
                throw error("入れ子が深すぎる")
            }
            guard at < bytes.count else {
                throw error("値がない")
            }
            switch bytes[at] {
            case UInt8(ascii: "{"): return try object(depth: depth)
            case UInt8(ascii: "["): return try array(depth: depth)
            case UInt8(ascii: "\""): return .string(try string())
            case UInt8(ascii: "-"), UInt8(ascii: "0")...UInt8(ascii: "9"): return try number()
            default:
                if literal("true") { return .bool(true) }
                if literal("false") { return .bool(false) }
                if literal("null") { return .null }
                throw error("値でない文字")
            }
        }

        mutating func object(depth: Int) throws -> JSON {
            at += 1
            var out: [String: JSON] = [:]
            space()
            if at < bytes.count, bytes[at] == UInt8(ascii: "}") {
                at += 1
                return .object(out)
            }
            while true {
                space()
                guard at < bytes.count, bytes[at] == UInt8(ascii: "\"") else {
                    throw error("欄の名前がない")
                }
                let key = try string()
                space()
                guard at < bytes.count, bytes[at] == UInt8(ascii: ":") else {
                    throw error(": がない")
                }
                at += 1
                space()
                out[key] = try value(depth: depth + 1)
                space()
                guard at < bytes.count else {
                    throw error("オブジェクトが閉じていない")
                }
                let c = bytes[at]
                at += 1
                if c == UInt8(ascii: "}") { return .object(out) }
                if c != UInt8(ascii: ",") { throw error(", か } がない") }
            }
        }

        mutating func array(depth: Int) throws -> JSON {
            at += 1
            var out: [JSON] = []
            space()
            if at < bytes.count, bytes[at] == UInt8(ascii: "]") {
                at += 1
                return .array(out)
            }
            while true {
                space()
                out.append(try value(depth: depth + 1))
                space()
                guard at < bytes.count else {
                    throw error("配列が閉じていない")
                }
                let c = bytes[at]
                at += 1
                if c == UInt8(ascii: "]") { return .array(out) }
                if c != UInt8(ascii: ",") { throw error(", か ] がない") }
            }
        }

        mutating func hex4() throws -> UInt32 {
            guard at + 4 <= bytes.count else {
                throw error("\\u のあとに 4 桁がない")
            }
            var code: UInt32 = 0
            for _ in 0..<4 {
                guard let digit = UInt32(String(UnicodeScalar(bytes[at])), radix: 16) else {
                    throw error("\\u のあとが 16 進でない")
                }
                code = code * 16 + digit
                at += 1
            }
            return code
        }

        mutating func string() throws -> String {
            at += 1
            var out = String.UnicodeScalarView()
            var start = at
            func flush(_ end: Int) throws {
                guard let text = String(bytes: bytes[start..<end], encoding: .utf8) else {
                    throw error("UTF-8 でない")
                }
                out.append(contentsOf: text.unicodeScalars)
            }
            while true {
                guard at < bytes.count else {
                    throw error("文字列が閉じていない")
                }
                let c = bytes[at]
                if c == UInt8(ascii: "\"") {
                    try flush(at)
                    at += 1
                    return String(out)
                }
                if c < 0x20 {
                    throw error("文字列に制御文字がある")
                }
                if c != UInt8(ascii: "\\") {
                    at += 1
                    continue
                }
                try flush(at)
                at += 1
                guard at < bytes.count else {
                    throw error("文字列が閉じていない")
                }
                let e = bytes[at]
                at += 1
                switch e {
                case UInt8(ascii: "\""): out.append("\"")
                case UInt8(ascii: "\\"): out.append("\\")
                case UInt8(ascii: "/"): out.append("/")
                case UInt8(ascii: "b"): out.append("\u{8}")
                case UInt8(ascii: "f"): out.append("\u{c}")
                case UInt8(ascii: "n"): out.append("\n")
                case UInt8(ascii: "r"): out.append("\r")
                case UInt8(ascii: "t"): out.append("\t")
                case UInt8(ascii: "u"):
                    var code = try hex4()
                    if (0xd800..<0xdc00).contains(code) {
                        // 上位のサロゲートは、続く \u の下位のサロゲートと組にする
                        guard at + 1 < bytes.count, bytes[at] == UInt8(ascii: "\\"), bytes[at + 1] == UInt8(ascii: "u") else {
                            throw error("サロゲートが組になっていない")
                        }
                        at += 2
                        let low = try hex4()
                        guard (0xdc00..<0xe000).contains(low) else {
                            throw error("サロゲートが組になっていない")
                        }
                        code = 0x10000 + ((code - 0xd800) << 10) + (low - 0xdc00)
                    }
                    guard let scalar = UnicodeScalar(code) else {
                        throw error("サロゲートが組になっていない")
                    }
                    out.append(scalar)
                default:
                    throw error("知らないエスケープ")
                }
                start = at
            }
        }

        mutating func digits() -> Int {
            let start = at
            while at < bytes.count, (UInt8(ascii: "0")...UInt8(ascii: "9")).contains(bytes[at]) {
                at += 1
            }
            return at - start
        }

        mutating func number() throws -> JSON {
            let start = at
            var integral = true
            if bytes[at] == UInt8(ascii: "-") {
                at += 1
            }
            guard at < bytes.count else {
                throw error("数が途中で終わる")
            }
            if bytes[at] == UInt8(ascii: "0") {
                at += 1
            } else if digits() == 0 {
                throw error("数の桁がない")
            }
            if at < bytes.count, bytes[at] == UInt8(ascii: ".") {
                integral = false
                at += 1
                if digits() == 0 { throw error("小数点のあとに桁がない") }
            }
            if at < bytes.count, bytes[at] == UInt8(ascii: "e") || bytes[at] == UInt8(ascii: "E") {
                integral = false
                at += 1
                if at < bytes.count, bytes[at] == UInt8(ascii: "+") || bytes[at] == UInt8(ascii: "-") {
                    at += 1
                }
                if digits() == 0 { throw error("指数の桁がない") }
            }
            let literal = String(decoding: bytes[start..<at], as: UTF8.self)
            // Int64 に収まらない整数は、小数で読む
            if integral, let value = Int64(literal) {
                return .integer(value)
            }
            guard let value = Double(literal) else {
                throw error("数を読めない")
            }
            return .number(value)
        }
    }
}

/// JSON の値と行き来できる型。メッセージの型の欄の型に使う
public protocol JSONRepresentable {
    /// JSON の値から読む。形が違えば nil
    init?(jsonValue: JSON)
    /// JSON の値にする
    var jsonValue: JSON { get }
}

extension JSON: JSONRepresentable {
    public init?(jsonValue: JSON) { self = jsonValue }
    public var jsonValue: JSON { self }
}

extension String: JSONRepresentable {
    public init?(jsonValue: JSON) {
        guard let value = jsonValue.stringValue else { return nil }
        self = value
    }
    public var jsonValue: JSON { .string(self) }
}

extension Bool: JSONRepresentable {
    public init?(jsonValue: JSON) {
        guard let value = jsonValue.boolValue else { return nil }
        self = value
    }
    public var jsonValue: JSON { .bool(self) }
}

extension Int64: JSONRepresentable {
    public init?(jsonValue: JSON) {
        guard let value = jsonValue.integerValue else { return nil }
        self = value
    }
    public var jsonValue: JSON { .integer(self) }
}

extension Double: JSONRepresentable {
    public init?(jsonValue: JSON) {
        guard let value = jsonValue.numberValue else { return nil }
        self = value
    }
    public var jsonValue: JSON { .number(self) }
}

/// 配列は、要素がどれも読めるときだけ読む
extension Array: JSONRepresentable where Element: JSONRepresentable {
    public init?(jsonValue: JSON) {
        guard let items = jsonValue.arrayValue else { return nil }
        var out: [Element] = []
        out.reserveCapacity(items.count)
        for item in items {
            guard let value = Element(jsonValue: item) else { return nil }
            out.append(value)
        }
        self = out
    }
    public var jsonValue: JSON { .array(map(\.jsonValue)) }
}

/// 表は、値がどれも読めるときだけ読む
extension Dictionary: JSONRepresentable where Key == String, Value: JSONRepresentable {
    public init?(jsonValue: JSON) {
        guard let fields = jsonValue.objectValue else { return nil }
        var out: [String: Value] = [:]
        for (key, field) in fields {
            guard let value = Value(jsonValue: field) else { return nil }
            out[key] = value
        }
        self = out
    }
    public var jsonValue: JSON { .object(mapValues(\.jsonValue)) }
}

/// メッセージの型が従う、JSON のオブジェクトの見方。知らない欄も `json` にそのまま持つ
public protocol Message: JSONRepresentable, Sendable {
    /// 包んでいる JSON のオブジェクト
    var json: [String: JSON] { get set }
    /// 読んだ JSON のオブジェクトを包む
    init(json: [String: JSON])
}

extension Message {
    /// JSON の値を包む。オブジェクトでなければ nil
    public init?(jsonValue: JSON) {
        guard let json = jsonValue.objectValue else { return nil }
        self.init(json: json)
    }

    public var jsonValue: JSON { .object(json) }

    /// JSON の文字列
    public var jsonText: String { (try? jsonValue.text()) ?? "" }

    /// 写しを書き換えて返す（例: `OpenRequest().with { $0.v = 1; $0.input = "a.dxf" }`）
    public func with(_ change: (inout Self) -> Void) -> Self {
        var copy = self
        change(&copy)
        return copy
    }

    /// 欄を読む。ない欄と形の違う値は nil
    public func readField<T: JSONRepresentable>(_ key: String) -> T? {
        json[key].flatMap(T.init(jsonValue:))
    }

    /// 欄を書く。nil なら欄を除く
    public mutating func writeField<T: JSONRepresentable>(_ key: String, _ value: T?) {
        json[key] = value?.jsonValue
    }

    /// 目印の欄が値と同じなら、その形の見方
    public func taggedView<T: Message>(_ key: String, _ value: String) -> T? {
        json[key] == .string(value) ? T(json: json) : nil
    }
}

/// JSON を読めないか、書けない
public struct JSONError: Error, CustomStringConvertible {
    public let message: String

    public init(_ message: String) {
        self.message = message
    }

    public var description: String { message }
}
