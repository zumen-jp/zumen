//! フォントと ICC の展開経路の合成入力（8.6.5.5・9.7・9.9・9.10）。

#![cfg(test)]

use crate::common;

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Type1,
    Type1Cff,
    TrueType,
    CidType1,
    CidCff,
    CidTrueType,
    Type1Unicode,
    TrueTypeUnicode,
    CidUnicode,
    Type3Unicode,
    Encoding,
    CidToGid,
    Icc,
}

pub const KINDS: [Kind; 13] = [
    Kind::Type1,
    Kind::Type1Cff,
    Kind::TrueType,
    Kind::CidType1,
    Kind::CidCff,
    Kind::CidTrueType,
    Kind::Type1Unicode,
    Kind::TrueTypeUnicode,
    Kind::CidUnicode,
    Kind::Type3Unicode,
    Kind::Encoding,
    Kind::CidToGid,
    Kind::Icc,
];

/// フォントの辞書を分けると、対応表は同じでも hayro-font がもう一度読む。
#[derive(Clone, Copy)]
pub struct Uses {
    pub direct: bool,
    pub calls: usize,
    pub fonts: usize,
    pub pages: usize,
    pub clip: bool,
}

impl Default for Uses {
    fn default() -> Self {
        Self {
            direct: false,
            calls: 1,
            fonts: 1,
            pages: 1,
            clip: false,
        }
    }
}

/// 字形 1 が endchar だけの CFF。drawing.rs の最小の合成フォントと同じ配置（Technical Note #5176）。
pub fn program() -> Vec<u8> {
    vec![
        1, 0, 4, 1, 0, 1, 1, 1, 2, b'F', 0, 1, 1, 1, 12, 28, 0, 30, 17, 28, 0, 0, 28, 0, 38, 18, 0,
        0, 0, 0, 0, 2, 1, 1, 2, 3, 14, 14,
    ]
}

pub fn plain(kind: Kind) -> Vec<u8> {
    match kind {
        Kind::Encoding => b"begincmap /CMapName /Test def /CMapType 1 def /WMode 0 def \
            1 begincodespacerange <0000> <FFFF> endcodespacerange \
            1 begincidchar <0001> 1 endcidchar endcmap"
            .to_vec(),
        Kind::Type1Unicode | Kind::TrueTypeUnicode | Kind::CidUnicode | Kind::Type3Unicode => {
            b"begincmap /CMapName /Test def /CMapType 2 def \
              1 begincodespacerange <0000> <FFFF> endcodespacerange \
              2 beginbfchar <0001> <0041> <0041> <0042> endbfchar endcmap"
                .to_vec()
        }
        Kind::CidToGid => vec![0, 0, 0, 1],
        Kind::Icc => {
            let mut profile = moxcms::ColorProfile::new_srgb().encode().unwrap();
            // moxcms は生成日時を入れる。ICC ヘッダーの日時（24〜35 バイト目）を固定して同じ入力にする。
            profile[24..36].copy_from_slice(&[7, 234, 0, 11, 0, 6, 0, 0, 0, 0, 0, 0]);
            profile
        }
        _ => program(),
    }
}

pub fn document(kind: Kind, filter: &str, data: &[u8], uses: Uses) -> Vec<u8> {
    let target = common::stream(&format!("/N 3 /Alternate /DeviceRGB {filter}"), data);
    let value = if uses.direct {
        target.as_slice()
    } else {
        b"3 0 R"
    };
    let first_page = 8 + uses.fonts;
    let kids = (0..uses.pages)
        .map(|page| format!("{} 0 R", first_page + page * 2))
        .collect::<Vec<_>>()
        .join(" ");
    let mut builder = common::Builder::default();
    builder.object(b"<< /Type /Catalog /Pages 2 0 R >>");
    builder.object(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {} >>",
        uses.pages
    ));
    builder.object(&target);
    builder.stream("", program());
    builder.stream("", plain(Kind::CidUnicode));
    builder.stream("", b"500 0 d0 0 0 1 1 re f");
    let cid = matches!(
        kind,
        Kind::CidType1
            | Kind::CidCff
            | Kind::CidTrueType
            | Kind::CidUnicode
            | Kind::Encoding
            | Kind::CidToGid
    );
    for index in 0..uses.fonts {
        let (prefix, suffix) = match kind {
            Kind::Icc => ("/ShadingType 2 /ColorSpace [/ICCBased ".to_owned(),
                "] /Coords [0 0 100 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 1 0] /N 1 >>".to_owned()),
            Kind::Type3Unicode => ("/Type /Font /Subtype /Type3 /FirstChar 65 /LastChar 65 /Widths [500] /FontMatrix [0.001 0 0 0.001 0 0] /Encoding << /Differences [65 /A] >> /CharProcs << /A 6 0 R >> /ToUnicode ".to_owned(), String::new()),
            _ if cid => {
                let descriptor = "/FontDescriptor << /FontName /Helvetica /Flags 4 /FontBBox [0 0 100 100]";
                let base = "/Type /Font /Subtype /Type0 /BaseFont /Helvetica";
                let descendant = "/DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Helvetica /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >>";
                match kind {
                    Kind::Encoding => (format!("{base} /Encoding "), format!(" {descendant} {descriptor} /FontFile3 4 0 R >> >>] /ToUnicode 5 0 R")),
                    Kind::CidUnicode => (format!("{base} /Encoding /Identity-H {descendant} {descriptor} /FontFile3 4 0 R >> >>] /ToUnicode "), String::new()),
                    Kind::CidToGid => (format!("{base} /Encoding /Identity-H {descendant} {descriptor} /FontFile3 4 0 R >> /CIDToGIDMap "), " >>] /ToUnicode 5 0 R".to_owned()),
                    _ => {
                        let key = match kind { Kind::CidType1 => "FontFile", Kind::CidCff => "FontFile3", _ => "FontFile2" };
                        (format!("{base} /Encoding /Identity-H {descendant} {descriptor} /{key} "), " >> >>] /ToUnicode 5 0 R".to_owned())
                    }
                }
            }
            _ => {
                let subtype = if matches!(kind, Kind::TrueType | Kind::TrueTypeUnicode) { "TrueType" } else { "Type1" };
                let base = format!("/Type /Font /Subtype /{subtype} /BaseFont /Helvetica /FirstChar 65 /LastChar 65 /Widths [500]");
                if matches!(kind, Kind::Type1Unicode | Kind::TrueTypeUnicode) {
                    (format!("{base} /ToUnicode "), String::new())
                } else {
                    let key = match kind { Kind::Type1 => "FontFile", Kind::Type1Cff => "FontFile3", _ => "FontFile2" };
                    (format!("{base} /FontDescriptor << /FontName /Helvetica /Flags 4 /{key} "), " >>".to_owned())
                }
            }
        };
        let mut font = format!("<< /Test {index} {prefix}").into_bytes();
        font.extend(value);
        font.extend(format!("{suffix} >>").as_bytes());
        builder.object(font);
    }
    let names = (0..uses.fonts)
        .map(|index| format!("/F{index} {} 0 R", 7 + index))
        .collect::<Vec<_>>()
        .join(" ");
    let category = if matches!(kind, Kind::Icc) {
        "Shading"
    } else {
        "Font"
    };
    let operations = (0..uses.fonts)
        .map(|index| {
            if matches!(kind, Kind::Icc) {
                format!("/F{index} sh ")
            } else {
                format!(
                    "BT /F{index} 12 Tf {} Tr 10 10 Td <{}> Tj ET ",
                    if uses.clip { 7 } else { 0 },
                    if cid { "0001" } else { "41" }
                )
            }
        })
        .collect::<String>()
        .repeat(uses.calls);
    for _ in 0..uses.pages {
        let contents = builder.stream("", format!("{operations}0 0 m 10 10 l S"));
        builder.object(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents {contents} 0 R /Resources << /{category} << {names} >> >> >>"));
    }
    builder.bytes(1)
}
