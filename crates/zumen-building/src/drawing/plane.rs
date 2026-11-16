//! 切る面と、プロジェクトから図面への座標変換。

use zumen_crs::ProjectPoint;
use zumen_geom3d::{Failure, Xyz, add, dot, scale, sub};

/// 2 点で決めた鉛直面を見る側。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// 始点から終点へ向かって右側から見る。
    Right,
    /// 始点から終点へ向かって左側から見る。
    Left,
}

/// 図面を切り出す面。長さは文書の単位で、深さは面の向こうへ測る。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    /// FL の点から height だけ上の面を、上から見る平断面。
    Plan {
        /// FL を表すプロジェクトの点。XY は図面の原点。
        floor: ProjectPoint,
        /// FL から切る面までの高さ。
        height: f64,
        /// 切る面から下へ見る深さ。0 より大きい有限の値。
        depth: f64,
    },
    /// 2 点の XY を結ぶ鉛直面。図面の縦は常にプロジェクトの上方向。
    Vertical {
        /// 図面の原点。Z は図面で高さ 0 とする値。
        start: ProjectPoint,
        /// 横方向を決める点。Z は start と同じ値に限る。
        end: ProjectPoint,
        /// 見る側。
        side: Side,
        /// 面から向こうへ見る深さ。0 より大きい有限の値。
        depth: f64,
        /// 断面なら真、立面なら偽。
        section: bool,
    },
}

/// プロジェクトの点と図面の点・奥行きの対応。軸は右手系の正規直交基底。
///
/// depth は見る側が正。表示の範囲は -far 以上 0 以下で、図面の点は
/// 文書単位の実寸である。大きな座標は origin を引いてから投影する。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    origin: ProjectPoint,
    right: Xyz,
    up: Xyz,
    toward: Xyz,
    far: f64,
    section: bool,
}

fn xyz(p: ProjectPoint) -> Xyz {
    [p.x, p.y, p.z]
}

impl View {
    /// 有限な値、深さと軸を検査し、座標変換を作る。
    pub fn frame(self) -> Result<Frame, Failure> {
        let result = match self {
            Self::Plan {
                floor,
                height,
                depth,
            } => Frame {
                origin: ProjectPoint {
                    z: floor.z + height,
                    ..floor
                },
                right: [1.0, 0.0, 0.0],
                up: [0.0, 1.0, 0.0],
                toward: [0.0, 0.0, 1.0],
                far: depth,
                section: true,
            },
            Self::Vertical {
                start,
                end,
                side,
                depth,
                section,
            } => {
                if !xyz(end).into_iter().all(f64::is_finite) || end.z != start.z {
                    return Err(Failure::Invalid("drawing_plane"));
                }
                let delta = [end.x - start.x, end.y - start.y];
                let length = libm::hypot(delta[0], delta[1]);
                if !length.is_finite() || length <= 0.0 {
                    return Err(Failure::Invalid("drawing_plane"));
                }
                let sign = if side == Side::Right { 1.0 } else { -1.0 };
                let [x, y] = delta.map(|v| v / length * sign);
                Frame {
                    origin: start,
                    right: [x, y, 0.0],
                    up: [0.0, 0.0, 1.0],
                    toward: [y, -x, 0.0],
                    far: depth,
                    section,
                }
            }
        };
        if !xyz(result.origin).into_iter().all(f64::is_finite)
            || !result.far.is_finite()
            || result.far <= 0.0
        {
            return Err(Failure::Invalid("drawing_plane"));
        }
        Ok(result)
    }
}

impl Frame {
    /// 図面の原点を表すプロジェクトの点。
    pub fn origin(self) -> ProjectPoint {
        self.origin
    }

    /// 図面の右、上、見る側への向き。
    pub fn axes(self) -> [Xyz; 3] {
        [self.right, self.up, self.toward]
    }

    /// 面から向こうへ見る深さ。
    pub fn depth(self) -> f64 {
        self.far
    }

    /// 切断の線も描くか。立面では偽。
    pub fn is_section(self) -> bool {
        self.section
    }

    /// プロジェクトの点を、図面の XY と面からの符号付き距離へ写す。
    pub fn project(self, point: ProjectPoint) -> Result<Xyz, Failure> {
        self.project_offset(sub(xyz(point), xyz(self.origin)))
    }

    /// 面の原点からの差を写す。配置の移動を先に差し引く経路でも共用する。
    pub(crate) fn project_offset(self, difference: Xyz) -> Result<Xyz, Failure> {
        let value = self.axes().map(|axis| dot(difference, axis));
        if !value.into_iter().all(f64::is_finite) {
            return Err(Failure::Invalid("drawing_coordinate"));
        }
        Ok(value)
    }

    /// 図面の XY と符号付き距離を、元のプロジェクトの点へ戻す。
    pub fn unproject(self, value: Xyz) -> Result<ProjectPoint, Failure> {
        let offset = add(
            add(scale(self.right, value[0]), scale(self.up, value[1])),
            scale(self.toward, value[2]),
        );
        let [x, y, z] = add(xyz(self.origin), offset);
        if ![x, y, z].into_iter().all(f64::is_finite) {
            return Err(Failure::Invalid("drawing_coordinate"));
        }
        Ok(ProjectPoint { x, y, z })
    }
}
