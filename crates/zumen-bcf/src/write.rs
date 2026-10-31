//! レビューの文書を bcfzip に書く

use zumen_base::{Diagnostic, Kind, codes};
use zumen_review::{Camera, Location, Place, Projection, Reference, Review, Topic};
use zumen_sdk::{Cx, Error};
use zumen_xml::Element;

use crate::read::{self, guid};
use crate::xml::{Writer, Xml};
use crate::{Bcf, Folder, Options};

/// 書いた viewpoint のアスペクト比。レビューの文書はカメラの縦横の比を持たないので、BCF 2.1 の読み手の
/// 既定と同じ 1 にする
const ASPECT_RATIO: f64 = 1.0;

fn work(cx: &mut Cx) -> Result<(), Error> {
    cx.check(Kind::Work, 1).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}

/// レビューの文書を、BCF 3.0 の bcfzip に書く。
///
/// - `options.target` を対象にした 3D の場所を viewpoint にする。2D の場所・ほかの対象の場所・カメラのない場所と
///   図形は書かずに診断する。
/// - 要素の参照は、IFC の GlobalId（22 字）の ID だけを書く。ほかの ID（`i` と番号）は除いて診断する。
/// - `original` に同じ Guid の topic があり、写した指摘が同じなら、フォルダーを元のバイト列のまま書く。変えた
///   topic でも、レビューの文書に写さない要素と、知らない要素・属性は、元の topic から写す。
/// - 種類・作者・日時のない指摘とコメントには、`options` の値を書いて診断する。値がなければ失敗する。
pub fn write(
    review: &Review,
    original: Option<&Bcf>,
    options: &Options,
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    for markup in &review.markups {
        cx.report(Diagnostic::new(codes::BCF_MARKUP_UNSUPPORTED).with("id", markup.id.as_str()));
    }
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut version = Writer::new();
    version.empty("Version", &[("VersionId", "3.0")]);
    entries.push(("bcf.version".to_string(), version.out));
    match original {
        Some(bcf) => entries.extend(bcf.files.iter().cloned()),
        None => entries.push(("extensions.xml".to_string(), extensions(review, options))),
    }
    for topic in &review.topics {
        work(cx)?;
        let id = bcf_guid(&topic.id);
        let written = writable(topic, &id, options, cx);
        let folder = match original {
            // フォルダーの名前は topic の Guid
            Some(bcf) => bcf.topics.iter().find(|f| guid(&f.name) == id),
            None => None,
        };
        let before = match folder {
            Some(folder) => Some(read::topic(
                folder,
                &options.target,
                &options.statuses,
                None,
                cx,
            )?),
            None => None,
        };
        entries.push((format!("{id}/"), Vec::new()));
        if let (Some(folder), Some(before)) = (folder, &before)
            && *before == written
        {
            entries.push((format!("{id}/markup.bcf"), folder.markup.clone()));
            for (name, data) in &folder.files {
                entries.push((format!("{id}/{name}"), data.clone()));
            }
            continue;
        }
        let files = markup(&written, folder, options, cx)?;
        for (name, data) in files {
            entries.push((format!("{id}/{name}"), data));
        }
    }
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, data)| (name.as_str(), data.as_slice()))
        .collect();
    zumen_zip::write(&refs).ok_or_else(|| {
        cx.report(Diagnostic::new(codes::BCF_SIZE_UNSUPPORTED));
        Error::Invalid
    })
}

/// UUID の形か
fn is_guid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// BCF の Guid。UUID の形ならそのまま小文字にし、ほかの ID からは、ID から決めた UUID にする
fn bcf_guid(id: &str) -> String {
    if is_guid(id) {
        return guid(id);
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"zumen-bcf/guid\0");
    hasher.update(id.as_bytes());
    let mut bytes = [0u8; 16];
    for (slot, byte) in bytes.iter_mut().zip(hasher.finalize().as_bytes()) {
        *slot = *byte;
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut text = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        text.push_str(&format!("{byte:02x}"));
    }
    text
}

/// IFC の GlobalId（22 字の `0-9A-Za-z_$`）か
fn is_ifc_guid(id: &str) -> bool {
    id.len() == 22
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
}

/// 書ける形にした指摘。BCF に書けない場所と参照を除き、ID を Guid にし、ハッシュ・移した状態・出所を除く。
/// 元の topic を写したものと比べる
fn writable(topic: &Topic, id: &str, options: &Options, cx: &mut Cx) -> Topic {
    if topic.origin.is_some() {
        cx.report(Diagnostic::new(codes::BCF_ORIGIN_DROPPED).with("topic", id));
    }
    let refs = |list: &[Reference], cx: &mut Cx| -> Vec<Reference> {
        list.iter()
            .filter(|r| {
                let ok = is_ifc_guid(&r.id);
                if !ok {
                    cx.report(
                        Diagnostic::new(codes::BCF_ELEMENT_UNSUPPORTED)
                            .with("topic", id)
                            .with("id", r.id.as_str()),
                    );
                }
                ok
            })
            .map(|r| Reference {
                id: r.id.clone(),
                hash: None,
            })
            .collect()
    };
    let mut locations = Vec::new();
    for location in &topic.locations {
        let reason = match &location.place {
            Place::Plane { .. } => Some("plane"),
            Place::Space { .. } if location.target != options.target => Some("target"),
            Place::Space { camera: None, .. } => Some("camera"),
            Place::Space { .. } => None,
        };
        if let Some(reason) = reason {
            cx.report(
                Diagnostic::new(codes::BCF_LOCATION_UNSUPPORTED)
                    .with("topic", id)
                    .with("location", location.id.as_str())
                    .with("reason", reason),
            );
            continue;
        }
        let Place::Space {
            camera,
            selected,
            visible,
            exceptions,
            colored,
            clips,
        } = &location.place
        else {
            continue;
        };
        let place = Place::Space {
            camera: camera.clone(),
            selected: refs(selected, cx),
            visible: *visible,
            exceptions: refs(exceptions, cx),
            colored: colored
                .iter()
                .map(|c| zumen_review::Colored {
                    color: c.color.to_ascii_uppercase(),
                    elements: refs(&c.elements, cx),
                })
                .filter(|c| !c.elements.is_empty())
                .collect(),
            clips: clips.clone(),
        };
        locations.push(Location {
            id: bcf_guid(&location.id),
            target: options.target.clone(),
            place,
            carried: None,
            extra: Vec::new(),
        });
    }
    let comments = topic
        .comments
        .iter()
        .map(|c| zumen_review::Comment {
            id: bcf_guid(&c.id),
            author: c.author.clone(),
            date: c.date.clone(),
            text: c.text.clone(),
            location: c
                .location
                .as_ref()
                .map(|l| bcf_guid(l))
                .filter(|l| locations.iter().any(|w| w.id == *l)),
            extra: Vec::new(),
        })
        .collect();
    Topic {
        id: id.to_string(),
        title: topic.title.clone(),
        description: topic.description.clone(),
        status: topic.status,
        status_label: topic.status_label.clone(),
        kind: topic.kind.clone(),
        priority: topic.priority.clone(),
        labels: topic.labels.clone(),
        assignee: topic.assignee.clone(),
        due: topic.due.clone(),
        author: topic.author.clone(),
        created: topic.created.clone(),
        modified_by: topic.modified_by.clone(),
        modified: topic.modified.clone(),
        related: topic.related.iter().map(|r| bcf_guid(r)).collect(),
        comments,
        locations,
        origin: None,
        extra: Vec::new(),
    }
}

/// 値か、なければ `options` の値。どちらもなければ `bcf.value.missing` で失敗する
fn required(
    value: Option<&str>,
    fallback: Option<&str>,
    topic: &str,
    field: &str,
    cx: &mut Cx,
) -> Result<String, Error> {
    if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
        return Ok(value.to_string());
    }
    match fallback.filter(|v| !v.trim().is_empty()) {
        Some(value) => {
            cx.report(
                Diagnostic::new(codes::BCF_VALUE_ASSUMED)
                    .with("topic", topic)
                    .with("field", field),
            );
            Ok(value.to_string())
        }
        None => {
            cx.report(
                Diagnostic::new(codes::BCF_VALUE_MISSING)
                    .with("topic", topic)
                    .with("field", field),
            );
            Err(Error::Invalid)
        }
    }
}

/// 元の markup の、写さない要素と知らない要素
struct Kept<'d> {
    xml: Option<Xml<'d>>,
    markup: Option<&'d Element>,
    topic: Option<&'d Element>,
}

/// Topic の子のうち、レビューの文書に写すもの
const MAPPED: &[&[u8]] = &[
    b"Title",
    b"Priority",
    b"Labels",
    b"CreationDate",
    b"CreationAuthor",
    b"ModifiedDate",
    b"ModifiedAuthor",
    b"DueDate",
    b"AssignedTo",
    b"Description",
    b"RelatedTopics",
    b"Comments",
    b"Viewpoints",
];

/// Topic の子のうち、写さずに元のまま保つもの
const KEPT: &[&[u8]] = &[
    b"ReferenceLinks",
    b"Index",
    b"Stage",
    b"BimSnippet",
    b"DocumentReferences",
];

impl<'d> Kept<'d> {
    fn raw(&self, local: &[u8], out: &mut Writer) {
        if let (Some(xml), Some(topic)) = (&self.xml, self.topic)
            && let Some(element) = xml.child(topic, local)
            && let Some(raw) = xml.raw(element)
        {
            out.raw(raw);
        }
    }

    /// 知らない子（写すものと保つもののほか）
    fn unknown(&self, parent: Option<&'d Element>, known: &[&[u8]], out: &mut Writer) {
        let (Some(xml), Some(parent)) = (&self.xml, parent) else {
            return;
        };
        for child in xml.children(parent) {
            if !known.contains(&xml.local(child))
                && let Some(raw) = xml.raw(child)
            {
                out.raw(raw);
            }
        }
    }
}

/// 指摘の markup.bcf と viewpoint を書く。フォルダーの中の名前とデータ
fn markup(
    topic: &Topic,
    folder: Option<&Folder>,
    options: &Options,
    cx: &mut Cx,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    let id = topic.id.as_str();
    if topic.title.trim().is_empty() {
        cx.report(
            Diagnostic::new(codes::BCF_VALUE_MISSING)
                .with("topic", id)
                .with("field", "Title"),
        );
        return Err(Error::Invalid);
    }
    let kind = required(
        topic.kind.as_deref(),
        options.kind.as_deref(),
        id,
        "TopicType",
        cx,
    )?;
    let created = required(
        topic.created.as_deref(),
        options.date.as_deref(),
        id,
        "CreationDate",
        cx,
    )?;
    let author = required(
        topic.author.as_deref(),
        options.author.as_deref(),
        id,
        "CreationAuthor",
        cx,
    )?;
    // 表にない状態の文字列は、読んだときに open にしたものなので、open のままなら元の文字列を書く
    let status = match (&topic.status_label, topic.status) {
        (Some(label), zumen_review::Status::Open) if options.statuses.status(label).is_none() => {
            label.clone()
        }
        _ => match options.statuses.text(topic.status) {
            Some(text) => text.to_string(),
            None => {
                cx.report(
                    Diagnostic::new(codes::BCF_VALUE_MISSING)
                        .with("topic", id)
                        .with("field", "TopicStatus"),
                );
                return Err(Error::Invalid);
            }
        },
    };
    let doc = match folder {
        Some(folder) => Some(zumen_xml::read(&folder.markup, cx)?),
        None => None,
    };
    let xml = doc.as_ref().map(|doc| Xml { doc });
    let markup_element = xml.as_ref().and_then(|x| x.root());
    let topic_element = match (&xml, markup_element) {
        (Some(x), Some(m)) => x.child(m, b"Topic"),
        _ => None,
    };
    let kept = Kept {
        xml,
        markup: markup_element,
        topic: topic_element,
    };
    let mut files = Vec::new();
    let mut out = Writer::new();
    out.open("Markup", &[], &[]);
    kept.raw_child_of_markup(b"Header", &mut out);
    let raw_attributes = match (&kept.xml, kept.topic) {
        (Some(x), Some(t)) => x.raw_attributes(t, &[b"Guid", b"TopicType", b"TopicStatus"]),
        _ => Vec::new(),
    };
    out.open(
        "Topic",
        &[("Guid", id), ("TopicType", &kind), ("TopicStatus", &status)],
        &raw_attributes,
    );
    kept.raw(b"ReferenceLinks", &mut out);
    out.text("Title", &topic.title);
    out.optional("Priority", topic.priority.as_deref());
    kept.raw(b"Index", &mut out);
    let labels: Vec<&String> = topic
        .labels
        .iter()
        .filter(|l| !l.trim().is_empty())
        .collect();
    if !labels.is_empty() {
        out.open("Labels", &[], &[]);
        for label in labels {
            out.text("Label", label);
        }
        out.close("Labels");
    }
    out.text("CreationDate", &created);
    out.text("CreationAuthor", &author);
    out.optional("ModifiedDate", topic.modified.as_deref());
    out.optional("ModifiedAuthor", topic.modified_by.as_deref());
    out.optional("DueDate", topic.due.as_deref());
    out.optional("AssignedTo", topic.assignee.as_deref());
    kept.raw(b"Stage", &mut out);
    out.optional("Description", topic.description.as_deref());
    kept.raw(b"BimSnippet", &mut out);
    kept.raw(b"DocumentReferences", &mut out);
    if !topic.related.is_empty() {
        out.open("RelatedTopics", &[], &[]);
        for related in &topic.related {
            out.empty("RelatedTopic", &[("Guid", related)]);
        }
        out.close("RelatedTopics");
    }
    let (before, before_comments) = match folder {
        Some(folder) => {
            let read = read::topic(folder, &options.target, &options.statuses, None, cx)?;
            (read.locations, read.comments)
        }
        None => (Vec::new(), Vec::new()),
    };
    if !topic.comments.is_empty() {
        out.open("Comments", &[], &[]);
        for comment in &topic.comments {
            work(cx)?;
            // 変えていないコメントは、元の要素のまま書く
            if let Some(raw) = unchanged_comment(&kept, &before_comments, comment) {
                out.raw(raw);
                continue;
            }
            let date = required(
                comment.date.as_deref(),
                options.date.as_deref(),
                id,
                "Comment/Date",
                cx,
            )?;
            let author = required(
                comment.author.as_deref(),
                options.author.as_deref(),
                id,
                "Comment/Author",
                cx,
            )?;
            out.open("Comment", &[("Guid", &comment.id)], &[]);
            out.text("Date", &date);
            out.text("Author", &author);
            out.optional("Comment", Some(comment.text.as_str()));
            if let Some(location) = &comment.location {
                out.empty("Viewpoint", &[("Guid", location)]);
            }
            out.close("Comment");
        }
        out.close("Comments");
    }
    // 元の viewpoint のうち、変えていないものはファイルごと保つ
    let mut dropped = Vec::new();
    if let (Some(x), Some(t)) = (&kept.xml, kept.topic)
        && let Some(list) = x.child(t, b"Viewpoints")
    {
        for point in x.all(list, b"ViewPoint") {
            let guid = x.attribute(point, b"Guid").map(|g| read::guid(&g));
            let unchanged = guid.as_ref().is_some_and(|g| {
                let old = before.iter().find(|l| l.id == *g);
                let new = topic.locations.iter().find(|l| l.id == *g);
                matches!((old, new), (Some(o), Some(n)) if o.place == n.place)
            });
            if !unchanged {
                dropped.extend(x.child_text(point, b"Viewpoint"));
                dropped.extend(x.child_text(point, b"Snapshot"));
            }
        }
    }
    if !topic.locations.is_empty() {
        out.open("Viewpoints", &[], &[]);
        for location in &topic.locations {
            work(cx)?;
            let old = before.iter().find(|l| l.id == location.id);
            let raw = match (&kept.xml, kept.topic, old) {
                (Some(x), Some(t), Some(o)) if o.place == location.place => x
                    .child(t, b"Viewpoints")
                    .and_then(|list| {
                        x.all(list, b"ViewPoint").into_iter().find(|p| {
                            x.attribute(p, b"Guid").map(|g| read::guid(&g)).as_deref()
                                == Some(location.id.as_str())
                        })
                    })
                    .and_then(|p| x.raw(p)),
                _ => None,
            };
            match raw {
                Some(raw) => out.raw(raw),
                None => {
                    let name = format!("{}.bcfv", location.id);
                    out.open("ViewPoint", &[("Guid", &location.id)], &[]);
                    out.text("Viewpoint", &name);
                    out.close("ViewPoint");
                    files.push((name, visinfo(location)));
                }
            }
        }
        out.close("Viewpoints");
    }
    kept.unknown(kept.topic, &[MAPPED, KEPT].concat(), &mut out);
    out.close("Topic");
    kept.unknown(kept.markup, &[b"Header".as_slice(), b"Topic"], &mut out);
    out.close("Markup");
    let mut all = vec![("markup.bcf".to_string(), out.out)];
    if let Some(folder) = folder {
        for (name, data) in &folder.files {
            if !dropped.contains(name) && !files.iter().any(|(n, _)| n == name) {
                all.push((name.clone(), data.clone()));
            }
        }
    }
    all.extend(files);
    Ok(all)
}

impl<'d> Kept<'d> {
    fn raw_child_of_markup(&self, local: &[u8], out: &mut Writer) {
        if let (Some(xml), Some(markup)) = (&self.xml, self.markup)
            && let Some(element) = xml.child(markup, local)
            && let Some(raw) = xml.raw(element)
        {
            out.raw(raw);
        }
    }
}

fn point(out: &mut Writer, name: &str, value: [f64; 3]) {
    out.open(name, &[], &[]);
    out.number("X", value[0]);
    out.number("Y", value[1]);
    out.number("Z", value[2]);
    out.close(name);
}

fn components(out: &mut Writer, name: &str, list: &[Reference]) {
    out.open(name, &[], &[]);
    for reference in list {
        out.empty("Component", &[("IfcGuid", &reference.id)]);
    }
    out.close(name);
}

/// 3D の場所の viewpoint
fn visinfo(location: &Location) -> Vec<u8> {
    let mut out = Writer::new();
    out.open("VisualizationInfo", &[("Guid", &location.id)], &[]);
    if let Place::Space {
        camera,
        selected,
        visible,
        exceptions,
        colored,
        clips,
    } = &location.place
    {
        if !selected.is_empty()
            || visible.is_some()
            || !exceptions.is_empty()
            || !colored.is_empty()
        {
            out.open("Components", &[], &[]);
            if !selected.is_empty() {
                components(&mut out, "Selection", selected);
            }
            if visible.is_some() || !exceptions.is_empty() {
                let default = if visible.unwrap_or(false) {
                    "true"
                } else {
                    "false"
                };
                out.open("Visibility", &[("DefaultVisibility", default)], &[]);
                if !exceptions.is_empty() {
                    components(&mut out, "Exceptions", exceptions);
                }
                out.close("Visibility");
            }
            if !colored.is_empty() {
                out.open("Coloring", &[], &[]);
                for color in colored {
                    let value = color.color.trim_start_matches('#');
                    out.open("Color", &[("Color", value)], &[]);
                    components(&mut out, "Components", &color.elements);
                    out.close("Color");
                }
                out.close("Coloring");
            }
            out.close("Components");
        }
        if let Some(Camera {
            projection,
            position,
            direction,
            up,
        }) = camera
        {
            let name = match projection {
                Projection::Perspective(_) => "PerspectiveCamera",
                Projection::Orthographic(_) => "OrthogonalCamera",
            };
            out.open(name, &[], &[]);
            point(&mut out, "CameraViewPoint", *position);
            point(&mut out, "CameraDirection", *direction);
            point(&mut out, "CameraUpVector", *up);
            match projection {
                Projection::Perspective(angle) => out.number("FieldOfView", *angle),
                Projection::Orthographic(scale) => out.number("ViewToWorldScale", *scale),
            }
            out.number("AspectRatio", ASPECT_RATIO);
            out.close(name);
        }
        if !clips.is_empty() {
            out.open("ClippingPlanes", &[], &[]);
            for clip in clips {
                out.open("ClippingPlane", &[], &[]);
                point(&mut out, "Location", clip.point);
                point(&mut out, "Direction", clip.normal);
                out.close("ClippingPlane");
            }
            out.close("ClippingPlanes");
        }
    }
    out.close("VisualizationInfo");
    out.out
}

/// 元の bcfzip がないときの extensions.xml。書いた種類・状態・優先度・ラベル・担当を並べる
fn extensions(review: &Review, options: &Options) -> Vec<u8> {
    fn add(list: &mut Vec<String>, value: Option<&str>) {
        if let Some(value) = value.filter(|v| !v.trim().is_empty())
            && !list.iter().any(|v| v == value)
        {
            list.push(value.to_string());
        }
    }
    let (mut kinds, mut statuses, mut priorities, mut labels, mut users) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (text, _) in &options.statuses.0 {
        add(&mut statuses, Some(text));
    }
    for topic in &review.topics {
        add(
            &mut kinds,
            topic.kind.as_deref().or(options.kind.as_deref()),
        );
        if topic.status == zumen_review::Status::Open
            && let Some(label) = &topic.status_label
            && options.statuses.status(label).is_none()
        {
            add(&mut statuses, Some(label));
        }
        add(&mut priorities, topic.priority.as_deref());
        for label in &topic.labels {
            add(&mut labels, Some(label));
        }
        add(&mut users, topic.assignee.as_deref());
    }
    let mut out = Writer::new();
    out.open("Extensions", &[], &[]);
    for (list, outer, inner) in [
        (&kinds, "TopicTypes", "TopicType"),
        (&statuses, "TopicStatuses", "TopicStatus"),
        (&priorities, "Priorities", "Priority"),
        (&labels, "TopicLabels", "TopicLabel"),
        (&users, "Users", "User"),
    ] {
        if !list.is_empty() {
            out.open(outer, &[], &[]);
            for value in list {
                out.text(inner, value);
            }
            out.close(outer);
        }
    }
    out.close("Extensions");
    out.out
}

/// 元の markup に、同じ Guid で写すと同じになるコメントがあれば、その元のバイト列
fn unchanged_comment<'d>(
    kept: &Kept<'d>,
    before: &[zumen_review::Comment],
    comment: &zumen_review::Comment,
) -> Option<&'d [u8]> {
    if !before.iter().any(|c| c == comment) {
        return None;
    }
    let xml = kept.xml.as_ref()?;
    let list = xml.child(kept.topic?, b"Comments")?;
    let element = xml.all(list, b"Comment").into_iter().find(|c| {
        xml.attribute(c, b"Guid").map(|g| guid(&g)).as_deref() == Some(comment.id.as_str())
    })?;
    xml.raw(element)
}
