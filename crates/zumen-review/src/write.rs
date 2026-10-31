//! 文書を JSON に書く。知っている欄を決まった順に書き、知らない欄を読んだ順に書き戻す。

use zumen_json::{ToJson, Writer};

use crate::{
    Camera, Carried, Comment, Extra, FORMAT, Location, Markup, Origin, Place, Projection,
    Reference, Review, Target, Topic, VERSION,
};

impl Review {
    /// JSON のバイト列にする
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Writer::new();
        self.to_json(&mut out);
        out.into_bytes()
    }
}

impl ToJson for Review {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        out.key("format");
        out.string(FORMAT);
        out.key("version");
        out.uint(VERSION);
        list(out, "targets", &self.targets, |out, t| t.to_json(out));
        list(out, "topics", &self.topics, |out, t| t.to_json(out));
        list(out, "markups", &self.markups, |out, m| m.to_json(out));
        extra(out, &self.extra);
        out.end_object();
    }
}

impl ToJson for Target {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        text(out, "id", &self.id);
        text(out, "format", &self.format);
        text(out, "hash", &self.hash);
        optional(out, "name", self.name.as_deref());
        extra(out, &self.extra);
        out.end_object();
    }
}

impl ToJson for Topic {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        text(out, "id", &self.id);
        text(out, "title", &self.title);
        optional(out, "description", self.description.as_deref());
        text(out, "status", self.status.name());
        optional(out, "status_label", self.status_label.as_deref());
        optional(out, "kind", self.kind.as_deref());
        optional(out, "priority", self.priority.as_deref());
        list(out, "labels", &self.labels, |out, l| out.string(l));
        optional(out, "assignee", self.assignee.as_deref());
        optional(out, "due", self.due.as_deref());
        optional(out, "author", self.author.as_deref());
        optional(out, "created", self.created.as_deref());
        optional(out, "modified_by", self.modified_by.as_deref());
        optional(out, "modified", self.modified.as_deref());
        list(out, "related", &self.related, |out, r| out.string(r));
        list(out, "comments", &self.comments, |out, c| c.to_json(out));
        list(out, "locations", &self.locations, |out, l| l.to_json(out));
        if let Some(origin) = &self.origin {
            out.key("origin");
            origin.to_json(out);
        }
        extra(out, &self.extra);
        out.end_object();
    }
}

impl ToJson for Comment {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        text(out, "id", &self.id);
        optional(out, "author", self.author.as_deref());
        optional(out, "date", self.date.as_deref());
        text(out, "text", &self.text);
        optional(out, "location", self.location.as_deref());
        extra(out, &self.extra);
        out.end_object();
    }
}

impl ToJson for Origin {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        text(out, "rule", &self.rule);
        out.key("diagnostic");
        out.value(&self.diagnostic);
        out.end_object();
    }
}

impl ToJson for Reference {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        text(out, "id", &self.id);
        optional(out, "hash", self.hash.as_deref());
        out.end_object();
    }
}

impl ToJson for Carried {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        text(out, "from", &self.from);
        text(out, "state", self.state.name());
        out.end_object();
    }
}

impl ToJson for Location {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        text(out, "id", &self.id);
        text(out, "target", &self.target);
        match &self.place {
            Place::Plane {
                view,
                pin,
                extent,
                elements,
            } => {
                text(out, "kind", "2d");
                optional(out, "view", view.as_deref());
                if let Some(pin) = pin {
                    point(out, "pin", pin);
                }
                if let Some(extent) = extent {
                    point(out, "extent", extent);
                }
                list(out, "elements", elements, |out, r| r.to_json(out));
            }
            Place::Space {
                camera,
                selected,
                visible,
                exceptions,
                colored,
                clips,
            } => {
                text(out, "kind", "3d");
                if let Some(camera) = camera {
                    out.key("camera");
                    camera.to_json(out);
                }
                list(out, "selected", selected, |out, r| r.to_json(out));
                if let Some(visible) = visible {
                    out.key("visible");
                    out.bool(*visible);
                }
                list(out, "exceptions", exceptions, |out, r| r.to_json(out));
                list(out, "colored", colored, |out, c| {
                    out.begin_object();
                    text(out, "color", &c.color);
                    list(out, "elements", &c.elements, |out, r| r.to_json(out));
                    out.end_object();
                });
                list(out, "clips", clips, |out, c| {
                    out.begin_object();
                    point(out, "point", &c.point);
                    point(out, "normal", &c.normal);
                    out.end_object();
                });
            }
        }
        if let Some(carried) = &self.carried {
            out.key("carried");
            carried.to_json(out);
        }
        extra(out, &self.extra);
        out.end_object();
    }
}

impl ToJson for Camera {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        match self.projection {
            Projection::Perspective(field_of_view) => {
                text(out, "projection", "perspective");
                out.key("field_of_view");
                out.float(field_of_view);
            }
            Projection::Orthographic(width) => {
                text(out, "projection", "orthographic");
                out.key("width");
                out.float(width);
            }
        }
        point(out, "position", &self.position);
        point(out, "direction", &self.direction);
        point(out, "up", &self.up);
        out.end_object();
    }
}

impl ToJson for Markup {
    fn to_json(&self, out: &mut Writer) {
        out.begin_object();
        text(out, "id", &self.id);
        text(out, "kind", self.kind.name());
        text(out, "target", &self.target);
        optional(out, "view", self.view.as_deref());
        list(out, "points", &self.points, |out, p| {
            out.array(p, |out, n| out.float(*n));
        });
        optional(out, "text", self.text.as_deref());
        optional(out, "topic", self.topic.as_deref());
        if let Some(carried) = &self.carried {
            out.key("carried");
            carried.to_json(out);
        }
        extra(out, &self.extra);
        out.end_object();
    }
}

fn text(out: &mut Writer, key: &str, value: &str) {
    out.key(key);
    out.string(value);
}

fn optional(out: &mut Writer, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        text(out, key, value);
    }
}

/// 空の列は書かない。読むときに空の列と同じに扱う
fn list<T>(out: &mut Writer, key: &str, items: &[T], each: impl FnMut(&mut Writer, &T)) {
    if !items.is_empty() {
        out.key(key);
        out.array(items, each);
    }
}

fn point(out: &mut Writer, key: &str, values: &[f64]) {
    out.key(key);
    out.array(values, |out, n| out.float(*n));
}

fn extra(out: &mut Writer, extra: &Extra) {
    for (key, value) in extra {
        out.key(key);
        out.value(value);
    }
}
