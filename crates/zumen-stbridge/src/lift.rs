//! ST-Bridge の L1 から、zumen の建物モデル（L2）を作る（決定 0207・0245、#400）。
//!
//! - 要素の種類は IFC の実体の名前（`IfcColumn` など）にし、元の要素の名前を `original` に残す。
//! - 要素の ID は `guid`（UUID か 22 文字の GlobalId）から作り、なければ XML の要素の番号で作る。
//! - 部材の属性と、参照する断面の属性は、元の要素の名前のプロパティのセットに、文字列のまま持つ。
//! - 配置は、部材を決める節点から作る（決め方は `docs/formats/stbridge.md`）。柱・梁・ブレース・スラブ・壁は、
//!   断面の形を押し出しの形にする（`crate::shape`）。
//! - 移さない要素は、要素の名前ごとに `stbridge.element.unsupported` で知らせる。断面は、形にしなかった子の要素と
//!   配筋を移さない。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_building::{
    Application, Building, Class, Document, ElementRef, GeometryData, GeometryError, GlobalId,
    Grid, GridSet, Header, Identity, Literal, Local, Measured, Membership, Opening, Parent,
    ParentKind, Placement, Properties, Property, PropertySet, Spatial, SpatialKind, Transform,
    TypedValue, Unit,
};
use zumen_sdk::{Error, Lift};

use crate::StbFile;
use crate::shape::{self, Figure, Found};

/// 部材の要素と、建物モデルでの種類と、節点の持ち方と、断面の要素の名前の始まり
struct MemberKind {
    element: &'static [u8],
    class: &'static str,
    predefined: Option<&'static str>,
    nodes: Nodes,
    section: Option<&'static str>,
    shape: Rule,
}

/// 断面の形の置き方
#[derive(Clone, Copy, PartialEq)]
enum Rule {
    /// 断面の中心を材軸に合わせる（柱・ブレース）
    Centered,
    /// 断面の上端を材軸に合わせる（梁）
    Top,
    /// 節点の面を上端にして、厚さを下へとる（スラブ）
    Slab,
    /// 節点の面を中心にして、厚さを両側へとる（壁）
    Wall,
    /// 形を作らない
    None,
}

/// 部材の配置に使った、形を作るための値
enum Extent {
    /// 線の部材の材軸の長さ
    Line(f64),
    /// 面の部材の節点（全体の座標）
    Plate(Vec<[f64; 3]>),
    None,
}

/// 部材を決める節点
#[derive(Clone, Copy)]
enum Nodes {
    /// 柱・間柱。下と上の節点。階は上の節点で決める
    BottomTop,
    /// 梁・ブレースなど。始端と終端の節点。`upper` なら、階を高い方の節点で決める
    StartEnd { upper: bool },
    /// スラブ。節点の並び。階は最初の節点で決める
    OrderFirst,
    /// 壁。節点の並び。階は最後の節点で決める
    OrderLast,
    /// 基礎・杭。1 つの節点
    Single,
}

const MEMBERS: &[MemberKind] = &[
    MemberKind {
        element: b"StbColumn",
        class: "IfcColumn",
        predefined: Some("COLUMN"),
        nodes: Nodes::BottomTop,
        section: Some("StbSecColumn"),
        shape: Rule::Centered,
    },
    MemberKind {
        element: b"StbPost",
        class: "IfcColumn",
        predefined: Some("COLUMN"),
        nodes: Nodes::BottomTop,
        section: Some("StbSecColumn"),
        shape: Rule::Centered,
    },
    MemberKind {
        element: b"StbGirder",
        class: "IfcBeam",
        predefined: Some("BEAM"),
        nodes: Nodes::StartEnd { upper: false },
        section: Some("StbSecBeam"),
        shape: Rule::Top,
    },
    MemberKind {
        element: b"StbBeam",
        class: "IfcBeam",
        predefined: Some("BEAM"),
        nodes: Nodes::StartEnd { upper: false },
        section: Some("StbSecBeam"),
        shape: Rule::Top,
    },
    MemberKind {
        element: b"StbBrace",
        class: "IfcMember",
        predefined: Some("BRACE"),
        nodes: Nodes::StartEnd { upper: true },
        section: Some("StbSecBrace"),
        shape: Rule::Centered,
    },
    MemberKind {
        element: b"StbSlab",
        class: "IfcSlab",
        predefined: None,
        nodes: Nodes::OrderFirst,
        section: Some("StbSecSlab"),
        shape: Rule::Slab,
    },
    MemberKind {
        element: b"StbWall",
        class: "IfcWall",
        predefined: None,
        nodes: Nodes::OrderLast,
        section: Some("StbSecWall"),
        shape: Rule::Wall,
    },
    MemberKind {
        element: b"StbFooting",
        class: "IfcFooting",
        predefined: Some("PAD_FOOTING"),
        nodes: Nodes::Single,
        section: Some("StbSecFoundation"),
        shape: Rule::None,
    },
    MemberKind {
        element: b"StbStripFooting",
        class: "IfcFooting",
        predefined: Some("STRIP_FOOTING"),
        nodes: Nodes::StartEnd { upper: false },
        section: Some("StbSecFoundation"),
        shape: Rule::None,
    },
    MemberKind {
        element: b"StbPile",
        class: "IfcPile",
        predefined: None,
        nodes: Nodes::Single,
        section: Some("StbSecPile"),
        shape: Rule::None,
    },
    // 基礎柱の断面は、柱と根巻きの 2 つを属性で指すので、断面のセットを持たせない
    MemberKind {
        element: b"StbFoundationColumn",
        class: "IfcColumn",
        predefined: None,
        nodes: Nodes::Single,
        section: None,
        shape: Rule::None,
    },
    MemberKind {
        element: b"StbParapet",
        class: "IfcWall",
        predefined: Some("PARAPET"),
        nodes: Nodes::StartEnd { upper: false },
        section: Some("StbSecParapet"),
        shape: Rule::None,
    },
];

/// 開口の断面の要素の名前の始まり
const OPEN_SECTION: &str = "StbSecOpen";

/// 節点
#[derive(Clone, Copy)]
struct NodePoint {
    point: [f64; 3],
    story: Option<ElementRef>,
}

/// 作っている途中のもの
struct Builder<'f, 'c, 'x> {
    file: &'f StbFile,
    cx: &'c mut Cx<'x>,
    building: Building,
    /// 移さない要素の名前
    unsupported: BTreeMap<String, Ref>,
}

fn millimetre() -> Unit {
    Unit {
        kind: "LENGTHUNIT".to_owned(),
        prefix: Some("MILLI".to_owned()),
        name: "METRE".to_owned(),
        scale: None,
    }
}

fn number(text: Option<String>) -> Option<f64> {
    text.and_then(|t| t.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

/// 空白で区切った整数の並び（monolist）
fn ids(text: &str) -> Vec<String> {
    text.split_ascii_whitespace().map(str::to_owned).collect()
}

/// `guid` を 22 文字の GlobalId にする。UUID（`8-4-4-4-12` か、区切りのない 32 桁）と 22 文字の GlobalId を読む
fn global_id(guid: &str) -> Option<String> {
    let guid = guid.trim();
    if let Some(id) = GlobalId::parse(guid) {
        return Some(id.to_string());
    }
    if let Some(id) = GlobalId::parse_uuid(guid) {
        return Some(id.to_string());
    }
    if guid.len() == 32 && guid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return u128::from_str_radix(guid, 16)
            .ok()
            .map(|v| GlobalId::from_uuid(v.to_be_bytes()).to_string());
    }
    None
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    [ax - bx, ay - by, az - bz]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let [ax, ay, az] = a;
    let [bx, by, bz] = b;
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let [x, y, z] = sub(b, a);
    libm::sqrt(x * x + y * y + z * z)
}

/// 面の部材の形。節点を局所の XY に写し、局所の Z が 1 mm を超えてずれる（平面でない）なら作らない
fn plate(
    points: &[[f64; 3]],
    thickness: f64,
    rule: Rule,
    local: &Transform,
    instance: u64,
) -> Option<GeometryData> {
    let Transform([[xx, yx, zx, ox], [xy, yy, zy, oy], [xz, yz, zz, oz]]) = *local;
    let mut flat = Vec::with_capacity(points.len());
    for &[px, py, pz] in points {
        let [dx, dy, dz] = [px - ox, py - oy, pz - oz];
        let dot = |[ax, ay, az]: [f64; 3]| ax * dx + ay * dy + az * dz;
        if dot([zx, zy, zz]).abs() > 1.0 {
            return None;
        }
        flat.push([dot([xx, xy, xz]), dot([yx, yy, yz])]);
    }
    if flat.len() < 3 {
        return None;
    }
    Some(match rule {
        // 上端の面から下へ。局所の Z が下を向いていれば、局所の +Z へ押し出す
        Rule::Slab => {
            let sign = if zz < 0.0 { 1.0 } else { -1.0 };
            shape::plate(&flat, thickness, 0.0, sign, instance)
        }
        _ => shape::plate(&flat, thickness, -thickness / 2.0, 1.0, instance),
    })
}

fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let [x, y, z] = v;
    let length = libm::sqrt(x * x + y * y + z * z);
    (length > 1e-9).then(|| [x / length, y / length, z / length])
}

/// 軸 `x`・`y`・`z` と原点 `origin` の変換。軸は列に置く
fn frame(x: [f64; 3], y: [f64; 3], z: [f64; 3], origin: [f64; 3]) -> Transform {
    let [xx, xy, xz] = x;
    let [yx, yy, yz] = y;
    let [zx, zy, zz] = z;
    let [ox, oy, oz] = origin;
    Transform([[xx, yx, zx, ox], [xy, yy, zy, oy], [xz, yz, zz, oz]])
}

/// 局所の Z のまわりに `degrees` 回した変換（右ねじ）
fn about_z(degrees: f64) -> Transform {
    let radians = degrees.to_radians();
    let (s, c) = (libm::sin(radians), libm::cos(radians));
    Transform([[c, -s, 0.0, 0.0], [s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]])
}

/// 材軸を局所の Z にする配置。材軸が鉛直なら局所の X を全体の X に、そうでなければ水平にとり、材軸のまわりに
/// `rotate` 度回す
fn linear(start: [f64; 3], end: [f64; 3], rotate: f64) -> Option<Transform> {
    let z = normalize(sub(end, start))?;
    let [_, _, zz] = z;
    let x = if zz.abs() > 1.0 - 1e-9 {
        [1.0, 0.0, 0.0]
    } else {
        normalize(cross([0.0, 0.0, 1.0], z))?
    };
    let y = cross(z, x);
    Some(frame(x, y, z, start).then(&about_z(rotate)))
}

/// 面の部材の配置。最初の節点を原点に、最初から 2 つ目の節点への向きを局所の X、面の法線を局所の Z にする
fn planar(points: &[[f64; 3]]) -> Option<Transform> {
    let origin = *points.first()?;
    let x = normalize(sub(*points.get(1)?, origin))?;
    // 法線は、最初の節点からの向きの外積の和（多角形の面積の向き）
    let mut normal = [0.0; 3];
    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        let [nx, ny, nz] = cross(sub(*a, origin), sub(*b, origin));
        let [sx, sy, sz] = normal;
        normal = [sx + nx, sy + ny, sz + nz];
    }
    let z = normalize(normal)?;
    let y = cross(z, x);
    Some(frame(x, y, z, origin))
}

impl<'f, 'c, 'x> Builder<'f, 'c, 'x> {
    fn at(&self, element: usize) -> Ref {
        let at = self
            .file
            .span(element)
            .map_or_else(Ref::default, |span| Ref::at_bytes(span.start, span.end));
        at.with_record(element as u64)
    }

    fn name_of(&self, element: usize) -> String {
        String::from_utf8_lossy(self.file.name(element)).into_owned()
    }

    fn unsupported(&mut self, element: usize) {
        let name = self.name_of(element);
        if !self.unsupported.contains_key(&name) {
            let at = self.at(element);
            self.unsupported.insert(name, at);
        }
    }

    fn missing(&mut self, element: usize, attribute: &str, target: &str) {
        let at = self.at(element);
        self.cx.report(
            Diagnostic::new(codes::STBRIDGE_REFERENCE_MISSING)
                .with("element", self.name_of(element))
                .with("attribute", attribute)
                .with("target", target)
                .at(at),
        );
    }

    /// 要素を足す。`guid` があれば ID にする
    fn push(&mut self, element: usize) -> Result<Option<ElementRef>, Error> {
        self.cx.check(Kind::Elements, 1)?;
        let guid = self
            .file
            .attribute(element, "guid")
            .and_then(|g| global_id(&g));
        Ok(self.building.push(guid.as_deref(), element as u64, self.cx))
    }

    fn class(&mut self, target: ElementRef, name: &str, predefined: Option<&str>, element: usize) {
        let original = self.name_of(element);
        self.building.components.class.insert(
            target,
            Class {
                name: name.to_owned(),
                predefined: predefined.map(str::to_owned),
                user_defined: None,
                original,
            },
        );
    }

    fn identity(&mut self, target: ElementRef, element: usize) {
        self.building.components.identity.insert(
            target,
            Identity {
                name: self.file.attribute(element, "name"),
                description: None,
                tag: self.file.attribute(element, "id"),
                long_name: None,
            },
        );
    }

    /// 要素の属性を、要素の名前のセットにする
    fn set_of(&self, element: usize) -> PropertySet {
        PropertySet {
            name: self.name_of(element),
            properties: self
                .file
                .attributes(element)
                .into_iter()
                .map(|(name, value)| Property {
                    name,
                    value: Some(TypedValue {
                        type_name: None,
                        value: Literal::Text(value),
                    }),
                    unit: None,
                })
                .collect(),
        }
    }

    fn parent(&mut self, target: ElementRef, of: ElementRef, kind: ParentKind) {
        let entry = self
            .building
            .components
            .parent
            .get(target)
            .cloned()
            .unwrap_or(Parent {
                of: None,
                referenced_in: Vec::new(),
            });
        self.building.components.parent.insert(
            target,
            Parent {
                of: Some(Membership {
                    target: of,
                    kind,
                    order: None,
                }),
                ..entry
            },
        );
    }

    fn place(&mut self, target: ElementRef, relative_to: Option<ElementRef>, local: Transform) {
        self.building.components.placement.insert(
            target,
            Placement {
                relative_to,
                local: Local::Transform(local),
            },
        );
    }
}

impl Lift<Building> for StbFile {
    fn lift(&self, cx: &mut Cx) -> Result<Building, Error> {
        let root = self.xml().root();
        let common = self.child(root, b"StbCommon");
        let model = self.child(root, b"StbModel");
        let project_name = common.and_then(|c| self.attribute(c, "project_name"));
        let application = common
            .and_then(|c| self.attribute(c, "app_name"))
            .map(|name| Application {
                name,
                version: common.and_then(|c| self.attribute(c, "app_version")),
            });
        let mut builder = Builder {
            file: self,
            cx,
            building: Building::new(Document {
                project_name: project_name.clone(),
                // 長さは mm、角度は度（仕様書の単位）
                units: vec![
                    millimetre(),
                    Unit {
                        kind: "PLANEANGLEUNIT".to_owned(),
                        prefix: None,
                        name: "DEGREE".to_owned(),
                        scale: None,
                    },
                ],
                application,
                header: Header {
                    schemas: vec![format!("ST-Bridge {}", self.version())],
                    ..Header::default()
                },
                ..Document::default()
            }),
            unsupported: BTreeMap::new(),
        };

        // プロジェクトと建物。建物は ST-Bridge に要素がないので、StbModel の番号で作る
        let project = builder.push(common.unwrap_or(root))?;
        if let Some(project) = project {
            builder.class(project, "IfcProject", None, common.unwrap_or(root));
            builder.building.components.identity.insert(
                project,
                Identity {
                    name: project_name,
                    ..Identity::default()
                },
            );
            if let Some(common) = common {
                let set = builder.set_of(common);
                builder
                    .building
                    .components
                    .properties
                    .insert(project, Properties { sets: vec![set] });
            }
            builder.building.document.project = Some(project);
        }
        let Some(model) = model else {
            return Ok(builder.finish());
        };
        builder.cx.check(Kind::Elements, 1)?;
        let building = builder.building.push(None, model as u64, builder.cx);
        if let Some(building) = building {
            builder.class(building, "IfcBuilding", None, model);
            builder.building.components.spatial.insert(
                building,
                Spatial {
                    kind: SpatialKind::Building,
                    elevation: None,
                    ref_elevation: None,
                },
            );
            if let Some(project) = project {
                builder.parent(building, project, ParentKind::Aggregates);
            }
        }

        // 階と、節点の階
        let mut stories: Vec<(ElementRef, f64)> = Vec::new();
        let mut node_story: BTreeMap<String, ElementRef> = BTreeMap::new();
        if let Some(list) = self.child(model, b"StbStories") {
            for story in self.children(list).collect::<Vec<_>>() {
                if self.name(story) != b"StbStory" {
                    builder.unsupported(story);
                    continue;
                }
                let Some(target) = builder.push(story)? else {
                    continue;
                };
                let height = number(self.attribute(story, "height")).unwrap_or(0.0);
                builder.class(target, "IfcBuildingStorey", None, story);
                builder.identity(target, story);
                builder.building.components.spatial.insert(
                    target,
                    Spatial {
                        kind: SpatialKind::Storey,
                        elevation: Some(Measured {
                            value: height,
                            unit: Some(millimetre()),
                        }),
                        ref_elevation: None,
                    },
                );
                let set = builder.set_of(story);
                builder
                    .building
                    .components
                    .properties
                    .insert(target, Properties { sets: vec![set] });
                if let Some(building) = building {
                    builder.parent(target, building, ParentKind::Aggregates);
                }
                stories.push((target, height));
                if let Some(nodes) = self.child(story, b"StbNodeIdList") {
                    for node in self.children(nodes) {
                        if let Some(id) = self.attribute(node, "id") {
                            node_story.entry(id.trim().to_owned()).or_insert(target);
                        }
                    }
                }
            }
        }

        // 節点。要素にはせず、部材の配置と階に使う
        let mut nodes: BTreeMap<String, NodePoint> = BTreeMap::new();
        if let Some(list) = self.child(model, b"StbNodes") {
            for node in self.children(list) {
                builder.cx.check(Kind::Work, 1)?;
                let Some(id) = self.attribute(node, "id") else {
                    continue;
                };
                let id = id.trim().to_owned();
                let point = [
                    number(self.attribute(node, "X")).unwrap_or(0.0),
                    number(self.attribute(node, "Y")).unwrap_or(0.0),
                    number(self.attribute(node, "Z")).unwrap_or(0.0),
                ];
                // 階の節点の一覧にない節点は、代表高さが最も近い階に属するとみなす
                let story = node_story.get(&id).copied().or_else(|| {
                    let [_, _, z] = point;
                    stories
                        .iter()
                        .min_by(|a, b| (a.1 - z).abs().total_cmp(&(b.1 - z).abs()))
                        .map(|(story, _)| *story)
                });
                nodes.entry(id).or_insert(NodePoint { point, story });
            }
        }

        // 通り芯。軸の組ごとに IfcGrid、軸ごとに IfcGridAxis にする
        if let Some(axes) = self.child(model, b"StbAxes") {
            for group in self.children(axes).collect::<Vec<_>>() {
                let Some(grid) = builder.push(group)? else {
                    continue;
                };
                builder.class(grid, "IfcGrid", None, group);
                builder.building.components.identity.insert(
                    grid,
                    Identity {
                        name: self.attribute(group, "group_name"),
                        ..Identity::default()
                    },
                );
                let origin = [
                    number(self.attribute(group, "X")).unwrap_or(0.0),
                    number(self.attribute(group, "Y")).unwrap_or(0.0),
                    0.0,
                ];
                let [ox, oy, oz] = origin;
                let angle = number(self.attribute(group, "angle")).unwrap_or(0.0);
                builder.place(
                    grid,
                    None,
                    Transform::translation(ox, oy, oz).then(&about_z(angle)),
                );
                let set = builder.set_of(group);
                let mut members = Vec::new();
                for axis in self.children(group).collect::<Vec<_>>() {
                    let Some(target) = builder.push(axis)? else {
                        continue;
                    };
                    builder.class(target, "IfcGridAxis", None, axis);
                    builder.building.components.identity.insert(
                        target,
                        Identity {
                            name: self.attribute(axis, "name"),
                            ..Identity::default()
                        },
                    );
                    builder.building.components.grid.insert(
                        target,
                        Grid::Axis {
                            tag: self.attribute(axis, "name"),
                            same_sense: true,
                            curve: None,
                        },
                    );
                    let set = builder.set_of(axis);
                    builder
                        .building
                        .components
                        .properties
                        .insert(target, Properties { sets: vec![set] });
                    members.push((GridSet::U, target));
                }
                builder
                    .building
                    .components
                    .grid
                    .insert(grid, Grid::Grid { axes: members });
                if let Some(building) = building {
                    builder.parent(grid, building, ParentKind::Contained);
                }
                builder
                    .building
                    .components
                    .properties
                    .insert(grid, Properties { sets: vec![set] });
            }
        }

        // 鋼材の規格の一覧。名前で引く。寸法を引けない種類は移さない
        let list = self.child(model, b"StbSections");
        let mut steels: BTreeMap<String, usize> = BTreeMap::new();
        for table in list.into_iter().flat_map(|list| self.children(list)) {
            if self.name(table) != b"StbSecSteel" {
                continue;
            }
            for steel in self.children(table).collect::<Vec<_>>() {
                builder.cx.check(Kind::Work, 1)?;
                if !shape::STEEL.contains(&self.name(steel)) {
                    builder.unsupported(steel);
                }
                if let Some(name) = self.attribute(steel, "name") {
                    steels.entry(name.trim().to_owned()).or_insert(steel);
                }
            }
        }

        // 断面。要素の名前と ID で引く
        let mut sections: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut figures: BTreeMap<usize, Figure> = BTreeMap::new();
        for section in list
            .into_iter()
            .flat_map(|list| self.children(list))
            .collect::<Vec<_>>()
        {
            builder.cx.check(Kind::Work, 1)?;
            if self.name(section) == b"StbSecSteel" {
                continue;
            }
            // ID のない要素は移さない
            let Some(id) = self.attribute(section, "id") else {
                builder.unsupported(section);
                continue;
            };
            sections
                .entry(id.trim().to_owned())
                .or_default()
                .push(section);
            let used = match shape::figure(self, section, &steels) {
                Found::Figure { figure, used } => {
                    figures.insert(section, figure);
                    used
                }
                Found::Missing { element, name } => {
                    builder.missing(element, "shape", &name);
                    Vec::new()
                }
                Found::None => Vec::new(),
            };
            // 形にした要素のほかの子（配筋など）は移さない
            let mut parents = vec![section];
            parents.extend(used.iter().copied().take(used.len().saturating_sub(1)));
            for parent in parents {
                for child in self.children(parent).collect::<Vec<_>>() {
                    if !used.contains(&child) {
                        builder.unsupported(child);
                    }
                }
            }
        }

        // 部材
        let mut hosts: BTreeMap<(&'static [u8], String), ElementRef> = BTreeMap::new();
        let mut opens_of: Vec<(usize, ElementRef)> = Vec::new();
        let mut opens_20: BTreeMap<String, usize> = BTreeMap::new();
        let mut arrangements: Vec<usize> = Vec::new();
        if let Some(members) = self.child(model, b"StbMembers") {
            for list in self.children(members).collect::<Vec<_>>() {
                for member in self.children(list).collect::<Vec<_>>() {
                    let name = self.name(member);
                    if name == b"StbOpenArrangement" {
                        arrangements.push(member);
                        continue;
                    }
                    if name == b"StbOpen" {
                        if let Some(id) = self.attribute(member, "id") {
                            opens_20.insert(id.trim().to_owned(), member);
                        }
                        continue;
                    }
                    let Some(kind) = MEMBERS.iter().find(|kind| kind.element == name) else {
                        builder.unsupported(member);
                        continue;
                    };
                    let Some(target) =
                        builder.member(member, kind, &nodes, (&sections, &figures), building)?
                    else {
                        continue;
                    };
                    if let Some(id) = self.attribute(member, "id") {
                        hosts.insert((kind.element, id.trim().to_owned()), target);
                    }
                    // 2.0 の開口は、スラブと壁が開口の ID の一覧を持つ
                    if let Some(list) = self.child(member, b"StbOpenIdList") {
                        for open in self.children(list) {
                            opens_of.push((open, target));
                        }
                    }
                }
            }
        }

        // 開口。2.1 は StbOpenArrangement、2.0 は StbOpen と部材の StbOpenIdList
        for arrangement in arrangements {
            let host_kind: &'static [u8] =
                match self.attribute(arrangement, "kind_member").as_deref() {
                    Some("SLAB") => b"StbSlab",
                    Some("WALL") => b"StbWall",
                    _ => b"",
                };
            let id = self
                .attribute(arrangement, "id_member")
                .unwrap_or_default()
                .trim()
                .to_owned();
            let Some(host) = hosts.get(&(host_kind, id.clone())).copied() else {
                builder.missing(arrangement, "id_member", &id);
                continue;
            };
            builder.opening(arrangement, host, &sections)?;
        }
        for (open, host) in opens_of {
            let id = self
                .attribute(open, "id")
                .unwrap_or_default()
                .trim()
                .to_owned();
            let Some(element) = opens_20.get(&id).copied() else {
                builder.missing(open, "id", &id);
                continue;
            };
            builder.opening(element, host, &sections)?;
        }

        // 移さない、根と StbModel の下のもの
        for element in self.children(root).collect::<Vec<_>>() {
            if !matches!(self.name(element), b"StbCommon" | b"StbModel") {
                builder.unsupported(element);
            }
        }
        for element in self.children(model).collect::<Vec<_>>() {
            if !matches!(
                self.name(element),
                b"StbNodes" | b"StbAxes" | b"StbStories" | b"StbMembers" | b"StbSections"
            ) {
                builder.unsupported(element);
            }
        }
        Ok(builder.finish())
    }
}

impl Builder<'_, '_, '_> {
    /// 部材を足す
    fn member(
        &mut self,
        member: usize,
        kind: &MemberKind,
        nodes: &BTreeMap<String, NodePoint>,
        (sections, figures): (&BTreeMap<String, Vec<usize>>, &BTreeMap<usize, Figure>),
        building: Option<ElementRef>,
    ) -> Result<Option<ElementRef>, Error> {
        let file = self.file;
        let Some(target) = self.push(member)? else {
            return Ok(None);
        };
        self.class(target, kind.class, kind.predefined, member);
        self.identity(target, member);
        let mut sets = vec![self.set_of(member)];

        // 節点
        let node = |this: &mut Self, attribute: &str| -> Option<NodePoint> {
            let id = file.attribute(member, attribute)?.trim().to_owned();
            let found = nodes.get(&id).copied();
            if found.is_none() {
                this.missing(member, attribute, &id);
            }
            found
        };
        let rotate = number(file.attribute(member, "rotate")).unwrap_or(0.0);
        let mut extent = Extent::None;
        let (placement, story) = match kind.nodes {
            Nodes::BottomTop => {
                let bottom = node(self, "id_node_bottom");
                let top = node(self, "id_node_top");
                match (bottom, top) {
                    (Some(b), Some(t)) => {
                        extent = Extent::Line(distance(b.point, t.point));
                        (linear(b.point, t.point, rotate), t.story)
                    }
                    _ => (None, top.or(bottom).and_then(|n| n.story)),
                }
            }
            Nodes::StartEnd { upper } => {
                let start = node(self, "id_node_start");
                let end = node(self, "id_node_end");
                match (start, end) {
                    (Some(s), Some(e)) => {
                        let [_, _, sz] = s.point;
                        let [_, _, ez] = e.point;
                        let story = if upper && ez > sz { e.story } else { s.story };
                        extent = Extent::Line(distance(s.point, e.point));
                        (linear(s.point, e.point, rotate), story)
                    }
                    _ => (None, start.or(end).and_then(|n| n.story)),
                }
            }
            Nodes::OrderFirst | Nodes::OrderLast => {
                let text = file
                    .child(member, b"StbNodeIdOrder")
                    .map(|order| file.text(order))
                    .unwrap_or_default();
                let mut points = Vec::new();
                for id in ids(&text) {
                    match nodes.get(&id) {
                        Some(found) => points.push(*found),
                        None => self.missing(member, "StbNodeIdOrder", &id),
                    }
                }
                let story = match kind.nodes {
                    Nodes::OrderFirst => points.first(),
                    _ => points.last(),
                }
                .and_then(|n| n.story);
                let coordinates: Vec<[f64; 3]> = points.iter().map(|n| n.point).collect();
                let placement = planar(&coordinates);
                extent = Extent::Plate(coordinates);
                (placement, story)
            }
            Nodes::Single => match node(self, "id_node") {
                Some(n) => {
                    let [x, y, z] = n.point;
                    (
                        Some(Transform::translation(x, y, z).then(&about_z(rotate))),
                        n.story,
                    )
                }
                None => (None, None),
            },
        };
        match placement {
            Some(local) => self.place(target, None, local),
            None => {
                let at = self.at(member);
                self.cx.report(
                    Diagnostic::new(codes::STBRIDGE_PLACEMENT_DROPPED)
                        .with("element", self.name_of(member))
                        .at(at),
                );
            }
        }
        if let Some(container) = story.or(building) {
            self.parent(target, container, ParentKind::Contained);
        }

        // 断面
        let mut figure = None;
        if let Some(prefix) = kind.section {
            match self.section(member, prefix, "id_section", sections) {
                Some(section) => {
                    sets.push(self.set_of(section));
                    figure = figures.get(&section);
                }
                None => {
                    let id = file.attribute(member, "id_section").unwrap_or_default();
                    self.missing(member, "id_section", id.trim());
                }
            }
        }
        if let Some(Figure::Line {
            steel: Some(steel), ..
        }) = figure
        {
            sets.push(self.set_of(*steel));
        }
        self.building
            .components
            .properties
            .insert(target, Properties { sets });
        if let (Some(local), Some(figure)) = (placement, figure) {
            self.shape(target, member, kind.shape, figure, &extent, &local)?;
        }
        Ok(Some(target))
    }

    /// 部材に断面の形を持たせる。形を作れなければ `stbridge.shape.dropped` で知らせる
    fn shape(
        &mut self,
        target: ElementRef,
        member: usize,
        rule: Rule,
        figure: &Figure,
        extent: &Extent,
        local: &Transform,
    ) -> Result<(), Error> {
        let instance = member as u64;
        let data = match (rule, figure, extent) {
            (Rule::None, ..) => return Ok(()),
            (
                Rule::Centered | Rule::Top,
                Figure::Line {
                    outline, rounded, ..
                },
                Extent::Line(length),
            ) => {
                if *rounded {
                    let at = self.at(member);
                    self.cx.report(
                        Diagnostic::new(codes::STBRIDGE_FILLET_DROPPED)
                            .with("element", self.name_of(member))
                            .at(at),
                    );
                }
                shape::line(outline, *length, rule == Rule::Top, instance)
            }
            (Rule::Slab | Rule::Wall, Figure::Plate { thickness }, Extent::Plate(points)) => {
                match plate(points, *thickness, rule, local, instance) {
                    Some(data) => data,
                    None => {
                        self.dropped(member, "non_planar");
                        return Ok(());
                    }
                }
            }
            _ => {
                self.dropped(member, "figure_mismatch");
                return Ok(());
            }
        };
        let stopped = |error: GeometryError| match error {
            GeometryError::Stopped(stop) => Error::Stopped(stop),
            _ => Error::Invalid,
        };
        let geometry = zumen_building::Geometry::new(data, self.cx).map_err(stopped)?;
        self.building
            .insert_geometry(target, geometry, self.cx)
            .map_err(stopped)?;
        Ok(())
    }

    fn dropped(&mut self, member: usize, reason: &str) {
        let at = self.at(member);
        self.cx.report(
            Diagnostic::new(codes::STBRIDGE_SHAPE_DROPPED)
                .with("element", self.name_of(member))
                .with("reason", reason)
                .at(at),
        );
    }

    /// 部材の断面。名前が `prefix` で始まり、ID が合う断面のうち、構造種別（`kind_structure`）が名前に合うもの
    fn section(
        &self,
        member: usize,
        prefix: &str,
        attribute: &str,
        sections: &BTreeMap<String, Vec<usize>>,
    ) -> Option<usize> {
        let id = self.file.attribute(member, attribute)?;
        let candidates: Vec<usize> = sections
            .get(id.trim())?
            .iter()
            .copied()
            .filter(|&s| self.file.name(s).starts_with(prefix.as_bytes()))
            .collect();
        if let [only] = candidates.as_slice() {
            return Some(*only);
        }
        let structure = self
            .file
            .attribute(member, "kind_structure")
            .unwrap_or_default()
            .to_ascii_lowercase();
        candidates
            .iter()
            .copied()
            .find(|&s| {
                let name = String::from_utf8_lossy(self.file.name(s)).to_ascii_lowercase();
                name.strip_prefix(&prefix.to_ascii_lowercase())
                    .map(|rest| rest.trim_start_matches('_'))
                    .is_some_and(|rest| !structure.is_empty() && rest == structure)
            })
            .or_else(|| candidates.first().copied())
    }

    /// 開口を足す。配置は部材の面に対する位置（X・Y）と回転
    fn opening(
        &mut self,
        element: usize,
        host: ElementRef,
        sections: &BTreeMap<String, Vec<usize>>,
    ) -> Result<(), Error> {
        let Some(target) = self.push(element)? else {
            return Ok(());
        };
        self.class(target, "IfcOpeningElement", None, element);
        self.identity(target, element);
        self.building
            .components
            .opening
            .insert(target, Opening::Voids(host));
        let x = number(self.file.attribute(element, "position_X")).unwrap_or(0.0);
        let y = number(self.file.attribute(element, "position_Y")).unwrap_or(0.0);
        let rotate = number(self.file.attribute(element, "rotate")).unwrap_or(0.0);
        let relative_to = self
            .building
            .components
            .placement
            .get(host)
            .is_some()
            .then_some(host);
        self.place(
            target,
            relative_to,
            Transform::translation(x, y, 0.0).then(&about_z(rotate)),
        );
        let mut sets = vec![self.set_of(element)];
        if self.file.attribute(element, "id_section").is_some() {
            match self.section(element, OPEN_SECTION, "id_section", sections) {
                Some(section) => sets.push(self.set_of(section)),
                None => {
                    let id = self
                        .file
                        .attribute(element, "id_section")
                        .unwrap_or_default();
                    self.missing(element, "id_section", id.trim());
                }
            }
        }
        self.building
            .components
            .properties
            .insert(target, Properties { sets });
        Ok(())
    }

    /// 移さない要素を知らせて、建物モデルを返す
    fn finish(self) -> Building {
        for (element, at) in self.unsupported {
            self.cx.report(
                Diagnostic::new(codes::STBRIDGE_ELEMENT_UNSUPPORTED)
                    .with("element", element)
                    .at(at),
            );
        }
        self.building
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_guids() {
        let uuid = "0f8fad5b-d9cb-469f-a165-70867728950e";
        let compact = "0F8FAD5BD9CB469FA16570867728950E";
        assert_eq!(global_id(uuid), global_id(compact));
        assert!(global_id(uuid).is_some_and(|id| id.len() == 22));
        assert_eq!(global_id("not a guid"), None);
    }

    #[test]
    fn places_linear_members() {
        // 水平の梁: 局所の Z は材軸、局所の Y は上
        let beam = linear([0.0, 0.0, 0.0], [1000.0, 0.0, 0.0], 0.0).unwrap();
        let up = beam.apply([0.0, 1.0, 0.0]);
        assert!((up[2] - 1.0).abs() < 1e-12, "{up:?}");
        // 柱: 局所の X は全体の X。90 度回すと全体の Y
        let column = linear([0.0, 0.0, 0.0], [0.0, 0.0, 3000.0], 90.0).unwrap();
        let x = column.apply([1.0, 0.0, 0.0]);
        assert!((x[1] - 1.0).abs() < 1e-12, "{x:?}");
        assert_eq!(linear([1.0, 2.0, 3.0], [1.0, 2.0, 3.0], 0.0), None);
    }
}
