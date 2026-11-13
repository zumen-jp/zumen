//! R2004 の共有表に加えて、同系統の書き出しに必要な DWG の参照順と共通フィールドを読む。

use std::collections::{BTreeMap, BTreeSet};

use super::{Bits, Class, DwgFile, Release, kinds, objects};
use crate::db::{AcadDb, Section};
use zumen_base::{Cx, Kind};
use zumen_sdk::Error;

#[derive(Default)]
pub(crate) struct Extra {
    pub(crate) reactors: Vec<u64>,
    pub(crate) picture: Vec<u8>,
    pub(crate) plotstyle: bool,
}

#[derive(Default)]
pub(crate) struct DxfSource<'a> {
    pub(crate) classes: Vec<(&'a Class, u64)>,
    pub(crate) extras: BTreeMap<u64, Extra>,
    pub(crate) order: BTreeMap<u64, Vec<u64>>,
    /// 表の名前ごとの、表のオブジェクト（CONTROL）のハンドル。共有の表の TABLE は表の名前を持たないので、
    /// 項目のない表を書くときは、オブジェクトの型で決めたこの対応を使う
    pub(crate) controls: BTreeMap<&'static [u8], u64>,
}

// 表のオブジェクト（CONTROL）の型（ODS 5.4.1 の 20.4）から、DXF の TABLE の名前を決める。
fn control_table(type_code: u16) -> Option<&'static [u8]> {
    Some(match type_code {
        kinds::VPORT_CONTROL => b"VPORT",
        kinds::LTYPE_CONTROL => b"LTYPE",
        kinds::LAYER_CONTROL => b"LAYER",
        kinds::STYLE_CONTROL => b"STYLE",
        kinds::VIEW_CONTROL => b"VIEW",
        kinds::UCS_CONTROL => b"UCS",
        kinds::APPID_CONTROL => b"APPID",
        kinds::DIMSTYLE_CONTROL => b"DIMSTYLE",
        kinds::BLOCK_RECORD_CONTROL => b"BLOCK_RECORD",
        _ => return None,
    })
}

impl DwgFile {
    pub(crate) fn dxf_source(
        &self,
        db: &AcadDb,
        kept: &BTreeSet<u64>,
        cx: &mut Cx<'_>,
    ) -> Result<DxfSource<'_>, Error> {
        let mut source = DxfSource::default();
        let mut instances = BTreeMap::new();
        for entry in &self.entries {
            cx.check(Kind::Work, 1)?;
            // 表のオブジェクトは、項目がなく `kept` に入っていなくても読む。書くかどうかは呼び出し側が決める
            let control = control_table(entry.type_code).filter(|_| {
                db.get(entry.handle)
                    .is_some_and(|o| o.kind == b"TABLE" && o.section == Section::Tables)
            });
            if let Some(table) = control
                && source.controls.insert(table, entry.handle).is_some()
            {
                return Err(Error::Unsupported);
            }
            if control.is_none() && !kept.contains(&entry.handle) {
                continue;
            }
            let object = db.get(entry.handle).ok_or(Error::Unsupported)?;
            // この R2004 DXF の対象では LAYOUT・ACDBPLACEHOLDER は組み込みの型。
            if matches!(
                object.kind.as_slice(),
                b"ARC_DIMENSION" | b"ACDBDICTIONARYWDFLT"
            ) {
                let (name, count) = instances
                    .entry(entry.type_code)
                    .or_insert((object.kind.as_slice(), 0_u64));
                if *name != object.kind {
                    return Err(Error::Unsupported);
                }
                *count += 1;
            }
            let data = self.data(entry);
            cx.check(Kind::Work, data.len() as u64)?;
            let entity = matches!(object.section, Section::Entities | Section::Blocks);
            let objects::DxfCommon {
                mut handles,
                count,
                picture,
                plotstyle,
            } = objects::common_for_dxf(data, entity).ok_or(Error::Unsupported)?;
            let mut extra = Extra {
                plotstyle,
                ..Extra::default()
            };
            for _ in 0..count {
                cx.check(Kind::Work, 1)?;
                let target = handles
                    .handle()
                    .and_then(|h| h.resolve(entry.handle))
                    .ok_or(Error::Unsupported)?;
                cx.check(Kind::DecodedBytes, 8)?;
                extra.reactors.push(target);
            }
            if let Some((at, size)) = picture {
                cx.check(Kind::DecodedBytes, size as u64)?;
                let mut bits = Bits::with_release(data, Release::R2004);
                bits.seek(at);
                extra.picture.reserve(size);
                for start in (0..size).step_by(127) {
                    cx.check(Kind::Work, 1)?;
                    for _ in 0..(size - start).min(127) {
                        extra.picture.push(bits.rc().ok_or(Error::Unsupported)?);
                    }
                }
            }
            if !extra.reactors.is_empty() || !extra.picture.is_empty() || plotstyle {
                source.extras.insert(entry.handle, extra);
            }
            if entry.type_code == kinds::BLOCK_RECORD {
                source
                    .order
                    .insert(entry.handle, objects::owned_ids(data, entry.handle, cx)?);
            }
        }
        // DWG はクラス番号で宣言を選び、DXF は名前で参照する。番号や出力名が衝突するときは切り替える。
        let mut declared = BTreeSet::new();
        for class in &self.classes {
            cx.check(Kind::Work, 1)?;
            let Some(&(name, count)) = instances.get(&class.number) else {
                continue;
            };
            let (entity, cpp): (bool, &[u8]) = match name {
                b"ARC_DIMENSION" => (true, b"AcDbArcDimension"),
                b"ACDBDICTIONARYWDFLT" => (false, b"AcDbDictionaryWithDefault"),
                _ => return Err(Error::Unsupported),
            };
            if class.name != name
                || class.entity != entity
                || class.cpp != cpp
                || class.proxy
                || !declared.insert(name)
            {
                return Err(Error::Unsupported);
            }
            source.classes.push((class, count));
        }
        if source.classes.len() != instances.len() {
            return Err(Error::Unsupported);
        }
        Ok(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dwg;
    use crate::dwg::bits::BitWriter;
    use crate::dxf::R2004Conversion;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};
    use zumen_sdk::{Codec, Transact, Transaction};

    fn public_file(cx: &mut Cx<'_>) -> DwgFile {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/dwg/zumen-arcdim-r2004.dwg");
        let bytes = std::fs::read(path).unwrap();
        Dwg::read(&bytes, cx).unwrap()
    }

    #[test]
    fn a_changed_dwg_block_order_is_the_order_read_back_from_the_dxf() {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut file = public_file(&mut cx);
        let db = file.objects(&mut cx).unwrap();
        let row = db
            .objects()
            .iter()
            .find(|r| {
                r.kind == b"BLOCK_RECORD" && r.first(2) == Some(&crate::Data::Text(b"*D1".to_vec()))
            })
            .unwrap();
        let id = row.handle().unwrap();
        let (index, entry) = file
            .entries
            .iter()
            .enumerate()
            .find(|(_, e)| e.handle == id)
            .unwrap();
        let data = file.data(entry);
        let owned = objects::owned(data, id, Release::R2004, None).unwrap();
        let [(first, _), (second, _), ..] = owned.handles.as_slice() else {
            panic!("ブロック内の図形が足りない")
        };
        let mut expected: Vec<_> = owned.handles.iter().map(|(_, h)| *h).collect();
        expected.swap(0, 1);
        let mut bits = BitWriter::default();
        for span in [
            0..first.start,
            second.clone(),
            first.clone(),
            second.end..data.len() * 8,
        ] {
            bits.copy(data, span).unwrap();
        }
        let mut txn = file.transaction();
        txn.set_data(index, &bits.into_bytes()).unwrap();
        txn.commit(&mut cx).unwrap();
        let R2004Conversion::Dxf(output) = file.reencode_r2004_dxf(&mut cx).unwrap() else {
            panic!("同系統の変換で書けない")
        };
        let written = crate::Dxf::read(&output.bytes, &mut cx)
            .unwrap()
            .objects(&mut cx)
            .unwrap();
        let actual: Vec<_> = written
            .objects()
            .iter()
            .filter(|r| r.owner() == Some(id) && !matches!(r.kind.as_slice(), b"BLOCK" | b"ENDBLK"))
            .filter_map(crate::Object::handle)
            .collect();
        assert_eq!(actual, expected);
    }

    // 項目のない VIEW・UCS の表を欠く DXF を、AutoCAD は読み込まない（#879）。
    #[test]
    fn tables_without_entries_keep_the_handles_of_their_control_objects() {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let file = public_file(&mut cx);
        let R2004Conversion::Dxf(output) = file.reencode_r2004_dxf(&mut cx).unwrap() else {
            panic!("同系統の変換で書けない")
        };
        let written = crate::Dxf::read(&output.bytes, &mut cx)
            .unwrap()
            .objects(&mut cx)
            .unwrap();
        let tables: Vec<_> = written
            .objects()
            .iter()
            .filter(|r| r.kind == b"TABLE")
            .map(|r| (r.first(2).cloned(), r.handle()))
            .collect();
        // AutoCAD が書く順。ハンドルは、DWG の表のオブジェクトのもの
        let expected = [
            (b"VPORT".as_slice(), kinds::VPORT_CONTROL),
            (b"LTYPE", kinds::LTYPE_CONTROL),
            (b"LAYER", kinds::LAYER_CONTROL),
            (b"STYLE", kinds::STYLE_CONTROL),
            (b"VIEW", kinds::VIEW_CONTROL),
            (b"UCS", kinds::UCS_CONTROL),
            (b"APPID", kinds::APPID_CONTROL),
            (b"DIMSTYLE", kinds::DIMSTYLE_CONTROL),
            (b"BLOCK_RECORD", kinds::BLOCK_RECORD_CONTROL),
        ]
        .map(|(name, code)| {
            let control = file.entries.iter().find(|e| e.type_code == code).unwrap();
            (Some(crate::Data::Text(name.to_vec())), Some(control.handle))
        });
        assert_eq!(tables, expected);
        for kind in [b"VIEW".as_slice(), b"UCS"] {
            assert!(written.objects().iter().all(|r| r.kind != kind));
            let table = written
                .objects()
                .iter()
                .find(|r| {
                    r.kind == b"TABLE" && r.first(2) == Some(&crate::Data::Text(kind.to_vec()))
                })
                .unwrap();
            assert_eq!(table.first(70), Some(&crate::Data::Int(0)));
        }
        // 書いた表のオブジェクトを、落としたレコードとして診断しない
        assert!(diagnostics.iter().all(|d| {
            d.code() != zumen_base::codes::DWG_DXF_RECORD_DROPPED
                || d.param("kind") != Some(&"TABLE".into())
        }));
    }

    fn convert(file: &DwgFile, limits: Limits) -> (Result<R2004Conversion, Error>, u64) {
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let stop = CancelToken::new();
        let result = file.reencode_r2004_dxf(&mut Cx::new(&mut budget, &mut diag, &stop));
        (result, budget.used(Kind::Work))
    }

    #[test]
    fn uses_the_declared_number_even_when_an_unused_class_has_the_same_name() {
        let mut file = public_file(&mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ));
        let (Ok(R2004Conversion::Dxf(original)), _) = convert(&file, Limits::default()) else {
            panic!("DXF を書けない")
        };
        let arc = file
            .classes
            .iter()
            .find(|c| c.name == b"ARC_DIMENSION")
            .unwrap()
            .clone();
        let shadow = Class {
            number: 60_000,
            cpp: b"OtherClass".to_vec(),
            entity: false,
            ..arc.clone()
        };
        file.classes.insert(0, shadow);
        let (Ok(R2004Conversion::Dxf(output)), _) = convert(&file, Limits::default()) else {
            panic!("元の宣言で書けない")
        };
        assert_eq!(output.bytes, original.bytes);
        file.classes.push(Class {
            cpp: b"OtherClass".to_vec(),
            ..arc
        });
        assert!(matches!(
            convert(&file, Limits::default()).0,
            Ok(R2004Conversion::DrawingSource(_))
        ));
    }

    #[test]
    fn unused_classes_spend_work_and_can_stop_conversion() {
        let mut file = public_file(&mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ));
        let (Ok(R2004Conversion::Dxf(before)), used) = convert(&file, Limits::default()) else {
            panic!("DXF を書けない")
        };
        let class = file.classes.first().unwrap().clone();
        for number in 60000..60500 {
            file.classes.push(Class {
                number,
                name: format!("UNUSED{number}").into_bytes(),
                ..class.clone()
            });
        }
        let (Ok(R2004Conversion::Dxf(after)), work) = convert(&file, Limits::default()) else {
            panic!("DXF を書けない")
        };
        assert_eq!(before.bytes, after.bytes);
        assert!(work >= used + 500);
        let (limited, _) = convert(
            &file,
            Limits::default().with(Kind::Work, Limit::Finite(used)),
        );
        assert!(matches!(limited, Err(Error::Stopped(_))));
    }
}
