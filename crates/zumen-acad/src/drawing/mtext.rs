//! MTEXT の内容を、書式の区間に分ける（AutoCAD のヘルプが示す MTEXT の書式の符号）。
//!
//! 解釈する符号は、`{}` の組、`\P`（段落）・`\N`（段）・`\~`（改行しない空白）・`\\`・`\{`・`\}`、下線・上線・
//! 取り消し線（`\L`・`\O`・`\K` と小文字）、`\U+XXXX`、値を `;` で閉じる `\H`・`\W`・`\T`・`\Q`・`\A`・`\C`・
//! `\f`・`\F`・`\S`。ほかの符号と、値を読めない符号は、元の符号のまま `Piece::Unparsed` に置き、黙って
//! 除かない（決定 0109）。真の色（`\c`）は値のバイト順を確かめていないので、段落の書式（`\p`）は区間の
//! 書式でないので、解釈しない（推定。docs/formats/dxf.md）

use zumen_drawing::{
    Color, Font, FontKind, Format, Inherit, Length, Piece, Scaled, Span, StackKind, Vertical,
};

use super::aci;

/// 内容を区間に分ける。`length` は図面の単位の値を長さにする
pub(super) fn spans(content: &str, length: &impl Fn(f64) -> Length) -> Vec<Span> {
    let mut parser = Parser {
        spans: Vec::new(),
        state: State::default(),
        length,
    };
    let mut groups = Vec::new();
    let mut rest = content;
    while let Some(c) = rest.chars().next() {
        let after = rest.get(c.len_utf8()..).unwrap_or_default();
        match c {
            '{' => {
                groups.push(parser.state.clone());
                rest = after;
            }
            '}' => {
                match groups.pop() {
                    Some(state) => parser.state = state,
                    None => parser.push(Piece::Unparsed("}".to_owned())),
                }
                rest = after;
            }
            '\\' => rest = parser.escape(after),
            _ => {
                let end = rest.find(['{', '}', '\\']).unwrap_or(rest.len());
                let (text, tail) = rest.split_at_checked(end).unwrap_or((rest, ""));
                parser.text(text);
                rest = tail;
            }
        }
    }
    parser.spans
}

/// 書式の状態。高さは図面の単位の値か倍率で持ち、区間を足すときに長さにする
#[derive(Clone, Debug, Default, PartialEq)]
struct State {
    height: Option<Scaled<f64>>,
    width_factor: Option<Scaled<f64>>,
    tracking: Option<Scaled<f64>>,
    oblique: Option<f64>,
    font: Option<Font>,
    color: Option<Inherit<Color>>,
    align: Option<Vertical>,
    underline: bool,
    overline: bool,
    strike: bool,
}

struct Parser<'l, L> {
    spans: Vec<Span>,
    state: State,
    length: &'l L,
}

impl<L: Fn(f64) -> Length> Parser<'_, L> {
    fn format(&self) -> Format {
        let state = &self.state;
        Format {
            height: state.height.map(|height| match height {
                Scaled::Value(value) => Scaled::Value((self.length)(value)),
                Scaled::Factor(factor) => Scaled::Factor(factor),
            }),
            width_factor: state.width_factor,
            tracking: state.tracking,
            oblique: state.oblique,
            font: state.font.clone(),
            color: state.color,
            align: state.align,
            underline: state.underline,
            overline: state.overline,
            strike: state.strike,
        }
    }

    fn push(&mut self, piece: Piece) {
        let format = self.format();
        self.spans.push(Span { piece, format });
    }

    /// 文字列を足す。前の区間が同じ書式の文字列なら、つなげる
    fn text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let format = self.format();
        if let Some(Span {
            piece: Piece::Text(last),
            format: before,
        }) = self.spans.last_mut()
            && *before == format
        {
            last.push_str(text);
            return;
        }
        self.spans.push(Span {
            piece: Piece::Text(text.to_owned()),
            format,
        });
    }

    /// `\` の後を読み、残りを返す
    fn escape<'a>(&mut self, rest: &'a str) -> &'a str {
        let Some(code) = rest.chars().next() else {
            self.push(Piece::Unparsed("\\".to_owned()));
            return rest;
        };
        let after = rest.get(code.len_utf8()..).unwrap_or_default();
        match code {
            '\\' | '{' | '}' => self.text(code.encode_utf8(&mut [0; 4])),
            'P' => self.push(Piece::Paragraph),
            'N' => self.push(Piece::Column),
            '~' => self.text("\u{a0}"),
            'L' | 'l' => self.state.underline = code == 'L',
            'O' | 'o' => self.state.overline = code == 'O',
            'K' | 'k' => self.state.strike = code == 'K',
            'U' => return self.unicode(after),
            // 2007 より前の版の、コード・ページの文字（\M+ と 5 文字）
            'M' => {
                let (taken, tail) = match after.split_at_checked(6) {
                    Some((taken, tail)) if taken.starts_with('+') => (taken, tail),
                    _ => ("", after),
                };
                self.push(Piece::Unparsed(format!("\\M{taken}")));
                return tail;
            }
            'H' | 'W' | 'T' | 'Q' | 'A' | 'C' | 'c' | 'f' | 'F' | 'S' | 'p' => {
                let Some((value, tail)) = terminator(after)
                    .and_then(|end| Some((after.get(..end)?, after.get(end + 1..)?)))
                else {
                    self.push(Piece::Unparsed(format!("\\{rest}")));
                    return "";
                };
                if !self.parameter(code, value) {
                    self.push(Piece::Unparsed(format!("\\{code}{value};")));
                }
                return tail;
            }
            _ => self.push(Piece::Unparsed(format!("\\{code}"))),
        }
        after
    }

    /// `\U+XXXX`。16 進の 4 桁でなければ、`\U` を解釈しない
    fn unicode<'a>(&mut self, after: &'a str) -> &'a str {
        let decoded = after
            .strip_prefix('+')
            .and_then(|hex| hex.get(..4))
            .filter(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .and_then(char::from_u32);
        match decoded {
            Some(c) => {
                self.text(c.encode_utf8(&mut [0; 4]));
                after.get(5..).unwrap_or_default()
            }
            None => {
                self.push(Piece::Unparsed("\\U".to_owned()));
                after
            }
        }
    }

    /// 値を持つ符号。解釈できなければ偽を返す
    fn parameter(&mut self, code: char, value: &str) -> bool {
        let state = &mut self.state;
        match code {
            'H' => apply(&mut state.height, scaled(value)),
            'W' => apply(&mut state.width_factor, scaled(value)),
            'T' => apply(&mut state.tracking, scaled(value)),
            'Q' => number(value)
                .map(|angle| state.oblique = Some(angle))
                .is_some(),
            'A' => {
                state.align = Some(match value {
                    "0" => Vertical::Bottom,
                    "1" => Vertical::Middle,
                    "2" => Vertical::Top,
                    _ => return false,
                });
                true
            }
            'C' => {
                state.color = Some(match value.parse::<i64>() {
                    Ok(0) => Inherit::Block,
                    Ok(256) => Inherit::Layer,
                    Ok(index @ 1..=255) => Inherit::Own(Color {
                        code: Some(aci(index)),
                        rgb: None,
                    }),
                    _ => return false,
                });
                true
            }
            'f' => font(value).map(|font| state.font = Some(font)).is_some(),
            // 形のファイルの名前。| の続く指定は確かめていない
            'F' if !value.is_empty() && !value.contains('|') => {
                state.font = Some(Font {
                    name: value.to_owned(),
                    kind: FontKind::Shape,
                });
                true
            }
            'S' => match stack(value) {
                Some(piece) => {
                    self.push(piece);
                    true
                }
                None => false,
            },
            _ => false,
        }
    }
}

/// 値を閉じる `;` の位置。`\;` は値の一部とする
fn terminator(after: &str) -> Option<usize> {
    let mut escaped = false;
    for (at, c) in after.char_indices() {
        match c {
            ';' if !escaped => return Some(at),
            '\\' => escaped = !escaped,
            _ => escaped = false,
        }
    }
    None
}

/// 有限の数
fn number(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// 正の値か、後ろに x の付いた倍率
fn scaled(value: &str) -> Option<Scaled<f64>> {
    match value.strip_suffix('x') {
        Some(factor) => number(factor).filter(|f| *f > 0.0).map(Scaled::Factor),
        None => number(value).filter(|v| *v > 0.0).map(Scaled::Value),
    }
}

/// 倍率は、いまの値に掛ける
fn apply(slot: &mut Option<Scaled<f64>>, new: Option<Scaled<f64>>) -> bool {
    let Some(new) = new else {
        return false;
    };
    *slot = Some(match (*slot, new) {
        (_, Scaled::Value(value)) => Scaled::Value(value),
        (None, Scaled::Factor(factor)) => Scaled::Factor(factor),
        (Some(Scaled::Value(value)), Scaled::Factor(factor)) => Scaled::Value(value * factor),
        (Some(Scaled::Factor(old)), Scaled::Factor(factor)) => Scaled::Factor(old * factor),
    });
    true
}

/// `\f名前|b1|i0|c128|p34;`。項目は省ける
fn font(value: &str) -> Option<Font> {
    let mut fields = value.split('|');
    let name = fields.next().filter(|name| !name.is_empty())?;
    let (mut bold, mut italic, mut charset, mut pitch) = (false, false, None, None);
    for field in fields {
        let (key, value) = field.split_at_checked(1)?;
        match key {
            "b" | "i" => {
                let on = match value {
                    "0" => false,
                    "1" => true,
                    _ => return None,
                };
                if key == "b" {
                    bold = on;
                } else {
                    italic = on;
                }
            }
            "c" => charset = Some(value.parse().ok()?),
            "p" => pitch = Some(value.parse().ok()?),
            _ => return None,
        }
    }
    Some(Font {
        name: name.to_owned(),
        kind: FontKind::Named {
            bold,
            italic,
            charset,
            pitch,
        },
    })
}

/// `\S上^下;`。最初の `^`・`/`・`#` で分ける。`\` の後の文字は区切りとしない
fn stack(value: &str) -> Option<Piece> {
    let mut escaped = false;
    for (at, c) in value.char_indices() {
        let kind = match c {
            '\\' => {
                escaped = !escaped;
                continue;
            }
            _ if escaped => {
                escaped = false;
                continue;
            }
            '/' => StackKind::Horizontal,
            '#' => StackKind::Diagonal,
            '^' => StackKind::Tolerance,
            _ => continue,
        };
        return Some(Piece::Stack {
            upper: value.get(..at)?.to_owned(),
            lower: value.get(at + 1..)?.to_owned(),
            kind,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_drawing::{ModelLength, SpaceId};

    fn parse(content: &str) -> Vec<Span> {
        spans(content, &|value| {
            Length::Model(ModelLength {
                space: SpaceId(0),
                value,
            })
        })
    }

    fn text(text: &str, format: Format) -> Span {
        Span {
            piece: Piece::Text(text.to_owned()),
            format,
        }
    }

    fn plain(piece: Piece) -> Span {
        Span {
            piece,
            format: Format::default(),
        }
    }

    #[test]
    fn groups_restore_the_format() {
        let bold = Format {
            underline: true,
            height: Some(Scaled::Factor(2.0)),
            ..Format::default()
        };
        assert_eq!(
            parse("a{\\L\\H2x;b\\Pc}d"),
            [
                text("a", Format::default()),
                text("b", bold.clone()),
                Span {
                    piece: Piece::Paragraph,
                    format: bold.clone(),
                },
                text("c", bold),
                text("d", Format::default()),
            ]
        );
    }

    #[test]
    fn heights_and_factors_combine() {
        let height = |value| {
            Some(Scaled::Value(Length::Model(ModelLength {
                space: SpaceId(0),
                value,
            })))
        };
        let spans = parse("\\H2.5;a\\H2x;b");
        let heights: Vec<_> = spans.iter().map(|span| span.format.height).collect();
        assert_eq!(heights, [height(2.5), height(5.0)]);
    }

    #[test]
    fn escapes_and_unicode_become_text() {
        assert_eq!(
            parse("\\\\\\{x\\}\\~\\U+5186"),
            [text("\\{x}\u{a0}円", Format::default())]
        );
    }

    #[test]
    fn fonts_colors_and_stacks_are_read() {
        let spans = parse("\\fMS Gothic|b1|i0|c128|p49;\\C1;a\\S1/2;\\FTXT;b");
        assert_eq!(
            spans[0].format.font,
            Some(Font {
                name: "MS Gothic".to_owned(),
                kind: FontKind::Named {
                    bold: true,
                    italic: false,
                    charset: Some(128),
                    pitch: Some(49),
                },
            })
        );
        assert_eq!(
            spans[0].format.color,
            Some(Inherit::Own(Color {
                code: Some(aci(1)),
                rgb: None,
            }))
        );
        assert_eq!(
            spans[1].piece,
            Piece::Stack {
                upper: "1".to_owned(),
                lower: "2".to_owned(),
                kind: StackKind::Horizontal,
            }
        );
        assert_eq!(
            spans[2].format.font.as_ref().map(|f| f.kind),
            Some(FontKind::Shape)
        );
    }

    #[test]
    fn what_cannot_be_read_is_kept_as_it_was() {
        assert_eq!(
            parse("\\c255;\\pxi-3;\\Habc;\\X\\M+18140\\U+zz}\\H1"),
            [
                plain(Piece::Unparsed("\\c255;".to_owned())),
                plain(Piece::Unparsed("\\pxi-3;".to_owned())),
                plain(Piece::Unparsed("\\Habc;".to_owned())),
                plain(Piece::Unparsed("\\X".to_owned())),
                plain(Piece::Unparsed("\\M+18140".to_owned())),
                plain(Piece::Unparsed("\\U".to_owned())),
                text("+zz", Format::default()),
                plain(Piece::Unparsed("}".to_owned())),
                plain(Piece::Unparsed("\\H1".to_owned())),
            ]
        );
    }
}
