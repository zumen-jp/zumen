//! DWG の変更（決定 0028）。R2000 と R2004 のオブジェクトを扱う。見出しの変数は R2000 だけを扱う。
//!
//! 変えたオブジェクトは、元のファイルの後ろに足し、オブジェクトの地図を作り直してその後ろに置く。
//! 見出しの区切りの記録の地図の位置と大きさと、見出しの CRC を直す。ほかのバイトは元のまま残す。
//! 除いたオブジェクトは、地図から除く。元のバイトは残る（決定 0051）。
//!
//! 足したオブジェクトは、変えたオブジェクトと同じく後ろに足し、地図に加える。見出しの変数の HANDSEED は、
//! 大きさ（バイト）が変わらないときだけ、その場で書き換えて区切りの CRC を直す（決定 0052）。
//!
//! R2004 は、展開した `AcDb:AcDbObjects` の区切りの後ろにオブジェクトを足し、`AcDb:Handles` の区切りを
//! 作り直して、入れ物の区切りを置き換える（決定 0071）。入れ物は、書くときに組み直す（決定 0070）。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_sdk::{Error, Transact, Transaction};

use super::bits::{Bits, crc16, push_modular_char, push_modular_short};
use super::header::{self, handle_seed, set_handle_seed, set_values};
use super::{
    DwgFile, Entry, LOCATOR_SIZE, LOCATORS_START, OBJECT_CRC_SEED, OBJECT_MAP_NUMBER,
    R2004_SECTIONS, Release, header_crc_mask, invalid, stopped,
};

/// オブジェクトの地図の 1 つの区切りの、データの大きさの上限。試料の DWG の区切りは、
/// これを超えない（推定）
const MAP_SECTION_LIMIT: usize = 2032;

/// DWG の変更をためるトランザクション。[`Transaction::commit`] で、まとめて確かめてから反映する。
/// 変更の操作が失敗したら、そのトランザクションは確定できない。オブジェクトは、
/// [`zumen_sdk::FileModel::record`] と同じ番号で名指しする
#[derive(Debug)]
pub struct DwgTxn<'a> {
    file: &'a mut DwgFile,
    staged: BTreeMap<usize, Vec<u8>>,
    removed: BTreeSet<usize>,
    added: Vec<(u64, Vec<u8>)>,
    seed: Option<u64>,
    units: Option<i16>,
    limits: Option<[f64; 4]>,
    rejected: Option<Diagnostic>,
}

impl DwgTxn<'_> {
    /// `record` 番目のオブジェクトのデータ（大きさ（MS）の後から CRC の前まで）を置き換える。
    /// 型（BS）を読めないデータは受け付けない
    pub fn set_data(&mut self, record: usize, data: &[u8]) -> Result<(), Error> {
        if let Some(diagnostic) = self.file.edit_unsupported() {
            return Err(self.reject(diagnostic));
        }
        if record >= self.file.entries.len() {
            return Err(self
                .reject(Diagnostic::new(codes::DWG_OBJECT_MISSING).with("record", record as u64)));
        }
        if type_code(data).is_none() {
            return Err(
                self.reject(Diagnostic::new(codes::DWG_DATA_INVALID).with("record", record as u64))
            );
        }
        self.staged.insert(record, data.to_vec());
        Ok(())
    }

    /// `record` 番目のオブジェクトを除く。置き換えたデータも捨てる。確定の後は、後ろのオブジェクトの番号が
    /// 詰まる。ほかのオブジェクトが指すハンドルは直さない
    pub fn remove(&mut self, record: usize) -> Result<(), Error> {
        if let Some(diagnostic) = self.file.edit_unsupported() {
            return Err(self.reject(diagnostic));
        }
        if record >= self.file.entries.len() {
            return Err(self
                .reject(Diagnostic::new(codes::DWG_OBJECT_MISSING).with("record", record as u64)));
        }
        self.staged.remove(&record);
        self.removed.insert(record);
        Ok(())
    }

    /// ハンドル `handle` のオブジェクトを、データ `data`（大きさ（MS）の後から CRC の前まで）で足す。
    /// 確定の後は、今のオブジェクトの後ろの番号になる。ほかのオブジェクトと同じハンドルと、型（BS）を
    /// 読めないデータは受け付けない
    pub fn add(&mut self, handle: u64, data: &[u8]) -> Result<(), Error> {
        if let Some(diagnostic) = self.file.edit_unsupported() {
            return Err(self.reject(diagnostic));
        }
        let taken = self.file.entries.iter().any(|entry| entry.handle == handle)
            || self.added.iter().any(|(h, _)| *h == handle);
        if taken {
            return Err(
                self.reject(Diagnostic::new(codes::DWG_CHANGE_UNSUPPORTED).with("kind", "handle"))
            );
        }
        if type_code(data).is_none() {
            let record = self.file.entries.len() + self.added.len();
            return Err(
                self.reject(Diagnostic::new(codes::DWG_DATA_INVALID).with("record", record as u64))
            );
        }
        self.added.push((handle, data.to_vec()));
        Ok(())
    }

    /// 見出しの変数の HANDSEED を `seed` にする。大きさ（バイト）が変わる値なら、確定のときに失敗する
    pub fn set_handle_seed(&mut self, seed: u64) -> Result<(), Error> {
        if let Some(diagnostic) = self.file.header_unsupported() {
            return Err(self.reject(diagnostic));
        }
        self.seed = Some(seed);
        Ok(())
    }

    /// 見出しの変数の INSUNITS を `units` にする（決定 0069）。大きさ（バイト）が変わる値なら、確定のときに
    /// 失敗する
    pub fn set_units(&mut self, units: i16) -> Result<(), Error> {
        if let Some(diagnostic) = self.file.header_unsupported() {
            return Err(self.reject(diagnostic));
        }
        self.units = Some(units);
        Ok(())
    }

    /// 見出しの変数の、用紙空間の PLIMMIN と PLIMMAX（X・Y の順）を `limits` にする（決定 0069）
    pub fn set_paper_limits(&mut self, limits: [f64; 4]) -> Result<(), Error> {
        if let Some(diagnostic) = self.file.header_unsupported() {
            return Err(self.reject(diagnostic));
        }
        self.limits = Some(limits);
        Ok(())
    }

    /// 最初の失敗を、確定のときに診断として返すために覚える
    pub(super) fn reject(&mut self, diagnostic: Diagnostic) -> Error {
        self.rejected.get_or_insert(diagnostic);
        Error::Rejected
    }
}

impl Transaction for DwgTxn<'_> {
    fn commit(self, cx: &mut Cx) -> Result<(), Error> {
        if let Some(diagnostic) = self.rejected {
            cx.report(diagnostic);
            return Err(Error::Rejected);
        }
        if self.staged.is_empty()
            && self.removed.is_empty()
            && self.added.is_empty()
            && self.seed.is_none()
            && self.units.is_none()
            && self.limits.is_none()
        {
            return Ok(());
        }
        // 予算を先にすべて確かめ、反映の途中では止めない
        let work = self.staged.len() + self.added.len() + self.file.entries.len();
        cx.check(Kind::Work, work as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let file = self.file;
        // R2000 はファイル全体、R2004 は展開したオブジェクトの区切りの後ろに足す
        let mut container = file.container.clone();
        let mut src = match &container {
            Some(container) => container
                .section(R2004_SECTIONS[3])
                .unwrap_or_default()
                .to_vec(),
            None => file.src.clone(),
        };
        let objects_changed = !self.staged.is_empty() || !self.added.is_empty();
        let mut entries = file.entries.clone();
        // R2004 の見出しの変数は、展開した見出しの区切りを書き換えて置き換える（決定 0078）
        let mut head = container
            .as_ref()
            .and_then(|container| container.section(R2004_SECTIONS[0]))
            .map(<[u8]>::to_vec);
        let header_changed = self.seed.is_some() || self.units.is_some() || self.limits.is_some();
        let (header_src, header) = match head.as_mut() {
            Some(head) => (head, Some(0)),
            None => (
                &mut src,
                file.locators
                    .iter()
                    .find(|l| l.number == header::NUMBER && l.size > 0)
                    .map(|l| l.seeker as usize),
            ),
        };
        let release = file.release;
        if let Some(value) = self.seed {
            header
                .and_then(|start| handle_seed(header_src, start, release))
                .and_then(|seed| set_handle_seed(header_src, &seed, value))
                .ok_or_else(|| {
                    invalid(
                        cx,
                        "header variables",
                        header.unwrap_or(0)..header_src.len(),
                    )
                })?;
        }
        if self.units.is_some() || self.limits.is_some() {
            header
                .and_then(|start| set_values(header_src, start, release, self.units, self.limits))
                .ok_or_else(|| {
                    invalid(
                        cx,
                        "header variables",
                        header.unwrap_or(0)..header_src.len(),
                    )
                })?;
        }
        for (record, data) in self.staged {
            let Some(entry) = entries.get_mut(record) else {
                return Err(Error::Rejected);
            };
            *entry = push_object(&mut src, entry.handle, &data).ok_or(Error::Rejected)?;
        }
        for (handle, data) in self.added {
            entries.push(push_object(&mut src, handle, &data).ok_or(Error::Rejected)?);
        }
        let entries: Vec<_> = entries
            .into_iter()
            .enumerate()
            .filter(|(record, _)| !self.removed.contains(record))
            .map(|(_, entry)| entry)
            .collect();
        if let Some(mut container) = container.take() {
            let mut map = Vec::new();
            push_object_map(&mut map, &entries);
            if objects_changed {
                container
                    .replace(R2004_SECTIONS[3], src)
                    .ok_or_else(|| invalid(cx, "section", 0..6))?;
            }
            if let Some(head) = head.filter(|_| header_changed) {
                container
                    .replace(R2004_SECTIONS[0], head)
                    .ok_or_else(|| invalid(cx, "section", 0..6))?;
            }
            container
                .replace(R2004_SECTIONS[2], map)
                .ok_or_else(|| invalid(cx, "section", 0..6))?;
            file.container = Some(container);
            file.entries = entries;
            return Ok(());
        }
        let map = src.len();
        push_object_map(&mut src, &entries);
        let size = src.len() - map;
        let at = src.len();
        let (Ok(seeker), Ok(size)) = (u32::try_from(map), u32::try_from(size)) else {
            return Err(invalid(cx, "object-map", map..at));
        };
        let mut locators = file.locators.clone();
        let Some(index) = locators.iter().position(|l| l.number == OBJECT_MAP_NUMBER) else {
            return Err(invalid(cx, "object-map", 0..at));
        };
        let place = LOCATORS_START + index * LOCATOR_SIZE;
        let crc_at = LOCATORS_START + locators.len() * LOCATOR_SIZE;
        put(&mut src, place + 1, &seeker.to_le_bytes())
            .and_then(|()| put(&mut src, place + 5, &size.to_le_bytes()))
            .ok_or_else(|| invalid(cx, "header", 0..at))?;
        let crc = crc16(0, src.get(..crc_at).unwrap_or_default()) ^ header_crc_mask(locators.len());
        put(&mut src, crc_at, &crc.to_le_bytes()).ok_or_else(|| invalid(cx, "header", 0..at))?;
        if let Some(locator) = locators.get_mut(index) {
            locator.seeker = seeker;
            locator.size = size;
        }
        file.src = src;
        file.entries = entries;
        file.locators = locators;
        Ok(())
    }
}

impl DwgFile {
    /// この版の DWG のオブジェクトを変えられなければ、その診断。R2000 と R2004 を変えられる（決定 0071）
    pub(super) fn edit_unsupported(&self) -> Option<Diagnostic> {
        self.unsupported(self.release > Release::R2004)
    }

    /// この版の DWG の見出しの変数を変えられず、共有の表から書き戻せなければ、その診断。R2000 と R2004 を
    /// 扱う（決定 0078）
    pub(super) fn header_unsupported(&self) -> Option<Diagnostic> {
        self.unsupported(self.release > Release::R2004)
    }

    fn unsupported(&self, unsupported: bool) -> Option<Diagnostic> {
        unsupported.then(|| {
            let version = String::from_utf8_lossy(self.version()).into_owned();
            Diagnostic::new(codes::DWG_EDIT_UNSUPPORTED).with("version", version)
        })
    }
}

impl Transact for DwgFile {
    type Txn<'a> = DwgTxn<'a>;

    fn transaction(&mut self) -> DwgTxn<'_> {
        DwgTxn {
            file: self,
            staged: BTreeMap::new(),
            removed: BTreeSet::new(),
            added: Vec::new(),
            seed: None,
            units: None,
            limits: None,
            rejected: None,
        }
    }
}

/// `at` から `bytes` を書き込む
pub(super) fn put(src: &mut [u8], at: usize, bytes: &[u8]) -> Option<()> {
    src.get_mut(at..at.checked_add(bytes.len())?)?
        .copy_from_slice(bytes);
    Some(())
}

/// ハンドル `handle` のオブジェクトのデータ `data` を、大きさ（MS）と CRC を付けて `src` の後ろに足す
fn push_object(src: &mut Vec<u8>, handle: u64, data: &[u8]) -> Option<Entry> {
    let type_code = type_code(data)?;
    let offset = src.len();
    push_modular_short(src, data.len() as u64);
    let start = src.len();
    src.extend_from_slice(data);
    let crc = crc16(OBJECT_CRC_SEED, src.get(offset..).unwrap_or_default());
    src.extend_from_slice(&crc.to_le_bytes());
    Some(Entry {
        handle,
        span: offset..src.len(),
        data: start..start + data.len(),
        handles: None,
        type_code,
    })
}

/// データの初めの型（BS）
fn type_code(data: &[u8]) -> Option<u16> {
    Bits::new(data)
        .bs()
        .and_then(|code| u16::try_from(code).ok())
}

/// オブジェクトの地図を、ハンドルの順に書く。区切りごとに、ハンドルと位置の差分を 0 から数え、
/// 大きさ（上位のバイトから）と CRC（上位のバイトから）を付ける。大きさが 2 の区切りで終える
fn push_object_map(out: &mut Vec<u8>, entries: &[super::Entry]) {
    let mut order: Vec<_> = entries.iter().collect();
    order.sort_by_key(|entry| entry.handle);
    let mut body = Vec::new();
    let (mut handle, mut location) = (0u64, 0i64);
    for entry in order {
        let offset = i64::try_from(entry.span.start).unwrap_or(i64::MAX);
        let mut pair = Vec::new();
        push_modular_char(&mut pair, (entry.handle - handle) as i64, false);
        push_modular_char(&mut pair, offset - location, true);
        if body.len() + pair.len() > MAP_SECTION_LIMIT {
            push_map_section(out, &body);
            body.clear();
            pair.clear();
            push_modular_char(&mut pair, entry.handle as i64, false);
            push_modular_char(&mut pair, offset, true);
        }
        body.extend_from_slice(&pair);
        (handle, location) = (entry.handle, offset);
    }
    if !body.is_empty() {
        push_map_section(out, &body);
    }
    push_map_section(out, &[]);
}

fn push_map_section(out: &mut Vec<u8>, body: &[u8]) {
    let start = out.len();
    out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(body);
    let crc = crc16(OBJECT_CRC_SEED, out.get(start..).unwrap_or_default());
    out.extend_from_slice(&crc.to_be_bytes());
}
