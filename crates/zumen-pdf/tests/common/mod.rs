//! 試験の PDF を作る。どれも文字だけで書き、相互参照表の位置を数えて付ける。

#![allow(dead_code)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics};
use zumen_drawing::Drawing2D;
use zumen_pdf::Pdf;
use zumen_sdk::{Lift, testkit};

/// オブジェクトを番号の順に並べて、PDF のバイト列にする
#[derive(Default)]
pub struct Builder {
    objects: Vec<Vec<u8>>,
    /// トレーラーの辞書に足す項目
    pub trailer: String,
}

impl Builder {
    /// 次に足すオブジェクトの番号
    pub fn next(&self) -> usize {
        self.objects.len() + 1
    }

    /// オブジェクトを足し、その番号を返す
    pub fn object(&mut self, body: impl AsRef<[u8]>) -> usize {
        self.objects.push(body.as_ref().to_vec());
        self.objects.len()
    }

    /// ストリームのオブジェクトを足し、その番号を返す。`dict` は辞書に足す項目
    pub fn stream(&mut self, dict: &str, data: impl AsRef<[u8]>) -> usize {
        self.object(stream(dict, data))
    }

    /// `root` をカタログにしたファイル
    pub fn bytes(&self, root: usize) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::new();
        for (index, body) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n", index + 1).as_bytes());
            out.extend(body);
            out.extend(b"\nendobj\n");
        }
        let xref = out.len();
        let size = self.objects.len() + 1;
        out.extend(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
        for offset in offsets {
            out.extend(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend(
            format!(
                "trailer\n<< /Size {size} /Root {root} 0 R {} >>\nstartxref\n{xref}\n%%EOF\n",
                self.trailer
            )
            .as_bytes(),
        );
        out
    }
}

/// 圧縮しない区画（RFC 1951 の 3.2.4）だけの zlib のデータ（RFC 1950）。`FlateDecode` で展開できる
#[cfg(test)]
pub fn zlib(data: &[u8]) -> Vec<u8> {
    let length = u16::try_from(data.len()).unwrap();
    let mut out = vec![0x78, 0x01, 0x01];
    out.extend(length.to_le_bytes());
    out.extend((!length).to_le_bytes());
    out.extend(data);
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend(((b << 16) | a).to_be_bytes());
    out
}

/// ストリームのオブジェクトの中身
pub fn stream(dict: &str, data: impl AsRef<[u8]>) -> Vec<u8> {
    let data = data.as_ref();
    let mut out = format!("<< /Length {} {dict} >>\nstream\n", data.len()).into_bytes();
    out.extend(data);
    out.extend(b"\nendstream");
    out
}

/// 試験のページ
#[derive(Clone, Copy)]
pub struct Page<'a> {
    /// 内容
    pub content: &'a str,
    /// 資源の辞書の中身
    pub resources: &'a str,
    /// ページの辞書に足す項目。`MediaBox` がなければ、US レター（612 × 792 pt）にする
    pub extra: &'a str,
    /// `Contents` の値。空なら、`content` のストリーム
    pub contents: &'a str,
}

impl<'a> Page<'a> {
    pub fn new(content: &'a str) -> Self {
        Self {
            content,
            resources: "",
            extra: "",
            contents: "",
        }
    }

    pub fn resources(self, resources: &'a str) -> Self {
        Self { resources, ..self }
    }

    pub fn extra(self, extra: &'a str) -> Self {
        Self { extra, ..self }
    }

    pub fn contents(self, contents: &'a str) -> Self {
        Self { contents, ..self }
    }
}

/// 試験の文書。1 番はカタログ、2 番はページの木、3 番から `objects`、その後ろにページとその内容を、ページごとに
/// 2 つずつ置く
pub struct Document<'a> {
    pub pages: Vec<Page<'a>>,
    /// 3 番からのオブジェクトの中身
    pub objects: Vec<Vec<u8>>,
    /// カタログに足す項目
    pub catalog: &'a str,
    /// トレーラーに足す項目
    pub trailer: &'a str,
}

impl<'a> Document<'a> {
    pub fn new(pages: Vec<Page<'a>>) -> Self {
        Self {
            pages,
            objects: Vec::new(),
            catalog: "",
            trailer: "",
        }
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut builder = Builder {
            trailer: self.trailer.to_owned(),
            ..Builder::default()
        };
        let first_page = 3 + self.objects.len();
        let kids: Vec<String> = (0..self.pages.len())
            .map(|index| format!("{} 0 R", first_page + 2 * index))
            .collect();
        builder.object(format!(
            "<< /Type /Catalog /Pages 2 0 R {} >>",
            self.catalog
        ));
        builder.object(format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            self.pages.len()
        ));
        for object in &self.objects {
            builder.object(object);
        }
        for page in &self.pages {
            let number = builder.next();
            let media = if page.extra.contains("/MediaBox") {
                ""
            } else {
                "/MediaBox [0 0 612 792]"
            };
            let contents = if page.contents.is_empty() {
                format!("{} 0 R", number + 1)
            } else {
                page.contents.to_owned()
            };
            builder.object(format!(
                "<< /Type /Page /Parent 2 0 R {media} /Contents {contents} /Resources << {} >> {} >>",
                page.resources, page.extra
            ));
            builder.stream("", page.content);
        }
        builder.bytes(1)
    }
}

/// 1 ページの文書
pub fn one_page(page: Page<'_>) -> Vec<u8> {
    Document::new(vec![page]).bytes()
}

/// 読んで図面モデルにする。読み取りと移しの診断を返す
#[cfg(test)]
pub fn lift(src: &[u8]) -> (Drawing2D, Vec<Diagnostic>) {
    let (file, mut diag) = testkit::read::<Pdf>(src);
    let file = file.unwrap();
    let mut budget = Budget::default();
    let mut lifted = Diagnostics::new();
    let cancel = CancelToken::new();
    let drawing = file
        .lift(&mut Cx::new(&mut budget, &mut lifted, &cancel))
        .unwrap();
    diag.extend(lifted.into_vec());
    (drawing, diag)
}

/// 診断の code の名前と件数
pub fn codes(diag: &[Diagnostic]) -> Vec<(&'static str, u64)> {
    diag.iter()
        .map(|diagnostic| (diagnostic.code().name(), diagnostic.count()))
        .collect()
}

/// pt を mm にする
pub fn mm(pt: f64) -> f64 {
    pt * 25.4 / 72.0
}

/// 誤差の範囲で等しい
pub fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}
