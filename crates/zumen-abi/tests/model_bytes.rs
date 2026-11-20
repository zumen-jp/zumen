//! 図面モデルのメモリ予算（`Kind::ModelBytes`・`budget.model_bytes`）の統合試験（#559）。

#![cfg(feature = "drawing")]

use zumen_abi1::{Session, Status};
use zumen_acad::{Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::{Drawing2D, binary};
use zumen_json::Value;
use zumen_jww::{Jww, JwwFile};
use zumen_sdk::{Codec, Lift};
use zumen_sima::{Sima, SimaFile};
use zumen_sxf::{Sxf, SxfFile};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn call(session: &mut Session, request: &str) -> Result<(Status, Value), String> {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    let parsed = zumen_json::parse(&reply.response).map_err(|e| format!("{e:?}"))?;
    Ok((reply.status, parsed))
}

fn multi_entity_dxf() -> Vec<u8> {
    let mut text = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1027\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
         0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n1\n\
         0\nLAYER\n5\n10\n100\nAcDbSymbolTableRecord\n100\nAcDbLayerTableRecord\n2\nWALL\n70\n0\n62\n1\n6\nCONTINUOUS\n\
         0\nENDTAB\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n",
    );
    for idx in 0..8_u32 {
        let handle = 0x20 + idx;
        let y = f64::from(idx) * 10.0;
        text.push_str(&format!(
            "0\nLWPOLYLINE\n5\n{handle:X}\n8\nWALL\n90\n5\n70\n1\n\
             10\n0.0\n20\n{y}\n10\n50.0\n20\n{y}\n10\n50.0\n20\n{}\n10\n25.0\n20\n{}\n10\n0.0\n20\n{}\n\
             1001\nZUMEN_TEST\n1000\npoly-{idx}-payload\n",
            y + 5.0,
            y + 8.0,
            y + 5.0,
        ));
    }
    text.push_str("0\nENDSEC\n0\nEOF\n");
    text.into_bytes()
}

fn lift_with_limit<F: Lift<Drawing2D>>(
    file: &F,
    limit: Limit,
) -> (Result<Drawing2D, zumen_sdk::Error>, u64, Diagnostics) {
    let limits = Limits::default().with(Kind::ModelBytes, limit);
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let res = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (res, budget.used(Kind::ModelBytes), diag)
}

#[test]
fn lifters_account_exact_logical_bytes_and_stop_mid_stream_with_source() -> TestResult {
    // 1. DXF（複数ポリライン＋XDATA）: DecodedBytes が 0 でも ModelBytes 単独で動作し、途中レコードの出所で止まる
    let dxf_bytes = multi_entity_dxf();
    let dxf: DxfFile = {
        let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(0)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        Dxf::read(&dxf_bytes, &mut Cx::new(&mut budget, &mut diag, &cancel))
            .map_err(|e| format!("{e:?}"))?
    };
    let (full, dxf_used, _) = lift_with_limit(&dxf, Limit::Unlimited);
    let full = full.map_err(|e| format!("{e:?}"))?;
    assert_eq!(dxf_used, full.total_model_bytes());

    // 1 つ目の要素の途中〜半分の上限では、最後まで作らず元の要素レコードの出所を添えて失敗する
    let mid_limit = dxf_used / 2;
    let (mid_res, _, mid_diag) = lift_with_limit(&dxf, Limit::Finite(mid_limit));
    assert!(matches!(mid_res, Err(zumen_sdk::Error::Stopped(_))));
    let exceeded = mid_diag
        .iter()
        .find(|d| d.code().name() == "budget.model-bytes.exceeded")
        .ok_or("budget.model-bytes.exceeded がない")?;
    let first_place = exceeded.places().first().ok_or("出所がない")?;
    assert!(first_place.record().is_some(), "{exceeded:?}");

    // ちょうど dxf_used - 1 で止まり、dxf_used で成功する
    let (minus_one, _, _) = lift_with_limit(&dxf, Limit::Finite(dxf_used - 1));
    assert!(matches!(minus_one, Err(zumen_sdk::Error::Stopped(_))));
    let (exact, exact_used, _) = lift_with_limit(&dxf, Limit::Finite(dxf_used));
    assert_eq!(exact.map_err(|e| format!("{e:?}"))?, full);
    assert_eq!(exact_used, dxf_used);

    // 2. DWG・JWW・SXF・SIMA でも total_model_bytes と一致し、超過時は元の場所付きで止まる
    let dwg: DwgFile = {
        let raw = include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg");
        let mut b = Budget::default();
        let mut d = Diagnostics::new();
        let c = CancelToken::new();
        Dwg::read(raw, &mut Cx::new(&mut b, &mut d, &c)).map_err(|e| format!("{e:?}"))?
    };
    let (dwg_full, dwg_used, _) = lift_with_limit(&dwg, Limit::Unlimited);
    let dwg_full = dwg_full.map_err(|e| format!("{e:?}"))?;
    assert_eq!(dwg_used, dwg_full.total_model_bytes());
    let (dwg_stop, _, dwg_diag) = lift_with_limit(&dwg, Limit::Finite(dwg_used / 2));
    assert!(matches!(dwg_stop, Err(zumen_sdk::Error::Stopped(_))));
    assert!(
        dwg_diag
            .iter()
            .any(|d| d.code().name() == "budget.model-bytes.exceeded" && !d.places().is_empty())
    );
    assert!(
        lift_with_limit(&dwg, Limit::Finite(dwg_used - 1))
            .0
            .is_err()
    );
    assert!(lift_with_limit(&dwg, Limit::Finite(dwg_used)).0.is_ok());

    let jww: JwwFile = {
        let raw = include_bytes!("../../../samples/jww/ezjww-geometry.jww");
        let mut b = Budget::default();
        let mut d = Diagnostics::new();
        let c = CancelToken::new();
        Jww::read(raw, &mut Cx::new(&mut b, &mut d, &c)).map_err(|e| format!("{e:?}"))?
    };
    let (jww_full, jww_used, _) = lift_with_limit(&jww, Limit::Unlimited);
    let jww_full = jww_full.map_err(|e| format!("{e:?}"))?;
    assert_eq!(jww_used, jww_full.total_model_bytes());
    let (jww_stop, _, jww_diag) = lift_with_limit(&jww, Limit::Finite(jww_used / 2));
    assert!(matches!(jww_stop, Err(zumen_sdk::Error::Stopped(_))));
    assert!(
        jww_diag
            .iter()
            .any(|d| d.code().name() == "budget.model-bytes.exceeded" && !d.places().is_empty())
    );
    assert!(
        lift_with_limit(&jww, Limit::Finite(jww_used - 1))
            .0
            .is_err()
    );
    assert!(lift_with_limit(&jww, Limit::Finite(jww_used)).0.is_ok());

    let sxf: SxfFile = {
        let raw = include_bytes!("../../../samples/sxf/ezsxf-writer-all-features.sfc");
        let mut b = Budget::default();
        let mut d = Diagnostics::new();
        let c = CancelToken::new();
        Sxf::read(raw, &mut Cx::new(&mut b, &mut d, &c)).map_err(|e| format!("{e:?}"))?
    };
    let (sxf_full, sxf_used, _) = lift_with_limit(&sxf, Limit::Unlimited);
    let sxf_full = sxf_full.map_err(|e| format!("{e:?}"))?;
    assert_eq!(sxf_used, sxf_full.total_model_bytes());
    let (sxf_stop, _, sxf_diag) = lift_with_limit(&sxf, Limit::Finite(sxf_used / 2));
    assert!(matches!(sxf_stop, Err(zumen_sdk::Error::Stopped(_))));
    assert!(
        sxf_diag
            .iter()
            .any(|d| d.code().name() == "budget.model-bytes.exceeded" && !d.places().is_empty())
    );
    assert!(
        lift_with_limit(&sxf, Limit::Finite(sxf_used - 1))
            .0
            .is_err()
    );
    assert!(lift_with_limit(&sxf, Limit::Finite(sxf_used)).0.is_ok());

    let sima: SimaFile = {
        let raw = b"G00,01,\x93\x8c\x8b\x9e,\nZ00,\x8d\x91\x93\x79,\nA00,\n\
                    A01,1,P1,100.000,200.000,10.000,\n\
                    A01,2,P2,110.000,200.000,10.000,\n\
                    A01,3,P3,110.000,210.000,10.000,\n\
                    A99,\nD00,1,LOT1,1,\nB01,1,P1,\nB01,2,P2,\nB01,3,P3,\nD99,\nZ99,\n";
        let mut b = Budget::default();
        let mut d = Diagnostics::new();
        let c = CancelToken::new();
        Sima::read(raw, &mut Cx::new(&mut b, &mut d, &c)).map_err(|e| format!("{e:?}"))?
    };
    let (sima_full, sima_used, _) = lift_with_limit(&sima, Limit::Unlimited);
    let sima_full = sima_full.map_err(|e| format!("{e:?}"))?;
    assert_eq!(sima_used, sima_full.total_model_bytes());
    assert!(
        lift_with_limit(&sima, Limit::Finite(sima_used - 1))
            .0
            .is_err()
    );
    assert!(lift_with_limit(&sima, Limit::Finite(sima_used)).0.is_ok());

    // 3. ZMDR バイナリの往復でも同じ論理バイト数で境界が決まる
    let zmdr = {
        let mut b = Budget::default();
        let mut d = Diagnostics::new();
        let c = CancelToken::new();
        let src = binary::Source {
            format: "dxf".to_owned(),
            family: "autocad".to_owned(),
            hash: zumen_drawing::Hash([1; 16]),
        };
        binary::write(&src, &full, &mut Cx::new(&mut b, &mut d, &c))
            .map_err(|e| format!("{e:?}"))?
    };
    for (limit, ok) in [(dxf_used - 1, false), (dxf_used, true)] {
        let mut b = Budget::new(Limits::default().with(Kind::ModelBytes, Limit::Finite(limit)));
        let mut d = Diagnostics::new();
        let c = CancelToken::new();
        let res = binary::read(&zmdr, &mut Cx::new(&mut b, &mut d, &c));
        assert_eq!(res.is_ok(), ok, "limit={limit}");
        if ok {
            assert_eq!(b.used(Kind::ModelBytes), dxf_used);
        }
    }
    Ok(())
}

#[test]
fn session_refuses_partial_results_and_preserves_document_on_model_bytes_exceeded() -> TestResult {
    let dxf_bytes = multi_entity_dxf();
    let total = {
        let mut b = Budget::default();
        let mut d = Diagnostics::new();
        let c = CancelToken::new();
        let file = Dxf::read(&dxf_bytes, &mut Cx::new(&mut b, &mut d, &c))
            .map_err(|e| format!("{e:?}"))?;
        let (_, used, _) = lift_with_limit(&file, Limit::Unlimited);
        used
    };

    let mut session = Session::new(b"").map_err(|e| format!("{e:?}"))?;
    session
        .push(b"plan.dxf", &dxf_bytes)
        .map_err(|e| format!("{e:?}"))?;
    let (st, _) = call(&mut session, r#"{"v":1,"op":"open","input":"plan.dxf"}"#)?;
    assert_eq!(st, Status::Ok);

    // 1. 変換前（cold）の上限超過: status=Failed、result なし、blob なし、budget.model-bytes.exceeded を返す
    for op in ["drawing", "entities", "lift"] {
        let (st, res) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"{op}","doc":"plan.dxf","budget":{{"model_bytes":{}}}}}"#,
                total - 1
            ),
        )?;
        assert_eq!(st, Status::Failed, "{op}: {res:?}");
        assert!(res.pointer("/result").is_none(), "{op}: {res:?}");
        assert_eq!(
            res.pointer("/blobs"),
            Some(&Value::Array(Vec::new())),
            "{op}: {res:?}"
        );
        assert_eq!(
            res.pointer("/diagnostics/0/code"),
            Some(&Value::String("budget.model-bytes.exceeded".to_owned())),
            "{op}: {res:?}"
        );
        assert_eq!(
            res.pointer("/diagnostics/0/params/limit")
                .and_then(|v| match v {
                    Value::Number(n) => n.to_safe_integer(),
                    _ => None,
                }),
            Some((total - 1) as i64),
            "{op}: {res:?}"
        );
        assert!(session.take(0).is_none());
    }

    // 2. 失敗のあとも文書は壊れず、ちょうど total の上限で全要素（8 件）と ZMDR blob を返す
    let (st, res) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"entities","doc":"plan.dxf","budget":{{"model_bytes":{total}}}}}"#
        ),
    )?;
    assert_eq!(st, Status::Ok, "{res:?}");
    assert_eq!(
        res.pointer("/result/total").and_then(|v| match v {
            Value::Number(n) => n.to_safe_integer(),
            _ => None,
        }),
        Some(8)
    );

    // 3. 常駐後（warm）でも小さい model_bytes を渡せば同じ理由で断り、部分結果を返さない
    let (st, res) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"drawing","doc":"plan.dxf","budget":{{"model_bytes":{}}}}}"#,
            total / 2
        ),
    )?;
    assert_eq!(st, Status::Failed, "{res:?}");
    assert!(res.pointer("/result").is_none());
    assert_eq!(
        res.pointer("/diagnostics/0/code"),
        Some(&Value::String("budget.model-bytes.exceeded".to_owned()))
    );

    // 4. lift した ZMDR を open するときも、ちょうど total - 1 で失敗し total で成功する
    let (st, _) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"lift","doc":"plan.dxf","budget":{{"model_bytes":{total}}}}}"#),
    )?;
    assert_eq!(st, Status::Ok);
    let zmdr = session.take(0).ok_or("ZMDR blob がない")?;
    session
        .push(b"plan.zmdr", &zmdr)
        .map_err(|e| format!("{e:?}"))?;

    let (st, res) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"open","input":"plan.zmdr","budget":{{"model_bytes":{}}}}}"#,
            total - 1
        ),
    )?;
    assert_eq!(st, Status::Failed, "{res:?}");
    assert!(res.pointer("/result").is_none());
    assert_eq!(
        res.pointer("/diagnostics/0/code"),
        Some(&Value::String("budget.model-bytes.exceeded".to_owned()))
    );

    let (st, res) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"open","input":"plan.zmdr","budget":{{"model_bytes":{total}}}}}"#),
    )?;
    assert_eq!(st, Status::Ok, "{res:?}");
    assert_eq!(
        res.pointer("/result/format"),
        Some(&Value::String("dxf".to_owned()))
    );
    Ok(())
}
