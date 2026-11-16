//! 3D のシーン（L3）。配置は schema/scene3d.toml、座標は Z が上のプロジェクト空間。
//! メッシュの頂点を共有し、f64 の塊の原点と f32 の局所座標・変換を持つ。

mod building;
pub mod layout;
mod packing;
mod read;
mod selection;
mod write;

pub use building::{Bounds, Options, Scene, build};
pub use read::{Column, ElementView, MeshView, Reader, read};
pub use write::encode;
use zumen_base::Stop;

/// メッシュ。頂点の番号は、このメッシュの中の番号。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// 文書単位の局所座標
    pub vertices: Vec<[f32; 3]>,
    /// 三角形。頂点の並びで向きを表す
    pub triangles: Vec<[u32; 3]>,
    /// 稜線。三角形の分け目は含めない
    pub edges: Vec<[u32; 2]>,
}

/// メッシュを置く 1 つ。
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    /// メッシュの番号
    pub mesh: u32,
    /// 要素の番号
    pub element: u32,
    /// 色の番号
    pub color: u32,
    /// メッシュから塊の原点に対する座標への変換。行優先の 3 行 4 列
    pub transform: [f32; 12],
}

/// 建物モデルの要素。形のない階と、評価できなかった要素も表に残す。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    /// 建物モデルの ID
    pub id: String,
    /// クラスの名前
    pub class: String,
    /// 名前。なければ空
    pub name: String,
    /// 所属する階の要素番号。なければ None
    pub storey: Option<u32>,
}

/// 色と不透明度。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    /// 赤・緑・青・不透明度。各 0〜1
    pub rgba: [f32; 4],
    /// 読み手が代替色を決めるか
    pub unresolved: bool,
}

/// インスタンスの続きを持つ塊。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chunk {
    /// プロジェクト空間の文書単位の原点
    pub origin: [f64; 3],
    /// 原点からの最小 X・Y・Z、最大 X・Y・Z
    pub bounds: [f32; 6],
    /// この塊のインスタンスの始まり。終わりは次の塊か全体の終わり
    pub instances: u32,
}

/// 列へ詰める前のシーン。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Data {
    /// 塊
    pub chunks: Vec<Chunk>,
    /// 共有するメッシュ
    pub meshes: Vec<Mesh>,
    /// 塊の順の配置
    pub instances: Vec<Instance>,
    /// 要素と参照先の階
    pub elements: Vec<Element>,
    /// 色
    pub colors: Vec<Color>,
}

/// 読み書きで止めた理由。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// バッファーや値が契約を満たさない。理由は入力の内容を含まない固定の語
    Invalid(&'static str),
    /// 予算か中断
    Stopped(Stop),
}

impl From<Stop> for Error {
    fn from(value: Stop) -> Self {
        Self::Stopped(value)
    }
}

impl From<zumen_geom3d::Failure> for Error {
    fn from(value: zumen_geom3d::Failure) -> Self {
        match value {
            zumen_geom3d::Failure::Stopped(stop) => Self::Stopped(stop),
            zumen_geom3d::Failure::Invalid(reason) | zumen_geom3d::Failure::Unsupported(reason) => {
                Self::Invalid(reason)
            }
            zumen_geom3d::Failure::Failed(_) => Self::Invalid("geometry_failed"),
        }
    }
}

/// 実装が受け付ける blob の上限（512 MiB）。呼び出し側の予算の上限も適用する。
pub const MAX_BYTES: u64 = 512 * 1024 * 1024;
