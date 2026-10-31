import Foundation

/// シーンの blob（決定 0137）の読み手
///
/// 見出し（16 バイト）と区画の表（区画ごとに 8 バイト）を読み、区画を名前で引く。値はリトルエンディアン。知らない区画は読み飛ばし、
/// blob にない区画は空の列にする。大版が違えば読まない。
public struct Scene: Sendable {
    private static let header = 16
    private static let entry = 8

    /// 大版
    public let major: UInt16
    /// 小版
    public let minor: UInt16
    /// 塊の数
    public let chunks: UInt32
    /// blob にある区画の数
    public let sections: UInt32

    private let blob: Data
    /// 区画の名前から、blob の中の範囲
    private let columns: [String: Range<Int>]

    /// blob を読む
    public init(_ blob: Data) throws {
        let bytes = [UInt8](blob)
        func u16(_ at: Int) -> UInt16 { UInt16(bytes[at]) | UInt16(bytes[at + 1]) << 8 }
        func u32(_ at: Int) -> UInt32 {
            (0..<4).reduce(UInt32(0)) { $0 | UInt32(bytes[at + $1]) << (8 * UInt32($1)) }
        }
        guard bytes.count >= Scene.header, bytes[0..<4].elementsEqual("ZMSC".utf8) else {
            throw JSONError("シーンの blob でない")
        }
        major = u16(4)
        minor = u16(6)
        chunks = u32(8)
        sections = u32(12)
        guard major == Constants.sceneMajor else {
            throw JSONError("シーンの大版が \(major)（この包みは \(Constants.sceneMajor)）")
        }
        guard Scene.header + Scene.entry * Int(sections) <= bytes.count else {
            throw JSONError("区画の表が blob に収まらない")
        }
        var columns: [String: Range<Int>] = [:]
        for (index, section) in Constants.sceneSections.enumerated() {
            guard index < Int(sections) else {
                columns[section.name] = 0..<0
                continue
            }
            let start = Int(u32(Scene.header + Scene.entry * index))
            let length = Int(u32(Scene.header + Scene.entry * index + 4))
            let row = section.type.size * section.width
            guard start + length <= bytes.count, length % row == 0 else {
                throw JSONError("区画 \(section.name) の始まりか長さが合わない")
            }
            columns[section.name] = start..<start + length
        }
        self.blob = Data(bytes)
        self.columns = columns
    }

    private static func section(_ name: String) throws -> Constants.SceneSection {
        guard let section = Constants.sceneSections.first(where: { $0.name == name }) else {
            throw JSONError("知らない区画: \(name)")
        }
        return section
    }

    private func column(_ name: String, _ type: Constants.ColumnType) throws -> Data {
        let section = try Scene.section(name)
        guard section.type == type else {
            throw JSONError("区画 \(name) の値の型は \(section.type)")
        }
        return blob.subdata(in: columns[name]!)
    }

    /// 区画のバイト列（写す）。値はリトルエンディアン
    public func buffer(_ name: String) throws -> Data {
        try column(name, try Scene.section(name).type)
    }

    /// 区画の行の数
    public func rows(_ name: String) throws -> Int {
        let section = try Scene.section(name)
        return columns[name]!.count / (section.type.size * section.width)
    }

    private func values<T>(_ name: String, _ type: Constants.ColumnType, _ read: (UnsafeRawBufferPointer, Int) -> T) throws -> [T] {
        let data = try column(name, type)
        return data.withUnsafeBytes { raw in
            (0..<data.count / type.size).map { read(raw, $0 * type.size) }
        }
    }

    /// u8 の区画の値
    public func u8(_ name: String) throws -> [UInt8] {
        [UInt8](try column(name, .u8))
    }

    /// u32 の区画の値
    public func u32(_ name: String) throws -> [UInt32] {
        try values(name, .u32) { UInt32(littleEndian: $0.loadUnaligned(fromByteOffset: $1, as: UInt32.self)) }
    }

    /// f32 の区画の値
    public func f32(_ name: String) throws -> [Float] {
        try values(name, .f32) {
            Float(bitPattern: UInt32(littleEndian: $0.loadUnaligned(fromByteOffset: $1, as: UInt32.self)))
        }
    }

    /// f64 の区画の値
    public func f64(_ name: String) throws -> [Double] {
        try values(name, .f64) {
            Double(bitPattern: UInt64(littleEndian: $0.loadUnaligned(fromByteOffset: $1, as: UInt64.self)))
        }
    }

    /// strings の区画から、文字列を引く（UTF-8）
    public func text(start: UInt32, length: UInt32) throws -> String {
        let strings = try column("strings", .u8)
        let start = Int(start)
        let end = start + Int(length)
        guard end <= strings.count else {
            throw JSONError("strings の区画の外を指す")
        }
        guard let text = String(data: strings.subdata(in: start..<end), encoding: .utf8) else {
            throw JSONError("strings の区画の文字列が UTF-8 でない")
        }
        return text
    }
}
