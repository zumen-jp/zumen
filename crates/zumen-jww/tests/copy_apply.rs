//! 要素を複写し（`Edit::Copy` / `JwwFile::apply_with_copies`）、写しが元の属性・拡張・関連情報を保つことと、
//! 保てない場合の理由付き全件拒否・予算停止時の原子性を確かめる（#889）。
#![cfg(feature = "drawing")]

mod common;

use core::f64::consts::PI;

use common::{Archive, Spec};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_drawing::{
    Affine, Changes, Drawing2D, Edit, Element, Field, FieldValue, Hash, ModelPoint, Seen, Shape,
    Target, id, transform,
};
use zumen_jww::{CopyFrom, Jww, JwwFile, JwwLive};
use zumen_sdk::{Apply, Codec, Lift, testkit};

const SHIFT: Affine = Affine {
    m: [1.0, 0.0, 100.0, 0.0, 1.0, 200.0],
};

const SCALE_SHIFT: Affine = Affine {
    m: [2.0, 0.0, 10.0, 0.0, 2.0, 20.0],
};

/// グループ 0（縮尺 1/100）に線（flag=4）・円・円弧・楕円・点・点マーカー・文字（字間つき）・ソリッド・ブロック参照、
/// グループ 1（縮尺 1/50）に線、さらに曲線番号つき線と寸法を持つ JWW を組み立てる
fn source_jww() -> Vec<u8> {
    let mut spec = Spec::new(700);
    let mut layers = [0; 16];
    layers[0] = 3;
    layers[1] = 2;
    spec.groups[0] = (3, 100.0, layers);
    spec.groups[1] = (2, 50.0, layers);
    spec.max_draw_width = -300;
    spec.pens[1] = 0x0000_0000;
    spec.pens[2] = 0x0000_00FF;
    spec.printer_widths[1] = 18;
    spec.printer_widths[2] = 25;

    let mut archive = Archive::new(700);
    archive.layer = 0;
    archive.group = 0;
    archive.pen_width = 0;
    archive.count(12);

    // 0: CDataSen (flag = 4 -> jww.entity 拡張)
    archive.flag = 4;
    archive.line();
    archive.flag = 0;

    // 1: CDataEnko (円)
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[2.0, 3.0, 5.0, 0.0, 2.0 * PI, 0.0, 1.0]);
    archive.out.extend(1u32.to_le_bytes());

    // 2: CDataEnko (円弧)
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[0.0, 0.0, 6.0, 0.0, PI / 2.0, 0.0, 1.0]);
    archive.out.extend(0u32.to_le_bytes());

    // 3: CDataEnko (楕円弧: 扁平率 0.5)
    archive.tag("CDataEnko");
    archive.base(1, 2);
    archive.reals(&[1.0, 2.0, 8.0, 0.0, PI, 0.25, 0.5]);
    archive.out.extend(0u32.to_le_bytes());

    // 4: CDataTen (通常点)
    archive.tag("CDataTen");
    archive.base(1, 2);
    archive.reals(&[3.0, 4.0]);
    archive.out.extend(0u32.to_le_bytes());

    // 5: CDataTen (点マーカー pen_style = 100)
    archive.tag("CDataTen");
    archive.base(100, 1);
    archive.reals(&[4.0, 5.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.out.extend(2i32.to_le_bytes());
    archive.reals(&[0.0, 1.5]);

    // 6: CDataMoji (文字種 3・字間 1.2)
    archive.tag("CDataMoji");
    archive.base(1, 1);
    archive.reals(&[1.0, 2.0, 12.0, 2.0]);
    archive.out.extend(3u32.to_le_bytes());
    archive.reals(&[3.5, 4.0, 1.2, 0.0]);
    archive.cstring("ＭＳ ゴシック");
    archive.cstring("複写試験");

    // 7: CDataSolid (4 点四角形ソリッド・任意色)
    archive.tag("CDataSolid");
    archive.base(1, 10);
    archive.reals(&[0.0, 0.0, 4.0, 0.0, 4.0, 3.0, 0.0, 3.0]);
    archive.out.extend(0x0010_2030u32.to_le_bytes());

    // 8: CDataBlock (ブロック参照 番号 1)
    archive.tag("CDataBlock");
    archive.base(1, 1);
    archive.reals(&[6.0, 7.0, 1.0, 1.0, 0.0]);
    archive.out.extend(1u32.to_le_bytes());

    // 9: 曲線属性 curve = 7 を持つ線（参照切れになるため複写拒否対象）
    archive.curve = 7;
    archive.line();
    archive.curve = 0;

    // 10: CDataSunpou（寸法は複写拒否対象）
    archive.tag("CDataSunpou");
    archive.base(1, 1);
    archive.base(1, 1);
    archive.reals(&[0.0, 0.0, 10.0, 0.0]);
    archive.base(1, 1);
    archive.text_body("10");
    archive.out.extend(1u16.to_le_bytes());
    for _ in 0..2 {
        archive.base(1, 1);
        archive.reals(&[0.0; 4]);
    }
    for _ in 0..4 {
        archive.base(1, 1);
        archive.reals(&[0.0; 2]);
        archive.out.extend(0u32.to_le_bytes());
    }

    // 11: グループ 1（space/1）の線
    archive.group = 1;
    archive.line();
    archive.group = 0;

    // ブロック定義 1 件（番号 0: "部品"）
    archive.count(1);
    archive.block("部品");
    // 同梱画像 0 件
    archive.out.extend(0u32.to_le_bytes());

    let mut out = spec.build();
    out.extend(&archive.out);
    out
}

fn cx_run<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let value = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (value, diag)
}

#[cfg(test)]
fn read(src: &[u8]) -> JwwFile {
    testkit::read::<Jww>(src).0.unwrap()
}

#[cfg(test)]
fn write(file: &JwwFile) -> Vec<u8> {
    cx_run(|cx| {
        let mut out = Vec::new();
        Jww::write(file, &mut out, cx).unwrap();
        out
    })
    .0
}

#[cfg(test)]
fn lift(file: &JwwFile) -> Drawing2D {
    cx_run(|cx| file.lift(cx).unwrap()).0
}

#[cfg(test)]
fn model_elements(drawing: &Drawing2D) -> Vec<(String, Hash, Element<ModelPoint>)> {
    let mut out = Vec::new();
    id::each_element(drawing, |_, elem_id, found| {
        if let id::Found::Model(el) = found {
            out.push((elem_id, Hash::element(el), el.clone()));
        }
        Ok::<_, ()>(())
    })
    .unwrap();
    out
}

fn element_extensions(drawing: &Drawing2D, elem_id: &str) -> Vec<(String, Vec<Field>)> {
    drawing
        .extensions
        .iter()
        .filter(|ext| ext.target == Target::Element(elem_id.to_owned()))
        .map(|ext| (ext.namespace.clone().into_owned(), ext.fields.clone()))
        .collect()
}

#[test]
fn copies_keep_shapes_styles_and_extensions_in_jww_and_inside_blocks() {
    let src = source_jww();
    let mut file = read(&src);
    let before = lift(&file);
    let was = model_elements(&before);

    // 1) space/0 の 9 要素（線・円・円弧・楕円弧・点・点マーカー・文字・ソリッド・ブロック参照）を一括複写
    let targets: Vec<(String, Hash, Affine)> = was
        .iter()
        .filter(|(id, ..)| id.starts_with("space/0/"))
        .take(9)
        .map(|(id, hash, el)| {
            let tf = if matches!(el.shape, Shape::Text(_)) {
                SCALE_SHIFT
            } else {
                SHIFT
            };
            (id.clone(), *hash, tf)
        })
        .collect();

    let edits: Vec<Edit> = targets
        .iter()
        .map(|(id, hash, tf)| Edit::Copy {
            target: Seen {
                id: id.clone(),
                hash: *hash,
            },
            transform: *tf,
        })
        .collect();

    let changes = Changes {
        document: None,
        edits,
    };
    let (res, diag) = cx_run(|cx| file.apply(&changes, cx));
    let (applied, shifted) = res.unwrap_or_else(|e| panic!("{e:?}: {diag:?}"));
    // 後続のブロック定義（CDataList）のレコード番号が後ろへずれるため shifted == true が返る
    assert!(shifted);
    assert_eq!(applied.len(), targets.len());

    let out = write(&file);
    for threads in [1, 4] {
        let mut live = cx_run(|cx| JwwLive::open_with_threads(&src, threads, cx).unwrap()).0;
        let (result, live_diag) = cx_run(|cx| live.apply(&changes, cx));
        let (live_applied, live_shifted) =
            result.unwrap_or_else(|e| panic!("{e:?}: {live_diag:?}"));
        assert_eq!(live_applied, applied);
        assert_eq!(live_shifted, shifted);
        assert_eq!(
            live_diag.iter().collect::<Vec<_>>(),
            diag.iter().collect::<Vec<_>>()
        );
        assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, out);
        assert!(cx_run(|cx| live.undo(cx).unwrap()).0);
        assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, src);
        assert!(cx_run(|cx| live.redo(cx).unwrap()).0);
        assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, out);
    }
    let after = lift(&read(&out));
    let now = model_elements(&after);

    // グループ空間（space/0・space/1）の元要素は ID もハッシュもすべて不変
    for (id, hash, _) in was
        .iter()
        .filter(|(id, ..)| id.starts_with("space/0/") || id.starts_with("space/1/"))
    {
        assert!(
            now.iter().any(|(i, h, _)| i == id && h == hash),
            "changed: {id}"
        );
    }
    assert_eq!(now.len(), was.len() + applied.len());

    for (result, (src_id, _, tf)) in applied.iter().zip(&targets) {
        let (_, _, orig) = was.iter().find(|(i, ..)| i == src_id).unwrap();
        let new_id = result.id.as_deref().unwrap();
        let (_, new_hash, copied) = now.iter().find(|(i, ..)| i == new_id).unwrap();
        assert_eq!(result.hash, Some(*new_hash), "{new_id}");
        assert_eq!(copied.layer, orig.layer, "{new_id}");
        assert_eq!(copied.style, orig.style, "{new_id}");
        if !matches!(
            orig.shape,
            Shape::Text(_) | Shape::Hatch(_) | Shape::Curve(zumen_drawing::Edge::Elliptic { .. })
        ) {
            assert_eq!(
                Some(copied.shape.clone()),
                transform::transform(&orig.shape, tf),
                "{new_id}"
            );
        }
    }

    // 線（0 番目）の jww.entity 拡張（flag = 4）が写しにも保たれている
    let copied_line_id = applied[0].id.as_deref().unwrap();
    assert_eq!(
        element_extensions(&after, copied_line_id),
        element_extensions(&before, &targets[0].0)
    );

    // 文字（6 番目）の jww.moji 拡張（kind = 3, spacing は 2 倍スケール）が保たれている
    let copied_text_id = applied[6].id.as_deref().unwrap();
    let orig_moji = element_extensions(&before, &targets[6].0);
    let copied_moji = element_extensions(&after, copied_text_id);
    assert_eq!(copied_moji.len(), 1);
    assert_eq!(copied_moji[0].0, "jww.moji");
    let orig_kind = orig_moji[0].1.iter().find(|f| f.key == "kind").unwrap();
    let copied_kind = copied_moji[0].1.iter().find(|f| f.key == "kind").unwrap();
    assert_eq!(copied_kind, orig_kind);
    let FieldValue::Real(orig_sp) = orig_moji[0]
        .1
        .iter()
        .find(|f| f.key == "spacing")
        .unwrap()
        .value
    else {
        panic!()
    };
    let FieldValue::Real(copied_sp) = copied_moji[0]
        .1
        .iter()
        .find(|f| f.key == "spacing")
        .unwrap()
        .value
    else {
        panic!()
    };
    assert!((copied_sp - orig_sp * 2.0).abs() < 1e-9);

    // 2) 末尾ブロック定義（space/16）内部の要素を複写すると、全要素の ID・ハッシュが不変（!shifted）
    let mut block_file = read(&out);
    let sym_el = now
        .iter()
        .find(|(id, ..)| id.starts_with("space/16/"))
        .unwrap();
    let (block_res, block_diag) = cx_run(|cx| {
        block_file.apply(
            &Changes {
                document: None,
                edits: vec![Edit::Copy {
                    target: Seen {
                        id: sym_el.0.clone(),
                        hash: sym_el.1,
                    },
                    transform: SHIFT,
                }],
            },
            cx,
        )
    });
    let (block_applied, block_shifted) =
        block_res.unwrap_or_else(|e| panic!("{e:?}: {block_diag:?}"));
    assert!(!block_shifted);
    let after_block = lift(&read(&write(&block_file)));
    let after_block_els = model_elements(&after_block);
    for (id, hash, _) in &now {
        assert!(
            after_block_els.iter().any(|(i, h, _)| i == id && h == hash),
            "changed: {id}"
        );
    }
    let new_sym_id = block_applied[0].id.as_deref().unwrap();
    assert!(new_sym_id.starts_with("space/16/"));
    let (_, new_sym_hash, _) = after_block_els
        .iter()
        .find(|(i, ..)| i == new_sym_id)
        .unwrap();
    assert_eq!(block_applied[0].hash, Some(*new_sym_hash));
}

#[test]
fn unsupported_jww_copies_are_refused_whole_without_mutation() {
    let src = source_jww();
    let base_file = read(&src);
    let before = lift(&base_file);
    let was = model_elements(&before);
    let space0: Vec<_> = was
        .iter()
        .filter(|(id, ..)| id.starts_with("space/0/"))
        .collect();

    let check_refusal = |changes: &Changes| {
        let mut file = read(&src);
        let (res, diag) = cx_run(|cx| file.apply(changes, cx));
        assert!(res.is_err());
        let mut live = cx_run(|cx| JwwLive::open(&src, cx).unwrap()).0;
        let (refused, live_diag) = cx_run(|cx| live.apply(changes, cx));
        assert!(refused.is_err());
        assert_eq!(
            live_diag.iter().collect::<Vec<_>>(),
            diag.iter().collect::<Vec<_>>()
        );
        assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, src);
        assert!(!cx_run(|cx| live.undo(cx).unwrap()).0);
        assert_eq!(write(&file), src);
        diag
    };

    // 1. 曲線属性つき図形（index 9）をバッチに含めると全体を拒む
    {
        let changes = Changes {
            document: None,
            edits: vec![
                Edit::Copy {
                    target: Seen {
                        id: space0[0].0.clone(),
                        hash: space0[0].1,
                    },
                    transform: SHIFT,
                },
                Edit::Copy {
                    target: Seen {
                        id: space0[9].0.clone(),
                        hash: space0[9].1,
                    },
                    transform: SHIFT,
                },
            ],
        };
        let diag = check_refusal(&changes);
        assert!(diag.iter().any(|d| {
            d.code() == codes::CHANGE_COPY_UNSUPPORTED && d.param("value") == Some(&"curve".into())
        }));
    }

    // 2. 寸法（CDataSunpou, index 10）をバッチに含めると全体を拒む
    {
        let changes = Changes {
            document: None,
            edits: vec![Edit::Copy {
                target: Seen {
                    id: space0[10].0.clone(),
                    hash: space0[10].1,
                },
                transform: SHIFT,
            }],
        };
        let diag = check_refusal(&changes);
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::CHANGE_COPY_UNSUPPORTED)
        );
    }

    // 3. 同一バッチ内で先に動かした要素の複写は source = changed で拒む
    {
        let changes = Changes {
            document: None,
            edits: vec![
                Edit::Transform {
                    target: Seen {
                        id: space0[0].0.clone(),
                        hash: space0[0].1,
                    },
                    transform: SHIFT,
                },
                Edit::Copy {
                    target: Seen {
                        id: space0[0].0.clone(),
                        hash: space0[0].1,
                    },
                    transform: SHIFT,
                },
            ],
        };
        let diag = check_refusal(&changes);
        assert!(diag.iter().any(|d| {
            d.code() == codes::CHANGE_COPY_UNSUPPORTED
                && d.param("value") == Some(&"changed".into())
        }));
    }

    // 4. 異なるグループ（space/0 -> space/1）への配置は拒む
    {
        let mut file = read(&src);
        let changes = Changes {
            document: None,
            edits: vec![Edit::Copy {
                target: Seen {
                    id: space0[0].0.clone(),
                    hash: space0[0].1,
                },
                transform: SHIFT,
            }],
        };
        let (res, diag) = cx_run(|cx| {
            file.apply_with_copies(
                &changes,
                &[CopyFrom {
                    place: Some("space/1"),
                    from: None,
                }],
                cx,
            )
        });
        assert!(res.is_err());
        let mut live = cx_run(|cx| JwwLive::open(&src, cx).unwrap()).0;
        let (refused, observed) = cx_run(|cx| live.apply_with_copy_place(&changes, "space/1", cx));
        assert!(refused.is_err());
        assert_eq!(
            observed.iter().collect::<Vec<_>>(),
            diag.iter().collect::<Vec<_>>()
        );
        assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, src);
        let (missing, observed) =
            cx_run(|cx| live.apply_with_copy_place(&changes, "space/999999", cx));
        assert!(missing.is_err());
        assert!(
            observed
                .iter()
                .any(|d| d.code() == codes::CHANGE_ELEMENT_MISSING)
        );
        assert!(
            cx_run(|cx| live.apply_with_copy_place(&changes, "space/0", cx))
                .0
                .is_ok()
        );

        assert!(diag.iter().any(|d| {
            d.code() == codes::CHANGE_COPY_UNSUPPORTED
                && d.param("value") == Some(&"group_mismatch".into())
        }));
        assert_eq!(write(&file), src);
    }
}

#[test]
fn cross_document_jww_copy_and_budget_stop_atomicity() {
    let src = source_jww();
    let src_file = read(&src);
    let before = lift(&src_file);
    let was = model_elements(&before);
    let line = &was[0];
    let block_ref = &was[8];

    // 同じヘッダー・ブロック定義を持つ別文書へは複写できる
    let mut dst_ok = read(&src);
    let (res, diag) = cx_run(|cx| {
        dst_ok.apply_with_copies(
            &Changes {
                document: None,
                edits: vec![
                    Edit::Copy {
                        target: Seen {
                            id: line.0.clone(),
                            hash: line.1,
                        },
                        transform: SHIFT,
                    },
                    Edit::Copy {
                        target: Seen {
                            id: block_ref.0.clone(),
                            hash: block_ref.1,
                        },
                        transform: SHIFT,
                    },
                ],
            },
            &[
                CopyFrom {
                    place: None,
                    from: Some(&src_file),
                },
                CopyFrom {
                    place: None,
                    from: Some(&src_file),
                },
            ],
            cx,
        )
    });
    let (applied, _) = res.unwrap_or_else(|e| panic!("{e:?}: {diag:?}"));
    assert_eq!(applied.len(), 2);

    // グループの縮尺が違う別文書へは scale = mismatch で全体を拒む
    let mut diff_spec = Spec::new(700);
    let mut layers = [0; 16];
    layers[0] = 3;
    diff_spec.groups[0] = (3, 200.0, layers);
    let mut diff_arc = Archive::new(700);
    diff_arc.layer = 0;
    diff_arc.group = 0;
    diff_arc.count(1);
    diff_arc.line();
    diff_arc.count(0);
    diff_arc.out.extend(0u32.to_le_bytes());
    let mut diff_bytes = diff_spec.build();
    diff_bytes.extend(&diff_arc.out);
    let mut dst_bad = read(&diff_bytes);
    let (res, diag) = cx_run(|cx| {
        dst_bad.apply_with_copies(
            &Changes {
                document: None,
                edits: vec![Edit::Copy {
                    target: Seen {
                        id: line.0.clone(),
                        hash: line.1,
                    },
                    transform: SHIFT,
                }],
            },
            &[CopyFrom {
                place: None,
                from: Some(&src_file),
            }],
            cx,
        )
    });
    assert!(res.is_err());
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::CHANGE_COPY_UNSUPPORTED)
    );
    assert_eq!(write(&dst_bad), diff_bytes);

    // 予算停止時の原子性: どこで止まっても元のバイト列を保つ
    let run = |limit: Option<u64>, resident: bool| -> (bool, u64, Vec<u8>) {
        let edits = vec![
            Edit::Copy {
                target: Seen {
                    id: line.0.clone(),
                    hash: line.1,
                },
                transform: SHIFT,
            },
            Edit::Copy {
                target: Seen {
                    id: block_ref.0.clone(),
                    hash: block_ref.1,
                },
                transform: SHIFT,
            },
        ];
        let limits = match limit {
            Some(w) => Limits::default().with(Kind::Work, Limit::Finite(w)),
            None => Limits::default(),
        };
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let changes = Changes {
            document: None,
            edits,
        };
        let cancel = CancelToken::new();
        let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
        let (ok, bytes) = if resident {
            let mut live = cx_run(|cx| JwwLive::open(&src, cx).unwrap()).0;
            let ok = live.apply(&changes, cx).is_ok();
            (ok, cx_run(|cx| live.save(cx).unwrap()).0)
        } else {
            let mut file = read(&src);
            let ok = file.apply(&changes, cx).is_ok();
            (ok, write(&file))
        };
        (ok, budget.used(Kind::Work), bytes)
    };
    for resident in [false, true] {
        let (done, used, _) = run(None, resident);
        assert!(done);
        for limit in [used / 4, used / 2, used.saturating_sub(1)] {
            let (done, _, written) = run(Some(limit), resident);
            assert!(!done, "{limit}");
            assert_eq!(written, src, "{limit}");
        }
    }
}
