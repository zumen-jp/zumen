use super::*;
use crate::DwgFile;
use crate::dwg::objects::dim_fields;
use zumen_base::{Budget, CancelToken, Diagnostics};
use zumen_drawing::{Changes, Edit};
use zumen_sdk::{Apply, Codec, Lift};

/// R2010 より前の DIMSTYLE のデータ `data` の、型ごとのフィールドの後からハンドルのストリームまでのビットの数
fn trailing_bits(data: &[u8], release: Release) -> Option<usize> {
    let [_, start, handles_at] = object_fields(data, release)?;
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    bits.text()?;
    bits.bit()?;
    bits.bs()?;
    bits.bit()?;
    for (_, field) in dim_fields(release) {
        match field {
            Dim::Text => bits.text().map(drop),
            Dim::Real(_) => bits.bd().map(drop),
            Dim::Bit(_) => bits.bit().map(drop),
            Dim::Short(_) => bits.bs().map(drop),
            Dim::Color(_) => bits.cmc().map(drop),
        }?;
    }
    handles_at.checked_sub(bits.pos())
}

fn dimstyles(file: &DwgFile) -> Vec<Option<usize>> {
    file.entries
        .iter()
        .filter(|entry| entry.type_code == kinds::DIMSTYLE)
        .map(|entry| trailing_bits(file.data(entry), file.release))
        .collect()
}

/// 公開サンプルの R2000・R2004 の DWG の寸法スタイルは、フィールドの後に使い道の分からない 1 ビットを持つ。足した
/// 寸法スタイルもこのビットを持つ。ないと、AutoCAD 2027 は図面を開かない（#893）
#[test]
fn added_dimension_styles_end_with_the_unknown_bit_before_r2010() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    let mut checked = BTreeSet::new();
    for name in ["zumen-layout-r2000.dwg", "zumen-layout-r2004.dwg"] {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let src = std::fs::read(dir.join(name)).unwrap();
        let mut file = crate::Dwg::read(&src, &mut cx).unwrap();
        let before = dimstyles(&file);
        assert!(
            !before.is_empty() && before.iter().all(|&rest| rest == Some(1)),
            "{name}: {before:?}"
        );
        let drawing = file.lift(&mut cx).unwrap();
        let mut style = drawing.dimension_styles.first().unwrap().clone();
        style.name = "R0_DIM_NEW".to_owned();
        style.size = 3.25;
        style.tick = Some(0.5);
        style.scale = 1.5;
        let changes = Changes {
            edits: vec![Edit::AddDimensionStyle {
                style: Box::new(style),
            }],
            document: None,
        };
        file.apply(&changes, &mut cx).unwrap();
        let mut out = Vec::new();
        crate::Dwg::write(&file, &mut out, &mut cx).unwrap();
        let again = crate::Dwg::read(&out, &mut cx).unwrap();
        let after = dimstyles(&again);
        assert_eq!(after.len(), before.len() + 1, "{name}");
        assert!(
            after.iter().all(|&rest| rest == Some(1)),
            "{name}: {after:?}"
        );
        checked.insert(again.release);
    }
    assert_eq!(checked.len(), 2);
}
