// 適合試験を、Swift の包み（bindings/swift）を通して流す（決定 0006・0225）。
//
// 使い方: ZumenConformance [リポジトリの根]
//
// bindings/conformance/cases/*.json を、包みのセッションのメソッドで流し、状態と応答の値が期待どおりで、ハッシュの一覧が
// hashes.txt と一致することを確かめる。シーンの blob は、包みの読み手で読み、応答の数と合うかを確かめる。手順の読み方と
// 応答の確かめ方は、Python の側（bindings/python/tests/conformance.py）と同じ規則で書く。iOS のシミュレーターでも、
// 同じものを `xcrun simctl spawn` で流す。

import CryptoKit
import Foundation
import Zumen

/// 手順の種類。手順は、どれか 1 つの欄を持つ
let actions = ["session", "push", "call", "call_text", "call_hex", "take"]

/// 手順の形が違う
struct CaseError: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}

/// 境界の関数を呼び、状態の名前と、書かれたバイト列を返す
func attempt(_ call: () throws -> Data?) throws -> (status: String, data: Data?) {
    do {
        return ("ok", try call())
    } catch let error as ZumenError {
        return (error.status, error.data)
    }
}

func decodeHex(_ text: String) throws -> Data {
    let digits = Array(text.unicodeScalars.filter { !CharacterSet.whitespacesAndNewlines.contains($0) })
    guard digits.count % 2 == 0 else { throw CaseError("16 進として読めない: \(text)") }
    var out = Data()
    for index in stride(from: 0, to: digits.count, by: 2) {
        guard let byte = UInt8(String(digits[index]) + String(digits[index + 1]), radix: 16) else {
            throw CaseError("16 進として読めない: \(text)")
        }
        out.append(byte)
    }
    return out
}

/// JSON Pointer で値を引く。なければ nil
func pointer(_ value: JSON, _ path: String) -> JSON? {
    if path.isEmpty { return value }
    var current = value
    for raw in path.dropFirst().split(separator: "/", omittingEmptySubsequences: false) {
        let token = raw.replacingOccurrences(of: "~1", with: "/").replacingOccurrences(of: "~0", with: "~")
        switch current {
        case .array(let items):
            guard token.range(of: "^(0|[1-9][0-9]{0,8})$", options: .regularExpression) != nil,
                  let index = Int(token), index < items.count
            else { return nil }
            current = items[index]
        case .object(let fields):
            guard let next = fields[token] else { return nil }
            current = next
        default:
            return nil
        }
    }
    return current
}

/// JSON の値として等しいか。真偽値と数は別にし、数は値で比べる
func same(_ a: JSON, _ b: JSON) -> Bool {
    switch (a, b) {
    case (.integer(let x), .integer(let y)):
        return x == y
    case (.integer, .number), (.number, .integer), (.number, .number):
        return a.numberValue == b.numberValue
    case (.array(let x), .array(let y)):
        return x.count == y.count && zip(x, y).allSatisfy { same($0, $1) }
    case (.object(let x), .object(let y)):
        return Set(x.keys) == Set(y.keys) && x.allSatisfy { key, value in same(value, y[key]!) }
    default:
        return a == b
    }
}

func sha256(_ data: Data) -> String {
    SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}

func text(_ value: JSON) -> String {
    (try? value.text()) ?? "\(value)"
}

final class Conformance {
    let repository: URL
    let conformance: URL
    var failures: [String] = []
    var hashes: [String] = []
    var scenes = 0

    init(repository: URL) {
        self.repository = repository
        conformance = repository.appendingPathComponent("bindings/conformance")
    }

    /// session の設定のバイト列。値は JSON に直し、文字列はそのまま渡す
    func optionsOf(_ input: JSON?) throws -> Data {
        if let options = input?["options"] { return try options.data() }
        if let text = input?["options_text"]?.stringValue { return Data(text.utf8) }
        return Data()
    }

    /// 公開サンプル（決定 0171）を読む。道筋はリポジトリの根から書き、samples/ の中に限る
    func readSample(_ file: String) throws -> Data {
        let parts = file.split(separator: "/", omittingEmptySubsequences: false)
        let valid = parts.count >= 2 && parts[0] == "samples"
            && parts.dropFirst().allSatisfy { !$0.isEmpty && $0 != "." && $0 != ".." && !$0.contains("\\") }
        guard valid else { throw CaseError("push の file は samples/ の中のファイルにする: \(file)") }
        return try Data(contentsOf: repository.appendingPathComponent(file))
    }

    /// call・call_text・call_hex の手順の、要求のバイト列
    func requestOf(_ action: String, _ input: JSON) throws -> Data {
        switch action {
        case "call_text": return Data((input.stringValue ?? "").utf8)
        case "call_hex": return try decodeHex(input.stringValue ?? "")
        default: return try input.data()
        }
    }

    /// 応答の値を、手順の expect と absent で確かめる
    func checkResponse(_ at: String, _ step: [String: JSON], _ response: JSON) {
        for (path, expected) in (step["expect"]?.objectValue ?? [:]).sorted(by: { $0.key < $1.key }) {
            if let found = pointer(response, path) {
                if !same(found, expected) {
                    failures.append("\(at): \(path) が \(text(found))（期待は \(text(expected))）")
                }
            } else {
                failures.append("\(at): \(path) がない")
            }
        }
        for path in step["absent"]?.arrayValue?.compactMap(\.stringValue) ?? [] where pointer(response, path) != nil {
            failures.append("\(at): \(path) があってはいけない")
        }
    }

    /// シーンの blob を読み手で読み、応答の数と合うかを確かめる
    func checkScene(_ at: String, _ blob: Data, _ result: JSON) {
        do {
            let scene = try Scene(blob)
            let counts = result["counts"]
            func count(_ name: String) -> Int64 { counts?[name]?.integerValue ?? -1 }
            let checks: [(String, Int64, Int64)] = [
                ("大版", Int64(scene.major), Int64(Constants.sceneMajor)),
                ("区画の数", Int64(scene.sections), Int64(Constants.sceneSections.count)),
                ("塊の数", Int64(scene.chunks), count("chunks")),
                ("chunk_origins の行", Int64(try scene.rows("chunk_origins")), count("chunks")),
                ("text_xy の行", Int64(try scene.rows("text_xy")), count("texts")),
                ("point_xy の行", Int64(try scene.rows("point_xy")), count("points")),
                ("glyph_xy の行", Int64(try scene.rows("glyph_xy")), count("glyphs")),
                ("line_xy と face_xy の行", Int64(try scene.rows("line_xy") + scene.rows("face_xy")), count("vertices")),
            ]
            // 文字の文字列は、strings の区画から引ける
            let strings = try scene.u32("text_strings")
            for row in stride(from: 0, to: strings.count - 1, by: 2) {
                do {
                    _ = try scene.text(start: strings[row], length: strings[row + 1])
                } catch {
                    failures.append("\(at): text_strings の行 \(row / 2) を引けない: \(error)")
                }
            }
            for (name, found, expected) in checks where found != expected {
                failures.append("\(at): シーンの\(name)が \(found)（期待は \(expected)）")
            }
        } catch {
            failures.append("\(at): シーンを読めない: \(error)")
        }
    }

    func run() throws -> Int32 {
        let files = try FileManager.default.contentsOfDirectory(atPath: conformance.appendingPathComponent("cases").path)
            .filter { $0.hasSuffix(".json") }
            .sorted()
        for file in files {
            let name = String(file.dropLast(".json".count))
            let source = try JSON.parse(Data(contentsOf: conformance.appendingPathComponent("cases/\(file)")))
            let steps = source["steps"]?.arrayValue ?? []
            var session: Session?
            // 直前の応答が返したシーンの blob の番号と結果
            var lastScene: (blob: Int64, result: JSON)?
            for (index, value) in steps.enumerated() {
                let at = "\(name) の手順 \(index)"
                guard let step = value.objectValue, let action = actions.first(where: { step[$0] != nil }) else {
                    failures.append("\(at): 手順の種類がない")
                    continue
                }
                let expected = step["status"]?.stringValue ?? "ok"
                let input = step[action]!
                if action != "session", session == nil {
                    session = try Session()
                }
                let current = session
                var outcome: (status: String, data: Data?)
                var kind: String?
                switch action {
                case "session":
                    session?.close()
                    var opened: Session?
                    outcome = try attempt {
                        opened = try Session(optionsData: try optionsOf(input))
                        return nil
                    }
                    if let opened {
                        session = opened
                    } else {
                        // 作れなかったら、閉じたセッションで続ける。境界は、空のセッションへの呼び出しを invalid にする
                        session = try Session()
                        session?.close()
                    }
                case "push":
                    let pushName = try input["name_hex"]?.stringValue.map(decodeHex) ?? Data((input["name"]?.stringValue ?? "").utf8)
                    let data: Data
                    if let hex = input["hex"]?.stringValue {
                        data = try decodeHex(hex)
                    } else if let file = input["file"]?.stringValue {
                        data = try readSample(file)
                    } else {
                        data = Data((input["text"]?.stringValue ?? "").utf8)
                    }
                    outcome = try attempt {
                        try current!.push(nameData: pushName, data)
                        return nil
                    }
                case "take":
                    let blob = input.integerValue ?? -1
                    outcome = try attempt { try current!.take(UInt32(truncatingIfNeeded: blob)) }
                    kind = outcome.data != nil ? "blob" : nil
                    if outcome.status == "ok", let data = outcome.data, let scene = lastScene, scene.blob == blob {
                        checkScene(at, data, scene.result)
                        scenes += 1
                    }
                default:
                    let request = try requestOf(action, input)
                    outcome = try attempt { try current!.callData(request) }
                    kind = outcome.data != nil ? "response" : nil
                }
                if outcome.status != expected {
                    failures.append("\(at): 状態が \(outcome.status)（期待は \(expected)）")
                }
                guard let kind, let data = outcome.data else { continue }
                hashes.append("\(name) \(index) \(kind) \(sha256(data))")
                guard kind == "response" else { continue }
                let response = try JSON.parse(data)
                checkResponse(at, step, response)
                lastScene = nil
                if let result = response["result"], result.objectValue != nil,
                   let blob = response["blobs"]?.arrayValue?.first(where: { $0["kind"] == .string("scene") }),
                   let id = blob["id"]?.integerValue {
                    lastScene = (id, result)
                }
            }
            // 動いている呼び出しがないので、何もしない
            session?.cancel()
            session?.close()
        }

        try compareHashes()
        try checkTypedCalls()

        let summary = "包みを通した試験の組 \(files.count) 件、ハッシュ \(hashes.count) 件、読んだシーン \(scenes) 件"
        print(summary)
        if let path = ProcessInfo.processInfo.environment["GITHUB_STEP_SUMMARY"], !path.isEmpty,
           let handle = FileHandle(forWritingAtPath: path) {
            #if os(iOS)
            let platform = "iOS のシミュレーター"
            #else
            let platform = "macOS"
            #endif
            handle.seekToEndOfFile()
            handle.write(Data("### Swift の包みの適合試験（\(platform)）\n\n\(summary)\n".utf8))
            handle.closeFile()
        }
        if scenes == 0 {
            failures.append("シーンの blob を 1 つも読んでいない")
        }
        if !failures.isEmpty {
            FileHandle.standardError.write(Data((["期待と違った:"] + failures).joined(separator: "\n").utf8 + [0x0a]))
            return 1
        }
        print("ネイティブと同じ一覧になった")
        return 0
    }

    /// ハッシュの一覧が hashes.txt と一致するかを確かめる
    func compareHashes() throws {
        let text = try String(contentsOf: conformance.appendingPathComponent("hashes.txt"), encoding: .utf8)
        let expected = text.split(separator: "\n").map(String.init).filter { !$0.hasPrefix("#") }
        if hashes != expected {
            var message = "ハッシュの一覧が hashes.txt と合わない:"
            for line in expected where !hashes.contains(line) { message += "\n- \(line)" }
            for line in hashes where !expected.contains(line) { message += "\n+ \(line)" }
            failures.append(message)
        }
    }

    /// 型を渡す call と、失敗した call と、閉じたあとの呼び出し
    func checkTypedCalls() throws {
        let session = try Session(options: SessionOptions().with { $0.v = Constants.schemaMajor })
        do {
            let response = try session.call(CapabilitiesRequest().with { $0.v = Constants.schemaMajor })
            let result = response.result(as: CapabilitiesResult.self)
            if response.v != Constants.schemaMajor || result?.package == nil {
                failures.append("call の応答が capabilities の形でない: \(response.jsonText)")
            }
        }
        do {
            _ = try session.call(OpenRequest().with {
                $0.v = Constants.schemaMajor
                $0.input = "none.dxf"
            })
            failures.append("ない入力を開く call が失敗しない")
        } catch let error as ZumenError {
            if error.status != "failed" || (error.response?.diagnostics ?? []).isEmpty {
                failures.append("ない入力を開く call の失敗が、診断のある failed でない: \(error)")
            }
        }
        // ほかのスレッドから cancel を呼べる（動いている呼び出しがなければ、何もしない）
        let done = DispatchSemaphore(value: 0)
        Thread {
            session.cancel()
            done.signal()
        }.start()
        done.wait()
        session.close()
        let request = try CapabilitiesRequest().with { $0.v = Constants.schemaMajor }.jsonValue.data()
        let outcome = try attempt { try session.callData(request) }
        if !session.isClosed || outcome.status != "invalid" {
            failures.append("閉じたあとの call が \(outcome.status)（期待は invalid）")
        }
    }
}

/// 渡されなければ、今の場所から上にたどって、bindings/conformance のあるフォルダーを根にする
func findRepository(_ arguments: [String]) throws -> URL {
    if arguments.count >= 2 {
        return URL(fileURLWithPath: arguments[1]).standardizedFileURL
    }
    var current = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
    while current.path != "/" {
        if FileManager.default.fileExists(atPath: current.appendingPathComponent("bindings/conformance/hashes.txt").path) {
            return current
        }
        current.deleteLastPathComponent()
    }
    throw CaseError("リポジトリの根が見つからない。引数で渡す")
}

let arguments = CommandLine.arguments
guard arguments.count <= 2 else {
    FileHandle.standardError.write(Data("使い方: ZumenConformance [リポジトリの根]\n".utf8))
    exit(2)
}
do {
    exit(try Conformance(repository: try findRepository(arguments)).run())
} catch {
    FileHandle.standardError.write(Data("試験を流せない: \(error)\n".utf8))
    exit(1)
}
