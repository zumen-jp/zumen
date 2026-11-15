//! 材料からの表示。割り当て・文脈・元値を保ち、形への割り振りや反射式の評価と分ける。

use super::*;
use zumen_base::Kind;

/// 材料の割り当てを持つ位置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialAssignmentLevel {
    /// 要素の指定
    Occurrence,
    /// 型から継承した指定
    Type,
}

/// 材料の表示を使う文脈
#[derive(Clone, Debug, PartialEq)]
pub enum GeometryMaterialContext {
    /// 幾何の値を持たない文脈
    General {
        /// 元の識別名
        identifier: Option<String>,
        /// 元の種類の名
        kind: Option<String>,
    },
    /// 幾何の文脈。位置・尺度・単位は未適用
    Geometric {
        /// 親と子の元の値
        definition: Box<RepresentationContext>,
        /// 元の長さの単位
        length_unit: Option<Unit>,
    },
}

/// 形の表現に対して材料の表示が合う位置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialContextMatch {
    /// 同じ文脈
    Exact,
    /// 形の子の文脈の親
    Parent,
}

/// 出所の番号から分けた、材料の表示と形の表現の結び付け
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeometryMaterialBinding {
    /// 同じ GeometryData の表現
    pub representation: GeometryRef,
    /// 文脈の関係
    pub context: MaterialContextMatch,
}

/// 材料に指定した表示の項目
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryMaterialItem {
    /// 元の名前
    pub name: Option<String>,
    /// 正負の面の元の指定。合成しない
    pub surfaces: Vec<GeometrySurfaceStyle>,
    /// 表示の項目と旧版の割り当て
    pub source: GeometrySource,
}

/// 1 つの文脈に対する材料の表示
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryMaterialRepresentation {
    /// 元の文脈
    pub context: GeometryMaterialContext,
    /// 元の識別名
    pub identifier: Option<String>,
    /// 元の表現の種類
    pub kind: Option<String>,
    /// 元の並びの表示
    pub items: Vec<GeometryMaterialItem>,
    /// 同じ文脈またはその子を使う形の表現
    pub bindings: Vec<GeometryMaterialBinding>,
    /// 表現・文脈・位置・単位の出所
    pub source: GeometrySource,
}

/// 1 材料の表示の定義
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryMaterialPresentation {
    /// 表示の定義の名前
    pub name: Option<String>,
    /// 表示の定義の説明
    pub description: Option<String>,
    /// 各文脈の表示。表示がなければ空
    pub representations: Vec<GeometryMaterialRepresentation>,
    /// 材料と表示の定義の出所
    pub source: GeometrySource,
}

/// 材料の構成内の 1 成分。省略と未対応を区別する
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryMaterialMember {
    /// 元の材料または未対応の構成の種類
    pub kind: String,
    /// 材料の表示。材料の省略は None、表示の未対応は Some(Unsupported)
    pub presentation: Option<Acquired<GeometryMaterialPresentation>>,
    /// 構成から材料までの参照の経路
    pub source: GeometrySource,
}

/// 1 つの材料の割り当て。複数成分を 1 つの色へまとめない
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryMaterialAssignment {
    /// 単一・リスト・層とその使用位置など、元の選択の種類
    pub kind: String,
    /// 構成の並び。層の厚さなどの属性は Building の材料成分と L1 で扱う
    pub members: Vec<GeometryMaterialMember>,
    /// 割り当てと構成の出所
    pub source: GeometrySource,
}

/// 要素または型から選んだ材料の表示
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryMaterials {
    /// 割り当てを選んだ位置
    pub level: MaterialAssignmentLevel,
    /// 選んだ位置にある全割り当て
    pub assignments: Vec<GeometryMaterialAssignment>,
    /// 所有者・要素と型を結ぶ関係の出所
    pub source: GeometrySource,
}

/// 項目・写像・材料から選んだ面の表示。形への割り振りと描画は行わない
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResolvedSurface<'a> {
    /// 面の表示の指定がない
    Unspecified,
    /// 項目・写像・レイヤから選んだ結果。未対応や競合も保つ
    Item(SurfaceAppearance<'a>),
    /// 1 材料・1 文脈の表示。複数の項目と正負の面を保つ
    Material {
        /// 割り当てと継承元
        materials: &'a GeometryMaterials,
        /// 材料の元の成分
        member: &'a GeometryMaterialMember,
        /// 選んだ文脈の表示
        representation: &'a GeometryMaterialRepresentation,
    },
    /// 材料の取得が未対応。型や別の文脈へ黙って読み替えない
    Unsupported,
    /// 複数の割り当て・材料・文脈がある。形への割り振りが必要
    Ambiguous {
        /// 競合する元値と出所
        materials: &'a GeometryMaterials,
    },
}

impl Geometry {
    /// 使用位置の項目・写像の表示を先に選び、指定がなければ材料の表示を引く。
    ///
    /// 材料は最上位の形の表現の文脈で選ぶ。同じ文脈を先に使い、なければ親の文脈の表示を使う。
    /// 複数の材料や同じ優先順位の表現を混ぜず、未対応と競合を明示する。確保は行わない。
    pub fn resolved_surface<'a>(
        &'a self,
        path: &[GeometryRef],
        cx: &mut Cx,
    ) -> Result<ResolvedSurface<'a>, GeometryError> {
        let item = self.surface_appearance(path, cx)?;
        if !matches!(item, SurfaceAppearance::Unspecified) {
            return Ok(ResolvedSurface::Item(item));
        }
        let Some(representation) = path.get(1) else {
            return Ok(ResolvedSurface::Unspecified);
        };
        let Some(materials) = &self.materials else {
            return Ok(ResolvedSurface::Unspecified);
        };
        cx.check(Kind::Work, 1)?;
        let Acquired::Value(materials) = materials else {
            return Ok(ResolvedSurface::Unsupported);
        };
        let [assignment] = materials.assignments.as_slice() else {
            return Ok(ResolvedSurface::Ambiguous { materials });
        };
        let [member] = assignment.members.as_slice() else {
            return Ok(ResolvedSurface::Ambiguous { materials });
        };
        let Some(value) = &member.presentation else {
            return Ok(ResolvedSurface::Unspecified);
        };
        let Acquired::Value(value) = value else {
            return Ok(ResolvedSurface::Unsupported);
        };
        let mut chosen = None;
        let mut exact = false;
        let mut ambiguous = false;
        for candidate in &value.representations {
            cx.check(Kind::Work, 1)?;
            for binding in &candidate.bindings {
                cx.check(Kind::Work, 1)?;
                if binding.representation != *representation {
                    continue;
                }
                let is_exact = binding.context == MaterialContextMatch::Exact;
                if chosen.is_none() || (is_exact && !exact) {
                    chosen = Some(candidate);
                    exact = is_exact;
                    ambiguous = false;
                } else if exact == is_exact {
                    ambiguous = true;
                }
            }
        }
        if ambiguous {
            return Ok(ResolvedSurface::Ambiguous { materials });
        }
        Ok(match chosen {
            Some(representation) => ResolvedSurface::Material {
                materials,
                member,
                representation,
            },
            None => ResolvedSurface::Unspecified,
        })
    }
}
