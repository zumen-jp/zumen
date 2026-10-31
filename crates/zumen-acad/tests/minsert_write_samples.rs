//! AutoCAD で作った MINSERT を変更して DWG に書き戻す（決定 0161、#316）。

use std::path::Path;
use zumen_acad::{Data, Dwg};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_sdk::Codec;

/// もともと属性を持つものと持たないものの列数・行の間隔を変え、付随する属性を保って読み直す。
#[test]
fn autocad_minserts_keep_their_attributes_when_the_grid_is_changed() {
    for version in ["r2000", "r2004", "r2018"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/dwg")
            .join(format!("zumen-minsert-{version}.dwg"));
        let src = std::fs::read(&path).unwrap();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut file = Dwg::read(&src, &mut cx).unwrap();
        let before = file.objects(&mut cx).unwrap();
        let mut changed = before.clone();
        let handles = [0x2e7, 0x2e8, 0x2e9, 0x2ef];
        for handle in handles {
            let index = changed
                .objects()
                .iter()
                .position(|o| o.handle() == Some(handle))
                .unwrap();
            let object = changed.object_mut(index).unwrap();
            assert_eq!(object.kind, b"INSERT", "{version} {handle:x}");
            for (code, value) in [(70, Data::Int(4)), (45, Data::Real(18.0))] {
                let field = object.values.iter_mut().find(|v| v.code == code).unwrap();
                field.data = value;
            }
        }
        let attributes: Vec<_> = before
            .objects()
            .iter()
            .filter(|o| o.kind == b"ATTRIB" && o.owner().is_some_and(|h| handles.contains(&h)))
            .cloned()
            .collect();
        assert_eq!(attributes.len(), 4, "{version}");
        for (handle, count) in [(0x2e7, 0), (0x2e8, 0), (0x2e9, 2), (0x2ef, 2)] {
            assert_eq!(
                attributes
                    .iter()
                    .filter(|o| o.owner() == Some(handle))
                    .count(),
                count,
                "{version} {handle:x}"
            );
        }
        let endings: Vec<_> = before
            .objects()
            .iter()
            .filter(|o| o.kind == b"SEQEND" && o.owner().is_some_and(|h| handles.contains(&h)))
            .cloned()
            .collect();
        for (handle, count) in [(0x2e7, 0), (0x2e8, 0), (0x2e9, 1), (0x2ef, 1)] {
            assert_eq!(
                endings.iter().filter(|o| o.owner() == Some(handle)).count(),
                count,
                "{version} {handle:x}"
            );
        }
        file.write_back(&changed, &mut cx).unwrap();
        let mut bytes = Vec::new();
        Dwg::write(&file, &mut bytes, &mut cx).unwrap();
        let mut fresh = Diagnostics::new();
        let mut budget = Budget::default();
        let mut cx = Cx::new(&mut budget, &mut fresh, &cancel);
        let after = Dwg::read(&bytes, &mut cx)
            .unwrap()
            .objects(&mut cx)
            .unwrap();
        assert!(
            fresh.iter().all(|d| d.code() != codes::DWG_CRC_INVALID),
            "{version}"
        );
        for handle in handles {
            let expected = changed
                .objects()
                .iter()
                .find(|o| o.handle() == Some(handle))
                .unwrap();
            let actual = after
                .objects()
                .iter()
                .find(|o| o.handle() == Some(handle))
                .unwrap();
            assert_eq!(actual.values, expected.values, "{version} {handle:x}");
        }
        for expected in attributes.into_iter().chain(endings) {
            let actual = after
                .objects()
                .iter()
                .find(|o| o.handle() == expected.handle())
                .unwrap();
            assert_eq!(
                actual.kind,
                expected.kind,
                "{version} {:?}",
                expected.handle()
            );
            assert_eq!(
                actual.owner(),
                expected.owner(),
                "{version} {:?}",
                expected.handle()
            );
            assert_eq!(
                actual.values,
                expected.values,
                "{version} {:?}",
                expected.handle()
            );
        }
    }
}
