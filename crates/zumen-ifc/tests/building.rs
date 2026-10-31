//! IFC から建物モデルを作る（決定 0207・0215）。ファイルは、どれも試験の中で作る。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, Value, codes};
use zumen_building::{
    Building, ElementRef, Grid, GridSet, Hash, Id, Literal, Local, MaterialKind, Opening,
    ParentKind, Placement, QuantityKind, SpatialKind, Transform,
};
use zumen_ifc::{Ifc, IfcFile};
use zumen_sdk::{Codec, Error, Lift};

#[derive(Clone, Copy, PartialEq)]
enum V {
    Ifc2x3,
    Ifc4,
    Ifc4x3,
}

const VERSIONS: [(V, &str); 3] = [
    (V::Ifc2x3, "IFC2X3"),
    (V::Ifc4, "IFC4"),
    (V::Ifc4x3, "IFC4X3_ADD2"),
];

/// 22 文字の GlobalId
fn gid(n: u32) -> String {
    format!("{n:0>22}")
}

/// 版ごとの最小の IFC。IFC4 からの属性は、`later` で足す
fn ifc(v: V, schema: &str) -> String {
    let later = |text: &str| if v == V::Ifc2x3 { "" } else { text }.to_owned();
    let mut lines =
        vec![
        format!("#1=IFCPROJECT('{}',#2,'Project',$,$,$,'Design',(#20,#25),#10);", gid(1)),
        "#2=IFCOWNERHISTORY($,#4,$,.ADDED.,$,$,$,0);".to_owned(),
        "#4=IFCAPPLICATION($,'1.0','Test App','test');".to_owned(),
        "#10=IFCUNITASSIGNMENT((#11,#12,#13));".to_owned(),
        "#11=(IFCNAMEDUNIT(*,.LENGTHUNIT.)IFCSIUNIT(.MILLI.,.METRE.));".to_owned(),
        "#12=IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.);".to_owned(),
        "#13=IFCCONVERSIONBASEDUNIT(#14,.PLANEANGLEUNIT.,'degree',#15);".to_owned(),
        "#14=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);".to_owned(),
        "#15=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.5),#16);".to_owned(),
        "#16=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);".to_owned(),
        "#20=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#21,#24);".to_owned(),
        "#21=IFCAXIS2PLACEMENT3D(#22,$,$);".to_owned(),
        "#22=IFCCARTESIANPOINT((0.,0.,0.));".to_owned(),
        "#24=IFCDIRECTION((0.,1.));".to_owned(),
        "#25=IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#20,$,.MODEL_VIEW.,$);"
            .to_owned(),
        format!(
            "#30=IFCSITE('{}',#2,'Site',$,$,#31,$,$,.ELEMENT.,$,$,12.5,$,$);",
            gid(30)
        ),
        "#31=IFCLOCALPLACEMENT($,#21);".to_owned(),
        format!(
            "#32=IFCBUILDING('{}',#2,'Building',$,$,#33,$,$,.ELEMENT.,100.,$,$);",
            gid(32)
        ),
        "#33=IFCLOCALPLACEMENT(#31,#21);".to_owned(),
        format!(
            "#34=IFCBUILDINGSTOREY('{}',#2,'1F',$,$,#35,$,'First',.ELEMENT.,3000.);",
            gid(34)
        ),
        "#35=IFCLOCALPLACEMENT(#33,#36);".to_owned(),
        "#36=IFCAXIS2PLACEMENT3D(#37,$,$);".to_owned(),
        "#37=IFCCARTESIANPOINT((0.,0.,3000.));".to_owned(),
        format!("#40=IFCRELAGGREGATES('{}',#2,$,$,#1,(#30));", gid(40)),
        format!("#41=IFCRELAGGREGATES('{}',#2,$,$,#30,(#32));", gid(41)),
        format!("#42=IFCRELAGGREGATES('{}',#2,$,$,#32,(#34));", gid(42)),
        format!(
            "#50=IFCWALL('{}',#2,'Wall',$,$,#51,$,'W-1'{});",
            gid(50),
            later(",.STANDARD.")
        ),
        "#51=IFCLOCALPLACEMENT(#52,#53);".to_owned(),
        "#52=IFCLOCALPLACEMENT(#35,#54);".to_owned(),
        "#53=IFCAXIS2PLACEMENT3D(#55,#56,#57);".to_owned(),
        "#54=IFCAXIS2PLACEMENT3D(#58,$,$);".to_owned(),
        "#55=IFCCARTESIANPOINT((1000.,0.,0.));".to_owned(),
        "#56=IFCDIRECTION((0.,0.,1.));".to_owned(),
        "#57=IFCDIRECTION((0.,2.,0.));".to_owned(),
        "#58=IFCCARTESIANPOINT((0.,500.));".to_owned(),
        format!(
            "#60=IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',#2,$,$,(#50,#80),#34);",
            gid(60)
        ),
        format!(
            "#61=IFCWALLTYPE('{}',#2,'Type A',$,$,(#62),$,$,'Basic',.STANDARD.);",
            gid(61)
        ),
        format!(
            "#62=IFCPROPERTYSET('{}',#2,'Pset_WallCommon',$,(#63,#64,#65));",
            gid(62)
        ),
        "#63=IFCPROPERTYSINGLEVALUE('Width',$,IFCPOSITIVELENGTHMEASURE(200.),$);".to_owned(),
        "#64=IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.T.),$);".to_owned(),
        "#65=IFCPROPERTYSINGLEVALUE('Reference',$,IFCLABEL('A\\X2\\58C1\\X0\\'),$);".to_owned(),
        format!("#66=IFCRELDEFINESBYTYPE('{}',#2,$,$,(#50),#61);", gid(66)),
        format!("#67=IFCPROPERTYSET('{}',#2,'Pset_Occ',$,(#68));", gid(67)),
        "#68=IFCPROPERTYSINGLEVALUE('Angle',$,IFCPLANEANGLEMEASURE(90.),$);".to_owned(),
        format!("#69=IFCRELDEFINESBYPROPERTIES('{}',#2,$,$,(#50),#67);", gid(69)),
        format!(
            "#70=IFCELEMENTQUANTITY('{}',#2,'Qto_WallBaseQuantities',$,'BaseQuantities',(#71));",
            gid(70)
        ),
        format!("#71=IFCQUANTITYLENGTH('Length',$,$,4000.{});", later(",$")),
        format!("#72=IFCRELDEFINESBYPROPERTIES('{}',#2,$,$,(#50),#70);", gid(72)),
        format!("#73=IFCMATERIAL('Concrete'{});", later(",$,$")),
        format!("#74=IFCMATERIALLAYER(#73,200.,$ {});", later(",'Core',$,$,$")),
        format!("#75=IFCMATERIALLAYERSET((#74),'Wall 200'{});", later(",$")),
        format!(
            "#76=IFCMATERIALLAYERSETUSAGE(#75,.AXIS2.,.POSITIVE.,-100.{});",
            later(",$")
        ),
        format!("#77=IFCRELASSOCIATESMATERIAL('{}',#2,$,$,(#50),#76);", gid(77)),
        format!(
            "#78=IFCOPENINGELEMENT('{}',#2,'Opening',$,$,$,$,${});",
            gid(78),
            later(",.OPENING.")
        ),
        format!("#79=IFCRELVOIDSELEMENT('{}',#2,$,$,#50,#78);", gid(79)),
        format!(
            "#80=IFCDOOR('{}',#2,'Door',$,$,$,$,$,2100.,900.{});",
            gid(80),
            later(",.DOOR.,$,$")
        ),
        format!("#81=IFCRELFILLSELEMENT('{}',#2,$,$,#78,#80);", gid(81)),
    ];
    if v != V::Ifc2x3 {
        lines.push("#90=IFCMAPCONVERSION(#20,#91,1000.,2000.,10.,1.,0.,$);".to_owned());
        lines.push("#91=IFCPROJECTEDCRS('EPSG:6677',$,$,$,$,$,$);".to_owned());
    }
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ViewDefinition [CoordinationView]'),'2;1');\n\
         FILE_NAME('model.ifc','2026-10-31T00:00:00',('author'),('org'),'pre','app','');\n\
         FILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
        lines.join("\n")
    )
}

fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn read(src: &[u8]) -> Option<IfcFile> {
    let (file, _) = with(Limits::default(), |cx| Ifc::read(src, cx));
    file.ok()
}

fn lift(src: &[u8]) -> (Result<Building, Error>, Diagnostics) {
    let Some(file) = read(src) else {
        return (Err(Error::Invalid), Diagnostics::new());
    };
    with(Limits::default(), |cx| file.lift(cx))
}

fn found(diag: &Diagnostics) -> Vec<Code> {
    diag.iter().map(|d| d.code()).collect()
}

/// GlobalId の要素
fn by_gid(building: &Building, n: u32) -> Option<ElementRef> {
    building.find(Id::Global(zumen_building::GlobalId::parse(&gid(n))?))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn lifts_every_version() {
    for (v, schema) in VERSIONS {
        let (building, diag) = lift(ifc(v, schema).as_bytes());
        let b = building.unwrap();
        // 単位の次元の指数だけが、建物モデルに写らない
        assert_eq!(
            found(&diag),
            [codes::IFC_ENTITY_DROPPED],
            "{schema}: {diag:?}"
        );
        let dropped = diag.iter().next().unwrap();
        assert_eq!(
            dropped.param("kind"),
            Some(&Value::Text("IFCDIMENSIONALEXPONENTS".to_owned()))
        );

        // 要素は書いた順。材料は GlobalId を持たないので `i` と番号
        let ids: Vec<String> = b.elements().iter().map(|e| e.id().to_string()).collect();
        let expected: Vec<String> = [1, 30, 32, 34, 50, 61]
            .iter()
            .map(|&n| gid(n))
            .chain(["i73".to_owned(), gid(78), gid(80)])
            .collect();
        assert_eq!(ids, expected, "{schema}");
        let c = &b.components;
        let [
            project,
            site,
            building,
            storey,
            wall,
            wall_type,
            concrete,
            opening,
            door,
        ] = [1, 30, 32, 34, 50, 61, 0, 78, 80].map(|n| {
            if n == 0 {
                b.find(Id::Instance(73)).unwrap()
            } else {
                by_gid(&b, n).unwrap()
            }
        });

        // クラスと名前
        let class = c.class.get(wall).unwrap();
        assert_eq!(class.name, "IfcWall");
        assert_eq!(class.original, "IfcWall");
        let predefined = if v == V::Ifc2x3 {
            None
        } else {
            Some("STANDARD".to_owned())
        };
        assert_eq!(class.predefined, predefined, "{schema}");
        assert_eq!(c.identity.get(wall).unwrap().tag.as_deref(), Some("W-1"));
        assert_eq!(
            c.class.get(wall_type).unwrap().user_defined.as_deref(),
            Some("Basic")
        );
        assert_eq!(
            c.class.get(wall_type).unwrap().predefined.as_deref(),
            Some("STANDARD")
        );
        assert_eq!(c.class.get(concrete).unwrap().name, "IfcMaterial");
        assert_eq!(
            c.identity.get(concrete).unwrap().name.as_deref(),
            Some("Concrete")
        );
        assert_eq!(
            c.identity.get(storey).unwrap().long_name.as_deref(),
            Some("First")
        );

        // 空間構造
        let spatial = c.spatial.get(storey).unwrap();
        assert_eq!(spatial.kind, SpatialKind::Storey);
        let elevation = spatial.elevation.as_ref().unwrap();
        assert_eq!(elevation.value, 3000.0);
        let unit = elevation.unit.as_ref().unwrap();
        assert_eq!(
            (
                unit.kind.as_str(),
                unit.prefix.as_deref(),
                unit.name.as_str()
            ),
            ("LENGTHUNIT", Some("MILLI"), "METRE")
        );
        assert_eq!(
            c.spatial
                .get(site)
                .unwrap()
                .ref_elevation
                .as_ref()
                .unwrap()
                .value,
            12.5
        );
        assert_eq!(
            c.spatial
                .get(building)
                .unwrap()
                .elevation
                .as_ref()
                .unwrap()
                .value,
            100.0
        );

        // 集約と包含
        let of = |e| c.parent.get(e).and_then(|p| p.of.clone()).unwrap();
        assert_eq!(
            (of(site).target, of(site).kind, of(site).order),
            (project, ParentKind::Aggregates, None)
        );
        assert_eq!(
            (of(wall).target, of(wall).kind),
            (storey, ParentKind::Contained)
        );
        assert_eq!(of(door).target, storey);

        // 型とプロパティ。型のプロパティは型の要素が持つ
        assert_eq!(c.r#type.get(wall).unwrap().target, wall_type);
        let type_sets = &c.properties.get(wall_type).unwrap().sets;
        assert_eq!(type_sets.len(), 1);
        let common = &type_sets[0];
        assert_eq!(common.name, "Pset_WallCommon");
        let width = &common.properties[0];
        let width_value = width.value.as_ref().unwrap();
        assert_eq!(
            width_value.type_name.as_deref(),
            Some("IfcPositiveLengthMeasure")
        );
        assert_eq!(width_value.value, Literal::Real(200.0));
        assert_eq!(
            width.unit.as_ref().unwrap().prefix.as_deref(),
            Some("MILLI")
        );
        assert_eq!(
            common.properties[1].value.as_ref().unwrap().value,
            Literal::Boolean(true)
        );
        assert_eq!(
            common.properties[2].value.as_ref().unwrap().value,
            Literal::Text("A壁".to_owned())
        );
        assert_eq!(common.properties[2].unit, None);
        let angle = &c.properties.get(wall).unwrap().sets[0].properties[0];
        let degree = angle.unit.as_ref().unwrap();
        assert_eq!((degree.name.as_str(), degree.scale), ("degree", Some(0.5)));

        // 数量は書いたとおり
        let quantities = &c.quantities.get(wall).unwrap().sets[0];
        assert_eq!(quantities.method.as_deref(), Some("BaseQuantities"));
        let length = &quantities.quantities[0];
        assert_eq!(
            (length.name.as_str(), length.kind, length.value.value),
            ("Length", QuantityKind::Length, 4000.0)
        );
        assert!(length.value.unit.is_some());

        // 材料
        let material = c.material.get(wall).unwrap();
        assert_eq!(material.kind, MaterialKind::Layers);
        assert_eq!(material.direction.as_deref(), Some("AXIS2"));
        assert_eq!(material.offset.as_ref().unwrap().value, -100.0);
        let layer = &material.parts[0];
        assert_eq!(layer.material, Some(concrete));
        assert_eq!(layer.thickness.as_ref().unwrap().value, 200.0);
        let core = if v == V::Ifc2x3 {
            None
        } else {
            Some("Core".to_owned())
        };
        assert_eq!(layer.name, core);

        // 開口
        assert_eq!(c.opening.get(opening), Some(&Opening::Voids(wall)));
        assert_eq!(c.opening.get(door), Some(&Opening::Fills(opening)));

        // 出所: 包含の関係の番号と、列の中の位置
        let relations = &b.element(door).unwrap().source().relations;
        let contained = relations.iter().find(|r| r.instance == 60).unwrap();
        assert_eq!(contained.position, Some(1));
        assert_eq!(contained.global_id, Some(gid(60)));
        assert_eq!(b.element(wall).unwrap().source().instance, Some(50));

        // 文書
        let d = &b.document;
        assert_eq!(d.project, Some(project));
        assert_eq!(
            (d.project_name.as_deref(), d.phase.as_deref()),
            (Some("Project"), Some("Design"))
        );
        assert_eq!(d.units.len(), 3);
        assert_eq!(d.contexts.len(), 2);
        assert_eq!(d.contexts[0].precision, Some(1e-5));
        assert_eq!(d.contexts[0].dimension, Some(3));
        assert_eq!(d.contexts[1].identifier.as_deref(), Some("Body"));
        assert_eq!(d.true_north, Some([0.0, 1.0]));
        let application = d.application.as_ref().unwrap();
        assert_eq!(
            (application.name.as_str(), application.version.as_deref()),
            ("Test App", Some("1.0"))
        );
        assert_eq!(d.header.name.as_deref(), Some("model.ifc"));
        assert_eq!(d.header.author, ["author"]);
        assert_eq!(d.header.schemas, [schema]);
        match v {
            V::Ifc2x3 => assert_eq!(d.map, None),
            _ => {
                let map = d.map.as_ref().unwrap();
                assert_eq!(map.crs_name.as_deref(), Some("EPSG:6677"));
                assert_eq!(
                    (map.eastings, map.northings, map.height),
                    (1000.0, 2000.0, 10.0)
                );
            }
        }
    }
}

#[test]
fn folds_intermediate_placements() {
    for (v, schema) in VERSIONS {
        let (building, _) = lift(ifc(v, schema).as_bytes());
        let b = building.unwrap();
        let wall = by_gid(&b, 50).unwrap();
        let storey = by_gid(&b, 34).unwrap();
        // 壁の配置は、どの要素も持たない #52 を畳んで、階に対する変換にする
        let Placement { relative_to, local } = b.components.placement.get(wall).unwrap();
        assert_eq!(*relative_to, Some(storey));
        let Local::Transform(local) = local else {
            panic!("{local:?}");
        };
        let origin = local.apply([0.0, 0.0, 0.0]);
        assert_eq!(origin, [1000.0, 500.0, 0.0]);
        // RefDirection の (0, 2, 0) は長さを 1 にし、X 軸にする
        let x = local.apply([1.0, 0.0, 0.0]);
        assert!(
            close(x[0], 1000.0) && close(x[1], 501.0) && close(x[2], 0.0),
            "{x:?}"
        );
        assert_eq!(b.element(wall).unwrap().source().folded_placements, [52]);
        let (world, _) = with(Limits::default(), |cx| b.world_placement(wall, cx));
        let world: Transform = world.unwrap().unwrap();
        assert_eq!(world.apply([0.0, 0.0, 0.0]), [1000.0, 500.0, 3000.0]);
    }
}

#[test]
fn resolves_units_and_reports_missing_ones() {
    let src = ifc(V::Ifc4, "IFC4")
        // 書いた単位を使う
        .replace(
            "IFCPLANEANGLEMEASURE(90.),$)",
            "IFCPLANEANGLEMEASURE(90.),#16)",
        )
        // 割り当てにない体積の単位と、表にない量の型
        .replace("(#63,#64,#65)", "(#63,#64,#65,#100,#101)")
        .replace(
            "#66=",
            "#100=IFCPROPERTYSINGLEVALUE('Volume',$,IFCVOLUMEMEASURE(1.),$);\n\
             #101=IFCPROPERTYSINGLEVALUE('U',$,IFCTHERMALTRANSMITTANCEMEASURE(0.3),$);\n#66=",
        );
    let (building, diag) = lift(src.as_bytes());
    let b = building.unwrap();
    let wall = by_gid(&b, 50).unwrap();
    let angle = &b.components.properties.get(wall).unwrap().sets[0].properties[0];
    assert_eq!(angle.unit.as_ref().unwrap().name, "RADIAN");
    let wall_type = by_gid(&b, 61).unwrap();
    let properties = &b.components.properties.get(wall_type).unwrap().sets[0].properties;
    assert_eq!(properties[3].unit, None);
    assert_eq!(properties[4].unit, None);
    let missing: Vec<_> = diag
        .iter()
        .filter(|d| d.code() == codes::IFC_UNIT_MISSING)
        .map(|d| d.param("type").cloned())
        .collect();
    assert_eq!(
        missing,
        [
            Some(Value::Text("IfcVolumeMeasure".to_owned())),
            // 表にない型の名前は、書いたまま
            Some(Value::Text("IFCTHERMALTRANSMITTANCEMEASURE".to_owned()))
        ]
    );
}

#[test]
fn decodes_text_and_replaces_unreadable_bytes() {
    let mut src = ifc(V::Ifc4, "IFC4")
        .replace("'Wall'", "'壁\\S\\D'")
        .into_bytes();
    // 名前の UTF-8 は読み、0xFF は置き換えの文字にする
    let at = src.windows(6).position(|w| w == b"'Site'").unwrap();
    src[at + 1] = 0xFF;
    let (building, diag) = lift(&src);
    let b = building.unwrap();
    let wall = by_gid(&b, 50).unwrap();
    assert_eq!(
        b.components.identity.get(wall).unwrap().name.as_deref(),
        Some("壁Ä")
    );
    let site = by_gid(&b, 30).unwrap();
    assert_eq!(
        b.components.identity.get(site).unwrap().name.as_deref(),
        Some("\u{FFFD}ite")
    );
    assert!(found(&diag).contains(&codes::IFC_TEXT_INVALID));
}

#[test]
fn reports_placements_that_cannot_be_resolved() {
    // 軸と RefDirection が平行なので、軸を作れない
    let src = ifc(V::Ifc4, "IFC4").replace(
        "#57=IFCDIRECTION((0.,2.,0.));",
        "#57=IFCDIRECTION((0.,0.,-3.));",
    );
    let (building, diag) = lift(src.as_bytes());
    let b = building.unwrap();
    let wall = by_gid(&b, 50).unwrap();
    assert!(b.components.placement.get(wall).is_none());
    let invalid = diag
        .iter()
        .find(|d| d.code() == codes::IFC_PLACEMENT_INVALID)
        .unwrap();
    assert_eq!(invalid.param("id"), Some(&Value::Text(gid(50))));

    // 中間の配置が循環する
    let src =
        ifc(V::Ifc4, "IFC4").replace("#52=IFCLOCALPLACEMENT(#35,", "#52=IFCLOCALPLACEMENT(#52,");
    let (building, diag) = lift(src.as_bytes());
    assert!(building.unwrap().components.placement.get(wall).is_none());
    assert!(found(&diag).contains(&codes::IFC_PLACEMENT_INVALID));

    // 通り芯と線形の配置は、種類だけを持つ
    for (kind, name, schema) in [
        ("IFCGRIDPLACEMENT($,#120,$)", "IfcGridPlacement", "IFC4"),
        (
            "IFCLINEARPLACEMENT($,#120,$)",
            "IfcLinearPlacement",
            "IFC4X3_ADD2",
        ),
    ] {
        let v = if schema == "IFC4" { V::Ifc4 } else { V::Ifc4x3 };
        let src =
            ifc(v, schema).replace("#51=IFCLOCALPLACEMENT(#52,#53);", &format!("#51={kind};"));
        let (building, diag) = lift(src.as_bytes());
        let b = building.unwrap();
        let wall = by_gid(&b, 50).unwrap();
        assert_eq!(
            b.components.placement.get(wall).unwrap().local,
            Local::Unsupported(name.to_owned())
        );
        let unsupported = diag
            .iter()
            .find(|d| d.code() == codes::IFC_PLACEMENT_UNSUPPORTED)
            .unwrap();
        assert_eq!(
            unsupported.param("kind"),
            Some(&Value::Text(name.to_owned()))
        );
    }
}

#[test]
fn makes_ids_from_instances_for_bad_global_ids() {
    let src = ifc(V::Ifc4, "IFC4")
        // 重複・不正・なし
        .replace(
            &format!("IFCDOOR('{}'", gid(80)),
            &format!("IFCDOOR('{}'", gid(50)),
        )
        .replace(
            &format!("IFCOPENINGELEMENT('{}'", gid(78)),
            "IFCOPENINGELEMENT('bad'",
        )
        .replace(&format!("IFCWALLTYPE('{}'", gid(61)), "IFCWALLTYPE($");
    let (building, diag) = lift(src.as_bytes());
    let b = building.unwrap();
    for n in [80, 78, 61] {
        let element = b.find(Id::Instance(n)).unwrap();
        assert_eq!(b.element(element).unwrap().source().instance, Some(n));
    }
    let codes_found = found(&diag);
    for code in [
        codes::BUILDING_GLOBAL_ID_EXISTS,
        codes::BUILDING_GLOBAL_ID_INVALID,
        codes::IFC_GLOBAL_ID_MISSING,
    ] {
        assert!(codes_found.contains(&code), "{code:?}: {diag:?}");
    }
    // 元の文字列は出所に残す
    let opening = b.find(Id::Instance(78)).unwrap();
    assert_eq!(
        b.element(opening).unwrap().source().global_id.as_deref(),
        Some("bad")
    );
}

#[test]
fn does_not_lift_other_versions() {
    for schema in ["IFC4X1", "IFC2X2_FINAL", "IFC5"] {
        let (building, diag) = lift(ifc(V::Ifc4, schema).as_bytes());
        assert!(matches!(building, Err(Error::Unsupported)), "{schema}");
        assert_eq!(found(&diag), [codes::IFC_VERSION_UNSUPPORTED]);
        assert_eq!(
            diag.iter().next().unwrap().param("schema"),
            Some(&Value::Text(schema.to_owned()))
        );
    }
}

#[test]
fn reads_grids_nests_and_groups() {
    let src = ifc(V::Ifc4, "IFC4").replace(
        "#90=",
        &format!(
            "#110=IFCGRID('{}',#2,'Grid',$,$,$,$,(#111),(#112),$,$);\n\
             #111=IFCGRIDAXIS('A',$,.T.);\n#112=IFCGRIDAXIS('1',$,.F.);\n\
             #113=IFCRELNESTS('{}',#2,$,$,#50,(#80,#78));\n\
             #114=IFCZONE('{}',#2,'Zone',$,$,'Long');\n\
             #115=IFCRELASSIGNSTOGROUP('{}',#2,$,$,(#50),$,#114);\n\
             #116=IFCRELCONNECTSPATHELEMENTS('{}',#2,$,$,$,#50,#80,(),(),.ATSTART.,.ATEND.);\n#90=",
            gid(110),
            gid(113),
            gid(114),
            gid(115),
            gid(116)
        ),
    );
    let (building, diag) = lift(src.as_bytes());
    let b = building.unwrap();
    let c = &b.components;
    let grid = by_gid(&b, 110).unwrap();
    let [a, one] = [111, 112].map(|n| b.find(Id::Instance(n)).unwrap());
    assert_eq!(
        c.grid.get(grid),
        Some(&Grid::Grid {
            axes: vec![(GridSet::U, a), (GridSet::V, one)]
        })
    );
    assert_eq!(
        c.grid.get(one),
        Some(&Grid::Axis {
            tag: Some("1".to_owned()),
            same_sense: false,
            curve: None
        })
    );
    // 入れ子は、先に包含で親を持つ扉を除いて写し、IFC4 からは順を持つ
    let wall = by_gid(&b, 50).unwrap();
    let opening = by_gid(&b, 78).unwrap();
    let nested = c.parent.get(opening).unwrap().of.clone().unwrap();
    assert_eq!(
        (nested.target, nested.kind, nested.order),
        (wall, ParentKind::Nests, Some(1))
    );
    assert!(found(&diag).contains(&codes::IFC_PARENT_EXISTS));
    let zone = by_gid(&b, 114).unwrap();
    assert_eq!(c.group.get(wall).unwrap().groups, [zone]);
    assert_eq!(
        c.identity.get(zone).unwrap().long_name.as_deref(),
        Some("Long")
    );
    let door = by_gid(&b, 80).unwrap();
    assert_eq!(c.connection.get(wall).unwrap().links[0].target, door);
    assert_eq!(c.connection.get(door).unwrap().links[0].target, wall);
}

#[test]
fn same_input_gives_same_ids_and_hashes() {
    for (v, schema) in VERSIONS {
        let src = ifc(v, schema);
        let hashes = |b: &Building| -> Vec<(String, Hash)> {
            b.elements()
                .iter()
                .enumerate()
                .map(|(at, e)| {
                    let element = ElementRef(at as u32);
                    (e.id().to_string(), Hash::element(b, element).unwrap())
                })
                .collect()
        };
        let (first, _) = lift(src.as_bytes());
        let (second, _) = lift(src.as_bytes());
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(hashes(&first), hashes(&second));
        assert_eq!(
            Hash::document_values(&first),
            Hash::document_values(&second)
        );
        assert_eq!(first, second);
        // 値を変えた要素だけ、ハッシュが変わる
        let (changed, _) = lift(src.replace("'W-1'", "'W-2'").as_bytes());
        let changed = changed.unwrap();
        let (a, b) = (hashes(&first), hashes(&changed));
        let differ: Vec<_> = a
            .iter()
            .zip(&b)
            .filter(|(x, y)| x != y)
            .map(|(x, _)| x.0.clone())
            .collect();
        assert_eq!(differ, [gid(50)], "{schema}");
    }
}

#[test]
fn budgets_stop_lifting() {
    let file = read(ifc(V::Ifc4, "IFC4").as_bytes()).unwrap();
    for (kind, code) in [
        (Kind::Elements, codes::BUDGET_ELEMENTS_EXCEEDED),
        (Kind::Work, codes::BUDGET_WORK_EXCEEDED),
    ] {
        let (result, diag) = with(Limits::default().with(kind, Limit::Finite(3)), |cx| {
            file.lift(cx)
        });
        assert!(matches!(result, Err(Error::Stopped(_))), "{kind:?}");
        assert!(found(&diag).contains(&code), "{kind:?}");
    }
}

/// どこで切っても、どのバイトを変えても、パニックしない
#[test]
fn truncated_and_mutated_files_do_not_panic() {
    let src = ifc(V::Ifc4, "IFC4").into_bytes();
    let try_all = |src: &[u8]| {
        let (file, _) = with(Limits::default(), |cx| Ifc::read(src, cx));
        if let Ok(file) = file {
            let _ = with(Limits::default(), |cx| file.lift(cx));
        }
    };
    for end in (0..src.len()).step_by(7) {
        try_all(&src[..end]);
    }
    for at in 0..src.len() {
        for byte in *b"#($0" {
            let mut changed = src.clone();
            changed[at] = byte;
            try_all(&changed);
        }
    }
}
