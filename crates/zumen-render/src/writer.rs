//! 確保の前に容量を数える出力。書式の一時的な String を作らない。

use std::fmt::{self, Write};
use zumen_base::{Cx, Kind};

use crate::Error;

pub(crate) struct Writer<'c, 'a> {
    pub cx: &'c mut Cx<'a>,
    bytes: Vec<u8>,
    reserved: usize,
    error: Option<Error>,
}

impl<'c, 'a> Writer<'c, 'a> {
    pub fn new(cx: &'c mut Cx<'a>) -> Self {
        Self {
            cx,
            bytes: Vec::new(),
            reserved: 0,
            error: None,
        }
    }

    pub fn text(&mut self, text: &str) -> Result<(), Error> {
        self.cx.check(Kind::Work, 1)?;
        let needed = self
            .bytes
            .len()
            .checked_add(text.len())
            .ok_or(Error::Allocation)?;
        if needed > self.reserved {
            // 小さい追加で全体を何度も写さない。確保量は wasm とネイティブで同じにする。
            let growth = needed
                .saturating_sub(self.reserved)
                .max(self.reserved)
                .max(4096);
            self.cx.check(Kind::DecodedBytes, growth as u64)?;
            let capacity = self.reserved.checked_add(growth).ok_or(Error::Allocation)?;
            // wasm32 の Vec の上限を共通にする。usize の幅で成功・失敗を変えない。
            if capacity > i32::MAX as usize {
                return Err(Error::Allocation);
            }
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|_| Error::Allocation)?;
            self.reserved = capacity;
        }
        self.bytes.extend_from_slice(text.as_bytes());
        Ok(())
    }

    pub fn formatted(&mut self, args: fmt::Arguments<'_>) -> Result<(), Error> {
        self.write_fmt(args)
            .map_err(|_| self.error.take().unwrap_or(Error::Allocation))
    }

    pub fn number(&mut self, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(Error::Options);
        }
        // 負の 0 を同じ綴りにそろえる。Rust の数値書式はロケールを使わない。
        self.formatted(format_args!("{}", if value == 0.0 { 0.0 } else { value }))
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for Writer<'_, '_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.text(text).map_err(|error| {
            self.error = Some(error);
            fmt::Error
        })
    }
}
