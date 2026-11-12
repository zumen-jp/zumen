// 曲線と通り芯の外部照合で、同じ元属性の書き方を使う。
use zumen_building::{
    Coordinates, CurveGraph, CurveKind, CurveTransition, CurveTrim, IndexedSegment,
    TrimmingPreference,
};
use zumen_json::Writer;

fn coordinates(out: &mut Writer, value: Coordinates) {
    out.begin_array();
    match value {
        Coordinates::Two(v) => {
            for x in v {
                out.float(x);
            }
        }
        Coordinates::Three(v) => {
            for x in v {
                out.float(x);
            }
        }
    }
    out.end_array();
}

fn flag(out: &mut Writer, value: Option<bool>) {
    if let Some(v) = value {
        out.bool(v);
    } else {
        out.null();
    }
}

fn trimmed(out: &mut Writer, values: &[CurveTrim]) {
    out.begin_array();
    for value in values {
        out.begin_object();
        match value {
            CurveTrim::Point(v) => {
                out.key("point");
                coordinates(out, *v);
            }
            CurveTrim::Parameter(v) => {
                out.key("parameter");
                out.float(*v);
            }
        }
        out.end_object();
    }
    out.end_array();
}

pub(super) fn write_graph(out: &mut Writer, graph: &CurveGraph) {
    for item in &graph.items {
        out.begin_object();
        out.key("instance");
        out.uint(item.source.instance.unwrap());
        out.key("kind");
        match &item.kind {
            CurveKind::Polyline(points) => {
                out.string("polyline");
                out.key("points");
                out.begin_array();
                for p in points {
                    coordinates(out, *p);
                }
                out.end_array();
            }
            CurveKind::Line {
                point,
                direction,
                magnitude,
            } => {
                out.string("line");
                out.key("point");
                coordinates(out, *point);
                out.key("direction");
                coordinates(out, *direction);
                out.key("magnitude");
                out.float(*magnitude);
            }
            CurveKind::Circle { position, radius } => {
                out.string("circle");
                out.key("location");
                coordinates(out, position.location);
                out.key("axis");
                if let Some(v) = position.axis {
                    coordinates(out, Coordinates::Three(v));
                } else {
                    out.null();
                }
                out.key("reference_direction");
                if let Some(v) = position.reference_direction {
                    coordinates(out, v);
                } else {
                    out.null();
                }
                out.key("radius");
                out.float(*radius);
            }
            CurveKind::Indexed {
                points,
                segments,
                self_intersect,
            } => {
                out.string("indexed");
                out.key("points");
                out.begin_array();
                for p in points {
                    coordinates(out, *p);
                }
                out.end_array();
                out.key("segments");
                if let Some(segments) = segments {
                    out.begin_array();
                    for segment in segments {
                        out.begin_object();
                        out.key("kind");
                        out.string(if matches!(segment, IndexedSegment::Line(_)) {
                            "line"
                        } else {
                            "arc"
                        });
                        out.key("indices");
                        out.begin_array();
                        let values: &[u32] = match segment {
                            IndexedSegment::Line(v) => v,
                            IndexedSegment::Arc(v) => v,
                        };
                        for n in values {
                            out.uint(u64::from(*n));
                        }
                        out.end_array();
                        out.end_object();
                    }
                    out.end_array();
                } else {
                    out.null();
                }
                out.key("self_intersect");
                flag(out, *self_intersect);
            }
            CurveKind::Composite {
                segments,
                self_intersect,
            } => {
                out.string("composite");
                out.key("segments");
                out.begin_array();
                for (at, segment) in segments.iter().enumerate() {
                    out.begin_object();
                    out.key("instance");
                    out.uint(item.source.records[at]);
                    out.key("curve");
                    out.uint(graph.item(segment.curve).unwrap().source.instance.unwrap());
                    out.key("same_sense");
                    out.bool(segment.same_sense);
                    out.key("transition");
                    out.string(match segment.transition {
                        CurveTransition::Discontinuous => "DISCONTINUOUS",
                        CurveTransition::Continuous => "CONTINUOUS",
                        CurveTransition::SameGradient => "CONTSAMEGRADIENT",
                        CurveTransition::SameGradientSameCurvature => {
                            "CONTSAMEGRADIENTSAMECURVATURE"
                        }
                    });
                    out.end_object();
                }
                out.end_array();
                out.key("self_intersect");
                flag(out, *self_intersect);
            }
            CurveKind::Trimmed {
                basis,
                trim1,
                trim2,
                sense_agreement,
                preference,
            } => {
                out.string("trimmed");
                out.key("basis");
                out.uint(graph.item(*basis).unwrap().source.instance.unwrap());
                out.key("trim1");
                trimmed(out, trim1);
                out.key("trim2");
                trimmed(out, trim2);
                out.key("sense_agreement");
                out.bool(*sense_agreement);
                out.key("preference");
                out.string(match preference {
                    TrimmingPreference::Cartesian => "CARTESIAN",
                    TrimmingPreference::Parameter => "PARAMETER",
                    TrimmingPreference::Unspecified => "UNSPECIFIED",
                });
            }
            CurveKind::Unsupported(kind) => {
                out.string("unsupported");
                out.key("name");
                out.string(kind);
            }
        }
        out.end_object();
    }
}
