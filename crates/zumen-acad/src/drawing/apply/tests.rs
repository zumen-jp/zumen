//! 書き戻しの索引・予算・中断・文書のハッシュの試験。

mod duplicate_handle_tests {
    use super::super::*;
    use crate::db::Origin;
    use zumen_base::{Budget, CancelToken, Kind, Limit, Limits};

    #[test]
    fn attribute_counts_are_indexed_once_and_obey_work_and_cancellation() {
        let mut db = AcadDb::default();
        for (section, kind, values) in [
            (
                Section::Entities,
                b"INSERT".as_slice(),
                vec![
                    Value {
                        code: 5,
                        data: Data::Handle(10),
                    },
                    Value {
                        code: 66,
                        data: Data::Int(1),
                    },
                ],
            ),
            (Section::Entities, b"ATTRIB".as_slice(), vec![]),
            (Section::Entities, b"SEQEND".as_slice(), vec![]),
            (
                Section::Blocks,
                b"INSERT".as_slice(),
                vec![Value {
                    code: 5,
                    data: Data::Handle(20),
                }],
            ),
            (
                Section::Blocks,
                b"ATTRIB".as_slice(),
                vec![Value {
                    code: 330,
                    data: Data::Handle(20),
                }],
            ),
        ] {
            db.push(Object {
                kind: kind.to_vec(),
                section,
                values,
                origin: Origin::New,
            });
        }
        let at = Ref::default();
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let counts =
            attribute_counts_for(&db, &at, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
        assert_eq!(counts.get(&0), Some(&1));
        assert_eq!(counts.get(&3), Some(&1));
        let spent = budget.used(Kind::Work);
        for _ in 0..100 {
            assert_eq!(counts.get(&0), Some(&1));
            assert_eq!(counts.get(&3), Some(&1));
        }
        assert_eq!(budget.used(Kind::Work), spent);
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        assert!(matches!(
            attribute_counts_for(&db, &at, &mut Cx::new(&mut budget, &mut diag, &cancel)),
            Err(Error::Stopped(_))
        ));
        let mut budget = Budget::default();
        cancel.cancel();
        assert!(matches!(
            attribute_counts_for(&db, &at, &mut Cx::new(&mut budget, &mut diag, &cancel)),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 0);
    }

    #[test]
    fn checking_handles_obeys_work_budget_and_cancellation() {
        let mut db = AcadDb::default();
        for handle in [1, 2] {
            db.push(Object {
                kind: b"LINE".to_vec(),
                section: Section::Entities,
                values: vec![Value {
                    code: 5,
                    data: Data::Handle(handle),
                }],
                origin: Origin::New,
            });
        }
        let changes = Changes {
            edits: vec![],
            document: None,
        };
        let before = db.clone();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        assert!(matches!(
            edit(
                &mut db,
                &changes,
                Target::Dxf,
                &mut Cx::new(&mut budget, &mut diag, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 1);
        assert_eq!(db, before);
        let mut budget = Budget::default();
        cancel.cancel();
        assert!(matches!(
            edit(
                &mut db,
                &changes,
                Target::Dxf,
                &mut Cx::new(&mut budget, &mut diag, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 0);
        assert_eq!(db, before);
    }
}

mod hash_budget_tests {
    use super::super::*;
    use zumen_base::{Budget, CancelToken, Key, Kind, Limit, Limits};
    use zumen_drawing::{Axes, LayerId, ModelSpace, Style, Unit, Vertex};

    #[test]
    fn stopped_apply_does_not_change_dxf() {
        use zumen_drawing::Seen;
        use zumen_sdk::Lift;
        let src = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n5\n2F\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDSEC\n0\nEOF\n";
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let mut cx = context(&mut budget, &mut diagnostics, &cancel);
        let file = Dxf::read(src, &mut cx).unwrap();
        let drawing: Drawing2D = file.lift(&mut cx).unwrap();
        let space = &drawing.spaces[0];
        let element = &space.elements[0];
        let place = id::space(space.id);
        let target = Seen {
            id: id::Namer::new(&place).next(0, element),
            hash: Hash::element(element),
        };
        let changes = Changes {
            edits: vec![Edit::Transform {
                target,
                transform: Affine {
                    m: [1.0, 0.0, 5.0, 0.0, 1.0, 7.0],
                },
            }],
            document: None,
        };
        let mut failed = 0;
        let mut succeeded = false;
        for limit in 0..500 {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let mut file =
                Dxf::read(src, &mut context(&mut budget, &mut diagnostics, &cancel)).unwrap();
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(limit)));
            if file
                .apply(
                    &changes,
                    &mut context(&mut budget, &mut diagnostics, &cancel),
                )
                .is_ok()
            {
                succeeded = true;
                break;
            }
            failed += 1;
            let mut bytes = Vec::new();
            let mut budget = Budget::default();
            Dxf::write(
                &file,
                &mut bytes,
                &mut context(&mut budget, &mut diagnostics, &cancel),
            )
            .unwrap();
            assert_eq!(bytes, src, "work={limit}");
        }
        assert!(failed > 0 && succeeded);
    }

    #[test]
    fn indexing_symbol_definitions_and_elements_counts_work() {
        use zumen_drawing::{SymbolDefinition, SymbolKey};
        let point = ModelPoint {
            space: SpaceId(1),
            x: 0.0,
            y: 0.0,
        };
        let definition = SymbolDefinition {
            key: SymbolKey {
                namespace: "dxf.block".into(),
                name: "B".to_owned(),
            },
            space: ModelSpace {
                id: SpaceId(1),
                unit: Unit::Unknown,
                axes: Axes::EastNorth,
                elements: vec![Element {
                    source: Ref::at_record(2),
                    layer: LayerId(0),
                    style: Style::BLOCK,
                    shape: Shape::Line {
                        start: point,
                        end: point,
                    },
                }],
                order: vec![0],
            },
            base: None,
            attributes: vec![],
            anonymous: false,
            external: None,
            source: Ref::at_record(1),
        };
        let drawing = Drawing2D {
            symbols: vec![definition],
            ..Drawing2D::default()
        };
        let cancel = CancelToken::new();
        for limit in [0, 1] {
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(limit)));
            let mut diagnostics = Diagnostics::new();
            assert!(matches!(
                Index::new(
                    &drawing,
                    &mut context(&mut budget, &mut diagnostics, &cancel)
                ),
                Err(Error::Stopped(_))
            ));
            assert_eq!(budget.used(Kind::Work), limit);
        }
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(2)));
        let mut diagnostics = Diagnostics::new();
        assert!(
            Index::new(
                &drawing,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            )
            .is_ok()
        );
        assert_eq!(budget.used(Kind::Work), 2);
    }

    fn context<'a>(
        budget: &'a mut Budget,
        diagnostics: &'a mut Diagnostics,
        cancel: &'a CancelToken,
    ) -> Cx<'a> {
        Cx::new(budget, diagnostics, cancel)
    }

    #[test]
    fn indexes_sheets_with_the_same_names_as_entities() {
        use zumen_drawing::{PaperPoint, Sheet};
        let sheets = (0..2)
            .map(|number| Sheet {
                id: SheetId(number),
                name: None,
                size: None,
                elements: vec![Element {
                    source: Ref::at_record(u64::from(number) + 10)
                        .with_key(Key::Handle(u64::from(number) + 10)),
                    layer: LayerId(0),
                    style: Style::BLOCK,
                    shape: Shape::Line {
                        start: PaperPoint {
                            sheet: SheetId(number),
                            x: 0.0,
                            y: 0.0,
                        },
                        end: PaperPoint {
                            sheet: SheetId(number),
                            x: 1.0,
                            y: 1.0,
                        },
                    },
                }],
                views: vec![],
                order: vec![],
                source: Ref::at_record(u64::from(number) + 20).with_key(Key::Handle(5)),
            })
            .collect();
        let drawing = Drawing2D {
            sheets,
            ..Drawing2D::default()
        };
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let index = Index::new(
            &drawing,
            &mut context(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        assert!(matches!(
            index.places.get("sheet/h5"),
            Some(Place::Sheet(SheetId(0)))
        ));
        assert!(matches!(
            index.places.get("sheet/h5/1"),
            Some(Place::Sheet(SheetId(1)))
        ));
        assert!(index.elements.contains_key("sheet/h5/hA"));
        assert!(index.elements.contains_key("sheet/h5/1/hB"));
    }

    #[test]
    fn checks_element_hashes_with_work_and_cancellation() {
        let point = ModelPoint {
            space: SpaceId(0),
            x: 0.0,
            y: 0.0,
        };
        let element = Element {
            source: Ref::at_record(1).with_key(Key::Handle(1)),
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Polyline(Polyline {
                vertices: vec![Vertex { point, bulge: 0.0 }; 100],
                closed: false,
                width: None,
                vertex_widths: vec![],
                line_type_generation: false,
            }),
        };
        let drawing = Drawing2D {
            spaces: vec![ModelSpace {
                id: SpaceId(0),
                unit: Unit::Unknown,
                axes: Axes::EastNorth,
                elements: vec![element],
                order: vec![0],
            }],
            ..Drawing2D::default()
        };
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        assert!(matches!(
            Index::new(
                &drawing,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        let mut budget = Budget::default();
        let index = Index::new(
            &drawing,
            &mut context(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        diagnostics = Diagnostics::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(10)));
        let result = index.hash(
            "space/0/h1",
            &mut context(&mut budget, &mut diagnostics, &cancel),
        );
        assert!(matches!(result, Err(Error::Stopped(_))));
        assert_eq!(
            diagnostics.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
        let mut budget = Budget::default();
        let actual = applied(
            &index,
            Some("space/0/h1".into()),
            &mut context(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        assert_eq!(
            actual.hash,
            Some(Hash::element(&drawing.spaces[0].elements[0]))
        );
        cancel.cancel();
        let mut budget = Budget::default();
        assert!(matches!(
            applied(
                &index,
                Some("space/0/h1".into()),
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
    }

    #[test]
    fn element_handle_index_rejects_ambiguous_sources() {
        let point = ModelPoint {
            space: MODEL,
            x: 0.0,
            y: 0.0,
        };
        let element = Element {
            source: Ref::at_record(7).with_key(Key::Handle(42)),
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Line {
                start: point,
                end: point,
            },
        };
        let drawing = Drawing2D {
            spaces: vec![ModelSpace {
                id: MODEL,
                unit: Unit::Unknown,
                axes: Axes::EastNorth,
                elements: vec![element.clone(), element],
                order: vec![0, 1],
            }],
            ..Drawing2D::default()
        };
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let index = Index::new(
            &drawing,
            &mut context(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        assert_eq!(index.element_handles.get(&42), Some(&None));
        assert!(index.elements.contains_key("space/0/h2A"));
        assert!(index.elements.contains_key("space/0/h2A/1"));
    }

    #[test]
    fn checks_document_hash_with_work_budget() {
        struct TestFile(Vec<u8>);
        fn write(file: &TestFile, out: &mut Vec<u8>, _: &mut Cx<'_>) -> Result<(), Error> {
            out.extend_from_slice(&file.0);
            Ok(())
        }
        let bytes = vec![42; 4096];
        let changes = Changes {
            edits: vec![],
            document: Some(Hash::document(&bytes)),
        };
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(2)));
        assert!(matches!(
            document(
                &TestFile(bytes),
                &changes,
                write,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(
            diagnostics.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
    }
}

mod definition_id_tests {
    use super::super::*;
    use zumen_base::Key;
    use zumen_drawing::{Axes, ModelSpace, SymbolDefinition, SymbolKey, Unit};

    #[test]
    fn duplicate_definition_handles_have_distinct_index_entries() {
        let mut drawing = Drawing2D::default();
        for number in 0..2 {
            let space = SpaceId(number);
            let point = ModelPoint {
                space,
                x: 0.0,
                y: 0.0,
            };
            let element = Element {
                source: Ref::at_record(u64::from(number) + 10).with_key(Key::Handle(0x31)),
                layer: LayerId(0),
                style: Style::BLOCK,
                shape: Shape::Line {
                    start: point,
                    end: point,
                },
            };
            drawing.symbols.push(SymbolDefinition {
                key: SymbolKey {
                    namespace: "dxf.block".into(),
                    name: format!("B{number}"),
                },
                space: ModelSpace {
                    id: space,
                    unit: Unit::Millimeter,
                    axes: Axes::EastNorth,
                    elements: vec![element],
                    order: vec![],
                },
                base: None,
                attributes: vec![],
                anonymous: false,
                external: None,
                source: Ref::at_record(u64::from(number)).with_key(Key::Handle(0x30)),
            });
        }
        let mut budget = zumen_base::Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = zumen_base::CancelToken::new();
        let index = Index::new(
            &drawing,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        assert!(index.elements.contains_key("space/h30/h31"));
        assert!(index.elements.contains_key("space/h30/1/h31"));
        assert_eq!(index.elements.len(), 2);
    }
}

mod index_tests {
    use super::super::*;
    use zumen_base::{Budget, CancelToken};
    use zumen_drawing::{Axes, ModelSpace, Sheet, SymbolDefinition, SymbolKey, Unit};

    fn line<P: Copy>(start: P, end: P, source: Ref) -> Element<P> {
        Element {
            source,
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Line { start, end },
        }
    }

    /// 書き戻しの索引は、形式固有の情報の付く先と同じ ID を要素に振る（決定 0156）
    #[test]
    fn index_names_elements_like_the_drawing_model() {
        let at = |space, x| ModelPoint { space, x, y: 0.0 };
        let on = |x| PaperPoint {
            sheet: SheetId(1),
            x,
            y: 0.0,
        };
        let keyed = |record, handle| Ref::at_record(record).with_key(Key::Handle(handle));
        let space = |id, elements| ModelSpace {
            id,
            unit: Unit::Millimeter,
            axes: Axes::EastNorth,
            elements,
            order: Vec::new(),
        };
        let symbol = |name: &str, id, elements, source| SymbolDefinition {
            key: SymbolKey {
                namespace: "dxf.block".into(),
                name: name.to_owned(),
            },
            space: space(id, elements),
            base: None,
            attributes: Vec::new(),
            anonymous: false,
            external: None,
            source,
        };
        let model = vec![
            line(at(MODEL, 0.0), at(MODEL, 1.0), keyed(3, 0x2F)),
            line(at(MODEL, 1.0), at(MODEL, 2.0), keyed(4, 0x2F)),
            line(at(MODEL, 2.0), at(MODEL, 3.0), Ref::at_record(5)),
            line(at(MODEL, 3.0), at(MODEL, 4.0), Ref::default()),
        ];
        let door = SpaceId(2);
        let window = SpaceId(3);
        let drawing = Drawing2D {
            spaces: vec![space(MODEL, model)],
            symbols: vec![
                symbol(
                    "DOOR",
                    door,
                    vec![line(at(door, 0.0), at(door, 1.0), keyed(8, 0x40))],
                    keyed(7, 0x1F),
                ),
                symbol(
                    "WINDOW",
                    window,
                    vec![line(at(window, 0.0), at(window, 1.0), Ref::at_record(10))],
                    keyed(9, 0x1F),
                ),
            ],
            sheets: vec![Sheet {
                id: SheetId(1),
                name: None,
                size: None,
                elements: vec![line(on(0.0), on(1.0), keyed(12, 0x50))],
                views: Vec::new(),
                order: Vec::new(),
                source: keyed(11, 0x22),
            }],
            ..Drawing2D::default()
        };
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let index = Index::new(&drawing, &mut cx).unwrap();
        let mut named = Vec::new();
        let _ = id::each_element(&drawing, |_, id, found| {
            named.push((id, found.hash()));
            Ok::<(), ()>(())
        });
        let indexed: Vec<_> = index
            .elements
            .iter()
            .map(|(id, found)| {
                let hash = match found {
                    Found::Model(element) => Hash::element(element),
                    Found::Paper(element) => Hash::element(element),
                };
                (id.clone(), hash)
            })
            .collect();
        named.sort();
        assert_eq!(indexed, named);
        let ids: Vec<_> = named.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "sheet/h22/h50",
                "space/0/e3",
                "space/0/h2F",
                "space/0/h2F/1",
                "space/0/r5",
                "space/h1F/1/r10",
                "space/h1F/h40",
            ]
        );
    }
}
