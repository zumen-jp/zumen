//! 利用者向けの `zumen` の crate を通す形（決定 0203）。シーンの blob は crate の読み手で読み、応答の数と比べる

use std::cell::RefCell;
use std::rc::Rc;

use zumen::scene::{Column, layout, read_scene};
use zumen::{CancelToken, Response, Session, Status};
use zumen_json::Value;

use crate::Engine;

/// `zumen` の crate の [`Session`] を呼ぶ形。読み手で確かめた違いを `failures` に足す
#[derive(Debug, Default)]
pub struct Wrapper {
    session: Option<Session>,
    /// 直前の応答が返したシーンの blob の番号と、結果の数（`counts`）
    last_scene: Option<(u32, Value)>,
    /// 読み手で確かめたシーンの数
    pub scenes: Rc<RefCell<usize>>,
    /// 読み手で確かめて、期待と違ったこと
    pub failures: Rc<RefCell<Vec<String>>>,
}

fn code(result: Result<(), zumen::Error>) -> i32 {
    match result {
        Ok(()) => Status::Ok.code(),
        Err(error) => error.status.code(),
    }
}

/// 結果の数の欄
fn count(counts: &Value, key: &str) -> usize {
    match counts.pointer(&format!("/{key}")) {
        Some(Value::Number(number)) => number
            .to_safe_integer()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(usize::MAX),
        _ => usize::MAX,
    }
}

impl Wrapper {
    /// 確かめた数と違いを、この形と分け合う新しい形を作る
    pub fn sharing(&self) -> Self {
        Self {
            scenes: self.scenes.clone(),
            failures: self.failures.clone(),
            ..Self::default()
        }
    }

    /// シーンの blob を読み手で読み、応答の数と合うかを確かめる
    fn check_scene(&self, blob: &[u8], counts: &Value) {
        let mut failures = self.failures.borrow_mut();
        let scene = match read_scene(blob) {
            Ok(scene) => scene,
            Err(error) => {
                failures.push(format!("シーンを読めない: {error}"));
                return;
            }
        };
        *self.scenes.borrow_mut() += 1;
        let checks = [
            ("大版", usize::from(scene.major), usize::from(layout::MAJOR)),
            (
                "区画の数",
                usize::try_from(scene.sections).unwrap_or(usize::MAX),
                layout::SECTIONS.len(),
            ),
            (
                "塊の数",
                usize::try_from(scene.chunks).unwrap_or(usize::MAX),
                count(counts, "chunks"),
            ),
            (
                "chunk_origins の行",
                scene.rows(layout::CHUNK_ORIGINS),
                count(counts, "chunks"),
            ),
            (
                "text_xy の行",
                scene.rows(layout::TEXT_XY),
                count(counts, "texts"),
            ),
            (
                "point_xy の行",
                scene.rows(layout::POINT_XY),
                count(counts, "points"),
            ),
            (
                "glyph_xy の行",
                scene.rows(layout::GLYPH_XY),
                count(counts, "glyphs"),
            ),
            (
                "line_xy と face_xy の行",
                scene.rows(layout::LINE_XY) + scene.rows(layout::FACE_XY),
                count(counts, "vertices"),
            ),
        ];
        for (label, actual, expected) in checks {
            if actual != expected {
                failures.push(format!("シーンの{label}が {actual}（期待は {expected}）"));
            }
        }
        // 文字の文字列は、strings の区画から引ける
        if let Some(Column::U32(strings)) = scene.column(layout::TEXT_STRINGS) {
            for (row, pair) in strings.as_chunks::<2>().0.iter().enumerate() {
                if scene.text(pair[0], pair[1]).is_none() {
                    failures.push(format!("text_strings の行 {row} を引けない"));
                }
            }
        }
    }
}

impl Engine for Wrapper {
    fn session_new(&mut self, options: &[u8]) -> i32 {
        self.last_scene = None;
        match Session::open_bytes(options) {
            Ok(session) => {
                self.session = Some(session);
                Status::Ok.code()
            }
            Err(error) => {
                self.session = None;
                error.status.code()
            }
        }
    }

    fn push(&mut self, name: &[u8], data: &[u8]) -> i32 {
        match &mut self.session {
            Some(session) => code(session.push_bytes(name, data)),
            None => Status::Invalid.code(),
        }
    }

    fn call(&mut self, request: &[u8]) -> (i32, Option<Vec<u8>>) {
        self.last_scene = None;
        let Some(session) = &mut self.session else {
            return (Status::Invalid.code(), None);
        };
        let (status, data) = match session.call_bytes(request, &CancelToken::new()) {
            Ok(data) => (Status::Ok, Some(data)),
            Err(error) => (error.status, error.data),
        };
        if let Some(response) = data.as_deref().and_then(|data| Response::parse(data).ok()) {
            let blob = response
                .blobs
                .iter()
                .find(|blob| blob.kind == zumen::messages::BlobKind::Scene);
            let counts = response
                .result
                .as_ref()
                .and_then(|result| result.pointer("/counts"));
            if let (Some(blob), Some(counts)) = (blob, counts) {
                self.last_scene = u32::try_from(blob.id).ok().map(|id| (id, counts.clone()));
            }
        }
        (status.code(), data)
    }

    fn take(&mut self, blob: u32) -> (i32, Option<Vec<u8>>) {
        let Some(session) = &mut self.session else {
            return (Status::Invalid.code(), None);
        };
        match session.take(blob) {
            Ok(data) => {
                if let Some((id, counts)) = &self.last_scene
                    && *id == blob
                {
                    self.check_scene(&data, counts);
                }
                (Status::Ok.code(), Some(data))
            }
            Err(error) => (error.status.code(), None),
        }
    }
}
