//! IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcSweptDiskSolid の属性の節、
//! DirectrixDim・InnerRadiusSize、IFC4 以降の DirectrixBounded に従う。
//! buildingSMART の公開 HTML の説明を根拠とし、EXPRESS は使わない。
//! 元の媒介変数を保ち、端点・接線の連続性・自己交差・曲率と半径の関係は評価しない。

use zumen_building::{CurveGraph, CurveKind, DiskSweep};

use super::{Version, arg, reference};
use crate::{
    Cx, Diagnostic, Error, Form, IfcFile, Kind, Record, ShapeNode, Value, at_bytes, codes, stopped,
};

/// 形の葉から取得した掃引円盤。定義と出所を分け、配置・写像・単位換算は適用しない
#[derive(Clone, Debug)]
pub struct SweptDisk<'f> {
    /// L1 に依存しない掃引の定義
    pub definition: DiskSweep,
    /// 元の経路と共有する基の曲線。半径の単位も length_unit に従う
    pub directrix: CurveGraph,
    /// 元の掃引のレコード。曲線の出所は directrix の各項目が持つ
    pub source: &'f Record,
}

impl IfcFile {
    /// ProductShape の葉から、掃引円盤と 3D の経路を必要時に取得する。
    ///
    /// 同じ IfcFile の ShapeNode を渡す。IFC2X3 の区間は必須、IFC4 以降では
    /// 有界か閉じた曲線の場合だけ省略できる。半径・内半径・区間の元値を保持する。
    /// 曲線を近似せず、曲率・自己交差・接合部・区間の幾何的な有効性は評価しない。
    /// 停止・不正・未対応で部分結果を返さず、既存の Building・ID・Hash は変更しない。
    pub fn swept_disk<'f>(
        &'f self,
        node: &ShapeNode<'f>,
        cx: &mut Cx,
    ) -> Result<SweptDisk<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                source: node.source,
            };
            reader.check(Kind::Work, 1)?;
            let solid = node
                .source
                .id()
                .and_then(|id| self.instance(id))
                .filter(|record| std::ptr::eq(*record, node.source))
                .ok_or_else(|| reader.invalid("sweep-source"))?;
            reader.source = solid;
            reader.check(Kind::Work, self.schema().len() as u64)?;
            let Some(version) = Version::of(self.schema()) else {
                reader.check(Kind::DecodedBytes, self.schema().len() as u64 * 3)?;
                reader
                    .cx
                    .report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                        "schema",
                        String::from_utf8_lossy(self.schema()).into_owned(),
                    ));
                return Err(Error::Unsupported);
            };
            reader
                .cx
                .enter()
                .map_err(|stop| stopped(reader.cx, stop, Some(solid.span())))?;
            let result = reader.read(solid, version);
            reader.cx.leave();
            result
        })
    }
}

struct Reader<'f, 'c, 'b> {
    file: &'f IfcFile,
    cx: &'c mut Cx<'b>,
    source: &'f Record,
}

impl<'f> Reader<'f, '_, '_> {
    fn check(&mut self, kind: Kind, count: u64) -> Result<(), Error> {
        self.cx
            .check(kind, count)
            .map_err(|stop| stopped(self.cx, stop, Some(self.source.span())))
    }

    fn invalid(&mut self, reason: &'static str) -> Error {
        self.cx.report(
            Diagnostic::new(codes::IFC_SHAPE_INVALID)
                .with("reason", reason)
                .at(at_bytes(self.source.span())
                    .with_id(format!("i{}", self.source.id().unwrap_or(0)))),
        );
        Error::Invalid
    }

    fn unsupported(&mut self) -> Result<SweptDisk<'f>, Error> {
        let kind = self.file.kind(self.source);
        self.check(Kind::Work, kind.len() as u64)?;
        self.check(Kind::DecodedBytes, kind.len() as u64 * 3)?;
        self.cx.report(
            Diagnostic::new(codes::IFC_SHAPE_UNSUPPORTED)
                .with("kind", String::from_utf8_lossy(kind).into_owned())
                .at(at_bytes(self.source.span())
                    .with_id(format!("i{}", self.source.id().unwrap_or(0)))),
        );
        Err(Error::Unsupported)
    }

    fn number(&mut self, value: Option<&Value>) -> Result<f64, Error> {
        let Some(Value::Number(raw)) = value else {
            return Err(self.invalid("sweep-number"));
        };
        self.check(Kind::Work, raw.len() as u64)?;
        std::str::from_utf8(self.file.bytes(raw.clone()))
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|n| n.is_finite())
            .ok_or_else(|| self.invalid("sweep-number"))
    }

    fn optional_number(&mut self, value: Option<&Value>) -> Result<Option<f64>, Error> {
        if matches!(value, Some(Value::Null)) {
            Ok(None)
        } else {
            self.number(value).map(Some)
        }
    }

    fn read(&mut self, solid: &'f Record, version: Version) -> Result<SweptDisk<'f>, Error> {
        self.check(Kind::Work, self.file.kind(solid).len() as u64)?;
        if solid.form() != Form::Entity
            || solid.parts().len() != 1
            || !self
                .file
                .kind(solid)
                .eq_ignore_ascii_case(b"IFCSWEPTDISKSOLID")
        {
            return self.unsupported();
        }
        if solid.parts().first().map_or(0, |p| p.args().len()) != 5 {
            return Err(self.invalid("sweep-attributes"));
        }
        self.check(Kind::Work, 5)?;
        let curve_id = reference(arg(solid, 1)).ok_or_else(|| self.invalid("sweep-reference"))?;
        // 参照の欠落は掃引側に場所を残す。経路の構造の診断は既存の曲線の読み取りに任せる。
        if self.file.instance(curve_id).is_none() {
            return Err(self.invalid("sweep-reference"));
        }
        let radius = self.number(arg(solid, 2))?;
        let inner_radius = self.optional_number(arg(solid, 3))?;
        if radius <= 0. || inner_radius.is_some_and(|inner| inner <= 0. || inner >= radius) {
            return Err(self.invalid("sweep-radius"));
        }
        let start_parameter = self.optional_number(arg(solid, 4))?;
        let end_parameter = self.optional_number(arg(solid, 5))?;
        if version == Version::Ifc2x3 && (start_parameter.is_none() || end_parameter.is_none()) {
            return Err(self.invalid("sweep-parameter"));
        }
        // 固定の結果も 32/64 bit で同じ量を確保前に数える。曲線の領域・深さは同じ Cx で数える。
        self.check(Kind::DecodedBytes, 256)?;
        let directrix = self.file.curve(curve_id, self.cx)?;
        self.check(Kind::Work, directrix.items.len() as u64)?;
        for item in &directrix.items {
            let dimension = match &item.kind {
                CurveKind::Polyline(points) | CurveKind::Indexed { points, .. } => {
                    points.first().map(|p| p.dimension())
                }
                CurveKind::Line { point, .. } => Some(point.dimension()),
                CurveKind::Circle { position, .. } => Some(position.location.dimension()),
                CurveKind::Composite { .. } | CurveKind::Trimmed { .. } => None,
                // 未対応の子も既存の curve() が種類・件数・出所を診断する。
                CurveKind::Unsupported(_) => return Err(Error::Unsupported),
            };
            if dimension.is_some_and(|dimension| dimension != 3) {
                return Err(self.invalid("sweep-dimension"));
            }
        }
        if (start_parameter.is_none() || end_parameter.is_none())
            && directrix
                .item(directrix.root)
                .is_some_and(|item| matches!(item.kind, CurveKind::Line { .. }))
        {
            return Err(self.invalid("sweep-unbounded"));
        }
        Ok(SweptDisk {
            definition: DiskSweep {
                directrix: directrix.root,
                radius,
                inner_radius,
                start_parameter,
                end_parameter,
            },
            directrix,
            source: solid,
        })
    }
}
