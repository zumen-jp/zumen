//! 図形の画層・線種などの参照の付け替え。

use super::*;

/// 図形のスタイルの変更で、ハンドルのストリームに書く画層と線種（#723）。`None` は変えない
#[derive(Clone, Copy, Default)]
pub(super) struct Restyle {
    /// 画層のハンドル
    pub(super) layer: Option<u64>,
    /// 線種のフラグとハンドル（フラグが 3 のときだけ使う）
    pub(super) ltype: Option<(u8, u64)>,
    /// TEXT・MTEXT が指す文字スタイル
    pub(super) text_style: Option<u64>,
}

impl Restyle {
    /// `old` から `new` に変えた図形の、画層と線種の書き方。名前を表から引けなければ `None`
    pub(super) fn of(db: &AcadDb, old: &Object, new: &Object) -> Option<Self> {
        let codes = |object: &Object, code: i16| -> Vec<Data> {
            object
                .values
                .iter()
                .filter(|value| value.code == code)
                .map(|value| value.data.clone())
                .collect()
        };
        let layer = if codes(old, 8) == codes(new, 8) {
            None
        } else {
            Some(record_handle(db, b"LAYER", text_of(new, 8)?)?)
        };
        let ltype = if codes(old, 6) == codes(new, 6) {
            None
        } else {
            Some(ltype_ref(db, new)?)
        };
        let text_style = if matches!(new.kind.as_slice(), b"TEXT" | b"MTEXT")
            && codes(old, 7) != codes(new, 7)
        {
            Some(record_handle(db, b"STYLE", text_of(new, 7)?)?)
        } else {
            None
        };
        Some(Self {
            layer,
            ltype,
            text_style,
        })
    }

    /// `old` から `new` に変えた LAYER のレコードの線種（#723）。名前（変えたとき）かハンドル（元のまま）で持つ
    pub(super) fn layer(db: &AcadDb, old: &Object, new: &Object) -> Option<Self> {
        let first = |object: &Object| {
            object
                .values
                .iter()
                .find(|value| value.code == 6)
                .map(|value| value.data.clone())
        };
        let ltype = match first(new) {
            found if found == first(old) => None,
            Some(Data::Text(name)) => Some((3, record_handle(db, b"LTYPE", &name)?)),
            Some(Data::Handle(handle)) => Some((3, handle)),
            _ => return None,
        };
        Some(Self {
            layer: None,
            ltype,
            text_style: None,
        })
    }

    pub(super) fn is_empty(self) -> bool {
        self.layer.is_none() && self.ltype.is_none() && self.text_style.is_none()
    }

    /// 元のデータ `data` のハンドルのストリーム（`handles_at` から終わりまで）を、画層と線種のハンドルを書き直して写したもの。
    /// `slots` は元のデータの画層と線種のハンドルの位置。ハンドルは固い参照（符号 5）で、絶対の値で書く
    pub(super) fn handles(
        self,
        data: &[u8],
        handles_at: usize,
        slots: &StyleSlots,
    ) -> Option<BitWriter> {
        let mut out = BitWriter::default();
        if self.is_empty() {
            out.copy(data, handles_at..data.len() * 8)?;
            return Some(out);
        }
        out.copy(data, handles_at..slots.layer.start)?;
        match self.layer {
            Some(layer) => out.handle(5, layer),
            None => out.copy(data, slots.layer.clone())?,
        }
        let after = slots
            .ltype
            .as_ref()
            .map_or(slots.layer.end, |ltype| ltype.end);
        match (self.ltype, &slots.ltype) {
            (None, Some(ltype)) => out.copy(data, ltype.clone())?,
            (Some((3, handle)), _) => out.handle(5, handle),
            _ => {}
        }
        let after = if let Some(style) = self.text_style {
            let slot = slots.text_style.as_ref()?;
            out.copy(data, after..slot.start)?;
            out.handle(5, style);
            slot.end
        } else {
            after
        };
        out.copy(data, after..data.len() * 8)?;
        Some(out)
    }
}

/// 元のデータの、画層と線種のハンドルの位置
pub(super) type StyleSlots = crate::dwg::objects::StyleHandles;

/// 図形 `object` の線種（6）の、DWG のフラグとハンドル。名前がなければ ByLayer
pub(super) fn ltype_ref(db: &AcadDb, object: &Object) -> Option<(u8, u64)> {
    Some(match object.values.iter().any(|value| value.code == 6) {
        false => (0, 0),
        true => match text_of(object, 6)? {
            name if name.eq_ignore_ascii_case(b"ByLayer") => (0, 0),
            name if name.eq_ignore_ascii_case(b"ByBlock") => (1, 0),
            name if name.eq_ignore_ascii_case(b"Continuous") => (2, 0),
            name => (3, record_handle(db, b"LTYPE", name)?),
        },
    })
}
