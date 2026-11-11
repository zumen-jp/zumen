//! SI の元値、共有 cache、診断・停止位置。入力は自作と再配布済みの公開サンプルだけ。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, Hash, Unit};
use zumen_ifc::{Ifc, IfcFile};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
const PREFIXES: [&str; 17] = [
    "$", ".EXA.", ".PETA.", ".TERA.", ".GIGA.", ".MEGA.", ".KILO.", ".HECTO.", ".DECA.", ".DECI.",
    ".CENTI.", ".MILLI.", ".MICRO.", ".NANO.", ".PICO.", ".FEMTO.", ".ATTO.",
];
// 単位名の列挙すべて。次元の等しい別名も後ろに含める。
const PAIRS: [(&str, &str); 35] = [
    ("LENGTHUNIT", "METRE"),
    ("AREAUNIT", "SQUARE_METRE"),
    ("VOLUMEUNIT", "CUBIC_METRE"),
    ("MASSUNIT", "GRAM"),
    ("TIMEUNIT", "SECOND"),
    ("ELECTRICCURRENTUNIT", "AMPERE"),
    ("THERMODYNAMICTEMPERATUREUNIT", "KELVIN"),
    ("AMOUNTOFSUBSTANCEUNIT", "MOLE"),
    ("LUMINOUSINTENSITYUNIT", "CANDELA"),
    ("PLANEANGLEUNIT", "RADIAN"),
    ("SOLIDANGLEUNIT", "STERADIAN"),
    ("FREQUENCYUNIT", "HERTZ"),
    ("FORCEUNIT", "NEWTON"),
    ("PRESSUREUNIT", "PASCAL"),
    ("ENERGYUNIT", "JOULE"),
    ("POWERUNIT", "WATT"),
    ("ELECTRICCHARGEUNIT", "COULOMB"),
    ("ELECTRICVOLTAGEUNIT", "VOLT"),
    ("ELECTRICCAPACITANCEUNIT", "FARAD"),
    ("ELECTRICRESISTANCEUNIT", "OHM"),
    ("ELECTRICCONDUCTANCEUNIT", "SIEMENS"),
    ("MAGNETICFLUXUNIT", "WEBER"),
    ("MAGNETICFLUXDENSITYUNIT", "TESLA"),
    ("INDUCTANCEUNIT", "HENRY"),
    ("THERMODYNAMICTEMPERATUREUNIT", "DEGREE_CELSIUS"),
    ("LUMINOUSFLUXUNIT", "LUMEN"),
    ("ILLUMINANCEUNIT", "LUX"),
    ("RADIOACTIVITYUNIT", "BECQUEREL"),
    ("ABSORBEDDOSEUNIT", "GRAY"),
    ("DOSEEQUIVALENTUNIT", "SIEVERT"),
    ("PLANEANGLEUNIT", "STERADIAN"),
    ("FREQUENCYUNIT", "BECQUEREL"),
    ("LUMINOUSFLUXUNIT", "CANDELA"),
    ("DOSEEQUIVALENTUNIT", "GRAY"),
    ("USERDEFINED", "METRE"),
];

fn with<T>(limits: Limits, signal: u8, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics, Budget) {
    let mut b = Budget::new(limits);
    let mut d = Diagnostics::new();
    let cancel = CancelToken::new();
    if signal == 1 {
        cancel.cancel();
    } else if signal == 2 {
        cancel.expire();
    }
    let r = f(&mut Cx::new(&mut b, &mut d, &cancel));
    (r, d, b)
}
fn read(src: &str) -> IfcFile {
    with(Limits::default(), 0, |cx| Ifc::read(src.as_bytes(), cx))
        .0
        .unwrap()
}
fn lift(
    file: &IfcFile,
    limits: Limits,
    signal: u8,
) -> (Result<Building, Error>, Diagnostics, Budget) {
    with(limits, signal, |cx| file.lift(cx))
}
fn source(version: &str, si: &str, conversion: bool) -> String {
    let (unit, extras) = if conversion {
        (
            "IFCCONVERSIONBASEDUNIT(#6,.LENGTHUNIT.,'converted',#4)".to_owned(),
            format!(
                "#4=IFCMEASUREWITHUNIT(IFCLENGTHMEASURE(0.5),#5);\n#5={si};\n#6=IFCDIMENSIONALEXPONENTS(1,0,0,0,0,0,0);"
            ),
        )
    } else {
        (si.to_owned(), String::new())
    };
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('units'),'2;1');\nFILE_NAME('units','',(),(),'','','');\nFILE_SCHEMA(('{version}'));\nENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
    #2=IFCUNITASSIGNMENT((#3));\n#3={unit};\n{extras}\n\
    #10=IFCWALL('0000000000000000000010',$,$,$,$,$,$,${later});\n\
    #11=IFCPROPERTYSET('0000000000000000000011',$,'Units',$,(#12,#13));\n\
    #12=IFCPROPERTYSINGLEVALUE('A',$,IFCLENGTHMEASURE(2.),#3);\n\
    #13=IFCPROPERTYSINGLEVALUE('B',$,IFCLENGTHMEASURE(3.),#3);\n\
    #14=IFCELEMENTQUANTITY('0000000000000000000014',$,'Q',$,$,(#15));\n\
    #15=IFCQUANTITYLENGTH('L',$,#3,4.{quantity});\n\
    #16=IFCRELDEFINESBYPROPERTIES('0000000000000000000016',$,$,$,(#10),#11);\n\
    #17=IFCRELDEFINESBYPROPERTIES('0000000000000000000017',$,$,$,(#10),#14);\n\
    ENDSEC;\nEND-ISO-10303-21;\n",
        later = if version == "IFC2X3" {
            ""
        } else {
            ",.NOTDEFINED."
        },
        quantity = if version == "IFC2X3" { "" } else { ",$" }
    )
}
fn si(kind: &str, prefix: &str, name: &str, layout: u8) -> String {
    match layout {
        1 => format!("(IFCNAMEDUNIT(*,.{kind}.)IFCSIUNIT({prefix},.{name}.))"),
        2 => format!("(IFCSIUNIT({prefix},.{name}.)IFCNAMEDUNIT(*,.{kind}.))"),
        _ => format!("IFCSIUNIT(*,.{kind}.,{prefix},.{name}.)"),
    }
}
fn invalids() -> Vec<(&'static str, &'static str)> {
    vec![
        ("IFCSIUNIT(*,.LENGTHUNIT.,.UNKNOWN.,.METRE.)", "prefix"),
        ("IFCSIUNIT(*,.LENGTHUNIT.,3.,.METRE.)", "prefix"),
        ("IFCSIUNIT(*,.LENGTHUNIT.,*,.METRE.)", "prefix"),
        ("IFCSIUNIT(*,.LENGTHUNIT.,'MILLI',.METRE.)", "prefix"),
        ("IFCSIUNIT(*,.LENGTHUNIT.,.milli.,.METRE.)", "prefix"),
        (
            "IFCSIUNIT(*,.LENGTHUNIT.,$,.SQUARE_METRE.)",
            "unit-type-dimensions",
        ),
        (
            "IFCSIUNIT(*,.AREAUNIT.,$,.CUBIC_METRE.)",
            "unit-type-dimensions",
        ),
        ("IFCSIUNIT(*,.MASSUNIT.,$,.SECOND.)", "unit-type-dimensions"),
        ("IFCSIUNIT(*,.UNKNOWN.,$,.METRE.)", "unit-type"),
        ("IFCSIUNIT(*,$,$,.METRE.)", "unit-type"),
        ("IFCSIUNIT(*,4,$,.METRE.)", "unit-type"),
        ("IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.UNKNOWN.)", "name"),
        ("IFCSIUNIT(*,.LENGTHUNIT.,$,2)", "name"),
        ("IFCSIUNIT(*,.LENGTHUNIT.,$,$)", "name"),
        ("IFCSIUNIT($,.LENGTHUNIT.,$,.METRE.)", "dimensions"),
        ("IFCSIUNIT(#99,.LENGTHUNIT.,$,.METRE.)", "dimensions"),
        ("IFCSIUNIT(*,.LENGTHUNIT.,$)", "layout"),
        (
            "(IFCNAMEDUNIT($,.LENGTHUNIT.)IFCSIUNIT($,.METRE.))",
            "dimensions",
        ),
        (
            "(IFCNAMEDUNIT(*,.AREAUNIT.)IFCSIUNIT($,.METRE.))",
            "unit-type-dimensions",
        ),
        (
            "(IFCNAMEDUNIT(*,.LENGTHUNIT.)IFCSIUNIT(7,.METRE.))",
            "prefix",
        ),
        (
            "(IFCNAMEDUNIT(*,.LENGTHUNIT.,$)IFCSIUNIT($,.METRE.))",
            "layout",
        ),
        (
            "(IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.)IFCNAMEDUNIT(*,.LENGTHUNIT.))",
            "layout",
        ),
        ("(IFCSIUNIT($,.METRE.)IFCSIUNIT($,.METRE.))", "layout"),
        (
            "(IFCUNKNOWN()IFCNAMEDUNIT(*,.LENGTHUNIT.)IFCSIUNIT($,.METRE.))",
            "layout",
        ),
        ("IFCSIUNIT(*,.lengthunit.,$,.METRE.)", "unit-type"),
        ("IFCSIUNIT(*,.LENGTHUNIT.,$,.Metre.)", "name"),
        ("IFCSIUNIT(*,.LENGTHUNIT.,.Milli.,.METRE.)", "prefix"),
    ]
}

/// SI と換算単位を混ぜた非正規の配置。SI の位置・全属性/ローカル属性を変える。
fn mixed_units() -> Vec<String> {
    let conversion = "IFCCONVERSIONBASEDUNIT(#6,.LENGTHUNIT.,'converted',#4)";
    let named = "IFCNAMEDUNIT(*,.LENGTHUNIT.)";
    let mut out = Vec::new();
    for si in [
        "IFCSIUNIT($,.METRE.)",
        "IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.)",
    ] {
        out.push(format!("({conversion}{si})"));
        out.push(format!("({si}{conversion})"));
        for parts in [
            [conversion, named, si],
            [conversion, si, named],
            [named, conversion, si],
            [named, si, conversion],
            [si, conversion, named],
            [si, named, conversion],
        ] {
            out.push(format!("({})", parts.concat()));
        }
    }
    out
}
fn mixed_source(version: &str, mixed: &str, base: bool) -> String {
    if base {
        source(version, mixed, true)
    } else {
        source(version, &si("LENGTHUNIT", "$", "METRE", 0), true).replace(
            "#3=IFCCONVERSIONBASEDUNIT(#6,.LENGTHUNIT.,'converted',#4);",
            &format!("#3={mixed};"),
        )
    }
}

#[test]
fn rejects_si_conversion_mixtures_without_falling_back_or_caching_a_unit() {
    for version in VERSIONS {
        for mixed in mixed_units() {
            for base in [false, true] {
                let file = read(&mixed_source(version, &mixed, base));
                let (r, d, _) = with(Limits::default(), 0, |cx| {
                    cx.with_diagnostics_in_record_order(|cx| file.lift(cx))
                });
                let b: Building = r.unwrap();
                assert!(
                    b.document.units.is_empty(),
                    "{version}/{mixed}/base={base}: {:?}",
                    b.document.units
                );
                let invalid: Vec<_> = d
                    .iter()
                    .filter(|x| x.code() == codes::IFC_UNIT_INVALID)
                    .collect();
                assert_eq!(invalid.len(), 1, "{d:?}");
                assert_eq!(invalid[0].count(), 1, "{d:?}");
                assert_eq!(
                    invalid[0].param("reason"),
                    Some(&zumen_base::Value::Text("layout".into()))
                );
                let span = file.instance(if base { 5 } else { 3 }).unwrap().span();
                assert_eq!(
                    invalid[0].places(),
                    [zumen_base::Ref::at_bytes(
                        span.start as u64,
                        span.end as u64
                    )]
                );
                let wall = b
                    .find(
                        b.elements()
                            .iter()
                            .find(|e| e.source().instance == Some(10))
                            .unwrap()
                            .id(),
                    )
                    .unwrap();
                let properties = &b.components.properties.get(wall).unwrap().sets[0].properties;
                assert_eq!(properties.len(), 2);
                assert!(
                    properties
                        .iter()
                        .all(|p| p.unit.is_none() && p.value.is_some())
                );
                assert!(
                    b.components.quantities.get(wall).unwrap().sets[0].quantities[0]
                        .value
                        .unit
                        .is_none()
                );
                let missing = d
                    .iter()
                    .find(|x| x.code() == codes::IFC_UNIT_MISSING)
                    .unwrap();
                assert_eq!(missing.count(), 2, "{d:?}");
                let places: Vec<_> = [12, 13]
                    .map(|id| {
                        let s = file.instance(id).unwrap().span();
                        zumen_base::Ref::at_bytes(s.start as u64, s.end as u64)
                    })
                    .into();
                assert_eq!(missing.places(), places, "{d:?}");
            }
        }
    }
}

#[test]
fn mixed_si_stops_before_rejection_and_keeps_its_span() {
    for version in VERSIONS {
        for mixed in mixed_units() {
            for base in [false, true] {
                let file = read(&mixed_source(version, &mixed, base));
                let id = if base { 5 } else { 3 };
                let span = file.instance(id).unwrap().span();
                for (kind, n) in [(Kind::Work, 0), (Kind::DecodedBytes, 511)] {
                    let (r, d, _) = lift(&file, Limits::default().with(kind, Limit::Finite(n)), 0);
                    assert!(matches!(r, Err(Error::Stopped(_))));
                    assert!(
                        !d.iter().any(|x| x.code() == codes::IFC_UNIT_INVALID
                            || x.code() == codes::IFC_UNIT_MISSING),
                        "{d:?}"
                    );
                    assert_eq!(
                        d.iter().next().unwrap().places(),
                        [zumen_base::Ref::at_bytes(
                            span.start as u64,
                            span.end as u64
                        )],
                        "{d:?}"
                    );
                }
                for signal in [1, 2] {
                    let (r, d, b) = lift(&file, Limits::default(), signal);
                    assert!(matches!(r, Err(Error::Stopped(_))));
                    assert_eq!(b.used(Kind::Work), 0);
                    assert!(
                        !d.iter().any(|x| x.code() == codes::IFC_UNIT_INVALID
                            || x.code() == codes::IFC_UNIT_MISSING),
                        "{d:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn an_invalid_mixed_base_cannot_be_reused_as_a_conversion_unit() {
    for version in VERSIONS {
        let mixed = "(IFCCONVERSIONBASEDUNIT(#6,.LENGTHUNIT.,'cached',#8)IFCSIUNIT($,.METRE.))";
        let src=mixed_source(version,mixed,true)
            .replace("IFCLENGTHMEASURE(3.),#3","IFCLENGTHMEASURE(3.),#5")
            .replace("#10=","#8=IFCMEASUREWITHUNIT(IFCLENGTHMEASURE(0.25),#9);\n#9=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n#10=");
        let file = read(&src);
        // 文書単位の基として #5 を拒否した後、別の値が #5 自体を初めて単位として読む。
        // #5 内の換算部分だけなら、有効な別の基 #9 から倍率を取得できてしまう。
        let (r, d, _) = lift(&file, Limits::default(), 0);
        let b = r.unwrap();
        assert!(b.document.units.is_empty());
        let wall = b
            .find(
                b.elements()
                    .iter()
                    .find(|e| e.source().instance == Some(10))
                    .unwrap()
                    .id(),
            )
            .unwrap();
        assert!(
            b.components.properties.get(wall).unwrap().sets[0]
                .properties
                .iter()
                .all(|p| p.unit.is_none())
        );
        let invalid = d
            .iter()
            .find(|x| x.code() == codes::IFC_UNIT_INVALID)
            .unwrap();
        assert_eq!(invalid.count(), 1, "{d:?}");
        let span = file.instance(5).unwrap().span();
        assert_eq!(
            invalid.places(),
            [zumen_base::Ref::at_bytes(
                span.start as u64,
                span.end as u64
            )]
        );
        assert_eq!(
            d.iter()
                .find(|x| x.code() == codes::IFC_UNIT_MISSING)
                .unwrap()
                .count(),
            2
        );
    }
}

#[test]
fn retains_all_names_prefixes_dimensions_and_raw_values_in_three_versions() {
    for version in VERSIONS {
        for (kind, name) in PAIRS {
            for prefix in PREFIXES {
                for layout in 0..3 {
                    let file = read(&source(version, &si(kind, prefix, name, layout), false));
                    let (r, d, _) = lift(&file, Limits::default(), 0);
                    let b = r.unwrap();
                    assert!(
                        !d.iter().any(|x| x.code() == codes::IFC_UNIT_INVALID),
                        "{version}/{kind}/{name}/{prefix}/{layout}: {d:?}"
                    );
                    assert_eq!(
                        b.document.units,
                        [Unit {
                            kind: kind.into(),
                            prefix: if prefix == "$" {
                                None
                            } else {
                                Some(prefix.trim_matches('.').into())
                            },
                            name: name.into(),
                            scale: None
                        }]
                    );
                }
            }
        }
        // 一段換算の基も全接頭辞・複合配置を経由する。最初に読んだ値と成功 cache の写しが一致する。
        for prefix in PREFIXES {
            for layout in 0..3 {
                for base_kind in ["LENGTHUNIT", "USERDEFINED"] {
                    let file = read(&source(
                        version,
                        &si(base_kind, prefix, "METRE", layout),
                        true,
                    ));
                    let (r, d, _) = lift(&file, Limits::default(), 0);
                    let b = r.unwrap();
                    assert!(
                        !d.iter().any(|x| x.code() == codes::IFC_UNIT_INVALID
                            || x.code() == codes::IFC_UNIT_MISSING),
                        "{d:?}"
                    );
                    let wall = b
                        .find(
                            b.elements()
                                .iter()
                                .find(|e| e.source().instance == Some(10))
                                .unwrap()
                                .id(),
                        )
                        .unwrap();
                    let unit = &b.document.units[0];
                    for p in &b.components.properties.get(wall).unwrap().sets[0].properties {
                        assert_eq!(p.unit.as_ref(), Some(unit));
                    }
                    assert_eq!(
                        b.components.quantities.get(wall).unwrap().sets[0].quantities[0]
                            .value
                            .unit
                            .as_ref(),
                        Some(unit)
                    );
                }
            }
        }
    }
}

#[test]
fn keeps_enum_spelling_in_l0_and_does_not_normalize_invalid_si_values() {
    for version in VERSIONS {
        for token in ["milli", "Milli", "MILLI"] {
            let src = source(
                version,
                &si("LENGTHUNIT", &format!(".{token}."), "METRE", 0),
                false,
            );
            let file = read(&src);
            let zumen_ifc::Value::Enum(raw) = &file.instance(3).unwrap().parts()[0].args()[2]
            else {
                panic!("enum");
            };
            assert_eq!(file.bytes(raw.clone()), token.as_bytes());
            let (output, _, _) = with(Limits::default(), 0, |cx| {
                let mut out = Vec::new();
                Ifc::write(&file, &mut out, cx).map(|()| out)
            });
            assert_eq!(output.unwrap(), src.as_bytes());
            let (r, d, _) = lift(&file, Limits::default(), 0);
            let b = r.unwrap();
            if token == "MILLI" {
                assert_eq!(b.document.units[0].prefix.as_deref(), Some(token));
            } else {
                assert!(b.document.units.is_empty());
                assert_eq!(
                    d.iter()
                        .find(|x| x.code() == codes::IFC_UNIT_INVALID)
                        .unwrap()
                        .count(),
                    1
                );
            }
        }
    }
}

#[test]
fn records_each_bad_si_once_and_orders_its_sources_with_the_callers_record_context() {
    for version in VERSIONS {
        let src = source(version, invalids()[0].0, false)
            .replace("((#3))", "((#7,#3,#7,#3))")
            .replace(
                "#10=",
                "#7=IFCSIUNIT(*,.LENGTHUNIT.,.UNKNOWN.,.METRE.);\n#10=",
            );
        let file = read(&src);
        let (r, d, _) = with(Limits::default(), 0, |cx| {
            cx.with_diagnostics_in_record_order(|cx| file.lift(cx))
        });
        let _: Building = r.unwrap();
        let invalid = d
            .iter()
            .find(|x| x.code() == codes::IFC_UNIT_INVALID)
            .unwrap();
        assert_eq!(invalid.count(), 2);
        let places: Vec<_> = [3, 7]
            .map(|id| {
                let span = file.instance(id).unwrap().span();
                zumen_base::Ref::at_bytes(span.start as u64, span.end as u64)
            })
            .into();
        assert_eq!(invalid.places(), places, "{d:?}");
        // 派生値の代わりに自分を参照しても、その参照をたどらず拒否する。
        let file = read(&source(
            version,
            "IFCSIUNIT(#3,.LENGTHUNIT.,$,.METRE.)",
            false,
        ));
        let (r, d, _) = lift(&file, Limits::default(), 0);
        assert!(r.unwrap().document.units.is_empty());
        assert_eq!(
            d.iter()
                .find(|x| x.code() == codes::IFC_UNIT_INVALID)
                .unwrap()
                .count(),
            1
        );
    }
}

#[test]
fn rejects_invalid_si_once_at_the_original_unit_and_keeps_property_values() {
    for version in VERSIONS {
        for (bad, reason) in invalids() {
            for conversion in [false, true] {
                let file = read(&source(version, bad, conversion));
                let (r, d, _) = lift(&file, Limits::default(), 0);
                let b = r.unwrap();
                assert!(b.document.units.is_empty(), "{version}/{bad}: {b:?}");
                let bad_record = file.instance(if conversion { 5 } else { 3 }).unwrap();
                let invalid: Vec<_> = d
                    .iter()
                    .filter(|x| x.code() == codes::IFC_UNIT_INVALID)
                    .collect();
                assert_eq!(invalid.len(), 1, "{version}/{bad}: {d:?}");
                assert_eq!(invalid[0].count(), 1, "{d:?}");
                assert_eq!(
                    invalid[0].param("reason"),
                    Some(&zumen_base::Value::Text(reason.into()))
                );
                assert_eq!(
                    invalid[0].places(),
                    [zumen_base::Ref::at_bytes(
                        bad_record.span().start as u64,
                        bad_record.span().end as u64
                    )]
                );
                let wall = b
                    .find(
                        b.elements()
                            .iter()
                            .find(|e| e.source().instance == Some(10))
                            .unwrap()
                            .id(),
                    )
                    .unwrap();
                let properties = &b.components.properties.get(wall).unwrap().sets[0].properties;
                assert_eq!(properties.len(), 2);
                assert!(
                    properties
                        .iter()
                        .all(|p| p.unit.is_none() && p.value.is_some())
                );
                assert_eq!(
                    b.components.quantities.get(wall).unwrap().sets[0]
                        .quantities
                        .len(),
                    1
                );
            }
        }
    }
}

#[test]
fn stops_before_invalid_cache_or_multi_part_scan_without_missing_diagnostics() {
    for version in VERSIONS {
        for (bad, _) in [invalids()[0], invalids()[23]] {
            let file = read(&source(version, bad, false));
            for (kind, n) in [(Kind::DecodedBytes, 511), (Kind::Work, 0), (Kind::Work, 2)] {
                if kind == Kind::Work && n == 2 && bad == invalids()[0].0 {
                    continue;
                }
                let (r, d, _) = lift(&file, Limits::default().with(kind, Limit::Finite(n)), 0);
                assert!(matches!(r, Err(Error::Stopped(_))), "{r:?}");
                assert!(
                    !d.iter().any(|x| x.code() == codes::IFC_UNIT_MISSING
                        || x.code() == codes::IFC_UNIT_INVALID),
                    "{d:?}"
                );
                assert_eq!(
                    d.iter().next().unwrap().places(),
                    [zumen_base::Ref::at_bytes(
                        file.instance(3).unwrap().span().start as u64,
                        file.instance(3).unwrap().span().end as u64
                    )],
                    "{d:?}"
                );
            }
            for signal in [1, 2] {
                let (r, d, b) = lift(&file, Limits::default(), signal);
                assert!(matches!(r, Err(Error::Stopped(_))));
                assert_eq!(b.used(Kind::Work), 0);
                assert!(
                    !d.iter().any(|x| x.code() == codes::IFC_UNIT_INVALID
                        || x.code() == codes::IFC_UNIT_MISSING),
                    "{d:?}"
                );
            }
        }
    }
}

#[test]
fn shares_invalid_conversion_bases_and_stops_before_a_success_cache_copy() {
    for version in VERSIONS {
        let src = source(version, invalids()[0].0, true)
            .replace("((#3))", "((#3,#7))")
            .replace(
                "#10=",
                "#7=IFCCONVERSIONBASEDUNIT(#6,.LENGTHUNIT.,'other',#4);\n#10=",
            )
            .replace("IFCLENGTHMEASURE(3.),#3", "IFCLENGTHMEASURE(3.),#7");
        let (r, d, _) = lift(&read(&src), Limits::default(), 0);
        assert!(r.unwrap().document.units.is_empty());
        assert_eq!(
            d.iter()
                .find(|x| x.code() == codes::IFC_UNIT_INVALID)
                .unwrap()
                .count(),
            1,
            "{d:?}"
        );
        let good = source(version, &si("LENGTHUNIT", ".MILLI.", "METRE", 0), false)
            .replace("((#3))", "((#3,#3))");
        let file = read(&good);
        let (r, d, b) = lift(&file, Limits::default(), 0);
        let building = r.unwrap();
        assert_eq!(building.document.units[0], building.document.units[1]);
        assert!(!d.iter().any(|x| x.code() == codes::IFC_UNIT_MISSING));
        let (r, d, _) = lift(
            &file,
            Limits::default().with(Kind::DecodedBytes, Limit::Finite(1)),
            0,
        );
        assert!(matches!(r, Err(Error::Stopped(_))));
        assert!(!d.iter().any(|x|x.code()==codes::IFC_UNIT_INVALID || x.code()==codes::IFC_UNIT_MISSING),"{d:?}");
        assert!(b.used(Kind::DecodedBytes) > 1);
    }
}

#[test]
fn legacy_curve_and_extruded_area_reject_invalid_units_without_changing_geometry() {
    let base = include_str!("../../../samples/ifc/gni/model_9.ifc");
    // 形の定義は公開サンプル。単位の不正値だけ自作する。
    {
        let src = base.to_owned();
        let file = read(&src);
        let shape = with(Limits::default(), 0, |cx| file.product_shape(131, cx))
            .0
            .unwrap();
        let leaf = shape
            .nodes
            .iter()
            .find(|n| n.source.id() == Some(114))
            .unwrap();
        let (before, _, _) = with(Limits::default(), 0, |cx| file.extruded_area(leaf, cx));
        let before = before.unwrap();
        // この公開サンプルの長さ SI は #19。未知 Prefix を明示したときに黙って倍率1へ戻さない。
        let id = 19;
        let record = file.instance(id).unwrap();
        let mut bad = src.clone();
        bad.replace_range(
            record.span(),
            &format!("#{id}=IFCSIUNIT(*,.LENGTHUNIT.,.UNKNOWN.,.METRE.);"),
        );
        let invalid = read(&bad);
        let shape = with(Limits::default(), 0, |cx| invalid.product_shape(131, cx))
            .0
            .unwrap();
        let leaf = shape
            .nodes
            .iter()
            .find(|n| n.source.id() == Some(114))
            .unwrap();
        let (after, d, _) = with(Limits::default(), 0, |cx| invalid.extruded_area(leaf, cx));
        let after = after.unwrap();
        assert_eq!(before.definition, after.definition);
        assert!(after.boundary.length_unit.is_none());
        assert_eq!(
            d.iter()
                .find(|x| x.code() == codes::IFC_UNIT_INVALID)
                .unwrap()
                .count(),
            1,
            "{d:?}"
        );
        assert!(d.iter().any(|x| x.code() == codes::IFC_UNIT_MISSING));
        let (_, d, _) = with(Limits::default(), 0, |cx| invalid.curve(111, cx));
        assert!(d.iter().any(|x| x.code() == codes::IFC_UNIT_INVALID));
    }
}

fn write_unit(out: &mut Writer, u: &Unit) {
    out.begin_object();
    out.key("kind");
    out.string(&u.kind);
    out.key("prefix");
    if let Some(p) = &u.prefix {
        out.string(p)
    } else {
        out.null()
    }
    out.key("name");
    out.string(&u.name);
    out.key("scale");
    if let Some(s) = u.scale {
        out.float(s)
    } else {
        out.null()
    }
    out.end_object();
}
fn snapshot(src: &str, limits: Limits, signal: u8) -> String {
    let file = read(src);
    let (r, d, b) = lift(&file, limits, signal);
    format!("{r:?}\n{d:?}\n{:?}\n", Kind::ALL.map(|k| b.used(k)))
}

#[test]
#[allow(clippy::disallowed_methods)] // 自作・公開サンプルの外部比較の出力先だけを受け取る
fn writes_units_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_UNITS_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut contract = String::new();
    let mut valid_contract = String::new();
    for version in VERSIONS {
        let mut out = Writer::new();
        out.begin_array();
        for (i, (kind, name)) in PAIRS.into_iter().enumerate() {
            let src = source(version, &si(kind, ".MILLI.", name, 0), false);
            let file = read(&src);
            let (r, d, b) = lift(&file, Limits::default(), 0);
            let building = r.unwrap();
            out.begin_object();
            out.key("file");
            let name = format!("{version}-{i}.ifc");
            out.string(&name);
            std::fs::write(folder.join(name), &src).unwrap();
            out.key("units");
            out.begin_array();
            for u in &building.document.units {
                write_unit(&mut out, u)
            }
            out.end_array();
            out.key("hash");
            out.string(&Hash::document_values(&building).to_string());
            out.key("diagnostics");
            out.string(&format!("{d:?}"));
            out.key("budgets");
            out.begin_array();
            for k in Kind::ALL {
                out.uint(b.used(k))
            }
            out.end_array();
            out.end_object();
            valid_contract += &snapshot(&src, Limits::default(), 0);
        }
        out.end_array();
        std::fs::write(folder.join(format!("{version}.json")), out.into_bytes()).unwrap();
        for layout in 0..3 {
            for prefix in PREFIXES {
                for base_kind in ["LENGTHUNIT", "USERDEFINED"] {
                    let src = source(version, &si(base_kind, prefix, "METRE", layout), true);
                    valid_contract += &snapshot(&src, Limits::default(), 0);
                }
                let src = source(version, &si("USERDEFINED", prefix, "METRE", layout), false);
                valid_contract += &snapshot(&src, Limits::default(), 0);
            }
        }
        for (bad, _) in invalids() {
            for conversion in [false, true] {
                let src = source(version, bad, conversion);
                contract += &snapshot(&src, Limits::default(), 0);
                let (_, _, b) = lift(&read(&src), Limits::default(), 0);
                for kind in Kind::ALL {
                    for limit in [0, b.used(kind).saturating_sub(1), b.used(kind)] {
                        contract +=
                            &snapshot(&src, Limits::default().with(kind, Limit::Finite(limit)), 0);
                    }
                }
                for signal in [1, 2] {
                    contract += &snapshot(&src, Limits::default(), signal);
                }
            }
        }
        for mixed in mixed_units() {
            for base in [false, true] {
                let src = mixed_source(version, &mixed, base);
                contract += &snapshot(&src, Limits::default(), 0);
                let (_, _, b) = lift(&read(&src), Limits::default(), 0);
                for kind in Kind::ALL {
                    for limit in [0, b.used(kind).saturating_sub(1), b.used(kind)] {
                        contract +=
                            &snapshot(&src, Limits::default().with(kind, Limit::Finite(limit)), 0);
                    }
                }
                for signal in [1, 2] {
                    contract += &snapshot(&src, Limits::default(), signal);
                }
            }
        }
    }
    let mut out = Writer::new();
    out.begin_array();
    for (name, src) in [
        (
            "model_9",
            include_str!("../../../samples/ifc/gni/model_9.ifc"),
        ),
        (
            "model_18",
            include_str!("../../../samples/ifc/gni/model_18.ifc"),
        ),
    ] {
        let file = read(src);
        let (r, _, _) = lift(&file, Limits::default(), 0);
        let building = r.unwrap();
        let name = format!("{name}.ifc");
        std::fs::write(folder.join(&name), src).unwrap();
        out.begin_object();
        out.key("file");
        out.string(&name);
        out.key("units");
        out.begin_array();
        for u in &building.document.units {
            write_unit(&mut out, u)
        }
        out.end_array();
        out.end_object();
        valid_contract += &snapshot(src, Limits::default(), 0);
    }
    out.end_array();
    std::fs::write(folder.join("public.json"), out.into_bytes()).unwrap();
    std::fs::write(folder.join("valid-contract.txt"), valid_contract).unwrap();
    std::fs::write(folder.join("contract.txt"), contract).unwrap();
}
