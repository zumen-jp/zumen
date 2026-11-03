//! bcfzip を読み、レビューの文書に写す

use zumen_base::{Diagnostic, Kind, codes};
use zumen_review::{
    Camera, Clip, Colored, Comment, Location, Place, Projection, Reference, Review, Status, Target,
    Topic, Version,
};
use zumen_sdk::{Cx, Error};
use zumen_xml::Element;

use crate::xml::Xml;
use crate::{Bcf, Edition, Folder, Statuses};

pub(crate) fn invalid(cx: &mut Cx, name: &str, near: &str) -> Error {
    cx.report(
        Diagnostic::new(codes::BCF_FILE_INVALID)
            .with("name", name)
            .with("near", near),
    );
    Error::Invalid
}

fn work(cx: &mut Cx) -> Result<(), Error> {
    cx.check(Kind::Work, 1).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}

pub(crate) fn archive(src: &[u8], cx: &mut Cx) -> Result<Bcf, Error> {
    let archive = zumen_zip::read(src, cx)?;
    let mut version = None;
    let mut folders: Vec<Folder> = Vec::new();
    let mut files = Vec::new();
    for entry in archive.entries() {
        if entry.is_directory() {
            continue;
        }
        work(cx)?;
        // 読み手は `\` の区切りも受ける
        let name = entry.name().replace('\\', "/");
        let data = archive.extract(entry, cx)?;
        if name == "bcf.version" {
            version = Some(data);
            continue;
        }
        match name.split_once('/') {
            Some((folder, rest)) if !rest.contains('/') && !rest.is_empty() => {
                let index = match folders.iter().position(|f| f.name == folder) {
                    Some(index) => index,
                    None => {
                        folders.push(Folder {
                            name: folder.to_string(),
                            ..Folder::default()
                        });
                        folders.len() - 1
                    }
                };
                if let Some(found) = folders.get_mut(index) {
                    if rest == "markup.bcf" {
                        found.markup = data;
                    } else {
                        found.files.push((rest.to_string(), data));
                    }
                }
            }
            _ => files.push((name, data)),
        }
    }
    let Some(version) = version else {
        return Err(invalid(cx, "bcf.version", "Version"));
    };
    let version = {
        let doc = zumen_xml::read(&version, cx)?;
        let xml = Xml { doc: &doc };
        match xml.root() {
            Some(root) if xml.local(root) == b"Version" => {
                xml.attribute(root, b"VersionId").unwrap_or_default()
            }
            _ => return Err(invalid(cx, "bcf.version", "Version")),
        }
    };
    let Some(version) = Edition::from_id(&version) else {
        cx.report(
            Diagnostic::new(codes::BCF_VERSION_UNSUPPORTED).with("version", version.as_str()),
        );
        return Err(Error::Invalid);
    };
    // markup.bcf のないフォルダー（Documents など）は、ほかの項目として保つ
    let mut topics = Vec::new();
    for folder in folders {
        if folder.markup.is_empty() {
            for (name, data) in folder.files {
                files.push((format!("{}/{name}", folder.name), data));
            }
        } else {
            topics.push(folder);
        }
    }
    Ok(Bcf {
        version,
        topics,
        files,
    })
}

/// bcfzip の topic を、`target` を対象にした指摘にする。要素の参照のハッシュは、`version` があればその版の
/// 要素のハッシュで埋め、なければなしにする。状態は `statuses` で写し、表にない文字列は `open` と
/// `status_label` にする
pub fn review(
    bcf: &Bcf,
    target: &Target,
    statuses: &Statuses,
    version: Option<&dyn Version>,
    cx: &mut Cx,
) -> Result<Review, Error> {
    let mut topics = Vec::with_capacity(bcf.topics.len());
    for folder in &bcf.topics {
        topics.push(topic(
            folder,
            bcf.version,
            &target.id,
            statuses,
            version,
            cx,
        )?);
    }
    Ok(Review {
        targets: vec![target.clone()],
        topics,
        markups: Vec::new(),
        extra: Vec::new(),
    })
}

/// Guid を小文字にする
pub(crate) fn guid(text: &str) -> String {
    text.trim().to_ascii_lowercase()
}

/// `parent` の子の `item` のすべて。`wrapper` があれば、その子の中の `item`
fn listed<'d>(
    xml: &Xml<'d>,
    parent: &Element,
    wrapper: Option<&[u8]>,
    item: &[u8],
) -> Vec<&'d Element> {
    match wrapper {
        Some(wrapper) => xml
            .child(parent, wrapper)
            .map(|list| xml.all(list, item))
            .unwrap_or_default(),
        None => xml.all(parent, item),
    }
}

/// 版ごとの、一覧の要素の置き場所。3.0 は一覧を包む要素の中に並べ、2.1 は包まずに並べる
impl Edition {
    /// コメント。3.0 は Topic の `Comments` の中、2.1 は Markup の直下の `Comment`
    pub(crate) fn comments<'d>(
        self,
        xml: &Xml<'d>,
        markup: &Element,
        topic: &Element,
    ) -> Vec<&'d Element> {
        match self {
            Self::V2_1 => listed(xml, markup, None, b"Comment"),
            Self::V3_0 => listed(xml, topic, Some(b"Comments"), b"Comment"),
        }
    }

    /// viewpoint の参照。3.0 は Topic の `Viewpoints` の中の `ViewPoint`、2.1 は Markup の直下の `Viewpoints`
    pub(crate) fn viewpoints<'d>(
        self,
        xml: &Xml<'d>,
        markup: &Element,
        topic: &Element,
    ) -> Vec<&'d Element> {
        match self {
            Self::V2_1 => listed(xml, markup, None, b"Viewpoints"),
            Self::V3_0 => listed(xml, topic, Some(b"Viewpoints"), b"ViewPoint"),
        }
    }

    /// ラベル。3.0 は `Labels` の中の `Label`、2.1 は 1 つずつの `Labels`
    fn labels<'d>(self, xml: &Xml<'d>, topic: &Element) -> Vec<&'d Element> {
        match self {
            Self::V2_1 => listed(xml, topic, None, b"Labels"),
            Self::V3_0 => listed(xml, topic, Some(b"Labels"), b"Label"),
        }
    }

    /// 関連する topic。3.0 は `RelatedTopics` の中、2.1 は Topic の直下
    fn related<'d>(self, xml: &Xml<'d>, topic: &Element) -> Vec<&'d Element> {
        match self {
            Self::V2_1 => listed(xml, topic, None, b"RelatedTopic"),
            Self::V3_0 => listed(xml, topic, Some(b"RelatedTopics"), b"RelatedTopic"),
        }
    }

    /// 色を付ける部品。3.0 は Color の `Components` の中、2.1 は Color の直下
    fn colored<'d>(self, xml: &Xml<'d>, color: &Element) -> Vec<&'d Element> {
        match self {
            Self::V2_1 => listed(xml, color, None, b"Component"),
            Self::V3_0 => listed(xml, color, Some(b"Components"), b"Component"),
        }
    }
}

pub(crate) fn topic(
    folder: &Folder,
    edition: Edition,
    target: &str,
    statuses: &Statuses,
    version: Option<&dyn Version>,
    cx: &mut Cx,
) -> Result<Topic, Error> {
    let name = format!("{}/markup.bcf", folder.name);
    let doc = zumen_xml::read(&folder.markup, cx)?;
    let xml = Xml { doc: &doc };
    let Some(markup) = xml.root().filter(|root| xml.local(root) == b"Markup") else {
        return Err(invalid(cx, &name, "Markup"));
    };
    let Some(element) = xml.child(markup, b"Topic") else {
        return Err(invalid(cx, &name, "Topic"));
    };
    let Some(id) = xml.attribute(element, b"Guid") else {
        return Err(invalid(cx, &name, "Guid"));
    };
    let Some(title) = xml.child_text(element, b"Title") else {
        return Err(invalid(cx, &name, "Title"));
    };
    let raw_status = xml.attribute(element, b"TopicStatus").unwrap_or_default();
    let (status, status_label) = match statuses.status(&raw_status) {
        Some(status) => (status, None),
        None if raw_status.is_empty() => (Status::Open, None),
        None => (Status::Open, Some(raw_status)),
    };
    let labels = edition
        .labels(&xml, element)
        .into_iter()
        .map(|label| xml.text(label))
        .filter(|label| !label.is_empty())
        .collect();
    let related = edition
        .related(&xml, element)
        .into_iter()
        .filter_map(|r| xml.attribute(r, b"Guid"))
        .map(|g| guid(&g))
        .collect();
    let mut comments = Vec::new();
    for comment in edition.comments(&xml, markup, element) {
        work(cx)?;
        let Some(id) = xml.attribute(comment, b"Guid") else {
            return Err(invalid(cx, &name, "Comment"));
        };
        comments.push(Comment {
            id: guid(&id),
            author: xml.child_text(comment, b"Author"),
            date: xml.child_text(comment, b"Date"),
            text: xml.child_text(comment, b"Comment").unwrap_or_default(),
            location: xml
                .child(comment, b"Viewpoint")
                .and_then(|v| xml.attribute(v, b"Guid"))
                .map(|g| guid(&g)),
            extra: Vec::new(),
        });
    }
    let mut locations = Vec::new();
    for point in edition.viewpoints(&xml, markup, element) {
        work(cx)?;
        let Some(id) = xml.attribute(point, b"Guid") else {
            return Err(invalid(cx, &name, "ViewPoint"));
        };
        let place = match xml.child_text(point, b"Viewpoint") {
            Some(file) => match folder.files.iter().find(|(n, _)| *n == file) {
                Some((_, data)) => {
                    let name = format!("{}/{file}", folder.name);
                    space(data, &name, edition, version, cx)?
                }
                None => {
                    return Err(invalid(cx, &format!("{}/{file}", folder.name), "Viewpoint"));
                }
            },
            None => empty_space(),
        };
        locations.push(Location {
            id: guid(&id),
            target: target.to_string(),
            place,
            carried: None,
            extra: Vec::new(),
        });
    }
    Ok(Topic {
        id: guid(&id),
        title,
        description: xml.child_text(element, b"Description"),
        status,
        status_label,
        kind: xml
            .attribute(element, b"TopicType")
            .filter(|t| !t.is_empty()),
        priority: xml.child_text(element, b"Priority"),
        labels,
        assignee: xml.child_text(element, b"AssignedTo"),
        due: xml.child_text(element, b"DueDate"),
        author: xml.child_text(element, b"CreationAuthor"),
        created: xml.child_text(element, b"CreationDate"),
        modified_by: xml.child_text(element, b"ModifiedAuthor"),
        modified: xml.child_text(element, b"ModifiedDate"),
        related,
        comments,
        locations,
        origin: None,
        extra: Vec::new(),
    })
}

fn empty_space() -> Place {
    Place::Space {
        camera: None,
        selected: Vec::new(),
        visible: None,
        exceptions: Vec::new(),
        colored: Vec::new(),
        clips: Vec::new(),
    }
}

/// viewpoint を 3D の場所にする
pub(crate) fn space(
    data: &[u8],
    name: &str,
    edition: Edition,
    version: Option<&dyn Version>,
    cx: &mut Cx,
) -> Result<Place, Error> {
    let doc = zumen_xml::read(data, cx)?;
    let xml = Xml { doc: &doc };
    let Some(root) = xml.root().filter(|r| xml.local(r) == b"VisualizationInfo") else {
        return Err(invalid(cx, name, "VisualizationInfo"));
    };
    let mut place = empty_space();
    let Place::Space {
        camera,
        selected,
        visible,
        exceptions,
        colored,
        clips,
    } = &mut place
    else {
        return Ok(place);
    };
    let components = |list: &[&Element], cx: &mut Cx| -> Result<Vec<Reference>, Error> {
        let mut out = Vec::new();
        for component in list {
            work(cx)?;
            // IfcGuid のない要素（AuthoringToolId だけのもの）は、建物モデルの ID で引けないので写さない
            if let Some(id) = xml
                .attribute(component, b"IfcGuid")
                .filter(|g| !g.is_empty())
            {
                out.push(Reference {
                    hash: version.and_then(|v| v.element(&id)),
                    id,
                });
            }
        }
        Ok(out)
    };
    if let Some(list) = xml.child(root, b"Components") {
        *selected = components(&listed(&xml, list, Some(b"Selection"), b"Component"), cx)?;
        if let Some(visibility) = xml.child(list, b"Visibility") {
            *visible =
                Some(xml.attribute(visibility, b"DefaultVisibility").as_deref() == Some("true"));
            *exceptions = components(
                &listed(&xml, visibility, Some(b"Exceptions"), b"Component"),
                cx,
            )?;
        }
        if let Some(coloring) = xml.child(list, b"Coloring") {
            for color in xml.all(coloring, b"Color") {
                let Some(value) = xml.attribute(color, b"Color") else {
                    return Err(invalid(cx, name, "Color"));
                };
                colored.push(Colored {
                    color: format!("#{}", value.to_ascii_uppercase()),
                    elements: components(&edition.colored(&xml, color), cx)?,
                });
            }
        }
    }
    let vector = |element: Option<&Element>, near: &str, cx: &mut Cx| -> Result<[f64; 3], Error> {
        let mut out = [0.0; 3];
        for (slot, axis) in out.iter_mut().zip([b"X", b"Y", b"Z"]) {
            let value = element
                .and_then(|e| xml.child_text(e, axis))
                .and_then(|t| t.parse::<f64>().ok())
                .filter(|v| v.is_finite());
            match value {
                Some(value) => *slot = value,
                None => return Err(invalid(cx, name, near)),
            }
        }
        Ok(out)
    };
    let scalar = |element: &Element, local: &[u8], cx: &mut Cx| -> Result<f64, Error> {
        xml.child_text(element, local)
            .and_then(|t| t.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .ok_or_else(|| invalid(cx, name, &String::from_utf8_lossy(local)))
    };
    let perspective = xml.child(root, b"PerspectiveCamera");
    let orthogonal = xml.child(root, b"OrthogonalCamera");
    if let Some(element) = perspective.or(orthogonal) {
        let projection = if perspective.is_some() {
            Projection::Perspective(scalar(element, b"FieldOfView", cx)?)
        } else {
            Projection::Orthographic(scalar(element, b"ViewToWorldScale", cx)?)
        };
        *camera = Some(Camera {
            projection,
            position: vector(
                xml.child(element, b"CameraViewPoint"),
                "CameraViewPoint",
                cx,
            )?,
            direction: vector(
                xml.child(element, b"CameraDirection"),
                "CameraDirection",
                cx,
            )?,
            up: vector(xml.child(element, b"CameraUpVector"), "CameraUpVector", cx)?,
        });
    }
    if let Some(planes) = xml.child(root, b"ClippingPlanes") {
        for plane in xml.all(planes, b"ClippingPlane") {
            work(cx)?;
            clips.push(Clip {
                point: vector(xml.child(plane, b"Location"), "Location", cx)?,
                normal: vector(xml.child(plane, b"Direction"), "Direction", cx)?,
            });
        }
    }
    Ok(place)
}
