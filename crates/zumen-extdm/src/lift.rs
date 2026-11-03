//! 拡張 DM のレコードを、地理の領域モデル（決定 0233）に移す。
//!
//! - ヘッダーのレコードを地物にする。型は `extdm:` とレコードタイプ（`extdm:E2` など）、ID はレコードの順の番号。
//! - 図郭レコードを親にし、グループヘッダーレコードの階層レベル（第 42 条）で入れ子にする。
//! - 分類コードを地物の分類コードにする。ほかのフィールドは、仕様の項目の名前の属性にする。
//! - 座標は図郭の原点（左下隅）からの値（(8)）で、座標値の単位（図郭レコード (b)）で m に直し、インデックス
//!   レコードの座標系の系の平面直角座標にする。

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_crs::{Crs, MapPoint, Zone};
use zumen_geo::{Attribute, Feature, Form, GeoModel, Geometry, Location, Position, Value};
use zumen_geom3d::{Shape, Xyz};
use zumen_sdk::{Cx, Error};

use crate::ExtDmFile;
use crate::record::{self, PAIRS, TRIPLES, VALUES, field, int};

pub(crate) fn lift(file: &ExtDmFile, cx: &mut Cx) -> Result<GeoModel, Error> {
    let mut reader = Reader {
        file,
        cx,
        model: GeoModel::default(),
        zone: None,
        sheet: None,
        stack: Vec::new(),
    };
    reader.model()?;
    Ok(reader.model)
}

/// 図郭。後に続く要素の座標を直すのに使う
struct Sheet {
    /// 図郭の左下隅の座標（m）。X は北、Y は東
    origin: [f64; 2],
    /// 座標値の 1 の長さ（m）と、座標値がないことを表す値。単位が読めなければ `None`
    unit: Option<(f64, i64)>,
    /// 座標系。分からないか、文書の座標系と違えば `None`
    crs: Option<Crs>,
}

/// 要素の座標の値（座標値の単位のまま）
#[derive(Clone, Copy)]
struct Raw {
    x: i64,
    y: i64,
    z: Option<i64>,
}

struct Reader<'f, 'c, 'b> {
    file: &'f ExtDmFile,
    cx: &'c mut Cx<'b>,
    model: GeoModel,
    /// インデックスレコードの座標系（平面直角座標系の系）
    zone: Option<Zone>,
    sheet: Option<Sheet>,
    /// 階層レベルと地物の番号。図郭の地物を 0 に置く
    stack: Vec<(i64, usize)>,
}

impl<'f> Reader<'f, '_, '_> {
    fn model(&mut self) -> Result<(), Error> {
        let records = self.file.records();
        let all: Vec<&[u8]> = (0..records.len()).map(|i| self.file.bytes(i)).collect();
        let mut index = 0;
        while let Some(line) = records.get(index) {
            let rest = all.get(index + 1..).unwrap_or_default();
            let Some(kind) = line.kind else {
                // ヘッダーのない実データのレコード。前のヘッダーの数より多いか、知らないレコードタイプ。続く同じ
                // レコードをまとめて知らせる
                let end = index
                    + 1
                    + records
                        .get(index + 1..)
                        .unwrap_or_default()
                        .iter()
                        .take_while(|line| line.kind.is_none())
                        .count();
                let text =
                    String::from_utf8_lossy(field(self.file.bytes(index), 0, 2)).into_owned();
                let at = self.at(index, end);
                self.cx.report(
                    Diagnostic::new(codes::EXTDM_RECORD_UNSUPPORTED)
                        .with("kind", text.trim())
                        .at(at),
                );
                index = end;
                continue;
            };
            // 実データのレコードは、ヘッダーの数だけ続く（L1 の分け方と同じ）
            let record = self.file.bytes(index);
            let follows = record::follows(kind, record, rest);
            let end = index + 1 + follows.min(rest.len());
            self.cx.check(Kind::Elements, 1).map_err(|stop| {
                let at = self.at(index, end);
                self.cx.report(stop.to_diagnostic().at(at));
                Error::Stopped(stop)
            })?;
            let data: Vec<&'f [u8]> = (index + 1..end).map(|i| self.file.bytes(i)).collect();
            if follows > data.len() {
                self.invalid(kind, "レコード数", index, end);
            }
            match kind {
                b"I" => self.index(index, end, record, &data),
                b"M" => self.sheet(index, end, record, &data),
                b"H" => self.group(index, end, record),
                b"G" => self.grid(index, end, record, &data)?,
                b"T" => self.tin(index, end, record, &data)?,
                _ => self.element(kind, index, end, record, &data)?,
            }
            index = end;
        }
        Ok(())
    }

    // ヘッダーのレコード

    /// インデックスレコード (a)〜(c)。座標系の系を覚え、計画機関名・作業規程・分類コードの対応を属性にする
    fn index(&mut self, index: usize, end: usize, record: &[u8], data: &[&[u8]]) {
        let feature = self.push(index, end, b"I", None);
        let mut attributes = Vec::new();
        match int(record, 2, 2) {
            Ok(Some(zone)) => {
                self.zone = u8::try_from(zone).ok().and_then(Zone::new);
                if self.zone.is_none() {
                    self.invalid(b"I", "座標系", index, end);
                }
            }
            _ => self.invalid(b"I", "座標系", index, end),
        }
        let name = self.text(field(record, 4, 30));
        self.number(&mut attributes, record, 34, 3, "図郭数", (b"I", index, end));
        self.number(
            &mut attributes,
            record,
            43,
            1,
            "転位処理フラグ",
            (b"I", index, end),
        );
        self.number(
            &mut attributes,
            record,
            44,
            1,
            "間断処理フラグ",
            (b"I", index, end),
        );
        self.number(
            &mut attributes,
            record,
            45,
            4,
            "西暦年号",
            (b"I", index, end),
        );
        self.push_text(&mut attributes, "作業規程名", field(record, 49, 30));
        if let Ok(Some(version)) = int(record, 79, 1) {
            attributes.push(attribute("バージョン", Value::Integer(version)));
            // 本仕様のバージョンは 2。旧 DM は 0 で、同じ書式とは限らない
            if version != 2 {
                let at = self.at(index, end);
                self.cx.report(
                    Diagnostic::new(codes::EXTDM_VERSION_UNSUPPORTED)
                        .with("version", u64::try_from(version).unwrap_or(0))
                        .at(at),
                );
            }
        }
        let sheets = int(record, 37, 2)
            .ok()
            .flatten()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0);
        for (row, data) in data.iter().enumerate() {
            if row < sheets {
                // (b) 図郭識別番号（A8 が 10 個）
                for column in 0..10 {
                    self.push_text(&mut attributes, "図郭識別番号", field(data, column * 8, 8));
                }
                continue;
            }
            // (c) 使用分類コードと標準の分類コードの対応
            let used = self.text(field(data, 0, 4));
            if used.is_empty() {
                continue;
            }
            let path = format!("使用分類コード/{used}/");
            self.push_text(
                &mut attributes,
                &format!("{path}標準の分類コード"),
                field(data, 4, 4),
            );
            self.push_text(
                &mut attributes,
                &format!("{path}使用データタイプフラグ"),
                field(data, 8, 9),
            );
            self.push_text(
                &mut attributes,
                &format!("{path}方向規定区分"),
                field(data, 17, 1),
            );
            self.push_text(
                &mut attributes,
                &format!("{path}座標次元区分"),
                field(data, 18, 1),
            );
            self.push_text(
                &mut attributes,
                &format!("{path}内容記述"),
                field(data, 19, 65),
            );
        }
        self.finish(
            feature,
            None,
            Some(name).filter(|n| !n.is_empty()),
            attributes,
            Vec::new(),
        );
    }

    /// 図郭レコード (a)〜(f)。図郭の原点・座標値の単位・測地成果から、後に続く要素の座標の直し方を決める
    fn sheet(&mut self, index: usize, end: usize, record: &[u8], data: &[&[u8]]) {
        self.stack.clear();
        let feature = self.push(index, end, b"M", None);
        self.stack.push((0, feature));
        let at = (b"M" as &[u8], index, end);
        let mut attributes = Vec::new();
        let id = self.text(field(record, 2, 8));
        self.push_text(&mut attributes, "図郭識別番号", field(record, 2, 8));
        let name = self.text(field(record, 10, 20));
        let level = int(record, 30, 5).ok().flatten();
        self.number(&mut attributes, record, 30, 5, "地図情報レベル", at);
        self.push_text(&mut attributes, "タイトル名", field(record, 35, 30));
        self.number(&mut attributes, record, 65, 2, "修正回数", at);
        self.number(&mut attributes, record, 67, 1, "バージョン", at);
        let b = data.first().copied().unwrap_or_default();
        let c = data.get(1).copied().unwrap_or_default();
        self.number(&mut attributes, b, 31, 6, "要素数", at);
        for (number, start) in (1..=8).zip((0..).step_by(8)) {
            let neighbor = field(c, start, 8);
            self.push_text(
                &mut attributes,
                &format!("隣接図郭識別番号/{number}"),
                neighbor,
            );
        }
        // (d)・(e)・(f) の組。修正のたびに後に足す。座標の直し方には、いちばん新しい組を使う
        let mut datum = None;
        let mut fractions = [0i64; 8];
        let mut at_set = 2;
        while let Some(d) = data.get(at_set) {
            self.push_text(&mut attributes, "作成年月", field(d, 0, 4));
            self.push_text(&mut attributes, "現地調査年月", field(d, 4, 4));
            self.push_text(&mut attributes, "入力機器名", field(d, 10, 30));
            self.push_text(&mut attributes, "公共測量承認番号", field(d, 40, 30));
            datum = int(d, 70, 1).ok().flatten();
            self.number(&mut attributes, d, 71, 1, "図郭識別コード", at);
            self.number(&mut attributes, d, 72, 1, "変換手法識別コード", at);
            if let Some(e) = data.get(at_set + 1) {
                self.push_text(&mut attributes, "作業機関名", field(e, 0, 40));
                for (slot, start) in fractions.iter_mut().zip((40..).step_by(4)) {
                    *slot = int(e, start, 4).ok().flatten().unwrap_or(0);
                }
            }
            let photos = int(d, 9, 1)
                .ok()
                .flatten()
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(0);
            for f in data.iter().skip(at_set + 2).take(photos) {
                self.push_text(&mut attributes, "撮影コース", f);
            }
            at_set += 2 + photos;
        }
        if let Some(datum) = datum {
            attributes.push(attribute("測地成果識別コード", Value::Integer(datum)));
        }
        // 図郭座標（m）と、メートル未満の端数（地図情報レベル 1000 以下は mm、2500 以上は cm）
        let fraction = if level.is_some_and(|level| level >= 2500) {
            0.01
        } else {
            0.001
        };
        let corner = |x: usize, y: usize, slot: usize| -> Option<[f64; 2]> {
            let x = int(b, x, 7).ok().flatten()?;
            let y = int(b, y, 7).ok().flatten()?;
            let fx = fractions.get(slot).copied().unwrap_or(0);
            let fy = fractions.get(slot + 1).copied().unwrap_or(0);
            Some([
                x as f64 + fx as f64 * fraction,
                y as f64 + fy as f64 * fraction,
            ])
        };
        // 左下・右上・左上・右下の順
        let corners = [
            corner(0, 7, 0),
            corner(14, 21, 2),
            corner(47, 54, 4),
            corner(61, 68, 6),
        ];
        let unit = match int(b, 44, 3) {
            Ok(Some(1)) => Some((0.001, -999_000)),
            Ok(Some(10)) => Some((0.01, -99_900)),
            Ok(Some(999)) => Some((1.0, -999)),
            _ => {
                self.invalid(b"M", "座標値の単位", index, end);
                None
            }
        };
        let Some(origin) = corners[0] else {
            self.invalid(b"M", "図郭座標", index, end);
            self.sheet = Some(Sheet {
                origin: [0.0, 0.0],
                unit: None,
                crs: None,
            });
            self.finish(
                feature,
                None,
                Some(name).filter(|n| !n.is_empty()),
                attributes,
                Vec::new(),
            );
            return;
        };
        let crs = self.crs(&id, datum, index, end);
        let mut geometries = Vec::new();
        if let (Some(_), [Some(ll), Some(ur), Some(ul), Some(lr)]) = (crs, corners) {
            let outer = [ll, lr, ur, ul]
                .into_iter()
                .map(|[x, y]| map(x, y, None))
                .collect();
            geometries.push(Geometry {
                lod: None,
                form: Form::Area {
                    outer,
                    holes: Vec::new(),
                },
            });
        }
        self.sheet = Some(Sheet { origin, unit, crs });
        self.finish(
            feature,
            None,
            Some(name).filter(|n| !n.is_empty()),
            attributes,
            geometries,
        );
    }

    /// 図郭の座標系。系はインデックスレコード、測地系は図郭レコード (d) の測地成果識別コードから決める。日本測地系
    /// （0）なら日本測地系の平面直角座標系、世界測地系（1・2）か書いていなければ JGD2011（JGD2024）の平面直角座標系
    /// とし、JGD2000 との区別がつかないので仮定したことを知らせる
    fn crs(&mut self, sheet: &str, datum: Option<i64>, index: usize, end: usize) -> Option<Crs> {
        let at = self.at(index, end);
        let Some(zone) = self.zone else {
            self.cx.report(
                Diagnostic::new(codes::EXTDM_CRS_UNSUPPORTED)
                    .with("sheet", sheet)
                    .at(at),
            );
            return None;
        };
        let crs = if datum == Some(0) {
            Crs::Tokyo(zone)
        } else {
            self.cx.report(
                Diagnostic::new(codes::EXTDM_DATUM_ASSUMED)
                    .with("sheet", sheet)
                    .at(at.clone()),
            );
            Crs::Plane(zone)
        };
        match self.model.crs {
            None => {
                self.model.crs = Some(crs);
                Some(crs)
            }
            Some(model) if model == crs => Some(crs),
            // 文書の座標系と違う図郭。座標系の違う地物を比べないよう、形を移さない
            Some(_) => {
                self.cx.report(
                    Diagnostic::new(codes::EXTDM_CRS_UNSUPPORTED)
                        .with("sheet", sheet)
                        .at(at),
                );
                None
            }
        }
    }

    /// グループヘッダーレコード（レイヤヘッダーレコードと要素グループヘッダーレコード）
    fn group(&mut self, index: usize, end: usize, record: &[u8]) {
        let level = self.level(b"H", record, 1, index, end);
        let feature = self.push(index, end, b"H", Some(level));
        self.stack.push((level, feature));
        let mut attributes = self.common(b"H", record, index, end, None);
        self.month(&mut attributes, record, 69, "取得年月");
        self.month(&mut attributes, record, 73, "更新の取得年月");
        self.month(&mut attributes, record, 77, "消去年月");
        self.number(
            &mut attributes,
            record,
            81,
            2,
            "数値化区分",
            (b"H", index, end),
        );
        let code = self.code(record);
        self.finish(feature, code, None, attributes, Vec::new());
    }

    /// 要素レコードと、その実データのレコード
    fn element(
        &mut self,
        kind: &'static [u8],
        index: usize,
        end: usize,
        record: &[u8],
        data: &[&[u8]],
    ) -> Result<(), Error> {
        let at = (kind, index, end);
        let level = self.level(kind, record, 2, index, end);
        let feature = self.push(index, end, kind, Some(level));
        let mut attributes = self.common(kind, record, index, end, Some(83));
        for (start, width, name) in [
            (18, 2, "図形区分"),
            (21, 2, "精度区分"),
            (23, 1, "注記区分"),
            (24, 2, "転位区分"),
            (26, 1, "間断区分"),
            (56, 2, "属性区分"),
        ] {
            self.number(&mut attributes, record, start, width, name, at);
        }
        self.push_text(&mut attributes, "属性データの書式", field(record, 58, 7));
        self.month(&mut attributes, record, 65, "取得年月");
        self.month(&mut attributes, record, 69, "更新の取得年月");
        self.month(&mut attributes, record, 73, "消去年月");
        // 属性数値は mm 単位（第 94 条）。等高線や点の標高（第 92 条・第 93 条）
        let value = match int(record, 49, 7) {
            Ok(value) => value,
            Err(()) => {
                self.invalid(kind, "属性数値", index, end);
                None
            }
        };
        let alti = value.map(|value| value as f64 / 1000.0);
        if let Some(alti) = alti {
            attributes.push(attribute(
                "属性数値",
                Value::Measure {
                    value: alti,
                    unit: Some("m".to_string()),
                },
            ));
        }
        let real = int(record, 20, 1).ok().flatten().unwrap_or(0);
        let count = int(record, 27, 4)
            .ok()
            .flatten()
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0);
        let mut name = None;
        let mut geometries = Vec::new();
        match kind {
            b"E7" => {
                name = self.annotation(&mut attributes, data);
                if let Some(point) = self.representative(record, alti, at) {
                    geometries.push(Form::Point(point));
                }
            }
            b"E8" => {
                for data in data {
                    self.push_text(&mut attributes, "属性データ", data);
                }
            }
            // 点データは要素レコードだけで、代表点の座標を使う（第 92 条）
            b"E5" if count == 0 || !matches!(real, 2 | 3 | 6) => {
                if let Some(point) = self.representative(record, alti, at) {
                    geometries.push(Form::Point(point));
                }
            }
            _ => {
                if !matches!(real, 2 | 3 | 6) {
                    self.invalid(kind, "実データ区分", index, end);
                } else if let Some(points) = self.coordinates(data, real != 2, count, at)? {
                    geometries.extend(self.shape(kind, points, alti, at));
                }
            }
        }
        let geometries = geometries
            .into_iter()
            .map(|form| Geometry { lod: None, form })
            .collect();
        let code = self.code(record);
        self.finish(feature, code, name, attributes, geometries);
        Ok(())
    }

    /// 注記レコード。縦横区分・文字列の方向・字大・字隔・線号を属性にし、注記データを返す。注記データは、
    /// レコードの区切りで全角の文字が分かれることがあるので、つないでから読む
    fn annotation(&mut self, attributes: &mut Vec<Attribute>, data: &[&[u8]]) -> Option<String> {
        let first = data.first()?;
        for (start, width, name) in [(0, 1, "縦横区分"), (1, 7, "文字列の方向"), (18, 2, "線号")]
        {
            if let Ok(Some(value)) = int(first, start, width) {
                attributes.push(attribute(name, Value::Integer(value)));
            }
        }
        // 字大・字隔は 0.1 mm 単位
        for (start, name) in [(8, "字大"), (13, "字隔")] {
            if let Ok(Some(value)) = int(first, start, 5) {
                attributes.push(attribute(
                    name,
                    Value::Measure {
                        value: value as f64 / 10.0,
                        unit: Some("mm".to_string()),
                    },
                ));
            }
        }
        let mut bytes = Vec::new();
        for data in data {
            bytes.extend_from_slice(field(data, 20, 64));
            // 短いレコードは、空白を補う
            bytes.resize(bytes.len() + 64 - field(data, 20, 64).len(), b' ');
        }
        Some(self.text(&bytes)).filter(|text| !text.is_empty())
    }

    /// グリッドヘッダーレコードとグリッドレコード。格子点を結ぶ三角形の集まりにする
    fn grid(
        &mut self,
        index: usize,
        end: usize,
        record: &[u8],
        data: &[&[u8]],
    ) -> Result<(), Error> {
        let at = (b"G" as &[u8], index, end);
        let level = self.level(b"G", record, 2, index, end);
        let feature = self.push(index, end, b"G", Some(level));
        let mut attributes = self.common(b"G", record, index, end, None);
        self.number(&mut attributes, record, 18, 4, "行数", at);
        self.number(&mut attributes, record, 22, 4, "列数", at);
        self.month(&mut attributes, record, 58, "取得年月");
        self.month(&mut attributes, record, 62, "更新の取得年月");
        self.month(&mut attributes, record, 66, "消去年月");
        self.number(&mut attributes, record, 70, 2, "図形区分", at);
        self.number(&mut attributes, record, 72, 2, "精度区分", at);
        let code = self.code(record);
        let mut geometries = Vec::new();
        let size = |start: usize| int(record, start, 7).ok().flatten();
        let dims = |start: usize| {
            int(record, start, 4)
                .ok()
                .flatten()
                .and_then(|n| usize::try_from(n).ok())
        };
        match (dims(18), dims(22), size(30), size(37), size(44), size(51)) {
            (Some(rows), Some(columns), Some(dx), Some(dy), Some(ox), Some(oy)) => {
                if let Some((sheet, (unit, missing))) = self.placed(at) {
                    let cells = rows.saturating_mul(columns);
                    self.segments(cells, at)?;
                    let values = self.values(data, cells, at);
                    let mut slots = vec![None; cells];
                    let mut positions: Vec<Xyz> = Vec::new();
                    for (cell, value) in values.into_iter().enumerate() {
                        let (Some(value), Some(slot)) = (value, slots.get_mut(cell)) else {
                            continue;
                        };
                        if value == missing {
                            continue;
                        }
                        // 原点（左下）から、行（X、北）の中で列（Y、東）の順
                        let (row, column) = ((cell / columns) as i64, (cell % columns) as i64);
                        let north = sheet[0] + (ox + row * dx) as f64 * unit;
                        let east = sheet[1] + (oy + column * dy) as f64 * unit;
                        *slot = u32::try_from(positions.len()).ok();
                        positions.push([east, north, value as f64 * unit]);
                    }
                    let slot = |r: usize, c: usize| slots.get(r * columns + c).copied().flatten();
                    let mut triangles = Vec::new();
                    for r in 0..rows.saturating_sub(1) {
                        for c in 0..columns.saturating_sub(1) {
                            let (sw, se, nw, ne) = (
                                slot(r, c),
                                slot(r, c + 1),
                                slot(r + 1, c),
                                slot(r + 1, c + 1),
                            );
                            if let (Some(sw), Some(se), Some(nw)) = (sw, se, nw) {
                                triangles.push([sw, se, nw]);
                            }
                            if let (Some(se), Some(ne), Some(nw)) = (se, ne, nw) {
                                triangles.push([se, ne, nw]);
                            }
                        }
                    }
                    geometries.push(Geometry {
                        lod: None,
                        form: Form::Solid(Shape::Triangles {
                            positions,
                            triangles,
                            closed: false,
                        }),
                    });
                }
            }
            _ => self.invalid(b"G", "グリッド", index, end),
        }
        self.finish(feature, code, None, attributes, geometries);
        Ok(())
    }

    /// 不整三角網ヘッダーレコードと不整三角網レコード。三角形の向きは決まっていないので、上から見て反時計回りに
    /// そろえる
    fn tin(
        &mut self,
        index: usize,
        end: usize,
        record: &[u8],
        data: &[&[u8]],
    ) -> Result<(), Error> {
        let at = (b"T" as &[u8], index, end);
        let level = self.level(b"T", record, 2, index, end);
        let feature = self.push(index, end, b"T", Some(level));
        let mut attributes = self.common(b"T", record, index, end, None);
        self.number(&mut attributes, record, 18, 2, "図形区分", at);
        self.number(&mut attributes, record, 20, 6, "三角形数", at);
        self.month(&mut attributes, record, 32, "取得年月");
        self.month(&mut attributes, record, 36, "更新の取得年月");
        self.month(&mut attributes, record, 40, "消去年月");
        self.number(&mut attributes, record, 44, 2, "精度区分", at);
        let code = self.code(record);
        let mut geometries = Vec::new();
        let count = int(record, 20, 6)
            .ok()
            .flatten()
            .and_then(|n| usize::try_from(n).ok());
        match count {
            Some(count) => {
                if let Some((sheet, (unit, missing))) = self.placed(at) {
                    let total = count.saturating_mul(9);
                    self.segments(total / 3, at)?;
                    let values = self.values(data, total, at);
                    let mut positions: Vec<Xyz> = Vec::new();
                    let mut triangles = Vec::new();
                    let mut skipped = false;
                    for chunk in values.chunks(9) {
                        let corners: Option<Vec<Xyz>> = chunk
                            .chunks(3)
                            .map(|p| match p {
                                [Some(x), Some(y), Some(z)] if *z != missing => Some([
                                    sheet[1] + *y as f64 * unit,
                                    sheet[0] + *x as f64 * unit,
                                    *z as f64 * unit,
                                ]),
                                _ => None,
                            })
                            .collect();
                        let Some([a, b, c]) = corners.and_then(|c| <[Xyz; 3]>::try_from(c).ok())
                        else {
                            skipped = true;
                            continue;
                        };
                        let Ok(first) = u32::try_from(positions.len()) else {
                            break;
                        };
                        let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
                        positions.extend(if cross < 0.0 { [a, c, b] } else { [a, b, c] });
                        triangles.push([first, first + 1, first + 2]);
                    }
                    if skipped {
                        self.invalid(b"T", "座標値", index, end);
                    }
                    geometries.push(Geometry {
                        lod: None,
                        form: Form::Solid(Shape::Triangles {
                            positions,
                            triangles,
                            closed: false,
                        }),
                    });
                }
            }
            None => self.invalid(b"T", "三角形数", index, end),
        }
        self.finish(feature, code, None, attributes, geometries);
        Ok(())
    }

    // 形

    /// 要素の座標の列（2 次元座標レコードか 3 次元座標レコード）。`count` 個に足りないか読めなければ `None`
    fn coordinates(
        &mut self,
        data: &[&[u8]],
        three: bool,
        count: usize,
        (kind, index, end): (&'static [u8], usize, usize),
    ) -> Result<Option<Vec<Raw>>, Error> {
        self.segments(count, (kind, index, end))?;
        let (per, width) = if three { (TRIPLES, 21) } else { (PAIRS, 14) };
        let mut points = Vec::with_capacity(count.min(data.len().saturating_mul(per)));
        'records: for data in data {
            for slot in 0..per {
                if points.len() == count {
                    break 'records;
                }
                let start = slot * width;
                let x = int(data, start, 7);
                let y = int(data, start + 7, 7);
                let z = if three {
                    int(data, start + 14, 7)
                } else {
                    Ok(None)
                };
                let (Ok(Some(x)), Ok(Some(y)), Ok(z)) = (x, y, z) else {
                    self.invalid(kind, "座標値", index, end);
                    return Ok(None);
                };
                points.push(Raw { x, y, z });
            }
        }
        if points.len() < count {
            self.invalid(kind, "データ数", index, end);
            return Ok(None);
        }
        Ok(Some(points))
    }

    /// 座標の列を、データタイプの形にする
    fn shape(
        &mut self,
        kind: &'static [u8],
        points: Vec<Raw>,
        alti: Option<f64>,
        at: (&'static [u8], usize, usize),
    ) -> Vec<Form> {
        let Some((sheet, (unit, missing))) = self.placed(at) else {
            return Vec::new();
        };
        let location = |p: &Raw| {
            let height = match p.z {
                Some(z) if z != missing => Some(z as f64 * unit),
                Some(_) => None,
                None => alti,
            };
            map(
                sheet[0] + p.x as f64 * unit,
                sheet[1] + p.y as f64 * unit,
                height,
            )
        };
        let locations: Vec<Location> = points.iter().map(location).collect();
        let (_, index, end) = at;
        match kind {
            // 面データ。始点と終点は同じ座標（第 41 条）
            b"E1" => {
                let mut outer = locations;
                if outer.len() > 1 && outer.first() == outer.last() {
                    outer.pop();
                }
                if outer.len() < 3 {
                    self.invalid(kind, "データ数", index, end);
                    return Vec::new();
                }
                vec![Form::Area {
                    outer,
                    holes: Vec::new(),
                }]
            }
            b"E2" => {
                if locations.len() < 2 {
                    self.invalid(kind, "データ数", index, end);
                    return Vec::new();
                }
                vec![Form::Line(locations)]
            }
            // 円データは円周上の 3 点、円弧データは始点・中間点・終点（第 41 条）
            b"E3" | b"E4" => {
                let [a, b, c] = match locations.as_slice() {
                    [a, b, c] => [*a, *b, *c],
                    _ => {
                        self.invalid(kind, "データ数", index, end);
                        return Vec::new();
                    }
                };
                let Some(points) = circle(a, b, c, kind == b"E3", unit) else {
                    self.invalid(kind, "座標値", index, end);
                    return Vec::new();
                };
                let at = self.at(index, end);
                self.cx.report(
                    Diagnostic::new(codes::EXTDM_CURVE_APPROXIMATED)
                        .with("kind", String::from_utf8_lossy(kind).into_owned())
                        .with("tolerance", unit)
                        .at(at),
                );
                if kind == b"E3" {
                    vec![Form::Area {
                        outer: points,
                        holes: Vec::new(),
                    }]
                } else {
                    vec![Form::Line(points)]
                }
            }
            // 点データの点群（標高点群など）
            b"E5" => locations.into_iter().map(Form::Point).collect(),
            // 方向データ。2 点 1 組で、1 点目が記号の位置、2 点目が向き（第 41 条）。組ごとに 2 点の線にする
            b"E6" => {
                if locations.len() < 2 || !locations.len().is_multiple_of(2) {
                    self.invalid(kind, "データ数", index, end);
                }
                let (pairs, _) = locations.as_chunks::<2>();
                pairs.iter().map(|pair| Form::Line(pair.to_vec())).collect()
            }
            _ => Vec::new(),
        }
    }

    /// 代表点の座標値（点データと注記データの原点、第 97 条）
    fn representative(
        &mut self,
        record: &[u8],
        alti: Option<f64>,
        (kind, index, end): (&'static [u8], usize, usize),
    ) -> Option<Location> {
        let (Ok(Some(x)), Ok(Some(y))) = (int(record, 35, 7), int(record, 42, 7)) else {
            self.invalid(kind, "代表点の座標値", index, end);
            return None;
        };
        let (sheet, (unit, _)) = self.placed((kind, index, end))?;
        Some(map(
            sheet[0] + x as f64 * unit,
            sheet[1] + y as f64 * unit,
            alti,
        ))
    }

    /// 座標を直すのに使う図郭の原点と座標値の単位。図郭がないか、座標系が分からなければ `None`
    fn placed(
        &mut self,
        (kind, index, end): (&'static [u8], usize, usize),
    ) -> Option<([f64; 2], (f64, i64))> {
        let Some(sheet) = &self.sheet else {
            self.invalid(kind, "図郭", index, end);
            return None;
        };
        // 座標系が分からない図郭と、単位が読めない図郭は、図郭レコードで知らせてある
        sheet.crs?;
        Some((sheet.origin, sheet.unit?))
    }

    /// グリッドレコードと不整三角網レコードの数値。読めない数値は `None` にし、足りなければ知らせる
    fn values(
        &mut self,
        data: &[&[u8]],
        total: usize,
        (kind, index, end): (&'static [u8], usize, usize),
    ) -> Vec<Option<i64>> {
        let mut values = Vec::with_capacity(total.min(data.len().saturating_mul(VALUES)));
        let mut broken = false;
        'records: for data in data {
            for slot in 0..VALUES {
                if values.len() == total {
                    break 'records;
                }
                let value = int(data, slot * 7, 7).ok().flatten();
                broken |= value.is_none();
                values.push(value);
            }
        }
        if broken || values.len() < total {
            self.invalid(kind, "数値", index, end);
        }
        values
    }

    // フィールド

    /// グループヘッダー・要素・グリッドヘッダー・不整三角網ヘッダーのレコードに共通のフィールド（地域分類・情報分類・要素識別番号）。
    /// `repeat` は要素識別番号反復回数のカラム
    fn common(
        &mut self,
        kind: &'static [u8],
        record: &[u8],
        index: usize,
        end: usize,
        repeat: Option<usize>,
    ) -> Vec<Attribute> {
        let mut attributes = Vec::new();
        self.number(
            &mut attributes,
            record,
            6,
            2,
            "地域分類",
            (kind, index, end),
        );
        self.number(
            &mut attributes,
            record,
            8,
            4,
            "情報分類",
            (kind, index, end),
        );
        match int(record, 12, 4) {
            Ok(Some(number)) => {
                // 4 桁を超える要素識別番号は、反復回数（1 から）で 10,000 ごとに数える
                let times = repeat
                    .and_then(|start| int(record, start, 1).ok().flatten())
                    .filter(|times| *times >= 1)
                    .unwrap_or(1);
                let number = (times - 1).saturating_mul(10_000).saturating_add(number);
                attributes.push(attribute("要素識別番号", Value::Integer(number)));
            }
            Ok(None) => {}
            Err(()) => self.invalid(kind, "要素識別番号", index, end),
        }
        attributes
    }

    /// 分類コード（レイヤと項目の 4 桁）。元の文字のまま
    fn code(&self, record: &[u8]) -> Option<String> {
        Some(self.text(field(record, 2, 4))).filter(|code| !code.is_empty())
    }

    /// 階層レベル。読めなければ `default`
    fn level(
        &mut self,
        kind: &'static [u8],
        record: &[u8],
        default: i64,
        index: usize,
        end: usize,
    ) -> i64 {
        match int(record, 16, 2) {
            Ok(Some(level)) => level,
            _ => {
                self.invalid(kind, "階層レベル", index, end);
                default
            }
        }
    }

    /// 整数のフィールドを属性にする。0 か空白（入力値なし）なら足さない
    fn number(
        &mut self,
        attributes: &mut Vec<Attribute>,
        record: &[u8],
        start: usize,
        width: usize,
        name: &str,
        (kind, index, end): (&'static [u8], usize, usize),
    ) {
        match int(record, start, width) {
            Ok(Some(0) | None) => {}
            Ok(Some(value)) => attributes.push(attribute(name, Value::Integer(value))),
            Err(()) => self.invalid(kind, "整数", index, end),
        }
    }

    /// 年月（西暦の下 2 桁と月の A4）を属性にする。未入力（"0000"）か空白なら足さない
    fn month(&self, attributes: &mut Vec<Attribute>, record: &[u8], start: usize, name: &str) {
        let text = self.text(field(record, start, 4));
        if !text.is_empty() && text != "0000" {
            attributes.push(attribute(name, Value::Text(text)));
        }
    }

    /// 文字型のフィールドを属性にする。空白なら足さない
    fn push_text(&self, attributes: &mut Vec<Attribute>, name: &str, bytes: &[u8]) {
        let text = self.text(bytes);
        if !text.is_empty() {
            attributes.push(attribute(name, Value::Text(text)));
        }
    }

    /// 文字型のフィールドの文字。前後の空白（全角の空白を含む）を除く
    fn text(&self, bytes: &[u8]) -> String {
        let text = if self.file.utf8() {
            String::from_utf8_lossy(bytes).into_owned()
        } else {
            encoding_rs::SHIFT_JIS
                .decode_without_bom_handling(bytes)
                .0
                .into_owned()
        };
        text.trim_matches([' ', '\u{3000}']).to_string()
    }

    // 地物

    /// 地物を足す。親は、図郭と、階層レベルの小さいグループヘッダーから決める
    fn push(&mut self, index: usize, end: usize, kind: &[u8], level: Option<i64>) -> usize {
        let parent = level.and_then(|level| {
            while self.stack.last().is_some_and(|(above, _)| *above >= level) {
                self.stack.pop();
            }
            self.stack.last().map(|(_, feature)| *feature)
        });
        let feature = self.model.features.len();
        self.model.features.push(Feature {
            id: index.to_string(),
            kind: format!("extdm:{}", String::from_utf8_lossy(kind)),
            parent,
            source: self.at(index, end),
            ..Feature::default()
        });
        feature
    }

    fn finish(
        &mut self,
        feature: usize,
        code: Option<String>,
        name: Option<String>,
        attributes: Vec<Attribute>,
        geometries: Vec<Geometry>,
    ) {
        if let Some(feature) = self.model.features.get_mut(feature) {
            feature.code = code;
            feature.name = name;
            feature.attributes = attributes;
            feature.geometries = geometries;
        }
    }

    fn segments(
        &mut self,
        amount: usize,
        (_, index, end): (&'static [u8], usize, usize),
    ) -> Result<(), Error> {
        self.cx
            .check(Kind::Segments, amount as u64)
            .map_err(|stop| {
                let at = self.at(index, end);
                self.cx.report(stop.to_diagnostic().at(at));
                Error::Stopped(stop)
            })
    }

    /// レコード `index` から `end` の前までのバイトの範囲
    fn at(&self, index: usize, end: usize) -> Ref {
        let records = self.file.records();
        let start = records.get(index).map_or(0, |line| line.range.start);
        let stop = records
            .get(end.saturating_sub(1).max(index))
            .map_or(start, |line| line.range.end);
        Ref::at_bytes(start as u64, stop as u64)
    }

    fn invalid(&mut self, kind: &[u8], field: &'static str, index: usize, end: usize) {
        let at = self.at(index, end);
        self.cx.report(
            Diagnostic::new(codes::EXTDM_RECORD_INVALID)
                .with("kind", String::from_utf8_lossy(kind).into_owned())
                .with("field", field)
                .at(at),
        );
    }
}

fn attribute(name: &str, value: Value) -> Attribute {
    Attribute {
        name: format!("extdm:{name}"),
        value,
    }
}

/// 平面直角座標の点（X は北、Y は東）
fn map(x: f64, y: f64, height: Option<f64>) -> Location {
    Location {
        position: Position::Map(MapPoint {
            northing: x,
            easting: y,
        }),
        height,
    }
}

/// 3 点を通る円（`full`）か、始点から中間点を通って終点までの円弧を、許容差 `tolerance`（m）の折れ線にする。
/// 3 点が一直線に並べば `None`。標高は、3 点が同じ標高のときだけ持たせる
fn circle(
    a: Location,
    b: Location,
    c: Location,
    full: bool,
    tolerance: f64,
) -> Option<Vec<Location>> {
    // 東を x、北を y にした平面で計算する
    let xy = |l: Location| match l.position {
        Position::Map(p) => [p.easting, p.northing],
        Position::Geo(p) => [p.longitude, p.latitude],
    };
    let ([ax, ay], [bx, by], [cx, cy]) = (xy(a), xy(b), xy(c));
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < f64::EPSILON || !d.is_finite() {
        return None;
    }
    let (a2, b2, c2) = (ax * ax + ay * ay, bx * bx + by * by, cx * cx + cy * cy);
    let center = [
        (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d,
        (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d,
    ];
    let radius = libm::hypot(ax - center[0], ay - center[1]);
    let angle = |x: f64, y: f64| libm::atan2(y - center[1], x - center[0]).to_degrees();
    let start = angle(ax, ay);
    let around = |to: f64| (to - start).rem_euclid(360.0);
    let sweep = if full {
        360.0
    } else {
        // 中間点が反時計回りの間にあれば反時計回り、なければ時計回り
        let (middle, last) = (around(angle(bx, by)), around(angle(cx, cy)));
        if middle <= last { last } else { last - 360.0 }
    };
    let mut points = Vec::new();
    zumen_geom2d::arc(center, radius, start, sweep, tolerance, &mut points);
    if full {
        points.pop();
    }
    let height = a
        .height
        .filter(|h| Some(*h) == b.height && Some(*h) == c.height);
    Some(
        points
            .into_iter()
            .map(|[east, north]| map(north, east, height))
            .collect(),
    )
}
