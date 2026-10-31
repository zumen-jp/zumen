//! IDS（buildingSMART International の Information Delivery Specification 1.0）を読み、建物モデルで照合する（決定 0207・0217）。
//!
//! IDS はプロファイルに写さず、照合の入力としてそのまま読む。facet は建物モデルの成分で確かめ、成分にない IFC の属性は、
//! 呼び手が渡す [`Lookup`] で引く（`zumen-ifc` が EXPRESS で引く）。この crate は IFC を知らない。

mod check;
mod pattern;
mod read;

use core::ops::Range;

pub use check::{
    AttributeValue, Finding, Lookup, Missing, Report, SpecificationSummary, Status, check,
};

/// 読んだ IDS
#[derive(Clone, Debug, PartialEq)]
pub struct Ids {
    title: Option<String>,
    specifications: Vec<Specification>,
}

impl Ids {
    /// `info` の題
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// 仕様。書いた順
    pub fn specifications(&self) -> &[Specification] {
        &self.specifications
    }
}

/// 仕様（`specification`）
#[derive(Clone, Debug, PartialEq)]
pub struct Specification {
    name: String,
    identifier: Option<String>,
    versions: Vec<String>,
    occurs: Occurs,
    applicability: Vec<Facet>,
    requirements: Vec<Facet>,
    span: Range<usize>,
}

impl Specification {
    /// 名前
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 識別子
    pub fn identifier(&self) -> Option<&str> {
        self.identifier.as_deref()
    }

    /// 対象の IFC の版（`IFC2X3`・`IFC4`・`IFC4X3_ADD2`）
    pub fn versions(&self) -> &[String] {
        &self.versions
    }
}

/// 適用の条件に合う要素の数の決まり（`minOccurs`・`maxOccurs`）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Occurs {
    pub(crate) min: u64,
    /// `None` は `unbounded`
    pub(crate) max: Option<u64>,
}

impl Default for Occurs {
    fn default() -> Self {
        Self { min: 1, max: None }
    }
}

/// 要件の facet の出現の決まり
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cardinality {
    Required,
    Optional,
    Prohibited,
}

/// 関係の種類（`partOf` の `relation`）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Relation {
    Aggregates,
    AssignsToGroup,
    ContainedInSpatialStructure,
    Nests,
    VoidsFills,
}

impl Relation {
    pub(crate) fn of(text: &str) -> Option<Self> {
        Some(match text {
            "IFCRELAGGREGATES" => Self::Aggregates,
            "IFCRELASSIGNSTOGROUP" => Self::AssignsToGroup,
            "IFCRELCONTAINEDINSPATIALSTRUCTURE" => Self::ContainedInSpatialStructure,
            "IFCRELNESTS" => Self::Nests,
            "IFCRELVOIDSELEMENT IFCRELFILLSELEMENT" => Self::VoidsFills,
            _ => return None,
        })
    }
}

/// facet
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Facet {
    Entity {
        name: Value,
        predefined: Option<Value>,
    },
    Attribute {
        name: Value,
        value: Option<Value>,
        cardinality: Cardinality,
    },
    Classification {
        value: Option<Value>,
        system: Option<Value>,
        cardinality: Cardinality,
    },
    Property {
        set: Value,
        name: Value,
        /// 大文字の型の名前（`IFCLABEL` など）
        data_type: Option<String>,
        value: Option<Value>,
        cardinality: Cardinality,
    },
    Material {
        value: Option<Value>,
        cardinality: Cardinality,
    },
    PartOf {
        /// [`Facet::Entity`]
        entity: Box<Facet>,
        relation: Option<Relation>,
        cardinality: Cardinality,
    },
    /// 知らない facet。確かめられない
    Unsupported {
        kind: String,
        cardinality: Cardinality,
    },
}

/// 値の決まり
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    /// `simpleValue`
    Simple(String),
    /// `xs:restriction`
    Restriction(Restriction),
}

/// `xs:restriction`
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Restriction {
    /// 接頭辞を除いた基の型（`string`・`double` など）
    pub(crate) base: Option<String>,
    pub(crate) constraints: Vec<Constraint>,
}

/// 制約
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Constraint {
    Enumeration(String),
    Pattern(Box<pattern::Pattern>),
    MinInclusive(f64),
    MaxInclusive(f64),
    MinExclusive(f64),
    MaxExclusive(f64),
    Length(u64),
    MinLength(u64),
    MaxLength(u64),
    /// 知らない制約か、読まない式。確かめられない
    Unsupported(String),
}
