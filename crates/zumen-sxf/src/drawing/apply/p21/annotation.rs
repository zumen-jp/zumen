//! SXF の STEP AP202 サブセット編の文字: 文字の表記、配置、範囲、書式を別の実体にする。
//! 共有した元の実体は変更せず、表記が指す実体を付け替える。

use core::ops::Range;

use zumen_live::RecordSlice;

use super::{Arguments, P21, Slot, Value, real, text};
use crate::Record;

/// 文字の実体を組むのに使う、書き方の基本の操作。バッチと文書エンジンが同じ実体を同じ順に足すために使う
pub(in crate::drawing) trait Build {
    /// 実体を足し、その番号を返す
    fn instance(&mut self, body: Vec<u8>) -> Option<u64>;
    /// 用紙の点を足す
    fn point(&mut self, x: f64, y: f64) -> Option<u64>;
    /// 色のコードの実体。なければ足す
    fn color(&mut self, code: i64) -> Option<u64>;
    /// 字体のコード（0 から）の実体
    fn font(&self, at: usize) -> Option<u64>;
    fn slot(&self, id: u64, name: &str, at: usize) -> Option<Slot>;
    fn reference(&self, slot: Slot) -> Option<u64>;
    fn list(&self, slot: Slot) -> Option<Vec<u64>>;
    fn set_list(&mut self, slot: Slot, list: Vec<u64>);
    fn set_arg(&mut self, slot: Slot, value: String);
    /// 指す所から外した実体。どこからも指されなくなれば除く
    fn loosen(&mut self, id: u64);
    /// 表記の実体の、見え方の並びと対象の引数の場所
    fn occurrence(&self, record: usize) -> Option<(Slot, Slot)>;
    fn record_id(&self, record: usize) -> Option<u64>;
    /// 要素をレイヤのコードのレイヤに移す
    fn layer(&mut self, id: u64, code: i64) -> Option<()>;
    /// 引数を 1 つ置き換えた実体の本体（`#n=` と `;` を除く）
    fn render_with(&mut self, slot: Slot, value: String) -> Option<Vec<u8>>;
    /// 実体の元の文を読む
    fn with_instance(&self, id: u64, f: &mut dyn FnMut(RecordSlice<'_>, &Record)) -> Option<()>;
}

impl Build for P21<'_> {
    fn instance(&mut self, body: Vec<u8>) -> Option<u64> {
        P21::instance(self, body)
    }

    fn point(&mut self, x: f64, y: f64) -> Option<u64> {
        P21::point(self, x, y)
    }

    fn color(&mut self, code: i64) -> Option<u64> {
        P21::color(self, code)
    }

    fn font(&self, at: usize) -> Option<u64> {
        self.fonts.get(at).copied()
    }

    fn slot(&self, id: u64, name: &str, at: usize) -> Option<Slot> {
        P21::slot(self, id, name, at)
    }

    fn reference(&self, slot: Slot) -> Option<u64> {
        P21::reference(self, slot)
    }

    fn list(&self, slot: Slot) -> Option<Vec<u64>> {
        P21::list(self, slot)
    }

    fn set_list(&mut self, slot: Slot, list: Vec<u64>) {
        self.lists.insert(slot, list);
    }

    fn set_arg(&mut self, slot: Slot, value: String) {
        self.args.insert(slot, value);
    }

    fn loosen(&mut self, id: u64) {
        self.loose.push(id);
    }

    fn occurrence(&self, record: usize) -> Option<(Slot, Slot)> {
        P21::occurrence(self, record)
    }

    fn record_id(&self, record: usize) -> Option<u64> {
        self.file.records().get(record)?.id()
    }

    fn layer(&mut self, id: u64, code: i64) -> Option<()> {
        P21::layer(self, id, code)
    }

    fn render_with(&mut self, slot: Slot, value: String) -> Option<Vec<u8>> {
        let prior = self.args.insert(slot, value);
        let rendered = self.render(slot.0);
        if let Some(prior) = prior {
            self.args.insert(slot, prior);
        } else {
            self.args.remove(&slot);
        }
        let rendered = rendered?;
        let id = self.file.records().get(slot.0)?.id()?;
        let start = format!("#{id}=").len();
        Some(
            rendered
                .get(start..rendered.len().checked_sub(1)?)?
                .to_vec(),
        )
    }

    fn with_instance(&self, id: u64, f: &mut dyn FnMut(RecordSlice<'_>, &Record)) -> Option<()> {
        let record = self.file.records().get(self.instances.get(id)?)?;
        f(
            RecordSlice {
                base: 0,
                bytes: &self.file.src,
            },
            record,
        );
        Some(())
    }
}

impl P21<'_> {
    pub(super) fn text_shape(&mut self, record: usize, args: &Arguments) -> Option<()> {
        text_shape(self, record, args)
    }

    pub(super) fn text_color(&mut self, styles: Slot, code: i64) -> Option<()> {
        text_color(self, styles, code)
    }

    pub(super) fn text_literal(&mut self, args: &Arguments) -> Option<u64> {
        text_literal(self, args)
    }

    pub(super) fn text_style(
        &mut self,
        args: &Arguments,
        color: i64,
        old: Option<u64>,
    ) -> Option<u64> {
        text_style(self, args, color, old)
    }
}

/// 文字の表記・配置・範囲の実体を足す
pub(in crate::drawing) fn text_literal<B: Build + ?Sized>(
    this: &mut B,
    args: &Arguments,
) -> Option<u64> {
    let number = |at: usize| {
        args.values
            .get(at)?
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    };
    let code = args.values.get(2)?.parse::<usize>().ok()?.checked_sub(1)?;
    let font = this.font(code)?;
    let content = text::p21_string(args.values.get(3)?)?;
    let (x, y, height, width, angle) = (number(4)?, number(5)?, number(6)?, number(7)?, number(9)?);
    let anchor = args.values.get(11)?.parse::<usize>().ok()?.checked_sub(1)?;
    let row = *["baseline", "middleline", "topline"].get(anchor / 3)?;
    let column = *["left", "centre", "right"].get(anchor % 3)?;
    let offset = match anchor / 3 {
        0 => height / 2.0,
        1 => -height / 2.0,
        _ => -height,
    };
    let (sin, cos) = (libm::sin(angle.to_radians()), libm::cos(angle.to_radians()));
    let point = this.point(x - sin * offset, y + cos * offset)?;
    let axis =
        this.instance(format!("DIRECTION('',({},{}))", real(cos)?, real(sin)?).into_bytes())?;
    let placement =
        this.instance(format!("AXIS2_PLACEMENT_2D('',#{point},#{axis})").into_bytes())?;
    let extent = this
        .instance(format!("PLANAR_EXTENT('',{},{})", real(width)?, real(height)?).into_bytes())?;
    let direction = match args.values.get(12)?.as_str() {
        "1" => "RIGHT",
        "2" => "DOWN",
        _ => return None,
    };
    this.instance(format!("TEXT_LITERAL_WITH_EXTENT('$$SXF_{row} {column}',{content},#{placement},'baseline {column}',.{direction}.,#{font},#{extent})").into_bytes())
}

/// 文字の見え方の実体を足す。`old` は元の見え方で、知らない部分と値を写す
pub(in crate::drawing) fn text_style<B: Build + ?Sized>(
    this: &mut B,
    args: &Arguments,
    color: i64,
    old: Option<u64>,
) -> Option<u64> {
    let colour = this.color(color)?;
    let appearance = match old {
        Some(old) => {
            let style = this.slot(old, "TEXT_STYLE", 1)?;
            let appearance = this.reference(style)?;
            let slot = this.slot(appearance, "TEXT_STYLE_FOR_DEFINED_FONT", 0)?;
            clone_with(this, slot, format!("#{colour}"))?
        }
        None => this.instance(format!("TEXT_STYLE_FOR_DEFINED_FONT(#{colour})").into_bytes())?,
    };
    let number = |at: usize| {
        args.values
            .get(at)?
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    };
    let height = real(number(6)?)?;
    let width = real(number(7)?)?;
    let spacing = real(number(8)?)?;
    let angle = real(number(9)?.to_radians())?;
    let slant = real(number(10)?.to_radians())?;
    let box_values = [
        ("BOX_HEIGHT", height),
        ("BOX_WIDTH", width),
        ("BOX_SLANT_ANGLE", slant),
        ("BOX_ROTATE_ANGLE", angle),
    ];
    let mut written = None;
    if let Some(old) = old {
        this.with_instance(old, &mut |rs, record| {
            written = old_style(rs, record, appearance, &box_values, &spacing);
        })?;
    }
    let (mut body, had_text, had_box, had_spacing) = match old {
        Some(_) => written?,
        None => (vec![b'('], false, false, false),
    };
    if !had_text {
        body.extend_from_slice(format!("TEXT_STYLE('',#{appearance})").as_bytes());
    }
    if !had_box {
        let values = box_values
            .iter()
            .map(|(name, value)| format!("{name}({value})"))
            .collect::<Vec<_>>()
            .join(",");
        body.extend_from_slice(
            format!("TEXT_STYLE_WITH_BOX_CHARACTERISTICS(({values}))").as_bytes(),
        );
    }
    if !had_spacing {
        body.extend_from_slice(
            format!("TEXT_STYLE_WITH_SPACING(LENGTH_MEASURE({spacing}))").as_bytes(),
        );
    }
    body.push(b')');
    this.instance(body)
}

/// 元の見え方の部分を、書き直す値を入れて写す。書いた本体と、文字・枠・間隔の部分があったか
fn old_style(
    rs: RecordSlice<'_>,
    record: &Record,
    appearance: u64,
    box_values: &[(&str, String); 4],
    spacing: &str,
) -> Option<(Vec<u8>, bool, bool, bool)> {
    let mut body = vec![b'('];
    let mut had_text = false;
    let mut had_box = false;
    let mut had_spacing = false;
    for part in record.parts() {
        let name = slice_in(rs, part.name())?;
        body.extend_from_slice(name);
        body.push(b'(');
        for (at, value) in part.args().iter().enumerate() {
            if at > 0 {
                body.push(b',');
            }
            if name.eq_ignore_ascii_case(b"TEXT_STYLE") && at == 1 {
                body.extend_from_slice(format!("#{appearance}").as_bytes());
                had_text = true;
            } else if name.eq_ignore_ascii_case(b"TEXT_STYLE_WITH_BOX_CHARACTERISTICS") && at == 0 {
                let Value::List(items) = value else {
                    return None;
                };
                body.push(b'(');
                let mut present = [false; 4];
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        body.push(b',');
                    }
                    let known = match item {
                        Value::Typed(part) => {
                            box_values.iter().enumerate().find(|(_, (name, _))| {
                                slice_in(rs, part.name()).is_some_and(|found| {
                                    found.eq_ignore_ascii_case(name.as_bytes())
                                })
                            })
                        }
                        _ => None,
                    };
                    if let Some((index, (name, value))) = known {
                        *present.get_mut(index)? = true;
                        body.extend_from_slice(format!("{name}({value})").as_bytes());
                    } else {
                        write_value_slice(rs, item, &mut body);
                    }
                }
                let mut count = items.len();
                for ((name, value), present) in box_values.iter().zip(present) {
                    if !present {
                        if count > 0 {
                            body.push(b',');
                        }
                        body.extend_from_slice(format!("{name}({value})").as_bytes());
                        count += 1;
                    }
                }
                body.push(b')');
                had_box = true;
            } else if name.eq_ignore_ascii_case(b"TEXT_STYLE_WITH_SPACING") && at == 0 {
                body.extend_from_slice(format!("LENGTH_MEASURE({spacing})").as_bytes());
                had_spacing = true;
            } else {
                write_value_slice(rs, value, &mut body);
            }
        }
        body.push(b')');
    }
    Some((body, had_text, had_box, had_spacing))
}

/// 文字の表記の実体の形を変える。表記・見え方の実体を足して付け替え、レイヤを移す
pub(in crate::drawing) fn text_shape<B: Build + ?Sized>(
    this: &mut B,
    record: usize,
    args: &Arguments,
) -> Option<()> {
    let (styles, item) = this.occurrence(record)?;
    let was = this.reference(item)?;
    let literal = text_literal(this, args)?;
    let color = args.values.get(1)?.parse::<i64>().ok()?;
    text_assignments(this, styles, &mut |this, old| {
        text_style(this, args, color, Some(old))
    })?;
    this.set_arg(item, format!("#{literal}"));
    this.loosen(was);
    let id = this.record_id(record)?;
    this.layer(id, args.values.first()?.parse().ok()?)?;
    Some(())
}

/// 文字の色を変える。見え方の実体を写して付け替える
pub(in crate::drawing) fn text_color<B: Build + ?Sized>(
    this: &mut B,
    styles: Slot,
    code: i64,
) -> Option<()> {
    let colour = this.color(code)?;
    text_assignments(this, styles, &mut |this, old| {
        let style_slot = this.slot(old, "TEXT_STYLE", 1)?;
        let appearance = this.reference(style_slot)?;
        let color_slot = this.slot(appearance, "TEXT_STYLE_FOR_DEFINED_FONT", 0)?;
        let appearance = clone_with(this, color_slot, format!("#{colour}"))?;
        clone_with(this, style_slot, format!("#{appearance}"))
    })
}

fn text_assignments<B: Build + ?Sized>(
    this: &mut B,
    styles: Slot,
    change: &mut dyn FnMut(&mut B, u64) -> Option<u64>,
) -> Option<()> {
    let mut list = this.list(styles)?;
    let mut changed = false;
    for assignment in &mut list {
        let slot = this.slot(*assignment, "PRESENTATION_STYLE_ASSIGNMENT", 0)?;
        let mut items = this.list(slot)?;
        let mut touched = false;
        for style in &mut items {
            if this.slot(*style, "TEXT_STYLE", 0).is_some() {
                if changed {
                    return None;
                }
                *style = change(this, *style)?;
                changed = true;
                touched = true;
            }
        }
        if touched {
            let values = items
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(",");
            let new =
                this.instance(format!("PRESENTATION_STYLE_ASSIGNMENT(({values}))").into_bytes())?;
            this.loosen(*assignment);
            *assignment = new;
        }
    }
    if !changed {
        return None;
    }
    this.set_list(styles, list);
    Some(())
}

/// 引数を 1 つ置き換えた写しの実体を足す
fn clone_with<B: Build + ?Sized>(this: &mut B, slot: Slot, value: String) -> Option<u64> {
    let body = this.render_with(slot, value)?;
    this.instance(body)
}

/// 元のバイト列の中の範囲
pub(in crate::drawing) fn slice_in(rs: RecordSlice<'_>, range: Range<usize>) -> Option<&[u8]> {
    let s = range.start.checked_sub(rs.base)?;
    let e = range.end.checked_sub(rs.base)?;
    rs.bytes.get(s..e)
}

/// 決定 0169 の「書き直し方」に従い、元の値と型の名前をそのまま写す。文字列の 8 ビットのバイトも残す。
pub(in crate::drawing) fn write_value_slice(rs: RecordSlice<'_>, value: &Value, out: &mut Vec<u8>) {
    let bytes = |r: Range<usize>| slice_in(rs, r).unwrap_or_default();
    let mut delimited = |mark: u8, raw: &[u8]| {
        out.push(mark);
        out.extend_from_slice(raw);
        out.push(mark);
    };
    match value {
        Value::Null => out.push(b'$'),
        Value::Derived => out.push(b'*'),
        Value::Ref(id) => out.extend_from_slice(format!("#{id}").as_bytes()),
        Value::Enum(range) => delimited(b'.', bytes(range.clone())),
        Value::Text { raw, .. } => delimited(b'\'', bytes(raw.clone())),
        Value::Number(range) | Value::Token(range) => out.extend_from_slice(bytes(range.clone())),
        Value::Binary(range) => delimited(b'"', bytes(range.clone())),
        Value::Typed(part) => {
            out.extend_from_slice(bytes(part.name()));
            write_list_slice(rs, part.args(), out);
        }
        Value::List(items) => write_list_slice(rs, items, out),
    }
}

fn write_list_slice(rs: RecordSlice<'_>, items: &[Value], out: &mut Vec<u8>) {
    out.push(b'(');
    for (at, item) in items.iter().enumerate() {
        if at > 0 {
            out.push(b',');
        }
        write_value_slice(rs, item, out);
    }
    out.push(b')');
}
