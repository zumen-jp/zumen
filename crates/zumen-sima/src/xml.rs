//! JPGIS 版 SIMA XML Schema（`jsima102.xsd`・`jpsSpatial.xsd`）の L1 レコード抽出と中間モデルへの読み取り。

use core::f64::consts::PI;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{ByteRange, Diagnostic, Kind, Ref, codes};
use zumen_crs::{Crs, Realization, Zone};
use zumen_sdk::{Cx, Error};
use zumen_xml::{Document, Element, Node};

use crate::model::{
    AttributeVal, MapLocation, Parcel, ParcelKind, PointKind, RouteSegment, RouteSegmentKind,
    SimaData, SurveyPoint, arc_to_drawing, bearing_and_distance, clothoid_curvatures, norm_zero,
    parse_map_point, sample_arc_locations, three_point_arc,
};
use crate::{JPGIS_NAMESPACE, NAMESPACE_201004, NAMESPACE_201206, Record, stopped};

const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";

type SurfaceRings = (Vec<MapLocation>, Vec<Vec<MapLocation>>, bool);

fn elem_bytes(elem: &Element) -> ByteRange {
    ByteRange {
        start: elem.span().start as u64,
        end: elem.span().end as u64,
    }
}

fn elem_ref(elem: &Element) -> Ref {
    let range = elem_bytes(elem);
    Ref::at_bytes(range.start, range.end)
}

fn is_jsima_ns(ns: Option<&str>) -> bool {
    matches!(ns, Some(NAMESPACE_201206) | Some(NAMESPACE_201004))
}

pub(crate) fn validate_and_collect_records(
    doc: &Document,
    cx: &mut Cx,
) -> Result<Vec<Record>, Error> {
    let Some(root) = doc.element(doc.root()) else {
        cx.report(
            Diagnostic::new(codes::SIMA_FILE_INVALID)
                .with("offset", 0u64)
                .at(Ref::at_bytes(0, 0)),
        );
        return Err(Error::Invalid);
    };
    let root_ns = doc.namespace(root.name());
    if doc.parsed_local_name(root.name()) != b"GI" || !is_jsima_ns(root_ns.as_deref()) {
        cx.report(
            Diagnostic::new(codes::SIMA_FILE_INVALID)
                .with("offset", root.span().start as u64)
                .at(elem_ref(root)),
        );
        return Err(Error::Invalid);
    }

    let mut records = Vec::new();
    for child in child_elements(doc, root) {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(elem_ref(child))))?;
        let local = doc.parsed_local_name(child.name());
        if local == b"dataset" {
            let ds_children = child_elements(doc, child);
            if ds_children.is_empty() {
                push_xml_record(doc, child, &mut records, cx)?;
            } else {
                for ds_child in ds_children {
                    if doc.parsed_local_name(ds_child.name()) == b"object" {
                        let objs = child_elements(doc, ds_child);
                        if objs.is_empty() {
                            push_xml_record(doc, ds_child, &mut records, cx)?;
                        } else {
                            for obj in objs {
                                push_xml_record(doc, obj, &mut records, cx)?;
                            }
                        }
                    } else {
                        push_xml_record(doc, ds_child, &mut records, cx)?;
                    }
                }
            }
        } else {
            push_xml_record(doc, child, &mut records, cx)?;
        }
    }
    if records.is_empty() {
        push_xml_record(doc, root, &mut records, cx)?;
    }
    Ok(records)
}

fn push_xml_record(
    doc: &Document,
    elem: &Element,
    records: &mut Vec<Record>,
    cx: &mut Cx,
) -> Result<(), Error> {
    let kind = doc.parsed_local_name(elem.name()).to_vec();
    cx.check(Kind::DecodedBytes, (kind.len() + 24) as u64)
        .map_err(|s| stopped(cx, s, Some(elem_ref(elem))))?;
    records.push(Record {
        kind,
        bytes: elem_bytes(elem),
    });
    Ok(())
}

fn child_elements<'d>(doc: &'d Document, elem: &'d Element) -> Vec<&'d Element> {
    elem.children()
        .iter()
        .filter_map(|node| match node {
            Node::Element(idx) => doc.element(*idx),
            Node::Text(_) | Node::CData(_) => None,
        })
        .collect()
}

pub(crate) fn parse_xml_data(doc: &Document, cx: &mut Cx) -> Result<SimaData, Error> {
    let mut reader = XmlReader {
        doc,
        cx,
        ids: BTreeMap::new(),
        sokuten_by_id: BTreeMap::new(),
        kakuchi_by_id: BTreeMap::new(),
        data: SimaData::default(),
    };
    reader.run()?;
    Ok(reader.data)
}

#[derive(Clone, Debug)]
struct ResolvedCurve {
    points: Vec<MapLocation>,
    segments: Vec<RouteSegment>,
    has_curve: bool,
}

struct XmlReader<'d, 'c, 'b> {
    doc: &'d Document,
    cx: &'c mut Cx<'b>,
    ids: BTreeMap<String, usize>,
    sokuten_by_id: BTreeMap<String, usize>,
    kakuchi_by_id: BTreeMap<String, usize>,
    data: SimaData,
}

impl<'d> XmlReader<'d, '_, '_> {
    fn run(&mut self) -> Result<(), Error> {
        let Some(root) = self.doc.element(self.doc.root()) else {
            return Err(Error::Invalid);
        };
        if let Some(attr) = self.doc.attribute(root, None, b"version") {
            let v = self.doc.value(attr);
            let t = v.trim();
            if !t.is_empty() {
                self.data.version = Some(t.to_string());
            }
        }

        for (idx, elem) in self.doc.elements().iter().enumerate() {
            self.cx
                .check(Kind::Work, 1)
                .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
            if let Some(id) = self.element_id(elem) {
                self.cx
                    .check(Kind::DecodedBytes, (id.len() + 16) as u64)
                    .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
                self.ids.entry(id).or_insert(idx);
            }
        }

        // dataset 配下のオブジェクトを順番に集める
        let mut objects: Vec<&'d Element> = Vec::new();
        for child in child_elements(self.doc, root) {
            let local = self.doc.parsed_local_name(child.name());
            match local {
                b"exchangeMetadata" => {}
                b"dataset" => {
                    for ds_child in child_elements(self.doc, child) {
                        let ds_local = self.doc.parsed_local_name(ds_child.name());
                        if ds_local == b"GenbaJoho" {
                            self.parse_genba_joho(ds_child)?;
                        } else if ds_local == b"object" {
                            for obj in child_elements(self.doc, ds_child) {
                                objects.push(obj);
                            }
                        } else {
                            self.report_unsupported(ds_child);
                        }
                    }
                }
                _ => self.report_unsupported(child),
            }
        }

        // 第 1 パス: GenbaJoho・Sokuten・Kakuchi・KaihouKakuchi を先に解決する（Kijyunten・Suijyunten・Chiban が参照するため）
        let mut deferred: Vec<&'d Element> = Vec::new();
        for obj in objects {
            self.cx
                .check(Kind::Work, 1)
                .map_err(|s| stopped(self.cx, s, Some(elem_ref(obj))))?;
            let ns = self.doc.namespace(obj.name());
            let local = self.doc.parsed_local_name(obj.name());
            if ns.as_deref() == Some(JPGIS_NAMESPACE) {
                // GM_Point・GM_Curve・GM_Surface などの空間プリミティブは地物から参照して解決する
                continue;
            }
            if !is_jsima_ns(ns.as_deref()) {
                self.report_unsupported(obj);
                continue;
            }
            match local {
                b"GenbaJoho" => self.parse_genba_joho(obj)?,
                b"Sokuten" => {
                    let _ = self.parse_sokuten(obj, PointKind::Sokuten)?;
                }
                b"Kakuchi" => {
                    let _ = self.parse_kakuchi(obj)?;
                }
                b"KaihouKakuchi" => self.parse_kaihou_kakuchi(obj)?,
                b"Kijyunten" | b"Suijyunten" | b"Chiban" => deferred.push(obj),
                _ => self.report_unsupported(obj),
            }
        }

        // 第 2 パス: Kijyunten・Suijyunten・Chiban を解決する
        for obj in deferred {
            let local = self.doc.parsed_local_name(obj.name());
            match local {
                b"Kijyunten" => self.parse_control_point(obj, PointKind::Kijyunten)?,
                b"Suijyunten" => self.parse_control_point(obj, PointKind::Suijyunten)?,
                b"Chiban" => self.parse_chiban(obj)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn element_id(&self, elem: &Element) -> Option<String> {
        let attr = self
            .doc
            .attribute(elem, None, b"id")
            .or_else(|| self.doc.attribute(elem, None, b"uuid"))?;
        let v = self.doc.value(attr);
        let t = v.trim();
        (!t.is_empty()).then(|| t.to_string())
    }

    fn ref_target(&self, elem: &Element) -> Option<String> {
        if let Some(attr) = self.doc.attribute(elem, None, b"idref") {
            let v = self.doc.value(attr);
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        if let Some(attr) = self.doc.attribute(elem, None, b"uuidref") {
            let v = self.doc.value(attr);
            let t = v.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        if let Some(attr) = self
            .doc
            .attribute(elem, Some(XLINK_NAMESPACE), b"href")
            .or_else(|| self.doc.attribute(elem, None, b"href"))
        {
            let v = self.doc.value(attr);
            let t = v.trim();
            let stripped = t.strip_prefix('#').unwrap_or(t);
            if !stripped.is_empty() {
                return Some(stripped.to_string());
            }
        }
        None
    }

    fn text_of(&mut self, elem: &Element) -> Result<String, Error> {
        let mut out = String::new();
        for node in elem.children() {
            match node {
                Node::Text(r) => {
                    let s = self.doc.text(r.clone());
                    self.cx
                        .check(Kind::Work, (s.len() as u64 / 64).max(1))
                        .map_err(|stop| stopped(self.cx, stop, Some(elem_ref(elem))))?;
                    self.cx
                        .check(Kind::DecodedBytes, s.len() as u64)
                        .map_err(|stop| stopped(self.cx, stop, Some(elem_ref(elem))))?;
                    out.push_str(&s);
                }
                Node::CData(r) => {
                    let s = self.doc.cdata(r.clone());
                    self.cx
                        .check(Kind::Work, (s.len() as u64 / 64).max(1))
                        .map_err(|stop| stopped(self.cx, stop, Some(elem_ref(elem))))?;
                    self.cx
                        .check(Kind::DecodedBytes, s.len() as u64)
                        .map_err(|stop| stopped(self.cx, stop, Some(elem_ref(elem))))?;
                    out.push_str(&s);
                }
                Node::Element(_) => {}
            }
        }
        Ok(out)
    }

    fn find_child<'e>(&self, elem: &'e Element, local: &[u8]) -> Option<&'e Element>
    where
        'd: 'e,
    {
        child_elements(self.doc, elem)
            .into_iter()
            .find(|c| self.doc.parsed_local_name(c.name()) == local)
    }

    fn report_unsupported(&mut self, elem: &Element) {
        let name = self.doc.local_name_text(elem.name()).into_owned();
        self.cx.report(
            Diagnostic::new(codes::SIMA_RECORD_UNSUPPORTED)
                .with("kind", name)
                .at(elem_ref(elem)),
        );
    }

    fn parse_genba_joho(&mut self, elem: &Element) -> Result<(), Error> {
        let mut zone_str: Option<(String, Ref)> = None;
        let mut crs_code: Option<(String, Ref)> = None;

        for child in child_elements(self.doc, elem) {
            self.cx
                .check(Kind::Work, 1)
                .map_err(|s| stopped(self.cx, s, Some(elem_ref(child))))?;
            let local = self.doc.parsed_local_name(child.name());
            let val = self.text_of(child)?;
            let trimmed = val.trim();
            if trimmed.is_empty() {
                continue;
            }
            match local {
                b"Name" => self.data.site_name = Some(trimmed.to_string()),
                b"CoordinateSystem" => zone_str = Some((trimmed.to_string(), elem_ref(child))),
                b"Crs" => crs_code = Some((trimmed.to_string(), elem_ref(child))),
                b"Worker" | b"Client" | b"Engineer" | b"Comment" => {
                    let key = String::from_utf8_lossy(local).into_owned();
                    self.data
                        .site_attributes
                        .push((key, AttributeVal::Text(trimmed.to_string())));
                }
                b"Start" | b"End" => {
                    let key = String::from_utf8_lossy(local).into_owned();
                    self.data
                        .site_attributes
                        .push((key, AttributeVal::Date(trimmed.to_string())));
                }
                _ => {}
            }
        }

        if let Some((z_text, z_ref)) = zone_str {
            let Some(zone) = z_text.parse::<u8>().ok().and_then(Zone::new) else {
                self.data.crs_reported = true;
                self.cx.report(
                    Diagnostic::new(codes::SIMA_CRS_UNSUPPORTED)
                        .with("system", z_text)
                        .at(z_ref),
                );
                return Ok(());
            };
            match crs_code {
                None => {
                    self.data.crs = Some(Crs::Plane(zone));
                    self.data.realization = Some(Realization::Survey2011);
                }
                Some((code_text, c_ref)) => match code_text.as_str() {
                    "1" => {
                        self.data.crs = Some(Crs::Jgd2000(zone));
                    }
                    "2" => {
                        self.data.crs = Some(Crs::Tokyo(zone));
                    }
                    "3" => {
                        self.data.crs = Some(Crs::Plane(zone));
                        self.data.realization = Some(Realization::Survey2011);
                    }
                    "4" => {
                        self.data.crs = Some(Crs::Plane(zone));
                        self.data.realization = Some(Realization::Survey2024);
                    }
                    _ => {
                        self.data.crs_reported = true;
                        self.cx.report(
                            Diagnostic::new(codes::SIMA_CRS_UNSUPPORTED)
                                .with(
                                    "system",
                                    format!("CoordinateSystem={z_text},Crs={code_text}"),
                                )
                                .at(c_ref),
                        );
                    }
                },
            }
        }
        Ok(())
    }

    fn parse_sokuten(
        &mut self,
        elem: &'d Element,
        kind: PointKind,
    ) -> Result<Option<usize>, Error> {
        let obj_id = self.element_id(elem);
        let mut number = String::new();
        let mut name = String::new();
        let mut altitude: Option<f64> = None;
        let mut ref_point_elem: Option<&'d Element> = None;

        for child in child_elements(self.doc, elem) {
            match self.doc.parsed_local_name(child.name()) {
                b"Number" => number = self.text_of(child)?.trim().to_string(),
                b"Name" => name = self.text_of(child)?.trim().to_string(),
                b"Altitude" => {
                    let raw = self.text_of(child)?;
                    let t = raw.trim();
                    if !t.is_empty() {
                        match t.parse::<f64>().ok().filter(|v| v.is_finite()) {
                            Some(z) => altitude = Some(norm_zero(z)),
                            None => self.cx.report(
                                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                    .with("kind", "Sokuten")
                                    .with("field", "altitude")
                                    .at(elem_ref(child)),
                            ),
                        }
                    }
                }
                b"RefPoint" => ref_point_elem = Some(child),
                _ => {}
            }
        }

        let Some(ref_pt) = ref_point_elem else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "Sokuten")
                    .with("field", "RefPoint")
                    .at(elem_ref(elem)),
            );
            return Ok(None);
        };
        let mut visited = BTreeSet::new();
        let Some(loc) = self.resolve_gm_point_ref(ref_pt, &mut visited)? else {
            return Ok(None);
        };
        let final_alt = altitude.or(loc.altitude);

        self.cx
            .check(Kind::Elements, 1)
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        self.cx
            .check(Kind::DecodedBytes, (number.len() + name.len() + 64) as u64)
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;

        let idx = self.data.points.len();
        if let Some(ref id) = obj_id {
            self.sokuten_by_id.insert(id.clone(), idx);
        }
        self.data.points.push(SurveyPoint {
            id_hint: obj_id,
            number,
            name,
            position: loc.point,
            altitude: final_alt,
            kind,
            attributes: Vec::new(),
            source: elem_ref(elem),
        });
        Ok(Some(idx))
    }

    fn parse_control_point(&mut self, elem: &'d Element, kind: PointKind) -> Result<(), Error> {
        let obj_id = self.element_id(elem);
        let kind_name = match kind {
            PointKind::Kijyunten => "Kijyunten",
            PointKind::Suijyunten => "Suijyunten",
            PointKind::Sokuten => "Sokuten",
        };
        let mut attrs = Vec::new();
        let mut ref_sokuten: Option<&'d Element> = None;
        let mut ave_altitude: Option<f64> = None;

        for child in child_elements(self.doc, elem) {
            let local = self.doc.parsed_local_name(child.name());
            if local == b"RefSokuten" {
                ref_sokuten = Some(child);
                continue;
            }
            let raw = self.text_of(child)?;
            let t = raw.trim();
            if t.is_empty() {
                continue;
            }
            let key = String::from_utf8_lossy(local).into_owned();
            match local {
                b"B" | b"L" | b"geoidH" | b"Scafac" | b"tnda" | b"pilLen" | b"antHei"
                | b"AveAltitude" => match t.parse::<f64>().ok().filter(|v| v.is_finite()) {
                    Some(val) => {
                        let n = norm_zero(val);
                        if local == b"AveAltitude" {
                            ave_altitude = Some(n);
                        }
                        attrs.push((key, AttributeVal::Real(n)));
                    }
                    None => self.cx.report(
                        Diagnostic::new(codes::SIMA_VALUE_INVALID)
                            .with("kind", kind_name)
                            .with("field", "number")
                            .at(elem_ref(child)),
                    ),
                },
                b"obsDay" | b"CalcDay" => attrs.push((key, AttributeVal::Date(t.to_string()))),
                _ => attrs.push((key, AttributeVal::Text(t.to_string()))),
            }
        }

        let Some(ref_elem) = ref_sokuten else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", kind_name)
                    .with("field", "RefSokuten")
                    .at(elem_ref(elem)),
            );
            return Ok(());
        };

        if let Some(inline_sokuten) = self.find_child(ref_elem, b"Sokuten") {
            let has_inline_id = self.element_id(inline_sokuten).is_some();
            if let Some(idx) = self.parse_sokuten(inline_sokuten, PointKind::Sokuten)? {
                if !has_inline_id {
                    if let Some(pt) = self.data.points.get_mut(idx) {
                        pt.id_hint = obj_id;
                        pt.kind = kind;
                        if ave_altitude.is_some() {
                            pt.altitude = ave_altitude;
                        }
                        pt.attributes.extend(attrs);
                        pt.source = elem_ref(elem);
                    }
                } else if let Some(base) = self.data.points.get(idx).cloned() {
                    self.push_derived_point(elem, obj_id, kind, base, ave_altitude, attrs)?;
                }
            }
        } else if let Some(target_id) = self.ref_target(ref_elem) {
            if let Some(&idx) = self.sokuten_by_id.get(&target_id) {
                if let Some(base) = self.data.points.get(idx).cloned() {
                    self.push_derived_point(elem, obj_id, kind, base, ave_altitude, attrs)?;
                }
            } else {
                self.cx.report(
                    Diagnostic::new(codes::SIMA_POINT_MISSING)
                        .with("target", target_id)
                        .at(elem_ref(ref_elem)),
                );
            }
        } else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", kind_name)
                    .with("field", "RefSokuten")
                    .at(elem_ref(ref_elem)),
            );
        }
        Ok(())
    }

    fn push_derived_point(
        &mut self,
        elem: &Element,
        obj_id: Option<String>,
        kind: PointKind,
        base: SurveyPoint,
        ave_altitude: Option<f64>,
        attrs: Vec<(String, AttributeVal)>,
    ) -> Result<(), Error> {
        self.cx
            .check(Kind::Elements, 1)
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        self.cx
            .check(
                Kind::DecodedBytes,
                (base.number.len() + base.name.len() + attrs.len() * 32 + 64) as u64,
            )
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        self.data.points.push(SurveyPoint {
            id_hint: obj_id,
            number: base.number,
            name: base.name,
            position: base.position,
            altitude: ave_altitude.or(base.altitude),
            kind,
            attributes: attrs,
            source: elem_ref(elem),
        });
        Ok(())
    }

    fn parse_kakuchi(&mut self, elem: &'d Element) -> Result<Option<usize>, Error> {
        let obj_id = self.element_id(elem);
        let mut number = String::new();
        let mut name: Option<String> = None;
        let mut ref_surface: Option<&'d Element> = None;

        for child in child_elements(self.doc, elem) {
            match self.doc.parsed_local_name(child.name()) {
                b"Number" => number = self.text_of(child)?.trim().to_string(),
                b"Name" => {
                    let t = self.text_of(child)?.trim().to_string();
                    if !t.is_empty() {
                        name = Some(t);
                    }
                }
                b"RefSurface" => ref_surface = Some(child),
                _ => {}
            }
        }

        let Some(ref_surf) = ref_surface else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "Kakuchi")
                    .with("field", "RefSurface")
                    .at(elem_ref(elem)),
            );
            return Ok(None);
        };
        let mut visited = BTreeSet::new();
        let Some((outer, holes, ring_curve_approximated)) =
            self.resolve_gm_surface_ref(ref_surf, &mut visited)?
        else {
            return Ok(None);
        };
        if outer.len() < 3 {
            self.cx.report(
                Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                    .with("kind", "parcel")
                    .at(elem_ref(elem)),
            );
            return Ok(None);
        }

        self.cx
            .check(Kind::Elements, 1)
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        let hole_pts: usize = holes.iter().map(Vec::len).sum();
        self.cx
            .check(
                Kind::DecodedBytes,
                ((outer.len() + hole_pts) * 32 + 64) as u64,
            )
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;

        let idx = self.data.parcels.len();
        if let Some(ref id) = obj_id {
            self.kakuchi_by_id.insert(id.clone(), idx);
        }
        self.data.parcels.push(Parcel {
            id_hint: obj_id,
            number,
            name,
            closed: true,
            outer,
            holes,
            curve_segments: Vec::new(),
            ring_curve_approximated,
            kind: ParcelKind::Kakuchi,
            attributes: Vec::new(),
            source: elem_ref(elem),
        });
        Ok(Some(idx))
    }

    fn parse_kaihou_kakuchi(&mut self, elem: &'d Element) -> Result<(), Error> {
        let obj_id = self.element_id(elem);
        let mut number = String::new();
        let mut name: Option<String> = None;
        let mut ref_curve: Option<&'d Element> = None;

        for child in child_elements(self.doc, elem) {
            match self.doc.parsed_local_name(child.name()) {
                b"Number" => number = self.text_of(child)?.trim().to_string(),
                b"Name" => {
                    let t = self.text_of(child)?.trim().to_string();
                    if !t.is_empty() {
                        name = Some(t);
                    }
                }
                b"RefCurve" => ref_curve = Some(child),
                _ => {}
            }
        }

        let Some(ref_c) = ref_curve else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "KaihouKakuchi")
                    .with("field", "RefCurve")
                    .at(elem_ref(elem)),
            );
            return Ok(());
        };
        let mut visited = BTreeSet::new();
        let Some(curve) = self.resolve_gm_curve_ref(ref_c, &mut visited)? else {
            return Ok(());
        };
        if curve.points.len() < 2 && curve.segments.is_empty() {
            self.cx.report(
                Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                    .with("kind", "polyline")
                    .at(elem_ref(elem)),
            );
            return Ok(());
        }

        self.cx
            .check(Kind::Elements, 1)
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        self.cx
            .check(
                Kind::DecodedBytes,
                ((curve.points.len() + curve.segments.len()) * 48 + 64) as u64,
            )
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;

        let has_arc = curve
            .segments
            .iter()
            .any(|s| !matches!(s.kind, RouteSegmentKind::Line));
        self.data.parcels.push(Parcel {
            id_hint: obj_id,
            number,
            name,
            closed: false,
            outer: curve.points,
            holes: Vec::new(),
            curve_segments: if has_arc { curve.segments } else { Vec::new() },
            ring_curve_approximated: false,
            kind: ParcelKind::KaihouKakuchi,
            attributes: Vec::new(),
            source: elem_ref(elem),
        });
        Ok(())
    }

    fn parse_chiban(&mut self, elem: &'d Element) -> Result<(), Error> {
        let obj_id = self.element_id(elem);
        let mut attrs = Vec::new();
        let mut chiban_label: Option<String> = None;
        let mut ref_kakuchi: Option<&'d Element> = None;

        for child in child_elements(self.doc, elem) {
            let local = self.doc.parsed_local_name(child.name());
            if local == b"RefKakuchi" {
                ref_kakuchi = Some(child);
                continue;
            }
            let raw = self.text_of(child)?;
            let t = raw.trim();
            if t.is_empty() {
                continue;
            }
            if local == b"Chiban" || local == b"Number" {
                chiban_label = Some(t.to_string());
            }
            let key = String::from_utf8_lossy(local).into_owned();
            match local {
                b"Area" | b"KArea" => match t.parse::<f64>().ok().filter(|v| v.is_finite()) {
                    Some(v) => attrs.push((key, AttributeVal::Real(norm_zero(v)))),
                    None => self.cx.report(
                        Diagnostic::new(codes::SIMA_VALUE_INVALID)
                            .with("kind", "Chiban")
                            .with("field", "area")
                            .at(elem_ref(child)),
                    ),
                },
                b"Rotation" => match t.parse::<i64>() {
                    Ok(v) => attrs.push((key, AttributeVal::Integer(v))),
                    Err(_) => self.cx.report(
                        Diagnostic::new(codes::SIMA_VALUE_INVALID)
                            .with("kind", "Chiban")
                            .with("field", "Rotation")
                            .at(elem_ref(child)),
                    ),
                },
                _ => attrs.push((key, AttributeVal::Text(t.to_string()))),
            }
        }

        let Some(ref_elem) = ref_kakuchi else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "Chiban")
                    .with("field", "RefKakuchi")
                    .at(elem_ref(elem)),
            );
            return Ok(());
        };

        if let Some(inline_kakuchi) = self.find_child(ref_elem, b"Kakuchi") {
            let has_inline_id = self.element_id(inline_kakuchi).is_some();
            if let Some(idx) = self.parse_kakuchi(inline_kakuchi)? {
                if !has_inline_id {
                    if let Some(parcel) = self.data.parcels.get_mut(idx) {
                        parcel.id_hint = obj_id;
                        if let Some(lbl) = chiban_label {
                            parcel.number = lbl;
                        }
                        parcel.kind = ParcelKind::Chiban;
                        parcel.attributes.extend(attrs);
                        parcel.source = elem_ref(elem);
                    }
                } else if let Some(base) = self.data.parcels.get(idx).cloned() {
                    self.push_derived_chiban(elem, obj_id, chiban_label, base, attrs)?;
                }
            }
        } else if let Some(target_id) = self.ref_target(ref_elem) {
            if let Some(&idx) = self.kakuchi_by_id.get(&target_id) {
                if let Some(base) = self.data.parcels.get(idx).cloned() {
                    self.push_derived_chiban(elem, obj_id, chiban_label, base, attrs)?;
                }
            } else {
                self.cx.report(
                    Diagnostic::new(codes::SIMA_POINT_MISSING)
                        .with("target", target_id)
                        .at(elem_ref(ref_elem)),
                );
            }
        } else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "Chiban")
                    .with("field", "RefKakuchi")
                    .at(elem_ref(ref_elem)),
            );
        }
        Ok(())
    }

    fn push_derived_chiban(
        &mut self,
        elem: &Element,
        obj_id: Option<String>,
        chiban_label: Option<String>,
        base: Parcel,
        attrs: Vec<(String, AttributeVal)>,
    ) -> Result<(), Error> {
        self.cx
            .check(Kind::Elements, 1)
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        let hole_pts: usize = base.holes.iter().map(Vec::len).sum();
        self.cx
            .check(
                Kind::DecodedBytes,
                ((base.outer.len() + hole_pts) * 32 + attrs.len() * 32 + 64) as u64,
            )
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        self.data.parcels.push(Parcel {
            id_hint: obj_id,
            number: chiban_label.unwrap_or(base.number),
            name: base.name,
            closed: base.closed,
            outer: base.outer,
            holes: base.holes,
            curve_segments: base.curve_segments,
            ring_curve_approximated: base.ring_curve_approximated,
            kind: ParcelKind::Chiban,
            attributes: attrs,
            source: elem_ref(elem),
        });
        Ok(())
    }

    fn resolve_gm_point_ref(
        &mut self,
        ref_elem: &'d Element,
        visited: &mut BTreeSet<usize>,
    ) -> Result<Option<MapLocation>, Error> {
        if let Some(inline) = self.find_child(ref_elem, b"GM_Point") {
            self.cx
                .enter()
                .map_err(|s| stopped(self.cx, s, Some(elem_ref(inline))))?;
            let res = self.read_gm_point(inline);
            self.cx.leave();
            return res;
        }
        let Some(target_id) = self.ref_target(ref_elem) else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "GM_Point")
                    .with("field", "reference")
                    .at(elem_ref(ref_elem)),
            );
            return Ok(None);
        };
        let Some(&idx) = self.ids.get(&target_id) else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_POINT_MISSING)
                    .with("target", target_id)
                    .at(elem_ref(ref_elem)),
            );
            return Ok(None);
        };
        if !visited.insert(idx) {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "GM_Point")
                    .with("field", "cycle")
                    .at(elem_ref(ref_elem)),
            );
            return Ok(None);
        }
        let Some(elem) = self.doc.element(idx) else {
            visited.remove(&idx);
            return Ok(None);
        };
        self.cx
            .enter()
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        let res = self.read_gm_point(elem);
        self.cx.leave();
        visited.remove(&idx);
        res
    }

    fn read_gm_point(&mut self, elem: &'d Element) -> Result<Option<MapLocation>, Error> {
        let Some(pos_elem) = self.find_child(elem, b"position") else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "GM_Point")
                    .with("field", "position")
                    .at(elem_ref(elem)),
            );
            return Ok(None);
        };
        self.read_direct_position(pos_elem)
    }

    fn read_direct_position(&mut self, elem: &'d Element) -> Result<Option<MapLocation>, Error> {
        let coord_elem = self.find_child(elem, b"coordinate").unwrap_or(elem);
        let raw = self.text_of(coord_elem)?;
        let mut nums = raw.split_whitespace();
        let x_opt = nums.next().and_then(|s| s.parse::<f64>().ok());
        let y_opt = nums.next().and_then(|s| s.parse::<f64>().ok());
        let z_opt = match nums.next() {
            None => None,
            Some(z_str) => match z_str.parse::<f64>().ok().filter(|z| z.is_finite()) {
                Some(z) => Some(norm_zero(z)),
                None => {
                    self.cx.report(
                        Diagnostic::new(codes::SIMA_VALUE_INVALID)
                            .with("kind", "DirectPosition")
                            .with("field", "altitude")
                            .at(elem_ref(coord_elem)),
                    );
                    None
                }
            },
        };
        let Some((x_north, y_east)) = x_opt.zip(y_opt) else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "DirectPosition")
                    .with("field", "coordinate")
                    .at(elem_ref(coord_elem)),
            );
            return Ok(None);
        };
        let Some(point) = parse_map_point(x_north, y_east) else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "DirectPosition")
                    .with("field", "coordinate")
                    .at(elem_ref(coord_elem)),
            );
            return Ok(None);
        };
        Ok(Some(MapLocation {
            point,
            altitude: z_opt,
        }))
    }

    fn resolve_gm_curve_ref(
        &mut self,
        ref_elem: &'d Element,
        visited: &mut BTreeSet<usize>,
    ) -> Result<Option<ResolvedCurve>, Error> {
        for name in [
            b"GM_Curve".as_slice(),
            b"GM_OrientableCurve".as_slice(),
            b"GM_CompositeCurve".as_slice(),
        ] {
            if let Some(inline) = self.find_child(ref_elem, name) {
                self.cx
                    .enter()
                    .map_err(|s| stopped(self.cx, s, Some(elem_ref(inline))))?;
                let res = self.read_gm_curve(inline, visited);
                self.cx.leave();
                return res;
            }
        }
        let Some(target_id) = self.ref_target(ref_elem) else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "GM_Curve")
                    .with("field", "reference")
                    .at(elem_ref(ref_elem)),
            );
            return Ok(None);
        };
        let Some(&idx) = self.ids.get(&target_id) else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_POINT_MISSING)
                    .with("target", target_id)
                    .at(elem_ref(ref_elem)),
            );
            return Ok(None);
        };
        if !visited.insert(idx) {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "GM_Curve")
                    .with("field", "cycle")
                    .at(elem_ref(ref_elem)),
            );
            return Ok(None);
        };
        let Some(elem) = self.doc.element(idx) else {
            visited.remove(&idx);
            return Ok(None);
        };
        self.cx
            .enter()
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        let res = self.read_gm_curve(elem, visited);
        self.cx.leave();
        visited.remove(&idx);
        res
    }

    fn read_gm_curve(
        &mut self,
        elem: &'d Element,
        visited: &mut BTreeSet<usize>,
    ) -> Result<Option<ResolvedCurve>, Error> {
        let mut reversed = false;
        if let Some(orient) = self.find_child(elem, b"orientation")
            && self.text_of(orient)?.trim() == "-"
        {
            reversed = true;
        }

        let mut points: Vec<MapLocation> = Vec::new();
        let mut segments: Vec<RouteSegment> = Vec::new();
        let mut has_curve = false;
        let mut has_own_segments = false;

        for child in child_elements(self.doc, elem) {
            let local = self.doc.parsed_local_name(child.name());
            if local == b"segment" {
                has_own_segments = true;
                for seg_elem in child_elements(self.doc, child) {
                    self.read_curve_segment(
                        seg_elem,
                        visited,
                        &mut points,
                        &mut segments,
                        &mut has_curve,
                    )?;
                }
            } else if local == b"generator" {
                has_own_segments = true;
                if let Some(sub) = self.resolve_gm_curve_ref(child, visited)? {
                    append_curve_points(&mut points, &sub.points);
                    segments.extend(sub.segments);
                    has_curve |= sub.has_curve;
                }
            }
        }

        // 自身の segment がなく primitive 参照で別の GM_Curve を指す GM_OrientableCurve の場合
        if !has_own_segments
            && let Some(prim) = self.find_child(elem, b"primitive")
            && let Some(target_id) = self.ref_target(prim)
            && self.element_id(elem).as_deref() != Some(target_id.as_str())
            && let Some(mut sub) = self.resolve_gm_curve_ref(prim, visited)?
        {
            if reversed {
                reverse_resolved_curve(&mut sub);
            }
            return Ok(Some(sub));
        }

        let mut resolved = ResolvedCurve {
            points,
            segments,
            has_curve,
        };
        if reversed {
            reverse_resolved_curve(&mut resolved);
        }
        Ok(Some(resolved))
    }

    fn read_curve_segment(
        &mut self,
        seg_elem: &'d Element,
        visited: &mut BTreeSet<usize>,
        points: &mut Vec<MapLocation>,
        segments: &mut Vec<RouteSegment>,
        has_curve: &mut bool,
    ) -> Result<(), Error> {
        let local = self.doc.parsed_local_name(seg_elem.name());
        if !matches!(
            local,
            b"GM_LineString" | b"GM_LineSegment" | b"GM_Arc" | b"GM_ArcString" | b"GM_Circle"
        ) {
            self.report_unsupported(seg_elem);
            return Ok(());
        }

        let Some(cp) = self.find_child(seg_elem, b"controlPoint") else {
            return Ok(());
        };
        let mut cols = Vec::new();
        for col in child_elements(self.doc, cp) {
            if self.doc.parsed_local_name(col.name()) != b"column" {
                continue;
            }
            self.cx
                .check(Kind::Work, 1)
                .map_err(|s| stopped(self.cx, s, Some(elem_ref(col))))?;
            if let Some(direct) = self.find_child(col, b"direct") {
                if let Some(loc) = self.read_direct_position(direct)? {
                    self.cx
                        .check(Kind::DecodedBytes, 32)
                        .map_err(|s| stopped(self.cx, s, Some(elem_ref(col))))?;
                    cols.push(loc);
                }
            } else if let Some(indirect) = self.find_child(col, b"indirect")
                && let Some(pt_ref) = self.find_child(indirect, b"point")
                && let Some(loc) = self.resolve_gm_point_ref(pt_ref, visited)?
            {
                self.cx
                    .check(Kind::DecodedBytes, 32)
                    .map_err(|s| stopped(self.cx, s, Some(elem_ref(col))))?;
                cols.push(loc);
            }
        }

        let src = elem_ref(seg_elem);
        match local {
            b"GM_Circle" => {
                let (Some(&p0), Some(&p1), Some(&p2)) = (cols.first(), cols.get(1), cols.get(2))
                else {
                    self.cx.report(
                        Diagnostic::new(codes::SIMA_VALUE_INVALID)
                            .with("kind", "GM_Circle")
                            .with("field", "controlPoint")
                            .at(src),
                    );
                    return Ok(());
                };
                let Some((direction, _, signed_radius)) =
                    three_point_arc(p0.point, p1.point, p2.point)
                else {
                    self.cx.report(
                        Diagnostic::new(codes::SIMA_VALUE_INVALID)
                            .with("kind", "GM_Circle")
                            .with("field", "controlPoint")
                            .at(src),
                    );
                    return Ok(());
                };
                let half_len = norm_zero(PI * libm::fabs(signed_radius));
                let Some((_, _, _, _, _, antipode)) =
                    arc_to_drawing(p0.point, direction, half_len, signed_radius)
                else {
                    self.cx.report(
                        Diagnostic::new(codes::SIMA_VALUE_INVALID)
                            .with("kind", "GM_Circle")
                            .with("field", "controlPoint")
                            .at(src),
                    );
                    return Ok(());
                };
                let dir2 = norm_zero((direction + PI).rem_euclid(2.0 * PI));
                self.cx
                    .check(Kind::Elements, 2)
                    .map_err(|s| stopped(self.cx, s, Some(src.clone())))?;
                self.cx
                    .check(Kind::DecodedBytes, 128)
                    .map_err(|s| stopped(self.cx, s, Some(src.clone())))?;
                segments.push(RouteSegment {
                    start: p0.point,
                    end: Some(antipode),
                    direction,
                    length: half_len,
                    kind: RouteSegmentKind::Arc {
                        radius: signed_radius,
                    },
                    source: src.clone(),
                });
                segments.push(RouteSegment {
                    start: antipode,
                    end: Some(p0.point),
                    direction: dir2,
                    length: half_len,
                    kind: RouteSegmentKind::Arc {
                        radius: signed_radius,
                    },
                    source: src.clone(),
                });
                let half1 = sample_arc_locations(
                    p0.point,
                    direction,
                    half_len,
                    signed_radius,
                    (p0.altitude, p2.altitude),
                    &src,
                    self.cx,
                )?;
                let half2 = sample_arc_locations(
                    antipode,
                    dir2,
                    half_len,
                    signed_radius,
                    (p2.altitude, p0.altitude),
                    &src,
                    self.cx,
                )?;
                append_curve_points(points, &half1);
                append_curve_points(points, &half2);
                *has_curve = true;
            }
            b"GM_Arc" | b"GM_ArcString" => {
                let kind_str = if local == b"GM_Arc" {
                    "GM_Arc"
                } else {
                    "GM_ArcString"
                };
                if cols.len() < 3 {
                    self.cx.report(
                        Diagnostic::new(codes::SIMA_VALUE_INVALID)
                            .with("kind", kind_str)
                            .with("field", "controlPoint")
                            .at(src),
                    );
                    return Ok(());
                }
                let mut i = 0usize;
                while i + 2 < cols.len() {
                    let (Some(&p0), Some(&p1), Some(&p2)) =
                        (cols.get(i), cols.get(i + 1), cols.get(i + 2))
                    else {
                        break;
                    };
                    if let Some((direction, length, radius)) =
                        three_point_arc(p0.point, p1.point, p2.point)
                    {
                        self.cx
                            .check(Kind::Elements, 1)
                            .map_err(|s| stopped(self.cx, s, Some(src.clone())))?;
                        self.cx
                            .check(Kind::DecodedBytes, 64)
                            .map_err(|s| stopped(self.cx, s, Some(src.clone())))?;
                        segments.push(RouteSegment {
                            start: p0.point,
                            end: Some(p2.point),
                            direction,
                            length,
                            kind: RouteSegmentKind::Arc { radius },
                            source: src.clone(),
                        });
                        let sampled = sample_arc_locations(
                            p0.point,
                            direction,
                            length,
                            radius,
                            (p0.altitude, p2.altitude),
                            &src,
                            self.cx,
                        )?;
                        append_curve_points(points, &sampled);
                        *has_curve = true;
                    } else {
                        self.cx.report(
                            Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                .with("kind", kind_str)
                                .with("field", "controlPoint")
                                .at(src.clone()),
                        );
                    }
                    i += 2;
                }
                if i + 1 < cols.len() {
                    self.cx.report(
                        Diagnostic::new(codes::SIMA_VALUE_INVALID)
                            .with("kind", kind_str)
                            .with("field", "controlPoint")
                            .at(src),
                    );
                }
            }
            _ => {
                for pair in cols.windows(2) {
                    let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                        continue;
                    };
                    if let Some((direction, length)) = bearing_and_distance(a.point, b.point) {
                        self.cx
                            .check(Kind::Elements, 1)
                            .map_err(|s| stopped(self.cx, s, Some(src.clone())))?;
                        self.cx
                            .check(Kind::DecodedBytes, 64)
                            .map_err(|s| stopped(self.cx, s, Some(src.clone())))?;
                        segments.push(RouteSegment {
                            start: a.point,
                            end: Some(b.point),
                            direction,
                            length,
                            kind: RouteSegmentKind::Line,
                            source: src.clone(),
                        });
                    }
                }
                append_curve_points(points, &cols);
            }
        }
        Ok(())
    }

    fn resolve_gm_surface_ref(
        &mut self,
        ref_elem: &'d Element,
        visited: &mut BTreeSet<usize>,
    ) -> Result<Option<SurfaceRings>, Error> {
        if let Some(inline) = self.find_child(ref_elem, b"GM_Surface") {
            self.cx
                .enter()
                .map_err(|s| stopped(self.cx, s, Some(elem_ref(inline))))?;
            let res = self.read_gm_surface(inline, visited);
            self.cx.leave();
            return res;
        }
        let Some(target_id) = self.ref_target(ref_elem) else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "GM_Surface")
                    .with("field", "reference")
                    .at(elem_ref(ref_elem)),
            );
            return Ok(None);
        };
        let Some(&idx) = self.ids.get(&target_id) else {
            self.cx.report(
                Diagnostic::new(codes::SIMA_POINT_MISSING)
                    .with("target", target_id)
                    .at(elem_ref(ref_elem)),
            );
            return Ok(None);
        };
        if !visited.insert(idx) {
            self.cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "GM_Surface")
                    .with("field", "cycle")
                    .at(elem_ref(ref_elem)),
            );
            return Ok(None);
        }
        let Some(elem) = self.doc.element(idx) else {
            visited.remove(&idx);
            return Ok(None);
        };
        self.cx
            .enter()
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
        let res = self.read_gm_surface(elem, visited);
        self.cx.leave();
        visited.remove(&idx);
        res
    }

    fn read_gm_surface(
        &mut self,
        elem: &'d Element,
        visited: &mut BTreeSet<usize>,
    ) -> Result<Option<SurfaceRings>, Error> {
        let mut outer = Vec::new();
        let mut holes = Vec::new();
        let mut curve_approx = false;
        let mut has_patch = false;

        for patch in child_elements(self.doc, elem) {
            if self.doc.parsed_local_name(patch.name()) != b"patch" {
                continue;
            }
            if let Some(poly) = self.find_child(patch, b"GM_Polygon") {
                if has_patch {
                    self.cx.report(
                        Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                            .with("kind", "surface_patch")
                            .at(elem_ref(poly)),
                    );
                    continue;
                }
                has_patch = true;
                if let Some(boundary) = self.find_child(poly, b"boundary") {
                    for ring_elem in child_elements(self.doc, boundary) {
                        let r_local = self.doc.parsed_local_name(ring_elem.name());
                        if r_local == b"exterior" {
                            if outer.is_empty() {
                                let (pts, approx) = self.read_gm_ring(ring_elem, visited)?;
                                outer = pts;
                                curve_approx |= approx;
                            } else {
                                self.cx.report(
                                    Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                        .with("kind", "exterior_ring")
                                        .at(elem_ref(ring_elem)),
                                );
                            }
                        } else if r_local == b"interior" {
                            let (h, approx) = self.read_gm_ring(ring_elem, visited)?;
                            if h.len() >= 3 {
                                holes.push(h);
                                curve_approx |= approx;
                            } else {
                                self.cx.report(
                                    Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                        .with("kind", "inner_ring")
                                        .at(elem_ref(ring_elem)),
                                );
                            }
                        }
                    }
                }
            } else if let Some(tri) = self.find_child(patch, b"GM_Triangle") {
                if has_patch {
                    self.cx.report(
                        Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                            .with("kind", "surface_patch")
                            .at(elem_ref(tri)),
                    );
                    continue;
                }
                has_patch = true;
                for corner in child_elements(self.doc, tri) {
                    if self.doc.parsed_local_name(corner.name()) != b"corners" {
                        continue;
                    }
                    if let Some(direct) = self.find_child(corner, b"direct") {
                        if let Some(loc) = self.read_direct_position(direct)? {
                            outer.push(loc);
                        }
                    } else if let Some(indirect) = self.find_child(corner, b"indirect")
                        && let Some(pt_ref) = self.find_child(indirect, b"point")
                        && let Some(loc) = self.resolve_gm_point_ref(pt_ref, visited)?
                    {
                        outer.push(loc);
                    }
                }
                if outer.len() >= 2
                    && outer.first().map(|p| p.point) == outer.last().map(|p| p.point)
                {
                    outer.pop();
                }
            } else {
                for other in child_elements(self.doc, patch) {
                    self.report_unsupported(other);
                }
            }
        }
        Ok((!outer.is_empty()).then_some((outer, holes, curve_approx)))
    }

    fn read_gm_ring(
        &mut self,
        ring_elem: &'d Element,
        visited: &mut BTreeSet<usize>,
    ) -> Result<(Vec<MapLocation>, bool), Error> {
        let target_ring = if let Some(inline_ring) = self.find_child(ring_elem, b"GM_Ring") {
            inline_ring
        } else if let Some(target_id) = self.ref_target(ring_elem) {
            let Some(&idx) = self.ids.get(&target_id) else {
                self.cx.report(
                    Diagnostic::new(codes::SIMA_POINT_MISSING)
                        .with("target", target_id)
                        .at(elem_ref(ring_elem)),
                );
                return Ok((Vec::new(), false));
            };
            if !visited.insert(idx) {
                self.cx.report(
                    Diagnostic::new(codes::SIMA_VALUE_INVALID)
                        .with("kind", "GM_Ring")
                        .with("field", "cycle")
                        .at(elem_ref(ring_elem)),
                );
                return Ok((Vec::new(), false));
            }
            let Some(elem) = self.doc.element(idx) else {
                visited.remove(&idx);
                return Ok((Vec::new(), false));
            };
            self.cx
                .enter()
                .map_err(|s| stopped(self.cx, s, Some(elem_ref(elem))))?;
            let res = self.read_gm_ring_body(elem, visited);
            self.cx.leave();
            visited.remove(&idx);
            return res;
        } else {
            ring_elem
        };

        self.cx
            .enter()
            .map_err(|s| stopped(self.cx, s, Some(elem_ref(target_ring))))?;
        let res = self.read_gm_ring_body(target_ring, visited);
        self.cx.leave();
        res
    }

    fn read_gm_ring_body(
        &mut self,
        target_ring: &'d Element,
        visited: &mut BTreeSet<usize>,
    ) -> Result<(Vec<MapLocation>, bool), Error> {
        let mut pts = Vec::new();
        let mut has_curve = false;
        for child in child_elements(self.doc, target_ring) {
            if self.doc.parsed_local_name(child.name()) == b"generator"
                && let Some(curve) = self.resolve_gm_curve_ref(child, visited)?
            {
                append_curve_points(&mut pts, &curve.points);
                has_curve |= curve.has_curve;
            }
        }
        if pts.len() >= 2 && pts.first().map(|p| p.point) == pts.last().map(|p| p.point) {
            pts.pop();
        }
        Ok((pts, has_curve))
    }
}

fn append_curve_points(dst: &mut Vec<MapLocation>, src: &[MapLocation]) {
    for &pt in src {
        if dst.last().map(|p| p.point) != Some(pt.point) {
            dst.push(pt);
        }
    }
}

fn reverse_resolved_curve(curve: &mut ResolvedCurve) {
    curve.points.reverse();
    curve.segments.reverse();
    for seg in &mut curve.segments {
        if let Some(end) = seg.end {
            let old_start = seg.start;
            let seg_len = seg.length;
            seg.start = end;
            seg.end = Some(old_start);
            match &mut seg.kind {
                RouteSegmentKind::Line => {
                    if let Some((dir, _)) = bearing_and_distance(seg.start, old_start) {
                        seg.direction = dir;
                    } else {
                        seg.direction = norm_zero((seg.direction + PI).rem_euclid(2.0 * PI));
                    }
                }
                RouteSegmentKind::Arc { radius } => {
                    let old_end_dir = if radius.is_finite() && libm::fabs(*radius) > 1e-9 {
                        seg.direction + seg_len / (*radius)
                    } else {
                        seg.direction
                    };
                    *radius = norm_zero(-*radius);
                    seg.direction = norm_zero((old_end_dir + PI).rem_euclid(2.0 * PI));
                }
                RouteSegmentKind::Clothoid {
                    parameter,
                    start_radius,
                    end_radius,
                } => {
                    let [k1, k2] =
                        clothoid_curvatures(*parameter, seg_len, *start_radius, *end_radius);
                    let old_end_dir = seg.direction + 0.5 * (k1 + k2) * seg_len;
                    let new_start = end_radius.map(|r| norm_zero(-r));
                    let new_end = start_radius.map(|r| norm_zero(-r));
                    *start_radius = new_start;
                    *end_radius = new_end;
                    seg.direction = norm_zero((old_end_dir + PI).rem_euclid(2.0 * PI));
                }
            }
        }
    }
}
