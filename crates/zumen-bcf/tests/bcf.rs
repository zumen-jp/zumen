//! BCF 3.0 の bcfzip を読み、レビューの文書に写して書き戻す（決定 0214・0220）。bcfzip は試験の中で作る

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_bcf::{Bcf, Options, Statuses};
use zumen_review::{
    Camera, Colored, Comment, Location, Place, Projection, Reference, Review, Status, Target,
    Topic, Version,
};
use zumen_sdk::Error;

const TOPIC: &str = "6e4a7c1c-1111-4c3b-9a3e-0f6c3a2b1d01";
const VIEW: &str = "0b2c3d4e-2222-4f5a-8b6c-7d8e9f0a1b02";
const COMMENT: &str = "9a8b7c6d-3333-4e5f-9071-82a3b4c5d603";
const DOOR: &str = "0000000000000000000080";

fn markup() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Markup>
  <Header><Files><File IfcProject="0000000000000000000001" IsExternal="true"><Filename>a.ifc</Filename></File></Files></Header>
  <Topic Guid="{TOPIC}" TopicType="Issue" TopicStatus="In Progress" ServerAssignedId="T-1" Custom="x">
    <ReferenceLinks><ReferenceLink>https://example.com/1</ReferenceLink></ReferenceLinks>
    <Title>扉の幅がない</Title>
    <Priority>High</Priority>
    <Labels><Label>Door</Label></Labels>
    <CreationDate>2026-10-01T09:00:00+09:00</CreationDate>
    <CreationAuthor>a@example.com</CreationAuthor>
    <AssignedTo>b@example.com</AssignedTo>
    <Stage>Design</Stage>
    <Description>OverallWidth がない</Description>
    <BimSnippet SnippetType="IDS"><Reference>door.ids</Reference><ReferenceSchema>ids.xsd</ReferenceSchema></BimSnippet>
    <Comments>
      <Comment Guid="{COMMENT}"><Date>2026-10-02T10:00:00+09:00</Date><Author>b@example.com</Author><Comment>直します</Comment><Viewpoint Guid="{VIEW}"/><ModifiedDate>2026-10-03T10:00:00+09:00</ModifiedDate></Comment>
    </Comments>
    <Viewpoints>
      <ViewPoint Guid="{VIEW}"><Viewpoint>viewpoint.bcfv</Viewpoint><Snapshot>snapshot.png</Snapshot></ViewPoint>
    </Viewpoints>
    <Vendor>keep</Vendor>
  </Topic>
</Markup>
"#
    )
}

fn visinfo() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<VisualizationInfo Guid="{VIEW}">
  <Components>
    <Selection><Component IfcGuid="{DOOR}"/><Component><AuthoringToolId>42</AuthoringToolId></Component></Selection>
    <Visibility DefaultVisibility="true"><Exceptions><Component IfcGuid="0000000000000000000034"/></Exceptions></Visibility>
    <Coloring><Color Color="ff0000"><Components><Component IfcGuid="{DOOR}"/></Components></Color></Coloring>
  </Components>
  <PerspectiveCamera>
    <CameraViewPoint><X>1</X><Y>2</Y><Z>3</Z></CameraViewPoint>
    <CameraDirection><X>0</X><Y>1</Y><Z>0</Z></CameraDirection>
    <CameraUpVector><X>0</X><Y>0</Y><Z>1</Z></CameraUpVector>
    <FieldOfView>60</FieldOfView>
    <AspectRatio>1.5</AspectRatio>
  </PerspectiveCamera>
  <ClippingPlanes><ClippingPlane><Location><X>0</X><Y>0</Y><Z>1.5</Z></Location><Direction><X>0</X><Y>0</Y><Z>1</Z></Direction></ClippingPlane></ClippingPlanes>
  <Bitmaps><Bitmap><Format>png</Format><Reference>logo.png</Reference><Location><X>0</X><Y>0</Y><Z>0</Z></Location><Normal><X>0</X><Y>0</Y><Z>1</Z></Normal><Up><X>0</X><Y>1</Y><Z>0</Z></Up><Height>1</Height></Bitmap></Bitmaps>
</VisualizationInfo>
"#
    )
}

const VERSION_3: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?><Version VersionId="3.0"/>"#;
const EXTENSIONS: &[u8] = br#"<Extensions><TopicStatuses><TopicStatus>In Progress</TopicStatus></TopicStatuses></Extensions>"#;
const SNAPSHOT: &[u8] = b"\x89PNG\r\n\x1a\nsnapshot";

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    zumen_zip::write(entries).unwrap_or_default()
}

fn sample() -> Vec<u8> {
    let markup = markup();
    let visinfo = visinfo();
    zip(&[
        ("bcf.version", VERSION_3),
        ("extensions.xml", EXTENSIONS),
        ("Documents/d.pdf", b"%PDF"),
        (&format!("{TOPIC}/"), b""),
        (&format!("{TOPIC}/markup.bcf"), markup.as_bytes()),
        (&format!("{TOPIC}/viewpoint.bcfv"), visinfo.as_bytes()),
        (&format!("{TOPIC}/snapshot.png"), SNAPSHOT),
    ])
}

fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn found(diag: &Diagnostics) -> Vec<Code> {
    diag.iter().map(|d| d.code()).collect()
}

fn target() -> Target {
    Target {
        id: "model".to_string(),
        format: "ifc".to_string(),
        hash: "v1:model".to_string(),
        name: None,
        extra: Vec::new(),
    }
}

fn options() -> Options {
    Options {
        statuses: Statuses::default(),
        target: "model".to_string(),
        kind: None,
        author: None,
        date: None,
    }
}

/// 扉だけにハッシュのある版
struct Model;

impl Version for Model {
    fn hash(&self) -> String {
        "v1:model".to_string()
    }

    fn element(&self, id: &str) -> Option<String> {
        (id == DOOR).then(|| "v1:door".to_string())
    }
}

fn read(bytes: &[u8]) -> Option<(Bcf, Review)> {
    let (bcf, diag) = with(Limits::default(), |cx| Bcf::read(bytes, cx));
    assert!(diag.is_empty(), "{diag:?}");
    let bcf = bcf.ok()?;
    let (review, diag) = with(Limits::default(), |cx| {
        zumen_bcf::review(&bcf, &target(), &Statuses::default(), Some(&Model), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    Some((bcf, review.ok()?))
}

fn write(
    review: &Review,
    original: Option<&Bcf>,
    options: &Options,
) -> Option<(Vec<u8>, Diagnostics)> {
    let (bytes, diag) = with(Limits::default(), |cx| {
        zumen_bcf::write(review, original, options, cx)
    });
    Some((bytes.ok()?, diag))
}

fn entry(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let (archive, _) = with(Limits::default(), |cx| zumen_zip::read(bytes, cx));
    let archive = archive.ok()?;
    let entry = archive.find(name)?;
    with(Limits::default(), |cx| archive.extract(entry, cx))
        .0
        .ok()
}

#[test]
fn reads_topics_comments_and_viewpoints_into_the_review() {
    let (bcf, review) = read(&sample()).unwrap();
    assert_eq!(bcf.version, "3.0");
    assert_eq!(bcf.topics.len(), 1);
    // markup.bcf のないフォルダーは、ほかの項目として保つ
    let names: Vec<&str> = bcf.files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["extensions.xml", "Documents/d.pdf"]);
    assert_eq!(review.targets, [target()]);
    let topic = &review.topics[0];
    assert_eq!(topic.id, TOPIC);
    assert_eq!(topic.title, "扉の幅がない");
    // 表にない状態は open にし、元の文字列を残す
    assert_eq!(topic.status, Status::Open);
    assert_eq!(topic.status_label.as_deref(), Some("In Progress"));
    assert_eq!(topic.kind.as_deref(), Some("Issue"));
    assert_eq!(topic.priority.as_deref(), Some("High"));
    assert_eq!(topic.labels, ["Door"]);
    assert_eq!(topic.assignee.as_deref(), Some("b@example.com"));
    assert_eq!(topic.author.as_deref(), Some("a@example.com"));
    assert_eq!(topic.created.as_deref(), Some("2026-10-01T09:00:00+09:00"));
    assert_eq!(
        topic.comments,
        [Comment {
            id: COMMENT.to_string(),
            author: Some("b@example.com".to_string()),
            date: Some("2026-10-02T10:00:00+09:00".to_string()),
            text: "直します".to_string(),
            location: Some(VIEW.to_string()),
            extra: Vec::new(),
        }]
    );
    let door = |hash: Option<&str>| Reference {
        id: DOOR.to_string(),
        hash: hash.map(str::to_string),
    };
    assert_eq!(
        topic.locations,
        [Location {
            id: VIEW.to_string(),
            target: "model".to_string(),
            place: Place::Space {
                camera: Some(Camera {
                    projection: Projection::Perspective(60.0),
                    position: [1.0, 2.0, 3.0],
                    direction: [0.0, 1.0, 0.0],
                    up: [0.0, 0.0, 1.0],
                }),
                // AuthoringToolId だけの要素は写さない
                selected: vec![door(Some("v1:door"))],
                visible: Some(true),
                exceptions: vec![Reference {
                    id: "0000000000000000000034".to_string(),
                    hash: None,
                }],
                colored: vec![Colored {
                    color: "#FF0000".to_string(),
                    elements: vec![door(Some("v1:door"))],
                }],
                clips: vec![zumen_review::Clip {
                    point: [0.0, 0.0, 1.5],
                    normal: [0.0, 0.0, 1.0],
                }],
            },
            carried: None,
            extra: Vec::new(),
        }]
    );
}

#[test]
fn writes_unchanged_topics_back_byte_for_byte() {
    let original = sample();
    let (bcf, review) = read(&original).unwrap();
    let (bytes, diag) = write(&review, Some(&bcf), &options()).unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    for name in [
        "extensions.xml",
        "Documents/d.pdf",
        &format!("{TOPIC}/markup.bcf"),
        &format!("{TOPIC}/viewpoint.bcfv"),
        &format!("{TOPIC}/snapshot.png"),
    ] {
        assert_eq!(entry(&bytes, name), entry(&original, name), "{name}");
    }
    assert_eq!(
        entry(&bytes, "bcf.version").unwrap(),
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Version VersionId=\"3.0\"/>\n"
    );
}

#[test]
fn keeps_unmapped_and_unknown_parts_of_changed_topics() {
    let (bcf, mut review) = read(&sample()).unwrap();
    review.topics[0].title = "扉の幅を入れる".to_string();
    review.topics[0].status = Status::Resolved;
    let (bytes, diag) = write(&review, Some(&bcf), &options()).unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    let markup = String::from_utf8(entry(&bytes, &format!("{TOPIC}/markup.bcf")).unwrap()).unwrap();
    for kept in [
        "<Filename>a.ifc</Filename>",
        "ServerAssignedId=\"T-1\"",
        "Custom=\"x\"",
        "<ReferenceLink>https://example.com/1</ReferenceLink>",
        "<Stage>Design</Stage>",
        "<BimSnippet SnippetType=\"IDS\">",
        "<Vendor>keep</Vendor>",
        // 変えていないコメントは元の要素のまま
        "<ModifiedDate>2026-10-03T10:00:00+09:00</ModifiedDate>",
        "TopicStatus=\"Resolved\"",
        "<Title>扉の幅を入れる</Title>",
    ] {
        assert!(markup.contains(kept), "{kept}\n{markup}");
    }
    // 変えていない viewpoint は、ファイルとスナップショットごと保つ
    assert_eq!(
        entry(&bytes, &format!("{TOPIC}/viewpoint.bcfv")).unwrap(),
        visinfo().as_bytes()
    );
    assert_eq!(
        entry(&bytes, &format!("{TOPIC}/snapshot.png")).unwrap(),
        SNAPSHOT
    );
    let (_, again) = read(&bytes).unwrap();
    assert_eq!(again.topics[0].title, "扉の幅を入れる");
    assert_eq!(again.topics[0].status, Status::Resolved);
    assert_eq!(again.topics[0].status_label, None);
    assert_eq!(again.topics[0].locations, review.topics[0].locations);

    // 場所を変えれば viewpoint を書き直し、元のスナップショットは捨てる
    let Place::Space { camera, .. } = &mut review.topics[0].locations[0].place else {
        panic!();
    };
    if let Some(camera) = camera {
        camera.position = [5.0, 5.0, 5.0];
    }
    let (bytes, diag) = write(&review, Some(&bcf), &options()).unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    assert!(entry(&bytes, &format!("{TOPIC}/snapshot.png")).is_none());
    assert!(entry(&bytes, &format!("{TOPIC}/viewpoint.bcfv")).is_none());
    assert!(entry(&bytes, &format!("{TOPIC}/{VIEW}.bcfv")).is_some());
    let (_, again) = read(&bytes).unwrap();
    assert_eq!(again.topics[0].locations, review.topics[0].locations);
}

fn adopted() -> Review {
    let space = |_target: &str, camera: Option<Camera>| Place::Space {
        camera,
        selected: vec![
            Reference {
                id: DOOR.to_string(),
                hash: Some("v1:door".to_string()),
            },
            Reference {
                id: "i12".to_string(),
                hash: None,
            },
        ],
        visible: None,
        exceptions: Vec::new(),
        colored: Vec::new(),
        clips: Vec::new(),
    };
    let camera = Camera {
        projection: Projection::Orthographic(20.0),
        position: [0.0, 0.0, 10.0],
        direction: [0.0, 0.0, -1.0],
        up: [0.0, 1.0, 0.0],
    };
    let location = |id: &str, target: &str, place: Place| Location {
        id: id.to_string(),
        target: target.to_string(),
        place,
        carried: None,
        extra: Vec::new(),
    };
    Review {
        targets: vec![target()],
        topics: vec![Topic {
            id: "a1b2c3d4-0000-8000-8000-000000000001".to_string(),
            title: "扉の幅".to_string(),
            description: None,
            status: Status::Open,
            status_label: None,
            kind: None,
            priority: None,
            labels: Vec::new(),
            assignee: None,
            due: None,
            author: None,
            created: None,
            modified_by: None,
            modified: None,
            related: Vec::new(),
            comments: vec![Comment {
                id: "c1".to_string(),
                author: Some("b@example.com".to_string()),
                date: Some("2026-10-02T10:00:00Z".to_string()),
                text: "見てください".to_string(),
                location: Some("t/l1".to_string()),
                extra: Vec::new(),
            }],
            locations: vec![
                location("t/l1", "model", space("model", Some(camera))),
                location("t/l2", "model", space("model", None)),
                location("t/l3", "plan", space("plan", None)),
                location(
                    "t/l4",
                    "model",
                    Place::Plane {
                        view: None,
                        pin: Some([1.0, 2.0]),
                        extent: None,
                        elements: Vec::new(),
                    },
                ),
            ],
            origin: Some(zumen_review::Origin {
                rule: "width".to_string(),
                diagnostic: zumen_json::Value::Null,
            }),
            extra: Vec::new(),
        }],
        markups: Vec::new(),
        extra: Vec::new(),
    }
}

#[test]
fn writes_a_new_bcf_and_reports_what_bcf_cannot_hold() {
    let review = adopted();
    // 作者・日時・種類がなく、引数にもなければ書けない
    let (bytes, diag) = with(Limits::default(), |cx| {
        zumen_bcf::write(&review, None, &options(), cx)
    });
    assert!(matches!(bytes, Err(Error::Invalid)));
    assert!(found(&diag).contains(&codes::BCF_VALUE_MISSING), "{diag:?}");

    let options = Options {
        kind: Some("Issue".to_string()),
        author: Some("zumen".to_string()),
        date: Some("2026-10-31T00:00:00Z".to_string()),
        ..options()
    };
    let (bytes, diag) = write(&review, None, &options).unwrap();
    let codes_found = found(&diag);
    for code in [
        codes::BCF_ORIGIN_DROPPED,
        codes::BCF_ELEMENT_UNSUPPORTED,
        codes::BCF_LOCATION_UNSUPPORTED,
        codes::BCF_VALUE_ASSUMED,
    ] {
        assert!(codes_found.contains(&code), "{code:?} {diag:?}");
    }
    let reasons: Vec<String> = diag
        .iter()
        .filter(|d| d.code() == codes::BCF_LOCATION_UNSUPPORTED)
        .filter_map(|d| match d.param("reason") {
            Some(zumen_base::Value::Text(text)) => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(reasons, ["camera", "target", "plane"]);
    assert!(entry(&bytes, "extensions.xml").is_some());
    let (_, again) = read(&bytes).unwrap();
    let topic = &again.topics[0];
    assert_eq!(topic.id, "a1b2c3d4-0000-8000-8000-000000000001");
    assert_eq!(topic.kind.as_deref(), Some("Issue"));
    assert_eq!(topic.author.as_deref(), Some("zumen"));
    assert_eq!(topic.status, Status::Open);
    assert_eq!(topic.locations.len(), 1);
    // UUID でない ID からは、ID から決めた Guid を作り、コメントの参照も同じにする
    let location = &topic.locations[0];
    assert_eq!(location.id.len(), 36);
    assert_eq!(topic.comments[0].location.as_ref(), Some(&location.id));
    let Place::Space {
        camera, selected, ..
    } = &location.place
    else {
        panic!();
    };
    assert_eq!(
        camera.as_ref().unwrap().projection,
        Projection::Orthographic(20.0)
    );
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, DOOR);
    // 同じ文書からは同じバイト列
    assert_eq!(write(&review, None, &options).unwrap().0, bytes);
}

#[test]
fn rejects_broken_archives_and_other_versions() {
    let cases: Vec<(Vec<u8>, Code)> = vec![
        (b"not a zip".to_vec(), codes::ZIP_ARCHIVE_INVALID),
        (
            zip(&[("a/markup.bcf", b"<Markup/>")]),
            codes::BCF_FILE_INVALID,
        ),
        (
            zip(&[("bcf.version", br#"<Version VersionId="2.1"/>"#)]),
            codes::BCF_VERSION_UNSUPPORTED,
        ),
        (
            zip(&[("bcf.version", b"<Other/>")]),
            codes::BCF_FILE_INVALID,
        ),
    ];
    for (bytes, code) in cases {
        let (result, diag) = with(Limits::default(), |cx| Bcf::read(&bytes, cx));
        assert!(result.is_err());
        assert!(found(&diag).contains(&code), "{code:?} {diag:?}");
    }
    // 壊れた markup と viewpoint は、写すときに失敗する
    let markup = markup();
    for (name, data) in [
        ("markup.bcf", "<Markup><Header/></Markup>".to_string()),
        (
            "markup.bcf",
            markup.replace("<Title>扉の幅がない</Title>", ""),
        ),
        ("viewpoint.bcfv", visinfo().replace("<X>1</X>", "<X>x</X>")),
        ("viewpoint.bcfv", "<Other/>".to_string()),
    ] {
        let visinfo = visinfo();
        let mut files: Vec<(String, Vec<u8>)> = vec![
            ("bcf.version".to_string(), VERSION_3.to_vec()),
            (format!("{TOPIC}/markup.bcf"), markup.clone().into_bytes()),
            (format!("{TOPIC}/viewpoint.bcfv"), visinfo.into_bytes()),
        ];
        for file in &mut files {
            if file.0.ends_with(name) {
                file.1 = data.clone().into_bytes();
            }
        }
        let refs: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_slice()))
            .collect();
        let bytes = zip(&refs);
        let (bcf, _) = with(Limits::default(), |cx| Bcf::read(&bytes, cx));
        let (result, diag) = with(Limits::default(), |cx| {
            zumen_bcf::review(&bcf.unwrap(), &target(), &Statuses::default(), None, cx)
        });
        assert!(result.is_err(), "{name}");
        assert!(
            found(&diag).contains(&codes::BCF_FILE_INVALID),
            "{name} {diag:?}"
        );
    }
}

#[test]
fn stops_at_the_budget() {
    let bytes = sample();
    for (kind, code) in [
        (Kind::DecodedBytes, codes::BUDGET_DECODED_BYTES_EXCEEDED),
        (Kind::Work, codes::BUDGET_WORK_EXCEEDED),
    ] {
        let (result, diag) = with(Limits::default().with(kind, Limit::Finite(2)), |cx| {
            Bcf::read(&bytes, cx)
        });
        assert!(matches!(result, Err(Error::Stopped(_))), "{kind:?}");
        assert!(found(&diag).contains(&code), "{kind:?} {diag:?}");
    }
    let (bcf, _) = read(&bytes).unwrap();
    let (result, diag) = with(Limits::default().with(Kind::Work, Limit::Finite(1)), |cx| {
        zumen_bcf::review(&bcf, &target(), &Statuses::default(), None, cx)
    });
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert!(found(&diag).contains(&codes::BUDGET_WORK_EXCEEDED));
}
