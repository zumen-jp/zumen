//! 公開・合成の PLY を独立した ply-rs の読みと照らす（#809）。

#![cfg(test)]

use std::io::Cursor;

use ply_rs::{parser::Parser, ply::DefaultElement, ply::Property as ExternalProperty};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, codes};
use zumen_ply::{Encoding, Ply, PlyFile};
use zumen_pointcloud::PointCloud;
use zumen_sdk::{Codec, Error, Lift, testkit};

const PUBLIC_SAMPLE: &[u8] = include_bytes!("../../../samples/ply/pdal-mesh-fixed.ply");

fn open(bytes: &[u8]) -> (PlyFile, PointCloud, Vec<Diagnostic>) {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let file = Ply::read(bytes, &mut cx).unwrap();
    let cloud = file.lift(&mut cx).unwrap();
    (file, cloud, diagnostics.into_vec())
}

fn number(value: &ExternalProperty) -> f64 {
    match value {
        ExternalProperty::Char(value) => f64::from(*value),
        ExternalProperty::UChar(value) => f64::from(*value),
        ExternalProperty::Short(value) => f64::from(*value),
        ExternalProperty::UShort(value) => f64::from(*value),
        ExternalProperty::Int(value) => f64::from(*value),
        ExternalProperty::UInt(value) => f64::from(*value),
        ExternalProperty::Float(value) => f64::from(*value),
        ExternalProperty::Double(value) => *value,
        _ => panic!("比較する列がスカラーでない"),
    }
}

fn compare(bytes: &[u8]) {
    let external = Parser::<DefaultElement>::new()
        .read_ply(&mut Cursor::new(bytes))
        .unwrap();
    let (file, cloud, _) = open(bytes);
    let vertices = &external.payload["vertex"];
    let ours = file
        .elements
        .iter()
        .find(|item| item.name == "vertex")
        .unwrap();
    assert_eq!(ours.count as usize, vertices.len());
    assert_eq!(cloud.points.len(), vertices.len());
    let columns: Vec<_> = ours
        .properties
        .iter()
        .filter(|p| p.list.is_none())
        .collect();
    for (index, vertex) in vertices.iter().enumerate() {
        for (column, property) in columns.iter().enumerate() {
            assert_eq!(
                ours.values[index * columns.len() + column].to_bits(),
                number(&vertex[&property.name]).to_bits(),
                "L1 の {index} 番目の {}",
                property.name
            );
        }
        for (name, values) in [
            ("x", &cloud.points.easting),
            ("y", &cloud.points.northing),
            ("z", &cloud.points.height),
        ] {
            assert_eq!(
                values[index].to_bits(),
                number(&vertex[name]).to_bits(),
                "{name} {index}"
            );
        }
        if let Some(colors) = &cloud.points.color {
            assert_eq!(
                colors[index],
                ["red", "green", "blue"].map(|name| number(&vertex[name]) as u16 * 257)
            );
        }
        if let Some(intensities) = &cloud.points.intensity {
            assert_eq!(intensities[index], number(&vertex["intensity"]) as u16);
        }
        if let Some(classes) = &cloud.points.classification {
            assert_eq!(classes[index], number(&vertex["classification"]) as u8);
        }
    }
}

#[test]
fn public_pdal_sample_matches_external_vertices_and_reports_faces() {
    compare(PUBLIC_SAMPLE);
    let (file, cloud, diagnostics) = open(PUBLIC_SAMPLE);
    assert_eq!(file.encoding, Encoding::Ascii);
    assert_eq!(cloud.points.easting, [1.0, 2.0, 1.0, 2.0]);
    assert_eq!(cloud.points.northing, [1.0, 1.0, 2.0, 2.0]);
    assert_eq!(cloud.points.height, [0.0, 0.0, 0.0, 2.0]);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code(), codes::PLY_ELEMENT_DROPPED);
    assert_eq!(diagnostics[0].param("element"), Some(&"face".into()));
    assert_eq!(diagnostics[0].param("found"), Some(&2_u64.into()));
    assert_eq!(diagnostics[1].code(), codes::PLY_CRS_MISSING);
    assert!(cloud.crs.is_none());
    testkit::records_in_order::<Ply>(PUBLIC_SAMPLE).unwrap();
    testkit::deterministic::<Ply>(PUBLIC_SAMPLE).unwrap();
    testkit::stops_at_same_place::<Ply>(PUBLIC_SAMPLE).unwrap();
    testkit::survives_lift::<Ply, PointCloud>(PUBLIC_SAMPLE).unwrap();
}

fn fixture(encoding: Encoding) -> Vec<u8> {
    let name = match encoding {
        Encoding::Ascii => "ascii",
        Encoding::Little => "binary_little_endian",
        Encoding::Big => "binary_big_endian",
    };
    let mut bytes = format!("ply\nformat {name} 1.0\nelement vertex 2\nproperty double x\nproperty double y\nproperty double z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nproperty ushort intensity\nproperty uchar classification\nproperty float nx\nend_header\n").into_bytes();
    for (point, color, intensity, class) in [
        ([-12.25_f64, 0.0, 34.125], [0_u8, 128, 255], 1234_u16, 2_u8),
        ([7654.5, -321.75, 0.5], [255, 1, 10], 65535, 31),
    ] {
        if encoding == Encoding::Ascii {
            bytes.extend_from_slice(
                format!(
                    "{} {} {} {} {} {} {intensity} {class} 0.25\n",
                    point[0], point[1], point[2], color[0], color[1], color[2]
                )
                .as_bytes(),
            );
        } else {
            for value in point {
                bytes.extend(if encoding == Encoding::Big {
                    value.to_be_bytes()
                } else {
                    value.to_le_bytes()
                });
            }
            bytes.extend(color);
            bytes.extend(if encoding == Encoding::Big {
                intensity.to_be_bytes()
            } else {
                intensity.to_le_bytes()
            });
            bytes.push(class);
            bytes.extend(if encoding == Encoding::Big {
                0.25_f32.to_be_bytes()
            } else {
                0.25_f32.to_le_bytes()
            });
        }
    }
    bytes
}

#[test]
fn compares_synthetic_positions_and_attributes_in_all_encodings() {
    for encoding in [Encoding::Ascii, Encoding::Little, Encoding::Big] {
        let bytes = fixture(encoding);
        compare(&bytes);
        let (_, _, diagnostics) = open(&bytes);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code(), codes::PLY_PROPERTY_DROPPED);
        assert_eq!(diagnostics[0].param("property"), Some(&"nx".into()));
        assert_eq!(diagnostics[1].code(), codes::PLY_CRS_MISSING);
    }
}

#[test]
fn public_sample_stops_at_input_elements_and_work_budgets() {
    for kind in [Kind::InputBytes, Kind::Elements, Kind::Work] {
        let run = || {
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(1)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let result = Ply::read(PUBLIC_SAMPLE, &mut cx).and_then(|file| file.lift(&mut cx));
            let Err(Error::Stopped(stop)) = result else {
                panic!("予算で止まらない")
            };
            assert_eq!(stop.to_diagnostic().code(), kind.exceeded_code());
            (
                stop,
                diagnostics.into_vec(),
                Kind::ALL.map(|item| budget.used(item)),
            )
        };
        assert_eq!(run(), run());
    }
}

#[test]
fn compares_ascii_float32_positions_with_external_precision() {
    for scalar in ["float", "float32"] {
        for words in [
            ["0.1", "-0.2", "0.3"],
            ["-0", "1.000000059604644775390625", "1e-30"],
        ] {
            let expected = words.map(|word| word.parse::<f32>().unwrap());
            for encoding in [Encoding::Ascii, Encoding::Little, Encoding::Big] {
                let name = match encoding {
                    Encoding::Ascii => "ascii",
                    Encoding::Little => "binary_little_endian",
                    Encoding::Big => "binary_big_endian",
                };
                let mut bytes = format!("ply\nformat {name} 1.0\nelement vertex 1\nproperty {scalar} x\nproperty {scalar} y\nproperty {scalar} z\nend_header\n").into_bytes();
                if encoding == Encoding::Ascii {
                    bytes.extend_from_slice(
                        format!("{} {} {}\n", words[0], words[1], words[2]).as_bytes(),
                    );
                } else {
                    for value in expected {
                        bytes.extend(if encoding == Encoding::Big {
                            value.to_be_bytes()
                        } else {
                            value.to_le_bytes()
                        });
                    }
                }
                compare(&bytes);
                let (_, cloud, _) = open(&bytes);
                assert_eq!(
                    [
                        cloud.points.easting[0],
                        cloud.points.northing[0],
                        cloud.points.height[0]
                    ]
                    .map(f64::to_bits),
                    expected.map(|value| f64::from(value).to_bits())
                );
            }
        }
    }
}
