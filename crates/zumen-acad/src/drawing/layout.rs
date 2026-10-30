//! レイアウトの用紙。ペーパー空間のブロックの中身とビューポートを、LAYOUT ごとの用紙に置く。
//! 描く順は SORTENTSTABLE から決める。

use std::collections::BTreeMap;

use super::{
    EMPTY, Home, LayoutInfo, Lifted, Lifter, MODEL, Out, PaperFrame, SortKeys, SpaceKeys, amount,
    arrange, attached, invalid, source, text, work,
};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use zumen_base::{Cx, Diagnostic, Ref, codes};
use zumen_drawing::{Affine, PaperLength, PaperPoint, Sheet, SheetId, View};
use zumen_sdk::Error;

impl Lifter<'_> {
    pub(super) fn layout_of<'a>(&self, db: &'a AcadDb, record: u64) -> Option<(usize, &'a Object)> {
        let index = *self.layouts.get(&record)?;
        Some((index, db.objects().get(index)?))
    }

    /// LAYOUT の名前・タブの順・範囲。読めない値は、ないものとする
    pub(super) fn layout_info(&self, layout: &Object) -> LayoutInfo {
        let name = layout
            .values
            .iter()
            .rfind(|value| value.code == 1)
            .and_then(|value| self.decode(&layout.kind, 1, &value.data).ok());
        let real = |code| self.real(layout, code, None).ok();
        let limits = (|| Some([[real(10)?, real(20)?], [real(11)?, real(21)?]]))();
        LayoutInfo {
            name,
            tab: self.int(layout, 71, 0).unwrap_or(0),
            limits,
        }
    }

    /// ほかのレイアウトのペーパー空間のブロックの中身を、1 枚の用紙にする。用紙の原点と大きさは LAYOUT の
    /// 範囲で決め、なければ原点を 0 とし、大きさを決めない（推定。docs/formats/dxf.md）。中身がなければ、
    /// LAYOUT があるときだけ、中身のない用紙を作る
    pub(super) fn layout(
        &self,
        db: &AcadDb,
        record: Option<&Object>,
        members: &[&Object],
        keys: &BTreeMap<u64, SortKeys>,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        let handle = record.and_then(Object::handle);
        let layout = handle.and_then(|handle| self.layout_of(db, handle));
        let info = layout.map(|(_, layout)| self.layout_info(layout));
        let limits = info.as_ref().and_then(|info| info.limits);
        let id = SheetId(out.filled_layouts.saturating_add(1));
        let lifter = Lifter {
            sheet: id,
            origin: limits.map_or([0.0, 0.0], |[min, _]| min),
            ..self.clone()
        };
        let paper = std::mem::take(&mut out.paper);
        let views = std::mem::take(&mut out.views);
        let sheet = std::mem::take(&mut out.sheet);
        let (attached, owned) = attached(members);
        for (index, object) in members.iter().enumerate() {
            let source = source(object);
            work(cx, amount(object), &source)?;
            if owned.contains(&index) {
                continue;
            }
            let inserted: Vec<&Object> = attached
                .get(&index)
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .filter_map(|at| members.get(*at).copied())
                .collect();
            if let Err(diagnostic) =
                lifter.place(db, object, source.clone(), &inserted, true, out, cx)
            {
                cx.report((*diagnostic).at(source));
            }
        }
        let elements = std::mem::replace(&mut out.paper, paper);
        let views = std::mem::replace(&mut out.views, views);
        let items = std::mem::replace(&mut out.sheet, sheet);
        // 中身のないレイアウトも、LAYOUT があれば、名前と大きさだけの用紙にする。ID は後で付ける
        let empty = elements.is_empty() && views.is_empty();
        if empty && layout.is_none() {
            return Ok(());
        }
        let size = match (limits, self.mm) {
            (Some([[x0, y0], [x1, y1]]), Some(mm)) => {
                let size = [(x1 - x0) * mm, (y1 - y0) * mm];
                size.iter().all(|s| *s > 0.0).then(|| size.map(PaperLength))
            }
            _ => None,
        };
        let order = arrange(handle.and_then(|handle| keys.get(&handle)), items, cx);
        let (name, tab) = info.map_or((None, 0), |info| (info.name, info.tab));
        if !empty {
            out.filled_layouts = id.0;
        }
        // 中身のない用紙に足す要素は、レイアウトのブロックのレコードを所有者とし、ペーパー空間の印を付ける。
        // 今のレイアウト（`*Paper_Space`）のものは ENTITIES の所に、ほかのレイアウトのものはそのブロックに置く
        let frame = match (record, self.mm) {
            (Some(record), Some(mm)) if empty => {
                let current = matches!(record.first(2), Some(Data::Text(name)) if name.eq_ignore_ascii_case(b"*Paper_Space"));
                Some(PaperFrame {
                    origin: if current { self.origin } else { lifter.origin },
                    mm,
                    home: Home {
                        owner: handle,
                        blocks: !current,
                        paper: true,
                    },
                })
            }
            _ => None,
        };
        out.layouts.push((
            tab,
            layout.map(|(index, _)| index),
            Sheet {
                id: if empty { EMPTY } else { id },
                name,
                size,
                elements,
                views,
                order,
                source: layout.map_or_else(Ref::default, |(_, layout)| source(layout)),
            },
            frame,
        ));
        Ok(())
    }

    /// ペーパー空間のビューポート（DXF Reference「VIEWPORT」）。ねじれ・視点の向き・注視点のないものだけを扱う
    pub(super) fn view(&self, object: &Object, mm: f64, source: Ref) -> Lifted<View> {
        let kind = object.kind.as_slice();
        let real = |code: i16, default: Option<f64>| self.real(object, code, default);
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
            sheet: self.sheet,
            x: (px + dx * width / 2.0 - ox) * mm,
            y: (py + dy * height / 2.0 - oy) * mm,
        };
        Ok(View {
            space: MODEL,
            transform,
            sheet: self.sheet,
            clip: vec![
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ],
            hidden_layers: Vec::new(),
            source,
        })
    }

    pub(super) fn paper_mm(&self, kind: &[u8]) -> Lifted<f64> {
        self.mm.ok_or_else(|| {
            Box::new(Diagnostic::new(codes::DXF_PAPER_SPACE_DROPPED).with("kind", text(kind)))
        })
    }

    /// モデル空間とペーパー空間の SORTENTSTABLE と、ほかのブロックのレコードのハンドルごとの SORTENTSTABLE
    pub(super) fn sort_keys(&self, db: &AcadDb, cx: &mut Cx) -> Result<SpaceKeys, Error> {
        let mut keys = [None, None];
        let mut blocks: BTreeMap<u64, SortKeys> = BTreeMap::new();
        for object in db.objects() {
            if object.section != Section::Objects || object.kind != b"SORTENTSTABLE" {
                continue;
            }
            let source = source(object);
            work(cx, 1, &source)?;
            match self.sortents(db, object) {
                Ok((record, space, sort)) => {
                    work(cx, sort.len() as u64, &source)?;
                    let all = match space {
                        Some(index) => keys.get_mut(index).map(|slot| {
                            &mut slot.get_or_insert_with(|| (source, BTreeMap::new())).1
                        }),
                        None => Some(
                            &mut blocks
                                .entry(record)
                                .or_insert_with(|| (source, BTreeMap::new()))
                                .1,
                        ),
                    };
                    if let Some(all) = all {
                        all.extend(sort);
                    }
                }
                Err(diagnostic) => cx.report((*diagnostic).at(source)),
            }
        }
        Ok((keys, blocks))
    }

    /// SORTENTSTABLE の、指すブロックのレコードのハンドルと空間（0 はモデル空間、1 はペーパー空間、ほかは
    /// `None`）と、図形のハンドル（331）から並べ替えのハンドル（5）への対応（DXF Reference「SORTENTSTABLE」）
    fn sortents(
        &self,
        db: &AcadDb,
        object: &Object,
    ) -> Lifted<(u64, Option<usize>, BTreeMap<u64, u64>)> {
        let kind = object.kind.as_slice();
        // DWG の表は区切りの 100 を持たないので、所有者（初めの 330）の後から読む
        let start = object
            .values
            .iter()
            .position(|value| {
                matches!((value.code, &value.data), (100, Data::Text(text)) if text == b"AcDbSortentsTable")
            })
            .or_else(|| match object.origin {
                Origin::Dwg { .. } => object.values.iter().position(|value| value.code == 330),
                _ => None,
            })
            .ok_or_else(|| invalid(kind, 100))?;
        let mut rest = object.values.iter().skip(start + 1);
        let (record, block) = match rest.next() {
            Some(Value {
                code: 330,
                data: Data::Handle(handle),
            }) => (
                *handle,
                db.get(*handle)
                    .filter(|record| record.kind == b"BLOCK_RECORD")
                    .ok_or_else(|| invalid(kind, 330))?,
            ),
            _ => return Err(invalid(kind, 330)),
        };
        let name = self.string(block, 2)?.unwrap_or_default();
        let space = ["*Model_Space", "*Paper_Space"]
            .iter()
            .position(|space| name.eq_ignore_ascii_case(space));
        let mut sort = BTreeMap::new();
        while let Some(value) = rest.next() {
            let (331, Data::Handle(entity)) = (value.code, &value.data) else {
                return Err(invalid(kind, value.code));
            };
            let Some(Value {
                code: 5,
                data: Data::Handle(key),
            }) = rest.next()
            else {
                return Err(invalid(kind, 5));
            };
            sort.insert(*entity, *key);
        }
        Ok((record, space, sort))
    }
}
