//! 図面モデルから新しい DXF（R2004）を作るときのひな形（決定 0206・0240）。
//!
//! ひな形は、表の既定の項目（線種 3 つ・画層 0・文字スタイルと寸法スタイルの Standard）、記号の定義ごとの空の部品、
//! モデルと用紙のレイアウト、用紙のビューポートと、それらのハンドルを持つ。図面モデルの表の項目と要素は、ひな形を
//! 読んだあとに書き戻し（apply）の追加で足し、表の件数は `AcadDb::finish_new_dxf_tables` で仕上げる。

use std::collections::BTreeMap;

use zumen_drawing::{Drawing2D, Sheet, SymbolKey, Unit, View};

use crate::charset::Charset;

/// 新しい DXF（R2004）のひな形と、記号の定義ごとに書いた部品の名前
#[derive(Clone, Debug)]
pub struct NewDxfTemplate {
    bytes: Vec<u8>,
    blocks: BTreeMap<(String, String), String>,
}

impl NewDxfTemplate {
    /// 図面モデル `drawing` のひな形を作る。最初の空間の単位、1 枚目の用紙の大きさとビュー、記号の定義を使う。
    /// 外部参照の記号の定義（`external` を持つもの）は書かず、診断にもしない。書かなかった定義と、それを置いた
    /// 記号（[`Self::block`] が `None` を返すもの）は、呼び出し側が診断にする
    pub fn new(drawing: &Drawing2D) -> Self {
        let blocks = block_names(drawing);
        let bytes = template(drawing, &blocks);
        Self { bytes, blocks }
    }

    /// ひな形の DXF。文字は ANSI_932（Shift_JIS）で書く
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// 記号の定義 `key` を書いた部品の名前。外部参照の定義と、図面にない定義は `None`
    pub fn block(&self, key: &SymbolKey) -> Option<&str> {
        self.blocks.get(&key_of(key)).map(String::as_str)
    }

    /// 用紙 `sheet` のビュー `view` を、ひな形の用紙空間に VIEWPORT で書くか（決定 0240）。ひな形は 1 枚目の用紙の
    /// ビューだけを書くので、`sheet` が 1 枚目の用紙のときの答えになる（ほかの用紙では、書ける形かだけを答える）。
    /// 書かないビューは、呼び出し側が用紙の要素に焼き込むか落とす
    pub fn writes_viewport(view: &View, sheet: &Sheet) -> bool {
        port(view, sheet).is_some()
    }
}

fn key_of(key: &SymbolKey) -> (String, String) {
    (key.namespace.to_string(), key.name.clone())
}

/// 記号の定義ごとの、書いた先の部品の名前。外部参照の定義は書かない。無名の定義と、ANSI_932 に戻せない名前には
/// 名前を付ける
fn block_names(drawing: &Drawing2D) -> BTreeMap<(String, String), String> {
    let mut names = BTreeMap::new();
    let mut used: BTreeMap<String, ()> = BTreeMap::new();
    for reserved in ["*model_space", "*paper_space"] {
        used.insert(reserved.to_string(), ());
    }
    for (index, definition) in drawing.symbols.iter().enumerate() {
        if definition.external.is_some() {
            continue;
        }
        let base = if definition.anonymous || definition.key.name.starts_with('*') {
            format!("zumen_{index}")
        } else {
            definition
                .key
                .name
                .chars()
                .map(|c| {
                    if "<>/\\\":;?*|=,`".contains(c) || c.is_control() {
                        '_'
                    } else {
                        c
                    }
                })
                .collect()
        };
        // 読む側と同じ型で符号化し、同じ名前に戻るものだけを使う（決定 0192）
        let base = if base.is_empty() || Charset::ShiftJis.encode(&base).is_err() {
            format!("zumen_{index}")
        } else {
            base
        };
        let mut name = base.clone();
        let mut repeat = 1;
        while used.contains_key(&name.to_lowercase()) {
            name = format!("{base}_{repeat}");
            repeat += 1;
        }
        used.insert(name.to_lowercase(), ());
        names.insert(key_of(&definition.key), name);
    }
    names
}

/// ひな形の DXF（R2004）。DXF Reference「TABLES Section」「Common Group Codes for
/// Symbol Table Entries」「BLOCK」「ENDBLK」「BLOCK_RECORD」「DICTIONARY」「LAYOUT」に従い、
/// 表とブロックの所有者・サブクラスと、モデル・用紙のレイアウトを持つ。用紙への追加には空の LAYOUT を使う。
fn template(drawing: &Drawing2D, blocks: &BTreeMap<(String, String), String>) -> Vec<u8> {
    let mut handle = 0x10_u64;
    let mut next = || {
        handle += 1;
        format!("{handle:X}")
    };
    let root = next();
    let layouts = next();
    let model_layout = next();
    let paper_layout = next();
    let mut tables = String::from("0\nSECTION\n2\nTABLES\n");
    let table = next();
    tables.push_str(&table_start("LTYPE", &table, 3));
    for name in ["ByBlock", "ByLayer", "Continuous"] {
        let description = if name == "Continuous" {
            "Solid line"
        } else {
            ""
        };
        tables.push_str(&format!(
            "0\nLTYPE\n5\n{}\n330\n{table}\n100\nAcDbSymbolTableRecord\n100\nAcDbLinetypeTableRecord\n2\n{name}\n70\n0\n3\n{description}\n72\n65\n73\n0\n40\n0.0\n",
            next()
        ));
    }
    tables.push_str("0\nENDTAB\n");
    let table = next();
    tables.push_str(&table_start("LAYER", &table, 1));
    tables.push_str(&format!(
        "0\nLAYER\n5\n{}\n330\n{table}\n100\nAcDbSymbolTableRecord\n100\nAcDbLayerTableRecord\n2\n0\n70\n0\n62\n7\n6\nContinuous\n290\n1\n370\n-3\n",
        next()
    ));
    tables.push_str("0\nENDTAB\n");
    let table = next();
    let style = next();
    tables.push_str(&table_start("STYLE", &table, 1));
    tables.push_str(&format!(
        "0\nSTYLE\n5\n{style}\n330\n{table}\n100\nAcDbSymbolTableRecord\n100\nAcDbTextStyleTableRecord\n2\nStandard\n70\n0\n40\n0.0\n41\n1.0\n50\n0.0\n71\n0\n42\n2.5\n3\ntxt\n4\n\n"
    ));
    tables.push_str("0\nENDTAB\n");
    let table = next();
    tables.push_str(&table_start("DIMSTYLE", &table, 1));
    tables.push_str("100\nAcDbDimStyleTable\n71\n0\n");
    tables.push_str(&format!(
        "0\nDIMSTYLE\n105\n{}\n330\n{table}\n100\nAcDbSymbolTableRecord\n100\nAcDbDimStyleTableRecord\n2\nStandard\n70\n0\n340\n{style}\n",
        next()
    ));
    tables.push_str("0\nENDTAB\n");
    let table = next();
    let model_record = next();
    let paper_record = next();
    let mut parts = vec![
        (
            "*Model_Space".to_string(),
            [0.0, 0.0],
            model_record.clone(),
            Some(&model_layout),
        ),
        (
            "*Paper_Space".to_string(),
            [0.0, 0.0],
            paper_record.clone(),
            Some(&paper_layout),
        ),
    ];
    for definition in &drawing.symbols {
        if let Some(name) = blocks.get(&key_of(&definition.key)) {
            let base = definition.base.map_or([0.0, 0.0], |base| [base.x, base.y]);
            parts.push((name.clone(), base, next(), None));
        }
    }
    tables.push_str(&table_start("BLOCK_RECORD", &table, parts.len()));
    let mut section = String::from("0\nSECTION\n2\nBLOCKS\n");
    for (name, [x, y], record, layout) in &parts {
        tables.push_str(&format!(
            "0\nBLOCK_RECORD\n5\n{record}\n330\n{table}\n100\nAcDbSymbolTableRecord\n100\nAcDbBlockTableRecord\n2\n{name}\n"
        ));
        if let Some(layout) = layout {
            tables.push_str(&format!("340\n{layout}\n"));
        }
        section.push_str(&format!(
            "0\nBLOCK\n5\n{}\n330\n{record}\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockBegin\n2\n{name}\n70\n0\n10\n{x:?}\n20\n{y:?}\n30\n0.0\n3\n{name}\n1\n\n0\nENDBLK\n5\n{}\n330\n{record}\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockEnd\n",
            next(),
            next()
        ));
    }
    tables.push_str("0\nENDTAB\n0\nENDSEC\n");
    section.push_str("0\nENDSEC\n");
    let unit = match drawing.spaces.first().map(|space| space.unit) {
        Some(Unit::Millimeter) => 4,
        Some(Unit::Centimeter) => 5,
        Some(Unit::Meter) => 6,
        Some(Unit::Inch) => 1,
        Some(Unit::Foot) => 2,
        None => 4,
        _ => 0,
    };
    // 用紙の座標は従来どおり INSUNITS の単位。LAYOUT の印刷尺度にも 1 単位の mm を書く。
    let mm = drawing
        .spaces
        .first()
        .and_then(|space| space.unit.millimeters())
        .unwrap_or(1.0);
    let size = drawing
        .sheets
        .first()
        .and_then(|sheet| sheet.size)
        .map(|size| size.map(|s| s.0));
    let limits = size.map_or(String::new(), |[width, height]| {
        format!(
            "9\n$PLIMMIN\n10\n0.0\n20\n0.0\n9\n$PLIMMAX\n10\n{:?}\n20\n{:?}\n",
            width / mm,
            height / mm
        )
    });
    let mut entities = String::from("0\nSECTION\n2\nENTITIES\n");
    if let Some(sheet) = drawing.sheets.first() {
        entities.push_str(&viewports(sheet, mm, &paper_record, &mut next));
    }
    entities.push_str("0\nENDSEC\n");
    let mut objects = format!(
        "0\nSECTION\n2\nOBJECTS\n0\nDICTIONARY\n5\n{root}\n330\n0\n100\nAcDbDictionary\n281\n1\n3\nACAD_LAYOUT\n350\n{layouts}\n0\nDICTIONARY\n5\n{layouts}\n330\n{root}\n100\nAcDbDictionary\n281\n1\n3\nModel\n350\n{model_layout}\n3\nLayout1\n350\n{paper_layout}\n"
    );
    objects.push_str(&layout(
        &model_layout,
        &layouts,
        &model_record,
        "Model",
        None,
        1.0,
        true,
    ));
    objects.push_str(&layout(
        &paper_layout,
        &layouts,
        &paper_record,
        "Layout1",
        size,
        mm,
        false,
    ));
    objects.push_str("0\nENDSEC\n");
    // VIEWPORT を含むすべてのハンドルを割り当てた後の、まだ使っていない値。
    let seed = next();
    let out = format!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$DWGCODEPAGE\n3\nANSI_932\n9\n$INSUNITS\n70\n{unit}\n{limits}9\n$HANDSEED\n5\n{seed}\n0\nENDSEC\n{tables}{section}{entities}{objects}0\nEOF\n"
    );
    // ASCII 以外は、block_names が ANSI_932 に戻せると確かめた名前だけなので、符号化は失敗しない。失敗したら、
    // 空のバイト列にして、読む所で失敗させる
    let encoded = Charset::ShiftJis.encode(&out);
    debug_assert!(encoded.is_ok());
    encoded.unwrap_or_default()
}

fn table_start(name: &str, handle: &str, count: usize) -> String {
    format!("0\nTABLE\n2\n{name}\n5\n{handle}\n330\n0\n100\nAcDbSymbolTable\n70\n{count}\n")
}

/// DXF Reference「LAYOUT」「PLOTSETTINGS」。印刷する用紙の単位は mm、尺度は mm/図面単位。
fn layout(
    handle: &str,
    owner: &str,
    block: &str,
    name: &str,
    size: Option<[f64; 2]>,
    mm: f64,
    model: bool,
) -> String {
    let [width, height] = size.unwrap_or([0.0, 0.0]);
    let [x, y] = [width / mm, height / mm];
    let flags = if model { 1024 } else { 0 };
    let tab = usize::from(!model);
    format!(
        "0\nLAYOUT\n5\n{handle}\n330\n{owner}\n100\nAcDbPlotSettings\n1\n\n2\n\n4\n\n6\n\n40\n0.0\n41\n0.0\n42\n0.0\n43\n0.0\n44\n{width:?}\n45\n{height:?}\n46\n0.0\n47\n0.0\n142\n{mm:?}\n143\n1.0\n70\n{flags}\n72\n1\n73\n0\n74\n5\n7\n\n75\n0\n147\n{mm:?}\n100\nAcDbLayout\n1\n{name}\n70\n1\n71\n{tab}\n10\n0.0\n20\n0.0\n11\n{x:?}\n21\n{y:?}\n12\n0.0\n22\n0.0\n32\n0.0\n14\n0.0\n24\n0.0\n34\n0.0\n15\n{x:?}\n25\n{y:?}\n35\n0.0\n146\n0.0\n13\n0.0\n23\n0.0\n33\n0.0\n16\n1.0\n26\n0.0\n36\n0.0\n17\n0.0\n27\n1.0\n37\n0.0\n76\n0\n330\n{block}\n"
    )
}

/// DXF の VIEWPORT で書けるビューの、用紙の中心と幅・高さ（mm）と、実寸の中心と高さ
struct Port {
    center: [f64; 2],
    size: [f64; 2],
    target: [f64; 2],
    height: f64,
}

/// ビューを VIEWPORT で書けるなら、その値（決定 0240）。読む側（`zumen-acad`）が扱う形に合わせ、回転と鏡映の
/// ない相似で、表示しないレイヤを持たないものだけを書く。枠は、切り抜きが軸に沿った長方形ならそれ、切り抜きが
/// なければ用紙の全体にする
fn port(view: &View, sheet: &Sheet) -> Option<Port> {
    let similar = view.transform.similarity()?;
    let turned = similar.rotation.rem_euclid(360.0);
    if similar.mirrored || turned.min(360.0 - turned) > 1e-9 || !view.hidden_layers.is_empty() {
        return None;
    }
    let [x0, y0, x1, y1] = match view.clip.as_slice() {
        [] => {
            let [width, height] = sheet.size?;
            [0.0, 0.0, width.0, height.0]
        }
        corners @ [_, _, _, _] => {
            let xs = corners.iter().map(|p| p.x);
            let ys = corners.iter().map(|p| p.y);
            let box_ = [
                xs.clone().fold(f64::INFINITY, f64::min),
                ys.clone().fold(f64::INFINITY, f64::min),
                xs.fold(f64::NEG_INFINITY, f64::max),
                ys.fold(f64::NEG_INFINITY, f64::max),
            ];
            let on = |v: f64, a: f64, b: f64| v == a || v == b;
            corners
                .iter()
                .all(|p| on(p.x, box_[0], box_[2]) && on(p.y, box_[1], box_[3]))
                .then_some(box_)?
        }
        _ => return None,
    };
    let size = [x1 - x0, y1 - y0];
    if !(size.iter().all(|v| v.is_finite() && *v > 0.0)) {
        return None;
    }
    let center = [(x0 + x1) / 2.0, (y0 + y1) / 2.0];
    let [_, _, c, _, _, f] = view.transform.m;
    let scale = similar.scale;
    Some(Port {
        center,
        size,
        target: [(center[0] - c) / scale, (center[1] - f) / scale],
        height: size[1] / scale,
    })
}

/// 用紙のビューのうち VIEWPORT で書けるものと、用紙そのものを表すビューポート（69 = 1）。用紙の値は、図面の
/// 単位（1 単位が `mm` mm）で書く
fn viewports(sheet: &Sheet, mm: f64, owner: &str, next: &mut impl FnMut() -> String) -> String {
    let ports: Vec<Port> = sheet
        .views
        .iter()
        .filter_map(|view| port(view, sheet))
        .collect();
    if ports.is_empty() {
        return String::new();
    }
    let entity = |handle: String, id: usize, port: &Port| {
        let [cx, cy] = port.center.map(|v| v / mm);
        let [width, height] = port.size.map(|v| v / mm);
        let [tx, ty] = port.target;
        format!(
            "0\nVIEWPORT\n5\n{handle}\n330\n{owner}\n100\nAcDbEntity\n67\n1\n8\n0\n100\nAcDbViewport\n10\n{cx:?}\n20\n{cy:?}\n30\n0.0\n40\n{width:?}\n41\n{height:?}\n68\n1\n69\n{id}\n12\n{tx:?}\n22\n{ty:?}\n16\n0.0\n26\n0.0\n36\n1.0\n17\n0.0\n27\n0.0\n37\n0.0\n45\n{:?}\n90\n32768\n",
            port.height
        )
    };
    let mut out = String::new();
    // 用紙そのもののビューポートは、用紙の全体を用紙の単位で映す
    if let Some([width, height]) = sheet.size {
        let whole = Port {
            center: [width.0 / 2.0, height.0 / 2.0],
            size: [width.0, height.0],
            target: [width.0 / 2.0 / mm, height.0 / 2.0 / mm],
            height: height.0 / mm,
        };
        out.push_str(&entity(next(), 1, &whole));
    }
    for (index, port) in ports.iter().enumerate() {
        out.push_str(&entity(next(), index + 2, port));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::Dxf;
    use crate::db::{AcadDb, Data, Object, Section};
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref};
    use zumen_drawing::{
        Affine, Axes, LayerId, ModelPoint, ModelSpace, PaperLength, PaperPoint, SheetId, SpaceId,
        SymbolDefinition,
    };
    use zumen_sdk::Codec;

    fn space(id: u32) -> ModelSpace {
        ModelSpace {
            id: SpaceId(id),
            unit: Unit::Millimeter,
            axes: Axes::EastNorth,
            elements: vec![],
            order: vec![],
        }
    }

    fn key(name: &str) -> SymbolKey {
        SymbolKey {
            namespace: "dxf.block".into(),
            name: name.to_owned(),
        }
    }

    fn definition(index: u32, name: &str, anonymous: bool, external: bool) -> SymbolDefinition {
        SymbolDefinition {
            key: key(name),
            space: space(index + 1),
            base: Some(ModelPoint {
                space: SpaceId(index + 1),
                x: 1.5,
                y: -2.0,
            }),
            attributes: vec![],
            anonymous,
            external: external.then(|| "other.dwg".to_owned()),
            source: Ref::default(),
        }
    }

    fn view(m: [f64; 6], clip: Vec<PaperPoint>) -> View {
        View {
            space: SpaceId(0),
            sheet: SheetId(0),
            transform: Affine { m },
            clip,
            hidden_layers: vec![],
            source: Ref::default(),
        }
    }

    /// mm のモデル空間と、7 つのビューを持つ A3 横の用紙と、記号の定義 8 つ
    fn drawing() -> Drawing2D {
        let corner = |x, y| PaperPoint {
            sheet: SheetId(0),
            x,
            y,
        };
        let clip = vec![
            corner(10.0, 10.0),
            corner(110.0, 10.0),
            corner(110.0, 60.0),
            corner(10.0, 60.0),
        ];
        let sheet = Sheet {
            id: SheetId(0),
            name: None,
            size: Some([PaperLength(420.0), PaperLength(297.0)]),
            elements: vec![],
            views: vec![
                // 1/2 の尺度で、切り抜きがない（用紙の全体を映す）ビュー
                view([0.5, 0.0, 10.0, 0.0, 0.5, 20.0], vec![]),
                // 1/4 の尺度で、軸に沿った長方形で切り抜いたビュー
                view([0.25, 0.0, 0.0, 0.0, 0.25, 0.0], clip),
                // 回したビューは VIEWPORT で書かない
                view([0.0, -1.0, 0.0, 1.0, 0.0, 0.0], vec![]),
                // 鏡映したビュー（回転は 0）、表示しないレイヤを持つビュー、菱形と 5 点で切り抜いたビューも書かない
                view([0.5, 0.0, 0.0, 0.0, -0.5, 0.0], vec![]),
                View {
                    hidden_layers: vec![LayerId(1)],
                    ..view([0.5, 0.0, 0.0, 0.0, 0.5, 0.0], vec![])
                },
                view(
                    [0.25, 0.0, 0.0, 0.0, 0.25, 0.0],
                    vec![
                        corner(60.0, 10.0),
                        corner(110.0, 35.0),
                        corner(60.0, 60.0),
                        corner(10.0, 35.0),
                    ],
                ),
                view(
                    [0.25, 0.0, 0.0, 0.0, 0.25, 0.0],
                    vec![
                        corner(10.0, 10.0),
                        corner(110.0, 10.0),
                        corner(110.0, 60.0),
                        corner(60.0, 60.0),
                        corner(10.0, 60.0),
                    ],
                ),
            ],
            order: vec![],
            source: Ref::default(),
        };
        Drawing2D {
            spaces: vec![space(0)],
            sheets: vec![sheet],
            symbols: vec![
                definition(0, "部品A", false, false),
                definition(1, "A<B", false, false),
                definition(2, "a_b", false, false),
                definition(3, "*U5", true, false),
                definition(4, "\u{1F600}", false, false),
                definition(5, "外部", false, true),
                // 無名の定義は、名前が `*` で始まらなくても名前を付ける。`*` で始まる名前も同じ
                definition(6, "名前", true, false),
                definition(7, "*D1", false, false),
            ],
            ..Drawing2D::default()
        }
    }

    fn read(template: &NewDxfTemplate) -> AcadDb {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let db = {
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let file = Dxf::read(template.bytes(), &mut cx).unwrap();
            file.objects(&mut cx).unwrap()
        };
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        db
    }

    fn text(value: &str) -> Data {
        Data::Text(value.as_bytes().to_vec())
    }

    /// 表・部品・レイアウト・ビューポートを結ぶ参照が、ファイルの中の正しい種類を指し、ハンドルが 0x11 から
    /// 重ならずに続き、$HANDSEED がその次になる
    fn check_structure(db: &AcadDb) -> BTreeSet<u64> {
        let mut handles = BTreeSet::new();
        for object in db.objects() {
            let Some(handle) = object.handle() else {
                assert_eq!(object.kind, b"ENDTAB");
                continue;
            };
            assert!(handles.insert(handle), "同じハンドルがある: {handle:X}");
            let owner = object.owner().unwrap();
            if owner != 0 {
                assert!(db.get(owner).is_some(), "所有者がない: {owner:X}");
            }
            let subclass = |name: &[u8]| {
                object
                    .values
                    .iter()
                    .any(|value| value.code == 100 && value.data == Data::Text(name.to_vec()))
            };
            match object.kind.as_slice() {
                b"TABLE" => {
                    assert!(subclass(b"AcDbSymbolTable"));
                    let entries = db
                        .objects()
                        .iter()
                        .filter(|entry| {
                            entry.section == Section::Tables && entry.owner() == Some(handle)
                        })
                        .count();
                    assert_eq!(
                        object.first(70),
                        Some(&Data::Int(i64::try_from(entries).unwrap()))
                    );
                }
                b"LTYPE" | b"LAYER" | b"STYLE" | b"DIMSTYLE" | b"BLOCK_RECORD" => {
                    assert!(subclass(b"AcDbSymbolTableRecord"));
                    assert_eq!(db.get(owner).unwrap().kind, b"TABLE");
                }
                b"BLOCK" | b"ENDBLK" | b"VIEWPORT" => {
                    assert!(subclass(b"AcDbEntity"));
                    assert_eq!(db.get(owner).unwrap().kind, b"BLOCK_RECORD");
                }
                b"LAYOUT" => {
                    assert!(subclass(b"AcDbPlotSettings") && subclass(b"AcDbLayout"));
                    assert_eq!(db.get(owner).unwrap().kind, b"DICTIONARY");
                    let block = object
                        .values
                        .iter()
                        .rfind(|value| value.code == 330)
                        .unwrap();
                    let Data::Handle(block) = block.data else {
                        panic!("ブロックの参照がない")
                    };
                    let block = db.get(block).unwrap();
                    assert_eq!(block.kind, b"BLOCK_RECORD");
                    assert_eq!(block.first(340), Some(&Data::Handle(handle)));
                }
                b"DICTIONARY" => assert!(subclass(b"AcDbDictionary")),
                kind => panic!("ひな形にない種類: {}", String::from_utf8_lossy(kind)),
            }
        }
        let Some([seed]) = db.variable(b"$HANDSEED") else {
            panic!("次のハンドルがない")
        };
        let Data::Handle(seed) = seed.data else {
            panic!("次のハンドルの型が違う")
        };
        assert_eq!(handles, (0x11..seed).collect::<BTreeSet<_>>());
        handles
    }

    fn of_kind<'d>(db: &'d AcadDb, kind: &[u8]) -> Vec<&'d Object> {
        db.objects().iter().filter(|o| o.kind == kind).collect()
    }

    #[test]
    fn layouts_blocks_and_viewports_refer_to_each_other() {
        let template = NewDxfTemplate::new(&drawing());
        let db = read(&template);
        check_structure(&db);
        let header = |name: &[u8]| {
            db.variable(name)
                .unwrap()
                .iter()
                .map(|v| &v.data)
                .collect::<Vec<_>>()
        };
        assert_eq!(header(b"$ACADVER"), [&text("AC1018")]);
        assert_eq!(header(b"$DWGCODEPAGE"), [&text("ANSI_932")]);
        assert_eq!(header(b"$INSUNITS"), [&Data::Int(4)]);
        assert_eq!(
            header(b"$PLIMMAX"),
            [&Data::Real(420.0), &Data::Real(297.0)]
        );

        // 線種 3 つと、Continuous の画層 0。寸法スタイルは文字スタイル Standard を指す
        let line_types: Vec<_> = of_kind(&db, b"LTYPE").iter().map(|o| o.first(2)).collect();
        let expected = [text("ByBlock"), text("ByLayer"), text("Continuous")];
        assert_eq!(line_types, expected.iter().map(Some).collect::<Vec<_>>());
        let [layer] = of_kind(&db, b"LAYER")[..] else {
            panic!("画層が 1 つでない")
        };
        assert_eq!(
            (layer.first(2), layer.first(6)),
            (Some(&text("0")), Some(&text("Continuous")))
        );
        let [style] = of_kind(&db, b"STYLE")[..] else {
            panic!("文字スタイルが 1 つでない")
        };
        let [dimension] = of_kind(&db, b"DIMSTYLE")[..] else {
            panic!("寸法スタイルが 1 つでない")
        };
        assert_eq!(
            dimension.first(340),
            Some(&Data::Handle(style.handle().unwrap()))
        );
        // どちらも Standard。寸法スタイルのハンドルは 5 でなく 105 に書く
        assert_eq!(style.first(2), Some(&text("Standard")));
        assert_eq!(dimension.first(2), Some(&text("Standard")));
        assert!(matches!(dimension.first(105), Some(Data::Handle(_))));
        assert_eq!(dimension.first(5), None);

        // 部品の表は、モデル・用紙・外部参照を除く記号の定義の順。部品の名前は BLOCK の 2 と 3 にもある
        let records = of_kind(&db, b"BLOCK_RECORD");
        let names: Vec<_> = records.iter().map(|o| o.first(2).cloned()).collect();
        let sjis = |name: &str| Data::Text(encoding_rs::SHIFT_JIS.encode(name).0.into_owned());
        let expected = ["*Model_Space", "*Paper_Space"]
            .map(text)
            .into_iter()
            .chain([
                sjis("部品A"),
                text("A_B"),
                text("a_b_1"),
                text("zumen_3"),
                text("zumen_4"),
                text("zumen_6"),
                text("zumen_7"),
            ])
            .map(Some)
            .collect::<Vec<_>>();
        assert_eq!(names, expected);
        for (block, record) in of_kind(&db, b"BLOCK").iter().zip(&records) {
            assert_eq!(block.owner(), record.handle());
            assert_eq!(block.first(2), record.first(2));
            assert_eq!(block.first(3), record.first(2));
        }
        let base = of_kind(&db, b"BLOCK")[2];
        assert_eq!(
            (base.first(10), base.first(20)),
            (Some(&Data::Real(1.5)), Some(&Data::Real(-2.0)))
        );

        // 根の辞書の ACAD_LAYOUT が、Model と Layout1 を持つレイアウトの辞書を指す。モデルと用紙の部品は、
        // それぞれのレイアウトを指す
        let codes = |o: &Object, codes: &[i16]| {
            o.values
                .iter()
                .filter(|v| codes.contains(&v.code))
                .map(|v| v.data.clone())
                .collect::<Vec<_>>()
        };
        let handle = |o: &Object| Data::Handle(o.handle().unwrap());
        let [root, dictionary] = of_kind(&db, b"DICTIONARY")[..] else {
            panic!("辞書が 2 つでない")
        };
        let layouts = of_kind(&db, b"LAYOUT");
        assert_eq!(root.owner(), Some(0));
        assert_eq!(
            codes(root, &[3, 350]),
            [text("ACAD_LAYOUT"), handle(dictionary)]
        );
        assert_eq!(dictionary.owner(), root.handle());
        assert_eq!(
            codes(dictionary, &[3, 350]),
            [
                text("Model"),
                handle(layouts[0]),
                text("Layout1"),
                handle(layouts[1])
            ]
        );
        assert_eq!(codes(layouts[0], &[1]), [text(""), text("Model")]);
        assert_eq!(codes(layouts[1], &[1]), [text(""), text("Layout1")]);
        assert_eq!(records[0].first(340), Some(&handle(layouts[0])));
        assert_eq!(records[1].first(340), Some(&handle(layouts[1])));

        // ビューポートは用紙そのもの（69 = 1）と、書ける 2 つのビュー。ほかのビューは書かない
        let ports = of_kind(&db, b"VIEWPORT");
        let values = |o: &Object| {
            [69, 10, 20, 40, 41, 12, 22, 45].map(|code| match o.first(code) {
                Some(Data::Int(v)) => f64::from(i32::try_from(*v).unwrap()),
                Some(Data::Real(v)) => *v,
                other => panic!("{code} がない: {other:?}"),
            })
        };
        let got: Vec<_> = ports.iter().map(|o| values(o)).collect();
        assert_eq!(
            got,
            [
                [1.0, 210.0, 148.5, 420.0, 297.0, 210.0, 148.5, 297.0],
                [2.0, 210.0, 148.5, 420.0, 297.0, 400.0, 257.0, 594.0],
                [3.0, 60.0, 35.0, 100.0, 50.0, 240.0, 140.0, 200.0],
            ]
        );
        assert!(ports.iter().all(|o| o.owner() == records[1].handle()));
        let model = drawing();
        let sheet = &model.sheets[0];
        let writes: Vec<_> = sheet
            .views
            .iter()
            .map(|v| NewDxfTemplate::writes_viewport(v, sheet))
            .collect();
        assert_eq!(writes, [true, true, false, false, false, false, false]);
    }

    #[test]
    fn block_names_round_trip_in_ansi_932() {
        let drawing = drawing();
        let template = NewDxfTemplate::new(&drawing);
        let names: Vec<_> = drawing
            .symbols
            .iter()
            .map(|d| template.block(&d.key))
            .collect();
        assert_eq!(
            names,
            [
                Some("部品A"),
                Some("A_B"),
                Some("a_b_1"),
                Some("zumen_3"),
                Some("zumen_4"),
                None,
                Some("zumen_6"),
                Some("zumen_7"),
            ]
        );
        assert_eq!(template.block(&key("ない")), None);
        // 名前は ANSI_932 で書き、UTF-8 では書かない
        let encoded = encoding_rs::SHIFT_JIS.encode("部品A").0;
        let count = |needle: &[u8]| {
            template
                .bytes()
                .windows(needle.len())
                .filter(|window| *window == needle)
                .count()
        };
        assert_eq!(count(&encoded), 3);
        assert_eq!(count("部品A".as_bytes()), 0);
        assert_eq!(
            count("外部".as_bytes()) + count(&encoding_rs::SHIFT_JIS.encode("外部").0),
            0
        );
        // 同じ図面からは同じバイト列
        assert_eq!(NewDxfTemplate::new(&drawing).bytes(), template.bytes());
    }

    #[test]
    fn an_empty_drawing_has_only_the_model_and_paper_layouts() {
        let template = NewDxfTemplate::new(&Drawing2D::default());
        let db = read(&template);
        let handles = check_structure(&db);
        assert_eq!(handles.len(), 21);
        assert_eq!(db.variable(b"$INSUNITS").unwrap()[0].data, Data::Int(4));
        assert!(db.variable(b"$PLIMMAX").is_none());
        assert_eq!(of_kind(&db, b"BLOCK_RECORD").len(), 2);
        assert!(of_kind(&db, b"VIEWPORT").is_empty());
        assert_eq!(of_kind(&db, b"LAYOUT").len(), 2);
    }

    #[test]
    fn paper_values_are_written_in_the_unit_of_the_first_space() {
        // 単位、$INSUNITS、1 単位の mm。長さの単位でない空間は、mm と同じ値で書き、$INSUNITS は 0
        for (unit, code, mm) in [
            (Unit::Millimeter, 4, 1.0),
            (Unit::Centimeter, 5, 10.0),
            (Unit::Meter, 6, 1000.0),
            (Unit::Inch, 1, 25.4),
            (Unit::Foot, 2, 304.8),
            (Unit::Unknown, 0, 1.0),
        ] {
            let mut drawing = drawing();
            drawing.spaces[0].unit = unit;
            let db = read(&NewDxfTemplate::new(&drawing));
            check_structure(&db);
            let header = |name: &[u8]| {
                db.variable(name)
                    .unwrap()
                    .iter()
                    .map(|v| v.data.clone())
                    .collect::<Vec<_>>()
            };
            let values = |o: &Object, codes: &[i16]| {
                codes
                    .iter()
                    .map(|&code| o.first(code).cloned())
                    .collect::<Vec<_>>()
            };
            let reals = |values: &[f64]| {
                values
                    .iter()
                    .map(|&v| Some(Data::Real(v)))
                    .collect::<Vec<_>>()
            };
            assert_eq!(header(b"$INSUNITS"), [Data::Int(code)], "{unit:?}");
            assert_eq!(
                header(b"$PLIMMAX"),
                [Data::Real(420.0 / mm), Data::Real(297.0 / mm)],
                "{unit:?}"
            );
            // 用紙のレイアウトの印刷尺度と範囲。モデルのレイアウトは単位によらない
            let layouts = of_kind(&db, b"LAYOUT");
            assert_eq!(
                values(layouts[1], &[142, 147, 11, 21]),
                reals(&[mm, mm, 420.0 / mm, 297.0 / mm]),
                "{unit:?}"
            );
            assert_eq!(
                values(layouts[0], &[142, 147]),
                reals(&[1.0, 1.0]),
                "{unit:?}"
            );
            // ビューポートの用紙の中心と大きさは図面の単位で書く。ビューの実寸の中心と高さは単位によらない
            let ports = of_kind(&db, b"VIEWPORT");
            assert_eq!(ports.len(), 3, "{unit:?}");
            let paper = reals(&[210.0 / mm, 148.5 / mm, 420.0 / mm, 297.0 / mm]);
            assert_eq!(values(ports[0], &[10, 20, 40, 41]), paper, "{unit:?}");
            assert_eq!(
                values(ports[0], &[12, 22, 45]),
                reals(&[210.0 / mm, 148.5 / mm, 297.0 / mm]),
                "{unit:?}"
            );
            assert_eq!(values(ports[1], &[10, 20, 40, 41]), paper, "{unit:?}");
            assert_eq!(
                values(ports[1], &[12, 22, 45]),
                reals(&[400.0, 257.0, 594.0]),
                "{unit:?}"
            );
        }
    }

    #[test]
    fn a_sheet_without_a_size_has_no_limits_and_writes_only_clipped_views() {
        let mut drawing = drawing();
        drawing.sheets[0].size = None;
        let sheet = &drawing.sheets[0];
        // 切り抜きのないビューは用紙の全体を映すので、用紙の大きさがなければ書かない
        let writes: Vec<_> = sheet
            .views
            .iter()
            .map(|v| NewDxfTemplate::writes_viewport(v, sheet))
            .collect();
        assert_eq!(writes, [false, true, false, false, false, false, false]);
        let db = read(&NewDxfTemplate::new(&drawing));
        check_structure(&db);
        assert!(db.variable(b"$PLIMMAX").is_none());
        // 用紙そのもののビューポート（69 = 1）もない
        let ids: Vec<_> = of_kind(&db, b"VIEWPORT")
            .iter()
            .map(|o| o.first(69).cloned())
            .collect();
        assert_eq!(ids, [Some(Data::Int(2))]);
    }
}
