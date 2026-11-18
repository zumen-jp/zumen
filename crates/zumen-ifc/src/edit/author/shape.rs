//! 要素の形（芯の線と押し出し）を書く変更（#395）。
//!
//! 属性の並びは、IFC2X3 TC1 と IFC4 ADD2 TC1・IFC4X3 ADD2 の、形の資源・断面の資源・表現の資源の実体の定義による。
//!
//! - `IfcProduct`: 6 ObjectPlacement・7 Representation。表現を持つなら配置が要る
//! - `IfcProductDefinitionShape`: 1 Name・2 Description・3 Representations
//! - `IfcShapeRepresentation`: 1 ContextOfItems・2 RepresentationIdentifier・3 RepresentationType・4 Items
//! - `IfcExtrudedAreaSolid`: 1 SweptArea・2 Position・3 ExtrudedDirection・4 Depth。Position は IFC4X3 から
//!   省略できるが、どのバージョンでも書く。押し出す向きは断面の面に平行にできない
//! - `IfcRectangleProfileDef`: 1 ProfileType・2 ProfileName・3 Position・4 XDim・5 YDim
//! - `IfcCircleProfileDef`: 1 ProfileType・2 ProfileName・3 Position・4 Radius。Position は IFC4X3 から省略できる
//! - `IfcArbitraryClosedProfileDef`: 1 ProfileType・2 ProfileName・3 OuterCurve
//! - `IfcArbitraryProfileDefWithVoids`: 4 InnerCurves
//! - `IfcAxis2Placement3D`: 1 Location・2 Axis・3 RefDirection。`IfcAxis2Placement2D`: 1 Location・2 RefDirection

use zumen_sdk::{Cx, Error};

use super::{Category, Draft, Item, Stmt, Target, null, raw, real, text, unsupported};
use crate::edit::refs_of;

/// 要素の形。芯の線は `Axis`、押し出しは `Body` の表現に書き、元の表現は置き換える
#[derive(Clone, Debug, PartialEq)]
pub struct NewShape {
    /// 芯の線（`Axis` の表現の `IfcPolyline`）。2 点以上
    pub axis: Option<Vec<[f64; 2]>>,
    /// 押し出し（`Body` の表現の `IfcExtrudedAreaSolid`）。空なら `Body` を書かない
    pub solids: Vec<NewSolid>,
}

/// 断面の押し出し
#[derive(Clone, Debug, PartialEq)]
pub struct NewSolid {
    /// 断面
    pub profile: NewProfile,
    /// 立体の局所の配置。なければ要素の配置のまま
    pub position: Option<NewAxis3>,
    /// 局所の押し出す向き。z は 0 にできない
    pub direction: [f64; 3],
    /// 深さ（文書の長さの単位）。正
    pub depth: f64,
}

/// 3 次元の局所の配置。向きは元の比のまま書く
#[derive(Clone, Debug, PartialEq)]
pub struct NewAxis3 {
    /// 原点
    pub location: [f64; 3],
    /// 局所の z 軸
    pub axis: Option<[f64; 3]>,
    /// 局所の x 軸の参照方向
    pub reference: Option<[f64; 3]>,
}

/// 2 次元の局所の配置
#[derive(Clone, Debug, PartialEq)]
pub struct NewAxis2 {
    /// 原点
    pub location: [f64; 2],
    /// 局所の x 軸の参照方向
    pub reference: Option<[f64; 2]>,
}

/// 面の断面
#[derive(Clone, Debug, PartialEq)]
pub struct NewProfile {
    /// 断面の名前
    pub name: Option<String>,
    /// 形
    pub kind: NewProfileKind,
}

/// 断面の形。長さは文書の長さの単位
#[derive(Clone, Debug, PartialEq)]
pub enum NewProfileKind {
    /// 中心を原点とする矩形（`IfcRectangleProfileDef`）
    Rectangle {
        /// 局所の x の全幅。正
        x: f64,
        /// 局所の y の全高。正
        y: f64,
        /// 断面の局所の配置
        position: Option<NewAxis2>,
    },
    /// 中心を原点とする円（`IfcCircleProfileDef`）
    Circle {
        /// 半径。正
        radius: f64,
        /// 断面の局所の配置
        position: Option<NewAxis2>,
    },
    /// 折れ線の外輪と内輪（`IfcArbitraryClosedProfileDef`・`IfcArbitraryProfileDefWithVoids`）。
    /// 輪は 3 点以上で、始点に戻る点は書かない（始点と同じインスタンスで閉じる）
    Polygon {
        /// 外輪
        outer: Vec<[f64; 2]>,
        /// 内輪
        holes: Vec<Vec<[f64; 2]>>,
    },
}

impl Draft<'_> {
    /// 要素の形を置き換える。どこからも使われなくなった元の表現は除く
    pub(in crate::edit) fn set_shape(
        &mut self,
        target: Target,
        shape: &NewShape,
        cx: &mut Cx,
    ) -> Result<bool, Error> {
        const EDIT: &str = "shape";
        let Some((id, mut stmt, _)) = self.object(target, &[Category::Element], EDIT, cx) else {
            return Ok(false);
        };
        let reason = rejected(shape)
            .or_else(|| matches!(stmt.arg(6), None | Some(Item::Raw(_))).then_some("placement"));
        if let Some(value) = reason {
            cx.report(unsupported(EDIT, &stmt.name).with("value", value));
            return Ok(false);
        }
        let Some(model) = self.model_context() else {
            cx.report(unsupported(EDIT, &stmt.name).with("value", "context"));
            return Ok(false);
        };
        let mut representations = Vec::new();
        if let Some(points) = &shape.axis {
            let context = self.subcontext(model, "Axis", b".GRAPH_VIEW.");
            let line = self.polyline(points, false);
            representations.push(self.add(Stmt {
                name: b"IFCSHAPEREPRESENTATION".to_vec(),
                args: vec![
                    Item::Ref(context),
                    text("Axis"),
                    text("Curve2D"),
                    Item::List(vec![Item::Ref(line)]),
                ],
            }));
        }
        if !shape.solids.is_empty() {
            let context = self.subcontext(model, "Body", b".MODEL_VIEW.");
            let mut items = Vec::with_capacity(shape.solids.len());
            for solid in &shape.solids {
                items.push(Item::Ref(self.solid(solid)));
            }
            representations.push(self.add(Stmt {
                name: b"IFCSHAPEREPRESENTATION".to_vec(),
                args: vec![
                    Item::Ref(context),
                    text("Body"),
                    text("SweptSolid"),
                    Item::List(items),
                ],
            }));
        }
        let product = self.product_shape(&representations);
        let old = refs_of(stmt.arg(7));
        stmt.set(7, Item::Ref(product));
        self.put(id, stmt);
        self.collect(old, cx)?;
        Ok(true)
    }

    /// 押し出し。値は [`rejected`] で確かめてある
    fn solid(&mut self, solid: &NewSolid) -> u64 {
        let profile = self.profile(&solid.profile);
        let position = solid.position.clone().unwrap_or(NewAxis3 {
            location: [0.0; 3],
            axis: None,
            reference: None,
        });
        let position = self.axis3(&position);
        let direction = self.point(b"IFCDIRECTION", &solid.direction);
        self.add(Stmt {
            name: b"IFCEXTRUDEDAREASOLID".to_vec(),
            args: vec![
                Item::Ref(profile),
                Item::Ref(position),
                Item::Ref(direction),
                number(solid.depth),
            ],
        })
    }

    fn profile(&mut self, profile: &NewProfile) -> u64 {
        let name = profile.name.as_deref().map_or_else(null, text);
        let origin = NewAxis2 {
            location: [0.0; 2],
            reference: None,
        };
        let (kind, args): (&[u8], Vec<Item>) = match &profile.kind {
            NewProfileKind::Rectangle { x, y, position } => {
                let position = self.axis2(position.as_ref().unwrap_or(&origin));
                (
                    b"IFCRECTANGLEPROFILEDEF",
                    vec![Item::Ref(position), number(*x), number(*y)],
                )
            }
            NewProfileKind::Circle { radius, position } => {
                let position = self.axis2(position.as_ref().unwrap_or(&origin));
                (
                    b"IFCCIRCLEPROFILEDEF",
                    vec![Item::Ref(position), number(*radius)],
                )
            }
            NewProfileKind::Polygon { outer, holes } if holes.is_empty() => {
                let outer = self.polyline(outer, true);
                (b"IFCARBITRARYCLOSEDPROFILEDEF", vec![Item::Ref(outer)])
            }
            NewProfileKind::Polygon { outer, holes } => {
                let outer = self.polyline(outer, true);
                let holes = holes
                    .iter()
                    .map(|hole| Item::Ref(self.polyline(hole, true)))
                    .collect();
                (
                    b"IFCARBITRARYPROFILEDEFWITHVOIDS",
                    vec![Item::Ref(outer), Item::List(holes)],
                )
            }
        };
        let mut all = vec![raw(b".AREA."), name];
        all.extend(args);
        self.add(Stmt {
            name: kind.to_vec(),
            args: all,
        })
    }

    /// 2 次元の点の `IfcPolyline`。`closed` なら始点と同じインスタンスで閉じる
    fn polyline(&mut self, points: &[[f64; 2]], closed: bool) -> u64 {
        let mut items: Vec<Item> = points
            .iter()
            .map(|p| Item::Ref(self.point(b"IFCCARTESIANPOINT", p)))
            .collect();
        if closed && let Some(first) = items.first().cloned() {
            items.push(first);
        }
        self.add(Stmt {
            name: b"IFCPOLYLINE".to_vec(),
            args: vec![Item::List(items)],
        })
    }

    fn axis3(&mut self, axis: &NewAxis3) -> u64 {
        let location = self.point(b"IFCCARTESIANPOINT", &axis.location);
        let z = axis
            .axis
            .map_or_else(null, |v| Item::Ref(self.point(b"IFCDIRECTION", &v)));
        let x = axis
            .reference
            .map_or_else(null, |v| Item::Ref(self.point(b"IFCDIRECTION", &v)));
        self.add(Stmt {
            name: b"IFCAXIS2PLACEMENT3D".to_vec(),
            args: vec![Item::Ref(location), z, x],
        })
    }

    fn axis2(&mut self, axis: &NewAxis2) -> u64 {
        let location = self.point(b"IFCCARTESIANPOINT", &axis.location);
        let x = axis
            .reference
            .map_or_else(null, |v| Item::Ref(self.point(b"IFCDIRECTION", &v)));
        self.add(Stmt {
            name: b"IFCAXIS2PLACEMENT2D".to_vec(),
            args: vec![Item::Ref(location), x],
        })
    }

    /// 座標の並びの点か向き。値は有限と確かめてある
    fn point(&mut self, name: &[u8], values: &[f64]) -> u64 {
        self.add(Stmt {
            name: name.to_vec(),
            args: vec![Item::List(values.iter().map(|&v| number(v)).collect())],
        })
    }
}

/// 有限と確かめた値
fn number(value: f64) -> Item {
    real(value).unwrap_or_else(null)
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// 有限で 0 でない向き
fn direction(values: &[f64]) -> bool {
    finite(values) && values.iter().any(|&v| v != 0.0)
}

fn ring(points: &[[f64; 2]]) -> bool {
    points.len() >= 3 && points.iter().all(|p| finite(p))
}

fn axis2(position: Option<&NewAxis2>) -> bool {
    position.is_none_or(|p| finite(&p.location) && p.reference.is_none_or(|r| direction(&r)))
}

/// 書けない形なら、診断に書く理由
pub(in crate::edit) fn rejected(shape: &NewShape) -> Option<&'static str> {
    if shape.axis.is_none() && shape.solids.is_empty() {
        return Some("empty");
    }
    if let Some(points) = &shape.axis
        && (points.len() < 2 || !points.iter().all(|p| finite(p)))
    {
        return Some("axis");
    }
    for solid in &shape.solids {
        if !(solid.depth.is_finite() && solid.depth > 0.0) {
            return Some("depth");
        }
        if !direction(&solid.direction) || solid.direction[2] == 0.0 {
            return Some("direction");
        }
        if let Some(p) = &solid.position
            && !(finite(&p.location)
                && p.axis.is_none_or(|v| direction(&v))
                && p.reference.is_none_or(|v| direction(&v)))
        {
            return Some("position");
        }
        let positive = |v: f64| v.is_finite() && v > 0.0;
        let profile = match &solid.profile.kind {
            NewProfileKind::Rectangle { x, y, position } => {
                positive(*x) && positive(*y) && axis2(position.as_ref())
            }
            NewProfileKind::Circle { radius, position } => {
                positive(*radius) && axis2(position.as_ref())
            }
            NewProfileKind::Polygon { outer, holes } => {
                ring(outer) && holes.iter().all(|hole| ring(hole))
            }
        };
        if !profile {
            return Some("profile");
        }
    }
    None
}
