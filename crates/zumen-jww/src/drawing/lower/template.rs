//! Jw_cad 公開データ形式 jwdatafmt.txt のヘッダーと CDataSen。
//! https://www.jwcad.net/jwdatafmt.txt
//! 外部の図面・設定ファイルを使わず、700 のヘッダーを組み立てる。

use super::quiet;
use super::{Item, Slots};
use crate::{Jww, JwwFile};
use std::collections::{BTreeMap, BTreeSet};
use zumen_base::{Cx, Error, Kind};
use zumen_drawing::{Color, Drawing2D, Inherit, LineTypeId, LineWeight, PaperLength, Shape, Sheet};
use zumen_sdk::Codec;

pub(super) fn paper(sheet: Option<&Sheet>) -> u32 {
    sheet
        .and_then(|s| s.size)
        .and_then(|size| {
            super::super::PAPERS
                .iter()
                .find(|(_, known)| *known == [size[0].0, size[1].0])
                .map(|(code, _)| *code)
        })
        .unwrap_or(3)
}

pub(super) fn make(
    items: &mut [Item],
    slots: &Slots,
    drawing: &Drawing2D,
    paper: u32,
    cx: &mut Cx<'_>,
) -> Result<(JwwFile, usize), Error> {
    cx.check(Kind::DecodedBytes, 65536)?;
    let mut spec = Spec::new(700);
    spec.paper = paper;
    spec.max_draw_width = -101;
    let mut colors = BTreeSet::new();
    let mut patterns = BTreeSet::new();
    for item in &*items {
        cx.check(Kind::Work, 64 + item.appearance.dashes.len() as u64)?;
        cx.check(
            Kind::DecodedBytes,
            512 + item.appearance.dashes.len() as u64 * 16,
        )?;
        colors.insert(item.appearance.rgb);
        if !item.appearance.dashes.is_empty() && item.appearance.dashes.len() <= 10 {
            patterns.insert(
                item.appearance
                    .dashes
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
            );
        }
    }
    let colors: BTreeMap<_, _> = colors
        .into_iter()
        .take(257)
        .enumerate()
        .map(|(i, rgb)| (rgb, i))
        .collect();
    for (rgb, i) in &colors {
        let color = u32::from(rgb[0]) | u32::from(rgb[1]) << 8 | u32::from(rgb[2]) << 16;
        spec.sxf_pens.push((*i, color, 25));
    }
    let patterns: BTreeMap<_, _> = patterns
        .into_iter()
        .take(33)
        .enumerate()
        .map(|(i, pattern)| (pattern, i))
        .collect();
    for (pattern, i) in &patterns {
        spec.sxf_line_types.push((
            *i,
            format!("line-{}", i + 1),
            pattern.iter().map(|v| f64::from_bits(*v).abs()).collect(),
        ));
    }
    for item in items {
        cx.check(Kind::Work, 64 + item.appearance.dashes.len() as u64)?;
        cx.check(Kind::DecodedBytes, item.appearance.dashes.len() as u64 * 16)?;
        let key: Vec<_> = item.appearance.dashes.iter().map(|v| v.to_bits()).collect();
        let line_type = if key.is_empty() {
            1
        } else if let Some(i) = patterns.get(&key) {
            30 + *i as u32
        } else {
            item.appearance.reasons.push("line_type_limit");
            item.fidelity = super::Fidelity::Drawn;
            1
        };
        for element in &mut item.elements {
            let plain = matches!(element.shape, Shape::Point(_) | Shape::Text(_));
            element.style.color = Inherit::Own(Color {
                code: None,
                rgb: Some(item.appearance.rgb),
            });
            element.style.line_type = if plain {
                Inherit::Layer
            } else {
                Inherit::Own(LineTypeId(line_type))
            };
            element.style.line_weight = (!plain).then_some(Inherit::Own(LineWeight::Width(
                PaperLength(item.appearance.width),
            )));
            element.style.line_type_scale = 1.0;
            element.style.transparency = Inherit::Layer;
        }
    }
    let mut groups = BTreeSet::new();
    let mut name_bytes = 0_u64;
    for (i, id) in slots.layers.iter().enumerate() {
        let Some(id) = id else {
            continue;
        };
        cx.check(Kind::Work, 1 + drawing.layers.len() as u64)?;
        let layer = drawing.layer(*id);
        cx.check(
            Kind::DecodedBytes,
            layer
                .map_or(64, |l| l.name.len() as u64)
                .saturating_mul(4)
                .saturating_add(128),
        )?;
        let name = layer.map_or_else(|| format!("Layer {}", i + 1), |l| l.name.clone());
        name_bytes = name_bytes.saturating_add(name.len() as u64);
        cx.check(
            Kind::DecodedBytes,
            (name.len() as u64).saturating_mul(4).saturating_add(128),
        )?;
        spec.layer_names.push((i, name));
        let group = i / 16;
        groups.insert(group);
        let state = spec.groups.get_mut(group).ok_or(Error::Invalid)?;
        state.0 = 2;
        state.1 = 1.0;
        *state.2.get_mut(i % 16).ok_or(Error::Invalid)? =
            layer.and_then(|l| l.state).map_or(2, |s| {
                if !s.visible || s.frozen {
                    0
                } else if !s.editable {
                    1
                } else {
                    2
                }
            });
    }
    cx.check(Kind::Work, 65536 + name_bytes)?;
    cx.check(Kind::DecodedBytes, 65536 + name_bytes.saturating_mul(4))?;
    let (mut out, background_at) = spec.build();
    super::super::apply::write_count(&mut out, groups.len());
    let mut numbers = BTreeMap::new();
    let mut next = 1;
    for group in groups {
        // CArchive のクラスのスキーマは、既存の自作入力と同じ 1（docs/formats/jww.md）。
        super::super::apply::write_object_tag(&mut out, &mut numbers, &mut next, b"CDataSen", 1);
        out.extend(0_u32.to_le_bytes());
        out.push(1);
        out.extend(1_u16.to_le_bytes());
        out.extend(25_u16.to_le_bytes());
        out.extend(0_u16.to_le_bytes());
        out.extend((group as u16).to_le_bytes());
        out.extend(0_u16.to_le_bytes());
        for value in [0.0_f64, 0.0, 1.0, 0.0] {
            out.extend(value.to_le_bytes());
        }
    }
    super::super::apply::write_count(&mut out, 0);
    out.extend(0_u32.to_le_bytes()); // 画像はない
    Ok((quiet(cx, |cx| Jww::read(&out, cx))?, background_at))
}

/// CString の長さ。UTF-16 は符号単位の数で、終端 NUL を含まない。
fn string_len(out: &mut Vec<u8>, len: usize) {
    if len < 255 {
        out.push(len as u8);
    } else if len < 65534 {
        out.push(255);
        out.extend((len as u16).to_le_bytes());
    } else {
        out.push(255);
        out.extend(65535_u16.to_le_bytes());
        out.extend((len as u32).to_le_bytes());
    }
}
fn cstring(out: &mut Vec<u8>, value: &str) {
    if value.is_ascii() {
        string_len(out, value.len());
        out.extend(value.as_bytes());
    } else {
        out.extend([255, 254, 255]);
        string_len(out, value.encode_utf16().count());
        for unit in value.encode_utf16() {
            out.extend(unit.to_le_bytes());
        }
    }
}

/// ヘッダーの値。指定しないフィールドは 0、名前は空
struct Spec {
    pub version: u32,
    pub paper: u32,
    /// レイヤグループの状態・縮尺・レイヤの状態
    pub groups: [(u32, f64, [u32; 16]); 16],
    /// レイヤの位置（グループ × 16 + レイヤ）と名前
    pub layer_names: Vec<(usize, String)>,
    /// 書き込むレイヤグループ
    pub write_group: u32,
    /// グループの位置と名前
    pub group_names: Vec<(usize, String)>,
    /// レイヤの位置（グループ × 16 + レイヤ）と保護
    pub protects: Vec<(usize, u32)>,
    pub max_draw_width: i32,
    pub pens: [u32; 10],
    pub printer_widths: [u32; 10],
    /// 線種 2〜9 と 16〜19
    pub patterns: [[u32; 4]; 12],
    /// 印刷の設定の 11 番目
    pub print_flag: u32,
    /// SXF の拡張色の位置・色・幅
    pub sxf_pens: Vec<(usize, u32, u32)>,
    /// SXF の線種の位置・名前・線と間
    pub sxf_line_types: Vec<(usize, String, Vec<f64>)>,
}

impl Spec {
    fn new(version: u32) -> Self {
        Self {
            version,
            paper: 0,
            groups: [(0, 0.0, [0; 16]); 16],
            layer_names: Vec::new(),
            write_group: 0,
            group_names: Vec::new(),
            protects: Vec::new(),
            max_draw_width: 0,
            pens: [0; 10],
            printer_widths: [0; 10],
            patterns: [[0; 4]; 12],
            print_flag: 0,
            sxf_pens: Vec::new(),
            sxf_line_types: Vec::new(),
        }
    }

    fn build(&self) -> (Vec<u8>, usize) {
        let version = self.version;
        let since = |v: u32, n: usize| if version >= v { n } else { 0 };
        let mut out = b"JwwData.".to_vec();
        out.extend(version.to_le_bytes());
        out.push(0);
        out.extend(self.paper.to_le_bytes());
        out.extend(self.write_group.to_le_bytes());
        for (group, (state, scale, layers)) in self.groups.iter().enumerate() {
            out.extend(state.to_le_bytes());
            out.extend([0; 4]);
            out.extend(scale.to_le_bytes());
            out.extend([0; 4]);
            for (layer, state) in layers.iter().enumerate() {
                let at = group * 16 + layer;
                let protect = self.protects.iter().find(|(i, _)| *i == at);
                out.extend(state.to_le_bytes());
                out.extend(protect.map_or(0, |p| p.1).to_le_bytes());
            }
        }
        out.extend([0; 80]);
        out.extend(self.max_draw_width.to_le_bytes());
        out.extend([0; 72]);
        for at in 0..256 {
            match self.layer_names.iter().find(|(i, _)| *i == at) {
                Some((_, name)) => cstring(&mut out, name),
                None => out.push(0),
            }
        }
        for at in 0..16 {
            match self.group_names.iter().find(|(i, _)| *i == at) {
                Some((_, name)) => cstring(&mut out, name),
                None => out.push(0),
            }
        }
        let jumps = if version >= 300 { 224 } else { 96 };
        out.extend(vec![
            0;
            28 + since(300, 16)
                + 4
                + 24
                + 24
                + jumps
                + since(300, 56)
                + 80
                + 8
        ]);
        // jwdatafmt.txt「色番号ごとの画面表示色、線幅」の 0 番は画面背景。
        let background_at = out.len();
        for pen in self.pens {
            out.extend(pen.to_le_bytes());
            out.extend([0; 4]);
        }
        for width in self.printer_widths {
            out.extend([0; 4]);
            out.extend(width.to_le_bytes());
            out.extend([0; 8]);
        }
        let words = |out: &mut Vec<u8>, pattern: &[u32; 4]| {
            for word in pattern {
                out.extend(word.to_le_bytes());
            }
        };
        for pattern in &self.patterns[..8] {
            words(&mut out, pattern);
        }
        out.extend([0; 100]);
        for pattern in &self.patterns[8..] {
            words(&mut out, pattern);
        }
        out.extend([0; 40]);
        out.extend(self.print_flag.to_le_bytes());
        out.extend(vec![0; since(223, 60) + since(225, 32) + since(230, 8)]);
        if version >= 420 {
            let pen = |at: usize| self.sxf_pens.iter().find(|(i, _, _)| *i == at);
            for at in 0..257 {
                out.extend(pen(at).map_or(0, |p| p.1).to_le_bytes());
                out.extend([0; 4]);
            }
            for at in 0..257 {
                out.push(0);
                out.extend([0; 4]);
                out.extend(pen(at).map_or(0, |p| p.2).to_le_bytes());
                out.extend([0; 8]);
            }
            out.extend(vec![0; 33 * 16]);
            for at in 0..33 {
                match self.sxf_line_types.iter().find(|(i, _, _)| *i == at) {
                    Some((_, name, pitches)) => {
                        cstring(&mut out, name);
                        out.extend((pitches.len() as u32).to_le_bytes());
                        for i in 0..10 {
                            out.extend(pitches.get(i).copied().unwrap_or(0.0).to_le_bytes());
                        }
                    }
                    None => out.extend([0; 85]),
                }
            }
        }
        out.extend(vec![0; 380]);
        (out, background_at)
    }
}
