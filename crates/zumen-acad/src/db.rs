//! AutoCAD の系統で、符号化によらずに共有するオブジェクトの表（決定 0008・0023）。
//!
//! DXF と DWG は、同じデータベースを別の方法で符号化したもの。表は、オブジェクトごとに種類と、
//! DXF のグループ・コードで名付けた値を持つ。値の型は、グループ・コードの範囲で決める
//! （DXF Reference「Group Code Value Types」）。DWG のコーデックも、読んだ欄を同じ名前で表に置く。
//!
//! 元のバイトは、符号化ごとのファイルモデル（[`crate::DxfFile`] など）が持つ。表のオブジェクトは、
//! [`Origin`] で元のレコードを指す。

use std::collections::BTreeMap;

/// オブジェクトの表
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AcadDb {
    objects: Vec<Object>,
    /// ハンドルから、最初にそのハンドルを持つオブジェクトの番号
    handles: BTreeMap<u64, usize>,
}

impl AcadDb {
    /// オブジェクトを足す
    pub fn push(&mut self, object: Object) {
        if let Some(handle) = object.handle() {
            self.handles.entry(handle).or_insert(self.objects.len());
        }
        self.objects.push(object);
    }

    /// オブジェクト。符号化の中の順に並ぶ
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    /// ハンドルで引く。同じハンドルが 2 つ以上あれば、最初のもの
    pub fn get(&self, handle: u64) -> Option<&Object> {
        self.handles
            .get(&handle)
            .and_then(|&index| self.objects.get(index))
    }
}

/// オブジェクトが属する所
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Section {
    /// クラスの定義
    Classes,
    /// 画層・線種などの表
    Tables,
    /// ブロックの定義と、その中の図形
    Blocks,
    /// モデル空間とペーパー空間の図形
    Entities,
    /// 図形でないオブジェクト（辞書・レイアウトなど）
    Objects,
    /// ほかの所。名前は符号化のまま
    Other(Vec<u8>),
}

/// 表の 1 つのオブジェクト
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    /// 種類（DXF の名前。`LINE`・`LAYER` など）
    pub kind: Vec<u8>,
    /// 属する所
    pub section: Section,
    /// 値。符号化の中の順に並ぶ。同じグループ・コードが何度も現れることがある
    pub values: Vec<Value>,
    /// 元のレコード
    pub origin: Origin,
}

impl Object {
    /// ハンドル（5。DIMSTYLE は 105）。なければ `None`
    pub fn handle(&self) -> Option<u64> {
        self.values.iter().find_map(|value| match value {
            Value {
                code: 5 | 105,
                data: Data::Handle(handle),
            } => Some(*handle),
            _ => None,
        })
    }

    /// 所有者のハンドル（330）。なければ `None`
    pub fn owner(&self) -> Option<u64> {
        self.first(330).and_then(|data| match data {
            Data::Handle(handle) => Some(*handle),
            _ => None,
        })
    }

    /// 最初の `code` の値
    pub fn first(&self, code: i16) -> Option<&Data> {
        self.values
            .iter()
            .find(|value| value.code == code)
            .map(|value| &value.data)
    }

    /// 最初の `code` の値を実数として読む。なければ `None`、実数でなければ `Some(None)`
    pub fn real(&self, code: i16) -> Option<Option<f64>> {
        self.first(code).map(|data| match data {
            Data::Real(value) => Some(*value),
            _ => None,
        })
    }
}

/// グループ・コードで名付けた値
#[derive(Clone, Debug, PartialEq)]
pub struct Value {
    /// グループ・コード
    pub code: i16,
    /// 値
    pub data: Data,
}

/// 値の型（DXF Reference「Group Code Value Types」）
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    /// 文字列。符号化のままのバイト列で、文字コードは版と `$DWGCODEPAGE` で決まる。
    /// 型どおりに読めなかった値も、ここに元のまま置く
    Text(Vec<u8>),
    /// 整数（16・32・64 ビット）
    Int(i64),
    /// 実数
    Real(f64),
    /// 真偽
    Bool(bool),
    /// ハンドル
    Handle(u64),
    /// バイナリ
    Binary(Vec<u8>),
}

/// グループ・コードの値の型
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataType {
    /// 文字列
    Text,
    /// 整数
    Int,
    /// 実数
    Real,
    /// 真偽
    Bool,
    /// ハンドル
    Handle,
    /// バイナリ
    Binary,
}

impl DataType {
    /// グループ・コードの値の型（DXF Reference「Group Code Value Types」）。仕様にない範囲は文字列とする
    pub fn of(code: i16) -> DataType {
        match code {
            5 | 105 | 320..=369 | 390..=399 | 480..=481 | 1005 => DataType::Handle,
            10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => DataType::Real,
            60..=79
            | 90..=99
            | 160..=179
            | 270..=289
            | 370..=389
            | 400..=409
            | 420..=429
            | 440..=459
            | 1060..=1071 => DataType::Int,
            290..=299 => DataType::Bool,
            310..=319 | 1004 => DataType::Binary,
            _ => DataType::Text,
        }
    }
}

/// 表のオブジェクトの元のレコード
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// DXF の `index` 番目のレコード
    Dxf {
        /// レコードの番号
        record: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_codes_map_to_value_types() {
        let cases = [
            (0, DataType::Text),
            (5, DataType::Handle),
            (8, DataType::Text),
            (10, DataType::Real),
            (40, DataType::Real),
            (62, DataType::Int),
            (90, DataType::Int),
            (80, DataType::Text),
            (100, DataType::Text),
            (160, DataType::Int),
            (210, DataType::Real),
            (290, DataType::Bool),
            (310, DataType::Binary),
            (330, DataType::Handle),
            (999, DataType::Text),
            (1000, DataType::Text),
            (1004, DataType::Binary),
            (1005, DataType::Handle),
            (1040, DataType::Real),
            (1071, DataType::Int),
        ];
        for (code, expected) in cases {
            assert_eq!(DataType::of(code), expected, "{code}");
        }
    }

    #[test]
    fn handles_find_the_first_object() {
        let object = |handle: u64, kind: &[u8]| Object {
            kind: kind.to_vec(),
            section: Section::Entities,
            values: vec![Value {
                code: 5,
                data: Data::Handle(handle),
            }],
            origin: Origin::Dxf { record: 0 },
        };
        let mut db = AcadDb::default();
        db.push(object(0x2A, b"LINE"));
        db.push(object(0x2A, b"TEXT"));
        assert_eq!(db.get(0x2A).map(|o| o.kind.as_slice()), Some(&b"LINE"[..]));
        assert_eq!(db.get(1), None);
    }
}
