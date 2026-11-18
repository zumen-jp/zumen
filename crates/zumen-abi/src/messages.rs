// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。

//! API のメッセージの型（決定 0012）。スキーマの定義ごとに 1 つの型がある。

use std::collections::BTreeMap;

use zumen_json::{self as json, FromJson, Invalid, ObjectReader, Problem, ToJson, Value, Writer};

/// 要求。op の値で形が決まる
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Request {
    /// コアのバージョンと、使える op・形式を尋ねる
    Capabilities(CapabilitiesRequest),
    /// 入力を読んで、文書を作る。形式は入力から判定する
    Open(OpenRequest),
    /// 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
    Close(CloseRequest),
    /// 診断の文を、カタログから組み立てる（決定 0009）
    Describe(DescribeRequest),
    /// 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合テストで比べるためにも使う（決定 0007）
    Drawing(DrawingRequest),
    /// 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）
    Entities(EntitiesRequest),
    /// 開いた文書を図面モデルに変換し、用紙の距離の校正をモデルの順に返す。マイナーバージョン 68 から
    Calibrations(CalibrationsRequest),
    /// 用紙と校正を明示的に選び、始点と終点の校正後の距離を求める。両点は校正の閉領域内にある必要がある。マイナーバージョン 68 から
    MeasureDistance(MeasureDistanceRequest),
    /// モデル空間の要素の長さ・周長・面積、2 要素または点の間の最短距離を測る。線幅と表示用の変換は含めない
    Measure(MeasureRequest),
    /// 同じ実寸の空間の 2 要素の交点・接点・重なりを求める。文書を変更しない。マイナーバージョン 90 から
    Intersect(IntersectRequest),
    /// 同じ実寸の空間の要素の線が囲む閉領域を検出する、または閉じた要素どうしの和・積・差・排他的論理和を求める。文書を変更しない。マイナーバージョン 91 から
    Regions(RegionsRequest),
    /// 同じ実寸の空間の要素のオフセット・トリム・延長・面取り・フィレットの結果の形を求める。文書を変更しない。結果は apply の remove・replace・add で当てる。形の種類が変わるときは replace でなく remove と add にする。マイナーバージョン 92 から
    Construct(ConstructRequest),
    /// 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。マイナーバージョン 7 から
    Apply(ApplyRequest),
    /// 入力なしで空の文書を作り、開く。初めは R2004 の DXF。既定のレイヤとスタイル、空のモデル空間と Layout1 を持つ。書いたファイルは blobs で返す。マイナーバージョン 83 から
    Create(CreateRequest),
    /// 開いた文書から別の形式のファイルを作り、新しい文書として開く。図面からは DXF（対応する R2004 DWG の弧長寸法は共有オブジェクト表から、それ以外は図面モデルから。決定 0139・0206・0281）または SIMA（.sim）、土木モデル（J-LandXML・IFC4.3・SIMA）からは SIMA（.sim）、地理の領域モデルの文書（CityGML・基盤地図情報・拡張 DM・GeoJSON・SIMA・GeoTIFF）からは GeoJSON または SIMA（.sim）を書く。書けないデータは種類ごとの件数を診断で返す。書いたファイルは blobs で返す。マイナーバージョン 41 から。geojson はマイナーバージョン 71 から。sima はマイナーバージョン 72 から
    Convert(ConvertRequest),
    /// 開いた図面を変換したときの忠実度と欠落を、convert と同じ書き手で見積もる。文書もファイルの blob も登録しない。DXF・SXF（P21）・JWW への変換に対応する。マイナーバージョン 66 から（SXF・JWW は 85 から）
    ConvertEstimate(ConvertEstimateRequest),
    /// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。マイナーバージョン 8 から
    Tables(TablesRequest),
    /// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。マイナーバージョン 33 から
    Scene(SceneRequest),
    /// 建物モデルを開口付きの 3D シーンにし、ZMS3 の blob で返す。86 から地物の面と高さのある線・面も扱う。階・クラス・ID の各条件の中は和、条件どうしは積。箱は要素の重なりで選び、メッシュを切らない。88 から点群を元の八分木の節ごとの点の区画（ZMS3 1.1）で返す。点群の点は pointcloud と同じ節の順で選び、座標系と高さの基準を変換しない。94 から平面直角座標系の GeoTIFF の DEM も、外枠の地物でなく標高格子を点群と同じ点の区画で返す。マイナーバージョン 82 から
    Scene3d(Scene3dRequest),
    /// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。マイナーバージョン 34 から
    Check(CheckRequest),
    /// zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。マイナーバージョン 42 から
    Validate(ValidateRequest),
    /// レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書のバージョンへ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元のバージョンのハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。マイナーバージョン 43 から
    ReviewCarry(ReviewCarryRequest),
    /// 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。マイナーバージョン 43 から
    ReviewAdopt(ReviewAdoptRequest),
    /// レビューの文書（決定 0214・0295）の 1 つの対象の図形と指摘の場所を、開いた図面の起点で、図面とは別の 2D のシーンにして blob で返す。対象の形式とハッシュは開いた文書から作り、文書の対象と合わなければ描かない。シーンの配置は scene と同じ schema/scene.toml。色・線幅・寸法は style で明示し、文書の値や旧版の値から推測しない。レビューの文書と図面は変えない。マイナーバージョン 95 から
    ReviewScene(ReviewSceneRequest),
    /// 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。マイナーバージョン 44 から
    Building(BuildingRequest),
    /// 開いた IFC の建物モデルの要素を、書いた順に返す。コンポーネントは components で選ぶ（決定 0207・0219）。マイナーバージョン 44 から
    BuildingElements(BuildingElementsRequest),
    /// 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。マイナーバージョン 44 から
    IdsCheck(IdsCheckRequest),
    /// 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。マイナーバージョン 44 から
    ReviewAdoptIds(ReviewAdoptIdsRequest),
    /// zm_push で渡した BCF 2.1 か 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。マイナーバージョン 45 から
    BcfRead(BcfReadRequest),
    /// zm_push で渡したレビューの文書を、BCF の bcfzip に書く（決定 0214・0220）。バージョンは version で選ぶ（マイナーバージョン 61 から。それまでは 3.0 だけ）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。マイナーバージョン 45 から
    BcfWrite(BcfWriteRequest),
    /// 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML・IFC4.3・SIMA の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面・BIM・土木地理・点群のモジュールに置く（決定 0253）。マイナーバージョン 46 から（doc と id はマイナーバージョン 47 から）
    Civil(CivilRequest),
    /// open で開いた点群（LAS・LAZ・PLY・XYZ・E57・GeoTIFF の DEM）の要約と、条件に合う点を返す。点は八分木の浅い段のノードから順に選ぶ（決定 0232）。点群のモジュールに置く（決定 0253）。マイナーバージョン 48 から
    Pointcloud(PointcloudRequest),
    /// 開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。点群のモジュールに置く（決定 0253）。マイナーバージョン 50 から
    Deviation(DeviationRequest),
    /// open で開いた J-LandXML・IFC4.3・SIMA の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。マイナーバージョン 51 から
    CivilDrawing(CivilDrawingRequest),
    /// open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。土木・地理のモジュールに置く（決定 0253）。マイナーバージョン 52 から
    Geo(GeoRequest),
    /// 電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。マイナーバージョン 53 から
    DeliveryCheck(DeliveryCheckRequest),
    /// 管理 XML の項目を変更し、file blob を返す。入力は変更しない。すべての経路は元の文書で解き、失敗時は結果を返さない。図面のモジュールに置く。マイナーバージョン 84 から
    DeliveryApply(DeliveryApplyRequest),
    /// 工事写真（JPEG）の Exif と XMP を読み、画素数・向き・撮影の日時・位置などを返す（#387）。画像は復号しない。図面のモジュールに置く。マイナーバージョン 55 から
    Photo(PhotoRequest),
    /// 地質・土質調査のボーリング交換用データ（BED0500.DTD の XML）を読み、ボーリングの位置・標高と、地層・色調・観察記事・標準貫入試験・孔内水位を返す（#390）。ほかの試験と判定表は読まない。図面のモジュールに置く。マイナーバージョン 57 から
    Boring(BoringRequest),
    /// 開いた IFC に、変更の一覧を当てて書き戻す（決定 0207・0244）。要素は、building_elements が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイル（ifcZIP で開いたものは ifcZIP）を blobs の file で返す。名前を挙げないインスタンスは、元のバイト列のまま書く。マイナーバージョン 58 から
    BuildingApply(BuildingApplyRequest),
    /// 開いた図面の文書を図面モデル（L2）に変換し、図面モデルのバイナリ（決定 0247）を blob で返す。配置は schema/drawing.toml にあり、元の文書の形式・系統・ハッシュも持つ。プラグインのモジュールが読んだ図面は、これを図面のモジュールの open に渡すと、元の形式とハッシュの読むだけの文書になる。マイナーバージョン 60 から
    Lift(LiftRequest),
    /// 開いた文書を scene と同じ起点・配置・フォントで 2D のシーンにし、画像を blob で返す（決定 0272・0275・0277）。字形の輪郭だけを描き、字形のない空でない文字・外部画像・名前付きの点記号は元の ID と unit を診断して描かない。シーン生成と描画で同じ予算と中断の合図を使い、失敗では画像を返さない。PNG は最大 16777216 画素で、変換後の座標は絶対値 2^31 ピクセル以内。SVG はマイナーバージョン 64、PNG は 65 から
    Render(RenderRequest),
    /// 開いた TIFF・GeoTIFF の指定ページの画素を RGBA8・非インターレースの PNG に復号し、画像の blob で返す（#877）。複数ページの TIFF は page で 1 から数えるページ番号を選ぶ（書かなければ 1）。max_side を指定すると、長辺が max_side 以下になるよう縦横比を保って縮小して返す（最大 16777216 画素）。マイナーバージョン 73 から
    Raster(RasterRequest),
}

impl Request {
    /// タグのフィールド（`op`）の値（スキーマの順）
    pub const TAGS: &[&str] = &[
        "capabilities",
        "open",
        "close",
        "describe",
        "drawing",
        "entities",
        "calibrations",
        "measure_distance",
        "measure",
        "intersect",
        "regions",
        "construct",
        "apply",
        "create",
        "convert",
        "convert_estimate",
        "tables",
        "scene",
        "scene3d",
        "check",
        "validate",
        "review_carry",
        "review_adopt",
        "review_scene",
        "building",
        "building_elements",
        "ids_check",
        "review_adopt_ids",
        "bcf_read",
        "bcf_write",
        "civil",
        "pointcloud",
        "deviation",
        "civil_drawing",
        "geo",
        "delivery_check",
        "delivery_apply",
        "photo",
        "boring",
        "building_apply",
        "lift",
        "render",
        "raster",
    ];

    /// タグのフィールド（`op`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Capabilities(_) => "capabilities",
            Self::Open(_) => "open",
            Self::Close(_) => "close",
            Self::Describe(_) => "describe",
            Self::Drawing(_) => "drawing",
            Self::Entities(_) => "entities",
            Self::Calibrations(_) => "calibrations",
            Self::MeasureDistance(_) => "measure_distance",
            Self::Measure(_) => "measure",
            Self::Intersect(_) => "intersect",
            Self::Regions(_) => "regions",
            Self::Construct(_) => "construct",
            Self::Apply(_) => "apply",
            Self::Create(_) => "create",
            Self::Convert(_) => "convert",
            Self::ConvertEstimate(_) => "convert_estimate",
            Self::Tables(_) => "tables",
            Self::Scene(_) => "scene",
            Self::Scene3d(_) => "scene3d",
            Self::Check(_) => "check",
            Self::Validate(_) => "validate",
            Self::ReviewCarry(_) => "review_carry",
            Self::ReviewAdopt(_) => "review_adopt",
            Self::ReviewScene(_) => "review_scene",
            Self::Building(_) => "building",
            Self::BuildingElements(_) => "building_elements",
            Self::IdsCheck(_) => "ids_check",
            Self::ReviewAdoptIds(_) => "review_adopt_ids",
            Self::BcfRead(_) => "bcf_read",
            Self::BcfWrite(_) => "bcf_write",
            Self::Civil(_) => "civil",
            Self::Pointcloud(_) => "pointcloud",
            Self::Deviation(_) => "deviation",
            Self::CivilDrawing(_) => "civil_drawing",
            Self::Geo(_) => "geo",
            Self::DeliveryCheck(_) => "delivery_check",
            Self::DeliveryApply(_) => "delivery_apply",
            Self::Photo(_) => "photo",
            Self::Boring(_) => "boring",
            Self::BuildingApply(_) => "building_apply",
            Self::Lift(_) => "lift",
            Self::Render(_) => "render",
            Self::Raster(_) => "raster",
        }
    }
}

impl FromJson for Request {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("op")? {
            "capabilities" => Self::Capabilities(CapabilitiesRequest::read_fields(&mut object)?),
            "open" => Self::Open(OpenRequest::read_fields(&mut object)?),
            "close" => Self::Close(CloseRequest::read_fields(&mut object)?),
            "describe" => Self::Describe(DescribeRequest::read_fields(&mut object)?),
            "drawing" => Self::Drawing(DrawingRequest::read_fields(&mut object)?),
            "entities" => Self::Entities(EntitiesRequest::read_fields(&mut object)?),
            "calibrations" => Self::Calibrations(CalibrationsRequest::read_fields(&mut object)?),
            "measure_distance" => {
                Self::MeasureDistance(MeasureDistanceRequest::read_fields(&mut object)?)
            }
            "measure" => Self::Measure(MeasureRequest::read_fields(&mut object)?),
            "intersect" => Self::Intersect(IntersectRequest::read_fields(&mut object)?),
            "regions" => Self::Regions(RegionsRequest::read_fields(&mut object)?),
            "construct" => Self::Construct(ConstructRequest::read_fields(&mut object)?),
            "apply" => Self::Apply(ApplyRequest::read_fields(&mut object)?),
            "create" => Self::Create(CreateRequest::read_fields(&mut object)?),
            "convert" => Self::Convert(ConvertRequest::read_fields(&mut object)?),
            "convert_estimate" => {
                Self::ConvertEstimate(ConvertEstimateRequest::read_fields(&mut object)?)
            }
            "tables" => Self::Tables(TablesRequest::read_fields(&mut object)?),
            "scene" => Self::Scene(SceneRequest::read_fields(&mut object)?),
            "scene3d" => Self::Scene3d(Scene3dRequest::read_fields(&mut object)?),
            "check" => Self::Check(CheckRequest::read_fields(&mut object)?),
            "validate" => Self::Validate(ValidateRequest::read_fields(&mut object)?),
            "review_carry" => Self::ReviewCarry(ReviewCarryRequest::read_fields(&mut object)?),
            "review_adopt" => Self::ReviewAdopt(ReviewAdoptRequest::read_fields(&mut object)?),
            "review_scene" => Self::ReviewScene(ReviewSceneRequest::read_fields(&mut object)?),
            "building" => Self::Building(BuildingRequest::read_fields(&mut object)?),
            "building_elements" => {
                Self::BuildingElements(BuildingElementsRequest::read_fields(&mut object)?)
            }
            "ids_check" => Self::IdsCheck(IdsCheckRequest::read_fields(&mut object)?),
            "review_adopt_ids" => {
                Self::ReviewAdoptIds(ReviewAdoptIdsRequest::read_fields(&mut object)?)
            }
            "bcf_read" => Self::BcfRead(BcfReadRequest::read_fields(&mut object)?),
            "bcf_write" => Self::BcfWrite(BcfWriteRequest::read_fields(&mut object)?),
            "civil" => Self::Civil(CivilRequest::read_fields(&mut object)?),
            "pointcloud" => Self::Pointcloud(PointcloudRequest::read_fields(&mut object)?),
            "deviation" => Self::Deviation(DeviationRequest::read_fields(&mut object)?),
            "civil_drawing" => Self::CivilDrawing(CivilDrawingRequest::read_fields(&mut object)?),
            "geo" => Self::Geo(GeoRequest::read_fields(&mut object)?),
            "delivery_check" => {
                Self::DeliveryCheck(DeliveryCheckRequest::read_fields(&mut object)?)
            }
            "delivery_apply" => {
                Self::DeliveryApply(DeliveryApplyRequest::read_fields(&mut object)?)
            }
            "photo" => Self::Photo(PhotoRequest::read_fields(&mut object)?),
            "boring" => Self::Boring(BoringRequest::read_fields(&mut object)?),
            "building_apply" => {
                Self::BuildingApply(BuildingApplyRequest::read_fields(&mut object)?)
            }
            "lift" => Self::Lift(LiftRequest::read_fields(&mut object)?),
            "render" => Self::Render(RenderRequest::read_fields(&mut object)?),
            "raster" => Self::Raster(RasterRequest::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("op")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Request {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Capabilities(value) => value.to_json(out),
            Self::Open(value) => value.to_json(out),
            Self::Close(value) => value.to_json(out),
            Self::Describe(value) => value.to_json(out),
            Self::Drawing(value) => value.to_json(out),
            Self::Entities(value) => value.to_json(out),
            Self::Calibrations(value) => value.to_json(out),
            Self::MeasureDistance(value) => value.to_json(out),
            Self::Measure(value) => value.to_json(out),
            Self::Intersect(value) => value.to_json(out),
            Self::Regions(value) => value.to_json(out),
            Self::Construct(value) => value.to_json(out),
            Self::Apply(value) => value.to_json(out),
            Self::Create(value) => value.to_json(out),
            Self::Convert(value) => value.to_json(out),
            Self::ConvertEstimate(value) => value.to_json(out),
            Self::Tables(value) => value.to_json(out),
            Self::Scene(value) => value.to_json(out),
            Self::Scene3d(value) => value.to_json(out),
            Self::Check(value) => value.to_json(out),
            Self::Validate(value) => value.to_json(out),
            Self::ReviewCarry(value) => value.to_json(out),
            Self::ReviewAdopt(value) => value.to_json(out),
            Self::ReviewScene(value) => value.to_json(out),
            Self::Building(value) => value.to_json(out),
            Self::BuildingElements(value) => value.to_json(out),
            Self::IdsCheck(value) => value.to_json(out),
            Self::ReviewAdoptIds(value) => value.to_json(out),
            Self::BcfRead(value) => value.to_json(out),
            Self::BcfWrite(value) => value.to_json(out),
            Self::Civil(value) => value.to_json(out),
            Self::Pointcloud(value) => value.to_json(out),
            Self::Deviation(value) => value.to_json(out),
            Self::CivilDrawing(value) => value.to_json(out),
            Self::Geo(value) => value.to_json(out),
            Self::DeliveryCheck(value) => value.to_json(out),
            Self::DeliveryApply(value) => value.to_json(out),
            Self::Photo(value) => value.to_json(out),
            Self::Boring(value) => value.to_json(out),
            Self::BuildingApply(value) => value.to_json(out),
            Self::Lift(value) => value.to_json(out),
            Self::Render(value) => value.to_json(out),
            Self::Raster(value) => value.to_json(out),
        }
    }
}

/// どの op の要求にも置けるフィールド
#[derive(Clone, Debug, PartialEq)]
pub struct RequestCommon {
    /// メッセージのスキーマのメジャーバージョン。今は 1
    pub v: u64,
    /// 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010）
    pub budget: Option<Budget>,
    /// 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010）
    pub partial: Option<bool>,
    /// 診断ごとに返す場所の数。書かなければ 16（決定 0009）
    pub max_places: Option<u64>,
    /// 時間の上限（ミリ秒）。ネイティブの API が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010）
    pub time_limit_ms: Option<u64>,
}

impl RequestCommon {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let v = object.required("v", |value| json::uint(value, 1, 9007199254740991))?;
        let budget = object.optional("budget", Budget::from_json)?;
        let partial = object.optional("partial", json::boolean)?;
        let max_places =
            object.optional("max_places", |value| json::uint(value, 0, 9007199254740991))?;
        let time_limit_ms = object.optional("time_limit_ms", |value| {
            json::uint(value, 1, 9007199254740991)
        })?;
        Ok(Self {
            v,
            budget,
            partial,
            max_places,
            time_limit_ms,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("v");
        out.uint(self.v);
        if let Some(value) = &self.budget {
            out.key("budget");
            value.to_json(out);
        }
        if let Some(value) = &self.partial {
            out.key("partial");
            out.bool(*value);
        }
        if let Some(value) = &self.max_places {
            out.key("max_places");
            out.uint(*value);
        }
        if let Some(value) = &self.time_limit_ms {
            out.key("time_limit_ms");
            out.uint(*value);
        }
    }
}

/// 予算の単位ごとの上限（決定 0010）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Budget {
    /// 受け取ったバイト数
    pub input_bytes: Option<Limit>,
    /// 展開・復号で作ったバイト数。入れ子の分も足す
    pub decoded_bytes: Option<Limit>,
    /// L0・L1 で読んだレコードの数
    pub records: Option<Limit>,
    /// L2・L3 で作った要素の数
    pub elements: Option<Limit>,
    /// 入れ子と再帰の深さ
    pub depth: Option<Limit>,
    /// 幾何の分割で作った点・線・面の数
    pub segments: Option<Limit>,
    /// ほかの単位に入らない繰り返しの回数
    pub work: Option<Limit>,
}

impl Budget {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let input_bytes = object.optional("input_bytes", Limit::from_json)?;
        let decoded_bytes = object.optional("decoded_bytes", Limit::from_json)?;
        let records = object.optional("records", Limit::from_json)?;
        let elements = object.optional("elements", Limit::from_json)?;
        let depth = object.optional("depth", Limit::from_json)?;
        let segments = object.optional("segments", Limit::from_json)?;
        let work = object.optional("work", Limit::from_json)?;
        Ok(Self {
            input_bytes,
            decoded_bytes,
            records,
            elements,
            depth,
            segments,
            work,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.input_bytes {
            out.key("input_bytes");
            value.to_json(out);
        }
        if let Some(value) = &self.decoded_bytes {
            out.key("decoded_bytes");
            value.to_json(out);
        }
        if let Some(value) = &self.records {
            out.key("records");
            value.to_json(out);
        }
        if let Some(value) = &self.elements {
            out.key("elements");
            value.to_json(out);
        }
        if let Some(value) = &self.depth {
            out.key("depth");
            value.to_json(out);
        }
        if let Some(value) = &self.segments {
            out.key("segments");
            value.to_json(out);
        }
        if let Some(value) = &self.work {
            out.key("work");
            value.to_json(out);
        }
    }
}

impl FromJson for Budget {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Budget {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 1 つの単位の上限。上限をなくすときは unlimited と書く
#[derive(Clone, Debug, PartialEq)]
pub enum Limit {
    /// この量までは使える
    Integer(u64),
    /// 上限なし
    Unlimited,
}

impl FromJson for Limit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        match value {
            Value::Number(_) => json::uint(value, 0, 9007199254740991).map(Self::Integer),
            Value::String(text) => match text.as_str() {
                "unlimited" => Ok(Self::Unlimited),
                _ => Err(Invalid::new(Problem::Value)),
            },
            _ => Err(Invalid::new(Problem::Type)),
        }
    }
}

impl ToJson for Limit {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Integer(value) => out.uint(*value),
            Self::Unlimited => out.string("unlimited"),
        }
    }
}

/// コアのバージョンと、使える op・形式を尋ねる
#[derive(Clone, Debug, PartialEq)]
pub struct CapabilitiesRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
}

impl CapabilitiesRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "capabilities")?;
        Ok(Self { request_common })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("capabilities");
    }
}

impl FromJson for CapabilitiesRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CapabilitiesRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// コアのバージョンと、使える op・形式（決定 0015）
#[derive(Clone, Debug, PartialEq)]
pub struct CapabilitiesResult {
    /// パッケージのバージョン（semver）
    pub package: String,
    /// ABI のメジャーバージョン。zm_abi_version と同じ
    pub abi: u64,
    /// メッセージのスキーマのバージョン
    pub schema: SchemaVersion,
    /// このコア（wasm ではこのモジュール）で使える op の名前
    pub ops: Vec<String>,
    /// 読み書きできる形式
    pub formats: Vec<Format>,
}

impl CapabilitiesResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let package = object.required("package", json::string)?;
        let abi = object.required("abi", |value| json::uint(value, 1, 9007199254740991))?;
        let schema = object.required("schema", SchemaVersion::from_json)?;
        let ops = object.required("ops", |value| json::array(value, json::string))?;
        let formats = object.required("formats", |value| json::array(value, Format::from_json))?;
        Ok(Self {
            package,
            abi,
            schema,
            ops,
            formats,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("package");
        out.string(&self.package);
        out.key("abi");
        out.uint(self.abi);
        out.key("schema");
        self.schema.to_json(out);
        out.key("ops");
        out.array(&self.ops, |out, item| out.string(item));
        out.key("formats");
        out.array(&self.formats, |out, item| item.to_json(out));
    }
}

impl FromJson for CapabilitiesResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CapabilitiesResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// メッセージのスキーマのバージョン。メジャーバージョンは要求と応答の v、マイナーバージョンは op・フィールド・列挙の値を足すたびに上げる
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaVersion {
    /// メジャーバージョン
    pub major: u64,
    /// マイナーバージョン
    pub minor: u64,
}

impl SchemaVersion {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let major = object.required("major", |value| json::uint(value, 1, 9007199254740991))?;
        let minor = object.required("minor", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { major, minor })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("major");
        out.uint(self.major);
        out.key("minor");
        out.uint(self.minor);
    }
}

impl FromJson for SchemaVersion {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SchemaVersion {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 読み書きできる形式
#[derive(Clone, Debug, PartialEq)]
pub struct Format {
    /// 形式の名前（dxf・sfc など）
    pub id: String,
    /// 形式の系統（autocad・sxf など、決定 0008）
    pub family: String,
    /// 読めるか
    pub read: bool,
    /// 書けるか
    pub write: bool,
    /// create で空の文書を作れるか。マイナーバージョン 83 から
    pub create: bool,
    /// この形式の ID の規則のバージョン（決定 0015）
    pub id_rules: u64,
}

impl Format {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let family = object.required("family", json::string)?;
        let read = object.required("read", json::boolean)?;
        let write = object.required("write", json::boolean)?;
        let create = object.required("create", json::boolean)?;
        let id_rules =
            object.required("id_rules", |value| json::uint(value, 1, 9007199254740991))?;
        Ok(Self {
            id,
            family,
            read,
            write,
            create,
            id_rules,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("family");
        out.string(&self.family);
        out.key("read");
        out.bool(self.read);
        out.key("write");
        out.bool(self.write);
        out.key("create");
        out.bool(self.create);
        out.key("id_rules");
        out.uint(self.id_rules);
    }
}

impl FromJson for Format {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Format {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 入力を読んで、文書を作る。形式は入力から判定する
#[derive(Clone, Debug, PartialEq)]
pub struct OpenRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 読む入力の名前。zm_push で渡したもの
    pub input: String,
    /// 作る文書の名前。書かなければ入力の名前
    pub doc: Option<String>,
    /// 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。マイナーバージョン 48 から
    pub size: Option<u64>,
    /// 見分けるマーカーのない形式（xyz・extdm・geojson）を名指して読む。書かなければ、入力から判定する（決定 0232）。マイナーバージョン 49 から。extdm はマイナーバージョン 62 から。geojson はマイナーバージョン 71 から
    pub format: Option<String>,
    /// SXF の属性ファイル（SAF）を入れた入力の名前。zm_push で渡したもの。属性ファイル用の属性付加機構（$$ATRF$$）の要素に、属性を sxf.saf の拡張で結ぶ（決定 0249）。書かなければ、P2Z・SFZ の中の、図面と同じ名前の SAF を使う。SXF でない入力には使えない。マイナーバージョン 63 から
    pub saf: Option<String>,
    /// IFC-SPF・ifcZIP の 8 ビットのエスケープを読む文字コード。省略時は auto。文書の building_apply 後も指定を保つ。IFC 以外の入力には使えない。マイナーバージョン 76 から
    pub ifc_text_encoding: Option<IfcTextEncoding>,
}

impl OpenRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "open")?;
        let input = object.required("input", json::string)?;
        let doc = object.optional("doc", json::string)?;
        let size = object.optional("size", |value| json::uint(value, 0, 9007199254740991))?;
        let format = object.optional("format", json::string)?;
        let saf = object.optional("saf", json::string)?;
        let ifc_text_encoding = object.optional("ifc_text_encoding", IfcTextEncoding::from_json)?;
        Ok(Self {
            request_common,
            input,
            doc,
            size,
            format,
            saf,
            ifc_text_encoding,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("open");
        out.key("input");
        out.string(&self.input);
        if let Some(value) = &self.doc {
            out.key("doc");
            out.string(value);
        }
        if let Some(value) = &self.size {
            out.key("size");
            out.uint(*value);
        }
        if let Some(value) = &self.format {
            out.key("format");
            out.string(value);
        }
        if let Some(value) = &self.saf {
            out.key("saf");
            out.string(value);
        }
        if let Some(value) = &self.ifc_text_encoding {
            out.key("ifc_text_encoding");
            value.to_json(out);
        }
    }
}

impl FromJson for OpenRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for OpenRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// IFC の文字列の読み方。auto はファイル内に CP932 の手がかりがあれば読み替える。iso_8859_1 は従来の ISO 8859-1 と Unicode、shift_jis は手がかりなしでも CP932 として読める文字列を読み替える。どの指定も元のバイト列を保つ
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IfcTextEncoding {
    /// `auto`
    Auto,
    /// `iso_8859_1`
    Iso88591,
    /// `shift_jis`
    ShiftJis,
}

impl IfcTextEncoding {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Auto, Self::Iso88591, Self::ShiftJis];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Iso88591 => "iso_8859_1",
            Self::ShiftJis => "shift_jis",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "auto" => Some(Self::Auto),
            "iso_8859_1" => Some(Self::Iso88591),
            "shift_jis" => Some(Self::ShiftJis),
            _ => None,
        }
    }
}

impl FromJson for IfcTextEncoding {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for IfcTextEncoding {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 作った文書
#[derive(Clone, Debug, PartialEq)]
pub struct OpenResult {
    /// 文書の名前
    pub doc: String,
    /// 判定した形式の名前
    pub format: String,
    /// 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。マイナーバージョン 48 から
    pub needs: Option<Vec<InputRange>>,
    /// 読み終えたので、close の inputs で捨ててよい範囲の入力。マイナーバージョン 48 から
    pub done: Option<Vec<InputRange>>,
    /// SXF を ZIP に入れたアーカイブ（P2Z・SFZ）なら、SXF の項目のほかの項目（属性ファイル・画像など）のアーカイブの中の名前。アーカイブの順。apply は、SXF の項目だけを置き換え、ほかの項目を写したアーカイブを書く（#385）。マイナーバージョン 54 から
    pub entries: Option<Vec<String>>,
}

impl OpenResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let needs = object.optional("needs", |value| json::array(value, InputRange::from_json))?;
        let done = object.optional("done", |value| json::array(value, InputRange::from_json))?;
        let entries = object.optional("entries", |value| json::array(value, json::string))?;
        Ok(Self {
            doc,
            format,
            needs,
            done,
            entries,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        if let Some(value) = &self.needs {
            out.key("needs");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.done {
            out.key("done");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.entries {
            out.key("entries");
            out.array(value, |out, item| out.string(item));
        }
    }
}

impl FromJson for OpenResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for OpenResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 入力の範囲と、その範囲を渡す入力の名前
#[derive(Clone, Debug, PartialEq)]
pub struct InputRange {
    /// 範囲を渡す入力の名前。<入力の名前>#<start>-<end>
    pub input: String,
    /// 範囲の初め（バイト）
    pub start: u64,
    /// 範囲の終わり（バイト、含まない）
    pub end: u64,
}

impl InputRange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let input = object.required("input", json::string)?;
        let start = object.required("start", |value| json::uint(value, 0, 9007199254740991))?;
        let end = object.required("end", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { input, start, end })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("input");
        out.string(&self.input);
        out.key("start");
        out.uint(self.start);
        out.key("end");
        out.uint(self.end);
    }
}

impl FromJson for InputRange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for InputRange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない
#[derive(Clone, Debug, PartialEq)]
pub struct CloseRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 捨てる文書の名前
    pub docs: Option<Vec<String>>,
    /// 捨てる入力の名前
    pub inputs: Option<Vec<String>>,
}

impl CloseRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "close")?;
        let docs = object.optional("docs", |value| json::array(value, json::string))?;
        let inputs = object.optional("inputs", |value| json::array(value, json::string))?;
        Ok(Self {
            request_common,
            docs,
            inputs,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("close");
        if let Some(value) = &self.docs {
            out.key("docs");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.inputs {
            out.key("inputs");
            out.array(value, |out, item| out.string(item));
        }
    }
}

impl FromJson for CloseRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CloseRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 捨てたこと。中身はない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CloseResult {}

impl CloseResult {
    fn read_fields(_object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        Ok(Self {})
    }

    fn write_fields(&self, _out: &mut Writer) {}
}

impl FromJson for CloseResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CloseResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断の文を、カタログから組み立てる（決定 0009）
#[derive(Clone, Debug, PartialEq)]
pub struct DescribeRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// カタログの入力の名前。zm_push で渡したもの。書かなければ、code と値をそのまま文にする
    pub catalog: Option<String>,
    /// 文にする診断。応答で受け取ったもの
    pub diagnostics: Vec<Diagnostic>,
}

impl DescribeRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "describe")?;
        let catalog = object.optional("catalog", json::string)?;
        let diagnostics = object.required("diagnostics", |value| {
            json::array(value, Diagnostic::from_json)
        })?;
        Ok(Self {
            request_common,
            catalog,
            diagnostics,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("describe");
        if let Some(value) = &self.catalog {
            out.key("catalog");
            out.string(value);
        }
        out.key("diagnostics");
        out.array(&self.diagnostics, |out, item| item.to_json(out));
    }
}

impl FromJson for DescribeRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DescribeRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断の文
#[derive(Clone, Debug, PartialEq)]
pub struct DescribeResult {
    /// 診断ごとの文。要求の diagnostics と同じ順。カタログにない code は、code と値をそのまま書く
    pub texts: Vec<String>,
}

impl DescribeResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let texts = object.required("texts", |value| json::array(value, json::string))?;
        Ok(Self { texts })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("texts");
        out.array(&self.texts, |out, item| out.string(item));
    }
}

impl FromJson for DescribeResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DescribeResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合テストで比べるためにも使う（決定 0007）
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
}

impl DrawingRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "drawing")?;
        let doc = object.required("doc", json::string)?;
        Ok(Self {
            request_common,
            doc,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("drawing");
        out.key("doc");
        out.string(&self.doc);
    }
}

impl FromJson for DrawingRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DrawingRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 図面モデルの要約。移さなかった図形と値は、診断で返す
#[derive(Clone, Debug, PartialEq)]
pub struct DrawingResult {
    /// 文書のハッシュ。open に渡したバイト列から作る。変更の一覧の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
    pub hash: Option<String>,
    /// 実寸の空間。モデルの順
    pub spaces: Vec<SpaceSummary>,
    /// 用紙。モデルの順
    pub sheets: Vec<SheetSummary>,
    /// 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。マイナーバージョン 39 から
    pub point_style: Option<PointStyleSummary>,
}

impl DrawingResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let hash = object.optional("hash", json::string)?;
        let spaces = object.required("spaces", |value| {
            json::array(value, SpaceSummary::from_json)
        })?;
        let sheets = object.required("sheets", |value| {
            json::array(value, SheetSummary::from_json)
        })?;
        let point_style = object.optional("point_style", PointStyleSummary::from_json)?;
        Ok(Self {
            hash,
            spaces,
            sheets,
            point_style,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
        out.key("spaces");
        out.array(&self.spaces, |out, item| item.to_json(out));
        out.key("sheets");
        out.array(&self.sheets, |out, item| item.to_json(out));
        if let Some(value) = &self.point_style {
            out.key("point_style");
            value.to_json(out);
        }
    }
}

impl FromJson for DrawingResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DrawingResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 2 次元の座標
#[derive(Clone, Debug, PartialEq)]
pub struct Point2 {
    /// X 座標
    pub x: f64,
    /// Y 座標
    pub y: f64,
}

impl Point2 {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let x = object.required("x", json::number)?;
        let y = object.required("y", json::number)?;
        Ok(Self { x, y })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("x");
        out.float(self.x);
        out.key("y");
        out.float(self.y);
    }
}

impl FromJson for Point2 {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Point2 {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 原点と相対座標を分けた用紙の点。両成分の正確な和を位置とし、先に足して丸めない。座標の単位は用紙の mm
#[derive(Clone, Debug, PartialEq)]
pub struct SplitPoint2 {
    /// 用紙の原点
    pub origin: Point2,
    /// 原点からの相対座標
    pub offset: Point2,
}

impl SplitPoint2 {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let origin = object.required("origin", Point2::from_json)?;
        let offset = object.required("offset", Point2::from_json)?;
        Ok(Self { origin, offset })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("origin");
        self.origin.to_json(out);
        out.key("offset");
        self.offset.to_json(out);
    }
}

impl FromJson for SplitPoint2 {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SplitPoint2 {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 用紙の閉じた矩形で使う距離の校正。重なる領域は ID で明示的に選ぶ。マイナーバージョン 68 から
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationItem {
    /// 文書内で重複しない校正の ID
    pub id: String,
    /// 元の校正の名前。翻訳しない。なければ持たない
    pub name: Option<String>,
    /// 校正を使える公開用紙の ID。drawing が返すもの
    pub from: String,
    /// 用紙の mm の閉じた矩形。[X の最小, Y の最小, X の最大, Y の最大] の 4 値で、幅と高さは正
    pub bounds: Vec<f64>,
    /// 用紙の mm の変位から校正単位の変位へ写す行優先の 2×2 行列。[a, b, c, d] の 4 値で、[dx, dy] を [a*dx+b*dy, c*dx+d*dy] に写す
    pub matrix: Vec<f64>,
    /// 校正後の長さの単位。ラベルだけから推定しない
    pub unit: Unit,
    /// 元の単位ラベル。未知の単位でも原文を保つ
    pub unit_label: String,
    /// 元の形式のレコード
    pub source: Option<Source>,
}

impl CalibrationItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.optional("name", json::string)?;
        let from = object.required("from", json::string)?;
        let bounds = object.required("bounds", |value| json::array(value, json::number))?;
        let matrix = object.required("matrix", |value| json::array(value, json::number))?;
        let unit = object.required("unit", Unit::from_json)?;
        let unit_label = object.required("unit_label", json::string)?;
        let source = object.optional("source", Source::from_json)?;
        Ok(Self {
            id,
            name,
            from,
            bounds,
            matrix,
            unit,
            unit_label,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        out.key("from");
        out.string(&self.from);
        out.key("bounds");
        out.array(&self.bounds, |out, item| out.float(*item));
        out.key("matrix");
        out.array(&self.matrix, |out, item| out.float(*item));
        out.key("unit");
        self.unit.to_json(out);
        out.key("unit_label");
        out.string(&self.unit_label);
        if let Some(value) = &self.source {
            out.key("source");
            value.to_json(out);
        }
    }
}

impl FromJson for CalibrationItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CalibrationItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を図面モデルに変換し、用紙の距離の校正をモデルの順に返す。マイナーバージョン 68 から
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationsRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// 公開用紙の ID。drawing が返すもの。書けばその用紙の校正だけを返し、書かなければすべての校正を返す
    pub from: Option<String>,
    /// 返す最初の校正の位置。from で絞った後の位置で、前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す校正の数の上限。書かなければ 1000
    pub limit: Option<u64>,
    /// 期待する元の文書のハッシュ。open に渡したバイト列のハッシュと照合する。図面モデルへ変換した ZMDR 自身のハッシュではない
    pub expected_hash: Option<String>,
}

impl CalibrationsRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "calibrations")?;
        let doc = object.required("doc", json::string)?;
        let from = object.optional("from", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        let expected_hash = object.optional("expected_hash", json::string)?;
        Ok(Self {
            request_common,
            doc,
            from,
            offset,
            limit,
            expected_hash,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("calibrations");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.from {
            out.key("from");
            out.string(value);
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
        if let Some(value) = &self.expected_hash {
            out.key("expected_hash");
            out.string(value);
        }
    }
}

impl FromJson for CalibrationsRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CalibrationsRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 距離の校正の一覧の一部
#[derive(Clone, Debug, PartialEq)]
pub struct CalibrationsResult {
    /// 元の文書のハッシュ。ZMDR から開いたときは、埋め込まれた元の文書のハッシュ（Source.hash）
    pub hash: String,
    /// from で絞った一覧の offset から limit 個までの校正
    pub items: Vec<CalibrationItem>,
    /// from で絞った後の校正の総数
    pub total: u64,
    /// 続きの最初の校正の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl CalibrationsResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let hash = object.required("hash", json::string)?;
        let items = object.required("items", |value| {
            json::array(value, CalibrationItem::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            hash,
            items,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("hash");
        out.string(&self.hash);
        out.key("items");
        out.array(&self.items, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for CalibrationsResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CalibrationsResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 用紙と校正を明示的に選び、始点と終点の校正後の距離を求める。両点は校正の閉領域内にある必要がある。マイナーバージョン 68 から
#[derive(Clone, Debug, PartialEq)]
pub struct MeasureDistanceRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// 公開用紙の ID。drawing が返すもので、選んだ校正と同じ用紙
    pub from: String,
    /// 使う校正の ID。calibrations が返すもの
    pub calibration: String,
    /// 始点。原点と相対座標を分けた用紙の mm の点
    pub start: SplitPoint2,
    /// 終点。原点と相対座標を分けた用紙の mm の点
    pub end: SplitPoint2,
    /// 期待する元の文書のハッシュ。open に渡したバイト列のハッシュと照合する。図面モデルへ変換した ZMDR 自身のハッシュではない
    pub expected_hash: Option<String>,
}

impl MeasureDistanceRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "measure_distance")?;
        let doc = object.required("doc", json::string)?;
        let from = object.required("from", json::string)?;
        let calibration = object.required("calibration", json::string)?;
        let start = object.required("start", SplitPoint2::from_json)?;
        let end = object.required("end", SplitPoint2::from_json)?;
        let expected_hash = object.optional("expected_hash", json::string)?;
        Ok(Self {
            request_common,
            doc,
            from,
            calibration,
            start,
            end,
            expected_hash,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("measure_distance");
        out.key("doc");
        out.string(&self.doc);
        out.key("from");
        out.string(&self.from);
        out.key("calibration");
        out.string(&self.calibration);
        out.key("start");
        self.start.to_json(out);
        out.key("end");
        self.end.to_json(out);
        if let Some(value) = &self.expected_hash {
            out.key("expected_hash");
            out.string(value);
        }
    }
}

impl FromJson for MeasureDistanceRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MeasureDistanceRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 選んだ用紙と校正による距離
#[derive(Clone, Debug, PartialEq)]
pub struct MeasureDistanceResult {
    /// 元の文書のハッシュ。ZMDR から開いたときは、埋め込まれた元の文書のハッシュ（Source.hash）
    pub hash: String,
    /// 計測した公開用紙の ID
    pub from: String,
    /// 使った校正の ID
    pub calibration: String,
    /// 校正後の距離。unit と unit_label が示す単位
    pub distance: f64,
    /// 校正後の長さの単位
    pub unit: Unit,
    /// 元の単位ラベル。未知の単位でも原文を保つ
    pub unit_label: String,
}

impl MeasureDistanceResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let hash = object.required("hash", json::string)?;
        let from = object.required("from", json::string)?;
        let calibration = object.required("calibration", json::string)?;
        let distance = object.required("distance", json::number)?;
        let unit = object.required("unit", Unit::from_json)?;
        let unit_label = object.required("unit_label", json::string)?;
        Ok(Self {
            hash,
            from,
            calibration,
            distance,
            unit,
            unit_label,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("hash");
        out.string(&self.hash);
        out.key("from");
        out.string(&self.from);
        out.key("calibration");
        out.string(&self.calibration);
        out.key("distance");
        out.float(self.distance);
        out.key("unit");
        self.unit.to_json(out);
        out.key("unit_label");
        out.string(&self.unit_label);
    }
}

impl FromJson for MeasureDistanceResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MeasureDistanceResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// モデル空間の要素の長さ・周長・面積、2 要素または点の間の最短距離を測る。線幅と表示用の変換は含めない
#[derive(Clone, Debug, PartialEq)]
pub struct MeasureRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 開いた文書の名前
    pub doc: String,
    /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
    pub from: String,
    /// 求める値
    pub quantity: MeasureQuantity,
    /// 計測する要素または点
    pub first: MeasureTarget,
    /// 最短距離のもう一方の要素または点。distance のときだけ必須
    pub second: Option<MeasureTarget>,
    /// 近似する曲線の分割で目標にする弦の誤差。空間の長さの単位で正の値。既定は 0.001。計測した長さ・面積・距離の誤差上限ではない
    pub tolerance: Option<f64>,
    /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
    pub expected_hash: Option<String>,
}

impl MeasureRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "measure")?;
        let doc = object.required("doc", json::string)?;
        let from = object.required("from", json::string)?;
        let quantity = object.required("quantity", MeasureQuantity::from_json)?;
        let first = object.required("first", MeasureTarget::from_json)?;
        let second = object.optional("second", MeasureTarget::from_json)?;
        let tolerance = object.optional("tolerance", json::number)?;
        let expected_hash = object.optional("expected_hash", json::string)?;
        Ok(Self {
            request_common,
            doc,
            from,
            quantity,
            first,
            second,
            tolerance,
            expected_hash,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("measure");
        out.key("doc");
        out.string(&self.doc);
        out.key("from");
        out.string(&self.from);
        out.key("quantity");
        self.quantity.to_json(out);
        out.key("first");
        self.first.to_json(out);
        if let Some(value) = &self.second {
            out.key("second");
            value.to_json(out);
        }
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.expected_hash {
            out.key("expected_hash");
            out.string(value);
        }
    }
}

impl FromJson for MeasureRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MeasureRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 長さ、閉じた経路の周長、向き付き線積分の絶対値の面積、境界間の最短距離。面積は自己交差を塗り規則で統合しない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeasureQuantity {
    /// `length`
    Length,
    /// `perimeter`
    Perimeter,
    /// `area`
    Area,
    /// `distance`
    Distance,
}

impl MeasureQuantity {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Length, Self::Perimeter, Self::Area, Self::Distance];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Perimeter => "perimeter",
            Self::Area => "area",
            Self::Distance => "distance",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "length" => Some(Self::Length),
            "perimeter" => Some(Self::Perimeter),
            "area" => Some(Self::Area),
            "distance" => Some(Self::Distance),
            _ => None,
        }
    }
}

impl FromJson for MeasureQuantity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for MeasureQuantity {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 同じモデル空間内の要素または座標
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum MeasureTarget {
    /// 計測する要素。元の公開 ID を使う
    Element(MeasureElement),
    /// 距離の計測で使う空間の座標
    Point(MeasurePoint),
}

impl MeasureTarget {
    /// タグのフィールド（`kind`）の値（スキーマの順）
    pub const TAGS: &[&str] = &["element", "point"];

    /// タグのフィールド（`kind`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Element(_) => "element",
            Self::Point(_) => "point",
        }
    }
}

impl FromJson for MeasureTarget {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("kind")? {
            "element" => Self::Element(MeasureElement::read_fields(&mut object)?),
            "point" => Self::Point(MeasurePoint::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("kind")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MeasureTarget {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Element(value) => value.to_json(out),
            Self::Point(value) => value.to_json(out),
        }
    }
}

/// 計測する要素。元の公開 ID を使う
#[derive(Clone, Debug, PartialEq)]
pub struct MeasureElement {
    /// 同じ空間の entities が返した要素の ID
    pub id: String,
}

impl MeasureElement {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "element")?;
        let id = object.required("id", json::string)?;
        Ok(Self { id })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("element");
        out.key("id");
        out.string(&self.id);
    }
}

impl FromJson for MeasureElement {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MeasureElement {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 距離の計測で使う空間の座標
#[derive(Clone, Debug, PartialEq)]
pub struct MeasurePoint {
    /// 空間の長さの単位の点
    pub position: Point2,
}

impl MeasurePoint {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "point")?;
        let position = object.required("position", Point2::from_json)?;
        Ok(Self { position })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("point");
        out.key("position");
        self.position.to_json(out);
    }
}

impl FromJson for MeasurePoint {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MeasurePoint {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素から求めた計測値。近似値かどうか、単位と出所の空間を明示する
#[derive(Clone, Debug, PartialEq)]
pub struct MeasureResult {
    /// 元の文書のハッシュ
    pub hash: String,
    /// 計測した実寸の空間の公開 ID
    pub from: String,
    /// 求めた値の種類
    pub quantity: MeasureQuantity,
    /// 計測値。0 以上の有限値
    pub value: f64,
    /// 空間の長さの単位。不明な単位を補わない
    pub unit: Unit,
    /// 単位の次数。長さと距離は 1、面積は 2
    pub unit_power: u64,
    /// 計測に曲線の折れ線近似を使った。浮動小数の丸めとは区別する
    pub approximate: bool,
    /// 指定した、または既定の分割の目標。近似しないときも返す
    pub tolerance: f64,
    /// 最短距離を作る最初の対象上の点。distance のときだけ返す
    pub nearest_first: Option<Point2>,
    /// 最短距離を作るもう一方の対象上の点。近似した対象では折れ線上の点
    pub nearest_second: Option<Point2>,
}

impl MeasureResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let hash = object.required("hash", json::string)?;
        let from = object.required("from", json::string)?;
        let quantity = object.required("quantity", MeasureQuantity::from_json)?;
        let value = object.required("value", json::number)?;
        let unit = object.required("unit", Unit::from_json)?;
        let unit_power = object.required("unit_power", |value| json::uint(value, 1, 2))?;
        let approximate = object.required("approximate", json::boolean)?;
        let tolerance = object.required("tolerance", json::number)?;
        let nearest_first = object.optional("nearest_first", Point2::from_json)?;
        let nearest_second = object.optional("nearest_second", Point2::from_json)?;
        Ok(Self {
            hash,
            from,
            quantity,
            value,
            unit,
            unit_power,
            approximate,
            tolerance,
            nearest_first,
            nearest_second,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("hash");
        out.string(&self.hash);
        out.key("from");
        out.string(&self.from);
        out.key("quantity");
        self.quantity.to_json(out);
        out.key("value");
        out.float(self.value);
        out.key("unit");
        self.unit.to_json(out);
        out.key("unit_power");
        out.uint(self.unit_power);
        out.key("approximate");
        out.bool(self.approximate);
        out.key("tolerance");
        out.float(self.tolerance);
        if let Some(value) = &self.nearest_first {
            out.key("nearest_first");
            value.to_json(out);
        }
        if let Some(value) = &self.nearest_second {
            out.key("nearest_second");
            value.to_json(out);
        }
    }
}

impl FromJson for MeasureResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MeasureResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 同じ実寸の空間の 2 要素の交点・接点・重なりを求める。文書を変更しない。マイナーバージョン 90 から
#[derive(Clone, Debug, PartialEq)]
pub struct IntersectRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 開いた文書の名前
    pub doc: String,
    /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
    pub from: String,
    /// 交わりを求める要素
    pub first: MeasureElement,
    /// 交わりを求めるもう一方の要素。first と異なる ID
    pub second: MeasureElement,
    /// 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、近似する曲線の分割の目標にも使う
    pub tolerance: Option<f64>,
    /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
    pub expected_hash: Option<String>,
}

impl IntersectRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "intersect")?;
        let doc = object.required("doc", json::string)?;
        let from = object.required("from", json::string)?;
        let first = object.required("first", MeasureElement::from_json)?;
        let second = object.required("second", MeasureElement::from_json)?;
        let tolerance = object.optional("tolerance", json::number)?;
        let expected_hash = object.optional("expected_hash", json::string)?;
        Ok(Self {
            request_common,
            doc,
            from,
            first,
            second,
            tolerance,
            expected_hash,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("intersect");
        out.key("doc");
        out.string(&self.doc);
        out.key("from");
        out.string(&self.from);
        out.key("first");
        self.first.to_json(out);
        out.key("second");
        self.second.to_json(out);
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.expected_hash {
            out.key("expected_hash");
            out.string(value);
        }
    }
}

impl FromJson for IntersectRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IntersectRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 交点または接点。格子点の座標
#[derive(Clone, Debug, PartialEq)]
pub struct IntersectionPoint {
    /// 空間の長さの単位の点
    pub position: Point2,
    /// 接点。同じ格子点にまとめたすべての交わりが接するときだけ真
    pub tangent: bool,
    /// 正確な交点が格子点でなく、近い格子点へ丸めた。まとめたすべての交わりが丸めたときだけ真
    pub rounded: bool,
}

impl IntersectionPoint {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let position = object.required("position", Point2::from_json)?;
        let tangent = object.required("tangent", json::boolean)?;
        let rounded = object.required("rounded", json::boolean)?;
        Ok(Self {
            position,
            tangent,
            rounded,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("position");
        self.position.to_json(out);
        out.key("tangent");
        out.bool(self.tangent);
        out.key("rounded");
        out.bool(self.rounded);
    }
}

impl FromJson for IntersectionPoint {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IntersectionPoint {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 重なりの形。segment は線分、arc は反時計回りの円弧、circle は円全体
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntersectionOverlapKind {
    /// `segment`
    Segment,
    /// `arc`
    Arc,
    /// `circle`
    Circle,
}

impl IntersectionOverlapKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Segment, Self::Arc, Self::Circle];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Segment => "segment",
            Self::Arc => "arc",
            Self::Circle => "circle",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "segment" => Some(Self::Segment),
            "arc" => Some(Self::Arc),
            "circle" => Some(Self::Circle),
            _ => None,
        }
    }
}

impl FromJson for IntersectionOverlapKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for IntersectionOverlapKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 2 要素が長さを持って重なる区間。区間の組ごとに返し、つながる区間をつながない
#[derive(Clone, Debug, PartialEq)]
pub struct IntersectionOverlap {
    /// 重なりの形
    pub kind: IntersectionOverlapKind,
    /// 始点。segment は X、Y の辞書順で小さい端、arc は反時計回りの始点。circle では持たない
    pub start: Option<Point2>,
    /// 終点。circle では持たない
    pub end: Option<Point2>,
    /// 円弧と円の中心。segment では持たない
    pub center: Option<Point2>,
    /// 円弧と円の格子上の半径。segment では持たない
    pub radius: Option<f64>,
}

impl IntersectionOverlap {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", IntersectionOverlapKind::from_json)?;
        let start = object.optional("start", Point2::from_json)?;
        let end = object.optional("end", Point2::from_json)?;
        let center = object.optional("center", Point2::from_json)?;
        let radius = object.optional("radius", json::number)?;
        Ok(Self {
            kind,
            start,
            end,
            center,
            radius,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.start {
            out.key("start");
            value.to_json(out);
        }
        if let Some(value) = &self.end {
            out.key("end");
            value.to_json(out);
        }
        if let Some(value) = &self.center {
            out.key("center");
            value.to_json(out);
        }
        if let Some(value) = &self.radius {
            out.key("radius");
            out.float(*value);
        }
    }
}

impl FromJson for IntersectionOverlap {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IntersectionOverlap {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 2 要素の交わり。first と second を入れ替えても同じ結果を返す
#[derive(Clone, Debug, PartialEq)]
pub struct IntersectResult {
    /// 元の文書のハッシュ
    pub hash: String,
    /// 求めた実寸の空間の公開 ID
    pub from: String,
    /// 空間の長さの単位。不明な単位を補わない
    pub unit: Unit,
    /// 指定した、または既定の許容差
    pub tolerance: f64,
    /// 曲線を折れ線に近似した要素がある
    pub approximate: bool,
    /// 交点と接点。格子点の X、Y の辞書順。重なりの端点と同じ点は含めない
    pub points: Vec<IntersectionPoint>,
    /// 重なり。形・座標の順
    pub overlaps: Vec<IntersectionOverlap>,
}

impl IntersectResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let hash = object.required("hash", json::string)?;
        let from = object.required("from", json::string)?;
        let unit = object.required("unit", Unit::from_json)?;
        let tolerance = object.required("tolerance", json::number)?;
        let approximate = object.required("approximate", json::boolean)?;
        let points = object.required("points", |value| {
            json::array(value, IntersectionPoint::from_json)
        })?;
        let overlaps = object.required("overlaps", |value| {
            json::array(value, IntersectionOverlap::from_json)
        })?;
        Ok(Self {
            hash,
            from,
            unit,
            tolerance,
            approximate,
            points,
            overlaps,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("hash");
        out.string(&self.hash);
        out.key("from");
        out.string(&self.from);
        out.key("unit");
        self.unit.to_json(out);
        out.key("tolerance");
        out.float(self.tolerance);
        out.key("approximate");
        out.bool(self.approximate);
        out.key("points");
        out.array(&self.points, |out, item| item.to_json(out));
        out.key("overlaps");
        out.array(&self.overlaps, |out, item| item.to_json(out));
    }
}

impl FromJson for IntersectResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IntersectResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 同じ実寸の空間の要素の線が囲む閉領域を検出する、または閉じた要素どうしの和・積・差・排他的論理和を求める。文書を変更しない。マイナーバージョン 91 から
#[derive(Clone, Debug, PartialEq)]
pub struct RegionsRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 開いた文書の名前
    pub doc: String,
    /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
    pub from: String,
    /// 求め方
    pub operation: RegionOperation,
    /// detect では線で面を囲む要素、ほかでは集合演算の左側の閉じた要素。1 つ以上で、同じ ID を繰り返さない
    pub subject: Vec<MeasureElement>,
    /// 集合演算の右側の閉じた要素。書かなければ空。detect では書かない。同じ ID を繰り返さない
    pub clip: Option<Vec<MeasureElement>>,
    /// 閉じた要素の内部を決める規則。書かなければ even_odd。detect では書かない
    pub fill: Option<RegionFill>,
    /// 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、円弧を弦に分ける目標にも使う
    pub tolerance: Option<f64>,
    /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
    pub expected_hash: Option<String>,
}

impl RegionsRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "regions")?;
        let doc = object.required("doc", json::string)?;
        let from = object.required("from", json::string)?;
        let operation = object.required("operation", RegionOperation::from_json)?;
        let subject = object.required("subject", |value| {
            json::array(value, MeasureElement::from_json)
        })?;
        let clip = object.optional("clip", |value| {
            json::array(value, MeasureElement::from_json)
        })?;
        let fill = object.optional("fill", RegionFill::from_json)?;
        let tolerance = object.optional("tolerance", json::number)?;
        let expected_hash = object.optional("expected_hash", json::string)?;
        Ok(Self {
            request_common,
            doc,
            from,
            operation,
            subject,
            clip,
            fill,
            tolerance,
            expected_hash,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("regions");
        out.key("doc");
        out.string(&self.doc);
        out.key("from");
        out.string(&self.from);
        out.key("operation");
        self.operation.to_json(out);
        out.key("subject");
        out.array(&self.subject, |out, item| item.to_json(out));
        if let Some(value) = &self.clip {
            out.key("clip");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.fill {
            out.key("fill");
            value.to_json(out);
        }
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.expected_hash {
            out.key("expected_hash");
            out.string(value);
        }
    }
}

impl FromJson for RegionsRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RegionsRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 閉領域の求め方。detect は subject の線が囲むすべての有限の面、union・intersection・difference・xor は subject と clip の和・積・差（subject から clip を除く）・排他的論理和
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegionOperation {
    /// `detect`
    Detect,
    /// `union`
    Union,
    /// `intersection`
    Intersection,
    /// `difference`
    Difference,
    /// `xor`
    Xor,
}

impl RegionOperation {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Detect,
        Self::Union,
        Self::Intersection,
        Self::Difference,
        Self::Xor,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Detect => "detect",
            Self::Union => "union",
            Self::Intersection => "intersection",
            Self::Difference => "difference",
            Self::Xor => "xor",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "detect" => Some(Self::Detect),
            "union" => Some(Self::Union),
            "intersection" => Some(Self::Intersection),
            "difference" => Some(Self::Difference),
            "xor" => Some(Self::Xor),
            _ => None,
        }
    }
}

impl FromJson for RegionOperation {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for RegionOperation {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 閉じた要素の内部を決める規則。even_odd は境界を横切る回数が奇数の部分、non_zero は符号を付けた周回数が 0 でない部分
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegionFill {
    /// `even_odd`
    EvenOdd,
    /// `non_zero`
    NonZero,
}

impl RegionFill {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::EvenOdd, Self::NonZero];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::EvenOdd => "even_odd",
            Self::NonZero => "non_zero",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "even_odd" => Some(Self::EvenOdd),
            "non_zero" => Some(Self::NonZero),
            _ => None,
        }
    }
}

impl FromJson for RegionFill {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for RegionFill {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 外周と穴で表した閉領域。最後の点から最初の点へ閉じる
#[derive(Clone, Debug, PartialEq)]
pub struct ClosedRegion {
    /// 反時計回りの外周。格子点の座標で、辞書順で最小の点から始める
    pub outer: Vec<Point2>,
    /// 時計回りの穴。輪郭の辞書順
    pub holes: Vec<Vec<Point2>>,
    /// 外周から穴を除いた面積。空間の長さの単位の 2 乗
    pub area: f64,
}

impl ClosedRegion {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let outer = object.required("outer", |value| json::array(value, Point2::from_json))?;
        let holes = object.required("holes", |value| {
            json::array(value, |value| json::array(value, Point2::from_json))
        })?;
        let area = object.required("area", json::number)?;
        Ok(Self { outer, holes, area })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("outer");
        out.array(&self.outer, |out, item| item.to_json(out));
        out.key("holes");
        out.array(&self.holes, |out, item| {
            out.array(item, |out, item| item.to_json(out))
        });
        out.key("area");
        out.float(self.area);
    }
}

impl FromJson for ClosedRegion {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ClosedRegion {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 交点を格子点へ丸めて元の要素の線を曲げた点
#[derive(Clone, Debug, PartialEq)]
pub struct RegionRounding {
    /// 曲げた要素の公開 ID
    pub id: String,
    /// 丸めた格子点
    pub position: Point2,
}

impl RegionRounding {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let position = object.required("position", Point2::from_json)?;
        Ok(Self { id, position })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("position");
        self.position.to_json(out);
    }
}

impl FromJson for RegionRounding {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RegionRounding {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 閉領域を作らなかった入力の種類。degenerate は点や一直線に縮んだ要素か長さ 0 の線、open は閉領域の境界に使わなかった線の部分
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegionDiscardKind {
    /// `degenerate`
    Degenerate,
    /// `open`
    Open,
}

impl RegionDiscardKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Degenerate, Self::Open];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Degenerate => "degenerate",
            Self::Open => "open",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "degenerate" => Some(Self::Degenerate),
            "open" => Some(Self::Open),
            _ => None,
        }
    }
}

impl FromJson for RegionDiscardKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for RegionDiscardKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 閉領域を作らなかった入力
#[derive(Clone, Debug, PartialEq)]
pub struct RegionDiscard {
    /// 元の要素の公開 ID
    pub id: String,
    /// 種類
    pub kind: RegionDiscardKind,
    /// 使わなかった部分の始点。open だけが持つ
    pub start: Option<Point2>,
    /// 使わなかった部分の終点。open だけが持つ
    pub end: Option<Point2>,
}

impl RegionDiscard {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let kind = object.required("kind", RegionDiscardKind::from_json)?;
        let start = object.optional("start", Point2::from_json)?;
        let end = object.optional("end", Point2::from_json)?;
        Ok(Self {
            id,
            kind,
            start,
            end,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.start {
            out.key("start");
            value.to_json(out);
        }
        if let Some(value) = &self.end {
            out.key("end");
            value.to_json(out);
        }
    }
}

impl FromJson for RegionDiscard {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RegionDiscard {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 閉領域。要素の順序を入れ替えても同じ領域を返す
#[derive(Clone, Debug, PartialEq)]
pub struct RegionsResult {
    /// 元の文書のハッシュ
    pub hash: String,
    /// 求めた実寸の空間の公開 ID
    pub from: String,
    /// 空間の長さの単位。不明な単位を補わない
    pub unit: Unit,
    /// 指定した、または既定の許容差
    pub tolerance: f64,
    /// 曲線を折れ線に近似した要素がある
    pub approximate: bool,
    /// 閉領域。外周、穴の順の辞書順
    pub regions: Vec<ClosedRegion>,
    /// 丸めた分割点。要素の指定順、格子点の辞書順
    pub rounded: Vec<RegionRounding>,
    /// 閉領域を作らなかった入力。degenerate、続いて open を元の入力順で返す
    pub discarded: Vec<RegionDiscard>,
}

impl RegionsResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let hash = object.required("hash", json::string)?;
        let from = object.required("from", json::string)?;
        let unit = object.required("unit", Unit::from_json)?;
        let tolerance = object.required("tolerance", json::number)?;
        let approximate = object.required("approximate", json::boolean)?;
        let regions = object.required("regions", |value| {
            json::array(value, ClosedRegion::from_json)
        })?;
        let rounded = object.required("rounded", |value| {
            json::array(value, RegionRounding::from_json)
        })?;
        let discarded = object.required("discarded", |value| {
            json::array(value, RegionDiscard::from_json)
        })?;
        Ok(Self {
            hash,
            from,
            unit,
            tolerance,
            approximate,
            regions,
            rounded,
            discarded,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("hash");
        out.string(&self.hash);
        out.key("from");
        out.string(&self.from);
        out.key("unit");
        self.unit.to_json(out);
        out.key("tolerance");
        out.float(self.tolerance);
        out.key("approximate");
        out.bool(self.approximate);
        out.key("regions");
        out.array(&self.regions, |out, item| item.to_json(out));
        out.key("rounded");
        out.array(&self.rounded, |out, item| item.to_json(out));
        out.key("discarded");
        out.array(&self.discarded, |out, item| item.to_json(out));
    }
}

impl FromJson for RegionsResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RegionsResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 同じ実寸の空間の要素のオフセット・トリム・延長・面取り・フィレットの結果の形を求める。文書を変更しない。結果は apply の remove・replace・add で当てる。形の種類が変わるときは replace でなく remove と add にする。マイナーバージョン 92 から
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 開いた文書の名前
    pub doc: String,
    /// 実寸の空間の公開 ID。用紙と記号定義の空間は対象外
    pub from: String,
    /// 作図の種類
    pub operation: ConstructOperation,
    /// 作図する要素。線分・円・円弧。面取りとフィレットでは 1 本目の線分
    pub target: MeasureElement,
    /// 面取りとフィレットの 2 本目の線分。target と端点を共有し、target と異なる ID
    pub second: Option<MeasureElement>,
    /// トリムと延長の境界の要素。線分・円・円弧・折れ線と、近似する曲線。1 つ以上で、同じ ID と target を含めない
    pub boundaries: Option<Vec<MeasureElement>>,
    /// トリムで削る区間を選ぶ点。線分は元の直線へ垂直に投影し、円・円弧は中心からの向きで選ぶ
    pub pick: Option<Point2>,
    /// 延長する端点
    pub end: Option<ConstructEnd>,
    /// オフセットの距離。線分は始点から終点へ向かって左、円・円弧は外側を正にする
    pub distance: Option<f64>,
    /// 面取りで、共有する端点から target と second の上を進む正の距離 [target, second]
    pub distances: Option<Vec<f64>>,
    /// フィレットの円弧の正の半径
    pub radius: Option<f64>,
    /// 許容差。空間の長さの単位で正の値。既定は 0.001。整数格子の間隔はこの 1/4 で、境界の曲線を近似する目標にも使う
    pub tolerance: Option<f64>,
    /// 期待する元の文書のハッシュ。open の元のバイト列のハッシュと照合する
    pub expected_hash: Option<String>,
}

impl ConstructRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "construct")?;
        let doc = object.required("doc", json::string)?;
        let from = object.required("from", json::string)?;
        let operation = object.required("operation", ConstructOperation::from_json)?;
        let target = object.required("target", MeasureElement::from_json)?;
        let second = object.optional("second", MeasureElement::from_json)?;
        let boundaries = object.optional("boundaries", |value| {
            json::array(value, MeasureElement::from_json)
        })?;
        let pick = object.optional("pick", Point2::from_json)?;
        let end = object.optional("end", ConstructEnd::from_json)?;
        let distance = object.optional("distance", json::number)?;
        let distances = object.optional("distances", |value| json::array(value, json::number))?;
        let radius = object.optional("radius", json::number)?;
        let tolerance = object.optional("tolerance", json::number)?;
        let expected_hash = object.optional("expected_hash", json::string)?;
        Ok(Self {
            request_common,
            doc,
            from,
            operation,
            target,
            second,
            boundaries,
            pick,
            end,
            distance,
            distances,
            radius,
            tolerance,
            expected_hash,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("construct");
        out.key("doc");
        out.string(&self.doc);
        out.key("from");
        out.string(&self.from);
        out.key("operation");
        self.operation.to_json(out);
        out.key("target");
        self.target.to_json(out);
        if let Some(value) = &self.second {
            out.key("second");
            value.to_json(out);
        }
        if let Some(value) = &self.boundaries {
            out.key("boundaries");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.pick {
            out.key("pick");
            value.to_json(out);
        }
        if let Some(value) = &self.end {
            out.key("end");
            value.to_json(out);
        }
        if let Some(value) = &self.distance {
            out.key("distance");
            out.float(*value);
        }
        if let Some(value) = &self.distances {
            out.key("distances");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.radius {
            out.key("radius");
            out.float(*value);
        }
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.expected_hash {
            out.key("expected_hash");
            out.string(value);
        }
    }
}

impl FromJson for ConstructRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConstructRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// offset は distance だけ平行に移した新しい形、trim は pick を含む区間を最初に当たる境界の間で削った残り、extend は end の端点を最初に当たる境界まで延ばした形、chamfer は target と second の角を distances で切る線分、fillet は radius の接する円弧
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConstructOperation {
    /// `offset`
    Offset,
    /// `trim`
    Trim,
    /// `extend`
    Extend,
    /// `chamfer`
    Chamfer,
    /// `fillet`
    Fillet,
}

impl ConstructOperation {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Offset,
        Self::Trim,
        Self::Extend,
        Self::Chamfer,
        Self::Fillet,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Offset => "offset",
            Self::Trim => "trim",
            Self::Extend => "extend",
            Self::Chamfer => "chamfer",
            Self::Fillet => "fillet",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "offset" => Some(Self::Offset),
            "trim" => Some(Self::Trim),
            "extend" => Some(Self::Extend),
            "chamfer" => Some(Self::Chamfer),
            "fillet" => Some(Self::Fillet),
            _ => None,
        }
    }
}

impl FromJson for ConstructOperation {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for ConstructOperation {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 延長する端点。start は始点を逆向きへ、end は終点を進む向きへ延ばす
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConstructEnd {
    /// `start`
    Start,
    /// `end`
    End,
}

impl ConstructEnd {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Start, Self::End];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "start" => Some(Self::Start),
            "end" => Some(Self::End),
            _ => None,
        }
    }
}

impl FromJson for ConstructEnd {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for ConstructEnd {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 元の要素を置き換える形。空なら要素を消す。2 つなら 1 つ目で置き換えて 2 つ目を足す。円をトリムした円弧のように種類が変わる形は、元の要素を消して足す
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructReplacement {
    /// 置き換える要素の公開 ID
    pub id: String,
    /// 置き換える形。元の向きの順
    pub geometries: Vec<Geometry>,
}

impl ConstructReplacement {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let geometries = object.required("geometries", |value| {
            json::array(value, Geometry::from_json)
        })?;
        Ok(Self { id, geometries })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("geometries");
        out.array(&self.geometries, |out, item| item.to_json(out));
    }
}

impl FromJson for ConstructReplacement {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConstructReplacement {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// トリムと延長で採用した境界と切断点
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructCut {
    /// 境界の要素の公開 ID
    pub boundary: String,
    /// 元の交点を丸めた格子点
    pub position: Point2,
    /// 元の交点が格子点でなく、近い格子点へ丸めた
    pub rounded: bool,
    /// 丸めた点を元の線・円の上へ投影したため、正確な切断位置と異なる
    pub adjusted: bool,
}

impl ConstructCut {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let boundary = object.required("boundary", json::string)?;
        let position = object.required("position", Point2::from_json)?;
        let rounded = object.required("rounded", json::boolean)?;
        let adjusted = object.required("adjusted", json::boolean)?;
        Ok(Self {
            boundary,
            position,
            rounded,
            adjusted,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("boundary");
        out.string(&self.boundary);
        out.key("position");
        self.position.to_json(out);
        out.key("rounded");
        out.bool(self.rounded);
        out.key("adjusted");
        out.bool(self.adjusted);
    }
}

impl FromJson for ConstructCut {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConstructCut {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 作図の結果の形。文書は変わらない
#[derive(Clone, Debug, PartialEq)]
pub struct ConstructResult {
    /// 元の文書のハッシュ
    pub hash: String,
    /// 求めた実寸の空間の公開 ID
    pub from: String,
    /// 空間の長さの単位。不明な単位を補わない
    pub unit: Unit,
    /// 指定した、または既定の許容差
    pub tolerance: f64,
    /// 境界の曲線を折れ線に近似した
    pub approximate: bool,
    /// 置き換える要素。target、second の順
    pub replace: Vec<ConstructReplacement>,
    /// 足す形。オフセットの形、面取りの線分、フィレットの円弧
    pub add: Vec<Geometry>,
    /// トリムと延長で採用した切断点。元の向きの順
    pub cuts: Vec<ConstructCut>,
}

impl ConstructResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let hash = object.required("hash", json::string)?;
        let from = object.required("from", json::string)?;
        let unit = object.required("unit", Unit::from_json)?;
        let tolerance = object.required("tolerance", json::number)?;
        let approximate = object.required("approximate", json::boolean)?;
        let replace = object.required("replace", |value| {
            json::array(value, ConstructReplacement::from_json)
        })?;
        let add = object.required("add", |value| json::array(value, Geometry::from_json))?;
        let cuts = object.required("cuts", |value| json::array(value, ConstructCut::from_json))?;
        Ok(Self {
            hash,
            from,
            unit,
            tolerance,
            approximate,
            replace,
            add,
            cuts,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("hash");
        out.string(&self.hash);
        out.key("from");
        out.string(&self.from);
        out.key("unit");
        self.unit.to_json(out);
        out.key("tolerance");
        out.float(self.tolerance);
        out.key("approximate");
        out.bool(self.approximate);
        out.key("replace");
        out.array(&self.replace, |out, item| item.to_json(out));
        out.key("add");
        out.array(&self.add, |out, item| item.to_json(out));
        out.key("cuts");
        out.array(&self.cuts, |out, item| item.to_json(out));
    }
}

impl FromJson for ConstructResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConstructResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文書の点の表し方。フィールドの意味は point_style_edit と同じ
#[derive(Clone, Debug, PartialEq)]
pub struct PointStyleSummary {
    /// 基本の形
    pub mark: PointMark,
    /// 円で囲む
    pub circle: bool,
    /// 四角で囲む
    pub square: bool,
    /// 大きさ。意味は size_kind で決まる
    pub size: f64,
    /// 大きさの意味
    pub size_kind: PointSizeKind,
}

impl PointStyleSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let mark = object.required("mark", PointMark::from_json)?;
        let circle = object.required("circle", json::boolean)?;
        let square = object.required("square", json::boolean)?;
        let size = object.required("size", json::number)?;
        let size_kind = object.required("size_kind", PointSizeKind::from_json)?;
        Ok(Self {
            mark,
            circle,
            square,
            size,
            size_kind,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("mark");
        self.mark.to_json(out);
        out.key("circle");
        out.bool(self.circle);
        out.key("square");
        out.bool(self.square);
        out.key("size");
        out.float(self.size);
        out.key("size_kind");
        self.size_kind.to_json(out);
    }
}

impl FromJson for PointStyleSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointStyleSummary {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 実寸の空間の要約
#[derive(Clone, Debug, PartialEq)]
pub struct SpaceSummary {
    /// 空間の ID（space/0）。entities の要素の ID の始まりと同じ（決定 0156）
    pub id: Option<String>,
    /// 文書の単位
    pub unit: Unit,
    /// 要素の数
    pub elements: ElementCounts,
    /// 線分の端点と文字の基点を囲む範囲。文書の単位。要素がなければ持たない
    pub extent: Option<Extent>,
}

impl SpaceSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.optional("id", json::string)?;
        let unit = object.required("unit", Unit::from_json)?;
        let elements = object.required("elements", ElementCounts::from_json)?;
        let extent = object.optional("extent", Extent::from_json)?;
        Ok(Self {
            id,
            unit,
            elements,
            extent,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        out.key("unit");
        self.unit.to_json(out);
        out.key("elements");
        self.elements.to_json(out);
        if let Some(value) = &self.extent {
            out.key("extent");
            value.to_json(out);
        }
    }
}

impl FromJson for SpaceSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SpaceSummary {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 用紙の要約
#[derive(Clone, Debug, PartialEq)]
pub struct SheetSummary {
    /// 用紙の ID。LAYOUT がハンドルを持てば sheet/h22、なければ読んだ順の番号（sheet/0）。entities の要素の ID の始まりと同じ（決定 0156）
    pub id: Option<String>,
    /// 元の形式の用紙の名前（DXF・DWG のレイアウトの名前）。翻訳しない。なければ持たない
    pub name: Option<String>,
    /// 用紙の幅と高さ（mm）。分からなければ持たない
    pub size: Option<Vec<f64>>,
    /// 要素の数
    pub elements: ElementCounts,
    /// ビューの数
    pub views: u64,
    /// ビューの詳細。元の順に返す。マイナーバージョン 81 から
    pub view_details: Vec<ViewDetail>,
    /// 線分の端点と文字の基点を囲む範囲。用紙の mm。要素がなければ持たない
    pub extent: Option<Extent>,
}

impl SheetSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.optional("id", json::string)?;
        let name = object.optional("name", json::string)?;
        let size = object.optional("size", |value| json::array(value, json::number))?;
        let elements = object.required("elements", ElementCounts::from_json)?;
        let views = object.required("views", |value| json::uint(value, 0, 9007199254740991))?;
        let view_details = object.required("view_details", |value| {
            json::array(value, ViewDetail::from_json)
        })?;
        let extent = object.optional("extent", Extent::from_json)?;
        Ok(Self {
            id,
            name,
            size,
            elements,
            views,
            view_details,
            extent,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.size {
            out.key("size");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("elements");
        self.elements.to_json(out);
        out.key("views");
        out.uint(self.views);
        out.key("view_details");
        out.array(&self.view_details, |out, item| item.to_json(out));
        if let Some(value) = &self.extent {
            out.key("extent");
            value.to_json(out);
        }
    }
}

impl FromJson for SheetSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SheetSummary {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 実寸の単位
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unit {
    /// `millimeter`
    Millimeter,
    /// `centimeter`
    Centimeter,
    /// `meter`
    Meter,
    /// `inch`
    Inch,
    /// `foot`
    Foot,
    /// `unknown`
    Unknown,
}

impl Unit {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Millimeter,
        Self::Centimeter,
        Self::Meter,
        Self::Inch,
        Self::Foot,
        Self::Unknown,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Millimeter => "millimeter",
            Self::Centimeter => "centimeter",
            Self::Meter => "meter",
            Self::Inch => "inch",
            Self::Foot => "foot",
            Self::Unknown => "unknown",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "millimeter" => Some(Self::Millimeter),
            "centimeter" => Some(Self::Centimeter),
            "meter" => Some(Self::Meter),
            "inch" => Some(Self::Inch),
            "foot" => Some(Self::Foot),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

impl FromJson for Unit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Unit {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 種類ごとの要素の数
#[derive(Clone, Debug, PartialEq)]
pub struct ElementCounts {
    /// 線分
    pub lines: u64,
    /// 文字
    pub texts: u64,
    /// 円。マイナーバージョン 81 から
    pub circles: u64,
    /// 円弧・楕円弧・スプラインなどの曲線。マイナーバージョン 81 から
    pub curves: u64,
    /// 折れ線。マイナーバージョン 81 から
    pub polylines: u64,
    /// 書式つきの文字。マイナーバージョン 81 から
    pub formatted_texts: u64,
    /// 寸法。マイナーバージョン 81 から
    pub dimensions: u64,
    /// 引出線。マイナーバージョン 81 から
    pub leaders: u64,
    /// 引出線つきの注記。マイナーバージョン 81 から
    pub callouts: u64,
    /// 表。マイナーバージョン 81 から
    pub tables: u64,
    /// アンダーレイ。マイナーバージョン 81 から
    pub underlays: u64,
    /// 記号の配置。中の要素を展開しない。マイナーバージョン 81 から
    pub symbols: u64,
    /// ハッチング。マイナーバージョン 81 から
    pub hatches: u64,
    /// 点。マイナーバージョン 81 から
    pub points: u64,
}

impl ElementCounts {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let lines = object.required("lines", |value| json::uint(value, 0, 9007199254740991))?;
        let texts = object.required("texts", |value| json::uint(value, 0, 9007199254740991))?;
        let circles = object.required("circles", |value| json::uint(value, 0, 9007199254740991))?;
        let curves = object.required("curves", |value| json::uint(value, 0, 9007199254740991))?;
        let polylines =
            object.required("polylines", |value| json::uint(value, 0, 9007199254740991))?;
        let formatted_texts = object.required("formatted_texts", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        let dimensions =
            object.required("dimensions", |value| json::uint(value, 0, 9007199254740991))?;
        let leaders = object.required("leaders", |value| json::uint(value, 0, 9007199254740991))?;
        let callouts =
            object.required("callouts", |value| json::uint(value, 0, 9007199254740991))?;
        let tables = object.required("tables", |value| json::uint(value, 0, 9007199254740991))?;
        let underlays =
            object.required("underlays", |value| json::uint(value, 0, 9007199254740991))?;
        let symbols = object.required("symbols", |value| json::uint(value, 0, 9007199254740991))?;
        let hatches = object.required("hatches", |value| json::uint(value, 0, 9007199254740991))?;
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            lines,
            texts,
            circles,
            curves,
            polylines,
            formatted_texts,
            dimensions,
            leaders,
            callouts,
            tables,
            underlays,
            symbols,
            hatches,
            points,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("lines");
        out.uint(self.lines);
        out.key("texts");
        out.uint(self.texts);
        out.key("circles");
        out.uint(self.circles);
        out.key("curves");
        out.uint(self.curves);
        out.key("polylines");
        out.uint(self.polylines);
        out.key("formatted_texts");
        out.uint(self.formatted_texts);
        out.key("dimensions");
        out.uint(self.dimensions);
        out.key("leaders");
        out.uint(self.leaders);
        out.key("callouts");
        out.uint(self.callouts);
        out.key("tables");
        out.uint(self.tables);
        out.key("underlays");
        out.uint(self.underlays);
        out.key("symbols");
        out.uint(self.symbols);
        out.key("hatches");
        out.uint(self.hatches);
        out.key("points");
        out.uint(self.points);
    }
}

impl FromJson for ElementCounts {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ElementCounts {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 2D のビュー。変換行列と切り抜きを保ち、尺度・向き・見えるモデル範囲を決定的に求める
#[derive(Clone, Debug, PartialEq)]
pub struct ViewDetail {
    /// 対象の実寸の空間の公開 ID
    pub space: String,
    /// モデル座標から用紙の mm への [a,b,c,d,e,f]。点を (ax+by+c,dx+ey+f) に写す
    pub matrix: Vec<f64>,
    /// モデルの X 軸・Y 軸の 1 単位に対応する用紙の長さ [SX,SY]（mm）。回転・せん断・反転でも各軸の正の長さを返す
    pub scale: Vec<f64>,
    /// モデル単位を mm に直した、各軸の縮尺の比 [SX,SY]。1:100 なら 0.01。単位不明なら持たない
    pub ratio: Option<Vec<f64>>,
    /// 用紙の X 軸から見たモデルの X 軸の向き。度、反時計回り、-180〜180
    pub rotation: f64,
    /// 変換が向きを反転する
    pub mirrored: bool,
    /// 用紙の mm での切り抜きの頂点。元の順を保ち、空なら切り抜かない
    pub clip: Vec<Point2>,
    /// 同じ頂点を逆変換したモデル座標。元の順を保つ
    pub model_clip: Vec<Point2>,
    /// model_clip を囲む範囲。切り抜きが空なら持たない。モデルの単位
    pub extent: Option<Extent>,
    /// ビューの出所
    pub source: Source,
}

impl ViewDetail {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let space = object.required("space", json::string)?;
        let matrix = object.required("matrix", |value| json::array(value, json::number))?;
        let scale = object.required("scale", |value| json::array(value, json::number))?;
        let ratio = object.optional("ratio", |value| json::array(value, json::number))?;
        let rotation = object.required("rotation", json::number)?;
        let mirrored = object.required("mirrored", json::boolean)?;
        let clip = object.required("clip", |value| json::array(value, Point2::from_json))?;
        let model_clip =
            object.required("model_clip", |value| json::array(value, Point2::from_json))?;
        let extent = object.optional("extent", Extent::from_json)?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            space,
            matrix,
            scale,
            ratio,
            rotation,
            mirrored,
            clip,
            model_clip,
            extent,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("space");
        out.string(&self.space);
        out.key("matrix");
        out.array(&self.matrix, |out, item| out.float(*item));
        out.key("scale");
        out.array(&self.scale, |out, item| out.float(*item));
        if let Some(value) = &self.ratio {
            out.key("ratio");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("rotation");
        out.float(self.rotation);
        out.key("mirrored");
        out.bool(self.mirrored);
        out.key("clip");
        out.array(&self.clip, |out, item| item.to_json(out));
        out.key("model_clip");
        out.array(&self.model_clip, |out, item| item.to_json(out));
        if let Some(value) = &self.extent {
            out.key("extent");
            value.to_json(out);
        }
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for ViewDetail {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ViewDetail {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 点を囲む、軸に沿った長方形
#[derive(Clone, Debug, PartialEq)]
pub struct Extent {
    /// X の最小
    pub min_x: f64,
    /// Y の最小
    pub min_y: f64,
    /// X の最大
    pub max_x: f64,
    /// Y の最大
    pub max_y: f64,
}

impl Extent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let min_x = object.required("min_x", json::number)?;
        let min_y = object.required("min_y", json::number)?;
        let max_x = object.required("max_x", json::number)?;
        let max_y = object.required("max_y", json::number)?;
        Ok(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("min_x");
        out.float(self.min_x);
        out.key("min_y");
        out.float(self.min_y);
        out.key("max_x");
        out.float(self.max_x);
        out.key("max_y");
        out.float(self.max_y);
    }
}

impl FromJson for Extent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Extent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117）
#[derive(Clone, Debug, PartialEq)]
pub struct EntitiesRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// すべての条件で絞った後の、返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す要素の数の上限。書かなければ 1000
    pub limit: Option<u64>,
    /// 返すフィールドの名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118）
    pub fields: Option<Vec<EntityField>>,
    /// [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118）
    pub region: Option<Vec<f64>>,
    /// 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。マイナーバージョン 31 から（決定 0177）
    pub place: Option<String>,
    /// 要素の公開 ID の一覧。いずれかに一致する要素だけを返す。空なら要素を返さず、ない ID は無視する。重複しても要素を増やさない。ほかの条件とも合わせる。マイナーバージョン 80 から
    pub ids: Option<Vec<String>>,
    /// tables.layers の公開 ID の一覧。名前ではない。いずれかのレイヤの要素だけを返す。空なら要素を返さず、ない ID は無視する。ほかの条件とも合わせる。マイナーバージョン 80 から
    pub layers: Option<Vec<String>>,
    /// 要素の種類の一覧。いずれかの種類だけを返す。空なら要素を返さず、重複しても要素を増やさない。ほかの条件とも合わせる。マイナーバージョン 80 から
    pub kinds: Option<Vec<EntityKind>>,
    /// 出所の条件。指定したすべての項目を満たす要素だけを返す。ほかの条件とも合わせる。マイナーバージョン 80 から
    pub source: Option<EntitySourceFilter>,
}

impl EntitiesRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "entities")?;
        let doc = object.required("doc", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        let fields =
            object.optional("fields", |value| json::array(value, EntityField::from_json))?;
        let region = object.optional("region", |value| json::array(value, json::number))?;
        let place = object.optional("place", json::string)?;
        let ids = object.optional("ids", |value| json::array(value, json::string))?;
        let layers = object.optional("layers", |value| json::array(value, json::string))?;
        let kinds = object.optional("kinds", |value| json::array(value, EntityKind::from_json))?;
        let source = object.optional("source", EntitySourceFilter::from_json)?;
        Ok(Self {
            request_common,
            doc,
            offset,
            limit,
            fields,
            region,
            place,
            ids,
            layers,
            kinds,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("entities");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
        if let Some(value) = &self.fields {
            out.key("fields");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.region {
            out.key("region");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.place {
            out.key("place");
            out.string(value);
        }
        if let Some(value) = &self.ids {
            out.key("ids");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.layers {
            out.key("layers");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.kinds {
            out.key("kinds");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.source {
            out.key("source");
            value.to_json(out);
        }
    }
}

impl FromJson for EntitiesRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntitiesRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 元の要素の出所による条件。空のオブジェクトなら制限しない。出所を持たない要素は指定した条件を満たさない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntitySourceFilter {
    /// L1 のレコード番号と完全に一致する
    pub record: Option<u64>,
    /// L1 のバイト範囲との重なり。start を含み end を含まない、空でない範囲。境界が接するだけなら一致しない
    pub bytes: Option<ByteRange>,
    /// AutoCAD のハンドル。接頭辞なし、1〜16 桁の 16 進の文字列。大文字と小文字を区別せず、数値で比べる。instance とは同時に指定できない
    pub handle: Option<String>,
    /// SXF のインスタンス番号。1〜20 桁の符号なし 64 bit 整数の 10 進文字列。数値で比べる。handle とは同時に指定できない
    pub instance: Option<String>,
}

impl EntitySourceFilter {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let record = object.optional("record", |value| json::uint(value, 0, 9007199254740991))?;
        let bytes = object.optional("bytes", ByteRange::from_json)?;
        let handle = object.optional("handle", json::string)?;
        let instance = object.optional("instance", json::string)?;
        Ok(Self {
            record,
            bytes,
            handle,
            instance,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.record {
            out.key("record");
            out.uint(*value);
        }
        if let Some(value) = &self.bytes {
            out.key("bytes");
            value.to_json(out);
        }
        if let Some(value) = &self.handle {
            out.key("handle");
            out.string(value);
        }
        if let Some(value) = &self.instance {
            out.key("instance");
            out.string(value);
        }
    }
}

impl FromJson for EntitySourceFilter {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntitySourceFilter {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素の一覧の一部。移さなかった図形と値は、診断で返す
#[derive(Clone, Debug, PartialEq)]
pub struct EntitiesResult {
    /// offset から limit 個までの要素
    pub items: Vec<Entity>,
    /// 全体の要素の数
    pub total: u64,
    /// 続きの最初の要素の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl EntitiesResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let items = object.required("items", |value| json::array(value, Entity::from_json))?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            items,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("items");
        out.array(&self.items, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for EntitiesResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntitiesResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 図面モデルの要素
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    /// 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つキー（ハンドル）があればキーで作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どのバージョンでも同じ ID を返す（決定 0117・0156）
    pub id: String,
    /// 形の種類
    pub kind: Option<EntityKind>,
    /// レイヤの元の名前。翻訳しない。図面モデルにレイヤがなければ持たない
    pub layer: Option<String>,
    /// 元の形式のレコード
    pub source: Option<Source>,
    /// 形の定義から求めた範囲。文字の字形・線の太さ・記号の中身は含めない。囲む点がなければ持たない（決定 0118）
    pub bbox: Option<Extent>,
    /// 要素のレイヤ・スタイル・形のハッシュ。出所の位置は含めない。変更の前提条件に使う（v2: と 32 桁の 16 進、決定 0156）
    pub hash: Option<String>,
    /// 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点かフィット点で決まるスプライン・寸法・引出線（LEADER）・点・引出線つきの注記・表（ACAD_TABLE）だけが持つ。apply の replace と add に渡せる形（決定 0139）。マイナーバージョン 10 から
    pub geometry: Option<Geometry>,
    /// 要素自身のスタイル。fields に style を指定したときだけ返す。マイナーバージョン 79 から
    pub style: Option<EntityStyle>,
    /// 要素に付く拡張の一覧。fields に extensions を指定したときだけ返し、拡張がなければ空の一覧。マイナーバージョン 79 から
    pub extensions: Option<Vec<EntityExtension>>,
}

impl Entity {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let kind = object.optional("kind", EntityKind::from_json)?;
        let layer = object.optional("layer", json::string)?;
        let source = object.optional("source", Source::from_json)?;
        let bbox = object.optional("bbox", Extent::from_json)?;
        let hash = object.optional("hash", json::string)?;
        let geometry = object.optional("geometry", Geometry::from_json)?;
        let style = object.optional("style", EntityStyle::from_json)?;
        let extensions = object.optional("extensions", |value| {
            json::array(value, EntityExtension::from_json)
        })?;
        Ok(Self {
            id,
            kind,
            layer,
            source,
            bbox,
            hash,
            geometry,
            style,
            extensions,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        if let Some(value) = &self.kind {
            out.key("kind");
            value.to_json(out);
        }
        if let Some(value) = &self.layer {
            out.key("layer");
            out.string(value);
        }
        if let Some(value) = &self.source {
            out.key("source");
            value.to_json(out);
        }
        if let Some(value) = &self.bbox {
            out.key("bbox");
            value.to_json(out);
        }
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
        if let Some(value) = &self.geometry {
            out.key("geometry");
            value.to_json(out);
        }
        if let Some(value) = &self.style {
            out.key("style");
            value.to_json(out);
        }
        if let Some(value) = &self.extensions {
            out.key("extensions");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for Entity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Entity {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// entities で返す要素のフィールド（決定 0118）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityField {
    /// `id`
    Id,
    /// `kind`
    Kind,
    /// `layer`
    Layer,
    /// `source`
    Source,
    /// `bbox`
    Bbox,
    /// `hash`
    Hash,
    /// `geometry`
    Geometry,
    /// `style`
    Style,
    /// `extensions`
    Extensions,
}

impl EntityField {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Id,
        Self::Kind,
        Self::Layer,
        Self::Source,
        Self::Bbox,
        Self::Hash,
        Self::Geometry,
        Self::Style,
        Self::Extensions,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Kind => "kind",
            Self::Layer => "layer",
            Self::Source => "source",
            Self::Bbox => "bbox",
            Self::Hash => "hash",
            Self::Geometry => "geometry",
            Self::Style => "style",
            Self::Extensions => "extensions",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "id" => Some(Self::Id),
            "kind" => Some(Self::Kind),
            "layer" => Some(Self::Layer),
            "source" => Some(Self::Source),
            "bbox" => Some(Self::Bbox),
            "hash" => Some(Self::Hash),
            "geometry" => Some(Self::Geometry),
            "style" => Some(Self::Style),
            "extensions" => Some(Self::Extensions),
            _ => None,
        }
    }
}

impl FromJson for EntityField {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for EntityField {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 要素自身が持つスタイル。継承は解かず、色表の RGB も補わない。色・線種・線幅の形は restyle と同じ。透過性は読むだけで、restyle の指定には含めない
#[derive(Clone, Debug, PartialEq)]
pub struct EntityStyle {
    /// 色の符号・図面内 RGB、または継承元
    pub color: ColorChange,
    /// 継承元か、tables が返す線種の公開 ID
    pub line_type: LineTypeChange,
    /// 要素自身の線種の倍率。文書や配置の倍率を掛けない
    pub line_type_scale: f64,
    /// 線幅の指定。元の形式から読めていなければ持たない
    pub line_weight: Option<LineWeightChange>,
    /// 透過性の指定
    pub transparency: EntityTransparency,
}

impl EntityStyle {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let color = object.required("color", ColorChange::from_json)?;
        let line_type = object.required("line_type", LineTypeChange::from_json)?;
        let line_type_scale = object.required("line_type_scale", json::number)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        let transparency = object.required("transparency", EntityTransparency::from_json)?;
        Ok(Self {
            color,
            line_type,
            line_type_scale,
            line_weight,
            transparency,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("color");
        self.color.to_json(out);
        out.key("line_type");
        self.line_type.to_json(out);
        out.key("line_type_scale");
        out.float(self.line_type_scale);
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
        out.key("transparency");
        self.transparency.to_json(out);
    }
}

impl FromJson for EntityStyle {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityStyle {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 透過性。inherit または alpha の一方を返す
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityTransparency {
    /// 継承元
    pub inherit: Option<Inherit>,
    /// 不透明度。0 は透明、255 は不透明
    pub alpha: Option<u64>,
}

impl EntityTransparency {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let inherit = object.optional("inherit", Inherit::from_json)?;
        let alpha = object.optional("alpha", |value| json::uint(value, 0, 255))?;
        Ok(Self { inherit, alpha })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.inherit {
            out.key("inherit");
            value.to_json(out);
        }
        if let Some(value) = &self.alpha {
            out.key("alpha");
            out.uint(*value);
        }
    }
}

impl FromJson for EntityTransparency {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityTransparency {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素に付く形式固有の情報。元の順序を保ち、失効した値も current を偽にして返す。書き戻す操作ではない
#[derive(Clone, Debug, PartialEq)]
pub struct EntityExtension {
    /// 値の型を定める名前空間
    pub namespace: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 元の形式のバージョン。不明なら持たない
    pub version: Option<String>,
    /// 名前空間の値の型のバージョン
    pub schema: u64,
    /// 拡張が依存する元の要素のハッシュ。要素に依存するハッシュがなければ持たない
    pub basis: Option<String>,
    /// basis が現在の要素のハッシュと一致する
    pub current: bool,
    /// 元の順序と型を保つフィールド
    pub fields: Vec<EntityExtensionField>,
    /// 拡張の出所
    pub source: Source,
}

impl EntityExtension {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let namespace = object.required("namespace", json::string)?;
        let name = object.required("name", json::string)?;
        let version = object.optional("version", json::string)?;
        let schema = object.required("schema", |value| json::uint(value, 0, 9007199254740991))?;
        let basis = object.optional("basis", json::string)?;
        let current = object.required("current", json::boolean)?;
        let fields = object.required("fields", |value| {
            json::array(value, EntityExtensionField::from_json)
        })?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            namespace,
            name,
            version,
            schema,
            basis,
            current,
            fields,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("namespace");
        out.string(&self.namespace);
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.version {
            out.key("version");
            out.string(value);
        }
        out.key("schema");
        out.uint(self.schema);
        if let Some(value) = &self.basis {
            out.key("basis");
            out.string(value);
        }
        out.key("current");
        out.bool(self.current);
        out.key("fields");
        out.array(&self.fields, |out, item| item.to_json(out));
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for EntityExtension {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityExtension {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 拡張のフィールド。同じキーの重複と並びを保つ
#[derive(Clone, Debug, PartialEq)]
pub struct EntityExtensionField {
    /// 元のキー
    pub key: String,
    /// 元の形式の値の番号。番号がなければ持たない
    pub code: Option<i64>,
    /// 型付きの値
    pub value: EntityExtensionValue,
}

impl EntityExtensionField {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let key = object.required("key", json::string)?;
        let code = object.optional("code", |value| json::int(value, -32768, 32767))?;
        let value = object.required("value", EntityExtensionValue::from_json)?;
        Ok(Self { key, code, value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("key");
        out.string(&self.key);
        if let Some(value) = &self.code {
            out.key("code");
            out.int(*value);
        }
        out.key("value");
        self.value.to_json(out);
    }
}

impl FromJson for EntityExtensionField {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityExtensionField {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 拡張の値。整数・ハンドル・バイト列は文字列で正確に返し、実数と点は有限値だけを返す
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum EntityExtensionValue {
    /// 元の文字列
    Text(EntityExtensionText),
    /// 符号付き 64 bit 整数
    Integer(EntityExtensionInteger),
    /// 有限の実数
    Real(EntityExtensionReal),
    /// 3 次元の点やベクトル。単位と空間は名前空間が定める
    Point(EntityExtensionPoint),
    /// 元の形式のオブジェクトを指す符号なし 64 bit の値
    Handle(EntityExtensionHandle),
    /// 元のバイト列
    Bytes(EntityExtensionBytes),
}

impl EntityExtensionValue {
    /// タグのフィールド（`kind`）の値（スキーマの順）
    pub const TAGS: &[&str] = &["text", "integer", "real", "point", "handle", "bytes"];

    /// タグのフィールド（`kind`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::Integer(_) => "integer",
            Self::Real(_) => "real",
            Self::Point(_) => "point",
            Self::Handle(_) => "handle",
            Self::Bytes(_) => "bytes",
        }
    }
}

impl FromJson for EntityExtensionValue {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("kind")? {
            "text" => Self::Text(EntityExtensionText::read_fields(&mut object)?),
            "integer" => Self::Integer(EntityExtensionInteger::read_fields(&mut object)?),
            "real" => Self::Real(EntityExtensionReal::read_fields(&mut object)?),
            "point" => Self::Point(EntityExtensionPoint::read_fields(&mut object)?),
            "handle" => Self::Handle(EntityExtensionHandle::read_fields(&mut object)?),
            "bytes" => Self::Bytes(EntityExtensionBytes::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("kind")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityExtensionValue {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Text(value) => value.to_json(out),
            Self::Integer(value) => value.to_json(out),
            Self::Real(value) => value.to_json(out),
            Self::Point(value) => value.to_json(out),
            Self::Handle(value) => value.to_json(out),
            Self::Bytes(value) => value.to_json(out),
        }
    }
}

/// 元の文字列
#[derive(Clone, Debug, PartialEq)]
pub struct EntityExtensionText {
    /// 文字列
    pub value: String,
}

impl EntityExtensionText {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "text")?;
        let value = object.required("value", json::string)?;
        Ok(Self { value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("text");
        out.key("value");
        out.string(&self.value);
    }
}

impl FromJson for EntityExtensionText {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityExtensionText {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 符号付き 64 bit 整数
#[derive(Clone, Debug, PartialEq)]
pub struct EntityExtensionInteger {
    /// 10 進の文字列。負数以外には符号を付けず、先頭の 0 を付けない
    pub value: String,
}

impl EntityExtensionInteger {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "integer")?;
        let value = object.required("value", json::string)?;
        Ok(Self { value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("integer");
        out.key("value");
        out.string(&self.value);
    }
}

impl FromJson for EntityExtensionInteger {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityExtensionInteger {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 有限の実数
#[derive(Clone, Debug, PartialEq)]
pub struct EntityExtensionReal {
    /// 実数
    pub value: f64,
}

impl EntityExtensionReal {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "real")?;
        let value = object.required("value", json::number)?;
        Ok(Self { value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("real");
        out.key("value");
        out.float(self.value);
    }
}

impl FromJson for EntityExtensionReal {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityExtensionReal {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 3 次元の点やベクトル。単位と空間は名前空間が定める
#[derive(Clone, Debug, PartialEq)]
pub struct EntityExtensionPoint {
    /// [X, Y, Z] の有限値
    pub value: Vec<f64>,
}

impl EntityExtensionPoint {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "point")?;
        let value = object.required("value", |value| json::array(value, json::number))?;
        Ok(Self { value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("point");
        out.key("value");
        out.array(&self.value, |out, item| out.float(*item));
    }
}

impl FromJson for EntityExtensionPoint {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityExtensionPoint {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 元の形式のオブジェクトを指す符号なし 64 bit の値
#[derive(Clone, Debug, PartialEq)]
pub struct EntityExtensionHandle {
    /// 大文字の 16 進の文字列。接頭辞と先頭の 0 を付けない
    pub value: String,
}

impl EntityExtensionHandle {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "handle")?;
        let value = object.required("value", json::string)?;
        Ok(Self { value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("handle");
        out.key("value");
        out.string(&self.value);
    }
}

impl FromJson for EntityExtensionHandle {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityExtensionHandle {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 元のバイト列
#[derive(Clone, Debug, PartialEq)]
pub struct EntityExtensionBytes {
    /// 1 バイトを小文字の 16 進 2 桁にした文字列。空も保つ
    pub value: String,
}

impl EntityExtensionBytes {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "bytes")?;
        let value = object.required("value", json::string)?;
        Ok(Self { value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("bytes");
        out.key("value");
        out.string(&self.value);
    }
}

impl FromJson for EntityExtensionBytes {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EntityExtensionBytes {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Geometry {
    /// 線分
    Line(LineGeometry),
    /// 円
    Circle(CircleGeometry),
    /// 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる
    Arc(ArcGeometry),
    /// 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。マイナーバージョン 11 から
    Ellipse(EllipseGeometry),
    /// 頂点を結んだ折れ線。幅は持たない
    Polyline(PolylineGeometry),
    /// 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせを元の値のままにし、書かない幅の倍率・斜体・反転も元の値を受け継ぐ。足すときは、2 点目がなければ書かない位置合わせを左の基準線にし、幅の倍率 1・斜体なし・反転なしにする
    Text(TextGeometry),
    /// 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味を元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。マイナーバージョン 13 から
    FormattedText(FormattedTextGeometry),
    /// 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。マイナーバージョン 14 から
    Symbol(SymbolGeometry),
    /// スプライン（NURBS）。制御点で決まるもの（マイナーバージョン 15 から）と、フィット点で決まるもの（マイナーバージョン 18 から）がある。フィット点で決まるものは、ノットと制御点を空にし、フィット点（fit）と、あれば両端の接線を持つ。元の図形に制御点やノットもあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものとフィット点で決まるものを入れ替えない
    Spline(SplineGeometry),
    /// 寸法。種類（kind）ごとに、その種類の定義点のフィールドを持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。置き換え（DXF と DWG の DIMENSION、DXF の ARC_DIMENSION）では定義点と文字を書き直し、測定値を測り直して表示の部品を作り直す（マイナーバージョン 74 から。それまでは置き換えも change.edit.unsupported だった）。measurement は省くか元と同じ値を書く。足すことは、まだしない。マイナーバージョン 19 から
    Dimension(DimensionGeometry),
    /// 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（マイナーバージョン 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印のフラグを必ず持つので、arrow を書かない引出線は書けない（マイナーバージョン 36 から。それまでは R2004 までの置き換えだけ）。マイナーバージョン 31 から
    Leader(LeaderGeometry),
    /// 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義のマーカーの点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。マイナーバージョン 35 から
    Point(PointGeometry),
    /// 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかのフィールドは元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。マイナーバージョン 36 から
    Callout(CalloutGeometry),
    /// 表（DXF・DWG の ACAD_TABLE、決定 0113）。挿入点・行の向き・行の高さ・列の幅と、行ごとのセルの並びを持つ。表のスタイル・セルの書式・型のある値・表示の部品（ブロック）は形の値に含めない。置き換え（DXF の ACAD_TABLE）では、行と列の数・セルの結合・文字でないセル（記号）を変えず、挿入点・行の向き・行の高さ・列の幅・文字と空のセルの内容と回転を書き直し、表示の部品を作り直す。足すことと DWG の書き戻しは、まだしない。マイナーバージョン 74 から
    Table(TableGeometry),
    /// 非連動のハッチング。閉じた折れ線（膨らみ付き）と円を境界にできる。塗りは単色か、線の定義を明示した模様。参照・勾配・自動検出した境界は扱わない。DXF と DWG に追加できる。置き換え・変換はまだ扱わない。entities は用紙上の模様を返さない。用紙への模様の追加は、原点が 0 で 1 単位が 1 mm のときに限る。マイナーバージョン 75 から
    Hatch(HatchGeometry),
}

impl Geometry {
    /// タグのフィールド（`shape`）の値（スキーマの順）
    pub const TAGS: &[&str] = &[
        "line",
        "circle",
        "arc",
        "ellipse",
        "polyline",
        "text",
        "formatted_text",
        "symbol",
        "spline",
        "dimension",
        "leader",
        "point",
        "callout",
        "table",
        "hatch",
    ];

    /// タグのフィールド（`shape`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Line(_) => "line",
            Self::Circle(_) => "circle",
            Self::Arc(_) => "arc",
            Self::Ellipse(_) => "ellipse",
            Self::Polyline(_) => "polyline",
            Self::Text(_) => "text",
            Self::FormattedText(_) => "formatted_text",
            Self::Symbol(_) => "symbol",
            Self::Spline(_) => "spline",
            Self::Dimension(_) => "dimension",
            Self::Leader(_) => "leader",
            Self::Point(_) => "point",
            Self::Callout(_) => "callout",
            Self::Table(_) => "table",
            Self::Hatch(_) => "hatch",
        }
    }
}

impl FromJson for Geometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("shape")? {
            "line" => Self::Line(LineGeometry::read_fields(&mut object)?),
            "circle" => Self::Circle(CircleGeometry::read_fields(&mut object)?),
            "arc" => Self::Arc(ArcGeometry::read_fields(&mut object)?),
            "ellipse" => Self::Ellipse(EllipseGeometry::read_fields(&mut object)?),
            "polyline" => Self::Polyline(PolylineGeometry::read_fields(&mut object)?),
            "text" => Self::Text(TextGeometry::read_fields(&mut object)?),
            "formatted_text" => {
                Self::FormattedText(FormattedTextGeometry::read_fields(&mut object)?)
            }
            "symbol" => Self::Symbol(SymbolGeometry::read_fields(&mut object)?),
            "spline" => Self::Spline(SplineGeometry::read_fields(&mut object)?),
            "dimension" => Self::Dimension(DimensionGeometry::read_fields(&mut object)?),
            "leader" => Self::Leader(LeaderGeometry::read_fields(&mut object)?),
            "point" => Self::Point(PointGeometry::read_fields(&mut object)?),
            "callout" => Self::Callout(CalloutGeometry::read_fields(&mut object)?),
            "table" => Self::Table(TableGeometry::read_fields(&mut object)?),
            "hatch" => Self::Hatch(HatchGeometry::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("shape")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Geometry {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Line(value) => value.to_json(out),
            Self::Circle(value) => value.to_json(out),
            Self::Arc(value) => value.to_json(out),
            Self::Ellipse(value) => value.to_json(out),
            Self::Polyline(value) => value.to_json(out),
            Self::Text(value) => value.to_json(out),
            Self::FormattedText(value) => value.to_json(out),
            Self::Symbol(value) => value.to_json(out),
            Self::Spline(value) => value.to_json(out),
            Self::Dimension(value) => value.to_json(out),
            Self::Leader(value) => value.to_json(out),
            Self::Point(value) => value.to_json(out),
            Self::Callout(value) => value.to_json(out),
            Self::Table(value) => value.to_json(out),
            Self::Hatch(value) => value.to_json(out),
        }
    }
}

/// 非連動のハッチング。閉じた折れ線（膨らみ付き）と円を境界にできる。塗りは単色か、線の定義を明示した模様。参照・勾配・自動検出した境界は扱わない。DXF と DWG に追加できる。置き換え・変換はまだ扱わない。entities は用紙上の模様を返さない。用紙への模様の追加は、原点が 0 で 1 単位が 1 mm のときに限る。マイナーバージョン 75 から
#[derive(Clone, Debug, PartialEq)]
pub struct HatchGeometry {
    /// 境界の輪。入力の順。1 つ以上
    pub loops: Vec<HatchLoop>,
    /// 輪の中の輪の埋め方
    pub islands: HatchIslands,
    /// 中を埋めるもの
    pub fill: HatchFill,
    /// fill が pattern のときだけ持つ模様の定義
    pub pattern: Option<HatchPattern>,
    /// 境界を探した点 [[X, Y], ...]。書かなければ空
    pub seeds: Option<Vec<Vec<f64>>>,
}

impl HatchGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "hatch")?;
        let loops = object.required("loops", |value| json::array(value, HatchLoop::from_json))?;
        let islands = object.required("islands", HatchIslands::from_json)?;
        let fill = object.required("fill", HatchFill::from_json)?;
        let pattern = object.optional("pattern", HatchPattern::from_json)?;
        let seeds = object.optional("seeds", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        Ok(Self {
            loops,
            islands,
            fill,
            pattern,
            seeds,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("hatch");
        out.key("loops");
        out.array(&self.loops, |out, item| item.to_json(out));
        out.key("islands");
        self.islands.to_json(out);
        out.key("fill");
        self.fill.to_json(out);
        if let Some(value) = &self.pattern {
            out.key("pattern");
            value.to_json(out);
        }
        if let Some(value) = &self.seeds {
            out.key("seeds");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.float(*item))
            });
        }
    }
}

impl FromJson for HatchGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for HatchGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// ハッチングの境界の輪。参照を持たない
#[derive(Clone, Debug, PartialEq)]
pub struct HatchLoop {
    /// 閉じた折れ線か円
    pub boundary: HatchBoundary,
    /// 外側の輪
    pub external: bool,
    /// いちばん外の輪
    pub outermost: bool,
}

impl HatchLoop {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let boundary = object.required("boundary", HatchBoundary::from_json)?;
        let external = object.required("external", json::boolean)?;
        let outermost = object.required("outermost", json::boolean)?;
        Ok(Self {
            boundary,
            external,
            outermost,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("boundary");
        self.boundary.to_json(out);
        out.key("external");
        out.bool(self.external);
        out.key("outermost");
        out.bool(self.outermost);
    }
}

impl FromJson for HatchLoop {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for HatchLoop {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 輪の形。折れ線は closed が真で頂点が 2 つ以上。最後の点から最初の点へ結ぶ
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum HatchBoundary {
    /// 頂点を結んだ折れ線。幅は持たない
    Polyline(PolylineGeometry),
    /// 円
    Circle(CircleGeometry),
}

impl HatchBoundary {
    /// タグのフィールド（`shape`）の値（スキーマの順）
    pub const TAGS: &[&str] = &["polyline", "circle"];

    /// タグのフィールド（`shape`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Polyline(_) => "polyline",
            Self::Circle(_) => "circle",
        }
    }
}

impl FromJson for HatchBoundary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("shape")? {
            "polyline" => Self::Polyline(PolylineGeometry::read_fields(&mut object)?),
            "circle" => Self::Circle(CircleGeometry::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("shape")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for HatchBoundary {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Polyline(value) => value.to_json(out),
            Self::Circle(value) => value.to_json(out),
        }
    }
}

/// nested は外から奇数番目の内側、outer はいちばん外と次の輪の間、ignore は内側の輪を無視して埋める
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HatchIslands {
    /// `nested`
    Nested,
    /// `outer`
    Outer,
    /// `ignore`
    Ignore,
}

impl HatchIslands {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Nested, Self::Outer, Self::Ignore];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Nested => "nested",
            Self::Outer => "outer",
            Self::Ignore => "ignore",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "nested" => Some(Self::Nested),
            "outer" => Some(Self::Outer),
            "ignore" => Some(Self::Ignore),
            _ => None,
        }
    }
}

impl FromJson for HatchIslands {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for HatchIslands {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// solid は要素の色で塗り、pattern は線の組で埋める
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HatchFill {
    /// `solid`
    Solid,
    /// `pattern`
    Pattern,
}

impl HatchFill {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Solid, Self::Pattern];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Solid => "solid",
            Self::Pattern => "pattern",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "solid" => Some(Self::Solid),
            "pattern" => Some(Self::Pattern),
            _ => None,
        }
    }
}

impl FromJson for HatchFill {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for HatchFill {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 模様の名前と線の定義。外部の定義ファイルを読まず、lines の値を使う
#[derive(Clone, Debug, PartialEq)]
pub struct HatchPattern {
    /// 元の名前。空と制御文字を含むものは受けない
    pub name: String,
    /// 定義の出どころ
    pub origin: HatchPatternOrigin,
    /// 定義に加えた回転（度）
    pub angle: f64,
    /// 定義に掛けた尺度か、利用者が決めた線の間隔。0 より大きい
    pub scale: f64,
    /// 線の組を 90 度回して重ねる
    pub double: bool,
    /// angle と scale を当てた後の線の組。1 つ以上
    pub lines: Vec<HatchPatternLine>,
}

impl HatchPattern {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let origin = object.required("origin", HatchPatternOrigin::from_json)?;
        let angle = object.required("angle", json::number)?;
        let scale = object.required("scale", json::number)?;
        let double = object.required("double", json::boolean)?;
        let lines = object.required("lines", |value| {
            json::array(value, HatchPatternLine::from_json)
        })?;
        Ok(Self {
            name,
            origin,
            angle,
            scale,
            double,
            lines,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("origin");
        self.origin.to_json(out);
        out.key("angle");
        out.float(self.angle);
        out.key("scale");
        out.float(self.scale);
        out.key("double");
        out.bool(self.double);
        out.key("lines");
        out.array(&self.lines, |out, item| item.to_json(out));
    }
}

impl FromJson for HatchPattern {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for HatchPattern {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// user_defined は利用者が決めた平行線、predefined は形式が名前で決めた定義、custom は利用者の定義
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HatchPatternOrigin {
    /// `user_defined`
    UserDefined,
    /// `predefined`
    Predefined,
    /// `custom`
    Custom,
}

impl HatchPatternOrigin {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::UserDefined, Self::Predefined, Self::Custom];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::UserDefined => "user_defined",
            Self::Predefined => "predefined",
            Self::Custom => "custom",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "user_defined" => Some(Self::UserDefined),
            "predefined" => Some(Self::Predefined),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

impl FromJson for HatchPatternOrigin {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for HatchPatternOrigin {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 1 組の平行線。基点・ずれ・線の長さは要素の座標と単位に従う
#[derive(Clone, Debug, PartialEq)]
pub struct HatchPatternLine {
    /// 線の向き（度）
    pub angle: f64,
    /// 1 本の線が通る点 [X, Y]
    pub base: Vec<f64>,
    /// 隣の線までのずれ [X, Y]。両方 0 にはしない
    pub offset: Vec<f64>,
    /// 正は線、負は間、0 は点。空なら実線
    pub dashes: Vec<f64>,
}

impl HatchPatternLine {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let angle = object.required("angle", json::number)?;
        let base = object.required("base", |value| json::array(value, json::number))?;
        let offset = object.required("offset", |value| json::array(value, json::number))?;
        let dashes = object.required("dashes", |value| json::array(value, json::number))?;
        Ok(Self {
            angle,
            base,
            offset,
            dashes,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("angle");
        out.float(self.angle);
        out.key("base");
        out.array(&self.base, |out, item| out.float(*item));
        out.key("offset");
        out.array(&self.offset, |out, item| out.float(*item));
        out.key("dashes");
        out.array(&self.dashes, |out, item| out.float(*item));
    }
}

impl FromJson for HatchPatternLine {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for HatchPatternLine {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線分
#[derive(Clone, Debug, PartialEq)]
pub struct LineGeometry {
    /// 始点 [X, Y]
    pub start: Vec<f64>,
    /// 終点 [X, Y]
    pub end: Vec<f64>,
}

impl LineGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "line")?;
        let start = object.required("start", |value| json::array(value, json::number))?;
        let end = object.required("end", |value| json::array(value, json::number))?;
        Ok(Self { start, end })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("line");
        out.key("start");
        out.array(&self.start, |out, item| out.float(*item));
        out.key("end");
        out.array(&self.end, |out, item| out.float(*item));
    }
}

impl FromJson for LineGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 円
#[derive(Clone, Debug, PartialEq)]
pub struct CircleGeometry {
    /// 中心 [X, Y]
    pub center: Vec<f64>,
    /// 半径。0 より大きい
    pub radius: f64,
}

impl CircleGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "circle")?;
        let center = object.required("center", |value| json::array(value, json::number))?;
        let radius = object.required("radius", json::number)?;
        Ok(Self { center, radius })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("circle");
        out.key("center");
        out.array(&self.center, |out, item| out.float(*item));
        out.key("radius");
        out.float(self.radius);
    }
}

impl FromJson for CircleGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CircleGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる
#[derive(Clone, Debug, PartialEq)]
pub struct ArcGeometry {
    /// 中心 [X, Y]
    pub center: Vec<f64>,
    /// 半径。0 より大きい
    pub radius: f64,
    /// 始まりの角度（度）
    pub start_angle: f64,
    /// 終わりの角度（度）
    pub end_angle: f64,
}

impl ArcGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "arc")?;
        let center = object.required("center", |value| json::array(value, json::number))?;
        let radius = object.required("radius", json::number)?;
        let start_angle = object.required("start_angle", json::number)?;
        let end_angle = object.required("end_angle", json::number)?;
        Ok(Self {
            center,
            radius,
            start_angle,
            end_angle,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("arc");
        out.key("center");
        out.array(&self.center, |out, item| out.float(*item));
        out.key("radius");
        out.float(self.radius);
        out.key("start_angle");
        out.float(self.start_angle);
        out.key("end_angle");
        out.float(self.end_angle);
    }
}

impl FromJson for ArcGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ArcGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。マイナーバージョン 11 から
#[derive(Clone, Debug, PartialEq)]
pub struct EllipseGeometry {
    /// 中心 [X, Y]
    pub center: Vec<f64>,
    /// 長軸の端の点 [X, Y]。中心と違う点
    pub major: Vec<f64>,
    /// 短軸の長軸に対する比。0 より大きく 1 以下
    pub ratio: f64,
    /// 始まりの角度（度）。楕円を円に引き延ばしたときの角度
    pub start_angle: f64,
    /// 終わりの角度（度）。楕円を円に引き延ばしたときの角度
    pub end_angle: f64,
}

impl EllipseGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "ellipse")?;
        let center = object.required("center", |value| json::array(value, json::number))?;
        let major = object.required("major", |value| json::array(value, json::number))?;
        let ratio = object.required("ratio", json::number)?;
        let start_angle = object.required("start_angle", json::number)?;
        let end_angle = object.required("end_angle", json::number)?;
        Ok(Self {
            center,
            major,
            ratio,
            start_angle,
            end_angle,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("ellipse");
        out.key("center");
        out.array(&self.center, |out, item| out.float(*item));
        out.key("major");
        out.array(&self.major, |out, item| out.float(*item));
        out.key("ratio");
        out.float(self.ratio);
        out.key("start_angle");
        out.float(self.start_angle);
        out.key("end_angle");
        out.float(self.end_angle);
    }
}

impl FromJson for EllipseGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for EllipseGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 頂点を結んだ折れ線。幅は持たない
#[derive(Clone, Debug, PartialEq)]
pub struct PolylineGeometry {
    /// 頂点の [X, Y] の並び。2 つ以上
    pub points: Vec<Vec<f64>>,
    /// 頂点ごとの、次の頂点への弧の膨らみ（中心角の 4 分の 1 の正接。正は反時計回り）。points と同じ数。書かなければどれも 0
    pub bulges: Option<Vec<f64>>,
    /// 最後の頂点から最初の頂点へ結ぶ
    pub closed: bool,
}

impl PolylineGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "polyline")?;
        let points = object.required("points", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let bulges = object.optional("bulges", |value| json::array(value, json::number))?;
        let closed = object.required("closed", json::boolean)?;
        Ok(Self {
            points,
            bulges,
            closed,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("polyline");
        out.key("points");
        out.array(&self.points, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
        if let Some(value) = &self.bulges {
            out.key("bulges");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("closed");
        out.bool(self.closed);
    }
}

impl FromJson for PolylineGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PolylineGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせを元の値のままにし、書かない幅の倍率・斜体・反転も元の値を受け継ぐ。足すときは、2 点目がなければ書かない位置合わせを左の基準線にし、幅の倍率 1・斜体なし・反転なしにする
#[derive(Clone, Debug, PartialEq)]
pub struct TextGeometry {
    /// 内容。1 行
    pub content: String,
    /// 基点 [X, Y]。2 点の間に合わせる文字では 1 点目
    pub position: Vec<f64>,
    /// 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。マイナーバージョン 17 から
    pub end: Option<Vec<f64>>,
    /// 2 点の間への合わせ方。2 点目（end）と一緒に書く。マイナーバージョン 17 から
    pub fit: Option<TextFit>,
    /// 1 点に置く文字の水平の位置合わせ。left のときは entities が省き、足すときも省けば left。2 点の間に合わせる文字には書けない。置き換えるときは元と同じ値だけを受ける。マイナーバージョン 69 から
    pub horizontal: Option<TextHorizontal>,
    /// 1 点に置く文字の垂直の位置合わせ。baseline のときは entities が省き、足すときも省けば baseline。2 点の間に合わせる文字には書けない。置き換えるときは元と同じ値だけを受ける。マイナーバージョン 69 から
    pub vertical: Option<TextVertical>,
    /// 高さ。0 より大きい
    pub height: f64,
    /// 高さ（height）を用紙の mm で表すか。実寸の空間の文字が用紙の長さで高さを持つとき（JWW）、entities は真を返す。足す・置き換えるときに真を書けば、高さを用紙の mm として渡す。偽のときは entities が省き、省けば高さを点と同じ単位とする。マイナーバージョン 93 から
    pub height_on_paper: Option<bool>,
    /// 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない
    pub rotation: f64,
    /// 幅の倍率。0 より大きい。1 のときは entities が省き、足すときも省けば 1、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
    pub width_factor: Option<f64>,
    /// 斜体の角度（度）。0 のときは entities が省き、足すときも省けば 0、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
    pub oblique: Option<f64>,
    /// 左右を裏返すか。偽のときは entities が省き、足すときも省けば偽、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
    pub backward: Option<bool>,
    /// 上下を反転するか。偽のときは entities が省き、足すときも省けば偽、置き換えるときは省けば元の値を受け継ぐ。マイナーバージョン 69 から
    pub upside_down: Option<bool>,
    /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、省けば元の名前を保ち、DXF・DWG は定義のある別の文字スタイルも受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。マイナーバージョン 12 から
    pub style: Option<String>,
    /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。マイナーバージョン 23 から
    pub style_id: Option<String>,
}

impl TextGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "text")?;
        let content = object.required("content", json::string)?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let end = object.optional("end", |value| json::array(value, json::number))?;
        let fit = object.optional("fit", TextFit::from_json)?;
        let horizontal = object.optional("horizontal", TextHorizontal::from_json)?;
        let vertical = object.optional("vertical", TextVertical::from_json)?;
        let height = object.required("height", json::number)?;
        let height_on_paper = object.optional("height_on_paper", json::boolean)?;
        let rotation = object.required("rotation", json::number)?;
        let width_factor = object.optional("width_factor", json::number)?;
        let oblique = object.optional("oblique", json::number)?;
        let backward = object.optional("backward", json::boolean)?;
        let upside_down = object.optional("upside_down", json::boolean)?;
        let style = object.optional("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        Ok(Self {
            content,
            position,
            end,
            fit,
            horizontal,
            vertical,
            height,
            height_on_paper,
            rotation,
            width_factor,
            oblique,
            backward,
            upside_down,
            style,
            style_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("text");
        out.key("content");
        out.string(&self.content);
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        if let Some(value) = &self.end {
            out.key("end");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.fit {
            out.key("fit");
            value.to_json(out);
        }
        if let Some(value) = &self.horizontal {
            out.key("horizontal");
            value.to_json(out);
        }
        if let Some(value) = &self.vertical {
            out.key("vertical");
            value.to_json(out);
        }
        out.key("height");
        out.float(self.height);
        if let Some(value) = &self.height_on_paper {
            out.key("height_on_paper");
            out.bool(*value);
        }
        out.key("rotation");
        out.float(self.rotation);
        if let Some(value) = &self.width_factor {
            out.key("width_factor");
            out.float(*value);
        }
        if let Some(value) = &self.oblique {
            out.key("oblique");
            out.float(*value);
        }
        if let Some(value) = &self.backward {
            out.key("backward");
            out.bool(*value);
        }
        if let Some(value) = &self.upside_down {
            out.key("upside_down");
            out.bool(*value);
        }
        if let Some(value) = &self.style {
            out.key("style");
            out.string(value);
        }
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
    }
}

impl FromJson for TextGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TextGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 1 点に置く文字の水平の位置合わせ。middle は水平と垂直の両方を外形の中央に合わせる（DXF の 72 = 4）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextHorizontal {
    /// `left`
    Left,
    /// `center`
    Center,
    /// `right`
    Right,
    /// `middle`
    Middle,
}

impl TextHorizontal {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Left, Self::Center, Self::Right, Self::Middle];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Middle => "middle",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            "middle" => Some(Self::Middle),
            _ => None,
        }
    }
}

impl FromJson for TextHorizontal {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for TextHorizontal {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 1 点に置く文字の垂直の位置合わせ
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextVertical {
    /// `baseline`
    Baseline,
    /// `bottom`
    Bottom,
    /// `middle`
    Middle,
    /// `top`
    Top,
}

impl TextVertical {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Baseline, Self::Bottom, Self::Middle, Self::Top];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Bottom => "bottom",
            Self::Middle => "middle",
            Self::Top => "top",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "baseline" => Some(Self::Baseline),
            "bottom" => Some(Self::Bottom),
            "middle" => Some(Self::Middle),
            "top" => Some(Self::Top),
            _ => None,
        }
    }
}

impl FromJson for TextVertical {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for TextVertical {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextFit {
    /// `aligned`
    Aligned,
    /// `fit`
    Fit,
}

impl TextFit {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Aligned, Self::Fit];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::Fit => "fit",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "aligned" => Some(Self::Aligned),
            "fit" => Some(Self::Fit),
            _ => None,
        }
    }
}

impl FromJson for TextFit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for TextFit {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味を元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。マイナーバージョン 13 から
#[derive(Clone, Debug, PartialEq)]
pub struct FormattedTextGeometry {
    /// 内容。書式の符号（\P の改行など）を含めた、元の形式の内容のまま
    pub content: String,
    /// 基点 [X, Y]
    pub position: Vec<f64>,
    /// 高さ。0 より大きい
    pub height: f64,
    /// 回転（度）
    pub rotation: f64,
    /// 基点に合わせる枠の位置
    pub attachment: Option<Attachment>,
    /// 折り返す枠の幅。0 より大きい。書かなければ折り返さない
    pub width: Option<f64>,
    /// 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、省けば元の名前を保ち、DXF・DWG は定義のある別の文字スタイルも受ける。style_id と一緒に書くなら、同じ文字スタイルを指す
    pub style: Option<String>,
    /// 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。マイナーバージョン 23 から
    pub style_id: Option<String>,
}

impl FormattedTextGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "formatted_text")?;
        let content = object.required("content", json::string)?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let height = object.required("height", json::number)?;
        let rotation = object.required("rotation", json::number)?;
        let attachment = object.optional("attachment", Attachment::from_json)?;
        let width = object.optional("width", json::number)?;
        let style = object.optional("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        Ok(Self {
            content,
            position,
            height,
            rotation,
            attachment,
            width,
            style,
            style_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("formatted_text");
        out.key("content");
        out.string(&self.content);
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        out.key("height");
        out.float(self.height);
        out.key("rotation");
        out.float(self.rotation);
        if let Some(value) = &self.attachment {
            out.key("attachment");
            value.to_json(out);
        }
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.style {
            out.key("style");
            out.string(value);
        }
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
    }
}

impl FromJson for FormattedTextGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for FormattedTextGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// スプライン（NURBS）。制御点で決まるもの（マイナーバージョン 15 から）と、フィット点で決まるもの（マイナーバージョン 18 から）がある。フィット点で決まるものは、ノットと制御点を空にし、フィット点（fit）と、あれば両端の接線を持つ。元の図形に制御点やノットもあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものとフィット点で決まるものを入れ替えない
#[derive(Clone, Debug, PartialEq)]
pub struct SplineGeometry {
    /// 次数。1 以上
    pub degree: u64,
    /// ノットの並び。周期的でなければ、制御点の数と次数と 1 の和の数。フィット点で決まるものは空
    pub knots: Vec<f64>,
    /// 制御点の [X, Y] の並び。次数より多い。フィット点で決まるものは空
    pub control: Vec<Vec<f64>>,
    /// 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない
    pub weights: Option<Vec<f64>>,
    /// フィット点の [X, Y] の並び。2 つ以上。書けば、ノットと制御点は空で、重みは書かない。マイナーバージョン 18 から
    pub fit: Option<Vec<Vec<f64>>>,
    /// フィット点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。マイナーバージョン 18 から
    pub start_tangent: Option<Vec<f64>>,
    /// フィット点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。マイナーバージョン 18 から
    pub end_tangent: Option<Vec<f64>>,
    /// 閉じている
    pub closed: bool,
    /// 周期的
    pub periodic: bool,
}

impl SplineGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "spline")?;
        let degree = object.required("degree", |value| json::uint(value, 1, 32767))?;
        let knots = object.required("knots", |value| json::array(value, json::number))?;
        let control = object.required("control", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let weights = object.optional("weights", |value| json::array(value, json::number))?;
        let fit = object.optional("fit", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let start_tangent =
            object.optional("start_tangent", |value| json::array(value, json::number))?;
        let end_tangent =
            object.optional("end_tangent", |value| json::array(value, json::number))?;
        let closed = object.required("closed", json::boolean)?;
        let periodic = object.required("periodic", json::boolean)?;
        Ok(Self {
            degree,
            knots,
            control,
            weights,
            fit,
            start_tangent,
            end_tangent,
            closed,
            periodic,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("spline");
        out.key("degree");
        out.uint(self.degree);
        out.key("knots");
        out.array(&self.knots, |out, item| out.float(*item));
        out.key("control");
        out.array(&self.control, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
        if let Some(value) = &self.weights {
            out.key("weights");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.fit {
            out.key("fit");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.float(*item))
            });
        }
        if let Some(value) = &self.start_tangent {
            out.key("start_tangent");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.end_tangent {
            out.key("end_tangent");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("closed");
        out.bool(self.closed);
        out.key("periodic");
        out.bool(self.periodic);
    }
}

impl FromJson for SplineGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SplineGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法。種類（kind）ごとに、その種類の定義点のフィールドを持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。置き換え（DXF と DWG の DIMENSION、DXF の ARC_DIMENSION）では定義点と文字を書き直し、測定値を測り直して表示の部品を作り直す（マイナーバージョン 74 から。それまでは置き換えも change.edit.unsupported だった）。measurement は省くか元と同じ値を書く。足すことは、まだしない。マイナーバージョン 19 から
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionGeometry {
    /// 寸法の種類
    pub kind: DimensionKind,
    /// 1 本目の補助線の点か、測る角度の 1 本目の線の点か、円弧の始めの点 [X, Y]。linear・aligned・angular・arc_length が持つ
    pub first: Option<Vec<f64>>,
    /// 2 本目の補助線の点か、測る角度の 2 本目の線の点か、円弧の終わりの点 [X, Y]。linear・aligned・angular・arc_length が持つ
    pub second: Option<Vec<f64>>,
    /// 寸法線が通る点 [X, Y]。linear・aligned が持つ
    pub line: Option<Vec<f64>>,
    /// 測る向き（度）。linear が持つ
    pub rotation: Option<f64>,
    /// 補助線の傾き（度）。傾けなければ 0。linear・aligned が持つ
    pub oblique: Option<f64>,
    /// 角度の頂点 [X, Y]。angular が持つ
    pub vertex: Option<Vec<f64>>,
    /// 1 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
    pub first_line: Option<Vec<Vec<f64>>>,
    /// 2 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ
    pub second_line: Option<Vec<Vec<f64>>>,
    /// 寸法の弧が通る点 [X, Y]。angular・angular_lines・arc_length が持つ
    pub arc: Option<Vec<f64>>,
    /// 中心 [X, Y]。arc_length・radius が持つ
    pub center: Option<Vec<f64>>,
    /// 円の上の点 [X, Y]。radius・diameter が持つ
    pub chord: Option<Vec<f64>>,
    /// chord と向かい合う円の上の点 [X, Y]。diameter が持つ
    pub opposite: Option<Vec<f64>>,
    /// 引出線の長さ。radius・diameter が持つ
    pub leader: Option<f64>,
    /// 原点 [X, Y]。ordinate が持つ
    pub origin: Option<Vec<f64>>,
    /// 測る点 [X, Y]。ordinate が持つ
    pub feature: Option<Vec<f64>>,
    /// 引出線の終点 [X, Y]。ordinate が持つ
    pub leader_end: Option<Vec<f64>>,
    /// 測る座標の軸。ordinate が持つ
    pub axis: Option<DimensionAxis>,
    /// 寸法値の文字の書き方
    pub label: DimensionLabel,
    /// 利用者が書いた寸法値の文字の元の内容。label が override のときだけ持つ。測定値に置き換える符号（DXF の <>）と書式の符号もそのまま持つ
    pub content: Option<String>,
    /// 元の形式が保存した測定値。単位と倍率は measurement_key のフィールドに従う
    pub measurement: Option<f64>,
    /// measurement の形式とフィールドを指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ
    pub measurement_key: Option<String>,
    /// 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない
    pub text_position: Option<Vec<f64>>,
    /// 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない
    pub text_rotation: Option<f64>,
    /// 寸法スタイルの元の名前
    pub style: String,
    /// 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける。マイナーバージョン 23 から
    pub style_id: Option<String>,
}

impl DimensionGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "dimension")?;
        let kind = object.required("kind", DimensionKind::from_json)?;
        let first = object.optional("first", |value| json::array(value, json::number))?;
        let second = object.optional("second", |value| json::array(value, json::number))?;
        let line = object.optional("line", |value| json::array(value, json::number))?;
        let rotation = object.optional("rotation", json::number)?;
        let oblique = object.optional("oblique", json::number)?;
        let vertex = object.optional("vertex", |value| json::array(value, json::number))?;
        let first_line = object.optional("first_line", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let second_line = object.optional("second_line", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let arc = object.optional("arc", |value| json::array(value, json::number))?;
        let center = object.optional("center", |value| json::array(value, json::number))?;
        let chord = object.optional("chord", |value| json::array(value, json::number))?;
        let opposite = object.optional("opposite", |value| json::array(value, json::number))?;
        let leader = object.optional("leader", json::number)?;
        let origin = object.optional("origin", |value| json::array(value, json::number))?;
        let feature = object.optional("feature", |value| json::array(value, json::number))?;
        let leader_end = object.optional("leader_end", |value| json::array(value, json::number))?;
        let axis = object.optional("axis", DimensionAxis::from_json)?;
        let label = object.required("label", DimensionLabel::from_json)?;
        let content = object.optional("content", json::string)?;
        let measurement = object.optional("measurement", json::number)?;
        let measurement_key = object.optional("measurement_key", json::string)?;
        let text_position =
            object.optional("text_position", |value| json::array(value, json::number))?;
        let text_rotation = object.optional("text_rotation", json::number)?;
        let style = object.required("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        Ok(Self {
            kind,
            first,
            second,
            line,
            rotation,
            oblique,
            vertex,
            first_line,
            second_line,
            arc,
            center,
            chord,
            opposite,
            leader,
            origin,
            feature,
            leader_end,
            axis,
            label,
            content,
            measurement,
            measurement_key,
            text_position,
            text_rotation,
            style,
            style_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("dimension");
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.first {
            out.key("first");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.second {
            out.key("second");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.line {
            out.key("line");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.rotation {
            out.key("rotation");
            out.float(*value);
        }
        if let Some(value) = &self.oblique {
            out.key("oblique");
            out.float(*value);
        }
        if let Some(value) = &self.vertex {
            out.key("vertex");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.first_line {
            out.key("first_line");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.float(*item))
            });
        }
        if let Some(value) = &self.second_line {
            out.key("second_line");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.float(*item))
            });
        }
        if let Some(value) = &self.arc {
            out.key("arc");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.center {
            out.key("center");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.chord {
            out.key("chord");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.opposite {
            out.key("opposite");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.leader {
            out.key("leader");
            out.float(*value);
        }
        if let Some(value) = &self.origin {
            out.key("origin");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.feature {
            out.key("feature");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.leader_end {
            out.key("leader_end");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.axis {
            out.key("axis");
            value.to_json(out);
        }
        out.key("label");
        self.label.to_json(out);
        if let Some(value) = &self.content {
            out.key("content");
            out.string(value);
        }
        if let Some(value) = &self.measurement {
            out.key("measurement");
            out.float(*value);
        }
        if let Some(value) = &self.measurement_key {
            out.key("measurement_key");
            out.string(value);
        }
        if let Some(value) = &self.text_position {
            out.key("text_position");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.text_rotation {
            out.key("text_rotation");
            out.float(*value);
        }
        out.key("style");
        out.string(&self.style);
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
    }
}

impl FromJson for DimensionGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DimensionGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（マイナーバージョン 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印のフラグを必ず持つので、arrow を書かない引出線は書けない（マイナーバージョン 36 から。それまでは R2004 までの置き換えだけ）。マイナーバージョン 31 から
#[derive(Clone, Debug, PartialEq)]
pub struct LeaderGeometry {
    /// 経路の頂点 [[X, Y], ...]。矢印の側から並べる。2 つ以上
    pub path: Vec<Vec<f64>>,
    /// 頂点を通る曲線で結ぶか。偽なら折れ線
    pub spline: bool,
    /// 始点に矢印を付けるか。書かなければ寸法スタイルに従う
    pub arrow: Option<bool>,
    /// 注記の要素の ID。同じ空間か用紙の要素で、entities が返したもの。注記がなければ書かない。置き換えるときは元と同じ ID だけを受ける。足すときは、同じ空間か用紙の書式つきの文字（MTEXT）を指す
    pub annotation: Option<String>,
    /// 寸法スタイルの元の名前。足すときは図面にある寸法スタイルの名前を指す。置き換えるときは元と同じ名前だけを受ける
    pub style: String,
    /// 寸法スタイルの ID。tables が返したもの。entities は、名前で引いた寸法スタイルの ID を返す。足す・置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける
    pub style_id: Option<String>,
}

impl LeaderGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "leader")?;
        let path = object.required("path", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let spline = object.required("spline", json::boolean)?;
        let arrow = object.optional("arrow", json::boolean)?;
        let annotation = object.optional("annotation", json::string)?;
        let style = object.required("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        Ok(Self {
            path,
            spline,
            arrow,
            annotation,
            style,
            style_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("leader");
        out.key("path");
        out.array(&self.path, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
        out.key("spline");
        out.bool(self.spline);
        if let Some(value) = &self.arrow {
            out.key("arrow");
            out.bool(*value);
        }
        if let Some(value) = &self.annotation {
            out.key("annotation");
            out.string(value);
        }
        out.key("style");
        out.string(&self.style);
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
    }
}

impl FromJson for LeaderGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LeaderGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかのフィールドは元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。マイナーバージョン 36 から
#[derive(Clone, Debug, PartialEq)]
pub struct CalloutGeometry {
    /// 枝。入力の順
    pub branches: Vec<CalloutBranch>,
    /// 線の引き方
    pub path: CalloutPath,
    /// 線を注記に、水平の線（ランディング）でつなぐか
    pub landing: bool,
    /// 端部記号の大きさ
    pub arrow_size: f64,
    /// 大きさに掛ける倍率
    pub scale: f64,
    /// 注記の種類
    pub content: CalloutContent,
    /// 注記の書式つきの文字の元の内容。content が text のときだけ持つ
    pub text: Option<String>,
    /// 注記の記号のブロックの名前。content が symbol のときだけ持つ
    pub symbol: Option<String>,
}

impl CalloutGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "callout")?;
        let branches = object.required("branches", |value| {
            json::array(value, CalloutBranch::from_json)
        })?;
        let path = object.required("path", CalloutPath::from_json)?;
        let landing = object.required("landing", json::boolean)?;
        let arrow_size = object.required("arrow_size", json::number)?;
        let scale = object.required("scale", json::number)?;
        let content = object.required("content", CalloutContent::from_json)?;
        let text = object.optional("text", json::string)?;
        let symbol = object.optional("symbol", json::string)?;
        Ok(Self {
            branches,
            path,
            landing,
            arrow_size,
            scale,
            content,
            text,
            symbol,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("callout");
        out.key("branches");
        out.array(&self.branches, |out, item| item.to_json(out));
        out.key("path");
        self.path.to_json(out);
        out.key("landing");
        out.bool(self.landing);
        out.key("arrow_size");
        out.float(self.arrow_size);
        out.key("scale");
        out.float(self.scale);
        out.key("content");
        self.content.to_json(out);
        if let Some(value) = &self.text {
            out.key("text");
            out.string(value);
        }
        if let Some(value) = &self.symbol {
            out.key("symbol");
            out.string(value);
        }
    }
}

impl FromJson for CalloutGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CalloutGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる
#[derive(Clone, Debug, PartialEq)]
pub struct CalloutBranch {
    /// 線ごとの頂点 [[[X, Y], ...], ...]。矢印の側から並べ、集まる点は含めない。線ごとに 1 つ以上
    pub lines: Vec<Vec<Vec<f64>>>,
    /// 線が集まる点 [X, Y]。元の形式が持たなければ書かない
    pub end: Option<Vec<f64>>,
    /// 集まる点から注記への折れ曲がり。なければ書かない
    pub dogleg: Option<CalloutDogleg>,
}

impl CalloutBranch {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let lines = object.required("lines", |value| {
            json::array(value, |value| {
                json::array(value, |value| json::array(value, json::number))
            })
        })?;
        let end = object.optional("end", |value| json::array(value, json::number))?;
        let dogleg = object.optional("dogleg", CalloutDogleg::from_json)?;
        Ok(Self { lines, end, dogleg })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("lines");
        out.array(&self.lines, |out, item| {
            out.array(item, |out, item| {
                out.array(item, |out, item| out.float(*item))
            })
        });
        if let Some(value) = &self.end {
            out.key("end");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.dogleg {
            out.key("dogleg");
            value.to_json(out);
        }
    }
}

impl FromJson for CalloutBranch {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CalloutBranch {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 枝の折れ曲がり
#[derive(Clone, Debug, PartialEq)]
pub struct CalloutDogleg {
    /// 向き [X, Y]。長さは意味を持たない
    pub direction: Vec<f64>,
    /// 長さ
    pub length: f64,
}

impl CalloutDogleg {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let direction = object.required("direction", |value| json::array(value, json::number))?;
        let length = object.required("length", json::number)?;
        Ok(Self { direction, length })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("direction");
        out.array(&self.direction, |out, item| out.float(*item));
        out.key("length");
        out.float(self.length);
    }
}

impl FromJson for CalloutDogleg {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CalloutDogleg {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 引出線つきの注記の線の引き方。straight は折れ線、spline は頂点を通る曲線、hidden は線を描かない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalloutPath {
    /// `straight`
    Straight,
    /// `spline`
    Spline,
    /// `hidden`
    Hidden,
}

impl CalloutPath {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Straight, Self::Spline, Self::Hidden];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Straight => "straight",
            Self::Spline => "spline",
            Self::Hidden => "hidden",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "straight" => Some(Self::Straight),
            "spline" => Some(Self::Spline),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }
}

impl FromJson for CalloutPath {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CalloutPath {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 引出線つきの注記の中身の種類。none は注記を持たず、text は書式つきの文字、symbol は記号（バルーンなどの囲み）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CalloutContent {
    /// `none`
    None,
    /// `text`
    Text,
    /// `symbol`
    Symbol,
}

impl CalloutContent {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::None, Self::Text, Self::Symbol];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Text => "text",
            Self::Symbol => "symbol",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "text" => Some(Self::Text),
            "symbol" => Some(Self::Symbol),
            _ => None,
        }
    }
}

impl FromJson for CalloutContent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CalloutContent {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 表（DXF・DWG の ACAD_TABLE、決定 0113）。挿入点・行の向き・行の高さ・列の幅と、行ごとのセルの並びを持つ。表のスタイル・セルの書式・型のある値・表示の部品（ブロック）は形の値に含めない。置き換え（DXF の ACAD_TABLE）では、行と列の数・セルの結合・文字でないセル（記号）を変えず、挿入点・行の向き・行の高さ・列の幅・文字と空のセルの内容と回転を書き直し、表示の部品を作り直す。足すことと DWG の書き戻しは、まだしない。マイナーバージョン 74 から
#[derive(Clone, Debug, PartialEq)]
pub struct TableGeometry {
    /// 挿入点 [X, Y]
    pub position: Vec<f64>,
    /// 行の向き [X, Y]。0 でない
    pub direction: Vec<f64>,
    /// 上の行からの行の高さ。どれも 0 より大きい。1 つ以上
    pub row_heights: Vec<f64>,
    /// 左の列からの列の幅。どれも 0 より大きい。1 つ以上
    pub column_widths: Vec<f64>,
    /// 上の行から、行ごとに左の列から並べたセル。数は行の数と列の数の積
    pub cells: Vec<TableCell>,
}

impl TableGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "table")?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let direction = object.required("direction", |value| json::array(value, json::number))?;
        let row_heights =
            object.required("row_heights", |value| json::array(value, json::number))?;
        let column_widths =
            object.required("column_widths", |value| json::array(value, json::number))?;
        let cells = object.required("cells", |value| json::array(value, TableCell::from_json))?;
        Ok(Self {
            position,
            direction,
            row_heights,
            column_widths,
            cells,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("table");
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        out.key("direction");
        out.array(&self.direction, |out, item| out.float(*item));
        out.key("row_heights");
        out.array(&self.row_heights, |out, item| out.float(*item));
        out.key("column_widths");
        out.array(&self.column_widths, |out, item| out.float(*item));
        out.key("cells");
        out.array(&self.cells, |out, item| item.to_json(out));
    }
}

impl FromJson for TableGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TableGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 表の 1 つのセル
#[derive(Clone, Debug, PartialEq)]
pub struct TableCell {
    /// 中身の種類
    pub content: TableCellContent,
    /// 書式の符号を含む元の文字列。空でない。content が text のときだけ持つ
    pub text: Option<String>,
    /// 記号のブロックの名前。content が symbol のときだけ持つ
    pub symbol: Option<String>,
    /// 結合して広げる列と行の数 [列, 行]。結合しなければ [1, 1]（省けば [1, 1]）
    pub span: Option<Vec<u64>>,
    /// ほかのセルに結合されて覆われるか。偽のときは省ける
    pub covered: Option<bool>,
    /// 文字か記号の回転。0 のときは省ける
    pub rotation: Option<f64>,
}

impl TableCell {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let content = object.required("content", TableCellContent::from_json)?;
        let text = object.optional("text", json::string)?;
        let symbol = object.optional("symbol", json::string)?;
        let span = object.optional("span", |value| {
            json::array(value, |value| json::uint(value, 1, 32767))
        })?;
        let covered = object.optional("covered", json::boolean)?;
        let rotation = object.optional("rotation", json::number)?;
        Ok(Self {
            content,
            text,
            symbol,
            span,
            covered,
            rotation,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("content");
        self.content.to_json(out);
        if let Some(value) = &self.text {
            out.key("text");
            out.string(value);
        }
        if let Some(value) = &self.symbol {
            out.key("symbol");
            out.string(value);
        }
        if let Some(value) = &self.span {
            out.key("span");
            out.array(value, |out, item| out.uint(*item));
        }
        if let Some(value) = &self.covered {
            out.key("covered");
            out.bool(*value);
        }
        if let Some(value) = &self.rotation {
            out.key("rotation");
            out.float(*value);
        }
    }
}

impl FromJson for TableCell {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TableCell {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 表のセルの中身の種類。empty は空、text は書式つきの文字、symbol は記号（ブロック）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TableCellContent {
    /// `empty`
    Empty,
    /// `text`
    Text,
    /// `symbol`
    Symbol,
}

impl TableCellContent {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Empty, Self::Text, Self::Symbol];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Text => "text",
            Self::Symbol => "symbol",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "empty" => Some(Self::Empty),
            "text" => Some(Self::Text),
            "symbol" => Some(Self::Symbol),
            _ => None,
        }
    }
}

impl FromJson for TableCellContent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for TableCellContent {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義のマーカーの点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。マイナーバージョン 35 から
#[derive(Clone, Debug, PartialEq)]
pub struct PointGeometry {
    /// 位置 [X, Y]
    pub position: Vec<f64>,
    /// 回転（度）
    pub rotation: f64,
    /// 既定義のマーカー。書かなければ文書の点の表し方に従う
    pub marker: Option<PointMarker>,
    /// 作図の補助の点（JWW の仮点）か。書かなければ偽
    pub construction: Option<bool>,
}

impl PointGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "point")?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let rotation = object.required("rotation", json::number)?;
        let marker = object.optional("marker", PointMarker::from_json)?;
        let construction = object.optional("construction", json::boolean)?;
        Ok(Self {
            position,
            rotation,
            marker,
            construction,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("point");
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        out.key("rotation");
        out.float(self.rotation);
        if let Some(value) = &self.marker {
            out.key("marker");
            value.to_json(out);
        }
        if let Some(value) = &self.construction {
            out.key("construction");
            out.bool(*value);
        }
    }
}

impl FromJson for PointGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 既定義のマーカーの表のキーと尺度。マーカーの形は、表示する側がキーから引く
#[derive(Clone, Debug, PartialEq)]
pub struct PointMarker {
    /// マーカーの表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点）
    pub namespace: String,
    /// 名前空間の中の元のコード
    pub code: String,
    /// 尺度
    pub scale: f64,
}

impl PointMarker {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let namespace = object.required("namespace", json::string)?;
        let code = object.required("code", json::string)?;
        let scale = object.required("scale", json::number)?;
        Ok(Self {
            namespace,
            code,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("namespace");
        out.string(&self.namespace);
        out.key("code");
        out.string(&self.code);
        out.key("scale");
        out.float(self.scale);
    }
}

impl FromJson for PointMarker {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointMarker {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法の種類。linear は 2 点を測る向きに、aligned は 2 点を結ぶ向きに測った長さ。angular は頂点から 2 点へ向かう線の、angular_lines は 2 本の線のなす角度。arc_length は円弧の長さ。radius は半径、diameter は直径。ordinate は原点からの X か Y の座標
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DimensionKind {
    /// `linear`
    Linear,
    /// `aligned`
    Aligned,
    /// `angular`
    Angular,
    /// `angular_lines`
    AngularLines,
    /// `arc_length`
    ArcLength,
    /// `radius`
    Radius,
    /// `diameter`
    Diameter,
    /// `ordinate`
    Ordinate,
}

impl DimensionKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Linear,
        Self::Aligned,
        Self::Angular,
        Self::AngularLines,
        Self::ArcLength,
        Self::Radius,
        Self::Diameter,
        Self::Ordinate,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Aligned => "aligned",
            Self::Angular => "angular",
            Self::AngularLines => "angular_lines",
            Self::ArcLength => "arc_length",
            Self::Radius => "radius",
            Self::Diameter => "diameter",
            Self::Ordinate => "ordinate",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "linear" => Some(Self::Linear),
            "aligned" => Some(Self::Aligned),
            "angular" => Some(Self::Angular),
            "angular_lines" => Some(Self::AngularLines),
            "arc_length" => Some(Self::ArcLength),
            "radius" => Some(Self::Radius),
            "diameter" => Some(Self::Diameter),
            "ordinate" => Some(Self::Ordinate),
            _ => None,
        }
    }
}

impl FromJson for DimensionKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for DimensionKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 座標の軸
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DimensionAxis {
    /// `x`
    X,
    /// `y`
    Y,
}

impl DimensionAxis {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::X, Self::Y];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            _ => None,
        }
    }
}

impl FromJson for DimensionAxis {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for DimensionAxis {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 寸法値の文字の書き方。measurement は測定値を書き、override は利用者が書いた文字（content）を書き、hidden は書かない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DimensionLabel {
    /// `measurement`
    Measurement,
    /// `override`
    Override,
    /// `hidden`
    Hidden,
}

impl DimensionLabel {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Measurement, Self::Override, Self::Hidden];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Measurement => "measurement",
            Self::Override => "override",
            Self::Hidden => "hidden",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "measurement" => Some(Self::Measurement),
            "override" => Some(Self::Override),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }
}

impl FromJson for DimensionLabel {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for DimensionLabel {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。マイナーバージョン 14 から
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolGeometry {
    /// 記号の定義（ブロック）の元の名前。足すときは図面にある定義の名前を指す
    pub name: String,
    /// 定義の基点を置く点 [X, Y]
    pub position: Vec<f64>,
    /// X と Y の尺度 [X, Y]。0 でない。負なら反転する
    pub scale: Vec<f64>,
    /// 回転（度）
    pub rotation: f64,
    /// 格子に並べて置く列と行。書かなければ 1 つだけ置く
    pub grid: Option<SymbolGrid>,
    /// 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。マイナーバージョン 16 から。置き換えで値を変えるのはマイナーバージョン 21 から
    pub attributes: Option<Vec<SymbolAttribute>>,
}

impl SymbolGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("shape", "symbol")?;
        let name = object.required("name", json::string)?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let scale = object.required("scale", |value| json::array(value, json::number))?;
        let rotation = object.required("rotation", json::number)?;
        let grid = object.optional("grid", SymbolGrid::from_json)?;
        let attributes = object.optional("attributes", |value| {
            json::array(value, SymbolAttribute::from_json)
        })?;
        Ok(Self {
            name,
            position,
            scale,
            rotation,
            grid,
            attributes,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("shape");
        out.string("symbol");
        out.key("name");
        out.string(&self.name);
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        out.key("scale");
        out.array(&self.scale, |out, item| out.float(*item));
        out.key("rotation");
        out.float(self.rotation);
        if let Some(value) = &self.grid {
            out.key("grid");
            value.to_json(out);
        }
        if let Some(value) = &self.attributes {
            out.key("attributes");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for SymbolGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SymbolGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 記号の属性のタグと値
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolAttribute {
    /// 元のタグ
    pub tag: String,
    /// 値。1 行
    pub value: String,
}

impl SymbolAttribute {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let tag = object.required("tag", json::string)?;
        let value = object.required("value", json::string)?;
        Ok(Self { tag, value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("tag");
        out.string(&self.tag);
        out.key("value");
        out.string(&self.value);
    }
}

impl FromJson for SymbolAttribute {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SymbolAttribute {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 記号を格子に並べる列と行。間隔は、記号を回転する前の向きに測る。置き換えるときは、列と行の数を変えない
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolGrid {
    /// 列の数。1 以上
    pub columns: u64,
    /// 行の数。1 以上。列と行がどちらも 1 の格子は持たない
    pub rows: u64,
    /// 列の間隔
    pub column_spacing: f64,
    /// 行の間隔
    pub row_spacing: f64,
}

impl SymbolGrid {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let columns = object.required("columns", |value| json::uint(value, 1, 32767))?;
        let rows = object.required("rows", |value| json::uint(value, 1, 32767))?;
        let column_spacing = object.required("column_spacing", json::number)?;
        let row_spacing = object.required("row_spacing", json::number)?;
        Ok(Self {
            columns,
            rows,
            column_spacing,
            row_spacing,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("columns");
        out.uint(self.columns);
        out.key("rows");
        out.uint(self.rows);
        out.key("column_spacing");
        out.float(self.column_spacing);
        out.key("row_spacing");
        out.float(self.row_spacing);
    }
}

impl FromJson for SymbolGrid {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SymbolGrid {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 基点に合わせる枠の位置。entities は返し、足すときに書かなければ左上、置き換えるときに書かなければ元のまま
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Attachment {
    /// `top_left`
    TopLeft,
    /// `top_center`
    TopCenter,
    /// `top_right`
    TopRight,
    /// `middle_left`
    MiddleLeft,
    /// `middle_center`
    MiddleCenter,
    /// `middle_right`
    MiddleRight,
    /// `bottom_left`
    BottomLeft,
    /// `bottom_center`
    BottomCenter,
    /// `bottom_right`
    BottomRight,
}

impl Attachment {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::MiddleLeft,
        Self::MiddleCenter,
        Self::MiddleRight,
        Self::BottomLeft,
        Self::BottomCenter,
        Self::BottomRight,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopCenter => "top_center",
            Self::TopRight => "top_right",
            Self::MiddleLeft => "middle_left",
            Self::MiddleCenter => "middle_center",
            Self::MiddleRight => "middle_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomCenter => "bottom_center",
            Self::BottomRight => "bottom_right",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "top_left" => Some(Self::TopLeft),
            "top_center" => Some(Self::TopCenter),
            "top_right" => Some(Self::TopRight),
            "middle_left" => Some(Self::MiddleLeft),
            "middle_center" => Some(Self::MiddleCenter),
            "middle_right" => Some(Self::MiddleRight),
            "bottom_left" => Some(Self::BottomLeft),
            "bottom_center" => Some(Self::BottomCenter),
            "bottom_right" => Some(Self::BottomRight),
            _ => None,
        }
    }
}

impl FromJson for Attachment {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Attachment {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 要素の形の種類（決定 0022）。point（点、決定 0160）はマイナーバージョン 32 から。それまで点は symbol だった
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    /// `line`
    Line,
    /// `circle`
    Circle,
    /// `curve`
    Curve,
    /// `polyline`
    Polyline,
    /// `text`
    Text,
    /// `formatted_text`
    FormattedText,
    /// `dimension`
    Dimension,
    /// `leader`
    Leader,
    /// `callout`
    Callout,
    /// `table`
    Table,
    /// `underlay`
    Underlay,
    /// `symbol`
    Symbol,
    /// `hatch`
    Hatch,
    /// `point`
    Point,
}

impl EntityKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Line,
        Self::Circle,
        Self::Curve,
        Self::Polyline,
        Self::Text,
        Self::FormattedText,
        Self::Dimension,
        Self::Leader,
        Self::Callout,
        Self::Table,
        Self::Underlay,
        Self::Symbol,
        Self::Hatch,
        Self::Point,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Circle => "circle",
            Self::Curve => "curve",
            Self::Polyline => "polyline",
            Self::Text => "text",
            Self::FormattedText => "formatted_text",
            Self::Dimension => "dimension",
            Self::Leader => "leader",
            Self::Callout => "callout",
            Self::Table => "table",
            Self::Underlay => "underlay",
            Self::Symbol => "symbol",
            Self::Hatch => "hatch",
            Self::Point => "point",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "line" => Some(Self::Line),
            "circle" => Some(Self::Circle),
            "curve" => Some(Self::Curve),
            "polyline" => Some(Self::Polyline),
            "text" => Some(Self::Text),
            "formatted_text" => Some(Self::FormattedText),
            "dimension" => Some(Self::Dimension),
            "leader" => Some(Self::Leader),
            "callout" => Some(Self::Callout),
            "table" => Some(Self::Table),
            "underlay" => Some(Self::Underlay),
            "symbol" => Some(Self::Symbol),
            "hatch" => Some(Self::Hatch),
            "point" => Some(Self::Point),
            _ => None,
        }
    }
}

impl FromJson for EntityKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for EntityKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。マイナーバージョン 7 から
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// 読んだときの文書のハッシュ（drawing の hash）。書けば、今の文書と同じときだけ当てる
    pub hash: Option<String>,
    /// 変更。並べた順に当てる
    pub edits: Vec<Edit>,
}

impl ApplyRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "apply")?;
        let doc = object.required("doc", json::string)?;
        let hash = object.optional("hash", json::string)?;
        let edits = object.required("edits", |value| json::array(value, Edit::from_json))?;
        Ok(Self {
            request_common,
            doc,
            hash,
            edits,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("apply");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
        out.key("edits");
        out.array(&self.edits, |out, item| item.to_json(out));
    }
}

impl FromJson for ApplyRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ApplyRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 当てた変更の結果。書き戻したファイルは blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ApplyResult {
    /// 変更ごとの結果。要求の edits と同じ順
    pub applied: Vec<Applied>,
    /// 書き戻した文書のハッシュ。次の apply の前提条件に使う
    pub hash: String,
    /// true なら、変えていない要素の ID がずれた。キーを持たない形式（JWW）か、ハンドルを持たない DXF（R12 など）で、前の図形か表のレコードを除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。マイナーバージョン 30 から
    pub ids_shifted: Option<bool>,
}

impl ApplyResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let applied = object.required("applied", |value| json::array(value, Applied::from_json))?;
        let hash = object.required("hash", json::string)?;
        let ids_shifted = object.optional("ids_shifted", json::boolean)?;
        Ok(Self {
            applied,
            hash,
            ids_shifted,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("applied");
        out.array(&self.applied, |out, item| item.to_json(out));
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.ids_shifted {
            out.key("ids_shifted");
            out.bool(*value);
        }
    }
}

impl FromJson for ApplyResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ApplyResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 入力なしで空の文書を作り、開く。初めは R2004 の DXF。既定のレイヤとスタイル、空のモデル空間と Layout1 を持つ。書いたファイルは blobs で返す。マイナーバージョン 83 から
#[derive(Clone, Debug, PartialEq)]
pub struct CreateRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 作る形式の名前。capabilities.formats の create が true の id。今は dxf
    pub format: String,
    /// 作る文書の名前。既存の文書と重ねない
    pub to: String,
    /// 実寸の空間の長さの単位。省略すると millimeter。unknown は単位なし
    pub unit: Option<Unit>,
}

impl CreateRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "create")?;
        let format = object.required("format", json::string)?;
        let to = object.required("to", json::string)?;
        let unit = object.optional("unit", Unit::from_json)?;
        Ok(Self {
            request_common,
            format,
            to,
            unit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("create");
        out.key("format");
        out.string(&self.format);
        out.key("to");
        out.string(&self.to);
        if let Some(value) = &self.unit {
            out.key("unit");
            value.to_json(out);
        }
    }
}

impl FromJson for CreateRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CreateRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 作った空の文書。書いたファイルは blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct CreateResult {
    /// 作った文書の名前
    pub doc: String,
    /// 書いた形式の名前
    pub format: String,
    /// 書いたファイルのハッシュ。apply の前提条件に使う
    pub hash: String,
    /// 書いた要素の数。空なので 0
    pub elements: u64,
}

impl CreateResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let hash = object.required("hash", json::string)?;
        let elements =
            object.required("elements", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            doc,
            format,
            hash,
            elements,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        out.key("hash");
        out.string(&self.hash);
        out.key("elements");
        out.uint(self.elements);
    }
}

impl FromJson for CreateResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CreateResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書から別の形式のファイルを作り、新しい文書として開く。図面からは DXF（対応する R2004 DWG の弧長寸法は共有オブジェクト表から、それ以外は図面モデルから。決定 0139・0206・0281）または SIMA（.sim）、土木モデル（J-LandXML・IFC4.3・SIMA）からは SIMA（.sim）、地理の領域モデルの文書（CityGML・基盤地図情報・拡張 DM・GeoJSON・SIMA・GeoTIFF）からは GeoJSON または SIMA（.sim）を書く。書けないデータは種類ごとの件数を診断で返す。書いたファイルは blobs で返す。マイナーバージョン 41 から。geojson はマイナーバージョン 71 から。sima はマイナーバージョン 72 から
#[derive(Clone, Debug, PartialEq)]
pub struct ConvertRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 書く元の文書の名前。open で作ったもの
    pub doc: String,
    /// 書く形式の名前（capabilities の formats の id）。dxf・sxf（P21）・jww・geojson・sima のいずれか。sxf・jww はマイナーバージョン 85 から
    pub format: String,
    /// 作る文書の名前
    pub to: String,
}

impl ConvertRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "convert")?;
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let to = object.required("to", json::string)?;
        Ok(Self {
            request_common,
            doc,
            format,
            to,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("convert");
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        out.key("to");
        out.string(&self.to);
    }
}

impl FromJson for ConvertRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConvertRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 書いた文書。書いたファイルは blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ConvertResult {
    /// 作った文書の名前
    pub doc: String,
    /// 書いた形式の名前
    pub format: String,
    /// 書いたファイルのハッシュ。apply の前提条件に使う
    pub hash: String,
    /// 書いた要素の数。記号から離した属性の文字を含む
    pub elements: u64,
}

impl ConvertResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let hash = object.required("hash", json::string)?;
        let elements =
            object.required("elements", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            doc,
            format,
            hash,
            elements,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        out.key("hash");
        out.string(&self.hash);
        out.key("elements");
        out.uint(self.elements);
    }
}

impl FromJson for ConvertResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConvertResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた図面を変換したときの忠実度と欠落を、convert と同じ書き手で見積もる。文書もファイルの blob も登録しない。DXF・SXF（P21）・JWW への変換に対応する。マイナーバージョン 66 から（SXF・JWW は 85 から）
#[derive(Clone, Debug, PartialEq)]
pub struct ConvertEstimateRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 書く元の文書の名前
    pub doc: String,
    /// 書く形式の名前。dxf・sxf・jww
    pub format: String,
    /// 予定する文書の名前。convert と同じ条件で検査するが、名前を確保しない
    pub to: String,
    /// 書き手と読み取り段階のそれぞれで返す明細の最大件数。省略時は 100。総数と全件集計はこの上限によらない
    pub limit: Option<u64>,
}

impl ConvertEstimateRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "convert_estimate")?;
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let to = object.required("to", json::string)?;
        let limit = object.optional("limit", |value| json::uint(value, 0, 1000))?;
        Ok(Self {
            request_common,
            doc,
            format,
            to,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("convert_estimate");
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        out.key("to");
        out.string(&self.to);
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for ConvertEstimateRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConvertEstimateRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 予定する変換の結果。元の文書・ID を保ち、ファイルの blob を返さない
#[derive(Clone, Debug, PartialEq)]
pub struct ConvertEstimateResult {
    /// 予定する出力と忠実度の全件集計・上限付き明細
    pub estimate: ConversionEstimate,
}

impl ConvertEstimateResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let estimate = object.required("estimate", ConversionEstimate::from_json)?;
        Ok(Self { estimate })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("estimate");
        self.estimate.to_json(out);
    }
}

impl FromJson for ConvertEstimateResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConvertEstimateResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 実変換と同じ処理で準備した出力。doc と hash は未登録の予定値。元の入力・設定・予算が同じなら convert の値と一致する
#[derive(Clone, Debug, PartialEq)]
pub struct ConversionEstimate {
    /// 予定する文書の名前
    pub doc: String,
    /// 予定する形式の名前
    pub format: String,
    /// 予定する出力バイト列のハッシュ。未登録なので apply の前提条件には使えない
    pub hash: String,
    /// 予定する出力の要素数。記号から離す属性の文字を含む
    pub elements: u64,
    /// 処理対象の出現数。同じ出所を持つ別の入力は別々に数え、診断の数では増やさない
    pub total: u64,
    /// 種類と忠実度ごとの全件集計
    pub counts: Vec<ConversionCount>,
    /// 処理順の明細。limit 件まで返す
    pub details: Vec<ConversionDetail>,
    /// 明細の上限で返さなかった対象がある
    pub truncated: bool,
    /// 入力の表の読み取りと lift 中の、集約前の診断発生記録。書き手の項目の集計には含めない。マイナーバージョン 67 から
    pub input_diagnostics: Option<ConversionInputDiagnostics>,
}

impl ConversionEstimate {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let hash = object.required("hash", json::string)?;
        let elements =
            object.required("elements", |value| json::uint(value, 0, 9007199254740991))?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let counts = object.required("counts", |value| {
            json::array(value, ConversionCount::from_json)
        })?;
        let details = object.required("details", |value| {
            json::array(value, ConversionDetail::from_json)
        })?;
        let truncated = object.required("truncated", json::boolean)?;
        let input_diagnostics =
            object.optional("input_diagnostics", ConversionInputDiagnostics::from_json)?;
        Ok(Self {
            doc,
            format,
            hash,
            elements,
            total,
            counts,
            details,
            truncated,
            input_diagnostics,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        out.key("hash");
        out.string(&self.hash);
        out.key("elements");
        out.uint(self.elements);
        out.key("total");
        out.uint(self.total);
        out.key("counts");
        out.array(&self.counts, |out, item| item.to_json(out));
        out.key("details");
        out.array(&self.details, |out, item| item.to_json(out));
        out.key("truncated");
        out.bool(self.truncated);
        if let Some(value) = &self.input_diagnostics {
            out.key("input_diagnostics");
            value.to_json(out);
        }
    }
}

impl FromJson for ConversionEstimate {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConversionEstimate {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 読み取り段階の診断発生数。失った要素数ではなく、同じ要素の複数の診断もそれぞれ数える。忠実度を持たない診断も含む。open 時や書き手の診断は含まない
#[derive(Clone, Debug, PartialEq)]
pub struct ConversionInputDiagnostics {
    /// 診断の発生件数の合計。max_places と limit によらない
    pub total: u64,
    /// 診断の code とカタログの group_by の値による全件集計。場所は持たず、値は最も早い入力位置の報告から取る
    pub counts: Vec<Diagnostic>,
    /// 集約前の報告を入力位置順に limit 件まで返す。同じ位置は報告順。診断の場所は max_places で制限しない
    pub details: Vec<ConversionInputDiagnostic>,
    /// 明細の上限で返さなかった報告がある
    pub truncated: bool,
}

impl ConversionInputDiagnostics {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let counts =
            object.required("counts", |value| json::array(value, Diagnostic::from_json))?;
        let details = object.required("details", |value| {
            json::array(value, ConversionInputDiagnostic::from_json)
        })?;
        let truncated = object.required("truncated", json::boolean)?;
        Ok(Self {
            total,
            counts,
            details,
            truncated,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("total");
        out.uint(self.total);
        out.key("counts");
        out.array(&self.counts, |out, item| item.to_json(out));
        out.key("details");
        out.array(&self.details, |out, item| item.to_json(out));
        out.key("truncated");
        out.bool(self.truncated);
    }
}

impl FromJson for ConversionInputDiagnostics {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConversionInputDiagnostics {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 集約前の診断と、その場所と同じ順序の出所。種類や失った値は診断の code と params からたどる
#[derive(Clone, Debug, PartialEq)]
pub struct ConversionInputDiagnostic {
    /// 集約前の診断。元の値と場所を保つ
    pub diagnostic: Diagnostic,
    /// diagnostic.at と同じ順序のレコード・バイト範囲・ハンドル・インスタンス番号。ID は diagnostic.at にある
    pub sources: Vec<Source>,
}

impl ConversionInputDiagnostic {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let diagnostic = object.required("diagnostic", Diagnostic::from_json)?;
        let sources = object.required("sources", |value| json::array(value, Source::from_json))?;
        Ok(Self {
            diagnostic,
            sources,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("diagnostic");
        self.diagnostic.to_json(out);
        out.key("sources");
        out.array(&self.sources, |out, item| item.to_json(out));
    }
}

impl FromJson for ConversionInputDiagnostic {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConversionInputDiagnostic {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 処理対象の種類と忠実度ごとの集計
#[derive(Clone, Debug, PartialEq)]
pub struct ConversionCount {
    /// 処理対象の種類のキー
    pub kind: String,
    /// その対象を移した結果の忠実度
    pub fidelity: ConversionFidelity,
    /// 入力の出現数。1 つの入力が複数の出力になっても 1 件
    pub count: u64,
    /// この対象から書いた図形の数。表や参照だけを移した場合は 0
    pub output_elements: u64,
}

impl ConversionCount {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", json::string)?;
        let fidelity = object.required("fidelity", ConversionFidelity::from_json)?;
        let count = object.required("count", |value| json::uint(value, 0, 9007199254740991))?;
        let output_elements = object.required("output_elements", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            kind,
            fidelity,
            count,
            output_elements,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string(&self.kind);
        out.key("fidelity");
        self.fidelity.to_json(out);
        out.key("count");
        out.uint(self.count);
        out.key("output_elements");
        out.uint(self.output_elements);
    }
}

impl FromJson for ConversionCount {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConversionCount {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 処理対象の出現 1 つ。複数の欠落の診断が出ても 1 件として数える
#[derive(Clone, Debug, PartialEq)]
pub struct ConversionDetail {
    /// 処理対象の種類のキー
    pub kind: String,
    /// その対象を移した結果の忠実度
    pub fidelity: ConversionFidelity,
    /// 値や参照を変えた理由のキー。text_height_meaning は文字高の意味、relocated_shape は形の値、style_default はスタイルの既定値、detached_attribute は属性の文字への分離、baked_view はビューの焼き込み、reference_dropped は参照の欠落
    pub reasons: Vec<String>,
    /// 元の要素の ID。持つ対象だけで返す
    pub id: Option<String>,
    /// 元の対象のレコード・バイト範囲・キー
    pub source: Source,
    /// この対象から書いた図形の数。表や参照だけを移した場合は 0
    pub output_elements: u64,
}

impl ConversionDetail {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", json::string)?;
        let fidelity = object.required("fidelity", ConversionFidelity::from_json)?;
        let reasons = object.required("reasons", |value| json::array(value, json::string))?;
        let id = object.optional("id", json::string)?;
        let source = object.required("source", Source::from_json)?;
        let output_elements = object.required("output_elements", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            kind,
            fidelity,
            reasons,
            id,
            source,
            output_elements,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string(&self.kind);
        out.key("fidelity");
        self.fidelity.to_json(out);
        out.key("reasons");
        out.array(&self.reasons, |out, item| out.string(item));
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        out.key("source");
        self.source.to_json(out);
        out.key("output_elements");
        out.uint(self.output_elements);
    }
}

impl FromJson for ConversionDetail {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConversionDetail {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 忠実度。`native` は元の値・参照を保持、`equivalent` は別の表現で同じ意味、`approximated` は近似、`drawn` は見た目だけ、`dropped` は欠落
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConversionFidelity {
    /// `native`
    Native,
    /// `equivalent`
    Equivalent,
    /// `approximated`
    Approximated,
    /// `drawn`
    Drawn,
    /// `dropped`
    Dropped,
}

impl ConversionFidelity {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Native,
        Self::Equivalent,
        Self::Approximated,
        Self::Drawn,
        Self::Dropped,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Equivalent => "equivalent",
            Self::Approximated => "approximated",
            Self::Drawn => "drawn",
            Self::Dropped => "dropped",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "native" => Some(Self::Native),
            "equivalent" => Some(Self::Equivalent),
            "approximated" => Some(Self::Approximated),
            "drawn" => Some(Self::Drawn),
            "dropped" => Some(Self::Dropped),
            _ => None,
        }
    }
}

impl FromJson for ConversionFidelity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for ConversionFidelity {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 当てた変更 1 つの結果
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    /// 変えた要素（レイヤの変更では、そのレイヤ）の、書き戻した後の ID。除いたときと、ID を決められないときは持たない
    pub id: Option<String>,
    /// 変えた要素（レイヤ）の、書き戻した後のハッシュ。id があるときに持つ
    pub hash: Option<String>,
}

impl Applied {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.optional("id", json::string)?;
        let hash = object.optional("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
    }
}

impl FromJson for Applied {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Applied {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素 1 つへの変更。kind の値で形が決まる
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Edit {
    /// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない。RGB の色は、DXF と DWG の R2004 から、線幅は R2000 から書ける（DWG の今ある図形のレイヤ・線種・線幅・RGB の色は、決定 0293 から変えられる）
    Restyle(RestyleEdit),
    /// 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
    Transform(TransformEdit),
    /// 要素を除く
    Remove(RemoveEdit),
    /// 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤと線のスタイルは変えない。DXF・DWG の文字と書式つきの文字は、定義のある文字スタイルへ指し替えられる。マイナーバージョン 10 から
    Replace(ReplaceEdit),
    /// 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。JWW と SXF の図形は色・線種・線幅を自分で持ち、レイヤかブロックに従う値を持たないので、JWW は色と線種を書き（点は色だけ）、SXF は色・線種・線幅を書く（点マーカーは色だけ）。書かないか、レイヤかブロックに従う値なら change.style.unsupported で断る。JWW の書かないかレイヤに従う線幅は、ペンの印刷の線幅に従う。足せる形は、形の値のどの種類も。マイナーバージョン 10 から
    Add(AddEdit),
    /// レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG の真の色は R2004 から書ける（今あるレイヤの線種と真の色は、決定 0293 から変えられる）。マイナーバージョン 22 から
    Layer(LayerEdit),
    /// レイヤを足す（決定 0162）。書かないフィールドは、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の後ろの要素の変更から、足したレイヤを名前で指せる。マイナーバージョン 22 から
    AddLayer(AddLayerEdit),
    /// 記号の定義（DXF・DWG のブロック）を足す（#684）。中の要素は、定義の空間に足す。足した定義は、同じ apply の後ろの add の記号から名前で指せる。属性の定義・名前のない定義・外部参照は足せない。結果は id と hash を持たない。足した定義は tables の symbols から名前で引く。マイナーバージョン 99 から
    AddSymbol(AddSymbolEdit),
    /// レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。マイナーバージョン 24 から
    RemoveLayer(RemoveLayerEdit),
    /// 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。マイナーバージョン 25 から
    TextStyle(TextStyleEdit),
    /// 文字スタイルを足す（決定 0163）。書かないフィールドは Standard と同じ。同じ apply の後ろの文字の追加・置き換えから、足した文字スタイルを名前で指せる。マイナーバージョン 25 から
    AddTextStyle(AddTextStyleEdit),
    /// 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 25 から
    RemoveTextStyle(RemoveTextStyleEdit),
    /// 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。マルチラインスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。マイナーバージョン 26 から
    LineType(LineTypeEdit),
    /// 線種を足す（決定 0164）。書かないフィールドは Continuous と同じ。同じ apply の後ろの要素とレイヤの変更から、足した線種を名前で指せる。マイナーバージョン 26 から
    AddLineType(AddLineTypeEdit),
    /// 線種を除く（決定 0164）。要素・レイヤ・マルチラインスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。マイナーバージョン 26 から
    RemoveLineType(RemoveLineTypeEdit),
    /// 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。マイナーバージョン 27 から
    DimensionStyle(DimensionStyleEdit),
    /// 寸法スタイルを足す（決定 0165）。書かないフィールドは Standard と同じ。同じ apply の後ろの要素の追加から、足した寸法スタイルを名前で指せる。マイナーバージョン 27 から
    AddDimensionStyle(AddDimensionStyleEdit),
    /// 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 27 から
    RemoveDimensionStyle(RemoveDimensionStyleEdit),
    /// 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG はヘッダーの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、マイナーバージョン 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。マイナーバージョン 38 から
    PointStyle(PointStyleEdit),
}

impl Edit {
    /// タグのフィールド（`kind`）の値（スキーマの順）
    pub const TAGS: &[&str] = &[
        "restyle",
        "transform",
        "remove",
        "replace",
        "add",
        "layer",
        "add_layer",
        "add_symbol",
        "remove_layer",
        "text_style",
        "add_text_style",
        "remove_text_style",
        "line_type",
        "add_line_type",
        "remove_line_type",
        "dimension_style",
        "add_dimension_style",
        "remove_dimension_style",
        "point_style",
    ];

    /// タグのフィールド（`kind`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Restyle(_) => "restyle",
            Self::Transform(_) => "transform",
            Self::Remove(_) => "remove",
            Self::Replace(_) => "replace",
            Self::Add(_) => "add",
            Self::Layer(_) => "layer",
            Self::AddLayer(_) => "add_layer",
            Self::AddSymbol(_) => "add_symbol",
            Self::RemoveLayer(_) => "remove_layer",
            Self::TextStyle(_) => "text_style",
            Self::AddTextStyle(_) => "add_text_style",
            Self::RemoveTextStyle(_) => "remove_text_style",
            Self::LineType(_) => "line_type",
            Self::AddLineType(_) => "add_line_type",
            Self::RemoveLineType(_) => "remove_line_type",
            Self::DimensionStyle(_) => "dimension_style",
            Self::AddDimensionStyle(_) => "add_dimension_style",
            Self::RemoveDimensionStyle(_) => "remove_dimension_style",
            Self::PointStyle(_) => "point_style",
        }
    }
}

impl FromJson for Edit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("kind")? {
            "restyle" => Self::Restyle(RestyleEdit::read_fields(&mut object)?),
            "transform" => Self::Transform(TransformEdit::read_fields(&mut object)?),
            "remove" => Self::Remove(RemoveEdit::read_fields(&mut object)?),
            "replace" => Self::Replace(ReplaceEdit::read_fields(&mut object)?),
            "add" => Self::Add(AddEdit::read_fields(&mut object)?),
            "layer" => Self::Layer(LayerEdit::read_fields(&mut object)?),
            "add_layer" => Self::AddLayer(AddLayerEdit::read_fields(&mut object)?),
            "add_symbol" => Self::AddSymbol(AddSymbolEdit::read_fields(&mut object)?),
            "remove_layer" => Self::RemoveLayer(RemoveLayerEdit::read_fields(&mut object)?),
            "text_style" => Self::TextStyle(TextStyleEdit::read_fields(&mut object)?),
            "add_text_style" => Self::AddTextStyle(AddTextStyleEdit::read_fields(&mut object)?),
            "remove_text_style" => {
                Self::RemoveTextStyle(RemoveTextStyleEdit::read_fields(&mut object)?)
            }
            "line_type" => Self::LineType(LineTypeEdit::read_fields(&mut object)?),
            "add_line_type" => Self::AddLineType(AddLineTypeEdit::read_fields(&mut object)?),
            "remove_line_type" => {
                Self::RemoveLineType(RemoveLineTypeEdit::read_fields(&mut object)?)
            }
            "dimension_style" => {
                Self::DimensionStyle(DimensionStyleEdit::read_fields(&mut object)?)
            }
            "add_dimension_style" => {
                Self::AddDimensionStyle(AddDimensionStyleEdit::read_fields(&mut object)?)
            }
            "remove_dimension_style" => {
                Self::RemoveDimensionStyle(RemoveDimensionStyleEdit::read_fields(&mut object)?)
            }
            "point_style" => Self::PointStyle(PointStyleEdit::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("kind")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Edit {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Restyle(value) => value.to_json(out),
            Self::Transform(value) => value.to_json(out),
            Self::Remove(value) => value.to_json(out),
            Self::Replace(value) => value.to_json(out),
            Self::Add(value) => value.to_json(out),
            Self::Layer(value) => value.to_json(out),
            Self::AddLayer(value) => value.to_json(out),
            Self::AddSymbol(value) => value.to_json(out),
            Self::RemoveLayer(value) => value.to_json(out),
            Self::TextStyle(value) => value.to_json(out),
            Self::AddTextStyle(value) => value.to_json(out),
            Self::RemoveTextStyle(value) => value.to_json(out),
            Self::LineType(value) => value.to_json(out),
            Self::AddLineType(value) => value.to_json(out),
            Self::RemoveLineType(value) => value.to_json(out),
            Self::DimensionStyle(value) => value.to_json(out),
            Self::AddDimensionStyle(value) => value.to_json(out),
            Self::RemoveDimensionStyle(value) => value.to_json(out),
            Self::PointStyle(value) => value.to_json(out),
        }
    }
}

/// 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない。RGB の色は、DXF と DWG の R2004 から、線幅は R2000 から書ける（DWG の今ある図形のレイヤ・線種・線幅・RGB の色は、決定 0293 から変えられる）
#[derive(Clone, Debug, PartialEq)]
pub struct RestyleEdit {
    /// 変える要素の ID。entities が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。entities が返したもの
    pub hash: String,
    /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない
    pub layer: Option<String>,
    /// レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。マイナーバージョン 8 から
    pub layer_id: Option<String>,
    /// 色
    pub color: Option<ColorChange>,
    /// 線種
    pub line_type: Option<LineTypeChange>,
    /// 要素の線種の倍率
    pub line_type_scale: Option<f64>,
    /// 線幅
    pub line_weight: Option<LineWeightChange>,
}

impl RestyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "restyle")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let layer = object.optional("layer", json::string)?;
        let layer_id = object.optional("layer_id", json::string)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type = object.optional("line_type", LineTypeChange::from_json)?;
        let line_type_scale = object.optional("line_type_scale", json::number)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        Ok(Self {
            id,
            hash,
            layer,
            layer_id,
            color,
            line_type,
            line_type_scale,
            line_weight,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("restyle");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.layer {
            out.key("layer");
            out.string(value);
        }
        if let Some(value) = &self.layer_id {
            out.key("layer_id");
            out.string(value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type {
            out.key("line_type");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type_scale {
            out.key("line_type_scale");
            out.float(*value);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
    }
}

impl FromJson for RestyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RestyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない
#[derive(Clone, Debug, PartialEq)]
pub struct TransformEdit {
    /// 動かす要素の ID。entities が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。entities が返したもの
    pub hash: String,
    /// [a, b, c, d, e, f]。要素の空間か用紙の点 (x, y) を (a x + b y + c, d x + e y + f) に写す
    pub matrix: Vec<f64>,
}

impl TransformEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "transform")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let matrix = object.required("matrix", |value| json::array(value, json::number))?;
        Ok(Self { id, hash, matrix })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("transform");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        out.key("matrix");
        out.array(&self.matrix, |out, item| out.float(*item));
    }
}

impl FromJson for TransformEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TransformEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素を除く
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveEdit {
    /// 除く要素の ID。entities が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。entities が返したもの
    pub hash: String,
}

impl RemoveEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤと線のスタイルは変えない。DXF・DWG の文字と書式つきの文字は、定義のある文字スタイルへ指し替えられる。マイナーバージョン 10 から
#[derive(Clone, Debug, PartialEq)]
pub struct ReplaceEdit {
    /// 置き換える要素の ID。entities が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。entities が返したもの
    pub hash: String,
    /// 新しい形。entities の geometry と同じ種類
    pub geometry: Geometry,
}

impl ReplaceEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "replace")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let geometry = object.required("geometry", Geometry::from_json)?;
        Ok(Self { id, hash, geometry })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("replace");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        out.key("geometry");
        self.geometry.to_json(out);
    }
}

impl FromJson for ReplaceEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReplaceEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。JWW と SXF の図形は色・線種・線幅を自分で持ち、レイヤかブロックに従う値を持たないので、JWW は色と線種を書き（点は色だけ）、SXF は色・線種・線幅を書く（点マーカーは色だけ）。書かないか、レイヤかブロックに従う値なら change.style.unsupported で断る。JWW の書かないかレイヤに従う線幅は、ペンの印刷の線幅に従う。足せる形は、形の値のどの種類も。マイナーバージョン 10 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddEdit {
    /// 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。マイナーバージョン 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177）
    pub place: String,
    /// レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
    pub layer: Option<String>,
    /// レイヤの ID。tables が返したもの
    pub layer_id: Option<String>,
    /// 色。SXF と JWW はレイヤに従う色を書けないので、書く。SXF は、sxf の番号か、利用者定義色と同じ RGB を受ける。JWW は、jww の番号（0〜9 と、バージョン 420 からの拡張色 100〜356）か、ペン 1〜9 と、バージョン 420 からの 100〜356 のどれかと同じ RGB を受ける。書かないか、受けられない色なら断る。マイナーバージョン 29 から
    pub color: Option<ColorChange>,
    /// 線種。マイナーバージョン 29 から
    pub line_type: Option<LineTypeChange>,
    /// 線幅。マイナーバージョン 29 から
    pub line_weight: Option<LineWeightChange>,
    /// 足す形
    pub geometry: Geometry,
}

impl AddEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add")?;
        let place = object.required("place", json::string)?;
        let layer = object.optional("layer", json::string)?;
        let layer_id = object.optional("layer_id", json::string)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type = object.optional("line_type", LineTypeChange::from_json)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        let geometry = object.required("geometry", Geometry::from_json)?;
        Ok(Self {
            place,
            layer,
            layer_id,
            color,
            line_type,
            line_weight,
            geometry,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add");
        out.key("place");
        out.string(&self.place);
        if let Some(value) = &self.layer {
            out.key("layer");
            out.string(value);
        }
        if let Some(value) = &self.layer_id {
            out.key("layer_id");
            out.string(value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type {
            out.key("line_type");
            value.to_json(out);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
        out.key("geometry");
        self.geometry.to_json(out);
    }
}

impl FromJson for AddEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG の真の色は R2004 から書ける（今あるレイヤの線種と真の色は、決定 0293 から変えられる）。マイナーバージョン 22 から
#[derive(Clone, Debug, PartialEq)]
pub struct LayerEdit {
    /// 変えるレイヤの ID。tables が返したもの
    pub id: String,
    /// 読んだときのレイヤのハッシュ。tables が返したもの
    pub hash: String,
    /// 新しい名前。ほかのレイヤと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。0 と Defpoints の名前は変えられない
    pub name: Option<String>,
    /// 表示する。凍結するなら偽
    pub visible: Option<bool>,
    /// 印刷する
    pub printable: Option<bool>,
    /// 編集できる（ロックしない）
    pub editable: Option<bool>,
    /// 凍結する
    pub frozen: Option<bool>,
    /// 色。aci が要る。inherit は書けない
    pub color: Option<ColorChange>,
    /// 線種。name か id で指す。inherit は書けない
    pub line_type: Option<LineTypeChange>,
    /// 線幅。mm か default。inherit は書けない
    pub line_weight: Option<LineWeightChange>,
}

impl LayerEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "layer")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let name = object.optional("name", json::string)?;
        let visible = object.optional("visible", json::boolean)?;
        let printable = object.optional("printable", json::boolean)?;
        let editable = object.optional("editable", json::boolean)?;
        let frozen = object.optional("frozen", json::boolean)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type = object.optional("line_type", LineTypeChange::from_json)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        Ok(Self {
            id,
            hash,
            name,
            visible,
            printable,
            editable,
            frozen,
            color,
            line_type,
            line_weight,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("layer");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.visible {
            out.key("visible");
            out.bool(*value);
        }
        if let Some(value) = &self.printable {
            out.key("printable");
            out.bool(*value);
        }
        if let Some(value) = &self.editable {
            out.key("editable");
            out.bool(*value);
        }
        if let Some(value) = &self.frozen {
            out.key("frozen");
            out.bool(*value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type {
            out.key("line_type");
            value.to_json(out);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
    }
}

impl FromJson for LayerEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LayerEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。マイナーバージョン 24 から
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveLayerEdit {
    /// 除くレイヤの ID。tables が返したもの
    pub id: String,
    /// 読んだときのレイヤのハッシュ。tables が返したもの
    pub hash: String,
}

impl RemoveLayerEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove_layer")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove_layer");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveLayerEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveLayerEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。マイナーバージョン 25 から
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyleEdit {
    /// 変える文字スタイルの ID。tables が返したもの
    pub id: String,
    /// 読んだときの文字スタイルのハッシュ。tables が返したもの
    pub hash: String,
    /// 新しい名前。ほかの文字スタイルと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。Standard の名前は変えられない
    pub name: Option<String>,
    /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
    pub font: Option<Font>,
    /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
    pub big_font: Option<Font>,
    /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
    pub height: Option<f64>,
    /// 幅の倍率。正の数
    pub width: Option<f64>,
    /// 傾き（度）。-85 から 85 まで
    pub oblique: Option<f64>,
    /// 左右を裏返す
    pub backward: Option<bool>,
    /// 上下を反転する
    pub upside_down: Option<bool>,
    /// 縦書き
    pub vertical: Option<bool>,
}

impl TextStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "text_style")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let name = object.optional("name", json::string)?;
        let font = object.optional("font", Font::from_json)?;
        let big_font = object.optional("big_font", Font::from_json)?;
        let height = object.optional("height", json::number)?;
        let width = object.optional("width", json::number)?;
        let oblique = object.optional("oblique", json::number)?;
        let backward = object.optional("backward", json::boolean)?;
        let upside_down = object.optional("upside_down", json::boolean)?;
        let vertical = object.optional("vertical", json::boolean)?;
        Ok(Self {
            id,
            hash,
            name,
            font,
            big_font,
            height,
            width,
            oblique,
            backward,
            upside_down,
            vertical,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("text_style");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.font {
            out.key("font");
            value.to_json(out);
        }
        if let Some(value) = &self.big_font {
            out.key("big_font");
            value.to_json(out);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.oblique {
            out.key("oblique");
            out.float(*value);
        }
        if let Some(value) = &self.backward {
            out.key("backward");
            out.bool(*value);
        }
        if let Some(value) = &self.upside_down {
            out.key("upside_down");
            out.bool(*value);
        }
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.bool(*value);
        }
    }
}

impl FromJson for TextStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TextStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文字スタイルを足す（決定 0163）。書かないフィールドは Standard と同じ。同じ apply の後ろの文字の追加・置き換えから、足した文字スタイルを名前で指せる。マイナーバージョン 25 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddTextStyleEdit {
    /// 名前。text_style の name と同じ決まり
    pub name: String,
    /// 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない
    pub font: Option<Font>,
    /// 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く
    pub big_font: Option<Font>,
    /// 決まった高さ。文書の単位。0 なら決めない。負の数は書けない
    pub height: Option<f64>,
    /// 幅の倍率。正の数
    pub width: Option<f64>,
    /// 傾き（度）。-85 から 85 まで
    pub oblique: Option<f64>,
    /// 左右を裏返す
    pub backward: Option<bool>,
    /// 上下を反転する
    pub upside_down: Option<bool>,
    /// 縦書き
    pub vertical: Option<bool>,
}

impl AddTextStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_text_style")?;
        let name = object.required("name", json::string)?;
        let font = object.optional("font", Font::from_json)?;
        let big_font = object.optional("big_font", Font::from_json)?;
        let height = object.optional("height", json::number)?;
        let width = object.optional("width", json::number)?;
        let oblique = object.optional("oblique", json::number)?;
        let backward = object.optional("backward", json::boolean)?;
        let upside_down = object.optional("upside_down", json::boolean)?;
        let vertical = object.optional("vertical", json::boolean)?;
        Ok(Self {
            name,
            font,
            big_font,
            height,
            width,
            oblique,
            backward,
            upside_down,
            vertical,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_text_style");
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.font {
            out.key("font");
            value.to_json(out);
        }
        if let Some(value) = &self.big_font {
            out.key("big_font");
            value.to_json(out);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.oblique {
            out.key("oblique");
            out.float(*value);
        }
        if let Some(value) = &self.backward {
            out.key("backward");
            out.bool(*value);
        }
        if let Some(value) = &self.upside_down {
            out.key("upside_down");
            out.bool(*value);
        }
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.bool(*value);
        }
    }
}

impl FromJson for AddTextStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddTextStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 25 から
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveTextStyleEdit {
    /// 除く文字スタイルの ID。tables が返したもの
    pub id: String,
    /// 読んだときの文字スタイルのハッシュ。tables が返したもの
    pub hash: String,
}

impl RemoveTextStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove_text_style")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove_text_style");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveTextStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveTextStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。マルチラインスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。マイナーバージョン 26 から
#[derive(Clone, Debug, PartialEq)]
pub struct LineTypeEdit {
    /// 変える線種の ID。tables が返したもの
    pub id: String,
    /// 読んだときの線種のハッシュ。tables が返したもの
    pub hash: String,
    /// 新しい名前。ほかの線種と重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。ByLayer・ByBlock・Continuous と外部参照に依る線種の名前は変えられない
    pub name: Option<String>,
    /// 説明
    pub description: Option<String>,
    /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
    pub pattern: Option<Vec<f64>>,
}

impl LineTypeEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "line_type")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let name = object.optional("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let pattern = object.optional("pattern", |value| json::array(value, json::number))?;
        Ok(Self {
            id,
            hash,
            name,
            description,
            pattern,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("line_type");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        if let Some(value) = &self.pattern {
            out.key("pattern");
            out.array(value, |out, item| out.float(*item));
        }
    }
}

impl FromJson for LineTypeEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineTypeEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種を足す（決定 0164）。書かないフィールドは Continuous と同じ。同じ apply の後ろの要素とレイヤの変更から、足した線種を名前で指せる。マイナーバージョン 26 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddLineTypeEdit {
    /// 名前。line_type の name と同じ決まり
    pub name: String,
    /// 説明
    pub description: Option<String>,
    /// 線と間の長さの列。文書の単位。正は線、負は間、0 は点
    pub pattern: Option<Vec<f64>>,
}

impl AddLineTypeEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_line_type")?;
        let name = object.required("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let pattern = object.optional("pattern", |value| json::array(value, json::number))?;
        Ok(Self {
            name,
            description,
            pattern,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_line_type");
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        if let Some(value) = &self.pattern {
            out.key("pattern");
            out.array(value, |out, item| out.float(*item));
        }
    }
}

impl FromJson for AddLineTypeEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddLineTypeEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種を除く（決定 0164）。要素・レイヤ・マルチラインスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。マイナーバージョン 26 から
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveLineTypeEdit {
    /// 除く線種の ID。tables が返したもの
    pub id: String,
    /// 読んだときの線種のハッシュ。tables が返したもの
    pub hash: String,
}

impl RemoveLineTypeEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove_line_type")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove_line_type");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveLineTypeEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveLineTypeEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。マイナーバージョン 27 から
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionStyleEdit {
    /// 変える寸法スタイルの ID。tables が返したもの
    pub id: String,
    /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
    pub hash: String,
    /// 新しい名前。ほかの寸法スタイルと重ならず（大文字と小文字を区別しない）、$ と < > / \ " : ; ? * | = ` を含まないもの。Standard・子の寸法スタイル（名前に $ を持つもの）・子を持つ寸法スタイル・外部参照に依る寸法スタイルの名前は変えられない
    pub name: Option<String>,
    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
    pub ends: Option<Vec<String>>,
    /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
    pub leader: Option<String>,
    /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
    pub size: Option<f64>,
    /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
    pub tick: Option<f64>,
    /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
    pub scale: Option<f64>,
}

impl DimensionStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "dimension_style")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let name = object.optional("name", json::string)?;
        let ends = object.optional("ends", |value| json::array(value, json::string))?;
        let leader = object.optional("leader", json::string)?;
        let size = object.optional("size", json::number)?;
        let tick = object.optional("tick", json::number)?;
        let scale = object.optional("scale", json::number)?;
        Ok(Self {
            id,
            hash,
            name,
            ends,
            leader,
            size,
            tick,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("dimension_style");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.ends {
            out.key("ends");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.leader {
            out.key("leader");
            out.string(value);
        }
        if let Some(value) = &self.size {
            out.key("size");
            out.float(*value);
        }
        if let Some(value) = &self.tick {
            out.key("tick");
            out.float(*value);
        }
        if let Some(value) = &self.scale {
            out.key("scale");
            out.float(*value);
        }
    }
}

impl FromJson for DimensionStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DimensionStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法スタイルを足す（決定 0165）。書かないフィールドは Standard と同じ。同じ apply の後ろの要素の追加から、足した寸法スタイルを名前で指せる。マイナーバージョン 27 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddDimensionStyleEdit {
    /// 名前。dimension_style の name と同じ決まり
    pub name: String,
    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない
    pub ends: Option<Vec<String>>,
    /// 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり
    pub leader: Option<String>,
    /// 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない
    pub size: Option<f64>,
    /// 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない
    pub tick: Option<f64>,
    /// 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない
    pub scale: Option<f64>,
}

impl AddDimensionStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_dimension_style")?;
        let name = object.required("name", json::string)?;
        let ends = object.optional("ends", |value| json::array(value, json::string))?;
        let leader = object.optional("leader", json::string)?;
        let size = object.optional("size", json::number)?;
        let tick = object.optional("tick", json::number)?;
        let scale = object.optional("scale", json::number)?;
        Ok(Self {
            name,
            ends,
            leader,
            size,
            tick,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_dimension_style");
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.ends {
            out.key("ends");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.leader {
            out.key("leader");
            out.string(value);
        }
        if let Some(value) = &self.size {
            out.key("size");
            out.float(*value);
        }
        if let Some(value) = &self.tick {
            out.key("tick");
            out.float(*value);
        }
        if let Some(value) = &self.scale {
            out.key("scale");
            out.float(*value);
        }
    }
}

impl FromJson for AddDimensionStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddDimensionStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。マイナーバージョン 27 から
#[derive(Clone, Debug, PartialEq)]
pub struct RemoveDimensionStyleEdit {
    /// 除く寸法スタイルの ID。tables が返したもの
    pub id: String,
    /// 読んだときの寸法スタイルのハッシュ。tables が返したもの
    pub hash: String,
}

impl RemoveDimensionStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove_dimension_style")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove_dimension_style");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for RemoveDimensionStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RemoveDimensionStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG はヘッダーの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、マイナーバージョン 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。マイナーバージョン 38 から
#[derive(Clone, Debug, PartialEq)]
pub struct PointStyleEdit {
    /// 基本の形
    pub mark: PointMark,
    /// 円で囲む。書かなければ偽
    pub circle: Option<bool>,
    /// 四角で囲む。書かなければ偽
    pub square: Option<bool>,
    /// 大きさ。0 より大きい有限の数。意味は size_kind で決まる
    pub size: f64,
    /// 大きさの意味
    pub size_kind: PointSizeKind,
}

impl PointStyleEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "point_style")?;
        let mark = object.required("mark", PointMark::from_json)?;
        let circle = object.optional("circle", json::boolean)?;
        let square = object.optional("square", json::boolean)?;
        let size = object.required("size", json::number)?;
        let size_kind = object.required("size_kind", PointSizeKind::from_json)?;
        Ok(Self {
            mark,
            circle,
            square,
            size,
            size_kind,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("point_style");
        out.key("mark");
        self.mark.to_json(out);
        if let Some(value) = &self.circle {
            out.key("circle");
            out.bool(*value);
        }
        if let Some(value) = &self.square {
            out.key("square");
            out.bool(*value);
        }
        out.key("size");
        out.float(self.size);
        out.key("size_kind");
        self.size_kind.to_json(out);
    }
}

impl FromJson for PointStyleEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointStyleEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文書の点の基本の形。dot は小さな点、none はなし、plus は十字、cross はバツ、tick は上向きの短い線
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PointMark {
    /// `dot`
    Dot,
    /// `none`
    None,
    /// `plus`
    Plus,
    /// `cross`
    Cross,
    /// `tick`
    Tick,
}

impl PointMark {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Dot, Self::None, Self::Plus, Self::Cross, Self::Tick];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dot => "dot",
            Self::None => "none",
            Self::Plus => "plus",
            Self::Cross => "cross",
            Self::Tick => "tick",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "dot" => Some(Self::Dot),
            "none" => Some(Self::None),
            "plus" => Some(Self::Plus),
            "cross" => Some(Self::Cross),
            "tick" => Some(Self::Tick),
            _ => None,
        }
    }
}

impl FromJson for PointMark {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for PointMark {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 文書の点の大きさの意味。length は点のある空間の長さ、screen は画面の高さに対する百分率
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PointSizeKind {
    /// `length`
    Length,
    /// `screen`
    Screen,
}

impl PointSizeKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Length, Self::Screen];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Screen => "screen",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "length" => Some(Self::Length),
            "screen" => Some(Self::Screen),
            _ => None,
        }
    }
}

impl FromJson for PointSizeKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for PointSizeKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// レイヤを足す（決定 0162）。書かないフィールドは、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の後ろの要素の変更から、足したレイヤを名前で指せる。マイナーバージョン 22 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddLayerEdit {
    /// 名前。layer の name と同じ決まり
    pub name: String,
    /// 表示する。凍結するなら偽
    pub visible: Option<bool>,
    /// 印刷する
    pub printable: Option<bool>,
    /// 編集できる（ロックしない）
    pub editable: Option<bool>,
    /// 凍結する
    pub frozen: Option<bool>,
    /// 色。aci が要る。inherit は書けない
    pub color: Option<ColorChange>,
    /// 線種。name か id で指す。inherit は書けない
    pub line_type: Option<LineTypeChange>,
    /// 線幅。mm か default。inherit は書けない
    pub line_weight: Option<LineWeightChange>,
}

impl AddLayerEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_layer")?;
        let name = object.required("name", json::string)?;
        let visible = object.optional("visible", json::boolean)?;
        let printable = object.optional("printable", json::boolean)?;
        let editable = object.optional("editable", json::boolean)?;
        let frozen = object.optional("frozen", json::boolean)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type = object.optional("line_type", LineTypeChange::from_json)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        Ok(Self {
            name,
            visible,
            printable,
            editable,
            frozen,
            color,
            line_type,
            line_weight,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_layer");
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.visible {
            out.key("visible");
            out.bool(*value);
        }
        if let Some(value) = &self.printable {
            out.key("printable");
            out.bool(*value);
        }
        if let Some(value) = &self.editable {
            out.key("editable");
            out.bool(*value);
        }
        if let Some(value) = &self.frozen {
            out.key("frozen");
            out.bool(*value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type {
            out.key("line_type");
            value.to_json(out);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
    }
}

impl FromJson for AddLayerEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddLayerEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 記号の定義（DXF・DWG のブロック）を足す（#684）。中の要素は、定義の空間に足す。足した定義は、同じ apply の後ろの add の記号から名前で指せる。属性の定義・名前のない定義・外部参照は足せない。結果は id と hash を持たない。足した定義は tables の symbols から名前で引く。マイナーバージョン 99 から
#[derive(Clone, Debug, PartialEq)]
pub struct AddSymbolEdit {
    /// 定義の名前。図面にない名前で、layer の name と同じ決まり（DXF・DWG のブロックの名前で書けない文字を含まない）。大文字と小文字は区別しない
    pub name: String,
    /// 定義の基点 [X, Y]。記号を置く点に重ねる。書かなければ [0, 0]
    pub base: Option<Vec<f64>>,
    /// 中の要素。点は定義の空間の座標で書く。並べた順に足す
    pub elements: Vec<SymbolElement>,
}

impl AddSymbolEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_symbol")?;
        let name = object.required("name", json::string)?;
        let base = object.optional("base", |value| json::array(value, json::number))?;
        let elements = object.required("elements", |value| {
            json::array(value, SymbolElement::from_json)
        })?;
        Ok(Self {
            name,
            base,
            elements,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_symbol");
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.base {
            out.key("base");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("elements");
        out.array(&self.elements, |out, item| item.to_json(out));
    }
}

impl FromJson for AddSymbolEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for AddSymbolEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 足す記号の定義の中の要素。フィールドは add と同じ意味で、place と kind を持たない。引出線の注記は指せない。マイナーバージョン 99 から
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolElement {
    /// レイヤの元の名前。layer_id と一緒には書かない。どちらも書かなければレイヤ 0
    pub layer: Option<String>,
    /// レイヤの ID。tables が返したもの
    pub layer_id: Option<String>,
    /// 色
    pub color: Option<ColorChange>,
    /// 線種
    pub line_type: Option<LineTypeChange>,
    /// 線幅
    pub line_weight: Option<LineWeightChange>,
    /// 足す形
    pub geometry: Geometry,
}

impl SymbolElement {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let layer = object.optional("layer", json::string)?;
        let layer_id = object.optional("layer_id", json::string)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type = object.optional("line_type", LineTypeChange::from_json)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        let geometry = object.required("geometry", Geometry::from_json)?;
        Ok(Self {
            layer,
            layer_id,
            color,
            line_type,
            line_weight,
            geometry,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.layer {
            out.key("layer");
            out.string(value);
        }
        if let Some(value) = &self.layer_id {
            out.key("layer_id");
            out.string(value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type {
            out.key("line_type");
            value.to_json(out);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
        out.key("geometry");
        self.geometry.to_json(out);
    }
}

impl FromJson for SymbolElement {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SymbolElement {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 値を継承する先。layer はレイヤから、block は要素を置いた記号の配置から
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Inherit {
    /// `layer`
    Layer,
    /// `block`
    Block,
}

impl Inherit {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Layer, Self::Block];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Layer => "layer",
            Self::Block => "block",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "layer" => Some(Self::Layer),
            "block" => Some(Self::Block),
            _ => None,
        }
    }
}

impl FromJson for Inherit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Inherit {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 新しい色。inherit か、aci・sxf・jww・rgb の少なくとも 1 つのどちらかを書く。aci・sxf・jww は 1 つだけ書く
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorChange {
    /// 継承する先
    pub inherit: Option<Inherit>,
    /// AutoCAD の色の番号（1〜255）
    pub aci: Option<u64>,
    /// SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。マイナーバージョン 29 から
    pub sxf: Option<u64>,
    /// Jw_cad のペンの番号。0〜9 と、バージョン 420 からの SXF の拡張色 100〜356。マイナーバージョン 30 から
    pub jww: Option<u64>,
    /// [赤, 緑, 青]。それぞれ 0〜255
    pub rgb: Option<Vec<u64>>,
}

impl ColorChange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let inherit = object.optional("inherit", Inherit::from_json)?;
        let aci = object.optional("aci", |value| json::uint(value, 1, 255))?;
        let sxf = object.optional("sxf", |value| json::uint(value, 1, 9007199254740991))?;
        let jww = object.optional("jww", |value| json::uint(value, 0, 356))?;
        let rgb = object.optional("rgb", |value| {
            json::array(value, |value| json::uint(value, 0, 255))
        })?;
        Ok(Self {
            inherit,
            aci,
            sxf,
            jww,
            rgb,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.inherit {
            out.key("inherit");
            value.to_json(out);
        }
        if let Some(value) = &self.aci {
            out.key("aci");
            out.uint(*value);
        }
        if let Some(value) = &self.sxf {
            out.key("sxf");
            out.uint(*value);
        }
        if let Some(value) = &self.jww {
            out.key("jww");
            out.uint(*value);
        }
        if let Some(value) = &self.rgb {
            out.key("rgb");
            out.array(value, |out, item| out.uint(*item));
        }
    }
}

impl FromJson for ColorChange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ColorChange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 新しい線種。inherit・name・id のどれか 1 つを書く
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineTypeChange {
    /// 継承する先
    pub inherit: Option<Inherit>,
    /// 線種の元の名前。図面にある線種で、名前が 1 つに決まるもの
    pub name: Option<String>,
    /// 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。マイナーバージョン 8 から
    pub id: Option<String>,
}

impl LineTypeChange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let inherit = object.optional("inherit", Inherit::from_json)?;
        let name = object.optional("name", json::string)?;
        let id = object.optional("id", json::string)?;
        Ok(Self { inherit, name, id })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.inherit {
            out.key("inherit");
            value.to_json(out);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
    }
}

impl FromJson for LineTypeChange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineTypeChange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 新しい線幅。inherit・mm・default のどれか 1 つを書く
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineWeightChange {
    /// 継承する先
    pub inherit: Option<Inherit>,
    /// 用紙の上の幅（mm）。0 以上
    pub mm: Option<f64>,
    /// true なら、文書の既定の線幅
    pub default: Option<bool>,
}

impl LineWeightChange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let inherit = object.optional("inherit", Inherit::from_json)?;
        let mm = object.optional("mm", json::number)?;
        let default = object.optional("default", json::boolean)?;
        Ok(Self {
            inherit,
            mm,
            default,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.inherit {
            out.key("inherit");
            value.to_json(out);
        }
        if let Some(value) = &self.mm {
            out.key("mm");
            out.float(*value);
        }
        if let Some(value) = &self.default {
            out.key("default");
            out.bool(*value);
        }
    }
}

impl FromJson for LineWeightChange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineWeightChange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。マイナーバージョン 8 から
#[derive(Clone, Debug, PartialEq)]
pub struct TablesRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
}

impl TablesRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "tables")?;
        let doc = object.required("doc", json::string)?;
        Ok(Self {
            request_common,
            doc,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("tables");
        out.key("doc");
        out.string(&self.doc);
    }
}

impl FromJson for TablesRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TablesRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 表の項目。図面モデルの順
#[derive(Clone, Debug, PartialEq)]
pub struct TablesResult {
    /// レイヤ
    pub layers: Vec<LayerItem>,
    /// 線種。マイナーバージョン 26 から、定義を持つ
    pub line_types: Vec<LineTypeItem>,
    /// 寸法スタイル。マイナーバージョン 27 から、端部記号と大きさを持つ
    pub dimension_styles: Vec<DimensionStyleItem>,
    /// 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義のフィールドを持たない項目として後ろに並ぶ。マイナーバージョン 20 から
    pub text_styles: Vec<TextStyleItem>,
    /// 記号の定義。配置されていないものも含む。マイナーバージョン 81 から
    pub symbols: Vec<SymbolDefinitionItem>,
}

impl TablesResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let layers = object.required("layers", |value| json::array(value, LayerItem::from_json))?;
        let line_types = object.required("line_types", |value| {
            json::array(value, LineTypeItem::from_json)
        })?;
        let dimension_styles = object.required("dimension_styles", |value| {
            json::array(value, DimensionStyleItem::from_json)
        })?;
        let text_styles = object.required("text_styles", |value| {
            json::array(value, TextStyleItem::from_json)
        })?;
        let symbols = object.required("symbols", |value| {
            json::array(value, SymbolDefinitionItem::from_json)
        })?;
        Ok(Self {
            layers,
            line_types,
            dimension_styles,
            text_styles,
            symbols,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("layers");
        out.array(&self.layers, |out, item| item.to_json(out));
        out.key("line_types");
        out.array(&self.line_types, |out, item| item.to_json(out));
        out.key("dimension_styles");
        out.array(&self.dimension_styles, |out, item| item.to_json(out));
        out.key("text_styles");
        out.array(&self.text_styles, |out, item| item.to_json(out));
        out.key("symbols");
        out.array(&self.symbols, |out, item| item.to_json(out));
    }
}

impl FromJson for TablesResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TablesResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 記号の定義。要素と属性の配置や書式は元の定義に保ち、名前と属性の既定値を一覧にする
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolDefinitionItem {
    /// 定義の空間の公開 ID。entities.place と同じ
    pub id: String,
    /// 定義を識別する名前空間
    pub namespace: String,
    /// 元の名前。add の記号の name に使う
    pub name: String,
    /// 定義の基点。定義の空間の座標と単位。不明なら持たない
    pub base: Option<Point2>,
    /// 属性のタグ・既定値・入力の指定。元の順を保つ
    pub attributes: Vec<SymbolAttributeDefinition>,
    /// 寸法などが作った無名の定義である
    pub anonymous: bool,
    /// 外部参照の元のパス。参照先を開かない。外部参照でなければ持たない
    pub external: Option<String>,
    /// 定義の出所
    pub source: Source,
}

impl SymbolDefinitionItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let namespace = object.required("namespace", json::string)?;
        let name = object.required("name", json::string)?;
        let base = object.optional("base", Point2::from_json)?;
        let attributes = object.required("attributes", |value| {
            json::array(value, SymbolAttributeDefinition::from_json)
        })?;
        let anonymous = object.required("anonymous", json::boolean)?;
        let external = object.optional("external", json::string)?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            id,
            namespace,
            name,
            base,
            attributes,
            anonymous,
            external,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("namespace");
        out.string(&self.namespace);
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.base {
            out.key("base");
            value.to_json(out);
        }
        out.key("attributes");
        out.array(&self.attributes, |out, item| item.to_json(out));
        out.key("anonymous");
        out.bool(self.anonymous);
        if let Some(value) = &self.external {
            out.key("external");
            out.string(value);
        }
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for SymbolDefinitionItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SymbolDefinitionItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 記号の属性の定義。タグと値は翻訳しない。配置・書式・スタイルは一覧には含めない
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolAttributeDefinition {
    /// 元のタグ
    pub tag: String,
    /// 既定の 1 行の値
    pub value: String,
    /// 複数行の値があるとき、その元の書式つきの文字列。読むための値
    pub formatted_value: Option<String>,
    /// 値を尋ねる文
    pub prompt: String,
    /// 値が決まっていて、配置ごとに変えない
    pub constant: bool,
    /// 入力した値を確かめる指定
    pub verify: bool,
    /// 尋ねずに既定の値を使う指定
    pub preset: bool,
    /// 属性を表示しない
    pub hidden: bool,
    /// 属性の出所
    pub source: Source,
}

impl SymbolAttributeDefinition {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let tag = object.required("tag", json::string)?;
        let value = object.required("value", json::string)?;
        let formatted_value = object.optional("formatted_value", json::string)?;
        let prompt = object.required("prompt", json::string)?;
        let constant = object.required("constant", json::boolean)?;
        let verify = object.required("verify", json::boolean)?;
        let preset = object.required("preset", json::boolean)?;
        let hidden = object.required("hidden", json::boolean)?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            tag,
            value,
            formatted_value,
            prompt,
            constant,
            verify,
            preset,
            hidden,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("tag");
        out.string(&self.tag);
        out.key("value");
        out.string(&self.value);
        if let Some(value) = &self.formatted_value {
            out.key("formatted_value");
            out.string(value);
        }
        out.key("prompt");
        out.string(&self.prompt);
        out.key("constant");
        out.bool(self.constant);
        out.key("verify");
        out.bool(self.verify);
        out.key("preset");
        out.bool(self.preset);
        out.key("hidden");
        out.bool(self.hidden);
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for SymbolAttributeDefinition {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SymbolAttributeDefinition {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。マイナーバージョン 33 から
#[derive(Clone, Debug, PartialEq)]
pub struct SceneRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// 起点。実寸の空間・記号の定義の空間・用紙の ID（drawing と entities が返すもの）
    pub from: String,
    /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した空間の要素だけを描く。from が用紙のときだけ書ける
    pub view: Option<u64>,
    /// [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
    pub region: Option<Vec<f64>>,
    /// 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000
    pub tolerance: Option<f64>,
    /// true なら印刷する要素を、書かないか false なら表示する要素を描く
    pub print: Option<bool>,
    /// 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決のフラグを立てる
    pub palette: Option<String>,
    /// 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形のセクションに形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。マイナーバージョン 37 から
    pub fonts: Option<Vec<SceneFont>>,
    /// 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは、表示と比べて決めた意味（dxf.TEXT.40 は capital_a。マイナーバージョン 59 から）にし、それもなければ大文字の高さとみなす。fonts を書いたときだけ使う。マイナーバージョン 37 から
    pub heights: Option<BTreeMap<String, HeightReference>>,
}

impl SceneRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "scene")?;
        let doc = object.required("doc", json::string)?;
        let from = object.required("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let region = object.optional("region", |value| json::array(value, json::number))?;
        let tolerance = object.optional("tolerance", json::number)?;
        let print = object.optional("print", json::boolean)?;
        let palette = object.optional("palette", json::string)?;
        let fonts = object.optional("fonts", |value| json::array(value, SceneFont::from_json))?;
        let heights = object.optional("heights", |value| {
            json::map(value, HeightReference::from_json)
        })?;
        Ok(Self {
            request_common,
            doc,
            from,
            view,
            region,
            tolerance,
            print,
            palette,
            fonts,
            heights,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("scene");
        out.key("doc");
        out.string(&self.doc);
        out.key("from");
        out.string(&self.from);
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        if let Some(value) = &self.region {
            out.key("region");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.print {
            out.key("print");
            out.bool(*value);
        }
        if let Some(value) = &self.palette {
            out.key("palette");
            out.string(value);
        }
        if let Some(value) = &self.fonts {
            out.key("fonts");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.heights {
            out.key("heights");
            out.map(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for SceneRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SceneRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 建物モデルを開口付きの 3D シーンにし、ZMS3 の blob で返す。86 から地物の面と高さのある線・面も扱う。階・クラス・ID の各条件の中は和、条件どうしは積。箱は要素の重なりで選び、メッシュを切らない。88 から点群を元の八分木の節ごとの点の区画（ZMS3 1.1）で返す。点群の点は pointcloud と同じ節の順で選び、座標系と高さの基準を変換しない。94 から平面直角座標系の GeoTIFF の DEM も、外枠の地物でなく標高格子を点群と同じ点の区画で返す。マイナーバージョン 82 から
#[derive(Clone, Debug, PartialEq)]
pub struct Scene3dRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// open で作った建物・地物・点群の文書名。点群は 88 から
    pub doc: String,
    /// 所属する階の要素の ID。省略は全階、空の配列は何も選ばない。地物と点群には指定できない
    pub storeys: Option<Vec<String>>,
    /// 建物のクラス名か地物の型。大文字・小文字を区別する。省略は全種類、空の配列は何も選ばない。点群には指定できない
    pub classes: Option<Vec<String>>,
    /// 建物の要素か元の地物の ID。省略は全要素、空の配列は何も選ばない。建物では参照する階の行も返す。点群には指定できない
    pub ids: Option<Vec<String>>,
    /// 地物だけに指定する投影先の平面直角座標系の系（1〜19）。省略時は元の系、経緯度では範囲の中心から計算用の系を仮定して診断する。86 から
    pub zone: Option<u64>,
    /// 地物だけに指定する LOD（0〜4）。省略時は選択した地物にある最大の LOD を 1 段選ぶ。LOD のない形は含める。86 から
    pub lod: Option<u64>,
    /// プロジェクト空間の文書単位の箱。重なる要素を選び、形のない要素も残す。切り抜きは読み手が行う
    pub r#box: Option<Scene3dBox>,
    /// 点群だけに指定する返す点の数の上限。省略時は選んだすべての点。88 から
    pub limit: Option<u64>,
    /// 文書単位の正の有限値。点群では点の間隔（メートル）で、これより細かい段の節を選ばない。建物では曲線を分ける弦の誤差。地物では f32 の座標の丸めをこの値の 1/4 までに制限し、投影した辺全体の誤差は含めない。省略時は対象範囲の対角線の 1/50000、空か点だけなら 1 mm 相当。建物の範囲は 1 mm の予備評価で求める
    pub tolerance: Option<f64>,
}

impl Scene3dRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "scene3d")?;
        let doc = object.required("doc", json::string)?;
        let storeys = object.optional("storeys", |value| json::array(value, json::string))?;
        let classes = object.optional("classes", |value| json::array(value, json::string))?;
        let ids = object.optional("ids", |value| json::array(value, json::string))?;
        let zone = object.optional("zone", |value| json::uint(value, 1, 19))?;
        let lod = object.optional("lod", |value| json::uint(value, 0, 4))?;
        let r#box = object.optional("box", Scene3dBox::from_json)?;
        let limit = object.optional("limit", |value| json::uint(value, 0, 9007199254740991))?;
        let tolerance = object.optional("tolerance", json::number)?;
        Ok(Self {
            request_common,
            doc,
            storeys,
            classes,
            ids,
            zone,
            lod,
            r#box,
            limit,
            tolerance,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("scene3d");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.storeys {
            out.key("storeys");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.classes {
            out.key("classes");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.ids {
            out.key("ids");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.zone {
            out.key("zone");
            out.uint(*value);
        }
        if let Some(value) = &self.lod {
            out.key("lod");
            out.uint(*value);
        }
        if let Some(value) = &self.r#box {
            out.key("box");
            value.to_json(out);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
    }
}

impl FromJson for Scene3dRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Scene3dRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 建物ではプロジェクト空間の文書単位、地物では投影先の、点群では元の座標系の東・北・高さのメートルの箱。Z が上
#[derive(Clone, Debug, PartialEq)]
pub struct Scene3dBox {
    /// 最小の X・Y・Z の 3 値
    pub min: Vec<f64>,
    /// 最大の X・Y・Z の 3 値
    pub max: Vec<f64>,
}

impl Scene3dBox {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let min = object.required("min", |value| json::array(value, json::number))?;
        let max = object.required("max", |value| json::array(value, json::number))?;
        Ok(Self { min, max })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("min");
        out.array(&self.min, |out, item| out.float(*item));
        out.key("max");
        out.array(&self.max, |out, item| out.float(*item));
    }
}

impl FromJson for Scene3dBox {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Scene3dBox {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 建物では文書が持つ地図への変換の元値で、シーンの座標へ適用していない。地物では投影先の EPSG 名と恒等変換を返し、シーンの X・Y は既に東・北のメートル
#[derive(Clone, Debug, PartialEq)]
pub struct Scene3dMap {
    /// 地図の座標系の名前。未指定なら持たない
    pub crs_name: Option<String>,
    /// 東への移動
    pub eastings: f64,
    /// 北への移動
    pub northings: f64,
    /// 高さの移動
    pub height: f64,
    /// 地図の X 軸の横の成分の元値
    pub x_axis_abscissa: Option<f64>,
    /// 地図の X 軸の縦の成分の元値
    pub x_axis_ordinate: Option<f64>,
    /// 地図への尺度の元値
    pub scale: Option<f64>,
}

impl Scene3dMap {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let crs_name = object.optional("crs_name", json::string)?;
        let eastings = object.required("eastings", json::number)?;
        let northings = object.required("northings", json::number)?;
        let height = object.required("height", json::number)?;
        let x_axis_abscissa = object.optional("x_axis_abscissa", json::number)?;
        let x_axis_ordinate = object.optional("x_axis_ordinate", json::number)?;
        let scale = object.optional("scale", json::number)?;
        Ok(Self {
            crs_name,
            eastings,
            northings,
            height,
            x_axis_abscissa,
            x_axis_ordinate,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.crs_name {
            out.key("crs_name");
            out.string(value);
        }
        out.key("eastings");
        out.float(self.eastings);
        out.key("northings");
        out.float(self.northings);
        out.key("height");
        out.float(self.height);
        if let Some(value) = &self.x_axis_abscissa {
            out.key("x_axis_abscissa");
            out.float(*value);
        }
        if let Some(value) = &self.x_axis_ordinate {
            out.key("x_axis_ordinate");
            out.float(*value);
        }
        if let Some(value) = &self.scale {
            out.key("scale");
            out.float(*value);
        }
    }
}

impl FromJson for Scene3dMap {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Scene3dMap {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 返した 3D シーンの列の行数。共有するメッシュの点と面は 1 回だけ数える
#[derive(Clone, Debug, PartialEq)]
pub struct Scene3dCounts {
    /// 塊
    pub chunks: u64,
    /// 共有するメッシュ
    pub meshes: u64,
    /// メッシュの配置
    pub instances: u64,
    /// 要素と参照先の階。形のない行も含む
    pub elements: u64,
    /// 色
    pub colors: u64,
    /// 局所の頂点
    pub vertices: u64,
    /// 三角形
    pub triangles: u64,
    /// 形の稜線
    pub edges: u64,
    /// 点群の点。点群のシーンだけが持つ。88 から
    pub points: Option<u64>,
}

impl Scene3dCounts {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let chunks = object.required("chunks", |value| json::uint(value, 0, 9007199254740991))?;
        let meshes = object.required("meshes", |value| json::uint(value, 0, 9007199254740991))?;
        let instances =
            object.required("instances", |value| json::uint(value, 0, 9007199254740991))?;
        let elements =
            object.required("elements", |value| json::uint(value, 0, 9007199254740991))?;
        let colors = object.required("colors", |value| json::uint(value, 0, 9007199254740991))?;
        let vertices =
            object.required("vertices", |value| json::uint(value, 0, 9007199254740991))?;
        let triangles =
            object.required("triangles", |value| json::uint(value, 0, 9007199254740991))?;
        let edges = object.required("edges", |value| json::uint(value, 0, 9007199254740991))?;
        let points = object.optional("points", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            chunks,
            meshes,
            instances,
            elements,
            colors,
            vertices,
            triangles,
            edges,
            points,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("chunks");
        out.uint(self.chunks);
        out.key("meshes");
        out.uint(self.meshes);
        out.key("instances");
        out.uint(self.instances);
        out.key("elements");
        out.uint(self.elements);
        out.key("colors");
        out.uint(self.colors);
        out.key("vertices");
        out.uint(self.vertices);
        out.key("triangles");
        out.uint(self.triangles);
        out.key("edges");
        out.uint(self.edges);
        if let Some(value) = &self.points {
            out.key("points");
            out.uint(*value);
        }
    }
}

impl FromJson for Scene3dCounts {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Scene3dCounts {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 3D シーンの座標・許容差・行数と blob の番号
#[derive(Clone, Debug, PartialEq)]
pub struct Scene3dResult {
    /// 元の建物か地物の文書名
    pub doc: String,
    /// シーンの座標と許容差の長さの単位
    pub unit: BuildingUnit,
    /// 文書の地図への変換。なければ持たない
    pub map: Option<Scene3dMap>,
    /// 地物の高さの基準と成果を表す EPSG。元が不明なら持たない。86 から
    pub height_crs: Option<u64>,
    /// 地物で選択した LOD。LOD のない形だけなら持たない。86 から
    pub lod: Option<u64>,
    /// 使った文書単位の許容差。建物では曲線の弦の誤差、地物では f32 の丸めの上限の 4 倍。点群では要求の点の間隔、省略時は返したもっとも深い段の点の間隔。丸めのずれは別の診断で返す
    pub tolerance: f64,
    /// 点群で上限のために返さなかった点がある。点群のシーンだけが持つ。88 から
    pub truncated: Option<bool>,
    /// 要求の断面の箱。なければ持たない
    pub r#box: Option<Scene3dBox>,
    /// 返した塊の文書単位の範囲。形がなければ持たない
    pub extent: Option<Scene3dBox>,
    /// シーンの行数
    pub counts: Scene3dCounts,
    /// 種類が scene3d の blob の番号。zm_take で取り出す
    pub blob: u64,
}

impl Scene3dResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let unit = object.required("unit", BuildingUnit::from_json)?;
        let map = object.optional("map", Scene3dMap::from_json)?;
        let height_crs =
            object.optional("height_crs", |value| json::uint(value, 0, 9007199254740991))?;
        let lod = object.optional("lod", |value| json::uint(value, 0, 4))?;
        let tolerance = object.required("tolerance", json::number)?;
        let truncated = object.optional("truncated", json::boolean)?;
        let r#box = object.optional("box", Scene3dBox::from_json)?;
        let extent = object.optional("extent", Scene3dBox::from_json)?;
        let counts = object.required("counts", Scene3dCounts::from_json)?;
        let blob = object.required("blob", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            doc,
            unit,
            map,
            height_crs,
            lod,
            tolerance,
            truncated,
            r#box,
            extent,
            counts,
            blob,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("unit");
        self.unit.to_json(out);
        if let Some(value) = &self.map {
            out.key("map");
            value.to_json(out);
        }
        if let Some(value) = &self.height_crs {
            out.key("height_crs");
            out.uint(*value);
        }
        if let Some(value) = &self.lod {
            out.key("lod");
            out.uint(*value);
        }
        out.key("tolerance");
        out.float(self.tolerance);
        if let Some(value) = &self.truncated {
            out.key("truncated");
            out.bool(*value);
        }
        if let Some(value) = &self.r#box {
            out.key("box");
            value.to_json(out);
        }
        if let Some(value) = &self.extent {
            out.key("extent");
            value.to_json(out);
        }
        out.key("counts");
        self.counts.to_json(out);
        out.key("blob");
        out.uint(self.blob);
    }
}

impl FromJson for Scene3dResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Scene3dResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を scene と同じ起点・配置・フォントで 2D のシーンにし、画像を blob で返す（決定 0272・0275・0277）。字形の輪郭だけを描き、字形のない空でない文字・外部画像・名前付きの点記号は元の ID と unit を診断して描かない。シーン生成と描画で同じ予算と中断の合図を使い、失敗では画像を返さない。PNG は最大 16777216 画素で、変換後の座標は絶対値 2^31 ピクセル以内。SVG はマイナーバージョン 64、PNG は 65 から
#[derive(Clone, Debug, PartialEq)]
pub struct RenderRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// scene と同じ起点。実寸の空間・記号の定義の空間・用紙の ID
    pub from: String,
    /// 起点の用紙のビューの番号。0 から数える。from が用紙のときだけ書ける。scene と同じ意味
    pub view: Option<u64>,
    /// 画像の出力形式
    pub format: RenderFormat,
    /// 出力の幅（ピクセル）
    pub width: u64,
    /// 出力の高さ（ピクセル）
    pub height: u64,
    /// [x の最小, y の最小, x の最大, y の最大]。起点の座標で、幅と高さが正の有限値になる範囲を明示する。scene の範囲の絞り込みにも使い、縦横同率で中央に収めて範囲外を切る。縦横比の違いによる余白は背景色にする
    pub region: Vec<f64>,
    /// 用紙 mm の線幅をピクセルへ直す DPI。正の有限値。座標の単位を mm と推測しない。線幅 0 は 1 ピクセル
    pub dpi: f64,
    /// 背景の sRGB と alpha。[r, g, b, a] の 4 値を 0〜255 で明示する
    pub background: Vec<u64>,
    /// ACI 7 と未解決の色に使う sRGB と alpha。[r, g, b, a] の 4 値を 0〜255 で明示する。未解決の色は診断する
    pub foreground: Vec<u64>,
    /// scene と同じ弦の誤差（起点の長さ）。0 より大きい。書かなければ用紙は 0.02 mm、空間は要素の範囲の対角線の 1/50000
    pub tolerance: Option<f64>,
    /// scene と同じ指定。true は印刷する要素、書かないか false は表示する要素を描く
    pub print: Option<bool>,
    /// 隠すレイヤの公開 ID の一覧。scene の layer_ids と同じ値で描画単位を除く。未知の ID は失敗、省略または空なら従来どおり描く
    pub hidden_layers: Option<Vec<String>>,
    /// scene と同じ色の表の入力の名前、または組み込みの表 builtin:cad-v1。zm_push で渡した JSON の values に rgb と aci・sxf・jww の符号を並べる
    pub palette: Option<String>,
    /// scene と同じフォントの指定。名前を合わせ、なければ名前のないフォントを使う。字形の輪郭を作るため、zm_push でフォントのバイト列を渡す。OS のフォントは探索しない
    pub fonts: Option<Vec<SceneFont>>,
    /// scene と同じ宣言値の高さの意味。fonts を書いたときだけ使う。書かないキーは既定の意味に従う（決定 0176・0188）
    pub heights: Option<BTreeMap<String, HeightReference>>,
}

impl RenderRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "render")?;
        let doc = object.required("doc", json::string)?;
        let from = object.required("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let format = object.required("format", RenderFormat::from_json)?;
        let width = object.required("width", |value| json::uint(value, 1, 4294967295))?;
        let height = object.required("height", |value| json::uint(value, 1, 4294967295))?;
        let region = object.required("region", |value| json::array(value, json::number))?;
        let dpi = object.required("dpi", json::number)?;
        let background = object.required("background", |value| {
            json::array(value, |value| json::uint(value, 0, 255))
        })?;
        let foreground = object.required("foreground", |value| {
            json::array(value, |value| json::uint(value, 0, 255))
        })?;
        let tolerance = object.optional("tolerance", json::number)?;
        let print = object.optional("print", json::boolean)?;
        let hidden_layers =
            object.optional("hidden_layers", |value| json::array(value, json::string))?;
        let palette = object.optional("palette", json::string)?;
        let fonts = object.optional("fonts", |value| json::array(value, SceneFont::from_json))?;
        let heights = object.optional("heights", |value| {
            json::map(value, HeightReference::from_json)
        })?;
        Ok(Self {
            request_common,
            doc,
            from,
            view,
            format,
            width,
            height,
            region,
            dpi,
            background,
            foreground,
            tolerance,
            print,
            hidden_layers,
            palette,
            fonts,
            heights,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("render");
        out.key("doc");
        out.string(&self.doc);
        out.key("from");
        out.string(&self.from);
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        out.key("format");
        self.format.to_json(out);
        out.key("width");
        out.uint(self.width);
        out.key("height");
        out.uint(self.height);
        out.key("region");
        out.array(&self.region, |out, item| out.float(*item));
        out.key("dpi");
        out.float(self.dpi);
        out.key("background");
        out.array(&self.background, |out, item| out.uint(*item));
        out.key("foreground");
        out.array(&self.foreground, |out, item| out.uint(*item));
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.print {
            out.key("print");
            out.bool(*value);
        }
        if let Some(value) = &self.hidden_layers {
            out.key("hidden_layers");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.palette {
            out.key("palette");
            out.string(value);
        }
        if let Some(value) = &self.fonts {
            out.key("fonts");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.heights {
            out.key("heights");
            out.map(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for RenderRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RenderRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 画像の出力形式。svg は SVG 1.1（image/svg+xml）、png は RGBA8・非インターレースの PNG（image/png、マイナーバージョン 65 から）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderFormat {
    /// `svg`
    Svg,
    /// `png`
    Png,
}

impl RenderFormat {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Svg, Self::Png];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "svg" => Some(Self::Svg),
            "png" => Some(Self::Png),
            _ => None,
        }
    }
}

impl FromJson for RenderFormat {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for RenderFormat {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 画像の形式・サイズと、シーンの起点・単位・許容差、画像の blob の番号
#[derive(Clone, Debug, PartialEq)]
pub struct RenderResult {
    /// 画像の出力形式
    pub format: RenderFormat,
    /// 出力の幅（ピクセル）
    pub width: u64,
    /// 出力の高さ（ピクセル）
    pub height: u64,
    /// シーンの起点の ID
    pub from: String,
    /// 起点のビューの番号。ビューでなければ持たない
    pub view: Option<u64>,
    /// 起点の座標の単位。用紙とビューからは millimeter
    pub unit: Unit,
    /// シーンで使った弦の誤差（起点の長さ）
    pub tolerance: f64,
    /// 画像の blob の番号（種類は image）。zm_take で取り出す
    pub blob: u64,
}

impl RenderResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let format = object.required("format", RenderFormat::from_json)?;
        let width = object.required("width", |value| json::uint(value, 1, 4294967295))?;
        let height = object.required("height", |value| json::uint(value, 1, 4294967295))?;
        let from = object.required("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let unit = object.required("unit", Unit::from_json)?;
        let tolerance = object.required("tolerance", json::number)?;
        let blob = object.required("blob", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            format,
            width,
            height,
            from,
            view,
            unit,
            tolerance,
            blob,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("format");
        self.format.to_json(out);
        out.key("width");
        out.uint(self.width);
        out.key("height");
        out.uint(self.height);
        out.key("from");
        out.string(&self.from);
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        out.key("unit");
        self.unit.to_json(out);
        out.key("tolerance");
        out.float(self.tolerance);
        out.key("blob");
        out.uint(self.blob);
    }
}

impl FromJson for RenderResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RenderResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた TIFF・GeoTIFF の指定ページの画素を RGBA8・非インターレースの PNG に復号し、画像の blob で返す（#877）。複数ページの TIFF は page で 1 から数えるページ番号を選ぶ（書かなければ 1）。max_side を指定すると、長辺が max_side 以下になるよう縦横比を保って縮小して返す（最大 16777216 画素）。マイナーバージョン 73 から
#[derive(Clone, Debug, PartialEq)]
pub struct RasterRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// ページ番号（IFD の 1 から始まる番号）。書かなければ 1
    pub page: Option<u64>,
    /// 出力の長辺の最大ピクセル数。元の長辺がこれを超えるときだけ縦横比を保って縮小する
    pub max_side: Option<u64>,
}

impl RasterRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "raster")?;
        let doc = object.required("doc", json::string)?;
        let page = object.optional("page", |value| json::uint(value, 1, 9007199254740991))?;
        let max_side = object.optional("max_side", |value| json::uint(value, 1, 16384))?;
        Ok(Self {
            request_common,
            doc,
            page,
            max_side,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("raster");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.page {
            out.key("page");
            out.uint(*value);
        }
        if let Some(value) = &self.max_side {
            out.key("max_side");
            out.uint(*value);
        }
    }
}

impl FromJson for RasterRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RasterRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 復号した PNG 画像のサイズ・元のページサイズ・総ページ数と、画像の blob の番号
#[derive(Clone, Debug, PartialEq)]
pub struct RasterResult {
    /// 出力した PNG の幅（ピクセル）
    pub width: u64,
    /// 出力した PNG の高さ（ピクセル）
    pub height: u64,
    /// 元のページの幅（ピクセル）
    pub source_width: u64,
    /// 元のページの高さ（ピクセル）
    pub source_height: u64,
    /// 復号したページ番号（1 から）
    pub page: u64,
    /// 文書に含まれる総ページ（IFD）数
    pub pages: u64,
    /// PNG 画像の blob の番号（種類は image）。zm_take で取り出す
    pub blob: u64,
}

impl RasterResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let width = object.required("width", |value| json::uint(value, 1, 4294967295))?;
        let height = object.required("height", |value| json::uint(value, 1, 4294967295))?;
        let source_width =
            object.required("source_width", |value| json::uint(value, 1, 4294967295))?;
        let source_height =
            object.required("source_height", |value| json::uint(value, 1, 4294967295))?;
        let page = object.required("page", |value| json::uint(value, 1, 9007199254740991))?;
        let pages = object.required("pages", |value| json::uint(value, 1, 9007199254740991))?;
        let blob = object.required("blob", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            width,
            height,
            source_width,
            source_height,
            page,
            pages,
            blob,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("width");
        out.uint(self.width);
        out.key("height");
        out.uint(self.height);
        out.key("source_width");
        out.uint(self.source_width);
        out.key("source_height");
        out.uint(self.source_height);
        out.key("page");
        out.uint(self.page);
        out.key("pages");
        out.uint(self.pages);
        out.key("blob");
        out.uint(self.blob);
    }
}

impl FromJson for RasterResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RasterResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// シーンの文字を置くフォント（決定 0188）
#[derive(Clone, Debug, PartialEq)]
pub struct SceneFont {
    /// 文字が指すフォントの名前（ファイルに書かれた元の名前）。書かなければ、名前の合わない文字に使う既定のフォント。同じ名前と、名前のないものは 1 つまで
    pub name: Option<String>,
    /// TrueType・OpenType（CFF・CFF2 を含む）のファイルの入力の名前（zm_push で渡したもの）
    pub input: String,
    /// フォントのコレクション（TTC）の中の番号。書かなければ 0
    pub index: Option<u64>,
}

impl SceneFont {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let input = object.required("input", json::string)?;
        let index = object.optional("index", |value| json::uint(value, 0, 4294967295))?;
        Ok(Self { name, input, index })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        out.key("input");
        out.string(&self.input);
        if let Some(value) = &self.index {
            out.key("index");
            out.uint(*value);
        }
    }
}

impl FromJson for SceneFont {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SceneFont {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ、capital_a は A の字形の外形の上（A がなければ大文字の高さ。マイナーバージョン 59 から）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HeightReference {
    /// `cap_height`
    CapHeight,
    /// `em`
    Em,
    /// `frame`
    Frame,
    /// `capital_a`
    CapitalA,
}

impl HeightReference {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::CapHeight, Self::Em, Self::Frame, Self::CapitalA];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::CapHeight => "cap_height",
            Self::Em => "em",
            Self::Frame => "frame",
            Self::CapitalA => "capital_a",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "cap_height" => Some(Self::CapHeight),
            "em" => Some(Self::Em),
            "frame" => Some(Self::Frame),
            "capital_a" => Some(Self::CapitalA),
            _ => None,
        }
    }
}

impl FromJson for HeightReference {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for HeightReference {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// シーンの要約と、シーンの blob の番号
#[derive(Clone, Debug, PartialEq)]
pub struct SceneResult {
    /// 起点の ID
    pub from: String,
    /// 起点のビューの番号。ビューでなければ持たない
    pub view: Option<u64>,
    /// 起点の座標の単位。用紙とビューからは millimeter
    pub unit: Unit,
    /// 使った弦の誤差（起点の長さ）
    pub tolerance: f64,
    /// 返した描くものの数
    pub counts: SceneCounts,
    /// 返した塊の範囲。起点の座標。塊がなければ持たない
    pub extent: Option<Extent>,
    /// シーンの blob の番号（種類は scene）。zm_take で取り出す
    pub blob: u64,
}

impl SceneResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let from = object.required("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let unit = object.required("unit", Unit::from_json)?;
        let tolerance = object.required("tolerance", json::number)?;
        let counts = object.required("counts", SceneCounts::from_json)?;
        let extent = object.optional("extent", Extent::from_json)?;
        let blob = object.required("blob", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            from,
            view,
            unit,
            tolerance,
            counts,
            extent,
            blob,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("from");
        out.string(&self.from);
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        out.key("unit");
        self.unit.to_json(out);
        out.key("tolerance");
        out.float(self.tolerance);
        out.key("counts");
        self.counts.to_json(out);
        if let Some(value) = &self.extent {
            out.key("extent");
            value.to_json(out);
        }
        out.key("blob");
        out.uint(self.blob);
    }
}

impl FromJson for SceneResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SceneResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// シーンの描くものの種類ごとの数
#[derive(Clone, Debug, PartialEq)]
pub struct SceneCounts {
    /// 塊
    pub chunks: u64,
    /// 線（折れ線）
    pub lines: u64,
    /// 線と面の頂点
    pub vertices: u64,
    /// 面
    pub faces: u64,
    /// 文字
    pub texts: u64,
    /// 点
    pub points: u64,
    /// 画像
    pub images: u64,
    /// 置いた字形。fonts を書かなければ 0。マイナーバージョン 37 から
    pub glyphs: u64,
}

impl SceneCounts {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let chunks = object.required("chunks", |value| json::uint(value, 0, 9007199254740991))?;
        let lines = object.required("lines", |value| json::uint(value, 0, 9007199254740991))?;
        let vertices =
            object.required("vertices", |value| json::uint(value, 0, 9007199254740991))?;
        let faces = object.required("faces", |value| json::uint(value, 0, 9007199254740991))?;
        let texts = object.required("texts", |value| json::uint(value, 0, 9007199254740991))?;
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        let images = object.required("images", |value| json::uint(value, 0, 9007199254740991))?;
        let glyphs = object.required("glyphs", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            chunks,
            lines,
            vertices,
            faces,
            texts,
            points,
            images,
            glyphs,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("chunks");
        out.uint(self.chunks);
        out.key("lines");
        out.uint(self.lines);
        out.key("vertices");
        out.uint(self.vertices);
        out.key("faces");
        out.uint(self.faces);
        out.key("texts");
        out.uint(self.texts);
        out.key("points");
        out.uint(self.points);
        out.key("images");
        out.uint(self.images);
        out.key("glyphs");
        out.uint(self.glyphs);
    }
}

impl FromJson for SceneCounts {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SceneCounts {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。マイナーバージョン 34 から
#[derive(Clone, Debug, PartialEq)]
pub struct CheckRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で作ったもの
    pub doc: String,
    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    pub profiles: Vec<String>,
    /// 照合に使うプロファイル。profiles の中から選ぶ
    pub profile: ProfileRef,
    /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
    pub from: Option<String>,
    /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
    pub view: Option<u64>,
    /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。マイナーバージョン 40 から
    pub file_name: Option<String>,
    /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す指摘の数の上限。書かなければ 1000
    pub limit: Option<u64>,
    /// 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
    pub palette: Option<String>,
}

impl CheckRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "check")?;
        let doc = object.required("doc", json::string)?;
        let profiles = object.required("profiles", |value| json::array(value, json::string))?;
        let profile = object.required("profile", ProfileRef::from_json)?;
        let from = object.optional("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let file_name = object.optional("file_name", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        let palette = object.optional("palette", json::string)?;
        Ok(Self {
            request_common,
            doc,
            profiles,
            profile,
            from,
            view,
            file_name,
            offset,
            limit,
            palette,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("check");
        out.key("doc");
        out.string(&self.doc);
        out.key("profiles");
        out.array(&self.profiles, |out, item| out.string(item));
        out.key("profile");
        self.profile.to_json(out);
        if let Some(value) = &self.from {
            out.key("from");
            out.string(value);
        }
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        if let Some(value) = &self.file_name {
            out.key("file_name");
            out.string(value);
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
        if let Some(value) = &self.palette {
            out.key("palette");
            out.string(value);
        }
    }
}

impl FromJson for CheckRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CheckRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// プロファイルの識別（決定 0138）
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileRef {
    /// 基準の系統（mlit-cad など）
    pub id: String,
    /// 基準の版（2025-12 など）
    pub edition: String,
}

impl ProfileRef {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let edition = object.required("edition", json::string)?;
        Ok(Self { id, edition })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("edition");
        out.string(&self.edition);
    }
}

impl FromJson for ProfileRef {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ProfileRef {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 照合の結果。指摘は offset から limit 個まで
#[derive(Clone, Debug, PartialEq)]
pub struct CheckResult {
    /// 照合に使ったプロファイルの鎖。extends の基から順
    pub chain: Vec<ProfileIdentity>,
    /// 照合した規則ごとの数。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
    pub rules: Vec<CheckRule>,
    /// 指摘。規則の順に、規則ごとにたどった順
    pub findings: Vec<CheckFinding>,
    /// 全体の指摘の数
    pub total: u64,
    /// 続きの最初の指摘の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl CheckResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let chain = object.required("chain", |value| {
            json::array(value, ProfileIdentity::from_json)
        })?;
        let rules = object.required("rules", |value| json::array(value, CheckRule::from_json))?;
        let findings = object.required("findings", |value| {
            json::array(value, CheckFinding::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            chain,
            rules,
            findings,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("chain");
        out.array(&self.chain, |out, item| item.to_json(out));
        out.key("rules");
        out.array(&self.rules, |out, item| item.to_json(out));
        out.key("findings");
        out.array(&self.findings, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for CheckResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CheckResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 照合に使ったプロファイルの識別と、データの直し
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileIdentity {
    /// 基準の系統
    pub id: String,
    /// 基準の版
    pub edition: String,
    /// このデータの直し。1 から
    pub revision: u64,
}

impl ProfileIdentity {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let edition = object.required("edition", json::string)?;
        let revision =
            object.required("revision", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            id,
            edition,
            revision,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("edition");
        out.string(&self.edition);
        out.key("revision");
        out.uint(self.revision);
    }
}

impl FromJson for ProfileIdentity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ProfileIdentity {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 規則ごとの数
#[derive(Clone, Debug, PartialEq)]
pub struct CheckRule {
    /// 規則の id
    pub id: String,
    /// 照合の種類（layer-name など）
    pub check: String,
    /// 指摘の数。照合できなかったものを除く
    pub findings: u64,
    /// 照合できなかった数
    pub unsupported: u64,
}

impl CheckRule {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let check = object.required("check", json::string)?;
        let findings =
            object.required("findings", |value| json::uint(value, 0, 9007199254740991))?;
        let unsupported = object.required("unsupported", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            id,
            check,
            findings,
            unsupported,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("check");
        out.string(&self.check);
        out.key("findings");
        out.uint(self.findings);
        out.key("unsupported");
        out.uint(self.unsupported);
    }
}

impl FromJson for CheckRule {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CheckRule {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 指摘。diagnostic は診断と同じ形で、describe で文にできる。severity は規則の level で、照合できない値（profile.<種類>.unsupported）は info。count は 1
#[derive(Clone, Debug, PartialEq)]
pub struct CheckFinding {
    /// 規則の id
    pub rule: String,
    /// code・重要度・場所（要素・実効のレイヤ・用紙の ID）・見つけた値
    pub diagnostic: Diagnostic,
}

impl CheckFinding {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let rule = object.required("rule", json::string)?;
        let diagnostic = object.required("diagnostic", Diagnostic::from_json)?;
        Ok(Self { rule, diagnostic })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("rule");
        out.string(&self.rule);
        out.key("diagnostic");
        self.diagnostic.to_json(out);
    }
}

impl FromJson for CheckFinding {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CheckFinding {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。マイナーバージョン 42 から
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// IFC の入力の名前（zm_push で渡した .ifc か .ifczip）
    pub input: String,
    /// EXPRESS の入力の名前（zm_push で渡した .exp）
    pub express: String,
    /// 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す指摘の数の上限。書かなければ 1000
    pub limit: Option<u64>,
}

impl ValidateRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "validate")?;
        let input = object.required("input", json::string)?;
        let express = object.required("express", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        Ok(Self {
            request_common,
            input,
            express,
            offset,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("validate");
        out.key("input");
        out.string(&self.input);
        out.key("express");
        out.string(&self.express);
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for ValidateRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ValidateRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// スキーマに照らしたチェックの結果。指摘は offset から limit 個まで。スキーマの名前の違いと評価しない規則は、診断で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ValidateResult {
    /// EXPRESS のスキーマの名前（大文字）
    pub schema: String,
    /// IFC の FILE_SCHEMA の名前
    pub file_schema: String,
    /// EXPRESS のバイト列のハッシュ（v2: と 32 桁の 16 進）。どの EXPRESS でチェックしたかを残す
    pub express_hash: String,
    /// 評価しなかった規則（WHERE・UNIQUE・RULE）の数。確認できなかった項目
    pub rules_not_evaluated: u64,
    /// 指摘。インスタンスの番号の順に、属性の順。診断と同じ形で、describe で文にできる。count は 1
    pub findings: Vec<Diagnostic>,
    /// 全体の指摘の数
    pub total: u64,
    /// 続きの最初の指摘の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl ValidateResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let schema = object.required("schema", json::string)?;
        let file_schema = object.required("file_schema", json::string)?;
        let express_hash = object.required("express_hash", json::string)?;
        let rules_not_evaluated = object.required("rules_not_evaluated", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        let findings = object.required("findings", |value| {
            json::array(value, Diagnostic::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            schema,
            file_schema,
            express_hash,
            rules_not_evaluated,
            findings,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("schema");
        out.string(&self.schema);
        out.key("file_schema");
        out.string(&self.file_schema);
        out.key("express_hash");
        out.string(&self.express_hash);
        out.key("rules_not_evaluated");
        out.uint(self.rules_not_evaluated);
        out.key("findings");
        out.array(&self.findings, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for ValidateResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ValidateResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書のバージョンへ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元のバージョンのハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。マイナーバージョン 43 から
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewCarryRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 新しいバージョンの文書の名前。open で作ったもの。マイナーバージョン 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く
    pub doc: String,
    /// レビューの文書の入力の名前（zm_push で渡した JSON）
    pub review: String,
    /// 移す対象の ID。レビューの文書の targets の id
    pub target: String,
    /// 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す参照の数の上限。書かなければ 1000
    pub limit: Option<u64>,
}

impl ReviewCarryRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "review_carry")?;
        let doc = object.required("doc", json::string)?;
        let review = object.required("review", json::string)?;
        let target = object.required("target", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        Ok(Self {
            request_common,
            doc,
            review,
            target,
            offset,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("review_carry");
        out.key("doc");
        out.string(&self.doc);
        out.key("review");
        out.string(&self.review);
        out.key("target");
        out.string(&self.target);
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for ReviewCarryRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewCarryRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 移した結果。参照は offset から limit 個まで。移した文書は blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewCarryResult {
    /// 移す前の対象のハッシュ
    pub from: String,
    /// 移した後の対象のハッシュ。文書のハッシュ
    pub hash: String,
    /// 要素の参照ごとの状態。指摘の順に、場所の順、参照の順
    pub references: Vec<CarriedReference>,
    /// 全体の参照の数
    pub total: u64,
    /// 続きの最初の参照の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl ReviewCarryResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let from = object.required("from", json::string)?;
        let hash = object.required("hash", json::string)?;
        let references = object.required("references", |value| {
            json::array(value, CarriedReference::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            from,
            hash,
            references,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("from");
        out.string(&self.from);
        out.key("hash");
        out.string(&self.hash);
        out.key("references");
        out.array(&self.references, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for ReviewCarryResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewCarryResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 移した要素の参照
#[derive(Clone, Debug, PartialEq)]
pub struct CarriedReference {
    /// 指摘の ID
    pub topic: String,
    /// 場所の ID
    pub location: String,
    /// 要素の ID
    pub element: String,
    /// 状態
    pub state: CarriedState,
}

impl CarriedReference {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let topic = object.required("topic", json::string)?;
        let location = object.required("location", json::string)?;
        let element = object.required("element", json::string)?;
        let state = object.required("state", CarriedState::from_json)?;
        Ok(Self {
            topic,
            location,
            element,
            state,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("topic");
        out.string(&self.topic);
        out.key("location");
        out.string(&self.location);
        out.key("element");
        out.string(&self.element);
        out.key("state");
        self.state.to_json(out);
    }
}

impl FromJson for CarriedReference {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CarriedReference {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しいバージョンに要素がない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CarriedState {
    /// `unchanged`
    Unchanged,
    /// `changed`
    Changed,
    /// `removed`
    Removed,
}

impl CarriedState {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Unchanged, Self::Changed, Self::Removed];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "unchanged" => Some(Self::Unchanged),
            "changed" => Some(Self::Changed),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }
}

impl FromJson for CarriedState {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CarriedState {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。マイナーバージョン 43 から
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAdoptRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 照合する文書の名前。open で作ったもの
    pub doc: String,
    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    pub profiles: Vec<String>,
    /// 照合に使うプロファイル。profiles の中から選ぶ
    pub profile: ProfileRef,
    /// たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる
    pub from: Option<String>,
    /// 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける
    pub view: Option<u64>,
    /// 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。マイナーバージョン 40 から
    pub file_name: Option<String>,
    /// 色の表の入力の名前（zm_push で渡したもの）、または組み込みの表 builtin:cad-v1。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く
    pub palette: Option<String>,
    /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
    pub review: Option<String>,
    /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
    pub target: String,
    /// 取り込む指摘の番号の列。check の指摘の並びの順で、0 から数える
    pub findings: Vec<u64>,
}

impl ReviewAdoptRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "review_adopt")?;
        let doc = object.required("doc", json::string)?;
        let profiles = object.required("profiles", |value| json::array(value, json::string))?;
        let profile = object.required("profile", ProfileRef::from_json)?;
        let from = object.optional("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let file_name = object.optional("file_name", json::string)?;
        let palette = object.optional("palette", json::string)?;
        let review = object.optional("review", json::string)?;
        let target = object.required("target", json::string)?;
        let findings = object.required("findings", |value| {
            json::array(value, |value| json::uint(value, 0, 9007199254740991))
        })?;
        Ok(Self {
            request_common,
            doc,
            profiles,
            profile,
            from,
            view,
            file_name,
            palette,
            review,
            target,
            findings,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("review_adopt");
        out.key("doc");
        out.string(&self.doc);
        out.key("profiles");
        out.array(&self.profiles, |out, item| out.string(item));
        out.key("profile");
        self.profile.to_json(out);
        if let Some(value) = &self.from {
            out.key("from");
            out.string(value);
        }
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        if let Some(value) = &self.file_name {
            out.key("file_name");
            out.string(value);
        }
        if let Some(value) = &self.palette {
            out.key("palette");
            out.string(value);
        }
        if let Some(value) = &self.review {
            out.key("review");
            out.string(value);
        }
        out.key("target");
        out.string(&self.target);
        out.key("findings");
        out.array(&self.findings, |out, item| out.uint(*item));
    }
}

impl FromJson for ReviewAdoptRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewAdoptRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 取り込んだ結果。足した文書は blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAdoptResult {
    /// 足した指摘の ID。取り込み済みで足さなかった指摘は持たない
    pub added: Vec<String>,
    /// 足した後の文書の指摘の数
    pub topics: u64,
}

impl ReviewAdoptResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let added = object.required("added", |value| json::array(value, json::string))?;
        let topics = object.required("topics", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { added, topics })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("added");
        out.array(&self.added, |out, item| out.string(item));
        out.key("topics");
        out.uint(self.topics);
    }
}

impl FromJson for ReviewAdoptResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewAdoptResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レビューの文書（決定 0214・0295）の 1 つの対象の図形と指摘の場所を、開いた図面の起点で、図面とは別の 2D のシーンにして blob で返す。対象の形式とハッシュは開いた文書から作り、文書の対象と合わなければ描かない。シーンの配置は scene と同じ schema/scene.toml。色・線幅・寸法は style で明示し、文書の値や旧版の値から推測しない。レビューの文書と図面は変えない。マイナーバージョン 95 から
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewSceneRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 対象の図面の文書の名前。open で作ったもの
    pub doc: String,
    /// レビューの文書の入力の名前（zm_push で渡した JSON）
    pub review: String,
    /// 描く対象の ID。レビューの文書の targets の id
    pub target: String,
    /// 起点。実寸の空間・記号の定義の空間・用紙の ID（scene と同じ）。文書の図形・場所の view がこの起点を指すものだけを描く
    pub from: String,
    /// 起点の用紙のビューの番号。0 から数える。from が用紙のときだけ書ける。書けば、文書の view が用紙の ID に /view/<番号> を足した値の項目だけを描く
    pub view: Option<u64>,
    /// [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す
    pub region: Option<Vec<f64>>,
    /// 弦の誤差（起点の長さ）。0 より大きい有限値。既定値を補わない
    pub tolerance: f64,
    /// 文字を置くフォント（scene と同じ）。style の font の名前で引く。書かなければ字形を置かず、注記の枠を文字高と字数から概算して診断する
    pub fonts: Option<Vec<SceneFont>>,
    /// 選んだ項目に共通で当てる色と寸法
    pub style: ReviewSceneStyle,
}

impl ReviewSceneRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "review_scene")?;
        let doc = object.required("doc", json::string)?;
        let review = object.required("review", json::string)?;
        let target = object.required("target", json::string)?;
        let from = object.required("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let region = object.optional("region", |value| json::array(value, json::number))?;
        let tolerance = object.required("tolerance", json::number)?;
        let fonts = object.optional("fonts", |value| json::array(value, SceneFont::from_json))?;
        let style = object.required("style", ReviewSceneStyle::from_json)?;
        Ok(Self {
            request_common,
            doc,
            review,
            target,
            from,
            view,
            region,
            tolerance,
            fonts,
            style,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("review_scene");
        out.key("doc");
        out.string(&self.doc);
        out.key("review");
        out.string(&self.review);
        out.key("target");
        out.string(&self.target);
        out.key("from");
        out.string(&self.from);
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        if let Some(value) = &self.region {
            out.key("region");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("tolerance");
        out.float(self.tolerance);
        if let Some(value) = &self.fonts {
            out.key("fonts");
            out.array(value, |out, item| item.to_json(out));
        }
        out.key("style");
        self.style.to_json(out);
    }
}

impl FromJson for ReviewSceneRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewSceneRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レビューの図形の色と寸法。幾何の寸法は起点の単位、線幅だけは用紙の mm。文書には保存しない
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewSceneStyle {
    /// 線・面・文字の色。#rrggbb
    pub color: String,
    /// 用紙の mm の線幅。0 以上の有限値。0 はもっとも細い線
    pub line_weight: f64,
    /// 起点の単位での文字の高さ。正の有限値
    pub text_height: f64,
    /// 起点の単位での注記の枠の余白。0 以上の有限値
    pub text_padding: f64,
    /// 起点の単位での矢じりの長さ。正の有限値
    pub arrow_length: f64,
    /// 赤線の矢じりを置く端
    pub arrows: ReviewArrows,
    /// 雲の円弧の弦の長さの上限。起点の単位の正の有限値
    pub cloud_pitch: f64,
    /// 写真と指摘の位置の記号の直径。起点の単位の正の有限値
    pub marker_size: f64,
    /// fonts から引くフォントの名前。書かなければ名前のないフォント
    pub font: Option<String>,
}

impl ReviewSceneStyle {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let color = object.required("color", json::string)?;
        let line_weight = object.required("line_weight", json::number)?;
        let text_height = object.required("text_height", json::number)?;
        let text_padding = object.required("text_padding", json::number)?;
        let arrow_length = object.required("arrow_length", json::number)?;
        let arrows = object.required("arrows", ReviewArrows::from_json)?;
        let cloud_pitch = object.required("cloud_pitch", json::number)?;
        let marker_size = object.required("marker_size", json::number)?;
        let font = object.optional("font", json::string)?;
        Ok(Self {
            color,
            line_weight,
            text_height,
            text_padding,
            arrow_length,
            arrows,
            cloud_pitch,
            marker_size,
            font,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("color");
        out.string(&self.color);
        out.key("line_weight");
        out.float(self.line_weight);
        out.key("text_height");
        out.float(self.text_height);
        out.key("text_padding");
        out.float(self.text_padding);
        out.key("arrow_length");
        out.float(self.arrow_length);
        out.key("arrows");
        self.arrows.to_json(out);
        out.key("cloud_pitch");
        out.float(self.cloud_pitch);
        out.key("marker_size");
        out.float(self.marker_size);
        if let Some(value) = &self.font {
            out.key("font");
            out.string(value);
        }
    }
}

impl FromJson for ReviewSceneStyle {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewSceneStyle {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 赤線の矢じりを置く端。none は置かない、start は最初の端点、end は最後の端点、both は両端
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReviewArrows {
    /// `none`
    None,
    /// `start`
    Start,
    /// `end`
    End,
    /// `both`
    Both,
}

impl ReviewArrows {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::None, Self::Start, Self::End, Self::Both];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Start => "start",
            Self::End => "end",
            Self::Both => "both",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "start" => Some(Self::Start),
            "end" => Some(Self::End),
            "both" => Some(Self::Both),
            _ => None,
        }
    }
}

impl FromJson for ReviewArrows {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for ReviewArrows {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// レビューのシーンの要約と、シーンの blob の番号、シーンの ID から元の図形・場所への対応
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewSceneResult {
    /// 起点の ID
    pub from: String,
    /// 起点のビューの番号。ビューでなければ持たない
    pub view: Option<u64>,
    /// 起点の座標の単位。用紙とビューからは millimeter。単位の不明な空間は unknown のまま
    pub unit: Unit,
    /// 使った弦の誤差（起点の長さ）
    pub tolerance: f64,
    /// 返した描くものの数
    pub counts: SceneCounts,
    /// 返した塊の範囲。起点の座標。塊がなければ持たない
    pub extent: Option<Extent>,
    /// 描いた図形・場所ごとの対応。描く順（文書の markups の順、続いて topics と各指摘の locations の順）
    pub sources: Vec<ReviewSceneSource>,
    /// シーンの blob の番号（種類は scene）。zm_take で取り出す
    pub blob: u64,
}

impl ReviewSceneResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let from = object.required("from", json::string)?;
        let view = object.optional("view", |value| json::uint(value, 0, 9007199254740991))?;
        let unit = object.required("unit", Unit::from_json)?;
        let tolerance = object.required("tolerance", json::number)?;
        let counts = object.required("counts", SceneCounts::from_json)?;
        let extent = object.optional("extent", Extent::from_json)?;
        let sources = object.required("sources", |value| {
            json::array(value, ReviewSceneSource::from_json)
        })?;
        let blob = object.required("blob", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            from,
            view,
            unit,
            tolerance,
            counts,
            extent,
            sources,
            blob,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("from");
        out.string(&self.from);
        if let Some(value) = &self.view {
            out.key("view");
            out.uint(*value);
        }
        out.key("unit");
        self.unit.to_json(out);
        out.key("tolerance");
        out.float(self.tolerance);
        out.key("counts");
        self.counts.to_json(out);
        if let Some(value) = &self.extent {
            out.key("extent");
            value.to_json(out);
        }
        out.key("sources");
        out.array(&self.sources, |out, item| item.to_json(out));
        out.key("blob");
        out.uint(self.blob);
    }
}

impl FromJson for ReviewSceneResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewSceneResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// シーンの ID（シーンの entity_ids）から、元のレビューの文書の図形か指摘の場所へ戻す対応
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewSceneSource {
    /// シーンの ID。review/markup/<hex> か review/location/<hex>/<hex>
    pub id: String,
    /// 元の項目の種類
    pub kind: ReviewSceneSourceKind,
    /// 元の図形の ID。kind が markup のときだけ持つ
    pub markup: Option<String>,
    /// 元の指摘の ID。kind が location のときだけ持つ
    pub topic: Option<String>,
    /// 元の場所の ID。kind が location のときだけ持つ
    pub location: Option<String>,
}

impl ReviewSceneSource {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let kind = object.required("kind", ReviewSceneSourceKind::from_json)?;
        let markup = object.optional("markup", json::string)?;
        let topic = object.optional("topic", json::string)?;
        let location = object.optional("location", json::string)?;
        Ok(Self {
            id,
            kind,
            markup,
            topic,
            location,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.markup {
            out.key("markup");
            out.string(value);
        }
        if let Some(value) = &self.topic {
            out.key("topic");
            out.string(value);
        }
        if let Some(value) = &self.location {
            out.key("location");
            out.string(value);
        }
    }
}

impl FromJson for ReviewSceneSource {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewSceneSource {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// シーンの ID の元の項目の種類。markup はレビューの文書の図形、location は指摘の中の場所
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReviewSceneSourceKind {
    /// `markup`
    Markup,
    /// `location`
    Location,
}

impl ReviewSceneSourceKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Markup, Self::Location];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Markup => "markup",
            Self::Location => "location",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "markup" => Some(Self::Markup),
            "location" => Some(Self::Location),
            _ => None,
        }
    }
}

impl FromJson for ReviewSceneSourceKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for ReviewSceneSourceKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。マイナーバージョン 44 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で IFC を開いたもの
    pub doc: String,
}

impl BuildingRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "building")?;
        let doc = object.required("doc", json::string)?;
        Ok(Self {
            request_common,
            doc,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("building");
        out.key("doc");
        out.string(&self.doc);
    }
}

impl FromJson for BuildingRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 建物モデルの要約
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingResult {
    /// IFC のスキーマの名前（FILE_SCHEMA の最初）
    pub schema: String,
    /// 入力のバイト列のハッシュ。review_carry の対象のバージョンに使う
    pub hash: String,
    /// プロジェクトの要素の ID。なければ持たない
    pub project: Option<String>,
    /// プロジェクトの名前。なければ持たない
    pub project_name: Option<String>,
    /// プロジェクトの単位
    pub units: Vec<BuildingUnit>,
    /// 要素の数
    pub elements: u64,
    /// クラスごとの要素の数。クラスの名前の順
    pub classes: Vec<ClassCount>,
}

impl BuildingResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let schema = object.required("schema", json::string)?;
        let hash = object.required("hash", json::string)?;
        let project = object.optional("project", json::string)?;
        let project_name = object.optional("project_name", json::string)?;
        let units =
            object.required("units", |value| json::array(value, BuildingUnit::from_json))?;
        let elements =
            object.required("elements", |value| json::uint(value, 0, 9007199254740991))?;
        let classes =
            object.required("classes", |value| json::array(value, ClassCount::from_json))?;
        Ok(Self {
            schema,
            hash,
            project,
            project_name,
            units,
            elements,
            classes,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("schema");
        out.string(&self.schema);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.project {
            out.key("project");
            out.string(value);
        }
        if let Some(value) = &self.project_name {
            out.key("project_name");
            out.string(value);
        }
        out.key("units");
        out.array(&self.units, |out, item| item.to_json(out));
        out.key("elements");
        out.uint(self.elements);
        out.key("classes");
        out.array(&self.classes, |out, item| item.to_json(out));
    }
}

impl FromJson for BuildingResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 単位
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingUnit {
    /// 単位の種類（LENGTHUNIT・AREAUNIT など）
    pub kind: String,
    /// SI の接頭辞（MILLI など）
    pub prefix: Option<String>,
    /// 単位の名前（METRE、換算する単位なら inch など）
    pub name: String,
    /// 換算する単位の、SI の単位での大きさ。SI の単位は持たない
    pub scale: Option<f64>,
}

impl BuildingUnit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", json::string)?;
        let prefix = object.optional("prefix", json::string)?;
        let name = object.required("name", json::string)?;
        let scale = object.optional("scale", json::number)?;
        Ok(Self {
            kind,
            prefix,
            name,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string(&self.kind);
        if let Some(value) = &self.prefix {
            out.key("prefix");
            out.string(value);
        }
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.scale {
            out.key("scale");
            out.float(*value);
        }
    }
}

impl FromJson for BuildingUnit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingUnit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// クラスの要素の数
#[derive(Clone, Debug, PartialEq)]
pub struct ClassCount {
    /// IFC のクラスの名前（IfcWall など）
    pub name: String,
    /// 要素の数
    pub count: u64,
}

impl ClassCount {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let count = object.required("count", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { name, count })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("count");
        out.uint(self.count);
    }
}

impl FromJson for ClassCount {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ClassCount {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた IFC の建物モデルの要素を、書いた順に返す。コンポーネントは components で選ぶ（決定 0207・0219）。マイナーバージョン 44 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingElementsRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で IFC を開いたもの
    pub doc: String,
    /// 返す要素の ID。書けば、その要素だけを、書いた順に返す。ない ID は返さない
    pub ids: Option<Vec<String>>,
    /// IFC のクラスの名前。書けば、そのクラスの要素だけを返す。大文字と小文字を区別しない
    pub class: Option<String>,
    /// 返すコンポーネントの名前。1 つ以上で、重ねない。書かなければ class と identity を返す
    pub components: Option<Vec<BuildingComponent>>,
    /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す数の上限。書かなければ 1000
    pub limit: Option<u64>,
}

impl BuildingElementsRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "building_elements")?;
        let doc = object.required("doc", json::string)?;
        let ids = object.optional("ids", |value| json::array(value, json::string))?;
        let class = object.optional("class", json::string)?;
        let components = object.optional("components", |value| {
            json::array(value, BuildingComponent::from_json)
        })?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        Ok(Self {
            request_common,
            doc,
            ids,
            class,
            components,
            offset,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("building_elements");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.ids {
            out.key("ids");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.class {
            out.key("class");
            out.string(value);
        }
        if let Some(value) = &self.components {
            out.key("components");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for BuildingElementsRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingElementsRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// building_elements で返すコンポーネント（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildingComponent {
    /// `identity`
    Identity,
    /// `class`
    Class,
    /// `classification`
    Classification,
    /// `spatial`
    Spatial,
    /// `parent`
    Parent,
    /// `placement`
    Placement,
    /// `type_of`
    TypeOf,
    /// `properties`
    Properties,
    /// `quantities`
    Quantities,
    /// `material`
    Material,
    /// `opening`
    Opening,
    /// `group`
    Group,
    /// `connection`
    Connection,
    /// `level`
    Level,
    /// `zone`
    Zone,
    /// `style`
    Style,
    /// `relations`
    Relations,
}

impl BuildingComponent {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Identity,
        Self::Class,
        Self::Classification,
        Self::Spatial,
        Self::Parent,
        Self::Placement,
        Self::TypeOf,
        Self::Properties,
        Self::Quantities,
        Self::Material,
        Self::Opening,
        Self::Group,
        Self::Connection,
        Self::Level,
        Self::Zone,
        Self::Style,
        Self::Relations,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Class => "class",
            Self::Classification => "classification",
            Self::Spatial => "spatial",
            Self::Parent => "parent",
            Self::Placement => "placement",
            Self::TypeOf => "type_of",
            Self::Properties => "properties",
            Self::Quantities => "quantities",
            Self::Material => "material",
            Self::Opening => "opening",
            Self::Group => "group",
            Self::Connection => "connection",
            Self::Level => "level",
            Self::Zone => "zone",
            Self::Style => "style",
            Self::Relations => "relations",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "identity" => Some(Self::Identity),
            "class" => Some(Self::Class),
            "classification" => Some(Self::Classification),
            "spatial" => Some(Self::Spatial),
            "parent" => Some(Self::Parent),
            "placement" => Some(Self::Placement),
            "type_of" => Some(Self::TypeOf),
            "properties" => Some(Self::Properties),
            "quantities" => Some(Self::Quantities),
            "material" => Some(Self::Material),
            "opening" => Some(Self::Opening),
            "group" => Some(Self::Group),
            "connection" => Some(Self::Connection),
            "level" => Some(Self::Level),
            "zone" => Some(Self::Zone),
            "style" => Some(Self::Style),
            "relations" => Some(Self::Relations),
            _ => None,
        }
    }
}

impl FromJson for BuildingComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for BuildingComponent {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 要素の一覧。offset から limit 個まで
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingElementsResult {
    /// 要素
    pub elements: Vec<BuildingElement>,
    /// 選んだ要素の数
    pub total: u64,
    /// 続きの最初の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl BuildingElementsResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let elements = object.required("elements", |value| {
            json::array(value, BuildingElement::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            elements,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("elements");
        out.array(&self.elements, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for BuildingElementsResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingElementsResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 建物モデルの要素。コンポーネントは、選んだもののうち要素が持つものだけを持つ
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingElement {
    /// 要素の ID（GlobalId か、i と番号）
    pub id: String,
    /// 要素のコンポーネントのハッシュ（v1: と 32 桁の 16 進、決定 0207）
    pub hash: String,
    /// 名前・説明・タグ・長い名前
    pub identity: Option<IdentityComponent>,
    /// クラス
    pub class: Option<ClassComponent>,
    /// 分類の参照
    pub classification: Option<Vec<ClassificationReference>>,
    /// 空間の要素の種類と高さ
    pub spatial: Option<SpatialComponent>,
    /// 親と、参照される空間
    pub parent: Option<ParentComponent>,
    /// 配置
    pub placement: Option<PlacementComponent>,
    /// 型の要素の ID
    pub type_of: Option<String>,
    /// プロパティのセット
    pub properties: Option<Vec<PropertySet>>,
    /// 数量のセット
    pub quantities: Option<Vec<QuantitySet>>,
    /// 材料
    pub material: Option<MaterialComponent>,
    /// 開口と、埋める要素
    pub opening: Option<OpeningComponent>,
    /// 属するグループの要素の ID
    pub group: Option<Vec<String>>,
    /// 接続
    pub connection: Option<Vec<ConnectionLink>>,
    /// レベル
    pub level: Option<LevelComponent>,
    /// 区画
    pub zone: Option<ZoneComponent>,
    /// 表示のスタイル
    pub style: Option<StyleComponent>,
    /// 索引から求める関係
    pub relations: Option<RelationsComponent>,
}

impl BuildingElement {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let identity = object.optional("identity", IdentityComponent::from_json)?;
        let class = object.optional("class", ClassComponent::from_json)?;
        let classification = object.optional("classification", |value| {
            json::array(value, ClassificationReference::from_json)
        })?;
        let spatial = object.optional("spatial", SpatialComponent::from_json)?;
        let parent = object.optional("parent", ParentComponent::from_json)?;
        let placement = object.optional("placement", PlacementComponent::from_json)?;
        let type_of = object.optional("type_of", json::string)?;
        let properties = object.optional("properties", |value| {
            json::array(value, PropertySet::from_json)
        })?;
        let quantities = object.optional("quantities", |value| {
            json::array(value, QuantitySet::from_json)
        })?;
        let material = object.optional("material", MaterialComponent::from_json)?;
        let opening = object.optional("opening", OpeningComponent::from_json)?;
        let group = object.optional("group", |value| json::array(value, json::string))?;
        let connection = object.optional("connection", |value| {
            json::array(value, ConnectionLink::from_json)
        })?;
        let level = object.optional("level", LevelComponent::from_json)?;
        let zone = object.optional("zone", ZoneComponent::from_json)?;
        let style = object.optional("style", StyleComponent::from_json)?;
        let relations = object.optional("relations", RelationsComponent::from_json)?;
        Ok(Self {
            id,
            hash,
            identity,
            class,
            classification,
            spatial,
            parent,
            placement,
            type_of,
            properties,
            quantities,
            material,
            opening,
            group,
            connection,
            level,
            zone,
            style,
            relations,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.identity {
            out.key("identity");
            value.to_json(out);
        }
        if let Some(value) = &self.class {
            out.key("class");
            value.to_json(out);
        }
        if let Some(value) = &self.classification {
            out.key("classification");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.spatial {
            out.key("spatial");
            value.to_json(out);
        }
        if let Some(value) = &self.parent {
            out.key("parent");
            value.to_json(out);
        }
        if let Some(value) = &self.placement {
            out.key("placement");
            value.to_json(out);
        }
        if let Some(value) = &self.type_of {
            out.key("type_of");
            out.string(value);
        }
        if let Some(value) = &self.properties {
            out.key("properties");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.quantities {
            out.key("quantities");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.material {
            out.key("material");
            value.to_json(out);
        }
        if let Some(value) = &self.opening {
            out.key("opening");
            value.to_json(out);
        }
        if let Some(value) = &self.group {
            out.key("group");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.connection {
            out.key("connection");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.level {
            out.key("level");
            value.to_json(out);
        }
        if let Some(value) = &self.zone {
            out.key("zone");
            value.to_json(out);
        }
        if let Some(value) = &self.style {
            out.key("style");
            value.to_json(out);
        }
        if let Some(value) = &self.relations {
            out.key("relations");
            value.to_json(out);
        }
    }
}

impl FromJson for BuildingElement {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingElement {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 名前・説明・タグ・長い名前
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IdentityComponent {
    /// 名前
    pub name: Option<String>,
    /// 説明
    pub description: Option<String>,
    /// タグ
    pub tag: Option<String>,
    /// 長い名前
    pub long_name: Option<String>,
}

impl IdentityComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let tag = object.optional("tag", json::string)?;
        let long_name = object.optional("long_name", json::string)?;
        Ok(Self {
            name,
            description,
            tag,
            long_name,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        if let Some(value) = &self.tag {
            out.key("tag");
            out.string(value);
        }
        if let Some(value) = &self.long_name {
            out.key("long_name");
            out.string(value);
        }
    }
}

impl FromJson for IdentityComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdentityComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// クラス
#[derive(Clone, Debug, PartialEq)]
pub struct ClassComponent {
    /// IFC4 のクラスの名前。IFC2X3 の StandardCase などは、元の名前を original に残して直す
    pub name: String,
    /// PredefinedType
    pub predefined: Option<String>,
    /// ObjectType など、USERDEFINED のときの名前
    pub user_defined: Option<String>,
    /// ファイルに書いた実体の名前
    pub original: String,
}

impl ClassComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let predefined = object.optional("predefined", json::string)?;
        let user_defined = object.optional("user_defined", json::string)?;
        let original = object.required("original", json::string)?;
        Ok(Self {
            name,
            predefined,
            user_defined,
            original,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.predefined {
            out.key("predefined");
            out.string(value);
        }
        if let Some(value) = &self.user_defined {
            out.key("user_defined");
            out.string(value);
        }
        out.key("original");
        out.string(&self.original);
    }
}

impl FromJson for ClassComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ClassComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 分類の参照
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassificationReference {
    /// 分類の体系の名前
    pub system: Option<String>,
    /// 体系のバージョン
    pub edition: Option<String>,
    /// 分類の記号
    pub code: Option<String>,
    /// 分類の名前
    pub name: Option<String>,
}

impl ClassificationReference {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let system = object.optional("system", json::string)?;
        let edition = object.optional("edition", json::string)?;
        let code = object.optional("code", json::string)?;
        let name = object.optional("name", json::string)?;
        Ok(Self {
            system,
            edition,
            code,
            name,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.system {
            out.key("system");
            out.string(value);
        }
        if let Some(value) = &self.edition {
            out.key("edition");
            out.string(value);
        }
        if let Some(value) = &self.code {
            out.key("code");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
    }
}

impl FromJson for ClassificationReference {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ClassificationReference {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 単位とともに持つ値
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    /// 値
    pub value: f64,
    /// 単位。解けなければ持たない
    pub unit: Option<BuildingUnit>,
}

impl Measured {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let value = object.required("value", json::number)?;
        let unit = object.optional("unit", BuildingUnit::from_json)?;
        Ok(Self { value, unit })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("value");
        out.float(self.value);
        if let Some(value) = &self.unit {
            out.key("unit");
            value.to_json(out);
        }
    }
}

impl FromJson for Measured {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Measured {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 空間の要素
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialComponent {
    /// site・building・storey・space・spatial_zone か、元の実体の名前
    pub kind: String,
    /// 高さ
    pub elevation: Option<Measured>,
    /// 基準の高さ
    pub ref_elevation: Option<Measured>,
}

impl SpatialComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", json::string)?;
        let elevation = object.optional("elevation", Measured::from_json)?;
        let ref_elevation = object.optional("ref_elevation", Measured::from_json)?;
        Ok(Self {
            kind,
            elevation,
            ref_elevation,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string(&self.kind);
        if let Some(value) = &self.elevation {
            out.key("elevation");
            value.to_json(out);
        }
        if let Some(value) = &self.ref_elevation {
            out.key("ref_elevation");
            value.to_json(out);
        }
    }
}

impl FromJson for SpatialComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SpatialComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 親
#[derive(Clone, Debug, PartialEq)]
pub struct ParentComponent {
    /// 親の要素
    pub of: Option<Membership>,
    /// 参照される空間の要素の ID
    pub referenced_in: Vec<String>,
}

impl ParentComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let of = object.optional("of", Membership::from_json)?;
        let referenced_in =
            object.required("referenced_in", |value| json::array(value, json::string))?;
        Ok(Self { of, referenced_in })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.of {
            out.key("of");
            value.to_json(out);
        }
        out.key("referenced_in");
        out.array(&self.referenced_in, |out, item| out.string(item));
    }
}

impl FromJson for ParentComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ParentComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 親との関係
#[derive(Clone, Debug, PartialEq)]
pub struct Membership {
    /// 親の要素の ID
    pub target: String,
    /// 関係の種類
    pub kind: ParentKind,
    /// 入れ子の順。0 から
    pub order: Option<u64>,
}

impl Membership {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let target = object.required("target", json::string)?;
        let kind = object.required("kind", ParentKind::from_json)?;
        let order = object.optional("order", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            target,
            kind,
            order,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("target");
        out.string(&self.target);
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.order {
            out.key("order");
            out.uint(*value);
        }
    }
}

impl FromJson for Membership {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Membership {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 配置
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlacementComponent {
    /// 配置の親の要素の ID
    pub relative_to: Option<String>,
    /// 親をたどった世界の変換。3 行 4 列（回転と移動）。解けなければ持たない
    pub world: Option<Vec<Vec<f64>>>,
    /// 解けなかった配置の実体の名前
    pub unsupported: Option<String>,
}

impl PlacementComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let relative_to = object.optional("relative_to", json::string)?;
        let world = object.optional("world", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let unsupported = object.optional("unsupported", json::string)?;
        Ok(Self {
            relative_to,
            world,
            unsupported,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.relative_to {
            out.key("relative_to");
            out.string(value);
        }
        if let Some(value) = &self.world {
            out.key("world");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.float(*item))
            });
        }
        if let Some(value) = &self.unsupported {
            out.key("unsupported");
            out.string(value);
        }
    }
}

impl FromJson for PlacementComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PlacementComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 値
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingValue {
    /// 値の型の名前（IfcLengthMeasure・IfcLabel など）
    pub type_name: Option<String>,
    /// 値の種類
    pub kind: BuildingValueKind,
    /// real と、2^53 − 1 までの integer の値
    pub number: Option<f64>,
    /// text と enumeration の値と、2^53 − 1 を超える integer の値
    pub text: Option<String>,
    /// boolean と logical の値。logical の不明は持たない
    pub boolean: Option<bool>,
    /// element の要素の ID
    pub element: Option<String>,
    /// list の値
    pub items: Option<Vec<BuildingValue>>,
}

impl BuildingValue {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let type_name = object.optional("type_name", json::string)?;
        let kind = object.required("kind", BuildingValueKind::from_json)?;
        let number = object.optional("number", json::number)?;
        let text = object.optional("text", json::string)?;
        let boolean = object.optional("boolean", json::boolean)?;
        let element = object.optional("element", json::string)?;
        let items = object.optional("items", |value| {
            json::array(value, BuildingValue::from_json)
        })?;
        Ok(Self {
            type_name,
            kind,
            number,
            text,
            boolean,
            element,
            items,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.type_name {
            out.key("type_name");
            out.string(value);
        }
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.number {
            out.key("number");
            out.float(*value);
        }
        if let Some(value) = &self.text {
            out.key("text");
            out.string(value);
        }
        if let Some(value) = &self.boolean {
            out.key("boolean");
            out.bool(*value);
        }
        if let Some(value) = &self.element {
            out.key("element");
            out.string(value);
        }
        if let Some(value) = &self.items {
            out.key("items");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for BuildingValue {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingValue {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// プロパティのセット
#[derive(Clone, Debug, PartialEq)]
pub struct PropertySet {
    /// 名前
    pub name: String,
    /// プロパティ
    pub properties: Vec<Property>,
}

impl PropertySet {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let properties = object.required("properties", |value| {
            json::array(value, Property::from_json)
        })?;
        Ok(Self { name, properties })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("properties");
        out.array(&self.properties, |out, item| item.to_json(out));
    }
}

impl FromJson for PropertySet {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PropertySet {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// プロパティ
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    /// 名前
    pub name: String,
    /// 値。値がなければ持たない
    pub value: Option<BuildingValue>,
    /// 単位
    pub unit: Option<BuildingUnit>,
}

impl Property {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let value = object.optional("value", BuildingValue::from_json)?;
        let unit = object.optional("unit", BuildingUnit::from_json)?;
        Ok(Self { name, value, unit })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.value {
            out.key("value");
            value.to_json(out);
        }
        if let Some(value) = &self.unit {
            out.key("unit");
            value.to_json(out);
        }
    }
}

impl FromJson for Property {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Property {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 数量のセット
#[derive(Clone, Debug, PartialEq)]
pub struct QuantitySet {
    /// 名前
    pub name: String,
    /// 計測の方法
    pub method: Option<String>,
    /// 数量
    pub quantities: Vec<Quantity>,
}

impl QuantitySet {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let method = object.optional("method", json::string)?;
        let quantities = object.required("quantities", |value| {
            json::array(value, Quantity::from_json)
        })?;
        Ok(Self {
            name,
            method,
            quantities,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.method {
            out.key("method");
            out.string(value);
        }
        out.key("quantities");
        out.array(&self.quantities, |out, item| item.to_json(out));
    }
}

impl FromJson for QuantitySet {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for QuantitySet {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 数量
#[derive(Clone, Debug, PartialEq)]
pub struct Quantity {
    /// 名前
    pub name: String,
    /// 種類
    pub kind: QuantityKind,
    /// 値
    pub value: Measured,
    /// 式
    pub formula: Option<String>,
}

impl Quantity {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let kind = object.required("kind", QuantityKind::from_json)?;
        let value = object.required("value", Measured::from_json)?;
        let formula = object.optional("formula", json::string)?;
        Ok(Self {
            name,
            kind,
            value,
            formula,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("kind");
        self.kind.to_json(out);
        out.key("value");
        self.value.to_json(out);
        if let Some(value) = &self.formula {
            out.key("formula");
            out.string(value);
        }
    }
}

impl FromJson for Quantity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Quantity {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 材料
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialComponent {
    /// 材料の組み方
    pub kind: MaterialKind,
    /// 部分
    pub parts: Vec<MaterialPart>,
    /// 層の向き
    pub direction: Option<String>,
    /// 層の基準からのずれ
    pub offset: Option<Measured>,
}

impl MaterialComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", MaterialKind::from_json)?;
        let parts =
            object.required("parts", |value| json::array(value, MaterialPart::from_json))?;
        let direction = object.optional("direction", json::string)?;
        let offset = object.optional("offset", Measured::from_json)?;
        Ok(Self {
            kind,
            parts,
            direction,
            offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        self.kind.to_json(out);
        out.key("parts");
        out.array(&self.parts, |out, item| item.to_json(out));
        if let Some(value) = &self.direction {
            out.key("direction");
            out.string(value);
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            value.to_json(out);
        }
    }
}

impl FromJson for MaterialComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MaterialComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 材料の部分
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialPart {
    /// 材料の要素の ID
    pub material: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// 厚さ
    pub thickness: Option<Measured>,
}

impl MaterialPart {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let material = object.optional("material", json::string)?;
        let name = object.optional("name", json::string)?;
        let thickness = object.optional("thickness", Measured::from_json)?;
        Ok(Self {
            material,
            name,
            thickness,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.material {
            out.key("material");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.thickness {
            out.key("thickness");
            value.to_json(out);
        }
    }
}

impl FromJson for MaterialPart {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for MaterialPart {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開口
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpeningComponent {
    /// この開口が穴を開ける要素の ID
    pub voids: Option<String>,
    /// この要素が埋める開口の要素の ID
    pub fills: Option<String>,
}

impl OpeningComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let voids = object.optional("voids", json::string)?;
        let fills = object.optional("fills", json::string)?;
        Ok(Self { voids, fills })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.voids {
            out.key("voids");
            out.string(value);
        }
        if let Some(value) = &self.fills {
            out.key("fills");
            out.string(value);
        }
    }
}

impl FromJson for OpeningComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for OpeningComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 接続
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionLink {
    /// 関係の実体の名前
    pub kind: String,
    /// 接続する要素の ID
    pub target: String,
}

impl ConnectionLink {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", json::string)?;
        let target = object.required("target", json::string)?;
        Ok(Self { kind, target })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string(&self.kind);
        out.key("target");
        out.string(&self.target);
    }
}

impl FromJson for ConnectionLink {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ConnectionLink {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レベル
#[derive(Clone, Debug, PartialEq)]
pub struct LevelComponent {
    /// 名前
    pub name: Option<String>,
    /// 種類
    pub kind: Option<String>,
    /// 高さ
    pub height: Measured,
    /// 基準
    pub datum: Option<String>,
}

impl LevelComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let kind = object.optional("kind", json::string)?;
        let height = object.required("height", Measured::from_json)?;
        let datum = object.optional("datum", json::string)?;
        Ok(Self {
            name,
            kind,
            height,
            datum,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.kind {
            out.key("kind");
            out.string(value);
        }
        out.key("height");
        self.height.to_json(out);
        if let Some(value) = &self.datum {
            out.key("datum");
            out.string(value);
        }
    }
}

impl FromJson for LevelComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LevelComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 区画
#[derive(Clone, Debug, PartialEq)]
pub struct ZoneComponent {
    /// 区画の種類
    pub kind: Option<String>,
    /// 構成する要素の ID
    pub members: Vec<String>,
}

impl ZoneComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.optional("kind", json::string)?;
        let members = object.required("members", |value| json::array(value, json::string))?;
        Ok(Self { kind, members })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.kind {
            out.key("kind");
            out.string(value);
        }
        out.key("members");
        out.array(&self.members, |out, item| out.string(item));
    }
}

impl FromJson for ZoneComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ZoneComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 表示のスタイル
#[derive(Clone, Debug, PartialEq)]
pub struct StyleComponent {
    /// 面の色の RGB（0 から 1）
    pub surface_color: Option<Vec<f64>>,
    /// 透明度（0 から 1）
    pub transparency: Option<f64>,
    /// 線の色の RGB（0 から 1）
    pub curve_color: Option<Vec<f64>>,
    /// 提示のレイヤの名前
    pub layers: Vec<String>,
}

impl StyleComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let surface_color =
            object.optional("surface_color", |value| json::array(value, json::number))?;
        let transparency = object.optional("transparency", json::number)?;
        let curve_color =
            object.optional("curve_color", |value| json::array(value, json::number))?;
        let layers = object.required("layers", |value| json::array(value, json::string))?;
        Ok(Self {
            surface_color,
            transparency,
            curve_color,
            layers,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.surface_color {
            out.key("surface_color");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.transparency {
            out.key("transparency");
            out.float(*value);
        }
        if let Some(value) = &self.curve_color {
            out.key("curve_color");
            out.array(value, |out, item| out.float(*item));
        }
        out.key("layers");
        out.array(&self.layers, |out, item| out.string(item));
    }
}

impl FromJson for StyleComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for StyleComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 索引から求める関係。要素の ID の列
#[derive(Clone, Debug, PartialEq)]
pub struct RelationsComponent {
    /// 子（集約・入れ子・包含）
    pub children: Vec<String>,
    /// 参照する要素
    pub referenced: Vec<String>,
    /// 型の実例
    pub instances: Vec<String>,
    /// 開口
    pub openings: Vec<String>,
    /// 開口を埋める要素
    pub fillings: Vec<String>,
    /// グループ・システム・区画の構成員
    pub members: Vec<String>,
}

impl RelationsComponent {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let children = object.required("children", |value| json::array(value, json::string))?;
        let referenced = object.required("referenced", |value| json::array(value, json::string))?;
        let instances = object.required("instances", |value| json::array(value, json::string))?;
        let openings = object.required("openings", |value| json::array(value, json::string))?;
        let fillings = object.required("fillings", |value| json::array(value, json::string))?;
        let members = object.required("members", |value| json::array(value, json::string))?;
        Ok(Self {
            children,
            referenced,
            instances,
            openings,
            fillings,
            members,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("children");
        out.array(&self.children, |out, item| out.string(item));
        out.key("referenced");
        out.array(&self.referenced, |out, item| out.string(item));
        out.key("instances");
        out.array(&self.instances, |out, item| out.string(item));
        out.key("openings");
        out.array(&self.openings, |out, item| out.string(item));
        out.key("fillings");
        out.array(&self.fillings, |out, item| out.string(item));
        out.key("members");
        out.array(&self.members, |out, item| out.string(item));
    }
}

impl FromJson for RelationsComponent {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for RelationsComponent {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 親との関係の種類。集約・入れ子・包含
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParentKind {
    /// `aggregates`
    Aggregates,
    /// `nests`
    Nests,
    /// `contained`
    Contained,
}

impl ParentKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Aggregates, Self::Nests, Self::Contained];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Aggregates => "aggregates",
            Self::Nests => "nests",
            Self::Contained => "contained",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "aggregates" => Some(Self::Aggregates),
            "nests" => Some(Self::Nests),
            "contained" => Some(Self::Contained),
            _ => None,
        }
    }
}

impl FromJson for ParentKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for ParentKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 値の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildingValueKind {
    /// `real`
    Real,
    /// `integer`
    Integer,
    /// `text`
    Text,
    /// `boolean`
    Boolean,
    /// `logical`
    Logical,
    /// `enumeration`
    Enumeration,
    /// `element`
    Element,
    /// `list`
    List,
}

impl BuildingValueKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Real,
        Self::Integer,
        Self::Text,
        Self::Boolean,
        Self::Logical,
        Self::Enumeration,
        Self::Element,
        Self::List,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Real => "real",
            Self::Integer => "integer",
            Self::Text => "text",
            Self::Boolean => "boolean",
            Self::Logical => "logical",
            Self::Enumeration => "enumeration",
            Self::Element => "element",
            Self::List => "list",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "real" => Some(Self::Real),
            "integer" => Some(Self::Integer),
            "text" => Some(Self::Text),
            "boolean" => Some(Self::Boolean),
            "logical" => Some(Self::Logical),
            "enumeration" => Some(Self::Enumeration),
            "element" => Some(Self::Element),
            "list" => Some(Self::List),
            _ => None,
        }
    }
}

impl FromJson for BuildingValueKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for BuildingValueKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 数量の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuantityKind {
    /// `length`
    Length,
    /// `area`
    Area,
    /// `volume`
    Volume,
    /// `count`
    Count,
    /// `weight`
    Weight,
    /// `time`
    Time,
}

impl QuantityKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Length,
        Self::Area,
        Self::Volume,
        Self::Count,
        Self::Weight,
        Self::Time,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Length => "length",
            Self::Area => "area",
            Self::Volume => "volume",
            Self::Count => "count",
            Self::Weight => "weight",
            Self::Time => "time",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "length" => Some(Self::Length),
            "area" => Some(Self::Area),
            "volume" => Some(Self::Volume),
            "count" => Some(Self::Count),
            "weight" => Some(Self::Weight),
            "time" => Some(Self::Time),
            _ => None,
        }
    }
}

impl FromJson for QuantityKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for QuantityKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 材料の組み方。単一・層・断面・構成・一覧
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaterialKind {
    /// `single`
    Single,
    /// `layers`
    Layers,
    /// `profiles`
    Profiles,
    /// `constituents`
    Constituents,
    /// `list`
    List,
}

impl MaterialKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Single,
        Self::Layers,
        Self::Profiles,
        Self::Constituents,
        Self::List,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Layers => "layers",
            Self::Profiles => "profiles",
            Self::Constituents => "constituents",
            Self::List => "list",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "single" => Some(Self::Single),
            "layers" => Some(Self::Layers),
            "profiles" => Some(Self::Profiles),
            "constituents" => Some(Self::Constituents),
            "list" => Some(Self::List),
            _ => None,
        }
    }
}

impl FromJson for MaterialKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for MaterialKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 仕様の結果。unknown は確かめられない facet があった
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdsStatus {
    /// `passed`
    Passed,
    /// `failed`
    Failed,
    /// `unknown`
    Unknown,
}

impl IdsStatus {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Passed, Self::Failed, Self::Unknown];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

impl FromJson for IdsStatus {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for IdsStatus {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 開いた IFC に、変更の一覧を当てて書き戻す（決定 0207・0244）。要素は、building_elements が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイル（ifcZIP で開いたものは ifcZIP）を blobs の file で返す。名前を挙げないインスタンスは、元のバイト列のまま書く。マイナーバージョン 58 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingApplyRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で IFC を開いたもの
    pub doc: String,
    /// 読んだときの文書のハッシュ（building の hash）。書けば、今の文書と同じときだけ当てる
    pub hash: Option<String>,
    /// 変更。並べた順に当てる
    pub edits: Vec<BuildingEdit>,
}

impl BuildingApplyRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "building_apply")?;
        let doc = object.required("doc", json::string)?;
        let hash = object.optional("hash", json::string)?;
        let edits =
            object.required("edits", |value| json::array(value, BuildingEdit::from_json))?;
        Ok(Self {
            request_common,
            doc,
            hash,
            edits,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("building_apply");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
        out.key("edits");
        out.array(&self.edits, |out, item| item.to_json(out));
    }
}

impl FromJson for BuildingApplyRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingApplyRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 建物の要素 1 つへの変更。kind の値で形が決まる。add_space・add_element・remove・place・contain・type・void・fill はマイナーバージョン 96 から、add_wall はマイナーバージョン 97 から、add_slab・add_column・add_beam・add_opening・add_door・add_window はマイナーバージョン 98 から
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum BuildingEdit {
    /// 要素のプロパティのセットの、プロパティ（IfcPropertySingleValue）の値を変える。セットかプロパティがなければ足す。ほかの要素と共有するセットは、写しを作って、この要素だけに付け替える。型の要素では、型のセット（HasPropertySets）を変える
    Property(BuildingPropertyEdit),
    /// 要素（IfcRoot）の名前か説明を変える
    Attribute(BuildingAttributeEdit),
    /// 敷地・建物・階を足し、IfcRelAggregates で親に結ぶ。結果の id は足した空間の ID。マイナーバージョン 96 から
    AddSpace(BuildingAddSpaceEdit),
    /// 要素を足し、container があれば IfcRelContainedInSpatialStructure で空間に結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・屋根・階段・斜路・基礎・杭・扉・窓・開口・代理要素（IfcBuildingElementProxy）。結果の id は足した要素の ID。マイナーバージョン 96 から
    AddElement(BuildingAddElementEdit),
    /// 要素を除く。関係からも外し、空になった関係を除く。壁の開口は一緒に除く。子のある空間と、関係のほかから指される要素は除けない。結果は id を持たない。マイナーバージョン 96 から
    Remove(BuildingRemoveEdit),
    /// 要素の配置を置き直す。ほかの要素と共有しない配置はその場で書き換え、共有する配置は新しく作る。配置の親をたどって輪になる配置は断る。マイナーバージョン 96 から
    Place(BuildingPlaceEdit),
    /// 要素を空間に移す。空間は IfcRelAggregates、要素は IfcRelContainedInSpatialStructure で結び、前の関係から外す。マイナーバージョン 96 から
    Contain(BuildingContainEdit),
    /// 要素の型を IfcRelDefinesByType で付け替える。element_type を書かなければ、型を外す。マイナーバージョン 96 から
    Type(BuildingTypeEdit),
    /// 開口（IfcOpeningElement）を要素に IfcRelVoidsElement で結ぶ。1 つの開口は 1 つの要素だけに結ぶ。マイナーバージョン 96 から
    Void(BuildingVoidEdit),
    /// 扉や窓を開口（IfcOpeningElement）に IfcRelFillsElement で結ぶ。1 つの開口は 1 つの要素だけで埋める。マイナーバージョン 96 から
    Fill(BuildingFillEdit),
    /// 芯の 2 点・高さ・材料の層で壁を足し、空間に IfcRelContainedInSpatialStructure で結ぶ。配置（芯の始点を原点、芯の向きを x 軸）・Axis と Body の表現・IfcMaterialLayerSetUsage はコアが書く。IFC2X3 は IfcWallStandardCase、IFC4 と IFC4X3 は IfcWall。結果の id は足した壁の ID。マイナーバージョン 97 から
    AddWall(BuildingAddWallEdit),
    /// 外形・材料の層で床を足し、空間に包含で結ぶ。上の面を配置の原点にし、層の厚さの合計だけ下へ押し出す Body と、IfcSlab の FLOOR、IfcMaterialLayerSetUsage（AXIS3・NEGATIVE）をコアが書く。結果の id は足した床の ID。マイナーバージョン 98 から
    AddSlab(BuildingAddSlabEdit),
    /// 位置・高さ・矩形の断面で柱を足し、空間に包含で結ぶ。断面の中心を配置の原点にして上へ押し出す Body と IfcColumn をコアが書く。結果の id は足した柱の ID。マイナーバージョン 98 から
    AddColumn(BuildingAddColumnEdit),
    /// 天端の芯の 2 点・矩形の断面で梁を足し、空間に包含で結ぶ。始点を配置の原点、芯の向きを x 軸、水平で芯の左を y 軸にし、断面を芯に沿って押し出す Body と IfcBeam をコアが書く。鉛直の芯は断る。結果の id は足した梁の ID。マイナーバージョン 98 から
    AddBeam(BuildingAddBeamEdit),
    /// 壁の芯に沿う位置・下端・矩形の大きさで開口を足し、IfcRelVoidsElement で壁に穴をあける。開口は壁の配置からの相対に置き、壁の材料の層の使い方（AXIS2）の厚さだけ押し出す Body と IfcOpeningElement をコアが書く。空間には所属させない。層の使い方を持たない要素は断る。結果の id は足した開口の ID。マイナーバージョン 98 から
    AddOpening(BuildingAddOpeningEdit),
    /// 開口を扉で埋める。扉は開口の配置に置き、IfcDoor（IFC4 と IFC4X3 は PredefinedType の DOOR）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。形と型はまだ書かない。もう埋まった開口は断る。結果の id は足した扉の ID。マイナーバージョン 98 から
    AddDoor(BuildingAddDoorEdit),
    /// 開口を窓で埋める。窓は開口の配置に置き、IfcWindow（IFC4 と IFC4X3 は PredefinedType の WINDOW）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。形と型はまだ書かない。もう埋まった開口は断る。結果の id は足した窓の ID。マイナーバージョン 98 から
    AddWindow(BuildingAddWindowEdit),
}

impl BuildingEdit {
    /// タグのフィールド（`kind`）の値（スキーマの順）
    pub const TAGS: &[&str] = &[
        "property",
        "attribute",
        "add_space",
        "add_element",
        "remove",
        "place",
        "contain",
        "type",
        "void",
        "fill",
        "add_wall",
        "add_slab",
        "add_column",
        "add_beam",
        "add_opening",
        "add_door",
        "add_window",
    ];

    /// タグのフィールド（`kind`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Property(_) => "property",
            Self::Attribute(_) => "attribute",
            Self::AddSpace(_) => "add_space",
            Self::AddElement(_) => "add_element",
            Self::Remove(_) => "remove",
            Self::Place(_) => "place",
            Self::Contain(_) => "contain",
            Self::Type(_) => "type",
            Self::Void(_) => "void",
            Self::Fill(_) => "fill",
            Self::AddWall(_) => "add_wall",
            Self::AddSlab(_) => "add_slab",
            Self::AddColumn(_) => "add_column",
            Self::AddBeam(_) => "add_beam",
            Self::AddOpening(_) => "add_opening",
            Self::AddDoor(_) => "add_door",
            Self::AddWindow(_) => "add_window",
        }
    }
}

impl FromJson for BuildingEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("kind")? {
            "property" => Self::Property(BuildingPropertyEdit::read_fields(&mut object)?),
            "attribute" => Self::Attribute(BuildingAttributeEdit::read_fields(&mut object)?),
            "add_space" => Self::AddSpace(BuildingAddSpaceEdit::read_fields(&mut object)?),
            "add_element" => Self::AddElement(BuildingAddElementEdit::read_fields(&mut object)?),
            "remove" => Self::Remove(BuildingRemoveEdit::read_fields(&mut object)?),
            "place" => Self::Place(BuildingPlaceEdit::read_fields(&mut object)?),
            "contain" => Self::Contain(BuildingContainEdit::read_fields(&mut object)?),
            "type" => Self::Type(BuildingTypeEdit::read_fields(&mut object)?),
            "void" => Self::Void(BuildingVoidEdit::read_fields(&mut object)?),
            "fill" => Self::Fill(BuildingFillEdit::read_fields(&mut object)?),
            "add_wall" => Self::AddWall(BuildingAddWallEdit::read_fields(&mut object)?),
            "add_slab" => Self::AddSlab(BuildingAddSlabEdit::read_fields(&mut object)?),
            "add_column" => Self::AddColumn(BuildingAddColumnEdit::read_fields(&mut object)?),
            "add_beam" => Self::AddBeam(BuildingAddBeamEdit::read_fields(&mut object)?),
            "add_opening" => Self::AddOpening(BuildingAddOpeningEdit::read_fields(&mut object)?),
            "add_door" => Self::AddDoor(BuildingAddDoorEdit::read_fields(&mut object)?),
            "add_window" => Self::AddWindow(BuildingAddWindowEdit::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("kind")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingEdit {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Property(value) => value.to_json(out),
            Self::Attribute(value) => value.to_json(out),
            Self::AddSpace(value) => value.to_json(out),
            Self::AddElement(value) => value.to_json(out),
            Self::Remove(value) => value.to_json(out),
            Self::Place(value) => value.to_json(out),
            Self::Contain(value) => value.to_json(out),
            Self::Type(value) => value.to_json(out),
            Self::Void(value) => value.to_json(out),
            Self::Fill(value) => value.to_json(out),
            Self::AddWall(value) => value.to_json(out),
            Self::AddSlab(value) => value.to_json(out),
            Self::AddColumn(value) => value.to_json(out),
            Self::AddBeam(value) => value.to_json(out),
            Self::AddOpening(value) => value.to_json(out),
            Self::AddDoor(value) => value.to_json(out),
            Self::AddWindow(value) => value.to_json(out),
        }
    }
}

/// 要素のプロパティのセットの、プロパティ（IfcPropertySingleValue）の値を変える。セットかプロパティがなければ足す。ほかの要素と共有するセットは、写しを作って、この要素だけに付け替える。型の要素では、型のセット（HasPropertySets）を変える
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingPropertyEdit {
    /// 変える要素の ID。building_elements が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。building_elements が返したもの
    pub hash: String,
    /// プロパティのセットの名前（Pset_WallCommon など）
    pub set: String,
    /// プロパティの名前
    pub name: String,
    /// 値。type_name は要る。kind は real・integer・text・boolean・logical のどれか
    pub value: BuildingValue,
}

impl BuildingPropertyEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "property")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let set = object.required("set", json::string)?;
        let name = object.required("name", json::string)?;
        let value = object.required("value", BuildingValue::from_json)?;
        Ok(Self {
            id,
            hash,
            set,
            name,
            value,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("property");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        out.key("set");
        out.string(&self.set);
        out.key("name");
        out.string(&self.name);
        out.key("value");
        self.value.to_json(out);
    }
}

impl FromJson for BuildingPropertyEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingPropertyEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素（IfcRoot）の名前か説明を変える
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAttributeEdit {
    /// 変える要素の ID。building_elements が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。building_elements が返したもの
    pub hash: String,
    /// 変える属性
    pub attribute: BuildingAttribute,
    /// 値。書かなければ、値をなくす
    pub value: Option<String>,
}

impl BuildingAttributeEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "attribute")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        let attribute = object.required("attribute", BuildingAttribute::from_json)?;
        let value = object.optional("value", json::string)?;
        Ok(Self {
            id,
            hash,
            attribute,
            value,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("attribute");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
        out.key("attribute");
        self.attribute.to_json(out);
        if let Some(value) = &self.value {
            out.key("value");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAttributeEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAttributeEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 変えられる IfcRoot の属性。name は Name、description は Description
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildingAttribute {
    /// `name`
    Name,
    /// `description`
    Description,
}

impl BuildingAttribute {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Name, Self::Description];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Description => "description",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "name" => Some(Self::Name),
            "description" => Some(Self::Description),
            _ => None,
        }
    }
}

impl FromJson for BuildingAttribute {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for BuildingAttribute {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 変更が指す要素。id と hash を書くか、added だけを書く
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BuildingRef {
    /// 要素の ID。building_elements が返したもの
    pub id: Option<String>,
    /// 読んだときの要素のハッシュ。building_elements が返したもの
    pub hash: Option<String>,
    /// 同じ要求の edits の、前にある要素を足す変更（add_ で始まる種類）の位置（0 から）。その変更で足した要素を指す
    pub added: Option<u64>,
}

impl BuildingRef {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.optional("id", json::string)?;
        let hash = object.optional("hash", json::string)?;
        let added = object.optional("added", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { id, hash, added })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
        if let Some(value) = &self.added {
            out.key("added");
            out.uint(*value);
        }
    }
}

impl FromJson for BuildingRef {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingRef {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素の配置（IfcLocalPlacement）。relative_to の要素の配置からの相対で、書かなければプロジェクトの空間に置く
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingPlacementChange {
    /// 配置の親の要素
    pub relative_to: Option<BuildingRef>,
    /// 局所の変換。3 行 4 列（回転と移動。building_elements の placement の world と同じ形）。回転は正規直交で行列式が 1、移動は文書の長さの単位
    pub matrix: Vec<Vec<f64>>,
}

impl BuildingPlacementChange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let relative_to = object.optional("relative_to", BuildingRef::from_json)?;
        let matrix = object.required("matrix", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        Ok(Self {
            relative_to,
            matrix,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.relative_to {
            out.key("relative_to");
            value.to_json(out);
        }
        out.key("matrix");
        out.array(&self.matrix, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
    }
}

impl FromJson for BuildingPlacementChange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingPlacementChange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 足す空間の種類。site は IfcSite、building は IfcBuilding、storey は IfcBuildingStorey
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildingSpaceKind {
    /// `site`
    Site,
    /// `building`
    Building,
    /// `storey`
    Storey,
}

impl BuildingSpaceKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Site, Self::Building, Self::Storey];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::Building => "building",
            Self::Storey => "storey",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "site" => Some(Self::Site),
            "building" => Some(Self::Building),
            "storey" => Some(Self::Storey),
            _ => None,
        }
    }
}

impl FromJson for BuildingSpaceKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for BuildingSpaceKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 敷地・建物・階を足し、IfcRelAggregates で親に結ぶ。結果の id は足した空間の ID。マイナーバージョン 96 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAddSpaceEdit {
    /// 空間の種類
    pub space: BuildingSpaceKind,
    /// 親。敷地はプロジェクトか敷地、建物はプロジェクト・敷地・建物、階は建物か階。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub parent: BuildingRef,
    /// 名前
    pub name: Option<String>,
    /// 配置。書かなければ、親の配置に重ねる
    pub placement: Option<BuildingPlacementChange>,
    /// 高さ。敷地は RefElevation、建物は ElevationOfRefHeight、階は Elevation に書く。文書の長さの単位
    pub elevation: Option<f64>,
    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    pub global_id: Option<String>,
}

impl BuildingAddSpaceEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_space")?;
        let space = object.required("space", BuildingSpaceKind::from_json)?;
        let parent = object.required("parent", BuildingRef::from_json)?;
        let name = object.optional("name", json::string)?;
        let placement = object.optional("placement", BuildingPlacementChange::from_json)?;
        let elevation = object.optional("elevation", json::number)?;
        let global_id = object.optional("global_id", json::string)?;
        Ok(Self {
            space,
            parent,
            name,
            placement,
            elevation,
            global_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_space");
        out.key("space");
        self.space.to_json(out);
        out.key("parent");
        self.parent.to_json(out);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.placement {
            out.key("placement");
            value.to_json(out);
        }
        if let Some(value) = &self.elevation {
            out.key("elevation");
            out.float(*value);
        }
        if let Some(value) = &self.global_id {
            out.key("global_id");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAddSpaceEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAddSpaceEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素を足し、container があれば IfcRelContainedInSpatialStructure で空間に結ぶ。足せるのは、壁・床・柱・梁・部材・板・被覆・手すり・屋根・階段・斜路・基礎・杭・扉・窓・開口・代理要素（IfcBuildingElementProxy）。結果の id は足した要素の ID。マイナーバージョン 96 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAddElementEdit {
    /// IFC の実体の名前（IfcWall など）。文書のバージョンにあるもの
    pub class: String,
    /// PredefinedType の列挙の値（SOLIDWALL など）。バージョンが持たない実体では書けない
    pub predefined: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// 包含する空間（敷地・建物・階・室）。開口には書けない。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub container: Option<BuildingRef>,
    /// 配置。書かなければ、空間の配置に重ねる（空間もなければ原点）
    pub placement: Option<BuildingPlacementChange>,
    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    pub global_id: Option<String>,
}

impl BuildingAddElementEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_element")?;
        let class = object.required("class", json::string)?;
        let predefined = object.optional("predefined", json::string)?;
        let name = object.optional("name", json::string)?;
        let container = object.optional("container", BuildingRef::from_json)?;
        let placement = object.optional("placement", BuildingPlacementChange::from_json)?;
        let global_id = object.optional("global_id", json::string)?;
        Ok(Self {
            class,
            predefined,
            name,
            container,
            placement,
            global_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_element");
        out.key("class");
        out.string(&self.class);
        if let Some(value) = &self.predefined {
            out.key("predefined");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.container {
            out.key("container");
            value.to_json(out);
        }
        if let Some(value) = &self.placement {
            out.key("placement");
            value.to_json(out);
        }
        if let Some(value) = &self.global_id {
            out.key("global_id");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAddElementEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAddElementEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素を除く。関係からも外し、空になった関係を除く。壁の開口は一緒に除く。子のある空間と、関係のほかから指される要素は除けない。結果は id を持たない。マイナーバージョン 96 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingRemoveEdit {
    /// 除く要素の ID。building_elements が返したもの
    pub id: String,
    /// 読んだときの要素のハッシュ。building_elements が返したもの
    pub hash: String,
}

impl BuildingRemoveEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "remove")?;
        let id = object.required("id", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("remove");
        out.key("id");
        out.string(&self.id);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for BuildingRemoveEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingRemoveEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素の配置を置き直す。ほかの要素と共有しない配置はその場で書き換え、共有する配置は新しく作る。配置の親をたどって輪になる配置は断る。マイナーバージョン 96 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingPlaceEdit {
    /// 置き直す要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub element: BuildingRef,
    /// 新しい配置
    pub placement: BuildingPlacementChange,
}

impl BuildingPlaceEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "place")?;
        let element = object.required("element", BuildingRef::from_json)?;
        let placement = object.required("placement", BuildingPlacementChange::from_json)?;
        Ok(Self { element, placement })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("place");
        out.key("element");
        self.element.to_json(out);
        out.key("placement");
        self.placement.to_json(out);
    }
}

impl FromJson for BuildingPlaceEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingPlaceEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素を空間に移す。空間は IfcRelAggregates、要素は IfcRelContainedInSpatialStructure で結び、前の関係から外す。マイナーバージョン 96 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingContainEdit {
    /// 移す要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub element: BuildingRef,
    /// 移す先の空間。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub space: BuildingRef,
}

impl BuildingContainEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "contain")?;
        let element = object.required("element", BuildingRef::from_json)?;
        let space = object.required("space", BuildingRef::from_json)?;
        Ok(Self { element, space })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("contain");
        out.key("element");
        self.element.to_json(out);
        out.key("space");
        self.space.to_json(out);
    }
}

impl FromJson for BuildingContainEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingContainEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 要素の型を IfcRelDefinesByType で付け替える。element_type を書かなければ、型を外す。マイナーバージョン 96 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingTypeEdit {
    /// 型を変える要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub element: BuildingRef,
    /// 型の要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub element_type: Option<BuildingRef>,
}

impl BuildingTypeEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "type")?;
        let element = object.required("element", BuildingRef::from_json)?;
        let element_type = object.optional("element_type", BuildingRef::from_json)?;
        Ok(Self {
            element,
            element_type,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("type");
        out.key("element");
        self.element.to_json(out);
        if let Some(value) = &self.element_type {
            out.key("element_type");
            value.to_json(out);
        }
    }
}

impl FromJson for BuildingTypeEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingTypeEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開口（IfcOpeningElement）を要素に IfcRelVoidsElement で結ぶ。1 つの開口は 1 つの要素だけに結ぶ。マイナーバージョン 96 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingVoidEdit {
    /// 開口を持つ要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub host: BuildingRef,
    /// 開口。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub opening: BuildingRef,
}

impl BuildingVoidEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "void")?;
        let host = object.required("host", BuildingRef::from_json)?;
        let opening = object.required("opening", BuildingRef::from_json)?;
        Ok(Self { host, opening })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("void");
        out.key("host");
        self.host.to_json(out);
        out.key("opening");
        self.opening.to_json(out);
    }
}

impl FromJson for BuildingVoidEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingVoidEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 材料の層
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingWallLayer {
    /// 材料の名前（IfcMaterial の Name）。同じ要素の中で同じ名前の層は、同じ材料を指す
    pub material: String,
    /// 層の厚さ。文書の長さの単位で、正の値
    pub thickness: f64,
}

impl BuildingWallLayer {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let material = object.required("material", json::string)?;
        let thickness = object.required("thickness", json::number)?;
        Ok(Self {
            material,
            thickness,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("material");
        out.string(&self.material);
        out.key("thickness");
        out.float(self.thickness);
    }
}

impl FromJson for BuildingWallLayer {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingWallLayer {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 芯の 2 点・高さ・材料の層で壁を足し、空間に IfcRelContainedInSpatialStructure で結ぶ。配置（芯の始点を原点、芯の向きを x 軸）・Axis と Body の表現・IfcMaterialLayerSetUsage はコアが書く。IFC2X3 は IfcWallStandardCase、IFC4 と IFC4X3 は IfcWall。結果の id は足した壁の ID。マイナーバージョン 97 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAddWallEdit {
    /// 所属する空間（階など）。壁はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub storey: BuildingRef,
    /// 芯の始点。空間の配置の [x, y] で、文書の長さの単位
    pub start: Vec<f64>,
    /// 芯の終点。始点と違う [x, y]
    pub end: Vec<f64>,
    /// 高さ。文書の長さの単位で、正の値
    pub height: f64,
    /// 材料の層。始点から終点を見て、芯の右から左へ並べる。1 つ以上
    pub layers: Vec<BuildingWallLayer>,
    /// 芯から最初の層の面までのずれ（OffsetFromReferenceLine、左が正）。書かなければ厚さの合計の半分を負にして、芯を厚さの中央にする
    pub offset: Option<f64>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    pub global_id: Option<String>,
}

impl BuildingAddWallEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_wall")?;
        let storey = object.required("storey", BuildingRef::from_json)?;
        let start = object.required("start", |value| json::array(value, json::number))?;
        let end = object.required("end", |value| json::array(value, json::number))?;
        let height = object.required("height", json::number)?;
        let layers = object.required("layers", |value| {
            json::array(value, BuildingWallLayer::from_json)
        })?;
        let offset = object.optional("offset", json::number)?;
        let name = object.optional("name", json::string)?;
        let global_id = object.optional("global_id", json::string)?;
        Ok(Self {
            storey,
            start,
            end,
            height,
            layers,
            offset,
            name,
            global_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_wall");
        out.key("storey");
        self.storey.to_json(out);
        out.key("start");
        out.array(&self.start, |out, item| out.float(*item));
        out.key("end");
        out.array(&self.end, |out, item| out.float(*item));
        out.key("height");
        out.float(self.height);
        out.key("layers");
        out.array(&self.layers, |out, item| item.to_json(out));
        if let Some(value) = &self.offset {
            out.key("offset");
            out.float(*value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.global_id {
            out.key("global_id");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAddWallEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAddWallEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 矩形の断面
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingSection {
    /// 幅。文書の長さの単位で、正の値
    pub width: f64,
    /// せい。文書の長さの単位で、正の値
    pub depth: f64,
}

impl BuildingSection {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let width = object.required("width", json::number)?;
        let depth = object.required("depth", json::number)?;
        Ok(Self { width, depth })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("width");
        out.float(self.width);
        out.key("depth");
        out.float(self.depth);
    }
}

impl FromJson for BuildingSection {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingSection {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 外形・材料の層で床を足し、空間に包含で結ぶ。上の面を配置の原点にし、層の厚さの合計だけ下へ押し出す Body と、IfcSlab の FLOOR、IfcMaterialLayerSetUsage（AXIS3・NEGATIVE）をコアが書く。結果の id は足した床の ID。マイナーバージョン 98 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAddSlabEdit {
    /// 所属する空間（階など）。床はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub storey: BuildingRef,
    /// 外形の頂点。空間の配置の [x, y] の並びで、3 つ以上。最後の点が最初の点と同じなら、その点は除く
    pub outline: Vec<Vec<f64>>,
    /// 材料の層。上から下へ並べる。1 つ以上
    pub layers: Vec<BuildingWallLayer>,
    /// 上の面の高さ（空間の配置の z）。書かなければ 0
    pub elevation: Option<f64>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    pub global_id: Option<String>,
}

impl BuildingAddSlabEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_slab")?;
        let storey = object.required("storey", BuildingRef::from_json)?;
        let outline = object.required("outline", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let layers = object.required("layers", |value| {
            json::array(value, BuildingWallLayer::from_json)
        })?;
        let elevation = object.optional("elevation", json::number)?;
        let name = object.optional("name", json::string)?;
        let global_id = object.optional("global_id", json::string)?;
        Ok(Self {
            storey,
            outline,
            layers,
            elevation,
            name,
            global_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_slab");
        out.key("storey");
        self.storey.to_json(out);
        out.key("outline");
        out.array(&self.outline, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
        out.key("layers");
        out.array(&self.layers, |out, item| item.to_json(out));
        if let Some(value) = &self.elevation {
            out.key("elevation");
            out.float(*value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.global_id {
            out.key("global_id");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAddSlabEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAddSlabEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 位置・高さ・矩形の断面で柱を足し、空間に包含で結ぶ。断面の中心を配置の原点にして上へ押し出す Body と IfcColumn をコアが書く。結果の id は足した柱の ID。マイナーバージョン 98 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAddColumnEdit {
    /// 所属する空間（階など）。柱はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub storey: BuildingRef,
    /// 断面の中心。空間の配置の [x, y]
    pub position: Vec<f64>,
    /// 柱の脚の高さ（空間の配置の z）。書かなければ 0
    pub base: Option<f64>,
    /// 高さ。文書の長さの単位で、正の値
    pub height: f64,
    /// 断面。幅は柱の x 軸、せいは y 軸の向き
    pub section: BuildingSection,
    /// 柱の x 軸の向き。空間の配置の [x, y]。書かなければ空間の x 軸
    pub direction: Option<Vec<f64>>,
    /// 材料の名前（IfcMaterial の Name）。書けば IfcRelAssociatesMaterial で結ぶ
    pub material: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    pub global_id: Option<String>,
}

impl BuildingAddColumnEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_column")?;
        let storey = object.required("storey", BuildingRef::from_json)?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let base = object.optional("base", json::number)?;
        let height = object.required("height", json::number)?;
        let section = object.required("section", BuildingSection::from_json)?;
        let direction = object.optional("direction", |value| json::array(value, json::number))?;
        let material = object.optional("material", json::string)?;
        let name = object.optional("name", json::string)?;
        let global_id = object.optional("global_id", json::string)?;
        Ok(Self {
            storey,
            position,
            base,
            height,
            section,
            direction,
            material,
            name,
            global_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_column");
        out.key("storey");
        self.storey.to_json(out);
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        if let Some(value) = &self.base {
            out.key("base");
            out.float(*value);
        }
        out.key("height");
        out.float(self.height);
        out.key("section");
        self.section.to_json(out);
        if let Some(value) = &self.direction {
            out.key("direction");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.material {
            out.key("material");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.global_id {
            out.key("global_id");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAddColumnEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAddColumnEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 天端の芯の 2 点・矩形の断面で梁を足し、空間に包含で結ぶ。始点を配置の原点、芯の向きを x 軸、水平で芯の左を y 軸にし、断面を芯に沿って押し出す Body と IfcBeam をコアが書く。鉛直の芯は断る。結果の id は足した梁の ID。マイナーバージョン 98 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAddBeamEdit {
    /// 所属する空間（階など）。梁はこの空間の配置からの相対に置く。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub storey: BuildingRef,
    /// 天端の芯の始点。空間の配置の [x, y, z]
    pub start: Vec<f64>,
    /// 天端の芯の終点。始点と x か y が違う [x, y, z]
    pub end: Vec<f64>,
    /// 断面。幅は芯の左右、せいは天端から下へ
    pub section: BuildingSection,
    /// 材料の名前（IfcMaterial の Name）。書けば IfcRelAssociatesMaterial で結ぶ
    pub material: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    pub global_id: Option<String>,
}

impl BuildingAddBeamEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_beam")?;
        let storey = object.required("storey", BuildingRef::from_json)?;
        let start = object.required("start", |value| json::array(value, json::number))?;
        let end = object.required("end", |value| json::array(value, json::number))?;
        let section = object.required("section", BuildingSection::from_json)?;
        let material = object.optional("material", json::string)?;
        let name = object.optional("name", json::string)?;
        let global_id = object.optional("global_id", json::string)?;
        Ok(Self {
            storey,
            start,
            end,
            section,
            material,
            name,
            global_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_beam");
        out.key("storey");
        self.storey.to_json(out);
        out.key("start");
        out.array(&self.start, |out, item| out.float(*item));
        out.key("end");
        out.array(&self.end, |out, item| out.float(*item));
        out.key("section");
        self.section.to_json(out);
        if let Some(value) = &self.material {
            out.key("material");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.global_id {
            out.key("global_id");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAddBeamEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAddBeamEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 壁の芯に沿う位置・下端・矩形の大きさで開口を足し、IfcRelVoidsElement で壁に穴をあける。開口は壁の配置からの相対に置き、壁の材料の層の使い方（AXIS2）の厚さだけ押し出す Body と IfcOpeningElement をコアが書く。空間には所属させない。層の使い方を持たない要素は断る。結果の id は足した開口の ID。マイナーバージョン 98 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAddOpeningEdit {
    /// 穴をあける壁。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub host: BuildingRef,
    /// 壁の配置の x 軸に沿う、壁の原点から開口の端までの距離
    pub distance: f64,
    /// 壁の下端から開口の下端までの高さ
    pub sill: f64,
    /// 幅。文書の長さの単位で、正の値
    pub width: f64,
    /// 高さ。文書の長さの単位で、正の値
    pub height: f64,
    /// 名前
    pub name: Option<String>,
    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    pub global_id: Option<String>,
}

impl BuildingAddOpeningEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_opening")?;
        let host = object.required("host", BuildingRef::from_json)?;
        let distance = object.required("distance", json::number)?;
        let sill = object.required("sill", json::number)?;
        let width = object.required("width", json::number)?;
        let height = object.required("height", json::number)?;
        let name = object.optional("name", json::string)?;
        let global_id = object.optional("global_id", json::string)?;
        Ok(Self {
            host,
            distance,
            sill,
            width,
            height,
            name,
            global_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_opening");
        out.key("host");
        self.host.to_json(out);
        out.key("distance");
        out.float(self.distance);
        out.key("sill");
        out.float(self.sill);
        out.key("width");
        out.float(self.width);
        out.key("height");
        out.float(self.height);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.global_id {
            out.key("global_id");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAddOpeningEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAddOpeningEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開口を扉で埋める。扉は開口の配置に置き、IfcDoor（IFC4 と IFC4X3 は PredefinedType の DOOR）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。形と型はまだ書かない。もう埋まった開口は断る。結果の id は足した扉の ID。マイナーバージョン 98 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAddDoorEdit {
    /// 埋める開口（IfcOpeningElement）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub opening: BuildingRef,
    /// 全体の幅（OverallWidth）。文書の長さの単位で、正の値
    pub width: Option<f64>,
    /// 全体の高さ（OverallHeight）。文書の長さの単位で、正の値
    pub height: Option<f64>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    pub global_id: Option<String>,
}

impl BuildingAddDoorEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_door")?;
        let opening = object.required("opening", BuildingRef::from_json)?;
        let width = object.optional("width", json::number)?;
        let height = object.optional("height", json::number)?;
        let name = object.optional("name", json::string)?;
        let global_id = object.optional("global_id", json::string)?;
        Ok(Self {
            opening,
            width,
            height,
            name,
            global_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_door");
        out.key("opening");
        self.opening.to_json(out);
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.global_id {
            out.key("global_id");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAddDoorEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAddDoorEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開口を窓で埋める。窓は開口の配置に置き、IfcWindow（IFC4 と IFC4X3 は PredefinedType の WINDOW）を IfcRelFillsElement で開口に、包含で壁と同じ空間に結ぶ。形と型はまだ書かない。もう埋まった開口は断る。結果の id は足した窓の ID。マイナーバージョン 98 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingAddWindowEdit {
    /// 埋める開口（IfcOpeningElement）。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub opening: BuildingRef,
    /// 全体の幅（OverallWidth）。文書の長さの単位で、正の値
    pub width: Option<f64>,
    /// 全体の高さ（OverallHeight）。文書の長さの単位で、正の値
    pub height: Option<f64>,
    /// 名前
    pub name: Option<String>,
    /// GlobalId（22 文字）。書かなければ、文書のハッシュと変更の位置から決める
    pub global_id: Option<String>,
}

impl BuildingAddWindowEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "add_window")?;
        let opening = object.required("opening", BuildingRef::from_json)?;
        let width = object.optional("width", json::number)?;
        let height = object.optional("height", json::number)?;
        let name = object.optional("name", json::string)?;
        let global_id = object.optional("global_id", json::string)?;
        Ok(Self {
            opening,
            width,
            height,
            name,
            global_id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("add_window");
        out.key("opening");
        self.opening.to_json(out);
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.global_id {
            out.key("global_id");
            out.string(value);
        }
    }
}

impl FromJson for BuildingAddWindowEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingAddWindowEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 扉や窓を開口（IfcOpeningElement）に IfcRelFillsElement で結ぶ。1 つの開口は 1 つの要素だけで埋める。マイナーバージョン 96 から
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingFillEdit {
    /// 開口。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub opening: BuildingRef,
    /// 埋める要素。building_elements が返した ID とハッシュか、同じ要求の前の、要素を足す変更（add_ で始まる種類）の位置で指す
    pub filler: BuildingRef,
}

impl BuildingFillEdit {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "fill")?;
        let opening = object.required("opening", BuildingRef::from_json)?;
        let filler = object.required("filler", BuildingRef::from_json)?;
        Ok(Self { opening, filler })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("fill");
        out.key("opening");
        self.opening.to_json(out);
        out.key("filler");
        self.filler.to_json(out);
    }
}

impl FromJson for BuildingFillEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingFillEdit {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 当てた変更の結果。書き戻したファイルは blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct BuildingApplyResult {
    /// 変更ごとの結果。要求の edits と同じ順
    pub applied: Vec<Applied>,
    /// 書き戻した文書のハッシュ。次の building_apply の前提条件に使う
    pub hash: String,
}

impl BuildingApplyResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let applied = object.required("applied", |value| json::array(value, Applied::from_json))?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { applied, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("applied");
        out.array(&self.applied, |out, item| item.to_json(out));
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for BuildingApplyResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BuildingApplyResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた図面の文書を図面モデル（L2）に変換し、図面モデルのバイナリ（決定 0247）を blob で返す。配置は schema/drawing.toml にあり、元の文書の形式・系統・ハッシュも持つ。プラグインのモジュールが読んだ図面は、これを図面のモジュールの open に渡すと、元の形式とハッシュの読むだけの文書になる。マイナーバージョン 60 から
#[derive(Clone, Debug, PartialEq)]
pub struct LiftRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で図面を開いたもの
    pub doc: String,
}

impl LiftRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "lift")?;
        let doc = object.required("doc", json::string)?;
        Ok(Self {
            request_common,
            doc,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("lift");
        out.key("doc");
        out.string(&self.doc);
    }
}

impl FromJson for LiftRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LiftRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 図面モデルのバイナリの blob の番号
#[derive(Clone, Debug, PartialEq)]
pub struct LiftResult {
    /// 図面モデルのバイナリの blob の番号（種類は drawing）。zm_take で取り出す
    pub blob: u64,
}

impl LiftResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let blob = object.required("blob", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { blob })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("blob");
        out.uint(self.blob);
    }
}

impl FromJson for LiftResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LiftResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。マイナーバージョン 44 から
#[derive(Clone, Debug, PartialEq)]
pub struct IdsCheckRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で IFC を開いたもの
    pub doc: String,
    /// IDS の入力の名前（zm_push で渡した .ids）
    pub ids: String,
    /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルのコンポーネントにある属性だけを照らす
    pub express: Option<String>,
    /// 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す数の上限。書かなければ 1000
    pub limit: Option<u64>,
}

impl IdsCheckRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "ids_check")?;
        let doc = object.required("doc", json::string)?;
        let ids = object.required("ids", json::string)?;
        let express = object.optional("express", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 1, 10000))?;
        Ok(Self {
            request_common,
            doc,
            ids,
            express,
            offset,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("ids_check");
        out.key("doc");
        out.string(&self.doc);
        out.key("ids");
        out.string(&self.ids);
        if let Some(value) = &self.express {
            out.key("express");
            out.string(value);
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for IdsCheckRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdsCheckRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 照合の結果。指摘は offset から limit 個まで
#[derive(Clone, Debug, PartialEq)]
pub struct IdsCheckResult {
    /// IDS の題
    pub title: Option<String>,
    /// 仕様ごとの結果。IDS の順
    pub specifications: Vec<IdsSpecification>,
    /// 指摘。仕様の順
    pub findings: Vec<IdsFinding>,
    /// 全体の指摘の数
    pub total: u64,
    /// 続きの最初の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
}

impl IdsCheckResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let title = object.optional("title", json::string)?;
        let specifications = object.required("specifications", |value| {
            json::array(value, IdsSpecification::from_json)
        })?;
        let findings = object.required("findings", |value| {
            json::array(value, IdsFinding::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            title,
            specifications,
            findings,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.title {
            out.key("title");
            out.string(value);
        }
        out.key("specifications");
        out.array(&self.specifications, |out, item| item.to_json(out));
        out.key("findings");
        out.array(&self.findings, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for IdsCheckResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdsCheckResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 仕様の結果
#[derive(Clone, Debug, PartialEq)]
pub struct IdsSpecification {
    /// 名前
    pub name: String,
    /// 識別子
    pub identifier: Option<String>,
    /// 結果
    pub status: IdsStatus,
    /// 適用の条件に合う要素の数
    pub applicable: u64,
    /// 要件を満たさない要素の数
    pub failed: u64,
    /// 確かめられなかった要素の数
    pub unsupported: u64,
}

impl IdsSpecification {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let identifier = object.optional("identifier", json::string)?;
        let status = object.required("status", IdsStatus::from_json)?;
        let applicable =
            object.required("applicable", |value| json::uint(value, 0, 9007199254740991))?;
        let failed = object.required("failed", |value| json::uint(value, 0, 9007199254740991))?;
        let unsupported = object.required("unsupported", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            name,
            identifier,
            status,
            applicable,
            failed,
            unsupported,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.identifier {
            out.key("identifier");
            out.string(value);
        }
        out.key("status");
        self.status.to_json(out);
        out.key("applicable");
        out.uint(self.applicable);
        out.key("failed");
        out.uint(self.failed);
        out.key("unsupported");
        out.uint(self.unsupported);
    }
}

impl FromJson for IdsSpecification {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdsSpecification {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 指摘。diagnostic は診断と同じ形で、describe で文にできる
#[derive(Clone, Debug, PartialEq)]
pub struct IdsFinding {
    /// 仕様の名前
    pub specification: String,
    /// code・重要度・場所（要素の ID）・値
    pub diagnostic: Diagnostic,
}

impl IdsFinding {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let specification = object.required("specification", json::string)?;
        let diagnostic = object.required("diagnostic", Diagnostic::from_json)?;
        Ok(Self {
            specification,
            diagnostic,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("specification");
        out.string(&self.specification);
        out.key("diagnostic");
        self.diagnostic.to_json(out);
    }
}

impl FromJson for IdsFinding {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for IdsFinding {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。マイナーバージョン 44 から
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAdoptIdsRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 文書の名前。open で IFC を開いたもの
    pub doc: String,
    /// IDS の入力の名前（zm_push で渡した .ids）
    pub ids: String,
    /// EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルのコンポーネントにある属性だけを照らす
    pub express: Option<String>,
    /// 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す
    pub review: Option<String>,
    /// 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す
    pub target: String,
    /// 取り込む指摘の番号の列。ids_check の指摘の並びの順で、0 から数える
    pub findings: Vec<u64>,
}

impl ReviewAdoptIdsRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "review_adopt_ids")?;
        let doc = object.required("doc", json::string)?;
        let ids = object.required("ids", json::string)?;
        let express = object.optional("express", json::string)?;
        let review = object.optional("review", json::string)?;
        let target = object.required("target", json::string)?;
        let findings = object.required("findings", |value| {
            json::array(value, |value| json::uint(value, 0, 9007199254740991))
        })?;
        Ok(Self {
            request_common,
            doc,
            ids,
            express,
            review,
            target,
            findings,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("review_adopt_ids");
        out.key("doc");
        out.string(&self.doc);
        out.key("ids");
        out.string(&self.ids);
        if let Some(value) = &self.express {
            out.key("express");
            out.string(value);
        }
        if let Some(value) = &self.review {
            out.key("review");
            out.string(value);
        }
        out.key("target");
        out.string(&self.target);
        out.key("findings");
        out.array(&self.findings, |out, item| out.uint(*item));
    }
}

impl FromJson for ReviewAdoptIdsRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ReviewAdoptIdsRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// zm_push で渡した BCF 2.1 か 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。マイナーバージョン 45 から
#[derive(Clone, Debug, PartialEq)]
pub struct BcfReadRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// BCF が指す建物モデルの文書の名前。open で IFC を開いたもの
    pub doc: String,
    /// bcfzip の入力の名前（zm_push で渡した .bcf）
    pub input: String,
    /// レビューの文書の対象の ID
    pub target: String,
    /// 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
    pub statuses: Option<Vec<BcfStatus>>,
}

impl BcfReadRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "bcf_read")?;
        let doc = object.required("doc", json::string)?;
        let input = object.required("input", json::string)?;
        let target = object.required("target", json::string)?;
        let statuses =
            object.optional("statuses", |value| json::array(value, BcfStatus::from_json))?;
        Ok(Self {
            request_common,
            doc,
            input,
            target,
            statuses,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("bcf_read");
        out.key("doc");
        out.string(&self.doc);
        out.key("input");
        out.string(&self.input);
        out.key("target");
        out.string(&self.target);
        if let Some(value) = &self.statuses {
            out.key("statuses");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for BcfReadRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfReadRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 状態の対応
#[derive(Clone, Debug, PartialEq)]
pub struct BcfStatus {
    /// BCF の TopicStatus の文字列
    pub text: String,
    /// 指摘の状態
    pub status: ReviewStatus,
}

impl BcfStatus {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let text = object.required("text", json::string)?;
        let status = object.required("status", ReviewStatus::from_json)?;
        Ok(Self { text, status })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("text");
        out.string(&self.text);
        out.key("status");
        self.status.to_json(out);
    }
}

impl FromJson for BcfStatus {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfStatus {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 指摘の状態（決定 0214）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReviewStatus {
    /// `open`
    Open,
    /// `resolved`
    Resolved,
    /// `closed`
    Closed,
}

impl ReviewStatus {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Open, Self::Resolved, Self::Closed];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Closed => "closed",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "open" => Some(Self::Open),
            "resolved" => Some(Self::Resolved),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

impl FromJson for ReviewStatus {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for ReviewStatus {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 読んだ bcfzip
#[derive(Clone, Debug, PartialEq)]
pub struct BcfReadResult {
    /// BCF のバージョン
    pub version: String,
    /// 指摘の数
    pub topics: u64,
}

impl BcfReadResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let version = object.required("version", json::string)?;
        let topics = object.required("topics", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { version, topics })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("version");
        out.string(&self.version);
        out.key("topics");
        out.uint(self.topics);
    }
}

impl FromJson for BcfReadResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfReadResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// zm_push で渡したレビューの文書を、BCF の bcfzip に書く（決定 0214・0220）。バージョンは version で選ぶ（マイナーバージョン 61 から。それまでは 3.0 だけ）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。マイナーバージョン 45 から
#[derive(Clone, Debug, PartialEq)]
pub struct BcfWriteRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// レビューの文書の入力の名前（zm_push で渡した JSON）
    pub review: String,
    /// viewpoint にする 3D の場所の対象の ID
    pub target: String,
    /// 元の bcfzip の入力の名前（zm_push で渡した .bcf）。書かなければ新しく書く
    pub original: Option<String>,
    /// 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す
    pub statuses: Option<Vec<BcfStatus>>,
    /// 種類のない指摘に書く TopicType
    pub kind: Option<String>,
    /// 作者のない指摘とコメントに書く作者
    pub author: Option<String>,
    /// 日時のない指摘とコメントに書く日時（RFC 3339）
    pub date: Option<String>,
    /// 書く BCF のバージョン。2.1 か 3.0。書かなければ、original のバージョンか 3.0。original と違うバージョンでは書かず、bcf.conversion.unsupported で失敗する。ほかの値は call.value.invalid。マイナーバージョン 61 から
    pub version: Option<String>,
}

impl BcfWriteRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "bcf_write")?;
        let review = object.required("review", json::string)?;
        let target = object.required("target", json::string)?;
        let original = object.optional("original", json::string)?;
        let statuses =
            object.optional("statuses", |value| json::array(value, BcfStatus::from_json))?;
        let kind = object.optional("kind", json::string)?;
        let author = object.optional("author", json::string)?;
        let date = object.optional("date", json::string)?;
        let version = object.optional("version", json::string)?;
        Ok(Self {
            request_common,
            review,
            target,
            original,
            statuses,
            kind,
            author,
            date,
            version,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("bcf_write");
        out.key("review");
        out.string(&self.review);
        out.key("target");
        out.string(&self.target);
        if let Some(value) = &self.original {
            out.key("original");
            out.string(value);
        }
        if let Some(value) = &self.statuses {
            out.key("statuses");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.kind {
            out.key("kind");
            out.string(value);
        }
        if let Some(value) = &self.author {
            out.key("author");
            out.string(value);
        }
        if let Some(value) = &self.date {
            out.key("date");
            out.string(value);
        }
        if let Some(value) = &self.version {
            out.key("version");
            out.string(value);
        }
    }
}

impl FromJson for BcfWriteRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfWriteRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 書いた bcfzip
#[derive(Clone, Debug, PartialEq)]
pub struct BcfWriteResult {
    /// 書いた指摘の数
    pub topics: u64,
}

impl BcfWriteResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let topics = object.required("topics", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { topics })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("topics");
        out.uint(self.topics);
    }
}

impl FromJson for BcfWriteResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BcfWriteResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 表の項目
#[derive(Clone, Debug, PartialEq)]
pub struct TableItem {
    /// ID。表のレコードがハンドルを持てば layer/h10・line-type/h14、なければ読んだ順の番号（layer/2）。寸法スタイルは dimension-style/h1A か、寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    pub hash: String,
}

impl TableItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        Ok(Self { id, name, hash })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
    }
}

impl FromJson for TableItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TableItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// レイヤの表の項目（決定 0162）。マイナーバージョン 22 から、状態とスタイルを持つ
#[derive(Clone, Debug, PartialEq)]
pub struct LayerItem {
    /// ID。表のレコードがハンドルを持てば layer/h10、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    pub hash: String,
    /// ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかのフィールドを持たない。マイナーバージョン 22 から
    pub defined: bool,
    /// 表示する。マイナーバージョン 22 から
    pub visible: Option<bool>,
    /// 印刷する。マイナーバージョン 22 から
    pub printable: Option<bool>,
    /// 編集できる。マイナーバージョン 22 から
    pub editable: Option<bool>,
    /// 凍結する。マイナーバージョン 22 から
    pub frozen: Option<bool>,
    /// 色。inherit を持たない。定義が色を持たなければ書かない。マイナーバージョン 22 から
    pub color: Option<ColorChange>,
    /// 線種の ID。定義が線種を持たなければ書かない。マイナーバージョン 22 から
    pub line_type_id: Option<String>,
    /// 線幅。inherit を持たない。定義が線幅を持たなければ書かない。マイナーバージョン 22 から
    pub line_weight: Option<LineWeightChange>,
}

impl LayerItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        let defined = object.required("defined", json::boolean)?;
        let visible = object.optional("visible", json::boolean)?;
        let printable = object.optional("printable", json::boolean)?;
        let editable = object.optional("editable", json::boolean)?;
        let frozen = object.optional("frozen", json::boolean)?;
        let color = object.optional("color", ColorChange::from_json)?;
        let line_type_id = object.optional("line_type_id", json::string)?;
        let line_weight = object.optional("line_weight", LineWeightChange::from_json)?;
        Ok(Self {
            id,
            name,
            hash,
            defined,
            visible,
            printable,
            editable,
            frozen,
            color,
            line_type_id,
            line_weight,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
        out.key("defined");
        out.bool(self.defined);
        if let Some(value) = &self.visible {
            out.key("visible");
            out.bool(*value);
        }
        if let Some(value) = &self.printable {
            out.key("printable");
            out.bool(*value);
        }
        if let Some(value) = &self.editable {
            out.key("editable");
            out.bool(*value);
        }
        if let Some(value) = &self.frozen {
            out.key("frozen");
            out.bool(*value);
        }
        if let Some(value) = &self.color {
            out.key("color");
            value.to_json(out);
        }
        if let Some(value) = &self.line_type_id {
            out.key("line_type_id");
            out.string(value);
        }
        if let Some(value) = &self.line_weight {
            out.key("line_weight");
            value.to_json(out);
        }
    }
}

impl FromJson for LayerItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LayerItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種の表の項目（決定 0164）。マイナーバージョン 26 から、定義を持つ
#[derive(Clone, Debug, PartialEq)]
pub struct LineTypeItem {
    /// ID。表のレコードがハンドルを持てば line-type/h14、なければ読んだ順の番号（line-type/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    pub hash: String,
    /// ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかのフィールドを持たない。マイナーバージョン 26 から
    pub defined: bool,
    /// 元の説明。マイナーバージョン 26 から
    pub description: Option<String>,
    /// 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。マイナーバージョン 26 から
    pub pattern: Option<Vec<f64>>,
    /// 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。マイナーバージョン 26 から
    pub paper: Option<bool>,
    /// 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。マイナーバージョン 26 から
    pub marked: Option<bool>,
    /// 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。マイナーバージョン 28 から
    pub marks: Option<Vec<LineTypeMark>>,
}

impl LineTypeItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        let defined = object.required("defined", json::boolean)?;
        let description = object.optional("description", json::string)?;
        let pattern = object.optional("pattern", |value| json::array(value, json::number))?;
        let paper = object.optional("paper", json::boolean)?;
        let marked = object.optional("marked", json::boolean)?;
        let marks =
            object.optional("marks", |value| json::array(value, LineTypeMark::from_json))?;
        Ok(Self {
            id,
            name,
            hash,
            defined,
            description,
            pattern,
            paper,
            marked,
            marks,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
        out.key("defined");
        out.bool(self.defined);
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        if let Some(value) = &self.pattern {
            out.key("pattern");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.paper {
            out.key("paper");
            out.bool(*value);
        }
        if let Some(value) = &self.marked {
            out.key("marked");
            out.bool(*value);
        }
        if let Some(value) = &self.marks {
            out.key("marks");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for LineTypeItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineTypeItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 寸法スタイルの表の項目（決定 0165）。マイナーバージョン 27 から、端部記号と大きさを持つ
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionStyleItem {
    /// ID。寸法スタイルのレコードがハンドルを持てば dimension-style/h1A、なければ寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    pub hash: String,
    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。マイナーバージョン 27 から
    pub ends: Option<Vec<String>>,
    /// 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。マイナーバージョン 27 から
    pub leader: Option<String>,
    /// 端部記号の大きさ。文書の単位で、scale を掛ける前の値。マイナーバージョン 27 から
    pub size: Option<f64>,
    /// 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。マイナーバージョン 27 から
    pub tick: Option<f64>,
    /// 大きさに掛ける倍率。0 はビューの縮尺から決める。マイナーバージョン 27 から
    pub scale: Option<f64>,
}

impl DimensionStyleItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        let ends = object.optional("ends", |value| json::array(value, json::string))?;
        let leader = object.optional("leader", json::string)?;
        let size = object.optional("size", json::number)?;
        let tick = object.optional("tick", json::number)?;
        let scale = object.optional("scale", json::number)?;
        Ok(Self {
            id,
            name,
            hash,
            ends,
            leader,
            size,
            tick,
            scale,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
        if let Some(value) = &self.ends {
            out.key("ends");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.leader {
            out.key("leader");
            out.string(value);
        }
        if let Some(value) = &self.size {
            out.key("size");
            out.float(*value);
        }
        if let Some(value) = &self.tick {
            out.key("tick");
            out.float(*value);
        }
        if let Some(value) = &self.scale {
            out.key("scale");
            out.float(*value);
        }
    }
}

impl FromJson for DimensionStyleItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DimensionStyleItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。マイナーバージョン 28 から
#[derive(Clone, Debug, PartialEq)]
pub struct LineTypeMark {
    /// 添える列の要素の番号（pattern の添字）
    pub index: u64,
    /// 形（shape）か文字（text）
    pub kind: LineTypeMarkKind,
    /// 形の形のファイル。元の形式が指す文字スタイルがなければ書かない
    pub font: Option<Font>,
    /// 形の番号
    pub number: Option<u64>,
    /// 文字の文字列
    pub text: Option<String>,
    /// 文字の文字スタイルの元の名前。元の形式が指す文字スタイルがなければ書かない
    pub style: Option<String>,
    /// 文字の文字スタイルの ID。tables の text_styles の ID で、名前で引いたもの（大文字と小文字を区別せず、同じ名前があれば先の項目）
    pub style_id: Option<String>,
    /// 文字の高さ。文字スタイルが決まった高さを持てば、それに scale を掛けたもの。持たなければ scale。単位は pattern と同じ
    pub height: Option<f64>,
    /// 倍率。文字なら、文字スタイルの高さへの倍率
    pub scale: f64,
    /// 回転（度、反時計回り）
    pub rotation: f64,
    /// 回転の基準
    pub rotation_basis: MarkRotation,
    /// 置く位置のずれ（x・y）。線の向きを x とする。単位は pattern と同じ
    pub offset: Vec<f64>,
}

impl LineTypeMark {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let index = object.required("index", |value| json::uint(value, 0, 9007199254740991))?;
        let kind = object.required("kind", LineTypeMarkKind::from_json)?;
        let font = object.optional("font", Font::from_json)?;
        let number = object.optional("number", |value| json::uint(value, 0, 9007199254740991))?;
        let text = object.optional("text", json::string)?;
        let style = object.optional("style", json::string)?;
        let style_id = object.optional("style_id", json::string)?;
        let height = object.optional("height", json::number)?;
        let scale = object.required("scale", json::number)?;
        let rotation = object.required("rotation", json::number)?;
        let rotation_basis = object.required("rotation_basis", MarkRotation::from_json)?;
        let offset = object.required("offset", |value| json::array(value, json::number))?;
        Ok(Self {
            index,
            kind,
            font,
            number,
            text,
            style,
            style_id,
            height,
            scale,
            rotation,
            rotation_basis,
            offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("index");
        out.uint(self.index);
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.font {
            out.key("font");
            value.to_json(out);
        }
        if let Some(value) = &self.number {
            out.key("number");
            out.uint(*value);
        }
        if let Some(value) = &self.text {
            out.key("text");
            out.string(value);
        }
        if let Some(value) = &self.style {
            out.key("style");
            out.string(value);
        }
        if let Some(value) = &self.style_id {
            out.key("style_id");
            out.string(value);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        out.key("scale");
        out.float(self.scale);
        out.key("rotation");
        out.float(self.rotation);
        out.key("rotation_basis");
        self.rotation_basis.to_json(out);
        out.key("offset");
        out.array(&self.offset, |out, item| out.float(*item));
    }
}

impl FromJson for LineTypeMark {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for LineTypeMark {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線種に添えるものの種類。shape は形のファイルの形、text は文字
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineTypeMarkKind {
    /// `shape`
    Shape,
    /// `text`
    Text,
}

impl LineTypeMarkKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Shape, Self::Text];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Text => "text",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "shape" => Some(Self::Shape),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

impl FromJson for LineTypeMarkKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for LineTypeMarkKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 回転の基準。relative は線の向きから、absolute は文書の x 軸から測る。upright は線の向きから測り、文字が逆さに見えるなら半回転する
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MarkRotation {
    /// `relative`
    Relative,
    /// `absolute`
    Absolute,
    /// `upright`
    Upright,
}

impl MarkRotation {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Relative, Self::Absolute, Self::Upright];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Relative => "relative",
            Self::Absolute => "absolute",
            Self::Upright => "upright",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "relative" => Some(Self::Relative),
            "absolute" => Some(Self::Absolute),
            "upright" => Some(Self::Upright),
            _ => None,
        }
    }
}

impl FromJson for MarkRotation {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for MarkRotation {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。マイナーバージョン 20 から
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyleItem {
    /// ID。表のレコードがハンドルを持てば text-style/h11、なければ文字スタイルの並びの中の順（text-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156）
    pub id: String,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156）
    pub hash: String,
    /// ファイルに定義がある。偽なら、ほかのフィールドを持たない
    pub defined: bool,
    /// 主のフォント。ファイルが決めていなければ書かない
    pub font: Option<Font>,
    /// 大きいフォント（漢字などの形のファイル）。なければ書かない
    pub big_font: Option<Font>,
    /// 決まった高さ。文書の単位。決めなければ書かない
    pub height: Option<f64>,
    /// 幅の倍率
    pub width: Option<f64>,
    /// 傾き（度）
    pub oblique: Option<f64>,
    /// 左右を裏返す
    pub backward: Option<bool>,
    /// 上下を反転する
    pub upside_down: Option<bool>,
    /// 縦書き
    pub vertical: Option<bool>,
}

impl TextStyleItem {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.required("name", json::string)?;
        let hash = object.required("hash", json::string)?;
        let defined = object.required("defined", json::boolean)?;
        let font = object.optional("font", Font::from_json)?;
        let big_font = object.optional("big_font", Font::from_json)?;
        let height = object.optional("height", json::number)?;
        let width = object.optional("width", json::number)?;
        let oblique = object.optional("oblique", json::number)?;
        let backward = object.optional("backward", json::boolean)?;
        let upside_down = object.optional("upside_down", json::boolean)?;
        let vertical = object.optional("vertical", json::boolean)?;
        Ok(Self {
            id,
            name,
            hash,
            defined,
            font,
            big_font,
            height,
            width,
            oblique,
            backward,
            upside_down,
            vertical,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("name");
        out.string(&self.name);
        out.key("hash");
        out.string(&self.hash);
        out.key("defined");
        out.bool(self.defined);
        if let Some(value) = &self.font {
            out.key("font");
            value.to_json(out);
        }
        if let Some(value) = &self.big_font {
            out.key("big_font");
            value.to_json(out);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.width {
            out.key("width");
            out.float(*value);
        }
        if let Some(value) = &self.oblique {
            out.key("oblique");
            out.float(*value);
        }
        if let Some(value) = &self.backward {
            out.key("backward");
            out.bool(*value);
        }
        if let Some(value) = &self.upside_down {
            out.key("upside_down");
            out.bool(*value);
        }
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.bool(*value);
        }
    }
}

impl FromJson for TextStyleItem {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for TextStyleItem {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// フォントの指定。取得と字形の評価は使い手が行う
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    /// 元の名前。形のファイルならファイルの名前、名前で指すフォントなら書体の名前。翻訳しない
    pub name: String,
    /// 種類
    pub kind: FontKind,
    /// 太字。名前で指すフォントだけ
    pub bold: Option<bool>,
    /// 斜体。名前で指すフォントだけ
    pub italic: Option<bool>,
    /// 文字集合の番号（128 は Shift_JIS）。名前で指すフォントで、元の形式が決めていれば
    pub charset: Option<u64>,
    /// ピッチと系統の番号。名前で指すフォントで、元の形式が決めていれば
    pub pitch: Option<u64>,
}

impl Font {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let kind = object.required("kind", FontKind::from_json)?;
        let bold = object.optional("bold", json::boolean)?;
        let italic = object.optional("italic", json::boolean)?;
        let charset = object.optional("charset", |value| json::uint(value, 0, 9007199254740991))?;
        let pitch = object.optional("pitch", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            name,
            kind,
            bold,
            italic,
            charset,
            pitch,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("kind");
        self.kind.to_json(out);
        if let Some(value) = &self.bold {
            out.key("bold");
            out.bool(*value);
        }
        if let Some(value) = &self.italic {
            out.key("italic");
            out.bool(*value);
        }
        if let Some(value) = &self.charset {
            out.key("charset");
            out.uint(*value);
        }
        if let Some(value) = &self.pitch {
            out.key("pitch");
            out.uint(*value);
        }
    }
}

impl FromJson for Font {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Font {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// フォントの種類。shape は形のファイル（SHX など）、named は名前で指すフォント（TrueType など）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FontKind {
    /// `shape`
    Shape,
    /// `named`
    Named,
}

impl FontKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Shape, Self::Named];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Named => "named",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "shape" => Some(Self::Shape),
            "named" => Some(Self::Named),
            _ => None,
        }
    }
}

impl FromJson for FontKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for FontKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つキー（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Source {
    /// L1 のレコードの番号
    pub record: Option<u64>,
    /// L1 のバイト範囲
    pub bytes: Option<ByteRange>,
    /// AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない
    pub handle: Option<String>,
    /// SXF のインスタンスの番号（#n の n）。10 進の文字列。マイナーバージョン 6 から
    pub instance: Option<String>,
}

impl Source {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let record = object.optional("record", |value| json::uint(value, 0, 9007199254740991))?;
        let bytes = object.optional("bytes", ByteRange::from_json)?;
        let handle = object.optional("handle", json::string)?;
        let instance = object.optional("instance", json::string)?;
        Ok(Self {
            record,
            bytes,
            handle,
            instance,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.record {
            out.key("record");
            out.uint(*value);
        }
        if let Some(value) = &self.bytes {
            out.key("bytes");
            value.to_json(out);
        }
        if let Some(value) = &self.handle {
            out.key("handle");
            out.string(value);
        }
        if let Some(value) = &self.instance {
            out.key("instance");
            out.string(value);
        }
    }
}

impl FromJson for Source {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Source {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断。人向けの文は持たず、文は describe で組み立てる（決定 0009）
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    /// 変わらない名前（budget.records.exceeded など）。catalog/codes.toml にある
    pub code: String,
    /// 重さ。code ごとに決まる
    pub severity: Severity,
    /// 変換の結果の段。code ごとに決まる。変換の結果を表さない code では持たない
    pub fidelity: Option<Fidelity>,
    /// まとめた件数
    pub count: u64,
    /// 場所。入力の順に先頭の数件
    pub at: Vec<Place>,
    /// 仕様の節
    pub cite: Option<Cite>,
    /// 文に差し込む値。名前と型は code ごとに台帳が決める。2^53 − 1 を超える整数は文字列にする
    pub params: BTreeMap<String, ParamValue>,
}

impl Diagnostic {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let code = object.required("code", json::string)?;
        let severity = object.required("severity", Severity::from_json)?;
        let fidelity = object.optional("fidelity", Fidelity::from_json)?;
        let count = object.required("count", |value| json::uint(value, 1, 9007199254740991))?;
        let at = object.required("at", |value| json::array(value, Place::from_json))?;
        let cite = object.optional("cite", Cite::from_json)?;
        let params = object.required("params", |value| json::map(value, ParamValue::from_json))?;
        Ok(Self {
            code,
            severity,
            fidelity,
            count,
            at,
            cite,
            params,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("code");
        out.string(&self.code);
        out.key("severity");
        self.severity.to_json(out);
        if let Some(value) = &self.fidelity {
            out.key("fidelity");
            value.to_json(out);
        }
        out.key("count");
        out.uint(self.count);
        out.key("at");
        out.array(&self.at, |out, item| item.to_json(out));
        if let Some(value) = &self.cite {
            out.key("cite");
            value.to_json(out);
        }
        out.key("params");
        out.map(&self.params, |out, item| item.to_json(out));
    }
}

impl FromJson for Diagnostic {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Diagnostic {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断の重さ（決定 0009）。info は何も失っていない、warning は結果を使える、error は失敗した
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// `info`
    Info,
    /// `warning`
    Warning,
    /// `error`
    Error,
}

impl Severity {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Info, Self::Warning, Self::Error];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

impl FromJson for Severity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Severity {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 変換の結果の段のうち、`native` 以外のもの（決定 0009）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fidelity {
    /// `equivalent`
    Equivalent,
    /// `approximated`
    Approximated,
    /// `drawn`
    Drawn,
    /// `dropped`
    Dropped,
}

impl Fidelity {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Equivalent,
        Self::Approximated,
        Self::Drawn,
        Self::Dropped,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Equivalent => "equivalent",
            Self::Approximated => "approximated",
            Self::Drawn => "drawn",
            Self::Dropped => "dropped",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "equivalent" => Some(Self::Equivalent),
            "approximated" => Some(Self::Approximated),
            "drawn" => Some(Self::Drawn),
            "dropped" => Some(Self::Dropped),
            _ => None,
        }
    }
}

impl FromJson for Fidelity {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Fidelity {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Place {
    /// ID
    pub id: Option<String>,
    /// L1 のレコードの番号
    pub record: Option<u64>,
    /// L1 のバイト範囲
    pub bytes: Option<ByteRange>,
}

impl Place {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.optional("id", json::string)?;
        let record = object.optional("record", |value| json::uint(value, 0, 9007199254740991))?;
        let bytes = object.optional("bytes", ByteRange::from_json)?;
        Ok(Self { id, record, bytes })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.record {
            out.key("record");
            out.uint(*value);
        }
        if let Some(value) = &self.bytes {
            out.key("bytes");
            value.to_json(out);
        }
    }
}

impl FromJson for Place {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Place {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// バイト範囲（start から end の手前まで）
#[derive(Clone, Debug, PartialEq)]
pub struct ByteRange {
    /// 先頭の位置
    pub start: u64,
    /// 終わりの位置（含まない）
    pub end: u64,
}

impl ByteRange {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let start = object.required("start", |value| json::uint(value, 0, 9007199254740991))?;
        let end = object.required("end", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { start, end })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("start");
        out.uint(self.start);
        out.key("end");
        out.uint(self.end);
    }
}

impl FromJson for ByteRange {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for ByteRange {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 仕様の節。仕様の本文は写さない
#[derive(Clone, Debug, PartialEq)]
pub struct Cite {
    /// 仕様の一覧での名前
    pub spec: String,
    /// 節
    pub section: String,
}

impl Cite {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let spec = object.required("spec", json::string)?;
        let section = object.required("section", json::string)?;
        Ok(Self { spec, section })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("spec");
        out.string(&self.spec);
        out.key("section");
        out.string(&self.section);
    }
}

impl FromJson for Cite {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Cite {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 診断の値。数か文字列
#[derive(Clone, Debug, PartialEq)]
pub enum ParamValue {
    /// 数
    Number(f64),
    /// 文字列。図面に記録された名前、列挙の値のキー、2^53 − 1 を超える整数
    String(String),
}

impl FromJson for ParamValue {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        match value {
            Value::Number(_) => json::number(value).map(Self::Number),
            Value::String(text) => Ok(Self::String(text.clone())),
            _ => Err(Invalid::new(Problem::Type)),
        }
    }
}

impl ToJson for ParamValue {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::Number(value) => out.float(*value),
            Self::String(value) => out.string(value),
        }
    }
}

/// バイナリの結果
#[derive(Clone, Debug, PartialEq)]
pub struct Blob {
    /// 番号。zm_take に渡す
    pub id: u64,
    /// 種類
    pub kind: BlobKind,
    /// バイト数
    pub size: u64,
}

impl Blob {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", |value| json::uint(value, 0, 4294967295))?;
        let kind = object.required("kind", BlobKind::from_json)?;
        let size = object.required("size", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { id, kind, size })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.uint(self.id);
        out.key("kind");
        self.kind.to_json(out);
        out.key("size");
        out.uint(self.size);
    }
}

impl FromJson for Blob {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Blob {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、scene3d は 3D のシーン（schema/scene3d.toml の配置）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる）、drawing は図面モデルのバイナリ（schema/drawing.toml の配置、決定 0247。マイナーバージョン 60 から）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlobKind {
    /// `file`
    File,
    /// `image`
    Image,
    /// `scene`
    Scene,
    /// `arrow`
    Arrow,
    /// `drawing`
    Drawing,
    /// `scene3d`
    Scene3d,
}

impl BlobKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::File,
        Self::Image,
        Self::Scene,
        Self::Arrow,
        Self::Drawing,
        Self::Scene3d,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Image => "image",
            Self::Scene => "scene",
            Self::Arrow => "arrow",
            Self::Drawing => "drawing",
            Self::Scene3d => "scene3d",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "file" => Some(Self::File),
            "image" => Some(Self::Image),
            "scene" => Some(Self::Scene),
            "arrow" => Some(Self::Arrow),
            "drawing" => Some(Self::Drawing),
            "scene3d" => Some(Self::Scene3d),
            _ => None,
        }
    }
}

impl FromJson for BlobKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for BlobKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// zm_session_new に渡す設定。渡さなければ既定の設定
#[derive(Clone, Debug, PartialEq)]
pub struct SessionOptions {
    /// メッセージのスキーマのメジャーバージョン。今は 1
    pub v: u64,
    /// 実行の形態。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013）
    pub engine: Option<Engine>,
    /// engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200）
    pub process_path: Option<String>,
}

impl SessionOptions {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let v = object.required("v", |value| json::uint(value, 1, 9007199254740991))?;
        let engine = object.optional("engine", Engine::from_json)?;
        let process_path = object.optional("process_path", json::string)?;
        Ok(Self {
            v,
            engine,
            process_path,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("v");
        out.uint(self.v);
        if let Some(value) = &self.engine {
            out.key("engine");
            value.to_json(out);
        }
        if let Some(value) = &self.process_path {
            out.key("process_path");
            out.string(value);
        }
    }
}

impl FromJson for SessionOptions {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for SessionOptions {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 実行の形態（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Engine {
    /// `native`
    Native,
    /// `process`
    Process,
    /// `wasm`
    Wasm,
}

impl Engine {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Native, Self::Process, Self::Wasm];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Process => "process",
            Self::Wasm => "wasm",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "native" => Some(Self::Native),
            "process" => Some(Self::Process),
            "wasm" => Some(Self::Wasm),
            _ => None,
        }
    }
}

impl FromJson for Engine {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for Engine {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML・IFC4.3・SIMA の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面・BIM・土木地理・点群のモジュールに置く（決定 0253）。マイナーバージョン 46 から（doc と id はマイナーバージョン 47 から）
#[derive(Clone, Debug, PartialEq)]
pub struct CivilRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// doc の座標点・高さ基準と、選んだ線形の IP・地盤縦断・片勾配・横断の値を返す。複数の線形があれば id が必要。省略は false。マイナーバージョン 87 から
    pub details: Option<bool>,
    /// 線形。doc と一緒には書かない
    pub alignment: Option<CivilAlignment>,
    /// open で開いた J-LandXML・IFC4.3・SIMA の文書の名前。マイナーバージョン 47 から
    pub doc: Option<String>,
    /// doc の線形の ID。文書に線形が 1 つなら書かなくてよい。マイナーバージョン 47 から
    pub id: Option<String>,
    /// 追加距離（メートル）の列
    pub distances: Option<Vec<f64>>,
    /// 測点の表記（No.12+5.000・No.12+5・No.12）の列。測点の不連続のために同じ値が何か所にもあれば、最初の所にして診断する
    pub stations: Option<Vec<String>>,
    /// 線形の初めから終わりまで、この間隔（メートル）ごとの所を足す。終わりの所も足す
    pub interval: Option<f64>,
    /// 測点の間隔（メートル）。書かなければ 20
    pub pitch: Option<f64>,
    /// 測点の表記の端数の小数の桁数。書かなければ 3
    pub digits: Option<u64>,
}

impl CivilRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        let details = object.optional("details", json::boolean)?;
        object.constant("op", "civil")?;
        let alignment = object.optional("alignment", CivilAlignment::from_json)?;
        let doc = object.optional("doc", json::string)?;
        let id = object.optional("id", json::string)?;
        let distances = object.optional("distances", |value| json::array(value, json::number))?;
        let stations = object.optional("stations", |value| json::array(value, json::string))?;
        let interval = object.optional("interval", json::number)?;
        let pitch = object.optional("pitch", json::number)?;
        let digits = object.optional("digits", |value| json::uint(value, 0, 9))?;
        Ok(Self {
            request_common,
            details,
            alignment,
            doc,
            id,
            distances,
            stations,
            interval,
            pitch,
            digits,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        if let Some(value) = &self.details {
            out.key("details");
            out.bool(*value);
        }
        out.key("op");
        out.string("civil");
        if let Some(value) = &self.alignment {
            out.key("alignment");
            value.to_json(out);
        }
        if let Some(value) = &self.doc {
            out.key("doc");
            out.string(value);
        }
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.distances {
            out.key("distances");
            out.array(value, |out, item| out.float(*item));
        }
        if let Some(value) = &self.stations {
            out.key("stations");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.interval {
            out.key("interval");
            out.float(*value);
        }
        if let Some(value) = &self.pitch {
            out.key("pitch");
            out.float(*value);
        }
        if let Some(value) = &self.digits {
            out.key("digits");
            out.uint(*value);
        }
    }
}

impl FromJson for CivilRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線形。点は地図の空間（北と東、メートル）に置き、方向角は北から時計回りのラジアンにする。半径は、右へ曲がるものを正にする（決定 0230）
#[derive(Clone, Debug, PartialEq)]
pub struct CivilAlignment {
    /// 平面線形の区間。線形の初めから順に並べる
    pub horizontal: Vec<CivilHorizontal>,
    /// 縦断線形の区間。書かなければ高さを返さない
    pub vertical: Option<Vec<CivilVertical>>,
    /// 線形の初めの測点の値（メートル）。書かなければ 0
    pub start_station: Option<f64>,
    /// 測点の不連続。追加距離の順に並べる
    pub breaks: Option<Vec<CivilStationBreak>>,
}

impl CivilAlignment {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let horizontal = object.required("horizontal", |value| {
            json::array(value, CivilHorizontal::from_json)
        })?;
        let vertical = object.optional("vertical", |value| {
            json::array(value, CivilVertical::from_json)
        })?;
        let start_station = object.optional("start_station", json::number)?;
        let breaks = object.optional("breaks", |value| {
            json::array(value, CivilStationBreak::from_json)
        })?;
        Ok(Self {
            horizontal,
            vertical,
            start_station,
            breaks,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("horizontal");
        out.array(&self.horizontal, |out, item| item.to_json(out));
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.start_station {
            out.key("start_station");
            out.float(*value);
        }
        if let Some(value) = &self.breaks {
            out.key("breaks");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for CivilAlignment {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilAlignment {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 平面線形の区間
#[derive(Clone, Debug, PartialEq)]
pub struct CivilHorizontal {
    /// 区間の種類
    pub kind: CivilHorizontalKind,
    /// 初めの点の北（メートル）
    pub northing: f64,
    /// 初めの点の東（メートル）
    pub easting: f64,
    /// 初めの方向角（北から時計回りのラジアン）
    pub direction: f64,
    /// 長さ（メートル）
    pub length: f64,
    /// 円弧の半径（メートル）。arc に書く
    pub radius: Option<f64>,
    /// 緩和曲線の初めの半径（メートル）。直線の側は書かない
    pub start_radius: Option<f64>,
    /// 緩和曲線の終わりの半径（メートル）。直線の側は書かない
    pub end_radius: Option<f64>,
    /// 緩和曲線の種類。transition に書き、書かなければ clothoid
    pub curve: Option<CivilTransitionCurve>,
}

impl CivilHorizontal {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", CivilHorizontalKind::from_json)?;
        let northing = object.required("northing", json::number)?;
        let easting = object.required("easting", json::number)?;
        let direction = object.required("direction", json::number)?;
        let length = object.required("length", json::number)?;
        let radius = object.optional("radius", json::number)?;
        let start_radius = object.optional("start_radius", json::number)?;
        let end_radius = object.optional("end_radius", json::number)?;
        let curve = object.optional("curve", CivilTransitionCurve::from_json)?;
        Ok(Self {
            kind,
            northing,
            easting,
            direction,
            length,
            radius,
            start_radius,
            end_radius,
            curve,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        self.kind.to_json(out);
        out.key("northing");
        out.float(self.northing);
        out.key("easting");
        out.float(self.easting);
        out.key("direction");
        out.float(self.direction);
        out.key("length");
        out.float(self.length);
        if let Some(value) = &self.radius {
            out.key("radius");
            out.float(*value);
        }
        if let Some(value) = &self.start_radius {
            out.key("start_radius");
            out.float(*value);
        }
        if let Some(value) = &self.end_radius {
            out.key("end_radius");
            out.float(*value);
        }
        if let Some(value) = &self.curve {
            out.key("curve");
            value.to_json(out);
        }
    }
}

impl FromJson for CivilHorizontal {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilHorizontal {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 縦断線形の区間
#[derive(Clone, Debug, PartialEq)]
pub struct CivilVertical {
    /// 区間の種類
    pub kind: CivilVerticalKind,
    /// 初めの追加距離（メートル）
    pub start_distance: f64,
    /// 初めの高さ（メートル）
    pub start_height: f64,
    /// 初めの勾配（高さ ÷ 平面の距離）
    pub start_gradient: f64,
    /// 平面の距離での長さ（メートル）
    pub length: f64,
    /// 放物線の終わりの勾配。parabola に書く
    pub end_gradient: Option<f64>,
    /// 円弧の半径（メートル）。上に凸のものを正にする。circular_arc に書く
    pub radius: Option<f64>,
}

impl CivilVertical {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", CivilVerticalKind::from_json)?;
        let start_distance = object.required("start_distance", json::number)?;
        let start_height = object.required("start_height", json::number)?;
        let start_gradient = object.required("start_gradient", json::number)?;
        let length = object.required("length", json::number)?;
        let end_gradient = object.optional("end_gradient", json::number)?;
        let radius = object.optional("radius", json::number)?;
        Ok(Self {
            kind,
            start_distance,
            start_height,
            start_gradient,
            length,
            end_gradient,
            radius,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        self.kind.to_json(out);
        out.key("start_distance");
        out.float(self.start_distance);
        out.key("start_height");
        out.float(self.start_height);
        out.key("start_gradient");
        out.float(self.start_gradient);
        out.key("length");
        out.float(self.length);
        if let Some(value) = &self.end_gradient {
            out.key("end_gradient");
            out.float(*value);
        }
        if let Some(value) = &self.radius {
            out.key("radius");
            out.float(*value);
        }
    }
}

impl FromJson for CivilVertical {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilVertical {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 平面線形の区間の種類。直線・円弧・緩和曲線
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CivilHorizontalKind {
    /// `line`
    Line,
    /// `arc`
    Arc,
    /// `transition`
    Transition,
}

impl CivilHorizontalKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Line, Self::Arc, Self::Transition];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Arc => "arc",
            Self::Transition => "transition",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "line" => Some(Self::Line),
            "arc" => Some(Self::Arc),
            "transition" => Some(Self::Transition),
            _ => None,
        }
    }
}

impl FromJson for CivilHorizontalKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CivilHorizontalKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 緩和曲線の種類。クロソイド・3 次放物線・サイン半波長逓減曲線・コサイン曲線・ブロス曲線・ヘルマート曲線・ウィーン曲線
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CivilTransitionCurve {
    /// `clothoid`
    Clothoid,
    /// `cubic_parabola`
    CubicParabola,
    /// `sine_half_wave`
    SineHalfWave,
    /// `cosine`
    Cosine,
    /// `bloss`
    Bloss,
    /// `helmert`
    Helmert,
    /// `viennese`
    Viennese,
}

impl CivilTransitionCurve {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Clothoid,
        Self::CubicParabola,
        Self::SineHalfWave,
        Self::Cosine,
        Self::Bloss,
        Self::Helmert,
        Self::Viennese,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Clothoid => "clothoid",
            Self::CubicParabola => "cubic_parabola",
            Self::SineHalfWave => "sine_half_wave",
            Self::Cosine => "cosine",
            Self::Bloss => "bloss",
            Self::Helmert => "helmert",
            Self::Viennese => "viennese",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "clothoid" => Some(Self::Clothoid),
            "cubic_parabola" => Some(Self::CubicParabola),
            "sine_half_wave" => Some(Self::SineHalfWave),
            "cosine" => Some(Self::Cosine),
            "bloss" => Some(Self::Bloss),
            "helmert" => Some(Self::Helmert),
            "viennese" => Some(Self::Viennese),
            _ => None,
        }
    }
}

impl FromJson for CivilTransitionCurve {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CivilTransitionCurve {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 縦断線形の区間の種類。一定の勾配・2 次放物線・円弧
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CivilVerticalKind {
    /// `constant_gradient`
    ConstantGradient,
    /// `parabola`
    Parabola,
    /// `circular_arc`
    CircularArc,
}

impl CivilVerticalKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::ConstantGradient, Self::Parabola, Self::CircularArc];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::ConstantGradient => "constant_gradient",
            Self::Parabola => "parabola",
            Self::CircularArc => "circular_arc",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "constant_gradient" => Some(Self::ConstantGradient),
            "parabola" => Some(Self::Parabola),
            "circular_arc" => Some(Self::CircularArc),
            _ => None,
        }
    }
}

impl FromJson for CivilVerticalKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for CivilVerticalKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 測点の不連続
#[derive(Clone, Debug, PartialEq)]
pub struct CivilStationBreak {
    /// 追加距離（メートル）
    pub distance: f64,
    /// 戻りの測点の値（不連続の手前から数えた値）
    pub back: f64,
    /// 進みの測点の値（不連続の先で数え直す値）
    pub ahead: f64,
}

impl CivilStationBreak {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let distance = object.required("distance", json::number)?;
        let back = object.required("back", json::number)?;
        let ahead = object.required("ahead", json::number)?;
        Ok(Self {
            distance,
            back,
            ahead,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("distance");
        out.float(self.distance);
        out.key("back");
        out.float(self.back);
        out.key("ahead");
        out.float(self.ahead);
    }
}

impl FromJson for CivilStationBreak {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilStationBreak {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線形の上の所
#[derive(Clone, Debug, PartialEq)]
pub struct CivilResult {
    /// details を求めた場合の保持値。マイナーバージョン 87 から
    pub details: Option<CivilDetails>,
    /// 平面線形の長さ（メートル）
    pub length: f64,
    /// 求めた所
    pub positions: Vec<CivilPosition>,
    /// 所を求めた doc の線形の ID。マイナーバージョン 47 から
    pub alignment: Option<String>,
    /// doc の線形の要約。doc を渡したときに返す。マイナーバージョン 47 から
    pub alignments: Option<Vec<CivilAlignmentSummary>>,
    /// doc の地表面（TIN）の要約。doc を渡したときに返す。マイナーバージョン 47 から
    pub surfaces: Option<Vec<CivilSurfaceSummary>>,
    /// doc の水平の座標系の EPSG のコード。分からなければ書かない。マイナーバージョン 47 から
    pub crs: Option<u64>,
}

impl CivilResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let details = object.optional("details", CivilDetails::from_json)?;
        let length = object.required("length", json::number)?;
        let positions = object.required("positions", |value| {
            json::array(value, CivilPosition::from_json)
        })?;
        let alignment = object.optional("alignment", json::string)?;
        let alignments = object.optional("alignments", |value| {
            json::array(value, CivilAlignmentSummary::from_json)
        })?;
        let surfaces = object.optional("surfaces", |value| {
            json::array(value, CivilSurfaceSummary::from_json)
        })?;
        let crs = object.optional("crs", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            details,
            length,
            positions,
            alignment,
            alignments,
            surfaces,
            crs,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.details {
            out.key("details");
            value.to_json(out);
        }
        out.key("length");
        out.float(self.length);
        out.key("positions");
        out.array(&self.positions, |out, item| item.to_json(out));
        if let Some(value) = &self.alignment {
            out.key("alignment");
            out.string(value);
        }
        if let Some(value) = &self.alignments {
            out.key("alignments");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.surfaces {
            out.key("surfaces");
            out.array(value, |out, item| item.to_json(out));
        }
        if let Some(value) = &self.crs {
            out.key("crs");
            out.uint(*value);
        }
    }
}

impl FromJson for CivilResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 土木モデルの保持値。スキーマ 1.87 から
#[derive(Clone, Debug, PartialEq)]
pub struct CivilDetails {
    /// 選んだ線形の値。線形がなければ書かない
    pub alignment: Option<CivilAlignmentValues>,
    /// 文書の座標点セット
    pub point_sets: Vec<CivilPointSet>,
    /// 文書の高さの基準。値は自動で T.P. に直さない
    pub height_reference: Option<CivilHeightReference>,
}

impl CivilDetails {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let alignment = object.optional("alignment", CivilAlignmentValues::from_json)?;
        let point_sets = object.required("point_sets", |value| {
            json::array(value, CivilPointSet::from_json)
        })?;
        let height_reference =
            object.optional("height_reference", CivilHeightReference::from_json)?;
        Ok(Self {
            alignment,
            point_sets,
            height_reference,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.alignment {
            out.key("alignment");
            value.to_json(out);
        }
        out.key("point_sets");
        out.array(&self.point_sets, |out, item| item.to_json(out));
        if let Some(value) = &self.height_reference {
            out.key("height_reference");
            value.to_json(out);
        }
    }
}

impl FromJson for CivilDetails {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilDetails {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 元の高さの基準
#[derive(Clone, Debug, PartialEq)]
pub struct CivilHeightReference {
    /// 元の基準名
    pub name: Option<String>,
    /// 元の高さに足すと T.P. の高さになる差（メートル）。不明なら書かない
    pub tokyo_bay_offset: Option<f64>,
    /// 元の位置
    pub source: Source,
}

impl CivilHeightReference {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let tokyo_bay_offset = object.optional("tokyo_bay_offset", json::number)?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            name,
            tokyo_bay_offset,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.tokyo_bay_offset {
            out.key("tokyo_bay_offset");
            out.float(*value);
        }
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for CivilHeightReference {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilHeightReference {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線形の付随する値。元の順序を保つ
#[derive(Clone, Debug, PartialEq)]
pub struct CivilAlignmentValues {
    /// 同じ名前の縦断が複数あり、名前で参照先を一意に定められない
    pub vertical_name_ambiguous: bool,
    /// 選んだ線形の ID
    pub id: String,
    /// 保持した縦断線形の元の名前
    pub vertical_name: Option<String>,
    /// IP の列。始点・終点を含む
    pub intersection_points: Vec<CivilSurveyPoint>,
    /// 地盤の縦断の折れ線
    pub ground_profiles: Vec<CivilGroundProfile>,
    /// 片勾配とカントの区間
    pub cant: Vec<CivilCant>,
    /// 横断の線と点
    pub cross_sections: Vec<CivilCrossSection>,
}

impl CivilAlignmentValues {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let vertical_name_ambiguous = object.required("vertical_name_ambiguous", json::boolean)?;
        let id = object.required("id", json::string)?;
        let vertical_name = object.optional("vertical_name", json::string)?;
        let intersection_points = object.required("intersection_points", |value| {
            json::array(value, CivilSurveyPoint::from_json)
        })?;
        let ground_profiles = object.required("ground_profiles", |value| {
            json::array(value, CivilGroundProfile::from_json)
        })?;
        let cant = object.required("cant", |value| json::array(value, CivilCant::from_json))?;
        let cross_sections = object.required("cross_sections", |value| {
            json::array(value, CivilCrossSection::from_json)
        })?;
        Ok(Self {
            vertical_name_ambiguous,
            id,
            vertical_name,
            intersection_points,
            ground_profiles,
            cant,
            cross_sections,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("vertical_name_ambiguous");
        out.bool(self.vertical_name_ambiguous);
        out.key("id");
        out.string(&self.id);
        if let Some(value) = &self.vertical_name {
            out.key("vertical_name");
            out.string(value);
        }
        out.key("intersection_points");
        out.array(&self.intersection_points, |out, item| item.to_json(out));
        out.key("ground_profiles");
        out.array(&self.ground_profiles, |out, item| item.to_json(out));
        out.key("cant");
        out.array(&self.cant, |out, item| item.to_json(out));
        out.key("cross_sections");
        out.array(&self.cross_sections, |out, item| item.to_json(out));
    }
}

impl FromJson for CivilAlignmentValues {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilAlignmentValues {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// IP・基準点などの座標点
#[derive(Clone, Debug, PartialEq)]
pub struct CivilSurveyPoint {
    /// 元の名前。永続 ID と同一視しない
    pub name: Option<String>,
    /// 注記
    pub description: Option<String>,
    /// 北の座標（メートル）
    pub northing: f64,
    /// 東の座標（メートル）
    pub easting: f64,
    /// 文書の基準での標高（メートル）。省略は 0 にしない
    pub height: Option<f64>,
    /// 基準点・水準点の種類
    pub class: Option<String>,
    /// 元の位置
    pub source: Source,
}

impl CivilSurveyPoint {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let northing = object.required("northing", json::number)?;
        let easting = object.required("easting", json::number)?;
        let height = object.optional("height", json::number)?;
        let class = object.optional("class", json::string)?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            name,
            description,
            northing,
            easting,
            height,
            class,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        out.key("northing");
        out.float(self.northing);
        out.key("easting");
        out.float(self.easting);
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.class {
            out.key("class");
            out.string(value);
        }
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for CivilSurveyPoint {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilSurveyPoint {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 元の順序を持つ座標点セット
#[derive(Clone, Debug, PartialEq)]
pub struct CivilPointSet {
    /// 元の名前
    pub name: Option<String>,
    /// 注記
    pub description: Option<String>,
    /// 点
    pub points: Vec<CivilSurveyPoint>,
    /// 元の位置
    pub source: Source,
}

impl CivilPointSet {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let points = object.required("points", |value| {
            json::array(value, CivilSurveyPoint::from_json)
        })?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            name,
            description,
            points,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        out.key("points");
        out.array(&self.points, |out, item| item.to_json(out));
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for CivilPointSet {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilPointSet {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地盤の縦断線
#[derive(Clone, Debug, PartialEq)]
pub struct CivilGroundProfile {
    /// 元の名前
    pub name: Option<String>,
    /// 注記
    pub description: Option<String>,
    /// 追加距離と高さ
    pub points: Vec<CivilProfilePoint>,
    /// 元の位置
    pub source: Source,
}

impl CivilGroundProfile {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let points = object.required("points", |value| {
            json::array(value, CivilProfilePoint::from_json)
        })?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            name,
            description,
            points,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        out.key("points");
        out.array(&self.points, |out, item| item.to_json(out));
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for CivilGroundProfile {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilGroundProfile {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地盤縦断の点
#[derive(Clone, Debug, PartialEq)]
pub struct CivilProfilePoint {
    /// 線形の初めからの追加距離（メートル）
    pub distance: f64,
    /// 文書の基準での標高（メートル）
    pub height: f64,
}

impl CivilProfilePoint {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let distance = object.required("distance", json::number)?;
        let height = object.required("height", json::number)?;
        Ok(Self { distance, height })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("distance");
        out.float(self.distance);
        out.key("height");
        out.float(self.height);
    }
}

impl FromJson for CivilProfilePoint {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilProfilePoint {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 片勾配またはカントの区間
#[derive(Clone, Debug, PartialEq)]
pub struct CivilCant {
    /// 初めの追加距離（メートル）
    pub start_distance: f64,
    /// 区間の長さ（メートル）
    pub length: f64,
    /// superelevation は道路の左右の勾配（%）、rail は鉄道の左右の高さの差（mm）
    pub kind: String,
    /// 初めの左と右
    pub start: Vec<f64>,
    /// 終わりの左と右
    pub end: Vec<f64>,
    /// 鉄道の軌間（mm）。不明なら書かない
    pub gauge: Option<f64>,
    /// 変わり方。元の値
    pub transition: Option<String>,
    /// 元の位置
    pub source: Source,
}

impl CivilCant {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let start_distance = object.required("start_distance", json::number)?;
        let length = object.required("length", json::number)?;
        let kind = object.required("kind", json::string)?;
        let start = object.required("start", |value| json::array(value, json::number))?;
        let end = object.required("end", |value| json::array(value, json::number))?;
        let gauge = object.optional("gauge", json::number)?;
        let transition = object.optional("transition", json::string)?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            start_distance,
            length,
            kind,
            start,
            end,
            gauge,
            transition,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("start_distance");
        out.float(self.start_distance);
        out.key("length");
        out.float(self.length);
        out.key("kind");
        out.string(&self.kind);
        out.key("start");
        out.array(&self.start, |out, item| out.float(*item));
        out.key("end");
        out.array(&self.end, |out, item| out.float(*item));
        if let Some(value) = &self.gauge {
            out.key("gauge");
            out.float(*value);
        }
        if let Some(value) = &self.transition {
            out.key("transition");
            out.string(value);
        }
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for CivilCant {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilCant {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 横断
#[derive(Clone, Debug, PartialEq)]
pub struct CivilCrossSection {
    /// 追加距離（メートル）
    pub distance: f64,
    /// 名前
    pub name: Option<String>,
    /// 参照する縦断線形の元の名前
    pub profile: Option<String>,
    /// 横断の線
    pub lines: Vec<CivilCrossSectionLine>,
    /// 元の位置
    pub source: Source,
}

impl CivilCrossSection {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let distance = object.required("distance", json::number)?;
        let name = object.optional("name", json::string)?;
        let profile = object.optional("profile", json::string)?;
        let lines = object.required("lines", |value| {
            json::array(value, CivilCrossSectionLine::from_json)
        })?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            distance,
            name,
            profile,
            lines,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("distance");
        out.float(self.distance);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.profile {
            out.key("profile");
            out.string(value);
        }
        out.key("lines");
        out.array(&self.lines, |out, item| item.to_json(out));
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for CivilCrossSection {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilCrossSection {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 横断の線。点の元の値を保つ
#[derive(Clone, Debug, PartialEq)]
pub struct CivilCrossSectionLine {
    /// 名前
    pub name: Option<String>,
    /// 注記
    pub description: Option<String>,
    /// 幅員中心から見た側。元の値。離れの符号は変えない
    pub side: Option<String>,
    /// existing、design または元の値
    pub role: String,
    /// elevation は文書の標高、formation_offset はその所の計画高からの差
    pub height_reference: String,
    /// 離れと元の高さ
    pub points: Vec<CivilCrossSectionPoint>,
    /// 元の位置
    pub source: Source,
}

impl CivilCrossSectionLine {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.optional("name", json::string)?;
        let description = object.optional("description", json::string)?;
        let side = object.optional("side", json::string)?;
        let role = object.required("role", json::string)?;
        let height_reference = object.required("height_reference", json::string)?;
        let points = object.required("points", |value| {
            json::array(value, CivilCrossSectionPoint::from_json)
        })?;
        let source = object.required("source", Source::from_json)?;
        Ok(Self {
            name,
            description,
            side,
            role,
            height_reference,
            points,
            source,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.description {
            out.key("description");
            out.string(value);
        }
        if let Some(value) = &self.side {
            out.key("side");
            out.string(value);
        }
        out.key("role");
        out.string(&self.role);
        out.key("height_reference");
        out.string(&self.height_reference);
        out.key("points");
        out.array(&self.points, |out, item| item.to_json(out));
        out.key("source");
        self.source.to_json(out);
    }
}

impl FromJson for CivilCrossSectionLine {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilCrossSectionLine {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 横断の点
#[derive(Clone, Debug, PartialEq)]
pub struct CivilCrossSectionPoint {
    /// 中心線からの離れ（右を正、メートル）
    pub offset: f64,
    /// 線の height_reference に従う高さ（メートル）
    pub height: f64,
}

impl CivilCrossSectionPoint {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let offset = object.required("offset", json::number)?;
        let height = object.required("height", json::number)?;
        Ok(Self { offset, height })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("offset");
        out.float(self.offset);
        out.key("height");
        out.float(self.height);
    }
}

impl FromJson for CivilCrossSectionPoint {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilCrossSectionPoint {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線形の要約
#[derive(Clone, Debug, PartialEq)]
pub struct CivilAlignmentSummary {
    /// 線形の ID
    pub id: String,
    /// 名前
    pub name: Option<String>,
    /// 平面線形の長さ（メートル）
    pub length: f64,
    /// 線形の初めの測点の値（メートル）
    pub start_station: f64,
    /// 平面線形の区間の数
    pub horizontal: u64,
    /// 縦断線形の区間の数
    pub vertical: u64,
    /// 測点の不連続の数
    pub breaks: u64,
    /// 横断の数
    pub cross_sections: u64,
}

impl CivilAlignmentSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.optional("name", json::string)?;
        let length = object.required("length", json::number)?;
        let start_station = object.required("start_station", json::number)?;
        let horizontal =
            object.required("horizontal", |value| json::uint(value, 0, 9007199254740991))?;
        let vertical =
            object.required("vertical", |value| json::uint(value, 0, 9007199254740991))?;
        let breaks = object.required("breaks", |value| json::uint(value, 0, 9007199254740991))?;
        let cross_sections = object.required("cross_sections", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            id,
            name,
            length,
            start_station,
            horizontal,
            vertical,
            breaks,
            cross_sections,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        out.key("length");
        out.float(self.length);
        out.key("start_station");
        out.float(self.start_station);
        out.key("horizontal");
        out.uint(self.horizontal);
        out.key("vertical");
        out.uint(self.vertical);
        out.key("breaks");
        out.uint(self.breaks);
        out.key("cross_sections");
        out.uint(self.cross_sections);
    }
}

impl FromJson for CivilAlignmentSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilAlignmentSummary {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地表面（TIN）の要約
#[derive(Clone, Debug, PartialEq)]
pub struct CivilSurfaceSummary {
    /// 地表面の ID
    pub id: String,
    /// 名前
    pub name: Option<String>,
    /// 役割。existing（現況）・design（計画）か、元の形式の値
    pub role: String,
    /// 点の数
    pub points: u64,
    /// 三角形の数
    pub triangles: u64,
}

impl CivilSurfaceSummary {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let name = object.optional("name", json::string)?;
        let role = object.required("role", json::string)?;
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        let triangles =
            object.required("triangles", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            id,
            name,
            role,
            points,
            triangles,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        out.key("role");
        out.string(&self.role);
        out.key("points");
        out.uint(self.points);
        out.key("triangles");
        out.uint(self.triangles);
    }
}

impl FromJson for CivilSurfaceSummary {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilSurfaceSummary {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 線形の上の 1 つの所
#[derive(Clone, Debug, PartialEq)]
pub struct CivilPosition {
    /// 追加距離（メートル）
    pub distance: f64,
    /// 測点の値（メートル）
    pub station: f64,
    /// 測点の表記。表記にできなければ書かない
    pub label: Option<String>,
    /// 点の北（メートル）
    pub northing: f64,
    /// 点の東（メートル）
    pub easting: f64,
    /// 接線の方向角（北から時計回りのラジアン）
    pub direction: f64,
    /// 縦断線形の高さ（メートル）。求められなければ書かない
    pub height: Option<f64>,
    /// 縦断線形の勾配。高さと同じく、求められなければ書かない
    pub gradient: Option<f64>,
}

impl CivilPosition {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let distance = object.required("distance", json::number)?;
        let station = object.required("station", json::number)?;
        let label = object.optional("label", json::string)?;
        let northing = object.required("northing", json::number)?;
        let easting = object.required("easting", json::number)?;
        let direction = object.required("direction", json::number)?;
        let height = object.optional("height", json::number)?;
        let gradient = object.optional("gradient", json::number)?;
        Ok(Self {
            distance,
            station,
            label,
            northing,
            easting,
            direction,
            height,
            gradient,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("distance");
        out.float(self.distance);
        out.key("station");
        out.float(self.station);
        if let Some(value) = &self.label {
            out.key("label");
            out.string(value);
        }
        out.key("northing");
        out.float(self.northing);
        out.key("easting");
        out.float(self.easting);
        out.key("direction");
        out.float(self.direction);
        if let Some(value) = &self.height {
            out.key("height");
            out.float(*value);
        }
        if let Some(value) = &self.gradient {
            out.key("gradient");
            out.float(*value);
        }
    }
}

impl FromJson for CivilPosition {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilPosition {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// open で開いた点群（LAS・LAZ・PLY・XYZ・E57・GeoTIFF の DEM）の要約と、条件に合う点を返す。点は八分木の浅い段のノードから順に選ぶ（決定 0232）。点群のモジュールに置く（決定 0253）。マイナーバージョン 48 から
#[derive(Clone, Debug, PartialEq)]
pub struct PointcloudRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// open で開いた点群の文書の名前
    pub doc: String,
    /// 点を選ぶ範囲。書かなければ全体
    pub region: Option<PointcloudBox>,
    /// 許容差（点の間隔、メートル）。これより細かい段の点を選ばない。書かなければすべての段
    pub tolerance: Option<f64>,
    /// 返す点の数の上限。書かなければ点を返さず、要約だけを返す
    pub limit: Option<u64>,
}

impl PointcloudRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "pointcloud")?;
        let doc = object.required("doc", json::string)?;
        let region = object.optional("region", PointcloudBox::from_json)?;
        let tolerance = object.optional("tolerance", json::number)?;
        let limit = object.optional("limit", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            request_common,
            doc,
            region,
            tolerance,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("pointcloud");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.region {
            out.key("region");
            value.to_json(out);
        }
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for PointcloudRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointcloudRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地図の空間の箱。北・東・標高（メートル）の最小と最大
#[derive(Clone, Debug, PartialEq)]
pub struct PointcloudBox {
    /// 最小の北・東・標高
    pub min: Vec<f64>,
    /// 最大の北・東・標高
    pub max: Vec<f64>,
}

impl PointcloudBox {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let min = object.required("min", |value| json::array(value, json::number))?;
        let max = object.required("max", |value| json::array(value, json::number))?;
        Ok(Self { min, max })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("min");
        out.array(&self.min, |out, item| out.float(*item));
        out.key("max");
        out.array(&self.max, |out, item| out.float(*item));
    }
}

impl FromJson for PointcloudBox {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointcloudBox {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 点群の要約と、選んだ点
#[derive(Clone, Debug, PartialEq)]
pub struct PointcloudResult {
    /// 点の数
    pub points: u64,
    /// 点の範囲。点がなければ書かない
    pub bounds: Option<PointcloudBox>,
    /// 水平の座標系の EPSG のコード。分からなければ書かない
    pub crs: Option<u64>,
    /// 高さの座標系の EPSG のコード。分からなければ書かない
    pub vertical: Option<u64>,
    /// 点が持つ属性（position・intensity・classification・color・returns・time）
    pub attributes: Vec<String>,
    /// 八分木のいちばん深い段
    pub depth: u64,
    /// 八分木のノードの数
    pub nodes: u64,
    /// 選んだ点。limit を書いたときに返す
    pub selected: Option<PointcloudPoints>,
}

impl PointcloudResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        let bounds = object.optional("bounds", PointcloudBox::from_json)?;
        let crs = object.optional("crs", |value| json::uint(value, 0, 9007199254740991))?;
        let vertical =
            object.optional("vertical", |value| json::uint(value, 0, 9007199254740991))?;
        let attributes = object.required("attributes", |value| json::array(value, json::string))?;
        let depth = object.required("depth", |value| json::uint(value, 0, 9007199254740991))?;
        let nodes = object.required("nodes", |value| json::uint(value, 0, 9007199254740991))?;
        let selected = object.optional("selected", PointcloudPoints::from_json)?;
        Ok(Self {
            points,
            bounds,
            crs,
            vertical,
            attributes,
            depth,
            nodes,
            selected,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("points");
        out.uint(self.points);
        if let Some(value) = &self.bounds {
            out.key("bounds");
            value.to_json(out);
        }
        if let Some(value) = &self.crs {
            out.key("crs");
            out.uint(*value);
        }
        if let Some(value) = &self.vertical {
            out.key("vertical");
            out.uint(*value);
        }
        out.key("attributes");
        out.array(&self.attributes, |out, item| out.string(item));
        out.key("depth");
        out.uint(self.depth);
        out.key("nodes");
        out.uint(self.nodes);
        if let Some(value) = &self.selected {
            out.key("selected");
            value.to_json(out);
        }
    }
}

impl FromJson for PointcloudResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointcloudResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 選んだ点。どの列も、選んだ点の順で同じ長さにする
#[derive(Clone, Debug, PartialEq)]
pub struct PointcloudPoints {
    /// 北・東・標高（メートル）
    pub positions: Vec<Vec<f64>>,
    /// 強度。点群が持つときに返す
    pub intensity: Option<Vec<u64>>,
    /// 分類。点群が持つときに返す
    pub classification: Option<Vec<u64>>,
    /// 色（赤・緑・青。16 ビット）。点群が持つときに返す
    pub color: Option<Vec<Vec<u64>>>,
    /// limit のために選ばなかった点がある
    pub truncated: bool,
    /// 選んだいちばん深い段
    pub level: u64,
}

impl PointcloudPoints {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let positions = object.required("positions", |value| {
            json::array(value, |value| json::array(value, json::number))
        })?;
        let intensity = object.optional("intensity", |value| {
            json::array(value, |value| json::uint(value, 0, 9007199254740991))
        })?;
        let classification = object.optional("classification", |value| {
            json::array(value, |value| json::uint(value, 0, 9007199254740991))
        })?;
        let color = object.optional("color", |value| {
            json::array(value, |value| {
                json::array(value, |value| json::uint(value, 0, 9007199254740991))
            })
        })?;
        let truncated = object.required("truncated", json::boolean)?;
        let level = object.required("level", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            positions,
            intensity,
            classification,
            color,
            truncated,
            level,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("positions");
        out.array(&self.positions, |out, item| {
            out.array(item, |out, item| out.float(*item))
        });
        if let Some(value) = &self.intensity {
            out.key("intensity");
            out.array(value, |out, item| out.uint(*item));
        }
        if let Some(value) = &self.classification {
            out.key("classification");
            out.array(value, |out, item| out.uint(*item));
        }
        if let Some(value) = &self.color {
            out.key("color");
            out.array(value, |out, item| {
                out.array(item, |out, item| out.uint(*item))
            });
        }
        out.key("truncated");
        out.bool(self.truncated);
        out.key("level");
        out.uint(self.level);
    }
}

impl FromJson for PointcloudPoints {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PointcloudPoints {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 管理 XML の項目を変更し、file blob を返す。入力は変更しない。すべての経路は元の文書で解き、失敗時は結果を返さない。図面のモジュールに置く。マイナーバージョン 84 から
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryApplyRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 管理 XML の入力の名前
    pub input: String,
    /// 元の XML の文書ハッシュ。違えば変更しない。省略すれば検査しない
    pub hash: Option<String>,
    /// 指定する順の変更。同じ親への追加は順序を保つ
    pub edits: Vec<DeliveryEdit>,
}

impl DeliveryApplyRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "delivery_apply")?;
        let input = object.required("input", json::string)?;
        let hash = object.optional("hash", json::string)?;
        let edits =
            object.required("edits", |value| json::array(value, DeliveryEdit::from_json))?;
        Ok(Self {
            request_common,
            input,
            hash,
            edits,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("delivery_apply");
        out.key("input");
        out.string(&self.input);
        if let Some(value) = &self.hash {
            out.key("hash");
            out.string(value);
        }
        out.key("edits");
        out.array(&self.edits, |out, item| item.to_json(out));
    }
}

impl FromJson for DeliveryApplyRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliveryApplyRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 変更後の管理 XML の情報。file blob は 1 個。入力は変更しない
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryApplyResult {
    /// 元の入力の名前
    pub input: String,
    /// 変更後のバイト列の文書ハッシュ
    pub hash: String,
    /// 適用した変更の数
    pub applied: u64,
}

impl DeliveryApplyResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let input = object.required("input", json::string)?;
        let hash = object.required("hash", json::string)?;
        let applied = object.required("applied", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            input,
            hash,
            applied,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("input");
        out.string(&self.input);
        out.key("hash");
        out.string(&self.hash);
        out.key("applied");
        out.uint(self.applied);
    }
}

impl FromJson for DeliveryApplyResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliveryApplyResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 根からたどる子。接頭辞を含む名前で区別する
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryPathStep {
    /// 接頭辞を含む元の要素名
    pub name: String,
    /// 同名の兄弟の中の位置。0 から数え、省略すると 0
    pub index: Option<u64>,
}

impl DeliveryPathStep {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let index = object.optional("index", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { name, index })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.index {
            out.key("index");
            out.uint(*value);
        }
    }
}

impl FromJson for DeliveryPathStep {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliveryPathStep {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 親の名前空間を引き継ぐ追加要素。属性と名前空間宣言は追加しない
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryNewElement {
    /// 接頭辞を含む要素名。接頭辞は親で宣言済みのものに限る
    pub name: String,
    /// 子の前に書く文字。文字参照にする前の値
    pub value: Option<String>,
    /// 子の要素。書く順。省略すると空
    pub children: Option<Vec<DeliveryNewElement>>,
}

impl DeliveryNewElement {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let value = object.optional("value", json::string)?;
        let children = object.optional("children", |value| {
            json::array(value, DeliveryNewElement::from_json)
        })?;
        Ok(Self {
            name,
            value,
            children,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.value {
            out.key("value");
            out.string(value);
        }
        if let Some(value) = &self.children {
            out.key("children");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for DeliveryNewElement {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliveryNewElement {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 元の文書の項目の更新か、子の記録の追加。空の path は根を指す。同じ項目の更新を重ねたり、更新と追加を混ぜたりしない
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum DeliveryEdit {
    /// 子の要素を持たない項目の文字を置き換える。注記と処理命令は保つ
    SetText(DeliverySetText),
    /// 親の末尾に子の記録を足す。名前・文字コード・予算を検査する
    Append(DeliveryAppend),
}

impl DeliveryEdit {
    /// タグのフィールド（`kind`）の値（スキーマの順）
    pub const TAGS: &[&str] = &["set_text", "append"];

    /// タグのフィールド（`kind`）の値
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::SetText(_) => "set_text",
            Self::Append(_) => "append",
        }
    }
}

impl FromJson for DeliveryEdit {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = match object.tag("kind")? {
            "set_text" => Self::SetText(DeliverySetText::read_fields(&mut object)?),
            "append" => Self::Append(DeliveryAppend::read_fields(&mut object)?),
            other => return Err(Invalid::new(Problem::Tag(other.to_string())).in_key("kind")),
        };
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliveryEdit {
    fn to_json(&self, out: &mut Writer) {
        match self {
            Self::SetText(value) => value.to_json(out),
            Self::Append(value) => value.to_json(out),
        }
    }
}

/// 子の要素を持たない項目の文字を置き換える。注記と処理命令は保つ
#[derive(Clone, Debug, PartialEq)]
pub struct DeliverySetText {
    /// 根からの経路
    pub path: Vec<DeliveryPathStep>,
    /// 文字参照にする前の値
    pub value: String,
}

impl DeliverySetText {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "set_text")?;
        let path = object.required("path", |value| {
            json::array(value, DeliveryPathStep::from_json)
        })?;
        let value = object.required("value", json::string)?;
        Ok(Self { path, value })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("set_text");
        out.key("path");
        out.array(&self.path, |out, item| item.to_json(out));
        out.key("value");
        out.string(&self.value);
    }
}

impl FromJson for DeliverySetText {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliverySetText {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 親の末尾に子の記録を足す。名前・文字コード・予算を検査する
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryAppend {
    /// 親の根からの経路
    pub path: Vec<DeliveryPathStep>,
    /// 追加する記録の木
    pub child: DeliveryNewElement,
}

impl DeliveryAppend {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        object.constant("kind", "append")?;
        let path = object.required("path", |value| {
            json::array(value, DeliveryPathStep::from_json)
        })?;
        let child = object.required("child", DeliveryNewElement::from_json)?;
        Ok(Self { path, child })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string("append");
        out.key("path");
        out.array(&self.path, |out, item| item.to_json(out));
        out.key("child");
        self.child.to_json(out);
    }
}

impl FromJson for DeliveryAppend {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliveryAppend {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。マイナーバージョン 53 から
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryCheckRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// 成果品の起点。/ で終わる入力の名前の前の部分（納品/ など）。archive と一緒には書かない
    pub root: Option<String>,
    /// 成果品の ZIP の入力の名前。root と一緒には書かない
    pub archive: Option<String>,
    /// プロファイルの入力の名前の列。extends で指す先も、すべて渡す
    pub profiles: Option<Vec<String>>,
    /// 照合に使うプロファイル。profiles の中から選ぶ
    pub profile: Option<ProfileRef>,
    /// 返す指摘の初めの位置
    pub offset: Option<u64>,
    /// 返す指摘の数の上限。書かなければ 10000
    pub limit: Option<u64>,
}

impl DeliveryCheckRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "delivery_check")?;
        let root = object.optional("root", json::string)?;
        let archive = object.optional("archive", json::string)?;
        let profiles = object.optional("profiles", |value| json::array(value, json::string))?;
        let profile = object.optional("profile", ProfileRef::from_json)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 0, 10000))?;
        Ok(Self {
            request_common,
            root,
            archive,
            profiles,
            profile,
            offset,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("delivery_check");
        if let Some(value) = &self.root {
            out.key("root");
            out.string(value);
        }
        if let Some(value) = &self.archive {
            out.key("archive");
            out.string(value);
        }
        if let Some(value) = &self.profiles {
            out.key("profiles");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.profile {
            out.key("profile");
            value.to_json(out);
        }
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for DeliveryCheckRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliveryCheckRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 成果品の要約と、照合の結果。照合の結果は check と同じ形
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryCheckResult {
    /// 成果品のファイルの数
    pub files: u64,
    /// 管理ファイルの要約。名前の順
    pub managed: Vec<DeliveryManaged>,
    /// 照合に使ったプロファイルの鎖。基から順に。プロファイルを渡さなければ空
    pub chain: Vec<ProfileIdentity>,
    /// 規則ごとの指摘の数
    pub rules: Vec<CheckRule>,
    /// 指摘
    pub findings: Vec<CheckFinding>,
    /// 指摘の数の全体
    pub total: u64,
    /// 続きの指摘の位置。続きがなければ書かない
    pub next_offset: Option<u64>,
}

impl DeliveryCheckResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let files = object.required("files", |value| json::uint(value, 0, 9007199254740991))?;
        let managed = object.required("managed", |value| {
            json::array(value, DeliveryManaged::from_json)
        })?;
        let chain = object.required("chain", |value| {
            json::array(value, ProfileIdentity::from_json)
        })?;
        let rules = object.required("rules", |value| json::array(value, CheckRule::from_json))?;
        let findings = object.required("findings", |value| {
            json::array(value, CheckFinding::from_json)
        })?;
        let total = object.required("total", |value| json::uint(value, 0, 9007199254740991))?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            files,
            managed,
            chain,
            rules,
            findings,
            total,
            next_offset,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("files");
        out.uint(self.files);
        out.key("managed");
        out.array(&self.managed, |out, item| item.to_json(out));
        out.key("chain");
        out.array(&self.chain, |out, item| item.to_json(out));
        out.key("rules");
        out.array(&self.rules, |out, item| item.to_json(out));
        out.key("findings");
        out.array(&self.findings, |out, item| item.to_json(out));
        out.key("total");
        out.uint(self.total);
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
    }
}

impl FromJson for DeliveryCheckResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliveryCheckResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 工事写真（JPEG）の Exif と XMP を読み、画素数・向き・撮影の日時・位置などを返す（#387）。画像は復号しない。図面のモジュールに置く。マイナーバージョン 55 から
#[derive(Clone, Debug, PartialEq)]
pub struct PhotoRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// JPEG の入力の名前
    pub input: String,
}

impl PhotoRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "photo")?;
        let input = object.required("input", json::string)?;
        Ok(Self {
            request_common,
            input,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("photo");
        out.key("input");
        out.string(&self.input);
    }
}

impl FromJson for PhotoRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PhotoRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 写真の情報。ファイルが持たない値は書かない
#[derive(Clone, Debug, PartialEq)]
pub struct PhotoResult {
    /// 画素の幅（フレームのヘッダー）
    pub width: Option<u64>,
    /// 画素の高さ（フレームのヘッダー）
    pub height: Option<u64>,
    /// 向き（Exif の Orientation、1〜8）
    pub orientation: Option<u64>,
    /// 撮影の日時（YYYY-MM-DDTHH:MM:SS、時差があれば +09:00 などを足す）
    pub taken: Option<String>,
    /// 機器の作り手
    pub make: Option<String>,
    /// 機器の型
    pub model: Option<String>,
    /// 撮った位置（GPS の緯度・経度、度）。測地系は換算しない
    pub position: Option<PhotoPosition>,
    /// 撮った向き（度）
    pub direction: Option<f64>,
    /// 向きの基準（T は真北、M は磁北）
    pub direction_reference: Option<String>,
    /// GPS の測地系の名前（GPSMapDatum）
    pub datum: Option<String>,
    /// XMP の性質。文書の順
    pub xmp: Vec<PhotoProperty>,
}

impl PhotoResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let width = object.optional("width", |value| json::uint(value, 0, 9007199254740991))?;
        let height = object.optional("height", |value| json::uint(value, 0, 9007199254740991))?;
        let orientation = object.optional("orientation", |value| json::uint(value, 1, 8))?;
        let taken = object.optional("taken", json::string)?;
        let make = object.optional("make", json::string)?;
        let model = object.optional("model", json::string)?;
        let position = object.optional("position", PhotoPosition::from_json)?;
        let direction = object.optional("direction", json::number)?;
        let direction_reference = object.optional("direction_reference", json::string)?;
        let datum = object.optional("datum", json::string)?;
        let xmp = object.required("xmp", |value| json::array(value, PhotoProperty::from_json))?;
        Ok(Self {
            width,
            height,
            orientation,
            taken,
            make,
            model,
            position,
            direction,
            direction_reference,
            datum,
            xmp,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.width {
            out.key("width");
            out.uint(*value);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.uint(*value);
        }
        if let Some(value) = &self.orientation {
            out.key("orientation");
            out.uint(*value);
        }
        if let Some(value) = &self.taken {
            out.key("taken");
            out.string(value);
        }
        if let Some(value) = &self.make {
            out.key("make");
            out.string(value);
        }
        if let Some(value) = &self.model {
            out.key("model");
            out.string(value);
        }
        if let Some(value) = &self.position {
            out.key("position");
            value.to_json(out);
        }
        if let Some(value) = &self.direction {
            out.key("direction");
            out.float(*value);
        }
        if let Some(value) = &self.direction_reference {
            out.key("direction_reference");
            out.string(value);
        }
        if let Some(value) = &self.datum {
            out.key("datum");
            out.string(value);
        }
        out.key("xmp");
        out.array(&self.xmp, |out, item| item.to_json(out));
    }
}

impl FromJson for PhotoResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PhotoResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 撮った位置
#[derive(Clone, Debug, PartialEq)]
pub struct PhotoPosition {
    /// 緯度（度、北が正）
    pub latitude: f64,
    /// 経度（度、東が正）
    pub longitude: f64,
    /// 高さ（m、海面より下は負）
    pub altitude: Option<f64>,
}

impl PhotoPosition {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let latitude = object.required("latitude", json::number)?;
        let longitude = object.required("longitude", json::number)?;
        let altitude = object.optional("altitude", json::number)?;
        Ok(Self {
            latitude,
            longitude,
            altitude,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("latitude");
        out.float(self.latitude);
        out.key("longitude");
        out.float(self.longitude);
        if let Some(value) = &self.altitude {
            out.key("altitude");
            out.float(*value);
        }
    }
}

impl FromJson for PhotoPosition {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PhotoPosition {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// XMP の性質
#[derive(Clone, Debug, PartialEq)]
pub struct PhotoProperty {
    /// 名前空間の URI
    pub namespace: String,
    /// 接頭辞を除いた名前
    pub name: String,
    /// 値。配列は最初の項目の値
    pub value: String,
}

impl PhotoProperty {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let namespace = object.required("namespace", json::string)?;
        let name = object.required("name", json::string)?;
        let value = object.required("value", json::string)?;
        Ok(Self {
            namespace,
            name,
            value,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("namespace");
        out.string(&self.namespace);
        out.key("name");
        out.string(&self.name);
        out.key("value");
        out.string(&self.value);
    }
}

impl FromJson for PhotoProperty {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for PhotoProperty {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地質・土質調査のボーリング交換用データ（BED0500.DTD の XML）を読み、ボーリングの位置・標高と、地層・色調・観察記事・標準貫入試験・孔内水位を返す（#390）。ほかの試験と判定表は読まない。図面のモジュールに置く。マイナーバージョン 57 から
#[derive(Clone, Debug, PartialEq)]
pub struct BoringRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// ボーリング交換用データの入力の名前
    pub input: String,
}

impl BoringRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "boring")?;
        let input = object.required("input", json::string)?;
        Ok(Self {
            request_common,
            input,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("boring");
        out.key("input");
        out.string(&self.input);
    }
}

impl FromJson for BoringRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BoringRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 読んだボーリング。深さは地表からの m（GL.-m）
#[derive(Clone, Debug, PartialEq)]
pub struct BoringResult {
    /// 適用規格。文書の順
    pub standards: Vec<String>,
    /// 公開フラグのコード
    pub publication: Option<String>,
    /// 事業工事名
    pub project: Option<String>,
    /// 調査名
    pub survey: Option<String>,
    /// ボーリング名
    pub name: Option<String>,
    /// ボーリング総数
    pub count: Option<u64>,
    /// ボーリング連番
    pub serial: Option<u64>,
    /// 孔口の位置。測地系は換算しない
    pub position: Option<BoringPosition>,
    /// 位置の測地系
    pub datum: Option<BoringDatum>,
    /// 調査位置の住所
    pub address: Option<String>,
    /// 調査期間の始まりの日（YYYY-MM-DD）
    pub start: Option<String>,
    /// 調査期間の終わりの日（YYYY-MM-DD）
    pub end: Option<String>,
    /// 孔口標高
    pub elevation: Option<BoringHeight>,
    /// 総削孔長（m）
    pub length: Option<f64>,
    /// 削孔の角度（度）
    pub angle: Option<f64>,
    /// 削孔の方位（度）
    pub direction: Option<f64>,
    /// 工学的地質区分名・現場土質名。文書の順
    pub layers: Vec<BoringLayer>,
    /// 色調。文書の順
    pub colors: Vec<BoringColor>,
    /// 観察記事。文書の順
    pub observations: Vec<BoringObservation>,
    /// 標準貫入試験。文書の順
    pub penetrations: Vec<BoringPenetration>,
    /// 孔内水位。文書の順
    pub water_levels: Vec<BoringWaterLevel>,
}

impl BoringResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let standards = object.required("standards", |value| json::array(value, json::string))?;
        let publication = object.optional("publication", json::string)?;
        let project = object.optional("project", json::string)?;
        let survey = object.optional("survey", json::string)?;
        let name = object.optional("name", json::string)?;
        let count = object.optional("count", |value| json::uint(value, 0, 9007199254740991))?;
        let serial = object.optional("serial", |value| json::uint(value, 0, 9007199254740991))?;
        let position = object.optional("position", BoringPosition::from_json)?;
        let datum = object.optional("datum", BoringDatum::from_json)?;
        let address = object.optional("address", json::string)?;
        let start = object.optional("start", json::string)?;
        let end = object.optional("end", json::string)?;
        let elevation = object.optional("elevation", BoringHeight::from_json)?;
        let length = object.optional("length", json::number)?;
        let angle = object.optional("angle", json::number)?;
        let direction = object.optional("direction", json::number)?;
        let layers =
            object.required("layers", |value| json::array(value, BoringLayer::from_json))?;
        let colors =
            object.required("colors", |value| json::array(value, BoringColor::from_json))?;
        let observations = object.required("observations", |value| {
            json::array(value, BoringObservation::from_json)
        })?;
        let penetrations = object.required("penetrations", |value| {
            json::array(value, BoringPenetration::from_json)
        })?;
        let water_levels = object.required("water_levels", |value| {
            json::array(value, BoringWaterLevel::from_json)
        })?;
        Ok(Self {
            standards,
            publication,
            project,
            survey,
            name,
            count,
            serial,
            position,
            datum,
            address,
            start,
            end,
            elevation,
            length,
            angle,
            direction,
            layers,
            colors,
            observations,
            penetrations,
            water_levels,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("standards");
        out.array(&self.standards, |out, item| out.string(item));
        if let Some(value) = &self.publication {
            out.key("publication");
            out.string(value);
        }
        if let Some(value) = &self.project {
            out.key("project");
            out.string(value);
        }
        if let Some(value) = &self.survey {
            out.key("survey");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.count {
            out.key("count");
            out.uint(*value);
        }
        if let Some(value) = &self.serial {
            out.key("serial");
            out.uint(*value);
        }
        if let Some(value) = &self.position {
            out.key("position");
            value.to_json(out);
        }
        if let Some(value) = &self.datum {
            out.key("datum");
            value.to_json(out);
        }
        if let Some(value) = &self.address {
            out.key("address");
            out.string(value);
        }
        if let Some(value) = &self.start {
            out.key("start");
            out.string(value);
        }
        if let Some(value) = &self.end {
            out.key("end");
            out.string(value);
        }
        if let Some(value) = &self.elevation {
            out.key("elevation");
            value.to_json(out);
        }
        if let Some(value) = &self.length {
            out.key("length");
            out.float(*value);
        }
        if let Some(value) = &self.angle {
            out.key("angle");
            out.float(*value);
        }
        if let Some(value) = &self.direction {
            out.key("direction");
            out.float(*value);
        }
        out.key("layers");
        out.array(&self.layers, |out, item| item.to_json(out));
        out.key("colors");
        out.array(&self.colors, |out, item| item.to_json(out));
        out.key("observations");
        out.array(&self.observations, |out, item| item.to_json(out));
        out.key("penetrations");
        out.array(&self.penetrations, |out, item| item.to_json(out));
        out.key("water_levels");
        out.array(&self.water_levels, |out, item| item.to_json(out));
    }
}

impl FromJson for BoringResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BoringResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 経度・緯度（度）。度・分・秒から求める
#[derive(Clone, Debug, PartialEq)]
pub struct BoringPosition {
    /// 緯度（度、北が正）
    pub latitude: f64,
    /// 経度（度、東が正）
    pub longitude: f64,
}

impl BoringPosition {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let latitude = object.required("latitude", json::number)?;
        let longitude = object.required("longitude", json::number)?;
        Ok(Self {
            latitude,
            longitude,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("latitude");
        out.float(self.latitude);
        out.key("longitude");
        out.float(self.longitude);
    }
}

impl FromJson for BoringPosition {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BoringPosition {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 測地系。tokyo は日本測地系（コード 00）、jgd2000・jgd2011・jgd2024 は世界測地系（コード 01〜03）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BoringDatum {
    /// `tokyo`
    Tokyo,
    /// `jgd2000`
    Jgd2000,
    /// `jgd2011`
    Jgd2011,
    /// `jgd2024`
    Jgd2024,
}

impl BoringDatum {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Tokyo, Self::Jgd2000, Self::Jgd2011, Self::Jgd2024];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tokyo => "tokyo",
            Self::Jgd2000 => "jgd2000",
            Self::Jgd2011 => "jgd2011",
            Self::Jgd2024 => "jgd2024",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "tokyo" => Some(Self::Tokyo),
            "jgd2000" => Some(Self::Jgd2000),
            "jgd2011" => Some(Self::Jgd2011),
            "jgd2024" => Some(Self::Jgd2024),
            _ => None,
        }
    }
}

impl FromJson for BoringDatum {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for BoringDatum {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 標高と、その基準と成果（決定 0019）
#[derive(Clone, Debug, PartialEq)]
pub struct BoringHeight {
    /// 標高（m）
    pub value: f64,
    /// 高さの基準。孔口標高は tokyo_bay（東京湾平均海面）とする
    pub datum: HeightDatum,
    /// 成果。測地系が jgd2024 なら survey_2024、そうでなければ調査の日から仮定する
    pub realization: HeightRealization,
}

impl BoringHeight {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let value = object.required("value", json::number)?;
        let datum = object.required("datum", HeightDatum::from_json)?;
        let realization = object.required("realization", HeightRealization::from_json)?;
        Ok(Self {
            value,
            datum,
            realization,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("value");
        out.float(self.value);
        out.key("datum");
        self.datum.to_json(out);
        out.key("realization");
        self.realization.to_json(out);
    }
}

impl FromJson for BoringHeight {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BoringHeight {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 高さの基準（決定 0019）。tokyo_bay は東京湾平均海面（T.P.）、island は離島の独自の平均海面、ellipsoid は楕円体（GRS80）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HeightDatum {
    /// `tokyo_bay`
    TokyoBay,
    /// `island`
    Island,
    /// `ellipsoid`
    Ellipsoid,
}

impl HeightDatum {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::TokyoBay, Self::Island, Self::Ellipsoid];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::TokyoBay => "tokyo_bay",
            Self::Island => "island",
            Self::Ellipsoid => "ellipsoid",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "tokyo_bay" => Some(Self::TokyoBay),
            "island" => Some(Self::Island),
            "ellipsoid" => Some(Self::Ellipsoid),
            _ => None,
        }
    }
}

impl FromJson for HeightDatum {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for HeightDatum {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 標高の成果（決定 0019）。survey_2011 は測地成果2011（2025-03-31 まで）、survey_2024 は測地成果2024（2025-04-01 から）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HeightRealization {
    /// `survey_2011`
    Survey2011,
    /// `survey_2024`
    Survey2024,
}

impl HeightRealization {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Survey2011, Self::Survey2024];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Survey2011 => "survey_2011",
            Self::Survey2024 => "survey_2024",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "survey_2011" => Some(Self::Survey2011),
            "survey_2024" => Some(Self::Survey2024),
            _ => None,
        }
    }
}

impl FromJson for HeightRealization {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for HeightRealization {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 地層の区間。上端は 1 つ前の区間の下端（最初は 0）
#[derive(Clone, Debug, PartialEq)]
pub struct BoringLayer {
    /// 下端の深さ（m）
    pub bottom: f64,
    /// 工学的地質区分名・現場土質名
    pub name: String,
    /// 記号
    pub symbol: Option<String>,
}

impl BoringLayer {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let bottom = object.required("bottom", json::number)?;
        let name = object.required("name", json::string)?;
        let symbol = object.optional("symbol", json::string)?;
        Ok(Self {
            bottom,
            name,
            symbol,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("bottom");
        out.float(self.bottom);
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.symbol {
            out.key("symbol");
            out.string(value);
        }
    }
}

impl FromJson for BoringLayer {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BoringLayer {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 色調の区間
#[derive(Clone, Debug, PartialEq)]
pub struct BoringColor {
    /// 下端の深さ（m）
    pub bottom: f64,
    /// 色調の名前
    pub name: Option<String>,
}

impl BoringColor {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let bottom = object.required("bottom", json::number)?;
        let name = object.optional("name", json::string)?;
        Ok(Self { bottom, name })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("bottom");
        out.float(self.bottom);
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
    }
}

impl FromJson for BoringColor {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BoringColor {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 観察記事
#[derive(Clone, Debug, PartialEq)]
pub struct BoringObservation {
    /// 上端の深さ（m）
    pub top: f64,
    /// 下端の深さ（m）
    pub bottom: f64,
    /// 記事
    pub text: Option<String>,
}

impl BoringObservation {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let top = object.required("top", json::number)?;
        let bottom = object.required("bottom", json::number)?;
        let text = object.optional("text", json::string)?;
        Ok(Self { top, bottom, text })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("top");
        out.float(self.top);
        out.key("bottom");
        out.float(self.bottom);
        if let Some(value) = &self.text {
            out.key("text");
            out.string(value);
        }
    }
}

impl FromJson for BoringObservation {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BoringObservation {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 標準貫入試験
#[derive(Clone, Debug, PartialEq)]
pub struct BoringPenetration {
    /// 開始の深さ（m）
    pub depth: f64,
    /// 合計の打撃回数
    pub blows: u64,
    /// 合計の貫入量（mm）
    pub penetration: f64,
}

impl BoringPenetration {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let depth = object.required("depth", json::number)?;
        let blows = object.required("blows", |value| json::uint(value, 0, 9007199254740991))?;
        let penetration = object.required("penetration", json::number)?;
        Ok(Self {
            depth,
            blows,
            penetration,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("depth");
        out.float(self.depth);
        out.key("blows");
        out.uint(self.blows);
        out.key("penetration");
        out.float(self.penetration);
    }
}

impl FromJson for BoringPenetration {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BoringPenetration {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 孔内水位
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoringWaterLevel {
    /// 測定の日（YYYY-MM-DD）
    pub date: Option<String>,
    /// 削孔状況のコード
    pub state: Option<String>,
    /// 水位の深さ（m）。地表より高ければ負。水位がなければ書かない
    pub depth: Option<f64>,
    /// 水位種別・備考
    pub note: Option<String>,
}

impl BoringWaterLevel {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let date = object.optional("date", json::string)?;
        let state = object.optional("state", json::string)?;
        let depth = object.optional("depth", json::number)?;
        let note = object.optional("note", json::string)?;
        Ok(Self {
            date,
            state,
            depth,
            note,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.date {
            out.key("date");
            out.string(value);
        }
        if let Some(value) = &self.state {
            out.key("state");
            out.string(value);
        }
        if let Some(value) = &self.depth {
            out.key("depth");
            out.float(*value);
        }
        if let Some(value) = &self.note {
            out.key("note");
            out.string(value);
        }
    }
}

impl FromJson for BoringWaterLevel {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for BoringWaterLevel {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 管理ファイルの要約
#[derive(Clone, Debug, PartialEq)]
pub struct DeliveryManaged {
    /// 成果品の中の名前
    pub name: String,
    /// 文書型宣言の DTD の名前
    pub doctype: Option<String>,
    /// 根の要素の名前
    pub root: String,
    /// 要素の記録の数
    pub records: u64,
    /// ファイルの名前の形の値の数
    pub links: u64,
    /// 成果品の中に見つからないファイルの名前。書いた順
    pub missing: Vec<String>,
}

impl DeliveryManaged {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let doctype = object.optional("doctype", json::string)?;
        let root = object.required("root", json::string)?;
        let records = object.required("records", |value| json::uint(value, 0, 9007199254740991))?;
        let links = object.required("links", |value| json::uint(value, 0, 9007199254740991))?;
        let missing = object.required("missing", |value| json::array(value, json::string))?;
        Ok(Self {
            name,
            doctype,
            root,
            records,
            links,
            missing,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        if let Some(value) = &self.doctype {
            out.key("doctype");
            out.string(value);
        }
        out.key("root");
        out.string(&self.root);
        out.key("records");
        out.uint(self.records);
        out.key("links");
        out.uint(self.links);
        out.key("missing");
        out.array(&self.missing, |out, item| out.string(item));
    }
}

impl FromJson for DeliveryManaged {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeliveryManaged {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。土木・地理のモジュールに置く（決定 0253）。マイナーバージョン 52 から
#[derive(Clone, Debug, PartialEq)]
pub struct GeoRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// open で開いた CityGML・基盤地図情報（GML）・拡張 DM・GeoJSON・SIMA・GeoTIFF の文書の名前
    pub doc: String,
    /// 中身を返す地物の ID
    pub id: Option<String>,
}

impl GeoRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "geo")?;
        let doc = object.required("doc", json::string)?;
        let id = object.optional("id", json::string)?;
        Ok(Self {
            request_common,
            doc,
            id,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("geo");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
    }
}

impl FromJson for GeoRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for GeoRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地理の領域モデルの要約
#[derive(Clone, Debug, PartialEq)]
pub struct GeoResult {
    /// 平面の座標系の EPSG のコード。分からなければ書かない
    pub crs: Option<u64>,
    /// 高さの座標系の EPSG のコード。分からなければ書かない
    pub height: Option<u64>,
    /// 地物の数
    pub features: u64,
    /// 地物の型ごとの数。型の名前の順
    pub kinds: Vec<GeoKind>,
    /// id で指した地物の中身
    pub feature: Option<GeoFeature>,
}

impl GeoResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let crs = object.optional("crs", |value| json::uint(value, 0, 9007199254740991))?;
        let height = object.optional("height", |value| json::uint(value, 0, 9007199254740991))?;
        let features =
            object.required("features", |value| json::uint(value, 0, 9007199254740991))?;
        let kinds = object.required("kinds", |value| json::array(value, GeoKind::from_json))?;
        let feature = object.optional("feature", GeoFeature::from_json)?;
        Ok(Self {
            crs,
            height,
            features,
            kinds,
            feature,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.crs {
            out.key("crs");
            out.uint(*value);
        }
        if let Some(value) = &self.height {
            out.key("height");
            out.uint(*value);
        }
        out.key("features");
        out.uint(self.features);
        out.key("kinds");
        out.array(&self.kinds, |out, item| item.to_json(out));
        if let Some(value) = &self.feature {
            out.key("feature");
            value.to_json(out);
        }
    }
}

impl FromJson for GeoResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for GeoResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地物の型と数
#[derive(Clone, Debug, PartialEq)]
pub struct GeoKind {
    /// 型。名前空間の接頭辞をつけた元の名前
    pub kind: String,
    /// 地物の数
    pub count: u64,
}

impl GeoKind {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let kind = object.required("kind", json::string)?;
        let count = object.required("count", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self { kind, count })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("kind");
        out.string(&self.kind);
        out.key("count");
        out.uint(self.count);
    }
}

impl FromJson for GeoKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for GeoKind {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 地物の中身
#[derive(Clone, Debug, PartialEq)]
pub struct GeoFeature {
    /// ID
    pub id: String,
    /// 型
    pub kind: String,
    /// 分類コード。元の値の文字のまま
    pub code: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// 親の地物の ID
    pub parent: Option<String>,
    /// 子の地物の数
    pub children: u64,
    /// 属性。入力の順
    pub attributes: Vec<GeoAttribute>,
    /// 形。入力の順
    pub geometries: Vec<GeoGeometry>,
}

impl GeoFeature {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let id = object.required("id", json::string)?;
        let kind = object.required("kind", json::string)?;
        let code = object.optional("code", json::string)?;
        let name = object.optional("name", json::string)?;
        let parent = object.optional("parent", json::string)?;
        let children =
            object.required("children", |value| json::uint(value, 0, 9007199254740991))?;
        let attributes = object.required("attributes", |value| {
            json::array(value, GeoAttribute::from_json)
        })?;
        let geometries = object.required("geometries", |value| {
            json::array(value, GeoGeometry::from_json)
        })?;
        Ok(Self {
            id,
            kind,
            code,
            name,
            parent,
            children,
            attributes,
            geometries,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("id");
        out.string(&self.id);
        out.key("kind");
        out.string(&self.kind);
        if let Some(value) = &self.code {
            out.key("code");
            out.string(value);
        }
        if let Some(value) = &self.name {
            out.key("name");
            out.string(value);
        }
        if let Some(value) = &self.parent {
            out.key("parent");
            out.string(value);
        }
        out.key("children");
        out.uint(self.children);
        out.key("attributes");
        out.array(&self.attributes, |out, item| item.to_json(out));
        out.key("geometries");
        out.array(&self.geometries, |out, item| item.to_json(out));
    }
}

impl FromJson for GeoFeature {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for GeoFeature {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 属性
#[derive(Clone, Debug, PartialEq)]
pub struct GeoAttribute {
    /// 名前。接頭辞をつけた元の名前
    pub name: String,
    /// 値の型
    pub kind: GeoValueKind,
    /// 値の文字
    pub value: String,
    /// 単位つきの量の単位
    pub unit: Option<String>,
    /// コードの一覧（codeSpace）
    pub space: Option<String>,
}

impl GeoAttribute {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let name = object.required("name", json::string)?;
        let kind = object.required("kind", GeoValueKind::from_json)?;
        let value = object.required("value", json::string)?;
        let unit = object.optional("unit", json::string)?;
        let space = object.optional("space", json::string)?;
        Ok(Self {
            name,
            kind,
            value,
            unit,
            space,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("name");
        out.string(&self.name);
        out.key("kind");
        self.kind.to_json(out);
        out.key("value");
        out.string(&self.value);
        if let Some(value) = &self.unit {
            out.key("unit");
            out.string(value);
        }
        if let Some(value) = &self.space {
            out.key("space");
            out.string(value);
        }
    }
}

impl FromJson for GeoAttribute {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for GeoAttribute {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 属性の値の型
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GeoValueKind {
    /// `text`
    Text,
    /// `integer`
    Integer,
    /// `real`
    Real,
    /// `bool`
    Bool,
    /// `date`
    Date,
    /// `code`
    Code,
    /// `measure`
    Measure,
}

impl GeoValueKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Text,
        Self::Integer,
        Self::Real,
        Self::Bool,
        Self::Date,
        Self::Code,
        Self::Measure,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Integer => "integer",
            Self::Real => "real",
            Self::Bool => "bool",
            Self::Date => "date",
            Self::Code => "code",
            Self::Measure => "measure",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "text" => Some(Self::Text),
            "integer" => Some(Self::Integer),
            "real" => Some(Self::Real),
            "bool" => Some(Self::Bool),
            "date" => Some(Self::Date),
            "code" => Some(Self::Code),
            "measure" => Some(Self::Measure),
            _ => None,
        }
    }
}

impl FromJson for GeoValueKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for GeoValueKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 形の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GeoForm {
    /// `point`
    Point,
    /// `line`
    Line,
    /// `area`
    Area,
    /// `solid`
    Solid,
}

impl GeoForm {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[Self::Point, Self::Line, Self::Area, Self::Solid];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Line => "line",
            Self::Area => "area",
            Self::Solid => "solid",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "point" => Some(Self::Point),
            "line" => Some(Self::Line),
            "area" => Some(Self::Area),
            "solid" => Some(Self::Solid),
            _ => None,
        }
    }
}

impl FromJson for GeoForm {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for GeoForm {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 形の要約
#[derive(Clone, Debug, PartialEq)]
pub struct GeoGeometry {
    /// LOD。2D の地物は書かない
    pub lod: Option<u64>,
    /// 形の種類
    pub form: GeoForm,
    /// 点の数。立体と面の集まりは、面の輪の点の数の和
    pub points: u64,
    /// 立体と面の集まりの面の数
    pub faces: Option<u64>,
}

impl GeoGeometry {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let lod = object.optional("lod", |value| json::uint(value, 0, 4))?;
        let form = object.required("form", GeoForm::from_json)?;
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        let faces = object.optional("faces", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            lod,
            form,
            points,
            faces,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.lod {
            out.key("lod");
            out.uint(*value);
        }
        out.key("form");
        self.form.to_json(out);
        out.key("points");
        out.uint(self.points);
        if let Some(value) = &self.faces {
            out.key("faces");
            out.uint(*value);
        }
    }
}

impl FromJson for GeoGeometry {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for GeoGeometry {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// open で開いた J-LandXML・IFC4.3・SIMA の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。マイナーバージョン 51 から
#[derive(Clone, Debug, PartialEq)]
pub struct CivilDrawingRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// open で開いた J-LandXML・IFC4.3・SIMA の文書の名前
    pub doc: String,
    /// doc の線形の ID。文書に線形が 1 つなら書かなくてよい
    pub id: Option<String>,
    /// 書く用紙の名前（結果の sheets の 1 つ）。書かなければ平面図
    pub sheet: Option<String>,
    /// 作る文書の名前
    pub to: String,
    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    pub profiles: Option<Vec<String>>,
    /// 作図の値に使うプロファイル。profiles の中から選ぶ。profiles と一緒に書く
    pub profile: Option<ProfileRef>,
}

impl CivilDrawingRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "civil_drawing")?;
        let doc = object.required("doc", json::string)?;
        let id = object.optional("id", json::string)?;
        let sheet = object.optional("sheet", json::string)?;
        let to = object.required("to", json::string)?;
        let profiles = object.optional("profiles", |value| json::array(value, json::string))?;
        let profile = object.optional("profile", ProfileRef::from_json)?;
        Ok(Self {
            request_common,
            doc,
            id,
            sheet,
            to,
            profiles,
            profile,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("civil_drawing");
        out.key("doc");
        out.string(&self.doc);
        if let Some(value) = &self.id {
            out.key("id");
            out.string(value);
        }
        if let Some(value) = &self.sheet {
            out.key("sheet");
            out.string(value);
        }
        out.key("to");
        out.string(&self.to);
        if let Some(value) = &self.profiles {
            out.key("profiles");
            out.array(value, |out, item| out.string(item));
        }
        if let Some(value) = &self.profile {
            out.key("profile");
            value.to_json(out);
        }
    }
}

impl FromJson for CivilDrawingRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilDrawingRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 作った文書。書いたファイルは blobs の file で返す
#[derive(Clone, Debug, PartialEq)]
pub struct CivilDrawingResult {
    /// 作った文書の名前
    pub doc: String,
    /// 書いた形式の名前。今は dxf
    pub format: String,
    /// 書いたファイルのハッシュ。apply の前提条件に使う
    pub hash: String,
    /// 書いた要素の数
    pub elements: u64,
    /// 図を作った線形の ID
    pub alignment: String,
    /// 作った用紙の名前。図を描ける順（平面図・縦断図・横断図）
    pub sheets: Vec<String>,
    /// 書いた用紙の名前
    pub sheet: String,
    /// 使った作図の値
    pub options: CivilDrawingOptions,
    /// 作図の値を読んだプロファイルの、extends を解いた鎖。基から順。プロファイルを渡さなければ書かない
    pub profile: Option<Vec<ProfileIdentity>>,
}

impl CivilDrawingResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let format = object.required("format", json::string)?;
        let hash = object.required("hash", json::string)?;
        let elements =
            object.required("elements", |value| json::uint(value, 0, 9007199254740991))?;
        let alignment = object.required("alignment", json::string)?;
        let sheets = object.required("sheets", |value| json::array(value, json::string))?;
        let sheet = object.required("sheet", json::string)?;
        let options = object.required("options", CivilDrawingOptions::from_json)?;
        let profile = object.optional("profile", |value| {
            json::array(value, ProfileIdentity::from_json)
        })?;
        Ok(Self {
            doc,
            format,
            hash,
            elements,
            alignment,
            sheets,
            sheet,
            options,
            profile,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("format");
        out.string(&self.format);
        out.key("hash");
        out.string(&self.hash);
        out.key("elements");
        out.uint(self.elements);
        out.key("alignment");
        out.string(&self.alignment);
        out.key("sheets");
        out.array(&self.sheets, |out, item| out.string(item));
        out.key("sheet");
        out.string(&self.sheet);
        out.key("options");
        self.options.to_json(out);
        if let Some(value) = &self.profile {
            out.key("profile");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for CivilDrawingResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilDrawingResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 土木の図の作図の値
#[derive(Clone, Debug, PartialEq)]
pub struct CivilDrawingOptions {
    /// 平面図の尺度の分母
    pub plan: f64,
    /// 縦断図の横の尺度の分母
    pub horizontal: f64,
    /// 縦断図の縦の尺度の分母
    pub vertical: f64,
    /// 横断図の尺度の分母
    pub cross_section: f64,
    /// 文字の高さ（mm）
    pub text_height: f64,
    /// 測点の間隔（メートル）
    pub station_pitch: f64,
}

impl CivilDrawingOptions {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let plan = object.required("plan", json::number)?;
        let horizontal = object.required("horizontal", json::number)?;
        let vertical = object.required("vertical", json::number)?;
        let cross_section = object.required("cross_section", json::number)?;
        let text_height = object.required("text_height", json::number)?;
        let station_pitch = object.required("station_pitch", json::number)?;
        Ok(Self {
            plan,
            horizontal,
            vertical,
            cross_section,
            text_height,
            station_pitch,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("plan");
        out.float(self.plan);
        out.key("horizontal");
        out.float(self.horizontal);
        out.key("vertical");
        out.float(self.vertical);
        out.key("cross_section");
        out.float(self.cross_section);
        out.key("text_height");
        out.float(self.text_height);
        out.key("station_pitch");
        out.float(self.station_pitch);
    }
}

impl FromJson for CivilDrawingOptions {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for CivilDrawingOptions {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。点群のモジュールに置く（決定 0253）。マイナーバージョン 50 から
#[derive(Clone, Debug, PartialEq)]
pub struct DeviationRequest {
    /// `request_common` のフィールド
    pub request_common: RequestCommon,
    /// open で開いた点群の文書の名前
    pub doc: String,
    /// 設計面の TIN
    pub surface: DeviationSurface,
    /// プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す
    pub profiles: Vec<String>,
    /// 規格値に使うプロファイル。profiles の中から選ぶ
    pub profile: ProfileRef,
    /// 工種の名前。規則の work と比べ、同じものだけを使う
    pub work: String,
    /// 返す最初の評価した点の位置。前の応答の next_offset を渡す。書かなければ 0
    pub offset: Option<u64>,
    /// 返す評価した点の数の上限。書かなければ点を返さない
    pub limit: Option<u64>,
    /// 差の色の段で点を塗った 3D のシーンも返す。点の選び方は点群の scene3d と同じ。89 から
    pub scene3d: Option<DeviationScene3d>,
}

impl DeviationRequest {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let request_common = RequestCommon::read_fields(object)?;
        object.constant("op", "deviation")?;
        let doc = object.required("doc", json::string)?;
        let surface = object.required("surface", DeviationSurface::from_json)?;
        let profiles = object.required("profiles", |value| json::array(value, json::string))?;
        let profile = object.required("profile", ProfileRef::from_json)?;
        let work = object.required("work", json::string)?;
        let offset = object.optional("offset", |value| json::uint(value, 0, 9007199254740991))?;
        let limit = object.optional("limit", |value| json::uint(value, 0, 10000))?;
        let scene3d = object.optional("scene3d", DeviationScene3d::from_json)?;
        Ok(Self {
            request_common,
            doc,
            surface,
            profiles,
            profile,
            work,
            offset,
            limit,
            scene3d,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        self.request_common.write_fields(out);
        out.key("op");
        out.string("deviation");
        out.key("doc");
        out.string(&self.doc);
        out.key("surface");
        self.surface.to_json(out);
        out.key("profiles");
        out.array(&self.profiles, |out, item| out.string(item));
        out.key("profile");
        self.profile.to_json(out);
        out.key("work");
        out.string(&self.work);
        if let Some(value) = &self.offset {
            out.key("offset");
            out.uint(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
        if let Some(value) = &self.scene3d {
            out.key("scene3d");
            value.to_json(out);
        }
    }
}

impl FromJson for DeviationRequest {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeviationRequest {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 設計面の TIN を持つ文書と、TIN の ID
#[derive(Clone, Debug, PartialEq)]
pub struct DeviationSurface {
    /// open で開いた土木の文書の名前
    pub doc: String,
    /// TIN の ID。civil の surfaces の id
    pub id: String,
}

impl DeviationSurface {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let doc = object.required("doc", json::string)?;
        let id = object.required("id", json::string)?;
        Ok(Self { doc, id })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("doc");
        out.string(&self.doc);
        out.key("id");
        out.string(&self.id);
    }
}

impl FromJson for DeviationSurface {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeviationSurface {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 標高の差の、規則の区分ごとの集計と指摘。合格・適合とは言わない
#[derive(Clone, Debug, PartialEq)]
pub struct DeviationResult {
    /// 使ったプロファイルの鎖。extends の基から順
    pub chain: Vec<ProfileIdentity>,
    /// 点群の点の数
    pub points: u64,
    /// TIN の外なので評価しなかった点の数
    pub outside: u64,
    /// どの規則の勾配にも入らない三角形の上なので評価しなかった点の数
    pub unmatched: u64,
    /// 区分の境の近く（規則の exclude の内）なので除いた点の数
    pub excluded: u64,
    /// 規則ごとの集計。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す
    pub rules: Vec<DeviationRule>,
    /// 指摘。規則の順。規格値を外れれば profile.surface-elevation.invalid、確認できなければ profile.surface-elevation.unsupported
    pub findings: Vec<CheckFinding>,
    /// 評価した点の数
    pub evaluated: u64,
    /// 評価した点。点群の順に、offset から limit 個まで
    pub differences: Vec<DeviationPoint>,
    /// 続きの最初の評価した点の位置。続きがなければ持たない
    pub next_offset: Option<u64>,
    /// 要求に scene3d を書いたときの 3D のシーン。点の色は bands の段の色で、元の色は持たない。強度と分類は元の値。89 から
    pub scene3d: Option<Scene3dResult>,
    /// 要求に scene3d を書いたときの色の段。規則の順に、その規則で起こりうる段を、差の小さい順に並べ、最後に評価しなかった点の段を置く。89 から
    pub bands: Option<Vec<DeviationBand>>,
}

impl DeviationResult {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let chain = object.required("chain", |value| {
            json::array(value, ProfileIdentity::from_json)
        })?;
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        let outside = object.required("outside", |value| json::uint(value, 0, 9007199254740991))?;
        let unmatched =
            object.required("unmatched", |value| json::uint(value, 0, 9007199254740991))?;
        let excluded =
            object.required("excluded", |value| json::uint(value, 0, 9007199254740991))?;
        let rules = object.required("rules", |value| {
            json::array(value, DeviationRule::from_json)
        })?;
        let findings = object.required("findings", |value| {
            json::array(value, CheckFinding::from_json)
        })?;
        let evaluated =
            object.required("evaluated", |value| json::uint(value, 0, 9007199254740991))?;
        let differences = object.required("differences", |value| {
            json::array(value, DeviationPoint::from_json)
        })?;
        let next_offset = object.optional("next_offset", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        let scene3d = object.optional("scene3d", Scene3dResult::from_json)?;
        let bands = object.optional("bands", |value| {
            json::array(value, DeviationBand::from_json)
        })?;
        Ok(Self {
            chain,
            points,
            outside,
            unmatched,
            excluded,
            rules,
            findings,
            evaluated,
            differences,
            next_offset,
            scene3d,
            bands,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("chain");
        out.array(&self.chain, |out, item| item.to_json(out));
        out.key("points");
        out.uint(self.points);
        out.key("outside");
        out.uint(self.outside);
        out.key("unmatched");
        out.uint(self.unmatched);
        out.key("excluded");
        out.uint(self.excluded);
        out.key("rules");
        out.array(&self.rules, |out, item| item.to_json(out));
        out.key("findings");
        out.array(&self.findings, |out, item| item.to_json(out));
        out.key("evaluated");
        out.uint(self.evaluated);
        out.key("differences");
        out.array(&self.differences, |out, item| item.to_json(out));
        if let Some(value) = &self.next_offset {
            out.key("next_offset");
            out.uint(*value);
        }
        if let Some(value) = &self.scene3d {
            out.key("scene3d");
            value.to_json(out);
        }
        if let Some(value) = &self.bands {
            out.key("bands");
            out.array(value, |out, item| item.to_json(out));
        }
    }
}

impl FromJson for DeviationResult {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeviationResult {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 差の色分けの 3D のシーンの点の選び方。意味は点群の scene3d の box・tolerance・limit と同じ
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeviationScene3d {
    /// 点群の元の座標系の東・北・高さのメートルの箱
    pub r#box: Option<Scene3dBox>,
    /// 点の間隔（メートル）の正の有限値。これより細かい段の節を選ばない
    pub tolerance: Option<f64>,
    /// 返す点の数の上限。省略時は選んだすべての点
    pub limit: Option<u64>,
}

impl DeviationScene3d {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let r#box = object.optional("box", Scene3dBox::from_json)?;
        let tolerance = object.optional("tolerance", json::number)?;
        let limit = object.optional("limit", |value| json::uint(value, 0, 9007199254740991))?;
        Ok(Self {
            r#box,
            tolerance,
            limit,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.r#box {
            out.key("box");
            value.to_json(out);
        }
        if let Some(value) = &self.tolerance {
            out.key("tolerance");
            out.float(*value);
        }
        if let Some(value) = &self.limit {
            out.key("limit");
            out.uint(*value);
        }
    }
}

impl FromJson for DeviationScene3d {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeviationScene3d {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 差の色の段。差は mm で、規則の each（個々の計測値の規格値）から決める。下限より小さい差は below、上限より大きい差は above で、規格値ちょうどは内側に入れる。内側は 0 から規格値の半分までと、半分から規格値までに分け、境の値は 0 に近い段に入れる。0 は high_inner。規格値のない側は inner の 1 段だけにする
#[derive(Clone, Debug, PartialEq)]
pub struct DeviationBand {
    /// 規則の id。評価しなかった点の段は持たない
    pub rule: Option<String>,
    /// 段の名前
    pub band: DeviationBandKind,
    /// 段の下の端の差（mm）。下に端がなければ持たない
    pub lower: Option<f64>,
    /// 段の上の端の差（mm）。上に端がなければ持たない
    pub upper: Option<f64>,
    /// シーンの点の色。16 ビットの赤・緑・青・不透明度
    pub color: Vec<u64>,
}

impl DeviationBand {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let rule = object.optional("rule", json::string)?;
        let band = object.required("band", DeviationBandKind::from_json)?;
        let lower = object.optional("lower", json::number)?;
        let upper = object.optional("upper", json::number)?;
        let color = object.required("color", |value| {
            json::array(value, |value| json::uint(value, 0, 65535))
        })?;
        Ok(Self {
            rule,
            band,
            lower,
            upper,
            color,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        if let Some(value) = &self.rule {
            out.key("rule");
            out.string(value);
        }
        out.key("band");
        self.band.to_json(out);
        if let Some(value) = &self.lower {
            out.key("lower");
            out.float(*value);
        }
        if let Some(value) = &self.upper {
            out.key("upper");
            out.float(*value);
        }
        out.key("color");
        out.array(&self.color, |out, item| out.uint(*item));
    }
}

impl FromJson for DeviationBand {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeviationBand {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 差の色の段。not_evaluated は TIN の外・どの規則の勾配にも入らない・区分の境の近くで除いた点
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeviationBandKind {
    /// `below`
    Below,
    /// `low_outer`
    LowOuter,
    /// `low_inner`
    LowInner,
    /// `high_inner`
    HighInner,
    /// `high_outer`
    HighOuter,
    /// `above`
    Above,
    /// `not_evaluated`
    NotEvaluated,
}

impl DeviationBandKind {
    /// すべての値（スキーマの順）
    pub const ALL: &[Self] = &[
        Self::Below,
        Self::LowOuter,
        Self::LowInner,
        Self::HighInner,
        Self::HighOuter,
        Self::Above,
        Self::NotEvaluated,
    ];

    /// JSON での名前
    pub const fn name(self) -> &'static str {
        match self {
            Self::Below => "below",
            Self::LowOuter => "low_outer",
            Self::LowInner => "low_inner",
            Self::HighInner => "high_inner",
            Self::HighOuter => "high_outer",
            Self::Above => "above",
            Self::NotEvaluated => "not_evaluated",
        }
    }

    /// JSON での名前から引く
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "below" => Some(Self::Below),
            "low_outer" => Some(Self::LowOuter),
            "low_inner" => Some(Self::LowInner),
            "high_inner" => Some(Self::HighInner),
            "high_outer" => Some(Self::HighOuter),
            "above" => Some(Self::Above),
            "not_evaluated" => Some(Self::NotEvaluated),
            _ => None,
        }
    }
}

impl FromJson for DeviationBandKind {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        Self::from_name(json::str(value)?).ok_or_else(|| Invalid::new(Problem::Value))
    }
}

impl ToJson for DeviationBandKind {
    fn to_json(&self, out: &mut Writer) {
        out.string(self.name());
    }
}

/// 規則の区分ごとの集計。差はメートル
#[derive(Clone, Debug, PartialEq)]
pub struct DeviationRule {
    /// 規則の id
    pub rule: String,
    /// 区分の名前
    pub part: String,
    /// 評価した点の数
    pub points: u64,
    /// 区分の三角形の、平面に投影した面積（m²）
    pub area: f64,
    /// 計測の密度（点／m²）。面積がなければ書かない
    pub density: Option<f64>,
    /// 差の平均。点がなければ書かない
    pub mean: Option<f64>,
    /// 差の最小。点がなければ書かない
    pub min: Option<f64>,
    /// 差の最大。点がなければ書かない
    pub max: Option<f64>,
    /// 指摘の数（確認できなかったものを除く）
    pub findings: u64,
    /// 確認できなかった数
    pub unsupported: u64,
}

impl DeviationRule {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let rule = object.required("rule", json::string)?;
        let part = object.required("part", json::string)?;
        let points = object.required("points", |value| json::uint(value, 0, 9007199254740991))?;
        let area = object.required("area", json::number)?;
        let density = object.optional("density", json::number)?;
        let mean = object.optional("mean", json::number)?;
        let min = object.optional("min", json::number)?;
        let max = object.optional("max", json::number)?;
        let findings =
            object.required("findings", |value| json::uint(value, 0, 9007199254740991))?;
        let unsupported = object.required("unsupported", |value| {
            json::uint(value, 0, 9007199254740991)
        })?;
        Ok(Self {
            rule,
            part,
            points,
            area,
            density,
            mean,
            min,
            max,
            findings,
            unsupported,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("rule");
        out.string(&self.rule);
        out.key("part");
        out.string(&self.part);
        out.key("points");
        out.uint(self.points);
        out.key("area");
        out.float(self.area);
        if let Some(value) = &self.density {
            out.key("density");
            out.float(*value);
        }
        if let Some(value) = &self.mean {
            out.key("mean");
            out.float(*value);
        }
        if let Some(value) = &self.min {
            out.key("min");
            out.float(*value);
        }
        if let Some(value) = &self.max {
            out.key("max");
            out.float(*value);
        }
        out.key("findings");
        out.uint(self.findings);
        out.key("unsupported");
        out.uint(self.unsupported);
    }
}

impl FromJson for DeviationRule {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeviationRule {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 評価した点
#[derive(Clone, Debug, PartialEq)]
pub struct DeviationPoint {
    /// 点群の中の番号
    pub index: u64,
    /// 北・東・標高（メートル）
    pub position: Vec<f64>,
    /// 点の標高から設計面の標高を引いた差（メートル）
    pub difference: f64,
    /// 評価した規則の id
    pub rule: String,
}

impl DeviationPoint {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let index = object.required("index", |value| json::uint(value, 0, 9007199254740991))?;
        let position = object.required("position", |value| json::array(value, json::number))?;
        let difference = object.required("difference", json::number)?;
        let rule = object.required("rule", json::string)?;
        Ok(Self {
            index,
            position,
            difference,
            rule,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("index");
        out.uint(self.index);
        out.key("position");
        out.array(&self.position, |out, item| out.float(*item));
        out.key("difference");
        out.float(self.difference);
        out.key("rule");
        out.string(&self.rule);
    }
}

impl FromJson for DeviationPoint {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for DeviationPoint {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// describe に使うカタログのファイル。catalog/<言語>.toml から生成し、zm_push で渡す（決定 0009）
#[derive(Clone, Debug, PartialEq)]
pub struct Catalog {
    /// カタログの形のメジャーバージョン。今は 1
    pub v: u64,
    /// 言語（ja・en）
    pub lang: String,
    /// パッケージのバージョン。コアと同じバージョンで出す
    pub package: String,
    /// 数の書き方
    pub number: NumberFormat,
    /// code ごとの文のひな形。{名前} に値が、{count} にまとめた件数が入る
    pub codes: BTreeMap<String, String>,
}

impl Catalog {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let v = object.required("v", |value| json::uint(value, 1, 9007199254740991))?;
        let lang = object.required("lang", json::string)?;
        let package = object.required("package", json::string)?;
        let number = object.required("number", NumberFormat::from_json)?;
        let codes = object.required("codes", |value| json::map(value, json::string))?;
        Ok(Self {
            v,
            lang,
            package,
            number,
            codes,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("v");
        out.uint(self.v);
        out.key("lang");
        out.string(&self.lang);
        out.key("package");
        out.string(&self.package);
        out.key("number");
        self.number.to_json(out);
        out.key("codes");
        out.map(&self.codes, |out, item| out.string(item));
    }
}

impl FromJson for Catalog {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for Catalog {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}

/// 文に差し込む数の書き方。OS のロケールは使わない（決定 0009）
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    /// 整数部を 3 桁ごとに区切る文字。空なら区切らない
    pub group_separator: String,
    /// 小数点の文字
    pub decimal_separator: String,
}

impl NumberFormat {
    fn read_fields(object: &mut ObjectReader<'_>) -> Result<Self, Invalid> {
        let group_separator = object.required("group_separator", json::string)?;
        let decimal_separator = object.required("decimal_separator", json::string)?;
        Ok(Self {
            group_separator,
            decimal_separator,
        })
    }

    fn write_fields(&self, out: &mut Writer) {
        out.key("group_separator");
        out.string(&self.group_separator);
        out.key("decimal_separator");
        out.string(&self.decimal_separator);
    }
}

impl FromJson for NumberFormat {
    fn from_json(value: &Value) -> Result<Self, Invalid> {
        let mut object = ObjectReader::new(value)?;
        let this = Self::read_fields(&mut object)?;
        object.finish()?;
        Ok(this)
    }
}

impl ToJson for NumberFormat {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        self.write_fields(out);
        out.end_object();
    }
}
