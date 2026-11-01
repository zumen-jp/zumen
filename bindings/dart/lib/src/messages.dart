// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。
//
// 境界のメッセージの型（決定 0012・0227）。スキーマの定義ごとに 1 つの型がある。型は JSON のオブジェクトを包む
// 見方で、知らない欄もそのまま持つ。列挙は文字列で読み、知らない値も受け取る（決定 0015）。

// ignore_for_file: lines_longer_than_80_chars

import 'message.dart';

/// 要求。op の値で形が決まる
class Request extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Request.fromJson(super.json);

  /// 空のオブジェクトから作る
  Request() : super({});

  /// op が capabilities の形なら、その見方。違えば null
  CapabilitiesRequest? get asCapabilitiesRequest => taggedView('op', 'capabilities', CapabilitiesRequest.fromJson);

  /// op が open の形なら、その見方。違えば null
  OpenRequest? get asOpenRequest => taggedView('op', 'open', OpenRequest.fromJson);

  /// op が close の形なら、その見方。違えば null
  CloseRequest? get asCloseRequest => taggedView('op', 'close', CloseRequest.fromJson);

  /// op が describe の形なら、その見方。違えば null
  DescribeRequest? get asDescribeRequest => taggedView('op', 'describe', DescribeRequest.fromJson);

  /// op が drawing の形なら、その見方。違えば null
  DrawingRequest? get asDrawingRequest => taggedView('op', 'drawing', DrawingRequest.fromJson);

  /// op が entities の形なら、その見方。違えば null
  EntitiesRequest? get asEntitiesRequest => taggedView('op', 'entities', EntitiesRequest.fromJson);

  /// op が apply の形なら、その見方。違えば null
  ApplyRequest? get asApplyRequest => taggedView('op', 'apply', ApplyRequest.fromJson);

  /// op が convert の形なら、その見方。違えば null
  ConvertRequest? get asConvertRequest => taggedView('op', 'convert', ConvertRequest.fromJson);

  /// op が tables の形なら、その見方。違えば null
  TablesRequest? get asTablesRequest => taggedView('op', 'tables', TablesRequest.fromJson);

  /// op が scene の形なら、その見方。違えば null
  SceneRequest? get asSceneRequest => taggedView('op', 'scene', SceneRequest.fromJson);

  /// op が check の形なら、その見方。違えば null
  CheckRequest? get asCheckRequest => taggedView('op', 'check', CheckRequest.fromJson);

  /// op が validate の形なら、その見方。違えば null
  ValidateRequest? get asValidateRequest => taggedView('op', 'validate', ValidateRequest.fromJson);

  /// op が review_carry の形なら、その見方。違えば null
  ReviewCarryRequest? get asReviewCarryRequest => taggedView('op', 'review_carry', ReviewCarryRequest.fromJson);

  /// op が review_adopt の形なら、その見方。違えば null
  ReviewAdoptRequest? get asReviewAdoptRequest => taggedView('op', 'review_adopt', ReviewAdoptRequest.fromJson);

  /// op が building の形なら、その見方。違えば null
  BuildingRequest? get asBuildingRequest => taggedView('op', 'building', BuildingRequest.fromJson);

  /// op が building_elements の形なら、その見方。違えば null
  BuildingElementsRequest? get asBuildingElementsRequest => taggedView('op', 'building_elements', BuildingElementsRequest.fromJson);

  /// op が ids_check の形なら、その見方。違えば null
  IdsCheckRequest? get asIdsCheckRequest => taggedView('op', 'ids_check', IdsCheckRequest.fromJson);

  /// op が review_adopt_ids の形なら、その見方。違えば null
  ReviewAdoptIdsRequest? get asReviewAdoptIdsRequest => taggedView('op', 'review_adopt_ids', ReviewAdoptIdsRequest.fromJson);

  /// op が bcf_read の形なら、その見方。違えば null
  BcfReadRequest? get asBcfReadRequest => taggedView('op', 'bcf_read', BcfReadRequest.fromJson);

  /// op が bcf_write の形なら、その見方。違えば null
  BcfWriteRequest? get asBcfWriteRequest => taggedView('op', 'bcf_write', BcfWriteRequest.fromJson);

  /// op が civil の形なら、その見方。違えば null
  CivilRequest? get asCivilRequest => taggedView('op', 'civil', CivilRequest.fromJson);

  /// op が pointcloud の形なら、その見方。違えば null
  PointcloudRequest? get asPointcloudRequest => taggedView('op', 'pointcloud', PointcloudRequest.fromJson);

  /// op が deviation の形なら、その見方。違えば null
  DeviationRequest? get asDeviationRequest => taggedView('op', 'deviation', DeviationRequest.fromJson);

  /// op が civil_drawing の形なら、その見方。違えば null
  CivilDrawingRequest? get asCivilDrawingRequest => taggedView('op', 'civil_drawing', CivilDrawingRequest.fromJson);

  /// op が geo の形なら、その見方。違えば null
  GeoRequest? get asGeoRequest => taggedView('op', 'geo', GeoRequest.fromJson);

  /// op が delivery_check の形なら、その見方。違えば null
  DeliveryCheckRequest? get asDeliveryCheckRequest => taggedView('op', 'delivery_check', DeliveryCheckRequest.fromJson);

  /// op が photo の形なら、その見方。違えば null
  PhotoRequest? get asPhotoRequest => taggedView('op', 'photo', PhotoRequest.fromJson);

  /// op が sheet の形なら、その見方。違えば null
  SheetRequest? get asSheetRequest => taggedView('op', 'sheet', SheetRequest.fromJson);
}

/// どの op の要求にも置ける欄
class RequestCommon extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RequestCommon.fromJson(super.json);

  /// 空のオブジェクトから作る
  RequestCommon() : super({});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);
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

/// コアの版と、使える op・形式を尋ねる
class CapabilitiesRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CapabilitiesRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  CapabilitiesRequest() : super({'op': 'capabilities'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);
}

/// コアの版と、使える op・形式（決定 0015）
class CapabilitiesResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CapabilitiesResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  CapabilitiesResult() : super({});

  /// パッケージの版（semver）
  String? get package => readField('package', asString);
  set package(String? value) => writeField('package', value);

  /// ABI の大版。zm_abi_version と同じ
  int? get abi => readField('abi', asInt);
  set abi(int? value) => writeField('abi', value);

  /// メッセージのスキーマの版
  SchemaVersion? get schema => readField('schema', objectOf(SchemaVersion.fromJson));
  set schema(SchemaVersion? value) => writeField('schema', value);

  /// このコア（wasm ではこのモジュール）で使える op の名前
  List<String>? get ops => readField('ops', listOf(asString));
  set ops(List<String>? value) => writeField('ops', value);

  /// 読み書きできる形式
  List<Format>? get formats => readField('formats', listOf(objectOf(Format.fromJson)));
  set formats(List<Format>? value) => writeField('formats', value);
}

/// メッセージのスキーマの版。大版は要求と応答の v、小版は op・欄・列挙の値を足すたびに上げる
class SchemaVersion extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SchemaVersion.fromJson(super.json);

  /// 空のオブジェクトから作る
  SchemaVersion() : super({});

  /// 大版
  int? get major => readField('major', asInt);
  set major(int? value) => writeField('major', value);

  /// 小版
  int? get minor => readField('minor', asInt);
  set minor(int? value) => writeField('minor', value);
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
  bool? get read => readField('read', asBool);
  set read(bool? value) => writeField('read', value);

  /// 書けるか
  bool? get write => readField('write', asBool);
  set write(bool? value) => writeField('write', value);

  /// この形式の ID の規則の版（決定 0015）
  int? get idRules => readField('id_rules', asInt);
  set idRules(int? value) => writeField('id_rules', value);
}

/// 入力を読んで、文書を作る。形式は入力から判定する
class OpenRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  OpenRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  OpenRequest() : super({'op': 'open'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 読む入力の名前。zm_push で渡したもの
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);

  /// 作る文書の名前。書かなければ入力の名前
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。小版 48 から
  int? get size => readField('size', asInt);
  set size(int? value) => writeField('size', value);

  /// 見分ける印のない形式（xyz）を名指して読む。書かなければ、入力から判定する（決定 0232）。小版 49 から
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);
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

  /// 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。小版 48 から
  List<InputRange>? get needs => readField('needs', listOf(objectOf(InputRange.fromJson)));
  set needs(List<InputRange>? value) => writeField('needs', value);

  /// 読み終えたので、close の inputs で捨ててよい範囲の入力。小版 48 から
  List<InputRange>? get done => readField('done', listOf(objectOf(InputRange.fromJson)));
  set done(List<InputRange>? value) => writeField('done', value);

  /// SXF を ZIP に入れた書庫（P2Z・SFZ）なら、SXF の項目のほかの項目（属性ファイル・画像など）の書庫の中の名前。書庫の順。apply は、SXF の項目だけを置き換え、ほかの項目を写した書庫を書く（#385）。小版 54 から
  List<String>? get entries => readField('entries', listOf(asString));
  set entries(List<String>? value) => writeField('entries', value);
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
  int? get start => readField('start', asInt);
  set start(int? value) => writeField('start', value);

  /// 範囲の終わり（バイト、含まない）
  int? get end => readField('end', asInt);
  set end(int? value) => writeField('end', value);
}

/// 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
class CloseRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CloseRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  CloseRequest() : super({'op': 'close'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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

  /// 目印の欄だけを持つオブジェクトから作る
  DescribeRequest() : super({'op': 'describe'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合試験で比べるためにも使う（決定 0007）
class DrawingRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DrawingRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  DrawingRequest() : super({'op': 'drawing'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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

  /// 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。小版 39 から
  PointStyleSummary? get pointStyle => readField('point_style', objectOf(PointStyleSummary.fromJson));
  set pointStyle(PointStyleSummary? value) => writeField('point_style', value);
}

/// 文書の点の表し方。欄の意味は point_style_edit と同じ
class PointStyleSummary extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointStyleSummary.fromJson(super.json);

  /// 空のオブジェクトから作る
  PointStyleSummary() : super({});

  /// 基本の形
  String? get mark => readField('mark', asString);
  set mark(String? value) => writeField('mark', value);

  /// 円で囲む
  bool? get circle => readField('circle', asBool);
  set circle(bool? value) => writeField('circle', value);

  /// 四角で囲む
  bool? get square => readField('square', asBool);
  set square(bool? value) => writeField('square', value);

  /// 大きさ。意味は size_kind で決まる
  double? get size => readField('size', asDouble);
  set size(double? value) => writeField('size', value);

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
  List<double>? get size => readField('size', listOf(asDouble));
  set size(List<double>? value) => writeField('size', value);

  /// 要素の数
  ElementCounts? get elements => readField('elements', objectOf(ElementCounts.fromJson));
  set elements(ElementCounts? value) => writeField('elements', value);

  /// ビューの数
  int? get views => readField('views', asInt);
  set views(int? value) => writeField('views', value);

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
  int? get lines => readField('lines', asInt);
  set lines(int? value) => writeField('lines', value);

  /// 文字
  int? get texts => readField('texts', asInt);
  set texts(int? value) => writeField('texts', value);
}

/// 点を囲む、軸に沿った長方形
class Extent extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Extent.fromJson(super.json);

  /// 空のオブジェクトから作る
  Extent() : super({});

  /// X の最小
  double? get minX => readField('min_x', asDouble);
  set minX(double? value) => writeField('min_x', value);

  /// Y の最小
  double? get minY => readField('min_y', asDouble);
  set minY(double? value) => writeField('min_y', value);

  /// X の最大
  double? get maxX => readField('max_x', asDouble);
  set maxX(double? value) => writeField('max_x', value);

  /// Y の最大
  double? get maxY => readField('max_y', asDouble);
  set maxY(double? value) => writeField('max_y', value);
}

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）
class EntitiesRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EntitiesRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  EntitiesRequest() : super({'op': 'entities'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0
  int? get offset => readField('offset', asInt);
  set offset(int? value) => writeField('offset', value);

  /// 返す要素の数の上限。書かなければ 1000
  int? get limit => readField('limit', asInt);
  set limit(int? value) => writeField('limit', value);

  /// 返す欄の名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118）
  List<String>? get fields => readField('fields', listOf(asString));
  set fields(List<String>? value) => writeField('fields', value);

  /// [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118）
  List<double>? get region => readField('region', listOf(asDouble));
  set region(List<double>? value) => writeField('region', value);

  /// 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。小版 31 から（決定 0177）
  String? get place => readField('place', asString);
  set place(String? value) => writeField('place', value);
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
  int? get total => readField('total', asInt);
  set total(int? value) => writeField('total', value);

  /// 続きの最初の要素の位置。続きがなければ持たない
  int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(int? value) => writeField('next_offset', value);
}

/// 図面モデルの要素
class Entity extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Entity.fromJson(super.json);

  /// 空のオブジェクトから作る
  Entity() : super({});

  /// 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つ鍵（ハンドル）があれば鍵で作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どの版でも同じ ID を返す（決定 0117・0156）
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

  /// 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点か当てはめる点で決まるスプライン・寸法・引出線（LEADER）だけが持つ。apply の replace と add に渡せる形（決定 0139）。小版 10 から
  Geometry? get geometry => readField('geometry', objectOf(Geometry.fromJson));
  set geometry(Geometry? value) => writeField('geometry', value);
}

/// entities で返す要素の欄（決定 0118）
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
}

/// 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる
class Geometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Geometry.fromJson(super.json);

  /// 空のオブジェクトから作る
  Geometry() : super({});

  /// shape が line の形なら、その見方。違えば null
  LineGeometry? get asLineGeometry => taggedView('shape', 'line', LineGeometry.fromJson);

  /// shape が circle の形なら、その見方。違えば null
  CircleGeometry? get asCircleGeometry => taggedView('shape', 'circle', CircleGeometry.fromJson);

  /// shape が arc の形なら、その見方。違えば null
  ArcGeometry? get asArcGeometry => taggedView('shape', 'arc', ArcGeometry.fromJson);

  /// shape が ellipse の形なら、その見方。違えば null
  EllipseGeometry? get asEllipseGeometry => taggedView('shape', 'ellipse', EllipseGeometry.fromJson);

  /// shape が polyline の形なら、その見方。違えば null
  PolylineGeometry? get asPolylineGeometry => taggedView('shape', 'polyline', PolylineGeometry.fromJson);

  /// shape が text の形なら、その見方。違えば null
  TextGeometry? get asTextGeometry => taggedView('shape', 'text', TextGeometry.fromJson);

  /// shape が formatted_text の形なら、その見方。違えば null
  FormattedTextGeometry? get asFormattedTextGeometry => taggedView('shape', 'formatted_text', FormattedTextGeometry.fromJson);

  /// shape が symbol の形なら、その見方。違えば null
  SymbolGeometry? get asSymbolGeometry => taggedView('shape', 'symbol', SymbolGeometry.fromJson);

  /// shape が spline の形なら、その見方。違えば null
  SplineGeometry? get asSplineGeometry => taggedView('shape', 'spline', SplineGeometry.fromJson);

  /// shape が dimension の形なら、その見方。違えば null
  DimensionGeometry? get asDimensionGeometry => taggedView('shape', 'dimension', DimensionGeometry.fromJson);

  /// shape が leader の形なら、その見方。違えば null
  LeaderGeometry? get asLeaderGeometry => taggedView('shape', 'leader', LeaderGeometry.fromJson);

  /// shape が point の形なら、その見方。違えば null
  PointGeometry? get asPointGeometry => taggedView('shape', 'point', PointGeometry.fromJson);

  /// shape が callout の形なら、その見方。違えば null
  CalloutGeometry? get asCalloutGeometry => taggedView('shape', 'callout', CalloutGeometry.fromJson);
}

/// 線分
class LineGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LineGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  LineGeometry() : super({'shape': 'line'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 始点 [X, Y]
  List<double>? get start => readField('start', listOf(asDouble));
  set start(List<double>? value) => writeField('start', value);

  /// 終点 [X, Y]
  List<double>? get end => readField('end', listOf(asDouble));
  set end(List<double>? value) => writeField('end', value);
}

/// 円
class CircleGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CircleGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  CircleGeometry() : super({'shape': 'circle'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 中心 [X, Y]
  List<double>? get center => readField('center', listOf(asDouble));
  set center(List<double>? value) => writeField('center', value);

  /// 半径。0 より大きい
  double? get radius => readField('radius', asDouble);
  set radius(double? value) => writeField('radius', value);
}

/// 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる
class ArcGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ArcGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  ArcGeometry() : super({'shape': 'arc'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 中心 [X, Y]
  List<double>? get center => readField('center', listOf(asDouble));
  set center(List<double>? value) => writeField('center', value);

  /// 半径。0 より大きい
  double? get radius => readField('radius', asDouble);
  set radius(double? value) => writeField('radius', value);

  /// 始まりの角度（度）
  double? get startAngle => readField('start_angle', asDouble);
  set startAngle(double? value) => writeField('start_angle', value);

  /// 終わりの角度（度）
  double? get endAngle => readField('end_angle', asDouble);
  set endAngle(double? value) => writeField('end_angle', value);
}

/// 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。小版 11 から
class EllipseGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  EllipseGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  EllipseGeometry() : super({'shape': 'ellipse'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 中心 [X, Y]
  List<double>? get center => readField('center', listOf(asDouble));
  set center(List<double>? value) => writeField('center', value);

  /// 長軸の端の点 [X, Y]。中心と違う点
  List<double>? get major => readField('major', listOf(asDouble));
  set major(List<double>? value) => writeField('major', value);

  /// 短軸の長軸に対する比。0 より大きく 1 以下
  double? get ratio => readField('ratio', asDouble);
  set ratio(double? value) => writeField('ratio', value);

  /// 始まりの角度（度）。楕円を円に引き延ばしたときの角度
  double? get startAngle => readField('start_angle', asDouble);
  set startAngle(double? value) => writeField('start_angle', value);

  /// 終わりの角度（度）。楕円を円に引き延ばしたときの角度
  double? get endAngle => readField('end_angle', asDouble);
  set endAngle(double? value) => writeField('end_angle', value);
}

/// 頂点を結んだ折れ線。幅は持たない
class PolylineGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PolylineGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  PolylineGeometry() : super({'shape': 'polyline'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 頂点の [X, Y] の並び。2 つ以上
  List<List<double>>? get points => readField('points', listOf(listOf(asDouble)));
  set points(List<List<double>>? value) => writeField('points', value);

  /// 頂点ごとの、次の頂点への弧の膨らみ（中心角の 4 分の 1 の正接。正は反時計回り）。points と同じ数。書かなければどれも 0
  List<double>? get bulges => readField('bulges', listOf(asDouble));
  set bulges(List<double>? value) => writeField('bulges', value);

  /// 最後の頂点から最初の頂点へ結ぶ
  bool? get closed => readField('closed', asBool);
  set closed(bool? value) => writeField('closed', value);
}

/// 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせ・幅の倍率・斜体・反転・文字スタイルを元の値のままにする。足すときは、2 点目がなければ左の基準線に合わせ、幅の倍率 1・斜体なし・反転なしにする
class TextGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TextGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  TextGeometry() : super({'shape': 'text'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 内容。1 行
  String? get content => readField('content', asString);
  set content(String? value) => writeField('content', value);

  /// 基点 [X, Y]。2 点の間に合わせる文字では 1 点目
  List<double>? get position => readField('position', listOf(asDouble));
  set position(List<double>? value) => writeField('position', value);

  /// 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。小版 17 から
  List<double>? get end => readField('end', listOf(asDouble));
  set end(List<double>? value) => writeField('end', value);

  /// 2 点の間への合わせ方。2 点目（end）と一緒に書く。小版 17 から
  String? get fit => readField('fit', asString);
  set fit(String? value) => writeField('fit', value);

  /// 高さ。0 より大きい
  double? get height => readField('height', asDouble);
  set height(double? value) => writeField('height', value);

  /// 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない
  double? get rotation => readField('rotation', asDouble);
  set rotation(double? value) => writeField('rotation', value);

  /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。小版 12 から
  String? get style => readField('style', asString);
  set style(String? value) => writeField('style', value);

  /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
  String? get styleId => readField('style_id', asString);
  set styleId(String? value) => writeField('style_id', value);
}

/// 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
///
/// 列挙の値。知らない値も受け取る
abstract final class TextFit {
  static const aligned = 'aligned';
  static const fit = 'fit';
}

/// 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味・文字スタイルを元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。小版 13 から
class FormattedTextGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  FormattedTextGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  FormattedTextGeometry() : super({'shape': 'formatted_text'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 内容。書式の符号（\P の改行など）を含めた、元の形式の内容のまま
  String? get content => readField('content', asString);
  set content(String? value) => writeField('content', value);

  /// 基点 [X, Y]
  List<double>? get position => readField('position', listOf(asDouble));
  set position(List<double>? value) => writeField('position', value);

  /// 高さ。0 より大きい
  double? get height => readField('height', asDouble);
  set height(double? value) => writeField('height', value);

  /// 回転（度）
  double? get rotation => readField('rotation', asDouble);
  set rotation(double? value) => writeField('rotation', value);

  /// 基点に合わせる枠の位置
  String? get attachment => readField('attachment', asString);
  set attachment(String? value) => writeField('attachment', value);

  /// 折り返す枠の幅。0 より大きい。書かなければ折り返さない
  double? get width => readField('width', asDouble);
  set width(double? value) => writeField('width', value);

  /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す
  String? get style => readField('style', asString);
  set style(String? value) => writeField('style', value);

  /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から
  String? get styleId => readField('style_id', asString);
  set styleId(String? value) => writeField('style_id', value);
}

/// スプライン（NURBS）。制御点で決まるもの（小版 15 から）と、当てはめる点で決まるもの（小版 18 から）がある。当てはめる点で決まるものは、節点と制御点を空にし、当てはめる点（fit）と、あれば両端の接線を持つ。元の図形に制御点や節点もあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない
class SplineGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SplineGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  SplineGeometry() : super({'shape': 'spline'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 次数。1 以上
  int? get degree => readField('degree', asInt);
  set degree(int? value) => writeField('degree', value);

  /// 節点の並び。周期的でなければ、制御点の数と次数と 1 の和の数。当てはめる点で決まるものは空
  List<double>? get knots => readField('knots', listOf(asDouble));
  set knots(List<double>? value) => writeField('knots', value);

  /// 制御点の [X, Y] の並び。次数より多い。当てはめる点で決まるものは空
  List<List<double>>? get control => readField('control', listOf(listOf(asDouble)));
  set control(List<List<double>>? value) => writeField('control', value);

  /// 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない
  List<double>? get weights => readField('weights', listOf(asDouble));
  set weights(List<double>? value) => writeField('weights', value);

  /// 当てはめる点の [X, Y] の並び。2 つ以上。書けば、節点と制御点は空で、重みは書かない。小版 18 から
  List<List<double>>? get fit => readField('fit', listOf(listOf(asDouble)));
  set fit(List<List<double>>? value) => writeField('fit', value);

  /// 当てはめる点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
  List<double>? get startTangent => readField('start_tangent', listOf(asDouble));
  set startTangent(List<double>? value) => writeField('start_tangent', value);

  /// 当てはめる点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から
  List<double>? get endTangent => readField('end_tangent', listOf(asDouble));
  set endTangent(List<double>? value) => writeField('end_tangent', value);

  /// 閉じている
  bool? get closed => readField('closed', asBool);
  set closed(bool? value) => writeField('closed', value);

  /// 周期的
  bool? get periodic => readField('periodic', asBool);
  set periodic(bool? value) => writeField('periodic', value);
}

/// 寸法。種類（kind）ごとに、その種類の定義点の欄を持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。entities が返すだけで、置き換えと追加は change.edit.unsupported になる（寸法を描いた部品を作り直す必要があるため）。小版 19 から
class DimensionGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DimensionGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  DimensionGeometry() : super({'shape': 'dimension'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 寸法の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 1 本目の補助線の点か、測る角度の 1 本目の線の点か、円弧の始めの点 [X, Y]。linear・aligned・angular・arc_length が持つ
  List<double>? get first => readField('first', listOf(asDouble));
  set first(List<double>? value) => writeField('first', value);

  /// 2 本目の補助線の点か、測る角度の 2 本目の線の点か、円弧の終わりの点 [X, Y]。linear・aligned・angular・arc_length が持つ
  List<double>? get second => readField('second', listOf(asDouble));
  set second(List<double>? value) => writeField('second', value);

  /// 寸法線が通る点 [X, Y]。linear・aligned が持つ
  List<double>? get line => readField('line', listOf(asDouble));
  set line(List<double>? value) => writeField('line', value);

  /// 測る向き（度）。linear が持つ
  double? get rotation => readField('rotation', asDouble);
  set rotation(double? value) => writeField('rotation', value);

  /// 補助線の傾き（度）。傾けなければ 0。linear・aligned が持つ
  double? get oblique => readField('oblique', asDouble);
  set oblique(double? value) => writeField('oblique', value);

  /// 角度の頂点 [X, Y]。angular が持つ
  List<double>? get vertex => readField('vertex', listOf(asDouble));
  set vertex(List<double>? value) => writeField('vertex', value);

  /// 1 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
  List<List<double>>? get firstLine => readField('first_line', listOf(listOf(asDouble)));
  set firstLine(List<List<double>>? value) => writeField('first_line', value);

  /// 2 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
  List<List<double>>? get secondLine => readField('second_line', listOf(listOf(asDouble)));
  set secondLine(List<List<double>>? value) => writeField('second_line', value);

  /// 寸法の弧が通る点 [X, Y]。angular・angular_lines・arc_length が持つ
  List<double>? get arc => readField('arc', listOf(asDouble));
  set arc(List<double>? value) => writeField('arc', value);

  /// 中心 [X, Y]。arc_length・radius が持つ
  List<double>? get center => readField('center', listOf(asDouble));
  set center(List<double>? value) => writeField('center', value);

  /// 円の上の点 [X, Y]。radius・diameter が持つ
  List<double>? get chord => readField('chord', listOf(asDouble));
  set chord(List<double>? value) => writeField('chord', value);

  /// chord と向かい合う円の上の点 [X, Y]。diameter が持つ
  List<double>? get opposite => readField('opposite', listOf(asDouble));
  set opposite(List<double>? value) => writeField('opposite', value);

  /// 引出線の長さ。radius・diameter が持つ
  double? get leader => readField('leader', asDouble);
  set leader(double? value) => writeField('leader', value);

  /// 原点 [X, Y]。ordinate が持つ
  List<double>? get origin => readField('origin', listOf(asDouble));
  set origin(List<double>? value) => writeField('origin', value);

  /// 測る点 [X, Y]。ordinate が持つ
  List<double>? get feature => readField('feature', listOf(asDouble));
  set feature(List<double>? value) => writeField('feature', value);

  /// 引出線の終点 [X, Y]。ordinate が持つ
  List<double>? get leaderEnd => readField('leader_end', listOf(asDouble));
  set leaderEnd(List<double>? value) => writeField('leader_end', value);

  /// 測る座標の軸。ordinate が持つ
  String? get axis => readField('axis', asString);
  set axis(String? value) => writeField('axis', value);

  /// 寸法値の文字の書き方
  String? get label => readField('label', asString);
  set label(String? value) => writeField('label', value);

  /// 利用者が書いた寸法値の文字の元の内容。label が override のときだけ持つ。測定値に置き換える符号（DXF の <>）と書式の符号もそのまま持つ
  String? get content => readField('content', asString);
  set content(String? value) => writeField('content', value);

  /// 元の形式が保存した測定値。単位と倍率は measurement_key の欄に従う
  double? get measurement => readField('measurement', asDouble);
  set measurement(double? value) => writeField('measurement', value);

  /// measurement の形式と欄を指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ
  String? get measurementKey => readField('measurement_key', asString);
  set measurementKey(String? value) => writeField('measurement_key', value);

  /// 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない
  List<double>? get textPosition => readField('text_position', listOf(asDouble));
  set textPosition(List<double>? value) => writeField('text_position', value);

  /// 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない
  double? get textRotation => readField('text_rotation', asDouble);
  set textRotation(double? value) => writeField('text_rotation', value);

  /// 寸法スタイルの元の名前
  String? get style => readField('style', asString);
  set style(String? value) => writeField('style', value);

  /// 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。小版 23 から
  String? get styleId => readField('style_id', asString);
  set styleId(String? value) => writeField('style_id', value);
}

/// 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（小版 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印の印を必ず持つので、arrow を書かない引出線は書けない（小版 36 から。それまでは R2004 までの置き換えだけ）。小版 31 から
class LeaderGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LeaderGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  LeaderGeometry() : super({'shape': 'leader'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 経路の頂点 [[X, Y], ...]。矢印の側から並べる。2 つ以上
  List<List<double>>? get path => readField('path', listOf(listOf(asDouble)));
  set path(List<List<double>>? value) => writeField('path', value);

  /// 頂点を通る曲線で結ぶか。偽なら折れ線
  bool? get spline => readField('spline', asBool);
  set spline(bool? value) => writeField('spline', value);

  /// 始点に矢印を付けるか。書かなければ寸法スタイルに従う
  bool? get arrow => readField('arrow', asBool);
  set arrow(bool? value) => writeField('arrow', value);

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

/// 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかの欄は元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。小版 36 から
class CalloutGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CalloutGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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
  bool? get landing => readField('landing', asBool);
  set landing(bool? value) => writeField('landing', value);

  /// 端部記号の大きさ
  double? get arrowSize => readField('arrow_size', asDouble);
  set arrowSize(double? value) => writeField('arrow_size', value);

  /// 大きさに掛ける倍率
  double? get scale => readField('scale', asDouble);
  set scale(double? value) => writeField('scale', value);

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
  List<List<List<double>>>? get lines => readField('lines', listOf(listOf(listOf(asDouble))));
  set lines(List<List<List<double>>>? value) => writeField('lines', value);

  /// 線が集まる点 [X, Y]。元の形式が持たなければ書かない
  List<double>? get end => readField('end', listOf(asDouble));
  set end(List<double>? value) => writeField('end', value);

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
  List<double>? get direction => readField('direction', listOf(asDouble));
  set direction(List<double>? value) => writeField('direction', value);

  /// 長さ
  double? get length => readField('length', asDouble);
  set length(double? value) => writeField('length', value);
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

/// 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義の印の点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。小版 35 から
class PointGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  PointGeometry() : super({'shape': 'point'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 位置 [X, Y]
  List<double>? get position => readField('position', listOf(asDouble));
  set position(List<double>? value) => writeField('position', value);

  /// 回転（度）
  double? get rotation => readField('rotation', asDouble);
  set rotation(double? value) => writeField('rotation', value);

  /// 既定義の印。書かなければ文書の点の表し方に従う
  PointMarker? get marker => readField('marker', objectOf(PointMarker.fromJson));
  set marker(PointMarker? value) => writeField('marker', value);

  /// 作図の補助の点（JWW の仮点）か。書かなければ偽
  bool? get construction => readField('construction', asBool);
  set construction(bool? value) => writeField('construction', value);
}

/// 既定義の印の表のキーと尺度。印の形は、表示する側がキーから引く
class PointMarker extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointMarker.fromJson(super.json);

  /// 空のオブジェクトから作る
  PointMarker() : super({});

  /// 印の表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点）
  String? get namespace => readField('namespace', asString);
  set namespace(String? value) => writeField('namespace', value);

  /// 名前空間の中の元のコード
  String? get code => readField('code', asString);
  set code(String? value) => writeField('code', value);

  /// 尺度
  double? get scale => readField('scale', asDouble);
  set scale(double? value) => writeField('scale', value);
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

/// 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。小版 14 から
class SymbolGeometry extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SymbolGeometry.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  SymbolGeometry() : super({'shape': 'symbol'});

  /// 形の種類
  String? get shape => readField('shape', asString);
  set shape(String? value) => writeField('shape', value);

  /// 記号の定義（ブロック）の元の名前。足すときは図面にある定義の名前を指す
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 定義の基点を置く点 [X, Y]
  List<double>? get position => readField('position', listOf(asDouble));
  set position(List<double>? value) => writeField('position', value);

  /// X と Y の尺度 [X, Y]。0 でない。負なら反転する
  List<double>? get scale => readField('scale', listOf(asDouble));
  set scale(List<double>? value) => writeField('scale', value);

  /// 回転（度）
  double? get rotation => readField('rotation', asDouble);
  set rotation(double? value) => writeField('rotation', value);

  /// 格子に並べて置く列と行。書かなければ 1 つだけ置く
  SymbolGrid? get grid => readField('grid', objectOf(SymbolGrid.fromJson));
  set grid(SymbolGrid? value) => writeField('grid', value);

  /// 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。小版 16 から。置き換えで値を変えるのは小版 21 から
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
  int? get columns => readField('columns', asInt);
  set columns(int? value) => writeField('columns', value);

  /// 行の数。1 以上。列と行がどちらも 1 の格子は持たない
  int? get rows => readField('rows', asInt);
  set rows(int? value) => writeField('rows', value);

  /// 列の間隔
  double? get columnSpacing => readField('column_spacing', asDouble);
  set columnSpacing(double? value) => writeField('column_spacing', value);

  /// 行の間隔
  double? get rowSpacing => readField('row_spacing', asDouble);
  set rowSpacing(double? value) => writeField('row_spacing', value);
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

/// 要素の形の種類（決定 0022）。point（点、決定 0160）は小版 32 から。それまで点は symbol だった
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

/// 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。小版 7 から
class ApplyRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ApplyRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  ApplyRequest() : super({'op': 'apply'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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

  /// true なら、変えていない要素の ID がずれた。鍵を持たない形式（JWW）で、前の図形を除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。小版 30 から
  bool? get idsShifted => readField('ids_shifted', asBool);
  set idsShifted(bool? value) => writeField('ids_shifted', value);
}

/// 開いた文書の図面モデルを、別の形式の新しいファイルに書き、新しい文書として開く（決定 0139・0206）。書けない要素は落とし、種類ごとの件数を診断で返す。書いたファイルは blobs で返す。今は DXF にだけ書ける。小版 41 から
class ConvertRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ConvertRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  ConvertRequest() : super({'op': 'convert'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 書く元の文書の名前。open で作ったもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// 書く形式の名前（capabilities の formats の id）。今は dxf だけ
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// 作る文書の名前
  String? get to => readField('to', asString);
  set to(String? value) => writeField('to', value);
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
  int? get elements => readField('elements', asInt);
  set elements(int? value) => writeField('elements', value);
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

  /// kind が restyle の形なら、その見方。違えば null
  RestyleEdit? get asRestyleEdit => taggedView('kind', 'restyle', RestyleEdit.fromJson);

  /// kind が transform の形なら、その見方。違えば null
  TransformEdit? get asTransformEdit => taggedView('kind', 'transform', TransformEdit.fromJson);

  /// kind が remove の形なら、その見方。違えば null
  RemoveEdit? get asRemoveEdit => taggedView('kind', 'remove', RemoveEdit.fromJson);

  /// kind が replace の形なら、その見方。違えば null
  ReplaceEdit? get asReplaceEdit => taggedView('kind', 'replace', ReplaceEdit.fromJson);

  /// kind が add の形なら、その見方。違えば null
  AddEdit? get asAddEdit => taggedView('kind', 'add', AddEdit.fromJson);

  /// kind が layer の形なら、その見方。違えば null
  LayerEdit? get asLayerEdit => taggedView('kind', 'layer', LayerEdit.fromJson);

  /// kind が add_layer の形なら、その見方。違えば null
  AddLayerEdit? get asAddLayerEdit => taggedView('kind', 'add_layer', AddLayerEdit.fromJson);

  /// kind が remove_layer の形なら、その見方。違えば null
  RemoveLayerEdit? get asRemoveLayerEdit => taggedView('kind', 'remove_layer', RemoveLayerEdit.fromJson);

  /// kind が text_style の形なら、その見方。違えば null
  TextStyleEdit? get asTextStyleEdit => taggedView('kind', 'text_style', TextStyleEdit.fromJson);

  /// kind が add_text_style の形なら、その見方。違えば null
  AddTextStyleEdit? get asAddTextStyleEdit => taggedView('kind', 'add_text_style', AddTextStyleEdit.fromJson);

  /// kind が remove_text_style の形なら、その見方。違えば null
  RemoveTextStyleEdit? get asRemoveTextStyleEdit => taggedView('kind', 'remove_text_style', RemoveTextStyleEdit.fromJson);

  /// kind が line_type の形なら、その見方。違えば null
  LineTypeEdit? get asLineTypeEdit => taggedView('kind', 'line_type', LineTypeEdit.fromJson);

  /// kind が add_line_type の形なら、その見方。違えば null
  AddLineTypeEdit? get asAddLineTypeEdit => taggedView('kind', 'add_line_type', AddLineTypeEdit.fromJson);

  /// kind が remove_line_type の形なら、その見方。違えば null
  RemoveLineTypeEdit? get asRemoveLineTypeEdit => taggedView('kind', 'remove_line_type', RemoveLineTypeEdit.fromJson);

  /// kind が dimension_style の形なら、その見方。違えば null
  DimensionStyleEdit? get asDimensionStyleEdit => taggedView('kind', 'dimension_style', DimensionStyleEdit.fromJson);

  /// kind が add_dimension_style の形なら、その見方。違えば null
  AddDimensionStyleEdit? get asAddDimensionStyleEdit => taggedView('kind', 'add_dimension_style', AddDimensionStyleEdit.fromJson);

  /// kind が remove_dimension_style の形なら、その見方。違えば null
  RemoveDimensionStyleEdit? get asRemoveDimensionStyleEdit => taggedView('kind', 'remove_dimension_style', RemoveDimensionStyleEdit.fromJson);

  /// kind が point_style の形なら、その見方。違えば null
  PointStyleEdit? get asPointStyleEdit => taggedView('kind', 'point_style', PointStyleEdit.fromJson);
}

/// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない
class RestyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RestyleEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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

  /// レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
  String? get layerId => readField('layer_id', asString);
  set layerId(String? value) => writeField('layer_id', value);

  /// 色
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 線種
  LineTypeChange? get lineType => readField('line_type', objectOf(LineTypeChange.fromJson));
  set lineType(LineTypeChange? value) => writeField('line_type', value);

  /// 要素の線種の倍率
  double? get lineTypeScale => readField('line_type_scale', asDouble);
  set lineTypeScale(double? value) => writeField('line_type_scale', value);

  /// 線幅
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);
}

/// 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
class TransformEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TransformEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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
  List<double>? get matrix => readField('matrix', listOf(asDouble));
  set matrix(List<double>? value) => writeField('matrix', value);
}

/// 要素を除く
class RemoveEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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

/// 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤとスタイルは変えない。小版 10 から
class ReplaceEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReplaceEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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

/// 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。足せる形は、形の値のどの種類も。小版 10 から
class AddEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  AddEdit() : super({'kind': 'add'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。小版 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177）
  String? get place => readField('place', asString);
  set place(String? value) => writeField('place', value);

  /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
  String? get layer => readField('layer', asString);
  set layer(String? value) => writeField('layer', value);

  /// レイヤの ID。tables が返したもの
  String? get layerId => readField('layer_id', asString);
  set layerId(String? value) => writeField('layer_id', value);

  /// 色。SXF はレイヤに従う色を書けないので、書く。小版 29 から
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 線種。小版 29 から
  LineTypeChange? get lineType => readField('line_type', objectOf(LineTypeChange.fromJson));
  set lineType(LineTypeChange? value) => writeField('line_type', value);

  /// 線幅。小版 29 から
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);

  /// 足す形
  Geometry? get geometry => readField('geometry', objectOf(Geometry.fromJson));
  set geometry(Geometry? value) => writeField('geometry', value);
}

/// レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG は、今あるレイヤの線種と真の色を変えられない。小版 22 から
class LayerEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LayerEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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
  bool? get visible => readField('visible', asBool);
  set visible(bool? value) => writeField('visible', value);

  /// 印刷する
  bool? get printable => readField('printable', asBool);
  set printable(bool? value) => writeField('printable', value);

  /// 編集できる（ロックしない）
  bool? get editable => readField('editable', asBool);
  set editable(bool? value) => writeField('editable', value);

  /// 凍結する
  bool? get frozen => readField('frozen', asBool);
  set frozen(bool? value) => writeField('frozen', value);

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

/// レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。小版 24 から
class RemoveLayerEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveLayerEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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

/// 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。小版 25 から
class TextStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TextStyleEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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
  double? get height => readField('height', asDouble);
  set height(double? value) => writeField('height', value);

  /// 幅の倍率。正の数
  double? get width => readField('width', asDouble);
  set width(double? value) => writeField('width', value);

  /// 傾き（度）。-85 から 85 まで
  double? get oblique => readField('oblique', asDouble);
  set oblique(double? value) => writeField('oblique', value);

  /// 左右を裏返す
  bool? get backward => readField('backward', asBool);
  set backward(bool? value) => writeField('backward', value);

  /// 上下を反転する
  bool? get upsideDown => readField('upside_down', asBool);
  set upsideDown(bool? value) => writeField('upside_down', value);

  /// 縦書き
  bool? get vertical => readField('vertical', asBool);
  set vertical(bool? value) => writeField('vertical', value);
}

/// 文字スタイルを足す（決定 0163）。書かない欄は Standard と同じ。同じ apply の要素は、足した文字スタイルを指せない。小版 25 から
class AddTextStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddTextStyleEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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
  double? get height => readField('height', asDouble);
  set height(double? value) => writeField('height', value);

  /// 幅の倍率。正の数
  double? get width => readField('width', asDouble);
  set width(double? value) => writeField('width', value);

  /// 傾き（度）。-85 から 85 まで
  double? get oblique => readField('oblique', asDouble);
  set oblique(double? value) => writeField('oblique', value);

  /// 左右を裏返す
  bool? get backward => readField('backward', asBool);
  set backward(bool? value) => writeField('backward', value);

  /// 上下を反転する
  bool? get upsideDown => readField('upside_down', asBool);
  set upsideDown(bool? value) => writeField('upside_down', value);

  /// 縦書き
  bool? get vertical => readField('vertical', asBool);
  set vertical(bool? value) => writeField('vertical', value);
}

/// 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。小版 25 から
class RemoveTextStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveTextStyleEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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

/// 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。多重線のスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。小版 26 から
class LineTypeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LineTypeEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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
  List<double>? get pattern => readField('pattern', listOf(asDouble));
  set pattern(List<double>? value) => writeField('pattern', value);
}

/// 線種を足す（決定 0164）。書かない欄は Continuous と同じ。同じ apply の要素とレイヤは、足した線種を指せない。小版 26 から
class AddLineTypeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddLineTypeEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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
  List<double>? get pattern => readField('pattern', listOf(asDouble));
  set pattern(List<double>? value) => writeField('pattern', value);
}

/// 線種を除く（決定 0164）。要素・レイヤ・多重線のスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。小版 26 から
class RemoveLineTypeEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveLineTypeEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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

/// 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。小版 27 から
class DimensionStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DimensionStyleEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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
  double? get size => readField('size', asDouble);
  set size(double? value) => writeField('size', value);

  /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
  double? get tick => readField('tick', asDouble);
  set tick(double? value) => writeField('tick', value);

  /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
  double? get scale => readField('scale', asDouble);
  set scale(double? value) => writeField('scale', value);
}

/// 寸法スタイルを足す（決定 0165）。書かない欄は Standard と同じ。同じ apply の要素は、足した寸法スタイルを指せない。小版 27 から
class AddDimensionStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddDimensionStyleEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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
  double? get size => readField('size', asDouble);
  set size(double? value) => writeField('size', value);

  /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
  double? get tick => readField('tick', asDouble);
  set tick(double? value) => writeField('tick', value);

  /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
  double? get scale => readField('scale', asDouble);
  set scale(double? value) => writeField('scale', value);
}

/// 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。小版 27 から
class RemoveDimensionStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  RemoveDimensionStyleEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
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

/// 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG は見出しの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、小版 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。小版 38 から
class PointStyleEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointStyleEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  PointStyleEdit() : super({'kind': 'point_style'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 基本の形
  String? get mark => readField('mark', asString);
  set mark(String? value) => writeField('mark', value);

  /// 円で囲む。書かなければ偽
  bool? get circle => readField('circle', asBool);
  set circle(bool? value) => writeField('circle', value);

  /// 四角で囲む。書かなければ偽
  bool? get square => readField('square', asBool);
  set square(bool? value) => writeField('square', value);

  /// 大きさ。0 より大きい有限の数。意味は size_kind で決まる
  double? get size => readField('size', asDouble);
  set size(double? value) => writeField('size', value);

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

/// レイヤを足す（決定 0162）。書かない欄は、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の要素は、足したレイヤを指せない。小版 22 から
class AddLayerEdit extends Message {
  /// 読んだ JSON のオブジェクトを包む
  AddLayerEdit.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  AddLayerEdit() : super({'kind': 'add_layer'});

  /// 変更の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 名前。layer の name と同じ決まり
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 表示する。凍結するなら偽
  bool? get visible => readField('visible', asBool);
  set visible(bool? value) => writeField('visible', value);

  /// 印刷する
  bool? get printable => readField('printable', asBool);
  set printable(bool? value) => writeField('printable', value);

  /// 編集できる（ロックしない）
  bool? get editable => readField('editable', asBool);
  set editable(bool? value) => writeField('editable', value);

  /// 凍結する
  bool? get frozen => readField('frozen', asBool);
  set frozen(bool? value) => writeField('frozen', value);

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
  int? get aci => readField('aci', asInt);
  set aci(int? value) => writeField('aci', value);

  /// SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。小版 29 から
  int? get sxf => readField('sxf', asInt);
  set sxf(int? value) => writeField('sxf', value);

  /// Jw_cad のペンの番号。0〜9 と、版 420 からの SXF の拡張色 100〜356。小版 30 から
  int? get jww => readField('jww', asInt);
  set jww(int? value) => writeField('jww', value);

  /// [赤, 緑, 青]。それぞれ 0〜255
  List<int>? get rgb => readField('rgb', listOf(asInt));
  set rgb(List<int>? value) => writeField('rgb', value);
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

  /// 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から
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
  double? get mm => readField('mm', asDouble);
  set mm(double? value) => writeField('mm', value);

  /// true なら、文書の既定の線幅
  bool? get defaultField => readField('default', asBool);
  set defaultField(bool? value) => writeField('default', value);
}

/// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から
class TablesRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  TablesRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  TablesRequest() : super({'op': 'tables'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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

  /// 線種。小版 26 から、定義を持つ
  List<LineTypeItem>? get lineTypes => readField('line_types', listOf(objectOf(LineTypeItem.fromJson)));
  set lineTypes(List<LineTypeItem>? value) => writeField('line_types', value);

  /// 寸法スタイル。小版 27 から、端部記号と大きさを持つ
  List<DimensionStyleItem>? get dimensionStyles => readField('dimension_styles', listOf(objectOf(DimensionStyleItem.fromJson)));
  set dimensionStyles(List<DimensionStyleItem>? value) => writeField('dimension_styles', value);

  /// 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義の欄を持たない項目として後ろに並ぶ。小版 20 から
  List<TextStyleItem>? get textStyles => readField('text_styles', listOf(objectOf(TextStyleItem.fromJson)));
  set textStyles(List<TextStyleItem>? value) => writeField('text_styles', value);
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。小版 33 から
class SceneRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SceneRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  SceneRequest() : super({'op': 'scene'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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
  int? get view => readField('view', asInt);
  set view(int? value) => writeField('view', value);

  /// [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
  List<double>? get region => readField('region', listOf(asDouble));
  set region(List<double>? value) => writeField('region', value);

  /// 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000
  double? get tolerance => readField('tolerance', asDouble);
  set tolerance(double? value) => writeField('tolerance', value);

  /// true なら印刷する要素を、書かないか false なら表示する要素を描く
  bool? get print => readField('print', asBool);
  set print(bool? value) => writeField('print', value);

  /// 色の表の入力の名前（zm_push で渡したもの）。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決の印を立てる
  String? get palette => readField('palette', asString);
  set palette(String? value) => writeField('palette', value);

  /// 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形の区画に形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。小版 37 から
  List<SceneFont>? get fonts => readField('fonts', listOf(objectOf(SceneFont.fromJson)));
  set fonts(List<SceneFont>? value) => writeField('fonts', value);

  /// 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは大文字の高さとみなす。fonts を書いたときだけ使う。小版 37 から
  Map<String, String>? get heights => readField('heights', mapOf(asString));
  set heights(Map<String, String>? value) => writeField('heights', value);
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
  int? get index => readField('index', asInt);
  set index(int? value) => writeField('index', value);
}

/// 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ
///
/// 列挙の値。知らない値も受け取る
abstract final class HeightReference {
  static const capHeight = 'cap_height';
  static const em = 'em';
  static const frame = 'frame';
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
  int? get view => readField('view', asInt);
  set view(int? value) => writeField('view', value);

  /// 起点の座標の単位。用紙とビューからは millimeter
  String? get unit => readField('unit', asString);
  set unit(String? value) => writeField('unit', value);

  /// 使った弦の誤差（起点の長さ）
  double? get tolerance => readField('tolerance', asDouble);
  set tolerance(double? value) => writeField('tolerance', value);

  /// 返した描くものの数
  SceneCounts? get counts => readField('counts', objectOf(SceneCounts.fromJson));
  set counts(SceneCounts? value) => writeField('counts', value);

  /// 返した塊の範囲。起点の座標。塊がなければ持たない
  Extent? get extent => readField('extent', objectOf(Extent.fromJson));
  set extent(Extent? value) => writeField('extent', value);

  /// シーンの blob の番号（種類は scene）。zm_take で取り出す
  int? get blob => readField('blob', asInt);
  set blob(int? value) => writeField('blob', value);
}

/// シーンの描くものの種類ごとの数
class SceneCounts extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SceneCounts.fromJson(super.json);

  /// 空のオブジェクトから作る
  SceneCounts() : super({});

  /// 塊
  int? get chunks => readField('chunks', asInt);
  set chunks(int? value) => writeField('chunks', value);

  /// 線（折れ線）
  int? get lines => readField('lines', asInt);
  set lines(int? value) => writeField('lines', value);

  /// 線と面の頂点
  int? get vertices => readField('vertices', asInt);
  set vertices(int? value) => writeField('vertices', value);

  /// 面
  int? get faces => readField('faces', asInt);
  set faces(int? value) => writeField('faces', value);

  /// 文字
  int? get texts => readField('texts', asInt);
  set texts(int? value) => writeField('texts', value);

  /// 点
  int? get points => readField('points', asInt);
  set points(int? value) => writeField('points', value);

  /// 画像
  int? get images => readField('images', asInt);
  set images(int? value) => writeField('images', value);

  /// 置いた字形。fonts を書かなければ 0。小版 37 から
  int? get glyphs => readField('glyphs', asInt);
  set glyphs(int? value) => writeField('glyphs', value);
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。小版 34 から
class CheckRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CheckRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  CheckRequest() : super({'op': 'check'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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
  int? get view => readField('view', asInt);
  set view(int? value) => writeField('view', value);

  /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
  String? get fileName => readField('file_name', asString);
  set fileName(String? value) => writeField('file_name', value);

  /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
  int? get offset => readField('offset', asInt);
  set offset(int? value) => writeField('offset', value);

  /// 返す指摘の数の上限。書かなければ 1000
  int? get limit => readField('limit', asInt);
  set limit(int? value) => writeField('limit', value);

  /// 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
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
  int? get total => readField('total', asInt);
  set total(int? value) => writeField('total', value);

  /// 続きの最初の指摘の位置。続きがなければ持たない
  int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(int? value) => writeField('next_offset', value);
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
  int? get revision => readField('revision', asInt);
  set revision(int? value) => writeField('revision', value);
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
  int? get findings => readField('findings', asInt);
  set findings(int? value) => writeField('findings', value);

  /// 照合できなかった数
  int? get unsupported => readField('unsupported', asInt);
  set unsupported(int? value) => writeField('unsupported', value);
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

/// zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。小版 42 から
class ValidateRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ValidateRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  ValidateRequest() : super({'op': 'validate'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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
  int? get offset => readField('offset', asInt);
  set offset(int? value) => writeField('offset', value);

  /// 返す指摘の数の上限。書かなければ 1000
  int? get limit => readField('limit', asInt);
  set limit(int? value) => writeField('limit', value);
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
  int? get rulesNotEvaluated => readField('rules_not_evaluated', asInt);
  set rulesNotEvaluated(int? value) => writeField('rules_not_evaluated', value);

  /// 指摘。インスタンスの番号の順に、属性の順。診断と同じ形で、describe で文にできる。count は 1
  List<Diagnostic>? get findings => readField('findings', listOf(objectOf(Diagnostic.fromJson)));
  set findings(List<Diagnostic>? value) => writeField('findings', value);

  /// 全体の指摘の数
  int? get total => readField('total', asInt);
  set total(int? value) => writeField('total', value);

  /// 続きの最初の指摘の位置。続きがなければ持たない
  int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(int? value) => writeField('next_offset', value);
}

/// レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書の版へ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元の版のハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。小版 43 から
class ReviewCarryRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewCarryRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  ReviewCarryRequest() : super({'op': 'review_carry'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 新しい版の文書の名前。open で作ったもの。小版 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// レビューの文書の入力の名前（zm_push で渡した JSON）
  String? get review => readField('review', asString);
  set review(String? value) => writeField('review', value);

  /// 移す対象の ID。レビューの文書の targets の id
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0
  int? get offset => readField('offset', asInt);
  set offset(int? value) => writeField('offset', value);

  /// 返す参照の数の上限。書かなければ 1000
  int? get limit => readField('limit', asInt);
  set limit(int? value) => writeField('limit', value);
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
  int? get total => readField('total', asInt);
  set total(int? value) => writeField('total', value);

  /// 続きの最初の参照の位置。続きがなければ持たない
  int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(int? value) => writeField('next_offset', value);
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

/// 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しい版に要素がない
///
/// 列挙の値。知らない値も受け取る
abstract final class CarriedState {
  static const unchanged = 'unchanged';
  static const changed = 'changed';
  static const removed = 'removed';
}

/// 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。小版 43 から
class ReviewAdoptRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewAdoptRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  ReviewAdoptRequest() : super({'op': 'review_adopt'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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
  int? get view => readField('view', asInt);
  set view(int? value) => writeField('view', value);

  /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から
  String? get fileName => readField('file_name', asString);
  set fileName(String? value) => writeField('file_name', value);

  /// 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
  String? get palette => readField('palette', asString);
  set palette(String? value) => writeField('palette', value);

  /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
  String? get review => readField('review', asString);
  set review(String? value) => writeField('review', value);

  /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 取り込む指摘の番号の列。check の指摘の並びの順で、0 から数える
  List<int>? get findings => readField('findings', listOf(asInt));
  set findings(List<int>? value) => writeField('findings', value);
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
  int? get topics => readField('topics', asInt);
  set topics(int? value) => writeField('topics', value);
}

/// 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。小版 44 から
class BuildingRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  BuildingRequest() : super({'op': 'building'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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

  /// 入力のバイト列のハッシュ。review_carry の対象の版に使う
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
  int? get elements => readField('elements', asInt);
  set elements(int? value) => writeField('elements', value);

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
  double? get scale => readField('scale', asDouble);
  set scale(double? value) => writeField('scale', value);
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
  int? get count => readField('count', asInt);
  set count(int? value) => writeField('count', value);
}

/// 開いた IFC の建物モデルの要素を、書いた順に返す。成分は components で選ぶ（決定 0207・0219）。小版 44 から
class BuildingElementsRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingElementsRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  BuildingElementsRequest() : super({'op': 'building_elements'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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

  /// 返す成分の名前。1 つ以上で、重ねない。書かなければ class と identity を返す
  List<String>? get components => readField('components', listOf(asString));
  set components(List<String>? value) => writeField('components', value);

  /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
  int? get offset => readField('offset', asInt);
  set offset(int? value) => writeField('offset', value);

  /// 返す数の上限。書かなければ 1000
  int? get limit => readField('limit', asInt);
  set limit(int? value) => writeField('limit', value);
}

/// building_elements で返す成分（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員
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
  int? get total => readField('total', asInt);
  set total(int? value) => writeField('total', value);

  /// 続きの最初の位置。続きがなければ持たない
  int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(int? value) => writeField('next_offset', value);
}

/// 建物モデルの要素。成分は、選んだもののうち要素が持つものだけを持つ
class BuildingElement extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BuildingElement.fromJson(super.json);

  /// 空のオブジェクトから作る
  BuildingElement() : super({});

  /// 要素の ID（GlobalId か、i と番号）
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 要素の成分のハッシュ（v1: と 32 桁の 16 進、決定 0207）
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

  /// 体系の版
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
  double? get value => readField('value', asDouble);
  set value(double? value) => writeField('value', value);

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
  int? get order => readField('order', asInt);
  set order(int? value) => writeField('order', value);
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
  List<List<double>>? get world => readField('world', listOf(listOf(asDouble)));
  set world(List<List<double>>? value) => writeField('world', value);

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
  double? get number => readField('number', asDouble);
  set number(double? value) => writeField('number', value);

  /// text と enumeration の値と、2^53 − 1 を超える integer の値
  String? get text => readField('text', asString);
  set text(String? value) => writeField('text', value);

  /// boolean と logical の値。logical の不明は持たない
  bool? get boolean => readField('boolean', asBool);
  set boolean(bool? value) => writeField('boolean', value);

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
  List<double>? get surfaceColor => readField('surface_color', listOf(asDouble));
  set surfaceColor(List<double>? value) => writeField('surface_color', value);

  /// 透明度（0 から 1）
  double? get transparency => readField('transparency', asDouble);
  set transparency(double? value) => writeField('transparency', value);

  /// 線の色の RGB（0 から 1）
  List<double>? get curveColor => readField('curve_color', listOf(asDouble));
  set curveColor(List<double>? value) => writeField('curve_color', value);

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

/// 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。小版 44 から
class IdsCheckRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  IdsCheckRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  IdsCheckRequest() : super({'op': 'ids_check'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で IFC を開いたもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// IDS の入力の名前（zm_push で渡した .ids）
  String? get ids => readField('ids', asString);
  set ids(String? value) => writeField('ids', value);

  /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす
  String? get express => readField('express', asString);
  set express(String? value) => writeField('express', value);

  /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
  int? get offset => readField('offset', asInt);
  set offset(int? value) => writeField('offset', value);

  /// 返す数の上限。書かなければ 1000
  int? get limit => readField('limit', asInt);
  set limit(int? value) => writeField('limit', value);
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
  int? get total => readField('total', asInt);
  set total(int? value) => writeField('total', value);

  /// 続きの最初の位置。続きがなければ持たない
  int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(int? value) => writeField('next_offset', value);
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
  int? get applicable => readField('applicable', asInt);
  set applicable(int? value) => writeField('applicable', value);

  /// 要件を満たさない要素の数
  int? get failed => readField('failed', asInt);
  set failed(int? value) => writeField('failed', value);

  /// 確かめられなかった要素の数
  int? get unsupported => readField('unsupported', asInt);
  set unsupported(int? value) => writeField('unsupported', value);
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

/// 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。小版 44 から
class ReviewAdoptIdsRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  ReviewAdoptIdsRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  ReviewAdoptIdsRequest() : super({'op': 'review_adopt_ids'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 文書の名前。open で IFC を開いたもの
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// IDS の入力の名前（zm_push で渡した .ids）
  String? get ids => readField('ids', asString);
  set ids(String? value) => writeField('ids', value);

  /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす
  String? get express => readField('express', asString);
  set express(String? value) => writeField('express', value);

  /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
  String? get review => readField('review', asString);
  set review(String? value) => writeField('review', value);

  /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
  String? get target => readField('target', asString);
  set target(String? value) => writeField('target', value);

  /// 取り込む指摘の番号の列。ids_check の指摘の並びの順で、0 から数える
  List<int>? get findings => readField('findings', listOf(asInt));
  set findings(List<int>? value) => writeField('findings', value);
}

/// zm_push で渡した BCF 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。小版 45 から
class BcfReadRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BcfReadRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  BcfReadRequest() : super({'op': 'bcf_read'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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

  /// BCF の版
  String? get version => readField('version', asString);
  set version(String? value) => writeField('version', value);

  /// 指摘の数
  int? get topics => readField('topics', asInt);
  set topics(int? value) => writeField('topics', value);
}

/// zm_push で渡したレビューの文書を、BCF 3.0 の bcfzip に書く（決定 0214・0220）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。小版 45 から
class BcfWriteRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BcfWriteRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  BcfWriteRequest() : super({'op': 'bcf_write'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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
}

/// 書いた bcfzip
class BcfWriteResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  BcfWriteResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  BcfWriteResult() : super({});

  /// 書いた指摘の数
  int? get topics => readField('topics', asInt);
  set topics(int? value) => writeField('topics', value);
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

/// レイヤの表の項目（決定 0162）。小版 22 から、状態とスタイルを持つ
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

  /// ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかの欄を持たない。小版 22 から
  bool? get defined => readField('defined', asBool);
  set defined(bool? value) => writeField('defined', value);

  /// 表示する。小版 22 から
  bool? get visible => readField('visible', asBool);
  set visible(bool? value) => writeField('visible', value);

  /// 印刷する。小版 22 から
  bool? get printable => readField('printable', asBool);
  set printable(bool? value) => writeField('printable', value);

  /// 編集できる。小版 22 から
  bool? get editable => readField('editable', asBool);
  set editable(bool? value) => writeField('editable', value);

  /// 凍結する。小版 22 から
  bool? get frozen => readField('frozen', asBool);
  set frozen(bool? value) => writeField('frozen', value);

  /// 色。inherit を持たない。定義が色を持たなければ書かない。小版 22 から
  ColorChange? get color => readField('color', objectOf(ColorChange.fromJson));
  set color(ColorChange? value) => writeField('color', value);

  /// 線種の ID。定義が線種を持たなければ書かない。小版 22 から
  String? get lineTypeId => readField('line_type_id', asString);
  set lineTypeId(String? value) => writeField('line_type_id', value);

  /// 線幅。inherit を持たない。定義が線幅を持たなければ書かない。小版 22 から
  LineWeightChange? get lineWeight => readField('line_weight', objectOf(LineWeightChange.fromJson));
  set lineWeight(LineWeightChange? value) => writeField('line_weight', value);
}

/// 線種の表の項目（決定 0164）。小版 26 から、定義を持つ
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

  /// ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかの欄を持たない。小版 26 から
  bool? get defined => readField('defined', asBool);
  set defined(bool? value) => writeField('defined', value);

  /// 元の説明。小版 26 から
  String? get description => readField('description', asString);
  set description(String? value) => writeField('description', value);

  /// 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。小版 26 から
  List<double>? get pattern => readField('pattern', listOf(asDouble));
  set pattern(List<double>? value) => writeField('pattern', value);

  /// 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。小版 26 から
  bool? get paper => readField('paper', asBool);
  set paper(bool? value) => writeField('paper', value);

  /// 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。小版 26 から
  bool? get marked => readField('marked', asBool);
  set marked(bool? value) => writeField('marked', value);

  /// 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。小版 28 から
  List<LineTypeMark>? get marks => readField('marks', listOf(objectOf(LineTypeMark.fromJson)));
  set marks(List<LineTypeMark>? value) => writeField('marks', value);
}

/// 寸法スタイルの表の項目（決定 0165）。小版 27 から、端部記号と大きさを持つ
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

  /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
  List<String>? get ends => readField('ends', listOf(asString));
  set ends(List<String>? value) => writeField('ends', value);

  /// 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から
  String? get leader => readField('leader', asString);
  set leader(String? value) => writeField('leader', value);

  /// 端部記号の大きさ。文書の単位で、scale を掛ける前の値。小版 27 から
  double? get size => readField('size', asDouble);
  set size(double? value) => writeField('size', value);

  /// 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。小版 27 から
  double? get tick => readField('tick', asDouble);
  set tick(double? value) => writeField('tick', value);

  /// 大きさに掛ける倍率。0 はビューの縮尺から決める。小版 27 から
  double? get scale => readField('scale', asDouble);
  set scale(double? value) => writeField('scale', value);
}

/// 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。小版 28 から
class LineTypeMark extends Message {
  /// 読んだ JSON のオブジェクトを包む
  LineTypeMark.fromJson(super.json);

  /// 空のオブジェクトから作る
  LineTypeMark() : super({});

  /// 添える列の要素の番号（pattern の添字）
  int? get index => readField('index', asInt);
  set index(int? value) => writeField('index', value);

  /// 形（shape）か文字（text）
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 形の形のファイル。元の形式が指す文字スタイルがなければ書かない
  Font? get font => readField('font', objectOf(Font.fromJson));
  set font(Font? value) => writeField('font', value);

  /// 形の番号
  int? get number => readField('number', asInt);
  set number(int? value) => writeField('number', value);

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
  double? get height => readField('height', asDouble);
  set height(double? value) => writeField('height', value);

  /// 倍率。文字なら、文字スタイルの高さへの倍率
  double? get scale => readField('scale', asDouble);
  set scale(double? value) => writeField('scale', value);

  /// 回転（度、反時計回り）
  double? get rotation => readField('rotation', asDouble);
  set rotation(double? value) => writeField('rotation', value);

  /// 回転の基準
  String? get rotationBasis => readField('rotation_basis', asString);
  set rotationBasis(String? value) => writeField('rotation_basis', value);

  /// 置く位置のずれ（x・y）。線の向きを x とする。単位は pattern と同じ
  List<double>? get offset => readField('offset', listOf(asDouble));
  set offset(List<double>? value) => writeField('offset', value);
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

/// 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。小版 20 から
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

  /// ファイルに定義がある。偽なら、ほかの欄を持たない
  bool? get defined => readField('defined', asBool);
  set defined(bool? value) => writeField('defined', value);

  /// 主のフォント。ファイルが決めていなければ書かない
  Font? get font => readField('font', objectOf(Font.fromJson));
  set font(Font? value) => writeField('font', value);

  /// 大きいフォント（漢字などの形のファイル）。なければ書かない
  Font? get bigFont => readField('big_font', objectOf(Font.fromJson));
  set bigFont(Font? value) => writeField('big_font', value);

  /// 決まった高さ。文書の単位。決めなければ書かない
  double? get height => readField('height', asDouble);
  set height(double? value) => writeField('height', value);

  /// 幅の倍率
  double? get width => readField('width', asDouble);
  set width(double? value) => writeField('width', value);

  /// 傾き（度）
  double? get oblique => readField('oblique', asDouble);
  set oblique(double? value) => writeField('oblique', value);

  /// 左右を裏返す
  bool? get backward => readField('backward', asBool);
  set backward(bool? value) => writeField('backward', value);

  /// 上下を反転する
  bool? get upsideDown => readField('upside_down', asBool);
  set upsideDown(bool? value) => writeField('upside_down', value);

  /// 縦書き
  bool? get vertical => readField('vertical', asBool);
  set vertical(bool? value) => writeField('vertical', value);
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
  bool? get bold => readField('bold', asBool);
  set bold(bool? value) => writeField('bold', value);

  /// 斜体。名前で指すフォントだけ
  bool? get italic => readField('italic', asBool);
  set italic(bool? value) => writeField('italic', value);

  /// 文字集合の番号（128 は Shift_JIS）。名前で指すフォントで、元の形式が決めていれば
  int? get charset => readField('charset', asInt);
  set charset(int? value) => writeField('charset', value);

  /// ピッチと系統の番号。名前で指すフォントで、元の形式が決めていれば
  int? get pitch => readField('pitch', asInt);
  set pitch(int? value) => writeField('pitch', value);
}

/// フォントの種類。shape は形のファイル（SHX など）、named は名前で指すフォント（TrueType など）
///
/// 列挙の値。知らない値も受け取る
abstract final class FontKind {
  static const shape = 'shape';
  static const named = 'named';
}

/// 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つ鍵（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない
class Source extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Source.fromJson(super.json);

  /// 空のオブジェクトから作る
  Source() : super({});

  /// L1 のレコードの番号
  int? get record => readField('record', asInt);
  set record(int? value) => writeField('record', value);

  /// L1 のバイト範囲
  ByteRange? get bytes => readField('bytes', objectOf(ByteRange.fromJson));
  set bytes(ByteRange? value) => writeField('bytes', value);

  /// AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない
  String? get handle => readField('handle', asString);
  set handle(String? value) => writeField('handle', value);

  /// SXF のインスタンスの番号（#n の n）。10 進の文字列。小版 6 から
  String? get instance => readField('instance', asString);
  set instance(String? value) => writeField('instance', value);
}

/// 応答。失敗したときも同じ形で、result を持たない
class Response extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Response.fromJson(super.json);

  /// 空のオブジェクトから作る
  Response() : super({});

  /// メッセージのスキーマの大版
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

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
  int? get count => readField('count', asInt);
  set count(int? value) => writeField('count', value);

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
  int? get record => readField('record', asInt);
  set record(int? value) => writeField('record', value);

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
  int? get start => readField('start', asInt);
  set start(int? value) => writeField('start', value);

  /// 終わりの位置（含まない）
  int? get end => readField('end', asInt);
  set end(int? value) => writeField('end', value);
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
  int? get id => readField('id', asInt);
  set id(int? value) => writeField('id', value);

  /// 種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// バイト数
  int? get size => readField('size', asInt);
  set size(int? value) => writeField('size', value);
}

/// バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）
///
/// 列挙の値。知らない値も受け取る
abstract final class BlobKind {
  static const file = 'file';
  static const image = 'image';
  static const scene = 'scene';
  static const arrow = 'arrow';
}

/// zm_session_new に渡す設定。渡さなければ既定の設定
class SessionOptions extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SessionOptions.fromJson(super.json);

  /// 空のオブジェクトから作る
  SessionOptions() : super({});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 実行の形。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013）
  String? get engine => readField('engine', asString);
  set engine(String? value) => writeField('engine', value);

  /// engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200）
  String? get processPath => readField('process_path', asString);
  set processPath(String? value) => writeField('process_path', value);
}

/// 実行の形（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
///
/// 列挙の値。知らない値も受け取る
abstract final class Engine {
  static const native = 'native';
  static const process = 'process';
  static const wasm = 'wasm';
}

/// 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面のモジュールに置く。小版 46 から（doc と id は小版 47 から）
class CivilRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  CivilRequest() : super({'op': 'civil'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 線形。doc と一緒には書かない
  CivilAlignment? get alignment => readField('alignment', objectOf(CivilAlignment.fromJson));
  set alignment(CivilAlignment? value) => writeField('alignment', value);

  /// open で開いた J-LandXML の文書の名前。小版 47 から
  String? get doc => readField('doc', asString);
  set doc(String? value) => writeField('doc', value);

  /// doc の線形の ID。文書に線形が 1 つなら書かなくてよい。小版 47 から
  String? get id => readField('id', asString);
  set id(String? value) => writeField('id', value);

  /// 追加距離（メートル）の列
  List<double>? get distances => readField('distances', listOf(asDouble));
  set distances(List<double>? value) => writeField('distances', value);

  /// 測点の表記（No.12+5.000・No.12+5・No.12）の列。測点の不連続のために同じ値が何か所にもあれば、最初の所にして診断する
  List<String>? get stations => readField('stations', listOf(asString));
  set stations(List<String>? value) => writeField('stations', value);

  /// 線形の初めから終わりまで、この間隔（メートル）ごとの所を足す。終わりの所も足す
  double? get interval => readField('interval', asDouble);
  set interval(double? value) => writeField('interval', value);

  /// 測点の間隔（メートル）。書かなければ 20
  double? get pitch => readField('pitch', asDouble);
  set pitch(double? value) => writeField('pitch', value);

  /// 測点の表記の端数の小数の桁数。書かなければ 3
  int? get digits => readField('digits', asInt);
  set digits(int? value) => writeField('digits', value);
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
  double? get startStation => readField('start_station', asDouble);
  set startStation(double? value) => writeField('start_station', value);

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
  double? get northing => readField('northing', asDouble);
  set northing(double? value) => writeField('northing', value);

  /// 初めの点の東（メートル）
  double? get easting => readField('easting', asDouble);
  set easting(double? value) => writeField('easting', value);

  /// 初めの方向角（北から時計回りのラジアン）
  double? get direction => readField('direction', asDouble);
  set direction(double? value) => writeField('direction', value);

  /// 長さ（メートル）
  double? get length => readField('length', asDouble);
  set length(double? value) => writeField('length', value);

  /// 円弧の半径（メートル）。arc に書く
  double? get radius => readField('radius', asDouble);
  set radius(double? value) => writeField('radius', value);

  /// 緩和曲線の初めの半径（メートル）。直線の側は書かない
  double? get startRadius => readField('start_radius', asDouble);
  set startRadius(double? value) => writeField('start_radius', value);

  /// 緩和曲線の終わりの半径（メートル）。直線の側は書かない
  double? get endRadius => readField('end_radius', asDouble);
  set endRadius(double? value) => writeField('end_radius', value);

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
  double? get startDistance => readField('start_distance', asDouble);
  set startDistance(double? value) => writeField('start_distance', value);

  /// 初めの高さ（メートル）
  double? get startHeight => readField('start_height', asDouble);
  set startHeight(double? value) => writeField('start_height', value);

  /// 初めの勾配（高さ ÷ 平面の距離）
  double? get startGradient => readField('start_gradient', asDouble);
  set startGradient(double? value) => writeField('start_gradient', value);

  /// 平面の距離での長さ（メートル）
  double? get length => readField('length', asDouble);
  set length(double? value) => writeField('length', value);

  /// 放物線の終わりの勾配。parabola に書く
  double? get endGradient => readField('end_gradient', asDouble);
  set endGradient(double? value) => writeField('end_gradient', value);

  /// 円弧の半径（メートル）。上に凸のものを正にする。circular_arc に書く
  double? get radius => readField('radius', asDouble);
  set radius(double? value) => writeField('radius', value);
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
  double? get distance => readField('distance', asDouble);
  set distance(double? value) => writeField('distance', value);

  /// 戻りの測点の値（不連続の手前から数えた値）
  double? get back => readField('back', asDouble);
  set back(double? value) => writeField('back', value);

  /// 進みの測点の値（不連続の先で数え直す値）
  double? get ahead => readField('ahead', asDouble);
  set ahead(double? value) => writeField('ahead', value);
}

/// 線形の上の所
class CivilResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilResult() : super({});

  /// 平面線形の長さ（メートル）
  double? get length => readField('length', asDouble);
  set length(double? value) => writeField('length', value);

  /// 求めた所
  List<CivilPosition>? get positions => readField('positions', listOf(objectOf(CivilPosition.fromJson)));
  set positions(List<CivilPosition>? value) => writeField('positions', value);

  /// 所を求めた doc の線形の ID。小版 47 から
  String? get alignment => readField('alignment', asString);
  set alignment(String? value) => writeField('alignment', value);

  /// doc の線形の要約。doc を渡したときに返す。小版 47 から
  List<CivilAlignmentSummary>? get alignments => readField('alignments', listOf(objectOf(CivilAlignmentSummary.fromJson)));
  set alignments(List<CivilAlignmentSummary>? value) => writeField('alignments', value);

  /// doc の地表面（TIN）の要約。doc を渡したときに返す。小版 47 から
  List<CivilSurfaceSummary>? get surfaces => readField('surfaces', listOf(objectOf(CivilSurfaceSummary.fromJson)));
  set surfaces(List<CivilSurfaceSummary>? value) => writeField('surfaces', value);

  /// doc の水平の座標系の EPSG のコード。分からなければ書かない。小版 47 から
  int? get crs => readField('crs', asInt);
  set crs(int? value) => writeField('crs', value);
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
  double? get length => readField('length', asDouble);
  set length(double? value) => writeField('length', value);

  /// 線形の初めの測点の値（メートル）
  double? get startStation => readField('start_station', asDouble);
  set startStation(double? value) => writeField('start_station', value);

  /// 平面線形の区間の数
  int? get horizontal => readField('horizontal', asInt);
  set horizontal(int? value) => writeField('horizontal', value);

  /// 縦断線形の区間の数
  int? get vertical => readField('vertical', asInt);
  set vertical(int? value) => writeField('vertical', value);

  /// 測点の不連続の数
  int? get breaks => readField('breaks', asInt);
  set breaks(int? value) => writeField('breaks', value);

  /// 横断の数
  int? get crossSections => readField('cross_sections', asInt);
  set crossSections(int? value) => writeField('cross_sections', value);
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
  int? get points => readField('points', asInt);
  set points(int? value) => writeField('points', value);

  /// 三角形の数
  int? get triangles => readField('triangles', asInt);
  set triangles(int? value) => writeField('triangles', value);
}

/// 線形の上の 1 つの所
class CivilPosition extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilPosition.fromJson(super.json);

  /// 空のオブジェクトから作る
  CivilPosition() : super({});

  /// 追加距離（メートル）
  double? get distance => readField('distance', asDouble);
  set distance(double? value) => writeField('distance', value);

  /// 測点の値（メートル）
  double? get station => readField('station', asDouble);
  set station(double? value) => writeField('station', value);

  /// 測点の表記。表記にできなければ書かない
  String? get label => readField('label', asString);
  set label(String? value) => writeField('label', value);

  /// 点の北（メートル）
  double? get northing => readField('northing', asDouble);
  set northing(double? value) => writeField('northing', value);

  /// 点の東（メートル）
  double? get easting => readField('easting', asDouble);
  set easting(double? value) => writeField('easting', value);

  /// 接線の方向角（北から時計回りのラジアン）
  double? get direction => readField('direction', asDouble);
  set direction(double? value) => writeField('direction', value);

  /// 縦断線形の高さ（メートル）。求められなければ書かない
  double? get height => readField('height', asDouble);
  set height(double? value) => writeField('height', value);

  /// 縦断線形の勾配。高さと同じく、求められなければ書かない
  double? get gradient => readField('gradient', asDouble);
  set gradient(double? value) => writeField('gradient', value);
}

/// open で開いた点群（LAS・LAZ）の要約と、条件に合う点を返す。点は八分木の浅い段の節から順に選ぶ（決定 0232）。図面のモジュールに置く。小版 48 から
class PointcloudRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointcloudRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  PointcloudRequest() : super({'op': 'pointcloud'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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
  double? get tolerance => readField('tolerance', asDouble);
  set tolerance(double? value) => writeField('tolerance', value);

  /// 返す点の数の上限。書かなければ点を返さず、要約だけを返す
  int? get limit => readField('limit', asInt);
  set limit(int? value) => writeField('limit', value);
}

/// 地図の空間の箱。北・東・標高（メートル）の最小と最大
class PointcloudBox extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointcloudBox.fromJson(super.json);

  /// 空のオブジェクトから作る
  PointcloudBox() : super({});

  /// 最小の北・東・標高
  List<double>? get min => readField('min', listOf(asDouble));
  set min(List<double>? value) => writeField('min', value);

  /// 最大の北・東・標高
  List<double>? get max => readField('max', listOf(asDouble));
  set max(List<double>? value) => writeField('max', value);
}

/// 点群の要約と、選んだ点
class PointcloudResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PointcloudResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  PointcloudResult() : super({});

  /// 点の数
  int? get points => readField('points', asInt);
  set points(int? value) => writeField('points', value);

  /// 点の範囲。点がなければ書かない
  PointcloudBox? get bounds => readField('bounds', objectOf(PointcloudBox.fromJson));
  set bounds(PointcloudBox? value) => writeField('bounds', value);

  /// 水平の座標系の EPSG のコード。分からなければ書かない
  int? get crs => readField('crs', asInt);
  set crs(int? value) => writeField('crs', value);

  /// 高さの座標系の EPSG のコード。分からなければ書かない
  int? get vertical => readField('vertical', asInt);
  set vertical(int? value) => writeField('vertical', value);

  /// 点が持つ属性（position・intensity・classification・color・returns・time）
  List<String>? get attributes => readField('attributes', listOf(asString));
  set attributes(List<String>? value) => writeField('attributes', value);

  /// 八分木のいちばん深い段
  int? get depth => readField('depth', asInt);
  set depth(int? value) => writeField('depth', value);

  /// 八分木の節の数
  int? get nodes => readField('nodes', asInt);
  set nodes(int? value) => writeField('nodes', value);

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
  List<List<double>>? get positions => readField('positions', listOf(listOf(asDouble)));
  set positions(List<List<double>>? value) => writeField('positions', value);

  /// 強度。点群が持つときに返す
  List<int>? get intensity => readField('intensity', listOf(asInt));
  set intensity(List<int>? value) => writeField('intensity', value);

  /// 分類。点群が持つときに返す
  List<int>? get classification => readField('classification', listOf(asInt));
  set classification(List<int>? value) => writeField('classification', value);

  /// 色（赤・緑・青。16 ビット）。点群が持つときに返す
  List<List<int>>? get color => readField('color', listOf(listOf(asInt)));
  set color(List<List<int>>? value) => writeField('color', value);

  /// limit のために選ばなかった点がある
  bool? get truncated => readField('truncated', asBool);
  set truncated(bool? value) => writeField('truncated', value);

  /// 選んだいちばん深い段
  int? get level => readField('level', asInt);
  set level(int? value) => writeField('level', value);
}

/// 電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。小版 53 から
class DeliveryCheckRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryCheckRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  DeliveryCheckRequest() : super({'op': 'delivery_check'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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
  int? get offset => readField('offset', asInt);
  set offset(int? value) => writeField('offset', value);

  /// 返す指摘の数の上限。書かなければ 10000
  int? get limit => readField('limit', asInt);
  set limit(int? value) => writeField('limit', value);
}

/// 成果品の要約と、照合の結果。照合の結果は check と同じ形
class DeliveryCheckResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeliveryCheckResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeliveryCheckResult() : super({});

  /// 成果品のファイルの数
  int? get files => readField('files', asInt);
  set files(int? value) => writeField('files', value);

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
  int? get total => readField('total', asInt);
  set total(int? value) => writeField('total', value);

  /// 続きの指摘の位置。続きがなければ書かない
  int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(int? value) => writeField('next_offset', value);
}

/// 工事写真（JPEG）の Exif と XMP を読み、画素数・向き・撮影の日時・位置などを返す（#387）。画像は復号しない。図面のモジュールに置く。小版 55 から
class PhotoRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  PhotoRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  PhotoRequest() : super({'op': 'photo'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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

  /// 画素の幅（フレームの見出し）
  int? get width => readField('width', asInt);
  set width(int? value) => writeField('width', value);

  /// 画素の高さ（フレームの見出し）
  int? get height => readField('height', asInt);
  set height(int? value) => writeField('height', value);

  /// 向き（Exif の Orientation、1〜8）
  int? get orientation => readField('orientation', asInt);
  set orientation(int? value) => writeField('orientation', value);

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
  double? get direction => readField('direction', asDouble);
  set direction(double? value) => writeField('direction', value);

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
  double? get latitude => readField('latitude', asDouble);
  set latitude(double? value) => writeField('latitude', value);

  /// 経度（度、東が正）
  double? get longitude => readField('longitude', asDouble);
  set longitude(double? value) => writeField('longitude', value);

  /// 高さ（m、海面より下は負）
  double? get altitude => readField('altitude', asDouble);
  set altitude(double? value) => writeField('altitude', value);
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

/// 表（Excel の xlsx と CSV）を、様式を知らずに読み、シートごとのセルの値・数式・結合セルを返す（#391）。数式は計算せず、保存された値を返す。図面のモジュールに置く。小版 56 から
class SheetRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SheetRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  SheetRequest() : super({'op': 'sheet'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// 表の入力の名前
  String? get input => readField('input', asString);
  set input(String? value) => writeField('input', value);

  /// 形式。書かなければ、ZIP の署名で始まれば xlsx、そうでなければ csv とする
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// CSV の文字コード。書かなければ、BOM か UTF-8 として正しければ utf_8、そうでなければ shift_jis とする
  String? get encoding => readField('encoding', asString);
  set encoding(String? value) => writeField('encoding', value);

  /// CSV の区切りの字。書かなければ comma
  String? get delimiter => readField('delimiter', asString);
  set delimiter(String? value) => writeField('delimiter', value);
}

/// 表の形式
///
/// 列挙の値。知らない値も受け取る
abstract final class SheetFormat {
  static const xlsx = 'xlsx';
  static const csv = 'csv';
}

/// CSV の文字コード。shift_jis は Windows の拡張を含む
///
/// 列挙の値。知らない値も受け取る
abstract final class SheetEncoding {
  static const utf8 = 'utf_8';
  static const shiftJis = 'shift_jis';
}

/// CSV の区切りの字。comma は「,」、tab はタブ、semicolon は「;」
///
/// 列挙の値。知らない値も受け取る
abstract final class SheetDelimiter {
  static const comma = 'comma';
  static const tab = 'tab';
  static const semicolon = 'semicolon';
}

/// 読んだ表
class SheetResult extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SheetResult.fromJson(super.json);

  /// 空のオブジェクトから作る
  SheetResult() : super({});

  /// 読んだ形式
  String? get format => readField('format', asString);
  set format(String? value) => writeField('format', value);

  /// CSV を読んだ文字コード。xlsx では書かない
  String? get encoding => readField('encoding', asString);
  set encoding(String? value) => writeField('encoding', value);

  /// シート。ブックの順。CSV は 1 つ。ワークシートでないシートは含めない
  List<Sheet>? get sheets => readField('sheets', listOf(objectOf(Sheet.fromJson)));
  set sheets(List<Sheet>? value) => writeField('sheets', value);
}

/// シート
class Sheet extends Message {
  /// 読んだ JSON のオブジェクトを包む
  Sheet.fromJson(super.json);

  /// 空のオブジェクトから作る
  Sheet() : super({});

  /// 名前。CSV は空
  String? get name => readField('name', asString);
  set name(String? value) => writeField('name', value);

  /// 行の数（値のある最後の行と、結合セルの最後の行まで）
  int? get rows => readField('rows', asInt);
  set rows(int? value) => writeField('rows', value);

  /// 列の数（値のある最後の列と、結合セルの最後の列まで）
  int? get columns => readField('columns', asInt);
  set columns(int? value) => writeField('columns', value);

  /// 値か数式のあるセル。行の順、行の中は列の順
  List<SheetCell>? get cells => readField('cells', listOf(objectOf(SheetCell.fromJson)));
  set cells(List<SheetCell>? value) => writeField('cells', value);

  /// 結合セル。ファイルの順
  List<SheetArea>? get merged => readField('merged', listOf(objectOf(SheetArea.fromJson)));
  set merged(List<SheetArea>? value) => writeField('merged', value);
}

/// セル。行と列は 0 から数える
class SheetCell extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SheetCell.fromJson(super.json);

  /// 空のオブジェクトから作る
  SheetCell() : super({});

  /// 行
  int? get row => readField('row', asInt);
  set row(int? value) => writeField('row', value);

  /// 列
  int? get column => readField('column', asInt);
  set column(int? value) => writeField('column', value);

  /// 値の種類
  String? get kind => readField('kind', asString);
  set kind(String? value) => writeField('kind', value);

  /// 値。数は保存された字のまま、真偽は TRUE か FALSE。日付の書式は解釈しないので、日付は数のまま
  String? get value => readField('value', asString);
  set value(String? value) => writeField('value', value);

  /// 数式（先頭の = を除く）。共有の数式は、元のセルだけが持つ
  String? get formula => readField('formula', asString);
  set formula(String? value) => writeField('formula', value);
}

/// セルの値の種類。CSV の値はすべて text
///
/// 列挙の値。知らない値も受け取る
abstract final class SheetCellKind {
  static const text = 'text';
  static const number = 'number';
  static const boolean = 'boolean';
  static const error = 'error';
}

/// 結合セルの範囲。両端を含む
class SheetArea extends Message {
  /// 読んだ JSON のオブジェクトを包む
  SheetArea.fromJson(super.json);

  /// 空のオブジェクトから作る
  SheetArea() : super({});

  /// 最初の行
  int? get firstRow => readField('first_row', asInt);
  set firstRow(int? value) => writeField('first_row', value);

  /// 最初の列
  int? get firstColumn => readField('first_column', asInt);
  set firstColumn(int? value) => writeField('first_column', value);

  /// 最後の行
  int? get lastRow => readField('last_row', asInt);
  set lastRow(int? value) => writeField('last_row', value);

  /// 最後の列
  int? get lastColumn => readField('last_column', asInt);
  set lastColumn(int? value) => writeField('last_column', value);
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
  int? get records => readField('records', asInt);
  set records(int? value) => writeField('records', value);

  /// ファイルの名前の形の値の数
  int? get links => readField('links', asInt);
  set links(int? value) => writeField('links', value);

  /// 成果品の中に見つからないファイルの名前。書いた順
  List<String>? get missing => readField('missing', listOf(asString));
  set missing(List<String>? value) => writeField('missing', value);
}

/// open で開いた CityGML の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。図面のモジュールに置く。小版 52 から
class GeoRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  GeoRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  GeoRequest() : super({'op': 'geo'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// open で開いた CityGML の文書の名前
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
  int? get crs => readField('crs', asInt);
  set crs(int? value) => writeField('crs', value);

  /// 高さの座標系の EPSG のコード。分からなければ書かない
  int? get height => readField('height', asInt);
  set height(int? value) => writeField('height', value);

  /// 地物の数
  int? get features => readField('features', asInt);
  set features(int? value) => writeField('features', value);

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
  int? get count => readField('count', asInt);
  set count(int? value) => writeField('count', value);
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
  int? get children => readField('children', asInt);
  set children(int? value) => writeField('children', value);

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
  int? get lod => readField('lod', asInt);
  set lod(int? value) => writeField('lod', value);

  /// 形の種類
  String? get form => readField('form', asString);
  set form(String? value) => writeField('form', value);

  /// 点の数。立体と面の集まりは、面の輪の点の数の和
  int? get points => readField('points', asInt);
  set points(int? value) => writeField('points', value);

  /// 立体と面の集まりの面の数
  int? get faces => readField('faces', asInt);
  set faces(int? value) => writeField('faces', value);
}

/// open で開いた J-LandXML の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。小版 51 から
class CivilDrawingRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  CivilDrawingRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  CivilDrawingRequest() : super({'op': 'civil_drawing'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

  /// op の名前
  String? get op => readField('op', asString);
  set op(String? value) => writeField('op', value);

  /// open で開いた J-LandXML の文書の名前
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
  int? get elements => readField('elements', asInt);
  set elements(int? value) => writeField('elements', value);

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
  double? get plan => readField('plan', asDouble);
  set plan(double? value) => writeField('plan', value);

  /// 縦断図の横の尺度の分母
  double? get horizontal => readField('horizontal', asDouble);
  set horizontal(double? value) => writeField('horizontal', value);

  /// 縦断図の縦の尺度の分母
  double? get vertical => readField('vertical', asDouble);
  set vertical(double? value) => writeField('vertical', value);

  /// 横断図の尺度の分母
  double? get crossSection => readField('cross_section', asDouble);
  set crossSection(double? value) => writeField('cross_section', value);

  /// 文字の高さ（mm）
  double? get textHeight => readField('text_height', asDouble);
  set textHeight(double? value) => writeField('text_height', value);

  /// 測点の間隔（メートル）
  double? get stationPitch => readField('station_pitch', asDouble);
  set stationPitch(double? value) => writeField('station_pitch', value);
}

/// 開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。図面のモジュールに置く。小版 50 から
class DeviationRequest extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeviationRequest.fromJson(super.json);

  /// 目印の欄だけを持つオブジェクトから作る
  DeviationRequest() : super({'op': 'deviation'});

  /// メッセージのスキーマの大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
  Budget? get budget => readField('budget', objectOf(Budget.fromJson));
  set budget(Budget? value) => writeField('budget', value);

  /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
  bool? get partial => readField('partial', asBool);
  set partial(bool? value) => writeField('partial', value);

  /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
  int? get maxPlaces => readField('max_places', asInt);
  set maxPlaces(int? value) => writeField('max_places', value);

  /// 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
  int? get timeLimitMs => readField('time_limit_ms', asInt);
  set timeLimitMs(int? value) => writeField('time_limit_ms', value);

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
  int? get offset => readField('offset', asInt);
  set offset(int? value) => writeField('offset', value);

  /// 返す評価した点の数の上限。書かなければ点を返さない
  int? get limit => readField('limit', asInt);
  set limit(int? value) => writeField('limit', value);
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
  int? get points => readField('points', asInt);
  set points(int? value) => writeField('points', value);

  /// TIN の外なので評価しなかった点の数
  int? get outside => readField('outside', asInt);
  set outside(int? value) => writeField('outside', value);

  /// どの規則の勾配にも入らない三角形の上なので評価しなかった点の数
  int? get unmatched => readField('unmatched', asInt);
  set unmatched(int? value) => writeField('unmatched', value);

  /// 区分の境の近く（規則の exclude の内）なので除いた点の数
  int? get excluded => readField('excluded', asInt);
  set excluded(int? value) => writeField('excluded', value);

  /// 規則ごとの集計。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
  List<DeviationRule>? get rules => readField('rules', listOf(objectOf(DeviationRule.fromJson)));
  set rules(List<DeviationRule>? value) => writeField('rules', value);

  /// 指摘。規則の順。規格値を外れれば profile.surface-elevation.invalid、確認できなければ profile.surface-elevation.unsupported
  List<CheckFinding>? get findings => readField('findings', listOf(objectOf(CheckFinding.fromJson)));
  set findings(List<CheckFinding>? value) => writeField('findings', value);

  /// 評価した点の数
  int? get evaluated => readField('evaluated', asInt);
  set evaluated(int? value) => writeField('evaluated', value);

  /// 評価した点。点群の順に、offset から limit 個まで
  List<DeviationPoint>? get differences => readField('differences', listOf(objectOf(DeviationPoint.fromJson)));
  set differences(List<DeviationPoint>? value) => writeField('differences', value);

  /// 続きの最初の評価した点の位置。続きがなければ持たない
  int? get nextOffset => readField('next_offset', asInt);
  set nextOffset(int? value) => writeField('next_offset', value);
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
  int? get points => readField('points', asInt);
  set points(int? value) => writeField('points', value);

  /// 区分の三角形の、平面に投影した面積（m²）
  double? get area => readField('area', asDouble);
  set area(double? value) => writeField('area', value);

  /// 計測の密度（点／m²）。面積がなければ書かない
  double? get density => readField('density', asDouble);
  set density(double? value) => writeField('density', value);

  /// 差の平均。点がなければ書かない
  double? get mean => readField('mean', asDouble);
  set mean(double? value) => writeField('mean', value);

  /// 差の最小。点がなければ書かない
  double? get min => readField('min', asDouble);
  set min(double? value) => writeField('min', value);

  /// 差の最大。点がなければ書かない
  double? get max => readField('max', asDouble);
  set max(double? value) => writeField('max', value);

  /// 指摘の数（確認できなかったものを除く）
  int? get findings => readField('findings', asInt);
  set findings(int? value) => writeField('findings', value);

  /// 確認できなかった数
  int? get unsupported => readField('unsupported', asInt);
  set unsupported(int? value) => writeField('unsupported', value);
}

/// 評価した点
class DeviationPoint extends Message {
  /// 読んだ JSON のオブジェクトを包む
  DeviationPoint.fromJson(super.json);

  /// 空のオブジェクトから作る
  DeviationPoint() : super({});

  /// 点群の中の番号
  int? get index => readField('index', asInt);
  set index(int? value) => writeField('index', value);

  /// 北・東・標高（メートル）
  List<double>? get position => readField('position', listOf(asDouble));
  set position(List<double>? value) => writeField('position', value);

  /// 点の標高から設計面の標高を引いた差（メートル）
  double? get difference => readField('difference', asDouble);
  set difference(double? value) => writeField('difference', value);

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

  /// カタログの形の大版。今は 1
  int? get v => readField('v', asInt);
  set v(int? value) => writeField('v', value);

  /// 言語（ja・en）
  String? get lang => readField('lang', asString);
  set lang(String? value) => writeField('lang', value);

  /// パッケージの版。コアと同じ版で出す
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
  static const apply = 'apply';
  static const convert = 'convert';
  static const tables = 'tables';
  static const scene = 'scene';
  static const check = 'check';
  static const validate = 'validate';
  static const reviewCarry = 'review_carry';
  static const reviewAdopt = 'review_adopt';
  static const building = 'building';
  static const buildingElements = 'building_elements';
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
  static const photo = 'photo';
  static const sheet = 'sheet';
}
