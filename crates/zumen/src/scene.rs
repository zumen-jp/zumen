//! シーンの blob の読み手（決定 0137・0203）。配置は `schema/scene.toml` にあり、[`layout`] に生成する。
//!
//! 見出しは `ZMSC`、大版（u16）、小版（u16）、塊の数（u32）、区画の数（u32）で、続けて区画ごとに始まりと長さ
//! （u32、バイト）を置く。数はリトルエンディアン。知らない区画（新しい小版で足したもの）は読み飛ばし、ない区画
//! （古い小版）は空の列にする。大版が違えば読まない。

use std::fmt;

pub use zumen_scene::layout;

use layout::{SECTIONS, Type};

/// 見出しのバイト数
const HEADER: usize = 16;
/// 区画の表の 1 行のバイト数
const ENTRY: usize = 8;

/// 区画の値の列。行ごとに区画の幅の数の値を並べる
#[derive(Clone, Debug, PartialEq)]
pub enum Column {
    /// `u8` の列
    U8(Vec<u8>),
    /// `u32` の列
    U32(Vec<u32>),
    /// `f32` の列
    F32(Vec<f32>),
    /// `f64` の列
    F64(Vec<f64>),
}

impl Column {
    /// 値の数
    pub fn len(&self) -> usize {
        match self {
            Self::U8(values) => values.len(),
            Self::U32(values) => values.len(),
            Self::F32(values) => values.len(),
            Self::F64(values) => values.len(),
        }
    }

    /// 値がない
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn read(ty: Type, bytes: &[u8]) -> Self {
        match ty {
            Type::U8 => Self::U8(bytes.to_vec()),
            Type::U32 => Self::U32(
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| u32::from_le_bytes(*b))
                    .collect(),
            ),
            Type::F32 => Self::F32(
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b))
                    .collect(),
            ),
            Type::F64 => Self::F64(
                bytes
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|b| f64::from_le_bytes(*b))
                    .collect(),
            ),
        }
    }

    fn empty(ty: Type) -> Self {
        Self::read(ty, &[])
    }
}

/// シーンの blob を読めない理由
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneError {
    /// シーンの blob でない
    NotScene,
    /// 大版が違う。blob の大版を持つ
    Major(u16),
    /// 区画の表が blob に収まらない
    Table,
    /// 区画の始まりか長さが合わない。区画の名前を持つ
    Section(&'static str),
}

impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotScene => write!(f, "シーンの blob でない"),
            Self::Major(major) => {
                write!(
                    f,
                    "シーンの大版が {major}（この crate は {}）",
                    layout::MAJOR
                )
            }
            Self::Table => write!(f, "区画の表が blob に収まらない"),
            Self::Section(name) => write!(f, "区画 {name} の始まりか長さが合わない"),
        }
    }
}

impl std::error::Error for SceneError {}

/// 読んだシーン
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// 大版
    pub major: u16,
    /// 小版
    pub minor: u16,
    /// 塊の数
    pub chunks: u32,
    /// blob にある区画の数。この crate が知らない区画も数える
    pub sections: u32,
    columns: Vec<Column>,
}

impl Scene {
    /// 区画の列。`section` は [`layout`] の区画の番号（`layout::TEXT_XY` など）
    pub fn column(&self, section: usize) -> Option<&Column> {
        self.columns.get(section)
    }

    /// 名前で区画の列を引く
    pub fn column_named(&self, name: &str) -> Option<&Column> {
        let index = SECTIONS.iter().position(|section| section.name == name)?;
        self.columns.get(index)
    }

    /// 区画の行の数。知らない区画は 0
    pub fn rows(&self, section: usize) -> usize {
        match (SECTIONS.get(section), self.columns.get(section)) {
            (Some(definition), Some(column)) if definition.width > 0 => {
                column.len() / definition.width
            }
            _ => 0,
        }
    }

    /// `strings` の区画から、始まりと長さ（バイト）で文字列を引く。区画の外か UTF-8 でなければ `None`
    pub fn text(&self, start: u32, length: u32) -> Option<&str> {
        let Some(Column::U8(strings)) = self.columns.get(layout::STRINGS) else {
            return None;
        };
        let start = usize::try_from(start).ok()?;
        let end = start.checked_add(usize::try_from(length).ok()?)?;
        std::str::from_utf8(strings.get(start..end)?).ok()
    }
}

fn u16_at(blob: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(blob.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(blob: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(blob.get(at..at + 4)?.try_into().ok()?))
}

/// シーンの blob を読む。列は blob から写す
pub fn read_scene(blob: &[u8]) -> Result<Scene, SceneError> {
    if blob.get(..4) != Some(&layout::MAGIC[..]) {
        return Err(SceneError::NotScene);
    }
    let (Some(major), Some(minor), Some(chunks), Some(sections)) = (
        u16_at(blob, 4),
        u16_at(blob, 6),
        u32_at(blob, 8),
        u32_at(blob, 12),
    ) else {
        return Err(SceneError::NotScene);
    };
    if major != layout::MAJOR {
        return Err(SceneError::Major(major));
    }
    let count = usize::try_from(sections).map_err(|_| SceneError::Table)?;
    let table = count
        .checked_mul(ENTRY)
        .and_then(|size| size.checked_add(HEADER))
        .ok_or(SceneError::Table)?;
    if table > blob.len() {
        return Err(SceneError::Table);
    }
    let mut columns = Vec::with_capacity(SECTIONS.len());
    for (index, section) in SECTIONS.iter().enumerate() {
        if index >= count {
            columns.push(Column::empty(section.ty));
            continue;
        }
        let at = HEADER + ENTRY * index;
        let bad = SceneError::Section(section.name);
        let (Some(start), Some(length)) = (u32_at(blob, at), u32_at(blob, at + 4)) else {
            return Err(bad);
        };
        let start = usize::try_from(start).map_err(|_| bad.clone())?;
        let length = usize::try_from(length).map_err(|_| bad.clone())?;
        let row = section.ty.size() * section.width;
        let Some(bytes) = start
            .checked_add(length)
            .and_then(|end| blob.get(start..end))
        else {
            return Err(bad);
        };
        if row == 0 || length % row != 0 {
            return Err(bad);
        }
        columns.push(Column::read(section.ty, bytes));
    }
    Ok(Scene {
        major,
        minor,
        chunks,
        sections,
        columns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 見出しと、区画ごとに `rows` 行の 0 を並べた blob を作る
    fn blob(sections: usize, rows: usize) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&layout::MAGIC);
        out.extend_from_slice(&layout::MAJOR.to_le_bytes());
        out.extend_from_slice(&layout::MINOR.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&u32::try_from(sections).unwrap().to_le_bytes());
        let mut start = HEADER + ENTRY * sections;
        let mut body = Vec::new();
        for index in 0..sections {
            let length = SECTIONS
                .get(index)
                .map_or(4, |section| section.ty.size() * section.width * rows);
            out.extend_from_slice(&u32::try_from(start).unwrap().to_le_bytes());
            out.extend_from_slice(&u32::try_from(length).unwrap().to_le_bytes());
            body.resize(body.len() + length, 0);
            start += length;
        }
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn reads_rows_of_every_section() {
        let scene = read_scene(&blob(SECTIONS.len(), 2)).unwrap();
        assert_eq!(scene.chunks, 1);
        assert_eq!(scene.rows(layout::CHUNK_ORIGINS), 2);
        assert_eq!(scene.rows(layout::OUTLINE_XY), 2);
        assert_eq!(
            scene.column_named("chunk_origins"),
            Some(&Column::F64(vec![0.0; 4]))
        );
        assert_eq!(scene.text(0, 2), Some("\0\0"));
        assert_eq!(scene.text(u32::MAX, 2), None);
    }

    #[test]
    fn skips_unknown_and_fills_missing_sections() {
        let newer = read_scene(&blob(SECTIONS.len() + 3, 1)).unwrap();
        assert_eq!(newer.sections, u32::try_from(SECTIONS.len() + 3).unwrap());
        assert_eq!(newer.rows(layout::OUTLINE_XY), 1);
        let older = read_scene(&blob(3, 1)).unwrap();
        assert_eq!(older.rows(2), 1);
        assert_eq!(older.rows(layout::OUTLINE_XY), 0);
        assert!(older.column(layout::OUTLINE_XY).unwrap().is_empty());
    }

    #[test]
    fn refuses_other_majors_and_broken_tables() {
        let mut other = blob(SECTIONS.len(), 1);
        other.splice(4..6, (layout::MAJOR + 1).to_le_bytes());
        assert_eq!(
            read_scene(&other),
            Err(SceneError::Major(layout::MAJOR + 1))
        );
        assert_eq!(read_scene(b"ZMSX"), Err(SceneError::NotScene));
        let mut broken = blob(SECTIONS.len(), 1);
        broken.splice(12..16, u32::MAX.to_le_bytes());
        assert_eq!(read_scene(&broken), Err(SceneError::Table));
        // 長さが行の大きさで割り切れない区画
        let mut ragged = blob(SECTIONS.len(), 1);
        ragged.splice(HEADER + 4..HEADER + 8, 3u32.to_le_bytes());
        assert_eq!(
            read_scene(&ragged),
            Err(SceneError::Section("chunk_origins"))
        );
    }

    #[test]
    fn never_panics_on_truncated_or_changed_bytes() {
        let whole = blob(SECTIONS.len(), 1);
        for end in 0..whole.len() {
            let _ = read_scene(whole.get(..end).unwrap());
        }
        for at in 0..HEADER + ENTRY * SECTIONS.len() {
            let mut changed = whole.clone();
            if let Some(byte) = changed.get_mut(at) {
                *byte ^= 0xff;
            }
            let _ = read_scene(&changed);
        }
    }
}
