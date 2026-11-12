//! 図面モデルから新しい DXF を作るときの画層の拡張データと表の件数（決定 0157・0206）。

use std::collections::{BTreeMap, BTreeSet};

use super::add::{Target, int, next_free_handle, text};
use super::layer::{is_layer, name_of, record, table_end};
use crate::ConversionFidelity;
use crate::conversion_report::Report;
use crate::db::{AcadDb, Data, Section, Value};
use crate::drawing::style::{LAYER_DESCRIPTION_APP, LAYER_TRANSPARENCY_APP};
use crate::drawing::{Charset, charset_of};
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_drawing::Layer;
use zumen_sdk::Error;

impl AcadDb {
    /// 新しい R2004 DXF の表を仕上げる。`layers` は、ひな形または変換で実際に作れた画層のうち、
    /// 説明・透過性を持つ元のレイヤ。元の文書の出所で欠落を診断する（決定 0206）。
    pub fn finish_new_dxf_tables(self, layers: &[&Layer], cx: &mut Cx<'_>) -> Result<Self, Error> {
        self.finish_new_dxf_tables_with_report(layers, &mut Report::new(0), cx)
    }

    pub(crate) fn finish_new_dxf_tables_with_report(
        mut self,
        layers: &[&Layer],
        report: &mut Report,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Error> {
        if !layers.is_empty() {
            self.write_new_layer_fields(layers, report, cx)?;
        }
        // DXF Reference「Symbol Table Group Codes」。TABLE の 70 には実際に書く項目数を置く。
        let mut current = None;
        let mut count = 0_i64;
        for index in 0..self.objects().len() {
            cx.check(Kind::Work, 1)?;
            let object = self.objects().get(index).ok_or(Error::Invalid)?;
            if object.section != Section::Tables {
                continue;
            }
            match object.kind.as_slice() {
                b"TABLE" => {
                    current = Some(index);
                    count = 0;
                }
                b"ENDTAB" => {
                    if let Some(table) = current.take() {
                        let values = &mut self.object_mut(table).ok_or(Error::Invalid)?.values;
                        values
                            .iter_mut()
                            .find(|v| v.code == 70)
                            .ok_or(Error::Invalid)?
                            .data = Data::Int(count);
                    }
                }
                _ => count += 1,
            }
        }
        Ok(self)
    }

    fn write_new_layer_fields(
        &mut self,
        layers: &[&Layer],
        report: &mut Report,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        let charset = charset_of(self);
        let mut sources = BTreeMap::new();
        for &layer in layers {
            cx.check(Kind::Work, 1 + layer.name.len() as u64)?;
            let name = charset.encode(&layer.name).map_err(|_| Error::Invalid)?;
            if sources.insert(name, layer).is_some() {
                return Err(Error::Invalid);
            }
        }
        let mut apps = BTreeSet::new();
        for index in 0..self.objects().len() {
            cx.check(Kind::Work, 1)?;
            let row = self.object_mut(index).ok_or(Error::Invalid)?;
            if !is_layer(row) {
                continue;
            }
            let name = name_of(row).ok_or(Error::Invalid)?;
            cx.check(Kind::Work, name.len() as u64)?;
            let Some(layer) = sources.remove(name) else {
                continue;
            };
            // 新規作成用。元ファイルの XDATA を編集する経路と混ぜず、同じ 1001 の組を二重に書かない。
            cx.check(Kind::Work, row.values.len() as u64)?;
            if row.values.iter().any(|v| v.code == 1001 && matches!(&v.data,Data::Text(n) if n.eq_ignore_ascii_case(LAYER_DESCRIPTION_APP.as_bytes()) || n.eq_ignore_ascii_case(LAYER_TRANSPARENCY_APP.as_bytes()))) {
                return Err(Error::Unsupported);
            }
            if let Some(description) = &layer.description {
                cx.check(Kind::Work, 1 + description.len() as u64)?;
                if let Some(bytes) = description_bytes(charset, description) {
                    report.record(
                        "layer_description",
                        ConversionFidelity::Equivalent,
                        &layer.source,
                        0,
                        cx,
                    )?;
                    apps.insert(LAYER_DESCRIPTION_APP);
                    row.values.extend([
                        text(1001, LAYER_DESCRIPTION_APP),
                        text(1000, ""),
                        Value {
                            code: 1000,
                            data: Data::Text(bytes),
                        },
                    ]);
                } else {
                    report.record(
                        "layer_description",
                        ConversionFidelity::Dropped,
                        &layer.source,
                        0,
                        cx,
                    )?;
                    cx.report(
                        Diagnostic::new(codes::CONVERT_LAYER_FIELD_DROPPED)
                            .with("field", zumen_base::Value::Key("description"))
                            .with("format", "DXF")
                            .at(layer.source.clone()),
                    );
                }
            }
            if let Some(t) = layer.style.and_then(|style| style.transparency) {
                report.record(
                    "layer_transparency",
                    ConversionFidelity::Equivalent,
                    &layer.source,
                    0,
                    cx,
                )?;
                apps.insert(LAYER_TRANSPARENCY_APP);
                row.values.extend([
                    text(1001, LAYER_TRANSPARENCY_APP),
                    int(1071, 0x0200_0000 | i64::from(t.alpha)),
                ]);
            }
        }
        if !sources.is_empty() {
            return Err(Error::Invalid);
        }
        if !apps.is_empty() {
            self.register_layer_apps(apps, cx)?;
        }
        Ok(())
    }

    // DXF Reference「APPID」「About Extended Data」。1001 に使う名前を 1 回ずつ登録する。
    fn register_layer_apps(
        &mut self,
        mut apps: BTreeSet<&str>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        let mut table = None;
        let mut last = None;
        for (index, object) in self.objects().iter().enumerate() {
            cx.check(Kind::Work, 1)?;
            if object.section != Section::Tables {
                continue;
            }
            last = Some(index);
            match object.kind.as_slice() {
                b"TABLE" if name_of(object) == Some(b"APPID") => {
                    table = Some(object.handle().ok_or(Error::Invalid)?)
                }
                b"APPID" => {
                    apps.retain(|name| {
                        !name_of(object).is_some_and(|n| n.eq_ignore_ascii_case(name.as_bytes()))
                    });
                }
                _ => {}
            }
        }
        if apps.is_empty() {
            return Ok(());
        }
        cx.check(Kind::Work, self.objects().len() as u64)?;
        let mut id = next_free_handle(self, Target::Dxf).ok_or(Error::Invalid)?;
        let mut next = || -> Result<u64, Error> {
            let result = id;
            id = id.checked_add(1).ok_or(Error::Invalid)?;
            Ok(result)
        };
        let mut records = Vec::new();
        let (at, owner) = if let Some(owner) = table {
            (table_end(self, b"APPID").ok_or(Error::Invalid)?, owner)
        } else {
            let owner = next()?;
            records.push(record(
                b"TABLE",
                vec![
                    text(2, "APPID"),
                    handle(5, owner),
                    handle(330, 0),
                    text(100, "AcDbSymbolTable"),
                    int(70, 0),
                ],
                None,
            ));
            apps.insert("ACAD");
            (last.ok_or(Error::Invalid)? + 1, owner)
        };
        for name in apps {
            records.push(record(
                b"APPID",
                vec![
                    handle(330, owner),
                    text(100, "AcDbSymbolTableRecord"),
                    text(100, "AcDbRegAppTableRecord"),
                    text(2, name),
                    int(70, 0),
                ],
                Some(next()?),
            ));
        }
        if table.is_none() {
            records.push(record(b"ENDTAB", Vec::new(), None));
        }
        cx.check(Kind::Work, self.objects().len() as u64)?;
        self.insert_many(at, records);
        Ok(())
    }
}

// DXF Reference「About Extended Data」。1000 は符号化後 255 バイトまで。空の説明は読み戻すと値を持たない。
fn description_bytes(charset: Charset, description: &str) -> Option<Vec<u8>> {
    if description.is_empty()
        || description.len() > 4 * 255
        || description.contains(['\0', '\r', '\n'])
    {
        return None;
    }
    charset
        .encode(description)
        .ok()
        .filter(|bytes| bytes.len() <= 255)
}

fn handle(code: i16, value: u64) -> Value {
    Value {
        code,
        data: Data::Handle(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits, Ref};
    use zumen_drawing::{LayerId, LayerStyle, Transparency};

    #[test]
    fn existing_appid_is_reused_and_table_counts_include_only_new_registrations() {
        let mut db = AcadDb::default();
        for object in [
            record(b"TABLE", vec![text(2, "LAYER"), int(70, 1)], Some(1)),
            record(b"LAYER", vec![handle(330, 1), text(2, "0")], Some(2)),
            record(b"ENDTAB", vec![], None),
            record(b"TABLE", vec![text(2, "APPID"), int(70, 1)], Some(3)),
            record(
                b"APPID",
                vec![handle(330, 3), text(2, "accmtransparency")],
                Some(4),
            ),
            record(b"ENDTAB", vec![], None),
        ] {
            db.push(object);
        }
        db.set_variable(b"$ACADVER", vec![text(1, "AC1018")]);
        db.set_variable(b"$DWGCODEPAGE", vec![text(3, "ANSI_932")]);
        let layer = Layer {
            id: LayerId(0),
            name: "0".into(),
            follows_placement: true,
            state: None,
            style: Some(LayerStyle {
                color: None,
                line_type: None,
                line_weight: None,
                transparency: Some(Transparency { alpha: 128 }),
            }),
            description: Some("壁".into()),
            source: Ref::default(),
        };
        let convert = |db: AcadDb, limits| {
            let mut budget = Budget::new(limits);
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let result = db.finish_new_dxf_tables(
                &[&layer],
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            );
            (result, budget.used(Kind::Work))
        };
        let (result, work) = convert(db.clone(), Limits::default());
        let out = result.unwrap();
        assert_eq!(out.get(3).unwrap().first(70), Some(&Data::Int(2)));
        assert_eq!(out.get(4), db.get(4));
        let new = out.get(5).unwrap();
        assert_eq!(name_of(new), Some(LAYER_DESCRIPTION_APP.as_bytes()));
        assert_eq!(new.owner(), Some(3));
        assert!(matches!(
            convert(
                db,
                Limits::default().with(Kind::Work, Limit::Finite(work - 1))
            )
            .0,
            Err(Error::Stopped(_))
        ));
    }

    #[test]
    fn description_uses_the_encoded_byte_length_and_must_round_trip() {
        let encode = |s: &str| description_bytes(Charset::ShiftJis, s);
        assert_eq!(
            encode(&format!("{}A", "図".repeat(127))).unwrap().len(),
            255
        );
        assert!(encode(&"図".repeat(128)).is_none());
        assert_eq!(encode(&"a".repeat(255)).unwrap().len(), 255);
        for text in ["", "a\0b", "a\rb", "a\nb", "¥", "🚧"] {
            assert!(encode(text).is_none(), "{text:?}");
        }
    }
}
