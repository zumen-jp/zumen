//! シーンの blob（`schema/scene.toml` の配置、決定 0137・0172）を読み、区画ごとの行の数を要約する（決定 0200）。
//!
//! 見出しは `ZMSC`、大版（u16）、小版（u16）、塊の数（u32）、区画の数（u32）。続けて区画ごとに、始まりと
//! 長さ（u32、バイト）を置く。数はリトルエンディアン。知らない区画（後の小版で足したもの）は、名前なしで数える。

use zumen_json::Writer;
use zumen_scene::layout::{MAGIC, MAJOR, SECTIONS};

/// 見出しのバイト数（区画の表の前まで）
const HEADER: usize = 16;

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    let raw = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes(raw.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let raw = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes(raw.try_into().ok()?))
}

/// シーンの blob を読み、要約の JSON を返す。配置に合わなければ、理由を返す
pub fn summary(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.get(..4) != Some(&MAGIC[..]) {
        return Err("シーンの blob でない（先頭が ZMSC でない）".to_string());
    }
    let short = || "シーンの blob が途中で切れている".to_string();
    let major = u16_at(bytes, 4).ok_or_else(short)?;
    let minor = u16_at(bytes, 6).ok_or_else(short)?;
    if major != MAJOR {
        return Err(format!(
            "シーンの大版 {major} は読めない（読めるのは {MAJOR}）"
        ));
    }
    let chunks = u32_at(bytes, 8).ok_or_else(short)?;
    let count = u32_at(bytes, 12).ok_or_else(short)?;
    let count = usize::try_from(count).map_err(|_| short())?;
    if count > bytes.len() / 8 {
        return Err(short());
    }
    let mut out = Writer::new();
    out.begin_object();
    out.key("major");
    out.uint(u64::from(major));
    out.key("minor");
    out.uint(u64::from(minor));
    out.key("chunks");
    out.uint(u64::from(chunks));
    out.key("bytes");
    out.uint(bytes.len() as u64);
    out.key("sections");
    out.begin_array();
    for index in 0..count {
        let at = HEADER + index * 8;
        let start = u32_at(bytes, at).ok_or_else(short)? as usize;
        let len = u32_at(bytes, at + 4).ok_or_else(short)? as usize;
        if start.checked_add(len).is_none_or(|end| end > bytes.len()) {
            return Err(format!("区画 {index} がバッファーの外を指す"));
        }
        out.begin_object();
        out.key("index");
        out.uint(index as u64);
        if let Some(section) = SECTIONS.get(index) {
            let row = section.ty.size() * section.width;
            if row == 0 || !len.is_multiple_of(row) {
                return Err(format!(
                    "区画 {} の長さ {len} が行の大きさ {row} で割り切れない",
                    section.name
                ));
            }
            out.key("name");
            out.string(section.name);
            out.key("rows");
            out.uint((len / row) as u64);
        }
        out.key("bytes");
        out.uint(len as u64);
        out.end_object();
    }
    out.end_array();
    out.end_object();
    Ok(out.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_abi1::Session;
    use zumen_base::CancelToken;

    #[test]
    fn summarises_the_scene_of_a_drawing() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n1\n0\nENDSEC\n0\nEOF\n";
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dxf", dxf.as_bytes()).unwrap();
        let cancel = CancelToken::new();
        let opened = session.call(br#"{"v":1,"op":"open","input":"a.dxf"}"#, &cancel);
        let opened = zumen_json::parse(&opened.response).unwrap();
        let Some(zumen_json::Value::String(doc)) = opened.pointer("/result/doc") else {
            panic!("open が文書を返さない");
        };
        let request = format!(r#"{{"v":1,"op":"scene","doc":"{doc}","from":"space/0"}}"#);
        let reply = session.call(request.as_bytes(), &cancel);
        assert_eq!(reply.status, zumen_abi1::Status::Ok);
        let blob = session.take(0).unwrap();
        let summary = zumen_json::parse(&summary(&blob).unwrap()).unwrap();
        assert_eq!(
            summary.pointer("/sections/9/name"),
            Some(&zumen_json::Value::String("line_xy".into()))
        );
        let Some(zumen_json::Value::Number(rows)) = summary.pointer("/sections/9/rows") else {
            panic!("行の数がない");
        };
        assert_eq!(rows.to_safe_integer(), Some(2));
        assert!(super::summary(b"ZMSX").is_err());
        assert!(super::summary(&blob[..20]).is_err());
    }
}
