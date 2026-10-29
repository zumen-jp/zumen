//! AutoCAD の系統で、符号化によらずに共有するオブジェクトの表（決定 0008・0023）。
//!
//! DXF と DWG は、同じデータベースを別の方法で符号化したもの。表は、オブジェクトごとに種類と、
//! DXF のグループ・コードで名付けた値を持つ。値の型は、グループ・コードの範囲で決める
//! （DXF Reference「Group Code Value Types」）。DWG のコーデックも、読んだ欄を同じ名前で表に置く。
//!
//! 見出しの変数も、変数の名前（`$INSUNITS` など）ごとに同じ形の値で持つ。DWG のコーデックは、読んだ変数だけを置く。
//!
//! 型付きの見方（[`Object::line`] など）も表の上にあり、どちらの符号化にも同じように使える。
//!
//! 元のバイトは、符号化ごとのファイルモデル（[`crate::DxfFile`]・[`crate::DwgFile`]）が持つ。表のオブジェクトは、
//! [`Origin`] で元のレコードを指す。

use std::collections::BTreeMap;

/// オブジェクトの表
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AcadDb {
    objects: Vec<Object>,
    /// ハンドルから、最初にそのハンドルを持つオブジェクトの番号
    handles: BTreeMap<u64, usize>,
    /// 見出しの変数の名前から、その値
    variables: BTreeMap<Vec<u8>, Vec<Value>>,
}

impl AcadDb {
    /// オブジェクトを足す
    pub fn push(&mut self, object: Object) {
        if let Some(handle) = object.handle() {
            self.handles.entry(handle).or_insert(self.objects.len());
        }
        self.objects.push(object);
    }

    /// オブジェクトを `index` 番目に足す。`index` が数より大きければ最後に足す
    pub fn insert(&mut self, index: usize, object: Object) {
        let index = index.min(self.objects.len());
        self.objects.insert(index, object);
        self.reindex();
    }

    /// `index` 番目のオブジェクトを除いて返す。なければ `None`。除いたオブジェクトのハンドルを持つものが
    /// ほかになければ、ほかのオブジェクトからの参照のうち、辞書の項目と 102 の組の中の参照も除く
    /// （[`Object::forget`]）
    pub fn remove(&mut self, index: usize) -> Option<Object> {
        if index >= self.objects.len() {
            return None;
        }
        let removed = self.objects.remove(index);
        self.reindex();
        if let Some(handle) = removed.handle()
            && self.get(handle).is_none()
        {
            for object in &mut self.objects {
                object.forget(handle);
            }
        }
        Some(removed)
    }

    /// ハンドルの索引を作り直す
    fn reindex(&mut self) {
        self.handles.clear();
        for (index, object) in self.objects.iter().enumerate() {
            if let Some(handle) = object.handle() {
                self.handles.entry(handle).or_insert(index);
            }
        }
    }

    /// オブジェクト。符号化の中の順に並ぶ
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    /// 変えるためのオブジェクト。DXF に書き戻すときは、種類と元の場所を変えない（決定 0024）
    pub fn object_mut(&mut self, index: usize) -> Option<&mut Object> {
        self.objects.get_mut(index)
    }

    /// ハンドルで引く。同じハンドルが 2 つ以上あれば、最初のもの
    pub fn get(&self, handle: u64) -> Option<&Object> {
        // 足した後にハンドルを変えたオブジェクトもあるので、索引が外れたら順に探す
        self.handles
            .get(&handle)
            .and_then(|&index| self.objects.get(index))
            .filter(|o| o.handle() == Some(handle))
            .or_else(|| self.objects.iter().find(|o| o.handle() == Some(handle)))
    }

    /// ハンドルで引いた位置。同じハンドルが 2 つ以上あれば、最初のもの
    pub fn position(&self, handle: u64) -> Option<usize> {
        self.handles
            .get(&handle)
            .copied()
            .filter(|&index| {
                self.objects
                    .get(index)
                    .is_some_and(|o| o.handle() == Some(handle))
            })
            .or_else(|| self.objects.iter().position(|o| o.handle() == Some(handle)))
    }

    /// 見出しの変数を置く。同じ名前がすでにあれば置き換える
    pub fn set_variable(&mut self, name: &[u8], values: Vec<Value>) {
        self.variables.insert(name.to_vec(), values);
    }

    /// 見出しの変数の値。なければ `None`
    pub fn variable(&self, name: &[u8]) -> Option<&[Value]> {
        self.variables.get(name).map(Vec::as_slice)
    }

    /// 見出しの変数の名前と値。名前のバイト列の順に並ぶ
    pub fn variables(&self) -> impl Iterator<Item = (&[u8], &[Value])> {
        self.variables
            .iter()
            .map(|(name, values)| (name.as_slice(), values.as_slice()))
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

    /// `handle` への参照のうち、除いたオブジェクトに残すと壊れるものを除く（DXF Reference「Common Group
    /// Codes for Objects」と「DICTIONARY」）。
    ///
    /// - DICTIONARY の項目（名前の 3 と、次の 350・360）
    /// - 102 の組（`{ACAD_REACTORS`・`{ACAD_XDICTIONARY` など）の中の 330・360。参照を除いて空になった組は、
    ///   組ごと除く
    ///
    /// 組の外の所有者（330）などは残す
    pub fn forget(&mut self, handle: u64) {
        let refers = |value: &Value| matches!(value.data, Data::Handle(h) if h == handle);
        let dictionary = self.kind == b"DICTIONARY";
        let mut out = Vec::with_capacity(self.values.len());
        // 開いている組の、`out` の中の始まりと、参照を除いたか
        let mut groups: Vec<(usize, bool)> = Vec::new();
        let mut values = core::mem::take(&mut self.values).into_iter().peekable();
        while let Some(value) = values.next() {
            match (value.code, &value.data) {
                (102, Data::Text(text)) if text.starts_with(b"{") => {
                    groups.push((out.len(), false));
                    out.push(value);
                }
                (102, Data::Text(text)) if text.starts_with(b"}") => match groups.pop() {
                    Some((start, true)) if out.len() == start + 1 => out.truncate(start),
                    _ => out.push(value),
                },
                (330 | 360, _) if refers(&value) && !groups.is_empty() => {
                    if let Some(group) = groups.last_mut() {
                        group.1 = true;
                    }
                }
                (3, _)
                    if dictionary
                        && groups.is_empty()
                        && values
                            .peek()
                            .is_some_and(|next| matches!(next.code, 350 | 360) && refers(next)) =>
                {
                    values.next();
                }
                _ => out.push(value),
            }
        }
        self.values = out;
    }

    /// 所有者のハンドル（330）。102 の組（`{ACAD_REACTORS` など）の中の 330 は除く。なければ `None`
    pub fn owner(&self) -> Option<u64> {
        let mut depth = 0usize;
        for value in &self.values {
            match (value.code, &value.data) {
                (102, Data::Text(text)) if text.starts_with(b"{") => depth += 1,
                (102, Data::Text(text)) if text.starts_with(b"}") => {
                    depth = depth.saturating_sub(1);
                }
                (330, data) if depth == 0 => {
                    return match data {
                        Data::Handle(handle) => Some(*handle),
                        _ => None,
                    };
                }
                _ => {}
            }
        }
        None
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

    /// LINE の型付きの見方（DXF Reference「LINE」）。LINE でなければ `None`
    pub fn line(&self) -> Option<Result<Line<'_>, ViewError>> {
        (self.kind == b"LINE").then(|| {
            let Some(Data::Text(layer)) = self.first(8) else {
                return Err(ViewError::Missing(8));
            };
            let point = |x: i16| -> Result<[f64; 3], ViewError> {
                // Z のないものは 0 とみなす（推定。docs/formats/dxf.md）
                let z = match self.finite(x + 20) {
                    Err(ViewError::Missing(_)) => 0.0,
                    z => z?,
                };
                Ok([self.finite(x)?, self.finite(x + 10)?, z])
            };
            Ok(Line {
                layer,
                start: point(10)?,
                end: point(11)?,
            })
        })
    }

    /// 最初の `code` の値を、有限の実数として読む
    fn finite(&self, code: i16) -> Result<f64, ViewError> {
        match self.real(code) {
            None => Err(ViewError::Missing(code)),
            Some(value) => value
                .filter(|v| v.is_finite())
                .ok_or(ViewError::NotReal(code)),
        }
    }
}

/// LINE の型付きの見方
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line<'a> {
    /// 画層の名前（グループ・コード 8）。生のバイト列
    pub layer: &'a [u8],
    /// 始点（10・20・30）
    pub start: [f64; 3],
    /// 終点（11・21・31）
    pub end: [f64; 3],
}

/// 型付きの見方で値を読めなかった理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// このグループ・コードの値がない
    Missing(i16),
    /// このグループ・コードの値が、有限の実数として読めない
    NotReal(i16),
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

/// 制御文字を DXF の書き方（0x0A なら `^J`）に直す。`^` は、後に空白を付けて `^ ` とする
pub(crate) fn caret(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    for &byte in text {
        match byte {
            0..0x20 => out.extend([b'^', byte + 0x40]),
            b'^' => out.extend(*b"^ "),
            _ => out.push(byte),
        }
    }
    out
}

/// DXF の書き方の文字列を、元のバイト列に戻す。`^` と文字は制御文字に、`^ ` は `^` にする。ほかの `^` の
/// 並びは、読みが作らないので受け付けない
pub(crate) fn uncaret(text: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len());
    let mut bytes = text.iter();
    while let Some(&byte) = bytes.next() {
        if byte != b'^' {
            out.push(byte);
            continue;
        }
        match bytes.next()? {
            b' ' => out.push(b'^'),
            &next @ 0x40..0x60 => out.push(next - 0x40),
            _ => return None,
        }
    }
    Some(out)
}

/// 表のオブジェクトの元のレコード
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// DXF の `index` 番目のレコード
    Dxf {
        /// レコードの番号
        record: usize,
    },
    /// DWG の、`offset` バイト目から `end` バイト目の前までのオブジェクト
    Dwg {
        /// ファイルの先頭からの位置
        offset: usize,
        /// 終わりの位置
        end: usize,
    },
    /// 表に足したもので、元のレコードがない
    New,
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

    #[test]
    fn the_owner_skips_reactors_in_groups() {
        let value = |code: i16, data: Data| Value { code, data };
        let object = Object {
            kind: b"DIMSTYLE".to_vec(),
            section: Section::Tables,
            values: vec![
                value(105, Data::Handle(0x39A)),
                value(102, Data::Text(b"{ACAD_REACTORS".to_vec())),
                value(330, Data::Handle(0x514)),
                value(102, Data::Text(b"}".to_vec())),
                value(330, Data::Handle(0x0A)),
            ],
            origin: Origin::Dxf { record: 0 },
        };
        assert_eq!(object.owner(), Some(0x0A));
    }
}
