//! BCF 2.1 と 3.0 の bcfzip を読み、レビューの文書に写して書き戻す（決定 0214・0220）。bcfzip はテストの中で作る

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_bcf::{Bcf, Edition, Options, Statuses};
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

/// [`markup`] と同じ指摘を、BCF 2.1 の形で書いたもの。コメントと viewpoint は Markup の直下に並び、一覧を包まない
fn markup_2_1() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Markup>
  <Header><File IfcProject="0000000000000000000001" isExternal="true"><Filename>a.ifc</Filename></File></Header>
  <Topic Guid="{TOPIC}" TopicType="Issue" TopicStatus="In Progress" Custom="x">
    <ReferenceLink>https://example.com/1</ReferenceLink>
    <ReferenceLink>https://example.com/2</ReferenceLink>
    <Title>扉の幅がない</Title>
    <Priority>High</Priority>
    <Labels>Door</Labels>
    <CreationDate>2026-10-01T09:00:00+09:00</CreationDate>
    <CreationAuthor>a@example.com</CreationAuthor>
    <AssignedTo>b@example.com</AssignedTo>
    <Stage>Design</Stage>
    <Description>OverallWidth がない</Description>
    <BimSnippet SnippetType="IDS"><Reference>door.ids</Reference><ReferenceSchema>ids.xsd</ReferenceSchema></BimSnippet>
    <DocumentReference Guid="5d1e2f3a-4444-4b5c-8d6e-7f8091a2b304" isExternal="true"><ReferencedDocument>../Documents/d.pdf</ReferencedDocument></DocumentReference>
    <Vendor>keep</Vendor>
  </Topic>
  <Comment Guid="{COMMENT}"><Date>2026-10-02T10:00:00+09:00</Date><Author>b@example.com</Author><Comment>直します</Comment><Viewpoint Guid="{VIEW}"/><ModifiedDate>2026-10-03T10:00:00+09:00</ModifiedDate></Comment>
  <Viewpoints Guid="{VIEW}"><Viewpoint>viewpoint.bcfv</Viewpoint><Snapshot>snapshot.png</Snapshot></Viewpoints>
  <VendorMarkup>keep</VendorMarkup>
</Markup>
"#
    )
}

/// [`visinfo`] と同じ場所を、BCF 2.1 の形で書いたもの。色の部品を包まず、カメラに縦横比がなく、Bitmap が直下にある
fn visinfo_2_1() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<VisualizationInfo Guid="{VIEW}">
  <Components>
    <Selection><Component IfcGuid="{DOOR}"/><Component><AuthoringToolId>42</AuthoringToolId></Component></Selection>
    <Visibility DefaultVisibility="true"><Exceptions><Component IfcGuid="0000000000000000000034"/></Exceptions></Visibility>
    <Coloring><Color Color="ff0000"><Component IfcGuid="{DOOR}"/></Color></Coloring>
  </Components>
  <PerspectiveCamera>
    <CameraViewPoint><X>1</X><Y>2</Y><Z>3</Z></CameraViewPoint>
    <CameraDirection><X>0</X><Y>1</Y><Z>0</Z></CameraDirection>
    <CameraUpVector><X>0</X><Y>0</Y><Z>1</Z></CameraUpVector>
    <FieldOfView>60</FieldOfView>
  </PerspectiveCamera>
  <ClippingPlanes><ClippingPlane><Location><X>0</X><Y>0</Y><Z>1.5</Z></Location><Direction><X>0</X><Y>0</Y><Z>1</Z></Direction></ClippingPlane></ClippingPlanes>
  <Bitmap><Bitmap>PNG</Bitmap><Reference>logo.png</Reference><Location><X>0</X><Y>0</Y><Z>0</Z></Location><Normal><X>0</X><Y>0</Y><Z>1</Z></Normal><Up><X>0</X><Y>1</Y><Z>0</Z></Up><Height>1</Height></Bitmap>
</VisualizationInfo>
"#
    )
}

const VERSION_2_1: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?><Version VersionId="2.1"><DetailedVersion>2.1</DetailedVersion></Version>"#;
const PROJECT: &[u8] = br#"<ProjectExtension><Project ProjectId="0000000000000000000001"><Name>a</Name></Project><ExtensionSchema>extensions.xsd</ExtensionSchema></ProjectExtension>"#;

fn sample_2_1() -> Vec<u8> {
    let markup = markup_2_1();
    let visinfo = visinfo_2_1();
    zip(&[
        ("bcf.version", VERSION_2_1),
        ("project.bcfp", PROJECT),
        (&format!("{TOPIC}/"), b""),
        (&format!("{TOPIC}/markup.bcf"), markup.as_bytes()),
        (&format!("{TOPIC}/viewpoint.bcfv"), visinfo.as_bytes()),
        (&format!("{TOPIC}/snapshot.png"), SNAPSHOT),
    ])
}

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
        version: None,
    }
}

/// 扉だけにハッシュのあるバージョン
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
    assert_eq!(bcf.version, Edition::V3_0);
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

/// UTF-16 の原本は未変更ならそのまま保ち、未知の XML を落とす更新は成功扱いしない。
#[test]
fn preserves_utf16_topics_and_rejects_updates_that_cannot_keep_opaque_xml() {
    for little in [true, false] {
        let (mut bcf, mut review) = read(&sample()).unwrap();
        let text = markup().replace("encoding=\"UTF-8\"", "encoding=\"UTF-16\"");
        let mut encoded = if little {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in text.encode_utf16() {
            encoded.extend_from_slice(&if little {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        bcf.topics[0].markup = encoded.clone();
        let (bytes, diag) = write(&review, Some(&bcf), &options()).unwrap();
        assert!(diag.is_empty(), "{diag:?}");
        assert_eq!(
            entry(&bytes, &format!("{TOPIC}/markup.bcf")).unwrap(),
            encoded
        );
        review.topics[0].title = "変更後".into();
        let (result, diag) = with(Limits::default(), |cx| {
            zumen_bcf::write(&review, Some(&bcf), &options(), cx)
        });
        assert!(matches!(result, Err(Error::Unsupported)), "{diag:?}");
        assert!(
            diag.iter()
                .any(|d| d.code().name() == "bcf.encoding.unsupported"),
            "{diag:?}"
        );
    }
}

#[test]
fn preserves_shift_jis_topics_and_rejects_opaque_updates() {
    // ASCII の値だけで、追加の符号化の依存なしに Shift_JIS の文書を作る。
    let text = markup()
        .replace("encoding=\"UTF-8\"", "encoding=\"Shift_JIS\"")
        .replace("扉の幅がない", "Door width")
        .replace("OverallWidth がない", "Missing OverallWidth")
        .replace("直します", "Will fix");
    assert!(text.is_ascii());
    let mut bcf = read(&sample()).unwrap().0;
    bcf.topics[0].markup = text.as_bytes().to_vec();
    let (review, diag) = with(Limits::default(), |cx| {
        zumen_bcf::review(&bcf, &target(), &Statuses::default(), Some(&Model), cx)
    });
    assert!(diag.is_empty(), "{diag:?}");
    let mut review = review.unwrap();
    let (bytes, diag) = write(&review, Some(&bcf), &options()).unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(
        entry(&bytes, &format!("{TOPIC}/markup.bcf")).unwrap(),
        text.as_bytes()
    );
    review.topics[0].title = "Changed".into();
    let (result, diag) = with(Limits::default(), |cx| {
        zumen_bcf::write(&review, Some(&bcf), &options(), cx)
    });
    assert!(matches!(result, Err(Error::Unsupported)), "{diag:?}");
    assert_eq!(found(&diag), [codes::BCF_ENCODING_UNSUPPORTED]);
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
            zip(&[("bcf.version", br#"<Version VersionId="1.0"/>"#)]),
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

#[test]
fn reads_bcf_2_1_into_the_same_review_as_3_0() {
    let (bcf, review) = read(&sample_2_1()).unwrap();
    assert_eq!(bcf.version, Edition::V2_1);
    let names: Vec<&str> = bcf.files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["project.bcfp"]);
    let (_, review_3) = read(&sample()).unwrap();
    assert_eq!(review, review_3);
}

#[test]
fn writes_bcf_2_1_back_with_its_unmapped_parts() {
    let original = sample_2_1();
    let (bcf, mut review) = read(&original).unwrap();
    // バージョンを渡さなければ、元のバージョンで書く
    let (bytes, diag) = write(&review, Some(&bcf), &options()).unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    for name in [
        "project.bcfp",
        &format!("{TOPIC}/markup.bcf"),
        &format!("{TOPIC}/viewpoint.bcfv"),
        &format!("{TOPIC}/snapshot.png"),
    ] {
        assert_eq!(entry(&bytes, name), entry(&original, name), "{name}");
    }
    assert_eq!(
        String::from_utf8(entry(&bytes, "bcf.version").unwrap()).unwrap(),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Version VersionId=\"2.1\">\n  <DetailedVersion>2.1</DetailedVersion>\n</Version>\n"
    );

    review.topics[0].title = "扉の幅を入れる".to_string();
    review.topics[0].labels.push("Width".to_string());
    review.topics[0].comments.push(Comment {
        id: "9a8b7c6d-3333-4e5f-9071-82a3b4c5d604".to_string(),
        author: Some("a@example.com".to_string()),
        date: Some("2026-10-04T10:00:00+09:00".to_string()),
        text: "確かめました".to_string(),
        location: None,
        extra: Vec::new(),
    });
    let options = Options {
        version: Some(Edition::V2_1),
        ..options()
    };
    let (bytes, diag) = write(&review, Some(&bcf), &options).unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    let markup = String::from_utf8(entry(&bytes, &format!("{TOPIC}/markup.bcf")).unwrap()).unwrap();
    for kept in [
        "<Header><File IfcProject",
        "Custom=\"x\"",
        "<ReferenceLink>https://example.com/1</ReferenceLink>",
        "<ReferenceLink>https://example.com/2</ReferenceLink>",
        "<Stage>Design</Stage>",
        "<BimSnippet SnippetType=\"IDS\">",
        "<DocumentReference Guid=",
        "<Vendor>keep</Vendor>",
        "<VendorMarkup>keep</VendorMarkup>",
        "<Labels>Door</Labels>\n    <Labels>Width</Labels>",
        // 変えていないコメントと viewpoint は、元の要素のまま Markup の直下に書く
        "<ModifiedDate>2026-10-03T10:00:00+09:00</ModifiedDate>",
        "</Topic>\n  <Comment Guid=",
        "<Viewpoints Guid=",
        "<Comment>確かめました</Comment>",
    ] {
        assert!(markup.contains(kept), "{kept}\n{markup}");
    }
    assert!(!markup.contains("<Comments>"), "{markup}");
    assert_eq!(
        entry(&bytes, &format!("{TOPIC}/viewpoint.bcfv")).unwrap(),
        visinfo_2_1().as_bytes()
    );
    let (bcf_again, again) = read(&bytes).unwrap();
    assert_eq!(bcf_again.version, Edition::V2_1);
    assert_eq!(again.topics, review.topics);
}

#[test]
fn writes_bcf_2_1_from_the_review_and_reads_it_back() {
    let (_, review) = read(&sample()).unwrap();
    let options = Options {
        version: Some(Edition::V2_1),
        ..options()
    };
    let (bytes, diag) = write(&review, None, &options).unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    // 2.1 の拡張は任意の XSD なので書かない
    assert!(entry(&bytes, "extensions.xml").is_none());
    let visinfo =
        String::from_utf8(entry(&bytes, &format!("{TOPIC}/{VIEW}.bcfv")).unwrap()).unwrap();
    assert!(!visinfo.contains("AspectRatio"), "{visinfo}");
    assert!(
        visinfo.contains("<Color Color=\"FF0000\">\n        <Component IfcGuid="),
        "{visinfo}"
    );
    let (bcf, again) = read(&bytes).unwrap();
    assert_eq!(bcf.version, Edition::V2_1);
    assert_eq!(again, review);
    // 同じ文書からは同じバイト列
    assert_eq!(write(&review, None, &options).unwrap().0, bytes);
    // 2.1 から読んだ文書を 3.0 で書いても、同じ指摘になる
    let (bytes_3, _) = write(&again, None, &options_3()).unwrap();
    let (bcf_3, again_3) = read(&bytes_3).unwrap();
    assert_eq!(bcf_3.version, Edition::V3_0);
    assert_eq!(again_3, review);
}

fn options_3() -> Options {
    Options {
        version: Some(Edition::V3_0),
        ..options()
    }
}

#[test]
fn does_not_write_back_in_another_version() {
    for (original, version) in [(sample(), Edition::V2_1), (sample_2_1(), Edition::V3_0)] {
        let (bcf, review) = read(&original).unwrap();
        let options = Options {
            version: Some(version),
            ..options()
        };
        let (result, diag) = with(Limits::default(), |cx| {
            zumen_bcf::write(&review, Some(&bcf), &options, cx)
        });
        assert!(matches!(result, Err(Error::Invalid)));
        assert_eq!(found(&diag), [codes::BCF_CONVERSION_UNSUPPORTED]);
    }
}
