//! 保存・開き直しをせずに `Document` が保持している状態から、公開 API（`entities`・`tables`・選択・診断・表示）と
//! 同じ値を組み立てる検査用の入口。
//!
//! 試作自身を開き直した結果との一致だけで代用せず、`Today`（今の実装）の出力と直接突き合わせるために使う。

use std::collections::BTreeMap;

use zumen_base::Ref;
use zumen_drawing::id::{
    layer as layer_id_str, line_type as line_type_id_str, origin as id_origin,
};
use zumen_drawing::{
    Element, Hash, Inherit, Layer, LayerId, LineType, LineTypeId, ModelPoint, Shape,
};

use crate::display::{RowBuffers, bulged, split_f64};
use crate::lift::{Kind, LineTypeRef, NO_DEFINITION};
use crate::store::Document;

/// 保持している 1 要素の検査結果
#[derive(Clone, Debug, PartialEq)]
pub struct InspectedElement {
    /// 行番号
    pub row: u32,
    /// ハンドル
    pub handle: Option<u64>,
    /// 公開 API の ID（`space/0/h...` または連番）
    pub id: String,
    /// 公開 API の `kind`（`"line"`・`"circle"`・`"arc"`・`"polyline"`・`"text"`・`"symbol"`）
    pub kind: &'static str,
    /// 画層の名前
    pub layer: String,
    /// `v2:...` のハッシュ
    pub hash: String,
    /// 囲む箱 `[min_x, min_y, max_x, max_y]`
    pub bounds: [f64; 4],
}

/// 保持している表の 1 項目（公開 ID・名前・ハッシュ）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectedTableItem {
    /// 公開 ID
    pub id: String,
    /// 名前
    pub name: String,
    /// ハッシュ
    pub hash: String,
}

/// 今の実装で保存ファイルを開き直したときに付く `LayerId` と `LineTypeId` への詰め直し表
pub struct LiveTableRemap {
    /// 旧 `LayerId.0` → 新 `LayerId`
    pub layers: Vec<Option<LayerId>>,
    /// 詰め直した後の生きている画層列
    pub live_layers: Vec<Layer>,
    /// 旧 `LineTypeId.0` → 新 `LineTypeId`
    pub line_types: Vec<Option<LineTypeId>>,
    /// 詰め直した後の生きている線種列
    pub live_line_types: Vec<LineType>,
}

fn api_kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Line => "line",
        Kind::Circle => "circle",
        Kind::Arc => "curve",
        Kind::Polyline => "polyline",
        Kind::Text => "text",
        Kind::Insert => "symbol",
    }
}

/// 今の実装の `Lifter` と同じ順（`TABLES` → ファイル順の `ENTITIES` → `BLOCKS`）で、生きている画層・線種 ID を割り当て直す
pub fn live_table_remap(doc: &Document) -> LiveTableRemap {
    let mut layer_map: Vec<Option<LayerId>> = vec![None; doc.layers.len()];
    let mut live_layers: Vec<Layer> = Vec::with_capacity(doc.layers.len());
    let mut lt_map: Vec<Option<LineTypeId>> = vec![None; doc.line_types.len()];
    let mut live_lts: Vec<LineType> = Vec::with_capacity(doc.line_types.len());

    let mut visit_layer = |old: u32| {
        let idx = old as usize;
        if let Some(slot) = layer_map.get_mut(idx)
            && slot.is_none()
            && let Some(orig) = doc.layers.get(idx)
        {
            let new_id = LayerId(live_layers.len() as u32);
            *slot = Some(new_id);
            let mut copy = orig.clone();
            copy.id = new_id;
            live_layers.push(copy);
        }
    };

    let mut visit_lt = |old: u32| {
        if old == u32::MAX {
            return;
        }
        let idx = old as usize;
        if let Some(slot) = lt_map.get_mut(idx)
            && slot.is_none()
            && let Some(orig) = doc.line_types.get(idx)
        {
            let new_id = LineTypeId(live_lts.len() as u32);
            *slot = Some(new_id);
            let mut copy = orig.clone();
            copy.id = new_id;
            live_lts.push(copy);
        }
    };

    // 1. TABLES に定義のある項目（および定義画層が参照する線種）は元の登録順のまま先頭に並ぶ
    let mut table_lts = vec![false; doc.line_types.len()];
    for (idx, lt) in doc.line_types.iter().enumerate() {
        if lt.source != Ref::default()
            && let Some(slot) = table_lts.get_mut(idx)
        {
            *slot = true;
        }
    }
    for (idx, l) in doc.layers.iter().enumerate() {
        if l.source != Ref::default() {
            if let Some(ltid) = l.style.as_ref().and_then(|s| s.line_type)
                && let Some(slot) = table_lts.get_mut(ltid.0 as usize)
            {
                *slot = true;
            }
            visit_layer(idx as u32);
        }
    }
    for (idx, &in_tables) in table_lts.iter().enumerate() {
        if in_tables {
            visit_lt(idx as u32);
        }
    }

    // 2. ファイル順の ENTITIES（6 種類の行と 6 種類以外のオブジェクトを `row_sort_key` でマージ）
    let row_indices = doc.live_row_indices();
    let mut non_slice: Vec<&crate::store::NonSliceUsage> = doc
        .non_slice_usages
        .iter()
        .filter(|u| !doc.gone.contains(&u.record))
        .collect();
    non_slice.sort_by_key(|u| u.record);

    let mut ri = 0usize;
    let mut ni = 0usize;
    while ri < row_indices.len() || ni < non_slice.len() {
        let r_key = row_indices
            .get(ri)
            .and_then(|&idx| doc.rows.get(idx as usize))
            .map(|r| doc.row_sort_key(r));
        let n_key = non_slice.get(ni).map(|u| (u.record, 0u32));

        let take_row = match (r_key, n_key) {
            (Some(rk), Some(nk)) => rk <= nk,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if take_row {
            if let Some(&row_idx) = row_indices.get(ri)
                && let Some(&r) = doc.rows.get(row_idx as usize)
            {
                if let Some(es_idx) = r.extra_style.checked_sub(1)
                    && let Some(es) = doc.shapes.pools.extra_styles.get(es_idx as usize)
                    && matches!(es.line_type, LineTypeRef::Named(_))
                {
                    visit_lt(es.line_type_id);
                }
                visit_layer(r.layer);
                if r.kind == Kind::Insert
                    && let Some(ins) = doc.shapes.inserts.get(r.shape as usize)
                {
                    for a in doc.shapes.pools.attribs_of(ins) {
                        if let Some(es_idx) = a.extra_style.checked_sub(1)
                            && let Some(es) = doc.shapes.pools.extra_styles.get(es_idx as usize)
                            && matches!(es.line_type, LineTypeRef::Named(_))
                        {
                            visit_lt(es.line_type_id);
                        }
                        visit_layer(a.layer);
                    }
                }
            }
            ri += 1;
        } else {
            if let Some(u) = non_slice.get(ni) {
                for &ltid in &u.line_types {
                    visit_lt(ltid.0);
                }
                for &lid in &u.layers {
                    visit_layer(lid.0);
                }
            }
            ni += 1;
        }
    }

    // 3. BLOCKS のブロック定義
    for sym in &doc.symbols {
        for el in &sym.space.elements {
            if let Inherit::Own(ltid) = el.style.line_type {
                visit_lt(ltid.0);
            }
            visit_layer(el.layer.0);
        }
        for ad in &sym.attributes {
            if let Inherit::Own(ltid) = ad.attribute.style.line_type {
                visit_lt(ltid.0);
            }
            visit_layer(ad.attribute.layer.0);
        }
    }

    for layer in &mut live_layers {
        if let Some(ref mut style) = layer.style
            && let Some(ltid) = style.line_type
            && let Some(Some(new_ltid)) = lt_map.get(ltid.0 as usize)
        {
            style.line_type = Some(*new_ltid);
        }
    }

    LiveTableRemap {
        layers: layer_map,
        live_layers,
        line_types: lt_map,
        live_line_types: live_lts,
    }
}

fn remap_element(el: &mut Element<ModelPoint>, remap: &LiveTableRemap) {
    if let Some(Some(new_lid)) = remap.layers.get(el.layer.0 as usize) {
        el.layer = *new_lid;
    }
    if let Inherit::Own(ltid) = el.style.line_type
        && let Some(Some(new_ltid)) = remap.line_types.get(ltid.0 as usize)
    {
        el.style.line_type = Inherit::Own(*new_ltid);
    }
    if let Shape::Symbol(ref mut sym) = el.shape {
        for attr in &mut sym.attributes {
            if let Some(Some(new_lid)) = remap.layers.get(attr.layer.0 as usize) {
                attr.layer = *new_lid;
            }
            if let Inherit::Own(ltid) = attr.style.line_type
                && let Some(Some(new_ltid)) = remap.line_types.get(ltid.0 as usize)
            {
                attr.style.line_type = Inherit::Own(*new_ltid);
            }
        }
    }
}

fn dedup_element_id(
    el: &Element<ModelPoint>,
    index: usize,
    seen: &mut BTreeMap<String, u64>,
) -> String {
    let orig = id_origin(&el.source, index);
    let repeat =
        if el.source.key().is_none() && el.source.record().is_none() && el.source.bytes().is_none()
        {
            0
        } else {
            let count = seen.entry(orig.clone()).or_insert(0);
            *count += 1;
            *count - 1
        };
    match repeat {
        0 => format!("space/0/{orig}"),
        n => format!("space/0/{orig}/{n}"),
    }
}

fn dedup_table_id(base: String, seen: &mut BTreeMap<String, u64>) -> String {
    let count = seen.entry(base.clone()).or_default();
    let result = if *count == 0 {
        base
    } else {
        format!("{base}/{count}")
    };
    *count = count.saturating_add(1);
    result
}

/// 保持しているモデル空間の 6 種類の全要素をファイル順に組み立てて返す
pub fn live_elements(doc: &Document) -> Vec<InspectedElement> {
    let remap = live_table_remap(doc);
    let indices = doc.live_row_indices();
    let mut seen = BTreeMap::new();
    let mut out = Vec::with_capacity(indices.len());

    for (pos, row_idx) in indices.into_iter().enumerate() {
        let Some(&row) = doc.rows.get(row_idx as usize) else {
            continue;
        };
        let Some(mut el) = doc.element(row_idx) else {
            continue;
        };
        remap_element(&mut el, &remap);
        let id = dedup_element_id(&el, pos, &mut seen);
        let hash = Hash::element(&el).to_string();
        let layer_name = doc
            .layers
            .get(row.layer as usize)
            .map(|l| l.name.clone())
            .unwrap_or_default();
        let bx = doc.boxes.get(row_idx as usize).copied().unwrap_or_default();
        out.push(InspectedElement {
            row: row_idx,
            handle: row.handle(),
            id,
            kind: api_kind(row.kind),
            layer: layer_name,
            hash,
            bounds: bx,
        });
    }
    out
}

/// 保持している画層と線種の表を、公開 API の `tables` と同じ ID・名前・ハッシュの列にして返す
pub fn live_tables(doc: &Document) -> (Vec<InspectedTableItem>, Vec<InspectedTableItem>) {
    let remap = live_table_remap(doc);
    let mut layer_seen = BTreeMap::new();
    let layers = remap
        .live_layers
        .iter()
        .map(|l| InspectedTableItem {
            id: dedup_table_id(layer_id_str(l), &mut layer_seen),
            name: l.name.clone(),
            hash: Hash::layer(l).to_string(),
        })
        .collect();

    let mut lt_seen = BTreeMap::new();
    let line_types = remap
        .live_line_types
        .iter()
        .map(|lt| InspectedTableItem {
            id: dedup_table_id(line_type_id_str(lt), &mut lt_seen),
            name: lt.name.clone(),
            hash: Hash::line_type(lt).to_string(),
        })
        .collect();

    (layers, line_types)
}

/// 保持している空間索引で矩形 `rect` を選び、ファイル順の公開 ID の列にして返す
pub fn select_ids(doc: &Document, rect: [f64; 4], inspected: &[InspectedElement]) -> Vec<String> {
    let mut by_row: BTreeMap<u32, &str> = BTreeMap::new();
    for item in inspected {
        by_row.insert(item.row, item.id.as_str());
    }
    let mut rows = Vec::new();
    doc.select(rect, &mut rows);
    rows.sort_by_key(|&r| {
        doc.rows
            .get(r as usize)
            .map_or((u32::MAX, u32::MAX), |row| doc.row_sort_key(row))
    });
    rows.into_iter()
        .filter_map(|r| by_row.get(&r).map(|&id| id.to_owned()))
        .collect()
}

/// 開いたときに記録した 6 種類の図形と単位の診断のうち、今も残っているものを `code` ごとの件数に集計する
pub fn open_diagnostic_counts(doc: &Document) -> BTreeMap<&'static str, usize> {
    let mut map = BTreeMap::new();
    for d in &doc.open_diagnostics {
        if d.record.is_some_and(|rec| doc.gone.contains(&rec)) {
            continue;
        }
        *map.entry(d.kind.code_str()).or_default() += 1;
    }
    map
}

/// 今の実装（`Today`）が返した 1 要素の `Geometry` JSON から、文書エンジンの `Display::row` が持つべき表示プリミティブを組む
pub fn display_from_today_geometry(
    doc: &Document,
    row: u32,
    geom: &zumen::messages::Geometry,
) -> Option<RowBuffers> {
    use zumen::messages::Geometry;
    let radians = |degrees: f64| degrees * (core::f64::consts::PI / 180.0);
    let seg = |[x0, y0]: [f64; 2], [x1, y1]: [f64; 2]| -> [f32; 8] {
        let ([a, b], [c, d], [e, f], [g, h]) =
            (split_f64(x0), split_f64(y0), split_f64(x1), split_f64(y1));
        [a, b, c, d, e, f, g, h]
    };
    let arc_val = |[cx, cy]: [f64; 2], radius: f64, start: f64, sweep: f64| -> [f32; 7] {
        let ([a, b], [c, d]) = (split_f64(cx), split_f64(cy));
        [a, b, c, d, radius as f32, start as f32, sweep as f32]
    };

    let mut out = RowBuffers::default();
    match geom {
        Geometry::Line(line) => {
            let &[x0, y0] = line.start.as_slice() else {
                return None;
            };
            let &[x1, y1] = line.end.as_slice() else {
                return None;
            };
            out.segments.push(seg([x0, y0], [x1, y1]));
        }
        Geometry::Circle(c) => {
            let &[cx, cy] = c.center.as_slice() else {
                return None;
            };
            out.arcs
                .push(arc_val([cx, cy], c.radius, 0.0, core::f64::consts::TAU));
        }
        Geometry::Arc(a) => {
            let &[cx, cy] = a.center.as_slice() else {
                return None;
            };
            let sweep_deg = match (a.end_angle - a.start_angle).rem_euclid(360.0) {
                0.0 => 360.0,
                sweep => sweep,
            };
            out.arcs.push(arc_val(
                [cx, cy],
                a.radius,
                radians(a.start_angle),
                radians(sweep_deg),
            ));
        }
        Geometry::Polyline(poly) => {
            let n = poly.points.len();
            let last = if poly.closed && n > 1 {
                n
            } else {
                n.saturating_sub(1)
            };
            for idx in 0..last {
                let p_slice = poly.points.get(idx)?.as_slice();
                let q_slice = poly.points.get((idx + 1) % n)?.as_slice();
                let (&[px, py], &[qx, qy]) = (p_slice, q_slice) else {
                    return None;
                };
                let bulge = poly
                    .bulges
                    .as_ref()
                    .and_then(|b| b.get(idx).copied())
                    .unwrap_or(0.0);
                if bulge == 0.0 {
                    out.segments.push(seg([px, py], [qx, qy]));
                } else {
                    out.arcs.push(bulged([px, py], [qx, qy], bulge));
                }
            }
        }
        Geometry::Text(t) => {
            let &[px, py] = t.position.as_slice() else {
                return None;
            };
            let ([a, b], [c, d]) = (split_f64(px), split_f64(py));
            out.texts.push([
                a,
                b,
                c,
                d,
                t.height as f32,
                radians(t.rotation) as f32,
                t.width_factor.unwrap_or(1.0) as f32,
                radians(t.oblique.unwrap_or(0.0)) as f32,
            ]);
        }
        Geometry::Symbol(sym) => {
            let &[px, py] = sym.position.as_slice() else {
                return None;
            };
            let &[sx, sy] = sym.scale.as_slice() else {
                return None;
            };
            let def_idx = doc
                .def_by_name
                .get(&sym.name)
                .copied()
                .unwrap_or(NO_DEFINITION);
            let (cols, rows_cnt, col_sp, row_sp) = match &sym.grid {
                Some(g) => (
                    u32::try_from(g.columns).unwrap_or(1).max(1),
                    u32::try_from(g.rows).unwrap_or(1).max(1),
                    g.column_spacing,
                    g.row_spacing,
                ),
                None => (1, 1, 0.0, 0.0),
            };
            let ([a, b], [c, d]) = (split_f64(px), split_f64(py));
            out.instances.push((
                [
                    a,
                    b,
                    c,
                    d,
                    sx as f32,
                    sy as f32,
                    radians(sym.rotation) as f32,
                    col_sp as f32,
                    row_sp as f32,
                ],
                [def_idx, cols, rows_cnt],
            ));
            // 公開 API の `SymbolGeometry.attributes` はタグと文字列値のみを持つため、表示する属性の配置値は
            // 保持している `AttribRow` から同じ規則で作り、記号本体の配置・尺度・回転・格子と定義参照を `Today` の `Geometry` と照合する。
            if let Some(r) = doc.rows.get(row as usize)
                && let Some(ins) = doc.shapes.inserts.get(r.shape as usize)
            {
                let pools = &doc.shapes.pools;
                for attrib in pools.attribs_of(ins) {
                    if !attrib.hidden() {
                        let at = attrib.text.point;
                        let ([ta, tb], [tc, td]) = (split_f64(at[0]), split_f64(at[1]));
                        let rot_deg = match attrib.text.place(pools) {
                            crate::lift::Place::Between { first, second, .. }
                                if first != second =>
                            {
                                libm::atan2(second[1] - first[1], second[0] - first[0]).to_degrees()
                            }
                            _ => attrib.text.rotation,
                        };
                        out.texts.push([
                            ta,
                            tb,
                            tc,
                            td,
                            attrib.text.height as f32,
                            radians(rot_deg) as f32,
                            attrib.text.width_factor(pools) as f32,
                            radians(attrib.text.oblique(pools)) as f32,
                        ]);
                    }
                }
            }
        }
        _ => return None,
    }
    Some(out)
}
