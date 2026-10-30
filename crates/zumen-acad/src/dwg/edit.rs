//! DWG の変更（決定 0028）。R2000 から R2018 までのオブジェクトを扱う（決定 0071・0083）。見出しの変数も R2018 まで扱う（決定 0084）。
//!
//! 変えたオブジェクトは、元のファイルの後ろに足し、オブジェクトの地図を作り直してその後ろに置く。
//! 見出しの区切りの記録の地図の位置と大きさと、見出しの CRC を直す。ほかのバイトは元のまま残す。
//! 除いたオブジェクトは、地図から除く。元のバイトは残る（決定 0051）。
//!
//! 足したオブジェクトは、変えたオブジェクトと同じく後ろに足し、地図に加える。見出しの変数の HANDSEED は、
//! 大きさ（バイト）が変わらないときだけ、その場で書き換えて区切りの CRC を直す（決定 0052）。
//!
//! 見出しの変数の PDMODE・PDSIZE を書く幅が変わり、見出しの区切りの大きさが変われば、R2000 は書き直した区切りを
//! ファイルの後ろに足し、区切りの記録の位置と大きさと、見出しの CRC を直す。元の区切りのバイトは残る。R2004 からは、
//! 展開した見出しの区切りの中で置き換える（決定 0193）。
//!
//! R2004 は、展開した `AcDb:AcDbObjects` の区切りの後ろにオブジェクトを足し、`AcDb:Handles` の区切りを
//! 作り直して、入れ物の区切りを置き換える（決定 0071）。入れ物は、書くときに組み直す（決定 0070）。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_sdk::{Error, Transact, Transaction};

use super::bits::{Bits, crc16, modular_char, push_modular_char, push_modular_short};
use super::header::{self, Values, handle_seed, set_handle_seed, set_values};
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
    values: Values,
    rejected: Option<Diagnostic>,
}

impl DwgTxn<'_> {
    /// `record` 番目のオブジェクトのデータ（大きさ（MS）の後から CRC の前まで）を置き換える。
    /// 型（BS）を読めないデータは受け付けない
    pub fn set_data(&mut self, record: usize, data: &[u8]) -> Result<(), Error> {
        if record >= self.file.entries.len() {
            return Err(self
                .reject(Diagnostic::new(codes::DWG_OBJECT_MISSING).with("record", record as u64)));
        }
        if type_code(data, self.file.release).is_none() {
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
        let taken = self.file.entries.iter().any(|entry| entry.handle == handle)
            || self.added.iter().any(|(h, _)| *h == handle);
        if taken {
            return Err(
                self.reject(Diagnostic::new(codes::DWG_CHANGE_UNSUPPORTED).with("kind", "handle"))
            );
        }
        if type_code(data, self.file.release).is_none() {
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
        self.seed = Some(seed);
        Ok(())
    }

    /// 見出しの変数の INSUNITS を `units` にする（決定 0069）。大きさ（バイト）が変わる値なら、確定のときに
    /// 失敗する
    pub fn set_units(&mut self, units: i16) -> Result<(), Error> {
        self.values.units = Some(units);
        Ok(())
    }

    /// 見出しの変数の、用紙空間の PLIMMIN と PLIMMAX（X・Y の順）を `limits` にする（決定 0069）
    pub fn set_paper_limits(&mut self, limits: [f64; 4]) -> Result<(), Error> {
        self.values.limits = Some(limits);
        Ok(())
    }

    /// 見出しの変数の PDMODE（点の表し方）を `mode` にする（決定 0190）。書く幅が変われば、見出しの区切りの大きさを
    /// 変える（決定 0193）
    pub fn set_point_mode(&mut self, mode: i16) -> Result<(), Error> {
        self.values.point_mode = Some(mode);
        Ok(())
    }

    /// 見出しの変数の PDSIZE（点の大きさ）を `size` にする（決定 0190）。書く幅が変われば、見出しの区切りの大きさを
    /// 変える（決定 0193）
    pub fn set_point_size(&mut self, size: f64) -> Result<(), Error> {
        self.values.point_size = Some(size);
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
            && !self.values.any()
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
        let header_changed = self.seed.is_some() || self.values.any();
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
        // 大きさが変わった見出しの区切り。R2000 は、ファイルの後ろに置いて区切りの記録を直す（決定 0193）
        let mut moved_header = None;
        if self.values.any() {
            let (span, section) = header
                .and_then(|start| set_values(header_src, start, release, self.values))
                .ok_or_else(|| {
                    invalid(
                        cx,
                        "header variables",
                        header.unwrap_or(0)..header_src.len(),
                    )
                })?;
            if section.len() == span.len() {
                put(header_src, span.start, &section).ok_or(Error::Rejected)?;
            } else if container.is_some() {
                // R2004 からは、展開した区切りの中で置き換える。入れ物は書くときに組み直す（決定 0070）
                header_src.splice(span, section);
            } else {
                moved_header = Some(section);
            }
        }
        for (record, data) in self.staged {
            let Some(entry) = entries.get_mut(record) else {
                return Err(Error::Rejected);
            };
            *entry = push_object(&mut src, entry.handle, &data, release).ok_or(Error::Rejected)?;
        }
        for (handle, data) in self.added {
            entries.push(push_object(&mut src, handle, &data, release).ok_or(Error::Rejected)?);
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
        let mut locators = file.locators.clone();
        if let Some(section) = moved_header {
            let at = src.len();
            src.extend_from_slice(&section);
            let (Ok(seeker), Ok(size)) = (u32::try_from(at), u32::try_from(section.len())) else {
                return Err(invalid(cx, "header variables", at..src.len()));
            };
            set_locator(&mut src, &mut locators, header::NUMBER, seeker, size)
                .ok_or_else(|| invalid(cx, "header", 0..at))?;
        }
        let map = src.len();
        push_object_map(&mut src, &entries);
        let size = src.len() - map;
        let at = src.len();
        let (Ok(seeker), Ok(size)) = (u32::try_from(map), u32::try_from(size)) else {
            return Err(invalid(cx, "object-map", map..at));
        };
        if !locators.iter().any(|l| l.number == OBJECT_MAP_NUMBER) {
            return Err(invalid(cx, "object-map", 0..at));
        }
        set_locator(&mut src, &mut locators, OBJECT_MAP_NUMBER, seeker, size)
            .ok_or_else(|| invalid(cx, "header", 0..at))?;
        file.src = src;
        file.entries = entries;
        file.locators = locators;
        Ok(())
    }
}

impl DwgFile {}

impl Transact for DwgFile {
    type Txn<'a> = DwgTxn<'a>;

    fn transaction(&mut self) -> DwgTxn<'_> {
        DwgTxn {
            file: self,
            staged: BTreeMap::new(),
            removed: BTreeSet::new(),
            added: Vec::new(),
            seed: None,
            values: Values::default(),
            rejected: None,
        }
    }
}

/// R2000 のファイルの見出しの、番号 `number` の区切りの記録の位置と大きさを書き直し、見出しの CRC を直す
fn set_locator(
    src: &mut [u8],
    locators: &mut [super::Locator],
    number: u8,
    seeker: u32,
    size: u32,
) -> Option<()> {
    let index = locators.iter().position(|l| l.number == number)?;
    let place = LOCATORS_START + index * LOCATOR_SIZE;
    let crc_at = LOCATORS_START + locators.len() * LOCATOR_SIZE;
    put(src, place + 1, &seeker.to_le_bytes())?;
    put(src, place + 5, &size.to_le_bytes())?;
    let crc = crc16(0, src.get(..crc_at)?) ^ header_crc_mask(locators.len());
    put(src, crc_at, &crc.to_le_bytes())?;
    let locator = locators.get_mut(index)?;
    locator.seeker = seeker;
    locator.size = size;
    Some(())
}

/// `at` から `bytes` を書き込む
pub(super) fn put(src: &mut [u8], at: usize, bytes: &[u8]) -> Option<()> {
    src.get_mut(at..at.checked_add(bytes.len())?)?
        .copy_from_slice(bytes);
    Some(())
}

/// 版 `release` のハンドル `handle` のオブジェクトのデータ `data` を、大きさ（MS）と CRC を付けて `src` の
/// 後ろに足す。R2010 からは、大きさは、データの初めのハンドルの流れの大きさ（MC）を除いたバイトの数
fn push_object(src: &mut Vec<u8>, handle: u64, data: &[u8], release: Release) -> Option<Entry> {
    let type_code = type_code(data, release)?;
    let (skip, handles) = match handle_bits(data, release) {
        Some((skip, bits)) => (skip, Some(data.len().checked_sub(skip)? * 8 - bits)),
        None => (0, None),
    };
    let offset = src.len();
    push_modular_short(src, (data.len() - skip) as u64);
    let start = src.len() + skip;
    src.extend_from_slice(data);
    let crc = crc16(OBJECT_CRC_SEED, src.get(offset..).unwrap_or_default());
    src.extend_from_slice(&crc.to_le_bytes());
    Some(Entry {
        handle,
        span: offset..src.len(),
        data: start..start + data.len() - skip,
        handles,
        type_code,
    })
}

/// 版 `release` のデータの初めの型。R2010 からは、ハンドルの流れの大きさ（MC）の後の BOT
fn type_code(data: &[u8], release: Release) -> Option<u16> {
    let code = if release >= Release::R2010 {
        let (skip, _) = handle_bits(data, release)?;
        Bits::new(data.get(skip..)?).bot()
    } else {
        Bits::new(data).bs()
    };
    code.and_then(|code| u16::try_from(code).ok())
}

/// R2010 からのデータの初めの、ハンドルの流れの大きさ（MC）のバイトの数と、その値（ビットの数）。MC を
/// 読めないか、ハンドルの流れが残りのデータより大きければ `None`。R2004 までは `None`
fn handle_bits(data: &[u8], release: Release) -> Option<(usize, usize)> {
    if release < Release::R2010 {
        return None;
    }
    let mut skip = 0;
    let bits = usize::try_from(modular_char(data, &mut skip, false)?).ok()?;
    (bits <= data.len().checked_sub(skip)? * 8).then_some((skip, bits))
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
