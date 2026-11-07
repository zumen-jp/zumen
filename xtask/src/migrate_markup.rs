//! 旧版の決定 0021 の JSON を、決定 0214・0218 の文書に移す（決定 0282）。

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};

use serde::Deserialize;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Limits};
use zumen_drawing::Hash;
use zumen_json::{self as json, Invalid, ObjectReader, Problem, Value};
use zumen_review::{Extra, Location, Markup, MarkupKind, Place, Review, Status, Target, Topic};

#[cfg(test)]
mod tests;

const JSON_LIMIT: u64 = 8 * 1024 * 1024;
const DRAWING_LIMIT: u64 = 256 * 1024 * 1024;
const ITEM_LIMIT: usize = 16_384;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mapping {
    target: String,
    default_view: Option<View>,
    #[serde(default)]
    views: BTreeMap<String, View>,
    #[serde(default)]
    topic_ids: BTreeMap<String, String>,
}

/// 旧版のビューに当たる現行の用紙か空間と、旧版の点をその座標に写す方法
#[derive(Deserialize)]
#[serde(untagged)]
enum View {
    /// 座標系が同じと確かめたビュー。点をそのまま使う
    Same(String),
    /// 現行の点 = 旧版の点 × scale + offset
    Moved(Moved),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Moved {
    view: String,
    scale: f64,
    offset: [f64; 2],
}

impl Moved {
    fn valid(&self) -> bool {
        self.scale.is_finite() && self.scale > 0.0 && self.offset.iter().all(|v| v.is_finite())
    }
}

impl View {
    fn id(&self) -> &str {
        match self {
            Self::Same(id) | Self::Moved(Moved { view: id, .. }) => id,
        }
    }

    /// 写した点。有限の数にならなければ `None`
    fn place(&self, [x, y]: [f64; 2]) -> Option<[f64; 2]> {
        let point = match self {
            Self::Same(_) => [x, y],
            Self::Moved(Moved {
                scale,
                offset: [dx, dy],
                ..
            }) => [x * scale + dx, y * scale + dy],
        };
        point.iter().all(|v| v.is_finite()).then_some(point)
    }
}

impl Mapping {
    /// 対応表のビューと、誤りの文でその場所を示す名前
    fn entries(&self) -> impl Iterator<Item = (String, &View)> {
        let default = self
            .default_view
            .iter()
            .map(|v| ("default_view".to_owned(), v));
        default.chain(
            self.views
                .iter()
                .map(|(name, v)| (format!("views の {name:?}"), v)),
        )
    }

    fn check(&self) -> Result<(), String> {
        // 対象とビューの ID は項目ごとに複写する。項目数と組み合わせて展開後の量を抑える。
        let ids = self.entries().map(|(_, view)| view.id());
        if std::iter::once(self.target.as_str())
            .chain(ids)
            .any(|id| id.is_empty() || id.len() > 1024)
        {
            return Err("対応表の対象とビューには 1〜1024 バイトの ID を指定してください".into());
        }
        let invalid = |view: &View| matches!(view, View::Moved(moved) if !moved.valid());
        if let Some((name, _)) = self.entries().find(|(_, view)| invalid(view)) {
            return Err(format!(
                "対応表の {name} の scale には正の有限の数を、offset には有限の数を 2 つ指定してください"
            ));
        }
        if !self.topic_ids.values().all(|id| is_uuid(id)) {
            return Err("対応表の topic_ids の値には UUID を指定してください".into());
        }
        Ok(())
    }

    /// 旧版の点をそのまま使えるのは、座標系が同じと確かめた JWW の用紙だけ（決定 0282）
    fn check_views(&self, format: &str) -> Result<(), String> {
        let unchecked = |view: &View| matches!(view, View::Same(id) if format != "jww" || !id.starts_with("sheet/"));
        if let Some((name, _)) = self.entries().find(|(_, view)| unchecked(view)) {
            return Err(format!(
                "対応表の {name} に view・scale・offset を指定してください。旧版の点をそのまま使えるのは、JWW の図面の用紙（sheet/）だけです"
            ));
        }
        Ok(())
    }
}

fn is_uuid(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

/// UUID でない指摘の ID を、旧版の図面の SHA-256 と組にして UUID（バージョン 8）にする。
/// 対応表がなくても、同じ旧版の文書からは同じ ID になる（決定 0282）
fn topic_uuid(sha: &str, id: &str) -> String {
    let mut input = b"zumen-review/legacy-markup\0".to_vec();
    for field in [sha, id] {
        input.extend_from_slice(&(field.len() as u64).to_le_bytes());
        input.extend_from_slice(field.as_bytes());
    }
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&crate::sha256::digest(&input)[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let mut text = String::with_capacity(36);
    for (index, byte) in bytes.into_iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        for nibble in [byte >> 4, byte & 0x0f] {
            text.extend(char::from_digit(nibble.into(), 16));
        }
    }
    text
}

fn array(value: &Value) -> Result<&[Value], Invalid> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(Invalid::new(Problem::Type)),
    }
}

fn point(value: &Value) -> Result<[f64; 2], Invalid> {
    let [x, y] = array(value)? else {
        return Err(Invalid::new(Problem::Value));
    };
    Ok([
        json::number(x).map_err(|e| e.in_index(0))?,
        json::number(y).map_err(|e| e.in_index(1))?,
    ])
}

fn points(value: &Value, minimum: usize) -> Result<Vec<[f64; 2]>, Invalid> {
    if array(value)?.len() < minimum {
        return Err(Invalid::new(Problem::Value));
    }
    json::array(value, point)
}

/// 移せない値は元の値で残す。現行の既知のキーと衝突しないよう、旧版のキーに共通の接頭辞を付ける。
fn retain(fields: Extra, prefix: &str, retained: &mut Vec<String>) -> Extra {
    fields
        .into_iter()
        .map(|(key, value)| {
            let escaped = key.replace('~', "~0").replace('/', "~1");
            retained.push(format!("{prefix}/{escaped}"));
            (format!("legacy.{key}"), value)
        })
        .collect()
}

/// 対象と、旧版が記録した図面の SHA-256 を返す。
fn target(
    value: &Value,
    bytes: &[u8],
    map: &Mapping,
    retained: &mut Vec<String>,
) -> Result<(Target, String), Invalid> {
    let mut object = ObjectReader::new(value)?;
    let sha = object.required("sha256", json::string)?;
    if sha != crate::sha256::hex(bytes) {
        return Err(Invalid::new(Problem::Value).in_key("sha256"));
    }
    let format = object.required("format", json::str)?;
    // 点をそのまま使う対応は、座標系を確かめた形式とビューにだけ許す。
    if format.is_empty() || map.check_views(format).is_err() {
        return Err(Invalid::new(Problem::Value).in_key("format"));
    }
    let mut extra = retain(object.rest(), "/drawing", retained);
    extra.push(("legacy.sha256".into(), Value::String(sha.clone())));
    // 旧版は SXF の入力の種類を format に持つ。現行の drawing の形式名は一律に sxf。
    let format = match format {
        "sfc" | "p21" | "sfz" | "p2z" => {
            extra.push(("legacy.format".into(), Value::String(format.into())));
            "sxf"
        }
        other => other,
    };
    let target = Target {
        id: map.target.clone(),
        format: format.into(),
        hash: Hash::document(bytes).to_string(),
        name: None,
        extra,
    };
    Ok((target, sha))
}

struct Migration {
    review: Review,
    retained: Vec<String>,
    /// 旧版が記録した図面の SHA-256
    sha: String,
    topic_ids: BTreeSet<String>,
    /// 使った対応表の topic_ids のキー
    mapped: BTreeSet<String>,
}

impl Migration {
    fn item<'a>(
        &mut self,
        value: &'a Value,
        prefix: &str,
        map: &Mapping,
        seen: &mut BTreeSet<String>,
    ) -> Result<&'a str, Invalid> {
        let mut item = ObjectReader::new(value)?;
        let id = item.required("id", json::str)?;
        if id.is_empty() || !seen.insert(id.to_owned()) {
            return Err(Invalid::new(Problem::Value).in_key("id"));
        }
        let old_view = item.optional("view", json::str)?;
        let view = match old_view {
            Some(name) => map.views.get(name),
            None => map.default_view.as_ref(),
        }
        .ok_or_else(|| Invalid::new(Problem::Missing).in_key("view"))?;
        let locate = |value: &Value| -> Result<[f64; 2], Invalid> {
            view.place(point(value)?)
                .ok_or_else(|| Invalid::new(Problem::Value))
        };
        let locate_all = |value: &Value, minimum: usize| -> Result<Vec<[f64; 2]>, Invalid> {
            points(value, minimum)?
                .into_iter()
                .enumerate()
                .map(|(index, point)| {
                    view.place(point)
                        .ok_or_else(|| Invalid::new(Problem::Value).in_index(index))
                })
                .collect()
        };
        // 写した点の元の値も、そのキーに残して報告する。
        let extras = |item: ObjectReader<'_>, moved: &str, retained: &mut Vec<String>| {
            let mut extra = retain(item.rest(), prefix, retained);
            if let (View::Moved(_), Some(original)) = (view, value.pointer(&format!("/{moved}"))) {
                retained.push(format!("{prefix}/{moved}"));
                extra.push((format!("legacy.{moved}"), original.clone()));
            }
            if let Some(old) = old_view {
                extra.push(("legacy.view".into(), Value::String(old.into())));
            }
            extra
        };
        let kind = item.required("kind", json::str)?;
        if kind == "finding" {
            // 対応表が先。旧版の UUID はそのまま使い、ほかは図面と項目の ID から決める。
            let topic_id = match map.topic_ids.get(id) {
                Some(mapped) => {
                    self.mapped.insert(id.to_owned());
                    mapped.clone()
                }
                None if is_uuid(id) => id.to_owned(),
                None => topic_uuid(&self.sha, id),
            };
            if !self.topic_ids.insert(topic_id.to_ascii_lowercase()) {
                return Err(Invalid::new(Problem::Value).in_key("id"));
            }
            let pin = item.required("position", locate)?;
            let status = item.required("status", |value| match json::str(value)? {
                "open" => Ok(Status::Open),
                "resolved" => Ok(Status::Resolved),
                "closed" => Ok(Status::Closed),
                _ => Err(Invalid::new(Problem::Value)),
            })?;
            // 多角形は範囲に近似する。元の頂点は rest に残し、このフィールドの位置も報告する。
            let extent = value
                .pointer("/points")
                .map(|value| {
                    let polygon = locate_all(value, 3).map_err(|e| e.in_key("points"))?;
                    let [x, y] = polygon[0];
                    Ok(polygon
                        .into_iter()
                        .fold([x, y, x, y], |[a, b, c, d], [x, y]| {
                            [a.min(x), b.min(y), c.max(x), d.max(y)]
                        }))
                })
                .transpose()?;
            let location = Location {
                id: id.into(),
                target: map.target.clone(),
                carried: None,
                extra: vec![],
                place: Place::Plane {
                    view: Some(view.id().into()),
                    pin: Some(pin),
                    extent,
                    elements: vec![],
                },
            };
            self.review.topics.push(Topic {
                id: topic_id,
                title: item
                    .optional("text", json::string)?
                    .unwrap_or_else(|| id.into()),
                status,
                author: item.optional("author", json::string)?,
                created: item.optional("createdAt", json::string)?,
                modified: item.optional("updatedAt", json::string)?,
                locations: vec![location],
                description: None,
                status_label: None,
                kind: None,
                priority: None,
                labels: vec![],
                assignee: None,
                due: None,
                modified_by: None,
                related: vec![],
                comments: vec![],
                origin: None,
                extra: extras(item, "position", &mut self.retained),
            });
            return Ok(id);
        }

        let (kind, points, text, moved) = match kind {
            "redline" | "cloud" => {
                let cloud = kind == "cloud";
                let points = item.required("points", |value| {
                    locate_all(value, if cloud { 3 } else { 2 })
                })?;
                let text = item.optional("text", json::string)?;
                (
                    if cloud {
                        MarkupKind::Cloud
                    } else {
                        MarkupKind::Redline
                    },
                    points,
                    text,
                    "points",
                )
            }
            "text" => (
                MarkupKind::Text,
                vec![item.required("position", locate)?],
                Some(item.required("text", json::string)?),
                "position",
            ),
            "photo" => (
                MarkupKind::Photo,
                vec![item.required("position", locate)?],
                Some(item.required("photo", json::string)?),
                "position",
            ),
            _ => return Err(Invalid::new(Problem::Value).in_key("kind")),
        };
        self.review.markups.push(Markup {
            id: id.into(),
            kind,
            target: map.target.clone(),
            view: Some(view.id().into()),
            points,
            text,
            topic: None,
            carried: None,
            extra: extras(item, moved, &mut self.retained),
        });
        Ok(id)
    }
}

fn migrate(src: &Value, drawing: &[u8], map: &Mapping) -> Result<Migration, Invalid> {
    let mut source = ObjectReader::new(src)?;
    source.constant("format", "zumen-markup")?;
    source.required("version", |v| json::uint(v, 1, 1))?;
    let mut retained = vec![];
    let (target, sha) = source.required("drawing", |v| target(v, drawing, map, &mut retained))?;
    let mut migration = Migration {
        review: Review::default(),
        retained,
        sha,
        topic_ids: BTreeSet::new(),
        mapped: BTreeSet::new(),
    };
    migration.review.targets.push(target);
    let items = source.required("items", array)?;
    if items.len() > ITEM_LIMIT {
        return Err(Invalid::new(Problem::Value).in_key("items"));
    }
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for (index, value) in items.iter().enumerate() {
        let id = migration
            .item(value, &format!("/items/{index}"), map, &mut seen)
            .map_err(|e| e.in_index(index).in_key("items"))?;
        order.push(Value::String(id.into()));
    }
    migration.review.extra = retain(source.rest(), "", &mut migration.retained);
    // 旧版は図形と指摘を同じ列の順で描く。現行の文書では 2 つの列に分かれる。
    if !migration.review.markups.is_empty() && !migration.review.topics.is_empty() {
        migration
            .review
            .extra
            .push(("legacy_item_order".into(), Value::Array(order)));
        migration.retained.push("/items".into());
    }
    Ok(migration)
}

fn read(path: &str, label: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|f| f.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("{label}を読めませんでした（{}）", e.kind()))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{label}が {limit} バイトの上限を超えました"));
    }
    Ok(bytes)
}

pub(crate) fn run(markup: &str, drawing: &str, mapping: &str, output: &str) -> Result<(), String> {
    let mapping = read(mapping, "対応表", 1024 * 1024)?;
    let map: Mapping =
        toml::from_slice(&mapping).map_err(|_| "対応表の TOML を読めませんでした")?;
    map.check()?;
    let markup = read(markup, "旧版の JSON", JSON_LIMIT)?;
    let value = json::parse(&markup)
        .map_err(|e| format!("旧版の JSON のバイト位置 {} を読めませんでした", e.offset))?;
    if let Some(Value::String(format)) = value.pointer("/drawing/format") {
        map.check_views(format)?;
    }
    let drawing = read(drawing, "対象の図面", DRAWING_LIMIT)?;
    let migration = migrate(&value, &drawing, &map).map_err(|e| {
        format!(
            "旧版の JSON の {:?} と対応表・図面を確認してください",
            e.pointer()
        )
    })?;
    // 使われない対応は、ID の書き違いとして止める。
    if let Some(id) = map
        .topic_ids
        .keys()
        .find(|id| !migration.mapped.contains(*id))
    {
        return Err(format!(
            "対応表の topic_ids の {id:?} に当たる旧版の指摘がありません"
        ));
    }
    let bytes = migration.review.to_bytes();
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    Review::read(&bytes, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)).map_err(|_| {
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code().name()).collect();
        format!("移行した文書を読み直せませんでした（{}）", codes.join("・"))
    })?;
    let mut file = File::create_new(output)
        .map_err(|e| format!("新しい出力先を作れませんでした（{}）", e.kind()))?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        drop(file);
        fs::remove_file(output).map_err(|_| "書き込みが失敗し、途中の出力を消せませんでした")?;
        return Err(format!("出力を書けませんでした（{}）", error.kind()));
    }
    eprintln!(
        "図形 {} 件、指摘 {} 件を移行しました。拡張フィールドに残した値は {} 件です。",
        migration.review.markups.len(),
        migration.review.topics.len(),
        migration.retained.len()
    );
    for pointer in migration.retained {
        eprintln!("  {pointer:?}");
    }
    Ok(())
}
