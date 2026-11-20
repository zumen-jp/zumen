//! 常駐文書の読み込み・差分編集・表示差分同期・空間選択・履歴を、小さい操作ごとの共有予算で繰り返す。

#![deny(clippy::indexing_slicing)]

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::{self, Write};

use zumen_acad::AcadLive;
use zumen_base::{
    Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Ref, Stop, codes,
};
use zumen_drawing::{
    Affine, Axes, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edge, Edit, Element, Hash,
    Height, HeightMeaning, Horizontal, Inherit, Layer, LayerId, LayerState, LayerStyle, Length,
    LineType, LineTypeDefinition, LineTypeId, LineWeight, ModelLength, ModelPoint, ModelSpace,
    PaperLength, PaperPoint, PatternBasis, Placed, Placement, PointMark, PointSize, PointStyle,
    Polyline, Position, Restyle, Seen, Shape, SpaceId, Style, Symbol, SymbolDefinition, SymbolKey,
    Text, Unit, Vertex, Vertical, id,
};
use zumen_ifc::IfcLive;
use zumen_jww::JwwLive;
use zumen_live::RowId;
use zumen_live::display::Display;
use zumen_live::display_wire::{self, Packet};
use zumen_sdk::Error;
use zumen_sxf::SxfLive;

/// 形式ごとの入口。SXF は SFC・P21 と圧縮されたアーカイブ（P2Z・SFZ、同梱の SAF を含む）を扱う。
#[derive(Clone, Copy)]
pub enum Format {
    Dxf,
    Dwg,
    Jww,
    Sxf,
    Ifc,
}

struct SxfArchive {
    entries: Vec<(String, Vec<u8>)>,
    index: usize,
}

impl SxfArchive {
    fn attribute_file(&self) -> Option<&[u8]> {
        let (sxf, _) = self.entries.get(self.index)?;
        let sxf = sxf.to_ascii_lowercase();
        let stem = sxf.rsplit_once('.').map_or(sxf.as_str(), |(stem, _)| stem);
        let names = [format!("{stem}.saf"), format!("{sxf}.saf")];
        self.entries
            .iter()
            .enumerate()
            .find(|(at, (name, _))| *at != self.index && names.contains(&name.to_ascii_lowercase()))
            .map(|(_, (_, data))| data.as_slice())
    }

    fn write(&self, sxf: &[u8], cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        let estimated = self
            .entries
            .iter()
            .enumerate()
            .map(|(at, (name, data))| {
                let body = if at == self.index {
                    sxf.len()
                } else {
                    data.len()
                };
                (name.len().saturating_mul(2).saturating_add(body) as u64).saturating_add(96)
            })
            .fold(32_u64, u64::saturating_add);
        cx.check(Kind::Work, self.entries.len() as u64 + 1)
            .and_then(|()| cx.check(Kind::DecodedBytes, estimated))
            .map_err(|stop| report_stop(cx, stop))?;
        let entries: Vec<(&str, &[u8])> = self
            .entries
            .iter()
            .enumerate()
            .map(|(at, (name, data))| {
                let data = if at == self.index { sxf } else { data };
                (name.as_str(), data)
            })
            .collect();
        zumen_zip::write(&entries).ok_or_else(|| {
            cx.report(Diagnostic::new(codes::SXF_ARCHIVE_UNSUPPORTED));
            Error::Rejected
        })
    }
}

fn report_stop(cx: &mut Cx<'_>, stop: Stop) -> Error {
    let diagnostic = stop.to_diagnostic();
    if !cx
        .diag
        .iter()
        .any(|reported| reported.code() == diagnostic.code())
    {
        cx.report(diagnostic);
    }
    Error::Stopped(stop)
}

fn is_sxf_entry(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".sfc") || lower.ends_with(".p21")
}

fn read_sxf_archive(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Option<(Vec<u8>, SxfArchive)>, Error> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return Ok(None);
    }
    let mut quiet = Diagnostics::with_max_places(cx.diag.max_places());
    let archive = match zumen_zip::read(bytes, &mut Cx::new(cx.budget, &mut quiet, cx.cancel)) {
        Ok(archive) => archive,
        Err(Error::Stopped(stop)) => return Err(report_stop(cx, stop)),
        Err(_) => return Ok(None),
    };
    let Some(index) = archive
        .entries()
        .iter()
        .position(|entry| !entry.is_directory() && is_sxf_entry(entry.name()))
    else {
        return Ok(None);
    };
    for diagnostic in quiet.iter() {
        cx.report(diagnostic.clone());
    }
    cx.check(
        Kind::DecodedBytes,
        (archive.entries().len() as u64).saturating_mul(64),
    )
    .map_err(|stop| report_stop(cx, stop))?;
    let mut entries = Vec::with_capacity(archive.entries().len());
    let mut sxf = Vec::new();
    for (at, entry) in archive.entries().iter().enumerate() {
        cx.check(Kind::Work, 1)
            .and_then(|()| cx.check(Kind::DecodedBytes, entry.name().len() as u64))
            .map_err(|stop| report_stop(cx, stop))?;
        let data = if entry.is_directory() {
            Vec::new()
        } else {
            archive.extract(entry, cx).map_err(|error| match error {
                Error::Stopped(stop) => report_stop(cx, stop),
                other => other,
            })?
        };
        if at == index {
            sxf = data;
            entries.push((entry.name().to_owned(), Vec::new()));
        } else {
            entries.push((entry.name().to_owned(), data));
        }
    }
    Ok(Some((sxf, SxfArchive { entries, index })))
}

fn open_sxf(
    data: &[u8],
    threads: usize,
    cx: &mut Cx<'_>,
) -> Result<(Box<SxfLive>, Option<SxfArchive>), Error> {
    if let Some((sxf_bytes, archive)) = read_sxf_archive(data, cx)? {
        let attributes = match archive.attribute_file() {
            Some(saf_bytes) => match zumen_sxf::saf::read(saf_bytes, cx) {
                Ok(file) => Some(file),
                Err(Error::Stopped(stop)) => return Err(report_stop(cx, stop)),
                Err(_) => None,
            },
            None => None,
        };
        let live = SxfLive::open_with_threads_and_attributes(&sxf_bytes, threads, attributes, cx)?;
        return Ok((Box::new(live), Some(archive)));
    }
    SxfLive::open_with_threads(data, threads, cx).map(|live| (Box::new(live), None))
}

enum Engine {
    Acad(Box<AcadLive>),
    Jww(Box<JwwLive>),
    Sxf(Box<SxfLive>, Option<SxfArchive>),
    Ifc(Box<IfcLive>),
}

enum Prepared {
    Drawing(Changes),
    Ifc(zumen_ifc::Edit),
    Empty,
}

impl Engine {
    fn open(format: Format, data: &[u8], threads: usize, cx: &mut Cx<'_>) -> Result<Self, Error> {
        match format {
            Format::Dxf => {
                AcadLive::open_dxf_with_threads(data, threads, cx).map(|v| Self::Acad(Box::new(v)))
            }
            Format::Dwg => {
                AcadLive::open_dwg_with_threads(data, threads, cx).map(|v| Self::Acad(Box::new(v)))
            }
            Format::Jww => {
                JwwLive::open_with_threads(data, threads, cx).map(|v| Self::Jww(Box::new(v)))
            }
            Format::Sxf => open_sxf(data, threads, cx).map(|(v, arc)| Self::Sxf(v, arc)),
            Format::Ifc => {
                IfcLive::open_with_threads(data, threads, cx).map(|v| Self::Ifc(Box::new(v)))
            }
        }
    }

    fn state(&self) -> (u64, u64, usize, usize) {
        match self {
            Self::Acad(v) => (v.state_id(), v.revision().0, v.undo_len(), v.redo_len()),
            Self::Jww(v) => (v.state_id(), v.revision().0, v.history_bytes(), 0),
            Self::Sxf(v, _) => (v.state_id(), v.revision().0, v.undo_len(), v.redo_len()),
            Self::Ifc(v) => (v.state_id(), v.revision().0, v.undo_len(), v.redo_len()),
        }
    }

    fn display(&self) -> Option<&Display> {
        match self {
            Self::Acad(v) => Some(v.display()),
            Self::Jww(v) => Some(v.display()),
            Self::Sxf(v, _) => Some(v.display()),
            Self::Ifc(_) => None,
        }
    }

    fn clear_dirty(&mut self) {
        match self {
            Self::Acad(v) => v.display_mut().clear_dirty(),
            Self::Jww(v) => v.clear_display_dirty(),
            Self::Sxf(v, _) => v.clear_display_dirty(),
            Self::Ifc(_) => {}
        }
    }

    fn dirty_symbols(&self) -> Option<&BTreeSet<u32>> {
        match self {
            Self::Acad(v) => Some(v.dirty_symbols()),
            Self::Jww(v) => Some(v.dirty_symbols()),
            Self::Sxf(v, _) => Some(v.dirty_symbols()),
            Self::Ifc(_) => None,
        }
    }

    fn clear_dirty_symbols(&mut self) {
        match self {
            Self::Acad(v) => v.clear_dirty_symbols(),
            Self::Jww(v) => v.clear_dirty_symbols(),
            Self::Sxf(v, _) => v.clear_dirty_symbols(),
            Self::Ifc(_) => {}
        }
    }

    fn drawing(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        match self {
            Self::Acad(v) => v.drawing(cx),
            Self::Jww(v) => v.drawing(cx),
            Self::Sxf(v, _) => v.drawing(cx),
            Self::Ifc(_) => Err(Error::Unsupported),
        }
    }

    fn model(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        if let Self::Ifc(v) = self {
            return bounded_debug(v.building(), cx);
        }
        let drawing = self.drawing(cx)?;
        let source = zumen_drawing::binary::Source {
            format: "fuzz".to_owned(),
            family: "fuzz".to_owned(),
            hash: Hash::document(&[]),
        };
        zumen_drawing::binary::write(&source, &drawing, cx).map_err(|error| match error {
            zumen_drawing::binary::WriteError::Stopped(stop) => Error::Stopped(stop),
            zumen_drawing::binary::WriteError::TooLarge => Error::Invalid,
        })
    }

    fn save(&self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        match self {
            Self::Acad(v) => v.save(cx),
            Self::Jww(v) => v.save(cx),
            Self::Sxf(v, archive) => {
                let sxf = v.save(cx)?;
                match archive {
                    Some(arc) => arc.write(&sxf, cx),
                    None => Ok(sxf),
                }
            }
            Self::Ifc(v) => v.save(cx),
        }
    }

    fn row_count_2d(&self) -> Option<usize> {
        match self {
            Self::Acad(v) => Some(v.row_count()),
            Self::Jww(v) => Some(v.row_count()),
            Self::Sxf(v, _) => Some(v.row_count()),
            Self::Ifc(_) => None,
        }
    }

    fn symbol_count_2d(&self) -> u32 {
        match self {
            Self::Acad(v) => v.display_symbol_count(),
            Self::Jww(v) => v.display_symbol_count(),
            Self::Sxf(v, _) => v.display_symbol_count(),
            Self::Ifc(_) => 0,
        }
    }

    fn seen_of_row(&self, row: RowId, cx: &mut Cx<'_>) -> Result<Option<Seen>, Error> {
        match self {
            Self::Acad(v) => v.seen_of_row(row, cx),
            Self::Jww(v) => v.seen_of_row(row, cx),
            Self::Sxf(v, _) => v.seen_of_row(row, cx),
            Self::Ifc(_) => Err(Error::Unsupported),
        }
    }

    fn display_template(
        &self,
        row: RowId,
        cx: &mut Cx<'_>,
    ) -> Result<Option<(String, Placed)>, Error> {
        match self {
            Self::Acad(v) => v.display_template(row, cx),
            Self::Jww(v) => v.display_template(row, cx),
            Self::Sxf(v, _) => v.display_template(row, cx),
            Self::Ifc(_) => Err(Error::Unsupported),
        }
    }

    fn sheet_of_row(&self, row: RowId) -> Option<Affine> {
        match self {
            Self::Acad(_) => Some(Affine {
                m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            }),
            Self::Jww(v) => v.sheet_of_row(row),
            Self::Sxf(v, _) => v.sheet_of_row(row),
            Self::Ifc(_) => None,
        }
    }

    fn seen_of_symbol_element(
        &self,
        symbol: u32,
        element: u32,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Seen>, Error> {
        match self {
            Self::Acad(v) => v.seen_of_symbol_element(symbol, element, cx),
            Self::Jww(v) => v.seen_of_symbol_element(symbol, element, cx),
            Self::Sxf(v, _) => v.seen_of_symbol_element(symbol, element, cx),
            Self::Ifc(_) => Err(Error::Unsupported),
        }
    }

    fn display_symbol_key(&self, index: u32) -> Option<SymbolKey> {
        match self {
            Self::Acad(v) => v.display_symbol_key(index),
            Self::Jww(v) => v.display_symbol_key(index),
            Self::Sxf(v, _) => v.display_symbol_key(index),
            Self::Ifc(_) => None,
        }
    }

    fn display_retexted(
        &self,
        row: RowId,
        content: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Placed>, Error> {
        match self {
            Self::Acad(v) => v.display_retexted(row, content, cx),
            Self::Jww(v) => v.display_retexted(row, content, cx),
            Self::Sxf(v, _) => v.display_retexted(row, content, cx),
            Self::Ifc(_) => Err(Error::Unsupported),
        }
    }

    fn prepare(&self, seed: u64, cx: &mut Cx<'_>) -> Result<Prepared, Error> {
        if let Self::Ifc(v) = self {
            let Some(element) = v
                .building()
                .elements()
                .iter()
                .find_map(|e| e.source().instance)
            else {
                return Ok(Prepared::Empty);
            };
            let edit = if seed & 1 == 0 {
                zumen_ifc::Edit::Attribute {
                    element,
                    attribute: zumen_ifc::RootAttribute::Name,
                    value: Some(format!("fuzz-{seed:X}")),
                }
            } else {
                zumen_ifc::Edit::Property {
                    element,
                    set: "LiveFuzz".to_owned(),
                    name: "Value".to_owned(),
                    value: zumen_ifc::NewValue {
                        type_name: "IfcLabel".to_owned(),
                        literal: zumen_ifc::NewLiteral::Text(format!("{seed:X}")),
                    },
                }
            };
            return Ok(Prepared::Ifc(edit));
        }

        let drawing = self.drawing(cx)?;
        let offset = match (seed >> 8) % 6 {
            0 => f64::NAN,
            1 => f64::INFINITY,
            _ => ((seed >> 16) as i8 as f64) / 8.0 + 1.25,
        };
        let dy = ((seed >> 24) as i8 as f64) / 8.0 - 0.75;
        let kind = (seed % 14) as u32;

        // 表の編集・定義追加・点スタイルは図形要素が 0 件でも試せる
        match kind {
            6 => {
                let sym_total = self.symbol_count_2d();
                if sym_total > 0 {
                    let sym_idx = (seed >> 4) as u32 % sym_total;
                    if let Some(seen) = self.seen_of_symbol_element(sym_idx, 0, cx)? {
                        return Ok(Prepared::Drawing(Changes {
                            document: None,
                            edits: vec![Edit::Transform {
                                target: seen,
                                transform: Affine::translation(offset, dy),
                            }],
                        }));
                    }
                }
            }
            7 => {
                let color_sys = match self {
                    Self::Acad(_) => ColorSystem::Aci,
                    Self::Jww(_) => ColorSystem::Jww,
                    Self::Sxf(_, _) => ColorSystem::Sxf,
                    Self::Ifc(_) => ColorSystem::Aci,
                };
                let layer = Layer {
                    id: LayerId(0),
                    name: format!("FZ_{:04X}", (seed >> 12) & 0xFFFF),
                    state: Some(LayerState {
                        visible: (seed & 0x100) == 0,
                        printable: true,
                        editable: true,
                        frozen: false,
                    }),
                    style: Some(LayerStyle {
                        color: Some(Color {
                            code: Some(ColorCode {
                                system: color_sys,
                                value: ((seed >> 20) as u32 % 7) + 1,
                            }),
                            rgb: None,
                        }),
                        line_type: drawing.line_types.first().map(|lt| lt.id),
                        line_weight: None,
                        transparency: None,
                    }),
                    follows_placement: false,
                    description: None,
                    source: Ref::default(),
                };
                return Ok(Prepared::Drawing(Changes {
                    document: None,
                    edits: vec![Edit::AddLayer {
                        layer: Box::new(layer),
                    }],
                }));
            }
            8 if !drawing.layers.is_empty() => {
                let idx = (seed as usize >> 4) % drawing.layers.len();
                if let Some(cur) = drawing.layers.get(idx) {
                    let hash =
                        Hash::layer_checked(cur, cx).map_err(|stop| report_stop(cx, stop))?;
                    let mut next = cur.clone();
                    let mut st = next.state.unwrap_or(LayerState {
                        visible: true,
                        printable: true,
                        editable: true,
                        frozen: false,
                    });
                    st.visible = !st.visible;
                    next.state = Some(st);
                    if seed & 0x200 != 0 {
                        next.name = format!("RN_{:04X}", (seed >> 16) & 0xFFFF);
                    }
                    return Ok(Prepared::Drawing(Changes {
                        document: None,
                        edits: vec![Edit::ReplaceLayer {
                            target: Seen {
                                id: id::layer(cur),
                                hash,
                            },
                            layer: Box::new(next),
                        }],
                    }));
                }
            }
            9 if !drawing.layers.is_empty() => {
                let idx = (seed as usize >> 4) % drawing.layers.len();
                if let Some(cur) = drawing.layers.get(idx) {
                    let hash =
                        Hash::layer_checked(cur, cx).map_err(|stop| report_stop(cx, stop))?;
                    return Ok(Prepared::Drawing(Changes {
                        document: None,
                        edits: vec![Edit::RemoveLayer {
                            target: Seen {
                                id: id::layer(cur),
                                hash,
                            },
                        }],
                    }));
                }
            }
            10 => {
                let lt = LineType {
                    id: LineTypeId(0),
                    name: format!("LT_{:04X}", (seed >> 12) & 0xFFFF),
                    definition: Some(LineTypeDefinition {
                        description: "Fuzz dash".to_owned(),
                        pattern: vec![4.0, -2.0],
                        basis: PatternBasis::Element,
                        marks: Vec::new(),
                    }),
                    source: Ref::default(),
                };
                return Ok(Prepared::Drawing(Changes {
                    document: None,
                    edits: vec![Edit::AddLineType {
                        line_type: Box::new(lt),
                    }],
                }));
            }
            11 if !drawing.line_types.is_empty() => {
                let idx = (seed as usize >> 4) % drawing.line_types.len();
                if let Some(cur) = drawing.line_types.get(idx) {
                    let hash =
                        Hash::line_type_checked(cur, cx).map_err(|stop| report_stop(cx, stop))?;
                    let mut next = cur.clone();
                    next.definition = Some(LineTypeDefinition {
                        description: "Updated".to_owned(),
                        pattern: vec![6.0, -3.0],
                        basis: PatternBasis::Element,
                        marks: Vec::new(),
                    });
                    return Ok(Prepared::Drawing(Changes {
                        document: None,
                        edits: vec![Edit::ReplaceLineType {
                            target: Seen {
                                id: id::line_type(cur),
                                hash,
                            },
                            line_type: Box::new(next),
                        }],
                    }));
                }
            }
            12 if !drawing.line_types.is_empty() => {
                let idx = (seed as usize >> 4) % drawing.line_types.len();
                if let Some(cur) = drawing.line_types.get(idx) {
                    let hash =
                        Hash::line_type_checked(cur, cx).map_err(|stop| report_stop(cx, stop))?;
                    return Ok(Prepared::Drawing(Changes {
                        document: None,
                        edits: vec![Edit::RemoveLineType {
                            target: Seen {
                                id: id::line_type(cur),
                                hash,
                            },
                        }],
                    }));
                }
            }
            13 => {
                if seed & 0x400 == 0 {
                    let space = SpaceId(900);
                    let layer = drawing.layers.first().map_or(LayerId(0), |l| l.id);
                    let sym = SymbolDefinition {
                        key: SymbolKey {
                            namespace: Cow::Borrowed("dxf.block"),
                            name: format!("SYM_{:04X}", (seed >> 12) & 0xFFFF),
                        },
                        space: ModelSpace {
                            id: space,
                            unit: Unit::Unknown,
                            axes: Axes::EastNorth,
                            elements: vec![Element {
                                source: Ref::default(),
                                layer,
                                style: Style {
                                    color: Inherit::Layer,
                                    line_type: Inherit::Layer,
                                    line_type_scale: 1.0,
                                    line_weight: Some(Inherit::Layer),
                                    transparency: Inherit::Layer,
                                },
                                shape: Shape::Line {
                                    start: ModelPoint {
                                        space,
                                        x: 0.0,
                                        y: 0.0,
                                    },
                                    end: ModelPoint {
                                        space,
                                        x: 5.0,
                                        y: 2.5,
                                    },
                                },
                            }],
                            order: vec![0],
                        },
                        base: Some(ModelPoint {
                            space,
                            x: 0.0,
                            y: 0.0,
                        }),
                        attributes: Vec::new(),
                        anonymous: false,
                        external: None,
                        source: Ref::default(),
                    };
                    return Ok(Prepared::Drawing(Changes {
                        document: None,
                        edits: vec![Edit::AddSymbol {
                            symbol: Box::new(sym),
                        }],
                    }));
                }
                return Ok(Prepared::Drawing(Changes {
                    document: None,
                    edits: vec![Edit::SetPointStyle {
                        style: PointStyle {
                            mark: PointMark::Cross,
                            circle: (seed & 0x800) != 0,
                            square: false,
                            size: PointSize::Screen(5.0),
                        },
                    }],
                }));
            }
            _ => {}
        }

        let Some(target) = pick_target(&drawing, seed) else {
            return Ok(Prepared::Empty);
        };
        let edit = match kind % 6 {
            0 => Edit::Transform {
                target: target.seen,
                transform: Affine::translation(offset, dy),
            },
            1 => Edit::Copy {
                target: target.seen,
                transform: Affine::translation(offset, dy),
            },
            2 => {
                let mut style = Restyle::default();
                match (seed >> 4) % 4 {
                    0 => style.line_type_scale = Some(offset),
                    1 => {
                        if let Some(layer) = drawing.layers.last() {
                            style.layer = Some(layer.id);
                        } else {
                            style.line_type_scale = Some(offset);
                        }
                    }
                    2 => {
                        let sys = match self {
                            Self::Acad(_) => ColorSystem::Aci,
                            Self::Jww(_) => ColorSystem::Jww,
                            Self::Sxf(_, _) => ColorSystem::Sxf,
                            Self::Ifc(_) => ColorSystem::Aci,
                        };
                        style.color = Some(Inherit::Own(Color {
                            code: Some(ColorCode {
                                system: sys,
                                value: ((seed >> 20) as u32 % 6) + 1,
                            }),
                            rgb: None,
                        }));
                    }
                    _ => {
                        style.line_weight =
                            Some(Inherit::Own(LineWeight::Width(PaperLength(0.25))));
                    }
                }
                Edit::Restyle {
                    target: target.seen,
                    style,
                }
            }
            3 => {
                // 文字行があれば `display_retexted` の置き換えも通す
                let row_count = self.row_count_2d().unwrap_or(0);
                let mut retexted_edit = None;
                if row_count > 0 {
                    let probe = RowId((seed as usize % row_count) as u32);
                    if let (Some(seen), Some(placed)) = (
                        self.seen_of_row(probe, cx)?,
                        self.display_retexted(probe, &format!("TXT-{seed:X}"), cx)?,
                    ) {
                        retexted_edit = Some(Edit::Replace {
                            target: seen,
                            element: Box::new(placed),
                        });
                    }
                }
                retexted_edit.unwrap_or_else(|| Edit::Replace {
                    target: target.seen,
                    element: Box::new(target.element),
                })
            }
            4 => {
                let added =
                    build_added_variant(&target.element, target.position, self, &drawing, seed);
                Edit::Add {
                    place: target.place,
                    element: Box::new(added),
                }
            }
            _ => Edit::Remove {
                target: target.seen,
            },
        };
        Ok(Prepared::Drawing(Changes {
            document: None,
            edits: vec![edit],
        }))
    }

    fn edit(&mut self, prepared: &Prepared, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        let changes = match prepared {
            Prepared::Drawing(changes) => changes,
            Prepared::Ifc(edit) => {
                let Self::Ifc(v) = self else {
                    return Err(Error::Invalid);
                };
                let result = v.apply(std::slice::from_ref(edit), b"live-fuzz", cx)?;
                return Ok(format!("{result:?}").into_bytes());
            }
            Prepared::Empty => return Ok(Vec::new()),
        };
        match self {
            Self::Acad(v) => {
                let result = v.apply(changes, cx)?;
                Ok(format!("{result:?}").into_bytes())
            }
            Self::Jww(v) => {
                let result = v.apply(changes, cx)?;
                Ok(format!("{result:?}").into_bytes())
            }
            Self::Sxf(v, _) => {
                let result = v.apply(changes, cx)?;
                Ok(format!("{result:?}").into_bytes())
            }
            Self::Ifc(_) => Err(Error::Unsupported),
        }
    }

    /// 依存グラフの節の数。依存をたどれない形式は 0。
    fn node_count(&self) -> usize {
        match self {
            Self::Acad(v) => v.row_count(),
            Self::Sxf(v, _) => v.row_count(),
            Self::Ifc(v) => v.building().elements().len(),
            Self::Jww(_) => 0,
        }
    }

    /// `node` を変えたときに影響を受ける節
    fn affected(&self, node: u32, cx: &mut Cx<'_>) -> Result<Vec<u32>, Error> {
        let rows = |rows: Vec<RowId>| rows.into_iter().map(|row| row.0).collect();
        match self {
            Self::Acad(v) => v.affected_rows(&[RowId(node)], cx).map(rows),
            Self::Sxf(v, _) => v.affected_rows(&[RowId(node)], cx).map(rows),
            Self::Ifc(v) => v
                .affected_elements(&[zumen_building::ElementRef(node)], cx)
                .map(|elements| elements.into_iter().map(|element| element.0).collect()),
            Self::Jww(_) => Err(Error::Unsupported),
        }
    }

    fn undo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        match self {
            Self::Acad(v) => v.undo(cx),
            Self::Jww(v) => v.undo(cx),
            Self::Sxf(v, _) => v.undo(cx),
            Self::Ifc(v) => v.undo(cx),
        }
    }

    fn redo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        match self {
            Self::Acad(v) => v.redo(cx),
            Self::Jww(v) => v.redo(cx),
            Self::Sxf(v, _) => v.redo(cx),
            Self::Ifc(v) => v.redo(cx),
        }
    }
}

struct Target {
    position: Position,
    seen: Seen,
    place: String,
    element: Placed,
}

fn pick_target(drawing: &Drawing2D, seed: u64) -> Option<Target> {
    let mut candidates: Vec<Target> = Vec::new();
    let _: Result<(), ()> = id::each_element(drawing, |position, name, element| {
        if candidates.len() >= 8 {
            return Err(());
        }
        let place = match position {
            Position::Model { space, .. } if drawing.spaces.iter().any(|s| s.id == space) => {
                id::space(space)
            }
            Position::Paper { sheet, .. } => {
                let Some(sheet) = drawing.sheets.iter().find(|s| s.id == sheet) else {
                    return Ok(());
                };
                id::sheet(sheet)
            }
            _ => return Ok(()),
        };
        let seen = Seen {
            id: name,
            hash: element.hash(),
        };
        let element = match element {
            id::Found::Model(value) => Placed::Model(value.clone()),
            id::Found::Paper(value) => Placed::Paper(value.clone()),
        };
        candidates.push(Target {
            position,
            seen,
            place,
            element,
        });
        Ok(())
    });
    if candidates.is_empty() {
        None
    } else {
        let idx = (seed as usize >> 3) % candidates.len();
        Some(candidates.swap_remove(idx))
    }
}

fn build_added_variant(
    template: &Placed,
    position: Position,
    engine: &Engine,
    drawing: &Drawing2D,
    seed: u64,
) -> Placed {
    let variant = (seed >> 5) % 6;
    let sym_key = (engine.symbol_count_2d() > 0)
        .then(|| engine.display_symbol_key((seed as u32) % engine.symbol_count_2d()))
        .flatten();
    let style_name = drawing
        .text_styles
        .first()
        .map_or_else(|| "STANDARD".to_owned(), |s| s.name.clone());
    let height_key: Cow<'static, str> = match engine {
        Engine::Acad(_) => Cow::Borrowed("dxf.TEXT.40"),
        Engine::Jww(_) => Cow::Borrowed("jww.moji"),
        Engine::Sxf(_, _) => Cow::Borrowed("sxf.text"),
        Engine::Ifc(_) => Cow::Borrowed("dxf.TEXT.40"),
    };
    match (template, position) {
        (Placed::Model(base), Position::Model { space, .. }) => {
            let pt = |x, y| ModelPoint { space, x, y };
            let len = |value| Length::Model(ModelLength { space, value });
            let shape = match variant {
                0 => Shape::Line {
                    start: pt(1.0, 2.0),
                    end: pt(8.0, 5.0),
                },
                1 => Shape::Circle {
                    center: pt(4.0, 4.0),
                    radius: len(3.0),
                },
                2 => Shape::Curve(Edge::Arc {
                    center: pt(4.0, 4.0),
                    radius: len(3.0),
                    start: 0.0,
                    end: 120.0,
                    counterclockwise: true,
                }),
                3 => Shape::Polyline(Polyline {
                    vertices: vec![
                        Vertex {
                            point: pt(0.0, 0.0),
                            bulge: 0.0,
                        },
                        Vertex {
                            point: pt(6.0, 0.0),
                            bulge: 0.0,
                        },
                        Vertex {
                            point: pt(6.0, 4.0),
                            bulge: 0.0,
                        },
                    ],
                    closed: (seed & 0x40) != 0,
                    width: None,
                    vertex_widths: Vec::new(),
                    line_type_generation: false,
                }),
                4 => Shape::Text(Text {
                    content: format!("ADD-{:04X}", seed & 0xFFFF),
                    height: Height {
                        value: len(2.5),
                        meaning: HeightMeaning::Declared(height_key),
                    },
                    placement: Placement::Anchor {
                        point: pt(2.0, 3.0),
                        horizontal: Horizontal::Left,
                        vertical: Vertical::Baseline,
                    },
                    rotation: 0.0,
                    width_factor: 1.0,
                    oblique: 0.0,
                    backward: false,
                    upside_down: false,
                    style: style_name,
                }),
                _ => match sym_key {
                    Some(key) => Shape::Symbol(Symbol {
                        key,
                        position: pt(5.0, 5.0),
                        scale: [1.0, 1.0],
                        rotation: 0.0,
                        grid: None,
                        attributes: Vec::new(),
                        clip: None,
                    }),
                    None => base.shape.clone(),
                },
            };
            Placed::Model(Element {
                source: Ref::default(),
                layer: base.layer,
                style: base.style,
                shape,
            })
        }
        (Placed::Paper(base), Position::Paper { sheet, .. }) => {
            let pt = |x, y| PaperPoint { sheet, x, y };
            let len = |value| Length::Paper(PaperLength(value));
            let shape = match variant % 4 {
                0 => Shape::Line {
                    start: pt(1.0, 2.0),
                    end: pt(8.0, 5.0),
                },
                1 => Shape::Circle {
                    center: pt(4.0, 4.0),
                    radius: len(3.0),
                },
                2 => Shape::Polyline(Polyline {
                    vertices: vec![
                        Vertex {
                            point: pt(0.0, 0.0),
                            bulge: 0.0,
                        },
                        Vertex {
                            point: pt(6.0, 0.0),
                            bulge: 0.0,
                        },
                        Vertex {
                            point: pt(6.0, 4.0),
                            bulge: 0.0,
                        },
                    ],
                    closed: false,
                    width: None,
                    vertex_widths: Vec::new(),
                    line_type_generation: false,
                }),
                _ => base.shape.clone(),
            };
            Placed::Paper(Element {
                source: Ref::default(),
                layer: base.layer,
                style: base.style,
                shape,
            })
        }
        _ => template.clone(),
    }
}

/// 受信側で 64 行ブロック差分を当てる 13 列の表示ミラー
#[derive(Clone, Default, PartialEq, Eq)]
struct DisplayMirror {
    columns: [Vec<u8>; 13],
    synced: bool,
}

impl DisplayMirror {
    fn apply_packet(&mut self, packet: &Packet<'_>) {
        for col in packet.columns() {
            let idx = col.kind() as usize;
            let new_len = (col.rows() as usize).saturating_mul(col.stride() as usize);
            let Some(buf) = self.columns.get_mut(idx) else {
                continue;
            };
            if packet.full() {
                buf.clear();
            }
            buf.resize(new_len, 0);
            for block in col.blocks() {
                let offset = (block.start as usize).saturating_mul(col.stride() as usize);
                let end = offset.saturating_add(block.bytes.len());
                if let Some(dst) = buf.get_mut(offset..end) {
                    dst.copy_from_slice(block.bytes);
                }
            }
        }
        self.synced = true;
    }
}

struct DebugBytes<'a, 'b> {
    bytes: Vec<u8>,
    cx: &'a mut Cx<'b>,
    stop: Option<Stop>,
}

impl Write for DebugBytes<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if let Err(stop) = self
            .cx
            .check(Kind::Work, 1)
            .and_then(|()| self.cx.check(Kind::DecodedBytes, text.len() as u64))
        {
            self.stop = Some(stop);
            return Err(fmt::Error);
        }
        self.bytes.extend_from_slice(text.as_bytes());
        Ok(())
    }
}

fn bounded_debug(value: &impl fmt::Debug, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
    let mut out = DebugBytes {
        bytes: Vec::new(),
        cx,
        stop: None,
    };
    if write!(out, "{value:?}").is_err() {
        return Err(out.stop.map_or(Error::Invalid, Error::Stopped));
    }
    Ok(out.bytes)
}

/// 保存・保持モデル・依存先の符号化。止まった取得は `None`。
type Snapshot = (Option<Vec<u8>>, Option<Vec<u8>>, Option<Vec<Vec<u8>>>);

struct Trace {
    limits: Limits,
    bytes: Vec<Vec<u8>>,
}

impl Trace {
    fn call<T>(
        &mut self,
        limits: Limits,
        cancel: bool,
        run: impl FnOnce(&mut Cx<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let token = CancelToken::new();
        if cancel {
            token.cancel();
        }
        let result = run(&mut Cx::new(&mut budget, &mut diagnostics, &token));
        let status = result.as_ref().map(|_| ());
        let used = Kind::ALL.map(|kind| budget.used(kind));
        self.bytes
            .push(format!("{status:?}\n{diagnostics:?}\n{used:?}").into_bytes());
        result
    }

    fn blob(&mut self, run: impl FnOnce(&mut Cx<'_>) -> Result<Vec<u8>, Error>) -> Option<Vec<u8>> {
        let bytes = self.call(self.limits, false, run).ok()?;
        self.bytes.push(bytes.clone());
        Some(bytes)
    }

    fn snapshot(&mut self, engine: &Engine) -> Snapshot {
        (
            self.blob(|cx| engine.save(cx)),
            self.blob(|cx| engine.model(cx)),
            self.dependencies(engine),
        )
    }

    /// 先頭の 16 節と最後の節について、変えたときに影響を受ける節を予算の中で求める。
    /// 取り消し・やり直しで、保存・保持モデルと同じく前後の依存へ戻ることを確かめる。
    fn dependencies(&mut self, engine: &Engine) -> Option<Vec<Vec<u8>>> {
        let count = engine.node_count();
        if count == 0 {
            return None;
        }
        let last = u32::try_from(count - 1).unwrap_or(u32::MAX);
        let mut nodes: Vec<u32> = (0..last.min(16)).collect();
        nodes.push(last);
        let mut edges = Vec::with_capacity(nodes.len());
        for node in nodes {
            let affected = self.blob(|cx| {
                let rows = engine.affected(node, cx)?;
                bounded_debug(&rows, cx)
            });
            edges.push(affected.unwrap_or_default());
        }
        Some(edges)
    }

    /// 全 2D 形式で行の正規 ID・ハッシュ・雛形・用紙変換・定義内要素の ID を試す
    fn row_targets(&mut self, engine: &Engine, seed: u64) {
        let Some(row_count) = engine.row_count_2d() else {
            return;
        };
        let rows = [0, (seed % row_count.max(1) as u64) as u32, u32::MAX];
        let work = self.limits.with(Kind::Work, Limit::Finite(0));
        let bytes = self.limits.with(Kind::DecodedBytes, Limit::Finite(0));
        for row in rows {
            let sheet = engine.sheet_of_row(RowId(row));
            self.bytes.push(format!("{sheet:?}").into_bytes());
            for (limits, cancel) in [
                (self.limits, false),
                (work, false),
                (bytes, false),
                (self.limits, true),
            ] {
                if let Ok(result) = self.call(limits, cancel, |cx| {
                    let target = engine.seen_of_row(RowId(row), cx)?;
                    let tmpl = engine.display_template(RowId(row), cx)?;
                    bounded_debug(&(target, tmpl), cx)
                }) {
                    self.bytes.push(result);
                }
            }
        }
        let sym_count = engine.symbol_count_2d();
        for sym in [0, (seed % u64::from(sym_count.max(1))) as u32, sym_count] {
            let key = engine.display_symbol_key(sym);
            self.bytes.push(format!("{key:?}").into_bytes());
            for (limits, cancel) in [(self.limits, false), (work, false), (self.limits, true)] {
                if let Ok(result) = self.call(limits, cancel, |cx| {
                    let s0 = engine.seen_of_symbol_element(sym, 0, cx)?;
                    let s_out = engine.seen_of_symbol_element(sym, u32::MAX, cx)?;
                    assert!(s_out.is_none(), "範囲外の定義内要素が Some を返した");
                    bounded_debug(&s0, cx)
                }) {
                    self.bytes.push(result);
                }
            }
        }
    }

    /// 常駐 IPC が呼ぶ読み取り（画層・線種・描く順・文字・内容の置き換え・記号の定義）を、
    /// 全量の予算・小さい予算・中断で呼ぶ。行の数を越える番号は `None` で、表示に何も足さない
    fn ipc_reads(&mut self, engine: &Engine, seed: u64) {
        let (row_count, symbol_count) = match engine {
            Engine::Acad(live) => (live.row_count(), live.display_symbol_count()),
            Engine::Jww(live) => (live.row_count(), live.display_symbol_count()),
            Engine::Sxf(live, _) => (live.row_count(), live.display_symbol_count()),
            Engine::Ifc(_) => return,
        };
        let rows = [0, (seed % row_count.max(1) as u64) as u32, u32::MAX];
        let symbols = [
            0,
            (seed % u64::from(symbol_count.max(1))) as u32,
            symbol_count,
        ];
        let content = format!("fuzz-{seed:X}");
        for (limits, cancel) in [
            (self.limits, false),
            (
                self.limits.with(Kind::Work, Limit::Finite(seed % 64)),
                false,
            ),
            (
                self.limits
                    .with(Kind::DecodedBytes, Limit::Finite(seed % 64)),
                false,
            ),
            (self.limits, true),
        ] {
            if let Ok(result) = self.call(limits, cancel, |cx| {
                let tables = match engine {
                    Engine::Acad(live) => (
                        live.display_layers(cx)?,
                        live.display_line_types(cx)?,
                        live.display_order(cx)?,
                    ),
                    Engine::Jww(live) => (
                        live.display_layers(cx)?,
                        live.display_line_types(cx)?,
                        live.display_order(cx)?,
                    ),
                    Engine::Sxf(live, _) => (
                        live.display_layers(cx)?,
                        live.display_line_types(cx)?,
                        live.display_order(cx)?,
                    ),
                    Engine::Ifc(_) => return Err(Error::Unsupported),
                };
                bounded_debug(&tables, cx)
            }) {
                self.bytes.push(result);
            }
            for row in rows {
                let row = RowId(row);
                if let Ok(result) = self.call(limits, cancel, |cx| {
                    let read = match engine {
                        Engine::Acad(live) => (
                            live.display_text(row, cx)?,
                            live.display_retexted(row, &content, cx)?,
                        ),
                        Engine::Jww(live) => (
                            live.display_text(row, cx)?,
                            live.display_retexted(row, &content, cx)?,
                        ),
                        Engine::Sxf(live, _) => (
                            live.display_text(row, cx)?,
                            live.display_retexted(row, &content, cx)?,
                        ),
                        Engine::Ifc(_) => return Err(Error::Unsupported),
                    };
                    bounded_debug(&read, cx)
                }) {
                    self.bytes.push(result);
                }
            }
            for symbol in symbols {
                if let Ok(result) = self.call(limits, cancel, |cx| {
                    let mut display = Display::with_rows(0);
                    let written = match engine {
                        Engine::Acad(live) => live.display_symbol(symbol, &mut display, cx)?,
                        Engine::Sxf(live, _) => live.display_symbol(symbol, &mut display, cx)?,
                        Engine::Jww(live) => live.display_symbol(symbol, &mut display, cx)?,
                        Engine::Ifc(_) => return Err(Error::Unsupported),
                    };
                    let rows = display.canonical();
                    match written {
                        Some(written) => assert_eq!(
                            written as usize,
                            rows.len(),
                            "記号の定義の行の数が表示と合わない"
                        ),
                        None => {
                            assert!(symbol >= symbol_count, "範囲内の定義が None になった");
                            assert!(rows.is_empty(), "範囲外の定義が表示に行を足した");
                        }
                    }
                    bounded_debug(&(written, rows), cx)
                }) {
                    self.bytes.push(result);
                }
            }
        }
    }

    /// 全 2D 形式で矩形選択と点選択を試し、停止時の出力配列ロールバックと昇順を確かめる
    fn spatial_queries(&mut self, engine: &Engine, seed: u64) {
        if matches!(engine, Engine::Ifc(_)) {
            return;
        }
        let rectangles = [
            [
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::INFINITY,
            ],
            [0.0, 0.0, 10.0, 10.0],
            [
                f64::from_bits(seed),
                -1.0,
                f64::from_bits(seed.rotate_left(23)),
                1.0,
            ],
        ];
        for rect in rectangles {
            for (limits, cancel) in [
                (self.limits, false),
                (
                    self.limits.with(Kind::Work, Limit::Finite(seed % 128)),
                    false,
                ),
                (
                    self.limits
                        .with(Kind::DecodedBytes, Limit::Finite(seed % 64)),
                    false,
                ),
                (
                    self.limits.with(Kind::Elements, Limit::Finite(seed % 16)),
                    false,
                ),
                (self.limits, true),
            ] {
                if let Ok(result) = self.call(limits, cancel, |cx| {
                    let mut rows = vec![RowId::NONE];
                    let res = match engine {
                        Engine::Acad(live) => live.select_rect_with_cx(rect, &mut rows, cx),
                        Engine::Jww(live) => live.select_rect_with_cx(rect, &mut rows, cx),
                        Engine::Sxf(live, _) => live.select_rect_with_cx(rect, &mut rows, cx),
                        Engine::Ifc(_) => return Err(Error::Unsupported),
                    };
                    if res.is_err() {
                        assert_eq!(rows, [RowId::NONE], "停止した矩形選択が出力を汚した");
                    }
                    res?;
                    assert!(
                        rows.get(1..)
                            .unwrap_or(&[])
                            .windows(2)
                            .all(|w| matches!(w, [a, b] if a < b)),
                        "矩形選択の結果が狭義昇順でない"
                    );
                    bounded_debug(&rows, cx)
                }) {
                    self.bytes.push(result);
                }
            }
        }

        let px = ((seed >> 8) as i16 as f64) / 16.0;
        let py = ((seed >> 24) as i16 as f64) / 16.0;
        let points = [
            ([0.0, 0.0], 2.5),
            ([px, py], 1.0),
            ([f64::NAN, 0.0], 1.0),
            ([0.0, 0.0], -1.0),
        ];
        for (pt, tol) in points {
            for (limits, cancel) in [
                (self.limits, false),
                (
                    self.limits.with(Kind::Work, Limit::Finite(seed % 128)),
                    false,
                ),
                (
                    self.limits
                        .with(Kind::DecodedBytes, Limit::Finite(seed % 64)),
                    false,
                ),
                (self.limits, true),
            ] {
                if let Ok(result) = self.call(limits, cancel, |cx| {
                    let mut rows = vec![RowId::NONE];
                    let res = match engine {
                        Engine::Acad(live) => live.select_point_with_cx(pt, tol, &mut rows, cx),
                        Engine::Jww(live) => live.select_point_with_cx(pt, tol, &mut rows, cx),
                        Engine::Sxf(live, _) => live.select_point_with_cx(pt, tol, &mut rows, cx),
                        Engine::Ifc(_) => return Err(Error::Unsupported),
                    };
                    if res.is_err() {
                        assert_eq!(rows, [RowId::NONE], "停止した点選択が出力を汚した");
                    }
                    res?;
                    assert!(
                        rows.get(1..)
                            .unwrap_or(&[])
                            .windows(2)
                            .all(|w| matches!(w, [a, b] if a < b)),
                        "点選択の結果が狭義昇順でない"
                    );
                    bounded_debug(&rows, cx)
                }) {
                    self.bytes.push(result);
                }
            }
        }
    }

    /// `ZLDP` の全体または 64 行ブロック差分パケットを符号化・検証・ミラーへ反映し、
    /// 反映後の 13 列が新規の全量パケットと完全に一致することを確かめる
    fn sync_display(&mut self, engine: &mut Engine, mirror: &mut DisplayMirror, seed: u64) {
        let Some(display) = engine.display() else {
            return;
        };
        if let Some(dirty_syms) = engine.dirty_symbols() {
            self.bytes.push(format!("{dirty_syms:?}").into_bytes());
        }
        let full = !mirror.synced;
        // まず小さい予算と中断で `encode` を呼び、失敗しても変更追跡フラグが失われないことを確かめる
        for (small_limits, cancel) in [
            (self.limits.with(Kind::Work, Limit::Finite(seed % 8)), false),
            (
                self.limits
                    .with(Kind::DecodedBytes, Limit::Finite(seed % 32)),
                false,
            ),
            (self.limits.with(Kind::Records, Limit::Finite(0)), false),
            (self.limits, true),
        ] {
            let _ = self.call(small_limits, cancel, |cx| {
                display_wire::encode(display, full, cx).map_err(wire_to_sdk_err)
            });
        }

        let Some(display) = engine.display() else {
            return;
        };
        let encoded = self.call(self.limits, false, |cx| {
            let bytes = display_wire::encode(display, full, cx).map_err(wire_to_sdk_err)?;
            let parsed = Packet::parse(&bytes, cx).map_err(wire_to_sdk_err)?;
            assert_eq!(parsed.full(), full);
            Ok(bytes)
        });
        let Ok(packet_bytes) = encoded else {
            mirror.synced = false;
            return;
        };
        self.bytes.push(packet_bytes.clone());

        let mut parse_budget = Budget::new(zumen_sdk::testkit::fuzz_call_limits());
        let mut parse_diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let packet = Packet::parse(
            &packet_bytes,
            &mut Cx::new(&mut parse_budget, &mut parse_diag, &cancel),
        )
        .expect("検査済みの ZLDP パケットが再読できない");
        mirror.apply_packet(&packet);
        engine.clear_dirty();
        engine.clear_dirty_symbols();

        // 差分で同期したミラーが、今の表示から作った全量パケットと全 13 列でバイト一致することを確かめる
        if !full && let Some(current_display) = engine.display() {
            let full_check = self.call(self.limits, false, |cx| {
                display_wire::encode(current_display, true, cx).map_err(wire_to_sdk_err)
            });
            if let Ok(full_bytes) = full_check {
                let mut fb = Budget::new(zumen_sdk::testkit::fuzz_call_limits());
                let mut fd = Diagnostics::new();
                let full_pkt = Packet::parse(&full_bytes, &mut Cx::new(&mut fb, &mut fd, &cancel))
                    .expect("全量 ZLDP パケットが再読できない");
                let mut fresh = DisplayMirror::default();
                fresh.apply_packet(&full_pkt);
                assert!(
                    mirror == &fresh,
                    "64 行ブロック差分を反映したミラーが全量表示と一致しない"
                );
            }
        }
    }

    /// 直前に変更フラグを消去した状態で編集・履歴操作が失敗または停止したとき、
    /// 表示の変更ブロックと `dirty_symbols` が空のまま保たれていることを確かめる
    fn assert_display_clean(&mut self, engine: &Engine) {
        if let Some(dirty_syms) = engine.dirty_symbols() {
            assert!(
                dirty_syms.is_empty(),
                "失敗・停止した操作が dirty_symbols を汚した"
            );
        }
        let Some(display) = engine.display() else {
            return;
        };
        if let Ok(delta_bytes) = self.call(self.limits, false, |cx| {
            display_wire::encode(display, false, cx).map_err(wire_to_sdk_err)
        }) {
            let mut b = Budget::new(zumen_sdk::testkit::fuzz_call_limits());
            let mut d = Diagnostics::new();
            let cancel = CancelToken::new();
            let pkt = Packet::parse(&delta_bytes, &mut Cx::new(&mut b, &mut d, &cancel))
                .expect("空差分の ZLDP が読めない");
            for col in pkt.columns() {
                assert_eq!(
                    col.blocks().count(),
                    0,
                    "失敗・停止した操作が表示の変更フラグを汚した"
                );
            }
        }
    }

    fn stopped_history(&mut self, engine: &mut Engine, mirror_synced: bool, redo: bool) {
        let before_state = engine.state();
        let before = self.snapshot(engine);
        let zero = self.limits.with(Kind::Work, Limit::Finite(0));
        for (limits, cancel) in [(zero, false), (self.limits, true)] {
            let result = self.call(limits, cancel, |cx| {
                if redo {
                    engine.redo(cx)
                } else {
                    engine.undo(cx)
                }
            });
            assert!(result.is_err(), "停止を指定した履歴操作が成功した");
            assert_eq!(
                engine.state(),
                before_state,
                "停止した履歴操作が版か履歴を変えた"
            );
            assert!(
                self.snapshot(engine) == before,
                "停止した履歴操作が保持モデルか保存を変えた"
            );
            if mirror_synced {
                self.assert_display_clean(engine);
            }
        }
    }
}

fn wire_to_sdk_err(err: display_wire::Error) -> Error {
    match err {
        display_wire::Error::Invalid => Error::Invalid,
        display_wire::Error::Stopped(stop) => Error::Stopped(stop),
    }
}

/// 1 操作の入れ子は同じ Cx に通し、別の操作と検証用の取得には新しい有限予算を使う。
/// 保存・保持モデル・表示差分・診断・使用量・編集結果を呼び出し順で戻す。
pub fn run(data: &[u8], format: Format, limits: Limits, threads: usize) -> Vec<Vec<u8>> {
    let limits = limits
        .with(Kind::DecodedBytes, Limit::Finite(4 << 20))
        .with(Kind::Elements, Limit::Finite(4096))
        .with(Kind::Work, Limit::Finite(1 << 18));
    let mut trace = Trace {
        limits,
        bytes: Vec::new(),
    };
    let zero = limits.with(Kind::Work, Limit::Finite(0));
    for (open_limits, cancel) in [(zero, false), (limits, true)] {
        let _ = trace.call(open_limits, cancel, |cx| {
            Engine::open(format, data, threads, cx)
        });
    }
    let Ok(mut engine) = trace.call(limits, false, |cx| Engine::open(format, data, threads, cx))
    else {
        return trace.bytes;
    };
    let mut seed = data
        .iter()
        .step_by((data.len() / 64).max(1))
        .fold(1_u64, |value, byte| {
            value.wrapping_mul(257).wrapping_add(u64::from(*byte))
        });

    let mut mirror = DisplayMirror::default();
    trace.sync_display(&mut engine, &mut mirror, seed);

    for mode in 0..4 {
        let before_state = engine.state();
        let before = trace.snapshot(&engine);
        let clean_before = mirror.synced;
        trace.row_targets(&engine, seed);
        trace.spatial_queries(&engine, seed);
        trace.ipc_reads(&engine, seed);
        let edit_limits = if mode == 2 {
            limits.with(Kind::Work, Limit::Finite(seed % 128))
        } else {
            limits
        };
        let prepared = trace.call(limits, false, |cx| engine.prepare(seed, cx));
        let edited = prepared.and_then(|prepared| {
            trace.call(edit_limits, mode == 3, |cx| engine.edit(&prepared, cx))
        });
        if let Ok(result) = &edited {
            trace.bytes.push(result.clone());
        }
        let after_state = engine.state();
        let after = trace.snapshot(&engine);
        trace.row_targets(&engine, seed);
        trace.spatial_queries(&engine, seed);
        trace.ipc_reads(&engine, seed);
        trace
            .bytes
            .push(format!("{before_state:?}\n{after_state:?}").into_bytes());
        if edited.is_err() {
            assert!(before_state == after_state, "停止した編集が版を変えた");
            assert!(before == after, "停止した編集が保持モデルか保存を変えた");
            if clean_before {
                trace.assert_display_clean(&engine);
            }
        } else if before_state.0 != after_state.0 {
            trace.sync_display(&mut engine, &mut mirror, seed);
            trace.stopped_history(&mut engine, mirror.synced, false);
            let undo = trace.call(limits, false, |cx| engine.undo(cx));
            let undone = trace.snapshot(&engine);
            match undo {
                Ok(true) => {
                    assert!(before == undone, "取り消しが保持モデルか保存を戻さない");
                    assert_eq!(engine.state().0, before_state.0);
                    trace.sync_display(&mut engine, &mut mirror, seed.rotate_left(7));
                    trace.stopped_history(&mut engine, mirror.synced, true);
                    let undo_state = engine.state();
                    let redo = trace.call(limits, false, |cx| engine.redo(cx));
                    let redone = trace.snapshot(&engine);
                    if matches!(redo, Ok(true)) {
                        assert!(after == redone, "やり直しが保持モデルか保存を戻さない");
                        assert_eq!(engine.state().0, after_state.0);
                        trace.sync_display(&mut engine, &mut mirror, seed.rotate_left(11));
                    } else if redo.is_err() {
                        assert_eq!(engine.state(), undo_state);
                        assert!(undone == redone, "停止したやり直しが状態を変えた");
                        if mirror.synced {
                            trace.assert_display_clean(&engine);
                        }
                    } else {
                        panic!("やり直しの履歴がない");
                    }
                }
                Err(_) => {
                    assert_eq!(engine.state(), after_state);
                    assert!(after == undone, "停止した取り消しが状態を変えた");
                    if mirror.synced {
                        trace.assert_display_clean(&engine);
                    }
                }
                Ok(false) => panic!("取り消しの履歴がない"),
            }
        } else {
            assert_eq!(
                before_state, after_state,
                "変更のない編集が版か履歴を変えた"
            );
            assert!(before == after, "変更のない編集が保持モデルか保存を変えた");
            if clean_before {
                trace.assert_display_clean(&engine);
            }
        }
        seed = seed.rotate_left(13).wrapping_add(0x9E37_79B9);
    }
    trace.bytes
}
