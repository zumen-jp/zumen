//! 記号の定義を足す（`Edit::AddSymbol`、#684）。定義は、ブロックのレコード（BLOCK_RECORD）と、BLOCKS の所の
//! BLOCK・中の図形・ENDBLK にする（DXF Reference「BLOCK_RECORD」「BLOCK」「ENDBLK」）。DXF は、レコードを
//! ブロックのレコードの表の終わりに、BLOCK から ENDBLK までを BLOCKS の所の終わりに置く。DWG は、足した
//! オブジェクトを表の終わりに置き、書き戻しがブロックのレコードと BLOCK・ENDBLK・図形を揃えて書く（決定 0068）。
//! ハンドルは、ほかの足す図形と同じく、変更の順に付ける。

use super::{Born, Record, Target, int, text};
use crate::db::{AcadDb, Data, Object, Section};
use crate::drawing::apply::InSpace;
use crate::drawing::apply::layer::{table_end, usable_name};
use crate::drawing::apply::replace::real;
use crate::drawing::{BLOCK, Home};
use zumen_drawing::{ModelPoint, SymbolDefinition};

/// 足せない定義の理由。診断の `field` と `value`
pub(in crate::drawing::apply) type Refusal = (String, &'static str);

/// DXF の BLOCKS の所の終わり（最後の BLOCKS のオブジェクトの次）。BLOCKS の所がなければ `None`
pub(super) fn blocks_end(db: &AcadDb) -> Option<usize> {
    db.objects()
        .iter()
        .rposition(|object| object.section == Section::Blocks)
        .map(|at| at + 1)
}

/// モデル空間のブロックのレコード
fn model_record(db: &AcadDb) -> Option<&Object> {
    db.objects().iter().find(|object| {
        object.kind == b"BLOCK_RECORD"
            && matches!(object.first(2), Some(Data::Text(name)) if name.eq_ignore_ascii_case(b"*Model_Space"))
    })
}

impl Born {
    /// 記号の定義 `symbol` を足すレコード。最初がブロックのレコード、次が BLOCK、中の図形、最後が ENDBLK。
    /// `index` は、この変更の一覧で先に足したレコードの数。足せなければ、その理由
    pub(in crate::drawing::apply) fn definition(
        &self,
        db: &AcadDb,
        symbol: &SymbolDefinition,
        index: u64,
    ) -> Result<Vec<Record>, Refusal> {
        let refuse = |field: &str, value| Err((field.to_owned(), value));
        if symbol.key.namespace != BLOCK {
            return refuse("namespace", "unsupported");
        }
        let name = symbol.key.name.as_str();
        if !usable_name(name) || name.starts_with('*') {
            return refuse("name", "invalid");
        }
        let lower = name.to_lowercase();
        if self.blocks.contains_key(&lower)
            || self
                .names
                .get(b"BLOCK_RECORD".as_slice())
                .is_some_and(|names| names.contains_key(&lower))
        {
            return refuse("name", "exists");
        }
        if symbol.anonymous {
            return refuse("anonymous", "unsupported");
        }
        if symbol.external.is_some() {
            return refuse("external", "unsupported");
        }
        if !symbol.attributes.is_empty() {
            return refuse("attributes", "unsupported");
        }
        let space = symbol.space.id;
        let base = match symbol.base {
            None => [0.0, 0.0],
            Some(ModelPoint { space: at, x, y })
                if at == space && x.is_finite() && y.is_finite() =>
            {
                [x, y]
            }
            Some(_) => return refuse("base", "invalid"),
        };
        // ブロックのレコードの表と、モデル空間のレコードと、BLOCKS の所が要る。ハンドルを持たない図面には足せない
        let dxf = self.target == Target::Dxf;
        let model = model_record(db);
        let (Some(next), Some(model), Some(table)) =
            (self.next, model, model.and_then(Object::owner))
        else {
            return refuse("table", "missing");
        };
        if dxf && (table_end(db, b"BLOCK_RECORD").is_none() || blocks_end(db).is_none()) {
            return refuse("table", "missing");
        }
        let record = next
            .checked_add(index)
            .filter(|handle| handle.checked_add(1).is_some())
            .ok_or_else(|| ("table".to_owned(), "handles"))?;
        let encoded = |code| {
            self.string(code, name)
                .ok_or_else(|| ("name".to_owned(), "encoding"))
        };
        let zero = self
            .named(8, b"LAYER", "0")
            .ok_or_else(|| ("table".to_owned(), "missing"))?;
        let real = |code, value| real(code, value).ok_or_else(|| ("base".to_owned(), "invalid"));
        // ブロックのレコード。DXF は、モデル空間のレコードのサブクラスマーカーと単位（70）・分解（280）・
        // 尺度（281）を写す。レイアウト（340）・拡張辞書・拡張データ・プレビューは写さない
        let mut values = Vec::new();
        if dxf {
            for value in &model.values {
                match value.code {
                    2 => values.push(encoded(2)?),
                    100 | 70 | 280 | 281 => values.push(value.clone()),
                    _ => {}
                }
            }
        } else {
            values.push(encoded(2)?);
        }
        let mut records = vec![Record {
            kind: b"BLOCK_RECORD",
            values,
            home: Home {
                owner: Some(table),
                blocks: false,
                paper: false,
            },
            followers: Vec::new(),
            annotation: None,
        }];
        let home = Home {
            owner: Some(record),
            blocks: true,
            paper: false,
        };
        // BLOCK（DXF Reference「BLOCK」）。名前は、2 と 3 に書く。BLOCK と ENDBLK は、DWG でも画層（8）を持つ。
        // DWG の書き戻しは、名前と基点をブロックのレコードに合わせて書く
        let markers = dxf && self.markers;
        let mut block = Vec::new();
        if markers {
            block.extend([text(100, "AcDbEntity"), zero.clone()]);
            block.push(text(100, "AcDbBlockBegin"));
        } else {
            block.push(zero.clone());
        }
        block.push(encoded(2)?);
        if dxf {
            block.push(int(70, 0));
        }
        block.extend([real(10, base[0])?, real(20, base[1])?, real(30, 0.0)?]);
        if dxf {
            block.extend([encoded(3)?, text(1, "")]);
        }
        records.push(Record {
            kind: b"BLOCK",
            values: block,
            home,
            followers: Vec::new(),
            annotation: None,
        });
        let mut count = 2u64;
        for (at, element) in symbol.space.elements.iter().enumerate() {
            let built = count
                .checked_add(index)
                .and_then(|index| self.build(element, &InSpace(space), home, index, true));
            let Some(built) = built else {
                return Err((format!("elements/{at}"), "unsupported"));
            };
            count = count.saturating_add(built.count());
            records.push(built);
        }
        let mut end = Vec::new();
        if markers {
            end.extend([text(100, "AcDbEntity"), zero, text(100, "AcDbBlockEnd")]);
        } else {
            end.push(zero);
        }
        records.push(Record {
            kind: b"ENDBLK",
            values: end,
            home,
            followers: Vec::new(),
            annotation: None,
        });
        // ENDBLK の後の HANDSEED まで付けられることを確かめる
        next.checked_add(index)
            .and_then(|first| first.checked_add(count))
            .and_then(|last| last.checked_add(1))
            .ok_or_else(|| ("table".to_owned(), "handles"))?;
        Ok(records)
    }
}
