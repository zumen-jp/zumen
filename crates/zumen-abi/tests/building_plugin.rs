//! 建物の形式のプラグインを窓口に登録し、建物の op で使う（#402）。登録はプロセスで 1 度だけなので、この試験は
//! ほかの試験と別の実行ファイルにする。

#![cfg(feature = "bim")]

use zumen_abi1::{Plugin, Session, Status};
use zumen_base::{CancelToken, Cx, Kind};
use zumen_building::{Building, Class, Document, Header, Identity};
use zumen_sdk::{Codec, Error, FileModel, Lift, RecordInfo, Registration, Rights, Signature};

static PLUGINS: &[Plugin] = &[Plugin::building::<Rooms>(Registration::of::<Rooms>(
    Rights::Public,
    "zumen_rooms",
    false,
    1,
))];

const HEADER: &[u8] = b"ZUMEN-ROOMS 1";

/// 試験のための形式。1 行目のあとの 1 行ずつが、室の名前
struct Rooms;

struct RoomsFile {
    names: Vec<String>,
}

impl Codec for Rooms {
    type File = RoomsFile;

    const FORMAT: &'static str = "zumen_rooms";

    const SIGNATURES: &'static [Signature] = &[Signature::Lines { lines: &[HEADER] }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<RoomsFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let text = core::str::from_utf8(src).map_err(|_| Error::Invalid)?;
        let names = text
            .lines()
            .skip(1)
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_owned())
            .collect();
        Ok(RoomsFile { names })
    }

    fn write(_: &RoomsFile, _: &mut Vec<u8>, _: &mut Cx) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}

impl FileModel for RoomsFile {
    fn record_count(&self) -> usize {
        self.names.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        self.names.get(index).map(|_| RecordInfo {
            kind: b"ROOM",
            bytes: None,
        })
    }
}

impl Lift<Building> for RoomsFile {
    fn lift(&self, cx: &mut Cx) -> Result<Building, Error> {
        let mut building = Building::new(Document {
            header: Header {
                schemas: vec!["ZUMEN-ROOMS 1".to_owned()],
                ..Header::default()
            },
            ..Document::default()
        });
        for (index, name) in self.names.iter().enumerate() {
            cx.check(Kind::Elements, 1)?;
            let Some(element) = building.push(None, index as u64 + 1, cx) else {
                continue;
            };
            building.components.class.insert(
                element,
                Class {
                    name: "IfcSpace".to_owned(),
                    predefined: None,
                    user_defined: None,
                    original: "ROOM".to_owned(),
                },
            );
            building.components.identity.insert(
                element,
                Identity {
                    name: Some(name.clone()),
                    ..Identity::default()
                },
            );
        }
        Ok(building)
    }
}

#[cfg(test)]
fn call(session: &mut Session, request: &str) -> (Status, String) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, String::from_utf8(reply.response).unwrap())
}

#[test]
fn opens_a_building_plugin_document_with_the_building_ops() {
    assert!(zumen_abi1::install(PLUGINS));
    let mut session = Session::new(b"").unwrap();
    session
        .push(b"a.txt", b"ZUMEN-ROOMS 1\n\xe5\xae\xa4 1\nHall\n")
        .unwrap();
    let (status, open) = call(&mut session, r#"{"v": 1, "op": "open", "input": "a.txt"}"#);
    assert_eq!(status, Status::Ok, "{open}");
    assert!(open.contains(r#""format":"zumen_rooms""#), "{open}");
    let (status, summary) = call(
        &mut session,
        r#"{"v": 1, "op": "building", "doc": "a.txt"}"#,
    );
    assert_eq!(status, Status::Ok, "{summary}");
    assert!(summary.contains(r#""schema":"ZUMEN-ROOMS 1""#), "{summary}");
    assert!(summary.contains(r#""elements":2"#), "{summary}");
    let (status, elements) = call(
        &mut session,
        r#"{"v": 1, "op": "building_elements", "doc": "a.txt", "components": ["class", "identity"]}"#,
    );
    assert_eq!(status, Status::Ok, "{elements}");
    assert!(elements.contains(r#""id":"i1""#), "{elements}");
    assert!(elements.contains("Hall"), "{elements}");
    let (_, capabilities) = call(&mut session, r#"{"v": 1, "op": "capabilities"}"#);
    assert!(
        capabilities.contains(r#""id":"zumen_rooms","family":"zumen_rooms""#),
        "{capabilities}"
    );
    // プラグインの文書は書き戻せない
    let (status, apply) = call(
        &mut session,
        r#"{"v": 1, "op": "building_apply", "doc": "a.txt", "edits": [{"kind": "attribute", "id": "i1", "hash": "v1:00000000000000000000000000000000", "attribute": "name", "value": "x"}]}"#,
    );
    assert_eq!(status, Status::Failed, "{apply}");
    assert!(apply.contains("call.document.unsupported"), "{apply}");
}
