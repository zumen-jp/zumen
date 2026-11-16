//! 4-3-24・図4-9/10。直線部へ戻らない隣の曲線を、明記された変化点でつなぐ。
use super::*;

impl Reader<'_, '_, '_> {
    pub(super) fn superelevation_link(
        &mut self,
        first: &(&Element, Vec<Cant>),
        next: &(&Element, Vec<Cant>),
        origin: f64,
        config: &Config,
    ) -> Result<Vec<Cant>, Error> {
        let maximum = |parts: &[Cant]| {
            parts
                .iter()
                .find(|p| p.transition.is_none())
                .map(|p| (p.start_distance, p.length, p.kind.clone()))
        };
        let (
            Some((
                a,
                length,
                CantKind::Superelevation {
                    end: [first_slope, _],
                    ..
                },
            )),
            Some((
                b,
                _,
                CantKind::Superelevation {
                    start: [next_slope, _],
                    ..
                },
            )),
        ) = (maximum(&first.1), maximum(&next.1))
        else {
            return Ok(Vec::new());
        };
        if !self.named(first.0, b"EndofRunoutSta").is_empty()
            || !self.named(next.0, b"BeginRunoutSta").is_empty()
        {
            return Ok(Vec::new());
        }
        let a = a + length;
        if !a.is_finite() || b <= a {
            if b != a || first_slope != next_slope {
                self.invalid(next.0, "range");
            }
            return Ok(Vec::new());
        }
        let mut points = vec![(a, [first_slope; 2]), (b, [next_slope; 2])];
        let mut reverse_count = 0;
        let mut flat_count = 0;
        let mut valid = true;
        for (element, slope, first_side) in
            [(first.0, first_slope, true), (next.0, next_slope, false)]
        {
            let mut count = 0;
            for feature in self.named(element, b"Feature") {
                let name = self.attribute(feature, b"name");
                let flat = name.as_deref() == Some("FlatSta");
                if name.as_deref() != Some("ReverseCrown") && !(flat && first_side) {
                    continue;
                }
                let mut has_station = false;
                for property in self.children(feature) {
                    self.charge(property, Kind::Work, property.span().len() as u64)?;
                    if self.local(property) != b"Property"
                        || self.attribute(property, b"label").as_deref().map(str::trim)
                            != Some("sta")
                    {
                        self.unsupported(property);
                        valid = false;
                        continue;
                    }
                    let Some(at) = self
                        .number(property, b"value")
                        .map(|n| n - origin)
                        .filter(|n| n.is_finite())
                    else {
                        self.invalid(property, "sta");
                        valid = false;
                        continue;
                    };
                    has_station = true;
                    if !(a < at && at < b) {
                        if flat {
                            self.invalid(property, "sta");
                            valid = false;
                        }
                        continue;
                    }
                    let value = if flat {
                        flat_count += 1;
                        0.0
                    } else {
                        count += 1;
                        reverse_count += 1;
                        let normal = config.normal.unwrap_or(0.0).abs();
                        if normal > slope.abs() {
                            self.invalid(property, "normalCrown");
                            valid = false;
                        }
                        if slope < 0.0 { -normal } else { normal }
                    };
                    if count > 1 || flat_count > 1 {
                        self.invalid(feature, "sta");
                        valid = false;
                        break;
                    }
                    points.push((at, [value; 2]));
                }
                if !has_station {
                    self.invalid(feature, "sta");
                    valid = false;
                }
            }
        }
        let opposite =
            (first_slope < 0.0 && next_slope > 0.0) || (first_slope > 0.0 && next_slope < 0.0);
        if opposite && reverse_count > 0 && flat_count != 1 {
            self.invalid(first.0, "FlatSta");
            valid = false;
        }
        if flat_count > 0 && !opposite {
            self.invalid(first.0, "FlatSta");
            valid = false;
        }
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        if points
            .windows(2)
            .any(|pair| matches!(pair, [a, b] if a.0 >= b.0))
        {
            self.invalid(first.0, "sta");
            valid = false;
        }
        if opposite && points.windows(2).any(|pair| matches!(pair, [a, b] if if first_slope < next_slope { a.1 > b.1 } else { a.1 < b.1 })) {
            self.invalid(first.0, "sta"); valid = false;
        }
        if valid {
            self.cant_intervals(first.0, &points)
        } else {
            Ok(Vec::new())
        }
    }

    pub(super) fn unlinked_phases(
        &mut self,
        part: &(&Element, Vec<Cant>),
        origin: f64,
        before: bool,
        after: bool,
    ) -> Result<(), Error> {
        let Some(maximum) = part.1.iter().find(|p| p.transition.is_none()) else {
            return Ok(());
        };
        let entry_missing = self.named(part.0, b"BeginRunoutSta").is_empty();
        let exit_missing = self.named(part.0, b"EndofRunoutSta").is_empty();
        for feature in self.named(part.0, b"Feature") {
            match self.attribute(feature, b"name").as_deref() {
                Some("FlatSta") if !after => self.unsupported(feature),
                Some("ReverseCrown") => {
                    for property in self.named(feature, b"Property") {
                        self.charge(property, Kind::Work, 1)?;
                        if let Some(at) = self.number(property, b"value").map(|n| n - origin)
                            && ((at < maximum.start_distance && entry_missing && !before)
                                || (at > maximum.start_distance + maximum.length
                                    && exit_missing
                                    && !after))
                        {
                            self.unsupported(property);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
