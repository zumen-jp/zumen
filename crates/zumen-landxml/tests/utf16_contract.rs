//! UTF-16 の原本・解析結果・領域モデル・予算を、同じ probe で比較する（#468）。

use std::ops::Range;

use zumen_base::{Budget, CancelToken, Diagnostics, Kind, Limit, Limits, Stop, Value, codes};
use zumen_civil::{CantKind, CivilModel};
use zumen_landxml::LandXml;
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift};
use zumen_xml::{Document, Name, Namespace, Node};

const XML: &str = "<!DOCTYPE p:根 [<!ELEMENT p:根 ANY>]><p:根 xmlns:p='urn:root' xmlns='urn:default' p:属性='甲&#x1F600;\r\n乙' xml:lang='ja' data='a&#xD;b'>前\r\n日本😀&#38;<子 属性='値'/>中<![CDATA[生&😀\r\n文]]><?pi 内容?><!-- 注記 -->尾</p:根>";
const CANT: &str = "<Superelevation staStart='1020' staEnd='1045'><FullSuperelev>-6.25</FullSuperelev><AdverseSE>non-adverse</AdverseSE></Superelevation>";

#[derive(Clone, Copy, Debug)]
enum Wire {
    Utf8,
    ShiftJis,
    Little,
    Big,
}

struct Input {
    name: String,
    src: Vec<u8>,
    wire: Wire,
    expected: Option<Error>,
    landxml: bool,
    external: bool,
}

fn encoded(text: &str, wire: Wire, bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    match wire {
        Wire::Utf8 | Wire::ShiftJis => bytes.extend_from_slice(text.as_bytes()),
        Wire::Little | Wire::Big => {
            if bom {
                bytes.extend_from_slice(if matches!(wire, Wire::Little) {
                    &[0xff, 0xfe]
                } else {
                    &[0xfe, 0xff]
                });
            }
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&if matches!(wire, Wire::Little) {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
        }
    }
    bytes
}

fn input(name: &str, text: &str, wire: Wire, bom: bool) -> Input {
    Input {
        name: name.to_string(),
        src: encoded(text, wire, bom),
        wire,
        expected: None,
        landxml: false,
        external: true,
    }
}

fn inputs() -> Vec<Input> {
    let mut cases = vec![input("utf8-xml", XML, Wire::Utf8, false)];
    let mut sjis = input("shift-jis-xml", "", Wire::ShiftJis, false);
    sjis.src = b"<?xml version='1.0' encoding='Shift_JIS'?><p:r xmlns:p='urn:r' a='\x93\xfa\x96\x7b'>\x93\xfa<p:\x93\xfa/><![CDATA[\x96\x7b\r\n]]></p:r>".to_vec();
    // Python の Expat は Shift_JIS のような複数バイトの外部文字コードに対応しない。
    sjis.external = false;
    cases.push(sjis);
    for wire in [Wire::Little, Wire::Big] {
        let label = if matches!(wire, Wire::Little) {
            "UTF-16LE"
        } else {
            "UTF-16BE"
        };
        for (name, declaration, bom) in [
            (
                "bom-generic",
                "<?xml version='1.0' encoding='UTF-16'?>".to_string(),
                true,
            ),
            ("bom-no-declaration", String::new(), true),
            (
                "specific-no-bom",
                format!("<?xml version='1.0' encoding='{label}'?>"),
                false,
            ),
        ] {
            cases.push(input(
                &format!("{wire:?}-{name}"),
                &format!("{declaration}{XML}"),
                wire,
                bom,
            ));
        }
        let entity = "<r>日本😀&unresolved;</r>";
        let mut unresolved = input(&format!("{wire:?}-unresolved"), entity, wire, true);
        unresolved.external = false;
        cases.push(unresolved);
        for (name, text, bom) in [
            (
                "bom-specific",
                format!("<?xml version='1.0' encoding='{label}'?><r/>"),
                true,
            ),
            (
                "bom-utf8-mismatch",
                "<?xml version='1.0' encoding='UTF-8'?><r/>".to_string(),
                true,
            ),
            (
                "generic-no-bom",
                "<?xml version='1.0' encoding='UTF-16'?><r/>".to_string(),
                false,
            ),
            (
                "specific-wrong-order",
                format!(
                    "<?xml version='1.0' encoding='{}'?><r/>",
                    if matches!(wire, Wire::Little) {
                        "UTF-16BE"
                    } else {
                        "UTF-16LE"
                    }
                ),
                false,
            ),
            (
                "bad-closing-tag",
                "<根>日本😀<子/></違う>".to_string(),
                true,
            ),
        ] {
            let mut invalid = input(&format!("{wire:?}-{name}"), &text, wire, bom);
            invalid.expected = Some(Error::Invalid);
            invalid.external = false;
            cases.push(invalid);
        }
        for (name, units) in [
            ("lone-high", vec![0xd800]),
            ("lone-low", vec![0xdc00]),
            ("bad-pair", vec![0xd800, 0x0041]),
            ("reversed-pair", vec![0xdc00, 0xd800]),
            ("nul", vec![0x0000]),
            ("invalid-char", vec![0xfffe]),
        ] {
            let mut invalid = input(&format!("{wire:?}-{name}"), "<r>日本😀", wire, true);
            for unit in units {
                invalid
                    .src
                    .extend_from_slice(&if matches!(wire, Wire::Little) {
                        u16::to_le_bytes(unit)
                    } else {
                        u16::to_be_bytes(unit)
                    });
            }
            invalid.expected = Some(Error::Invalid);
            invalid.external = false;
            cases.push(invalid);
        }
        let mut odd = input(&format!("{wire:?}-odd"), "<r/>", wire, true);
        odd.src.pop();
        odd.expected = Some(Error::Invalid);
        odd.external = false;
        cases.push(odd);
    }
    for (name, prefix) in [
        ("utf32-le", [0xff, 0xfe, 0, 0]),
        ("utf32-be", [0, 0, 0xfe, 0xff]),
    ] {
        cases.push(Input {
            name: name.to_string(),
            src: prefix.to_vec(),
            wire: Wire::Utf8,
            expected: Some(Error::Unsupported),
            landxml: false,
            external: false,
        });
    }
    for wire in [Wire::Utf8, Wire::Little, Wire::Big] {
        let text = format!(
            "<LandXML xmlns='http://www.landxml.org/schema/LandXML-1.2' version='1.2'><CoordinateSystem horizontalDatum='JGD2011' verticalDatum='T.P' horizontalCoordinateSystemName='9(X,Y)'/><Alignments><Alignment name='本線😀' staStart='1000' length='200'><CoordGeom><Line length='10.5'><Start>0 0</Start><End>10.5 0</End></Line></CoordGeom>{CANT}</Alignment></Alignments></LandXML>"
        );
        let mut land = input(
            &format!("{wire:?}-landxml"),
            &text,
            wire,
            !matches!(wire, Wire::Utf8),
        );
        land.landxml = true;
        cases.push(land);
    }
    cases
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Xml,
    LandXml,
    Lift,
}

struct Success {
    document: Document,
    model: Option<CivilModel>,
    records: Vec<(Vec<u8>, Option<zumen_base::ByteRange>)>,
    written: Vec<u8>,
}

struct Outcome {
    result: Result<Success, Error>,
    diag: Diagnostics,
    budget: Budget,
}

fn run(case: &Input, stage: Stage, limits: Limits, signal: Option<Stop>) -> Outcome {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    match signal {
        Some(Stop::Cancelled) => cancel.cancel(),
        Some(Stop::TimedOut) => cancel.expire(),
        _ => {}
    }
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let result = match stage {
        Stage::Xml => zumen_xml::read(&case.src, &mut cx).map(|document| Success {
            document,
            model: None,
            records: vec![],
            written: vec![],
        }),
        Stage::LandXml | Stage::Lift => {
            let file = if matches!(stage, Stage::Lift) {
                let mut initial_budget = Budget::default();
                let mut initial_diag = Diagnostics::new();
                let initial_cancel = CancelToken::new();
                LandXml::read(
                    &case.src,
                    &mut Cx::new(&mut initial_budget, &mut initial_diag, &initial_cancel),
                )
            } else {
                LandXml::read(&case.src, &mut cx)
            };
            file.and_then(|file| {
                let model = file.lift(&mut cx)?;
                let records = (0..file.record_count())
                    .filter_map(|index| {
                        file.record(index)
                            .map(|record| (record.kind.to_vec(), record.bytes))
                    })
                    .collect();
                let mut written = Vec::new();
                LandXml::write(&file, &mut written, &mut cx)?;
                Ok(Success {
                    document: file.document().clone(),
                    model: Some(model),
                    records,
                    written,
                })
            })
        }
    };
    Outcome {
        result,
        diag,
        budget,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn name_snapshot(doc: &Document, name: &Name) -> String {
    format!(
        "{:?} {:?} {:?} {:?} {} {} {:?}",
        name.raw(),
        name.local(),
        name.prefix(),
        name.namespace(),
        hex(doc.local_name(name)),
        hex(doc.parsed_local_name(name)),
        doc.local_name_text(name)
    )
}

fn snapshot(outcome: &Outcome) -> String {
    let mut out = String::new();
    match &outcome.result {
        Err(error) => out.push_str(&format!("error {error:?}\n")),
        Ok(success) => {
            let doc = &success.document;
            out.push_str(&format!(
                "document {:?} {} {:?}\n",
                doc.encoding(),
                hex(doc.src()),
                doc.doctype()
            ));
            for (index, element) in doc.elements().iter().enumerate() {
                out.push_str(&format!(
                    "element {index} {:?} {:?} {} {:?}\n",
                    element.span(),
                    element.parent(),
                    name_snapshot(doc, element.name()),
                    doc.namespace(element.name())
                ));
                for attr in element.attributes() {
                    out.push_str(&format!(
                        "attribute {} {:?} {:?}\n",
                        name_snapshot(doc, attr.name()),
                        attr.raw(),
                        doc.value(attr)
                    ));
                }
                for node in element.children() {
                    match node {
                        Node::Element(index) => out.push_str(&format!("child {index}\n")),
                        Node::Text(range) => {
                            out.push_str(&format!("text {range:?} {:?}\n", doc.text(range.clone())))
                        }
                        Node::CData(range) => out
                            .push_str(&format!("cdata {range:?} {:?}\n", doc.cdata(range.clone()))),
                    }
                }
            }
            out.push_str(&format!(
                "records {:?}\nwritten {}\n",
                success.records,
                hex(&success.written)
            ));
            if let Some(model) = &success.model {
                out.push_str(&format!(
                    "model {:?} {:?} {:?}\n",
                    model.crs, model.realization, model.height_datum
                ));
                for alignment in &model.alignments {
                    out.push_str(&format!(
                        "alignment {:?} {:?} {:?} {:016x}\n",
                        alignment.id,
                        alignment.name,
                        alignment.source,
                        alignment.stations.start.to_bits()
                    ));
                    for horizontal in &alignment.horizontal {
                        out.push_str(&format!(
                            "horizontal {:016x} {:016x} {:016x} {:016x} {:?} {:?}\n",
                            horizontal.start.northing.to_bits(),
                            horizontal.start.easting.to_bits(),
                            horizontal.direction.to_bits(),
                            horizontal.length.to_bits(),
                            horizontal.kind,
                            horizontal.source
                        ));
                    }
                    for cant in &alignment.cant {
                        out.push_str(&format!(
                            "cant {:016x} {:016x} {:?} {:?}\n",
                            cant.start_distance.to_bits(),
                            cant.length.to_bits(),
                            cant.transition,
                            cant.source
                        ));
                        if let CantKind::Superelevation { start, end } = cant.kind {
                            out.push_str(&format!(
                                "slopes {:?} {:?}\n",
                                start.map(f64::to_bits),
                                end.map(f64::to_bits)
                            ));
                        }
                    }
                }
            }
        }
    }
    for diagnostic in outcome.diag.iter() {
        out.push_str(&format!(
            "diagnostic {} {} {:?} {:?} {:?} {:?}\n",
            diagnostic.code().name(),
            diagnostic.count(),
            diagnostic.severity(),
            diagnostic.fidelity(),
            diagnostic.places(),
            diagnostic.spec()
        ));
        for (name, value) in diagnostic.params() {
            match value {
                Value::Float(value) => {
                    out.push_str(&format!("param {name} float {:016x}\n", value.to_bits()))
                }
                value => out.push_str(&format!("param {name} {value:?}\n")),
            }
        }
    }
    for kind in Kind::ALL {
        out.push_str(&format!(
            "budget {} {}\n",
            kind.name(),
            outcome.budget.used(kind)
        ));
    }
    out.push_str(&format!("exceeded {:?}\n", outcome.budget.exceeded()));
    out
}

fn check_range(case: &Input, range: Range<usize>) {
    assert!(
        range.start <= range.end && range.end <= case.src.len(),
        "{}: {range:?}",
        case.name
    );
    if matches!(case.wire, Wire::Little | Wire::Big) {
        assert_eq!(range.start % 2 + range.end % 2, 0);
    }
}

fn verify_success(case: &Input, success: &Success) -> Result<(), String> {
    let doc = &success.document;
    assert_eq!(doc.src(), case.src);
    for element in doc.elements() {
        check_range(case, element.span());
        for name in std::iter::once(element.name())
            .chain(element.attributes().iter().map(|attr| attr.name()))
        {
            for range in [Some(name.raw()), Some(name.local()), name.prefix()]
                .into_iter()
                .flatten()
            {
                check_range(case, range);
            }
            assert_eq!(doc.local_name(name), doc.bytes(name.local()));
            if !matches!(case.wire, Wire::ShiftJis) {
                assert_eq!(
                    doc.local_name(name),
                    encoded(&doc.local_name_text(name), case.wire, false)
                );
                assert_eq!(
                    doc.parsed_local_name(name),
                    doc.local_name_text(name).as_bytes()
                );
            }
            if let Some(Namespace::Declared(range)) = name.namespace() {
                check_range(case, range.clone());
            }
        }
        for attr in element.attributes() {
            check_range(case, attr.raw());
        }
        for node in element.children() {
            if let Node::Text(range) | Node::CData(range) = node {
                check_range(case, range.clone());
            }
        }
    }
    if let Some(range) = doc.doctype() {
        check_range(case, range.clone());
        assert_eq!(
            doc.bytes(range),
            encoded("<!DOCTYPE p:根 [<!ELEMENT p:根 ANY>]>", case.wire, false)
        );
        let root = doc
            .element(doc.root())
            .ok_or_else(|| "ルートがない".to_string())?;
        assert!(doc.is(root.name(), Some("urn:root"), "根".as_bytes()));
        assert_eq!(
            doc.bytes(root.name().raw()),
            encoded("p:根", case.wire, false)
        );
        let prefix = root
            .name()
            .prefix()
            .ok_or_else(|| "接頭辞がない".to_string())?;
        assert_eq!(doc.bytes(prefix), encoded("p", case.wire, false));
        let attribute = doc
            .attribute(root, Some("urn:root"), "属性".as_bytes())
            .ok_or_else(|| "属性がない".to_string())?;
        assert_eq!(
            doc.bytes(attribute.raw()),
            encoded("甲&#x1F600;\r\n乙", case.wire, false)
        );
        assert_eq!(doc.value(attribute), "甲😀 乙");
        let raw_text: Vec<_> = root
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::Text(range) => Some(doc.bytes(range.clone())),
                _ => None,
            })
            .collect();
        let expected_text: Vec<_> = ["前\r\n日本😀&#38;", "中", "尾"]
            .map(|text| encoded(text, case.wire, false))
            .into_iter()
            .collect();
        assert_eq!(raw_text, expected_text);
        let raw_cdata: Vec<_> = root
            .children()
            .iter()
            .filter_map(|node| match node {
                Node::CData(range) => Some(doc.bytes(range.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(raw_cdata, [encoded("生&😀\r\n文", case.wire, false)]);
    }
    if let Some(model) = &success.model {
        assert_eq!(success.written, case.src);
        assert_eq!(success.records.len(), doc.elements().len());
        for ((kind, bytes), element) in success.records.iter().zip(doc.elements()) {
            assert_eq!(kind, doc.local_name(element.name()));
            let bytes = bytes.ok_or_else(|| "レコードの原本範囲がない".to_string())?;
            assert_eq!(
                (bytes.start, bytes.end),
                (element.span().start as u64, element.span().end as u64)
            );
        }
        let [alignment] = model.alignments.as_slice() else {
            return Err("線形の数が違う".to_string());
        };
        assert_eq!(alignment.id, "本線😀");
        let [line] = alignment.horizontal.as_slice() else {
            return Err("平面区間の数が違う".to_string());
        };
        assert_eq!(line.length.to_bits(), 10.5_f64.to_bits());
        let [cant] = alignment.cant.as_slice() else {
            return Err("片勾配の数が違う".to_string());
        };
        assert_eq!((cant.start_distance, cant.length), (20.0, 25.0));
        assert_eq!(
            cant.kind,
            CantKind::Superelevation {
                start: [-6.25; 2],
                end: [-6.25; 2]
            }
        );
        let raw = cant
            .source
            .bytes()
            .ok_or_else(|| "片勾配の出所がない".to_string())?;
        assert_eq!(
            case.src.get(raw.start as usize..raw.end as usize),
            Some(encoded(CANT, case.wire, false).as_slice())
        );
    }
    Ok(())
}

fn verify_diagnostics(case: &Input, outcome: &Outcome) {
    for diagnostic in outcome.diag.iter() {
        for place in diagnostic.places() {
            if let Some(bytes) = place.bytes() {
                assert!(bytes.start <= bytes.end && bytes.end <= case.src.len() as u64);
                if !case.name.ends_with("-odd") {
                    check_range(case, bytes.start as usize..bytes.end as usize);
                }
                if diagnostic.code() == codes::XML_ENTITY_UNSUPPORTED {
                    assert_eq!(
                        case.src.get(bytes.start as usize..bytes.end as usize),
                        Some(encoded("&unresolved;", case.wire, false).as_slice())
                    );
                }
                if diagnostic.code() == codes::XML_DTD_DROPPED {
                    assert_eq!(
                        case.src.get(bytes.start as usize..bytes.end as usize),
                        Some(
                            encoded("<!DOCTYPE p:根 [<!ELEMENT p:根 ANY>]>", case.wire, false)
                                .as_slice()
                        )
                    );
                }
                if diagnostic.code() == codes::LANDXML_ELEMENT_UNSUPPORTED {
                    assert_eq!(diagnostic.param("element"), Some(&Value::from("AdverseSE")));
                    assert_eq!(
                        case.src.get(bytes.start as usize..bytes.end as usize),
                        Some(
                            encoded("<AdverseSE>non-adverse</AdverseSE>", case.wire, false)
                                .as_slice()
                        )
                    );
                }
            }
        }
    }
    if let Ok(success) = &outcome.result {
        if success.document.doctype().is_some() {
            assert_eq!(
                outcome
                    .diag
                    .iter()
                    .find(|d| d.code() == codes::XML_DTD_DROPPED)
                    .map(|d| d.count()),
                Some(1)
            );
        }
        if case.name.ends_with("-unresolved") {
            assert_eq!(
                outcome
                    .diag
                    .iter()
                    .find(|d| d.code() == codes::XML_ENTITY_UNSUPPORTED)
                    .map(|d| d.count()),
                Some(1)
            );
        }
        if success.model.is_some() {
            assert_eq!(
                outcome
                    .diag
                    .iter()
                    .find(|d| d.code() == codes::LANDXML_ELEMENT_UNSUPPORTED)
                    .map(|d| d.count()),
                Some(1)
            );
        }
    }
}

fn minimum_depth(case: &Input, stage: Stage) -> Result<u64, String> {
    for depth in 0..=32 {
        let outcome = run(
            case,
            stage,
            Limits::default().with(Kind::Depth, Limit::Finite(depth)),
            None,
        );
        match outcome.result {
            Ok(_) => return Ok(depth),
            Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::Depth => {}
            _ => return Err("深さの探索中に別の失敗があった".to_string()),
        }
    }
    Err("合成入力の深さが 32 を超えた".to_string())
}

/// 原本範囲・全 Kind の境界・停止・繰り返しを検査し、比較する値を返す。
pub fn verify_contracts() -> Result<String, String> {
    let mut out = String::new();
    for case in inputs() {
        for stage in if case.landxml {
            &[Stage::Xml, Stage::LandXml, Stage::Lift][..]
        } else {
            &[Stage::Xml][..]
        } {
            let baseline = run(&case, *stage, Limits::default(), None);
            verify_diagnostics(&case, &baseline);
            match (&baseline.result, case.expected) {
                (Ok(success), None) => verify_success(&case, success)?,
                (Err(error), Some(expected)) => {
                    assert_eq!(*error, expected, "{}", case.name);
                    assert!(baseline.diag.iter().any(|d| d.code()
                        == if expected == Error::Unsupported {
                            codes::XML_ENCODING_UNSUPPORTED
                        } else {
                            codes::XML_DOCUMENT_INVALID
                        }));
                }
                _ => return Err(format!("{}: 受理条件と結果が違う", case.name)),
            }
            let baseline_text = snapshot(&baseline);
            assert_eq!(
                baseline_text,
                snapshot(&run(&case, *stage, Limits::default(), None))
            );
            out.push_str(&format!("case {} {stage:?}\n{baseline_text}", case.name));
            if case.expected.is_some() {
                continue;
            }
            for kind in Kind::ALL {
                let used = if kind == Kind::Depth {
                    minimum_depth(&case, *stage)?
                } else {
                    baseline.budget.used(kind)
                };
                let exact = run(
                    &case,
                    *stage,
                    Limits::default().with(kind, Limit::Finite(used)),
                    None,
                );
                assert_eq!(
                    baseline_text,
                    snapshot(&exact),
                    "{} {stage:?} {kind:?}",
                    case.name
                );
                out.push_str(&format!("boundary {} {used}\n", kind.name()));
                if let Some(limit) = used.checked_sub(1) {
                    let stopped = run(
                        &case,
                        *stage,
                        Limits::default().with(kind, Limit::Finite(limit)),
                        None,
                    );
                    assert!(
                        matches!(stopped.result, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == kind && e.limit == limit),
                        "{} {stage:?} {kind:?}",
                        case.name
                    );
                    verify_diagnostics(&case, &stopped);
                    out.push_str(&snapshot(&stopped));
                }
            }
            for signal in [Stop::Cancelled, Stop::TimedOut] {
                let stopped = run(&case, *stage, Limits::default(), Some(signal));
                assert!(matches!(stopped.result, Err(Error::Stopped(reason)) if reason == signal));
                assert!(
                    Kind::ALL
                        .into_iter()
                        .all(|kind| stopped.budget.used(kind) == 0)
                );
                out.push_str(&format!("signal {signal:?}\n{}", snapshot(&stopped)));
            }
        }
    }
    Ok(out)
}

fn external_element(doc: &Document, index: usize, out: &mut String) -> Result<(), String> {
    let element = doc
        .element(index)
        .ok_or_else(|| "要素の番号がない".to_string())?;
    let ns = doc
        .namespace(element.name())
        .map(|value| hex(value.as_bytes()))
        .unwrap_or_else(|| "-".to_string());
    out.push_str(&format!(
        "START\t{index}\t{}\t{}\t{}\t{}\t{ns}\n",
        element
            .parent()
            .map(|parent| parent.to_string())
            .unwrap_or_else(|| "-".to_string()),
        element.span().start,
        element.span().end,
        hex(doc.local_name_text(element.name()).as_bytes())
    ));
    for attr in element.attributes() {
        if matches!(attr.name().namespace(), Some(Namespace::Xmlns)) {
            continue;
        }
        out.push_str(&format!(
            "ATTR\t{index}\t{}\t{}\t{}\n",
            hex(doc.local_name_text(attr.name()).as_bytes()),
            doc.namespace(attr.name())
                .map(|value| hex(value.as_bytes()))
                .unwrap_or_else(|| "-".to_string()),
            hex(doc.value(attr).as_bytes())
        ));
    }
    for node in element.children() {
        match node {
            Node::Element(child) => external_element(doc, *child, out)?,
            Node::Text(range) => out.push_str(&format!(
                "TEXT\t{index}\ttext\t{}\n",
                hex(doc.text(range.clone()).as_bytes())
            )),
            Node::CData(range) => out.push_str(&format!(
                "TEXT\t{index}\tcdata\t{}\n",
                hex(doc.cdata(range.clone()).as_bytes())
            )),
        }
    }
    out.push_str(&format!("END\t{index}\n"));
    Ok(())
}

/// 受理済みの合成入力と、独立した XML 実装と比べる公開値を TSV で返す。
pub fn external_cases() -> Result<String, String> {
    let mut out = String::new();
    for case in inputs()
        .into_iter()
        .filter(|case| case.external && case.expected.is_none())
    {
        let outcome = run(&case, Stage::Xml, Limits::default(), None);
        let success = outcome
            .result
            .map_err(|_| format!("{}: 外部照合用の入力を読めない", case.name))?;
        out.push_str(&format!("CASE\t{}\t{}\n", case.name, hex(&case.src)));
        external_element(&success.document, success.document.root(), &mut out)?;
        out.push_str("CASEEND\n");
    }
    Ok(out)
}

#[cfg(test)]
#[test]
fn utf16_original_ranges_models_diagnostics_and_budgets_are_repeatable() {
    assert_eq!(verify_contracts().unwrap(), verify_contracts().unwrap());
    assert_eq!(external_cases().unwrap(), external_cases().unwrap());
}
