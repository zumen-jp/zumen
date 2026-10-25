//! DXF から図面モデル（L2）への変換（決定 0022）。いまは ENTITIES の LINE・TEXT・VIEWPORT だけを移す。
//!
//! モデル空間の図形は実寸の空間に、ペーパー空間（67 = 1）の図形とビューポートは 1 枚の用紙に置く。
//! 実寸の単位は見出しの `$INSUNITS` で決め、分からなければ不明とする。用紙の点は `$PLIMMIN` を原点にした mm で、
//! 単位が分からなければ mm に直せないので、ペーパー空間の図形を移さない。
//! 移さない図形と値は、レコードを指す診断で返す。何をどう読んだかの推定は `docs/formats/dxf.md` に記録した。

use super::version::{name, text};
use super::{DxfFile, Record, Tag, ViewError, parse_real, stopped};
use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_drawing::{
    Affine, Drawing2D, Element, Fit, Height, HeightMeaning, Horizontal, Length, ModelLength,
    ModelPoint, ModelSpace, PaperLength, PaperPoint, Placement, Shape, Sheet, SheetId, SpaceId,
    Text, Unit, Vertical, View,
};
use zumen_sdk::{Error, Lift};

/// UTF-8 で書く最初の版（2007）
const UTF8: &[u8] = b"AC1021";

/// 移さない理由。診断は大きいので箱に入れる
type Lifted<T> = Result<T, Box<Diagnostic>>;

const MODEL: SpaceId = SpaceId(0);
const SHEET: SheetId = SheetId(0);

impl Lift<Drawing2D> for DxfFile {
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        let unit = self.unit(cx);
        let limits = |name: &[u8]| {
            self.variable(name)
                .and_then(|tags| Some([self.header_real(tags, 10)?, self.header_real(tags, 20)?]))
        };
        let lifter = Lifter {
            file: self,
            unicode: self.version().is_some_and(|v| v >= UTF8),
            mm: unit.millimeters(),
            origin: limits(b"$PLIMMIN").unwrap_or_default(),
        };
        let mut out = Out::default();
        let mut section: &[u8] = b"";
        for (index, record) in self.records.iter().enumerate() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            match self.kind(record) {
                b"SECTION" => section = name(self, record),
                b"ENDSEC" => section = b"",
                kind if section == b"ENTITIES" => {
                    if let Err(diagnostic) = lifter.entity(index, record, kind, &mut out) {
                        cx.report((*diagnostic).at(Ref::at_record(index as u64)));
                    }
                }
                _ => {}
            }
        }
        let mut drawing = Drawing2D::default();
        drawing.spaces.push(ModelSpace {
            id: MODEL,
            unit,
            elements: out.model,
        });
        if !out.paper.is_empty() || !out.views.is_empty() {
            let size = match (limits(b"$PLIMMAX"), lifter.mm) {
                (Some([x, y]), Some(mm)) => {
                    let [ox, oy] = lifter.origin;
                    let size = [(x - ox) * mm, (y - oy) * mm];
                    size.iter().all(|s| *s > 0.0).then(|| size.map(PaperLength))
                }
                _ => None,
            };
            drawing.sheets.push(Sheet {
                id: SHEET,
                size,
                elements: out.paper,
                views: out.views,
            });
        }
        Ok(drawing)
    }
}

impl DxfFile {
    /// 見出しの `$INSUNITS`（DXF Reference「HEADER Section Group Codes」）。長さの単位でなければ不明
    fn unit(&self, cx: &mut Cx) -> Unit {
        let code = self
            .variable(b"$INSUNITS")
            .and_then(|tags| tags.iter().find(|t| t.code == 70))
            .and_then(|tag| int(self.value(tag)));
        let unit = match code {
            Some(1) => Unit::Inch,
            Some(2) => Unit::Foot,
            Some(4) => Unit::Millimeter,
            Some(5) => Unit::Centimeter,
            Some(6) => Unit::Meter,
            _ => Unit::Unknown,
        };
        if unit == Unit::Unknown {
            cx.report(Diagnostic::new(codes::DXF_UNITS_MISSING));
        }
        unit
    }

    /// 見出しの変数の値のタグ。なければ `None`
    fn variable(&self, name: &[u8]) -> Option<&[Tag]> {
        self.records
            .iter()
            .filter(|record| self.kind(record) == b"SECTION")
            .flat_map(|record| record.tags.chunk_by(|_, b| b.code != 9))
            .find_map(|chunk| {
                let (head, tags) = chunk.split_first()?;
                (head.code == 9 && self.value(head).trim_ascii() == name).then_some(tags)
            })
    }

    fn header_real(&self, tags: &[Tag], code: i16) -> Option<f64> {
        tags.iter()
            .find(|t| t.code == code)
            .and_then(|t| parse_real(self.value(t)))
    }
}

#[derive(Default)]
struct Out {
    model: Vec<Element<ModelPoint>>,
    paper: Vec<Element<PaperPoint>>,
    views: Vec<View>,
}

struct Lifter<'a> {
    file: &'a DxfFile,
    /// 2007 以降の版なら、文字の値を UTF-8 として読む
    unicode: bool,
    /// 1 単位の mm の数。不明なら `None`
    mm: Option<f64>,
    /// 用紙の原点（`$PLIMMIN`、図面の単位）
    origin: [f64; 2],
}

impl Lifter<'_> {
    fn entity(&self, index: usize, record: &Record, kind: &[u8], out: &mut Out) -> Lifted<()> {
        let source = Ref::at_record(index as u64);
        let paper = self.int(record, kind, 67, 0)? == 1;
        match kind {
            // 69 = 1 は用紙そのものを表すビューポート
            b"VIEWPORT" if paper && self.int(record, kind, 69, 0)? == 1 => {}
            b"VIEWPORT" if paper => {
                let mm = self.paper_mm(kind)?;
                out.views.push(self.view(record, kind, mm, source)?);
            }
            b"LINE" | b"TEXT" if paper => {
                let mm = self.paper_mm(kind)?;
                let [ox, oy] = self.origin;
                let point = |x: f64, y: f64| PaperPoint {
                    sheet: SHEET,
                    x: (x - ox) * mm,
                    y: (y - oy) * mm,
                };
                let length = |value: f64| Length::Paper(PaperLength(value * mm));
                out.paper.push(Element {
                    layer: self.layer(record, kind)?,
                    shape: self.shape(record, kind, point, length)?,
                    source,
                });
            }
            b"LINE" | b"TEXT" => {
                let point = |x: f64, y: f64| ModelPoint { space: MODEL, x, y };
                let length = |value: f64| {
                    Length::Model(ModelLength {
                        space: MODEL,
                        value,
                    })
                };
                out.model.push(Element {
                    layer: self.layer(record, kind)?,
                    shape: self.shape(record, kind, point, length)?,
                    source,
                });
            }
            _ => {
                return Err(Box::new(
                    Diagnostic::new(codes::DXF_LIFT_DROPPED).with("kind", text(kind)),
                ));
            }
        }
        Ok(())
    }

    fn shape<P: Copy>(
        &self,
        record: &Record,
        kind: &[u8],
        point: impl Fn(f64, f64) -> P,
        length: impl Fn(f64) -> Length,
    ) -> Lifted<Shape<P>> {
        self.planar(record, kind)?;
        if kind == b"LINE" {
            let line = self
                .file
                .line(record)
                .unwrap_or(Err(ViewError::Missing(10)))
                .map_err(|error| match error {
                    ViewError::Missing(code) | ViewError::NotReal(code) => invalid(kind, code),
                })?;
            if line.start[2] != 0.0 || line.end[2] != 0.0 {
                return Err(non_planar(kind));
            }
            return Ok(Shape::Line {
                start: point(line.start[0], line.start[1]),
                end: point(line.end[0], line.end[1]),
            });
        }
        // TEXT（DXF Reference「TEXT」）
        let at = |x: i16| -> Lifted<P> {
            if self.real(record, kind, x + 20, Some(0.0))? != 0.0 {
                return Err(non_planar(kind));
            }
            Ok(point(
                self.real(record, kind, x, None)?,
                self.real(record, kind, x + 10, None)?,
            ))
        };
        let first = at(10)?;
        let horizontal = self.int(record, kind, 72, 0)?;
        let vertical = self.int(record, kind, 73, 0)?;
        let placement = match (horizontal, vertical) {
            (0, 0) => Placement::Anchor {
                point: first,
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline,
            },
            (3, _) | (5, _) => Placement::Between {
                first,
                second: at(11)?,
                fit: if horizontal == 3 {
                    Fit::Aligned
                } else {
                    Fit::Fit
                },
            },
            _ => Placement::Anchor {
                point: at(11)?,
                horizontal: match horizontal {
                    0 => Horizontal::Left,
                    1 => Horizontal::Center,
                    2 => Horizontal::Right,
                    4 => Horizontal::Middle,
                    _ => return Err(invalid(kind, 72)),
                },
                vertical: match vertical {
                    0 => Vertical::Baseline,
                    1 => Vertical::Bottom,
                    2 => Vertical::Middle,
                    3 => Vertical::Top,
                    _ => return Err(invalid(kind, 73)),
                },
            },
        };
        let flags = self.int(record, kind, 71, 0)?;
        Ok(Shape::Text(Text {
            content: self
                .string(record, kind, 1)?
                .ok_or_else(|| invalid(kind, 1))?,
            height: Height {
                value: length(self.real(record, kind, 40, None)?),
                meaning: HeightMeaning::Declared("dxf.TEXT.40"),
            },
            placement,
            rotation: self.real(record, kind, 50, Some(0.0))?,
            width_factor: self.real(record, kind, 41, Some(1.0))?,
            oblique: self.real(record, kind, 51, Some(0.0))?,
            backward: flags & 2 != 0,
            upside_down: flags & 4 != 0,
            style: self
                .string(record, kind, 7)?
                .unwrap_or_else(|| "STANDARD".to_owned()),
        }))
    }

    /// ペーパー空間のビューポート（DXF Reference「VIEWPORT」）。ねじれ・視点の向き・注視点のないものだけを扱う
    fn view(&self, record: &Record, kind: &[u8], mm: f64, source: Ref) -> Lifted<View> {
        let real = |code: i16, default: Option<f64>| self.real(record, kind, code, default);
        let unsupported = |code: i16| {
            Box::new(Diagnostic::new(codes::DXF_VIEWPORT_UNSUPPORTED).with("code", i64::from(code)))
        };
        if [
            real(16, Some(0.0))?,
            real(26, Some(0.0))?,
            real(36, Some(1.0))?,
        ] != [0.0, 0.0, 1.0]
        {
            return Err(unsupported(16));
        }
        if [
            real(17, Some(0.0))?,
            real(27, Some(0.0))?,
            real(37, Some(0.0))?,
        ] != [0.0; 3]
        {
            return Err(unsupported(17));
        }
        if real(51, Some(0.0))? != 0.0 {
            return Err(unsupported(51));
        }
        let [px, py] = [real(10, None)?, real(20, None)?];
        let [width, height] = [real(40, None)?, real(41, None)?];
        let [vx, vy] = [real(12, Some(0.0))?, real(22, Some(0.0))?];
        let view_height = real(45, None)?;
        for (code, value) in [(40, width), (41, height), (45, view_height)] {
            if value <= 0.0 {
                return Err(invalid(kind, code));
            }
        }
        // 用紙の mm = mm × (用紙の中心 + 縮尺 × (実寸の点 − ビューの中心) − 用紙の原点)
        let [ox, oy] = self.origin;
        let scale = height / view_height * mm;
        let transform = Affine {
            m: [
                scale,
                0.0,
                (px - ox) * mm - scale * vx,
                0.0,
                scale,
                (py - oy) * mm - scale * vy,
            ],
        };
        let corner = |dx: f64, dy: f64| PaperPoint {
            sheet: SHEET,
            x: (px + dx * width / 2.0 - ox) * mm,
            y: (py + dy * height / 2.0 - oy) * mm,
        };
        Ok(View {
            space: MODEL,
            transform,
            clip: vec![
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ],
            source,
        })
    }

    /// 押し出しの向き（210・220・230）が Z でなければ、平面に移さない
    fn planar(&self, record: &Record, kind: &[u8]) -> Lifted<()> {
        let extrusion = [
            self.real(record, kind, 210, Some(0.0))?,
            self.real(record, kind, 220, Some(0.0))?,
            self.real(record, kind, 230, Some(1.0))?,
        ];
        if extrusion == [0.0, 0.0, 1.0] {
            Ok(())
        } else {
            Err(non_planar(kind))
        }
    }

    fn paper_mm(&self, kind: &[u8]) -> Lifted<f64> {
        self.mm.ok_or_else(|| {
            Box::new(Diagnostic::new(codes::DXF_PAPER_SPACE_DROPPED).with("kind", text(kind)))
        })
    }

    fn layer(&self, record: &Record, kind: &[u8]) -> Lifted<String> {
        self.string(record, kind, 8)?
            .ok_or_else(|| invalid(kind, 8))
    }

    fn tag<'r>(&self, record: &'r Record, code: i16) -> Option<&'r Tag> {
        record.tags.iter().find(|t| t.code == code)
    }

    /// 実数の値。なければ `default`、それもなければ読めないとする
    fn real(&self, record: &Record, kind: &[u8], code: i16, default: Option<f64>) -> Lifted<f64> {
        match self.tag(record, code) {
            Some(tag) => parse_real(self.file.value(tag)),
            None => default,
        }
        .ok_or_else(|| invalid(kind, code))
    }

    /// 整数の値。なければ `default`
    fn int(&self, record: &Record, kind: &[u8], code: i16, default: i64) -> Lifted<i64> {
        match self.tag(record, code) {
            Some(tag) => int(self.file.value(tag)).ok_or_else(|| invalid(kind, code)),
            None => Ok(default),
        }
    }

    /// 文字の値。2007 より前の版の ASCII でない文字は、コード・ページを読まないので受け付けない
    fn string(&self, record: &Record, kind: &[u8], code: i16) -> Lifted<Option<String>> {
        let Some(tag) = self.tag(record, code) else {
            return Ok(None);
        };
        let value = self.file.value(tag);
        if self.unicode || value.is_ascii() {
            match core::str::from_utf8(value) {
                Ok(value) => Ok(Some(value.to_owned())),
                Err(_) => Err(invalid(kind, code)),
            }
        } else {
            Err(Box::new(
                Diagnostic::new(codes::DXF_CODE_PAGE_UNSUPPORTED).with("kind", text(kind)),
            ))
        }
    }
}

fn int(value: &[u8]) -> Option<i64> {
    core::str::from_utf8(value.trim_ascii()).ok()?.parse().ok()
}

fn invalid(kind: &[u8], code: i16) -> Box<Diagnostic> {
    Box::new(
        Diagnostic::new(codes::DXF_LIFT_INVALID)
            .with("kind", text(kind))
            .with("code", i64::from(code)),
    )
}

fn non_planar(kind: &[u8]) -> Box<Diagnostic> {
    Box::new(Diagnostic::new(codes::DXF_NON_PLANAR_DROPPED).with("kind", text(kind)))
}
