//! LAZ 1.0〜1.3（項目バージョン 1・2、逐次圧縮）の項目復号器。

use zumen_base::Kind;
use zumen_sdk::{Cx, Error};

use super::coder::{
    IntDecoder, LazErr, LazResult, Median5, RangeDecoder, SliceCursor, SymbolModel, clamp_u8,
};

const NUMBER_RETURN_MAP: [[u8; 8]; 8] = [
    [15, 14, 13, 12, 11, 10, 9, 8],
    [14, 0, 1, 3, 6, 10, 10, 9],
    [13, 1, 2, 4, 7, 11, 11, 10],
    [12, 3, 4, 5, 8, 12, 12, 11],
    [11, 6, 7, 8, 9, 13, 13, 12],
    [10, 10, 11, 12, 13, 14, 14, 13],
    [9, 10, 11, 12, 13, 14, 15, 14],
    [8, 9, 10, 11, 12, 13, 14, 15],
];

const GPS_MULTI: i32 = 500;
const GPS_MULTI_MINUS: i32 = -10;
const GPS_MULTI_UNCHANGED: i32 = GPS_MULTI - GPS_MULTI_MINUS + 1; // 511
const GPS_MULTI_CODE_FULL: i32 = GPS_MULTI - GPS_MULTI_MINUS + 2; // 512
const GPS_MULTI_TOTAL: u32 = (GPS_MULTI - GPS_MULTI_MINUS + 6) as u32; // 516

#[derive(Clone, Copy, Debug, Default)]
struct Point0 {
    x: i32,
    y: i32,
    z: i32,
    intensity: u16,
    bit_fields: u8,
    classification: u8,
    scan_angle_rank: i8,
    user_data: u8,
    point_source_id: u16,
}

impl Point0 {
    const SIZE: usize = 20;

    fn unpack(buf: &[u8]) -> LazResult<Self> {
        let x = i32::from_le_bytes(
            buf.get(0..4)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let y = i32::from_le_bytes(
            buf.get(4..8)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let z = i32::from_le_bytes(
            buf.get(8..12)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let intensity = u16::from_le_bytes(
            buf.get(12..14)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let bit_fields = *buf.get(14).ok_or(LazErr)?;
        let classification = *buf.get(15).ok_or(LazErr)?;
        let scan_angle_rank = *buf.get(16).ok_or(LazErr)? as i8;
        let user_data = *buf.get(17).ok_or(LazErr)?;
        let point_source_id = u16::from_le_bytes(
            buf.get(18..20)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        Ok(Self {
            x,
            y,
            z,
            intensity,
            bit_fields,
            classification,
            scan_angle_rank,
            user_data,
            point_source_id,
        })
    }

    fn pack(&self, out: &mut [u8]) -> LazResult<()> {
        out.get_mut(0..4)
            .ok_or(LazErr)?
            .copy_from_slice(&self.x.to_le_bytes());
        out.get_mut(4..8)
            .ok_or(LazErr)?
            .copy_from_slice(&self.y.to_le_bytes());
        out.get_mut(8..12)
            .ok_or(LazErr)?
            .copy_from_slice(&self.z.to_le_bytes());
        out.get_mut(12..14)
            .ok_or(LazErr)?
            .copy_from_slice(&self.intensity.to_le_bytes());
        *out.get_mut(14).ok_or(LazErr)? = self.bit_fields;
        *out.get_mut(15).ok_or(LazErr)? = self.classification;
        *out.get_mut(16).ok_or(LazErr)? = self.scan_angle_rank as u8;
        *out.get_mut(17).ok_or(LazErr)? = self.user_data;
        out.get_mut(18..20)
            .ok_or(LazErr)?
            .copy_from_slice(&self.point_source_id.to_le_bytes());
        Ok(())
    }

    fn return_number(&self) -> usize {
        usize::from(self.bit_fields & 0x07)
    }

    fn number_of_returns(&self) -> usize {
        usize::from((self.bit_fields >> 3) & 0x07)
    }

    fn scan_direction(&self) -> usize {
        usize::from((self.bit_fields >> 6) & 0x01)
    }
}

fn median3(arr: &[i32; 3]) -> i32 {
    let [a, b, c] = *arr;
    if a < b {
        if b < c {
            b
        } else if a < c {
            c
        } else {
            a
        }
    } else if a < c {
        a
    } else if b < c {
        c
    } else {
        b
    }
}

struct Point10V1 {
    last: Point0,
    last_x_diffs: [i32; 3],
    last_y_diffs: [i32; 3],
    last_incr: usize,
    ic_dx: IntDecoder,
    ic_dy: IntDecoder,
    ic_dz: IntDecoder,
    ic_intensity: IntDecoder,
    ic_scan_angle: IntDecoder,
    ic_point_source: IntDecoder,
    changed_values: SymbolModel,
    bit_byte: Vec<Option<SymbolModel>>,
    classification: Vec<Option<SymbolModel>>,
    user_data: Vec<Option<SymbolModel>>,
}

impl Point10V1 {
    fn new() -> Self {
        Self {
            last: Point0::default(),
            last_x_diffs: [0; 3],
            last_y_diffs: [0; 3],
            last_incr: 0,
            ic_dx: IntDecoder::new(32, 1),
            ic_dy: IntDecoder::new(32, 20),
            ic_dz: IntDecoder::new(32, 20),
            ic_intensity: IntDecoder::new(16, 1),
            ic_scan_angle: IntDecoder::new(8, 2),
            ic_point_source: IntDecoder::new(16, 1),
            changed_values: SymbolModel::new(64),
            bit_byte: vec![None; 256],
            classification: vec![None; 256],
            user_data: vec![None; 256],
        }
    }

    fn init(&mut self, cursor: &mut SliceCursor<'_>, out: &mut [u8]) -> LazResult<()> {
        cursor.read_into(out)?;
        self.last = Point0::unpack(out)?;
        Ok(())
    }

    fn decode(&mut self, dec: &mut RangeDecoder<'_>, out: &mut [u8]) -> LazResult<()> {
        let med_x = median3(&self.last_x_diffs);
        let med_y = median3(&self.last_y_diffs);
        let x_diff = self.ic_dx.decode(dec, med_x, 0)?;
        self.last.x = self.last.x.wrapping_add(x_diff);
        let k_dx = self.ic_dx.k();
        let y_diff = self.ic_dy.decode(dec, med_y, k_dx.min(19))?;
        self.last.y = self.last.y.wrapping_add(y_diff);
        let k_avg = (k_dx + self.ic_dy.k()) / 2;
        self.last.z = self.ic_dz.decode(dec, self.last.z, k_avg.min(19))?;

        let changed = dec.decode_symbol(&mut self.changed_values)?;
        if changed != 0 {
            if (changed & 32) != 0 {
                self.last.intensity =
                    self.ic_intensity
                        .decode(dec, i32::from(self.last.intensity), 0)? as u16;
            }
            if (changed & 16) != 0 {
                let model = self
                    .bit_byte
                    .get_mut(usize::from(self.last.bit_fields))
                    .ok_or(LazErr)?
                    .get_or_insert_with(|| SymbolModel::new(256));
                self.last.bit_fields = dec.decode_symbol(model)? as u8;
            }
            if (changed & 8) != 0 {
                let model = self
                    .classification
                    .get_mut(usize::from(self.last.classification))
                    .ok_or(LazErr)?
                    .get_or_insert_with(|| SymbolModel::new(256));
                self.last.classification = dec.decode_symbol(model)? as u8;
            }
            if (changed & 4) != 0 {
                self.last.scan_angle_rank = self.ic_scan_angle.decode(
                    dec,
                    i32::from(self.last.scan_angle_rank),
                    u32::from(k_avg < 3),
                )? as i8;
            }
            if (changed & 2) != 0 {
                let model = self
                    .user_data
                    .get_mut(usize::from(self.last.user_data))
                    .ok_or(LazErr)?
                    .get_or_insert_with(|| SymbolModel::new(256));
                self.last.user_data = dec.decode_symbol(model)? as u8;
            }
            if (changed & 1) != 0 {
                self.last.point_source_id =
                    self.ic_point_source
                        .decode(dec, i32::from(self.last.point_source_id), 0)?
                        as u16;
            }
        }
        if let Some(slot) = self.last_x_diffs.get_mut(self.last_incr) {
            *slot = x_diff;
        }
        if let Some(slot) = self.last_y_diffs.get_mut(self.last_incr) {
            *slot = y_diff;
        }
        self.last_incr = (self.last_incr + 1) % 3;
        self.last.pack(out)
    }
}

struct Point10V2 {
    last: Point0,
    last_intensity: [u16; 16],
    last_x_diff_median: [Median5; 16],
    last_y_diff_median: [Median5; 16],
    last_height: [i32; 8],
    changed_values: SymbolModel,
    scan_angle_rank: [SymbolModel; 2],
    bit_byte: Vec<SymbolModel>,
    classification: Vec<SymbolModel>,
    user_data: Vec<SymbolModel>,
    ic_intensity: IntDecoder,
    ic_point_source: IntDecoder,
    ic_dx: IntDecoder,
    ic_dy: IntDecoder,
    ic_z: IntDecoder,
}

impl Point10V2 {
    fn new() -> Self {
        Self {
            last: Point0::default(),
            last_intensity: [0; 16],
            last_x_diff_median: [Median5::default(); 16],
            last_y_diff_median: [Median5::default(); 16],
            last_height: [0; 8],
            changed_values: SymbolModel::new(64),
            scan_angle_rank: [SymbolModel::new(256), SymbolModel::new(256)],
            bit_byte: (0..256).map(|_| SymbolModel::new(256)).collect(),
            classification: (0..256).map(|_| SymbolModel::new(256)).collect(),
            user_data: (0..256).map(|_| SymbolModel::new(256)).collect(),
            ic_intensity: IntDecoder::new(16, 4),
            ic_point_source: IntDecoder::new(16, 1),
            ic_dx: IntDecoder::new(32, 2),
            ic_dy: IntDecoder::new(32, 22),
            ic_z: IntDecoder::new(32, 20),
        }
    }

    fn init(&mut self, cursor: &mut SliceCursor<'_>, out: &mut [u8]) -> LazResult<()> {
        cursor.read_into(out)?;
        self.last = Point0::unpack(out)?;
        self.last.intensity = 0;
        Ok(())
    }

    fn decode(&mut self, dec: &mut RangeDecoder<'_>, out: &mut [u8]) -> LazResult<()> {
        let changed = dec.decode_symbol(&mut self.changed_values)?;
        let (r, n, m, l);
        if changed != 0 {
            if (changed & 32) != 0 {
                let model = self
                    .bit_byte
                    .get_mut(usize::from(self.last.bit_fields))
                    .ok_or(LazErr)?;
                self.last.bit_fields = dec.decode_symbol(model)? as u8;
            }
            r = self.last.return_number();
            n = self.last.number_of_returns();
            m = usize::from(
                *NUMBER_RETURN_MAP
                    .get(n)
                    .and_then(|row| row.get(r))
                    .ok_or(LazErr)?,
            );
            l = n.abs_diff(r);

            let prev_int = *self.last_intensity.get(m).ok_or(LazErr)?;
            if (changed & 16) != 0 {
                let ctx = (m as u32).min(3);
                let val = self.ic_intensity.decode(dec, i32::from(prev_int), ctx)? as u16;
                self.last.intensity = val;
                *self.last_intensity.get_mut(m).ok_or(LazErr)? = val;
            } else {
                self.last.intensity = prev_int;
            }

            if (changed & 8) != 0 {
                let model = self
                    .classification
                    .get_mut(usize::from(self.last.classification))
                    .ok_or(LazErr)?;
                self.last.classification = dec.decode_symbol(model)? as u8;
            }
            if (changed & 4) != 0 {
                let model = self
                    .scan_angle_rank
                    .get_mut(self.last.scan_direction())
                    .ok_or(LazErr)?;
                let delta = dec.decode_symbol(model)? as i8;
                self.last.scan_angle_rank = self.last.scan_angle_rank.wrapping_add(delta);
            }
            if (changed & 2) != 0 {
                let model = self
                    .user_data
                    .get_mut(usize::from(self.last.user_data))
                    .ok_or(LazErr)?;
                self.last.user_data = dec.decode_symbol(model)? as u8;
            }
            if (changed & 1) != 0 {
                self.last.point_source_id =
                    self.ic_point_source
                        .decode(dec, i32::from(self.last.point_source_id), 0)?
                        as u16;
            }
        } else {
            r = self.last.return_number();
            n = self.last.number_of_returns();
            m = usize::from(
                *NUMBER_RETURN_MAP
                    .get(n)
                    .and_then(|row| row.get(r))
                    .ok_or(LazErr)?,
            );
            l = n.abs_diff(r);
        }

        let med_x = self.last_x_diff_median.get(m).ok_or(LazErr)?.get();
        let dx = self.ic_dx.decode(dec, med_x, u32::from(n == 1))?;
        self.last.x = self.last.x.wrapping_add(dx);
        self.last_x_diff_median.get_mut(m).ok_or(LazErr)?.add(dx);

        let med_y = self.last_y_diff_median.get(m).ok_or(LazErr)?.get();
        let k_dx = self.ic_dx.k();
        let ctx_y = u32::from(n == 1) + if k_dx < 20 { k_dx & !1 } else { 20 };
        let dy = self.ic_dy.decode(dec, med_y, ctx_y)?;
        self.last.y = self.last.y.wrapping_add(dy);
        self.last_y_diff_median.get_mut(m).ok_or(LazErr)?.add(dy);

        let k_z = (k_dx + self.ic_dy.k()) / 2;
        let ctx_z = u32::from(n == 1) + if k_z < 18 { k_z & !1 } else { 18 };
        let prev_h = *self.last_height.get(l).ok_or(LazErr)?;
        let z = self.ic_z.decode(dec, prev_h, ctx_z)?;
        self.last.z = z;
        *self.last_height.get_mut(l).ok_or(LazErr)? = z;

        self.last.pack(out)
    }
}

struct GpsTimeV1 {
    last_gps: i64,
    last_diff: i32,
    extreme_counter: i32,
    multi_model: SymbolModel,
    zero_diff_model: SymbolModel,
    ic_gps: IntDecoder,
}

impl GpsTimeV1 {
    const MULTI_MAX: u32 = 512;

    fn new() -> Self {
        Self {
            last_gps: 0,
            last_diff: 0,
            extreme_counter: 0,
            multi_model: SymbolModel::new(Self::MULTI_MAX),
            zero_diff_model: SymbolModel::new(3),
            ic_gps: IntDecoder::new(32, 6),
        }
    }

    fn init(&mut self, cursor: &mut SliceCursor<'_>, out: &mut [u8]) -> LazResult<()> {
        cursor.read_into(out)?;
        let arr: [u8; 8] = out.try_into().map_err(|_| LazErr)?;
        self.last_gps = i64::from_le_bytes(arr);
        Ok(())
    }

    fn decode(&mut self, dec: &mut RangeDecoder<'_>, out: &mut [u8]) -> LazResult<()> {
        if self.last_diff == 0 {
            let multi = dec.decode_symbol(&mut self.zero_diff_model)?;
            if multi == 1 {
                self.last_diff = self.ic_gps.decode(dec, 0, 0)?;
                self.last_gps = self.last_gps.wrapping_add(i64::from(self.last_diff));
            } else if multi == 2 {
                self.last_gps = dec.read_u64()? as i64;
            }
        } else {
            let multi = dec.decode_symbol(&mut self.multi_model)?;
            if multi < Self::MULTI_MAX - 2 {
                let diff = if multi == 1 {
                    let d = self.ic_gps.decode(dec, self.last_diff, 1)?;
                    self.last_diff = d;
                    self.extreme_counter = 0;
                    d
                } else if multi == 0 {
                    let d = self.ic_gps.decode(dec, self.last_diff / 4, 2)?;
                    self.extreme_counter += 1;
                    if self.extreme_counter > 3 {
                        self.last_diff = d;
                        self.extreme_counter = 0;
                    }
                    d
                } else {
                    let ctx = if multi < 10 {
                        3
                    } else if multi < 50 {
                        4
                    } else {
                        5
                    };
                    let d =
                        self.ic_gps
                            .decode(dec, self.last_diff.wrapping_mul(multi as i32), ctx)?;
                    if multi == Self::MULTI_MAX - 3 {
                        self.extreme_counter += 1;
                        if self.extreme_counter > 3 {
                            self.last_diff = d;
                            self.extreme_counter = 0;
                        }
                    }
                    d
                };
                self.last_gps = self.last_gps.wrapping_add(i64::from(diff));
            } else if multi < Self::MULTI_MAX - 1 {
                self.last_gps = dec.read_u64()? as i64;
            }
        }
        out.copy_from_slice(&self.last_gps.to_le_bytes());
        Ok(())
    }
}

struct GpsTimeV2 {
    last: usize,
    next: usize,
    last_times: [i64; 4],
    last_diffs: [i32; 4],
    extreme_counters: [i32; 4],
    multi_model: SymbolModel,
    zero_diff_model: SymbolModel,
    ic_gps: IntDecoder,
}

impl GpsTimeV2 {
    fn new() -> Self {
        Self {
            last: 0,
            next: 0,
            last_times: [0; 4],
            last_diffs: [0; 4],
            extreme_counters: [0; 4],
            multi_model: SymbolModel::new(GPS_MULTI_TOTAL),
            zero_diff_model: SymbolModel::new(6),
            ic_gps: IntDecoder::new(32, 9),
        }
    }

    fn init(&mut self, cursor: &mut SliceCursor<'_>, out: &mut [u8]) -> LazResult<()> {
        cursor.read_into(out)?;
        let arr: [u8; 8] = out.try_into().map_err(|_| LazErr)?;
        self.last_times[0] = i64::from_le_bytes(arr);
        Ok(())
    }

    fn decode(&mut self, dec: &mut RangeDecoder<'_>, out: &mut [u8]) -> LazResult<()> {
        // 系列切り替え（4 系列）は通常 1 回で終わる。細工された無限再帰を防ぐため上限を設ける。
        let mut decoded = false;
        for _ in 0..8 {
            let cur_diff = *self.last_diffs.get(self.last).ok_or(LazErr)?;
            if cur_diff == 0 {
                let multi = dec.decode_symbol(&mut self.zero_diff_model)? as i32;
                if multi == 0 {
                    decoded = true;
                    break;
                } else if multi == 1 {
                    let d = self.ic_gps.decode(dec, 0, 0)?;
                    *self.last_diffs.get_mut(self.last).ok_or(LazErr)? = d;
                    let t = self.last_times.get_mut(self.last).ok_or(LazErr)?;
                    *t = t.wrapping_add(i64::from(d));
                    *self.extreme_counters.get_mut(self.last).ok_or(LazErr)? = 0;
                    decoded = true;
                    break;
                } else if multi == 2 {
                    self.next = (self.next + 1) & 3;
                    let prev_hi = (*self.last_times.get(self.last).ok_or(LazErr)? >> 32) as i32;
                    let hi = i64::from(self.ic_gps.decode(dec, prev_hi, 8)?);
                    let lo = i64::from(dec.read_u32()?);
                    *self.last_times.get_mut(self.next).ok_or(LazErr)? = (hi << 32) | lo;
                    self.last = self.next;
                    *self.last_diffs.get_mut(self.last).ok_or(LazErr)? = 0;
                    *self.extreme_counters.get_mut(self.last).ok_or(LazErr)? = 0;
                    decoded = true;
                    break;
                } else {
                    self.last = (self.last + (multi as usize) - 2) & 3;
                }
            } else {
                let multi = dec.decode_symbol(&mut self.multi_model)? as i32;
                if multi == 1 {
                    let d = self.ic_gps.decode(dec, cur_diff, 1)?;
                    let t = self.last_times.get_mut(self.last).ok_or(LazErr)?;
                    *t = t.wrapping_add(i64::from(d));
                    *self.extreme_counters.get_mut(self.last).ok_or(LazErr)? = 0;
                    decoded = true;
                    break;
                } else if multi < GPS_MULTI_UNCHANGED {
                    let diff = if multi == 0 {
                        let d = self.ic_gps.decode(dec, 0, 7)?;
                        let ec = self.extreme_counters.get_mut(self.last).ok_or(LazErr)?;
                        *ec += 1;
                        if *ec > 3 {
                            *ec = 0;
                            *self.last_diffs.get_mut(self.last).ok_or(LazErr)? = d;
                        }
                        d
                    } else if multi < GPS_MULTI {
                        let ctx = if multi < 10 { 2 } else { 3 };
                        self.ic_gps.decode(dec, multi.wrapping_mul(cur_diff), ctx)?
                    } else if multi == GPS_MULTI {
                        let d = self.ic_gps.decode(dec, multi.wrapping_mul(cur_diff), 4)?;
                        let ec = self.extreme_counters.get_mut(self.last).ok_or(LazErr)?;
                        *ec += 1;
                        if *ec > 3 {
                            *ec = 0;
                            *self.last_diffs.get_mut(self.last).ok_or(LazErr)? = d;
                        }
                        d
                    } else {
                        let neg = GPS_MULTI - multi;
                        if neg > GPS_MULTI_MINUS {
                            self.ic_gps.decode(dec, neg.wrapping_mul(cur_diff), 5)?
                        } else {
                            let d = self.ic_gps.decode(
                                dec,
                                GPS_MULTI_MINUS.wrapping_mul(cur_diff),
                                6,
                            )?;
                            let ec = self.extreme_counters.get_mut(self.last).ok_or(LazErr)?;
                            *ec += 1;
                            if *ec > 3 {
                                *ec = 0;
                                *self.last_diffs.get_mut(self.last).ok_or(LazErr)? = d;
                            }
                            d
                        }
                    };
                    let t = self.last_times.get_mut(self.last).ok_or(LazErr)?;
                    *t = t.wrapping_add(i64::from(diff));
                    decoded = true;
                    break;
                } else if multi == GPS_MULTI_UNCHANGED {
                    decoded = true;
                    break;
                } else if multi == GPS_MULTI_CODE_FULL {
                    self.next = (self.next + 1) & 3;
                    let prev_hi = (*self.last_times.get(self.last).ok_or(LazErr)? >> 32) as i32;
                    let hi = i64::from(self.ic_gps.decode(dec, prev_hi, 8)?);
                    let lo = i64::from(dec.read_u32()?);
                    *self.last_times.get_mut(self.next).ok_or(LazErr)? = (hi << 32) | lo;
                    self.last = self.next;
                    *self.last_diffs.get_mut(self.last).ok_or(LazErr)? = 0;
                    *self.extreme_counters.get_mut(self.last).ok_or(LazErr)? = 0;
                    decoded = true;
                    break;
                } else {
                    self.last = (self.last + (multi as usize) - (GPS_MULTI_CODE_FULL as usize)) & 3;
                }
            }
        }
        if !decoded {
            return Err(LazErr);
        }
        let val = *self.last_times.get(self.last).ok_or(LazErr)?;
        out.copy_from_slice(&val.to_le_bytes());
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RgbValue {
    pub(super) r: u16,
    pub(super) g: u16,
    pub(super) b: u16,
}

impl RgbValue {
    pub(super) const SIZE: usize = 6;

    pub(super) fn unpack(buf: &[u8]) -> LazResult<Self> {
        let r = u16::from_le_bytes(
            buf.get(0..2)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let g = u16::from_le_bytes(
            buf.get(2..4)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let b = u16::from_le_bytes(
            buf.get(4..6)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        Ok(Self { r, g, b })
    }

    pub(super) fn pack(&self, out: &mut [u8]) -> LazResult<()> {
        out.get_mut(0..2)
            .ok_or(LazErr)?
            .copy_from_slice(&self.r.to_le_bytes());
        out.get_mut(2..4)
            .ok_or(LazErr)?
            .copy_from_slice(&self.g.to_le_bytes());
        out.get_mut(4..6)
            .ok_or(LazErr)?
            .copy_from_slice(&self.b.to_le_bytes());
        Ok(())
    }
}

struct Rgb12V1 {
    last: RgbValue,
    byte_used: SymbolModel,
    ic: IntDecoder,
}

impl Rgb12V1 {
    fn new() -> Self {
        Self {
            last: RgbValue::default(),
            byte_used: SymbolModel::new(64),
            ic: IntDecoder::new(8, 6),
        }
    }

    fn init(&mut self, cursor: &mut SliceCursor<'_>, out: &mut [u8]) -> LazResult<()> {
        cursor.read_into(out)?;
        self.last = RgbValue::unpack(out)?;
        Ok(())
    }

    fn decode(&mut self, dec: &mut RangeDecoder<'_>, out: &mut [u8]) -> LazResult<()> {
        let sym = dec.decode_symbol(&mut self.byte_used)?;
        if (sym & 1) != 0 {
            let lo = self
                .ic
                .decode(dec, i32::from((self.last.r & 0xFF) as u8), 0)? as u16;
            self.last.r = (self.last.r & 0xFF00) | (lo & 0xFF);
        }
        if (sym & 2) != 0 {
            let hi = self
                .ic
                .decode(dec, i32::from((self.last.r >> 8) as u8), 1)? as u16;
            self.last.r = (self.last.r & 0x00FF) | ((hi & 0xFF) << 8);
        }
        if (sym & 4) != 0 {
            let lo = self
                .ic
                .decode(dec, i32::from((self.last.g & 0xFF) as u8), 2)? as u16;
            self.last.g = (self.last.g & 0xFF00) | (lo & 0xFF);
        }
        if (sym & 8) != 0 {
            let hi = self
                .ic
                .decode(dec, i32::from((self.last.g >> 8) as u8), 3)? as u16;
            self.last.g = (self.last.g & 0x00FF) | ((hi & 0xFF) << 8);
        }
        if (sym & 16) != 0 {
            let lo = self
                .ic
                .decode(dec, i32::from((self.last.b & 0xFF) as u8), 4)? as u16;
            self.last.b = (self.last.b & 0xFF00) | (lo & 0xFF);
        }
        if (sym & 32) != 0 {
            let hi = self
                .ic
                .decode(dec, i32::from((self.last.b >> 8) as u8), 5)? as u16;
            self.last.b = (self.last.b & 0x00FF) | ((hi & 0xFF) << 8);
        }
        self.last.pack(out)
    }
}

#[derive(Clone, Debug)]
pub(super) struct RgbModelsV2 {
    byte_used: SymbolModel,
    lo_r: SymbolModel,
    hi_r: SymbolModel,
    lo_g: SymbolModel,
    hi_g: SymbolModel,
    lo_b: SymbolModel,
    hi_b: SymbolModel,
}

impl Default for RgbModelsV2 {
    fn default() -> Self {
        Self {
            byte_used: SymbolModel::new(128),
            lo_r: SymbolModel::new(256),
            hi_r: SymbolModel::new(256),
            lo_g: SymbolModel::new(256),
            hi_g: SymbolModel::new(256),
            lo_b: SymbolModel::new(256),
            hi_b: SymbolModel::new(256),
        }
    }
}

pub(super) fn decode_rgb_v2(
    dec: &mut RangeDecoder<'_>,
    models: &mut RgbModelsV2,
    last: RgbValue,
) -> LazResult<RgbValue> {
    let sym = dec.decode_symbol(&mut models.byte_used)?;
    let mut cur = RgbValue::default();
    let last_r_lo = (last.r & 0xFF) as u8;
    let last_r_hi = (last.r >> 8) as u8;
    let last_g_lo = (last.g & 0xFF) as u8;
    let last_g_hi = (last.g >> 8) as u8;
    let last_b_lo = (last.b & 0xFF) as u8;
    let last_b_hi = (last.b >> 8) as u8;

    if (sym & 1) != 0 {
        let corr = dec.decode_symbol(&mut models.lo_r)? as u8;
        cur.r = u16::from(corr.wrapping_add(last_r_lo));
    } else {
        cur.r = u16::from(last_r_lo);
    }
    if (sym & 2) != 0 {
        let corr = dec.decode_symbol(&mut models.hi_r)? as u8;
        cur.r |= u16::from(corr.wrapping_add(last_r_hi)) << 8;
    } else {
        cur.r |= u16::from(last_r_hi) << 8;
    }

    if (sym & 64) != 0 {
        let cur_r_lo = (cur.r & 0xFF) as u8;
        let mut diff = i32::from(cur_r_lo) - i32::from(last_r_lo);
        if (sym & 4) != 0 {
            let corr = dec.decode_symbol(&mut models.lo_g)? as u8;
            cur.g = u16::from(corr.wrapping_add(clamp_u8(diff + i32::from(last_g_lo))));
        } else {
            cur.g = u16::from(last_g_lo);
        }
        if (sym & 16) != 0 {
            let corr = dec.decode_symbol(&mut models.lo_b)? as u8;
            let cur_g_lo = (cur.g & 0xFF) as u8;
            diff = (diff + i32::from(cur_g_lo) - i32::from(last_g_lo)) / 2;
            cur.b = u16::from(corr.wrapping_add(clamp_u8(diff + i32::from(last_b_lo))));
        } else {
            cur.b = u16::from(last_b_lo);
        }

        let cur_r_hi = (cur.r >> 8) as u8;
        diff = i32::from(cur_r_hi) - i32::from(last_r_hi);
        if (sym & 8) != 0 {
            let corr = dec.decode_symbol(&mut models.hi_g)? as u8;
            cur.g |= u16::from(corr.wrapping_add(clamp_u8(diff + i32::from(last_g_hi)))) << 8;
        } else {
            cur.g |= u16::from(last_g_hi) << 8;
        }
        if (sym & 32) != 0 {
            let corr = dec.decode_symbol(&mut models.hi_b)? as u8;
            let cur_g_hi = (cur.g >> 8) as u8;
            diff = (diff + i32::from(cur_g_hi) - i32::from(last_g_hi)) / 2;
            cur.b |= u16::from(corr.wrapping_add(clamp_u8(diff + i32::from(last_b_hi)))) << 8;
        } else {
            cur.b |= u16::from(last_b_hi) << 8;
        }
    } else {
        cur.g = cur.r;
        cur.b = cur.r;
    }
    Ok(cur)
}

struct Rgb12V2 {
    last: RgbValue,
    models: RgbModelsV2,
}

impl Rgb12V2 {
    fn new() -> Self {
        Self {
            last: RgbValue::default(),
            models: RgbModelsV2::default(),
        }
    }

    fn init(&mut self, cursor: &mut SliceCursor<'_>, out: &mut [u8]) -> LazResult<()> {
        cursor.read_into(out)?;
        self.last = RgbValue::unpack(out)?;
        Ok(())
    }

    fn decode(&mut self, dec: &mut RangeDecoder<'_>, out: &mut [u8]) -> LazResult<()> {
        self.last = decode_rgb_v2(dec, &mut self.models, self.last)?;
        self.last.pack(out)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct WavePacketValue {
    index: u8,
    offset: u64,
    size: u32,
    return_point: i32,
    dx: i32,
    dy: i32,
    dz: i32,
}

impl WavePacketValue {
    pub(super) const SIZE: usize = 29;

    pub(super) fn unpack(buf: &[u8]) -> LazResult<Self> {
        let index = *buf.first().ok_or(LazErr)?;
        let offset = u64::from_le_bytes(
            buf.get(1..9)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let size = u32::from_le_bytes(
            buf.get(9..13)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let return_point = i32::from_le_bytes(
            buf.get(13..17)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let dx = i32::from_le_bytes(
            buf.get(17..21)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let dy = i32::from_le_bytes(
            buf.get(21..25)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let dz = i32::from_le_bytes(
            buf.get(25..29)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        Ok(Self {
            index,
            offset,
            size,
            return_point,
            dx,
            dy,
            dz,
        })
    }

    pub(super) fn pack(&self, out: &mut [u8]) -> LazResult<()> {
        *out.first_mut().ok_or(LazErr)? = self.index;
        out.get_mut(1..9)
            .ok_or(LazErr)?
            .copy_from_slice(&self.offset.to_le_bytes());
        out.get_mut(9..13)
            .ok_or(LazErr)?
            .copy_from_slice(&self.size.to_le_bytes());
        out.get_mut(13..17)
            .ok_or(LazErr)?
            .copy_from_slice(&self.return_point.to_le_bytes());
        out.get_mut(17..21)
            .ok_or(LazErr)?
            .copy_from_slice(&self.dx.to_le_bytes());
        out.get_mut(21..25)
            .ok_or(LazErr)?
            .copy_from_slice(&self.dy.to_le_bytes());
        out.get_mut(25..29)
            .ok_or(LazErr)?
            .copy_from_slice(&self.dz.to_le_bytes());
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(super) struct WavePacketDecoder {
    pub(super) last: WavePacketValue,
    last_offset_diff: i32,
    last_sym_offset_diff: usize,
    packet_index: SymbolModel,
    offset_diff: [SymbolModel; 4],
    ic_offset_diff: IntDecoder,
    ic_packet_size: IntDecoder,
    ic_return_point: IntDecoder,
    ic_xyz: IntDecoder,
}

impl Default for WavePacketDecoder {
    fn default() -> Self {
        Self {
            last: WavePacketValue::default(),
            last_offset_diff: 0,
            last_sym_offset_diff: 0,
            packet_index: SymbolModel::new(256),
            offset_diff: [
                SymbolModel::new(4),
                SymbolModel::new(4),
                SymbolModel::new(4),
                SymbolModel::new(4),
            ],
            ic_offset_diff: IntDecoder::new(32, 1),
            ic_packet_size: IntDecoder::new(32, 1),
            ic_return_point: IntDecoder::new(32, 1),
            ic_xyz: IntDecoder::new(32, 3),
        }
    }
}

impl WavePacketDecoder {
    pub(super) fn init(&mut self, cursor: &mut SliceCursor<'_>, out: &mut [u8]) -> LazResult<()> {
        cursor.read_into(out)?;
        self.last = WavePacketValue::unpack(out)?;
        Ok(())
    }

    pub(super) fn decode(&mut self, dec: &mut RangeDecoder<'_>, out: &mut [u8]) -> LazResult<()> {
        let index = dec.decode_symbol(&mut self.packet_index)? as u8;
        let sym_model = self
            .offset_diff
            .get_mut(self.last_sym_offset_diff)
            .ok_or(LazErr)?;
        let sym = dec.decode_symbol(sym_model)? as usize;
        self.last_sym_offset_diff = sym;

        let offset = if sym == 0 {
            self.last.offset
        } else if sym == 1 {
            self.last.offset.wrapping_add(u64::from(self.last.size))
        } else if sym == 2 {
            self.last_offset_diff = self.ic_offset_diff.decode(dec, self.last_offset_diff, 0)?;
            (self.last.offset as i64).wrapping_add(i64::from(self.last_offset_diff)) as u64
        } else {
            dec.read_u64()?
        };

        let size = self.ic_packet_size.decode(dec, self.last.size as i32, 0)? as u32;
        let return_point = self
            .ic_return_point
            .decode(dec, self.last.return_point, 0)?;
        let dx = self.ic_xyz.decode(dec, self.last.dx, 0)?;
        let dy = self.ic_xyz.decode(dec, self.last.dy, 1)?;
        let dz = self.ic_xyz.decode(dec, self.last.dz, 2)?;
        let cur = WavePacketValue {
            index,
            offset,
            size,
            return_point,
            dx,
            dy,
            dz,
        };
        cur.pack(out)?;
        self.last = cur;
        Ok(())
    }
}

struct ExtraBytesV1V2 {
    last: Vec<u8>,
    models: Vec<SymbolModel>,
}

impl ExtraBytesV1V2 {
    fn new(count: usize) -> Self {
        Self {
            last: vec![0; count],
            models: (0..count).map(|_| SymbolModel::new(256)).collect(),
        }
    }

    fn init(&mut self, cursor: &mut SliceCursor<'_>, out: &mut [u8]) -> LazResult<()> {
        cursor.read_into(out)?;
        self.last.copy_from_slice(out);
        Ok(())
    }

    fn decode(&mut self, dec: &mut RangeDecoder<'_>, out: &mut [u8]) -> LazResult<()> {
        for ((last_b, model), dst_b) in self
            .last
            .iter_mut()
            .zip(self.models.iter_mut())
            .zip(out.iter_mut())
        {
            let diff = dec.decode_symbol(model)? as u8;
            *last_b = last_b.wrapping_add(diff);
            *dst_b = *last_b;
        }
        Ok(())
    }
}

enum SeqField {
    Point10V1(Box<Point10V1>),
    Point10V2(Box<Point10V2>),
    GpsTimeV1(Box<GpsTimeV1>),
    GpsTimeV2(Box<GpsTimeV2>),
    Rgb12V1(Box<Rgb12V1>),
    Rgb12V2(Box<Rgb12V2>),
    WavePacket13(Box<WavePacketDecoder>),
    ExtraBytes(ExtraBytesV1V2),
}

impl SeqField {
    fn size(&self) -> usize {
        match self {
            Self::Point10V1(_) | Self::Point10V2(_) => Point0::SIZE,
            Self::GpsTimeV1(_) | Self::GpsTimeV2(_) => 8,
            Self::Rgb12V1(_) | Self::Rgb12V2(_) => RgbValue::SIZE,
            Self::WavePacket13(_) => WavePacketValue::SIZE,
            Self::ExtraBytes(eb) => eb.last.len(),
        }
    }

    fn init(&mut self, cursor: &mut SliceCursor<'_>, out: &mut [u8]) -> LazResult<()> {
        match self {
            Self::Point10V1(d) => d.init(cursor, out),
            Self::Point10V2(d) => d.init(cursor, out),
            Self::GpsTimeV1(d) => d.init(cursor, out),
            Self::GpsTimeV2(d) => d.init(cursor, out),
            Self::Rgb12V1(d) => d.init(cursor, out),
            Self::Rgb12V2(d) => d.init(cursor, out),
            Self::WavePacket13(d) => d.init(cursor, out),
            Self::ExtraBytes(d) => d.init(cursor, out),
        }
    }

    fn decode(&mut self, dec: &mut RangeDecoder<'_>, out: &mut [u8]) -> LazResult<()> {
        match self {
            Self::Point10V1(d) => d.decode(dec, out),
            Self::Point10V2(d) => d.decode(dec, out),
            Self::GpsTimeV1(d) => d.decode(dec, out),
            Self::GpsTimeV2(d) => d.decode(dec, out),
            Self::Rgb12V1(d) => d.decode(dec, out),
            Self::Rgb12V2(d) => d.decode(dec, out),
            Self::WavePacket13(d) => d.decode(dec, out),
            Self::ExtraBytes(d) => d.decode(dec, out),
        }
    }
}

fn build_sequential_fields(items: &[(u16, u16, u16)]) -> LazResult<Vec<SeqField>> {
    let mut fields = Vec::with_capacity(items.len());
    for &(item_type, size, version) in items {
        let field = match (item_type, version) {
            (6, 1) if size == 20 => SeqField::Point10V1(Box::new(Point10V1::new())),
            (6, 2) if size == 20 => SeqField::Point10V2(Box::new(Point10V2::new())),
            (7, 1) if size == 8 => SeqField::GpsTimeV1(Box::new(GpsTimeV1::new())),
            (7, 2) if size == 8 => SeqField::GpsTimeV2(Box::new(GpsTimeV2::new())),
            (8, 1) if size == 6 => SeqField::Rgb12V1(Box::new(Rgb12V1::new())),
            (8, 2) if size == 6 => SeqField::Rgb12V2(Box::new(Rgb12V2::new())),
            (9, 1 | 2) if size == 29 => SeqField::WavePacket13(Box::default()),
            (0, 1 | 2) if size > 0 => SeqField::ExtraBytes(ExtraBytesV1V2::new(usize::from(size))),
            _ => return Err(LazErr),
        };
        fields.push(field);
    }
    Ok(fields)
}

/// 逐次圧縮された 1 チャンク（`out` に入る点数ぶん）を展開する。
pub(super) fn decompress_sequential_chunk<'a>(
    items: &[(u16, u16, u16)],
    mut cursor: SliceCursor<'a>,
    record_size: usize,
    out: &mut [u8],
    cx: &mut Cx<'_>,
) -> Result<SliceCursor<'a>, Error> {
    if out.is_empty() {
        return Ok(cursor);
    }
    let mut fields = build_sequential_fields(items).map_err(|_| Error::Invalid)?;
    let mut records = out.chunks_exact_mut(record_size);
    let Some(first_rec) = records.next() else {
        return Ok(cursor);
    };
    cx.check(Kind::Work, 1)?;
    let mut offset = 0usize;
    for field in &mut fields {
        let len = field.size();
        let dst = first_rec
            .get_mut(offset..offset + len)
            .ok_or(Error::Invalid)?;
        field.init(&mut cursor, dst).map_err(|_| Error::Invalid)?;
        offset += len;
    }

    // 1 点のみのチャンクでも算術符号器の初期 4 バイトが書かれる。
    let mut dec = RangeDecoder::from_cursor(cursor).map_err(|_| Error::Invalid)?;
    for rec in records {
        cx.check(Kind::Work, 1)?;
        let mut off = 0usize;
        for field in &mut fields {
            let len = field.size();
            let dst = rec.get_mut(off..off + len).ok_or(Error::Invalid)?;
            field.decode(&mut dec, dst).map_err(|_| Error::Invalid)?;
            off += len;
        }
    }
    Ok(dec.into_cursor())
}
