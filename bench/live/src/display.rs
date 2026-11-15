//! 表示のバッファー。本体は `zumen-live` の [`zumen_live::display`] に置き、このモジュールでは
//! 試作の列型（[`Geom`]・[`Pools`]）から [`DisplayWriter`] へ書き出す橋渡しだけを定義する。

use crate::lift::{Geom, POLY_CLOSED, Pools, TextRow};

pub use zumen_live::dirty::{DIRTY_BLOCK, DirtyColumn};
pub use zumen_live::display::{
    Display, DisplayDirty, DisplayWriter, NO_OWNER, RowBuffers, Slot, arc_f32, bulged,
    ccw_sweep_deg, edges, instance_f32, radians_from_deg, segment_f32, split_f64, text_f32,
};

/// 文字行の値を GPU 文字プリミティブ表現に直す
fn lettering(row: &TextRow, pools: &Pools) -> [f32; 8] {
    let rot_deg = match row.place(pools) {
        crate::lift::Place::Between { first, second, .. } if first != second => {
            libm::atan2(second[1] - first[1], second[0] - first[0]).to_degrees()
        }
        _ => row.rotation,
    };
    text_f32(
        row.point,
        row.height,
        radians_from_deg(rot_deg),
        row.width_factor(pools),
        radians_from_deg(row.oblique(pools)),
    )
}

/// 要素が要る線分・円弧・文字・挿入の数
fn geom_counts(geom: &Geom, pools: &Pools) -> [u32; 4] {
    match geom {
        Geom::Line(_) => [1, 0, 0, 0],
        Geom::Circle(_) | Geom::Arc(_) => [0, 1, 0, 0],
        Geom::Polyline {
            first,
            count,
            flags,
            ..
        } => {
            let mut counts = [0; 4];
            for (_, _, bulge) in pools.edges_of(*first, *count, flags & POLY_CLOSED != 0) {
                counts[usize::from(bulge != 0.0)] += 1;
            }
            counts
        }
        Geom::Text(_) => [0, 0, 1, 0],
        Geom::Insert(insert) => {
            let shown = pools
                .attribs_of(insert)
                .filter(|attribute| !attribute.hidden())
                .count();
            [0, 0, shown as u32, 1]
        }
    }
}

/// 要素のプリミティブを [`DisplayWriter`] へ書き込む
fn write_geom(w: &mut DisplayWriter<'_>, geom: &Geom, pools: &Pools, style: [u32; 2]) {
    match geom {
        Geom::Line([x0, y0, x1, y1]) => {
            w.push_segment([*x0, *y0], [*x1, *y1]);
        }
        Geom::Circle([x, y, radius]) => {
            w.push_circle([*x, *y], *radius);
        }
        Geom::Arc([x, y, radius, start, end]) => {
            w.push_arc_deg([*x, *y], *radius, *start, *end);
        }
        Geom::Polyline {
            first,
            count,
            flags,
            ..
        } => {
            for (from, to, bulge) in pools.edges_of(*first, *count, flags & POLY_CLOSED != 0) {
                if bulge == 0.0 {
                    w.push_segment(from, to);
                } else {
                    w.push_bulged(from, to, bulge);
                }
            }
        }
        Geom::Text(row) => {
            w.push_text_raw(lettering(row, pools), style);
        }
        Geom::Insert(insert) => {
            let grid = insert.grid(pools);
            let spacing = insert.spacing(pools);
            w.push_instance(
                insert.position,
                insert.scale,
                radians_from_deg(insert.rotation),
                spacing,
                insert.definition,
                grid,
            );
            for attribute in pools.attribs_of(insert).filter(|a| !a.hidden()) {
                w.push_text_raw(
                    lettering(&attribute.text, pools),
                    [attribute.layer, attribute.color],
                );
            }
        }
    }
}

/// 試作の [`Geom`] を [`Display`] へ追加・更新する拡張メソッド
pub trait DisplayGeomExt {
    /// 次の行の要素を末尾に足す
    fn push(&mut self, geom: &Geom, pools: &Pools, style: [u32; 2]);
    /// 行 `row` の要素を書き直す
    fn update(&mut self, row: u32, geom: &Geom, pools: &Pools, style: [u32; 2]);
}

impl DisplayGeomExt for Display {
    fn push(&mut self, geom: &Geom, pools: &Pools, style: [u32; 2]) {
        let counts = geom_counts(geom, pools);
        self.push_with(counts, style, |w| write_geom(w, geom, pools, style));
    }

    fn update(&mut self, row: u32, geom: &Geom, pools: &Pools, style: [u32; 2]) {
        let counts = geom_counts(geom, pools);
        self.update_with(row, counts, style, |w| write_geom(w, geom, pools, style));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lift::{ATTRIB_HIDDEN, AttribRow, InsertExtra, InsertRow};
    use crate::paged::TextSpan;

    #[test]
    fn an_update_keeps_the_place_when_counts_match() {
        let mut pools = Pools::default();
        pools
            .vertices
            .extend_from_slice(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.5], [1.0, 1.0, 0.0]]);
        let line = Geom::Polyline {
            first: 0,
            count: 3,
            flags: POLY_CLOSED,
            width: 0.0,
        };
        let mut display = Display::default();
        display.push(&Geom::Line([0.0, 0.0, 1.0, 1.0]), &pools, [0, 0]);
        display.push(&line, &pools, [0, 0]);
        assert_eq!(display.segments.len(), 3);
        assert_eq!(display.arcs.len(), 1);
        let before = display.slots.clone();
        *pools.vertices.get_mut(2).expect("v") = [2.0, 2.0, 0.0];
        display.update(1, &line, &pools, [3, 4]);
        assert_eq!(display.slots, before);
        assert_eq!(display.dead, 0);
        assert_eq!(
            display.segment_style.slice_to_vec(0, 3),
            [[0, 0], [3, 4], [3, 4]]
        );
        pools.vertices.get_mut(1).expect("v")[2] = 0.0;
        display.update(1, &line, &pools, [0, 0]);
        assert_eq!(display.dead, 3);
        assert_eq!(display.slot_counts(1)[0], 3);
        assert_eq!(core::mem::size_of::<Slot>(), 16);
        assert_eq!(
            display.segment_owner.slice_to_vec(0, 6),
            [0, NO_OWNER, NO_OWNER, 1, 1, 1]
        );
    }

    fn text(x: f64, flags: u8) -> TextRow {
        TextRow {
            point: [x, 0.0],
            height: 2.5,
            rotation: 0.0,
            content: TextSpan::EMPTY,
            style: TextSpan::STANDARD,
            extra: 0,
            h_align: 0,
            v_align: 0,
            flags,
            pad: 0,
        }
    }

    #[test]
    fn an_insert_is_one_instance_and_its_shown_attributes() {
        let attribute = |x: f64, hidden: bool| AttribRow {
            handle: 0,
            text: text(x, if hidden { ATTRIB_HIDDEN } else { 0 }),
            tag: TextSpan::EMPTY,
            record: 0,
            layer: 7,
            color: 9,
            extra_style: 0,
        };
        let mut pools = Pools::default();
        pools.attribs.extend([
            attribute(1.0, false),
            attribute(2.0, true),
            attribute(3.0, false),
        ]);
        pools.insert_extras.push(InsertExtra {
            grid: [3, 2],
            spacing: [5.0, 6.0],
            clip: 0,
        });
        let insert = Geom::Insert(InsertRow {
            position: [10.0, 20.0],
            scale: [2.0, 2.0],
            rotation: 90.0,
            name: TextSpan::EMPTY,
            definition: 4,
            attrib_first: 0,
            attrib_count: 3,
            attached: 3,
            extra: 1,
            flags: 1,
            pad: [0; 3],
        });
        let mut display = Display::default();
        display.push(&insert, &pools, [1, 2]);
        assert_eq!(display.instance_ref.slice_to_vec(0, 1), [[4, 3, 2]]);
        assert_eq!(display.instance_style.slice_to_vec(0, 1), [[1, 2]]);
        assert_eq!(display.texts.len(), 2);
        assert_eq!(display.text_style.slice_to_vec(0, 2), [[7, 9], [7, 9]]);
        assert_eq!(display.texts.get(1).expect("t")[0], 3.0);
        let row = display.row(0);
        assert_eq!(row.instances.len(), 1);
        assert_eq!(
            row.instances[0].0[4..],
            [2.0, 2.0, core::f32::consts::FRAC_PI_2, 5.0, 6.0]
        );
    }

    #[test]
    fn dirty_blocks_sync_gpu_mirror_with_minimal_bytes() {
        let pools = Pools::default();
        let mut display = Display::with_rows(5000);
        for i in 0..5000 {
            let x = f64::from(i);
            display.push(&Geom::Line([x, 0.0, x + 1.0, 1.0]), &pools, [0, 1]);
        }
        let mut mirror = display.clone();
        display.clear_dirty();
        assert!(!display.has_dirty());
        assert_eq!(display.dirty_bytes(), 0);

        display.update(2500, &Geom::Line([99.0, 5.0, 100.0, 6.0]), &pools, [0, 1]);
        assert!(display.has_dirty());
        assert_eq!(display.dirty_bytes(), DIRTY_BLOCK * 32);
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);
        assert!(!display.has_dirty());

        display.update(2500, &Geom::Line([99.0, 5.0, 100.0, 6.0]), &pools, [0, 5]);
        assert_eq!(display.dirty_bytes(), DIRTY_BLOCK * 8);
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);

        display.hide(1200);
        assert_eq!(display.dirty_bytes(), DIRTY_BLOCK * 4);
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);

        display.push(&Geom::Circle([10.0, 20.0, 3.0]), &pools, [2, 4]);
        assert!(display.has_dirty());
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);

        display.pop();
        assert!(display.has_dirty());
        display.sync_gpu_mirror(&mut mirror);
        assert_eq!(display, mirror);
    }
}
