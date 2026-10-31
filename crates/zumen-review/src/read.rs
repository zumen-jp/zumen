//! 文書を読み、ID の重なりと参照の先を確かめる。

use std::collections::BTreeSet;

use zumen_base::{Diagnostic, Kind, codes};
use zumen_json::{self as json, Invalid, ObjectReader, Problem, Value};
use zumen_sdk::{Cx, Error};

use crate::{
    Camera, Carried, Change, Clip, Colored, Comment, FORMAT, Location, Markup, MarkupKind, Origin,
    Place, Projection, Reference, Review, Status, Target, Topic, VERSION,
};

impl Review {
    /// 文書を読む。形が合わなければ `review.file.invalid`、ほかの形式か大きい版なら
    /// `review.format.unsupported`・`review.version.unsupported` を出して失敗する。ID が重なるか、参照の先が
    /// なければ、`review.id.exists`・`review.reference.missing` を出して失敗する
    pub fn read(src: &[u8], cx: &mut Cx) -> Result<Self, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Error::Stopped(stop)
            })?;
        let value = json::parse(src).map_err(|error| {
            cx.report(
                Diagnostic::new(codes::REVIEW_FILE_INVALID)
                    .with("pointer", format!("@{}", error.offset)),
            );
            Error::Invalid
        })?;
        if let Some(format) = value.pointer("/format")
            && format != &Value::String(FORMAT.to_owned())
        {
            let format = match format {
                Value::String(text) => text.clone(),
                _ => String::new(),
            };
            cx.report(Diagnostic::new(codes::REVIEW_FORMAT_UNSUPPORTED).with("format", format));
            return Err(Error::Unsupported);
        }
        if let Some(version) = value
            .pointer("/version")
            .and_then(|v| json::uint(v, 0, json::MAX_SAFE_INTEGER as u64).ok())
            && version != VERSION
        {
            cx.report(Diagnostic::new(codes::REVIEW_VERSION_UNSUPPORTED).with("version", version));
            return Err(Error::Unsupported);
        }
        let review = review(&value).map_err(|error| {
            cx.report(Diagnostic::new(codes::REVIEW_FILE_INVALID).with("pointer", error.pointer()));
            Error::Invalid
        })?;
        let count = review.targets.len()
            + review.markups.len()
            + review
                .topics
                .iter()
                .map(|t| 1 + t.comments.len() + t.locations.len())
                .sum::<usize>();
        cx.check(Kind::Records, count as u64).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        if let Some(diagnostic) = review.problem() {
            cx.report(diagnostic);
            return Err(Error::Invalid);
        }
        Ok(review)
    }

    /// ID の重なりか、参照の先がないこと
    pub(crate) fn problem(&self) -> Option<Diagnostic> {
        let exists = |id: &str| Diagnostic::new(codes::REVIEW_ID_EXISTS).with("id", id);
        let missing = |id: &str| Diagnostic::new(codes::REVIEW_REFERENCE_MISSING).with("id", id);
        let mut targets = BTreeSet::new();
        for target in &self.targets {
            if !targets.insert(target.id.as_str()) {
                return Some(exists(&target.id));
            }
        }
        let mut topics = BTreeSet::new();
        for topic in &self.topics {
            if !topics.insert(topic.id.as_str()) {
                return Some(exists(&topic.id));
            }
        }
        // 場所・コメント・図形の ID は、それぞれ文書の中で一意
        let mut locations = BTreeSet::new();
        let mut comments = BTreeSet::new();
        for topic in &self.topics {
            for location in &topic.locations {
                if !locations.insert(location.id.as_str()) {
                    return Some(exists(&location.id));
                }
                if !targets.contains(location.target.as_str()) {
                    return Some(missing(&location.target));
                }
            }
            for comment in &topic.comments {
                if !comments.insert(comment.id.as_str()) {
                    return Some(exists(&comment.id));
                }
                if let Some(location) = &comment.location
                    && !topic.locations.iter().any(|l| &l.id == location)
                {
                    return Some(missing(location));
                }
            }
            if let Some(related) = topic.related.iter().find(|r| !topics.contains(r.as_str())) {
                return Some(missing(related));
            }
        }
        let mut markups = BTreeSet::new();
        for markup in &self.markups {
            if !markups.insert(markup.id.as_str()) {
                return Some(exists(&markup.id));
            }
            if !targets.contains(markup.target.as_str()) {
                return Some(missing(&markup.target));
            }
            if let Some(topic) = &markup.topic
                && !topics.contains(topic.as_str())
            {
                return Some(missing(topic));
            }
        }
        None
    }
}

fn review(value: &Value) -> Result<Review, Invalid> {
    let mut object = ObjectReader::new(value)?;
    object.constant("format", FORMAT)?;
    object.required("version", |v| json::uint(v, VERSION, VERSION))?;
    let targets = list(&mut object, "targets", target)?;
    let topics = list(&mut object, "topics", topic)?;
    let markups = list(&mut object, "markups", markup)?;
    Ok(Review {
        targets,
        topics,
        markups,
        extra: object.rest(),
    })
}

fn list<T>(
    object: &mut ObjectReader<'_>,
    key: &str,
    item: impl FnMut(&Value) -> Result<T, Invalid>,
) -> Result<Vec<T>, Invalid> {
    Ok(object
        .optional(key, |v| json::array(v, item))?
        .unwrap_or_default())
}

fn text(object: &mut ObjectReader<'_>, key: &str) -> Result<Option<String>, Invalid> {
    object.optional(key, json::string)
}

fn texts(object: &mut ObjectReader<'_>, key: &str) -> Result<Vec<String>, Invalid> {
    list(object, key, json::string)
}

/// 空でない文字列の ID
fn id(value: &Value) -> Result<String, Invalid> {
    let text = json::string(value)?;
    if text.is_empty() {
        return Err(Invalid::new(Problem::Value));
    }
    Ok(text)
}

fn target(value: &Value) -> Result<Target, Invalid> {
    let mut object = ObjectReader::new(value)?;
    Ok(Target {
        id: object.required("id", id)?,
        format: object.required("format", json::string)?,
        hash: object.required("hash", json::string)?,
        name: text(&mut object, "name")?,
        extra: object.rest(),
    })
}

fn topic(value: &Value) -> Result<Topic, Invalid> {
    let mut object = ObjectReader::new(value)?;
    Ok(Topic {
        id: object.required("id", id)?,
        title: object.required("title", json::string)?,
        description: text(&mut object, "description")?,
        status: object.required("status", |v| match json::str(v)? {
            "open" => Ok(Status::Open),
            "resolved" => Ok(Status::Resolved),
            "closed" => Ok(Status::Closed),
            _ => Err(Invalid::new(Problem::Value)),
        })?,
        status_label: text(&mut object, "status_label")?,
        kind: text(&mut object, "kind")?,
        priority: text(&mut object, "priority")?,
        labels: texts(&mut object, "labels")?,
        assignee: text(&mut object, "assignee")?,
        due: text(&mut object, "due")?,
        author: text(&mut object, "author")?,
        created: text(&mut object, "created")?,
        modified_by: text(&mut object, "modified_by")?,
        modified: text(&mut object, "modified")?,
        related: texts(&mut object, "related")?,
        comments: list(&mut object, "comments", comment)?,
        locations: list(&mut object, "locations", location)?,
        origin: object.optional("origin", origin)?,
        extra: object.rest(),
    })
}

fn comment(value: &Value) -> Result<Comment, Invalid> {
    let mut object = ObjectReader::new(value)?;
    Ok(Comment {
        id: object.required("id", id)?,
        author: text(&mut object, "author")?,
        date: text(&mut object, "date")?,
        text: object.required("text", json::string)?,
        location: text(&mut object, "location")?,
        extra: object.rest(),
    })
}

fn origin(value: &Value) -> Result<Origin, Invalid> {
    let mut object = ObjectReader::new(value)?;
    let origin = Origin {
        rule: object.required("rule", json::string)?,
        diagnostic: object.required("diagnostic", |v| match v {
            Value::Object(_) => Ok(v.clone()),
            _ => Err(Invalid::new(Problem::Type)),
        })?,
    };
    object.finish()?;
    Ok(origin)
}

fn reference(value: &Value) -> Result<Reference, Invalid> {
    let mut object = ObjectReader::new(value)?;
    let reference = Reference {
        id: object.required("id", id)?,
        hash: text(&mut object, "hash")?,
    };
    object.finish()?;
    Ok(reference)
}

fn references(object: &mut ObjectReader<'_>, key: &str) -> Result<Vec<Reference>, Invalid> {
    list(object, key, reference)
}

fn point<const N: usize>(value: &Value) -> Result<[f64; N], Invalid> {
    json::array(value, json::number)?
        .try_into()
        .map_err(|_| Invalid::new(Problem::Value))
}

fn carried(value: &Value) -> Result<Carried, Invalid> {
    let mut object = ObjectReader::new(value)?;
    let carried = Carried {
        from: object.required("from", json::string)?,
        state: object.required("state", |v| match json::str(v)? {
            "unchanged" => Ok(Change::Unchanged),
            "changed" => Ok(Change::Changed),
            "removed" => Ok(Change::Removed),
            _ => Err(Invalid::new(Problem::Value)),
        })?,
    };
    object.finish()?;
    Ok(carried)
}

fn location(value: &Value) -> Result<Location, Invalid> {
    let mut object = ObjectReader::new(value)?;
    let id = object.required("id", self::id)?;
    let target = object.required("target", self::id)?;
    let place = match object.required("kind", json::string)?.as_str() {
        "2d" => Place::Plane {
            view: text(&mut object, "view")?,
            pin: object.optional("pin", point::<2>)?,
            extent: object.optional("extent", point::<4>)?,
            elements: references(&mut object, "elements")?,
        },
        "3d" => Place::Space {
            camera: object.optional("camera", camera)?,
            selected: references(&mut object, "selected")?,
            visible: object.optional("visible", json::boolean)?,
            exceptions: references(&mut object, "exceptions")?,
            colored: list(&mut object, "colored", colored)?,
            clips: list(&mut object, "clips", clip)?,
        },
        other => return Err(Invalid::new(Problem::Tag(other.to_owned())).in_key("kind")),
    };
    Ok(Location {
        id,
        target,
        place,
        carried: object.optional("carried", carried)?,
        extra: object.rest(),
    })
}

fn camera(value: &Value) -> Result<Camera, Invalid> {
    let mut object = ObjectReader::new(value)?;
    let projection = match object.required("projection", json::string)?.as_str() {
        "perspective" => Projection::Perspective(object.required("field_of_view", json::number)?),
        "orthographic" => Projection::Orthographic(object.required("width", json::number)?),
        other => return Err(Invalid::new(Problem::Tag(other.to_owned())).in_key("projection")),
    };
    let camera = Camera {
        projection,
        position: object.required("position", point::<3>)?,
        direction: object.required("direction", point::<3>)?,
        up: object.required("up", point::<3>)?,
    };
    object.finish()?;
    Ok(camera)
}

fn colored(value: &Value) -> Result<Colored, Invalid> {
    let mut object = ObjectReader::new(value)?;
    let colored = Colored {
        color: object.required("color", json::string)?,
        elements: references(&mut object, "elements")?,
    };
    object.finish()?;
    Ok(colored)
}

fn clip(value: &Value) -> Result<Clip, Invalid> {
    let mut object = ObjectReader::new(value)?;
    let clip = Clip {
        point: object.required("point", point::<3>)?,
        normal: object.required("normal", point::<3>)?,
    };
    object.finish()?;
    Ok(clip)
}

fn markup(value: &Value) -> Result<Markup, Invalid> {
    let mut object = ObjectReader::new(value)?;
    Ok(Markup {
        id: object.required("id", id)?,
        kind: object.required("kind", |v| match json::str(v)? {
            "redline" => Ok(MarkupKind::Redline),
            "cloud" => Ok(MarkupKind::Cloud),
            "text" => Ok(MarkupKind::Text),
            "photo" => Ok(MarkupKind::Photo),
            _ => Err(Invalid::new(Problem::Value)),
        })?,
        target: object.required("target", id)?,
        view: text(&mut object, "view")?,
        points: list(&mut object, "points", point::<2>)?,
        text: text(&mut object, "text")?,
        topic: text(&mut object, "topic")?,
        carried: object.optional("carried", carried)?,
        extra: object.rest(),
    })
}
