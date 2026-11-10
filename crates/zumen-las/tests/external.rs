//! 非圧縮 LAS の読みを laspy と照らす（#804）。外部ツールと受領図面は同梱しない。
//! LAS 1.4 R15 の Public Header Block と Point Data Record Format 0〜10。

#![cfg(test)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, codes};
use zumen_las::{Las, LasFile};
use zumen_pointcloud::PointCloud;
use zumen_sdk::ranged::{Missing, Ranged, Source};
use zumen_sdk::{Codec, Error, Lift, testkit};

// PDAL が作った、分類の synthetic フラグを持つ 1 点。来歴と権利は samples/ledger.toml。
const PUBLIC_SAMPLE: &[u8] = include_bytes!("../../../samples/las/pdal-synthetic.las");

// コアの base_length を使わず、仕様の各点形式の長さでファイルを作る。
const LENGTHS: [u16; 11] = [20, 28, 26, 34, 57, 63, 30, 36, 38, 59, 67];
const POSITIONS: [[f64; 3]; 4] = [
    [75.0, -100.0, -7.5],
    [100.0, -202.5, 36.25],
    [350.0, -700.0, 92.5],
    [101.75, -194.5, 31.625],
];
const INTENSITIES: [u16; 4] = [1, 257, 65535, 0];
const TIMES: [f64; 4] = [0.0, 12.5, -1.0, 1_000_000_000.25];
const COLORS: [[u16; 3]; 4] = [[1, 256, 65535], [0, 0, 0], [65535, 7, 99], [17, 19, 23]];

fn fixture(format: u8) -> Vec<u8> {
    let minor = match format {
        0..=3 => 2,
        4..=5 => 3,
        _ => 4,
    };
    let size: u16 = match minor {
        2 => 227,
        3 => 235,
        _ => 375,
    };
    let modern = format >= 6;
    let mut vlr = vec![0; 54];
    vlr[2..17].copy_from_slice(b"LASF_Projection");
    let (id, data): (u16, Vec<u8>) = if modern {
        (
            2112,
            b"LOCAL_CS[\"zumen synthetic\",LOCAL_DATUM[\"local\",0],UNIT[\"metre\",1],AXIS[\"Easting\",EAST],AXIS[\"Northing\",NORTH],AXIS[\"Height\",UP]]\0"
                .to_vec(),
        )
    } else {
        (
            34735,
            [1u16, 1, 0, 1, 3072, 0, 1, 6677]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect(),
        )
    };
    vlr[18..20].copy_from_slice(&id.to_le_bytes());
    vlr[20..22].copy_from_slice(&u16::try_from(data.len()).unwrap().to_le_bytes());
    vlr.extend(data);
    let length = LENGTHS[usize::from(format)];
    let mut bytes = vec![0; usize::from(size)];
    bytes[..4].copy_from_slice(b"LASF");
    // 時刻は Adjusted Standard GPS Time。1.4 の新しい形式は WKT のフラグも立てる。
    bytes[6..8].copy_from_slice(&(if modern { 17u16 } else { 1 }).to_le_bytes());
    bytes[24..26].copy_from_slice(&[1, minor]);
    bytes[94..96].copy_from_slice(&size.to_le_bytes());
    bytes[96..100]
        .copy_from_slice(&(u32::from(size) + u32::try_from(vlr.len()).unwrap()).to_le_bytes());
    bytes[100..104].copy_from_slice(&1u32.to_le_bytes());
    bytes[104] = format;
    bytes[105..107].copy_from_slice(&length.to_le_bytes());
    // 1.4 の新しい点形式は 64 ビットの件数だけを持つ。
    if !modern {
        bytes[107..111].copy_from_slice(&4u32.to_le_bytes());
        for (index, count) in [2u32, 1, 0, 0, 1].into_iter().enumerate() {
            bytes[111 + index * 4..115 + index * 4].copy_from_slice(&count.to_le_bytes());
        }
    }
    for (at, value) in [
        (131, 0.25f64),
        (139, 0.5),
        (147, 0.125),
        (155, 100.0),
        (163, -200.0),
        (171, 30.0),
        (179, 350.0),
        (187, 75.0),
        (195, -100.0),
        (203, -700.0),
        (211, 92.5),
        (219, -7.5),
    ] {
        bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }
    if modern {
        bytes[247..255].copy_from_slice(&4u64.to_le_bytes());
        for (index, count) in [2u64, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1]
            .into_iter()
            .enumerate()
        {
            bytes[255 + index * 8..263 + index * 8].copy_from_slice(&count.to_le_bytes());
        }
    }
    bytes.extend(vlr);
    for (index, raw) in [
        [-100i32, 200, -300],
        [0, -5, 50],
        [1000, -1000, 500],
        [7, 11, 13],
    ]
    .into_iter()
    .enumerate()
    {
        let mut point = vec![0; usize::from(length)];
        for (axis, value) in raw.into_iter().enumerate() {
            point[axis * 4..axis * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        point[12..14].copy_from_slice(&INTENSITIES[index].to_le_bytes());
        if modern {
            point[14] = [0x21, 0x98, 0xff, 0x11][index];
            point[15] = 0xff;
            point[16] = [2, 64, 200, 255][index];
        } else {
            point[14] = [0x11, 0x12, 0x2d, 0x09][index] | 0xc0;
            point[15] = [2, 6, 31, 0][index] | 0xe0;
        }
        let time = match format {
            1 | 3..=5 => Some(20),
            6..=10 => Some(22),
            _ => None,
        };
        if let Some(at) = time {
            point[at..at + 8].copy_from_slice(&TIMES[index].to_le_bytes());
        }
        let color = match format {
            2 => Some(20),
            3 | 5 => Some(28),
            7 | 8 | 10 => Some(30),
            _ => None,
        };
        if let Some(at) = color {
            for (axis, value) in COLORS[index].into_iter().enumerate() {
                point[at + axis * 2..at + axis * 2 + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        bytes.extend(point);
    }
    bytes
}

fn open(bytes: &[u8], ranged: bool) -> (LasFile, PointCloud, Vec<Diagnostic>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = if ranged {
        let mut parts = vec![(0, &bytes[..100])];
        let mut file = None;
        for _ in 0..4 {
            match Las::read_ranged(
                &Source {
                    size: bytes.len() as u64,
                    parts: &parts,
                },
                &mut cx,
            ) {
                Ok(found) => {
                    file = Some(found);
                    break;
                }
                Err(Missing::Need(needs)) => {
                    for range in needs {
                        parts.push((
                            range.start,
                            &bytes[range.start as usize..range.end as usize],
                        ));
                    }
                }
                Err(error) => panic!("範囲で読めない: {error:?}"),
            }
        }
        file.expect("4 回以内に範囲の読みが終わらない")
    } else {
        Las::read(bytes, &mut cx).unwrap()
    };
    let cloud = file.lift(&mut cx).unwrap();
    (file, cloud, diag.into_vec())
}

// コアの決定性とは別に、テストのツールの起動条件を読む。
#[allow(clippy::disallowed_methods, clippy::print_stderr)]
fn oracle() -> Option<(PathBuf, PathBuf)> {
    let explicit = std::env::var_os("ZUMEN_LASPY_PYTHON");
    let python = explicit
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| "python".into());
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../xtask/verify/laspy/read.py");
    let result = Command::new(&python)
        .arg("-I")
        .arg(&script)
        .arg("--check")
        .output();
    let missing = match &result {
        Ok(output) if output.status.success() => {
            assert_eq!(
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .collect::<Vec<_>>(),
                ["laspy\t2.7.0", "numpy\t2.5.4"]
            );
            return Some((python, script));
        }
        Ok(output) => output.status.code() == Some(77),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    };
    assert!(missing, "laspy の起動に失敗: {result:?}");
    assert!(
        explicit.is_none() && std::env::var_os("ZUMEN_LASPY_REQUIRED").is_none(),
        "laspy が必要だが、入っていない"
    );
    eprintln!("laspy がないため外部の照合を飛ばす（xtask/verify/laspy/README.md）");
    None
}

fn external(oracle: &(PathBuf, PathBuf), bytes: &[u8]) -> String {
    let mut child = Command::new(&oracle.0)
        .arg("-I")
        .arg(&oracle.1)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "laspy が読めなかった: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn row<'a>(lines: &mut impl Iterator<Item = &'a str>, name: &str) -> Vec<&'a str> {
    let mut values = lines.next().expect("laspy の行が足りない").split('\t');
    assert_eq!(values.next(), Some(name));
    values.collect()
}

fn floats(values: &[&str]) -> Vec<u64> {
    values
        .iter()
        .map(|value| value.parse::<f64>().unwrap().to_bits())
        .collect()
}

fn compare(file: &LasFile, cloud: &PointCloud, output: &str) {
    let mut lines = output.lines();
    let header = &file.header;
    assert_eq!(
        row(&mut lines, "header")
            .iter()
            .map(|value| value.parse::<u64>().unwrap())
            .collect::<Vec<_>>(),
        [
            u64::from(header.version.0),
            u64::from(header.version.1),
            u64::from(header.point_format),
            u64::from(header.record_length),
            header.point_count,
        ]
    );
    for (name, expected) in [
        ("scale", header.scale.to_vec()),
        ("offset", header.offset.to_vec()),
        ("bounds", header.bounds.into_iter().flatten().collect()),
    ] {
        assert_eq!(
            floats(&row(&mut lines, name)),
            expected
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            "{name}"
        );
    }
    let points = &cloud.points;
    assert_eq!(points.len() as u64, header.point_count);
    for index in 0..points.len() {
        let values = row(&mut lines, "point");
        assert_eq!(values.len(), 11);
        assert_eq!(
            floats(&values[..3]),
            [
                points.easting[index],
                points.northing[index],
                points.height[index]
            ]
            .map(f64::to_bits),
            "座標 {index}"
        );
        assert_eq!(
            values[3].parse::<u16>().unwrap(),
            points.intensity.as_ref().unwrap()[index]
        );
        assert_eq!(
            values[4].parse::<u8>().unwrap(),
            points.classification.as_ref().unwrap()[index]
        );
        assert_eq!(
            [
                values[5].parse::<u8>().unwrap(),
                values[6].parse::<u8>().unwrap()
            ],
            points.returns.as_ref().unwrap()[index]
        );
        if let Some(time) = &points.time {
            assert_eq!(floats(&values[7..8]), [time[index].to_bits()]);
        } else {
            assert_eq!(values[7], "-");
        }
        if let Some(color) = &points.color {
            assert_eq!(
                values[8..]
                    .iter()
                    .map(|value| value.parse::<u16>().unwrap())
                    .collect::<Vec<_>>(),
                color[index]
            );
        } else {
            assert_eq!(&values[8..], ["-", "-", "-"]);
        }
    }
    assert!(lines.next().is_none(), "laspy の点が余る");
}

#[test]
fn reads_known_values_in_all_point_formats() {
    for format in 0..=10 {
        let bytes = fixture(format);
        let (_, cloud, _) = open(&bytes, false);
        assert_eq!(cloud.points.len(), 4);
        for (index, expected) in POSITIONS.into_iter().enumerate() {
            assert_eq!(
                [
                    cloud.points.easting[index],
                    cloud.points.northing[index],
                    cloud.points.height[index]
                ],
                expected
            );
        }
        assert_eq!(
            cloud.points.intensity.as_deref(),
            Some(INTENSITIES.as_slice())
        );
        assert_eq!(
            cloud.points.classification.as_deref(),
            Some(if format <= 5 {
                [2, 6, 31, 0].as_slice()
            } else {
                [2, 64, 200, 255].as_slice()
            })
        );
        assert_eq!(
            cloud.points.returns.as_deref(),
            Some(if format <= 5 {
                [[1, 2], [2, 2], [5, 5], [1, 1]].as_slice()
            } else {
                [[1, 2], [8, 9], [15, 15], [1, 1]].as_slice()
            })
        );
        if let Some(time) = cloud.points.time {
            assert_eq!(time, TIMES);
        }
        if let Some(color) = cloud.points.color {
            assert_eq!(color, COLORS);
        }
    }
}

#[test]
fn compares_synthetic_and_public_las_with_laspy() {
    let Some(oracle) = oracle() else {
        return;
    };
    for format in 0..=10 {
        let bytes = fixture(format);
        let (file, cloud, _) = open(&bytes, false);
        compare(&file, &cloud, &external(&oracle, &bytes));
    }
    let (file, cloud, _) = open(PUBLIC_SAMPLE, false);
    compare(&file, &cloud, &external(&oracle, PUBLIC_SAMPLE));
}

#[test]
fn public_sample_keeps_its_values_when_read_in_ranges() {
    let bytes = PUBLIC_SAMPLE;
    let whole = open(bytes, false);
    assert_eq!(open(bytes, true), whole);
    assert_eq!(whole.1.points.len(), 1);
    assert_eq!(
        [
            whole.1.points.easting[0],
            whole.1.points.northing[0],
            whole.1.points.height[0]
        ],
        [1.0, 2.0, 3.0]
    );
    testkit::records_in_order::<Las>(bytes).unwrap();
    testkit::deterministic::<Las>(bytes).unwrap();
    testkit::stops_at_same_place::<Las>(bytes).unwrap();
    testkit::survives_lift::<Las, PointCloud>(bytes).unwrap();
}

#[test]
fn stops_deterministically_at_input_elements_and_work_budgets() {
    let bytes = fixture(10);
    for kind in [Kind::InputBytes, Kind::Elements, Kind::Work] {
        let run = || {
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(1)));
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let result = Las::read(&bytes, &mut cx).and_then(|file| file.lift(&mut cx));
            let Err(Error::Stopped(stop)) = result else {
                panic!("予算で止まらない");
            };
            assert_eq!(stop.to_diagnostic().code(), kind.exceeded_code());
            (
                stop,
                diag.into_vec(),
                Kind::ALL.map(|item| budget.used(item)),
            )
        };
        assert_eq!(run(), run());
    }
}

#[test]
fn reports_attributes_the_pointcloud_model_does_not_keep() {
    let (_, _, diagnostics) = open(&fixture(10), false);
    for name in [
        "scan-angle",
        "user-data",
        "point-source",
        "flags",
        "near-infrared",
        "waveform",
    ] {
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code() == codes::LAS_ATTRIBUTE_DROPPED
                    && d.param("attribute") == Some(&name.into()))
        );
    }
}
