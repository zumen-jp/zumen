//! ZIP の書庫を書く。項目は stored で書き、名前は UTF-8 の印を付ける。日時は 1980-01-01 00:00 に固定し、
//! 同じ項目からは同じバイト列を作る。ZIP64 は書かない

use zumen_base::crc::crc32;

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
/// 名前を UTF-8 で書いた印（汎用の目的の旗の 11 番目）
const UTF8: u16 = 1 << 11;
/// MS-DOS の日付の 1980-01-01
const DATE: u16 = (1 << 5) | 1;
/// 展開に要る版（2.0）
const VERSION: u16 = 20;

fn push16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// 項目の名前とデータを、並べた順に書いた書庫。名前が `/` で終われば、フォルダーの項目にする。
/// 項目が 65535 を、名前が 65535 バイトを、書庫が 4 GiB を超えれば、ZIP64 が要るので `None`
pub fn write(entries: &[(&str, &[u8])]) -> Option<Vec<u8>> {
    let count = u16::try_from(entries.len()).ok()?;
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = u32::try_from(out.len()).ok()?;
        let length = u16::try_from(name.len()).ok()?;
        let size = u32::try_from(data.len()).ok()?;
        let crc = crc32(data);
        push32(&mut out, LOCAL);
        for value in [VERSION, UTF8, 0, 0, DATE] {
            push16(&mut out, value);
        }
        for value in [crc, size, size] {
            push32(&mut out, value);
        }
        push16(&mut out, length);
        push16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        push32(&mut central, CENTRAL);
        for value in [VERSION, VERSION, UTF8, 0, 0, DATE] {
            push16(&mut central, value);
        }
        for value in [crc, size, size] {
            push32(&mut central, value);
        }
        for value in [length, 0, 0, 0, 0] {
            push16(&mut central, value);
        }
        // フォルダーなら MS-DOS のフォルダーの属性
        push32(&mut central, if name.ends_with('/') { 0x10 } else { 0 });
        push32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let start = u32::try_from(out.len()).ok()?;
    let size = u32::try_from(central.len()).ok()?;
    start.checked_add(size)?;
    out.extend_from_slice(&central);
    push32(&mut out, END);
    for value in [0, 0, count, count] {
        push16(&mut out, value);
    }
    push32(&mut out, size);
    push32(&mut out, start);
    push16(&mut out, 0);
    Some(out)
}
