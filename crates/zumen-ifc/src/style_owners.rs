//! 表示・材料の逆参照が存在しないファイルで、同じレコード列を要素ごとに走査しない。

use crate::{Cx, Error, IfcFile, Kind, Record, stopped};
use zumen_base::Limit;

#[derive(Clone, Copy)]
#[repr(u8)]
pub(crate) enum Owner {
    StyledItem = 1,
    Layer = 2,
    Material = 4,
    Type = 8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StyleOwners {
    present: u8,
    instance_parts: u64,
}

impl StyleOwners {
    /// 番号表を作る 1 レコード分の検査で数える。複合実体では後続の部分も候補になりうる。
    pub(crate) fn observe(&mut self, record: &Record, source: &[u8]) {
        if record.id().is_some() {
            self.instance_parts += record.parts().len() as u64;
        }
        let [part] = record.parts() else {
            if record.parts().len() > 1 {
                self.present = u8::MAX;
            }
            return;
        };
        let name = source.get(part.name()).unwrap_or_default();
        let owner = if name.eq_ignore_ascii_case(b"IFCSTYLEDITEM") {
            Owner::StyledItem
        } else if name.eq_ignore_ascii_case(b"IFCPRESENTATIONLAYERASSIGNMENT")
            || name.eq_ignore_ascii_case(b"IFCPRESENTATIONLAYERWITHSTYLE")
        {
            Owner::Layer
        } else if name.eq_ignore_ascii_case(b"IFCRELASSOCIATESMATERIAL") {
            Owner::Material
        } else if name.eq_ignore_ascii_case(b"IFCRELDEFINESBYTYPE") {
            Owner::Type
        } else {
            return;
        };
        self.present |= owner as u8;
    }
}

impl IfcFile {
    pub(crate) fn owner_records(&self, owner: Owner, cx: &mut Cx) -> Result<&[Record], Error> {
        if self.style_owners.present & owner as u8 != 0 {
            return Ok(self.records());
        }
        // 単純実体だけなのでレイヤと直接表示は各レコード 1 Work。
        // 材料・型の関係は番号を持つ各レコードの部分も 1 Work ずつ数える。
        let mut work = self.records().len() as u64;
        if matches!(owner, Owner::Material | Owner::Type) {
            work = work.saturating_add(self.style_owners.instance_parts);
        }
        if let Limit::Finite(limit) = cx.budget.limits().get(Kind::Work)
            && work > limit.saturating_sub(cx.budget.used(Kind::Work))
        {
            // 途中で上限に達するときは、従来の順序で正確な場所と消費量を返す。
            return Ok(self.records());
        }
        if let Some(first) = self.records().first() {
            cx.check(Kind::Work, work)
                .map_err(|stop| stopped(cx, stop, Some(first.span())))?;
        }
        Ok(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ifc;
    use zumen_base::{Budget, CancelToken, Diagnostic, Diagnostics, Limits};
    use zumen_building::Geometry;
    use zumen_sdk::Codec;

    #[test]
    fn absent_owners_match_full_scans_at_every_work_limit() {
        let source = b"ISO-10303-21;HEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('IFC4'));ENDSEC;DATA;
#1=IFCCARTESIANPOINT((0.,0.,0.));#2=IFCCARTESIANPOINT((2.,0.,0.));
#3=IFCPOLYLINE((#1,#2));#4=IFCAXIS2PLACEMENT3D(#1,$,$);
#5=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#4,$);
#6=IFCSHAPEREPRESENTATION(#5,'Body','Curve3D',(#3));#7=IFCPRODUCTDEFINITIONSHAPE($,$,(#6));
#8=IFCWALL('0000000000000000000001',$,'Wall',$,$,$,#7,$,.STANDARD.);ENDSEC;END-ISO-10303-21;";
        let cancel = CancelToken::new();
        let fast = Ifc::read(
            source,
            &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel),
        )
        .unwrap();
        let mut full = fast.clone();
        assert_eq!(fast.style_owners.present, 0);
        full.style_owners.present = u8::MAX;
        let run = |file: &IfcFile, work| -> (Result<Geometry, Error>, Vec<Diagnostic>, Budget) {
            let mut budget = Budget::new(Limits::default().with(Kind::Work, work));
            let mut diag = Diagnostics::new();
            let result = file.owned_geometry(8, &mut Cx::new(&mut budget, &mut diag, &cancel));
            (result, diag.into_vec(), budget)
        };
        let expected = run(&full, Limit::Unlimited);
        assert!(expected.0.is_ok());
        assert_eq!(run(&fast, Limit::Unlimited), expected);
        for limit in 0..=expected.2.used(Kind::Work) {
            let work = Limit::Finite(limit);
            assert_eq!(run(&fast, work), run(&full, work), "work = {limit}");
        }
    }
}
