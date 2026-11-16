//! IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcGeometricRepresentationContext の
//! 属性と North2D、IfcGeometricRepresentationSubContext の ParentNoSub・UserTargetProvided、
//! IfcGeometricProjectionEnum の自然文の定義に従う。座標・尺度・真北を適用しない。

use zumen_building::{
    ContextPlacement, GeometricContext, RepresentationContext, SubContext, TargetView, Unit,
};

use super::{Reader, Version, arg, face_length_unit};
use crate::{Cx, Diagnostic, Error, IfcFile, Kind, Record, Value, at_bytes, codes};

/// 表現の文脈の元値と単位。意味値と L1 の出所を分ける
#[derive(Clone, Debug)]
pub struct ContextGeometry<'f> {
    /// 座標系・精度・表示尺度の定義
    pub definition: RepresentationContext,
    /// 原点と精度の長さの単位。換算は行わない
    pub length_unit: Option<Unit>,
    /// 元のレコードへの参照
    pub source: ContextSource<'f>,
}

/// 文脈を取得した出所
#[derive(Clone, Debug)]
pub struct ContextSource<'f> {
    /// 取得で指定した文脈
    pub context: &'f Record,
    /// 子の場合の親。親を再帰することは認めない
    pub parent: Option<&'f Record>,
    /// 読んだ文脈・配置・点・方向・単位を読み取り順に保持する
    pub records: Vec<&'f Record>,
}

impl IfcFile {
    /// 幾何の文脈かその子から、座標系と表示尺度を取得する。
    ///
    /// 子の派生属性は親から読み、親と子の名前を分けて保持する。深さは親までの最大 2 段。
    /// 世界座標・地図への変換・真北・尺度は未適用。宣言した次元と座標系の次元の幾何評価は行わない。
    pub fn representation_context(
        &self,
        instance: u64,
        cx: &mut Cx,
    ) -> Result<ContextGeometry<'_>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: instance,
                records: Vec::new(),
                shape_diagnostics: true,
            };
            reader.check(Kind::Work, self.schema().len() as u64)?;
            let Some(version) = Version::of(self.schema()) else {
                reader.alloc(self.schema().len(), 3)?;
                reader
                    .cx
                    .report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                        "schema",
                        String::from_utf8_lossy(self.schema()).into_owned(),
                    ));
                return Err(Error::Unsupported);
            };
            let mut entered = 0;
            let result = reader.context_geometry(instance, version, &mut entered);
            for _ in 0..entered {
                reader.cx.leave();
            }
            result
        })
    }
}

impl<'f> Reader<'f, '_, '_> {
    fn context_text(&mut self, record: &Record, at: usize) -> Result<Option<String>, Error> {
        self.current = record.id().unwrap_or(0);
        match arg(record, at) {
            Some(Value::Null) => Ok(None),
            Some(Value::Text { raw, .. }) => {
                self.check(Kind::Work, raw.len() as u64)?;
                self.alloc(raw.len(), 12)?;
                let (text, invalid) = self.file.decode_text(self.file.bytes(raw.clone()));
                if invalid {
                    self.cx.report(
                        Diagnostic::new(codes::IFC_TEXT_INVALID).at(at_bytes(record.span())),
                    );
                }
                Ok(Some(text))
            }
            _ => Err(self.invalid("context-text")),
        }
    }

    fn sub_context(&mut self, record: &Record) -> Result<SubContext, Error> {
        self.current = record.id().unwrap_or(0);
        if record.parts().first().map_or(0, |p| p.args().len()) != 10
            || !(3..=6).all(|at| matches!(arg(record, at), Some(Value::Derived)))
        {
            return Err(self.invalid("context-attributes"));
        }
        let identifier = self.context_text(record, 1)?;
        let kind = self.context_text(record, 2)?;
        let target_scale = match arg(record, 8) {
            Some(Value::Null) => None,
            value => {
                let scale = self.number(value)?;
                if scale <= 0. {
                    return Err(self.invalid("context-scale"));
                }
                Some(scale)
            }
        };
        let Some(Value::Enum(raw)) = arg(record, 9) else {
            return Err(self.invalid("context-view"));
        };
        self.check(Kind::Work, raw.len() as u64)?;
        let target_view = match self.file.bytes(raw.clone()) {
            b"GRAPH_VIEW" => TargetView::Graph,
            b"SKETCH_VIEW" => TargetView::Sketch,
            b"MODEL_VIEW" => TargetView::Model,
            b"PLAN_VIEW" => TargetView::Plan,
            b"REFLECTED_PLAN_VIEW" => TargetView::ReflectedPlan,
            b"SECTION_VIEW" => TargetView::Section,
            b"ELEVATION_VIEW" => TargetView::Elevation,
            b"USERDEFINED" => TargetView::UserDefined,
            b"NOTDEFINED" => TargetView::NotDefined,
            _ => return Err(self.invalid("context-view")),
        };
        let user_defined_target_view = self.context_text(record, 10)?;
        if target_view == TargetView::UserDefined && user_defined_target_view.is_none() {
            return Err(self.invalid("context-view-name"));
        }
        Ok(SubContext {
            identifier,
            kind,
            target_scale,
            target_view,
            user_defined_target_view,
        })
    }

    fn context_geometry(
        &mut self,
        instance: u64,
        version: Version,
        entered: &mut u64,
    ) -> Result<ContextGeometry<'f>, Error> {
        self.enter(entered)?;
        self.check(Kind::Elements, 1)?;
        let context = self.take_typed(instance)?;
        let (base, view, parent) = if self.is(context, b"IFCGEOMETRICREPRESENTATIONSUBCONTEXT") {
            let view = self.sub_context(context)?;
            self.current = self.target(context, 7)?;
            self.enter(entered)?;
            self.check(Kind::Elements, 1)?;
            let parent = self.take_typed(self.current)?;
            // ParentNoSub により、自己参照・相互参照・子から子をたどる鎖をここで止める。
            if !self.is(parent, b"IFCGEOMETRICREPRESENTATIONCONTEXT") {
                return Err(self.invalid("context-parent"));
            }
            (parent, Some(view), Some(parent))
        } else if self.is(context, b"IFCGEOMETRICREPRESENTATIONCONTEXT") {
            (context, None, None)
        } else {
            return Err(self.unsupported_kind("context"));
        };
        self.current = base.id().unwrap_or(0);
        if base.parts().first().map_or(0, |p| p.args().len()) != 6 {
            return Err(self.invalid("context-attributes"));
        }
        let identifier = self.context_text(base, 1)?;
        let kind = self.context_text(base, 2)?;
        let Some(Value::Number(raw)) = arg(base, 3) else {
            return Err(self.invalid("context-dimension"));
        };
        self.check(Kind::Work, raw.len() as u64)?;
        let dimension = std::str::from_utf8(self.file.bytes(raw.clone()))
            .ok()
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|v| (1..=3).contains(v))
            .ok_or_else(|| self.invalid("context-dimension"))?;
        let precision = match arg(base, 4) {
            Some(Value::Null) => None,
            value => Some(self.number(value)?),
        };
        self.current = self.target(base, 5)?;
        let position = self.take_typed(self.current)?;
        let world_coordinate_system = if self.is(position, b"IFCAXIS2PLACEMENT2D") {
            ContextPlacement::Plane(self.placement2(self.current, version)?)
        } else if self.is(position, b"IFCAXIS2PLACEMENT3D") {
            ContextPlacement::Space(self.placement(self.current)?)
        } else {
            return Err(self.invalid("context-placement"));
        };
        self.current = base.id().unwrap_or(0);
        let true_north = if matches!(arg(base, 6), Some(Value::Null)) {
            None
        } else {
            let id = self.target(base, 6)?;
            Some(self.vector2(id, false, version)?)
        };
        let records = core::mem::take(&mut self.records);
        let checked = self.strict_attributes(&records);
        self.records = records;
        checked?;
        let (length_unit, units) = face_length_unit(self.file, context, version, self.cx)?;
        self.current = instance;
        self.alloc(units.len(), 32)?;
        self.records.extend(units);
        if length_unit.is_none() {
            self.cx.report(
                Diagnostic::new(codes::IFC_UNIT_MISSING)
                    .with("type", "IfcLengthMeasure")
                    .at(at_bytes(context.span())),
            );
        }
        self.alloc(1, 1024)?;
        Ok(ContextGeometry {
            definition: RepresentationContext {
                geometric: GeometricContext {
                    identifier,
                    kind,
                    dimension,
                    precision,
                    world_coordinate_system,
                    true_north,
                },
                view,
            },
            length_unit,
            source: ContextSource {
                context,
                parent,
                records: core::mem::take(&mut self.records),
            },
        })
    }
}
