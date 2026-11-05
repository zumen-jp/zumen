//! 共通の展開を使う描画資源の合成入力（7.10・8.6.6.3・8.7.4・11.6.5.2）。

use crate::common;

/// Type 0・Type 4・メッシュ・Indexed・ソフトマスクの順。
#[cfg(test)]
pub fn plain(kind: usize) -> &'static [u8] {
    match kind {
        0 => &[0, 255],
        1 => b"{ dup }",
        2 => &[0, 0, 0, 100, 0, 0, 0, 100, 255, 100, 100, 255],
        3 => &[255, 0, 0, 0, 255, 0],
        4 => b"0 0 10 10 re f",
        _ => panic!("未定義の資源: {kind}"),
    }
}

/// 同じストリームを参照か直接の値に置き、資源を `calls` 回使う。
#[cfg(test)]
pub fn document(kind: usize, filter: &str, data: &[u8], direct: bool, calls: usize) -> Vec<u8> {
    let (dict, prefix, suffix, operation) = match kind {
        0 | 1 => (
            if kind == 0 {
                "/FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8"
            } else {
                "/FunctionType 4 /Domain [0 1] /Range [0 1]"
            },
            "/ExtGState << /T << /TR ",
            " >> >>",
            "/T gs ",
        ),
        2 => (
            "/ShadingType 5 /ColorSpace /DeviceGray /BitsPerCoordinate 8 /BitsPerComponent 8 \
             /VerticesPerRow 2 /Decode [0 100 0 100 0 1]",
            "/Shading << /Sh ",
            " >>",
            "/Sh sh ",
        ),
        3 => (
            "",
            "/Shading << /Sh << /ShadingType 2 /ColorSpace [/Indexed /DeviceRGB 1 ",
            "] /Coords [0 0 100 0] /Function << /FunctionType 2 /Domain [0 1] \
             /C0 [0] /C1 [1] /N 1 >> >> >>",
            "/Sh sh ",
        ),
        4 => (
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Group << /S /Transparency /CS /DeviceGray >>",
            "/ExtGState << /T << /SMask << /S /Luminosity /G ",
            " >> >> >>",
            "/T gs ",
        ),
        _ => panic!("未定義の資源: {kind}"),
    };
    let stream = common::stream(&format!("{dict} {filter}"), data);
    let mut builder = common::Builder::default();
    builder.object(b"<< /Type /Catalog /Pages 2 0 R >>");
    builder.object(b"<< /Type /Pages /Kids [5 0 R] /Count 1 >>");
    builder.object(&stream);
    builder.stream("", format!("{}0 0 m 10 10 l S", operation.repeat(calls)));
    let mut page =
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R /Resources << "
            .to_vec();
    page.extend(prefix.as_bytes());
    page.extend(if direct { stream.as_slice() } else { b"3 0 R" });
    page.extend(suffix.as_bytes());
    page.extend(b" >> >>");
    builder.object(page);
    builder.bytes(1)
}
