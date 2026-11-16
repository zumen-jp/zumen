//! ACadSharp が書かない R2000・R2004 の表を、自作の小さなレコードで埋める。
//! 先に TableFixtures.cs で作った同じフォルダーを引数にする。製品のファイルは使わない。

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    reason = "自作の固定した試験入力だけを書き出す開発用の例"
)]

use std::path::Path;
use zumen_acad::Dwg;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_sdk::{Codec, Transact, Transaction};

#[derive(Default)]
struct Bits(Vec<bool>);

impl Bits {
    fn bits(&mut self, value: u64, count: usize) {
        self.0.extend((0..count).rev().map(|i| value >> i & 1 != 0));
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.bits(u64::from(byte), 8);
        }
    }

    fn bs(&mut self, value: u16) {
        match value {
            0 => self.bits(2, 2),
            256 => self.bits(3, 2),
            1..=255 => {
                self.bits(1, 2);
                self.bits(u64::from(value), 8);
            }
            _ => {
                self.bits(0, 2);
                self.bytes(&value.to_le_bytes());
            }
        }
    }

    fn bl(&mut self, value: u8) {
        if value == 0 {
            self.bits(2, 2);
        } else {
            self.bits(1, 2);
            self.bits(u64::from(value), 8);
        }
    }

    fn bd(&mut self, value: f64) {
        if value == 0.0 {
            self.bits(2, 2);
        } else if value == 1.0 {
            self.bits(1, 2);
        } else {
            self.bits(0, 2);
            self.bytes(&value.to_le_bytes());
        }
    }

    fn point(&mut self, point: [f64; 3]) {
        for v in point {
            self.bd(v);
        }
    }

    fn handle(&mut self, code: u8, value: u64) {
        let bytes = value.to_be_bytes();
        let start = bytes.iter().position(|&b| b != 0).unwrap_or(8);
        self.bits(u64::from(code), 4);
        self.bits((8 - start) as u64, 4);
        self.bytes(&bytes[start..]);
    }

    fn finish(self) -> Vec<u8> {
        self.0
            .chunks(8)
            .map(|chunk| {
                chunk
                    .iter()
                    .enumerate()
                    .fold(0, |byte, (i, &bit)| byte | (u8::from(bit) << (7 - i)))
            })
            .collect()
    }
}

// ODS 5.4.1 の 20.4.96.1。INSERT の後に 2 列 2 行、文字セル、上書きなしを並べる。
fn table_data(class: u16, handle: u64, block: u64, layer: u64, r2004: bool) -> Vec<u8> {
    let mut w = Bits::default();
    w.bs(class);
    let size_at = w.0.len();
    w.bytes(&0u32.to_le_bytes());
    w.handle(0, handle);
    w.bs(0); // XDATA の終わり
    w.bits(0, 1); // 画像なし
    w.bits(2, 2); // モデル空間
    w.bl(0); // リアクターなし
    w.bits(1, 1); // R2000 は前後へのリンクなし、R2004 は拡張辞書なし
    w.bs(256); // 色は画層から
    w.bd(1.0); // 線種の尺度
    w.bits(0, 4); // 線種・印刷スタイルは画層から
    w.bs(0); // 表示する
    w.bits(0x1d, 8); // 線幅は画層から
    w.point([10.0, 20.0, 0.0]);
    w.bits(3, 2); // 尺度はすべて 1
    w.bd(0.0); // 回転
    w.point([0.0, 0.0, 1.0]);
    w.bits(0, 1); // 属性なし
    w.bs(0); // 表のフラグ
    w.point([1.0, 0.0, 0.0]);
    w.bl(2); // 列
    w.bl(2); // 行
    for value in [20.0, 30.0, 10.0, 15.0] {
        w.bd(value);
    }
    for text in [b"A00", b"A01", b"A10", b"A11"] {
        w.bs(1); // 文字セル
        w.bits(0, 8); // フラグ
        w.bits(0, 2); // 結合・自動合わせなし
        w.bl(1); // 列の数
        w.bl(1); // 行の数
        w.bd(0.0); // 角度
        w.bs(4); // TV: 終端を含むバイト数
        w.bytes(text);
        w.bits(0, 8);
        w.bits(0, 1); // セルの上書きなし
    }
    w.bits(0, 4); // 表・枠の色・線幅・表示の上書きなし
    let handles_at = u32::try_from(w.0.len()).unwrap();
    let mut size = Bits::default();
    size.bytes(&handles_at.to_le_bytes());
    w.0[size_at..size_at + 32].copy_from_slice(&size.0);
    if !r2004 {
        w.handle(3, 0); // 拡張辞書
    }
    w.handle(5, layer);
    w.handle(5, block);
    w.handle(5, 0); // 表のスタイル
    for _ in 0..4 {
        w.handle(5, 0); // 各文字セルのフィールド
    }
    w.finish()
}

fn main() {
    let folder = std::env::args().nth(1).expect("入力フォルダーを指定");
    for (name, r2004) in [("table-r2000.dwg", false), ("table-r2004.dwg", true)] {
        let path = Path::new(&folder).join(name);
        let meta: Vec<u64> = std::fs::read_to_string(path.with_extension("dwg.meta"))
            .unwrap()
            .lines()
            .map(|v| v.parse().unwrap())
            .collect();
        let [class, handle, block, layer] = meta.as_slice() else {
            panic!("合成入力のメタデータが違う")
        };
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let mut file = Dwg::read(&std::fs::read(&path).unwrap(), &mut cx).unwrap();
        assert_eq!(file.version(), if r2004 { b"AC1018" } else { b"AC1015" });
        assert_eq!(
            file.objects(&mut cx).unwrap().get(*handle).unwrap().kind,
            b"LINE"
        );
        let record = (0..file.entry_count())
            .find(|&i| file.entry_summary(i).unwrap().0 == *handle)
            .unwrap();
        let data = table_data(
            u16::try_from(*class).unwrap(),
            *handle,
            *block,
            *layer,
            r2004,
        );
        let mut txn = file.transaction();
        txn.set_data(record, &data).unwrap();
        txn.commit(&mut cx).unwrap();
        let mut output = Vec::new();
        Dwg::write(&file, &mut output, &mut cx).unwrap();
        let again = Dwg::read(&output, &mut cx).unwrap();
        let db = again.objects(&mut cx).unwrap();
        assert_eq!(db.get(*handle).unwrap().kind, b"ACAD_TABLE");
        std::fs::write(&path, output).unwrap();
        println!("{name}");
    }
}
