//! IFC から建物モデルを作る（決定 0207・0215）。
//!
//! 意味を持つ実体ごとに、版ごとの属性の位置を手で書いた表で読む。位置は、各版の仕様の実体の定義（IFC2X3 TC1・
//! IFC4 ADD2 TC1・IFC 4.3 ADD2 の 5 章から 8 章の、実体ごとの「属性」の節）による。形状表現・表示の様式・階・区域・
//! 分類は、まだ作らない（決定 0207 の「試料を得てから決めること」）。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Diagnostic, Kind, codes};
use zumen_building::{
    Application, Building, Class, Connection, Context, Document, ElementRef, Grid, GridSet, Header,
    Identity, Link, Literal, Local, MapConversion, Material, MaterialKind, MaterialPart, Measured,
    Membership, Opening, ParentKind, Placement, Properties, Property, PropertySet, Quantities,
    Quantity, QuantityKind, QuantitySet, RelationSource, Spatial, SpatialKind, Transform, TypeOf,
    TypedValue, Unit,
};
use zumen_sdk::{Cx, Error, Lift};

use crate::{IfcFile, IfcZipFile, Record, Value, at_bytes, stopped};

/// 建物モデルを作る版
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Version {
    Ifc2x3,
    Ifc4,
    Ifc4x3,
}

impl Version {
    /// `FILE_SCHEMA` の名前から。IFC4X1・IFC4X2 は正式の版でないので読まない
    fn of(schema: &[u8]) -> Option<Self> {
        let upper = schema.to_ascii_uppercase();
        if upper.starts_with(b"IFC2X3") {
            Some(Self::Ifc2x3)
        } else if upper.starts_with(b"IFC4X3") {
            Some(Self::Ifc4x3)
        } else if upper == b"IFC4" {
            Some(Self::Ifc4)
        } else {
            None
        }
    }

    /// 表の列
    fn column(self) -> usize {
        match self {
            Self::Ifc2x3 => 0,
            Self::Ifc4 => 1,
            Self::Ifc4x3 => 2,
        }
    }
}

/// 実体の分け方。属性の位置は分け方ごとに同じ
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    /// `IfcProject`。5 ObjectType・6 LongName・7 Phase・8 RepresentationContexts・9 UnitsInContext
    Project,
    /// `IfcSpatialStructureElement`（IFC4 からは `IfcSpatialElement`）。6 ObjectPlacement・8 LongName
    Spatial,
    /// `IfcElement`。5 ObjectType・6 ObjectPlacement・8 Tag
    Element,
    /// `IfcElementType`。6 HasPropertySets・8 Tag・9 ElementType
    Type,
    /// `IfcDoorStyle`・`IfcWindowStyle`（`IfcTypeProduct`）。6 HasPropertySets・8 Tag
    Style,
    /// `IfcGroup`。5 ObjectType
    Group,
    /// `IfcGrid`。6 ObjectPlacement・8 UAxes・9 VAxes・10 WAxes
    Grid,
}

/// その版に実体がない
const ABSENT: u8 = u8::MAX;
/// その版の実体は、定義された種類を持たない
const NONE: u8 = 0;

/// 意味を持つ実体。`predefined` は、IFC2X3・IFC4・IFC4X3 の順の、定義された種類の属性の位置（1 から）
struct Entity {
    /// 元のスキーマでの名前
    name: &'static str,
    /// IFC4.3 でのクラスの名前。`StandardCase` などの派生は、元のクラスにまとめる
    class: &'static str,
    category: Category,
    predefined: [u8; 3],
}

const fn entity(
    name: &'static str,
    class: &'static str,
    category: Category,
    predefined: [u8; 3],
) -> Entity {
    Entity {
        name,
        class,
        category,
        predefined,
    }
}

use Category::{
    Element as E, Grid as G, Group as R, Project as P, Spatial as S, Style as Y, Type as T,
};

/// 意味を持つ実体の表。IFC2X3 の `IfcRoof`・`IfcStair`・`IfcRamp` の 9 ShapeType は、IFC4 の PredefinedType と同じ
/// 列挙（`IfcRoofTypeEnum` など）なので、定義された種類として読む
const ENTITIES: &[Entity] = &[
    entity("IfcProject", "IfcProject", P, [NONE, NONE, NONE]),
    entity("IfcSite", "IfcSite", S, [NONE, NONE, NONE]),
    entity("IfcBuilding", "IfcBuilding", S, [NONE, NONE, NONE]),
    entity(
        "IfcBuildingStorey",
        "IfcBuildingStorey",
        S,
        [NONE, NONE, NONE],
    ),
    entity("IfcSpace", "IfcSpace", S, [NONE, 10, 10]),
    entity("IfcSpatialZone", "IfcSpatialZone", S, [ABSENT, 9, 9]),
    entity("IfcFacility", "IfcFacility", S, [ABSENT, ABSENT, NONE]),
    entity("IfcBridge", "IfcBridge", S, [ABSENT, ABSENT, 10]),
    entity("IfcRoad", "IfcRoad", S, [ABSENT, ABSENT, 10]),
    entity("IfcRailway", "IfcRailway", S, [ABSENT, ABSENT, 10]),
    entity("IfcWall", "IfcWall", E, [NONE, 9, 9]),
    entity("IfcWallStandardCase", "IfcWall", E, [NONE, 9, 9]),
    entity("IfcWallElementedCase", "IfcWall", E, [ABSENT, 9, ABSENT]),
    entity("IfcCurtainWall", "IfcCurtainWall", E, [NONE, 9, 9]),
    entity("IfcSlab", "IfcSlab", E, [9, 9, 9]),
    entity("IfcSlabStandardCase", "IfcSlab", E, [ABSENT, 9, ABSENT]),
    entity("IfcSlabElementedCase", "IfcSlab", E, [ABSENT, 9, ABSENT]),
    entity("IfcColumn", "IfcColumn", E, [NONE, 9, 9]),
    entity("IfcColumnStandardCase", "IfcColumn", E, [ABSENT, 9, ABSENT]),
    entity("IfcBeam", "IfcBeam", E, [NONE, 9, 9]),
    entity("IfcBeamStandardCase", "IfcBeam", E, [ABSENT, 9, ABSENT]),
    entity("IfcMember", "IfcMember", E, [NONE, 9, 9]),
    entity("IfcMemberStandardCase", "IfcMember", E, [ABSENT, 9, ABSENT]),
    entity("IfcPlate", "IfcPlate", E, [NONE, 9, 9]),
    entity("IfcPlateStandardCase", "IfcPlate", E, [ABSENT, 9, ABSENT]),
    entity("IfcRoof", "IfcRoof", E, [9, 9, 9]),
    entity("IfcStair", "IfcStair", E, [9, 9, 9]),
    entity("IfcStairFlight", "IfcStairFlight", E, [NONE, 13, 13]),
    entity("IfcRamp", "IfcRamp", E, [9, 9, 9]),
    entity("IfcRampFlight", "IfcRampFlight", E, [NONE, 9, 9]),
    entity("IfcRailing", "IfcRailing", E, [9, 9, 9]),
    entity("IfcCovering", "IfcCovering", E, [9, 9, 9]),
    entity("IfcFooting", "IfcFooting", E, [9, 9, 9]),
    entity("IfcPile", "IfcPile", E, [9, 9, 9]),
    entity("IfcDoor", "IfcDoor", E, [NONE, 11, 11]),
    entity("IfcDoorStandardCase", "IfcDoor", E, [ABSENT, 11, ABSENT]),
    entity("IfcWindow", "IfcWindow", E, [NONE, 11, 11]),
    entity(
        "IfcWindowStandardCase",
        "IfcWindow",
        E,
        [ABSENT, 11, ABSENT],
    ),
    entity(
        "IfcBuildingElementProxy",
        "IfcBuildingElementProxy",
        E,
        [NONE, 9, 9],
    ),
    entity("IfcOpeningElement", "IfcOpeningElement", E, [NONE, 9, 9]),
    entity(
        "IfcOpeningStandardCase",
        "IfcOpeningElement",
        E,
        [ABSENT, 9, ABSENT],
    ),
    entity(
        "IfcFurnishingElement",
        "IfcFurnishingElement",
        E,
        [NONE, NONE, NONE],
    ),
    entity("IfcFurniture", "IfcFurniture", E, [ABSENT, 9, 9]),
    entity("IfcElementAssembly", "IfcElementAssembly", E, [10, 10, 10]),
    entity(
        "IfcDistributionElement",
        "IfcDistributionElement",
        E,
        [NONE, NONE, NONE],
    ),
    entity("IfcFlowSegment", "IfcFlowSegment", E, [NONE, NONE, NONE]),
    entity("IfcFlowFitting", "IfcFlowFitting", E, [NONE, NONE, NONE]),
    entity("IfcFlowTerminal", "IfcFlowTerminal", E, [NONE, NONE, NONE]),
    entity("IfcWallType", "IfcWallType", T, [10, 10, 10]),
    entity("IfcCurtainWallType", "IfcCurtainWallType", T, [10, 10, 10]),
    entity("IfcSlabType", "IfcSlabType", T, [10, 10, 10]),
    entity("IfcColumnType", "IfcColumnType", T, [10, 10, 10]),
    entity("IfcBeamType", "IfcBeamType", T, [10, 10, 10]),
    entity("IfcMemberType", "IfcMemberType", T, [10, 10, 10]),
    entity("IfcPlateType", "IfcPlateType", T, [10, 10, 10]),
    entity("IfcRoofType", "IfcRoofType", T, [ABSENT, 10, 10]),
    entity("IfcStairType", "IfcStairType", T, [ABSENT, 10, 10]),
    entity("IfcStairFlightType", "IfcStairFlightType", T, [10, 10, 10]),
    entity("IfcRampType", "IfcRampType", T, [ABSENT, 10, 10]),
    entity("IfcRailingType", "IfcRailingType", T, [10, 10, 10]),
    entity("IfcCoveringType", "IfcCoveringType", T, [10, 10, 10]),
    entity("IfcFootingType", "IfcFootingType", T, [ABSENT, 10, 10]),
    entity("IfcPileType", "IfcPileType", T, [ABSENT, 10, 10]),
    entity("IfcDoorType", "IfcDoorType", T, [ABSENT, 10, 10]),
    entity("IfcWindowType", "IfcWindowType", T, [ABSENT, 10, 10]),
    entity(
        "IfcBuildingElementProxyType",
        "IfcBuildingElementProxyType",
        T,
        [10, 10, 10],
    ),
    entity(
        "IfcFurnishingElementType",
        "IfcFurnishingElementType",
        T,
        [NONE, NONE, NONE],
    ),
    entity("IfcFurnitureType", "IfcFurnitureType", T, [NONE, 11, 11]),
    entity(
        "IfcElementAssemblyType",
        "IfcElementAssemblyType",
        T,
        [ABSENT, 10, 10],
    ),
    entity("IfcSpaceType", "IfcSpaceType", T, [10, 10, 10]),
    entity("IfcDoorStyle", "IfcDoorStyle", Y, [NONE, NONE, ABSENT]),
    entity("IfcWindowStyle", "IfcWindowStyle", Y, [NONE, NONE, ABSENT]),
    entity("IfcGroup", "IfcGroup", R, [NONE, NONE, NONE]),
    entity("IfcSystem", "IfcSystem", R, [NONE, NONE, NONE]),
    entity("IfcZone", "IfcZone", R, [NONE, NONE, NONE]),
    entity(
        "IfcDistributionSystem",
        "IfcDistributionSystem",
        R,
        [ABSENT, 7, 7],
    ),
    entity("IfcBuildingSystem", "IfcBuildingSystem", R, [ABSENT, 6, 6]),
    entity("IfcGrid", "IfcGrid", G, [NONE, 11, 11]),
];

/// 値の型の名前と、値の単位の種類（`IfcUnitEnum`）。単位を持たない型は `None`
pub(crate) const MEASURES: &[(&str, Option<&str>)] = &[
    ("IfcLengthMeasure", Some("LENGTHUNIT")),
    ("IfcPositiveLengthMeasure", Some("LENGTHUNIT")),
    ("IfcNonNegativeLengthMeasure", Some("LENGTHUNIT")),
    ("IfcAreaMeasure", Some("AREAUNIT")),
    ("IfcVolumeMeasure", Some("VOLUMEUNIT")),
    ("IfcPlaneAngleMeasure", Some("PLANEANGLEUNIT")),
    ("IfcPositivePlaneAngleMeasure", Some("PLANEANGLEUNIT")),
    ("IfcSolidAngleMeasure", Some("SOLIDANGLEUNIT")),
    ("IfcMassMeasure", Some("MASSUNIT")),
    ("IfcTimeMeasure", Some("TIMEUNIT")),
    (
        "IfcThermodynamicTemperatureMeasure",
        Some("THERMODYNAMICTEMPERATUREUNIT"),
    ),
    ("IfcElectricCurrentMeasure", Some("ELECTRICCURRENTUNIT")),
    ("IfcElectricVoltageMeasure", Some("ELECTRICVOLTAGEUNIT")),
    ("IfcLuminousIntensityMeasure", Some("LUMINOUSINTENSITYUNIT")),
    ("IfcLuminousFluxMeasure", Some("LUMINOUSFLUXUNIT")),
    ("IfcIlluminanceMeasure", Some("ILLUMINANCEUNIT")),
    ("IfcAmountOfSubstanceMeasure", Some("AMOUNTOFSUBSTANCEUNIT")),
    ("IfcEnergyMeasure", Some("ENERGYUNIT")),
    ("IfcPowerMeasure", Some("POWERUNIT")),
    ("IfcPressureMeasure", Some("PRESSUREUNIT")),
    ("IfcForceMeasure", Some("FORCEUNIT")),
    ("IfcFrequencyMeasure", Some("FREQUENCYUNIT")),
    ("IfcCountMeasure", None),
    ("IfcRatioMeasure", None),
    ("IfcPositiveRatioMeasure", None),
    ("IfcNormalisedRatioMeasure", None),
    ("IfcNumericMeasure", None),
    ("IfcDescriptiveMeasure", None),
    ("IfcLabel", None),
    ("IfcText", None),
    ("IfcIdentifier", None),
    ("IfcBoolean", None),
    ("IfcLogical", None),
    ("IfcInteger", None),
    ("IfcReal", None),
    ("IfcDate", None),
    ("IfcTime", None),
    ("IfcDateTime", None),
    ("IfcDuration", None),
    ("IfcTimeStamp", None),
    ("IfcURIReference", None),
];

/// SI の接頭辞と倍率
const PREFIXES: &[(&str, f64)] = &[
    ("EXA", 1e18),
    ("PETA", 1e15),
    ("TERA", 1e12),
    ("GIGA", 1e9),
    ("MEGA", 1e6),
    ("KILO", 1e3),
    ("HECTO", 1e2),
    ("DECA", 1e1),
    ("DECI", 1e-1),
    ("CENTI", 1e-2),
    ("MILLI", 1e-3),
    ("MICRO", 1e-6),
    ("NANO", 1e-9),
    ("PICO", 1e-12),
    ("FEMTO", 1e-15),
    ("ATTO", 1e-18),
];

/// 数量の実体と、数量の種類と、単位の種類。属性は 1 Name・3 Unit・4 値・5 Formula（IFC4 から）
const QUANTITIES: &[(&str, QuantityKind, Option<&str>)] = &[
    (
        "IFCQUANTITYLENGTH",
        QuantityKind::Length,
        Some("LENGTHUNIT"),
    ),
    ("IFCQUANTITYAREA", QuantityKind::Area, Some("AREAUNIT")),
    (
        "IFCQUANTITYVOLUME",
        QuantityKind::Volume,
        Some("VOLUMEUNIT"),
    ),
    ("IFCQUANTITYCOUNT", QuantityKind::Count, None),
    ("IFCQUANTITYWEIGHT", QuantityKind::Weight, Some("MASSUNIT")),
    ("IFCQUANTITYTIME", QuantityKind::Time, Some("TIMEUNIT")),
];

/// 空間の要素どうしをつなぐ関係の、空間の側から引く種類
const SPACE_BOUNDARIES: &[(&str, &str)] = &[
    ("IFCRELSPACEBOUNDARY", "IfcRelSpaceBoundary"),
    ("IFCRELSPACEBOUNDARY1STLEVEL", "IfcRelSpaceBoundary1stLevel"),
    ("IFCRELSPACEBOUNDARY2NDLEVEL", "IfcRelSpaceBoundary2ndLevel"),
];

impl Lift<Building> for IfcFile {
    /// IFC2X3・IFC4・IFC4X3 から作る。ほかの版は、診断して作らない（決定 0215）
    fn lift(&self, cx: &mut Cx) -> Result<Building, Error> {
        let Some(version) = Version::of(self.schema()) else {
            cx.report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                "schema",
                String::from_utf8_lossy(self.schema()).into_owned(),
            ));
            return Err(Error::Unsupported);
        };
        Lifter::new(self, version).run(cx)
    }
}

impl Lift<Building> for IfcZipFile {
    /// 書庫の中の IFC-SPF から作る
    fn lift(&self, cx: &mut Cx) -> Result<Building, Error> {
        self.ifc().lift(cx)
    }
}

/// 読んで作った定義。同じ定義を複数の要素が使うので、一度だけ読む
#[derive(Clone)]
enum Definition {
    Properties(PropertySet),
    Quantities(QuantitySet),
}

struct Lifter<'f> {
    file: &'f IfcFile,
    version: Version,
    entities: BTreeMap<Vec<u8>, &'static Entity>,
    building: Building,
    /// インスタンスの番号から要素
    elements: BTreeMap<u64, ElementRef>,
    /// 建物モデルに写したインスタンス
    used: BTreeSet<u64>,
    /// 要素と、要素の持つ配置のインスタンス。書いた順
    placements: Vec<(ElementRef, u64)>,
    /// 配置のインスタンスから、それを持つ初めの要素
    owners: BTreeMap<u64, ElementRef>,
    /// 格子と、そのインスタンス
    grids: Vec<(ElementRef, u64)>,
    /// 型と、そのインスタンス
    types: Vec<(ElementRef, u64)>,
    units: BTreeMap<u64, Option<Unit>>,
    definitions: BTreeMap<u64, Option<Definition>>,
    materials: BTreeMap<u64, Option<Material>>,
}

impl<'f> Lifter<'f> {
    fn new(file: &'f IfcFile, version: Version) -> Self {
        let entities = ENTITIES
            .iter()
            .filter(|e| {
                e.predefined
                    .get(version.column())
                    .is_some_and(|&at| at != ABSENT)
            })
            .map(|e| (e.name.to_ascii_uppercase().into_bytes(), e))
            .collect();
        Self {
            file,
            version,
            entities,
            building: Building::new(Document::default()),
            elements: BTreeMap::new(),
            used: BTreeSet::new(),
            placements: Vec::new(),
            owners: BTreeMap::new(),
            grids: Vec::new(),
            types: Vec::new(),
            units: BTreeMap::new(),
            definitions: BTreeMap::new(),
            materials: BTreeMap::new(),
        }
    }

    fn run(mut self, cx: &mut Cx) -> Result<Building, Error> {
        self.building.document.header = self.header(cx);
        let project = self.find(b"IFCPROJECT");
        if let Some(project) = project {
            self.document_units(project, cx);
        }
        let ids: Vec<u64> = self.file.ids().collect();
        for &id in &ids {
            let Some(record) = self.file.instance(id) else {
                continue;
            };
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            self.element(id, record, cx)?;
        }
        if let Some(project) = project {
            self.document(project, cx);
        }
        self.contexts(&ids, cx);
        for &id in &ids {
            let Some(record) = self.file.instance(id) else {
                continue;
            };
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            self.relation(id, record, cx);
        }
        for (element, id) in core::mem::take(&mut self.types) {
            if let Some(record) = self.file.instance(id) {
                let sets = refs(arg(record, 6));
                self.definitions_of(element, &sets, cx);
            }
        }
        for (element, id) in core::mem::take(&mut self.grids) {
            self.grid(element, id, cx);
        }
        for (element, id) in core::mem::take(&mut self.placements) {
            self.placement(element, id, cx)?;
        }
        for &id in &ids {
            if self.used.contains(&id) {
                continue;
            }
            let Some(record) = self.file.instance(id) else {
                continue;
            };
            cx.report(
                Diagnostic::new(codes::IFC_ENTITY_DROPPED)
                    .with(
                        "kind",
                        String::from_utf8_lossy(self.file.kind(record)).into_owned(),
                    )
                    .at(at_bytes(record.span())),
            );
        }
        Ok(self.building)
    }

    /// 初めの、その種類のインスタンス
    fn find(&self, kind: &[u8]) -> Option<u64> {
        self.file.ids().find(|&id| self.is(id, kind))
    }

    /// インスタンスの種類が `kind`（大文字）か
    fn is(&self, id: u64, kind: &[u8]) -> bool {
        self.file
            .instance(id)
            .is_some_and(|record| self.file.kind(record).eq_ignore_ascii_case(kind))
    }

    /// 写すインスタンスを引き、使ったことを覚える
    fn take(&mut self, id: u64) -> Option<&'f Record> {
        let record = self.file.instance(id)?;
        self.used.insert(id);
        Some(record)
    }

    // 値

    /// 文字列のエスケープを戻す。0x80 以上のバイトは UTF-8 として読み、読めない所は置き換えの文字にして診断する
    fn text(&self, value: Option<&Value>, place: &Record, cx: &mut Cx) -> Option<String> {
        let Some(Value::Text { raw, .. }) = value else {
            return None;
        };
        let (text, replaced) = decode(self.file.bytes(raw.clone()));
        if replaced {
            cx.report(Diagnostic::new(codes::IFC_TEXT_INVALID).at(at_bytes(place.span())));
        }
        Some(text)
    }

    /// 文字列の列
    fn texts(&self, value: Option<&Value>, place: &Record, cx: &mut Cx) -> Vec<String> {
        match value {
            Some(Value::List(values)) => values
                .iter()
                .filter_map(|v| self.text(Some(v), place, cx))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// 列挙の値の名前（`.ELEMENT.` の `ELEMENT`）
    fn enumeration(&self, value: Option<&Value>) -> Option<String> {
        match value {
            Some(Value::Enum(range)) => {
                Some(String::from_utf8_lossy(self.file.bytes(range.clone())).into_owned())
            }
            _ => None,
        }
    }

    /// 数。型を付けた値（`IFCLENGTHMEASURE(1.)`）なら、その中の数
    fn real(&self, value: Option<&Value>) -> Option<f64> {
        match value? {
            Value::Number(range) => core::str::from_utf8(self.file.bytes(range.clone()))
                .ok()?
                .parse()
                .ok(),
            Value::Typed(part) => match part.args() {
                [only] => self.real(Some(only)),
                _ => None,
            },
            _ => None,
        }
    }

    /// 数の列
    fn reals(&self, value: Option<&Value>) -> Option<Vec<f64>> {
        match value? {
            Value::List(values) => values.iter().map(|v| self.real(Some(v))).collect(),
            _ => None,
        }
    }

    /// 長さを、文書の長さの単位とともに
    fn length(&self, value: Option<&Value>, place: &Record, cx: &mut Cx) -> Option<Measured> {
        let value = self.real(value)?;
        Some(Measured {
            value,
            unit: self.document_unit("LENGTHUNIT", "IfcLengthMeasure", place, cx),
        })
    }

    /// 型の位置の値が読めないことを診断する
    fn invalid(&self, record: &Record, position: u64, cx: &mut Cx) {
        cx.report(
            Diagnostic::new(codes::IFC_ATTRIBUTE_INVALID)
                .with(
                    "kind",
                    String::from_utf8_lossy(self.file.kind(record)).into_owned(),
                )
                .with("position", position)
                .at(at_bytes(record.span())),
        );
    }

    // 見出しと文書

    /// `FILE_DESCRIPTION`・`FILE_NAME`・`FILE_SCHEMA` の値
    fn header(&self, cx: &mut Cx) -> Header {
        let mut header = Header::default();
        for record in self.file.records() {
            if record.id().is_some() {
                continue;
            }
            let kind = self.file.kind(record);
            if kind.eq_ignore_ascii_case(b"FILE_DESCRIPTION") {
                header.description = self.texts(arg(record, 1), record, cx);
            } else if kind.eq_ignore_ascii_case(b"FILE_NAME") {
                header.name = self.text(arg(record, 1), record, cx);
                header.time_stamp = self.text(arg(record, 2), record, cx);
                header.author = self.texts(arg(record, 3), record, cx);
                header.organization = self.texts(arg(record, 4), record, cx);
                header.preprocessor = self.text(arg(record, 5), record, cx);
                header.originating_system = self.text(arg(record, 6), record, cx);
                header.authorization = self.text(arg(record, 7), record, cx);
            } else if kind.eq_ignore_ascii_case(b"FILE_SCHEMA") {
                header.schemas = self.texts(arg(record, 1), record, cx);
            }
        }
        header
    }

    /// プロジェクトの単位の割り当て（9 UnitsInContext → `IfcUnitAssignment` の 1 Units）
    fn document_units(&mut self, project: u64, cx: &mut Cx) {
        let Some(record) = self.file.instance(project) else {
            return;
        };
        let Some(assignment) = reference(arg(record, 9)) else {
            return;
        };
        if !self.is(assignment, b"IFCUNITASSIGNMENT") {
            return;
        }
        let Some(assignment) = self.take(assignment) else {
            return;
        };
        for id in refs(arg(assignment, 1)) {
            if let Some(unit) = self.unit(id, cx) {
                self.building.document.units.push(unit);
            }
        }
    }

    /// プロジェクトの名前・段階と、書き出したアプリケーションと、地図への変換
    fn document(&mut self, project: u64, cx: &mut Cx) {
        let Some(record) = self.file.instance(project) else {
            return;
        };
        let document = &mut self.building.document;
        document.project = self.elements.get(&project).copied();
        let name = self.text(arg(record, 3), record, cx);
        let phase = self.text(arg(record, 7), record, cx);
        let application = self.application(reference(arg(record, 2)), cx);
        let map = self.map(cx);
        let document = &mut self.building.document;
        document.project_name = name;
        document.phase = phase;
        document.application = application;
        document.map = map;
    }

    /// `IfcOwnerHistory` の 2 OwningApplication → `IfcApplication` の 2 Version・3 ApplicationFullName
    fn application(&mut self, history: Option<u64>, cx: &mut Cx) -> Option<Application> {
        let history = history.filter(|&id| self.is(id, b"IFCOWNERHISTORY"))?;
        let history = self.take(history)?;
        let application =
            reference(arg(history, 2)).filter(|&id| self.is(id, b"IFCAPPLICATION"))?;
        let application = self.take(application)?;
        Some(Application {
            name: self.text(arg(application, 3), application, cx)?,
            version: self.text(arg(application, 2), application, cx),
        })
    }

    /// 初めの `IfcMapConversion`（IFC4 から）。2 TargetCRS → `IfcProjectedCRS` の 1 Name、3 Eastings・4 Northings・
    /// 5 OrthogonalHeight・6 XAxisAbscissa・7 XAxisOrdinate・8 Scale
    fn map(&mut self, cx: &mut Cx) -> Option<MapConversion> {
        let id = self.find(b"IFCMAPCONVERSION")?;
        let record = self.file.instance(id)?;
        let (Some(eastings), Some(northings), Some(height)) = (
            self.real(arg(record, 3)),
            self.real(arg(record, 4)),
            self.real(arg(record, 5)),
        ) else {
            self.invalid(record, 3, cx);
            return None;
        };
        self.used.insert(id);
        let crs_name =
            match reference(arg(record, 2)).filter(|&crs| self.is(crs, b"IFCPROJECTEDCRS")) {
                Some(crs) => self
                    .take(crs)
                    .and_then(|crs| self.text(arg(crs, 1), crs, cx)),
                None => None,
            };
        Some(MapConversion {
            crs_name,
            eastings,
            northings,
            height,
            x_axis_abscissa: self.real(arg(record, 6)),
            x_axis_ordinate: self.real(arg(record, 7)),
            scale: self.real(arg(record, 8)),
        })
    }

    /// 表現の文脈。書いた順。`IfcGeometricRepresentationContext` は 1 ContextIdentifier・2 ContextType・
    /// 3 CoordinateSpaceDimension・4 Precision・5 WorldCoordinateSystem・6 TrueNorth、
    /// `IfcGeometricRepresentationSubContext` は 1・2 だけを持つ（3 から 6 は導く値）
    fn contexts(&mut self, ids: &[u64], cx: &mut Cx) {
        for &id in ids {
            let sub = self.is(id, b"IFCGEOMETRICREPRESENTATIONSUBCONTEXT");
            if !sub && !self.is(id, b"IFCGEOMETRICREPRESENTATIONCONTEXT") {
                continue;
            }
            let Some(record) = self.take(id) else {
                continue;
            };
            let mut context = Context {
                identifier: self.text(arg(record, 1), record, cx),
                kind: self.text(arg(record, 2), record, cx),
                dimension: None,
                precision: None,
            };
            if !sub {
                context.dimension = self
                    .real(arg(record, 3))
                    .filter(|d| d.fract() == 0.0 && (0.0..=3.0).contains(d))
                    .map(|d| d as u32);
                context.precision = self.real(arg(record, 4));
                if let Some(system) = reference(arg(record, 5)) {
                    // 世界座標系は、配置と同じく読んで、使ったことにする
                    let _ = self.axes(system);
                }
                if let Some(north) = reference(arg(record, 6)) {
                    let ratios = self.direction(north);
                    if self.building.document.true_north.is_none() {
                        match ratios.as_deref() {
                            Some([x, y, ..]) => {
                                self.building.document.true_north = Some([*x, *y]);
                            }
                            _ => self.invalid(record, 6, cx),
                        }
                    }
                }
            }
            self.building.document.contexts.push(context);
        }
    }

    // 単位

    /// 単位のインスタンスを読む。`IfcSIUnit`（1 Dimensions・2 UnitType・3 Prefix・4 Name）、
    /// `IfcNamedUnit` と `IfcSIUnit` の複合のインスタンス、`IfcConversionBasedUnit`（2 UnitType・3 Name・
    /// 4 ConversionFactor → `IfcMeasureWithUnit` の 1 ValueComponent・2 UnitComponent）を読む
    fn unit(&mut self, id: u64, cx: &mut Cx) -> Option<Unit> {
        if let Some(unit) = self.units.get(&id) {
            return unit.clone();
        }
        let unit = self.read_unit(id, cx);
        if unit.is_some() {
            self.used.insert(id);
        }
        self.units.insert(id, unit.clone());
        unit
    }

    fn read_unit(&mut self, id: u64, cx: &mut Cx) -> Option<Unit> {
        let record = self.file.instance(id)?;
        if let Some(unit) = self.si_unit(record) {
            return Some(unit);
        }
        if !self.is(id, b"IFCCONVERSIONBASEDUNIT") {
            return None;
        }
        let kind = self.enumeration(arg(record, 2))?;
        let name = self.text(arg(record, 3), record, cx)?;
        let factor = reference(arg(record, 4)).filter(|&f| self.is(f, b"IFCMEASUREWITHUNIT"))?;
        let factor_record = self.file.instance(factor)?;
        let value = self.real(arg(factor_record, 1))?;
        let base = self.si_unit(self.file.instance(reference(arg(factor_record, 2))?)?)?;
        self.used.insert(factor);
        if let Some(base) = reference(arg(factor_record, 2)) {
            self.used.insert(base);
        }
        Some(Unit {
            kind,
            prefix: None,
            name,
            scale: Some(value * prefix_scale(&base)),
        })
    }

    /// SI の単位。単独と、`IfcNamedUnit` との複合のインスタンスのどちらも読む
    fn si_unit(&self, record: &Record) -> Option<Unit> {
        let part = |name: &[u8]| {
            record
                .parts()
                .iter()
                .find(|part| self.file.bytes(part.name()).eq_ignore_ascii_case(name))
        };
        let si = part(b"IFCSIUNIT")?;
        let (kind, prefix, name) = match si.args() {
            [_, kind, prefix, name] => (self.enumeration(Some(kind))?, prefix, name),
            [prefix, name] => {
                let named = part(b"IFCNAMEDUNIT")?;
                (self.enumeration(named.args().get(1))?, prefix, name)
            }
            _ => return None,
        };
        Some(Unit {
            kind,
            prefix: self.enumeration(Some(prefix)),
            name: self.enumeration(Some(name))?,
            scale: None,
        })
    }

    /// 文書の単位の割り当てから、種類 `kind` の単位。なければ診断する
    fn document_unit(
        &self,
        kind: &str,
        type_name: &str,
        place: &Record,
        cx: &mut Cx,
    ) -> Option<Unit> {
        let unit = self
            .building
            .document
            .units
            .iter()
            .find(|unit| unit.kind == kind)
            .cloned();
        if unit.is_none() {
            cx.report(
                Diagnostic::new(codes::IFC_UNIT_MISSING)
                    .with("type", type_name)
                    .at(at_bytes(place.span())),
            );
        }
        unit
    }

    // 要素

    /// 意味を持つ実体なら要素を作る。`IfcMaterial` と `IfcGridAxis` は、GlobalId を持たない要素にする
    fn element(&mut self, id: u64, record: &'f Record, cx: &mut Cx) -> Result<(), Error> {
        let kind = self.file.kind(record).to_ascii_uppercase();
        let entity = self.entities.get(&kind).copied();
        let resource = match kind.as_slice() {
            b"IFCMATERIAL" => Some("IfcMaterial"),
            b"IFCGRIDAXIS" => Some("IfcGridAxis"),
            _ => None,
        };
        if entity.is_none() && resource.is_none() {
            return Ok(());
        }
        cx.check(Kind::Elements, 1)
            .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
        let global_id = match entity {
            Some(_) => {
                let text = self.text(arg(record, 1), record, cx);
                if text.is_none() {
                    cx.report(
                        Diagnostic::new(codes::IFC_GLOBAL_ID_MISSING)
                            .at(at_bytes(record.span()).with_id(format!("i{id}"))),
                    );
                }
                text
            }
            None => None,
        };
        self.used.insert(id);
        let Some(element) = self.building.push(global_id.as_deref(), id, cx) else {
            return Ok(());
        };
        self.elements.insert(id, element);
        match (entity, resource) {
            (Some(entity), _) => self.object(element, record, entity, cx),
            (None, Some("IfcMaterial")) => {
                let identity = Identity {
                    name: self.text(arg(record, 1), record, cx),
                    description: match self.version {
                        Version::Ifc2x3 => None,
                        _ => self.text(arg(record, 2), record, cx),
                    },
                    ..Identity::default()
                };
                self.class(element, "IfcMaterial", None, None, "IfcMaterial");
                self.identity(element, identity);
            }
            (None, Some(_)) => {
                // `IfcGridAxis` は 1 AxisTag・2 AxisCurve・3 SameSense。曲線は形状表現と一緒に足す
                self.class(element, "IfcGridAxis", None, None, "IfcGridAxis");
                let same_sense = match self.enumeration(arg(record, 3)).as_deref() {
                    Some("T") => true,
                    Some("F") => false,
                    _ => {
                        self.invalid(record, 3, cx);
                        return Ok(());
                    }
                };
                let axis = Grid::Axis {
                    tag: self.text(arg(record, 1), record, cx),
                    same_sense,
                    curve: None,
                };
                self.building.components.grid.insert(element, axis);
            }
            (None, None) => {}
        }
        Ok(())
    }

    /// `IfcObject` と `IfcTypeObject` の成分
    fn object(&mut self, element: ElementRef, record: &'f Record, entity: &Entity, cx: &mut Cx) {
        let tag = match entity.category {
            Category::Element | Category::Type | Category::Style => {
                self.text(arg(record, 8), record, cx)
            }
            _ => None,
        };
        let long_name = match (entity.category, entity.name) {
            (Category::Project, _) => Some(6),
            (Category::Spatial, _) => Some(8),
            (_, "IfcZone") if self.version != Version::Ifc2x3 => Some(6),
            (_, "IfcDistributionSystem") => Some(6),
            (_, "IfcBuildingSystem") => Some(7),
            (_, "IfcSpaceType") if self.version != Version::Ifc2x3 => Some(11),
            _ => None,
        };
        let identity = Identity {
            name: self.text(arg(record, 3), record, cx),
            description: self.text(arg(record, 4), record, cx),
            tag,
            long_name: long_name.and_then(|at| self.text(arg(record, at), record, cx)),
        };
        self.identity(element, identity);
        let predefined = entity
            .predefined
            .get(self.version.column())
            .copied()
            .filter(|&at| at != NONE && at != ABSENT)
            .and_then(|at| self.enumeration(arg(record, usize::from(at))));
        let user_defined = match entity.category {
            Category::Type => self.text(arg(record, 9), record, cx),
            Category::Style => None,
            _ => self.text(arg(record, 5), record, cx),
        };
        self.class(element, entity.class, predefined, user_defined, entity.name);
        match entity.category {
            Category::Spatial => self.spatial(element, record, entity, cx),
            Category::Element | Category::Grid => self.owns_placement(element, record),
            Category::Type | Category::Style => self.types.push((element, record_id(record))),
            Category::Project | Category::Group => {}
        }
        if entity.category == Category::Grid {
            self.grids.push((element, record_id(record)));
        }
    }

    fn identity(&mut self, element: ElementRef, identity: Identity) {
        if identity != Identity::default() {
            self.building.components.identity.insert(element, identity);
        }
    }

    fn class(
        &mut self,
        element: ElementRef,
        name: &str,
        predefined: Option<String>,
        user_defined: Option<String>,
        original: &str,
    ) {
        let class = Class {
            name: name.to_owned(),
            predefined,
            user_defined,
            original: original.to_owned(),
        };
        self.building.components.class.insert(element, class);
    }

    /// 空間の種類と標高。`IfcSite` の 12 RefElevation、`IfcBuilding` の 10 ElevationOfRefHeight、
    /// `IfcBuildingStorey` の 10 Elevation、`IfcSpace` の 11 ElevationWithFlooring
    fn spatial(&mut self, element: ElementRef, record: &'f Record, entity: &Entity, cx: &mut Cx) {
        let (kind, elevation, ref_elevation) = match entity.name {
            "IfcSite" => (SpatialKind::Site, None, Some(12)),
            "IfcBuilding" => (SpatialKind::Building, Some(10), None),
            "IfcBuildingStorey" => (SpatialKind::Storey, Some(10), None),
            "IfcSpace" => (SpatialKind::Space, Some(11), None),
            "IfcSpatialZone" => (SpatialKind::SpatialZone, None, None),
            other => (SpatialKind::Other(other.to_owned()), None, None),
        };
        let spatial = Spatial {
            kind,
            elevation: elevation.and_then(|at| self.length(arg(record, at), record, cx)),
            ref_elevation: ref_elevation.and_then(|at| self.length(arg(record, at), record, cx)),
        };
        self.building.components.spatial.insert(element, spatial);
        self.owns_placement(element, record);
    }

    /// 6 ObjectPlacement を覚える。配置を読むのは、すべての要素を作ってから
    fn owns_placement(&mut self, element: ElementRef, record: &Record) {
        if let Some(placement) = reference(arg(record, 6)) {
            self.placements.push((element, placement));
            self.owners.entry(placement).or_insert(element);
        }
    }

    /// 格子の軸。8 UAxes・9 VAxes・10 WAxes
    fn grid(&mut self, element: ElementRef, id: u64, _cx: &mut Cx) {
        let Some(record) = self.file.instance(id) else {
            return;
        };
        let mut axes = Vec::new();
        for (at, set) in [(8, GridSet::U), (9, GridSet::V), (10, GridSet::W)] {
            for axis in refs(arg(record, at)) {
                if let Some(&axis) = self.elements.get(&axis) {
                    axes.push((set, axis));
                }
            }
        }
        self.building
            .components
            .grid
            .insert(element, Grid::Grid { axes });
    }

    // 関係

    /// 関係を、成分と出所に写す。位置は `IfcRoot` の 1 から 4 に続く
    fn relation(&mut self, id: u64, record: &'f Record, cx: &mut Cx) {
        let kind = self.file.kind(record).to_ascii_uppercase();
        let handled = match kind.as_slice() {
            // 5 RelatingObject・6 RelatedObjects
            b"IFCRELAGGREGATES" => self.parents(id, record, 5, 6, ParentKind::Aggregates, cx),
            // IFC4 から 6 RelatedObjects は順のある列
            b"IFCRELNESTS" => self.parents(id, record, 5, 6, ParentKind::Nests, cx),
            // 5 RelatedElements・6 RelatingStructure
            b"IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                self.parents(id, record, 6, 5, ParentKind::Contained, cx)
            }
            b"IFCRELREFERENCEDINSPATIALSTRUCTURE" => self.referenced(id, record, cx),
            // 5 RelatedObjects・6 RelatingType
            b"IFCRELDEFINESBYTYPE" => self.each(id, record, 6, 5, |lifter, element, target| {
                lifter
                    .building
                    .components
                    .r#type
                    .insert(element, TypeOf { target });
            }),
            // 5 RelatedObjects・6 RelatingPropertyDefinition（IFC4 からは定義の集合でもよい）
            b"IFCRELDEFINESBYPROPERTIES" => {
                let sets = refs(arg(record, 6));
                let related = self.related(record, 5);
                for (position, element) in &related {
                    self.definitions_of(*element, &sets, cx);
                    self.source(*element, id, record, Some(*position), cx);
                }
                true
            }
            // 5 RelatedObjects・6 RelatingMaterial
            b"IFCRELASSOCIATESMATERIAL" => {
                let Some(material) = reference(arg(record, 6)) else {
                    return;
                };
                let Some(material) = self.material(material, cx) else {
                    return;
                };
                for (position, element) in self.related(record, 5) {
                    self.building
                        .components
                        .material
                        .insert(element, material.clone());
                    self.source(element, id, record, Some(position), cx);
                }
                true
            }
            // 5 RelatingBuildingElement・6 RelatedOpeningElement。開口の側に持つ
            b"IFCRELVOIDSELEMENT" => self.each(id, record, 5, 6, |lifter, opening, host| {
                lifter
                    .building
                    .components
                    .opening
                    .insert(opening, Opening::Voids(host));
            }),
            // 5 RelatingOpeningElement・6 RelatedBuildingElement。建具の側に持つ
            b"IFCRELFILLSELEMENT" => self.each(id, record, 5, 6, |lifter, element, opening| {
                lifter
                    .building
                    .components
                    .opening
                    .insert(element, Opening::Fills(opening));
            }),
            // 5 RelatedObjects・6 RelatedObjectsType・7 RelatingGroup
            b"IFCRELASSIGNSTOGROUP" => self.each(id, record, 7, 5, |lifter, element, group| {
                let mut groups = lifter
                    .building
                    .components
                    .group
                    .remove(element)
                    .unwrap_or_default();
                groups.groups.push(group);
                lifter.building.components.group.insert(element, groups);
            }),
            // 5 ConnectionGeometry・6 RelatingElement・7 RelatedElement。両方の側に持つ
            b"IFCRELCONNECTSPATHELEMENTS" => {
                let pair = (
                    reference(arg(record, 6)).and_then(|e| self.elements.get(&e).copied()),
                    reference(arg(record, 7)).and_then(|e| self.elements.get(&e).copied()),
                );
                let (Some(relating), Some(related)) = pair else {
                    return;
                };
                for (element, target) in [(relating, related), (related, relating)] {
                    self.link(element, "IfcRelConnectsPathElements", target);
                    self.source(element, id, record, None, cx);
                }
                true
            }
            _ => {
                // 5 RelatingSpace・6 RelatedBuildingElement。空間の側に持つ
                let Some((_, name)) = SPACE_BOUNDARIES
                    .iter()
                    .find(|(upper, _)| upper.as_bytes() == kind.as_slice())
                else {
                    return;
                };
                self.each(id, record, 6, 5, |lifter, space, element| {
                    lifter.link(space, name, element);
                })
            }
        };
        if handled {
            self.used.insert(id);
        }
    }

    /// 位置 `related` の要素の列（または 1 つ）と、列の中の位置
    fn related(&self, record: &Record, related: usize) -> Vec<(u32, ElementRef)> {
        refs(arg(record, related))
            .into_iter()
            .enumerate()
            .filter_map(|(at, id)| {
                let element = *self.elements.get(&id)?;
                Some((u32::try_from(at).ok()?, element))
            })
            .collect()
    }

    /// 位置 `target` の 1 つの要素と、位置 `related` の要素のそれぞれに `apply` する
    fn each(
        &mut self,
        id: u64,
        record: &'f Record,
        target: usize,
        related: usize,
        mut apply: impl FnMut(&mut Self, ElementRef, ElementRef),
    ) -> bool {
        let Some(target) =
            reference(arg(record, target)).and_then(|t| self.elements.get(&t).copied())
        else {
            return false;
        };
        let list = matches!(arg(record, related), Some(Value::List(_)));
        for (position, element) in self.related(record, related) {
            apply(self, element, target);
            self.source_quiet(element, id, record, list.then_some(position));
        }
        true
    }

    /// 親の関係
    fn parents(
        &mut self,
        id: u64,
        record: &'f Record,
        target: usize,
        related: usize,
        kind: ParentKind,
        cx: &mut Cx,
    ) -> bool {
        let Some(target) =
            reference(arg(record, target)).and_then(|t| self.elements.get(&t).copied())
        else {
            return false;
        };
        let ordered = kind == ParentKind::Nests && self.version != Version::Ifc2x3;
        for (position, element) in self.related(record, related) {
            let mut parent = self
                .building
                .components
                .parent
                .remove(element)
                .unwrap_or_default();
            if parent.of.is_some() {
                cx.report(
                    Diagnostic::new(codes::IFC_PARENT_EXISTS)
                        .with("id", self.id_text(element))
                        .at(at_bytes(record.span())),
                );
            } else {
                parent.of = Some(Membership {
                    target,
                    kind,
                    order: ordered.then_some(position),
                });
            }
            self.building.components.parent.insert(element, parent);
            self.source_quiet(element, id, record, Some(position));
        }
        true
    }

    /// 5 RelatedElements・6 RelatingStructure の、参照する空間
    fn referenced(&mut self, id: u64, record: &'f Record, _cx: &mut Cx) -> bool {
        let Some(target) = reference(arg(record, 6)).and_then(|t| self.elements.get(&t).copied())
        else {
            return false;
        };
        for (position, element) in self.related(record, 5) {
            let mut parent = self
                .building
                .components
                .parent
                .remove(element)
                .unwrap_or_default();
            parent.referenced_in.push(target);
            self.building.components.parent.insert(element, parent);
            self.source_quiet(element, id, record, Some(position));
        }
        true
    }

    fn link(&mut self, element: ElementRef, kind: &str, target: ElementRef) {
        let mut connection: Connection = self
            .building
            .components
            .connection
            .remove(element)
            .unwrap_or_default();
        connection.links.push(Link {
            kind: kind.to_owned(),
            target,
        });
        self.building
            .components
            .connection
            .insert(element, connection);
    }

    fn id_text(&self, element: ElementRef) -> String {
        self.building
            .id(element)
            .map(|id| id.to_string())
            .unwrap_or_default()
    }

    /// 成分を付けた関係を、要素の出所に残す
    fn source(
        &mut self,
        element: ElementRef,
        id: u64,
        record: &Record,
        position: Option<u32>,
        cx: &mut Cx,
    ) {
        let global_id = self.text(arg(record, 1), record, cx);
        if let Some(source) = self.building.source_mut(element) {
            source.relations.push(RelationSource {
                instance: id,
                global_id,
                position,
            });
        }
    }

    /// GlobalId の文字を診断せずに読む `source`。GlobalId は ASCII なので、置き換えは起きない
    fn source_quiet(
        &mut self,
        element: ElementRef,
        id: u64,
        record: &Record,
        position: Option<u32>,
    ) {
        let global_id = match arg(record, 1) {
            Some(Value::Text { raw, .. }) => Some(decode(self.file.bytes(raw.clone())).0),
            _ => None,
        };
        if let Some(source) = self.building.source_mut(element) {
            source.relations.push(RelationSource {
                instance: id,
                global_id,
                position,
            });
        }
    }

    // プロパティと数量

    /// 定義の集合を、要素のプロパティと数量に足す
    fn definitions_of(&mut self, element: ElementRef, sets: &[u64], cx: &mut Cx) {
        for &set in sets {
            match self.definition(set, cx) {
                Some(Definition::Properties(set)) => {
                    let mut properties: Properties = self
                        .building
                        .components
                        .properties
                        .remove(element)
                        .unwrap_or_default();
                    properties.sets.push(set);
                    self.building
                        .components
                        .properties
                        .insert(element, properties);
                }
                Some(Definition::Quantities(set)) => {
                    let mut quantities: Quantities = self
                        .building
                        .components
                        .quantities
                        .remove(element)
                        .unwrap_or_default();
                    quantities.sets.push(set);
                    self.building
                        .components
                        .quantities
                        .insert(element, quantities);
                }
                None => {}
            }
        }
    }

    /// `IfcPropertySet`（3 Name・5 HasProperties）と `IfcElementQuantity`（3 Name・5 MethodOfMeasurement・
    /// 6 Quantities）
    fn definition(&mut self, id: u64, cx: &mut Cx) -> Option<Definition> {
        if let Some(definition) = self.definitions.get(&id) {
            return definition.clone();
        }
        let record = self.file.instance(id)?;
        let name = self.text(arg(record, 3), record, cx).unwrap_or_default();
        let definition = if self.is(id, b"IFCPROPERTYSET") {
            let mut properties = Vec::new();
            for property in refs(arg(record, 5)) {
                if let Some(property) = self.property(property, cx) {
                    properties.push(property);
                }
            }
            Some(Definition::Properties(PropertySet { name, properties }))
        } else if self.is(id, b"IFCELEMENTQUANTITY") {
            let method = self.text(arg(record, 5), record, cx);
            let mut quantities = Vec::new();
            for quantity in refs(arg(record, 6)) {
                if let Some(quantity) = self.quantity(quantity, cx) {
                    quantities.push(quantity);
                }
            }
            Some(Definition::Quantities(QuantitySet {
                name,
                method,
                quantities,
            }))
        } else {
            None
        };
        if definition.is_some() {
            self.used.insert(id);
        }
        self.definitions.insert(id, definition.clone());
        definition
    }

    /// `IfcPropertySingleValue`（1 Name・3 NominalValue・4 Unit）。ほかのプロパティは、まだ写さない
    fn property(&mut self, id: u64, cx: &mut Cx) -> Option<Property> {
        if !self.is(id, b"IFCPROPERTYSINGLEVALUE") {
            return None;
        }
        let record = self.take(id)?;
        let name = self.text(arg(record, 1), record, cx).unwrap_or_default();
        let value = match arg(record, 3) {
            None | Some(Value::Null) => None,
            Some(value) => {
                let typed = self.typed_value(value, record, cx);
                if typed.is_none() {
                    self.invalid(record, 3, cx);
                }
                typed
            }
        };
        let unit = match reference(arg(record, 4)) {
            Some(unit) => {
                let resolved = self.unit(unit, cx);
                if resolved.is_none() {
                    let type_name = value
                        .as_ref()
                        .and_then(|v| v.type_name.clone())
                        .unwrap_or_default();
                    cx.report(
                        Diagnostic::new(codes::IFC_UNIT_MISSING)
                            .with("type", type_name)
                            .at(at_bytes(record.span())),
                    );
                }
                resolved
            }
            None => match value.as_ref().and_then(|v| v.type_name.as_deref()) {
                Some(type_name) => self.measure_unit(type_name, record, cx),
                None => None,
            },
        };
        Some(Property { name, value, unit })
    }

    /// 値の型から、文書の単位の割り当ての単位。単位を持たない型は `None`、知らない量の型は診断する
    fn measure_unit(&self, type_name: &str, place: &Record, cx: &mut Cx) -> Option<Unit> {
        match MEASURES.iter().find(|(name, _)| *name == type_name) {
            Some((_, Some(kind))) => self.document_unit(kind, type_name, place, cx),
            Some((_, None)) => None,
            None if type_name.to_ascii_uppercase().ends_with("MEASURE") => {
                cx.report(
                    Diagnostic::new(codes::IFC_UNIT_MISSING)
                        .with("type", type_name)
                        .at(at_bytes(place.span())),
                );
                None
            }
            None => None,
        }
    }

    /// 型を付けた値。型の名前は、表にあれば仕様の書き方（`IfcLengthMeasure`）、なければ書いたまま
    fn typed_value(&mut self, value: &Value, place: &Record, cx: &mut Cx) -> Option<TypedValue> {
        match value {
            Value::Typed(part) => {
                let written = String::from_utf8_lossy(self.file.bytes(part.name())).into_owned();
                let type_name = MEASURES
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(&written))
                    .map_or(written, |(name, _)| (*name).to_owned());
                let [inner] = part.args() else {
                    return None;
                };
                let value = self.literal(&type_name, inner, place, cx)?;
                Some(TypedValue {
                    type_name: Some(type_name),
                    value,
                })
            }
            other => Some(TypedValue {
                type_name: None,
                value: self.literal("", other, place, cx)?,
            }),
        }
    }

    /// 値。列は 1 段だけ読む
    fn literal(
        &mut self,
        type_name: &str,
        value: &Value,
        place: &Record,
        cx: &mut Cx,
    ) -> Option<Literal> {
        match value {
            Value::List(values) => values
                .iter()
                .map(|v| match v {
                    Value::List(_) => None,
                    v => self.literal(type_name, v, place, cx),
                })
                .collect::<Option<Vec<_>>>()
                .map(Literal::List),
            Value::Number(range) => {
                let text = core::str::from_utf8(self.file.bytes(range.clone())).ok()?;
                if text.contains(['.', 'E', 'e']) {
                    text.parse().ok().map(Literal::Real)
                } else {
                    text.parse().ok().map(Literal::Integer)
                }
            }
            Value::Text { .. } => self.text(Some(value), place, cx).map(Literal::Text),
            Value::Enum(_) => {
                let name = self.enumeration(Some(value))?;
                match (type_name, name.as_str()) {
                    ("IfcBoolean", "T") => Some(Literal::Boolean(true)),
                    ("IfcBoolean", "F") => Some(Literal::Boolean(false)),
                    ("IfcLogical", "T") => Some(Literal::Logical(Some(true))),
                    ("IfcLogical", "F") => Some(Literal::Logical(Some(false))),
                    ("IfcLogical", "U") => Some(Literal::Logical(None)),
                    _ => Some(Literal::Enumeration(name)),
                }
            }
            Value::Ref(id) => self.elements.get(id).copied().map(Literal::Element),
            _ => None,
        }
    }

    /// `IfcQuantityLength` など（1 Name・3 Unit・4 値・5 Formula）
    fn quantity(&mut self, id: u64, cx: &mut Cx) -> Option<Quantity> {
        let record = self.file.instance(id)?;
        let kind = self.file.kind(record).to_ascii_uppercase();
        let (_, quantity_kind, unit_kind) = QUANTITIES
            .iter()
            .find(|(name, ..)| name.as_bytes() == kind.as_slice())?;
        let Some(value) = self.real(arg(record, 4)) else {
            self.invalid(record, 4, cx);
            return None;
        };
        self.used.insert(id);
        let unit = match reference(arg(record, 3)) {
            Some(unit) => self.unit(unit, cx),
            None => match unit_kind {
                Some(unit_kind) => {
                    let type_name = match quantity_kind {
                        QuantityKind::Length => "IfcLengthMeasure",
                        QuantityKind::Area => "IfcAreaMeasure",
                        QuantityKind::Volume => "IfcVolumeMeasure",
                        QuantityKind::Weight => "IfcMassMeasure",
                        QuantityKind::Time => "IfcTimeMeasure",
                        QuantityKind::Count => "IfcCountMeasure",
                    };
                    self.document_unit(unit_kind, type_name, record, cx)
                }
                None => None,
            },
        };
        Some(Quantity {
            name: self.text(arg(record, 1), record, cx).unwrap_or_default(),
            kind: *quantity_kind,
            value: Measured { value, unit },
            formula: match self.version {
                Version::Ifc2x3 => None,
                _ => self.text(arg(record, 5), record, cx),
            },
        })
    }

    // 材料

    /// `IfcMaterial`・`IfcMaterialLayerSet`（1 MaterialLayers・2 LayerSetName）・
    /// `IfcMaterialLayerSetUsage`（1 ForLayerSet・2 LayerSetDirection・4 OffsetFromReferenceLine）・
    /// `IfcMaterialList`（1 Materials）。ほかの材料の定義は、まだ写さない
    fn material(&mut self, id: u64, cx: &mut Cx) -> Option<Material> {
        if let Some(material) = self.materials.get(&id) {
            return material.clone();
        }
        let material = self.read_material(id, cx);
        if material.is_some() {
            self.used.insert(id);
        }
        self.materials.insert(id, material.clone());
        material
    }

    fn read_material(&mut self, id: u64, cx: &mut Cx) -> Option<Material> {
        let record = self.file.instance(id)?;
        let kind = self.file.kind(record).to_ascii_uppercase();
        let part = |lifter: &Self, material: u64| MaterialPart {
            material: lifter.elements.get(&material).copied(),
            name: None,
            thickness: None,
        };
        match kind.as_slice() {
            b"IFCMATERIAL" => Some(Material {
                kind: MaterialKind::Single,
                parts: vec![part(self, id)],
                direction: None,
                offset: None,
            }),
            b"IFCMATERIALLIST" => Some(Material {
                kind: MaterialKind::List,
                parts: refs(arg(record, 1))
                    .into_iter()
                    .map(|m| part(self, m))
                    .collect(),
                direction: None,
                offset: None,
            }),
            b"IFCMATERIALLAYERSET" => Some(Material {
                kind: MaterialKind::Layers,
                parts: self.layers(record, cx),
                direction: None,
                offset: None,
            }),
            b"IFCMATERIALLAYERSETUSAGE" => {
                let set =
                    reference(arg(record, 1)).filter(|&s| self.is(s, b"IFCMATERIALLAYERSET"))?;
                let set_record = self.take(set)?;
                Some(Material {
                    kind: MaterialKind::Layers,
                    parts: self.layers(set_record, cx),
                    direction: self.enumeration(arg(record, 2)),
                    offset: self.length(arg(record, 4), record, cx),
                })
            }
            _ => None,
        }
    }

    /// `IfcMaterialLayer` の 1 Material・2 LayerThickness・4 Name（IFC4 から）
    fn layers(&mut self, set: &Record, cx: &mut Cx) -> Vec<MaterialPart> {
        let mut parts = Vec::new();
        for layer in refs(arg(set, 1)) {
            if !self.is(layer, b"IFCMATERIALLAYER") {
                continue;
            }
            let Some(record) = self.take(layer) else {
                continue;
            };
            parts.push(MaterialPart {
                material: reference(arg(record, 1)).and_then(|m| self.elements.get(&m).copied()),
                name: match self.version {
                    Version::Ifc2x3 => None,
                    _ => self.text(arg(record, 4), record, cx),
                },
                thickness: self.length(arg(record, 2), record, cx),
            });
        }
        parts
    }

    // 配置

    /// 要素の配置。`IfcLocalPlacement`（1 PlacementRelTo・2 RelativePlacement）の連鎖を、親の配置を持つ要素まで
    /// たどる。どの要素も持たない中間の配置は、局所の変換に畳んで出所に残す
    fn placement(&mut self, element: ElementRef, start: u64, cx: &mut Cx) -> Result<(), Error> {
        let mut folded: Vec<u64> = Vec::new();
        let result = self.fold(element, start, &mut folded, cx);
        for _ in &folded {
            cx.leave();
        }
        let placement = match result {
            Ok(placement) => placement,
            Err(Fold::Stopped(error)) => return Err(error),
            Err(Fold::Invalid) => {
                let place = self.file.instance(start).map(Record::span);
                let mut diagnostic =
                    Diagnostic::new(codes::IFC_PLACEMENT_INVALID).with("id", self.id_text(element));
                if let Some(place) = place {
                    diagnostic = diagnostic.at(at_bytes(place));
                }
                cx.report(diagnostic);
                return Ok(());
            }
        };
        if let Local::Unsupported(kind) = &placement.local {
            cx.report(
                Diagnostic::new(codes::IFC_PLACEMENT_UNSUPPORTED)
                    .with("kind", kind.as_str())
                    .at(at_bytes(
                        self.file.instance(start).map(Record::span).unwrap_or(0..0),
                    )),
            );
        }
        if let Some(source) = self.building.source_mut(element) {
            source.folded_placements = folded;
        }
        self.building
            .components
            .placement
            .insert(element, placement);
        Ok(())
    }

    fn fold(
        &mut self,
        element: ElementRef,
        start: u64,
        folded: &mut Vec<u64>,
        cx: &mut Cx,
    ) -> Result<Placement, Fold> {
        let mut local = match self.local(start)? {
            Step::Local(transform) => transform,
            Step::Unsupported(kind) => {
                return Ok(Placement {
                    relative_to: None,
                    local: Local::Unsupported(kind.to_owned()),
                });
            }
        };
        let mut parent = self.parent_of(start);
        loop {
            let Some(at) = parent else {
                return Ok(Placement {
                    relative_to: None,
                    local: Local::Transform(local),
                });
            };
            if let Some(&owner) = self.owners.get(&at) {
                if owner == element {
                    return Err(Fold::Invalid);
                }
                return Ok(Placement {
                    relative_to: Some(owner),
                    local: Local::Transform(local),
                });
            }
            if at == start || folded.contains(&at) {
                return Err(Fold::Invalid);
            }
            cx.enter().map_err(|stop| {
                let place = self.file.instance(at).map(Record::span);
                Fold::Stopped(stopped(cx, stop, place))
            })?;
            folded.push(at);
            match self.local(at)? {
                Step::Local(transform) => local = transform.then(&local),
                Step::Unsupported(kind) => {
                    return Ok(Placement {
                        relative_to: None,
                        local: Local::Unsupported(kind.to_owned()),
                    });
                }
            }
            parent = self.parent_of(at);
        }
    }

    /// 配置の親の配置
    fn parent_of(&self, placement: u64) -> Option<u64> {
        let record = self.file.instance(placement)?;
        reference(arg(record, 1))
    }

    /// 1 つの配置の局所の変換
    fn local(&mut self, id: u64) -> Result<Step, Fold> {
        let record = self.file.instance(id).ok_or(Fold::Invalid)?;
        let kind = self.file.kind(record).to_ascii_uppercase();
        match kind.as_slice() {
            b"IFCLOCALPLACEMENT" => {
                self.used.insert(id);
                let axes = reference(arg(record, 2)).ok_or(Fold::Invalid)?;
                let transform = self.axes(axes).ok_or(Fold::Invalid)?;
                Ok(Step::Local(transform))
            }
            b"IFCGRIDPLACEMENT" => {
                self.used.insert(id);
                Ok(Step::Unsupported("IfcGridPlacement"))
            }
            b"IFCLINEARPLACEMENT" => {
                self.used.insert(id);
                Ok(Step::Unsupported("IfcLinearPlacement"))
            }
            _ => Err(Fold::Invalid),
        }
    }

    /// `IfcAxis2Placement3D`（1 Location・2 Axis・3 RefDirection）と `IfcAxis2Placement2D`（1 Location・
    /// 2 RefDirection）を、IFC の `IfcBuildAxes`・`IfcFirstProjAxis` の規則で正規直交の軸にする。軸を作れなければ
    /// `None`
    fn axes(&mut self, id: u64) -> Option<Transform> {
        let record = self.file.instance(id)?;
        let kind = self.file.kind(record).to_ascii_uppercase();
        let (location, axis, reference_direction) = match kind.as_slice() {
            b"IFCAXIS2PLACEMENT3D" => (
                reference(arg(record, 1)),
                reference(arg(record, 2)),
                reference(arg(record, 3)),
            ),
            b"IFCAXIS2PLACEMENT2D" => (reference(arg(record, 1)), None, reference(arg(record, 2))),
            _ => return None,
        };
        self.used.insert(id);
        let location = self.point(location?)?;
        let z = match axis {
            Some(axis) => normalize(pad(&self.direction(axis)?)?)?,
            None => [0.0, 0.0, 1.0],
        };
        let reference_direction = match reference_direction {
            Some(direction) => pad(&self.direction(direction)?)?,
            None if z == [1.0, 0.0, 0.0] => [0.0, 1.0, 0.0],
            None => [1.0, 0.0, 0.0],
        };
        let along = dot(reference_direction, z);
        let [rx, ry, rz] = reference_direction;
        let [zx, zy, zz] = z;
        let x = normalize([rx - along * zx, ry - along * zy, rz - along * zz])?;
        let [xx, xy, xz] = x;
        let [yx, yy, yz] = cross(z, x);
        let [tx, ty, tz] = location;
        Some(Transform([
            [xx, yx, zx, tx],
            [xy, yy, zy, ty],
            [xz, yz, zz, tz],
        ]))
    }

    /// `IfcCartesianPoint` の 1 Coordinates。2 次元なら z を 0 にする
    fn point(&mut self, id: u64) -> Option<[f64; 3]> {
        if !self.is(id, b"IFCCARTESIANPOINT") {
            return None;
        }
        let record = self.take(id)?;
        pad(&self.reals(arg(record, 1))?)
    }

    /// `IfcDirection` の 1 DirectionRatios
    fn direction(&mut self, id: u64) -> Option<Vec<f64>> {
        if !self.is(id, b"IFCDIRECTION") {
            return None;
        }
        let record = self.take(id)?;
        self.reals(arg(record, 1))
    }
}

/// 配置をたどった 1 歩
enum Step {
    Local(Transform),
    Unsupported(&'static str),
}

/// 配置をたどれなかった理由
enum Fold {
    Invalid,
    Stopped(Error),
}

/// 位置 `at`（1 から）の引数。複合のインスタンスは最初の部分
fn arg(record: &Record, at: usize) -> Option<&Value> {
    record.parts().first()?.args().get(at.checked_sub(1)?)
}

fn record_id(record: &Record) -> u64 {
    record.id().unwrap_or_default()
}

fn reference(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Ref(id) => Some(*id),
        _ => None,
    }
}

/// 参照の列か、1 つの参照
fn refs(value: Option<&Value>) -> Vec<u64> {
    match value {
        Some(Value::Ref(id)) => vec![*id],
        Some(Value::List(values)) => values.iter().filter_map(|v| reference(Some(v))).collect(),
        _ => Vec::new(),
    }
}

/// 文字列を戻す。置き換えの文字を使ったかも返す
pub(crate) fn decode(raw: &[u8]) -> (String, bool) {
    let mut invalid = false;
    let mut written = 0usize;
    let text = zumen_step::decode(raw, |rest| {
        for len in 2..=4 {
            if let Some(text) = rest.get(..len).and_then(|b| core::str::from_utf8(b).ok()) {
                if text == "\u{FFFD}" {
                    written += 1;
                }
                return (text.to_owned(), len);
            }
        }
        invalid = true;
        (char::REPLACEMENT_CHARACTER.to_string(), 1)
    });
    let replaced = text.matches(char::REPLACEMENT_CHARACTER).count() > written;
    (text, invalid || replaced)
}

/// SI の接頭辞の倍率。面積と体積の単位は、接頭辞を 2 乗・3 乗する
fn prefix_scale(unit: &Unit) -> f64 {
    let Some(prefix) = unit.prefix.as_deref() else {
        return 1.0;
    };
    let factor = PREFIXES
        .iter()
        .find(|(name, _)| *name == prefix)
        .map_or(1.0, |(_, factor)| *factor);
    match unit.name.as_str() {
        "SQUARE_METRE" => factor * factor,
        "CUBIC_METRE" => factor * factor * factor,
        _ => factor,
    }
}

fn pad(values: &[f64]) -> Option<[f64; 3]> {
    match values {
        [x, y] => Some([*x, *y, 0.0]),
        [x, y, z] => Some([*x, *y, *z]),
        _ => None,
    }
}

fn dot([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> f64 {
    ax * bx + ay * by + az * bz
}

fn cross([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> [f64; 3] {
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

/// 長さを 1 にする。長さが 0 か有限でなければ `None`
fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let length = libm::sqrt(dot(v, v));
    if !length.is_finite() || length <= f64::EPSILON {
        return None;
    }
    let [x, y, z] = v;
    Some([x / length, y / length, z / length])
}
