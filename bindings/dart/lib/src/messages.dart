// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。
//
// API のメッセージの型（決定 0012・0227）。スキーマの定義ごとに 1 つの型がある。型は JSON のオブジェクトを包む
// ビューで、知らないフィールドもそのまま持つ。列挙は文字列で読み、知らない値も受け取る（決定 0015）。

// ignore_for_file: lines_longer_than_80_chars

import 'dart:core';
import 'dart:core' as core;

import 'message.dart';

/// 要求。op の値で形が決まる
class Request extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Request.fromJson(super.json);

  /// 空のオブジェクトから作る
  Request() : super({});

  /// op が capabilities の形なら、そのビュー。違えば null
  CapabilitiesRequest? get asCapabilitiesRequest => taggedView('op', 'capabilities', CapabilitiesRequest.fromJson);

  /// op が open の形なら、そのビュー。違えば null
  OpenRequest? get asOpenRequest => taggedView('op', 'open', OpenRequest.fromJson);

  /// op が close の形なら、そのビュー。違えば null
  CloseRequest? get asCloseRequest => taggedView('op', 'close', CloseRequest.fromJson);

  /// op が describe の形なら、そのビュー。違えば null
  DescribeRequest? get asDescribeRequest => taggedView('op', 'describe', DescribeRequest.fromJson);

  /// op が drawing の形なら、そのビュー。違えば null
  DrawingRequest? get asDrawingRequest => taggedView('op', 'drawing', DrawingRequest.fromJson);

  /// op が entities の形なら、そのビュー。違えば null
  EntitiesRequest? get asEntitiesRequest => taggedView('op', 'entities', EntitiesRequest.fromJson);

  /// op が calibrations の形なら、そのビュー。違えば null
  CalibrationsRequest? get asCalibrationsRequest => taggedView('op', 'calibrations', CalibrationsRequest.fromJson);

  /// op が measure_distance の形なら、そのビュー。違えば null
  MeasureDistanceRequest? get asMeasureDistanceRequest => taggedView('op', 'measure_distance', MeasureDistanceRequest.fromJson);

  /// op が measure の形なら、そのビュー。違えば null
  MeasureRequest? get asMeasureRequest => taggedView('op', 'measure', MeasureRequest.fromJson);

  /// op が intersect の形なら、そのビュー。違えば null
  IntersectRequest? get asIntersectRequest => taggedView('op', 'intersect', IntersectRequest.fromJson);

  /// op が regions の形なら、そのビュー。違えば null
  RegionsRequest? get asRegionsRequest => taggedView('op', 'regions', RegionsRequest.fromJson);

  /// op が construct の形なら、そのビュー。違えば null
  ConstructRequest? get asConstructRequest => taggedView('op', 'construct', ConstructRequest.fromJson);

  /// op が apply の形なら、そのビュー。違えば null
  ApplyRequest? get asApplyRequest => taggedView('op', 'apply', ApplyRequest.fromJson);

  /// op が create の形なら、そのビュー。違えば null
  CreateRequest? get asCreateRequest => taggedView('op', 'create', CreateRequest.fromJson);

  /// op が convert の形なら、そのビュー。違えば null
  ConvertRequest? get asConvertRequest => taggedView('op', 'convert', ConvertRequest.fromJson);

  /// op が convert_estimate の形なら、そのビュー。違えば null
  ConvertEstimateRequest? get asConvertEstimateRequest => taggedView('op', 'convert_estimate', ConvertEstimateRequest.fromJson);

  /// op が tables の形なら、そのビュー。違えば null
  TablesRequest? get asTablesRequest => taggedView('op', 'tables', TablesRequest.fromJson);

  /// op が scene の形なら、そのビュー。違えば null
  SceneRequest? get asSceneRequest => taggedView('op', 'scene', SceneRequest.fromJson);

  /// op が scene3d の形なら、そのビュー。違えば null
  Scene3dRequest? get asScene3dRequest => taggedView('op', 'scene3d', Scene3dRequest.fromJson);

  /// op が check の形なら、そのビュー。違えば null
  CheckRequest? get asCheckRequest => taggedView('op', 'check', CheckRequest.fromJson);

  /// op が validate の形なら、そのビュー。違えば null
  ValidateRequest? get asValidateRequest => taggedView('op', 'validate', ValidateRequest.fromJson);

  /// op が review_carry の形なら、そのビュー。違えば null
  ReviewCarryRequest? get asReviewCarryRequest => taggedView('op', 'review_carry', ReviewCarryRequest.fromJson);

  /// op が review_adopt の形なら、そのビュー。違えば null
  ReviewAdoptRequest? get asReviewAdoptRequest => taggedView('op', 'review_adopt', ReviewAdoptRequest.fromJson);

  /// op が review_scene の形なら、そのビュー。違えば null
  ReviewSceneRequest? get asReviewSceneRequest => taggedView('op', 'review_scene', ReviewSceneRequest.fromJson);

  /// op が building の形なら、そのビュー。違えば null
  BuildingRequest? get asBuildingRequest => taggedView('op', 'building', BuildingRequest.fromJson);

  /// op が building_elements の形なら、そのビュー。違えば null
  BuildingElementsRequest? get asBuildingElementsRequest => taggedView('op', 'building_elements', BuildingElementsRequest.fromJson);

  /// op が building_drawing の形なら、そのビュー。違えば null
  BuildingDrawingRequest? get asBuildingDrawingRequest => taggedView('op', 'building_drawing', BuildingDrawingRequest.fromJson);

  /// op が ids_check の形なら、そのビュー。違えば null
  IdsCheckRequest? get asIdsCheckRequest => taggedView('op', 'ids_check', IdsCheckRequest.fromJson);

  /// op が review_adopt_ids の形なら、そのビュー。違えば null
  ReviewAdoptIdsRequest? get asReviewAdoptIdsRequest => taggedView('op', 'review_adopt_ids', ReviewAdoptIdsRequest.fromJson);

  /// op が bcf_read の形なら、そのビュー。違えば null
  BcfReadRequest? get asBcfReadRequest => taggedView('op', 'bcf_read', BcfReadRequest.fromJson);

  /// op が bcf_write の形なら、そのビュー。違えば null
  BcfWriteRequest? get asBcfWriteRequest => taggedView('op', 'bcf_write', BcfWriteRequest.fromJson);

  /// op が civil の形なら、そのビュー。違えば null
  CivilRequest? get asCivilRequest => taggedView('op', 'civil', CivilRequest.fromJson);

  /// op が pointcloud の形なら、そのビュー。違えば null
  PointcloudRequest? get asPointcloudRequest => taggedView('op', 'pointcloud', PointcloudRequest.fromJson);

  /// op が deviation の形なら、そのビュー。違えば null
  DeviationRequest? get asDeviationRequest => taggedView('op', 'deviation', DeviationRequest.fromJson);

  /// op が civil_drawing の形なら、そのビュー。違えば null
  CivilDrawingRequest? get asCivilDrawingRequest => taggedView('op', 'civil_drawing', CivilDrawingRequest.fromJson);

  /// op が geo の形なら、そのビュー。違えば null
  GeoRequest? get asGeoRequest => taggedView('op', 'geo', GeoRequest.fromJson);

  /// op が delivery_check の形なら、そのビュー。違えば null
  DeliveryCheckRequest? get asDeliveryCheckRequest => taggedView('op', 'delivery_check', DeliveryCheckRequest.fromJson);

  /// op が delivery_apply の形なら、そのビュー。違えば null
  DeliveryApplyRequest? get asDeliveryApplyRequest => taggedView('op', 'delivery_apply', DeliveryApplyRequest.fromJson);

  /// op が photo の形なら、そのビュー。違えば null
  PhotoRequest? get asPhotoRequest => taggedView('op', 'photo', PhotoRequest.fromJson);

  /// op が boring の形なら、そのビュー。違えば null
  BoringRequest? get asBoringRequest => taggedView('op', 'boring', BoringRequest.fromJson);

  /// op が building_apply の形なら、そのビュー。違えば null
  BuildingApplyRequest? get asBuildingApplyRequest => taggedView('op', 'building_apply', BuildingApplyRequest.fromJson);

  /// op が lift の形なら、そのビュー。違えば null
  LiftRequest? get asLiftRequest => taggedView('op', 'lift', LiftRequest.fromJson);

  /// op が render の形なら、そのビュー。違えば null
  RenderRequest? get asRenderRequest => taggedView('op', 'render', RenderRequest.fromJson);

  /// op が raster の形なら、そのビュー。違えば null
  RasterRequest? get asRasterRequest => taggedView('op', 'raster', RasterRequest.fromJson);

  /// op が georeference の形なら、そのビュー。違えば null
  GeoreferenceRequest? get asGeoreferenceRequest => taggedView('op', 'georeference', GeoreferenceRequest.fromJson);
}

/// どの op の要求にも置けるフィールド
class RequestCommon extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RequestCommon.fromJson(super.json);

  /// 空のオブジェクトから作る
  RequestCommon() : super({});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);
}

/// 予算の単位ごとの上限（決定 0010）
class Budget extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Budget.fromJson(super.json);

  /// 空のオブジェクトから作る
  Budget() : super({});

  /// 受け取ったバイト数
  Object? get inputBytes => readField('input_bytes', asJson);
  set inputBytes(Object? value) => writeField('input_bytes', value);

  /// 展開・復号で作ったバイト数。入れ子の分も足す
  Object? get decodedBytes => readField('decoded_bytes', asJson);
  set decodedBytes(Object? value) => writeField('decoded_bytes', value);

  /// 図面モデル（L2）が保持する要素・表・拡張の論理バイト数。マイナーバージョン 113 から
  Object? get modelBytes => readField('model_bytes', asJson);
  set modelBytes(Object? value) => writeField('model_bytes', value);

  /// L0・L1 で読んだレコードの数
  Object? get records => readField('records', asJson);
  set records(Object? value) => writeField('records', value);

  /// L2・L3 で作った要素の数
  Object? get elements => readField('elements', asJson);
  set elements(Object? value) => writeField('elements', value);

  /// 入れ子と再帰の深さ
  Object? get depth => readField('depth', asJson);
  set depth(Object? value) => writeField('depth', value);

  /// 幾何の分割で作った点・線・面の数
  Object? get segments => readField('segments', asJson);
  set segments(Object? value) => writeField('segments', value);

  /// ほかの単位に入らない繰り返しの回数
  Object? get work => readField('work', asJson);
  set work(Object? value) => writeField('work', value);
}

/// コアのバージョンと、使える op・形式を尋ねる
class CapabilitiesRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CapabilitiesRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CapabilitiesRequest() : super({'op': 'capabilities'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);
}

/// コアのバージョンと、使える op・形式（決定 0015）
class CapabilitiesResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CapabilitiesResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  CapabilitiesResult() : super({});

  /// パッケージのバージョン（semver）
  String? get package => readField('package', asString);
  set package(String? value) => writeField('package', value);

  /// ABI のメジャーバージョン。zm_abi_version と同じ
  core.int? get abi => readField('abi', asInt);
  set abi(core.int? value) => writeField('abi', value);

  /// メッセージのスキーマのバージョン
  SchemaVersion? get schema => readField('schema', objectOf(SchemaVersion.fromJson));
  set schema(SchemaVersion? value) => writeField('schema', value);

  /// このコア（wasm ではこのモジュール）で使える op の名前
  List<String>? get ops => readField('ops', listOf(asString));
  set ops(List<String>? value) => writeField('ops', value);

  /// 読み書きできる形式
  List<Format>? get formats => readField('formats', listOf(objectOf(Format.fromJson)));
  set formats(List<Format>? value) => writeField('formats', value);
}

/// メッセージのスキーマのバージョン。メジャーバージョンは要求と応答の v、マイナーバージョンは op・フィールド・列挙の値を足すたびに上げる
class SchemaVersion extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SchemaVersion.fromJson(super.json);

  /// 空のオブジェクトから作る
  SchemaVersion() : super({});

  /// メジャーバージョン
  core.int? get major => readField('major', asInt);
  set major(core.int? value) => writeField('major', value);

  /// マイナーバージョン
  core.int? get minor => readField('minor', asInt);
  set minor(core.int? value) => writeField('minor', value);
}

/// 読み書きできる形式
class Format extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Format.fromJson(super.json);

  /// 空のオブジェクトから作る
  Format() : super({});

  /// 形式の名前（dxf・sfc など）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 形式の系統（autocad・sxf など、決定 0008）
  String? get family => readField('family', asString);
  set family(String? value) => writeField('family', value);

  /// 読めるか
  core.bool? get read => readField('read', asBool);
  set read(core.bool? value) => writeField('read', value);

  /// 書けるか
  core.bool? get write => readField('write', asBool);
  set write(core.bool? value) => writeField('write', value);

  /// create で空の文書を作れるか。マイナーバージョン 83 から
  core.bool? get create => readField('create', asBool);
  set create(core.bool? value) => writeField('create', value);

  /// この形式の ID の規則のバージョン（決定 0015）
  core.int? get idRules => readField('id_rules', asInt);
  set idRules(core.int? value) => writeField('id_rules', value);
}

/// 入力を読んで、文書を作る。形式は入力から判定する
class OpenRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  OpenRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  OpenRequest() : super({'op': 'open'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 読む入力の名前。zm_push で渡したもの
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);

  /// 作る文書の名前。書かなければ入力の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。マイナーバージョン 48 から
  core.int? get size => readField('size', asInt);
  set size(core.int? value) => writeField('size', value);

  /// 見分けるマーカーのない形式（xyz・extdm・geojson）を名指して読む。書かなければ、入力から判定する（決定 0232）。マイナーバージョン 49 から。extdm はマイナーバージョン 62 から。geojson はマイナーバージョン 71 から
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// SXF の属性ファイル（SAF）を入れた入力の名前。zm_push で渡したもの。属性ファイル用の属性付加機構（$$ATRF$$）の要素に、属性を sxf.saf の拡張で結ぶ（決定 0249）。書かなければ、P2Z・SFZ の中の、図面と同じ名前の SAF を使う。SXF でない入力には使えない。マイナーバージョン 63 から
  String? get saf => readField('saf', asString);
  set saf(String? value) => writeField('saf', value);

  /// IFC-SPF・ifcZIP の 8 ビットのエスケープを読む文字コード。省略時は auto。文書の building_apply 後も指定を保つ。IFC 以外の入力には使えない。マイナーバージョン 76 から
  String? get ifcTextEncoding => readField('ifc_text_encoding', asString);
  set ifcTextEncoding(String? value) => writeField('ifc_text_encoding', value);

  /// COPC を、この条件に掛かる八分木の節の範囲だけで読む。足りない範囲は needs で求める。作った文書は読んだ節の点だけを持ち、pointcloud・scene3d はその点を扱う。条件を変えるときは開き直す。書かなければ全体を読む。COPC でない入力には使えない（#899、決定 0232）。マイナーバージョン 104 から
  OpenPointcloud? get pointcloud => readField('pointcloud', objectOf(OpenPointcloud.fromJson));
  set pointcloud(OpenPointcloud? value) => writeField('pointcloud', value);
}

/// COPC の八分木の節を選ぶ条件。pointcloud の op の同じ名前のフィールドと同じ規則で節を選ぶ
class OpenPointcloud extends Message {
  /// 読んだ JSON のオブジェクトを包む
  OpenPointcloud.fromJson(super.json);

  /// 空のオブジェクトから作る
  OpenPointcloud() : super({});

  /// 節を選ぶ範囲。書かなければ全体
  PointcloudBox? get region => readField('region', objectOf(PointcloudBox.fromJson));
  set region(PointcloudBox? value) => writeField('region', value);

  /// 許容差（点の間隔、メートル）。これより細かい段の節を読まない。書かなければすべての段
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 点の数の上限。節を浅い段から順に数え、範囲にすべて入る節の点の数の和が上限に届いたら、それより後の節を読まない。範囲に一部だけ掛かる節は数えない。書かなければ上限なし
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);
}

/// IFC の文字列の読み方。auto はファイル内に CP932 の手がかりがあれば読み替える。iso_8859_1 は従来の ISO 8859-1 と Unicode、shift_jis は手がかりなしでも CP932 として読める文字列を読み替える。どの指定も元のバイト列を保つ
///
/// 列挙の値。知らない値も受け取る
abstract final class IfcTextEncoding {
  static const auto = 'auto';
  static const iso88591 = 'iso_8859_1';
  static const shiftJis = 'shift_jis';
}

/// 作った文書
class OpenResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  OpenResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  OpenResult() : super({});

  /// 文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 判定した形式の名前
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。マイナーバージョン 48 から
  List<InputRange>? get needs => readField('needs', listOf(objectOf(InputRange.fromJson)));
  set needs(List<InputRange>? value) => writeField('needs', value);

  /// 読み終えたので、close の inputs で捨ててよい範囲の入力。マイナーバージョン 48 から
  List<InputRange>? get done => readField('done', listOf(objectOf(InputRange.fromJson)));
  set done(List<InputRange>? value) => writeField('done', value);

  /// SXF を ZIP に入れたアーカイブ（P2Z・SFZ）なら、SXF の項目のほかの項目（属性ファイル・画像など）のアーカイブの中の名前。アーカイブの順。apply は、SXF の項目だけを置き換え、ほかの項目を写したアーカイブを書く（#385）。マイナーバージョン 54 から
  List<String>? get entries => readField('entries', listOf(asString));
  set entries(List<String>? value) => writeField('entries', value);

  /// open の pointcloud の上限のために、読まなかった節がある。COPC を条件で読んだときだけ書く（#899）。マイナーバージョン 104 から
  core.bool? get truncated => readField('truncated', asBool);
  set truncated(core.bool? value) => writeField('truncated', value);
}

/// 入力の範囲と、その範囲を渡す入力の名前
class InputRange extends Message {
  /// 読んだ JSON のオブジェクトを包む
  InputRange.fromJson(super.json);

  /// 空のオブジェクトから作る
  InputRange() : super({});

  /// 範囲を渡す入力の名前。<入力の名前>#<start>-<end>
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);

  /// 範囲の初め（バイト）
  core.int? get start => readField('start', asInt);
  set start(core.int? value) => writeField('start', value);

  /// 範囲の終わり（バイト、含まない）
  core.int? get end => readField('end', asInt);
  set end(core.int? value) => writeField('end', value);
}

/// 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
class CloseRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CloseRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CloseRequest() : super({'op': 'close'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 捨てる文書の名前
  List<String>? get docs => readField('docs', listOf(asString));
  set docs(List<String>? value) => writeField('docs', value);

  /// 捨てる入力の名前
  List<String>? get inputs => readField('inputs', listOf(asString));
  set inputs(List<String>? value) => writeField('inputs', value);
}

/// 捨てたこと。中身はない
class CloseResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CloseResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  CloseResult() : super({});
}

/// 診断の文を、カタログから組み立てる（決定 0009）
class DescribeRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DescribeRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  DescribeRequest() : super({'op': 'describe'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// カタログの入力の名前。zm_push で渡したもの。書かなければ、code と値をそのまま文にする
  String? get catalog => readField('catalog', asString);
  set catalog(String? value) => writeField('catalog', value);

  /// 文にする診断。応答で受け取ったもの
  List<Diagnostic>? get diagnostics => readField('diagnostics', listOf(objectOf(Diagnostic.fromJson)));
  set diagnostics(List<Diagnostic>? value) => writeField('diagnostics', value);
}

/// 診断の文
class DescribeResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DescribeResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  DescribeResult() : super({});

  /// 診断ごとの文。要求の diagnostics と同じ順。カタログにない code は、code と値をそのまま書く
  List<String>? get texts => readField('texts', listOf(asString));
  set texts(List<String>? value) => writeField('texts', value);
}

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合テストで比べるためにも使う（決定 0007）
class DrawingRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DrawingRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  DrawingRequest() : super({'op': 'drawing'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);
}

/// 図面モデルの要約。移さなかった図形と値は、診断で返す
class DrawingResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DrawingResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  DrawingResult() : super({});

  /// 文書のハッシュ。open に渡したバイト列から作る。変更の一覧の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 実寸の空間。モデルの順
  List<SpaceSummary>? get spaces => readField('spaces', listOf(objectOf(SpaceSummary.fromJson)));
  set spaces(List<SpaceSummary>? value) => writeField('spaces', value);

  /// 用紙。モデルの順
  List<SheetSummary>? get sheets => readField('sheets', listOf(objectOf(SheetSummary.fromJson)));
  set sheets(List<SheetSummary>? value) => writeField('sheets', value);

  /// 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。マイナーバージョン 39 から
  PointStyleSummary? get pointStyle => readField('point_style', objectOf(PointStyleSummary.fromJson));
  set pointStyle(PointStyleSummary? value) => writeField('point_style', value);

  /// 図面の実寸の空間から地図の座標系への相似の変換。DXF・DWG の GEODATA から読めたときだけ持つ。マイナーバージョン 112 から
  Georeference? get georeference => readField('georeference', objectOf(Georeference.fromJson));
  set georeference(Georeference? value) => writeField('georeference', value);
}

/// 図面の実寸から地図の東・北・高さ（m）への相似の変換。東は scale * (x_axis_abscissa * x - x_axis_ordinate * y) + eastings、北は scale * (x_axis_ordinate * x + x_axis_abscissa * y) + northings、高さは scale * z + orthogonal_height
class MapConversion extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MapConversion.fromJson(super.json);

  /// 空のオブジェクトから作る
  MapConversion() : super({});

  /// 東への移動（m）
  core.double? get eastings => readField('eastings', asDouble);
  set eastings(core.double? value) => writeField('eastings', value);

  /// 北への移動（m）
  core.double? get northings => readField('northings', asDouble);
  set northings(core.double? value) => writeField('northings', value);

  /// 高さへの移動（m）
  core.double? get orthogonalHeight => readField('orthogonal_height', asDouble);
  set orthogonalHeight(core.double? value) => writeField('orthogonal_height', value);

  /// 図面の X 軸の東方向の単位ベクトル成分
  core.double? get xAxisAbscissa => readField('x_axis_abscissa', asDouble);
  set xAxisAbscissa(core.double? value) => writeField('x_axis_abscissa', value);

  /// 図面の X 軸の北方向の単位ベクトル成分
  core.double? get xAxisOrdinate => readField('x_axis_ordinate', asDouble);
  set xAxisOrdinate(core.double? value) => writeField('x_axis_ordinate', value);

  /// 図面の 1 単位あたりの地図のメートル数
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);
}

/// 地図の座標系と、図面の実寸から地図への相似の変換
class Georeference extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Georeference.fromJson(super.json);

  /// 空のオブジェクトから作る
  Georeference() : super({});

  /// 地図の座標系の EPSG のコード
  core.int? get crs => readField('crs', asInt);
  set crs(core.int? value) => writeField('crs', value);

  /// 図面の実寸から地図への相似の変換
  MapConversion? get conversion => readField('conversion', objectOf(MapConversion.fromJson));
  set conversion(MapConversion? value) => writeField('conversion', value);
}

/// 地図の投影座標（m）。東（easting）と北（northing）を名前で分ける
class MapPoint2 extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MapPoint2.fromJson(super.json);

  /// 空のオブジェクトから作る
  MapPoint2() : super({});

  /// 東方向の座標（m。平面直角座標系の Y）
  core.double? get easting => readField('easting', asDouble);
  set easting(core.double? value) => writeField('easting', value);

  /// 北方向の座標（m。平面直角座標系の X）
  core.double? get northing => readField('northing', asDouble);
  set northing(core.double? value) => writeField('northing', value);
}

/// 図面の実寸の点と、対応する地図の投影座標の組
class GeoreferenceControlPoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoreferenceControlPoint.fromJson(super.json);

  /// 空のオブジェクトから作る
  GeoreferenceControlPoint() : super({});

  /// 図面の実寸の点
  Point2? get drawing => readField('drawing', objectOf(Point2.fromJson));
  set drawing(Point2? value) => writeField('drawing', value);

  /// 対応する地図の投影座標（m）
  MapPoint2? get map => readField('map', objectOf(MapPoint2.fromJson));
  set map(MapPoint2? value) => writeField('map', value);
}

/// 開いた図面の GEODATA、または対応点と地図の平面直角座標系から、図面の実寸から地図への相似の変換を求め、必要なら図面の点を地図の座標へ写す。マイナーバージョン 112 から
class GeoreferenceRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoreferenceRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  GeoreferenceRequest() : super({'op': 'georeference'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 開いた図面の文書の名前。GEODATA から変換を読むときに渡し、crs・control_points と一緒には渡せない
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// doc に期待する元の文書のハッシュ。doc と一緒にだけ渡せる
  String? get expectedHash => readField('expected_hash', asString);
  set expectedHash(String? value) => writeField('expected_hash', value);

  /// 対応点の地図の平面直角座標系の EPSG のコード（6669〜6687・2443〜2461・30161〜30179）。control_points と一緒に渡す
  core.int? get crs => readField('crs', asInt);
  set crs(core.int? value) => writeField('crs', value);

  /// 図面の実寸の点と地図の投影座標の組（2 組以上）。最小二乗で相似の変換を求める
  List<GeoreferenceControlPoint>? get controlPoints => readField('control_points', listOf(objectOf(GeoreferenceControlPoint.fromJson)));
  set controlPoints(List<GeoreferenceControlPoint>? value) => writeField('control_points', value);

  /// 求めた変換で地図へ写す図面の実寸の点
  List<Point2>? get points => readField('points', listOf(objectOf(Point2.fromJson)));
  set points(List<Point2>? value) => writeField('points', value);
}

/// 図面の実寸から地図への相似の変換と、対応点の残差・写した地図の点
class GeoreferenceResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoreferenceResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  GeoreferenceResult() : super({});

  /// doc を渡したときの元の文書のハッシュ
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 地図の座標系と、図面の実寸から地図への相似の変換。doc の GEODATA から作れなかったときは持たない
  Georeference? get georeference => readField('georeference', objectOf(Georeference.fromJson));
  set georeference(Georeference? value) => writeField('georeference', value);

  /// control_points の組ごとの残差（渡した地図の点から、変換で写した図面の点を引いた投影座標。m）。渡した組の順
  List<MapPoint2>? get residuals => readField('residuals', listOf(objectOf(MapPoint2.fromJson)));
  set residuals(List<MapPoint2>? value) => writeField('residuals', value);

  /// points を変換で地図へ写した投影座標（m）。変換が作れなかったときは持たない
  List<MapPoint2>? get mapped => readField('mapped', listOf(objectOf(MapPoint2.fromJson)));
  set mapped(List<MapPoint2>? value) => writeField('mapped', value);
}

/// 2 次元の座標
class Point2 extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Point2.fromJson(super.json);

  /// 空のオブジェクトから作る
  Point2() : super({});

  /// X 座標
  core.double? get x => readField('x', asDouble);
  set x(core.double? value) => writeField('x', value);

  /// Y 座標
  core.double? get y => readField('y', asDouble);
  set y(core.double? value) => writeField('y', value);
}

/// 原点と相対座標を分けた点。両成分の正確な和を位置とし、先に足して丸めない。座標の単位は使う所に従う（measure_distance では用紙の mm、measure では from の空間か用紙の単位）
class SplitPoint2 extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SplitPoint2.fromJson(super.json);

  /// 空のオブジェクトから作る
  SplitPoint2() : super({});

  /// 用紙の原点
  Point2? get origin => readField('origin', objectOf(Point2.fromJson));
  set origin(Point2? value) => writeField('origin', value);

  /// 原点からの相対座標
  Point2? get offset => readField('offset', objectOf(Point2.fromJson));
  set offset(Point2? value) => writeField('offset', value);
}

/// 用紙の閉じた矩形で使う距離の校正。重なる領域は ID で明示的に選ぶ。マイナーバージョン 68 から
class CalibrationItem extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CalibrationItem.fromJson(super.json);

  /// 空のオブジェクトから作る
  CalibrationItem() : super({});

  /// 文書内で重複しない校正の ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 元の校正の名前。翻訳しない。なければ持たない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 校正を使える公開用紙の ID。drawing が返すもの
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 用紙の mm の閉じた矩形。[X の最小, Y の最小, X の最大, Y の最大] の 4 値で、幅と高さは正
  List<core.double>? get bounds => readField('bounds', listOf(asDouble));
  set bounds(List<core.double>? value) => writeField('bounds', value);

  /// 用紙の mm の変位から校正単位の変位へ写す行優先の 2×2 行列。[a, b, c, d] の 4 値で、[dx, dy] を [a*dx+b*dy, c*dx+d*dy] に写す
  List<core.double>? get matrix => readField('matrix', listOf(asDouble));
  set matrix(List<core.double>? value) => writeField('matrix', value);

  /// 校正後の長さの単位。ラベルだけから推定しない
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 元の単位ラベル。未知の単位でも原文を保つ
  String? get unitLabel => readField('unit_label', asString);
  set unitLabel(String? value) => writeField('unit_label', value);

  /// 元の形式のレコード
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 開いた文書を図面モデルに変換し、用紙の距離の校正をモデルの順に返す。マイナーバージョン 68 から
class CalibrationsRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CalibrationsRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CalibrationsRequest() : super({'op': 'calibrations'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 公開用紙の ID。drawing が返すもの。書けばその用紙の校正だけを返し、書かなければすべての校正を返す
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 返す最初の校正の位置。from で絞った後の位置で、前の応答の next_offset を渡す。書かなければ 0
  core.int? get offset => readField('offset', asInt);
  set offset(core.int? value) => writeField('offset', value);

  /// 返す校正の数の上限。書かなければ 1000
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);

  /// 期待する元の文書のハッシュ。open に渡したバイト列のハッシュと照合する。図面モデルへ変換した ZMDR 自身のハッシュではない
  String? get expectedHash => readField('expected_hash', asString);
  set expectedHash(String? value) => writeField('expected_hash', value);
}

/// 距離の校正の一覧の一部
class CalibrationsResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CalibrationsResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  CalibrationsResult() : super({});

  /// 元の文書のハッシュ。ZMDR から開いたときは、埋め込まれた元の文書のハッシュ（Source.hash）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// from で絞った一覧の offset から limit 個までの校正
  List<CalibrationItem>? get items => readField('items', listOf(objectOf(CalibrationItem.fromJson)));
  set items(List<CalibrationItem>? value) => writeField('items', value);

  /// from で絞った後の校正の総数
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 続きの最初の校正の位置。続きがなければ持たない
  core.int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(core.int? value) => writeField('next_offset', value);
}

/// 用紙と校正を明示的に選び、始点と終点の校正後の距離を求める。両点は校正の閉領域内にある必要がある。マイナーバージョン 68 から
class MeasureDistanceRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MeasureDistanceRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  MeasureDistanceRequest() : super({'op': 'measure_distance'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 公開用紙の ID。drawing が返すもので、選んだ校正と同じ用紙
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 使う校正の ID。calibrations が返すもの
  String? get calibration => readField('calibration', asString);
  set calibration(String? value) => writeField('calibration', value);

  /// 始点。原点と相対座標を分けた用紙の mm の点
  SplitPoint2? get start => readField('start', objectOf(SplitPoint2.fromJson));
  set start(SplitPoint2? value) => writeField('start', value);

  /// 終点。原点と相対座標を分けた用紙の mm の点
  SplitPoint2? get end => readField('end', objectOf(SplitPoint2.fromJson));
  set end(SplitPoint2? value) => writeField('end', value);

  /// 期待する元の文書のハッシュ。open に渡したバイト列のハッシュと照合する。図面モデルへ変換した ZMDR 自身のハッシュではない
  String? get expectedHash => readField('expected_hash', asString);
  set expectedHash(String? value) => writeField('expected_hash', value);
}

/// 選んだ用紙と校正による距離
class MeasureDistanceResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MeasureDistanceResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  MeasureDistanceResult() : super({});

  /// 元の文書のハッシュ。ZMDR から開いたときは、埋め込まれた元の文書のハッシュ（Source.hash）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 計測した公開用紙の ID
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 使った校正の ID
  String? get calibration => readField('calibration', asString);
  set calibration(String? value) => writeField('calibration', value);

  /// 校正後の距離。unit と unit_label が示す単位
  core.double? get distance => readField('distance', asDouble);
  set distance(core.double? value) => writeField('distance', value);

  /// 校正後の長さの単位
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 元の単位ラベル。未知の単位でも原文を保つ
  String? get unitLabel => readField('unit_label', asString);
  set unitLabel(String? value) => writeField('unit_label', value);
}

/// モデル空間の要素の長さ・周長・面積、2 要素または点の間の最短距離を測る。線幅と表示用の変換は含めない
class MeasureRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MeasureRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  MeasureRequest() : super({'op': 'measure'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 開いた文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 実寸の空間の公開 ID。記号定義の空間は対象外。マイナーバージョン 102 から、distance で両方の対象が点なら公開用紙の ID も使え、校正しない用紙の座標の距離を返す
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 求める値
  String? get quantity => readField('quantity', asString);
  set quantity(String? value) => writeField('quantity', value);

  /// 計測する要素または点
  MeasureTarget? get first => readField('first', objectOf(MeasureTarget.fromJson));
  set first(MeasureTarget? value) => writeField('first', value);

  /// 最短距離のもう一方の要素または点。distance のときだけ必須
  MeasureTarget? get second => readField('second', objectOf(MeasureTarget.fromJson));
  set second(MeasureTarget? value) => writeField('second', value);

  /// 近似する曲線の分割で目標にする弦の誤差。空間の長さの単位で正の値。既定は 0.001。計測した長さ・面積・距離の誤差上限ではない
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
  String? get expectedHash => readField('expected_hash', asString);
  set expectedHash(String? value) => writeField('expected_hash', value);
}

/// 長さ、閉じた経路の周長、向き付き線積分の絶対値の面積、境界間の最短距離。面積は自己交差を塗り規則で統合しない
///
/// 列挙の値。知らない値も受け取る
abstract final class MeasureQuantity {
  static const length = 'length';
  static const perimeter = 'perimeter';
  static const area = 'area';
  static const distance = 'distance';
}

/// 同じモデル空間内の要素または座標
class MeasureTarget extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MeasureTarget.fromJson(super.json);

  /// 空のオブジェクトから作る
  MeasureTarget() : super({});

  /// kind が element の形なら、そのビュー。違えば null
  MeasureElement? get asMeasureElement => taggedView('kind', 'element', MeasureElement.fromJson);

  /// kind が point の形なら、そのビュー。違えば null
  MeasurePoint? get asMeasurePoint => taggedView('kind', 'point', MeasurePoint.fromJson);

  /// kind が split_point の形なら、そのビュー。違えば null
  MeasureSplitPoint? get asMeasureSplitPoint => taggedView('kind', 'split_point', MeasureSplitPoint.fromJson);
}

/// 計測する要素。元の公開 ID を使う
class MeasureElement extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MeasureElement.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  MeasureElement() : super({'kind': 'element'});

  /// 対象の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 同じ空間の entities が返した要素の ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);
}

/// 距離の計測で使う空間の座標
class MeasurePoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MeasurePoint.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  MeasurePoint() : super({'kind': 'point'});

  /// 対象の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 空間の長さの単位の点
  Point2? get position => readField('position', objectOf(Point2.fromJson));
  set position(Point2? value) => writeField('position', value);
}

/// 距離の計測で使う、原点と相対座標を分けた座標。distance で、もう一方も点のときだけ使える。マイナーバージョン 102 から
class MeasureSplitPoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MeasureSplitPoint.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  MeasureSplitPoint() : super({'kind': 'split_point'});

  /// 対象の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 原点と相対座標を分けた点。成分ごとの差を正確に求めてから一度だけ丸める
  SplitPoint2? get position => readField('position', objectOf(SplitPoint2.fromJson));
  set position(SplitPoint2? value) => writeField('position', value);
}

/// 要素から求めた計測値。近似値かどうか、単位と出所の空間を明示する
class MeasureResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MeasureResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  MeasureResult() : super({});

  /// 元の文書のハッシュ
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 計測した実寸の空間の公開 ID
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 求めた値の種類
  String? get quantity => readField('quantity', asString);
  set quantity(String? value) => writeField('quantity', value);

  /// 計測値。0 以上の有限値
  core.double? get value => readField('value', asDouble);
  set value(core.double? value) => writeField('value', value);

  /// 空間の長さの単位。不明な単位を補わない。from が用紙なら用紙の mm
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// from が用紙で、value が校正しない用紙の座標の距離なら true。実寸ではない。実寸の空間では返さない。マイナーバージョン 102 から
  core.bool? get sheet => readField('sheet', asBool);
  set sheet(core.bool? value) => writeField('sheet', value);

  /// 単位の次数。長さと距離は 1、面積は 2
  core.int? get unitPower => readField('unit_power', asInt);
  set unitPower(core.int? value) => writeField('unit_power', value);

  /// 計測に曲線の折れ線近似を使った。浮動小数の丸めとは区別する
  core.bool? get approximate => readField('approximate', asBool);
  set approximate(core.bool? value) => writeField('approximate', value);

  /// 指定した、または既定の分割の目標。近似しないときも返す
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 最短距離を作る最初の対象上の点。distance のときだけ返す。split_point なら原点と相対座標の和を丸めた点
  Point2? get nearestFirst => readField('nearest_first', objectOf(Point2.fromJson));
  set nearestFirst(Point2? value) => writeField('nearest_first', value);

  /// 最短距離を作るもう一方の対象上の点。近似した対象では折れ線上の点
  Point2? get nearestSecond => readField('nearest_second', objectOf(Point2.fromJson));
  set nearestSecond(Point2? value) => writeField('nearest_second', value);
}

/// 同じ実寸の空間の 2 要素の交点・接点・重なりを求める。文書を変更しない。マイナーバージョン 90 から
class IntersectRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IntersectRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  IntersectRequest() : super({'op': 'intersect'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 開いた文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 交わりを求める要素
  MeasureElement? get first => readField('first', objectOf(MeasureElement.fromJson));
  set first(MeasureElement? value) => writeField('first', value);

  /// 交わりを求めるもう一方の要素。first と異なる ID
  MeasureElement? get second => readField('second', objectOf(MeasureElement.fromJson));
  set second(MeasureElement? value) => writeField('second', value);

  /// 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、近似する曲線の分割の目標にも使う
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
  String? get expectedHash => readField('expected_hash', asString);
  set expectedHash(String? value) => writeField('expected_hash', value);
}

/// 交点または接点。格子点の座標
class IntersectionPoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IntersectionPoint.fromJson(super.json);

  /// 空のオブジェクトから作る
  IntersectionPoint() : super({});

  /// 空間の長さの単位の点
  Point2? get position => readField('position', objectOf(Point2.fromJson));
  set position(Point2? value) => writeField('position', value);

  /// 接点。同じ格子点にまとめたすべての交わりが接するときだけ真
  core.bool? get tangent => readField('tangent', asBool);
  set tangent(core.bool? value) => writeField('tangent', value);

  /// 正確な交点が格子点でなく、近い格子点へ丸めた。まとめたすべての交わりが丸めたときだけ真
  core.bool? get rounded => readField('rounded', asBool);
  set rounded(core.bool? value) => writeField('rounded', value);
}

/// 重なりの形。segment は線分、arc は反時計回りの円弧、circle は円全体
///
/// 列挙の値。知らない値も受け取る
abstract final class IntersectionOverlapKind {
  static const segment = 'segment';
  static const arc = 'arc';
  static const circle = 'circle';
}

/// 2 要素が長さを持って重なる区間。区間の組ごとに返し、つながる区間をつながない
class IntersectionOverlap extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IntersectionOverlap.fromJson(super.json);

  /// 空のオブジェクトから作る
  IntersectionOverlap() : super({});

  /// 重なりの形
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 始点。segment は X、Y の辞書順で小さい端、arc は反時計回りの始点。circle では持たない
  Point2? get start => readField('start', objectOf(Point2.fromJson));
  set start(Point2? value) => writeField('start', value);

  /// 終点。circle では持たない
  Point2? get end => readField('end', objectOf(Point2.fromJson));
  set end(Point2? value) => writeField('end', value);

  /// 円弧と円の中心。segment では持たない
  Point2? get center => readField('center', objectOf(Point2.fromJson));
  set center(Point2? value) => writeField('center', value);

  /// 円弧と円の格子上の半径。segment では持たない
  core.double? get radius => readField('radius', asDouble);
  set radius(core.double? value) => writeField('radius', value);
}

/// 2 要素の交わり。first と second を入れ替えても同じ結果を返す
class IntersectResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IntersectResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  IntersectResult() : super({});

  /// 元の文書のハッシュ
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 求めた実寸の空間の公開 ID
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 空間の長さの単位。不明な単位を補わない
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 指定した、または既定の許容差
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 曲線を折れ線に近似した要素がある
  core.bool? get approximate => readField('approximate', asBool);
  set approximate(core.bool? value) => writeField('approximate', value);

  /// 交点と接点。格子点の X、Y の辞書順。重なりの端点と同じ点は含めない
  List<IntersectionPoint>? get points => readField('points', listOf(objectOf(IntersectionPoint.fromJson)));
  set points(List<IntersectionPoint>? value) => writeField('points', value);

  /// 重なり。形・座標の順
  List<IntersectionOverlap>? get overlaps => readField('overlaps', listOf(objectOf(IntersectionOverlap.fromJson)));
  set overlaps(List<IntersectionOverlap>? value) => writeField('overlaps', value);
}

/// 同じ実寸の空間の要素の線が囲む閉領域を検出する、または閉じた要素どうしの和・積・差・排他的論理和を求める。文書を変更しない。マイナーバージョン 91 から
class RegionsRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RegionsRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  RegionsRequest() : super({'op': 'regions'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 開いた文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 求め方
  String? get operation => readField('operation', asString);
  set operation(String? value) => writeField('operation', value);

  /// detect では線で面を囲む要素、ほかでは集合演算の左側の閉じた要素。1 つ以上で、同じ ID を繰り返さない
  List<MeasureElement>? get subject => readField('subject', listOf(objectOf(MeasureElement.fromJson)));
  set subject(List<MeasureElement>? value) => writeField('subject', value);

  /// 集合演算の右側の閉じた要素。書かなければ空。detect では書かない。同じ ID を繰り返さない
  List<MeasureElement>? get clip => readField('clip', listOf(objectOf(MeasureElement.fromJson)));
  set clip(List<MeasureElement>? value) => writeField('clip', value);

  /// 閉じた要素の内部を決める規則。書かなければ even_odd。detect では書かない
  String? get fill => readField('fill', asString);
  set fill(String? value) => writeField('fill', value);

  /// 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、円弧を弦に分ける目標にも使う
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
  String? get expectedHash => readField('expected_hash', asString);
  set expectedHash(String? value) => writeField('expected_hash', value);
}

/// 閉領域の求め方。detect は subject の線が囲むすべての有限の面、union・intersection・difference・xor は subject と clip の和・積・差（subject から clip を除く）・排他的論理和
///
/// 列挙の値。知らない値も受け取る
abstract final class RegionOperation {
  static const detect = 'detect';
  static const union = 'union';
  static const intersection = 'intersection';
  static const difference = 'difference';
  static const xor = 'xor';
}

/// 閉じた要素の内部を決める規則。even_odd は境界を横切る回数が奇数の部分、non_zero は符号を付けた周回数が 0 でない部分
///
/// 列挙の値。知らない値も受け取る
abstract final class RegionFill {
  static const evenOdd = 'even_odd';
  static const nonZero = 'non_zero';
}

/// 外周と穴で表した閉領域。最後の点から最初の点へ閉じる
class ClosedRegion extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ClosedRegion.fromJson(super.json);

  /// 空のオブジェクトから作る
  ClosedRegion() : super({});

  /// 反時計回りの外周。格子点の座標で、辞書順で最小の点から始める
  List<Point2>? get outer => readField('outer', listOf(objectOf(Point2.fromJson)));
  set outer(List<Point2>? value) => writeField('outer', value);

  /// 時計回りの穴。輪郭の辞書順
  List<List<Point2>>? get holes => readField('holes', listOf(listOf(objectOf(Point2.fromJson))));
  set holes(List<List<Point2>>? value) => writeField('holes', value);

  /// 外周から穴を除いた面積。空間の長さの単位の 2 乗
  core.double? get area => readField('area', asDouble);
  set area(core.double? value) => writeField('area', value);
}

/// 交点を格子点へ丸めて元の要素の線を曲げた点
class RegionRounding extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RegionRounding.fromJson(super.json);

  /// 空のオブジェクトから作る
  RegionRounding() : super({});

  /// 曲げた要素の公開 ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 丸めた格子点
  Point2? get position => readField('position', objectOf(Point2.fromJson));
  set position(Point2? value) => writeField('position', value);
}

/// 閉領域を作らなかった入力の種類。degenerate は点や一直線に縮んだ要素か長さ 0 の線、open は閉領域の境界に使わなかった線の部分
///
/// 列挙の値。知らない値も受け取る
abstract final class RegionDiscardKind {
  static const degenerate = 'degenerate';
  static const open = 'open';
}

/// 閉領域を作らなかった入力
class RegionDiscard extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RegionDiscard.fromJson(super.json);

  /// 空のオブジェクトから作る
  RegionDiscard() : super({});

  /// 元の要素の公開 ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 使わなかった部分の始点。open だけが持つ
  Point2? get start => readField('start', objectOf(Point2.fromJson));
  set start(Point2? value) => writeField('start', value);

  /// 使わなかった部分の終点。open だけが持つ
  Point2? get end => readField('end', objectOf(Point2.fromJson));
  set end(Point2? value) => writeField('end', value);
}

/// 閉領域。要素の順序を入れ替えても同じ領域を返す
class RegionsResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RegionsResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  RegionsResult() : super({});

  /// 元の文書のハッシュ
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 求めた実寸の空間の公開 ID
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 空間の長さの単位。不明な単位を補わない
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 指定した、または既定の許容差
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 曲線を折れ線に近似した要素がある
  core.bool? get approximate => readField('approximate', asBool);
  set approximate(core.bool? value) => writeField('approximate', value);

  /// 閉領域。外周、穴の順の辞書順
  List<ClosedRegion>? get regions => readField('regions', listOf(objectOf(ClosedRegion.fromJson)));
  set regions(List<ClosedRegion>? value) => writeField('regions', value);

  /// 丸めた分割点。要素の指定順、格子点の辞書順
  List<RegionRounding>? get rounded => readField('rounded', listOf(objectOf(RegionRounding.fromJson)));
  set rounded(List<RegionRounding>? value) => writeField('rounded', value);

  /// 閉領域を作らなかった入力。degenerate、続いて open を元の入力順で返す
  List<RegionDiscard>? get discarded => readField('discarded', listOf(objectOf(RegionDiscard.fromJson)));
  set discarded(List<RegionDiscard>? value) => writeField('discarded', value);
}

/// 同じ実寸の空間の要素のオフセット・トリム・延長・面取り・フィレットの結果の形を求める。文書を変更しない。結果は apply の remove・replace・add で当てる。形の種類が変わるときは replace でなく remove と add にする。マイナーバージョン 92 から
class ConstructRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConstructRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ConstructRequest() : super({'op': 'construct'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 開いた文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 作図の種類
  String? get operation => readField('operation', asString);
  set operation(String? value) => writeField('operation', value);

  /// 作図する要素。線分・円・円弧。面取りとフィレットでは 1 本目の線分
  MeasureElement? get target => readField('target', objectOf(MeasureElement.fromJson));
  set target(MeasureElement? value) => writeField('target', value);

  /// 面取りとフィレットの 2 本目の線分。target と端点を共有し、target と異なる ID
  MeasureElement? get second => readField('second', objectOf(MeasureElement.fromJson));
  set second(MeasureElement? value) => writeField('second', value);

  /// トリムと延長の境界の要素。線分・円・円弧・折れ線と、近似する曲線。1 つ以上で、同じ ID と target を含めない
  List<MeasureElement>? get boundaries => readField('boundaries', listOf(objectOf(MeasureElement.fromJson)));
  set boundaries(List<MeasureElement>? value) => writeField('boundaries', value);

  /// トリムで削る区間を選ぶ点。線分は元の直線へ垂直に投影し、円・円弧は中心からの向きで選ぶ
  Point2? get pick => readField('pick', objectOf(Point2.fromJson));
  set pick(Point2? value) => writeField('pick', value);

  /// 延長する端点
  String? get end => readField('end', asString);
  set end(String? value) => writeField('end', value);

  /// オフセットの距離。線分は始点から終点へ向かって左、円・円弧は外側を正にする
  core.double? get distance => readField('distance', asDouble);
  set distance(core.double? value) => writeField('distance', value);

  /// 面取りで、共有する端点から target と second の上を進む正の距離 [target, second]
  List<core.double>? get distances => readField('distances', listOf(asDouble));
  set distances(List<core.double>? value) => writeField('distances', value);

  /// フィレットの円弧の正の半径
  core.double? get radius => readField('radius', asDouble);
  set radius(core.double? value) => writeField('radius', value);

  /// 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、境界の曲線を近似する目標にも使う
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
  String? get expectedHash => readField('expected_hash', asString);
  set expectedHash(String? value) => writeField('expected_hash', value);
}

/// offset は distance だけ平行に移した新しい形、trim は pick を含む区間を最初に当たる境界の間で削った残り、extend は end の端点を最初に当たる境界まで延ばした形、chamfer は target と second の角を distances で切る線分、fillet は radius の接する円弧
///
/// 列挙の値。知らない値も受け取る
abstract final class ConstructOperation {
  static const offset = 'offset';
  static const trim = 'trim';
  static const extend = 'extend';
  static const chamfer = 'chamfer';
  static const fillet = 'fillet';
}

/// 延長する端点。start は始点を逆向きへ、end は終点を進む向きへ延ばす
///
/// 列挙の値。知らない値も受け取る
abstract final class ConstructEnd {
  static const start = 'start';
  static const end = 'end';
}

/// 元の要素を置き換える形。空なら要素を消す。2 つなら 1 つ目で置き換えて 2 つ目を足す。円をトリムした円弧のように種類が変わる形は、元の要素を消して足す
class ConstructReplacement extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConstructReplacement.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConstructReplacement() : super({});

  /// 置き換える要素の公開 ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 置き換える形。元の向きの順
  List<Geometry>? get geometries => readField('geometries', listOf(objectOf(Geometry.fromJson)));
  set geometries(List<Geometry>? value) => writeField('geometries', value);
}

/// トリムと延長で採用した境界と切断点
class ConstructCut extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConstructCut.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConstructCut() : super({});

  /// 境界の要素の公開 ID
  String? get boundary => readField('boundary', asString);
  set boundary(String? value) => writeField('boundary', value);

  /// 元の交点を丸めた格子点
  Point2? get position => readField('position', objectOf(Point2.fromJson));
  set position(Point2? value) => writeField('position', value);

  /// 元の交点が格子点でなく、近い格子点へ丸めた
  core.bool? get rounded => readField('rounded', asBool);
  set rounded(core.bool? value) => writeField('rounded', value);

  /// 丸めた点を元の線・円の上へ投影したため、正確な切断位置と異なる
  core.bool? get adjusted => readField('adjusted', asBool);
  set adjusted(core.bool? value) => writeField('adjusted', value);
}

/// 作図の結果の形。文書は変わらない
class ConstructResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConstructResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConstructResult() : super({});

  /// 元の文書のハッシュ
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 求めた実寸の空間の公開 ID
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 空間の長さの単位。不明な単位を補わない
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 指定した、または既定の許容差
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 境界の曲線を折れ線に近似した
  core.bool? get approximate => readField('approximate', asBool);
  set approximate(core.bool? value) => writeField('approximate', value);

  /// 置き換える要素。target、second の順
  List<ConstructReplacement>? get replace => readField('replace', listOf(objectOf(ConstructReplacement.fromJson)));
  set replace(List<ConstructReplacement>? value) => writeField('replace', value);

  /// 足す形。オフセットの形、面取りの線分、フィレットの円弧
  List<Geometry>? get add => readField('add', listOf(objectOf(Geometry.fromJson)));
  set add(List<Geometry>? value) => writeField('add', value);

  /// トリムと延長で採用した切断点。元の向きの順
  List<ConstructCut>? get cuts => readField('cuts', listOf(objectOf(ConstructCut.fromJson)));
  set cuts(List<ConstructCut>? value) => writeField('cuts', value);
}

/// 文書の点の表し方。フィールドの意味は point_style_edit と同じ
class PointStyleSummary extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointStyleSummary.fromJson(super.json);

  /// 空のオブジェクトから作る
  PointStyleSummary() : super({});

  /// 基本の形
  String? get mark => readField('mark', asString);
  set mark(String? value) => writeField('mark', value);

  /// 円で囲む
  core.bool? get circle => readField('circle', asBool);
  set circle(core.bool? value) => writeField('circle', value);

  /// 四角で囲む
  core.bool? get square => readField('square', asBool);
  set square(core.bool? value) => writeField('square', value);

  /// 大きさ。意味は size_kind で決まる
  core.double? get size => readField('size', asDouble);
  set size(core.double? value) => writeField('size', value);

  /// 大きさの意味
  String? get sizeKind => readField('size_kind', asString);
  set sizeKind(String? value) => writeField('size_kind', value);
}

/// 実寸の空間の要約
class SpaceSummary extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SpaceSummary.fromJson(super.json);

  /// 空のオブジェクトから作る
  SpaceSummary() : super({});

  /// 空間の ID（space/0）。entities の要素の ID の始まりと同じ（決定 0156）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 文書の単位
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 要素の数
  ElementCounts? get elements => readField('elements', objectOf(ElementCounts.fromJson));
  set elements(ElementCounts? value) => writeField('elements', value);

  /// 線分の端点と文字の基点を囲む範囲。文書の単位。要素がなければ持たない
  Extent? get extent => readField('extent', objectOf(Extent.fromJson));
  set extent(Extent? value) => writeField('extent', value);
}

/// 用紙の要約
class SheetSummary extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SheetSummary.fromJson(super.json);

  /// 空のオブジェクトから作る
  SheetSummary() : super({});

  /// 用紙の ID。LAYOUT がハンドルを持てば sheet/h22、なければ読んだ順の番号（sheet/0）。entities の要素の ID の始まりと同じ（決定 0156）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 元の形式の用紙の名前（DXF・DWG のレイアウトの名前）。翻訳しない。なければ持たない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 用紙の幅と高さ（mm）。分からなければ持たない
  List<core.double>? get size => readField('size', listOf(asDouble));
  set size(List<core.double>? value) => writeField('size', value);

  /// 要素の数
  ElementCounts? get elements => readField('elements', objectOf(ElementCounts.fromJson));
  set elements(ElementCounts? value) => writeField('elements', value);

  /// ビューの数
  core.int? get views => readField('views', asInt);
  set views(core.int? value) => writeField('views', value);

  /// ビューの詳細。元の順に返す。マイナーバージョン 81 から
  List<ViewDetail>? get viewDetails => readField('view_details', listOf(objectOf(ViewDetail.fromJson)));
  set viewDetails(List<ViewDetail>? value) => writeField('view_details', value);

  /// 線分の端点と文字の基点を囲む範囲。用紙の mm。要素がなければ持たない
  Extent? get extent => readField('extent', objectOf(Extent.fromJson));
  set extent(Extent? value) => writeField('extent', value);
}

/// 実寸の単位
///
/// 列挙の値。知らない値も受け取る
abstract final class Unit {
  static const millimeter = 'millimeter';
  static const centimeter = 'centimeter';
  static const meter = 'meter';
  static const inch = 'inch';
  static const foot = 'foot';
  static const unknown = 'unknown';
}

/// 種類ごとの要素の数
class ElementCounts extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ElementCounts.fromJson(super.json);

  /// 空のオブジェクトから作る
  ElementCounts() : super({});

  /// 線分
  core.int? get lines => readField('lines', asInt);
  set lines(core.int? value) => writeField('lines', value);

  /// 文字
  core.int? get texts => readField('texts', asInt);
  set texts(core.int? value) => writeField('texts', value);

  /// 円。マイナーバージョン 81 から
  core.int? get circles => readField('circles', asInt);
  set circles(core.int? value) => writeField('circles', value);

  /// 円弧・楕円弧・スプラインなどの曲線。マイナーバージョン 81 から
  core.int? get curves => readField('curves', asInt);
  set curves(core.int? value) => writeField('curves', value);

  /// 折れ線。マイナーバージョン 81 から
  core.int? get polylines => readField('polylines', asInt);
  set polylines(core.int? value) => writeField('polylines', value);

  /// 書式つきの文字。マイナーバージョン 81 から
  core.int? get formattedTexts => readField('formatted_texts', asInt);
  set formattedTexts(core.int? value) => writeField('formatted_texts', value);

  /// 寸法。マイナーバージョン 81 から
  core.int? get dimensions => readField('dimensions', asInt);
  set dimensions(core.int? value) => writeField('dimensions', value);

  /// 引出線。マイナーバージョン 81 から
  core.int? get leaders => readField('leaders', asInt);
  set leaders(core.int? value) => writeField('leaders', value);

  /// 引出線つきの注記。マイナーバージョン 81 から
  core.int? get callouts => readField('callouts', asInt);
  set callouts(core.int? value) => writeField('callouts', value);

  /// 表。マイナーバージョン 81 から
  core.int? get tables => readField('tables', asInt);
  set tables(core.int? value) => writeField('tables', value);

  /// アンダーレイ。マイナーバージョン 81 から
  core.int? get underlays => readField('underlays', asInt);
  set underlays(core.int? value) => writeField('underlays', value);

  /// 記号の配置。中の要素を展開しない。マイナーバージョン 81 から
  core.int? get symbols => readField('symbols', asInt);
  set symbols(core.int? value) => writeField('symbols', value);

  /// ハッチング。マイナーバージョン 81 から
  core.int? get hatches => readField('hatches', asInt);
  set hatches(core.int? value) => writeField('hatches', value);

  /// 点。マイナーバージョン 81 から
  core.int? get points => readField('points', asInt);
  set points(core.int? value) => writeField('points', value);
}

/// 2D のビュー。変換行列と切り抜きを保ち、尺度・向き・見えるモデル範囲を決定的に求める
class ViewDetail extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ViewDetail.fromJson(super.json);

  /// 空のオブジェクトから作る
  ViewDetail() : super({});

  /// 対象の実寸の空間の公開 ID
  String? get space => readField('space', asString);
  set space(String? value) => writeField('space', value);

  /// モデル座標から用紙の mm への [a,b,c,d,e,f]。点を (ax+by+c,dx+ey+f) に写す
  List<core.double>? get matrix => readField('matrix', listOf(asDouble));
  set matrix(List<core.double>? value) => writeField('matrix', value);

  /// モデルの X 軸・Y 軸の 1 単位に対応する用紙の長さ [SX,SY]（mm）。回転・せん断・反転でも各軸の正の長さを返す
  List<core.double>? get scale => readField('scale', listOf(asDouble));
  set scale(List<core.double>? value) => writeField('scale', value);

  /// モデル単位を mm に直した、各軸の縮尺の比 [SX,SY]。1:100 なら 0.01。単位不明なら持たない
  List<core.double>? get ratio => readField('ratio', listOf(asDouble));
  set ratio(List<core.double>? value) => writeField('ratio', value);

  /// 用紙の X 軸から見たモデルの X 軸の向き。度、反時計回り、-180〜180
  core.double? get rotation => readField('rotation', asDouble);
  set rotation(core.double? value) => writeField('rotation', value);

  /// 変換が向きを反転する
  core.bool? get mirrored => readField('mirrored', asBool);
  set mirrored(core.bool? value) => writeField('mirrored', value);

  /// 用紙の mm での切り抜きの頂点。元の順を保ち、空なら切り抜かない
  List<Point2>? get clip => readField('clip', listOf(objectOf(Point2.fromJson)));
  set clip(List<Point2>? value) => writeField('clip', value);

  /// 同じ頂点を逆変換したモデル座標。元の順を保つ
  List<Point2>? get modelClip => readField('model_clip', listOf(objectOf(Point2.fromJson)));
  set modelClip(List<Point2>? value) => writeField('model_clip', value);

  /// model_clip を囲む範囲。切り抜きが空なら持たない。モデルの単位
  Extent? get extent => readField('extent', objectOf(Extent.fromJson));
  set extent(Extent? value) => writeField('extent', value);

  /// ビューの出所
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 点を囲む、軸に沿った長方形
class Extent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Extent.fromJson(super.json);

  /// 空のオブジェクトから作る
  Extent() : super({});

  /// X の最小
  core.double? get minX => readField('min_x', asDouble);
  set minX(core.double? value) => writeField('min_x', value);

  /// Y の最小
  core.double? get minY => readField('min_y', asDouble);
  set minY(core.double? value) => writeField('min_y', value);

  /// X の最大
  core.double? get maxX => readField('max_x', asDouble);
  set maxX(core.double? value) => writeField('max_x', value);

  /// Y の最大
  core.double? get maxY => readField('max_y', asDouble);
  set maxY(core.double? value) => writeField('max_y', value);
}

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）
class EntitiesRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntitiesRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  EntitiesRequest() : super({'op': 'entities'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// すべての条件で絞った後の、返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0
  core.int? get offset => readField('offset', asInt);
  set offset(core.int? value) => writeField('offset', value);

  /// 返す要素の数の上限。書かなければ 1000
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);

  /// 返すフィールドの名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118）
  List<String>? get fields => readField('fields', listOf(asString));
  set fields(List<String>? value) => writeField('fields', value);

  /// [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118）
  List<core.double>? get region => readField('region', listOf(asDouble));
  set region(List<core.double>? value) => writeField('region', value);

  /// 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。マイナーバージョン 31 から（決定 0177）
  String? get place => readField('place', asString);
  set place(String? value) => writeField('place', value);

  /// 要素の公開 ID の一覧。いずれかに一致する要素だけを返す。空なら要素を返さず、ない ID は無視する。重複しても要素を増やさない。ほかの条件とも合わせる。マイナーバージョン 80 から
  List<String>? get ids => readField('ids', listOf(asString));
  set ids(List<String>? value) => writeField('ids', value);

  /// tables.layers の公開 ID の一覧。名前ではない。いずれかのレイヤの要素だけを返す。空なら要素を返さず、ない ID は無視する。ほかの条件とも合わせる。マイナーバージョン 80 から
  List<String>? get layers => readField('layers', listOf(asString));
  set layers(List<String>? value) => writeField('layers', value);

  /// 要素の種類の一覧。いずれかの種類だけを返す。空なら要素を返さず、重複しても要素を増やさない。ほかの条件とも合わせる。マイナーバージョン 80 から
  List<String>? get kinds => readField('kinds', listOf(asString));
  set kinds(List<String>? value) => writeField('kinds', value);

  /// 出所の条件。指定したすべての項目を満たす要素だけを返す。ほかの条件とも合わせる。マイナーバージョン 80 から
  EntitySourceFilter? get source => readField('source', objectOf(EntitySourceFilter.fromJson));
  set source(EntitySourceFilter? value) => writeField('source', value);
}

/// 元の要素の出所による条件。空のオブジェクトなら制限しない。出所を持たない要素は指定した条件を満たさない
class EntitySourceFilter extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntitySourceFilter.fromJson(super.json);

  /// 空のオブジェクトから作る
  EntitySourceFilter() : super({});

  /// L1 のレコード番号と完全に一致する
  core.int? get record => readField('record', asInt);
  set record(core.int? value) => writeField('record', value);

  /// L1 のバイト範囲との重なり。start を含み end を含まない、空でない範囲。境界が接するだけなら一致しない
  ByteRange? get bytes => readField('bytes', objectOf(ByteRange.fromJson));
  set bytes(ByteRange? value) => writeField('bytes', value);

  /// AutoCAD のハンドル。接頭辞なし、1〜16 桁の 16 進の文字列。大文字と小文字を区別せず、数値で比べる。instance とは同時に指定できない
  String? get handle => readField('handle', asString);
  set handle(String? value) => writeField('handle', value);

  /// SXF のインスタンス番号。1〜20 桁の符号なし 64 bit 整数の 10 進文字列。数値で比べる。handle とは同時に指定できない
  String? get instance => readField('instance', asString);
  set instance(String? value) => writeField('instance', value);
}

/// 要素の一覧の一部。移さなかった図形と値は、診断で返す
class EntitiesResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntitiesResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  EntitiesResult() : super({});

  /// offset から limit 個までの要素
  List<Entity>? get items => readField('items', listOf(objectOf(Entity.fromJson)));
  set items(List<Entity>? value) => writeField('items', value);

  /// 全体の要素の数
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 続きの最初の要素の位置。続きがなければ持たない
  core.int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(core.int? value) => writeField('next_offset', value);
}

/// 図面モデルの要素
class Entity extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Entity.fromJson(super.json);

  /// 空のオブジェクトから作る
  Entity() : super({});

  /// 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つキー（ハンドル）があればキーで作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どのバージョンでも同じ ID を返す（決定 0117・0156）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 形の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// レイヤの元の名前。翻訳しない。図面モデルにレイヤがなければ持たない
  String? get layer => readField('layer', asString);
  set layer(String? value) => writeField('layer', value);

  /// 元の形式のレコード
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);

  /// 形の定義から求めた範囲。文字の字形・線の太さ・記号の中身は含めない。囲む点がなければ持たない（決定 0118）
  Extent? get bbox => readField('bbox', objectOf(Extent.fromJson));
  set bbox(Extent? value) => writeField('bbox', value);

  /// 要素のレイヤ・スタイル・形のハッシュ。出所の位置は含めない。変更の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点かフィット点で決まるスプライン・寸法・引出線（LEADER）・点・引出線つきの注記・表（ACAD_TABLE）だけが持つ。apply の replace と add に渡せる形（決定 0139）。マイナーバージョン 10 から
  Geometry? get geometry => readField('geometry', objectOf(Geometry.fromJson));
  set geometry(Geometry? value) => writeField('geometry', value);

  /// 要素自身のスタイル。fields に style を指定したときだけ返す。マイナーバージョン 79 から
  EntityStyle? get style => readField('style', objectOf(EntityStyle.fromJson));
  set style(EntityStyle? value) => writeField('style', value);

  /// 要素に付く拡張の一覧。fields に extensions を指定したときだけ返し、拡張がなければ空の一覧。マイナーバージョン 79 から
  List<EntityExtension>? get extensions => readField('extensions', listOf(objectOf(EntityExtension.fromJson)));
  set extensions(List<EntityExtension>? value) => writeField('extensions', value);
}

/// entities で返す要素のフィールド（決定 0118）
///
/// 列挙の値。知らない値も受け取る
abstract final class EntityField {
  static const id = 'id';
  static const kind = 'kind';
  static const layer = 'layer';
  static const source = 'source';
  static const bbox = 'bbox';
  static const hash = 'hash';
  static const geometry = 'geometry';
  static const style = 'style';
  static const extensions = 'extensions';
}

/// 要素自身が持つスタイル。継承は解かず、色表の RGB も補わない。色・線種・線幅の形は restyle と同じ。透過性は読むだけで、restyle の指定には含めない
class EntityStyle extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityStyle.fromJson(super.json);

  /// 空のオブジェクトから作る
  EntityStyle() : super({});

  /// 色の符号・図面内 RGB、または継承元
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 継承元か、tables が返す線種の公開 ID
  LineTypeChange? get lineType => readField('line_type', objectOf(LineTypeChange.fromJson));
  set lineType(LineTypeChange? value) => writeField('line_type', value);

  /// 要素自身の線種の倍率。文書や配置の倍率を掛けない
  core.double? get lineTypeScale => readField('line_type_scale', asDouble);
  set lineTypeScale(core.double? value) => writeField('line_type_scale', value);

  /// 線幅の指定。元の形式から読めていなければ持たない
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);

  /// 透過性の指定
  EntityTransparency? get transparency => readField('transparency', objectOf(EntityTransparency.fromJson));
  set transparency(EntityTransparency? value) => writeField('transparency', value);
}

/// 透過性。inherit または alpha の一方を返す
class EntityTransparency extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityTransparency.fromJson(super.json);

  /// 空のオブジェクトから作る
  EntityTransparency() : super({});

  /// 継承元
  String? get inherit => readField('inherit', asString);
  set inherit(String? value) => writeField('inherit', value);

  /// 不透明度。0 は透明、255 は不透明
  core.int? get alpha => readField('alpha', asInt);
  set alpha(core.int? value) => writeField('alpha', value);
}

/// 要素に付く形式固有の情報。元の順序を保ち、失効した値も current を偽にして返す。書き戻す操作ではない
class EntityExtension extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityExtension.fromJson(super.json);

  /// 空のオブジェクトから作る
  EntityExtension() : super({});

  /// 値の型を定める名前空間
  String? get namespace => readField('namespace', asString);
  set namespace(String? value) => writeField('namespace', value);

  /// 元の名前。翻訳しない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 元の形式のバージョン。不明なら持たない
  String? get version => readField('version', asString);
  set version(String? value) => writeField('version', value);

  /// 名前空間の値の型のバージョン
  core.int? get schema => readField('schema', asInt);
  set schema(core.int? value) => writeField('schema', value);

  /// 拡張が依存する元の要素のハッシュ。要素に依存するハッシュがなければ持たない
  String? get basis => readField('basis', asString);
  set basis(String? value) => writeField('basis', value);

  /// basis が現在の要素のハッシュと一致する
  core.bool? get current => readField('current', asBool);
  set current(core.bool? value) => writeField('current', value);

  /// 元の順序と型を保つフィールド
  List<EntityExtensionField>? get fields => readField('fields', listOf(objectOf(EntityExtensionField.fromJson)));
  set fields(List<EntityExtensionField>? value) => writeField('fields', value);

  /// 拡張の出所
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 拡張のフィールド。同じキーの重複と並びを保つ
class EntityExtensionField extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityExtensionField.fromJson(super.json);

  /// 空のオブジェクトから作る
  EntityExtensionField() : super({});

  /// 元のキー
  String? get key => readField('key', asString);
  set key(String? value) => writeField('key', value);

  /// 元の形式の値の番号。番号がなければ持たない
  core.int? get code => readField('code', asInt);
  set code(core.int? value) => writeField('code', value);

  /// 型付きの値
  EntityExtensionValue? get value => readField('value', objectOf(EntityExtensionValue.fromJson));
  set value(EntityExtensionValue? value) => writeField('value', value);
}

/// 拡張の値。整数・ハンドル・バイト列は文字列で正確に返し、実数と点は有限値だけを返す
class EntityExtensionValue extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityExtensionValue.fromJson(super.json);

  /// 空のオブジェクトから作る
  EntityExtensionValue() : super({});

  /// kind が text の形なら、そのビュー。違えば null
  EntityExtensionText? get asEntityExtensionText => taggedView('kind', 'text', EntityExtensionText.fromJson);

  /// kind が integer の形なら、そのビュー。違えば null
  EntityExtensionInteger? get asEntityExtensionInteger => taggedView('kind', 'integer', EntityExtensionInteger.fromJson);

  /// kind が real の形なら、そのビュー。違えば null
  EntityExtensionReal? get asEntityExtensionReal => taggedView('kind', 'real', EntityExtensionReal.fromJson);

  /// kind が point の形なら、そのビュー。違えば null
  EntityExtensionPoint? get asEntityExtensionPoint => taggedView('kind', 'point', EntityExtensionPoint.fromJson);

  /// kind が handle の形なら、そのビュー。違えば null
  EntityExtensionHandle? get asEntityExtensionHandle => taggedView('kind', 'handle', EntityExtensionHandle.fromJson);

  /// kind が bytes の形なら、そのビュー。違えば null
  EntityExtensionBytes? get asEntityExtensionBytes => taggedView('kind', 'bytes', EntityExtensionBytes.fromJson);
}

/// 元の文字列
class EntityExtensionText extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityExtensionText.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  EntityExtensionText() : super({'kind': 'text'});

  /// 値の型
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 文字列
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);
}

/// 符号付き 64 bit 整数
class EntityExtensionInteger extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityExtensionInteger.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  EntityExtensionInteger() : super({'kind': 'integer'});

  /// 値の型
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 10 進の文字列。負数以外には符号を付けず、先頭の 0 を付けない
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);
}

/// 有限の実数
class EntityExtensionReal extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityExtensionReal.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  EntityExtensionReal() : super({'kind': 'real'});

  /// 値の型
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 実数
  core.double? get value => readField('value', asDouble);
  set value(core.double? value) => writeField('value', value);
}

/// 3 次元の点やベクトル。単位と空間は名前空間が定める
class EntityExtensionPoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityExtensionPoint.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  EntityExtensionPoint() : super({'kind': 'point'});

  /// 値の型
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// [X, Y, Z] の有限値
  List<core.double>? get value => readField('value', listOf(asDouble));
  set value(List<core.double>? value) => writeField('value', value);
}

/// 元の形式のオブジェクトを指す符号なし 64 bit の値
class EntityExtensionHandle extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityExtensionHandle.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  EntityExtensionHandle() : super({'kind': 'handle'});

  /// 値の型
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 大文字の 16 進の文字列。接頭辞と先頭の 0 を付けない
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);
}

/// 元のバイト列
class EntityExtensionBytes extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntityExtensionBytes.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  EntityExtensionBytes() : super({'kind': 'bytes'});

  /// 値の型
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 1 バイトを小文字の 16 進 2 桁にした文字列。空も保つ
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);
}

/// 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる
class Geometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Geometry.fromJson(super.json);

  /// 空のオブジェクトから作る
  Geometry() : super({});

  /// shape が line の形なら、そのビュー。違えば null
  LineGeometry? get asLineGeometry => taggedView('shape', 'line', LineGeometry.fromJson);

  /// shape が circle の形なら、そのビュー。違えば null
  CircleGeometry? get asCircleGeometry => taggedView('shape', 'circle', CircleGeometry.fromJson);

  /// shape が arc の形なら、そのビュー。違えば null
  ArcGeometry? get asArcGeometry => taggedView('shape', 'arc', ArcGeometry.fromJson);

  /// shape が ellipse の形なら、そのビュー。違えば null
  EllipseGeometry? get asEllipseGeometry => taggedView('shape', 'ellipse', EllipseGeometry.fromJson);

  /// shape が polyline の形なら、そのビュー。違えば null
  PolylineGeometry? get asPolylineGeometry => taggedView('shape', 'polyline', PolylineGeometry.fromJson);

  /// shape が text の形なら、そのビュー。違えば null
  TextGeometry? get asTextGeometry => taggedView('shape', 'text', TextGeometry.fromJson);

  /// shape が formatted_text の形なら、そのビュー。違えば null
  FormattedTextGeometry? get asFormattedTextGeometry => taggedView('shape', 'formatted_text', FormattedTextGeometry.fromJson);

  /// shape が symbol の形なら、そのビュー。違えば null
  SymbolGeometry? get asSymbolGeometry => taggedView('shape', 'symbol', SymbolGeometry.fromJson);

  /// shape が spline の形なら、そのビュー。違えば null
  SplineGeometry? get asSplineGeometry => taggedView('shape', 'spline', SplineGeometry.fromJson);

  /// shape が dimension の形なら、そのビュー。違えば null
  DimensionGeometry? get asDimensionGeometry => taggedView('shape', 'dimension', DimensionGeometry.fromJson);

  /// shape が leader の形なら、そのビュー。違えば null
  LeaderGeometry? get asLeaderGeometry => taggedView('shape', 'leader', LeaderGeometry.fromJson);

  /// shape が point の形なら、そのビュー。違えば null
  PointGeometry? get asPointGeometry => taggedView('shape', 'point', PointGeometry.fromJson);

  /// shape が callout の形なら、そのビュー。違えば null
  CalloutGeometry? get asCalloutGeometry => taggedView('shape', 'callout', CalloutGeometry.fromJson);

  /// shape が table の形なら、そのビュー。違えば null
  TableGeometry? get asTableGeometry => taggedView('shape', 'table', TableGeometry.fromJson);

  /// shape が hatch の形なら、そのビュー。違えば null
  HatchGeometry? get asHatchGeometry => taggedView('shape', 'hatch', HatchGeometry.fromJson);
}

/// 非連動のハッチング。閉じた折れ線（膨らみ付き）と円を境界にできる。塗りは単色か、線の定義を明示した模様。参照・勾配・自動検出した境界は扱わない。DXF と DWG に追加できる。置き換え・変換はまだ扱わない。entities は用紙上の模様を返さない。用紙への模様の追加は、原点が 0 で 1 単位が 1 mm のときに限る。マイナーバージョン 75 から
class HatchGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  HatchGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  HatchGeometry() : super({'shape': 'hatch'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 境界の輪。入力の順。1 つ以上
  List<HatchLoop>? get loops => readField('loops', listOf(objectOf(HatchLoop.fromJson)));
  set loops(List<HatchLoop>? value) => writeField('loops', value);

  /// 輪の中の輪の埋め方
  String? get islands => readField('islands', asString);
  set islands(String? value) => writeField('islands', value);

  /// 中を埋めるもの
  String? get fill => readField('fill', asString);
  set fill(String? value) => writeField('fill', value);

  /// fill が pattern のときだけ持つ模様の定義
  HatchPattern? get pattern => readField('pattern', objectOf(HatchPattern.fromJson));
  set pattern(HatchPattern? value) => writeField('pattern', value);

  /// 境界を探した点 [[X, Y], ...]。書かなければ空
  List<List<core.double>>? get seeds => readField('seeds', listOf(listOf(asDouble)));
  set seeds(List<List<core.double>>? value) => writeField('seeds', value);
}

/// ハッチングの境界の輪。参照を持たない
class HatchLoop extends Message {
  /// 読んだ JSON のオブジェクトを包む
  HatchLoop.fromJson(super.json);

  /// 空のオブジェクトから作る
  HatchLoop() : super({});

  /// 閉じた折れ線か円
  HatchBoundary? get boundary => readField('boundary', objectOf(HatchBoundary.fromJson));
  set boundary(HatchBoundary? value) => writeField('boundary', value);

  /// 外側の輪
  core.bool? get externalField => readField('external', asBool);
  set externalField(core.bool? value) => writeField('external', value);

  /// いちばん外の輪
  core.bool? get outermost => readField('outermost', asBool);
  set outermost(core.bool? value) => writeField('outermost', value);
}

/// 輪の形。折れ線は closed が真で頂点が 2 つ以上。最後の点から最初の点へ結ぶ
class HatchBoundary extends Message {
  /// 読んだ JSON のオブジェクトを包む
  HatchBoundary.fromJson(super.json);

  /// 空のオブジェクトから作る
  HatchBoundary() : super({});

  /// shape が polyline の形なら、そのビュー。違えば null
  PolylineGeometry? get asPolylineGeometry => taggedView('shape', 'polyline', PolylineGeometry.fromJson);

  /// shape が circle の形なら、そのビュー。違えば null
  CircleGeometry? get asCircleGeometry => taggedView('shape', 'circle', CircleGeometry.fromJson);
}

/// nested は外から奇数番目の内側、outer はいちばん外と次の輪の間、ignore は内側の輪を無視して埋める
///
/// 列挙の値。知らない値も受け取る
abstract final class HatchIslands {
  static const nested = 'nested';
  static const outer = 'outer';
  static const ignore = 'ignore';
}

/// solid は要素の色で塗り、pattern は線の組で埋める
///
/// 列挙の値。知らない値も受け取る
abstract final class HatchFill {
  static const solid = 'solid';
  static const pattern = 'pattern';
}

/// 模様の名前と線の定義。外部の定義ファイルを読まず、lines の値を使う
class HatchPattern extends Message {
  /// 読んだ JSON のオブジェクトを包む
  HatchPattern.fromJson(super.json);

  /// 空のオブジェクトから作る
  HatchPattern() : super({});

  /// 元の名前。空と制御文字を含むものは受けない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 定義の出どころ
  String? get origin => readField('origin', asString);
  set origin(String? value) => writeField('origin', value);

  /// 定義に加えた回転（度）
  core.double? get angle => readField('angle', asDouble);
  set angle(core.double? value) => writeField('angle', value);

  /// 定義に掛けた尺度か、利用者が決めた線の間隔。0 より大きい
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);

  /// 線の組を 90 度回して重ねる
  core.bool? get double => readField('double', asBool);
  set double(core.bool? value) => writeField('double', value);

  /// angle と scale を当てた後の線の組。1 つ以上
  List<HatchPatternLine>? get lines => readField('lines', listOf(objectOf(HatchPatternLine.fromJson)));
  set lines(List<HatchPatternLine>? value) => writeField('lines', value);
}

/// user_defined は利用者が決めた平行線、predefined は形式が名前で決めた定義、custom は利用者の定義
///
/// 列挙の値。知らない値も受け取る
abstract final class HatchPatternOrigin {
  static const userDefined = 'user_defined';
  static const predefined = 'predefined';
  static const custom = 'custom';
}

/// 1 組の平行線。基点・ずれ・線の長さは要素の座標と単位に従う
class HatchPatternLine extends Message {
  /// 読んだ JSON のオブジェクトを包む
  HatchPatternLine.fromJson(super.json);

  /// 空のオブジェクトから作る
  HatchPatternLine() : super({});

  /// 線の向き（度）
  core.double? get angle => readField('angle', asDouble);
  set angle(core.double? value) => writeField('angle', value);

  /// 1 本の線が通る点 [X, Y]
  List<core.double>? get baseField => readField('base', listOf(asDouble));
  set baseField(List<core.double>? value) => writeField('base', value);

  /// 隣の線までのずれ [X, Y]。両方 0 にはしない
  List<core.double>? get offset => readField('offset', listOf(asDouble));
  set offset(List<core.double>? value) => writeField('offset', value);

  /// 正は線、負は間、0 は点。空なら実線
  List<core.double>? get dashes => readField('dashes', listOf(asDouble));
  set dashes(List<core.double>? value) => writeField('dashes', value);
}

/// 線分
class LineGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LineGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  LineGeometry() : super({'shape': 'line'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 始点 [X, Y]
  List<core.double>? get start => readField('start', listOf(asDouble));
  set start(List<core.double>? value) => writeField('start', value);

  /// 終点 [X, Y]
  List<core.double>? get end => readField('end', listOf(asDouble));
  set end(List<core.double>? value) => writeField('end', value);
}

/// 円
class CircleGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CircleGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CircleGeometry() : super({'shape': 'circle'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 中心 [X, Y]
  List<core.double>? get center => readField('center', listOf(asDouble));
  set center(List<core.double>? value) => writeField('center', value);

  /// 半径。0 より大きい
  core.double? get radius => readField('radius', asDouble);
  set radius(core.double? value) => writeField('radius', value);
}

/// 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる
class ArcGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ArcGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ArcGeometry() : super({'shape': 'arc'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 中心 [X, Y]
  List<core.double>? get center => readField('center', listOf(asDouble));
  set center(List<core.double>? value) => writeField('center', value);

  /// 半径。0 より大きい
  core.double? get radius => readField('radius', asDouble);
  set radius(core.double? value) => writeField('radius', value);

  /// 始まりの角度（度）
  core.double? get startAngle => readField('start_angle', asDouble);
  set startAngle(core.double? value) => writeField('start_angle', value);

  /// 終わりの角度（度）
  core.double? get endAngle => readField('end_angle', asDouble);
  set endAngle(core.double? value) => writeField('end_angle', value);
}

/// 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。マイナーバージョン 11 から
class EllipseGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EllipseGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  EllipseGeometry() : super({'shape': 'ellipse'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 中心 [X, Y]
  List<core.double>? get center => readField('center', listOf(asDouble));
  set center(List<core.double>? value) => writeField('center', value);

  /// 長軸の端の点 [X, Y]。中心と違う点
  List<core.double>? get major => readField('major', listOf(asDouble));
  set major(List<core.double>? value) => writeField('major', value);

  /// 短軸の長軸に対する比。0 より大きく 1 以下
  core.double? get ratio => readField('ratio', asDouble);
  set ratio(core.double? value) => writeField('ratio', value);

  /// 始まりの角度（度）。楕円を円に引き延ばしたときの角度
  core.double? get startAngle => readField('start_angle', asDouble);
  set startAngle(core.double? value) => writeField('start_angle', value);

  /// 終わりの角度（度）。楕円を円に引き延ばしたときの角度
  core.double? get endAngle => readField('end_angle', asDouble);
  set endAngle(core.double? value) => writeField('end_angle', value);
}

/// 頂点を結んだ折れ線。幅は持たない
class PolylineGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PolylineGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  PolylineGeometry() : super({'shape': 'polyline'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 頂点の [X, Y] の並び。2 つ以上
  List<List<core.double>>? get points => readField('points', listOf(listOf(asDouble)));
  set points(List<List<core.double>>? value) => writeField('points', value);

  /// 頂点ごとの、次の頂点への弧の膨らみ（中心角の 4 分の 1 の正接。正は反時計回り）。points と同じ数。書かなければどれも 0
  List<core.double>? get bulges => readField('bulges', listOf(asDouble));
  set bulges(List<core.double>? value) => writeField('bulges', value);

  /// 最後の頂点から最初の頂点へ結ぶ
  core.bool? get closed => readField('closed', asBool);
  set closed(core.bool? value) => writeField('closed', value);
}

/// 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせを元の値のままにし、書かない幅の倍率・斜体・反転も元の値を受け継ぐ。足すときは、2 点目がなければ書かない位置合わせを左の基準線にし、幅の倍率 1・斜体なし・反転なしにする
class TextGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TextGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  TextGeometry() : super({'shape': 'text'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 内容。1 行
  String? get content => readField('content', asString);
  set content(String? value) => writeField('content', value);

  /// 基点 [X, Y]。2 点の間に合わせる文字では 1 点目
  List<core.double>? get position => readField('position', listOf(asDouble));
  set position(List<core.double>? value) => writeField('position', value);

  /// 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。マイナーバージョン 17 から
  List<core.double>? get end => readField('end', listOf(asDouble));
  set end(List<core.double>? value) => writeField('end', value);

  /// 2 点の間への合わせ方。2 点目（end）と一緒に書く。マイナーバージョン 17 から
  String? get fit => readField('fit', asString);
  set fit(String? value) => writeField('fit', value);

  /// 1 点に置く文字の水平の位置合わせ。left のときは entities が省き、足すときも省けば left。2 点の間に合わせる文字には書けない。置き換えるときは元と同じ値だけを受ける。マイナーバージョン 69 から
  String? get horizontal => readField('horizontal', asString);
  set horizontal(String? value) => writeField('horizontal', value);

  /// 1 点に置く文字の垂直の位置合わせ。baseline のときは entities が省き、足すときも省けば baseline。2 点の間に合わせる文字には書けない。置き換えるときは元と同じ値だけを受ける。マイナーバージョン 69 から
  String? get vertical => readField('vertical', asString);
  set vertical(String? value) => writeField('vertical', value);

  /// 高さ。0 より大きい
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 高さ（height）を用紙の mm で表すか。実寸の空間の文字が用紙の長さで高さを持つとき（JWW）、entities は真を返す。足す・置き換えるときに真を書けば、高さを用紙の mm として渡す。偽のときは entities が省き、省けば高さを点と同じ単位とする。マイナーバージョン 93 から
  core.bool? get heightOnPaper => readField('height_on_paper', asBool);
  set heightOnPaper(core.bool? value) => writeField('height_on_paper', value);

  /// 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない
  core.double? get rotation => readField('rotation', asDouble);
  set rotation(core.double? value) => writeField('rotation', value);

  /// 幅の倍率。0 より大きい。1 のときは entities が省き、足すときも省けば 1、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
  core.double? get widthFactor => readField('width_factor', asDouble);
  set widthFactor(core.double? value) => writeField('width_factor', value);

  /// 斜体の角度（度）。0 のときは entities が省き、足すときも省けば 0、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
  core.double? get oblique => readField('oblique', asDouble);
  set oblique(core.double? value) => writeField('oblique', value);

  /// 左右を裏返すか。偽のときは entities が省き、足すときも省けば偽、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
  core.bool? get backward => readField('backward', asBool);
  set backward(core.bool? value) => writeField('backward', value);

  /// 上下を反転するか。偽のときは entities が省き、足すときも省けば偽、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
  core.bool? get upsideDown => readField('upside_down', asBool);
  set upsideDown(core.bool? value) => writeField('upside_down', value);

  /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、省けば元の名前を保ち、DXF・DWG は定義のある別の文字スタイルも受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。マイナーバージョン 12 から
  String? get style => readField('style', asString);
  set style(String? value) => writeField('style', value);

  /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。マイナーバージョン 23 から
  String? get styleId => readField('style_id', asString);
  set styleId(String? value) => writeField('style_id', value);
}

/// 1 点に置く文字の水平の位置合わせ。middle は水平と垂直の両方を外形の中央に合わせる（DXF の 72 = 4）
///
/// 列挙の値。知らない値も受け取る
abstract final class TextHorizontal {
  static const left = 'left';
  static const center = 'center';
  static const right = 'right';
  static const middle = 'middle';
}

/// 1 点に置く文字の垂直の位置合わせ
///
/// 列挙の値。知らない値も受け取る
abstract final class TextVertical {
  static const baseline = 'baseline';
  static const bottom = 'bottom';
  static const middle = 'middle';
  static const top = 'top';
}

/// 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
///
/// 列挙の値。知らない値も受け取る
abstract final class TextFit {
  static const aligned = 'aligned';
  static const fit = 'fit';
}

/// 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味を元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。マイナーバージョン 13 から
class FormattedTextGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  FormattedTextGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  FormattedTextGeometry() : super({'shape': 'formatted_text'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 内容。書式の符号（\P の改行など）を含めた、元の形式の内容のまま
  String? get content => readField('content', asString);
  set content(String? value) => writeField('content', value);

  /// 基点 [X, Y]
  List<core.double>? get position => readField('position', listOf(asDouble));
  set position(List<core.double>? value) => writeField('position', value);

  /// 高さ。0 より大きい
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 回転（度）
  core.double? get rotation => readField('rotation', asDouble);
  set rotation(core.double? value) => writeField('rotation', value);

  /// 基点に合わせる枠の位置
  String? get attachment => readField('attachment', asString);
  set attachment(String? value) => writeField('attachment', value);

  /// 折り返す枠の幅。0 より大きい。書かなければ折り返さない
  core.double? get width => readField('width', asDouble);
  set width(core.double? value) => writeField('width', value);

  /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、省けば元の名前を保ち、DXF・DWG は定義のある別の文字スタイルも受ける。style_id と一緒に書くなら、同じ文字スタイルを指す
  String? get style => readField('style', asString);
  set style(String? value) => writeField('style', value);

  /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。マイナーバージョン 23 から
  String? get styleId => readField('style_id', asString);
  set styleId(String? value) => writeField('style_id', value);
}

/// スプライン（NURBS）。制御点で決まるもの（マイナーバージョン 15 から）と、フィット点で決まるもの（マイナーバージョン 18 から）がある。フィット点で決まるものは、ノットと制御点を空にし、フィット点（fit）と、あれば両端の接線を持つ。元の図形に制御点やノットもあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものとフィット点で決まるものを入れ替えない
class SplineGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SplineGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  SplineGeometry() : super({'shape': 'spline'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 次数。1 以上
  core.int? get degree => readField('degree', asInt);
  set degree(core.int? value) => writeField('degree', value);

  /// ノットの並び。周期的でなければ、制御点の数と次数と 1 の和の数。フィット点で決まるものは空
  List<core.double>? get knots => readField('knots', listOf(asDouble));
  set knots(List<core.double>? value) => writeField('knots', value);

  /// 制御点の [X, Y] の並び。次数より多い。フィット点で決まるものは空
  List<List<core.double>>? get control => readField('control', listOf(listOf(asDouble)));
  set control(List<List<core.double>>? value) => writeField('control', value);

  /// 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない
  List<core.double>? get weights => readField('weights', listOf(asDouble));
  set weights(List<core.double>? value) => writeField('weights', value);

  /// フィット点の [X, Y] の並び。2 つ以上。書けば、ノットと制御点は空で、重みは書かない。マイナーバージョン 18 から
  List<List<core.double>>? get fit => readField('fit', listOf(listOf(asDouble)));
  set fit(List<List<core.double>>? value) => writeField('fit', value);

  /// フィット点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。マイナーバージョン 18 から
  List<core.double>? get startTangent => readField('start_tangent', listOf(asDouble));
  set startTangent(List<core.double>? value) => writeField('start_tangent', value);

  /// フィット点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。マイナーバージョン 18 から
  List<core.double>? get endTangent => readField('end_tangent', listOf(asDouble));
  set endTangent(List<core.double>? value) => writeField('end_tangent', value);

  /// 閉じている
  core.bool? get closed => readField('closed', asBool);
  set closed(core.bool? value) => writeField('closed', value);

  /// 周期的
  core.bool? get periodic => readField('periodic', asBool);
  set periodic(core.bool? value) => writeField('periodic', value);
}

/// 寸法。種類（kind）ごとに、その種類の定義点のフィールドを持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。置き換え（DXF と DWG の DIMENSION・対応する ARC_DIMENSION）では定義点と文字を書き直し、測定値を測り直して表示の部品を作り直す（マイナーバージョン 74 から。それまでは置き換えも change.edit.unsupported だった）。measurement は省くか元と同じ値を書く。通常寸法 7 種は対応する DXF・DWG に、弧長寸法は R2004 以降の対応形式に追加できる。DWG の弧長寸法には ARC_DIMENSION クラスが必要。マイナーバージョン 19 から
class DimensionGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DimensionGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  DimensionGeometry() : super({'shape': 'dimension'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 寸法の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 1 本目の補助線の点か、測る角度の 1 本目の線の点か、円弧の始めの点 [X, Y]。linear・aligned・angular・arc_length が持つ
  List<core.double>? get first => readField('first', listOf(asDouble));
  set first(List<core.double>? value) => writeField('first', value);

  /// 2 本目の補助線の点か、測る角度の 2 本目の線の点か、円弧の終わりの点 [X, Y]。linear・aligned・angular・arc_length が持つ
  List<core.double>? get second => readField('second', listOf(asDouble));
  set second(List<core.double>? value) => writeField('second', value);

  /// 寸法線が通る点 [X, Y]。linear・aligned が持つ
  List<core.double>? get line => readField('line', listOf(asDouble));
  set line(List<core.double>? value) => writeField('line', value);

  /// 測る向き（度）。linear が持つ
  core.double? get rotation => readField('rotation', asDouble);
  set rotation(core.double? value) => writeField('rotation', value);

  /// 補助線の傾き（度）。傾けなければ 0。linear・aligned が持つ
  core.double? get oblique => readField('oblique', asDouble);
  set oblique(core.double? value) => writeField('oblique', value);

  /// 角度の頂点 [X, Y]。angular が持つ
  List<core.double>? get vertex => readField('vertex', listOf(asDouble));
  set vertex(List<core.double>? value) => writeField('vertex', value);

  /// 1 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
  List<List<core.double>>? get firstLine => readField('first_line', listOf(listOf(asDouble)));
  set firstLine(List<List<core.double>>? value) => writeField('first_line', value);

  /// 2 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
  List<List<core.double>>? get secondLine => readField('second_line', listOf(listOf(asDouble)));
  set secondLine(List<List<core.double>>? value) => writeField('second_line', value);

  /// 寸法の弧が通る点 [X, Y]。angular・angular_lines・arc_length が持つ
  List<core.double>? get arc => readField('arc', listOf(asDouble));
  set arc(List<core.double>? value) => writeField('arc', value);

  /// 中心 [X, Y]。arc_length・radius が持つ
  List<core.double>? get center => readField('center', listOf(asDouble));
  set center(List<core.double>? value) => writeField('center', value);

  /// 円の上の点 [X, Y]。radius・diameter が持つ
  List<core.double>? get chord => readField('chord', listOf(asDouble));
  set chord(List<core.double>? value) => writeField('chord', value);

  /// chord と向かい合う円の上の点 [X, Y]。diameter が持つ
  List<core.double>? get opposite => readField('opposite', listOf(asDouble));
  set opposite(List<core.double>? value) => writeField('opposite', value);

  /// 引出線の長さ。radius・diameter が持つ
  core.double? get leader => readField('leader', asDouble);
  set leader(core.double? value) => writeField('leader', value);

  /// 原点 [X, Y]。ordinate が持つ
  List<core.double>? get origin => readField('origin', listOf(asDouble));
  set origin(List<core.double>? value) => writeField('origin', value);

  /// 測る点 [X, Y]。ordinate が持つ
  List<core.double>? get feature => readField('feature', listOf(asDouble));
  set feature(List<core.double>? value) => writeField('feature', value);

  /// 引出線の終点 [X, Y]。ordinate が持つ
  List<core.double>? get leaderEnd => readField('leader_end', listOf(asDouble));
  set leaderEnd(List<core.double>? value) => writeField('leader_end', value);

  /// 測る座標の軸。ordinate が持つ
  String? get axis => readField('axis', asString);
  set axis(String? value) => writeField('axis', value);

  /// 寸法値の文字の書き方
  String? get label => readField('label', asString);
  set label(String? value) => writeField('label', value);

  /// 利用者が書いた寸法値の文字の元の内容。label が override のときだけ持つ。測定値に置き換える符号（DXF の <>）と書式の符号もそのまま持つ
  String? get content => readField('content', asString);
  set content(String? value) => writeField('content', value);

  /// 元の形式が保存した測定値。単位と倍率は measurement_key のフィールドに従う
  core.double? get measurement => readField('measurement', asDouble);
  set measurement(core.double? value) => writeField('measurement', value);

  /// measurement の形式とフィールドを指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ
  String? get measurementKey => readField('measurement_key', asString);
  set measurementKey(String? value) => writeField('measurement_key', value);

  /// 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない
  List<core.double>? get textPosition => readField('text_position', listOf(asDouble));
  set textPosition(List<core.double>? value) => writeField('text_position', value);

  /// 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない
  core.double? get textRotation => readField('text_rotation', asDouble);
  set textRotation(core.double? value) => writeField('text_rotation', value);

  /// 寸法スタイルの元の名前
  String? get style => readField('style', asString);
  set style(String? value) => writeField('style', value);

  /// 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける。マイナーバージョン 23 から
  String? get styleId => readField('style_id', asString);
  set styleId(String? value) => writeField('style_id', value);
}

/// 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（マイナーバージョン 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印のフラグを必ず持つので、arrow を書かない引出線は書けない（マイナーバージョン 36 から。それまでは R2004 までの置き換えだけ）。マイナーバージョン 31 から
class LeaderGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LeaderGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  LeaderGeometry() : super({'shape': 'leader'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 経路の頂点 [[X, Y], ...]。矢印の側から並べる。2 つ以上
  List<List<core.double>>? get path => readField('path', listOf(listOf(asDouble)));
  set path(List<List<core.double>>? value) => writeField('path', value);

  /// 頂点を通る曲線で結ぶか。偽なら折れ線
  core.bool? get spline => readField('spline', asBool);
  set spline(core.bool? value) => writeField('spline', value);

  /// 始点に矢印を付けるか。書かなければ寸法スタイルに従う
  core.bool? get arrow => readField('arrow', asBool);
  set arrow(core.bool? value) => writeField('arrow', value);

  /// 注記の要素の ID。同じ空間か用紙の要素で、entities が返したもの。注記がなければ書かない。置き換えるときは元と同じ ID だけを受ける。足すときは、同じ空間か用紙の書式つきの文字（MTEXT）を指す
  String? get annotation => readField('annotation', asString);
  set annotation(String? value) => writeField('annotation', value);

  /// 寸法スタイルの元の名前。足すときは図面にある寸法スタイルの名前を指す。置き換えるときは元と同じ名前だけを受ける
  String? get style => readField('style', asString);
  set style(String? value) => writeField('style', value);

  /// 寸法スタイルの ID。tables が返したもの。entities は、名前で引いた寸法スタイルの ID を返す。足す・置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける
  String? get styleId => readField('style_id', asString);
  set styleId(String? value) => writeField('style_id', value);
}

/// 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかのフィールドは元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。マイナーバージョン 36 から
class CalloutGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CalloutGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CalloutGeometry() : super({'shape': 'callout'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 枝。入力の順
  List<CalloutBranch>? get branches => readField('branches', listOf(objectOf(CalloutBranch.fromJson)));
  set branches(List<CalloutBranch>? value) => writeField('branches', value);

  /// 線の引き方
  String? get path => readField('path', asString);
  set path(String? value) => writeField('path', value);

  /// 線を注記に、水平の線（ランディング）でつなぐか
  core.bool? get landing => readField('landing', asBool);
  set landing(core.bool? value) => writeField('landing', value);

  /// 端部記号の大きさ
  core.double? get arrowSize => readField('arrow_size', asDouble);
  set arrowSize(core.double? value) => writeField('arrow_size', value);

  /// 大きさに掛ける倍率
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);

  /// 注記の種類
  String? get content => readField('content', asString);
  set content(String? value) => writeField('content', value);

  /// 注記の書式つきの文字の元の内容。content が text のときだけ持つ
  String? get text => readField('text', asString);
  set text(String? value) => writeField('text', value);

  /// 注記の記号のブロックの名前。content が symbol のときだけ持つ
  String? get symbol => readField('symbol', asString);
  set symbol(String? value) => writeField('symbol', value);
}

/// 引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる
class CalloutBranch extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CalloutBranch.fromJson(super.json);

  /// 空のオブジェクトから作る
  CalloutBranch() : super({});

  /// 線ごとの頂点 [[[X, Y], ...], ...]。矢印の側から並べ、集まる点は含めない。線ごとに 1 つ以上
  List<List<List<core.double>>>? get lines => readField('lines', listOf(listOf(listOf(asDouble))));
  set lines(List<List<List<core.double>>>? value) => writeField('lines', value);

  /// 線が集まる点 [X, Y]。元の形式が持たなければ書かない
  List<core.double>? get end => readField('end', listOf(asDouble));
  set end(List<core.double>? value) => writeField('end', value);

  /// 集まる点から注記への折れ曲がり。なければ書かない
  CalloutDogleg? get dogleg => readField('dogleg', objectOf(CalloutDogleg.fromJson));
  set dogleg(CalloutDogleg? value) => writeField('dogleg', value);
}

/// 枝の折れ曲がり
class CalloutDogleg extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CalloutDogleg.fromJson(super.json);

  /// 空のオブジェクトから作る
  CalloutDogleg() : super({});

  /// 向き [X, Y]。長さは意味を持たない
  List<core.double>? get direction => readField('direction', listOf(asDouble));
  set direction(List<core.double>? value) => writeField('direction', value);

  /// 長さ
  core.double? get length => readField('length', asDouble);
  set length(core.double? value) => writeField('length', value);
}

/// 引出線つきの注記の線の引き方。straight は折れ線、spline は頂点を通る曲線、hidden は線を描かない
///
/// 列挙の値。知らない値も受け取る
abstract final class CalloutPath {
  static const straight = 'straight';
  static const spline = 'spline';
  static const hidden = 'hidden';
}

/// 引出線つきの注記の中身の種類。none は注記を持たず、text は書式つきの文字、symbol は記号（バルーンなどの囲み）
///
/// 列挙の値。知らない値も受け取る
abstract final class CalloutContent {
  static const none = 'none';
  static const text = 'text';
  static const symbol = 'symbol';
}

/// 表（DXF・DWG の ACAD_TABLE、決定 0113）。挿入点・行の向き・行の高さ・列の幅と、行ごとのセルの並びを持つ。表のスタイル・セルの書式・型のある値・表示の部品（ブロック）は形の値に含めない。置き換え（DXF の ACAD_TABLE）では、行と列の数・セルの結合・文字でないセル（記号）を変えず、挿入点・行の向き・行の高さ・列の幅・文字と空のセルの内容と回転を書き直し、表示の部品を作り直す。R2004 以降の DXF で Standard スタイルを使って追加できる。R2004 DWG では ACAD_TABLE クラスと読み取れる Standard 表スタイルのある図面に、未結合・無回転の文字と空のセルの表を追加できる。既存の対応する DWG 表ではセル文字・行高さ・列幅・向きの変更と平行移動も扱う。マイナーバージョン 74 から
class TableGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TableGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  TableGeometry() : super({'shape': 'table'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 挿入点 [X, Y]
  List<core.double>? get position => readField('position', listOf(asDouble));
  set position(List<core.double>? value) => writeField('position', value);

  /// 行の向き [X, Y]。0 でない
  List<core.double>? get direction => readField('direction', listOf(asDouble));
  set direction(List<core.double>? value) => writeField('direction', value);

  /// 上の行からの行の高さ。どれも 0 より大きい。1 つ以上
  List<core.double>? get rowHeights => readField('row_heights', listOf(asDouble));
  set rowHeights(List<core.double>? value) => writeField('row_heights', value);

  /// 左の列からの列の幅。どれも 0 より大きい。1 つ以上
  List<core.double>? get columnWidths => readField('column_widths', listOf(asDouble));
  set columnWidths(List<core.double>? value) => writeField('column_widths', value);

  /// 上の行から、行ごとに左の列から並べたセル。数は行の数と列の数の積
  List<TableCell>? get cells => readField('cells', listOf(objectOf(TableCell.fromJson)));
  set cells(List<TableCell>? value) => writeField('cells', value);
}

/// 表の 1 つのセル
class TableCell extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TableCell.fromJson(super.json);

  /// 空のオブジェクトから作る
  TableCell() : super({});

  /// 中身の種類
  String? get content => readField('content', asString);
  set content(String? value) => writeField('content', value);

  /// 書式の符号を含む元の文字列。空でない。content が text のときだけ持つ
  String? get text => readField('text', asString);
  set text(String? value) => writeField('text', value);

  /// 記号のブロックの名前。content が symbol のときだけ持つ
  String? get symbol => readField('symbol', asString);
  set symbol(String? value) => writeField('symbol', value);

  /// 結合して広げる列と行の数 [列, 行]。結合しなければ [1, 1]（省けば [1, 1]）
  List<core.int>? get span => readField('span', listOf(asInt));
  set span(List<core.int>? value) => writeField('span', value);

  /// ほかのセルに結合されて覆われるか。偽のときは省ける
  core.bool? get covered => readField('covered', asBool);
  set covered(core.bool? value) => writeField('covered', value);

  /// 文字か記号の回転。0 のときは省ける
  core.double? get rotation => readField('rotation', asDouble);
  set rotation(core.double? value) => writeField('rotation', value);
}

/// 表のセルの中身の種類。empty は空、text は書式つきの文字、symbol は記号（ブロック）
///
/// 列挙の値。知らない値も受け取る
abstract final class TableCellContent {
  static const empty = 'empty';
  static const text = 'text';
  static const symbol = 'symbol';
}

/// 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義のマーカーの点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。マイナーバージョン 35 から
class PointGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  PointGeometry() : super({'shape': 'point'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 位置 [X, Y]
  List<core.double>? get position => readField('position', listOf(asDouble));
  set position(List<core.double>? value) => writeField('position', value);

  /// 回転（度）
  core.double? get rotation => readField('rotation', asDouble);
  set rotation(core.double? value) => writeField('rotation', value);

  /// 既定義のマーカー。書かなければ文書の点の表し方に従う
  PointMarker? get marker => readField('marker', objectOf(PointMarker.fromJson));
  set marker(PointMarker? value) => writeField('marker', value);

  /// 作図の補助の点（JWW の仮点）か。書かなければ偽
  core.bool? get construction => readField('construction', asBool);
  set construction(core.bool? value) => writeField('construction', value);
}

/// 既定義のマーカーの表のキーと尺度。マーカーの形は、表示する側がキーから引く
class PointMarker extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointMarker.fromJson(super.json);

  /// 空のオブジェクトから作る
  PointMarker() : super({});

  /// マーカーの表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点）
  String? get namespace => readField('namespace', asString);
  set namespace(String? value) => writeField('namespace', value);

  /// 名前空間の中の元のコード
  String? get code => readField('code', asString);
  set code(String? value) => writeField('code', value);

  /// 尺度
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);
}

/// 寸法の種類。linear は 2 点を測る向きに、aligned は 2 点を結ぶ向きに測った長さ。angular は頂点から 2 点へ向かう線の、angular_lines は 2 本の線のなす角度。arc_length は円弧の長さ。radius は半径、diameter は直径。ordinate は原点からの X か Y の座標
///
/// 列挙の値。知らない値も受け取る
abstract final class DimensionKind {
  static const linear = 'linear';
  static const aligned = 'aligned';
  static const angular = 'angular';
  static const angularLines = 'angular_lines';
  static const arcLength = 'arc_length';
  static const radius = 'radius';
  static const diameter = 'diameter';
  static const ordinate = 'ordinate';
}

/// 座標の軸
///
/// 列挙の値。知らない値も受け取る
abstract final class DimensionAxis {
  static const x = 'x';
  static const y = 'y';
}

/// 寸法値の文字の書き方。measurement は測定値を書き、override は利用者が書いた文字（content）を書き、hidden は書かない
///
/// 列挙の値。知らない値も受け取る
abstract final class DimensionLabel {
  static const measurement = 'measurement';
  static const override = 'override';
  static const hidden = 'hidden';
}

/// 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。マイナーバージョン 14 から
class SymbolGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SymbolGeometry.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  SymbolGeometry() : super({'shape': 'symbol'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 記号の定義（ブロック）の元の名前。足すときは図面にある定義の名前を指す
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 定義の基点を置く点 [X, Y]
  List<core.double>? get position => readField('position', listOf(asDouble));
  set position(List<core.double>? value) => writeField('position', value);

  /// X と Y の尺度 [X, Y]。0 でない。負なら反転する
  List<core.double>? get scale => readField('scale', listOf(asDouble));
  set scale(List<core.double>? value) => writeField('scale', value);

  /// 回転（度）
  core.double? get rotation => readField('rotation', asDouble);
  set rotation(core.double? value) => writeField('rotation', value);

  /// 格子に並べて置く列と行。書かなければ 1 つだけ置く
  SymbolGrid? get grid => readField('grid', objectOf(SymbolGrid.fromJson));
  set grid(SymbolGrid? value) => writeField('grid', value);

  /// 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。マイナーバージョン 16 から。置き換えで値を変えるのはマイナーバージョン 21 から
  List<SymbolAttribute>? get attributes => readField('attributes', listOf(objectOf(SymbolAttribute.fromJson)));
  set attributes(List<SymbolAttribute>? value) => writeField('attributes', value);
}

/// 記号の属性のタグと値
class SymbolAttribute extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SymbolAttribute.fromJson(super.json);

  /// 空のオブジェクトから作る
  SymbolAttribute() : super({});

  /// 元のタグ
  String? get tag => readField('tag', asString);
  set tag(String? value) => writeField('tag', value);

  /// 値。1 行
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);
}

/// 記号を格子に並べる列と行。間隔は、記号を回転する前の向きに測る。置き換えるときは、列と行の数を変えない
class SymbolGrid extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SymbolGrid.fromJson(super.json);

  /// 空のオブジェクトから作る
  SymbolGrid() : super({});

  /// 列の数。1 以上
  core.int? get columns => readField('columns', asInt);
  set columns(core.int? value) => writeField('columns', value);

  /// 行の数。1 以上。列と行がどちらも 1 の格子は持たない
  core.int? get rows => readField('rows', asInt);
  set rows(core.int? value) => writeField('rows', value);

  /// 列の間隔
  core.double? get columnSpacing => readField('column_spacing', asDouble);
  set columnSpacing(core.double? value) => writeField('column_spacing', value);

  /// 行の間隔
  core.double? get rowSpacing => readField('row_spacing', asDouble);
  set rowSpacing(core.double? value) => writeField('row_spacing', value);
}

/// 基点に合わせる枠の位置。entities は返し、足すときに書かなければ左上、置き換えるときに書かなければ元のまま
///
/// 列挙の値。知らない値も受け取る
abstract final class Attachment {
  static const topLeft = 'top_left';
  static const topCenter = 'top_center';
  static const topRight = 'top_right';
  static const middleLeft = 'middle_left';
  static const middleCenter = 'middle_center';
  static const middleRight = 'middle_right';
  static const bottomLeft = 'bottom_left';
  static const bottomCenter = 'bottom_center';
  static const bottomRight = 'bottom_right';
}

/// 要素の形の種類（決定 0022）。point（点、決定 0160）はマイナーバージョン 32 から。それまで点は symbol だった
///
/// 列挙の値。知らない値も受け取る
abstract final class EntityKind {
  static const line = 'line';
  static const circle = 'circle';
  static const curve = 'curve';
  static const polyline = 'polyline';
  static const text = 'text';
  static const formattedText = 'formatted_text';
  static const dimension = 'dimension';
  static const leader = 'leader';
  static const callout = 'callout';
  static const table = 'table';
  static const underlay = 'underlay';
  static const symbol = 'symbol';
  static const hatch = 'hatch';
  static const point = 'point';
}

/// 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。マイナーバージョン 7 から
class ApplyRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ApplyRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ApplyRequest() : super({'op': 'apply'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 読んだときの文書のハッシュ（drawing の hash）。書けば、今の文書と同じときだけ当てる
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 変更。並べた順に当てる
  List<Edit>? get edits => readField('edits', listOf(objectOf(Edit.fromJson)));
  set edits(List<Edit>? value) => writeField('edits', value);
}

/// 当てた変更の結果。書き戻したファイルは blobs の file で返す
class ApplyResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ApplyResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  ApplyResult() : super({});

  /// 変更ごとの結果。要求の edits と同じ順
  List<Applied>? get applied => readField('applied', listOf(objectOf(Applied.fromJson)));
  set applied(List<Applied>? value) => writeField('applied', value);

  /// 書き戻した文書のハッシュ。次の apply の前提条件に使う
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// true なら、変えていない要素の ID がずれた。キーを持たない形式（JWW）か、ハンドルを持たない DXF（R12 など）で、前の図形か表のレコードを除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。マイナーバージョン 30 から
  core.bool? get idsShifted => readField('ids_shifted', asBool);
  set idsShifted(core.bool? value) => writeField('ids_shifted', value);
}

/// 入力なしで空の文書を作り、開く。dxf は R2004 の DXF で、既定のレイヤとスタイル、空のモデル空間と Layout1 を持つ。ifc は IFC のひな形で、プロジェクト・長さとラジアンの単位・3 次元の Model の文脈と Body の下位の文脈を持つ（#395）。書いたファイルは blobs で返す。マイナーバージョン 83 から。ifc はマイナーバージョン 109 から
class CreateRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CreateRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CreateRequest() : super({'op': 'create'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 作る形式の名前。dxf か ifc
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 作る文書の名前。既存の文書と重ねない
  String? get to => readField('to', asString);
  set to(String? value) => writeField('to', value);

  /// 実寸の空間の長さの単位。省略すると millimeter。unknown は単位なしで、ifc では書けない
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// IFC を書く設定。format が ifc のときに必須で、ほかの形式では書けない。マイナーバージョン 109 から
  IfcOutput? get ifc => readField('ifc', objectOf(IfcOutput.fromJson));
  set ifc(IfcOutput? value) => writeField('ifc', value);
}

/// 作った空の文書。書いたファイルは blobs の file で返す
class CreateResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CreateResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  CreateResult() : super({});

  /// 作った文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 書いた形式の名前
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 書いたファイルのハッシュ。apply の前提条件に使う
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 書いた要素の数。空なので 0
  core.int? get elements => readField('elements', asInt);
  set elements(core.int? value) => writeField('elements', value);
}

/// 開いた文書から別の形式のファイルを作り、新しい文書として開く。図面からは DXF（対応する R2004 DWG の弧長寸法は共有オブジェクト表から、それ以外は図面モデルから。決定 0139・0206・0281）または SIMA（.sim）、土木モデル（J-LandXML・IFC4.3・SIMA）からは SIMA（.sim）、地理の領域モデルの文書（CityGML・基盤地図情報・拡張 DM・GeoJSON・SIMA・GeoTIFF）からは GeoJSON または SIMA（.sim）を書く。建物モデルの文書（IFC・ST-Bridge）からは、ifc で指定したバージョンの新しい IFC を書く（#395）。書けないデータは種類ごとの件数を診断で返す。書いたファイルは blobs で返す。マイナーバージョン 41 から。geojson はマイナーバージョン 71 から。sima はマイナーバージョン 72 から。ifc はマイナーバージョン 109 から
class ConvertRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConvertRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ConvertRequest() : super({'op': 'convert'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 書く元の文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 書く形式の名前（capabilities の formats の id）。dxf・sxf（P21）・jww・geojson・sima・ifc のいずれか。sxf・jww はマイナーバージョン 85 から
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 作る文書の名前
  String? get to => readField('to', asString);
  set to(String? value) => writeField('to', value);

  /// IFC を書く設定。format が ifc のときに必須で、ほかの形式では書けない。マイナーバージョン 109 から
  IfcOutput? get ifc => readField('ifc', objectOf(IfcOutput.fromJson));
  set ifc(IfcOutput? value) => writeField('ifc', value);
}

/// 書いた文書。書いたファイルは blobs の file で返す
class ConvertResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConvertResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConvertResult() : super({});

  /// 作った文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 書いた形式の名前
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 書いたファイルのハッシュ。apply の前提条件に使う
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 書いた要素の数。記号から離した属性の文字を含む
  core.int? get elements => readField('elements', asInt);
  set elements(core.int? value) => writeField('elements', value);
}

/// 開いた文書を変換したときの忠実度と欠落を、convert と同じ書き手で見積もる。文書もファイルの blob も登録しない。図面から DXF・SXF（P21）・JWW、建物モデルの文書から IFC への変換に対応する。マイナーバージョン 66 から（SXF・JWW は 85 から、IFC は 109 から）
class ConvertEstimateRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConvertEstimateRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ConvertEstimateRequest() : super({'op': 'convert_estimate'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 書く元の文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 書く形式の名前。dxf・sxf・jww・ifc
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 予定する文書の名前。convert と同じ条件で検査するが、名前を確保しない
  String? get to => readField('to', asString);
  set to(String? value) => writeField('to', value);

  /// IFC を書く設定。format が ifc のときに必須で、ほかの形式では書けない。マイナーバージョン 109 から
  IfcOutput? get ifc => readField('ifc', objectOf(IfcOutput.fromJson));
  set ifc(IfcOutput? value) => writeField('ifc', value);

  /// 書き手と読み取り段階のそれぞれで返す明細の最大件数。省略時は 100。総数と全件集計はこの上限によらない
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);
}

/// 新しい IFC を書く設定（#395）。コアは時計を読まないので、時刻は呼び出し元が渡す
class IfcOutput extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IfcOutput.fromJson(super.json);

  /// 空のオブジェクトから作る
  IfcOutput() : super({});

  /// 書くバージョン
  String? get version => readField('version', asString);
  set version(String? value) => writeField('version', value);

  /// ヘッダー（FILE_DESCRIPTION と FILE_NAME）の値。書かなければ空の値で書く
  IfcHeader? get header => readField('header', objectOf(IfcHeader.fromJson));
  set header(IfcHeader? value) => writeField('header', value);

  /// 所有履歴（IfcOwnerHistory）の値。ifc2x3 では必須。ifc4 からは書いたときだけ書く
  IfcOwner? get owner => readField('owner', objectOf(IfcOwner.fromJson));
  set owner(IfcOwner? value) => writeField('owner', value);

  /// GlobalId を導く名前空間。書かなければ、convert は元の IfcProject の GlobalId（なければ元の文書のハッシュ）、create は作る文書の名前を使う。元の文書のハッシュでない名前空間なら、元を一部直して書き直しても、直していない要素の GlobalId は変わらない
  String? get namespace => readField('namespace', asString);
  set namespace(String? value) => writeField('namespace', value);
}

/// IFC のバージョン。ifc2x3 は IFC2X3 TC1、ifc4 は IFC4 ADD2 TC1、ifc4x3 は IFC4X3 ADD2
///
/// 列挙の値。知らない値も受け取る
abstract final class IfcVersion {
  static const ifc2x3 = 'ifc2x3';
  static const ifc4 = 'ifc4';
  static const ifc4x3 = 'ifc4x3';
}

/// IFC のヘッダーの値。書かない値は空で書く
class IfcHeader extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IfcHeader.fromJson(super.json);

  /// 空のオブジェクトから作る
  IfcHeader() : super({});

  /// FILE_DESCRIPTION に ViewDefinition [<値>] として書く MVD の名前。書かなければ書かない
  String? get viewDefinition => readField('view_definition', asString);
  set viewDefinition(String? value) => writeField('view_definition', value);

  /// FILE_NAME の name
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// FILE_NAME の time_stamp。呼び出し元が書いた時刻
  String? get timeStamp => readField('time_stamp', asString);
  set timeStamp(String? value) => writeField('time_stamp', value);

  /// FILE_NAME の author
  List<String>? get author => readField('author', listOf(asString));
  set author(List<String>? value) => writeField('author', value);

  /// FILE_NAME の organization
  List<String>? get organization => readField('organization', listOf(asString));
  set organization(List<String>? value) => writeField('organization', value);

  /// FILE_NAME の preprocessor_version
  String? get preprocessorVersion => readField('preprocessor_version', asString);
  set preprocessorVersion(String? value) => writeField('preprocessor_version', value);

  /// FILE_NAME の originating_system
  String? get originatingSystem => readField('originating_system', asString);
  set originatingSystem(String? value) => writeField('originating_system', value);

  /// FILE_NAME の authorization
  String? get authorization => readField('authorization', asString);
  set authorization(String? value) => writeField('authorization', value);
}

/// 所有履歴の値
class IfcOwner extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IfcOwner.fromJson(super.json);

  /// 空のオブジェクトから作る
  IfcOwner() : super({});

  /// 所有する人の IfcPerson の FamilyName
  String? get person => readField('person', asString);
  set person(String? value) => writeField('person', value);

  /// 所有する組織の IfcOrganization の Name
  String? get organization => readField('organization', asString);
  set organization(String? value) => writeField('organization', value);

  /// 作ったソフトの開発元の IfcOrganization の Name
  String? get developer => readField('developer', asString);
  set developer(String? value) => writeField('developer', value);

  /// 作ったソフトの名前（IfcApplication の ApplicationFullName）
  String? get application => readField('application', asString);
  set application(String? value) => writeField('application', value);

  /// 作ったソフトのバージョン（IfcApplication の Version）
  String? get version => readField('version', asString);
  set version(String? value) => writeField('version', value);

  /// 作ったソフトの識別子（IfcApplication の ApplicationIdentifier）
  String? get identifier => readField('identifier', asString);
  set identifier(String? value) => writeField('identifier', value);

  /// 作った日時。1970-01-01T00:00:00Z からの秒（IfcTimeStamp）
  core.int? get created => readField('created', asInt);
  set created(core.int? value) => writeField('created', value);
}

/// 予定する変換の結果。元の文書・ID を保ち、ファイルの blob を返さない
class ConvertEstimateResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConvertEstimateResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConvertEstimateResult() : super({});

  /// 予定する出力と忠実度の全件集計・上限付き明細
  ConversionEstimate? get estimate => readField('estimate', objectOf(ConversionEstimate.fromJson));
  set estimate(ConversionEstimate? value) => writeField('estimate', value);
}

/// 実変換と同じ処理で準備した出力。doc と hash は未登録の予定値。元の入力・設定・予算が同じなら convert の値と一致する
class ConversionEstimate extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConversionEstimate.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConversionEstimate() : super({});

  /// 予定する文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 予定する形式の名前
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 予定する出力バイト列のハッシュ。未登録なので apply の前提条件には使えない
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 予定する出力の要素数。記号から離す属性の文字を含む
  core.int? get elements => readField('elements', asInt);
  set elements(core.int? value) => writeField('elements', value);

  /// 処理対象の出現数。同じ出所を持つ別の入力は別々に数え、診断の数では増やさない
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 種類と忠実度ごとの全件集計
  List<ConversionCount>? get counts => readField('counts', listOf(objectOf(ConversionCount.fromJson)));
  set counts(List<ConversionCount>? value) => writeField('counts', value);

  /// 処理順の明細。limit 件まで返す
  List<ConversionDetail>? get details => readField('details', listOf(objectOf(ConversionDetail.fromJson)));
  set details(List<ConversionDetail>? value) => writeField('details', value);

  /// 明細の上限で返さなかった対象がある
  core.bool? get truncated => readField('truncated', asBool);
  set truncated(core.bool? value) => writeField('truncated', value);

  /// 入力の表の読み取りと lift 中の、集約前の診断発生記録。書き手の項目の集計には含めない。マイナーバージョン 67 から
  ConversionInputDiagnostics? get inputDiagnostics => readField('input_diagnostics', objectOf(ConversionInputDiagnostics.fromJson));
  set inputDiagnostics(ConversionInputDiagnostics? value) => writeField('input_diagnostics', value);
}

/// 読み取り段階の診断発生数。失った要素数ではなく、同じ要素の複数の診断もそれぞれ数える。忠実度を持たない診断も含む。open 時や書き手の診断は含まない
class ConversionInputDiagnostics extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConversionInputDiagnostics.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConversionInputDiagnostics() : super({});

  /// 診断の発生件数の合計。max_places と limit によらない
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 診断の code とカタログの group_by の値による全件集計。場所は持たず、値は最も早い入力位置の報告から取る
  List<Diagnostic>? get counts => readField('counts', listOf(objectOf(Diagnostic.fromJson)));
  set counts(List<Diagnostic>? value) => writeField('counts', value);

  /// 集約前の報告を入力位置順に limit 件まで返す。同じ位置は報告順。診断の場所は max_places で制限しない
  List<ConversionInputDiagnostic>? get details => readField('details', listOf(objectOf(ConversionInputDiagnostic.fromJson)));
  set details(List<ConversionInputDiagnostic>? value) => writeField('details', value);

  /// 明細の上限で返さなかった報告がある
  core.bool? get truncated => readField('truncated', asBool);
  set truncated(core.bool? value) => writeField('truncated', value);
}

/// 集約前の診断と、その場所と同じ順序の出所。種類や失った値は診断の code と params からたどる
class ConversionInputDiagnostic extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConversionInputDiagnostic.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConversionInputDiagnostic() : super({});

  /// 集約前の診断。元の値と場所を保つ
  Diagnostic? get diagnostic => readField('diagnostic', objectOf(Diagnostic.fromJson));
  set diagnostic(Diagnostic? value) => writeField('diagnostic', value);

  /// diagnostic.at と同じ順序のレコード・バイト範囲・ハンドル・インスタンス番号。ID は diagnostic.at にある
  List<Source>? get sources => readField('sources', listOf(objectOf(Source.fromJson)));
  set sources(List<Source>? value) => writeField('sources', value);
}

/// 処理対象の種類と忠実度ごとの集計
class ConversionCount extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConversionCount.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConversionCount() : super({});

  /// 処理対象の種類のキー
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// その対象を移した結果の忠実度
  String? get fidelity => readField('fidelity', asString);
  set fidelity(String? value) => writeField('fidelity', value);

  /// 入力の出現数。1 つの入力が複数の出力になっても 1 件
  core.int? get count => readField('count', asInt);
  set count(core.int? value) => writeField('count', value);

  /// この対象から書いた図形の数。表や参照だけを移した場合は 0
  core.int? get outputElements => readField('output_elements', asInt);
  set outputElements(core.int? value) => writeField('output_elements', value);
}

/// 処理対象の出現 1 つ。複数の欠落の診断が出ても 1 件として数える
class ConversionDetail extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConversionDetail.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConversionDetail() : super({});

  /// 処理対象の種類のキー
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// その対象を移した結果の忠実度
  String? get fidelity => readField('fidelity', asString);
  set fidelity(String? value) => writeField('fidelity', value);

  /// 値や参照を変えた理由のキー。text_height_meaning は文字高の意味、relocated_shape は形の値、style_default はスタイルの既定値、detached_attribute は属性の文字への分離、baked_view はビューの焼き込み、reference_dropped は参照の欠落
  List<String>? get reasons => readField('reasons', listOf(asString));
  set reasons(List<String>? value) => writeField('reasons', value);

  /// 元の要素の ID。持つ対象だけで返す
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 元の対象のレコード・バイト範囲・キー
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);

  /// この対象から書いた図形の数。表や参照だけを移した場合は 0
  core.int? get outputElements => readField('output_elements', asInt);
  set outputElements(core.int? value) => writeField('output_elements', value);
}

/// 忠実度。`native` は元の値・参照を保持、`equivalent` は別の表現で同じ意味、`approximated` は近似、`drawn` は見た目だけ、`dropped` は欠落
///
/// 列挙の値。知らない値も受け取る
abstract final class ConversionFidelity {
  static const native = 'native';
  static const equivalent = 'equivalent';
  static const approximated = 'approximated';
  static const drawn = 'drawn';
  static const dropped = 'dropped';
}

/// 当てた変更 1 つの結果
class Applied extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Applied.fromJson(super.json);

  /// 空のオブジェクトから作る
  Applied() : super({});

  /// 変えた要素（レイヤの変更では、そのレイヤ）の、書き戻した後の ID。除いたときと、ID を決められないときは持たない
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 変えた要素（レイヤ）の、書き戻した後のハッシュ。id があるときに持つ
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);
}

/// 要素 1 つへの変更。kind の値で形が決まる
class Edit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Edit.fromJson(super.json);

  /// 空のオブジェクトから作る
  Edit() : super({});

  /// kind が restyle の形なら、そのビュー。違えば null
  RestyleEdit? get asRestyleEdit => taggedView('kind', 'restyle', RestyleEdit.fromJson);

  /// kind が transform の形なら、そのビュー。違えば null
  TransformEdit? get asTransformEdit => taggedView('kind', 'transform', TransformEdit.fromJson);

  /// kind が copy の形なら、そのビュー。違えば null
  CopyEdit? get asCopyEdit => taggedView('kind', 'copy', CopyEdit.fromJson);

  /// kind が remove の形なら、そのビュー。違えば null
  RemoveEdit? get asRemoveEdit => taggedView('kind', 'remove', RemoveEdit.fromJson);

  /// kind が replace の形なら、そのビュー。違えば null
  ReplaceEdit? get asReplaceEdit => taggedView('kind', 'replace', ReplaceEdit.fromJson);

  /// kind が add の形なら、そのビュー。違えば null
  AddEdit? get asAddEdit => taggedView('kind', 'add', AddEdit.fromJson);

  /// kind が layer の形なら、そのビュー。違えば null
  LayerEdit? get asLayerEdit => taggedView('kind', 'layer', LayerEdit.fromJson);

  /// kind が add_layer の形なら、そのビュー。違えば null
  AddLayerEdit? get asAddLayerEdit => taggedView('kind', 'add_layer', AddLayerEdit.fromJson);

  /// kind が add_symbol の形なら、そのビュー。違えば null
  AddSymbolEdit? get asAddSymbolEdit => taggedView('kind', 'add_symbol', AddSymbolEdit.fromJson);

  /// kind が remove_layer の形なら、そのビュー。違えば null
  RemoveLayerEdit? get asRemoveLayerEdit => taggedView('kind', 'remove_layer', RemoveLayerEdit.fromJson);

  /// kind が text_style の形なら、そのビュー。違えば null
  TextStyleEdit? get asTextStyleEdit => taggedView('kind', 'text_style', TextStyleEdit.fromJson);

  /// kind が add_text_style の形なら、そのビュー。違えば null
  AddTextStyleEdit? get asAddTextStyleEdit => taggedView('kind', 'add_text_style', AddTextStyleEdit.fromJson);

  /// kind が remove_text_style の形なら、そのビュー。違えば null
  RemoveTextStyleEdit? get asRemoveTextStyleEdit => taggedView('kind', 'remove_text_style', RemoveTextStyleEdit.fromJson);

  /// kind が line_type の形なら、そのビュー。違えば null
  LineTypeEdit? get asLineTypeEdit => taggedView('kind', 'line_type', LineTypeEdit.fromJson);

  /// kind が add_line_type の形なら、そのビュー。違えば null
  AddLineTypeEdit? get asAddLineTypeEdit => taggedView('kind', 'add_line_type', AddLineTypeEdit.fromJson);

  /// kind が remove_line_type の形なら、そのビュー。違えば null
  RemoveLineTypeEdit? get asRemoveLineTypeEdit => taggedView('kind', 'remove_line_type', RemoveLineTypeEdit.fromJson);

  /// kind が dimension_style の形なら、そのビュー。違えば null
  DimensionStyleEdit? get asDimensionStyleEdit => taggedView('kind', 'dimension_style', DimensionStyleEdit.fromJson);

  /// kind が add_dimension_style の形なら、そのビュー。違えば null
  AddDimensionStyleEdit? get asAddDimensionStyleEdit => taggedView('kind', 'add_dimension_style', AddDimensionStyleEdit.fromJson);

  /// kind が remove_dimension_style の形なら、そのビュー。違えば null
  RemoveDimensionStyleEdit? get asRemoveDimensionStyleEdit => taggedView('kind', 'remove_dimension_style', RemoveDimensionStyleEdit.fromJson);

  /// kind が point_style の形なら、そのビュー。違えば null
  PointStyleEdit? get asPointStyleEdit => taggedView('kind', 'point_style', PointStyleEdit.fromJson);
}

/// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない。RGB の色は、DXF と DWG の R2004 から、線幅は R2000 から書ける（DWG の今ある図形のレイヤ・線種・線幅・RGB の色は、決定 0293 から変えられる）
class RestyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RestyleEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  RestyleEdit() : super({'kind': 'restyle'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 変える要素の ID。entities が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。entities が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない
  String? get layer => readField('layer', asString);
  set layer(String? value) => writeField('layer', value);

  /// レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。マイナーバージョン 8 から
  String? get layerId => readField('layer_id', asString);
  set layerId(String? value) => writeField('layer_id', value);

  /// 色
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 線種
  LineTypeChange? get lineType => readField('line_type', objectOf(LineTypeChange.fromJson));
  set lineType(LineTypeChange? value) => writeField('line_type', value);

  /// 要素の線種の倍率
  core.double? get lineTypeScale => readField('line_type_scale', asDouble);
  set lineTypeScale(core.double? value) => writeField('line_type_scale', value);

  /// 線幅
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);
}

/// 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
class TransformEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TransformEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  TransformEdit() : super({'kind': 'transform'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 動かす要素の ID。entities が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。entities が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// [a, b, c, d, e, f]。要素の空間か用紙の点 (x, y) を (a x + b y + c, d x + e y + f) に写す
  List<core.double>? get matrix => readField('matrix', listOf(asDouble));
  set matrix(List<core.double>? value) => writeField('matrix', value);
}

/// 要素を複写し、写しをアフィン変換で動かす。元の要素は変えない。写しは、元のレイヤ・色・線種・線種の倍率・線幅・透過性と形式固有の値（AutoCAD の系統の XDATA、JWW のフラグ・文字種・字間、SXF の文字幅・字間など）を保ち、図形とともに動く XDATA と記号の属性も同じ変換で直す。DXF の寸法・表は新しい匿名ブロックを作る。place で配置先の空間・用紙を、from_doc で複写元の文書を指定でき（マイナーバージョン 110 から）、単位・画層・線種・文字スタイル・寸法スタイル・記号定義・APPID の対応と名前の衝突を確かめる。保てない値・参照・対応を持つ要素は change.copy.unsupported で全体を拒む。applied は写しの ID とハッシュを返す。マイナーバージョン 100 から
class CopyEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CopyEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CopyEdit() : super({'kind': 'copy'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 複写する要素の ID。entities が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。entities が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// [a, b, c, d, e, f]。写しの点 (x, y) を (a x + b y + c, d x + e y + f) に写す。元の位置に写すなら [1, 0, 0, 0, 1, 0]
  List<core.double>? get matrix => readField('matrix', listOf(asDouble));
  set matrix(List<core.double>? value) => writeField('matrix', value);

  /// 配置先の空間か用紙の ID。省けば元の要素と同じ場所 ID。マイナーバージョン 110 から
  String? get place => readField('place', asString);
  set place(String? value) => writeField('place', value);

  /// 複写元の文書の名前。open で作ったもの。省けば doc と同じ文書。マイナーバージョン 110 から
  String? get fromDoc => readField('from_doc', asString);
  set fromDoc(String? value) => writeField('from_doc', value);
}

/// 要素を除く
class RemoveEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  RemoveEdit() : super({'kind': 'remove'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 除く要素の ID。entities が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。entities が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);
}

/// 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤと線のスタイルは変えない。DXF・DWG の文字と書式つきの文字は、定義のある文字スタイルへ指し替えられる。マイナーバージョン 10 から
class ReplaceEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReplaceEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ReplaceEdit() : super({'kind': 'replace'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 置き換える要素の ID。entities が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。entities が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 新しい形。entities の geometry と同じ種類
  Geometry? get geometry => readField('geometry', objectOf(Geometry.fromJson));
  set geometry(Geometry? value) => writeField('geometry', value);
}

/// 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。JWW と SXF の図形は色・線種・線幅を自分で持ち、レイヤかブロックに従う値を持たないので、JWW は色と線種を書き（点は色だけ）、SXF は色・線種・線幅を書く（点マーカーは色だけ）。書かないか、レイヤかブロックに従う値なら change.style.unsupported で断る。JWW の書かないかレイヤに従う線幅は、ペンの印刷の線幅に従う。足せる形は、形の値のどの種類も。マイナーバージョン 10 から
class AddEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  AddEdit() : super({'kind': 'add'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。マイナーバージョン 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177）
  String? get place => readField('place', asString);
  set place(String? value) => writeField('place', value);

  /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
  String? get layer => readField('layer', asString);
  set layer(String? value) => writeField('layer', value);

  /// レイヤの ID。tables が返したもの
  String? get layerId => readField('layer_id', asString);
  set layerId(String? value) => writeField('layer_id', value);

  /// 色。SXF と JWW はレイヤに従う色を書けないので、書く。SXF は、sxf の番号か、利用者定義色と同じ RGB を受ける。JWW は、jww の番号（0〜9 と、バージョン 420 からの拡張色 100〜356）か、ペン 1〜9 と、バージョン 420 からの 100〜356 のどれかと同じ RGB を受ける。書かないか、受けられない色なら断る。マイナーバージョン 29 から
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 線種。マイナーバージョン 29 から
  LineTypeChange? get lineType => readField('line_type', objectOf(LineTypeChange.fromJson));
  set lineType(LineTypeChange? value) => writeField('line_type', value);

  /// 線幅。マイナーバージョン 29 から
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);

  /// 足す形
  Geometry? get geometry => readField('geometry', objectOf(Geometry.fromJson));
  set geometry(Geometry? value) => writeField('geometry', value);
}

/// レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG の真の色は R2004 から書ける（今あるレイヤの線種と真の色は、決定 0293 から変えられる）。マイナーバージョン 22 から
class LayerEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LayerEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  LayerEdit() : super({'kind': 'layer'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 変えるレイヤの ID。tables が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときのレイヤのハッシュ。tables が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 新しい名前。ほかのレイヤと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。0 と Defpoints の名前は変えられない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 表示する。凍結するなら偽
  core.bool? get visible => readField('visible', asBool);
  set visible(core.bool? value) => writeField('visible', value);

  /// 印刷する
  core.bool? get printable => readField('printable', asBool);
  set printable(core.bool? value) => writeField('printable', value);

  /// 編集できる（ロックしない）
  core.bool? get editable => readField('editable', asBool);
  set editable(core.bool? value) => writeField('editable', value);

  /// 凍結する
  core.bool? get frozen => readField('frozen', asBool);
  set frozen(core.bool? value) => writeField('frozen', value);

  /// 色。aci が要る。inherit は書けない
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 線種。name か id で指す。inherit は書けない
  LineTypeChange? get lineType => readField('line_type', objectOf(LineTypeChange.fromJson));
  set lineType(LineTypeChange? value) => writeField('line_type', value);

  /// 線幅。mm か default。inherit は書けない
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);
}

/// レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。マイナーバージョン 24 から
class RemoveLayerEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveLayerEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  RemoveLayerEdit() : super({'kind': 'remove_layer'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 除くレイヤの ID。tables が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときのレイヤのハッシュ。tables が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);
}

/// 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。マイナーバージョン 25 から
class TextStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TextStyleEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  TextStyleEdit() : super({'kind': 'text_style'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 変える文字スタイルの ID。tables が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの文字スタイルのハッシュ。tables が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 新しい名前。ほかの文字スタイルと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。Standard の名前は変えられない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
  Font? get font => readField('font', objectOf(Font.fromJson));
  set font(Font? value) => writeField('font', value);

  /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
  Font? get bigFont => readField('big_font', objectOf(Font.fromJson));
  set bigFont(Font? value) => writeField('big_font', value);

  /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 幅の倍率。正の数
  core.double? get width => readField('width', asDouble);
  set width(core.double? value) => writeField('width', value);

  /// 傾き（度）。-85 から 85 まで
  core.double? get oblique => readField('oblique', asDouble);
  set oblique(core.double? value) => writeField('oblique', value);

  /// 左右を裏返す
  core.bool? get backward => readField('backward', asBool);
  set backward(core.bool? value) => writeField('backward', value);

  /// 上下を反転する
  core.bool? get upsideDown => readField('upside_down', asBool);
  set upsideDown(core.bool? value) => writeField('upside_down', value);

  /// 縦書き
  core.bool? get vertical => readField('vertical', asBool);
  set vertical(core.bool? value) => writeField('vertical', value);
}

/// 文字スタイルを足す（決定 0163）。書かないフィールドは Standard と同じ。同じ apply の後ろの文字の追加・置き換えから、足した文字スタイルを名前で指せる。マイナーバージョン 25 から
class AddTextStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddTextStyleEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  AddTextStyleEdit() : super({'kind': 'add_text_style'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 名前。text_style の name と同じ決まり
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
  Font? get font => readField('font', objectOf(Font.fromJson));
  set font(Font? value) => writeField('font', value);

  /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
  Font? get bigFont => readField('big_font', objectOf(Font.fromJson));
  set bigFont(Font? value) => writeField('big_font', value);

  /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 幅の倍率。正の数
  core.double? get width => readField('width', asDouble);
  set width(core.double? value) => writeField('width', value);

  /// 傾き（度）。-85 から 85 まで
  core.double? get oblique => readField('oblique', asDouble);
  set oblique(core.double? value) => writeField('oblique', value);

  /// 左右を裏返す
  core.bool? get backward => readField('backward', asBool);
  set backward(core.bool? value) => writeField('backward', value);

  /// 上下を反転する
  core.bool? get upsideDown => readField('upside_down', asBool);
  set upsideDown(core.bool? value) => writeField('upside_down', value);

  /// 縦書き
  core.bool? get vertical => readField('vertical', asBool);
  set vertical(core.bool? value) => writeField('vertical', value);
}

/// 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 25 から
class RemoveTextStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveTextStyleEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  RemoveTextStyleEdit() : super({'kind': 'remove_text_style'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 除く文字スタイルの ID。tables が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの文字スタイルのハッシュ。tables が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);
}

/// 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。マルチラインスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。マイナーバージョン 26 から
class LineTypeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LineTypeEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  LineTypeEdit() : super({'kind': 'line_type'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 変える線種の ID。tables が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの線種のハッシュ。tables が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 新しい名前。ほかの線種と重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。ByLayer・ByBlock・Continuous と外部参照に依る線種の名前は変えられない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 説明
  String? get description => readField('description', asString);
  set description(String? value) => writeField('description', value);

  /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
  List<core.double>? get pattern => readField('pattern', listOf(asDouble));
  set pattern(List<core.double>? value) => writeField('pattern', value);
}

/// 線種を足す（決定 0164）。書かないフィールドは Continuous と同じ。同じ apply の後ろの要素とレイヤの変更から、足した線種を名前で指せる。マイナーバージョン 26 から
class AddLineTypeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddLineTypeEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  AddLineTypeEdit() : super({'kind': 'add_line_type'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 名前。line_type の name と同じ決まり
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 説明
  String? get description => readField('description', asString);
  set description(String? value) => writeField('description', value);

  /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
  List<core.double>? get pattern => readField('pattern', listOf(asDouble));
  set pattern(List<core.double>? value) => writeField('pattern', value);
}

/// 線種を除く（決定 0164）。要素・レイヤ・マルチラインスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。マイナーバージョン 26 から
class RemoveLineTypeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveLineTypeEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  RemoveLineTypeEdit() : super({'kind': 'remove_line_type'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 除く線種の ID。tables が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの線種のハッシュ。tables が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);
}

/// 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。マイナーバージョン 27 から
class DimensionStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DimensionStyleEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  DimensionStyleEdit() : super({'kind': 'dimension_style'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 変える寸法スタイルの ID。tables が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 新しい名前。ほかの寸法スタイルと重ならず（大文字と小文字を区別しない）、$ と < > / \ " : ; ? * | = ` を含まないもの。Standard・子の寸法スタイル（名前に $ を持つもの）・子を持つ寸法スタイル・外部参照に依る寸法スタイルの名前は変えられない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
  List<String>? get ends => readField('ends', listOf(asString));
  set ends(List<String>? value) => writeField('ends', value);

  /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
  String? get leader => readField('leader', asString);
  set leader(String? value) => writeField('leader', value);

  /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
  core.double? get size => readField('size', asDouble);
  set size(core.double? value) => writeField('size', value);

  /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
  core.double? get tick => readField('tick', asDouble);
  set tick(core.double? value) => writeField('tick', value);

  /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);
}

/// 寸法スタイルを足す（決定 0165）。書かないフィールドは Standard と同じ。同じ apply の後ろの要素の追加から、足した寸法スタイルを名前で指せる。マイナーバージョン 27 から
class AddDimensionStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddDimensionStyleEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  AddDimensionStyleEdit() : super({'kind': 'add_dimension_style'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 名前。dimension_style の name と同じ決まり
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
  List<String>? get ends => readField('ends', listOf(asString));
  set ends(List<String>? value) => writeField('ends', value);

  /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
  String? get leader => readField('leader', asString);
  set leader(String? value) => writeField('leader', value);

  /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
  core.double? get size => readField('size', asDouble);
  set size(core.double? value) => writeField('size', value);

  /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
  core.double? get tick => readField('tick', asDouble);
  set tick(core.double? value) => writeField('tick', value);

  /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);
}

/// 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 27 から
class RemoveDimensionStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveDimensionStyleEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  RemoveDimensionStyleEdit() : super({'kind': 'remove_dimension_style'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 除く寸法スタイルの ID。tables が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);
}

/// 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG はヘッダーの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、マイナーバージョン 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。マイナーバージョン 38 から
class PointStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointStyleEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  PointStyleEdit() : super({'kind': 'point_style'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 基本の形
  String? get mark => readField('mark', asString);
  set mark(String? value) => writeField('mark', value);

  /// 円で囲む。書かなければ偽
  core.bool? get circle => readField('circle', asBool);
  set circle(core.bool? value) => writeField('circle', value);

  /// 四角で囲む。書かなければ偽
  core.bool? get square => readField('square', asBool);
  set square(core.bool? value) => writeField('square', value);

  /// 大きさ。0 より大きい有限の数。意味は size_kind で決まる
  core.double? get size => readField('size', asDouble);
  set size(core.double? value) => writeField('size', value);

  /// 大きさの意味
  String? get sizeKind => readField('size_kind', asString);
  set sizeKind(String? value) => writeField('size_kind', value);
}

/// 文書の点の基本の形。dot は小さな点、none はなし、plus は十字、cross はバツ、tick は上向きの短い線
///
/// 列挙の値。知らない値も受け取る
abstract final class PointMark {
  static const dot = 'dot';
  static const none = 'none';
  static const plus = 'plus';
  static const cross = 'cross';
  static const tick = 'tick';
}

/// 文書の点の大きさの意味。length は点のある空間の長さ、screen は画面の高さに対する百分率
///
/// 列挙の値。知らない値も受け取る
abstract final class PointSizeKind {
  static const length = 'length';
  static const screen = 'screen';
}

/// レイヤを足す（決定 0162）。書かないフィールドは、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の後ろの要素の変更から、足したレイヤを名前で指せる。マイナーバージョン 22 から
class AddLayerEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddLayerEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  AddLayerEdit() : super({'kind': 'add_layer'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 名前。layer の name と同じ決まり
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 表示する。凍結するなら偽
  core.bool? get visible => readField('visible', asBool);
  set visible(core.bool? value) => writeField('visible', value);

  /// 印刷する
  core.bool? get printable => readField('printable', asBool);
  set printable(core.bool? value) => writeField('printable', value);

  /// 編集できる（ロックしない）
  core.bool? get editable => readField('editable', asBool);
  set editable(core.bool? value) => writeField('editable', value);

  /// 凍結する
  core.bool? get frozen => readField('frozen', asBool);
  set frozen(core.bool? value) => writeField('frozen', value);

  /// 色。aci が要る。inherit は書けない
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 線種。name か id で指す。inherit は書けない
  LineTypeChange? get lineType => readField('line_type', objectOf(LineTypeChange.fromJson));
  set lineType(LineTypeChange? value) => writeField('line_type', value);

  /// 線幅。mm か default。inherit は書けない
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);
}

/// 記号の定義（DXF・DWG のブロック）を足す（#684）。中の要素は、定義の空間に足す。足した定義は、同じ apply の後ろの add の記号から名前で指せる。属性の定義・名前のない定義・外部参照は足せない。結果は id と hash を持たない。足した定義は tables の symbols から名前で引く。マイナーバージョン 99 から
class AddSymbolEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddSymbolEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  AddSymbolEdit() : super({'kind': 'add_symbol'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 定義の名前。図面にない名前で、layer の name と同じ決まり（DXF・DWG のブロックの名前で書けない文字を含まない）。大文字と小文字は区別しない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 定義の基点 [X, Y]。記号を置く点に重ねる。書かなければ [0, 0]
  List<core.double>? get baseField => readField('base', listOf(asDouble));
  set baseField(List<core.double>? value) => writeField('base', value);

  /// 中の要素。点は定義の空間の座標で書く。並べた順に足す
  List<SymbolElement>? get elements => readField('elements', listOf(objectOf(SymbolElement.fromJson)));
  set elements(List<SymbolElement>? value) => writeField('elements', value);
}

/// 足す記号の定義の中の要素。フィールドは add と同じ意味で、place と kind を持たない。引出線の注記は指せない。マイナーバージョン 99 から
class SymbolElement extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SymbolElement.fromJson(super.json);

  /// 空のオブジェクトから作る
  SymbolElement() : super({});

  /// レイヤの元の名前。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
  String? get layer => readField('layer', asString);
  set layer(String? value) => writeField('layer', value);

  /// レイヤの ID。tables が返したもの
  String? get layerId => readField('layer_id', asString);
  set layerId(String? value) => writeField('layer_id', value);

  /// 色
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 線種
  LineTypeChange? get lineType => readField('line_type', objectOf(LineTypeChange.fromJson));
  set lineType(LineTypeChange? value) => writeField('line_type', value);

  /// 線幅
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);

  /// 足す形
  Geometry? get geometry => readField('geometry', objectOf(Geometry.fromJson));
  set geometry(Geometry? value) => writeField('geometry', value);
}

/// 値を継承する先。layer はレイヤから、block は要素を置いた記号の配置から
///
/// 列挙の値。知らない値も受け取る
abstract final class Inherit {
  static const layer = 'layer';
  static const block = 'block';
}

/// 新しい色。inherit か、aci・sxf・jww・rgb の少なくとも 1 つのどちらかを書く。aci・sxf・jww は 1 つだけ書く
class ColorChange extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ColorChange.fromJson(super.json);

  /// 空のオブジェクトから作る
  ColorChange() : super({});

  /// 継承する先
  String? get inherit => readField('inherit', asString);
  set inherit(String? value) => writeField('inherit', value);

  /// AutoCAD の色の番号（1〜255）
  core.int? get aci => readField('aci', asInt);
  set aci(core.int? value) => writeField('aci', value);

  /// SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。マイナーバージョン 29 から
  core.int? get sxf => readField('sxf', asInt);
  set sxf(core.int? value) => writeField('sxf', value);

  /// Jw_cad のペンの番号。0〜9 と、バージョン 420 からの SXF の拡張色 100〜356。マイナーバージョン 30 から
  core.int? get jww => readField('jww', asInt);
  set jww(core.int? value) => writeField('jww', value);

  /// [赤, 緑, 青]。それぞれ 0〜255
  List<core.int>? get rgb => readField('rgb', listOf(asInt));
  set rgb(List<core.int>? value) => writeField('rgb', value);
}

/// 新しい線種。inherit・name・id のどれか 1 つを書く
class LineTypeChange extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LineTypeChange.fromJson(super.json);

  /// 空のオブジェクトから作る
  LineTypeChange() : super({});

  /// 継承する先
  String? get inherit => readField('inherit', asString);
  set inherit(String? value) => writeField('inherit', value);

  /// 線種の元の名前。図面にある線種で、名前が 1 つに決まるもの
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。マイナーバージョン 8 から
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);
}

/// 新しい線幅。inherit・mm・default のどれか 1 つを書く
class LineWeightChange extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LineWeightChange.fromJson(super.json);

  /// 空のオブジェクトから作る
  LineWeightChange() : super({});

  /// 継承する先
  String? get inherit => readField('inherit', asString);
  set inherit(String? value) => writeField('inherit', value);

  /// 用紙の上の幅（mm）。0 以上
  core.double? get mm => readField('mm', asDouble);
  set mm(core.double? value) => writeField('mm', value);

  /// true なら、文書の既定の線幅
  core.bool? get defaultField => readField('default', asBool);
  set defaultField(core.bool? value) => writeField('default', value);
}

/// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。マイナーバージョン 8 から
class TablesRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TablesRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  TablesRequest() : super({'op': 'tables'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);
}

/// 表の項目。図面モデルの順
class TablesResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TablesResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  TablesResult() : super({});

  /// レイヤ
  List<LayerItem>? get layers => readField('layers', listOf(objectOf(LayerItem.fromJson)));
  set layers(List<LayerItem>? value) => writeField('layers', value);

  /// 線種。マイナーバージョン 26 から、定義を持つ
  List<LineTypeItem>? get lineTypes => readField('line_types', listOf(objectOf(LineTypeItem.fromJson)));
  set lineTypes(List<LineTypeItem>? value) => writeField('line_types', value);

  /// 寸法スタイル。マイナーバージョン 27 から、端部記号と大きさを持つ
  List<DimensionStyleItem>? get dimensionStyles => readField('dimension_styles', listOf(objectOf(DimensionStyleItem.fromJson)));
  set dimensionStyles(List<DimensionStyleItem>? value) => writeField('dimension_styles', value);

  /// 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義のフィールドを持たない項目として後ろに並ぶ。マイナーバージョン 20 から
  List<TextStyleItem>? get textStyles => readField('text_styles', listOf(objectOf(TextStyleItem.fromJson)));
  set textStyles(List<TextStyleItem>? value) => writeField('text_styles', value);

  /// 記号の定義。配置されていないものも含む。マイナーバージョン 81 から
  List<SymbolDefinitionItem>? get symbols => readField('symbols', listOf(objectOf(SymbolDefinitionItem.fromJson)));
  set symbols(List<SymbolDefinitionItem>? value) => writeField('symbols', value);
}

/// 記号の定義。要素と属性の配置や書式は元の定義に保ち、名前と属性の既定値を一覧にする
class SymbolDefinitionItem extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SymbolDefinitionItem.fromJson(super.json);

  /// 空のオブジェクトから作る
  SymbolDefinitionItem() : super({});

  /// 定義の空間の公開 ID。entities.place と同じ
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 定義を識別する名前空間
  String? get namespace => readField('namespace', asString);
  set namespace(String? value) => writeField('namespace', value);

  /// 元の名前。add の記号の name に使う
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 定義の基点。定義の空間の座標と単位。不明なら持たない
  Point2? get baseField => readField('base', objectOf(Point2.fromJson));
  set baseField(Point2? value) => writeField('base', value);

  /// 属性のタグ・既定値・入力の指定。元の順を保つ
  List<SymbolAttributeDefinition>? get attributes => readField('attributes', listOf(objectOf(SymbolAttributeDefinition.fromJson)));
  set attributes(List<SymbolAttributeDefinition>? value) => writeField('attributes', value);

  /// 寸法などが作った無名の定義である
  core.bool? get anonymous => readField('anonymous', asBool);
  set anonymous(core.bool? value) => writeField('anonymous', value);

  /// 外部参照の元のパス。参照先を開かない。外部参照でなければ持たない
  String? get externalField => readField('external', asString);
  set externalField(String? value) => writeField('external', value);

  /// 定義の出所
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 記号の属性の定義。タグと値は翻訳しない。配置・書式・スタイルは一覧には含めない
class SymbolAttributeDefinition extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SymbolAttributeDefinition.fromJson(super.json);

  /// 空のオブジェクトから作る
  SymbolAttributeDefinition() : super({});

  /// 元のタグ
  String? get tag => readField('tag', asString);
  set tag(String? value) => writeField('tag', value);

  /// 既定の 1 行の値
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);

  /// 複数行の値があるとき、その元の書式つきの文字列。読むための値
  String? get formattedValue => readField('formatted_value', asString);
  set formattedValue(String? value) => writeField('formatted_value', value);

  /// 値を尋ねる文
  String? get prompt => readField('prompt', asString);
  set prompt(String? value) => writeField('prompt', value);

  /// 値が決まっていて、配置ごとに変えない
  core.bool? get constant => readField('constant', asBool);
  set constant(core.bool? value) => writeField('constant', value);

  /// 入力した値を確かめる指定
  core.bool? get verify => readField('verify', asBool);
  set verify(core.bool? value) => writeField('verify', value);

  /// 尋ねずに既定の値を使う指定
  core.bool? get preset => readField('preset', asBool);
  set preset(core.bool? value) => writeField('preset', value);

  /// 属性を表示しない
  core.bool? get hidden => readField('hidden', asBool);
  set hidden(core.bool? value) => writeField('hidden', value);

  /// 属性の出所
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。マイナーバージョン 33 から
class SceneRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SceneRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  SceneRequest() : super({'op': 'scene'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 起点。実寸の空間・記号の定義の空間・用紙の ID（drawing と entities が返すもの）
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した空間の要素だけを描く。from が用紙のときだけ書ける
  core.int? get view => readField('view', asInt);
  set view(core.int? value) => writeField('view', value);

  /// [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
  List<core.double>? get region => readField('region', listOf(asDouble));
  set region(List<core.double>? value) => writeField('region', value);

  /// 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// true なら印刷する要素を、書かないか false なら表示する要素を描く
  core.bool? get print => readField('print', asBool);
  set print(core.bool? value) => writeField('print', value);

  /// 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決のフラグを立てる
  String? get palette => readField('palette', asString);
  set palette(String? value) => writeField('palette', value);

  /// 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形のセクションに形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。マイナーバージョン 37 から
  List<SceneFont>? get fonts => readField('fonts', listOf(objectOf(SceneFont.fromJson)));
  set fonts(List<SceneFont>? value) => writeField('fonts', value);

  /// 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは、表示と比べて決めた意味（dxf.TEXT.40 は capital_a。マイナーバージョン 59 から）にし、それもなければ大文字の高さとみなす。fonts を書いたときだけ使う。マイナーバージョン 37 から
  Map<String, String>? get heights => readField('heights', mapOf(asString));
  set heights(Map<String, String>? value) => writeField('heights', value);
}

/// 建物モデルを開口付きの 3D シーンにし、ZMS3 の blob で返す。86 から地物の面と高さのある線・面も扱う。階・クラス・ID の各条件の中は和、条件どうしは積。箱は要素の重なりで選び、メッシュを切らない。88 から点群を元の八分木の節ごとの点の区画（ZMS3 1.1）で返す。点群の点は pointcloud と同じ節の順で選び、座標系と高さの基準を変換しない。94 から平面直角座標系の GeoTIFF の DEM も、外枠の地物でなく標高格子を点群と同じ点の区画で返す。105 から surface で、DEM を地形の面（セル代表点を頂点にした三角形のメッシュ）で返す。マイナーバージョン 82 から
class Scene3dRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Scene3dRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  Scene3dRequest() : super({'op': 'scene3d'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// open で作った建物・地物・点群の文書名。点群は 88 から
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 所属する階の要素の ID。省略は全階、空の配列は何も選ばない。地物と点群には指定できない
  List<String>? get storeys => readField('storeys', listOf(asString));
  set storeys(List<String>? value) => writeField('storeys', value);

  /// 建物のクラス名か地物の型。大文字・小文字を区別する。省略は全種類、空の配列は何も選ばない。点群には指定できない
  List<String>? get classes => readField('classes', listOf(asString));
  set classes(List<String>? value) => writeField('classes', value);

  /// 建物の要素か元の地物の ID。省略は全要素、空の配列は何も選ばない。建物では参照する階の行も返す。点群には指定できない
  List<String>? get ids => readField('ids', listOf(asString));
  set ids(List<String>? value) => writeField('ids', value);

  /// 地物だけに指定する投影先の平面直角座標系の系（1〜19）。省略時は元の系、経緯度では範囲の中心から計算用の系を仮定して診断する。86 から
  core.int? get zone => readField('zone', asInt);
  set zone(core.int? value) => writeField('zone', value);

  /// 地物だけに指定する LOD（0〜4）。省略時は選択した地物にある最大の LOD を 1 段選ぶ。LOD のない形は含める。86 から
  core.int? get lod => readField('lod', asInt);
  set lod(core.int? value) => writeField('lod', value);

  /// プロジェクト空間の文書単位の箱。重なる要素を選び、形のない要素も残す。切り抜きは読み手が行う
  Scene3dBox? get box => readField('box', objectOf(Scene3dBox.fromJson));
  set box(Scene3dBox? value) => writeField('box', value);

  /// 点群だけに指定する返す点の数の上限。省略時は選んだすべての点。88 から。DEM を surface で返すときは、頂点にする格子点の数の上限。超えれば間引く間隔を 2 倍ずつ広げ、それでも超えれば何も返さない
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);

  /// 平面直角座標系の GeoTIFF の DEM だけに指定する。真なら標高格子を点でなく地形の面で返す。セル代表点を頂点にし、隣り合う 4 つの格子点が点にできれば 2 個、3 つなら 1 個の三角形を作る。nodata などのセルは面に含めない。面は 128 間隔四方の区画ごとの要素（ID は区画の初めの行と列の r<行>c<列>、クラスは dem）に分け、隣の区画と縁の頂点を共有する。座標系と高さの基準は変換しない。偽か省略なら点で返す。105 から
  core.bool? get surface => readField('surface', asBool);
  set surface(core.bool? value) => writeField('surface', value);

  /// 文書単位の正の有限値。点群では点の間隔（メートル）で、これより細かい段の節を選ばない。DEM の面では頂点の間隔（メートル）で、セル代表点の間隔の整数倍のうち、これ以上でもっとも細かい間隔に間引く。建物では曲線を分ける弦の誤差。地物では f32 の座標の丸めをこの値の 1/4 までに制限し、投影した辺全体の誤差は含めない。省略時は対象範囲の対角線の 1/50000、空か点だけなら 1 mm 相当。建物の範囲は 1 mm の予備評価で求める
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);
}

/// 建物ではプロジェクト空間の文書単位、地物では投影先の、点群では元の座標系の東・北・高さのメートルの箱。Z が上
class Scene3dBox extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Scene3dBox.fromJson(super.json);

  /// 空のオブジェクトから作る
  Scene3dBox() : super({});

  /// 最小の X・Y・Z の 3 値
  List<core.double>? get min => readField('min', listOf(asDouble));
  set min(List<core.double>? value) => writeField('min', value);

  /// 最大の X・Y・Z の 3 値
  List<core.double>? get max => readField('max', listOf(asDouble));
  set max(List<core.double>? value) => writeField('max', value);
}

/// 建物では文書が持つ地図への変換の元値で、シーンの座標へ適用していない。地物では投影先の EPSG 名と恒等変換を返し、シーンの X・Y は既に東・北のメートル
class Scene3dMap extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Scene3dMap.fromJson(super.json);

  /// 空のオブジェクトから作る
  Scene3dMap() : super({});

  /// 地図の座標系の名前。未指定なら持たない
  String? get crsName => readField('crs_name', asString);
  set crsName(String? value) => writeField('crs_name', value);

  /// 東への移動
  core.double? get eastings => readField('eastings', asDouble);
  set eastings(core.double? value) => writeField('eastings', value);

  /// 北への移動
  core.double? get northings => readField('northings', asDouble);
  set northings(core.double? value) => writeField('northings', value);

  /// 高さの移動
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 地図の X 軸の横の成分の元値
  core.double? get xAxisAbscissa => readField('x_axis_abscissa', asDouble);
  set xAxisAbscissa(core.double? value) => writeField('x_axis_abscissa', value);

  /// 地図の X 軸の縦の成分の元値
  core.double? get xAxisOrdinate => readField('x_axis_ordinate', asDouble);
  set xAxisOrdinate(core.double? value) => writeField('x_axis_ordinate', value);

  /// 地図への尺度の元値
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);
}

/// 返した 3D シーンの列の行数。共有するメッシュの点と面は 1 回だけ数える
class Scene3dCounts extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Scene3dCounts.fromJson(super.json);

  /// 空のオブジェクトから作る
  Scene3dCounts() : super({});

  /// 塊
  core.int? get chunks => readField('chunks', asInt);
  set chunks(core.int? value) => writeField('chunks', value);

  /// 共有するメッシュ
  core.int? get meshes => readField('meshes', asInt);
  set meshes(core.int? value) => writeField('meshes', value);

  /// メッシュの配置
  core.int? get instances => readField('instances', asInt);
  set instances(core.int? value) => writeField('instances', value);

  /// 要素と参照先の階。形のない行も含む
  core.int? get elements => readField('elements', asInt);
  set elements(core.int? value) => writeField('elements', value);

  /// 色
  core.int? get colors => readField('colors', asInt);
  set colors(core.int? value) => writeField('colors', value);

  /// 局所の頂点
  core.int? get vertices => readField('vertices', asInt);
  set vertices(core.int? value) => writeField('vertices', value);

  /// 三角形
  core.int? get triangles => readField('triangles', asInt);
  set triangles(core.int? value) => writeField('triangles', value);

  /// 形の稜線
  core.int? get edges => readField('edges', asInt);
  set edges(core.int? value) => writeField('edges', value);

  /// 点群の点。点群のシーンだけが持つ。88 から
  core.int? get points => readField('points', asInt);
  set points(core.int? value) => writeField('points', value);
}

/// 3D シーンの座標・許容差・行数と blob の番号
class Scene3dResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Scene3dResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  Scene3dResult() : super({});

  /// 元の建物か地物の文書名
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// シーンの座標と許容差の長さの単位
  BuildingUnit? get unit => readField('unit', objectOf(BuildingUnit.fromJson));
  set unit(BuildingUnit? value) => writeField('unit', value);

  /// 文書の地図への変換。なければ持たない
  Scene3dMap? get map => readField('map', objectOf(Scene3dMap.fromJson));
  set map(Scene3dMap? value) => writeField('map', value);

  /// 地物の高さの基準と成果を表す EPSG。元が不明なら持たない。86 から
  core.int? get heightCrs => readField('height_crs', asInt);
  set heightCrs(core.int? value) => writeField('height_crs', value);

  /// 地物で選択した LOD。LOD のない形だけなら持たない。86 から
  core.int? get lod => readField('lod', asInt);
  set lod(core.int? value) => writeField('lod', value);

  /// 使った文書単位の許容差。建物では曲線の弦の誤差、地物では f32 の丸めの上限の 4 倍。点群では要求の点の間隔、省略時は返したもっとも深い段の点の間隔。DEM の面では使った頂点の間隔。丸めのずれは別の診断で返す
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 点群で上限のために返さなかった点がある。DEM の面では、上限のために面を返さなかった。点群と DEM のシーンだけが持つ。88 から
  core.bool? get truncated => readField('truncated', asBool);
  set truncated(core.bool? value) => writeField('truncated', value);

  /// 要求の断面の箱。なければ持たない
  Scene3dBox? get box => readField('box', objectOf(Scene3dBox.fromJson));
  set box(Scene3dBox? value) => writeField('box', value);

  /// 返した塊の文書単位の範囲。形がなければ持たない
  Scene3dBox? get extent => readField('extent', objectOf(Scene3dBox.fromJson));
  set extent(Scene3dBox? value) => writeField('extent', value);

  /// シーンの行数
  Scene3dCounts? get counts => readField('counts', objectOf(Scene3dCounts.fromJson));
  set counts(Scene3dCounts? value) => writeField('counts', value);

  /// 種類が scene3d の blob の番号。zm_take で取り出す
  core.int? get blob => readField('blob', asInt);
  set blob(core.int? value) => writeField('blob', value);
}

/// 開いた文書を scene と同じ起点・配置・フォントで 2D のシーンにし、画像を blob で返す（決定 0272・0275・0277）。字形の輪郭だけを描き、字形のない空でない文字・外部画像・名前付きの点記号は元の ID と unit を診断して描かない。シーン生成と描画で同じ予算と中断の合図を使い、失敗では画像を返さない。PNG は最大 16777216 画素で、変換後の座標は絶対値 2^31 ピクセル以内。SVG はマイナーバージョン 64、PNG は 65 から
class RenderRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RenderRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  RenderRequest() : super({'op': 'render'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// scene と同じ起点。実寸の空間・記号の定義の空間・用紙の ID
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 起点の用紙のビューの番号。0 から数える。from が用紙のときだけ書ける。scene と同じ意味
  core.int? get view => readField('view', asInt);
  set view(core.int? value) => writeField('view', value);

  /// 画像の出力形式
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 出力の幅（ピクセル）
  core.int? get width => readField('width', asInt);
  set width(core.int? value) => writeField('width', value);

  /// 出力の高さ（ピクセル）
  core.int? get height => readField('height', asInt);
  set height(core.int? value) => writeField('height', value);

  /// [x の最小, y の最小, x の最大, y の最大]。起点の座標で、幅と高さが正の有限値になる範囲を明示する。scene の範囲の絞り込みにも使い、縦横同率で中央に収めて範囲外を切る。縦横比の違いによる余白は背景色にする
  List<core.double>? get region => readField('region', listOf(asDouble));
  set region(List<core.double>? value) => writeField('region', value);

  /// 用紙 mm の線幅をピクセルへ直す DPI。正の有限値。座標の単位を mm と推測しない。線幅 0 は 1 ピクセル
  core.double? get dpi => readField('dpi', asDouble);
  set dpi(core.double? value) => writeField('dpi', value);

  /// 背景の sRGB と alpha。[r, g, b, a] の 4 値を 0〜255 で明示する
  List<core.int>? get background => readField('background', listOf(asInt));
  set background(List<core.int>? value) => writeField('background', value);

  /// ACI 7 と未解決の色に使う sRGB と alpha。[r, g, b, a] の 4 値を 0〜255 で明示する。未解決の色は診断する
  List<core.int>? get foreground => readField('foreground', listOf(asInt));
  set foreground(List<core.int>? value) => writeField('foreground', value);

  /// scene と同じ弦の誤差（起点の長さ）。0 より大きい。書かなければ用紙は 0.02 mm、空間は要素の範囲の対角線の 1/50000
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// scene と同じ指定。true は印刷する要素、書かないか false は表示する要素を描く
  core.bool? get print => readField('print', asBool);
  set print(core.bool? value) => writeField('print', value);

  /// 隠すレイヤの公開 ID の一覧。scene の layer_ids と同じ値で描画単位を除く。未知の ID は失敗、省略または空なら従来どおり描く
  List<String>? get hiddenLayers => readField('hidden_layers', listOf(asString));
  set hiddenLayers(List<String>? value) => writeField('hidden_layers', value);

  /// scene と同じ色の表の入力の名前、または組み込みの表 builtin:cad-v1。zm_push で渡した JSON の values に rgb と aci・sxf・jww の符号を並べる
  String? get palette => readField('palette', asString);
  set palette(String? value) => writeField('palette', value);

  /// scene と同じフォントの指定。名前を合わせ、なければ名前のないフォントを使う。字形の輪郭を作るため、zm_push でフォントのバイト列を渡す。OS のフォントは探索しない
  List<SceneFont>? get fonts => readField('fonts', listOf(objectOf(SceneFont.fromJson)));
  set fonts(List<SceneFont>? value) => writeField('fonts', value);

  /// scene と同じ宣言値の高さの意味。fonts を書いたときだけ使う。書かないキーは既定の意味に従う（決定 0176・0188）
  Map<String, String>? get heights => readField('heights', mapOf(asString));
  set heights(Map<String, String>? value) => writeField('heights', value);
}

/// 画像の出力形式。svg は SVG 1.1（image/svg+xml）、png は RGBA8・非インターレースの PNG（image/png、マイナーバージョン 65 から）
///
/// 列挙の値。知らない値も受け取る
abstract final class RenderFormat {
  static const svg = 'svg';
  static const png = 'png';
}

/// 画像の形式・サイズと、シーンの起点・単位・許容差、画像の blob の番号
class RenderResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RenderResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  RenderResult() : super({});

  /// 画像の出力形式
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 出力の幅（ピクセル）
  core.int? get width => readField('width', asInt);
  set width(core.int? value) => writeField('width', value);

  /// 出力の高さ（ピクセル）
  core.int? get height => readField('height', asInt);
  set height(core.int? value) => writeField('height', value);

  /// シーンの起点の ID
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 起点のビューの番号。ビューでなければ持たない
  core.int? get view => readField('view', asInt);
  set view(core.int? value) => writeField('view', value);

  /// 起点の座標の単位。用紙とビューからは millimeter
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// シーンで使った弦の誤差（起点の長さ）
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 画像の blob の番号（種類は image）。zm_take で取り出す
  core.int? get blob => readField('blob', asInt);
  set blob(core.int? value) => writeField('blob', value);
}

/// 開いた TIFF・GeoTIFF の指定ページの画素を RGBA8・非インターレースの PNG に復号し、画像の blob で返す（#877）。複数ページの TIFF は page で 1 から数えるページ番号を選ぶ（書かなければ 1）。max_side を指定すると、長辺が max_side 以下になるよう縦横比を保って縮小して返す（最大 16777216 画素）。マイナーバージョン 73 から
class RasterRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RasterRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  RasterRequest() : super({'op': 'raster'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// ページ番号（IFD の 1 から始まる番号）。書かなければ 1
  core.int? get page => readField('page', asInt);
  set page(core.int? value) => writeField('page', value);

  /// 出力の長辺の最大ピクセル数。元の長辺がこれを超えるときだけ縦横比を保って縮小する
  core.int? get maxSide => readField('max_side', asInt);
  set maxSide(core.int? value) => writeField('max_side', value);
}

/// 復号した PNG 画像のサイズ・元のページサイズ・総ページ数と、画像の blob の番号
class RasterResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RasterResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  RasterResult() : super({});

  /// 出力した PNG の幅（ピクセル）
  core.int? get width => readField('width', asInt);
  set width(core.int? value) => writeField('width', value);

  /// 出力した PNG の高さ（ピクセル）
  core.int? get height => readField('height', asInt);
  set height(core.int? value) => writeField('height', value);

  /// 元のページの幅（ピクセル）
  core.int? get sourceWidth => readField('source_width', asInt);
  set sourceWidth(core.int? value) => writeField('source_width', value);

  /// 元のページの高さ（ピクセル）
  core.int? get sourceHeight => readField('source_height', asInt);
  set sourceHeight(core.int? value) => writeField('source_height', value);

  /// 復号したページ番号（1 から）
  core.int? get page => readField('page', asInt);
  set page(core.int? value) => writeField('page', value);

  /// 文書に含まれる総ページ（IFD）数
  core.int? get pages => readField('pages', asInt);
  set pages(core.int? value) => writeField('pages', value);

  /// PNG 画像の blob の番号（種類は image）。zm_take で取り出す
  core.int? get blob => readField('blob', asInt);
  set blob(core.int? value) => writeField('blob', value);
}

/// シーンの文字を置くフォント（決定 0188）
class SceneFont extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SceneFont.fromJson(super.json);

  /// 空のオブジェクトから作る
  SceneFont() : super({});

  /// 文字が指すフォントの名前（ファイルに書かれた元の名前）。書かなければ、名前の合わない文字に使う既定のフォント。同じ名前と、名前のないものは 1 つまで
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// TrueType・OpenType（CFF・CFF2 を含む）のファイルの入力の名前（zm_push で渡したもの）
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);

  /// フォントのコレクション（TTC）の中の番号。書かなければ 0
  core.int? get index => readField('index', asInt);
  set index(core.int? value) => writeField('index', value);
}

/// 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ、capital_a は A の字形の外形の上（A がなければ大文字の高さ。マイナーバージョン 59 から）
///
/// 列挙の値。知らない値も受け取る
abstract final class HeightReference {
  static const capHeight = 'cap_height';
  static const em = 'em';
  static const frame = 'frame';
  static const capitalA = 'capital_a';
}

/// シーンの要約と、シーンの blob の番号
class SceneResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SceneResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  SceneResult() : super({});

  /// 起点の ID
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 起点のビューの番号。ビューでなければ持たない
  core.int? get view => readField('view', asInt);
  set view(core.int? value) => writeField('view', value);

  /// 起点の座標の単位。用紙とビューからは millimeter
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 使った弦の誤差（起点の長さ）
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 返した描くものの数
  SceneCounts? get counts => readField('counts', objectOf(SceneCounts.fromJson));
  set counts(SceneCounts? value) => writeField('counts', value);

  /// 返した塊の範囲。起点の座標。塊がなければ持たない
  Extent? get extent => readField('extent', objectOf(Extent.fromJson));
  set extent(Extent? value) => writeField('extent', value);

  /// シーンの blob の番号（種類は scene）。zm_take で取り出す
  core.int? get blob => readField('blob', asInt);
  set blob(core.int? value) => writeField('blob', value);
}

/// シーンの描くものの種類ごとの数
class SceneCounts extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SceneCounts.fromJson(super.json);

  /// 空のオブジェクトから作る
  SceneCounts() : super({});

  /// 塊
  core.int? get chunks => readField('chunks', asInt);
  set chunks(core.int? value) => writeField('chunks', value);

  /// 線（折れ線）
  core.int? get lines => readField('lines', asInt);
  set lines(core.int? value) => writeField('lines', value);

  /// 線と面の頂点
  core.int? get vertices => readField('vertices', asInt);
  set vertices(core.int? value) => writeField('vertices', value);

  /// 面
  core.int? get faces => readField('faces', asInt);
  set faces(core.int? value) => writeField('faces', value);

  /// 文字
  core.int? get texts => readField('texts', asInt);
  set texts(core.int? value) => writeField('texts', value);

  /// 点
  core.int? get points => readField('points', asInt);
  set points(core.int? value) => writeField('points', value);

  /// 画像
  core.int? get images => readField('images', asInt);
  set images(core.int? value) => writeField('images', value);

  /// 置いた字形。fonts を書かなければ 0。マイナーバージョン 37 から
  core.int? get glyphs => readField('glyphs', asInt);
  set glyphs(core.int? value) => writeField('glyphs', value);
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。マイナーバージョン 34 から
class CheckRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CheckRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CheckRequest() : super({'op': 'check'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
  List<String>? get profiles => readField('profiles', listOf(asString));
  set profiles(List<String>? value) => writeField('profiles', value);

  /// 照合に使うプロファイル。profiles の中から選ぶ
  ProfileRef? get profile => readField('profile', objectOf(ProfileRef.fromJson));
  set profile(ProfileRef? value) => writeField('profile', value);

  /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
  core.int? get view => readField('view', asInt);
  set view(core.int? value) => writeField('view', value);

  /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。マイナーバージョン 40 から
  String? get fileName => readField('file_name', asString);
  set fileName(String? value) => writeField('file_name', value);

  /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
  core.int? get offset => readField('offset', asInt);
  set offset(core.int? value) => writeField('offset', value);

  /// 返す指摘の数の上限。書かなければ 1000
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);

  /// 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
  String? get palette => readField('palette', asString);
  set palette(String? value) => writeField('palette', value);
}

/// プロファイルの識別（決定 0138）
class ProfileRef extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ProfileRef.fromJson(super.json);

  /// 空のオブジェクトから作る
  ProfileRef() : super({});

  /// 基準の系統（mlit-cad など）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 基準の版（2025-12 など）
  String? get edition => readField('edition', asString);
  set edition(String? value) => writeField('edition', value);
}

/// 照合の結果。指摘は offset から limit 個まで
class CheckResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CheckResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  CheckResult() : super({});

  /// 照合に使ったプロファイルの鎖。extends の基から順
  List<ProfileIdentity>? get chain => readField('chain', listOf(objectOf(ProfileIdentity.fromJson)));
  set chain(List<ProfileIdentity>? value) => writeField('chain', value);

  /// 照合した規則ごとの数。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
  List<CheckRule>? get rules => readField('rules', listOf(objectOf(CheckRule.fromJson)));
  set rules(List<CheckRule>? value) => writeField('rules', value);

  /// 指摘。規則の順に、規則ごとにたどった順
  List<CheckFinding>? get findings => readField('findings', listOf(objectOf(CheckFinding.fromJson)));
  set findings(List<CheckFinding>? value) => writeField('findings', value);

  /// 全体の指摘の数
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 続きの最初の指摘の位置。続きがなければ持たない
  core.int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(core.int? value) => writeField('next_offset', value);
}

/// 照合に使ったプロファイルの識別と、データの直し
class ProfileIdentity extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ProfileIdentity.fromJson(super.json);

  /// 空のオブジェクトから作る
  ProfileIdentity() : super({});

  /// 基準の系統
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 基準の版
  String? get edition => readField('edition', asString);
  set edition(String? value) => writeField('edition', value);

  /// このデータの直し。1 から
  core.int? get revision => readField('revision', asInt);
  set revision(core.int? value) => writeField('revision', value);
}

/// 規則ごとの数
class CheckRule extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CheckRule.fromJson(super.json);

  /// 空のオブジェクトから作る
  CheckRule() : super({});

  /// 規則の id
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 照合の種類（layer-name など）
  String? get check => readField('check', asString);
  set check(String? value) => writeField('check', value);

  /// 指摘の数。照合できなかったものを除く
  core.int? get findings => readField('findings', asInt);
  set findings(core.int? value) => writeField('findings', value);

  /// 照合できなかった数
  core.int? get unsupported => readField('unsupported', asInt);
  set unsupported(core.int? value) => writeField('unsupported', value);
}

/// 指摘。diagnostic は診断と同じ形で、describe で文にできる。severity は規則の level で、照合できない値（profile.<種類>.unsupported）は info。count は 1
class CheckFinding extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CheckFinding.fromJson(super.json);

  /// 空のオブジェクトから作る
  CheckFinding() : super({});

  /// 規則の id
  String? get rule => readField('rule', asString);
  set rule(String? value) => writeField('rule', value);

  /// code・重要度・場所（要素・実効のレイヤ・用紙の ID）・見つけた値
  Diagnostic? get diagnostic => readField('diagnostic', objectOf(Diagnostic.fromJson));
  set diagnostic(Diagnostic? value) => writeField('diagnostic', value);
}

/// zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。マイナーバージョン 42 から
class ValidateRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ValidateRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ValidateRequest() : super({'op': 'validate'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// IFC の入力の名前（zm_push で渡した .ifc か .ifczip）
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);

  /// EXPRESS の入力の名前（zm_push で渡した .exp）
  String? get express => readField('express', asString);
  set express(String? value) => writeField('express', value);

  /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
  core.int? get offset => readField('offset', asInt);
  set offset(core.int? value) => writeField('offset', value);

  /// 返す指摘の数の上限。書かなければ 1000
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);
}

/// スキーマに照らしたチェックの結果。指摘は offset から limit 個まで。スキーマの名前の違いと評価しない規則は、診断で返す
class ValidateResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ValidateResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  ValidateResult() : super({});

  /// EXPRESS のスキーマの名前（大文字）
  String? get schema => readField('schema', asString);
  set schema(String? value) => writeField('schema', value);

  /// IFC の FILE_SCHEMA の名前
  String? get fileSchema => readField('file_schema', asString);
  set fileSchema(String? value) => writeField('file_schema', value);

  /// EXPRESS のバイト列のハッシュ（v2: と 32 桁の 16 進）。どの EXPRESS でチェックしたかを残す
  String? get expressHash => readField('express_hash', asString);
  set expressHash(String? value) => writeField('express_hash', value);

  /// 評価しなかった規則（WHERE・UNIQUE・RULE）の数。確認できなかった項目
  core.int? get rulesNotEvaluated => readField('rules_not_evaluated', asInt);
  set rulesNotEvaluated(core.int? value) => writeField('rules_not_evaluated', value);

  /// 指摘。インスタンスの番号の順に、属性の順。診断と同じ形で、describe で文にできる。count は 1
  List<Diagnostic>? get findings => readField('findings', listOf(objectOf(Diagnostic.fromJson)));
  set findings(List<Diagnostic>? value) => writeField('findings', value);

  /// 全体の指摘の数
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 続きの最初の指摘の位置。続きがなければ持たない
  core.int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(core.int? value) => writeField('next_offset', value);
}

/// レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書のバージョンへ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元のバージョンのハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。マイナーバージョン 43 から
class ReviewCarryRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewCarryRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ReviewCarryRequest() : super({'op': 'review_carry'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 新しいバージョンの文書の名前。open で作ったもの。マイナーバージョン 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// レビューの文書の入力の名前（zm_push で渡した JSON）
  String? get review => readField('review', asString);
  set review(String? value) => writeField('review', value);

  /// 移す対象の ID。レビューの文書の targets の id
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0
  core.int? get offset => readField('offset', asInt);
  set offset(core.int? value) => writeField('offset', value);

  /// 返す参照の数の上限。書かなければ 1000
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);
}

/// 移した結果。参照は offset から limit 個まで。移した文書は blobs の file で返す
class ReviewCarryResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewCarryResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  ReviewCarryResult() : super({});

  /// 移す前の対象のハッシュ
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 移した後の対象のハッシュ。文書のハッシュ
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 要素の参照ごとの状態。指摘の順に、場所の順、参照の順
  List<CarriedReference>? get references => readField('references', listOf(objectOf(CarriedReference.fromJson)));
  set references(List<CarriedReference>? value) => writeField('references', value);

  /// 全体の参照の数
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 続きの最初の参照の位置。続きがなければ持たない
  core.int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(core.int? value) => writeField('next_offset', value);
}

/// 移した要素の参照
class CarriedReference extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CarriedReference.fromJson(super.json);

  /// 空のオブジェクトから作る
  CarriedReference() : super({});

  /// 指摘の ID
  String? get topic => readField('topic', asString);
  set topic(String? value) => writeField('topic', value);

  /// 場所の ID
  String? get location => readField('location', asString);
  set location(String? value) => writeField('location', value);

  /// 要素の ID
  String? get element => readField('element', asString);
  set element(String? value) => writeField('element', value);

  /// 状態
  String? get state => readField('state', asString);
  set state(String? value) => writeField('state', value);
}

/// 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しいバージョンに要素がない
///
/// 列挙の値。知らない値も受け取る
abstract final class CarriedState {
  static const unchanged = 'unchanged';
  static const changed = 'changed';
  static const removed = 'removed';
}

/// 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。マイナーバージョン 43 から
class ReviewAdoptRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewAdoptRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ReviewAdoptRequest() : super({'op': 'review_adopt'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 照合する文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
  List<String>? get profiles => readField('profiles', listOf(asString));
  set profiles(List<String>? value) => writeField('profiles', value);

  /// 照合に使うプロファイル。profiles の中から選ぶ
  ProfileRef? get profile => readField('profile', objectOf(ProfileRef.fromJson));
  set profile(ProfileRef? value) => writeField('profile', value);

  /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
  core.int? get view => readField('view', asInt);
  set view(core.int? value) => writeField('view', value);

  /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。マイナーバージョン 40 から
  String? get fileName => readField('file_name', asString);
  set fileName(String? value) => writeField('file_name', value);

  /// 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
  String? get palette => readField('palette', asString);
  set palette(String? value) => writeField('palette', value);

  /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
  String? get review => readField('review', asString);
  set review(String? value) => writeField('review', value);

  /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 取り込む指摘の番号の列。check の指摘の並びの順で、0 から数える
  List<core.int>? get findings => readField('findings', listOf(asInt));
  set findings(List<core.int>? value) => writeField('findings', value);
}

/// 取り込んだ結果。足した文書は blobs の file で返す
class ReviewAdoptResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewAdoptResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  ReviewAdoptResult() : super({});

  /// 足した指摘の ID。取り込み済みで足さなかった指摘は持たない
  List<String>? get added => readField('added', listOf(asString));
  set added(List<String>? value) => writeField('added', value);

  /// 足した後の文書の指摘の数
  core.int? get topics => readField('topics', asInt);
  set topics(core.int? value) => writeField('topics', value);
}

/// レビューの文書（決定 0214・0295）の 1 つの対象の図形と指摘の場所を、開いた図面の起点で、図面とは別の 2D のシーンにして blob で返す。対象の形式とハッシュは開いた文書から作り、文書の対象と合わなければ描かない。シーンの配置は scene と同じ schema/scene.toml。色・線幅・寸法は style で明示し、文書の値から推測しない。レビューの文書と図面は変えない。マイナーバージョン 95 から
class ReviewSceneRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewSceneRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ReviewSceneRequest() : super({'op': 'review_scene'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 対象の図面の文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// レビューの文書の入力の名前（zm_push で渡した JSON）
  String? get review => readField('review', asString);
  set review(String? value) => writeField('review', value);

  /// 描く対象の ID。レビューの文書の targets の id
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 起点。実寸の空間・記号の定義の空間・用紙の ID（scene と同じ）。文書の図形・場所の view がこの起点を指すものだけを描く
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 起点の用紙のビューの番号。0 から数える。from が用紙のときだけ書ける。書けば、文書の view が用紙の ID に /view/<番号> を足した値の項目だけを描く
  core.int? get view => readField('view', asInt);
  set view(core.int? value) => writeField('view', value);

  /// [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
  List<core.double>? get region => readField('region', listOf(asDouble));
  set region(List<core.double>? value) => writeField('region', value);

  /// 弦の誤差（起点の長さ）。0 より大きい有限値。既定値を補わない
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 文字を置くフォント（scene と同じ）。style の font の名前で引く。書かなければ字形を置かず、注記の枠を文字高と字数から概算して診断する
  List<SceneFont>? get fonts => readField('fonts', listOf(objectOf(SceneFont.fromJson)));
  set fonts(List<SceneFont>? value) => writeField('fonts', value);

  /// 選んだ項目に共通で当てる色と寸法
  ReviewSceneStyle? get style => readField('style', objectOf(ReviewSceneStyle.fromJson));
  set style(ReviewSceneStyle? value) => writeField('style', value);
}

/// レビューの図形の色と寸法。幾何の寸法は起点の単位、線幅だけは用紙の mm。文書には保存しない
class ReviewSceneStyle extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewSceneStyle.fromJson(super.json);

  /// 空のオブジェクトから作る
  ReviewSceneStyle() : super({});

  /// 線・面・文字の色。#rrggbb
  String? get color => readField('color', asString);
  set color(String? value) => writeField('color', value);

  /// 用紙の mm の線幅。0 以上の有限値。0 はもっとも細い線
  core.double? get lineWeight => readField('line_weight', asDouble);
  set lineWeight(core.double? value) => writeField('line_weight', value);

  /// 起点の単位での文字の高さ。正の有限値
  core.double? get textHeight => readField('text_height', asDouble);
  set textHeight(core.double? value) => writeField('text_height', value);

  /// 起点の単位での注記の枠の余白。0 以上の有限値
  core.double? get textPadding => readField('text_padding', asDouble);
  set textPadding(core.double? value) => writeField('text_padding', value);

  /// 起点の単位での矢じりの長さ。正の有限値
  core.double? get arrowLength => readField('arrow_length', asDouble);
  set arrowLength(core.double? value) => writeField('arrow_length', value);

  /// 赤線の矢じりを置く端
  String? get arrows => readField('arrows', asString);
  set arrows(String? value) => writeField('arrows', value);

  /// 雲の円弧の弦の長さの上限。起点の単位の正の有限値
  core.double? get cloudPitch => readField('cloud_pitch', asDouble);
  set cloudPitch(core.double? value) => writeField('cloud_pitch', value);

  /// 写真と指摘の位置の記号の直径。起点の単位の正の有限値
  core.double? get markerSize => readField('marker_size', asDouble);
  set markerSize(core.double? value) => writeField('marker_size', value);

  /// fonts から引くフォントの名前。書かなければ名前のないフォント
  String? get font => readField('font', asString);
  set font(String? value) => writeField('font', value);
}

/// 赤線の矢じりを置く端。none は置かない、start は最初の端点、end は最後の端点、both は両端
///
/// 列挙の値。知らない値も受け取る
abstract final class ReviewArrows {
  static const none = 'none';
  static const start = 'start';
  static const end = 'end';
  static const both = 'both';
}

/// レビューのシーンの要約と、シーンの blob の番号、シーンの ID から元の図形・場所への対応
class ReviewSceneResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewSceneResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  ReviewSceneResult() : super({});

  /// 起点の ID
  String? get from => readField('from', asString);
  set from(String? value) => writeField('from', value);

  /// 起点のビューの番号。ビューでなければ持たない
  core.int? get view => readField('view', asInt);
  set view(core.int? value) => writeField('view', value);

  /// 起点の座標の単位。用紙とビューからは millimeter。単位の不明な空間は unknown のまま
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 使った弦の誤差（起点の長さ）
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 返した描くものの数
  SceneCounts? get counts => readField('counts', objectOf(SceneCounts.fromJson));
  set counts(SceneCounts? value) => writeField('counts', value);

  /// 返した塊の範囲。起点の座標。塊がなければ持たない
  Extent? get extent => readField('extent', objectOf(Extent.fromJson));
  set extent(Extent? value) => writeField('extent', value);

  /// 描いた図形・場所ごとの対応。描く順（文書の markups の順、続いて topics と各指摘の locations の順）
  List<ReviewSceneSource>? get sources => readField('sources', listOf(objectOf(ReviewSceneSource.fromJson)));
  set sources(List<ReviewSceneSource>? value) => writeField('sources', value);

  /// シーンの blob の番号（種類は scene）。zm_take で取り出す
  core.int? get blob => readField('blob', asInt);
  set blob(core.int? value) => writeField('blob', value);
}

/// シーンの ID（シーンの entity_ids）から、元のレビューの文書の図形か指摘の場所へ戻す対応
class ReviewSceneSource extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewSceneSource.fromJson(super.json);

  /// 空のオブジェクトから作る
  ReviewSceneSource() : super({});

  /// シーンの ID。review/markup/<hex> か review/location/<hex>/<hex>
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 元の項目の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 元の図形の ID。kind が markup のときだけ持つ
  String? get markup => readField('markup', asString);
  set markup(String? value) => writeField('markup', value);

  /// 元の指摘の ID。kind が location のときだけ持つ
  String? get topic => readField('topic', asString);
  set topic(String? value) => writeField('topic', value);

  /// 元の場所の ID。kind が location のときだけ持つ
  String? get location => readField('location', asString);
  set location(String? value) => writeField('location', value);
}

/// シーンの ID の元の項目の種類。markup はレビューの文書の図形、location は指摘の中の場所
///
/// 列挙の値。知らない値も受け取る
abstract final class ReviewSceneSourceKind {
  static const markup = 'markup';
  static const location = 'location';
}

/// 開いた IFC の階を基準に水平に切り、元要素への対応を持つ図面を作る。マイナーバージョン 114 から
class BuildingDrawingRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingDrawingRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingDrawingRequest() : super({'op': 'building_drawing'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 開いた IFC の文書名
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 作る図面の文書名
  String? get to => readField('to', asString);
  set to(String? value) => writeField('to', value);

  /// 切る高さの基準となる階の要素 ID
  String? get storey => readField('storey', asString);
  set storey(String? value) => writeField('storey', value);

  /// 切る高さ・見る深さ・許容差・弦の誤差の長さの単位。unknown は使えない
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 階から切る面までの高さ
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 切る面から下向きに見る深さ。正の値
  core.double? get depth => readField('depth', asDouble);
  set depth(core.double? value) => writeField('depth', value);

  /// 用紙の縮尺の分母。正の値
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);

  /// 同じ点とみなす格子の幅。正の値
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 曲線の近似に使う弦の誤差。正の値
  core.double? get chord => readField('chord', asDouble);
  set chord(core.double? value) => writeField('chord', value);
}

/// 実寸の線と用紙のビューを持つ図面。線の拡張 zumen.building.drawing に派生 ID・役割・元の要素と形の出所を持つ
class BuildingDrawingResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingDrawingResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingDrawingResult() : super({});

  /// 作った図面の文書名
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 図面モデルのバイナリの blob 番号。種類は drawing
  core.int? get blob => readField('blob', asInt);
  set blob(core.int? value) => writeField('blob', value);

  /// 図面に書いた線の数
  core.int? get lines => readField('lines', asInt);
  set lines(core.int? value) => writeField('lines', value);

  /// 切り出しで形状の欠落がなかったか
  core.bool? get complete => readField('complete', asBool);
  set complete(core.bool? value) => writeField('complete', value);

  /// 形状の評価が完全でなかった要素の数
  core.int? get evaluationFailures => readField('evaluation_failures', asInt);
  set evaluationFailures(core.int? value) => writeField('evaluation_failures', value);

  /// 投影や切り口の計算に失敗した部品の数
  core.int? get projectionFailures => readField('projection_failures', asInt);
  set projectionFailures(core.int? value) => writeField('projection_failures', value);

  /// 格子で点などへ縮んだ辺や輪郭の数
  core.int? get collapsed => readField('collapsed', asInt);
  set collapsed(core.int? value) => writeField('collapsed', value);
}

/// 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。マイナーバージョン 44 から
class BuildingRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingRequest() : super({'op': 'building'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で IFC を開いたもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);
}

/// 建物モデルの要約
class BuildingResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingResult() : super({});

  /// IFC のスキーマの名前（FILE_SCHEMA の最初）
  String? get schema => readField('schema', asString);
  set schema(String? value) => writeField('schema', value);

  /// 入力のバイト列のハッシュ。review_carry の対象のバージョンに使う
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// プロジェクトの要素の ID。なければ持たない
  String? get project => readField('project', asString);
  set project(String? value) => writeField('project', value);

  /// プロジェクトの名前。なければ持たない
  String? get projectName => readField('project_name', asString);
  set projectName(String? value) => writeField('project_name', value);

  /// プロジェクトの単位
  List<BuildingUnit>? get units => readField('units', listOf(objectOf(BuildingUnit.fromJson)));
  set units(List<BuildingUnit>? value) => writeField('units', value);

  /// 要素の数
  core.int? get elements => readField('elements', asInt);
  set elements(core.int? value) => writeField('elements', value);

  /// クラスごとの要素の数。クラスの名前の順
  List<ClassCount>? get classes => readField('classes', listOf(objectOf(ClassCount.fromJson)));
  set classes(List<ClassCount>? value) => writeField('classes', value);
}

/// 単位
class BuildingUnit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingUnit.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingUnit() : super({});

  /// 単位の種類（LENGTHUNIT・AREAUNIT など）
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// SI の接頭辞（MILLI など）
  String? get prefix => readField('prefix', asString);
  set prefix(String? value) => writeField('prefix', value);

  /// 単位の名前（METRE、換算する単位なら inch など）
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 換算する単位の、SI の単位での大きさ。SI の単位は持たない
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);
}

/// クラスの要素の数
class ClassCount extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ClassCount.fromJson(super.json);

  /// 空のオブジェクトから作る
  ClassCount() : super({});

  /// IFC のクラスの名前（IfcWall など）
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 要素の数
  core.int? get count => readField('count', asInt);
  set count(core.int? value) => writeField('count', value);
}

/// 開いた IFC の建物モデルの要素を、書いた順に返す。コンポーネントは components で選ぶ（決定 0207・0219）。マイナーバージョン 44 から
class BuildingElementsRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingElementsRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingElementsRequest() : super({'op': 'building_elements'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で IFC を開いたもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 返す要素の ID。書けば、その要素だけを、書いた順に返す。ない ID は返さない
  List<String>? get ids => readField('ids', listOf(asString));
  set ids(List<String>? value) => writeField('ids', value);

  /// IFC のクラスの名前。書けば、そのクラスの要素だけを返す。大文字と小文字を区別しない
  String? get classField => readField('class', asString);
  set classField(String? value) => writeField('class', value);

  /// 返すコンポーネントの名前。1 つ以上で、重ねない。書かなければ class と identity を返す
  List<String>? get components => readField('components', listOf(asString));
  set components(List<String>? value) => writeField('components', value);

  /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
  core.int? get offset => readField('offset', asInt);
  set offset(core.int? value) => writeField('offset', value);

  /// 返す数の上限。書かなければ 1000
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);
}

/// building_elements で返すコンポーネント（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員
///
/// 列挙の値。知らない値も受け取る
abstract final class BuildingComponent {
  static const identity = 'identity';
  static const classValue = 'class';
  static const classification = 'classification';
  static const spatial = 'spatial';
  static const parent = 'parent';
  static const placement = 'placement';
  static const typeOf = 'type_of';
  static const properties = 'properties';
  static const quantities = 'quantities';
  static const material = 'material';
  static const opening = 'opening';
  static const joinery = 'joinery';
  static const group = 'group';
  static const connection = 'connection';
  static const level = 'level';
  static const zone = 'zone';
  static const style = 'style';
  static const relations = 'relations';
}

/// 要素の一覧。offset から limit 個まで
class BuildingElementsResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingElementsResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingElementsResult() : super({});

  /// 要素
  List<BuildingElement>? get elements => readField('elements', listOf(objectOf(BuildingElement.fromJson)));
  set elements(List<BuildingElement>? value) => writeField('elements', value);

  /// 選んだ要素の数
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 続きの最初の位置。続きがなければ持たない
  core.int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(core.int? value) => writeField('next_offset', value);
}

/// 建物モデルの要素。コンポーネントは、選んだもののうち要素が持つものだけを持つ
class BuildingElement extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingElement.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingElement() : super({});

  /// 要素の ID（GlobalId か、i と番号）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 要素のコンポーネントのハッシュ（v1: と 32 桁の 16 進、決定 0207）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 名前・説明・タグ・長い名前
  IdentityComponent? get identity => readField('identity', objectOf(IdentityComponent.fromJson));
  set identity(IdentityComponent? value) => writeField('identity', value);

  /// クラス
  ClassComponent? get classField => readField('class', objectOf(ClassComponent.fromJson));
  set classField(ClassComponent? value) => writeField('class', value);

  /// 分類の参照
  List<ClassificationReference>? get classification => readField('classification', listOf(objectOf(ClassificationReference.fromJson)));
  set classification(List<ClassificationReference>? value) => writeField('classification', value);

  /// 空間の要素の種類と高さ
  SpatialComponent? get spatial => readField('spatial', objectOf(SpatialComponent.fromJson));
  set spatial(SpatialComponent? value) => writeField('spatial', value);

  /// 親と、参照される空間
  ParentComponent? get parent => readField('parent', objectOf(ParentComponent.fromJson));
  set parent(ParentComponent? value) => writeField('parent', value);

  /// 配置
  PlacementComponent? get placement => readField('placement', objectOf(PlacementComponent.fromJson));
  set placement(PlacementComponent? value) => writeField('placement', value);

  /// 型の要素の ID
  String? get typeOf => readField('type_of', asString);
  set typeOf(String? value) => writeField('type_of', value);

  /// プロパティのセット
  List<PropertySet>? get properties => readField('properties', listOf(objectOf(PropertySet.fromJson)));
  set properties(List<PropertySet>? value) => writeField('properties', value);

  /// 数量のセット
  List<QuantitySet>? get quantities => readField('quantities', listOf(objectOf(QuantitySet.fromJson)));
  set quantities(List<QuantitySet>? value) => writeField('quantities', value);

  /// 材料
  MaterialComponent? get material => readField('material', objectOf(MaterialComponent.fromJson));
  set material(MaterialComponent? value) => writeField('material', value);

  /// 建具の元の全幅と全高。開口や数量からの推定は含まない
  JoineryComponent? get joinery => readField('joinery', objectOf(JoineryComponent.fromJson));
  set joinery(JoineryComponent? value) => writeField('joinery', value);

  /// 開口と、埋める要素
  OpeningComponent? get opening => readField('opening', objectOf(OpeningComponent.fromJson));
  set opening(OpeningComponent? value) => writeField('opening', value);

  /// 属するグループの要素の ID
  List<String>? get group => readField('group', listOf(asString));
  set group(List<String>? value) => writeField('group', value);

  /// 接続
  List<ConnectionLink>? get connection => readField('connection', listOf(objectOf(ConnectionLink.fromJson)));
  set connection(List<ConnectionLink>? value) => writeField('connection', value);

  /// レベル
  LevelComponent? get level => readField('level', objectOf(LevelComponent.fromJson));
  set level(LevelComponent? value) => writeField('level', value);

  /// 区画
  ZoneComponent? get zone => readField('zone', objectOf(ZoneComponent.fromJson));
  set zone(ZoneComponent? value) => writeField('zone', value);

  /// 表示のスタイル
  StyleComponent? get style => readField('style', objectOf(StyleComponent.fromJson));
  set style(StyleComponent? value) => writeField('style', value);

  /// 索引から求める関係
  RelationsComponent? get relations => readField('relations', objectOf(RelationsComponent.fromJson));
  set relations(RelationsComponent? value) => writeField('relations', value);
}

/// 名前・説明・タグ・長い名前
class IdentityComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IdentityComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  IdentityComponent() : super({});

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 説明
  String? get description => readField('description', asString);
  set description(String? value) => writeField('description', value);

  /// タグ
  String? get tag => readField('tag', asString);
  set tag(String? value) => writeField('tag', value);

  /// 長い名前
  String? get longName => readField('long_name', asString);
  set longName(String? value) => writeField('long_name', value);
}

/// クラス
class ClassComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ClassComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  ClassComponent() : super({});

  /// IFC4 のクラスの名前。IFC2X3 の StandardCase などは、元の名前を original に残して直す
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// PredefinedType
  String? get predefined => readField('predefined', asString);
  set predefined(String? value) => writeField('predefined', value);

  /// ObjectType など、USERDEFINED のときの名前
  String? get userDefined => readField('user_defined', asString);
  set userDefined(String? value) => writeField('user_defined', value);

  /// ファイルに書いた実体の名前
  String? get original => readField('original', asString);
  set original(String? value) => writeField('original', value);
}

/// 分類の参照
class ClassificationReference extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ClassificationReference.fromJson(super.json);

  /// 空のオブジェクトから作る
  ClassificationReference() : super({});

  /// 分類の体系の名前
  String? get system => readField('system', asString);
  set system(String? value) => writeField('system', value);

  /// 体系のバージョン
  String? get edition => readField('edition', asString);
  set edition(String? value) => writeField('edition', value);

  /// 分類の記号
  String? get code => readField('code', asString);
  set code(String? value) => writeField('code', value);

  /// 分類の名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);
}

/// 単位とともに持つ値
class Measured extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Measured.fromJson(super.json);

  /// 空のオブジェクトから作る
  Measured() : super({});

  /// 値
  core.double? get value => readField('value', asDouble);
  set value(core.double? value) => writeField('value', value);

  /// 単位。解けなければ持たない
  BuildingUnit? get unit => readField('unit', objectOf(BuildingUnit.fromJson));
  set unit(BuildingUnit? value) => writeField('unit', value);
}

/// 空間の要素
class SpatialComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SpatialComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  SpatialComponent() : super({});

  /// site・building・storey・space・spatial_zone か、元の実体の名前
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 高さ
  Measured? get elevation => readField('elevation', objectOf(Measured.fromJson));
  set elevation(Measured? value) => writeField('elevation', value);

  /// 基準の高さ
  Measured? get refElevation => readField('ref_elevation', objectOf(Measured.fromJson));
  set refElevation(Measured? value) => writeField('ref_elevation', value);
}

/// 親
class ParentComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ParentComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  ParentComponent() : super({});

  /// 親の要素
  Membership? get ofField => readField('of', objectOf(Membership.fromJson));
  set ofField(Membership? value) => writeField('of', value);

  /// 参照される空間の要素の ID
  List<String>? get referencedIn => readField('referenced_in', listOf(asString));
  set referencedIn(List<String>? value) => writeField('referenced_in', value);
}

/// 親との関係
class Membership extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Membership.fromJson(super.json);

  /// 空のオブジェクトから作る
  Membership() : super({});

  /// 親の要素の ID
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 関係の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 入れ子の順。0 から
  core.int? get order => readField('order', asInt);
  set order(core.int? value) => writeField('order', value);
}

/// 配置
class PlacementComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PlacementComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  PlacementComponent() : super({});

  /// 配置の親の要素の ID
  String? get relativeTo => readField('relative_to', asString);
  set relativeTo(String? value) => writeField('relative_to', value);

  /// 親をたどった世界の変換。3 行 4 列（回転と移動）。解けなければ持たない
  List<List<core.double>>? get world => readField('world', listOf(listOf(asDouble)));
  set world(List<List<core.double>>? value) => writeField('world', value);

  /// 解けなかった配置の実体の名前
  String? get unsupported => readField('unsupported', asString);
  set unsupported(String? value) => writeField('unsupported', value);
}

/// 値
class BuildingValue extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingValue.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingValue() : super({});

  /// 値の型の名前（IfcLengthMeasure・IfcLabel など）
  String? get typeName => readField('type_name', asString);
  set typeName(String? value) => writeField('type_name', value);

  /// 値の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// real と、2^53 − 1 までの integer の値
  core.double? get number => readField('number', asDouble);
  set number(core.double? value) => writeField('number', value);

  /// text と enumeration の値と、2^53 − 1 を超える integer の値
  String? get text => readField('text', asString);
  set text(String? value) => writeField('text', value);

  /// boolean と logical の値。logical の不明は持たない
  core.bool? get boolean => readField('boolean', asBool);
  set boolean(core.bool? value) => writeField('boolean', value);

  /// element の要素の ID
  String? get element => readField('element', asString);
  set element(String? value) => writeField('element', value);

  /// list の値
  List<BuildingValue>? get items => readField('items', listOf(objectOf(BuildingValue.fromJson)));
  set items(List<BuildingValue>? value) => writeField('items', value);
}

/// プロパティのセット
class PropertySet extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PropertySet.fromJson(super.json);

  /// 空のオブジェクトから作る
  PropertySet() : super({});

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// プロパティ
  List<Property>? get properties => readField('properties', listOf(objectOf(Property.fromJson)));
  set properties(List<Property>? value) => writeField('properties', value);
}

/// プロパティ
class Property extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Property.fromJson(super.json);

  /// 空のオブジェクトから作る
  Property() : super({});

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 値。値がなければ持たない
  BuildingValue? get value => readField('value', objectOf(BuildingValue.fromJson));
  set value(BuildingValue? value) => writeField('value', value);

  /// 単位
  BuildingUnit? get unit => readField('unit', objectOf(BuildingUnit.fromJson));
  set unit(BuildingUnit? value) => writeField('unit', value);
}

/// 数量のセット
class QuantitySet extends Message {
  /// 読んだ JSON のオブジェクトを包む
  QuantitySet.fromJson(super.json);

  /// 空のオブジェクトから作る
  QuantitySet() : super({});

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 計測の方法
  String? get method => readField('method', asString);
  set method(String? value) => writeField('method', value);

  /// 数量
  List<Quantity>? get quantities => readField('quantities', listOf(objectOf(Quantity.fromJson)));
  set quantities(List<Quantity>? value) => writeField('quantities', value);
}

/// 数量
class Quantity extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Quantity.fromJson(super.json);

  /// 空のオブジェクトから作る
  Quantity() : super({});

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 値
  Measured? get value => readField('value', objectOf(Measured.fromJson));
  set value(Measured? value) => writeField('value', value);

  /// 式
  String? get formula => readField('formula', asString);
  set formula(String? value) => writeField('formula', value);
}

/// 材料
class MaterialComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MaterialComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  MaterialComponent() : super({});

  /// 材料の組み方
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 部分
  List<MaterialPart>? get parts => readField('parts', listOf(objectOf(MaterialPart.fromJson)));
  set parts(List<MaterialPart>? value) => writeField('parts', value);

  /// 層の向き
  String? get direction => readField('direction', asString);
  set direction(String? value) => writeField('direction', value);

  /// 層の基準からのずれ
  Measured? get offset => readField('offset', objectOf(Measured.fromJson));
  set offset(Measured? value) => writeField('offset', value);
}

/// 材料の部分
class MaterialPart extends Message {
  /// 読んだ JSON のオブジェクトを包む
  MaterialPart.fromJson(super.json);

  /// 空のオブジェクトから作る
  MaterialPart() : super({});

  /// 材料の要素の ID
  String? get material => readField('material', asString);
  set material(String? value) => writeField('material', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 厚さ
  Measured? get thickness => readField('thickness', objectOf(Measured.fromJson));
  set thickness(Measured? value) => writeField('thickness', value);
}

/// 建具の実例に直接記録された寸法。値を持たない属性は省く
class JoineryComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  JoineryComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  JoineryComponent() : super({});

  /// 建具自体の全幅。IFC の OverallWidth
  Measured? get overallWidth => readField('overall_width', objectOf(Measured.fromJson));
  set overallWidth(Measured? value) => writeField('overall_width', value);

  /// 建具自体の全高。IFC の OverallHeight
  Measured? get overallHeight => readField('overall_height', objectOf(Measured.fromJson));
  set overallHeight(Measured? value) => writeField('overall_height', value);
}

/// 開口
class OpeningComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  OpeningComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  OpeningComponent() : super({});

  /// この開口が穴を開ける要素の ID
  String? get voids => readField('voids', asString);
  set voids(String? value) => writeField('voids', value);

  /// この要素が埋める開口の要素の ID
  String? get fills => readField('fills', asString);
  set fills(String? value) => writeField('fills', value);
}

/// 接続
class ConnectionLink extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConnectionLink.fromJson(super.json);

  /// 空のオブジェクトから作る
  ConnectionLink() : super({});

  /// 関係の実体の名前
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 接続する要素の ID
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);
}

/// レベル
class LevelComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LevelComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  LevelComponent() : super({});

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 高さ
  Measured? get height => readField('height', objectOf(Measured.fromJson));
  set height(Measured? value) => writeField('height', value);

  /// 基準
  String? get datum => readField('datum', asString);
  set datum(String? value) => writeField('datum', value);
}

/// 区画
class ZoneComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ZoneComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  ZoneComponent() : super({});

  /// 区画の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 構成する要素の ID
  List<String>? get members => readField('members', listOf(asString));
  set members(List<String>? value) => writeField('members', value);
}

/// 表示のスタイル
class StyleComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  StyleComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  StyleComponent() : super({});

  /// 面の色の RGB（0 から 1）
  List<core.double>? get surfaceColor => readField('surface_color', listOf(asDouble));
  set surfaceColor(List<core.double>? value) => writeField('surface_color', value);

  /// 透明度（0 から 1）
  core.double? get transparency => readField('transparency', asDouble);
  set transparency(core.double? value) => writeField('transparency', value);

  /// 線の色の RGB（0 から 1）
  List<core.double>? get curveColor => readField('curve_color', listOf(asDouble));
  set curveColor(List<core.double>? value) => writeField('curve_color', value);

  /// 提示のレイヤの名前
  List<String>? get layers => readField('layers', listOf(asString));
  set layers(List<String>? value) => writeField('layers', value);
}

/// 索引から求める関係。要素の ID の列
class RelationsComponent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RelationsComponent.fromJson(super.json);

  /// 空のオブジェクトから作る
  RelationsComponent() : super({});

  /// 子（集約・入れ子・包含）
  List<String>? get children => readField('children', listOf(asString));
  set children(List<String>? value) => writeField('children', value);

  /// 参照する要素
  List<String>? get referenced => readField('referenced', listOf(asString));
  set referenced(List<String>? value) => writeField('referenced', value);

  /// 型の実例
  List<String>? get instances => readField('instances', listOf(asString));
  set instances(List<String>? value) => writeField('instances', value);

  /// 開口
  List<String>? get openings => readField('openings', listOf(asString));
  set openings(List<String>? value) => writeField('openings', value);

  /// 開口を埋める要素
  List<String>? get fillings => readField('fillings', listOf(asString));
  set fillings(List<String>? value) => writeField('fillings', value);

  /// グループ・システム・区画の構成員
  List<String>? get members => readField('members', listOf(asString));
  set members(List<String>? value) => writeField('members', value);
}

/// 親との関係の種類。集約・入れ子・包含
///
/// 列挙の値。知らない値も受け取る
abstract final class ParentKind {
  static const aggregates = 'aggregates';
  static const nests = 'nests';
  static const contained = 'contained';
}

/// 値の種類
///
/// 列挙の値。知らない値も受け取る
abstract final class BuildingValueKind {
  static const real = 'real';
  static const integer = 'integer';
  static const text = 'text';
  static const boolean = 'boolean';
  static const logical = 'logical';
  static const enumeration = 'enumeration';
  static const element = 'element';
  static const list = 'list';
}

/// 数量の種類
///
/// 列挙の値。知らない値も受け取る
abstract final class QuantityKind {
  static const length = 'length';
  static const area = 'area';
  static const volume = 'volume';
  static const count = 'count';
  static const weight = 'weight';
  static const time = 'time';
}

/// 材料の組み方。単一・層・断面・構成・一覧
///
/// 列挙の値。知らない値も受け取る
abstract final class MaterialKind {
  static const single = 'single';
  static const layers = 'layers';
  static const profiles = 'profiles';
  static const constituents = 'constituents';
  static const list = 'list';
}

/// 仕様の結果。unknown は確かめられない facet があった
///
/// 列挙の値。知らない値も受け取る
abstract final class IdsStatus {
  static const passed = 'passed';
  static const failed = 'failed';
  static const unknown = 'unknown';
}

/// 開いた IFC に、変更の一覧を当てて書き戻す（決定 0207・0244）。要素は、building_elements が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイル（ifcZIP で開いたものは ifcZIP）を blobs の file で返す。名前を挙げないインスタンスは、元のバイト列のまま書く。マイナーバージョン 58 から
class BuildingApplyRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingApplyRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingApplyRequest() : super({'op': 'building_apply'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で IFC を開いたもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 読んだときの文書のハッシュ（building の hash）。書けば、今の文書と同じときだけ当てる
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 変更。並べた順に当てる
  List<BuildingEdit>? get edits => readField('edits', listOf(objectOf(BuildingEdit.fromJson)));
  set edits(List<BuildingEdit>? value) => writeField('edits', value);
}

/// 建物の要素 1 つへの変更。kind の値で形が決まる。add_space・add_element・remove・place・contain・type・void・fill はマイナーバージョン 96 から、add_wall はマイナーバージョン 97 から、add_slab・add_column・add_beam・add_opening・add_door・add_window はマイナーバージョン 98 から
class BuildingEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingEdit.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingEdit() : super({});

  /// kind が property の形なら、そのビュー。違えば null
  BuildingPropertyEdit? get asBuildingPropertyEdit => taggedView('kind', 'property', BuildingPropertyEdit.fromJson);

  /// kind が attribute の形なら、そのビュー。違えば null
  BuildingAttributeEdit? get asBuildingAttributeEdit => taggedView('kind', 'attribute', BuildingAttributeEdit.fromJson);

  /// kind が add_space の形なら、そのビュー。違えば null
  BuildingAddSpaceEdit? get asBuildingAddSpaceEdit => taggedView('kind', 'add_space', BuildingAddSpaceEdit.fromJson);

  /// kind が add_element の形なら、そのビュー。違えば null
  BuildingAddElementEdit? get asBuildingAddElementEdit => taggedView('kind', 'add_element', BuildingAddElementEdit.fromJson);

  /// kind が remove の形なら、そのビュー。違えば null
  BuildingRemoveEdit? get asBuildingRemoveEdit => taggedView('kind', 'remove', BuildingRemoveEdit.fromJson);

  /// kind が place の形なら、そのビュー。違えば null
  BuildingPlaceEdit? get asBuildingPlaceEdit => taggedView('kind', 'place', BuildingPlaceEdit.fromJson);

  /// kind が contain の形なら、そのビュー。違えば null
  BuildingContainEdit? get asBuildingContainEdit => taggedView('kind', 'contain', BuildingContainEdit.fromJson);

  /// kind が type の形なら、そのビュー。違えば null
  BuildingTypeEdit? get asBuildingTypeEdit => taggedView('kind', 'type', BuildingTypeEdit.fromJson);

  /// kind が void の形なら、そのビュー。違えば null
  BuildingVoidEdit? get asBuildingVoidEdit => taggedView('kind', 'void', BuildingVoidEdit.fromJson);

  /// kind が fill の形なら、そのビュー。違えば null
  BuildingFillEdit? get asBuildingFillEdit => taggedView('kind', 'fill', BuildingFillEdit.fromJson);

  /// kind が add_wall の形なら、そのビュー。違えば null
  BuildingAddWallEdit? get asBuildingAddWallEdit => taggedView('kind', 'add_wall', BuildingAddWallEdit.fromJson);

  /// kind が add_slab の形なら、そのビュー。違えば null
  BuildingAddSlabEdit? get asBuildingAddSlabEdit => taggedView('kind', 'add_slab', BuildingAddSlabEdit.fromJson);

  /// kind が add_column の形なら、そのビュー。違えば null
  BuildingAddColumnEdit? get asBuildingAddColumnEdit => taggedView('kind', 'add_column', BuildingAddColumnEdit.fromJson);

  /// kind が add_beam の形なら、そのビュー。違えば null
  BuildingAddBeamEdit? get asBuildingAddBeamEdit => taggedView('kind', 'add_beam', BuildingAddBeamEdit.fromJson);

  /// kind が add_opening の形なら、そのビュー。違えば null
  BuildingAddOpeningEdit? get asBuildingAddOpeningEdit => taggedView('kind', 'add_opening', BuildingAddOpeningEdit.fromJson);

  /// kind が add_door の形なら、そのビュー。違えば null
  BuildingAddDoorEdit? get asBuildingAddDoorEdit => taggedView('kind', 'add_door', BuildingAddDoorEdit.fromJson);

  /// kind が add_window の形なら、そのビュー。違えば null
  BuildingAddWindowEdit? get asBuildingAddWindowEdit => taggedView('kind', 'add_window', BuildingAddWindowEdit.fromJson);

  /// kind が add_grid の形なら、そのビュー。違えば null
  BuildingAddGridEdit? get asBuildingAddGridEdit => taggedView('kind', 'add_grid', BuildingAddGridEdit.fromJson);

  /// kind が elevation の形なら、そのビュー。違えば null
  BuildingElevationEdit? get asBuildingElevationEdit => taggedView('kind', 'elevation', BuildingElevationEdit.fromJson);

  /// kind が wall_shape の形なら、そのビュー。違えば null
  BuildingWallShapeEdit? get asBuildingWallShapeEdit => taggedView('kind', 'wall_shape', BuildingWallShapeEdit.fromJson);

  /// kind が opening_shape の形なら、そのビュー。違えば null
  BuildingOpeningShapeEdit? get asBuildingOpeningShapeEdit => taggedView('kind', 'opening_shape', BuildingOpeningShapeEdit.fromJson);

  /// kind が add_type の形なら、そのビュー。違えば null
  BuildingAddTypeEdit? get asBuildingAddTypeEdit => taggedView('kind', 'add_type', BuildingAddTypeEdit.fromJson);

  /// kind が quantity の形なら、そのビュー。違えば null
  BuildingQuantityEdit? get asBuildingQuantityEdit => taggedView('kind', 'quantity', BuildingQuantityEdit.fromJson);

  /// kind が add_unit の形なら、そのビュー。違えば null
  BuildingAddUnitEdit? get asBuildingAddUnitEdit => taggedView('kind', 'add_unit', BuildingAddUnitEdit.fromJson);

  /// kind が material の形なら、そのビュー。違えば null
  BuildingMaterialEdit? get asBuildingMaterialEdit => taggedView('kind', 'material', BuildingMaterialEdit.fromJson);

  /// kind が shape の形なら、そのビュー。違えば null
  BuildingShapeEdit? get asBuildingShapeEdit => taggedView('kind', 'shape', BuildingShapeEdit.fromJson);
}

/// 要素のプロパティのセットの、プロパティ（IfcPropertySingleValue）の値を変える。セットかプロパティがなければ足す。ほかの要素と共有するセットは、写しを作って、この要素だけに付け替える。型の要素では、型のセット（HasPropertySets）を変える
class BuildingPropertyEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingPropertyEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingPropertyEdit() : super({'kind': 'property'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 変える要素の ID。building_elements が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。building_elements が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// プロパティのセットの名前（Pset_WallCommon など）
  String? get setField => readField('set', asString);
  set setField(String? value) => writeField('set', value);

  /// プロパティの名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 値。type_name は要る。kind は real・integer・text・boolean・logical のどれか
  BuildingValue? get value => readField('value', objectOf(BuildingValue.fromJson));
  set value(BuildingValue? value) => writeField('value', value);
}

/// 要素（IfcRoot）の名前か説明を変える
class BuildingAttributeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAttributeEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAttributeEdit() : super({'kind': 'attribute'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 変える要素の ID。building_elements が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。building_elements が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 変える属性
  String? get attribute => readField('attribute', asString);
  set attribute(String? value) => writeField('attribute', value);

  /// 値。書かなければ、値をなくす
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);
}

/// 変えられる IfcRoot の属性。name は Name、description は Description
///
/// 列挙の値。知らない値も受け取る
abstract final class BuildingAttribute {
  static const name = 'name';
  static const description = 'description';
}

/// 変更が指す要素。id と hash を書くか、added だけを書く
class BuildingRef extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingRef.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingRef() : super({});

  /// 要素の ID。building_elements が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。building_elements が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 同じ要求の edits の、前にある要素を足す変更（add_ で始まる種類）の位置（0 から）。その変更で足した要素を指す
  core.int? get added => readField('added', asInt);
  set added(core.int? value) => writeField('added', value);
}

/// 要素の配置（IfcLocalPlacement）。relative_to の要素の配置からの相対で、書かなければプロジェクトの空間に置く
class BuildingPlacementChange extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingPlacementChange.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingPlacementChange() : super({});

  /// 配置の親の要素
  BuildingRef? get relativeTo => readField('relative_to', objectOf(BuildingRef.fromJson));
  set relativeTo(BuildingRef? value) => writeField('relative_to', value);

  /// 局所の変換。3 行 4 列（回転と移動。building_elements の placement の world と同じ形）。回転は正規直交で行列式が 1、移動は文書の長さの単位
  List<List<core.double>>? get matrix => readField('matrix', listOf(listOf(asDouble)));
  set matrix(List<List<core.double>>? value) => writeField('matrix', value);
}

/// 足す空間の種類。site は IfcSite、building は IfcBuilding、storey は IfcBuildingStorey
///
/// 列挙の値。知らない値も受け取る
abstract final class BuildingSpaceKind {
  static const site = 'site';
  static const building = 'building';
  static const storey = 'storey';
}

/// 敷地・建物・階を足し、IfcRelAggregates で親に結ぶ。結果の id は足した空間の ID。マイナーバージョン 96 から
class BuildingAddSpaceEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddSpaceEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddSpaceEdit() : super({'kind': 'add_space'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 空間の種類
  String? get space => readField('space', asString);
  set space(String? value) => writeField('space', value);

  /// 親。敷地はプロジェクトか敷地、建物はプロジェクト・敷地・建物、階は建物か階。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get parent => readField('parent', objectOf(BuildingRef.fromJson));
  set parent(BuildingRef? value) => writeField('parent', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 配置。書かなければ、親の配置に重ねる
  BuildingPlacementChange? get placement => readField('placement', objectOf(BuildingPlacementChange.fromJson));
  set placement(BuildingPlacementChange? value) => writeField('placement', value);

  /// 高さ。敷地は RefElevation、建物は ElevationOfRefHeight、階は Elevation に書く。文書の長さの単位
  core.double? get elevation => readField('elevation', asDouble);
  set elevation(core.double? value) => writeField('elevation', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 要素を足し、container があれば IfcRelContainedInSpatialStructure で空間に結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・屋根・階段・斜路・基礎・杭・扉・窓・開口・代理要素（IfcBuildingElementProxy）。結果の id は足した要素の ID。マイナーバージョン 96 から
class BuildingAddElementEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddElementEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddElementEdit() : super({'kind': 'add_element'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// IFC の実体の名前（IfcWall など）。文書のバージョンにあるもの
  String? get classField => readField('class', asString);
  set classField(String? value) => writeField('class', value);

  /// PredefinedType の列挙の値（SOLIDWALL など）。バージョンが持たない実体では書けない
  String? get predefined => readField('predefined', asString);
  set predefined(String? value) => writeField('predefined', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 包含する空間（敷地・建物・階・室）。開口には書けない。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get container => readField('container', objectOf(BuildingRef.fromJson));
  set container(BuildingRef? value) => writeField('container', value);

  /// 配置。書かなければ、空間の配置に重ねる（空間もなければ原点）
  BuildingPlacementChange? get placement => readField('placement', objectOf(BuildingPlacementChange.fromJson));
  set placement(BuildingPlacementChange? value) => writeField('placement', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 要素を除く。関係からも外し、空になった関係を除く。壁の開口は一緒に除く。子のある空間と、関係のほかから指される要素は除けない。結果は id を持たない。マイナーバージョン 96 から
class BuildingRemoveEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingRemoveEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingRemoveEdit() : super({'kind': 'remove'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 除く要素の ID。building_elements が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。building_elements が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);
}

/// 要素の配置を置き直す。ほかの要素と共有しない配置はその場で書き換え、共有する配置は新しく作る。配置の親をたどって輪になる配置は断る。マイナーバージョン 96 から
class BuildingPlaceEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingPlaceEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingPlaceEdit() : super({'kind': 'place'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 置き直す要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get element => readField('element', objectOf(BuildingRef.fromJson));
  set element(BuildingRef? value) => writeField('element', value);

  /// 新しい配置
  BuildingPlacementChange? get placement => readField('placement', objectOf(BuildingPlacementChange.fromJson));
  set placement(BuildingPlacementChange? value) => writeField('placement', value);
}

/// 要素を空間に移す。空間は IfcRelAggregates、要素は IfcRelContainedInSpatialStructure で結び、前の関係から外す。マイナーバージョン 96 から
class BuildingContainEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingContainEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingContainEdit() : super({'kind': 'contain'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 移す要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get element => readField('element', objectOf(BuildingRef.fromJson));
  set element(BuildingRef? value) => writeField('element', value);

  /// 移す先の空間。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get space => readField('space', objectOf(BuildingRef.fromJson));
  set space(BuildingRef? value) => writeField('space', value);
}

/// 要素の型を IfcRelDefinesByType で付け替える。element_type を書かなければ、型を外す。マイナーバージョン 96 から
class BuildingTypeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingTypeEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingTypeEdit() : super({'kind': 'type'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 型を変える要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get element => readField('element', objectOf(BuildingRef.fromJson));
  set element(BuildingRef? value) => writeField('element', value);

  /// 型の要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get elementType => readField('element_type', objectOf(BuildingRef.fromJson));
  set elementType(BuildingRef? value) => writeField('element_type', value);
}

/// 開口（IfcOpeningElement）を要素に IfcRelVoidsElement で結ぶ。1 つの開口は 1 つの要素だけに結ぶ。マイナーバージョン 96 から
class BuildingVoidEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingVoidEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingVoidEdit() : super({'kind': 'void'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 開口を持つ要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get host => readField('host', objectOf(BuildingRef.fromJson));
  set host(BuildingRef? value) => writeField('host', value);

  /// 開口。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get opening => readField('opening', objectOf(BuildingRef.fromJson));
  set opening(BuildingRef? value) => writeField('opening', value);
}

/// 材料の層
class BuildingWallLayer extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingWallLayer.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingWallLayer() : super({});

  /// 材料の名前（IfcMaterial の Name）。同じ要素の中で同じ名前の層は、同じ材料を指す
  String? get material => readField('material', asString);
  set material(String? value) => writeField('material', value);

  /// 層の厚さ。文書の長さの単位で、正の値
  core.double? get thickness => readField('thickness', asDouble);
  set thickness(core.double? value) => writeField('thickness', value);
}

/// 芯の 2 点・高さ・材料の層か型で壁を足し、空間に IfcRelContainedInSpatialStructure で結ぶ。配置（芯の始点を原点、芯の向きを x 軸）・Axis と Body の表現・IfcMaterialLayerSetUsage はコアが書く。layers を省けば element_type の IfcMaterialLayerSet を使う。IFC2X3 は IfcWallStandardCase、IFC4 と IFC4X3 は IfcWall。結果の id は足した壁の ID。マイナーバージョン 97 から（element_type は 110 から）
class BuildingAddWallEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddWallEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddWallEdit() : super({'kind': 'add_wall'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 所属する空間（階など）。壁はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get storey => readField('storey', objectOf(BuildingRef.fromJson));
  set storey(BuildingRef? value) => writeField('storey', value);

  /// 芯の始点。空間の配置の [x, y] で、文書の長さの単位
  List<core.double>? get start => readField('start', listOf(asDouble));
  set start(List<core.double>? value) => writeField('start', value);

  /// 芯の終点。始点と違う [x, y]
  List<core.double>? get end => readField('end', listOf(asDouble));
  set end(List<core.double>? value) => writeField('end', value);

  /// 高さ。文書の長さの単位で、正の値
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 材料の層。始点から終点を見て、芯の右から左へ並べる。省くか空なら element_type の材料の層を使う
  List<BuildingWallLayer>? get layers => readField('layers', listOf(objectOf(BuildingWallLayer.fromJson)));
  set layers(List<BuildingWallLayer>? value) => writeField('layers', value);

  /// 芯から最初の層の面までのずれ（OffsetFromReferenceLine、左が正）。書かなければ厚さの合計の半分を負にして、芯を厚さの中央にする
  core.double? get offset => readField('offset', asDouble);
  set offset(core.double? value) => writeField('offset', value);

  /// 結ぶ壁の型（IfcWallType）。マイナーバージョン 110 から
  BuildingRef? get elementType => readField('element_type', objectOf(BuildingRef.fromJson));
  set elementType(BuildingRef? value) => writeField('element_type', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 矩形の断面
class BuildingSection extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingSection.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingSection() : super({});

  /// 幅。文書の長さの単位で、正の値
  core.double? get width => readField('width', asDouble);
  set width(core.double? value) => writeField('width', value);

  /// せい。文書の長さの単位で、正の値
  core.double? get depth => readField('depth', asDouble);
  set depth(core.double? value) => writeField('depth', value);
}

/// 外形・材料の層か型で床を足し、空間に包含で結ぶ。上の面を配置の原点にし、層の厚さの合計だけ下へ押し出す Body と、IfcSlab（型が PredefinedType を持たなければ FLOOR）、IfcMaterialLayerSetUsage（AXIS3・NEGATIVE）をコアが書く。layers を省けば element_type の IfcMaterialLayerSet を使う。結果の id は足した床の ID。マイナーバージョン 98 から（element_type は 110 から）
class BuildingAddSlabEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddSlabEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddSlabEdit() : super({'kind': 'add_slab'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 所属する空間（階など）。床はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get storey => readField('storey', objectOf(BuildingRef.fromJson));
  set storey(BuildingRef? value) => writeField('storey', value);

  /// 外形の頂点。空間の配置の [x, y] の並びで、3 つ以上。最後の点が最初の点と同じなら、その点は除く
  List<List<core.double>>? get outline => readField('outline', listOf(listOf(asDouble)));
  set outline(List<List<core.double>>? value) => writeField('outline', value);

  /// 材料の層。上から下へ並べる。省くか空なら element_type の材料の層を使う
  List<BuildingWallLayer>? get layers => readField('layers', listOf(objectOf(BuildingWallLayer.fromJson)));
  set layers(List<BuildingWallLayer>? value) => writeField('layers', value);

  /// 上の面の高さ（空間の配置の z）。書かなければ 0
  core.double? get elevation => readField('elevation', asDouble);
  set elevation(core.double? value) => writeField('elevation', value);

  /// 結ぶ床の型（IfcSlabType）。マイナーバージョン 110 から
  BuildingRef? get elementType => readField('element_type', objectOf(BuildingRef.fromJson));
  set elementType(BuildingRef? value) => writeField('element_type', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 位置・高さ・矩形の断面で柱を足し、空間に包含で結ぶ。断面の中心を配置の原点にして上へ押し出す Body と IfcColumn をコアが書く。結果の id は足した柱の ID。マイナーバージョン 98 から（element_type は 110 から）
class BuildingAddColumnEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddColumnEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddColumnEdit() : super({'kind': 'add_column'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 所属する空間（階など）。柱はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get storey => readField('storey', objectOf(BuildingRef.fromJson));
  set storey(BuildingRef? value) => writeField('storey', value);

  /// 断面の中心。空間の配置の [x, y]
  List<core.double>? get position => readField('position', listOf(asDouble));
  set position(List<core.double>? value) => writeField('position', value);

  /// 柱の脚の高さ（空間の配置の z）。書かなければ 0
  core.double? get baseField => readField('base', asDouble);
  set baseField(core.double? value) => writeField('base', value);

  /// 高さ。文書の長さの単位で、正の値
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 断面。幅は柱の x 軸、せいは y 軸の向き
  BuildingSection? get section => readField('section', objectOf(BuildingSection.fromJson));
  set section(BuildingSection? value) => writeField('section', value);

  /// 柱の x 軸の向き。空間の配置の [x, y]。書かなければ空間の x 軸
  List<core.double>? get direction => readField('direction', listOf(asDouble));
  set direction(List<core.double>? value) => writeField('direction', value);

  /// 材料の名前（IfcMaterial の Name）。書けば IfcRelAssociatesMaterial で結ぶ
  String? get material => readField('material', asString);
  set material(String? value) => writeField('material', value);

  /// 結ぶ柱の型（IfcColumnType）。マイナーバージョン 110 から
  BuildingRef? get elementType => readField('element_type', objectOf(BuildingRef.fromJson));
  set elementType(BuildingRef? value) => writeField('element_type', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 天端の芯の 2 点・矩形の断面で梁を足し、空間に包含で結ぶ。始点を配置の原点、芯の向きを x 軸、水平で芯の左を y 軸にし、断面を芯に沿って押し出す Body と IfcBeam をコアが書く。鉛直の芯は断る。結果の id は足した梁の ID。マイナーバージョン 98 から（element_type は 110 から）
class BuildingAddBeamEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddBeamEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddBeamEdit() : super({'kind': 'add_beam'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 所属する空間（階など）。梁はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get storey => readField('storey', objectOf(BuildingRef.fromJson));
  set storey(BuildingRef? value) => writeField('storey', value);

  /// 天端の芯の始点。空間の配置の [x, y, z]
  List<core.double>? get start => readField('start', listOf(asDouble));
  set start(List<core.double>? value) => writeField('start', value);

  /// 天端の芯の終点。始点と x か y が違う [x, y, z]
  List<core.double>? get end => readField('end', listOf(asDouble));
  set end(List<core.double>? value) => writeField('end', value);

  /// 断面。幅は芯の左右、せいは天端から下へ
  BuildingSection? get section => readField('section', objectOf(BuildingSection.fromJson));
  set section(BuildingSection? value) => writeField('section', value);

  /// 材料の名前（IfcMaterial の Name）。書けば IfcRelAssociatesMaterial で結ぶ
  String? get material => readField('material', asString);
  set material(String? value) => writeField('material', value);

  /// 結ぶ梁の型（IfcBeamType）。マイナーバージョン 110 から
  BuildingRef? get elementType => readField('element_type', objectOf(BuildingRef.fromJson));
  set elementType(BuildingRef? value) => writeField('element_type', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 壁の芯に沿う位置・下端・矩形の大きさで開口を足し、IfcRelVoidsElement で壁に穴をあける。開口は壁の配置からの相対に置き、壁の材料の層の使い方（AXIS2）の厚さだけ押し出す Body と IfcOpeningElement をコアが書く。空間には所属させない。層の使い方を持たない要素は断る。結果の id は足した開口の ID。マイナーバージョン 98 から
class BuildingAddOpeningEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddOpeningEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddOpeningEdit() : super({'kind': 'add_opening'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 穴をあける壁。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get host => readField('host', objectOf(BuildingRef.fromJson));
  set host(BuildingRef? value) => writeField('host', value);

  /// 壁の配置の x 軸に沿う、壁の原点から開口の端までの距離
  core.double? get distance => readField('distance', asDouble);
  set distance(core.double? value) => writeField('distance', value);

  /// 壁の下端から開口の下端までの高さ
  core.double? get sill => readField('sill', asDouble);
  set sill(core.double? value) => writeField('sill', value);

  /// 幅。文書の長さの単位で、正の値
  core.double? get width => readField('width', asDouble);
  set width(core.double? value) => writeField('width', value);

  /// 高さ。文書の長さの単位で、正の値
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 開口を扉で埋める。扉は開口の配置に置き、壁の層の中央に厚さの 4 分の 1 の板を押し出す Body と、IfcDoor（IFC4 と IFC4X3 で型が PredefinedType を持たなければ DOOR）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。もう埋まった開口は断る。結果の id は足した扉の ID。マイナーバージョン 98 から（Body と element_type は 110 から）
class BuildingAddDoorEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddDoorEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddDoorEdit() : super({'kind': 'add_door'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 埋める開口（IfcOpeningElement）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get opening => readField('opening', objectOf(BuildingRef.fromJson));
  set opening(BuildingRef? value) => writeField('opening', value);

  /// 全体の幅（OverallWidth）。文書の長さの単位で、正の値。書かなければ Body は開口の幅を使う
  core.double? get width => readField('width', asDouble);
  set width(core.double? value) => writeField('width', value);

  /// 全体の高さ（OverallHeight）。文書の長さの単位で、正の値。書かなければ Body は開口の高さを使う
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 結ぶ扉の型（IFC4 からの IfcDoorType、IFC2X3 の IfcDoorStyle）。マイナーバージョン 110 から
  BuildingRef? get elementType => readField('element_type', objectOf(BuildingRef.fromJson));
  set elementType(BuildingRef? value) => writeField('element_type', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 開口を窓で埋める。窓は開口の配置に置き、壁の層の中央に厚さの 4 分の 1 の板を押し出す Body と、IfcWindow（IFC4 と IFC4X3 で型が PredefinedType を持たなければ WINDOW）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。もう埋まった開口は断る。結果の id は足した窓の ID。マイナーバージョン 98 から（Body と element_type は 110 から）
class BuildingAddWindowEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddWindowEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddWindowEdit() : super({'kind': 'add_window'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 埋める開口（IfcOpeningElement）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get opening => readField('opening', objectOf(BuildingRef.fromJson));
  set opening(BuildingRef? value) => writeField('opening', value);

  /// 全体の幅（OverallWidth）。文書の長さの単位で、正の値。書かなければ Body は開口の幅を使う
  core.double? get width => readField('width', asDouble);
  set width(core.double? value) => writeField('width', value);

  /// 全体の高さ（OverallHeight）。文書の長さの単位で、正の値。書かなければ Body は開口の高さを使う
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 結ぶ窓の型（IFC4 からの IfcWindowType、IFC2X3 の IfcWindowStyle）。マイナーバージョン 110 から
  BuildingRef? get elementType => readField('element_type', objectOf(BuildingRef.fromJson));
  set elementType(BuildingRef? value) => writeField('element_type', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 通り芯の軸。2 点の折れ線の IfcGridAxis に書く。マイナーバージョン 101 から
class BuildingGridAxis extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingGridAxis.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingGridAxis() : super({});

  /// 軸の名前（AxisTag）。同じ通り芯の中で重ならない
  String? get tag => readField('tag', asString);
  set tag(String? value) => writeField('tag', value);

  /// 始点。通り芯の配置の [x, y]
  List<core.double>? get start => readField('start', listOf(asDouble));
  set start(List<core.double>? value) => writeField('start', value);

  /// 終点。始点と違う [x, y]
  List<core.double>? get end => readField('end', listOf(asDouble));
  set end(List<core.double>? value) => writeField('end', value);
}

/// U と V の軸で通り芯（IfcGrid）を足し、空間に包含で結ぶ。通り芯は空間の配置からの相対に置く。U か V が空・同じ名前の軸・始点と終点が同じ軸は断る。結果の id は足した通り芯の ID。マイナーバージョン 101 から
class BuildingAddGridEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddGridEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddGridEdit() : super({'kind': 'add_grid'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 所属する空間（階など）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get storey => readField('storey', objectOf(BuildingRef.fromJson));
  set storey(BuildingRef? value) => writeField('storey', value);

  /// U の軸。1 つ以上
  List<BuildingGridAxis>? get u => readField('u', listOf(objectOf(BuildingGridAxis.fromJson)));
  set u(List<BuildingGridAxis>? value) => writeField('u', value);

  /// V の軸。1 つ以上
  List<BuildingGridAxis>? get v => readField('v', listOf(objectOf(BuildingGridAxis.fromJson)));
  set v(List<BuildingGridAxis>? value) => writeField('v', value);

  /// 高さ（空間の配置の z）。書かなければ 0
  core.double? get elevation => readField('elevation', asDouble);
  set elevation(core.double? value) => writeField('elevation', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 階の高さ（IfcBuildingStorey の Elevation）を直し、階の配置の z を高さの差だけ動かす。今の高さがなければ、配置の z を今の高さとみなす。階に属する要素は配置の相対で付いてくる。結果の id は直した階の ID。マイナーバージョン 103 から
class BuildingElevationEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingElevationEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingElevationEdit() : super({'kind': 'elevation'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 直す階。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get storey => readField('storey', objectOf(BuildingRef.fromJson));
  set storey(BuildingRef? value) => writeField('storey', value);

  /// 新しい高さ。文書の長さの単位
  core.double? get elevation => readField('elevation', asDouble);
  set elevation(core.double? value) => writeField('elevation', value);
}

/// 壁の芯・高さ・材料の層か型を、add_wall と同じ値で直す。配置・表現・材料の層の使い方を書き直し、同じ名前の層は元の IfcMaterial を使う。開口と建具は壁の配置からの相対のまま付いてきて、開口と建具の Body の押し出しを新しい層の範囲に合わせる。合わせられない開口があれば断る。結果の id は直した壁の ID。マイナーバージョン 103 から（element_type は 110 から）
class BuildingWallShapeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingWallShapeEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingWallShapeEdit() : super({'kind': 'wall_shape'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 直す壁（IfcWall か IfcWallStandardCase）。材料の層の使い方（AXIS2）を持つもの。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get wall => readField('wall', objectOf(BuildingRef.fromJson));
  set wall(BuildingRef? value) => writeField('wall', value);

  /// 芯の始点。壁の配置の親の配置の [x, y]
  List<core.double>? get start => readField('start', listOf(asDouble));
  set start(List<core.double>? value) => writeField('start', value);

  /// 芯の終点。始点と違う [x, y]
  List<core.double>? get end => readField('end', listOf(asDouble));
  set end(List<core.double>? value) => writeField('end', value);

  /// 高さ。文書の長さの単位で、正の値
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 材料の層。始点から終点を見て、芯の右から左へ並べる。省くか空なら element_type（なければ今の壁の型）の材料の層を使う
  List<BuildingWallLayer>? get layers => readField('layers', listOf(objectOf(BuildingWallLayer.fromJson)));
  set layers(List<BuildingWallLayer>? value) => writeField('layers', value);

  /// 芯から最初の層の面までのずれ（左が正）。書かなければ厚さの合計の半分を負にして、芯を壁の厚さの中央にする
  core.double? get offset => readField('offset', asDouble);
  set offset(core.double? value) => writeField('offset', value);

  /// 付け替える壁の型（IfcWallType）。マイナーバージョン 110 から
  BuildingRef? get elementType => readField('element_type', objectOf(BuildingRef.fromJson));
  set elementType(BuildingRef? value) => writeField('element_type', value);
}

/// 開口の位置と大きさを、add_opening と同じ値で直す。配置と Body を書き直し、埋める扉と窓の Body と全体の高さ・幅（OverallHeight・OverallWidth）を合わせる。結果の id は直した開口の ID。マイナーバージョン 103 から
class BuildingOpeningShapeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingOpeningShapeEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingOpeningShapeEdit() : super({'kind': 'opening_shape'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 直す開口（IfcOpeningElement）。材料の層の使い方を持つ要素に穴をあけるもの。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get opening => readField('opening', objectOf(BuildingRef.fromJson));
  set opening(BuildingRef? value) => writeField('opening', value);

  /// 穴をあけた要素の配置の x 軸に沿う、要素の原点から開口の端までの距離
  core.double? get distance => readField('distance', asDouble);
  set distance(core.double? value) => writeField('distance', value);

  /// 要素の下端から開口の下端までの高さ
  core.double? get sill => readField('sill', asDouble);
  set sill(core.double? value) => writeField('sill', value);

  /// 幅。文書の長さの単位で、正の値
  core.double? get width => readField('width', asDouble);
  set width(core.double? value) => writeField('width', value);

  /// 高さ。文書の長さの単位で、正の値
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);
}

/// 型を足す。配置と表現は持たず、要素とは type で結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・代理要素の型と、IFC4 からの屋根・階段・斜路・基礎・杭・扉・窓の型。PredefinedType を書かなければ NOTDEFINED を書き、扉と窓の型の OperationType・PartitioningType も NOTDEFINED にする。結果の id は足した型の ID。マイナーバージョン 106 から
class BuildingAddTypeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddTypeEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddTypeEdit() : super({'kind': 'add_type'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// IFC の型の実体の名前（IfcWallType など）。文書のバージョンにあるもの
  String? get classField => readField('class', asString);
  set classField(String? value) => writeField('class', value);

  /// PredefinedType の列挙の値（SOLIDWALL など）
  String? get predefined => readField('predefined', asString);
  set predefined(String? value) => writeField('predefined', value);

  /// 利用者が決めた種類（ElementType）
  String? get elementType => readField('element_type', asString);
  set elementType(String? value) => writeField('element_type', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
  String? get globalId => readField('global_id', asString);
  set globalId(String? value) => writeField('global_id', value);
}

/// 要素の数量のセット（IfcElementQuantity）の数量の値を変える。セットか数量がなければ足す。ほかの要素と共有するセットは、写しを作って、この要素だけに付け替える。型の要素では、型のセット（HasPropertySets）を変える。単位は書かず、文書の単位の割り当てに従う。結果の id は要素の ID。マイナーバージョン 106 から
class BuildingQuantityEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingQuantityEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingQuantityEdit() : super({'kind': 'quantity'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 変える要素の ID。building_elements が返したもの
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 読んだときの要素のハッシュ。building_elements が返したもの
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 数量のセットの名前（Qto_WallBaseQuantities など）
  String? get setField => readField('set', asString);
  set setField(String? value) => writeField('set', value);

  /// セットを足すときの測り方（MethodOfMeasurement）
  String? get method => readField('method', asString);
  set method(String? value) => writeField('method', value);

  /// 数量の名前（Length など）
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 数量の種類。IfcQuantityLength・IfcQuantityArea・IfcQuantityVolume・IfcQuantityCount・IfcQuantityWeight・IfcQuantityTime のどれを書くか
  String? get quantity => readField('quantity', asString);
  set quantity(String? value) => writeField('quantity', value);

  /// 値。文書の単位で、有限の負でない値。IFC4X3 の個数は整数
  core.double? get value => readField('value', asDouble);
  set value(core.double? value) => writeField('value', value);

  /// 式（Formula）。IFC2X3 は持たないので、書けば断る
  String? get formula => readField('formula', asString);
  set formula(String? value) => writeField('formula', value);
}

/// プロジェクトの単位の割り当て（IfcUnitAssignment）に SI の単位（IfcSIUnit）を足す。長さと平面角の単位は受けず、同じ種類の単位があれば断る。結果は id を持たない。マイナーバージョン 106 から
class BuildingAddUnitEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAddUnitEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingAddUnitEdit() : super({'kind': 'add_unit'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 単位の種類（IfcUnitEnum の AREAUNIT など）
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// SI の接頭辞（IfcSIPrefix の MILLI など）
  String? get prefix => readField('prefix', asString);
  set prefix(String? value) => writeField('prefix', value);

  /// SI の単位の名前（IfcSIUnitName の SQUARE_METRE など）。単位の種類に合うもの
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);
}

/// 要素か型に材料の構成を IfcRelAssociatesMaterial で結ぶ。結んであった材料は外す。同じ名前の材料（IfcMaterial）は文書のものを使う。結果の id は要素の ID。マイナーバージョン 106 から
class BuildingMaterialEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingMaterialEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingMaterialEdit() : super({'kind': 'material'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 材料を結ぶ要素か型。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get element => readField('element', objectOf(BuildingRef.fromJson));
  set element(BuildingRef? value) => writeField('element', value);

  /// 材料の構成
  BuildingMaterialChange? get material => readField('material', objectOf(BuildingMaterialChange.fromJson));
  set material(BuildingMaterialChange? value) => writeField('material', value);
}

/// 結ぶ材料の構成。kind に合うフィールドだけを書く。single は name、list は names、layers は layers と usage、constituents は constituents
class BuildingMaterialChange extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingMaterialChange.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingMaterialChange() : super({});

  /// 構成の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 単一の材料の名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 材料の並びの名前。1 つ以上
  List<String>? get names => readField('names', listOf(asString));
  set names(List<String>? value) => writeField('names', value);

  /// 材料の層。1 つ以上
  List<BuildingMaterialLayer>? get layers => readField('layers', listOf(objectOf(BuildingMaterialLayer.fromJson)));
  set layers(List<BuildingMaterialLayer>? value) => writeField('layers', value);

  /// 層の使い方（IfcMaterialLayerSetUsage）。書かなければ、層の組（IfcMaterialLayerSet）をそのまま結ぶ
  BuildingLayerUsage? get usage => readField('usage', objectOf(BuildingLayerUsage.fromJson));
  set usage(BuildingLayerUsage? value) => writeField('usage', value);

  /// 構成要素。1 つ以上
  List<BuildingMaterialConstituent>? get constituents => readField('constituents', listOf(objectOf(BuildingMaterialConstituent.fromJson)));
  set constituents(List<BuildingMaterialConstituent>? value) => writeField('constituents', value);
}

/// 結ぶ材料の構成の種類。single は IfcMaterial、list は IfcMaterialList（IFC2X3 だけ。IFC4 からは非推奨なので断る）、layers は IfcMaterialLayerSet、constituents は IfcMaterialConstituentSet（IFC4 から）
///
/// 列挙の値。知らない値も受け取る
abstract final class BuildingMaterialChangeKind {
  static const single = 'single';
  static const list = 'list';
  static const layers = 'layers';
  static const constituents = 'constituents';
}

/// 材料の層
class BuildingMaterialLayer extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingMaterialLayer.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingMaterialLayer() : super({});

  /// 材料の名前。書かなければ、材料のない層（空気層など）
  String? get material => readField('material', asString);
  set material(String? value) => writeField('material', value);

  /// 層の厚さ。文書の長さの単位で、IFC2X3 は正、IFC4 からは負でない値
  core.double? get thickness => readField('thickness', asDouble);
  set thickness(core.double? value) => writeField('thickness', value);

  /// 層の名前。IFC2X3 は持たないので、書けば断る
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);
}

/// 材料の層の使い方
class BuildingLayerUsage extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingLayerUsage.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingLayerUsage() : super({});

  /// 層を重ねる向き
  String? get direction => readField('direction', asString);
  set direction(String? value) => writeField('direction', value);

  /// true なら負の向きに重ねる（NEGATIVE）。書かなければ正の向き
  core.bool? get negative => readField('negative', asBool);
  set negative(core.bool? value) => writeField('negative', value);

  /// 基準線からのずれ。文書の長さの単位
  core.double? get offset => readField('offset', asDouble);
  set offset(core.double? value) => writeField('offset', value);
}

/// 層を重ねる向き（IfcLayerSetDirectionEnum）。axis1 は AXIS1、axis2 は AXIS2、axis3 は AXIS3
///
/// 列挙の値。知らない値も受け取る
abstract final class BuildingLayerDirection {
  static const axis1 = 'axis1';
  static const axis2 = 'axis2';
  static const axis3 = 'axis3';
}

/// 材料の構成要素
class BuildingMaterialConstituent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingMaterialConstituent.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingMaterialConstituent() : super({});

  /// 材料の名前
  String? get material => readField('material', asString);
  set material(String? value) => writeField('material', value);

  /// 構成要素の名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);
}

/// 要素の形（Axis と Body の表現）を置き換える。項目は items に並べ、Body に置く項目を body の位置で挙げる。集合演算と写像は、items の前にある項目を位置で指し、同じ項目を何度指してもよい（それぞれに写して書く）。表現の種類（SweptSolid・AdvancedSweptSolid・Brep・SolidModel・Tessellation・Clipping・CSG・MappedRepresentation）はコアが決め、決まらない組み合わせ・表現に直接置く半空間・16 段を超える入れ子・写して 10,000 項目を超える Body は断る。使われなくなった元の表現は除く。要素は配置を持つこと。結果の id は要素の ID。マイナーバージョン 107 から
class BuildingShapeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingShapeEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingShapeEdit() : super({'kind': 'shape'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 形を置き換える要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get element => readField('element', objectOf(BuildingRef.fromJson));
  set element(BuildingRef? value) => writeField('element', value);

  /// 芯の線（Axis の IfcPolyline）。要素の配置の [x, y] を 2 点以上。書かなければ Axis を書かない
  List<List<core.double>>? get axis => readField('axis', listOf(listOf(asDouble)));
  set axis(List<List<core.double>>? value) => writeField('axis', value);

  /// 形の項目の表
  List<BuildingShapeItem>? get items => readField('items', listOf(objectOf(BuildingShapeItem.fromJson)));
  set items(List<BuildingShapeItem>? value) => writeField('items', value);

  /// Body の表現に置く項目の、items の位置（0 から）。空なら Body を書かない
  List<core.int>? get body => readField('body', listOf(asInt));
  set body(List<core.int>? value) => writeField('body', value);
}

/// 形の項目。kind に合うフィールドだけを書く。extrusion は profile・position・direction・depth、revolution は profile・position・axis・angle、swept_disk は directrix・radius・inner_radius・start・end、faces は points・faces・closed、boolean は operation・first・second、half_space は plane・agreement・box か polygon、mapped は origin・target・members。長さは文書の長さの単位
class BuildingShapeItem extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingShapeItem.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingShapeItem() : super({});

  /// 項目の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 押し出す・回す断面
  BuildingShapeProfile? get profile => readField('profile', objectOf(BuildingShapeProfile.fromJson));
  set profile(BuildingShapeProfile? value) => writeField('profile', value);

  /// 押し出し・回転の局所の配置。書かなければ要素の配置のまま
  BuildingAxis3? get position => readField('position', objectOf(BuildingAxis3.fromJson));
  set position(BuildingAxis3? value) => writeField('position', value);

  /// 局所の押し出す向き [x, y, z]。z は 0 にできない。書かなければ [0, 0, 1]
  List<core.double>? get direction => readField('direction', listOf(asDouble));
  set direction(List<core.double>? value) => writeField('direction', value);

  /// 押し出す深さ。正の値
  core.double? get depth => readField('depth', asDouble);
  set depth(core.double? value) => writeField('depth', value);

  /// 断面を回す軸。断面の平面（局所の XY 平面）にある。マイナーバージョン 108 から
  BuildingShapeRevolutionAxis? get axis => readField('axis', objectOf(BuildingShapeRevolutionAxis.fromJson));
  set axis(BuildingShapeRevolutionAxis? value) => writeField('axis', value);

  /// 回す角度（ラジアン）。0 より大きく 2π 以下。文書の平面角の単位に直して書くので、文書は平面角の単位を持つこと。マイナーバージョン 108 から
  core.double? get angle => readField('angle', asDouble);
  set angle(core.double? value) => writeField('angle', value);

  /// 円盤を沿わせる折れ線の [x, y, z] を 2 点以上。続く点は重ならない。マイナーバージョン 108 から
  List<List<core.double>>? get directrix => readField('directrix', listOf(listOf(asDouble)));
  set directrix(List<List<core.double>>? value) => writeField('directrix', value);

  /// 円盤の半径。正の値。マイナーバージョン 108 から
  core.double? get radius => readField('radius', asDouble);
  set radius(core.double? value) => writeField('radius', value);

  /// 管の内側の半径。正で radius より小さい。書かなければ中実。マイナーバージョン 108 から
  core.double? get innerRadius => readField('inner_radius', asDouble);
  set innerRadius(core.double? value) => writeField('inner_radius', value);

  /// 掃引を始める折れ線のパラメーター（1 区間で 1 進む）。書かなければ始点。IFC2X3 では書かなければ 0 を書く。マイナーバージョン 108 から
  core.double? get start => readField('start', asDouble);
  set start(core.double? value) => writeField('start', value);

  /// 掃引を終える折れ線のパラメーター。start より大きく区間の数以下。書かなければ終点。IFC2X3 では書かなければ区間の数を書く。マイナーバージョン 108 から
  core.double? get end => readField('end', asDouble);
  set end(core.double? value) => writeField('end', value);

  /// 面集合の点 [x, y, z]
  List<List<core.double>>? get points => readField('points', listOf(listOf(asDouble)));
  set points(List<List<core.double>>? value) => writeField('points', value);

  /// 面集合の面
  List<BuildingShapeFace>? get faces => readField('faces', listOf(objectOf(BuildingShapeFace.fromJson)));
  set faces(List<BuildingShapeFace>? value) => writeField('faces', value);

  /// true なら閉殻（立体。IfcFacetedBrep）。false か書かなければ開殻の面（IFC2X3 は IfcShellBasedSurfaceModel、IFC4 からは IfcPolygonalFaceSet）
  core.bool? get closed => readField('closed', asBool);
  set closed(core.bool? value) => writeField('closed', value);

  /// 集合演算
  String? get operation => readField('operation', asString);
  set operation(String? value) => writeField('operation', value);

  /// 集合演算の第 1 の対象。items の前にある項目の位置
  core.int? get first => readField('first', asInt);
  set first(core.int? value) => writeField('first', value);

  /// 集合演算の第 2 の対象。差では第 1 から取り除く。items の前にある項目の位置
  core.int? get second => readField('second', asInt);
  set second(core.int? value) => writeField('second', value);

  /// 半空間の基準面の配置。局所の z 軸が面の法線
  BuildingAxis3? get plane => readField('plane', objectOf(BuildingAxis3.fromJson));
  set plane(BuildingAxis3? value) => writeField('plane', value);

  /// true なら法線と逆の側、false か書かなければ法線の向く側を内部とする
  core.bool? get agreement => readField('agreement', asBool);
  set agreement(core.bool? value) => writeField('agreement', value);

  /// 半空間の計算用の囲みの箱（IfcBoxedHalfSpace）。座標系は IFC2X3 では基準面、IFC4 からは要素
  BuildingShapeBox? get box => readField('box', objectOf(BuildingShapeBox.fromJson));
  set box(BuildingShapeBox? value) => writeField('box', value);

  /// 半空間を制限する多角形（IfcPolygonalBoundedHalfSpace）
  BuildingShapePolygon? get polygon => readField('polygon', objectOf(BuildingShapePolygon.fromJson));
  set polygon(BuildingShapePolygon? value) => writeField('polygon', value);

  /// 写像元の基準位置。書かなければ原点
  BuildingAxis3? get origin => readField('origin', objectOf(BuildingAxis3.fromJson));
  set origin(BuildingAxis3? value) => writeField('origin', value);

  /// 写像先の変換
  BuildingShapeMapping? get target => readField('target', objectOf(BuildingShapeMapping.fromJson));
  set target(BuildingShapeMapping? value) => writeField('target', value);

  /// 写像元の表現の項目。items の前にある項目の位置で、1 つ以上
  List<core.int>? get members => readField('members', listOf(asInt));
  set members(List<core.int>? value) => writeField('members', value);
}

/// 形の項目の種類。extrusion は断面の押し出し（IfcExtrudedAreaSolid）、faces は面集合、boolean は集合演算（IfcBooleanResult。差で第 2 が半空間なら IfcBooleanClippingResult）、half_space は平面の半空間（集合演算の対象にだけ置く）、mapped は写像（IfcMappedItem）。revolution は断面の回転（IfcRevolvedAreaSolid）、swept_disk は円盤の掃引（IfcSweptDiskSolid）で、マイナーバージョン 108 から
///
/// 列挙の値。知らない値も受け取る
abstract final class BuildingShapeItemKind {
  static const extrusion = 'extrusion';
  static const faces = 'faces';
  static const boolean = 'boolean';
  static const halfSpace = 'half_space';
  static const mapped = 'mapped';
  static const revolution = 'revolution';
  static const sweptDisk = 'swept_disk';
}

/// 断面を回す軸（IfcAxis1Placement）。断面の平面にある
class BuildingShapeRevolutionAxis extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingShapeRevolutionAxis.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingShapeRevolutionAxis() : super({});

  /// 軸の通る点 [x, y]
  List<core.double>? get location => readField('location', listOf(asDouble));
  set location(List<core.double>? value) => writeField('location', value);

  /// 軸の向き [x, y]。0 にできない
  List<core.double>? get direction => readField('direction', listOf(asDouble));
  set direction(List<core.double>? value) => writeField('direction', value);
}

/// 集合演算。union は和、intersection は積、difference は差
///
/// 列挙の値。知らない値も受け取る
abstract final class BuildingBooleanOperation {
  static const union = 'union';
  static const intersection = 'intersection';
  static const difference = 'difference';
}

/// 3 次元の局所の配置
class BuildingAxis3 extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingAxis3.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingAxis3() : super({});

  /// 原点 [x, y, z]
  List<core.double>? get location => readField('location', listOf(asDouble));
  set location(List<core.double>? value) => writeField('location', value);

  /// 局所の z 軸 [x, y, z]
  List<core.double>? get axis => readField('axis', listOf(asDouble));
  set axis(List<core.double>? value) => writeField('axis', value);

  /// 局所の x 軸の参照方向 [x, y, z]
  List<core.double>? get reference => readField('reference', listOf(asDouble));
  set reference(List<core.double>? value) => writeField('reference', value);
}

/// 押し出す断面。rectangle は x と y、circle は radius、polygon は outer と holes を書く
class BuildingShapeProfile extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingShapeProfile.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingShapeProfile() : super({});

  /// 断面の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 断面の名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 矩形の局所の x の全幅。正の値
  core.double? get x => readField('x', asDouble);
  set x(core.double? value) => writeField('x', value);

  /// 矩形の局所の y の全高。正の値
  core.double? get y => readField('y', asDouble);
  set y(core.double? value) => writeField('y', value);

  /// 円の半径。正の値
  core.double? get radius => readField('radius', asDouble);
  set radius(core.double? value) => writeField('radius', value);

  /// 矩形と円の中心 [x, y]。書かなければ原点
  List<core.double>? get location => readField('location', listOf(asDouble));
  set location(List<core.double>? value) => writeField('location', value);

  /// 矩形と円の局所の x 軸の参照方向 [x, y]。location と一緒に書く
  List<core.double>? get reference => readField('reference', listOf(asDouble));
  set reference(List<core.double>? value) => writeField('reference', value);

  /// 多角形の外輪の [x, y] を 3 点以上。始点に戻る点は書かない
  List<List<core.double>>? get outer => readField('outer', listOf(listOf(asDouble)));
  set outer(List<List<core.double>>? value) => writeField('outer', value);

  /// 多角形の内輪
  List<List<List<core.double>>>? get holes => readField('holes', listOf(listOf(listOf(asDouble))));
  set holes(List<List<List<core.double>>>? value) => writeField('holes', value);
}

/// 断面の種類。rectangle は IfcRectangleProfileDef、circle は IfcCircleProfileDef、polygon は IfcArbitraryClosedProfileDef（内輪があれば IfcArbitraryProfileDefWithVoids）
///
/// 列挙の値。知らない値も受け取る
abstract final class BuildingShapeProfileKind {
  static const rectangle = 'rectangle';
  static const circle = 'circle';
  static const polygon = 'polygon';
}

/// 面集合の面。points の位置（0 から）で外輪と内輪を書く
class BuildingShapeFace extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingShapeFace.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingShapeFace() : super({});

  /// 外輪の点の位置。3 つ以上
  List<core.int>? get boundary => readField('boundary', listOf(asInt));
  set boundary(List<core.int>? value) => writeField('boundary', value);

  /// 内輪ごとの点の位置
  List<List<core.int>>? get holes => readField('holes', listOf(listOf(asInt)));
  set holes(List<List<core.int>>? value) => writeField('holes', value);
}

/// 囲みの箱
class BuildingShapeBox extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingShapeBox.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingShapeBox() : super({});

  /// 箱の隅 [x, y, z]
  List<core.double>? get corner => readField('corner', listOf(asDouble));
  set corner(List<core.double>? value) => writeField('corner', value);

  /// 各軸の長さ [x, y, z]。正の値
  List<core.double>? get dimensions => readField('dimensions', listOf(asDouble));
  set dimensions(List<core.double>? value) => writeField('dimensions', value);
}

/// 半空間を制限する多角形
class BuildingShapePolygon extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingShapePolygon.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingShapePolygon() : super({});

  /// 境界の配置
  BuildingAxis3? get position => readField('position', objectOf(BuildingAxis3.fromJson));
  set position(BuildingAxis3? value) => writeField('position', value);

  /// 境界の局所の XY 平面の [x, y] を 3 点以上。始点に戻る点は書かない
  List<List<core.double>>? get boundary => readField('boundary', listOf(listOf(asDouble)));
  set boundary(List<List<core.double>>? value) => writeField('boundary', value);
}

/// 写像先の変換（IfcCartesianTransformationOperator3D）。scales を書けば軸ごとの尺度（NonUniform）
class BuildingShapeMapping extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingShapeMapping.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingShapeMapping() : super({});

  /// 写像先の原点 [x, y, z]。要素の座標系
  List<core.double>? get origin => readField('origin', listOf(asDouble));
  set origin(List<core.double>? value) => writeField('origin', value);

  /// x 軸を導く方向 [x, y, z]
  List<core.double>? get axis1 => readField('axis1', listOf(asDouble));
  set axis1(List<core.double>? value) => writeField('axis1', value);

  /// y 軸を導く方向 [x, y, z]
  List<core.double>? get axis2 => readField('axis2', listOf(asDouble));
  set axis2(List<core.double>? value) => writeField('axis2', value);

  /// z 軸を定める方向 [x, y, z]
  List<core.double>? get axis3 => readField('axis3', listOf(asDouble));
  set axis3(List<core.double>? value) => writeField('axis3', value);

  /// 一様な尺度。正の値で、書かなければ 1
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);

  /// 軸ごとの尺度 [x, y, z]。正の値。scale とは一緒に書けない
  List<core.double>? get scales => readField('scales', listOf(asDouble));
  set scales(List<core.double>? value) => writeField('scales', value);
}

/// 扉や窓を開口（IfcOpeningElement）に IfcRelFillsElement で結ぶ。1 つの開口は 1 つの要素だけで埋める。マイナーバージョン 96 から
class BuildingFillEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingFillEdit.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BuildingFillEdit() : super({'kind': 'fill'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 開口。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get opening => readField('opening', objectOf(BuildingRef.fromJson));
  set opening(BuildingRef? value) => writeField('opening', value);

  /// 埋める要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
  BuildingRef? get filler => readField('filler', objectOf(BuildingRef.fromJson));
  set filler(BuildingRef? value) => writeField('filler', value);
}

/// 当てた変更の結果。書き戻したファイルは blobs の file で返す
class BuildingApplyResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingApplyResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingApplyResult() : super({});

  /// 変更ごとの結果。要求の edits と同じ順
  List<Applied>? get applied => readField('applied', listOf(objectOf(Applied.fromJson)));
  set applied(List<Applied>? value) => writeField('applied', value);

  /// 書き戻した文書のハッシュ。次の building_apply の前提条件に使う
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);
}

/// 開いた図面の文書を図面モデル（L2）に変換し、図面モデルのバイナリ（決定 0247）を blob で返す。配置は schema/drawing.toml にあり、元の文書の形式・系統・ハッシュも持つ。プラグインのモジュールが読んだ図面は、これを図面のモジュールの open に渡すと、元の形式とハッシュの読むだけの文書になる。マイナーバージョン 60 から
class LiftRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LiftRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  LiftRequest() : super({'op': 'lift'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で図面を開いたもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);
}

/// 図面モデルのバイナリの blob の番号
class LiftResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LiftResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  LiftResult() : super({});

  /// 図面モデルのバイナリの blob の番号（種類は drawing）。zm_take で取り出す
  core.int? get blob => readField('blob', asInt);
  set blob(core.int? value) => writeField('blob', value);
}

/// 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。マイナーバージョン 44 から
class IdsCheckRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IdsCheckRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  IdsCheckRequest() : super({'op': 'ids_check'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で IFC を開いたもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// IDS の入力の名前（zm_push で渡した .ids）
  String? get ids => readField('ids', asString);
  set ids(String? value) => writeField('ids', value);

  /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルのコンポーネントにある属性だけを照らす
  String? get express => readField('express', asString);
  set express(String? value) => writeField('express', value);

  /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
  core.int? get offset => readField('offset', asInt);
  set offset(core.int? value) => writeField('offset', value);

  /// 返す数の上限。書かなければ 1000
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);
}

/// 照合の結果。指摘は offset から limit 個まで
class IdsCheckResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IdsCheckResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  IdsCheckResult() : super({});

  /// IDS の題
  String? get title => readField('title', asString);
  set title(String? value) => writeField('title', value);

  /// 仕様ごとの結果。IDS の順
  List<IdsSpecification>? get specifications => readField('specifications', listOf(objectOf(IdsSpecification.fromJson)));
  set specifications(List<IdsSpecification>? value) => writeField('specifications', value);

  /// 指摘。仕様の順
  List<IdsFinding>? get findings => readField('findings', listOf(objectOf(IdsFinding.fromJson)));
  set findings(List<IdsFinding>? value) => writeField('findings', value);

  /// 全体の指摘の数
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 続きの最初の位置。続きがなければ持たない
  core.int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(core.int? value) => writeField('next_offset', value);
}

/// 仕様の結果
class IdsSpecification extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IdsSpecification.fromJson(super.json);

  /// 空のオブジェクトから作る
  IdsSpecification() : super({});

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 識別子
  String? get identifier => readField('identifier', asString);
  set identifier(String? value) => writeField('identifier', value);

  /// 結果
  String? get status => readField('status', asString);
  set status(String? value) => writeField('status', value);

  /// 適用の条件に合う要素の数
  core.int? get applicable => readField('applicable', asInt);
  set applicable(core.int? value) => writeField('applicable', value);

  /// 要件を満たさない要素の数
  core.int? get failed => readField('failed', asInt);
  set failed(core.int? value) => writeField('failed', value);

  /// 確かめられなかった要素の数
  core.int? get unsupported => readField('unsupported', asInt);
  set unsupported(core.int? value) => writeField('unsupported', value);
}

/// 指摘。diagnostic は診断と同じ形で、describe で文にできる
class IdsFinding extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IdsFinding.fromJson(super.json);

  /// 空のオブジェクトから作る
  IdsFinding() : super({});

  /// 仕様の名前
  String? get specification => readField('specification', asString);
  set specification(String? value) => writeField('specification', value);

  /// code・重要度・場所（要素の ID）・値
  Diagnostic? get diagnostic => readField('diagnostic', objectOf(Diagnostic.fromJson));
  set diagnostic(Diagnostic? value) => writeField('diagnostic', value);
}

/// 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。マイナーバージョン 44 から
class ReviewAdoptIdsRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewAdoptIdsRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  ReviewAdoptIdsRequest() : super({'op': 'review_adopt_ids'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で IFC を開いたもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// IDS の入力の名前（zm_push で渡した .ids）
  String? get ids => readField('ids', asString);
  set ids(String? value) => writeField('ids', value);

  /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルのコンポーネントにある属性だけを照らす
  String? get express => readField('express', asString);
  set express(String? value) => writeField('express', value);

  /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
  String? get review => readField('review', asString);
  set review(String? value) => writeField('review', value);

  /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 取り込む指摘の番号の列。ids_check の指摘の並びの順で、0 から数える
  List<core.int>? get findings => readField('findings', listOf(asInt));
  set findings(List<core.int>? value) => writeField('findings', value);
}

/// zm_push で渡した BCF 2.1 か 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。マイナーバージョン 45 から
class BcfReadRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BcfReadRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BcfReadRequest() : super({'op': 'bcf_read'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// BCF が指す建物モデルの文書の名前。open で IFC を開いたもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// bcfzip の入力の名前（zm_push で渡した .bcf）
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);

  /// レビューの文書の対象の ID
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
  List<BcfStatus>? get statuses => readField('statuses', listOf(objectOf(BcfStatus.fromJson)));
  set statuses(List<BcfStatus>? value) => writeField('statuses', value);
}

/// 状態の対応
class BcfStatus extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BcfStatus.fromJson(super.json);

  /// 空のオブジェクトから作る
  BcfStatus() : super({});

  /// BCF の TopicStatus の文字列
  String? get text => readField('text', asString);
  set text(String? value) => writeField('text', value);

  /// 指摘の状態
  String? get status => readField('status', asString);
  set status(String? value) => writeField('status', value);
}

/// 指摘の状態（決定 0214）
///
/// 列挙の値。知らない値も受け取る
abstract final class ReviewStatus {
  static const open = 'open';
  static const resolved = 'resolved';
  static const closed = 'closed';
}

/// 読んだ bcfzip
class BcfReadResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BcfReadResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  BcfReadResult() : super({});

  /// BCF のバージョン
  String? get version => readField('version', asString);
  set version(String? value) => writeField('version', value);

  /// 指摘の数
  core.int? get topics => readField('topics', asInt);
  set topics(core.int? value) => writeField('topics', value);
}

/// zm_push で渡したレビューの文書を、BCF の bcfzip に書く（決定 0214・0220）。バージョンは version で選ぶ（マイナーバージョン 61 から。それまでは 3.0 だけ）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。マイナーバージョン 45 から
class BcfWriteRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BcfWriteRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BcfWriteRequest() : super({'op': 'bcf_write'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// レビューの文書の入力の名前（zm_push で渡した JSON）
  String? get review => readField('review', asString);
  set review(String? value) => writeField('review', value);

  /// viewpoint にする 3D の場所の対象の ID
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 元の bcfzip の入力の名前（zm_push で渡した .bcf）。書かなければ新しく書く
  String? get original => readField('original', asString);
  set original(String? value) => writeField('original', value);

  /// 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
  List<BcfStatus>? get statuses => readField('statuses', listOf(objectOf(BcfStatus.fromJson)));
  set statuses(List<BcfStatus>? value) => writeField('statuses', value);

  /// 種類のない指摘に書く TopicType
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 作者のない指摘とコメントに書く作者
  String? get author => readField('author', asString);
  set author(String? value) => writeField('author', value);

  /// 日時のない指摘とコメントに書く日時（RFC 3339）
  String? get date => readField('date', asString);
  set date(String? value) => writeField('date', value);

  /// 書く BCF のバージョン。2.1 か 3.0。書かなければ、original のバージョンか 3.0。original と違うバージョンでは書かず、bcf.conversion.unsupported で失敗する。ほかの値は call.value.invalid。マイナーバージョン 61 から
  String? get version => readField('version', asString);
  set version(String? value) => writeField('version', value);
}

/// 書いた bcfzip
class BcfWriteResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BcfWriteResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  BcfWriteResult() : super({});

  /// 書いた指摘の数
  core.int? get topics => readField('topics', asInt);
  set topics(core.int? value) => writeField('topics', value);
}

/// 表の項目
class TableItem extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TableItem.fromJson(super.json);

  /// 空のオブジェクトから作る
  TableItem() : super({});

  /// ID。表のレコードがハンドルを持てば layer/h10・line-type/h14、なければ読んだ順の番号（layer/2）。寸法スタイルは dimension-style/h1A か、寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 元の名前。翻訳しない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);
}

/// レイヤの表の項目（決定 0162）。マイナーバージョン 22 から、状態とスタイルを持つ
class LayerItem extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LayerItem.fromJson(super.json);

  /// 空のオブジェクトから作る
  LayerItem() : super({});

  /// ID。表のレコードがハンドルを持てば layer/h10、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 元の名前。翻訳しない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかのフィールドを持たない。マイナーバージョン 22 から
  core.bool? get defined => readField('defined', asBool);
  set defined(core.bool? value) => writeField('defined', value);

  /// 表示する。マイナーバージョン 22 から
  core.bool? get visible => readField('visible', asBool);
  set visible(core.bool? value) => writeField('visible', value);

  /// 印刷する。マイナーバージョン 22 から
  core.bool? get printable => readField('printable', asBool);
  set printable(core.bool? value) => writeField('printable', value);

  /// 編集できる。マイナーバージョン 22 から
  core.bool? get editable => readField('editable', asBool);
  set editable(core.bool? value) => writeField('editable', value);

  /// 凍結する。マイナーバージョン 22 から
  core.bool? get frozen => readField('frozen', asBool);
  set frozen(core.bool? value) => writeField('frozen', value);

  /// 色。inherit を持たない。定義が色を持たなければ書かない。マイナーバージョン 22 から
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 線種の ID。定義が線種を持たなければ書かない。マイナーバージョン 22 から
  String? get lineTypeId => readField('line_type_id', asString);
  set lineTypeId(String? value) => writeField('line_type_id', value);

  /// 線幅。inherit を持たない。定義が線幅を持たなければ書かない。マイナーバージョン 22 から
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);
}

/// 線種の表の項目（決定 0164）。マイナーバージョン 26 から、定義を持つ
class LineTypeItem extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LineTypeItem.fromJson(super.json);

  /// 空のオブジェクトから作る
  LineTypeItem() : super({});

  /// ID。表のレコードがハンドルを持てば line-type/h14、なければ読んだ順の番号（line-type/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 元の名前。翻訳しない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかのフィールドを持たない。マイナーバージョン 26 から
  core.bool? get defined => readField('defined', asBool);
  set defined(core.bool? value) => writeField('defined', value);

  /// 元の説明。マイナーバージョン 26 から
  String? get description => readField('description', asString);
  set description(String? value) => writeField('description', value);

  /// 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。マイナーバージョン 26 から
  List<core.double>? get pattern => readField('pattern', listOf(asDouble));
  set pattern(List<core.double>? value) => writeField('pattern', value);

  /// 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。マイナーバージョン 26 から
  core.bool? get paper => readField('paper', asBool);
  set paper(core.bool? value) => writeField('paper', value);

  /// 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。マイナーバージョン 26 から
  core.bool? get marked => readField('marked', asBool);
  set marked(core.bool? value) => writeField('marked', value);

  /// 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。マイナーバージョン 28 から
  List<LineTypeMark>? get marks => readField('marks', listOf(objectOf(LineTypeMark.fromJson)));
  set marks(List<LineTypeMark>? value) => writeField('marks', value);
}

/// 寸法スタイルの表の項目（決定 0165）。マイナーバージョン 27 から、端部記号と大きさを持つ
class DimensionStyleItem extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DimensionStyleItem.fromJson(super.json);

  /// 空のオブジェクトから作る
  DimensionStyleItem() : super({});

  /// ID。寸法スタイルのレコードがハンドルを持てば dimension-style/h1A、なければ寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 元の名前。翻訳しない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。マイナーバージョン 27 から
  List<String>? get ends => readField('ends', listOf(asString));
  set ends(List<String>? value) => writeField('ends', value);

  /// 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。マイナーバージョン 27 から
  String? get leader => readField('leader', asString);
  set leader(String? value) => writeField('leader', value);

  /// 端部記号の大きさ。文書の単位で、scale を掛ける前の値。マイナーバージョン 27 から
  core.double? get size => readField('size', asDouble);
  set size(core.double? value) => writeField('size', value);

  /// 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。マイナーバージョン 27 から
  core.double? get tick => readField('tick', asDouble);
  set tick(core.double? value) => writeField('tick', value);

  /// 大きさに掛ける倍率。0 はビューの縮尺から決める。マイナーバージョン 27 から
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);
}

/// 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。マイナーバージョン 28 から
class LineTypeMark extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LineTypeMark.fromJson(super.json);

  /// 空のオブジェクトから作る
  LineTypeMark() : super({});

  /// 添える列の要素の番号（pattern の添字）
  core.int? get index => readField('index', asInt);
  set index(core.int? value) => writeField('index', value);

  /// 形（shape）か文字（text）
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 形の形のファイル。元の形式が指す文字スタイルがなければ書かない
  Font? get font => readField('font', objectOf(Font.fromJson));
  set font(Font? value) => writeField('font', value);

  /// 形の番号
  core.int? get number => readField('number', asInt);
  set number(core.int? value) => writeField('number', value);

  /// 文字の文字列
  String? get text => readField('text', asString);
  set text(String? value) => writeField('text', value);

  /// 文字の文字スタイルの元の名前。元の形式が指す文字スタイルがなければ書かない
  String? get style => readField('style', asString);
  set style(String? value) => writeField('style', value);

  /// 文字の文字スタイルの ID。tables の text_styles の ID で、名前で引いたもの（大文字と小文字を区別せず、同じ名前があれば先の項目）
  String? get styleId => readField('style_id', asString);
  set styleId(String? value) => writeField('style_id', value);

  /// 文字の高さ。文字スタイルが決まった高さを持てば、それに scale を掛けたもの。持たなければ scale。単位は pattern と同じ
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 倍率。文字なら、文字スタイルの高さへの倍率
  core.double? get scale => readField('scale', asDouble);
  set scale(core.double? value) => writeField('scale', value);

  /// 回転（度、反時計回り）
  core.double? get rotation => readField('rotation', asDouble);
  set rotation(core.double? value) => writeField('rotation', value);

  /// 回転の基準
  String? get rotationBasis => readField('rotation_basis', asString);
  set rotationBasis(String? value) => writeField('rotation_basis', value);

  /// 置く位置のずれ（x・y）。線の向きを x とする。単位は pattern と同じ
  List<core.double>? get offset => readField('offset', listOf(asDouble));
  set offset(List<core.double>? value) => writeField('offset', value);
}

/// 線種に添えるものの種類。shape は形のファイルの形、text は文字
///
/// 列挙の値。知らない値も受け取る
abstract final class LineTypeMarkKind {
  static const shape = 'shape';
  static const text = 'text';
}

/// 回転の基準。relative は線の向きから、absolute は文書の x 軸から測る。upright は線の向きから測り、文字が逆さに見えるなら半回転する
///
/// 列挙の値。知らない値も受け取る
abstract final class MarkRotation {
  static const relative = 'relative';
  static const absolute = 'absolute';
  static const upright = 'upright';
}

/// 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。マイナーバージョン 20 から
class TextStyleItem extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TextStyleItem.fromJson(super.json);

  /// 空のオブジェクトから作る
  TextStyleItem() : super({});

  /// ID。表のレコードがハンドルを持てば text-style/h11、なければ文字スタイルの並びの中の順（text-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 元の名前。翻訳しない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// ファイルに定義がある。偽なら、ほかのフィールドを持たない
  core.bool? get defined => readField('defined', asBool);
  set defined(core.bool? value) => writeField('defined', value);

  /// 主のフォント。ファイルが決めていなければ書かない
  Font? get font => readField('font', objectOf(Font.fromJson));
  set font(Font? value) => writeField('font', value);

  /// 大きいフォント（漢字などの形のファイル）。なければ書かない
  Font? get bigFont => readField('big_font', objectOf(Font.fromJson));
  set bigFont(Font? value) => writeField('big_font', value);

  /// 決まった高さ。文書の単位。決めなければ書かない
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 幅の倍率
  core.double? get width => readField('width', asDouble);
  set width(core.double? value) => writeField('width', value);

  /// 傾き（度）
  core.double? get oblique => readField('oblique', asDouble);
  set oblique(core.double? value) => writeField('oblique', value);

  /// 左右を裏返す
  core.bool? get backward => readField('backward', asBool);
  set backward(core.bool? value) => writeField('backward', value);

  /// 上下を反転する
  core.bool? get upsideDown => readField('upside_down', asBool);
  set upsideDown(core.bool? value) => writeField('upside_down', value);

  /// 縦書き
  core.bool? get vertical => readField('vertical', asBool);
  set vertical(core.bool? value) => writeField('vertical', value);
}

/// フォントの指定。取得と字形の評価は使い手が行う
class Font extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Font.fromJson(super.json);

  /// 空のオブジェクトから作る
  Font() : super({});

  /// 元の名前。形のファイルならファイルの名前、名前で指すフォントなら書体の名前。翻訳しない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 太字。名前で指すフォントだけ
  core.bool? get bold => readField('bold', asBool);
  set bold(core.bool? value) => writeField('bold', value);

  /// 斜体。名前で指すフォントだけ
  core.bool? get italic => readField('italic', asBool);
  set italic(core.bool? value) => writeField('italic', value);

  /// 文字集合の番号（128 は Shift_JIS）。名前で指すフォントで、元の形式が決めていれば
  core.int? get charset => readField('charset', asInt);
  set charset(core.int? value) => writeField('charset', value);

  /// ピッチと系統の番号。名前で指すフォントで、元の形式が決めていれば
  core.int? get pitch => readField('pitch', asInt);
  set pitch(core.int? value) => writeField('pitch', value);
}

/// フォントの種類。shape は形のファイル（SHX など）、named は名前で指すフォント（TrueType など）
///
/// 列挙の値。知らない値も受け取る
abstract final class FontKind {
  static const shape = 'shape';
  static const named = 'named';
}

/// 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つキー（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない
class Source extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Source.fromJson(super.json);

  /// 空のオブジェクトから作る
  Source() : super({});

  /// L1 のレコードの番号
  core.int? get record => readField('record', asInt);
  set record(core.int? value) => writeField('record', value);

  /// L1 のバイト範囲
  ByteRange? get bytes => readField('bytes', objectOf(ByteRange.fromJson));
  set bytes(ByteRange? value) => writeField('bytes', value);

  /// AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない
  String? get handle => readField('handle', asString);
  set handle(String? value) => writeField('handle', value);

  /// SXF のインスタンスの番号（#n の n）。10 進の文字列。マイナーバージョン 6 から
  String? get instance => readField('instance', asString);
  set instance(String? value) => writeField('instance', value);
}

/// 応答。失敗したときも同じ形で、result を持たない
class Response extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Response.fromJson(super.json);

  /// 空のオブジェクトから作る
  Response() : super({});

  /// メッセージのスキーマのメジャーバージョン
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 診断。入力の順
  List<Diagnostic>? get diagnostics => readField('diagnostics', listOf(objectOf(Diagnostic.fromJson)));
  set diagnostics(List<Diagnostic>? value) => writeField('diagnostics', value);

  /// バイナリの結果。take で取り出す。次の call で捨てる
  List<Blob>? get blobs => readField('blobs', listOf(objectOf(Blob.fromJson)));
  set blobs(List<Blob>? value) => writeField('blobs', value);

  /// op ごとの結果（<op>_result の形の JSON のオブジェクト）。失敗したときは null
  Map<String, Object?>? get result => readField('result', mapOf(asJson));
  set result(Map<String, Object?>? value) => writeField('result', value);

  /// op ごとの結果を、その op の結果の型で読む（例: `response.resultAs(OpenResult.fromJson)`）。失敗したときは null
  T? resultAs<T>(T Function(Map<String, Object?>) of) => readField('result', objectOf(of));
}

/// 診断。人向けの文は持たず、文は describe で組み立てる（決定 0009）
class Diagnostic extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Diagnostic.fromJson(super.json);

  /// 空のオブジェクトから作る
  Diagnostic() : super({});

  /// 変わらない名前（budget.records.exceeded など）。catalog/codes.toml にある
  String? get code => readField('code', asString);
  set code(String? value) => writeField('code', value);

  /// 重さ。code ごとに決まる
  String? get severity => readField('severity', asString);
  set severity(String? value) => writeField('severity', value);

  /// 変換の結果の段。code ごとに決まる。変換の結果を表さない code では持たない
  String? get fidelity => readField('fidelity', asString);
  set fidelity(String? value) => writeField('fidelity', value);

  /// まとめた件数
  core.int? get count => readField('count', asInt);
  set count(core.int? value) => writeField('count', value);

  /// 場所。入力の順に先頭の数件
  List<Place>? get at => readField('at', listOf(objectOf(Place.fromJson)));
  set at(List<Place>? value) => writeField('at', value);

  /// 仕様の節
  Cite? get cite => readField('cite', objectOf(Cite.fromJson));
  set cite(Cite? value) => writeField('cite', value);

  /// 文に差し込む値。名前と型は code ごとに台帳が決める。2^53 − 1 を超える整数は文字列にする
  Map<String, Object?>? get params => readField('params', mapOf(asJson));
  set params(Map<String, Object?>? value) => writeField('params', value);
}

/// 診断の重さ（決定 0009）。info は何も失っていない、warning は結果を使える、error は失敗した
///
/// 列挙の値。知らない値も受け取る
abstract final class Severity {
  static const info = 'info';
  static const warning = 'warning';
  static const error = 'error';
}

/// 変換の結果の段のうち、`native` 以外のもの（決定 0009）
///
/// 列挙の値。知らない値も受け取る
abstract final class Fidelity {
  static const equivalent = 'equivalent';
  static const approximated = 'approximated';
  static const drawn = 'drawn';
  static const dropped = 'dropped';
}

/// 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所）
class Place extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Place.fromJson(super.json);

  /// 空のオブジェクトから作る
  Place() : super({});

  /// ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// L1 のレコードの番号
  core.int? get record => readField('record', asInt);
  set record(core.int? value) => writeField('record', value);

  /// L1 のバイト範囲
  ByteRange? get bytes => readField('bytes', objectOf(ByteRange.fromJson));
  set bytes(ByteRange? value) => writeField('bytes', value);
}

/// バイト範囲（start から end の手前まで）
class ByteRange extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ByteRange.fromJson(super.json);

  /// 空のオブジェクトから作る
  ByteRange() : super({});

  /// 先頭の位置
  core.int? get start => readField('start', asInt);
  set start(core.int? value) => writeField('start', value);

  /// 終わりの位置（含まない）
  core.int? get end => readField('end', asInt);
  set end(core.int? value) => writeField('end', value);
}

/// 仕様の節。仕様の本文は写さない
class Cite extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Cite.fromJson(super.json);

  /// 空のオブジェクトから作る
  Cite() : super({});

  /// 仕様の一覧での名前
  String? get spec => readField('spec', asString);
  set spec(String? value) => writeField('spec', value);

  /// 節
  String? get section => readField('section', asString);
  set section(String? value) => writeField('section', value);
}

/// バイナリの結果
class Blob extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Blob.fromJson(super.json);

  /// 空のオブジェクトから作る
  Blob() : super({});

  /// 番号。zm_take に渡す
  core.int? get id => readField('id', asInt);
  set id(core.int? value) => writeField('id', value);

  /// 種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// バイト数
  core.int? get size => readField('size', asInt);
  set size(core.int? value) => writeField('size', value);
}

/// バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、scene3d は 3D のシーン（schema/scene3d.toml の配置）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）、drawing は図面モデルのバイナリ（schema/drawing.toml の配置、決定 0247。マイナーバージョン 60 から）
///
/// 列挙の値。知らない値も受け取る
abstract final class BlobKind {
  static const file = 'file';
  static const image = 'image';
  static const scene = 'scene';
  static const arrow = 'arrow';
  static const drawing = 'drawing';
  static const scene3d = 'scene3d';
}

/// zm_session_new に渡す設定。渡さなければ既定の設定
class SessionOptions extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SessionOptions.fromJson(super.json);

  /// 空のオブジェクトから作る
  SessionOptions() : super({});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 実行の形態。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013）
  String? get engine => readField('engine', asString);
  set engine(String? value) => writeField('engine', value);

  /// engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200）
  String? get processPath => readField('process_path', asString);
  set processPath(String? value) => writeField('process_path', value);
}

/// 実行の形態（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
///
/// 列挙の値。知らない値も受け取る
abstract final class Engine {
  static const native = 'native';
  static const process = 'process';
  static const wasm = 'wasm';
}

/// 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML・IFC4.3・SIMA の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面・BIM・土木地理・点群のモジュールに置く（決定 0253）。マイナーバージョン 46 から（doc と id はマイナーバージョン 47 から）
class CivilRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CivilRequest() : super({'op': 'civil'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// doc の座標点・高さ基準と、選んだ線形の IP・地盤縦断・片勾配・横断の値を返す。複数の線形があれば id が必要。省略は false。マイナーバージョン 87 から
  core.bool? get details => readField('details', asBool);
  set details(core.bool? value) => writeField('details', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 線形。doc と一緒には書かない
  CivilAlignment? get alignment => readField('alignment', objectOf(CivilAlignment.fromJson));
  set alignment(CivilAlignment? value) => writeField('alignment', value);

  /// open で開いた J-LandXML・IFC4.3・SIMA の文書の名前。マイナーバージョン 47 から
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// doc の線形の ID。文書に線形が 1 つなら書かなくてよい。マイナーバージョン 47 から
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 追加距離（メートル）の列
  List<core.double>? get distances => readField('distances', listOf(asDouble));
  set distances(List<core.double>? value) => writeField('distances', value);

  /// 測点の表記（No.12+5.000・No.12+5・No.12）の列。測点の不連続のために同じ値が何か所にもあれば、最初の所にして診断する
  List<String>? get stations => readField('stations', listOf(asString));
  set stations(List<String>? value) => writeField('stations', value);

  /// 線形の初めから終わりまで、この間隔（メートル）ごとの所を足す。終わりの所も足す
  core.double? get interval => readField('interval', asDouble);
  set interval(core.double? value) => writeField('interval', value);

  /// 測点の間隔（メートル）。書かなければ 20
  core.double? get pitch => readField('pitch', asDouble);
  set pitch(core.double? value) => writeField('pitch', value);

  /// 測点の表記の端数の小数の桁数。書かなければ 3
  core.int? get digits => readField('digits', asInt);
  set digits(core.int? value) => writeField('digits', value);
}

/// 線形。点は地図の空間（北と東、メートル）に置き、方向角は北から時計回りのラジアンにする。半径は、右へ曲がるものを正にする（決定 0230）
class CivilAlignment extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilAlignment.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilAlignment() : super({});

  /// 平面線形の区間。線形の初めから順に並べる
  List<CivilHorizontal>? get horizontal => readField('horizontal', listOf(objectOf(CivilHorizontal.fromJson)));
  set horizontal(List<CivilHorizontal>? value) => writeField('horizontal', value);

  /// 縦断線形の区間。書かなければ高さを返さない
  List<CivilVertical>? get vertical => readField('vertical', listOf(objectOf(CivilVertical.fromJson)));
  set vertical(List<CivilVertical>? value) => writeField('vertical', value);

  /// 線形の初めの測点の値（メートル）。書かなければ 0
  core.double? get startStation => readField('start_station', asDouble);
  set startStation(core.double? value) => writeField('start_station', value);

  /// 測点の不連続。追加距離の順に並べる
  List<CivilStationBreak>? get breaks => readField('breaks', listOf(objectOf(CivilStationBreak.fromJson)));
  set breaks(List<CivilStationBreak>? value) => writeField('breaks', value);
}

/// 平面線形の区間
class CivilHorizontal extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilHorizontal.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilHorizontal() : super({});

  /// 区間の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 初めの点の北（メートル）
  core.double? get northing => readField('northing', asDouble);
  set northing(core.double? value) => writeField('northing', value);

  /// 初めの点の東（メートル）
  core.double? get easting => readField('easting', asDouble);
  set easting(core.double? value) => writeField('easting', value);

  /// 初めの方向角（北から時計回りのラジアン）
  core.double? get direction => readField('direction', asDouble);
  set direction(core.double? value) => writeField('direction', value);

  /// 長さ（メートル）
  core.double? get length => readField('length', asDouble);
  set length(core.double? value) => writeField('length', value);

  /// 円弧の半径（メートル）。arc に書く
  core.double? get radius => readField('radius', asDouble);
  set radius(core.double? value) => writeField('radius', value);

  /// 緩和曲線の初めの半径（メートル）。直線の側は書かない
  core.double? get startRadius => readField('start_radius', asDouble);
  set startRadius(core.double? value) => writeField('start_radius', value);

  /// 緩和曲線の終わりの半径（メートル）。直線の側は書かない
  core.double? get endRadius => readField('end_radius', asDouble);
  set endRadius(core.double? value) => writeField('end_radius', value);

  /// 緩和曲線の種類。transition に書き、書かなければ clothoid
  String? get curve => readField('curve', asString);
  set curve(String? value) => writeField('curve', value);
}

/// 縦断線形の区間
class CivilVertical extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilVertical.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilVertical() : super({});

  /// 区間の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 初めの追加距離（メートル）
  core.double? get startDistance => readField('start_distance', asDouble);
  set startDistance(core.double? value) => writeField('start_distance', value);

  /// 初めの高さ（メートル）
  core.double? get startHeight => readField('start_height', asDouble);
  set startHeight(core.double? value) => writeField('start_height', value);

  /// 初めの勾配（高さ ÷ 平面の距離）
  core.double? get startGradient => readField('start_gradient', asDouble);
  set startGradient(core.double? value) => writeField('start_gradient', value);

  /// 平面の距離での長さ（メートル）
  core.double? get length => readField('length', asDouble);
  set length(core.double? value) => writeField('length', value);

  /// 放物線の終わりの勾配。parabola に書く
  core.double? get endGradient => readField('end_gradient', asDouble);
  set endGradient(core.double? value) => writeField('end_gradient', value);

  /// 円弧の半径（メートル）。上に凸のものを正にする。circular_arc に書く
  core.double? get radius => readField('radius', asDouble);
  set radius(core.double? value) => writeField('radius', value);
}

/// 平面線形の区間の種類。直線・円弧・緩和曲線
///
/// 列挙の値。知らない値も受け取る
abstract final class CivilHorizontalKind {
  static const line = 'line';
  static const arc = 'arc';
  static const transition = 'transition';
}

/// 緩和曲線の種類。クロソイド・3 次放物線・サイン半波長逓減曲線・コサイン曲線・ブロス曲線・ヘルマート曲線・ウィーン曲線
///
/// 列挙の値。知らない値も受け取る
abstract final class CivilTransitionCurve {
  static const clothoid = 'clothoid';
  static const cubicParabola = 'cubic_parabola';
  static const sineHalfWave = 'sine_half_wave';
  static const cosine = 'cosine';
  static const bloss = 'bloss';
  static const helmert = 'helmert';
  static const viennese = 'viennese';
}

/// 縦断線形の区間の種類。一定の勾配・2 次放物線・円弧
///
/// 列挙の値。知らない値も受け取る
abstract final class CivilVerticalKind {
  static const constantGradient = 'constant_gradient';
  static const parabola = 'parabola';
  static const circularArc = 'circular_arc';
}

/// 測点の不連続
class CivilStationBreak extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilStationBreak.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilStationBreak() : super({});

  /// 追加距離（メートル）
  core.double? get distance => readField('distance', asDouble);
  set distance(core.double? value) => writeField('distance', value);

  /// 戻りの測点の値（不連続の手前から数えた値）
  core.double? get back => readField('back', asDouble);
  set back(core.double? value) => writeField('back', value);

  /// 進みの測点の値（不連続の先で数え直す値）
  core.double? get ahead => readField('ahead', asDouble);
  set ahead(core.double? value) => writeField('ahead', value);
}

/// 線形の上の所
class CivilResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilResult() : super({});

  /// details を求めた場合の保持値。マイナーバージョン 87 から
  CivilDetails? get details => readField('details', objectOf(CivilDetails.fromJson));
  set details(CivilDetails? value) => writeField('details', value);

  /// 平面線形の長さ（メートル）
  core.double? get length => readField('length', asDouble);
  set length(core.double? value) => writeField('length', value);

  /// 求めた所
  List<CivilPosition>? get positions => readField('positions', listOf(objectOf(CivilPosition.fromJson)));
  set positions(List<CivilPosition>? value) => writeField('positions', value);

  /// 所を求めた doc の線形の ID。マイナーバージョン 47 から
  String? get alignment => readField('alignment', asString);
  set alignment(String? value) => writeField('alignment', value);

  /// doc の線形の要約。doc を渡したときに返す。マイナーバージョン 47 から
  List<CivilAlignmentSummary>? get alignments => readField('alignments', listOf(objectOf(CivilAlignmentSummary.fromJson)));
  set alignments(List<CivilAlignmentSummary>? value) => writeField('alignments', value);

  /// doc の地表面（TIN）の要約。doc を渡したときに返す。マイナーバージョン 47 から
  List<CivilSurfaceSummary>? get surfaces => readField('surfaces', listOf(objectOf(CivilSurfaceSummary.fromJson)));
  set surfaces(List<CivilSurfaceSummary>? value) => writeField('surfaces', value);

  /// doc の水平の座標系の EPSG のコード。分からなければ書かない。マイナーバージョン 47 から
  core.int? get crs => readField('crs', asInt);
  set crs(core.int? value) => writeField('crs', value);
}

/// 土木モデルの保持値。スキーマ 1.87 から
class CivilDetails extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilDetails.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilDetails() : super({});

  /// 選んだ線形の値。線形がなければ書かない
  CivilAlignmentValues? get alignment => readField('alignment', objectOf(CivilAlignmentValues.fromJson));
  set alignment(CivilAlignmentValues? value) => writeField('alignment', value);

  /// 文書の座標点セット
  List<CivilPointSet>? get pointSets => readField('point_sets', listOf(objectOf(CivilPointSet.fromJson)));
  set pointSets(List<CivilPointSet>? value) => writeField('point_sets', value);

  /// 文書の高さの基準。値は自動で T.P. に直さない
  CivilHeightReference? get heightReference => readField('height_reference', objectOf(CivilHeightReference.fromJson));
  set heightReference(CivilHeightReference? value) => writeField('height_reference', value);
}

/// 元の高さの基準
class CivilHeightReference extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilHeightReference.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilHeightReference() : super({});

  /// 元の基準名
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 元の高さに足すと T.P. の高さになる差（メートル）。不明なら書かない
  core.double? get tokyoBayOffset => readField('tokyo_bay_offset', asDouble);
  set tokyoBayOffset(core.double? value) => writeField('tokyo_bay_offset', value);

  /// 元の位置
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 線形の付随する値。元の順序を保つ
class CivilAlignmentValues extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilAlignmentValues.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilAlignmentValues() : super({});

  /// 同じ名前の縦断が複数あり、名前で参照先を一意に定められない
  core.bool? get verticalNameAmbiguous => readField('vertical_name_ambiguous', asBool);
  set verticalNameAmbiguous(core.bool? value) => writeField('vertical_name_ambiguous', value);

  /// 選んだ線形の ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 保持した縦断線形の元の名前
  String? get verticalName => readField('vertical_name', asString);
  set verticalName(String? value) => writeField('vertical_name', value);

  /// IP の列。始点・終点を含む
  List<CivilSurveyPoint>? get intersectionPoints => readField('intersection_points', listOf(objectOf(CivilSurveyPoint.fromJson)));
  set intersectionPoints(List<CivilSurveyPoint>? value) => writeField('intersection_points', value);

  /// 地盤の縦断の折れ線
  List<CivilGroundProfile>? get groundProfiles => readField('ground_profiles', listOf(objectOf(CivilGroundProfile.fromJson)));
  set groundProfiles(List<CivilGroundProfile>? value) => writeField('ground_profiles', value);

  /// 片勾配とカントの区間
  List<CivilCant>? get cant => readField('cant', listOf(objectOf(CivilCant.fromJson)));
  set cant(List<CivilCant>? value) => writeField('cant', value);

  /// 横断の線と点
  List<CivilCrossSection>? get crossSections => readField('cross_sections', listOf(objectOf(CivilCrossSection.fromJson)));
  set crossSections(List<CivilCrossSection>? value) => writeField('cross_sections', value);
}

/// IP・基準点などの座標点
class CivilSurveyPoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilSurveyPoint.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilSurveyPoint() : super({});

  /// 元の名前。永続 ID と同一視しない
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 注記
  String? get description => readField('description', asString);
  set description(String? value) => writeField('description', value);

  /// 北の座標（メートル）
  core.double? get northing => readField('northing', asDouble);
  set northing(core.double? value) => writeField('northing', value);

  /// 東の座標（メートル）
  core.double? get easting => readField('easting', asDouble);
  set easting(core.double? value) => writeField('easting', value);

  /// 文書の基準での標高（メートル）。省略は 0 にしない
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 基準点・水準点の種類
  String? get classField => readField('class', asString);
  set classField(String? value) => writeField('class', value);

  /// 元の位置
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 元の順序を持つ座標点セット
class CivilPointSet extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilPointSet.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilPointSet() : super({});

  /// 元の名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 注記
  String? get description => readField('description', asString);
  set description(String? value) => writeField('description', value);

  /// 点
  List<CivilSurveyPoint>? get points => readField('points', listOf(objectOf(CivilSurveyPoint.fromJson)));
  set points(List<CivilSurveyPoint>? value) => writeField('points', value);

  /// 元の位置
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 地盤の縦断線
class CivilGroundProfile extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilGroundProfile.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilGroundProfile() : super({});

  /// 元の名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 注記
  String? get description => readField('description', asString);
  set description(String? value) => writeField('description', value);

  /// 追加距離と高さ
  List<CivilProfilePoint>? get points => readField('points', listOf(objectOf(CivilProfilePoint.fromJson)));
  set points(List<CivilProfilePoint>? value) => writeField('points', value);

  /// 元の位置
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 地盤縦断の点
class CivilProfilePoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilProfilePoint.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilProfilePoint() : super({});

  /// 線形の初めからの追加距離（メートル）
  core.double? get distance => readField('distance', asDouble);
  set distance(core.double? value) => writeField('distance', value);

  /// 文書の基準での標高（メートル）
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);
}

/// 片勾配またはカントの区間
class CivilCant extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilCant.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilCant() : super({});

  /// 初めの追加距離（メートル）
  core.double? get startDistance => readField('start_distance', asDouble);
  set startDistance(core.double? value) => writeField('start_distance', value);

  /// 区間の長さ（メートル）
  core.double? get length => readField('length', asDouble);
  set length(core.double? value) => writeField('length', value);

  /// superelevation は道路の左右の勾配（%）、rail は鉄道の左右の高さの差（mm）
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 初めの左と右
  List<core.double>? get start => readField('start', listOf(asDouble));
  set start(List<core.double>? value) => writeField('start', value);

  /// 終わりの左と右
  List<core.double>? get end => readField('end', listOf(asDouble));
  set end(List<core.double>? value) => writeField('end', value);

  /// 鉄道の軌間（mm）。不明なら書かない
  core.double? get gauge => readField('gauge', asDouble);
  set gauge(core.double? value) => writeField('gauge', value);

  /// 変わり方。元の値
  String? get transition => readField('transition', asString);
  set transition(String? value) => writeField('transition', value);

  /// 元の位置
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 横断
class CivilCrossSection extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilCrossSection.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilCrossSection() : super({});

  /// 追加距離（メートル）
  core.double? get distance => readField('distance', asDouble);
  set distance(core.double? value) => writeField('distance', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 参照する縦断線形の元の名前
  String? get profile => readField('profile', asString);
  set profile(String? value) => writeField('profile', value);

  /// 横断の線
  List<CivilCrossSectionLine>? get lines => readField('lines', listOf(objectOf(CivilCrossSectionLine.fromJson)));
  set lines(List<CivilCrossSectionLine>? value) => writeField('lines', value);

  /// 元の位置
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 横断の線。点の元の値を保つ
class CivilCrossSectionLine extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilCrossSectionLine.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilCrossSectionLine() : super({});

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 注記
  String? get description => readField('description', asString);
  set description(String? value) => writeField('description', value);

  /// 幅員中心から見た側。元の値。離れの符号は変えない
  String? get side => readField('side', asString);
  set side(String? value) => writeField('side', value);

  /// existing、design または元の値
  String? get role => readField('role', asString);
  set role(String? value) => writeField('role', value);

  /// elevation は文書の標高、formation_offset はその所の計画高からの差
  String? get heightReference => readField('height_reference', asString);
  set heightReference(String? value) => writeField('height_reference', value);

  /// 離れと元の高さ
  List<CivilCrossSectionPoint>? get points => readField('points', listOf(objectOf(CivilCrossSectionPoint.fromJson)));
  set points(List<CivilCrossSectionPoint>? value) => writeField('points', value);

  /// 元の位置
  Source? get source => readField('source', objectOf(Source.fromJson));
  set source(Source? value) => writeField('source', value);
}

/// 横断の点
class CivilCrossSectionPoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilCrossSectionPoint.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilCrossSectionPoint() : super({});

  /// 中心線からの離れ（右を正、メートル）
  core.double? get offset => readField('offset', asDouble);
  set offset(core.double? value) => writeField('offset', value);

  /// 線の height_reference に従う高さ（メートル）
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);
}

/// 線形の要約
class CivilAlignmentSummary extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilAlignmentSummary.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilAlignmentSummary() : super({});

  /// 線形の ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 平面線形の長さ（メートル）
  core.double? get length => readField('length', asDouble);
  set length(core.double? value) => writeField('length', value);

  /// 線形の初めの測点の値（メートル）
  core.double? get startStation => readField('start_station', asDouble);
  set startStation(core.double? value) => writeField('start_station', value);

  /// 平面線形の区間の数
  core.int? get horizontal => readField('horizontal', asInt);
  set horizontal(core.int? value) => writeField('horizontal', value);

  /// 縦断線形の区間の数
  core.int? get vertical => readField('vertical', asInt);
  set vertical(core.int? value) => writeField('vertical', value);

  /// 測点の不連続の数
  core.int? get breaks => readField('breaks', asInt);
  set breaks(core.int? value) => writeField('breaks', value);

  /// 横断の数
  core.int? get crossSections => readField('cross_sections', asInt);
  set crossSections(core.int? value) => writeField('cross_sections', value);
}

/// 地表面（TIN）の要約
class CivilSurfaceSummary extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilSurfaceSummary.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilSurfaceSummary() : super({});

  /// 地表面の ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 役割。existing（現況）・design（計画）か、元の形式の値
  String? get role => readField('role', asString);
  set role(String? value) => writeField('role', value);

  /// 点の数
  core.int? get points => readField('points', asInt);
  set points(core.int? value) => writeField('points', value);

  /// 三角形の数
  core.int? get triangles => readField('triangles', asInt);
  set triangles(core.int? value) => writeField('triangles', value);
}

/// 線形の上の 1 つの所
class CivilPosition extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilPosition.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilPosition() : super({});

  /// 追加距離（メートル）
  core.double? get distance => readField('distance', asDouble);
  set distance(core.double? value) => writeField('distance', value);

  /// 測点の値（メートル）
  core.double? get station => readField('station', asDouble);
  set station(core.double? value) => writeField('station', value);

  /// 測点の表記。表記にできなければ書かない
  String? get label => readField('label', asString);
  set label(String? value) => writeField('label', value);

  /// 点の北（メートル）
  core.double? get northing => readField('northing', asDouble);
  set northing(core.double? value) => writeField('northing', value);

  /// 点の東（メートル）
  core.double? get easting => readField('easting', asDouble);
  set easting(core.double? value) => writeField('easting', value);

  /// 接線の方向角（北から時計回りのラジアン）
  core.double? get direction => readField('direction', asDouble);
  set direction(core.double? value) => writeField('direction', value);

  /// 縦断線形の高さ（メートル）。求められなければ書かない
  core.double? get height => readField('height', asDouble);
  set height(core.double? value) => writeField('height', value);

  /// 縦断線形の勾配。高さと同じく、求められなければ書かない
  core.double? get gradient => readField('gradient', asDouble);
  set gradient(core.double? value) => writeField('gradient', value);
}

/// open で開いた点群（LAS・LAZ・PLY・XYZ・E57・GeoTIFF の DEM）の要約と、条件に合う点を返す。点は八分木の浅い段のノードから順に選ぶ（決定 0232）。点群のモジュールに置く（決定 0253）。マイナーバージョン 48 から
class PointcloudRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointcloudRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  PointcloudRequest() : super({'op': 'pointcloud'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// open で開いた点群の文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 点を選ぶ範囲。書かなければ全体
  PointcloudBox? get region => readField('region', objectOf(PointcloudBox.fromJson));
  set region(PointcloudBox? value) => writeField('region', value);

  /// 許容差（点の間隔、メートル）。これより細かい段の点を選ばない。書かなければすべての段
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 返す点の数の上限。書かなければ点を返さず、要約だけを返す
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);
}

/// 地図の空間の箱。北・東・標高（メートル）の最小と最大
class PointcloudBox extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointcloudBox.fromJson(super.json);

  /// 空のオブジェクトから作る
  PointcloudBox() : super({});

  /// 最小の北・東・標高
  List<core.double>? get min => readField('min', listOf(asDouble));
  set min(List<core.double>? value) => writeField('min', value);

  /// 最大の北・東・標高
  List<core.double>? get max => readField('max', listOf(asDouble));
  set max(List<core.double>? value) => writeField('max', value);
}

/// 点群の要約と、選んだ点
class PointcloudResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointcloudResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  PointcloudResult() : super({});

  /// 点の数
  core.int? get points => readField('points', asInt);
  set points(core.int? value) => writeField('points', value);

  /// 点の範囲。点がなければ書かない
  PointcloudBox? get bounds => readField('bounds', objectOf(PointcloudBox.fromJson));
  set bounds(PointcloudBox? value) => writeField('bounds', value);

  /// 水平の座標系の EPSG のコード。分からなければ書かない
  core.int? get crs => readField('crs', asInt);
  set crs(core.int? value) => writeField('crs', value);

  /// 高さの座標系の EPSG のコード。分からなければ書かない
  core.int? get vertical => readField('vertical', asInt);
  set vertical(core.int? value) => writeField('vertical', value);

  /// 点が持つ属性（position・intensity・classification・color・returns・time）
  List<String>? get attributes => readField('attributes', listOf(asString));
  set attributes(List<String>? value) => writeField('attributes', value);

  /// 八分木のいちばん深い段
  core.int? get depth => readField('depth', asInt);
  set depth(core.int? value) => writeField('depth', value);

  /// 八分木のノードの数
  core.int? get nodes => readField('nodes', asInt);
  set nodes(core.int? value) => writeField('nodes', value);

  /// 選んだ点。limit を書いたときに返す
  PointcloudPoints? get selected => readField('selected', objectOf(PointcloudPoints.fromJson));
  set selected(PointcloudPoints? value) => writeField('selected', value);
}

/// 選んだ点。どの列も、選んだ点の順で同じ長さにする
class PointcloudPoints extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointcloudPoints.fromJson(super.json);

  /// 空のオブジェクトから作る
  PointcloudPoints() : super({});

  /// 北・東・標高（メートル）
  List<List<core.double>>? get positions => readField('positions', listOf(listOf(asDouble)));
  set positions(List<List<core.double>>? value) => writeField('positions', value);

  /// 強度。点群が持つときに返す
  List<core.int>? get intensity => readField('intensity', listOf(asInt));
  set intensity(List<core.int>? value) => writeField('intensity', value);

  /// 分類。点群が持つときに返す
  List<core.int>? get classification => readField('classification', listOf(asInt));
  set classification(List<core.int>? value) => writeField('classification', value);

  /// 色（赤・緑・青。16 ビット）。点群が持つときに返す
  List<List<core.int>>? get color => readField('color', listOf(listOf(asInt)));
  set color(List<List<core.int>>? value) => writeField('color', value);

  /// limit のために選ばなかった点がある
  core.bool? get truncated => readField('truncated', asBool);
  set truncated(core.bool? value) => writeField('truncated', value);

  /// 選んだいちばん深い段
  core.int? get level => readField('level', asInt);
  set level(core.int? value) => writeField('level', value);
}

/// 管理 XML の項目を変更し、file blob を返す。入力は変更しない。すべての経路は元の文書で解き、失敗時は結果を返さない。図面のモジュールに置く。マイナーバージョン 84 から
class DeliveryApplyRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryApplyRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  DeliveryApplyRequest() : super({'op': 'delivery_apply'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 管理 XML の入力の名前
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);

  /// 元の XML の文書ハッシュ。違えば変更しない。省略すれば検査しない
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 指定する順の変更。同じ親への追加は順序を保つ
  List<DeliveryEdit>? get edits => readField('edits', listOf(objectOf(DeliveryEdit.fromJson)));
  set edits(List<DeliveryEdit>? value) => writeField('edits', value);

  /// 版の照合に使うプロファイルと継承元の入力名
  List<String>? get profiles => readField('profiles', listOf(asString));
  set profiles(List<String>? value) => writeField('profiles', value);

  /// 編集後に照らす基準の系統と版。省略すると編集だけを行う
  ProfileRef? get profile => readField('profile', objectOf(ProfileRef.fromJson));
  set profile(ProfileRef? value) => writeField('profile', value);
}

/// 変更後の管理 XML の情報。file blob は 1 個。入力は変更しない
class DeliveryApplyResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryApplyResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeliveryApplyResult() : super({});

  /// 元の入力の名前
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);

  /// 変更後のバイト列の文書ハッシュ
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 適用した変更の数
  core.int? get applied => readField('applied', asInt);
  set applied(core.int? value) => writeField('applied', value);

  /// 版を指定したときの部分照合。違反があっても編集後のファイルを返す
  DeliveryEditValidation? get validation => readField('validation', objectOf(DeliveryEditValidation.fromJson));
  set validation(DeliveryEditValidation? value) => writeField('validation', value);
}

/// 指定版で編集した箇所を照らした結果。マイナーバージョン 115 から
class DeliveryEditValidation extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryEditValidation.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeliveryEditValidation() : super({});

  /// 解決したプロファイルを継承元から並べた列
  List<ProfileIdentity>? get chain => readField('chain', listOf(objectOf(ProfileIdentity.fromJson)));
  set chain(List<ProfileIdentity>? value) => writeField('chain', value);

  /// 変更と規則の順に並べた結果
  List<DeliveryEditCheck>? get checks => readField('checks', listOf(objectOf(DeliveryEditCheck.fromJson)));
  set checks(List<DeliveryEditCheck>? value) => writeField('checks', value);

  /// 照合の対象外。子の順序と最小数、属性、追加した部分木の内容、変更していない項目
  List<String>? get unchecked => readField('unchecked', listOf(asString));
  set unchecked(List<String>? value) => writeField('unchecked', value);
}

/// 変更に対する版ごとの規則と出力 XML のバイト範囲
class DeliveryEditCheck extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryEditCheck.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeliveryEditCheck() : super({});

  /// edits の位置。0 から数える
  core.int? get edit => readField('edit', asInt);
  set edit(core.int? value) => writeField('edit', value);

  /// matched・violated・unchecked のいずれか
  String? get status => readField('status', asString);
  set status(String? value) => writeField('status', value);

  /// 照らした text・child_count、または確認できない理由
  String? get reason => readField('reason', asString);
  set reason(String? value) => writeField('reason', value);

  /// 照らした規則の ID。該当する規則がなければ省略する
  String? get rule => readField('rule', asString);
  set rule(String? value) => writeField('rule', value);

  /// 変更後の対象要素の最初のバイト
  core.int? get start => readField('start', asInt);
  set start(core.int? value) => writeField('start', value);

  /// 変更後の対象要素の末尾の次のバイト
  core.int? get end => readField('end', asInt);
  set end(core.int? value) => writeField('end', value);
}

/// 根からたどる子。接頭辞を含む名前で区別する
class DeliveryPathStep extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryPathStep.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeliveryPathStep() : super({});

  /// 接頭辞を含む元の要素名
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 同名の兄弟の中の位置。0 から数え、省略すると 0
  core.int? get index => readField('index', asInt);
  set index(core.int? value) => writeField('index', value);
}

/// 親の名前空間を引き継ぐ追加要素。属性と名前空間宣言は追加しない
class DeliveryNewElement extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryNewElement.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeliveryNewElement() : super({});

  /// 接頭辞を含む要素名。接頭辞は親で宣言済みのものに限る
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 子の前に書く文字。文字参照にする前の値
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);

  /// 子の要素。書く順。省略すると空
  List<DeliveryNewElement>? get children => readField('children', listOf(objectOf(DeliveryNewElement.fromJson)));
  set children(List<DeliveryNewElement>? value) => writeField('children', value);
}

/// 元の文書の項目の更新か、子の記録の追加。空の path は根を指す。同じ項目の更新を重ねたり、更新と追加を混ぜたりしない
class DeliveryEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryEdit.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeliveryEdit() : super({});

  /// kind が set_text の形なら、そのビュー。違えば null
  DeliverySetText? get asDeliverySetText => taggedView('kind', 'set_text', DeliverySetText.fromJson);

  /// kind が append の形なら、そのビュー。違えば null
  DeliveryAppend? get asDeliveryAppend => taggedView('kind', 'append', DeliveryAppend.fromJson);
}

/// 子の要素を持たない項目の文字を置き換える。注記と処理命令は保つ
class DeliverySetText extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliverySetText.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  DeliverySetText() : super({'kind': 'set_text'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 根からの経路
  List<DeliveryPathStep>? get path => readField('path', listOf(objectOf(DeliveryPathStep.fromJson)));
  set path(List<DeliveryPathStep>? value) => writeField('path', value);

  /// 文字参照にする前の値
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);
}

/// 親の末尾に子の記録を足す。名前・文字コード・予算を検査する
class DeliveryAppend extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryAppend.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  DeliveryAppend() : super({'kind': 'append'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 親の根からの経路
  List<DeliveryPathStep>? get path => readField('path', listOf(objectOf(DeliveryPathStep.fromJson)));
  set path(List<DeliveryPathStep>? value) => writeField('path', value);

  /// 追加する記録の木
  DeliveryNewElement? get child => readField('child', objectOf(DeliveryNewElement.fromJson));
  set child(DeliveryNewElement? value) => writeField('child', value);
}

/// 電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。マイナーバージョン 53 から
class DeliveryCheckRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryCheckRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  DeliveryCheckRequest() : super({'op': 'delivery_check'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 成果品の起点。/ で終わる入力の名前の前の部分（納品/ など）。archive と一緒には書かない
  String? get root => readField('root', asString);
  set root(String? value) => writeField('root', value);

  /// 成果品の ZIP の入力の名前。root と一緒には書かない
  String? get archive => readField('archive', asString);
  set archive(String? value) => writeField('archive', value);

  /// プロファイルの入力の名前の列。extends で指す先も、すべて渡す
  List<String>? get profiles => readField('profiles', listOf(asString));
  set profiles(List<String>? value) => writeField('profiles', value);

  /// 照合に使うプロファイル。profiles の中から選ぶ
  ProfileRef? get profile => readField('profile', objectOf(ProfileRef.fromJson));
  set profile(ProfileRef? value) => writeField('profile', value);

  /// 返す指摘の初めの位置
  core.int? get offset => readField('offset', asInt);
  set offset(core.int? value) => writeField('offset', value);

  /// 返す指摘の数の上限。書かなければ 10000
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);
}

/// 成果品の要約と、照合の結果。照合の結果は check と同じ形
class DeliveryCheckResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryCheckResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeliveryCheckResult() : super({});

  /// 成果品のファイルの数
  core.int? get files => readField('files', asInt);
  set files(core.int? value) => writeField('files', value);

  /// 管理ファイルの要約。名前の順
  List<DeliveryManaged>? get managed => readField('managed', listOf(objectOf(DeliveryManaged.fromJson)));
  set managed(List<DeliveryManaged>? value) => writeField('managed', value);

  /// 照合に使ったプロファイルの鎖。基から順に。プロファイルを渡さなければ空
  List<ProfileIdentity>? get chain => readField('chain', listOf(objectOf(ProfileIdentity.fromJson)));
  set chain(List<ProfileIdentity>? value) => writeField('chain', value);

  /// 規則ごとの指摘の数
  List<CheckRule>? get rules => readField('rules', listOf(objectOf(CheckRule.fromJson)));
  set rules(List<CheckRule>? value) => writeField('rules', value);

  /// 指摘
  List<CheckFinding>? get findings => readField('findings', listOf(objectOf(CheckFinding.fromJson)));
  set findings(List<CheckFinding>? value) => writeField('findings', value);

  /// 指摘の数の全体
  core.int? get total => readField('total', asInt);
  set total(core.int? value) => writeField('total', value);

  /// 続きの指摘の位置。続きがなければ書かない
  core.int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(core.int? value) => writeField('next_offset', value);
}

/// 工事写真（JPEG）の Exif と XMP を読み、画素数・向き・撮影の日時・位置などを返す（#387）。画像は復号しない。図面のモジュールに置く。マイナーバージョン 55 から
class PhotoRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PhotoRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  PhotoRequest() : super({'op': 'photo'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// JPEG の入力の名前
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);
}

/// 写真の情報。ファイルが持たない値は書かない
class PhotoResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PhotoResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  PhotoResult() : super({});

  /// 画素の幅（フレームのヘッダー）
  core.int? get width => readField('width', asInt);
  set width(core.int? value) => writeField('width', value);

  /// 画素の高さ（フレームのヘッダー）
  core.int? get height => readField('height', asInt);
  set height(core.int? value) => writeField('height', value);

  /// 向き（Exif の Orientation、1〜8）
  core.int? get orientation => readField('orientation', asInt);
  set orientation(core.int? value) => writeField('orientation', value);

  /// 撮影の日時（YYYY-MM-DDTHH:MM:SS、時差があれば +09:00 などを足す）
  String? get taken => readField('taken', asString);
  set taken(String? value) => writeField('taken', value);

  /// 機器の作り手
  String? get make => readField('make', asString);
  set make(String? value) => writeField('make', value);

  /// 機器の型
  String? get model => readField('model', asString);
  set model(String? value) => writeField('model', value);

  /// 撮った位置（GPS の緯度・経度、度）。測地系は換算しない
  PhotoPosition? get position => readField('position', objectOf(PhotoPosition.fromJson));
  set position(PhotoPosition? value) => writeField('position', value);

  /// 撮った向き（度）
  core.double? get direction => readField('direction', asDouble);
  set direction(core.double? value) => writeField('direction', value);

  /// 向きの基準（T は真北、M は磁北）
  String? get directionReference => readField('direction_reference', asString);
  set directionReference(String? value) => writeField('direction_reference', value);

  /// GPS の測地系の名前（GPSMapDatum）
  String? get datum => readField('datum', asString);
  set datum(String? value) => writeField('datum', value);

  /// XMP の性質。文書の順
  List<PhotoProperty>? get xmp => readField('xmp', listOf(objectOf(PhotoProperty.fromJson)));
  set xmp(List<PhotoProperty>? value) => writeField('xmp', value);
}

/// 撮った位置
class PhotoPosition extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PhotoPosition.fromJson(super.json);

  /// 空のオブジェクトから作る
  PhotoPosition() : super({});

  /// 緯度（度、北が正）
  core.double? get latitude => readField('latitude', asDouble);
  set latitude(core.double? value) => writeField('latitude', value);

  /// 経度（度、東が正）
  core.double? get longitude => readField('longitude', asDouble);
  set longitude(core.double? value) => writeField('longitude', value);

  /// 高さ（m、海面より下は負）
  core.double? get altitude => readField('altitude', asDouble);
  set altitude(core.double? value) => writeField('altitude', value);
}

/// XMP の性質
class PhotoProperty extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PhotoProperty.fromJson(super.json);

  /// 空のオブジェクトから作る
  PhotoProperty() : super({});

  /// 名前空間の URI
  String? get namespace => readField('namespace', asString);
  set namespace(String? value) => writeField('namespace', value);

  /// 接頭辞を除いた名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 値。配列は最初の項目の値
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);
}

/// 地質・土質調査のボーリング交換用データ（BED0500.DTD の XML）を読み、ボーリングの位置・標高と、地層・色調・観察記事・標準貫入試験・孔内水位を返す（#390）。ほかの試験と判定表は読まない。図面のモジュールに置く。マイナーバージョン 57 から
class BoringRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BoringRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  BoringRequest() : super({'op': 'boring'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// ボーリング交換用データの入力の名前
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);
}

/// 読んだボーリング。深さは地表からの m（GL.-m）
class BoringResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BoringResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  BoringResult() : super({});

  /// 適用規格。文書の順
  List<String>? get standards => readField('standards', listOf(asString));
  set standards(List<String>? value) => writeField('standards', value);

  /// 公開フラグのコード
  String? get publication => readField('publication', asString);
  set publication(String? value) => writeField('publication', value);

  /// 事業工事名
  String? get project => readField('project', asString);
  set project(String? value) => writeField('project', value);

  /// 調査名
  String? get survey => readField('survey', asString);
  set survey(String? value) => writeField('survey', value);

  /// ボーリング名
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// ボーリング総数
  core.int? get count => readField('count', asInt);
  set count(core.int? value) => writeField('count', value);

  /// ボーリング連番
  core.int? get serial => readField('serial', asInt);
  set serial(core.int? value) => writeField('serial', value);

  /// 孔口の位置。測地系は換算しない
  BoringPosition? get position => readField('position', objectOf(BoringPosition.fromJson));
  set position(BoringPosition? value) => writeField('position', value);

  /// 位置の測地系
  String? get datum => readField('datum', asString);
  set datum(String? value) => writeField('datum', value);

  /// 調査位置の住所
  String? get address => readField('address', asString);
  set address(String? value) => writeField('address', value);

  /// 調査期間の始まりの日（YYYY-MM-DD）
  String? get start => readField('start', asString);
  set start(String? value) => writeField('start', value);

  /// 調査期間の終わりの日（YYYY-MM-DD）
  String? get end => readField('end', asString);
  set end(String? value) => writeField('end', value);

  /// 孔口標高
  BoringHeight? get elevation => readField('elevation', objectOf(BoringHeight.fromJson));
  set elevation(BoringHeight? value) => writeField('elevation', value);

  /// 総削孔長（m）
  core.double? get length => readField('length', asDouble);
  set length(core.double? value) => writeField('length', value);

  /// 削孔の角度（度）
  core.double? get angle => readField('angle', asDouble);
  set angle(core.double? value) => writeField('angle', value);

  /// 削孔の方位（度）
  core.double? get direction => readField('direction', asDouble);
  set direction(core.double? value) => writeField('direction', value);

  /// 工学的地質区分名・現場土質名。文書の順
  List<BoringLayer>? get layers => readField('layers', listOf(objectOf(BoringLayer.fromJson)));
  set layers(List<BoringLayer>? value) => writeField('layers', value);

  /// 色調。文書の順
  List<BoringColor>? get colors => readField('colors', listOf(objectOf(BoringColor.fromJson)));
  set colors(List<BoringColor>? value) => writeField('colors', value);

  /// 観察記事。文書の順
  List<BoringObservation>? get observations => readField('observations', listOf(objectOf(BoringObservation.fromJson)));
  set observations(List<BoringObservation>? value) => writeField('observations', value);

  /// 標準貫入試験。文書の順
  List<BoringPenetration>? get penetrations => readField('penetrations', listOf(objectOf(BoringPenetration.fromJson)));
  set penetrations(List<BoringPenetration>? value) => writeField('penetrations', value);

  /// 孔内水位。文書の順
  List<BoringWaterLevel>? get waterLevels => readField('water_levels', listOf(objectOf(BoringWaterLevel.fromJson)));
  set waterLevels(List<BoringWaterLevel>? value) => writeField('water_levels', value);
}

/// 経度・緯度（度）。度・分・秒から求める
class BoringPosition extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BoringPosition.fromJson(super.json);

  /// 空のオブジェクトから作る
  BoringPosition() : super({});

  /// 緯度（度、北が正）
  core.double? get latitude => readField('latitude', asDouble);
  set latitude(core.double? value) => writeField('latitude', value);

  /// 経度（度、東が正）
  core.double? get longitude => readField('longitude', asDouble);
  set longitude(core.double? value) => writeField('longitude', value);
}

/// 測地系。tokyo は日本測地系（コード 00）、jgd2000・jgd2011・jgd2024 は世界測地系（コード 01〜03）
///
/// 列挙の値。知らない値も受け取る
abstract final class BoringDatum {
  static const tokyo = 'tokyo';
  static const jgd2000 = 'jgd2000';
  static const jgd2011 = 'jgd2011';
  static const jgd2024 = 'jgd2024';
}

/// 標高と、その基準と成果（決定 0019）
class BoringHeight extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BoringHeight.fromJson(super.json);

  /// 空のオブジェクトから作る
  BoringHeight() : super({});

  /// 標高（m）
  core.double? get value => readField('value', asDouble);
  set value(core.double? value) => writeField('value', value);

  /// 高さの基準。孔口標高は tokyo_bay（東京湾平均海面）とする
  String? get datum => readField('datum', asString);
  set datum(String? value) => writeField('datum', value);

  /// 成果。測地系が jgd2024 なら survey_2024、そうでなければ調査の日から仮定する
  String? get realization => readField('realization', asString);
  set realization(String? value) => writeField('realization', value);
}

/// 高さの基準（決定 0019）。tokyo_bay は東京湾平均海面（T.P.）、island は離島の独自の平均海面、ellipsoid は楕円体（GRS80）
///
/// 列挙の値。知らない値も受け取る
abstract final class HeightDatum {
  static const tokyoBay = 'tokyo_bay';
  static const island = 'island';
  static const ellipsoid = 'ellipsoid';
}

/// 標高の成果（決定 0019）。survey_2011 は測地成果2011（2025-03-31 まで）、survey_2024 は測地成果2024（2025-04-01 から）
///
/// 列挙の値。知らない値も受け取る
abstract final class HeightRealization {
  static const survey2011 = 'survey_2011';
  static const survey2024 = 'survey_2024';
}

/// 地層の区間。上端は 1 つ前の区間の下端（最初は 0）
class BoringLayer extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BoringLayer.fromJson(super.json);

  /// 空のオブジェクトから作る
  BoringLayer() : super({});

  /// 下端の深さ（m）
  core.double? get bottom => readField('bottom', asDouble);
  set bottom(core.double? value) => writeField('bottom', value);

  /// 工学的地質区分名・現場土質名
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 記号
  String? get symbol => readField('symbol', asString);
  set symbol(String? value) => writeField('symbol', value);
}

/// 色調の区間
class BoringColor extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BoringColor.fromJson(super.json);

  /// 空のオブジェクトから作る
  BoringColor() : super({});

  /// 下端の深さ（m）
  core.double? get bottom => readField('bottom', asDouble);
  set bottom(core.double? value) => writeField('bottom', value);

  /// 色調の名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);
}

/// 観察記事
class BoringObservation extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BoringObservation.fromJson(super.json);

  /// 空のオブジェクトから作る
  BoringObservation() : super({});

  /// 上端の深さ（m）
  core.double? get top => readField('top', asDouble);
  set top(core.double? value) => writeField('top', value);

  /// 下端の深さ（m）
  core.double? get bottom => readField('bottom', asDouble);
  set bottom(core.double? value) => writeField('bottom', value);

  /// 記事
  String? get text => readField('text', asString);
  set text(String? value) => writeField('text', value);
}

/// 標準貫入試験
class BoringPenetration extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BoringPenetration.fromJson(super.json);

  /// 空のオブジェクトから作る
  BoringPenetration() : super({});

  /// 開始の深さ（m）
  core.double? get depth => readField('depth', asDouble);
  set depth(core.double? value) => writeField('depth', value);

  /// 合計の打撃回数
  core.int? get blows => readField('blows', asInt);
  set blows(core.int? value) => writeField('blows', value);

  /// 合計の貫入量（mm）
  core.double? get penetration => readField('penetration', asDouble);
  set penetration(core.double? value) => writeField('penetration', value);
}

/// 孔内水位
class BoringWaterLevel extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BoringWaterLevel.fromJson(super.json);

  /// 空のオブジェクトから作る
  BoringWaterLevel() : super({});

  /// 測定の日（YYYY-MM-DD）
  String? get date => readField('date', asString);
  set date(String? value) => writeField('date', value);

  /// 削孔状況のコード
  String? get state => readField('state', asString);
  set state(String? value) => writeField('state', value);

  /// 水位の深さ（m）。地表より高ければ負。水位がなければ書かない
  core.double? get depth => readField('depth', asDouble);
  set depth(core.double? value) => writeField('depth', value);

  /// 水位種別・備考
  String? get note => readField('note', asString);
  set note(String? value) => writeField('note', value);
}

/// 管理ファイルの要約
class DeliveryManaged extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryManaged.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeliveryManaged() : super({});

  /// 成果品の中の名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 文書型宣言の DTD の名前
  String? get doctype => readField('doctype', asString);
  set doctype(String? value) => writeField('doctype', value);

  /// 根の要素の名前
  String? get root => readField('root', asString);
  set root(String? value) => writeField('root', value);

  /// 要素の記録の数
  core.int? get records => readField('records', asInt);
  set records(core.int? value) => writeField('records', value);

  /// ファイルの名前の形の値の数
  core.int? get links => readField('links', asInt);
  set links(core.int? value) => writeField('links', value);

  /// 成果品の中に見つからないファイルの名前。書いた順
  List<String>? get missing => readField('missing', listOf(asString));
  set missing(List<String>? value) => writeField('missing', value);
}

/// open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。土木・地理のモジュールに置く（決定 0253）。マイナーバージョン 52 から
class GeoRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  GeoRequest() : super({'op': 'geo'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 中身を返す地物の ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);
}

/// 地理の領域モデルの要約
class GeoResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  GeoResult() : super({});

  /// 平面の座標系の EPSG のコード。分からなければ書かない
  core.int? get crs => readField('crs', asInt);
  set crs(core.int? value) => writeField('crs', value);

  /// 高さの座標系の EPSG のコード。分からなければ書かない
  core.int? get height => readField('height', asInt);
  set height(core.int? value) => writeField('height', value);

  /// 地物の数
  core.int? get features => readField('features', asInt);
  set features(core.int? value) => writeField('features', value);

  /// 地物の型ごとの数。型の名前の順
  List<GeoKind>? get kinds => readField('kinds', listOf(objectOf(GeoKind.fromJson)));
  set kinds(List<GeoKind>? value) => writeField('kinds', value);

  /// id で指した地物の中身
  GeoFeature? get feature => readField('feature', objectOf(GeoFeature.fromJson));
  set feature(GeoFeature? value) => writeField('feature', value);
}

/// 地物の型と数
class GeoKind extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoKind.fromJson(super.json);

  /// 空のオブジェクトから作る
  GeoKind() : super({});

  /// 型。名前空間の接頭辞をつけた元の名前
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 地物の数
  core.int? get count => readField('count', asInt);
  set count(core.int? value) => writeField('count', value);
}

/// 地物の中身
class GeoFeature extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoFeature.fromJson(super.json);

  /// 空のオブジェクトから作る
  GeoFeature() : super({});

  /// ID
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 型
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 分類コード。元の値の文字のまま
  String? get code => readField('code', asString);
  set code(String? value) => writeField('code', value);

  /// 名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 親の地物の ID
  String? get parent => readField('parent', asString);
  set parent(String? value) => writeField('parent', value);

  /// 子の地物の数
  core.int? get children => readField('children', asInt);
  set children(core.int? value) => writeField('children', value);

  /// 属性。入力の順
  List<GeoAttribute>? get attributes => readField('attributes', listOf(objectOf(GeoAttribute.fromJson)));
  set attributes(List<GeoAttribute>? value) => writeField('attributes', value);

  /// 形。入力の順
  List<GeoGeometry>? get geometries => readField('geometries', listOf(objectOf(GeoGeometry.fromJson)));
  set geometries(List<GeoGeometry>? value) => writeField('geometries', value);
}

/// 属性
class GeoAttribute extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoAttribute.fromJson(super.json);

  /// 空のオブジェクトから作る
  GeoAttribute() : super({});

  /// 名前。接頭辞をつけた元の名前
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 値の型
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 値の文字
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);

  /// 単位つきの量の単位
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// コードの一覧（codeSpace）
  String? get space => readField('space', asString);
  set space(String? value) => writeField('space', value);
}

/// 属性の値の型
///
/// 列挙の値。知らない値も受け取る
abstract final class GeoValueKind {
  static const text = 'text';
  static const integer = 'integer';
  static const real = 'real';
  static const bool = 'bool';
  static const date = 'date';
  static const code = 'code';
  static const measure = 'measure';
}

/// 形の種類
///
/// 列挙の値。知らない値も受け取る
abstract final class GeoForm {
  static const point = 'point';
  static const line = 'line';
  static const area = 'area';
  static const solid = 'solid';
}

/// 形の要約
class GeoGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoGeometry.fromJson(super.json);

  /// 空のオブジェクトから作る
  GeoGeometry() : super({});

  /// LOD。2D の地物は書かない
  core.int? get lod => readField('lod', asInt);
  set lod(core.int? value) => writeField('lod', value);

  /// 形の種類
  String? get form => readField('form', asString);
  set form(String? value) => writeField('form', value);

  /// 点の数。立体と面の集まりは、面の輪の点の数の和
  core.int? get points => readField('points', asInt);
  set points(core.int? value) => writeField('points', value);

  /// 立体と面の集まりの面の数
  core.int? get faces => readField('faces', asInt);
  set faces(core.int? value) => writeField('faces', value);
}

/// open で開いた J-LandXML・IFC4.3・SIMA の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。マイナーバージョン 51 から
class CivilDrawingRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilDrawingRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  CivilDrawingRequest() : super({'op': 'civil_drawing'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// open で開いた J-LandXML・IFC4.3・SIMA の文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// doc の線形の ID。文書に線形が 1 つなら書かなくてよい
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 書く用紙の名前（結果の sheets の 1 つ）。書かなければ平面図
  String? get sheet => readField('sheet', asString);
  set sheet(String? value) => writeField('sheet', value);

  /// 作る文書の名前
  String? get to => readField('to', asString);
  set to(String? value) => writeField('to', value);

  /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
  List<String>? get profiles => readField('profiles', listOf(asString));
  set profiles(List<String>? value) => writeField('profiles', value);

  /// 作図の値に使うプロファイル。profiles の中から選ぶ。profiles と一緒に書く
  ProfileRef? get profile => readField('profile', objectOf(ProfileRef.fromJson));
  set profile(ProfileRef? value) => writeField('profile', value);
}

/// 作った文書。書いたファイルは blobs の file で返す
class CivilDrawingResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilDrawingResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilDrawingResult() : super({});

  /// 作った文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 書いた形式の名前。今は dxf
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 書いたファイルのハッシュ。apply の前提条件に使う
  String? get hash => readField('hash', asString);
  set hash(String? value) => writeField('hash', value);

  /// 書いた要素の数
  core.int? get elements => readField('elements', asInt);
  set elements(core.int? value) => writeField('elements', value);

  /// 図を作った線形の ID
  String? get alignment => readField('alignment', asString);
  set alignment(String? value) => writeField('alignment', value);

  /// 作った用紙の名前。図を描ける順（平面図・縦断図・横断図）
  List<String>? get sheets => readField('sheets', listOf(asString));
  set sheets(List<String>? value) => writeField('sheets', value);

  /// 書いた用紙の名前
  String? get sheet => readField('sheet', asString);
  set sheet(String? value) => writeField('sheet', value);

  /// 使った作図の値
  CivilDrawingOptions? get options => readField('options', objectOf(CivilDrawingOptions.fromJson));
  set options(CivilDrawingOptions? value) => writeField('options', value);

  /// 作図の値を読んだプロファイルの、extends を解いた鎖。基から順。プロファイルを渡さなければ書かない
  List<ProfileIdentity>? get profile => readField('profile', listOf(objectOf(ProfileIdentity.fromJson)));
  set profile(List<ProfileIdentity>? value) => writeField('profile', value);
}

/// 土木の図の作図の値
class CivilDrawingOptions extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilDrawingOptions.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilDrawingOptions() : super({});

  /// 平面図の尺度の分母
  core.double? get plan => readField('plan', asDouble);
  set plan(core.double? value) => writeField('plan', value);

  /// 縦断図の横の尺度の分母
  core.double? get horizontal => readField('horizontal', asDouble);
  set horizontal(core.double? value) => writeField('horizontal', value);

  /// 縦断図の縦の尺度の分母
  core.double? get vertical => readField('vertical', asDouble);
  set vertical(core.double? value) => writeField('vertical', value);

  /// 横断図の尺度の分母
  core.double? get crossSection => readField('cross_section', asDouble);
  set crossSection(core.double? value) => writeField('cross_section', value);

  /// 文字の高さ（mm）
  core.double? get textHeight => readField('text_height', asDouble);
  set textHeight(core.double? value) => writeField('text_height', value);

  /// 測点の間隔（メートル）
  core.double? get stationPitch => readField('station_pitch', asDouble);
  set stationPitch(core.double? value) => writeField('station_pitch', value);
}

/// 開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。点群のモジュールに置く（決定 0253）。マイナーバージョン 50 から
class DeviationRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeviationRequest.fromJson(super.json);

  /// タグのフィールドだけを持つオブジェクトから作る
  DeviationRequest() : super({'op': 'deviation'});

  /// メッセージのスキーマのメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  core.bool? get partial => readField('partial', asBool);
  set partial(core.bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  core.int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(core.int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  core.int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(core.int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// open で開いた点群の文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 設計面の TIN
  DeviationSurface? get surface => readField('surface', objectOf(DeviationSurface.fromJson));
  set surface(DeviationSurface? value) => writeField('surface', value);

  /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
  List<String>? get profiles => readField('profiles', listOf(asString));
  set profiles(List<String>? value) => writeField('profiles', value);

  /// 規格値に使うプロファイル。profiles の中から選ぶ
  ProfileRef? get profile => readField('profile', objectOf(ProfileRef.fromJson));
  set profile(ProfileRef? value) => writeField('profile', value);

  /// 工種の名前。規則の work と比べ、同じものだけを使う
  String? get work => readField('work', asString);
  set work(String? value) => writeField('work', value);

  /// 返す最初の評価した点の位置。前の応答の next_offset を渡す。書かなければ 0
  core.int? get offset => readField('offset', asInt);
  set offset(core.int? value) => writeField('offset', value);

  /// 返す評価した点の数の上限。書かなければ点を返さない
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);

  /// 差の色の段で点を塗った 3D のシーンも返す。点の選び方は点群の scene3d と同じ。89 から
  DeviationScene3d? get scene3d => readField('scene3d', objectOf(DeviationScene3d.fromJson));
  set scene3d(DeviationScene3d? value) => writeField('scene3d', value);
}

/// 設計面の TIN を持つ文書と、TIN の ID
class DeviationSurface extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeviationSurface.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeviationSurface() : super({});

  /// open で開いた土木の文書の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// TIN の ID。civil の surfaces の id
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);
}

/// 標高の差の、規則の区分ごとの集計と指摘。合格・適合とは言わない
class DeviationResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeviationResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeviationResult() : super({});

  /// 使ったプロファイルの鎖。extends の基から順
  List<ProfileIdentity>? get chain => readField('chain', listOf(objectOf(ProfileIdentity.fromJson)));
  set chain(List<ProfileIdentity>? value) => writeField('chain', value);

  /// 点群の点の数
  core.int? get points => readField('points', asInt);
  set points(core.int? value) => writeField('points', value);

  /// TIN の外なので評価しなかった点の数
  core.int? get outside => readField('outside', asInt);
  set outside(core.int? value) => writeField('outside', value);

  /// どの規則の勾配にも入らない三角形の上なので評価しなかった点の数
  core.int? get unmatched => readField('unmatched', asInt);
  set unmatched(core.int? value) => writeField('unmatched', value);

  /// 区分の境の近く（規則の exclude の内）なので除いた点の数
  core.int? get excluded => readField('excluded', asInt);
  set excluded(core.int? value) => writeField('excluded', value);

  /// 規則ごとの集計。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
  List<DeviationRule>? get rules => readField('rules', listOf(objectOf(DeviationRule.fromJson)));
  set rules(List<DeviationRule>? value) => writeField('rules', value);

  /// 指摘。規則の順。規格値を外れれば profile.surface-elevation.invalid、確認できなければ profile.surface-elevation.unsupported
  List<CheckFinding>? get findings => readField('findings', listOf(objectOf(CheckFinding.fromJson)));
  set findings(List<CheckFinding>? value) => writeField('findings', value);

  /// 評価した点の数
  core.int? get evaluated => readField('evaluated', asInt);
  set evaluated(core.int? value) => writeField('evaluated', value);

  /// 評価した点。点群の順に、offset から limit 個まで
  List<DeviationPoint>? get differences => readField('differences', listOf(objectOf(DeviationPoint.fromJson)));
  set differences(List<DeviationPoint>? value) => writeField('differences', value);

  /// 続きの最初の評価した点の位置。続きがなければ持たない
  core.int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(core.int? value) => writeField('next_offset', value);

  /// 要求に scene3d を書いたときの 3D のシーン。点の色は bands の段の色で、元の色は持たない。強度と分類は元の値。89 から
  Scene3dResult? get scene3d => readField('scene3d', objectOf(Scene3dResult.fromJson));
  set scene3d(Scene3dResult? value) => writeField('scene3d', value);

  /// 要求に scene3d を書いたときの色の段。規則の順に、その規則で起こりうる段を、差の小さい順に並べ、最後に評価しなかった点の段を置く。89 から
  List<DeviationBand>? get bands => readField('bands', listOf(objectOf(DeviationBand.fromJson)));
  set bands(List<DeviationBand>? value) => writeField('bands', value);
}

/// 差の色分けの 3D のシーンの点の選び方。意味は点群の scene3d の box・tolerance・limit と同じ
class DeviationScene3d extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeviationScene3d.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeviationScene3d() : super({});

  /// 点群の元の座標系の東・北・高さのメートルの箱
  Scene3dBox? get box => readField('box', objectOf(Scene3dBox.fromJson));
  set box(Scene3dBox? value) => writeField('box', value);

  /// 点の間隔（メートル）の正の有限値。これより細かい段の節を選ばない
  core.double? get tolerance => readField('tolerance', asDouble);
  set tolerance(core.double? value) => writeField('tolerance', value);

  /// 返す点の数の上限。省略時は選んだすべての点
  core.int? get limit => readField('limit', asInt);
  set limit(core.int? value) => writeField('limit', value);
}

/// 差の色の段。差は mm で、規則の each（個々の計測値の規格値）から決める。下限より小さい差は below、上限より大きい差は above で、規格値ちょうどは内側に入れる。内側は 0 から規格値の半分までと、半分から規格値までに分け、境の値は 0 に近い段に入れる。0 は high_inner。規格値のない側は inner の 1 段だけにする
class DeviationBand extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeviationBand.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeviationBand() : super({});

  /// 規則の id。評価しなかった点の段は持たない
  String? get rule => readField('rule', asString);
  set rule(String? value) => writeField('rule', value);

  /// 段の名前
  String? get band => readField('band', asString);
  set band(String? value) => writeField('band', value);

  /// 段の下の端の差（mm）。下に端がなければ持たない
  core.double? get lower => readField('lower', asDouble);
  set lower(core.double? value) => writeField('lower', value);

  /// 段の上の端の差（mm）。上に端がなければ持たない
  core.double? get upper => readField('upper', asDouble);
  set upper(core.double? value) => writeField('upper', value);

  /// シーンの点の色。16 ビットの赤・緑・青・不透明度
  List<core.int>? get color => readField('color', listOf(asInt));
  set color(List<core.int>? value) => writeField('color', value);
}

/// 差の色の段。not_evaluated は TIN の外・どの規則の勾配にも入らない・区分の境の近くで除いた点
///
/// 列挙の値。知らない値も受け取る
abstract final class DeviationBandKind {
  static const below = 'below';
  static const lowOuter = 'low_outer';
  static const lowInner = 'low_inner';
  static const highInner = 'high_inner';
  static const highOuter = 'high_outer';
  static const above = 'above';
  static const notEvaluated = 'not_evaluated';
}

/// 規則の区分ごとの集計。差はメートル
class DeviationRule extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeviationRule.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeviationRule() : super({});

  /// 規則の id
  String? get rule => readField('rule', asString);
  set rule(String? value) => writeField('rule', value);

  /// 区分の名前
  String? get partField => readField('part', asString);
  set partField(String? value) => writeField('part', value);

  /// 評価した点の数
  core.int? get points => readField('points', asInt);
  set points(core.int? value) => writeField('points', value);

  /// 区分の三角形の、平面に投影した面積（m²）
  core.double? get area => readField('area', asDouble);
  set area(core.double? value) => writeField('area', value);

  /// 計測の密度（点／m²）。面積がなければ書かない
  core.double? get density => readField('density', asDouble);
  set density(core.double? value) => writeField('density', value);

  /// 差の平均。点がなければ書かない
  core.double? get mean => readField('mean', asDouble);
  set mean(core.double? value) => writeField('mean', value);

  /// 差の最小。点がなければ書かない
  core.double? get min => readField('min', asDouble);
  set min(core.double? value) => writeField('min', value);

  /// 差の最大。点がなければ書かない
  core.double? get max => readField('max', asDouble);
  set max(core.double? value) => writeField('max', value);

  /// 指摘の数（確認できなかったものを除く）
  core.int? get findings => readField('findings', asInt);
  set findings(core.int? value) => writeField('findings', value);

  /// 確認できなかった数
  core.int? get unsupported => readField('unsupported', asInt);
  set unsupported(core.int? value) => writeField('unsupported', value);
}

/// 評価した点
class DeviationPoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeviationPoint.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeviationPoint() : super({});

  /// 点群の中の番号
  core.int? get index => readField('index', asInt);
  set index(core.int? value) => writeField('index', value);

  /// 北・東・標高（メートル）
  List<core.double>? get position => readField('position', listOf(asDouble));
  set position(List<core.double>? value) => writeField('position', value);

  /// 点の標高から設計面の標高を引いた差（メートル）
  core.double? get difference => readField('difference', asDouble);
  set difference(core.double? value) => writeField('difference', value);

  /// 評価した規則の id
  String? get rule => readField('rule', asString);
  set rule(String? value) => writeField('rule', value);
}

/// describe に使うカタログのファイル。catalog/<言語>.toml から生成し、zm_push で渡す（決定 0009）
class Catalog extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Catalog.fromJson(super.json);

  /// 空のオブジェクトから作る
  Catalog() : super({});

  /// カタログの形のメジャーバージョン。今は 1
  core.int? get v => readField('v', asInt);
  set v(core.int? value) => writeField('v', value);

  /// 言語（ja・en）
  String? get lang => readField('lang', asString);
  set lang(String? value) => writeField('lang', value);

  /// パッケージのバージョン。コアと同じバージョンで出す
  String? get package => readField('package', asString);
  set package(String? value) => writeField('package', value);

  /// 数の書き方
  NumberFormat? get number => readField('number', objectOf(NumberFormat.fromJson));
  set number(NumberFormat? value) => writeField('number', value);

  /// code ごとの文のひな形。{名前} に値が、{count} にまとめた件数が入る
  Map<String, String>? get codes => readField('codes', mapOf(asString));
  set codes(Map<String, String>? value) => writeField('codes', value);
}

/// 文に差し込む数の書き方。OS のロケールは使わない（決定 0009）
class NumberFormat extends Message {
  /// 読んだ JSON のオブジェクトを包む
  NumberFormat.fromJson(super.json);

  /// 空のオブジェクトから作る
  NumberFormat() : super({});

  /// 整数部を 3 桁ごとに区切る文字。空なら区切らない
  String? get groupSeparator => readField('group_separator', asString);
  set groupSeparator(String? value) => writeField('group_separator', value);

  /// 小数点の文字
  String? get decimalSeparator => readField('decimal_separator', asString);
  set decimalSeparator(String? value) => writeField('decimal_separator', value);
}

/// op の名前。要求の型は `<Op>Request`、結果の型は `<Op>Result`
abstract final class Operations {
  static const capabilities = 'capabilities';
  static const open = 'open';
  static const close = 'close';
  static const describe = 'describe';
  static const drawing = 'drawing';
  static const entities = 'entities';
  static const calibrations = 'calibrations';
  static const measureDistance = 'measure_distance';
  static const measure = 'measure';
  static const intersect = 'intersect';
  static const regions = 'regions';
  static const construct = 'construct';
  static const apply = 'apply';
  static const create = 'create';
  static const convert = 'convert';
  static const convertEstimate = 'convert_estimate';
  static const tables = 'tables';
  static const scene = 'scene';
  static const scene3d = 'scene3d';
  static const check = 'check';
  static const validate = 'validate';
  static const reviewCarry = 'review_carry';
  static const reviewAdopt = 'review_adopt';
  static const reviewScene = 'review_scene';
  static const building = 'building';
  static const buildingElements = 'building_elements';
  static const buildingDrawing = 'building_drawing';
  static const idsCheck = 'ids_check';
  static const reviewAdoptIds = 'review_adopt_ids';
  static const bcfRead = 'bcf_read';
  static const bcfWrite = 'bcf_write';
  static const civil = 'civil';
  static const pointcloud = 'pointcloud';
  static const deviation = 'deviation';
  static const civilDrawing = 'civil_drawing';
  static const geo = 'geo';
  static const deliveryCheck = 'delivery_check';
  static const deliveryApply = 'delivery_apply';
  static const photo = 'photo';
  static const boring = 'boring';
  static const buildingApply = 'building_apply';
  static const lift = 'lift';
  static const render = 'render';
  static const raster = 'raster';
  static const georeference = 'georeference';
}
