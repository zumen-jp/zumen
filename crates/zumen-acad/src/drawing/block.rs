//! BLOCKS の BLOCK を記号の定義にし（決定 0112）、INSERT を記号の配置に、ATTRIB と ATTDEF を属性にする。
//! INSERT の切り抜き（XCLIP）は、拡張辞書の SPATIAL_FILTER から読む（決定 0149）。

use std::borrow::Cow;
use std::collections::BTreeMap;

use super::extension::extension_work;
use super::{
    BLOCK, Lifted, Lifter, Out, SortKeys, amount, arrange, attached, insertion_units, invalid,
    lift_err, single, source, text, unit_of, work,
};
use crate::db::{AcadDb, Data, Object, Section};
use zumen_base::{Cx, Diagnostic, codes};
use zumen_drawing::{
    Attribute, AttributeDefinition, AttributeValue, Axes, Clip, Grid, Length, ModelLength,
    ModelPoint, ModelSpace, Position, SpaceId, Symbol, SymbolDefinition, SymbolKey,
};
use zumen_sdk::Error;

impl Lifter<'_> {
    /// INSERT の切り抜き（XCLIP、決定 0149・決定 0208）
    pub(super) fn symbol_clip(&self, db: &AcadDb, object: &Object) -> Lifted<Option<Clip>> {
        single::symbol_clip(&object.values, |handle| {
            let obj = db.get(handle)?;
            Some((
                Cow::Borrowed(obj.kind.as_slice()),
                Cow::Borrowed(obj.values.as_slice()),
            ))
        })
        .map_err(|code| {
            Box::new(Diagnostic::new(codes::DXF_CLIP_DROPPED).with("code", i64::from(code)))
        })
    }

    /// BLOCKS のセクションの BLOCK を、記号の定義にする（決定 0112）。中身は、所有者（330）をたどって着くブロックの
    /// レコードの BLOCK のもの、所有者が分からなければ、並びの中で前の BLOCK から ENDBLK までのものとする。
    /// 名前は BLOCK の 2、なければブロックのレコードの 2。基点は 10、フラグ（70）の 1 は名前のない定義、4 は外部参照
    /// で、そのパス（1）を持つ。名前が `*` で始まるものも名前のない定義とする（推定）。
    /// 空間のブロック（`*Model_Space`・`*Paper_Space`）は移さない。中身の描く順は、ブロックのレコードを指す
    /// SORTENTSTABLE（`keys`。ブロックのレコードのハンドルごと）で決め、なければ入力の順とする
    pub(super) fn definitions(
        &self,
        db: &AcadDb,
        keys: &BTreeMap<u64, SortKeys>,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Result<Vec<SymbolDefinition>, Error> {
        let objects: Vec<&Object> = db
            .objects()
            .iter()
            .filter(|object| object.section == Section::Blocks)
            .collect();
        let starts: Vec<usize> = objects
            .iter()
            .enumerate()
            .filter(|(_, object)| object.kind == b"BLOCK")
            .map(|(index, _)| index)
            .collect();
        let by_record: BTreeMap<u64, usize> = starts
            .iter()
            .enumerate()
            .filter_map(|(at, index)| Some((objects.get(*index)?.owner()?, at)))
            .collect();
        let owners: BTreeMap<u64, Option<u64>> = objects
            .iter()
            .filter_map(|object| Some((object.handle()?, object.owner())))
            .collect();
        let mut members: Vec<Vec<&Object>> = vec![Vec::new(); starts.len()];
        let mut open = None;
        for object in &objects {
            work(cx, 1, &source(object))?;
            match object.kind.as_slice() {
                b"BLOCK" => {
                    open = starts.iter().position(|index| {
                        objects
                            .get(*index)
                            .is_some_and(|o| std::ptr::eq(*o, *object))
                    });
                    continue;
                }
                b"ENDBLK" => {
                    open = None;
                    continue;
                }
                _ => {}
            }
            // 所有者が図形（ATTRIB の INSERT など）なら、その所有者へ。輪は数えて止める
            let mut owner = object.owner();
            let mut at = None;
            for _ in 0..=owners.len() {
                let Some(handle) = owner else { break };
                if let Some(found) = by_record.get(&handle) {
                    at = Some(*found);
                    break;
                }
                owner = owners.get(&handle).copied().flatten();
            }
            match at.or(open).and_then(|at| members.get_mut(at)) {
                Some(group) => group.push(object),
                None => cx.report((*invalid(&object.kind, 330)).at(source(object))),
            }
        }
        let mut symbols = Vec::new();
        for (at, (index, members)) in starts.iter().zip(members).enumerate() {
            let Some(block) = objects.get(*index).copied() else {
                continue;
            };
            let space = SpaceId(u32::try_from(at).unwrap_or(u32::MAX).saturating_add(1));
            if let Some(symbol) = self.definition(db, block, space, &members, keys, out, cx)? {
                symbols.push(symbol);
            }
        }
        Ok(symbols)
    }

    /// BLOCK と中身の図形を、空間 `space` の記号の定義にする。空間のブロックなら `None`。中身の図形のうち
    /// 移せないものは、その図形を指す診断を返し、定義は移す
    #[allow(clippy::too_many_arguments)]
    fn definition(
        &self,
        db: &AcadDb,
        block: &Object,
        space: SpaceId,
        members: &[&Object],
        keys: &BTreeMap<u64, SortKeys>,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Result<Option<SymbolDefinition>, Error> {
        let kind = block.kind.as_slice();
        let record = block
            .owner()
            .and_then(|owner| db.get(owner))
            .filter(|record| record.kind == b"BLOCK_RECORD");
        let name = match self.string(block, 2) {
            Ok(Some(name)) => Some(name),
            Ok(None) => record.and_then(|record| self.string(record, 2).ok().flatten()),
            Err(diagnostic) => {
                cx.report((*diagnostic).at(source(block)));
                return Ok(None);
            }
        };
        let Some(name) = name else {
            cx.report((*invalid(kind, 2)).at(source(block)));
            return Ok(None);
        };
        if layout_block(&name) {
            if paper_layout(&name) {
                self.layout(db, record, members, keys, out, cx)?;
            }
            return Ok(None);
        }
        let point = |x: f64, y: f64| ModelPoint { space, x, y };
        let length = |value: f64| Length::Model(ModelLength { space, value });
        let head = (|| -> Lifted<_> {
            let base = match block.first(10) {
                Some(_) => Some(self.planar_point(block, 10, &point)?),
                None => None,
            };
            let flags = self.int(block, 70, 0)?;
            let external = if flags & 4 != 0 {
                Some(self.string(block, 1)?.unwrap_or_default())
            } else {
                None
            };
            Ok((base, flags & 1 != 0 || name.starts_with('*'), external))
        })();
        let (base, anonymous, external) = match head {
            Ok(head) => head,
            Err(diagnostic) => {
                cx.report((*diagnostic).at(source(block)));
                return Ok(None);
            }
        };
        let (attached, owned) = attached(members);
        let mut elements = Vec::new();
        let mut handles = Vec::new();
        let mut attributes = Vec::new();
        for (index, object) in members.iter().enumerate() {
            let source = source(object);
            work(cx, amount(object), &source)?;
            if owned.contains(&index) {
                continue;
            }
            extension_work(db, object, cx)?;
            let lifted = match object.kind.as_slice() {
                b"ATTDEF" => self
                    .attribute_definition(object, &point, &length, out)
                    .map(|definition| attributes.push(definition)),
                b"LINE" | b"CIRCLE" | b"ARC" | b"ELLIPSE" | b"LWPOLYLINE" | b"SPLINE" | b"TEXT"
                | b"MTEXT" | b"DIMENSION" | b"ARC_DIMENSION" | b"LEADER" | b"MULTILEADER"
                | b"INSERT" | b"HATCH" | b"ACAD_TABLE" | b"IMAGE" | b"PDFUNDERLAY"
                | b"DWFUNDERLAY" | b"DGNUNDERLAY" | b"POLYLINE" | b"SOLID" | b"TRACE"
                | b"POINT" => {
                    let inserted: Vec<&Object> = attached
                        .get(&index)
                        .map_or(&[][..], Vec::as_slice)
                        .iter()
                        .filter_map(|at| members.get(*at).copied())
                        .collect();
                    self.element(
                        db,
                        object,
                        source.clone(),
                        &inserted,
                        point,
                        length,
                        out,
                        cx,
                    )
                    .map(|element| {
                        let at = Position::Model {
                            space,
                            index: elements.len(),
                        };
                        self.extend(db, object, at, out, cx);
                        elements.push(element);
                        handles.push(object.handle());
                    })
                }
                kind => Err(Box::new(
                    Diagnostic::new(codes::DXF_LIFT_DROPPED).with("kind", text(kind)),
                )),
            };
            if let Err(diagnostic) = lifted {
                cx.report((*diagnostic).at(source));
            }
        }
        let keys = record
            .and_then(Object::handle)
            .and_then(|handle| keys.get(&handle));
        let order = arrange(keys, handles.into_iter().enumerate().collect(), cx);
        // ブロックのレコードの挿入単位。0（単位なし）か、なければ文書の単位とする。配置の尺度は、
        // 単位の違いを掛けた値を持つ（推定。決定 0144）
        let unit = match record.and_then(insertion_units) {
            Some(code) if code != 0 => unit_of(code),
            _ => self.unit,
        };
        Ok(Some(SymbolDefinition {
            key: SymbolKey {
                namespace: BLOCK.into(),
                name,
            },
            space: ModelSpace {
                id: space,
                unit,
                axes: Axes::EastNorth,
                order,
                elements,
            },
            base,
            attributes,
            anonymous,
            external,
            source: source(block),
        }))
    }

    /// ATTDEF（DXF Reference「ATTDEF」）。ATTRIB と同じフィールドに、値を尋ねる文（3）を持つ。フラグ（70）の 1 は表示しない、
    /// 2 は値が決まっている、4 は確かめる、8 は尋ねない
    pub(super) fn attribute_definition(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> ModelPoint,
        length: &impl Fn(f64) -> Length,
        out: &mut Out,
    ) -> Lifted<AttributeDefinition> {
        let attribute = self.attribute(object, point, length, out)?;
        let flags = self.int(object, 70, 0)?;
        Ok(AttributeDefinition {
            attribute,
            prompt: self.string(object, 3)?.unwrap_or_default(),
            constant: flags & 2 != 0,
            verify: flags & 4 != 0,
            preset: flags & 8 != 0,
        })
    }

    /// INSERT（DXF Reference「INSERT」）。尺度（41・42）が 0 なら読めないとする。Z の尺度（43）は平面の形を
    /// 変えないので移さない。列と行（70・71）が 1 より多ければ、格子に並べる（MINSERT）。属性は後で足す
    pub(super) fn symbol<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Symbol<P>> {
        let kind = object.kind.as_slice();
        let raw = single::insert_geom(&object).map_err(|err| lift_err(kind, err))?;
        let name = self.decode_bytes(kind, 2, raw.name)?;
        let [columns, rows] = raw.grid;
        let grid = if (columns, rows) == (1, 1) {
            None
        } else {
            Some(Grid {
                columns,
                rows,
                column_spacing: length(raw.spacing[0]),
                row_spacing: length(raw.spacing[1]),
            })
        };
        Ok(Symbol {
            key: SymbolKey {
                namespace: BLOCK.into(),
                name,
            },
            position: point(raw.position[0], raw.position[1]),
            scale: raw.scale,
            rotation: raw.rotation,
            grid,
            attributes: Vec::new(),
            clip: None,
        })
    }

    /// ATTRIB（DXF Reference「ATTRIB」）。タグ（2）と、TEXT と同じフィールドの値を移し、70 の 1 なら表示しないとする。
    /// 複数行の属性は、埋め込みのオブジェクトのマーカー（101）の後の MTEXT の値も移す。マーカーの前の、最後の 71（属性の
    /// 種類）と 72 の組は、TEXT のフィールドと取り違えないよう、1 行の値から除く（決定 0102）
    pub(super) fn attribute<P: zumen_drawing::transform::Point>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
        out: &mut Out,
    ) -> Lifted<Attribute<P>> {
        let converted = super::Converted::default();
        let point = &|x, y| converted.point(point(x, y));
        let length = &|value| converted.length(length(value));
        let kind = object.kind.as_slice();
        self.planar(object)?;
        let embedded = object
            .values
            .iter()
            .position(|v| v.code == 101)
            .and_then(|at| object.values.split_at_checked(at));
        let (single, formatted) = match embedded {
            None => (None, None),
            Some((before, marked)) => {
                // DXF は 71 と 72 の後に、もう一度位置合わせの点（11・21・31）を書く
                let mut values = before.to_vec();
                let kind_at = values
                    .windows(2)
                    .rposition(|pair| matches!(pair, [a, b] if a.code == 71 && b.code == 72));
                if let Some(at) = kind_at {
                    values.drain(at..at + 2);
                }
                let single = Object {
                    values,
                    ..object.clone()
                };
                let mut embedded = Object {
                    values: marked.get(1..).unwrap_or_default().to_vec(),
                    ..object.clone()
                };
                // 文字スタイルは、埋め込みの値になければ属性のものとする
                if embedded.first(7).is_none()
                    && let Some(style) = before.iter().find(|v| v.code == 7)
                {
                    embedded.values.push(style.clone());
                }
                let formatted = self.formatted(&embedded, point, length)?;
                (Some(single), Some(formatted))
            }
        };
        let object = single.as_ref().unwrap_or(object);
        let text = self.text(object, point, length, 74, "dxf.ATTRIB.40")?;
        converted.check(kind)?;
        let tag = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let hidden = self.int(object, 70, 0)? & 1 != 0;
        let style = self.style(object, out)?;
        let layer = self.layer(object, out)?;
        Ok(Attribute {
            tag,
            text,
            formatted,
            hidden,
            layer,
            style,
            source: source(object),
        })
    }

    /// MULTILEADER の記号の属性の値。属性の定義（330）・番号（177）・幅（44）・値（302）の組を、入力の順に
    /// 読む。番号と幅は、まだ移さない
    pub(super) fn attribute_values(
        &self,
        db: &AcadDb,
        entity: &Object,
    ) -> Lifted<Vec<AttributeValue>> {
        let kind = entity.kind.as_slice();
        let mut values = Vec::new();
        let mut definition = None;
        // 属性の組は、記号のブロック（344）の後にある。前の 330 は図形の所有者なので読まない
        let start = entity
            .values
            .iter()
            .position(|value| value.code == 344)
            .map_or(0, |at| at + 1);
        for value in entity.values.iter().skip(start) {
            match (value.code, &value.data) {
                (330, Data::Handle(handle)) => {
                    let found = db
                        .get(*handle)
                        .filter(|record| record.kind == b"ATTDEF")
                        .ok_or_else(|| invalid(kind, 330))?;
                    definition = Some(source(found));
                }
                (302, data) => values.push(AttributeValue {
                    definition: definition.take().ok_or_else(|| invalid(kind, 330))?,
                    text: self.decode(kind, 302, data)?,
                }),
                _ => {}
            }
        }
        Ok(values)
    }
}

/// 用紙とモデルの空間のブロック。中身は空間の図形として移すので、記号の定義にしない
fn layout_block(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("*model_space") || name.starts_with("*paper_space")
}

/// ペーパー空間のブロック（`*Paper_Space`・`*Paper_Space0` など）か。今のレイアウトの図形は ENTITIES に
/// あるので、BLOCKS にあるペーパー空間のブロックの中身は、ほかのレイアウトのもの。DWG は名前に番号を持たない
fn paper_layout(name: &str) -> bool {
    name.to_ascii_lowercase().starts_with("*paper_space")
}
