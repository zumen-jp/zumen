//! 図面モデルへ移す数の検査（決定 0012）。入力を検査した後も、加算・角度・単位の換算で有限でなくなりうる。

use std::cell::Cell;

use super::{Lifted, invalid};
use zumen_drawing::{Length, PaperLength, transform::Point};

pub(super) fn finite(kind: &[u8], code: i16, value: f64) -> Lifted<f64> {
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| invalid(kind, code))
}

/// 点と長さの換算を呼んだ所で検査する。出来上がった形をもう一度たどらず、属性は本体と別に検査する。
/// 換算後の X・Y は座標のコード 10・20、長さは 40 で報告する。入力の個別のコードは読み取る側で検査する。
#[derive(Default)]
pub(super) struct Converted {
    code: Cell<Option<i16>>,
}

impl Converted {
    fn number(&self, code: i16, value: f64) {
        if !value.is_finite() && self.code.get().is_none() {
            self.code.set(Some(code));
        }
    }

    pub(super) fn point<P: Point>(&self, point: P) -> P {
        for (code, value) in [10, 20].into_iter().zip(point.xy()) {
            self.number(code, value);
        }
        point
    }

    pub(super) fn length(&self, length: Length) -> Length {
        let value = match length {
            Length::Model(length) => length.value,
            Length::Paper(length) => length.0,
        };
        self.number(40, value);
        length
    }

    pub(super) fn check(&self, kind: &[u8]) -> Lifted<()> {
        self.code
            .get()
            .map_or(Ok(()), |code| Err(invalid(kind, code)))
    }
}

pub(super) fn size(
    kind: &[u8],
    codes: [i16; 2],
    min: [f64; 2],
    max: [f64; 2],
    mm: f64,
) -> Lifted<Option<[PaperLength; 2]>> {
    let size = [
        finite(kind, codes[0], (max[0] - min[0]) * mm)?,
        finite(kind, codes[1], (max[1] - min[1]) * mm)?,
    ];
    Ok(size
        .iter()
        .all(|value| *value > 0.0)
        .then(|| size.map(PaperLength)))
}
