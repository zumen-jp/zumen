//! 取得した値を移動し、借用する出所だけを番号へ写す。形・表示を再読込しない。

use super::super::{Category, Lifter, category, lift_with_units_mode};
use zumen_building::{
    Acquired, Building, ElementRef, Geometry, GeometryData, GeometryDefinition as Definition,
    GeometryError, GeometryGrid, GeometryGridAxis, GeometryLayer, GeometryLinks, GeometryNode,
    GeometryPresentation, GeometryRef, GeometrySource, GeometrySourceGroup,
    GeometryStyle as OwnedStyle, GeometrySurfaceStyle,
};

use super::{GeometryValue, ProductGeometry, Retrieved, check, reserve};
use crate::{
    Cx, Error, IfcFile, ItemPresentation, Kind, Record, ShapeLinks, ShapeRef, SurfaceStyleBinding,
};

impl IfcFile {
    /// 属性の lift に、所有する形・表示・通り芯とその意味値ハッシュを足す。
    ///
    /// 既存の Lift<Building> は属性だけの経路として保つ。この経路は同じ ID と既存の成分を作り、
    /// 形のある成分を追加する。形の取得順でハッシュを変えず、不正と停止では建物の部分結果を返さない。
    pub fn lift_geometry(&self, cx: &mut Cx) -> Result<Building, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            lift_with_units_mode(self, cx, true).map(|(building, _, _)| building)
        })
    }

    /// 形・表示・通り芯を、L1 を借用しない所有する値として取得する。
    ///
    /// 参照は返した値の中に閉じ、元のレコードの番号は出所に分ける。
    /// 個別の定義を移動するので、曲線・面・表示の値を複製しない。配置の適用と形の評価は行わない。
    pub fn owned_geometry(&self, instance: u64, cx: &mut Cx) -> Result<Geometry, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let geometry = self.product_geometry(instance, cx)?;
            let record = geometry.shape.product;
            let data = Own {
                file: self,
                cx,
                current: geometry.shape.product,
            }
            .geometry(geometry)?;
            Geometry::new(data, cx).map_err(|e| geometry_error(e, record, cx))
        })
    }
}

fn geometry_error(error: GeometryError, record: &Record, cx: &mut Cx) -> Error {
    match error {
        GeometryError::Stopped(stop) => crate::stopped(cx, stop, Some(record.span())),
        _ => super::invalid(cx, Some(record), "geometry-reference"),
    }
}

impl Lifter<'_> {
    pub(in crate::building) fn own_shapes(&mut self, cx: &mut Cx) -> Result<(), Error> {
        for at in 0..self.building.elements().len() {
            check(cx, None, Kind::Work, 1)?;
            let element = self
                .building
                .element(ElementRef(at as u32))
                .ok_or_else(|| super::invalid(cx, None, "element-source"))?;
            let Some(instance) = element.source().instance else {
                continue;
            };
            let Some(record) = self.file.instance(instance) else {
                return Err(super::invalid(cx, None, "element-source"));
            };
            if !matches!(
                category(self.file.kind(record), self.file.schema()),
                Some(Category::Element | Category::Spatial | Category::Grid)
            ) {
                continue;
            }
            let geometry = self.file.owned_geometry(instance, cx)?;
            self.geometry_used(geometry.data(), cx)?;
            self.building
                .insert_geometry(ElementRef(at as u32), geometry, cx)
                .map_err(|e| geometry_error(e, record, cx))?;
        }
        Ok(())
    }

    fn geometry_record_used(&mut self, id: u64, cx: &mut Cx) -> Result<(), Error> {
        let record = self.file.instance(id);
        check(cx, record, Kind::Work, 64)?;
        if !self.used.contains(&id) {
            check(cx, record, Kind::DecodedBytes, 384)?;
            self.used.insert(id);
        }
        Ok(())
    }

    fn geometry_source_used(&mut self, source: &GeometrySource, cx: &mut Cx) -> Result<(), Error> {
        if let Some(id) = source.instance {
            self.geometry_record_used(id, cx)?;
        }
        for group in &source.groups {
            check(cx, None, Kind::Work, 1)?;
            for &id in &group.instances {
                self.geometry_record_used(id, cx)?;
            }
        }
        Ok(())
    }

    fn geometry_curve_used(
        &mut self,
        graph: &zumen_building::CurveGraph,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        for item in &graph.items {
            check(cx, None, Kind::Work, 1)?;
            if matches!(item.kind, zumen_building::CurveKind::Unsupported(_)) {
                continue;
            }
            if let Some(id) = item.source.instance {
                self.geometry_record_used(id, cx)?;
            }
            for &id in &item.source.records {
                self.geometry_record_used(id, cx)?;
            }
        }
        Ok(())
    }

    fn geometry_used(&mut self, data: &GeometryData, cx: &mut Cx) -> Result<(), Error> {
        for node in &data.nodes {
            check(cx, None, Kind::Work, 1)?;
            if matches!(node.definition, Definition::Unsupported(_)) {
                continue;
            }
            // Links の origin は参照だけの場合がある。値を読んだ Mapping の records から使用を数える。
            if matches!(node.definition, Definition::Links) {
                if let Some(id) = node.source.instance {
                    self.geometry_record_used(id, cx)?;
                }
            } else {
                self.geometry_source_used(&node.source, cx)?;
            }
            match &node.definition {
                Definition::Curve(g) | Definition::SweptDisk { directrix: g, .. } => {
                    self.geometry_curve_used(g, cx)?
                }
                Definition::Extrusion {
                    boundary: Some(g), ..
                }
                | Definition::Revolution {
                    boundary: Some(g), ..
                }
                | Definition::HalfSpace {
                    boundary: Some(g), ..
                } => self.geometry_curve_used(g, cx)?,
                _ => {}
            }
        }
        for style in &data.styles {
            check(cx, None, Kind::Work, 1)?;
            let Acquired::Value(p) = &style.presentation else {
                continue;
            };
            // 表示を読んだことは、未対応の形そのものの属性を読んだことにはしない。
            for group in &p.source.groups {
                check(cx, None, Kind::Work, 1)?;
                for &id in &group.instances {
                    self.geometry_record_used(id, cx)?;
                }
            }
            for direct in &p.direct {
                self.geometry_source_used(&direct.source, cx)?;
            }
            for layer in &p.layers {
                self.geometry_source_used(&layer.source, cx)?;
                for style in &layer.styles {
                    self.geometry_source_used(&style.source, cx)?;
                }
            }
        }
        if let Some(Acquired::Value(grid)) = &data.grid {
            for axis in &grid.axes {
                self.geometry_source_used(&axis.source, cx)?;
                self.geometry_curve_used(&axis.curve, cx)?;
            }
        }
        Ok(())
    }
}

const NODE_BYTES: usize = 2048;
const STYLE_BYTES: usize = 1024;
const GROUP_BYTES: usize = 64;
const _: () = assert!(size_of::<GeometryNode>() <= NODE_BYTES);
const _: () = assert!(size_of::<OwnedStyle>() <= STYLE_BYTES);
const _: () = assert!(size_of::<GeometrySurfaceStyle>() <= STYLE_BYTES);
const _: () = assert!(size_of::<GeometryLayer>() <= STYLE_BYTES);
const _: () = assert!(size_of::<GeometryGridAxis>() <= STYLE_BYTES);
const _: () = assert!(size_of::<GeometrySourceGroup>() <= GROUP_BYTES);

struct Own<'f, 'c, 'b> {
    file: &'f IfcFile,
    cx: &'c mut Cx<'b>,
    current: &'f Record,
}

fn reference(value: ShapeRef) -> GeometryRef {
    GeometryRef(value.0)
}
fn source_of(record: &Record) -> GeometrySource {
    GeometrySource {
        instance: record.id(),
        groups: Vec::new(),
    }
}

impl<'f> Own<'f, '_, '_> {
    fn capacity(&mut self, count: usize, bytes: usize) -> Result<(), Error> {
        check(self.cx, Some(self.current), Kind::Work, count as u64)?;
        reserve(self.cx, Some(self.current), count, bytes)
    }

    fn group(
        &mut self,
        source: &mut GeometrySource,
        role: &str,
        records: &[&Record],
    ) -> Result<(), Error> {
        self.capacity(records.len(), 8)?;
        self.capacity(role.len(), 1)?;
        self.capacity(1, GROUP_BYTES)?;
        let mut instances = Vec::with_capacity(records.len());
        instances.extend(records.iter().filter_map(|r| r.id()));
        source.groups.reserve_exact(1);
        source.groups.push(GeometrySourceGroup {
            role: role.to_owned(),
            instances,
        });
        Ok(())
    }

    fn optional(
        &mut self,
        source: &mut GeometrySource,
        role: &str,
        record: Option<&Record>,
    ) -> Result<(), Error> {
        if let Some(record) = record {
            self.group(source, role, &[record])?;
        }
        Ok(())
    }

    fn references(&mut self, refs: Vec<ShapeRef>) -> Result<Vec<GeometryRef>, Error> {
        self.capacity(refs.len(), 8)?;
        Ok(refs.into_iter().map(reference).collect())
    }

    fn links(
        &mut self,
        links: ShapeLinks,
        source: &mut GeometrySource,
    ) -> Result<GeometryLinks, Error> {
        Ok(match links {
            ShapeLinks::Definition { representations } => {
                GeometryLinks::Definition(self.references(representations)?)
            }
            ShapeLinks::Representation {
                context,
                identifier,
                representation_type,
                items,
            } => {
                self.group(source, "context", &[context])?;
                GeometryLinks::Representation {
                    identifier,
                    kind: representation_type,
                    items: self.references(items)?,
                }
            }
            ShapeLinks::Mapped { source: at, target } => {
                self.group(source, "target", &[target])?;
                GeometryLinks::Mapped(reference(at))
            }
            ShapeLinks::Map {
                origin,
                representation,
            } => {
                self.group(source, "origin", &[origin])?;
                GeometryLinks::Map(reference(representation))
            }
            ShapeLinks::Set { elements } => GeometryLinks::Set {
                curves_only: self
                    .file
                    .kind(self.current)
                    .eq_ignore_ascii_case(b"IFCGEOMETRICCURVESET"),
                items: self.references(elements)?,
            },
            ShapeLinks::Boolean {
                operation,
                first,
                second,
            } => GeometryLinks::Boolean {
                operation,
                first: reference(first),
                second: reference(second),
            },
            ShapeLinks::Curve | ShapeLinks::Unsupported => GeometryLinks::Leaf,
        })
    }

    fn definition(
        &mut self,
        value: GeometryValue,
        source: &mut GeometrySource,
    ) -> Result<Definition, Error> {
        Ok(match value {
            GeometryValue::Links => Definition::Links,
            GeometryValue::Context(v) => {
                self.optional(source, "parent", v.source.parent)?;
                self.group(source, "records", &v.source.records)?;
                Definition::Context {
                    definition: v.definition,
                    length_unit: v.length_unit,
                }
            }
            GeometryValue::Curve(v) => Definition::Curve(v),
            GeometryValue::Extrusion(v) => {
                self.group(source, "profile", &[v.source.profile])?;
                self.group(source, "records", &v.source.records)?;
                Definition::Extrusion {
                    definition: v.definition,
                    boundary: v.boundary,
                    length_unit: v.length_unit,
                }
            }
            GeometryValue::Revolution(v) => {
                self.group(source, "profile", &[v.source.profile])?;
                self.group(source, "records", &v.source.records)?;
                Definition::Revolution {
                    definition: v.definition,
                    boundary: v.boundary,
                    length_unit: v.length_unit,
                }
            }
            GeometryValue::SweptDisk(v) => Definition::SweptDisk {
                definition: v.definition,
                directrix: v.directrix,
            },
            GeometryValue::Faces(v) => {
                for (role, records) in [
                    ("sets", &v.source.sets),
                    ("faces", &v.source.faces),
                    ("bounds", &v.source.bounds),
                    ("loops", &v.source.loops),
                    ("points", &v.source.points),
                    ("units", &v.source.units),
                ] {
                    self.group(source, role, records)?;
                }
                Definition::Faces {
                    definition: v.definition,
                    length_unit: v.length_unit,
                }
            }
            GeometryValue::Tessellation(v) => {
                self.group(source, "coordinates", &[v.source.coordinates])?;
                self.group(source, "faces", &v.source.faces)?;
                self.group(source, "units", &v.source.units)?;
                Definition::Tessellation {
                    definition: v.definition,
                    length_unit: v.length_unit,
                }
            }
            GeometryValue::HalfSpace(v) => {
                self.group(source, "surface", &[v.source.surface])?;
                self.group(source, "records", &v.source.records)?;
                Definition::HalfSpace {
                    definition: v.definition,
                    boundary: v.boundary,
                    length_unit: v.length_unit,
                }
            }
            GeometryValue::Mapping(v) => {
                self.group(source, "map", &[v.source.map])?;
                self.group(source, "representation", &[v.source.representation])?;
                self.group(source, "records", &v.source.records)?;
                Definition::Mapping {
                    definition: v.definition,
                    length_unit: v.length_unit,
                }
            }
            GeometryValue::Unsupported { kind } => Definition::Unsupported(kind),
        })
    }

    fn surfaces(
        &mut self,
        values: Vec<SurfaceStyleBinding<'f>>,
    ) -> Result<Vec<GeometrySurfaceStyle>, Error> {
        self.capacity(values.len(), STYLE_BYTES)?;
        let mut result = Vec::with_capacity(values.len());
        for value in values {
            self.current = value.source.style;
            let mut source = source_of(value.source.style);
            self.group(&mut source, "shading", &[value.source.shading])?;
            self.group(&mut source, "colours", &value.source.colours)?;
            result.push(GeometrySurfaceStyle {
                definition: value.definition,
                source,
            });
        }
        Ok(result)
    }

    fn presentation(&mut self, value: ItemPresentation<'f>) -> Result<GeometryPresentation, Error> {
        self.current = value.direct.source.item;
        let mut source = source_of(value.direct.source.item);
        self.optional(&mut source, "styled_item", value.direct.source.styled_item)?;
        self.group(&mut source, "assignments", &value.direct.source.assignments)?;
        let direct = self.surfaces(value.direct.styles)?;
        self.capacity(value.layers.len(), STYLE_BYTES)?;
        let mut layers = Vec::with_capacity(value.layers.len());
        for layer in value.layers {
            self.current = layer.source;
            layers.push(GeometryLayer {
                definition: layer.definition,
                styles: self.surfaces(layer.styles)?,
                level: layer.level,
                source: source_of(layer.source),
            });
        }
        Ok(GeometryPresentation {
            name: value.direct.name,
            direct,
            layers,
            layer_source: value.layer_source,
            surface_source: value.surface_source,
            source,
        })
    }

    fn geometry(&mut self, geometry: ProductGeometry<'f>) -> Result<GeometryData, Error> {
        let mut origin = source_of(geometry.shape.product);
        self.optional(&mut origin, "placement", geometry.shape.placement)?;
        self.capacity(geometry.shape.nodes.len(), NODE_BYTES)?;
        let mut nodes = Vec::with_capacity(geometry.shape.nodes.len());
        for (node, value) in geometry.shape.nodes.into_iter().zip(geometry.values) {
            self.current = node.source;
            check(self.cx, Some(self.current), Kind::Work, 1)?;
            let mut source = source_of(node.source);
            let links = self.links(node.links, &mut source)?;
            let definition = self.definition(value, &mut source)?;
            nodes.push(GeometryNode {
                links,
                definition,
                source,
            });
        }
        self.capacity(geometry.styles.len(), STYLE_BYTES)?;
        let mut styles = Vec::with_capacity(geometry.styles.len());
        for style in geometry.styles {
            check(self.cx, Some(self.current), Kind::Work, 1)?;
            styles.push(OwnedStyle {
                item: reference(style.item),
                representation: style.representation.map(reference),
                presentation: match style.presentation {
                    Retrieved::Value(value) => Acquired::Value(self.presentation(value)?),
                    Retrieved::Unsupported => Acquired::Unsupported,
                },
            });
        }
        let grid = match geometry.grid {
            None => None,
            Some(Retrieved::Unsupported) => Some(Acquired::Unsupported),
            Some(Retrieved::Value(grid)) => {
                self.current = grid.source.grid;
                let mut source = source_of(grid.source.grid);
                self.optional(&mut source, "placement", grid.source.placement)?;
                self.optional(&mut source, "representation", grid.source.representation)?;
                self.capacity(grid.axes.len(), STYLE_BYTES)?;
                let mut axes = Vec::with_capacity(grid.axes.len());
                for axis in grid.axes {
                    check(self.cx, Some(axis.source), Kind::Work, 1)?;
                    axes.push(GeometryGridAxis {
                        definition: axis.definition,
                        curve: axis.curve,
                        source: source_of(axis.source),
                    });
                }
                Some(Acquired::Value(GeometryGrid {
                    axes,
                    layout: grid.layout,
                    source,
                }))
            }
        };
        Ok(GeometryData {
            root: geometry.shape.definition.map(reference),
            nodes,
            styles,
            grid,
            source: origin,
        })
    }
}
