//! 常駐文書の読み込み・差分編集・履歴を、小さい操作ごとの共有予算で繰り返す。

use std::fmt::{self, Write};

use zumen_acad::AcadLive;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_drawing::{Affine, Changes, Drawing2D, Edit, Hash, Placed, Position, Restyle, Seen, id};
use zumen_ifc::IfcLive;
use zumen_jww::JwwLive;
use zumen_sdk::Error;
use zumen_sxf::SxfLive;

/// 形式ごとの入口。SXF は SFC・P21 と圧縮された入力を含む。
#[derive(Clone, Copy)]
pub enum Format {
    Dxf,
    Dwg,
    Jww,
    Sxf,
    Ifc,
}

enum Engine {
    Acad(Box<AcadLive>),
    Jww(Box<JwwLive>),
    Sxf(Box<SxfLive>),
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
            Format::Sxf => {
                SxfLive::open_with_threads(data, threads, cx).map(|v| Self::Sxf(Box::new(v)))
            }
            Format::Ifc => {
                IfcLive::open_with_threads(data, threads, cx).map(|v| Self::Ifc(Box::new(v)))
            }
        }
    }

    fn state(&self) -> (u64, u64, usize, usize) {
        match self {
            Self::Acad(v) => (v.state_id(), v.revision().0, v.undo_len(), v.redo_len()),
            Self::Jww(v) => (v.state_id(), v.revision().0, v.history_bytes(), 0),
            Self::Sxf(v) => (v.state_id(), v.revision().0, v.undo_len(), v.redo_len()),
            Self::Ifc(v) => (v.state_id(), v.revision().0, v.undo_len(), v.redo_len()),
        }
    }

    fn drawing(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        match self {
            Self::Acad(v) => v.drawing(cx),
            Self::Jww(v) => v.drawing(cx),
            Self::Sxf(v) => v.drawing(cx),
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
            Self::Sxf(v) => v.save(cx),
            Self::Ifc(v) => v.save(cx),
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
        let Some(target) = first_target(&drawing) else {
            return Ok(Prepared::Empty);
        };
        let offset = match (seed >> 8) % 5 {
            0 => f64::NAN,
            1 => f64::INFINITY,
            _ => ((seed >> 16) as i8 as f64) / 16.0,
        };
        let edit = match seed % 5 {
            0 => Edit::Transform {
                target: target.seen,
                transform: Affine::translation(offset, -0.75),
            },
            1 => Edit::Restyle {
                target: target.seen,
                style: Restyle {
                    line_type_scale: Some(offset),
                    ..Restyle::default()
                },
            },
            2 => Edit::Replace {
                target: target.seen,
                element: Box::new(target.element),
            },
            3 => Edit::Add {
                place: target.place,
                element: Box::new(target.element),
            },
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
            Self::Sxf(v) => {
                let result = v.apply(changes, cx)?;
                Ok(format!("{result:?}").into_bytes())
            }
            Self::Ifc(_) => Err(Error::Unsupported),
        }
    }

    fn undo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        match self {
            Self::Acad(v) => v.undo(cx),
            Self::Jww(v) => v.undo(cx),
            Self::Sxf(v) => v.undo(cx),
            Self::Ifc(v) => v.undo(cx),
        }
    }

    fn redo(&mut self, cx: &mut Cx<'_>) -> Result<bool, Error> {
        match self {
            Self::Acad(v) => v.redo(cx),
            Self::Jww(v) => v.redo(cx),
            Self::Sxf(v) => v.redo(cx),
            Self::Ifc(v) => v.redo(cx),
        }
    }
}

struct Target {
    seen: Seen,
    place: String,
    element: Placed,
}

fn first_target(drawing: &Drawing2D) -> Option<Target> {
    let result: Result<(), Box<Target>> = id::each_element(drawing, |position, name, element| {
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
        Err(Box::new(Target {
            seen,
            place,
            element,
        }))
    });
    result.err().map(|target| *target)
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

    fn snapshot(&mut self, engine: &Engine) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        (
            self.blob(|cx| engine.save(cx)),
            self.blob(|cx| engine.model(cx)),
        )
    }

    fn row_targets(&mut self, engine: &Engine, seed: u64) {
        let Engine::Acad(live) = engine else {
            return;
        };
        let rows = [0, (seed % live.row_count().max(1) as u64) as u32, u32::MAX];
        let work = self.limits.with(Kind::Work, Limit::Finite(0));
        let bytes = self.limits.with(Kind::DecodedBytes, Limit::Finite(0));
        for row in rows {
            for (limits, cancel) in [
                (self.limits, false),
                (work, false),
                (bytes, false),
                (self.limits, true),
            ] {
                if let Ok(result) = self.call(limits, cancel, |cx| {
                    let target = live.seen_of_row(zumen_live::RowId(row), cx)?;
                    bounded_debug(&target, cx)
                }) {
                    self.bytes.push(result);
                }
            }
        }
    }

    fn rectangle_queries(&mut self, engine: &Engine, seed: u64) {
        if !matches!(engine, Engine::Acad(_) | Engine::Jww(_)) {
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
                if let Ok(result) = self.call(limits, cancel, |cx| match engine {
                    Engine::Acad(live) => {
                        let mut rows = vec![zumen_live::RowId::NONE];
                        let result = live.select_rect_with_cx(rect, &mut rows, cx);
                        if result.is_err() {
                            assert_eq!(rows, [zumen_live::RowId::NONE]);
                        }
                        result?;
                        bounded_debug(&rows, cx)
                    }
                    Engine::Jww(live) => {
                        let ids = live.select_sheet(rect, cx)?;
                        bounded_debug(&ids, cx)
                    }
                    _ => Err(Error::Unsupported),
                }) {
                    self.bytes.push(result);
                }
            }
        }
    }

    fn stopped_history(&mut self, engine: &mut Engine, redo: bool) {
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
        }
    }
}

/// 1 操作の入れ子は同じ Cx に通し、別の操作と検証用の取得には新しい有限予算を使う。
/// 保存・保持モデル・診断・使用量・編集結果を呼び出し順で戻す。
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
    for mode in 0..4 {
        let before_state = engine.state();
        let before = trace.snapshot(&engine);
        trace.row_targets(&engine, seed);
        trace.rectangle_queries(&engine, seed);
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
        trace.rectangle_queries(&engine, seed);
        trace
            .bytes
            .push(format!("{before_state:?}\n{after_state:?}").into_bytes());
        if edited.is_err() {
            assert!(before_state == after_state, "停止した編集が版を変えた");
            assert!(before == after, "停止した編集が保持モデルか保存を変えた");
        } else if before_state.0 != after_state.0 {
            trace.stopped_history(&mut engine, false);
            let undo = trace.call(limits, false, |cx| engine.undo(cx));
            let undone = trace.snapshot(&engine);
            match undo {
                Ok(true) => {
                    assert!(before == undone, "取り消しが保持モデルか保存を戻さない");
                    assert_eq!(engine.state().0, before_state.0);
                    trace.stopped_history(&mut engine, true);
                    let undo_state = engine.state();
                    let redo = trace.call(limits, false, |cx| engine.redo(cx));
                    let redone = trace.snapshot(&engine);
                    if matches!(redo, Ok(true)) {
                        assert!(after == redone, "やり直しが保持モデルか保存を戻さない");
                        assert_eq!(engine.state().0, after_state.0);
                    } else if redo.is_err() {
                        assert_eq!(engine.state(), undo_state);
                        assert!(undone == redone, "停止したやり直しが状態を変えた");
                    } else {
                        panic!("やり直しの履歴がない");
                    }
                }
                Err(_) => {
                    assert_eq!(engine.state(), after_state);
                    assert!(after == undone, "停止した取り消しが状態を変えた");
                }
                Ok(false) => panic!("取り消しの履歴がない"),
            }
        } else {
            assert_eq!(
                before_state, after_state,
                "変更のない編集が版か履歴を変えた"
            );
            assert!(before == after, "変更のない編集が保持モデルか保存を変えた");
        }
        seed = seed.rotate_left(13).wrapping_add(0x9E37_79B9);
    }
    trace.bytes
}
