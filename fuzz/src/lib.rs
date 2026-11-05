//! 境界を通す fuzz の target（`open`・`font`・`profile`・`review`・`json`、#517・決定 0262）が使う部品。
//!
//! [`Calls`] が、`zumen` の [`Session`] に入力を渡して API の関数を呼び、応答の JSON と、取り出したバイナリの結果を
//! 呼んだ順に並べる。並べたものを、テストキットの `survives_calls` が 2 回で比べる。要求の `budget` には、テストキット
//! の境界の呼び出しの fuzz の上限（`fuzz_call_limits`）を書く。応答ごとに、次のことを確かめ、合わなければパニックに
//! して知らせる。
//!
//! - 応答が JSON として読め、成功したときだけ `result` を持つ。失敗した応答は、理由の診断を持つ（AGENTS.md の
//!   「黙って捨てない」）
//! - 応答の `blobs` の結果を、どれも取り出せ、記録した大きさと同じ。シーンの blob は、`zumen` のリーダーで読める
//!
//! 文書の中身に依らない入力（図面・プロファイル・色の表・線形・設計面・点群・SXF の図面）は、適合テストの組
//! （`bindings/conformance/cases`）の合成の入力を写した、ここの定数を使う。図面（[`DXF`]）だけは、写したものに図形と
//! 文字を足した。

use zumen::json::{self, Value, Writer};
use zumen::{CancelToken, Session, Status};
use zumen_base::{Kind, Limit, Limits};

/// 要求の予算のフィールドの名前。単位の名前（`decoded-bytes`）の `-` を `_` にしたもの（`schema/messages.json` の
/// `budget`）
fn budget_key(kind: Kind) -> String {
    kind.name().replace('-', "_")
}

/// 予算のフィールドを書く
fn write_budget(out: &mut Writer, limits: &Limits) {
    out.key("budget");
    out.begin_object();
    for kind in Kind::ALL {
        out.key(&budget_key(kind));
        match limits.get(kind) {
            Limit::Finite(limit) => out.uint(limit),
            Limit::Unlimited => out.string("unlimited"),
        }
    }
    out.end_object();
}

/// 1 回の呼び出しの応答
#[derive(Debug)]
pub struct Reply {
    /// 成功したか
    pub ok: bool,
    /// 応答の JSON
    pub value: Value,
    /// 取り出したバイナリの結果の、種類とバイト列。応答の `blobs` の順
    pub blobs: Vec<(String, Vec<u8>)>,
}

impl Reply {
    /// 結果の中を JSON Pointer で指した値。失敗したときは `None`
    pub fn result(&self, pointer: &str) -> Option<&Value> {
        self.value.pointer(&format!("/result{pointer}"))
    }

    /// 結果の中の文字列
    pub fn text(&self, pointer: &str) -> Option<&str> {
        match self.result(pointer)? {
            Value::String(text) => Some(text),
            _ => None,
        }
    }

    /// 結果の中の配列。なければ空
    pub fn items(&self, pointer: &str) -> &[Value] {
        match self.result(pointer) {
            Some(Value::Array(items)) => items,
            _ => &[],
        }
    }

    /// 取り出したバイナリの結果のうち、種類が `kind` の最初のもの
    pub fn blob(&self, kind: &str) -> Option<&[u8]> {
        self.blobs
            .iter()
            .find(|(found, _)| found == kind)
            .map(|(_, bytes)| bytes.as_slice())
    }
}

/// 値の中の文字列
pub fn text<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    match value.pointer(pointer)? {
        Value::String(text) => Some(text),
        _ => None,
    }
}

/// 値の中の 0 以上の整数
pub fn count(value: &Value, pointer: &str) -> Option<u64> {
    json::uint(value.pointer(pointer)?, 0, json::MAX_SAFE_INTEGER as u64).ok()
}

/// 1 つのセッションの呼び出しと、その応答の並び
pub struct Calls {
    session: Session,
    limits: Limits,
    replies: Vec<Vec<u8>>,
}

impl Calls {
    /// 既定の設定でセッションを作る。`limits` を、どの要求の予算にもする
    pub fn new(limits: &Limits) -> Self {
        let session = match Session::open() {
            Ok(session) => session,
            Err(error) => panic!("既定の設定でセッションを作れない: {error}"),
        };
        Self {
            session,
            limits: *limits,
            replies: Vec::new(),
        }
    }

    /// 入力を名前で足す。同じ名前で足すと、後ろに足す
    pub fn push(&mut self, name: &str, data: &[u8]) {
        if let Err(error) = self.session.push(name, data) {
            panic!("入力 {name} を足せない: {error}");
        }
    }

    /// op を呼ぶ。`fields` は、`v`・`op`・`budget` のほかのフィールドを書く
    pub fn call(&mut self, op: &str, fields: impl FnOnce(&mut Writer)) -> Reply {
        let mut out = Writer::new();
        out.begin_object();
        out.key("v");
        out.uint(1);
        out.key("op");
        out.string(op);
        fields(&mut out);
        write_budget(&mut out, &self.limits);
        out.end_object();
        self.call_bytes(&out.into_bytes())
    }

    /// 受け取った要求を呼ぶ。オブジェクトなら、`budget` を境界の呼び出しの fuzz の上限に替え、時計で止める
    /// `time_limit_ms` を除く（止まる所が実行ごとに変わるので、2 回で比べられない）。オブジェクトでなければ、そのまま
    /// 渡す
    pub fn call_value(&mut self, request: &Value) -> Reply {
        let mut out = Writer::new();
        match request {
            Value::Object(object) => {
                out.begin_object();
                for (key, value) in object.iter() {
                    if key != "budget" && key != "time_limit_ms" {
                        out.key(key);
                        out.value(value);
                    }
                }
                write_budget(&mut out, &self.limits);
                out.end_object();
            }
            other => out.value(other),
        }
        self.call_bytes(&out.into_bytes())
    }

    /// 要求の JSON のバイト列を、そのまま渡す
    pub fn call_bytes(&mut self, request: &[u8]) -> Reply {
        let (ok, response) = match self.session.call_bytes(request, &CancelToken::new()) {
            Ok(response) => (true, response),
            Err(error) if error.status == Status::Failed => match error.data {
                Some(data) => (false, data),
                None => panic!("失敗した呼び出しに応答がない"),
            },
            Err(error) => panic!("呼び出しが {} で終わった", error.status.name()),
        };
        let value = match json::parse(&response) {
            Ok(value) => value,
            Err(error) => panic!("応答を JSON として読めない（位置 {}）", error.offset),
        };
        assert_eq!(
            value.pointer("/result").is_some(),
            ok,
            "成功したときだけ result を持つ"
        );
        if !ok {
            assert!(
                matches!(value.pointer("/diagnostics"), Some(Value::Array(items)) if !items.is_empty()),
                "失敗した応答に、理由の診断がない"
            );
        }
        self.replies.push(response);
        let mut blobs = Vec::new();
        if let Some(Value::Array(items)) = value.pointer("/blobs") {
            for item in items {
                let (Some(id), Some(kind), Some(size)) = (
                    count(item, "/id"),
                    text(item, "/kind"),
                    count(item, "/size"),
                ) else {
                    panic!("応答の blob に id・kind・size がない");
                };
                let Ok(id) = u32::try_from(id) else {
                    panic!("応答の blob の id が u32 に収まらない");
                };
                let bytes = match self.session.take(id) {
                    Ok(bytes) => bytes,
                    Err(error) => panic!("応答の blob {id} を取り出せない: {error}"),
                };
                assert_eq!(
                    bytes.len() as u64,
                    size,
                    "blob {id} の大きさが、応答の size と違う"
                );
                if kind == "scene"
                    && let Err(error) = zumen::scene::read_scene(&bytes)
                {
                    panic!("コアが作ったシーンの blob {id} を読めない: {error}");
                }
                self.replies.push(bytes.clone());
                blobs.push((kind.to_string(), bytes));
            }
        }
        Reply { ok, value, blobs }
    }

    /// 応答の並び。応答の JSON と、取り出したバイナリの結果を、呼んだ順に並べる
    pub fn finish(self) -> Vec<Vec<u8>> {
        self.replies
    }
}

/// 文字列のフィールドを書く
pub fn field(out: &mut Writer, key: &str, value: &str) {
    out.key(key);
    out.string(value);
}

/// 文字列の配列のフィールドを書く
pub fn list(out: &mut Writer, key: &str, values: &[&str]) {
    out.key(key);
    out.begin_array();
    for value in values {
        out.string(value);
    }
    out.end_array();
}

/// プロファイルの識別（`profile_ref`）のフィールドを書く
pub fn profile(out: &mut Writer, id: &str, edition: &str) {
    out.key("profile");
    out.begin_object();
    field(out, "id", id);
    field(out, "edition", edition);
    out.end_object();
}

/// 図面。線・円・円弧・点と、ASCII と日本語の文字を持つ。適合テストの組の `check.json` の図面に、図形と文字を足した
/// もの。日本語の文字を UTF-8 で書けるように、版を R2013（AC1027）にする
pub const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1027\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n5\nA1\n8\nC-STR\n62\n1\n6\nCONTINUOUS\n370\n35\n10\n0.0\n20\n0.0\n11\n10.0\n21\n0.0\n\
0\nLINE\n5\nA2\n8\nBAD\n62\n3\n6\nDASHED\n370\n30\n10\n0.0\n20\n1.0\n11\n10.0\n21\n1.0\n\
0\nTEXT\n5\nA3\n8\nC-STR\n62\n1\n10\n0.0\n20\n5.0\n40\n2.5\n1\nABC\n\
0\nTEXT\n5\nA4\n8\nC-STR\n10\n0.0\n20\n9.0\n40\n3.5\n1\n平面図 Ag-1/2\n\
0\nCIRCLE\n5\nA5\n8\n0\n10\n20.0\n20\n5.0\n40\n3.0\n\
0\nARC\n5\nA6\n8\n0\n10\n30.0\n20\n5.0\n40\n4.0\n50\n30.0\n51\n120.0\n\
0\nPOINT\n5\nA7\n8\n0\n10\n5.0\n20\n6.0\n\
0\nENDSEC\n0\nEOF\n";

/// 照合のプロファイル。適合テストの組の `check.json` の `base.json`
pub const BASE_PROFILE: &str = r##"{"format":"0.1","id":"base","edition":"2025-12","revision":1,"vocabularies":{"who":{"values":[{"code":"C"},{"code":"D"}]},"what":{"values":[{"code":"STR"}]},"color":{"values":[{"name":"赤","rgb":"#ff0000","sxf":2}]},"line-type":{"values":[{"name":"実線","sxf":"continuous","acad":"CONTINUOUS"}]},"width":{"values":[0.13,0.18,0.25,0.35,0.5]},"height":{"values":[2.5,3.5]},"letters":{"values":["41"]}},"rules":[{"id":"layer-name","check":"layer-name","level":"error","parts":["who","what"],"user-part":true,"separator":"-","max-width":256},{"id":"color","check":"color","level":"warning","vocabulary":"color"},{"id":"line-type","check":"line-type","level":"warning","vocabulary":"line-type"},{"id":"line-width","check":"line-width","level":"warning","vocabulary":"width"},{"id":"text-height","check":"text-height","level":"warning","vocabulary":"height"},{"id":"characters","check":"characters","level":"warning","vocabulary":"letters"}]}"##;

/// [`BASE_PROFILE`] を引き継ぐプロファイル。適合テストの組の `check.json` の `office.json`
pub const OFFICE_PROFILE: &str = r#"{"format":"0.1","id":"office","edition":"1","revision":2,"extends":{"id":"base","edition":"2025-12"},"rules":[{"id":"color","check":"color","level":"error","vocabulary":"color"}]}"#;

/// 色の表。適合テストの組の `check.json` の `palette.json`
pub const PALETTE: &str = r##"{"values":[{"name":"赤","rgb":"#ff0000","aci":1}]}"##;

/// 線形と地盤の TIN。適合テストの組の `civil_drawing.json` の `road.xml`
pub const ROAD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2">
<CoordinateSystem horizontalDatum="JGD2011" verticalDatum="T.P" horizontalCoordinateSystemName="9(X,Y)"/>
<Alignments><Alignment name="本線" length="100" staStart="0"><CoordGeom><Line><Start>0 0</Start><End>100 0</End></Line></CoordGeom><Profile><ProfAlign><PVI>0 10</PVI><PVI>100 12</PVI></ProfAlign></Profile></Alignment></Alignments>
<Surfaces><Surface name="地盤" desc="ExistingGround"><Definition surfType="TIN"><Pnts><P id="1">0 0 10</P><P id="2">0 10 11</P><P id="3">10 10 12</P></Pnts><Faces><F>1 2 3</F></Faces></Definition></Surface></Surfaces>
</LandXML>
"#;

/// 設計面の TIN。適合テストの組の `deviation.json` の `plan.xml`
pub const PLAN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2">
<CoordinateSystem horizontalDatum="JGD2011" verticalDatum="T.P" horizontalCoordinateSystemName="9(X,Y)"/>
<Surfaces><Surface name="計画面" desc="FinishedGrade"><Definition surfType="TIN"><Pnts><P id="1">0 0 10</P><P id="2">0 20 10</P><P id="3">20 20 10</P><P id="4">20 0 10</P></Pnts><Faces><F>1 2 3</F><F>1 3 4</F></Faces></Definition></Surface></Surfaces>
</LandXML>
"#;

/// [`PLAN`] の TIN の名前
pub const PLAN_SURFACE: &str = "計画面";

/// 点群。適合テストの組の `deviation.json` の `a.xyz`
pub const CLOUD: &str = "5 5 10.02\n15 15 9.98\n5 15 10.2\n30 30 10\n";

/// 属性ファイル（SAF）を結ぶ SXF の図面。属性ファイル用の属性付加機構（`$$ATRF$$`）の記号を持つ。適合テストの組の
/// `open_sxf_saf.json` の `D0PL001Z.SFC`
pub const SFC: &str = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\n\
FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n\
/*SXF3\n#10 = layer_feature('L','1')\nSXF3*/\n\
/*SXF3\n#20 = line_feature('1','1','1','1','0','0','1','0')\nSXF3*/\n\
/*SXF3\n#30 = sfig_org_feature('$$ATRF$$10$$D0PL001Z.SAF','3')\nSXF3*/\n\
/*SXF3\n#40 = line_feature('1','1','1','1','0','0','2','2')\nSXF3*/\n\
/*SXF3\n#50 = sfig_org_feature('$$ATRF$$30','3')\nSXF3*/\n\
/*SXF3\n#60 = sfig_locate_feature('1','$$ATRF$$10$$D0PL001Z.SAF','0','0','0','1','1')\nSXF3*/\n\
/*SXF3\n#70 = sfig_locate_feature('1','$$ATRF$$30','0','0','0','1','1')\nSXF3*/\n\
/*SXF3\n#80 = drawing_sheet_feature('S','3','1','0','0')\nSXF3*/\n\
ENDSEC;\nEND-ISO-10303-21;\n";

/// 固定の入力を開く。`format` は、名指す形式
fn open_fixture(calls: &mut Calls, input: &str, format: Option<&str>) {
    let opened = calls.call("open", |out| {
        field(out, "input", input);
        if let Some(format) = format {
            field(out, "format", format);
        }
    });
    assert!(
        opened.ok,
        "固定の入力 {input} を開けない: {:?}",
        opened.value
    );
}

/// 固定の図面（[`DXF`]）を `a.dxf` の名前で足して開く
pub fn open_drawing(calls: &mut Calls) {
    calls.push("a.dxf", DXF.as_bytes());
    open_fixture(calls, "a.dxf", None);
}

/// 固定の照合のプロファイル（[`BASE_PROFILE`]・[`OFFICE_PROFILE`]）と色の表（[`PALETTE`]）を、`base.json`・
/// `office.json`・`palette.json` の名前で足す
pub fn push_profiles(calls: &mut Calls) {
    calls.push("base.json", BASE_PROFILE.as_bytes());
    calls.push("office.json", OFFICE_PROFILE.as_bytes());
    calls.push("palette.json", PALETTE.as_bytes());
}

/// 固定の入力をすべて足して開く。[`open_drawing`] と [`push_profiles`] の入力と、線形（[`ROAD`]、`road.xml`）・
/// 設計面（[`PLAN`]、`plan.xml`）・点群（[`CLOUD`]、`a.xyz`）
pub fn open_fixtures(calls: &mut Calls) {
    open_drawing(calls);
    push_profiles(calls);
    for (name, data) in [("road.xml", ROAD), ("plan.xml", PLAN), ("a.xyz", CLOUD)] {
        calls.push(name, data.as_bytes());
    }
    open_fixture(calls, "road.xml", None);
    open_fixture(calls, "plan.xml", None);
    open_fixture(calls, "a.xyz", Some("xyz"));
}
