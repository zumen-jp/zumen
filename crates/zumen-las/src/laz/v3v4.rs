//! LAZ 1.4（OGC 24-070r1 第 6 章、項目バージョン 3・4、レイヤ圧縮）の項目復号器。
//!
//! 各レイヤのバイト列は入力スライスをゼロコピーで参照し、巨大なレイヤ長が書かれた壊れた入力でも
//! メモリ確保を行わずに即座に弾く。

use zumen_base::Kind;
use zumen_sdk::{Cx, Error};

use super::coder::{
    IntDecoder, LazErr, LazResult, Median5, RangeDecoder, SliceCursor, SymbolModel,
};
use super::v1v2::{RgbModelsV2, RgbValue, WavePacketDecoder, WavePacketValue, decode_rgb_v2};

const NUMBER_RETURN_MAP_6CTX: [[u8; 16]; 16] = [
    [0, 1, 2, 3, 4, 5, 3, 4, 4, 5, 5, 5, 5, 5, 5, 5],
    [1, 0, 1, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3],
    [2, 1, 2, 4, 4, 4, 4, 4, 4, 4, 4, 3, 3, 3, 3, 3],
    [3, 3, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4],
    [4, 3, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4],
    [5, 3, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4],
    [3, 3, 4, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4, 4],
    [4, 3, 4, 4, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4, 4],
    [4, 3, 4, 4, 4, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4, 4],
    [5, 3, 4, 4, 4, 4, 4, 4, 4, 5, 4, 4, 4, 4, 4, 4],
    [5, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 4, 4, 4, 4, 4],
    [5, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 4, 4, 4],
    [5, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 4, 4],
    [5, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 4],
    [5, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5],
    [5, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5],
];

const GPS_MULTI: i32 = 500;
const GPS_MULTI_MINUS: i32 = -10;
const GPS_MULTI_CODE_FULL: i32 = GPS_MULTI - GPS_MULTI_MINUS + 1; // 511
const GPS_MULTI_TOTAL: u32 = (GPS_MULTI - GPS_MULTI_MINUS + 5) as u32; // 515

#[derive(Clone, Copy, Debug, Default)]
struct Point6 {
    x: i32,
    y: i32,
    z: i32,
    intensity: u16,
    bit_fields: u8,
    flags: u8,
    classification: u8,
    user_data: u8,
    scan_angle: i16,
    point_source_id: u16,
    gps_time: i64,
    gps_time_change: bool,
}

impl Point6 {
    const SIZE: usize = 30;

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
        let flags = *buf.get(15).ok_or(LazErr)?;
        let classification = *buf.get(16).ok_or(LazErr)?;
        let user_data = *buf.get(17).ok_or(LazErr)?;
        let scan_angle = i16::from_le_bytes(
            buf.get(18..20)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let point_source_id = u16::from_le_bytes(
            buf.get(20..22)
                .ok_or(LazErr)?
                .try_into()
                .map_err(|_| LazErr)?,
        );
        let gps_time = i64::from_le_bytes(
            buf.get(22..30)
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
            flags,
            classification,
            user_data,
            scan_angle,
            point_source_id,
            gps_time,
            gps_time_change: false,
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
        *out.get_mut(15).ok_or(LazErr)? = self.flags;
        *out.get_mut(16).ok_or(LazErr)? = self.classification;
        *out.get_mut(17).ok_or(LazErr)? = self.user_data;
        out.get_mut(18..20)
            .ok_or(LazErr)?
            .copy_from_slice(&self.scan_angle.to_le_bytes());
        out.get_mut(20..22)
            .ok_or(LazErr)?
            .copy_from_slice(&self.point_source_id.to_le_bytes());
        out.get_mut(22..30)
            .ok_or(LazErr)?
            .copy_from_slice(&self.gps_time.to_le_bytes());
        Ok(())
    }

    fn return_number(&self) -> u8 {
        self.bit_fields & 0x0F
    }

    fn set_return_number(&mut self, r: u8) {
        self.bit_fields = (self.bit_fields & 0xF0) | (r & 0x0F);
    }

    fn number_of_returns(&self) -> u8 {
        (self.bit_fields >> 4) & 0x0F
    }

    fn set_number_of_returns(&mut self, n: u8) {
        self.bit_fields = (self.bit_fields & 0x0F) | ((n & 0x0F) << 4);
    }

    fn classification_flags(&self) -> u8 {
        self.flags & 0x0F
    }

    fn scanner_channel(&self) -> u8 {
        (self.flags >> 4) & 0x03
    }

    fn set_scanner_channel(&mut self, ch: u8) {
        self.flags = (self.flags & !0x30) | ((ch & 0x03) << 4);
    }

    fn scan_direction(&self) -> u8 {
        (self.flags >> 6) & 0x01
    }

    fn edge_of_flight_line(&self) -> u8 {
        (self.flags >> 7) & 0x01
    }

    fn last_point_return_ctx(&self) -> usize {
        let mut lpr = usize::from(self.return_number() == 1);
        if self.return_number() >= self.number_of_returns() {
            lpr += 2;
        }
        if self.gps_time_change {
            lpr += 4;
        }
        lpr
    }
}

struct Point6Context {
    unused: bool,
    last_point: Point6,
    last_intensities: [u16; 8],
    last_x_diff_median5: [Median5; 12],
    last_y_diff_median5: [Median5; 12],
    last_z: [i32; 8],
    changed_values: Vec<SymbolModel>,
    scanner_channel: SymbolModel,
    number_of_returns: Vec<Option<SymbolModel>>,
    return_number: Vec<Option<SymbolModel>>,
    return_number_gps_same: SymbolModel,
    classification: Vec<Option<SymbolModel>>,
    classification_flags: Vec<Option<SymbolModel>>,
    user_data: Vec<Option<SymbolModel>>,
    gps_time_multi: SymbolModel,
    gps_time_no_diff: SymbolModel,
    ic_dx: IntDecoder,
    ic_dy: IntDecoder,
    ic_z: IntDecoder,
    ic_intensity: IntDecoder,
    ic_scan_angle: IntDecoder,
    ic_source_id: IntDecoder,
    ic_gps_time: IntDecoder,
    gps_last: usize,
    gps_next: usize,
    gps_times: [i64; 4],
    gps_diffs: [i32; 4],
    gps_extreme: [i32; 4],
}

impl Point6Context {
    fn new_unused() -> Self {
        Self::from_point_inner(Point6::default(), true)
    }

    fn from_point(pt: Point6) -> Self {
        Self::from_point_inner(pt, false)
    }

    fn from_point_inner(mut pt: Point6, unused: bool) -> Self {
        pt.gps_time_change = false;
        let mut gps_times = [0i64; 4];
        gps_times[0] = pt.gps_time;
        Self {
            unused,
            last_point: pt,
            last_intensities: [pt.intensity; 8],
            last_x_diff_median5: [Median5::default(); 12],
            last_y_diff_median5: [Median5::default(); 12],
            last_z: [pt.z; 8],
            changed_values: (0..8).map(|_| SymbolModel::new(128)).collect(),
            scanner_channel: SymbolModel::new(3),
            number_of_returns: vec![None; 16],
            return_number: vec![None; 16],
            return_number_gps_same: SymbolModel::new(13),
            classification: vec![None; 64],
            classification_flags: vec![None; 64],
            user_data: vec![None; 64],
            gps_time_multi: SymbolModel::new(GPS_MULTI_TOTAL),
            gps_time_no_diff: SymbolModel::new(5),
            ic_dx: IntDecoder::new(32, 2),
            ic_dy: IntDecoder::new(32, 22),
            ic_z: IntDecoder::new(32, 20),
            ic_intensity: IntDecoder::new(16, 4),
            ic_scan_angle: IntDecoder::new(16, 2),
            ic_source_id: IntDecoder::new(16, 1),
            ic_gps_time: IntDecoder::new(32, 9),
            gps_last: 0,
            gps_next: 0,
            gps_times,
            gps_diffs: [0; 4],
            gps_extreme: [0; 4],
        }
    }

    fn decode_gps_time(&mut self, dec: &mut RangeDecoder<'_>) -> LazResult<i64> {
        let mut decoded = false;
        for _ in 0..8 {
            let cur_diff = *self.gps_diffs.get(self.gps_last).ok_or(LazErr)?;
            if cur_diff == 0 {
                let multi = dec.decode_symbol(&mut self.gps_time_no_diff)? as i32;
                if multi == 0 {
                    let d = self.ic_gps_time.decode(dec, 0, 0)?;
                    *self.gps_diffs.get_mut(self.gps_last).ok_or(LazErr)? = d;
                    let t = self.gps_times.get_mut(self.gps_last).ok_or(LazErr)?;
                    *t = t.wrapping_add(i64::from(d));
                    *self.gps_extreme.get_mut(self.gps_last).ok_or(LazErr)? = 0;
                    decoded = true;
                    break;
                } else if multi == 1 {
                    self.gps_next = (self.gps_next + 1) & 3;
                    let prev_hi = (*self.gps_times.get(self.gps_last).ok_or(LazErr)? >> 32) as i32;
                    let hi = i64::from(self.ic_gps_time.decode(dec, prev_hi, 8)?);
                    let lo = i64::from(dec.read_u32()?);
                    *self.gps_times.get_mut(self.gps_next).ok_or(LazErr)? = (hi << 32) | lo;
                    self.gps_last = self.gps_next;
                    *self.gps_diffs.get_mut(self.gps_last).ok_or(LazErr)? = 0;
                    *self.gps_extreme.get_mut(self.gps_last).ok_or(LazErr)? = 0;
                    decoded = true;
                    break;
                } else {
                    self.gps_last = (self.gps_last + (multi as usize) - 1) & 3;
                }
            } else {
                let multi = dec.decode_symbol(&mut self.gps_time_multi)? as i32;
                if multi == 1 {
                    let d = self.ic_gps_time.decode(dec, cur_diff, 1)?;
                    let t = self.gps_times.get_mut(self.gps_last).ok_or(LazErr)?;
                    *t = t.wrapping_add(i64::from(d));
                    *self.gps_extreme.get_mut(self.gps_last).ok_or(LazErr)? = 0;
                    decoded = true;
                    break;
                } else if multi < GPS_MULTI_CODE_FULL {
                    let diff = if multi == 0 {
                        let d = self.ic_gps_time.decode(dec, 0, 7)?;
                        let ec = self.gps_extreme.get_mut(self.gps_last).ok_or(LazErr)?;
                        *ec += 1;
                        if *ec > 3 {
                            *ec = 0;
                            *self.gps_diffs.get_mut(self.gps_last).ok_or(LazErr)? = d;
                        }
                        d
                    } else if multi < GPS_MULTI {
                        let ctx = if multi < 10 { 2 } else { 3 };
                        self.ic_gps_time
                            .decode(dec, multi.wrapping_mul(cur_diff), ctx)?
                    } else if multi == GPS_MULTI {
                        let d =
                            self.ic_gps_time
                                .decode(dec, GPS_MULTI.wrapping_mul(cur_diff), 4)?;
                        let ec = self.gps_extreme.get_mut(self.gps_last).ok_or(LazErr)?;
                        *ec += 1;
                        if *ec > 3 {
                            *ec = 0;
                            *self.gps_diffs.get_mut(self.gps_last).ok_or(LazErr)? = d;
                        }
                        d
                    } else {
                        let neg = GPS_MULTI - multi;
                        if neg > GPS_MULTI_MINUS {
                            self.ic_gps_time
                                .decode(dec, neg.wrapping_mul(cur_diff), 5)?
                        } else {
                            let d = self.ic_gps_time.decode(
                                dec,
                                GPS_MULTI_MINUS.wrapping_mul(cur_diff),
                                6,
                            )?;
                            let ec = self.gps_extreme.get_mut(self.gps_last).ok_or(LazErr)?;
                            *ec += 1;
                            if *ec > 3 {
                                *ec = 0;
                                *self.gps_diffs.get_mut(self.gps_last).ok_or(LazErr)? = d;
                            }
                            d
                        }
                    };
                    let t = self.gps_times.get_mut(self.gps_last).ok_or(LazErr)?;
                    *t = t.wrapping_add(i64::from(diff));
                    decoded = true;
                    break;
                } else if multi == GPS_MULTI_CODE_FULL {
                    self.gps_next = (self.gps_next + 1) & 3;
                    let prev_hi = (*self.gps_times.get(self.gps_last).ok_or(LazErr)? >> 32) as i32;
                    let hi = i64::from(self.ic_gps_time.decode(dec, prev_hi, 8)?);
                    let lo = i64::from(dec.read_u32()?);
                    *self.gps_times.get_mut(self.gps_next).ok_or(LazErr)? = (hi << 32) | lo;
                    self.gps_last = self.gps_next;
                    *self.gps_diffs.get_mut(self.gps_last).ok_or(LazErr)? = 0;
                    *self.gps_extreme.get_mut(self.gps_last).ok_or(LazErr)? = 0;
                    decoded = true;
                    break;
                } else {
                    self.gps_last =
                        (self.gps_last + (multi as usize) - (GPS_MULTI_CODE_FULL as usize)) & 3;
                }
            }
        }
        if !decoded {
            return Err(LazErr);
        }
        self.gps_times.get(self.gps_last).copied().ok_or(LazErr)
    }
}

fn opt_layer<'a>(cursor: &mut SliceCursor<'a>, len: u32) -> LazResult<Option<RangeDecoder<'a>>> {
    let n = usize::try_from(len).map_err(|_| LazErr)?;
    if n == 0 {
        Ok(None)
    } else {
        let slice = cursor.take(n)?;
        Ok(Some(RangeDecoder::from_slice(slice)?))
    }
}

struct Point14Decoder<'a> {
    version: u16,
    layer_sizes: [u32; 9],
    dec_xy: Option<RangeDecoder<'a>>,
    dec_z: Option<RangeDecoder<'a>>,
    dec_class: Option<RangeDecoder<'a>>,
    dec_flags: Option<RangeDecoder<'a>>,
    dec_intensity: Option<RangeDecoder<'a>>,
    dec_scan_angle: Option<RangeDecoder<'a>>,
    dec_user_data: Option<RangeDecoder<'a>>,
    dec_point_source: Option<RangeDecoder<'a>>,
    dec_gps_time: Option<RangeDecoder<'a>>,
    current_ctx: usize,
    contexts: [Point6Context; 4],
}

impl<'a> Point14Decoder<'a> {
    fn new(version: u16) -> Self {
        Self {
            version,
            layer_sizes: [0; 9],
            dec_xy: None,
            dec_z: None,
            dec_class: None,
            dec_flags: None,
            dec_intensity: None,
            dec_scan_angle: None,
            dec_user_data: None,
            dec_point_source: None,
            dec_gps_time: None,
            current_ctx: 0,
            contexts: [
                Point6Context::new_unused(),
                Point6Context::new_unused(),
                Point6Context::new_unused(),
                Point6Context::new_unused(),
            ],
        }
    }

    fn init_first(
        &mut self,
        cursor: &mut SliceCursor<'a>,
        out: &mut [u8],
        ctx_out: &mut usize,
    ) -> LazResult<()> {
        for c in &mut self.contexts {
            c.unused = true;
        }
        cursor.read_into(out)?;
        let pt = Point6::unpack(out)?;
        let ch = usize::from(pt.scanner_channel());
        self.current_ctx = ch;
        *ctx_out = ch;
        *self.contexts.get_mut(ch).ok_or(LazErr)? = Point6Context::from_point(pt);
        Ok(())
    }

    fn read_sizes(&mut self, cursor: &mut SliceCursor<'a>) -> LazResult<()> {
        for s in &mut self.layer_sizes {
            *s = cursor.u32_le()?;
        }
        Ok(())
    }

    fn read_layers(&mut self, cursor: &mut SliceCursor<'a>) -> LazResult<()> {
        let [s0, s1, s2, s3, s4, s5, s6, s7, s8] = self.layer_sizes;
        self.dec_xy = opt_layer(cursor, s0)?;
        self.dec_z = opt_layer(cursor, s1)?;
        self.dec_class = opt_layer(cursor, s2)?;
        self.dec_flags = opt_layer(cursor, s3)?;
        self.dec_intensity = opt_layer(cursor, s4)?;
        self.dec_scan_angle = opt_layer(cursor, s5)?;
        self.dec_user_data = opt_layer(cursor, s6)?;
        self.dec_point_source = opt_layer(cursor, s7)?;
        self.dec_gps_time = opt_layer(cursor, s8)?;
        Ok(())
    }

    fn decode(&mut self, out: &mut [u8], ctx_out: &mut usize) -> LazResult<()> {
        let dec_xy = self.dec_xy.as_mut().ok_or(LazErr)?;
        let changed = {
            let ctx = self.contexts.get_mut(self.current_ctx).ok_or(LazErr)?;
            let lpr = ctx.last_point.last_point_return_ctx();
            let model = ctx.changed_values.get_mut(lpr).ok_or(LazErr)?;
            dec_xy.decode_symbol(model)?
        };

        if (changed & 64) != 0 {
            let diff = {
                let ctx = self.contexts.get_mut(self.current_ctx).ok_or(LazErr)?;
                dec_xy.decode_symbol(&mut ctx.scanner_channel)?
            };
            let next_ch = (self.current_ctx + (diff as usize) + 1) & 3;
            let is_unused = self.contexts.get(next_ch).ok_or(LazErr)?.unused;
            if is_unused {
                let prev_pt = self
                    .contexts
                    .get(self.current_ctx)
                    .ok_or(LazErr)?
                    .last_point;
                *self.contexts.get_mut(next_ch).ok_or(LazErr)? = Point6Context::from_point(prev_pt);
            }
            self.current_ctx = next_ch;
            if self.version == 3 {
                *ctx_out = self.current_ctx;
            }
            self.contexts
                .get_mut(self.current_ctx)
                .ok_or(LazErr)?
                .last_point
                .set_scanner_channel(next_ch as u8);
        }
        if self.version == 4 {
            *ctx_out = self.current_ctx;
        }

        let point_source_changed = (changed & 32) != 0;
        let gps_time_changed = (changed & 16) != 0;
        let scan_angle_changed = (changed & 8) != 0;

        let ctx = self.contexts.get_mut(self.current_ctx).ok_or(LazErr)?;
        let last_n = ctx.last_point.number_of_returns();
        let last_r = ctx.last_point.return_number();

        let n = if (changed & 4) != 0 {
            let model = ctx
                .number_of_returns
                .get_mut(usize::from(last_n))
                .ok_or(LazErr)?
                .get_or_insert_with(|| SymbolModel::new(16));
            dec_xy.decode_symbol(model)? as u8
        } else {
            last_n
        };
        ctx.last_point.set_number_of_returns(n);

        let r = match changed & 3 {
            0 => last_r,
            1 => (last_r + 1) & 15,
            2 => (last_r + 15) & 15,
            _ => {
                if gps_time_changed {
                    let model = ctx
                        .return_number
                        .get_mut(usize::from(last_r))
                        .ok_or(LazErr)?
                        .get_or_insert_with(|| SymbolModel::new(16));
                    dec_xy.decode_symbol(model)? as u8
                } else {
                    let sym = dec_xy.decode_symbol(&mut ctx.return_number_gps_same)? as u8;
                    (last_r.wrapping_add(sym).wrapping_add(2)) & 15
                }
            }
        };
        ctx.last_point.set_return_number(r);

        let m = usize::from(
            *NUMBER_RETURN_MAP_6CTX
                .get(usize::from(n))
                .and_then(|row| row.get(usize::from(r)))
                .ok_or(LazErr)?,
        );
        let l = usize::from(n.abs_diff(r).min(7));
        let cpr = (if r == 1 { 2u32 } else { 0 }) + u32::from(r >= n);

        let med_idx = (m << 1) | usize::from(gps_time_changed);
        let med_x = ctx.last_x_diff_median5.get(med_idx).ok_or(LazErr)?.get();
        let dx = ctx.ic_dx.decode(dec_xy, med_x, u32::from(n == 1))?;
        ctx.last_point.x = ctx.last_point.x.wrapping_add(dx);
        ctx.last_x_diff_median5
            .get_mut(med_idx)
            .ok_or(LazErr)?
            .add(dx);

        let med_y = ctx.last_y_diff_median5.get(med_idx).ok_or(LazErr)?.get();
        let k_dx = ctx.ic_dx.k();
        let ctx_y = u32::from(n == 1) + if k_dx < 20 { k_dx & !1 } else { 20 };
        let dy = ctx.ic_dy.decode(dec_xy, med_y, ctx_y)?;
        ctx.last_point.y = ctx.last_point.y.wrapping_add(dy);
        ctx.last_y_diff_median5
            .get_mut(med_idx)
            .ok_or(LazErr)?
            .add(dy);

        if let Some(dec_z) = &mut self.dec_z {
            let k_z = (ctx.ic_dx.k() + ctx.ic_dy.k()) / 2;
            let ctx_z = u32::from(n == 1) + if k_z < 18 { k_z & !1 } else { 18 };
            let prev_z = *ctx.last_z.get(l).ok_or(LazErr)?;
            let z = ctx.ic_z.decode(dec_z, prev_z, ctx_z)?;
            ctx.last_point.z = z;
            *ctx.last_z.get_mut(l).ok_or(LazErr)? = z;
        }

        if let Some(dec_class) = &mut self.dec_class {
            let ccc =
                usize::from(((ctx.last_point.classification & 0x1F) << 1) | u8::from(cpr == 3));
            let model = ctx
                .classification
                .get_mut(ccc)
                .ok_or(LazErr)?
                .get_or_insert_with(|| SymbolModel::new(256));
            ctx.last_point.classification = dec_class.decode_symbol(model)? as u8;
        }

        if let Some(dec_flags) = &mut self.dec_flags {
            let prev_flags = usize::from(
                (ctx.last_point.edge_of_flight_line() << 5)
                    | (ctx.last_point.scan_direction() << 4)
                    | ctx.last_point.classification_flags(),
            );
            let model = ctx
                .classification_flags
                .get_mut(prev_flags)
                .ok_or(LazErr)?
                .get_or_insert_with(|| SymbolModel::new(64));
            let flags = dec_flags.decode_symbol(model)? as u8;
            let ch_bits = (ctx.last_point.scanner_channel() << 4) & 0x30;
            ctx.last_point.flags =
                (((flags >> 5) & 1) << 7) | (((flags >> 4) & 1) << 6) | ch_bits | (flags & 0x0F);
        }

        if let Some(dec_int) = &mut self.dec_intensity {
            let idx = ((cpr << 1) | u32::from(gps_time_changed)) as usize;
            let prev_int = *ctx.last_intensities.get(idx).ok_or(LazErr)?;
            let val = ctx.ic_intensity.decode(dec_int, i32::from(prev_int), cpr)? as u16;
            ctx.last_point.intensity = val;
            *ctx.last_intensities.get_mut(idx).ok_or(LazErr)? = val;
        }

        if scan_angle_changed && let Some(dec_sa) = &mut self.dec_scan_angle {
            ctx.last_point.scan_angle = ctx.ic_scan_angle.decode(
                dec_sa,
                i32::from(ctx.last_point.scan_angle),
                u32::from(gps_time_changed),
            )? as i16;
        }

        if let Some(dec_ud) = &mut self.dec_user_data {
            let bucket = usize::from(ctx.last_point.user_data / 4);
            let model = ctx
                .user_data
                .get_mut(bucket)
                .ok_or(LazErr)?
                .get_or_insert_with(|| SymbolModel::new(256));
            ctx.last_point.user_data = dec_ud.decode_symbol(model)? as u8;
        }

        if point_source_changed && let Some(dec_ps) = &mut self.dec_point_source {
            ctx.last_point.point_source_id =
                ctx.ic_source_id
                    .decode(dec_ps, i32::from(ctx.last_point.point_source_id), 0)?
                    as u16;
        }

        if gps_time_changed && let Some(dec_gps) = &mut self.dec_gps_time {
            ctx.last_point.gps_time = ctx.decode_gps_time(dec_gps)?;
        }

        ctx.last_point.gps_time_change = gps_time_changed;
        ctx.last_point.pack(out)
    }
}

struct Rgb14Decoder<'a> {
    version: u16,
    layer_size: u32,
    dec: Option<RangeDecoder<'a>>,
    unused: [bool; 4],
    models: [RgbModelsV2; 4],
    last_rgbs: [RgbValue; 4],
    last_ctx: usize,
}

impl<'a> Rgb14Decoder<'a> {
    fn new(version: u16) -> Self {
        Self {
            version,
            layer_size: 0,
            dec: None,
            unused: [true; 4],
            models: [
                RgbModelsV2::default(),
                RgbModelsV2::default(),
                RgbModelsV2::default(),
                RgbModelsV2::default(),
            ],
            last_rgbs: [RgbValue::default(); 4],
            last_ctx: 0,
        }
    }

    fn init_first(
        &mut self,
        cursor: &mut SliceCursor<'a>,
        out: &mut [u8],
        ctx: usize,
    ) -> LazResult<()> {
        self.unused = [true; 4];
        cursor.read_into(out)?;
        *self.last_rgbs.get_mut(ctx).ok_or(LazErr)? = RgbValue::unpack(out)?;
        *self.unused.get_mut(ctx).ok_or(LazErr)? = false;
        self.last_ctx = ctx;
        Ok(())
    }

    fn decode(&mut self, out: &mut [u8], ctx: usize) -> LazResult<()> {
        let mut last_idx = self.last_ctx;
        if self.last_ctx != ctx {
            self.last_ctx = ctx;
            if *self.unused.get(ctx).ok_or(LazErr)? {
                let prev = *self.last_rgbs.get(last_idx).ok_or(LazErr)?;
                *self.last_rgbs.get_mut(ctx).ok_or(LazErr)? = prev;
                *self.models.get_mut(ctx).ok_or(LazErr)? = RgbModelsV2::default();
                *self.unused.get_mut(ctx).ok_or(LazErr)? = false;
                if self.version == 3 {
                    last_idx = ctx;
                }
            }
            if self.version == 4 {
                last_idx = ctx;
            }
        }
        let prev = *self.last_rgbs.get(last_idx).ok_or(LazErr)?;
        if let Some(dec) = &mut self.dec {
            let models = self.models.get_mut(self.last_ctx).ok_or(LazErr)?;
            let next = decode_rgb_v2(dec, models, prev)?;
            next.pack(out)?;
            *self.last_rgbs.get_mut(last_idx).ok_or(LazErr)? = next;
        } else {
            prev.pack(out)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct NirModels {
    bytes_used: SymbolModel,
    lo_diff: SymbolModel,
    hi_diff: SymbolModel,
}

impl Default for NirModels {
    fn default() -> Self {
        Self {
            bytes_used: SymbolModel::new(4),
            lo_diff: SymbolModel::new(256),
            hi_diff: SymbolModel::new(256),
        }
    }
}

struct Nir14Decoder<'a> {
    version: u16,
    layer_size: u32,
    dec: Option<RangeDecoder<'a>>,
    unused: [bool; 4],
    models: [NirModels; 4],
    last_nirs: [u16; 4],
    last_ctx: usize,
}

impl<'a> Nir14Decoder<'a> {
    fn new(version: u16) -> Self {
        Self {
            version,
            layer_size: 0,
            dec: None,
            unused: [true; 4],
            models: [
                NirModels::default(),
                NirModels::default(),
                NirModels::default(),
                NirModels::default(),
            ],
            last_nirs: [0; 4],
            last_ctx: 0,
        }
    }

    fn init_first(
        &mut self,
        cursor: &mut SliceCursor<'a>,
        out: &mut [u8],
        ctx: usize,
    ) -> LazResult<()> {
        self.unused = [true; 4];
        cursor.read_into(out)?;
        let arr: [u8; 2] = out.try_into().map_err(|_| LazErr)?;
        *self.last_nirs.get_mut(ctx).ok_or(LazErr)? = u16::from_le_bytes(arr);
        *self.unused.get_mut(ctx).ok_or(LazErr)? = false;
        self.last_ctx = ctx;
        Ok(())
    }

    fn decode(&mut self, out: &mut [u8], ctx: usize) -> LazResult<()> {
        let mut last_idx = self.last_ctx;
        if self.last_ctx != ctx {
            self.last_ctx = ctx;
            if *self.unused.get(ctx).ok_or(LazErr)? {
                let prev = *self.last_nirs.get(last_idx).ok_or(LazErr)?;
                *self.last_nirs.get_mut(ctx).ok_or(LazErr)? = prev;
                *self.models.get_mut(ctx).ok_or(LazErr)? = NirModels::default();
                *self.unused.get_mut(ctx).ok_or(LazErr)? = false;
                if self.version == 3 {
                    last_idx = ctx;
                }
            }
            if self.version == 4 {
                last_idx = ctx;
            }
        }
        let prev = *self.last_nirs.get(last_idx).ok_or(LazErr)?;
        let val = if let Some(dec) = &mut self.dec {
            let m = self.models.get_mut(self.last_ctx).ok_or(LazErr)?;
            let sym = dec.decode_symbol(&mut m.bytes_used)?;
            let mut next = if (sym & 1) != 0 {
                let d = dec.decode_symbol(&mut m.lo_diff)? as u8;
                u16::from(d.wrapping_add((prev & 0xFF) as u8))
            } else {
                prev & 0x00FF
            };
            if (sym & 2) != 0 {
                let d = dec.decode_symbol(&mut m.hi_diff)? as u8;
                next |= u16::from(d.wrapping_add((prev >> 8) as u8)) << 8;
            } else {
                next |= prev & 0xFF00;
            }
            *self.last_nirs.get_mut(last_idx).ok_or(LazErr)? = next;
            next
        } else {
            prev
        };
        out.copy_from_slice(&val.to_le_bytes());
        Ok(())
    }
}

struct WavePacket14Decoder<'a> {
    version: u16,
    layer_size: u32,
    dec: Option<RangeDecoder<'a>>,
    unused: [bool; 4],
    contexts: [WavePacketDecoder; 4],
    last_packets: [WavePacketValue; 4],
    last_ctx: usize,
}

impl<'a> WavePacket14Decoder<'a> {
    fn new(version: u16) -> Self {
        Self {
            version,
            layer_size: 0,
            dec: None,
            unused: [true; 4],
            contexts: [
                WavePacketDecoder::default(),
                WavePacketDecoder::default(),
                WavePacketDecoder::default(),
                WavePacketDecoder::default(),
            ],
            last_packets: [WavePacketValue::default(); 4],
            last_ctx: 0,
        }
    }

    fn init_first(
        &mut self,
        cursor: &mut SliceCursor<'a>,
        out: &mut [u8],
        ctx: usize,
    ) -> LazResult<()> {
        self.unused = [true; 4];
        let wp_ctx = self.contexts.get_mut(ctx).ok_or(LazErr)?;
        wp_ctx.init(cursor, out)?;
        *self.last_packets.get_mut(ctx).ok_or(LazErr)? = wp_ctx.last;
        *self.unused.get_mut(ctx).ok_or(LazErr)? = false;
        self.last_ctx = ctx;
        Ok(())
    }

    fn decode(&mut self, out: &mut [u8], ctx: usize) -> LazResult<()> {
        let mut last_idx = self.last_ctx;
        if self.last_ctx != ctx {
            self.last_ctx = ctx;
            if *self.unused.get(ctx).ok_or(LazErr)? {
                let prev = *self.last_packets.get(last_idx).ok_or(LazErr)?;
                *self.last_packets.get_mut(ctx).ok_or(LazErr)? = prev;
                *self.contexts.get_mut(ctx).ok_or(LazErr)? = WavePacketDecoder::default();
                *self.unused.get_mut(ctx).ok_or(LazErr)? = false;
                if self.version == 3 {
                    last_idx = ctx;
                }
            }
            if self.version == 4 {
                last_idx = ctx;
            }
        }
        let prev = *self.last_packets.get(last_idx).ok_or(LazErr)?;
        if let Some(dec) = &mut self.dec {
            let wp_ctx = self.contexts.get_mut(self.last_ctx).ok_or(LazErr)?;
            wp_ctx.last = prev;
            wp_ctx.decode(dec, out)?;
            *self.last_packets.get_mut(last_idx).ok_or(LazErr)? = wp_ctx.last;
        } else {
            prev.pack(out)?;
        }
        Ok(())
    }
}

struct Byte14Decoder<'a> {
    version: u16,
    count: usize,
    layer_sizes: Vec<u32>,
    decs: Vec<Option<RangeDecoder<'a>>>,
    unused: [bool; 4],
    models: [Vec<SymbolModel>; 4],
    last_bytes: [Vec<u8>; 4],
    last_ctx: usize,
}

impl<'a> Byte14Decoder<'a> {
    fn new(version: u16, count: usize) -> Self {
        let mk_models = || (0..count).map(|_| SymbolModel::new(256)).collect();
        Self {
            version,
            count,
            layer_sizes: vec![0; count],
            decs: Vec::with_capacity(count),
            unused: [true; 4],
            models: [mk_models(), mk_models(), mk_models(), mk_models()],
            last_bytes: [
                vec![0; count],
                vec![0; count],
                vec![0; count],
                vec![0; count],
            ],
            last_ctx: 0,
        }
    }

    fn init_first(
        &mut self,
        cursor: &mut SliceCursor<'a>,
        out: &mut [u8],
        ctx: usize,
    ) -> LazResult<()> {
        self.unused = [true; 4];
        cursor.read_into(out)?;
        self.last_bytes
            .get_mut(ctx)
            .ok_or(LazErr)?
            .copy_from_slice(out);
        *self.unused.get_mut(ctx).ok_or(LazErr)? = false;
        self.last_ctx = ctx;
        Ok(())
    }

    fn read_sizes(&mut self, cursor: &mut SliceCursor<'a>) -> LazResult<()> {
        for s in &mut self.layer_sizes {
            *s = cursor.u32_le()?;
        }
        Ok(())
    }

    fn read_layers(&mut self, cursor: &mut SliceCursor<'a>) -> LazResult<()> {
        self.decs.clear();
        for &len in &self.layer_sizes {
            self.decs.push(opt_layer(cursor, len)?);
        }
        Ok(())
    }

    fn decode(&mut self, out: &mut [u8], ctx: usize) -> LazResult<()> {
        let mut last_idx = self.last_ctx;
        if self.last_ctx != ctx {
            self.last_ctx = ctx;
            if *self.unused.get(ctx).ok_or(LazErr)? {
                let prev = self.last_bytes.get(last_idx).ok_or(LazErr)?.clone();
                *self.last_bytes.get_mut(ctx).ok_or(LazErr)? = prev;
                *self.models.get_mut(ctx).ok_or(LazErr)? =
                    (0..self.count).map(|_| SymbolModel::new(256)).collect();
                *self.unused.get_mut(ctx).ok_or(LazErr)? = false;
                if self.version == 3 {
                    last_idx = ctx;
                }
            }
            if self.version == 4 {
                last_idx = ctx;
            }
        }
        let lb = self.last_bytes.get_mut(last_idx).ok_or(LazErr)?;
        let md = self.models.get_mut(self.last_ctx).ok_or(LazErr)?;
        for ((byte_slot, model), dec_opt) in
            lb.iter_mut().zip(md.iter_mut()).zip(self.decs.iter_mut())
        {
            if let Some(dec) = dec_opt {
                let diff = dec.decode_symbol(model)? as u8;
                *byte_slot = byte_slot.wrapping_add(diff);
            }
        }
        out.copy_from_slice(lb);
        Ok(())
    }
}

enum LayeredField<'a> {
    Point14(Box<Point14Decoder<'a>>),
    Rgb14(Box<Rgb14Decoder<'a>>),
    Nir14(Box<Nir14Decoder<'a>>),
    WavePacket14(Box<WavePacket14Decoder<'a>>),
    Byte14(Box<Byte14Decoder<'a>>),
}

impl<'a> LayeredField<'a> {
    fn size(&self) -> usize {
        match self {
            Self::Point14(_) => Point6::SIZE,
            Self::Rgb14(_) => RgbValue::SIZE,
            Self::Nir14(_) => 2,
            Self::WavePacket14(_) => WavePacketValue::SIZE,
            Self::Byte14(d) => d.count,
        }
    }

    fn init_first(
        &mut self,
        cursor: &mut SliceCursor<'a>,
        out: &mut [u8],
        ctx: &mut usize,
    ) -> LazResult<()> {
        match self {
            Self::Point14(d) => d.init_first(cursor, out, ctx),
            Self::Rgb14(d) => d.init_first(cursor, out, *ctx),
            Self::Nir14(d) => d.init_first(cursor, out, *ctx),
            Self::WavePacket14(d) => d.init_first(cursor, out, *ctx),
            Self::Byte14(d) => d.init_first(cursor, out, *ctx),
        }
    }

    fn read_sizes(&mut self, cursor: &mut SliceCursor<'a>) -> LazResult<()> {
        match self {
            Self::Point14(d) => d.read_sizes(cursor),
            Self::Rgb14(d) => {
                d.layer_size = cursor.u32_le()?;
                Ok(())
            }
            Self::Nir14(d) => {
                d.layer_size = cursor.u32_le()?;
                Ok(())
            }
            Self::WavePacket14(d) => {
                d.layer_size = cursor.u32_le()?;
                Ok(())
            }
            Self::Byte14(d) => d.read_sizes(cursor),
        }
    }

    fn read_layers(&mut self, cursor: &mut SliceCursor<'a>) -> LazResult<()> {
        match self {
            Self::Point14(d) => d.read_layers(cursor),
            Self::Rgb14(d) => {
                d.dec = opt_layer(cursor, d.layer_size)?;
                Ok(())
            }
            Self::Nir14(d) => {
                d.dec = opt_layer(cursor, d.layer_size)?;
                Ok(())
            }
            Self::WavePacket14(d) => {
                d.dec = opt_layer(cursor, d.layer_size)?;
                Ok(())
            }
            Self::Byte14(d) => d.read_layers(cursor),
        }
    }

    fn decode(&mut self, out: &mut [u8], ctx: &mut usize) -> LazResult<()> {
        match self {
            Self::Point14(d) => d.decode(out, ctx),
            Self::Rgb14(d) => d.decode(out, *ctx),
            Self::Nir14(d) => d.decode(out, *ctx),
            Self::WavePacket14(d) => d.decode(out, *ctx),
            Self::Byte14(d) => d.decode(out, *ctx),
        }
    }
}

fn build_layered_fields<'a>(items: &[(u16, u16, u16)]) -> LazResult<Vec<LayeredField<'a>>> {
    let mut fields = Vec::with_capacity(items.len() + 1);
    for &(item_type, size, version) in items {
        if version != 3 && version != 4 {
            return Err(LazErr);
        }
        match item_type {
            10 if size == 30 => {
                fields.push(LayeredField::Point14(Box::new(Point14Decoder::new(
                    version,
                ))));
            }
            11 if size == 6 => {
                fields.push(LayeredField::Rgb14(Box::new(Rgb14Decoder::new(version))));
            }
            12 if size == 8 => {
                fields.push(LayeredField::Rgb14(Box::new(Rgb14Decoder::new(version))));
                fields.push(LayeredField::Nir14(Box::new(Nir14Decoder::new(version))));
            }
            13 if size == 29 => {
                fields.push(LayeredField::WavePacket14(Box::new(
                    WavePacket14Decoder::new(version),
                )));
            }
            14 if size > 0 => {
                fields.push(LayeredField::Byte14(Box::new(Byte14Decoder::new(
                    version,
                    usize::from(size),
                ))));
            }
            _ => return Err(LazErr),
        }
    }
    Ok(fields)
}

/// レイヤ圧縮された 1 チャンクを読み、展開した点数を返す。
pub(super) fn decompress_layered_chunk<'a>(
    items: &[(u16, u16, u16)],
    mut cursor: SliceCursor<'a>,
    record_size: usize,
    max_points: usize,
    out: &mut [u8],
    cx: &mut Cx<'_>,
) -> Result<(SliceCursor<'a>, usize), Error> {
    if max_points == 0 {
        return Ok((cursor, 0));
    }
    let mut fields = build_layered_fields(items).map_err(|_| Error::Invalid)?;
    let first_rec = out.get_mut(..record_size).ok_or(Error::Invalid)?;
    cx.check(Kind::Work, 1)?;
    let mut ctx = 0usize;
    let mut off = 0usize;
    for field in &mut fields {
        let len = field.size();
        let dst = first_rec.get_mut(off..off + len).ok_or(Error::Invalid)?;
        field
            .init_first(&mut cursor, dst, &mut ctx)
            .map_err(|_| Error::Invalid)?;
        off += len;
    }

    let chunk_count_u32 = cursor.u32_le().map_err(|_| Error::Invalid)?;
    let chunk_count = usize::try_from(chunk_count_u32).map_err(|_| Error::Invalid)?;
    if chunk_count == 0 {
        return Err(Error::Invalid);
    }
    let target_points = chunk_count.min(max_points);

    for field in &mut fields {
        field.read_sizes(&mut cursor).map_err(|_| Error::Invalid)?;
    }
    for field in &mut fields {
        field.read_layers(&mut cursor).map_err(|_| Error::Invalid)?;
    }

    for pt_idx in 1..target_points {
        cx.check(Kind::Work, 1)?;
        let rec_start = pt_idx * record_size;
        let rec = out
            .get_mut(rec_start..rec_start + record_size)
            .ok_or(Error::Invalid)?;
        ctx = 0;
        let mut field_off = 0usize;
        for field in &mut fields {
            let len = field.size();
            let dst = rec
                .get_mut(field_off..field_off + len)
                .ok_or(Error::Invalid)?;
            field.decode(dst, &mut ctx).map_err(|_| Error::Invalid)?;
            field_off += len;
        }
    }

    Ok((cursor, target_points))
}
