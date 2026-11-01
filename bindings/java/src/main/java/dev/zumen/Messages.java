// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。

package dev.zumen;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 境界のメッセージの型（決定 0012・0224）。スキーマの定義ごとに 1 つの型がある。
 *
 * <p>型は、JSON のオブジェクト（{@link Map}）を包む見方で、知らない欄もそのまま持つ。列挙は文字列で読み、
 * 知らない値も受け取る（決定 0015）。欄は {@code getXxx} で読み（ない欄と形の違う値は null）、{@code withXxx} で書く。
 */
public final class Messages {
    private Messages() {}

    /** メッセージの型が継ぐ、JSON のオブジェクトの見方 */
    public abstract static class Message {
        /** 包んでいる JSON のオブジェクト */
        protected final Map<String, Object> json;

        protected Message(Map<String, Object> json) {
            if (json == null) {
                throw new NullPointerException("json");
            }
            this.json = json;
        }

        /** 包んでいる JSON のオブジェクト。書き換えると、この見方からも見える */
        public Map<String, Object> json() {
            return json;
        }

        /** 欄を書く。null なら欄を除く。メッセージの型と、その列・表は JSON の値に直して持つ */
        protected void put(String key, Object value) {
            if (value == null) {
                json.remove(key);
            } else {
                json.put(key, Json.unwrap(value));
            }
        }

        /** JSON の文字列にする */
        @Override
        public String toString() {
            return Json.write(json);
        }
    }

    /** 要求。op の値で形が決まる */
    public static final class Request extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Request(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Request of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Request(json);
        }

        /** op が capabilities の形なら、その見方。違えば null */
        public CapabilitiesRequest asCapabilitiesRequest() {
            return "capabilities".equals(json.get("op")) ? new CapabilitiesRequest(json) : null;
        }

        /** op が open の形なら、その見方。違えば null */
        public OpenRequest asOpenRequest() {
            return "open".equals(json.get("op")) ? new OpenRequest(json) : null;
        }

        /** op が close の形なら、その見方。違えば null */
        public CloseRequest asCloseRequest() {
            return "close".equals(json.get("op")) ? new CloseRequest(json) : null;
        }

        /** op が describe の形なら、その見方。違えば null */
        public DescribeRequest asDescribeRequest() {
            return "describe".equals(json.get("op")) ? new DescribeRequest(json) : null;
        }

        /** op が drawing の形なら、その見方。違えば null */
        public DrawingRequest asDrawingRequest() {
            return "drawing".equals(json.get("op")) ? new DrawingRequest(json) : null;
        }

        /** op が entities の形なら、その見方。違えば null */
        public EntitiesRequest asEntitiesRequest() {
            return "entities".equals(json.get("op")) ? new EntitiesRequest(json) : null;
        }

        /** op が apply の形なら、その見方。違えば null */
        public ApplyRequest asApplyRequest() {
            return "apply".equals(json.get("op")) ? new ApplyRequest(json) : null;
        }

        /** op が convert の形なら、その見方。違えば null */
        public ConvertRequest asConvertRequest() {
            return "convert".equals(json.get("op")) ? new ConvertRequest(json) : null;
        }

        /** op が tables の形なら、その見方。違えば null */
        public TablesRequest asTablesRequest() {
            return "tables".equals(json.get("op")) ? new TablesRequest(json) : null;
        }

        /** op が scene の形なら、その見方。違えば null */
        public SceneRequest asSceneRequest() {
            return "scene".equals(json.get("op")) ? new SceneRequest(json) : null;
        }

        /** op が check の形なら、その見方。違えば null */
        public CheckRequest asCheckRequest() {
            return "check".equals(json.get("op")) ? new CheckRequest(json) : null;
        }

        /** op が validate の形なら、その見方。違えば null */
        public ValidateRequest asValidateRequest() {
            return "validate".equals(json.get("op")) ? new ValidateRequest(json) : null;
        }

        /** op が review_carry の形なら、その見方。違えば null */
        public ReviewCarryRequest asReviewCarryRequest() {
            return "review_carry".equals(json.get("op")) ? new ReviewCarryRequest(json) : null;
        }

        /** op が review_adopt の形なら、その見方。違えば null */
        public ReviewAdoptRequest asReviewAdoptRequest() {
            return "review_adopt".equals(json.get("op")) ? new ReviewAdoptRequest(json) : null;
        }

        /** op が building の形なら、その見方。違えば null */
        public BuildingRequest asBuildingRequest() {
            return "building".equals(json.get("op")) ? new BuildingRequest(json) : null;
        }

        /** op が building_elements の形なら、その見方。違えば null */
        public BuildingElementsRequest asBuildingElementsRequest() {
            return "building_elements".equals(json.get("op")) ? new BuildingElementsRequest(json) : null;
        }

        /** op が ids_check の形なら、その見方。違えば null */
        public IdsCheckRequest asIdsCheckRequest() {
            return "ids_check".equals(json.get("op")) ? new IdsCheckRequest(json) : null;
        }

        /** op が review_adopt_ids の形なら、その見方。違えば null */
        public ReviewAdoptIdsRequest asReviewAdoptIdsRequest() {
            return "review_adopt_ids".equals(json.get("op")) ? new ReviewAdoptIdsRequest(json) : null;
        }

        /** op が bcf_read の形なら、その見方。違えば null */
        public BcfReadRequest asBcfReadRequest() {
            return "bcf_read".equals(json.get("op")) ? new BcfReadRequest(json) : null;
        }

        /** op が bcf_write の形なら、その見方。違えば null */
        public BcfWriteRequest asBcfWriteRequest() {
            return "bcf_write".equals(json.get("op")) ? new BcfWriteRequest(json) : null;
        }

        /** op が civil の形なら、その見方。違えば null */
        public CivilRequest asCivilRequest() {
            return "civil".equals(json.get("op")) ? new CivilRequest(json) : null;
        }

        /** op が pointcloud の形なら、その見方。違えば null */
        public PointcloudRequest asPointcloudRequest() {
            return "pointcloud".equals(json.get("op")) ? new PointcloudRequest(json) : null;
        }

        /** op が deviation の形なら、その見方。違えば null */
        public DeviationRequest asDeviationRequest() {
            return "deviation".equals(json.get("op")) ? new DeviationRequest(json) : null;
        }

        /** op が civil_drawing の形なら、その見方。違えば null */
        public CivilDrawingRequest asCivilDrawingRequest() {
            return "civil_drawing".equals(json.get("op")) ? new CivilDrawingRequest(json) : null;
        }

        /** op が geo の形なら、その見方。違えば null */
        public GeoRequest asGeoRequest() {
            return "geo".equals(json.get("op")) ? new GeoRequest(json) : null;
        }

        /** op が delivery_check の形なら、その見方。違えば null */
        public DeliveryCheckRequest asDeliveryCheckRequest() {
            return "delivery_check".equals(json.get("op")) ? new DeliveryCheckRequest(json) : null;
        }

        /** op が photo の形なら、その見方。違えば null */
        public PhotoRequest asPhotoRequest() {
            return "photo".equals(json.get("op")) ? new PhotoRequest(json) : null;
        }

        /** op が sheet の形なら、その見方。違えば null */
        public SheetRequest asSheetRequest() {
            return "sheet".equals(json.get("op")) ? new SheetRequest(json) : null;
        }

        /** op が boring の形なら、その見方。違えば null */
        public BoringRequest asBoringRequest() {
            return "boring".equals(json.get("op")) ? new BoringRequest(json) : null;
        }
    }

    /** どの op の要求にも置ける欄 */
    public static final class RequestCommon extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public RequestCommon(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static RequestCommon of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new RequestCommon(json);
        }

        /** 空のオブジェクトから作る */
        public RequestCommon() {
            super(new LinkedHashMap<String, Object>());
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public RequestCommon withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public RequestCommon withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public RequestCommon withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public RequestCommon withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public RequestCommon withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }
    }

    /** 予算の単位ごとの上限（決定 0010） */
    public static final class Budget extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Budget(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Budget of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Budget(json);
        }

        /** 空のオブジェクトから作る */
        public Budget() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 受け取ったバイト数 */
        public Object getInputBytes() {
            return Json.read(json.get("input_bytes"), Json::value);
        }

        /** {@link #getInputBytes()} を書く。null なら欄を除く */
        public Budget withInputBytes(Object value) {
            put("input_bytes", value);
            return this;
        }

        /** 展開・復号で作ったバイト数。入れ子の分も足す */
        public Object getDecodedBytes() {
            return Json.read(json.get("decoded_bytes"), Json::value);
        }

        /** {@link #getDecodedBytes()} を書く。null なら欄を除く */
        public Budget withDecodedBytes(Object value) {
            put("decoded_bytes", value);
            return this;
        }

        /** L0・L1 で読んだレコードの数 */
        public Object getRecords() {
            return Json.read(json.get("records"), Json::value);
        }

        /** {@link #getRecords()} を書く。null なら欄を除く */
        public Budget withRecords(Object value) {
            put("records", value);
            return this;
        }

        /** L2・L3 で作った要素の数 */
        public Object getElements() {
            return Json.read(json.get("elements"), Json::value);
        }

        /** {@link #getElements()} を書く。null なら欄を除く */
        public Budget withElements(Object value) {
            put("elements", value);
            return this;
        }

        /** 入れ子と再帰の深さ */
        public Object getDepth() {
            return Json.read(json.get("depth"), Json::value);
        }

        /** {@link #getDepth()} を書く。null なら欄を除く */
        public Budget withDepth(Object value) {
            put("depth", value);
            return this;
        }

        /** 幾何の分割で作った点・線・面の数 */
        public Object getSegments() {
            return Json.read(json.get("segments"), Json::value);
        }

        /** {@link #getSegments()} を書く。null なら欄を除く */
        public Budget withSegments(Object value) {
            put("segments", value);
            return this;
        }

        /** ほかの単位に入らない繰り返しの回数 */
        public Object getWork() {
            return Json.read(json.get("work"), Json::value);
        }

        /** {@link #getWork()} を書く。null なら欄を除く */
        public Budget withWork(Object value) {
            put("work", value);
            return this;
        }
    }

    /** コアの版と、使える op・形式を尋ねる */
    public static final class CapabilitiesRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CapabilitiesRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CapabilitiesRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CapabilitiesRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public CapabilitiesRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "capabilities");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public CapabilitiesRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public CapabilitiesRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public CapabilitiesRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public CapabilitiesRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public CapabilitiesRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public CapabilitiesRequest withOp(String value) {
            put("op", value);
            return this;
        }
    }

    /** コアの版と、使える op・形式（決定 0015） */
    public static final class CapabilitiesResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CapabilitiesResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CapabilitiesResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CapabilitiesResult(json);
        }

        /** 空のオブジェクトから作る */
        public CapabilitiesResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** パッケージの版（semver） */
        public String getPackage() {
            return Json.read(json.get("package"), Json::string);
        }

        /** {@link #getPackage()} を書く。null なら欄を除く */
        public CapabilitiesResult withPackage(String value) {
            put("package", value);
            return this;
        }

        /** ABI の大版。zm_abi_version と同じ */
        public Long getAbi() {
            return Json.read(json.get("abi"), Json::integer);
        }

        /** {@link #getAbi()} を書く。null なら欄を除く */
        public CapabilitiesResult withAbi(Long value) {
            put("abi", value);
            return this;
        }

        /** メッセージのスキーマの版 */
        public SchemaVersion getSchema() {
            return Json.read(json.get("schema"), SchemaVersion::of);
        }

        /** {@link #getSchema()} を書く。null なら欄を除く */
        public CapabilitiesResult withSchema(SchemaVersion value) {
            put("schema", value);
            return this;
        }

        /** このコア（wasm ではこのモジュール）で使える op の名前 */
        public List<String> getOps() {
            return Json.read(json.get("ops"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getOps()} を書く。null なら欄を除く */
        public CapabilitiesResult withOps(List<String> value) {
            put("ops", value);
            return this;
        }

        /** 読み書きできる形式 */
        public List<Format> getFormats() {
            return Json.read(json.get("formats"), v0 -> Json.list(v0, Format::of));
        }

        /** {@link #getFormats()} を書く。null なら欄を除く */
        public CapabilitiesResult withFormats(List<Format> value) {
            put("formats", value);
            return this;
        }
    }

    /** メッセージのスキーマの版。大版は要求と応答の v、小版は op・欄・列挙の値を足すたびに上げる */
    public static final class SchemaVersion extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SchemaVersion(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SchemaVersion of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SchemaVersion(json);
        }

        /** 空のオブジェクトから作る */
        public SchemaVersion() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 大版 */
        public Long getMajor() {
            return Json.read(json.get("major"), Json::integer);
        }

        /** {@link #getMajor()} を書く。null なら欄を除く */
        public SchemaVersion withMajor(Long value) {
            put("major", value);
            return this;
        }

        /** 小版 */
        public Long getMinor() {
            return Json.read(json.get("minor"), Json::integer);
        }

        /** {@link #getMinor()} を書く。null なら欄を除く */
        public SchemaVersion withMinor(Long value) {
            put("minor", value);
            return this;
        }
    }

    /** 読み書きできる形式 */
    public static final class Format extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Format(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Format of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Format(json);
        }

        /** 空のオブジェクトから作る */
        public Format() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 形式の名前（dxf・sfc など） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public Format withId(String value) {
            put("id", value);
            return this;
        }

        /** 形式の系統（autocad・sxf など、決定 0008） */
        public String getFamily() {
            return Json.read(json.get("family"), Json::string);
        }

        /** {@link #getFamily()} を書く。null なら欄を除く */
        public Format withFamily(String value) {
            put("family", value);
            return this;
        }

        /** 読めるか */
        public Boolean getRead() {
            return Json.read(json.get("read"), Json::bool);
        }

        /** {@link #getRead()} を書く。null なら欄を除く */
        public Format withRead(Boolean value) {
            put("read", value);
            return this;
        }

        /** 書けるか */
        public Boolean getWrite() {
            return Json.read(json.get("write"), Json::bool);
        }

        /** {@link #getWrite()} を書く。null なら欄を除く */
        public Format withWrite(Boolean value) {
            put("write", value);
            return this;
        }

        /** この形式の ID の規則の版（決定 0015） */
        public Long getIdRules() {
            return Json.read(json.get("id_rules"), Json::integer);
        }

        /** {@link #getIdRules()} を書く。null なら欄を除く */
        public Format withIdRules(Long value) {
            put("id_rules", value);
            return this;
        }
    }

    /** 入力を読んで、文書を作る。形式は入力から判定する */
    public static final class OpenRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public OpenRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static OpenRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new OpenRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public OpenRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "open");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public OpenRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public OpenRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public OpenRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public OpenRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public OpenRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public OpenRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 読む入力の名前。zm_push で渡したもの */
        public String getInput() {
            return Json.read(json.get("input"), Json::string);
        }

        /** {@link #getInput()} を書く。null なら欄を除く */
        public OpenRequest withInput(String value) {
            put("input", value);
            return this;
        }

        /** 作る文書の名前。書かなければ入力の名前 */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public OpenRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。小版 48 から */
        public Long getSize() {
            return Json.read(json.get("size"), Json::integer);
        }

        /** {@link #getSize()} を書く。null なら欄を除く */
        public OpenRequest withSize(Long value) {
            put("size", value);
            return this;
        }

        /** 見分ける印のない形式（xyz）を名指して読む。書かなければ、入力から判定する（決定 0232）。小版 49 から */
        public String getFormat() {
            return Json.read(json.get("format"), Json::string);
        }

        /** {@link #getFormat()} を書く。null なら欄を除く */
        public OpenRequest withFormat(String value) {
            put("format", value);
            return this;
        }
    }

    /** 作った文書 */
    public static final class OpenResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public OpenResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static OpenResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new OpenResult(json);
        }

        /** 空のオブジェクトから作る */
        public OpenResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 文書の名前 */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public OpenResult withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 判定した形式の名前 */
        public String getFormat() {
            return Json.read(json.get("format"), Json::string);
        }

        /** {@link #getFormat()} を書く。null なら欄を除く */
        public OpenResult withFormat(String value) {
            put("format", value);
            return this;
        }

        /** 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。小版 48 から */
        public List<InputRange> getNeeds() {
            return Json.read(json.get("needs"), v0 -> Json.list(v0, InputRange::of));
        }

        /** {@link #getNeeds()} を書く。null なら欄を除く */
        public OpenResult withNeeds(List<InputRange> value) {
            put("needs", value);
            return this;
        }

        /** 読み終えたので、close の inputs で捨ててよい範囲の入力。小版 48 から */
        public List<InputRange> getDone() {
            return Json.read(json.get("done"), v0 -> Json.list(v0, InputRange::of));
        }

        /** {@link #getDone()} を書く。null なら欄を除く */
        public OpenResult withDone(List<InputRange> value) {
            put("done", value);
            return this;
        }

        /** SXF を ZIP に入れた書庫（P2Z・SFZ）なら、SXF の項目のほかの項目（属性ファイル・画像など）の書庫の中の名前。書庫の順。apply は、SXF の項目だけを置き換え、ほかの項目を写した書庫を書く（#385）。小版 54 から */
        public List<String> getEntries() {
            return Json.read(json.get("entries"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getEntries()} を書く。null なら欄を除く */
        public OpenResult withEntries(List<String> value) {
            put("entries", value);
            return this;
        }
    }

    /** 入力の範囲と、その範囲を渡す入力の名前 */
    public static final class InputRange extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public InputRange(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static InputRange of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new InputRange(json);
        }

        /** 空のオブジェクトから作る */
        public InputRange() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 範囲を渡す入力の名前。<入力の名前>#<start>-<end> */
        public String getInput() {
            return Json.read(json.get("input"), Json::string);
        }

        /** {@link #getInput()} を書く。null なら欄を除く */
        public InputRange withInput(String value) {
            put("input", value);
            return this;
        }

        /** 範囲の初め（バイト） */
        public Long getStart() {
            return Json.read(json.get("start"), Json::integer);
        }

        /** {@link #getStart()} を書く。null なら欄を除く */
        public InputRange withStart(Long value) {
            put("start", value);
            return this;
        }

        /** 範囲の終わり（バイト、含まない） */
        public Long getEnd() {
            return Json.read(json.get("end"), Json::integer);
        }

        /** {@link #getEnd()} を書く。null なら欄を除く */
        public InputRange withEnd(Long value) {
            put("end", value);
            return this;
        }
    }

    /** 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない */
    public static final class CloseRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CloseRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CloseRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CloseRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public CloseRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "close");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public CloseRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public CloseRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public CloseRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public CloseRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public CloseRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public CloseRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 捨てる文書の名前 */
        public List<String> getDocs() {
            return Json.read(json.get("docs"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getDocs()} を書く。null なら欄を除く */
        public CloseRequest withDocs(List<String> value) {
            put("docs", value);
            return this;
        }

        /** 捨てる入力の名前 */
        public List<String> getInputs() {
            return Json.read(json.get("inputs"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getInputs()} を書く。null なら欄を除く */
        public CloseRequest withInputs(List<String> value) {
            put("inputs", value);
            return this;
        }
    }

    /** 捨てたこと。中身はない */
    public static final class CloseResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CloseResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CloseResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CloseResult(json);
        }

        /** 空のオブジェクトから作る */
        public CloseResult() {
            super(new LinkedHashMap<String, Object>());
        }
    }

    /** 診断の文を、カタログから組み立てる（決定 0009） */
    public static final class DescribeRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DescribeRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DescribeRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DescribeRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public DescribeRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "describe");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public DescribeRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public DescribeRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public DescribeRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public DescribeRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public DescribeRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public DescribeRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** カタログの入力の名前。zm_push で渡したもの。書かなければ、code と値をそのまま文にする */
        public String getCatalog() {
            return Json.read(json.get("catalog"), Json::string);
        }

        /** {@link #getCatalog()} を書く。null なら欄を除く */
        public DescribeRequest withCatalog(String value) {
            put("catalog", value);
            return this;
        }

        /** 文にする診断。応答で受け取ったもの */
        public List<Diagnostic> getDiagnostics() {
            return Json.read(json.get("diagnostics"), v0 -> Json.list(v0, Diagnostic::of));
        }

        /** {@link #getDiagnostics()} を書く。null なら欄を除く */
        public DescribeRequest withDiagnostics(List<Diagnostic> value) {
            put("diagnostics", value);
            return this;
        }
    }

    /** 診断の文 */
    public static final class DescribeResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DescribeResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DescribeResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DescribeResult(json);
        }

        /** 空のオブジェクトから作る */
        public DescribeResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 診断ごとの文。要求の diagnostics と同じ順。カタログにない code は、code と値をそのまま書く */
        public List<String> getTexts() {
            return Json.read(json.get("texts"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getTexts()} を書く。null なら欄を除く */
        public DescribeResult withTexts(List<String> value) {
            put("texts", value);
            return this;
        }
    }

    /** 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合試験で比べるためにも使う（決定 0007） */
    public static final class DrawingRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DrawingRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DrawingRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DrawingRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public DrawingRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "drawing");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public DrawingRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public DrawingRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public DrawingRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public DrawingRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public DrawingRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public DrawingRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で作ったもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public DrawingRequest withDoc(String value) {
            put("doc", value);
            return this;
        }
    }

    /** 図面モデルの要約。移さなかった図形と値は、診断で返す */
    public static final class DrawingResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DrawingResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DrawingResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DrawingResult(json);
        }

        /** 空のオブジェクトから作る */
        public DrawingResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 文書のハッシュ。open に渡したバイト列から作る。変更の一覧の前提条件に使う（v2: と 32 桁の 16 進、決定 0156） */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public DrawingResult withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 実寸の空間。モデルの順 */
        public List<SpaceSummary> getSpaces() {
            return Json.read(json.get("spaces"), v0 -> Json.list(v0, SpaceSummary::of));
        }

        /** {@link #getSpaces()} を書く。null なら欄を除く */
        public DrawingResult withSpaces(List<SpaceSummary> value) {
            put("spaces", value);
            return this;
        }

        /** 用紙。モデルの順 */
        public List<SheetSummary> getSheets() {
            return Json.read(json.get("sheets"), v0 -> Json.list(v0, SheetSummary::of));
        }

        /** {@link #getSheets()} を書く。null なら欄を除く */
        public DrawingResult withSheets(List<SheetSummary> value) {
            put("sheets", value);
            return this;
        }

        /** 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。小版 39 から */
        public PointStyleSummary getPointStyle() {
            return Json.read(json.get("point_style"), PointStyleSummary::of);
        }

        /** {@link #getPointStyle()} を書く。null なら欄を除く */
        public DrawingResult withPointStyle(PointStyleSummary value) {
            put("point_style", value);
            return this;
        }
    }

    /** 文書の点の表し方。欄の意味は point_style_edit と同じ */
    public static final class PointStyleSummary extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PointStyleSummary(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PointStyleSummary of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PointStyleSummary(json);
        }

        /** 空のオブジェクトから作る */
        public PointStyleSummary() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 基本の形 */
        public String getMark() {
            return Json.read(json.get("mark"), Json::string);
        }

        /** {@link #getMark()} を書く。null なら欄を除く */
        public PointStyleSummary withMark(String value) {
            put("mark", value);
            return this;
        }

        /** 円で囲む */
        public Boolean getCircle() {
            return Json.read(json.get("circle"), Json::bool);
        }

        /** {@link #getCircle()} を書く。null なら欄を除く */
        public PointStyleSummary withCircle(Boolean value) {
            put("circle", value);
            return this;
        }

        /** 四角で囲む */
        public Boolean getSquare() {
            return Json.read(json.get("square"), Json::bool);
        }

        /** {@link #getSquare()} を書く。null なら欄を除く */
        public PointStyleSummary withSquare(Boolean value) {
            put("square", value);
            return this;
        }

        /** 大きさ。意味は size_kind で決まる */
        public Double getSize() {
            return Json.read(json.get("size"), Json::number);
        }

        /** {@link #getSize()} を書く。null なら欄を除く */
        public PointStyleSummary withSize(Double value) {
            put("size", value);
            return this;
        }

        /** 大きさの意味 */
        public String getSizeKind() {
            return Json.read(json.get("size_kind"), Json::string);
        }

        /** {@link #getSizeKind()} を書く。null なら欄を除く */
        public PointStyleSummary withSizeKind(String value) {
            put("size_kind", value);
            return this;
        }
    }

    /** 実寸の空間の要約 */
    public static final class SpaceSummary extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SpaceSummary(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SpaceSummary of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SpaceSummary(json);
        }

        /** 空のオブジェクトから作る */
        public SpaceSummary() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 空間の ID（space/0）。entities の要素の ID の始まりと同じ（決定 0156） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public SpaceSummary withId(String value) {
            put("id", value);
            return this;
        }

        /** 文書の単位 */
        public String getUnit() {
            return Json.read(json.get("unit"), Json::string);
        }

        /** {@link #getUnit()} を書く。null なら欄を除く */
        public SpaceSummary withUnit(String value) {
            put("unit", value);
            return this;
        }

        /** 要素の数 */
        public ElementCounts getElements() {
            return Json.read(json.get("elements"), ElementCounts::of);
        }

        /** {@link #getElements()} を書く。null なら欄を除く */
        public SpaceSummary withElements(ElementCounts value) {
            put("elements", value);
            return this;
        }

        /** 線分の端点と文字の基点を囲む範囲。文書の単位。要素がなければ持たない */
        public Extent getExtent() {
            return Json.read(json.get("extent"), Extent::of);
        }

        /** {@link #getExtent()} を書く。null なら欄を除く */
        public SpaceSummary withExtent(Extent value) {
            put("extent", value);
            return this;
        }
    }

    /** 用紙の要約 */
    public static final class SheetSummary extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SheetSummary(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SheetSummary of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SheetSummary(json);
        }

        /** 空のオブジェクトから作る */
        public SheetSummary() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 用紙の ID。LAYOUT がハンドルを持てば sheet/h22、なければ読んだ順の番号（sheet/0）。entities の要素の ID の始まりと同じ（決定 0156） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public SheetSummary withId(String value) {
            put("id", value);
            return this;
        }

        /** 元の形式の用紙の名前（DXF・DWG のレイアウトの名前）。翻訳しない。なければ持たない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public SheetSummary withName(String value) {
            put("name", value);
            return this;
        }

        /** 用紙の幅と高さ（mm）。分からなければ持たない */
        public List<Double> getSize() {
            return Json.read(json.get("size"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getSize()} を書く。null なら欄を除く */
        public SheetSummary withSize(List<Double> value) {
            put("size", value);
            return this;
        }

        /** 要素の数 */
        public ElementCounts getElements() {
            return Json.read(json.get("elements"), ElementCounts::of);
        }

        /** {@link #getElements()} を書く。null なら欄を除く */
        public SheetSummary withElements(ElementCounts value) {
            put("elements", value);
            return this;
        }

        /** ビューの数 */
        public Long getViews() {
            return Json.read(json.get("views"), Json::integer);
        }

        /** {@link #getViews()} を書く。null なら欄を除く */
        public SheetSummary withViews(Long value) {
            put("views", value);
            return this;
        }

        /** 線分の端点と文字の基点を囲む範囲。用紙の mm。要素がなければ持たない */
        public Extent getExtent() {
            return Json.read(json.get("extent"), Extent::of);
        }

        /** {@link #getExtent()} を書く。null なら欄を除く */
        public SheetSummary withExtent(Extent value) {
            put("extent", value);
            return this;
        }
    }

    /**
     * 実寸の単位
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class Unit {
        private Unit() {}

        public static final String MILLIMETER = "millimeter";

        public static final String CENTIMETER = "centimeter";

        public static final String METER = "meter";

        public static final String INCH = "inch";

        public static final String FOOT = "foot";

        public static final String UNKNOWN = "unknown";
    }

    /** 種類ごとの要素の数 */
    public static final class ElementCounts extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ElementCounts(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ElementCounts of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ElementCounts(json);
        }

        /** 空のオブジェクトから作る */
        public ElementCounts() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 線分 */
        public Long getLines() {
            return Json.read(json.get("lines"), Json::integer);
        }

        /** {@link #getLines()} を書く。null なら欄を除く */
        public ElementCounts withLines(Long value) {
            put("lines", value);
            return this;
        }

        /** 文字 */
        public Long getTexts() {
            return Json.read(json.get("texts"), Json::integer);
        }

        /** {@link #getTexts()} を書く。null なら欄を除く */
        public ElementCounts withTexts(Long value) {
            put("texts", value);
            return this;
        }
    }

    /** 点を囲む、軸に沿った長方形 */
    public static final class Extent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Extent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Extent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Extent(json);
        }

        /** 空のオブジェクトから作る */
        public Extent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** X の最小 */
        public Double getMinX() {
            return Json.read(json.get("min_x"), Json::number);
        }

        /** {@link #getMinX()} を書く。null なら欄を除く */
        public Extent withMinX(Double value) {
            put("min_x", value);
            return this;
        }

        /** Y の最小 */
        public Double getMinY() {
            return Json.read(json.get("min_y"), Json::number);
        }

        /** {@link #getMinY()} を書く。null なら欄を除く */
        public Extent withMinY(Double value) {
            put("min_y", value);
            return this;
        }

        /** X の最大 */
        public Double getMaxX() {
            return Json.read(json.get("max_x"), Json::number);
        }

        /** {@link #getMaxX()} を書く。null なら欄を除く */
        public Extent withMaxX(Double value) {
            put("max_x", value);
            return this;
        }

        /** Y の最大 */
        public Double getMaxY() {
            return Json.read(json.get("max_y"), Json::number);
        }

        /** {@link #getMaxY()} を書く。null なら欄を除く */
        public Extent withMaxY(Double value) {
            put("max_y", value);
            return this;
        }
    }

    /** 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117） */
    public static final class EntitiesRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public EntitiesRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static EntitiesRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new EntitiesRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public EntitiesRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "entities");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public EntitiesRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public EntitiesRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public EntitiesRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public EntitiesRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public EntitiesRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public EntitiesRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で作ったもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public EntitiesRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0 */
        public Long getOffset() {
            return Json.read(json.get("offset"), Json::integer);
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public EntitiesRequest withOffset(Long value) {
            put("offset", value);
            return this;
        }

        /** 返す要素の数の上限。書かなければ 1000 */
        public Long getLimit() {
            return Json.read(json.get("limit"), Json::integer);
        }

        /** {@link #getLimit()} を書く。null なら欄を除く */
        public EntitiesRequest withLimit(Long value) {
            put("limit", value);
            return this;
        }

        /** 返す欄の名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118） */
        public List<String> getFields() {
            return Json.read(json.get("fields"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getFields()} を書く。null なら欄を除く */
        public EntitiesRequest withFields(List<String> value) {
            put("fields", value);
            return this;
        }

        /** [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118） */
        public List<Double> getRegion() {
            return Json.read(json.get("region"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getRegion()} を書く。null なら欄を除く */
        public EntitiesRequest withRegion(List<Double> value) {
            put("region", value);
            return this;
        }

        /** 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。小版 31 から（決定 0177） */
        public String getPlace() {
            return Json.read(json.get("place"), Json::string);
        }

        /** {@link #getPlace()} を書く。null なら欄を除く */
        public EntitiesRequest withPlace(String value) {
            put("place", value);
            return this;
        }
    }

    /** 要素の一覧の一部。移さなかった図形と値は、診断で返す */
    public static final class EntitiesResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public EntitiesResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static EntitiesResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new EntitiesResult(json);
        }

        /** 空のオブジェクトから作る */
        public EntitiesResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** offset から limit 個までの要素 */
        public List<Entity> getItems() {
            return Json.read(json.get("items"), v0 -> Json.list(v0, Entity::of));
        }

        /** {@link #getItems()} を書く。null なら欄を除く */
        public EntitiesResult withItems(List<Entity> value) {
            put("items", value);
            return this;
        }

        /** 全体の要素の数 */
        public Long getTotal() {
            return Json.read(json.get("total"), Json::integer);
        }

        /** {@link #getTotal()} を書く。null なら欄を除く */
        public EntitiesResult withTotal(Long value) {
            put("total", value);
            return this;
        }

        /** 続きの最初の要素の位置。続きがなければ持たない */
        public Long getNextOffset() {
            return Json.read(json.get("next_offset"), Json::integer);
        }

        /** {@link #getNextOffset()} を書く。null なら欄を除く */
        public EntitiesResult withNextOffset(Long value) {
            put("next_offset", value);
            return this;
        }
    }

    /** 図面モデルの要素 */
    public static final class Entity extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Entity(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Entity of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Entity(json);
        }

        /** 空のオブジェクトから作る */
        public Entity() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つ鍵（ハンドル）があれば鍵で作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どの版でも同じ ID を返す（決定 0117・0156） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public Entity withId(String value) {
            put("id", value);
            return this;
        }

        /** 形の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public Entity withKind(String value) {
            put("kind", value);
            return this;
        }

        /** レイヤの元の名前。翻訳しない。図面モデルにレイヤがなければ持たない */
        public String getLayer() {
            return Json.read(json.get("layer"), Json::string);
        }

        /** {@link #getLayer()} を書く。null なら欄を除く */
        public Entity withLayer(String value) {
            put("layer", value);
            return this;
        }

        /** 元の形式のレコード */
        public Source getSource() {
            return Json.read(json.get("source"), Source::of);
        }

        /** {@link #getSource()} を書く。null なら欄を除く */
        public Entity withSource(Source value) {
            put("source", value);
            return this;
        }

        /** 形の定義から求めた範囲。文字の字形・線の太さ・記号の中身は含めない。囲む点がなければ持たない（決定 0118） */
        public Extent getBbox() {
            return Json.read(json.get("bbox"), Extent::of);
        }

        /** {@link #getBbox()} を書く。null なら欄を除く */
        public Entity withBbox(Extent value) {
            put("bbox", value);
            return this;
        }

        /** 要素のレイヤ・スタイル・形のハッシュ。出所の位置は含めない。変更の前提条件に使う（v2: と 32 桁の 16 進、決定 0156） */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public Entity withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点か当てはめる点で決まるスプライン・寸法・引出線（LEADER）だけが持つ。apply の replace と add に渡せる形（決定 0139）。小版 10 から */
        public Geometry getGeometry() {
            return Json.read(json.get("geometry"), Geometry::of);
        }

        /** {@link #getGeometry()} を書く。null なら欄を除く */
        public Entity withGeometry(Geometry value) {
            put("geometry", value);
            return this;
        }
    }

    /**
     * entities で返す要素の欄（決定 0118）
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class EntityField {
        private EntityField() {}

        public static final String ID = "id";

        public static final String KIND = "kind";

        public static final String LAYER = "layer";

        public static final String SOURCE = "source";

        public static final String BBOX = "bbox";

        public static final String HASH = "hash";

        public static final String GEOMETRY = "geometry";
    }

    /** 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる */
    public static final class Geometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Geometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Geometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Geometry(json);
        }

        /** shape が line の形なら、その見方。違えば null */
        public LineGeometry asLineGeometry() {
            return "line".equals(json.get("shape")) ? new LineGeometry(json) : null;
        }

        /** shape が circle の形なら、その見方。違えば null */
        public CircleGeometry asCircleGeometry() {
            return "circle".equals(json.get("shape")) ? new CircleGeometry(json) : null;
        }

        /** shape が arc の形なら、その見方。違えば null */
        public ArcGeometry asArcGeometry() {
            return "arc".equals(json.get("shape")) ? new ArcGeometry(json) : null;
        }

        /** shape が ellipse の形なら、その見方。違えば null */
        public EllipseGeometry asEllipseGeometry() {
            return "ellipse".equals(json.get("shape")) ? new EllipseGeometry(json) : null;
        }

        /** shape が polyline の形なら、その見方。違えば null */
        public PolylineGeometry asPolylineGeometry() {
            return "polyline".equals(json.get("shape")) ? new PolylineGeometry(json) : null;
        }

        /** shape が text の形なら、その見方。違えば null */
        public TextGeometry asTextGeometry() {
            return "text".equals(json.get("shape")) ? new TextGeometry(json) : null;
        }

        /** shape が formatted_text の形なら、その見方。違えば null */
        public FormattedTextGeometry asFormattedTextGeometry() {
            return "formatted_text".equals(json.get("shape")) ? new FormattedTextGeometry(json) : null;
        }

        /** shape が symbol の形なら、その見方。違えば null */
        public SymbolGeometry asSymbolGeometry() {
            return "symbol".equals(json.get("shape")) ? new SymbolGeometry(json) : null;
        }

        /** shape が spline の形なら、その見方。違えば null */
        public SplineGeometry asSplineGeometry() {
            return "spline".equals(json.get("shape")) ? new SplineGeometry(json) : null;
        }

        /** shape が dimension の形なら、その見方。違えば null */
        public DimensionGeometry asDimensionGeometry() {
            return "dimension".equals(json.get("shape")) ? new DimensionGeometry(json) : null;
        }

        /** shape が leader の形なら、その見方。違えば null */
        public LeaderGeometry asLeaderGeometry() {
            return "leader".equals(json.get("shape")) ? new LeaderGeometry(json) : null;
        }

        /** shape が point の形なら、その見方。違えば null */
        public PointGeometry asPointGeometry() {
            return "point".equals(json.get("shape")) ? new PointGeometry(json) : null;
        }

        /** shape が callout の形なら、その見方。違えば null */
        public CalloutGeometry asCalloutGeometry() {
            return "callout".equals(json.get("shape")) ? new CalloutGeometry(json) : null;
        }
    }

    /** 線分 */
    public static final class LineGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LineGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LineGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LineGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public LineGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "line");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public LineGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 始点 [X, Y] */
        public List<Double> getStart() {
            return Json.read(json.get("start"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getStart()} を書く。null なら欄を除く */
        public LineGeometry withStart(List<Double> value) {
            put("start", value);
            return this;
        }

        /** 終点 [X, Y] */
        public List<Double> getEnd() {
            return Json.read(json.get("end"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getEnd()} を書く。null なら欄を除く */
        public LineGeometry withEnd(List<Double> value) {
            put("end", value);
            return this;
        }
    }

    /** 円 */
    public static final class CircleGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CircleGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CircleGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CircleGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public CircleGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "circle");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public CircleGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 中心 [X, Y] */
        public List<Double> getCenter() {
            return Json.read(json.get("center"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getCenter()} を書く。null なら欄を除く */
        public CircleGeometry withCenter(List<Double> value) {
            put("center", value);
            return this;
        }

        /** 半径。0 より大きい */
        public Double getRadius() {
            return Json.read(json.get("radius"), Json::number);
        }

        /** {@link #getRadius()} を書く。null なら欄を除く */
        public CircleGeometry withRadius(Double value) {
            put("radius", value);
            return this;
        }
    }

    /** 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる */
    public static final class ArcGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ArcGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ArcGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ArcGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public ArcGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "arc");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public ArcGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 中心 [X, Y] */
        public List<Double> getCenter() {
            return Json.read(json.get("center"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getCenter()} を書く。null なら欄を除く */
        public ArcGeometry withCenter(List<Double> value) {
            put("center", value);
            return this;
        }

        /** 半径。0 より大きい */
        public Double getRadius() {
            return Json.read(json.get("radius"), Json::number);
        }

        /** {@link #getRadius()} を書く。null なら欄を除く */
        public ArcGeometry withRadius(Double value) {
            put("radius", value);
            return this;
        }

        /** 始まりの角度（度） */
        public Double getStartAngle() {
            return Json.read(json.get("start_angle"), Json::number);
        }

        /** {@link #getStartAngle()} を書く。null なら欄を除く */
        public ArcGeometry withStartAngle(Double value) {
            put("start_angle", value);
            return this;
        }

        /** 終わりの角度（度） */
        public Double getEndAngle() {
            return Json.read(json.get("end_angle"), Json::number);
        }

        /** {@link #getEndAngle()} を書く。null なら欄を除く */
        public ArcGeometry withEndAngle(Double value) {
            put("end_angle", value);
            return this;
        }
    }

    /** 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。小版 11 から */
    public static final class EllipseGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public EllipseGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static EllipseGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new EllipseGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public EllipseGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "ellipse");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public EllipseGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 中心 [X, Y] */
        public List<Double> getCenter() {
            return Json.read(json.get("center"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getCenter()} を書く。null なら欄を除く */
        public EllipseGeometry withCenter(List<Double> value) {
            put("center", value);
            return this;
        }

        /** 長軸の端の点 [X, Y]。中心と違う点 */
        public List<Double> getMajor() {
            return Json.read(json.get("major"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getMajor()} を書く。null なら欄を除く */
        public EllipseGeometry withMajor(List<Double> value) {
            put("major", value);
            return this;
        }

        /** 短軸の長軸に対する比。0 より大きく 1 以下 */
        public Double getRatio() {
            return Json.read(json.get("ratio"), Json::number);
        }

        /** {@link #getRatio()} を書く。null なら欄を除く */
        public EllipseGeometry withRatio(Double value) {
            put("ratio", value);
            return this;
        }

        /** 始まりの角度（度）。楕円を円に引き延ばしたときの角度 */
        public Double getStartAngle() {
            return Json.read(json.get("start_angle"), Json::number);
        }

        /** {@link #getStartAngle()} を書く。null なら欄を除く */
        public EllipseGeometry withStartAngle(Double value) {
            put("start_angle", value);
            return this;
        }

        /** 終わりの角度（度）。楕円を円に引き延ばしたときの角度 */
        public Double getEndAngle() {
            return Json.read(json.get("end_angle"), Json::number);
        }

        /** {@link #getEndAngle()} を書く。null なら欄を除く */
        public EllipseGeometry withEndAngle(Double value) {
            put("end_angle", value);
            return this;
        }
    }

    /** 頂点を結んだ折れ線。幅は持たない */
    public static final class PolylineGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PolylineGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PolylineGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PolylineGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public PolylineGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "polyline");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public PolylineGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 頂点の [X, Y] の並び。2 つ以上 */
        public List<List<Double>> getPoints() {
            return Json.read(json.get("points"), v0 -> Json.list(v0, v1 -> Json.list(v1, Json::number)));
        }

        /** {@link #getPoints()} を書く。null なら欄を除く */
        public PolylineGeometry withPoints(List<List<Double>> value) {
            put("points", value);
            return this;
        }

        /** 頂点ごとの、次の頂点への弧の膨らみ（中心角の 4 分の 1 の正接。正は反時計回り）。points と同じ数。書かなければどれも 0 */
        public List<Double> getBulges() {
            return Json.read(json.get("bulges"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getBulges()} を書く。null なら欄を除く */
        public PolylineGeometry withBulges(List<Double> value) {
            put("bulges", value);
            return this;
        }

        /** 最後の頂点から最初の頂点へ結ぶ */
        public Boolean getClosed() {
            return Json.read(json.get("closed"), Json::bool);
        }

        /** {@link #getClosed()} を書く。null なら欄を除く */
        public PolylineGeometry withClosed(Boolean value) {
            put("closed", value);
            return this;
        }
    }

    /** 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせ・幅の倍率・斜体・反転・文字スタイルを元の値のままにする。足すときは、2 点目がなければ左の基準線に合わせ、幅の倍率 1・斜体なし・反転なしにする */
    public static final class TextGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public TextGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static TextGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new TextGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public TextGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "text");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public TextGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 内容。1 行 */
        public String getContent() {
            return Json.read(json.get("content"), Json::string);
        }

        /** {@link #getContent()} を書く。null なら欄を除く */
        public TextGeometry withContent(String value) {
            put("content", value);
            return this;
        }

        /** 基点 [X, Y]。2 点の間に合わせる文字では 1 点目 */
        public List<Double> getPosition() {
            return Json.read(json.get("position"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getPosition()} を書く。null なら欄を除く */
        public TextGeometry withPosition(List<Double> value) {
            put("position", value);
            return this;
        }

        /** 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。小版 17 から */
        public List<Double> getEnd() {
            return Json.read(json.get("end"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getEnd()} を書く。null なら欄を除く */
        public TextGeometry withEnd(List<Double> value) {
            put("end", value);
            return this;
        }

        /** 2 点の間への合わせ方。2 点目（end）と一緒に書く。小版 17 から */
        public String getFit() {
            return Json.read(json.get("fit"), Json::string);
        }

        /** {@link #getFit()} を書く。null なら欄を除く */
        public TextGeometry withFit(String value) {
            put("fit", value);
            return this;
        }

        /** 高さ。0 より大きい */
        public Double getHeight() {
            return Json.read(json.get("height"), Json::number);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public TextGeometry withHeight(Double value) {
            put("height", value);
            return this;
        }

        /** 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない */
        public Double getRotation() {
            return Json.read(json.get("rotation"), Json::number);
        }

        /** {@link #getRotation()} を書く。null なら欄を除く */
        public TextGeometry withRotation(Double value) {
            put("rotation", value);
            return this;
        }

        /** 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。小版 12 から */
        public String getStyle() {
            return Json.read(json.get("style"), Json::string);
        }

        /** {@link #getStyle()} を書く。null なら欄を除く */
        public TextGeometry withStyle(String value) {
            put("style", value);
            return this;
        }

        /** 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から */
        public String getStyleId() {
            return Json.read(json.get("style_id"), Json::string);
        }

        /** {@link #getStyleId()} を書く。null なら欄を除く */
        public TextGeometry withStyleId(String value) {
            put("style_id", value);
            return this;
        }
    }

    /**
     * 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class TextFit {
        private TextFit() {}

        public static final String ALIGNED = "aligned";

        public static final String FIT = "fit";
    }

    /** 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味・文字スタイルを元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。小版 13 から */
    public static final class FormattedTextGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public FormattedTextGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static FormattedTextGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new FormattedTextGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public FormattedTextGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "formatted_text");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public FormattedTextGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 内容。書式の符号（\P の改行など）を含めた、元の形式の内容のまま */
        public String getContent() {
            return Json.read(json.get("content"), Json::string);
        }

        /** {@link #getContent()} を書く。null なら欄を除く */
        public FormattedTextGeometry withContent(String value) {
            put("content", value);
            return this;
        }

        /** 基点 [X, Y] */
        public List<Double> getPosition() {
            return Json.read(json.get("position"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getPosition()} を書く。null なら欄を除く */
        public FormattedTextGeometry withPosition(List<Double> value) {
            put("position", value);
            return this;
        }

        /** 高さ。0 より大きい */
        public Double getHeight() {
            return Json.read(json.get("height"), Json::number);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public FormattedTextGeometry withHeight(Double value) {
            put("height", value);
            return this;
        }

        /** 回転（度） */
        public Double getRotation() {
            return Json.read(json.get("rotation"), Json::number);
        }

        /** {@link #getRotation()} を書く。null なら欄を除く */
        public FormattedTextGeometry withRotation(Double value) {
            put("rotation", value);
            return this;
        }

        /** 基点に合わせる枠の位置 */
        public String getAttachment() {
            return Json.read(json.get("attachment"), Json::string);
        }

        /** {@link #getAttachment()} を書く。null なら欄を除く */
        public FormattedTextGeometry withAttachment(String value) {
            put("attachment", value);
            return this;
        }

        /** 折り返す枠の幅。0 より大きい。書かなければ折り返さない */
        public Double getWidth() {
            return Json.read(json.get("width"), Json::number);
        }

        /** {@link #getWidth()} を書く。null なら欄を除く */
        public FormattedTextGeometry withWidth(Double value) {
            put("width", value);
            return this;
        }

        /** 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す */
        public String getStyle() {
            return Json.read(json.get("style"), Json::string);
        }

        /** {@link #getStyle()} を書く。null なら欄を除く */
        public FormattedTextGeometry withStyle(String value) {
            put("style", value);
            return this;
        }

        /** 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から */
        public String getStyleId() {
            return Json.read(json.get("style_id"), Json::string);
        }

        /** {@link #getStyleId()} を書く。null なら欄を除く */
        public FormattedTextGeometry withStyleId(String value) {
            put("style_id", value);
            return this;
        }
    }

    /** スプライン（NURBS）。制御点で決まるもの（小版 15 から）と、当てはめる点で決まるもの（小版 18 から）がある。当てはめる点で決まるものは、節点と制御点を空にし、当てはめる点（fit）と、あれば両端の接線を持つ。元の図形に制御点や節点もあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない */
    public static final class SplineGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SplineGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SplineGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SplineGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public SplineGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "spline");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public SplineGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 次数。1 以上 */
        public Long getDegree() {
            return Json.read(json.get("degree"), Json::integer);
        }

        /** {@link #getDegree()} を書く。null なら欄を除く */
        public SplineGeometry withDegree(Long value) {
            put("degree", value);
            return this;
        }

        /** 節点の並び。周期的でなければ、制御点の数と次数と 1 の和の数。当てはめる点で決まるものは空 */
        public List<Double> getKnots() {
            return Json.read(json.get("knots"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getKnots()} を書く。null なら欄を除く */
        public SplineGeometry withKnots(List<Double> value) {
            put("knots", value);
            return this;
        }

        /** 制御点の [X, Y] の並び。次数より多い。当てはめる点で決まるものは空 */
        public List<List<Double>> getControl() {
            return Json.read(json.get("control"), v0 -> Json.list(v0, v1 -> Json.list(v1, Json::number)));
        }

        /** {@link #getControl()} を書く。null なら欄を除く */
        public SplineGeometry withControl(List<List<Double>> value) {
            put("control", value);
            return this;
        }

        /** 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない */
        public List<Double> getWeights() {
            return Json.read(json.get("weights"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getWeights()} を書く。null なら欄を除く */
        public SplineGeometry withWeights(List<Double> value) {
            put("weights", value);
            return this;
        }

        /** 当てはめる点の [X, Y] の並び。2 つ以上。書けば、節点と制御点は空で、重みは書かない。小版 18 から */
        public List<List<Double>> getFit() {
            return Json.read(json.get("fit"), v0 -> Json.list(v0, v1 -> Json.list(v1, Json::number)));
        }

        /** {@link #getFit()} を書く。null なら欄を除く */
        public SplineGeometry withFit(List<List<Double>> value) {
            put("fit", value);
            return this;
        }

        /** 当てはめる点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から */
        public List<Double> getStartTangent() {
            return Json.read(json.get("start_tangent"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getStartTangent()} を書く。null なら欄を除く */
        public SplineGeometry withStartTangent(List<Double> value) {
            put("start_tangent", value);
            return this;
        }

        /** 当てはめる点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から */
        public List<Double> getEndTangent() {
            return Json.read(json.get("end_tangent"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getEndTangent()} を書く。null なら欄を除く */
        public SplineGeometry withEndTangent(List<Double> value) {
            put("end_tangent", value);
            return this;
        }

        /** 閉じている */
        public Boolean getClosed() {
            return Json.read(json.get("closed"), Json::bool);
        }

        /** {@link #getClosed()} を書く。null なら欄を除く */
        public SplineGeometry withClosed(Boolean value) {
            put("closed", value);
            return this;
        }

        /** 周期的 */
        public Boolean getPeriodic() {
            return Json.read(json.get("periodic"), Json::bool);
        }

        /** {@link #getPeriodic()} を書く。null なら欄を除く */
        public SplineGeometry withPeriodic(Boolean value) {
            put("periodic", value);
            return this;
        }
    }

    /** 寸法。種類（kind）ごとに、その種類の定義点の欄を持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。entities が返すだけで、置き換えと追加は change.edit.unsupported になる（寸法を描いた部品を作り直す必要があるため）。小版 19 から */
    public static final class DimensionGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DimensionGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DimensionGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DimensionGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public DimensionGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "dimension");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public DimensionGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 寸法の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public DimensionGeometry withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 1 本目の補助線の点か、測る角度の 1 本目の線の点か、円弧の始めの点 [X, Y]。linear・aligned・angular・arc_length が持つ */
        public List<Double> getFirst() {
            return Json.read(json.get("first"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getFirst()} を書く。null なら欄を除く */
        public DimensionGeometry withFirst(List<Double> value) {
            put("first", value);
            return this;
        }

        /** 2 本目の補助線の点か、測る角度の 2 本目の線の点か、円弧の終わりの点 [X, Y]。linear・aligned・angular・arc_length が持つ */
        public List<Double> getSecond() {
            return Json.read(json.get("second"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getSecond()} を書く。null なら欄を除く */
        public DimensionGeometry withSecond(List<Double> value) {
            put("second", value);
            return this;
        }

        /** 寸法線が通る点 [X, Y]。linear・aligned が持つ */
        public List<Double> getLine() {
            return Json.read(json.get("line"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getLine()} を書く。null なら欄を除く */
        public DimensionGeometry withLine(List<Double> value) {
            put("line", value);
            return this;
        }

        /** 測る向き（度）。linear が持つ */
        public Double getRotation() {
            return Json.read(json.get("rotation"), Json::number);
        }

        /** {@link #getRotation()} を書く。null なら欄を除く */
        public DimensionGeometry withRotation(Double value) {
            put("rotation", value);
            return this;
        }

        /** 補助線の傾き（度）。傾けなければ 0。linear・aligned が持つ */
        public Double getOblique() {
            return Json.read(json.get("oblique"), Json::number);
        }

        /** {@link #getOblique()} を書く。null なら欄を除く */
        public DimensionGeometry withOblique(Double value) {
            put("oblique", value);
            return this;
        }

        /** 角度の頂点 [X, Y]。angular が持つ */
        public List<Double> getVertex() {
            return Json.read(json.get("vertex"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getVertex()} を書く。null なら欄を除く */
        public DimensionGeometry withVertex(List<Double> value) {
            put("vertex", value);
            return this;
        }

        /** 1 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ */
        public List<List<Double>> getFirstLine() {
            return Json.read(json.get("first_line"), v0 -> Json.list(v0, v1 -> Json.list(v1, Json::number)));
        }

        /** {@link #getFirstLine()} を書く。null なら欄を除く */
        public DimensionGeometry withFirstLine(List<List<Double>> value) {
            put("first_line", value);
            return this;
        }

        /** 2 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ */
        public List<List<Double>> getSecondLine() {
            return Json.read(json.get("second_line"), v0 -> Json.list(v0, v1 -> Json.list(v1, Json::number)));
        }

        /** {@link #getSecondLine()} を書く。null なら欄を除く */
        public DimensionGeometry withSecondLine(List<List<Double>> value) {
            put("second_line", value);
            return this;
        }

        /** 寸法の弧が通る点 [X, Y]。angular・angular_lines・arc_length が持つ */
        public List<Double> getArc() {
            return Json.read(json.get("arc"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getArc()} を書く。null なら欄を除く */
        public DimensionGeometry withArc(List<Double> value) {
            put("arc", value);
            return this;
        }

        /** 中心 [X, Y]。arc_length・radius が持つ */
        public List<Double> getCenter() {
            return Json.read(json.get("center"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getCenter()} を書く。null なら欄を除く */
        public DimensionGeometry withCenter(List<Double> value) {
            put("center", value);
            return this;
        }

        /** 円の上の点 [X, Y]。radius・diameter が持つ */
        public List<Double> getChord() {
            return Json.read(json.get("chord"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getChord()} を書く。null なら欄を除く */
        public DimensionGeometry withChord(List<Double> value) {
            put("chord", value);
            return this;
        }

        /** chord と向かい合う円の上の点 [X, Y]。diameter が持つ */
        public List<Double> getOpposite() {
            return Json.read(json.get("opposite"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getOpposite()} を書く。null なら欄を除く */
        public DimensionGeometry withOpposite(List<Double> value) {
            put("opposite", value);
            return this;
        }

        /** 引出線の長さ。radius・diameter が持つ */
        public Double getLeader() {
            return Json.read(json.get("leader"), Json::number);
        }

        /** {@link #getLeader()} を書く。null なら欄を除く */
        public DimensionGeometry withLeader(Double value) {
            put("leader", value);
            return this;
        }

        /** 原点 [X, Y]。ordinate が持つ */
        public List<Double> getOrigin() {
            return Json.read(json.get("origin"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getOrigin()} を書く。null なら欄を除く */
        public DimensionGeometry withOrigin(List<Double> value) {
            put("origin", value);
            return this;
        }

        /** 測る点 [X, Y]。ordinate が持つ */
        public List<Double> getFeature() {
            return Json.read(json.get("feature"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getFeature()} を書く。null なら欄を除く */
        public DimensionGeometry withFeature(List<Double> value) {
            put("feature", value);
            return this;
        }

        /** 引出線の終点 [X, Y]。ordinate が持つ */
        public List<Double> getLeaderEnd() {
            return Json.read(json.get("leader_end"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getLeaderEnd()} を書く。null なら欄を除く */
        public DimensionGeometry withLeaderEnd(List<Double> value) {
            put("leader_end", value);
            return this;
        }

        /** 測る座標の軸。ordinate が持つ */
        public String getAxis() {
            return Json.read(json.get("axis"), Json::string);
        }

        /** {@link #getAxis()} を書く。null なら欄を除く */
        public DimensionGeometry withAxis(String value) {
            put("axis", value);
            return this;
        }

        /** 寸法値の文字の書き方 */
        public String getLabel() {
            return Json.read(json.get("label"), Json::string);
        }

        /** {@link #getLabel()} を書く。null なら欄を除く */
        public DimensionGeometry withLabel(String value) {
            put("label", value);
            return this;
        }

        /** 利用者が書いた寸法値の文字の元の内容。label が override のときだけ持つ。測定値に置き換える符号（DXF の <>）と書式の符号もそのまま持つ */
        public String getContent() {
            return Json.read(json.get("content"), Json::string);
        }

        /** {@link #getContent()} を書く。null なら欄を除く */
        public DimensionGeometry withContent(String value) {
            put("content", value);
            return this;
        }

        /** 元の形式が保存した測定値。単位と倍率は measurement_key の欄に従う */
        public Double getMeasurement() {
            return Json.read(json.get("measurement"), Json::number);
        }

        /** {@link #getMeasurement()} を書く。null なら欄を除く */
        public DimensionGeometry withMeasurement(Double value) {
            put("measurement", value);
            return this;
        }

        /** measurement の形式と欄を指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ */
        public String getMeasurementKey() {
            return Json.read(json.get("measurement_key"), Json::string);
        }

        /** {@link #getMeasurementKey()} を書く。null なら欄を除く */
        public DimensionGeometry withMeasurementKey(String value) {
            put("measurement_key", value);
            return this;
        }

        /** 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない */
        public List<Double> getTextPosition() {
            return Json.read(json.get("text_position"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getTextPosition()} を書く。null なら欄を除く */
        public DimensionGeometry withTextPosition(List<Double> value) {
            put("text_position", value);
            return this;
        }

        /** 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない */
        public Double getTextRotation() {
            return Json.read(json.get("text_rotation"), Json::number);
        }

        /** {@link #getTextRotation()} を書く。null なら欄を除く */
        public DimensionGeometry withTextRotation(Double value) {
            put("text_rotation", value);
            return this;
        }

        /** 寸法スタイルの元の名前 */
        public String getStyle() {
            return Json.read(json.get("style"), Json::string);
        }

        /** {@link #getStyle()} を書く。null なら欄を除く */
        public DimensionGeometry withStyle(String value) {
            put("style", value);
            return this;
        }

        /** 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。小版 23 から */
        public String getStyleId() {
            return Json.read(json.get("style_id"), Json::string);
        }

        /** {@link #getStyleId()} を書く。null なら欄を除く */
        public DimensionGeometry withStyleId(String value) {
            put("style_id", value);
            return this;
        }
    }

    /** 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（小版 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印の印を必ず持つので、arrow を書かない引出線は書けない（小版 36 から。それまでは R2004 までの置き換えだけ）。小版 31 から */
    public static final class LeaderGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LeaderGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LeaderGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LeaderGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public LeaderGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "leader");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public LeaderGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 経路の頂点 [[X, Y], ...]。矢印の側から並べる。2 つ以上 */
        public List<List<Double>> getPath() {
            return Json.read(json.get("path"), v0 -> Json.list(v0, v1 -> Json.list(v1, Json::number)));
        }

        /** {@link #getPath()} を書く。null なら欄を除く */
        public LeaderGeometry withPath(List<List<Double>> value) {
            put("path", value);
            return this;
        }

        /** 頂点を通る曲線で結ぶか。偽なら折れ線 */
        public Boolean getSpline() {
            return Json.read(json.get("spline"), Json::bool);
        }

        /** {@link #getSpline()} を書く。null なら欄を除く */
        public LeaderGeometry withSpline(Boolean value) {
            put("spline", value);
            return this;
        }

        /** 始点に矢印を付けるか。書かなければ寸法スタイルに従う */
        public Boolean getArrow() {
            return Json.read(json.get("arrow"), Json::bool);
        }

        /** {@link #getArrow()} を書く。null なら欄を除く */
        public LeaderGeometry withArrow(Boolean value) {
            put("arrow", value);
            return this;
        }

        /** 注記の要素の ID。同じ空間か用紙の要素で、entities が返したもの。注記がなければ書かない。置き換えるときは元と同じ ID だけを受ける。足すときは、同じ空間か用紙の書式つきの文字（MTEXT）を指す */
        public String getAnnotation() {
            return Json.read(json.get("annotation"), Json::string);
        }

        /** {@link #getAnnotation()} を書く。null なら欄を除く */
        public LeaderGeometry withAnnotation(String value) {
            put("annotation", value);
            return this;
        }

        /** 寸法スタイルの元の名前。足すときは図面にある寸法スタイルの名前を指す。置き換えるときは元と同じ名前だけを受ける */
        public String getStyle() {
            return Json.read(json.get("style"), Json::string);
        }

        /** {@link #getStyle()} を書く。null なら欄を除く */
        public LeaderGeometry withStyle(String value) {
            put("style", value);
            return this;
        }

        /** 寸法スタイルの ID。tables が返したもの。entities は、名前で引いた寸法スタイルの ID を返す。足す・置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける */
        public String getStyleId() {
            return Json.read(json.get("style_id"), Json::string);
        }

        /** {@link #getStyleId()} を書く。null なら欄を除く */
        public LeaderGeometry withStyleId(String value) {
            put("style_id", value);
            return this;
        }
    }

    /** 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかの欄は元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。小版 36 から */
    public static final class CalloutGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CalloutGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CalloutGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CalloutGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public CalloutGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "callout");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public CalloutGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 枝。入力の順 */
        public List<CalloutBranch> getBranches() {
            return Json.read(json.get("branches"), v0 -> Json.list(v0, CalloutBranch::of));
        }

        /** {@link #getBranches()} を書く。null なら欄を除く */
        public CalloutGeometry withBranches(List<CalloutBranch> value) {
            put("branches", value);
            return this;
        }

        /** 線の引き方 */
        public String getPath() {
            return Json.read(json.get("path"), Json::string);
        }

        /** {@link #getPath()} を書く。null なら欄を除く */
        public CalloutGeometry withPath(String value) {
            put("path", value);
            return this;
        }

        /** 線を注記に、水平の線（ランディング）でつなぐか */
        public Boolean getLanding() {
            return Json.read(json.get("landing"), Json::bool);
        }

        /** {@link #getLanding()} を書く。null なら欄を除く */
        public CalloutGeometry withLanding(Boolean value) {
            put("landing", value);
            return this;
        }

        /** 端部記号の大きさ */
        public Double getArrowSize() {
            return Json.read(json.get("arrow_size"), Json::number);
        }

        /** {@link #getArrowSize()} を書く。null なら欄を除く */
        public CalloutGeometry withArrowSize(Double value) {
            put("arrow_size", value);
            return this;
        }

        /** 大きさに掛ける倍率 */
        public Double getScale() {
            return Json.read(json.get("scale"), Json::number);
        }

        /** {@link #getScale()} を書く。null なら欄を除く */
        public CalloutGeometry withScale(Double value) {
            put("scale", value);
            return this;
        }

        /** 注記の種類 */
        public String getContent() {
            return Json.read(json.get("content"), Json::string);
        }

        /** {@link #getContent()} を書く。null なら欄を除く */
        public CalloutGeometry withContent(String value) {
            put("content", value);
            return this;
        }

        /** 注記の書式つきの文字の元の内容。content が text のときだけ持つ */
        public String getText() {
            return Json.read(json.get("text"), Json::string);
        }

        /** {@link #getText()} を書く。null なら欄を除く */
        public CalloutGeometry withText(String value) {
            put("text", value);
            return this;
        }

        /** 注記の記号のブロックの名前。content が symbol のときだけ持つ */
        public String getSymbol() {
            return Json.read(json.get("symbol"), Json::string);
        }

        /** {@link #getSymbol()} を書く。null なら欄を除く */
        public CalloutGeometry withSymbol(String value) {
            put("symbol", value);
            return this;
        }
    }

    /** 引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる */
    public static final class CalloutBranch extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CalloutBranch(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CalloutBranch of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CalloutBranch(json);
        }

        /** 空のオブジェクトから作る */
        public CalloutBranch() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 線ごとの頂点 [[[X, Y], ...], ...]。矢印の側から並べ、集まる点は含めない。線ごとに 1 つ以上 */
        public List<List<List<Double>>> getLines() {
            return Json.read(json.get("lines"), v0 -> Json.list(v0, v1 -> Json.list(v1, v2 -> Json.list(v2, Json::number))));
        }

        /** {@link #getLines()} を書く。null なら欄を除く */
        public CalloutBranch withLines(List<List<List<Double>>> value) {
            put("lines", value);
            return this;
        }

        /** 線が集まる点 [X, Y]。元の形式が持たなければ書かない */
        public List<Double> getEnd() {
            return Json.read(json.get("end"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getEnd()} を書く。null なら欄を除く */
        public CalloutBranch withEnd(List<Double> value) {
            put("end", value);
            return this;
        }

        /** 集まる点から注記への折れ曲がり。なければ書かない */
        public CalloutDogleg getDogleg() {
            return Json.read(json.get("dogleg"), CalloutDogleg::of);
        }

        /** {@link #getDogleg()} を書く。null なら欄を除く */
        public CalloutBranch withDogleg(CalloutDogleg value) {
            put("dogleg", value);
            return this;
        }
    }

    /** 枝の折れ曲がり */
    public static final class CalloutDogleg extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CalloutDogleg(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CalloutDogleg of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CalloutDogleg(json);
        }

        /** 空のオブジェクトから作る */
        public CalloutDogleg() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 向き [X, Y]。長さは意味を持たない */
        public List<Double> getDirection() {
            return Json.read(json.get("direction"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getDirection()} を書く。null なら欄を除く */
        public CalloutDogleg withDirection(List<Double> value) {
            put("direction", value);
            return this;
        }

        /** 長さ */
        public Double getLength() {
            return Json.read(json.get("length"), Json::number);
        }

        /** {@link #getLength()} を書く。null なら欄を除く */
        public CalloutDogleg withLength(Double value) {
            put("length", value);
            return this;
        }
    }

    /**
     * 引出線つきの注記の線の引き方。straight は折れ線、spline は頂点を通る曲線、hidden は線を描かない
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class CalloutPath {
        private CalloutPath() {}

        public static final String STRAIGHT = "straight";

        public static final String SPLINE = "spline";

        public static final String HIDDEN = "hidden";
    }

    /**
     * 引出線つきの注記の中身の種類。none は注記を持たず、text は書式つきの文字、symbol は記号（バルーンなどの囲み）
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class CalloutContent {
        private CalloutContent() {}

        public static final String NONE = "none";

        public static final String TEXT = "text";

        public static final String SYMBOL = "symbol";
    }

    /** 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義の印の点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。小版 35 から */
    public static final class PointGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PointGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PointGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PointGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public PointGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "point");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public PointGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 位置 [X, Y] */
        public List<Double> getPosition() {
            return Json.read(json.get("position"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getPosition()} を書く。null なら欄を除く */
        public PointGeometry withPosition(List<Double> value) {
            put("position", value);
            return this;
        }

        /** 回転（度） */
        public Double getRotation() {
            return Json.read(json.get("rotation"), Json::number);
        }

        /** {@link #getRotation()} を書く。null なら欄を除く */
        public PointGeometry withRotation(Double value) {
            put("rotation", value);
            return this;
        }

        /** 既定義の印。書かなければ文書の点の表し方に従う */
        public PointMarker getMarker() {
            return Json.read(json.get("marker"), PointMarker::of);
        }

        /** {@link #getMarker()} を書く。null なら欄を除く */
        public PointGeometry withMarker(PointMarker value) {
            put("marker", value);
            return this;
        }

        /** 作図の補助の点（JWW の仮点）か。書かなければ偽 */
        public Boolean getConstruction() {
            return Json.read(json.get("construction"), Json::bool);
        }

        /** {@link #getConstruction()} を書く。null なら欄を除く */
        public PointGeometry withConstruction(Boolean value) {
            put("construction", value);
            return this;
        }
    }

    /** 既定義の印の表のキーと尺度。印の形は、表示する側がキーから引く */
    public static final class PointMarker extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PointMarker(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PointMarker of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PointMarker(json);
        }

        /** 空のオブジェクトから作る */
        public PointMarker() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 印の表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点） */
        public String getNamespace() {
            return Json.read(json.get("namespace"), Json::string);
        }

        /** {@link #getNamespace()} を書く。null なら欄を除く */
        public PointMarker withNamespace(String value) {
            put("namespace", value);
            return this;
        }

        /** 名前空間の中の元のコード */
        public String getCode() {
            return Json.read(json.get("code"), Json::string);
        }

        /** {@link #getCode()} を書く。null なら欄を除く */
        public PointMarker withCode(String value) {
            put("code", value);
            return this;
        }

        /** 尺度 */
        public Double getScale() {
            return Json.read(json.get("scale"), Json::number);
        }

        /** {@link #getScale()} を書く。null なら欄を除く */
        public PointMarker withScale(Double value) {
            put("scale", value);
            return this;
        }
    }

    /**
     * 寸法の種類。linear は 2 点を測る向きに、aligned は 2 点を結ぶ向きに測った長さ。angular は頂点から 2 点へ向かう線の、angular_lines は 2 本の線のなす角度。arc_length は円弧の長さ。radius は半径、diameter は直径。ordinate は原点からの X か Y の座標
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class DimensionKind {
        private DimensionKind() {}

        public static final String LINEAR = "linear";

        public static final String ALIGNED = "aligned";

        public static final String ANGULAR = "angular";

        public static final String ANGULAR_LINES = "angular_lines";

        public static final String ARC_LENGTH = "arc_length";

        public static final String RADIUS = "radius";

        public static final String DIAMETER = "diameter";

        public static final String ORDINATE = "ordinate";
    }

    /**
     * 座標の軸
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class DimensionAxis {
        private DimensionAxis() {}

        public static final String X = "x";

        public static final String Y = "y";
    }

    /**
     * 寸法値の文字の書き方。measurement は測定値を書き、override は利用者が書いた文字（content）を書き、hidden は書かない
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class DimensionLabel {
        private DimensionLabel() {}

        public static final String MEASUREMENT = "measurement";

        public static final String OVERRIDE = "override";

        public static final String HIDDEN = "hidden";
    }

    /** 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。小版 14 から */
    public static final class SymbolGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SymbolGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SymbolGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SymbolGeometry(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public SymbolGeometry() {
            super(new LinkedHashMap<String, Object>());
            json.put("shape", "symbol");
        }

        /** 形の種類 */
        public String getShape() {
            return Json.read(json.get("shape"), Json::string);
        }

        /** {@link #getShape()} を書く。null なら欄を除く */
        public SymbolGeometry withShape(String value) {
            put("shape", value);
            return this;
        }

        /** 記号の定義（ブロック）の元の名前。足すときは図面にある定義の名前を指す */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public SymbolGeometry withName(String value) {
            put("name", value);
            return this;
        }

        /** 定義の基点を置く点 [X, Y] */
        public List<Double> getPosition() {
            return Json.read(json.get("position"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getPosition()} を書く。null なら欄を除く */
        public SymbolGeometry withPosition(List<Double> value) {
            put("position", value);
            return this;
        }

        /** X と Y の尺度 [X, Y]。0 でない。負なら反転する */
        public List<Double> getScale() {
            return Json.read(json.get("scale"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getScale()} を書く。null なら欄を除く */
        public SymbolGeometry withScale(List<Double> value) {
            put("scale", value);
            return this;
        }

        /** 回転（度） */
        public Double getRotation() {
            return Json.read(json.get("rotation"), Json::number);
        }

        /** {@link #getRotation()} を書く。null なら欄を除く */
        public SymbolGeometry withRotation(Double value) {
            put("rotation", value);
            return this;
        }

        /** 格子に並べて置く列と行。書かなければ 1 つだけ置く */
        public SymbolGrid getGrid() {
            return Json.read(json.get("grid"), SymbolGrid::of);
        }

        /** {@link #getGrid()} を書く。null なら欄を除く */
        public SymbolGeometry withGrid(SymbolGrid value) {
            put("grid", value);
            return this;
        }

        /** 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。小版 16 から。置き換えで値を変えるのは小版 21 から */
        public List<SymbolAttribute> getAttributes() {
            return Json.read(json.get("attributes"), v0 -> Json.list(v0, SymbolAttribute::of));
        }

        /** {@link #getAttributes()} を書く。null なら欄を除く */
        public SymbolGeometry withAttributes(List<SymbolAttribute> value) {
            put("attributes", value);
            return this;
        }
    }

    /** 記号の属性のタグと値 */
    public static final class SymbolAttribute extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SymbolAttribute(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SymbolAttribute of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SymbolAttribute(json);
        }

        /** 空のオブジェクトから作る */
        public SymbolAttribute() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 元のタグ */
        public String getTag() {
            return Json.read(json.get("tag"), Json::string);
        }

        /** {@link #getTag()} を書く。null なら欄を除く */
        public SymbolAttribute withTag(String value) {
            put("tag", value);
            return this;
        }

        /** 値。1 行 */
        public String getValue() {
            return Json.read(json.get("value"), Json::string);
        }

        /** {@link #getValue()} を書く。null なら欄を除く */
        public SymbolAttribute withValue(String value) {
            put("value", value);
            return this;
        }
    }

    /** 記号を格子に並べる列と行。間隔は、記号を回転する前の向きに測る。置き換えるときは、列と行の数を変えない */
    public static final class SymbolGrid extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SymbolGrid(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SymbolGrid of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SymbolGrid(json);
        }

        /** 空のオブジェクトから作る */
        public SymbolGrid() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 列の数。1 以上 */
        public Long getColumns() {
            return Json.read(json.get("columns"), Json::integer);
        }

        /** {@link #getColumns()} を書く。null なら欄を除く */
        public SymbolGrid withColumns(Long value) {
            put("columns", value);
            return this;
        }

        /** 行の数。1 以上。列と行がどちらも 1 の格子は持たない */
        public Long getRows() {
            return Json.read(json.get("rows"), Json::integer);
        }

        /** {@link #getRows()} を書く。null なら欄を除く */
        public SymbolGrid withRows(Long value) {
            put("rows", value);
            return this;
        }

        /** 列の間隔 */
        public Double getColumnSpacing() {
            return Json.read(json.get("column_spacing"), Json::number);
        }

        /** {@link #getColumnSpacing()} を書く。null なら欄を除く */
        public SymbolGrid withColumnSpacing(Double value) {
            put("column_spacing", value);
            return this;
        }

        /** 行の間隔 */
        public Double getRowSpacing() {
            return Json.read(json.get("row_spacing"), Json::number);
        }

        /** {@link #getRowSpacing()} を書く。null なら欄を除く */
        public SymbolGrid withRowSpacing(Double value) {
            put("row_spacing", value);
            return this;
        }
    }

    /**
     * 基点に合わせる枠の位置。entities は返し、足すときに書かなければ左上、置き換えるときに書かなければ元のまま
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class Attachment {
        private Attachment() {}

        public static final String TOP_LEFT = "top_left";

        public static final String TOP_CENTER = "top_center";

        public static final String TOP_RIGHT = "top_right";

        public static final String MIDDLE_LEFT = "middle_left";

        public static final String MIDDLE_CENTER = "middle_center";

        public static final String MIDDLE_RIGHT = "middle_right";

        public static final String BOTTOM_LEFT = "bottom_left";

        public static final String BOTTOM_CENTER = "bottom_center";

        public static final String BOTTOM_RIGHT = "bottom_right";
    }

    /**
     * 要素の形の種類（決定 0022）。point（点、決定 0160）は小版 32 から。それまで点は symbol だった
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class EntityKind {
        private EntityKind() {}

        public static final String LINE = "line";

        public static final String CIRCLE = "circle";

        public static final String CURVE = "curve";

        public static final String POLYLINE = "polyline";

        public static final String TEXT = "text";

        public static final String FORMATTED_TEXT = "formatted_text";

        public static final String DIMENSION = "dimension";

        public static final String LEADER = "leader";

        public static final String CALLOUT = "callout";

        public static final String TABLE = "table";

        public static final String UNDERLAY = "underlay";

        public static final String SYMBOL = "symbol";

        public static final String HATCH = "hatch";

        public static final String POINT = "point";
    }

    /** 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。小版 7 から */
    public static final class ApplyRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ApplyRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ApplyRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ApplyRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public ApplyRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "apply");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public ApplyRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public ApplyRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public ApplyRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public ApplyRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public ApplyRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public ApplyRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で作ったもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public ApplyRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 読んだときの文書のハッシュ（drawing の hash）。書けば、今の文書と同じときだけ当てる */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public ApplyRequest withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 変更。並べた順に当てる */
        public List<Edit> getEdits() {
            return Json.read(json.get("edits"), v0 -> Json.list(v0, Edit::of));
        }

        /** {@link #getEdits()} を書く。null なら欄を除く */
        public ApplyRequest withEdits(List<Edit> value) {
            put("edits", value);
            return this;
        }
    }

    /** 当てた変更の結果。書き戻したファイルは blobs の file で返す */
    public static final class ApplyResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ApplyResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ApplyResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ApplyResult(json);
        }

        /** 空のオブジェクトから作る */
        public ApplyResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 変更ごとの結果。要求の edits と同じ順 */
        public List<Applied> getApplied() {
            return Json.read(json.get("applied"), v0 -> Json.list(v0, Applied::of));
        }

        /** {@link #getApplied()} を書く。null なら欄を除く */
        public ApplyResult withApplied(List<Applied> value) {
            put("applied", value);
            return this;
        }

        /** 書き戻した文書のハッシュ。次の apply の前提条件に使う */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public ApplyResult withHash(String value) {
            put("hash", value);
            return this;
        }

        /** true なら、変えていない要素の ID がずれた。鍵を持たない形式（JWW）で、前の図形を除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。小版 30 から */
        public Boolean getIdsShifted() {
            return Json.read(json.get("ids_shifted"), Json::bool);
        }

        /** {@link #getIdsShifted()} を書く。null なら欄を除く */
        public ApplyResult withIdsShifted(Boolean value) {
            put("ids_shifted", value);
            return this;
        }
    }

    /** 開いた文書の図面モデルを、別の形式の新しいファイルに書き、新しい文書として開く（決定 0139・0206）。書けない要素は落とし、種類ごとの件数を診断で返す。書いたファイルは blobs で返す。今は DXF にだけ書ける。小版 41 から */
    public static final class ConvertRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ConvertRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ConvertRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ConvertRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public ConvertRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "convert");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public ConvertRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public ConvertRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public ConvertRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public ConvertRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public ConvertRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public ConvertRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 書く元の文書の名前。open で作ったもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public ConvertRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 書く形式の名前（capabilities の formats の id）。今は dxf だけ */
        public String getFormat() {
            return Json.read(json.get("format"), Json::string);
        }

        /** {@link #getFormat()} を書く。null なら欄を除く */
        public ConvertRequest withFormat(String value) {
            put("format", value);
            return this;
        }

        /** 作る文書の名前 */
        public String getTo() {
            return Json.read(json.get("to"), Json::string);
        }

        /** {@link #getTo()} を書く。null なら欄を除く */
        public ConvertRequest withTo(String value) {
            put("to", value);
            return this;
        }
    }

    /** 書いた文書。書いたファイルは blobs の file で返す */
    public static final class ConvertResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ConvertResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ConvertResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ConvertResult(json);
        }

        /** 空のオブジェクトから作る */
        public ConvertResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 作った文書の名前 */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public ConvertResult withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 書いた形式の名前 */
        public String getFormat() {
            return Json.read(json.get("format"), Json::string);
        }

        /** {@link #getFormat()} を書く。null なら欄を除く */
        public ConvertResult withFormat(String value) {
            put("format", value);
            return this;
        }

        /** 書いたファイルのハッシュ。apply の前提条件に使う */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public ConvertResult withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 書いた要素の数。記号から離した属性の文字を含む */
        public Long getElements() {
            return Json.read(json.get("elements"), Json::integer);
        }

        /** {@link #getElements()} を書く。null なら欄を除く */
        public ConvertResult withElements(Long value) {
            put("elements", value);
            return this;
        }
    }

    /** 当てた変更 1 つの結果 */
    public static final class Applied extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Applied(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Applied of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Applied(json);
        }

        /** 空のオブジェクトから作る */
        public Applied() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 変えた要素（レイヤの変更では、そのレイヤ）の、書き戻した後の ID。除いたときと、ID を決められないときは持たない */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public Applied withId(String value) {
            put("id", value);
            return this;
        }

        /** 変えた要素（レイヤ）の、書き戻した後のハッシュ。id があるときに持つ */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public Applied withHash(String value) {
            put("hash", value);
            return this;
        }
    }

    /** 要素 1 つへの変更。kind の値で形が決まる */
    public static final class Edit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Edit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Edit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Edit(json);
        }

        /** kind が restyle の形なら、その見方。違えば null */
        public RestyleEdit asRestyleEdit() {
            return "restyle".equals(json.get("kind")) ? new RestyleEdit(json) : null;
        }

        /** kind が transform の形なら、その見方。違えば null */
        public TransformEdit asTransformEdit() {
            return "transform".equals(json.get("kind")) ? new TransformEdit(json) : null;
        }

        /** kind が remove の形なら、その見方。違えば null */
        public RemoveEdit asRemoveEdit() {
            return "remove".equals(json.get("kind")) ? new RemoveEdit(json) : null;
        }

        /** kind が replace の形なら、その見方。違えば null */
        public ReplaceEdit asReplaceEdit() {
            return "replace".equals(json.get("kind")) ? new ReplaceEdit(json) : null;
        }

        /** kind が add の形なら、その見方。違えば null */
        public AddEdit asAddEdit() {
            return "add".equals(json.get("kind")) ? new AddEdit(json) : null;
        }

        /** kind が layer の形なら、その見方。違えば null */
        public LayerEdit asLayerEdit() {
            return "layer".equals(json.get("kind")) ? new LayerEdit(json) : null;
        }

        /** kind が add_layer の形なら、その見方。違えば null */
        public AddLayerEdit asAddLayerEdit() {
            return "add_layer".equals(json.get("kind")) ? new AddLayerEdit(json) : null;
        }

        /** kind が remove_layer の形なら、その見方。違えば null */
        public RemoveLayerEdit asRemoveLayerEdit() {
            return "remove_layer".equals(json.get("kind")) ? new RemoveLayerEdit(json) : null;
        }

        /** kind が text_style の形なら、その見方。違えば null */
        public TextStyleEdit asTextStyleEdit() {
            return "text_style".equals(json.get("kind")) ? new TextStyleEdit(json) : null;
        }

        /** kind が add_text_style の形なら、その見方。違えば null */
        public AddTextStyleEdit asAddTextStyleEdit() {
            return "add_text_style".equals(json.get("kind")) ? new AddTextStyleEdit(json) : null;
        }

        /** kind が remove_text_style の形なら、その見方。違えば null */
        public RemoveTextStyleEdit asRemoveTextStyleEdit() {
            return "remove_text_style".equals(json.get("kind")) ? new RemoveTextStyleEdit(json) : null;
        }

        /** kind が line_type の形なら、その見方。違えば null */
        public LineTypeEdit asLineTypeEdit() {
            return "line_type".equals(json.get("kind")) ? new LineTypeEdit(json) : null;
        }

        /** kind が add_line_type の形なら、その見方。違えば null */
        public AddLineTypeEdit asAddLineTypeEdit() {
            return "add_line_type".equals(json.get("kind")) ? new AddLineTypeEdit(json) : null;
        }

        /** kind が remove_line_type の形なら、その見方。違えば null */
        public RemoveLineTypeEdit asRemoveLineTypeEdit() {
            return "remove_line_type".equals(json.get("kind")) ? new RemoveLineTypeEdit(json) : null;
        }

        /** kind が dimension_style の形なら、その見方。違えば null */
        public DimensionStyleEdit asDimensionStyleEdit() {
            return "dimension_style".equals(json.get("kind")) ? new DimensionStyleEdit(json) : null;
        }

        /** kind が add_dimension_style の形なら、その見方。違えば null */
        public AddDimensionStyleEdit asAddDimensionStyleEdit() {
            return "add_dimension_style".equals(json.get("kind")) ? new AddDimensionStyleEdit(json) : null;
        }

        /** kind が remove_dimension_style の形なら、その見方。違えば null */
        public RemoveDimensionStyleEdit asRemoveDimensionStyleEdit() {
            return "remove_dimension_style".equals(json.get("kind")) ? new RemoveDimensionStyleEdit(json) : null;
        }

        /** kind が point_style の形なら、その見方。違えば null */
        public PointStyleEdit asPointStyleEdit() {
            return "point_style".equals(json.get("kind")) ? new PointStyleEdit(json) : null;
        }
    }

    /** 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない */
    public static final class RestyleEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public RestyleEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static RestyleEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new RestyleEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public RestyleEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "restyle");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public RestyleEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 変える要素の ID。entities が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public RestyleEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの要素のハッシュ。entities が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public RestyleEdit withHash(String value) {
            put("hash", value);
            return this;
        }

        /** レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない */
        public String getLayer() {
            return Json.read(json.get("layer"), Json::string);
        }

        /** {@link #getLayer()} を書く。null なら欄を除く */
        public RestyleEdit withLayer(String value) {
            put("layer", value);
            return this;
        }

        /** レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から */
        public String getLayerId() {
            return Json.read(json.get("layer_id"), Json::string);
        }

        /** {@link #getLayerId()} を書く。null なら欄を除く */
        public RestyleEdit withLayerId(String value) {
            put("layer_id", value);
            return this;
        }

        /** 色 */
        public ColorChange getColor() {
            return Json.read(json.get("color"), ColorChange::of);
        }

        /** {@link #getColor()} を書く。null なら欄を除く */
        public RestyleEdit withColor(ColorChange value) {
            put("color", value);
            return this;
        }

        /** 線種 */
        public LineTypeChange getLineType() {
            return Json.read(json.get("line_type"), LineTypeChange::of);
        }

        /** {@link #getLineType()} を書く。null なら欄を除く */
        public RestyleEdit withLineType(LineTypeChange value) {
            put("line_type", value);
            return this;
        }

        /** 要素の線種の倍率 */
        public Double getLineTypeScale() {
            return Json.read(json.get("line_type_scale"), Json::number);
        }

        /** {@link #getLineTypeScale()} を書く。null なら欄を除く */
        public RestyleEdit withLineTypeScale(Double value) {
            put("line_type_scale", value);
            return this;
        }

        /** 線幅 */
        public LineWeightChange getLineWeight() {
            return Json.read(json.get("line_weight"), LineWeightChange::of);
        }

        /** {@link #getLineWeight()} を書く。null なら欄を除く */
        public RestyleEdit withLineWeight(LineWeightChange value) {
            put("line_weight", value);
            return this;
        }
    }

    /** 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない */
    public static final class TransformEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public TransformEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static TransformEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new TransformEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public TransformEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "transform");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public TransformEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 動かす要素の ID。entities が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public TransformEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの要素のハッシュ。entities が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public TransformEdit withHash(String value) {
            put("hash", value);
            return this;
        }

        /** [a, b, c, d, e, f]。要素の空間か用紙の点 (x, y) を (a x + b y + c, d x + e y + f) に写す */
        public List<Double> getMatrix() {
            return Json.read(json.get("matrix"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getMatrix()} を書く。null なら欄を除く */
        public TransformEdit withMatrix(List<Double> value) {
            put("matrix", value);
            return this;
        }
    }

    /** 要素を除く */
    public static final class RemoveEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public RemoveEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static RemoveEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new RemoveEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public RemoveEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "remove");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public RemoveEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 除く要素の ID。entities が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public RemoveEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの要素のハッシュ。entities が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public RemoveEdit withHash(String value) {
            put("hash", value);
            return this;
        }
    }

    /** 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤとスタイルは変えない。小版 10 から */
    public static final class ReplaceEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ReplaceEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ReplaceEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ReplaceEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public ReplaceEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "replace");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public ReplaceEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 置き換える要素の ID。entities が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public ReplaceEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの要素のハッシュ。entities が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public ReplaceEdit withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 新しい形。entities の geometry と同じ種類 */
        public Geometry getGeometry() {
            return Json.read(json.get("geometry"), Geometry::of);
        }

        /** {@link #getGeometry()} を書く。null なら欄を除く */
        public ReplaceEdit withGeometry(Geometry value) {
            put("geometry", value);
            return this;
        }
    }

    /** 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。足せる形は、形の値のどの種類も。小版 10 から */
    public static final class AddEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public AddEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static AddEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new AddEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public AddEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "add");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public AddEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。小版 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177） */
        public String getPlace() {
            return Json.read(json.get("place"), Json::string);
        }

        /** {@link #getPlace()} を書く。null なら欄を除く */
        public AddEdit withPlace(String value) {
            put("place", value);
            return this;
        }

        /** レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない。どちらも書かなければレイヤ 0 */
        public String getLayer() {
            return Json.read(json.get("layer"), Json::string);
        }

        /** {@link #getLayer()} を書く。null なら欄を除く */
        public AddEdit withLayer(String value) {
            put("layer", value);
            return this;
        }

        /** レイヤの ID。tables が返したもの */
        public String getLayerId() {
            return Json.read(json.get("layer_id"), Json::string);
        }

        /** {@link #getLayerId()} を書く。null なら欄を除く */
        public AddEdit withLayerId(String value) {
            put("layer_id", value);
            return this;
        }

        /** 色。SXF はレイヤに従う色を書けないので、書く。小版 29 から */
        public ColorChange getColor() {
            return Json.read(json.get("color"), ColorChange::of);
        }

        /** {@link #getColor()} を書く。null なら欄を除く */
        public AddEdit withColor(ColorChange value) {
            put("color", value);
            return this;
        }

        /** 線種。小版 29 から */
        public LineTypeChange getLineType() {
            return Json.read(json.get("line_type"), LineTypeChange::of);
        }

        /** {@link #getLineType()} を書く。null なら欄を除く */
        public AddEdit withLineType(LineTypeChange value) {
            put("line_type", value);
            return this;
        }

        /** 線幅。小版 29 から */
        public LineWeightChange getLineWeight() {
            return Json.read(json.get("line_weight"), LineWeightChange::of);
        }

        /** {@link #getLineWeight()} を書く。null なら欄を除く */
        public AddEdit withLineWeight(LineWeightChange value) {
            put("line_weight", value);
            return this;
        }

        /** 足す形 */
        public Geometry getGeometry() {
            return Json.read(json.get("geometry"), Geometry::of);
        }

        /** {@link #getGeometry()} を書く。null なら欄を除く */
        public AddEdit withGeometry(Geometry value) {
            put("geometry", value);
            return this;
        }
    }

    /** レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG は、今あるレイヤの線種と真の色を変えられない。小版 22 から */
    public static final class LayerEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LayerEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LayerEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LayerEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public LayerEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "layer");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public LayerEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 変えるレイヤの ID。tables が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public LayerEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときのレイヤのハッシュ。tables が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public LayerEdit withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 新しい名前。ほかのレイヤと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。0 と Defpoints の名前は変えられない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public LayerEdit withName(String value) {
            put("name", value);
            return this;
        }

        /** 表示する。凍結するなら偽 */
        public Boolean getVisible() {
            return Json.read(json.get("visible"), Json::bool);
        }

        /** {@link #getVisible()} を書く。null なら欄を除く */
        public LayerEdit withVisible(Boolean value) {
            put("visible", value);
            return this;
        }

        /** 印刷する */
        public Boolean getPrintable() {
            return Json.read(json.get("printable"), Json::bool);
        }

        /** {@link #getPrintable()} を書く。null なら欄を除く */
        public LayerEdit withPrintable(Boolean value) {
            put("printable", value);
            return this;
        }

        /** 編集できる（ロックしない） */
        public Boolean getEditable() {
            return Json.read(json.get("editable"), Json::bool);
        }

        /** {@link #getEditable()} を書く。null なら欄を除く */
        public LayerEdit withEditable(Boolean value) {
            put("editable", value);
            return this;
        }

        /** 凍結する */
        public Boolean getFrozen() {
            return Json.read(json.get("frozen"), Json::bool);
        }

        /** {@link #getFrozen()} を書く。null なら欄を除く */
        public LayerEdit withFrozen(Boolean value) {
            put("frozen", value);
            return this;
        }

        /** 色。aci が要る。inherit は書けない */
        public ColorChange getColor() {
            return Json.read(json.get("color"), ColorChange::of);
        }

        /** {@link #getColor()} を書く。null なら欄を除く */
        public LayerEdit withColor(ColorChange value) {
            put("color", value);
            return this;
        }

        /** 線種。name か id で指す。inherit は書けない */
        public LineTypeChange getLineType() {
            return Json.read(json.get("line_type"), LineTypeChange::of);
        }

        /** {@link #getLineType()} を書く。null なら欄を除く */
        public LayerEdit withLineType(LineTypeChange value) {
            put("line_type", value);
            return this;
        }

        /** 線幅。mm か default。inherit は書けない */
        public LineWeightChange getLineWeight() {
            return Json.read(json.get("line_weight"), LineWeightChange::of);
        }

        /** {@link #getLineWeight()} を書く。null なら欄を除く */
        public LayerEdit withLineWeight(LineWeightChange value) {
            put("line_weight", value);
            return this;
        }
    }

    /** レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。小版 24 から */
    public static final class RemoveLayerEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public RemoveLayerEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static RemoveLayerEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new RemoveLayerEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public RemoveLayerEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "remove_layer");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public RemoveLayerEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 除くレイヤの ID。tables が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public RemoveLayerEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときのレイヤのハッシュ。tables が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public RemoveLayerEdit withHash(String value) {
            put("hash", value);
            return this;
        }
    }

    /** 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。小版 25 から */
    public static final class TextStyleEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public TextStyleEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static TextStyleEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new TextStyleEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public TextStyleEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "text_style");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public TextStyleEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 変える文字スタイルの ID。tables が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public TextStyleEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの文字スタイルのハッシュ。tables が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public TextStyleEdit withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 新しい名前。ほかの文字スタイルと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。Standard の名前は変えられない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public TextStyleEdit withName(String value) {
            put("name", value);
            return this;
        }

        /** 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない */
        public Font getFont() {
            return Json.read(json.get("font"), Font::of);
        }

        /** {@link #getFont()} を書く。null なら欄を除く */
        public TextStyleEdit withFont(Font value) {
            put("font", value);
            return this;
        }

        /** 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く */
        public Font getBigFont() {
            return Json.read(json.get("big_font"), Font::of);
        }

        /** {@link #getBigFont()} を書く。null なら欄を除く */
        public TextStyleEdit withBigFont(Font value) {
            put("big_font", value);
            return this;
        }

        /** 決まった高さ。文書の単位。0 なら決めない。負の数は書けない */
        public Double getHeight() {
            return Json.read(json.get("height"), Json::number);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public TextStyleEdit withHeight(Double value) {
            put("height", value);
            return this;
        }

        /** 幅の倍率。正の数 */
        public Double getWidth() {
            return Json.read(json.get("width"), Json::number);
        }

        /** {@link #getWidth()} を書く。null なら欄を除く */
        public TextStyleEdit withWidth(Double value) {
            put("width", value);
            return this;
        }

        /** 傾き（度）。-85 から 85 まで */
        public Double getOblique() {
            return Json.read(json.get("oblique"), Json::number);
        }

        /** {@link #getOblique()} を書く。null なら欄を除く */
        public TextStyleEdit withOblique(Double value) {
            put("oblique", value);
            return this;
        }

        /** 左右を裏返す */
        public Boolean getBackward() {
            return Json.read(json.get("backward"), Json::bool);
        }

        /** {@link #getBackward()} を書く。null なら欄を除く */
        public TextStyleEdit withBackward(Boolean value) {
            put("backward", value);
            return this;
        }

        /** 上下を反転する */
        public Boolean getUpsideDown() {
            return Json.read(json.get("upside_down"), Json::bool);
        }

        /** {@link #getUpsideDown()} を書く。null なら欄を除く */
        public TextStyleEdit withUpsideDown(Boolean value) {
            put("upside_down", value);
            return this;
        }

        /** 縦書き */
        public Boolean getVertical() {
            return Json.read(json.get("vertical"), Json::bool);
        }

        /** {@link #getVertical()} を書く。null なら欄を除く */
        public TextStyleEdit withVertical(Boolean value) {
            put("vertical", value);
            return this;
        }
    }

    /** 文字スタイルを足す（決定 0163）。書かない欄は Standard と同じ。同じ apply の要素は、足した文字スタイルを指せない。小版 25 から */
    public static final class AddTextStyleEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public AddTextStyleEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static AddTextStyleEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new AddTextStyleEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public AddTextStyleEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "add_text_style");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public AddTextStyleEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 名前。text_style の name と同じ決まり */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public AddTextStyleEdit withName(String value) {
            put("name", value);
            return this;
        }

        /** 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない */
        public Font getFont() {
            return Json.read(json.get("font"), Font::of);
        }

        /** {@link #getFont()} を書く。null なら欄を除く */
        public AddTextStyleEdit withFont(Font value) {
            put("font", value);
            return this;
        }

        /** 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く */
        public Font getBigFont() {
            return Json.read(json.get("big_font"), Font::of);
        }

        /** {@link #getBigFont()} を書く。null なら欄を除く */
        public AddTextStyleEdit withBigFont(Font value) {
            put("big_font", value);
            return this;
        }

        /** 決まった高さ。文書の単位。0 なら決めない。負の数は書けない */
        public Double getHeight() {
            return Json.read(json.get("height"), Json::number);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public AddTextStyleEdit withHeight(Double value) {
            put("height", value);
            return this;
        }

        /** 幅の倍率。正の数 */
        public Double getWidth() {
            return Json.read(json.get("width"), Json::number);
        }

        /** {@link #getWidth()} を書く。null なら欄を除く */
        public AddTextStyleEdit withWidth(Double value) {
            put("width", value);
            return this;
        }

        /** 傾き（度）。-85 から 85 まで */
        public Double getOblique() {
            return Json.read(json.get("oblique"), Json::number);
        }

        /** {@link #getOblique()} を書く。null なら欄を除く */
        public AddTextStyleEdit withOblique(Double value) {
            put("oblique", value);
            return this;
        }

        /** 左右を裏返す */
        public Boolean getBackward() {
            return Json.read(json.get("backward"), Json::bool);
        }

        /** {@link #getBackward()} を書く。null なら欄を除く */
        public AddTextStyleEdit withBackward(Boolean value) {
            put("backward", value);
            return this;
        }

        /** 上下を反転する */
        public Boolean getUpsideDown() {
            return Json.read(json.get("upside_down"), Json::bool);
        }

        /** {@link #getUpsideDown()} を書く。null なら欄を除く */
        public AddTextStyleEdit withUpsideDown(Boolean value) {
            put("upside_down", value);
            return this;
        }

        /** 縦書き */
        public Boolean getVertical() {
            return Json.read(json.get("vertical"), Json::bool);
        }

        /** {@link #getVertical()} を書く。null なら欄を除く */
        public AddTextStyleEdit withVertical(Boolean value) {
            put("vertical", value);
            return this;
        }
    }

    /** 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。小版 25 から */
    public static final class RemoveTextStyleEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public RemoveTextStyleEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static RemoveTextStyleEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new RemoveTextStyleEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public RemoveTextStyleEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "remove_text_style");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public RemoveTextStyleEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 除く文字スタイルの ID。tables が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public RemoveTextStyleEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの文字スタイルのハッシュ。tables が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public RemoveTextStyleEdit withHash(String value) {
            put("hash", value);
            return this;
        }
    }

    /** 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。多重線のスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。小版 26 から */
    public static final class LineTypeEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LineTypeEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LineTypeEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LineTypeEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public LineTypeEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "line_type");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public LineTypeEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 変える線種の ID。tables が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public LineTypeEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの線種のハッシュ。tables が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public LineTypeEdit withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 新しい名前。ほかの線種と重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。ByLayer・ByBlock・Continuous と外部参照に依る線種の名前は変えられない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public LineTypeEdit withName(String value) {
            put("name", value);
            return this;
        }

        /** 説明 */
        public String getDescription() {
            return Json.read(json.get("description"), Json::string);
        }

        /** {@link #getDescription()} を書く。null なら欄を除く */
        public LineTypeEdit withDescription(String value) {
            put("description", value);
            return this;
        }

        /** 線と間の長さの列。文書の単位。正は線、負は間、0 は点 */
        public List<Double> getPattern() {
            return Json.read(json.get("pattern"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getPattern()} を書く。null なら欄を除く */
        public LineTypeEdit withPattern(List<Double> value) {
            put("pattern", value);
            return this;
        }
    }

    /** 線種を足す（決定 0164）。書かない欄は Continuous と同じ。同じ apply の要素とレイヤは、足した線種を指せない。小版 26 から */
    public static final class AddLineTypeEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public AddLineTypeEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static AddLineTypeEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new AddLineTypeEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public AddLineTypeEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "add_line_type");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public AddLineTypeEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 名前。line_type の name と同じ決まり */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public AddLineTypeEdit withName(String value) {
            put("name", value);
            return this;
        }

        /** 説明 */
        public String getDescription() {
            return Json.read(json.get("description"), Json::string);
        }

        /** {@link #getDescription()} を書く。null なら欄を除く */
        public AddLineTypeEdit withDescription(String value) {
            put("description", value);
            return this;
        }

        /** 線と間の長さの列。文書の単位。正は線、負は間、0 は点 */
        public List<Double> getPattern() {
            return Json.read(json.get("pattern"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getPattern()} を書く。null なら欄を除く */
        public AddLineTypeEdit withPattern(List<Double> value) {
            put("pattern", value);
            return this;
        }
    }

    /** 線種を除く（決定 0164）。要素・レイヤ・多重線のスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。小版 26 から */
    public static final class RemoveLineTypeEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public RemoveLineTypeEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static RemoveLineTypeEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new RemoveLineTypeEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public RemoveLineTypeEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "remove_line_type");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public RemoveLineTypeEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 除く線種の ID。tables が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public RemoveLineTypeEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの線種のハッシュ。tables が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public RemoveLineTypeEdit withHash(String value) {
            put("hash", value);
            return this;
        }
    }

    /** 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。小版 27 から */
    public static final class DimensionStyleEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DimensionStyleEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DimensionStyleEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DimensionStyleEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public DimensionStyleEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "dimension_style");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public DimensionStyleEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 変える寸法スタイルの ID。tables が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public DimensionStyleEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの寸法スタイルのハッシュ。tables が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public DimensionStyleEdit withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 新しい名前。ほかの寸法スタイルと重ならず（大文字と小文字を区別しない）、$ と < > / \ " : ; ? * | = ` を含まないもの。Standard・子の寸法スタイル（名前に $ を持つもの）・子を持つ寸法スタイル・外部参照に依る寸法スタイルの名前は変えられない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public DimensionStyleEdit withName(String value) {
            put("name", value);
            return this;
        }

        /** 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない */
        public List<String> getEnds() {
            return Json.read(json.get("ends"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getEnds()} を書く。null なら欄を除く */
        public DimensionStyleEdit withEnds(List<String> value) {
            put("ends", value);
            return this;
        }

        /** 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり */
        public String getLeader() {
            return Json.read(json.get("leader"), Json::string);
        }

        /** {@link #getLeader()} を書く。null なら欄を除く */
        public DimensionStyleEdit withLeader(String value) {
            put("leader", value);
            return this;
        }

        /** 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない */
        public Double getSize() {
            return Json.read(json.get("size"), Json::number);
        }

        /** {@link #getSize()} を書く。null なら欄を除く */
        public DimensionStyleEdit withSize(Double value) {
            put("size", value);
            return this;
        }

        /** 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない */
        public Double getTick() {
            return Json.read(json.get("tick"), Json::number);
        }

        /** {@link #getTick()} を書く。null なら欄を除く */
        public DimensionStyleEdit withTick(Double value) {
            put("tick", value);
            return this;
        }

        /** 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない */
        public Double getScale() {
            return Json.read(json.get("scale"), Json::number);
        }

        /** {@link #getScale()} を書く。null なら欄を除く */
        public DimensionStyleEdit withScale(Double value) {
            put("scale", value);
            return this;
        }
    }

    /** 寸法スタイルを足す（決定 0165）。書かない欄は Standard と同じ。同じ apply の要素は、足した寸法スタイルを指せない。小版 27 から */
    public static final class AddDimensionStyleEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public AddDimensionStyleEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static AddDimensionStyleEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new AddDimensionStyleEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public AddDimensionStyleEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "add_dimension_style");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public AddDimensionStyleEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 名前。dimension_style の name と同じ決まり */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public AddDimensionStyleEdit withName(String value) {
            put("name", value);
            return this;
        }

        /** 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない */
        public List<String> getEnds() {
            return Json.read(json.get("ends"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getEnds()} を書く。null なら欄を除く */
        public AddDimensionStyleEdit withEnds(List<String> value) {
            put("ends", value);
            return this;
        }

        /** 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり */
        public String getLeader() {
            return Json.read(json.get("leader"), Json::string);
        }

        /** {@link #getLeader()} を書く。null なら欄を除く */
        public AddDimensionStyleEdit withLeader(String value) {
            put("leader", value);
            return this;
        }

        /** 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない */
        public Double getSize() {
            return Json.read(json.get("size"), Json::number);
        }

        /** {@link #getSize()} を書く。null なら欄を除く */
        public AddDimensionStyleEdit withSize(Double value) {
            put("size", value);
            return this;
        }

        /** 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない */
        public Double getTick() {
            return Json.read(json.get("tick"), Json::number);
        }

        /** {@link #getTick()} を書く。null なら欄を除く */
        public AddDimensionStyleEdit withTick(Double value) {
            put("tick", value);
            return this;
        }

        /** 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない */
        public Double getScale() {
            return Json.read(json.get("scale"), Json::number);
        }

        /** {@link #getScale()} を書く。null なら欄を除く */
        public AddDimensionStyleEdit withScale(Double value) {
            put("scale", value);
            return this;
        }
    }

    /** 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。小版 27 から */
    public static final class RemoveDimensionStyleEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public RemoveDimensionStyleEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static RemoveDimensionStyleEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new RemoveDimensionStyleEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public RemoveDimensionStyleEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "remove_dimension_style");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public RemoveDimensionStyleEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 除く寸法スタイルの ID。tables が返したもの */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public RemoveDimensionStyleEdit withId(String value) {
            put("id", value);
            return this;
        }

        /** 読んだときの寸法スタイルのハッシュ。tables が返したもの */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public RemoveDimensionStyleEdit withHash(String value) {
            put("hash", value);
            return this;
        }
    }

    /** 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG は見出しの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、小版 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。小版 38 から */
    public static final class PointStyleEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PointStyleEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PointStyleEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PointStyleEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public PointStyleEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "point_style");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public PointStyleEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 基本の形 */
        public String getMark() {
            return Json.read(json.get("mark"), Json::string);
        }

        /** {@link #getMark()} を書く。null なら欄を除く */
        public PointStyleEdit withMark(String value) {
            put("mark", value);
            return this;
        }

        /** 円で囲む。書かなければ偽 */
        public Boolean getCircle() {
            return Json.read(json.get("circle"), Json::bool);
        }

        /** {@link #getCircle()} を書く。null なら欄を除く */
        public PointStyleEdit withCircle(Boolean value) {
            put("circle", value);
            return this;
        }

        /** 四角で囲む。書かなければ偽 */
        public Boolean getSquare() {
            return Json.read(json.get("square"), Json::bool);
        }

        /** {@link #getSquare()} を書く。null なら欄を除く */
        public PointStyleEdit withSquare(Boolean value) {
            put("square", value);
            return this;
        }

        /** 大きさ。0 より大きい有限の数。意味は size_kind で決まる */
        public Double getSize() {
            return Json.read(json.get("size"), Json::number);
        }

        /** {@link #getSize()} を書く。null なら欄を除く */
        public PointStyleEdit withSize(Double value) {
            put("size", value);
            return this;
        }

        /** 大きさの意味 */
        public String getSizeKind() {
            return Json.read(json.get("size_kind"), Json::string);
        }

        /** {@link #getSizeKind()} を書く。null なら欄を除く */
        public PointStyleEdit withSizeKind(String value) {
            put("size_kind", value);
            return this;
        }
    }

    /**
     * 文書の点の基本の形。dot は小さな点、none はなし、plus は十字、cross はバツ、tick は上向きの短い線
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class PointMark {
        private PointMark() {}

        public static final String DOT = "dot";

        public static final String NONE = "none";

        public static final String PLUS = "plus";

        public static final String CROSS = "cross";

        public static final String TICK = "tick";
    }

    /**
     * 文書の点の大きさの意味。length は点のある空間の長さ、screen は画面の高さに対する百分率
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class PointSizeKind {
        private PointSizeKind() {}

        public static final String LENGTH = "length";

        public static final String SCREEN = "screen";
    }

    /** レイヤを足す（決定 0162）。書かない欄は、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の要素は、足したレイヤを指せない。小版 22 から */
    public static final class AddLayerEdit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public AddLayerEdit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static AddLayerEdit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new AddLayerEdit(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public AddLayerEdit() {
            super(new LinkedHashMap<String, Object>());
            json.put("kind", "add_layer");
        }

        /** 変更の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public AddLayerEdit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 名前。layer の name と同じ決まり */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public AddLayerEdit withName(String value) {
            put("name", value);
            return this;
        }

        /** 表示する。凍結するなら偽 */
        public Boolean getVisible() {
            return Json.read(json.get("visible"), Json::bool);
        }

        /** {@link #getVisible()} を書く。null なら欄を除く */
        public AddLayerEdit withVisible(Boolean value) {
            put("visible", value);
            return this;
        }

        /** 印刷する */
        public Boolean getPrintable() {
            return Json.read(json.get("printable"), Json::bool);
        }

        /** {@link #getPrintable()} を書く。null なら欄を除く */
        public AddLayerEdit withPrintable(Boolean value) {
            put("printable", value);
            return this;
        }

        /** 編集できる（ロックしない） */
        public Boolean getEditable() {
            return Json.read(json.get("editable"), Json::bool);
        }

        /** {@link #getEditable()} を書く。null なら欄を除く */
        public AddLayerEdit withEditable(Boolean value) {
            put("editable", value);
            return this;
        }

        /** 凍結する */
        public Boolean getFrozen() {
            return Json.read(json.get("frozen"), Json::bool);
        }

        /** {@link #getFrozen()} を書く。null なら欄を除く */
        public AddLayerEdit withFrozen(Boolean value) {
            put("frozen", value);
            return this;
        }

        /** 色。aci が要る。inherit は書けない */
        public ColorChange getColor() {
            return Json.read(json.get("color"), ColorChange::of);
        }

        /** {@link #getColor()} を書く。null なら欄を除く */
        public AddLayerEdit withColor(ColorChange value) {
            put("color", value);
            return this;
        }

        /** 線種。name か id で指す。inherit は書けない */
        public LineTypeChange getLineType() {
            return Json.read(json.get("line_type"), LineTypeChange::of);
        }

        /** {@link #getLineType()} を書く。null なら欄を除く */
        public AddLayerEdit withLineType(LineTypeChange value) {
            put("line_type", value);
            return this;
        }

        /** 線幅。mm か default。inherit は書けない */
        public LineWeightChange getLineWeight() {
            return Json.read(json.get("line_weight"), LineWeightChange::of);
        }

        /** {@link #getLineWeight()} を書く。null なら欄を除く */
        public AddLayerEdit withLineWeight(LineWeightChange value) {
            put("line_weight", value);
            return this;
        }
    }

    /**
     * 値を継承する先。layer はレイヤから、block は要素を置いた記号の配置から
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class Inherit {
        private Inherit() {}

        public static final String LAYER = "layer";

        public static final String BLOCK = "block";
    }

    /** 新しい色。inherit か、aci・sxf・jww・rgb の少なくとも 1 つのどちらかを書く。aci・sxf・jww は 1 つだけ書く */
    public static final class ColorChange extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ColorChange(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ColorChange of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ColorChange(json);
        }

        /** 空のオブジェクトから作る */
        public ColorChange() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 継承する先 */
        public String getInherit() {
            return Json.read(json.get("inherit"), Json::string);
        }

        /** {@link #getInherit()} を書く。null なら欄を除く */
        public ColorChange withInherit(String value) {
            put("inherit", value);
            return this;
        }

        /** AutoCAD の色の番号（1〜255） */
        public Long getAci() {
            return Json.read(json.get("aci"), Json::integer);
        }

        /** {@link #getAci()} を書く。null なら欄を除く */
        public ColorChange withAci(Long value) {
            put("aci", value);
            return this;
        }

        /** SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。小版 29 から */
        public Long getSxf() {
            return Json.read(json.get("sxf"), Json::integer);
        }

        /** {@link #getSxf()} を書く。null なら欄を除く */
        public ColorChange withSxf(Long value) {
            put("sxf", value);
            return this;
        }

        /** Jw_cad のペンの番号。0〜9 と、版 420 からの SXF の拡張色 100〜356。小版 30 から */
        public Long getJww() {
            return Json.read(json.get("jww"), Json::integer);
        }

        /** {@link #getJww()} を書く。null なら欄を除く */
        public ColorChange withJww(Long value) {
            put("jww", value);
            return this;
        }

        /** [赤, 緑, 青]。それぞれ 0〜255 */
        public List<Long> getRgb() {
            return Json.read(json.get("rgb"), v0 -> Json.list(v0, Json::integer));
        }

        /** {@link #getRgb()} を書く。null なら欄を除く */
        public ColorChange withRgb(List<Long> value) {
            put("rgb", value);
            return this;
        }
    }

    /** 新しい線種。inherit・name・id のどれか 1 つを書く */
    public static final class LineTypeChange extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LineTypeChange(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LineTypeChange of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LineTypeChange(json);
        }

        /** 空のオブジェクトから作る */
        public LineTypeChange() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 継承する先 */
        public String getInherit() {
            return Json.read(json.get("inherit"), Json::string);
        }

        /** {@link #getInherit()} を書く。null なら欄を除く */
        public LineTypeChange withInherit(String value) {
            put("inherit", value);
            return this;
        }

        /** 線種の元の名前。図面にある線種で、名前が 1 つに決まるもの */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public LineTypeChange withName(String value) {
            put("name", value);
            return this;
        }

        /** 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public LineTypeChange withId(String value) {
            put("id", value);
            return this;
        }
    }

    /** 新しい線幅。inherit・mm・default のどれか 1 つを書く */
    public static final class LineWeightChange extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LineWeightChange(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LineWeightChange of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LineWeightChange(json);
        }

        /** 空のオブジェクトから作る */
        public LineWeightChange() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 継承する先 */
        public String getInherit() {
            return Json.read(json.get("inherit"), Json::string);
        }

        /** {@link #getInherit()} を書く。null なら欄を除く */
        public LineWeightChange withInherit(String value) {
            put("inherit", value);
            return this;
        }

        /** 用紙の上の幅（mm）。0 以上 */
        public Double getMm() {
            return Json.read(json.get("mm"), Json::number);
        }

        /** {@link #getMm()} を書く。null なら欄を除く */
        public LineWeightChange withMm(Double value) {
            put("mm", value);
            return this;
        }

        /** true なら、文書の既定の線幅 */
        public Boolean getDefault() {
            return Json.read(json.get("default"), Json::bool);
        }

        /** {@link #getDefault()} を書く。null なら欄を除く */
        public LineWeightChange withDefault(Boolean value) {
            put("default", value);
            return this;
        }
    }

    /** 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から */
    public static final class TablesRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public TablesRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static TablesRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new TablesRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public TablesRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "tables");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public TablesRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public TablesRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public TablesRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public TablesRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public TablesRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public TablesRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で作ったもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public TablesRequest withDoc(String value) {
            put("doc", value);
            return this;
        }
    }

    /** 表の項目。図面モデルの順 */
    public static final class TablesResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public TablesResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static TablesResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new TablesResult(json);
        }

        /** 空のオブジェクトから作る */
        public TablesResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** レイヤ */
        public List<LayerItem> getLayers() {
            return Json.read(json.get("layers"), v0 -> Json.list(v0, LayerItem::of));
        }

        /** {@link #getLayers()} を書く。null なら欄を除く */
        public TablesResult withLayers(List<LayerItem> value) {
            put("layers", value);
            return this;
        }

        /** 線種。小版 26 から、定義を持つ */
        public List<LineTypeItem> getLineTypes() {
            return Json.read(json.get("line_types"), v0 -> Json.list(v0, LineTypeItem::of));
        }

        /** {@link #getLineTypes()} を書く。null なら欄を除く */
        public TablesResult withLineTypes(List<LineTypeItem> value) {
            put("line_types", value);
            return this;
        }

        /** 寸法スタイル。小版 27 から、端部記号と大きさを持つ */
        public List<DimensionStyleItem> getDimensionStyles() {
            return Json.read(json.get("dimension_styles"), v0 -> Json.list(v0, DimensionStyleItem::of));
        }

        /** {@link #getDimensionStyles()} を書く。null なら欄を除く */
        public TablesResult withDimensionStyles(List<DimensionStyleItem> value) {
            put("dimension_styles", value);
            return this;
        }

        /** 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義の欄を持たない項目として後ろに並ぶ。小版 20 から */
        public List<TextStyleItem> getTextStyles() {
            return Json.read(json.get("text_styles"), v0 -> Json.list(v0, TextStyleItem::of));
        }

        /** {@link #getTextStyles()} を書く。null なら欄を除く */
        public TablesResult withTextStyles(List<TextStyleItem> value) {
            put("text_styles", value);
            return this;
        }
    }

    /** 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。小版 33 から */
    public static final class SceneRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SceneRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SceneRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SceneRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public SceneRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "scene");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public SceneRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public SceneRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public SceneRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public SceneRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public SceneRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public SceneRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で作ったもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public SceneRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 起点。実寸の空間・記号の定義の空間・用紙の ID（drawing と entities が返すもの） */
        public String getFrom() {
            return Json.read(json.get("from"), Json::string);
        }

        /** {@link #getFrom()} を書く。null なら欄を除く */
        public SceneRequest withFrom(String value) {
            put("from", value);
            return this;
        }

        /** 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した空間の要素だけを描く。from が用紙のときだけ書ける */
        public Long getView() {
            return Json.read(json.get("view"), Json::integer);
        }

        /** {@link #getView()} を書く。null なら欄を除く */
        public SceneRequest withView(Long value) {
            put("view", value);
            return this;
        }

        /** [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す */
        public List<Double> getRegion() {
            return Json.read(json.get("region"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getRegion()} を書く。null なら欄を除く */
        public SceneRequest withRegion(List<Double> value) {
            put("region", value);
            return this;
        }

        /** 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000 */
        public Double getTolerance() {
            return Json.read(json.get("tolerance"), Json::number);
        }

        /** {@link #getTolerance()} を書く。null なら欄を除く */
        public SceneRequest withTolerance(Double value) {
            put("tolerance", value);
            return this;
        }

        /** true なら印刷する要素を、書かないか false なら表示する要素を描く */
        public Boolean getPrint() {
            return Json.read(json.get("print"), Json::bool);
        }

        /** {@link #getPrint()} を書く。null なら欄を除く */
        public SceneRequest withPrint(Boolean value) {
            put("print", value);
            return this;
        }

        /** 色の表の入力の名前（zm_push で渡したもの）。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決の印を立てる */
        public String getPalette() {
            return Json.read(json.get("palette"), Json::string);
        }

        /** {@link #getPalette()} を書く。null なら欄を除く */
        public SceneRequest withPalette(String value) {
            put("palette", value);
            return this;
        }

        /** 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形の区画に形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。小版 37 から */
        public List<SceneFont> getFonts() {
            return Json.read(json.get("fonts"), v0 -> Json.list(v0, SceneFont::of));
        }

        /** {@link #getFonts()} を書く。null なら欄を除く */
        public SceneRequest withFonts(List<SceneFont> value) {
            put("fonts", value);
            return this;
        }

        /** 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは大文字の高さとみなす。fonts を書いたときだけ使う。小版 37 から */
        public Map<String, String> getHeights() {
            return Json.read(json.get("heights"), v0 -> Json.map(v0, Json::string));
        }

        /** {@link #getHeights()} を書く。null なら欄を除く */
        public SceneRequest withHeights(Map<String, String> value) {
            put("heights", value);
            return this;
        }
    }

    /** シーンの文字を置くフォント（決定 0188） */
    public static final class SceneFont extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SceneFont(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SceneFont of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SceneFont(json);
        }

        /** 空のオブジェクトから作る */
        public SceneFont() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 文字が指すフォントの名前（ファイルに書かれた元の名前）。書かなければ、名前の合わない文字に使う既定のフォント。同じ名前と、名前のないものは 1 つまで */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public SceneFont withName(String value) {
            put("name", value);
            return this;
        }

        /** TrueType・OpenType（CFF・CFF2 を含む）のファイルの入力の名前（zm_push で渡したもの） */
        public String getInput() {
            return Json.read(json.get("input"), Json::string);
        }

        /** {@link #getInput()} を書く。null なら欄を除く */
        public SceneFont withInput(String value) {
            put("input", value);
            return this;
        }

        /** フォントのコレクション（TTC）の中の番号。書かなければ 0 */
        public Long getIndex() {
            return Json.read(json.get("index"), Json::integer);
        }

        /** {@link #getIndex()} を書く。null なら欄を除く */
        public SceneFont withIndex(Long value) {
            put("index", value);
            return this;
        }
    }

    /**
     * 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class HeightReference {
        private HeightReference() {}

        public static final String CAP_HEIGHT = "cap_height";

        public static final String EM = "em";

        public static final String FRAME = "frame";
    }

    /** シーンの要約と、シーンの blob の番号 */
    public static final class SceneResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SceneResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SceneResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SceneResult(json);
        }

        /** 空のオブジェクトから作る */
        public SceneResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 起点の ID */
        public String getFrom() {
            return Json.read(json.get("from"), Json::string);
        }

        /** {@link #getFrom()} を書く。null なら欄を除く */
        public SceneResult withFrom(String value) {
            put("from", value);
            return this;
        }

        /** 起点のビューの番号。ビューでなければ持たない */
        public Long getView() {
            return Json.read(json.get("view"), Json::integer);
        }

        /** {@link #getView()} を書く。null なら欄を除く */
        public SceneResult withView(Long value) {
            put("view", value);
            return this;
        }

        /** 起点の座標の単位。用紙とビューからは millimeter */
        public String getUnit() {
            return Json.read(json.get("unit"), Json::string);
        }

        /** {@link #getUnit()} を書く。null なら欄を除く */
        public SceneResult withUnit(String value) {
            put("unit", value);
            return this;
        }

        /** 使った弦の誤差（起点の長さ） */
        public Double getTolerance() {
            return Json.read(json.get("tolerance"), Json::number);
        }

        /** {@link #getTolerance()} を書く。null なら欄を除く */
        public SceneResult withTolerance(Double value) {
            put("tolerance", value);
            return this;
        }

        /** 返した描くものの数 */
        public SceneCounts getCounts() {
            return Json.read(json.get("counts"), SceneCounts::of);
        }

        /** {@link #getCounts()} を書く。null なら欄を除く */
        public SceneResult withCounts(SceneCounts value) {
            put("counts", value);
            return this;
        }

        /** 返した塊の範囲。起点の座標。塊がなければ持たない */
        public Extent getExtent() {
            return Json.read(json.get("extent"), Extent::of);
        }

        /** {@link #getExtent()} を書く。null なら欄を除く */
        public SceneResult withExtent(Extent value) {
            put("extent", value);
            return this;
        }

        /** シーンの blob の番号（種類は scene）。zm_take で取り出す */
        public Long getBlob() {
            return Json.read(json.get("blob"), Json::integer);
        }

        /** {@link #getBlob()} を書く。null なら欄を除く */
        public SceneResult withBlob(Long value) {
            put("blob", value);
            return this;
        }
    }

    /** シーンの描くものの種類ごとの数 */
    public static final class SceneCounts extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SceneCounts(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SceneCounts of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SceneCounts(json);
        }

        /** 空のオブジェクトから作る */
        public SceneCounts() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 塊 */
        public Long getChunks() {
            return Json.read(json.get("chunks"), Json::integer);
        }

        /** {@link #getChunks()} を書く。null なら欄を除く */
        public SceneCounts withChunks(Long value) {
            put("chunks", value);
            return this;
        }

        /** 線（折れ線） */
        public Long getLines() {
            return Json.read(json.get("lines"), Json::integer);
        }

        /** {@link #getLines()} を書く。null なら欄を除く */
        public SceneCounts withLines(Long value) {
            put("lines", value);
            return this;
        }

        /** 線と面の頂点 */
        public Long getVertices() {
            return Json.read(json.get("vertices"), Json::integer);
        }

        /** {@link #getVertices()} を書く。null なら欄を除く */
        public SceneCounts withVertices(Long value) {
            put("vertices", value);
            return this;
        }

        /** 面 */
        public Long getFaces() {
            return Json.read(json.get("faces"), Json::integer);
        }

        /** {@link #getFaces()} を書く。null なら欄を除く */
        public SceneCounts withFaces(Long value) {
            put("faces", value);
            return this;
        }

        /** 文字 */
        public Long getTexts() {
            return Json.read(json.get("texts"), Json::integer);
        }

        /** {@link #getTexts()} を書く。null なら欄を除く */
        public SceneCounts withTexts(Long value) {
            put("texts", value);
            return this;
        }

        /** 点 */
        public Long getPoints() {
            return Json.read(json.get("points"), Json::integer);
        }

        /** {@link #getPoints()} を書く。null なら欄を除く */
        public SceneCounts withPoints(Long value) {
            put("points", value);
            return this;
        }

        /** 画像 */
        public Long getImages() {
            return Json.read(json.get("images"), Json::integer);
        }

        /** {@link #getImages()} を書く。null なら欄を除く */
        public SceneCounts withImages(Long value) {
            put("images", value);
            return this;
        }

        /** 置いた字形。fonts を書かなければ 0。小版 37 から */
        public Long getGlyphs() {
            return Json.read(json.get("glyphs"), Json::integer);
        }

        /** {@link #getGlyphs()} を書く。null なら欄を除く */
        public SceneCounts withGlyphs(Long value) {
            put("glyphs", value);
            return this;
        }
    }

    /** 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。小版 34 から */
    public static final class CheckRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CheckRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CheckRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CheckRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public CheckRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "check");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public CheckRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public CheckRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public CheckRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public CheckRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public CheckRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public CheckRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で作ったもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public CheckRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す */
        public List<String> getProfiles() {
            return Json.read(json.get("profiles"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getProfiles()} を書く。null なら欄を除く */
        public CheckRequest withProfiles(List<String> value) {
            put("profiles", value);
            return this;
        }

        /** 照合に使うプロファイル。profiles の中から選ぶ */
        public ProfileRef getProfile() {
            return Json.read(json.get("profile"), ProfileRef::of);
        }

        /** {@link #getProfile()} を書く。null なら欄を除く */
        public CheckRequest withProfile(ProfileRef value) {
            put("profile", value);
            return this;
        }

        /** たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる */
        public String getFrom() {
            return Json.read(json.get("from"), Json::string);
        }

        /** {@link #getFrom()} を書く。null なら欄を除く */
        public CheckRequest withFrom(String value) {
            put("from", value);
            return this;
        }

        /** 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける */
        public Long getView() {
            return Json.read(json.get("view"), Json::integer);
        }

        /** {@link #getView()} を書く。null なら欄を除く */
        public CheckRequest withView(Long value) {
            put("view", value);
            return this;
        }

        /** 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から */
        public String getFileName() {
            return Json.read(json.get("file_name"), Json::string);
        }

        /** {@link #getFileName()} を書く。null なら欄を除く */
        public CheckRequest withFileName(String value) {
            put("file_name", value);
            return this;
        }

        /** 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0 */
        public Long getOffset() {
            return Json.read(json.get("offset"), Json::integer);
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public CheckRequest withOffset(Long value) {
            put("offset", value);
            return this;
        }

        /** 返す指摘の数の上限。書かなければ 1000 */
        public Long getLimit() {
            return Json.read(json.get("limit"), Json::integer);
        }

        /** {@link #getLimit()} を書く。null なら欄を除く */
        public CheckRequest withLimit(Long value) {
            put("limit", value);
            return this;
        }

        /** 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く */
        public String getPalette() {
            return Json.read(json.get("palette"), Json::string);
        }

        /** {@link #getPalette()} を書く。null なら欄を除く */
        public CheckRequest withPalette(String value) {
            put("palette", value);
            return this;
        }
    }

    /** プロファイルの識別（決定 0138） */
    public static final class ProfileRef extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ProfileRef(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ProfileRef of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ProfileRef(json);
        }

        /** 空のオブジェクトから作る */
        public ProfileRef() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 基準の系統（mlit-cad など） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public ProfileRef withId(String value) {
            put("id", value);
            return this;
        }

        /** 基準の版（2025-12 など） */
        public String getEdition() {
            return Json.read(json.get("edition"), Json::string);
        }

        /** {@link #getEdition()} を書く。null なら欄を除く */
        public ProfileRef withEdition(String value) {
            put("edition", value);
            return this;
        }
    }

    /** 照合の結果。指摘は offset から limit 個まで */
    public static final class CheckResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CheckResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CheckResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CheckResult(json);
        }

        /** 空のオブジェクトから作る */
        public CheckResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 照合に使ったプロファイルの鎖。extends の基から順 */
        public List<ProfileIdentity> getChain() {
            return Json.read(json.get("chain"), v0 -> Json.list(v0, ProfileIdentity::of));
        }

        /** {@link #getChain()} を書く。null なら欄を除く */
        public CheckResult withChain(List<ProfileIdentity> value) {
            put("chain", value);
            return this;
        }

        /** 照合した規則ごとの数。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す */
        public List<CheckRule> getRules() {
            return Json.read(json.get("rules"), v0 -> Json.list(v0, CheckRule::of));
        }

        /** {@link #getRules()} を書く。null なら欄を除く */
        public CheckResult withRules(List<CheckRule> value) {
            put("rules", value);
            return this;
        }

        /** 指摘。規則の順に、規則ごとにたどった順 */
        public List<CheckFinding> getFindings() {
            return Json.read(json.get("findings"), v0 -> Json.list(v0, CheckFinding::of));
        }

        /** {@link #getFindings()} を書く。null なら欄を除く */
        public CheckResult withFindings(List<CheckFinding> value) {
            put("findings", value);
            return this;
        }

        /** 全体の指摘の数 */
        public Long getTotal() {
            return Json.read(json.get("total"), Json::integer);
        }

        /** {@link #getTotal()} を書く。null なら欄を除く */
        public CheckResult withTotal(Long value) {
            put("total", value);
            return this;
        }

        /** 続きの最初の指摘の位置。続きがなければ持たない */
        public Long getNextOffset() {
            return Json.read(json.get("next_offset"), Json::integer);
        }

        /** {@link #getNextOffset()} を書く。null なら欄を除く */
        public CheckResult withNextOffset(Long value) {
            put("next_offset", value);
            return this;
        }
    }

    /** 照合に使ったプロファイルの識別と、データの直し */
    public static final class ProfileIdentity extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ProfileIdentity(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ProfileIdentity of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ProfileIdentity(json);
        }

        /** 空のオブジェクトから作る */
        public ProfileIdentity() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 基準の系統 */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public ProfileIdentity withId(String value) {
            put("id", value);
            return this;
        }

        /** 基準の版 */
        public String getEdition() {
            return Json.read(json.get("edition"), Json::string);
        }

        /** {@link #getEdition()} を書く。null なら欄を除く */
        public ProfileIdentity withEdition(String value) {
            put("edition", value);
            return this;
        }

        /** このデータの直し。1 から */
        public Long getRevision() {
            return Json.read(json.get("revision"), Json::integer);
        }

        /** {@link #getRevision()} を書く。null なら欄を除く */
        public ProfileIdentity withRevision(Long value) {
            put("revision", value);
            return this;
        }
    }

    /** 規則ごとの数 */
    public static final class CheckRule extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CheckRule(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CheckRule of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CheckRule(json);
        }

        /** 空のオブジェクトから作る */
        public CheckRule() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 規則の id */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public CheckRule withId(String value) {
            put("id", value);
            return this;
        }

        /** 照合の種類（layer-name など） */
        public String getCheck() {
            return Json.read(json.get("check"), Json::string);
        }

        /** {@link #getCheck()} を書く。null なら欄を除く */
        public CheckRule withCheck(String value) {
            put("check", value);
            return this;
        }

        /** 指摘の数。照合できなかったものを除く */
        public Long getFindings() {
            return Json.read(json.get("findings"), Json::integer);
        }

        /** {@link #getFindings()} を書く。null なら欄を除く */
        public CheckRule withFindings(Long value) {
            put("findings", value);
            return this;
        }

        /** 照合できなかった数 */
        public Long getUnsupported() {
            return Json.read(json.get("unsupported"), Json::integer);
        }

        /** {@link #getUnsupported()} を書く。null なら欄を除く */
        public CheckRule withUnsupported(Long value) {
            put("unsupported", value);
            return this;
        }
    }

    /** 指摘。diagnostic は診断と同じ形で、describe で文にできる。severity は規則の level で、照合できない値（profile.<種類>.unsupported）は info。count は 1 */
    public static final class CheckFinding extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CheckFinding(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CheckFinding of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CheckFinding(json);
        }

        /** 空のオブジェクトから作る */
        public CheckFinding() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 規則の id */
        public String getRule() {
            return Json.read(json.get("rule"), Json::string);
        }

        /** {@link #getRule()} を書く。null なら欄を除く */
        public CheckFinding withRule(String value) {
            put("rule", value);
            return this;
        }

        /** code・重要度・場所（要素・実効のレイヤ・用紙の ID）・見つけた値 */
        public Diagnostic getDiagnostic() {
            return Json.read(json.get("diagnostic"), Diagnostic::of);
        }

        /** {@link #getDiagnostic()} を書く。null なら欄を除く */
        public CheckFinding withDiagnostic(Diagnostic value) {
            put("diagnostic", value);
            return this;
        }
    }

    /** zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。小版 42 から */
    public static final class ValidateRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ValidateRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ValidateRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ValidateRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public ValidateRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "validate");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public ValidateRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public ValidateRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public ValidateRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public ValidateRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public ValidateRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public ValidateRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** IFC の入力の名前（zm_push で渡した .ifc か .ifczip） */
        public String getInput() {
            return Json.read(json.get("input"), Json::string);
        }

        /** {@link #getInput()} を書く。null なら欄を除く */
        public ValidateRequest withInput(String value) {
            put("input", value);
            return this;
        }

        /** EXPRESS の入力の名前（zm_push で渡した .exp） */
        public String getExpress() {
            return Json.read(json.get("express"), Json::string);
        }

        /** {@link #getExpress()} を書く。null なら欄を除く */
        public ValidateRequest withExpress(String value) {
            put("express", value);
            return this;
        }

        /** 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0 */
        public Long getOffset() {
            return Json.read(json.get("offset"), Json::integer);
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public ValidateRequest withOffset(Long value) {
            put("offset", value);
            return this;
        }

        /** 返す指摘の数の上限。書かなければ 1000 */
        public Long getLimit() {
            return Json.read(json.get("limit"), Json::integer);
        }

        /** {@link #getLimit()} を書く。null なら欄を除く */
        public ValidateRequest withLimit(Long value) {
            put("limit", value);
            return this;
        }
    }

    /** スキーマに照らしたチェックの結果。指摘は offset から limit 個まで。スキーマの名前の違いと評価しない規則は、診断で返す */
    public static final class ValidateResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ValidateResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ValidateResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ValidateResult(json);
        }

        /** 空のオブジェクトから作る */
        public ValidateResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** EXPRESS のスキーマの名前（大文字） */
        public String getSchema() {
            return Json.read(json.get("schema"), Json::string);
        }

        /** {@link #getSchema()} を書く。null なら欄を除く */
        public ValidateResult withSchema(String value) {
            put("schema", value);
            return this;
        }

        /** IFC の FILE_SCHEMA の名前 */
        public String getFileSchema() {
            return Json.read(json.get("file_schema"), Json::string);
        }

        /** {@link #getFileSchema()} を書く。null なら欄を除く */
        public ValidateResult withFileSchema(String value) {
            put("file_schema", value);
            return this;
        }

        /** EXPRESS のバイト列のハッシュ（v2: と 32 桁の 16 進）。どの EXPRESS でチェックしたかを残す */
        public String getExpressHash() {
            return Json.read(json.get("express_hash"), Json::string);
        }

        /** {@link #getExpressHash()} を書く。null なら欄を除く */
        public ValidateResult withExpressHash(String value) {
            put("express_hash", value);
            return this;
        }

        /** 評価しなかった規則（WHERE・UNIQUE・RULE）の数。確認できなかった項目 */
        public Long getRulesNotEvaluated() {
            return Json.read(json.get("rules_not_evaluated"), Json::integer);
        }

        /** {@link #getRulesNotEvaluated()} を書く。null なら欄を除く */
        public ValidateResult withRulesNotEvaluated(Long value) {
            put("rules_not_evaluated", value);
            return this;
        }

        /** 指摘。インスタンスの番号の順に、属性の順。診断と同じ形で、describe で文にできる。count は 1 */
        public List<Diagnostic> getFindings() {
            return Json.read(json.get("findings"), v0 -> Json.list(v0, Diagnostic::of));
        }

        /** {@link #getFindings()} を書く。null なら欄を除く */
        public ValidateResult withFindings(List<Diagnostic> value) {
            put("findings", value);
            return this;
        }

        /** 全体の指摘の数 */
        public Long getTotal() {
            return Json.read(json.get("total"), Json::integer);
        }

        /** {@link #getTotal()} を書く。null なら欄を除く */
        public ValidateResult withTotal(Long value) {
            put("total", value);
            return this;
        }

        /** 続きの最初の指摘の位置。続きがなければ持たない */
        public Long getNextOffset() {
            return Json.read(json.get("next_offset"), Json::integer);
        }

        /** {@link #getNextOffset()} を書く。null なら欄を除く */
        public ValidateResult withNextOffset(Long value) {
            put("next_offset", value);
            return this;
        }
    }

    /** レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書の版へ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元の版のハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。小版 43 から */
    public static final class ReviewCarryRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ReviewCarryRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ReviewCarryRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ReviewCarryRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public ReviewCarryRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "review_carry");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public ReviewCarryRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public ReviewCarryRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public ReviewCarryRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public ReviewCarryRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public ReviewCarryRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public ReviewCarryRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 新しい版の文書の名前。open で作ったもの。小版 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public ReviewCarryRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** レビューの文書の入力の名前（zm_push で渡した JSON） */
        public String getReview() {
            return Json.read(json.get("review"), Json::string);
        }

        /** {@link #getReview()} を書く。null なら欄を除く */
        public ReviewCarryRequest withReview(String value) {
            put("review", value);
            return this;
        }

        /** 移す対象の ID。レビューの文書の targets の id */
        public String getTarget() {
            return Json.read(json.get("target"), Json::string);
        }

        /** {@link #getTarget()} を書く。null なら欄を除く */
        public ReviewCarryRequest withTarget(String value) {
            put("target", value);
            return this;
        }

        /** 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0 */
        public Long getOffset() {
            return Json.read(json.get("offset"), Json::integer);
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public ReviewCarryRequest withOffset(Long value) {
            put("offset", value);
            return this;
        }

        /** 返す参照の数の上限。書かなければ 1000 */
        public Long getLimit() {
            return Json.read(json.get("limit"), Json::integer);
        }

        /** {@link #getLimit()} を書く。null なら欄を除く */
        public ReviewCarryRequest withLimit(Long value) {
            put("limit", value);
            return this;
        }
    }

    /** 移した結果。参照は offset から limit 個まで。移した文書は blobs の file で返す */
    public static final class ReviewCarryResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ReviewCarryResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ReviewCarryResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ReviewCarryResult(json);
        }

        /** 空のオブジェクトから作る */
        public ReviewCarryResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 移す前の対象のハッシュ */
        public String getFrom() {
            return Json.read(json.get("from"), Json::string);
        }

        /** {@link #getFrom()} を書く。null なら欄を除く */
        public ReviewCarryResult withFrom(String value) {
            put("from", value);
            return this;
        }

        /** 移した後の対象のハッシュ。文書のハッシュ */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public ReviewCarryResult withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 要素の参照ごとの状態。指摘の順に、場所の順、参照の順 */
        public List<CarriedReference> getReferences() {
            return Json.read(json.get("references"), v0 -> Json.list(v0, CarriedReference::of));
        }

        /** {@link #getReferences()} を書く。null なら欄を除く */
        public ReviewCarryResult withReferences(List<CarriedReference> value) {
            put("references", value);
            return this;
        }

        /** 全体の参照の数 */
        public Long getTotal() {
            return Json.read(json.get("total"), Json::integer);
        }

        /** {@link #getTotal()} を書く。null なら欄を除く */
        public ReviewCarryResult withTotal(Long value) {
            put("total", value);
            return this;
        }

        /** 続きの最初の参照の位置。続きがなければ持たない */
        public Long getNextOffset() {
            return Json.read(json.get("next_offset"), Json::integer);
        }

        /** {@link #getNextOffset()} を書く。null なら欄を除く */
        public ReviewCarryResult withNextOffset(Long value) {
            put("next_offset", value);
            return this;
        }
    }

    /** 移した要素の参照 */
    public static final class CarriedReference extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CarriedReference(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CarriedReference of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CarriedReference(json);
        }

        /** 空のオブジェクトから作る */
        public CarriedReference() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 指摘の ID */
        public String getTopic() {
            return Json.read(json.get("topic"), Json::string);
        }

        /** {@link #getTopic()} を書く。null なら欄を除く */
        public CarriedReference withTopic(String value) {
            put("topic", value);
            return this;
        }

        /** 場所の ID */
        public String getLocation() {
            return Json.read(json.get("location"), Json::string);
        }

        /** {@link #getLocation()} を書く。null なら欄を除く */
        public CarriedReference withLocation(String value) {
            put("location", value);
            return this;
        }

        /** 要素の ID */
        public String getElement() {
            return Json.read(json.get("element"), Json::string);
        }

        /** {@link #getElement()} を書く。null なら欄を除く */
        public CarriedReference withElement(String value) {
            put("element", value);
            return this;
        }

        /** 状態 */
        public String getState() {
            return Json.read(json.get("state"), Json::string);
        }

        /** {@link #getState()} を書く。null なら欄を除く */
        public CarriedReference withState(String value) {
            put("state", value);
            return this;
        }
    }

    /**
     * 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しい版に要素がない
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class CarriedState {
        private CarriedState() {}

        public static final String UNCHANGED = "unchanged";

        public static final String CHANGED = "changed";

        public static final String REMOVED = "removed";
    }

    /** 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。小版 43 から */
    public static final class ReviewAdoptRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ReviewAdoptRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ReviewAdoptRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ReviewAdoptRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public ReviewAdoptRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "review_adopt");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 照合する文書の名前。open で作ったもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す */
        public List<String> getProfiles() {
            return Json.read(json.get("profiles"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getProfiles()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withProfiles(List<String> value) {
            put("profiles", value);
            return this;
        }

        /** 照合に使うプロファイル。profiles の中から選ぶ */
        public ProfileRef getProfile() {
            return Json.read(json.get("profile"), ProfileRef::of);
        }

        /** {@link #getProfile()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withProfile(ProfileRef value) {
            put("profile", value);
            return this;
        }

        /** たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる */
        public String getFrom() {
            return Json.read(json.get("from"), Json::string);
        }

        /** {@link #getFrom()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withFrom(String value) {
            put("from", value);
            return this;
        }

        /** 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける */
        public Long getView() {
            return Json.read(json.get("view"), Json::integer);
        }

        /** {@link #getView()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withView(Long value) {
            put("view", value);
            return this;
        }

        /** 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から */
        public String getFileName() {
            return Json.read(json.get("file_name"), Json::string);
        }

        /** {@link #getFileName()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withFileName(String value) {
            put("file_name", value);
            return this;
        }

        /** 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く */
        public String getPalette() {
            return Json.read(json.get("palette"), Json::string);
        }

        /** {@link #getPalette()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withPalette(String value) {
            put("palette", value);
            return this;
        }

        /** 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す */
        public String getReview() {
            return Json.read(json.get("review"), Json::string);
        }

        /** {@link #getReview()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withReview(String value) {
            put("review", value);
            return this;
        }

        /** 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す */
        public String getTarget() {
            return Json.read(json.get("target"), Json::string);
        }

        /** {@link #getTarget()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withTarget(String value) {
            put("target", value);
            return this;
        }

        /** 取り込む指摘の番号の列。check の指摘の並びの順で、0 から数える */
        public List<Long> getFindings() {
            return Json.read(json.get("findings"), v0 -> Json.list(v0, Json::integer));
        }

        /** {@link #getFindings()} を書く。null なら欄を除く */
        public ReviewAdoptRequest withFindings(List<Long> value) {
            put("findings", value);
            return this;
        }
    }

    /** 取り込んだ結果。足した文書は blobs の file で返す */
    public static final class ReviewAdoptResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ReviewAdoptResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ReviewAdoptResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ReviewAdoptResult(json);
        }

        /** 空のオブジェクトから作る */
        public ReviewAdoptResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 足した指摘の ID。取り込み済みで足さなかった指摘は持たない */
        public List<String> getAdded() {
            return Json.read(json.get("added"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getAdded()} を書く。null なら欄を除く */
        public ReviewAdoptResult withAdded(List<String> value) {
            put("added", value);
            return this;
        }

        /** 足した後の文書の指摘の数 */
        public Long getTopics() {
            return Json.read(json.get("topics"), Json::integer);
        }

        /** {@link #getTopics()} を書く。null なら欄を除く */
        public ReviewAdoptResult withTopics(Long value) {
            put("topics", value);
            return this;
        }
    }

    /** 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。小版 44 から */
    public static final class BuildingRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BuildingRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BuildingRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BuildingRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public BuildingRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "building");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public BuildingRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public BuildingRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public BuildingRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public BuildingRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public BuildingRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public BuildingRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で IFC を開いたもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public BuildingRequest withDoc(String value) {
            put("doc", value);
            return this;
        }
    }

    /** 建物モデルの要約 */
    public static final class BuildingResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BuildingResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BuildingResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BuildingResult(json);
        }

        /** 空のオブジェクトから作る */
        public BuildingResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** IFC のスキーマの名前（FILE_SCHEMA の最初） */
        public String getSchema() {
            return Json.read(json.get("schema"), Json::string);
        }

        /** {@link #getSchema()} を書く。null なら欄を除く */
        public BuildingResult withSchema(String value) {
            put("schema", value);
            return this;
        }

        /** 入力のバイト列のハッシュ。review_carry の対象の版に使う */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public BuildingResult withHash(String value) {
            put("hash", value);
            return this;
        }

        /** プロジェクトの要素の ID。なければ持たない */
        public String getProject() {
            return Json.read(json.get("project"), Json::string);
        }

        /** {@link #getProject()} を書く。null なら欄を除く */
        public BuildingResult withProject(String value) {
            put("project", value);
            return this;
        }

        /** プロジェクトの名前。なければ持たない */
        public String getProjectName() {
            return Json.read(json.get("project_name"), Json::string);
        }

        /** {@link #getProjectName()} を書く。null なら欄を除く */
        public BuildingResult withProjectName(String value) {
            put("project_name", value);
            return this;
        }

        /** プロジェクトの単位 */
        public List<BuildingUnit> getUnits() {
            return Json.read(json.get("units"), v0 -> Json.list(v0, BuildingUnit::of));
        }

        /** {@link #getUnits()} を書く。null なら欄を除く */
        public BuildingResult withUnits(List<BuildingUnit> value) {
            put("units", value);
            return this;
        }

        /** 要素の数 */
        public Long getElements() {
            return Json.read(json.get("elements"), Json::integer);
        }

        /** {@link #getElements()} を書く。null なら欄を除く */
        public BuildingResult withElements(Long value) {
            put("elements", value);
            return this;
        }

        /** クラスごとの要素の数。クラスの名前の順 */
        public List<ClassCount> getClasses() {
            return Json.read(json.get("classes"), v0 -> Json.list(v0, ClassCount::of));
        }

        /** {@link #getClasses()} を書く。null なら欄を除く */
        public BuildingResult withClasses(List<ClassCount> value) {
            put("classes", value);
            return this;
        }
    }

    /** 単位 */
    public static final class BuildingUnit extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BuildingUnit(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BuildingUnit of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BuildingUnit(json);
        }

        /** 空のオブジェクトから作る */
        public BuildingUnit() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 単位の種類（LENGTHUNIT・AREAUNIT など） */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public BuildingUnit withKind(String value) {
            put("kind", value);
            return this;
        }

        /** SI の接頭辞（MILLI など） */
        public String getPrefix() {
            return Json.read(json.get("prefix"), Json::string);
        }

        /** {@link #getPrefix()} を書く。null なら欄を除く */
        public BuildingUnit withPrefix(String value) {
            put("prefix", value);
            return this;
        }

        /** 単位の名前（METRE、換算する単位なら inch など） */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public BuildingUnit withName(String value) {
            put("name", value);
            return this;
        }

        /** 換算する単位の、SI の単位での大きさ。SI の単位は持たない */
        public Double getScale() {
            return Json.read(json.get("scale"), Json::number);
        }

        /** {@link #getScale()} を書く。null なら欄を除く */
        public BuildingUnit withScale(Double value) {
            put("scale", value);
            return this;
        }
    }

    /** クラスの要素の数 */
    public static final class ClassCount extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ClassCount(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ClassCount of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ClassCount(json);
        }

        /** 空のオブジェクトから作る */
        public ClassCount() {
            super(new LinkedHashMap<String, Object>());
        }

        /** IFC のクラスの名前（IfcWall など） */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public ClassCount withName(String value) {
            put("name", value);
            return this;
        }

        /** 要素の数 */
        public Long getCount() {
            return Json.read(json.get("count"), Json::integer);
        }

        /** {@link #getCount()} を書く。null なら欄を除く */
        public ClassCount withCount(Long value) {
            put("count", value);
            return this;
        }
    }

    /** 開いた IFC の建物モデルの要素を、書いた順に返す。成分は components で選ぶ（決定 0207・0219）。小版 44 から */
    public static final class BuildingElementsRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BuildingElementsRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BuildingElementsRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BuildingElementsRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public BuildingElementsRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "building_elements");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public BuildingElementsRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public BuildingElementsRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public BuildingElementsRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public BuildingElementsRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public BuildingElementsRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public BuildingElementsRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で IFC を開いたもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public BuildingElementsRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 返す要素の ID。書けば、その要素だけを、書いた順に返す。ない ID は返さない */
        public List<String> getIds() {
            return Json.read(json.get("ids"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getIds()} を書く。null なら欄を除く */
        public BuildingElementsRequest withIds(List<String> value) {
            put("ids", value);
            return this;
        }

        /** IFC のクラスの名前。書けば、そのクラスの要素だけを返す。大文字と小文字を区別しない */
        public String getClassValue() {
            return Json.read(json.get("class"), Json::string);
        }

        /** {@link #getClassValue()} を書く。null なら欄を除く */
        public BuildingElementsRequest withClassValue(String value) {
            put("class", value);
            return this;
        }

        /** 返す成分の名前。1 つ以上で、重ねない。書かなければ class と identity を返す */
        public List<String> getComponents() {
            return Json.read(json.get("components"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getComponents()} を書く。null なら欄を除く */
        public BuildingElementsRequest withComponents(List<String> value) {
            put("components", value);
            return this;
        }

        /** 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0 */
        public Long getOffset() {
            return Json.read(json.get("offset"), Json::integer);
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public BuildingElementsRequest withOffset(Long value) {
            put("offset", value);
            return this;
        }

        /** 返す数の上限。書かなければ 1000 */
        public Long getLimit() {
            return Json.read(json.get("limit"), Json::integer);
        }

        /** {@link #getLimit()} を書く。null なら欄を除く */
        public BuildingElementsRequest withLimit(Long value) {
            put("limit", value);
            return this;
        }
    }

    /**
     * building_elements で返す成分（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class BuildingComponent {
        private BuildingComponent() {}

        public static final String IDENTITY = "identity";

        public static final String CLASS = "class";

        public static final String CLASSIFICATION = "classification";

        public static final String SPATIAL = "spatial";

        public static final String PARENT = "parent";

        public static final String PLACEMENT = "placement";

        public static final String TYPE_OF = "type_of";

        public static final String PROPERTIES = "properties";

        public static final String QUANTITIES = "quantities";

        public static final String MATERIAL = "material";

        public static final String OPENING = "opening";

        public static final String GROUP = "group";

        public static final String CONNECTION = "connection";

        public static final String LEVEL = "level";

        public static final String ZONE = "zone";

        public static final String STYLE = "style";

        public static final String RELATIONS = "relations";
    }

    /** 要素の一覧。offset から limit 個まで */
    public static final class BuildingElementsResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BuildingElementsResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BuildingElementsResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BuildingElementsResult(json);
        }

        /** 空のオブジェクトから作る */
        public BuildingElementsResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 要素 */
        public List<BuildingElement> getElements() {
            return Json.read(json.get("elements"), v0 -> Json.list(v0, BuildingElement::of));
        }

        /** {@link #getElements()} を書く。null なら欄を除く */
        public BuildingElementsResult withElements(List<BuildingElement> value) {
            put("elements", value);
            return this;
        }

        /** 選んだ要素の数 */
        public Long getTotal() {
            return Json.read(json.get("total"), Json::integer);
        }

        /** {@link #getTotal()} を書く。null なら欄を除く */
        public BuildingElementsResult withTotal(Long value) {
            put("total", value);
            return this;
        }

        /** 続きの最初の位置。続きがなければ持たない */
        public Long getNextOffset() {
            return Json.read(json.get("next_offset"), Json::integer);
        }

        /** {@link #getNextOffset()} を書く。null なら欄を除く */
        public BuildingElementsResult withNextOffset(Long value) {
            put("next_offset", value);
            return this;
        }
    }

    /** 建物モデルの要素。成分は、選んだもののうち要素が持つものだけを持つ */
    public static final class BuildingElement extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BuildingElement(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BuildingElement of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BuildingElement(json);
        }

        /** 空のオブジェクトから作る */
        public BuildingElement() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 要素の ID（GlobalId か、i と番号） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public BuildingElement withId(String value) {
            put("id", value);
            return this;
        }

        /** 要素の成分のハッシュ（v1: と 32 桁の 16 進、決定 0207） */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public BuildingElement withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 名前・説明・タグ・長い名前 */
        public IdentityComponent getIdentity() {
            return Json.read(json.get("identity"), IdentityComponent::of);
        }

        /** {@link #getIdentity()} を書く。null なら欄を除く */
        public BuildingElement withIdentity(IdentityComponent value) {
            put("identity", value);
            return this;
        }

        /** クラス */
        public ClassComponent getClassValue() {
            return Json.read(json.get("class"), ClassComponent::of);
        }

        /** {@link #getClassValue()} を書く。null なら欄を除く */
        public BuildingElement withClassValue(ClassComponent value) {
            put("class", value);
            return this;
        }

        /** 分類の参照 */
        public List<ClassificationReference> getClassification() {
            return Json.read(json.get("classification"), v0 -> Json.list(v0, ClassificationReference::of));
        }

        /** {@link #getClassification()} を書く。null なら欄を除く */
        public BuildingElement withClassification(List<ClassificationReference> value) {
            put("classification", value);
            return this;
        }

        /** 空間の要素の種類と高さ */
        public SpatialComponent getSpatial() {
            return Json.read(json.get("spatial"), SpatialComponent::of);
        }

        /** {@link #getSpatial()} を書く。null なら欄を除く */
        public BuildingElement withSpatial(SpatialComponent value) {
            put("spatial", value);
            return this;
        }

        /** 親と、参照される空間 */
        public ParentComponent getParent() {
            return Json.read(json.get("parent"), ParentComponent::of);
        }

        /** {@link #getParent()} を書く。null なら欄を除く */
        public BuildingElement withParent(ParentComponent value) {
            put("parent", value);
            return this;
        }

        /** 配置 */
        public PlacementComponent getPlacement() {
            return Json.read(json.get("placement"), PlacementComponent::of);
        }

        /** {@link #getPlacement()} を書く。null なら欄を除く */
        public BuildingElement withPlacement(PlacementComponent value) {
            put("placement", value);
            return this;
        }

        /** 型の要素の ID */
        public String getTypeOf() {
            return Json.read(json.get("type_of"), Json::string);
        }

        /** {@link #getTypeOf()} を書く。null なら欄を除く */
        public BuildingElement withTypeOf(String value) {
            put("type_of", value);
            return this;
        }

        /** プロパティのセット */
        public List<PropertySet> getProperties() {
            return Json.read(json.get("properties"), v0 -> Json.list(v0, PropertySet::of));
        }

        /** {@link #getProperties()} を書く。null なら欄を除く */
        public BuildingElement withProperties(List<PropertySet> value) {
            put("properties", value);
            return this;
        }

        /** 数量のセット */
        public List<QuantitySet> getQuantities() {
            return Json.read(json.get("quantities"), v0 -> Json.list(v0, QuantitySet::of));
        }

        /** {@link #getQuantities()} を書く。null なら欄を除く */
        public BuildingElement withQuantities(List<QuantitySet> value) {
            put("quantities", value);
            return this;
        }

        /** 材料 */
        public MaterialComponent getMaterial() {
            return Json.read(json.get("material"), MaterialComponent::of);
        }

        /** {@link #getMaterial()} を書く。null なら欄を除く */
        public BuildingElement withMaterial(MaterialComponent value) {
            put("material", value);
            return this;
        }

        /** 開口と、埋める要素 */
        public OpeningComponent getOpening() {
            return Json.read(json.get("opening"), OpeningComponent::of);
        }

        /** {@link #getOpening()} を書く。null なら欄を除く */
        public BuildingElement withOpening(OpeningComponent value) {
            put("opening", value);
            return this;
        }

        /** 属するグループの要素の ID */
        public List<String> getGroup() {
            return Json.read(json.get("group"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getGroup()} を書く。null なら欄を除く */
        public BuildingElement withGroup(List<String> value) {
            put("group", value);
            return this;
        }

        /** 接続 */
        public List<ConnectionLink> getConnection() {
            return Json.read(json.get("connection"), v0 -> Json.list(v0, ConnectionLink::of));
        }

        /** {@link #getConnection()} を書く。null なら欄を除く */
        public BuildingElement withConnection(List<ConnectionLink> value) {
            put("connection", value);
            return this;
        }

        /** レベル */
        public LevelComponent getLevel() {
            return Json.read(json.get("level"), LevelComponent::of);
        }

        /** {@link #getLevel()} を書く。null なら欄を除く */
        public BuildingElement withLevel(LevelComponent value) {
            put("level", value);
            return this;
        }

        /** 区画 */
        public ZoneComponent getZone() {
            return Json.read(json.get("zone"), ZoneComponent::of);
        }

        /** {@link #getZone()} を書く。null なら欄を除く */
        public BuildingElement withZone(ZoneComponent value) {
            put("zone", value);
            return this;
        }

        /** 表示のスタイル */
        public StyleComponent getStyle() {
            return Json.read(json.get("style"), StyleComponent::of);
        }

        /** {@link #getStyle()} を書く。null なら欄を除く */
        public BuildingElement withStyle(StyleComponent value) {
            put("style", value);
            return this;
        }

        /** 索引から求める関係 */
        public RelationsComponent getRelations() {
            return Json.read(json.get("relations"), RelationsComponent::of);
        }

        /** {@link #getRelations()} を書く。null なら欄を除く */
        public BuildingElement withRelations(RelationsComponent value) {
            put("relations", value);
            return this;
        }
    }

    /** 名前・説明・タグ・長い名前 */
    public static final class IdentityComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public IdentityComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static IdentityComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new IdentityComponent(json);
        }

        /** 空のオブジェクトから作る */
        public IdentityComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public IdentityComponent withName(String value) {
            put("name", value);
            return this;
        }

        /** 説明 */
        public String getDescription() {
            return Json.read(json.get("description"), Json::string);
        }

        /** {@link #getDescription()} を書く。null なら欄を除く */
        public IdentityComponent withDescription(String value) {
            put("description", value);
            return this;
        }

        /** タグ */
        public String getTag() {
            return Json.read(json.get("tag"), Json::string);
        }

        /** {@link #getTag()} を書く。null なら欄を除く */
        public IdentityComponent withTag(String value) {
            put("tag", value);
            return this;
        }

        /** 長い名前 */
        public String getLongName() {
            return Json.read(json.get("long_name"), Json::string);
        }

        /** {@link #getLongName()} を書く。null なら欄を除く */
        public IdentityComponent withLongName(String value) {
            put("long_name", value);
            return this;
        }
    }

    /** クラス */
    public static final class ClassComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ClassComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ClassComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ClassComponent(json);
        }

        /** 空のオブジェクトから作る */
        public ClassComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** IFC4 のクラスの名前。IFC2X3 の StandardCase などは、元の名前を original に残して直す */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public ClassComponent withName(String value) {
            put("name", value);
            return this;
        }

        /** PredefinedType */
        public String getPredefined() {
            return Json.read(json.get("predefined"), Json::string);
        }

        /** {@link #getPredefined()} を書く。null なら欄を除く */
        public ClassComponent withPredefined(String value) {
            put("predefined", value);
            return this;
        }

        /** ObjectType など、USERDEFINED のときの名前 */
        public String getUserDefined() {
            return Json.read(json.get("user_defined"), Json::string);
        }

        /** {@link #getUserDefined()} を書く。null なら欄を除く */
        public ClassComponent withUserDefined(String value) {
            put("user_defined", value);
            return this;
        }

        /** ファイルに書いた実体の名前 */
        public String getOriginal() {
            return Json.read(json.get("original"), Json::string);
        }

        /** {@link #getOriginal()} を書く。null なら欄を除く */
        public ClassComponent withOriginal(String value) {
            put("original", value);
            return this;
        }
    }

    /** 分類の参照 */
    public static final class ClassificationReference extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ClassificationReference(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ClassificationReference of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ClassificationReference(json);
        }

        /** 空のオブジェクトから作る */
        public ClassificationReference() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 分類の体系の名前 */
        public String getSystem() {
            return Json.read(json.get("system"), Json::string);
        }

        /** {@link #getSystem()} を書く。null なら欄を除く */
        public ClassificationReference withSystem(String value) {
            put("system", value);
            return this;
        }

        /** 体系の版 */
        public String getEdition() {
            return Json.read(json.get("edition"), Json::string);
        }

        /** {@link #getEdition()} を書く。null なら欄を除く */
        public ClassificationReference withEdition(String value) {
            put("edition", value);
            return this;
        }

        /** 分類の記号 */
        public String getCode() {
            return Json.read(json.get("code"), Json::string);
        }

        /** {@link #getCode()} を書く。null なら欄を除く */
        public ClassificationReference withCode(String value) {
            put("code", value);
            return this;
        }

        /** 分類の名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public ClassificationReference withName(String value) {
            put("name", value);
            return this;
        }
    }

    /** 単位とともに持つ値 */
    public static final class Measured extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Measured(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Measured of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Measured(json);
        }

        /** 空のオブジェクトから作る */
        public Measured() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 値 */
        public Double getValue() {
            return Json.read(json.get("value"), Json::number);
        }

        /** {@link #getValue()} を書く。null なら欄を除く */
        public Measured withValue(Double value) {
            put("value", value);
            return this;
        }

        /** 単位。解けなければ持たない */
        public BuildingUnit getUnit() {
            return Json.read(json.get("unit"), BuildingUnit::of);
        }

        /** {@link #getUnit()} を書く。null なら欄を除く */
        public Measured withUnit(BuildingUnit value) {
            put("unit", value);
            return this;
        }
    }

    /** 空間の要素 */
    public static final class SpatialComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SpatialComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SpatialComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SpatialComponent(json);
        }

        /** 空のオブジェクトから作る */
        public SpatialComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** site・building・storey・space・spatial_zone か、元の実体の名前 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public SpatialComponent withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 高さ */
        public Measured getElevation() {
            return Json.read(json.get("elevation"), Measured::of);
        }

        /** {@link #getElevation()} を書く。null なら欄を除く */
        public SpatialComponent withElevation(Measured value) {
            put("elevation", value);
            return this;
        }

        /** 基準の高さ */
        public Measured getRefElevation() {
            return Json.read(json.get("ref_elevation"), Measured::of);
        }

        /** {@link #getRefElevation()} を書く。null なら欄を除く */
        public SpatialComponent withRefElevation(Measured value) {
            put("ref_elevation", value);
            return this;
        }
    }

    /** 親 */
    public static final class ParentComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ParentComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ParentComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ParentComponent(json);
        }

        /** 空のオブジェクトから作る */
        public ParentComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 親の要素 */
        public Membership getOf() {
            return Json.read(json.get("of"), Membership::of);
        }

        /** {@link #getOf()} を書く。null なら欄を除く */
        public ParentComponent withOf(Membership value) {
            put("of", value);
            return this;
        }

        /** 参照される空間の要素の ID */
        public List<String> getReferencedIn() {
            return Json.read(json.get("referenced_in"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getReferencedIn()} を書く。null なら欄を除く */
        public ParentComponent withReferencedIn(List<String> value) {
            put("referenced_in", value);
            return this;
        }
    }

    /** 親との関係 */
    public static final class Membership extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Membership(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Membership of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Membership(json);
        }

        /** 空のオブジェクトから作る */
        public Membership() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 親の要素の ID */
        public String getTarget() {
            return Json.read(json.get("target"), Json::string);
        }

        /** {@link #getTarget()} を書く。null なら欄を除く */
        public Membership withTarget(String value) {
            put("target", value);
            return this;
        }

        /** 関係の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public Membership withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 入れ子の順。0 から */
        public Long getOrder() {
            return Json.read(json.get("order"), Json::integer);
        }

        /** {@link #getOrder()} を書く。null なら欄を除く */
        public Membership withOrder(Long value) {
            put("order", value);
            return this;
        }
    }

    /** 配置 */
    public static final class PlacementComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PlacementComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PlacementComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PlacementComponent(json);
        }

        /** 空のオブジェクトから作る */
        public PlacementComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 配置の親の要素の ID */
        public String getRelativeTo() {
            return Json.read(json.get("relative_to"), Json::string);
        }

        /** {@link #getRelativeTo()} を書く。null なら欄を除く */
        public PlacementComponent withRelativeTo(String value) {
            put("relative_to", value);
            return this;
        }

        /** 親をたどった世界の変換。3 行 4 列（回転と移動）。解けなければ持たない */
        public List<List<Double>> getWorld() {
            return Json.read(json.get("world"), v0 -> Json.list(v0, v1 -> Json.list(v1, Json::number)));
        }

        /** {@link #getWorld()} を書く。null なら欄を除く */
        public PlacementComponent withWorld(List<List<Double>> value) {
            put("world", value);
            return this;
        }

        /** 解けなかった配置の実体の名前 */
        public String getUnsupported() {
            return Json.read(json.get("unsupported"), Json::string);
        }

        /** {@link #getUnsupported()} を書く。null なら欄を除く */
        public PlacementComponent withUnsupported(String value) {
            put("unsupported", value);
            return this;
        }
    }

    /** 値 */
    public static final class BuildingValue extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BuildingValue(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BuildingValue of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BuildingValue(json);
        }

        /** 空のオブジェクトから作る */
        public BuildingValue() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 値の型の名前（IfcLengthMeasure・IfcLabel など） */
        public String getTypeName() {
            return Json.read(json.get("type_name"), Json::string);
        }

        /** {@link #getTypeName()} を書く。null なら欄を除く */
        public BuildingValue withTypeName(String value) {
            put("type_name", value);
            return this;
        }

        /** 値の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public BuildingValue withKind(String value) {
            put("kind", value);
            return this;
        }

        /** real と、2^53 − 1 までの integer の値 */
        public Double getNumber() {
            return Json.read(json.get("number"), Json::number);
        }

        /** {@link #getNumber()} を書く。null なら欄を除く */
        public BuildingValue withNumber(Double value) {
            put("number", value);
            return this;
        }

        /** text と enumeration の値と、2^53 − 1 を超える integer の値 */
        public String getText() {
            return Json.read(json.get("text"), Json::string);
        }

        /** {@link #getText()} を書く。null なら欄を除く */
        public BuildingValue withText(String value) {
            put("text", value);
            return this;
        }

        /** boolean と logical の値。logical の不明は持たない */
        public Boolean getBoolean() {
            return Json.read(json.get("boolean"), Json::bool);
        }

        /** {@link #getBoolean()} を書く。null なら欄を除く */
        public BuildingValue withBoolean(Boolean value) {
            put("boolean", value);
            return this;
        }

        /** element の要素の ID */
        public String getElement() {
            return Json.read(json.get("element"), Json::string);
        }

        /** {@link #getElement()} を書く。null なら欄を除く */
        public BuildingValue withElement(String value) {
            put("element", value);
            return this;
        }

        /** list の値 */
        public List<BuildingValue> getItems() {
            return Json.read(json.get("items"), v0 -> Json.list(v0, BuildingValue::of));
        }

        /** {@link #getItems()} を書く。null なら欄を除く */
        public BuildingValue withItems(List<BuildingValue> value) {
            put("items", value);
            return this;
        }
    }

    /** プロパティのセット */
    public static final class PropertySet extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PropertySet(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PropertySet of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PropertySet(json);
        }

        /** 空のオブジェクトから作る */
        public PropertySet() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public PropertySet withName(String value) {
            put("name", value);
            return this;
        }

        /** プロパティ */
        public List<Property> getProperties() {
            return Json.read(json.get("properties"), v0 -> Json.list(v0, Property::of));
        }

        /** {@link #getProperties()} を書く。null なら欄を除く */
        public PropertySet withProperties(List<Property> value) {
            put("properties", value);
            return this;
        }
    }

    /** プロパティ */
    public static final class Property extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Property(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Property of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Property(json);
        }

        /** 空のオブジェクトから作る */
        public Property() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public Property withName(String value) {
            put("name", value);
            return this;
        }

        /** 値。値がなければ持たない */
        public BuildingValue getValue() {
            return Json.read(json.get("value"), BuildingValue::of);
        }

        /** {@link #getValue()} を書く。null なら欄を除く */
        public Property withValue(BuildingValue value) {
            put("value", value);
            return this;
        }

        /** 単位 */
        public BuildingUnit getUnit() {
            return Json.read(json.get("unit"), BuildingUnit::of);
        }

        /** {@link #getUnit()} を書く。null なら欄を除く */
        public Property withUnit(BuildingUnit value) {
            put("unit", value);
            return this;
        }
    }

    /** 数量のセット */
    public static final class QuantitySet extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public QuantitySet(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static QuantitySet of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new QuantitySet(json);
        }

        /** 空のオブジェクトから作る */
        public QuantitySet() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public QuantitySet withName(String value) {
            put("name", value);
            return this;
        }

        /** 計測の方法 */
        public String getMethod() {
            return Json.read(json.get("method"), Json::string);
        }

        /** {@link #getMethod()} を書く。null なら欄を除く */
        public QuantitySet withMethod(String value) {
            put("method", value);
            return this;
        }

        /** 数量 */
        public List<Quantity> getQuantities() {
            return Json.read(json.get("quantities"), v0 -> Json.list(v0, Quantity::of));
        }

        /** {@link #getQuantities()} を書く。null なら欄を除く */
        public QuantitySet withQuantities(List<Quantity> value) {
            put("quantities", value);
            return this;
        }
    }

    /** 数量 */
    public static final class Quantity extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Quantity(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Quantity of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Quantity(json);
        }

        /** 空のオブジェクトから作る */
        public Quantity() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public Quantity withName(String value) {
            put("name", value);
            return this;
        }

        /** 種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public Quantity withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 値 */
        public Measured getValue() {
            return Json.read(json.get("value"), Measured::of);
        }

        /** {@link #getValue()} を書く。null なら欄を除く */
        public Quantity withValue(Measured value) {
            put("value", value);
            return this;
        }

        /** 式 */
        public String getFormula() {
            return Json.read(json.get("formula"), Json::string);
        }

        /** {@link #getFormula()} を書く。null なら欄を除く */
        public Quantity withFormula(String value) {
            put("formula", value);
            return this;
        }
    }

    /** 材料 */
    public static final class MaterialComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public MaterialComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static MaterialComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new MaterialComponent(json);
        }

        /** 空のオブジェクトから作る */
        public MaterialComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 材料の組み方 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public MaterialComponent withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 部分 */
        public List<MaterialPart> getParts() {
            return Json.read(json.get("parts"), v0 -> Json.list(v0, MaterialPart::of));
        }

        /** {@link #getParts()} を書く。null なら欄を除く */
        public MaterialComponent withParts(List<MaterialPart> value) {
            put("parts", value);
            return this;
        }

        /** 層の向き */
        public String getDirection() {
            return Json.read(json.get("direction"), Json::string);
        }

        /** {@link #getDirection()} を書く。null なら欄を除く */
        public MaterialComponent withDirection(String value) {
            put("direction", value);
            return this;
        }

        /** 層の基準からのずれ */
        public Measured getOffset() {
            return Json.read(json.get("offset"), Measured::of);
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public MaterialComponent withOffset(Measured value) {
            put("offset", value);
            return this;
        }
    }

    /** 材料の部分 */
    public static final class MaterialPart extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public MaterialPart(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static MaterialPart of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new MaterialPart(json);
        }

        /** 空のオブジェクトから作る */
        public MaterialPart() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 材料の要素の ID */
        public String getMaterial() {
            return Json.read(json.get("material"), Json::string);
        }

        /** {@link #getMaterial()} を書く。null なら欄を除く */
        public MaterialPart withMaterial(String value) {
            put("material", value);
            return this;
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public MaterialPart withName(String value) {
            put("name", value);
            return this;
        }

        /** 厚さ */
        public Measured getThickness() {
            return Json.read(json.get("thickness"), Measured::of);
        }

        /** {@link #getThickness()} を書く。null なら欄を除く */
        public MaterialPart withThickness(Measured value) {
            put("thickness", value);
            return this;
        }
    }

    /** 開口 */
    public static final class OpeningComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public OpeningComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static OpeningComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new OpeningComponent(json);
        }

        /** 空のオブジェクトから作る */
        public OpeningComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** この開口が穴を開ける要素の ID */
        public String getVoids() {
            return Json.read(json.get("voids"), Json::string);
        }

        /** {@link #getVoids()} を書く。null なら欄を除く */
        public OpeningComponent withVoids(String value) {
            put("voids", value);
            return this;
        }

        /** この要素が埋める開口の要素の ID */
        public String getFills() {
            return Json.read(json.get("fills"), Json::string);
        }

        /** {@link #getFills()} を書く。null なら欄を除く */
        public OpeningComponent withFills(String value) {
            put("fills", value);
            return this;
        }
    }

    /** 接続 */
    public static final class ConnectionLink extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ConnectionLink(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ConnectionLink of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ConnectionLink(json);
        }

        /** 空のオブジェクトから作る */
        public ConnectionLink() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 関係の実体の名前 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public ConnectionLink withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 接続する要素の ID */
        public String getTarget() {
            return Json.read(json.get("target"), Json::string);
        }

        /** {@link #getTarget()} を書く。null なら欄を除く */
        public ConnectionLink withTarget(String value) {
            put("target", value);
            return this;
        }
    }

    /** レベル */
    public static final class LevelComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LevelComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LevelComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LevelComponent(json);
        }

        /** 空のオブジェクトから作る */
        public LevelComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public LevelComponent withName(String value) {
            put("name", value);
            return this;
        }

        /** 種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public LevelComponent withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 高さ */
        public Measured getHeight() {
            return Json.read(json.get("height"), Measured::of);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public LevelComponent withHeight(Measured value) {
            put("height", value);
            return this;
        }

        /** 基準 */
        public String getDatum() {
            return Json.read(json.get("datum"), Json::string);
        }

        /** {@link #getDatum()} を書く。null なら欄を除く */
        public LevelComponent withDatum(String value) {
            put("datum", value);
            return this;
        }
    }

    /** 区画 */
    public static final class ZoneComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ZoneComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ZoneComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ZoneComponent(json);
        }

        /** 空のオブジェクトから作る */
        public ZoneComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 区画の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public ZoneComponent withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 構成する要素の ID */
        public List<String> getMembers() {
            return Json.read(json.get("members"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getMembers()} を書く。null なら欄を除く */
        public ZoneComponent withMembers(List<String> value) {
            put("members", value);
            return this;
        }
    }

    /** 表示のスタイル */
    public static final class StyleComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public StyleComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static StyleComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new StyleComponent(json);
        }

        /** 空のオブジェクトから作る */
        public StyleComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 面の色の RGB（0 から 1） */
        public List<Double> getSurfaceColor() {
            return Json.read(json.get("surface_color"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getSurfaceColor()} を書く。null なら欄を除く */
        public StyleComponent withSurfaceColor(List<Double> value) {
            put("surface_color", value);
            return this;
        }

        /** 透明度（0 から 1） */
        public Double getTransparency() {
            return Json.read(json.get("transparency"), Json::number);
        }

        /** {@link #getTransparency()} を書く。null なら欄を除く */
        public StyleComponent withTransparency(Double value) {
            put("transparency", value);
            return this;
        }

        /** 線の色の RGB（0 から 1） */
        public List<Double> getCurveColor() {
            return Json.read(json.get("curve_color"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getCurveColor()} を書く。null なら欄を除く */
        public StyleComponent withCurveColor(List<Double> value) {
            put("curve_color", value);
            return this;
        }

        /** 提示のレイヤの名前 */
        public List<String> getLayers() {
            return Json.read(json.get("layers"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getLayers()} を書く。null なら欄を除く */
        public StyleComponent withLayers(List<String> value) {
            put("layers", value);
            return this;
        }
    }

    /** 索引から求める関係。要素の ID の列 */
    public static final class RelationsComponent extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public RelationsComponent(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static RelationsComponent of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new RelationsComponent(json);
        }

        /** 空のオブジェクトから作る */
        public RelationsComponent() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 子（集約・入れ子・包含） */
        public List<String> getChildren() {
            return Json.read(json.get("children"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getChildren()} を書く。null なら欄を除く */
        public RelationsComponent withChildren(List<String> value) {
            put("children", value);
            return this;
        }

        /** 参照する要素 */
        public List<String> getReferenced() {
            return Json.read(json.get("referenced"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getReferenced()} を書く。null なら欄を除く */
        public RelationsComponent withReferenced(List<String> value) {
            put("referenced", value);
            return this;
        }

        /** 型の実例 */
        public List<String> getInstances() {
            return Json.read(json.get("instances"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getInstances()} を書く。null なら欄を除く */
        public RelationsComponent withInstances(List<String> value) {
            put("instances", value);
            return this;
        }

        /** 開口 */
        public List<String> getOpenings() {
            return Json.read(json.get("openings"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getOpenings()} を書く。null なら欄を除く */
        public RelationsComponent withOpenings(List<String> value) {
            put("openings", value);
            return this;
        }

        /** 開口を埋める要素 */
        public List<String> getFillings() {
            return Json.read(json.get("fillings"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getFillings()} を書く。null なら欄を除く */
        public RelationsComponent withFillings(List<String> value) {
            put("fillings", value);
            return this;
        }

        /** グループ・システム・区画の構成員 */
        public List<String> getMembers() {
            return Json.read(json.get("members"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getMembers()} を書く。null なら欄を除く */
        public RelationsComponent withMembers(List<String> value) {
            put("members", value);
            return this;
        }
    }

    /**
     * 親との関係の種類。集約・入れ子・包含
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class ParentKind {
        private ParentKind() {}

        public static final String AGGREGATES = "aggregates";

        public static final String NESTS = "nests";

        public static final String CONTAINED = "contained";
    }

    /**
     * 値の種類
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class BuildingValueKind {
        private BuildingValueKind() {}

        public static final String REAL = "real";

        public static final String INTEGER = "integer";

        public static final String TEXT = "text";

        public static final String BOOLEAN = "boolean";

        public static final String LOGICAL = "logical";

        public static final String ENUMERATION = "enumeration";

        public static final String ELEMENT = "element";

        public static final String LIST = "list";
    }

    /**
     * 数量の種類
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class QuantityKind {
        private QuantityKind() {}

        public static final String LENGTH = "length";

        public static final String AREA = "area";

        public static final String VOLUME = "volume";

        public static final String COUNT = "count";

        public static final String WEIGHT = "weight";

        public static final String TIME = "time";
    }

    /**
     * 材料の組み方。単一・層・断面・構成・一覧
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class MaterialKind {
        private MaterialKind() {}

        public static final String SINGLE = "single";

        public static final String LAYERS = "layers";

        public static final String PROFILES = "profiles";

        public static final String CONSTITUENTS = "constituents";

        public static final String LIST = "list";
    }

    /**
     * 仕様の結果。unknown は確かめられない facet があった
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class IdsStatus {
        private IdsStatus() {}

        public static final String PASSED = "passed";

        public static final String FAILED = "failed";

        public static final String UNKNOWN = "unknown";
    }

    /** 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。小版 44 から */
    public static final class IdsCheckRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public IdsCheckRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static IdsCheckRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new IdsCheckRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public IdsCheckRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "ids_check");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public IdsCheckRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public IdsCheckRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public IdsCheckRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public IdsCheckRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public IdsCheckRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public IdsCheckRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で IFC を開いたもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public IdsCheckRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** IDS の入力の名前（zm_push で渡した .ids） */
        public String getIds() {
            return Json.read(json.get("ids"), Json::string);
        }

        /** {@link #getIds()} を書く。null なら欄を除く */
        public IdsCheckRequest withIds(String value) {
            put("ids", value);
            return this;
        }

        /** EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす */
        public String getExpress() {
            return Json.read(json.get("express"), Json::string);
        }

        /** {@link #getExpress()} を書く。null なら欄を除く */
        public IdsCheckRequest withExpress(String value) {
            put("express", value);
            return this;
        }

        /** 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0 */
        public Long getOffset() {
            return Json.read(json.get("offset"), Json::integer);
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public IdsCheckRequest withOffset(Long value) {
            put("offset", value);
            return this;
        }

        /** 返す数の上限。書かなければ 1000 */
        public Long getLimit() {
            return Json.read(json.get("limit"), Json::integer);
        }

        /** {@link #getLimit()} を書く。null なら欄を除く */
        public IdsCheckRequest withLimit(Long value) {
            put("limit", value);
            return this;
        }
    }

    /** 照合の結果。指摘は offset から limit 個まで */
    public static final class IdsCheckResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public IdsCheckResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static IdsCheckResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new IdsCheckResult(json);
        }

        /** 空のオブジェクトから作る */
        public IdsCheckResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** IDS の題 */
        public String getTitle() {
            return Json.read(json.get("title"), Json::string);
        }

        /** {@link #getTitle()} を書く。null なら欄を除く */
        public IdsCheckResult withTitle(String value) {
            put("title", value);
            return this;
        }

        /** 仕様ごとの結果。IDS の順 */
        public List<IdsSpecification> getSpecifications() {
            return Json.read(json.get("specifications"), v0 -> Json.list(v0, IdsSpecification::of));
        }

        /** {@link #getSpecifications()} を書く。null なら欄を除く */
        public IdsCheckResult withSpecifications(List<IdsSpecification> value) {
            put("specifications", value);
            return this;
        }

        /** 指摘。仕様の順 */
        public List<IdsFinding> getFindings() {
            return Json.read(json.get("findings"), v0 -> Json.list(v0, IdsFinding::of));
        }

        /** {@link #getFindings()} を書く。null なら欄を除く */
        public IdsCheckResult withFindings(List<IdsFinding> value) {
            put("findings", value);
            return this;
        }

        /** 全体の指摘の数 */
        public Long getTotal() {
            return Json.read(json.get("total"), Json::integer);
        }

        /** {@link #getTotal()} を書く。null なら欄を除く */
        public IdsCheckResult withTotal(Long value) {
            put("total", value);
            return this;
        }

        /** 続きの最初の位置。続きがなければ持たない */
        public Long getNextOffset() {
            return Json.read(json.get("next_offset"), Json::integer);
        }

        /** {@link #getNextOffset()} を書く。null なら欄を除く */
        public IdsCheckResult withNextOffset(Long value) {
            put("next_offset", value);
            return this;
        }
    }

    /** 仕様の結果 */
    public static final class IdsSpecification extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public IdsSpecification(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static IdsSpecification of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new IdsSpecification(json);
        }

        /** 空のオブジェクトから作る */
        public IdsSpecification() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public IdsSpecification withName(String value) {
            put("name", value);
            return this;
        }

        /** 識別子 */
        public String getIdentifier() {
            return Json.read(json.get("identifier"), Json::string);
        }

        /** {@link #getIdentifier()} を書く。null なら欄を除く */
        public IdsSpecification withIdentifier(String value) {
            put("identifier", value);
            return this;
        }

        /** 結果 */
        public String getStatus() {
            return Json.read(json.get("status"), Json::string);
        }

        /** {@link #getStatus()} を書く。null なら欄を除く */
        public IdsSpecification withStatus(String value) {
            put("status", value);
            return this;
        }

        /** 適用の条件に合う要素の数 */
        public Long getApplicable() {
            return Json.read(json.get("applicable"), Json::integer);
        }

        /** {@link #getApplicable()} を書く。null なら欄を除く */
        public IdsSpecification withApplicable(Long value) {
            put("applicable", value);
            return this;
        }

        /** 要件を満たさない要素の数 */
        public Long getFailed() {
            return Json.read(json.get("failed"), Json::integer);
        }

        /** {@link #getFailed()} を書く。null なら欄を除く */
        public IdsSpecification withFailed(Long value) {
            put("failed", value);
            return this;
        }

        /** 確かめられなかった要素の数 */
        public Long getUnsupported() {
            return Json.read(json.get("unsupported"), Json::integer);
        }

        /** {@link #getUnsupported()} を書く。null なら欄を除く */
        public IdsSpecification withUnsupported(Long value) {
            put("unsupported", value);
            return this;
        }
    }

    /** 指摘。diagnostic は診断と同じ形で、describe で文にできる */
    public static final class IdsFinding extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public IdsFinding(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static IdsFinding of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new IdsFinding(json);
        }

        /** 空のオブジェクトから作る */
        public IdsFinding() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 仕様の名前 */
        public String getSpecification() {
            return Json.read(json.get("specification"), Json::string);
        }

        /** {@link #getSpecification()} を書く。null なら欄を除く */
        public IdsFinding withSpecification(String value) {
            put("specification", value);
            return this;
        }

        /** code・重要度・場所（要素の ID）・値 */
        public Diagnostic getDiagnostic() {
            return Json.read(json.get("diagnostic"), Diagnostic::of);
        }

        /** {@link #getDiagnostic()} を書く。null なら欄を除く */
        public IdsFinding withDiagnostic(Diagnostic value) {
            put("diagnostic", value);
            return this;
        }
    }

    /** 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。小版 44 から */
    public static final class ReviewAdoptIdsRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ReviewAdoptIdsRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ReviewAdoptIdsRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ReviewAdoptIdsRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public ReviewAdoptIdsRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "review_adopt_ids");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 文書の名前。open で IFC を開いたもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** IDS の入力の名前（zm_push で渡した .ids） */
        public String getIds() {
            return Json.read(json.get("ids"), Json::string);
        }

        /** {@link #getIds()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withIds(String value) {
            put("ids", value);
            return this;
        }

        /** EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす */
        public String getExpress() {
            return Json.read(json.get("express"), Json::string);
        }

        /** {@link #getExpress()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withExpress(String value) {
            put("express", value);
            return this;
        }

        /** 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す */
        public String getReview() {
            return Json.read(json.get("review"), Json::string);
        }

        /** {@link #getReview()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withReview(String value) {
            put("review", value);
            return this;
        }

        /** 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す */
        public String getTarget() {
            return Json.read(json.get("target"), Json::string);
        }

        /** {@link #getTarget()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withTarget(String value) {
            put("target", value);
            return this;
        }

        /** 取り込む指摘の番号の列。ids_check の指摘の並びの順で、0 から数える */
        public List<Long> getFindings() {
            return Json.read(json.get("findings"), v0 -> Json.list(v0, Json::integer));
        }

        /** {@link #getFindings()} を書く。null なら欄を除く */
        public ReviewAdoptIdsRequest withFindings(List<Long> value) {
            put("findings", value);
            return this;
        }
    }

    /** zm_push で渡した BCF 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。小版 45 から */
    public static final class BcfReadRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BcfReadRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BcfReadRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BcfReadRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public BcfReadRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "bcf_read");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public BcfReadRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public BcfReadRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public BcfReadRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public BcfReadRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public BcfReadRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public BcfReadRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** BCF が指す建物モデルの文書の名前。open で IFC を開いたもの */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public BcfReadRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** bcfzip の入力の名前（zm_push で渡した .bcf） */
        public String getInput() {
            return Json.read(json.get("input"), Json::string);
        }

        /** {@link #getInput()} を書く。null なら欄を除く */
        public BcfReadRequest withInput(String value) {
            put("input", value);
            return this;
        }

        /** レビューの文書の対象の ID */
        public String getTarget() {
            return Json.read(json.get("target"), Json::string);
        }

        /** {@link #getTarget()} を書く。null なら欄を除く */
        public BcfReadRequest withTarget(String value) {
            put("target", value);
            return this;
        }

        /** 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す */
        public List<BcfStatus> getStatuses() {
            return Json.read(json.get("statuses"), v0 -> Json.list(v0, BcfStatus::of));
        }

        /** {@link #getStatuses()} を書く。null なら欄を除く */
        public BcfReadRequest withStatuses(List<BcfStatus> value) {
            put("statuses", value);
            return this;
        }
    }

    /** 状態の対応 */
    public static final class BcfStatus extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BcfStatus(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BcfStatus of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BcfStatus(json);
        }

        /** 空のオブジェクトから作る */
        public BcfStatus() {
            super(new LinkedHashMap<String, Object>());
        }

        /** BCF の TopicStatus の文字列 */
        public String getText() {
            return Json.read(json.get("text"), Json::string);
        }

        /** {@link #getText()} を書く。null なら欄を除く */
        public BcfStatus withText(String value) {
            put("text", value);
            return this;
        }

        /** 指摘の状態 */
        public String getStatus() {
            return Json.read(json.get("status"), Json::string);
        }

        /** {@link #getStatus()} を書く。null なら欄を除く */
        public BcfStatus withStatus(String value) {
            put("status", value);
            return this;
        }
    }

    /**
     * 指摘の状態（決定 0214）
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class ReviewStatus {
        private ReviewStatus() {}

        public static final String OPEN = "open";

        public static final String RESOLVED = "resolved";

        public static final String CLOSED = "closed";
    }

    /** 読んだ bcfzip */
    public static final class BcfReadResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BcfReadResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BcfReadResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BcfReadResult(json);
        }

        /** 空のオブジェクトから作る */
        public BcfReadResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** BCF の版 */
        public String getVersion() {
            return Json.read(json.get("version"), Json::string);
        }

        /** {@link #getVersion()} を書く。null なら欄を除く */
        public BcfReadResult withVersion(String value) {
            put("version", value);
            return this;
        }

        /** 指摘の数 */
        public Long getTopics() {
            return Json.read(json.get("topics"), Json::integer);
        }

        /** {@link #getTopics()} を書く。null なら欄を除く */
        public BcfReadResult withTopics(Long value) {
            put("topics", value);
            return this;
        }
    }

    /** zm_push で渡したレビューの文書を、BCF 3.0 の bcfzip に書く（決定 0214・0220）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。小版 45 から */
    public static final class BcfWriteRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BcfWriteRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BcfWriteRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BcfWriteRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public BcfWriteRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "bcf_write");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public BcfWriteRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public BcfWriteRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public BcfWriteRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public BcfWriteRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public BcfWriteRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public BcfWriteRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** レビューの文書の入力の名前（zm_push で渡した JSON） */
        public String getReview() {
            return Json.read(json.get("review"), Json::string);
        }

        /** {@link #getReview()} を書く。null なら欄を除く */
        public BcfWriteRequest withReview(String value) {
            put("review", value);
            return this;
        }

        /** viewpoint にする 3D の場所の対象の ID */
        public String getTarget() {
            return Json.read(json.get("target"), Json::string);
        }

        /** {@link #getTarget()} を書く。null なら欄を除く */
        public BcfWriteRequest withTarget(String value) {
            put("target", value);
            return this;
        }

        /** 元の bcfzip の入力の名前（zm_push で渡した .bcf）。書かなければ新しく書く */
        public String getOriginal() {
            return Json.read(json.get("original"), Json::string);
        }

        /** {@link #getOriginal()} を書く。null なら欄を除く */
        public BcfWriteRequest withOriginal(String value) {
            put("original", value);
            return this;
        }

        /** 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す */
        public List<BcfStatus> getStatuses() {
            return Json.read(json.get("statuses"), v0 -> Json.list(v0, BcfStatus::of));
        }

        /** {@link #getStatuses()} を書く。null なら欄を除く */
        public BcfWriteRequest withStatuses(List<BcfStatus> value) {
            put("statuses", value);
            return this;
        }

        /** 種類のない指摘に書く TopicType */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public BcfWriteRequest withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 作者のない指摘とコメントに書く作者 */
        public String getAuthor() {
            return Json.read(json.get("author"), Json::string);
        }

        /** {@link #getAuthor()} を書く。null なら欄を除く */
        public BcfWriteRequest withAuthor(String value) {
            put("author", value);
            return this;
        }

        /** 日時のない指摘とコメントに書く日時（RFC 3339） */
        public String getDate() {
            return Json.read(json.get("date"), Json::string);
        }

        /** {@link #getDate()} を書く。null なら欄を除く */
        public BcfWriteRequest withDate(String value) {
            put("date", value);
            return this;
        }
    }

    /** 書いた bcfzip */
    public static final class BcfWriteResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BcfWriteResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BcfWriteResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BcfWriteResult(json);
        }

        /** 空のオブジェクトから作る */
        public BcfWriteResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 書いた指摘の数 */
        public Long getTopics() {
            return Json.read(json.get("topics"), Json::integer);
        }

        /** {@link #getTopics()} を書く。null なら欄を除く */
        public BcfWriteResult withTopics(Long value) {
            put("topics", value);
            return this;
        }
    }

    /** 表の項目 */
    public static final class TableItem extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public TableItem(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static TableItem of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new TableItem(json);
        }

        /** 空のオブジェクトから作る */
        public TableItem() {
            super(new LinkedHashMap<String, Object>());
        }

        /** ID。表のレコードがハンドルを持てば layer/h10・line-type/h14、なければ読んだ順の番号（layer/2）。寸法スタイルは dimension-style/h1A か、寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public TableItem withId(String value) {
            put("id", value);
            return this;
        }

        /** 元の名前。翻訳しない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public TableItem withName(String value) {
            put("name", value);
            return this;
        }

        /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public TableItem withHash(String value) {
            put("hash", value);
            return this;
        }
    }

    /** レイヤの表の項目（決定 0162）。小版 22 から、状態とスタイルを持つ */
    public static final class LayerItem extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LayerItem(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LayerItem of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LayerItem(json);
        }

        /** 空のオブジェクトから作る */
        public LayerItem() {
            super(new LinkedHashMap<String, Object>());
        }

        /** ID。表のレコードがハンドルを持てば layer/h10、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public LayerItem withId(String value) {
            put("id", value);
            return this;
        }

        /** 元の名前。翻訳しない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public LayerItem withName(String value) {
            put("name", value);
            return this;
        }

        /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public LayerItem withHash(String value) {
            put("hash", value);
            return this;
        }

        /** ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかの欄を持たない。小版 22 から */
        public Boolean getDefined() {
            return Json.read(json.get("defined"), Json::bool);
        }

        /** {@link #getDefined()} を書く。null なら欄を除く */
        public LayerItem withDefined(Boolean value) {
            put("defined", value);
            return this;
        }

        /** 表示する。小版 22 から */
        public Boolean getVisible() {
            return Json.read(json.get("visible"), Json::bool);
        }

        /** {@link #getVisible()} を書く。null なら欄を除く */
        public LayerItem withVisible(Boolean value) {
            put("visible", value);
            return this;
        }

        /** 印刷する。小版 22 から */
        public Boolean getPrintable() {
            return Json.read(json.get("printable"), Json::bool);
        }

        /** {@link #getPrintable()} を書く。null なら欄を除く */
        public LayerItem withPrintable(Boolean value) {
            put("printable", value);
            return this;
        }

        /** 編集できる。小版 22 から */
        public Boolean getEditable() {
            return Json.read(json.get("editable"), Json::bool);
        }

        /** {@link #getEditable()} を書く。null なら欄を除く */
        public LayerItem withEditable(Boolean value) {
            put("editable", value);
            return this;
        }

        /** 凍結する。小版 22 から */
        public Boolean getFrozen() {
            return Json.read(json.get("frozen"), Json::bool);
        }

        /** {@link #getFrozen()} を書く。null なら欄を除く */
        public LayerItem withFrozen(Boolean value) {
            put("frozen", value);
            return this;
        }

        /** 色。inherit を持たない。定義が色を持たなければ書かない。小版 22 から */
        public ColorChange getColor() {
            return Json.read(json.get("color"), ColorChange::of);
        }

        /** {@link #getColor()} を書く。null なら欄を除く */
        public LayerItem withColor(ColorChange value) {
            put("color", value);
            return this;
        }

        /** 線種の ID。定義が線種を持たなければ書かない。小版 22 から */
        public String getLineTypeId() {
            return Json.read(json.get("line_type_id"), Json::string);
        }

        /** {@link #getLineTypeId()} を書く。null なら欄を除く */
        public LayerItem withLineTypeId(String value) {
            put("line_type_id", value);
            return this;
        }

        /** 線幅。inherit を持たない。定義が線幅を持たなければ書かない。小版 22 から */
        public LineWeightChange getLineWeight() {
            return Json.read(json.get("line_weight"), LineWeightChange::of);
        }

        /** {@link #getLineWeight()} を書く。null なら欄を除く */
        public LayerItem withLineWeight(LineWeightChange value) {
            put("line_weight", value);
            return this;
        }
    }

    /** 線種の表の項目（決定 0164）。小版 26 から、定義を持つ */
    public static final class LineTypeItem extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LineTypeItem(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LineTypeItem of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LineTypeItem(json);
        }

        /** 空のオブジェクトから作る */
        public LineTypeItem() {
            super(new LinkedHashMap<String, Object>());
        }

        /** ID。表のレコードがハンドルを持てば line-type/h14、なければ読んだ順の番号（line-type/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public LineTypeItem withId(String value) {
            put("id", value);
            return this;
        }

        /** 元の名前。翻訳しない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public LineTypeItem withName(String value) {
            put("name", value);
            return this;
        }

        /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public LineTypeItem withHash(String value) {
            put("hash", value);
            return this;
        }

        /** ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかの欄を持たない。小版 26 から */
        public Boolean getDefined() {
            return Json.read(json.get("defined"), Json::bool);
        }

        /** {@link #getDefined()} を書く。null なら欄を除く */
        public LineTypeItem withDefined(Boolean value) {
            put("defined", value);
            return this;
        }

        /** 元の説明。小版 26 から */
        public String getDescription() {
            return Json.read(json.get("description"), Json::string);
        }

        /** {@link #getDescription()} を書く。null なら欄を除く */
        public LineTypeItem withDescription(String value) {
            put("description", value);
            return this;
        }

        /** 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。小版 26 から */
        public List<Double> getPattern() {
            return Json.read(json.get("pattern"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getPattern()} を書く。null なら欄を除く */
        public LineTypeItem withPattern(List<Double> value) {
            put("pattern", value);
            return this;
        }

        /** 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。小版 26 から */
        public Boolean getPaper() {
            return Json.read(json.get("paper"), Json::bool);
        }

        /** {@link #getPaper()} を書く。null なら欄を除く */
        public LineTypeItem withPaper(Boolean value) {
            put("paper", value);
            return this;
        }

        /** 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。小版 26 から */
        public Boolean getMarked() {
            return Json.read(json.get("marked"), Json::bool);
        }

        /** {@link #getMarked()} を書く。null なら欄を除く */
        public LineTypeItem withMarked(Boolean value) {
            put("marked", value);
            return this;
        }

        /** 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。小版 28 から */
        public List<LineTypeMark> getMarks() {
            return Json.read(json.get("marks"), v0 -> Json.list(v0, LineTypeMark::of));
        }

        /** {@link #getMarks()} を書く。null なら欄を除く */
        public LineTypeItem withMarks(List<LineTypeMark> value) {
            put("marks", value);
            return this;
        }
    }

    /** 寸法スタイルの表の項目（決定 0165）。小版 27 から、端部記号と大きさを持つ */
    public static final class DimensionStyleItem extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DimensionStyleItem(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DimensionStyleItem of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DimensionStyleItem(json);
        }

        /** 空のオブジェクトから作る */
        public DimensionStyleItem() {
            super(new LinkedHashMap<String, Object>());
        }

        /** ID。寸法スタイルのレコードがハンドルを持てば dimension-style/h1A、なければ寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public DimensionStyleItem withId(String value) {
            put("id", value);
            return this;
        }

        /** 元の名前。翻訳しない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public DimensionStyleItem withName(String value) {
            put("name", value);
            return this;
        }

        /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public DimensionStyleItem withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から */
        public List<String> getEnds() {
            return Json.read(json.get("ends"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getEnds()} を書く。null なら欄を除く */
        public DimensionStyleItem withEnds(List<String> value) {
            put("ends", value);
            return this;
        }

        /** 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から */
        public String getLeader() {
            return Json.read(json.get("leader"), Json::string);
        }

        /** {@link #getLeader()} を書く。null なら欄を除く */
        public DimensionStyleItem withLeader(String value) {
            put("leader", value);
            return this;
        }

        /** 端部記号の大きさ。文書の単位で、scale を掛ける前の値。小版 27 から */
        public Double getSize() {
            return Json.read(json.get("size"), Json::number);
        }

        /** {@link #getSize()} を書く。null なら欄を除く */
        public DimensionStyleItem withSize(Double value) {
            put("size", value);
            return this;
        }

        /** 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。小版 27 から */
        public Double getTick() {
            return Json.read(json.get("tick"), Json::number);
        }

        /** {@link #getTick()} を書く。null なら欄を除く */
        public DimensionStyleItem withTick(Double value) {
            put("tick", value);
            return this;
        }

        /** 大きさに掛ける倍率。0 はビューの縮尺から決める。小版 27 から */
        public Double getScale() {
            return Json.read(json.get("scale"), Json::number);
        }

        /** {@link #getScale()} を書く。null なら欄を除く */
        public DimensionStyleItem withScale(Double value) {
            put("scale", value);
            return this;
        }
    }

    /** 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。小版 28 から */
    public static final class LineTypeMark extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public LineTypeMark(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static LineTypeMark of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new LineTypeMark(json);
        }

        /** 空のオブジェクトから作る */
        public LineTypeMark() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 添える列の要素の番号（pattern の添字） */
        public Long getIndex() {
            return Json.read(json.get("index"), Json::integer);
        }

        /** {@link #getIndex()} を書く。null なら欄を除く */
        public LineTypeMark withIndex(Long value) {
            put("index", value);
            return this;
        }

        /** 形（shape）か文字（text） */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public LineTypeMark withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 形の形のファイル。元の形式が指す文字スタイルがなければ書かない */
        public Font getFont() {
            return Json.read(json.get("font"), Font::of);
        }

        /** {@link #getFont()} を書く。null なら欄を除く */
        public LineTypeMark withFont(Font value) {
            put("font", value);
            return this;
        }

        /** 形の番号 */
        public Long getNumber() {
            return Json.read(json.get("number"), Json::integer);
        }

        /** {@link #getNumber()} を書く。null なら欄を除く */
        public LineTypeMark withNumber(Long value) {
            put("number", value);
            return this;
        }

        /** 文字の文字列 */
        public String getText() {
            return Json.read(json.get("text"), Json::string);
        }

        /** {@link #getText()} を書く。null なら欄を除く */
        public LineTypeMark withText(String value) {
            put("text", value);
            return this;
        }

        /** 文字の文字スタイルの元の名前。元の形式が指す文字スタイルがなければ書かない */
        public String getStyle() {
            return Json.read(json.get("style"), Json::string);
        }

        /** {@link #getStyle()} を書く。null なら欄を除く */
        public LineTypeMark withStyle(String value) {
            put("style", value);
            return this;
        }

        /** 文字の文字スタイルの ID。tables の text_styles の ID で、名前で引いたもの（大文字と小文字を区別せず、同じ名前があれば先の項目） */
        public String getStyleId() {
            return Json.read(json.get("style_id"), Json::string);
        }

        /** {@link #getStyleId()} を書く。null なら欄を除く */
        public LineTypeMark withStyleId(String value) {
            put("style_id", value);
            return this;
        }

        /** 文字の高さ。文字スタイルが決まった高さを持てば、それに scale を掛けたもの。持たなければ scale。単位は pattern と同じ */
        public Double getHeight() {
            return Json.read(json.get("height"), Json::number);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public LineTypeMark withHeight(Double value) {
            put("height", value);
            return this;
        }

        /** 倍率。文字なら、文字スタイルの高さへの倍率 */
        public Double getScale() {
            return Json.read(json.get("scale"), Json::number);
        }

        /** {@link #getScale()} を書く。null なら欄を除く */
        public LineTypeMark withScale(Double value) {
            put("scale", value);
            return this;
        }

        /** 回転（度、反時計回り） */
        public Double getRotation() {
            return Json.read(json.get("rotation"), Json::number);
        }

        /** {@link #getRotation()} を書く。null なら欄を除く */
        public LineTypeMark withRotation(Double value) {
            put("rotation", value);
            return this;
        }

        /** 回転の基準 */
        public String getRotationBasis() {
            return Json.read(json.get("rotation_basis"), Json::string);
        }

        /** {@link #getRotationBasis()} を書く。null なら欄を除く */
        public LineTypeMark withRotationBasis(String value) {
            put("rotation_basis", value);
            return this;
        }

        /** 置く位置のずれ（x・y）。線の向きを x とする。単位は pattern と同じ */
        public List<Double> getOffset() {
            return Json.read(json.get("offset"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public LineTypeMark withOffset(List<Double> value) {
            put("offset", value);
            return this;
        }
    }

    /**
     * 線種に添えるものの種類。shape は形のファイルの形、text は文字
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class LineTypeMarkKind {
        private LineTypeMarkKind() {}

        public static final String SHAPE = "shape";

        public static final String TEXT = "text";
    }

    /**
     * 回転の基準。relative は線の向きから、absolute は文書の x 軸から測る。upright は線の向きから測り、文字が逆さに見えるなら半回転する
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class MarkRotation {
        private MarkRotation() {}

        public static final String RELATIVE = "relative";

        public static final String ABSOLUTE = "absolute";

        public static final String UPRIGHT = "upright";
    }

    /** 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。小版 20 から */
    public static final class TextStyleItem extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public TextStyleItem(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static TextStyleItem of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new TextStyleItem(json);
        }

        /** 空のオブジェクトから作る */
        public TextStyleItem() {
            super(new LinkedHashMap<String, Object>());
        }

        /** ID。表のレコードがハンドルを持てば text-style/h11、なければ文字スタイルの並びの中の順（text-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public TextStyleItem withId(String value) {
            put("id", value);
            return this;
        }

        /** 元の名前。翻訳しない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public TextStyleItem withName(String value) {
            put("name", value);
            return this;
        }

        /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public TextStyleItem withHash(String value) {
            put("hash", value);
            return this;
        }

        /** ファイルに定義がある。偽なら、ほかの欄を持たない */
        public Boolean getDefined() {
            return Json.read(json.get("defined"), Json::bool);
        }

        /** {@link #getDefined()} を書く。null なら欄を除く */
        public TextStyleItem withDefined(Boolean value) {
            put("defined", value);
            return this;
        }

        /** 主のフォント。ファイルが決めていなければ書かない */
        public Font getFont() {
            return Json.read(json.get("font"), Font::of);
        }

        /** {@link #getFont()} を書く。null なら欄を除く */
        public TextStyleItem withFont(Font value) {
            put("font", value);
            return this;
        }

        /** 大きいフォント（漢字などの形のファイル）。なければ書かない */
        public Font getBigFont() {
            return Json.read(json.get("big_font"), Font::of);
        }

        /** {@link #getBigFont()} を書く。null なら欄を除く */
        public TextStyleItem withBigFont(Font value) {
            put("big_font", value);
            return this;
        }

        /** 決まった高さ。文書の単位。決めなければ書かない */
        public Double getHeight() {
            return Json.read(json.get("height"), Json::number);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public TextStyleItem withHeight(Double value) {
            put("height", value);
            return this;
        }

        /** 幅の倍率 */
        public Double getWidth() {
            return Json.read(json.get("width"), Json::number);
        }

        /** {@link #getWidth()} を書く。null なら欄を除く */
        public TextStyleItem withWidth(Double value) {
            put("width", value);
            return this;
        }

        /** 傾き（度） */
        public Double getOblique() {
            return Json.read(json.get("oblique"), Json::number);
        }

        /** {@link #getOblique()} を書く。null なら欄を除く */
        public TextStyleItem withOblique(Double value) {
            put("oblique", value);
            return this;
        }

        /** 左右を裏返す */
        public Boolean getBackward() {
            return Json.read(json.get("backward"), Json::bool);
        }

        /** {@link #getBackward()} を書く。null なら欄を除く */
        public TextStyleItem withBackward(Boolean value) {
            put("backward", value);
            return this;
        }

        /** 上下を反転する */
        public Boolean getUpsideDown() {
            return Json.read(json.get("upside_down"), Json::bool);
        }

        /** {@link #getUpsideDown()} を書く。null なら欄を除く */
        public TextStyleItem withUpsideDown(Boolean value) {
            put("upside_down", value);
            return this;
        }

        /** 縦書き */
        public Boolean getVertical() {
            return Json.read(json.get("vertical"), Json::bool);
        }

        /** {@link #getVertical()} を書く。null なら欄を除く */
        public TextStyleItem withVertical(Boolean value) {
            put("vertical", value);
            return this;
        }
    }

    /** フォントの指定。取得と字形の評価は使い手が行う */
    public static final class Font extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Font(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Font of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Font(json);
        }

        /** 空のオブジェクトから作る */
        public Font() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 元の名前。形のファイルならファイルの名前、名前で指すフォントなら書体の名前。翻訳しない */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public Font withName(String value) {
            put("name", value);
            return this;
        }

        /** 種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public Font withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 太字。名前で指すフォントだけ */
        public Boolean getBold() {
            return Json.read(json.get("bold"), Json::bool);
        }

        /** {@link #getBold()} を書く。null なら欄を除く */
        public Font withBold(Boolean value) {
            put("bold", value);
            return this;
        }

        /** 斜体。名前で指すフォントだけ */
        public Boolean getItalic() {
            return Json.read(json.get("italic"), Json::bool);
        }

        /** {@link #getItalic()} を書く。null なら欄を除く */
        public Font withItalic(Boolean value) {
            put("italic", value);
            return this;
        }

        /** 文字集合の番号（128 は Shift_JIS）。名前で指すフォントで、元の形式が決めていれば */
        public Long getCharset() {
            return Json.read(json.get("charset"), Json::integer);
        }

        /** {@link #getCharset()} を書く。null なら欄を除く */
        public Font withCharset(Long value) {
            put("charset", value);
            return this;
        }

        /** ピッチと系統の番号。名前で指すフォントで、元の形式が決めていれば */
        public Long getPitch() {
            return Json.read(json.get("pitch"), Json::integer);
        }

        /** {@link #getPitch()} を書く。null なら欄を除く */
        public Font withPitch(Long value) {
            put("pitch", value);
            return this;
        }
    }

    /**
     * フォントの種類。shape は形のファイル（SHX など）、named は名前で指すフォント（TrueType など）
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class FontKind {
        private FontKind() {}

        public static final String SHAPE = "shape";

        public static final String NAMED = "named";
    }

    /** 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つ鍵（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない */
    public static final class Source extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Source(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Source of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Source(json);
        }

        /** 空のオブジェクトから作る */
        public Source() {
            super(new LinkedHashMap<String, Object>());
        }

        /** L1 のレコードの番号 */
        public Long getRecord() {
            return Json.read(json.get("record"), Json::integer);
        }

        /** {@link #getRecord()} を書く。null なら欄を除く */
        public Source withRecord(Long value) {
            put("record", value);
            return this;
        }

        /** L1 のバイト範囲 */
        public ByteRange getBytes() {
            return Json.read(json.get("bytes"), ByteRange::of);
        }

        /** {@link #getBytes()} を書く。null なら欄を除く */
        public Source withBytes(ByteRange value) {
            put("bytes", value);
            return this;
        }

        /** AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない */
        public String getHandle() {
            return Json.read(json.get("handle"), Json::string);
        }

        /** {@link #getHandle()} を書く。null なら欄を除く */
        public Source withHandle(String value) {
            put("handle", value);
            return this;
        }

        /** SXF のインスタンスの番号（#n の n）。10 進の文字列。小版 6 から */
        public String getInstance() {
            return Json.read(json.get("instance"), Json::string);
        }

        /** {@link #getInstance()} を書く。null なら欄を除く */
        public Source withInstance(String value) {
            put("instance", value);
            return this;
        }
    }

    /** 応答。失敗したときも同じ形で、result を持たない */
    public static final class Response extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Response(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Response of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Response(json);
        }

        /** メッセージのスキーマの大版 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public Response withV(Long value) {
            put("v", value);
            return this;
        }

        /** 診断。入力の順 */
        public List<Diagnostic> getDiagnostics() {
            return Json.read(json.get("diagnostics"), v0 -> Json.list(v0, Diagnostic::of));
        }

        /** {@link #getDiagnostics()} を書く。null なら欄を除く */
        public Response withDiagnostics(List<Diagnostic> value) {
            put("diagnostics", value);
            return this;
        }

        /** バイナリの結果。take で取り出す。次の call で捨てる */
        public List<Blob> getBlobs() {
            return Json.read(json.get("blobs"), v0 -> Json.list(v0, Blob::of));
        }

        /** {@link #getBlobs()} を書く。null なら欄を除く */
        public Response withBlobs(List<Blob> value) {
            put("blobs", value);
            return this;
        }

        /** op ごとの結果（<op>_result の形の JSON のオブジェクト）。失敗したときは null */
        public Map<String, Object> getResult() {
            return Json.object(json.get("result"));
        }

        /**
         * op ごとの結果を、その op の結果の型で読む（例: {@code response.result(OpenResult::of)}）
         *
         * 失敗したときは null
         */
        public <T> T result(java.util.function.Function<Object, T> read) {
            return Json.read(json.get("result"), read);
        }
    }

    /** 診断。人向けの文は持たず、文は describe で組み立てる（決定 0009） */
    public static final class Diagnostic extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Diagnostic(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Diagnostic of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Diagnostic(json);
        }

        /** 空のオブジェクトから作る */
        public Diagnostic() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 変わらない名前（budget.records.exceeded など）。catalog/codes.toml にある */
        public String getCode() {
            return Json.read(json.get("code"), Json::string);
        }

        /** {@link #getCode()} を書く。null なら欄を除く */
        public Diagnostic withCode(String value) {
            put("code", value);
            return this;
        }

        /** 重さ。code ごとに決まる */
        public String getSeverity() {
            return Json.read(json.get("severity"), Json::string);
        }

        /** {@link #getSeverity()} を書く。null なら欄を除く */
        public Diagnostic withSeverity(String value) {
            put("severity", value);
            return this;
        }

        /** 変換の結果の段。code ごとに決まる。変換の結果を表さない code では持たない */
        public String getFidelity() {
            return Json.read(json.get("fidelity"), Json::string);
        }

        /** {@link #getFidelity()} を書く。null なら欄を除く */
        public Diagnostic withFidelity(String value) {
            put("fidelity", value);
            return this;
        }

        /** まとめた件数 */
        public Long getCount() {
            return Json.read(json.get("count"), Json::integer);
        }

        /** {@link #getCount()} を書く。null なら欄を除く */
        public Diagnostic withCount(Long value) {
            put("count", value);
            return this;
        }

        /** 場所。入力の順に先頭の数件 */
        public List<Place> getAt() {
            return Json.read(json.get("at"), v0 -> Json.list(v0, Place::of));
        }

        /** {@link #getAt()} を書く。null なら欄を除く */
        public Diagnostic withAt(List<Place> value) {
            put("at", value);
            return this;
        }

        /** 仕様の節 */
        public Cite getCite() {
            return Json.read(json.get("cite"), Cite::of);
        }

        /** {@link #getCite()} を書く。null なら欄を除く */
        public Diagnostic withCite(Cite value) {
            put("cite", value);
            return this;
        }

        /** 文に差し込む値。名前と型は code ごとに台帳が決める。2^53 − 1 を超える整数は文字列にする */
        public Map<String, Object> getParams() {
            return Json.read(json.get("params"), v0 -> Json.map(v0, Json::value));
        }

        /** {@link #getParams()} を書く。null なら欄を除く */
        public Diagnostic withParams(Map<String, Object> value) {
            put("params", value);
            return this;
        }
    }

    /**
     * 診断の重さ（決定 0009）。info は何も失っていない、warning は結果を使える、error は失敗した
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class Severity {
        private Severity() {}

        public static final String INFO = "info";

        public static final String WARNING = "warning";

        public static final String ERROR = "error";
    }

    /**
     * 変換の結果の段のうち、`native` 以外のもの（決定 0009）
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class Fidelity {
        private Fidelity() {}

        public static final String EQUIVALENT = "equivalent";

        public static final String APPROXIMATED = "approximated";

        public static final String DRAWN = "drawn";

        public static final String DROPPED = "dropped";
    }

    /** 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所） */
    public static final class Place extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Place(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Place of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Place(json);
        }

        /** 空のオブジェクトから作る */
        public Place() {
            super(new LinkedHashMap<String, Object>());
        }

        /** ID */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public Place withId(String value) {
            put("id", value);
            return this;
        }

        /** L1 のレコードの番号 */
        public Long getRecord() {
            return Json.read(json.get("record"), Json::integer);
        }

        /** {@link #getRecord()} を書く。null なら欄を除く */
        public Place withRecord(Long value) {
            put("record", value);
            return this;
        }

        /** L1 のバイト範囲 */
        public ByteRange getBytes() {
            return Json.read(json.get("bytes"), ByteRange::of);
        }

        /** {@link #getBytes()} を書く。null なら欄を除く */
        public Place withBytes(ByteRange value) {
            put("bytes", value);
            return this;
        }
    }

    /** バイト範囲（start から end の手前まで） */
    public static final class ByteRange extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public ByteRange(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static ByteRange of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new ByteRange(json);
        }

        /** 空のオブジェクトから作る */
        public ByteRange() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 先頭の位置 */
        public Long getStart() {
            return Json.read(json.get("start"), Json::integer);
        }

        /** {@link #getStart()} を書く。null なら欄を除く */
        public ByteRange withStart(Long value) {
            put("start", value);
            return this;
        }

        /** 終わりの位置（含まない） */
        public Long getEnd() {
            return Json.read(json.get("end"), Json::integer);
        }

        /** {@link #getEnd()} を書く。null なら欄を除く */
        public ByteRange withEnd(Long value) {
            put("end", value);
            return this;
        }
    }

    /** 仕様の節。仕様の本文は写さない */
    public static final class Cite extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Cite(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Cite of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Cite(json);
        }

        /** 空のオブジェクトから作る */
        public Cite() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 仕様の一覧での名前 */
        public String getSpec() {
            return Json.read(json.get("spec"), Json::string);
        }

        /** {@link #getSpec()} を書く。null なら欄を除く */
        public Cite withSpec(String value) {
            put("spec", value);
            return this;
        }

        /** 節 */
        public String getSection() {
            return Json.read(json.get("section"), Json::string);
        }

        /** {@link #getSection()} を書く。null なら欄を除く */
        public Cite withSection(String value) {
            put("section", value);
            return this;
        }
    }

    /** バイナリの結果 */
    public static final class Blob extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Blob(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Blob of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Blob(json);
        }

        /** 空のオブジェクトから作る */
        public Blob() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 番号。zm_take に渡す */
        public Long getId() {
            return Json.read(json.get("id"), Json::integer);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public Blob withId(Long value) {
            put("id", value);
            return this;
        }

        /** 種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public Blob withKind(String value) {
            put("kind", value);
            return this;
        }

        /** バイト数 */
        public Long getSize() {
            return Json.read(json.get("size"), Json::integer);
        }

        /** {@link #getSize()} を書く。null なら欄を除く */
        public Blob withSize(Long value) {
            put("size", value);
            return this;
        }
    }

    /**
     * バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class BlobKind {
        private BlobKind() {}

        public static final String FILE = "file";

        public static final String IMAGE = "image";

        public static final String SCENE = "scene";

        public static final String ARROW = "arrow";
    }

    /** zm_session_new に渡す設定。渡さなければ既定の設定 */
    public static final class SessionOptions extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SessionOptions(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SessionOptions of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SessionOptions(json);
        }

        /** 空のオブジェクトから作る */
        public SessionOptions() {
            super(new LinkedHashMap<String, Object>());
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public SessionOptions withV(Long value) {
            put("v", value);
            return this;
        }

        /** 実行の形。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013） */
        public String getEngine() {
            return Json.read(json.get("engine"), Json::string);
        }

        /** {@link #getEngine()} を書く。null なら欄を除く */
        public SessionOptions withEngine(String value) {
            put("engine", value);
            return this;
        }

        /** engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200） */
        public String getProcessPath() {
            return Json.read(json.get("process_path"), Json::string);
        }

        /** {@link #getProcessPath()} を書く。null なら欄を除く */
        public SessionOptions withProcessPath(String value) {
            put("process_path", value);
            return this;
        }
    }

    /**
     * 実行の形（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class Engine {
        private Engine() {}

        public static final String NATIVE = "native";

        public static final String PROCESS = "process";

        public static final String WASM = "wasm";
    }

    /** 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面のモジュールに置く。小版 46 から（doc と id は小版 47 から） */
    public static final class CivilRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public CivilRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "civil");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public CivilRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public CivilRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public CivilRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public CivilRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public CivilRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public CivilRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 線形。doc と一緒には書かない */
        public CivilAlignment getAlignment() {
            return Json.read(json.get("alignment"), CivilAlignment::of);
        }

        /** {@link #getAlignment()} を書く。null なら欄を除く */
        public CivilRequest withAlignment(CivilAlignment value) {
            put("alignment", value);
            return this;
        }

        /** open で開いた J-LandXML の文書の名前。小版 47 から */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public CivilRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** doc の線形の ID。文書に線形が 1 つなら書かなくてよい。小版 47 から */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public CivilRequest withId(String value) {
            put("id", value);
            return this;
        }

        /** 追加距離（メートル）の列 */
        public List<Double> getDistances() {
            return Json.read(json.get("distances"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getDistances()} を書く。null なら欄を除く */
        public CivilRequest withDistances(List<Double> value) {
            put("distances", value);
            return this;
        }

        /** 測点の表記（No.12+5.000・No.12+5・No.12）の列。測点の不連続のために同じ値が何か所にもあれば、最初の所にして診断する */
        public List<String> getStations() {
            return Json.read(json.get("stations"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getStations()} を書く。null なら欄を除く */
        public CivilRequest withStations(List<String> value) {
            put("stations", value);
            return this;
        }

        /** 線形の初めから終わりまで、この間隔（メートル）ごとの所を足す。終わりの所も足す */
        public Double getInterval() {
            return Json.read(json.get("interval"), Json::number);
        }

        /** {@link #getInterval()} を書く。null なら欄を除く */
        public CivilRequest withInterval(Double value) {
            put("interval", value);
            return this;
        }

        /** 測点の間隔（メートル）。書かなければ 20 */
        public Double getPitch() {
            return Json.read(json.get("pitch"), Json::number);
        }

        /** {@link #getPitch()} を書く。null なら欄を除く */
        public CivilRequest withPitch(Double value) {
            put("pitch", value);
            return this;
        }

        /** 測点の表記の端数の小数の桁数。書かなければ 3 */
        public Long getDigits() {
            return Json.read(json.get("digits"), Json::integer);
        }

        /** {@link #getDigits()} を書く。null なら欄を除く */
        public CivilRequest withDigits(Long value) {
            put("digits", value);
            return this;
        }
    }

    /** 線形。点は地図の空間（北と東、メートル）に置き、方向角は北から時計回りのラジアンにする。半径は、右へ曲がるものを正にする（決定 0230） */
    public static final class CivilAlignment extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilAlignment(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilAlignment of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilAlignment(json);
        }

        /** 空のオブジェクトから作る */
        public CivilAlignment() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 平面線形の区間。線形の初めから順に並べる */
        public List<CivilHorizontal> getHorizontal() {
            return Json.read(json.get("horizontal"), v0 -> Json.list(v0, CivilHorizontal::of));
        }

        /** {@link #getHorizontal()} を書く。null なら欄を除く */
        public CivilAlignment withHorizontal(List<CivilHorizontal> value) {
            put("horizontal", value);
            return this;
        }

        /** 縦断線形の区間。書かなければ高さを返さない */
        public List<CivilVertical> getVertical() {
            return Json.read(json.get("vertical"), v0 -> Json.list(v0, CivilVertical::of));
        }

        /** {@link #getVertical()} を書く。null なら欄を除く */
        public CivilAlignment withVertical(List<CivilVertical> value) {
            put("vertical", value);
            return this;
        }

        /** 線形の初めの測点の値（メートル）。書かなければ 0 */
        public Double getStartStation() {
            return Json.read(json.get("start_station"), Json::number);
        }

        /** {@link #getStartStation()} を書く。null なら欄を除く */
        public CivilAlignment withStartStation(Double value) {
            put("start_station", value);
            return this;
        }

        /** 測点の不連続。追加距離の順に並べる */
        public List<CivilStationBreak> getBreaks() {
            return Json.read(json.get("breaks"), v0 -> Json.list(v0, CivilStationBreak::of));
        }

        /** {@link #getBreaks()} を書く。null なら欄を除く */
        public CivilAlignment withBreaks(List<CivilStationBreak> value) {
            put("breaks", value);
            return this;
        }
    }

    /** 平面線形の区間 */
    public static final class CivilHorizontal extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilHorizontal(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilHorizontal of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilHorizontal(json);
        }

        /** 空のオブジェクトから作る */
        public CivilHorizontal() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 区間の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public CivilHorizontal withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 初めの点の北（メートル） */
        public Double getNorthing() {
            return Json.read(json.get("northing"), Json::number);
        }

        /** {@link #getNorthing()} を書く。null なら欄を除く */
        public CivilHorizontal withNorthing(Double value) {
            put("northing", value);
            return this;
        }

        /** 初めの点の東（メートル） */
        public Double getEasting() {
            return Json.read(json.get("easting"), Json::number);
        }

        /** {@link #getEasting()} を書く。null なら欄を除く */
        public CivilHorizontal withEasting(Double value) {
            put("easting", value);
            return this;
        }

        /** 初めの方向角（北から時計回りのラジアン） */
        public Double getDirection() {
            return Json.read(json.get("direction"), Json::number);
        }

        /** {@link #getDirection()} を書く。null なら欄を除く */
        public CivilHorizontal withDirection(Double value) {
            put("direction", value);
            return this;
        }

        /** 長さ（メートル） */
        public Double getLength() {
            return Json.read(json.get("length"), Json::number);
        }

        /** {@link #getLength()} を書く。null なら欄を除く */
        public CivilHorizontal withLength(Double value) {
            put("length", value);
            return this;
        }

        /** 円弧の半径（メートル）。arc に書く */
        public Double getRadius() {
            return Json.read(json.get("radius"), Json::number);
        }

        /** {@link #getRadius()} を書く。null なら欄を除く */
        public CivilHorizontal withRadius(Double value) {
            put("radius", value);
            return this;
        }

        /** 緩和曲線の初めの半径（メートル）。直線の側は書かない */
        public Double getStartRadius() {
            return Json.read(json.get("start_radius"), Json::number);
        }

        /** {@link #getStartRadius()} を書く。null なら欄を除く */
        public CivilHorizontal withStartRadius(Double value) {
            put("start_radius", value);
            return this;
        }

        /** 緩和曲線の終わりの半径（メートル）。直線の側は書かない */
        public Double getEndRadius() {
            return Json.read(json.get("end_radius"), Json::number);
        }

        /** {@link #getEndRadius()} を書く。null なら欄を除く */
        public CivilHorizontal withEndRadius(Double value) {
            put("end_radius", value);
            return this;
        }

        /** 緩和曲線の種類。transition に書き、書かなければ clothoid */
        public String getCurve() {
            return Json.read(json.get("curve"), Json::string);
        }

        /** {@link #getCurve()} を書く。null なら欄を除く */
        public CivilHorizontal withCurve(String value) {
            put("curve", value);
            return this;
        }
    }

    /** 縦断線形の区間 */
    public static final class CivilVertical extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilVertical(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilVertical of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilVertical(json);
        }

        /** 空のオブジェクトから作る */
        public CivilVertical() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 区間の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public CivilVertical withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 初めの追加距離（メートル） */
        public Double getStartDistance() {
            return Json.read(json.get("start_distance"), Json::number);
        }

        /** {@link #getStartDistance()} を書く。null なら欄を除く */
        public CivilVertical withStartDistance(Double value) {
            put("start_distance", value);
            return this;
        }

        /** 初めの高さ（メートル） */
        public Double getStartHeight() {
            return Json.read(json.get("start_height"), Json::number);
        }

        /** {@link #getStartHeight()} を書く。null なら欄を除く */
        public CivilVertical withStartHeight(Double value) {
            put("start_height", value);
            return this;
        }

        /** 初めの勾配（高さ ÷ 平面の距離） */
        public Double getStartGradient() {
            return Json.read(json.get("start_gradient"), Json::number);
        }

        /** {@link #getStartGradient()} を書く。null なら欄を除く */
        public CivilVertical withStartGradient(Double value) {
            put("start_gradient", value);
            return this;
        }

        /** 平面の距離での長さ（メートル） */
        public Double getLength() {
            return Json.read(json.get("length"), Json::number);
        }

        /** {@link #getLength()} を書く。null なら欄を除く */
        public CivilVertical withLength(Double value) {
            put("length", value);
            return this;
        }

        /** 放物線の終わりの勾配。parabola に書く */
        public Double getEndGradient() {
            return Json.read(json.get("end_gradient"), Json::number);
        }

        /** {@link #getEndGradient()} を書く。null なら欄を除く */
        public CivilVertical withEndGradient(Double value) {
            put("end_gradient", value);
            return this;
        }

        /** 円弧の半径（メートル）。上に凸のものを正にする。circular_arc に書く */
        public Double getRadius() {
            return Json.read(json.get("radius"), Json::number);
        }

        /** {@link #getRadius()} を書く。null なら欄を除く */
        public CivilVertical withRadius(Double value) {
            put("radius", value);
            return this;
        }
    }

    /**
     * 平面線形の区間の種類。直線・円弧・緩和曲線
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class CivilHorizontalKind {
        private CivilHorizontalKind() {}

        public static final String LINE = "line";

        public static final String ARC = "arc";

        public static final String TRANSITION = "transition";
    }

    /**
     * 緩和曲線の種類。クロソイド・3 次放物線・サイン半波長逓減曲線・コサイン曲線・ブロス曲線・ヘルマート曲線・ウィーン曲線
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class CivilTransitionCurve {
        private CivilTransitionCurve() {}

        public static final String CLOTHOID = "clothoid";

        public static final String CUBIC_PARABOLA = "cubic_parabola";

        public static final String SINE_HALF_WAVE = "sine_half_wave";

        public static final String COSINE = "cosine";

        public static final String BLOSS = "bloss";

        public static final String HELMERT = "helmert";

        public static final String VIENNESE = "viennese";
    }

    /**
     * 縦断線形の区間の種類。一定の勾配・2 次放物線・円弧
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class CivilVerticalKind {
        private CivilVerticalKind() {}

        public static final String CONSTANT_GRADIENT = "constant_gradient";

        public static final String PARABOLA = "parabola";

        public static final String CIRCULAR_ARC = "circular_arc";
    }

    /** 測点の不連続 */
    public static final class CivilStationBreak extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilStationBreak(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilStationBreak of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilStationBreak(json);
        }

        /** 空のオブジェクトから作る */
        public CivilStationBreak() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 追加距離（メートル） */
        public Double getDistance() {
            return Json.read(json.get("distance"), Json::number);
        }

        /** {@link #getDistance()} を書く。null なら欄を除く */
        public CivilStationBreak withDistance(Double value) {
            put("distance", value);
            return this;
        }

        /** 戻りの測点の値（不連続の手前から数えた値） */
        public Double getBack() {
            return Json.read(json.get("back"), Json::number);
        }

        /** {@link #getBack()} を書く。null なら欄を除く */
        public CivilStationBreak withBack(Double value) {
            put("back", value);
            return this;
        }

        /** 進みの測点の値（不連続の先で数え直す値） */
        public Double getAhead() {
            return Json.read(json.get("ahead"), Json::number);
        }

        /** {@link #getAhead()} を書く。null なら欄を除く */
        public CivilStationBreak withAhead(Double value) {
            put("ahead", value);
            return this;
        }
    }

    /** 線形の上の所 */
    public static final class CivilResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilResult(json);
        }

        /** 空のオブジェクトから作る */
        public CivilResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 平面線形の長さ（メートル） */
        public Double getLength() {
            return Json.read(json.get("length"), Json::number);
        }

        /** {@link #getLength()} を書く。null なら欄を除く */
        public CivilResult withLength(Double value) {
            put("length", value);
            return this;
        }

        /** 求めた所 */
        public List<CivilPosition> getPositions() {
            return Json.read(json.get("positions"), v0 -> Json.list(v0, CivilPosition::of));
        }

        /** {@link #getPositions()} を書く。null なら欄を除く */
        public CivilResult withPositions(List<CivilPosition> value) {
            put("positions", value);
            return this;
        }

        /** 所を求めた doc の線形の ID。小版 47 から */
        public String getAlignment() {
            return Json.read(json.get("alignment"), Json::string);
        }

        /** {@link #getAlignment()} を書く。null なら欄を除く */
        public CivilResult withAlignment(String value) {
            put("alignment", value);
            return this;
        }

        /** doc の線形の要約。doc を渡したときに返す。小版 47 から */
        public List<CivilAlignmentSummary> getAlignments() {
            return Json.read(json.get("alignments"), v0 -> Json.list(v0, CivilAlignmentSummary::of));
        }

        /** {@link #getAlignments()} を書く。null なら欄を除く */
        public CivilResult withAlignments(List<CivilAlignmentSummary> value) {
            put("alignments", value);
            return this;
        }

        /** doc の地表面（TIN）の要約。doc を渡したときに返す。小版 47 から */
        public List<CivilSurfaceSummary> getSurfaces() {
            return Json.read(json.get("surfaces"), v0 -> Json.list(v0, CivilSurfaceSummary::of));
        }

        /** {@link #getSurfaces()} を書く。null なら欄を除く */
        public CivilResult withSurfaces(List<CivilSurfaceSummary> value) {
            put("surfaces", value);
            return this;
        }

        /** doc の水平の座標系の EPSG のコード。分からなければ書かない。小版 47 から */
        public Long getCrs() {
            return Json.read(json.get("crs"), Json::integer);
        }

        /** {@link #getCrs()} を書く。null なら欄を除く */
        public CivilResult withCrs(Long value) {
            put("crs", value);
            return this;
        }
    }

    /** 線形の要約 */
    public static final class CivilAlignmentSummary extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilAlignmentSummary(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilAlignmentSummary of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilAlignmentSummary(json);
        }

        /** 空のオブジェクトから作る */
        public CivilAlignmentSummary() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 線形の ID */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public CivilAlignmentSummary withId(String value) {
            put("id", value);
            return this;
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public CivilAlignmentSummary withName(String value) {
            put("name", value);
            return this;
        }

        /** 平面線形の長さ（メートル） */
        public Double getLength() {
            return Json.read(json.get("length"), Json::number);
        }

        /** {@link #getLength()} を書く。null なら欄を除く */
        public CivilAlignmentSummary withLength(Double value) {
            put("length", value);
            return this;
        }

        /** 線形の初めの測点の値（メートル） */
        public Double getStartStation() {
            return Json.read(json.get("start_station"), Json::number);
        }

        /** {@link #getStartStation()} を書く。null なら欄を除く */
        public CivilAlignmentSummary withStartStation(Double value) {
            put("start_station", value);
            return this;
        }

        /** 平面線形の区間の数 */
        public Long getHorizontal() {
            return Json.read(json.get("horizontal"), Json::integer);
        }

        /** {@link #getHorizontal()} を書く。null なら欄を除く */
        public CivilAlignmentSummary withHorizontal(Long value) {
            put("horizontal", value);
            return this;
        }

        /** 縦断線形の区間の数 */
        public Long getVertical() {
            return Json.read(json.get("vertical"), Json::integer);
        }

        /** {@link #getVertical()} を書く。null なら欄を除く */
        public CivilAlignmentSummary withVertical(Long value) {
            put("vertical", value);
            return this;
        }

        /** 測点の不連続の数 */
        public Long getBreaks() {
            return Json.read(json.get("breaks"), Json::integer);
        }

        /** {@link #getBreaks()} を書く。null なら欄を除く */
        public CivilAlignmentSummary withBreaks(Long value) {
            put("breaks", value);
            return this;
        }

        /** 横断の数 */
        public Long getCrossSections() {
            return Json.read(json.get("cross_sections"), Json::integer);
        }

        /** {@link #getCrossSections()} を書く。null なら欄を除く */
        public CivilAlignmentSummary withCrossSections(Long value) {
            put("cross_sections", value);
            return this;
        }
    }

    /** 地表面（TIN）の要約 */
    public static final class CivilSurfaceSummary extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilSurfaceSummary(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilSurfaceSummary of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilSurfaceSummary(json);
        }

        /** 空のオブジェクトから作る */
        public CivilSurfaceSummary() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 地表面の ID */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public CivilSurfaceSummary withId(String value) {
            put("id", value);
            return this;
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public CivilSurfaceSummary withName(String value) {
            put("name", value);
            return this;
        }

        /** 役割。existing（現況）・design（計画）か、元の形式の値 */
        public String getRole() {
            return Json.read(json.get("role"), Json::string);
        }

        /** {@link #getRole()} を書く。null なら欄を除く */
        public CivilSurfaceSummary withRole(String value) {
            put("role", value);
            return this;
        }

        /** 点の数 */
        public Long getPoints() {
            return Json.read(json.get("points"), Json::integer);
        }

        /** {@link #getPoints()} を書く。null なら欄を除く */
        public CivilSurfaceSummary withPoints(Long value) {
            put("points", value);
            return this;
        }

        /** 三角形の数 */
        public Long getTriangles() {
            return Json.read(json.get("triangles"), Json::integer);
        }

        /** {@link #getTriangles()} を書く。null なら欄を除く */
        public CivilSurfaceSummary withTriangles(Long value) {
            put("triangles", value);
            return this;
        }
    }

    /** 線形の上の 1 つの所 */
    public static final class CivilPosition extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilPosition(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilPosition of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilPosition(json);
        }

        /** 空のオブジェクトから作る */
        public CivilPosition() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 追加距離（メートル） */
        public Double getDistance() {
            return Json.read(json.get("distance"), Json::number);
        }

        /** {@link #getDistance()} を書く。null なら欄を除く */
        public CivilPosition withDistance(Double value) {
            put("distance", value);
            return this;
        }

        /** 測点の値（メートル） */
        public Double getStation() {
            return Json.read(json.get("station"), Json::number);
        }

        /** {@link #getStation()} を書く。null なら欄を除く */
        public CivilPosition withStation(Double value) {
            put("station", value);
            return this;
        }

        /** 測点の表記。表記にできなければ書かない */
        public String getLabel() {
            return Json.read(json.get("label"), Json::string);
        }

        /** {@link #getLabel()} を書く。null なら欄を除く */
        public CivilPosition withLabel(String value) {
            put("label", value);
            return this;
        }

        /** 点の北（メートル） */
        public Double getNorthing() {
            return Json.read(json.get("northing"), Json::number);
        }

        /** {@link #getNorthing()} を書く。null なら欄を除く */
        public CivilPosition withNorthing(Double value) {
            put("northing", value);
            return this;
        }

        /** 点の東（メートル） */
        public Double getEasting() {
            return Json.read(json.get("easting"), Json::number);
        }

        /** {@link #getEasting()} を書く。null なら欄を除く */
        public CivilPosition withEasting(Double value) {
            put("easting", value);
            return this;
        }

        /** 接線の方向角（北から時計回りのラジアン） */
        public Double getDirection() {
            return Json.read(json.get("direction"), Json::number);
        }

        /** {@link #getDirection()} を書く。null なら欄を除く */
        public CivilPosition withDirection(Double value) {
            put("direction", value);
            return this;
        }

        /** 縦断線形の高さ（メートル）。求められなければ書かない */
        public Double getHeight() {
            return Json.read(json.get("height"), Json::number);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public CivilPosition withHeight(Double value) {
            put("height", value);
            return this;
        }

        /** 縦断線形の勾配。高さと同じく、求められなければ書かない */
        public Double getGradient() {
            return Json.read(json.get("gradient"), Json::number);
        }

        /** {@link #getGradient()} を書く。null なら欄を除く */
        public CivilPosition withGradient(Double value) {
            put("gradient", value);
            return this;
        }
    }

    /** open で開いた点群（LAS・LAZ）の要約と、条件に合う点を返す。点は八分木の浅い段の節から順に選ぶ（決定 0232）。図面のモジュールに置く。小版 48 から */
    public static final class PointcloudRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PointcloudRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PointcloudRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PointcloudRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public PointcloudRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "pointcloud");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public PointcloudRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public PointcloudRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public PointcloudRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public PointcloudRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public PointcloudRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public PointcloudRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** open で開いた点群の文書の名前 */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public PointcloudRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 点を選ぶ範囲。書かなければ全体 */
        public PointcloudBox getRegion() {
            return Json.read(json.get("region"), PointcloudBox::of);
        }

        /** {@link #getRegion()} を書く。null なら欄を除く */
        public PointcloudRequest withRegion(PointcloudBox value) {
            put("region", value);
            return this;
        }

        /** 許容差（点の間隔、メートル）。これより細かい段の点を選ばない。書かなければすべての段 */
        public Double getTolerance() {
            return Json.read(json.get("tolerance"), Json::number);
        }

        /** {@link #getTolerance()} を書く。null なら欄を除く */
        public PointcloudRequest withTolerance(Double value) {
            put("tolerance", value);
            return this;
        }

        /** 返す点の数の上限。書かなければ点を返さず、要約だけを返す */
        public Long getLimit() {
            return Json.read(json.get("limit"), Json::integer);
        }

        /** {@link #getLimit()} を書く。null なら欄を除く */
        public PointcloudRequest withLimit(Long value) {
            put("limit", value);
            return this;
        }
    }

    /** 地図の空間の箱。北・東・標高（メートル）の最小と最大 */
    public static final class PointcloudBox extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PointcloudBox(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PointcloudBox of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PointcloudBox(json);
        }

        /** 空のオブジェクトから作る */
        public PointcloudBox() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 最小の北・東・標高 */
        public List<Double> getMin() {
            return Json.read(json.get("min"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getMin()} を書く。null なら欄を除く */
        public PointcloudBox withMin(List<Double> value) {
            put("min", value);
            return this;
        }

        /** 最大の北・東・標高 */
        public List<Double> getMax() {
            return Json.read(json.get("max"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getMax()} を書く。null なら欄を除く */
        public PointcloudBox withMax(List<Double> value) {
            put("max", value);
            return this;
        }
    }

    /** 点群の要約と、選んだ点 */
    public static final class PointcloudResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PointcloudResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PointcloudResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PointcloudResult(json);
        }

        /** 空のオブジェクトから作る */
        public PointcloudResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 点の数 */
        public Long getPoints() {
            return Json.read(json.get("points"), Json::integer);
        }

        /** {@link #getPoints()} を書く。null なら欄を除く */
        public PointcloudResult withPoints(Long value) {
            put("points", value);
            return this;
        }

        /** 点の範囲。点がなければ書かない */
        public PointcloudBox getBounds() {
            return Json.read(json.get("bounds"), PointcloudBox::of);
        }

        /** {@link #getBounds()} を書く。null なら欄を除く */
        public PointcloudResult withBounds(PointcloudBox value) {
            put("bounds", value);
            return this;
        }

        /** 水平の座標系の EPSG のコード。分からなければ書かない */
        public Long getCrs() {
            return Json.read(json.get("crs"), Json::integer);
        }

        /** {@link #getCrs()} を書く。null なら欄を除く */
        public PointcloudResult withCrs(Long value) {
            put("crs", value);
            return this;
        }

        /** 高さの座標系の EPSG のコード。分からなければ書かない */
        public Long getVertical() {
            return Json.read(json.get("vertical"), Json::integer);
        }

        /** {@link #getVertical()} を書く。null なら欄を除く */
        public PointcloudResult withVertical(Long value) {
            put("vertical", value);
            return this;
        }

        /** 点が持つ属性（position・intensity・classification・color・returns・time） */
        public List<String> getAttributes() {
            return Json.read(json.get("attributes"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getAttributes()} を書く。null なら欄を除く */
        public PointcloudResult withAttributes(List<String> value) {
            put("attributes", value);
            return this;
        }

        /** 八分木のいちばん深い段 */
        public Long getDepth() {
            return Json.read(json.get("depth"), Json::integer);
        }

        /** {@link #getDepth()} を書く。null なら欄を除く */
        public PointcloudResult withDepth(Long value) {
            put("depth", value);
            return this;
        }

        /** 八分木の節の数 */
        public Long getNodes() {
            return Json.read(json.get("nodes"), Json::integer);
        }

        /** {@link #getNodes()} を書く。null なら欄を除く */
        public PointcloudResult withNodes(Long value) {
            put("nodes", value);
            return this;
        }

        /** 選んだ点。limit を書いたときに返す */
        public PointcloudPoints getSelected() {
            return Json.read(json.get("selected"), PointcloudPoints::of);
        }

        /** {@link #getSelected()} を書く。null なら欄を除く */
        public PointcloudResult withSelected(PointcloudPoints value) {
            put("selected", value);
            return this;
        }
    }

    /** 選んだ点。どの列も、選んだ点の順で同じ長さにする */
    public static final class PointcloudPoints extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PointcloudPoints(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PointcloudPoints of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PointcloudPoints(json);
        }

        /** 空のオブジェクトから作る */
        public PointcloudPoints() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 北・東・標高（メートル） */
        public List<List<Double>> getPositions() {
            return Json.read(json.get("positions"), v0 -> Json.list(v0, v1 -> Json.list(v1, Json::number)));
        }

        /** {@link #getPositions()} を書く。null なら欄を除く */
        public PointcloudPoints withPositions(List<List<Double>> value) {
            put("positions", value);
            return this;
        }

        /** 強度。点群が持つときに返す */
        public List<Long> getIntensity() {
            return Json.read(json.get("intensity"), v0 -> Json.list(v0, Json::integer));
        }

        /** {@link #getIntensity()} を書く。null なら欄を除く */
        public PointcloudPoints withIntensity(List<Long> value) {
            put("intensity", value);
            return this;
        }

        /** 分類。点群が持つときに返す */
        public List<Long> getClassification() {
            return Json.read(json.get("classification"), v0 -> Json.list(v0, Json::integer));
        }

        /** {@link #getClassification()} を書く。null なら欄を除く */
        public PointcloudPoints withClassification(List<Long> value) {
            put("classification", value);
            return this;
        }

        /** 色（赤・緑・青。16 ビット）。点群が持つときに返す */
        public List<List<Long>> getColor() {
            return Json.read(json.get("color"), v0 -> Json.list(v0, v1 -> Json.list(v1, Json::integer)));
        }

        /** {@link #getColor()} を書く。null なら欄を除く */
        public PointcloudPoints withColor(List<List<Long>> value) {
            put("color", value);
            return this;
        }

        /** limit のために選ばなかった点がある */
        public Boolean getTruncated() {
            return Json.read(json.get("truncated"), Json::bool);
        }

        /** {@link #getTruncated()} を書く。null なら欄を除く */
        public PointcloudPoints withTruncated(Boolean value) {
            put("truncated", value);
            return this;
        }

        /** 選んだいちばん深い段 */
        public Long getLevel() {
            return Json.read(json.get("level"), Json::integer);
        }

        /** {@link #getLevel()} を書く。null なら欄を除く */
        public PointcloudPoints withLevel(Long value) {
            put("level", value);
            return this;
        }
    }

    /** 電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。小版 53 から */
    public static final class DeliveryCheckRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DeliveryCheckRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DeliveryCheckRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DeliveryCheckRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public DeliveryCheckRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "delivery_check");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 成果品の起点。/ で終わる入力の名前の前の部分（納品/ など）。archive と一緒には書かない */
        public String getRoot() {
            return Json.read(json.get("root"), Json::string);
        }

        /** {@link #getRoot()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withRoot(String value) {
            put("root", value);
            return this;
        }

        /** 成果品の ZIP の入力の名前。root と一緒には書かない */
        public String getArchive() {
            return Json.read(json.get("archive"), Json::string);
        }

        /** {@link #getArchive()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withArchive(String value) {
            put("archive", value);
            return this;
        }

        /** プロファイルの入力の名前の列。extends で指す先も、すべて渡す */
        public List<String> getProfiles() {
            return Json.read(json.get("profiles"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getProfiles()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withProfiles(List<String> value) {
            put("profiles", value);
            return this;
        }

        /** 照合に使うプロファイル。profiles の中から選ぶ */
        public ProfileRef getProfile() {
            return Json.read(json.get("profile"), ProfileRef::of);
        }

        /** {@link #getProfile()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withProfile(ProfileRef value) {
            put("profile", value);
            return this;
        }

        /** 返す指摘の初めの位置 */
        public Long getOffset() {
            return Json.read(json.get("offset"), Json::integer);
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withOffset(Long value) {
            put("offset", value);
            return this;
        }

        /** 返す指摘の数の上限。書かなければ 10000 */
        public Long getLimit() {
            return Json.read(json.get("limit"), Json::integer);
        }

        /** {@link #getLimit()} を書く。null なら欄を除く */
        public DeliveryCheckRequest withLimit(Long value) {
            put("limit", value);
            return this;
        }
    }

    /** 成果品の要約と、照合の結果。照合の結果は check と同じ形 */
    public static final class DeliveryCheckResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DeliveryCheckResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DeliveryCheckResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DeliveryCheckResult(json);
        }

        /** 空のオブジェクトから作る */
        public DeliveryCheckResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 成果品のファイルの数 */
        public Long getFiles() {
            return Json.read(json.get("files"), Json::integer);
        }

        /** {@link #getFiles()} を書く。null なら欄を除く */
        public DeliveryCheckResult withFiles(Long value) {
            put("files", value);
            return this;
        }

        /** 管理ファイルの要約。名前の順 */
        public List<DeliveryManaged> getManaged() {
            return Json.read(json.get("managed"), v0 -> Json.list(v0, DeliveryManaged::of));
        }

        /** {@link #getManaged()} を書く。null なら欄を除く */
        public DeliveryCheckResult withManaged(List<DeliveryManaged> value) {
            put("managed", value);
            return this;
        }

        /** 照合に使ったプロファイルの鎖。基から順に。プロファイルを渡さなければ空 */
        public List<ProfileIdentity> getChain() {
            return Json.read(json.get("chain"), v0 -> Json.list(v0, ProfileIdentity::of));
        }

        /** {@link #getChain()} を書く。null なら欄を除く */
        public DeliveryCheckResult withChain(List<ProfileIdentity> value) {
            put("chain", value);
            return this;
        }

        /** 規則ごとの指摘の数 */
        public List<CheckRule> getRules() {
            return Json.read(json.get("rules"), v0 -> Json.list(v0, CheckRule::of));
        }

        /** {@link #getRules()} を書く。null なら欄を除く */
        public DeliveryCheckResult withRules(List<CheckRule> value) {
            put("rules", value);
            return this;
        }

        /** 指摘 */
        public List<CheckFinding> getFindings() {
            return Json.read(json.get("findings"), v0 -> Json.list(v0, CheckFinding::of));
        }

        /** {@link #getFindings()} を書く。null なら欄を除く */
        public DeliveryCheckResult withFindings(List<CheckFinding> value) {
            put("findings", value);
            return this;
        }

        /** 指摘の数の全体 */
        public Long getTotal() {
            return Json.read(json.get("total"), Json::integer);
        }

        /** {@link #getTotal()} を書く。null なら欄を除く */
        public DeliveryCheckResult withTotal(Long value) {
            put("total", value);
            return this;
        }

        /** 続きの指摘の位置。続きがなければ書かない */
        public Long getNextOffset() {
            return Json.read(json.get("next_offset"), Json::integer);
        }

        /** {@link #getNextOffset()} を書く。null なら欄を除く */
        public DeliveryCheckResult withNextOffset(Long value) {
            put("next_offset", value);
            return this;
        }
    }

    /** 工事写真（JPEG）の Exif と XMP を読み、画素数・向き・撮影の日時・位置などを返す（#387）。画像は復号しない。図面のモジュールに置く。小版 55 から */
    public static final class PhotoRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PhotoRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PhotoRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PhotoRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public PhotoRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "photo");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public PhotoRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public PhotoRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public PhotoRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public PhotoRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public PhotoRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public PhotoRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** JPEG の入力の名前 */
        public String getInput() {
            return Json.read(json.get("input"), Json::string);
        }

        /** {@link #getInput()} を書く。null なら欄を除く */
        public PhotoRequest withInput(String value) {
            put("input", value);
            return this;
        }
    }

    /** 写真の情報。ファイルが持たない値は書かない */
    public static final class PhotoResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PhotoResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PhotoResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PhotoResult(json);
        }

        /** 空のオブジェクトから作る */
        public PhotoResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 画素の幅（フレームの見出し） */
        public Long getWidth() {
            return Json.read(json.get("width"), Json::integer);
        }

        /** {@link #getWidth()} を書く。null なら欄を除く */
        public PhotoResult withWidth(Long value) {
            put("width", value);
            return this;
        }

        /** 画素の高さ（フレームの見出し） */
        public Long getHeight() {
            return Json.read(json.get("height"), Json::integer);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public PhotoResult withHeight(Long value) {
            put("height", value);
            return this;
        }

        /** 向き（Exif の Orientation、1〜8） */
        public Long getOrientation() {
            return Json.read(json.get("orientation"), Json::integer);
        }

        /** {@link #getOrientation()} を書く。null なら欄を除く */
        public PhotoResult withOrientation(Long value) {
            put("orientation", value);
            return this;
        }

        /** 撮影の日時（YYYY-MM-DDTHH:MM:SS、時差があれば +09:00 などを足す） */
        public String getTaken() {
            return Json.read(json.get("taken"), Json::string);
        }

        /** {@link #getTaken()} を書く。null なら欄を除く */
        public PhotoResult withTaken(String value) {
            put("taken", value);
            return this;
        }

        /** 機器の作り手 */
        public String getMake() {
            return Json.read(json.get("make"), Json::string);
        }

        /** {@link #getMake()} を書く。null なら欄を除く */
        public PhotoResult withMake(String value) {
            put("make", value);
            return this;
        }

        /** 機器の型 */
        public String getModel() {
            return Json.read(json.get("model"), Json::string);
        }

        /** {@link #getModel()} を書く。null なら欄を除く */
        public PhotoResult withModel(String value) {
            put("model", value);
            return this;
        }

        /** 撮った位置（GPS の緯度・経度、度）。測地系は換算しない */
        public PhotoPosition getPosition() {
            return Json.read(json.get("position"), PhotoPosition::of);
        }

        /** {@link #getPosition()} を書く。null なら欄を除く */
        public PhotoResult withPosition(PhotoPosition value) {
            put("position", value);
            return this;
        }

        /** 撮った向き（度） */
        public Double getDirection() {
            return Json.read(json.get("direction"), Json::number);
        }

        /** {@link #getDirection()} を書く。null なら欄を除く */
        public PhotoResult withDirection(Double value) {
            put("direction", value);
            return this;
        }

        /** 向きの基準（T は真北、M は磁北） */
        public String getDirectionReference() {
            return Json.read(json.get("direction_reference"), Json::string);
        }

        /** {@link #getDirectionReference()} を書く。null なら欄を除く */
        public PhotoResult withDirectionReference(String value) {
            put("direction_reference", value);
            return this;
        }

        /** GPS の測地系の名前（GPSMapDatum） */
        public String getDatum() {
            return Json.read(json.get("datum"), Json::string);
        }

        /** {@link #getDatum()} を書く。null なら欄を除く */
        public PhotoResult withDatum(String value) {
            put("datum", value);
            return this;
        }

        /** XMP の性質。文書の順 */
        public List<PhotoProperty> getXmp() {
            return Json.read(json.get("xmp"), v0 -> Json.list(v0, PhotoProperty::of));
        }

        /** {@link #getXmp()} を書く。null なら欄を除く */
        public PhotoResult withXmp(List<PhotoProperty> value) {
            put("xmp", value);
            return this;
        }
    }

    /** 撮った位置 */
    public static final class PhotoPosition extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PhotoPosition(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PhotoPosition of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PhotoPosition(json);
        }

        /** 空のオブジェクトから作る */
        public PhotoPosition() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 緯度（度、北が正） */
        public Double getLatitude() {
            return Json.read(json.get("latitude"), Json::number);
        }

        /** {@link #getLatitude()} を書く。null なら欄を除く */
        public PhotoPosition withLatitude(Double value) {
            put("latitude", value);
            return this;
        }

        /** 経度（度、東が正） */
        public Double getLongitude() {
            return Json.read(json.get("longitude"), Json::number);
        }

        /** {@link #getLongitude()} を書く。null なら欄を除く */
        public PhotoPosition withLongitude(Double value) {
            put("longitude", value);
            return this;
        }

        /** 高さ（m、海面より下は負） */
        public Double getAltitude() {
            return Json.read(json.get("altitude"), Json::number);
        }

        /** {@link #getAltitude()} を書く。null なら欄を除く */
        public PhotoPosition withAltitude(Double value) {
            put("altitude", value);
            return this;
        }
    }

    /** XMP の性質 */
    public static final class PhotoProperty extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public PhotoProperty(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static PhotoProperty of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new PhotoProperty(json);
        }

        /** 空のオブジェクトから作る */
        public PhotoProperty() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前空間の URI */
        public String getNamespace() {
            return Json.read(json.get("namespace"), Json::string);
        }

        /** {@link #getNamespace()} を書く。null なら欄を除く */
        public PhotoProperty withNamespace(String value) {
            put("namespace", value);
            return this;
        }

        /** 接頭辞を除いた名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public PhotoProperty withName(String value) {
            put("name", value);
            return this;
        }

        /** 値。配列は最初の項目の値 */
        public String getValue() {
            return Json.read(json.get("value"), Json::string);
        }

        /** {@link #getValue()} を書く。null なら欄を除く */
        public PhotoProperty withValue(String value) {
            put("value", value);
            return this;
        }
    }

    /** 表（Excel の xlsx と CSV）を、様式を知らずに読み、シートごとのセルの値・数式・結合セルを返す（#391）。数式は計算せず、保存された値を返す。図面のモジュールに置く。小版 56 から */
    public static final class SheetRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SheetRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SheetRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SheetRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public SheetRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "sheet");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public SheetRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public SheetRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public SheetRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public SheetRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public SheetRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public SheetRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** 表の入力の名前 */
        public String getInput() {
            return Json.read(json.get("input"), Json::string);
        }

        /** {@link #getInput()} を書く。null なら欄を除く */
        public SheetRequest withInput(String value) {
            put("input", value);
            return this;
        }

        /** 形式。書かなければ、ZIP の署名で始まれば xlsx、そうでなければ csv とする */
        public String getFormat() {
            return Json.read(json.get("format"), Json::string);
        }

        /** {@link #getFormat()} を書く。null なら欄を除く */
        public SheetRequest withFormat(String value) {
            put("format", value);
            return this;
        }

        /** CSV の文字コード。書かなければ、BOM か UTF-8 として正しければ utf_8、そうでなければ shift_jis とする */
        public String getEncoding() {
            return Json.read(json.get("encoding"), Json::string);
        }

        /** {@link #getEncoding()} を書く。null なら欄を除く */
        public SheetRequest withEncoding(String value) {
            put("encoding", value);
            return this;
        }

        /** CSV の区切りの字。書かなければ comma */
        public String getDelimiter() {
            return Json.read(json.get("delimiter"), Json::string);
        }

        /** {@link #getDelimiter()} を書く。null なら欄を除く */
        public SheetRequest withDelimiter(String value) {
            put("delimiter", value);
            return this;
        }
    }

    /**
     * 表の形式
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class SheetFormat {
        private SheetFormat() {}

        public static final String XLSX = "xlsx";

        public static final String CSV = "csv";
    }

    /**
     * CSV の文字コード。shift_jis は Windows の拡張を含む
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class SheetEncoding {
        private SheetEncoding() {}

        public static final String UTF_8 = "utf_8";

        public static final String SHIFT_JIS = "shift_jis";
    }

    /**
     * CSV の区切りの字。comma は「,」、tab はタブ、semicolon は「;」
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class SheetDelimiter {
        private SheetDelimiter() {}

        public static final String COMMA = "comma";

        public static final String TAB = "tab";

        public static final String SEMICOLON = "semicolon";
    }

    /** 読んだ表 */
    public static final class SheetResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SheetResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SheetResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SheetResult(json);
        }

        /** 空のオブジェクトから作る */
        public SheetResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 読んだ形式 */
        public String getFormat() {
            return Json.read(json.get("format"), Json::string);
        }

        /** {@link #getFormat()} を書く。null なら欄を除く */
        public SheetResult withFormat(String value) {
            put("format", value);
            return this;
        }

        /** CSV を読んだ文字コード。xlsx では書かない */
        public String getEncoding() {
            return Json.read(json.get("encoding"), Json::string);
        }

        /** {@link #getEncoding()} を書く。null なら欄を除く */
        public SheetResult withEncoding(String value) {
            put("encoding", value);
            return this;
        }

        /** シート。ブックの順。CSV は 1 つ。ワークシートでないシートは含めない */
        public List<Sheet> getSheets() {
            return Json.read(json.get("sheets"), v0 -> Json.list(v0, Sheet::of));
        }

        /** {@link #getSheets()} を書く。null なら欄を除く */
        public SheetResult withSheets(List<Sheet> value) {
            put("sheets", value);
            return this;
        }
    }

    /** シート */
    public static final class Sheet extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Sheet(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Sheet of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Sheet(json);
        }

        /** 空のオブジェクトから作る */
        public Sheet() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前。CSV は空 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public Sheet withName(String value) {
            put("name", value);
            return this;
        }

        /** 行の数（値のある最後の行と、結合セルの最後の行まで） */
        public Long getRows() {
            return Json.read(json.get("rows"), Json::integer);
        }

        /** {@link #getRows()} を書く。null なら欄を除く */
        public Sheet withRows(Long value) {
            put("rows", value);
            return this;
        }

        /** 列の数（値のある最後の列と、結合セルの最後の列まで） */
        public Long getColumns() {
            return Json.read(json.get("columns"), Json::integer);
        }

        /** {@link #getColumns()} を書く。null なら欄を除く */
        public Sheet withColumns(Long value) {
            put("columns", value);
            return this;
        }

        /** 値か数式のあるセル。行の順、行の中は列の順 */
        public List<SheetCell> getCells() {
            return Json.read(json.get("cells"), v0 -> Json.list(v0, SheetCell::of));
        }

        /** {@link #getCells()} を書く。null なら欄を除く */
        public Sheet withCells(List<SheetCell> value) {
            put("cells", value);
            return this;
        }

        /** 結合セル。ファイルの順 */
        public List<SheetArea> getMerged() {
            return Json.read(json.get("merged"), v0 -> Json.list(v0, SheetArea::of));
        }

        /** {@link #getMerged()} を書く。null なら欄を除く */
        public Sheet withMerged(List<SheetArea> value) {
            put("merged", value);
            return this;
        }
    }

    /** セル。行と列は 0 から数える */
    public static final class SheetCell extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SheetCell(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SheetCell of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SheetCell(json);
        }

        /** 空のオブジェクトから作る */
        public SheetCell() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 行 */
        public Long getRow() {
            return Json.read(json.get("row"), Json::integer);
        }

        /** {@link #getRow()} を書く。null なら欄を除く */
        public SheetCell withRow(Long value) {
            put("row", value);
            return this;
        }

        /** 列 */
        public Long getColumn() {
            return Json.read(json.get("column"), Json::integer);
        }

        /** {@link #getColumn()} を書く。null なら欄を除く */
        public SheetCell withColumn(Long value) {
            put("column", value);
            return this;
        }

        /** 値の種類 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public SheetCell withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 値。数は保存された字のまま、真偽は TRUE か FALSE。日付の書式は解釈しないので、日付は数のまま */
        public String getValue() {
            return Json.read(json.get("value"), Json::string);
        }

        /** {@link #getValue()} を書く。null なら欄を除く */
        public SheetCell withValue(String value) {
            put("value", value);
            return this;
        }

        /** 数式（先頭の = を除く）。共有の数式は、元のセルだけが持つ */
        public String getFormula() {
            return Json.read(json.get("formula"), Json::string);
        }

        /** {@link #getFormula()} を書く。null なら欄を除く */
        public SheetCell withFormula(String value) {
            put("formula", value);
            return this;
        }
    }

    /**
     * セルの値の種類。CSV の値はすべて text
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class SheetCellKind {
        private SheetCellKind() {}

        public static final String TEXT = "text";

        public static final String NUMBER = "number";

        public static final String BOOLEAN = "boolean";

        public static final String ERROR = "error";
    }

    /** 結合セルの範囲。両端を含む */
    public static final class SheetArea extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public SheetArea(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static SheetArea of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new SheetArea(json);
        }

        /** 空のオブジェクトから作る */
        public SheetArea() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 最初の行 */
        public Long getFirstRow() {
            return Json.read(json.get("first_row"), Json::integer);
        }

        /** {@link #getFirstRow()} を書く。null なら欄を除く */
        public SheetArea withFirstRow(Long value) {
            put("first_row", value);
            return this;
        }

        /** 最初の列 */
        public Long getFirstColumn() {
            return Json.read(json.get("first_column"), Json::integer);
        }

        /** {@link #getFirstColumn()} を書く。null なら欄を除く */
        public SheetArea withFirstColumn(Long value) {
            put("first_column", value);
            return this;
        }

        /** 最後の行 */
        public Long getLastRow() {
            return Json.read(json.get("last_row"), Json::integer);
        }

        /** {@link #getLastRow()} を書く。null なら欄を除く */
        public SheetArea withLastRow(Long value) {
            put("last_row", value);
            return this;
        }

        /** 最後の列 */
        public Long getLastColumn() {
            return Json.read(json.get("last_column"), Json::integer);
        }

        /** {@link #getLastColumn()} を書く。null なら欄を除く */
        public SheetArea withLastColumn(Long value) {
            put("last_column", value);
            return this;
        }
    }

    /** 地質・土質調査のボーリング交換用データ（BED0500.DTD の XML）を読み、ボーリングの位置・標高と、地層・色調・観察記事・標準貫入試験・孔内水位を返す（#390）。ほかの試験と判定表は読まない。図面のモジュールに置く。小版 57 から */
    public static final class BoringRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BoringRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BoringRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BoringRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public BoringRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "boring");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public BoringRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public BoringRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public BoringRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public BoringRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public BoringRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public BoringRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** ボーリング交換用データの入力の名前 */
        public String getInput() {
            return Json.read(json.get("input"), Json::string);
        }

        /** {@link #getInput()} を書く。null なら欄を除く */
        public BoringRequest withInput(String value) {
            put("input", value);
            return this;
        }
    }

    /** 読んだボーリング。深さは地表からの m（GL.-m） */
    public static final class BoringResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BoringResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BoringResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BoringResult(json);
        }

        /** 空のオブジェクトから作る */
        public BoringResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 適用規格。文書の順 */
        public List<String> getStandards() {
            return Json.read(json.get("standards"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getStandards()} を書く。null なら欄を除く */
        public BoringResult withStandards(List<String> value) {
            put("standards", value);
            return this;
        }

        /** 公開フラグのコード */
        public String getPublication() {
            return Json.read(json.get("publication"), Json::string);
        }

        /** {@link #getPublication()} を書く。null なら欄を除く */
        public BoringResult withPublication(String value) {
            put("publication", value);
            return this;
        }

        /** 事業工事名 */
        public String getProject() {
            return Json.read(json.get("project"), Json::string);
        }

        /** {@link #getProject()} を書く。null なら欄を除く */
        public BoringResult withProject(String value) {
            put("project", value);
            return this;
        }

        /** 調査名 */
        public String getSurvey() {
            return Json.read(json.get("survey"), Json::string);
        }

        /** {@link #getSurvey()} を書く。null なら欄を除く */
        public BoringResult withSurvey(String value) {
            put("survey", value);
            return this;
        }

        /** ボーリング名 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public BoringResult withName(String value) {
            put("name", value);
            return this;
        }

        /** ボーリング総数 */
        public Long getCount() {
            return Json.read(json.get("count"), Json::integer);
        }

        /** {@link #getCount()} を書く。null なら欄を除く */
        public BoringResult withCount(Long value) {
            put("count", value);
            return this;
        }

        /** ボーリング連番 */
        public Long getSerial() {
            return Json.read(json.get("serial"), Json::integer);
        }

        /** {@link #getSerial()} を書く。null なら欄を除く */
        public BoringResult withSerial(Long value) {
            put("serial", value);
            return this;
        }

        /** 孔口の位置。測地系は換算しない */
        public BoringPosition getPosition() {
            return Json.read(json.get("position"), BoringPosition::of);
        }

        /** {@link #getPosition()} を書く。null なら欄を除く */
        public BoringResult withPosition(BoringPosition value) {
            put("position", value);
            return this;
        }

        /** 位置の測地系 */
        public String getDatum() {
            return Json.read(json.get("datum"), Json::string);
        }

        /** {@link #getDatum()} を書く。null なら欄を除く */
        public BoringResult withDatum(String value) {
            put("datum", value);
            return this;
        }

        /** 調査位置の住所 */
        public String getAddress() {
            return Json.read(json.get("address"), Json::string);
        }

        /** {@link #getAddress()} を書く。null なら欄を除く */
        public BoringResult withAddress(String value) {
            put("address", value);
            return this;
        }

        /** 調査期間の始まりの日（YYYY-MM-DD） */
        public String getStart() {
            return Json.read(json.get("start"), Json::string);
        }

        /** {@link #getStart()} を書く。null なら欄を除く */
        public BoringResult withStart(String value) {
            put("start", value);
            return this;
        }

        /** 調査期間の終わりの日（YYYY-MM-DD） */
        public String getEnd() {
            return Json.read(json.get("end"), Json::string);
        }

        /** {@link #getEnd()} を書く。null なら欄を除く */
        public BoringResult withEnd(String value) {
            put("end", value);
            return this;
        }

        /** 孔口標高 */
        public BoringHeight getElevation() {
            return Json.read(json.get("elevation"), BoringHeight::of);
        }

        /** {@link #getElevation()} を書く。null なら欄を除く */
        public BoringResult withElevation(BoringHeight value) {
            put("elevation", value);
            return this;
        }

        /** 総削孔長（m） */
        public Double getLength() {
            return Json.read(json.get("length"), Json::number);
        }

        /** {@link #getLength()} を書く。null なら欄を除く */
        public BoringResult withLength(Double value) {
            put("length", value);
            return this;
        }

        /** 削孔の角度（度） */
        public Double getAngle() {
            return Json.read(json.get("angle"), Json::number);
        }

        /** {@link #getAngle()} を書く。null なら欄を除く */
        public BoringResult withAngle(Double value) {
            put("angle", value);
            return this;
        }

        /** 削孔の方位（度） */
        public Double getDirection() {
            return Json.read(json.get("direction"), Json::number);
        }

        /** {@link #getDirection()} を書く。null なら欄を除く */
        public BoringResult withDirection(Double value) {
            put("direction", value);
            return this;
        }

        /** 工学的地質区分名・現場土質名。文書の順 */
        public List<BoringLayer> getLayers() {
            return Json.read(json.get("layers"), v0 -> Json.list(v0, BoringLayer::of));
        }

        /** {@link #getLayers()} を書く。null なら欄を除く */
        public BoringResult withLayers(List<BoringLayer> value) {
            put("layers", value);
            return this;
        }

        /** 色調。文書の順 */
        public List<BoringColor> getColors() {
            return Json.read(json.get("colors"), v0 -> Json.list(v0, BoringColor::of));
        }

        /** {@link #getColors()} を書く。null なら欄を除く */
        public BoringResult withColors(List<BoringColor> value) {
            put("colors", value);
            return this;
        }

        /** 観察記事。文書の順 */
        public List<BoringObservation> getObservations() {
            return Json.read(json.get("observations"), v0 -> Json.list(v0, BoringObservation::of));
        }

        /** {@link #getObservations()} を書く。null なら欄を除く */
        public BoringResult withObservations(List<BoringObservation> value) {
            put("observations", value);
            return this;
        }

        /** 標準貫入試験。文書の順 */
        public List<BoringPenetration> getPenetrations() {
            return Json.read(json.get("penetrations"), v0 -> Json.list(v0, BoringPenetration::of));
        }

        /** {@link #getPenetrations()} を書く。null なら欄を除く */
        public BoringResult withPenetrations(List<BoringPenetration> value) {
            put("penetrations", value);
            return this;
        }

        /** 孔内水位。文書の順 */
        public List<BoringWaterLevel> getWaterLevels() {
            return Json.read(json.get("water_levels"), v0 -> Json.list(v0, BoringWaterLevel::of));
        }

        /** {@link #getWaterLevels()} を書く。null なら欄を除く */
        public BoringResult withWaterLevels(List<BoringWaterLevel> value) {
            put("water_levels", value);
            return this;
        }
    }

    /** 経度・緯度（度）。度・分・秒から求める */
    public static final class BoringPosition extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BoringPosition(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BoringPosition of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BoringPosition(json);
        }

        /** 空のオブジェクトから作る */
        public BoringPosition() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 緯度（度、北が正） */
        public Double getLatitude() {
            return Json.read(json.get("latitude"), Json::number);
        }

        /** {@link #getLatitude()} を書く。null なら欄を除く */
        public BoringPosition withLatitude(Double value) {
            put("latitude", value);
            return this;
        }

        /** 経度（度、東が正） */
        public Double getLongitude() {
            return Json.read(json.get("longitude"), Json::number);
        }

        /** {@link #getLongitude()} を書く。null なら欄を除く */
        public BoringPosition withLongitude(Double value) {
            put("longitude", value);
            return this;
        }
    }

    /**
     * 測地系。tokyo は日本測地系（コード 00）、jgd2000・jgd2011・jgd2024 は世界測地系（コード 01〜03）
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class BoringDatum {
        private BoringDatum() {}

        public static final String TOKYO = "tokyo";

        public static final String JGD2000 = "jgd2000";

        public static final String JGD2011 = "jgd2011";

        public static final String JGD2024 = "jgd2024";
    }

    /** 標高と、その基準と成果（決定 0019） */
    public static final class BoringHeight extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BoringHeight(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BoringHeight of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BoringHeight(json);
        }

        /** 空のオブジェクトから作る */
        public BoringHeight() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 標高（m） */
        public Double getValue() {
            return Json.read(json.get("value"), Json::number);
        }

        /** {@link #getValue()} を書く。null なら欄を除く */
        public BoringHeight withValue(Double value) {
            put("value", value);
            return this;
        }

        /** 高さの基準。孔口標高は tokyo_bay（東京湾平均海面）とする */
        public String getDatum() {
            return Json.read(json.get("datum"), Json::string);
        }

        /** {@link #getDatum()} を書く。null なら欄を除く */
        public BoringHeight withDatum(String value) {
            put("datum", value);
            return this;
        }

        /** 成果。測地系が jgd2024 なら survey_2024、そうでなければ調査の日から仮定する */
        public String getRealization() {
            return Json.read(json.get("realization"), Json::string);
        }

        /** {@link #getRealization()} を書く。null なら欄を除く */
        public BoringHeight withRealization(String value) {
            put("realization", value);
            return this;
        }
    }

    /**
     * 高さの基準（決定 0019）。tokyo_bay は東京湾平均海面（T.P.）、island は離島の独自の平均海面、ellipsoid は楕円体（GRS80）
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class HeightDatum {
        private HeightDatum() {}

        public static final String TOKYO_BAY = "tokyo_bay";

        public static final String ISLAND = "island";

        public static final String ELLIPSOID = "ellipsoid";
    }

    /**
     * 標高の成果（決定 0019）。survey_2011 は測地成果2011（2025-03-31 まで）、survey_2024 は測地成果2024（2025-04-01 から）
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class HeightRealization {
        private HeightRealization() {}

        public static final String SURVEY_2011 = "survey_2011";

        public static final String SURVEY_2024 = "survey_2024";
    }

    /** 地層の区間。上端は 1 つ前の区間の下端（最初は 0） */
    public static final class BoringLayer extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BoringLayer(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BoringLayer of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BoringLayer(json);
        }

        /** 空のオブジェクトから作る */
        public BoringLayer() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 下端の深さ（m） */
        public Double getBottom() {
            return Json.read(json.get("bottom"), Json::number);
        }

        /** {@link #getBottom()} を書く。null なら欄を除く */
        public BoringLayer withBottom(Double value) {
            put("bottom", value);
            return this;
        }

        /** 工学的地質区分名・現場土質名 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public BoringLayer withName(String value) {
            put("name", value);
            return this;
        }

        /** 記号 */
        public String getSymbol() {
            return Json.read(json.get("symbol"), Json::string);
        }

        /** {@link #getSymbol()} を書く。null なら欄を除く */
        public BoringLayer withSymbol(String value) {
            put("symbol", value);
            return this;
        }
    }

    /** 色調の区間 */
    public static final class BoringColor extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BoringColor(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BoringColor of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BoringColor(json);
        }

        /** 空のオブジェクトから作る */
        public BoringColor() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 下端の深さ（m） */
        public Double getBottom() {
            return Json.read(json.get("bottom"), Json::number);
        }

        /** {@link #getBottom()} を書く。null なら欄を除く */
        public BoringColor withBottom(Double value) {
            put("bottom", value);
            return this;
        }

        /** 色調の名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public BoringColor withName(String value) {
            put("name", value);
            return this;
        }
    }

    /** 観察記事 */
    public static final class BoringObservation extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BoringObservation(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BoringObservation of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BoringObservation(json);
        }

        /** 空のオブジェクトから作る */
        public BoringObservation() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 上端の深さ（m） */
        public Double getTop() {
            return Json.read(json.get("top"), Json::number);
        }

        /** {@link #getTop()} を書く。null なら欄を除く */
        public BoringObservation withTop(Double value) {
            put("top", value);
            return this;
        }

        /** 下端の深さ（m） */
        public Double getBottom() {
            return Json.read(json.get("bottom"), Json::number);
        }

        /** {@link #getBottom()} を書く。null なら欄を除く */
        public BoringObservation withBottom(Double value) {
            put("bottom", value);
            return this;
        }

        /** 記事 */
        public String getText() {
            return Json.read(json.get("text"), Json::string);
        }

        /** {@link #getText()} を書く。null なら欄を除く */
        public BoringObservation withText(String value) {
            put("text", value);
            return this;
        }
    }

    /** 標準貫入試験 */
    public static final class BoringPenetration extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BoringPenetration(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BoringPenetration of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BoringPenetration(json);
        }

        /** 空のオブジェクトから作る */
        public BoringPenetration() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 開始の深さ（m） */
        public Double getDepth() {
            return Json.read(json.get("depth"), Json::number);
        }

        /** {@link #getDepth()} を書く。null なら欄を除く */
        public BoringPenetration withDepth(Double value) {
            put("depth", value);
            return this;
        }

        /** 合計の打撃回数 */
        public Long getBlows() {
            return Json.read(json.get("blows"), Json::integer);
        }

        /** {@link #getBlows()} を書く。null なら欄を除く */
        public BoringPenetration withBlows(Long value) {
            put("blows", value);
            return this;
        }

        /** 合計の貫入量（mm） */
        public Double getPenetration() {
            return Json.read(json.get("penetration"), Json::number);
        }

        /** {@link #getPenetration()} を書く。null なら欄を除く */
        public BoringPenetration withPenetration(Double value) {
            put("penetration", value);
            return this;
        }
    }

    /** 孔内水位 */
    public static final class BoringWaterLevel extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public BoringWaterLevel(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static BoringWaterLevel of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new BoringWaterLevel(json);
        }

        /** 空のオブジェクトから作る */
        public BoringWaterLevel() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 測定の日（YYYY-MM-DD） */
        public String getDate() {
            return Json.read(json.get("date"), Json::string);
        }

        /** {@link #getDate()} を書く。null なら欄を除く */
        public BoringWaterLevel withDate(String value) {
            put("date", value);
            return this;
        }

        /** 削孔状況のコード */
        public String getState() {
            return Json.read(json.get("state"), Json::string);
        }

        /** {@link #getState()} を書く。null なら欄を除く */
        public BoringWaterLevel withState(String value) {
            put("state", value);
            return this;
        }

        /** 水位の深さ（m）。地表より高ければ負。水位がなければ書かない */
        public Double getDepth() {
            return Json.read(json.get("depth"), Json::number);
        }

        /** {@link #getDepth()} を書く。null なら欄を除く */
        public BoringWaterLevel withDepth(Double value) {
            put("depth", value);
            return this;
        }

        /** 水位種別・備考 */
        public String getNote() {
            return Json.read(json.get("note"), Json::string);
        }

        /** {@link #getNote()} を書く。null なら欄を除く */
        public BoringWaterLevel withNote(String value) {
            put("note", value);
            return this;
        }
    }

    /** 管理ファイルの要約 */
    public static final class DeliveryManaged extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DeliveryManaged(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DeliveryManaged of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DeliveryManaged(json);
        }

        /** 空のオブジェクトから作る */
        public DeliveryManaged() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 成果品の中の名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public DeliveryManaged withName(String value) {
            put("name", value);
            return this;
        }

        /** 文書型宣言の DTD の名前 */
        public String getDoctype() {
            return Json.read(json.get("doctype"), Json::string);
        }

        /** {@link #getDoctype()} を書く。null なら欄を除く */
        public DeliveryManaged withDoctype(String value) {
            put("doctype", value);
            return this;
        }

        /** 根の要素の名前 */
        public String getRoot() {
            return Json.read(json.get("root"), Json::string);
        }

        /** {@link #getRoot()} を書く。null なら欄を除く */
        public DeliveryManaged withRoot(String value) {
            put("root", value);
            return this;
        }

        /** 要素の記録の数 */
        public Long getRecords() {
            return Json.read(json.get("records"), Json::integer);
        }

        /** {@link #getRecords()} を書く。null なら欄を除く */
        public DeliveryManaged withRecords(Long value) {
            put("records", value);
            return this;
        }

        /** ファイルの名前の形の値の数 */
        public Long getLinks() {
            return Json.read(json.get("links"), Json::integer);
        }

        /** {@link #getLinks()} を書く。null なら欄を除く */
        public DeliveryManaged withLinks(Long value) {
            put("links", value);
            return this;
        }

        /** 成果品の中に見つからないファイルの名前。書いた順 */
        public List<String> getMissing() {
            return Json.read(json.get("missing"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getMissing()} を書く。null なら欄を除く */
        public DeliveryManaged withMissing(List<String> value) {
            put("missing", value);
            return this;
        }
    }

    /** open で開いた CityGML の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。図面のモジュールに置く。小版 52 から */
    public static final class GeoRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public GeoRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static GeoRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new GeoRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public GeoRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "geo");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public GeoRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public GeoRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public GeoRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public GeoRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public GeoRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public GeoRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** open で開いた CityGML の文書の名前 */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public GeoRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 中身を返す地物の ID */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public GeoRequest withId(String value) {
            put("id", value);
            return this;
        }
    }

    /** 地理の領域モデルの要約 */
    public static final class GeoResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public GeoResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static GeoResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new GeoResult(json);
        }

        /** 空のオブジェクトから作る */
        public GeoResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 平面の座標系の EPSG のコード。分からなければ書かない */
        public Long getCrs() {
            return Json.read(json.get("crs"), Json::integer);
        }

        /** {@link #getCrs()} を書く。null なら欄を除く */
        public GeoResult withCrs(Long value) {
            put("crs", value);
            return this;
        }

        /** 高さの座標系の EPSG のコード。分からなければ書かない */
        public Long getHeight() {
            return Json.read(json.get("height"), Json::integer);
        }

        /** {@link #getHeight()} を書く。null なら欄を除く */
        public GeoResult withHeight(Long value) {
            put("height", value);
            return this;
        }

        /** 地物の数 */
        public Long getFeatures() {
            return Json.read(json.get("features"), Json::integer);
        }

        /** {@link #getFeatures()} を書く。null なら欄を除く */
        public GeoResult withFeatures(Long value) {
            put("features", value);
            return this;
        }

        /** 地物の型ごとの数。型の名前の順 */
        public List<GeoKind> getKinds() {
            return Json.read(json.get("kinds"), v0 -> Json.list(v0, GeoKind::of));
        }

        /** {@link #getKinds()} を書く。null なら欄を除く */
        public GeoResult withKinds(List<GeoKind> value) {
            put("kinds", value);
            return this;
        }

        /** id で指した地物の中身 */
        public GeoFeature getFeature() {
            return Json.read(json.get("feature"), GeoFeature::of);
        }

        /** {@link #getFeature()} を書く。null なら欄を除く */
        public GeoResult withFeature(GeoFeature value) {
            put("feature", value);
            return this;
        }
    }

    /** 地物の型と数 */
    public static final class GeoKind extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public GeoKind(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static GeoKind of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new GeoKind(json);
        }

        /** 空のオブジェクトから作る */
        public GeoKind() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 型。名前空間の接頭辞をつけた元の名前 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public GeoKind withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 地物の数 */
        public Long getCount() {
            return Json.read(json.get("count"), Json::integer);
        }

        /** {@link #getCount()} を書く。null なら欄を除く */
        public GeoKind withCount(Long value) {
            put("count", value);
            return this;
        }
    }

    /** 地物の中身 */
    public static final class GeoFeature extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public GeoFeature(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static GeoFeature of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new GeoFeature(json);
        }

        /** 空のオブジェクトから作る */
        public GeoFeature() {
            super(new LinkedHashMap<String, Object>());
        }

        /** ID */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public GeoFeature withId(String value) {
            put("id", value);
            return this;
        }

        /** 型 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public GeoFeature withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 分類コード。元の値の文字のまま */
        public String getCode() {
            return Json.read(json.get("code"), Json::string);
        }

        /** {@link #getCode()} を書く。null なら欄を除く */
        public GeoFeature withCode(String value) {
            put("code", value);
            return this;
        }

        /** 名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public GeoFeature withName(String value) {
            put("name", value);
            return this;
        }

        /** 親の地物の ID */
        public String getParent() {
            return Json.read(json.get("parent"), Json::string);
        }

        /** {@link #getParent()} を書く。null なら欄を除く */
        public GeoFeature withParent(String value) {
            put("parent", value);
            return this;
        }

        /** 子の地物の数 */
        public Long getChildren() {
            return Json.read(json.get("children"), Json::integer);
        }

        /** {@link #getChildren()} を書く。null なら欄を除く */
        public GeoFeature withChildren(Long value) {
            put("children", value);
            return this;
        }

        /** 属性。入力の順 */
        public List<GeoAttribute> getAttributes() {
            return Json.read(json.get("attributes"), v0 -> Json.list(v0, GeoAttribute::of));
        }

        /** {@link #getAttributes()} を書く。null なら欄を除く */
        public GeoFeature withAttributes(List<GeoAttribute> value) {
            put("attributes", value);
            return this;
        }

        /** 形。入力の順 */
        public List<GeoGeometry> getGeometries() {
            return Json.read(json.get("geometries"), v0 -> Json.list(v0, GeoGeometry::of));
        }

        /** {@link #getGeometries()} を書く。null なら欄を除く */
        public GeoFeature withGeometries(List<GeoGeometry> value) {
            put("geometries", value);
            return this;
        }
    }

    /** 属性 */
    public static final class GeoAttribute extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public GeoAttribute(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static GeoAttribute of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new GeoAttribute(json);
        }

        /** 空のオブジェクトから作る */
        public GeoAttribute() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 名前。接頭辞をつけた元の名前 */
        public String getName() {
            return Json.read(json.get("name"), Json::string);
        }

        /** {@link #getName()} を書く。null なら欄を除く */
        public GeoAttribute withName(String value) {
            put("name", value);
            return this;
        }

        /** 値の型 */
        public String getKind() {
            return Json.read(json.get("kind"), Json::string);
        }

        /** {@link #getKind()} を書く。null なら欄を除く */
        public GeoAttribute withKind(String value) {
            put("kind", value);
            return this;
        }

        /** 値の文字 */
        public String getValue() {
            return Json.read(json.get("value"), Json::string);
        }

        /** {@link #getValue()} を書く。null なら欄を除く */
        public GeoAttribute withValue(String value) {
            put("value", value);
            return this;
        }

        /** 単位つきの量の単位 */
        public String getUnit() {
            return Json.read(json.get("unit"), Json::string);
        }

        /** {@link #getUnit()} を書く。null なら欄を除く */
        public GeoAttribute withUnit(String value) {
            put("unit", value);
            return this;
        }

        /** コードの一覧（codeSpace） */
        public String getSpace() {
            return Json.read(json.get("space"), Json::string);
        }

        /** {@link #getSpace()} を書く。null なら欄を除く */
        public GeoAttribute withSpace(String value) {
            put("space", value);
            return this;
        }
    }

    /**
     * 属性の値の型
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class GeoValueKind {
        private GeoValueKind() {}

        public static final String TEXT = "text";

        public static final String INTEGER = "integer";

        public static final String REAL = "real";

        public static final String BOOL = "bool";

        public static final String DATE = "date";

        public static final String CODE = "code";

        public static final String MEASURE = "measure";
    }

    /**
     * 形の種類
     * 
     * 列挙の値。知らない値も受け取る
     */
    public static final class GeoForm {
        private GeoForm() {}

        public static final String POINT = "point";

        public static final String LINE = "line";

        public static final String AREA = "area";

        public static final String SOLID = "solid";
    }

    /** 形の要約 */
    public static final class GeoGeometry extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public GeoGeometry(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static GeoGeometry of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new GeoGeometry(json);
        }

        /** 空のオブジェクトから作る */
        public GeoGeometry() {
            super(new LinkedHashMap<String, Object>());
        }

        /** LOD。2D の地物は書かない */
        public Long getLod() {
            return Json.read(json.get("lod"), Json::integer);
        }

        /** {@link #getLod()} を書く。null なら欄を除く */
        public GeoGeometry withLod(Long value) {
            put("lod", value);
            return this;
        }

        /** 形の種類 */
        public String getForm() {
            return Json.read(json.get("form"), Json::string);
        }

        /** {@link #getForm()} を書く。null なら欄を除く */
        public GeoGeometry withForm(String value) {
            put("form", value);
            return this;
        }

        /** 点の数。立体と面の集まりは、面の輪の点の数の和 */
        public Long getPoints() {
            return Json.read(json.get("points"), Json::integer);
        }

        /** {@link #getPoints()} を書く。null なら欄を除く */
        public GeoGeometry withPoints(Long value) {
            put("points", value);
            return this;
        }

        /** 立体と面の集まりの面の数 */
        public Long getFaces() {
            return Json.read(json.get("faces"), Json::integer);
        }

        /** {@link #getFaces()} を書く。null なら欄を除く */
        public GeoGeometry withFaces(Long value) {
            put("faces", value);
            return this;
        }
    }

    /** open で開いた J-LandXML の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。小版 51 から */
    public static final class CivilDrawingRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilDrawingRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilDrawingRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilDrawingRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public CivilDrawingRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "civil_drawing");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public CivilDrawingRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public CivilDrawingRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public CivilDrawingRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public CivilDrawingRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public CivilDrawingRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public CivilDrawingRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** open で開いた J-LandXML の文書の名前 */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public CivilDrawingRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** doc の線形の ID。文書に線形が 1 つなら書かなくてよい */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public CivilDrawingRequest withId(String value) {
            put("id", value);
            return this;
        }

        /** 書く用紙の名前（結果の sheets の 1 つ）。書かなければ平面図 */
        public String getSheet() {
            return Json.read(json.get("sheet"), Json::string);
        }

        /** {@link #getSheet()} を書く。null なら欄を除く */
        public CivilDrawingRequest withSheet(String value) {
            put("sheet", value);
            return this;
        }

        /** 作る文書の名前 */
        public String getTo() {
            return Json.read(json.get("to"), Json::string);
        }

        /** {@link #getTo()} を書く。null なら欄を除く */
        public CivilDrawingRequest withTo(String value) {
            put("to", value);
            return this;
        }

        /** プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す */
        public List<String> getProfiles() {
            return Json.read(json.get("profiles"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getProfiles()} を書く。null なら欄を除く */
        public CivilDrawingRequest withProfiles(List<String> value) {
            put("profiles", value);
            return this;
        }

        /** 作図の値に使うプロファイル。profiles の中から選ぶ。profiles と一緒に書く */
        public ProfileRef getProfile() {
            return Json.read(json.get("profile"), ProfileRef::of);
        }

        /** {@link #getProfile()} を書く。null なら欄を除く */
        public CivilDrawingRequest withProfile(ProfileRef value) {
            put("profile", value);
            return this;
        }
    }

    /** 作った文書。書いたファイルは blobs の file で返す */
    public static final class CivilDrawingResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilDrawingResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilDrawingResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilDrawingResult(json);
        }

        /** 空のオブジェクトから作る */
        public CivilDrawingResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 作った文書の名前 */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public CivilDrawingResult withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 書いた形式の名前。今は dxf */
        public String getFormat() {
            return Json.read(json.get("format"), Json::string);
        }

        /** {@link #getFormat()} を書く。null なら欄を除く */
        public CivilDrawingResult withFormat(String value) {
            put("format", value);
            return this;
        }

        /** 書いたファイルのハッシュ。apply の前提条件に使う */
        public String getHash() {
            return Json.read(json.get("hash"), Json::string);
        }

        /** {@link #getHash()} を書く。null なら欄を除く */
        public CivilDrawingResult withHash(String value) {
            put("hash", value);
            return this;
        }

        /** 書いた要素の数 */
        public Long getElements() {
            return Json.read(json.get("elements"), Json::integer);
        }

        /** {@link #getElements()} を書く。null なら欄を除く */
        public CivilDrawingResult withElements(Long value) {
            put("elements", value);
            return this;
        }

        /** 図を作った線形の ID */
        public String getAlignment() {
            return Json.read(json.get("alignment"), Json::string);
        }

        /** {@link #getAlignment()} を書く。null なら欄を除く */
        public CivilDrawingResult withAlignment(String value) {
            put("alignment", value);
            return this;
        }

        /** 作った用紙の名前。図を描ける順（平面図・縦断図・横断図） */
        public List<String> getSheets() {
            return Json.read(json.get("sheets"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getSheets()} を書く。null なら欄を除く */
        public CivilDrawingResult withSheets(List<String> value) {
            put("sheets", value);
            return this;
        }

        /** 書いた用紙の名前 */
        public String getSheet() {
            return Json.read(json.get("sheet"), Json::string);
        }

        /** {@link #getSheet()} を書く。null なら欄を除く */
        public CivilDrawingResult withSheet(String value) {
            put("sheet", value);
            return this;
        }

        /** 使った作図の値 */
        public CivilDrawingOptions getOptions() {
            return Json.read(json.get("options"), CivilDrawingOptions::of);
        }

        /** {@link #getOptions()} を書く。null なら欄を除く */
        public CivilDrawingResult withOptions(CivilDrawingOptions value) {
            put("options", value);
            return this;
        }

        /** 作図の値を読んだプロファイルの、extends を解いた鎖。基から順。プロファイルを渡さなければ書かない */
        public List<ProfileIdentity> getProfile() {
            return Json.read(json.get("profile"), v0 -> Json.list(v0, ProfileIdentity::of));
        }

        /** {@link #getProfile()} を書く。null なら欄を除く */
        public CivilDrawingResult withProfile(List<ProfileIdentity> value) {
            put("profile", value);
            return this;
        }
    }

    /** 土木の図の作図の値 */
    public static final class CivilDrawingOptions extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public CivilDrawingOptions(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static CivilDrawingOptions of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new CivilDrawingOptions(json);
        }

        /** 空のオブジェクトから作る */
        public CivilDrawingOptions() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 平面図の尺度の分母 */
        public Double getPlan() {
            return Json.read(json.get("plan"), Json::number);
        }

        /** {@link #getPlan()} を書く。null なら欄を除く */
        public CivilDrawingOptions withPlan(Double value) {
            put("plan", value);
            return this;
        }

        /** 縦断図の横の尺度の分母 */
        public Double getHorizontal() {
            return Json.read(json.get("horizontal"), Json::number);
        }

        /** {@link #getHorizontal()} を書く。null なら欄を除く */
        public CivilDrawingOptions withHorizontal(Double value) {
            put("horizontal", value);
            return this;
        }

        /** 縦断図の縦の尺度の分母 */
        public Double getVertical() {
            return Json.read(json.get("vertical"), Json::number);
        }

        /** {@link #getVertical()} を書く。null なら欄を除く */
        public CivilDrawingOptions withVertical(Double value) {
            put("vertical", value);
            return this;
        }

        /** 横断図の尺度の分母 */
        public Double getCrossSection() {
            return Json.read(json.get("cross_section"), Json::number);
        }

        /** {@link #getCrossSection()} を書く。null なら欄を除く */
        public CivilDrawingOptions withCrossSection(Double value) {
            put("cross_section", value);
            return this;
        }

        /** 文字の高さ（mm） */
        public Double getTextHeight() {
            return Json.read(json.get("text_height"), Json::number);
        }

        /** {@link #getTextHeight()} を書く。null なら欄を除く */
        public CivilDrawingOptions withTextHeight(Double value) {
            put("text_height", value);
            return this;
        }

        /** 測点の間隔（メートル） */
        public Double getStationPitch() {
            return Json.read(json.get("station_pitch"), Json::number);
        }

        /** {@link #getStationPitch()} を書く。null なら欄を除く */
        public CivilDrawingOptions withStationPitch(Double value) {
            put("station_pitch", value);
            return this;
        }
    }

    /** 開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。図面のモジュールに置く。小版 50 から */
    public static final class DeviationRequest extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DeviationRequest(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DeviationRequest of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DeviationRequest(json);
        }

        /** 目印の欄だけを持つオブジェクトから作る */
        public DeviationRequest() {
            super(new LinkedHashMap<String, Object>());
            json.put("op", "deviation");
        }

        /** メッセージのスキーマの大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public DeviationRequest withV(Long value) {
            put("v", value);
            return this;
        }

        /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
        public Budget getBudget() {
            return Json.read(json.get("budget"), Budget::of);
        }

        /** {@link #getBudget()} を書く。null なら欄を除く */
        public DeviationRequest withBudget(Budget value) {
            put("budget", value);
            return this;
        }

        /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
        public Boolean getPartial() {
            return Json.read(json.get("partial"), Json::bool);
        }

        /** {@link #getPartial()} を書く。null なら欄を除く */
        public DeviationRequest withPartial(Boolean value) {
            put("partial", value);
            return this;
        }

        /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
        public Long getMaxPlaces() {
            return Json.read(json.get("max_places"), Json::integer);
        }

        /** {@link #getMaxPlaces()} を書く。null なら欄を除く */
        public DeviationRequest withMaxPlaces(Long value) {
            put("max_places", value);
            return this;
        }

        /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
        public Long getTimeLimitMs() {
            return Json.read(json.get("time_limit_ms"), Json::integer);
        }

        /** {@link #getTimeLimitMs()} を書く。null なら欄を除く */
        public DeviationRequest withTimeLimitMs(Long value) {
            put("time_limit_ms", value);
            return this;
        }

        /** op の名前 */
        public String getOp() {
            return Json.read(json.get("op"), Json::string);
        }

        /** {@link #getOp()} を書く。null なら欄を除く */
        public DeviationRequest withOp(String value) {
            put("op", value);
            return this;
        }

        /** open で開いた点群の文書の名前 */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public DeviationRequest withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** 設計面の TIN */
        public DeviationSurface getSurface() {
            return Json.read(json.get("surface"), DeviationSurface::of);
        }

        /** {@link #getSurface()} を書く。null なら欄を除く */
        public DeviationRequest withSurface(DeviationSurface value) {
            put("surface", value);
            return this;
        }

        /** プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す */
        public List<String> getProfiles() {
            return Json.read(json.get("profiles"), v0 -> Json.list(v0, Json::string));
        }

        /** {@link #getProfiles()} を書く。null なら欄を除く */
        public DeviationRequest withProfiles(List<String> value) {
            put("profiles", value);
            return this;
        }

        /** 規格値に使うプロファイル。profiles の中から選ぶ */
        public ProfileRef getProfile() {
            return Json.read(json.get("profile"), ProfileRef::of);
        }

        /** {@link #getProfile()} を書く。null なら欄を除く */
        public DeviationRequest withProfile(ProfileRef value) {
            put("profile", value);
            return this;
        }

        /** 工種の名前。規則の work と比べ、同じものだけを使う */
        public String getWork() {
            return Json.read(json.get("work"), Json::string);
        }

        /** {@link #getWork()} を書く。null なら欄を除く */
        public DeviationRequest withWork(String value) {
            put("work", value);
            return this;
        }

        /** 返す最初の評価した点の位置。前の応答の next_offset を渡す。書かなければ 0 */
        public Long getOffset() {
            return Json.read(json.get("offset"), Json::integer);
        }

        /** {@link #getOffset()} を書く。null なら欄を除く */
        public DeviationRequest withOffset(Long value) {
            put("offset", value);
            return this;
        }

        /** 返す評価した点の数の上限。書かなければ点を返さない */
        public Long getLimit() {
            return Json.read(json.get("limit"), Json::integer);
        }

        /** {@link #getLimit()} を書く。null なら欄を除く */
        public DeviationRequest withLimit(Long value) {
            put("limit", value);
            return this;
        }
    }

    /** 設計面の TIN を持つ文書と、TIN の ID */
    public static final class DeviationSurface extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DeviationSurface(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DeviationSurface of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DeviationSurface(json);
        }

        /** 空のオブジェクトから作る */
        public DeviationSurface() {
            super(new LinkedHashMap<String, Object>());
        }

        /** open で開いた土木の文書の名前 */
        public String getDoc() {
            return Json.read(json.get("doc"), Json::string);
        }

        /** {@link #getDoc()} を書く。null なら欄を除く */
        public DeviationSurface withDoc(String value) {
            put("doc", value);
            return this;
        }

        /** TIN の ID。civil の surfaces の id */
        public String getId() {
            return Json.read(json.get("id"), Json::string);
        }

        /** {@link #getId()} を書く。null なら欄を除く */
        public DeviationSurface withId(String value) {
            put("id", value);
            return this;
        }
    }

    /** 標高の差の、規則の区分ごとの集計と指摘。合格・適合とは言わない */
    public static final class DeviationResult extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DeviationResult(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DeviationResult of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DeviationResult(json);
        }

        /** 空のオブジェクトから作る */
        public DeviationResult() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 使ったプロファイルの鎖。extends の基から順 */
        public List<ProfileIdentity> getChain() {
            return Json.read(json.get("chain"), v0 -> Json.list(v0, ProfileIdentity::of));
        }

        /** {@link #getChain()} を書く。null なら欄を除く */
        public DeviationResult withChain(List<ProfileIdentity> value) {
            put("chain", value);
            return this;
        }

        /** 点群の点の数 */
        public Long getPoints() {
            return Json.read(json.get("points"), Json::integer);
        }

        /** {@link #getPoints()} を書く。null なら欄を除く */
        public DeviationResult withPoints(Long value) {
            put("points", value);
            return this;
        }

        /** TIN の外なので評価しなかった点の数 */
        public Long getOutside() {
            return Json.read(json.get("outside"), Json::integer);
        }

        /** {@link #getOutside()} を書く。null なら欄を除く */
        public DeviationResult withOutside(Long value) {
            put("outside", value);
            return this;
        }

        /** どの規則の勾配にも入らない三角形の上なので評価しなかった点の数 */
        public Long getUnmatched() {
            return Json.read(json.get("unmatched"), Json::integer);
        }

        /** {@link #getUnmatched()} を書く。null なら欄を除く */
        public DeviationResult withUnmatched(Long value) {
            put("unmatched", value);
            return this;
        }

        /** 区分の境の近く（規則の exclude の内）なので除いた点の数 */
        public Long getExcluded() {
            return Json.read(json.get("excluded"), Json::integer);
        }

        /** {@link #getExcluded()} を書く。null なら欄を除く */
        public DeviationResult withExcluded(Long value) {
            put("excluded", value);
            return this;
        }

        /** 規則ごとの集計。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す */
        public List<DeviationRule> getRules() {
            return Json.read(json.get("rules"), v0 -> Json.list(v0, DeviationRule::of));
        }

        /** {@link #getRules()} を書く。null なら欄を除く */
        public DeviationResult withRules(List<DeviationRule> value) {
            put("rules", value);
            return this;
        }

        /** 指摘。規則の順。規格値を外れれば profile.surface-elevation.invalid、確認できなければ profile.surface-elevation.unsupported */
        public List<CheckFinding> getFindings() {
            return Json.read(json.get("findings"), v0 -> Json.list(v0, CheckFinding::of));
        }

        /** {@link #getFindings()} を書く。null なら欄を除く */
        public DeviationResult withFindings(List<CheckFinding> value) {
            put("findings", value);
            return this;
        }

        /** 評価した点の数 */
        public Long getEvaluated() {
            return Json.read(json.get("evaluated"), Json::integer);
        }

        /** {@link #getEvaluated()} を書く。null なら欄を除く */
        public DeviationResult withEvaluated(Long value) {
            put("evaluated", value);
            return this;
        }

        /** 評価した点。点群の順に、offset から limit 個まで */
        public List<DeviationPoint> getDifferences() {
            return Json.read(json.get("differences"), v0 -> Json.list(v0, DeviationPoint::of));
        }

        /** {@link #getDifferences()} を書く。null なら欄を除く */
        public DeviationResult withDifferences(List<DeviationPoint> value) {
            put("differences", value);
            return this;
        }

        /** 続きの最初の評価した点の位置。続きがなければ持たない */
        public Long getNextOffset() {
            return Json.read(json.get("next_offset"), Json::integer);
        }

        /** {@link #getNextOffset()} を書く。null なら欄を除く */
        public DeviationResult withNextOffset(Long value) {
            put("next_offset", value);
            return this;
        }
    }

    /** 規則の区分ごとの集計。差はメートル */
    public static final class DeviationRule extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DeviationRule(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DeviationRule of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DeviationRule(json);
        }

        /** 空のオブジェクトから作る */
        public DeviationRule() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 規則の id */
        public String getRule() {
            return Json.read(json.get("rule"), Json::string);
        }

        /** {@link #getRule()} を書く。null なら欄を除く */
        public DeviationRule withRule(String value) {
            put("rule", value);
            return this;
        }

        /** 区分の名前 */
        public String getPart() {
            return Json.read(json.get("part"), Json::string);
        }

        /** {@link #getPart()} を書く。null なら欄を除く */
        public DeviationRule withPart(String value) {
            put("part", value);
            return this;
        }

        /** 評価した点の数 */
        public Long getPoints() {
            return Json.read(json.get("points"), Json::integer);
        }

        /** {@link #getPoints()} を書く。null なら欄を除く */
        public DeviationRule withPoints(Long value) {
            put("points", value);
            return this;
        }

        /** 区分の三角形の、平面に投影した面積（m²） */
        public Double getArea() {
            return Json.read(json.get("area"), Json::number);
        }

        /** {@link #getArea()} を書く。null なら欄を除く */
        public DeviationRule withArea(Double value) {
            put("area", value);
            return this;
        }

        /** 計測の密度（点／m²）。面積がなければ書かない */
        public Double getDensity() {
            return Json.read(json.get("density"), Json::number);
        }

        /** {@link #getDensity()} を書く。null なら欄を除く */
        public DeviationRule withDensity(Double value) {
            put("density", value);
            return this;
        }

        /** 差の平均。点がなければ書かない */
        public Double getMean() {
            return Json.read(json.get("mean"), Json::number);
        }

        /** {@link #getMean()} を書く。null なら欄を除く */
        public DeviationRule withMean(Double value) {
            put("mean", value);
            return this;
        }

        /** 差の最小。点がなければ書かない */
        public Double getMin() {
            return Json.read(json.get("min"), Json::number);
        }

        /** {@link #getMin()} を書く。null なら欄を除く */
        public DeviationRule withMin(Double value) {
            put("min", value);
            return this;
        }

        /** 差の最大。点がなければ書かない */
        public Double getMax() {
            return Json.read(json.get("max"), Json::number);
        }

        /** {@link #getMax()} を書く。null なら欄を除く */
        public DeviationRule withMax(Double value) {
            put("max", value);
            return this;
        }

        /** 指摘の数（確認できなかったものを除く） */
        public Long getFindings() {
            return Json.read(json.get("findings"), Json::integer);
        }

        /** {@link #getFindings()} を書く。null なら欄を除く */
        public DeviationRule withFindings(Long value) {
            put("findings", value);
            return this;
        }

        /** 確認できなかった数 */
        public Long getUnsupported() {
            return Json.read(json.get("unsupported"), Json::integer);
        }

        /** {@link #getUnsupported()} を書く。null なら欄を除く */
        public DeviationRule withUnsupported(Long value) {
            put("unsupported", value);
            return this;
        }
    }

    /** 評価した点 */
    public static final class DeviationPoint extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public DeviationPoint(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static DeviationPoint of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new DeviationPoint(json);
        }

        /** 空のオブジェクトから作る */
        public DeviationPoint() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 点群の中の番号 */
        public Long getIndex() {
            return Json.read(json.get("index"), Json::integer);
        }

        /** {@link #getIndex()} を書く。null なら欄を除く */
        public DeviationPoint withIndex(Long value) {
            put("index", value);
            return this;
        }

        /** 北・東・標高（メートル） */
        public List<Double> getPosition() {
            return Json.read(json.get("position"), v0 -> Json.list(v0, Json::number));
        }

        /** {@link #getPosition()} を書く。null なら欄を除く */
        public DeviationPoint withPosition(List<Double> value) {
            put("position", value);
            return this;
        }

        /** 点の標高から設計面の標高を引いた差（メートル） */
        public Double getDifference() {
            return Json.read(json.get("difference"), Json::number);
        }

        /** {@link #getDifference()} を書く。null なら欄を除く */
        public DeviationPoint withDifference(Double value) {
            put("difference", value);
            return this;
        }

        /** 評価した規則の id */
        public String getRule() {
            return Json.read(json.get("rule"), Json::string);
        }

        /** {@link #getRule()} を書く。null なら欄を除く */
        public DeviationPoint withRule(String value) {
            put("rule", value);
            return this;
        }
    }

    /** describe に使うカタログのファイル。catalog/<言語>.toml から生成し、zm_push で渡す（決定 0009） */
    public static final class Catalog extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public Catalog(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static Catalog of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new Catalog(json);
        }

        /** 空のオブジェクトから作る */
        public Catalog() {
            super(new LinkedHashMap<String, Object>());
        }

        /** カタログの形の大版。今は 1 */
        public Long getV() {
            return Json.read(json.get("v"), Json::integer);
        }

        /** {@link #getV()} を書く。null なら欄を除く */
        public Catalog withV(Long value) {
            put("v", value);
            return this;
        }

        /** 言語（ja・en） */
        public String getLang() {
            return Json.read(json.get("lang"), Json::string);
        }

        /** {@link #getLang()} を書く。null なら欄を除く */
        public Catalog withLang(String value) {
            put("lang", value);
            return this;
        }

        /** パッケージの版。コアと同じ版で出す */
        public String getPackage() {
            return Json.read(json.get("package"), Json::string);
        }

        /** {@link #getPackage()} を書く。null なら欄を除く */
        public Catalog withPackage(String value) {
            put("package", value);
            return this;
        }

        /** 数の書き方 */
        public NumberFormat getNumber() {
            return Json.read(json.get("number"), NumberFormat::of);
        }

        /** {@link #getNumber()} を書く。null なら欄を除く */
        public Catalog withNumber(NumberFormat value) {
            put("number", value);
            return this;
        }

        /** code ごとの文のひな形。{名前} に値が、{count} にまとめた件数が入る */
        public Map<String, String> getCodes() {
            return Json.read(json.get("codes"), v0 -> Json.map(v0, Json::string));
        }

        /** {@link #getCodes()} を書く。null なら欄を除く */
        public Catalog withCodes(Map<String, String> value) {
            put("codes", value);
            return this;
        }
    }

    /** 文に差し込む数の書き方。OS のロケールは使わない（決定 0009） */
    public static final class NumberFormat extends Message {
        /** 読んだ JSON のオブジェクトを包む */
        public NumberFormat(Map<String, Object> json) {
            super(json);
        }

        /** JSON の値を包む。オブジェクトでなければ null */
        public static NumberFormat of(Object value) {
            Map<String, Object> json = Json.object(value);
            return json == null ? null : new NumberFormat(json);
        }

        /** 空のオブジェクトから作る */
        public NumberFormat() {
            super(new LinkedHashMap<String, Object>());
        }

        /** 整数部を 3 桁ごとに区切る文字。空なら区切らない */
        public String getGroupSeparator() {
            return Json.read(json.get("group_separator"), Json::string);
        }

        /** {@link #getGroupSeparator()} を書く。null なら欄を除く */
        public NumberFormat withGroupSeparator(String value) {
            put("group_separator", value);
            return this;
        }

        /** 小数点の文字 */
        public String getDecimalSeparator() {
            return Json.read(json.get("decimal_separator"), Json::string);
        }

        /** {@link #getDecimalSeparator()} を書く。null なら欄を除く */
        public NumberFormat withDecimalSeparator(String value) {
            put("decimal_separator", value);
            return this;
        }
    }

    /** op の名前。要求の型は {@code <Op>Request}、結果の型は {@code <Op>Result} */
    public static final class Operations {
        private Operations() {}

        public static final String CAPABILITIES = "capabilities";

        public static final String OPEN = "open";

        public static final String CLOSE = "close";

        public static final String DESCRIBE = "describe";

        public static final String DRAWING = "drawing";

        public static final String ENTITIES = "entities";

        public static final String APPLY = "apply";

        public static final String CONVERT = "convert";

        public static final String TABLES = "tables";

        public static final String SCENE = "scene";

        public static final String CHECK = "check";

        public static final String VALIDATE = "validate";

        public static final String REVIEW_CARRY = "review_carry";

        public static final String REVIEW_ADOPT = "review_adopt";

        public static final String BUILDING = "building";

        public static final String BUILDING_ELEMENTS = "building_elements";

        public static final String IDS_CHECK = "ids_check";

        public static final String REVIEW_ADOPT_IDS = "review_adopt_ids";

        public static final String BCF_READ = "bcf_read";

        public static final String BCF_WRITE = "bcf_write";

        public static final String CIVIL = "civil";

        public static final String POINTCLOUD = "pointcloud";

        public static final String DEVIATION = "deviation";

        public static final String CIVIL_DRAWING = "civil_drawing";

        public static final String GEO = "geo";

        public static final String DELIVERY_CHECK = "delivery_check";

        public static final String PHOTO = "photo";

        public static final String SHEET = "sheet";

        public static final String BORING = "boring";
    }
}
