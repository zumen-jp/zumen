//! J-LandXML Ver.1.8 4-3-12・24。片勾配の設定と任意横断勾配の値。
use super::*;

mod link;
mod transition;

struct Config {
    single_lane: bool,
    normal: Option<f64>,
    valid: bool,
}

impl Reader<'_, '_, '_> {
    pub(super) fn cants(&mut self, element: &Element, origin: f64) -> Result<Vec<Cant>, Error> {
        // 子の列、復号した設定、勾配の変化点と区間を、確保の前に同じ論理量で数える。
        let count = element.children().len() as u64;
        self.charge(element, Kind::Work, count.saturating_mul(4))?;
        self.charge(element, Kind::DecodedBytes, count.saturating_mul(64))?;
        let doc = self.doc;
        for node in element.children() {
            let Node::Element(index) = node else { continue };
            let Some(child) = doc.element(*index) else {
                continue;
            };
            if matches!(self.local(child), b"Feature" | b"Superelevation") {
                let bytes = child.span().len() as u64;
                self.charge(child, Kind::Work, bytes)?;
                self.charge(
                    child,
                    Kind::DecodedBytes,
                    bytes.saturating_mul(32).saturating_add(1024),
                )?;
            }
        }
        let use_list = self.uses_slope_list(element)?;
        let config = self.cant_config(element)?;
        let mut out = Vec::new();
        if use_list {
            out = self.slope_list(element, origin, &config)?;
        }
        let mut parts = Vec::new();
        for child in self.named(element, b"Superelevation") {
            if use_list || !config.valid {
                self.unsupported(child);
            } else {
                parts.push((child, self.superelevation_parts(child, origin, &config)?));
            }
        }
        let mut linked_before = false;
        for index in 0..parts.len() {
            let link = match (parts.get(index), parts.get(index + 1)) {
                (Some(first), Some(next)) if config.normal.is_some() => {
                    self.superelevation_link(first, next, origin, &config)?
                }
                _ => Vec::new(),
            };
            if let Some(part) = parts.get(index)
                && config.normal.is_some()
            {
                self.unlinked_phases(part, origin, linked_before, !link.is_empty())?;
            }
            linked_before = !link.is_empty();
            if let Some((_, part)) = parts.get_mut(index) {
                out.append(part);
            }
            out.extend(link);
        }
        if !use_list
            && config.valid
            && parts.is_empty()
            && self.named(element, b"Superelevation").is_empty()
            && let Some(normal) = config.normal
        {
            if let Some(length) = self.number(element, b"length").filter(|n| *n > 0.0) {
                self.spend(element, 1)?;
                let slopes = if config.single_lane {
                    [normal; 2]
                } else {
                    [-normal, normal]
                };
                out.push(Cant {
                    start_distance: 0.0,
                    length,
                    kind: CantKind::Superelevation {
                        start: slopes,
                        end: slopes,
                    },
                    transition: None,
                    source: self.at(element),
                });
            } else {
                self.invalid(element, "length");
            }
        }
        Ok(out)
    }

    fn cant_config(&mut self, alignment: &Element) -> Result<Config, Error> {
        let mut config = Config {
            single_lane: false,
            normal: None,
            valid: true,
        };
        let mut seen = [false; 3];
        for feature in self.named(alignment, b"Feature") {
            if self.attribute(feature, b"name").as_deref() != Some("SuperelevationConfig") {
                continue;
            }
            for property in self.children(feature) {
                self.charge(property, Kind::Work, property.span().len() as u64)?;
                if self.local(property) != b"Property" {
                    self.unsupported(property);
                    continue;
                }
                let label = self.attribute(property, b"label");
                let slot = match label.as_deref().map(str::trim) {
                    Some("singleLaneRoad") => 0,
                    Some("normalCrown") => 1,
                    Some("useSlopeList") => 2,
                    _ => {
                        self.unsupported(property);
                        continue;
                    }
                };
                let Some(seen) = seen.get_mut(slot) else {
                    continue;
                };
                if *seen {
                    self.invalid(property, "label");
                    config.valid = false;
                    continue;
                }
                *seen = true;
                let value = self.attribute(property, b"value");
                let value = value.as_deref().map(str::trim);
                let valid = if slot == 1 {
                    config.normal = value
                        .and_then(|s| s.parse::<f64>().ok())
                        .filter(|n| n.is_finite());
                    config.normal.is_some()
                } else {
                    match value {
                        Some("true" | "1") => {
                            if slot == 0 {
                                config.single_lane = true;
                            }
                            true
                        }
                        Some("false" | "0") => true,
                        _ => false,
                    }
                };
                if !valid {
                    self.invalid(property, "value");
                    config.valid = false;
                }
            }
        }
        if !config.single_lane && config.normal.is_some_and(|n| n < 0.0) {
            self.invalid(alignment, "normalCrown");
            config.valid = false;
        }
        Ok(config)
    }

    fn slope_list(
        &mut self,
        alignment: &Element,
        origin: f64,
        config: &Config,
    ) -> Result<Vec<Cant>, Error> {
        let mut points = Vec::new();
        let mut valid = config.valid;
        let mut lists = Vec::new();
        for feature in self.named(alignment, b"Feature") {
            if self.attribute(feature, b"name").as_deref() != Some("SlopeList") {
                continue;
            }
            lists.push(feature);
            for property in self.children(feature) {
                self.charge(property, Kind::Work, property.span().len() as u64)?;
                if self.local(property) != b"Property"
                    || self.attribute(property, b"label").as_deref().map(str::trim)
                        != Some("slopeValue")
                {
                    self.unsupported(property);
                    valid = false;
                    continue;
                }
                let values = self
                    .attribute(property, b"value")
                    .map(|v| numbers(&v))
                    .unwrap_or_default();
                let point = match values.as_slice() {
                    [sta, slope] if config.single_lane => Some((*sta - origin, [*slope; 2])),
                    [sta, left, right] if !config.single_lane => {
                        Some((*sta - origin, [*left, *right]))
                    }
                    _ => None,
                };
                let Some((distance, slopes)) = point.filter(|(d, _)| d.is_finite()) else {
                    self.invalid(property, "slopeValue");
                    valid = false;
                    continue;
                };
                if points
                    .last()
                    .is_some_and(|p: &(f64, [f64; 2], Ref)| p.0 >= distance)
                {
                    self.invalid(property, "sta");
                    valid = false;
                }
                points.push((distance, slopes, self.at(property)));
            }
        }
        if points.len() < 2 || !valid {
            if lists.is_empty() {
                for feature in self.named(alignment, b"Feature") {
                    if self.attribute(feature, b"name").as_deref() == Some("SuperelevationConfig") {
                        self.unsupported(feature);
                    }
                }
            }
            for feature in lists {
                self.unsupported(feature);
            }
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for pair in points.windows(2) {
            let [first, last] = pair else { continue };
            let length = last.0 - first.0;
            if !length.is_finite() {
                self.invalid(alignment, "slopeValue");
                return Ok(Vec::new());
            }
            self.spend(alignment, 1)?;
            out.push(Cant {
                start_distance: first.0,
                length,
                kind: CantKind::Superelevation {
                    start: first.1,
                    end: last.1,
                },
                transition: Some("linear".into()),
                source: first.2.clone(),
            });
        }
        Ok(out)
    }
}
