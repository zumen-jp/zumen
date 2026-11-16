//! 保持した診断を、別の明示的な取得予算で JSON に写す。

use zumen_base::{Cx, Diagnostics, Error, Fidelity, Key, Kind, Ref, Severity, Value};
use zumen_json::Writer;

const MAX_BYTES: u64 = 16 << 20;

fn add(total: &mut u64, amount: u64) -> Result<(), Error> {
    *total = total
        .checked_add(amount)
        .filter(|&value| value <= MAX_BYTES)
        .ok_or(Error::Invalid)?;
    Ok(())
}

fn text_size(total: &mut u64, value: &str, cx: &mut Cx<'_>) -> Result<(), Error> {
    cx.check(Kind::Work, 1 + value.len() as u64 / 1024)?;
    add(
        total,
        (value.len() as u64).saturating_mul(6).saturating_add(8),
    )
}

pub(super) fn encode(diagnostics: &Diagnostics, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
    let mut maximum = 128;
    for diagnostic in diagnostics.iter() {
        cx.check(Kind::Work, 1)?;
        add(&mut maximum, 1024)?;
        text_size(&mut maximum, diagnostic.code().name(), cx)?;
        if let Some(spec) = diagnostic.spec() {
            text_size(&mut maximum, spec.spec, cx)?;
            text_size(&mut maximum, spec.section, cx)?;
        }
        for (name, value) in diagnostic.params() {
            cx.check(Kind::Work, 1)?;
            add(&mut maximum, 128)?;
            text_size(&mut maximum, name, cx)?;
            match value {
                Value::Text(text) => text_size(&mut maximum, text, cx)?,
                Value::Key(text) => text_size(&mut maximum, text, cx)?,
                _ => {}
            }
        }
        for place in diagnostic.places() {
            cx.check(Kind::Work, 1)?;
            add(&mut maximum, 256)?;
            if let Some(id) = place.id() {
                text_size(&mut maximum, id, cx)?;
            }
        }
    }
    // Writer の幾何的な容量拡張と、固定の入れ子の管理領域も確保前に数える。
    cx.check(
        Kind::DecodedBytes,
        maximum.saturating_mul(2).saturating_add(128),
    )?;
    let mut out = Writer::new();
    out.begin_array();
    for diagnostic in diagnostics.iter() {
        out.begin_object();
        out.key("code");
        out.string(diagnostic.code().name());
        out.key("severity");
        out.string(match diagnostic.severity() {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        });
        if let Some(fidelity) = diagnostic.fidelity() {
            out.key("fidelity");
            out.string(match fidelity {
                Fidelity::Equivalent => "equivalent",
                Fidelity::Approximated => "approximated",
                Fidelity::Drawn => "drawn",
                Fidelity::Dropped => "dropped",
            });
        }
        out.key("count");
        out.uint(diagnostic.count());
        out.key("params");
        out.begin_object();
        for (name, value) in diagnostic.params() {
            out.key(name);
            match value {
                Value::UInt(value) => out.uint(*value),
                Value::Int(value) => out.int(*value),
                Value::Float(value) => out.float(*value),
                Value::Text(value) => out.string(value),
                Value::Key(value) => out.string(value),
            }
        }
        out.end_object();
        out.key("places");
        out.begin_array();
        for place in diagnostic.places() {
            reference(&mut out, place);
        }
        out.end_array();
        if let Some(spec) = diagnostic.spec() {
            out.key("spec");
            out.begin_object();
            out.key("spec");
            out.string(spec.spec);
            out.key("section");
            out.string(spec.section);
            out.end_object();
        }
        out.end_object();
    }
    out.end_array();
    cx.check(Kind::Work, 0)?;
    if out.len() as u64 > maximum {
        return Err(Error::Invalid);
    }
    Ok(out.into_bytes())
}

fn reference(out: &mut Writer, value: &Ref) {
    out.begin_object();
    if let Some(id) = value.id() {
        out.key("id");
        out.string(id);
    }
    if let Some(record) = value.record() {
        out.key("record");
        out.uint(record);
    }
    if let Some(bytes) = value.bytes() {
        out.key("bytes");
        out.begin_object();
        out.key("start");
        out.uint(bytes.start);
        out.key("end");
        out.uint(bytes.end);
        out.end_object();
    }
    if let Some(key) = value.key() {
        match key {
            Key::Handle(value) => {
                out.key("handle");
                out.uint(value);
            }
            Key::Instance(value) => {
                out.key("instance");
                out.uint(value);
            }
        }
    }
    out.end_object();
}
