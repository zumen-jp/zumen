//! P21 の書き方（決定 0169）。
//!
//! 要素は、表記の実体（`…_OCCURRENCE`）である。スタイルを変えるときは、曲線の見え方（CURVE_STYLE）と見え方の割り当てを
//! 足し、表記の実体がそれを指すようにする。形を変えるときは、幾何の実体を足し、表記の実体がそれを指すようにする。
//! 見え方と幾何は、ほかの要素と共有しうるので、元の実体は書き換えない。除くときは、表記の実体と、用紙・レイヤ・
//! 非表示・複合図形の定義の並びの中の参照を除く。どこからも指されなくなった幾何と見え方の実体も除く。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::Ref;
use zumen_sdk::{Cx, Error};

use super::{Arguments, Writer, line_end, newline, patched, work};
use crate::drawing::{LINE_TYPES, WIDTHS};
use crate::p21::{COLORS, Codes};
use crate::{Form, SxfFile, Value};

/// 引数の場所。レコード・部分・引数の番号
type Slot = (usize, usize, usize);

/// 要素を並べる実体の名前と、並びの引数の番号。要素を除くときは、この並びから参照を除く
const CONTAINERS: [(&str, usize); 4] = [
    ("DRAWING_SHEET_REVISION", 1),
    ("PRESENTATION_LAYER_ASSIGNMENT", 2),
    ("INVISIBILITY", 0),
    ("DRAUGHTING_SUBFIGURE_REPRESENTATION", 1),
];

/// どこからも指されなくなったら除く実体（幾何と見え方）
const GARBAGE: [&str; 17] = [
    "CARTESIAN_POINT",
    "DIRECTION",
    "VECTOR",
    "LINE",
    "POLYLINE",
    "CIRCLE",
    "ELLIPSE",
    "BEZIER_CURVE",
    "TRIMMED_CURVE",
    "AXIS2_PLACEMENT_2D",
    "CURVE_STYLE",
    "PRESENTATION_STYLE_ASSIGNMENT",
    "TEXT_LITERAL_WITH_EXTENT",
    "PLANAR_EXTENT",
    "TEXT_STYLE_FOR_DEFINED_FONT",
    "SYMBOL_STYLE",
    "SYMBOL_COLOUR",
];

pub(super) struct P21<'f> {
    file: &'f SxfFile,
    /// インスタンスの番号から、レコードの番号。同じ番号の 2 つ目は使わない（変換と同じ）
    instances: BTreeMap<u64, usize>,
    /// インスタンスの番号から、それを指すレコード
    referrers: BTreeMap<u64, BTreeSet<usize>>,
    codes: Codes,
    /// 利用者定義線幅（mm）。コードは 11 から
    widths: Vec<Option<f64>>,
    /// 書き直す引数と、その値
    args: BTreeMap<Slot, String>,
    /// 書き直す参照の並び
    lists: BTreeMap<Slot, Vec<u64>>,
    /// 除くレコード
    removed: BTreeSet<usize>,
    /// 足す実体の番号と本体（`#n=` と `;` を除く）
    added: Vec<(u64, String)>,
    next: u64,
    /// 指す所から外した実体。最後に、どこからも指されなければ除く
    loose: Vec<u64>,
    /// 足した既定義の色と線種。実体の名前と色・線種の名前から、番号
    predefined: BTreeMap<(&'static str, &'static str), u64>,
    newline: &'static str,
}

impl<'f> P21<'f> {
    pub(super) fn new(
        file: &'f SxfFile,
        codes: Codes,
        widths: Vec<Option<f64>>,
        cx: &mut Cx,
    ) -> Result<Self, Error> {
        let mut instances = BTreeMap::new();
        let mut referrers: BTreeMap<u64, BTreeSet<usize>> = BTreeMap::new();
        for (record, entry) in file.records().iter().enumerate() {
            work(cx, 1, &Ref::default())?;
            if let Some(id) = entry.id() {
                instances.entry(id).or_insert(record);
            }
            let mut refs = Vec::new();
            for part in entry.parts() {
                work(cx, 1, &Ref::default())?;
                count_values(part.args(), cx)?;
                collect(part.args(), &mut refs);
            }
            for id in refs {
                work(cx, 1, &Ref::default())?;
                referrers.entry(id).or_default().insert(record);
            }
        }
        let next = instances
            .keys()
            .next_back()
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        work(cx, 1 + file.src.len() as u64 / 1024, &Ref::default())?;
        Ok(P21 {
            file,
            instances,
            referrers,
            codes,
            widths,
            args: BTreeMap::new(),
            lists: BTreeMap::new(),
            removed: BTreeSet::new(),
            added: Vec::new(),
            next,
            loose: Vec::new(),
            predefined: BTreeMap::new(),
            newline: newline(&file.src),
        })
    }

    fn name(&self, record: usize, part: usize) -> &'f [u8] {
        self.file
            .records()
            .get(record)
            .and_then(|entry| entry.parts().get(part))
            .map(|part| self.file.bytes(part.name()))
            .unwrap_or_default()
    }

    /// 名前の部分の番号
    fn part(&self, record: usize, name: &str) -> Option<usize> {
        let entry = self.file.records().get(record)?;
        (0..entry.parts().len()).find(|part| {
            self.name(record, *part)
                .eq_ignore_ascii_case(name.as_bytes())
        })
    }

    fn value(&self, (record, part, at): Slot) -> Option<&'f Value> {
        self.file
            .records()
            .get(record)?
            .parts()
            .get(part)?
            .args()
            .get(at)
    }

    /// 番号の実体の、名前の部分の引数の場所
    fn slot(&self, id: u64, name: &str, at: usize) -> Option<Slot> {
        let record = *self.instances.get(&id)?;
        Some((record, self.part(record, name)?, at))
    }

    /// 参照の並び。書き直していれば、書き直した並び。参照のほかの値があれば `None`
    fn list(&self, slot: Slot) -> Option<Vec<u64>> {
        if let Some(list) = self.lists.get(&slot) {
            return Some(list.clone());
        }
        match self.value(slot)? {
            Value::List(items) => items
                .iter()
                .map(|item| match item {
                    Value::Ref(id) => Some(*id),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    }

    fn reference(&self, slot: Slot) -> Option<u64> {
        match self.value(slot)? {
            Value::Ref(id) => Some(*id),
            _ => None,
        }
    }

    fn number(&self, value: &Value) -> Option<f64> {
        match value {
            Value::Number(raw) => core::str::from_utf8(self.file.bytes(raw.clone()))
                .ok()?
                .parse()
                .ok(),
            Value::Typed(part) => self.number(part.args().first()?),
            _ => None,
        }
    }

    /// 表記の実体の、見え方の並びと対象の引数の場所。曲線の表記の実体だけ
    fn occurrence(&self, record: usize) -> Option<(Slot, Slot)> {
        self.part(record, "ANNOTATION_CURVE_OCCURRENCE")?;
        let part = self.part(record, "STYLED_ITEM").or_else(|| {
            let entry = self.file.records().get(record)?;
            (entry.parts().len() == 1).then_some(0)
        })?;
        let count = self
            .file
            .records()
            .get(record)?
            .parts()
            .get(part)?
            .args()
            .len();
        let at = usize::from(count >= 3);
        Some(((record, part, at), (record, part, at + 1)))
    }

    fn instance(&mut self, body: String) -> Option<u64> {
        let id = self.next;
        self.next = self.next.checked_add(1)?;
        self.added.push((id, body));
        Some(id)
    }

    fn point(&mut self, x: f64, y: f64) -> Option<u64> {
        let body = format!("CARTESIAN_POINT(' ',({},{}))", real(x)?, real(y)?);
        self.instance(body)
    }

    /// 既定義の色か線種の実体。なければ足す
    fn predefined(&mut self, entity: &'static str, name: &'static str) -> Option<u64> {
        if let Some(id) = self.predefined.get(&(entity, name)) {
            return Some(*id);
        }
        let id = self.instance(format!("{entity}('{name}')"))?;
        self.predefined.insert((entity, name), id);
        Some(id)
    }

    /// 色のコードの実体
    fn color(&mut self, code: i64) -> Option<u64> {
        let found = self
            .codes
            .colors
            .iter()
            .find(|(_, known)| **known == code)
            .map(|(id, _)| *id);
        if found.is_some() {
            return found;
        }
        let name = COLORS.get(usize::try_from(code.checked_sub(1)?).ok()?)?;
        self.predefined("DRAUGHTING_PRE_DEFINED_COLOUR", name)
    }

    /// 線種のコードの実体
    fn line_type(&mut self, code: i64) -> Option<u64> {
        let found = self
            .codes
            .line_types
            .iter()
            .find(|(_, known)| **known == code)
            .map(|(id, _)| *id);
        if found.is_some() {
            return found;
        }
        let name = LINE_TYPES.get(usize::try_from(code.checked_sub(1)?).ok()?)?;
        self.predefined("DRAUGHTING_PRE_DEFINED_CURVE_FONT", name)
    }

    /// 線幅のコードの値。同じ太さの LENGTH_MEASURE_WITH_UNIT があればそれを、なければ型を付けた数を書く
    fn width(&self, code: i64) -> Option<String> {
        let mm = match usize::try_from(code.checked_sub(1)?).ok()? {
            at if at < WIDTHS.len() => *WIDTHS.get(at)?,
            _ => {
                (*self
                    .widths
                    .get(usize::try_from(code.checked_sub(11)?).ok()?)?)?
            }
        };
        for (id, record) in &self.instances {
            let Some(part) = self.part(*record, "LENGTH_MEASURE_WITH_UNIT") else {
                continue;
            };
            let value = self.value((*record, part, 0)).and_then(|v| self.number(v));
            if value == Some(mm) {
                return Some(format!("#{id}"));
            }
        }
        Some(format!("POSITIVE_LENGTH_MEASURE({})", real(mm)?))
    }

    /// 曲線の見え方（CURVE_STYLE）と見え方の割り当てを足し、その番号を返す。`was` は元の CURVE_STYLE
    fn curve_style(&mut self, was: Option<u64>, codes: &[(usize, i64)]) -> Option<u64> {
        let mut values = match was {
            Some(was) => {
                let record = *self.instances.get(&was)?;
                let part = self.part(record, "CURVE_STYLE")?;
                (0..4)
                    .map(|at| Some(text(self.file, self.value((record, part, at))?)))
                    .collect::<Option<Vec<String>>>()?
            }
            None => vec![
                "' '".to_owned(),
                String::new(),
                String::new(),
                String::new(),
            ],
        };
        for (at, code) in codes {
            let value = match at {
                1 => format!("#{}", self.color(*code)?),
                2 => format!("#{}", self.line_type(*code)?),
                3 => self.width(*code)?,
                _ => continue,
            };
            // CURVE_STYLE は、名前・線種・線幅・色の順
            let at = match at {
                1 => 3,
                other => *other - 1,
            };
            *values.get_mut(at)? = value;
        }
        if values.iter().any(String::is_empty) {
            return None;
        }
        let style = self.instance(format!("CURVE_STYLE({})", values.join(",")))?;
        self.instance(format!("PRESENTATION_STYLE_ASSIGNMENT((#{style}))"))
    }

    /// 形の幾何の実体を足し、その番号を返す
    fn geometry(&mut self, args: &Arguments) -> Option<u64> {
        let value = |at: usize| args.values.get(at)?.parse::<f64>().ok();
        match args.name {
            "line" => {
                let (x1, y1, x2, y2) = (value(4)?, value(5)?, value(6)?, value(7)?);
                let length = libm::hypot(x2 - x1, y2 - y1);
                if !(length > 0.0 && length.is_finite()) {
                    return None;
                }
                let start = self.point(x1, y1)?;
                let end = self.point(x2, y2)?;
                let (dx, dy) = ((x2 - x1) / length, (y2 - y1) / length);
                let direction =
                    self.instance(format!("DIRECTION(' ',({},{}))", real(dx)?, real(dy)?))?;
                let vector =
                    self.instance(format!("VECTOR(' ',#{direction},{})", real(length)?))?;
                let line = self.instance(format!("LINE(' ',#{start},#{vector})"))?;
                self.instance(format!(
                    "TRIMMED_CURVE(' ',#{line},(#{start}),(#{end}),.T.,.CARTESIAN.)"
                ))
            }
            "polyline" => {
                let coordinates = |at: usize| -> Option<Vec<f64>> {
                    let list = args.values.get(at)?;
                    let inner = list.strip_prefix('(')?.strip_suffix(')')?;
                    inner.split(',').map(|item| item.parse().ok()).collect()
                };
                let (xs, ys) = (coordinates(5)?, coordinates(6)?);
                if xs.len() != ys.len() || xs.len() < 2 {
                    return None;
                }
                let mut points = Vec::new();
                for (x, y) in xs.into_iter().zip(ys) {
                    points.push(format!("#{}", self.point(x, y)?));
                }
                self.instance(format!("POLYLINE(' ',({}))", points.join(",")))
            }
            "circle" => self.circle(value(4)?, value(5)?, value(6)?),
            "arc" => {
                let circle = self.circle(value(4)?, value(5)?, value(6)?)?;
                // 向きのフラグは 0 が反時計回り。角度は度で、媒介変数はラジアン
                let sense = if args.values.get(7)? == "0" { "T" } else { "F" };
                let start = real(value(8)?.to_radians())?;
                let end = real(value(9)?.to_radians())?;
                self.instance(format!(
                    "TRIMMED_CURVE(' ',#{circle},(PARAMETER_VALUE({start})),(PARAMETER_VALUE({end})),.{sense}.,.PARAMETER.)"
                ))
            }
            _ => None,
        }
    }

    fn circle(&mut self, x: f64, y: f64, radius: f64) -> Option<u64> {
        let center = self.point(x, y)?;
        let placement = self.instance(format!("AXIS2_PLACEMENT_2D(' ',#{center},$)"))?;
        self.instance(format!("CIRCLE(' ',#{placement},{})", real(radius)?))
    }

    /// 要素をレイヤのコードのレイヤに移す。ほかのレイヤ（仮のレイヤを除く）の並びから除く
    fn layer(&mut self, id: u64, code: i64) -> Option<()> {
        let target = *self
            .codes
            .layers
            .get(usize::try_from(code.checked_sub(1)?).ok()?)?;
        for layer in self.codes.layers.clone() {
            let slot = self.slot(layer, "PRESENTATION_LAYER_ASSIGNMENT", 2)?;
            let mut list = self.list(slot)?;
            let had = list.contains(&id);
            list.retain(|item| *item != id);
            if layer == target {
                list.push(id);
            } else if !had {
                continue;
            }
            // レイヤの要素の並びは、空にできない
            if list.is_empty() {
                return None;
            }
            self.lists.insert(slot, list);
        }
        Some(())
    }

    /// 実体を本体のテキストにする。書き直した引数と並びを使う
    fn render(&self, record: usize) -> Option<Vec<u8>> {
        let entry = self.file.records().get(record)?;
        let nl = self.newline.as_bytes();
        let mut out = format!("#{}=", entry.id()?).into_bytes();
        let complex = entry.form() == Form::Complex;
        if complex {
            out.push(b'(');
            out.extend_from_slice(nl);
        }
        for (index, part) in entry.parts().iter().enumerate() {
            out.extend_from_slice(self.file.bytes(part.name()));
            out.push(b'(');
            for (at, value) in part.args().iter().enumerate() {
                if at > 0 {
                    out.push(b',');
                }
                let slot = (record, index, at);
                if let Some(text) = self.args.get(&slot) {
                    out.extend_from_slice(text.as_bytes());
                } else if let Some(list) = self.lists.get(&slot) {
                    let items: Vec<String> = list.iter().map(|id| format!("#{id}")).collect();
                    out.extend_from_slice(format!("({})", items.join(",")).as_bytes());
                } else {
                    out.extend_from_slice(text(self.file, value).as_bytes());
                }
            }
            out.push(b')');
            if complex {
                out.extend_from_slice(nl);
            }
        }
        if complex {
            out.push(b')');
        }
        out.push(b';');
        Some(out)
    }

    /// どこからも指されなくなった幾何と見え方の実体を除く
    fn collect_garbage(&mut self, cx: &mut Cx) -> Result<(), Error> {
        let mut counts: BTreeMap<u64, usize> = BTreeMap::new();
        let mut count = |refs: Vec<u64>| {
            for id in refs {
                *counts.entry(id).or_default() += 1;
            }
        };
        for (record, entry) in self.file.records().iter().enumerate() {
            work(cx, 1, &Ref::default())?;
            if self.removed.contains(&record) {
                continue;
            }
            let mut refs = Vec::new();
            for (index, part) in entry.parts().iter().enumerate() {
                work(cx, 1, &Ref::default())?;
                for (at, value) in part.args().iter().enumerate() {
                    let slot = (record, index, at);
                    if let Some(text) = self.args.get(&slot) {
                        work(cx, 1 + text.len() as u64, &Ref::default())?;
                        refs.extend(scan(text));
                    } else if let Some(list) = self.lists.get(&slot) {
                        work(cx, 1 + list.len() as u64, &Ref::default())?;
                        refs.extend(list);
                    } else {
                        count_values(core::slice::from_ref(value), cx)?;
                        collect(core::slice::from_ref(value), &mut refs);
                    }
                }
            }
            work(cx, refs.len() as u64, &Ref::default())?;
            count(refs);
        }
        for (_, body) in &self.added {
            work(cx, 1 + body.len() as u64, &Ref::default())?;
            let refs = scan(body);
            work(cx, refs.len() as u64, &Ref::default())?;
            count(refs);
        }
        while let Some(id) = self.loose.pop() {
            work(cx, 1, &Ref::default())?;
            let Some(&record) = self.instances.get(&id) else {
                continue;
            };
            let garbage = self
                .file
                .records()
                .get(record)
                .is_some_and(|entry| entry.parts().len() == 1)
                && GARBAGE
                    .iter()
                    .any(|name| self.name(record, 0).eq_ignore_ascii_case(name.as_bytes()));
            if !garbage
                || self.removed.contains(&record)
                || counts.get(&id).copied().unwrap_or(0) > 0
            {
                continue;
            }
            self.removed.insert(record);
            let mut refs = Vec::new();
            if let Some(entry) = self.file.records().get(record) {
                for part in entry.parts() {
                    work(cx, 1, &Ref::default())?;
                    count_values(part.args(), cx)?;
                    collect(part.args(), &mut refs);
                }
            }
            for child in refs {
                work(cx, 1, &Ref::default())?;
                if let Some(count) = counts.get_mut(&child) {
                    *count = count.saturating_sub(1);
                }
                self.loose.push(child);
            }
        }
        Ok(())
    }
}

impl Writer for P21<'_> {
    fn codes(
        &mut self,
        record: usize,
        codes: &[(usize, i64)],
        cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        self.count_codes(record, codes, cx)?;
        Ok(self.codes_inner(record, codes))
    }

    fn shape(&mut self, record: usize, args: &Arguments, cx: &mut Cx) -> Result<Option<()>, Error> {
        self.count_part_search(record, cx)?;
        count_arguments(args, cx)?;
        Ok((|| {
            let (_, item) = self.occurrence(record)?;
            let was = self.reference(item)?;
            let geometry = self.geometry(args)?;
            self.args.insert(item, format!("#{geometry}"));
            self.loose.push(was);
            Some(())
        })())
    }

    fn remove(&mut self, record: usize, cx: &mut Cx) -> Result<Option<()>, Error> {
        // 参照元と並びは、実際に触るものだけを、複製や走査の前に数える。
        if let Some(entry) = self.file.records().get(record) {
            for part in entry.parts() {
                work(cx, 1, &Ref::default())?;
                count_values(part.args(), cx)?;
            }
            if let Some(referrers) = entry.id().and_then(|id| self.referrers.get(&id)) {
                work(cx, referrers.len() as u64, &Ref::default())?;
                for referrer in referrers {
                    let Some(entry) = self.file.records().get(*referrer) else {
                        continue;
                    };
                    work(cx, entry.parts().len() as u64, &Ref::default())?;
                    for (index, part) in entry.parts().iter().enumerate() {
                        count_values(part.args(), cx)?;
                        for (at, value) in part.args().iter().enumerate() {
                            let slot = (*referrer, index, at);
                            if let Some(list) = self.lists.get(&slot) {
                                work(cx, (list.len() as u64).saturating_mul(2), &Ref::default())?;
                            } else if let Value::List(items) = value {
                                work(cx, (items.len() as u64).saturating_mul(2), &Ref::default())?;
                            }
                        }
                    }
                }
            }
        }
        Ok(self.remove_inner(record))
    }

    fn add(&mut self, args: &Arguments, cx: &mut Cx) -> Result<Option<u64>, Error> {
        count_arguments(args, cx)?;
        for (id, record) in &self.instances {
            work(cx, 1, &Ref::default())?;
            self.count_part_search(*record, cx)?;
            if let Some(slot) = self.slot(*id, "DRAWING_SHEET_REVISION", 1) {
                self.count_list(slot, cx)?;
                break;
            }
        }
        let codes: Vec<_> = args
            .values
            .iter()
            .take(4)
            .enumerate()
            .filter_map(|(at, value)| value.parse().ok().map(|code| (at, code)))
            .collect();
        self.count_codes(usize::MAX, &codes, cx)?;
        Ok(self.add_inner(args))
    }

    fn bytes(mut self: Box<Self>, src: &[u8], cx: &mut Cx) -> Result<Vec<u8>, Error> {
        self.collect_garbage(cx)?;
        self.finish(src, cx)
    }
}

impl P21<'_> {
    fn count_part_search(&self, record: usize, cx: &mut Cx) -> Result<(), Error> {
        if let Some(entry) = self.file.records().get(record) {
            work(cx, entry.parts().len() as u64, &Ref::default())?;
        }
        Ok(())
    }

    fn count_list(&self, slot: Slot, cx: &mut Cx) -> Result<(), Error> {
        let len = if let Some(list) = self.lists.get(&slot) {
            list.len()
        } else if let Some(Value::List(items)) = self.value(slot) {
            items.len()
        } else {
            0
        };
        work(
            cx,
            (len as u64).saturating_mul(3).saturating_add(1),
            &Ref::default(),
        )
    }

    fn count_codes(&self, record: usize, codes: &[(usize, i64)], cx: &mut Cx) -> Result<(), Error> {
        self.count_part_search(record, cx)?;
        if codes.iter().any(|(at, _)| *at == 0) {
            work(cx, self.codes.layers.len() as u64, &Ref::default())?;
            for layer in &self.codes.layers {
                if let Some(&layer_record) = self.instances.get(layer) {
                    self.count_part_search(layer_record, cx)?;
                    if let Some(slot) = self.slot(*layer, "PRESENTATION_LAYER_ASSIGNMENT", 2) {
                        self.count_list(slot, cx)?;
                    }
                }
            }
        }
        if codes.iter().any(|(at, _)| *at > 0) {
            if let Some((styles, _)) = self.occurrence(record) {
                self.count_list(styles, cx)?;
                if let Some(assignments) = self.list(styles) {
                    for assignment in assignments {
                        if let Some(&assignment_record) = self.instances.get(&assignment) {
                            self.count_part_search(assignment_record, cx)?;
                            if let Some(slot) =
                                self.slot(assignment, "PRESENTATION_STYLE_ASSIGNMENT", 0)
                            {
                                self.count_list(slot, cx)?;
                                if let Some(styles) = self.list(slot) {
                                    for style in styles {
                                        if let Some(&style_record) = self.instances.get(&style) {
                                            self.count_part_search(style_record, cx)?;
                                            if let Some(entry) =
                                                self.file.records().get(style_record)
                                            {
                                                for part in entry.parts() {
                                                    count_values(part.args(), cx)?;
                                                }
                                                let span = entry.span();
                                                work(
                                                    cx,
                                                    span.end.saturating_sub(span.start) as u64,
                                                    &Ref::default(),
                                                )?;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some((_, code)) = codes.iter().find(|(at, _)| *at == 1) {
                for known in self.codes.colors.values() {
                    work(cx, 1, &Ref::default())?;
                    if known == code {
                        break;
                    }
                }
            }
            if let Some((_, code)) = codes.iter().find(|(at, _)| *at == 2) {
                for known in self.codes.line_types.values() {
                    work(cx, 1, &Ref::default())?;
                    if known == code {
                        break;
                    }
                }
            }
            if let Some((_, code)) = codes.iter().find(|(at, _)| *at == 3) {
                let mm = code
                    .checked_sub(1)
                    .and_then(|at| usize::try_from(at).ok())
                    .and_then(|at| {
                        if at < WIDTHS.len() {
                            WIDTHS.get(at).copied()
                        } else {
                            code.checked_sub(11)
                                .and_then(|at| usize::try_from(at).ok())
                                .and_then(|at| self.widths.get(at).copied().flatten())
                        }
                    });
                if mm.is_none() {
                    return Ok(());
                }
                for record in self.instances.values() {
                    work(cx, 1, &Ref::default())?;
                    self.count_part_search(*record, cx)?;
                    if let Some(part) = self.part(*record, "LENGTH_MEASURE_WITH_UNIT")
                        && let Some(value) = self.value((*record, part, 0))
                    {
                        count_values(core::slice::from_ref(value), cx)?;
                        count_number(value, cx)?;
                        count_number(value, cx)?;
                        if self.number(value) == mm && mm.is_some() {
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn codes_inner(&mut self, record: usize, codes: &[(usize, i64)]) -> Option<()> {
        let (styles, _) = self.occurrence(record)?;
        let id = self.file.records().get(record)?.id()?;
        for (at, code) in codes {
            if *at == 0 {
                self.layer(id, *code)?;
            }
        }
        let style: Vec<(usize, i64)> = codes.iter().copied().filter(|(at, _)| *at > 0).collect();
        if style.is_empty() {
            return Some(());
        }
        let mut list = self.list(styles)?;
        let (index, was, curve) = list.iter().enumerate().find_map(|(index, assignment)| {
            let slot = self.slot(*assignment, "PRESENTATION_STYLE_ASSIGNMENT", 0)?;
            let curve = self
                .list(slot)?
                .into_iter()
                .find(|style| self.slot(*style, "CURVE_STYLE", 0).is_some())?;
            Some((index, *assignment, curve))
        })?;
        if self
            .list(self.slot(was, "PRESENTATION_STYLE_ASSIGNMENT", 0)?)?
            .len()
            != 1
        {
            return None;
        }
        let assignment = self.curve_style(Some(curve), &style)?;
        *list.get_mut(index)? = assignment;
        self.lists.insert(styles, list);
        self.loose.push(was);
        Some(())
    }
    fn remove_inner(&mut self, record: usize) -> Option<()> {
        let id = self.file.records().get(record)?.id()?;
        if self.instances.get(&id) != Some(&record) {
            return None;
        }
        let referrers = self.referrers.get(&id).cloned().unwrap_or_default();
        let mut slots = Vec::new();
        for referrer in referrers {
            let slot = CONTAINERS.iter().find_map(|(name, at)| {
                let part = self.part(referrer, name)?;
                Some((referrer, part, *at))
            })?;
            // 並びのほかの引数から指すものは、除けない
            let mut refs = Vec::new();
            let entry = self.file.records().get(referrer)?;
            for (index, part) in entry.parts().iter().enumerate() {
                for (at, value) in part.args().iter().enumerate() {
                    if (referrer, index, at) != slot {
                        collect(core::slice::from_ref(value), &mut refs);
                    }
                }
            }
            if refs.contains(&id) {
                return None;
            }
            slots.push(slot);
        }
        for slot in slots {
            let mut list = self.list(slot)?;
            list.retain(|item| *item != id);
            if list.is_empty() {
                // 並びは空にできない。非表示の並びは、実体ごと除く
                self.part(slot.0, "INVISIBILITY")?;
                self.removed.insert(slot.0);
            }
            self.lists.insert(slot, list);
        }
        self.removed.insert(record);
        let entry = self.file.records().get(record)?;
        for part in entry.parts() {
            collect(part.args(), &mut self.loose);
        }
        Some(())
    }

    fn add_inner(&mut self, args: &Arguments) -> Option<u64> {
        let code = |at: usize| args.values.get(at)?.parse::<i64>().ok();
        let (layer, color, line_type, width) = (code(0)?, code(1)?, code(2)?, code(3)?);
        let sheet = self
            .instances
            .iter()
            .find_map(|(id, _)| self.slot(*id, "DRAWING_SHEET_REVISION", 1))?;
        let geometry = self.geometry(args)?;
        let font = self.line_type(line_type)?;
        let colour = self.color(color)?;
        let width = self.width(width)?;
        let style = self.instance(format!("CURVE_STYLE(' ',#{font},{width},#{colour})"))?;
        let assignment = self.instance(format!("PRESENTATION_STYLE_ASSIGNMENT((#{style}))"))?;
        let id = self.instance(format!(
            "ANNOTATION_CURVE_OCCURRENCE(' ',(#{assignment}),#{geometry})"
        ))?;
        let mut items = self.list(sheet)?;
        items.push(id);
        self.lists.insert(sheet, items);
        self.layer(id, layer)?;
        Some(id)
    }

    fn finish(self: Box<Self>, src: &[u8], cx: &mut Cx) -> Result<Vec<u8>, Error> {
        let mut patches = Vec::new();
        let records = self.file.records();
        for record in &self.removed {
            work(cx, 1, &Ref::default())?;
            if let Some(entry) = records.get(*record) {
                let span = entry.span();
                patches.push((span.start..line_end(src, span.end), Vec::new()));
            }
        }
        work(
            cx,
            (self.args.len() as u64).saturating_add(self.lists.len() as u64),
            &Ref::default(),
        )?;
        let rewritten: BTreeSet<usize> = self
            .args
            .keys()
            .chain(self.lists.keys())
            .map(|(record, _, _)| *record)
            .filter(|record| !self.removed.contains(record))
            .collect();
        for record in rewritten {
            let Some(entry) = records.get(record) else {
                return Err(Error::Rejected);
            };
            for (index, part) in entry.parts().iter().enumerate() {
                work(cx, 1, &Ref::default())?;
                count_values(part.args(), cx)?;
                for (at, _) in part.args().iter().enumerate() {
                    let slot = (record, index, at);
                    if let Some(list) = self.lists.get(&slot) {
                        work(cx, list.len() as u64, &Ref::default())?;
                    }
                    if let Some(text) = self.args.get(&slot) {
                        work(cx, 1 + text.len() as u64 / 1024, &Ref::default())?;
                    }
                }
            }
            let span = entry.span();
            work(
                cx,
                span.end.saturating_sub(span.start) as u64,
                &Ref::default(),
            )?;
            let Some(text) = self.render(record) else {
                return Err(Error::Rejected);
            };
            patches.push((entry.span(), text));
        }
        if !self.added.is_empty() {
            // 最後の実体の後に足す
            work(cx, records.len() as u64, &Ref::default())?;
            let Some(end) = records
                .iter()
                .filter(|entry| entry.id().is_some())
                .map(|entry| entry.span().end)
                .max()
            else {
                return Err(Error::Rejected);
            };
            let nl = self.newline;
            for (_, body) in &self.added {
                work(cx, 1 + body.len() as u64, &Ref::default())?;
            }
            let text: String = self
                .added
                .iter()
                .map(|(id, body)| format!("{nl}#{id}={body};"))
                .collect();
            patches.push((end..end, text.into_bytes()));
        }
        patched(patches, src, cx)
    }
}

/// 値の入れ子をたどる作業を、参照を集める前に数える。
fn count_values(values: &[Value], cx: &mut Cx) -> Result<(), Error> {
    for value in values {
        work(cx, 1, &Ref::default())?;
        match value {
            Value::List(items) => count_values(items, cx)?,
            Value::Typed(part) => count_values(part.args(), cx)?,
            _ => {}
        }
    }
    Ok(())
}

fn count_arguments(args: &Arguments, cx: &mut Cx) -> Result<(), Error> {
    for text in &args.values {
        work(cx, 1 + text.len() as u64, &Ref::default())?;
    }
    Ok(())
}

/// 線幅の数値は文字列から解析するので、その長さも数える。
fn count_number(mut value: &Value, cx: &mut Cx) -> Result<(), Error> {
    loop {
        work(cx, 1, &Ref::default())?;
        match value {
            Value::Number(range) => {
                work(
                    cx,
                    1 + range.end.saturating_sub(range.start) as u64 / 1024,
                    &Ref::default(),
                )?;
                return Ok(());
            }
            Value::Typed(part) => match part.args().first() {
                Some(inner) => value = inner,
                None => return Ok(()),
            },
            _ => return Ok(()),
        }
    }
}

/// 値の中の参照を足す
fn collect(values: &[Value], out: &mut Vec<u64>) {
    for value in values {
        match value {
            Value::Ref(id) => out.push(*id),
            Value::List(items) => collect(items, out),
            Value::Typed(part) => collect(part.args(), out),
            _ => {}
        }
    }
}

/// 書いたテキストの中の参照
fn scan(text: &str) -> Vec<u64> {
    text.split('#')
        .skip(1)
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect()
}

/// 値を P21 のテキストにする
fn text(file: &SxfFile, value: &Value) -> String {
    let raw = |range: &core::ops::Range<usize>| String::from_utf8_lossy(file.bytes(range.clone()));
    match value {
        Value::Null => "$".to_owned(),
        Value::Derived => "*".to_owned(),
        Value::Ref(id) => format!("#{id}"),
        Value::Enum(range) => format!(".{}.", raw(range)),
        Value::Text { raw: range, .. } => format!("'{}'", raw(range)),
        Value::Number(range) | Value::Token(range) => raw(range).into_owned(),
        Value::Binary(range) => format!("\"{}\"", raw(range)),
        Value::Typed(part) => {
            let args: Vec<String> = part.args().iter().map(|value| text(file, value)).collect();
            format!(
                "{}({})",
                String::from_utf8_lossy(file.bytes(part.name())),
                args.join(",")
            )
        }
        Value::List(items) => {
            let items: Vec<String> = items.iter().map(|value| text(file, value)).collect();
            format!("({})", items.join(","))
        }
    }
}

/// 実数。読み直すと同じ値になる最短の 10 進に、小数点を付ける
fn real(value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let mut text = format!("{value}");
    if !text.contains('.') {
        text.push('.');
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sxf;
    use zumen_base::{Budget, CancelToken, Diagnostics, Kind, Limit, Limits};
    use zumen_sdk::testkit;

    #[test]
    fn indexing_and_garbage_collection_count_work() {
        let src = b"ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1=POLYLINE(' ',(#2,#3));\n#2=CARTESIAN_POINT(' ',(0.,0.));\n#3=CARTESIAN_POINT(' ',(1.,1.));\nENDSEC;\nEND-ISO-10303-21;\n";
        let file = testkit::read::<Sxf>(src).0.unwrap();
        let codes = || Codes {
            layers: Vec::new(),
            colors: BTreeMap::new(),
            line_types: BTreeMap::new(),
        };
        let cancel = CancelToken::new();
        let mut diagnostics = Diagnostics::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        assert!(matches!(
            P21::new(
                &file,
                codes(),
                Vec::new(),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));

        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let mut writer = P21::new(
            &file,
            codes(),
            Vec::new(),
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        let record = *writer.instances.get(&2).unwrap();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        let mut diagnostics = Diagnostics::new();
        assert!(matches!(
            writer.remove(record, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err(Error::Stopped(_))
        ));
        assert!(writer.removed.is_empty());
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        let mut diagnostics = Diagnostics::new();
        assert!(matches!(
            writer.codes(
                record,
                &[(3, 1)],
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert!(writer.args.is_empty() && writer.lists.is_empty() && writer.added.is_empty());
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        let mut diagnostics = Diagnostics::new();
        let args = Arguments {
            name: "line",
            values: vec!["1".to_owned(); 8],
        };
        assert!(matches!(
            writer.add(&args, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err(Error::Stopped(_))
        ));
        assert!(writer.args.is_empty() && writer.lists.is_empty() && writer.added.is_empty());
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        let mut diagnostics = Diagnostics::new();
        assert!(matches!(
            writer.collect_garbage(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err(Error::Stopped(_))
        ));
    }

    #[test]
    fn parsing_a_long_number_counts_its_length() {
        let value = Value::Number(0..8192);
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(2)));
        let mut diagnostics = Diagnostics::new();
        assert!(matches!(
            count_number(&value, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err(Error::Stopped(_))
        ));
    }
}
