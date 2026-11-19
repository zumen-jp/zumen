//! DXF の GEODATA（AutoCAD の地理位置）から、図面から地図への変換を作る（#685）。
//!
//! GEODATA の `AcDbGeoData` の値（グループ・コードと値の並び。DXF Reference「GEODATA」）を [`GeoData`] に読み、
//! 座標系の定義（XML）から地図の座標系（[`Crs`]）を引いて、図面の点を地図へ写す [`MapConversion`] にする。
//! 読むのは DXF の形に依らない値の並びで、DXF の読みは呼ぶ側（`zumen-acad` の表のオブジェクトなど）が渡す。
//!
//! 変換は ezdxf 1.4.5（MIT）の `GeoData.get_crs_transformation` と同じ式にする。図面の点 `p` は、設計の点 `d`
//! を引き、水平の単位の倍率 `s` を掛け、北の向き `(nx, ny)` を地図の北へ回し、基準の点 `r` を足して地図へ写す。
//! 回す角は `π/2 − atan2(ny, nx)`（反時計回り）なので、図面の X 軸は地図の `(ny, nx)` の向きになる。
//! 高さは同じ式で `r.z + s·(p.z − d.z)` とする（推定。ezdxf は高さを写さない）。
//!
//! ezdxf が確かめた設定（座標の種類が局所の格子・尺度の推定なし・海面補正なし・上の向きが Z・メッシュの面なし・
//! 最初の軸が東か西）のほかは、変換を作らずに診断を返す。

use zumen_base::{Cx, Diagnostic, Kind, Ref, Value, codes};
use zumen_xml::{Document, Node};

use crate::{Crs, MapConversion};

/// GEODATA の値の 1 つ。DXF のグループ・コードの型（DXF Reference「Group Code Value Types」）で読んだもの
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeoValue<'a> {
    /// 整数
    Int(i64),
    /// 実数
    Real(f64),
    /// 真偽（294 の海面補正）
    Bool(bool),
    /// 文字列。符号化のままのバイト列
    Text(&'a [u8]),
    /// ハンドルなど、使わない型の値
    Other,
}

/// GEODATA の `AcDbGeoData` の値。書かれていない値は、ezdxf 1.4.5 の既定の値にする（推定）
#[derive(Clone, Debug, PartialEq)]
pub struct GeoData {
    /// 版（90）。1 は R2009、2 は R2010 から
    pub version: i64,
    /// 座標の種類（70）。0 は不明、1 は局所の格子、2 は投影の格子、3 は経緯度
    pub coordinate_type: i64,
    /// 設計の点（10・20・30）。図面の座標
    pub design_point: [f64; 3],
    /// 基準の点（11・21・31）。座標系の座標
    pub reference_point: [f64; 3],
    /// 水平の単位の倍率（40）。図面の座標に掛けると m になる
    pub horizontal_unit_scale: f64,
    /// 垂直の単位の倍率（41）
    pub vertical_unit_scale: f64,
    /// 上の向き（210・220・230）
    pub up_direction: [f64; 3],
    /// 図面の上の北の向き（12・22）。版 1 では読まない
    pub north_direction: [f64; 2],
    /// 尺度の推定の方法（95）。1 はなし、2 は利用者の倍率、3 は基準の点の格子の倍率、4 は角柱
    pub scale_estimation_method: i64,
    /// 利用者の倍率（141）
    pub user_scale_factor: f64,
    /// 海面補正をするか（294）
    pub sea_level_correction: bool,
    /// 海面補正の標高（142）
    pub sea_level_elevation: f64,
    /// 座標系の定義（303 の続きと 301）。`^J` などの書き方は戻してある
    pub definition: Vec<u8>,
    /// メッシュの元の点の数（版 2 は 13、版 1 は 12 の数）
    pub mesh_points: u64,
    /// メッシュの面の数（97 の数）
    pub mesh_faces: u64,
}

/// GEODATA から作った、図面から地図への変換
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Georeference {
    /// 地図の座標系
    pub crs: Crs,
    /// 図面（プロジェクトの空間）から地図への変換
    pub conversion: MapConversion,
}

impl Default for GeoData {
    fn default() -> Self {
        Self {
            version: 2,
            coordinate_type: 3,
            design_point: [0.0; 3],
            reference_point: [0.0; 3],
            horizontal_unit_scale: 1.0,
            vertical_unit_scale: 1.0,
            up_direction: [0.0, 0.0, 1.0],
            north_direction: [0.0, 1.0],
            scale_estimation_method: 1,
            user_scale_factor: 1.0,
            sea_level_correction: false,
            sea_level_elevation: 0.0,
            definition: Vec::new(),
            mesh_points: 0,
            mesh_faces: 0,
        }
    }
}

/// 読んだ値の型の誤り。グループ・コードを持つ
struct Wrong(i16);

impl GeoData {
    /// GEODATA のオブジェクトの値の並び `values`（グループ・コードと値。書かれた順）を読む。
    ///
    /// サブクラスの印（100 の `AcDbGeoData`）があれば、その後の値だけを読む。同じグループ・コードが何度もあれば、
    /// 最初の値を使う。使う値が数でないか有限でないか、倍率か北の向きが 0 なら `crs.geodata.invalid`（`code` は
    /// グループ・コード）を `at` の場所で診断して `None` を返す。値の数だけ予算の作業量（`work`）を数え、止まれば
    /// 理由を診断して `None` を返す
    pub fn read<'a>(
        values: impl IntoIterator<Item = (i16, GeoValue<'a>)>,
        at: &Ref,
        cx: &mut Cx,
    ) -> Option<GeoData> {
        let mut tags = Vec::new();
        for (code, value) in values {
            if let Err(stop) = cx.check(Kind::Work, 1) {
                cx.report(stop.to_diagnostic().at(at.clone()));
                return None;
            }
            if code == 100
                && matches!(value, GeoValue::Text(name) if name.trim_ascii() == b"AcDbGeoData")
            {
                tags.clear();
                continue;
            }
            tags.push((code, value));
        }
        match Self::from_tags(&tags) {
            Ok(data) => Some(data),
            Err(Wrong(code)) => {
                cx.report(
                    Diagnostic::new(codes::CRS_GEODATA_INVALID)
                        .with("code", u64::from(code.unsigned_abs()))
                        .at(at.clone()),
                );
                None
            }
        }
    }

    fn from_tags(tags: &[(i16, GeoValue)]) -> Result<GeoData, Wrong> {
        let first = |code: i16| tags.iter().find(|(c, _)| *c == code).map(|(_, v)| *v);
        let int = |code: i16, default: i64| match first(code) {
            None => Ok(default),
            Some(GeoValue::Int(value)) => Ok(value),
            Some(_) => Err(Wrong(code)),
        };
        let real = |code: i16, default: f64| match first(code) {
            None => Ok(default),
            Some(GeoValue::Real(value)) if value.is_finite() => Ok(value),
            Some(_) => Err(Wrong(code)),
        };
        let mut data = GeoData {
            version: int(90, 2)?,
            ..GeoData::default()
        };
        data.coordinate_type = int(70, data.coordinate_type)?;
        for (index, axis) in data.design_point.iter_mut().enumerate() {
            *axis = real(10 + 10 * index as i16, 0.0)?;
        }
        for (index, axis) in data.reference_point.iter_mut().enumerate() {
            *axis = real(11 + 10 * index as i16, 0.0)?;
        }
        data.horizontal_unit_scale = real(40, 1.0)?;
        if data.horizontal_unit_scale == 0.0 {
            return Err(Wrong(40));
        }
        data.vertical_unit_scale = real(41, 1.0)?;
        if data.vertical_unit_scale == 0.0 {
            return Err(Wrong(41));
        }
        data.up_direction = [real(210, 0.0)?, real(220, 0.0)?, real(230, 1.0)?];
        // 版 1 の 12・22 は、メッシュの元の点（DXF Reference R2009）
        let source = if data.version < 2 { 12 } else { 13 };
        if data.version >= 2 {
            data.north_direction = [real(12, 0.0)?, real(22, 1.0)?];
            if data.north_direction == [0.0, 0.0] {
                return Err(Wrong(12));
            }
        }
        data.scale_estimation_method = int(95, 1)?;
        data.user_scale_factor = real(141, 1.0)?;
        data.sea_level_correction = match first(294) {
            None => false,
            Some(GeoValue::Bool(value)) => value,
            Some(GeoValue::Int(value)) => value != 0,
            Some(_) => return Err(Wrong(294)),
        };
        data.sea_level_elevation = real(142, 0.0)?;
        for (code, value) in tags {
            match (*code, value) {
                (301 | 303, GeoValue::Text(text)) => data.definition.extend(uncaret(text)),
                (301 | 303, _) => return Err(Wrong(*code)),
                (code, _) if code == source => data.mesh_points += 1,
                (97, _) => data.mesh_faces += 1,
                _ => {}
            }
        }
        Ok(data)
    }

    /// 図面から地図への変換と、地図の座標系を作る。
    ///
    /// ezdxf 1.4.5 が確かめた設定のほかは `crs.geodata.unsupported`（`reason` は `version`・`coordinate-type`・
    /// `scale-estimation`・`sea-level`・`up-direction`・`mesh`・`vertical-scale`・`axis-order`）を、座標系の定義から
    /// 扱う EPSG のコードを引けなければ `crs.geodata-crs.unsupported`（`name` は見つけた EPSG のコードか座標系の
    /// 名前。なければ空）を、`at` の場所で診断して `None` を返す。版 1 はグループ・コードの意味が違うので、`version`
    /// だけを診断する
    pub fn georeference(&self, at: &Ref, cx: &mut Cx) -> Option<Georeference> {
        let unsupported = |reason: &'static str, cx: &mut Cx| {
            cx.report(
                Diagnostic::new(codes::CRS_GEODATA_UNSUPPORTED)
                    .with("reason", Value::Key(reason))
                    .at(at.clone()),
            );
        };
        if self.version < 2 {
            unsupported("version", cx);
            return None;
        }
        let mut reasons = Vec::new();
        if self.coordinate_type != 1 {
            reasons.push("coordinate-type");
        }
        if self.scale_estimation_method != 1 || self.user_scale_factor != 1.0 {
            reasons.push("scale-estimation");
        }
        if self.sea_level_correction || self.sea_level_elevation != 0.0 {
            reasons.push("sea-level");
        }
        if self.up_direction != [0.0, 0.0, 1.0] {
            reasons.push("up-direction");
        }
        if self.mesh_faces > 0 {
            reasons.push("mesh");
        }
        if self.vertical_unit_scale != self.horizontal_unit_scale {
            reasons.push("vertical-scale");
        }
        let crs = match definition_crs(&self.definition, cx) {
            Ok(crs) => Some(crs),
            Err(Unknown::AxisOrder) => {
                reasons.push("axis-order");
                None
            }
            Err(Unknown::Crs(name)) => {
                cx.report(
                    Diagnostic::new(codes::CRS_GEODATA_CRS_UNSUPPORTED)
                        .with("name", name)
                        .at(at.clone()),
                );
                None
            }
        };
        for reason in &reasons {
            unsupported(reason, cx);
        }
        let crs = crs.filter(|_| reasons.is_empty())?;
        let conversion = self.conversion();
        let valid = [
            conversion.eastings,
            conversion.northings,
            conversion.orthogonal_height,
            conversion.x_axis_abscissa,
            conversion.x_axis_ordinate,
            conversion.scale,
        ]
        .iter()
        .all(|v| v.is_finite());
        if !valid {
            cx.report(
                Diagnostic::new(codes::CRS_GEODATA_INVALID)
                    .with("code", 10u64)
                    .at(at.clone()),
            );
            return None;
        }
        Some(Georeference { crs, conversion })
    }

    /// 設定を確かめずに、変換を作る
    fn conversion(&self) -> MapConversion {
        let [nx, ny] = self.north_direction;
        let length = libm::sqrt(nx * nx + ny * ny);
        // 図面の X 軸の地図の上の向き（東・北）
        let (cos, sin) = (ny / length, nx / length);
        let s = self.horizontal_unit_scale;
        let [dx, dy, dz] = self.design_point.map(|v| v * s);
        let [rx, ry, rz] = self.reference_point;
        MapConversion {
            eastings: rx - (dx * cos - dy * sin),
            northings: ry - (dx * sin + dy * cos),
            orthogonal_height: rz - dz,
            x_axis_abscissa: cos,
            x_axis_ordinate: sin,
            scale: s,
        }
    }
}

/// 座標系を引けない理由
enum Unknown {
    /// 扱わない座標系。見つけた EPSG のコードか名前
    Crs(String),
    /// 最初の軸が北か南か、決まらない
    AxisOrder,
}

/// 座標系の定義（Autodesk の座標系の XML）から、EPSG のコードの別名（`Alias` の `type="CoordinateSystem"` で、
/// `Namespace` が `EPSG Code`）と最初の軸（`AxisOrder` が 1 の `AxisAbbreviation`）を読む。名前空間は見ない
/// （ezdxf 1.4.5 の `GeoData.get_crs` と同じ）
fn definition_crs(definition: &[u8], cx: &mut Cx) -> Result<Crs, Unknown> {
    // 宣言の文字コード（UTF-16 と書くことがある）は、DXF の文字列の符号化と合わないので、宣言を除いて読む
    let start = definition
        .windows(2)
        .position(|pair| pair == b"?>")
        .filter(|_| definition.trim_ascii_start().starts_with(b"<?xml"))
        .map_or(0, |at| at + 2);
    let Ok(doc) = zumen_xml::read(definition.get(start..).unwrap_or_default(), cx) else {
        return Err(Unknown::Crs(String::new()));
    };
    let named = |index: usize, local: &[u8]| {
        doc.element(index)
            .is_some_and(|element| doc.parsed_local_name(element.name()) == local)
    };
    let child_text = |index: usize, local: &[u8]| {
        doc.element(index).and_then(|element| {
            element.children().iter().find_map(|node| match node {
                Node::Element(child) if named(*child, local) => Some(text(&doc, *child)),
                _ => None,
            })
        })
    };
    let mut epsg = None;
    let mut name = String::new();
    for index in 0..doc.elements().len() {
        let Some(element) = doc.element(index) else {
            continue;
        };
        let local = doc.parsed_local_name(element.name());
        if matches!(
            local,
            b"ProjectedCoordinateSystem" | b"GeographicCoordinateSystem"
        ) && name.is_empty()
            && let Some(id) = doc.attribute(element, None, b"id")
        {
            name = doc.value(id);
        }
        let kind = doc.attribute(element, None, b"type").map(|a| doc.value(a));
        if local == b"Alias"
            && kind.as_deref() == Some("CoordinateSystem")
            && child_text(index, b"Namespace").as_deref() == Some("EPSG Code")
            && epsg.is_none()
        {
            let id = doc
                .attribute(element, None, b"id")
                .map(|a| doc.value(a))
                .unwrap_or_default();
            epsg = Some(id);
        }
    }
    let Some(id) = epsg else {
        return Err(Unknown::Crs(name));
    };
    let Some(crs) = id.trim().parse().ok().and_then(Crs::from_epsg) else {
        return Err(Unknown::Crs(format!("EPSG:{}", id.trim())));
    };
    let first_axis = (0..doc.elements().len())
        .filter(|index| named(*index, b"CoordinateSystemAxis"))
        .find(|index| child_text(*index, b"AxisOrder").as_deref() == Some("1"))
        .and_then(|index| child_text(index, b"AxisAbbreviation"));
    match first_axis.as_deref() {
        Some("E" | "W") => Ok(crs),
        _ => Err(Unknown::AxisOrder),
    }
}

/// 要素の中の文字を、前後の空白を除いてつなぐ
fn text(doc: &Document, index: usize) -> String {
    let Some(element) = doc.element(index) else {
        return String::new();
    };
    let mut out = String::new();
    for node in element.children() {
        match node {
            Node::Text(range) => out.push_str(&doc.text(range.clone())),
            Node::CData(range) => out.push_str(&doc.cdata(range.clone())),
            Node::Element(_) => {}
        }
    }
    out.trim().to_owned()
}

/// DXF の文字列の書き方（`^J` は改行、`^ ` は `^`）を戻す。ほかの `^` の並びは、そのまま残す
fn uncaret(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.iter().peekable();
    while let Some(&byte) = bytes.next() {
        match (byte, bytes.peek()) {
            (b'^', Some(b' ')) => {
                out.push(b'^');
                bytes.next();
            }
            (b'^', Some(&&next @ 0x40..0x60)) => {
                out.push(next - 0x40);
                bytes.next();
            }
            _ => out.push(byte),
        }
    }
    out
}
