//! 弧長寸法。共通部分は ODS 5.4.1 の DIMENSION、固有部分は読む側と同じ並び。
//! 固有部分の並びは公開 DWG/DXF の組からの推定で、docs/formats/dwg.md に記録する。

use super::*;

/// 共通部分と 3 つの定義点を通常の寸法の書き手へ渡し、部分・角度・引出線の値を続ける。
pub(super) fn write(
    fields: &mut BitWriter,
    old: &Object,
    new: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<()> {
    let (old_common, _) = split(old)?;
    let (new_common, arc) = split(new)?;
    dimension(
        fields,
        &old_common,
        &new_common,
        kinds::DIMENSION_ANGULAR_3PT,
        data,
        start,
        release,
    )?;
    fields.bit(flag(&arc, 70)?);
    // 弧長固有の 40・41 はラジアンのまま共有表に置かれている。
    fields.bd(real(&arc, 40)??);
    fields.bd(real(&arc, 41)??);
    fields.bit(flag(&arc, 71)?);
    point3(fields, &arc, 16)?;
    point3(fields, &arc, 17)?;
    Some(())
}

fn flag(object: &Object, code: i16) -> Option<bool> {
    match small(object, code)?? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn split(object: &Object) -> Option<(Object, Object)> {
    let at = object.values.iter().position(|value| value.code == 13)?;
    let (head, tail) = object.values.split_at_checked(at)?;
    let make = |values| Object {
        kind: object.kind.clone(),
        section: object.section.clone(),
        values,
        origin: object.origin,
    };
    let mut common = head.to_vec();
    common.extend(
        tail.iter()
            .filter(|value| matches!(value.code, 13 | 23 | 33 | 14 | 24 | 34 | 15 | 25 | 35))
            .cloned(),
    );
    Some((make(common), make(tail.to_vec())))
}
