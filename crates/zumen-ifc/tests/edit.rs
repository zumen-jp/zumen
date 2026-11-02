//! IFC の L1 の変更（決定 0207・0244、#393）。ファイルは、どれも試験の中で作る。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_building::{Building, ElementRef, GlobalId, Id, Literal};
use zumen_ifc::{Edit, Ifc, IfcFile, NewLiteral, NewValue, RootAttribute, apply};
use zumen_sdk::{Codec, Error, Lift};

fn gid(n: u32) -> String {
    format!("{n:0>22}")
}

/// 壁 2 つがセットを共有し、扉は自分だけのセットを持ち、扉の型は型のセットを持つ IFC4
fn sample() -> String {
    let lines = [
        format!("#1=IFCPROJECT('{}',#2,'Project',$,$,$,$,$,$);", gid(1)),
        "#2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);".to_owned(),
        format!("#30=IFCWALL('{}',#2,'Wall A',$,$,$,$,$,$);", gid(30)),
        format!("#31=IFCWALL('{}',#2,'Wall B',$,$,$,$,$,$);", gid(31)),
        format!("#32=IFCDOOR('{}',#2,'Door',$,$,$,$,$,$,$,$,$,$);", gid(32)),
        format!("#33=IFCSLAB('{}',#2,'Slab',$,$,$,$,$,$);", gid(33)),
        format!(
            "#34=IFCDOORTYPE('{}',#2,'Door type',$,$,(#60),$,$,$,.DOOR.,$,$,$);",
            gid(34)
        ),
        "#40=IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('EI30'),$);".to_owned(),
        "#41=IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.F.),$);".to_owned(),
        format!(
            "#42=IFCPROPERTYSET('{}',#2,'Pset_WallCommon',$,(#40,#41));",
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
        "#59=IFCPROPERTYSINGLEVALUE('Reference',$,IFCIDENTIFIER('D1'),$);".to_owned(),
        format!(
            "#60=IFCPROPERTYSET('{}',#2,'Pset_DoorTypeCommon',$,(#59));",
            gid(60)
        ),
        "#70=IFCMATERIAL('Concrete',$,$);".to_owned(),
    ];
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [x]'),'2;1');\nFILE_NAME('a.ifc','2026-01-01T00:00:00',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
        lines.join("\n")
    )
}

fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn read(src: &[u8]) -> Option<IfcFile> {
    with(|cx| Ifc::read(src, cx)).0.ok()
}

fn lift(src: &[u8]) -> Option<Building> {
    let file = read(src)?;
    with(|cx| file.lift(cx)).0.ok()
}

fn run(src: &[u8], edits: &[Edit]) -> (Result<Vec<u8>, Error>, Vec<Code>) {
    let Some(file) = read(src) else {
        return (Err(Error::Invalid), Vec::new());
    };
    let (result, diag) = with(|cx| apply(&file, edits, b"seed", cx));
    (
        result.map(|applied| applied.bytes),
        diag.iter().map(|d| d.code()).collect(),
    )
}

fn element(building: &Building, n: u32) -> Option<ElementRef> {
    building.find(Id::Global(GlobalId::parse(&gid(n))?))
}

/// 要素のセット `set` のプロパティ `name` の値
fn value(building: &Building, n: u32, set: &str, name: &str) -> Option<Literal> {
    let sets = &building
        .components
        .properties
        .get(element(building, n)?)?
        .sets;
    let set = sets.iter().find(|s| s.name == set)?;
    let property = set.properties.iter().find(|p| p.name == name)?;
    Some(property.value.as_ref()?.value.clone())
}

fn label(text: &str) -> NewValue {
    NewValue {
        type_name: "IfcLabel".to_owned(),
        literal: NewLiteral::Text(text.to_owned()),
    }
}

fn property(element: u64, set: &str, name: &str, value: NewValue) -> Edit {
    Edit::Property {
        element,
        set: set.to_owned(),
        name: name.to_owned(),
        value,
    }
}

/// 行ごとに比べ、違う行を返す
fn changed_lines(before: &str, after: &str) -> Vec<String> {
    let before: Vec<&str> = before.lines().collect();
    after
        .lines()
        .filter(|line| !before.contains(line))
        .map(str::to_owned)
        .collect()
}

#[test]
fn changes_only_the_property_of_an_exclusive_set() {
    let src = sample();
    let (bytes, codes) = run(
        src.as_bytes(),
        &[property(32, "Pset_DoorCommon", "FireRating", label("EI90"))],
    );
    let bytes = bytes.unwrap();
    assert!(codes.is_empty(), "{codes:?}");
    let after = String::from_utf8(bytes.clone()).unwrap();
    assert_eq!(
        changed_lines(&src, &after),
        ["#50=IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('EI90'),$);"]
    );
    // 触れない行は、バイト単位で同じ
    assert_eq!(after.replace("IFCLABEL('EI90')", "IFCLABEL('EI60')"), src);
    let building = lift(&bytes).unwrap();
    assert_eq!(
        value(&building, 32, "Pset_DoorCommon", "FireRating"),
        Some(Literal::Text("EI90".to_owned()))
    );
}

#[test]
fn copies_a_shared_set_and_keeps_the_other_element() {
    let src = sample();
    let (bytes, codes) = run(
        src.as_bytes(),
        &[property(
            30,
            "Pset_WallCommon",
            "FireRating",
            label("EI120"),
        )],
    );
    assert!(codes.is_empty(), "{codes:?}");
    let bytes = bytes.unwrap();
    let building = lift(&bytes).unwrap();
    assert_eq!(
        value(&building, 30, "Pset_WallCommon", "FireRating"),
        Some(Literal::Text("EI120".to_owned()))
    );
    // 共有していた壁 B の値と、写したセットのほかのプロパティは変わらない
    assert_eq!(
        value(&building, 31, "Pset_WallCommon", "FireRating"),
        Some(Literal::Text("EI30".to_owned()))
    );
    assert_eq!(
        value(&building, 30, "Pset_WallCommon", "IsExternal"),
        Some(Literal::Boolean(false))
    );
    let after = String::from_utf8(bytes).unwrap();
    let changed = changed_lines(&src, &after);
    // 関係から壁 A を外し、写したセット・新しいプロパティ・新しい関係を足す
    assert!(changed.contains(&format!(
        "#43=IFCRELDEFINESBYPROPERTIES('{}',#2,$,$,(#31),#42);",
        gid(43)
    )));
    assert_eq!(changed.len(), 4, "{changed:?}");
    assert!(changed.iter().any(|l| l.starts_with("#71=IFCPROPERTYSET(")));
}

#[test]
fn adds_a_set_and_a_property() {
    let src = sample();
    let edits = [
        property(33, "Pset_SlabCommon", "Reference", label("S1")),
        property(
            33,
            "Pset_SlabCommon",
            "Thickness",
            NewValue {
                type_name: "IfcPositiveLengthMeasure".to_owned(),
                literal: NewLiteral::Real(200.0),
            },
        ),
        property(
            32,
            "Pset_DoorCommon",
            "HandicapAccessible",
            NewValue {
                type_name: "IfcBoolean".to_owned(),
                literal: NewLiteral::Boolean(true),
            },
        ),
    ];
    let (bytes, codes) = run(src.as_bytes(), &edits);
    assert!(codes.is_empty(), "{codes:?}");
    let bytes = bytes.unwrap();
    let after = String::from_utf8(bytes.clone()).unwrap();
    assert!(after.contains("IFCPOSITIVELENGTHMEASURE(200.)"));
    // 足したものは、データの節の終わりの前
    assert!(after.ends_with("\nENDSEC;\nEND-ISO-10303-21;\n"));
    let building = lift(&bytes).unwrap();
    assert_eq!(
        value(&building, 33, "Pset_SlabCommon", "Reference"),
        Some(Literal::Text("S1".to_owned()))
    );
    assert_eq!(
        value(&building, 33, "Pset_SlabCommon", "Thickness"),
        Some(Literal::Real(200.0))
    );
    assert_eq!(
        value(&building, 32, "Pset_DoorCommon", "HandicapAccessible"),
        Some(Literal::Boolean(true))
    );
    // 同じ入力と変更なら、同じファイルになる
    assert_eq!(run(src.as_bytes(), &edits).0.unwrap(), bytes);
}

#[test]
fn changes_the_set_of_a_type() {
    let src = sample();
    let (bytes, codes) = run(
        src.as_bytes(),
        &[
            property(34, "Pset_DoorTypeCommon", "Reference", label("D2")),
            property(34, "Pset_Extra", "Note", label("x")),
        ],
    );
    assert!(codes.is_empty(), "{codes:?}");
    let building = lift(&bytes.unwrap()).unwrap();
    assert_eq!(
        value(&building, 34, "Pset_DoorTypeCommon", "Reference"),
        Some(Literal::Text("D2".to_owned()))
    );
    assert_eq!(
        value(&building, 34, "Pset_Extra", "Note"),
        Some(Literal::Text("x".to_owned()))
    );
}

#[test]
fn changes_the_name_and_writes_any_character() {
    let src = sample();
    let name = "壁 'A' \\ 🧱";
    let (bytes, codes) = run(
        src.as_bytes(),
        &[
            Edit::Attribute {
                element: 30,
                attribute: RootAttribute::Name,
                value: Some(name.to_owned()),
            },
            Edit::Attribute {
                element: 30,
                attribute: RootAttribute::Description,
                value: Some("説明".to_owned()),
            },
            Edit::Attribute {
                element: 31,
                attribute: RootAttribute::Name,
                value: None,
            },
        ],
    );
    assert!(codes.is_empty(), "{codes:?}");
    let building = lift(&bytes.unwrap()).unwrap();
    let identity = |n| {
        building
            .components
            .identity
            .get(element(&building, n).unwrap())
    };
    assert_eq!(identity(30).unwrap().name.as_deref(), Some(name));
    assert_eq!(identity(30).unwrap().description.as_deref(), Some("説明"));
    assert_eq!(identity(31).and_then(|i| i.name.clone()), None);
}

#[test]
fn rejects_what_it_cannot_write() {
    let src = sample();
    let cases = [
        // GlobalId を持たない材料
        property(70, "Pset_MaterialCommon", "Porosity", label("x")),
        // ないインスタンス
        property(999, "Pset_WallCommon", "FireRating", label("x")),
        // 型の名前が書けない
        property(
            30,
            "Pset_WallCommon",
            "FireRating",
            NewValue {
                type_name: "Ifc Label".to_owned(),
                literal: NewLiteral::Text("x".to_owned()),
            },
        ),
        property(
            30,
            "Pset_WallCommon",
            "FireRating",
            NewValue {
                type_name: "IfcReal".to_owned(),
                literal: NewLiteral::Real(f64::NAN),
            },
        ),
    ];
    for edit in cases {
        let (bytes, codes) = run(
            src.as_bytes(),
            &[
                property(32, "Pset_DoorCommon", "FireRating", label("ok")),
                edit,
            ],
        );
        assert_eq!(bytes, Err(Error::Invalid));
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED]);
    }
}

#[test]
fn truncated_files_do_not_panic() {
    let src = sample();
    for end in (0..src.len()).step_by(7) {
        let Some(part) = src.get(..end) else {
            continue;
        };
        let Ok(file) = with(|cx| Ifc::read(part.as_bytes(), cx)).0 else {
            continue;
        };
        let _ = with(|cx| {
            apply(
                &file,
                &[property(30, "Pset_WallCommon", "FireRating", label("x"))],
                b"seed",
                cx,
            )
        });
    }
}

/// `sample` と同じ要素を、IFC2X3 の属性の数で書いたもの。扉の型は `IfcDoorStyle`
fn sample_2x3() -> String {
    sample()
        .replace("FILE_SCHEMA(('IFC4'))", "FILE_SCHEMA(('IFC2X3'))")
        .replace("'Wall A',$,$,$,$,$,$);", "'Wall A',$,$,$,$,$);")
        .replace("'Wall B',$,$,$,$,$,$);", "'Wall B',$,$,$,$,$);")
        .replace("'Door',$,$,$,$,$,$,$,$,$,$);", "'Door',$,$,$,$,$,$,$);")
        .replace("#34=IFCDOORTYPE(", "#34=IFCDOORSTYLE(")
        .replace(
            "'Door type',$,$,(#60),$,$,$,.DOOR.,$,$,$);",
            "'Door type',$,$,(#60),$,$,.SINGLE_SWING_LEFT.,.NOTDEFINED.,.F.,.F.);",
        )
        .replace("IFCMATERIAL('Concrete',$,$)", "IFCMATERIAL('Concrete')")
}

/// 別のソフトで確かめるための変更。`tests/ifcopenshell/check.py` が、この結果を確かめる
fn edits_for_other_readers() -> Vec<Edit> {
    vec![
        // その場で書き直すセット
        property(32, "Pset_DoorCommon", "FireRating", label("EI90")),
        // 共有するセットの写し
        property(30, "Pset_WallCommon", "FireRating", label("EI120")),
        // 足すセットとプロパティ
        property(33, "Pset_SlabCommon", "Reference", label("S1")),
        property(
            33,
            "Pset_SlabCommon",
            "Thickness",
            NewValue {
                type_name: "IfcPositiveLengthMeasure".to_owned(),
                literal: NewLiteral::Real(200.0),
            },
        ),
        property(
            32,
            "Pset_DoorCommon",
            "HandicapAccessible",
            NewValue {
                type_name: "IfcBoolean".to_owned(),
                literal: NewLiteral::Boolean(true),
            },
        ),
        // 型のセット
        property(34, "Pset_DoorTypeCommon", "Reference", label("D2")),
        property(34, "Pset_Extra", "Note", label("備考")),
        // 名前と説明
        Edit::Attribute {
            element: 30,
            attribute: RootAttribute::Name,
            value: Some("壁 'A' \\ 🧱".to_owned()),
        },
        Edit::Attribute {
            element: 30,
            attribute: RootAttribute::Description,
            value: Some("説明".to_owned()),
        },
        Edit::Attribute {
            element: 31,
            attribute: RootAttribute::Name,
            value: None,
        },
    ]
}

/// 変更をまとめて当てられることを確かめ、`ZUMEN_IFC_EDITED` があれば、そのフォルダーに変える前と後の
/// ファイルを書く（#482）。毎晩の組が、書いたファイルを IfcOpenShell で開く
#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（決定 0007）"
)]
fn writes_the_edited_files_for_other_readers() {
    let out = std::env::var_os("ZUMEN_IFC_EDITED").map(std::path::PathBuf::from);
    for (schema, src) in [("ifc4", sample()), ("ifc2x3", sample_2x3())] {
        let (bytes, codes) = run(src.as_bytes(), &edits_for_other_readers());
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let bytes = bytes.unwrap();
        let building = lift(&bytes).unwrap();
        assert_eq!(
            value(&building, 31, "Pset_WallCommon", "FireRating"),
            Some(Literal::Text("EI30".to_owned())),
            "{schema}"
        );
        if let Some(dir) = &out {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(format!("{schema}.ifc")), &src).unwrap();
            std::fs::write(dir.join(format!("{schema}.edited.ifc")), &bytes).unwrap();
        }
    }
}
