//! 試験の中で JWW を組み立てる。

#![allow(dead_code)]

use std::collections::BTreeMap;

/// 知っている欄だけの見出し。メモと名前は空、値は 0
pub fn header(version: u32) -> Vec<u8> {
    Spec::new(version).build()
}

/// 見出しの値。指定しない欄は 0、名前は空
pub struct Spec {
    pub version: u32,
    pub paper: u32,
    /// レイヤグループの状態・縮尺・レイヤの状態
    pub groups: [(u32, f64, [u32; 16]); 16],
    /// レイヤの位置（グループ × 16 + レイヤ）と名前
    pub layer_names: Vec<(usize, &'static str)>,
    pub max_draw_width: i32,
    pub pens: [u32; 10],
    pub printer_widths: [u32; 10],
    /// 線種 2〜9 と 16〜19
    pub patterns: [[u32; 4]; 12],
    /// 印刷の設定の 11 番目
    pub print_flag: u32,
    /// SXF の拡張色の位置・色・幅
    pub sxf_pens: Vec<(usize, u32, u32)>,
    /// SXF の線種の位置・名前・線と間
    pub sxf_line_types: Vec<(usize, &'static str, Vec<f64>)>,
}

impl Spec {
    pub fn new(version: u32) -> Self {
        Self {
            version,
            paper: 0,
            groups: [(0, 0.0, [0; 16]); 16],
            layer_names: Vec::new(),
            max_draw_width: 0,
            pens: [0; 10],
            printer_widths: [0; 10],
            patterns: [[0; 4]; 12],
            print_flag: 0,
            sxf_pens: Vec::new(),
            sxf_line_types: Vec::new(),
        }
    }

    pub fn build(&self) -> Vec<u8> {
        let version = self.version;
        let since = |v: u32, n: usize| if version >= v { n } else { 0 };
        let mut out = b"JwwData.".to_vec();
        out.extend(version.to_le_bytes());
        out.push(0);
        out.extend(self.paper.to_le_bytes());
        out.extend([0; 4]);
        for (state, scale, layers) in &self.groups {
            out.extend(state.to_le_bytes());
            out.extend([0; 4]);
            out.extend(scale.to_le_bytes());
            out.extend([0; 4]);
            for layer in layers {
                out.extend(layer.to_le_bytes());
                out.extend([0; 4]);
            }
        }
        out.extend([0; 80]);
        out.extend(self.max_draw_width.to_le_bytes());
        out.extend([0; 72]);
        for at in 0..256 {
            match self.layer_names.iter().find(|(i, _)| *i == at) {
                Some((_, name)) => cstring(&mut out, name),
                None => out.push(0),
            }
        }
        out.extend([0; 16]);
        let jumps = if version >= 300 { 224 } else { 96 };
        out.extend(vec![
            0;
            28 + since(300, 16)
                + 4
                + 24
                + 24
                + jumps
                + since(300, 56)
                + 80
                + 8
        ]);
        for pen in self.pens {
            out.extend(pen.to_le_bytes());
            out.extend([0; 4]);
        }
        for width in self.printer_widths {
            out.extend([0; 4]);
            out.extend(width.to_le_bytes());
            out.extend([0; 8]);
        }
        let words = |out: &mut Vec<u8>, pattern: &[u32; 4]| {
            for word in pattern {
                out.extend(word.to_le_bytes());
            }
        };
        for pattern in &self.patterns[..8] {
            words(&mut out, pattern);
        }
        out.extend([0; 100]);
        for pattern in &self.patterns[8..] {
            words(&mut out, pattern);
        }
        out.extend([0; 40]);
        out.extend(self.print_flag.to_le_bytes());
        out.extend(vec![0; since(223, 60) + since(225, 32) + since(230, 8)]);
        if version >= 420 {
            let pen = |at: usize| self.sxf_pens.iter().find(|(i, _, _)| *i == at);
            for at in 0..257 {
                out.extend(pen(at).map_or(0, |p| p.1).to_le_bytes());
                out.extend([0; 4]);
            }
            for at in 0..257 {
                out.push(0);
                out.extend([0; 4]);
                out.extend(pen(at).map_or(0, |p| p.2).to_le_bytes());
                out.extend([0; 8]);
            }
            out.extend(vec![0; 33 * 16]);
            for at in 0..33 {
                match self.sxf_line_types.iter().find(|(i, _, _)| *i == at) {
                    Some((_, name, pitches)) => {
                        cstring(&mut out, name);
                        out.extend((pitches.len() as u32).to_le_bytes());
                        for i in 0..10 {
                            out.extend(pitches.get(i).copied().unwrap_or(0.0).to_le_bytes());
                        }
                    }
                    None => out.extend([0; 85]),
                }
            }
        }
        out.extend(vec![0; 380]);
        out
    }
}

/// MFC の CString。ASCII でなければ UTF-16LE
pub fn cstring(out: &mut Vec<u8>, text: &str) {
    if text.is_ascii() {
        out.push(text.len() as u8);
        out.extend(text.as_bytes());
    } else {
        let units: Vec<u16> = text.encode_utf16().collect();
        out.extend([0xFF, 0xFE, 0xFF, units.len() as u8]);
        for unit in units {
            out.extend(unit.to_le_bytes());
        }
    }
}

/// CArchive のオブジェクトを書く。クラスとオブジェクトの番号は 1 から数える
pub struct Archive {
    pub version: u32,
    pub out: Vec<u8>,
    classes: BTreeMap<&'static str, u16>,
    next: u16,
    /// 書いたオブジェクトの始まり
    pub starts: Vec<usize>,
    /// 次に書く図形のレイヤ・レイヤグループ・線幅
    pub layer: u16,
    pub group: u16,
    pub pen_width: u16,
}

impl Archive {
    pub fn new(version: u32) -> Self {
        Self {
            version,
            out: Vec::new(),
            classes: BTreeMap::new(),
            next: 1,
            starts: Vec::new(),
            layer: 2,
            group: 3,
            pen_width: 1,
        }
    }

    pub fn count(&mut self, n: u16) {
        self.out.extend(n.to_le_bytes());
    }

    pub fn tag(&mut self, class: &'static str) {
        self.starts.push(self.out.len());
        if let Some(&number) = self.classes.get(class) {
            self.out.extend((0x8000 | number).to_le_bytes());
        } else {
            self.out.extend([0xFF, 0xFF, 1, 0]);
            self.out.extend((class.len() as u16).to_le_bytes());
            self.out.extend(class.as_bytes());
            self.classes.insert(class, self.next);
            self.next += 1;
        }
        self.next += 1;
    }

    pub fn null(&mut self) {
        self.starts.push(self.out.len());
        self.out.extend([0, 0]);
    }

    pub fn base(&mut self, pen_style: u8, pen_color: u16) {
        self.out.extend(7u32.to_le_bytes());
        self.out.push(pen_style);
        self.out.extend(pen_color.to_le_bytes());
        if self.version >= 351 {
            self.out.extend(self.pen_width.to_le_bytes());
        }
        self.out.extend(self.layer.to_le_bytes());
        self.out.extend(self.group.to_le_bytes());
        self.out.extend([0, 0]);
    }

    pub fn reals(&mut self, values: &[f64]) {
        for value in values {
            self.out.extend(value.to_le_bytes());
        }
    }

    pub fn cstring(&mut self, text: &str) {
        cstring(&mut self.out, text);
    }

    pub fn line(&mut self) {
        self.tag("CDataSen");
        self.base(1, 1);
        self.reals(&[0.0, 0.0, 10.0, 0.0]);
    }

    pub fn text_body(&mut self, content: &str) {
        self.reals(&[0.0, 0.0, 5.0, 0.0]);
        self.out.extend(1u32.to_le_bytes());
        self.reals(&[3.0, 3.0, 0.0, 15.0]);
        self.cstring("ＭＳ ゴシック");
        self.cstring(content);
    }

    /// 図形を 1 つずつ。版 700 の寸法は SXF の欄を持つ
    pub fn all_kinds(&mut self) {
        self.line();
        self.line();
        self.tag("CDataEnko");
        self.base(1, 2);
        self.reals(&[0.0, 0.0, 5.0, 0.0, 1.0, 1.0, 0.0]);
        self.out.extend(0u32.to_le_bytes());
        self.tag("CDataTen");
        self.base(100, 1);
        self.reals(&[1.0, 1.0]);
        self.out.extend([0, 0, 0, 0, 2, 0, 0, 0]);
        self.reals(&[0.0, 1.0]);
        self.tag("CDataMoji");
        self.base(1, 1);
        self.text_body("図面");
        self.tag("CDataSolid");
        self.base(1, 10);
        self.reals(&[0.0; 8]);
        self.out.extend(0x00FF_0000u32.to_le_bytes());
        self.tag("CDataBlock");
        self.base(1, 1);
        self.reals(&[5.0, 5.0, 1.0, 1.0, 0.0]);
        self.out.extend(1u32.to_le_bytes());
        self.tag("CDataSunpou");
        self.base(1, 1);
        self.base(1, 1);
        self.reals(&[0.0, 0.0, 10.0, 0.0]);
        self.base(1, 1);
        self.text_body("10");
        if self.version >= 420 {
            self.out.extend(1u16.to_le_bytes());
            for _ in 0..2 {
                self.base(1, 1);
                self.reals(&[0.0; 4]);
            }
            for _ in 0..4 {
                self.base(1, 1);
                self.reals(&[0.0; 2]);
                self.out.extend(0u32.to_le_bytes());
            }
        }
        self.null();
    }

    pub fn block(&mut self, name: &str) {
        self.tag("CDataList");
        self.base(1, 1);
        self.out.extend([1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        self.cstring(name);
        self.count(1);
        self.line();
    }
}
