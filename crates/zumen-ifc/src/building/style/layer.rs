//! buildingSMART IFC2X3 TC1 / IFC4 ADD2 TC1 / IFC4.3 ADD2 の
//! IfcRepresentationItem・IfcPresentationLayerAssignment・IfcPresentationLayerWithStyle の
//! 属性と使用規則。項目の割り当ては表現の割り当てに、StyledItem は同種のレイヤ表示に優先する。

pub use zumen_building::{LayerAssignmentLevel, SurfaceStyleSelection};
use zumen_building::{LayerState, PresentationLayer};

use super::{ItemStyleSource, ItemSurfaceStyles, Reader, SurfaceStyleBinding, Version, arg};
use crate::{Cx, Error, Form, IfcFile, Kind, Record, ShapeNode, Value, stopped};

/// レイヤの値・表示と、借用する出所
#[derive(Clone, Debug)]
pub struct LayerPresentation<'f> {
    /// 出所を含まない値
    pub definition: PresentationLayer,
    /// 元の集合の並びの面の表示
    pub styles: Vec<SurfaceStyleBinding<'f>>,
    /// 割り当ての位置
    pub level: LayerAssignmentLevel,
    /// 元のレイヤ。AssignedItems はこのレコードに残る
    pub source: &'f Record,
}

/// 形の項目と、それを直接含む表現の表示。写像の別の階層とは合成しない
#[derive(Clone, Debug)]
pub struct ItemPresentation<'f> {
    /// 直接の StyledItem。従来の surface_styles と同じ元値
    pub direct: ItemSurfaceStyles<'f>,
    /// 関係する全レイヤ。上書きされた指定も、元のレコード順で保持する
    pub layers: Vec<LayerPresentation<'f>>,
    /// 適用するレイヤの位置。項目への指定があれば表現への指定より優先する
    pub layer_source: LayerAssignmentLevel,
    /// 面の表示を選ぶ規則の結果
    pub surface_source: SurfaceStyleSelection,
}

impl IfcFile {
    /// 直接の面の表示とレイヤの指定を必要時に読む。
    ///
    /// representation は項目を直接 Items に含む ShapeRepresentation に限る。省略すれば項目だけを調べる。
    /// 項目にレイヤがあれば表現のレイヤに優先し、直接の面の表示があればレイヤの面の表示に優先する。
    /// 上書きされたレイヤも値と出所を保持する。未知の表示を混ぜた部分成功にはしない。
    /// レイヤの表示・固定・編集禁止の不明値を保ち、表示色・材質・写像間の継承は評価しない。
    pub fn item_presentation<'f>(
        &'f self,
        node: &ShapeNode<'_>,
        representation: Option<&ShapeNode<'_>>,
        cx: &mut Cx,
    ) -> Result<ItemPresentation<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            // 旧経路の検査と意味値はそのまま使い、追加の走査も同じ予算で数える。
            let direct = self.surface_styles(node, cx)?;
            let mut reader = Reader {
                file: self,
                cx,
                current: node.source.id().unwrap_or(0),
                colours: Vec::new(),
            };
            let version = Version::of(self.schema()).ok_or(Error::Unsupported)?;
            reader.within(reader.current, |reader, item| {
                reader.presentation_layers(item, representation, direct, version)
            })
        })
    }
}

impl<'f> Reader<'f, '_, '_> {
    fn logical(&mut self, value: Option<&'f Value>) -> Result<Option<bool>, Error> {
        let value = self.enumeration(value)?;
        if value.eq_ignore_ascii_case(b"T") {
            Ok(Some(true))
        } else if value.eq_ignore_ascii_case(b"F") {
            Ok(Some(false))
        } else if value.eq_ignore_ascii_case(b"U") {
            Ok(None)
        } else {
            Err(self.invalid("logical"))
        }
    }

    fn layer(
        &mut self,
        record: &'f Record,
        level: LayerAssignmentLevel,
        version: Version,
    ) -> Result<LayerPresentation<'f>, Error> {
        if record.form() != Form::Entity {
            return Err(self.invalid("type"));
        }
        let styled = self.is(record, b"IFCPRESENTATIONLAYERWITHSTYLE");
        self.arity(record, if styled { 8 } else { 4 })?;
        let name = self
            .text(arg(record, 1), record)?
            .ok_or_else(|| self.invalid("name"))?;
        let description = self.text(arg(record, 2), record)?;
        self.list(arg(record, 3), usize::MAX)?;
        let identifier = self.text(arg(record, 4), record)?;
        // IFC4 以降の StylePossible: 表現への WithStyle の割り当ては禁止。
        if styled && version != Version::Ifc2x3 && level == LayerAssignmentLevel::Representation {
            return Err(self.invalid("layer-target"));
        }
        let mut result = ItemSurfaceStyles {
            name: None,
            styles: Vec::new(),
            source: ItemStyleSource {
                item: record,
                styled_item: None,
                assignments: Vec::new(),
            },
        };
        let state = if styled {
            let state = LayerState {
                on: self.logical(arg(record, 5))?,
                frozen: self.logical(arg(record, 6))?,
                blocked: self.logical(arg(record, 7))?,
            };
            let Some(Value::List(values)) = arg(record, 8) else {
                return Err(self.invalid("list"));
            };
            if !values.is_empty() {
                // 旧版の NULL は表示を補う指示なので、空集合と同一視しない。
                self.check(Kind::Work, values.len() as u64)?;
                if values
                    .iter()
                    .any(|v| matches!(v, Value::Enum(_) | Value::Typed(_)))
                {
                    if version == Version::Ifc2x3
                        && values.iter().any(|v| match v {
                            Value::Enum(raw) => {
                                self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"NULL")
                            }
                            Value::Typed(p) => self
                                .file
                                .bytes(p.name())
                                .eq_ignore_ascii_case(b"IFCNULLSTYLE"),
                            _ => false,
                        })
                    {
                        return Err(self.unsupported_kind(record, b"IFCNULLSTYLE"));
                    }
                    return Err(self.invalid("select"));
                }
                let values = self.list(arg(record, 8), usize::MAX)?;
                for value in values {
                    let id = self.target(Some(value))?;
                    self.reserve_style(&mut result)?;
                    result.styles.push(self.surface(id, version)?);
                }
            }
            Some(state)
        } else {
            None
        };
        Ok(LayerPresentation {
            definition: PresentationLayer {
                name,
                description,
                identifier,
                state,
            },
            styles: result.styles,
            level,
            source: record,
        })
    }

    fn presentation_layers(
        &mut self,
        item: &'f Record,
        representation: Option<&ShapeNode<'_>>,
        direct: ItemSurfaceStyles<'f>,
        version: Version,
    ) -> Result<ItemPresentation<'f>, Error> {
        if item.form() != Form::Entity {
            return Err(self.invalid("type"));
        }
        if [
            b"IFCPRODUCTDEFINITIONSHAPE".as_slice(),
            b"IFCSHAPEREPRESENTATION",
            b"IFCREPRESENTATIONMAP",
        ]
        .iter()
        .any(|kind| self.is(item, kind))
        {
            return Err(self.invalid("item"));
        }
        let rep = if let Some(node) = representation {
            let id = node.source.id().ok_or_else(|| self.invalid("source"))?;
            Some(self.within(id, |reader, r| {
                if !std::ptr::eq(r, node.source) || r.form() != Form::Entity {
                    return Err(reader.invalid("source"));
                }
                if !reader.is(r, b"IFCSHAPEREPRESENTATION") {
                    return Err(reader.invalid("representation"));
                }
                reader.arity(r, 4)?;
                let items = reader.list(arg(r, 4), usize::MAX)?;
                if !items.iter().any(|v| super::reference(Some(v)) == item.id()) {
                    return Err(reader.invalid("membership"));
                }
                Ok(r)
            })?)
        } else {
            None
        };
        let mut layers = Vec::new();
        let mut item_count = 0_u64;
        for record in self
            .file
            .owner_records(crate::style_owners::Owner::Layer, self.cx)?
        {
            self.cx
                .check(Kind::Work, 1)
                .map_err(|stop| stopped(self.cx, stop, Some(record.span())))?;
            if record.id().is_none() {
                continue;
            }
            let previous = self.current;
            self.current = record.id().unwrap_or(previous);
            self.check(Kind::Work, record.parts().len().saturating_sub(1) as u64)?;
            let mut assigned = None;
            let mut styled_part = false;
            for part in record.parts() {
                let kind = self.file.bytes(part.name());
                if kind.eq_ignore_ascii_case(b"IFCPRESENTATIONLAYERASSIGNMENT") {
                    if part.args().len() != 4 || assigned.is_some() {
                        return Err(self.invalid("owner-layout"));
                    }
                    assigned = part.args().get(2);
                } else if kind.eq_ignore_ascii_case(b"IFCPRESENTATIONLAYERWITHSTYLE") {
                    styled_part = true;
                    if part.args().len() == 8 {
                        if assigned.is_some() {
                            return Err(self.invalid("owner-layout"));
                        }
                        assigned = part.args().get(2);
                    } else if part.args().len() != 4 || record.parts().len() == 1 {
                        return Err(self.invalid("owner-layout"));
                    }
                }
            }
            if assigned.is_none() && styled_part {
                return Err(self.invalid("owner-layout"));
            }
            let Some(assigned) = assigned else {
                self.current = previous;
                continue;
            };
            let Value::List(values) = assigned else {
                return Err(self.invalid("owner-list"));
            };
            self.check(Kind::Work, values.len() as u64)?;
            let mut on_item = false;
            let mut on_rep = false;
            for value in values {
                let id = self.target(Some(value))?;
                on_item |= Some(id) == item.id();
                on_rep |= rep.is_some_and(|r| r.id() == Some(id));
            }
            if on_item || on_rep {
                let level = if on_item {
                    LayerAssignmentLevel::Item
                } else {
                    LayerAssignmentLevel::Representation
                };
                if on_item {
                    item_count += 1;
                    if version != Version::Ifc2x3 && item_count > 1 {
                        return Err(self.invalid("layer-count"));
                    }
                }
                // 表現にも同時に付く WithStyle は、項目が優先される場合も不正。
                if on_rep
                    && version != Version::Ifc2x3
                    && self.is(record, b"IFCPRESENTATIONLAYERWITHSTYLE")
                {
                    return Err(self.invalid("layer-target"));
                }
                self.check(Kind::Elements, 1)?;
                if layers.len() == layers.capacity() {
                    let old = layers.capacity();
                    let capacity = old.saturating_mul(2).max(1).min(i32::MAX as usize / 1024);
                    if capacity <= old {
                        return Err(self.invalid("capacity"));
                    }
                    self.check(Kind::Work, layers.len() as u64)?;
                    self.alloc(capacity - old, 1024)?;
                    layers.reserve_exact(capacity - layers.len());
                }
                layers
                    .push(self.within(self.current, |reader, r| reader.layer(r, level, version))?);
            }
            self.current = previous;
        }
        let layer_source = if item_count > 0 {
            LayerAssignmentLevel::Item
        } else if layers.is_empty() {
            LayerAssignmentLevel::None
        } else {
            LayerAssignmentLevel::Representation
        };
        let mut surface_source = if direct.styles.is_empty() {
            SurfaceStyleSelection::None
        } else {
            SurfaceStyleSelection::Direct
        };
        self.check(Kind::Work, layers.len() as u64)?;
        if surface_source != SurfaceStyleSelection::Direct {
            for (at, layer) in layers.iter().enumerate() {
                if layer.level == layer_source && !layer.styles.is_empty() {
                    surface_source = match surface_source {
                        SurfaceStyleSelection::None => SurfaceStyleSelection::Layer(at as u32),
                        _ => SurfaceStyleSelection::MultipleLayers,
                    };
                }
            }
        }
        Ok(ItemPresentation {
            direct,
            layers,
            layer_source,
            surface_source,
        })
    }
}
