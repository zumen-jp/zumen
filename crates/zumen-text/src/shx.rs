//! 呼び出し側が渡した AutoCAD の形のフォント（`.shx`）を読み、ベクター命令を解釈する（#875）。
//!
//! Autodesk の公開ヘルプ（Compiling Shape/Font Files・Big Font Descriptions・Extended Big Font）と
//! 寛容なライセンスの実装を根拠に、`AutoCAD-86 shapes 1.0/1.1`・`AutoCAD-86 unifont 1.0`・
//! `AutoCAD-86 bigfont 1.0` のヘッダー・索引・ベクター命令を読む。
//!
//! 受け取ったファイルは信用しない。位置と長さは `u64` か溢れを確かめて扱い、索引の読み取りは [`Cx`] の
//! 予算で数える。字形の命令の解釈では、部分シェイプの入れ子を [`MAX_DEPTH`]、位置のスタックを
//! [`MAX_STACK`]、1 字形あたりの命令数を [`MAX_STEPS`] に制限する。三角関数は `libm` を使う。

use zumen_base::{Cx, Kind, Stop};

use crate::layout::{FontMetrics, Glyph, Glyphs, VerticalGlyph};

/// ヘッダーの先頭から DOS EOF（`0x1A`）を探す最大バイト数
const MAX_HEADER_BYTES: usize = 1024;
/// 部分シェイプ呼び出し（命令 `7`）の再帰の深さの上限
pub const MAX_DEPTH: usize = 8;
/// 位置のスタック（命令 `5`・`6`）の深さの上限（AutoCAD の仕様は 4）
const MAX_STACK: usize = 4;
/// 1 字形の命令・頂点の評価回数の上限（`zumen-scene` の事前予約 `2^16` 未満）
const MAX_STEPS: u32 = 16_384;
/// `unifont` の字形数の上限
const MAX_UNIFONT_ENTRIES: u32 = 65_537;
/// 既定の基準線から上の高さ
const DEFAULT_ABOVE: u8 = 8;
/// 既定の基準線から下の深さ
const DEFAULT_BELOW: u8 = 2;

/// `.shx` ファイルの種別
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShxKind {
    /// `AutoCAD-86 shapes 1.0` または `1.1`（単バイト文字・形のファイル）
    Shapes,
    /// `AutoCAD-86 unifont 1.0`（Unicode 2 バイト文字）
    Unifont,
    /// `AutoCAD-86 bigfont 1.0`（Shift_JIS などの多バイト文字）
    Bigfont,
}

/// `.shx` のヘッダーと索引を読めない理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShxError {
    /// 先頭の識別子が `.shx` のどれでもない
    Magic,
    /// ヘッダー・索引・範囲が壊れている
    Malformed,
    /// 予算か中断の上限に達した
    Budget(Stop),
}

/// 1 字形のベクター命令を解釈できない理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShxStrokeError {
    /// 命令の引数の不足、0 除算、スタックや部分シェイプの上限超過
    Malformed,
    /// 未対応の命令（`0x0F` など）
    UnsupportedOpcode(u8),
}

/// 字形の開いた折れ線・円弧の 1 区間。座標はフォントの単位
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShxSegment {
    /// 直線の区間（終点）
    Line([f64; 2]),
    /// 円弧の区間（中心・半径・開始角・掃引角・終点。角度はラジアン、反時計回りが正）
    Arc {
        /// 円の中心
        center: [f64; 2],
        /// 半径（0 より大きい）
        radius: f64,
        /// 開始角（ラジアン）
        start: f64,
        /// 掃引角（ラジアン、時計回りは負）
        sweep: f64,
        /// 終点
        end: [f64; 2],
    },
}

/// ペンを下ろしてから上げるまでの 1 本の連続したストローク
#[derive(Clone, Debug, PartialEq)]
pub struct ShxPath {
    /// 始点
    pub start: [f64; 2],
    /// 続く区間（1 つ以上）
    pub segments: Vec<ShxSegment>,
}

/// 1 字形のストロークと終点・外形
#[derive(Clone, Debug, PartialEq)]
pub struct ShxGlyphPaths {
    /// ストロークの列
    pub paths: Vec<ShxPath>,
    /// 命令終了時のペン位置
    pub end: [f64; 2],
    /// ストローク全体の外形 `[min_x, min_y, max_x, max_y]`（線がなければ `None`）
    pub bounds: Option<[f64; 4]>,
    /// 末尾のペン上げ移動などで送りが明示されたか
    pub explicit_advance: bool,
}

/// 読んだ `.shx` フォント
#[derive(Clone, Debug)]
pub struct ShxFont<'a> {
    kind: ShxKind,
    /// 文字・形の番号で昇順に並べた `(code, bytecode)`
    entries: Vec<(u16, &'a [u8])>,
    metrics: FontMetrics,
    /// 縦書き命令（`0x0E`）の切り替えを持つか
    dual_orientation: bool,
    /// 常時縦書きのフォントか
    default_vertical: bool,
    /// 拡張ビッグフォントか
    is_extended: bool,
    /// 縦書き兼用のビッグフォント（`modes == 2`）か
    vertical_dual_bigfont: bool,
    /// 情報ブロック `0` を持たない純粋な形のファイルか
    plain_shape: bool,
    /// 既定の字幅
    default_width: f64,
    fallback: Glyph,
}

impl<'a> ShxFont<'a> {
    /// バイト列から `.shx` フォントを読む
    pub fn parse(bytes: &'a [u8], cx: &mut Cx<'_>) -> Result<Self, ShxError> {
        cx.check(Kind::InputBytes, bytes.len() as u64)
            .map_err(ShxError::Budget)?;
        let search_len = bytes.len().min(MAX_HEADER_BYTES);
        let head = bytes.get(..search_len).ok_or(ShxError::Magic)?;
        let Some(eof_pos) = head.iter().position(|&b| b == 0x1a) else {
            cx.check(Kind::Work, search_len as u64)
                .map_err(ShxError::Budget)?;
            return Err(ShxError::Magic);
        };
        cx.check(Kind::Work, (eof_pos + 1) as u64)
            .map_err(ShxError::Budget)?;
        let raw_header = head.get(..eof_pos).ok_or(ShxError::Magic)?;
        let header = trim_header(raw_header);
        let kind = match header {
            b"AutoCAD-86 shapes 1.0" | b"AutoCAD-86 shapes 1.1" => ShxKind::Shapes,
            b"AutoCAD-86 unifont 1.0" => ShxKind::Unifont,
            b"AutoCAD-86 bigfont 1.0" => ShxKind::Bigfont,
            _ => return Err(ShxError::Magic),
        };
        let rest = bytes.get(eof_pos + 1..).ok_or(ShxError::Malformed)?;
        match kind {
            ShxKind::Shapes => parse_shapes(rest, cx),
            ShxKind::Unifont => parse_unifont(rest, cx),
            ShxKind::Bigfont => parse_bigfont(bytes, eof_pos + 1, cx),
        }
    }

    /// フォントの種別
    pub fn kind(&self) -> ShxKind {
        self.kind
    }

    /// 情報ブロック `0` を持たない形のファイルか
    pub fn is_plain_shape(&self) -> bool {
        self.plain_shape
    }

    /// 文字に対応する `.shx` 内のコード（ビッグフォントでは Shift_JIS の 2 バイト値など）を返す
    pub fn code_of(&self, ch: char) -> Option<u16> {
        match self.kind {
            ShxKind::Unifont => {
                let code = u16::try_from(u32::from(ch)).ok()?;
                (code != 0 && self.bytecode(code).is_some()).then_some(code)
            }
            ShxKind::Shapes => {
                let cp = u32::from(ch);
                if cp != 0 && cp <= 0xff {
                    let code = cp as u16;
                    if self.bytecode(code).is_some() {
                        return Some(code);
                    }
                }
                // DXF の `%%d`・`%%p`・`%%c` を復号した記号に対する AutoCAD 標準 SHX の番号
                let candidates: &[u16] = match ch {
                    '\u{00B0}' => &[127, 0xB0],
                    '\u{00B1}' => &[128, 0xB1],
                    '\u{2205}' | '\u{00D8}' | '\u{00F8}' => &[129, 0xD8, 0xF8],
                    _ => &[],
                };
                candidates
                    .iter()
                    .copied()
                    .find(|&code| self.bytecode(code).is_some())
            }
            ShxKind::Bigfont => {
                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                let (cow, _, unmapped) = encoding_rs::SHIFT_JIS.encode(s);
                if unmapped {
                    return None;
                }
                let code = match cow.as_ref() {
                    [b0] if *b0 != 0 => u16::from(*b0),
                    [b0, b1] => u16::from_be_bytes([*b0, *b1]),
                    _ => return None,
                };
                (code != 0 && self.bytecode(code).is_some()).then_some(code)
            }
        }
    }

    /// 文字 `ch` のストロークを評価する。フォントに文字がなければ `Ok(None)`
    pub fn strokes(
        &self,
        ch: char,
        vertical: bool,
    ) -> Result<Option<ShxGlyphPaths>, ShxStrokeError> {
        let Some(code) = self.code_of(ch) else {
            return Ok(None);
        };
        self.strokes_by_code(code, vertical).map(Some)
    }

    /// 番号 `code` のストロークを評価する。`vertical` が真で縦書き対応フォントなら、列の中心上端を
    /// `(advance / 2, 0)` に揃えた座標で返す
    pub fn strokes_by_code(
        &self,
        code: u16,
        vertical: bool,
    ) -> Result<ShxGlyphPaths, ShxStrokeError> {
        let mut out = self.evaluate_code(code, vertical)?;
        if vertical && self.supports_vertical() {
            let horiz_adv = self
                .evaluate_code(code, false)
                .map(|h| self.advance_from(&h))
                .unwrap_or(self.default_width);
            shift_paths(&mut out, horiz_adv * 0.5, 0.0);
        }
        Ok(out)
    }

    fn supports_vertical(&self) -> bool {
        self.dual_orientation || self.default_vertical || self.vertical_dual_bigfont
    }

    fn bytecode(&self, code: u16) -> Option<&'a [u8]> {
        self.entries
            .binary_search_by_key(&code, |&(c, _)| c)
            .ok()
            .and_then(|idx| self.entries.get(idx).map(|&(_, b)| b))
    }

    fn advance_from(&self, paths: &ShxGlyphPaths) -> f64 {
        let [end_x, _] = paths.end;
        if end_x > 1e-9 || (paths.explicit_advance && end_x >= 0.0) {
            end_x.max(0.0)
        } else if let Some([_, _, right, _]) = paths.bounds {
            right.max(self.default_width * 0.5)
        } else {
            self.default_width
        }
    }

    fn evaluate_code(&self, code: u16, vertical: bool) -> Result<ShxGlyphPaths, ShxStrokeError> {
        let bytes = self.bytecode(code).ok_or(ShxStrokeError::Malformed)?;
        let mut steps = 0u32;
        let initial_pen_down = self.initial_pen_down(code, bytes);
        self.run_bytecode(bytes, initial_pen_down, true, vertical, 0, &mut steps)
    }

    fn initial_pen_down(&self, code: u16, bytes: &[u8]) -> bool {
        if self.kind != ShxKind::Bigfont || code <= 0xff {
            return true;
        }
        matches!(bytes.first(), Some(0x01 | 0x0a..=0x0d | 0x10..=0xff))
    }

    fn should_skip_vertical_command(&self, vertical: bool) -> bool {
        match self.kind {
            ShxKind::Bigfont => {
                if self.vertical_dual_bigfont {
                    !vertical
                } else {
                    true
                }
            }
            ShxKind::Shapes | ShxKind::Unifont => {
                if self.dual_orientation {
                    !vertical
                } else {
                    !self.default_vertical
                }
            }
        }
    }

    fn run_bytecode(
        &self,
        data: &[u8],
        initial_pen_down: bool,
        flush_on_end: bool,
        vertical: bool,
        depth: usize,
        steps: &mut u32,
    ) -> Result<ShxGlyphPaths, ShxStrokeError> {
        if depth > MAX_DEPTH {
            return Err(ShxStrokeError::Malformed);
        }
        let mut state = EvalState::new(initial_pen_down);
        let mut pc = 0usize;
        while let Some(&cmd) = data.get(pc) {
            tick(steps)?;
            pc += 1;
            if cmd == 0x00 {
                state.finalize_advance();
                if flush_on_end {
                    state.flush();
                }
                state.pen_down = false;
                break;
            }
            match cmd {
                0x01 => {
                    state.pending_advance = false;
                    state.set_pen_down();
                }
                0x02 => {
                    state.set_pen_up();
                }
                0x03 => {
                    state.pending_advance = false;
                    let div = read_u8(data, &mut pc)?;
                    if div == 0 {
                        return Err(ShxStrokeError::Malformed);
                    }
                    state.scale /= f64::from(div);
                    if !state.scale.is_finite() || state.scale <= 0.0 {
                        return Err(ShxStrokeError::Malformed);
                    }
                }
                0x04 => {
                    state.pending_advance = false;
                    let mul = read_u8(data, &mut pc)?;
                    if mul == 0 {
                        return Err(ShxStrokeError::Malformed);
                    }
                    state.scale *= f64::from(mul);
                    if !state.scale.is_finite() || state.scale <= 0.0 {
                        return Err(ShxStrokeError::Malformed);
                    }
                }
                0x05 => {
                    state.pending_advance = false;
                    if state.stack.len() >= MAX_STACK {
                        return Err(ShxStrokeError::Malformed);
                    }
                    state.stack.push(state.cur);
                }
                0x06 => {
                    state.pending_advance = false;
                    let Some(popped) = state.stack.pop() else {
                        return Err(ShxStrokeError::Malformed);
                    };
                    state.flush();
                    state.cur = popped;
                    if state.pen_down {
                        state.active_start = Some(state.cur);
                    }
                }
                0x07 => {
                    state.pending_advance = false;
                    self.exec_subshape(data, &mut pc, &mut state, vertical, depth, steps)?;
                }
                0x08 => {
                    let dx = f64::from(read_i8(data, &mut pc)?) * state.scale;
                    let dy = f64::from(read_i8(data, &mut pc)?) * state.scale;
                    state.move_by(dx, dy);
                }
                0x09 => loop {
                    tick(steps)?;
                    let dx_raw = read_i8(data, &mut pc)?;
                    let dy_raw = read_i8(data, &mut pc)?;
                    if dx_raw == 0 && dy_raw == 0 {
                        break;
                    }
                    state.move_by(
                        f64::from(dx_raw) * state.scale,
                        f64::from(dy_raw) * state.scale,
                    );
                },
                0x0a => {
                    state.pending_advance = false;
                    let r_raw = read_u8(data, &mut pc)?;
                    let flags = read_i8(data, &mut pc)?;
                    exec_octant_arc(&mut state, r_raw, flags)?;
                }
                0x0b => {
                    state.pending_advance = false;
                    let start_off = read_u8(data, &mut pc)?;
                    let end_off = read_u8(data, &mut pc)?;
                    let hr = read_u8(data, &mut pc)?;
                    let lr = read_u8(data, &mut pc)?;
                    let flags = read_i8(data, &mut pc)?;
                    exec_fractional_arc(&mut state, start_off, end_off, hr, lr, flags)?;
                }
                0x0c => {
                    state.pending_advance = false;
                    let dx = f64::from(read_i8(data, &mut pc)?) * state.scale;
                    let dy = f64::from(read_i8(data, &mut pc)?) * state.scale;
                    let bulge = read_i8(data, &mut pc)?;
                    exec_bulge_arc(&mut state, dx, dy, bulge)?;
                }
                0x0d => {
                    state.pending_advance = false;
                    loop {
                        tick(steps)?;
                        let dx_raw = read_i8(data, &mut pc)?;
                        let dy_raw = read_i8(data, &mut pc)?;
                        if dx_raw == 0 && dy_raw == 0 {
                            break;
                        }
                        let bulge = read_i8(data, &mut pc)?;
                        let scale = state.scale;
                        exec_bulge_arc(
                            &mut state,
                            f64::from(dx_raw) * scale,
                            f64::from(dy_raw) * scale,
                            bulge,
                        )?;
                    }
                }
                0x0e => {
                    state.pending_advance = false;
                    if self.should_skip_vertical_command(vertical) {
                        self.skip_one_command(data, &mut pc, steps)?;
                    }
                }
                0x0f => {
                    return Err(ShxStrokeError::UnsupportedOpcode(0x0f));
                }
                vector => {
                    state.pending_advance = false;
                    let len = f64::from(vector >> 4) * state.scale;
                    let [vx, vy] = direction_vector(vector & 0x0f);
                    state.line_by(vx * len, vy * len)?;
                }
            }
        }
        state.finalize_advance();
        if flush_on_end {
            state.flush();
        }
        Ok(state.into_paths())
    }

    fn exec_subshape(
        &self,
        data: &[u8],
        pc: &mut usize,
        state: &mut EvalState,
        vertical: bool,
        depth: usize,
        steps: &mut u32,
    ) -> Result<(), ShxStrokeError> {
        if depth + 1 > MAX_DEPTH {
            return Err(ShxStrokeError::Malformed);
        }
        state.flush();
        match self.kind {
            ShxKind::Shapes => {
                let sub_code = u16::from(read_u8(data, pc)?);
                if sub_code == 0 {
                    return Ok(());
                }
                let sub_bytes = self.bytecode(sub_code).ok_or(ShxStrokeError::Malformed)?;
                let mut sub =
                    self.run_bytecode(sub_bytes, true, true, vertical, depth + 1, steps)?;
                transform_paths(
                    &mut sub,
                    state.scale,
                    state.scale,
                    state.cur[0],
                    state.cur[1],
                );
                state.paths.extend(sub.paths);
                state.cur = sub.end;
                if sub.explicit_advance {
                    state.explicit_advance = true;
                }
                if state.pen_down {
                    state.active_start = Some(state.cur);
                }
            }
            ShxKind::Unifont => {
                let hi = read_u8(data, pc)?;
                let lo = read_u8(data, pc)?;
                let sub_code = u16::from_be_bytes([hi, lo]);
                if sub_code == 0 {
                    return Ok(());
                }
                let sub_bytes = self.bytecode(sub_code).ok_or(ShxStrokeError::Malformed)?;
                let mut sub =
                    self.run_bytecode(sub_bytes, state.pen_down, true, vertical, depth + 1, steps)?;
                transform_paths(
                    &mut sub,
                    state.scale,
                    state.scale,
                    state.cur[0],
                    state.cur[1],
                );
                if !sub.paths.is_empty() {
                    state.paths.extend(sub.paths);
                    state.cur = sub.end;
                }
                if state.pen_down {
                    state.active_start = Some(state.cur);
                }
            }
            ShxKind::Bigfont => {
                let first = read_u8(data, pc)?;
                if self.vertical_dual_bigfont && first == 0x8e {
                    return Ok(());
                }
                if self.vertical_dual_bigfont && first == 0x8f {
                    let _ = read_u8(data, pc)?;
                    return Ok(());
                }
                if first == 0 {
                    // 拡張ビッグフォント形式: 7, 0, prim_hi, prim_lo, ox, oy, [w], h
                    let hi = read_u8(data, pc)?;
                    let lo = read_u8(data, pc)?;
                    let sub_code = u16::from_be_bytes([hi, lo]);
                    let ox = f64::from(read_i8(data, pc)?) * state.scale;
                    let oy = f64::from(read_i8(data, pc)?) * state.scale;
                    let (target_w, target_h) = if self.is_extended {
                        let w = f64::from(read_u8(data, pc)?) * state.scale;
                        let h = f64::from(read_u8(data, pc)?) * state.scale;
                        (w, h)
                    } else {
                        let h = f64::from(read_u8(data, pc)?) * state.scale;
                        (h, h)
                    };
                    if sub_code == 0 {
                        return Ok(());
                    }
                    let sub_bytes = self.bytecode(sub_code).ok_or(ShxStrokeError::Malformed)?;
                    let init_pen = self.initial_pen_down(sub_code, sub_bytes);
                    let mut sub =
                        self.run_bytecode(sub_bytes, init_pen, true, vertical, depth + 1, steps)?;
                    let (sx, sy) = if let Some([min_x, min_y, max_x, max_y]) = sub.bounds {
                        let bw = max_x - min_x;
                        let bh = max_y - min_y;
                        (
                            if bw > 1e-9 { target_w / bw } else { 1.0 },
                            if bh > 1e-9 { target_h / bh } else { 1.0 },
                        )
                    } else {
                        let cap = self.metrics.cap_height.max(1.0);
                        (target_w / cap, target_h / cap)
                    };
                    transform_paths(&mut sub, sx, sy, ox, oy);
                    state.paths.extend(sub.paths);
                } else {
                    let sub_code = u16::from(first);
                    let sub_bytes = self.bytecode(sub_code).ok_or(ShxStrokeError::Malformed)?;
                    let init_pen = self.initial_pen_down(sub_code, sub_bytes);
                    let mut sub =
                        self.run_bytecode(sub_bytes, init_pen, false, vertical, depth + 1, steps)?;
                    transform_paths(
                        &mut sub,
                        state.scale,
                        state.scale,
                        state.cur[0],
                        state.cur[1],
                    );
                    state.paths.extend(sub.paths);
                    if sub_code == 2 {
                        let cell_w = sub.end[0] - state.cur[0];
                        if cell_w > state.cur[0] {
                            state.cur[0] = cell_w;
                        }
                        state.explicit_advance = true;
                    }
                }
            }
        }
        Ok(())
    }

    fn skip_one_command(
        &self,
        data: &[u8],
        pc: &mut usize,
        steps: &mut u32,
    ) -> Result<(), ShxStrokeError> {
        let cmd = read_u8(data, pc)?;
        match cmd {
            0x00..=0x02 | 0x05 | 0x06 | 0x0e => Ok(()),
            0x03 | 0x04 => {
                let _ = read_u8(data, pc)?;
                Ok(())
            }
            0x07 => match self.kind {
                ShxKind::Shapes => {
                    let _ = read_u8(data, pc)?;
                    Ok(())
                }
                ShxKind::Unifont => {
                    let _ = read_u8(data, pc)?;
                    let _ = read_u8(data, pc)?;
                    Ok(())
                }
                ShxKind::Bigfont => {
                    let b = read_u8(data, pc)?;
                    if self.vertical_dual_bigfont && b == 0x8e {
                        Ok(())
                    } else if self.vertical_dual_bigfont && b == 0x8f {
                        let _ = read_u8(data, pc)?;
                        Ok(())
                    } else if b == 0 {
                        let extra = if self.is_extended { 6 } else { 5 };
                        advance_bytes(data, pc, extra)
                    } else {
                        Ok(())
                    }
                }
            },
            0x08 | 0x0a => advance_bytes(data, pc, 2),
            0x09 => loop {
                tick(steps)?;
                let dx = read_i8(data, pc)?;
                let dy = read_i8(data, pc)?;
                if dx == 0 && dy == 0 {
                    break Ok(());
                }
            },
            0x0b => advance_bytes(data, pc, 5),
            0x0c => advance_bytes(data, pc, 3),
            0x0d => loop {
                tick(steps)?;
                let dx = read_i8(data, pc)?;
                let dy = read_i8(data, pc)?;
                if dx == 0 && dy == 0 {
                    break Ok(());
                }
                let _ = read_i8(data, pc)?;
            },
            0x0f => Err(ShxStrokeError::UnsupportedOpcode(0x0f)),
            0x10..=0xff => Ok(()),
        }
    }
}

impl Glyphs for ShxFont<'_> {
    fn metrics(&self) -> FontMetrics {
        self.metrics
    }

    fn glyph(&self, ch: char) -> Option<Glyph> {
        let code = self.code_of(ch)?;
        let paths = self.evaluate_code(code, false).ok()?;
        Some(Glyph {
            advance: self.advance_from(&paths),
            bounds: paths.bounds,
        })
    }

    fn fallback(&self) -> Glyph {
        self.fallback
    }

    fn vertical(&self, ch: char) -> Option<VerticalGlyph> {
        if !self.supports_vertical() {
            return self
                .glyph(ch)
                .map(|glyph| VerticalGlyph::upright(glyph, &self.metrics));
        }
        let code = self.code_of(ch)?;
        let horiz = self.evaluate_code(code, false).ok()?;
        let horiz_adv = self.advance_from(&horiz);
        let mut vert = self.evaluate_code(code, true).ok()?;
        shift_paths(&mut vert, horiz_adv * 0.5, 0.0);
        let vert_adv = if vert.end[1] < -1e-9 {
            -vert.end[1]
        } else {
            self.metrics.ascent + self.metrics.descent
        };
        Some(VerticalGlyph {
            glyph: Glyph {
                advance: horiz_adv,
                bounds: vert.bounds,
            },
            advance: vert_adv,
            top: 0.0,
            substituted: true,
        })
    }
}

struct EvalState {
    cur: [f64; 2],
    pen_down: bool,
    scale: f64,
    stack: Vec<[f64; 2]>,
    active_start: Option<[f64; 2]>,
    active_segments: Vec<ShxSegment>,
    paths: Vec<ShxPath>,
    explicit_advance: bool,
    pending_advance: bool,
}

impl EvalState {
    fn new(pen_down: bool) -> Self {
        Self {
            cur: [0.0, 0.0],
            pen_down,
            scale: 1.0,
            stack: Vec::new(),
            active_start: pen_down.then_some([0.0, 0.0]),
            active_segments: Vec::new(),
            paths: Vec::new(),
            explicit_advance: false,
            pending_advance: false,
        }
    }

    fn set_pen_down(&mut self) {
        if !self.pen_down {
            self.pen_down = true;
            self.active_start = Some(self.cur);
        }
    }

    fn set_pen_up(&mut self) {
        self.flush();
        self.pen_down = false;
    }

    fn flush(&mut self) {
        if let Some(start) = self.active_start.take() {
            if !self.active_segments.is_empty() {
                self.paths.push(ShxPath {
                    start,
                    segments: core::mem::take(&mut self.active_segments),
                });
            }
        } else {
            self.active_segments.clear();
        }
    }

    fn move_by(&mut self, dx: f64, dy: f64) {
        let next = [self.cur[0] + dx, self.cur[1] + dy];
        if self.pen_down {
            self.pending_advance = false;
            if self.active_start.is_none() {
                self.active_start = Some(self.cur);
            }
            self.active_segments.push(ShxSegment::Line(next));
        } else {
            self.pending_advance = true;
        }
        self.cur = next;
    }

    fn line_by(&mut self, dx: f64, dy: f64) -> Result<(), ShxStrokeError> {
        let next = [self.cur[0] + dx, self.cur[1] + dy];
        if !next[0].is_finite() || !next[1].is_finite() {
            return Err(ShxStrokeError::Malformed);
        }
        if self.pen_down {
            if self.active_start.is_none() {
                self.active_start = Some(self.cur);
            }
            self.active_segments.push(ShxSegment::Line(next));
        }
        self.cur = next;
        Ok(())
    }

    fn push_arc(
        &mut self,
        center: [f64; 2],
        radius: f64,
        start: f64,
        sweep: f64,
        end: [f64; 2],
    ) -> Result<(), ShxStrokeError> {
        if ![center[0], center[1], radius, start, sweep, end[0], end[1]]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(ShxStrokeError::Malformed);
        }
        if self.pen_down {
            if self.active_start.is_none() {
                self.active_start = Some(self.cur);
            }
            if radius <= 1e-12 || sweep.abs() <= 1e-12 {
                self.active_segments.push(ShxSegment::Line(end));
            } else {
                self.active_segments.push(ShxSegment::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                    end,
                });
            }
        }
        self.cur = end;
        Ok(())
    }

    fn finalize_advance(&mut self) {
        if !self.pending_advance {
            return;
        }
        let has_ink = !self.paths.is_empty() || !self.active_segments.is_empty();
        if self.cur[0].abs() > 1e-6 || !has_ink {
            self.explicit_advance = true;
        }
    }

    fn into_paths(self) -> ShxGlyphPaths {
        let bounds = compute_bounds(&self.paths);
        ShxGlyphPaths {
            paths: self.paths,
            end: self.cur,
            bounds,
            explicit_advance: self.explicit_advance,
        }
    }
}

fn tick(steps: &mut u32) -> Result<(), ShxStrokeError> {
    *steps = steps.saturating_add(1);
    if *steps > MAX_STEPS {
        Err(ShxStrokeError::Malformed)
    } else {
        Ok(())
    }
}

fn read_u8(data: &[u8], pc: &mut usize) -> Result<u8, ShxStrokeError> {
    let b = *data.get(*pc).ok_or(ShxStrokeError::Malformed)?;
    *pc += 1;
    Ok(b)
}

fn read_i8(data: &[u8], pc: &mut usize) -> Result<i8, ShxStrokeError> {
    read_u8(data, pc).map(|b| b as i8)
}

fn advance_bytes(data: &[u8], pc: &mut usize, count: usize) -> Result<(), ShxStrokeError> {
    let next = pc.checked_add(count).ok_or(ShxStrokeError::Malformed)?;
    if next > data.len() {
        return Err(ShxStrokeError::Malformed);
    }
    *pc = next;
    Ok(())
}

fn direction_vector(dir: u8) -> [f64; 2] {
    match dir & 0x0f {
        0x0 => [1.0, 0.0],
        0x1 => [1.0, 0.5],
        0x2 => [1.0, 1.0],
        0x3 => [0.5, 1.0],
        0x4 => [0.0, 1.0],
        0x5 => [-0.5, 1.0],
        0x6 => [-1.0, 1.0],
        0x7 => [-1.0, 0.5],
        0x8 => [-1.0, 0.0],
        0x9 => [-1.0, -0.5],
        0xa => [-1.0, -1.0],
        0xb => [-0.5, -1.0],
        0xc => [0.0, -1.0],
        0xd => [0.5, -1.0],
        0xe => [1.0, -1.0],
        _ => [1.0, -0.5],
    }
}

fn exec_octant_arc(state: &mut EvalState, r_raw: u8, flags: i8) -> Result<(), ShxStrokeError> {
    let radius = f64::from(r_raw) * state.scale;
    if radius <= 0.0 {
        return Ok(());
    }
    let bits = flags as u8;
    let start_octant = (bits >> 4) & 0x07;
    let count_octants = {
        let c = bits & 0x07;
        if c == 0 { 8 } else { c }
    };
    let pi_4 = core::f64::consts::FRAC_PI_4;
    let start = pi_4 * f64::from(start_octant);
    let sweep = if flags < 0 {
        -pi_4 * f64::from(count_octants)
    } else {
        pi_4 * f64::from(count_octants)
    };
    let center = [
        state.cur[0] - radius * libm::cos(start),
        state.cur[1] - radius * libm::sin(start),
    ];
    let end = if count_octants == 8 {
        state.cur
    } else {
        let end_angle = start + sweep;
        [
            center[0] + radius * libm::cos(end_angle),
            center[1] + radius * libm::sin(end_angle),
        ]
    };
    state.push_arc(center, radius, start, sweep, end)
}

fn exec_fractional_arc(
    state: &mut EvalState,
    start_off: u8,
    end_off: u8,
    hr: u8,
    lr: u8,
    flags: i8,
) -> Result<(), ShxStrokeError> {
    let radius = f64::from(u16::from(hr) * 256 + u16::from(lr)) * state.scale;
    if radius <= 0.0 {
        return Ok(());
    }
    let bits = flags as u8;
    let n1 = (bits >> 4) & 0x07;
    let mut n2 = bits & 0x07;
    if n2 == 0 {
        n2 = 8;
    }
    if end_off != 0 {
        n2 = n2.saturating_sub(1);
    }
    let sign = if flags < 0 { -1.0 } else { 1.0 };
    let pi_4 = core::f64::consts::FRAC_PI_4;
    let start = pi_4 * f64::from(n1) + sign * pi_4 * (f64::from(start_off) / 256.0);
    let end_angle = pi_4 * f64::from(n1)
        + sign * pi_4 * f64::from(n2)
        + sign * pi_4 * (f64::from(end_off) / 256.0);
    let sweep = end_angle - start;
    let center = [
        state.cur[0] - radius * libm::cos(start),
        state.cur[1] - radius * libm::sin(start),
    ];
    let end = [
        center[0] + radius * libm::cos(end_angle),
        center[1] + radius * libm::sin(end_angle),
    ];
    state.push_arc(center, radius, start, sweep, end)
}

fn exec_bulge_arc(
    state: &mut EvalState,
    dx: f64,
    dy: f64,
    bulge_raw: i8,
) -> Result<(), ShxStrokeError> {
    let clamped = f64::from(bulge_raw).max(-127.0);
    let chord = libm::hypot(dx, dy);
    if bulge_raw == 0 || chord <= 1e-12 {
        return state.line_by(dx, dy);
    }
    let b = clamped / 127.0;
    let end = [state.cur[0] + dx, state.cur[1] + dy];
    let sweep = 4.0 * libm::atan(b);
    let radius = chord * (1.0 + b * b) / (4.0 * b.abs());
    let factor = (1.0 - b * b) / (4.0 * b);
    let mx = 0.5 * (state.cur[0] + end[0]);
    let my = 0.5 * (state.cur[1] + end[1]);
    let center = [mx - dy * factor, my + dx * factor];
    let start = libm::atan2(state.cur[1] - center[1], state.cur[0] - center[0]);
    state.push_arc(center, radius, start, sweep, end)
}

fn compute_bounds(paths: &[ShxPath]) -> Option<[f64; 4]> {
    let mut acc: Option<[f64; 4]> = None;
    let mut include = |[x, y]: [f64; 2]| {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        acc = Some(match acc {
            Some([min_x, min_y, max_x, max_y]) => {
                [min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y)]
            }
            None => [x, y, x, y],
        });
    };
    for path in paths {
        include(path.start);
        for seg in &path.segments {
            match *seg {
                ShxSegment::Line(q) => include(q),
                ShxSegment::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                    end,
                } => {
                    include(end);
                    let tau = core::f64::consts::TAU;
                    let half_pi = core::f64::consts::FRAC_PI_2;
                    for k in 0..4 {
                        let angle = half_pi * f64::from(k);
                        if angle_in_sweep(angle, start, sweep, tau) {
                            include([
                                center[0] + radius * libm::cos(angle),
                                center[1] + radius * libm::sin(angle),
                            ]);
                        }
                    }
                }
            }
        }
    }
    acc
}

fn angle_in_sweep(angle: f64, start: f64, sweep: f64, tau: f64) -> bool {
    if sweep.abs() >= tau - 1e-12 {
        return true;
    }
    if sweep >= 0.0 {
        let mut diff = libm::remquo(angle - start, tau).0;
        if diff < 0.0 {
            diff += tau;
        }
        diff <= sweep + 1e-12
    } else {
        let mut diff = libm::remquo(start - angle, tau).0;
        if diff < 0.0 {
            diff += tau;
        }
        diff <= (-sweep) + 1e-12
    }
}

fn shift_paths(paths: &mut ShxGlyphPaths, dx: f64, dy: f64) {
    transform_paths(paths, 1.0, 1.0, dx, dy);
}

fn transform_paths(paths: &mut ShxGlyphPaths, sx: f64, sy: f64, ox: f64, oy: f64) {
    let map = |[x, y]: [f64; 2]| [x * sx + ox, y * sy + oy];
    let uniform = (sx - sy).abs() <= 1e-9 && sx > 0.0;
    for path in &mut paths.paths {
        let mut prev = path.start;
        path.start = map(path.start);
        let old_segments = core::mem::take(&mut path.segments);
        for seg in old_segments {
            match seg {
                ShxSegment::Line(q) => {
                    prev = q;
                    path.segments.push(ShxSegment::Line(map(q)));
                }
                ShxSegment::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                    end,
                } => {
                    if uniform {
                        path.segments.push(ShxSegment::Arc {
                            center: map(center),
                            radius: radius * sx,
                            start,
                            sweep,
                            end: map(end),
                        });
                    } else {
                        // 縦横の倍率が異なる拡張ビッグフォントの部分シェイプは、円弧を折れ線にして写す
                        let n =
                            ((sweep.abs() / (core::f64::consts::PI / 12.0)) as usize).clamp(4, 32);
                        for step in 1..=n {
                            let a = start + sweep * (step as f64 / n as f64);
                            let pt = if step == n {
                                end
                            } else {
                                [
                                    center[0] + radius * libm::cos(a),
                                    center[1] + radius * libm::sin(a),
                                ]
                            };
                            path.segments.push(ShxSegment::Line(map(pt)));
                        }
                    }
                    let _ = prev;
                    prev = end;
                }
            }
        }
    }
    paths.end = map(paths.end);
    paths.bounds = compute_bounds(&paths.paths);
}

fn trim_header(mut bytes: &[u8]) -> &[u8] {
    while let Some((&last, head)) = bytes.split_last() {
        if matches!(last, b'\r' | b'\n' | b' ') {
            bytes = head;
        } else {
            break;
        }
    }
    bytes
}

fn u16_le(bytes: &[u8], at: usize) -> Option<u16> {
    let slice = bytes.get(at..at.checked_add(2)?)?;
    let &[b0, b1] = slice else {
        return None;
    };
    Some(u16::from_le_bytes([b0, b1]))
}

fn u32_le(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at.checked_add(4)?)?;
    let &[b0, b1, b2, b3] = slice else {
        return None;
    };
    Some(u32::from_le_bytes([b0, b1, b2, b3]))
}

fn split_name_and_bytecode(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|&b| b == 0) {
        Some(pos) => bytes.get(pos + 1..).unwrap_or_default(),
        None => bytes,
    }
}

fn sort_dedup_entries<'a>(raw: Vec<(u16, &'a [u8])>) -> Vec<(u16, &'a [u8])> {
    let mut indexed: Vec<(u16, usize, &'a [u8])> = raw
        .into_iter()
        .enumerate()
        .map(|(idx, (code, data))| (code, idx, data))
        .collect();
    indexed.sort_by_key(|&(code, idx, _)| (code, idx));
    let mut out: Vec<(u16, &'a [u8])> = Vec::with_capacity(indexed.len());
    for (code, _, data) in indexed {
        if let Some(last) = out.last_mut()
            && last.0 == code
        {
            last.1 = data;
            continue;
        }
        out.push((code, data));
    }
    out
}

struct FontMeta {
    above: u8,
    below: u8,
    modes: u8,
    is_extended: bool,
    vertical_dual_bigfont: bool,
    plain_shape: bool,
    width_override: Option<u8>,
}

fn build_font<'a>(kind: ShxKind, entries: Vec<(u16, &'a [u8])>, meta: FontMeta) -> ShxFont<'a> {
    let ascent = f64::from(meta.above.max(1));
    let descent = f64::from(meta.below);
    let units_per_em = ascent + descent;
    let default_width = meta
        .width_override
        .filter(|&w| w > 0)
        .map_or(units_per_em, f64::from);
    let dual_orientation = meta.modes == 2;
    let default_vertical = meta.modes != 0 && meta.modes != 2;
    ShxFont {
        kind,
        entries: sort_dedup_entries(entries),
        metrics: FontMetrics {
            units_per_em,
            ascent,
            descent,
            cap_height: ascent,
        },
        dual_orientation,
        default_vertical,
        is_extended: meta.is_extended,
        vertical_dual_bigfont: meta.vertical_dual_bigfont,
        plain_shape: meta.plain_shape,
        default_width,
        fallback: Glyph {
            advance: default_width,
            bounds: None,
        },
    }
}

fn parse_shapes<'a>(rest: &'a [u8], cx: &mut Cx<'_>) -> Result<ShxFont<'a>, ShxError> {
    let _start = u16_le(rest, 0).ok_or(ShxError::Malformed)?;
    let _end = u16_le(rest, 2).ok_or(ShxError::Malformed)?;
    let count = u16_le(rest, 4).ok_or(ShxError::Malformed)?;
    if count == 0 {
        return Err(ShxError::Malformed);
    }
    let count_u64 = u64::from(count);
    cx.check(Kind::Records, count_u64)
        .map_err(ShxError::Budget)?;
    cx.check(Kind::Work, count_u64.saturating_mul(8))
        .map_err(ShxError::Budget)?;
    cx.check(Kind::DecodedBytes, count_u64.saturating_mul(24))
        .map_err(ShxError::Budget)?;
    let dir_bytes = usize::from(count)
        .checked_mul(4)
        .ok_or(ShxError::Malformed)?;
    let mut data_cursor = 6usize.checked_add(dir_bytes).ok_or(ShxError::Malformed)?;
    if data_cursor > rest.len() {
        return Err(ShxError::Malformed);
    }

    let mut entries = Vec::with_capacity(usize::from(count));
    let mut info_block: Option<&'a [u8]> = None;
    for idx in 0..usize::from(count) {
        let dir_at = 6 + idx * 4;
        let code = u16_le(rest, dir_at).ok_or(ShxError::Malformed)?;
        let len = usize::from(u16_le(rest, dir_at + 2).ok_or(ShxError::Malformed)?);
        if len == 0 {
            continue;
        }
        let end = data_cursor.checked_add(len).ok_or(ShxError::Malformed)?;
        let raw = rest.get(data_cursor..end).ok_or(ShxError::Malformed)?;
        data_cursor = end;
        if code == 0 {
            info_block = Some(raw);
        } else {
            entries.push((code, split_name_and_bytecode(raw)));
        }
    }

    let (above, below, modes, plain_shape) = match info_block {
        Some(info) => {
            let term = info
                .iter()
                .position(|&b| matches!(b, b'\0' | b'\r' | b'\n'));
            match term.and_then(|i| info.get(i + 1..i + 4)) {
                Some(&[up, down, m]) => (if up == 0 { DEFAULT_ABOVE } else { up }, down, m, false),
                _ => (DEFAULT_ABOVE, DEFAULT_BELOW, 0, false),
            }
        }
        None => (DEFAULT_ABOVE, 0, 0, true),
    };

    Ok(build_font(
        ShxKind::Shapes,
        entries,
        FontMeta {
            above,
            below,
            modes,
            is_extended: false,
            vertical_dual_bigfont: false,
            plain_shape,
            width_override: None,
        },
    ))
}

fn parse_unifont<'a>(rest: &'a [u8], cx: &mut Cx<'_>) -> Result<ShxFont<'a>, ShxError> {
    let count = u32_le(rest, 0).ok_or(ShxError::Malformed)?;
    if count == 0 || count > MAX_UNIFONT_ENTRIES {
        return Err(ShxError::Malformed);
    }
    let info_len = usize::from(u16_le(rest, 4).ok_or(ShxError::Malformed)?);
    let info_end = 6usize.checked_add(info_len).ok_or(ShxError::Malformed)?;
    let info = rest.get(6..info_end).ok_or(ShxError::Malformed)?;
    let glyph_count = count - 1;
    let count_u64 = u64::from(glyph_count);
    cx.check(Kind::Records, count_u64 + 1)
        .map_err(ShxError::Budget)?;
    cx.check(Kind::Work, (count_u64 + 1).saturating_mul(8))
        .map_err(ShxError::Budget)?;
    cx.check(Kind::DecodedBytes, count_u64.saturating_mul(24))
        .map_err(ShxError::Budget)?;

    let (above, below, modes) = match info
        .iter()
        .position(|&b| b == 0)
        .and_then(|pos| info.get(pos + 1..pos + 4))
    {
        Some(&[up, down, m]) => (if up == 0 { DEFAULT_ABOVE } else { up }, down, m),
        _ => (DEFAULT_ABOVE, DEFAULT_BELOW, 0),
    };

    let mut cursor = info_end;
    let mut entries = Vec::with_capacity(glyph_count as usize);
    for _ in 0..glyph_count {
        let code = u16_le(rest, cursor).ok_or(ShxError::Malformed)?;
        let len = usize::from(u16_le(rest, cursor + 2).ok_or(ShxError::Malformed)?);
        cursor = cursor.checked_add(4).ok_or(ShxError::Malformed)?;
        if len == 0 {
            continue;
        }
        let end = cursor.checked_add(len).ok_or(ShxError::Malformed)?;
        let raw = rest.get(cursor..end).ok_or(ShxError::Malformed)?;
        cursor = end;
        let bytecode = split_name_and_bytecode(raw);
        if code != 0 && !bytecode.is_empty() {
            entries.push((code, bytecode));
        }
    }

    Ok(build_font(
        ShxKind::Unifont,
        entries,
        FontMeta {
            above,
            below,
            modes,
            is_extended: false,
            vertical_dual_bigfont: false,
            plain_shape: false,
            width_override: None,
        },
    ))
}

fn parse_bigfont<'a>(
    full: &'a [u8],
    body_offset: usize,
    cx: &mut Cx<'_>,
) -> Result<ShxFont<'a>, ShxError> {
    let rest = full.get(body_offset..).ok_or(ShxError::Malformed)?;
    let _item_len = u16_le(rest, 0).ok_or(ShxError::Malformed)?;
    let count = u16_le(rest, 2).ok_or(ShxError::Malformed)?;
    let num_ranges = u16_le(rest, 4).ok_or(ShxError::Malformed)?;
    if count == 0 {
        return Err(ShxError::Malformed);
    }
    let count_u64 = u64::from(count);
    let ranges_u64 = u64::from(num_ranges);
    cx.check(Kind::Records, count_u64)
        .map_err(ShxError::Budget)?;
    cx.check(Kind::Work, (count_u64 + ranges_u64).saturating_mul(8))
        .map_err(ShxError::Budget)?;
    cx.check(Kind::DecodedBytes, count_u64.saturating_mul(24))
        .map_err(ShxError::Budget)?;

    let ranges_bytes = usize::from(num_ranges)
        .checked_mul(4)
        .ok_or(ShxError::Malformed)?;
    let dir_start = 6usize
        .checked_add(ranges_bytes)
        .ok_or(ShxError::Malformed)?;
    let dir_bytes = usize::from(count)
        .checked_mul(8)
        .ok_or(ShxError::Malformed)?;
    let dir_end = dir_start
        .checked_add(dir_bytes)
        .ok_or(ShxError::Malformed)?;
    if dir_end > rest.len() {
        return Err(ShxError::Malformed);
    }

    let mut entries = Vec::with_capacity(usize::from(count));
    let mut info_block: Option<&'a [u8]> = None;
    for idx in 0..usize::from(count) {
        let entry_at = dir_start + idx * 8;
        let code = u16_le(rest, entry_at).ok_or(ShxError::Malformed)?;
        let len = u16_le(rest, entry_at + 2).ok_or(ShxError::Malformed)?;
        let offset = u32_le(rest, entry_at + 4).ok_or(ShxError::Malformed)?;
        if code == 0 && len == 0 && offset == 0 {
            continue;
        }
        if len == 0 {
            continue;
        }
        let start = usize::try_from(offset).map_err(|_| ShxError::Malformed)?;
        let end = start
            .checked_add(usize::from(len))
            .ok_or(ShxError::Malformed)?;
        let raw = full.get(start..end).ok_or(ShxError::Malformed)?;
        if code == 0 {
            info_block = Some(raw);
        } else {
            entries.push((code, strip_bigfont_prefix(code, raw)));
        }
    }

    let meta = match info_block {
        Some(info) => parse_bigfont_info(info),
        None => FontMeta {
            above: DEFAULT_ABOVE,
            below: DEFAULT_BELOW,
            modes: 0,
            is_extended: false,
            vertical_dual_bigfont: false,
            plain_shape: false,
            width_override: None,
        },
    };

    Ok(build_font(ShxKind::Bigfont, entries, meta))
}

fn strip_bigfont_prefix(code: u16, raw: &[u8]) -> &[u8] {
    if code > 0xff {
        let [hi, lo] = code.to_be_bytes();
        if let Some(&[b0, b1, ..]) = raw.get(..)
            && b0 == hi
            && b1 == lo
        {
            let after = raw.get(2..).unwrap_or_default();
            return if after.first() == Some(&0) && after.len() > 1 {
                after.get(1..).unwrap_or_default()
            } else {
                after
            };
        }
    }
    if raw.first() == Some(&0) && raw.len() > 1 {
        raw.get(1..).unwrap_or_default()
    } else {
        raw
    }
}

fn parse_bigfont_info(info: &[u8]) -> FontMeta {
    let default = FontMeta {
        above: DEFAULT_ABOVE,
        below: DEFAULT_BELOW,
        modes: 0,
        is_extended: false,
        vertical_dual_bigfont: false,
        plain_shape: false,
        width_override: None,
    };
    let Some(nul_pos) = info.iter().position(|&b| b == 0) else {
        return default;
    };
    let mut tail = info.get(nul_pos + 1..).unwrap_or_default();
    while let Some((&0, rest)) = tail.split_first() {
        tail = rest;
    }
    match *tail {
        [height, 0, modes, width, _, ..] => FontMeta {
            above: if height == 0 { DEFAULT_ABOVE } else { height },
            below: 0,
            modes,
            is_extended: true,
            vertical_dual_bigfont: false,
            plain_shape: false,
            width_override: Some(width),
        },
        [height, 0, modes, width] if width > 0 && width != height => FontMeta {
            above: if height == 0 { DEFAULT_ABOVE } else { height },
            below: 0,
            modes,
            is_extended: true,
            vertical_dual_bigfont: false,
            plain_shape: false,
            width_override: Some(width),
        },
        [above, below, modes, _] => FontMeta {
            above: if above == 0 { DEFAULT_ABOVE } else { above },
            below,
            modes,
            is_extended: false,
            vertical_dual_bigfont: false,
            plain_shape: false,
            width_override: None,
        },
        [above, modes, _] => FontMeta {
            above: if above == 0 { DEFAULT_ABOVE } else { above },
            below: 0,
            modes,
            is_extended: modes == 2,
            vertical_dual_bigfont: modes == 2,
            plain_shape: false,
            width_override: None,
        },
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    fn test_cx() -> (Budget, Diagnostics, CancelToken) {
        (
            Budget::new(Limits::default()),
            Diagnostics::new(),
            CancelToken::new(),
        )
    }

    fn build_shapes_font(modes: u8, shapes: &[(u16, &[u8])]) -> Vec<u8> {
        let mut out = b"AutoCAD-86 shapes 1.0\r\n\x1a".to_vec();
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&255u16.to_le_bytes());
        let total = (shapes.len() + 1) as u16;
        out.extend_from_slice(&total.to_le_bytes());

        // 0 番: フォント情報（above = 10, below = 2, modes）
        let info = [b't', b'e', b's', b't', 0, 10, 2, modes];
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(info.len() as u16).to_le_bytes());
        for &(code, bc) in shapes {
            // 先頭の空の名前 `\0` + 命令列
            let len = (1 + bc.len()) as u16;
            out.extend_from_slice(&code.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
        }
        out.extend_from_slice(&info);
        for &(_, bc) in shapes {
            out.push(0);
            out.extend_from_slice(bc);
        }
        out
    }

    fn build_unifont(shapes: &[(u16, &[u8])]) -> Vec<u8> {
        let mut out = b"AutoCAD-86 unifont 1.0\r\n\x1a".to_vec();
        let count = (shapes.len() + 1) as u32;
        out.extend_from_slice(&count.to_le_bytes());
        let info = [b'u', b'n', b'i', 0, 12, 3, 2, 0, 0, 0];
        out.extend_from_slice(&(info.len() as u16).to_le_bytes());
        out.extend_from_slice(&info);
        for &(code, bc) in shapes {
            out.extend_from_slice(&code.to_le_bytes());
            out.extend_from_slice(&((1 + bc.len()) as u16).to_le_bytes());
            out.push(0);
            out.extend_from_slice(bc);
        }
        out
    }

    fn build_bigfont(shapes: &[(u16, &[u8])]) -> Vec<u8> {
        let mut out = b"AutoCAD-86 bigfont 1.0\r\n\x1a".to_vec();
        out.extend_from_slice(&8u16.to_le_bytes());
        let count = (shapes.len() + 1) as u16;
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        // Shift_JIS 第 1 バイト範囲: 0x81..=0x9f, 0xe0..=0xfc
        out.extend_from_slice(&0x81u16.to_le_bytes());
        out.extend_from_slice(&0x9fu16.to_le_bytes());
        out.extend_from_slice(&0xe0u16.to_le_bytes());
        out.extend_from_slice(&0xfcu16.to_le_bytes());

        let dir_pos = out.len();
        out.resize(dir_pos + usize::from(count) * 8, 0);

        let info = [b'j', b'p', 0, 10, 2, 0, 0];
        let info_off = out.len() as u32;
        out.extend_from_slice(&info);
        out[dir_pos..dir_pos + 2].copy_from_slice(&0u16.to_le_bytes());
        out[dir_pos + 2..dir_pos + 4].copy_from_slice(&(info.len() as u16).to_le_bytes());
        out[dir_pos + 4..dir_pos + 8].copy_from_slice(&info_off.to_le_bytes());

        for (idx, &(code, bc)) in shapes.iter().enumerate() {
            let off = out.len() as u32;
            let [hi, lo] = code.to_be_bytes();
            out.push(hi);
            out.push(lo);
            out.push(0);
            out.extend_from_slice(bc);
            let len = (3 + bc.len()) as u16;
            let entry = dir_pos + (idx + 1) * 8;
            out[entry..entry + 2].copy_from_slice(&code.to_le_bytes());
            out[entry + 2..entry + 4].copy_from_slice(&len.to_le_bytes());
            out[entry + 4..entry + 8].copy_from_slice(&off.to_le_bytes());
        }
        out
    }

    #[test]
    fn parses_single_byte_shapes_with_vectors_arcs_and_vertical_mode() {
        let (mut budget, mut diag, cancel) = test_cx();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        // 'L' (76): 縦書き時はペン上げで (-3, -10) へ移り、原点から上へ 10、戻って右へ 6、ペン上げで右へ 2、
        // 縦書き時はペン上げで (-5, -2) へ送る
        let l_bc: &[u8] = &[
            0x0e,
            0x02,
            0x0e,
            0x08,
            (-3i8) as u8,
            (-10i8) as u8,
            0x01,
            0xa4,
            0xac,
            0x60,
            0x02,
            0x20,
            0x0e,
            0x08,
            (-5i8) as u8,
            (-2i8) as u8,
            0x00,
        ];
        // 'O' (79): 半径 5 の八分円弧（完全な円）+ ペン上げ送り 12、およびバルジ円弧
        let o_bc: &[u8] = &[
            0x02,
            0x08,
            0,
            5,
            0x01,
            0x0a,
            5,
            0x40,
            0x02,
            0x08,
            12,
            (-5i8) as u8,
            0x00,
        ];
        // 'C' (67): バルジ円弧（0x0c）
        let c_bc: &[u8] = &[
            0x02,
            0x08,
            6,
            10,
            0x01,
            0x0c,
            0,
            (-10i8) as u8,
            127,
            0x02,
            0x08,
            8,
            0,
            0x00,
        ];
        let raw = build_shapes_font(
            2,
            &[
                (b'L'.into(), l_bc),
                (b'O'.into(), o_bc),
                (b'C'.into(), c_bc),
            ],
        );
        let font = ShxFont::parse(&raw, &mut cx).unwrap();

        let g_l = font.glyph('L').unwrap();
        assert!((g_l.advance - 8.0).abs() < 1e-9);
        assert_eq!(g_l.bounds, Some([0.0, 0.0, 6.0, 10.0]));

        let v_l = font.vertical('L').unwrap();
        assert!(v_l.substituted);
        assert!((v_l.advance - 12.0).abs() < 1e-9);

        let paths_o = font.strokes('O', false).unwrap().unwrap();
        assert_eq!(paths_o.paths.len(), 1);
        assert!(matches!(
            paths_o.paths[0].segments[0],
            ShxSegment::Arc { .. }
        ));
        let b_o = paths_o.bounds.unwrap();
        assert!((b_o[2] - b_o[0] - 10.0).abs() < 1e-6);

        let paths_c = font.strokes('C', false).unwrap().unwrap();
        assert_eq!(paths_c.paths.len(), 1);
    }

    #[test]
    fn parses_unifont_and_shift_jis_bigfont() {
        let (mut budget, mut diag, cancel) = test_cx();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        // Unicode '山' (U+5C71)
        let yama_bc: &[u8] = &[0x01, 0x84, 0x02, 0x08, 12, (-8i8) as u8, 0x00];
        let uni_bytes = build_unifont(&[(0x5c71, yama_bc)]);
        let uni = ShxFont::parse(&uni_bytes, &mut cx).unwrap();
        let g_uni = uni.glyph('山').unwrap();
        assert!((g_uni.advance - 12.0).abs() < 1e-9);
        assert!(uni.glyph('川').is_none());

        // Shift_JIS '図' = 0x907D
        let zu_bc: &[u8] = &[
            0x01,
            0x08,
            0,
            10,
            0x08,
            10,
            0,
            0x08,
            0,
            (-10i8) as u8,
            0x08,
            (-10i8) as u8,
            0,
            0x02,
            0x08,
            12,
            0,
            0x00,
        ];
        let big_bytes = build_bigfont(&[(0x907d, zu_bc)]);
        let big = ShxFont::parse(&big_bytes, &mut cx).unwrap();
        assert_eq!(big.code_of('図'), Some(0x907d));
        let g_big = big.glyph('図').unwrap();
        assert!((g_big.advance - 12.0).abs() < 1e-9);
        assert_eq!(g_big.bounds, Some([0.0, 0.0, 10.0, 10.0]));
    }

    #[test]
    fn enforces_subshape_recursion_limit_and_survives_corrupt_inputs() {
        let (mut budget, mut diag, cancel) = test_cx();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        // 自分自身（65 = 'A'）を呼ぶ無限再帰の部分シェイプ
        let cycle_bc: &[u8] = &[0x07, b'A', 0x00];
        let cycle_raw = build_shapes_font(0, &[(b'A'.into(), cycle_bc)]);
        let cycle_font = ShxFont::parse(&cycle_raw, &mut cx).unwrap();
        assert_eq!(
            cycle_font.strokes('A', false),
            Err(ShxStrokeError::Malformed)
        );
        assert!(cycle_font.glyph('A').is_none());

        // 未対応の命令 0x0F
        let bad_op_raw = build_shapes_font(0, &[(b'B'.into(), &[0x0f, 0x00])]);
        let bad_op_font = ShxFont::parse(&bad_op_raw, &mut cx).unwrap();
        assert_eq!(
            bad_op_font.strokes('B', false),
            Err(ShxStrokeError::UnsupportedOpcode(0x0f))
        );

        // 0 除算（0x03, 0）とスタック溢れ（0x05 × 5）
        let div0_raw = build_shapes_font(
            0,
            &[
                (b'D'.into(), &[0x03, 0x00, 0x00]),
                (b'S'.into(), &[0x05, 0x05, 0x05, 0x05, 0x05, 0x00]),
            ],
        );
        let div0_font = ShxFont::parse(&div0_raw, &mut cx).unwrap();
        assert_eq!(
            div0_font.strokes('D', false),
            Err(ShxStrokeError::Malformed)
        );
        assert_eq!(
            div0_font.strokes('S', false),
            Err(ShxStrokeError::Malformed)
        );

        // 途中で切れた破損入力でもパニックしない
        for cut in 0..cycle_raw.len() {
            let mut b = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1000)));
            let mut d = Diagnostics::new();
            let mut sub_cx = Cx::new(&mut b, &mut d, &cancel);
            let _ = ShxFont::parse(&cycle_raw[..cut], &mut sub_cx);
        }
    }
}
