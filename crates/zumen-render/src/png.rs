//! PNG 第 3 版 5・7・9・10・11、RFC 1950 2.2、RFC 1951 3.2.4。
//! RGBA8 / filter None / stored DEFLATE。外部の圧縮器と可変の一時バッファーを持たない。

use zumen_base::{Cx, Kind};

use crate::{Error, MAX_PNG_PIXELS, Options};

pub(crate) struct Encoder {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    raw: usize,
}

impl Encoder {
    pub fn new(o: &Options, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let count = u64::from(o.width) * u64::from(o.height);
        if count == 0 || count > MAX_PNG_PIXELS {
            return Err(Error::Options);
        }
        let raw = count * 4 + u64::from(o.height);
        let size = raw + raw.div_ceil(65535) * 17 + 80;
        cx.check(Kind::DecodedBytes, size)?;
        if size > i32::MAX as u64 {
            return Err(Error::Allocation);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size as usize)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            bytes,
            width: o.width,
            height: o.height,
            raw: raw as usize,
        })
    }

    pub fn finish(mut self, pixels: &[[u8; 4]], cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        self.bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        let start = self.chunk(b"IHDR", 13);
        self.bytes.extend_from_slice(&self.width.to_be_bytes());
        self.bytes.extend_from_slice(&self.height.to_be_bytes());
        self.bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        self.crc(start, cx)?;
        let start = self.chunk(b"sRGB", 1);
        self.bytes.push(0);
        self.crc(start, cx)?;
        let start = self.chunk(b"gAMA", 4);
        self.bytes.extend_from_slice(&45455_u32.to_be_bytes());
        self.crc(start, cx)?;
        let stride = self.width as usize * 4 + 1;
        let (mut a, mut b) = (1_u32, 0_u32);
        for start in (0..self.raw).step_by(65535) {
            cx.check(Kind::Work, 1)?;
            let length = (self.raw - start).min(65535);
            let last = start + length == self.raw;
            let crc_start = self.chunk(
                b"IDAT",
                (length + 5 + if start == 0 { 2 } else { 0 } + if last { 4 } else { 0 }) as u32,
            );
            if start == 0 {
                self.bytes.extend_from_slice(&[0x78, 0x01]);
            }
            self.bytes.push(u8::from(last)); // BFINAL と BTYPE=00。各ブロックはバイト境界。
            let len = length as u16;
            self.bytes.extend_from_slice(&len.to_le_bytes());
            self.bytes.extend_from_slice(&(!len).to_le_bytes());
            for at in start..start + length {
                cx.check(Kind::Work, 1)?;
                let column = at % stride;
                let byte = if column == 0 {
                    0
                } else {
                    let index = (at / stride) * self.width as usize + (column - 1) / 4;
                    *pixels
                        .get(index)
                        .and_then(|p| p.get((column - 1) % 4))
                        .ok_or(Error::Scene)?
                };
                self.bytes.push(byte);
                a = (a + u32::from(byte)) % 65521;
                b = (b + a) % 65521;
            }
            if last {
                self.bytes.extend_from_slice(&((b << 16) | a).to_be_bytes());
            }
            self.crc(crc_start, cx)?;
        }
        let start = self.chunk(b"IEND", 0);
        self.crc(start, cx)?;
        Ok(self.bytes)
    }

    fn chunk(&mut self, name: &[u8; 4], length: u32) -> usize {
        self.bytes.extend_from_slice(&length.to_be_bytes());
        let start = self.bytes.len();
        self.bytes.extend_from_slice(name);
        start
    }

    fn crc(&mut self, start: usize, cx: &mut Cx<'_>) -> Result<(), Error> {
        let mut state = u32::MAX;
        for byte in self.bytes.get(start..).ok_or(Error::Scene)? {
            cx.check(Kind::Work, 1)?;
            state ^= u32::from(*byte);
            for _ in 0..8 {
                state = (state >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(state & 1));
            }
        }
        self.bytes.extend_from_slice(&(!state).to_be_bytes());
        Ok(())
    }
}
