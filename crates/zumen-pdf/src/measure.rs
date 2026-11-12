//! VP/Measure の原値と、紙面上の変位にだけ作用する距離校正。
//!
//! PDF4QT 0cfea2f280c40d5f5e0d065496704654468390b5 の MIT 実装
//! `pdfmeasure.cpp` の X/Y/CYX と単位の扱いを比較先にする。R は表示文字列のまま持ち、
//! O は絶対位置の復元に使わない。紙の要素・実寸空間・ビューは作り替えない。
//! 既知のキーと任意項目は Arlington 603c174b024d1d2bb15a33686af83800d5c41269 の
//! MeasureRL/NumberFormat に照らす。CYX は任意で、PDF4QT と同じく Y があれば既定 1 とする。

use std::collections::BTreeMap;

use hayro_syntax::object::{Array, Dict, MaybeRef, Object, ObjectIdentifier};
use hayro_syntax::page::Page;
use hayro_syntax::reader::{Readable, Reader, ReaderContext, ReaderExt};
use kurbo::{Point, Rect};
use zumen_base::{Diagnostic, Kind, Ref, Stop, codes};
use zumen_drawing::{Calibration, Draft, Field, FieldValue, PaperPoint, Position, SheetId, Unit};
use zumen_sdk::Cx;

use crate::inflate;
use crate::page::Geometry;
use crate::strings;
use crate::walk::Reads;

/// 新しい入口で使う原本と、guard が求めた確保の上限。
pub(crate) struct Context<'a, 'x> {
    pub page: &'a Page<'x>,
    pub sheet: SheetId,
    pub geometry: Geometry,
    pub reads: Reads<'x>,
    pub metadata: inflate::Source<'x>,
    pub numbers: crate::measure_numbers::Source<'x>,
    pub objects: &'a BTreeMap<(u32, i32), Ref>,
    pub source: &'a Ref,
}

const FIELD_BYTES: u64 = 512;
// prefix・ID の有限の文字列、要素ごとの Vec の伸長、呼出側への下書きの移譲も含める。
const ENTRY_BYTES: u64 = 1024;

/// 値の形を確かめるだけでは、文字列を展開しない。
struct Raw<'a>(&'a [u8]);

impl<'a> Readable<'a> for Raw<'a> {
    fn read(reader: &mut Reader<'a>, _: &ReaderContext<'a>) -> Option<Self> {
        reader.skip::<Object<'_>>(false).map(Self)
    }
}

#[derive(Clone, Copy)]
enum Expected {
    Name,
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Dict,
    Formats,
}

struct Value<'a> {
    present: bool,
    object: Option<Object<'a>>,
    raw: Option<&'a [u8]>,
    reference: Option<ObjectIdentifier>,
    number_raw: Option<&'a [u8]>,
    number: Option<Number>,
}

#[derive(Clone, Copy)]
struct Number {
    real: f64,
    integer: Option<i64>,
}

#[derive(Default)]
struct Format {
    unit: Option<String>,
    factor: Option<f64>,
    supported: bool,
}

struct Parser<'a, 'x, 'c, 'd> {
    input: Context<'a, 'x>,
    cx: &'c mut Cx<'d>,
}

/// VP の元の順序を保ち、Measure を持つ項目ごとに原値の拡張を作る。
pub(crate) fn read(input: Context<'_, '_>, cx: &mut Cx<'_>) -> Result<Vec<Draft>, Stop> {
    let result = Parser { input, cx }.read()?;
    cx.check(Kind::Work, 0)?;
    Ok(result)
}

impl<'a, 'x> Parser<'a, 'x, '_, '_> {
    fn read(&mut self) -> Result<Vec<Draft>, Stop> {
        let mut drafts = Vec::new();
        let page = self.input.page.raw();
        if !page.contains_key(b"VP") {
            return Ok(drafts);
        }
        // 計測の有無が分からない値を、Measure と断定して保存しない。ここでは型付きで読むだけにする。
        self.input.metadata.reserve_tables(4, self.cx)?;
        self.cx.check(
            Kind::Work,
            self.input.reads.for_appearances().value_work(page, b"VP"),
        )?;
        let Some(viewports) = page.get::<Array<'_>>(b"VP") else {
            return Ok(drafts);
        };
        self.cx
            .check(Kind::Work, (viewports.data().len() as u64).div_ceil(4))?;
        let mut values = viewports.iter::<Dict<'_>>();
        let mut raw = Reader::new(viewports.data());
        let mut ordinal = 0_u64;
        loop {
            raw.skip_white_spaces_and_comments();
            if raw.at_end() {
                break;
            }
            self.cx.check(Kind::Work, 1)?;
            let Some(token) = raw.read_with_context::<MaybeRef<Raw<'_>>>(&ReaderContext::dummy())
            else {
                self.invalid("vp", "unreadable", self.input.source.clone());
                break;
            };
            self.input.metadata.reserve_tables(4, self.cx)?;
            if let MaybeRef::Ref(id) = token {
                self.cx.check(
                    Kind::Work,
                    self.input.reads.for_appearances().read(id.into()).0,
                )?;
            }
            let Some(viewport) = values.next() else {
                break;
            };
            if !viewport.contains_key(b"Measure") {
                ordinal = ordinal.saturating_add(1);
                continue;
            }
            self.cx.enter()?;
            let result = self.viewport(&viewport, ordinal, &mut drafts);
            self.cx.leave();
            result?;
            ordinal = ordinal.saturating_add(1);
        }
        Ok(drafts)
    }

    fn viewport(
        &mut self,
        viewport: &Dict<'x>,
        ordinal: u64,
        drafts: &mut Vec<Draft>,
    ) -> Result<(), Stop> {
        let mut fields = Vec::new();
        self.push(
            &mut fields,
            "sheet",
            FieldValue::Integer(i64::from(self.input.sheet.0)),
        )?;
        self.push(
            &mut fields,
            "viewport/ordinal",
            FieldValue::Integer(i64::try_from(ordinal).unwrap_or(i64::MAX)),
        )?;
        self.bytes(&mut fields, "viewport/raw", viewport.data())?;
        let vp_source = self.source(viewport.obj_id());
        let viewport_kind = self.entry(
            viewport,
            b"Type",
            "viewport/type_name",
            Expected::Name,
            &mut fields,
        )?;
        let viewport_supported = !viewport_kind.present || named(&viewport_kind, b"Viewport");
        if matches!(viewport_kind.object, Some(Object::Name(_))) && !viewport_supported {
            self.invalid("viewport/type_name", "name", vp_source.clone());
        }
        let name = self.entry(
            viewport,
            b"Name",
            "viewport/name",
            Expected::String,
            &mut fields,
        )?;
        let name = self.text_value(&name)?;
        let bbox = self.entry(
            viewport,
            b"BBox",
            "viewport/bbox",
            Expected::Array,
            &mut fields,
        )?;
        if !bbox.present {
            self.invalid("viewport/bbox", "missing", vp_source.clone());
        }
        let bbox = self.numbers(bbox, "viewport/bbox", &mut fields)?;
        if bbox.as_ref().is_some_and(|bbox| bbox.len() != 4) {
            self.invalid("viewport/bbox", "count", vp_source.clone());
        }
        let measure = self.entry(viewport, b"Measure", "measure", Expected::Dict, &mut fields)?;
        let source = match &measure.object {
            Some(Object::Dict(dict)) => dict
                .obj_id()
                .map_or_else(|| vp_source.clone(), |id| self.source(Some(id))),
            _ => vp_source,
        };
        let id = format!("sheet/{}/vp/{ordinal}", self.input.sheet.0);
        let mut calibration = None;
        if let Some(Object::Dict(measure)) = measure.object {
            self.prepare_dict(&measure)?;
            let kind = self.entry(
                &measure,
                b"Type",
                "measure/type_name",
                Expected::Name,
                &mut fields,
            )?;
            let subtype = self.entry(
                &measure,
                b"Subtype",
                "measure/subtype",
                Expected::Name,
                &mut fields,
            )?;
            let rl = (!kind.present || named(&kind, b"Measure"))
                && (!subtype.present || named(&subtype, b"RL"));
            self.entry(&measure, b"R", "r", Expected::String, &mut fields)?;
            if rl && !measure.contains_key(b"X") {
                self.invalid("x", "missing", source.clone());
            }
            let x = self.formats(&measure, b"X", "x", &mut fields)?;
            let y_present = measure.contains_key(b"Y");
            let y = self.formats(&measure, b"Y", "y", &mut fields)?;
            for (key, prefix) in [
                (b"D".as_slice(), "d"),
                (b"A", "a"),
                (b"T", "t"),
                (b"S", "s"),
            ] {
                self.formats(&measure, key, prefix, &mut fields)?;
            }
            let origin = self.entry(&measure, b"O", "o", Expected::Array, &mut fields)?;
            self.numbers(origin, "o", &mut fields)?;
            let cyx = self.entry(&measure, b"CYX", "cyx", Expected::Number, &mut fields)?;
            if y_present && number(&cyx).is_some_and(|value| value == 0.0) {
                self.invalid("cyx", "invalid-number", source.clone());
            }
            if rl && viewport_supported {
                calibration = self.calibration(
                    &id,
                    name,
                    bbox.as_deref(),
                    x,
                    y_present,
                    y,
                    &cyx,
                    source.clone(),
                );
            }
        }
        self.draft(drafts, id, fields, source.clone())?;
        if let Some(calibration) = calibration {
            self.cx.check(Kind::Work, 1)?;
            self.cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
            drafts.push(calibration.into_draft(self.cx)?);
            self.cx
                .report(Diagnostic::new(codes::PDF_MEASURE_DISTANCE_EQUIVALENT).at(source));
        } else {
            self.cx
                .report(Diagnostic::new(codes::PDF_MEASURE_UNSUPPORTED).at(source));
        }
        Ok(())
    }

    fn formats(
        &mut self,
        dict: &Dict<'x>,
        key: &[u8],
        prefix: &str,
        fields: &mut Vec<Field>,
    ) -> Result<Format, Stop> {
        let value = self.entry(dict, key, prefix, Expected::Formats, fields)?;
        match value.object {
            Some(Object::Array(array)) => {
                self.prepare_array(&array)?;
                let mut values = array.raw_iter();
                let mut raw = Reader::new(array.data());
                let mut count = 0_u64;
                let mut first = Format::default();
                let mut readable = true;
                loop {
                    raw.skip_white_spaces_and_comments();
                    if raw.at_end() {
                        break;
                    }
                    self.cx.check(Kind::Work, 1)?;
                    let Some(token) =
                        raw.read_with_context::<MaybeRef<Raw<'_>>>(&ReaderContext::dummy())
                    else {
                        self.invalid(prefix, "unreadable", self.input.source.clone());
                        break;
                    };
                    let value = self.array_value(token, &mut values, &mut readable)?;
                    let item = format!("{prefix}/{count}");
                    self.record(&item, &value, fields)?;
                    let format = if let Some(Object::Dict(dict)) = value.object {
                        self.cx.enter()?;
                        let result = self.number_format(&dict, &item, fields);
                        self.cx.leave();
                        result?
                    } else {
                        self.invalid(&item, "type", self.input.source.clone());
                        Format::default()
                    };
                    if count == 0 {
                        first = format;
                    }
                    count = count.saturating_add(1);
                }
                self.push(
                    fields,
                    &format!("{prefix}/count"),
                    FieldValue::Integer(i64::try_from(count).unwrap_or(i64::MAX)),
                )?;
                Ok(first)
            }
            Some(Object::Dict(dict)) => {
                // 比較先は単独辞書も読むが、配列だったとみなして距離校正には採用しない。
                self.push(fields, &format!("{prefix}/count"), FieldValue::Integer(1))?;
                self.cx.enter()?;
                let result = self.number_format(&dict, &format!("{prefix}/0"), fields);
                self.cx.leave();
                let mut result = result?;
                result.supported = false;
                Ok(result)
            }
            _ => Ok(Format::default()),
        }
    }

    fn number_format(
        &mut self,
        dict: &Dict<'x>,
        prefix: &str,
        fields: &mut Vec<Field>,
    ) -> Result<Format, Stop> {
        self.prepare_dict(dict)?;
        let mut result = Format {
            supported: true,
            ..Format::default()
        };
        for (key, suffix, expected) in [
            (b"Type".as_slice(), "type_name", Expected::Name),
            (b"U", "u", Expected::String),
            (b"C", "c", Expected::Number),
            (b"F", "f", Expected::Name),
            (b"D", "d", Expected::Integer),
            (b"FD", "fd", Expected::Boolean),
            (b"RT", "rt", Expected::String),
            (b"RD", "rd", Expected::String),
            (b"PS", "ps", Expected::String),
            (b"SS", "ss", Expected::String),
            (b"O", "o", Expected::Name),
        ] {
            self.cx.check(Kind::Work, 1)?;
            self.cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
            let value = self.entry(dict, key, &format!("{prefix}/{suffix}"), expected, fields)?;
            match key {
                b"Type" if value.present && !named(&value, b"NumberFormat") => {
                    result.supported = false
                }
                b"U" => {
                    if !value.present {
                        self.invalid(
                            &format!("{prefix}/u"),
                            "missing",
                            self.source(dict.obj_id()),
                        );
                    }
                    result.unit = self.text_value(&value)?;
                }
                b"C" => {
                    if !value.present {
                        self.invalid(
                            &format!("{prefix}/c"),
                            "missing",
                            self.source(dict.obj_id()),
                        );
                    }
                    result.factor = number(&value);
                    if result.factor.is_some_and(|value| value == 0.0) {
                        self.invalid(
                            &format!("{prefix}/c"),
                            "invalid-number",
                            self.source(dict.obj_id()),
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(result)
    }

    fn numbers(
        &mut self,
        value: Value<'x>,
        prefix: &str,
        fields: &mut Vec<Field>,
    ) -> Result<Option<Vec<f64>>, Stop> {
        let Some(Object::Array(array)) = value.object else {
            return Ok(None);
        };
        self.prepare_array(&array)?;
        let mut raw = Reader::new(array.data());
        let mut values = array.raw_iter();
        let mut numbers = Vec::new();
        let mut count = 0_u64;
        let mut valid = true;
        let mut readable = true;
        loop {
            raw.skip_white_spaces_and_comments();
            if raw.at_end() {
                break;
            }
            self.cx.check(Kind::Work, 1)?;
            let Some(token) = raw.read_with_context::<MaybeRef<Raw<'_>>>(&ReaderContext::dummy())
            else {
                valid = false;
                break;
            };
            let value = self.array_value(token, &mut values, &mut readable)?;
            let item = format!("{prefix}/{count}");
            self.record(&item, &value, fields)?;
            if let Some(number) = number(&value).filter(|number| number.is_finite()) {
                self.cx.check(Kind::DecodedBytes, 32)?;
                numbers.push(number);
            } else {
                valid = false;
                if !matches!(value.object, Some(Object::Number(_))) {
                    self.invalid(&item, "type", self.input.source.clone());
                }
            }
            count = count.saturating_add(1);
        }
        self.push(
            fields,
            &format!("{prefix}/count"),
            FieldValue::Integer(i64::try_from(count).unwrap_or(i64::MAX)),
        )?;
        Ok(valid.then_some(numbers))
    }

    fn entry(
        &mut self,
        dict: &Dict<'x>,
        key: &[u8],
        prefix: &str,
        expected: Expected,
        fields: &mut Vec<Field>,
    ) -> Result<Value<'x>, Stop> {
        self.cx.check(Kind::Work, 1)?;
        self.cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
        let present = dict.contains_key(key);
        let raw = dict.get_raw::<Raw<'_>>(key);
        let (object, raw, reference) = match raw {
            Some(MaybeRef::Ref(reference)) => {
                self.input.metadata.reserve_tables(4, self.cx)?;
                self.input
                    .metadata
                    .reserve_scalar(self.input.metadata.metadata_bound(), self.cx)?;
                (
                    self.input.reads.xref.get::<Object<'x>>(reference.into()),
                    None,
                    Some(reference.into()),
                )
            }
            Some(MaybeRef::NotRef(raw)) => {
                self.input.metadata.reserve_tables(4, self.cx)?;
                self.input
                    .metadata
                    .reserve_scalar(raw.0.len() as u64, self.cx)?;
                (dict.get::<Object<'x>>(key), Some(raw.0), None)
            }
            None => (None, None, None),
        };
        let mut value = Value {
            present,
            object,
            raw,
            reference,
            number_raw: None,
            number: None,
        };
        self.number_value(&mut value)?;
        self.record(prefix, &value, fields)?;
        if present && !matches_expected(&value, expected) {
            self.invalid(
                prefix,
                if value.object.is_none() && value.reference.is_some() {
                    "unresolved"
                } else if value.object.is_none() {
                    "unreadable"
                } else {
                    "type"
                },
                self.source(dict.obj_id()),
            );
        }
        Ok(value)
    }

    fn array_value(
        &mut self,
        token: MaybeRef<Raw<'x>>,
        values: &mut hayro_syntax::object::array::ArrayIter<'x>,
        readable: &mut bool,
    ) -> Result<Value<'x>, Stop> {
        self.cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
        let (raw, reference) = match token {
            MaybeRef::NotRef(raw) => (Some(raw.0), None),
            MaybeRef::Ref(reference) => (None, Some(reference.into())),
        };
        if reference.is_some() {
            self.input.metadata.reserve_tables(4, self.cx)?;
            self.input
                .metadata
                .reserve_scalar(self.input.metadata.metadata_bound(), self.cx)?;
        }
        let parsed = if *readable { values.next() } else { None };
        if parsed.is_none() {
            *readable = false;
        }
        let object = match parsed {
            Some(MaybeRef::NotRef(object)) => Some(object),
            Some(MaybeRef::Ref(reference)) => {
                self.input.reads.xref.get::<Object<'x>>(reference.into())
            }
            None => None,
        };
        let mut value = Value {
            present: true,
            object,
            raw,
            reference,
            number_raw: None,
            number: None,
        };
        self.number_value(&mut value)?;
        Ok(value)
    }

    fn number_value(&mut self, value: &mut Value<'x>) -> Result<(), Stop> {
        if !matches!(value.object, Some(Object::Number(_))) {
            return Ok(());
        }
        value.number_raw = if let Some(raw) = value.raw {
            Some(raw)
        } else if let Some(reference) = value.reference {
            self.input.numbers.raw(reference, self.cx)?
        } else {
            None
        };
        if let Some(raw) = value.number_raw {
            self.cx.check(Kind::Work, (raw.len() as u64).div_ceil(16))?;
            value.number = parse_number(raw);
        }
        Ok(())
    }

    fn record(
        &mut self,
        prefix: &str,
        value: &Value<'_>,
        fields: &mut Vec<Field>,
    ) -> Result<(), Stop> {
        self.push(
            fields,
            &format!("{prefix}/present"),
            FieldValue::Integer(i64::from(value.present)),
        )?;
        if let Some(raw) = value.raw {
            self.bytes(fields, &format!("{prefix}/raw"), raw)?;
        }
        if let Some(reference) = value.reference {
            self.reference(prefix, reference, fields)?;
        }
        let kind = match &value.object {
            None if value.present && value.reference.is_some() => "unresolved",
            None if value.present => "unreadable",
            None => "missing",
            Some(Object::Null(_)) => "null",
            Some(Object::Boolean(_)) => "boolean",
            Some(Object::Number(_)) => "number",
            Some(Object::String(_)) => "string",
            Some(Object::Name(_)) => "name",
            Some(Object::Array(_)) => "array",
            Some(Object::Dict(_)) => "dictionary",
            Some(Object::Stream(_)) => "stream",
        };
        self.push(
            fields,
            &format!("{prefix}/type"),
            FieldValue::Text(kind.to_owned()),
        )?;
        match &value.object {
            Some(Object::Number(_)) => {
                if value.reference.is_some()
                    && let Some(raw) = value.number_raw
                {
                    self.bytes(fields, &format!("{prefix}/resolved_raw"), raw)?;
                }
                if let Some(number) = value.number {
                    let field = number
                        .integer
                        .map_or(FieldValue::Real(number.real), FieldValue::Integer);
                    self.push(fields, prefix, field)?;
                } else {
                    let reason = if value.number_raw.is_some() {
                        "invalid-number"
                    } else {
                        "raw-unavailable"
                    };
                    self.invalid(prefix, reason, self.input.source.clone());
                }
            }
            Some(Object::Boolean(value)) => {
                self.push(fields, prefix, FieldValue::Integer(i64::from(*value)))?
            }
            Some(Object::String(value)) => {
                let bytes = value.as_bytes();
                self.bytes(fields, &format!("{prefix}/bytes"), bytes)?;
                self.cx
                    .check(Kind::DecodedBytes, (bytes.len() as u64).saturating_mul(4))?;
                self.push(fields, prefix, FieldValue::Text(strings::text(bytes)))?;
            }
            Some(Object::Name(value)) => {
                self.bytes(fields, &format!("{prefix}/bytes"), value.as_ref())?;
                self.cx
                    .check(Kind::DecodedBytes, (value.len() as u64).saturating_mul(3))?;
                self.push(
                    fields,
                    prefix,
                    FieldValue::Text(String::from_utf8_lossy(value.as_ref()).into_owned()),
                )?;
            }
            Some(Object::Dict(dict)) => {
                self.bytes(fields, &format!("{prefix}/resolved_raw"), dict.data())?
            }
            Some(Object::Array(array)) => {
                self.bytes(fields, &format!("{prefix}/resolved_raw"), array.data())?
            }
            Some(Object::Stream(stream)) => self.bytes(
                fields,
                &format!("{prefix}/resolved_raw"),
                stream.dict().data(),
            )?,
            _ => {}
        }
        Ok(())
    }

    fn reference(
        &mut self,
        prefix: &str,
        id: ObjectIdentifier,
        fields: &mut Vec<Field>,
    ) -> Result<(), Stop> {
        self.push(
            fields,
            &format!("{prefix}/ref/object"),
            FieldValue::Integer(i64::from(id.obj_number)),
        )?;
        self.push(
            fields,
            &format!("{prefix}/ref/generation"),
            FieldValue::Integer(i64::from(id.gen_number)),
        )
    }

    fn text_value(&mut self, value: &Value<'_>) -> Result<Option<String>, Stop> {
        let Some(Object::String(value)) = &value.object else {
            return Ok(None);
        };
        self.cx
            .check(Kind::Work, (value.as_bytes().len() as u64).div_ceil(16))?;
        self.cx.check(
            Kind::DecodedBytes,
            (value.as_bytes().len() as u64).saturating_mul(4),
        )?;
        Ok(Some(strings::text(value.as_bytes())))
    }

    fn prepare_dict(&mut self, dict: &Dict<'_>) -> Result<(), Stop> {
        self.input.metadata.reserve_tables(4, self.cx)?;
        self.cx
            .check(Kind::Work, (dict.data().len() as u64).div_ceil(4))
    }

    fn prepare_array(&mut self, array: &Array<'_>) -> Result<(), Stop> {
        self.input.metadata.reserve_tables(4, self.cx)?;
        self.input
            .metadata
            .reserve_scalar(array.data().len() as u64, self.cx)
    }

    fn bytes(&mut self, fields: &mut Vec<Field>, key: &str, bytes: &[u8]) -> Result<(), Stop> {
        self.cx
            .check(Kind::Work, (bytes.len() as u64).div_ceil(16))?;
        self.cx
            .check(Kind::DecodedBytes, (bytes.len() as u64).saturating_mul(2))?;
        self.push(fields, key, FieldValue::Bytes(bytes.to_vec()))
    }

    fn push(&mut self, fields: &mut Vec<Field>, key: &str, value: FieldValue) -> Result<(), Stop> {
        self.cx.check(Kind::Work, 1)?;
        self.cx.check(
            Kind::DecodedBytes,
            FIELD_BYTES.saturating_add((key.len() as u64).saturating_mul(2)),
        )?;
        fields.push(Field {
            key: key.to_owned().into(),
            code: None,
            value,
        });
        Ok(())
    }

    fn draft(
        &mut self,
        drafts: &mut Vec<Draft>,
        name: String,
        fields: Vec<Field>,
        source: Ref,
    ) -> Result<(), Stop> {
        self.cx.check(Kind::Work, 1)?;
        self.cx.check(Kind::DecodedBytes, ENTRY_BYTES)?;
        drafts.push(Draft {
            namespace: "pdf.measure".into(),
            name,
            version: None,
            schema: 1,
            at: Position::Document,
            fields,
            source,
        });
        Ok(())
    }

    fn source(&self, id: Option<ObjectIdentifier>) -> Ref {
        id.and_then(|id| self.input.objects.get(&(id.obj_number, id.gen_number)))
            .cloned()
            .unwrap_or_else(|| self.input.source.clone())
    }

    fn invalid(&mut self, field: &str, reason: &str, source: Ref) {
        self.cx.report(
            Diagnostic::new(codes::PDF_MEASURE_INVALID)
                .with("field", field)
                .with("reason", reason)
                .at(source),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn calibration(
        &self,
        id: &str,
        name: Option<String>,
        bbox: Option<&[f64]>,
        x: Format,
        y_present: bool,
        y: Format,
        cyx: &Value<'_>,
        source: Ref,
    ) -> Option<Calibration> {
        let bbox: [f64; 4] = bbox?.try_into().ok()?;
        if !x.supported || (y_present && !y.supported) {
            return None;
        }
        let (x_label, cx) = (x.unit?, x.factor?);
        let unit = unit(&x_label);
        let (cy, factor) = if y_present {
            let (_y_label, cy) = (y.unit?, y.factor?);
            let factor = number(cyx).or_else(|| (!cyx.present).then_some(1.0))?;
            (cy, factor)
        } else {
            (cx, 1.0)
        };
        if ![cx, cy]
            .iter()
            .all(|value| value.is_finite() && *value != 0.0)
            || !factor.is_finite()
            || factor == 0.0
        {
            return None;
        }
        let [a, b, c, d, _, _] = self.input.geometry.user_space.as_coeffs();
        let scale = a.abs().max(b.abs()).max(c.abs()).max(d.abs());
        if !scale.is_finite() || scale == 0.0 {
            return None;
        }
        let [a, b, c, d] = [a / scale, b / scale, c / scale, d / scale];
        let determinant = a * d - b * c;
        let y = cy * factor;
        let matrix = [
            (cx / scale) * (d / determinant),
            (cx / scale) * (-c / determinant),
            (y / scale) * (-b / determinant),
            (y / scale) * (a / determinant),
        ];
        if !determinant.is_finite() || determinant == 0.0 || !matrix.iter().all(|v| v.is_finite()) {
            return None;
        }
        if !bbox.iter().all(|v| v.is_finite()) {
            return None;
        }
        let [x0, y0, x1, y1] = bbox;
        if x0 == x1 || y0 == y1 {
            return None;
        }
        let rect = self
            .input
            .geometry
            .user_space
            .transform_rect_bbox(Rect::from_points(Point::new(x0, y0), Point::new(x1, y1)));
        let bounds = [rect.x0, rect.y0, rect.x1, rect.y1];
        if !bounds.iter().all(|v| v.is_finite()) || rect.width() <= 0.0 || rect.height() <= 0.0 {
            return None;
        }
        let calibration = Calibration {
            id: id.to_owned(),
            name,
            sheet: self.input.sheet,
            bounds,
            matrix,
            unit,
            unit_label: x_label,
            source,
        };
        // core と同じ正確な非特異判定を使い、行列式の二乗の overflow/underflow で拒まない。
        let at = PaperPoint {
            sheet: self.input.sheet,
            x: rect.x0,
            y: rect.y0,
        };
        calibration.measure(at, at).ok()?;
        Some(calibration)
    }
}

fn number(value: &Value<'_>) -> Option<f64> {
    value.number.map(|number| number.real)
}

/// hayro の Number は長い小数で std::powi を通る。原字句から、JSON と同じ変換で保存値を作る。
fn parse_number(raw: &[u8]) -> Option<Number> {
    let digits = match raw.first() {
        Some(b'+' | b'-') => raw.get(1..)?,
        _ => raw,
    };
    let mut decimal = false;
    let mut has_digit = false;
    for &byte in digits {
        match byte {
            b'0'..=b'9' => has_digit = true,
            b'.' if !decimal => decimal = true,
            _ => return None,
        }
    }
    if !has_digit {
        return None;
    }
    let spelling = std::str::from_utf8(raw).ok()?;
    let real = spelling
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())?;
    Some(Number {
        real,
        integer: spelling.parse::<i64>().ok(),
    })
}

fn named(value: &Value<'_>, expected: &[u8]) -> bool {
    matches!(&value.object, Some(Object::Name(value)) if value.as_ref() == expected)
}

fn matches_expected(value: &Value<'_>, expected: Expected) -> bool {
    if let (Some(Object::Number(_)), Expected::Integer) = (&value.object, expected) {
        // 字句が読めないときの診断は record が返す。型の診断で重複させない。
        return value.number.is_none_or(|number| number.integer.is_some());
    }
    matches!(
        (&value.object, expected),
        (Some(Object::Name(_)), Expected::Name)
            | (Some(Object::String(_)), Expected::String)
            | (Some(Object::Number(_)), Expected::Number)
            | (Some(Object::Boolean(_)), Expected::Boolean)
            | (Some(Object::Array(_)), Expected::Array | Expected::Formats)
            | (Some(Object::Dict(_)), Expected::Dict)
    )
}

/// 単位の原文は別に保持する。PDF4QT の既知の英語表記だけを、共通モデルの既知単位に対応させる。
fn unit(label: &str) -> Unit {
    match label.trim().to_ascii_lowercase().as_str() {
        "mm" | "millimeter" | "millimeters" | "millimetre" | "millimetres" => Unit::Millimeter,
        "cm" | "centimeter" | "centimeters" | "centimetre" | "centimetres" => Unit::Centimeter,
        "m" | "meter" | "meters" | "metre" | "metres" => Unit::Meter,
        "in" | "ins" | "inch" | "inches" | "\"" => Unit::Inch,
        "ft" | "foot" | "feet" | "'" => Unit::Foot,
        _ => Unit::Unknown,
    }
}
