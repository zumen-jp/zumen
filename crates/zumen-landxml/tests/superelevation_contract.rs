//! 合成した片勾配の値・出所・診断・予算を、同じ probe でネイティブと wasm に渡す（#468）。

use zumen_base::{Budget, CancelToken, Diagnostics, Kind, Limit, Limits, Stop, Value, codes};
use zumen_civil::{CantKind, CivilModel};
use zumen_landxml::LandXml;
use zumen_sdk::{Codec, Cx, Error, Lift};

const PRIORITY: &str = r#"<Superelevation staStart="bad" staEnd="bad"><FullSuperSta>1110.25</FullSuperSta><RunoffSta>1117.75</RunoffSta><FullSuperelev>-6.25</FullSuperelev></Superelevation>"#;
const FALLBACK: &str = r#"<Superelevation staStart="1150" staEnd="1170"><FullSuperelev>2.75</FullSuperelev></Superelevation>"#;
const TEXT: &str = r#"<Superelevation staStart="1180" staEnd="1190"><FullSuperelev> -<![CDATA[6]]>&#46;25 </FullSuperelev></Superelevation>"#;
const UNSUPPORTED_PARTS: &str = r#"<Superelevation staStart="1100" staEnd="1120"><FullSuperelev>6</FullSuperelev><AdverseSE>-2</AdverseSE><Feature name="unmapped"/></Superelevation>"#;

struct Case {
    name: &'static str,
    src: String,
    expected: Vec<(&'static str, f64, f64, f64)>,
    unsupported: bool,
    invalid: bool,
}

fn case(
    name: &'static str,
    body: &str,
    expected: Vec<(&'static str, f64, f64, f64)>,
    unsupported: bool,
    invalid: bool,
) -> Case {
    Case {
        name,
        src: format!(
            r#"<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2">
<CoordinateSystem horizontalDatum="JGD2011" verticalDatum="T.P" horizontalCoordinateSystemName="9(X,Y)"/>
<Alignments><Alignment name="片勾配" staStart="1000" length="300">{body}</Alignment></Alignments></LandXML>"#
        ),
        expected,
        unsupported,
        invalid,
    }
}

fn cases() -> Vec<Case> {
    vec![
        case(
            "priority-fallback-text",
            &format!("{PRIORITY}{FALLBACK}{TEXT}"),
            vec![
                (PRIORITY, 110.25, 7.5, -6.25),
                (FALLBACK, 150.0, 20.0, 2.75),
                (TEXT, 180.0, 10.0, -6.25),
            ],
            false,
            false,
        ),
        case(
            "unsupported-parts",
            UNSUPPORTED_PARTS,
            vec![(UNSUPPORTED_PARTS, 100.0, 20.0, 6.0)],
            true,
            false,
        ),
        case(
            "duplicate-values",
            r#"<Superelevation staStart="1100" staEnd="1120"><FullSuperelev>6</FullSuperelev><FullSuperelev>4</FullSuperelev></Superelevation>"#,
            vec![],
            true,
            false,
        ),
        case(
            "slope-list-after-cant",
            &format!(
                "{PRIORITY}<Feature name=\"SlopeList\"><Property label=\"unmapped\" value=\"1\"/></Feature>"
            ),
            vec![],
            true,
            false,
        ),
        case(
            "slope-list-config",
            &format!(
                "{PRIORITY}<Feature name=\"SuperelevationConfig\"><Property label=\"useSlopeList\" value=\"true\"/></Feature>"
            ),
            vec![],
            true,
            false,
        ),
        case(
            "missing-stations",
            "<Superelevation><FullSuperelev>6</FullSuperelev></Superelevation>",
            vec![],
            true,
            false,
        ),
        case(
            "zero-width",
            r#"<Superelevation staStart="1100" staEnd="1100"><FullSuperelev>6</FullSuperelev></Superelevation>"#,
            vec![],
            true,
            false,
        ),
        case(
            "invalid-slope",
            r#"<Superelevation staStart="1100" staEnd="1120"><FullSuperelev>NaN</FullSuperelev></Superelevation>"#,
            vec![],
            false,
            true,
        ),
        case(
            "negative-width",
            r#"<Superelevation staStart="1120" staEnd="1100"><FullSuperelev>6</FullSuperelev></Superelevation>"#,
            vec![],
            false,
            true,
        ),
    ]
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    ReadAndLift,
    Lift,
}

struct Outcome {
    result: Result<CivilModel, Error>,
    diag: Diagnostics,
    budget: Budget,
}

fn run(src: &str, stage: Stage, limits: Limits, signal: Option<Stop>) -> Outcome {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    match signal {
        Some(Stop::Cancelled) => cancel.cancel(),
        Some(Stop::TimedOut) => cancel.expire(),
        _ => {}
    }
    let result = match stage {
        Stage::ReadAndLift => {
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            LandXml::read(src.as_bytes(), &mut cx).and_then(|file| file.lift(&mut cx))
        }
        Stage::Lift => {
            let mut read_budget = Budget::default();
            let mut read_diag = Diagnostics::new();
            let read_cancel = CancelToken::new();
            let file = LandXml::read(
                src.as_bytes(),
                &mut Cx::new(&mut read_budget, &mut read_diag, &read_cancel),
            );
            file.and_then(|file| file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel)))
        }
    };
    Outcome {
        result,
        diag,
        budget,
    }
}

fn snapshot(outcome: &Outcome) -> String {
    let mut out = String::new();
    match &outcome.result {
        Ok(model) => {
            out.push_str(&format!(
                "model {:?} {:?} {:?}\n",
                model.crs, model.realization, model.height_datum
            ));
            for alignment in &model.alignments {
                out.push_str(&format!(
                    "alignment {:?} {:?} {:016x} {:?}\n",
                    alignment.id,
                    alignment.name,
                    alignment.stations.start.to_bits(),
                    alignment.source
                ));
                for cant in &alignment.cant {
                    out.push_str(&format!(
                        "cant {:016x} {:016x} {:?} {:?}\n",
                        cant.start_distance.to_bits(),
                        cant.length.to_bits(),
                        cant.transition,
                        cant.source
                    ));
                    match cant.kind {
                        CantKind::Superelevation { start, end } => {
                            out.push_str(&format!(
                                "superelevation {:?} {:?}\n",
                                start.map(f64::to_bits),
                                end.map(f64::to_bits)
                            ));
                        }
                        CantKind::Rail { .. } => out.push_str("unexpected-rail\n"),
                    }
                }
            }
        }
        Err(error) => out.push_str(&format!("error {error:?}\n")),
    }
    for diagnostic in outcome.diag.iter() {
        out.push_str(&format!(
            "diagnostic {} {} {:?} {:?} {:?} {:?}\n",
            diagnostic.code().name(),
            diagnostic.count(),
            diagnostic.severity(),
            diagnostic.fidelity(),
            diagnostic.places(),
            diagnostic.spec()
        ));
        for (name, value) in diagnostic.params() {
            match value {
                Value::Float(value) => {
                    out.push_str(&format!("param {name} float {:016x}\n", value.to_bits()))
                }
                value => out.push_str(&format!("param {name} {value:?}\n")),
            }
        }
    }
    for kind in Kind::ALL {
        out.push_str(&format!(
            "budget {} {}\n",
            kind.name(),
            outcome.budget.used(kind)
        ));
    }
    out.push_str(&format!("exceeded {:?}\n", outcome.budget.exceeded()));
    out
}

fn verify_values(case: &Case, outcome: &Outcome) -> Result<(), String> {
    let model = outcome
        .result
        .as_ref()
        .map_err(|e| format!("{}: {e:?}", case.name))?;
    let [alignment] = model.alignments.as_slice() else {
        return Err(format!("{}: 線形の数が違う", case.name));
    };
    assert_eq!(alignment.cant.len(), case.expected.len(), "{}", case.name);
    for (cant, (fragment, start, length, slope)) in alignment.cant.iter().zip(&case.expected) {
        assert_eq!(cant.start_distance.to_bits(), start.to_bits());
        assert_eq!(cant.length.to_bits(), length.to_bits());
        assert_eq!(
            cant.kind,
            CantKind::Superelevation {
                start: [*slope; 2],
                end: [*slope; 2]
            }
        );
        assert_eq!(cant.transition, None);
        let offset = case
            .src
            .find(fragment)
            .ok_or_else(|| "出所の期待値がない".to_string())?;
        let bytes = cant
            .source
            .bytes()
            .ok_or_else(|| "出所のバイト範囲がない".to_string())?;
        assert_eq!(
            (bytes.start, bytes.end),
            (offset as u64, (offset + fragment.len()) as u64)
        );
    }
    assert_eq!(
        outcome
            .diag
            .iter()
            .any(|d| d.code() == codes::LANDXML_ELEMENT_UNSUPPORTED),
        case.unsupported,
        "{}",
        case.name
    );
    assert_eq!(
        outcome
            .diag
            .iter()
            .any(|d| d.code() == codes::LANDXML_VALUE_INVALID),
        case.invalid,
        "{}",
        case.name
    );
    Ok(())
}

fn minimum_depth(src: &str, stage: Stage) -> Result<u64, String> {
    for depth in 0..=64 {
        let outcome = run(
            src,
            stage,
            Limits::default().with(Kind::Depth, Limit::Finite(depth)),
            None,
        );
        match outcome.result {
            Ok(_) => return Ok(depth),
            Err(Error::Stopped(Stop::Exceeded(exceeded))) if exceeded.kind == Kind::Depth => {}
            error => return Err(format!("深さの探索中に別の結果になった: {error:?}")),
        }
    }
    Err("合成入力の深さが 64 を超えた".to_string())
}

/// 同じ検査とビット列の記録を、Rust テストと wasm 用 example で共有する。
pub fn verify_contracts() -> Result<String, String> {
    let mut out = String::new();
    for case in cases() {
        for stage in [Stage::ReadAndLift, Stage::Lift] {
            let baseline = run(&case.src, stage, Limits::default(), None);
            verify_values(&case, &baseline)?;
            let baseline_snapshot = snapshot(&baseline);
            assert_eq!(
                baseline_snapshot,
                snapshot(&run(&case.src, stage, Limits::default(), None))
            );
            out.push_str(&format!(
                "case {} {stage:?}\n{baseline_snapshot}",
                case.name
            ));
            for kind in Kind::ALL {
                // Depth の used は終わりの深さなので、成功する最小の上限を探す。
                let boundary = if kind == Kind::Depth {
                    minimum_depth(&case.src, stage)?
                } else {
                    baseline.budget.used(kind)
                };
                let exact = run(
                    &case.src,
                    stage,
                    Limits::default().with(kind, Limit::Finite(boundary)),
                    None,
                );
                assert_eq!(
                    baseline_snapshot,
                    snapshot(&exact),
                    "{} {stage:?} {kind:?}",
                    case.name
                );
                out.push_str(&format!("boundary {} {boundary}\n", kind.name()));
                if let Some(below) = boundary.checked_sub(1) {
                    let stopped = run(
                        &case.src,
                        stage,
                        Limits::default().with(kind, Limit::Finite(below)),
                        None,
                    );
                    assert!(
                        matches!(stopped.result, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == kind && e.limit == below),
                        "{} {stage:?} {kind:?}: {:?}",
                        case.name,
                        stopped.result
                    );
                    out.push_str(&snapshot(&stopped));
                }
            }
            for signal in [Stop::Cancelled, Stop::TimedOut] {
                let stopped = run(&case.src, stage, Limits::default(), Some(signal));
                assert_eq!(stopped.result, Err(Error::Stopped(signal)));
                for kind in Kind::ALL {
                    assert_eq!(stopped.budget.used(kind), 0);
                }
                out.push_str(&format!("signal {signal:?}\n{}", snapshot(&stopped)));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
#[test]
fn superelevation_values_sources_diagnostics_and_budget_are_repeatable() {
    assert_eq!(verify_contracts().unwrap(), verify_contracts().unwrap());
}

#[cfg(test)]
#[test]
fn replaced_superelevations_spend_work_in_both_document_orders() {
    const COUNT: usize = 512;
    let intervals = FALLBACK.repeat(COUNT);
    for feature in [
        r#"<Feature name="SlopeList"/>"#,
        r#"<Feature name="SuperelevationConfig"><Property label="useSlopeList" value="true"/></Feature>"#,
    ] {
        for body in [
            format!("{feature}{intervals}"),
            format!("{intervals}{feature}"),
        ] {
            let case = case("replaced-many", &body, vec![], true, false);
            let stopped = run(
                &case.src,
                Stage::Lift,
                Limits::default().with(Kind::Work, Limit::Finite(32)),
                None,
            );
            assert!(matches!(
                stopped.result,
                Err(Error::Stopped(Stop::Exceeded(exceeded)))
                    if exceeded.kind == Kind::Work
            ));
            let outcome = run(&case.src, Stage::Lift, Limits::default(), None);
            verify_values(&case, &outcome).unwrap();
            let counts: Vec<_> = outcome
                .diag
                .iter()
                .map(|diagnostic| {
                    assert_eq!(diagnostic.code(), codes::LANDXML_ELEMENT_UNSUPPORTED);
                    (diagnostic.param("element").cloned(), diagnostic.count())
                })
                .collect();
            assert_eq!(counts.len(), 2);
            assert!(counts.contains(&(Some(Value::from("Superelevation")), COUNT as u64)));
            assert!(counts.contains(&(Some(Value::from("Feature")), 1)));
        }
    }
}
