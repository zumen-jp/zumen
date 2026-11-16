//! 4-3-24〜32・図4-8。明記された始終点と直線部勾配から、左右のすりつけを区間へ移す。
use super::*;

impl Reader<'_, '_, '_> {
    pub(super) fn superelevation_parts(
        &mut self,
        element: &Element,
        origin: f64,
        config: &Config,
    ) -> Result<Vec<Cant>, Error> {
        let Some(maximum) = self.superelevation(element, origin, config.normal.is_some())? else {
            return Ok(Vec::new());
        };
        let Some(_) = config.normal else {
            return Ok(vec![maximum]);
        };
        let CantKind::Superelevation {
            start: [full, _], ..
        } = maximum.kind
        else {
            return Ok(vec![maximum]);
        };
        let begin = maximum.start_distance;
        let end = begin + maximum.length;
        let mut stations = [None; 4];
        let mut reverse = Vec::new();
        let mut adverse = None;
        let mut valid = true;
        let mut phase_elements = Vec::new();
        for child in self.children(element) {
            self.charge(child, Kind::Work, 1)?;
            let slot = match self.local(child) {
                b"BeginRunoutSta" => Some(0),
                b"BeginRunoffSta" => Some(1),
                b"StartofRunoutSta" => Some(2),
                b"EndofRunoutSta" => Some(3),
                b"AdverseSE" => {
                    phase_elements.push(child);
                    let text = self.text(child);
                    let value = match text.trim() {
                        "adverse" => Some(true),
                        "non-adverse" => Some(false),
                        _ => None,
                    };
                    if adverse.is_some() || value.is_none() {
                        self.invalid(child, "text");
                        valid = false;
                    }
                    adverse = value;
                    None
                }
                b"Feature" => {
                    if self.attribute(child, b"name").as_deref() == Some("ReverseCrown") {
                        phase_elements.push(child);
                        let count_before = reverse.len();
                        for property in self.children(child) {
                            self.charge(property, Kind::Work, property.span().len() as u64)?;
                            if self.local(property) != b"Property"
                                || self.attribute(property, b"label").as_deref().map(str::trim)
                                    != Some("sta")
                            {
                                self.unsupported(property);
                                valid = false;
                                continue;
                            }
                            match self
                                .number(property, b"value")
                                .map(|v| v - origin)
                                .filter(|v| v.is_finite())
                            {
                                Some(v) if reverse.len() < 2 => reverse.push(v),
                                _ => {
                                    self.invalid(property, "sta");
                                    valid = false;
                                }
                            }
                        }
                        if reverse.len() == count_before {
                            self.invalid(child, "sta");
                            valid = false;
                        }
                    } else if self.attribute(child, b"name").as_deref() != Some("FlatSta") {
                        self.unsupported(child);
                    }
                    None
                }
                _ => None,
            };
            if let Some(slot) = slot.and_then(|i| stations.get_mut(i)) {
                phase_elements.push(child);
                let value = self
                    .scalar_text(child)?
                    .map(|v| v - origin)
                    .filter(|v| v.is_finite());
                if slot.is_some() || value.is_none() {
                    self.invalid(child, "text");
                    valid = false;
                }
                *slot = Some(value);
            }
        }
        let [runout, runoff, start_runout, end_runout] = stations.map(Option::flatten);
        let before: Vec<_> = reverse.iter().copied().filter(|v| *v < begin).collect();
        let after: Vec<_> = reverse.iter().copied().filter(|v| *v > end).collect();
        if before.len() > 1 || after.len() > 1 || before.len() + after.len() != reverse.len() {
            self.invalid(element, "ReverseCrown");
            valid = false;
        }
        if !valid {
            return Ok(vec![maximum]);
        }
        let adverse = adverse.unwrap_or(true);
        let entry = side(
            runout,
            runoff,
            runout.and(before.first().copied()),
            begin,
            full,
            config,
            adverse,
        );
        let exit = side(
            end_runout,
            start_runout,
            end_runout.and(after.first().copied()),
            end,
            full,
            config,
            adverse,
        );
        let (Some(mut entry), Some(mut exit)) = (entry, exit) else {
            for child in phase_elements {
                self.unsupported(child);
            }
            return Ok(vec![maximum]);
        };
        entry.sort_by(|a, b| a.0.total_cmp(&b.0));
        exit.sort_by(|a, b| a.0.total_cmp(&b.0));
        if entry.first().is_some_and(|p| p.0 > begin)
            || entry.last().is_some_and(|p| p.0 != begin)
            || exit.first().is_some_and(|p| p.0 != end)
            || exit.last().is_some_and(|p| p.0 < end)
        {
            self.invalid(element, "range");
            return Ok(vec![maximum]);
        }
        let mut out = self.cant_intervals(element, &entry)?;
        out.push(maximum);
        out.extend(self.cant_intervals(element, &exit)?);
        Ok(out)
    }

    pub(super) fn cant_intervals(
        &mut self,
        element: &Element,
        points: &[(f64, [f64; 2])],
    ) -> Result<Vec<Cant>, Error> {
        let mut out = Vec::new();
        for pair in points.windows(2) {
            let [a, b] = pair else { continue };
            let length = b.0 - a.0;
            if !length.is_finite() || length <= 0.0 {
                self.invalid(element, "range");
                return Ok(Vec::new());
            }
            self.spend(element, 1)?;
            out.push(Cant {
                start_distance: a.0,
                length,
                kind: CantKind::Superelevation {
                    start: a.1,
                    end: b.1,
                },
                transition: Some("linear".into()),
                source: self.at(element),
            });
        }
        Ok(out)
    }
}

/// 直線部から最大区間へ向かう変化点。終わり側も時刻を逆向きに渡せる。
fn side(
    runout: Option<f64>,
    runoff: Option<f64>,
    reverse: Option<f64>,
    full_at: f64,
    full: f64,
    config: &Config,
    adverse: bool,
) -> Option<Vec<(f64, [f64; 2])>> {
    let normal = config.normal?;
    let single = config.single_lane;
    let Some(runout) = runout else {
        return (runoff.is_none() && reverse.is_none()).then(Vec::new);
    };
    let span = full_at - runout;
    if !span.is_finite() || span == 0.0 {
        return None;
    }
    let between = |t: f64, a: f64, b: f64| {
        if a < b {
            a < t && t < b
        } else {
            b < t && t < a
        }
    };
    let normal_pair = if single {
        [normal; 2]
    } else {
        [-normal, normal]
    };
    let mut points = vec![(runout, normal_pair)];
    if !adverse || normal == 0.0 {
        if runoff.is_some() || reverse.is_some() {
            return None;
        }
    } else if single {
        if reverse.is_some() {
            return None;
        }
        if let Some(runoff) = runoff {
            if !between(runoff, runout, full_at) {
                return None;
            }
            points.push((runoff, [0.0; 2]));
        }
    } else {
        let runoff = runoff?;
        if !between(runoff, runout, full_at) || full.abs() < normal {
            return None;
        }
        let sign = if full < 0.0 { -1.0 } else { 1.0 };
        let flat = if full < 0.0 {
            [-normal, 0.0]
        } else {
            [0.0, normal]
        };
        points.push((runoff, flat));
        let reverse = reverse.unwrap_or(runoff + (full_at - runoff) * (normal / full.abs()));
        if !reverse.is_finite() || (!between(reverse, runoff, full_at) && reverse != full_at) {
            return None;
        }
        if reverse != full_at {
            points.push((reverse, [sign * normal; 2]));
        }
    }
    points.push((full_at, [full; 2]));
    Some(points)
}
