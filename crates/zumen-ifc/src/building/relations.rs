//! 接続・被覆・担当の関係。IFC2X3 / IFC4 / IFC4.3 の各実体の Attributes による。

use super::*;

impl Lifter<'_> {
    pub(super) fn metadata_relation(&mut self, id: u64, record: &Record, cx: &mut Cx) -> bool {
        if self
            .file
            .kind(record)
            .eq_ignore_ascii_case(b"IFCRELCONNECTSELEMENTS")
        {
            let left = self.relation_endpoint(record, 6, false, cx);
            let right = self.relation_endpoint(record, 7, false, cx);
            if let (Some(left), Some(right)) = (left, right) {
                self.metadata_link(left, right, "IfcRelConnectsElements", id, record, None, cx);
                self.metadata_link(right, left, "IfcRelConnectsElements", id, record, None, cx);
            }
            return true;
        }
        let covers = self
            .file
            .kind(record)
            .eq_ignore_ascii_case(b"IFCRELCOVERSBLDGELEMENTS");
        let (target_at, list_at, name) = if covers {
            (5, 6, "IfcRelCoversBldgElements")
        } else {
            (7, 5, "IfcRelAssignsToActor")
        };
        let Some(target) = self.relation_endpoint(record, target_at, !covers, cx) else {
            return true;
        };
        let Some(Value::List(values)) = arg(record, list_at) else {
            self.invalid(record, list_at as u64, cx);
            return true;
        };
        if values.is_empty() {
            self.invalid(record, list_at as u64, cx);
        }
        for (position, value) in values.iter().enumerate() {
            if !self.charge(cx, Kind::Work, 1) {
                break;
            }
            let Some(related_id) =
                reference(Some(value)).filter(|&id| self.file.instance(id).is_some())
            else {
                self.invalid(record, list_at as u64, cx);
                continue;
            };
            let Ok(position) = u32::try_from(position) else {
                self.invalid(record, list_at as u64, cx);
                continue;
            };
            let Some(&related) = self.elements.get(&related_id) else {
                if !self.copied(cx, ITEM) {
                    break;
                }
                self.unmapped_relations.push((id, related_id, position));
                continue;
            };
            let valid = if covers {
                self.is(related_id, b"IFCCOVERING")
            } else {
                self.building.id(related).is_some_and(|_| {
                    self.file
                        .instance(related_id)
                        .is_some_and(|r| category(self.file.kind(r), self.file.schema()).is_some())
                })
            };
            if !valid {
                self.invalid(record, list_at as u64, cx);
                continue;
            }
            self.metadata_link(related, target, name, id, record, Some(position), cx);
            if covers {
                self.metadata_link(target, related, name, id, record, Some(position), cx);
            }
        }
        true
    }

    /// 必須の単独参照。接続・被覆は物理要素、担当先は Actor / Occupant と確認する。
    fn relation_endpoint(
        &mut self,
        record: &Record,
        at: usize,
        actor: bool,
        cx: &mut Cx,
    ) -> Option<ElementRef> {
        let valid = reference(arg(record, at)).and_then(|id| {
            let source = self.file.instance(id)?;
            let accepted = if actor {
                self.is(id, b"IFCACTOR") || self.is(id, b"IFCOCCUPANT")
            } else {
                category(self.file.kind(source), self.file.schema()) == Some(Category::Element)
            };
            accepted.then(|| self.elements.get(&id).copied()).flatten()
        });
        if valid.is_none() && !self.is_halted() {
            self.invalid(record, at as u64, cx);
        }
        valid
    }

    #[allow(clippy::too_many_arguments)]
    fn metadata_link(
        &mut self,
        element: ElementRef,
        target: ElementRef,
        name: &str,
        id: u64,
        record: &Record,
        position: Option<u32>,
        cx: &mut Cx,
    ) {
        if !self.charge(cx, Kind::Work, 1) || !self.copied(cx, 128 + name.len() as u64) {
            return;
        }
        self.link(element, name, target);
        self.source(element, id, record, position, cx);
    }
}
