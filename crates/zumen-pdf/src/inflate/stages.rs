//! 単独と系列で共通のフィルター引数と、予測子の事前予約（#769・#774）。

use hayro_syntax::Filter;
use hayro_syntax::object::{Dict, Name, Object};
use zumen_base::{Cx, Kind, Stop};

use super::{allocation, metadata::Metadata};

#[derive(Clone, Copy)]
pub(super) struct Stage {
    pub(super) filter: Filter,
    pub(super) predictor: u8,
    pub(super) colors: u8,
    pub(super) bits: u8,
    pub(super) columns: usize,
    pub(super) early: bool,
}

impl Stage {
    pub(super) fn new(filter: Filter, params: Option<Dict<'_>>) -> Self {
        // 予測子を使わない段では、不要な引数の値の参照を解かない。
        let params = if matches!(filter, Filter::FlateDecode | Filter::LzwDecode) {
            params.unwrap_or_default()
        } else {
            Dict::default()
        };
        // hayro-syntax 0.9.0 の PredictorParams::from_params と同じ型と既定値。
        // guard は、この処理より前に、環境の大きさの値と予測子の積を確かめる。
        Self {
            filter,
            predictor: params.get(b"Predictor").unwrap_or(1),
            colors: params.get(b"Colors").unwrap_or(1),
            bits: params.get(b"BitsPerComponent").unwrap_or(8),
            columns: params.get(b"Columns").unwrap_or(1),
            early: params.get::<u8>(b"EarlyChange").is_none_or(|n| n != 0),
        }
    }

    pub(super) fn name(self) -> &'static str {
        match self.filter {
            Filter::FlateDecode => "FlateDecode",
            Filter::LzwDecode => "LZWDecode",
            Filter::RunLengthDecode => "RunLengthDecode",
            // ASCII は合成するストリームで展開せず、既存の確認で扱う。
            _ => "Crypt",
        }
    }

    /// 予測子の出力は入力以下。8 ビットの専用経路以外は、小さい列を画素ごとに
    /// 作るので、その確保も含む。guard が先に整数の積・行の長さを制限している。
    pub(super) fn predictor_cost(self, bytes: u64) -> (u64, u64) {
        if self.predictor == 1 || !matches!(self.filter, Filter::FlateDecode | Filter::LzwDecode) {
            return (0, 0);
        }
        if !matches!(self.bits, 1 | 2 | 4 | 8 | 16) {
            // 上流も行の長さを計算したあと、バッファーを作る前に失敗する。
            return (1, 0);
        }
        let pixel = u64::from(self.colors).saturating_mul(u64::from(self.bits));
        let row = (self.columns as u64).saturating_mul(pixel).div_ceil(8);
        let png = self.predictor >= 10;
        let chunk = if png {
            pixel.div_ceil(8)
        } else {
            u64::from(self.colors)
        };
        if (png || self.bits == 8)
            && matches!(chunk, 1 | 2 | 3 | 4 | 6 | 8)
            && row.is_multiple_of(chunk)
        {
            // apply_predictor_8bit_png は同じ Vec 内で行を詰め直し、追加の列を作らない。
            // 行の移動、unfilter と最後の切り詰めを、1 バイトあたり Work の 8 に数える。
            return (bytes.saturating_mul(8).saturating_add(1), 0);
        }
        // BitChunks は 1 成分に u16 を持ち、伸長の全確保は 8 バイト/成分未満。
        // 現在の行・前の行・出力からの読み直しの 3 列と、行頭の列を 2 回複製する。
        // clone も容量を伸ばすが、成分数 C > 4 の各列の全確保は 8C - 24 以下、
        // clone 2 回は 8C - 8 以下。出力と末尾の余りも含め、各行は 256 * 行長以下。
        // 固定の空行と初回の列も別に数えるので、行がない短い入力でも確保を覆う。
        let allocated = bytes
            .saturating_mul(256)
            .saturating_add(row)
            .saturating_add(8192);
        let work = bytes
            .saturating_mul(128)
            .saturating_add(row)
            .saturating_add(256);
        (work, allocated)
    }
}

/// Stream::filters_and_params と同じ優先順。未知名もパラメーターを 1 つ進め、
/// 名前でない項目で両方の走査を終える。パラメーター内の参照は元の文脈で解く。
pub(super) fn read(
    dict: &Dict<'_>,
    metadata: Metadata<'_>,
    cx: &mut Cx<'_>,
) -> Result<Vec<Stage>, Stop> {
    let mut out = Vec::new();
    let name = match metadata.name(dict, b"F", cx)? {
        Some(name) => Some(name),
        None => metadata.name(dict, b"Filter", cx)?,
    };
    if let Some(filter) = name.and_then(|name| filter(&name)) {
        allocation::reserve((1, 128), cx)?;
        let params = match metadata.dict(dict, b"DP", cx)? {
            Some(params) => Some(params),
            None => metadata.dict(dict, b"DecodeParms", cx)?,
        };
        out.push(Stage::new(filter, params));
    } else {
        let filters = match metadata.array(dict, b"F", cx)? {
            Some(filters) => Some(filters),
            None => metadata.array(dict, b"Filter", cx)?,
        };
        let Some(filters) = filters else {
            return Ok(out);
        };
        let attempts = metadata.names(&filters, cx)?;
        let params = match metadata.array(dict, b"DP", cx)? {
            Some(params) => Some(params),
            None => metadata.array(dict, b"DecodeParms", cx)?,
        };
        if let Some(params) = &params {
            metadata.objects(params, attempts, cx)?;
        }
        let mut params = params.map(|array| array.iter::<Object<'_>>());
        for name in filters.iter::<Name<'_>>() {
            cx.check(Kind::Work, 1)?;
            let params = params
                .as_mut()
                .and_then(Iterator::next)
                .and_then(Object::into_dict);
            if let Some(filter) = filter(&name) {
                // Stage はどの環境でも 32 バイト以下。Vec の全伸長を 4 倍で数える。
                allocation::reserve((1, 128), cx)?;
                out.push(Stage::new(filter, params));
            }
        }
    }
    Ok(out)
}

fn filter(name: &Name<'_>) -> Option<Filter> {
    Some(match name.as_ref() {
        b"ASCIIHexDecode" | b"AHx" => Filter::AsciiHexDecode,
        b"ASCII85Decode" | b"A85" => Filter::Ascii85Decode,
        b"LZWDecode" | b"LZW" => Filter::LzwDecode,
        b"FlateDecode" | b"Fl" => Filter::FlateDecode,
        b"RunLengthDecode" | b"RL" => Filter::RunLengthDecode,
        b"CCITTFaxDecode" | b"CCF" => Filter::CcittFaxDecode,
        b"JBIG2Decode" => Filter::Jbig2Decode,
        b"DCTDecode" | b"DCT" => Filter::DctDecode,
        b"JPXDecode" => Filter::JpxDecode,
        b"Crypt" => Filter::Crypt,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predictor_dispatch_matches_hayro_for_packed_rows_and_sixteen_bit_png() {
        for (predictor, colors, bits, columns, additional_array) in [
            (2, 3, 1, 2, true),
            (2, 3, 2, 2, true),
            (2, 3, 4, 2, true),
            (2, 3, 8, 2, false),
            (2, 3, 16, 2, true),
            (12, 3, 4, 1, false),
            (12, 3, 4, 2, true),
            (12, 3, 16, 1, false),
            (12, 5, 8, 1, true),
            (12, 5, 16, 1, true),
        ] {
            let stage = Stage {
                filter: Filter::FlateDecode,
                predictor,
                colors,
                bits,
                columns,
                early: true,
            };
            let (_, bytes) = stage.predictor_cost(16);
            assert_eq!(
                bytes > 0,
                additional_array,
                "{predictor}/{colors}/{bits}/{columns}"
            );
        }
    }

    #[test]
    fn empty_generic_rows_still_need_the_zero_row_and_disabled_predictors_do_not() {
        let mut stage = Stage {
            filter: Filter::FlateDecode,
            predictor: 12,
            colors: 5,
            bits: 8,
            columns: 209_715,
            early: true,
        };
        let (work, bytes) = stage.predictor_cost(0);
        assert!(bytes >= 1_048_575);
        assert!(work >= 1_048_575);
        stage.predictor = 1;
        stage.colors = 0;
        stage.bits = 0;
        stage.columns = 0;
        assert_eq!(stage.predictor_cost(u64::MAX), (0, 0));
        stage.predictor = 12;
        stage.bits = 3;
        assert_eq!(stage.predictor_cost(u64::MAX), (1, 0));
    }

    #[test]
    fn stage_capacity_upper_bound_is_the_same_on_supported_word_sizes() {
        assert!(std::mem::size_of::<Stage>() <= 32);
    }
}
