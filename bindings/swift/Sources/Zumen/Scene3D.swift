import Foundation

/// シーンの blob（決定 0213）のリーダー
///
/// ヘッダー（16 バイト）とセクションの表（セクションごとに 8 バイト）を読み、セクションを名前で引く。値はリトルエンディアン。知らないセクションは読み飛ばし、
/// 基本の 20 セクションを必要とする。メジャーバージョンが違えば読まない。
public struct Scene3D: Sendable {
    private static let header = 16
    private static let entry = 8

    /// メジャーバージョン
    public let major: UInt16
    /// マイナーバージョン
    public let minor: UInt16
    /// 塊の数
    public let chunks: UInt32
    /// blob にあるセクションの数
    public let sections: UInt32

    private let blob: Data
    /// セクションの名前から、blob の中の範囲
    private let columns: [String: Range<Int>]

    /// blob を読む
    public init(_ blob: Data) throws {
        guard blob.count <= 536870912 else { throw JSONError("3D シーンのバイト数の上限") }
        let bytes = [UInt8](blob)
        func u16(_ at: Int) -> UInt16 { UInt16(bytes[at]) | UInt16(bytes[at + 1]) << 8 }
        func u32(_ at: Int) -> UInt32 {
            (0..<4).reduce(UInt32(0)) { $0 | UInt32(bytes[at + $1]) << (8 * UInt32($1)) }
        }
        guard bytes.count >= Scene3D.header, bytes[0..<4].elementsEqual("ZMS3".utf8) else {
            throw JSONError("シーンの blob でない")
        }
        major = u16(4)
        minor = u16(6)
        chunks = u32(8)
        sections = u32(12)
        guard major == Scene3DLayout.major else {
            throw JSONError("シーンのメジャーバージョンが \(major)（このラッパーは \(Scene3DLayout.major)）")
        }
        guard UInt64(Scene3D.header) + UInt64(Scene3D.entry) * UInt64(sections) <= UInt64(bytes.count) else {
            throw JSONError("セクションの表が blob に収まらない")
        }
        guard sections >= 20 else { throw JSONError("3D シーンの基本列がない") }
        var previous = Scene3D.header + Scene3D.entry * Int(sections)
        for i in 0..<Int(sections) {
            let start = Int(u32(Scene3D.header + Scene3D.entry * i))
            let length = Int(u32(Scene3D.header + Scene3D.entry * i + 4))
            guard start % 8 == 0, start >= previous, start <= bytes.count, length <= bytes.count - start else {
                throw JSONError("3D シーンのセクションの範囲")
            }
            previous = start + length
        }
        var columns: [String: Range<Int>] = [:]
        for (index, section) in Scene3DLayout.sections.enumerated() {
            guard index < Int(sections) else {
                columns[section.name] = 0..<0
                continue
            }
            let start = Int(u32(Scene3D.header + Scene3D.entry * index))
            let length = Int(u32(Scene3D.header + Scene3D.entry * index + 4))
            let row = section.type.size * section.width
            guard start + length <= bytes.count, length % row == 0 else {
                throw JSONError("セクション \(section.name) の始まりか長さが合わない")
            }
            columns[section.name] = start..<start + length
        }
        self.blob = Data(bytes)
        self.columns = columns
    }

    private func reference(_ name: String, _ row: Int, _ component: Int = 0) throws -> UInt32 {
        let section = try Scene3D.section(name)
        guard row >= 0, row < (try rows(name)), component >= 0, component < section.width, let range = columns[name] else {
            throw JSONError("3D シーンの行の範囲")
        }
        return blob.withUnsafeBytes { raw in
            UInt32(littleEndian: raw.loadUnaligned(fromByteOffset: range.lowerBound + (row * section.width + component) * 4, as: UInt32.self))
        }
    }
    private func instanceReference(_ name: String, _ target: String, _ row: Int) throws -> UInt32 {
        let value = try reference(name, row)
        guard UInt64(value) < UInt64(try rows(target)) else { throw JSONError("3D シーンの参照") }
        return value
    }
    /// 元の建物の要素 ID
    public func elementID(_ row: Int) throws -> String {
        try text(start: reference("element_ids", row), length: reference("element_ids", row, 1))
    }
    /// インスタンスが指すメッシュの行
    public func instanceMesh(_ row: Int) throws -> UInt32 { try instanceReference("instance_meshes", "mesh_vertices", row) }
    /// インスタンスが指す要素の行
    public func instanceElement(_ row: Int) throws -> UInt32 { try instanceReference("instance_elements", "element_ids", row) }
    /// インスタンスが指す色の行
    public func instanceColor(_ row: Int) throws -> UInt32 { try instanceReference("instance_colors", "color_rgba", row) }

    private static func section(_ name: String) throws -> Constants.SceneSection {
        guard let section = Scene3DLayout.sections.first(where: { $0.name == name }) else {
            throw JSONError("知らないセクション: \(name)")
        }
        return section
    }

    private func column(_ name: String, _ type: Constants.ColumnType) throws -> Data {
        let section = try Scene3D.section(name)
        guard section.type == type else {
            throw JSONError("セクション \(name) の値の型は \(section.type)")
        }
        return blob.subdata(in: columns[name]!)
    }

    /// セクションのバイト列（写す）。値はリトルエンディアン
    public func buffer(_ name: String) throws -> Data {
        try column(name, try Scene3D.section(name).type)
    }

    /// セクションの行の数
    public func rows(_ name: String) throws -> Int {
        let section = try Scene3D.section(name)
        return columns[name]!.count / (section.type.size * section.width)
    }

    private func values<T>(_ name: String, _ type: Constants.ColumnType, _ read: (UnsafeRawBufferPointer, Int) -> T) throws -> [T] {
        let data = try column(name, type)
        return data.withUnsafeBytes { raw in
            (0..<data.count / type.size).map { read(raw, $0 * type.size) }
        }
    }

    /// u8 のセクションの値
    public func u8(_ name: String) throws -> [UInt8] {
        [UInt8](try column(name, .u8))
    }

    /// u32 のセクションの値
    public func u32(_ name: String) throws -> [UInt32] {
        try values(name, .u32) { UInt32(littleEndian: $0.loadUnaligned(fromByteOffset: $1, as: UInt32.self)) }
    }

    /// f32 のセクションの値
    public func f32(_ name: String) throws -> [Float] {
        try values(name, .f32) {
            Float(bitPattern: UInt32(littleEndian: $0.loadUnaligned(fromByteOffset: $1, as: UInt32.self)))
        }
    }

    /// u16 のセクションの値
    public func u16(_ name: String) throws -> [UInt16] {
        try values(name, .u16) {
            UInt16(littleEndian: $0.loadUnaligned(fromByteOffset: $1, as: UInt16.self))
        }
    }

    /// f64 のセクションの値
    public func f64(_ name: String) throws -> [Double] {
        try values(name, .f64) {
            Double(bitPattern: UInt64(littleEndian: $0.loadUnaligned(fromByteOffset: $1, as: UInt64.self)))
        }
    }

    /// strings のセクションから、文字列を引く（UTF-8）
    public func text(start: UInt32, length: UInt32) throws -> String {
        let strings = try column("strings", .u8)
        let start = Int(start)
        let end = start + Int(length)
        guard end <= strings.count else {
            throw JSONError("strings のセクションの外を指す")
        }
        guard let text = String(data: strings.subdata(in: start..<end), encoding: .utf8) else {
            throw JSONError("strings のセクションの文字列が UTF-8 でない")
        }
        return text
    }
}
