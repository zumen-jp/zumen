//! 今の実装（要求ごとに図面の全体を作り直す一括の処理）を、同じプロセスの中で呼ぶ。

use std::fmt::Write as _;

use zumen::json::Value;
use zumen::messages::BlobKind;
use zumen::{CancelToken, Response, Session};

/// セッションの中の文書の名前
const DOC: &str = "a.dxf";

/// 失敗した call
#[derive(Clone, Debug)]
pub struct Failure {
    /// 状態
    pub status: String,
    /// 診断のコード
    pub codes: Vec<String>,
}

impl Failure {
    /// 1 行の説明
    pub fn describe(&self) -> String {
        format!("{} [{}]", self.status, self.codes.join(", "))
    }

    /// 受け付けなかった理由。図面を読んだときの診断（`dxf.`）を除いたもの
    pub fn reason(&self) -> String {
        let mut codes: Vec<&str> = self
            .codes
            .iter()
            .map(String::as_str)
            .filter(|code| !code.starts_with("dxf."))
            .collect();
        codes.dedup();
        format!("{} [{}]", self.status, codes.join(", "))
    }
}

fn failure(error: &zumen::Error) -> Failure {
    Failure {
        status: format!("{:?}", error.status),
        codes: error.response().map_or_else(Vec::new, |response| {
            response
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code.clone())
                .collect()
        }),
    }
}

/// 要素の ID・種類・ハッシュ
#[derive(Clone, Debug)]
pub struct Element {
    pub id: String,
    pub kind: String,
    pub hash: String,
}

/// 要素を動かす変更（ID・要素のハッシュ・行列）
pub type Move = (String, String, [f64; 6]);

/// 要素を動かす変更の JSON
pub fn transform_json(id: &str, hash: &str, m: &[f64; 6]) -> String {
    let mut out = String::new();
    // f64 の表示は、同じ値に読み戻せるいちばん短い 10 進。指数の表記は使わない
    let _ = write!(
        out,
        r#"{{"kind":"transform","id":"{id}","hash":"{hash}","matrix":[{},{},{},{},{},{}]}}"#,
        m[0], m[1], m[2], m[3], m[4], m[5]
    );
    out
}

/// 受け付けられた変更
pub struct Applied {
    /// 変更ごとの、変えた（足した）要素の ID と、新しいハッシュ。除いた要素は、どちらも空
    pub hashes: Vec<(String, String)>,
    /// 書き戻したファイル
    pub file: Option<Vec<u8>>,
}

fn text(value: &Value, pointer: &str) -> String {
    match value.pointer(pointer) {
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

fn items<'a>(response: &'a Response, pointer: &str) -> &'a [Value] {
    match response
        .result
        .as_ref()
        .and_then(|result| result.pointer(pointer))
    {
        Some(Value::Array(items)) => items,
        _ => &[],
    }
}

/// 今の実装のセッションと、開いた 1 つの文書
pub struct Today {
    session: Session,
    cancel: CancelToken,
    /// 要求に足す予算（JSON のオブジェクト）
    budget: Option<String>,
    /// 入力に付けた名前の数
    pushed: usize,
}

impl Today {
    /// セッションを開き、入力を渡す。文書は、まだ開かない
    pub fn start(bytes: &[u8], budget: Option<&str>) -> Result<Today, Failure> {
        let session = Session::open().map_err(|error| failure(&error))?;
        let mut today = Today {
            session,
            cancel: CancelToken::new(),
            budget: budget.map(str::to_owned),
            pushed: 0,
        };
        today
            .session
            .push(DOC, bytes)
            .map_err(|error| failure(&error))?;
        Ok(today)
    }

    /// 渡した入力を文書として開き、入力を閉じる
    pub fn open(&mut self) -> Result<(), Failure> {
        self.call(&format!(
            r#"{{"v":1,"op":"open","input":"{DOC}","doc":"{DOC}"}}"#
        ))?;
        self.call(&format!(r#"{{"v":1,"op":"close","inputs":["{DOC}"]}}"#))?;
        Ok(())
    }

    /// セッションを開き、入力を文書として開く
    pub fn opened(bytes: &[u8], budget: Option<&str>) -> Result<Today, Failure> {
        let mut today = Today::start(bytes, budget)?;
        today.open()?;
        Ok(today)
    }

    fn call(&mut self, request: &str) -> Result<Response, Failure> {
        let request = match &self.budget {
            Some(budget) => request.replacen('{', &format!("{{\"budget\":{budget},"), 1),
            None => request.to_owned(),
        };
        let reply = self
            .session
            .call_bytes(request.as_bytes(), &self.cancel)
            .map_err(|error| failure(&error))?;
        Response::parse(&reply).map_err(|_| Failure {
            status: "応答を読めない".to_owned(),
            codes: Vec::new(),
        })
    }

    /// 応答の blob をすべて取り出す。バイト数の和を返す
    fn drain(&mut self, response: &Response) -> usize {
        let mut bytes = 0;
        for blob in &response.blobs {
            if let Ok(id) = u32::try_from(blob.id)
                && let Ok(data) = self.session.take(id)
            {
                bytes += data.len();
            }
        }
        bytes
    }

    /// モデル空間を表示のデータにする。blob のバイト数を返す
    pub fn scene(&mut self) -> Result<usize, Failure> {
        let response = self.call(&format!(
            r#"{{"v":1,"op":"scene","doc":"{DOC}","from":"space/0"}}"#
        ))?;
        Ok(self.drain(&response))
    }

    /// すべての要素の ID・種類・ハッシュ
    pub fn elements(&mut self) -> Result<Vec<Element>, Failure> {
        let mut out = Vec::new();
        let mut offset = 0u64;
        loop {
            let response = self.call(&format!(
                r#"{{"v":1,"op":"entities","doc":"{DOC}","fields":["kind","hash"],"offset":{offset},"limit":10000}}"#
            ))?;
            for item in items(&response, "/items") {
                out.push(Element {
                    id: text(item, "/id"),
                    kind: text(item, "/kind"),
                    hash: text(item, "/hash"),
                });
            }
            let next = response
                .result
                .as_ref()
                .and_then(|result| result.pointer("/next_offset"));
            match next {
                Some(Value::Number(number)) => {
                    match number
                        .to_safe_integer()
                        .and_then(|next| u64::try_from(next).ok())
                    {
                        Some(next) if next > offset => offset = next,
                        _ => break,
                    }
                }
                _ => break,
            }
        }
        Ok(out)
    }

    /// モデル空間の範囲の要素を、種類・レイヤ・ハッシュ・形とともに選ぶ。件数を返す
    pub fn select(&mut self, [x0, y0, x1, y1]: [f64; 4]) -> Result<usize, Failure> {
        let response = self.call(&format!(
            r#"{{"v":1,"op":"entities","doc":"{DOC}","place":"space/0","region":[{x0},{y0},{x1},{y1}],"fields":["kind","layer","hash","geometry"]}}"#
        ))?;
        Ok(items(&response, "/items").len())
    }

    /// 要素を動かす。受け付けられなければ、文書は変わらない
    pub fn apply(&mut self, moves: &[Move]) -> Result<Applied, Failure> {
        let edits: Vec<String> = moves
            .iter()
            .map(|(id, hash, matrix)| transform_json(id, hash, matrix))
            .collect();
        self.edit(&edits)
    }

    /// 変更を当てる。`edits` は、変更ごとの JSON のオブジェクト。受け付けられなければ、文書は変わらない
    pub fn edit(&mut self, edits: &[String]) -> Result<Applied, Failure> {
        let mut request = format!(r#"{{"v":1,"op":"apply","doc":"{DOC}","edits":["#);
        for (index, edit) in edits.iter().enumerate() {
            if index > 0 {
                request.push(',');
            }
            request.push_str(edit);
        }
        request.push_str("]}");
        let response = self.call(&request)?;
        let hashes = items(&response, "/applied")
            .iter()
            .map(|item| (text(item, "/id"), text(item, "/hash")))
            .collect();
        let file = response
            .blobs
            .iter()
            .find(|blob| matches!(blob.kind, BlobKind::File))
            .and_then(|blob| u32::try_from(blob.id).ok())
            .and_then(|id| self.session.take(id).ok());
        Ok(Applied { hashes, file })
    }

    /// 開き直すためのバイト列を渡す。付けた名前を返す
    pub fn stage(&mut self, bytes: &[u8]) -> Result<String, Failure> {
        self.pushed += 1;
        let name = format!("v{}-{DOC}", self.pushed);
        self.session
            .push(&name, bytes)
            .map_err(|error| failure(&error))?;
        Ok(name)
    }

    /// 文書を、渡してあるバイト列から開き直す（今の実装での取り消し）
    pub fn reopen(&mut self, name: &str) -> Result<(), Failure> {
        self.call(&format!(r#"{{"v":1,"op":"close","docs":["{DOC}"]}}"#))?;
        self.call(&format!(
            r#"{{"v":1,"op":"open","input":"{name}","doc":"{DOC}"}}"#
        ))?;
        self.call(&format!(r#"{{"v":1,"op":"close","inputs":["{name}"]}}"#))?;
        Ok(())
    }

    /// モデル空間の最初の `limit` 件の要素
    pub fn first(&mut self, limit: usize) -> Result<Vec<Element>, Failure> {
        let response = self.call(&format!(
            r#"{{"v":1,"op":"entities","doc":"{DOC}","place":"space/0","fields":["kind","hash"],"offset":0,"limit":{limit}}}"#
        ))?;
        Ok(items(&response, "/items")
            .iter()
            .map(|item| Element {
                id: text(item, "/id"),
                kind: text(item, "/kind"),
                hash: text(item, "/hash"),
            })
            .collect())
    }
}
