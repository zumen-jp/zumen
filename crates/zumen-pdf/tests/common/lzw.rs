// 合成テスト用の LZW の符号化。出力の検証には hayro-syntax の展開を使う。

use std::collections::BTreeMap;

#[allow(dead_code)]
#[cfg(test)]
pub fn encode(data: &[u8], early: bool) -> Vec<u8> {
    let initial = || {
        (0..=255u16)
            .map(|b| (vec![b as u8], b))
            .collect::<BTreeMap<_, _>>()
    };
    let mut table = initial();
    let mut next = 258;
    let mut codes = vec![256];
    let mut word = Vec::new();
    for &byte in data {
        let mut joined = word.clone();
        joined.push(byte);
        if table.contains_key(&joined) {
            word = joined;
            continue;
        }
        codes.push(table[&word]);
        if next < 4096 {
            table.insert(joined, next);
            next += 1;
        } else {
            codes.push(256);
            table = initial();
            next = 258;
        }
        word = vec![byte];
    }
    if !word.is_empty() {
        codes.push(table[&word]);
    }
    codes.push(257);
    pack(&codes, early)
}

/// 各符号を PDF の上位ビットからの順で書く。符号を数える側の表の増加に合わせる
#[cfg(test)]
pub fn pack(codes: &[u16], early: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut byte = 0;
    let mut written = 0;
    let mut entries = 258u16;
    let mut previous = false;
    for &code in codes {
        let width = (entries + u16::from(early)).ilog2() + 1;
        for bit in (0..width.min(12)).rev() {
            byte = (byte << 1) | ((code >> bit) & 1) as u8;
            written += 1;
            if written == 8 {
                out.push(byte);
                byte = 0;
                written = 0;
            }
        }
        match code {
            256 => {
                entries = 258;
                previous = false;
            }
            257 => {}
            _ => {
                if previous && entries < 4096 {
                    entries += 1;
                }
                previous = true;
            }
        }
    }
    if written != 0 {
        out.push(byte << (8 - written));
    }
    out
}
