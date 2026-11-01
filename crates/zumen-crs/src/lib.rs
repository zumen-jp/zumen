//! zumen の座標系と標高（決定 0019）。
//!
//! - [`point`] は、空間ごとの点の型（地図・地理・プロジェクト）と、空間の間の変換（`IfcMapConversion`）、平面の軸の並び
//! - [`crs`] は、平面直角座標系の 19 の系と、EPSG のコードと名前による名指し
//! - [`projection`] は、経緯度と平面直角座標の換算（国土地理院の計算式）
//! - [`height`] は、標高の基準と成果。成果の違う値を一緒に使うと診断を返す
//! - [`grid`] は、ジオイド高と基準面補正量の格子（ISG format 2.0）と、双一次の補間
//!
//! 三角関数と双曲線関数は、どの環境でも同じ値になる `libm` で求める（決定 0007）。国土地理院のジオイド・モデルは
//! 同梱しない。利用者が入手して渡す（決定 0019）。

pub mod crs;
pub mod grid;
pub mod height;
pub mod point;
pub mod projection;

pub use crs::{Crs, VerticalCrs, Zone, wkt_codes};
pub use grid::{Grid, Model, Undulation};
pub use height::{Height, HeightDatum, Realization};
pub use point::{Axes, GeoPoint, MapConversion, MapPoint, MapPosition, ProjectPoint};
pub use projection::{Projected, Unprojected, to_geographic, to_plane};
