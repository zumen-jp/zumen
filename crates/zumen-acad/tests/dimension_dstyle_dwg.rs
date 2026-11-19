//! R2010・R2013・R2018 の DWG で、個別の `DSTYLE` 上書きと拡張辞書を持つ平行寸法を移動したときに、
//! 再生成した匿名ブロックが実効の文字高・矢印長さ・小数桁数を反映し、補助 `POINT` を除いた表示部品になり、
//! 測点・測定値・拡張辞書・上書き値を保つことを確かめる（#905）。
#![cfg(feature = "drawing")]

use zumen_acad::{AcadLive, Data, Dwg, DwgFile, Object, Section};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{
    Affine, Changes, DimensionKind, Drawing2D, Edit, Field, FieldValue, Hash, ModelPoint, Seen,
    Shape, Target, id,
};
use zumen_sdk::{Apply, Lift, testkit};

#[cfg(test)]
fn cx_run<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let value = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (value, diag)
}

#[cfg(test)]
fn first_dimension(drawing: &Drawing2D) -> (String, Hash, zumen_drawing::Element<ModelPoint>) {
    let mut out = None;
    id::each_element(drawing, |_, elem_id, found| {
        if let id::Found::Model(el) = found
            && matches!(el.shape, Shape::Dimension(_))
        {
            out = Some((elem_id, Hash::element(el), el.clone()));
        }
        Ok::<_, ()>(())
    })
    .unwrap();
    out.unwrap()
}

#[cfg(test)]
fn extensions_for<'a>(
    drawing: &'a Drawing2D,
    elem_id: &str,
) -> Vec<(&'a str, &'a str, &'a [Field])> {
    drawing
        .extensions
        .iter()
        .filter(|ext| ext.target == Target::Element(elem_id.to_owned()))
        .map(|ext| {
            (
                ext.namespace.as_ref(),
                ext.name.as_str(),
                ext.fields.as_slice(),
            )
        })
        .collect()
}

#[cfg(test)]
fn solid_arrow_length(obj: &Object) -> f64 {
    let real_of = |code: i16| match obj.first(code) {
        Some(Data::Real(v)) => *v,
        _ => panic!("missing real {code} in SOLID: {obj:?}"),
    };
    let pts = [
        [real_of(10), real_of(20)],
        [real_of(11), real_of(21)],
        [real_of(12), real_of(22)],
        [real_of(13), real_of(23)],
    ];
    let mut uniq: Vec<[f64; 2]> = Vec::new();
    for p in pts {
        if !uniq
            .iter()
            .any(|u| (u[0] - p[0]).abs() < 1e-9 && (u[1] - p[1]).abs() < 1e-9)
        {
            uniq.push(p);
        }
    }
    assert_eq!(uniq.len(), 3, "expected triangle SOLID, got {uniq:?}");
    let mut shortest_base = f64::MAX;
    let mut arrow_len = 0.0;
    for i in 0..3 {
        let a = uniq[(i + 1) % 3];
        let b = uniq[(i + 2) % 3];
        let base_len = libm::hypot(b[0] - a[0], b[1] - a[1]);
        if base_len < shortest_base {
            shortest_base = base_len;
            let mid = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
            arrow_len = libm::hypot(uniq[i][0] - mid[0], uniq[i][1] - mid[1]);
        }
    }
    arrow_len
}

#[cfg(test)]
fn verify_rebuilt_dwg(label: &str, bytes: &[u8]) {
    let file: DwgFile = testkit::read::<Dwg>(bytes).0.unwrap();
    let db = cx_run(|cx| file.objects(cx)).0.unwrap();
    let drawing = cx_run(|cx| file.lift(cx)).0.unwrap();
    let (dim_id, _, dim_el) = first_dimension(&drawing);

    // 1. 測点 (14, 20)・(44, 60) と寸法長 50 を確認
    let Shape::Dimension(ref dim) = dim_el.shape else {
        panic!("{label}: expected Dimension");
    };
    let DimensionKind::Aligned { first, second, .. } = dim.kind else {
        panic!("{label}: expected Aligned dimension, got {:?}", dim.kind);
    };
    assert!(
        (first.x - 14.0).abs() < 1e-9 && (first.y - 20.0).abs() < 1e-9,
        "{label}: {first:?}"
    );
    assert!(
        (second.x - 44.0).abs() < 1e-9 && (second.y - 60.0).abs() < 1e-9,
        "{label}: {second:?}"
    );
    let dim_obj = db
        .objects()
        .iter()
        .find(|o| o.section == Section::Entities && o.kind == b"DIMENSION")
        .unwrap();
    let Some(Data::Real(measurement)) = dim_obj.first(42) else {
        panic!("{label}: missing 42 measurement on DIMENSION");
    };
    assert!(
        (*measurement - 50.0).abs() < 1e-9,
        "{label}: measurement={measurement}"
    );

    // 2. DSTYLE 上書き（末尾 U+0000 なし）と拡張辞書 VERIFY_VALUES の 3 値を確認
    let exts = extensions_for(&drawing, &dim_id);
    let (_, _, acad_fields) = exts
        .iter()
        .find(|(ns, name, _)| *ns == "dxf.xdata" && *name == "ACAD")
        .unwrap_or_else(|| panic!("{label}: missing ACAD xdata in {exts:?}"));
    assert_eq!(
        acad_fields[0].value,
        FieldValue::Text("DSTYLE".to_owned()),
        "{label}: {acad_fields:?}"
    );
    assert!(
        acad_fields.windows(2).any(|w| {
            w[0].value == FieldValue::Integer(41) && w[1].value == FieldValue::Real(3.625)
        }),
        "{label}: missing 41=3.625 in {acad_fields:?}"
    );
    assert!(
        acad_fields.windows(2).any(|w| {
            w[0].value == FieldValue::Integer(140) && w[1].value == FieldValue::Real(4.75)
        }),
        "{label}: missing 140=4.75 in {acad_fields:?}"
    );
    assert!(
        acad_fields.windows(2).any(|w| {
            w[0].value == FieldValue::Integer(271) && w[1].value == FieldValue::Integer(3)
        }),
        "{label}: missing 271=3 in {acad_fields:?}"
    );

    let (_, _, xrec_fields) = exts
        .iter()
        .find(|(ns, name, _)| *ns == "dxf.xrecord" && *name == "VERIFY_VALUES")
        .unwrap_or_else(|| panic!("{label}: missing VERIFY_VALUES xrecord in {exts:?}"));
    assert!(
        xrec_fields
            .iter()
            .any(|f| f.code == Some(1) && f.value == FieldValue::Text("A-19".to_owned())),
        "{label}: {xrec_fields:?}"
    );
    assert!(
        xrec_fields
            .iter()
            .any(|f| f.code == Some(70) && f.value == FieldValue::Integer(321)),
        "{label}: {xrec_fields:?}"
    );
    assert!(
        xrec_fields
            .iter()
            .any(|f| f.code == Some(40) && f.value == FieldValue::Real(6.125)),
        "{label}: {xrec_fields:?}"
    );

    // 3. 匿名ブロックの部品構成（補助 POINT は残さず LINE×3・SOLID×2・MTEXT×1）と実効文字高 4.75・矢印長さ 3.625・小数桁 3 を確認
    let Some(Data::Text(block_name)) = dim_obj.first(2) else {
        panic!("{label}: missing block name 2 on DIMENSION: {dim_obj:?}");
    };
    let d1_rec = db
        .objects()
        .iter()
        .find(|o| {
            o.section == Section::Tables
                && o.kind == b"BLOCK_RECORD"
                && matches!(o.first(2), Some(Data::Text(n)) if n.eq_ignore_ascii_case(block_name))
        })
        .unwrap_or_else(|| panic!("{label}: missing BLOCK_RECORD for {block_name:?}"));
    let d1_handle = d1_rec.handle().unwrap();
    let parts: Vec<&Object> = db
        .objects()
        .iter()
        .filter(|o| {
            o.section == Section::Blocks
                && o.kind != b"BLOCK"
                && o.kind != b"ENDBLK"
                && matches!(o.first(330), Some(Data::Handle(h)) if *h == d1_handle)
        })
        .collect();
    let count_kind = |k: &[u8]| parts.iter().filter(|o| o.kind.as_slice() == k).count();
    assert_eq!(
        count_kind(b"POINT"),
        0,
        "{label}: auxiliary POINTs must not remain"
    );
    assert_eq!(count_kind(b"LINE"), 3, "{label}");
    assert_eq!(count_kind(b"SOLID"), 2, "{label}");
    assert_eq!(count_kind(b"MTEXT"), 1, "{label}");

    let mtext = parts.iter().find(|o| o.kind == b"MTEXT").unwrap();
    let Some(Data::Real(h)) = mtext.first(40) else {
        panic!("{label}: missing MTEXT 40 height");
    };
    assert!((*h - 4.75).abs() < 1e-9, "{label}: mtext height={h}");
    let Some(Data::Text(txt)) = mtext.first(1) else {
        panic!("{label}: missing MTEXT 1 text");
    };
    assert_eq!(txt.as_slice(), b"50", "{label}: mtext text={txt:?}");

    for solid in parts.iter().filter(|o| o.kind == b"SOLID") {
        let len = solid_arrow_length(solid);
        assert!(
            (len - 3.625).abs() < 1e-9,
            "{label}: expected arrow length 3.625, got {len}"
        );
    }
}

#[test]
fn dwg_dimension_rebuild_applies_dstyle_overrides_and_omits_auxiliary_points() {
    let shift = Affine {
        m: [1.0, 0.0, 11.0, 0.0, 1.0, 13.0],
    };
    let fixtures: &[(&str, &[u8])] = &[
        ("r2010", include_bytes!("fixtures/dim-dstyle-r2010.dwg")),
        ("r2013", include_bytes!("fixtures/dim-dstyle-r2013.dwg")),
        ("r2018", include_bytes!("fixtures/dim-dstyle-r2018.dwg")),
    ];

    for &(ver, src) in fixtures {
        // 入力時点でも DSTYLE の文字列に末尾 NUL が付かず、元の *D1 には POINT×4 があることを確認
        let orig_file = testkit::read::<Dwg>(src).0.unwrap();
        let orig_db = cx_run(|cx| orig_file.objects(cx)).0.unwrap();
        let orig_drawing = cx_run(|cx| orig_file.lift(cx)).0.unwrap();
        let (dim_id, dim_hash, _) = first_dimension(&orig_drawing);
        let orig_exts = extensions_for(&orig_drawing, &dim_id);
        let (_, _, orig_acad) = orig_exts
            .iter()
            .find(|(ns, name, _)| *ns == "dxf.xdata" && *name == "ACAD")
            .unwrap();
        assert_eq!(
            orig_acad[0].value,
            FieldValue::Text("DSTYLE".to_owned()),
            "{ver}"
        );
        let orig_points = orig_db
            .objects()
            .iter()
            .filter(|o| o.section == Section::Blocks && o.kind == b"POINT")
            .count();
        assert_eq!(orig_points, 4, "{ver}");

        let changes = Changes {
            document: None,
            edits: vec![Edit::Transform {
                target: Seen {
                    id: dim_id,
                    hash: dim_hash,
                },
                transform: shift,
            }],
        };

        // DwgFile::apply と AcadLive::apply の両方で同じ結果を確かめる
        let mut by_file = orig_file;
        cx_run(|cx| by_file.apply(&changes, cx)).0.unwrap();
        let out_file_bytes = testkit::write::<Dwg>(&by_file).unwrap();
        verify_rebuilt_dwg(&format!("{ver}-file"), &out_file_bytes);

        let mut live = cx_run(|cx| AcadLive::open_dwg(src, cx)).0.unwrap();
        cx_run(|cx| live.apply(&changes, cx)).0.unwrap();
        let out_live_bytes = cx_run(|cx| live.save(cx)).0.unwrap();
        verify_rebuilt_dwg(&format!("{ver}-live"), &out_live_bytes);
    }
}
