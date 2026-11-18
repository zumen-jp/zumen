//! 同じ apply の中で先に足した表を指す変更。表だけを作業用の文書に準備し、要素の変更はまとめて検査する。

use super::*;
use crate::changes::changes_at;
use std::collections::{BTreeMap, BTreeSet};
use zumen_drawing::{Changes, Edit, Position};

type Reference<'a> = (u8, &'a str, &'static str);

fn addition(edit: &messages::Edit) -> Option<(u8, &str)> {
    match edit {
        messages::Edit::AddLayer(edit) => Some((0, &edit.name)),
        messages::Edit::AddLineType(edit) => Some((1, &edit.name)),
        messages::Edit::AddTextStyle(edit) => Some((2, &edit.name)),
        messages::Edit::AddDimensionStyle(edit) => Some((3, &edit.name)),
        messages::Edit::AddSymbol(edit) => Some((4, &edit.name)),
        _ => None,
    }
}

fn references(edit: &messages::Edit) -> [Option<Reference<'_>>; 4] {
    let (layer, line_type, geometry) = match edit {
        messages::Edit::Add(edit) => (
            edit.layer.as_deref(),
            edit.line_type.as_ref(),
            Some(&edit.geometry),
        ),
        messages::Edit::Restyle(edit) => (edit.layer.as_deref(), edit.line_type.as_ref(), None),
        messages::Edit::Replace(edit) => (None, None, Some(&edit.geometry)),
        messages::Edit::AddLayer(edit) => (None, edit.line_type.as_ref(), None),
        messages::Edit::Layer(edit) => (None, edit.line_type.as_ref(), None),
        _ => (None, None, None),
    };
    let style = match geometry {
        Some(messages::Geometry::Text(text)) => text
            .style
            .as_deref()
            .map(|name| (2, name, "geometry/style")),
        Some(messages::Geometry::FormattedText(text)) => text
            .style
            .as_deref()
            .map(|name| (2, name, "geometry/style")),
        Some(messages::Geometry::Dimension(dimension)) => {
            Some((3, dimension.style.as_str(), "geometry/style"))
        }
        Some(messages::Geometry::Leader(leader)) => {
            Some((3, leader.style.as_str(), "geometry/style"))
        }
        _ => None,
    };
    // 足した記号の定義を置く記号（#684）
    let symbol = match geometry {
        Some(messages::Geometry::Symbol(symbol)) => {
            Some((4, symbol.name.as_str(), "geometry/name"))
        }
        _ => None,
    };
    [
        layer.map(|name| (0, name, "layer")),
        line_type
            .and_then(|line| line.name.as_deref())
            .map(|name| (1, name, "line_type")),
        style,
        symbol,
    ]
}

fn key(kind: u8, name: &str, cx: &mut Cx<'_>) -> Result<(u8, String), Failed> {
    let size = (name.len() as u64).saturating_add(256);
    cx.check(Kind::Work, size)
        .and_then(|()| cx.check(Kind::DecodedBytes, size))
        .map_err(|stop| stopped(cx, stop))?;
    Ok((kind, name.to_ascii_lowercase()))
}

/// 新しい表への後方参照があるときだけ、この経路を使う。
pub(super) fn needed(request: &ApplyRequest, cx: &mut Cx<'_>) -> Result<bool, Failed> {
    if request.edits.len() < 2 {
        return Ok(false);
    }
    let mut names = BTreeSet::new();
    for edit in &request.edits {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        for (kind, name, _) in references(edit).into_iter().flatten() {
            if !names.is_empty() && names.contains(&key(kind, name, cx)?) {
                return Ok(true);
            }
        }
        if let Some((kind, name)) = addition(edit) {
            names.insert(key(kind, name, cx)?);
        }
    }
    Ok(false)
}

fn invalid(reason: &str, cx: &mut Cx<'_>) -> Failed {
    fail(
        cx,
        Diagnostic::new(codes::CHANGE_REFERENCES_FAILED).with("reason", reason),
    )
}

fn reserve<T>(values: &mut Vec<T>, count: usize, cx: &mut Cx<'_>) -> Result<(), Failed> {
    super::details::reserve(values, count, 1024, cx).map_err(|error| match error {
        super::details::Error::Stopped(stop) => stopped(cx, stop),
        super::details::Error::Invalid(reason) => invalid(reason, cx),
    })
}

/// 形式ごとの既存の書き戻しを使い、成功するまで呼び出し元のファイルを変えない。
#[allow(clippy::too_many_arguments)]
pub(super) fn run<F>(
    request: &ApplyRequest,
    original: &Drawing2D,
    original_hash: Hash,
    file: &F,
    mut apply: impl FnMut(&F, &Changes, &mut Cx<'_>) -> Result<(F, Vec<Applied>, bool), Failed>,
    mut lift: impl FnMut(&F, &mut Cx<'_>) -> Result<Drawing2D, Failed>,
    cx: &mut Cx<'_>,
) -> Result<(F, Vec<Applied>, bool), Failed> {
    let condition = changes_at(&[], request.hash.as_deref(), original, original, 0, cx)?;
    if condition.document.is_some_and(|hash| hash != original_hash) {
        return Err(fail(cx, Diagnostic::new(codes::CHANGE_DOCUMENT_STALE)));
    }
    let mut first = BTreeMap::new();
    for (index, edit) in request.edits.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if let Some((kind, name)) = addition(edit) {
            first.entry(key(kind, name, cx)?).or_insert(index);
        }
    }
    let mut results = Vec::new();
    let mut deferred = Vec::new();
    let mut slots = Vec::new();
    reserve(&mut results, request.edits.len(), cx)?;
    reserve(&mut deferred, request.edits.len(), cx)?;
    reserve(&mut slots, request.edits.len(), cx)?;
    results.resize_with(request.edits.len(), || Applied {
        id: None,
        hash: None,
    });
    let mut prepared: Option<F> = None;
    let mut current = Cow::Borrowed(original);
    let mut shifted = false;
    for (index, edit) in request.edits.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        // 名前で持つ文字・寸法スタイルも、後から追加する定義へ先回りして結ばない。
        for (kind, name, field) in references(edit).into_iter().flatten() {
            if first
                .get(&key(kind, name, cx)?)
                .is_some_and(|first| *first >= index)
            {
                return Err(fail(cx, invalid_value(format!("/edits/{index}/{field}"))));
            }
        }
        let adding = addition(edit).is_some();
        let resolved = changes_at(
            std::slice::from_ref(edit),
            None,
            if adding { &current } else { original },
            &current,
            index,
            cx,
        )?;
        if adding {
            let (next, mut applied, moved) =
                apply(prepared.as_ref().unwrap_or(file), &resolved, cx)?;
            let after = lift(&next, cx)?;
            shifted |= moved
                || (keyless(&current, cx)? && record_ids_shifted(&current, &after, &resolved, cx)?);
            let Some(result) = applied.pop().filter(|_| applied.is_empty()) else {
                return Err(invalid("result-count", cx));
            };
            *results
                .get_mut(index)
                .ok_or_else(|| invalid("result-count", cx))? = result;
            current = Cow::Owned(after);
            prepared = Some(next);
        } else {
            // この時点で参照できる表だけで検査する。出所は全表を足した後に引き直す。
            slots.push(index);
        }
    }
    for &index in &slots {
        let edit = request
            .edits
            .get(index)
            .ok_or_else(|| invalid("result-count", cx))?;
        let mut resolved = changes_at(
            std::slice::from_ref(edit),
            None,
            original,
            &current,
            index,
            cx,
        )?;
        deferred.append(&mut resolved.edits);
    }
    remap(original, &current, &mut deferred, cx)?;
    if !deferred.is_empty() {
        let changes = Changes {
            edits: deferred,
            document: None,
        };
        let (next, applied, moved) = apply(prepared.as_ref().unwrap_or(file), &changes, cx)?;
        let after = lift(&next, cx)?;
        shifted |= moved
            || (keyless(&current, cx)? && record_ids_shifted(&current, &after, &changes, cx)?);
        if applied.len() != slots.len() {
            return Err(invalid("result-count", cx));
        }
        for (index, applied) in slots.into_iter().zip(applied) {
            *results
                .get_mut(index)
                .ok_or_else(|| invalid("result-count", cx))? = applied;
        }
        current = Cow::Owned(after);
        prepared = Some(next);
    }
    settle_tables(request, &current, &mut results, cx)?;
    Ok((
        prepared.ok_or_else(|| invalid("no-addition", cx))?,
        results,
        shifted,
    ))
}

type Identity = (String, Hash, Option<Key>);

fn identities(
    drawing: &Drawing2D,
    cx: &mut Cx<'_>,
) -> Result<BTreeMap<Position, Identity>, Failed> {
    let mut result = BTreeMap::new();
    id::each_element(drawing, |position, name, element| {
        cx.check(Kind::Work, 256)
            .and_then(|()| cx.check(Kind::DecodedBytes, 512))
            .map_err(|stop| stopped(cx, stop))?;
        let source = match element {
            id::Found::Model(element) => &element.source,
            id::Found::Paper(element) => &element.source,
        };
        let hash = element.hash_checked(cx).map_err(|stop| stopped(cx, stop))?;
        if result
            .insert(position, (name, hash, source.key()))
            .is_some()
        {
            return Err(invalid("ambiguous-position", cx));
        }
        Ok(())
    })?;
    Ok(result)
}

/// 表と記号の定義の追加だけなら、元の要素の順と値は変わらない。両方を照合し、番号が動いた公開 ID だけを引き直す。
fn remap(
    before: &Drawing2D,
    after: &Drawing2D,
    changes: &mut [Edit],
    cx: &mut Cx<'_>,
) -> Result<(), Failed> {
    let before = identities(before, cx)?;
    let mut after = identities(after, cx)?;
    // 足した記号の定義の中の要素だけが増える（#684）
    if before.len() > after.len() {
        return Err(invalid("element-count", cx));
    }
    let mut names = BTreeMap::new();
    for (position, (old, hash, key)) in before {
        cx.check(Kind::Work, 256)
            .and_then(|()| cx.check(Kind::DecodedBytes, 512))
            .map_err(|stop| stopped(cx, stop))?;
        let Some((new, new_hash, new_key)) = after.remove(&position) else {
            return Err(invalid("element-position", cx));
        };
        if hash != new_hash || key != new_key {
            return Err(invalid("element-value", cx));
        }
        names.insert(old, new);
    }
    for change in changes {
        cx.check(Kind::Work, 256)
            .map_err(|stop| stopped(cx, stop))?;
        let target = match change {
            Edit::Replace { target, .. }
            | Edit::Restyle { target, .. }
            | Edit::Transform { target, .. }
            | Edit::Copy { target, .. }
            | Edit::Remove { target } => target,
            _ => continue,
        };
        if let Some(current) = names.get(&target.id) {
            target.id = current.clone();
        }
    }
    Ok(())
}

fn settle_tables(
    request: &ApplyRequest,
    drawing: &Drawing2D,
    applied: &mut [Applied],
    cx: &mut Cx<'_>,
) -> Result<(), Failed> {
    let mut hashes = BTreeMap::new();
    let mut seen = BTreeMap::new();
    for layer in &drawing.layers {
        cx.check(Kind::Work, 256)
            .and_then(|()| cx.check(Kind::DecodedBytes, 512))
            .map_err(|stop| stopped(cx, stop))?;
        hashes.insert(
            table_id(id::layer(layer), &mut seen),
            Hash::layer_checked(layer, cx).map_err(|stop| stopped(cx, stop))?,
        );
    }
    seen.clear();
    for line in &drawing.line_types {
        cx.check(Kind::Work, 256)
            .and_then(|()| cx.check(Kind::DecodedBytes, 512))
            .map_err(|stop| stopped(cx, stop))?;
        hashes.insert(
            table_id(id::line_type(line), &mut seen),
            Hash::line_type_checked(line, cx).map_err(|stop| stopped(cx, stop))?,
        );
    }
    seen.clear();
    for (index, style) in drawing.text_styles.iter().enumerate() {
        cx.check(Kind::Work, 256)
            .and_then(|()| cx.check(Kind::DecodedBytes, 512))
            .map_err(|stop| stopped(cx, stop))?;
        hashes.insert(
            table_id(id::text_style(style, index), &mut seen),
            Hash::text_style_checked(style, cx).map_err(|stop| stopped(cx, stop))?,
        );
    }
    seen.clear();
    for (index, style) in drawing.dimension_styles.iter().enumerate() {
        cx.check(Kind::Work, 256)
            .and_then(|()| cx.check(Kind::DecodedBytes, 512))
            .map_err(|stop| stopped(cx, stop))?;
        hashes.insert(
            table_id(id::dimension_style(style, index), &mut seen),
            Hash::dimension_style_checked(style, cx).map_err(|stop| stopped(cx, stop))?,
        );
    }
    for (edit, result) in request.edits.iter().zip(applied) {
        if addition(edit).is_some() {
            result.hash = result.id.as_ref().and_then(|id| hashes.get(id)).copied();
            if result.hash.is_none() {
                result.id = None;
            }
        }
    }
    Ok(())
}
