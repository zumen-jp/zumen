//! ZIP の L0（決定 0008・0210）。
//!
//! 中央ディレクトリを読み、項目の名前と範囲を返す。項目は、求められたときに stored か deflate で展開し、
//! CRC-32 を確かめる。ZIP64 は読み、分けた書庫・暗号化した項目・ほかの圧縮の方式は読まない。
//!
//! 安全でない名前・範囲の重なる項目・壊れた見出しの項目は、読まずに診断を返し、残りの項目を読む。

mod inflate;
mod read;

use core::ops::Range;
use std::borrow::Cow;

use zumen_base::crc::crc32;
use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_sdk::Error;

pub use read::read;

/// 圧縮の方式
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// 圧縮しない（0）
    Stored,
    /// deflate（8）
    Deflate,
}

/// 書庫の項目
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    name: String,
    method: Method,
    crc: u32,
    size: u64,
    header: Range<usize>,
    data: Range<usize>,
}

impl Entry {
    /// 名前。区切りは `/`。ディレクトリの項目は `/` で終わる
    pub fn name(&self) -> &str {
        &self.name
    }

    /// ディレクトリの項目なら真
    pub fn is_directory(&self) -> bool {
        self.name.ends_with('/')
    }

    /// 圧縮の方式
    pub fn method(&self) -> Method {
        self.method
    }

    /// 記録した CRC-32
    pub fn crc(&self) -> u32 {
        self.crc
    }

    /// 展開した大きさ（記録した値）
    pub fn size(&self) -> u64 {
        self.size
    }

    /// 局所の見出しから、データの終わりまでの範囲
    pub fn span(&self) -> Range<usize> {
        self.header.start..self.data.end
    }

    /// 圧縮したデータの範囲
    pub fn data(&self) -> Range<usize> {
        self.data.clone()
    }
}

/// 読んだ ZIP の書庫
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archive<'a> {
    src: Cow<'a, [u8]>,
    entries: Vec<Entry>,
    depth: u64,
}

impl<'a> Archive<'a> {
    /// 元のバイト列
    pub fn src(&self) -> &[u8] {
        &self.src
    }

    /// 読めた項目。中央ディレクトリの順
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// 名前の合う項目
    pub fn find(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    /// 項目を展開する。展開した大きさ（記録した値）を `decoded-bytes` で数え、CRC-32 を確かめる
    pub fn extract(&self, entry: &Entry, cx: &mut Cx) -> Result<Vec<u8>, Error> {
        let place = || Ref::at_bytes(entry.data.start as u64, entry.data.end as u64);
        let invalid = |cx: &mut Cx, code| {
            cx.report(
                Diagnostic::new(code)
                    .with("name", entry.name.as_str())
                    .at(place()),
            );
            Error::Invalid
        };
        if let Err(stop) = cx.check(Kind::DecodedBytes, entry.size) {
            cx.report(stop.to_diagnostic().at(place()));
            return Err(Error::Stopped(stop));
        }
        let Ok(size) = usize::try_from(entry.size) else {
            return Err(invalid(cx, codes::ZIP_DATA_INVALID));
        };
        let src = self.src.get(entry.data.clone()).unwrap_or_default();
        let data = match entry.method {
            Method::Stored => src.to_vec(),
            Method::Deflate => match inflate::inflate(src, size, cx) {
                Ok(data) => data,
                Err(inflate::Fail::Invalid) => return Err(invalid(cx, codes::ZIP_DATA_INVALID)),
                Err(inflate::Fail::Stop(stop)) => {
                    cx.report(stop.to_diagnostic().at(place()));
                    return Err(Error::Stopped(stop));
                }
            },
        };
        if data.len() != size {
            return Err(invalid(cx, codes::ZIP_DATA_INVALID));
        }
        if crc32(&data) != entry.crc {
            return Err(invalid(cx, codes::ZIP_CRC_INVALID));
        }
        Ok(data)
    }

    /// 書庫の中の書庫を展開して読む。入れ子の段の数を `depth` で数える
    pub fn open(&self, entry: &Entry, cx: &mut Cx) -> Result<Archive<'static>, Error> {
        let depth = self.depth + 1;
        let mut entered = 0;
        let mut result = Ok(());
        while entered < depth {
            if let Err(stop) = cx.enter() {
                cx.report(stop.to_diagnostic().at(Ref::at_bytes(
                    entry.data.start as u64,
                    entry.data.end as u64,
                )));
                result = Err(Error::Stopped(stop));
                break;
            }
            entered += 1;
        }
        let archive = result.and_then(|()| {
            let data = self.extract(entry, cx)?;
            let mut archive = read::directory(Cow::Owned(data), cx)?;
            archive.depth = depth;
            Ok(archive)
        });
        for _ in 0..entered {
            cx.leave();
        }
        archive
    }
}
