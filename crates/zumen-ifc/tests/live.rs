//! IFC 常駐文書エンジン（[`IfcLive`]）の統合テスト（#878）。
#![cfg(feature = "building")]

use std::path::{Path, PathBuf};

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_building::{Building, ElementRef, Hash};
#[cfg(feature = "ids")]
use zumen_ids::Lookup;
use zumen_ifc::{Edit, Ifc, IfcFile, IfcLive, IfcZip, NewLiteral, NewValue, RootAttribute, apply};
#[cfg(feature = "ids")]
use zumen_ifc::{Express, IdsLookup};
use zumen_sdk::{Codec, Error, Lift};

#[cfg(test)]
fn gid(n: u32) -> String {
    format!("{n:0>22}")
}

#[cfg(test)]
fn with_cx<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (out, diag)
}

#[cfg(test)]
fn read_file(src: &[u8]) -> IfcFile {
    with_cx(|cx| Ifc::read(src, cx)).0.unwrap()
}

#[cfg(test)]
fn lift_building(src: &[u8]) -> Building {
    let file = read_file(src);
    with_cx(|cx| file.lift(cx)).0.unwrap()
}

#[cfg(test)]
fn element_hashes(building: &Building) -> Vec<Option<Hash>> {
    let mut out = Vec::with_capacity(building.elements().len());
    for idx in 0..building.elements().len() {
        let elem = ElementRef(idx as u32);
        let h = with_cx(|cx| Hash::element_checked(building, elem, cx))
            .0
            .unwrap();
        out.push(h);
    }
    out
}

#[cfg(test)]
fn rich_sample_ifc() -> String {
    let lines = [
        format!(
            "#1=IFCPROJECT('{}',#2,'Main Project',$,$,$,$,$,#10);",
            gid(1)
        ),
        "#2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);".to_owned(),
        "#10=IFCUNITASSIGNMENT((#11,#12));".to_owned(),
        "#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);".to_owned(),
        "#12=IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.);".to_owned(),
        "#13=IFCSIUNIT(*,.VOLUMEUNIT.,$,.CUBIC_METRE.);".to_owned(),
        format!(
            "#15=IFCBUILDING('{}',#2,'Tower',$,$,$,$,$,.ELEMENT.,0.,$,$);",
            gid(15)
        ),
        format!(
            "#20=IFCBUILDINGSTOREY('{}',#2,'1F',$,$,$,$,$,.ELEMENT.,3000.);",
            gid(20)
        ),
        format!("#21=IFCRELAGGREGATES('{}',#2,$,$,#15,(#20));", gid(21)),
        format!(
            "#25=IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',#2,$,$,(#30,#31,#32,#33),#20);",
            gid(25)
        ),
        format!(
            "#30=IFCWALL('{}',#2,'Wall A',$,$,$,$,$,.STANDARD.);",
            gid(30)
        ),
        format!("#31=IFCWALL('{}',#2,'Wall B',$,$,$,$,$,$);", gid(31)),
        format!(
            "#32=IFCDOOR('{}',#2,'Door 1',$,$,$,$,$,$,$,$,$,$);",
            gid(32)
        ),
        format!("#33=IFCSLAB('{}',#2,'Slab 1',$,$,$,$,$,.FLOOR.);", gid(33)),
        format!(
            "#34=IFCDOORTYPE('{}',#2,'DoorType A',$,$,(#60),$,$,$,.DOOR.,$,$,$);",
            gid(34)
        ),
        "#40=IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('EI30'),$);".to_owned(),
        "#41=IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.F.),$);".to_owned(),
        "#44=IFCPROPERTYSINGLEVALUE('NetArea',$,IFCAREAMEASURE(12.5),#12);".to_owned(),
        format!(
            "#42=IFCPROPERTYSET('{}',#2,'Pset_WallCommon',$,(#40,#41,#44));",
            gid(42)
        ),
        format!(
            "#43=IFCRELDEFINESBYPROPERTIES('{}',#2,$,$,(#30,#31),#42);",
            gid(43)
        ),
        "#50=IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('EI60'),$);".to_owned(),
        format!(
            "#51=IFCPROPERTYSET('{}',#2,'Pset_DoorCommon',$,(#50));",
            gid(51)
        ),
        format!(
            "#52=IFCRELDEFINESBYPROPERTIES('{}',#2,$,$,(#32),#51);",
            gid(52)
        ),
        // #50 を別のセット #55 とも共有させ、共有プロパティ単体の写し作成を検証する
        format!(
            "#55=IFCPROPERTYSET('{}',#2,'Pset_SharedPropHolder',$,(#50));",
            gid(55)
        ),
        format!(
            "#56=IFCRELDEFINESBYPROPERTIES('{}',#2,$,$,(#31),#55);",
            gid(56)
        ),
        "#59=IFCPROPERTYSINGLEVALUE('Reference',$,IFCIDENTIFIER('D1'),$);".to_owned(),
        format!(
            "#60=IFCPROPERTYSET('{}',#2,'Pset_DoorTypeCommon',$,(#59));",
            gid(60)
        ),
        "#70=IFCMATERIAL('Concrete',$,$);".to_owned(),
        format!(
            "#75=IFCRELASSOCIATESMATERIAL('{}',#2,$,$,(#30,#31),#70);",
            gid(75)
        ),
        "#81=IFCQUANTITYVOLUME('NetVolume',$,#13,4.25,'w*h*t');".to_owned(),
        format!(
            "#80=IFCELEMENTQUANTITY('{}',#2,'Qto_WallBaseQuantities',$,'BaseQuantities',(#81));",
            gid(80)
        ),
        format!(
            "#82=IFCRELDEFINESBYPROPERTIES('{}',#2,$,$,(#30),#80);",
            gid(82)
        ),
    ];
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');\nFILE_NAME('rich.ifc','2026-01-01T00:00:00',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
        lines.join("\n")
    )
}

#[cfg(test)]
fn prop_edit(element: u64, set: &str, name: &str, type_name: &str, literal: NewLiteral) -> Edit {
    Edit::Property {
        element,
        set: set.to_owned(),
        name: name.to_owned(),
        value: NewValue {
            type_name: type_name.to_owned(),
            literal,
        },
    }
}

#[cfg(test)]
fn collect_ifc_samples(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_ifc_samples(&path, out);
        } else if path
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| {
                ext.eq_ignore_ascii_case("ifc") || ext.eq_ignore_ascii_case("ifczip")
            })
        {
            out.push(path);
        }
    }
}

#[test]
fn multi_step_edits_save_building_hash_and_undo_redo_parity() {
    let initial_text = rich_sample_ifc();
    let initial_bytes = initial_text.as_bytes().to_vec();

    let mut live = with_cx(|cx| IfcLive::open(&initial_bytes, cx)).0.unwrap();
    assert!(live.is_clean());
    assert!(!live.is_zip());
    assert_eq!(live.schema(), b"IFC4");
    assert_eq!(live.state_id(), 0);

    let initial_building = lift_building(&initial_bytes);
    assert_eq!(live.building(), &initial_building);
    assert_eq!(with_cx(|cx| live.save_spf(cx)).0.unwrap(), initial_bytes);

    let steps: Vec<(Vec<Edit>, &[u8])> = vec![
        // Step 1: 専用セット内の共有 IfcPropertySingleValue (#50) の写し作成と更新
        (
            vec![prop_edit(
                32,
                "Pset_DoorCommon",
                "FireRating",
                "IfcLabel",
                NewLiteral::Text("EI90".to_owned()),
            )],
            b"seed-step-1",
        ),
        // Step 2: 共有セット (#42) の写し作成（兄弟 #31 の RelationSource.position が 1 -> 0 に詰まる）
        (
            vec![prop_edit(
                30,
                "Pset_WallCommon",
                "FireRating",
                "IfcLabel",
                NewLiteral::Text("EI120".to_owned()),
            )],
            b"seed-step-2",
        ),
        // Step 3: 新規セットと複数型（Text / Real+単位 / Integer / Boolean / Logical）の追加
        (
            vec![
                prop_edit(
                    33,
                    "Pset_SlabCommon",
                    "Reference",
                    "IfcIdentifier",
                    NewLiteral::Text("SL-01".to_owned()),
                ),
                prop_edit(
                    33,
                    "Pset_SlabCommon",
                    "Thickness",
                    "IfcPositiveLengthMeasure",
                    NewLiteral::Real(220.0),
                ),
                prop_edit(
                    33,
                    "Pset_SlabCommon",
                    "Count",
                    "IfcInteger",
                    NewLiteral::Integer(4),
                ),
                prop_edit(
                    33,
                    "Pset_SlabCommon",
                    "LoadBearing",
                    "IfcBoolean",
                    NewLiteral::Boolean(true),
                ),
                prop_edit(
                    33,
                    "Pset_SlabCommon",
                    "Combustible",
                    "IfcLogical",
                    NewLiteral::Logical(None),
                ),
            ],
            b"seed-step-3",
        ),
        // Step 4: 型要素 (#34, IfcDoorType) の既存セット変更と新規セット追加
        (
            vec![
                prop_edit(
                    34,
                    "Pset_DoorTypeCommon",
                    "Reference",
                    "IfcIdentifier",
                    NewLiteral::Text("D2".to_owned()),
                ),
                prop_edit(
                    34,
                    "Pset_DoorTypeExtra",
                    "AcousticRating",
                    "IfcLabel",
                    NewLiteral::Text("STC-45".to_owned()),
                ),
            ],
            b"seed-step-4",
        ),
        // Step 5: Attribute 変更（IfcProject 名・IfcBuildingStorey の Level 名・非 ASCII と絵文字・Name 削除による Identity 消去）
        (
            vec![
                Edit::Attribute {
                    element: 1,
                    attribute: RootAttribute::Name,
                    value: Some("プロジェクト改".to_owned()),
                },
                Edit::Attribute {
                    element: 20,
                    attribute: RootAttribute::Name,
                    value: Some("1階ロビー".to_owned()),
                },
                Edit::Attribute {
                    element: 30,
                    attribute: RootAttribute::Name,
                    value: Some("外壁 'A' \\ 🧱".to_owned()),
                },
                Edit::Attribute {
                    element: 30,
                    attribute: RootAttribute::Description,
                    value: Some("耐火壁".to_owned()),
                },
                Edit::Attribute {
                    element: 31,
                    attribute: RootAttribute::Name,
                    value: None,
                },
            ],
            b"seed-step-5",
        ),
        // Step 6: L1 で直接 IfcPropertySet (#42) と IfcElementQuantity (#80) の Name を書き換え
        (
            vec![
                Edit::Attribute {
                    element: 42,
                    attribute: RootAttribute::Name,
                    value: Some("Pset_WallCommon_B".to_owned()),
                },
                Edit::Attribute {
                    element: 80,
                    attribute: RootAttribute::Name,
                    value: Some("Qto_WallBase_Renamed".to_owned()),
                },
            ],
            b"seed-step-6",
        ),
        // Step 7: 前の段階で新設したセット（#30 の Pset_WallCommon と #33 の Pset_SlabCommon）の再編集
        (
            vec![
                prop_edit(
                    30,
                    "Pset_WallCommon",
                    "ThermalTransmittance",
                    "IfcReal",
                    NewLiteral::Real(0.35),
                ),
                prop_edit(
                    33,
                    "Pset_SlabCommon",
                    "Thickness",
                    "IfcPositiveLengthMeasure",
                    NewLiteral::Real(250.0),
                ),
            ],
            b"seed-step-7",
        ),
    ];

    let mut snapshots: Vec<(Vec<u8>, Building, Vec<Option<Hash>>)> = vec![(
        initial_bytes.clone(),
        initial_building.clone(),
        element_hashes(&initial_building),
    )];

    let mut seq_bytes = initial_bytes;
    for (step_idx, (edits, seed)) in steps.iter().enumerate() {
        let seq_file = read_file(&seq_bytes);
        let seq_applied = with_cx(|cx| apply(&seq_file, edits, seed, cx)).0.unwrap();
        seq_bytes = seq_applied.bytes;

        let live_changed = with_cx(|cx| live.apply(edits, seed, cx)).0.unwrap();
        assert_eq!(
            live_changed,
            seq_applied.changed,
            "step {} changed count mismatch",
            step_idx + 1
        );
        assert!(!live.take_dirty_element_spans().is_empty());

        let saved = with_cx(|cx| live.save_spf(cx)).0.unwrap();
        assert_eq!(saved, seq_bytes, "step {} SPF byte mismatch", step_idx + 1);

        let expected_building = lift_building(&seq_bytes);
        assert_eq!(
            live.building(),
            &expected_building,
            "step {} Building mismatch",
            step_idx + 1
        );
        let expected_hashes = element_hashes(&expected_building);
        assert_eq!(
            element_hashes(live.building()),
            expected_hashes,
            "step {} element hash mismatch",
            step_idx + 1
        );

        snapshots.push((seq_bytes.clone(), expected_building, expected_hashes));
    }

    assert_eq!(live.undo_len(), steps.len());
    assert_eq!(live.redo_len(), 0);

    // 全段を 1 段ずつ取り消し、各段のバイト列・Building・ハッシュ一致を確かめる
    for rev_idx in (0..steps.len()).rev() {
        assert!(with_cx(|cx| live.undo(cx)).0.unwrap());
        let (exp_bytes, exp_building, exp_hashes) = &snapshots[rev_idx];
        assert_eq!(
            &with_cx(|cx| live.save_spf(cx)).0.unwrap(),
            exp_bytes,
            "undo to step {rev_idx} byte mismatch"
        );
        assert_eq!(
            live.building(),
            exp_building,
            "undo to step {rev_idx} Building mismatch"
        );
        assert_eq!(&element_hashes(live.building()), exp_hashes);
    }
    assert!(live.is_clean());
    assert!(!with_cx(|cx| live.undo(cx)).0.unwrap());

    // 全段を 1 段ずつやり直し、各段のバイト列・Building・ハッシュ一致を確かめる
    for (fwd_idx, (exp_bytes, exp_building, exp_hashes)) in snapshots.iter().enumerate().skip(1) {
        assert!(with_cx(|cx| live.redo(cx)).0.unwrap());
        assert_eq!(
            &with_cx(|cx| live.save_spf(cx)).0.unwrap(),
            exp_bytes,
            "redo to step {fwd_idx} byte mismatch"
        );
        assert_eq!(
            live.building(),
            exp_building,
            "redo to step {fwd_idx} Building mismatch"
        );
        assert_eq!(&element_hashes(live.building()), exp_hashes);
    }
    assert!(!with_cx(|cx| live.redo(cx)).0.unwrap());
}

#[test]
fn ifczip_roundtrip_edit_and_undo_restores_exact_archive_bytes() {
    let spf = rich_sample_ifc();
    let mut zip_bytes = zumen_zip::write(&[("nested/model.ifc", spf.as_bytes())]).unwrap();
    // 元の ZIP の末尾コメント領域の違いも未編集・取り消し後にそのまま戻ることを確かめる
    let archive_check = with_cx(|cx| IfcZip::read(&zip_bytes, cx)).0.unwrap();
    assert_eq!(archive_check.name(), "nested/model.ifc");

    let mut live = with_cx(|cx| IfcLive::open(&zip_bytes, cx)).0.unwrap();
    assert!(live.is_zip());
    assert_eq!(live.zip_entry(), Some("nested/model.ifc"));
    assert!(live.is_clean());
    assert_eq!(with_cx(|cx| live.save(cx)).0.unwrap(), zip_bytes);

    let edits = [prop_edit(
        32,
        "Pset_DoorCommon",
        "FireRating",
        "IfcLabel",
        NewLiteral::Text("EI120".to_owned()),
    )];
    let _ = with_cx(|cx| live.apply(&edits, b"zip-seed", cx)).0.unwrap();
    assert!(!live.is_clean());

    let edited_zip = with_cx(|cx| live.save(cx)).0.unwrap();
    assert_ne!(edited_zip, zip_bytes);
    let reopened = with_cx(|cx| IfcZip::read(&edited_zip, cx)).0.unwrap();
    assert_eq!(reopened.name(), "nested/model.ifc");

    let orig_file = read_file(spf.as_bytes());
    let expected_spf = with_cx(|cx| apply(&orig_file, &edits, b"zip-seed", cx))
        .0
        .unwrap()
        .bytes;
    let mut reopened_spf = Vec::new();
    with_cx(|cx| Ifc::write(reopened.ifc(), &mut reopened_spf, cx))
        .0
        .unwrap();
    assert_eq!(reopened_spf, expected_spf);

    assert!(with_cx(|cx| live.undo(cx)).0.unwrap());
    assert!(live.is_clean());
    let restored_zip = with_cx(|cx| live.save(cx)).0.unwrap();
    assert_eq!(restored_zip, zip_bytes);
    zip_bytes.clear();
}

#[test]
fn crlf_and_duplicate_id_byte_for_byte_parity() {
    let crlf_src = concat!(
        "ISO-10303-21;\r\n",
        "HEADER;\r\n",
        "FILE_DESCRIPTION(('ViewDefinition [x]'),'2;1');\r\n",
        "FILE_NAME('dup.ifc','2026-01-01T00:00:00',(''),(''),'','','');\r\n",
        "FILE_SCHEMA(('IFC4'));\r\n",
        "ENDSEC;\r\n",
        "DATA;\r\n",
        "/* leading comment */\r\n",
        "#1=IFCPROJECT('0000000000000000000001',#2,'Proj',$,$,$,$,$,$);\r\n",
        "#2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);\r\n",
        "#30=IFCWALL( '0000000000000000000030' , #2 , 'First' , $ , $ , $ , $ , $ , $ );\r\n",
        "/* duplicate #30 kept verbatim */\r\n",
        "#30=IFCWALL( '0000000000000000000030' , #2 , 'Duplicate' , $ , $ , $ , $ , $ , $ );\r\n",
        "ENDSEC;\r\n",
        "END-ISO-10303-21;\r\n",
    );

    let file = read_file(crlf_src.as_bytes());
    let mut live = with_cx(|cx| IfcLive::open(crlf_src.as_bytes(), cx))
        .0
        .unwrap();

    let edits = [
        Edit::Attribute {
            element: 30,
            attribute: RootAttribute::Name,
            value: Some("UpdatedFirst".to_owned()),
        },
        prop_edit(
            30,
            "Pset_WallCommon",
            "Reference",
            "IfcIdentifier",
            NewLiteral::Text("W-1".to_owned()),
        ),
    ];

    let expected = with_cx(|cx| apply(&file, &edits, b"crlf", cx))
        .0
        .unwrap()
        .bytes;
    let _ = with_cx(|cx| live.apply(&edits, b"crlf", cx)).0.unwrap();
    let got = with_cx(|cx| live.save_spf(cx)).0.unwrap();
    assert_eq!(got, expected);

    assert!(with_cx(|cx| live.undo(cx)).0.unwrap());
    assert_eq!(
        with_cx(|cx| live.save_spf(cx)).0.unwrap(),
        crlf_src.as_bytes()
    );
}

/// セットとプロパティの名前を、開いたファイルの文字コードの指定で比べる。CP932 で読むファイルでは、
/// エスケープで書いた日本語の名前のセットとプロパティを書き換え、従来どおり読むファイルでは新しいセットを足す。
/// どちらも今までの経路（[`apply`]）とバイト列が一致し、2 回目の変更も一致する
#[test]
fn set_and_property_names_follow_the_text_encoding_like_apply() {
    // 「テスト」の CP932 のエスケープ。`\X\83` が手がかりになる
    let name = r"\X\83\X\65\X\83\X\58\X\83\X\67";
    let src = format!(
        concat!(
            "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('','',(''),(''),'','','');\n",
            "FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n",
            "#1=IFCPROJECT('{p}',#2,'P',$,$,$,$,$,$);\n",
            "#2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);\n",
            "#30=IFCWALL('{w}',#2,'W',$,$,$,$,$,.NOTDEFINED.);\n",
            "#40=IFCPROPERTYSINGLEVALUE('{n}',$,IFCLABEL('A'),$);\n",
            "#41=IFCPROPERTYSET('{s}',#2,'{n}',$,(#40));\n",
            "#42=IFCRELDEFINESBYPROPERTIES('{r}',#2,$,$,(#30),#41);\n",
            "ENDSEC;\nEND-ISO-10303-21;\n"
        ),
        p = gid(1),
        w = gid(30),
        s = gid(41),
        r = gid(42),
        n = name,
    );
    let edits = [prop_edit(
        30,
        "テスト",
        "テスト",
        "IfcLabel",
        NewLiteral::Text("B".to_owned()),
    )];
    let again = [prop_edit(
        30,
        "テスト",
        "テスト",
        "IfcLabel",
        NewLiteral::Text("C".to_owned()),
    )];
    for (encoding, shift_jis) in [
        (zumen_ifc::TextEncoding::Auto, true),
        (zumen_ifc::TextEncoding::ShiftJis, true),
        (zumen_ifc::TextEncoding::Iso8859_1, false),
    ] {
        let mut file = read_file(src.as_bytes());
        file.set_text_encoding(encoding);
        let mut live = with_cx(|cx| IfcLive::from_file(file.clone(), 1, cx))
            .0
            .unwrap();
        assert_eq!(live.text_encoding(), encoding);
        assert_eq!(live.reads_shift_jis(), shift_jis);

        let expected = with_cx(|cx| apply(&file, &edits, b"seed", cx)).0.unwrap();
        let changed = with_cx(|cx| live.apply(&edits, b"seed", cx)).0.unwrap();
        assert_eq!(changed, expected.changed, "{encoding:?}");
        let saved = with_cx(|cx| live.save_spf(cx)).0.unwrap();
        assert_eq!(saved, expected.bytes, "{encoding:?}");
        // CP932 で読むなら、元のプロパティを書き換えるだけで、インスタンスを足さない
        assert_eq!(saved.len() < src.len() + 40, shift_jis, "{encoding:?}");

        // 書き戻したファイルを読み直して当てた 2 回目とも一致する
        let mut reread = read_file(&expected.bytes);
        reread.set_text_encoding(encoding);
        let second = with_cx(|cx| apply(&reread, &again, b"seed-2", cx))
            .0
            .unwrap();
        with_cx(|cx| live.apply(&again, b"seed-2", cx)).0.unwrap();
        assert_eq!(
            with_cx(|cx| live.save_spf(cx)).0.unwrap(),
            second.bytes,
            "{encoding:?}"
        );
        // 履歴を空にしても、常駐する状態は変えない
        assert!(live.can_undo());
        live.clear_history();
        assert!(!live.can_undo());
        assert_eq!(
            with_cx(|cx| live.save_spf(cx)).0.unwrap(),
            second.bytes,
            "{encoding:?}"
        );
    }
}

#[cfg(feature = "ids")]
#[test]
fn live_ids_lookup_matches_file_ids_lookup_before_after_edit_and_undo() {
    let express_src = "SCHEMA IFC4;\n\
        TYPE IfcLabel = STRING; END_TYPE;\n\
        TYPE IfcReal = REAL; END_TYPE;\n\
        TYPE IfcLengthMeasure = REAL; END_TYPE;\n\
        ENTITY IfcRoot ABSTRACT;\n\
          GlobalId : IfcLabel;\n\
          OwnerHistory : OPTIONAL IfcRoot;\n\
          Name : OPTIONAL IfcLabel;\n\
          Description : OPTIONAL IfcLabel;\n\
        END_ENTITY;\n\
        ENTITY IfcProject SUBTYPE OF (IfcRoot);\n\
          A, B, C, D, E : OPTIONAL IfcLabel;\n\
        END_ENTITY;\n\
        ENTITY IfcDoor SUBTYPE OF (IfcRoot);\n\
          F, G, H, Tag : OPTIONAL IfcLabel;\n\
          OverallHeight : OPTIONAL IfcLengthMeasure;\n\
          I, J, K, L : OPTIONAL IfcLabel;\n\
        END_ENTITY;\n\
        END_SCHEMA;\n";
    let ifc_src = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
        FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
        #1=IFCPROJECT('0000000000000000000001',#2,'Project',$,$,$,$,$,#10);\n\
        #2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);\n\
        #10=IFCUNITASSIGNMENT((#11));\n\
        #11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
        #80=IFCDOOR('0000000000000000000080',#2,'Door-A',$,$,$,$,'D-1',2100.,$,.DOOR.,$,$);\n\
        #81=IFCDOOR('0000000000000000000081',#2,'Door-B',$,$,$,$,'D-2',2000.,$,.DOOR.,$,$);\n\
        ENDSEC;\nEND-ISO-10303-21;\n";

    let schema = with_cx(|cx| Express::read(express_src.as_bytes(), cx))
        .0
        .unwrap();
    let mut live = with_cx(|cx| IfcLive::open(ifc_src.as_bytes(), cx))
        .0
        .unwrap();

    let compare_lookup = |live: &IfcLive| {
        let saved = with_cx(|cx| live.save_spf(cx)).0.unwrap();
        let file = read_file(&saved);
        let file_building = lift_building(&saved);

        let mut live_budget = Budget::new(Limits::default());
        let mut live_diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut live_cx = Cx::new(&mut live_budget, &mut live_diag, &cancel);
        let live_lookup = live.ids_lookup(Some(&schema));
        let mut live_attrs = Vec::new();
        for i in 0..live.building().elements().len() {
            live_attrs.push(
                live_lookup
                    .attributes(live.building(), ElementRef(i as u32), &mut live_cx)
                    .unwrap(),
            );
        }

        let mut file_budget = Budget::new(Limits::default());
        let mut file_diag = Diagnostics::new();
        let mut file_cx = Cx::new(&mut file_budget, &mut file_diag, &cancel);
        let file_lookup = IdsLookup::new(&file, Some(&schema));
        let mut file_attrs = Vec::new();
        for i in 0..file_building.elements().len() {
            file_attrs.push(
                file_lookup
                    .attributes(&file_building, ElementRef(i as u32), &mut file_cx)
                    .unwrap(),
            );
        }

        assert_eq!(live_attrs, file_attrs);
        assert_eq!(live_budget.used(Kind::Work), file_budget.used(Kind::Work));
    };

    compare_lookup(&live);

    let edits = [
        Edit::Attribute {
            element: 80,
            attribute: RootAttribute::Name,
            value: Some("Door-A-Renamed".to_owned()),
        },
        Edit::Attribute {
            element: 80,
            attribute: RootAttribute::Description,
            value: Some("Entrance Door".to_owned()),
        },
        prop_edit(
            80,
            "Pset_DoorCommon",
            "FireRating",
            "IfcLabel",
            NewLiteral::Text("EI60".to_owned()),
        ),
    ];
    let _ = with_cx(|cx| live.apply(&edits, b"ids-seed", cx)).0.unwrap();
    compare_lookup(&live);

    assert!(with_cx(|cx| live.undo(cx)).0.unwrap());
    compare_lookup(&live);
}

#[test]
fn public_ifc_samples_open_threads_edit_undo_redo_parity() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let samples_dir = manifest_dir.join("../../samples/ifc");
    let mut paths = Vec::new();
    collect_ifc_samples(&samples_dir, &mut paths);
    assert!(!paths.is_empty(), "samples/ifc must contain sample files");

    for path in paths {
        let bytes = std::fs::read(&path).unwrap();
        let mut live_1 = with_cx(|cx| IfcLive::open_with_threads(&bytes, 1, cx))
            .0
            .unwrap();
        let live_4 = with_cx(|cx| IfcLive::open_with_threads(&bytes, 4, cx))
            .0
            .unwrap();

        assert_eq!(
            live_1.building(),
            live_4.building(),
            "1-thread vs 4-thread Building mismatch on {}",
            path.display()
        );
        let clean_1 = with_cx(|cx| live_1.save(cx)).0.unwrap();
        let clean_4 = with_cx(|cx| live_4.save(cx)).0.unwrap();
        assert_eq!(clean_1, bytes, "clean save mismatch on {}", path.display());
        assert_eq!(
            clean_4,
            bytes,
            "4-thread clean save mismatch on {}",
            path.display()
        );

        let target_inst = live_1
            .building()
            .elements()
            .iter()
            .enumerate()
            .find_map(|(idx, el)| {
                let eref = ElementRef(idx as u32);
                let cls = live_1.building().components.class.get(eref)?;
                if cls.name == "IfcMaterial" || cls.name == "IfcGridAxis" {
                    return None;
                }
                el.source().instance
            });
        let Some(inst) = target_inst else {
            continue;
        };
        assert!(live_1.element_by_instance(inst).is_some());

        let edits = [
            Edit::Attribute {
                element: inst,
                attribute: RootAttribute::Description,
                value: Some("LiveEdited".to_owned()),
            },
            prop_edit(
                inst,
                "Pset_ZumenLiveTest",
                "Checked",
                "IfcBoolean",
                NewLiteral::Boolean(true),
            ),
        ];

        let base_file = read_file(&bytes);
        let seq = with_cx(|cx| apply(&base_file, &edits, b"sample-seed", cx))
            .0
            .unwrap();
        let changed = with_cx(|cx| live_1.apply(&edits, b"sample-seed", cx))
            .0
            .unwrap();
        assert_eq!(
            changed,
            seq.changed,
            "changed mismatch on {}",
            path.display()
        );

        let saved_spf = with_cx(|cx| live_1.save_spf(cx)).0.unwrap();
        assert_eq!(saved_spf, seq.bytes, "SPF mismatch on {}", path.display());
        assert_eq!(
            live_1.building(),
            &lift_building(&seq.bytes),
            "Building mismatch after edit on {}",
            path.display()
        );

        assert!(with_cx(|cx| live_1.undo(cx)).0.unwrap());
        assert!(live_1.is_clean());
        assert_eq!(
            with_cx(|cx| live_1.save(cx)).0.unwrap(),
            bytes,
            "undo save mismatch on {}",
            path.display()
        );
        assert_eq!(
            live_1.building(),
            live_4.building(),
            "undo Building mismatch on {}",
            path.display()
        );

        assert!(with_cx(|cx| live_1.redo(cx)).0.unwrap());
        assert_eq!(
            with_cx(|cx| live_1.save_spf(cx)).0.unwrap(),
            seq.bytes,
            "redo SPF mismatch on {}",
            path.display()
        );
    }
}

#[test]
fn refused_edits_and_cx_budget_or_cancel_leave_live_untouched() {
    let src = rich_sample_ifc();
    let bytes = src.as_bytes();
    let file = read_file(bytes);
    let mut live = with_cx(|cx| IfcLive::open(bytes, cx)).0.unwrap();
    let rev_before = live.revision();
    let building_before = live.building().clone();

    // 1. 妥当な変更と不当な変更が混ざったバッチは全体を拒否し、診断コードも一致する
    let bad_batch = [
        prop_edit(
            32,
            "Pset_DoorCommon",
            "FireRating",
            "IfcLabel",
            NewLiteral::Text("EI90".to_owned()),
        ),
        prop_edit(
            999_999,
            "Pset_WallCommon",
            "FireRating",
            "IfcLabel",
            NewLiteral::Text("X".to_owned()),
        ),
    ];
    let (seq_res, seq_diag) = with_cx(|cx| apply(&file, &bad_batch, b"s", cx));
    let (live_res, live_diag) = with_cx(|cx| live.apply(&bad_batch, b"s", cx));
    assert_eq!(seq_res.unwrap_err(), Error::Invalid);
    assert_eq!(live_res.unwrap_err(), Error::Invalid);
    let seq_codes: Vec<Code> = seq_diag.iter().map(|d| d.code()).collect();
    let live_codes: Vec<Code> = live_diag.iter().map(|d| d.code()).collect();
    assert_eq!(live_codes, seq_codes);
    assert_eq!(live.revision(), rev_before);
    assert_eq!(live.building(), &building_before);
    assert!(live.is_clean());

    // 2. 有限の Work 予算超過で止まっても状態は 1 ビットも変わらない
    let valid_batch = [
        prop_edit(
            30,
            "Pset_WallCommon",
            "FireRating",
            "IfcLabel",
            NewLiteral::Text("EI180".to_owned()),
        ),
        prop_edit(
            33,
            "Pset_SlabCommon",
            "Thickness",
            "IfcPositiveLengthMeasure",
            NewLiteral::Real(240.0),
        ),
    ];
    for max_work in [0u64, 1, 2, 4] {
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(max_work)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let res = live.apply(
            &valid_batch,
            b"w",
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        );
        assert!(matches!(res, Err(Error::Stopped(Stop::Exceeded(_)))));
        assert_eq!(live.revision(), rev_before);
        assert_eq!(live.building(), &building_before);
        assert!(live.is_clean());
    }

    // 3. CancelToken による中断でも状態は変わらない
    {
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        cancel.cancel();
        let res = live.apply(
            &valid_batch,
            b"c",
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        );
        assert_eq!(res, Err(Error::Stopped(Stop::Cancelled)));
        assert_eq!(live.revision(), rev_before);
        assert_eq!(live.building(), &building_before);
    }

    // 4. 正常適用後の undo でも、Work 予算 0 なら取り消されず現在の状態を保つ
    let _ = with_cx(|cx| live.apply(&valid_batch, b"ok", cx)).0.unwrap();
    let rev_after = live.revision();
    let building_after = live.building().clone();
    {
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let res = live.undo(&mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(matches!(res, Err(Error::Stopped(Stop::Exceeded(_)))));
        assert_eq!(live.revision(), rev_after);
        assert_eq!(live.building(), &building_after);
    }
    assert!(with_cx(|cx| live.undo(cx)).0.unwrap());
    assert_eq!(live.building(), &building_before);
    assert_eq!(with_cx(|cx| live.save_spf(cx)).0.unwrap(), bytes);
}

#[test]
fn dep_graph_tracks_spatial_type_opening_material_and_property_element_refs() {
    let lines = [
        format!("#1=IFCPROJECT('{}',#2,'Proj',$,$,$,$,$,#10);", gid(1)),
        "#2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);".to_owned(),
        "#10=IFCUNITASSIGNMENT((#11));".to_owned(),
        "#11=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);".to_owned(),
        format!(
            "#20=IFCBUILDINGSTOREY('{}',#2,'1F',$,$,$,$,$,.ELEMENT.,0.);",
            gid(20)
        ),
        format!(
            "#30=IFCWALL('{}',#2,'HostWall',$,$,#90,$,$,.STANDARD.);",
            gid(30)
        ),
        format!(
            "#31=IFCOPENINGELEMENT('{}',#2,'Void1',$,$,#91,$,$,.OPENING.);",
            gid(31)
        ),
        format!(
            "#32=IFCDOOR('{}',#2,'Door1',$,$,#92,$,$,2000.,900.,$,$,$);",
            gid(32)
        ),
        format!(
            "#34=IFCDOORTYPE('{}',#2,'DoorTypeA',$,$,$,$,$,$,.DOOR.,.SINGLE_SWING_LEFT.,.F.,$);",
            gid(34)
        ),
        format!(
            "#35=IFCSPACE('{}',#2,'Room101',$,$,$,$,$,.ELEMENT.,.INTERNAL.,$);",
            gid(35)
        ),
        "#80=IFCCARTESIANPOINT((0.,0.,0.));".to_owned(),
        "#81=IFCAXIS2PLACEMENT3D(#80,$,$);".to_owned(),
        "#90=IFCLOCALPLACEMENT($,#81);".to_owned(),
        "#91=IFCLOCALPLACEMENT(#90,#81);".to_owned(),
        "#92=IFCLOCALPLACEMENT(#91,#81);".to_owned(),
        format!(
            "#100=IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',#2,$,$,(#30,#32,#35),#20);",
            gid(100)
        ),
        format!("#101=IFCRELVOIDSELEMENT('{}',#2,$,$,#30,#31);", gid(101)),
        format!("#102=IFCRELFILLSELEMENT('{}',#2,$,$,#31,#32);", gid(102)),
        format!("#103=IFCRELDEFINESBYTYPE('{}',#2,$,$,(#32),#34);", gid(103)),
        // #35 (Room101) のプロパティから #30 (HostWall) を要素参照し、後で #32 (Door1) に書き換えて undo/redo を検証する
        "#110=IFCPROPERTYSINGLEVALUE('BoundingRef',$,#30,$);".to_owned(),
        format!(
            "#111=IFCPROPERTYSET('{}',#2,'Pset_CustomLink',$,(#110));",
            gid(111)
        ),
        format!(
            "#112=IFCRELDEFINESBYPROPERTIES('{}',#2,$,$,(#35),#111);",
            gid(112)
        ),
    ];
    let spf = format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');\nFILE_NAME('dep.ifc','2026-01-01T00:00:00',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
        lines.join("\n")
    );
    let mut live = with_cx(|cx| IfcLive::open(spf.as_bytes(), cx)).0.unwrap();

    let storey = live.element_by_instance(20).unwrap();
    let wall = live.element_by_instance(30).unwrap();
    let opening = live.element_by_instance(31).unwrap();
    let door = live.element_by_instance(32).unwrap();
    let door_type = live.element_by_instance(34).unwrap();
    let space = live.element_by_instance(35).unwrap();

    // 1. 階 (#20) の変更 -> 壁 (#30)・開口 (#31, 壁との相対配置経由)・扉 (#32)・室 (#35) に波及する
    let from_storey = with_cx(|cx| live.affected_elements(&[storey], cx))
        .0
        .unwrap();
    assert!(from_storey.contains(&wall));
    assert!(from_storey.contains(&opening));
    assert!(from_storey.contains(&door));
    assert!(from_storey.contains(&space));

    // 2. 型 (#34) の変更 -> 扉 (#32) に波及する
    let from_type = with_cx(|cx| live.affected_elements(&[door_type], cx))
        .0
        .unwrap();
    assert_eq!(from_type, vec![door]);

    // 3. 開口 (#31) の変更 -> 穴をあける壁 (#30) と充填する扉 (#32)、および壁を参照する室 (#35) に波及する
    let from_opening = with_cx(|cx| live.affected_elements(&[opening], cx))
        .0
        .unwrap();
    assert!(from_opening.contains(&wall));
    assert!(from_opening.contains(&door));
    assert!(from_opening.contains(&space));

    // 4. 室 (#35) の BoundingRef プロパティをスカラ文字列に書き換えると、壁 (#30) -> 室 (#35) のプロパティ依存辺が外れ、undo/redo で戻る
    let edit_prop = [prop_edit(
        35,
        "Pset_CustomLink",
        "BoundingRef",
        "IfcLabel",
        NewLiteral::Text("NONE".to_owned()),
    )];
    let _ = with_cx(|cx| live.apply(&edit_prop, b"dep", cx)).0.unwrap();
    let from_door_type_after = with_cx(|cx| live.affected_elements(&[door_type], cx))
        .0
        .unwrap();
    assert_eq!(from_door_type_after, vec![door]);
    let space_srcs_after = live.dep_graph().sources_of(zumen_live::RowId(space.0));
    assert!(!space_srcs_after.contains(&zumen_live::RowId(wall.0)));

    assert!(with_cx(|cx| live.undo(cx)).0.unwrap());
    let space_srcs_undo = live.dep_graph().sources_of(zumen_live::RowId(space.0));
    assert!(space_srcs_undo.contains(&zumen_live::RowId(wall.0)));

    assert!(with_cx(|cx| live.redo(cx)).0.unwrap());
    let space_srcs_redo = live.dep_graph().sources_of(zumen_live::RowId(space.0));
    assert!(!space_srcs_redo.contains(&zumen_live::RowId(wall.0)));
}
