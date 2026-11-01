// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。
//
// 境界のメッセージの型（決定 0012・0225）。スキーマの定義ごとに 1 つの型がある。型は JSON のオブジェクトを包む
// 見方で、知らない欄もそのまま持つ。列挙は文字列で読み、知らない値も受け取る（決定 0015）。

/// 要求。op の値で形が決まる
public struct Request: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// op が capabilities の形なら、その見方。違えば nil
    public var asCapabilitiesRequest: CapabilitiesRequest? {
        taggedView("op", "capabilities")
    }

    /// op が open の形なら、その見方。違えば nil
    public var asOpenRequest: OpenRequest? {
        taggedView("op", "open")
    }

    /// op が close の形なら、その見方。違えば nil
    public var asCloseRequest: CloseRequest? {
        taggedView("op", "close")
    }

    /// op が describe の形なら、その見方。違えば nil
    public var asDescribeRequest: DescribeRequest? {
        taggedView("op", "describe")
    }

    /// op が drawing の形なら、その見方。違えば nil
    public var asDrawingRequest: DrawingRequest? {
        taggedView("op", "drawing")
    }

    /// op が entities の形なら、その見方。違えば nil
    public var asEntitiesRequest: EntitiesRequest? {
        taggedView("op", "entities")
    }

    /// op が apply の形なら、その見方。違えば nil
    public var asApplyRequest: ApplyRequest? {
        taggedView("op", "apply")
    }

    /// op が convert の形なら、その見方。違えば nil
    public var asConvertRequest: ConvertRequest? {
        taggedView("op", "convert")
    }

    /// op が tables の形なら、その見方。違えば nil
    public var asTablesRequest: TablesRequest? {
        taggedView("op", "tables")
    }

    /// op が scene の形なら、その見方。違えば nil
    public var asSceneRequest: SceneRequest? {
        taggedView("op", "scene")
    }

    /// op が check の形なら、その見方。違えば nil
    public var asCheckRequest: CheckRequest? {
        taggedView("op", "check")
    }

    /// op が validate の形なら、その見方。違えば nil
    public var asValidateRequest: ValidateRequest? {
        taggedView("op", "validate")
    }

    /// op が review_carry の形なら、その見方。違えば nil
    public var asReviewCarryRequest: ReviewCarryRequest? {
        taggedView("op", "review_carry")
    }

    /// op が review_adopt の形なら、その見方。違えば nil
    public var asReviewAdoptRequest: ReviewAdoptRequest? {
        taggedView("op", "review_adopt")
    }

    /// op が building の形なら、その見方。違えば nil
    public var asBuildingRequest: BuildingRequest? {
        taggedView("op", "building")
    }

    /// op が building_elements の形なら、その見方。違えば nil
    public var asBuildingElementsRequest: BuildingElementsRequest? {
        taggedView("op", "building_elements")
    }

    /// op が ids_check の形なら、その見方。違えば nil
    public var asIdsCheckRequest: IdsCheckRequest? {
        taggedView("op", "ids_check")
    }

    /// op が review_adopt_ids の形なら、その見方。違えば nil
    public var asReviewAdoptIdsRequest: ReviewAdoptIdsRequest? {
        taggedView("op", "review_adopt_ids")
    }

    /// op が bcf_read の形なら、その見方。違えば nil
    public var asBcfReadRequest: BcfReadRequest? {
        taggedView("op", "bcf_read")
    }

    /// op が bcf_write の形なら、その見方。違えば nil
    public var asBcfWriteRequest: BcfWriteRequest? {
        taggedView("op", "bcf_write")
    }

    /// op が civil の形なら、その見方。違えば nil
    public var asCivilRequest: CivilRequest? {
        taggedView("op", "civil")
    }

    /// op が pointcloud の形なら、その見方。違えば nil
    public var asPointcloudRequest: PointcloudRequest? {
        taggedView("op", "pointcloud")
    }

    /// op が deviation の形なら、その見方。違えば nil
    public var asDeviationRequest: DeviationRequest? {
        taggedView("op", "deviation")
    }

    /// op が civil_drawing の形なら、その見方。違えば nil
    public var asCivilDrawingRequest: CivilDrawingRequest? {
        taggedView("op", "civil_drawing")
    }

    /// op が geo の形なら、その見方。違えば nil
    public var asGeoRequest: GeoRequest? {
        taggedView("op", "geo")
    }

    /// op が delivery_check の形なら、その見方。違えば nil
    public var asDeliveryCheckRequest: DeliveryCheckRequest? {
        taggedView("op", "delivery_check")
    }
}

/// どの op の要求にも置ける欄
public struct RequestCommon: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }
}

/// 予算の単位ごとの上限（決定 0010）
public struct Budget: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 受け取ったバイト数
    public var inputBytes: JSON? {
        get { readField("input_bytes") }
        set { writeField("input_bytes", newValue) }
    }

    /// 展開・復号で作ったバイト数。入れ子の分も足す
    public var decodedBytes: JSON? {
        get { readField("decoded_bytes") }
        set { writeField("decoded_bytes", newValue) }
    }

    /// L0・L1 で読んだレコードの数
    public var records: JSON? {
        get { readField("records") }
        set { writeField("records", newValue) }
    }

    /// L2・L3 で作った要素の数
    public var elements: JSON? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }

    /// 入れ子と再帰の深さ
    public var depth: JSON? {
        get { readField("depth") }
        set { writeField("depth", newValue) }
    }

    /// 幾何の分割で作った点・線・面の数
    public var segments: JSON? {
        get { readField("segments") }
        set { writeField("segments", newValue) }
    }

    /// ほかの単位に入らない繰り返しの回数
    public var work: JSON? {
        get { readField("work") }
        set { writeField("work", newValue) }
    }
}

/// コアの版と、使える op・形式を尋ねる
public struct CapabilitiesRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("capabilities")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }
}

/// コアの版と、使える op・形式（決定 0015）
public struct CapabilitiesResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// パッケージの版（semver）
    public var `package`: String? {
        get { readField("package") }
        set { writeField("package", newValue) }
    }

    /// ABI の大版。zm_abi_version と同じ
    public var abi: Int64? {
        get { readField("abi") }
        set { writeField("abi", newValue) }
    }

    /// メッセージのスキーマの版
    public var schema: SchemaVersion? {
        get { readField("schema") }
        set { writeField("schema", newValue) }
    }

    /// このコア（wasm ではこのモジュール）で使える op の名前
    public var ops: [String]? {
        get { readField("ops") }
        set { writeField("ops", newValue) }
    }

    /// 読み書きできる形式
    public var formats: [Format]? {
        get { readField("formats") }
        set { writeField("formats", newValue) }
    }
}

/// メッセージのスキーマの版。大版は要求と応答の v、小版は op・欄・列挙の値を足すたびに上げる
public struct SchemaVersion: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 大版
    public var major: Int64? {
        get { readField("major") }
        set { writeField("major", newValue) }
    }

    /// 小版
    public var minor: Int64? {
        get { readField("minor") }
        set { writeField("minor", newValue) }
    }
}

/// 読み書きできる形式
public struct Format: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 形式の名前（dxf・sfc など）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 形式の系統（autocad・sxf など、決定 0008）
    public var family: String? {
        get { readField("family") }
        set { writeField("family", newValue) }
    }

    /// 読めるか
    public var read: Bool? {
        get { readField("read") }
        set { writeField("read", newValue) }
    }

    /// 書けるか
    public var write: Bool? {
        get { readField("write") }
        set { writeField("write", newValue) }
    }

    /// この形式の ID の規則の版（決定 0015）
    public var idRules: Int64? {
        get { readField("id_rules") }
        set { writeField("id_rules", newValue) }
    }
}

/// 入力を読んで、文書を作る。形式は入力から判定する
public struct OpenRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("open")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 読む入力の名前。zm_push で渡したもの
    public var input: String? {
        get { readField("input") }
        set { writeField("input", newValue) }
    }

    /// 作る文書の名前。書かなければ入力の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。小版 48 から
    public var size: Int64? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }

    /// 見分ける印のない形式（xyz）を名指して読む。書かなければ、入力から判定する（決定 0232）。小版 49 から
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }
}

/// 作った文書
public struct OpenResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 判定した形式の名前
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。小版 48 から
    public var needs: [InputRange]? {
        get { readField("needs") }
        set { writeField("needs", newValue) }
    }

    /// 読み終えたので、close の inputs で捨ててよい範囲の入力。小版 48 から
    public var done: [InputRange]? {
        get { readField("done") }
        set { writeField("done", newValue) }
    }

    /// SXF を ZIP に入れた書庫（P2Z・SFZ）なら、SXF の項目のほかの項目（属性ファイル・画像など）の書庫の中の名前。書庫の順。apply は、SXF の項目だけを置き換え、ほかの項目を写した書庫を書く（#385）。小版 54 から
    public var entries: [String]? {
        get { readField("entries") }
        set { writeField("entries", newValue) }
    }
}

/// 入力の範囲と、その範囲を渡す入力の名前
public struct InputRange: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 範囲を渡す入力の名前。<入力の名前>#<start>-<end>
    public var input: String? {
        get { readField("input") }
        set { writeField("input", newValue) }
    }

    /// 範囲の初め（バイト）
    public var start: Int64? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 範囲の終わり（バイト、含まない）
    public var end: Int64? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }
}

/// 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
public struct CloseRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("close")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 捨てる文書の名前
    public var docs: [String]? {
        get { readField("docs") }
        set { writeField("docs", newValue) }
    }

    /// 捨てる入力の名前
    public var inputs: [String]? {
        get { readField("inputs") }
        set { writeField("inputs", newValue) }
    }
}

/// 捨てたこと。中身はない
public struct CloseResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }
}

/// 診断の文を、カタログから組み立てる（決定 0009）
public struct DescribeRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("describe")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// カタログの入力の名前。zm_push で渡したもの。書かなければ、code と値をそのまま文にする
    public var catalog: String? {
        get { readField("catalog") }
        set { writeField("catalog", newValue) }
    }

    /// 文にする診断。応答で受け取ったもの
    public var diagnostics: [Diagnostic]? {
        get { readField("diagnostics") }
        set { writeField("diagnostics", newValue) }
    }
}

/// 診断の文
public struct DescribeResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 診断ごとの文。要求の diagnostics と同じ順。カタログにない code は、code と値をそのまま書く
    public var texts: [String]? {
        get { readField("texts") }
        set { writeField("texts", newValue) }
    }
}

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合試験で比べるためにも使う（決定 0007）
public struct DrawingRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("drawing")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で作ったもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }
}

/// 図面モデルの要約。移さなかった図形と値は、診断で返す
public struct DrawingResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 文書のハッシュ。open に渡したバイト列から作る。変更の一覧の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 実寸の空間。モデルの順
    public var spaces: [SpaceSummary]? {
        get { readField("spaces") }
        set { writeField("spaces", newValue) }
    }

    /// 用紙。モデルの順
    public var sheets: [SheetSummary]? {
        get { readField("sheets") }
        set { writeField("sheets", newValue) }
    }

    /// 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。小版 39 から
    public var pointStyle: PointStyleSummary? {
        get { readField("point_style") }
        set { writeField("point_style", newValue) }
    }
}

/// 文書の点の表し方。欄の意味は point_style_edit と同じ
public struct PointStyleSummary: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 基本の形
    public var mark: String? {
        get { readField("mark") }
        set { writeField("mark", newValue) }
    }

    /// 円で囲む
    public var circle: Bool? {
        get { readField("circle") }
        set { writeField("circle", newValue) }
    }

    /// 四角で囲む
    public var square: Bool? {
        get { readField("square") }
        set { writeField("square", newValue) }
    }

    /// 大きさ。意味は size_kind で決まる
    public var size: Double? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }

    /// 大きさの意味
    public var sizeKind: String? {
        get { readField("size_kind") }
        set { writeField("size_kind", newValue) }
    }
}

/// 実寸の空間の要約
public struct SpaceSummary: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 空間の ID（space/0）。entities の要素の ID の始まりと同じ（決定 0156）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 文書の単位
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// 要素の数
    public var elements: ElementCounts? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }

    /// 線分の端点と文字の基点を囲む範囲。文書の単位。要素がなければ持たない
    public var extent: Extent? {
        get { readField("extent") }
        set { writeField("extent", newValue) }
    }
}

/// 用紙の要約
public struct SheetSummary: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 用紙の ID。LAYOUT がハンドルを持てば sheet/h22、なければ読んだ順の番号（sheet/0）。entities の要素の ID の始まりと同じ（決定 0156）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 元の形式の用紙の名前（DXF・DWG のレイアウトの名前）。翻訳しない。なければ持たない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 用紙の幅と高さ（mm）。分からなければ持たない
    public var size: [Double]? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }

    /// 要素の数
    public var elements: ElementCounts? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }

    /// ビューの数
    public var views: Int64? {
        get { readField("views") }
        set { writeField("views", newValue) }
    }

    /// 線分の端点と文字の基点を囲む範囲。用紙の mm。要素がなければ持たない
    public var extent: Extent? {
        get { readField("extent") }
        set { writeField("extent", newValue) }
    }
}

/// 実寸の単位
///
/// 列挙の値。知らない値も受け取る
public enum Unit {
    public static let millimeter = "millimeter"
    public static let centimeter = "centimeter"
    public static let meter = "meter"
    public static let inch = "inch"
    public static let foot = "foot"
    public static let unknown = "unknown"
}

/// 種類ごとの要素の数
public struct ElementCounts: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 線分
    public var lines: Int64? {
        get { readField("lines") }
        set { writeField("lines", newValue) }
    }

    /// 文字
    public var texts: Int64? {
        get { readField("texts") }
        set { writeField("texts", newValue) }
    }
}

/// 点を囲む、軸に沿った長方形
public struct Extent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// X の最小
    public var minX: Double? {
        get { readField("min_x") }
        set { writeField("min_x", newValue) }
    }

    /// Y の最小
    public var minY: Double? {
        get { readField("min_y") }
        set { writeField("min_y", newValue) }
    }

    /// X の最大
    public var maxX: Double? {
        get { readField("max_x") }
        set { writeField("max_x", newValue) }
    }

    /// Y の最大
    public var maxY: Double? {
        get { readField("max_y") }
        set { writeField("max_y", newValue) }
    }
}

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）
public struct EntitiesRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("entities")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で作ったもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す要素の数の上限。書かなければ 1000
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }

    /// 返す欄の名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118）
    public var fields: [String]? {
        get { readField("fields") }
        set { writeField("fields", newValue) }
    }

    /// [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118）
    public var region: [Double]? {
        get { readField("region") }
        set { writeField("region", newValue) }
    }

    /// 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。小版 31 から（決定 0177）
    public var place: String? {
        get { readField("place") }
        set { writeField("place", newValue) }
    }
}

/// 要素の一覧の一部。移さなかった図形と値は、診断で返す
public struct EntitiesResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// offset から limit 個までの要素
    public var items: [Entity]? {
        get { readField("items") }
        set { writeField("items", newValue) }
    }

    /// 全体の要素の数
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 続きの最初の要素の位置。続きがなければ持たない
    public var nextOffset: Int64? {
        get { readField("next_offset") }
        set { writeField("next_offset", newValue) }
    }
}

/// 図面モデルの要素
public struct Entity: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つ鍵（ハンドル）があれば鍵で作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どの版でも同じ ID を返す（決定 0117・0156）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 形の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// レイヤの元の名前。翻訳しない。図面モデルにレイヤがなければ持たない
    public var layer: String? {
        get { readField("layer") }
        set { writeField("layer", newValue) }
    }

    /// 元の形式のレコード
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }

    /// 形の定義から求めた範囲。文字の字形・線の太さ・記号の中身は含めない。囲む点がなければ持たない（決定 0118）
    public var bbox: Extent? {
        get { readField("bbox") }
        set { writeField("bbox", newValue) }
    }

    /// 要素のレイヤ・スタイル・形のハッシュ。出所の位置は含めない。変更の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点か当てはめる点で決まるスプライン・寸法・引出線（LEADER）だけが持つ。apply の replace と add に渡せる形（決定 0139）。小版 10 から
    public var geometry: Geometry? {
        get { readField("geometry") }
        set { writeField("geometry", newValue) }
    }
}

/// entities で返す要素の欄（決定 0118）
///
/// 列挙の値。知らない値も受け取る
public enum EntityField {
    public static let id = "id"
    public static let kind = "kind"
    public static let layer = "layer"
    public static let source = "source"
    public static let bbox = "bbox"
    public static let hash = "hash"
    public static let geometry = "geometry"
}

/// 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる
public struct Geometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// shape が line の形なら、その見方。違えば nil
    public var asLineGeometry: LineGeometry? {
        taggedView("shape", "line")
    }

    /// shape が circle の形なら、その見方。違えば nil
    public var asCircleGeometry: CircleGeometry? {
        taggedView("shape", "circle")
    }

    /// shape が arc の形なら、その見方。違えば nil
    public var asArcGeometry: ArcGeometry? {
        taggedView("shape", "arc")
    }

    /// shape が ellipse の形なら、その見方。違えば nil
    public var asEllipseGeometry: EllipseGeometry? {
        taggedView("shape", "ellipse")
    }

    /// shape が polyline の形なら、その見方。違えば nil
    public var asPolylineGeometry: PolylineGeometry? {
        taggedView("shape", "polyline")
    }

    /// shape が text の形なら、その見方。違えば nil
    public var asTextGeometry: TextGeometry? {
        taggedView("shape", "text")
    }

    /// shape が formatted_text の形なら、その見方。違えば nil
    public var asFormattedTextGeometry: FormattedTextGeometry? {
        taggedView("shape", "formatted_text")
    }

    /// shape が symbol の形なら、その見方。違えば nil
    public var asSymbolGeometry: SymbolGeometry? {
        taggedView("shape", "symbol")
    }

    /// shape が spline の形なら、その見方。違えば nil
    public var asSplineGeometry: SplineGeometry? {
        taggedView("shape", "spline")
    }

    /// shape が dimension の形なら、その見方。違えば nil
    public var asDimensionGeometry: DimensionGeometry? {
        taggedView("shape", "dimension")
    }

    /// shape が leader の形なら、その見方。違えば nil
    public var asLeaderGeometry: LeaderGeometry? {
        taggedView("shape", "leader")
    }

    /// shape が point の形なら、その見方。違えば nil
    public var asPointGeometry: PointGeometry? {
        taggedView("shape", "point")
    }

    /// shape が callout の形なら、その見方。違えば nil
    public var asCalloutGeometry: CalloutGeometry? {
        taggedView("shape", "callout")
    }
}

/// 線分
public struct LineGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("line")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 始点 [X, Y]
    public var start: [Double]? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 終点 [X, Y]
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }
}

/// 円
public struct CircleGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("circle")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 中心 [X, Y]
    public var center: [Double]? {
        get { readField("center") }
        set { writeField("center", newValue) }
    }

    /// 半径。0 より大きい
    public var radius: Double? {
        get { readField("radius") }
        set { writeField("radius", newValue) }
    }
}

/// 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる
public struct ArcGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("arc")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 中心 [X, Y]
    public var center: [Double]? {
        get { readField("center") }
        set { writeField("center", newValue) }
    }

    /// 半径。0 より大きい
    public var radius: Double? {
        get { readField("radius") }
        set { writeField("radius", newValue) }
    }

    /// 始まりの角度（度）
    public var startAngle: Double? {
        get { readField("start_angle") }
        set { writeField("start_angle", newValue) }
    }

    /// 終わりの角度（度）
    public var endAngle: Double? {
        get { readField("end_angle") }
        set { writeField("end_angle", newValue) }
    }
}

/// 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。小版 11 から
public struct EllipseGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("ellipse")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 中心 [X, Y]
    public var center: [Double]? {
        get { readField("center") }
        set { writeField("center", newValue) }
    }

    /// 長軸の端の点 [X, Y]。中心と違う点
    public var major: [Double]? {
        get { readField("major") }
        set { writeField("major", newValue) }
    }

    /// 短軸の長軸に対する比。0 より大きく 1 以下
    public var ratio: Double? {
        get { readField("ratio") }
        set { writeField("ratio", newValue) }
    }

    /// 始まりの角度（度）。楕円を円に引き延ばしたときの角度
    public var startAngle: Double? {
        get { readField("start_angle") }
        set { writeField("start_angle", newValue) }
    }

    /// 終わりの角度（度）。楕円を円に引き延ばしたときの角度
    public var endAngle: Double? {
        get { readField("end_angle") }
        set { writeField("end_angle", newValue) }
    }
}

/// 頂点を結んだ折れ線。幅は持たない
public struct PolylineGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("polyline")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 頂点の [X, Y] の並び。2 つ以上
    public var points: [[Double]]? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 頂点ごとの、次の頂点への弧の膨らみ（中心角の 4 分の 1 の正接。正は反時計回り）。points と同じ数。書かなければどれも 0
    public var bulges: [Double]? {
        get { readField("bulges") }
        set { writeField("bulges", newValue) }
    }

    /// 最後の頂点から最初の頂点へ結ぶ
    public var closed: Bool? {
        get { readField("closed") }
        set { writeField("closed", newValue) }
    }
}

/// 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせ・幅の倍率・斜体・反転・文字スタイルを元の値のままにする。足すときは、2 点目がなければ左の基準線に合わせ、幅の倍率 1・斜体なし・反転なしにする
public struct TextGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("text")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 内容。1 行
    public var content: String? {
        get { readField("content") }
        set { writeField("content", newValue) }
    }

    /// 基点 [X, Y]。2 点の間に合わせる文字では 1 点目
    public var position: [Double]? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。小版 17 から
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 2 点の間への合わせ方。2 点目（end）と一緒に書く。小版 17 から
    public var fit: String? {
        get { readField("fit") }
        set { writeField("fit", newValue) }
    }

    /// 高さ。0 より大きい
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない
    public var rotation: Double? {
        get { readField("rotation") }
        set { writeField("rotation", newValue) }
    }

    /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。小版 12 から
    public var style: String? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }

    /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
    public var styleId: String? {
        get { readField("style_id") }
        set { writeField("style_id", newValue) }
    }
}

/// 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
///
/// 列挙の値。知らない値も受け取る
public enum TextFit {
    public static let aligned = "aligned"
    public static let fit = "fit"
}

/// 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味・文字スタイルを元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。小版 13 から
public struct FormattedTextGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("formatted_text")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 内容。書式の符号（\P の改行など）を含めた、元の形式の内容のまま
    public var content: String? {
        get { readField("content") }
        set { writeField("content", newValue) }
    }

    /// 基点 [X, Y]
    public var position: [Double]? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 高さ。0 より大きい
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 回転（度）
    public var rotation: Double? {
        get { readField("rotation") }
        set { writeField("rotation", newValue) }
    }

    /// 基点に合わせる枠の位置
    public var attachment: String? {
        get { readField("attachment") }
        set { writeField("attachment", newValue) }
    }

    /// 折り返す枠の幅。0 より大きい。書かなければ折り返さない
    public var width: Double? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す
    public var style: String? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }

    /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
    public var styleId: String? {
        get { readField("style_id") }
        set { writeField("style_id", newValue) }
    }
}

/// スプライン（NURBS）。制御点で決まるもの（小版 15 から）と、当てはめる点で決まるもの（小版 18 から）がある。当てはめる点で決まるものは、節点と制御点を空にし、当てはめる点（fit）と、あれば両端の接線を持つ。元の図形に制御点や節点もあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない
public struct SplineGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("spline")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 次数。1 以上
    public var degree: Int64? {
        get { readField("degree") }
        set { writeField("degree", newValue) }
    }

    /// 節点の並び。周期的でなければ、制御点の数と次数と 1 の和の数。当てはめる点で決まるものは空
    public var knots: [Double]? {
        get { readField("knots") }
        set { writeField("knots", newValue) }
    }

    /// 制御点の [X, Y] の並び。次数より多い。当てはめる点で決まるものは空
    public var control: [[Double]]? {
        get { readField("control") }
        set { writeField("control", newValue) }
    }

    /// 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない
    public var weights: [Double]? {
        get { readField("weights") }
        set { writeField("weights", newValue) }
    }

    /// 当てはめる点の [X, Y] の並び。2 つ以上。書けば、節点と制御点は空で、重みは書かない。小版 18 から
    public var fit: [[Double]]? {
        get { readField("fit") }
        set { writeField("fit", newValue) }
    }

    /// 当てはめる点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
    public var startTangent: [Double]? {
        get { readField("start_tangent") }
        set { writeField("start_tangent", newValue) }
    }

    /// 当てはめる点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
    public var endTangent: [Double]? {
        get { readField("end_tangent") }
        set { writeField("end_tangent", newValue) }
    }

    /// 閉じている
    public var closed: Bool? {
        get { readField("closed") }
        set { writeField("closed", newValue) }
    }

    /// 周期的
    public var periodic: Bool? {
        get { readField("periodic") }
        set { writeField("periodic", newValue) }
    }
}

/// 寸法。種類（kind）ごとに、その種類の定義点の欄を持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。entities が返すだけで、置き換えと追加は change.edit.unsupported になる（寸法を描いた部品を作り直す必要があるため）。小版 19 から
public struct DimensionGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("dimension")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 寸法の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 1 本目の補助線の点か、測る角度の 1 本目の線の点か、円弧の始めの点 [X, Y]。linear・aligned・angular・arc_length が持つ
    public var first: [Double]? {
        get { readField("first") }
        set { writeField("first", newValue) }
    }

    /// 2 本目の補助線の点か、測る角度の 2 本目の線の点か、円弧の終わりの点 [X, Y]。linear・aligned・angular・arc_length が持つ
    public var second: [Double]? {
        get { readField("second") }
        set { writeField("second", newValue) }
    }

    /// 寸法線が通る点 [X, Y]。linear・aligned が持つ
    public var line: [Double]? {
        get { readField("line") }
        set { writeField("line", newValue) }
    }

    /// 測る向き（度）。linear が持つ
    public var rotation: Double? {
        get { readField("rotation") }
        set { writeField("rotation", newValue) }
    }

    /// 補助線の傾き（度）。傾けなければ 0。linear・aligned が持つ
    public var oblique: Double? {
        get { readField("oblique") }
        set { writeField("oblique", newValue) }
    }

    /// 角度の頂点 [X, Y]。angular が持つ
    public var vertex: [Double]? {
        get { readField("vertex") }
        set { writeField("vertex", newValue) }
    }

    /// 1 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
    public var firstLine: [[Double]]? {
        get { readField("first_line") }
        set { writeField("first_line", newValue) }
    }

    /// 2 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
    public var secondLine: [[Double]]? {
        get { readField("second_line") }
        set { writeField("second_line", newValue) }
    }

    /// 寸法の弧が通る点 [X, Y]。angular・angular_lines・arc_length が持つ
    public var arc: [Double]? {
        get { readField("arc") }
        set { writeField("arc", newValue) }
    }

    /// 中心 [X, Y]。arc_length・radius が持つ
    public var center: [Double]? {
        get { readField("center") }
        set { writeField("center", newValue) }
    }

    /// 円の上の点 [X, Y]。radius・diameter が持つ
    public var chord: [Double]? {
        get { readField("chord") }
        set { writeField("chord", newValue) }
    }

    /// chord と向かい合う円の上の点 [X, Y]。diameter が持つ
    public var opposite: [Double]? {
        get { readField("opposite") }
        set { writeField("opposite", newValue) }
    }

    /// 引出線の長さ。radius・diameter が持つ
    public var leader: Double? {
        get { readField("leader") }
        set { writeField("leader", newValue) }
    }

    /// 原点 [X, Y]。ordinate が持つ
    public var origin: [Double]? {
        get { readField("origin") }
        set { writeField("origin", newValue) }
    }

    /// 測る点 [X, Y]。ordinate が持つ
    public var feature: [Double]? {
        get { readField("feature") }
        set { writeField("feature", newValue) }
    }

    /// 引出線の終点 [X, Y]。ordinate が持つ
    public var leaderEnd: [Double]? {
        get { readField("leader_end") }
        set { writeField("leader_end", newValue) }
    }

    /// 測る座標の軸。ordinate が持つ
    public var axis: String? {
        get { readField("axis") }
        set { writeField("axis", newValue) }
    }

    /// 寸法値の文字の書き方
    public var label: String? {
        get { readField("label") }
        set { writeField("label", newValue) }
    }

    /// 利用者が書いた寸法値の文字の元の内容。label が override のときだけ持つ。測定値に置き換える符号（DXF の <>）と書式の符号もそのまま持つ
    public var content: String? {
        get { readField("content") }
        set { writeField("content", newValue) }
    }

    /// 元の形式が保存した測定値。単位と倍率は measurement_key の欄に従う
    public var measurement: Double? {
        get { readField("measurement") }
        set { writeField("measurement", newValue) }
    }

    /// measurement の形式と欄を指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ
    public var measurementKey: String? {
        get { readField("measurement_key") }
        set { writeField("measurement_key", newValue) }
    }

    /// 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない
    public var textPosition: [Double]? {
        get { readField("text_position") }
        set { writeField("text_position", newValue) }
    }

    /// 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない
    public var textRotation: Double? {
        get { readField("text_rotation") }
        set { writeField("text_rotation", newValue) }
    }

    /// 寸法スタイルの元の名前
    public var style: String? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }

    /// 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。小版 23 から
    public var styleId: String? {
        get { readField("style_id") }
        set { writeField("style_id", newValue) }
    }
}

/// 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（小版 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印の印を必ず持つので、arrow を書かない引出線は書けない（小版 36 から。それまでは R2004 までの置き換えだけ）。小版 31 から
public struct LeaderGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("leader")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 経路の頂点 [[X, Y], ...]。矢印の側から並べる。2 つ以上
    public var path: [[Double]]? {
        get { readField("path") }
        set { writeField("path", newValue) }
    }

    /// 頂点を通る曲線で結ぶか。偽なら折れ線
    public var spline: Bool? {
        get { readField("spline") }
        set { writeField("spline", newValue) }
    }

    /// 始点に矢印を付けるか。書かなければ寸法スタイルに従う
    public var arrow: Bool? {
        get { readField("arrow") }
        set { writeField("arrow", newValue) }
    }

    /// 注記の要素の ID。同じ空間か用紙の要素で、entities が返したもの。注記がなければ書かない。置き換えるときは元と同じ ID だけを受ける。足すときは、同じ空間か用紙の書式つきの文字（MTEXT）を指す
    public var annotation: String? {
        get { readField("annotation") }
        set { writeField("annotation", newValue) }
    }

    /// 寸法スタイルの元の名前。足すときは図面にある寸法スタイルの名前を指す。置き換えるときは元と同じ名前だけを受ける
    public var style: String? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }

    /// 寸法スタイルの ID。tables が返したもの。entities は、名前で引いた寸法スタイルの ID を返す。足す・置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける
    public var styleId: String? {
        get { readField("style_id") }
        set { writeField("style_id", newValue) }
    }
}

/// 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかの欄は元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。小版 36 から
public struct CalloutGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("callout")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 枝。入力の順
    public var branches: [CalloutBranch]? {
        get { readField("branches") }
        set { writeField("branches", newValue) }
    }

    /// 線の引き方
    public var path: String? {
        get { readField("path") }
        set { writeField("path", newValue) }
    }

    /// 線を注記に、水平の線（ランディング）でつなぐか
    public var landing: Bool? {
        get { readField("landing") }
        set { writeField("landing", newValue) }
    }

    /// 端部記号の大きさ
    public var arrowSize: Double? {
        get { readField("arrow_size") }
        set { writeField("arrow_size", newValue) }
    }

    /// 大きさに掛ける倍率
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }

    /// 注記の種類
    public var content: String? {
        get { readField("content") }
        set { writeField("content", newValue) }
    }

    /// 注記の書式つきの文字の元の内容。content が text のときだけ持つ
    public var text: String? {
        get { readField("text") }
        set { writeField("text", newValue) }
    }

    /// 注記の記号のブロックの名前。content が symbol のときだけ持つ
    public var symbol: String? {
        get { readField("symbol") }
        set { writeField("symbol", newValue) }
    }
}

/// 引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる
public struct CalloutBranch: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 線ごとの頂点 [[[X, Y], ...], ...]。矢印の側から並べ、集まる点は含めない。線ごとに 1 つ以上
    public var lines: [[[Double]]]? {
        get { readField("lines") }
        set { writeField("lines", newValue) }
    }

    /// 線が集まる点 [X, Y]。元の形式が持たなければ書かない
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 集まる点から注記への折れ曲がり。なければ書かない
    public var dogleg: CalloutDogleg? {
        get { readField("dogleg") }
        set { writeField("dogleg", newValue) }
    }
}

/// 枝の折れ曲がり
public struct CalloutDogleg: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 向き [X, Y]。長さは意味を持たない
    public var direction: [Double]? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// 長さ
    public var length: Double? {
        get { readField("length") }
        set { writeField("length", newValue) }
    }
}

/// 引出線つきの注記の線の引き方。straight は折れ線、spline は頂点を通る曲線、hidden は線を描かない
///
/// 列挙の値。知らない値も受け取る
public enum CalloutPath {
    public static let straight = "straight"
    public static let spline = "spline"
    public static let hidden = "hidden"
}

/// 引出線つきの注記の中身の種類。none は注記を持たず、text は書式つきの文字、symbol は記号（バルーンなどの囲み）
///
/// 列挙の値。知らない値も受け取る
public enum CalloutContent {
    public static let none = "none"
    public static let text = "text"
    public static let symbol = "symbol"
}

/// 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義の印の点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。小版 35 から
public struct PointGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("point")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 位置 [X, Y]
    public var position: [Double]? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 回転（度）
    public var rotation: Double? {
        get { readField("rotation") }
        set { writeField("rotation", newValue) }
    }

    /// 既定義の印。書かなければ文書の点の表し方に従う
    public var marker: PointMarker? {
        get { readField("marker") }
        set { writeField("marker", newValue) }
    }

    /// 作図の補助の点（JWW の仮点）か。書かなければ偽
    public var construction: Bool? {
        get { readField("construction") }
        set { writeField("construction", newValue) }
    }
}

/// 既定義の印の表のキーと尺度。印の形は、表示する側がキーから引く
public struct PointMarker: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 印の表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点）
    public var namespace: String? {
        get { readField("namespace") }
        set { writeField("namespace", newValue) }
    }

    /// 名前空間の中の元のコード
    public var code: String? {
        get { readField("code") }
        set { writeField("code", newValue) }
    }

    /// 尺度
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }
}

/// 寸法の種類。linear は 2 点を測る向きに、aligned は 2 点を結ぶ向きに測った長さ。angular は頂点から 2 点へ向かう線の、angular_lines は 2 本の線のなす角度。arc_length は円弧の長さ。radius は半径、diameter は直径。ordinate は原点からの X か Y の座標
///
/// 列挙の値。知らない値も受け取る
public enum DimensionKind {
    public static let linear = "linear"
    public static let aligned = "aligned"
    public static let angular = "angular"
    public static let angularLines = "angular_lines"
    public static let arcLength = "arc_length"
    public static let radius = "radius"
    public static let diameter = "diameter"
    public static let ordinate = "ordinate"
}

/// 座標の軸
///
/// 列挙の値。知らない値も受け取る
public enum DimensionAxis {
    public static let x = "x"
    public static let y = "y"
}

/// 寸法値の文字の書き方。measurement は測定値を書き、override は利用者が書いた文字（content）を書き、hidden は書かない
///
/// 列挙の値。知らない値も受け取る
public enum DimensionLabel {
    public static let measurement = "measurement"
    public static let override = "override"
    public static let hidden = "hidden"
}

/// 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。小版 14 から
public struct SymbolGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("symbol")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 記号の定義（ブロック）の元の名前。足すときは図面にある定義の名前を指す
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 定義の基点を置く点 [X, Y]
    public var position: [Double]? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// X と Y の尺度 [X, Y]。0 でない。負なら反転する
    public var scale: [Double]? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }

    /// 回転（度）
    public var rotation: Double? {
        get { readField("rotation") }
        set { writeField("rotation", newValue) }
    }

    /// 格子に並べて置く列と行。書かなければ 1 つだけ置く
    public var grid: SymbolGrid? {
        get { readField("grid") }
        set { writeField("grid", newValue) }
    }

    /// 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。小版 16 から。置き換えで値を変えるのは小版 21 から
    public var attributes: [SymbolAttribute]? {
        get { readField("attributes") }
        set { writeField("attributes", newValue) }
    }
}

/// 記号の属性のタグと値
public struct SymbolAttribute: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 元のタグ
    public var tag: String? {
        get { readField("tag") }
        set { writeField("tag", newValue) }
    }

    /// 値。1 行
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 記号を格子に並べる列と行。間隔は、記号を回転する前の向きに測る。置き換えるときは、列と行の数を変えない
public struct SymbolGrid: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 列の数。1 以上
    public var columns: Int64? {
        get { readField("columns") }
        set { writeField("columns", newValue) }
    }

    /// 行の数。1 以上。列と行がどちらも 1 の格子は持たない
    public var rows: Int64? {
        get { readField("rows") }
        set { writeField("rows", newValue) }
    }

    /// 列の間隔
    public var columnSpacing: Double? {
        get { readField("column_spacing") }
        set { writeField("column_spacing", newValue) }
    }

    /// 行の間隔
    public var rowSpacing: Double? {
        get { readField("row_spacing") }
        set { writeField("row_spacing", newValue) }
    }
}

/// 基点に合わせる枠の位置。entities は返し、足すときに書かなければ左上、置き換えるときに書かなければ元のまま
///
/// 列挙の値。知らない値も受け取る
public enum Attachment {
    public static let topLeft = "top_left"
    public static let topCenter = "top_center"
    public static let topRight = "top_right"
    public static let middleLeft = "middle_left"
    public static let middleCenter = "middle_center"
    public static let middleRight = "middle_right"
    public static let bottomLeft = "bottom_left"
    public static let bottomCenter = "bottom_center"
    public static let bottomRight = "bottom_right"
}

/// 要素の形の種類（決定 0022）。point（点、決定 0160）は小版 32 から。それまで点は symbol だった
///
/// 列挙の値。知らない値も受け取る
public enum EntityKind {
    public static let line = "line"
    public static let circle = "circle"
    public static let curve = "curve"
    public static let polyline = "polyline"
    public static let text = "text"
    public static let formattedText = "formatted_text"
    public static let dimension = "dimension"
    public static let leader = "leader"
    public static let callout = "callout"
    public static let table = "table"
    public static let underlay = "underlay"
    public static let symbol = "symbol"
    public static let hatch = "hatch"
    public static let point = "point"
}

/// 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。小版 7 から
public struct ApplyRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("apply")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で作ったもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 読んだときの文書のハッシュ（drawing の hash）。書けば、今の文書と同じときだけ当てる
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 変更。並べた順に当てる
    public var edits: [Edit]? {
        get { readField("edits") }
        set { writeField("edits", newValue) }
    }
}

/// 当てた変更の結果。書き戻したファイルは blobs の file で返す
public struct ApplyResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 変更ごとの結果。要求の edits と同じ順
    public var applied: [Applied]? {
        get { readField("applied") }
        set { writeField("applied", newValue) }
    }

    /// 書き戻した文書のハッシュ。次の apply の前提条件に使う
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// true なら、変えていない要素の ID がずれた。鍵を持たない形式（JWW）で、前の図形を除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。小版 30 から
    public var idsShifted: Bool? {
        get { readField("ids_shifted") }
        set { writeField("ids_shifted", newValue) }
    }
}

/// 開いた文書の図面モデルを、別の形式の新しいファイルに書き、新しい文書として開く（決定 0139・0206）。書けない要素は落とし、種類ごとの件数を診断で返す。書いたファイルは blobs で返す。今は DXF にだけ書ける。小版 41 から
public struct ConvertRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("convert")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 書く元の文書の名前。open で作ったもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 書く形式の名前（capabilities の formats の id）。今は dxf だけ
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 作る文書の名前
    public var to: String? {
        get { readField("to") }
        set { writeField("to", newValue) }
    }
}

/// 書いた文書。書いたファイルは blobs の file で返す
public struct ConvertResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 作った文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 書いた形式の名前
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 書いたファイルのハッシュ。apply の前提条件に使う
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 書いた要素の数。記号から離した属性の文字を含む
    public var elements: Int64? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }
}

/// 当てた変更 1 つの結果
public struct Applied: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 変えた要素（レイヤの変更では、そのレイヤ）の、書き戻した後の ID。除いたときと、ID を決められないときは持たない
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 変えた要素（レイヤ）の、書き戻した後のハッシュ。id があるときに持つ
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }
}

/// 要素 1 つへの変更。kind の値で形が決まる
public struct Edit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// kind が restyle の形なら、その見方。違えば nil
    public var asRestyleEdit: RestyleEdit? {
        taggedView("kind", "restyle")
    }

    /// kind が transform の形なら、その見方。違えば nil
    public var asTransformEdit: TransformEdit? {
        taggedView("kind", "transform")
    }

    /// kind が remove の形なら、その見方。違えば nil
    public var asRemoveEdit: RemoveEdit? {
        taggedView("kind", "remove")
    }

    /// kind が replace の形なら、その見方。違えば nil
    public var asReplaceEdit: ReplaceEdit? {
        taggedView("kind", "replace")
    }

    /// kind が add の形なら、その見方。違えば nil
    public var asAddEdit: AddEdit? {
        taggedView("kind", "add")
    }

    /// kind が layer の形なら、その見方。違えば nil
    public var asLayerEdit: LayerEdit? {
        taggedView("kind", "layer")
    }

    /// kind が add_layer の形なら、その見方。違えば nil
    public var asAddLayerEdit: AddLayerEdit? {
        taggedView("kind", "add_layer")
    }

    /// kind が remove_layer の形なら、その見方。違えば nil
    public var asRemoveLayerEdit: RemoveLayerEdit? {
        taggedView("kind", "remove_layer")
    }

    /// kind が text_style の形なら、その見方。違えば nil
    public var asTextStyleEdit: TextStyleEdit? {
        taggedView("kind", "text_style")
    }

    /// kind が add_text_style の形なら、その見方。違えば nil
    public var asAddTextStyleEdit: AddTextStyleEdit? {
        taggedView("kind", "add_text_style")
    }

    /// kind が remove_text_style の形なら、その見方。違えば nil
    public var asRemoveTextStyleEdit: RemoveTextStyleEdit? {
        taggedView("kind", "remove_text_style")
    }

    /// kind が line_type の形なら、その見方。違えば nil
    public var asLineTypeEdit: LineTypeEdit? {
        taggedView("kind", "line_type")
    }

    /// kind が add_line_type の形なら、その見方。違えば nil
    public var asAddLineTypeEdit: AddLineTypeEdit? {
        taggedView("kind", "add_line_type")
    }

    /// kind が remove_line_type の形なら、その見方。違えば nil
    public var asRemoveLineTypeEdit: RemoveLineTypeEdit? {
        taggedView("kind", "remove_line_type")
    }

    /// kind が dimension_style の形なら、その見方。違えば nil
    public var asDimensionStyleEdit: DimensionStyleEdit? {
        taggedView("kind", "dimension_style")
    }

    /// kind が add_dimension_style の形なら、その見方。違えば nil
    public var asAddDimensionStyleEdit: AddDimensionStyleEdit? {
        taggedView("kind", "add_dimension_style")
    }

    /// kind が remove_dimension_style の形なら、その見方。違えば nil
    public var asRemoveDimensionStyleEdit: RemoveDimensionStyleEdit? {
        taggedView("kind", "remove_dimension_style")
    }

    /// kind が point_style の形なら、その見方。違えば nil
    public var asPointStyleEdit: PointStyleEdit? {
        taggedView("kind", "point_style")
    }
}

/// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない
public struct RestyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("restyle")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 変える要素の ID。entities が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。entities が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない
    public var layer: String? {
        get { readField("layer") }
        set { writeField("layer", newValue) }
    }

    /// レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
    public var layerId: String? {
        get { readField("layer_id") }
        set { writeField("layer_id", newValue) }
    }

    /// 色
    public var color: ColorChange? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// 線種
    public var lineType: LineTypeChange? {
        get { readField("line_type") }
        set { writeField("line_type", newValue) }
    }

    /// 要素の線種の倍率
    public var lineTypeScale: Double? {
        get { readField("line_type_scale") }
        set { writeField("line_type_scale", newValue) }
    }

    /// 線幅
    public var lineWeight: LineWeightChange? {
        get { readField("line_weight") }
        set { writeField("line_weight", newValue) }
    }
}

/// 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
public struct TransformEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("transform")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 動かす要素の ID。entities が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。entities が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// [a, b, c, d, e, f]。要素の空間か用紙の点 (x, y) を (a x + b y + c, d x + e y + f) に写す
    public var matrix: [Double]? {
        get { readField("matrix") }
        set { writeField("matrix", newValue) }
    }
}

/// 要素を除く
public struct RemoveEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("remove")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 除く要素の ID。entities が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。entities が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }
}

/// 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤとスタイルは変えない。小版 10 から
public struct ReplaceEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("replace")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 置き換える要素の ID。entities が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。entities が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 新しい形。entities の geometry と同じ種類
    public var geometry: Geometry? {
        get { readField("geometry") }
        set { writeField("geometry", newValue) }
    }
}

/// 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。足せる形は、形の値のどの種類も。小版 10 から
public struct AddEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。小版 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177）
    public var place: String? {
        get { readField("place") }
        set { writeField("place", newValue) }
    }

    /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
    public var layer: String? {
        get { readField("layer") }
        set { writeField("layer", newValue) }
    }

    /// レイヤの ID。tables が返したもの
    public var layerId: String? {
        get { readField("layer_id") }
        set { writeField("layer_id", newValue) }
    }

    /// 色。SXF はレイヤに従う色を書けないので、書く。小版 29 から
    public var color: ColorChange? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// 線種。小版 29 から
    public var lineType: LineTypeChange? {
        get { readField("line_type") }
        set { writeField("line_type", newValue) }
    }

    /// 線幅。小版 29 から
    public var lineWeight: LineWeightChange? {
        get { readField("line_weight") }
        set { writeField("line_weight", newValue) }
    }

    /// 足す形
    public var geometry: Geometry? {
        get { readField("geometry") }
        set { writeField("geometry", newValue) }
    }
}

/// レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG は、今あるレイヤの線種と真の色を変えられない。小版 22 から
public struct LayerEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("layer")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 変えるレイヤの ID。tables が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときのレイヤのハッシュ。tables が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 新しい名前。ほかのレイヤと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。0 と Defpoints の名前は変えられない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 表示する。凍結するなら偽
    public var visible: Bool? {
        get { readField("visible") }
        set { writeField("visible", newValue) }
    }

    /// 印刷する
    public var printable: Bool? {
        get { readField("printable") }
        set { writeField("printable", newValue) }
    }

    /// 編集できる（ロックしない）
    public var editable: Bool? {
        get { readField("editable") }
        set { writeField("editable", newValue) }
    }

    /// 凍結する
    public var frozen: Bool? {
        get { readField("frozen") }
        set { writeField("frozen", newValue) }
    }

    /// 色。aci が要る。inherit は書けない
    public var color: ColorChange? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// 線種。name か id で指す。inherit は書けない
    public var lineType: LineTypeChange? {
        get { readField("line_type") }
        set { writeField("line_type", newValue) }
    }

    /// 線幅。mm か default。inherit は書けない
    public var lineWeight: LineWeightChange? {
        get { readField("line_weight") }
        set { writeField("line_weight", newValue) }
    }
}

/// レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。小版 24 から
public struct RemoveLayerEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("remove_layer")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 除くレイヤの ID。tables が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときのレイヤのハッシュ。tables が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }
}

/// 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。小版 25 から
public struct TextStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("text_style")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 変える文字スタイルの ID。tables が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの文字スタイルのハッシュ。tables が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 新しい名前。ほかの文字スタイルと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。Standard の名前は変えられない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
    public var font: Font? {
        get { readField("font") }
        set { writeField("font", newValue) }
    }

    /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
    public var bigFont: Font? {
        get { readField("big_font") }
        set { writeField("big_font", newValue) }
    }

    /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 幅の倍率。正の数
    public var width: Double? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 傾き（度）。-85 から 85 まで
    public var oblique: Double? {
        get { readField("oblique") }
        set { writeField("oblique", newValue) }
    }

    /// 左右を裏返す
    public var backward: Bool? {
        get { readField("backward") }
        set { writeField("backward", newValue) }
    }

    /// 上下を反転する
    public var upsideDown: Bool? {
        get { readField("upside_down") }
        set { writeField("upside_down", newValue) }
    }

    /// 縦書き
    public var vertical: Bool? {
        get { readField("vertical") }
        set { writeField("vertical", newValue) }
    }
}

/// 文字スタイルを足す（決定 0163）。書かない欄は Standard と同じ。同じ apply の要素は、足した文字スタイルを指せない。小版 25 から
public struct AddTextStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_text_style")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 名前。text_style の name と同じ決まり
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
    public var font: Font? {
        get { readField("font") }
        set { writeField("font", newValue) }
    }

    /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
    public var bigFont: Font? {
        get { readField("big_font") }
        set { writeField("big_font", newValue) }
    }

    /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 幅の倍率。正の数
    public var width: Double? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 傾き（度）。-85 から 85 まで
    public var oblique: Double? {
        get { readField("oblique") }
        set { writeField("oblique", newValue) }
    }

    /// 左右を裏返す
    public var backward: Bool? {
        get { readField("backward") }
        set { writeField("backward", newValue) }
    }

    /// 上下を反転する
    public var upsideDown: Bool? {
        get { readField("upside_down") }
        set { writeField("upside_down", newValue) }
    }

    /// 縦書き
    public var vertical: Bool? {
        get { readField("vertical") }
        set { writeField("vertical", newValue) }
    }
}

/// 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。小版 25 から
public struct RemoveTextStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("remove_text_style")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 除く文字スタイルの ID。tables が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの文字スタイルのハッシュ。tables が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }
}

/// 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。多重線のスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。小版 26 から
public struct LineTypeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("line_type")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 変える線種の ID。tables が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの線種のハッシュ。tables が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 新しい名前。ほかの線種と重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。ByLayer・ByBlock・Continuous と外部参照に依る線種の名前は変えられない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 説明
    public var description: String? {
        get { readField("description") }
        set { writeField("description", newValue) }
    }

    /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
    public var pattern: [Double]? {
        get { readField("pattern") }
        set { writeField("pattern", newValue) }
    }
}

/// 線種を足す（決定 0164）。書かない欄は Continuous と同じ。同じ apply の要素とレイヤは、足した線種を指せない。小版 26 から
public struct AddLineTypeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_line_type")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 名前。line_type の name と同じ決まり
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 説明
    public var description: String? {
        get { readField("description") }
        set { writeField("description", newValue) }
    }

    /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
    public var pattern: [Double]? {
        get { readField("pattern") }
        set { writeField("pattern", newValue) }
    }
}

/// 線種を除く（決定 0164）。要素・レイヤ・多重線のスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。小版 26 から
public struct RemoveLineTypeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("remove_line_type")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 除く線種の ID。tables が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの線種のハッシュ。tables が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }
}

/// 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。小版 27 から
public struct DimensionStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("dimension_style")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 変える寸法スタイルの ID。tables が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 新しい名前。ほかの寸法スタイルと重ならず（大文字と小文字を区別しない）、$ と < > / \ " : ; ? * | = ` を含まないもの。Standard・子の寸法スタイル（名前に $ を持つもの）・子を持つ寸法スタイル・外部参照に依る寸法スタイルの名前は変えられない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
    public var ends: [String]? {
        get { readField("ends") }
        set { writeField("ends", newValue) }
    }

    /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
    public var leader: String? {
        get { readField("leader") }
        set { writeField("leader", newValue) }
    }

    /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
    public var size: Double? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }

    /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
    public var tick: Double? {
        get { readField("tick") }
        set { writeField("tick", newValue) }
    }

    /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }
}

/// 寸法スタイルを足す（決定 0165）。書かない欄は Standard と同じ。同じ apply の要素は、足した寸法スタイルを指せない。小版 27 から
public struct AddDimensionStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_dimension_style")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 名前。dimension_style の name と同じ決まり
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
    public var ends: [String]? {
        get { readField("ends") }
        set { writeField("ends", newValue) }
    }

    /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
    public var leader: String? {
        get { readField("leader") }
        set { writeField("leader", newValue) }
    }

    /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
    public var size: Double? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }

    /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
    public var tick: Double? {
        get { readField("tick") }
        set { writeField("tick", newValue) }
    }

    /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }
}

/// 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。小版 27 から
public struct RemoveDimensionStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("remove_dimension_style")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 除く寸法スタイルの ID。tables が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }
}

/// 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG は見出しの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、小版 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。小版 38 から
public struct PointStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("point_style")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 基本の形
    public var mark: String? {
        get { readField("mark") }
        set { writeField("mark", newValue) }
    }

    /// 円で囲む。書かなければ偽
    public var circle: Bool? {
        get { readField("circle") }
        set { writeField("circle", newValue) }
    }

    /// 四角で囲む。書かなければ偽
    public var square: Bool? {
        get { readField("square") }
        set { writeField("square", newValue) }
    }

    /// 大きさ。0 より大きい有限の数。意味は size_kind で決まる
    public var size: Double? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }

    /// 大きさの意味
    public var sizeKind: String? {
        get { readField("size_kind") }
        set { writeField("size_kind", newValue) }
    }
}

/// 文書の点の基本の形。dot は小さな点、none はなし、plus は十字、cross はバツ、tick は上向きの短い線
///
/// 列挙の値。知らない値も受け取る
public enum PointMark {
    public static let dot = "dot"
    public static let none = "none"
    public static let plus = "plus"
    public static let cross = "cross"
    public static let tick = "tick"
}

/// 文書の点の大きさの意味。length は点のある空間の長さ、screen は画面の高さに対する百分率
///
/// 列挙の値。知らない値も受け取る
public enum PointSizeKind {
    public static let length = "length"
    public static let screen = "screen"
}

/// レイヤを足す（決定 0162）。書かない欄は、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の要素は、足したレイヤを指せない。小版 22 から
public struct AddLayerEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_layer")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 名前。layer の name と同じ決まり
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 表示する。凍結するなら偽
    public var visible: Bool? {
        get { readField("visible") }
        set { writeField("visible", newValue) }
    }

    /// 印刷する
    public var printable: Bool? {
        get { readField("printable") }
        set { writeField("printable", newValue) }
    }

    /// 編集できる（ロックしない）
    public var editable: Bool? {
        get { readField("editable") }
        set { writeField("editable", newValue) }
    }

    /// 凍結する
    public var frozen: Bool? {
        get { readField("frozen") }
        set { writeField("frozen", newValue) }
    }

    /// 色。aci が要る。inherit は書けない
    public var color: ColorChange? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// 線種。name か id で指す。inherit は書けない
    public var lineType: LineTypeChange? {
        get { readField("line_type") }
        set { writeField("line_type", newValue) }
    }

    /// 線幅。mm か default。inherit は書けない
    public var lineWeight: LineWeightChange? {
        get { readField("line_weight") }
        set { writeField("line_weight", newValue) }
    }
}

/// 値を継承する先。layer はレイヤから、block は要素を置いた記号の配置から
///
/// 列挙の値。知らない値も受け取る
public enum Inherit {
    public static let layer = "layer"
    public static let block = "block"
}

/// 新しい色。inherit か、aci・sxf・jww・rgb の少なくとも 1 つのどちらかを書く。aci・sxf・jww は 1 つだけ書く
public struct ColorChange: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 継承する先
    public var inherit: String? {
        get { readField("inherit") }
        set { writeField("inherit", newValue) }
    }

    /// AutoCAD の色の番号（1〜255）
    public var aci: Int64? {
        get { readField("aci") }
        set { writeField("aci", newValue) }
    }

    /// SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。小版 29 から
    public var sxf: Int64? {
        get { readField("sxf") }
        set { writeField("sxf", newValue) }
    }

    /// Jw_cad のペンの番号。0〜9 と、版 420 からの SXF の拡張色 100〜356。小版 30 から
    public var jww: Int64? {
        get { readField("jww") }
        set { writeField("jww", newValue) }
    }

    /// [赤, 緑, 青]。それぞれ 0〜255
    public var rgb: [Int64]? {
        get { readField("rgb") }
        set { writeField("rgb", newValue) }
    }
}

/// 新しい線種。inherit・name・id のどれか 1 つを書く
public struct LineTypeChange: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 継承する先
    public var inherit: String? {
        get { readField("inherit") }
        set { writeField("inherit", newValue) }
    }

    /// 線種の元の名前。図面にある線種で、名前が 1 つに決まるもの
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }
}

/// 新しい線幅。inherit・mm・default のどれか 1 つを書く
public struct LineWeightChange: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 継承する先
    public var inherit: String? {
        get { readField("inherit") }
        set { writeField("inherit", newValue) }
    }

    /// 用紙の上の幅（mm）。0 以上
    public var mm: Double? {
        get { readField("mm") }
        set { writeField("mm", newValue) }
    }

    /// true なら、文書の既定の線幅
    public var `default`: Bool? {
        get { readField("default") }
        set { writeField("default", newValue) }
    }
}

/// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から
public struct TablesRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("tables")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で作ったもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }
}

/// 表の項目。図面モデルの順
public struct TablesResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// レイヤ
    public var layers: [LayerItem]? {
        get { readField("layers") }
        set { writeField("layers", newValue) }
    }

    /// 線種。小版 26 から、定義を持つ
    public var lineTypes: [LineTypeItem]? {
        get { readField("line_types") }
        set { writeField("line_types", newValue) }
    }

    /// 寸法スタイル。小版 27 から、端部記号と大きさを持つ
    public var dimensionStyles: [DimensionStyleItem]? {
        get { readField("dimension_styles") }
        set { writeField("dimension_styles", newValue) }
    }

    /// 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義の欄を持たない項目として後ろに並ぶ。小版 20 から
    public var textStyles: [TextStyleItem]? {
        get { readField("text_styles") }
        set { writeField("text_styles", newValue) }
    }
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。小版 33 から
public struct SceneRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("scene")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で作ったもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 起点。実寸の空間・記号の定義の空間・用紙の ID（drawing と entities が返すもの）
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した空間の要素だけを描く。from が用紙のときだけ書ける
    public var view: Int64? {
        get { readField("view") }
        set { writeField("view", newValue) }
    }

    /// [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
    public var region: [Double]? {
        get { readField("region") }
        set { writeField("region", newValue) }
    }

    /// 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// true なら印刷する要素を、書かないか false なら表示する要素を描く
    public var print: Bool? {
        get { readField("print") }
        set { writeField("print", newValue) }
    }

    /// 色の表の入力の名前（zm_push で渡したもの）。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決の印を立てる
    public var palette: String? {
        get { readField("palette") }
        set { writeField("palette", newValue) }
    }

    /// 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形の区画に形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。小版 37 から
    public var fonts: [SceneFont]? {
        get { readField("fonts") }
        set { writeField("fonts", newValue) }
    }

    /// 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは大文字の高さとみなす。fonts を書いたときだけ使う。小版 37 から
    public var heights: [String: String]? {
        get { readField("heights") }
        set { writeField("heights", newValue) }
    }
}

/// シーンの文字を置くフォント（決定 0188）
public struct SceneFont: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 文字が指すフォントの名前（ファイルに書かれた元の名前）。書かなければ、名前の合わない文字に使う既定のフォント。同じ名前と、名前のないものは 1 つまで
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// TrueType・OpenType（CFF・CFF2 を含む）のファイルの入力の名前（zm_push で渡したもの）
    public var input: String? {
        get { readField("input") }
        set { writeField("input", newValue) }
    }

    /// フォントのコレクション（TTC）の中の番号。書かなければ 0
    public var index: Int64? {
        get { readField("index") }
        set { writeField("index", newValue) }
    }
}

/// 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ
///
/// 列挙の値。知らない値も受け取る
public enum HeightReference {
    public static let capHeight = "cap_height"
    public static let em = "em"
    public static let frame = "frame"
}

/// シーンの要約と、シーンの blob の番号
public struct SceneResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 起点の ID
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 起点のビューの番号。ビューでなければ持たない
    public var view: Int64? {
        get { readField("view") }
        set { writeField("view", newValue) }
    }

    /// 起点の座標の単位。用紙とビューからは millimeter
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// 使った弦の誤差（起点の長さ）
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 返した描くものの数
    public var counts: SceneCounts? {
        get { readField("counts") }
        set { writeField("counts", newValue) }
    }

    /// 返した塊の範囲。起点の座標。塊がなければ持たない
    public var extent: Extent? {
        get { readField("extent") }
        set { writeField("extent", newValue) }
    }

    /// シーンの blob の番号（種類は scene）。zm_take で取り出す
    public var blob: Int64? {
        get { readField("blob") }
        set { writeField("blob", newValue) }
    }
}

/// シーンの描くものの種類ごとの数
public struct SceneCounts: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 塊
    public var chunks: Int64? {
        get { readField("chunks") }
        set { writeField("chunks", newValue) }
    }

    /// 線（折れ線）
    public var lines: Int64? {
        get { readField("lines") }
        set { writeField("lines", newValue) }
    }

    /// 線と面の頂点
    public var vertices: Int64? {
        get { readField("vertices") }
        set { writeField("vertices", newValue) }
    }

    /// 面
    public var faces: Int64? {
        get { readField("faces") }
        set { writeField("faces", newValue) }
    }

    /// 文字
    public var texts: Int64? {
        get { readField("texts") }
        set { writeField("texts", newValue) }
    }

    /// 点
    public var points: Int64? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 画像
    public var images: Int64? {
        get { readField("images") }
        set { writeField("images", newValue) }
    }

    /// 置いた字形。fonts を書かなければ 0。小版 37 から
    public var glyphs: Int64? {
        get { readField("glyphs") }
        set { writeField("glyphs", newValue) }
    }
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。小版 34 から
public struct CheckRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("check")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で作ったもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    public var profiles: [String]? {
        get { readField("profiles") }
        set { writeField("profiles", newValue) }
    }

    /// 照合に使うプロファイル。profiles の中から選ぶ
    public var profile: ProfileRef? {
        get { readField("profile") }
        set { writeField("profile", newValue) }
    }

    /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
    public var view: Int64? {
        get { readField("view") }
        set { writeField("view", newValue) }
    }

    /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
    public var fileName: String? {
        get { readField("file_name") }
        set { writeField("file_name", newValue) }
    }

    /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す指摘の数の上限。書かなければ 1000
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }

    /// 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
    public var palette: String? {
        get { readField("palette") }
        set { writeField("palette", newValue) }
    }
}

/// プロファイルの識別（決定 0138）
public struct ProfileRef: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 基準の系統（mlit-cad など）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 基準の版（2025-12 など）
    public var edition: String? {
        get { readField("edition") }
        set { writeField("edition", newValue) }
    }
}

/// 照合の結果。指摘は offset から limit 個まで
public struct CheckResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 照合に使ったプロファイルの鎖。extends の基から順
    public var chain: [ProfileIdentity]? {
        get { readField("chain") }
        set { writeField("chain", newValue) }
    }

    /// 照合した規則ごとの数。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
    public var rules: [CheckRule]? {
        get { readField("rules") }
        set { writeField("rules", newValue) }
    }

    /// 指摘。規則の順に、規則ごとにたどった順
    public var findings: [CheckFinding]? {
        get { readField("findings") }
        set { writeField("findings", newValue) }
    }

    /// 全体の指摘の数
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 続きの最初の指摘の位置。続きがなければ持たない
    public var nextOffset: Int64? {
        get { readField("next_offset") }
        set { writeField("next_offset", newValue) }
    }
}

/// 照合に使ったプロファイルの識別と、データの直し
public struct ProfileIdentity: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 基準の系統
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 基準の版
    public var edition: String? {
        get { readField("edition") }
        set { writeField("edition", newValue) }
    }

    /// このデータの直し。1 から
    public var revision: Int64? {
        get { readField("revision") }
        set { writeField("revision", newValue) }
    }
}

/// 規則ごとの数
public struct CheckRule: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 規則の id
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 照合の種類（layer-name など）
    public var check: String? {
        get { readField("check") }
        set { writeField("check", newValue) }
    }

    /// 指摘の数。照合できなかったものを除く
    public var findings: Int64? {
        get { readField("findings") }
        set { writeField("findings", newValue) }
    }

    /// 照合できなかった数
    public var unsupported: Int64? {
        get { readField("unsupported") }
        set { writeField("unsupported", newValue) }
    }
}

/// 指摘。diagnostic は診断と同じ形で、describe で文にできる。severity は規則の level で、照合できない値（profile.<種類>.unsupported）は info。count は 1
public struct CheckFinding: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 規則の id
    public var rule: String? {
        get { readField("rule") }
        set { writeField("rule", newValue) }
    }

    /// code・重要度・場所（要素・実効のレイヤ・用紙の ID）・見つけた値
    public var diagnostic: Diagnostic? {
        get { readField("diagnostic") }
        set { writeField("diagnostic", newValue) }
    }
}

/// zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。小版 42 から
public struct ValidateRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("validate")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// IFC の入力の名前（zm_push で渡した .ifc か .ifczip）
    public var input: String? {
        get { readField("input") }
        set { writeField("input", newValue) }
    }

    /// EXPRESS の入力の名前（zm_push で渡した .exp）
    public var express: String? {
        get { readField("express") }
        set { writeField("express", newValue) }
    }

    /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す指摘の数の上限。書かなければ 1000
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// スキーマに照らしたチェックの結果。指摘は offset から limit 個まで。スキーマの名前の違いと評価しない規則は、診断で返す
public struct ValidateResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// EXPRESS のスキーマの名前（大文字）
    public var schema: String? {
        get { readField("schema") }
        set { writeField("schema", newValue) }
    }

    /// IFC の FILE_SCHEMA の名前
    public var fileSchema: String? {
        get { readField("file_schema") }
        set { writeField("file_schema", newValue) }
    }

    /// EXPRESS のバイト列のハッシュ（v2: と 32 桁の 16 進）。どの EXPRESS でチェックしたかを残す
    public var expressHash: String? {
        get { readField("express_hash") }
        set { writeField("express_hash", newValue) }
    }

    /// 評価しなかった規則（WHERE・UNIQUE・RULE）の数。確認できなかった項目
    public var rulesNotEvaluated: Int64? {
        get { readField("rules_not_evaluated") }
        set { writeField("rules_not_evaluated", newValue) }
    }

    /// 指摘。インスタンスの番号の順に、属性の順。診断と同じ形で、describe で文にできる。count は 1
    public var findings: [Diagnostic]? {
        get { readField("findings") }
        set { writeField("findings", newValue) }
    }

    /// 全体の指摘の数
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 続きの最初の指摘の位置。続きがなければ持たない
    public var nextOffset: Int64? {
        get { readField("next_offset") }
        set { writeField("next_offset", newValue) }
    }
}

/// レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書の版へ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元の版のハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。小版 43 から
public struct ReviewCarryRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("review_carry")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 新しい版の文書の名前。open で作ったもの。小版 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// レビューの文書の入力の名前（zm_push で渡した JSON）
    public var review: String? {
        get { readField("review") }
        set { writeField("review", newValue) }
    }

    /// 移す対象の ID。レビューの文書の targets の id
    public var target: String? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }

    /// 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す参照の数の上限。書かなければ 1000
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// 移した結果。参照は offset から limit 個まで。移した文書は blobs の file で返す
public struct ReviewCarryResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 移す前の対象のハッシュ
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 移した後の対象のハッシュ。文書のハッシュ
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 要素の参照ごとの状態。指摘の順に、場所の順、参照の順
    public var references: [CarriedReference]? {
        get { readField("references") }
        set { writeField("references", newValue) }
    }

    /// 全体の参照の数
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 続きの最初の参照の位置。続きがなければ持たない
    public var nextOffset: Int64? {
        get { readField("next_offset") }
        set { writeField("next_offset", newValue) }
    }
}

/// 移した要素の参照
public struct CarriedReference: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 指摘の ID
    public var topic: String? {
        get { readField("topic") }
        set { writeField("topic", newValue) }
    }

    /// 場所の ID
    public var location: String? {
        get { readField("location") }
        set { writeField("location", newValue) }
    }

    /// 要素の ID
    public var element: String? {
        get { readField("element") }
        set { writeField("element", newValue) }
    }

    /// 状態
    public var state: String? {
        get { readField("state") }
        set { writeField("state", newValue) }
    }
}

/// 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しい版に要素がない
///
/// 列挙の値。知らない値も受け取る
public enum CarriedState {
    public static let unchanged = "unchanged"
    public static let changed = "changed"
    public static let removed = "removed"
}

/// 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。小版 43 から
public struct ReviewAdoptRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("review_adopt")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 照合する文書の名前。open で作ったもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    public var profiles: [String]? {
        get { readField("profiles") }
        set { writeField("profiles", newValue) }
    }

    /// 照合に使うプロファイル。profiles の中から選ぶ
    public var profile: ProfileRef? {
        get { readField("profile") }
        set { writeField("profile", newValue) }
    }

    /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
    public var view: Int64? {
        get { readField("view") }
        set { writeField("view", newValue) }
    }

    /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
    public var fileName: String? {
        get { readField("file_name") }
        set { writeField("file_name", newValue) }
    }

    /// 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
    public var palette: String? {
        get { readField("palette") }
        set { writeField("palette", newValue) }
    }

    /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
    public var review: String? {
        get { readField("review") }
        set { writeField("review", newValue) }
    }

    /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
    public var target: String? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }

    /// 取り込む指摘の番号の列。check の指摘の並びの順で、0 から数える
    public var findings: [Int64]? {
        get { readField("findings") }
        set { writeField("findings", newValue) }
    }
}

/// 取り込んだ結果。足した文書は blobs の file で返す
public struct ReviewAdoptResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 足した指摘の ID。取り込み済みで足さなかった指摘は持たない
    public var added: [String]? {
        get { readField("added") }
        set { writeField("added", newValue) }
    }

    /// 足した後の文書の指摘の数
    public var topics: Int64? {
        get { readField("topics") }
        set { writeField("topics", newValue) }
    }
}

/// 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。小版 44 から
public struct BuildingRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("building")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で IFC を開いたもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }
}

/// 建物モデルの要約
public struct BuildingResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// IFC のスキーマの名前（FILE_SCHEMA の最初）
    public var schema: String? {
        get { readField("schema") }
        set { writeField("schema", newValue) }
    }

    /// 入力のバイト列のハッシュ。review_carry の対象の版に使う
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// プロジェクトの要素の ID。なければ持たない
    public var project: String? {
        get { readField("project") }
        set { writeField("project", newValue) }
    }

    /// プロジェクトの名前。なければ持たない
    public var projectName: String? {
        get { readField("project_name") }
        set { writeField("project_name", newValue) }
    }

    /// プロジェクトの単位
    public var units: [BuildingUnit]? {
        get { readField("units") }
        set { writeField("units", newValue) }
    }

    /// 要素の数
    public var elements: Int64? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }

    /// クラスごとの要素の数。クラスの名前の順
    public var classes: [ClassCount]? {
        get { readField("classes") }
        set { writeField("classes", newValue) }
    }
}

/// 単位
public struct BuildingUnit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 単位の種類（LENGTHUNIT・AREAUNIT など）
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// SI の接頭辞（MILLI など）
    public var prefix: String? {
        get { readField("prefix") }
        set { writeField("prefix", newValue) }
    }

    /// 単位の名前（METRE、換算する単位なら inch など）
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 換算する単位の、SI の単位での大きさ。SI の単位は持たない
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }
}

/// クラスの要素の数
public struct ClassCount: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// IFC のクラスの名前（IfcWall など）
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 要素の数
    public var count: Int64? {
        get { readField("count") }
        set { writeField("count", newValue) }
    }
}

/// 開いた IFC の建物モデルの要素を、書いた順に返す。成分は components で選ぶ（決定 0207・0219）。小版 44 から
public struct BuildingElementsRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("building_elements")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で IFC を開いたもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 返す要素の ID。書けば、その要素だけを、書いた順に返す。ない ID は返さない
    public var ids: [String]? {
        get { readField("ids") }
        set { writeField("ids", newValue) }
    }

    /// IFC のクラスの名前。書けば、そのクラスの要素だけを返す。大文字と小文字を区別しない
    public var `class`: String? {
        get { readField("class") }
        set { writeField("class", newValue) }
    }

    /// 返す成分の名前。1 つ以上で、重ねない。書かなければ class と identity を返す
    public var components: [String]? {
        get { readField("components") }
        set { writeField("components", newValue) }
    }

    /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す数の上限。書かなければ 1000
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// building_elements で返す成分（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員
///
/// 列挙の値。知らない値も受け取る
public enum BuildingComponent {
    public static let identity = "identity"
    public static let `class` = "class"
    public static let classification = "classification"
    public static let spatial = "spatial"
    public static let parent = "parent"
    public static let placement = "placement"
    public static let typeOf = "type_of"
    public static let properties = "properties"
    public static let quantities = "quantities"
    public static let material = "material"
    public static let opening = "opening"
    public static let group = "group"
    public static let connection = "connection"
    public static let level = "level"
    public static let zone = "zone"
    public static let style = "style"
    public static let relations = "relations"
}

/// 要素の一覧。offset から limit 個まで
public struct BuildingElementsResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 要素
    public var elements: [BuildingElement]? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }

    /// 選んだ要素の数
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 続きの最初の位置。続きがなければ持たない
    public var nextOffset: Int64? {
        get { readField("next_offset") }
        set { writeField("next_offset", newValue) }
    }
}

/// 建物モデルの要素。成分は、選んだもののうち要素が持つものだけを持つ
public struct BuildingElement: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 要素の ID（GlobalId か、i と番号）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 要素の成分のハッシュ（v1: と 32 桁の 16 進、決定 0207）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 名前・説明・タグ・長い名前
    public var identity: IdentityComponent? {
        get { readField("identity") }
        set { writeField("identity", newValue) }
    }

    /// クラス
    public var `class`: ClassComponent? {
        get { readField("class") }
        set { writeField("class", newValue) }
    }

    /// 分類の参照
    public var classification: [ClassificationReference]? {
        get { readField("classification") }
        set { writeField("classification", newValue) }
    }

    /// 空間の要素の種類と高さ
    public var spatial: SpatialComponent? {
        get { readField("spatial") }
        set { writeField("spatial", newValue) }
    }

    /// 親と、参照される空間
    public var parent: ParentComponent? {
        get { readField("parent") }
        set { writeField("parent", newValue) }
    }

    /// 配置
    public var placement: PlacementComponent? {
        get { readField("placement") }
        set { writeField("placement", newValue) }
    }

    /// 型の要素の ID
    public var typeOf: String? {
        get { readField("type_of") }
        set { writeField("type_of", newValue) }
    }

    /// プロパティのセット
    public var properties: [PropertySet]? {
        get { readField("properties") }
        set { writeField("properties", newValue) }
    }

    /// 数量のセット
    public var quantities: [QuantitySet]? {
        get { readField("quantities") }
        set { writeField("quantities", newValue) }
    }

    /// 材料
    public var material: MaterialComponent? {
        get { readField("material") }
        set { writeField("material", newValue) }
    }

    /// 開口と、埋める要素
    public var opening: OpeningComponent? {
        get { readField("opening") }
        set { writeField("opening", newValue) }
    }

    /// 属するグループの要素の ID
    public var group: [String]? {
        get { readField("group") }
        set { writeField("group", newValue) }
    }

    /// 接続
    public var connection: [ConnectionLink]? {
        get { readField("connection") }
        set { writeField("connection", newValue) }
    }

    /// レベル
    public var level: LevelComponent? {
        get { readField("level") }
        set { writeField("level", newValue) }
    }

    /// 区画
    public var zone: ZoneComponent? {
        get { readField("zone") }
        set { writeField("zone", newValue) }
    }

    /// 表示のスタイル
    public var style: StyleComponent? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }

    /// 索引から求める関係
    public var relations: RelationsComponent? {
        get { readField("relations") }
        set { writeField("relations", newValue) }
    }
}

/// 名前・説明・タグ・長い名前
public struct IdentityComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 説明
    public var description: String? {
        get { readField("description") }
        set { writeField("description", newValue) }
    }

    /// タグ
    public var tag: String? {
        get { readField("tag") }
        set { writeField("tag", newValue) }
    }

    /// 長い名前
    public var longName: String? {
        get { readField("long_name") }
        set { writeField("long_name", newValue) }
    }
}

/// クラス
public struct ClassComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// IFC4 のクラスの名前。IFC2X3 の StandardCase などは、元の名前を original に残して直す
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// PredefinedType
    public var predefined: String? {
        get { readField("predefined") }
        set { writeField("predefined", newValue) }
    }

    /// ObjectType など、USERDEFINED のときの名前
    public var userDefined: String? {
        get { readField("user_defined") }
        set { writeField("user_defined", newValue) }
    }

    /// ファイルに書いた実体の名前
    public var original: String? {
        get { readField("original") }
        set { writeField("original", newValue) }
    }
}

/// 分類の参照
public struct ClassificationReference: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 分類の体系の名前
    public var system: String? {
        get { readField("system") }
        set { writeField("system", newValue) }
    }

    /// 体系の版
    public var edition: String? {
        get { readField("edition") }
        set { writeField("edition", newValue) }
    }

    /// 分類の記号
    public var code: String? {
        get { readField("code") }
        set { writeField("code", newValue) }
    }

    /// 分類の名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }
}

/// 単位とともに持つ値
public struct Measured: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 値
    public var value: Double? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }

    /// 単位。解けなければ持たない
    public var unit: BuildingUnit? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }
}

/// 空間の要素
public struct SpatialComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// site・building・storey・space・spatial_zone か、元の実体の名前
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 高さ
    public var elevation: Measured? {
        get { readField("elevation") }
        set { writeField("elevation", newValue) }
    }

    /// 基準の高さ
    public var refElevation: Measured? {
        get { readField("ref_elevation") }
        set { writeField("ref_elevation", newValue) }
    }
}

/// 親
public struct ParentComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 親の要素
    public var of: Membership? {
        get { readField("of") }
        set { writeField("of", newValue) }
    }

    /// 参照される空間の要素の ID
    public var referencedIn: [String]? {
        get { readField("referenced_in") }
        set { writeField("referenced_in", newValue) }
    }
}

/// 親との関係
public struct Membership: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 親の要素の ID
    public var target: String? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }

    /// 関係の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 入れ子の順。0 から
    public var order: Int64? {
        get { readField("order") }
        set { writeField("order", newValue) }
    }
}

/// 配置
public struct PlacementComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 配置の親の要素の ID
    public var relativeTo: String? {
        get { readField("relative_to") }
        set { writeField("relative_to", newValue) }
    }

    /// 親をたどった世界の変換。3 行 4 列（回転と移動）。解けなければ持たない
    public var world: [[Double]]? {
        get { readField("world") }
        set { writeField("world", newValue) }
    }

    /// 解けなかった配置の実体の名前
    public var unsupported: String? {
        get { readField("unsupported") }
        set { writeField("unsupported", newValue) }
    }
}

/// 値
public struct BuildingValue: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 値の型の名前（IfcLengthMeasure・IfcLabel など）
    public var typeName: String? {
        get { readField("type_name") }
        set { writeField("type_name", newValue) }
    }

    /// 値の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// real と、2^53 − 1 までの integer の値
    public var number: Double? {
        get { readField("number") }
        set { writeField("number", newValue) }
    }

    /// text と enumeration の値と、2^53 − 1 を超える integer の値
    public var text: String? {
        get { readField("text") }
        set { writeField("text", newValue) }
    }

    /// boolean と logical の値。logical の不明は持たない
    public var boolean: Bool? {
        get { readField("boolean") }
        set { writeField("boolean", newValue) }
    }

    /// element の要素の ID
    public var element: String? {
        get { readField("element") }
        set { writeField("element", newValue) }
    }

    /// list の値
    public var items: [BuildingValue]? {
        get { readField("items") }
        set { writeField("items", newValue) }
    }
}

/// プロパティのセット
public struct PropertySet: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// プロパティ
    public var properties: [Property]? {
        get { readField("properties") }
        set { writeField("properties", newValue) }
    }
}

/// プロパティ
public struct Property: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 値。値がなければ持たない
    public var value: BuildingValue? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }

    /// 単位
    public var unit: BuildingUnit? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }
}

/// 数量のセット
public struct QuantitySet: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 計測の方法
    public var method: String? {
        get { readField("method") }
        set { writeField("method", newValue) }
    }

    /// 数量
    public var quantities: [Quantity]? {
        get { readField("quantities") }
        set { writeField("quantities", newValue) }
    }
}

/// 数量
public struct Quantity: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 値
    public var value: Measured? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }

    /// 式
    public var formula: String? {
        get { readField("formula") }
        set { writeField("formula", newValue) }
    }
}

/// 材料
public struct MaterialComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 材料の組み方
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 部分
    public var parts: [MaterialPart]? {
        get { readField("parts") }
        set { writeField("parts", newValue) }
    }

    /// 層の向き
    public var direction: String? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// 層の基準からのずれ
    public var offset: Measured? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }
}

/// 材料の部分
public struct MaterialPart: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 材料の要素の ID
    public var material: String? {
        get { readField("material") }
        set { writeField("material", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 厚さ
    public var thickness: Measured? {
        get { readField("thickness") }
        set { writeField("thickness", newValue) }
    }
}

/// 開口
public struct OpeningComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// この開口が穴を開ける要素の ID
    public var voids: String? {
        get { readField("voids") }
        set { writeField("voids", newValue) }
    }

    /// この要素が埋める開口の要素の ID
    public var fills: String? {
        get { readField("fills") }
        set { writeField("fills", newValue) }
    }
}

/// 接続
public struct ConnectionLink: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 関係の実体の名前
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 接続する要素の ID
    public var target: String? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }
}

/// レベル
public struct LevelComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 高さ
    public var height: Measured? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 基準
    public var datum: String? {
        get { readField("datum") }
        set { writeField("datum", newValue) }
    }
}

/// 区画
public struct ZoneComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 区画の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 構成する要素の ID
    public var members: [String]? {
        get { readField("members") }
        set { writeField("members", newValue) }
    }
}

/// 表示のスタイル
public struct StyleComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 面の色の RGB（0 から 1）
    public var surfaceColor: [Double]? {
        get { readField("surface_color") }
        set { writeField("surface_color", newValue) }
    }

    /// 透明度（0 から 1）
    public var transparency: Double? {
        get { readField("transparency") }
        set { writeField("transparency", newValue) }
    }

    /// 線の色の RGB（0 から 1）
    public var curveColor: [Double]? {
        get { readField("curve_color") }
        set { writeField("curve_color", newValue) }
    }

    /// 提示のレイヤの名前
    public var layers: [String]? {
        get { readField("layers") }
        set { writeField("layers", newValue) }
    }
}

/// 索引から求める関係。要素の ID の列
public struct RelationsComponent: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 子（集約・入れ子・包含）
    public var children: [String]? {
        get { readField("children") }
        set { writeField("children", newValue) }
    }

    /// 参照する要素
    public var referenced: [String]? {
        get { readField("referenced") }
        set { writeField("referenced", newValue) }
    }

    /// 型の実例
    public var instances: [String]? {
        get { readField("instances") }
        set { writeField("instances", newValue) }
    }

    /// 開口
    public var openings: [String]? {
        get { readField("openings") }
        set { writeField("openings", newValue) }
    }

    /// 開口を埋める要素
    public var fillings: [String]? {
        get { readField("fillings") }
        set { writeField("fillings", newValue) }
    }

    /// グループ・システム・区画の構成員
    public var members: [String]? {
        get { readField("members") }
        set { writeField("members", newValue) }
    }
}

/// 親との関係の種類。集約・入れ子・包含
///
/// 列挙の値。知らない値も受け取る
public enum ParentKind {
    public static let aggregates = "aggregates"
    public static let nests = "nests"
    public static let contained = "contained"
}

/// 値の種類
///
/// 列挙の値。知らない値も受け取る
public enum BuildingValueKind {
    public static let real = "real"
    public static let integer = "integer"
    public static let text = "text"
    public static let boolean = "boolean"
    public static let logical = "logical"
    public static let enumeration = "enumeration"
    public static let element = "element"
    public static let list = "list"
}

/// 数量の種類
///
/// 列挙の値。知らない値も受け取る
public enum QuantityKind {
    public static let length = "length"
    public static let area = "area"
    public static let volume = "volume"
    public static let count = "count"
    public static let weight = "weight"
    public static let time = "time"
}

/// 材料の組み方。単一・層・断面・構成・一覧
///
/// 列挙の値。知らない値も受け取る
public enum MaterialKind {
    public static let single = "single"
    public static let layers = "layers"
    public static let profiles = "profiles"
    public static let constituents = "constituents"
    public static let list = "list"
}

/// 仕様の結果。unknown は確かめられない facet があった
///
/// 列挙の値。知らない値も受け取る
public enum IdsStatus {
    public static let passed = "passed"
    public static let failed = "failed"
    public static let unknown = "unknown"
}

/// 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。小版 44 から
public struct IdsCheckRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("ids_check")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で IFC を開いたもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// IDS の入力の名前（zm_push で渡した .ids）
    public var ids: String? {
        get { readField("ids") }
        set { writeField("ids", newValue) }
    }

    /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす
    public var express: String? {
        get { readField("express") }
        set { writeField("express", newValue) }
    }

    /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す数の上限。書かなければ 1000
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// 照合の結果。指摘は offset から limit 個まで
public struct IdsCheckResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// IDS の題
    public var title: String? {
        get { readField("title") }
        set { writeField("title", newValue) }
    }

    /// 仕様ごとの結果。IDS の順
    public var specifications: [IdsSpecification]? {
        get { readField("specifications") }
        set { writeField("specifications", newValue) }
    }

    /// 指摘。仕様の順
    public var findings: [IdsFinding]? {
        get { readField("findings") }
        set { writeField("findings", newValue) }
    }

    /// 全体の指摘の数
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 続きの最初の位置。続きがなければ持たない
    public var nextOffset: Int64? {
        get { readField("next_offset") }
        set { writeField("next_offset", newValue) }
    }
}

/// 仕様の結果
public struct IdsSpecification: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 識別子
    public var identifier: String? {
        get { readField("identifier") }
        set { writeField("identifier", newValue) }
    }

    /// 結果
    public var status: String? {
        get { readField("status") }
        set { writeField("status", newValue) }
    }

    /// 適用の条件に合う要素の数
    public var applicable: Int64? {
        get { readField("applicable") }
        set { writeField("applicable", newValue) }
    }

    /// 要件を満たさない要素の数
    public var failed: Int64? {
        get { readField("failed") }
        set { writeField("failed", newValue) }
    }

    /// 確かめられなかった要素の数
    public var unsupported: Int64? {
        get { readField("unsupported") }
        set { writeField("unsupported", newValue) }
    }
}

/// 指摘。diagnostic は診断と同じ形で、describe で文にできる
public struct IdsFinding: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 仕様の名前
    public var specification: String? {
        get { readField("specification") }
        set { writeField("specification", newValue) }
    }

    /// code・重要度・場所（要素の ID）・値
    public var diagnostic: Diagnostic? {
        get { readField("diagnostic") }
        set { writeField("diagnostic", newValue) }
    }
}

/// 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。小版 44 から
public struct ReviewAdoptIdsRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("review_adopt_ids")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で IFC を開いたもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// IDS の入力の名前（zm_push で渡した .ids）
    public var ids: String? {
        get { readField("ids") }
        set { writeField("ids", newValue) }
    }

    /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす
    public var express: String? {
        get { readField("express") }
        set { writeField("express", newValue) }
    }

    /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
    public var review: String? {
        get { readField("review") }
        set { writeField("review", newValue) }
    }

    /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
    public var target: String? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }

    /// 取り込む指摘の番号の列。ids_check の指摘の並びの順で、0 から数える
    public var findings: [Int64]? {
        get { readField("findings") }
        set { writeField("findings", newValue) }
    }
}

/// zm_push で渡した BCF 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。小版 45 から
public struct BcfReadRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("bcf_read")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// BCF が指す建物モデルの文書の名前。open で IFC を開いたもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// bcfzip の入力の名前（zm_push で渡した .bcf）
    public var input: String? {
        get { readField("input") }
        set { writeField("input", newValue) }
    }

    /// レビューの文書の対象の ID
    public var target: String? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }

    /// 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
    public var statuses: [BcfStatus]? {
        get { readField("statuses") }
        set { writeField("statuses", newValue) }
    }
}

/// 状態の対応
public struct BcfStatus: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// BCF の TopicStatus の文字列
    public var text: String? {
        get { readField("text") }
        set { writeField("text", newValue) }
    }

    /// 指摘の状態
    public var status: String? {
        get { readField("status") }
        set { writeField("status", newValue) }
    }
}

/// 指摘の状態（決定 0214）
///
/// 列挙の値。知らない値も受け取る
public enum ReviewStatus {
    public static let `open` = "open"
    public static let resolved = "resolved"
    public static let closed = "closed"
}

/// 読んだ bcfzip
public struct BcfReadResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// BCF の版
    public var version: String? {
        get { readField("version") }
        set { writeField("version", newValue) }
    }

    /// 指摘の数
    public var topics: Int64? {
        get { readField("topics") }
        set { writeField("topics", newValue) }
    }
}

/// zm_push で渡したレビューの文書を、BCF 3.0 の bcfzip に書く（決定 0214・0220）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。小版 45 から
public struct BcfWriteRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("bcf_write")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// レビューの文書の入力の名前（zm_push で渡した JSON）
    public var review: String? {
        get { readField("review") }
        set { writeField("review", newValue) }
    }

    /// viewpoint にする 3D の場所の対象の ID
    public var target: String? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }

    /// 元の bcfzip の入力の名前（zm_push で渡した .bcf）。書かなければ新しく書く
    public var original: String? {
        get { readField("original") }
        set { writeField("original", newValue) }
    }

    /// 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
    public var statuses: [BcfStatus]? {
        get { readField("statuses") }
        set { writeField("statuses", newValue) }
    }

    /// 種類のない指摘に書く TopicType
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 作者のない指摘とコメントに書く作者
    public var author: String? {
        get { readField("author") }
        set { writeField("author", newValue) }
    }

    /// 日時のない指摘とコメントに書く日時（RFC 3339）
    public var date: String? {
        get { readField("date") }
        set { writeField("date", newValue) }
    }
}

/// 書いた bcfzip
public struct BcfWriteResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 書いた指摘の数
    public var topics: Int64? {
        get { readField("topics") }
        set { writeField("topics", newValue) }
    }
}

/// 表の項目
public struct TableItem: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// ID。表のレコードがハンドルを持てば layer/h10・line-type/h14、なければ読んだ順の番号（layer/2）。寸法スタイルは dimension-style/h1A か、寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 元の名前。翻訳しない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }
}

/// レイヤの表の項目（決定 0162）。小版 22 から、状態とスタイルを持つ
public struct LayerItem: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// ID。表のレコードがハンドルを持てば layer/h10、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 元の名前。翻訳しない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかの欄を持たない。小版 22 から
    public var defined: Bool? {
        get { readField("defined") }
        set { writeField("defined", newValue) }
    }

    /// 表示する。小版 22 から
    public var visible: Bool? {
        get { readField("visible") }
        set { writeField("visible", newValue) }
    }

    /// 印刷する。小版 22 から
    public var printable: Bool? {
        get { readField("printable") }
        set { writeField("printable", newValue) }
    }

    /// 編集できる。小版 22 から
    public var editable: Bool? {
        get { readField("editable") }
        set { writeField("editable", newValue) }
    }

    /// 凍結する。小版 22 から
    public var frozen: Bool? {
        get { readField("frozen") }
        set { writeField("frozen", newValue) }
    }

    /// 色。inherit を持たない。定義が色を持たなければ書かない。小版 22 から
    public var color: ColorChange? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// 線種の ID。定義が線種を持たなければ書かない。小版 22 から
    public var lineTypeId: String? {
        get { readField("line_type_id") }
        set { writeField("line_type_id", newValue) }
    }

    /// 線幅。inherit を持たない。定義が線幅を持たなければ書かない。小版 22 から
    public var lineWeight: LineWeightChange? {
        get { readField("line_weight") }
        set { writeField("line_weight", newValue) }
    }
}

/// 線種の表の項目（決定 0164）。小版 26 から、定義を持つ
public struct LineTypeItem: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// ID。表のレコードがハンドルを持てば line-type/h14、なければ読んだ順の番号（line-type/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 元の名前。翻訳しない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかの欄を持たない。小版 26 から
    public var defined: Bool? {
        get { readField("defined") }
        set { writeField("defined", newValue) }
    }

    /// 元の説明。小版 26 から
    public var description: String? {
        get { readField("description") }
        set { writeField("description", newValue) }
    }

    /// 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。小版 26 から
    public var pattern: [Double]? {
        get { readField("pattern") }
        set { writeField("pattern", newValue) }
    }

    /// 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。小版 26 から
    public var paper: Bool? {
        get { readField("paper") }
        set { writeField("paper", newValue) }
    }

    /// 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。小版 26 から
    public var marked: Bool? {
        get { readField("marked") }
        set { writeField("marked", newValue) }
    }

    /// 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。小版 28 から
    public var marks: [LineTypeMark]? {
        get { readField("marks") }
        set { writeField("marks", newValue) }
    }
}

/// 寸法スタイルの表の項目（決定 0165）。小版 27 から、端部記号と大きさを持つ
public struct DimensionStyleItem: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// ID。寸法スタイルのレコードがハンドルを持てば dimension-style/h1A、なければ寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 元の名前。翻訳しない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
    public var ends: [String]? {
        get { readField("ends") }
        set { writeField("ends", newValue) }
    }

    /// 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
    public var leader: String? {
        get { readField("leader") }
        set { writeField("leader", newValue) }
    }

    /// 端部記号の大きさ。文書の単位で、scale を掛ける前の値。小版 27 から
    public var size: Double? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }

    /// 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。小版 27 から
    public var tick: Double? {
        get { readField("tick") }
        set { writeField("tick", newValue) }
    }

    /// 大きさに掛ける倍率。0 はビューの縮尺から決める。小版 27 から
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }
}

/// 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。小版 28 から
public struct LineTypeMark: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 添える列の要素の番号（pattern の添字）
    public var index: Int64? {
        get { readField("index") }
        set { writeField("index", newValue) }
    }

    /// 形（shape）か文字（text）
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 形の形のファイル。元の形式が指す文字スタイルがなければ書かない
    public var font: Font? {
        get { readField("font") }
        set { writeField("font", newValue) }
    }

    /// 形の番号
    public var number: Int64? {
        get { readField("number") }
        set { writeField("number", newValue) }
    }

    /// 文字の文字列
    public var text: String? {
        get { readField("text") }
        set { writeField("text", newValue) }
    }

    /// 文字の文字スタイルの元の名前。元の形式が指す文字スタイルがなければ書かない
    public var style: String? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }

    /// 文字の文字スタイルの ID。tables の text_styles の ID で、名前で引いたもの（大文字と小文字を区別せず、同じ名前があれば先の項目）
    public var styleId: String? {
        get { readField("style_id") }
        set { writeField("style_id", newValue) }
    }

    /// 文字の高さ。文字スタイルが決まった高さを持てば、それに scale を掛けたもの。持たなければ scale。単位は pattern と同じ
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 倍率。文字なら、文字スタイルの高さへの倍率
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }

    /// 回転（度、反時計回り）
    public var rotation: Double? {
        get { readField("rotation") }
        set { writeField("rotation", newValue) }
    }

    /// 回転の基準
    public var rotationBasis: String? {
        get { readField("rotation_basis") }
        set { writeField("rotation_basis", newValue) }
    }

    /// 置く位置のずれ（x・y）。線の向きを x とする。単位は pattern と同じ
    public var offset: [Double]? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }
}

/// 線種に添えるものの種類。shape は形のファイルの形、text は文字
///
/// 列挙の値。知らない値も受け取る
public enum LineTypeMarkKind {
    public static let shape = "shape"
    public static let text = "text"
}

/// 回転の基準。relative は線の向きから、absolute は文書の x 軸から測る。upright は線の向きから測り、文字が逆さに見えるなら半回転する
///
/// 列挙の値。知らない値も受け取る
public enum MarkRotation {
    public static let relative = "relative"
    public static let absolute = "absolute"
    public static let upright = "upright"
}

/// 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。小版 20 から
public struct TextStyleItem: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// ID。表のレコードがハンドルを持てば text-style/h11、なければ文字スタイルの並びの中の順（text-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 元の名前。翻訳しない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// ファイルに定義がある。偽なら、ほかの欄を持たない
    public var defined: Bool? {
        get { readField("defined") }
        set { writeField("defined", newValue) }
    }

    /// 主のフォント。ファイルが決めていなければ書かない
    public var font: Font? {
        get { readField("font") }
        set { writeField("font", newValue) }
    }

    /// 大きいフォント（漢字などの形のファイル）。なければ書かない
    public var bigFont: Font? {
        get { readField("big_font") }
        set { writeField("big_font", newValue) }
    }

    /// 決まった高さ。文書の単位。決めなければ書かない
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 幅の倍率
    public var width: Double? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 傾き（度）
    public var oblique: Double? {
        get { readField("oblique") }
        set { writeField("oblique", newValue) }
    }

    /// 左右を裏返す
    public var backward: Bool? {
        get { readField("backward") }
        set { writeField("backward", newValue) }
    }

    /// 上下を反転する
    public var upsideDown: Bool? {
        get { readField("upside_down") }
        set { writeField("upside_down", newValue) }
    }

    /// 縦書き
    public var vertical: Bool? {
        get { readField("vertical") }
        set { writeField("vertical", newValue) }
    }
}

/// フォントの指定。取得と字形の評価は使い手が行う
public struct Font: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 元の名前。形のファイルならファイルの名前、名前で指すフォントなら書体の名前。翻訳しない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 太字。名前で指すフォントだけ
    public var bold: Bool? {
        get { readField("bold") }
        set { writeField("bold", newValue) }
    }

    /// 斜体。名前で指すフォントだけ
    public var italic: Bool? {
        get { readField("italic") }
        set { writeField("italic", newValue) }
    }

    /// 文字集合の番号（128 は Shift_JIS）。名前で指すフォントで、元の形式が決めていれば
    public var charset: Int64? {
        get { readField("charset") }
        set { writeField("charset", newValue) }
    }

    /// ピッチと系統の番号。名前で指すフォントで、元の形式が決めていれば
    public var pitch: Int64? {
        get { readField("pitch") }
        set { writeField("pitch", newValue) }
    }
}

/// フォントの種類。shape は形のファイル（SHX など）、named は名前で指すフォント（TrueType など）
///
/// 列挙の値。知らない値も受け取る
public enum FontKind {
    public static let shape = "shape"
    public static let named = "named"
}

/// 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つ鍵（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない
public struct Source: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// L1 のレコードの番号
    public var record: Int64? {
        get { readField("record") }
        set { writeField("record", newValue) }
    }

    /// L1 のバイト範囲
    public var bytes: ByteRange? {
        get { readField("bytes") }
        set { writeField("bytes", newValue) }
    }

    /// AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない
    public var handle: String? {
        get { readField("handle") }
        set { writeField("handle", newValue) }
    }

    /// SXF のインスタンスの番号（#n の n）。10 進の文字列。小版 6 から
    public var instance: String? {
        get { readField("instance") }
        set { writeField("instance", newValue) }
    }
}

/// 応答。失敗したときも同じ形で、result を持たない
public struct Response: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// メッセージのスキーマの大版
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 診断。入力の順
    public var diagnostics: [Diagnostic]? {
        get { readField("diagnostics") }
        set { writeField("diagnostics", newValue) }
    }

    /// バイナリの結果。take で取り出す。次の call で捨てる
    public var blobs: [Blob]? {
        get { readField("blobs") }
        set { writeField("blobs", newValue) }
    }

    /// op ごとの結果（<op>_result の形の JSON のオブジェクト）。失敗したときは nil
    public var result: [String: JSON]? {
        get { readField("result") }
        set { writeField("result", newValue) }
    }

    /// op ごとの結果を、その op の結果の型で読む（例: `response.result(as: OpenResult.self)`）。失敗したときは nil
    public func result<T: Message>(as type: T.Type) -> T? {
        readField("result")
    }
}

/// 診断。人向けの文は持たず、文は describe で組み立てる（決定 0009）
public struct Diagnostic: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 変わらない名前（budget.records.exceeded など）。catalog/codes.toml にある
    public var code: String? {
        get { readField("code") }
        set { writeField("code", newValue) }
    }

    /// 重さ。code ごとに決まる
    public var severity: String? {
        get { readField("severity") }
        set { writeField("severity", newValue) }
    }

    /// 変換の結果の段。code ごとに決まる。変換の結果を表さない code では持たない
    public var fidelity: String? {
        get { readField("fidelity") }
        set { writeField("fidelity", newValue) }
    }

    /// まとめた件数
    public var count: Int64? {
        get { readField("count") }
        set { writeField("count", newValue) }
    }

    /// 場所。入力の順に先頭の数件
    public var at: [Place]? {
        get { readField("at") }
        set { writeField("at", newValue) }
    }

    /// 仕様の節
    public var cite: Cite? {
        get { readField("cite") }
        set { writeField("cite", newValue) }
    }

    /// 文に差し込む値。名前と型は code ごとに台帳が決める。2^53 − 1 を超える整数は文字列にする
    public var params: [String: JSON]? {
        get { readField("params") }
        set { writeField("params", newValue) }
    }
}

/// 診断の重さ（決定 0009）。info は何も失っていない、warning は結果を使える、error は失敗した
///
/// 列挙の値。知らない値も受け取る
public enum Severity {
    public static let info = "info"
    public static let warning = "warning"
    public static let error = "error"
}

/// 変換の結果の段のうち、`native` 以外のもの（決定 0009）
///
/// 列挙の値。知らない値も受け取る
public enum Fidelity {
    public static let equivalent = "equivalent"
    public static let approximated = "approximated"
    public static let drawn = "drawn"
    public static let dropped = "dropped"
}

/// 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所）
public struct Place: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// L1 のレコードの番号
    public var record: Int64? {
        get { readField("record") }
        set { writeField("record", newValue) }
    }

    /// L1 のバイト範囲
    public var bytes: ByteRange? {
        get { readField("bytes") }
        set { writeField("bytes", newValue) }
    }
}

/// バイト範囲（start から end の手前まで）
public struct ByteRange: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 先頭の位置
    public var start: Int64? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 終わりの位置（含まない）
    public var end: Int64? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }
}

/// 仕様の節。仕様の本文は写さない
public struct Cite: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 仕様の一覧での名前
    public var spec: String? {
        get { readField("spec") }
        set { writeField("spec", newValue) }
    }

    /// 節
    public var section: String? {
        get { readField("section") }
        set { writeField("section", newValue) }
    }
}

/// バイナリの結果
public struct Blob: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 番号。zm_take に渡す
    public var id: Int64? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// バイト数
    public var size: Int64? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }
}

/// バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）
///
/// 列挙の値。知らない値も受け取る
public enum BlobKind {
    public static let file = "file"
    public static let image = "image"
    public static let scene = "scene"
    public static let arrow = "arrow"
}

/// zm_session_new に渡す設定。渡さなければ既定の設定
public struct SessionOptions: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 実行の形。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013）
    public var engine: String? {
        get { readField("engine") }
        set { writeField("engine", newValue) }
    }

    /// engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200）
    public var processPath: String? {
        get { readField("process_path") }
        set { writeField("process_path", newValue) }
    }
}

/// 実行の形（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
///
/// 列挙の値。知らない値も受け取る
public enum Engine {
    public static let native = "native"
    public static let process = "process"
    public static let wasm = "wasm"
}

/// 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面のモジュールに置く。小版 46 から（doc と id は小版 47 から）
public struct CivilRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("civil")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 線形。doc と一緒には書かない
    public var alignment: CivilAlignment? {
        get { readField("alignment") }
        set { writeField("alignment", newValue) }
    }

    /// open で開いた J-LandXML の文書の名前。小版 47 から
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// doc の線形の ID。文書に線形が 1 つなら書かなくてよい。小版 47 から
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 追加距離（メートル）の列
    public var distances: [Double]? {
        get { readField("distances") }
        set { writeField("distances", newValue) }
    }

    /// 測点の表記（No.12+5.000・No.12+5・No.12）の列。測点の不連続のために同じ値が何か所にもあれば、最初の所にして診断する
    public var stations: [String]? {
        get { readField("stations") }
        set { writeField("stations", newValue) }
    }

    /// 線形の初めから終わりまで、この間隔（メートル）ごとの所を足す。終わりの所も足す
    public var interval: Double? {
        get { readField("interval") }
        set { writeField("interval", newValue) }
    }

    /// 測点の間隔（メートル）。書かなければ 20
    public var pitch: Double? {
        get { readField("pitch") }
        set { writeField("pitch", newValue) }
    }

    /// 測点の表記の端数の小数の桁数。書かなければ 3
    public var digits: Int64? {
        get { readField("digits") }
        set { writeField("digits", newValue) }
    }
}

/// 線形。点は地図の空間（北と東、メートル）に置き、方向角は北から時計回りのラジアンにする。半径は、右へ曲がるものを正にする（決定 0230）
public struct CivilAlignment: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 平面線形の区間。線形の初めから順に並べる
    public var horizontal: [CivilHorizontal]? {
        get { readField("horizontal") }
        set { writeField("horizontal", newValue) }
    }

    /// 縦断線形の区間。書かなければ高さを返さない
    public var vertical: [CivilVertical]? {
        get { readField("vertical") }
        set { writeField("vertical", newValue) }
    }

    /// 線形の初めの測点の値（メートル）。書かなければ 0
    public var startStation: Double? {
        get { readField("start_station") }
        set { writeField("start_station", newValue) }
    }

    /// 測点の不連続。追加距離の順に並べる
    public var breaks: [CivilStationBreak]? {
        get { readField("breaks") }
        set { writeField("breaks", newValue) }
    }
}

/// 平面線形の区間
public struct CivilHorizontal: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 区間の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 初めの点の北（メートル）
    public var northing: Double? {
        get { readField("northing") }
        set { writeField("northing", newValue) }
    }

    /// 初めの点の東（メートル）
    public var easting: Double? {
        get { readField("easting") }
        set { writeField("easting", newValue) }
    }

    /// 初めの方向角（北から時計回りのラジアン）
    public var direction: Double? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// 長さ（メートル）
    public var length: Double? {
        get { readField("length") }
        set { writeField("length", newValue) }
    }

    /// 円弧の半径（メートル）。arc に書く
    public var radius: Double? {
        get { readField("radius") }
        set { writeField("radius", newValue) }
    }

    /// 緩和曲線の初めの半径（メートル）。直線の側は書かない
    public var startRadius: Double? {
        get { readField("start_radius") }
        set { writeField("start_radius", newValue) }
    }

    /// 緩和曲線の終わりの半径（メートル）。直線の側は書かない
    public var endRadius: Double? {
        get { readField("end_radius") }
        set { writeField("end_radius", newValue) }
    }

    /// 緩和曲線の種類。transition に書き、書かなければ clothoid
    public var curve: String? {
        get { readField("curve") }
        set { writeField("curve", newValue) }
    }
}

/// 縦断線形の区間
public struct CivilVertical: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 区間の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 初めの追加距離（メートル）
    public var startDistance: Double? {
        get { readField("start_distance") }
        set { writeField("start_distance", newValue) }
    }

    /// 初めの高さ（メートル）
    public var startHeight: Double? {
        get { readField("start_height") }
        set { writeField("start_height", newValue) }
    }

    /// 初めの勾配（高さ ÷ 平面の距離）
    public var startGradient: Double? {
        get { readField("start_gradient") }
        set { writeField("start_gradient", newValue) }
    }

    /// 平面の距離での長さ（メートル）
    public var length: Double? {
        get { readField("length") }
        set { writeField("length", newValue) }
    }

    /// 放物線の終わりの勾配。parabola に書く
    public var endGradient: Double? {
        get { readField("end_gradient") }
        set { writeField("end_gradient", newValue) }
    }

    /// 円弧の半径（メートル）。上に凸のものを正にする。circular_arc に書く
    public var radius: Double? {
        get { readField("radius") }
        set { writeField("radius", newValue) }
    }
}

/// 平面線形の区間の種類。直線・円弧・緩和曲線
///
/// 列挙の値。知らない値も受け取る
public enum CivilHorizontalKind {
    public static let line = "line"
    public static let arc = "arc"
    public static let transition = "transition"
}

/// 緩和曲線の種類。クロソイド・3 次放物線・サイン半波長逓減曲線・コサイン曲線・ブロス曲線・ヘルマート曲線・ウィーン曲線
///
/// 列挙の値。知らない値も受け取る
public enum CivilTransitionCurve {
    public static let clothoid = "clothoid"
    public static let cubicParabola = "cubic_parabola"
    public static let sineHalfWave = "sine_half_wave"
    public static let cosine = "cosine"
    public static let bloss = "bloss"
    public static let helmert = "helmert"
    public static let viennese = "viennese"
}

/// 縦断線形の区間の種類。一定の勾配・2 次放物線・円弧
///
/// 列挙の値。知らない値も受け取る
public enum CivilVerticalKind {
    public static let constantGradient = "constant_gradient"
    public static let parabola = "parabola"
    public static let circularArc = "circular_arc"
}

/// 測点の不連続
public struct CivilStationBreak: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 追加距離（メートル）
    public var distance: Double? {
        get { readField("distance") }
        set { writeField("distance", newValue) }
    }

    /// 戻りの測点の値（不連続の手前から数えた値）
    public var back: Double? {
        get { readField("back") }
        set { writeField("back", newValue) }
    }

    /// 進みの測点の値（不連続の先で数え直す値）
    public var ahead: Double? {
        get { readField("ahead") }
        set { writeField("ahead", newValue) }
    }
}

/// 線形の上の所
public struct CivilResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 平面線形の長さ（メートル）
    public var length: Double? {
        get { readField("length") }
        set { writeField("length", newValue) }
    }

    /// 求めた所
    public var positions: [CivilPosition]? {
        get { readField("positions") }
        set { writeField("positions", newValue) }
    }

    /// 所を求めた doc の線形の ID。小版 47 から
    public var alignment: String? {
        get { readField("alignment") }
        set { writeField("alignment", newValue) }
    }

    /// doc の線形の要約。doc を渡したときに返す。小版 47 から
    public var alignments: [CivilAlignmentSummary]? {
        get { readField("alignments") }
        set { writeField("alignments", newValue) }
    }

    /// doc の地表面（TIN）の要約。doc を渡したときに返す。小版 47 から
    public var surfaces: [CivilSurfaceSummary]? {
        get { readField("surfaces") }
        set { writeField("surfaces", newValue) }
    }

    /// doc の水平の座標系の EPSG のコード。分からなければ書かない。小版 47 から
    public var crs: Int64? {
        get { readField("crs") }
        set { writeField("crs", newValue) }
    }
}

/// 線形の要約
public struct CivilAlignmentSummary: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 線形の ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 平面線形の長さ（メートル）
    public var length: Double? {
        get { readField("length") }
        set { writeField("length", newValue) }
    }

    /// 線形の初めの測点の値（メートル）
    public var startStation: Double? {
        get { readField("start_station") }
        set { writeField("start_station", newValue) }
    }

    /// 平面線形の区間の数
    public var horizontal: Int64? {
        get { readField("horizontal") }
        set { writeField("horizontal", newValue) }
    }

    /// 縦断線形の区間の数
    public var vertical: Int64? {
        get { readField("vertical") }
        set { writeField("vertical", newValue) }
    }

    /// 測点の不連続の数
    public var breaks: Int64? {
        get { readField("breaks") }
        set { writeField("breaks", newValue) }
    }

    /// 横断の数
    public var crossSections: Int64? {
        get { readField("cross_sections") }
        set { writeField("cross_sections", newValue) }
    }
}

/// 地表面（TIN）の要約
public struct CivilSurfaceSummary: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 地表面の ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 役割。existing（現況）・design（計画）か、元の形式の値
    public var role: String? {
        get { readField("role") }
        set { writeField("role", newValue) }
    }

    /// 点の数
    public var points: Int64? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 三角形の数
    public var triangles: Int64? {
        get { readField("triangles") }
        set { writeField("triangles", newValue) }
    }
}

/// 線形の上の 1 つの所
public struct CivilPosition: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 追加距離（メートル）
    public var distance: Double? {
        get { readField("distance") }
        set { writeField("distance", newValue) }
    }

    /// 測点の値（メートル）
    public var station: Double? {
        get { readField("station") }
        set { writeField("station", newValue) }
    }

    /// 測点の表記。表記にできなければ書かない
    public var label: String? {
        get { readField("label") }
        set { writeField("label", newValue) }
    }

    /// 点の北（メートル）
    public var northing: Double? {
        get { readField("northing") }
        set { writeField("northing", newValue) }
    }

    /// 点の東（メートル）
    public var easting: Double? {
        get { readField("easting") }
        set { writeField("easting", newValue) }
    }

    /// 接線の方向角（北から時計回りのラジアン）
    public var direction: Double? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// 縦断線形の高さ（メートル）。求められなければ書かない
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 縦断線形の勾配。高さと同じく、求められなければ書かない
    public var gradient: Double? {
        get { readField("gradient") }
        set { writeField("gradient", newValue) }
    }
}

/// open で開いた点群（LAS・LAZ）の要約と、条件に合う点を返す。点は八分木の浅い段の節から順に選ぶ（決定 0232）。図面のモジュールに置く。小版 48 から
public struct PointcloudRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("pointcloud")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// open で開いた点群の文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 点を選ぶ範囲。書かなければ全体
    public var region: PointcloudBox? {
        get { readField("region") }
        set { writeField("region", newValue) }
    }

    /// 許容差（点の間隔、メートル）。これより細かい段の点を選ばない。書かなければすべての段
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 返す点の数の上限。書かなければ点を返さず、要約だけを返す
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// 地図の空間の箱。北・東・標高（メートル）の最小と最大
public struct PointcloudBox: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 最小の北・東・標高
    public var min: [Double]? {
        get { readField("min") }
        set { writeField("min", newValue) }
    }

    /// 最大の北・東・標高
    public var max: [Double]? {
        get { readField("max") }
        set { writeField("max", newValue) }
    }
}

/// 点群の要約と、選んだ点
public struct PointcloudResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 点の数
    public var points: Int64? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 点の範囲。点がなければ書かない
    public var bounds: PointcloudBox? {
        get { readField("bounds") }
        set { writeField("bounds", newValue) }
    }

    /// 水平の座標系の EPSG のコード。分からなければ書かない
    public var crs: Int64? {
        get { readField("crs") }
        set { writeField("crs", newValue) }
    }

    /// 高さの座標系の EPSG のコード。分からなければ書かない
    public var vertical: Int64? {
        get { readField("vertical") }
        set { writeField("vertical", newValue) }
    }

    /// 点が持つ属性（position・intensity・classification・color・returns・time）
    public var attributes: [String]? {
        get { readField("attributes") }
        set { writeField("attributes", newValue) }
    }

    /// 八分木のいちばん深い段
    public var depth: Int64? {
        get { readField("depth") }
        set { writeField("depth", newValue) }
    }

    /// 八分木の節の数
    public var nodes: Int64? {
        get { readField("nodes") }
        set { writeField("nodes", newValue) }
    }

    /// 選んだ点。limit を書いたときに返す
    public var selected: PointcloudPoints? {
        get { readField("selected") }
        set { writeField("selected", newValue) }
    }
}

/// 選んだ点。どの列も、選んだ点の順で同じ長さにする
public struct PointcloudPoints: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 北・東・標高（メートル）
    public var positions: [[Double]]? {
        get { readField("positions") }
        set { writeField("positions", newValue) }
    }

    /// 強度。点群が持つときに返す
    public var intensity: [Int64]? {
        get { readField("intensity") }
        set { writeField("intensity", newValue) }
    }

    /// 分類。点群が持つときに返す
    public var classification: [Int64]? {
        get { readField("classification") }
        set { writeField("classification", newValue) }
    }

    /// 色（赤・緑・青。16 ビット）。点群が持つときに返す
    public var color: [[Int64]]? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// limit のために選ばなかった点がある
    public var truncated: Bool? {
        get { readField("truncated") }
        set { writeField("truncated", newValue) }
    }

    /// 選んだいちばん深い段
    public var level: Int64? {
        get { readField("level") }
        set { writeField("level", newValue) }
    }
}

/// 電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。小版 53 から
public struct DeliveryCheckRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("delivery_check")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 成果品の起点。/ で終わる入力の名前の前の部分（納品/ など）。archive と一緒には書かない
    public var root: String? {
        get { readField("root") }
        set { writeField("root", newValue) }
    }

    /// 成果品の ZIP の入力の名前。root と一緒には書かない
    public var archive: String? {
        get { readField("archive") }
        set { writeField("archive", newValue) }
    }

    /// プロファイルの入力の名前の列。extends で指す先も、すべて渡す
    public var profiles: [String]? {
        get { readField("profiles") }
        set { writeField("profiles", newValue) }
    }

    /// 照合に使うプロファイル。profiles の中から選ぶ
    public var profile: ProfileRef? {
        get { readField("profile") }
        set { writeField("profile", newValue) }
    }

    /// 返す指摘の初めの位置
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す指摘の数の上限。書かなければ 10000
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// 成果品の要約と、照合の結果。照合の結果は check と同じ形
public struct DeliveryCheckResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 成果品のファイルの数
    public var files: Int64? {
        get { readField("files") }
        set { writeField("files", newValue) }
    }

    /// 管理ファイルの要約。名前の順
    public var managed: [DeliveryManaged]? {
        get { readField("managed") }
        set { writeField("managed", newValue) }
    }

    /// 照合に使ったプロファイルの鎖。基から順に。プロファイルを渡さなければ空
    public var chain: [ProfileIdentity]? {
        get { readField("chain") }
        set { writeField("chain", newValue) }
    }

    /// 規則ごとの指摘の数
    public var rules: [CheckRule]? {
        get { readField("rules") }
        set { writeField("rules", newValue) }
    }

    /// 指摘
    public var findings: [CheckFinding]? {
        get { readField("findings") }
        set { writeField("findings", newValue) }
    }

    /// 指摘の数の全体
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 続きの指摘の位置。続きがなければ書かない
    public var nextOffset: Int64? {
        get { readField("next_offset") }
        set { writeField("next_offset", newValue) }
    }
}

/// 管理ファイルの要約
public struct DeliveryManaged: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 成果品の中の名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 文書型宣言の DTD の名前
    public var doctype: String? {
        get { readField("doctype") }
        set { writeField("doctype", newValue) }
    }

    /// 根の要素の名前
    public var root: String? {
        get { readField("root") }
        set { writeField("root", newValue) }
    }

    /// 要素の記録の数
    public var records: Int64? {
        get { readField("records") }
        set { writeField("records", newValue) }
    }

    /// ファイルの名前の形の値の数
    public var links: Int64? {
        get { readField("links") }
        set { writeField("links", newValue) }
    }

    /// 成果品の中に見つからないファイルの名前。書いた順
    public var missing: [String]? {
        get { readField("missing") }
        set { writeField("missing", newValue) }
    }
}

/// open で開いた CityGML の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。図面のモジュールに置く。小版 52 から
public struct GeoRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("geo")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// open で開いた CityGML の文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 中身を返す地物の ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }
}

/// 地理の領域モデルの要約
public struct GeoResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 平面の座標系の EPSG のコード。分からなければ書かない
    public var crs: Int64? {
        get { readField("crs") }
        set { writeField("crs", newValue) }
    }

    /// 高さの座標系の EPSG のコード。分からなければ書かない
    public var height: Int64? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 地物の数
    public var features: Int64? {
        get { readField("features") }
        set { writeField("features", newValue) }
    }

    /// 地物の型ごとの数。型の名前の順
    public var kinds: [GeoKind]? {
        get { readField("kinds") }
        set { writeField("kinds", newValue) }
    }

    /// id で指した地物の中身
    public var feature: GeoFeature? {
        get { readField("feature") }
        set { writeField("feature", newValue) }
    }
}

/// 地物の型と数
public struct GeoKind: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 型。名前空間の接頭辞をつけた元の名前
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 地物の数
    public var count: Int64? {
        get { readField("count") }
        set { writeField("count", newValue) }
    }
}

/// 地物の中身
public struct GeoFeature: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 型
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 分類コード。元の値の文字のまま
    public var code: String? {
        get { readField("code") }
        set { writeField("code", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 親の地物の ID
    public var parent: String? {
        get { readField("parent") }
        set { writeField("parent", newValue) }
    }

    /// 子の地物の数
    public var children: Int64? {
        get { readField("children") }
        set { writeField("children", newValue) }
    }

    /// 属性。入力の順
    public var attributes: [GeoAttribute]? {
        get { readField("attributes") }
        set { writeField("attributes", newValue) }
    }

    /// 形。入力の順
    public var geometries: [GeoGeometry]? {
        get { readField("geometries") }
        set { writeField("geometries", newValue) }
    }
}

/// 属性
public struct GeoAttribute: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 名前。接頭辞をつけた元の名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 値の型
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 値の文字
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }

    /// 単位つきの量の単位
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// コードの一覧（codeSpace）
    public var space: String? {
        get { readField("space") }
        set { writeField("space", newValue) }
    }
}

/// 属性の値の型
///
/// 列挙の値。知らない値も受け取る
public enum GeoValueKind {
    public static let text = "text"
    public static let integer = "integer"
    public static let real = "real"
    public static let bool = "bool"
    public static let date = "date"
    public static let code = "code"
    public static let measure = "measure"
}

/// 形の種類
///
/// 列挙の値。知らない値も受け取る
public enum GeoForm {
    public static let point = "point"
    public static let line = "line"
    public static let area = "area"
    public static let solid = "solid"
}

/// 形の要約
public struct GeoGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// LOD。2D の地物は書かない
    public var lod: Int64? {
        get { readField("lod") }
        set { writeField("lod", newValue) }
    }

    /// 形の種類
    public var form: String? {
        get { readField("form") }
        set { writeField("form", newValue) }
    }

    /// 点の数。立体と面の集まりは、面の輪の点の数の和
    public var points: Int64? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 立体と面の集まりの面の数
    public var faces: Int64? {
        get { readField("faces") }
        set { writeField("faces", newValue) }
    }
}

/// open で開いた J-LandXML の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。小版 51 から
public struct CivilDrawingRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("civil_drawing")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// open で開いた J-LandXML の文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// doc の線形の ID。文書に線形が 1 つなら書かなくてよい
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 書く用紙の名前（結果の sheets の 1 つ）。書かなければ平面図
    public var sheet: String? {
        get { readField("sheet") }
        set { writeField("sheet", newValue) }
    }

    /// 作る文書の名前
    public var to: String? {
        get { readField("to") }
        set { writeField("to", newValue) }
    }

    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    public var profiles: [String]? {
        get { readField("profiles") }
        set { writeField("profiles", newValue) }
    }

    /// 作図の値に使うプロファイル。profiles の中から選ぶ。profiles と一緒に書く
    public var profile: ProfileRef? {
        get { readField("profile") }
        set { writeField("profile", newValue) }
    }
}

/// 作った文書。書いたファイルは blobs の file で返す
public struct CivilDrawingResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 作った文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 書いた形式の名前。今は dxf
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 書いたファイルのハッシュ。apply の前提条件に使う
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 書いた要素の数
    public var elements: Int64? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }

    /// 図を作った線形の ID
    public var alignment: String? {
        get { readField("alignment") }
        set { writeField("alignment", newValue) }
    }

    /// 作った用紙の名前。図を描ける順（平面図・縦断図・横断図）
    public var sheets: [String]? {
        get { readField("sheets") }
        set { writeField("sheets", newValue) }
    }

    /// 書いた用紙の名前
    public var sheet: String? {
        get { readField("sheet") }
        set { writeField("sheet", newValue) }
    }

    /// 使った作図の値
    public var options: CivilDrawingOptions? {
        get { readField("options") }
        set { writeField("options", newValue) }
    }

    /// 作図の値を読んだプロファイルの、extends を解いた鎖。基から順。プロファイルを渡さなければ書かない
    public var profile: [ProfileIdentity]? {
        get { readField("profile") }
        set { writeField("profile", newValue) }
    }
}

/// 土木の図の作図の値
public struct CivilDrawingOptions: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 平面図の尺度の分母
    public var plan: Double? {
        get { readField("plan") }
        set { writeField("plan", newValue) }
    }

    /// 縦断図の横の尺度の分母
    public var horizontal: Double? {
        get { readField("horizontal") }
        set { writeField("horizontal", newValue) }
    }

    /// 縦断図の縦の尺度の分母
    public var vertical: Double? {
        get { readField("vertical") }
        set { writeField("vertical", newValue) }
    }

    /// 横断図の尺度の分母
    public var crossSection: Double? {
        get { readField("cross_section") }
        set { writeField("cross_section", newValue) }
    }

    /// 文字の高さ（mm）
    public var textHeight: Double? {
        get { readField("text_height") }
        set { writeField("text_height", newValue) }
    }

    /// 測点の間隔（メートル）
    public var stationPitch: Double? {
        get { readField("station_pitch") }
        set { writeField("station_pitch", newValue) }
    }
}

/// 開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。図面のモジュールに置く。小版 50 から
public struct DeviationRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 目印の欄だけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("deviation")]
    }

    /// メッセージのスキーマの大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    public var budget: Budget? {
        get { readField("budget") }
        set { writeField("budget", newValue) }
    }

    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    public var partial: Bool? {
        get { readField("partial") }
        set { writeField("partial", newValue) }
    }

    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    public var maxPlaces: Int64? {
        get { readField("max_places") }
        set { writeField("max_places", newValue) }
    }

    /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// open で開いた点群の文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 設計面の TIN
    public var surface: DeviationSurface? {
        get { readField("surface") }
        set { writeField("surface", newValue) }
    }

    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    public var profiles: [String]? {
        get { readField("profiles") }
        set { writeField("profiles", newValue) }
    }

    /// 規格値に使うプロファイル。profiles の中から選ぶ
    public var profile: ProfileRef? {
        get { readField("profile") }
        set { writeField("profile", newValue) }
    }

    /// 工種の名前。規則の work と比べ、同じものだけを使う
    public var work: String? {
        get { readField("work") }
        set { writeField("work", newValue) }
    }

    /// 返す最初の評価した点の位置。前の応答の next_offset を渡す。書かなければ 0
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す評価した点の数の上限。書かなければ点を返さない
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// 設計面の TIN を持つ文書と、TIN の ID
public struct DeviationSurface: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// open で開いた土木の文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// TIN の ID。civil の surfaces の id
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }
}

/// 標高の差の、規則の区分ごとの集計と指摘。合格・適合とは言わない
public struct DeviationResult: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 使ったプロファイルの鎖。extends の基から順
    public var chain: [ProfileIdentity]? {
        get { readField("chain") }
        set { writeField("chain", newValue) }
    }

    /// 点群の点の数
    public var points: Int64? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// TIN の外なので評価しなかった点の数
    public var outside: Int64? {
        get { readField("outside") }
        set { writeField("outside", newValue) }
    }

    /// どの規則の勾配にも入らない三角形の上なので評価しなかった点の数
    public var unmatched: Int64? {
        get { readField("unmatched") }
        set { writeField("unmatched", newValue) }
    }

    /// 区分の境の近く（規則の exclude の内）なので除いた点の数
    public var excluded: Int64? {
        get { readField("excluded") }
        set { writeField("excluded", newValue) }
    }

    /// 規則ごとの集計。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
    public var rules: [DeviationRule]? {
        get { readField("rules") }
        set { writeField("rules", newValue) }
    }

    /// 指摘。規則の順。規格値を外れれば profile.surface-elevation.invalid、確認できなければ profile.surface-elevation.unsupported
    public var findings: [CheckFinding]? {
        get { readField("findings") }
        set { writeField("findings", newValue) }
    }

    /// 評価した点の数
    public var evaluated: Int64? {
        get { readField("evaluated") }
        set { writeField("evaluated", newValue) }
    }

    /// 評価した点。点群の順に、offset から limit 個まで
    public var differences: [DeviationPoint]? {
        get { readField("differences") }
        set { writeField("differences", newValue) }
    }

    /// 続きの最初の評価した点の位置。続きがなければ持たない
    public var nextOffset: Int64? {
        get { readField("next_offset") }
        set { writeField("next_offset", newValue) }
    }
}

/// 規則の区分ごとの集計。差はメートル
public struct DeviationRule: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 規則の id
    public var rule: String? {
        get { readField("rule") }
        set { writeField("rule", newValue) }
    }

    /// 区分の名前
    public var part: String? {
        get { readField("part") }
        set { writeField("part", newValue) }
    }

    /// 評価した点の数
    public var points: Int64? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 区分の三角形の、平面に投影した面積（m²）
    public var area: Double? {
        get { readField("area") }
        set { writeField("area", newValue) }
    }

    /// 計測の密度（点／m²）。面積がなければ書かない
    public var density: Double? {
        get { readField("density") }
        set { writeField("density", newValue) }
    }

    /// 差の平均。点がなければ書かない
    public var mean: Double? {
        get { readField("mean") }
        set { writeField("mean", newValue) }
    }

    /// 差の最小。点がなければ書かない
    public var min: Double? {
        get { readField("min") }
        set { writeField("min", newValue) }
    }

    /// 差の最大。点がなければ書かない
    public var max: Double? {
        get { readField("max") }
        set { writeField("max", newValue) }
    }

    /// 指摘の数（確認できなかったものを除く）
    public var findings: Int64? {
        get { readField("findings") }
        set { writeField("findings", newValue) }
    }

    /// 確認できなかった数
    public var unsupported: Int64? {
        get { readField("unsupported") }
        set { writeField("unsupported", newValue) }
    }
}

/// 評価した点
public struct DeviationPoint: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 点群の中の番号
    public var index: Int64? {
        get { readField("index") }
        set { writeField("index", newValue) }
    }

    /// 北・東・標高（メートル）
    public var position: [Double]? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 点の標高から設計面の標高を引いた差（メートル）
    public var difference: Double? {
        get { readField("difference") }
        set { writeField("difference", newValue) }
    }

    /// 評価した規則の id
    public var rule: String? {
        get { readField("rule") }
        set { writeField("rule", newValue) }
    }
}

/// describe に使うカタログのファイル。catalog/<言語>.toml から生成し、zm_push で渡す（決定 0009）
public struct Catalog: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// カタログの形の大版。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 言語（ja・en）
    public var lang: String? {
        get { readField("lang") }
        set { writeField("lang", newValue) }
    }

    /// パッケージの版。コアと同じ版で出す
    public var `package`: String? {
        get { readField("package") }
        set { writeField("package", newValue) }
    }

    /// 数の書き方
    public var number: NumberFormat? {
        get { readField("number") }
        set { writeField("number", newValue) }
    }

    /// code ごとの文のひな形。{名前} に値が、{count} にまとめた件数が入る
    public var codes: [String: String]? {
        get { readField("codes") }
        set { writeField("codes", newValue) }
    }
}

/// 文に差し込む数の書き方。OS のロケールは使わない（決定 0009）
public struct NumberFormat: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// 空のオブジェクトから作る
    public init() {
        json = [:]
    }

    /// 整数部を 3 桁ごとに区切る文字。空なら区切らない
    public var groupSeparator: String? {
        get { readField("group_separator") }
        set { writeField("group_separator", newValue) }
    }

    /// 小数点の文字
    public var decimalSeparator: String? {
        get { readField("decimal_separator") }
        set { writeField("decimal_separator", newValue) }
    }
}

/// op の名前。要求の型は `<Op>Request`、結果の型は `<Op>Result`
public enum Operations {
    public static let capabilities = "capabilities"
    public static let `open` = "open"
    public static let close = "close"
    public static let describe = "describe"
    public static let drawing = "drawing"
    public static let entities = "entities"
    public static let apply = "apply"
    public static let convert = "convert"
    public static let tables = "tables"
    public static let scene = "scene"
    public static let check = "check"
    public static let validate = "validate"
    public static let reviewCarry = "review_carry"
    public static let reviewAdopt = "review_adopt"
    public static let building = "building"
    public static let buildingElements = "building_elements"
    public static let idsCheck = "ids_check"
    public static let reviewAdoptIds = "review_adopt_ids"
    public static let bcfRead = "bcf_read"
    public static let bcfWrite = "bcf_write"
    public static let civil = "civil"
    public static let pointcloud = "pointcloud"
    public static let deviation = "deviation"
    public static let civilDrawing = "civil_drawing"
    public static let geo = "geo"
    public static let deliveryCheck = "delivery_check"
}
