//! SXF の STEP AP202 サブセット編の文字: 文字の表記、配置、範囲、書式を別の実体にする。
//! 共有した元の実体は変更せず、表記が指す実体を付け替える。

use super::{Arguments, P21, Slot, Value, real, text, write_value};

impl P21<'_> {
    pub(super) fn text_literal(&mut self, args: &Arguments) -> Option<u64> {
        let number = |at: usize| {
            args.values
                .get(at)?
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
        };
        let code = args.values.get(2)?.parse::<usize>().ok()?.checked_sub(1)?;
        let font = *self.fonts.get(code)?;
        let content = text::p21_string(args.values.get(3)?)?;
        let (x, y, height, width, angle) =
            (number(4)?, number(5)?, number(6)?, number(7)?, number(9)?);
        let anchor = args.values.get(11)?.parse::<usize>().ok()?.checked_sub(1)?;
        let row = *["baseline", "middleline", "topline"].get(anchor / 3)?;
        let column = *["left", "centre", "right"].get(anchor % 3)?;
        let offset = match anchor / 3 {
            0 => height / 2.0,
            1 => -height / 2.0,
            _ => -height,
        };
        let (sin, cos) = (libm::sin(angle.to_radians()), libm::cos(angle.to_radians()));
        let point = self.point(x - sin * offset, y + cos * offset)?;
        let axis = self.instance(format!("DIRECTION('',({},{}))", real(cos)?, real(sin)?))?;
        let placement = self.instance(format!("AXIS2_PLACEMENT_2D('',#{point},#{axis})"))?;
        let extent = self.instance(format!(
            "PLANAR_EXTENT('',{},{})",
            real(width)?,
            real(height)?
        ))?;
        let direction = match args.values.get(12)?.as_str() {
            "1" => "RIGHT",
            "2" => "DOWN",
            _ => return None,
        };
        self.instance(format!("TEXT_LITERAL_WITH_EXTENT('$$SXF_{row} {column}',{content},#{placement},'baseline {column}',.{direction}.,#{font},#{extent})"))
    }

    pub(super) fn text_style(
        &mut self,
        args: &Arguments,
        color: i64,
        old: Option<u64>,
    ) -> Option<u64> {
        let colour = self.color(color)?;
        let appearance = match old {
            Some(old) => {
                let style = self.slot(old, "TEXT_STYLE", 1)?;
                let appearance = self.reference(style)?;
                let slot = self.slot(appearance, "TEXT_STYLE_FOR_DEFINED_FONT", 0)?;
                self.clone_with(slot, format!("#{colour}"))?
            }
            None => self.instance(format!("TEXT_STYLE_FOR_DEFINED_FONT(#{colour})"))?,
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
        let mut body = vec![b'('];
        let mut had_text = false;
        let mut had_box = false;
        let mut had_spacing = false;
        if let Some(old) = old {
            let record = self.file.records().get(self.instances.get(old)?)?;
            for part in record.parts() {
                let name = self.file.bytes(part.name());
                body.extend_from_slice(name);
                body.push(b'(');
                for (at, value) in part.args().iter().enumerate() {
                    if at > 0 {
                        body.push(b',');
                    }
                    if name.eq_ignore_ascii_case(b"TEXT_STYLE") && at == 1 {
                        body.extend_from_slice(format!("#{appearance}").as_bytes());
                        had_text = true;
                    } else if name.eq_ignore_ascii_case(b"TEXT_STYLE_WITH_BOX_CHARACTERISTICS")
                        && at == 0
                    {
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
                                        self.file
                                            .bytes(part.name())
                                            .eq_ignore_ascii_case(name.as_bytes())
                                    })
                                }
                                _ => None,
                            };
                            if let Some((index, (name, value))) = known {
                                *present.get_mut(index)? = true;
                                body.extend_from_slice(format!("{name}({value})").as_bytes());
                            } else {
                                write_value(self.file, item, &mut body);
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
                        write_value(self.file, value, &mut body);
                    }
                }
                body.push(b')');
            }
        }
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
        self.instance(body)
    }

    pub(super) fn text_shape(&mut self, record: usize, args: &Arguments) -> Option<()> {
        let (styles, item) = self.occurrence(record)?;
        let was = self.reference(item)?;
        let literal = self.text_literal(args)?;
        let color = args.values.get(1)?.parse::<i64>().ok()?;
        self.text_assignments(styles, |this, old| this.text_style(args, color, Some(old)))?;
        self.args.insert(item, format!("#{literal}"));
        self.loose.push(was);
        let id = self.file.records().get(record)?.id()?;
        self.layer(id, args.values.first()?.parse().ok()?)?;
        Some(())
    }

    pub(super) fn text_color(&mut self, styles: Slot, code: i64) -> Option<()> {
        let colour = self.color(code)?;
        self.text_assignments(styles, |this, old| {
            let style_slot = this.slot(old, "TEXT_STYLE", 1)?;
            let appearance = this.reference(style_slot)?;
            let color_slot = this.slot(appearance, "TEXT_STYLE_FOR_DEFINED_FONT", 0)?;
            let appearance = this.clone_with(color_slot, format!("#{colour}"))?;
            this.clone_with(style_slot, format!("#{appearance}"))
        })
    }

    fn text_assignments(
        &mut self,
        styles: Slot,
        mut change: impl FnMut(&mut Self, u64) -> Option<u64>,
    ) -> Option<()> {
        let mut list = self.list(styles)?;
        let mut changed = false;
        for assignment in &mut list {
            let slot = self.slot(*assignment, "PRESENTATION_STYLE_ASSIGNMENT", 0)?;
            let mut items = self.list(slot)?;
            let mut touched = false;
            for style in &mut items {
                if self.slot(*style, "TEXT_STYLE", 0).is_some() {
                    if changed {
                        return None;
                    }
                    *style = change(self, *style)?;
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
                let new = self.instance(format!("PRESENTATION_STYLE_ASSIGNMENT(({values}))"))?;
                self.loose.push(*assignment);
                *assignment = new;
            }
        }
        if !changed {
            return None;
        }
        self.lists.insert(styles, list);
        Some(())
    }

    fn clone_with(&mut self, slot: Slot, value: String) -> Option<u64> {
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
        let body = rendered
            .get(start..rendered.len().checked_sub(1)?)?
            .to_vec();
        self.instance(body)
    }
}
