use super::*;

#[derive(Debug, PartialEq)]
struct EmbeddedContext {
    attachment: i32,
    axis: [f64; 3],
    point: [f64; 3],
    width: f64,
    height: f64,
    extents: [f64; 2],
    kind: i32,
}

fn int(code: i16, value: i64) -> Value {
    Value {
        code,
        data: Data::Int(value),
    }
}

fn real(code: i16, value: f64) -> Value {
    Value {
        code,
        data: Data::Real(value),
    }
}

fn text_val(code: i16, value: &[u8]) -> Value {
    Value {
        code,
        data: Data::Text(value.to_vec()),
    }
}

#[allow(clippy::too_many_arguments)]
fn mtext_object(
    point: [f64; 3],
    axis: Option<[f64; 3]>,
    width: f64,
    frame_height: f64,
    attachment: i64,
    content: &[u8],
) -> Object {
    let mut values = vec![
        real(10, point[0]),
        real(20, point[1]),
        real(30, point[2]),
        real(41, width),
        real(46, frame_height),
        real(40, 5.0),
        int(71, attachment),
        int(72, 1),
        text_val(1, content),
        int(73, 1),
        real(44, 1.0),
    ];
    if let Some([x, y, z]) = axis {
        values.extend([real(11, x), real(21, y), real(31, z)]);
    }
    Object {
        kind: b"MTEXT".to_vec(),
        section: Section::Entities,
        origin: Origin::New,
        values,
    }
}

fn split_mc(encoded: &[u8]) -> (&[u8], usize) {
    let mut at = 0;
    let handle_bits = usize::try_from(modular_char(encoded, &mut at, false).unwrap()).unwrap();
    let data = encoded.get(at..).unwrap();
    (data, data.len() * 8 - handle_bits)
}

/// R2018 の MTEXT のデータ（MC の後）から、背景の後に続く埋め込みの注釈フィールドを読む
fn read_r2018_embedded(data: &[u8], handles_at: usize) -> EmbeddedContext {
    let [_, start, end] = r2010_entity_fields(data, handles_at, Release::R2018).unwrap();
    let mut bits = Bits::with_release(data, Release::R2018);
    bits.seek(start);
    let _ = super::super::objects::mtext(&mut bits).unwrap();
    assert!(bits.bit().unwrap());
    assert_eq!(bits.bs().unwrap(), 4);
    assert!(bits.bit().unwrap());
    let attachment = bits.bl().unwrap();
    let axis = [bits.bd().unwrap(), bits.bd().unwrap(), bits.bd().unwrap()];
    let point = [bits.bd().unwrap(), bits.bd().unwrap(), bits.bd().unwrap()];
    let width = bits.bd().unwrap();
    let height = bits.bd().unwrap();
    let extents = [bits.bd().unwrap(), bits.bd().unwrap()];
    let kind = bits.bl().unwrap();
    if kind == 0 {
        assert_eq!(bits.pos(), end);
    }
    EmbeddedContext {
        attachment,
        axis,
        point,
        width,
        height,
        extents,
        kind,
    }
}

/// R2018 の DWG に足した MTEXT を書き戻し直すと、主のデータだけでなく埋め込みの注釈フィールドの揃え・X 軸・
/// 挿入点・枠の幅と高さも更新される（#880）
#[test]
fn r2018_mtext_updates_embedded_context_fields_on_write_back() {
    let initial = mtext_object([100.0, -590.0, 0.0], None, 0.0, 0.0, 1, b"ZUMEN\\P878");
    let added = NewEntity {
        object: &initial,
        handle: 0x49F,
        type_code: kinds::MTEXT,
        mode: 2,
        owner: 0x1F,
        layer: 0x10,
        ltype: (0, 0),
        pointed: Some(0x11),
        embedded: None,
        children: None,
    };
    let encoded = added.encode(0, 0, Release::R2018).unwrap();
    let (data, handles_at) = split_mc(&encoded);
    assert_eq!(
        read_r2018_embedded(data, handles_at),
        EmbeddedContext {
            attachment: 1,
            axis: [1.0, 0.0, 0.0],
            point: [100.0, -590.0, 0.0],
            width: 0.0,
            height: 0.0,
            extents: [0.0, 0.0],
            kind: 0,
        }
    );

    let updated = mtext_object(
        [105.0, -585.0, 0.0],
        Some([0.0, 1.0, 0.0]),
        60.0,
        25.0,
        8,
        b"ZUMEN\\P880",
    );
    let rewritten = encode_r2010(
        data,
        handles_at,
        &initial,
        &updated,
        Release::R2018,
        Restyle::default(),
    )
    .unwrap();
    let (data2, handles_at2) = split_mc(&rewritten);
    assert_eq!(
        read_r2018_embedded(data2, handles_at2),
        EmbeddedContext {
            attachment: 8,
            axis: [0.0, 1.0, 0.0],
            point: [105.0, -585.0, 0.0],
            width: 60.0,
            height: 25.0,
            extents: [0.0, 0.0],
            kind: 0,
        }
    );
}

/// 注釈的（`not_annotative == false`）または既定でない（`is_default == false`）R2018 の MTEXT は、文字だけの変更は
/// 元のビットを写して受け付け、位置や幅の変更は古い埋め込みの値を残さないよう断る（#880）
#[test]
fn r2018_annotative_mtext_rejects_position_change_but_allows_text_change() {
    let mut fields = BitWriter::with_strings();
    let base = mtext_object([10.0, 20.0, 0.0], None, 0.0, 0.0, 1, b"before");
    // まず通常の足した MTEXT のビット列を作り、R2018 の注釈的でないフラグ（B）を false に書き換える
    let added = NewEntity {
        object: &base,
        handle: 0x40,
        type_code: kinds::MTEXT,
        mode: 2,
        owner: 0x1F,
        layer: 0x10,
        ltype: (0, 0),
        pointed: Some(0x11),
        embedded: None,
        children: None,
    };
    let encoded = added.encode(0, 0, Release::R2018).unwrap();
    let (data, handles_at) = split_mc(&encoded);
    let [_, start, end] = r2010_entity_fields(data, handles_at, Release::R2018).unwrap();
    let mut bits = Bits::with_release(data, Release::R2018);
    bits.seek(start);
    let _ = super::super::objects::mtext(&mut bits).unwrap();
    let annotative_bit = bits.pos();
    fields.copy(data, 0..annotative_bit).unwrap();
    fields.bit(false);
    fields
        .copy(data, (annotative_bit + 1)..(data.len() * 8))
        .unwrap();
    let patched = fields.into_bytes();
    assert!(end > annotative_bit);

    let text_only = mtext_object([10.0, 20.0, 0.0], None, 0.0, 0.0, 1, b"after");
    assert!(
        encode_r2010(
            &patched,
            handles_at,
            &base,
            &text_only,
            Release::R2018,
            Restyle::default(),
        )
        .is_some()
    );

    let moved = mtext_object([15.0, 25.0, 0.0], None, 0.0, 0.0, 1, b"before");
    assert!(
        encode_r2010(
            &patched,
            handles_at,
            &base,
            &moved,
            Release::R2018,
            Restyle::default(),
        )
        .is_none()
    );
}

/// R2018 の複数行の ATTRIB も、埋め込みの MTEXT の注釈フィールドを更新する（#880）
#[test]
fn r2018_multiline_attrib_updates_embedded_mtext_context_fields() {
    let make_attrib = |point: [f64; 3], width: f64| Object {
        kind: b"ATTRIB".to_vec(),
        section: Section::Entities,
        origin: Origin::New,
        values: vec![
            real(10, point[0]),
            real(20, point[1]),
            real(30, point[2]),
            real(40, 2.5),
            text_val(1, b"one"),
            int(280, 0),
            text_val(2, b"TAG"),
            int(70, 0),
            int(280, 0),
            int(71, 2),
            int(72, 0),
            text_val(101, b"Embedded Object"),
            real(10, point[0]),
            real(20, point[1]),
            real(30, point[2]),
            real(41, width),
            real(46, 0.0),
            real(40, 2.5),
            int(71, 1),
            int(72, 5),
            text_val(1, b"one\\Ptwo"),
            int(73, 1),
            real(44, 1.0),
        ],
    };
    let initial = make_attrib([8.0, 9.0, 0.0], 0.0);
    let added = NewEntity {
        object: &initial,
        handle: 0x50,
        type_code: kinds::ATTRIB,
        mode: 0,
        owner: 0x4F,
        layer: 0x10,
        ltype: (0, 0),
        pointed: Some(0x11),
        embedded: Some((0x10, 0x11)),
        children: None,
    };
    let encoded = added.encode(0, 0, Release::R2018).unwrap();
    let (data, handles_at) = split_mc(&encoded);
    let updated = make_attrib([18.0, 29.0, 0.0], 45.0);
    let rewritten = encode_r2010(
        data,
        handles_at,
        &initial,
        &updated,
        Release::R2018,
        Restyle::default(),
    )
    .unwrap();
    let (data2, handles_at2) = split_mc(&rewritten);
    let [_, start, _] = r2010_entity_fields(data2, handles_at2, Release::R2018).unwrap();
    let [_, mtext_at, mtext_end] = embedded_span(data2, start, Release::R2018).unwrap();
    let mut bits = Bits::with_release(data2, Release::R2018);
    bits.seek(mtext_at);
    let _ = super::super::objects::mtext(&mut bits).unwrap();
    assert!(bits.bit().unwrap());
    assert_eq!(bits.bs().unwrap(), 4);
    assert!(bits.bit().unwrap());
    assert_eq!(bits.bl().unwrap(), 1);
    assert_eq!(
        [bits.bd().unwrap(), bits.bd().unwrap(), bits.bd().unwrap()],
        [1.0, 0.0, 0.0]
    );
    assert_eq!(
        [bits.bd().unwrap(), bits.bd().unwrap(), bits.bd().unwrap()],
        [18.0, 29.0, 0.0]
    );
    assert_eq!(bits.bd().unwrap(), 45.0);
    assert_eq!(bits.bd().unwrap(), 0.0);
    let _ = [bits.bd().unwrap(), bits.bd().unwrap()];
    assert_eq!(bits.bl().unwrap(), 0);
    assert_eq!(bits.pos(), mtext_end);
}

/// 公開サンプル `zumen-layout-r2018.dwg` に `apply` で MTEXT を足し、書き戻した DWG を開き直して `Transform` と
/// `Replace` を当てたとき、AutoCAD 2027 が読む埋め込みの注釈フィールドも (105, -585) と幅 60 になる（#880）
#[cfg(feature = "drawing")]
#[test]
fn added_mtext_on_r2018_sample_updates_embedded_fields_across_reopen_and_apply() {
    use std::path::Path;
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref};
    use zumen_drawing::{
        Affine, Changes, Edit, Element, Flow, FormattedText, Hash, Height, HeightMeaning,
        Horizontal, Length, LineSpacing, ModelLength, ModelPoint, Placed, Rotation, Seen, Shape,
        Vertical, id,
    };
    use zumen_sdk::{Apply, Codec, Lift};

    let read = |bytes: &[u8]| {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        crate::Dwg::read(
            bytes,
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        )
        .unwrap()
    };
    let write = |file: &DwgFile| {
        let mut out = Vec::new();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        crate::Dwg::write(
            file,
            &mut out,
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        )
        .unwrap();
        out
    };
    let lift = |file: &DwgFile| {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        file.lift(&mut Cx::new(&mut budget, &mut diag, &CancelToken::new()))
            .unwrap()
    };
    let apply_edit = |file: &mut DwgFile, edit: Edit| {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        file.apply(
            &Changes {
                document: None,
                edits: vec![edit],
            },
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        )
        .unwrap();
    };
    let inspect_mtext = |file: &DwgFile| {
        let entry = file
            .entries
            .iter()
            .find(|entry| entry.type_code == kinds::MTEXT)
            .unwrap();
        read_r2018_embedded(file.data(entry), entry.handles.unwrap())
    };

    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg/zumen-layout-r2018.dwg");
    let src = std::fs::read(path).unwrap();
    let mut step1 = read(&src);
    let before = lift(&step1);
    let space = before.spaces.first().unwrap();
    let space_id = space.id;
    let model_len = |value| {
        Length::Model(ModelLength {
            space: space_id,
            value,
        })
    };
    let template = space.elements.first().unwrap();
    let content = "ZUMEN\\P878".to_owned();
    let spans = crate::mtext_spans(&content, &model_len);
    let text = FormattedText {
        content,
        spans,
        height: Height {
            value: model_len(5.0),
            meaning: HeightMeaning::Declared("dxf.MTEXT.40".into()),
        },
        point: ModelPoint {
            space: space_id,
            x: 100.0,
            y: -590.0,
        },
        horizontal: Horizontal::Left,
        vertical: Vertical::Top,
        rotation: Rotation::Angle(0.0),
        width: None,
        frame_height: None,
        flow: Flow::LeftToRight,
        line_spacing: LineSpacing {
            factor: 1.0,
            exact: false,
        },
        background: None,
        style: "Standard".to_owned(),
    };
    apply_edit(
        &mut step1,
        Edit::Add {
            place: "space/0".to_owned(),
            element: Box::new(Placed::Model(Element {
                source: Ref::default(),
                layer: template.layer,
                style: template.style,
                shape: Shape::Formatted(text),
            })),
        },
    );
    let step1_bytes = write(&step1);

    // 2 回目（Transform）: 開き直して [1, 0, 5, 0, 1, 5] で動かす
    let mut moved_file = read(&step1_bytes);
    let after_add = lift(&moved_file);
    let added_space = after_add.spaces.first().unwrap();
    let place = id::space(added_space.id);
    let mut namer = id::Namer::new(&place);
    let (mtext_id, mtext_elem) = added_space
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| (namer.next(index, element), element))
        .find(|(_, element)| matches!(element.shape, Shape::Formatted(_)))
        .unwrap();
    assert_eq!(mtext_id, "space/0/h49F");
    let target = Seen {
        id: mtext_id,
        hash: Hash::element(mtext_elem),
    };
    apply_edit(
        &mut moved_file,
        Edit::Transform {
            target: target.clone(),
            transform: Affine {
                m: [1.0, 0.0, 5.0, 0.0, 1.0, 5.0],
            },
        },
    );
    let moved_reopened = read(&write(&moved_file));
    assert_eq!(inspect_mtext(&moved_reopened).point, [105.0, -585.0, 0.0]);

    // 2 回目（Replace）: 開き直して幅 60・枠の高さ 25・位置合わせ（中央下 = 8）を付ける
    let mut replaced_file = read(&step1_bytes);
    let mut replaced_shape = match &mtext_elem.shape {
        Shape::Formatted(shape) => Some(shape.clone()),
        _ => None,
    }
    .unwrap();
    replaced_shape.width = Some(model_len(60.0));
    replaced_shape.frame_height = Some(model_len(25.0));
    replaced_shape.horizontal = Horizontal::Center;
    replaced_shape.vertical = Vertical::Bottom;
    apply_edit(
        &mut replaced_file,
        Edit::Replace {
            target,
            element: Box::new(Placed::Model(Element {
                shape: Shape::Formatted(replaced_shape),
                ..mtext_elem.clone()
            })),
        },
    );
    let replaced_reopened = read(&write(&replaced_file));
    let embedded = inspect_mtext(&replaced_reopened);
    assert_eq!(embedded.point, [100.0, -590.0, 0.0]);
    assert_eq!(embedded.width, 60.0);
    assert_eq!(embedded.height, 25.0);
    assert_eq!(embedded.attachment, 8);
}
