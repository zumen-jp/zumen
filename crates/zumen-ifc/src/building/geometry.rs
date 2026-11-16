//! 個別の取得を共有 Cx で組み合わせる。原本の形のグラフを保ち、値の評価は行わない。

use zumen_building::{CurveGraph, Element};

mod owned;

use super::arg;
use crate::{
    ContextGeometry, Cx, Diagnostic, Error, FacetedFaces, GridAxes, HalfSpaceGeometry, IfcFile,
    ItemPresentation, Kind, MappedShape3, ProductShape, ProfiledExtrusion, Record, RevolvedArea,
    ShapeLinks, ShapeRef, SweptDisk, TessellatedFaces, Value, at_bytes, codes, stopped,
};

/// 指定値を取得できたか。未対応を省略や空の値と同一視しない
#[derive(Clone, Debug)]
pub enum Retrieved<T> {
    /// 取得した元の値
    Value(T),
    /// 種類・場所を診断済み。元レコードは呼び出し元の出所に残る
    Unsupported,
}

/// 構造グラフの項目に対応する値。参照・配置・単位の適用は行わない
#[derive(Clone, Debug)]
pub enum GeometryValue<'f> {
    /// 定義・写像元・幾何集合・ブーリアン。参照と演算は shape の links に持つ
    Links,
    /// 表現が参照する文脈。座標系・精度・表示尺度は未適用
    Context(ContextGeometry<'f>),
    /// 曲線。曲線内部の共有はこのグラフに閉じる
    Curve(CurveGraph),
    /// 断面の押し出し
    Extrusion(ProfiledExtrusion<'f>),
    /// 断面の回転
    Revolution(RevolvedArea<'f>),
    /// 円盤の掃引
    SweptDisk(SweptDisk<'f>),
    /// 多角形の面集合
    Faces(FacetedFaces<'f>),
    /// 添字付きの面集合
    Tessellation(TessellatedFaces<'f>),
    /// 平面の半空間
    HalfSpace(HalfSpaceGeometry<'f>),
    /// 共有する形の 3D の写像
    Mapping(MappedShape3<'f>),
    /// 項目全体を取得できない。元の種類と shape の出所を保持する
    Unsupported {
        /// 形の項目の種類。未対応の子の詳しい種類は診断に残す
        kind: String,
    },
}

/// 形の項目を使う位置の表示。同じ項目でも異なる表現からの参照を区別する
#[derive(Clone, Debug)]
pub struct GeometryStyle<'f> {
    /// 項目の参照
    pub item: ShapeRef,
    /// 項目を直接含む表現。ブーリアンや集合の子だけなら None
    pub representation: Option<ShapeRef>,
    /// 直接とレイヤの表示。未対応を指定なしと区別する
    pub presentation: Retrieved<ItemPresentation<'f>>,
}

/// 要素から一度に取得した形・写像・表示・通り芯。Building 自体は書き換えない
#[derive(Clone, Debug)]
pub struct ProductGeometry<'f> {
    /// 元の参照関係。同じ項目は一度だけ持つ
    pub shape: ProductShape<'f>,
    /// shape.nodes と同じ位置の元値。構造ビューで未展開の葉もここで取得する
    pub values: Vec<GeometryValue<'f>>,
    /// 表現と項目の組ごとの表示。写像の階層を越える上書きは未評価
    pub styles: Vec<GeometryStyle<'f>>,
    /// 通り芯でなければ None。軸が未対応の場合は Some(Unsupported)
    pub grid: Option<Retrieved<GridAxes<'f>>>,
}

impl ProductGeometry<'_> {
    /// この取得の ShapeRef から元値を引く
    pub fn value(&self, at: ShapeRef) -> Option<&GeometryValue<'_>> {
        self.values.get(at.0 as usize)
    }
}

// プラットフォームによらず容量の確保前に予約する。入れ子の列は個別の取得が数える。
const VALUE_BYTES: usize = 2048;
const STYLE_BYTES: usize = 1024;
const _: () = assert!(size_of::<GeometryValue<'static>>() <= VALUE_BYTES);
const _: () = assert!(size_of::<GeometryStyle<'static>>() <= STYLE_BYTES);

impl IfcFile {
    /// Building の要素の出所から形・写像・表示・通り芯を取得する。
    ///
    /// この IfcFile から lift した要素を渡す。出所の番号と元の GlobalId を照合する。
    /// 取得やその順序で Building と既存の要素ハッシュを変えない。
    pub fn element_geometry<'f>(
        &'f self,
        element: &Element,
        cx: &mut Cx,
    ) -> Result<ProductGeometry<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let source = element.source();
            let record = source.instance.and_then(|id| self.instance(id));
            check(cx, record, Kind::Work, 1)?;
            let record = record.ok_or_else(|| invalid(cx, record, "element-source"))?;
            if let Some(expected) = source.global_id.as_deref() {
                let Some(Value::Text { raw, .. }) = arg(record, 1) else {
                    return Err(invalid(cx, Some(record), "element-source"));
                };
                check(cx, Some(record), Kind::Work, raw.len() as u64)?;
                check(
                    cx,
                    Some(record),
                    Kind::DecodedBytes,
                    (raw.len() as u64).saturating_mul(12).saturating_add(32),
                )?;
                let (actual, _) = self.decode_text(self.bytes(raw.clone()));
                if actual != expected {
                    return Err(invalid(cx, Some(record), "element-source"));
                }
            }
            self.product_geometry(record.id().unwrap_or(0), cx)
        })
    }

    /// 1 要素の形・写像・表示・通り芯を、型と借用出所を保って取得する。
    ///
    /// 未知の項目・未対応の定義は Unsupported と診断を残す明示的な欠落である。
    /// 不正・停止では全体を返さない。共有項目の値は一度だけ取得し、表示は参照元の表現ごとに持つ。
    /// 配置・単位換算・幾何・材質・写像間の表示の継承は評価しない。
    pub fn product_geometry(
        &self,
        instance: u64,
        cx: &mut Cx,
    ) -> Result<ProductGeometry<'_>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let shape = self.product_shape_inner(instance, cx, false)?;
            let n = shape.nodes.len();
            check(cx, Some(shape.product), Kind::Elements, n as u64)?;
            reserve(cx, Some(shape.product), n, VALUE_BYTES + 8)?;
            let mut values = Vec::with_capacity(n);
            let mut represented = vec![false; n];
            for node in &shape.nodes {
                check(cx, Some(node.source), Kind::Work, 1)?;
                let kind = self.kind(node.source);
                let result = match &node.links {
                    ShapeLinks::Representation { context, .. } => self
                        .representation_context(context.id().unwrap_or(0), cx)
                        .map(GeometryValue::Context),
                    ShapeLinks::Mapped { .. } => {
                        self.mapped_shape3(node, cx).map(GeometryValue::Mapping)
                    }
                    ShapeLinks::Curve => self
                        .curve(node.source.id().unwrap_or(0), cx)
                        .map(GeometryValue::Curve),
                    ShapeLinks::Unsupported => {
                        if kind.eq_ignore_ascii_case(b"IFCEXTRUDEDAREASOLID") {
                            self.extrusion(node, cx).map(GeometryValue::Extrusion)
                        } else if kind.eq_ignore_ascii_case(b"IFCREVOLVEDAREASOLID") {
                            self.revolved_area(node, cx).map(GeometryValue::Revolution)
                        } else if kind.eq_ignore_ascii_case(b"IFCSWEPTDISKSOLID") {
                            self.swept_disk(node, cx).map(GeometryValue::SweptDisk)
                        } else if [
                            b"IFCFACETEDBREP".as_slice(),
                            b"IFCSHELLBASEDSURFACEMODEL",
                            b"IFCFACEBASEDSURFACEMODEL",
                        ]
                        .iter()
                        .any(|k| kind.eq_ignore_ascii_case(k))
                        {
                            self.faceted_faces(node, cx).map(GeometryValue::Faces)
                        } else if [b"IFCTRIANGULATEDFACESET".as_slice(), b"IFCPOLYGONALFACESET"]
                            .iter()
                            .any(|k| kind.eq_ignore_ascii_case(k))
                        {
                            self.tessellated_faces(node, cx)
                                .map(GeometryValue::Tessellation)
                        } else if [
                            b"IFCHALFSPACESOLID".as_slice(),
                            b"IFCBOXEDHALFSPACE",
                            b"IFCPOLYGONALBOUNDEDHALFSPACE",
                        ]
                        .iter()
                        .any(|k| kind.eq_ignore_ascii_case(k))
                        {
                            self.half_space(node, cx).map(GeometryValue::HalfSpace)
                        } else {
                            check(cx, Some(node.source), Kind::Work, kind.len() as u64)?;
                            check(
                                cx,
                                Some(node.source),
                                Kind::DecodedBytes,
                                (kind.len() as u64).saturating_mul(6),
                            )?;
                            cx.report(
                                Diagnostic::new(codes::IFC_SHAPE_UNSUPPORTED)
                                    .with("kind", String::from_utf8_lossy(kind).into_owned())
                                    .at(at_bytes(node.source.span())),
                            );
                            Err(Error::Unsupported)
                        }
                    }
                    _ => Ok(GeometryValue::Links),
                };
                values.push(match result {
                    Ok(value) => value,
                    Err(Error::Unsupported) => {
                        check(cx, Some(node.source), Kind::Work, kind.len() as u64)?;
                        check(
                            cx,
                            Some(node.source),
                            Kind::DecodedBytes,
                            (kind.len() as u64).saturating_mul(3),
                        )?;
                        GeometryValue::Unsupported {
                            kind: String::from_utf8_lossy(kind).into_owned(),
                        }
                    }
                    Err(error) => return Err(error),
                });
            }
            let mut styles = Vec::new();
            for (i, node) in shape.nodes.iter().enumerate() {
                check(cx, Some(node.source), Kind::Work, 1)?;
                if let ShapeLinks::Representation { items, .. } = &node.links {
                    for &item in items {
                        check(cx, Some(node.source), Kind::Work, 1)?;
                        let target = shape
                            .node(item)
                            .ok_or_else(|| invalid(cx, Some(node.source), "reference"))?;
                        reserve_style(cx, target.source, &mut styles)?;
                        let presentation =
                            retrieved(self.item_presentation(target, Some(node), cx))?;
                        let marked = represented
                            .get_mut(item.0 as usize)
                            .ok_or_else(|| invalid(cx, Some(node.source), "reference"))?;
                        *marked = true;
                        styles.push(GeometryStyle {
                            item,
                            representation: Some(ShapeRef(i as u32)),
                            presentation,
                        });
                    }
                }
            }
            for (i, (node, represented)) in shape.nodes.iter().zip(&represented).enumerate() {
                check(cx, Some(node.source), Kind::Work, 1)?;
                if !represented
                    && !matches!(
                        node.links,
                        ShapeLinks::Definition { .. }
                            | ShapeLinks::Representation { .. }
                            | ShapeLinks::Map { .. }
                    )
                {
                    reserve_style(cx, node.source, &mut styles)?;
                    styles.push(GeometryStyle {
                        item: ShapeRef(i as u32),
                        representation: None,
                        presentation: retrieved(self.item_presentation(node, None, cx))?,
                    });
                }
            }
            let grid = if self.kind(shape.product).eq_ignore_ascii_case(b"IFCGRID") {
                Some(retrieved(self.grid_axes(instance, cx))?)
            } else {
                None
            };
            Ok(ProductGeometry {
                shape,
                values,
                styles,
                grid,
            })
        })
    }
}

fn retrieved<T>(result: Result<T, Error>) -> Result<Retrieved<T>, Error> {
    match result {
        Ok(value) => Ok(Retrieved::Value(value)),
        Err(Error::Unsupported) => Ok(Retrieved::Unsupported),
        Err(error) => Err(error),
    }
}
fn check(cx: &mut Cx, record: Option<&Record>, kind: Kind, count: u64) -> Result<(), Error> {
    cx.check(kind, count)
        .map_err(|stop| stopped(cx, stop, record.map(Record::span)))
}
fn invalid(cx: &mut Cx, record: Option<&Record>, reason: &'static str) -> Error {
    let mut d = Diagnostic::new(codes::IFC_SHAPE_INVALID).with("reason", reason);
    if let Some(r) = record {
        d = d.at(at_bytes(r.span()));
    }
    cx.report(d);
    Error::Invalid
}
fn reserve(cx: &mut Cx, record: Option<&Record>, count: usize, bytes: usize) -> Result<(), Error> {
    if count > i32::MAX as usize / bytes {
        return Err(invalid(cx, record, "capacity"));
    }
    check(
        cx,
        record,
        Kind::DecodedBytes,
        (count as u64).saturating_mul(bytes as u64),
    )
}
fn reserve_style(
    cx: &mut Cx,
    record: &Record,
    styles: &mut Vec<GeometryStyle<'_>>,
) -> Result<(), Error> {
    if styles.len() == styles.capacity() {
        let old = styles.capacity();
        let capacity = old
            .saturating_mul(2)
            .max(1)
            .min(i32::MAX as usize / STYLE_BYTES);
        if capacity <= old {
            return Err(invalid(cx, Some(record), "capacity"));
        }
        check(cx, Some(record), Kind::Work, styles.len() as u64)?;
        reserve(cx, Some(record), capacity - old, STYLE_BYTES)?;
        styles.reserve_exact(capacity - styles.len());
    }
    Ok(())
}
