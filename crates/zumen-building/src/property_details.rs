//! 値の型・範囲・複合の構造を持つプロパティ。既存の平らな properties と分けて保持する。

use crate::{TypedValue, Unit};

/// `property-details`: 既存の単一値と値の並びだけでは表せないプロパティの構造
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PropertyDetails {
    /// 要素か型に直接付くセット。継承した値と混ぜない
    pub sets: Vec<DetailedPropertySet>,
}

/// 1 つのプロパティのセット。木を親が先の平らな列にし、深い所有権の入れ子を作らない
#[derive(Clone, Debug, PartialEq)]
pub struct DetailedPropertySet {
    /// セットの名前
    pub name: String,
    /// 説明
    pub description: Option<String>,
    /// 元の順の項目。共有する複合の子は、各親の下へ予算を数えて写す
    pub properties: Vec<DetailedProperty>,
    /// 元の定義の番号。ハッシュには含めない
    pub source_instance: Option<u64>,
}

/// 複合の親か、値を持つ葉
#[derive(Clone, Debug, PartialEq)]
pub struct DetailedProperty {
    /// 同じセットの中の親の位置。None は根。親は自身より前の Complex の項目
    pub parent: Option<u32>,
    /// 名前
    pub name: String,
    /// 説明
    pub description: Option<String>,
    /// 値と構造
    pub value: PropertyDetailValue,
    /// 元の項目の番号。ハッシュには含めない
    pub source_instance: Option<u64>,
}

/// プロパティの値の構成。省略を空の列や 0 と区別する
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyDetailValue {
    /// 単一の値
    Single {
        /// 型を含む元の値
        value: Option<TypedValue>,
        /// 明示した単位か、文書の既定の単位
        unit: Option<Unit>,
    },
    /// 値の並び。各値の型を持つ
    List {
        /// 元の順の値。None は属性の省略
        values: Option<Vec<TypedValue>>,
        /// 明示した単位か、文書の既定の単位
        unit: Option<Unit>,
    },
    /// 選んだ値と、定義されている場合の選択肢
    Enumerated {
        /// 選んだ値
        values: Option<Vec<TypedValue>>,
        /// 元の選択肢の名前・値・単位
        choices: Option<PropertyChoices>,
    },
    /// 下限・上限・設定値。それぞれの省略を保つ
    Bounded {
        /// 下限
        lower: Option<TypedValue>,
        /// 上限
        upper: Option<TypedValue>,
        /// 設定値
        set_point: Option<TypedValue>,
        /// 明示した単位か、文書の既定の単位
        unit: Option<Unit>,
    },
    /// 複合の親。子は同じセットでこの項目を parent として指す
    Complex {
        /// 同じ名前の複合を区別する用途
        usage_name: String,
    },
    /// 取得できなかった種類。出所と欠落の診断を残す
    Unsupported {
        /// 元の種類
        kind: String,
    },
}

/// 列挙の選択肢。選択した値の列とは分けて持つ
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyChoices {
    /// 選択肢の名前
    pub name: String,
    /// 定義された値
    pub values: Vec<TypedValue>,
    /// 明示された単位。省略を既定の単位と同一視しない
    pub unit: Option<Unit>,
    /// 元の選択肢の定義。ハッシュには含めない
    pub source_instance: Option<u64>,
}
