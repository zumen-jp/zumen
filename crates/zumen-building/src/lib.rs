//! zumen の建物モデル（L2、決定 0008・0207）。
//!
//! 要素は ID と出所だけを持ち、意味は成分で持つ。成分は種類ごとに、要素の番号の順の列で持つ。値は文書の単位のまま
//! 単位とともに持ち、黙って換算しない。IFC を知らない（決定 0008）。IFC からの lift は `zumen-ifc` の側に置く。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Ref, codes};

mod component;
mod hash;
mod id;
mod placement;
mod relations;

pub use component::*;
pub use hash::{Canon, Encode, Hash};
pub use id::{GlobalId, Id};
pub use placement::Transform;
pub use relations::Relations;

/// 要素の番号。ファイルに書いた順
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementRef(pub u32);

/// 要素の出所。ハッシュには入れない
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Source {
    /// L1 のインスタンスの番号
    pub instance: Option<u64>,
    /// ファイルに書いてあった GlobalId。使えずに `i` と番号で ID を作ったときも、元の文字列を残す
    pub global_id: Option<String>,
    /// 局所の変換に畳んだ、どの要素も持たない中間の配置のインスタンスの番号。内側から順
    pub folded_placements: Vec<u64>,
    /// 成分を付けた関係
    pub relations: Vec<RelationSource>,
}

/// 成分を付けた関係の出所（決定 0207）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationSource {
    /// 関係のインスタンスの番号
    pub instance: u64,
    /// 関係の GlobalId
    pub global_id: Option<String>,
    /// 関係の並びの中の、この要素の位置（0 から）。並びでなければ `None`
    pub position: Option<u32>,
}

/// 要素。ID と出所だけを持つ
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    id: Id,
    source: Source,
}

impl Element {
    /// ID
    pub fn id(&self) -> Id {
        self.id
    }

    /// 出所
    pub fn source(&self) -> &Source {
        &self.source
    }
}

/// 1 つの種類の成分の列。要素の番号の順に並べ、番号で引く
#[derive(Clone, Debug, PartialEq)]
pub struct Column<T> {
    entries: Vec<(ElementRef, T)>,
}

impl<T> Default for Column<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T> Column<T> {
    /// 要素の成分
    pub fn get(&self, element: ElementRef) -> Option<&T> {
        let at = self
            .entries
            .binary_search_by_key(&element, |(e, _)| *e)
            .ok()?;
        self.entries.get(at).map(|(_, value)| value)
    }

    /// 要素の成分を置く。前の値があれば返す
    pub fn insert(&mut self, element: ElementRef, value: T) -> Option<T> {
        match self.entries.binary_search_by_key(&element, |(e, _)| *e) {
            Ok(at) => self
                .entries
                .get_mut(at)
                .map(|slot| core::mem::replace(&mut slot.1, value)),
            Err(at) => {
                self.entries.insert(at, (element, value));
                None
            }
        }
    }

    /// 要素の成分を除く
    pub fn remove(&mut self, element: ElementRef) -> Option<T> {
        let at = self
            .entries
            .binary_search_by_key(&element, |(e, _)| *e)
            .ok()?;
        Some(self.entries.remove(at).1)
    }

    /// 要素の番号の順の成分
    pub fn iter(&self) -> impl Iterator<Item = (ElementRef, &T)> {
        self.entries
            .iter()
            .map(|(element, value)| (*element, value))
    }

    /// 成分を持つ要素の数
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 成分を持つ要素がないか
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// 決定 0207 の成分の列。種類は足すだけにし、名前と意味を変えない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Components {
    /// `identity`: 名前・説明・タグ・長い名前
    pub identity: Column<Identity>,
    /// `class`: IFC のクラス
    pub class: Column<Class>,
    /// `classification`: 分類の項目
    pub classification: Column<Classification>,
    /// `spatial`: 空間の種類と高さ
    pub spatial: Column<Spatial>,
    /// `parent`: 所属の先
    pub parent: Column<Parent>,
    /// `placement`: 配置
    pub placement: Column<Placement>,
    /// `representation`: 形状表現
    pub representation: Column<Representation>,
    /// `style`: 色と表示のレイヤ
    pub style: Column<Style>,
    /// `type`: 型の要素
    pub r#type: Column<TypeOf>,
    /// `properties`: プロパティのセット
    pub properties: Column<Properties>,
    /// `quantities`: 数量のセット
    pub quantities: Column<Quantities>,
    /// `material`: 材料の構成
    pub material: Column<Material>,
    /// `opening`: 開口と充填
    pub opening: Column<Opening>,
    /// `group`: 所属するグループ
    pub group: Column<Group>,
    /// `connection`: 接続と空間の境界
    pub connection: Column<Connection>,
    /// `grid`: 通り芯
    pub grid: Column<Grid>,
    /// `level`: レベル
    pub level: Column<Level>,
    /// `zone`: 区画
    pub zone: Column<Zone>,
    /// `extension`: 形式に固有の値
    pub extension: Column<Extension>,
}

/// 建物モデル。文書と、要素の一覧と、成分の列
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Building {
    /// 文書の値
    pub document: Document,
    /// 成分の列
    pub components: Components,
    elements: Vec<Element>,
    ids: BTreeMap<Id, ElementRef>,
}

impl Building {
    /// 文書の値から、要素のない建物モデルを作る
    pub fn new(document: Document) -> Self {
        Self {
            document,
            ..Self::default()
        }
    }

    /// 要素を足す。GlobalId があれば ID にし、ないか使えなければ `i` とインスタンスの番号で ID を作る（決定 0207）。
    ///
    /// 同じ GlobalId が 2 度目なら、`i` と番号で作って診断を返す。22 文字でないか使えない文字を含む GlobalId も、
    /// `i` と番号で作って診断を返す。作った ID がもうあれば、足さずに `None` を返して診断する。
    pub fn push(
        &mut self,
        global_id: Option<&str>,
        instance: u64,
        cx: &mut Cx<'_>,
    ) -> Option<ElementRef> {
        let fallback = Id::Instance(instance);
        let id = match global_id {
            None => fallback,
            Some(text) => match GlobalId::parse(text).map(Id::Global) {
                Some(id) if !self.ids.contains_key(&id) => id,
                Some(_) => {
                    cx.report(
                        Diagnostic::new(codes::BUILDING_GLOBAL_ID_EXISTS)
                            .with("global_id", text)
                            .at(Ref::default().with_id(fallback.to_string())),
                    );
                    fallback
                }
                None => {
                    cx.report(
                        Diagnostic::new(codes::BUILDING_GLOBAL_ID_INVALID)
                            .with("global_id", text)
                            .at(Ref::default().with_id(fallback.to_string())),
                    );
                    fallback
                }
            },
        };
        if self.ids.contains_key(&id) {
            cx.report(Diagnostic::new(codes::BUILDING_ID_EXISTS).with("id", id.to_string()));
            return None;
        }
        let element = ElementRef(u32::try_from(self.elements.len()).ok()?);
        self.elements.push(Element {
            id,
            source: Source {
                instance: Some(instance),
                global_id: global_id.map(str::to_owned),
                ..Source::default()
            },
        });
        self.ids.insert(id, element);
        Some(element)
    }

    /// 要素
    pub fn element(&self, element: ElementRef) -> Option<&Element> {
        self.elements.get(element.0 as usize)
    }

    /// 要素の出所を変える
    pub fn source_mut(&mut self, element: ElementRef) -> Option<&mut Source> {
        self.elements
            .get_mut(element.0 as usize)
            .map(|element| &mut element.source)
    }

    /// 要素の一覧。ファイルに書いた順
    pub fn elements(&self) -> &[Element] {
        &self.elements
    }

    /// ID で要素を引く
    pub fn find(&self, id: Id) -> Option<ElementRef> {
        self.ids.get(&id).copied()
    }

    /// 要素の ID
    pub fn id(&self, element: ElementRef) -> Option<Id> {
        self.element(element).map(Element::id)
    }

    pub(crate) fn place(&self, element: ElementRef) -> Ref {
        match self.id(element) {
            Some(id) => Ref::default().with_id(id.to_string()),
            None => Ref::default(),
        }
    }
}
