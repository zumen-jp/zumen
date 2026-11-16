//! 新しい P21 の表と用紙。外部のひな形は使わず、SXF Ver.3.1 の AP202 サブセットの実体を組む。
//! 幅を持つ仮の円を各幅に 1 個置き、apply の表を作った後で必ず除く。

use super::Item;
use super::quiet;
use crate::{Sxf, SxfFile};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use zumen_base::{Cx, Error, Kind};
use zumen_drawing::{Inherit, Layer, LineTypeId, Sheet};
use zumen_sdk::Codec;

struct Writer {
    out: String,
    next: u64,
}
impl Writer {
    fn add(&mut self, body: String) -> u64 {
        let id = self.next;
        self.next += 1;
        let _ = writeln!(self.out, "#{id}={body};");
        id
    }
}

/// STEP の文字列。ASCII 以外は X2、引用符と逆斜線はエスケープする。
fn quoted(value: &str) -> String {
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

fn real(value: f64) -> Result<String, Error> {
    super::super::apply::p21::real(value).ok_or(Error::Invalid)
}

pub(super) fn make(
    items: &mut [Item],
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
    colors.entry([0; 3]).or_insert(0);
    widths.entry(0.25_f64.to_bits()).or_insert(0);
    let names = layers.iter().map(|l| l.name.len() as u64).sum::<u64>();
    cx.check(Kind::Work, names.saturating_add(layers.len() as u64 * 64))?;
    cx.check(
        Kind::DecodedBytes,
        16384 + names.saturating_mul(16) + layers.len() as u64 * 512,
    )?;
    let mut writer = Writer {
        out: concat!(
            "ISO-10303-21;\nHEADER;\n",
            "FILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\n",
            "FILE_NAME('zumen.p21','',(''),(''),'zumen','zumen','');\n",
            "FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n"
        )
        .into(),
        next: 1,
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
    let mut layer_ids = Vec::new();
    for i in 0..layers.len().max(1) {
        let layer = layers.get(i);
        let fallback = format!("Layer {}", i + 1);
        let name = layer
            .map(|l| l.name.as_str())
            .filter(|name| !name.is_empty() && !name.starts_with("$$SXF_dummy"))
            .unwrap_or(&fallback);
        let id = writer.add(format!(
            "PRESENTATION_LAYER_ASSIGNMENT({},'',({}))",
            quoted(name),
            if i == 0 {
                format!("#{point},{}", seeds.join(","))
            } else {
                format!("#{point}")
            }
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
    let sheet = writer.add(format!(
        "DRAWING_SHEET_REVISION('{name}',({}),'','')",
        seeds.join(",")
    ));
    writer.add(format!("PRESENTATION_SIZE(#{sheet},#{frame})"));
    for layer in layer_ids {
        writer.add(format!("PRESENTATION_LAYER_USAGE(#{layer},#{sheet})"));
    }
    writer.out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    quiet(cx, |cx| Sxf::read(writer.out.as_bytes(), cx))
}
