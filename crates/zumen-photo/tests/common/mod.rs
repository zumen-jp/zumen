//! テストの JPEG を組み立てる補助（`read.rs`・`external.rs`）。個人情報のない合成のもの。

#![cfg(test)]
#![allow(dead_code)]

/// IFD の項目の値
pub enum V {
    Ascii(&'static str),
    /// ASCII の型で書くバイト列（終わりの NUL は足さない）
    Bytes(&'static [u8]),
    Short(u16),
    Byte(u8),
    Rationals(Vec<(u32, u32)>),
    /// 別の IFD の位置。書くときに決める
    Pointer(usize),
}

/// 型の番号・数・値のバイト列
fn encode(value: &V, little: bool, pointers: &[u32]) -> (u16, u32, Vec<u8>) {
    let u16b = |v: u16| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    let u32b = |v: u32| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    match value {
        V::Ascii(text) => {
            let mut bytes = text.as_bytes().to_vec();
            bytes.push(0);
            (2, bytes.len() as u32, bytes)
        }
        V::Bytes(bytes) => (2, bytes.len() as u32, bytes.to_vec()),
        V::Short(v) => (3, 1, u16b(*v).to_vec()),
        V::Byte(v) => (1, 1, vec![*v]),
        V::Rationals(values) => {
            let mut bytes = Vec::new();
            for (n, d) in values {
                bytes.extend(u32b(*n));
                bytes.extend(u32b(*d));
            }
            (5, values.len() as u32, bytes)
        }
        V::Pointer(index) => (4, 1, u32b(pointers[*index]).to_vec()),
    }
}

/// IFD の列から TIFF を書く。0 番目の IFD を最初に置き、`V::Pointer(i)` は i 番目の IFD を指す
pub fn tiff(ifds: &[Vec<(u16, V)>], little: bool) -> Vec<u8> {
    let u16b = |v: u16| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    let u32b = |v: u32| {
        if little {
            v.to_le_bytes()
        } else {
            v.to_be_bytes()
        }
    };
    // IFD の大きさ（値の外の領域を含む）から位置を決める
    let size = |ifd: &Vec<(u16, V)>| {
        let extra: usize = ifd
            .iter()
            .map(|(_, v)| encode(v, little, &[0; 8]).2.len())
            .filter(|len| *len > 4)
            .sum();
        2 + ifd.len() * 12 + 4 + extra
    };
    let mut pointers = Vec::new();
    let mut at = 8;
    for ifd in ifds {
        pointers.push(at as u32);
        at += size(ifd);
    }
    let mut out = if little {
        b"II*\0".to_vec()
    } else {
        b"MM\0*".to_vec()
    };
    out.extend(u32b(8));
    for (index, ifd) in ifds.iter().enumerate() {
        let start = pointers[index] as usize;
        let mut extra_at = start + 2 + ifd.len() * 12 + 4;
        let mut extra = Vec::new();
        out.extend(u16b(ifd.len() as u16));
        for (tag, value) in ifd {
            let (kind, count, bytes) = encode(value, little, &pointers);
            out.extend(u16b(*tag));
            out.extend(u16b(kind));
            out.extend(u32b(count));
            if bytes.len() <= 4 {
                let mut inline = bytes.clone();
                inline.resize(4, 0);
                out.extend(inline);
            } else {
                out.extend(u32b(extra_at as u32));
                extra_at += bytes.len();
                extra.extend(bytes);
            }
        }
        out.extend(u32b(0));
        out.extend(extra);
    }
    out
}

pub fn segment(marker: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, marker];
    out.extend(((body.len() + 2) as u16).to_be_bytes());
    out.extend(body);
    out
}
