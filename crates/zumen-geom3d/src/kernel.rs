//! カーネルと平面での切断の trait

use zumen_base::{Cx, Stop};

use crate::{Mesh, Plane, Shape, Xy};

/// 評価の許容差。長さは形と同じ単位
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerance {
    /// 曲線を折れ線にするときの弦の誤差
    pub chord: f64,
    /// 同じ点とみなす距離
    pub length: f64,
}

/// 評価に失敗した理由。カーネルはパニックせず、これを返す（決定 0010）
#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// このカーネルが評価しない形の種類（[`Shape::kind`]）
    Unsupported(&'static str),
    /// 形が正しくない（長さが 0・有限でない値・点の番号が範囲の外など）
    Invalid(&'static str),
    /// 正しい形を、カーネルが評価できなかった
    Failed(String),
    /// 予算・中断で止めた
    Stopped(Stop),
}

impl From<Stop> for Failure {
    fn from(stop: Stop) -> Self {
        Failure::Stopped(stop)
    }
}

/// 形を評価してメッシュにするカーネル（決定 0018）。
///
/// 作った三角形の数を、予算の `segments` で数える（決定 0010）。同じ形と許容差からは、どの環境でも
/// 同じメッシュを返す（決定 0007）。
pub trait Kernel {
    /// カーネルの名前
    fn name(&self) -> &'static str;

    /// 形を評価する。閉じた形は、外向きの三角形の閉じたメッシュにする
    fn evaluate(&self, shape: &Shape, tolerance: Tolerance, cx: &mut Cx) -> Result<Mesh, Failure>;
}

/// メッシュを平面で切り、切り口を 2D にする。
///
/// 切り口は平面の局所の座標（[`Plane::frame`]）の輪で、閉じたメッシュなら、中身を左に見る向き
/// （外の輪は反時計回り、穴は時計回り）にする。作った辺の数を、予算の `segments` で数える。
pub trait Section {
    /// 切る
    fn section(&self, mesh: &Mesh, plane: &Plane, cx: &mut Cx) -> Result<Vec<Vec<Xy>>, Failure>;
}
