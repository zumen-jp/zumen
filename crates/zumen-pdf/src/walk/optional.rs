//! 単一のレイヤで保持できる表示条件と、画像の警告の保留。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Diagnostics, Kind, Stop, codes};

pub(super) type Key = (u32, i32);

/// 表示条件の積を、単一 OCG で表せるか。見えるかそのものは解釈器と同じ方法で別に決める。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Context {
    #[default]
    None,
    Single(Key),
    Unknown,
}

impl Context {
    pub(super) fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::None, value) | (value, Self::None) => value,
            (Self::Single(left), Self::Single(right)) if left == right => Self::Single(left),
            _ => Self::Unknown,
        }
    }
}

/// 画像のレイヤを保持できた場合にだけ省く警告。ほかの警告も、同じ code の場所の順を保つために記録する。
#[derive(Clone, Debug)]
pub(super) struct Event {
    image: Option<Key>,
    diagnostic: Diagnostic,
}

/// 確保を伸ばす間の新旧の列、場所、診断の写しを含む固定の論理量。
const EVENT_BYTES: u64 = 1024;
const ENTRY_BYTES: u64 = 1024;
const STACK_BYTES: u64 = 128;

#[derive(Default)]
pub(super) struct Content {
    stack: Vec<Context>,
    images: BTreeMap<Key, Context>,
    events: Vec<Event>,
    recording: Vec<Vec<Event>>,
    untrusted: bool,
}

impl Content {
    pub(super) fn clear(&mut self) {
        self.stack.clear();
        self.images.clear();
        self.events.clear();
        self.recording.clear();
        self.untrusted = false;
    }

    pub(super) fn current(&self) -> Context {
        self.stack.last().copied().unwrap_or_default()
    }

    pub(super) fn push(&mut self, own: Context, cx: &mut Cx<'_>) -> Result<(), Stop> {
        cx.check(Kind::Work, 1)?;
        cx.check(Kind::DecodedBytes, STACK_BYTES)?;
        self.stack.push(self.current().and(own));
        Ok(())
    }

    pub(super) fn pop(&mut self) {
        self.stack.pop();
    }

    /// 解釈器と装置の対応を確かめられない内容では、このページの新しいレイヤ指定を行わない。
    pub(super) fn distrust(&mut self) {
        self.untrusted = true;
    }

    pub(super) fn observe(
        &mut self,
        image: Key,
        context: Context,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        cx.check(Kind::Work, 1)?;
        if let Some(existing) = self.images.get_mut(&image) {
            if *existing != context {
                *existing = Context::Unknown;
            }
        } else {
            cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
            self.images.insert(image, context);
        }
        Ok(())
    }

    pub(super) fn record(
        &mut self,
        image: Option<Key>,
        diagnostic: &Diagnostic,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        let copies = 1 + u64::from(!self.recording.is_empty());
        charge(diagnostic, copies, cx)?;
        self.events.push(Event {
            image,
            diagnostic: diagnostic.clone(),
        });
        if let Some(recording) = self.recording.last_mut() {
            recording.push(Event {
                image,
                diagnostic: diagnostic.clone(),
            });
        }
        Ok(())
    }

    pub(super) fn replay(&mut self, events: &[Event], cx: &mut Cx<'_>) -> Result<(), Stop> {
        for event in events {
            self.record(event.image, &event.diagnostic, cx)?;
        }
        Ok(())
    }

    pub(super) fn begin_recording(&mut self, cx: &mut Cx<'_>) -> Result<(), Stop> {
        cx.check(Kind::Work, 1)?;
        cx.check(Kind::DecodedBytes, STACK_BYTES)?;
        self.recording.push(Vec::new());
        Ok(())
    }

    pub(super) fn end_recording(&mut self, cx: &mut Cx<'_>) -> Result<Vec<Event>, Stop> {
        let events = self.recording.pop().unwrap_or_default();
        if let Some(recording) = self.recording.last_mut() {
            for event in &events {
                charge(&event.diagnostic, 1, cx)?;
                recording.push(event.clone());
            }
        }
        Ok(events)
    }

    pub(super) fn abort_recording(&mut self) {
        self.recording.pop();
    }

    /// 呼び出しを覚える表の追加キーと、Vec から Rc の列を作る際の写しを先に数える。
    pub(super) fn cache(events: &[Event], cx: &mut Cx<'_>) -> Result<(), Stop> {
        cx.check(Kind::Work, 1)?;
        cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
        for event in events {
            charge(&event.diagnostic, 1, cx)?;
        }
        Ok(())
    }

    pub(super) fn layers(&self, cx: &mut Cx<'_>) -> Result<BTreeMap<Key, Key>, Stop> {
        let mut layers = BTreeMap::new();
        if self.untrusted {
            return Ok(layers);
        }
        for (&image, context) in &self.images {
            cx.check(Kind::Work, 1)?;
            if let Context::Single(group) = context {
                cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
                layers.insert(image, *group);
            }
        }
        Ok(layers)
    }

    /// ほかの code と、保持できない警告の件数・場所の先頭を維持し、保持した画像の警告だけを除く。
    pub(super) fn diagnostics(
        &self,
        found: &mut Diagnostics,
        layers: &BTreeMap<Key, Key>,
        cx: &mut Cx<'_>,
    ) -> Result<Vec<Diagnostic>, Stop> {
        if layers.is_empty() {
            return Ok(std::mem::take(found).into_vec());
        }
        let mut removed = false;
        for event in &self.events {
            cx.check(Kind::Work, 1)?;
            if event.image.is_some_and(|image| layers.contains_key(&image)) {
                removed = true;
                break;
            }
        }
        if !removed {
            return Ok(std::mem::take(found).into_vec());
        }
        let mut remaining = Diagnostics::with_max_places(found.max_places());
        for event in &self.events {
            cx.check(Kind::Work, 1)?;
            if !event.image.is_some_and(|image| layers.contains_key(&image)) {
                charge(&event.diagnostic, 1, cx)?;
                remaining.report(event.diagnostic.clone());
            }
        }
        let replacement = remaining.into_vec().pop();
        // この先は停止しない。失敗時は、呼ぶ側が未加工の診断を返せるよう、ここまで found を動かさない。
        let mut diagnostics = std::mem::take(found).into_vec();
        if let Some(index) = diagnostics
            .iter()
            .position(|diagnostic| diagnostic.code() == codes::PDF_OPTIONAL_CONTENT_APPROXIMATED)
        {
            if let Some(replacement) = replacement {
                if let Some(slot) = diagnostics.get_mut(index) {
                    *slot = replacement;
                }
            } else {
                diagnostics.remove(index);
            }
        }
        Ok(diagnostics)
    }
}

fn charge(diagnostic: &Diagnostic, copies: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
    cx.check(
        Kind::Work,
        copies.saturating_mul(1 + diagnostic.places().len() as u64),
    )?;
    let ids = diagnostic
        .places()
        .iter()
        .map(|place| place.id().map_or(0, |id| id.len() as u64))
        .fold(0_u64, u64::saturating_add);
    cx.check(
        Kind::DecodedBytes,
        copies.saturating_mul(EVENT_BYTES.saturating_add(ids)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Limit, Limits, Ref};

    fn warning(record: u64) -> Diagnostic {
        Diagnostic::new(codes::PDF_OPTIONAL_CONTENT_APPROXIMATED)
            .at(Ref::default().with_record(record))
    }

    #[test]
    fn combines_single_constraints_and_never_recovers_mixed_image_uses() {
        let first = Context::Single((3, 0));
        let second = Context::Single((4, 0));
        assert_eq!(Context::None.and(first), first);
        assert_eq!(first.and(Context::None), first);
        assert_eq!(first.and(first), first);
        assert_eq!(first.and(second), Context::Unknown);
        assert_eq!(Context::Unknown.and(first), Context::Unknown);
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut content = Content::default();
        content.push(first, &mut cx).unwrap();
        content.push(first, &mut cx).unwrap();
        assert_eq!(content.current(), first);
        content.push(second, &mut cx).unwrap();
        assert_eq!(content.current(), Context::Unknown);
        content.pop();
        assert_eq!(content.current(), first);
        content.observe((8, 0), first, &mut cx).unwrap();
        content.observe((8, 0), first, &mut cx).unwrap();
        assert_eq!(content.layers(&mut cx).unwrap().get(&(8, 0)), Some(&(3, 0)));
        content.observe((8, 0), second, &mut cx).unwrap();
        content.observe((8, 0), first, &mut cx).unwrap();
        assert!(content.layers(&mut cx).unwrap().is_empty());
    }

    #[test]
    fn cached_events_preserve_warning_counts_places_and_code_order() {
        for max_places in [0, 1, 16] {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let mut content = Content::default();
            let mut found = Diagnostics::with_max_places(max_places);
            let mut cached = Diagnostics::new();
            content.begin_recording(&mut cx).unwrap();
            for (origin, record) in [(Some((8, 0)), 8), (None, 5), (Some((9, 0)), 9)] {
                let diagnostic = warning(record);
                content.record(origin, &diagnostic, &mut cx).unwrap();
                found.report(diagnostic.clone());
                cached.report(diagnostic);
            }
            let events = content.end_recording(&mut cx).unwrap();
            found.report(Diagnostic::new(codes::PDF_CONTENT_INVALID));
            for _ in 0..2 {
                content.replay(&events, &mut cx).unwrap();
                for diagnostic in cached.iter() {
                    found.report(diagnostic.clone());
                }
            }
            content
                .observe((8, 0), Context::Single((3, 0)), &mut cx)
                .unwrap();
            content.observe((9, 0), Context::Unknown, &mut cx).unwrap();
            let layers = content.layers(&mut cx).unwrap();
            let found = content.diagnostics(&mut found, &layers, &mut cx).unwrap();
            assert_eq!(found.len(), 2);
            assert_eq!(found[0].code(), codes::PDF_OPTIONAL_CONTENT_APPROXIMATED);
            assert_eq!(found[0].count(), 6);
            let expected = [5, 9, 5, 9, 5, 9];
            let places: Vec<_> = found[0].places().iter().map(Ref::record).collect();
            assert_eq!(
                places,
                expected
                    .into_iter()
                    .take(max_places)
                    .map(Some)
                    .collect::<Vec<_>>()
            );
            assert_eq!(found[1].code(), codes::PDF_CONTENT_INVALID);
        }
    }

    #[test]
    fn fallback_keeps_original_diagnostics_and_allocation_stops_before_recording() {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut content = Content::default();
        let mut found = Diagnostics::with_max_places(1);
        for record in [7, 3, 9] {
            content
                .record(Some((8, 0)), &warning(record), &mut cx)
                .unwrap();
            found.report(warning(record));
        }
        let expected = found.clone().into_vec();
        content
            .observe((8, 0), Context::Single((3, 0)), &mut cx)
            .unwrap();
        content.distrust();
        let layers = content.layers(&mut cx).unwrap();
        assert!(layers.is_empty());
        assert_eq!(
            content.diagnostics(&mut found, &layers, &mut cx).unwrap(),
            expected
        );

        let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(0)));
        let mut diagnostics = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut empty = Content::default();
        assert!(empty.record(Some((8, 0)), &warning(8), &mut cx).is_err());
        assert!(empty.events.is_empty());
        assert!(
            empty
                .observe((8, 0), Context::Single((3, 0)), &mut cx)
                .is_err()
        );
        assert!(empty.images.is_empty());
    }

    #[test]
    fn postprocessing_stops_keep_the_original_diagnostics_before_the_stop_code() {
        let fixture = || {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let mut content = Content::default();
            let mut found = Diagnostics::with_max_places(16);
            found.report(Diagnostic::new(codes::PDF_CONTENT_INVALID).at(Ref::at_record(1)));
            for (origin, record) in [(Some((8, 0)), 8), (None, 3), (Some((9, 0)), 9)] {
                let diagnostic = warning(record);
                content.record(origin, &diagnostic, &mut cx).unwrap();
                found.report(diagnostic);
            }
            found.report(Diagnostic::new(codes::PDF_GEOMETRY_DROPPED).at(Ref::at_record(10)));
            content
                .observe((8, 0), Context::Single((3, 0)), &mut cx)
                .unwrap();
            content.observe((9, 0), Context::Unknown, &mut cx).unwrap();
            (content, found)
        };
        let (content, mut found) = fixture();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let layers = content.layers(&mut cx).unwrap();
        let layers_work = cx.budget.used(Kind::Work);
        let layers_bytes = cx.budget.used(Kind::DecodedBytes);
        let expected_success = content.diagnostics(&mut found, &layers, &mut cx).unwrap();
        let full_work = cx.budget.used(Kind::Work);
        let full_bytes = cx.budget.used(Kind::DecodedBytes);
        for (kind, layer_limit, full_limit) in [
            (Kind::Work, layers_work, full_work),
            (Kind::DecodedBytes, layers_bytes, full_bytes),
        ] {
            let mut stopped_in_layers = false;
            let mut stopped_in_diagnostics = false;
            for limit in [0, layer_limit - 1, layer_limit, full_limit - 1, full_limit] {
                let (content, mut found) = fixture();
                let expected_failure = found.clone().into_vec();
                let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                let mut diagnostics = Diagnostics::new();
                let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
                let mut passed_layers = false;
                let result = content.layers(&mut cx).and_then(|layers| {
                    passed_layers = true;
                    content.diagnostics(&mut found, &layers, &mut cx)
                });
                match result {
                    Ok(result) => {
                        assert_eq!(limit, full_limit);
                        assert_eq!(result, expected_success);
                        assert!(found.is_empty());
                    }
                    Err(stop) => {
                        stopped_in_layers |= !passed_layers;
                        stopped_in_diagnostics |= passed_layers;
                        assert!(cx.diag.is_empty(), "Cx::check は停止の診断を先に返さない");
                        // Walker は未加工の found を先に返し、その後で Lifter が停止の診断を添える。
                        for diagnostic in found.into_vec() {
                            cx.report(diagnostic);
                        }
                        cx.report(stop.to_diagnostic().at(Ref::at_record(5)));
                        let returned: Vec<_> = cx.diag.iter().cloned().collect();
                        assert_eq!(
                            &returned[..expected_failure.len()],
                            expected_failure.as_slice()
                        );
                        assert_eq!(returned.len(), expected_failure.len() + 1);
                        assert_eq!(returned.last().unwrap().code(), stop.to_diagnostic().code());
                        assert_eq!(returned.last().unwrap().places(), &[Ref::at_record(5)]);
                    }
                }
            }
            assert!(stopped_in_layers);
            assert!(stopped_in_diagnostics);
        }
    }
}
