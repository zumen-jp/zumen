//! 新しい IFC のひな形（#395）。
//!
//! バージョン・長さの単位・ヘッダーと所有履歴の値から、プロジェクト・単位・3 次元の `Model` の表現の文脈と、その `Body` だけの
//! 文書を書く。空間・要素は、[`super::apply`] と同じ書き手で足す。コアは時計を読まず、ヘッダーの時刻や所有履歴の
//! 作った日時は呼び出し元から受け取る。渡されないヘッダーの値は空にする。
//!
//! 実体の属性の並びは、buildingSMART が公開する各バージョンの文書に従う（IFC2X3 TC1・IFC4 ADD2 TC1・
//! IFC4.3 ADD2）。どのバージョンでも同じ位置の属性だけを書き、違うものは下に記す。
//!
//! - `IfcProject`（8 RepresentationContexts・9 UnitsInContext）。IFC2X3 は 2 OwnerHistory・8・9 が必須で、
//!   8 は 1 つ以上。IFC4 から 2・8・9 は省ける
//! - `IfcOwnerHistory`（1 OwningUser・2 OwningApplication・3 State・4 ChangeAction・5 LastModifiedDate・
//!   6 LastModifyingUser・7 LastModifyingApplication・8 CreationDate）。IFC2X3 は 4 ChangeAction が必須なので
//!   `.ADDED.` を書く。IFC4 からは 4 を省け、規則 CorrectChangeAction が 5 のないときの 4 を `NOTDEFINED` か
//!   `NOCHANGE` に限るので書かない
//! - `IfcPerson`（2 FamilyName）と `IfcOrganization`（2 Name）。1 は IFC2X3 で Id、IFC4 から Identification
//! - `IfcConversionBasedUnit` のインチとフィートの名前は、IFC4 の文書の表の `inch`・`foot` にする
//!
//! 角度の単位はラジアンにする。面積・体積などの単位は書かず、書き手が足す値の単位は長さの単位から導く。

use zumen_base::{Diagnostic, Kind};
use zumen_building::{GlobalId, Hash};
use zumen_sdk::{Cx, Error};

use super::author::{null, raw, real};
use super::{Item, Stmt, text, write_stmt};
use crate::building::Version;
use crate::stopped;

/// 作る文書の長さの単位
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LengthUnit {
    /// ミリメートル
    Millimeter,
    /// センチメートル
    Centimeter,
    /// メートル
    Meter,
    /// インチ（0.0254 m）
    Inch,
    /// フィート（0.3048 m）
    Foot,
}

/// ヘッダーの値（ISO 10303-21 の `FILE_DESCRIPTION` と `FILE_NAME`）。渡されない値は空の文字列で書く
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// `FILE_DESCRIPTION` に `ViewDefinition [<値>]` として書く MVD の名前。`None` なら書かない
    pub view_definition: Option<String>,
    /// `FILE_NAME` の 1 name
    pub name: String,
    /// `FILE_NAME` の 2 time_stamp。呼び出し元が書いた時刻
    pub time_stamp: String,
    /// `FILE_NAME` の 3 author
    pub author: Vec<String>,
    /// `FILE_NAME` の 4 organization
    pub organization: Vec<String>,
    /// `FILE_NAME` の 5 preprocessor_version
    pub preprocessor_version: String,
    /// `FILE_NAME` の 6 originating_system
    pub originating_system: String,
    /// `FILE_NAME` の 7 authorization
    pub authorization: String,
}

/// 所有履歴（`IfcOwnerHistory`）の値
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerHistory {
    /// 所有する人の `IfcPerson` の FamilyName
    pub person: String,
    /// 所有する組織の `IfcOrganization` の Name
    pub organization: String,
    /// 作ったソフトの開発元の `IfcOrganization` の Name
    pub developer: String,
    /// 作ったソフトの名前（`IfcApplication` の ApplicationFullName）
    pub application: String,
    /// 作ったソフトのバージョン（`IfcApplication` の Version）
    pub version: String,
    /// 作ったソフトの識別子（`IfcApplication` の ApplicationIdentifier）
    pub identifier: String,
    /// 作った日時。1970-01-01T00:00:00Z からの秒（`IfcTimeStamp`）
    pub created: i64,
}

/// 新しい IFC のひな形
#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    /// 書くバージョン
    pub version: Version,
    /// 長さの単位
    pub unit: LengthUnit,
    /// `IfcProject` の GlobalId。[`derive_global_id`] で名前空間から導くか、元の文書の値を渡す
    pub project: GlobalId,
    /// `IfcProject` の 3 Name
    pub name: Option<String>,
    /// ヘッダーの値
    pub header: Header,
    /// 所有履歴の値。IFC2X3 では必須
    pub owner: Option<OwnerHistory>,
}

/// 名前空間と安定したキーから導く GlobalId。同じ名前空間とキーなら、いつも同じ値になる
pub fn derive_global_id(namespace: &[u8], key: &[u8]) -> GlobalId {
    let mut input = b"zumen-ifc global-id 1\0".to_vec();
    input.extend_from_slice(&(namespace.len() as u64).to_le_bytes());
    input.extend_from_slice(namespace);
    input.extend_from_slice(key);
    let Hash(bytes) = Hash::document(&input);
    GlobalId::from_uuid(bytes)
}

/// ひな形を書く。IFC2X3 で所有履歴の値がなければ、診断して失敗する
pub fn create(template: &Template, cx: &mut Cx) -> Result<Vec<u8>, Error> {
    let schema = template.version.file_schema();
    if template.version == Version::Ifc2x3 && template.owner.is_none() {
        cx.report(
            Diagnostic::new(zumen_base::codes::IFC_OWNER_HISTORY_MISSING).with("schema", schema),
        );
        return Err(Error::Invalid);
    }
    let mut data = Data::default();
    let owner = template
        .owner
        .as_ref()
        .map(|owner| data.owner_history(template.version, owner));
    let units = data.units(template.unit);
    let context = data.model_context();
    let project = Stmt {
        name: b"IFCPROJECT".to_vec(),
        args: vec![
            text(&template.project.to_string()),
            owner.map_or_else(null, Item::Ref),
            template.name.as_deref().map_or_else(null, text),
            null(),
            null(),
            null(),
            null(),
            Item::List(vec![Item::Ref(context)]),
            Item::Ref(units),
        ],
    };
    // プロジェクトを最初のインスタンスにする
    data.stmts.insert(0, project);

    let header = &template.header;
    let strings = header
        .author
        .iter()
        .chain(&header.organization)
        .map(|s| s.len() as u64)
        .sum::<u64>()
        .saturating_add(header.name.len() as u64)
        .saturating_add(header.time_stamp.len() as u64);
    cx.check(
        Kind::Work,
        (data.stmts.len() as u64).saturating_add(strings / 1024),
    )
    .map_err(|stop| stopped(cx, stop, None))?;

    let mut out = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(".to_vec();
    let description = match &header.view_definition {
        Some(view) => text(&format!("ViewDefinition [{view}]")),
        None => text(""),
    };
    write_items(&[Item::List(vec![description]), text("2;1")], &mut out);
    out.extend_from_slice(b");\nFILE_NAME(");
    write_items(
        &[
            text(&header.name),
            text(&header.time_stamp),
            list(&header.author),
            list(&header.organization),
            text(&header.preprocessor_version),
            text(&header.originating_system),
            text(&header.authorization),
        ],
        &mut out,
    );
    out.extend_from_slice(b");\nFILE_SCHEMA((");
    write_items(&[text(schema)], &mut out);
    out.extend_from_slice(b"));\nENDSEC;\nDATA;\n");
    for (at, stmt) in data.stmts.iter().enumerate() {
        write_stmt(at as u64 + 1, stmt, &mut out);
        out.push(b'\n');
    }
    out.extend_from_slice(b"ENDSEC;\nEND-ISO-10303-21;\n");
    Ok(out)
}

/// 文字列の並び。空なら空の文字列 1 つにする（`FILE_NAME` の並びは 1 つ以上）
fn list(values: &[String]) -> Item {
    if values.is_empty() {
        return Item::List(vec![text("")]);
    }
    Item::List(values.iter().map(|value| text(value)).collect())
}

fn write_items(items: &[Item], out: &mut Vec<u8>) {
    let stmt = Stmt {
        name: Vec::new(),
        args: items.to_vec(),
    };
    // `#0=(…);` の中だけを使う
    let mut written = Vec::new();
    write_stmt(0, &stmt, &mut written);
    let inner = written
        .strip_prefix(b"#0=(")
        .and_then(|rest| rest.strip_suffix(b");"))
        .unwrap_or_default();
    out.extend_from_slice(inner);
}

/// プロジェクトの後に書くインスタンス。番号は、プロジェクトを 1 として並びの順に振る
#[derive(Default)]
struct Data {
    stmts: Vec<Stmt>,
}

impl Data {
    /// 足したインスタンスの番号。プロジェクトが前に入るので 2 から
    fn add(&mut self, name: &[u8], args: Vec<Item>) -> u64 {
        self.stmts.push(Stmt {
            name: name.to_vec(),
            args,
        });
        self.stmts.len() as u64 + 1
    }

    fn organization(&mut self, name: &str) -> u64 {
        self.add(
            b"IFCORGANIZATION",
            vec![null(), text(name), null(), null(), null()],
        )
    }

    fn owner_history(&mut self, version: Version, owner: &OwnerHistory) -> u64 {
        let person = self.add(
            b"IFCPERSON",
            vec![
                null(),
                text(&owner.person),
                null(),
                null(),
                null(),
                null(),
                null(),
                null(),
            ],
        );
        let organization = self.organization(&owner.organization);
        let user = self.add(
            b"IFCPERSONANDORGANIZATION",
            vec![Item::Ref(person), Item::Ref(organization), null()],
        );
        let developer = self.organization(&owner.developer);
        let application = self.add(
            b"IFCAPPLICATION",
            vec![
                Item::Ref(developer),
                text(&owner.version),
                text(&owner.application),
                text(&owner.identifier),
            ],
        );
        let change = match version {
            Version::Ifc2x3 => raw(b".ADDED."),
            Version::Ifc4 | Version::Ifc4x3 => null(),
        };
        self.add(
            b"IFCOWNERHISTORY",
            vec![
                Item::Ref(user),
                Item::Ref(application),
                null(),
                change,
                null(),
                null(),
                null(),
                raw(owner.created.to_string().as_bytes()),
            ],
        )
    }

    fn units(&mut self, unit: LengthUnit) -> u64 {
        let si = |prefix: &[u8]| {
            vec![
                raw(b"*"),
                raw(b".LENGTHUNIT."),
                raw(prefix),
                raw(b".METRE."),
            ]
        };
        let length = match unit {
            LengthUnit::Millimeter => self.add(b"IFCSIUNIT", si(b".MILLI.")),
            LengthUnit::Centimeter => self.add(b"IFCSIUNIT", si(b".CENTI.")),
            LengthUnit::Meter => self.add(b"IFCSIUNIT", si(b"$")),
            LengthUnit::Inch => self.conversion("inch", b"0.0254"),
            LengthUnit::Foot => self.conversion("foot", b"0.3048"),
        };
        let angle = self.add(
            b"IFCSIUNIT",
            vec![
                raw(b"*"),
                raw(b".PLANEANGLEUNIT."),
                null(),
                raw(b".RADIAN."),
            ],
        );
        self.add(
            b"IFCUNITASSIGNMENT",
            vec![Item::List(vec![Item::Ref(length), Item::Ref(angle)])],
        )
    }

    /// メートルからの換算で決める長さの単位
    fn conversion(&mut self, name: &str, metres: &[u8]) -> u64 {
        let exponents = self.add(
            b"IFCDIMENSIONALEXPONENTS",
            [1, 0, 0, 0, 0, 0, 0]
                .iter()
                .map(|n: &i32| raw(n.to_string().as_bytes()))
                .collect(),
        );
        let metre = self.add(
            b"IFCSIUNIT",
            vec![raw(b"*"), raw(b".LENGTHUNIT."), null(), raw(b".METRE.")],
        );
        let mut measure = b"IFCLENGTHMEASURE(".to_vec();
        measure.extend_from_slice(metres);
        measure.push(b')');
        let factor = self.add(b"IFCMEASUREWITHUNIT", vec![raw(&measure), Item::Ref(metre)]);
        self.add(
            b"IFCCONVERSIONBASEDUNIT",
            vec![
                Item::Ref(exponents),
                raw(b".LENGTHUNIT."),
                text(name),
                Item::Ref(factor),
            ],
        )
    }

    /// 3 次元の `Model` の表現の文脈と、その `Body` の下位の文脈。書き手が足すものと同じ値にする。
    /// ifc-gherkin-rules の GEM052（業界の慣行）は、文脈ごとに下位の文脈を 1 つ以上求める
    fn model_context(&mut self) -> u64 {
        let context = self.context();
        self.add(
            b"IFCGEOMETRICREPRESENTATIONSUBCONTEXT",
            vec![
                text("Body"),
                text("Model"),
                raw(b"*"),
                raw(b"*"),
                raw(b"*"),
                raw(b"*"),
                Item::Ref(context),
                null(),
                raw(b".MODEL_VIEW."),
                null(),
            ],
        );
        context
    }

    fn context(&mut self) -> u64 {
        let origin = self.add(
            b"IFCCARTESIANPOINT",
            vec![Item::List(vec![raw(b"0."), raw(b"0."), raw(b"0.")])],
        );
        let system = self.add(
            b"IFCAXIS2PLACEMENT3D",
            vec![Item::Ref(origin), null(), null()],
        );
        self.add(
            b"IFCGEOMETRICREPRESENTATIONCONTEXT",
            vec![
                null(),
                text("Model"),
                raw(b"3"),
                real(1e-5).unwrap_or_else(|| raw(b"1.E-05")),
                Item::Ref(system),
                null(),
            ],
        )
    }
}
