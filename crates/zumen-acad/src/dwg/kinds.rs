//! DWG のオブジェクトの型の番号。読む側（`objects`）と書き戻す側（`write_back`）で同じ定数を使う。
//!
//! 型は、オブジェクトのデータの初めの BS（R2010 からは BOT）にあり、読む側は `u16` に直して持つ。
//! 500 以上の型は、ファイルのクラスで種類が決まるので、ここには置かない。

pub(super) const TEXT: u16 = 0x01;
pub(super) const ATTRIB: u16 = 0x02;
pub(super) const ATTDEF: u16 = 0x03;
pub(super) const BLOCK: u16 = 0x04;
pub(super) const ENDBLK: u16 = 0x05;
pub(super) const SEQEND: u16 = 0x06;
pub(super) const INSERT: u16 = 0x07;
/// MINSERT の型の番号
pub(super) const MINSERT: u16 = 0x08;
pub(super) const VERTEX_2D: u16 = 0x0A;
pub(super) const VERTEX_3D: u16 = 0x0B;
pub(super) const VERTEX_MESH: u16 = 0x0C;
pub(super) const VERTEX_PFACE: u16 = 0x0D;
pub(super) const VERTEX_PFACE_FACE: u16 = 0x0E;
pub(super) const POLYLINE_2D: u16 = 0x0F;
pub(super) const POLYLINE_3D: u16 = 0x10;
pub(super) const ARC: u16 = 0x11;
pub(super) const CIRCLE: u16 = 0x12;
pub(super) const LINE: u16 = 0x13;
pub(super) const DIMENSION_ORDINATE: u16 = 0x14;
pub(super) const DIMENSION_LINEAR: u16 = 0x15;
pub(super) const DIMENSION_ALIGNED: u16 = 0x16;
pub(super) const DIMENSION_ANGULAR_3PT: u16 = 0x17;
pub(super) const DIMENSION_ANGULAR_2LINE: u16 = 0x18;
pub(super) const DIMENSION_RADIUS: u16 = 0x19;
pub(super) const DIMENSION_DIAMETER: u16 = 0x1A;
pub(super) const POINT: u16 = 0x1B;
pub(super) const FACE3D: u16 = 0x1C;
pub(super) const POLYLINE_PFACE: u16 = 0x1D;
pub(super) const POLYLINE_MESH: u16 = 0x1E;
pub(super) const SOLID: u16 = 0x1F;
pub(super) const TRACE: u16 = 0x20;
pub(super) const SHAPE: u16 = 0x21;
pub(super) const VIEWPORT: u16 = 0x22;
pub(super) const ELLIPSE: u16 = 0x23;
pub(super) const SPLINE: u16 = 0x24;
pub(super) const RAY: u16 = 0x28;
pub(super) const XLINE: u16 = 0x29;
pub(super) const DICTIONARY: u16 = 0x2A;
pub(super) const MTEXT: u16 = 0x2C;
pub(super) const LEADER: u16 = 0x2D;
pub(super) const TOLERANCE: u16 = 0x2E;
pub(super) const MLINE: u16 = 0x2F;
pub(super) const BLOCK_RECORD_CONTROL: u16 = 0x30;
pub(super) const BLOCK_RECORD: u16 = 0x31;
pub(super) const LAYER_CONTROL: u16 = 0x32;
pub(super) const LAYER: u16 = 0x33;
pub(super) const STYLE_CONTROL: u16 = 0x34;
pub(super) const STYLE: u16 = 0x35;
pub(super) const LTYPE_CONTROL: u16 = 0x38;
pub(super) const LTYPE: u16 = 0x39;
pub(super) const VIEW_CONTROL: u16 = 0x3C;
pub(super) const VIEW: u16 = 0x3D;
pub(super) const UCS_CONTROL: u16 = 0x3E;
pub(super) const UCS: u16 = 0x3F;
pub(super) const VPORT_CONTROL: u16 = 0x40;
pub(super) const VPORT: u16 = 0x41;
pub(super) const APPID_CONTROL: u16 = 0x42;
pub(super) const APPID: u16 = 0x43;
pub(super) const DIMSTYLE_CONTROL: u16 = 0x44;
/// DIMSTYLE の表のレコードの型。ハンドルのグループ・コードが 105
pub(super) const DIMSTYLE: u16 = 0x45;
pub(super) const VP_ENT_HDR_CONTROL: u16 = 0x46;
pub(super) const VP_ENT_HDR: u16 = 0x47;
pub(super) const GROUP: u16 = 0x48;
pub(super) const MLINESTYLE: u16 = 0x49;
pub(super) const LWPOLYLINE: u16 = 0x4D;
