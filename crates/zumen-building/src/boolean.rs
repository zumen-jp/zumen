//! 評価前の立体の集合演算。

/// 2 つの立体への集合演算。差では第 1 の立体から第 2 の立体を取り除く
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOperation {
    /// 和
    Union,
    /// 積
    Intersection,
    /// 差
    Difference,
}
