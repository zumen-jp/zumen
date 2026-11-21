// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。
//
// API のメッセージの型（決定 0012・0225）。スキーマの定義ごとに 1 つの型がある。型は JSON のオブジェクトを包む
// ビューで、知らないフィールドもそのまま持つ。列挙は文字列で読み、知らない値も受け取る（決定 0015）。

/// 要求。op の値で形が決まる
public struct Request: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// op が capabilities の形なら、そのビュー。違えば nil
    public var asCapabilitiesRequest: CapabilitiesRequest? {
        taggedView("op", "capabilities")
    }

    /// op が open の形なら、そのビュー。違えば nil
    public var asOpenRequest: OpenRequest? {
        taggedView("op", "open")
    }

    /// op が close の形なら、そのビュー。違えば nil
    public var asCloseRequest: CloseRequest? {
        taggedView("op", "close")
    }

    /// op が describe の形なら、そのビュー。違えば nil
    public var asDescribeRequest: DescribeRequest? {
        taggedView("op", "describe")
    }

    /// op が drawing の形なら、そのビュー。違えば nil
    public var asDrawingRequest: DrawingRequest? {
        taggedView("op", "drawing")
    }

    /// op が entities の形なら、そのビュー。違えば nil
    public var asEntitiesRequest: EntitiesRequest? {
        taggedView("op", "entities")
    }

    /// op が calibrations の形なら、そのビュー。違えば nil
    public var asCalibrationsRequest: CalibrationsRequest? {
        taggedView("op", "calibrations")
    }

    /// op が measure_distance の形なら、そのビュー。違えば nil
    public var asMeasureDistanceRequest: MeasureDistanceRequest? {
        taggedView("op", "measure_distance")
    }

    /// op が measure の形なら、そのビュー。違えば nil
    public var asMeasureRequest: MeasureRequest? {
        taggedView("op", "measure")
    }

    /// op が intersect の形なら、そのビュー。違えば nil
    public var asIntersectRequest: IntersectRequest? {
        taggedView("op", "intersect")
    }

    /// op が regions の形なら、そのビュー。違えば nil
    public var asRegionsRequest: RegionsRequest? {
        taggedView("op", "regions")
    }

    /// op が construct の形なら、そのビュー。違えば nil
    public var asConstructRequest: ConstructRequest? {
        taggedView("op", "construct")
    }

    /// op が apply の形なら、そのビュー。違えば nil
    public var asApplyRequest: ApplyRequest? {
        taggedView("op", "apply")
    }

    /// op が create の形なら、そのビュー。違えば nil
    public var asCreateRequest: CreateRequest? {
        taggedView("op", "create")
    }

    /// op が convert の形なら、そのビュー。違えば nil
    public var asConvertRequest: ConvertRequest? {
        taggedView("op", "convert")
    }

    /// op が convert_estimate の形なら、そのビュー。違えば nil
    public var asConvertEstimateRequest: ConvertEstimateRequest? {
        taggedView("op", "convert_estimate")
    }

    /// op が tables の形なら、そのビュー。違えば nil
    public var asTablesRequest: TablesRequest? {
        taggedView("op", "tables")
    }

    /// op が scene の形なら、そのビュー。違えば nil
    public var asSceneRequest: SceneRequest? {
        taggedView("op", "scene")
    }

    /// op が scene3d の形なら、そのビュー。違えば nil
    public var asScene3dRequest: Scene3dRequest? {
        taggedView("op", "scene3d")
    }

    /// op が check の形なら、そのビュー。違えば nil
    public var asCheckRequest: CheckRequest? {
        taggedView("op", "check")
    }

    /// op が validate の形なら、そのビュー。違えば nil
    public var asValidateRequest: ValidateRequest? {
        taggedView("op", "validate")
    }

    /// op が review_carry の形なら、そのビュー。違えば nil
    public var asReviewCarryRequest: ReviewCarryRequest? {
        taggedView("op", "review_carry")
    }

    /// op が review_adopt の形なら、そのビュー。違えば nil
    public var asReviewAdoptRequest: ReviewAdoptRequest? {
        taggedView("op", "review_adopt")
    }

    /// op が review_scene の形なら、そのビュー。違えば nil
    public var asReviewSceneRequest: ReviewSceneRequest? {
        taggedView("op", "review_scene")
    }

    /// op が building の形なら、そのビュー。違えば nil
    public var asBuildingRequest: BuildingRequest? {
        taggedView("op", "building")
    }

    /// op が building_elements の形なら、そのビュー。違えば nil
    public var asBuildingElementsRequest: BuildingElementsRequest? {
        taggedView("op", "building_elements")
    }

    /// op が building_drawing の形なら、そのビュー。違えば nil
    public var asBuildingDrawingRequest: BuildingDrawingRequest? {
        taggedView("op", "building_drawing")
    }

    /// op が ids_check の形なら、そのビュー。違えば nil
    public var asIdsCheckRequest: IdsCheckRequest? {
        taggedView("op", "ids_check")
    }

    /// op が review_adopt_ids の形なら、そのビュー。違えば nil
    public var asReviewAdoptIdsRequest: ReviewAdoptIdsRequest? {
        taggedView("op", "review_adopt_ids")
    }

    /// op が bcf_read の形なら、そのビュー。違えば nil
    public var asBcfReadRequest: BcfReadRequest? {
        taggedView("op", "bcf_read")
    }

    /// op が bcf_write の形なら、そのビュー。違えば nil
    public var asBcfWriteRequest: BcfWriteRequest? {
        taggedView("op", "bcf_write")
    }

    /// op が civil の形なら、そのビュー。違えば nil
    public var asCivilRequest: CivilRequest? {
        taggedView("op", "civil")
    }

    /// op が pointcloud の形なら、そのビュー。違えば nil
    public var asPointcloudRequest: PointcloudRequest? {
        taggedView("op", "pointcloud")
    }

    /// op が deviation の形なら、そのビュー。違えば nil
    public var asDeviationRequest: DeviationRequest? {
        taggedView("op", "deviation")
    }

    /// op が civil_drawing の形なら、そのビュー。違えば nil
    public var asCivilDrawingRequest: CivilDrawingRequest? {
        taggedView("op", "civil_drawing")
    }

    /// op が geo の形なら、そのビュー。違えば nil
    public var asGeoRequest: GeoRequest? {
        taggedView("op", "geo")
    }

    /// op が delivery_check の形なら、そのビュー。違えば nil
    public var asDeliveryCheckRequest: DeliveryCheckRequest? {
        taggedView("op", "delivery_check")
    }

    /// op が delivery_apply の形なら、そのビュー。違えば nil
    public var asDeliveryApplyRequest: DeliveryApplyRequest? {
        taggedView("op", "delivery_apply")
    }

    /// op が photo の形なら、そのビュー。違えば nil
    public var asPhotoRequest: PhotoRequest? {
        taggedView("op", "photo")
    }

    /// op が boring の形なら、そのビュー。違えば nil
    public var asBoringRequest: BoringRequest? {
        taggedView("op", "boring")
    }

    /// op が building_apply の形なら、そのビュー。違えば nil
    public var asBuildingApplyRequest: BuildingApplyRequest? {
        taggedView("op", "building_apply")
    }

    /// op が lift の形なら、そのビュー。違えば nil
    public var asLiftRequest: LiftRequest? {
        taggedView("op", "lift")
    }

    /// op が render の形なら、そのビュー。違えば nil
    public var asRenderRequest: RenderRequest? {
        taggedView("op", "render")
    }

    /// op が raster の形なら、そのビュー。違えば nil
    public var asRasterRequest: RasterRequest? {
        taggedView("op", "raster")
    }

    /// op が georeference の形なら、そのビュー。違えば nil
    public var asGeoreferenceRequest: GeoreferenceRequest? {
        taggedView("op", "georeference")
    }
}

/// どの op の要求にも置けるフィールド
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

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 図面モデル（L2）が保持する要素・表・拡張の論理バイト数。マイナーバージョン 113 から
    public var modelBytes: JSON? {
        get { readField("model_bytes") }
        set { writeField("model_bytes", newValue) }
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

/// コアのバージョンと、使える op・形式を尋ねる
public struct CapabilitiesRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("capabilities")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

/// コアのバージョンと、使える op・形式（決定 0015）
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

    /// パッケージのバージョン（semver）
    public var `package`: String? {
        get { readField("package") }
        set { writeField("package", newValue) }
    }

    /// ABI のメジャーバージョン。zm_abi_version と同じ
    public var abi: Int64? {
        get { readField("abi") }
        set { writeField("abi", newValue) }
    }

    /// メッセージのスキーマのバージョン
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

/// メッセージのスキーマのバージョン。メジャーバージョンは要求と応答の v、マイナーバージョンは op・フィールド・列挙の値を足すたびに上げる
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

    /// メジャーバージョン
    public var major: Int64? {
        get { readField("major") }
        set { writeField("major", newValue) }
    }

    /// マイナーバージョン
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

    /// create で空の文書を作れるか。マイナーバージョン 83 から
    public var create: Bool? {
        get { readField("create") }
        set { writeField("create", newValue) }
    }

    /// この形式の ID の規則のバージョン（決定 0015）
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

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("open")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。マイナーバージョン 48 から
    public var size: Int64? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }

    /// 見分けるマーカーのない形式（xyz・extdm・geojson）を名指して読む。書かなければ、入力から判定する（決定 0232）。マイナーバージョン 49 から。extdm はマイナーバージョン 62 から。geojson はマイナーバージョン 71 から
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// SXF の属性ファイル（SAF）を入れた入力の名前。zm_push で渡したもの。属性ファイル用の属性付加機構（$$ATRF$$）の要素に、属性を sxf.saf の拡張で結ぶ（決定 0249）。書かなければ、P2Z・SFZ の中の、図面と同じ名前の SAF を使う。SXF でない入力には使えない。マイナーバージョン 63 から
    public var saf: String? {
        get { readField("saf") }
        set { writeField("saf", newValue) }
    }

    /// IFC-SPF・ifcZIP の 8 ビットのエスケープを読む文字コード。省略時は auto。文書の building_apply 後も指定を保つ。IFC 以外の入力には使えない。マイナーバージョン 76 から
    public var ifcTextEncoding: String? {
        get { readField("ifc_text_encoding") }
        set { writeField("ifc_text_encoding", newValue) }
    }

    /// COPC を、この条件に掛かる八分木の節の範囲だけで読む。足りない範囲は needs で求める。作った文書は読んだ節の点だけを持ち、pointcloud・scene3d はその点を扱う。条件を変えるときは開き直す。書かなければ全体を読む。COPC でない入力には使えない（#899、決定 0232）。マイナーバージョン 104 から
    public var pointcloud: OpenPointcloud? {
        get { readField("pointcloud") }
        set { writeField("pointcloud", newValue) }
    }
}

/// COPC の八分木の節を選ぶ条件。pointcloud の op の同じ名前のフィールドと同じ規則で節を選ぶ
public struct OpenPointcloud: Message {
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

    /// 節を選ぶ範囲。書かなければ全体
    public var region: PointcloudBox? {
        get { readField("region") }
        set { writeField("region", newValue) }
    }

    /// 許容差（点の間隔、メートル）。これより細かい段の節を読まない。書かなければすべての段
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 点の数の上限。節を浅い段から順に数え、範囲にすべて入る節の点の数の和が上限に届いたら、それより後の節を読まない。範囲に一部だけ掛かる節は数えない。書かなければ上限なし
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// IFC の文字列の読み方。auto はファイル内に CP932 の手がかりがあれば読み替える。iso_8859_1 は従来の ISO 8859-1 と Unicode、shift_jis は手がかりなしでも CP932 として読める文字列を読み替える。どの指定も元のバイト列を保つ
///
/// 列挙の値。知らない値も受け取る
public enum IfcTextEncoding {
    public static let auto = "auto"
    public static let iso88591 = "iso_8859_1"
    public static let shiftJis = "shift_jis"
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

    /// 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。マイナーバージョン 48 から
    public var needs: [InputRange]? {
        get { readField("needs") }
        set { writeField("needs", newValue) }
    }

    /// 読み終えたので、close の inputs で捨ててよい範囲の入力。マイナーバージョン 48 から
    public var done: [InputRange]? {
        get { readField("done") }
        set { writeField("done", newValue) }
    }

    /// SXF を ZIP に入れたアーカイブ（P2Z・SFZ）なら、SXF の項目のほかの項目（属性ファイル・画像など）のアーカイブの中の名前。アーカイブの順。apply は、SXF の項目だけを置き換え、ほかの項目を写したアーカイブを書く（#385）。マイナーバージョン 54 から
    public var entries: [String]? {
        get { readField("entries") }
        set { writeField("entries", newValue) }
    }

    /// open の pointcloud の上限のために、読まなかった節がある。COPC を条件で読んだときだけ書く（#899）。マイナーバージョン 104 から
    public var truncated: Bool? {
        get { readField("truncated") }
        set { writeField("truncated", newValue) }
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

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("close")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("describe")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合テストで比べるためにも使う（決定 0007）
public struct DrawingRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("drawing")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。マイナーバージョン 39 から
    public var pointStyle: PointStyleSummary? {
        get { readField("point_style") }
        set { writeField("point_style", newValue) }
    }

    /// 図面の実寸の空間から地図の座標系への相似の変換。DXF・DWG の GEODATA から読めたときだけ持つ。マイナーバージョン 112 から
    public var georeference: Georeference? {
        get { readField("georeference") }
        set { writeField("georeference", newValue) }
    }
}

/// 図面の実寸から地図の東・北・高さ（m）への相似の変換。東は scale * (x_axis_abscissa * x - x_axis_ordinate * y) + eastings、北は scale * (x_axis_ordinate * x + x_axis_abscissa * y) + northings、高さは scale * z + orthogonal_height
public struct MapConversion: Message {
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

    /// 東への移動（m）
    public var eastings: Double? {
        get { readField("eastings") }
        set { writeField("eastings", newValue) }
    }

    /// 北への移動（m）
    public var northings: Double? {
        get { readField("northings") }
        set { writeField("northings", newValue) }
    }

    /// 高さへの移動（m）
    public var orthogonalHeight: Double? {
        get { readField("orthogonal_height") }
        set { writeField("orthogonal_height", newValue) }
    }

    /// 図面の X 軸の東方向の単位ベクトル成分
    public var xAxisAbscissa: Double? {
        get { readField("x_axis_abscissa") }
        set { writeField("x_axis_abscissa", newValue) }
    }

    /// 図面の X 軸の北方向の単位ベクトル成分
    public var xAxisOrdinate: Double? {
        get { readField("x_axis_ordinate") }
        set { writeField("x_axis_ordinate", newValue) }
    }

    /// 図面の 1 単位あたりの地図のメートル数
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }
}

/// 地図の座標系と、図面の実寸から地図への相似の変換
public struct Georeference: Message {
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

    /// 地図の座標系の EPSG のコード
    public var crs: Int64? {
        get { readField("crs") }
        set { writeField("crs", newValue) }
    }

    /// 図面の実寸から地図への相似の変換
    public var conversion: MapConversion? {
        get { readField("conversion") }
        set { writeField("conversion", newValue) }
    }
}

/// 地図の投影座標（m）。東（easting）と北（northing）を名前で分ける
public struct MapPoint2: Message {
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

    /// 東方向の座標（m。平面直角座標系の Y）
    public var easting: Double? {
        get { readField("easting") }
        set { writeField("easting", newValue) }
    }

    /// 北方向の座標（m。平面直角座標系の X）
    public var northing: Double? {
        get { readField("northing") }
        set { writeField("northing", newValue) }
    }
}

/// 図面の実寸の点と、対応する地図の投影座標の組
public struct GeoreferenceControlPoint: Message {
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

    /// 図面の実寸の点
    public var drawing: Point2? {
        get { readField("drawing") }
        set { writeField("drawing", newValue) }
    }

    /// 対応する地図の投影座標（m）
    public var map: MapPoint2? {
        get { readField("map") }
        set { writeField("map", newValue) }
    }
}

/// 開いた図面の GEODATA、または対応点と地図の平面直角座標系から、図面の実寸から地図への相似の変換を求め、必要なら図面の点を地図の座標へ写す。マイナーバージョン 112 から
public struct GeoreferenceRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("georeference")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 開いた図面の文書の名前。GEODATA から変換を読むときに渡し、crs・control_points と一緒には渡せない
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// doc に期待する元の文書のハッシュ。doc と一緒にだけ渡せる
    public var expectedHash: String? {
        get { readField("expected_hash") }
        set { writeField("expected_hash", newValue) }
    }

    /// 対応点の地図の平面直角座標系の EPSG のコード（6669〜6687・2443〜2461・30161〜30179）。control_points と一緒に渡す
    public var crs: Int64? {
        get { readField("crs") }
        set { writeField("crs", newValue) }
    }

    /// 図面の実寸の点と地図の投影座標の組（2 組以上）。最小二乗で相似の変換を求める
    public var controlPoints: [GeoreferenceControlPoint]? {
        get { readField("control_points") }
        set { writeField("control_points", newValue) }
    }

    /// 求めた変換で地図へ写す図面の実寸の点
    public var points: [Point2]? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }
}

/// 図面の実寸から地図への相似の変換と、対応点の残差・写した地図の点
public struct GeoreferenceResult: Message {
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

    /// doc を渡したときの元の文書のハッシュ
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 地図の座標系と、図面の実寸から地図への相似の変換。doc の GEODATA から作れなかったときは持たない
    public var georeference: Georeference? {
        get { readField("georeference") }
        set { writeField("georeference", newValue) }
    }

    /// control_points の組ごとの残差（渡した地図の点から、変換で写した図面の点を引いた投影座標。m）。渡した組の順
    public var residuals: [MapPoint2]? {
        get { readField("residuals") }
        set { writeField("residuals", newValue) }
    }

    /// points を変換で地図へ写した投影座標（m）。変換が作れなかったときは持たない
    public var mapped: [MapPoint2]? {
        get { readField("mapped") }
        set { writeField("mapped", newValue) }
    }
}

/// 2 次元の座標
public struct Point2: Message {
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

    /// X 座標
    public var x: Double? {
        get { readField("x") }
        set { writeField("x", newValue) }
    }

    /// Y 座標
    public var y: Double? {
        get { readField("y") }
        set { writeField("y", newValue) }
    }
}

/// 原点と相対座標を分けた点。両成分の正確な和を位置とし、先に足して丸めない。座標の単位は使う所に従う（measure_distance では用紙の mm、measure では from の空間か用紙の単位）
public struct SplitPoint2: Message {
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

    /// 用紙の原点
    public var origin: Point2? {
        get { readField("origin") }
        set { writeField("origin", newValue) }
    }

    /// 原点からの相対座標
    public var offset: Point2? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }
}

/// 用紙の閉じた矩形で使う距離の校正。重なる領域は ID で明示的に選ぶ。マイナーバージョン 68 から
public struct CalibrationItem: Message {
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

    /// 文書内で重複しない校正の ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 元の校正の名前。翻訳しない。なければ持たない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 校正を使える公開用紙の ID。drawing が返すもの
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 用紙の mm の閉じた矩形。[X の最小, Y の最小, X の最大, Y の最大] の 4 値で、幅と高さは正
    public var bounds: [Double]? {
        get { readField("bounds") }
        set { writeField("bounds", newValue) }
    }

    /// 用紙の mm の変位から校正単位の変位へ写す行優先の 2×2 行列。[a, b, c, d] の 4 値で、[dx, dy] を [a*dx+b*dy, c*dx+d*dy] に写す
    public var matrix: [Double]? {
        get { readField("matrix") }
        set { writeField("matrix", newValue) }
    }

    /// 校正後の長さの単位。ラベルだけから推定しない
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// 元の単位ラベル。未知の単位でも原文を保つ
    public var unitLabel: String? {
        get { readField("unit_label") }
        set { writeField("unit_label", newValue) }
    }

    /// 元の形式のレコード
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 開いた文書を図面モデルに変換し、用紙の距離の校正をモデルの順に返す。マイナーバージョン 68 から
public struct CalibrationsRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("calibrations")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 公開用紙の ID。drawing が返すもの。書けばその用紙の校正だけを返し、書かなければすべての校正を返す
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 返す最初の校正の位置。from で絞った後の位置で、前の応答の next_offset を渡す。書かなければ 0
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す校正の数の上限。書かなければ 1000
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }

    /// 期待する元の文書のハッシュ。open に渡したバイト列のハッシュと照合する。図面モデルへ変換した ZMDR 自身のハッシュではない
    public var expectedHash: String? {
        get { readField("expected_hash") }
        set { writeField("expected_hash", newValue) }
    }
}

/// 距離の校正の一覧の一部
public struct CalibrationsResult: Message {
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

    /// 元の文書のハッシュ。ZMDR から開いたときは、埋め込まれた元の文書のハッシュ（Source.hash）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// from で絞った一覧の offset から limit 個までの校正
    public var items: [CalibrationItem]? {
        get { readField("items") }
        set { writeField("items", newValue) }
    }

    /// from で絞った後の校正の総数
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 続きの最初の校正の位置。続きがなければ持たない
    public var nextOffset: Int64? {
        get { readField("next_offset") }
        set { writeField("next_offset", newValue) }
    }
}

/// 用紙と校正を明示的に選び、始点と終点の校正後の距離を求める。両点は校正の閉領域内にある必要がある。マイナーバージョン 68 から
public struct MeasureDistanceRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("measure_distance")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 公開用紙の ID。drawing が返すもので、選んだ校正と同じ用紙
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 使う校正の ID。calibrations が返すもの
    public var calibration: String? {
        get { readField("calibration") }
        set { writeField("calibration", newValue) }
    }

    /// 始点。原点と相対座標を分けた用紙の mm の点
    public var start: SplitPoint2? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 終点。原点と相対座標を分けた用紙の mm の点
    public var end: SplitPoint2? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 期待する元の文書のハッシュ。open に渡したバイト列のハッシュと照合する。図面モデルへ変換した ZMDR 自身のハッシュではない
    public var expectedHash: String? {
        get { readField("expected_hash") }
        set { writeField("expected_hash", newValue) }
    }
}

/// 選んだ用紙と校正による距離
public struct MeasureDistanceResult: Message {
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

    /// 元の文書のハッシュ。ZMDR から開いたときは、埋め込まれた元の文書のハッシュ（Source.hash）
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 計測した公開用紙の ID
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 使った校正の ID
    public var calibration: String? {
        get { readField("calibration") }
        set { writeField("calibration", newValue) }
    }

    /// 校正後の距離。unit と unit_label が示す単位
    public var distance: Double? {
        get { readField("distance") }
        set { writeField("distance", newValue) }
    }

    /// 校正後の長さの単位
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// 元の単位ラベル。未知の単位でも原文を保つ
    public var unitLabel: String? {
        get { readField("unit_label") }
        set { writeField("unit_label", newValue) }
    }
}

/// モデル空間の要素の長さ・周長・面積、2 要素または点の間の最短距離を測る。線幅と表示用の変換は含めない
public struct MeasureRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("measure")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 開いた文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 実寸の空間の公開 ID。記号定義の空間は対象外。マイナーバージョン 102 から、distance で両方の対象が点なら公開用紙の ID も使え、校正しない用紙の座標の距離を返す
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 求める値
    public var quantity: String? {
        get { readField("quantity") }
        set { writeField("quantity", newValue) }
    }

    /// 計測する要素または点
    public var first: MeasureTarget? {
        get { readField("first") }
        set { writeField("first", newValue) }
    }

    /// 最短距離のもう一方の要素または点。distance のときだけ必須
    public var second: MeasureTarget? {
        get { readField("second") }
        set { writeField("second", newValue) }
    }

    /// 近似する曲線の分割で目標にする弦の誤差。空間の長さの単位で正の値。既定は 0.001。計測した長さ・面積・距離の誤差上限ではない
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
    public var expectedHash: String? {
        get { readField("expected_hash") }
        set { writeField("expected_hash", newValue) }
    }
}

/// 長さ、閉じた経路の周長、向き付き線積分の絶対値の面積、境界間の最短距離。面積は自己交差を塗り規則で統合しない
///
/// 列挙の値。知らない値も受け取る
public enum MeasureQuantity {
    public static let length = "length"
    public static let perimeter = "perimeter"
    public static let area = "area"
    public static let distance = "distance"
}

/// 同じモデル空間内の要素または座標
public struct MeasureTarget: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// kind が element の形なら、そのビュー。違えば nil
    public var asMeasureElement: MeasureElement? {
        taggedView("kind", "element")
    }

    /// kind が point の形なら、そのビュー。違えば nil
    public var asMeasurePoint: MeasurePoint? {
        taggedView("kind", "point")
    }

    /// kind が split_point の形なら、そのビュー。違えば nil
    public var asMeasureSplitPoint: MeasureSplitPoint? {
        taggedView("kind", "split_point")
    }
}

/// 計測する要素。元の公開 ID を使う
public struct MeasureElement: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("element")]
    }

    /// 対象の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 同じ空間の entities が返した要素の ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }
}

/// 距離の計測で使う空間の座標
public struct MeasurePoint: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("point")]
    }

    /// 対象の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 空間の長さの単位の点
    public var position: Point2? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }
}

/// 距離の計測で使う、原点と相対座標を分けた座標。distance で、もう一方も点のときだけ使える。マイナーバージョン 102 から
public struct MeasureSplitPoint: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("split_point")]
    }

    /// 対象の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 原点と相対座標を分けた点。成分ごとの差を正確に求めてから一度だけ丸める
    public var position: SplitPoint2? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }
}

/// 要素から求めた計測値。近似値かどうか、単位と出所の空間を明示する
public struct MeasureResult: Message {
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

    /// 元の文書のハッシュ
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 計測した実寸の空間の公開 ID
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 求めた値の種類
    public var quantity: String? {
        get { readField("quantity") }
        set { writeField("quantity", newValue) }
    }

    /// 計測値。0 以上の有限値
    public var value: Double? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }

    /// 空間の長さの単位。不明な単位を補わない。from が用紙なら用紙の mm
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// from が用紙で、value が校正しない用紙の座標の距離なら true。実寸ではない。実寸の空間では返さない。マイナーバージョン 102 から
    public var sheet: Bool? {
        get { readField("sheet") }
        set { writeField("sheet", newValue) }
    }

    /// 単位の次数。長さと距離は 1、面積は 2
    public var unitPower: Int64? {
        get { readField("unit_power") }
        set { writeField("unit_power", newValue) }
    }

    /// 計測に曲線の折れ線近似を使った。浮動小数の丸めとは区別する
    public var approximate: Bool? {
        get { readField("approximate") }
        set { writeField("approximate", newValue) }
    }

    /// 指定した、または既定の分割の目標。近似しないときも返す
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 最短距離を作る最初の対象上の点。distance のときだけ返す。split_point なら原点と相対座標の和を丸めた点
    public var nearestFirst: Point2? {
        get { readField("nearest_first") }
        set { writeField("nearest_first", newValue) }
    }

    /// 最短距離を作るもう一方の対象上の点。近似した対象では折れ線上の点
    public var nearestSecond: Point2? {
        get { readField("nearest_second") }
        set { writeField("nearest_second", newValue) }
    }
}

/// 同じ実寸の空間の 2 要素の交点・接点・重なりを求める。文書を変更しない。マイナーバージョン 90 から
public struct IntersectRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("intersect")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 開いた文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 交わりを求める要素
    public var first: MeasureElement? {
        get { readField("first") }
        set { writeField("first", newValue) }
    }

    /// 交わりを求めるもう一方の要素。first と異なる ID
    public var second: MeasureElement? {
        get { readField("second") }
        set { writeField("second", newValue) }
    }

    /// 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、近似する曲線の分割の目標にも使う
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
    public var expectedHash: String? {
        get { readField("expected_hash") }
        set { writeField("expected_hash", newValue) }
    }
}

/// 交点または接点。格子点の座標
public struct IntersectionPoint: Message {
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

    /// 空間の長さの単位の点
    public var position: Point2? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 接点。同じ格子点にまとめたすべての交わりが接するときだけ真
    public var tangent: Bool? {
        get { readField("tangent") }
        set { writeField("tangent", newValue) }
    }

    /// 正確な交点が格子点でなく、近い格子点へ丸めた。まとめたすべての交わりが丸めたときだけ真
    public var rounded: Bool? {
        get { readField("rounded") }
        set { writeField("rounded", newValue) }
    }
}

/// 重なりの形。segment は線分、arc は反時計回りの円弧、circle は円全体
///
/// 列挙の値。知らない値も受け取る
public enum IntersectionOverlapKind {
    public static let segment = "segment"
    public static let arc = "arc"
    public static let circle = "circle"
}

/// 2 要素が長さを持って重なる区間。区間の組ごとに返し、つながる区間をつながない
public struct IntersectionOverlap: Message {
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

    /// 重なりの形
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 始点。segment は X、Y の辞書順で小さい端、arc は反時計回りの始点。circle では持たない
    public var start: Point2? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 終点。circle では持たない
    public var end: Point2? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 円弧と円の中心。segment では持たない
    public var center: Point2? {
        get { readField("center") }
        set { writeField("center", newValue) }
    }

    /// 円弧と円の格子上の半径。segment では持たない
    public var radius: Double? {
        get { readField("radius") }
        set { writeField("radius", newValue) }
    }
}

/// 2 要素の交わり。first と second を入れ替えても同じ結果を返す
public struct IntersectResult: Message {
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

    /// 元の文書のハッシュ
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 求めた実寸の空間の公開 ID
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 空間の長さの単位。不明な単位を補わない
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// 指定した、または既定の許容差
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 曲線を折れ線に近似した要素がある
    public var approximate: Bool? {
        get { readField("approximate") }
        set { writeField("approximate", newValue) }
    }

    /// 交点と接点。格子点の X、Y の辞書順。重なりの端点と同じ点は含めない
    public var points: [IntersectionPoint]? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 重なり。形・座標の順
    public var overlaps: [IntersectionOverlap]? {
        get { readField("overlaps") }
        set { writeField("overlaps", newValue) }
    }
}

/// 同じ実寸の空間の要素の線が囲む閉領域を検出する、または閉じた要素どうしの和・積・差・排他的論理和を求める。文書を変更しない。マイナーバージョン 91 から
public struct RegionsRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("regions")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 開いた文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 求め方
    public var operation: String? {
        get { readField("operation") }
        set { writeField("operation", newValue) }
    }

    /// detect では線で面を囲む要素、ほかでは集合演算の左側の閉じた要素。1 つ以上で、同じ ID を繰り返さない
    public var subject: [MeasureElement]? {
        get { readField("subject") }
        set { writeField("subject", newValue) }
    }

    /// 集合演算の右側の閉じた要素。書かなければ空。detect では書かない。同じ ID を繰り返さない
    public var clip: [MeasureElement]? {
        get { readField("clip") }
        set { writeField("clip", newValue) }
    }

    /// 閉じた要素の内部を決める規則。書かなければ even_odd。detect では書かない
    public var fill: String? {
        get { readField("fill") }
        set { writeField("fill", newValue) }
    }

    /// 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、円弧を弦に分ける目標にも使う
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
    public var expectedHash: String? {
        get { readField("expected_hash") }
        set { writeField("expected_hash", newValue) }
    }
}

/// 閉領域の求め方。detect は subject の線が囲むすべての有限の面、union・intersection・difference・xor は subject と clip の和・積・差（subject から clip を除く）・排他的論理和
///
/// 列挙の値。知らない値も受け取る
public enum RegionOperation {
    public static let detect = "detect"
    public static let union = "union"
    public static let intersection = "intersection"
    public static let difference = "difference"
    public static let xor = "xor"
}

/// 閉じた要素の内部を決める規則。even_odd は境界を横切る回数が奇数の部分、non_zero は符号を付けた周回数が 0 でない部分
///
/// 列挙の値。知らない値も受け取る
public enum RegionFill {
    public static let evenOdd = "even_odd"
    public static let nonZero = "non_zero"
}

/// 外周と穴で表した閉領域。最後の点から最初の点へ閉じる
public struct ClosedRegion: Message {
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

    /// 反時計回りの外周。格子点の座標で、辞書順で最小の点から始める
    public var outer: [Point2]? {
        get { readField("outer") }
        set { writeField("outer", newValue) }
    }

    /// 時計回りの穴。輪郭の辞書順
    public var holes: [[Point2]]? {
        get { readField("holes") }
        set { writeField("holes", newValue) }
    }

    /// 外周から穴を除いた面積。空間の長さの単位の 2 乗
    public var area: Double? {
        get { readField("area") }
        set { writeField("area", newValue) }
    }
}

/// 交点を格子点へ丸めて元の要素の線を曲げた点
public struct RegionRounding: Message {
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

    /// 曲げた要素の公開 ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 丸めた格子点
    public var position: Point2? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }
}

/// 閉領域を作らなかった入力の種類。degenerate は点や一直線に縮んだ要素か長さ 0 の線、open は閉領域の境界に使わなかった線の部分
///
/// 列挙の値。知らない値も受け取る
public enum RegionDiscardKind {
    public static let degenerate = "degenerate"
    public static let `open` = "open"
}

/// 閉領域を作らなかった入力
public struct RegionDiscard: Message {
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

    /// 元の要素の公開 ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 使わなかった部分の始点。open だけが持つ
    public var start: Point2? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 使わなかった部分の終点。open だけが持つ
    public var end: Point2? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }
}

/// 閉領域。要素の順序を入れ替えても同じ領域を返す
public struct RegionsResult: Message {
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

    /// 元の文書のハッシュ
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 求めた実寸の空間の公開 ID
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 空間の長さの単位。不明な単位を補わない
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// 指定した、または既定の許容差
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 曲線を折れ線に近似した要素がある
    public var approximate: Bool? {
        get { readField("approximate") }
        set { writeField("approximate", newValue) }
    }

    /// 閉領域。外周、穴の順の辞書順
    public var regions: [ClosedRegion]? {
        get { readField("regions") }
        set { writeField("regions", newValue) }
    }

    /// 丸めた分割点。要素の指定順、格子点の辞書順
    public var rounded: [RegionRounding]? {
        get { readField("rounded") }
        set { writeField("rounded", newValue) }
    }

    /// 閉領域を作らなかった入力。degenerate、続いて open を元の入力順で返す
    public var discarded: [RegionDiscard]? {
        get { readField("discarded") }
        set { writeField("discarded", newValue) }
    }
}

/// 同じ実寸の空間の要素のオフセット・トリム・延長・面取り・フィレットの結果の形を求める。文書を変更しない。結果は apply の remove・replace・add で当てる。形の種類が変わるときは replace でなく remove と add にする。マイナーバージョン 92 から
public struct ConstructRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("construct")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 開いた文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 作図の種類
    public var operation: String? {
        get { readField("operation") }
        set { writeField("operation", newValue) }
    }

    /// 作図する要素。線分・円・円弧。面取りとフィレットでは 1 本目の線分
    public var target: MeasureElement? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }

    /// 面取りとフィレットの 2 本目の線分。target と端点を共有し、target と異なる ID
    public var second: MeasureElement? {
        get { readField("second") }
        set { writeField("second", newValue) }
    }

    /// トリムと延長の境界の要素。線分・円・円弧・折れ線と、近似する曲線。1 つ以上で、同じ ID と target を含めない
    public var boundaries: [MeasureElement]? {
        get { readField("boundaries") }
        set { writeField("boundaries", newValue) }
    }

    /// トリムで削る区間を選ぶ点。線分は元の直線へ垂直に投影し、円・円弧は中心からの向きで選ぶ
    public var pick: Point2? {
        get { readField("pick") }
        set { writeField("pick", newValue) }
    }

    /// 延長する端点
    public var end: String? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// オフセットの距離。線分は始点から終点へ向かって左、円・円弧は外側を正にする
    public var distance: Double? {
        get { readField("distance") }
        set { writeField("distance", newValue) }
    }

    /// 面取りで、共有する端点から target と second の上を進む正の距離 [target, second]
    public var distances: [Double]? {
        get { readField("distances") }
        set { writeField("distances", newValue) }
    }

    /// フィレットの円弧の正の半径
    public var radius: Double? {
        get { readField("radius") }
        set { writeField("radius", newValue) }
    }

    /// 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、境界の曲線を近似する目標にも使う
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
    public var expectedHash: String? {
        get { readField("expected_hash") }
        set { writeField("expected_hash", newValue) }
    }
}

/// offset は distance だけ平行に移した新しい形、trim は pick を含む区間を最初に当たる境界の間で削った残り、extend は end の端点を最初に当たる境界まで延ばした形、chamfer は target と second の角を distances で切る線分、fillet は radius の接する円弧
///
/// 列挙の値。知らない値も受け取る
public enum ConstructOperation {
    public static let offset = "offset"
    public static let trim = "trim"
    public static let extend = "extend"
    public static let chamfer = "chamfer"
    public static let fillet = "fillet"
}

/// 延長する端点。start は始点を逆向きへ、end は終点を進む向きへ延ばす
///
/// 列挙の値。知らない値も受け取る
public enum ConstructEnd {
    public static let start = "start"
    public static let end = "end"
}

/// 元の要素を置き換える形。空なら要素を消す。2 つなら 1 つ目で置き換えて 2 つ目を足す。円をトリムした円弧のように種類が変わる形は、元の要素を消して足す
public struct ConstructReplacement: Message {
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

    /// 置き換える要素の公開 ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 置き換える形。元の向きの順
    public var geometries: [Geometry]? {
        get { readField("geometries") }
        set { writeField("geometries", newValue) }
    }
}

/// トリムと延長で採用した境界と切断点
public struct ConstructCut: Message {
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

    /// 境界の要素の公開 ID
    public var boundary: String? {
        get { readField("boundary") }
        set { writeField("boundary", newValue) }
    }

    /// 元の交点を丸めた格子点
    public var position: Point2? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 元の交点が格子点でなく、近い格子点へ丸めた
    public var rounded: Bool? {
        get { readField("rounded") }
        set { writeField("rounded", newValue) }
    }

    /// 丸めた点を元の線・円の上へ投影したため、正確な切断位置と異なる
    public var adjusted: Bool? {
        get { readField("adjusted") }
        set { writeField("adjusted", newValue) }
    }
}

/// 作図の結果の形。文書は変わらない
public struct ConstructResult: Message {
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

    /// 元の文書のハッシュ
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 求めた実寸の空間の公開 ID
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 空間の長さの単位。不明な単位を補わない
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// 指定した、または既定の許容差
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 境界の曲線を折れ線に近似した
    public var approximate: Bool? {
        get { readField("approximate") }
        set { writeField("approximate", newValue) }
    }

    /// 置き換える要素。target、second の順
    public var replace: [ConstructReplacement]? {
        get { readField("replace") }
        set { writeField("replace", newValue) }
    }

    /// 足す形。オフセットの形、面取りの線分、フィレットの円弧
    public var add: [Geometry]? {
        get { readField("add") }
        set { writeField("add", newValue) }
    }

    /// トリムと延長で採用した切断点。元の向きの順
    public var cuts: [ConstructCut]? {
        get { readField("cuts") }
        set { writeField("cuts", newValue) }
    }
}

/// 文書の点の表し方。フィールドの意味は point_style_edit と同じ
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

    /// ビューの詳細。元の順に返す。マイナーバージョン 81 から
    public var viewDetails: [ViewDetail]? {
        get { readField("view_details") }
        set { writeField("view_details", newValue) }
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

    /// 円。マイナーバージョン 81 から
    public var circles: Int64? {
        get { readField("circles") }
        set { writeField("circles", newValue) }
    }

    /// 円弧・楕円弧・スプラインなどの曲線。マイナーバージョン 81 から
    public var curves: Int64? {
        get { readField("curves") }
        set { writeField("curves", newValue) }
    }

    /// 折れ線。マイナーバージョン 81 から
    public var polylines: Int64? {
        get { readField("polylines") }
        set { writeField("polylines", newValue) }
    }

    /// 書式つきの文字。マイナーバージョン 81 から
    public var formattedTexts: Int64? {
        get { readField("formatted_texts") }
        set { writeField("formatted_texts", newValue) }
    }

    /// 寸法。マイナーバージョン 81 から
    public var dimensions: Int64? {
        get { readField("dimensions") }
        set { writeField("dimensions", newValue) }
    }

    /// 引出線。マイナーバージョン 81 から
    public var leaders: Int64? {
        get { readField("leaders") }
        set { writeField("leaders", newValue) }
    }

    /// 引出線つきの注記。マイナーバージョン 81 から
    public var callouts: Int64? {
        get { readField("callouts") }
        set { writeField("callouts", newValue) }
    }

    /// 表。マイナーバージョン 81 から
    public var tables: Int64? {
        get { readField("tables") }
        set { writeField("tables", newValue) }
    }

    /// アンダーレイ。マイナーバージョン 81 から
    public var underlays: Int64? {
        get { readField("underlays") }
        set { writeField("underlays", newValue) }
    }

    /// 記号の配置。中の要素を展開しない。マイナーバージョン 81 から
    public var symbols: Int64? {
        get { readField("symbols") }
        set { writeField("symbols", newValue) }
    }

    /// ハッチング。マイナーバージョン 81 から
    public var hatches: Int64? {
        get { readField("hatches") }
        set { writeField("hatches", newValue) }
    }

    /// 点。マイナーバージョン 81 から
    public var points: Int64? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }
}

/// 2D のビュー。変換行列と切り抜きを保ち、尺度・向き・見えるモデル範囲を決定的に求める
public struct ViewDetail: Message {
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

    /// 対象の実寸の空間の公開 ID
    public var space: String? {
        get { readField("space") }
        set { writeField("space", newValue) }
    }

    /// モデル座標から用紙の mm への [a,b,c,d,e,f]。点を (ax+by+c,dx+ey+f) に写す
    public var matrix: [Double]? {
        get { readField("matrix") }
        set { writeField("matrix", newValue) }
    }

    /// モデルの X 軸・Y 軸の 1 単位に対応する用紙の長さ [SX,SY]（mm）。回転・せん断・反転でも各軸の正の長さを返す
    public var scale: [Double]? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }

    /// モデル単位を mm に直した、各軸の縮尺の比 [SX,SY]。1:100 なら 0.01。単位不明なら持たない
    public var ratio: [Double]? {
        get { readField("ratio") }
        set { writeField("ratio", newValue) }
    }

    /// 用紙の X 軸から見たモデルの X 軸の向き。度、反時計回り、-180〜180
    public var rotation: Double? {
        get { readField("rotation") }
        set { writeField("rotation", newValue) }
    }

    /// 変換が向きを反転する
    public var mirrored: Bool? {
        get { readField("mirrored") }
        set { writeField("mirrored", newValue) }
    }

    /// 用紙の mm での切り抜きの頂点。元の順を保ち、空なら切り抜かない
    public var clip: [Point2]? {
        get { readField("clip") }
        set { writeField("clip", newValue) }
    }

    /// 同じ頂点を逆変換したモデル座標。元の順を保つ
    public var modelClip: [Point2]? {
        get { readField("model_clip") }
        set { writeField("model_clip", newValue) }
    }

    /// model_clip を囲む範囲。切り抜きが空なら持たない。モデルの単位
    public var extent: Extent? {
        get { readField("extent") }
        set { writeField("extent", newValue) }
    }

    /// ビューの出所
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
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

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("entities")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// すべての条件で絞った後の、返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0
    public var offset: Int64? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 返す要素の数の上限。書かなければ 1000
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }

    /// 返すフィールドの名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118）
    public var fields: [String]? {
        get { readField("fields") }
        set { writeField("fields", newValue) }
    }

    /// [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118）
    public var region: [Double]? {
        get { readField("region") }
        set { writeField("region", newValue) }
    }

    /// 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。マイナーバージョン 31 から（決定 0177）
    public var place: String? {
        get { readField("place") }
        set { writeField("place", newValue) }
    }

    /// 要素の公開 ID の一覧。いずれかに一致する要素だけを返す。空なら要素を返さず、ない ID は無視する。重複しても要素を増やさない。ほかの条件とも合わせる。マイナーバージョン 80 から
    public var ids: [String]? {
        get { readField("ids") }
        set { writeField("ids", newValue) }
    }

    /// tables.layers の公開 ID の一覧。名前ではない。いずれかのレイヤの要素だけを返す。空なら要素を返さず、ない ID は無視する。ほかの条件とも合わせる。マイナーバージョン 80 から
    public var layers: [String]? {
        get { readField("layers") }
        set { writeField("layers", newValue) }
    }

    /// 要素の種類の一覧。いずれかの種類だけを返す。空なら要素を返さず、重複しても要素を増やさない。ほかの条件とも合わせる。マイナーバージョン 80 から
    public var kinds: [String]? {
        get { readField("kinds") }
        set { writeField("kinds", newValue) }
    }

    /// 出所の条件。指定したすべての項目を満たす要素だけを返す。ほかの条件とも合わせる。マイナーバージョン 80 から
    public var source: EntitySourceFilter? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 元の要素の出所による条件。空のオブジェクトなら制限しない。出所を持たない要素は指定した条件を満たさない
public struct EntitySourceFilter: Message {
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

    /// L1 のレコード番号と完全に一致する
    public var record: Int64? {
        get { readField("record") }
        set { writeField("record", newValue) }
    }

    /// L1 のバイト範囲との重なり。start を含み end を含まない、空でない範囲。境界が接するだけなら一致しない
    public var bytes: ByteRange? {
        get { readField("bytes") }
        set { writeField("bytes", newValue) }
    }

    /// AutoCAD のハンドル。接頭辞なし、1〜16 桁の 16 進の文字列。大文字と小文字を区別せず、数値で比べる。instance とは同時に指定できない
    public var handle: String? {
        get { readField("handle") }
        set { writeField("handle", newValue) }
    }

    /// SXF のインスタンス番号。1〜20 桁の符号なし 64 bit 整数の 10 進文字列。数値で比べる。handle とは同時に指定できない
    public var instance: String? {
        get { readField("instance") }
        set { writeField("instance", newValue) }
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

    /// 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つキー（ハンドル）があればキーで作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どのバージョンでも同じ ID を返す（決定 0117・0156）
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

    /// 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点かフィット点で決まるスプライン・寸法・引出線（LEADER）・点・引出線つきの注記・表（ACAD_TABLE）だけが持つ。apply の replace と add に渡せる形（決定 0139）。マイナーバージョン 10 から
    public var geometry: Geometry? {
        get { readField("geometry") }
        set { writeField("geometry", newValue) }
    }

    /// 要素自身のスタイル。fields に style を指定したときだけ返す。マイナーバージョン 79 から
    public var style: EntityStyle? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }

    /// 要素に付く拡張の一覧。fields に extensions を指定したときだけ返し、拡張がなければ空の一覧。マイナーバージョン 79 から
    public var extensions: [EntityExtension]? {
        get { readField("extensions") }
        set { writeField("extensions", newValue) }
    }
}

/// entities で返す要素のフィールド（決定 0118）
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
    public static let style = "style"
    public static let extensions = "extensions"
}

/// 要素自身が持つスタイル。継承は解かず、色表の RGB も補わない。色・線種・線幅の形は restyle と同じ。透過性は読むだけで、restyle の指定には含めない
public struct EntityStyle: Message {
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

    /// 色の符号・図面内 RGB、または継承元
    public var color: ColorChange? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// 継承元か、tables が返す線種の公開 ID
    public var lineType: LineTypeChange? {
        get { readField("line_type") }
        set { writeField("line_type", newValue) }
    }

    /// 要素自身の線種の倍率。文書や配置の倍率を掛けない
    public var lineTypeScale: Double? {
        get { readField("line_type_scale") }
        set { writeField("line_type_scale", newValue) }
    }

    /// 線幅の指定。元の形式から読めていなければ持たない
    public var lineWeight: LineWeightChange? {
        get { readField("line_weight") }
        set { writeField("line_weight", newValue) }
    }

    /// 透過性の指定
    public var transparency: EntityTransparency? {
        get { readField("transparency") }
        set { writeField("transparency", newValue) }
    }
}

/// 透過性。inherit または alpha の一方を返す
public struct EntityTransparency: Message {
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

    /// 継承元
    public var inherit: String? {
        get { readField("inherit") }
        set { writeField("inherit", newValue) }
    }

    /// 不透明度。0 は透明、255 は不透明
    public var alpha: Int64? {
        get { readField("alpha") }
        set { writeField("alpha", newValue) }
    }
}

/// 要素に付く形式固有の情報。元の順序を保ち、失効した値も current を偽にして返す。書き戻す操作ではない
public struct EntityExtension: Message {
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

    /// 値の型を定める名前空間
    public var namespace: String? {
        get { readField("namespace") }
        set { writeField("namespace", newValue) }
    }

    /// 元の名前。翻訳しない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 元の形式のバージョン。不明なら持たない
    public var version: String? {
        get { readField("version") }
        set { writeField("version", newValue) }
    }

    /// 名前空間の値の型のバージョン
    public var schema: Int64? {
        get { readField("schema") }
        set { writeField("schema", newValue) }
    }

    /// 拡張が依存する元の要素のハッシュ。要素に依存するハッシュがなければ持たない
    public var basis: String? {
        get { readField("basis") }
        set { writeField("basis", newValue) }
    }

    /// basis が現在の要素のハッシュと一致する
    public var current: Bool? {
        get { readField("current") }
        set { writeField("current", newValue) }
    }

    /// 元の順序と型を保つフィールド
    public var fields: [EntityExtensionField]? {
        get { readField("fields") }
        set { writeField("fields", newValue) }
    }

    /// 拡張の出所
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 拡張のフィールド。同じキーの重複と並びを保つ
public struct EntityExtensionField: Message {
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

    /// 元のキー
    public var key: String? {
        get { readField("key") }
        set { writeField("key", newValue) }
    }

    /// 元の形式の値の番号。番号がなければ持たない
    public var code: Int64? {
        get { readField("code") }
        set { writeField("code", newValue) }
    }

    /// 型付きの値
    public var value: EntityExtensionValue? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 拡張の値。整数・ハンドル・バイト列は文字列で正確に返し、実数と点は有限値だけを返す
public struct EntityExtensionValue: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// kind が text の形なら、そのビュー。違えば nil
    public var asEntityExtensionText: EntityExtensionText? {
        taggedView("kind", "text")
    }

    /// kind が integer の形なら、そのビュー。違えば nil
    public var asEntityExtensionInteger: EntityExtensionInteger? {
        taggedView("kind", "integer")
    }

    /// kind が real の形なら、そのビュー。違えば nil
    public var asEntityExtensionReal: EntityExtensionReal? {
        taggedView("kind", "real")
    }

    /// kind が point の形なら、そのビュー。違えば nil
    public var asEntityExtensionPoint: EntityExtensionPoint? {
        taggedView("kind", "point")
    }

    /// kind が handle の形なら、そのビュー。違えば nil
    public var asEntityExtensionHandle: EntityExtensionHandle? {
        taggedView("kind", "handle")
    }

    /// kind が bytes の形なら、そのビュー。違えば nil
    public var asEntityExtensionBytes: EntityExtensionBytes? {
        taggedView("kind", "bytes")
    }
}

/// 元の文字列
public struct EntityExtensionText: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("text")]
    }

    /// 値の型
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 文字列
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 符号付き 64 bit 整数
public struct EntityExtensionInteger: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("integer")]
    }

    /// 値の型
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 10 進の文字列。負数以外には符号を付けず、先頭の 0 を付けない
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 有限の実数
public struct EntityExtensionReal: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("real")]
    }

    /// 値の型
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 実数
    public var value: Double? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 3 次元の点やベクトル。単位と空間は名前空間が定める
public struct EntityExtensionPoint: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("point")]
    }

    /// 値の型
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// [X, Y, Z] の有限値
    public var value: [Double]? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 元の形式のオブジェクトを指す符号なし 64 bit の値
public struct EntityExtensionHandle: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("handle")]
    }

    /// 値の型
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 大文字の 16 進の文字列。接頭辞と先頭の 0 を付けない
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 元のバイト列
public struct EntityExtensionBytes: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("bytes")]
    }

    /// 値の型
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 1 バイトを小文字の 16 進 2 桁にした文字列。空も保つ
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる
public struct Geometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// shape が line の形なら、そのビュー。違えば nil
    public var asLineGeometry: LineGeometry? {
        taggedView("shape", "line")
    }

    /// shape が circle の形なら、そのビュー。違えば nil
    public var asCircleGeometry: CircleGeometry? {
        taggedView("shape", "circle")
    }

    /// shape が arc の形なら、そのビュー。違えば nil
    public var asArcGeometry: ArcGeometry? {
        taggedView("shape", "arc")
    }

    /// shape が ellipse の形なら、そのビュー。違えば nil
    public var asEllipseGeometry: EllipseGeometry? {
        taggedView("shape", "ellipse")
    }

    /// shape が polyline の形なら、そのビュー。違えば nil
    public var asPolylineGeometry: PolylineGeometry? {
        taggedView("shape", "polyline")
    }

    /// shape が text の形なら、そのビュー。違えば nil
    public var asTextGeometry: TextGeometry? {
        taggedView("shape", "text")
    }

    /// shape が formatted_text の形なら、そのビュー。違えば nil
    public var asFormattedTextGeometry: FormattedTextGeometry? {
        taggedView("shape", "formatted_text")
    }

    /// shape が symbol の形なら、そのビュー。違えば nil
    public var asSymbolGeometry: SymbolGeometry? {
        taggedView("shape", "symbol")
    }

    /// shape が spline の形なら、そのビュー。違えば nil
    public var asSplineGeometry: SplineGeometry? {
        taggedView("shape", "spline")
    }

    /// shape が dimension の形なら、そのビュー。違えば nil
    public var asDimensionGeometry: DimensionGeometry? {
        taggedView("shape", "dimension")
    }

    /// shape が leader の形なら、そのビュー。違えば nil
    public var asLeaderGeometry: LeaderGeometry? {
        taggedView("shape", "leader")
    }

    /// shape が point の形なら、そのビュー。違えば nil
    public var asPointGeometry: PointGeometry? {
        taggedView("shape", "point")
    }

    /// shape が callout の形なら、そのビュー。違えば nil
    public var asCalloutGeometry: CalloutGeometry? {
        taggedView("shape", "callout")
    }

    /// shape が table の形なら、そのビュー。違えば nil
    public var asTableGeometry: TableGeometry? {
        taggedView("shape", "table")
    }

    /// shape が hatch の形なら、そのビュー。違えば nil
    public var asHatchGeometry: HatchGeometry? {
        taggedView("shape", "hatch")
    }
}

/// 非連動のハッチング。閉じた折れ線（膨らみ付き）と円を境界にできる。塗りは単色か、線の定義を明示した模様。参照・勾配・自動検出した境界は扱わない。DXF と DWG に追加できる。置き換え・変換はまだ扱わない。entities は用紙上の模様を返さない。用紙への模様の追加は、原点が 0 で 1 単位が 1 mm のときに限る。マイナーバージョン 75 から
public struct HatchGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("hatch")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 境界の輪。入力の順。1 つ以上
    public var loops: [HatchLoop]? {
        get { readField("loops") }
        set { writeField("loops", newValue) }
    }

    /// 輪の中の輪の埋め方
    public var islands: String? {
        get { readField("islands") }
        set { writeField("islands", newValue) }
    }

    /// 中を埋めるもの
    public var fill: String? {
        get { readField("fill") }
        set { writeField("fill", newValue) }
    }

    /// fill が pattern のときだけ持つ模様の定義
    public var pattern: HatchPattern? {
        get { readField("pattern") }
        set { writeField("pattern", newValue) }
    }

    /// 境界を探した点 [[X, Y], ...]。書かなければ空
    public var seeds: [[Double]]? {
        get { readField("seeds") }
        set { writeField("seeds", newValue) }
    }
}

/// ハッチングの境界の輪。参照を持たない
public struct HatchLoop: Message {
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

    /// 閉じた折れ線か円
    public var boundary: HatchBoundary? {
        get { readField("boundary") }
        set { writeField("boundary", newValue) }
    }

    /// 外側の輪
    public var external: Bool? {
        get { readField("external") }
        set { writeField("external", newValue) }
    }

    /// いちばん外の輪
    public var outermost: Bool? {
        get { readField("outermost") }
        set { writeField("outermost", newValue) }
    }
}

/// 輪の形。折れ線は closed が真で頂点が 2 つ以上。最後の点から最初の点へ結ぶ
public struct HatchBoundary: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// shape が polyline の形なら、そのビュー。違えば nil
    public var asPolylineGeometry: PolylineGeometry? {
        taggedView("shape", "polyline")
    }

    /// shape が circle の形なら、そのビュー。違えば nil
    public var asCircleGeometry: CircleGeometry? {
        taggedView("shape", "circle")
    }
}

/// nested は外から奇数番目の内側、outer はいちばん外と次の輪の間、ignore は内側の輪を無視して埋める
///
/// 列挙の値。知らない値も受け取る
public enum HatchIslands {
    public static let nested = "nested"
    public static let outer = "outer"
    public static let ignore = "ignore"
}

/// solid は要素の色で塗り、pattern は線の組で埋める
///
/// 列挙の値。知らない値も受け取る
public enum HatchFill {
    public static let solid = "solid"
    public static let pattern = "pattern"
}

/// 模様の名前と線の定義。外部の定義ファイルを読まず、lines の値を使う
public struct HatchPattern: Message {
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

    /// 元の名前。空と制御文字を含むものは受けない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 定義の出どころ
    public var origin: String? {
        get { readField("origin") }
        set { writeField("origin", newValue) }
    }

    /// 定義に加えた回転（度）
    public var angle: Double? {
        get { readField("angle") }
        set { writeField("angle", newValue) }
    }

    /// 定義に掛けた尺度か、利用者が決めた線の間隔。0 より大きい
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }

    /// 線の組を 90 度回して重ねる
    public var double: Bool? {
        get { readField("double") }
        set { writeField("double", newValue) }
    }

    /// angle と scale を当てた後の線の組。1 つ以上
    public var lines: [HatchPatternLine]? {
        get { readField("lines") }
        set { writeField("lines", newValue) }
    }
}

/// user_defined は利用者が決めた平行線、predefined は形式が名前で決めた定義、custom は利用者の定義
///
/// 列挙の値。知らない値も受け取る
public enum HatchPatternOrigin {
    public static let userDefined = "user_defined"
    public static let predefined = "predefined"
    public static let custom = "custom"
}

/// 1 組の平行線。基点・ずれ・線の長さは要素の座標と単位に従う
public struct HatchPatternLine: Message {
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

    /// 線の向き（度）
    public var angle: Double? {
        get { readField("angle") }
        set { writeField("angle", newValue) }
    }

    /// 1 本の線が通る点 [X, Y]
    public var base: [Double]? {
        get { readField("base") }
        set { writeField("base", newValue) }
    }

    /// 隣の線までのずれ [X, Y]。両方 0 にはしない
    public var offset: [Double]? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 正は線、負は間、0 は点。空なら実線
    public var dashes: [Double]? {
        get { readField("dashes") }
        set { writeField("dashes", newValue) }
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

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。マイナーバージョン 11 から
public struct EllipseGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせを元の値のままにし、書かない幅の倍率・斜体・反転も元の値を受け継ぐ。足すときは、2 点目がなければ書かない位置合わせを左の基準線にし、幅の倍率 1・斜体なし・反転なしにする
public struct TextGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。マイナーバージョン 17 から
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 2 点の間への合わせ方。2 点目（end）と一緒に書く。マイナーバージョン 17 から
    public var fit: String? {
        get { readField("fit") }
        set { writeField("fit", newValue) }
    }

    /// 1 点に置く文字の水平の位置合わせ。left のときは entities が省き、足すときも省けば left。2 点の間に合わせる文字には書けない。置き換えるときは元と同じ値だけを受ける。マイナーバージョン 69 から
    public var horizontal: String? {
        get { readField("horizontal") }
        set { writeField("horizontal", newValue) }
    }

    /// 1 点に置く文字の垂直の位置合わせ。baseline のときは entities が省き、足すときも省けば baseline。2 点の間に合わせる文字には書けない。置き換えるときは元と同じ値だけを受ける。マイナーバージョン 69 から
    public var vertical: String? {
        get { readField("vertical") }
        set { writeField("vertical", newValue) }
    }

    /// 高さ。0 より大きい
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 高さ（height）を用紙の mm で表すか。実寸の空間の文字が用紙の長さで高さを持つとき（JWW）、entities は真を返す。足す・置き換えるときに真を書けば、高さを用紙の mm として渡す。偽のときは entities が省き、省けば高さを点と同じ単位とする。マイナーバージョン 93 から
    public var heightOnPaper: Bool? {
        get { readField("height_on_paper") }
        set { writeField("height_on_paper", newValue) }
    }

    /// 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない
    public var rotation: Double? {
        get { readField("rotation") }
        set { writeField("rotation", newValue) }
    }

    /// 幅の倍率。0 より大きい。1 のときは entities が省き、足すときも省けば 1、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
    public var widthFactor: Double? {
        get { readField("width_factor") }
        set { writeField("width_factor", newValue) }
    }

    /// 斜体の角度（度）。0 のときは entities が省き、足すときも省けば 0、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
    public var oblique: Double? {
        get { readField("oblique") }
        set { writeField("oblique", newValue) }
    }

    /// 左右を裏返すか。偽のときは entities が省き、足すときも省けば偽、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
    public var backward: Bool? {
        get { readField("backward") }
        set { writeField("backward", newValue) }
    }

    /// 上下を反転するか。偽のときは entities が省き、足すときも省けば偽、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
    public var upsideDown: Bool? {
        get { readField("upside_down") }
        set { writeField("upside_down", newValue) }
    }

    /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、省けば元の名前を保ち、DXF・DWG は定義のある別の文字スタイルも受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。マイナーバージョン 12 から
    public var style: String? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }

    /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。マイナーバージョン 23 から
    public var styleId: String? {
        get { readField("style_id") }
        set { writeField("style_id", newValue) }
    }
}

/// 1 点に置く文字の水平の位置合わせ。middle は水平と垂直の両方を外形の中央に合わせる（DXF の 72 = 4）
///
/// 列挙の値。知らない値も受け取る
public enum TextHorizontal {
    public static let left = "left"
    public static let center = "center"
    public static let right = "right"
    public static let middle = "middle"
}

/// 1 点に置く文字の垂直の位置合わせ
///
/// 列挙の値。知らない値も受け取る
public enum TextVertical {
    public static let baseline = "baseline"
    public static let bottom = "bottom"
    public static let middle = "middle"
    public static let top = "top"
}

/// 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
///
/// 列挙の値。知らない値も受け取る
public enum TextFit {
    public static let aligned = "aligned"
    public static let fit = "fit"
}

/// 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味を元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。マイナーバージョン 13 から
public struct FormattedTextGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、省けば元の名前を保ち、DXF・DWG は定義のある別の文字スタイルも受ける。style_id と一緒に書くなら、同じ文字スタイルを指す
    public var style: String? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }

    /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。マイナーバージョン 23 から
    public var styleId: String? {
        get { readField("style_id") }
        set { writeField("style_id", newValue) }
    }
}

/// スプライン（NURBS）。制御点で決まるもの（マイナーバージョン 15 から）と、フィット点で決まるもの（マイナーバージョン 18 から）がある。フィット点で決まるものは、ノットと制御点を空にし、フィット点（fit）と、あれば両端の接線を持つ。元の図形に制御点やノットもあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものとフィット点で決まるものを入れ替えない
public struct SplineGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// ノットの並び。周期的でなければ、制御点の数と次数と 1 の和の数。フィット点で決まるものは空
    public var knots: [Double]? {
        get { readField("knots") }
        set { writeField("knots", newValue) }
    }

    /// 制御点の [X, Y] の並び。次数より多い。フィット点で決まるものは空
    public var control: [[Double]]? {
        get { readField("control") }
        set { writeField("control", newValue) }
    }

    /// 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない
    public var weights: [Double]? {
        get { readField("weights") }
        set { writeField("weights", newValue) }
    }

    /// フィット点の [X, Y] の並び。2 つ以上。書けば、ノットと制御点は空で、重みは書かない。マイナーバージョン 18 から
    public var fit: [[Double]]? {
        get { readField("fit") }
        set { writeField("fit", newValue) }
    }

    /// フィット点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。マイナーバージョン 18 から
    public var startTangent: [Double]? {
        get { readField("start_tangent") }
        set { writeField("start_tangent", newValue) }
    }

    /// フィット点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。マイナーバージョン 18 から
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

/// 寸法。種類（kind）ごとに、その種類の定義点のフィールドを持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。置き換え（DXF と DWG の DIMENSION・対応する ARC_DIMENSION）では定義点と文字を書き直し、測定値を測り直して表示の部品を作り直す（マイナーバージョン 74 から。それまでは置き換えも change.edit.unsupported だった）。measurement は省くか元と同じ値を書く。通常寸法 7 種は対応する DXF・DWG に、弧長寸法は R2004 以降の対応形式に追加できる。DWG の弧長寸法には ARC_DIMENSION クラスが必要。マイナーバージョン 19 から
public struct DimensionGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// 元の形式が保存した測定値。単位と倍率は measurement_key のフィールドに従う
    public var measurement: Double? {
        get { readField("measurement") }
        set { writeField("measurement", newValue) }
    }

    /// measurement の形式とフィールドを指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ
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

    /// 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける。マイナーバージョン 23 から
    public var styleId: String? {
        get { readField("style_id") }
        set { writeField("style_id", newValue) }
    }
}

/// 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（マイナーバージョン 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印のフラグを必ず持つので、arrow を書かない引出線は書けない（マイナーバージョン 36 から。それまでは R2004 までの置き換えだけ）。マイナーバージョン 31 から
public struct LeaderGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかのフィールドは元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。マイナーバージョン 36 から
public struct CalloutGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 表（DXF・DWG の ACAD_TABLE、決定 0113）。挿入点・行の向き・行の高さ・列の幅と、行ごとのセルの並びを持つ。表のスタイル・セルの書式・型のある値・表示の部品（ブロック）は形の値に含めない。置き換え（DXF の ACAD_TABLE）では、行と列の数・セルの結合・文字でないセル（記号）を変えず、挿入点・行の向き・行の高さ・列の幅・文字と空のセルの内容と回転を書き直し、表示の部品を作り直す。R2004 以降の DXF で Standard スタイルを使って追加できる。R2004 DWG では ACAD_TABLE クラスと読み取れる Standard 表スタイルのある図面に、未結合・無回転の文字と空のセルの表を追加できる。既存の対応する DWG 表ではセル文字・行高さ・列幅・向きの変更と平行移動も扱う。マイナーバージョン 74 から
public struct TableGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["shape": .string("table")]
    }

    /// 形の種類
    public var shape: String? {
        get { readField("shape") }
        set { writeField("shape", newValue) }
    }

    /// 挿入点 [X, Y]
    public var position: [Double]? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 行の向き [X, Y]。0 でない
    public var direction: [Double]? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// 上の行からの行の高さ。どれも 0 より大きい。1 つ以上
    public var rowHeights: [Double]? {
        get { readField("row_heights") }
        set { writeField("row_heights", newValue) }
    }

    /// 左の列からの列の幅。どれも 0 より大きい。1 つ以上
    public var columnWidths: [Double]? {
        get { readField("column_widths") }
        set { writeField("column_widths", newValue) }
    }

    /// 上の行から、行ごとに左の列から並べたセル。数は行の数と列の数の積
    public var cells: [TableCell]? {
        get { readField("cells") }
        set { writeField("cells", newValue) }
    }
}

/// 表の 1 つのセル
public struct TableCell: Message {
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

    /// 中身の種類
    public var content: String? {
        get { readField("content") }
        set { writeField("content", newValue) }
    }

    /// 書式の符号を含む元の文字列。空でない。content が text のときだけ持つ
    public var text: String? {
        get { readField("text") }
        set { writeField("text", newValue) }
    }

    /// 記号のブロックの名前。content が symbol のときだけ持つ
    public var symbol: String? {
        get { readField("symbol") }
        set { writeField("symbol", newValue) }
    }

    /// 結合して広げる列と行の数 [列, 行]。結合しなければ [1, 1]（省けば [1, 1]）
    public var span: [Int64]? {
        get { readField("span") }
        set { writeField("span", newValue) }
    }

    /// ほかのセルに結合されて覆われるか。偽のときは省ける
    public var covered: Bool? {
        get { readField("covered") }
        set { writeField("covered", newValue) }
    }

    /// 文字か記号の回転。0 のときは省ける
    public var rotation: Double? {
        get { readField("rotation") }
        set { writeField("rotation", newValue) }
    }
}

/// 表のセルの中身の種類。empty は空、text は書式つきの文字、symbol は記号（ブロック）
///
/// 列挙の値。知らない値も受け取る
public enum TableCellContent {
    public static let empty = "empty"
    public static let text = "text"
    public static let symbol = "symbol"
}

/// 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義のマーカーの点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。マイナーバージョン 35 から
public struct PointGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// 既定義のマーカー。書かなければ文書の点の表し方に従う
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

/// 既定義のマーカーの表のキーと尺度。マーカーの形は、表示する側がキーから引く
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

    /// マーカーの表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点）
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

/// 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。マイナーバージョン 14 から
public struct SymbolGeometry: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。マイナーバージョン 16 から。置き換えで値を変えるのはマイナーバージョン 21 から
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

/// 要素の形の種類（決定 0022）。point（点、決定 0160）はマイナーバージョン 32 から。それまで点は symbol だった
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

/// 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。マイナーバージョン 7 から
public struct ApplyRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("apply")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// true なら、変えていない要素の ID がずれた。キーを持たない形式（JWW）か、ハンドルを持たない DXF（R12 など）で、前の図形か表のレコードを除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。マイナーバージョン 30 から
    public var idsShifted: Bool? {
        get { readField("ids_shifted") }
        set { writeField("ids_shifted", newValue) }
    }
}

/// 入力なしで空の文書を作り、開く。dxf は R2004 の DXF で、既定のレイヤとスタイル、空のモデル空間と Layout1 を持つ。ifc は IFC のひな形で、プロジェクト・長さとラジアンの単位・3 次元の Model の文脈と Body の下位の文脈を持つ（#395）。書いたファイルは blobs で返す。マイナーバージョン 83 から。ifc はマイナーバージョン 109 から
public struct CreateRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("create")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 作る形式の名前。dxf か ifc
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 作る文書の名前。既存の文書と重ねない
    public var to: String? {
        get { readField("to") }
        set { writeField("to", newValue) }
    }

    /// 実寸の空間の長さの単位。省略すると millimeter。unknown は単位なしで、ifc では書けない
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// IFC を書く設定。format が ifc のときに必須で、ほかの形式では書けない。マイナーバージョン 109 から
    public var ifc: IfcOutput? {
        get { readField("ifc") }
        set { writeField("ifc", newValue) }
    }
}

/// 作った空の文書。書いたファイルは blobs の file で返す
public struct CreateResult: Message {
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

    /// 書いた要素の数。空なので 0
    public var elements: Int64? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }
}

/// 開いた文書から別の形式のファイルを作り、新しい文書として開く。図面からは DXF（対応する R2004 DWG の弧長寸法は共有オブジェクト表から、それ以外は図面モデルから。決定 0139・0206・0281）または SIMA（.sim）、土木モデル（J-LandXML・IFC4.3・SIMA）からは SIMA（.sim）、地理の領域モデルの文書（CityGML・基盤地図情報・拡張 DM・GeoJSON・SIMA・GeoTIFF）からは GeoJSON または SIMA（.sim）を書く。建物モデルの文書（IFC・ST-Bridge）からは、ifc で指定したバージョンの新しい IFC を書く（#395）。書けないデータは種類ごとの件数を診断で返す。書いたファイルは blobs で返す。マイナーバージョン 41 から。geojson はマイナーバージョン 71 から。sima はマイナーバージョン 72 から。ifc はマイナーバージョン 109 から
public struct ConvertRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("convert")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 書く形式の名前（capabilities の formats の id）。dxf・sxf（P21）・jww・geojson・sima・ifc のいずれか。sxf・jww はマイナーバージョン 85 から
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 作る文書の名前
    public var to: String? {
        get { readField("to") }
        set { writeField("to", newValue) }
    }

    /// IFC を書く設定。format が ifc のときに必須で、ほかの形式では書けない。マイナーバージョン 109 から
    public var ifc: IfcOutput? {
        get { readField("ifc") }
        set { writeField("ifc", newValue) }
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

/// 開いた文書を変換したときの忠実度と欠落を、convert と同じ書き手で見積もる。文書もファイルの blob も登録しない。図面から DXF・SXF（P21）・JWW、建物モデルの文書から IFC への変換に対応する。マイナーバージョン 66 から（SXF・JWW は 85 から、IFC は 109 から）
public struct ConvertEstimateRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("convert_estimate")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 書く元の文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 書く形式の名前。dxf・sxf・jww・ifc
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 予定する文書の名前。convert と同じ条件で検査するが、名前を確保しない
    public var to: String? {
        get { readField("to") }
        set { writeField("to", newValue) }
    }

    /// IFC を書く設定。format が ifc のときに必須で、ほかの形式では書けない。マイナーバージョン 109 から
    public var ifc: IfcOutput? {
        get { readField("ifc") }
        set { writeField("ifc", newValue) }
    }

    /// 書き手と読み取り段階のそれぞれで返す明細の最大件数。省略時は 100。総数と全件集計はこの上限によらない
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// 新しい IFC を書く設定（#395）。コアは時計を読まないので、時刻は呼び出し元が渡す
public struct IfcOutput: Message {
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

    /// 書くバージョン
    public var version: String? {
        get { readField("version") }
        set { writeField("version", newValue) }
    }

    /// ヘッダー（FILE_DESCRIPTION と FILE_NAME）の値。書かなければ空の値で書く
    public var header: IfcHeader? {
        get { readField("header") }
        set { writeField("header", newValue) }
    }

    /// 所有履歴（IfcOwnerHistory）の値。ifc2x3 では必須。ifc4 からは書いたときだけ書く
    public var owner: IfcOwner? {
        get { readField("owner") }
        set { writeField("owner", newValue) }
    }

    /// GlobalId を導く名前空間。書かなければ、convert は元の IfcProject の GlobalId（なければ元の文書のハッシュ）、create は作る文書の名前を使う。元の文書のハッシュでない名前空間なら、元を一部直して書き直しても、直していない要素の GlobalId は変わらない
    public var namespace: String? {
        get { readField("namespace") }
        set { writeField("namespace", newValue) }
    }
}

/// IFC のバージョン。ifc2x3 は IFC2X3 TC1、ifc4 は IFC4 ADD2 TC1、ifc4x3 は IFC4X3 ADD2
///
/// 列挙の値。知らない値も受け取る
public enum IfcVersion {
    public static let ifc2x3 = "ifc2x3"
    public static let ifc4 = "ifc4"
    public static let ifc4x3 = "ifc4x3"
}

/// IFC のヘッダーの値。書かない値は空で書く
public struct IfcHeader: Message {
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

    /// FILE_DESCRIPTION に ViewDefinition [<値>] として書く MVD の名前。書かなければ書かない
    public var viewDefinition: String? {
        get { readField("view_definition") }
        set { writeField("view_definition", newValue) }
    }

    /// FILE_NAME の name
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// FILE_NAME の time_stamp。呼び出し元が書いた時刻
    public var timeStamp: String? {
        get { readField("time_stamp") }
        set { writeField("time_stamp", newValue) }
    }

    /// FILE_NAME の author
    public var author: [String]? {
        get { readField("author") }
        set { writeField("author", newValue) }
    }

    /// FILE_NAME の organization
    public var organization: [String]? {
        get { readField("organization") }
        set { writeField("organization", newValue) }
    }

    /// FILE_NAME の preprocessor_version
    public var preprocessorVersion: String? {
        get { readField("preprocessor_version") }
        set { writeField("preprocessor_version", newValue) }
    }

    /// FILE_NAME の originating_system
    public var originatingSystem: String? {
        get { readField("originating_system") }
        set { writeField("originating_system", newValue) }
    }

    /// FILE_NAME の authorization
    public var authorization: String? {
        get { readField("authorization") }
        set { writeField("authorization", newValue) }
    }
}

/// 所有履歴の値
public struct IfcOwner: Message {
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

    /// 所有する人の IfcPerson の FamilyName
    public var person: String? {
        get { readField("person") }
        set { writeField("person", newValue) }
    }

    /// 所有する組織の IfcOrganization の Name
    public var organization: String? {
        get { readField("organization") }
        set { writeField("organization", newValue) }
    }

    /// 作ったソフトの開発元の IfcOrganization の Name
    public var developer: String? {
        get { readField("developer") }
        set { writeField("developer", newValue) }
    }

    /// 作ったソフトの名前（IfcApplication の ApplicationFullName）
    public var application: String? {
        get { readField("application") }
        set { writeField("application", newValue) }
    }

    /// 作ったソフトのバージョン（IfcApplication の Version）
    public var version: String? {
        get { readField("version") }
        set { writeField("version", newValue) }
    }

    /// 作ったソフトの識別子（IfcApplication の ApplicationIdentifier）
    public var identifier: String? {
        get { readField("identifier") }
        set { writeField("identifier", newValue) }
    }

    /// 作った日時。1970-01-01T00:00:00Z からの秒（IfcTimeStamp）
    public var created: Int64? {
        get { readField("created") }
        set { writeField("created", newValue) }
    }
}

/// 予定する変換の結果。元の文書・ID を保ち、ファイルの blob を返さない
public struct ConvertEstimateResult: Message {
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

    /// 予定する出力と忠実度の全件集計・上限付き明細
    public var estimate: ConversionEstimate? {
        get { readField("estimate") }
        set { writeField("estimate", newValue) }
    }
}

/// 実変換と同じ処理で準備した出力。doc と hash は未登録の予定値。元の入力・設定・予算が同じなら convert の値と一致する
public struct ConversionEstimate: Message {
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

    /// 予定する文書の名前
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 予定する形式の名前
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 予定する出力バイト列のハッシュ。未登録なので apply の前提条件には使えない
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 予定する出力の要素数。記号から離す属性の文字を含む
    public var elements: Int64? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }

    /// 処理対象の出現数。同じ出所を持つ別の入力は別々に数え、診断の数では増やさない
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 種類と忠実度ごとの全件集計
    public var counts: [ConversionCount]? {
        get { readField("counts") }
        set { writeField("counts", newValue) }
    }

    /// 処理順の明細。limit 件まで返す
    public var details: [ConversionDetail]? {
        get { readField("details") }
        set { writeField("details", newValue) }
    }

    /// 明細の上限で返さなかった対象がある
    public var truncated: Bool? {
        get { readField("truncated") }
        set { writeField("truncated", newValue) }
    }

    /// 入力の表の読み取りと lift 中の、集約前の診断発生記録。書き手の項目の集計には含めない。マイナーバージョン 67 から
    public var inputDiagnostics: ConversionInputDiagnostics? {
        get { readField("input_diagnostics") }
        set { writeField("input_diagnostics", newValue) }
    }
}

/// 読み取り段階の診断発生数。失った要素数ではなく、同じ要素の複数の診断もそれぞれ数える。忠実度を持たない診断も含む。open 時や書き手の診断は含まない
public struct ConversionInputDiagnostics: Message {
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

    /// 診断の発生件数の合計。max_places と limit によらない
    public var total: Int64? {
        get { readField("total") }
        set { writeField("total", newValue) }
    }

    /// 診断の code とカタログの group_by の値による全件集計。場所は持たず、値は最も早い入力位置の報告から取る
    public var counts: [Diagnostic]? {
        get { readField("counts") }
        set { writeField("counts", newValue) }
    }

    /// 集約前の報告を入力位置順に limit 件まで返す。同じ位置は報告順。診断の場所は max_places で制限しない
    public var details: [ConversionInputDiagnostic]? {
        get { readField("details") }
        set { writeField("details", newValue) }
    }

    /// 明細の上限で返さなかった報告がある
    public var truncated: Bool? {
        get { readField("truncated") }
        set { writeField("truncated", newValue) }
    }
}

/// 集約前の診断と、その場所と同じ順序の出所。種類や失った値は診断の code と params からたどる
public struct ConversionInputDiagnostic: Message {
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

    /// 集約前の診断。元の値と場所を保つ
    public var diagnostic: Diagnostic? {
        get { readField("diagnostic") }
        set { writeField("diagnostic", newValue) }
    }

    /// diagnostic.at と同じ順序のレコード・バイト範囲・ハンドル・インスタンス番号。ID は diagnostic.at にある
    public var sources: [Source]? {
        get { readField("sources") }
        set { writeField("sources", newValue) }
    }
}

/// 処理対象の種類と忠実度ごとの集計
public struct ConversionCount: Message {
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

    /// 処理対象の種類のキー
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// その対象を移した結果の忠実度
    public var fidelity: String? {
        get { readField("fidelity") }
        set { writeField("fidelity", newValue) }
    }

    /// 入力の出現数。1 つの入力が複数の出力になっても 1 件
    public var count: Int64? {
        get { readField("count") }
        set { writeField("count", newValue) }
    }

    /// この対象から書いた図形の数。表や参照だけを移した場合は 0
    public var outputElements: Int64? {
        get { readField("output_elements") }
        set { writeField("output_elements", newValue) }
    }
}

/// 処理対象の出現 1 つ。複数の欠落の診断が出ても 1 件として数える
public struct ConversionDetail: Message {
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

    /// 処理対象の種類のキー
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// その対象を移した結果の忠実度
    public var fidelity: String? {
        get { readField("fidelity") }
        set { writeField("fidelity", newValue) }
    }

    /// 値や参照を変えた理由のキー。text_height_meaning は文字高の意味、relocated_shape は形の値、style_default はスタイルの既定値、detached_attribute は属性の文字への分離、baked_view はビューの焼き込み、reference_dropped は参照の欠落
    public var reasons: [String]? {
        get { readField("reasons") }
        set { writeField("reasons", newValue) }
    }

    /// 元の要素の ID。持つ対象だけで返す
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 元の対象のレコード・バイト範囲・キー
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }

    /// この対象から書いた図形の数。表や参照だけを移した場合は 0
    public var outputElements: Int64? {
        get { readField("output_elements") }
        set { writeField("output_elements", newValue) }
    }
}

/// 忠実度。`native` は元の値・参照を保持、`equivalent` は別の表現で同じ意味、`approximated` は近似、`drawn` は見た目だけ、`dropped` は欠落
///
/// 列挙の値。知らない値も受け取る
public enum ConversionFidelity {
    public static let native = "native"
    public static let equivalent = "equivalent"
    public static let approximated = "approximated"
    public static let drawn = "drawn"
    public static let dropped = "dropped"
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

    /// kind が restyle の形なら、そのビュー。違えば nil
    public var asRestyleEdit: RestyleEdit? {
        taggedView("kind", "restyle")
    }

    /// kind が transform の形なら、そのビュー。違えば nil
    public var asTransformEdit: TransformEdit? {
        taggedView("kind", "transform")
    }

    /// kind が copy の形なら、そのビュー。違えば nil
    public var asCopyEdit: CopyEdit? {
        taggedView("kind", "copy")
    }

    /// kind が remove の形なら、そのビュー。違えば nil
    public var asRemoveEdit: RemoveEdit? {
        taggedView("kind", "remove")
    }

    /// kind が replace の形なら、そのビュー。違えば nil
    public var asReplaceEdit: ReplaceEdit? {
        taggedView("kind", "replace")
    }

    /// kind が add の形なら、そのビュー。違えば nil
    public var asAddEdit: AddEdit? {
        taggedView("kind", "add")
    }

    /// kind が layer の形なら、そのビュー。違えば nil
    public var asLayerEdit: LayerEdit? {
        taggedView("kind", "layer")
    }

    /// kind が add_layer の形なら、そのビュー。違えば nil
    public var asAddLayerEdit: AddLayerEdit? {
        taggedView("kind", "add_layer")
    }

    /// kind が add_symbol の形なら、そのビュー。違えば nil
    public var asAddSymbolEdit: AddSymbolEdit? {
        taggedView("kind", "add_symbol")
    }

    /// kind が remove_layer の形なら、そのビュー。違えば nil
    public var asRemoveLayerEdit: RemoveLayerEdit? {
        taggedView("kind", "remove_layer")
    }

    /// kind が text_style の形なら、そのビュー。違えば nil
    public var asTextStyleEdit: TextStyleEdit? {
        taggedView("kind", "text_style")
    }

    /// kind が add_text_style の形なら、そのビュー。違えば nil
    public var asAddTextStyleEdit: AddTextStyleEdit? {
        taggedView("kind", "add_text_style")
    }

    /// kind が remove_text_style の形なら、そのビュー。違えば nil
    public var asRemoveTextStyleEdit: RemoveTextStyleEdit? {
        taggedView("kind", "remove_text_style")
    }

    /// kind が line_type の形なら、そのビュー。違えば nil
    public var asLineTypeEdit: LineTypeEdit? {
        taggedView("kind", "line_type")
    }

    /// kind が add_line_type の形なら、そのビュー。違えば nil
    public var asAddLineTypeEdit: AddLineTypeEdit? {
        taggedView("kind", "add_line_type")
    }

    /// kind が remove_line_type の形なら、そのビュー。違えば nil
    public var asRemoveLineTypeEdit: RemoveLineTypeEdit? {
        taggedView("kind", "remove_line_type")
    }

    /// kind が dimension_style の形なら、そのビュー。違えば nil
    public var asDimensionStyleEdit: DimensionStyleEdit? {
        taggedView("kind", "dimension_style")
    }

    /// kind が add_dimension_style の形なら、そのビュー。違えば nil
    public var asAddDimensionStyleEdit: AddDimensionStyleEdit? {
        taggedView("kind", "add_dimension_style")
    }

    /// kind が remove_dimension_style の形なら、そのビュー。違えば nil
    public var asRemoveDimensionStyleEdit: RemoveDimensionStyleEdit? {
        taggedView("kind", "remove_dimension_style")
    }

    /// kind が point_style の形なら、そのビュー。違えば nil
    public var asPointStyleEdit: PointStyleEdit? {
        taggedView("kind", "point_style")
    }
}

/// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない。RGB の色は、DXF と DWG の R2004 から、線幅は R2000 から書ける（DWG の今ある図形のレイヤ・線種・線幅・RGB の色は、決定 0293 から変えられる）
public struct RestyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

    /// レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。マイナーバージョン 8 から
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

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 要素を複写し、写しをアフィン変換で動かす。元の要素は変えない。写しは、元のレイヤ・色・線種・線種の倍率・線幅・透過性と形式固有の値（AutoCAD の系統の XDATA、JWW のフラグ・文字種・字間、SXF の文字幅・字間など）を保ち、図形とともに動く XDATA と記号の属性も同じ変換で直す。DXF の寸法・表は新しい匿名ブロックを作る。place で配置先の空間・用紙を、from_doc で複写元の文書を指定でき（マイナーバージョン 110 から）、単位・画層・線種・文字スタイル・寸法スタイル・記号定義・APPID の対応と名前の衝突を確かめる。保てない値・参照・対応を持つ要素は change.copy.unsupported で全体を拒む。applied は写しの ID とハッシュを返す。マイナーバージョン 100 から
public struct CopyEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("copy")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 複写する要素の ID。entities が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。entities が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// [a, b, c, d, e, f]。写しの点 (x, y) を (a x + b y + c, d x + e y + f) に写す。元の位置に写すなら [1, 0, 0, 0, 1, 0]
    public var matrix: [Double]? {
        get { readField("matrix") }
        set { writeField("matrix", newValue) }
    }

    /// 配置先の空間か用紙の ID。省けば元の要素と同じ場所 ID。マイナーバージョン 110 から
    public var place: String? {
        get { readField("place") }
        set { writeField("place", newValue) }
    }

    /// 複写元の文書の名前。open で作ったもの。省けば doc と同じ文書。マイナーバージョン 110 から
    public var fromDoc: String? {
        get { readField("from_doc") }
        set { writeField("from_doc", newValue) }
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

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤと線のスタイルは変えない。DXF・DWG の文字と書式つきの文字は、定義のある文字スタイルへ指し替えられる。マイナーバージョン 10 から
public struct ReplaceEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。JWW と SXF の図形は色・線種・線幅を自分で持ち、レイヤかブロックに従う値を持たないので、JWW は色と線種を書き（点は色だけ）、SXF は色・線種・線幅を書く（点マーカーは色だけ）。書かないか、レイヤかブロックに従う値なら change.style.unsupported で断る。JWW の書かないかレイヤに従う線幅は、ペンの印刷の線幅に従う。足せる形は、形の値のどの種類も。マイナーバージョン 10 から
public struct AddEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。マイナーバージョン 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177）
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

    /// 色。SXF と JWW はレイヤに従う色を書けないので、書く。SXF は、sxf の番号か、利用者定義色と同じ RGB を受ける。JWW は、jww の番号（0〜9 と、バージョン 420 からの拡張色 100〜356）か、ペン 1〜9 と、バージョン 420 からの 100〜356 のどれかと同じ RGB を受ける。書かないか、受けられない色なら断る。マイナーバージョン 29 から
    public var color: ColorChange? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// 線種。マイナーバージョン 29 から
    public var lineType: LineTypeChange? {
        get { readField("line_type") }
        set { writeField("line_type", newValue) }
    }

    /// 線幅。マイナーバージョン 29 から
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

/// レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG の真の色は R2004 から書ける（今あるレイヤの線種と真の色は、決定 0293 から変えられる）。マイナーバージョン 22 から
public struct LayerEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。マイナーバージョン 24 から
public struct RemoveLayerEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。マイナーバージョン 25 から
public struct TextStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 文字スタイルを足す（決定 0163）。書かないフィールドは Standard と同じ。同じ apply の後ろの文字の追加・置き換えから、足した文字スタイルを名前で指せる。マイナーバージョン 25 から
public struct AddTextStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 25 から
public struct RemoveTextStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。マルチラインスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。マイナーバージョン 26 から
public struct LineTypeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 線種を足す（決定 0164）。書かないフィールドは Continuous と同じ。同じ apply の後ろの要素とレイヤの変更から、足した線種を名前で指せる。マイナーバージョン 26 から
public struct AddLineTypeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 線種を除く（決定 0164）。要素・レイヤ・マルチラインスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。マイナーバージョン 26 から
public struct RemoveLineTypeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。マイナーバージョン 27 から
public struct DimensionStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 寸法スタイルを足す（決定 0165）。書かないフィールドは Standard と同じ。同じ apply の後ろの要素の追加から、足した寸法スタイルを名前で指せる。マイナーバージョン 27 から
public struct AddDimensionStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 27 から
public struct RemoveDimensionStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG はヘッダーの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、マイナーバージョン 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。マイナーバージョン 38 から
public struct PointStyleEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// レイヤを足す（決定 0162）。書かないフィールドは、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の後ろの要素の変更から、足したレイヤを名前で指せる。マイナーバージョン 22 から
public struct AddLayerEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
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

/// 記号の定義（DXF・DWG のブロック）を足す（#684）。中の要素は、定義の空間に足す。足した定義は、同じ apply の後ろの add の記号から名前で指せる。属性の定義・名前のない定義・外部参照は足せない。結果は id と hash を持たない。足した定義は tables の symbols から名前で引く。マイナーバージョン 99 から
public struct AddSymbolEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_symbol")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 定義の名前。図面にない名前で、layer の name と同じ決まり（DXF・DWG のブロックの名前で書けない文字を含まない）。大文字と小文字は区別しない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 定義の基点 [X, Y]。記号を置く点に重ねる。書かなければ [0, 0]
    public var base: [Double]? {
        get { readField("base") }
        set { writeField("base", newValue) }
    }

    /// 中の要素。点は定義の空間の座標で書く。並べた順に足す
    public var elements: [SymbolElement]? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }
}

/// 足す記号の定義の中の要素。フィールドは add と同じ意味で、place と kind を持たない。引出線の注記は指せない。マイナーバージョン 99 から
public struct SymbolElement: Message {
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

    /// レイヤの元の名前。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
    public var layer: String? {
        get { readField("layer") }
        set { writeField("layer", newValue) }
    }

    /// レイヤの ID。tables が返したもの
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

    /// 線幅
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

    /// SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。マイナーバージョン 29 から
    public var sxf: Int64? {
        get { readField("sxf") }
        set { writeField("sxf", newValue) }
    }

    /// Jw_cad のペンの番号。0〜9 と、バージョン 420 からの SXF の拡張色 100〜356。マイナーバージョン 30 から
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

    /// 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。マイナーバージョン 8 から
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

/// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。マイナーバージョン 8 から
public struct TablesRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("tables")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 線種。マイナーバージョン 26 から、定義を持つ
    public var lineTypes: [LineTypeItem]? {
        get { readField("line_types") }
        set { writeField("line_types", newValue) }
    }

    /// 寸法スタイル。マイナーバージョン 27 から、端部記号と大きさを持つ
    public var dimensionStyles: [DimensionStyleItem]? {
        get { readField("dimension_styles") }
        set { writeField("dimension_styles", newValue) }
    }

    /// 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義のフィールドを持たない項目として後ろに並ぶ。マイナーバージョン 20 から
    public var textStyles: [TextStyleItem]? {
        get { readField("text_styles") }
        set { writeField("text_styles", newValue) }
    }

    /// 記号の定義。配置されていないものも含む。マイナーバージョン 81 から
    public var symbols: [SymbolDefinitionItem]? {
        get { readField("symbols") }
        set { writeField("symbols", newValue) }
    }
}

/// 記号の定義。要素と属性の配置や書式は元の定義に保ち、名前と属性の既定値を一覧にする
public struct SymbolDefinitionItem: Message {
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

    /// 定義の空間の公開 ID。entities.place と同じ
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 定義を識別する名前空間
    public var namespace: String? {
        get { readField("namespace") }
        set { writeField("namespace", newValue) }
    }

    /// 元の名前。add の記号の name に使う
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 定義の基点。定義の空間の座標と単位。不明なら持たない
    public var base: Point2? {
        get { readField("base") }
        set { writeField("base", newValue) }
    }

    /// 属性のタグ・既定値・入力の指定。元の順を保つ
    public var attributes: [SymbolAttributeDefinition]? {
        get { readField("attributes") }
        set { writeField("attributes", newValue) }
    }

    /// 寸法などが作った無名の定義である
    public var anonymous: Bool? {
        get { readField("anonymous") }
        set { writeField("anonymous", newValue) }
    }

    /// 外部参照の元のパス。参照先を開かない。外部参照でなければ持たない
    public var external: String? {
        get { readField("external") }
        set { writeField("external", newValue) }
    }

    /// 定義の出所
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 記号の属性の定義。タグと値は翻訳しない。配置・書式・スタイルは一覧には含めない
public struct SymbolAttributeDefinition: Message {
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

    /// 既定の 1 行の値
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }

    /// 複数行の値があるとき、その元の書式つきの文字列。読むための値
    public var formattedValue: String? {
        get { readField("formatted_value") }
        set { writeField("formatted_value", newValue) }
    }

    /// 値を尋ねる文
    public var prompt: String? {
        get { readField("prompt") }
        set { writeField("prompt", newValue) }
    }

    /// 値が決まっていて、配置ごとに変えない
    public var constant: Bool? {
        get { readField("constant") }
        set { writeField("constant", newValue) }
    }

    /// 入力した値を確かめる指定
    public var verify: Bool? {
        get { readField("verify") }
        set { writeField("verify", newValue) }
    }

    /// 尋ねずに既定の値を使う指定
    public var preset: Bool? {
        get { readField("preset") }
        set { writeField("preset", newValue) }
    }

    /// 属性を表示しない
    public var hidden: Bool? {
        get { readField("hidden") }
        set { writeField("hidden", newValue) }
    }

    /// 属性の出所
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。マイナーバージョン 33 から
public struct SceneRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("scene")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決のフラグを立てる
    public var palette: String? {
        get { readField("palette") }
        set { writeField("palette", newValue) }
    }

    /// 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形のセクションに形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。マイナーバージョン 37 から
    public var fonts: [SceneFont]? {
        get { readField("fonts") }
        set { writeField("fonts", newValue) }
    }

    /// 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは、表示と比べて決めた意味（dxf.TEXT.40 は capital_a。マイナーバージョン 59 から）にし、それもなければ大文字の高さとみなす。fonts を書いたときだけ使う。マイナーバージョン 37 から
    public var heights: [String: String]? {
        get { readField("heights") }
        set { writeField("heights", newValue) }
    }
}

/// 建物モデルを開口付きの 3D シーンにし、ZMS3 の blob で返す。86 から地物の面と高さのある線・面も扱う。階・クラス・ID の各条件の中は和、条件どうしは積。箱は要素の重なりで選び、メッシュを切らない。88 から点群を元の八分木の節ごとの点の区画（ZMS3 1.1）で返す。点群の点は pointcloud と同じ節の順で選び、座標系と高さの基準を変換しない。94 から平面直角座標系の GeoTIFF の DEM も、外枠の地物でなく標高格子を点群と同じ点の区画で返す。105 から surface で、DEM を地形の面（セル代表点を頂点にした三角形のメッシュ）で返す。マイナーバージョン 82 から
public struct Scene3dRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("scene3d")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// open で作った建物・地物・点群の文書名。点群は 88 から
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 所属する階の要素の ID。省略は全階、空の配列は何も選ばない。地物と点群には指定できない
    public var storeys: [String]? {
        get { readField("storeys") }
        set { writeField("storeys", newValue) }
    }

    /// 建物のクラス名か地物の型。大文字・小文字を区別する。省略は全種類、空の配列は何も選ばない。点群には指定できない
    public var classes: [String]? {
        get { readField("classes") }
        set { writeField("classes", newValue) }
    }

    /// 建物の要素か元の地物の ID。省略は全要素、空の配列は何も選ばない。建物では参照する階の行も返す。点群には指定できない
    public var ids: [String]? {
        get { readField("ids") }
        set { writeField("ids", newValue) }
    }

    /// 地物だけに指定する投影先の平面直角座標系の系（1〜19）。省略時は元の系、経緯度では範囲の中心から計算用の系を仮定して診断する。86 から
    public var zone: Int64? {
        get { readField("zone") }
        set { writeField("zone", newValue) }
    }

    /// 地物だけに指定する LOD（0〜4）。省略時は選択した地物にある最大の LOD を 1 段選ぶ。LOD のない形は含める。86 から
    public var lod: Int64? {
        get { readField("lod") }
        set { writeField("lod", newValue) }
    }

    /// プロジェクト空間の文書単位の箱。重なる要素を選び、形のない要素も残す。切り抜きは読み手が行う
    public var box: Scene3dBox? {
        get { readField("box") }
        set { writeField("box", newValue) }
    }

    /// 点群だけに指定する返す点の数の上限。省略時は選んだすべての点。88 から。DEM を surface で返すときは、頂点にする格子点の数の上限。超えれば間引く間隔を 2 倍ずつ広げ、それでも超えれば何も返さない
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }

    /// 平面直角座標系の GeoTIFF の DEM だけに指定する。真なら標高格子を点でなく地形の面で返す。セル代表点を頂点にし、隣り合う 4 つの格子点が点にできれば 2 個、3 つなら 1 個の三角形を作る。nodata などのセルは面に含めない。面は 128 間隔四方の区画ごとの要素（ID は区画の初めの行と列の r<行>c<列>、クラスは dem）に分け、隣の区画と縁の頂点を共有する。座標系と高さの基準は変換しない。偽か省略なら点で返す。105 から
    public var surface: Bool? {
        get { readField("surface") }
        set { writeField("surface", newValue) }
    }

    /// 文書単位の正の有限値。点群では点の間隔（メートル）で、これより細かい段の節を選ばない。DEM の面では頂点の間隔（メートル）で、セル代表点の間隔の整数倍のうち、これ以上でもっとも細かい間隔に間引く。建物では曲線を分ける弦の誤差。地物では f32 の座標の丸めをこの値の 1/4 までに制限し、投影した辺全体の誤差は含めない。省略時は対象範囲の対角線の 1/50000、空か点だけなら 1 mm 相当。建物の範囲は 1 mm の予備評価で求める
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }
}

/// 建物ではプロジェクト空間の文書単位、地物では投影先の、点群では元の座標系の東・北・高さのメートルの箱。Z が上
public struct Scene3dBox: Message {
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

    /// 最小の X・Y・Z の 3 値
    public var min: [Double]? {
        get { readField("min") }
        set { writeField("min", newValue) }
    }

    /// 最大の X・Y・Z の 3 値
    public var max: [Double]? {
        get { readField("max") }
        set { writeField("max", newValue) }
    }
}

/// 建物では文書が持つ地図への変換の元値で、シーンの座標へ適用していない。地物では投影先の EPSG 名と恒等変換を返し、シーンの X・Y は既に東・北のメートル
public struct Scene3dMap: Message {
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

    /// 地図の座標系の名前。未指定なら持たない
    public var crsName: String? {
        get { readField("crs_name") }
        set { writeField("crs_name", newValue) }
    }

    /// 東への移動
    public var eastings: Double? {
        get { readField("eastings") }
        set { writeField("eastings", newValue) }
    }

    /// 北への移動
    public var northings: Double? {
        get { readField("northings") }
        set { writeField("northings", newValue) }
    }

    /// 高さの移動
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 地図の X 軸の横の成分の元値
    public var xAxisAbscissa: Double? {
        get { readField("x_axis_abscissa") }
        set { writeField("x_axis_abscissa", newValue) }
    }

    /// 地図の X 軸の縦の成分の元値
    public var xAxisOrdinate: Double? {
        get { readField("x_axis_ordinate") }
        set { writeField("x_axis_ordinate", newValue) }
    }

    /// 地図への尺度の元値
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }
}

/// 返した 3D シーンの列の行数。共有するメッシュの点と面は 1 回だけ数える
public struct Scene3dCounts: Message {
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

    /// 共有するメッシュ
    public var meshes: Int64? {
        get { readField("meshes") }
        set { writeField("meshes", newValue) }
    }

    /// メッシュの配置
    public var instances: Int64? {
        get { readField("instances") }
        set { writeField("instances", newValue) }
    }

    /// 要素と参照先の階。形のない行も含む
    public var elements: Int64? {
        get { readField("elements") }
        set { writeField("elements", newValue) }
    }

    /// 色
    public var colors: Int64? {
        get { readField("colors") }
        set { writeField("colors", newValue) }
    }

    /// 局所の頂点
    public var vertices: Int64? {
        get { readField("vertices") }
        set { writeField("vertices", newValue) }
    }

    /// 三角形
    public var triangles: Int64? {
        get { readField("triangles") }
        set { writeField("triangles", newValue) }
    }

    /// 形の稜線
    public var edges: Int64? {
        get { readField("edges") }
        set { writeField("edges", newValue) }
    }

    /// 点群の点。点群のシーンだけが持つ。88 から
    public var points: Int64? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }
}

/// 3D シーンの座標・許容差・行数と blob の番号
public struct Scene3dResult: Message {
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

    /// 元の建物か地物の文書名
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// シーンの座標と許容差の長さの単位
    public var unit: BuildingUnit? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// 文書の地図への変換。なければ持たない
    public var map: Scene3dMap? {
        get { readField("map") }
        set { writeField("map", newValue) }
    }

    /// 地物の高さの基準と成果を表す EPSG。元が不明なら持たない。86 から
    public var heightCrs: Int64? {
        get { readField("height_crs") }
        set { writeField("height_crs", newValue) }
    }

    /// 地物で選択した LOD。LOD のない形だけなら持たない。86 から
    public var lod: Int64? {
        get { readField("lod") }
        set { writeField("lod", newValue) }
    }

    /// 使った文書単位の許容差。建物では曲線の弦の誤差、地物では f32 の丸めの上限の 4 倍。点群では要求の点の間隔、省略時は返したもっとも深い段の点の間隔。DEM の面では使った頂点の間隔。丸めのずれは別の診断で返す
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 点群で上限のために返さなかった点がある。DEM の面では、上限のために面を返さなかった。点群と DEM のシーンだけが持つ。88 から
    public var truncated: Bool? {
        get { readField("truncated") }
        set { writeField("truncated", newValue) }
    }

    /// 要求の断面の箱。なければ持たない
    public var box: Scene3dBox? {
        get { readField("box") }
        set { writeField("box", newValue) }
    }

    /// 返した塊の文書単位の範囲。形がなければ持たない
    public var extent: Scene3dBox? {
        get { readField("extent") }
        set { writeField("extent", newValue) }
    }

    /// シーンの行数
    public var counts: Scene3dCounts? {
        get { readField("counts") }
        set { writeField("counts", newValue) }
    }

    /// 種類が scene3d の blob の番号。zm_take で取り出す
    public var blob: Int64? {
        get { readField("blob") }
        set { writeField("blob", newValue) }
    }
}

/// 開いた文書を scene と同じ起点・配置・フォントで 2D のシーンにし、画像を blob で返す（決定 0272・0275・0277）。字形の輪郭だけを描き、字形のない空でない文字・外部画像・名前付きの点記号は元の ID と unit を診断して描かない。シーン生成と描画で同じ予算と中断の合図を使い、失敗では画像を返さない。PNG は最大 16777216 画素で、変換後の座標は絶対値 2^31 ピクセル以内。SVG はマイナーバージョン 64、PNG は 65 から
public struct RenderRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("render")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// scene と同じ起点。実寸の空間・記号の定義の空間・用紙の ID
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 起点の用紙のビューの番号。0 から数える。from が用紙のときだけ書ける。scene と同じ意味
    public var view: Int64? {
        get { readField("view") }
        set { writeField("view", newValue) }
    }

    /// 画像の出力形式
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 出力の幅（ピクセル）
    public var width: Int64? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 出力の高さ（ピクセル）
    public var height: Int64? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// [x の最小, y の最小, x の最大, y の最大]。起点の座標で、幅と高さが正の有限値になる範囲を明示する。scene の範囲の絞り込みにも使い、縦横同率で中央に収めて範囲外を切る。縦横比の違いによる余白は背景色にする
    public var region: [Double]? {
        get { readField("region") }
        set { writeField("region", newValue) }
    }

    /// 用紙 mm の線幅をピクセルへ直す DPI。正の有限値。座標の単位を mm と推測しない。線幅 0 は 1 ピクセル
    public var dpi: Double? {
        get { readField("dpi") }
        set { writeField("dpi", newValue) }
    }

    /// 背景の sRGB と alpha。[r, g, b, a] の 4 値を 0〜255 で明示する
    public var background: [Int64]? {
        get { readField("background") }
        set { writeField("background", newValue) }
    }

    /// ACI 7 と未解決の色に使う sRGB と alpha。[r, g, b, a] の 4 値を 0〜255 で明示する。未解決の色は診断する
    public var foreground: [Int64]? {
        get { readField("foreground") }
        set { writeField("foreground", newValue) }
    }

    /// scene と同じ弦の誤差（起点の長さ）。0 より大きい。書かなければ用紙は 0.02 mm、空間は要素の範囲の対角線の 1/50000
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// scene と同じ指定。true は印刷する要素、書かないか false は表示する要素を描く
    public var print: Bool? {
        get { readField("print") }
        set { writeField("print", newValue) }
    }

    /// 隠すレイヤの公開 ID の一覧。scene の layer_ids と同じ値で描画単位を除く。未知の ID は失敗、省略または空なら従来どおり描く
    public var hiddenLayers: [String]? {
        get { readField("hidden_layers") }
        set { writeField("hidden_layers", newValue) }
    }

    /// scene と同じ色の表の入力の名前、または組み込みの表 builtin:cad-v1。zm_push で渡した JSON の values に rgb と aci・sxf・jww の符号を並べる
    public var palette: String? {
        get { readField("palette") }
        set { writeField("palette", newValue) }
    }

    /// scene と同じフォントの指定。名前を合わせ、なければ名前のないフォントを使う。字形の輪郭を作るため、zm_push でフォントのバイト列を渡す。OS のフォントは探索しない
    public var fonts: [SceneFont]? {
        get { readField("fonts") }
        set { writeField("fonts", newValue) }
    }

    /// scene と同じ宣言値の高さの意味。fonts を書いたときだけ使う。書かないキーは既定の意味に従う（決定 0176・0188）
    public var heights: [String: String]? {
        get { readField("heights") }
        set { writeField("heights", newValue) }
    }
}

/// 画像の出力形式。svg は SVG 1.1（image/svg+xml）、png は RGBA8・非インターレースの PNG（image/png、マイナーバージョン 65 から）
///
/// 列挙の値。知らない値も受け取る
public enum RenderFormat {
    public static let svg = "svg"
    public static let png = "png"
}

/// 画像の形式・サイズと、シーンの起点・単位・許容差、画像の blob の番号
public struct RenderResult: Message {
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

    /// 画像の出力形式
    public var format: String? {
        get { readField("format") }
        set { writeField("format", newValue) }
    }

    /// 出力の幅（ピクセル）
    public var width: Int64? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 出力の高さ（ピクセル）
    public var height: Int64? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// シーンの起点の ID
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

    /// シーンで使った弦の誤差（起点の長さ）
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 画像の blob の番号（種類は image）。zm_take で取り出す
    public var blob: Int64? {
        get { readField("blob") }
        set { writeField("blob", newValue) }
    }
}

/// 開いた TIFF・GeoTIFF の指定ページの画素を RGBA8・非インターレースの PNG に復号し、画像の blob で返す（#877）。複数ページの TIFF は page で 1 から数えるページ番号を選ぶ（書かなければ 1）。max_side を指定すると、長辺が max_side 以下になるよう縦横比を保って縮小して返す（最大 16777216 画素）。マイナーバージョン 73 から
public struct RasterRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("raster")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// ページ番号（IFD の 1 から始まる番号）。書かなければ 1
    public var page: Int64? {
        get { readField("page") }
        set { writeField("page", newValue) }
    }

    /// 出力の長辺の最大ピクセル数。元の長辺がこれを超えるときだけ縦横比を保って縮小する
    public var maxSide: Int64? {
        get { readField("max_side") }
        set { writeField("max_side", newValue) }
    }
}

/// 復号した PNG 画像のサイズ・元のページサイズ・総ページ数と、画像の blob の番号
public struct RasterResult: Message {
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

    /// 出力した PNG の幅（ピクセル）
    public var width: Int64? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 出力した PNG の高さ（ピクセル）
    public var height: Int64? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 元のページの幅（ピクセル）
    public var sourceWidth: Int64? {
        get { readField("source_width") }
        set { writeField("source_width", newValue) }
    }

    /// 元のページの高さ（ピクセル）
    public var sourceHeight: Int64? {
        get { readField("source_height") }
        set { writeField("source_height", newValue) }
    }

    /// 復号したページ番号（1 から）
    public var page: Int64? {
        get { readField("page") }
        set { writeField("page", newValue) }
    }

    /// 文書に含まれる総ページ（IFD）数
    public var pages: Int64? {
        get { readField("pages") }
        set { writeField("pages", newValue) }
    }

    /// PNG 画像の blob の番号（種類は image）。zm_take で取り出す
    public var blob: Int64? {
        get { readField("blob") }
        set { writeField("blob", newValue) }
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

/// 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ、capital_a は A の字形の外形の上（A がなければ大文字の高さ。マイナーバージョン 59 から）
///
/// 列挙の値。知らない値も受け取る
public enum HeightReference {
    public static let capHeight = "cap_height"
    public static let em = "em"
    public static let frame = "frame"
    public static let capitalA = "capital_a"
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

    /// 置いた字形。fonts を書かなければ 0。マイナーバージョン 37 から
    public var glyphs: Int64? {
        get { readField("glyphs") }
        set { writeField("glyphs", newValue) }
    }
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。マイナーバージョン 34 から
public struct CheckRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("check")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。マイナーバージョン 40 から
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

    /// 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
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

/// zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。マイナーバージョン 42 から
public struct ValidateRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("validate")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

/// レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書のバージョンへ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元のバージョンのハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。マイナーバージョン 43 から
public struct ReviewCarryRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("review_carry")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 新しいバージョンの文書の名前。open で作ったもの。マイナーバージョン 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く
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

/// 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しいバージョンに要素がない
///
/// 列挙の値。知らない値も受け取る
public enum CarriedState {
    public static let unchanged = "unchanged"
    public static let changed = "changed"
    public static let removed = "removed"
}

/// 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。マイナーバージョン 43 から
public struct ReviewAdoptRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("review_adopt")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。マイナーバージョン 40 から
    public var fileName: String? {
        get { readField("file_name") }
        set { writeField("file_name", newValue) }
    }

    /// 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
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

/// レビューの文書（決定 0214・0295）の 1 つの対象の図形と指摘の場所を、開いた図面の起点で、図面とは別の 2D のシーンにして blob で返す。対象の形式とハッシュは開いた文書から作り、文書の対象と合わなければ描かない。シーンの配置は scene と同じ schema/scene.toml。色・線幅・寸法は style で明示し、文書の値から推測しない。レビューの文書と図面は変えない。マイナーバージョン 95 から
public struct ReviewSceneRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("review_scene")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 対象の図面の文書の名前。open で作ったもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// レビューの文書の入力の名前（zm_push で渡した JSON）
    public var review: String? {
        get { readField("review") }
        set { writeField("review", newValue) }
    }

    /// 描く対象の ID。レビューの文書の targets の id
    public var target: String? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }

    /// 起点。実寸の空間・記号の定義の空間・用紙の ID（scene と同じ）。文書の図形・場所の view がこの起点を指すものだけを描く
    public var from: String? {
        get { readField("from") }
        set { writeField("from", newValue) }
    }

    /// 起点の用紙のビューの番号。0 から数える。from が用紙のときだけ書ける。書けば、文書の view が用紙の ID に /view/<番号> を足した値の項目だけを描く
    public var view: Int64? {
        get { readField("view") }
        set { writeField("view", newValue) }
    }

    /// [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
    public var region: [Double]? {
        get { readField("region") }
        set { writeField("region", newValue) }
    }

    /// 弦の誤差（起点の長さ）。0 より大きい有限値。既定値を補わない
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 文字を置くフォント（scene と同じ）。style の font の名前で引く。書かなければ字形を置かず、注記の枠を文字高と字数から概算して診断する
    public var fonts: [SceneFont]? {
        get { readField("fonts") }
        set { writeField("fonts", newValue) }
    }

    /// 選んだ項目に共通で当てる色と寸法
    public var style: ReviewSceneStyle? {
        get { readField("style") }
        set { writeField("style", newValue) }
    }
}

/// レビューの図形の色と寸法。幾何の寸法は起点の単位、線幅だけは用紙の mm。文書には保存しない
public struct ReviewSceneStyle: Message {
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

    /// 線・面・文字の色。#rrggbb
    public var color: String? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// 用紙の mm の線幅。0 以上の有限値。0 はもっとも細い線
    public var lineWeight: Double? {
        get { readField("line_weight") }
        set { writeField("line_weight", newValue) }
    }

    /// 起点の単位での文字の高さ。正の有限値
    public var textHeight: Double? {
        get { readField("text_height") }
        set { writeField("text_height", newValue) }
    }

    /// 起点の単位での注記の枠の余白。0 以上の有限値
    public var textPadding: Double? {
        get { readField("text_padding") }
        set { writeField("text_padding", newValue) }
    }

    /// 起点の単位での矢じりの長さ。正の有限値
    public var arrowLength: Double? {
        get { readField("arrow_length") }
        set { writeField("arrow_length", newValue) }
    }

    /// 赤線の矢じりを置く端
    public var arrows: String? {
        get { readField("arrows") }
        set { writeField("arrows", newValue) }
    }

    /// 雲の円弧の弦の長さの上限。起点の単位の正の有限値
    public var cloudPitch: Double? {
        get { readField("cloud_pitch") }
        set { writeField("cloud_pitch", newValue) }
    }

    /// 写真と指摘の位置の記号の直径。起点の単位の正の有限値
    public var markerSize: Double? {
        get { readField("marker_size") }
        set { writeField("marker_size", newValue) }
    }

    /// fonts から引くフォントの名前。書かなければ名前のないフォント
    public var font: String? {
        get { readField("font") }
        set { writeField("font", newValue) }
    }
}

/// 赤線の矢じりを置く端。none は置かない、start は最初の端点、end は最後の端点、both は両端
///
/// 列挙の値。知らない値も受け取る
public enum ReviewArrows {
    public static let none = "none"
    public static let start = "start"
    public static let end = "end"
    public static let both = "both"
}

/// レビューのシーンの要約と、シーンの blob の番号、シーンの ID から元の図形・場所への対応
public struct ReviewSceneResult: Message {
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

    /// 起点の座標の単位。用紙とビューからは millimeter。単位の不明な空間は unknown のまま
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

    /// 描いた図形・場所ごとの対応。描く順（文書の markups の順、続いて topics と各指摘の locations の順）
    public var sources: [ReviewSceneSource]? {
        get { readField("sources") }
        set { writeField("sources", newValue) }
    }

    /// シーンの blob の番号（種類は scene）。zm_take で取り出す
    public var blob: Int64? {
        get { readField("blob") }
        set { writeField("blob", newValue) }
    }
}

/// シーンの ID（シーンの entity_ids）から、元のレビューの文書の図形か指摘の場所へ戻す対応
public struct ReviewSceneSource: Message {
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

    /// シーンの ID。review/markup/<hex> か review/location/<hex>/<hex>
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 元の項目の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 元の図形の ID。kind が markup のときだけ持つ
    public var markup: String? {
        get { readField("markup") }
        set { writeField("markup", newValue) }
    }

    /// 元の指摘の ID。kind が location のときだけ持つ
    public var topic: String? {
        get { readField("topic") }
        set { writeField("topic", newValue) }
    }

    /// 元の場所の ID。kind が location のときだけ持つ
    public var location: String? {
        get { readField("location") }
        set { writeField("location", newValue) }
    }
}

/// シーンの ID の元の項目の種類。markup はレビューの文書の図形、location は指摘の中の場所
///
/// 列挙の値。知らない値も受け取る
public enum ReviewSceneSourceKind {
    public static let markup = "markup"
    public static let location = "location"
}

/// 開いた IFC から階の平面図、または指定した鉛直面の断面図・立面図を作る。平面は storey と height、鉛直面は vertical を指定する
public struct BuildingDrawingRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("building_drawing")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 開いた IFC の文書名
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 作る図面の文書名
    public var to: String? {
        get { readField("to") }
        set { writeField("to", newValue) }
    }

    /// プロジェクト座標系の鉛直面。マイナーバージョン 117 から
    public var vertical: BuildingVerticalView? {
        get { readField("vertical") }
        set { writeField("vertical", newValue) }
    }

    /// 切る高さの基準となる階の要素 ID
    public var storey: String? {
        get { readField("storey") }
        set { writeField("storey", newValue) }
    }

    /// 切る高さ・見る深さ・許容差・弦の誤差の長さの単位。unknown は使えない
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// 階から切る面までの高さ
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 切る面から向こうに見る深さ。正の値
    public var depth: Double? {
        get { readField("depth") }
        set { writeField("depth", newValue) }
    }

    /// 用紙の縮尺の分母。正の値
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }

    /// 同じ点とみなす格子の幅。正の値
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 曲線の近似に使う弦の誤差。正の値
    public var chord: Double? {
        get { readField("chord") }
        set { writeField("chord", newValue) }
    }
}

/// プロジェクト座標系の 2 点を結ぶ鉛直面。図面の縦は上向き、原点は始点。長さは要求の unit
public struct BuildingVerticalView: Message {
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

    /// 図面の原点 [x,y,z]
    public var start: [Double]? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 横軸の方向を決める終点 [x,y,z]。始点と同じ z で、xy は異なる
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 始点から終点へ向かって見る側
    public var side: String? {
        get { readField("side") }
        set { writeField("side", newValue) }
    }

    /// 断面図か立面図
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }
}

/// 始点から終点に向かって左側・右側のどちらから見るか
///
/// 列挙の値。知らない値も受け取る
public enum BuildingViewSide {
    public static let left = "left"
    public static let right = "right"
}

/// section は切断線と見え掛かり、elevation は見え掛かりを出す
///
/// 列挙の値。知らない値も受け取る
public enum BuildingVerticalKind {
    public static let section = "section"
    public static let elevation = "elevation"
}

/// 実寸の線と用紙のビューを持つ図面。線の拡張 zumen.building.drawing に派生 ID・役割・元の要素と形の出所を持つ
public struct BuildingDrawingResult: Message {
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

    /// 作った図面の文書名
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// 図面モデルのバイナリの blob 番号。種類は drawing
    public var blob: Int64? {
        get { readField("blob") }
        set { writeField("blob", newValue) }
    }

    /// 図面に書いた線の数
    public var lines: Int64? {
        get { readField("lines") }
        set { writeField("lines", newValue) }
    }

    /// 切り出しで形状の欠落がなかったか
    public var complete: Bool? {
        get { readField("complete") }
        set { writeField("complete", newValue) }
    }

    /// 形状の評価が完全でなかった要素の数
    public var evaluationFailures: Int64? {
        get { readField("evaluation_failures") }
        set { writeField("evaluation_failures", newValue) }
    }

    /// 投影や切り口の計算に失敗した部品の数
    public var projectionFailures: Int64? {
        get { readField("projection_failures") }
        set { writeField("projection_failures", newValue) }
    }

    /// 格子で点などへ縮んだ辺や輪郭の数
    public var collapsed: Int64? {
        get { readField("collapsed") }
        set { writeField("collapsed", newValue) }
    }
}

/// 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。マイナーバージョン 44 から
public struct BuildingRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("building")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 入力のバイト列のハッシュ。review_carry の対象のバージョンに使う
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

/// 開いた IFC の建物モデルの要素を、書いた順に返す。コンポーネントは components で選ぶ（決定 0207・0219）。マイナーバージョン 44 から
public struct BuildingElementsRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("building_elements")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 返すコンポーネントの名前。1 つ以上で、重ねない。書かなければ class と identity を返す
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

/// building_elements で返すコンポーネント（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員
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
    public static let joinery = "joinery"
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

/// 建物モデルの要素。コンポーネントは、選んだもののうち要素が持つものだけを持つ
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

    /// 要素のコンポーネントのハッシュ（v1: と 32 桁の 16 進、決定 0207）
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

    /// 建具の元の全幅と全高。開口や数量からの推定は含まない
    public var joinery: JoineryComponent? {
        get { readField("joinery") }
        set { writeField("joinery", newValue) }
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

    /// 体系のバージョン
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

/// 建具の実例に直接記録された寸法。値を持たない属性は省く
public struct JoineryComponent: Message {
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

    /// 建具自体の全幅。IFC の OverallWidth
    public var overallWidth: Measured? {
        get { readField("overall_width") }
        set { writeField("overall_width", newValue) }
    }

    /// 建具自体の全高。IFC の OverallHeight
    public var overallHeight: Measured? {
        get { readField("overall_height") }
        set { writeField("overall_height", newValue) }
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

/// 開いた IFC に、変更の一覧を当てて書き戻す（決定 0207・0244）。要素は、building_elements が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイル（ifcZIP で開いたものは ifcZIP）を blobs の file で返す。名前を挙げないインスタンスは、元のバイト列のまま書く。マイナーバージョン 58 から
public struct BuildingApplyRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("building_apply")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 読んだときの文書のハッシュ（building の hash）。書けば、今の文書と同じときだけ当てる
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 変更。並べた順に当てる
    public var edits: [BuildingEdit]? {
        get { readField("edits") }
        set { writeField("edits", newValue) }
    }
}

/// 建物の要素 1 つへの変更。kind の値で形が決まる。add_space・add_element・remove・place・contain・type・void・fill はマイナーバージョン 96 から、add_wall はマイナーバージョン 97 から、add_slab・add_column・add_beam・add_opening・add_door・add_window はマイナーバージョン 98 から
public struct BuildingEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// kind が property の形なら、そのビュー。違えば nil
    public var asBuildingPropertyEdit: BuildingPropertyEdit? {
        taggedView("kind", "property")
    }

    /// kind が attribute の形なら、そのビュー。違えば nil
    public var asBuildingAttributeEdit: BuildingAttributeEdit? {
        taggedView("kind", "attribute")
    }

    /// kind が add_space の形なら、そのビュー。違えば nil
    public var asBuildingAddSpaceEdit: BuildingAddSpaceEdit? {
        taggedView("kind", "add_space")
    }

    /// kind が add_element の形なら、そのビュー。違えば nil
    public var asBuildingAddElementEdit: BuildingAddElementEdit? {
        taggedView("kind", "add_element")
    }

    /// kind が remove の形なら、そのビュー。違えば nil
    public var asBuildingRemoveEdit: BuildingRemoveEdit? {
        taggedView("kind", "remove")
    }

    /// kind が place の形なら、そのビュー。違えば nil
    public var asBuildingPlaceEdit: BuildingPlaceEdit? {
        taggedView("kind", "place")
    }

    /// kind が contain の形なら、そのビュー。違えば nil
    public var asBuildingContainEdit: BuildingContainEdit? {
        taggedView("kind", "contain")
    }

    /// kind が type の形なら、そのビュー。違えば nil
    public var asBuildingTypeEdit: BuildingTypeEdit? {
        taggedView("kind", "type")
    }

    /// kind が void の形なら、そのビュー。違えば nil
    public var asBuildingVoidEdit: BuildingVoidEdit? {
        taggedView("kind", "void")
    }

    /// kind が fill の形なら、そのビュー。違えば nil
    public var asBuildingFillEdit: BuildingFillEdit? {
        taggedView("kind", "fill")
    }

    /// kind が add_wall の形なら、そのビュー。違えば nil
    public var asBuildingAddWallEdit: BuildingAddWallEdit? {
        taggedView("kind", "add_wall")
    }

    /// kind が add_slab の形なら、そのビュー。違えば nil
    public var asBuildingAddSlabEdit: BuildingAddSlabEdit? {
        taggedView("kind", "add_slab")
    }

    /// kind が add_column の形なら、そのビュー。違えば nil
    public var asBuildingAddColumnEdit: BuildingAddColumnEdit? {
        taggedView("kind", "add_column")
    }

    /// kind が add_beam の形なら、そのビュー。違えば nil
    public var asBuildingAddBeamEdit: BuildingAddBeamEdit? {
        taggedView("kind", "add_beam")
    }

    /// kind が add_opening の形なら、そのビュー。違えば nil
    public var asBuildingAddOpeningEdit: BuildingAddOpeningEdit? {
        taggedView("kind", "add_opening")
    }

    /// kind が add_door の形なら、そのビュー。違えば nil
    public var asBuildingAddDoorEdit: BuildingAddDoorEdit? {
        taggedView("kind", "add_door")
    }

    /// kind が add_window の形なら、そのビュー。違えば nil
    public var asBuildingAddWindowEdit: BuildingAddWindowEdit? {
        taggedView("kind", "add_window")
    }

    /// kind が add_grid の形なら、そのビュー。違えば nil
    public var asBuildingAddGridEdit: BuildingAddGridEdit? {
        taggedView("kind", "add_grid")
    }

    /// kind が elevation の形なら、そのビュー。違えば nil
    public var asBuildingElevationEdit: BuildingElevationEdit? {
        taggedView("kind", "elevation")
    }

    /// kind が wall_shape の形なら、そのビュー。違えば nil
    public var asBuildingWallShapeEdit: BuildingWallShapeEdit? {
        taggedView("kind", "wall_shape")
    }

    /// kind が opening_shape の形なら、そのビュー。違えば nil
    public var asBuildingOpeningShapeEdit: BuildingOpeningShapeEdit? {
        taggedView("kind", "opening_shape")
    }

    /// kind が add_type の形なら、そのビュー。違えば nil
    public var asBuildingAddTypeEdit: BuildingAddTypeEdit? {
        taggedView("kind", "add_type")
    }

    /// kind が quantity の形なら、そのビュー。違えば nil
    public var asBuildingQuantityEdit: BuildingQuantityEdit? {
        taggedView("kind", "quantity")
    }

    /// kind が add_unit の形なら、そのビュー。違えば nil
    public var asBuildingAddUnitEdit: BuildingAddUnitEdit? {
        taggedView("kind", "add_unit")
    }

    /// kind が material の形なら、そのビュー。違えば nil
    public var asBuildingMaterialEdit: BuildingMaterialEdit? {
        taggedView("kind", "material")
    }

    /// kind が shape の形なら、そのビュー。違えば nil
    public var asBuildingShapeEdit: BuildingShapeEdit? {
        taggedView("kind", "shape")
    }
}

/// 要素のプロパティのセットの、プロパティ（IfcPropertySingleValue）の値を変える。セットかプロパティがなければ足す。ほかの要素と共有するセットは、写しを作って、この要素だけに付け替える。型の要素では、型のセット（HasPropertySets）を変える
public struct BuildingPropertyEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("property")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 変える要素の ID。building_elements が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。building_elements が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// プロパティのセットの名前（Pset_WallCommon など）
    public var set: String? {
        get { readField("set") }
        set { writeField("set", newValue) }
    }

    /// プロパティの名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 値。type_name は要る。kind は real・integer・text・boolean・logical のどれか
    public var value: BuildingValue? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 要素（IfcRoot）の名前か説明を変える
public struct BuildingAttributeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("attribute")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 変える要素の ID。building_elements が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。building_elements が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 変える属性
    public var attribute: String? {
        get { readField("attribute") }
        set { writeField("attribute", newValue) }
    }

    /// 値。書かなければ、値をなくす
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 変えられる IfcRoot の属性。name は Name、description は Description
///
/// 列挙の値。知らない値も受け取る
public enum BuildingAttribute {
    public static let name = "name"
    public static let description = "description"
}

/// 変更が指す要素。id と hash を書くか、added だけを書く
public struct BuildingRef: Message {
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

    /// 要素の ID。building_elements が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。building_elements が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 同じ要求の edits の、前にある要素を足す変更（add_ で始まる種類）の位置（0 から）。その変更で足した要素を指す
    public var added: Int64? {
        get { readField("added") }
        set { writeField("added", newValue) }
    }
}

/// 要素の配置（IfcLocalPlacement）。relative_to の要素の配置からの相対で、書かなければプロジェクトの空間に置く
public struct BuildingPlacementChange: Message {
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

    /// 配置の親の要素
    public var relativeTo: BuildingRef? {
        get { readField("relative_to") }
        set { writeField("relative_to", newValue) }
    }

    /// 局所の変換。3 行 4 列（回転と移動。building_elements の placement の world と同じ形）。回転は正規直交で行列式が 1、移動は文書の長さの単位
    public var matrix: [[Double]]? {
        get { readField("matrix") }
        set { writeField("matrix", newValue) }
    }
}

/// 足す空間の種類。site は IfcSite、building は IfcBuilding、storey は IfcBuildingStorey
///
/// 列挙の値。知らない値も受け取る
public enum BuildingSpaceKind {
    public static let site = "site"
    public static let building = "building"
    public static let storey = "storey"
}

/// 敷地・建物・階を足し、IfcRelAggregates で親に結ぶ。結果の id は足した空間の ID。マイナーバージョン 96 から
public struct BuildingAddSpaceEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_space")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 空間の種類
    public var space: String? {
        get { readField("space") }
        set { writeField("space", newValue) }
    }

    /// 親。敷地はプロジェクトか敷地、建物はプロジェクト・敷地・建物、階は建物か階。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var parent: BuildingRef? {
        get { readField("parent") }
        set { writeField("parent", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 配置。書かなければ、親の配置に重ねる
    public var placement: BuildingPlacementChange? {
        get { readField("placement") }
        set { writeField("placement", newValue) }
    }

    /// 高さ。敷地は RefElevation、建物は ElevationOfRefHeight、階は Elevation に書く。文書の長さの単位
    public var elevation: Double? {
        get { readField("elevation") }
        set { writeField("elevation", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 要素を足し、container があれば IfcRelContainedInSpatialStructure で空間に結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・屋根・階段・斜路・基礎・杭・扉・窓・開口・代理要素（IfcBuildingElementProxy）。結果の id は足した要素の ID。マイナーバージョン 96 から
public struct BuildingAddElementEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_element")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// IFC の実体の名前（IfcWall など）。文書のバージョンにあるもの
    public var `class`: String? {
        get { readField("class") }
        set { writeField("class", newValue) }
    }

    /// PredefinedType の列挙の値（SOLIDWALL など）。バージョンが持たない実体では書けない
    public var predefined: String? {
        get { readField("predefined") }
        set { writeField("predefined", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 包含する空間（敷地・建物・階・室）。開口には書けない。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var container: BuildingRef? {
        get { readField("container") }
        set { writeField("container", newValue) }
    }

    /// 配置。書かなければ、空間の配置に重ねる（空間もなければ原点）
    public var placement: BuildingPlacementChange? {
        get { readField("placement") }
        set { writeField("placement", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 要素を除く。関係からも外し、空になった関係を除く。壁の開口は一緒に除く。子のある空間と、関係のほかから指される要素は除けない。結果は id を持たない。マイナーバージョン 96 から
public struct BuildingRemoveEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("remove")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 除く要素の ID。building_elements が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。building_elements が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }
}

/// 要素の配置を置き直す。ほかの要素と共有しない配置はその場で書き換え、共有する配置は新しく作る。配置の親をたどって輪になる配置は断る。マイナーバージョン 96 から
public struct BuildingPlaceEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("place")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 置き直す要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var element: BuildingRef? {
        get { readField("element") }
        set { writeField("element", newValue) }
    }

    /// 新しい配置
    public var placement: BuildingPlacementChange? {
        get { readField("placement") }
        set { writeField("placement", newValue) }
    }
}

/// 要素を空間に移す。空間は IfcRelAggregates、要素は IfcRelContainedInSpatialStructure で結び、前の関係から外す。マイナーバージョン 96 から
public struct BuildingContainEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("contain")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 移す要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var element: BuildingRef? {
        get { readField("element") }
        set { writeField("element", newValue) }
    }

    /// 移す先の空間。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var space: BuildingRef? {
        get { readField("space") }
        set { writeField("space", newValue) }
    }
}

/// 要素の型を IfcRelDefinesByType で付け替える。element_type を書かなければ、型を外す。マイナーバージョン 96 から
public struct BuildingTypeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("type")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 型を変える要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var element: BuildingRef? {
        get { readField("element") }
        set { writeField("element", newValue) }
    }

    /// 型の要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var elementType: BuildingRef? {
        get { readField("element_type") }
        set { writeField("element_type", newValue) }
    }
}

/// 開口（IfcOpeningElement）を要素に IfcRelVoidsElement で結ぶ。1 つの開口は 1 つの要素だけに結ぶ。マイナーバージョン 96 から
public struct BuildingVoidEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("void")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 開口を持つ要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var host: BuildingRef? {
        get { readField("host") }
        set { writeField("host", newValue) }
    }

    /// 開口。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var opening: BuildingRef? {
        get { readField("opening") }
        set { writeField("opening", newValue) }
    }
}

/// 材料の層
public struct BuildingWallLayer: Message {
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

    /// 材料の名前（IfcMaterial の Name）。同じ要素の中で同じ名前の層は、同じ材料を指す
    public var material: String? {
        get { readField("material") }
        set { writeField("material", newValue) }
    }

    /// 層の厚さ。文書の長さの単位で、正の値
    public var thickness: Double? {
        get { readField("thickness") }
        set { writeField("thickness", newValue) }
    }
}

/// 芯の 2 点・高さ・材料の層か型で壁を足し、空間に IfcRelContainedInSpatialStructure で結ぶ。配置（芯の始点を原点、芯の向きを x 軸）・Axis と Body の表現・IfcMaterialLayerSetUsage はコアが書く。layers を省けば element_type の IfcMaterialLayerSet を使う。IFC2X3 は IfcWallStandardCase、IFC4 と IFC4X3 は IfcWall。結果の id は足した壁の ID。マイナーバージョン 97 から（element_type は 110 から）
public struct BuildingAddWallEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_wall")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 所属する空間（階など）。壁はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var storey: BuildingRef? {
        get { readField("storey") }
        set { writeField("storey", newValue) }
    }

    /// 芯の始点。空間の配置の [x, y] で、文書の長さの単位
    public var start: [Double]? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 芯の終点。始点と違う [x, y]
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 高さ。文書の長さの単位で、正の値
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 材料の層。始点から終点を見て、芯の右から左へ並べる。省くか空なら element_type の材料の層を使う
    public var layers: [BuildingWallLayer]? {
        get { readField("layers") }
        set { writeField("layers", newValue) }
    }

    /// 芯から最初の層の面までのずれ（OffsetFromReferenceLine、左が正）。書かなければ厚さの合計の半分を負にして、芯を厚さの中央にする
    public var offset: Double? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 結ぶ壁の型（IfcWallType）。マイナーバージョン 110 から
    public var elementType: BuildingRef? {
        get { readField("element_type") }
        set { writeField("element_type", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 矩形の断面
public struct BuildingSection: Message {
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

    /// 幅。文書の長さの単位で、正の値
    public var width: Double? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// せい。文書の長さの単位で、正の値
    public var depth: Double? {
        get { readField("depth") }
        set { writeField("depth", newValue) }
    }
}

/// 外形・材料の層か型で床を足し、空間に包含で結ぶ。上の面を配置の原点にし、層の厚さの合計だけ下へ押し出す Body と、IfcSlab（型が PredefinedType を持たなければ FLOOR）、IfcMaterialLayerSetUsage（AXIS3・NEGATIVE）をコアが書く。layers を省けば element_type の IfcMaterialLayerSet を使う。結果の id は足した床の ID。マイナーバージョン 98 から（element_type は 110 から）
public struct BuildingAddSlabEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_slab")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 所属する空間（階など）。床はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var storey: BuildingRef? {
        get { readField("storey") }
        set { writeField("storey", newValue) }
    }

    /// 外形の頂点。空間の配置の [x, y] の並びで、3 つ以上。最後の点が最初の点と同じなら、その点は除く
    public var outline: [[Double]]? {
        get { readField("outline") }
        set { writeField("outline", newValue) }
    }

    /// 材料の層。上から下へ並べる。省くか空なら element_type の材料の層を使う
    public var layers: [BuildingWallLayer]? {
        get { readField("layers") }
        set { writeField("layers", newValue) }
    }

    /// 上の面の高さ（空間の配置の z）。書かなければ 0
    public var elevation: Double? {
        get { readField("elevation") }
        set { writeField("elevation", newValue) }
    }

    /// 結ぶ床の型（IfcSlabType）。マイナーバージョン 110 から
    public var elementType: BuildingRef? {
        get { readField("element_type") }
        set { writeField("element_type", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 位置・高さ・矩形の断面で柱を足し、空間に包含で結ぶ。断面の中心を配置の原点にして上へ押し出す Body と IfcColumn をコアが書く。結果の id は足した柱の ID。マイナーバージョン 98 から（element_type は 110 から）
public struct BuildingAddColumnEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_column")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 所属する空間（階など）。柱はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var storey: BuildingRef? {
        get { readField("storey") }
        set { writeField("storey", newValue) }
    }

    /// 断面の中心。空間の配置の [x, y]
    public var position: [Double]? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 柱の脚の高さ（空間の配置の z）。書かなければ 0
    public var base: Double? {
        get { readField("base") }
        set { writeField("base", newValue) }
    }

    /// 高さ。文書の長さの単位で、正の値
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 断面。幅は柱の x 軸、せいは y 軸の向き
    public var section: BuildingSection? {
        get { readField("section") }
        set { writeField("section", newValue) }
    }

    /// 柱の x 軸の向き。空間の配置の [x, y]。書かなければ空間の x 軸
    public var direction: [Double]? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// 材料の名前（IfcMaterial の Name）。書けば IfcRelAssociatesMaterial で結ぶ
    public var material: String? {
        get { readField("material") }
        set { writeField("material", newValue) }
    }

    /// 結ぶ柱の型（IfcColumnType）。マイナーバージョン 110 から
    public var elementType: BuildingRef? {
        get { readField("element_type") }
        set { writeField("element_type", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 天端の芯の 2 点・矩形の断面で梁を足し、空間に包含で結ぶ。始点を配置の原点、芯の向きを x 軸、水平で芯の左を y 軸にし、断面を芯に沿って押し出す Body と IfcBeam をコアが書く。鉛直の芯は断る。結果の id は足した梁の ID。マイナーバージョン 98 から（element_type は 110 から）
public struct BuildingAddBeamEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_beam")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 所属する空間（階など）。梁はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var storey: BuildingRef? {
        get { readField("storey") }
        set { writeField("storey", newValue) }
    }

    /// 天端の芯の始点。空間の配置の [x, y, z]
    public var start: [Double]? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 天端の芯の終点。始点と x か y が違う [x, y, z]
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 断面。幅は芯の左右、せいは天端から下へ
    public var section: BuildingSection? {
        get { readField("section") }
        set { writeField("section", newValue) }
    }

    /// 材料の名前（IfcMaterial の Name）。書けば IfcRelAssociatesMaterial で結ぶ
    public var material: String? {
        get { readField("material") }
        set { writeField("material", newValue) }
    }

    /// 結ぶ梁の型（IfcBeamType）。マイナーバージョン 110 から
    public var elementType: BuildingRef? {
        get { readField("element_type") }
        set { writeField("element_type", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 壁の芯に沿う位置・下端・矩形の大きさで開口を足し、IfcRelVoidsElement で壁に穴をあける。開口は壁の配置からの相対に置き、壁の材料の層の使い方（AXIS2）の厚さだけ押し出す Body と IfcOpeningElement をコアが書く。空間には所属させない。層の使い方を持たない要素は断る。結果の id は足した開口の ID。マイナーバージョン 98 から
public struct BuildingAddOpeningEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_opening")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 穴をあける壁。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var host: BuildingRef? {
        get { readField("host") }
        set { writeField("host", newValue) }
    }

    /// 壁の配置の x 軸に沿う、壁の原点から開口の端までの距離
    public var distance: Double? {
        get { readField("distance") }
        set { writeField("distance", newValue) }
    }

    /// 壁の下端から開口の下端までの高さ
    public var sill: Double? {
        get { readField("sill") }
        set { writeField("sill", newValue) }
    }

    /// 幅。文書の長さの単位で、正の値
    public var width: Double? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 高さ。文書の長さの単位で、正の値
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 開口を扉で埋める。扉は開口の配置に置き、壁の層の中央に厚さの 4 分の 1 の板を押し出す Body と、IfcDoor（IFC4 と IFC4X3 で型が PredefinedType を持たなければ DOOR）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。もう埋まった開口は断る。結果の id は足した扉の ID。マイナーバージョン 98 から（Body と element_type は 110 から）
public struct BuildingAddDoorEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_door")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 埋める開口（IfcOpeningElement）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var opening: BuildingRef? {
        get { readField("opening") }
        set { writeField("opening", newValue) }
    }

    /// 全体の幅（OverallWidth）。文書の長さの単位で、正の値。書かなければ Body は開口の幅を使う
    public var width: Double? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 全体の高さ（OverallHeight）。文書の長さの単位で、正の値。書かなければ Body は開口の高さを使う
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 結ぶ扉の型（IFC4 からの IfcDoorType、IFC2X3 の IfcDoorStyle）。マイナーバージョン 110 から
    public var elementType: BuildingRef? {
        get { readField("element_type") }
        set { writeField("element_type", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 開口を窓で埋める。窓は開口の配置に置き、壁の層の中央に厚さの 4 分の 1 の板を押し出す Body と、IfcWindow（IFC4 と IFC4X3 で型が PredefinedType を持たなければ WINDOW）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。もう埋まった開口は断る。結果の id は足した窓の ID。マイナーバージョン 98 から（Body と element_type は 110 から）
public struct BuildingAddWindowEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_window")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 埋める開口（IfcOpeningElement）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var opening: BuildingRef? {
        get { readField("opening") }
        set { writeField("opening", newValue) }
    }

    /// 全体の幅（OverallWidth）。文書の長さの単位で、正の値。書かなければ Body は開口の幅を使う
    public var width: Double? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 全体の高さ（OverallHeight）。文書の長さの単位で、正の値。書かなければ Body は開口の高さを使う
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 結ぶ窓の型（IFC4 からの IfcWindowType、IFC2X3 の IfcWindowStyle）。マイナーバージョン 110 から
    public var elementType: BuildingRef? {
        get { readField("element_type") }
        set { writeField("element_type", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 通り芯の軸。2 点の折れ線の IfcGridAxis に書く。マイナーバージョン 101 から
public struct BuildingGridAxis: Message {
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

    /// 軸の名前（AxisTag）。同じ通り芯の中で重ならない
    public var tag: String? {
        get { readField("tag") }
        set { writeField("tag", newValue) }
    }

    /// 始点。通り芯の配置の [x, y]
    public var start: [Double]? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 終点。始点と違う [x, y]
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }
}

/// U と V の軸で通り芯（IfcGrid）を足し、空間に包含で結ぶ。通り芯は空間の配置からの相対に置く。U か V が空・同じ名前の軸・始点と終点が同じ軸は断る。結果の id は足した通り芯の ID。マイナーバージョン 101 から
public struct BuildingAddGridEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_grid")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 所属する空間（階など）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var storey: BuildingRef? {
        get { readField("storey") }
        set { writeField("storey", newValue) }
    }

    /// U の軸。1 つ以上
    public var u: [BuildingGridAxis]? {
        get { readField("u") }
        set { writeField("u", newValue) }
    }

    /// V の軸。1 つ以上
    public var v: [BuildingGridAxis]? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 高さ（空間の配置の z）。書かなければ 0
    public var elevation: Double? {
        get { readField("elevation") }
        set { writeField("elevation", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 階の高さ（IfcBuildingStorey の Elevation）を直し、階の配置の z を高さの差だけ動かす。今の高さがなければ、配置の z を今の高さとみなす。階に属する要素は配置の相対で付いてくる。結果の id は直した階の ID。マイナーバージョン 103 から
public struct BuildingElevationEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("elevation")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 直す階。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var storey: BuildingRef? {
        get { readField("storey") }
        set { writeField("storey", newValue) }
    }

    /// 新しい高さ。文書の長さの単位
    public var elevation: Double? {
        get { readField("elevation") }
        set { writeField("elevation", newValue) }
    }
}

/// 壁の芯・高さ・材料の層か型を、add_wall と同じ値で直す。配置・表現・材料の層の使い方を書き直し、同じ名前の層は元の IfcMaterial を使う。開口と建具は壁の配置からの相対のまま付いてきて、開口と建具の Body の押し出しを新しい層の範囲に合わせる。合わせられない開口があれば断る。結果の id は直した壁の ID。マイナーバージョン 103 から（element_type は 110 から）
public struct BuildingWallShapeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("wall_shape")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 直す壁（IfcWall か IfcWallStandardCase）。材料の層の使い方（AXIS2）を持つもの。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var wall: BuildingRef? {
        get { readField("wall") }
        set { writeField("wall", newValue) }
    }

    /// 芯の始点。壁の配置の親の配置の [x, y]
    public var start: [Double]? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 芯の終点。始点と違う [x, y]
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 高さ。文書の長さの単位で、正の値
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 材料の層。始点から終点を見て、芯の右から左へ並べる。省くか空なら element_type（なければ今の壁の型）の材料の層を使う
    public var layers: [BuildingWallLayer]? {
        get { readField("layers") }
        set { writeField("layers", newValue) }
    }

    /// 芯から最初の層の面までのずれ（左が正）。書かなければ厚さの合計の半分を負にして、芯を壁の厚さの中央にする
    public var offset: Double? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 付け替える壁の型（IfcWallType）。マイナーバージョン 110 から
    public var elementType: BuildingRef? {
        get { readField("element_type") }
        set { writeField("element_type", newValue) }
    }
}

/// 開口の位置と大きさを、add_opening と同じ値で直す。配置と Body を書き直し、埋める扉と窓の Body と全体の高さ・幅（OverallHeight・OverallWidth）を合わせる。結果の id は直した開口の ID。マイナーバージョン 103 から
public struct BuildingOpeningShapeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("opening_shape")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 直す開口（IfcOpeningElement）。材料の層の使い方を持つ要素に穴をあけるもの。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var opening: BuildingRef? {
        get { readField("opening") }
        set { writeField("opening", newValue) }
    }

    /// 穴をあけた要素の配置の x 軸に沿う、要素の原点から開口の端までの距離
    public var distance: Double? {
        get { readField("distance") }
        set { writeField("distance", newValue) }
    }

    /// 要素の下端から開口の下端までの高さ
    public var sill: Double? {
        get { readField("sill") }
        set { writeField("sill", newValue) }
    }

    /// 幅。文書の長さの単位で、正の値
    public var width: Double? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 高さ。文書の長さの単位で、正の値
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }
}

/// 型を足す。配置と表現は持たず、要素とは type で結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・代理要素の型と、IFC4 からの屋根・階段・斜路・基礎・杭・扉・窓の型。PredefinedType を書かなければ NOTDEFINED を書き、扉と窓の型の OperationType・PartitioningType も NOTDEFINED にする。結果の id は足した型の ID。マイナーバージョン 106 から
public struct BuildingAddTypeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_type")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// IFC の型の実体の名前（IfcWallType など）。文書のバージョンにあるもの
    public var `class`: String? {
        get { readField("class") }
        set { writeField("class", newValue) }
    }

    /// PredefinedType の列挙の値（SOLIDWALL など）
    public var predefined: String? {
        get { readField("predefined") }
        set { writeField("predefined", newValue) }
    }

    /// 利用者が決めた種類（ElementType）
    public var elementType: String? {
        get { readField("element_type") }
        set { writeField("element_type", newValue) }
    }

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    public var globalId: String? {
        get { readField("global_id") }
        set { writeField("global_id", newValue) }
    }
}

/// 要素の数量のセット（IfcElementQuantity）の数量の値を変える。セットか数量がなければ足す。ほかの要素と共有するセットは、写しを作って、この要素だけに付け替える。型の要素では、型のセット（HasPropertySets）を変える。単位は書かず、文書の単位の割り当てに従う。結果の id は要素の ID。マイナーバージョン 106 から
public struct BuildingQuantityEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("quantity")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 変える要素の ID。building_elements が返したもの
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 読んだときの要素のハッシュ。building_elements が返したもの
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 数量のセットの名前（Qto_WallBaseQuantities など）
    public var set: String? {
        get { readField("set") }
        set { writeField("set", newValue) }
    }

    /// セットを足すときの測り方（MethodOfMeasurement）
    public var method: String? {
        get { readField("method") }
        set { writeField("method", newValue) }
    }

    /// 数量の名前（Length など）
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 数量の種類。IfcQuantityLength・IfcQuantityArea・IfcQuantityVolume・IfcQuantityCount・IfcQuantityWeight・IfcQuantityTime のどれを書くか
    public var quantity: String? {
        get { readField("quantity") }
        set { writeField("quantity", newValue) }
    }

    /// 値。文書の単位で、有限の負でない値。IFC4X3 の個数は整数
    public var value: Double? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }

    /// 式（Formula）。IFC2X3 は持たないので、書けば断る
    public var formula: String? {
        get { readField("formula") }
        set { writeField("formula", newValue) }
    }
}

/// プロジェクトの単位の割り当て（IfcUnitAssignment）に SI の単位（IfcSIUnit）を足す。長さと平面角の単位は受けず、同じ種類の単位があれば断る。結果は id を持たない。マイナーバージョン 106 から
public struct BuildingAddUnitEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("add_unit")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 単位の種類（IfcUnitEnum の AREAUNIT など）
    public var unit: String? {
        get { readField("unit") }
        set { writeField("unit", newValue) }
    }

    /// SI の接頭辞（IfcSIPrefix の MILLI など）
    public var prefix: String? {
        get { readField("prefix") }
        set { writeField("prefix", newValue) }
    }

    /// SI の単位の名前（IfcSIUnitName の SQUARE_METRE など）。単位の種類に合うもの
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }
}

/// 要素か型に材料の構成を IfcRelAssociatesMaterial で結ぶ。結んであった材料は外す。同じ名前の材料（IfcMaterial）は文書のものを使う。結果の id は要素の ID。マイナーバージョン 106 から
public struct BuildingMaterialEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("material")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 材料を結ぶ要素か型。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var element: BuildingRef? {
        get { readField("element") }
        set { writeField("element", newValue) }
    }

    /// 材料の構成
    public var material: BuildingMaterialChange? {
        get { readField("material") }
        set { writeField("material", newValue) }
    }
}

/// 結ぶ材料の構成。kind に合うフィールドだけを書く。single は name、list は names、layers は layers と usage、constituents は constituents
public struct BuildingMaterialChange: Message {
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

    /// 構成の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 単一の材料の名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 材料の並びの名前。1 つ以上
    public var names: [String]? {
        get { readField("names") }
        set { writeField("names", newValue) }
    }

    /// 材料の層。1 つ以上
    public var layers: [BuildingMaterialLayer]? {
        get { readField("layers") }
        set { writeField("layers", newValue) }
    }

    /// 層の使い方（IfcMaterialLayerSetUsage）。書かなければ、層の組（IfcMaterialLayerSet）をそのまま結ぶ
    public var usage: BuildingLayerUsage? {
        get { readField("usage") }
        set { writeField("usage", newValue) }
    }

    /// 構成要素。1 つ以上
    public var constituents: [BuildingMaterialConstituent]? {
        get { readField("constituents") }
        set { writeField("constituents", newValue) }
    }
}

/// 結ぶ材料の構成の種類。single は IfcMaterial、list は IfcMaterialList（IFC2X3 だけ。IFC4 からは非推奨なので断る）、layers は IfcMaterialLayerSet、constituents は IfcMaterialConstituentSet（IFC4 から）
///
/// 列挙の値。知らない値も受け取る
public enum BuildingMaterialChangeKind {
    public static let single = "single"
    public static let list = "list"
    public static let layers = "layers"
    public static let constituents = "constituents"
}

/// 材料の層
public struct BuildingMaterialLayer: Message {
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

    /// 材料の名前。書かなければ、材料のない層（空気層など）
    public var material: String? {
        get { readField("material") }
        set { writeField("material", newValue) }
    }

    /// 層の厚さ。文書の長さの単位で、IFC2X3 は正、IFC4 からは負でない値
    public var thickness: Double? {
        get { readField("thickness") }
        set { writeField("thickness", newValue) }
    }

    /// 層の名前。IFC2X3 は持たないので、書けば断る
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }
}

/// 材料の層の使い方
public struct BuildingLayerUsage: Message {
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

    /// 層を重ねる向き
    public var direction: String? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// true なら負の向きに重ねる（NEGATIVE）。書かなければ正の向き
    public var negative: Bool? {
        get { readField("negative") }
        set { writeField("negative", newValue) }
    }

    /// 基準線からのずれ。文書の長さの単位
    public var offset: Double? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }
}

/// 層を重ねる向き（IfcLayerSetDirectionEnum）。axis1 は AXIS1、axis2 は AXIS2、axis3 は AXIS3
///
/// 列挙の値。知らない値も受け取る
public enum BuildingLayerDirection {
    public static let axis1 = "axis1"
    public static let axis2 = "axis2"
    public static let axis3 = "axis3"
}

/// 材料の構成要素
public struct BuildingMaterialConstituent: Message {
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

    /// 材料の名前
    public var material: String? {
        get { readField("material") }
        set { writeField("material", newValue) }
    }

    /// 構成要素の名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }
}

/// 要素の形（Axis と Body の表現）を置き換える。項目は items に並べ、Body に置く項目を body の位置で挙げる。集合演算と写像は、items の前にある項目を位置で指し、同じ項目を何度指してもよい（それぞれに写して書く）。表現の種類（SweptSolid・AdvancedSweptSolid・Brep・SolidModel・Tessellation・Clipping・CSG・MappedRepresentation）はコアが決め、決まらない組み合わせ・表現に直接置く半空間・16 段を超える入れ子・写して 10,000 項目を超える Body は断る。使われなくなった元の表現は除く。要素は配置を持つこと。結果の id は要素の ID。マイナーバージョン 107 から
public struct BuildingShapeEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("shape")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 形を置き換える要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var element: BuildingRef? {
        get { readField("element") }
        set { writeField("element", newValue) }
    }

    /// 芯の線（Axis の IfcPolyline）。要素の配置の [x, y] を 2 点以上。書かなければ Axis を書かない
    public var axis: [[Double]]? {
        get { readField("axis") }
        set { writeField("axis", newValue) }
    }

    /// 形の項目の表
    public var items: [BuildingShapeItem]? {
        get { readField("items") }
        set { writeField("items", newValue) }
    }

    /// Body の表現に置く項目の、items の位置（0 から）。空なら Body を書かない
    public var body: [Int64]? {
        get { readField("body") }
        set { writeField("body", newValue) }
    }
}

/// 形の項目。kind に合うフィールドだけを書く。extrusion は profile・position・direction・depth、revolution は profile・position・axis・angle、swept_disk は directrix・radius・inner_radius・start・end、faces は points・faces・closed、boolean は operation・first・second、half_space は plane・agreement・box か polygon、mapped は origin・target・members。長さは文書の長さの単位
public struct BuildingShapeItem: Message {
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

    /// 項目の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 押し出す・回す断面
    public var profile: BuildingShapeProfile? {
        get { readField("profile") }
        set { writeField("profile", newValue) }
    }

    /// 押し出し・回転の局所の配置。書かなければ要素の配置のまま
    public var position: BuildingAxis3? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 局所の押し出す向き [x, y, z]。z は 0 にできない。書かなければ [0, 0, 1]
    public var direction: [Double]? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// 押し出す深さ。正の値
    public var depth: Double? {
        get { readField("depth") }
        set { writeField("depth", newValue) }
    }

    /// 断面を回す軸。断面の平面（局所の XY 平面）にある。マイナーバージョン 108 から
    public var axis: BuildingShapeRevolutionAxis? {
        get { readField("axis") }
        set { writeField("axis", newValue) }
    }

    /// 回す角度（ラジアン）。0 より大きく 2π 以下。文書の平面角の単位に直して書くので、文書は平面角の単位を持つこと。マイナーバージョン 108 から
    public var angle: Double? {
        get { readField("angle") }
        set { writeField("angle", newValue) }
    }

    /// 円盤を沿わせる折れ線の [x, y, z] を 2 点以上。続く点は重ならない。マイナーバージョン 108 から
    public var directrix: [[Double]]? {
        get { readField("directrix") }
        set { writeField("directrix", newValue) }
    }

    /// 円盤の半径。正の値。マイナーバージョン 108 から
    public var radius: Double? {
        get { readField("radius") }
        set { writeField("radius", newValue) }
    }

    /// 管の内側の半径。正で radius より小さい。書かなければ中実。マイナーバージョン 108 から
    public var innerRadius: Double? {
        get { readField("inner_radius") }
        set { writeField("inner_radius", newValue) }
    }

    /// 掃引を始める折れ線のパラメーター（1 区間で 1 進む）。書かなければ始点。IFC2X3 では書かなければ 0 を書く。マイナーバージョン 108 から
    public var start: Double? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 掃引を終える折れ線のパラメーター。start より大きく区間の数以下。書かなければ終点。IFC2X3 では書かなければ区間の数を書く。マイナーバージョン 108 から
    public var end: Double? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 面集合の点 [x, y, z]
    public var points: [[Double]]? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 面集合の面
    public var faces: [BuildingShapeFace]? {
        get { readField("faces") }
        set { writeField("faces", newValue) }
    }

    /// true なら閉殻（立体。IfcFacetedBrep）。false か書かなければ開殻の面（IFC2X3 は IfcShellBasedSurfaceModel、IFC4 からは IfcPolygonalFaceSet）
    public var closed: Bool? {
        get { readField("closed") }
        set { writeField("closed", newValue) }
    }

    /// 集合演算
    public var operation: String? {
        get { readField("operation") }
        set { writeField("operation", newValue) }
    }

    /// 集合演算の第 1 の対象。items の前にある項目の位置
    public var first: Int64? {
        get { readField("first") }
        set { writeField("first", newValue) }
    }

    /// 集合演算の第 2 の対象。差では第 1 から取り除く。items の前にある項目の位置
    public var second: Int64? {
        get { readField("second") }
        set { writeField("second", newValue) }
    }

    /// 半空間の基準面の配置。局所の z 軸が面の法線
    public var plane: BuildingAxis3? {
        get { readField("plane") }
        set { writeField("plane", newValue) }
    }

    /// true なら法線と逆の側、false か書かなければ法線の向く側を内部とする
    public var agreement: Bool? {
        get { readField("agreement") }
        set { writeField("agreement", newValue) }
    }

    /// 半空間の計算用の囲みの箱（IfcBoxedHalfSpace）。座標系は IFC2X3 では基準面、IFC4 からは要素
    public var box: BuildingShapeBox? {
        get { readField("box") }
        set { writeField("box", newValue) }
    }

    /// 半空間を制限する多角形（IfcPolygonalBoundedHalfSpace）
    public var polygon: BuildingShapePolygon? {
        get { readField("polygon") }
        set { writeField("polygon", newValue) }
    }

    /// 写像元の基準位置。書かなければ原点
    public var origin: BuildingAxis3? {
        get { readField("origin") }
        set { writeField("origin", newValue) }
    }

    /// 写像先の変換
    public var target: BuildingShapeMapping? {
        get { readField("target") }
        set { writeField("target", newValue) }
    }

    /// 写像元の表現の項目。items の前にある項目の位置で、1 つ以上
    public var members: [Int64]? {
        get { readField("members") }
        set { writeField("members", newValue) }
    }
}

/// 形の項目の種類。extrusion は断面の押し出し（IfcExtrudedAreaSolid）、faces は面集合、boolean は集合演算（IfcBooleanResult。差で第 2 が半空間なら IfcBooleanClippingResult）、half_space は平面の半空間（集合演算の対象にだけ置く）、mapped は写像（IfcMappedItem）。revolution は断面の回転（IfcRevolvedAreaSolid）、swept_disk は円盤の掃引（IfcSweptDiskSolid）で、マイナーバージョン 108 から
///
/// 列挙の値。知らない値も受け取る
public enum BuildingShapeItemKind {
    public static let extrusion = "extrusion"
    public static let faces = "faces"
    public static let boolean = "boolean"
    public static let halfSpace = "half_space"
    public static let mapped = "mapped"
    public static let revolution = "revolution"
    public static let sweptDisk = "swept_disk"
}

/// 断面を回す軸（IfcAxis1Placement）。断面の平面にある
public struct BuildingShapeRevolutionAxis: Message {
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

    /// 軸の通る点 [x, y]
    public var location: [Double]? {
        get { readField("location") }
        set { writeField("location", newValue) }
    }

    /// 軸の向き [x, y]。0 にできない
    public var direction: [Double]? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }
}

/// 集合演算。union は和、intersection は積、difference は差
///
/// 列挙の値。知らない値も受け取る
public enum BuildingBooleanOperation {
    public static let union = "union"
    public static let intersection = "intersection"
    public static let difference = "difference"
}

/// 3 次元の局所の配置
public struct BuildingAxis3: Message {
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

    /// 原点 [x, y, z]
    public var location: [Double]? {
        get { readField("location") }
        set { writeField("location", newValue) }
    }

    /// 局所の z 軸 [x, y, z]
    public var axis: [Double]? {
        get { readField("axis") }
        set { writeField("axis", newValue) }
    }

    /// 局所の x 軸の参照方向 [x, y, z]
    public var reference: [Double]? {
        get { readField("reference") }
        set { writeField("reference", newValue) }
    }
}

/// 押し出す断面。rectangle は x と y、circle は radius、polygon は outer と holes を書く
public struct BuildingShapeProfile: Message {
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

    /// 断面の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 断面の名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 矩形の局所の x の全幅。正の値
    public var x: Double? {
        get { readField("x") }
        set { writeField("x", newValue) }
    }

    /// 矩形の局所の y の全高。正の値
    public var y: Double? {
        get { readField("y") }
        set { writeField("y", newValue) }
    }

    /// 円の半径。正の値
    public var radius: Double? {
        get { readField("radius") }
        set { writeField("radius", newValue) }
    }

    /// 矩形と円の中心 [x, y]。書かなければ原点
    public var location: [Double]? {
        get { readField("location") }
        set { writeField("location", newValue) }
    }

    /// 矩形と円の局所の x 軸の参照方向 [x, y]。location と一緒に書く
    public var reference: [Double]? {
        get { readField("reference") }
        set { writeField("reference", newValue) }
    }

    /// 多角形の外輪の [x, y] を 3 点以上。始点に戻る点は書かない
    public var outer: [[Double]]? {
        get { readField("outer") }
        set { writeField("outer", newValue) }
    }

    /// 多角形の内輪
    public var holes: [[[Double]]]? {
        get { readField("holes") }
        set { writeField("holes", newValue) }
    }
}

/// 断面の種類。rectangle は IfcRectangleProfileDef、circle は IfcCircleProfileDef、polygon は IfcArbitraryClosedProfileDef（内輪があれば IfcArbitraryProfileDefWithVoids）
///
/// 列挙の値。知らない値も受け取る
public enum BuildingShapeProfileKind {
    public static let rectangle = "rectangle"
    public static let circle = "circle"
    public static let polygon = "polygon"
}

/// 面集合の面。points の位置（0 から）で外輪と内輪を書く
public struct BuildingShapeFace: Message {
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

    /// 外輪の点の位置。3 つ以上
    public var boundary: [Int64]? {
        get { readField("boundary") }
        set { writeField("boundary", newValue) }
    }

    /// 内輪ごとの点の位置
    public var holes: [[Int64]]? {
        get { readField("holes") }
        set { writeField("holes", newValue) }
    }
}

/// 囲みの箱
public struct BuildingShapeBox: Message {
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

    /// 箱の隅 [x, y, z]
    public var corner: [Double]? {
        get { readField("corner") }
        set { writeField("corner", newValue) }
    }

    /// 各軸の長さ [x, y, z]。正の値
    public var dimensions: [Double]? {
        get { readField("dimensions") }
        set { writeField("dimensions", newValue) }
    }
}

/// 半空間を制限する多角形
public struct BuildingShapePolygon: Message {
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

    /// 境界の配置
    public var position: BuildingAxis3? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 境界の局所の XY 平面の [x, y] を 3 点以上。始点に戻る点は書かない
    public var boundary: [[Double]]? {
        get { readField("boundary") }
        set { writeField("boundary", newValue) }
    }
}

/// 写像先の変換（IfcCartesianTransformationOperator3D）。scales を書けば軸ごとの尺度（NonUniform）
public struct BuildingShapeMapping: Message {
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

    /// 写像先の原点 [x, y, z]。要素の座標系
    public var origin: [Double]? {
        get { readField("origin") }
        set { writeField("origin", newValue) }
    }

    /// x 軸を導く方向 [x, y, z]
    public var axis1: [Double]? {
        get { readField("axis1") }
        set { writeField("axis1", newValue) }
    }

    /// y 軸を導く方向 [x, y, z]
    public var axis2: [Double]? {
        get { readField("axis2") }
        set { writeField("axis2", newValue) }
    }

    /// z 軸を定める方向 [x, y, z]
    public var axis3: [Double]? {
        get { readField("axis3") }
        set { writeField("axis3", newValue) }
    }

    /// 一様な尺度。正の値で、書かなければ 1
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }

    /// 軸ごとの尺度 [x, y, z]。正の値。scale とは一緒に書けない
    public var scales: [Double]? {
        get { readField("scales") }
        set { writeField("scales", newValue) }
    }
}

/// 扉や窓を開口（IfcOpeningElement）に IfcRelFillsElement で結ぶ。1 つの開口は 1 つの要素だけで埋める。マイナーバージョン 96 から
public struct BuildingFillEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("fill")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 開口。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var opening: BuildingRef? {
        get { readField("opening") }
        set { writeField("opening", newValue) }
    }

    /// 埋める要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    public var filler: BuildingRef? {
        get { readField("filler") }
        set { writeField("filler", newValue) }
    }
}

/// 当てた変更の結果。書き戻したファイルは blobs の file で返す
public struct BuildingApplyResult: Message {
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

    /// 書き戻した文書のハッシュ。次の building_apply の前提条件に使う
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }
}

/// 開いた図面の文書を図面モデル（L2）に変換し、図面モデルのバイナリ（決定 0247）を blob で返す。配置は schema/drawing.toml にあり、元の文書の形式・系統・ハッシュも持つ。プラグインのモジュールが読んだ図面は、これを図面のモジュールの open に渡すと、元の形式とハッシュの読むだけの文書になる。マイナーバージョン 60 から
public struct LiftRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("lift")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 文書の名前。open で図面を開いたもの
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }
}

/// 図面モデルのバイナリの blob の番号
public struct LiftResult: Message {
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

    /// 図面モデルのバイナリの blob の番号（種類は drawing）。zm_take で取り出す
    public var blob: Int64? {
        get { readField("blob") }
        set { writeField("blob", newValue) }
    }
}

/// 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。マイナーバージョン 44 から
public struct IdsCheckRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("ids_check")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルのコンポーネントにある属性だけを照らす
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

/// 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。マイナーバージョン 44 から
public struct ReviewAdoptIdsRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("review_adopt_ids")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルのコンポーネントにある属性だけを照らす
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

/// zm_push で渡した BCF 2.1 か 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。マイナーバージョン 45 から
public struct BcfReadRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("bcf_read")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// BCF のバージョン
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

/// zm_push で渡したレビューの文書を、BCF の bcfzip に書く（決定 0214・0220）。バージョンは version で選ぶ（マイナーバージョン 61 から。それまでは 3.0 だけ）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。マイナーバージョン 45 から
public struct BcfWriteRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("bcf_write")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 書く BCF のバージョン。2.1 か 3.0。書かなければ、original のバージョンか 3.0。original と違うバージョンでは書かず、bcf.conversion.unsupported で失敗する。ほかの値は call.value.invalid。マイナーバージョン 61 から
    public var version: String? {
        get { readField("version") }
        set { writeField("version", newValue) }
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

/// レイヤの表の項目（決定 0162）。マイナーバージョン 22 から、状態とスタイルを持つ
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

    /// ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかのフィールドを持たない。マイナーバージョン 22 から
    public var defined: Bool? {
        get { readField("defined") }
        set { writeField("defined", newValue) }
    }

    /// 表示する。マイナーバージョン 22 から
    public var visible: Bool? {
        get { readField("visible") }
        set { writeField("visible", newValue) }
    }

    /// 印刷する。マイナーバージョン 22 から
    public var printable: Bool? {
        get { readField("printable") }
        set { writeField("printable", newValue) }
    }

    /// 編集できる。マイナーバージョン 22 から
    public var editable: Bool? {
        get { readField("editable") }
        set { writeField("editable", newValue) }
    }

    /// 凍結する。マイナーバージョン 22 から
    public var frozen: Bool? {
        get { readField("frozen") }
        set { writeField("frozen", newValue) }
    }

    /// 色。inherit を持たない。定義が色を持たなければ書かない。マイナーバージョン 22 から
    public var color: ColorChange? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }

    /// 線種の ID。定義が線種を持たなければ書かない。マイナーバージョン 22 から
    public var lineTypeId: String? {
        get { readField("line_type_id") }
        set { writeField("line_type_id", newValue) }
    }

    /// 線幅。inherit を持たない。定義が線幅を持たなければ書かない。マイナーバージョン 22 から
    public var lineWeight: LineWeightChange? {
        get { readField("line_weight") }
        set { writeField("line_weight", newValue) }
    }
}

/// 線種の表の項目（決定 0164）。マイナーバージョン 26 から、定義を持つ
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

    /// ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかのフィールドを持たない。マイナーバージョン 26 から
    public var defined: Bool? {
        get { readField("defined") }
        set { writeField("defined", newValue) }
    }

    /// 元の説明。マイナーバージョン 26 から
    public var description: String? {
        get { readField("description") }
        set { writeField("description", newValue) }
    }

    /// 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。マイナーバージョン 26 から
    public var pattern: [Double]? {
        get { readField("pattern") }
        set { writeField("pattern", newValue) }
    }

    /// 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。マイナーバージョン 26 から
    public var paper: Bool? {
        get { readField("paper") }
        set { writeField("paper", newValue) }
    }

    /// 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。マイナーバージョン 26 から
    public var marked: Bool? {
        get { readField("marked") }
        set { writeField("marked", newValue) }
    }

    /// 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。マイナーバージョン 28 から
    public var marks: [LineTypeMark]? {
        get { readField("marks") }
        set { writeField("marks", newValue) }
    }
}

/// 寸法スタイルの表の項目（決定 0165）。マイナーバージョン 27 から、端部記号と大きさを持つ
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

    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。マイナーバージョン 27 から
    public var ends: [String]? {
        get { readField("ends") }
        set { writeField("ends", newValue) }
    }

    /// 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。マイナーバージョン 27 から
    public var leader: String? {
        get { readField("leader") }
        set { writeField("leader", newValue) }
    }

    /// 端部記号の大きさ。文書の単位で、scale を掛ける前の値。マイナーバージョン 27 から
    public var size: Double? {
        get { readField("size") }
        set { writeField("size", newValue) }
    }

    /// 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。マイナーバージョン 27 から
    public var tick: Double? {
        get { readField("tick") }
        set { writeField("tick", newValue) }
    }

    /// 大きさに掛ける倍率。0 はビューの縮尺から決める。マイナーバージョン 27 から
    public var scale: Double? {
        get { readField("scale") }
        set { writeField("scale", newValue) }
    }
}

/// 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。マイナーバージョン 28 から
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

/// 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。マイナーバージョン 20 から
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

    /// ファイルに定義がある。偽なら、ほかのフィールドを持たない
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

/// 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つキー（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない
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

    /// SXF のインスタンスの番号（#n の n）。10 進の文字列。マイナーバージョン 6 から
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

    /// メッセージのスキーマのメジャーバージョン
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

/// バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、scene3d は 3D のシーン（schema/scene3d.toml の配置）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）、drawing は図面モデルのバイナリ（schema/drawing.toml の配置、決定 0247。マイナーバージョン 60 から）
///
/// 列挙の値。知らない値も受け取る
public enum BlobKind {
    public static let file = "file"
    public static let image = "image"
    public static let scene = "scene"
    public static let arrow = "arrow"
    public static let drawing = "drawing"
    public static let scene3d = "scene3d"
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

    /// メッセージのスキーマのメジャーバージョン。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 実行の形態。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013）
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

/// 実行の形態（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
///
/// 列挙の値。知らない値も受け取る
public enum Engine {
    public static let native = "native"
    public static let process = "process"
    public static let wasm = "wasm"
}

/// 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML・IFC4.3・SIMA の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面・BIM・土木地理・点群のモジュールに置く（決定 0253）。マイナーバージョン 46 から（doc と id はマイナーバージョン 47 から）
public struct CivilRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("civil")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// doc の座標点・高さ基準と、選んだ線形の IP・地盤縦断・片勾配・横断の値を返す。複数の線形があれば id が必要。省略は false。マイナーバージョン 87 から
    public var details: Bool? {
        get { readField("details") }
        set { writeField("details", newValue) }
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

    /// open で開いた J-LandXML・IFC4.3・SIMA の文書の名前。マイナーバージョン 47 から
    public var doc: String? {
        get { readField("doc") }
        set { writeField("doc", newValue) }
    }

    /// doc の線形の ID。文書に線形が 1 つなら書かなくてよい。マイナーバージョン 47 から
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

    /// details を求めた場合の保持値。マイナーバージョン 87 から
    public var details: CivilDetails? {
        get { readField("details") }
        set { writeField("details", newValue) }
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

    /// 所を求めた doc の線形の ID。マイナーバージョン 47 から
    public var alignment: String? {
        get { readField("alignment") }
        set { writeField("alignment", newValue) }
    }

    /// doc の線形の要約。doc を渡したときに返す。マイナーバージョン 47 から
    public var alignments: [CivilAlignmentSummary]? {
        get { readField("alignments") }
        set { writeField("alignments", newValue) }
    }

    /// doc の地表面（TIN）の要約。doc を渡したときに返す。マイナーバージョン 47 から
    public var surfaces: [CivilSurfaceSummary]? {
        get { readField("surfaces") }
        set { writeField("surfaces", newValue) }
    }

    /// doc の水平の座標系の EPSG のコード。分からなければ書かない。マイナーバージョン 47 から
    public var crs: Int64? {
        get { readField("crs") }
        set { writeField("crs", newValue) }
    }
}

/// 土木モデルの保持値。スキーマ 1.87 から
public struct CivilDetails: Message {
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

    /// 選んだ線形の値。線形がなければ書かない
    public var alignment: CivilAlignmentValues? {
        get { readField("alignment") }
        set { writeField("alignment", newValue) }
    }

    /// 文書の座標点セット
    public var pointSets: [CivilPointSet]? {
        get { readField("point_sets") }
        set { writeField("point_sets", newValue) }
    }

    /// 文書の高さの基準。値は自動で T.P. に直さない
    public var heightReference: CivilHeightReference? {
        get { readField("height_reference") }
        set { writeField("height_reference", newValue) }
    }
}

/// 元の高さの基準
public struct CivilHeightReference: Message {
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

    /// 元の基準名
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 元の高さに足すと T.P. の高さになる差（メートル）。不明なら書かない
    public var tokyoBayOffset: Double? {
        get { readField("tokyo_bay_offset") }
        set { writeField("tokyo_bay_offset", newValue) }
    }

    /// 元の位置
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 線形の付随する値。元の順序を保つ
public struct CivilAlignmentValues: Message {
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

    /// 同じ名前の縦断が複数あり、名前で参照先を一意に定められない
    public var verticalNameAmbiguous: Bool? {
        get { readField("vertical_name_ambiguous") }
        set { writeField("vertical_name_ambiguous", newValue) }
    }

    /// 選んだ線形の ID
    public var id: String? {
        get { readField("id") }
        set { writeField("id", newValue) }
    }

    /// 保持した縦断線形の元の名前
    public var verticalName: String? {
        get { readField("vertical_name") }
        set { writeField("vertical_name", newValue) }
    }

    /// IP の列。始点・終点を含む
    public var intersectionPoints: [CivilSurveyPoint]? {
        get { readField("intersection_points") }
        set { writeField("intersection_points", newValue) }
    }

    /// 地盤の縦断の折れ線
    public var groundProfiles: [CivilGroundProfile]? {
        get { readField("ground_profiles") }
        set { writeField("ground_profiles", newValue) }
    }

    /// 片勾配とカントの区間
    public var cant: [CivilCant]? {
        get { readField("cant") }
        set { writeField("cant", newValue) }
    }

    /// 横断の線と点
    public var crossSections: [CivilCrossSection]? {
        get { readField("cross_sections") }
        set { writeField("cross_sections", newValue) }
    }
}

/// IP・基準点などの座標点
public struct CivilSurveyPoint: Message {
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

    /// 元の名前。永続 ID と同一視しない
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 注記
    public var description: String? {
        get { readField("description") }
        set { writeField("description", newValue) }
    }

    /// 北の座標（メートル）
    public var northing: Double? {
        get { readField("northing") }
        set { writeField("northing", newValue) }
    }

    /// 東の座標（メートル）
    public var easting: Double? {
        get { readField("easting") }
        set { writeField("easting", newValue) }
    }

    /// 文書の基準での標高（メートル）。省略は 0 にしない
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 基準点・水準点の種類
    public var `class`: String? {
        get { readField("class") }
        set { writeField("class", newValue) }
    }

    /// 元の位置
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 元の順序を持つ座標点セット
public struct CivilPointSet: Message {
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

    /// 元の名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 注記
    public var description: String? {
        get { readField("description") }
        set { writeField("description", newValue) }
    }

    /// 点
    public var points: [CivilSurveyPoint]? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 元の位置
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 地盤の縦断線
public struct CivilGroundProfile: Message {
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

    /// 元の名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 注記
    public var description: String? {
        get { readField("description") }
        set { writeField("description", newValue) }
    }

    /// 追加距離と高さ
    public var points: [CivilProfilePoint]? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 元の位置
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 地盤縦断の点
public struct CivilProfilePoint: Message {
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

    /// 線形の初めからの追加距離（メートル）
    public var distance: Double? {
        get { readField("distance") }
        set { writeField("distance", newValue) }
    }

    /// 文書の基準での標高（メートル）
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }
}

/// 片勾配またはカントの区間
public struct CivilCant: Message {
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

    /// 初めの追加距離（メートル）
    public var startDistance: Double? {
        get { readField("start_distance") }
        set { writeField("start_distance", newValue) }
    }

    /// 区間の長さ（メートル）
    public var length: Double? {
        get { readField("length") }
        set { writeField("length", newValue) }
    }

    /// superelevation は道路の左右の勾配（%）、rail は鉄道の左右の高さの差（mm）
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 初めの左と右
    public var start: [Double]? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 終わりの左と右
    public var end: [Double]? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 鉄道の軌間（mm）。不明なら書かない
    public var gauge: Double? {
        get { readField("gauge") }
        set { writeField("gauge", newValue) }
    }

    /// 変わり方。元の値
    public var transition: String? {
        get { readField("transition") }
        set { writeField("transition", newValue) }
    }

    /// 元の位置
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 横断
public struct CivilCrossSection: Message {
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

    /// 名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 参照する縦断線形の元の名前
    public var profile: String? {
        get { readField("profile") }
        set { writeField("profile", newValue) }
    }

    /// 横断の線
    public var lines: [CivilCrossSectionLine]? {
        get { readField("lines") }
        set { writeField("lines", newValue) }
    }

    /// 元の位置
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 横断の線。点の元の値を保つ
public struct CivilCrossSectionLine: Message {
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

    /// 注記
    public var description: String? {
        get { readField("description") }
        set { writeField("description", newValue) }
    }

    /// 幅員中心から見た側。元の値。離れの符号は変えない
    public var side: String? {
        get { readField("side") }
        set { writeField("side", newValue) }
    }

    /// existing、design または元の値
    public var role: String? {
        get { readField("role") }
        set { writeField("role", newValue) }
    }

    /// elevation は文書の標高、formation_offset はその所の計画高からの差
    public var heightReference: String? {
        get { readField("height_reference") }
        set { writeField("height_reference", newValue) }
    }

    /// 離れと元の高さ
    public var points: [CivilCrossSectionPoint]? {
        get { readField("points") }
        set { writeField("points", newValue) }
    }

    /// 元の位置
    public var source: Source? {
        get { readField("source") }
        set { writeField("source", newValue) }
    }
}

/// 横断の点
public struct CivilCrossSectionPoint: Message {
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

    /// 中心線からの離れ（右を正、メートル）
    public var offset: Double? {
        get { readField("offset") }
        set { writeField("offset", newValue) }
    }

    /// 線の height_reference に従う高さ（メートル）
    public var height: Double? {
        get { readField("height") }
        set { writeField("height", newValue) }
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

/// open で開いた点群（LAS・LAZ・PLY・XYZ・E57・GeoTIFF の DEM）の要約と、条件に合う点を返す。点は八分木の浅い段のノードから順に選ぶ（決定 0232）。点群のモジュールに置く（決定 0253）。マイナーバージョン 48 から
public struct PointcloudRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("pointcloud")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 八分木のノードの数
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

/// 管理 XML の項目を変更し、file blob を返す。入力は変更しない。すべての経路は元の文書で解き、失敗時は結果を返さない。図面のモジュールに置く。マイナーバージョン 84 から
public struct DeliveryApplyRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("delivery_apply")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// 管理 XML の入力の名前
    public var input: String? {
        get { readField("input") }
        set { writeField("input", newValue) }
    }

    /// 元の XML の文書ハッシュ。違えば変更しない。省略すれば検査しない
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 指定する順の変更。同じ親への追加は順序を保つ
    public var edits: [DeliveryEdit]? {
        get { readField("edits") }
        set { writeField("edits", newValue) }
    }

    /// 版の照合に使うプロファイルと継承元の入力名
    public var profiles: [String]? {
        get { readField("profiles") }
        set { writeField("profiles", newValue) }
    }

    /// 編集後に照らす基準の系統と版。省略すると編集だけを行う
    public var profile: ProfileRef? {
        get { readField("profile") }
        set { writeField("profile", newValue) }
    }
}

/// 変更後の管理 XML の情報。file blob は 1 個。入力は変更しない
public struct DeliveryApplyResult: Message {
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

    /// 元の入力の名前
    public var input: String? {
        get { readField("input") }
        set { writeField("input", newValue) }
    }

    /// 変更後のバイト列の文書ハッシュ
    public var hash: String? {
        get { readField("hash") }
        set { writeField("hash", newValue) }
    }

    /// 適用した変更の数
    public var applied: Int64? {
        get { readField("applied") }
        set { writeField("applied", newValue) }
    }

    /// 版を指定したときの部分照合。違反があっても編集後のファイルを返す
    public var validation: DeliveryEditValidation? {
        get { readField("validation") }
        set { writeField("validation", newValue) }
    }
}

/// 指定版で編集した箇所を照らした結果。マイナーバージョン 115 から
public struct DeliveryEditValidation: Message {
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

    /// 解決したプロファイルを継承元から並べた列
    public var chain: [ProfileIdentity]? {
        get { readField("chain") }
        set { writeField("chain", newValue) }
    }

    /// 変更と規則の順に並べた結果
    public var checks: [DeliveryEditCheck]? {
        get { readField("checks") }
        set { writeField("checks", newValue) }
    }

    /// 照合の対象外。子の順序と最小数、属性、追加した部分木の内容、変更していない項目
    public var unchecked: [String]? {
        get { readField("unchecked") }
        set { writeField("unchecked", newValue) }
    }
}

/// 変更に対する版ごとの規則と出力 XML のバイト範囲
public struct DeliveryEditCheck: Message {
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

    /// edits の位置。0 から数える
    public var edit: Int64? {
        get { readField("edit") }
        set { writeField("edit", newValue) }
    }

    /// matched・violated・unchecked のいずれか
    public var status: String? {
        get { readField("status") }
        set { writeField("status", newValue) }
    }

    /// 照らした text・child_count、または確認できない理由
    public var reason: String? {
        get { readField("reason") }
        set { writeField("reason", newValue) }
    }

    /// 照らした規則の ID。該当する規則がなければ省略する
    public var rule: String? {
        get { readField("rule") }
        set { writeField("rule", newValue) }
    }

    /// 変更後の対象要素の最初のバイト
    public var start: Int64? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 変更後の対象要素の末尾の次のバイト
    public var end: Int64? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }
}

/// 根からたどる子。接頭辞を含む名前で区別する
public struct DeliveryPathStep: Message {
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

    /// 接頭辞を含む元の要素名
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 同名の兄弟の中の位置。0 から数え、省略すると 0
    public var index: Int64? {
        get { readField("index") }
        set { writeField("index", newValue) }
    }
}

/// 親の名前空間を引き継ぐ追加要素。属性と名前空間宣言は追加しない
public struct DeliveryNewElement: Message {
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

    /// 接頭辞を含む要素名。接頭辞は親で宣言済みのものに限る
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 子の前に書く文字。文字参照にする前の値
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }

    /// 子の要素。書く順。省略すると空
    public var children: [DeliveryNewElement]? {
        get { readField("children") }
        set { writeField("children", newValue) }
    }
}

/// 元の文書の項目の更新か、子の記録の追加。空の path は根を指す。同じ項目の更新を重ねたり、更新と追加を混ぜたりしない
public struct DeliveryEdit: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// kind が set_text の形なら、そのビュー。違えば nil
    public var asDeliverySetText: DeliverySetText? {
        taggedView("kind", "set_text")
    }

    /// kind が append の形なら、そのビュー。違えば nil
    public var asDeliveryAppend: DeliveryAppend? {
        taggedView("kind", "append")
    }
}

/// 子の要素を持たない項目の文字を置き換える。注記と処理命令は保つ
public struct DeliverySetText: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("set_text")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 根からの経路
    public var path: [DeliveryPathStep]? {
        get { readField("path") }
        set { writeField("path", newValue) }
    }

    /// 文字参照にする前の値
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 親の末尾に子の記録を足す。名前・文字コード・予算を検査する
public struct DeliveryAppend: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["kind": .string("append")]
    }

    /// 変更の種類
    public var kind: String? {
        get { readField("kind") }
        set { writeField("kind", newValue) }
    }

    /// 親の根からの経路
    public var path: [DeliveryPathStep]? {
        get { readField("path") }
        set { writeField("path", newValue) }
    }

    /// 追加する記録の木
    public var child: DeliveryNewElement? {
        get { readField("child") }
        set { writeField("child", newValue) }
    }
}

/// 電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。マイナーバージョン 53 から
public struct DeliveryCheckRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("delivery_check")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

/// 工事写真（JPEG）の Exif と XMP を読み、画素数・向き・撮影の日時・位置などを返す（#387）。画像は復号しない。図面のモジュールに置く。マイナーバージョン 55 から
public struct PhotoRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("photo")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// JPEG の入力の名前
    public var input: String? {
        get { readField("input") }
        set { writeField("input", newValue) }
    }
}

/// 写真の情報。ファイルが持たない値は書かない
public struct PhotoResult: Message {
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

    /// 画素の幅（フレームのヘッダー）
    public var width: Int64? {
        get { readField("width") }
        set { writeField("width", newValue) }
    }

    /// 画素の高さ（フレームのヘッダー）
    public var height: Int64? {
        get { readField("height") }
        set { writeField("height", newValue) }
    }

    /// 向き（Exif の Orientation、1〜8）
    public var orientation: Int64? {
        get { readField("orientation") }
        set { writeField("orientation", newValue) }
    }

    /// 撮影の日時（YYYY-MM-DDTHH:MM:SS、時差があれば +09:00 などを足す）
    public var taken: String? {
        get { readField("taken") }
        set { writeField("taken", newValue) }
    }

    /// 機器の作り手
    public var make: String? {
        get { readField("make") }
        set { writeField("make", newValue) }
    }

    /// 機器の型
    public var model: String? {
        get { readField("model") }
        set { writeField("model", newValue) }
    }

    /// 撮った位置（GPS の緯度・経度、度）。測地系は換算しない
    public var position: PhotoPosition? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 撮った向き（度）
    public var direction: Double? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// 向きの基準（T は真北、M は磁北）
    public var directionReference: String? {
        get { readField("direction_reference") }
        set { writeField("direction_reference", newValue) }
    }

    /// GPS の測地系の名前（GPSMapDatum）
    public var datum: String? {
        get { readField("datum") }
        set { writeField("datum", newValue) }
    }

    /// XMP の性質。文書の順
    public var xmp: [PhotoProperty]? {
        get { readField("xmp") }
        set { writeField("xmp", newValue) }
    }
}

/// 撮った位置
public struct PhotoPosition: Message {
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

    /// 緯度（度、北が正）
    public var latitude: Double? {
        get { readField("latitude") }
        set { writeField("latitude", newValue) }
    }

    /// 経度（度、東が正）
    public var longitude: Double? {
        get { readField("longitude") }
        set { writeField("longitude", newValue) }
    }

    /// 高さ（m、海面より下は負）
    public var altitude: Double? {
        get { readField("altitude") }
        set { writeField("altitude", newValue) }
    }
}

/// XMP の性質
public struct PhotoProperty: Message {
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

    /// 名前空間の URI
    public var namespace: String? {
        get { readField("namespace") }
        set { writeField("namespace", newValue) }
    }

    /// 接頭辞を除いた名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 値。配列は最初の項目の値
    public var value: String? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }
}

/// 地質・土質調査のボーリング交換用データ（BED0500.DTD の XML）を読み、ボーリングの位置・標高と、地層・色調・観察記事・標準貫入試験・孔内水位を返す（#390）。ほかの試験と判定表は読まない。図面のモジュールに置く。マイナーバージョン 57 から
public struct BoringRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("boring")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// ボーリング交換用データの入力の名前
    public var input: String? {
        get { readField("input") }
        set { writeField("input", newValue) }
    }
}

/// 読んだボーリング。深さは地表からの m（GL.-m）
public struct BoringResult: Message {
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

    /// 適用規格。文書の順
    public var standards: [String]? {
        get { readField("standards") }
        set { writeField("standards", newValue) }
    }

    /// 公開フラグのコード
    public var publication: String? {
        get { readField("publication") }
        set { writeField("publication", newValue) }
    }

    /// 事業工事名
    public var project: String? {
        get { readField("project") }
        set { writeField("project", newValue) }
    }

    /// 調査名
    public var survey: String? {
        get { readField("survey") }
        set { writeField("survey", newValue) }
    }

    /// ボーリング名
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// ボーリング総数
    public var count: Int64? {
        get { readField("count") }
        set { writeField("count", newValue) }
    }

    /// ボーリング連番
    public var serial: Int64? {
        get { readField("serial") }
        set { writeField("serial", newValue) }
    }

    /// 孔口の位置。測地系は換算しない
    public var position: BoringPosition? {
        get { readField("position") }
        set { writeField("position", newValue) }
    }

    /// 位置の測地系
    public var datum: String? {
        get { readField("datum") }
        set { writeField("datum", newValue) }
    }

    /// 調査位置の住所
    public var address: String? {
        get { readField("address") }
        set { writeField("address", newValue) }
    }

    /// 調査期間の始まりの日（YYYY-MM-DD）
    public var start: String? {
        get { readField("start") }
        set { writeField("start", newValue) }
    }

    /// 調査期間の終わりの日（YYYY-MM-DD）
    public var end: String? {
        get { readField("end") }
        set { writeField("end", newValue) }
    }

    /// 孔口標高
    public var elevation: BoringHeight? {
        get { readField("elevation") }
        set { writeField("elevation", newValue) }
    }

    /// 総削孔長（m）
    public var length: Double? {
        get { readField("length") }
        set { writeField("length", newValue) }
    }

    /// 削孔の角度（度）
    public var angle: Double? {
        get { readField("angle") }
        set { writeField("angle", newValue) }
    }

    /// 削孔の方位（度）
    public var direction: Double? {
        get { readField("direction") }
        set { writeField("direction", newValue) }
    }

    /// 工学的地質区分名・現場土質名。文書の順
    public var layers: [BoringLayer]? {
        get { readField("layers") }
        set { writeField("layers", newValue) }
    }

    /// 色調。文書の順
    public var colors: [BoringColor]? {
        get { readField("colors") }
        set { writeField("colors", newValue) }
    }

    /// 観察記事。文書の順
    public var observations: [BoringObservation]? {
        get { readField("observations") }
        set { writeField("observations", newValue) }
    }

    /// 標準貫入試験。文書の順
    public var penetrations: [BoringPenetration]? {
        get { readField("penetrations") }
        set { writeField("penetrations", newValue) }
    }

    /// 孔内水位。文書の順
    public var waterLevels: [BoringWaterLevel]? {
        get { readField("water_levels") }
        set { writeField("water_levels", newValue) }
    }
}

/// 経度・緯度（度）。度・分・秒から求める
public struct BoringPosition: Message {
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

    /// 緯度（度、北が正）
    public var latitude: Double? {
        get { readField("latitude") }
        set { writeField("latitude", newValue) }
    }

    /// 経度（度、東が正）
    public var longitude: Double? {
        get { readField("longitude") }
        set { writeField("longitude", newValue) }
    }
}

/// 測地系。tokyo は日本測地系（コード 00）、jgd2000・jgd2011・jgd2024 は世界測地系（コード 01〜03）
///
/// 列挙の値。知らない値も受け取る
public enum BoringDatum {
    public static let tokyo = "tokyo"
    public static let jgd2000 = "jgd2000"
    public static let jgd2011 = "jgd2011"
    public static let jgd2024 = "jgd2024"
}

/// 標高と、その基準と成果（決定 0019）
public struct BoringHeight: Message {
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

    /// 標高（m）
    public var value: Double? {
        get { readField("value") }
        set { writeField("value", newValue) }
    }

    /// 高さの基準。孔口標高は tokyo_bay（東京湾平均海面）とする
    public var datum: String? {
        get { readField("datum") }
        set { writeField("datum", newValue) }
    }

    /// 成果。測地系が jgd2024 なら survey_2024、そうでなければ調査の日から仮定する
    public var realization: String? {
        get { readField("realization") }
        set { writeField("realization", newValue) }
    }
}

/// 高さの基準（決定 0019）。tokyo_bay は東京湾平均海面（T.P.）、island は離島の独自の平均海面、ellipsoid は楕円体（GRS80）
///
/// 列挙の値。知らない値も受け取る
public enum HeightDatum {
    public static let tokyoBay = "tokyo_bay"
    public static let island = "island"
    public static let ellipsoid = "ellipsoid"
}

/// 標高の成果（決定 0019）。survey_2011 は測地成果2011（2025-03-31 まで）、survey_2024 は測地成果2024（2025-04-01 から）
///
/// 列挙の値。知らない値も受け取る
public enum HeightRealization {
    public static let survey2011 = "survey_2011"
    public static let survey2024 = "survey_2024"
}

/// 地層の区間。上端は 1 つ前の区間の下端（最初は 0）
public struct BoringLayer: Message {
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

    /// 下端の深さ（m）
    public var bottom: Double? {
        get { readField("bottom") }
        set { writeField("bottom", newValue) }
    }

    /// 工学的地質区分名・現場土質名
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }

    /// 記号
    public var symbol: String? {
        get { readField("symbol") }
        set { writeField("symbol", newValue) }
    }
}

/// 色調の区間
public struct BoringColor: Message {
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

    /// 下端の深さ（m）
    public var bottom: Double? {
        get { readField("bottom") }
        set { writeField("bottom", newValue) }
    }

    /// 色調の名前
    public var name: String? {
        get { readField("name") }
        set { writeField("name", newValue) }
    }
}

/// 観察記事
public struct BoringObservation: Message {
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

    /// 上端の深さ（m）
    public var top: Double? {
        get { readField("top") }
        set { writeField("top", newValue) }
    }

    /// 下端の深さ（m）
    public var bottom: Double? {
        get { readField("bottom") }
        set { writeField("bottom", newValue) }
    }

    /// 記事
    public var text: String? {
        get { readField("text") }
        set { writeField("text", newValue) }
    }
}

/// 標準貫入試験
public struct BoringPenetration: Message {
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

    /// 開始の深さ（m）
    public var depth: Double? {
        get { readField("depth") }
        set { writeField("depth", newValue) }
    }

    /// 合計の打撃回数
    public var blows: Int64? {
        get { readField("blows") }
        set { writeField("blows", newValue) }
    }

    /// 合計の貫入量（mm）
    public var penetration: Double? {
        get { readField("penetration") }
        set { writeField("penetration", newValue) }
    }
}

/// 孔内水位
public struct BoringWaterLevel: Message {
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

    /// 測定の日（YYYY-MM-DD）
    public var date: String? {
        get { readField("date") }
        set { writeField("date", newValue) }
    }

    /// 削孔状況のコード
    public var state: String? {
        get { readField("state") }
        set { writeField("state", newValue) }
    }

    /// 水位の深さ（m）。地表より高ければ負。水位がなければ書かない
    public var depth: Double? {
        get { readField("depth") }
        set { writeField("depth", newValue) }
    }

    /// 水位種別・備考
    public var note: String? {
        get { readField("note") }
        set { writeField("note", newValue) }
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

/// open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。土木・地理のモジュールに置く（決定 0253）。マイナーバージョン 52 から
public struct GeoRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("geo")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書の名前
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

/// open で開いた J-LandXML・IFC4.3・SIMA の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。マイナーバージョン 51 から
public struct CivilDrawingRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("civil_drawing")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    public var timeLimitMs: Int64? {
        get { readField("time_limit_ms") }
        set { writeField("time_limit_ms", newValue) }
    }

    /// op の名前
    public var op: String? {
        get { readField("op") }
        set { writeField("op", newValue) }
    }

    /// open で開いた J-LandXML・IFC4.3・SIMA の文書の名前
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

/// 開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。点群のモジュールに置く（決定 0253）。マイナーバージョン 50 から
public struct DeviationRequest: Message {
    /// 包んでいる JSON のオブジェクト
    public var json: [String: JSON]

    /// 読んだ JSON のオブジェクトを包む
    public init(json: [String: JSON]) {
        self.json = json
    }

    /// タグのフィールドだけを持つオブジェクトから作る
    public init() {
        json = ["op": .string("deviation")]
    }

    /// メッセージのスキーマのメジャーバージョン。今は 1
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

    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
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

    /// 差の色の段で点を塗った 3D のシーンも返す。点の選び方は点群の scene3d と同じ。89 から
    public var scene3d: DeviationScene3d? {
        get { readField("scene3d") }
        set { writeField("scene3d", newValue) }
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

    /// 要求に scene3d を書いたときの 3D のシーン。点の色は bands の段の色で、元の色は持たない。強度と分類は元の値。89 から
    public var scene3d: Scene3dResult? {
        get { readField("scene3d") }
        set { writeField("scene3d", newValue) }
    }

    /// 要求に scene3d を書いたときの色の段。規則の順に、その規則で起こりうる段を、差の小さい順に並べ、最後に評価しなかった点の段を置く。89 から
    public var bands: [DeviationBand]? {
        get { readField("bands") }
        set { writeField("bands", newValue) }
    }
}

/// 差の色分けの 3D のシーンの点の選び方。意味は点群の scene3d の box・tolerance・limit と同じ
public struct DeviationScene3d: Message {
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

    /// 点群の元の座標系の東・北・高さのメートルの箱
    public var box: Scene3dBox? {
        get { readField("box") }
        set { writeField("box", newValue) }
    }

    /// 点の間隔（メートル）の正の有限値。これより細かい段の節を選ばない
    public var tolerance: Double? {
        get { readField("tolerance") }
        set { writeField("tolerance", newValue) }
    }

    /// 返す点の数の上限。省略時は選んだすべての点
    public var limit: Int64? {
        get { readField("limit") }
        set { writeField("limit", newValue) }
    }
}

/// 差の色の段。差は mm で、規則の each（個々の計測値の規格値）から決める。下限より小さい差は below、上限より大きい差は above で、規格値ちょうどは内側に入れる。内側は 0 から規格値の半分までと、半分から規格値までに分け、境の値は 0 に近い段に入れる。0 は high_inner。規格値のない側は inner の 1 段だけにする
public struct DeviationBand: Message {
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

    /// 規則の id。評価しなかった点の段は持たない
    public var rule: String? {
        get { readField("rule") }
        set { writeField("rule", newValue) }
    }

    /// 段の名前
    public var band: String? {
        get { readField("band") }
        set { writeField("band", newValue) }
    }

    /// 段の下の端の差（mm）。下に端がなければ持たない
    public var lower: Double? {
        get { readField("lower") }
        set { writeField("lower", newValue) }
    }

    /// 段の上の端の差（mm）。上に端がなければ持たない
    public var upper: Double? {
        get { readField("upper") }
        set { writeField("upper", newValue) }
    }

    /// シーンの点の色。16 ビットの赤・緑・青・不透明度
    public var color: [Int64]? {
        get { readField("color") }
        set { writeField("color", newValue) }
    }
}

/// 差の色の段。not_evaluated は TIN の外・どの規則の勾配にも入らない・区分の境の近くで除いた点
///
/// 列挙の値。知らない値も受け取る
public enum DeviationBandKind {
    public static let below = "below"
    public static let lowOuter = "low_outer"
    public static let lowInner = "low_inner"
    public static let highInner = "high_inner"
    public static let highOuter = "high_outer"
    public static let above = "above"
    public static let notEvaluated = "not_evaluated"
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

    /// カタログの形のメジャーバージョン。今は 1
    public var v: Int64? {
        get { readField("v") }
        set { writeField("v", newValue) }
    }

    /// 言語（ja・en）
    public var lang: String? {
        get { readField("lang") }
        set { writeField("lang", newValue) }
    }

    /// パッケージのバージョン。コアと同じバージョンで出す
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
    public static let calibrations = "calibrations"
    public static let measureDistance = "measure_distance"
    public static let measure = "measure"
    public static let intersect = "intersect"
    public static let regions = "regions"
    public static let construct = "construct"
    public static let apply = "apply"
    public static let create = "create"
    public static let convert = "convert"
    public static let convertEstimate = "convert_estimate"
    public static let tables = "tables"
    public static let scene = "scene"
    public static let scene3d = "scene3d"
    public static let check = "check"
    public static let validate = "validate"
    public static let reviewCarry = "review_carry"
    public static let reviewAdopt = "review_adopt"
    public static let reviewScene = "review_scene"
    public static let building = "building"
    public static let buildingElements = "building_elements"
    public static let buildingDrawing = "building_drawing"
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
    public static let deliveryApply = "delivery_apply"
    public static let photo = "photo"
    public static let boring = "boring"
    public static let buildingApply = "building_apply"
    public static let lift = "lift"
    public static let render = "render"
    public static let raster = "raster"
    public static let georeference = "georeference"
}
