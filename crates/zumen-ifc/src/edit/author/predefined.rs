//! 要素と型の定義された種類（`PredefinedType` と IFC2X3 の `ShapeType`）の、バージョンごとの列挙の表（#395）。
//!
//! 値は、buildingSMART が公開する各バージョンの仕様の列挙型（IFC2X3 TC1 の 6.1.2・6.2.2、IFC4 ADD2 TC1 と
//! IFC4X3 ADD2 の 5.4.2・6.1.2・6.2.2 の各 `Ifc…TypeEnum`）による。スキーマは同梱せず、スキーマから生成もしない。

use crate::building::Version;

/// 同じ列挙型を使う実体の幹（`IfcWall` と `IfcWallType` なら `"IfcWall"`）と、全版共通・IFC4 以降・IFC4X3 のみの値
struct EnumRow {
    stem: &'static str,
    common: &'static [&'static str],
    from_ifc4: &'static [&'static str],
    only_ifc4x3: &'static [&'static str],
}

const ROWS: &[EnumRow] = &[
    // IfcWallTypeEnum（IFC2X3 6.1.2.9・IFC4 6.1.2.15・IFC4X3 6.1.2.15）
    EnumRow {
        stem: "IfcWall",
        common: &[
            "STANDARD",
            "POLYGONAL",
            "SHEAR",
            "ELEMENTEDWALL",
            "PLUMBINGWALL",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &["MOVABLE", "PARAPET", "PARTITIONING", "SOLIDWALL"],
        only_ifc4x3: &["RETAININGWALL", "WAVEWALL"],
    },
    // IfcSlabTypeEnum（IFC2X3 6.1.2.8・IFC4 6.1.2.13・IFC4X3 6.1.2.13）
    EnumRow {
        stem: "IfcSlab",
        common: &[
            "FLOOR",
            "ROOF",
            "LANDING",
            "BASESLAB",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &[],
        only_ifc4x3: &[
            "APPROACH_SLAB",
            "PAVING",
            "SIDEWALK",
            "TRACKSLAB",
            "WEARING",
        ],
    },
    // IfcColumnTypeEnum（IFC2X3 6.1.2.2・IFC4 6.1.2.3・IFC4X3 6.1.2.3）
    EnumRow {
        stem: "IfcColumn",
        common: &["COLUMN", "USERDEFINED", "NOTDEFINED"],
        from_ifc4: &["PILASTER"],
        only_ifc4x3: &["PIERSTEM", "PIERSTEM_SEGMENT", "STANDCOLUMN"],
    },
    // IfcBeamTypeEnum（IFC2X3 6.1.2.1・IFC4 6.1.2.1・IFC4X3 6.1.2.1）
    EnumRow {
        stem: "IfcBeam",
        common: &[
            "BEAM",
            "JOIST",
            "LINTEL",
            "T_BEAM",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &["HOLLOWCORE", "SPANDREL"],
        only_ifc4x3: &[
            "CORNICE",
            "DIAPHRAGM",
            "EDGEBEAM",
            "GIRDER_SEGMENT",
            "HATSTONE",
            "PIERCAP",
        ],
    },
    // IfcMemberTypeEnum（IFC2X3 6.1.2.5・IFC4 6.1.2.8・IFC4X3 6.1.2.8）
    EnumRow {
        stem: "IfcMember",
        common: &[
            "BRACE",
            "CHORD",
            "COLLAR",
            "MEMBER",
            "MULLION",
            "PLATE",
            "POST",
            "PURLIN",
            "RAFTER",
            "STRINGER",
            "STRUT",
            "STUD",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &[],
        only_ifc4x3: &[
            "ARCH_SEGMENT",
            "STAY_CABLE",
            "STIFFENING_RIB",
            "STRUCTURALCABLE",
            "SUSPENDER",
            "SUSPENSION_CABLE",
            "TIEBAR",
        ],
    },
    // IfcPlateTypeEnum（IFC2X3 6.1.2.6・IFC4 6.1.2.9・IFC4X3 6.1.2.9）
    EnumRow {
        stem: "IfcPlate",
        common: &["CURTAIN_PANEL", "SHEET", "USERDEFINED", "NOTDEFINED"],
        from_ifc4: &[],
        only_ifc4x3: &[
            "BASE_PLATE",
            "COVER_PLATE",
            "FLANGE_PLATE",
            "GUSSET_PLATE",
            "SPLICE_PLATE",
            "STIFFENER_PLATE",
            "WEB_PLATE",
        ],
    },
    // IfcCoveringTypeEnum（IFC2X3 6.1.2.3・IFC4 6.1.2.4・IFC4X3 6.1.2.4）
    EnumRow {
        stem: "IfcCovering",
        common: &[
            "CEILING",
            "FLOORING",
            "CLADDING",
            "ROOFING",
            "INSULATION",
            "MEMBRANE",
            "SLEEVING",
            "WRAPPING",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &["MOLDING", "SKIRTINGBOARD"],
        only_ifc4x3: &["COPING", "TOPPING"],
    },
    // IfcRailingTypeEnum（IFC2X3 6.1.2.7・IFC4 6.1.2.10・IFC4X3 6.1.2.10）
    EnumRow {
        stem: "IfcRailing",
        common: &[
            "HANDRAIL",
            "GUARDRAIL",
            "BALUSTRADE",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &[],
        only_ifc4x3: &["FENCE"],
    },
    // IfcRoofTypeEnum（IFC2X3 6.1.2.12・IFC4 6.1.2.12・IFC4X3 6.1.2.12。IFC2X3 には USERDEFINED がない）
    EnumRow {
        stem: "IfcRoof",
        common: &[
            "FLAT_ROOF",
            "SHED_ROOF",
            "GABLE_ROOF",
            "HIP_ROOF",
            "HIPPED_GABLE_ROOF",
            "GAMBREL_ROOF",
            "MANSARD_ROOF",
            "BARREL_ROOF",
            "RAINBOW_ROOF",
            "BUTTERFLY_ROOF",
            "PAVILION_ROOF",
            "DOME_ROOF",
            "FREEFORM",
            "NOTDEFINED",
        ],
        from_ifc4: &["USERDEFINED"],
        only_ifc4x3: &[],
    },
    // IfcStairTypeEnum（IFC2X3 6.1.2.14・IFC4 6.1.2.14・IFC4X3 6.1.2.14）
    EnumRow {
        stem: "IfcStair",
        common: &[
            "STRAIGHT_RUN_STAIR",
            "TWO_STRAIGHT_RUN_STAIR",
            "QUARTER_WINDING_STAIR",
            "QUARTER_TURN_STAIR",
            "HALF_WINDING_STAIR",
            "HALF_TURN_STAIR",
            "TWO_QUARTER_WINDING_STAIR",
            "TWO_QUARTER_TURN_STAIR",
            "THREE_QUARTER_WINDING_STAIR",
            "THREE_QUARTER_TURN_STAIR",
            "SPIRAL_STAIR",
            "DOUBLE_RETURN_STAIR",
            "CURVED_RUN_STAIR",
            "TWO_CURVED_RUN_STAIR",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &[],
        only_ifc4x3: &["LADDER"],
    },
    // IfcRampTypeEnum（IFC2X3 6.1.2.11・IFC4 6.1.2.11・IFC4X3 6.1.2.11）
    EnumRow {
        stem: "IfcRamp",
        common: &[
            "STRAIGHT_RUN_RAMP",
            "TWO_STRAIGHT_RUN_RAMP",
            "QUARTER_TURN_RAMP",
            "TWO_QUARTER_TURN_RAMP",
            "HALF_TURN_RAMP",
            "SPIRAL_RAMP",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &[],
        only_ifc4x3: &[],
    },
    // IfcFootingTypeEnum（IFC2X3 6.2.2.1・IFC4 6.2.2.2・IFC4X3 6.2.2.3）
    EnumRow {
        stem: "IfcFooting",
        common: &[
            "FOOTING_BEAM",
            "PAD_FOOTING",
            "PILE_CAP",
            "STRIP_FOOTING",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &["CAISSON_FOUNDATION"],
        only_ifc4x3: &[],
    },
    // IfcPileTypeEnum（IFC2X3 6.2.2.3・IFC4 6.2.2.4・IFC4X3 6.2.2.5）
    EnumRow {
        stem: "IfcPile",
        common: &[
            "COHESION",
            "FRICTION",
            "SUPPORT",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        from_ifc4: &["BORED", "DRIVEN", "JETGROUTING"],
        only_ifc4x3: &[],
    },
    // IfcDoorTypeEnum（IFC4 6.1.2.6・IFC4X3 6.1.2.6。IFC2X3 にはない）
    EnumRow {
        stem: "IfcDoor",
        common: &[],
        from_ifc4: &["DOOR", "GATE", "TRAPDOOR", "USERDEFINED", "NOTDEFINED"],
        only_ifc4x3: &["BOOM_BARRIER", "TURNSTILE"],
    },
    // IfcWindowTypeEnum（IFC4 6.1.2.16・IFC4X3 6.1.2.16。IFC2X3 にはない）
    EnumRow {
        stem: "IfcWindow",
        common: &[],
        from_ifc4: &[
            "WINDOW",
            "SKYLIGHT",
            "LIGHTDOME",
            "USERDEFINED",
            "NOTDEFINED",
        ],
        only_ifc4x3: &[],
    },
    // IfcOpeningElementTypeEnum（IFC4 5.4.2.9・IFC4X3 5.4.2.9。IFC2X3 にはない）
    EnumRow {
        stem: "IfcOpeningElement",
        common: &[],
        from_ifc4: &["OPENING", "RECESS", "USERDEFINED", "NOTDEFINED"],
        only_ifc4x3: &[],
    },
    // IfcBuildingElementProxyTypeEnum（IFC2X3 6.1.2.10・IFC4 6.1.2.2・IFC4X3 6.1.2.2）
    EnumRow {
        stem: "IfcBuildingElementProxy",
        common: &["USERDEFINED", "NOTDEFINED"],
        from_ifc4: &[
            "COMPLEX",
            "ELEMENT",
            "PARTIAL",
            "PROVISIONFORVOID",
            "PROVISIONFORSPACE",
        ],
        only_ifc4x3: &[],
    },
];

/// 実体（要素または `…Type`）の定義された種類 `value` が、バージョン `version` の列挙にあるか
pub(in crate::edit) fn valid_predefined(class: &str, value: &str, version: Version) -> bool {
    let stem = strip_type_suffix(class);
    let Some(row) = ROWS.iter().find(|r| r.stem.eq_ignore_ascii_case(stem)) else {
        return false;
    };
    let contains = |list: &[&str]| list.iter().any(|v| v.eq_ignore_ascii_case(value));
    contains(row.common)
        || (version != Version::Ifc2x3 && contains(row.from_ifc4))
        || (version == Version::Ifc4x3 && contains(row.only_ifc4x3))
}

fn strip_type_suffix(class: &str) -> &str {
    match class.len().checked_sub(4) {
        Some(at)
            if class
                .get(at..)
                .is_some_and(|s| s.eq_ignore_ascii_case("Type")) =>
        {
            class.get(..at).unwrap_or(class)
        }
        _ => class,
    }
}
