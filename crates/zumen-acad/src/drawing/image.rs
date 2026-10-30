//! IMAGE と PDFUNDERLAY などの下敷きを、外部のファイルを指す資源とともに移す（決定 0114）。

use super::{Lifted, Lifter, invalid, non_planar, source};
use crate::db::{AcadDb, Data, Object};
use zumen_drawing::{Clip, Length, Resource, Underlay, UnderlayFrame, UnderlayKind};

impl Lifter<'_> {
    /// 下敷きが指す定義（340）の、参照名（1）。定義が `definition` の種類でなければ読めないとする。
    /// 定義と出所を返す
    fn resource<'a>(
        &self,
        db: &'a AcadDb,
        object: &Object,
        definition: &[u8],
    ) -> Lifted<(&'a Object, Resource)> {
        let kind = object.kind.as_slice();
        let found = match object.first(340) {
            Some(Data::Handle(handle)) => db.get(*handle),
            _ => None,
        }
        .filter(|found| found.kind == definition)
        .ok_or_else(|| invalid(kind, 340))?;
        let path = self
            .string(found, 1)?
            .ok_or_else(|| invalid(&found.kind, 1))?;
        Ok((
            found,
            Resource {
                path,
                definition: source(found),
            },
        ))
    }

    /// IMAGE（DXF Reference「IMAGE」「IMAGEDEF」、決定 0114）。挿入点（10）は左下の角、1 画素の横と縦の向きと
    /// 大きさ（11・21・31 と 12・22・32）、画素の数（13・23）から、右下と左上の角を求める。表示の印（70）の 1 は
    /// 表示する。切り抜く（280 が 1）なら、境界（71 が 1 なら長方形、2 なら多角形）の頂点（14・24）を、数（91）と
    /// 合わせて読み、290 が 1 なら内側を隠す。明るさ・コントラスト・薄さ（281〜283）は、まだ移さない
    pub(super) fn image<P>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Underlay<P>> {
        let kind = object.kind.as_slice();
        let (_, resource) = self.resource(db, object, b"IMAGEDEF")?;
        if self.real(object, 30, Some(0.0))? != 0.0
            || self.real(object, 31, Some(0.0))? != 0.0
            || self.real(object, 32, Some(0.0))? != 0.0
        {
            return Err(non_planar(kind));
        }
        let [x, y] = [self.real(object, 10, None)?, self.real(object, 20, None)?];
        let u = [self.real(object, 11, None)?, self.real(object, 21, None)?];
        let v = [self.real(object, 12, None)?, self.real(object, 22, None)?];
        let size = [self.real(object, 13, None)?, self.real(object, 23, None)?];
        if size.iter().any(|pixels| *pixels <= 0.0) {
            return Err(invalid(kind, 13));
        }
        let [width, height] = size;
        let clip = if self.int(object, 280, 0)? == 1 {
            let vertices: Vec<[f64; 2]> = self.points(kind, &object.values, 14, &|x, y| [x, y])?;
            let expected = match self.int(object, 71, 1)? {
                1 => 2,
                2 => vertices.len().max(3),
                _ => return Err(invalid(kind, 71)),
            };
            if usize::try_from(self.int(object, 91, -1)?).ok() != Some(vertices.len())
                || vertices.len() != expected
            {
                return Err(invalid(kind, 91));
            }
            Some(Clip {
                vertices,
                inverted: self.flag(object, 290, false)?,
            })
        } else {
            None
        };
        Ok(Underlay {
            kind: UnderlayKind::Image,
            resource,
            page: None,
            frame: UnderlayFrame::Pixels {
                size,
                origin: point(x, y),
                right: point(x + u[0] * width, y + u[1] * width),
                top: point(x + v[0] * height, y + v[1] * height),
            },
            clip,
            visible: self.int(object, 70, 1)? & 1 != 0,
        })
    }

    /// PDFUNDERLAY・DWFUNDERLAY・DGNUNDERLAY（DXF Reference「UNDERLAY」「UNDERLAYDEFINITION」、決定 0114）。
    /// 定義の 2 はページなどの名前。挿入点（10）・尺度（41・42）・回転（50）を持つ。印（280）の 2 は表示する、1 は
    /// 切り抜く、16 は内側を切り抜く（内側を隠す）。切り抜くなら、境界の頂点（11・21）を読む。コントラスト・薄さ・
    /// 単色（281・282 と 280 の 4・8）は、まだ移さない
    pub(super) fn underlay<P>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Underlay<P>> {
        let kind = object.kind.as_slice();
        let (underlay, definition): (UnderlayKind, &[u8]) = match kind {
            b"PDFUNDERLAY" => (UnderlayKind::Pdf, b"PDFDEFINITION"),
            b"DWFUNDERLAY" => (UnderlayKind::Dwf, b"DWFDEFINITION"),
            _ => (UnderlayKind::Dgn, b"DGNDEFINITION"),
        };
        let (found, resource) = self.resource(db, object, definition)?;
        let scale = |code: i16| -> Lifted<Length> {
            match self.real(object, code, Some(1.0))? {
                scale if scale > 0.0 => Ok(length(scale)),
                _ => Err(invalid(kind, code)),
            }
        };
        let flags = self.int(object, 280, 2)?;
        let clip = if flags & 1 != 0 {
            let vertices: Vec<[f64; 2]> = self.points(kind, &object.values, 11, &|x, y| [x, y])?;
            if vertices.len() < 2 {
                return Err(invalid(kind, 11));
            }
            Some(Clip {
                vertices,
                inverted: flags & 16 != 0,
            })
        } else {
            None
        };
        Ok(Underlay {
            kind: underlay,
            resource,
            page: self.string(found, 2)?,
            frame: UnderlayFrame::Scaled {
                origin: self.planar_point(object, 10, point)?,
                scale: [scale(41)?, scale(42)?],
                rotation: self.real(object, 50, Some(0.0))?,
            },
            clip,
            visible: flags & 2 != 0,
        })
    }
}
