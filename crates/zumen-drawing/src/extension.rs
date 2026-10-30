//! 形式固有の情報と、その付く先・値。

use zumen_base::Ref;

use crate::{
    Drawing2D, Element, Layer, LayerId, LineType, LineTypeId, ModelPoint, PaperPoint, SheetId,
    SpaceId,
};

/// 形式固有の情報（決定 0022・0115）。共通の要素にできない残りを、系統・版・型付きの値・出所と、
/// 読んだときの付く先の値とともに持つ。付く先の値が変われば失効したものとし、古い値を書き戻さない
#[derive(Clone, Debug, PartialEq)]
pub struct Extension {
    /// 名前空間（`dxf.xdata` など）。値の型の契約を決める
    pub namespace: &'static str,
    /// 名前空間の中の元の名前（XDATA の登録アプリケーション名など）。翻訳しない
    pub name: String,
    /// 元の形式の版（DXF の `$ACADVER` など）。分からなければ `None`
    pub version: Option<String>,
    /// 名前空間の値の型の契約の版
    pub schema: u32,
    /// 付く先
    pub target: Target,
    /// 読んだときの付く先の値
    pub basis: Basis,
    /// 型付きの値。順序は元の形式の順
    pub fields: Vec<Field>,
    /// 元の形式のレコード
    pub source: Ref,
}

impl Extension {
    /// 付く先が読んだときのままか
    pub fn is_current(&self, drawing: &Drawing2D) -> bool {
        drawing.basis(&self.target).as_ref() == Some(&self.basis)
    }
}

/// 形式固有の情報が付く先
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// 図面全体
    Document,
    /// 実寸の空間か記号の定義の空間の、`index` 番目の要素
    Model {
        /// 空間
        space: SpaceId,
        /// 要素の添字
        index: usize,
    },
    /// 用紙の、`index` 番目の要素
    Paper {
        /// 用紙
        sheet: SheetId,
        /// 要素の添字
        index: usize,
    },
    /// レイヤ（決定 0152）
    Layer(LayerId),
    /// 線種（決定 0152）
    LineType(LineTypeId),
}

/// 形式固有の情報が依存する、付く先の値
#[derive(Clone, Debug, PartialEq)]
pub enum Basis {
    /// 図面全体。共通の欄には依存しない
    Document,
    /// 実寸の要素の、レイヤ・スタイル・形と意味・出所
    Model(Element<ModelPoint>),
    /// 用紙の要素の、レイヤ・スタイル・形と意味・出所
    Paper(Element<PaperPoint>),
    /// レイヤの名前・状態・スタイル・出所
    Layer(Layer),
    /// 線種の名前・定義・出所
    LineType(LineType),
}

/// 形式固有の情報の 1 つの値
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// 名前空間の中の値の名前
    pub key: &'static str,
    /// 元の形式の値の番号（DXF のグループ・コード。点は X のコード）。元の形式が値に番号を付けなければ `None`
    /// （決定 0157）
    pub code: Option<i16>,
    /// 値
    pub value: FieldValue,
}

/// 形式固有の情報の値の型
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    /// 文字列
    Text(String),
    /// 整数
    Integer(i64),
    /// 実数
    Real(f64),
    /// 3 次元の点やベクトル。座標の空間は名前空間が決める
    Point([f64; 3]),
    /// 元の形式のオブジェクトを指す値
    Handle(u64),
    /// バイト列
    Bytes(Vec<u8>),
}
