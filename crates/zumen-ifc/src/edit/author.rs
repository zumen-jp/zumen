//! 空間・要素・関係を足し、除き、置き直す変更（#897）。
//!
//! 書く実体と属性の並びは、lift と同じく手書きのバージョンの表で持つ。スキーマは同梱せず、スキーマから生成もしない。
//! 属性の位置は、buildingSMART が公開する各バージョンの仕様の実体の節（IFC2X3 TC1 の 5.4・7.1・7.2、IFC4 ADD2 TC1 と
//! IFC4X3 ADD2 の 5.4・8.18・8.19 の実体の定義）による。表にない実体と、そのバージョンにない実体は書かずに診断する。

use std::collections::{BTreeSet, VecDeque};

use zumen_base::Kind;
use zumen_building::{GlobalId, Transform};
use zumen_sdk::{Cx, Error};

use super::{Draft, Item, Stmt, refs_in, refs_of, text, text_of, unsupported};
use crate::building::{Category, Version, category};
use crate::stopped;

/// 変更が指す要素。文書にあるインスタンスか、同じ要求の中で足した要素
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// 文書にあるインスタンスの番号
    Instance(u64),
    /// 同じ要求の中で、足す変更（[`super::Edit::Space`]・[`super::Edit::Element`]）が足した要素。足した順に 0 から
    Added(usize),
}

/// 足す空間の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceKind {
    /// 敷地（`IfcSite`）
    Site,
    /// 建物（`IfcBuilding`）
    Building,
    /// 階（`IfcBuildingStorey`）
    Storey,
}

/// 配置。親の配置を持つ要素からの、局所の変換
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewPlacement {
    /// 親の配置を持つ要素。なければプロジェクトの空間に置く
    pub relative_to: Option<Target>,
    /// 局所の変換。回転は正規直交で、行列式が 1 のもの
    pub local: Transform,
}

/// 足す空間
#[derive(Clone, Debug, PartialEq)]
pub struct NewSpace {
    /// 空間の種類
    pub kind: SpaceKind,
    /// 名前
    pub name: Option<String>,
    /// 集約の親（プロジェクトか空間）
    pub parent: Target,
    /// 配置。なければ、親の配置があればそこに恒等の変換で置く
    pub placement: Option<NewPlacement>,
    /// 標高（文書の長さの単位）。階は 10 Elevation、建物は 10 ElevationOfRefHeight、敷地は 12 RefElevation
    pub elevation: Option<f64>,
    /// GlobalId。なければ文書のハッシュと足した数から作る
    pub global_id: Option<GlobalId>,
}

/// 足す要素
#[derive(Clone, Debug, PartialEq)]
pub struct NewElement {
    /// IFC のクラスの名前（`IfcWall` など）。書ける実体の表にあるもの
    pub class: String,
    /// 定義された種類（`SOLIDWALL` など）。そのバージョンの実体が持たなければ書かずに診断する
    pub predefined: Option<String>,
    /// 名前
    pub name: Option<String>,
    /// 所属する空間（`IfcRelContainedInSpatialStructure`）
    pub container: Option<Target>,
    /// 配置。なければ、所属する空間の配置があればそこに恒等の変換で置く
    pub placement: Option<NewPlacement>,
    /// GlobalId。なければ文書のハッシュと足した数から作る
    pub global_id: Option<GlobalId>,
}

/// そのバージョンに実体がない
const ABSENT: u8 = 0;

/// 書ける要素の実体。`arity` は IFC2X3・IFC4・IFC4X3 の順の属性の数、`predefined` は定義された種類の位置（1 から、
/// 0 は持たない）、`required` は定義された種類が必須のバージョン。属性 1〜8 は `IfcElement` と同じ（1 GlobalId・
/// 2 OwnerHistory・3 Name・4 Description・5 ObjectType・6 ObjectPlacement・7 Representation・8 Tag）。9 より後で、
/// 定義された種類でない属性（扉と窓の大きさ・杭の工法など）は `$` にする
struct Writable {
    name: &'static str,
    arity: [u8; 3],
    predefined: [u8; 3],
    required: [bool; 3],
}

const fn writable(
    name: &'static str,
    arity: [u8; 3],
    predefined: [u8; 3],
    required: [bool; 3],
) -> Writable {
    Writable {
        name,
        arity,
        predefined,
        required,
    }
}

const NO: [bool; 3] = [false, false, false];

/// 書ける要素の表。IFC2X3 の `IfcRoof`・`IfcStair`・`IfcRamp` は 9 ShapeType、`IfcFooting`・`IfcPile` は
/// 9 PredefinedType が必須（IFC2X3 TC1 の 5.4 の各実体）
const WRITABLE: &[Writable] = &[
    writable("IfcWall", [8, 9, 9], [0, 9, 9], NO),
    writable("IfcSlab", [9, 9, 9], [9, 9, 9], NO),
    writable("IfcColumn", [8, 9, 9], [0, 9, 9], NO),
    writable("IfcBeam", [8, 9, 9], [0, 9, 9], NO),
    writable("IfcMember", [8, 9, 9], [0, 9, 9], NO),
    writable("IfcPlate", [8, 9, 9], [0, 9, 9], NO),
    writable("IfcCovering", [9, 9, 9], [9, 9, 9], NO),
    writable("IfcRailing", [9, 9, 9], [9, 9, 9], NO),
    writable("IfcRoof", [9, 9, 9], [9, 9, 9], [true, false, false]),
    writable("IfcStair", [9, 9, 9], [9, 9, 9], [true, false, false]),
    writable("IfcRamp", [9, 9, 9], [9, 9, 9], [true, false, false]),
    writable("IfcFooting", [9, 9, 9], [9, 9, 9], [true, false, false]),
    writable("IfcPile", [10, 10, 10], [9, 9, 9], [true, false, false]),
    writable("IfcDoor", [10, 13, 13], [0, 11, 11], NO),
    writable("IfcWindow", [10, 13, 13], [0, 11, 11], NO),
    writable("IfcOpeningElement", [8, 9, 9], [0, 9, 9], NO),
    writable("IfcBuildingElementProxy", [9, 9, 9], [0, 9, 9], NO),
];

/// バージョンの列の値
fn pick<T: Copy>(row: [T; 3], version: Version) -> T {
    let [ifc2x3, ifc4, ifc4x3] = row;
    match version {
        Version::Ifc2x3 => ifc2x3,
        Version::Ifc4 => ifc4,
        Version::Ifc4x3 => ifc4x3,
    }
}

/// 属性 `at`（1 から）に書く。並びの外なら書かない
fn set_at(args: &mut [Item], at: usize, item: Item) {
    if let Some(slot) = at.checked_sub(1).and_then(|at| args.get_mut(at)) {
        *slot = item;
    }
}

fn raw(bytes: &[u8]) -> Item {
    Item::Raw(bytes.to_vec())
}

fn null() -> Item {
    raw(b"$")
}

/// 実数。有限でなければ `None`。`-0` は `0` にする
fn real(value: f64) -> Option<Item> {
    if !value.is_finite() {
        return None;
    }
    let value = if value == 0.0 { 0.0 } else { value };
    let mut written = format!("{value}");
    if !written.contains('.') {
        written.push('.');
    }
    Some(Item::Raw(written.into_bytes()))
}

fn enumeration(value: &str) -> Option<Item> {
    let valid = value
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_');
    valid.then(|| Item::Raw(format!(".{}.", value.to_ascii_uppercase()).into_bytes()))
}

/// 正規直交で行列式が 1 の回転か
fn rigid(local: &Transform) -> bool {
    const TOLERANCE: f64 = 1e-9;
    let m = local.0;
    if m.iter().flatten().any(|v| !v.is_finite()) {
        return false;
    }
    let column = |k: usize| -> [f64; 3] {
        let pick = |row: [f64; 4]| row.get(k).copied().unwrap_or(f64::NAN);
        [pick(m[0]), pick(m[1]), pick(m[2])]
    };
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let (x, y, z) = (column(0), column(1), column(2));
    let cross = [
        x[1] * y[2] - x[2] * y[1],
        x[2] * y[0] - x[0] * y[2],
        x[0] * y[1] - x[1] * y[0],
    ];
    (dot(x, x) - 1.0).abs() < TOLERANCE
        && (dot(y, y) - 1.0).abs() < TOLERANCE
        && (dot(z, z) - 1.0).abs() < TOLERANCE
        && dot(x, y).abs() < TOLERANCE
        && (dot(cross, z) - 1.0).abs() < TOLERANCE
}

/// 関係の実体の、関係させる側と関係する側の属性の位置
#[derive(Clone, Copy)]
struct Relation {
    name: &'static [u8],
    relating: usize,
    related: usize,
}

/// `IfcRelAggregates`: 5 RelatingObject・6 RelatedObjects
const AGGREGATES: Relation = Relation {
    name: b"IFCRELAGGREGATES",
    relating: 5,
    related: 6,
};
/// `IfcRelContainedInSpatialStructure`: 5 RelatedElements・6 RelatingStructure
const CONTAINED: Relation = Relation {
    name: b"IFCRELCONTAINEDINSPATIALSTRUCTURE",
    relating: 6,
    related: 5,
};
/// `IfcRelDefinesByType`: 5 RelatedObjects・6 RelatingType
const DEFINES_BY_TYPE: Relation = Relation {
    name: b"IFCRELDEFINESBYTYPE",
    relating: 6,
    related: 5,
};
/// `IfcRelVoidsElement`: 5 RelatingBuildingElement・6 RelatedOpeningElement
const VOIDS: &[u8] = b"IFCRELVOIDSELEMENT";
/// `IfcRelFillsElement`: 5 RelatingOpeningElement・6 RelatedBuildingElement
const FILLS: &[u8] = b"IFCRELFILLSELEMENT";

/// 除くときに、ほかから使われなくなっても残す実体。文書の定義と、材料のような共有の定義
const KEPT: &[&[u8]] = &[
    b"IFCOWNERHISTORY",
    b"IFCGEOMETRICREPRESENTATIONCONTEXT",
    b"IFCGEOMETRICREPRESENTATIONSUBCONTEXT",
    b"IFCMATERIAL",
    b"IFCUNITASSIGNMENT",
];

impl Draft<'_> {
    fn version(&self) -> Option<Version> {
        Version::of(self.file.schema())
    }

    /// 指す要素のインスタンスの番号。ないか除いたものなら `None`
    fn resolve(&self, target: Target) -> Option<u64> {
        let id = match target {
            Target::Instance(id) => id,
            Target::Added(at) => *self.handles.get(at)?,
        };
        (!self.removed.contains(&id) && self.stmt(id).is_some()).then_some(id)
    }

    /// 指す要素と、その分け方。要素でないか、分け方が `allowed` になければ診断して `None`
    fn object(
        &self,
        target: Target,
        allowed: &[Category],
        edit: &str,
        cx: &mut Cx,
    ) -> Option<(u64, Stmt, Category)> {
        let found = self.resolve(target).and_then(|id| {
            let stmt = self.stmt(id)?;
            let category = category(&stmt.name, self.file.schema())?;
            allowed.contains(&category).then_some((id, stmt, category))
        });
        if found.is_none() {
            let kind = self
                .resolve(target)
                .map(|id| self.kind(id))
                .unwrap_or_default();
            cx.report(unsupported(edit, &kind));
        }
        found
    }

    /// 足す `IfcRoot` の 2 OwnerHistory。プロジェクトの所有履歴を使う。IFC2X3 で所有履歴がなければ、診断して `None`
    fn history(&self, edit: &str, kind: &[u8], cx: &mut Cx) -> Option<Item> {
        let found = self
            .project
            .and_then(|project| self.stmt(project))
            .and_then(|project| match project.arg(2) {
                Some(Item::Ref(id)) => Some(Item::Ref(*id)),
                _ => None,
            });
        match (found, self.version()) {
            (Some(history), _) => Some(history),
            (None, Some(Version::Ifc4 | Version::Ifc4x3)) => Some(null()),
            _ => {
                cx.report(unsupported(edit, kind).with("missing", "OwnerHistory"));
                None
            }
        }
    }

    /// 足す GlobalId。渡された値は、文書と、この要求で足したものに重複がなければ使う
    fn new_global_id(
        &mut self,
        given: Option<GlobalId>,
        edit: &str,
        kind: &[u8],
        cx: &mut Cx,
    ) -> Result<Option<Item>, Error> {
        let Some(given) = given else {
            return Ok(Some(self.global_id()));
        };
        let written = given.to_string();
        if self.global_ids.is_none() {
            let mut found = BTreeSet::new();
            for id in self.file.ids() {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if let Some(global_id) = self
                    .stmt(id)
                    .and_then(|stmt| text_of(stmt.arg(1)?))
                    .filter(|g| g.len() == 22)
                {
                    found.insert(global_id);
                }
            }
            self.global_ids = Some(found);
        }
        let Some(taken) = self.global_ids.as_mut() else {
            return Ok(None);
        };
        if !taken.insert(written.clone()) {
            cx.report(unsupported(edit, kind).with("global_id", written));
            return Ok(None);
        }
        Ok(Some(text(&written)))
    }

    /// 要素の 6 ObjectPlacement の `IfcLocalPlacement`
    fn placement_of(&self, id: u64) -> Option<u64> {
        let stmt = self.stmt(id)?;
        match stmt.arg(6) {
            Some(Item::Ref(placement)) if self.kind(*placement) == b"IFCLOCALPLACEMENT" => {
                Some(*placement)
            }
            _ => None,
        }
    }

    /// 配置の `IfcAxis2Placement3D` を足す
    fn axis_placement(&mut self, local: &Transform) -> Option<u64> {
        let m = local.0;
        let point = |values: [f64; 3]| -> Option<Item> {
            Some(Item::List(vec![
                real(values[0])?,
                real(values[1])?,
                real(values[2])?,
            ]))
        };
        let location = point([m[0][3], m[1][3], m[2][3]])?;
        let axis = [m[0][2], m[1][2], m[2][2]];
        let reference = [m[0][0], m[1][0], m[2][0]];
        let location = self.add(Stmt {
            name: b"IFCCARTESIANPOINT".to_vec(),
            args: vec![location],
        });
        let (axis, reference) = if axis == [0.0, 0.0, 1.0] && reference == [1.0, 0.0, 0.0] {
            (null(), null())
        } else {
            let axis = point(axis)?;
            let reference = point(reference)?;
            let axis = self.add(Stmt {
                name: b"IFCDIRECTION".to_vec(),
                args: vec![axis],
            });
            let reference = self.add(Stmt {
                name: b"IFCDIRECTION".to_vec(),
                args: vec![reference],
            });
            (Item::Ref(axis), Item::Ref(reference))
        };
        Some(self.add(Stmt {
            name: b"IFCAXIS2PLACEMENT3D".to_vec(),
            args: vec![Item::Ref(location), axis, reference],
        }))
    }

    /// 配置の親の `IfcLocalPlacement`。親がなければ `Some(None)`、親に配置がなければ診断して `None`
    fn parent_placement(
        &self,
        relative_to: Option<Target>,
        edit: &str,
        cx: &mut Cx,
    ) -> Option<Option<u64>> {
        let Some(target) = relative_to else {
            return Some(None);
        };
        let found = self.resolve(target).and_then(|id| self.placement_of(id));
        if found.is_none() {
            let kind = self
                .resolve(target)
                .map(|id| self.kind(id))
                .unwrap_or_default();
            cx.report(unsupported(edit, &kind).with("missing", "ObjectPlacement"));
        }
        found.map(Some)
    }

    /// 新しい `IfcLocalPlacement` を足す。変換が書けなければ診断して `None`
    fn new_placement(
        &mut self,
        placement: &NewPlacement,
        edit: &str,
        kind: &[u8],
        cx: &mut Cx,
    ) -> Option<u64> {
        let parent = self.parent_placement(placement.relative_to, edit, cx)?;
        if !rigid(&placement.local) {
            cx.report(unsupported(edit, kind).with("value", "placement"));
            return None;
        }
        let axis = self.axis_placement(&placement.local)?;
        Some(self.add(Stmt {
            name: b"IFCLOCALPLACEMENT".to_vec(),
            args: vec![parent.map_or_else(null, Item::Ref), Item::Ref(axis)],
        }))
    }

    /// 配置。渡されなければ、`fallback` の配置に恒等の変換で置く。`fallback` にも配置がなければ `$`
    fn placement_or(
        &mut self,
        placement: Option<&NewPlacement>,
        fallback: Option<u64>,
        edit: &str,
        kind: &[u8],
        cx: &mut Cx,
    ) -> Option<Item> {
        if let Some(placement) = placement {
            return self.new_placement(placement, edit, kind, cx).map(Item::Ref);
        }
        let Some(parent) = fallback.and_then(|id| self.placement_of(id)) else {
            return Some(null());
        };
        let axis = self.axis_placement(&Transform::IDENTITY)?;
        Some(Item::Ref(self.add(Stmt {
            name: b"IFCLOCALPLACEMENT".to_vec(),
            args: vec![Item::Ref(parent), Item::Ref(axis)],
        })))
    }

    /// 関係 `relation` で `relating` に `related` を結ぶ。`relating` の関係があれば、関係する側の列へ足し、なければ関係を足す
    fn join(&mut self, relation: Relation, relating: u64, related: u64, history: Item) {
        let existing = self.referrers(relating).into_iter().find(|&rel| {
            self.kind(rel) == relation.name
                && self
                    .stmt(rel)
                    .is_some_and(|stmt| refs_of(stmt.arg(relation.relating)) == [relating])
        });
        if let Some(rel) = existing
            && let Some(mut stmt) = self.stmt(rel)
        {
            let mut items = match stmt.arg(relation.related) {
                Some(Item::List(items)) => items.clone(),
                _ => Vec::new(),
            };
            items.push(Item::Ref(related));
            stmt.set(relation.related, Item::List(items));
            self.put(rel, stmt);
            return;
        }
        let mut args = vec![self.global_id(), history, null(), null(), null(), null()];
        if let Some(slot) = args.get_mut(relation.relating - 1) {
            *slot = Item::Ref(relating);
        }
        if let Some(slot) = args.get_mut(relation.related - 1) {
            *slot = Item::List(vec![Item::Ref(related)]);
        }
        self.add(Stmt {
            name: relation.name.to_vec(),
            args,
        });
    }

    /// 関係 `relation` の関係する側から `related` を外す。並びが空になれば関係を除く
    fn detach(&mut self, relation: Relation, related: u64) {
        for rel in self.referrers(related) {
            if self.kind(rel) != relation.name {
                continue;
            }
            let Some(mut stmt) = self.stmt(rel) else {
                continue;
            };
            let Some(Item::List(items)) = stmt.arg(relation.related) else {
                continue;
            };
            if !items.contains(&Item::Ref(related)) {
                continue;
            }
            let rest: Vec<Item> = items
                .iter()
                .filter(|item| **item != Item::Ref(related))
                .cloned()
                .collect();
            if rest.is_empty() {
                self.drop_instance(rel);
            } else {
                stmt.set(relation.related, Item::List(rest));
                self.put(rel, stmt);
            }
        }
    }

    /// 空間を足す
    pub(super) fn add_space(&mut self, space: &NewSpace, cx: &mut Cx) -> Result<bool, Error> {
        let (name, arity, elevation_at): (&[u8], usize, usize) = match space.kind {
            SpaceKind::Site => (b"IFCSITE", 14, 12),
            SpaceKind::Building => (b"IFCBUILDING", 12, 10),
            SpaceKind::Storey => (b"IFCBUILDINGSTOREY", 10, 10),
        };
        if self.version().is_none() {
            cx.report(unsupported("space", name));
            return Ok(false);
        }
        let Some((parent, ..)) = self.object(
            space.parent,
            &[Category::Project, Category::Spatial],
            "space",
            cx,
        ) else {
            return Ok(false);
        };
        let Some(history) = self.history("space", name, cx) else {
            return Ok(false);
        };
        let Some(global_id) = self.new_global_id(space.global_id, "space", name, cx)? else {
            return Ok(false);
        };
        let elevation = match space.elevation {
            Some(value) => match real(value) {
                Some(item) => Some(item),
                None => {
                    cx.report(unsupported("space", name).with("value", "elevation"));
                    return Ok(false);
                }
            },
            None => None,
        };
        let Some(placement) =
            self.placement_or(space.placement.as_ref(), Some(parent), "space", name, cx)
        else {
            return Ok(false);
        };
        let mut args = vec![null(); arity];
        set_at(&mut args, 1, global_id);
        set_at(&mut args, 2, history.clone());
        set_at(&mut args, 3, space.name.as_deref().map_or_else(null, text));
        set_at(&mut args, 6, placement);
        // 9 CompositionType は IFC2X3 で必須。IFC4 と IFC4X3 でも同じ値を書ける
        set_at(&mut args, 9, raw(b".ELEMENT."));
        if let Some(elevation) = elevation {
            set_at(&mut args, elevation_at, elevation);
        }
        let id = self.add(Stmt {
            name: name.to_vec(),
            args,
        });
        self.join(AGGREGATES, parent, id, history);
        self.handles.push(id);
        Ok(true)
    }

    /// 要素を足す
    pub(super) fn add_element(&mut self, element: &NewElement, cx: &mut Cx) -> Result<bool, Error> {
        let kind = element.class.to_ascii_uppercase().into_bytes();
        let Some(version) = self.version() else {
            cx.report(unsupported("element", &kind));
            return Ok(false);
        };
        let entity = WRITABLE
            .iter()
            .find(|w| w.name.eq_ignore_ascii_case(&element.class))
            .filter(|w| pick(w.arity, version) != ABSENT);
        let Some(entity) = entity else {
            cx.report(unsupported("element", &kind));
            return Ok(false);
        };
        let predefined_at = usize::from(pick(entity.predefined, version));
        let predefined = match (&element.predefined, predefined_at) {
            (None, 0) => None,
            (None, at) => Some((
                at,
                if pick(entity.required, version) {
                    raw(b".NOTDEFINED.")
                } else {
                    null()
                },
            )),
            (Some(value), at) if at > 0 => match enumeration(value) {
                Some(item) => Some((at, item)),
                None => {
                    cx.report(unsupported("element", &kind).with("value", "predefined"));
                    return Ok(false);
                }
            },
            (Some(_), _) => {
                cx.report(unsupported("element", &kind).with("value", "predefined"));
                return Ok(false);
            }
        };
        let container = match element.container {
            Some(target) => match self.object(target, &[Category::Spatial], "element", cx) {
                Some((id, ..)) => Some(id),
                None => return Ok(false),
            },
            None => None,
        };
        let Some(history) = self.history("element", &kind, cx) else {
            return Ok(false);
        };
        let Some(global_id) = self.new_global_id(element.global_id, "element", &kind, cx)? else {
            return Ok(false);
        };
        let Some(placement) =
            self.placement_or(element.placement.as_ref(), container, "element", &kind, cx)
        else {
            return Ok(false);
        };
        let mut args = vec![null(); usize::from(pick(entity.arity, version))];
        set_at(&mut args, 1, global_id);
        set_at(&mut args, 2, history.clone());
        set_at(
            &mut args,
            3,
            element.name.as_deref().map_or_else(null, text),
        );
        set_at(&mut args, 6, placement);
        if let Some((at, item)) = predefined
            && let Some(slot) = args.get_mut(at - 1)
        {
            *slot = item;
        }
        let id = self.add(Stmt { name: kind, args });
        if let Some(container) = container {
            self.join(CONTAINED, container, id, history);
        }
        self.handles.push(id);
        Ok(true)
    }

    /// 要素を置き直す。今の配置がこの要素と子の配置だけに使われていれば、その場で書き直し、子の配置は付いてくる
    pub(super) fn place(
        &mut self,
        target: Target,
        placement: &NewPlacement,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let Some((id, mut stmt, _)) = self.object(
            target,
            &[Category::Spatial, Category::Element, Category::Grid],
            "place",
            cx,
        ) else {
            return Ok(false);
        };
        let Some(parent) = self.parent_placement(placement.relative_to, "place", cx) else {
            return Ok(false);
        };
        if !rigid(&placement.local) {
            cx.report(unsupported("place", &stmt.name).with("value", "placement"));
            return Ok(false);
        }
        let current = self.placement_of(id);
        // 親の配置が、この要素の配置の子孫なら輪になる
        if let (Some(current), Some(mut at)) = (current, parent) {
            let mut seen = BTreeSet::new();
            loop {
                cx.check(Kind::Work, 1)
                    .map_err(|stop| stopped(cx, stop, None))?;
                if at == current {
                    cx.report(unsupported("place", &stmt.name).with("value", "relative_to"));
                    return Ok(false);
                }
                if !seen.insert(at) {
                    break;
                }
                match self.stmt(at).and_then(|s| s.arg(1).cloned()) {
                    Some(Item::Ref(next)) => at = next,
                    _ => break,
                }
            }
        }
        let Some(axis) = self.axis_placement(&placement.local) else {
            cx.report(unsupported("place", &stmt.name).with("value", "placement"));
            return Ok(false);
        };
        let new_stmt = Stmt {
            name: b"IFCLOCALPLACEMENT".to_vec(),
            args: vec![parent.map_or_else(null, Item::Ref), Item::Ref(axis)],
        };
        let exclusive = current.filter(|&current| {
            self.referrers(current)
                .into_iter()
                .all(|by| by == id || self.kind(by) == b"IFCLOCALPLACEMENT")
        });
        match exclusive {
            Some(current) => {
                let old = self.stmt(current);
                self.put(current, new_stmt);
                if let Some(old) = old {
                    self.collect(refs_in(&old.args), cx)?;
                }
            }
            None => {
                let new_id = self.add(new_stmt);
                stmt.set(6, Item::Ref(new_id));
                self.put(id, stmt);
            }
        }
        Ok(true)
    }

    /// 要素の所属する空間を変える。空間は集約（`IfcRelAggregates`）、ほかは包含で結ぶ
    pub(super) fn contain(
        &mut self,
        target: Target,
        space: Target,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let Some((id, stmt, category)) = self.object(
            target,
            &[Category::Spatial, Category::Element, Category::Grid],
            "contain",
            cx,
        ) else {
            return Ok(false);
        };
        let (relation, allowed): (Relation, &[Category]) = if category == Category::Spatial {
            (AGGREGATES, &[Category::Project, Category::Spatial])
        } else {
            (CONTAINED, &[Category::Spatial])
        };
        let Some((container, ..)) = self.object(space, allowed, "contain", cx) else {
            return Ok(false);
        };
        if container == id {
            let found = self.kind(container);
            cx.report(unsupported("contain", &found).with("value", "container"));
            return Ok(false);
        }
        let history = stmt.arg(2).cloned().unwrap_or_else(null);
        self.detach(relation, id);
        self.join(relation, container, id, history);
        Ok(true)
    }

    /// 要素の型を変える。`None` は型を外す
    pub(super) fn retype(
        &mut self,
        target: Target,
        type_of: Option<Target>,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let Some((id, stmt, _)) =
            self.object(target, &[Category::Element, Category::Spatial], "type", cx)
        else {
            return Ok(false);
        };
        let type_id = match type_of {
            Some(type_of) => {
                match self.object(type_of, &[Category::Type, Category::Style], "type", cx) {
                    Some((type_id, ..)) => Some(type_id),
                    None => return Ok(false),
                }
            }
            None => None,
        };
        let history = stmt.arg(2).cloned().unwrap_or_else(null);
        self.detach(DEFINES_BY_TYPE, id);
        if let Some(type_id) = type_id {
            self.join(DEFINES_BY_TYPE, type_id, id, history);
        }
        Ok(true)
    }

    /// 開口で要素に穴をあける（`IfcRelVoidsElement`）か、建具で開口を埋める（`IfcRelFillsElement`）
    pub(super) fn relate_opening(
        &mut self,
        fills: bool,
        relating: Target,
        related: Target,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        let edit = if fills { "fill" } else { "void" };
        let Some((relating, relating_stmt, _)) =
            self.object(relating, &[Category::Element], edit, cx)
        else {
            return Ok(false);
        };
        let Some((related, related_stmt, _)) = self.object(related, &[Category::Element], edit, cx)
        else {
            return Ok(false);
        };
        let opening_side = if fills { &relating_stmt } else { &related_stmt };
        if opening_side.name != b"IFCOPENINGELEMENT" || relating == related {
            cx.report(unsupported(edit, &opening_side.name));
            return Ok(false);
        }
        let name: &[u8] = if fills { FILLS } else { VOIDS };
        // 開口が穴をあける要素は 1 つ（IFC の VoidsElements は 1 つまで）。埋める側も 1 つにする
        let unique = if fills { relating } else { related };
        let taken = self.referrers(unique).into_iter().any(|rel| {
            self.kind(rel) == name
                && self
                    .stmt(rel)
                    .is_some_and(|stmt| refs_of(stmt.arg(if fills { 5 } else { 6 })) == [unique])
        });
        if taken {
            cx.report(unsupported(edit, &opening_side.name).with("value", "taken"));
            return Ok(false);
        }
        let history = relating_stmt.arg(2).cloned().unwrap_or_else(null);
        let global_id = self.global_id();
        self.add(Stmt {
            name: name.to_vec(),
            args: vec![
                global_id,
                history,
                null(),
                null(),
                Item::Ref(relating),
                Item::Ref(related),
            ],
        });
        Ok(true)
    }

    /// 要素を除く。要素だけが使う配置・表現・定義・関係も除き、ほかの要素と共有するものは残す。要素の開口も除く
    pub(super) fn remove(&mut self, target: Target, cx: &mut Cx) -> Result<bool, Error> {
        let Some((id, stmt, _)) = self.object(
            target,
            &[
                Category::Spatial,
                Category::Element,
                Category::Type,
                Category::Style,
                Category::Group,
                Category::Grid,
            ],
            "remove",
            cx,
        ) else {
            return Ok(false);
        };
        // 子や所属する要素を持つ空間は除かない
        let holds = self.referrers(id).into_iter().any(|rel| {
            let kind = self.kind(rel);
            let relating = if kind == AGGREGATES.name {
                AGGREGATES.relating
            } else if kind == CONTAINED.name {
                CONTAINED.relating
            } else {
                return false;
            };
            self.stmt(rel)
                .is_some_and(|rel| refs_of(rel.arg(relating)) == [id])
        });
        if holds {
            cx.report(unsupported("remove", &stmt.name).with("value", "children"));
            return Ok(false);
        }
        let mut queue = VecDeque::from([id]);
        let mut doomed = BTreeSet::new();
        let mut loose = Vec::new();
        while let Some(at) = queue.pop_front() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            if !doomed.insert(at) {
                continue;
            }
            for by in self.referrers(at) {
                if doomed.contains(&by) {
                    continue;
                }
                let kind = self.kind(by);
                if !kind.starts_with(b"IFCREL") {
                    cx.report(unsupported("remove", &kind).with("value", "referenced"));
                    return Ok(false);
                }
                let Some(mut rel) = self.stmt(by) else {
                    continue;
                };
                // 要素の開口は、要素とともに除く
                if kind == VOIDS && refs_of(rel.arg(5)) == [at] {
                    queue.extend(refs_of(rel.arg(6)));
                }
                let mut whole = false;
                for slot in rel.args.iter_mut() {
                    match slot {
                        Item::Ref(r) if *r == at => whole = true,
                        Item::List(items) if items.contains(&Item::Ref(at)) => {
                            items.retain(|item| *item != Item::Ref(at));
                            whole |= items.is_empty();
                        }
                        _ => {}
                    }
                }
                if whole {
                    queue.push_back(by);
                } else {
                    self.put(by, rel);
                }
            }
        }
        for &at in &doomed {
            if let Some(stmt) = self.stmt(at) {
                loose.extend(refs_in(&stmt.args));
            }
        }
        for &at in &doomed {
            self.drop_instance(at);
        }
        self.collect(loose, cx)?;
        Ok(true)
    }

    /// インスタンスを除き、逆向きの参照も直す
    fn drop_instance(&mut self, id: u64) {
        if let Some(stmt) = self.stmt(id) {
            for target in refs_in(&stmt.args) {
                if let Some(set) = self.referrers.get_mut(&target) {
                    set.remove(&id);
                }
            }
        }
        self.removed.insert(id);
    }

    /// どこからも使われなくなったインスタンスを除く。要素・型・空間などの `IfcRoot` の対象と、共有の定義は残す
    fn collect(&mut self, start: Vec<u64>, cx: &mut Cx) -> Result<(), Error> {
        let mut queue = VecDeque::from(start);
        while let Some(at) = queue.pop_front() {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, None))?;
            if self.removed.contains(&at) || !self.referrers(at).is_empty() {
                continue;
            }
            let Some(stmt) = self.stmt(at) else {
                continue;
            };
            if category(&stmt.name, self.file.schema()).is_some()
                || KEPT.contains(&stmt.name.as_slice())
                || stmt.name.ends_with(b"UNIT")
            {
                continue;
            }
            queue.extend(refs_in(&stmt.args));
            self.drop_instance(at);
        }
        Ok(())
    }
}
