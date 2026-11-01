//! 地質・土質調査の成果（#390、決定 0242）。ボーリング交換用データ（BED、地質・土質調査成果電子納品要領の
//! BED0500.DTD）を読み、ボーリングの位置・標高と、深さごとの地層・色調・観察記事・標準貫入試験・孔内水位を返す。
//!
//! - XML は `zumen-xml` で読む（L0）。DTD は処理せず、要素は名前で見分ける。DTD は同梱しない。
//! - 位置は、経度・緯度の度・分・秒を `zumen-crs` の地理の点（度）にする。測地系は換算せず、コードの値を返す。
//! - 孔口標高は東京湾平均海面（T.P.）からの標高とする。成果は書かれていないので、測地系が JGD2024 なら測地成果2024、
//!   そうでなければ調査の終わりの日（なければ始まりの日）から仮定し、`crs.height-realization.assumed` を返す。
//! - 読まない要素（ほかの試験・判定表・削孔の記録・調査会社など）は、名前ごとに `boring.element.unsupported` を返す。
//!   調査会社の技術者の名前は返さない。

use zumen_base::{Cx, Diagnostic, Kind, Ref, Stop, codes};
use zumen_crs::{GeoPoint, Height, HeightDatum, Realization};
use zumen_sdk::Error;
use zumen_xml::{Document, Element, Node};

/// 根の要素の名前
const ROOT: &str = "ボーリング情報";

/// 測地成果2024 が使われ始めた日
const SURVEY_2024: &str = "2025-04-01";

/// ボーリング 1 本の成果
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Boring {
    /// 適用規格（JIS の番号など）。文書の順
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
    pub count: Option<u32>,
    /// ボーリング連番
    pub serial: Option<u32>,
    /// 孔口の位置（経度・緯度）
    pub position: Option<GeoPoint>,
    /// 位置の測地系
    pub datum: Option<Datum>,
    /// 調査位置の住所
    pub address: Option<String>,
    /// 調査期間の始まりの日（`YYYY-MM-DD`）
    pub start: Option<String>,
    /// 調査期間の終わりの日（`YYYY-MM-DD`）
    pub end: Option<String>,
    /// 孔口標高
    pub elevation: Option<Height>,
    /// 総削孔長（m）
    pub length: Option<f64>,
    /// 削孔の角度（度）
    pub angle: Option<f64>,
    /// 削孔の方位（度）
    pub direction: Option<f64>,
    /// 工学的地質区分名・現場土質名。文書の順
    pub layers: Vec<Layer>,
    /// 色調。文書の順
    pub colors: Vec<Color>,
    /// 観察記事。文書の順
    pub observations: Vec<Observation>,
    /// 標準貫入試験。文書の順
    pub penetrations: Vec<Penetration>,
    /// 孔内水位。文書の順
    pub water_levels: Vec<WaterLevel>,
}

/// 位置の測地系
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Datum {
    /// 日本測地系（旧測地系）。コード 00
    Tokyo,
    /// 世界測地系（JGD2000）。コード 01
    Jgd2000,
    /// 世界測地系（JGD2011）。コード 02
    Jgd2011,
    /// 世界測地系（JGD2024）。コード 03
    Jgd2024,
}

impl Datum {
    fn from_code(code: &str) -> Option<Datum> {
        match code {
            "00" => Some(Datum::Tokyo),
            "01" => Some(Datum::Jgd2000),
            "02" => Some(Datum::Jgd2011),
            "03" => Some(Datum::Jgd2024),
            _ => None,
        }
    }
}

/// 地層の区間（工学的地質区分名・現場土質名）。上端は 1 つ上の区間の下端（最初は 0）
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// 下端の深さ（GL.-m）
    pub bottom: f64,
    /// 名前
    pub name: String,
    /// 記号
    pub symbol: Option<String>,
}

/// 色調の区間
#[derive(Clone, Debug, PartialEq)]
pub struct Color {
    /// 下端の深さ（GL.-m）
    pub bottom: f64,
    /// 色調の名前
    pub name: Option<String>,
}

/// 観察記事
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    /// 上端の深さ（GL.-m）
    pub top: f64,
    /// 下端の深さ（GL.-m）
    pub bottom: f64,
    /// 記事
    pub text: Option<String>,
}

/// 標準貫入試験
#[derive(Clone, Debug, PartialEq)]
pub struct Penetration {
    /// 開始の深さ（GL.-m）
    pub depth: f64,
    /// 合計の打撃回数
    pub blows: u32,
    /// 合計の貫入量（mm）
    pub penetration: f64,
}

/// 孔内水位
#[derive(Clone, Debug, PartialEq)]
pub struct WaterLevel {
    /// 測定の日（`YYYY-MM-DD`）
    pub date: Option<String>,
    /// 削孔状況のコード
    pub state: Option<String>,
    /// 水位（GL.-m）。地表より高ければ負。水位がなければ `None`
    pub depth: Option<f64>,
    /// 水位種別・備考
    pub note: Option<String>,
}

fn stopped(cx: &mut Cx, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

/// ボーリング交換用データを読む。XML でないか、根が `ボーリング情報` でなければ `boring.file.invalid` を返して
/// 失敗する。数として読めない値は `boring.value.invalid` を返して飛ばす
pub fn read(src: &[u8], cx: &mut Cx) -> Result<Boring, Error> {
    cx.check(Kind::InputBytes, src.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let mut quiet = zumen_base::Diagnostics::new();
    let read = {
        let mut inner = Cx::new(cx.budget, &mut quiet, cx.cancel);
        zumen_xml::read(src, &mut inner)
    };
    // DTD は処理しない決まりなので、落とした診断は返さない
    for diagnostic in quiet.iter() {
        if diagnostic.code() != codes::XML_DTD_DROPPED {
            cx.report(diagnostic.clone());
        }
    }
    let doc = match read {
        Ok(doc) => doc,
        Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
        Err(_) => {
            cx.report(Diagnostic::new(codes::BORING_FILE_INVALID));
            return Err(Error::Invalid);
        }
    };
    let Some(root) = doc
        .element(doc.root())
        .filter(|root| doc.local_name(root.name()) == ROOT.as_bytes())
    else {
        cx.report(Diagnostic::new(codes::BORING_FILE_INVALID));
        return Err(Error::Invalid);
    };
    let mut reader = Reader {
        doc: &doc,
        boring: Boring::default(),
        unsupported: Vec::new(),
    };
    for (name, element) in reader.children(root) {
        cx.check(Kind::Records, 1)
            .map_err(|stop| stopped(cx, stop))?;
        match name.as_str() {
            "基礎情報" => reader.basic(element),
            "標題情報" => reader.title(element, cx),
            "コア情報" => reader.core(element, cx).map_err(|stop| stopped(cx, stop))?,
            _ => reader.unsupported(name),
        }
    }
    let Reader {
        mut boring,
        unsupported,
        ..
    } = reader;
    boring.elevation = boring.elevation.map(|height| {
        let written = (boring.datum == Some(Datum::Jgd2024)).then_some(Realization::Survey2024);
        let date = boring.end.as_deref().or(boring.start.as_deref());
        let assumed = match date {
            Some(date) if date >= SURVEY_2024 => Realization::Survey2024,
            _ => Realization::Survey2011,
        };
        let at = Ref::at_record(0);
        Height {
            realization: Realization::or_assume(written, assumed, at, cx),
            ..height
        }
    });
    for name in unsupported {
        cx.report(Diagnostic::new(codes::BORING_ELEMENT_UNSUPPORTED).with("name", name.as_str()));
    }
    Ok(boring)
}

struct Reader<'d> {
    doc: &'d Document,
    boring: Boring,
    /// 読まなかった要素の名前。最初に出た順
    unsupported: Vec<String>,
}

impl<'d> Reader<'d> {
    /// 子の要素と、その名前
    fn children(&self, element: &'d Element) -> Vec<(String, &'d Element)> {
        let doc = self.doc;
        element
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::Element(index) => doc.element(*index),
                _ => None,
            })
            .map(|child| {
                let name = String::from_utf8_lossy(doc.local_name(child.name())).into_owned();
                (name, child)
            })
            .collect()
    }

    /// 子の要素の文字。前後の空白を除き、空なら `None`
    fn text(&self, element: &Element, name: &str) -> Option<String> {
        self.children(element)
            .into_iter()
            .find(|(child, _)| child == name)
            .and_then(|(_, child)| self.own_text(child))
    }

    fn own_text(&self, element: &Element) -> Option<String> {
        let mut out = String::new();
        for node in element.children() {
            match node {
                Node::Text(range) => out.push_str(&self.doc.text(range.clone())),
                Node::CData(range) => out.push_str(&self.doc.cdata(range.clone())),
                Node::Element(_) => {}
            }
        }
        let trimmed = out.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    }

    /// 子の要素の数。読めなければ `boring.value.invalid` を返して `None`
    fn number(&self, element: &Element, name: &str, cx: &mut Cx) -> Option<f64> {
        let text = self.text(element, name)?;
        match text.parse::<f64>() {
            Ok(value) if value.is_finite() => Some(value),
            _ => {
                invalid(cx, name, &text);
                None
            }
        }
    }

    fn integer(&self, element: &Element, name: &str, cx: &mut Cx) -> Option<u32> {
        let text = self.text(element, name)?;
        match text.parse::<u32>() {
            Ok(value) => Some(value),
            Err(_) => {
                invalid(cx, name, &text);
                None
            }
        }
    }

    fn unsupported(&mut self, name: String) {
        if !self.unsupported.contains(&name) {
            self.unsupported.push(name);
        }
    }

    fn basic(&mut self, element: &'d Element) {
        for (name, child) in self.children(element) {
            match name.as_str() {
                "適用規格" => {
                    if let Some(text) = self.own_text(child) {
                        self.boring.standards.push(text);
                    }
                }
                "公開フラグ" => {
                    self.boring.publication = self.text(child, "公開フラグ_コード")
                }
                _ => self.unsupported(name),
            }
        }
    }

    fn title(&mut self, element: &'d Element, cx: &mut Cx) {
        for (name, child) in self.children(element) {
            match name.as_str() {
                "調査基本情報" => {
                    self.boring.project = self.text(child, "事業工事名");
                    self.boring.survey = self.text(child, "調査名");
                    self.boring.name = self.text(child, "ボーリング名");
                    self.boring.count = self.integer(child, "ボーリング総数", cx);
                    self.boring.serial = self.integer(child, "ボーリング連番", cx);
                }
                "経度緯度情報" => self.position(child, cx),
                "調査位置" => self.boring.address = self.text(child, "調査位置住所"),
                "調査期間" => {
                    self.boring.start = self.text(child, "調査期間_開始年月日");
                    self.boring.end = self.text(child, "調査期間_終了年月日");
                }
                "ボーリング基本情報" => {
                    self.boring.elevation =
                        self.number(child, "孔口標高", cx).map(|value| Height {
                            value,
                            datum: HeightDatum::TokyoBay,
                            // 読み終えてから決める
                            realization: Realization::Survey2011,
                        });
                    self.boring.length = self.number(child, "総削孔長", cx);
                    self.boring.angle = self.number(child, "角度", cx);
                    self.boring.direction = self.number(child, "方位", cx);
                }
                _ => self.unsupported(name),
            }
        }
    }

    fn position(&mut self, element: &Element, cx: &mut Cx) {
        let mut angle = |prefix: &str| -> Option<f64> {
            let degrees = self.number(element, &format!("{prefix}_度"), cx)?;
            let minutes = self.number(element, &format!("{prefix}_分"), cx)?;
            let seconds = self.number(element, &format!("{prefix}_秒"), cx)?;
            Some(degrees + minutes / 60.0 + seconds / 3600.0)
        };
        let longitude = angle("経度");
        let latitude = angle("緯度");
        if let (Some(latitude), Some(longitude)) = (latitude, longitude) {
            self.boring.position = Some(GeoPoint {
                latitude,
                longitude,
            });
        }
        if let Some(code) = self.text(element, "測地系") {
            self.boring.datum = Datum::from_code(&code);
            if self.boring.datum.is_none() {
                invalid(cx, "測地系", &code);
            }
        }
    }

    fn core(&mut self, element: &'d Element, cx: &mut Cx) -> Result<(), Stop> {
        const LAYER: &str = "工学的地質区分名現場土質名";
        for (name, child) in self.children(element) {
            cx.check(Kind::Records, 1)?;
            match name.as_str() {
                LAYER => {
                    let bottom = self.number(child, &format!("{LAYER}_下端深度"), cx);
                    let text = self.text(child, &format!("{LAYER}_{LAYER}"));
                    if let (Some(bottom), Some(name)) = (bottom, text) {
                        let symbol = self.text(child, &format!("{LAYER}_{LAYER}記号"));
                        self.boring.layers.push(Layer {
                            bottom,
                            name,
                            symbol,
                        });
                    }
                }
                "色調" => {
                    if let Some(bottom) = self.number(child, "色調_下端深度", cx) {
                        let name = self.text(child, "色調_色調名");
                        self.boring.colors.push(Color { bottom, name });
                    }
                }
                "観察記事" => {
                    let top = self.number(child, "観察記事_上端深度", cx);
                    let bottom = self.number(child, "観察記事_下端深度", cx);
                    if let (Some(top), Some(bottom)) = (top, bottom) {
                        let text = self.text(child, "観察記事_記事");
                        self.boring
                            .observations
                            .push(Observation { top, bottom, text });
                    }
                }
                "標準貫入試験" => {
                    let depth = self.number(child, "標準貫入試験_開始深度", cx);
                    let blows = self.integer(child, "標準貫入試験_合計打撃回数", cx);
                    let penetration = self.number(child, "標準貫入試験_合計貫入量", cx);
                    if let (Some(depth), Some(blows), Some(penetration)) =
                        (depth, blows, penetration)
                    {
                        self.boring.penetrations.push(Penetration {
                            depth,
                            blows,
                            penetration,
                        });
                    }
                }
                "孔内水位" => {
                    let level = WaterLevel {
                        date: self.text(child, "孔内水位_測定年月日"),
                        state: self.text(child, "孔内水位_削孔状況コード"),
                        depth: self.number(child, "孔内水位_孔内水位", cx),
                        note: self.text(child, "孔内水位_水位種別備考"),
                    };
                    self.boring.water_levels.push(level);
                }
                _ => self.unsupported(name),
            }
        }
        Ok(())
    }
}

fn invalid(cx: &mut Cx, name: &str, value: &str) {
    cx.report(
        Diagnostic::new(codes::BORING_VALUE_INVALID)
            .with("name", name)
            .with("value", value),
    );
}
