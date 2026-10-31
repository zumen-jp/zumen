//! zumen の CLI（決定 0013・0197）。crates.io の zumen-cli で配る（決定 0205）。
//!
//! コアを同じプロセスで動かし、境界と同じ要求と応答（`schema/messages.json`）を運ぶ。応答は書き換えない。
//!
//! - `zumen call`: 要求の JSON をそのまま渡す
//! - `zumen <op> [ファイル]`: よく使う op の短い形。ファイルを入力にして open し、その文書で op を呼ぶ
//! - `zumen serve --stdio`: 境界の関数を、標準入出力の JSON-RPC で出す（`serve.rs`）
//! - `zumen serve --http`: 同じ JSON-RPC を HTTP で受ける（`http.rs`）
//! - `zumen mcp`: MCP のサーバー（`mcp.rs`）
//! - `zumen scene-info`: シーンの blob の区画を要約する（`scene.rs`）

mod host;
mod http;
mod mcp;
mod rpc;
mod scene;
mod serve;

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zumen_abi1::Status;
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

use crate::host::{Host, blobs, str_field};

const USAGE: &str = "\
使い方:
  zumen call [--input 名前=ファイル]... [--options 設定の JSON] [--blobs フォルダー] <要求の JSON のファイル | ->
  zumen <op> [ファイル] [--欄 値]... [--blobs フォルダー]
  zumen serve --stdio
  zumen serve --http [--listen アドレス:ポート]
  zumen mcp [--root フォルダー]
  zumen scene-info <シーンの blob のファイル | ->
  zumen --version

応答の JSON を標準出力に書く。終了コードは境界の状態（0 ok・1 failed・2 invalid・3 busy・4 panicked）で、
引数とファイルの誤りは 64。
<op> の短い形は、ファイルを入力にして open し、その文書を doc に入れて op を呼ぶ。open と capabilities は
それだけを呼ぶ。--欄 の値は JSON として読み、読めなければ文字列にする。v は 1 にする。
--blobs を書くと、応答の blobs を <フォルダー>/<id>.<kind> に書き出す。
serve --http は既定で 127.0.0.1:24180 で待ち受け、待ち受けたアドレスを 1 行目に書く。
scene-info は、scene の op が返すシーンの blob の、区画ごとの行の数を JSON で書く。";

/// 引数とファイルの誤り
const USAGE_ERROR: u8 = 64;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => ExitCode::from(code),
        Err(problem) => {
            let _ = writeln!(io::stderr(), "{problem}");
            ExitCode::from(USAGE_ERROR)
        }
    }
}

fn run(args: &[String]) -> Result<u8, String> {
    let Some((command, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    match command.as_str() {
        "--version" | "version" => {
            write_out(format!("zumen {}\n", env!("CARGO_PKG_VERSION")).as_bytes())?;
            Ok(0)
        }
        "--help" | "help" | "-h" => {
            write_out(format!("{USAGE}\n").as_bytes())?;
            Ok(0)
        }
        "serve" => match rest {
            [flag] if flag == "--stdio" => {
                rpc::serve(serve::Serve::new());
                Ok(0)
            }
            [flag] if flag == "--http" => {
                http::serve(http::DEFAULT_LISTEN, http::Limits::default())?;
                Ok(0)
            }
            [flag, listen, address] if flag == "--http" && listen == "--listen" => {
                http::serve(address, http::Limits::default())?;
                Ok(0)
            }
            _ => Err(USAGE.to_string()),
        },
        "mcp" => {
            let root = match rest {
                [] => None,
                [flag, root] if flag == "--root" => {
                    Some(fs::canonicalize(root).map_err(|e| format!("{root} を開けない: {e}"))?)
                }
                _ => return Err(USAGE.to_string()),
            };
            rpc::serve(mcp::Mcp::new(root, mcp::Limits::default()));
            Ok(0)
        }
        "call" => call(rest),
        "scene-info" => match rest {
            [path] => {
                let mut summary = scene::summary(&read_file(path)?)?;
                summary.push(b'\n');
                write_out(&summary)?;
                Ok(0)
            }
            _ => Err(USAGE.to_string()),
        },
        op if !op.starts_with('-') => short(op, rest),
        _ => Err(USAGE.to_string()),
    }
}

fn write_out(bytes: &[u8]) -> Result<(), String> {
    let mut out = io::stdout().lock();
    out.write_all(bytes)
        .and_then(|()| out.flush())
        .map_err(|e| format!("標準出力に書けない: {e}"))
}

/// `--名前 値` を読む。値がなければ誤り
fn flag_value<'a>(
    args: &mut impl Iterator<Item = &'a String>,
    flag: &str,
) -> Result<&'a String, String> {
    args.next().ok_or(format!("{flag} の値がない\n\n{USAGE}"))
}

fn read_file(path: &str) -> Result<Vec<u8>, String> {
    if path == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|e| format!("標準入力を読めない: {e}"))?;
        return Ok(bytes);
    }
    fs::read(path).map_err(|e| format!("{path} を読めない: {e}"))
}

/// 応答を書き、blobs を書き出し、状態を終了コードにする
fn finish(
    host: &mut Host,
    status: Status,
    response: Option<&[u8]>,
    dir: Option<&Path>,
) -> Result<u8, String> {
    if let Some(response) = response {
        let mut line = response.to_vec();
        line.push(b'\n');
        write_out(&line)?;
        if let Some(dir) = dir {
            fs::create_dir_all(dir).map_err(|e| format!("{} を作れない: {e}", dir.display()))?;
            for (id, kind) in blobs(response) {
                let Some(data) = host.take(id) else { continue };
                let path = dir.join(format!("{id}.{kind}"));
                fs::write(&path, data)
                    .map_err(|e| format!("{} に書けない: {e}", path.display()))?;
            }
        }
    }
    u8::try_from(status.code()).map_err(|_| "状態の値が範囲の外".to_string())
}

fn push(host: &mut Host, name: &str, path: &str) -> Result<(), String> {
    let data = read_file(path)?;
    match host.push(name.as_bytes(), &data) {
        Status::Ok => Ok(()),
        status => Err(format!(
            "入力の名前 {name} を使えない（{}）。/ で区切った相対の名前にする",
            status.name()
        )),
    }
}

fn call(args: &[String]) -> Result<u8, String> {
    let mut inputs = Vec::new();
    let mut options = Vec::new();
    let mut dir = None;
    let mut request = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => {
                let value = flag_value(&mut args, arg)?;
                let (name, path) = value
                    .split_once('=')
                    .ok_or(format!("--input は 名前=ファイル にする: {value}"))?;
                inputs.push((name.to_string(), path.to_string()));
            }
            "--options" => options = flag_value(&mut args, arg)?.as_bytes().to_vec(),
            "--blobs" => dir = Some(PathBuf::from(flag_value(&mut args, arg)?)),
            path if request.is_none() && (path == "-" || !path.starts_with('-')) => {
                request = Some(path.to_string());
            }
            _ => return Err(USAGE.to_string()),
        }
    }
    let request = read_file(&request.ok_or(USAGE.to_string())?)?;
    let mut host = match Host::new(&options) {
        Ok(host) => host,
        Err(status) => return Err(format!("--options を使えない（{}）", status.name())),
    };
    for (name, path) in &inputs {
        push(&mut host, name, path)?;
    }
    let answer = host.call(&request, &CancelToken::new());
    finish(
        &mut host,
        answer.status,
        answer.response.as_deref(),
        dir.as_deref(),
    )
}

/// `--欄` の値。JSON として読めなければ文字列
fn flag_json(text: &str) -> Value {
    zumen_json::parse(text.as_bytes()).unwrap_or_else(|_| Value::String(text.to_string()))
}

fn request(op: &str, fields: &[(&str, Value)]) -> Vec<u8> {
    let mut out = Writer::new();
    out.begin_object();
    out.key("v");
    out.uint(1);
    out.key("op");
    out.string(op);
    for (key, value) in fields {
        out.key(key);
        out.value(value);
    }
    out.end_object();
    out.into_bytes()
}

fn short(op: &str, args: &[String]) -> Result<u8, String> {
    let mut file = None;
    let mut fields: Vec<(String, Value)> = Vec::new();
    let mut dir = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--blobs" {
            dir = Some(PathBuf::from(flag_value(&mut args, arg)?));
        } else if let Some(key) = arg.strip_prefix("--") {
            let value = flag_value(&mut args, arg)?;
            fields.push((key.to_string(), flag_json(value)));
        } else if file.is_none() {
            file = Some(arg.clone());
        } else {
            return Err(USAGE.to_string());
        }
    }
    let fields: Vec<(&str, Value)> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect();
    let mut host =
        Host::new(b"").map_err(|status| format!("セッションを作れない（{}）", status.name()))?;
    let cancel = CancelToken::new();
    let Some(path) = file else {
        let answer = host.call(&request(op, &fields), &cancel);
        return finish(
            &mut host,
            answer.status,
            answer.response.as_deref(),
            dir.as_deref(),
        );
    };
    let name = Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or(format!("{path} はファイルでない"))?;
    push(&mut host, &name, &path)?;
    let mut open_fields = vec![("input", Value::String(name.clone()))];
    if op == "open" {
        open_fields.extend(fields);
        let answer = host.call(&request("open", &open_fields), &cancel);
        return finish(
            &mut host,
            answer.status,
            answer.response.as_deref(),
            dir.as_deref(),
        );
    }
    let opened = host.call(&request("open", &open_fields), &cancel);
    let doc = opened
        .response
        .as_deref()
        .and_then(|response| zumen_json::parse(response).ok())
        .and_then(|response| {
            response
                .pointer("/result")
                .and_then(|r| str_field(r, "doc"))
                .map(str::to_string)
        });
    let Some(doc) = doc.filter(|_| opened.status == Status::Ok) else {
        return finish(&mut host, opened.status, opened.response.as_deref(), None);
    };
    let mut op_fields = vec![("doc", Value::String(doc))];
    op_fields.extend(fields);
    let answer = host.call(&request(op, &op_fields), &cancel);
    finish(
        &mut host,
        answer.status,
        answer.response.as_deref(),
        dir.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::compact;

    #[test]
    fn writes_requests_from_flags() {
        let request = request(
            "entities",
            &[("limit", flag_json("2")), ("from", flag_json("space/0"))],
        );
        assert_eq!(
            request,
            br#"{"v":1,"op":"entities","limit":2,"from":"space/0"}"#
        );
        assert_eq!(compact(&flag_json(r#"["bbox"]"#)), br#"["bbox"]"#);
    }
}
