//! 図形の XDATA（決定 0115）と、拡張辞書の XRECORD（決定 0148）を、形式固有の情報にする。
//! 図面モデルの値にない図形の欄も、`dxf.entity` の拡張に控える（決定 0142）。

use super::{
    ENTITY, ENTITY_SCHEMA, Lifted, Lifter, Out, XDATA, XDATA_SCHEMA, XRECORD, XRECORD_SCHEMA,
    amount, invalid, source, text, work,
};
use crate::db::{AcadDb, Data, Object, Value};
use zumen_base::{Cx, Diagnostic, codes};
use zumen_drawing::{Basis, Extension, Field, FieldValue, Target};
use zumen_sdk::Error;

impl Lifter<'_> {
    /// 図形の XDATA（DXF Reference「Extended Data」、決定 0115）を、登録アプリケーション名（1001）ごとの
    /// 形式固有の情報にして、図形の値とともに控える。最初の 1001 より前の XDATA の値と、読めない値を持つ
    /// 名前の分は移さず、診断する。図面モデルの値にない図形の欄は、`dxf.entity` の拡張に控える（決定 0142）
    pub(super) fn extend(
        &self,
        db: &AcadDb,
        object: &Object,
        target: Target,
        basis: impl Fn() -> Basis,
        out: &mut Out,
        cx: &mut Cx,
    ) {
        match entity_fields(object) {
            Ok(fields) if !fields.is_empty() => out.extensions.push(Extension {
                namespace: ENTITY,
                name: String::new(),
                version: self.version.clone(),
                schema: ENTITY_SCHEMA,
                target,
                basis: basis(),
                fields,
                source: source(object),
            }),
            Ok(_) => {}
            Err(diagnostic) => cx.report((*diagnostic).at(source(object))),
        }
        let start = object
            .values
            .iter()
            .position(|value| value.code == 1001)
            .unwrap_or(object.values.len());
        let (head, xdata) = object.values.split_at(start);
        if let Some(stray) = head.iter().find(|value| value.code >= 1000) {
            cx.report((*invalid(&object.kind, stray.code)).at(source(object)));
        }
        for group in xdata.chunk_by(|_, next| next.code != 1001) {
            let Some((name, values)) = group.split_first() else {
                continue;
            };
            match self.xdata(&object.kind, name, values) {
                Ok((name, fields)) => out.extensions.push(Extension {
                    namespace: XDATA,
                    name,
                    version: self.version.clone(),
                    schema: XDATA_SCHEMA,
                    target,
                    basis: basis(),
                    fields,
                    source: source(object),
                }),
                Err(diagnostic) => cx.report((*diagnostic).at(source(object))),
            }
        }
        self.dictionary(db, object, target, &basis, out, cx);
    }

    /// 図形の拡張辞書（`{ACAD_XDICTIONARY` の組の 360 が指す DICTIONARY。DXF Reference「Extension
    /// Dictionary and Persistent Reactors」、決定 0148）の XRECORD の項目を、項目の名前ごとの形式固有の
    /// 情報にして、図形の値とともに控える。値は、グループ・コード（`code`）と、その型の値の組を順に並べる。
    /// XRECORD でない項目と、読めない値を持つ XRECORD は移さず、診断する
    pub(super) fn dictionary(
        &self,
        db: &AcadDb,
        object: &Object,
        target: Target,
        basis: &impl Fn() -> Basis,
        out: &mut Out,
        cx: &mut Cx,
    ) {
        let Some(dictionary) = xdictionary(object)
            .and_then(|handle| db.get(handle))
            .filter(|found| found.kind == b"DICTIONARY")
        else {
            return;
        };
        let mut values = dictionary.values.iter().peekable();
        while let Some(value) = values.next() {
            let (
                Value {
                    code: 3,
                    data: name,
                },
                Some(Value {
                    code: 350 | 360,
                    data: Data::Handle(handle),
                }),
            ) = (value, values.peek())
            else {
                continue;
            };
            values.next();
            // 記号の切り抜きは、記号が持つ（決定 0149）
            if object.kind == b"INSERT"
                && matches!(name, Data::Text(text) if text == b"ACAD_FILTER")
            {
                continue;
            }
            // 項目のオブジェクトがなければ、移すものはない
            let Some(item) = db.get(*handle) else {
                continue;
            };
            let named =
                self.decode(b"DICTIONARY", 3, name)
                    .and_then(|name| match item.kind.as_slice() {
                        b"XRECORD" => Ok((name.clone(), self.xrecord(item)?)),
                        kind => Err(Box::new(
                            Diagnostic::new(codes::DXF_EXTENSION_DICTIONARY_DROPPED)
                                .with("key", text(name.as_bytes()))
                                .with("kind", text(kind)),
                        )),
                    });
            match named {
                Ok((name, fields)) => out.extensions.push(Extension {
                    namespace: XRECORD,
                    name,
                    version: self.version.clone(),
                    schema: XRECORD_SCHEMA,
                    target,
                    basis: basis(),
                    fields,
                    source: source(object),
                }),
                Err(diagnostic) => cx.report((*diagnostic).at(source(object))),
            }
        }
    }

    /// XRECORD のデータの値。所有者などの共通の値と、複製の扱い（280）の後に並ぶ
    fn xrecord(&self, item: &Object) -> Lifted<Vec<Field>> {
        let mut depth = 0usize;
        let mut values = item.values.iter().skip_while(|value| {
            match (value.code, &value.data) {
                (102, Data::Text(text)) if text.starts_with(b"{") => depth += 1,
                (102, Data::Text(text)) if text.starts_with(b"}") => {
                    depth = depth.saturating_sub(1);
                    return true;
                }
                _ => {}
            }
            depth > 0 || matches!(value.code, 5 | 100 | 330)
        });
        let mut fields = Vec::new();
        let mut first = true;
        for Value { code, data } in values.by_ref() {
            if core::mem::take(&mut first) && *code == 280 {
                continue;
            }
            let value = match data {
                Data::Text(_) => FieldValue::Text(self.decode(&item.kind, *code, data)?),
                Data::Int(value) => FieldValue::Integer(*value),
                Data::Real(value) => FieldValue::Real(*value),
                Data::Bool(value) => FieldValue::Integer(i64::from(*value)),
                Data::Handle(handle) => FieldValue::Handle(*handle),
                Data::Binary(bytes) => FieldValue::Bytes(bytes.clone()),
            };
            let key = match value {
                FieldValue::Text(_) => "text",
                FieldValue::Integer(_) => "integer",
                FieldValue::Real(_) => "real",
                FieldValue::Handle(_) => "handle",
                _ => "binary",
            };
            fields.push(Field {
                key: "code",
                value: FieldValue::Integer(i64::from(*code)),
            });
            fields.push(Field { key, value });
        }
        Ok(fields)
    }

    /// 1 つの登録アプリケーション名の XDATA。点（1010〜1013）は、続く Y（+10）と Z（+20）と合わせる
    fn xdata(&self, kind: &[u8], name: &Value, values: &[Value]) -> Lifted<(String, Vec<Field>)> {
        let name = self.decode(kind, 1001, &name.data)?;
        let mut fields = Vec::new();
        let mut values = values.iter();
        while let Some(Value { code, data }) = values.next() {
            let code = *code;
            let key = match code {
                1000 => "string",
                1002 => "control",
                1003 => "layer",
                1004 => "binary",
                1005 => "handle",
                1010 => "point",
                1011 => "position",
                1012 => "displacement",
                1013 => "direction",
                1040 => "real",
                1041 => "distance",
                1042 => "scale",
                1070 => "integer",
                1071 => "long",
                _ => return Err(invalid(kind, code)),
            };
            let value = match (code, data) {
                (1002, data) => match self.decode(kind, code, data)?.as_str() {
                    text @ ("{" | "}") => FieldValue::Text(text.to_owned()),
                    _ => return Err(invalid(kind, code)),
                },
                (1000 | 1003, data) => FieldValue::Text(self.decode(kind, code, data)?),
                (1004, Data::Binary(bytes)) => FieldValue::Bytes(bytes.clone()),
                (1005, Data::Handle(handle)) => FieldValue::Handle(*handle),
                (1010..=1013, Data::Real(x)) => {
                    let mut axis = |offset: i16| match values.next() {
                        Some(Value {
                            code: next,
                            data: Data::Real(value),
                        }) if *next == code + offset => Ok(*value),
                        _ => Err(invalid(kind, code + offset)),
                    };
                    FieldValue::Point([*x, axis(10)?, axis(20)?])
                }
                (1040..=1042, Data::Real(value)) => FieldValue::Real(*value),
                (1070 | 1071, Data::Int(value)) => FieldValue::Integer(*value),
                _ => return Err(invalid(kind, code)),
            };
            fields.push(Field { key, value });
        }
        Ok((name, fields))
    }
}

/// 拡張辞書の項目と XRECORD の値の複製を、移す前に数える。同じ辞書を複数の図形が指しても、
/// 図形ごとに数える。形式固有の情報に控える図形の値の複製も、図形の作業量として加える。
pub(super) fn extension_work(db: &AcadDb, object: &Object, cx: &mut Cx) -> Result<(), Error> {
    let Some(dictionary) = xdictionary(object)
        .and_then(|handle| db.get(handle))
        .filter(|found| found.kind == b"DICTIONARY")
    else {
        return Ok(());
    };
    let source = source(object);
    for pair in dictionary.values.windows(2) {
        work(cx, 1, &source)?;
        if let [
            Value {
                code: 3,
                data: name,
            },
            Value {
                code: 350 | 360,
                data: Data::Handle(handle),
            },
        ] = pair
            && let Some(item) = db.get(*handle)
        {
            if item.kind == b"XRECORD" {
                work(
                    cx,
                    (item.values.len() as u64).saturating_add(amount(object)),
                    &source,
                )?;
            } else if object.kind == b"INSERT"
                && item.kind == b"DICTIONARY"
                && matches!(name, Data::Text(name) if name == b"ACAD_FILTER")
            {
                // 共有する切り抜きも、配置ごとに辞書の探索と境界の値の複製・変換を数える。
                work(cx, item.values.len() as u64, &source)?;
                if let Some(filter) = entry(db, *handle, b"SPATIAL")
                    .and_then(|handle| db.get(handle))
                    .filter(|filter| filter.kind == b"SPATIAL_FILTER")
                {
                    work(cx, filter.values.len() as u64, &source)?;
                }
            }
        }
    }
    Ok(())
}

/// 図面モデルの値にない図形の欄（決定 0142）。厚さ（39）は、平面の図形を Z の向きに押し出す長さで、上から
/// 見た形を変えないので、0 でなければ `thickness` に控える。LWPOLYLINE・POLYLINE の線種の生成（70 の 128）は、
/// 線種の模様を頂点ごとに切らずに続けることで、立っていれば `line_type_generation` を 1 とする
fn entity_fields(object: &Object) -> Lifted<Vec<Field>> {
    let mut fields = Vec::new();
    let real = |code: i16| match object.first(code) {
        Some(Data::Real(value)) if value.is_finite() => Ok(Some(*value)),
        None => Ok(None),
        Some(_) => Err(invalid(&object.kind, code)),
    };
    let thick = matches!(
        object.kind.as_slice(),
        b"LINE"
            | b"CIRCLE"
            | b"ARC"
            | b"TEXT"
            | b"LWPOLYLINE"
            | b"POLYLINE"
            | b"SOLID"
            | b"TRACE"
            | b"POINT"
            | b"SHAPE"
    );
    if thick
        && let Some(thickness) = real(39)?
        && thickness != 0.0
    {
        fields.push(Field {
            key: "thickness",
            value: FieldValue::Real(thickness),
        });
    }
    if matches!(object.kind.as_slice(), b"LWPOLYLINE" | b"POLYLINE")
        && matches!(object.first(70), Some(Data::Int(flags)) if flags & 128 != 0)
    {
        fields.push(Field {
            key: "line_type_generation",
            value: FieldValue::Integer(1),
        });
    }
    if object.kind == b"SPLINE" {
        let tolerances = [
            (42, "knot_tolerance"),
            (43, "control_tolerance"),
            (44, "fit_tolerance"),
        ];
        for (code, key) in tolerances {
            if let Some(tolerance) = real(code)? {
                fields.push(Field {
                    key,
                    value: FieldValue::Real(tolerance),
                });
            }
        }
    }
    Ok(fields)
}

/// DICTIONARY（`handle`）の、名前が `name` の項目のハンドル
pub(super) fn entry(db: &AcadDb, handle: u64, name: &[u8]) -> Option<u64> {
    let dictionary = db.get(handle).filter(|found| found.kind == b"DICTIONARY")?;
    dictionary.values.windows(2).find_map(|pair| match pair {
        [
            Value {
                code: 3,
                data: Data::Text(text),
            },
            Value {
                code: 350 | 360,
                data: Data::Handle(item),
            },
        ] if text == name => Some(*item),
        _ => None,
    })
}

/// 図形の拡張辞書のハンドル（`{ACAD_XDICTIONARY` の組の 360）
pub(super) fn xdictionary(object: &Object) -> Option<u64> {
    let mut inside = false;
    for value in &object.values {
        match (value.code, &value.data) {
            (102, Data::Text(text)) if text == b"{ACAD_XDICTIONARY" => inside = true,
            (102, Data::Text(text)) if text.starts_with(b"}") => inside = false,
            (360, Data::Handle(handle)) if inside => return Some(*handle),
            _ => {}
        }
    }
    None
}
