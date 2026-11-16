//! J-LandXML Ver.1.8 4-3-6・9・10・22・23・37〜42。元の座標と高さの基準を保つ。
use super::*;

impl<'d> Reader<'d, '_, '_> {
    /// 子の列・文字・数値列・索引・モデルを確保する前に、対象の原本範囲から上限を数える。
    /// 文字列の索引の比較を含め、32 bit と 64 bit で同じ論理量を使う。
    pub(super) fn reserve_values(&mut self, element: &Element) -> Result<(), Error> {
        let bytes = element.span().len() as u64;
        self.charge(element, Kind::Work, bytes.saturating_mul(64))?;
        self.charge(
            element,
            Kind::DecodedBytes,
            bytes.saturating_mul(32).saturating_add(1024),
        )
    }

    pub(super) fn copy_attribute(
        &mut self,
        element: &Element,
        name: &[u8],
    ) -> Result<Option<String>, Error> {
        let doc = self.doc;
        let Some(attribute) = doc.attribute(element, None, name) else {
            return Ok(None);
        };
        let bytes = (attribute.raw().len() as u64).saturating_mul(4);
        self.charge(element, Kind::Work, bytes)?;
        self.charge(element, Kind::DecodedBytes, bytes)?;
        Ok(Some(doc.value(attribute)))
    }

    pub(super) fn reserve_text(&mut self, element: &Element, bytes: usize) -> Result<(), Error> {
        self.charge(element, Kind::Work, bytes as u64)?;
        self.charge(element, Kind::DecodedBytes, bytes as u64)
    }

    pub(super) fn height_reference(&mut self, element: &Element) -> HeightReference {
        let name = self.attribute(element, b"verticalDatum");
        let mut offset = None;
        let mut count = 0;
        let mut valid = true;
        for feature in self.children(element) {
            if self.local(feature) != b"Feature" {
                self.unsupported(feature);
                continue;
            }
            for property in self.children(feature) {
                if self.local(property) == b"Property"
                    && self.attribute(property, b"label").as_deref().map(str::trim)
                        == Some("differTP")
                {
                    count += 1;
                    offset = self.number(property, b"value");
                    if count != 1 || offset.is_none() {
                        self.invalid(property, "differTP");
                        valid = false;
                    }
                } else {
                    self.unsupported(property);
                }
            }
        }
        let is_tp = matches!(name.as_deref().map(str::trim), Some("T.P" | "T.P."));
        if is_tp && offset.is_some_and(|value| value != 0.0) {
            self.invalid(element, "differTP");
            valid = false;
        }
        HeightReference {
            name,
            tokyo_bay_offset: if valid {
                offset.or(is_tp.then_some(0.0))
            } else {
                None
            },
            source: self.at(element),
        }
    }

    fn survey_point(&mut self, element: &Element) -> Result<Option<SurveyPoint>, Error> {
        self.spend(element, 1)?;
        let values = numbers(&self.text(element));
        let (point, height) = match values.as_slice() {
            [northing, easting] => (
                MapPoint {
                    northing: *northing,
                    easting: *easting,
                },
                None,
            ),
            [northing, easting, height] => (
                MapPoint {
                    northing: *northing,
                    easting: *easting,
                },
                Some(*height),
            ),
            _ => {
                self.invalid(element, "text");
                return Ok(None);
            }
        };
        for child in self.children(element) {
            self.unsupported(child);
        }
        Ok(Some(SurveyPoint {
            name: self.attribute(element, b"name"),
            description: self.attribute(element, b"desc"),
            point,
            height,
            class: None,
            source: self.at(element),
        }))
    }

    pub(super) fn intersection_points(
        &mut self,
        element: &Element,
    ) -> Result<Vec<SurveyPoint>, Error> {
        self.reserve_values(element)?;
        let mut points = Vec::new();
        for group in self.children(element) {
            if self.local(group) != b"AlignPI" {
                self.unsupported(group);
                continue;
            }
            for point in self.children(group) {
                if self.local(point) == b"PI" {
                    if let Some(point) = self.survey_point(point)? {
                        points.push(point);
                    }
                } else {
                    self.unsupported(point);
                }
            }
        }
        Ok(points)
    }

    pub(super) fn point_set(&mut self, element: &Element) -> Result<SurveyPointSet, Error> {
        self.reserve_values(element)?;
        let mut classes = BTreeMap::new();
        for feature in self.named(element, b"Feature") {
            let mut class = None;
            let mut count = 0;
            for property in self.children(feature) {
                if self.local(property) == b"Property"
                    && self.attribute(property, b"label").as_deref().map(str::trim) == Some("class")
                {
                    count += 1;
                    class = self.attribute(property, b"value");
                    if count != 1 || class.is_none() {
                        self.invalid(property, "class");
                    }
                } else {
                    self.unsupported(property);
                }
            }
            if count != 1 {
                class = None;
            }
            if let Some(name) = self.attribute(feature, b"name") {
                match classes.entry(name) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(Some(class));
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        entry.insert(None);
                        self.invalid(feature, "name");
                    }
                }
            } else if count != 0 {
                self.invalid(feature, "name");
            }
        }
        let mut points = Vec::new();
        for child in self.children(element) {
            match self.local(child) {
                b"CgPoint" => {
                    if let Some(mut point) = self.survey_point(child)? {
                        if let Some(name) = self.attribute(child, b"featureRef") {
                            match classes.get(&name) {
                                Some(Some(class)) => {
                                    self.reserve_text(
                                        child,
                                        class.as_ref().map_or(0, String::len),
                                    )?;
                                    point.class.clone_from(class);
                                }
                                _ => self.invalid(child, "featureRef"),
                            }
                        }
                        points.push(point);
                    }
                }
                b"Feature" => {}
                _ => self.unsupported(child),
            }
        }
        Ok(SurveyPointSet {
            name: self.attribute(element, b"name"),
            description: self.attribute(element, b"desc"),
            points,
            source: self.at(element),
        })
    }

    pub(super) fn ground_profiles(
        &mut self,
        element: &Element,
        start: f64,
    ) -> Result<Vec<GroundProfile>, Error> {
        self.reserve_values(element)?;
        let mut profiles = Vec::new();
        for list in self.children(element) {
            if self.local(list) != b"PntList2D" {
                self.unsupported(list);
                continue;
            }
            let Some(mut points) = pairs(&numbers(&self.text(list))) else {
                self.invalid(list, "text");
                continue;
            };
            self.spend(list, points.len() as u64)?;
            for [distance, _] in &mut points {
                *distance -= start;
            }
            if points.iter().any(|[distance, _]| !distance.is_finite())
                || points
                    .windows(2)
                    .any(|pair| matches!(pair, [[a, _], [b, _]] if a > b))
            {
                self.invalid(list, "station");
                continue;
            }
            profiles.push(GroundProfile {
                name: self.copy_attribute(element, b"name")?,
                description: self.copy_attribute(element, b"desc")?,
                points,
                source: self.at(list),
            });
        }
        Ok(profiles)
    }

    pub(super) fn cross_profile(&mut self, element: &Element) -> Option<String> {
        let mut result = None;
        for feature in self.named(element, b"Feature") {
            for property in self.children(feature) {
                if self.local(property) == b"Property"
                    && self.attribute(property, b"label").as_deref().map(str::trim)
                        == Some("profAlignRefs")
                {
                    if result.is_some() {
                        self.invalid(property, "profAlignRefs");
                        // 空の参照は評価できない。重複を省略扱いにして既定の縦断へ結び付けない。
                        result = Some(String::new());
                    } else {
                        result = Some(self.attribute(property, b"value").unwrap_or_default());
                        if result.as_deref() == Some("") {
                            self.invalid(property, "profAlignRefs");
                        }
                    }
                } else {
                    self.unsupported(property);
                }
            }
        }
        result
    }

    pub(super) fn cross_height(&mut self, element: &Element) -> Option<CrossSectionHeight> {
        let mut result = CrossSectionHeight::Elevation;
        let mut count = 0;
        let mut valid = true;
        for feature in self.named(element, b"Feature") {
            for property in self.children(feature) {
                if self.local(property) != b"Property"
                    || self.attribute(property, b"label").as_deref().map(str::trim)
                        != Some("heightType")
                {
                    self.unsupported(property);
                    continue;
                }
                count += 1;
                match self.attribute(property, b"value").as_deref().map(str::trim) {
                    Some("elevation") => result = CrossSectionHeight::Elevation,
                    Some("fhOffset") => result = CrossSectionHeight::FormationOffset,
                    _ => {
                        self.unsupported(property);
                        valid = false;
                    }
                }
                if count != 1 {
                    self.invalid(property, "heightType");
                    valid = false;
                }
            }
        }
        valid.then_some(result)
    }
}
