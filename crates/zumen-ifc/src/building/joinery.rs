//! 図面の記号選択に使う、建具の直接の属性。元のプロパティセットへ混ぜない。
//! buildingSMART IFC4 ADD2 TC1 / IFC4.3 ADD2 の IfcDoor・IfcWindow・各 Type の
//! Attributes、IFC2X3 TC1 の DoorStyle・WindowStyle の属性の自然文による。

use super::*;
use zumen_building::{Extension, ExtensionValue};

/// 建具の元の属性に記録された全高と全幅。値は文書の長さ単位で、未記録の寸法は持たない。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JoineryDimensions {
    /// IfcDoor または IfcWindow の OverallHeight
    pub overall_height: Option<f64>,
    /// IfcDoor または IfcWindow の OverallWidth
    pub overall_width: Option<f64>,
}

impl IfcFile {
    /// 建具の実例から直接の寸法を読む。開口の形や数量セットの値は使わない。
    pub fn joinery_dimensions(&self, instance: u64, cx: &mut Cx) -> Option<JoineryDimensions> {
        let record = self.instance(instance)?;
        let kind = self.kind(record);
        if ![
            b"IFCDOOR".as_slice(),
            b"IFCDOORSTANDARDCASE",
            b"IFCWINDOW",
            b"IFCWINDOWSTANDARDCASE",
        ]
        .iter()
        .any(|name| kind.eq_ignore_ascii_case(name))
        {
            return None;
        }
        Some(self.joinery_dimensions_of(record, cx))
    }

    fn joinery_dimensions_of(&self, record: &Record, cx: &mut Cx) -> JoineryDimensions {
        let mut dimension = |position| {
            let source = arg(record, position)?;
            if matches!(source, Value::Null | Value::Derived) {
                return None;
            }
            let value = number(self, Some(source)).filter(|v| v.is_finite() && *v > 0.0);
            if value.is_none() {
                invalid_attribute(self, record, position as u64, cx);
            }
            value
        };
        JoineryDimensions {
            overall_height: dimension(9),
            overall_width: dimension(10),
        }
    }
}

impl Lifter<'_> {
    // R4 の形状付きの読み取りだけに追加する。通常の Lift と常駐編集の属性の契約は変えない。
    pub(super) fn joinery_attributes(&mut self, cx: &mut Cx) -> Result<(), Error> {
        for index in 0..self.building.elements().len() {
            cx.check(Kind::Work, 1).map_err(|s| stopped(cx, s, None))?;
            let element = ElementRef(index as u32);
            let Some(id) = self
                .building
                .element(element)
                .and_then(|e| e.source().instance)
            else {
                continue;
            };
            let Some(record) = self.file.instance(id) else {
                continue;
            };
            let kind = self.file.kind(record);
            let door = kind.eq_ignore_ascii_case(b"IFCDOOR")
                || kind.eq_ignore_ascii_case(b"IFCDOORSTANDARDCASE");
            let window = kind.eq_ignore_ascii_case(b"IFCWINDOW")
                || kind.eq_ignore_ascii_case(b"IFCWINDOWSTANDARDCASE");
            let door_type = kind.eq_ignore_ascii_case(b"IFCDOORTYPE");
            let window_type = kind.eq_ignore_ascii_case(b"IFCWINDOWTYPE");
            let door_style = kind.eq_ignore_ascii_case(b"IFCDOORSTYLE");
            let window_style = kind.eq_ignore_ascii_case(b"IFCWINDOWSTYLE");
            if !(door || window || door_type || window_type || door_style || window_style) {
                continue;
            }
            // 最大 3 属性、型・名前・値と列の確保。元の文字列は長さを別に数える。
            if !self.copied(cx, 2048) {
                self.halted(cx, Some(record.span()))?;
            }
            let mut values = Vec::with_capacity(3);
            if door || window {
                let dimensions = self.file.joinery_dimensions_of(record, cx);
                for (name, number) in [
                    ("OverallHeight", dimensions.overall_height),
                    ("OverallWidth", dimensions.overall_width),
                ] {
                    if let Some(number) = number {
                        values.push(attribute(
                            name,
                            "IfcPositiveLengthMeasure",
                            Literal::Real(number),
                        ));
                    }
                }
            }
            let operation = if (door || window) && self.version != Version::Ifc2x3 {
                Some((
                    12,
                    if door {
                        "OperationType"
                    } else {
                        "PartitioningType"
                    },
                    if door {
                        "IfcDoorTypeOperationEnum"
                    } else {
                        "IfcWindowTypePartitioningEnum"
                    },
                ))
            } else if door_type || window_type {
                Some((
                    11,
                    if door_type {
                        "OperationType"
                    } else {
                        "PartitioningType"
                    },
                    if door_type {
                        "IfcDoorTypeOperationEnum"
                    } else {
                        "IfcWindowTypePartitioningEnum"
                    },
                ))
            } else if door_style {
                Some((9, "OperationType", "IfcDoorStyleOperationEnum"))
            } else if window_style {
                Some((10, "OperationType", "IfcWindowStyleOperationEnum"))
            } else {
                None
            };
            if let Some((position, name, type_name)) = operation {
                match arg(record, position) {
                    None | Some(Value::Null | Value::Derived) => {}
                    Some(Value::Enum(range)) => {
                        if !self.copied(cx, range.len() as u64) {
                            self.halted(cx, Some(record.span()))?;
                        }
                        if let Some(value) = self.enumeration(arg(record, position)) {
                            values.push(attribute(name, type_name, Literal::Enumeration(value)));
                        }
                    }
                    _ => self.invalid(record, position as u64, cx),
                }
            }
            if !values.is_empty() {
                self.building
                    .components
                    .extension
                    .insert(element, Extension { values });
            }
            self.halted(cx, Some(record.span()))?;
        }
        Ok(())
    }
}

fn attribute(name: &str, type_name: &str, value: Literal) -> ExtensionValue {
    ExtensionValue {
        namespace: "ifc.attribute".into(),
        name: name.into(),
        value: TypedValue {
            type_name: Some(type_name.into()),
            value,
        },
    }
}
