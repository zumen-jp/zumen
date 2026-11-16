//! 8 ビットのエスケープの読み分け・元バイト・編集・環境間の一致（#540）。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, ElementRef, Hash, Literal};
use zumen_ifc::{Edit, Ifc, IfcFile, NewLiteral, NewValue, TextEncoding, apply};
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
const RAW: &[&str] = &[
    r"\X\83\X\65\X\83\X\58\X\83\X\67",
    r"\X\8D\S\^\X\97\S\?",
    r"\X\95\X2\00C7\X0\",
    r"\X\A6",
    r"\X\F0\X\40",
    r"\X\B0",
    r"\X\81\X\30",
    r"\X\83\X2\65E5\X0\",
    r"\X2\65E5672C\X0\",
    r"\X4\0001F3E0\X0\",
    "日本ä",
    r"\X\95\\",
    r"\X\83\X\65''\\",
];
const READ: &[&str] = &[
    "テスト",
    "材料",
    "壁",
    "ｦ",
    "\u{E000}",
    "ｰ",
    "\u{81}0",
    "\u{83}日",
    "日本",
    "🏠",
    "日本ä",
    "表",
    "テ'\\",
];

fn sample(version: &str, names: &[&str], extra: &str) -> String {
    let mut src = format!(
        "ISO-10303-21;HEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('{version}'));ENDSEC;DATA;\n"
    );
    for (i, name) in names.iter().enumerate() {
        let id = i + 1;
        let end = if version == "IFC2X3" { "" } else { ",$" };
        src.push_str(&format!(
            "#{id}=IFCWALL('{id:022}',$,'{name}',$,$,$,$,${end});\n"
        ));
    }
    src.push_str(extra);
    src.push_str("ENDSEC;END-ISO-10303-21;\n");
    src
}

fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics, Budget) {
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (result, diagnostics, budget)
}

fn read(src: &str) -> IfcFile {
    with(|cx| Ifc::read(src.as_bytes(), cx)).0.unwrap()
}

fn names(building: &Building) -> Vec<&str> {
    (0..building.elements().len())
        .map(|i| {
            building
                .components
                .identity
                .get(ElementRef(i as u32))
                .unwrap()
                .name
                .as_deref()
                .unwrap()
        })
        .collect()
}

#[test]
fn mixed_escapes_use_one_file_policy_and_keep_source() {
    for version in VERSIONS {
        let src = sample(version, RAW, "");
        let file = read(&src);
        let (building, diagnostics, _) = with(|cx| Lift::<Building>::lift(&file, cx));
        assert_eq!(names(&building.unwrap()), READ);
        let repaired: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code() == codes::IFC_TEXT_REPAIRED)
            .collect();
        assert_eq!(repaired.iter().map(|d| d.count()).sum::<u64>(), 8);
        assert!(repaired.iter().all(|d| !d.places().is_empty()));
        assert!(
            !diagnostics
                .iter()
                .any(|d| d.code() == codes::IFC_TEXT_INVALID)
        );
        let mut out = Vec::new();
        with(|cx| Ifc::write(&file, &mut out, cx)).0.unwrap();
        assert_eq!(out, src.as_bytes());
        assert_eq!(file, read(&src));
    }
}

#[test]
fn explicit_iso_restores_latin_and_preserves_ids() {
    let mut file = read(&sample(
        "IFC4",
        &[RAW[0], r"\X\B0", r"\X\D7", r"\X2\00E4\X0\"],
        "",
    ));
    let auto: Building = with(|cx| file.lift(cx)).0.unwrap();
    assert_eq!(names(&auto)[1..], ["ｰ", "ﾗ", "ä"]);
    file.set_text_encoding(TextEncoding::Iso8859_1);
    assert_eq!(file.text_encoding(), TextEncoding::Iso8859_1);
    let (iso, diagnostics, _) = with(|cx| Lift::<Building>::lift(&file, cx));
    let iso = iso.unwrap();
    assert_eq!(names(&iso)[1..], ["°", "×", "ä"]);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.code() == codes::IFC_TEXT_REPAIRED)
    );
    for i in 0..auto.elements().len() {
        assert_eq!(
            auto.element(ElementRef(i as u32)).unwrap().id(),
            iso.element(ElementRef(i as u32)).unwrap().id()
        );
    }
}

#[test]
fn absent_or_quoted_hint_does_not_select_shift_jis() {
    for clue in ["", r"/* '\X\83' */", r"#90=UNKNOWN('\\X\\83');"] {
        let src = sample("IFC4", &[r"\X\B0", r"\X\D7", r"\X2\00E4\X0\"], clue);
        let mut file = read(&src);
        let (building, diagnostics, _) = with(|cx| Lift::<Building>::lift(&file, cx));
        assert_eq!(names(&building.unwrap()), ["°", "×", "ä"]);
        assert!(
            !diagnostics
                .iter()
                .any(|d| d.code() == codes::IFC_TEXT_REPAIRED)
        );
        file.set_text_encoding(TextEncoding::ShiftJis);
        let building: Building = with(|cx| file.lift(cx)).0.unwrap();
        assert_eq!(names(&building), ["ｰ", "ﾗ", "ä"]);
    }
}

#[test]
fn hint_in_late_nested_value_applies_to_earlier_strings() {
    let src = sample(
        "IFC4",
        &[r"\S\&", r"\X2\00F00040\X0\"],
        r"#90=UNKNOWN(((IFCLABEL('\X\83\X\65'))));",
    );
    let file = read(&src);
    let (building, diagnostics, _) = with(|cx| Lift::<Building>::lift(&file, cx));
    assert_eq!(names(&building.unwrap()), ["ｦ", "\u{E000}"]);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|d| d.code() == codes::IFC_TEXT_REPAIRED)
            .map(|d| d.count())
            .sum::<u64>(),
        3
    );
}

#[test]
fn explicit_unicode_and_incomplete_escapes_keep_standard_decoding() {
    let file = read(&sample(
        "IFC4",
        &[
            RAW[0],
            r"\X4\000000A6\X0\",
            r"\\X\\83",
            r"\X\83\X2\0100\X0\",
            r"\X\95\X\zz",
        ],
        "",
    ));
    let building: Building = with(|cx| file.lift(cx)).0.unwrap();
    assert_eq!(
        names(&building),
        ["テスト", "¦", r"\X\83", "\u{83}Ā", "\u{95}\\X\\zz"]
    );
}

#[test]
fn geometry_getters_share_file_encoding_without_requiring_lift() {
    let mut file = read(&sample(
        "IFC4",
        &[RAW[0]],
        r"#20=IFCGEOMETRICREPRESENTATIONCONTEXT('\X\A6','Model',3,$,#21,$);#21=IFCAXIS2PLACEMENT3D(#22,$,$);#22=IFCCARTESIANPOINT((0.,0.,0.));",
    ));
    let auto = with(|cx| file.representation_context(20, cx)).0.unwrap();
    assert_eq!(auto.definition.geometric.identifier.as_deref(), Some("ｦ"));
    file.set_text_encoding(TextEncoding::Iso8859_1);
    let iso = with(|cx| file.representation_context(20, cx)).0.unwrap();
    assert_eq!(iso.definition.geometric.identifier.as_deref(), Some("¦"));
}

#[test]
fn property_edit_matches_decoded_names_and_keeps_other_records() {
    let src = sample(
        "IFC4",
        &[RAW[2]],
        &format!(
            "#10=IFCPROPERTYSINGLEVALUE('{}',$,IFCLABEL('old'),$);\n#11=IFCPROPERTYSET('0000000000000000000011',$,'{}',$,(#10));\n#12=IFCRELDEFINESBYPROPERTIES('0000000000000000000012',$,$,$,(#1),#11);\n",
            RAW[2], RAW[1]
        ),
    );
    let file = read(&src);
    let result = with(|cx| {
        apply(
            &file,
            &[Edit::Property {
                element: 1,
                set: "材料".to_owned(),
                name: "壁".to_owned(),
                value: NewValue {
                    type_name: "IfcLabel".to_owned(),
                    literal: NewLiteral::Text("更新".to_owned()),
                },
            }],
            b"text",
            cx,
        )
    })
    .0
    .unwrap();
    let updated = with(|cx| Ifc::read(&result.bytes, cx)).0.unwrap();
    assert_eq!(updated.records().len(), file.records().len());
    for id in [1, 11, 12] {
        assert_eq!(
            file.bytes(file.instance(id).unwrap().span()),
            updated.bytes(updated.instance(id).unwrap().span())
        );
    }
    assert!(String::from_utf8_lossy(&result.bytes).contains(r"\X2\66F465B0\X0\"));
    let building: Building = with(|cx| updated.lift(cx)).0.unwrap();
    let set = &building
        .components
        .properties
        .get(ElementRef(0))
        .unwrap()
        .sets[0];
    assert_eq!(set.name, "材料");
    assert_eq!(set.properties.len(), 1);
    assert_eq!(
        set.properties[0].value.as_ref().unwrap().value,
        Literal::Text("更新".to_owned())
    );
}

#[test]
#[allow(clippy::disallowed_methods)] // 比較先の指定は検査器だけが読む。
fn encoding_contract_is_bounded_and_exported() {
    let mut contract = String::new();
    for version in VERSIONS {
        for encoding in [
            TextEncoding::Auto,
            TextEncoding::Iso8859_1,
            TextEncoding::ShiftJis,
        ] {
            let src = sample(version, RAW, "");
            let (result, read_diag, read_budget) = with(|cx| Ifc::read(src.as_bytes(), cx));
            let mut file = result.unwrap();
            file.set_text_encoding(encoding);
            let (result, diag, budget) = with(|cx| Lift::<Building>::lift(&file, cx));
            let building = result.unwrap();
            contract.push_str(&format!(
                "{version} {encoding:?}\n{:?}\n{read_diag:?}\n{diag:?}\n",
                names(&building)
            ));
            for i in 0..building.elements().len() {
                let element = building.element(ElementRef(i as u32)).unwrap();
                contract.push_str(&format!(
                    "{:?} {:?}\n",
                    element.id(),
                    Hash::element(&building, ElementRef(i as u32))
                ));
            }
            for kind in [
                Kind::InputBytes,
                Kind::DecodedBytes,
                Kind::Records,
                Kind::Elements,
                Kind::Depth,
                Kind::Segments,
                Kind::Work,
            ] {
                contract.push_str(&format!(
                    "{kind:?} {} {}\n",
                    read_budget.used(kind),
                    budget.used(kind)
                ));
            }
            for (kind, amount) in [(Kind::Work, 0), (Kind::DecodedBytes, 0)] {
                if kind == Kind::DecodedBytes && encoding == TextEncoding::Iso8859_1 {
                    continue;
                }
                let mut limits = Limits::default();
                limits.set(kind, Limit::Finite(amount));
                let mut stopped_budget = Budget::new(limits);
                let mut diagnostics = Diagnostics::new();
                let cancel = CancelToken::new();
                let result = Lift::<Building>::lift(
                    &file,
                    &mut Cx::new(&mut stopped_budget, &mut diagnostics, &cancel),
                );
                assert!(matches!(result, Err(Error::Stopped(_))));
                contract.push_str(&format!("stop {kind:?}: {diagnostics:?}\n"));
            }
        }
    }
    if let Some(dir) = std::env::var_os("ZUMEN_IFC_TEXT_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join("text.txt"), contract).unwrap();
    }
}

/// #513 の旧形式 4 件をリポジトリ外から受け取ったときだけ流す。名前や属性の値は出力しない。
#[test]
#[allow(clippy::disallowed_methods, clippy::print_stdout)] // 手元の入力で測った件数だけを出す検査器。
fn locally_supplied_examples_restore_names_without_exporting_values() {
    let Some(root) = std::env::var_os("ZUMEN_SAMPLES") else {
        return;
    };
    for id in ["B01", "B02", "B03", "B04"] {
        let src = std::fs::read(std::path::Path::new(&root).join(format!("{id}.ifc"))).unwrap();
        let mut file = with(|cx| Ifc::read(&src, cx)).0.unwrap();
        let (auto, diagnostics, _) = with(|cx| Lift::<Building>::lift(&file, cx));
        let auto = auto.unwrap();
        file.set_text_encoding(TextEncoding::Iso8859_1);
        let iso: Building = with(|cx| file.lift(cx)).0.unwrap();
        assert!(auto.elements().len() == iso.elements().len());
        let mut changed = 0;
        let mut controls_before = 0;
        let mut controls_after = 0;
        for (at, a) in auto.components.identity.iter() {
            let b = iso.components.identity.get(at).unwrap();
            changed += u64::from(a.name != b.name);
            controls_before += u64::from(
                b.name
                    .as_deref()
                    .is_some_and(|s| s.chars().any(|c| ('\u{80}'..='\u{9f}').contains(&c))),
            );
            controls_after += u64::from(
                a.name
                    .as_deref()
                    .is_some_and(|s| s.chars().any(|c| ('\u{80}'..='\u{9f}').contains(&c))),
            );
            assert!(auto.element(at).unwrap().id() == iso.element(at).unwrap().id());
        }
        assert!(changed > 0 && controls_before > 0 && controls_after == 0);
        let repaired = diagnostics
            .iter()
            .filter(|d| d.code() == codes::IFC_TEXT_REPAIRED)
            .map(|d| d.count())
            .sum::<u64>();
        println!(
            "{id} repaired={repaired} changed-names={changed} c1-before={controls_before} c1-after={controls_after}"
        );
    }
}
