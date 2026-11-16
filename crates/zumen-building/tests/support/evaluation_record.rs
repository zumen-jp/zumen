//! 同じ入力・設定・予算から得る形と停止を、環境に依存しない順で記録する。

#![allow(clippy::unwrap_used, reason = "試験と照合用の実行ファイルの固定入力")]

use super::fixtures;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::evaluation::{self, Options};
use zumen_geom3d::{Kernel, ManifoldKernel, Mesh, Tolerance, cases};

fn number(out: &mut Vec<u8>, n: u64) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn real(out: &mut Vec<u8>, n: f64) {
    number(out, n.to_bits());
}
fn text(out: &mut Vec<u8>, s: &str) {
    number(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}
fn mesh(out: &mut Vec<u8>, mesh: &Mesh) {
    number(out, mesh.positions.len() as u64);
    for p in &mesh.positions {
        for &v in p {
            real(out, v);
        }
    }
    number(out, mesh.triangles.len() as u64);
    for t in &mesh.triangles {
        for v in t {
            number(out, u64::from(*v));
        }
    }
    number(out, mesh.edges.len() as u64);
    for e in &mesh.edges {
        for v in e {
            number(out, u64::from(*v));
        }
    }
}
fn observations(out: &mut Vec<u8>, budget: &Budget, diagnostics: Diagnostics) {
    for kind in Kind::ALL {
        number(out, budget.used(kind));
    }
    text(out, &format!("{diagnostics:?}"));
}

pub fn record() -> Vec<u8> {
    let mut out = b"zumen-building evaluation 1\n".to_vec();
    let cancel = CancelToken::new();
    let tol = Tolerance {
        chord: 0.001,
        length: 1e-9,
    };
    for case in cases::all() {
        text(&mut out, case.name);
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        match ManifoldKernel.evaluate(
            &case.shape,
            tol,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        ) {
            Ok(value) => {
                text(&mut out, "ok");
                mesh(&mut out, &value);
            }
            Err(error) => text(&mut out, &format!("{error:?}")),
        }
        observations(&mut out, &budget, diagnostics);
    }
    for decomposed in [false, true] {
        let mut building = fixtures::building();
        if decomposed {
            use zumen_building::{ElementRef, Membership, Opening, Parent, ParentKind};
            let parent = fixtures::with(|cx| building.push(None, 100, cx)).unwrap();
            building.components.parent.insert(
                ElementRef(0),
                Parent {
                    of: Some(Membership {
                        target: parent,
                        kind: ParentKind::Aggregates,
                        order: None,
                    }),
                    referenced_in: vec![],
                },
            );
            building
                .components
                .opening
                .insert(ElementRef(4), Opening::Voids(parent));
        }
        let mut used = Budget::default();
        evaluation::evaluate(
            &building,
            Options::default(),
            &mut Cx::new(&mut used, &mut Diagnostics::new(), &cancel),
        )
        .unwrap();
        let mut runs = vec![
            (Limits::default(), false, true),
            (Limits::default(), false, false),
            (Limits::default(), true, true),
        ];
        for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments, Kind::Depth] {
            let value = used.used(kind);
            for bound in [0, value.saturating_sub(1), value] {
                runs.push((
                    Limits::default().with(kind, Limit::Finite(bound)),
                    false,
                    true,
                ));
            }
        }
        for (limits, cancelled, openings) in runs {
            let cancel = CancelToken::new();
            if cancelled {
                cancel.cancel();
            }
            let mut budget = Budget::new(limits);
            let mut diagnostics = Diagnostics::new();
            let result = evaluation::evaluate(
                &building,
                Options {
                    openings,
                    ..Options::default()
                },
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            );
            match result {
                Ok(elements) => {
                    text(&mut out, "ok");
                    number(&mut out, elements.len() as u64);
                    for element in elements {
                        text(&mut out, &element.id.to_string());
                        number(&mut out, u64::from(element.element.0));
                        number(&mut out, u64::from(element.complete));
                        number(&mut out, element.parts.len() as u64);
                        for part in element.parts {
                            number(&mut out, u64::from(part.item.0));
                            real(&mut out, part.tolerance.chord);
                            real(&mut out, part.tolerance.length);
                            for row in part.placement.0 {
                                for v in row {
                                    real(&mut out, v);
                                }
                            }
                            mesh(&mut out, &part.mesh);
                        }
                    }
                }
                Err(error) => text(&mut out, &format!("{error:?}")),
            }
            observations(&mut out, &budget, diagnostics);
        }
    }
    out
}
