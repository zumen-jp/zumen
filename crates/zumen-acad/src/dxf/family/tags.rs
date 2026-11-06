//! R2004 の ASCII DXF のレコード。構造・サブクラスのマーカーは DXF Reference の
//! 「Common Group Codes for Entities」「Symbol Tables」と各図形・オブジェクトの節に従う。

use zumen_base::{Cx, Kind};
use zumen_sdk::Error;

use super::{AcadDb, Data, Object, Plan, Value, entity_class, handle, owner, string, text};
use crate::dxf::{has_newline, value_bytes};

pub(super) struct Writer<'p, 'a, 'c, 'b> {
    plan: &'p Plan<'a>,
    cx: &'c mut Cx<'b>,
    bytes: Vec<u8>,
    omitted: Vec<(&'a Object, i16)>,
}

impl<'p, 'a, 'c, 'b> Writer<'p, 'a, 'c, 'b> {
    pub(super) fn new(plan: &'p Plan<'a>, cx: &'c mut Cx<'b>) -> Self {
        Self {
            plan,
            cx,
            bytes: Vec::new(),
            omitted: Vec::new(),
        }
    }

    pub(super) fn finish(self) -> (Vec<u8>, Vec<(&'a Object, i16)>) {
        (self.bytes, self.omitted)
    }

    pub(super) fn section(&mut self, name: &[u8]) -> Result<(), Error> {
        self.tag(0, b"SECTION")?;
        self.tag(2, name)
    }

    pub(super) fn end(&mut self) -> Result<(), Error> {
        self.tag(0, b"ENDSEC")
    }

    pub(super) fn tag(&mut self, code: i16, value: &[u8]) -> Result<(), Error> {
        let group = code.to_string();
        let size = (value.len() as u64).saturating_add(group.len() as u64 + 2);
        self.cx.check(Kind::Work, size)?;
        if has_newline(value) || value.contains(&0) {
            return Err(Error::Unsupported);
        }
        self.cx.check(Kind::DecodedBytes, size)?;
        self.bytes.extend_from_slice(group.as_bytes());
        self.bytes.push(b'\n');
        self.bytes.extend_from_slice(value);
        self.bytes.push(b'\n');
        Ok(())
    }

    pub(super) fn hex(&mut self, code: i16, handle: u64) -> Result<(), Error> {
        self.tag(code, format!("{handle:X}").as_bytes())
    }
    pub(super) fn integer(&mut self, code: i16, n: u64) -> Result<(), Error> {
        self.tag(code, n.to_string().as_bytes())
    }

    pub(super) fn value(&mut self, value: &Value) -> Result<(), Error> {
        if value.code <= 0 || value.code == 100 {
            return Err(Error::Unsupported);
        }
        match &value.data {
            Data::Text(name) if value.code == 1001 => {
                self.plan.named(b"APPID", name, self.cx)?;
            }
            Data::Handle(h) if *h != 0 && !self.plan.kept.contains(h) => {
                return Err(Error::Unsupported);
            }
            Data::Binary(bytes) => {
                let size = (bytes.len() as u64).saturating_mul(2);
                self.cx.check(Kind::Work, size)?;
                self.cx.check(Kind::DecodedBytes, size)?;
            }
            _ => {}
        }
        let bytes = value_bytes(&value.data).ok_or(Error::Unsupported)?;
        self.tag(value.code, &bytes)
    }

    // 共通のハンドルと所有者を先に書く。所有者と、102 の組にあるリアクターの 330 は区別する。
    fn header(
        &mut self,
        object: &'a Object,
        table_name: Option<&[u8]>,
    ) -> Result<Vec<&'a Value>, Error> {
        self.cx.check(Kind::Work, object.values.len() as u64)?;
        self.tag(0, &object.kind)?;
        if let Some(name) = table_name {
            self.tag(2, name)?;
        }
        let id_code = if object.kind == b"DIMSTYLE" { 105 } else { 5 };
        self.hex(id_code, handle(object)?)?;
        self.hex(330, owner(object)?)?;
        let mut body = Vec::new();
        let mut groups = 0usize;
        let mut owner_written = false;
        let mut id_written = false;
        for v in &object.values {
            if v.code == 102 {
                match text(v) {
                    Some(t) if t.starts_with(b"{") => {
                        if groups == 0 {
                            self.omitted.push((object, 102));
                        }
                        groups = groups.checked_add(1).ok_or(Error::Unsupported)?;
                    }
                    Some(b"}") if groups > 0 => groups -= 1,
                    _ => return Err(Error::Unsupported),
                }
            } else if groups == 0 {
                if v.code == id_code && !id_written {
                    id_written = true;
                } else if v.code == 330 && !owner_written {
                    owner_written = true;
                } else {
                    body.push(v);
                }
            }
        }
        if groups != 0 || !id_written || !owner_written {
            return Err(Error::Unsupported);
        }
        Ok(body)
    }

    fn values(&mut self, values: impl IntoIterator<Item = &'a Value>) -> Result<(), Error> {
        for v in values {
            self.value(v)?;
        }
        Ok(())
    }

    pub(super) fn table(
        &mut self,
        control: &'a Object,
        name: &[u8],
        count: usize,
    ) -> Result<(), Error> {
        let count = i16::try_from(count).map_err(|_| Error::Unsupported)?;
        if !self.header(control, Some(name))?.is_empty() {
            return Err(Error::Unsupported);
        }
        self.tag(100, b"AcDbSymbolTable")?;
        self.integer(70, count as u64)?;
        if name == b"DIMSTYLE" {
            self.tag(100, b"AcDbDimStyleTable")?;
            self.tag(71, b"0")?;
        }
        Ok(())
    }

    pub(super) fn table_record(
        &mut self,
        object: &'a Object,
        class: &[u8],
        db: &AcadDb,
    ) -> Result<(), Error> {
        let body = self.header(object, None)?;
        self.tag(100, b"AcDbSymbolTableRecord")?;
        self.tag(100, class)?;
        if object.kind == b"LAYER" {
            self.plan.named(b"LTYPE", string(object, 6)?, self.cx)?;
        }
        let (main, xdata) = xdata(&body);
        let layout = self.plan.layouts.get(&handle(object)?);
        for v in main.iter().copied() {
            if let Data::Handle(h) = v.data {
                let target: &[u8] = match (object.kind.as_slice(), v.code) {
                    (b"BLOCK_RECORD", 340) if Some(&h) == layout || layout.is_none() && h == 0 => {
                        continue;
                    }
                    (b"BLOCK_RECORD", 340) => return Err(Error::Unsupported),
                    (b"DIMSTYLE" | b"LTYPE", 340) => b"STYLE",
                    (b"DIMSTYLE", 341..=344) => b"BLOCK_RECORD",
                    (b"DIMSTYLE", 345..=347) => b"LTYPE",
                    (b"LAYER", 390) => b"ACDBPLACEHOLDER",
                    _ => {
                        self.value(v)?;
                        continue;
                    }
                };
                if h != 0 && !db.get(h).is_some_and(|r| r.kind == target) {
                    return Err(Error::Unsupported);
                }
            }
            self.value(v)?;
        }
        if let Some(layout) = layout {
            self.hex(340, *layout)?;
        }
        self.values(xdata.iter().copied())
    }

    pub(super) fn entity(
        &mut self,
        object: &'a Object,
        block_name: Option<&[u8]>,
    ) -> Result<(), Error> {
        let body = self.header(object, None)?;
        let (main, xdata) = xdata(&body);
        self.plan.named(b"LAYER", string(object, 8)?, self.cx)?;
        if let Some(Data::Text(ltype)) = object.first(6) {
            self.plan.named(b"LTYPE", ltype, self.cx)?;
        }
        if object.kind == b"MTEXT" {
            let style = object
                .first(7)
                .and_then(|v| {
                    if let Data::Text(name) = v {
                        Some(name.as_slice())
                    } else {
                        None
                    }
                })
                .unwrap_or(b"Standard");
            self.plan.named(b"STYLE", style, self.cx)?;
        }
        // DWG のリーダーは壊れた図形について、読めた所までの値を返すことがある。
        // 不完全な定義点を DXF の既定値で埋めない。
        let coordinates: &[i16] = match object.kind.as_slice() {
            b"ARC_DIMENSION" => &[10, 20, 11, 21, 13, 23, 14, 24, 15, 25, 40, 41],
            b"ARC" => &[10, 20, 40, 50, 51],
            b"CIRCLE" | b"MTEXT" => &[10, 20, 40],
            b"LINE" => &[10, 20, 11, 21],
            b"TRACE" | b"SOLID" => &[10, 20, 11, 21, 12, 22, 13, 23],
            b"POINT" | b"BLOCK" => &[10, 20],
            _ => &[],
        };
        if coordinates.iter().any(|&code| object.finite(code).is_err())
            || matches!(object.kind.as_slice(), b"ARC" | b"CIRCLE" | b"MTEXT")
                && object.finite(40).is_ok_and(|n| n <= 0.0)
        {
            return Err(Error::Unsupported);
        }
        self.tag(100, b"AcDbEntity")?;
        self.values(main.iter().copied().filter(|v| common_entity(v.code)))?;
        let own: Vec<_> = main
            .iter()
            .copied()
            .filter(|v| !common_entity(v.code))
            .collect();
        self.tag(100, entity_class(&object.kind).ok_or(Error::Unsupported)?)?;
        match object.kind.as_slice() {
            b"BLOCK" => {
                let name = block_name.ok_or(Error::Unsupported)?;
                if !matches!(object.first(70),Some(Data::Int(flags)) if flags & (4|8|16|32) ==0) {
                    return Err(Error::Unsupported);
                }
                for v in &own {
                    if matches!(v.code, 2 | 3) && text(v) != Some(name)
                        || v.code == 1 && text(v) != Some(b"")
                    {
                        return Err(Error::Unsupported);
                    }
                }
                self.tag(2, name)?;
                self.values(own.iter().copied().filter(|v| !matches!(v.code, 1..=3)))?;
                self.tag(3, name)?;
                self.tag(1, b"")?;
            }
            b"ARC" => {
                self.values(own.iter().copied().filter(|v| !matches!(v.code, 50 | 51)))?;
                self.tag(100, b"AcDbArc")?;
                self.values(own.iter().copied().filter(|v| matches!(v.code, 50 | 51)))?;
            }
            b"ARC_DIMENSION" => self.dimension(object, &own)?,
            _ => self.values(own)?,
        }
        self.values(xdata.iter().copied())
    }

    // 決定 0228。DWG の共通部と弧長寸法の固有部に 70・71 が一つずつある。
    // 13 を境に分け、寸法スタイルと描画ブロックの名前は AcDbDimension の中に置く。
    fn dimension(&mut self, object: &Object, values: &[&'a Value]) -> Result<(), Error> {
        let depiction = self
            .plan
            .named(b"BLOCK_RECORD", string(object, 2)?, self.cx)?;
        let block = self
            .plan
            .blocks
            .get(&handle(depiction)?)
            .ok_or(Error::Unsupported)?;
        if block.entities.is_empty()
            || !matches!(block.begin.and_then(|r| r.first(70)), Some(Data::Int(flags)) if flags & 1 != 0)
        {
            return Err(Error::Unsupported);
        }
        self.plan.named(b"DIMSTYLE", string(object, 3)?, self.cx)?;
        let split = values
            .iter()
            .position(|v| v.code == 13)
            .ok_or(Error::Unsupported)?;
        let (common, arc) = values.split_at(split);
        // AcDbDimension の 70 は寸法の種類を含む。AcDbArcDimension の 70・71 は真偽の値。
        if !matches!(object.first(70), Some(Data::Int(n)) if n & 15 == 5)
            || ![70, 71].into_iter().all(|code| {
                arc.iter()
                    .any(|v| v.code == code && matches!(v.data, Data::Int(0 | 1)))
            })
        {
            return Err(Error::Unsupported);
        }
        self.values(values.iter().copied().filter(|v| matches!(v.code, 2 | 3)))?;
        self.values(common.iter().copied().filter(|v| !matches!(v.code, 2 | 3)))?;
        self.tag(100, b"AcDbArcDimension")?;
        self.values(arc.iter().copied().filter(|v| !matches!(v.code, 2 | 3)))
    }

    pub(super) fn metadata(&mut self, object: &'a Object) -> Result<(), Error> {
        let body = self.header(object, None)?;
        let (main, xdata) = xdata(&body);
        match object.kind.as_slice() {
            b"LAYOUT" => {
                let split = main
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.code == 1)
                    .nth(1)
                    .ok_or(Error::Unsupported)?
                    .0;
                let (plot, layout) = main.split_at(split);
                self.tag(100, b"AcDbPlotSettings")?;
                self.values(plot.iter().copied())?;
                self.tag(100, b"AcDbLayout")?;
                self.values(layout.iter().copied())?;
            }
            b"DICTIONARY" | b"ACDBDICTIONARYWDFLT" => {
                self.tag(100, b"AcDbDictionary")?;
                let mut iter = main.iter().copied();
                while let Some(v) = iter.next() {
                    match v.code {
                        3 => {
                            let target = iter
                                .next()
                                .filter(|v| matches!(v.code, 350 | 360))
                                .ok_or(Error::Unsupported)?;
                            let Data::Handle(h) = target.data else {
                                return Err(Error::Unsupported);
                            };
                            if self.plan.objects.contains(&h) {
                                self.value(v)?;
                                self.value(target)?;
                            } else {
                                self.omitted.push((object, target.code));
                            }
                        }
                        340 if object.kind == b"ACDBDICTIONARYWDFLT" => {
                            self.tag(100, b"AcDbDictionaryWithDefault")?;
                            self.value(v)?;
                        }
                        280 | 281 => self.value(v)?,
                        _ => return Err(Error::Unsupported),
                    }
                }
            }
            b"ACDBPLACEHOLDER" if main.is_empty() => (),
            _ => return Err(Error::Unsupported),
        }
        self.values(xdata.iter().copied())
    }
}

fn xdata<'v, 'a>(values: &'v [&'a Value]) -> (&'v [&'a Value], &'v [&'a Value]) {
    values.split_at(
        values
            .iter()
            .position(|v| v.code >= 1000)
            .unwrap_or(values.len()),
    )
}

fn common_entity(code: i16) -> bool {
    matches!(code, 6 | 8 | 48 | 60 | 62 | 67 | 370 | 420 | 430 | 440)
}
