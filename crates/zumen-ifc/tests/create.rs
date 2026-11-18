//! 新しい IFC のひな形（#395）。ファイルは、どれもテストの中で作る。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_building::{Building, Id, Unit};
use zumen_ifc::{
    Edit, Header, Ifc, LengthUnit, NewSpace, OwnerHistory, SpaceKind, Target, Template, Version,
    apply, create, derive_global_id,
};
use zumen_sdk::{Codec, Lift};

const VERSIONS: [Version; 3] = [Version::Ifc2x3, Version::Ifc4, Version::Ifc4x3];

fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn owner() -> OwnerHistory {
    OwnerHistory {
        person: "設計者".to_owned(),
        organization: "Example".to_owned(),
        developer: "zumen".to_owned(),
        application: "zumen".to_owned(),
        version: "0.3.0".to_owned(),
        identifier: "zumen".to_owned(),
        created: 1_700_000_000,
    }
}

fn template(version: Version, unit: LengthUnit, owner: Option<OwnerHistory>) -> Template {
    Template {
        version,
        unit,
        project: derive_global_id(b"namespace", b"project"),
        name: Some("新しい建物".to_owned()),
        header: Header {
            view_definition: Some("ReferenceView_V1.2".to_owned()),
            name: "a.ifc".to_owned(),
            time_stamp: "2026-11-18T09:00:00".to_owned(),
            author: vec!["O'Brien".to_owned()],
            organization: Vec::new(),
            preprocessor_version: String::new(),
            originating_system: "zumen 0.3.0".to_owned(),
            authorization: String::new(),
        },
        owner,
    }
}

fn made(template: &Template) -> (Option<Vec<u8>>, Vec<Code>) {
    let (result, diag) = with(|cx| create(template, cx));
    (result.ok(), diag.iter().map(|d| d.code()).collect())
}

fn lift(src: &[u8]) -> Option<Building> {
    let (file, diag) = with(|cx| Ifc::read(src, cx));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    let file = file.ok()?;
    let (building, diag) = with(|cx| file.lift(cx));
    // 建物モデルは人・組織・次元の指数を持たないので、落とした実体として数える
    let others: Vec<_> = diag
        .iter()
        .filter(|d| d.code() != codes::IFC_ENTITY_DROPPED)
        .collect();
    assert!(others.is_empty(), "{others:?}");
    building.ok()
}

fn length(building: &Building) -> Option<&Unit> {
    building
        .document
        .units
        .iter()
        .find(|unit| unit.kind == "LENGTHUNIT")
}

#[test]
fn writes_a_project_with_units_and_header_in_every_version() {
    for version in VERSIONS {
        let template = template(version, LengthUnit::Millimeter, Some(owner()));
        let (bytes, codes) = made(&template);
        assert!(codes.is_empty(), "{version:?}: {codes:?}");
        let bytes = bytes.unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(
            text.contains(&format!("FILE_SCHEMA(('{}'));", version.file_schema())),
            "{text}"
        );
        assert!(text.contains("FILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');"));
        assert!(text.contains("('O''Brien'),('')"), "{text}");
        assert!(text.contains("#1=IFCPROJECT("), "{text}");
        let building = lift(&bytes).unwrap();
        assert_eq!(
            building
                .element(building.document.project.unwrap())
                .unwrap()
                .id(),
            Id::Global(template.project)
        );
        assert_eq!(
            building.document.project_name.as_deref(),
            Some("新しい建物")
        );
        let unit = length(&building).unwrap();
        assert_eq!(
            (unit.prefix.as_deref(), unit.name.as_str()),
            (Some("MILLI"), "METRE")
        );
        assert_eq!(
            building.document.header.time_stamp.as_deref(),
            Some("2026-11-18T09:00:00")
        );
        assert_eq!(building.document.header.author, ["O'Brien"]);
        assert_eq!(
            building
                .document
                .application
                .as_ref()
                .map(|a| a.name.as_str()),
            Some("zumen")
        );
        assert_eq!(building.elements().len(), 1, "{version:?}");
    }
}

#[test]
fn writes_each_length_unit() {
    for (unit, prefix, name, scale) in [
        (LengthUnit::Millimeter, Some("MILLI"), "METRE", None),
        (LengthUnit::Centimeter, Some("CENTI"), "METRE", None),
        (LengthUnit::Meter, None, "METRE", None),
        (LengthUnit::Inch, None, "inch", Some(0.0254)),
        (LengthUnit::Foot, None, "foot", Some(0.3048)),
    ] {
        let (bytes, codes) = made(&template(Version::Ifc4, unit, None));
        assert!(codes.is_empty(), "{unit:?}: {codes:?}");
        let building = lift(&bytes.unwrap()).unwrap();
        let found = length(&building).unwrap();
        assert_eq!(found.prefix.as_deref(), prefix, "{unit:?}");
        assert_eq!(found.name, name, "{unit:?}");
        assert_eq!(found.scale, scale, "{unit:?}");
    }
}

#[test]
fn needs_an_owner_history_only_in_ifc2x3() {
    let (bytes, codes) = made(&template(Version::Ifc2x3, LengthUnit::Millimeter, None));
    assert!(bytes.is_none());
    assert_eq!(codes, [codes::IFC_OWNER_HISTORY_MISSING]);
    for version in [Version::Ifc4, Version::Ifc4x3] {
        let (bytes, codes) = made(&template(version, LengthUnit::Millimeter, None));
        assert!(codes.is_empty(), "{version:?}: {codes:?}");
        let text = String::from_utf8(bytes.unwrap()).unwrap();
        assert!(!text.contains("IFCOWNERHISTORY"), "{text}");
        assert!(text.contains("#1=IFCPROJECT('"), "{text}");
    }
}

#[test]
fn leaves_missing_header_values_empty_and_writes_the_same_bytes() {
    let mut bare = template(Version::Ifc4, LengthUnit::Millimeter, None);
    bare.header = Header::default();
    bare.name = None;
    let (a, _) = made(&bare);
    let (b, _) = made(&bare);
    let a = a.unwrap();
    assert_eq!(Some(&a), b.as_ref());
    let text = String::from_utf8(a).unwrap();
    assert!(text.contains("FILE_DESCRIPTION((''),'2;1');"), "{text}");
    assert!(
        text.contains("FILE_NAME('','',(''),(''),'','','');"),
        "{text}"
    );
}

#[test]
fn derives_global_ids_from_the_namespace_and_key() {
    let a = derive_global_id(b"namespace", b"project");
    assert_eq!(a, derive_global_id(b"namespace", b"project"));
    assert_ne!(a, derive_global_id(b"namespace", b"site"));
    assert_ne!(a, derive_global_id(b"other", b"project"));
    // 名前空間とキーの境目を変えると別の値になる
    assert_ne!(derive_global_id(b"ab", b"c"), derive_global_id(b"a", b"bc"));
}

#[test]
fn the_writer_adds_spaces_to_the_template() {
    for version in VERSIONS {
        let (bytes, _) = made(&template(version, LengthUnit::Millimeter, Some(owner())));
        let bytes = bytes.unwrap();
        let (file, _) = with(|cx| Ifc::read(&bytes, cx));
        let file = file.unwrap();
        let site = Edit::Space(NewSpace {
            kind: SpaceKind::Site,
            name: Some("敷地".to_owned()),
            parent: Target::Instance(1),
            placement: None,
            elevation: None,
            global_id: Some(derive_global_id(b"namespace", b"site")),
        });
        let (applied, diag) = with(|cx| apply(&file, &[site], b"seed", cx));
        assert!(
            diag.is_empty(),
            "{version:?}: {:?}",
            diag.iter().collect::<Vec<_>>()
        );
        let building = lift(&applied.unwrap().bytes).unwrap();
        assert!(
            building
                .find(Id::Global(derive_global_id(b"namespace", b"site")))
                .is_some()
        );
        // 書き手は、ひな形の文脈（Model と Body）を使い、足さない
        assert_eq!(building.document.contexts.len(), 2, "{version:?}");
    }
}

#[allow(clippy::disallowed_methods)]
#[test]
fn writes_the_templates_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_AUTHORED") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    for version in VERSIONS {
        let (bytes, _) = made(&template(version, LengthUnit::Millimeter, Some(owner())));
        std::fs::write(
            folder.join(format!("{}.template.ifc", version.file_schema())),
            bytes.unwrap(),
        )
        .unwrap();
    }
}
