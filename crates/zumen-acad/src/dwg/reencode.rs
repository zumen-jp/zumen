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
    pub(crate) classes: &'a [Class],
    pub(crate) extras: BTreeMap<u64, Extra>,
    pub(crate) order: BTreeMap<u64, Vec<u64>>,
}

impl DwgFile {
    pub(crate) fn dxf_source(
        &self,
        db: &AcadDb,
        kept: &BTreeSet<u64>,
        cx: &mut Cx<'_>,
    ) -> Result<DxfSource<'_>, Error> {
        let mut source = DxfSource {
            classes: &self.classes,
            ..DxfSource::default()
        };
        for entry in &self.entries {
            cx.check(Kind::Work, 1)?;
            if !kept.contains(&entry.handle) {
                continue;
            }
            let object = db.get(entry.handle).ok_or(Error::Unsupported)?;
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
        Ok(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dwg;
    use crate::dwg::bits::BitWriter;
    use crate::dxf::R2004Conversion;
    use zumen_base::{Budget, CancelToken, Diagnostics};
    use zumen_sdk::{Codec, Transact, Transaction};

    #[test]
    fn a_changed_dwg_block_order_is_the_order_read_back_from_the_dxf() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/dwg/zumen-arcdim-r2004.dwg");
        let bytes = std::fs::read(path).unwrap();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut file = Dwg::read(&bytes, &mut cx).unwrap();
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
}
