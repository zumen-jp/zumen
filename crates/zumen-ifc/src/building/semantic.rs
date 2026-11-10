//! 階・区域・分類。仕様の実体の属性の節の事実だけを使う。規則は docs/formats/ifc.md にある。

use super::*;
use zumen_building::{Classification, ClassificationReference, Level};

fn classification_size(value: &ClassificationReference) -> u64 {
    ITEM + [&value.system, &value.edition, &value.code, &value.name]
        .iter()
        .map(|text| optional_text_size(text.as_deref()))
        .sum::<u64>()
}

impl Lifter<'_> {
    /// IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcBuildingStorey.Elevation。
    /// 親の関係を読んだあとで、名前と高さを写す。語彙は #423 で足すため、種類を推定しない。
    pub(super) fn levels(&mut self, cx: &mut Cx) {
        for element in 0..self.building.elements().len() {
            if !self.charge(cx, Kind::Work, 1) {
                return;
            }
            let element = ElementRef(element as u32);
            if !self
                .building
                .components
                .spatial
                .get(element)
                .is_some_and(|s| s.kind == SpatialKind::Storey)
            {
                continue;
            }
            let Some(record) = self
                .building
                .element(element)
                .and_then(|e| e.source().instance)
                .and_then(|id| self.file.instance(id))
            else {
                continue;
            };
            let Some(height) = self
                .building
                .components
                .spatial
                .get(element)
                .and_then(|s| s.elevation.as_ref())
                .filter(|h| h.value.is_finite())
            else {
                self.invalid(record, 10, cx);
                continue;
            };
            let name = self
                .building
                .components
                .identity
                .get(element)
                .and_then(|i| i.name.as_deref());
            if !self.copied(cx, ITEM + measured_size(height) + optional_text_size(name)) {
                return;
            }
            let mut level = Level {
                name: name.map(str::to_owned),
                kind: None,
                height: height.clone(),
                datum: None,
            };
            // datum は高さの基準を持つ建物の ID。値と単位はその spatial.elevation で引く。
            let mut next = Some(element);
            let mut seen = BTreeSet::new();
            let mut depth = 0;
            while let Some(parent) = next {
                if !self.charge(cx, Kind::Work, 1) || !self.charge(cx, Kind::Depth, 1) {
                    break;
                }
                depth += 1;
                if !seen.insert(parent) {
                    self.invalid(record, 10, cx);
                    break;
                }
                if let Some(spatial) = self.building.components.spatial.get(parent)
                    && spatial.kind == SpatialKind::Building
                {
                    if spatial
                        .elevation
                        .as_ref()
                        .is_some_and(|h| h.value.is_finite())
                    {
                        let id = self.id_text(parent);
                        if self.copied(cx, text_size(&id)) {
                            level.datum = Some(id);
                        }
                    }
                    break;
                }
                next = self
                    .building
                    .components
                    .parent
                    .get(parent)
                    .and_then(|p| p.of.as_ref())
                    .map(|p| p.target);
            }
            for _ in 0..depth {
                cx.leave();
            }
            if self.is_halted() {
                return;
            }
            self.building.components.level.insert(element, level);
        }
    }

    /// IfcZone の IsGroupedBy（IfcRelAssignsToGroup）と、IfcSpatialZone の ReferencesElements
    /// （IfcRelReferencedInSpatialStructure）。IFC2X3 の IfcZone.WR1、IFC4/4.3 の 5.4 の両実体。
    /// 区域の種類は語彙を待ち、室と区域の参照を直接持つ。境界の形は #419 で扱う。
    pub(super) fn zone_members(
        &mut self,
        id: u64,
        record: &Record,
        target: usize,
        group: bool,
        cx: &mut Cx,
    ) {
        let Some(zone) =
            reference(arg(record, target)).and_then(|n| self.elements.get(&n).copied())
        else {
            return;
        };
        let expected = if group { "IfcZone" } else { "IfcSpatialZone" };
        if !self
            .building
            .components
            .class
            .get(zone)
            .is_some_and(|c| c.name == expected)
        {
            return;
        }
        let (related, mut incomplete) = self.semantic_related(record, cx);
        for (position, member) in related {
            if !self.charge(cx, Kind::Work, 1) {
                return;
            }
            let allowed = self.building.components.class.get(member).is_some_and(|c| {
                matches!(c.name.as_str(), "IfcSpace" | "IfcZone" | "IfcSpatialZone")
            });
            if !allowed {
                // 空間の参照は物理の要素も持てる。それらは既存の parent.referenced_in に残る。
                if group {
                    self.invalid(record, 5, cx);
                    incomplete = true;
                }
                continue;
            }
            if !self.copied(cx, ITEM) {
                return;
            }
            let mut value = self
                .building
                .components
                .zone
                .remove(zone)
                .unwrap_or_default();
            value.members.push(member);
            self.building.components.zone.insert(zone, value);
            self.source(zone, id, record, Some(position), cx);
        }
        if incomplete
            && self
                .building
                .components
                .zone
                .get(zone)
                .is_some_and(|zone| zone.members.is_empty())
        {
            self.building.components.zone.remove(zone);
        }
    }

    /// IfcRelAssociatesClassification: 5 RelatedObjects・6 RelatingClassification。
    /// IFC2X3 は IfcClassificationReference、IFC4/4.3 は体系そのものも参照できる。
    pub(super) fn classification(&mut self, id: u64, record: &Record, cx: &mut Cx) -> bool {
        let Some(target) = reference(arg(record, 6)) else {
            self.invalid(record, 6, cx);
            return true;
        };
        let Some(value) = self.classification_reference(target, record, cx) else {
            return true;
        };
        let (related, _) = self.semantic_related(record, cx);
        for (position, element) in related {
            if !self.charge(cx, Kind::Work, 1) || !self.copied(cx, classification_size(&value)) {
                return true;
            }
            let mut classification: Classification = self
                .building
                .components
                .classification
                .remove(element)
                .unwrap_or_default();
            classification.references.push(value.clone());
            self.building
                .components
                .classification
                .insert(element, classification);
            self.source(element, id, record, Some(position), cx);
        }
        true
    }

    /// 集合を元の位置のまま検査する。参照でない値と存在しない参照を診断し、有効だが要素でない対象は最後に確かめる。
    fn semantic_related(&mut self, record: &Record, cx: &mut Cx) -> (Vec<(u32, ElementRef)>, bool) {
        let Some(Value::List(values)) = arg(record, 5) else {
            self.invalid(record, 5, cx);
            return (Vec::new(), true);
        };
        let mut incomplete = values.is_empty();
        if incomplete {
            self.invalid(record, 5, cx);
        }
        for (position, value) in values.iter().enumerate() {
            if !self.charge(cx, Kind::Work, 1) {
                return (Vec::new(), incomplete);
            }
            let Some(target) =
                reference(Some(value)).filter(|&id| self.file.instance(id).is_some())
            else {
                self.invalid(record, 5, cx);
                incomplete = true;
                continue;
            };
            if !self.elements.contains_key(&target) {
                incomplete = true;
                let Ok(position) = u32::try_from(position) else {
                    self.invalid(record, 5, cx);
                    continue;
                };
                if !self.copied(cx, ITEM) {
                    return (Vec::new(), incomplete);
                }
                self.unmapped_relations
                    .push((record_id(record), target, position));
            }
        }
        (self.related(record, 5), incomplete)
    }

    /// 実体自体を落とす場合は既存の entity.dropped だけにする。別成分で使った対象への関係の欠落だけを診断する。
    pub(super) fn report_unmapped_relations(&mut self, cx: &mut Cx) -> Result<(), Error> {
        for (relation, target, position) in core::mem::take(&mut self.unmapped_relations) {
            if !self.charge(cx, Kind::Work, 1) {
                return self.halted(cx, self.file.instance(relation).map(Record::span));
            }
            if !self.used.contains(&target) {
                continue;
            }
            let (Some(relation), Some(target)) =
                (self.file.instance(relation), self.file.instance(target))
            else {
                continue;
            };
            cx.report(
                Diagnostic::new(codes::IFC_RELATION_TARGET_UNSUPPORTED)
                    .with(
                        "kind",
                        String::from_utf8_lossy(self.file.kind(relation)).into_owned(),
                    )
                    .with(
                        "target_kind",
                        String::from_utf8_lossy(self.file.kind(target)).into_owned(),
                    )
                    .with("position", u64::from(position))
                    .at(at_bytes(relation.span())),
            );
            if !self.charge(cx, Kind::Work, 0) {
                return self.halted(cx, Some(relation.span()));
            }
        }
        Ok(())
    }

    /// IfcClassificationReference の 2 ItemReference（IFC4 から Identification）・3 Name・4 ReferencedSource。
    /// IFC4/4.3 の 8.6.3.2、IFC2X3 の IfcExternalReferenceResource の両実体の属性の節。
    /// IfcClassification は全版で 2 Edition・4 Name。IFC4 から参照の階層をたどれる。
    fn classification_reference(
        &mut self,
        target: u64,
        relation: &Record,
        cx: &mut Cx,
    ) -> Option<ClassificationReference> {
        let mut value = ClassificationReference {
            system: None,
            edition: None,
            code: None,
            name: None,
        };
        let mut next = Some(target);
        let mut seen = BTreeSet::new();
        let mut depth = 0;
        let mut valid = true;
        while let Some(id) = next {
            if !self.charge(cx, Kind::Work, 1) || !self.charge(cx, Kind::Depth, 1) {
                valid = false;
                break;
            }
            depth += 1;
            if !seen.insert(id) {
                self.invalid(relation, 6, cx);
                valid = false;
                break;
            }
            let Some(record) = self.file.instance(id) else {
                self.invalid(relation, 6, cx);
                valid = false;
                break;
            };
            if self.is(id, b"IFCCLASSIFICATION")
                && (id != target || self.version != Version::Ifc2x3)
            {
                if !self.optional_texts_valid(record, &[2], cx) {
                    valid = false;
                    break;
                }
                if !matches!(arg(record, 4), Some(Value::Text { .. })) {
                    self.invalid(record, 4, cx);
                    valid = false;
                    break;
                }
                if !self.copied(
                    cx,
                    ITEM * 2 + raw_text_len(arg(record, 2)) + raw_text_len(arg(record, 4)),
                ) {
                    valid = false;
                    break;
                }
                value.system = self.text(arg(record, 4), record, cx);
                value.edition = self.text(arg(record, 2), record, cx);
                if value.system.is_none() {
                    self.invalid(record, 4, cx);
                    valid = false;
                }
                self.used.insert(id);
                break;
            }
            if !self.is(id, b"IFCCLASSIFICATIONREFERENCE")
                || (id != target && self.version == Version::Ifc2x3)
            {
                self.invalid(relation, 6, cx);
                valid = false;
                break;
            }
            self.used.insert(id);
            if id == target {
                if !self.optional_texts_valid(record, &[2, 3], cx) {
                    valid = false;
                    break;
                }
                if !self.copied(
                    cx,
                    ITEM * 4 + raw_text_len(arg(record, 2)) + raw_text_len(arg(record, 3)),
                ) {
                    valid = false;
                    break;
                }
                value.code = self.text(arg(record, 2), record, cx);
                value.name = self.text(arg(record, 3), record, cx);
            }
            next = match arg(record, 4) {
                Some(Value::Null) => None,
                Some(Value::Ref(_)) => reference(arg(record, 4)),
                _ => {
                    self.invalid(record, 4, cx);
                    valid = false;
                    break;
                }
            };
        }
        for _ in 0..depth {
            cx.leave();
        }
        if !valid || self.is_halted() {
            return None;
        }
        if value.system.is_none() && value.code.is_none() && value.name.is_none() {
            self.invalid(relation, 6, cx);
            return None;
        }
        Some(value)
    }

    /// 意味を持つ文字列の位置で、型の違う値を黙って落とさない。
    fn optional_texts_valid(&self, record: &Record, positions: &[usize], cx: &mut Cx) -> bool {
        for &position in positions {
            if !matches!(
                arg(record, position),
                Some(Value::Null | Value::Text { .. })
            ) {
                self.invalid(record, position as u64, cx);
                return false;
            }
        }
        true
    }
}
