//! 新しい P21 の表と用紙。外部のひな形は使わず、SXF Ver.3.1 の AP202 サブセットの実体を組む。
//! 幅を持つ仮の円を各幅に 1 個置き、apply の表を作った後で必ず除く。

use super::Item;
use super::dimension::{self, Linear};
use super::quiet;
use crate::{Sxf, SxfFile};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use zumen_base::{Cx, Error, Kind};
use zumen_drawing::{Inherit, Layer, LineTypeId, Sheet};
use zumen_sdk::Codec;

pub(super) struct Writer {
    out: String,
    next: u64,
    multiline: bool,
}
impl Writer {
    pub(super) fn context(&mut self, length: u64) -> u64 {
        let angle = self.add("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))".into());
        self.add(format!("(GEOMETRIC_REPRESENTATION_CONTEXT(2) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{angle},#{length})) REPRESENTATION_CONTEXT('ID1','2D'))"))
    }

    pub(super) fn add(&mut self, body: String) -> u64 {
        let id = self.next;
        self.next += 1;
        let _ = write!(self.out, "#{id}=");
        if self.multiline && body.starts_with('(') {
            // 複合実体の各型を独立した行に書く。ラベルの文字列中の括弧・引用符には手を加えない。
            let mut depth = 0;
            let mut quoted = false;
            for ch in body.chars() {
                if ch == '\'' {
                    quoted = !quoted;
                }
                if !quoted && ch == ')' {
                    depth -= 1;
                }
                if !(depth == 1 && !quoted && ch == ' ') {
                    self.out.push(ch);
                }
                if !quoted && ch == '(' {
                    depth += 1;
                }
                if !quoted && (ch == '(' || ch == ')') && depth == 1 {
                    self.out.push('\n');
                }
            }
        } else {
            self.out.push_str(&body);
        }
        self.out.push_str(";\n");
        id
    }
}

/// STEP の文字列。ASCII 以外は X2、引用符と逆斜線はエスケープする。
pub(super) fn quoted(value: &str) -> String {
    let mut out = String::from("'");
    for ch in value.chars() {
        match ch {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            ' '..='~' => out.push(ch),
            _ => {
                out.push_str("\\X2\\");
                for unit in ch.encode_utf16(&mut [0; 2]) {
                    let _ = write!(out, "{unit:04X}");
                }
                out.push_str("\\X0\\");
            }
        }
    }
    out.push('\'');
    out
}

pub(super) fn real(value: f64) -> Result<String, Error> {
    super::super::apply::p21::real(value).ok_or(Error::Invalid)
}

pub(super) fn make(
    items: &mut [Item],
    dimensions: &[Linear],
    layers: &[Layer],
    sheet: Option<&Sheet>,
    cx: &mut Cx<'_>,
) -> Result<SxfFile, Error> {
    cx.check(Kind::DecodedBytes, 1024)?;
    let mut colors = BTreeMap::new();
    let mut widths = BTreeMap::new();
    let mut patterns = BTreeMap::new();
    for item in &*items {
        cx.check(Kind::Work, 64 + item.appearance.dashes.len() as u64)?;
        cx.check(
            Kind::DecodedBytes,
            2048 + item.appearance.dashes.len() as u64 * 256,
        )?;
        colors.insert(item.appearance.rgb, 0);
        widths.insert(item.appearance.width.to_bits(), 0);
        if !item.appearance.dashes.is_empty() {
            patterns.insert(
                item.appearance
                    .dashes
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                0,
            );
        }
    }
    for dim in dimensions {
        cx.check(Kind::Work, 1)?;
        colors.insert(dim.rgb, 0);
        widths.insert(dim.width.to_bits(), 0);
    }
    colors.entry([0; 3]).or_insert(0);
    widths.entry(0.25_f64.to_bits()).or_insert(0);
    let names = layers.iter().map(|l| l.name.len() as u64).sum::<u64>();
    cx.check(Kind::Work, names.saturating_add(layers.len() as u64 * 64))?;
    cx.check(
        Kind::DecodedBytes,
        16384 + names.saturating_mul(16) + layers.len() as u64 * 512,
    )?;
    let metadata = if dimensions.is_empty() {
        "'zumen.p21','',(''),(''),'zumen','zumen',''"
    } else {
        // AP202 サブセット編 3-1-2。実行時刻に依存しない交換用のタイムスタンプ。
        "'zumen.p21','1970-01-01T00:00:00',(''),(''),'zumen$$3.1','zumen',''"
    };
    let mut writer = Writer {
        out: format!(
            "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\nFILE_NAME({metadata});\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n"
        ),
        next: 1,
        multiline: !dimensions.is_empty(),
    };
    let unit = writer.add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))".into());
    let continuous = writer.add("DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous')".into());
    for (rgb, id) in &mut colors {
        *id = writer.add(format!(
            "COLOUR_RGB('',{},{},{})",
            real(f64::from(rgb[0]) / 255.0)?,
            real(f64::from(rgb[1]) / 255.0)?,
            real(f64::from(rgb[2]) / 255.0)?
        ));
    }
    for (i, (pattern, id)) in patterns.iter_mut().enumerate() {
        let mut refs = Vec::new();
        for pair in pattern.as_chunks::<2>().0 {
            refs.push(format!(
                "#{}",
                writer.add(format!(
                    "CURVE_STYLE_FONT_PATTERN({},{})",
                    real(f64::from_bits(pair[0]))?,
                    real(-f64::from_bits(pair[1]))?
                ))
            ));
        }
        *id = writer.add(format!(
            "CURVE_STYLE_FONT('line-{}',({}))",
            i + 1,
            refs.join(",")
        ));
    }
    for item in items {
        let code = if item.appearance.dashes.is_empty() {
            1
        } else {
            cx.check(
                Kind::Work,
                (patterns.len() as u64).saturating_mul(item.appearance.dashes.len() as u64 + 1),
            )?;
            cx.check(Kind::DecodedBytes, item.appearance.dashes.len() as u64 * 8)?;
            let key: Vec<_> = item.appearance.dashes.iter().map(|v| v.to_bits()).collect();
            17 + patterns
                .keys()
                .position(|p| p == &key)
                .ok_or(Error::Invalid)? as u32
        };
        item.element.style.line_type = Inherit::Own(LineTypeId(code));
    }
    let color = *colors.values().next().ok_or(Error::Invalid)?;
    let point = writer.add("CARTESIAN_POINT('',(0.,0.))".into());
    let axis = writer.add(format!("AXIS2_PLACEMENT_2D('',#{point},$)"));
    let circle = writer.add(format!("CIRCLE('',#{axis},1.)"));
    let mut seeds = Vec::new();
    for (width, id) in &mut widths {
        *id = writer.add(format!(
            "LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE({}),#{unit})",
            real(f64::from_bits(*width))?
        ));
        let style = writer.add(format!("CURVE_STYLE('',#{continuous},#{id},#{color})"));
        let assignment = writer.add(format!("PRESENTATION_STYLE_ASSIGNMENT((#{style}))"));
        seeds.push(format!(
            "#{}",
            writer.add(format!(
                "ANNOTATION_CURVE_OCCURRENCE('',(#{assignment}),#{circle})"
            ))
        ));
    }
    let seed_circles = seeds.join(",");
    let mut layer_members = dimension::write(
        &mut writer,
        dimensions,
        &colors,
        &widths,
        continuous,
        axis,
        unit,
        &mut seeds,
        cx,
    )?;
    let mut layer_ids = Vec::new();
    if let Some(instances) = layer_members.remove(&0) {
        let refs = instances
            .iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(",");
        layer_ids.push(writer.add(format!(
            "PRESENTATION_LAYER_ASSIGNMENT('$$SXF_dummy_layer_for_subfigure',' ',({refs}))"
        )));
    }
    for i in 0..layers.len().max(1) {
        let layer = layers.get(i);
        let fallback = format!("Layer {}", i + 1);
        let name = layer
            .map(|l| l.name.as_str())
            .filter(|name| !name.is_empty() && !name.starts_with("$$SXF_dummy"))
            .unwrap_or(&fallback);
        let mut members = format!("#{point}");
        if i == 0 {
            members.push(',');
            members.push_str(&seed_circles);
        }
        if let Some(extra) = layer_members.remove(&(i + 1)) {
            for id in extra {
                let _ = write!(members, ",#{id}");
            }
        }
        let id = writer.add(format!(
            "PRESENTATION_LAYER_ASSIGNMENT({},'',({members}))",
            quoted(name)
        ));
        layer_ids.push(id);
        if layer
            .and_then(|l| l.state)
            .is_some_and(|s| !s.visible || s.frozen)
        {
            writer.add(format!("INVISIBILITY((#{id}))"));
        }
    }
    let size = sheet
        .and_then(|s| s.size)
        .map(|size| [size[0].0, size[1].0]);
    let name = match size {
        Some([1189.0, 841.0]) => "A0_horizontal",
        Some([841.0, 594.0]) => "A1_horizontal",
        Some([594.0, 420.0]) => "A2_horizontal",
        Some([420.0, 297.0]) => "A3_horizontal",
        Some([297.0, 210.0]) => "A4_horizontal",
        Some([841.0, 1189.0]) => "A0_vertical",
        Some([594.0, 841.0]) => "A1_vertical",
        Some([420.0, 594.0]) => "A2_vertical",
        Some([297.0, 420.0]) => "A3_vertical",
        Some([210.0, 297.0]) => "A4_vertical",
        Some(_) => "custom",
        None => "A3_horizontal",
    };
    let extent = size
        .filter(|s| s.iter().all(|v| v.is_finite() && *v > 0.0))
        .unwrap_or([420.0, 297.0]);
    let frame = writer.add(format!(
        "PLANAR_BOX('',{},{},#{axis})",
        real(extent[0])?,
        real(extent[1])?
    ));
    seeds.push(format!("#{frame}"));
    let members = seeds.join(",");
    let sheet = if dimensions.is_empty() {
        writer.add(format!(
            "DRAWING_SHEET_REVISION('{name}',({members}),'','')"
        ))
    } else {
        let definition = writer.add("DRAWING_DEFINITION('01',$)".into());
        let revision = writer.add(format!("DRAUGHTING_DRAWING_REVISION('01',#{definition},$)"));
        writer.add(format!("DRAUGHTING_TITLE((#{revision}),'JAPANESE',' ')"));
        let context = writer.context(unit);
        let id = writer.add(format!(
            "DRAWING_SHEET_REVISION('{name}',({members}),#{context},'01')"
        ));
        writer.add(format!(
            "DRAWING_SHEET_REVISION_USAGE(#{id},#{revision},'01')"
        ));
        id
    };
    writer.add(format!("PRESENTATION_SIZE(#{sheet},#{frame})"));
    for layer in layer_ids {
        writer.add(format!("PRESENTATION_LAYER_USAGE(#{layer},#{sheet})"));
    }
    writer.out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    quiet(cx, |cx| Sxf::read(writer.out.as_bytes(), cx))
}
