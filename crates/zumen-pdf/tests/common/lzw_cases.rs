//! LZW の事前確認を通る入口の合成入力。

#![cfg(test)]

use crate::{common, fonts, resources};

pub const COUNT: usize = 22;

pub fn plain(case: usize) -> Vec<u8> {
    match case {
        0..=2 => b"0 0 m 10 10 l S".to_vec(),
        3 => b"30 0 null".to_vec(),
        4..=8 => resources::plain(case - 4).to_vec(),
        _ => fonts::plain(fonts::KINDS[case - 9]),
    }
}

pub fn document(case: usize, filter: &str, data: &[u8], direct: bool, calls: usize) -> Vec<u8> {
    if case >= 9 {
        return fonts::document(
            fonts::KINDS[case - 9],
            filter,
            data,
            fonts::Uses {
                direct,
                calls,
                ..Default::default()
            },
        );
    }
    if case >= 4 {
        return resources::document(case - 4, filter, data, direct, calls);
    }
    let mut builder = common::Builder::default();
    builder.object(b"<< /Type /Catalog /Pages 2 0 R >>");
    builder.object(b"<< /Type /Pages /Kids [4 0 R] /Count 1 >>");
    let extra = match case {
        2 => "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
        3 => "/Type /ObjStm /N 1 /First 5",
        _ => "",
    };
    let stream = common::stream(&format!("{extra} {filter}"), data);
    builder.object(&stream);
    let value = if direct { stream.as_slice() } else { b"3 0 R" };
    let mut page = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] ".to_vec();
    match case {
        0 => {
            page.extend(b"/Contents ");
            page.extend(value);
        }
        1 => {
            page.extend(b"/Contents [");
            for _ in 0..calls {
                page.extend(value);
                page.push(b' ');
            }
            page.push(b']');
        }
        2 => {
            page.extend(b"/Resources << /XObject << /Fm ");
            page.extend(value);
            page.extend(b" >> >> /Contents ");
            page.extend(common::stream("", "/Fm Do ".repeat(calls)));
        }
        _ => {
            page.extend(b"/Contents ");
            page.extend(common::stream("", "0 0 m 10 10 l S"));
        }
    }
    page.extend(b" >>");
    builder.object(page);
    builder.bytes(1)
}
