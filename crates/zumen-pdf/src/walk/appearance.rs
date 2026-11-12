//! hayro-interpret 0.9.0 の通常外観の選択・配置と、独立した走査。

use hayro_syntax::object::Rect as PdfRect;
use hayro_syntax::reader::{Readable, Reader, ReaderContext, ReaderExt};
use kurbo::{Affine, Rect, Shape};

use super::*;

/// 外観の配置、外側の状態の保存と復元、root transform と切り抜きの固定分。
const AP_WORK: u64 = 64;
const AP_BYTES: u64 = 4096;
/// 描画する注釈 1 件あたりの `AnnotationStep` の固定分。
const STEP_BYTES: u64 = 256;
/// BTreeSet の節と String の写しを含む、報告済み診断の 1 項目の固定分。
const REPORTED_BYTES: u64 = 1024;

/// 型を確かめるだけの読み取りで、文字列や名前を展開しない。
pub(super) struct Raw<'a>(pub(super) &'a [u8]);

impl<'a> Readable<'a> for Raw<'a> {
    fn read(reader: &mut Reader<'a>, _: &ReaderContext<'a>) -> Option<Self> {
        reader.skip::<Object<'_>>(false).map(Self)
    }
}

impl Reads<'_> {
    pub(crate) fn for_appearances(self) -> Self {
        Self {
            scalar_free: true,
            ..self
        }
    }

    /// 採用した注釈の診断名も、scalar の確保前に数える。
    pub(crate) fn annotation_kind(
        self,
        annotation: &Dict<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<String, Stop> {
        self.source.reserve_tables(4, cx)?;
        cx.check(Kind::Work, 1)?;
        let bound = match annotation.get_raw::<Raw<'_>>(b"Subtype") {
            Some(MaybeRef::NotRef(raw)) => raw.0.len() as u64,
            Some(MaybeRef::Ref(_)) => self.source.metadata_bound(),
            None => 0,
        };
        self.source.reserve_scalar(bound, cx)?;
        Ok(annotation
            .get::<Name<'_>>(b"Subtype")
            .map(|name| name_text(name.as_ref()))
            .unwrap_or_default())
    }
}

impl<'a> Walker<'a, '_> {
    /// 全可視注釈の通常外観を選択・配置・走査できたときだけ、その独立した結果を返す。
    /// ページの内容と状態を共有せず、ページで既に返した「1 度だけ」の診断の覚えだけを写す。
    pub(crate) fn annotations(
        &mut self,
        page: &Page<'a>,
        place: &Ref,
        page_walk: &Self,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Survey>, Stop> {
        if !page.raw().contains_key(b"Annots") {
            return Ok(None);
        }
        self.reset(place);
        self.appearance = true;
        self.reads = self.reads.for_appearances();

        let mut found = Diagnostics::with_max_places(cx.diag.max_places());
        let mut annotation_steps = Vec::new();
        let surveyed = self.survey_annotations(
            page,
            place,
            page_walk,
            &mut annotation_steps,
            &mut Cx::new(cx.budget, &mut found, cx.cancel),
        );
        let selected = match surveyed {
            Ok(selected) => selected,
            Err(stop) => {
                for diagnostic in found.into_vec() {
                    cx.report(diagnostic);
                }
                return Err(stop);
            }
        };
        if !selected || !self.appearance_valid || self.refused.is_some() {
            return Ok(None);
        }
        let finished = self.optional_content.layers(cx).and_then(|image_layers| {
            let diagnostics = self
                .optional_content
                .diagnostics(&mut found, &image_layers, cx)?;
            Ok((image_layers, diagnostics))
        });
        let (image_layers, diagnostics) = match finished {
            Ok(finished) => finished,
            Err(stop) => {
                for diagnostic in found.into_vec() {
                    cx.report(diagnostic);
                }
                return Err(stop);
            }
        };
        for diagnostic in diagnostics {
            cx.report(diagnostic);
        }
        Ok(Some(Survey {
            transfer: self.transfer,
            refused: None,
            image_layers,
            annotation_steps,
        }))
    }

    fn survey_annotations(
        &mut self,
        page: &Page<'a>,
        default_source: &Ref,
        page_walk: &Self,
        steps: &mut Vec<AnnotationStep>,
        cx: &mut Cx<'_>,
    ) -> Result<bool, Stop> {
        // interpret::interpret_page は Annots の辞書でない最初の項目で反復を終える。
        // Array の型付き読みは借用と読み飛ばしだけで、辞書表も scalar も作らない。
        cx.check(Kind::Work, 1)?;
        let Some(annotations) = page.raw().get::<Array<'_>>(b"Annots") else {
            return Ok(false);
        };
        cx.check(Kind::Work, (annotations.data().len() as u64).div_ceil(16))?;
        let mut dicts = annotations.iter::<Dict<'_>>();
        let mut raw = Reader::new(annotations.data());
        let mut any = false;
        let mut reread = lookup_work(annotations.data());
        loop {
            raw.skip_white_spaces_and_comments();
            let Some(item) = raw.read_with_context::<MaybeRef<Raw<'_>>>(&ReaderContext::dummy())
            else {
                break;
            };
            cx.check(Kind::Work, 1)?;
            if let MaybeRef::Ref(id) = &item
                && let Some(array) = self.xref.get::<Array<'_>>((*id).into())
            {
                // 辞書でない大きな配列を、全ファイルや配列の後ろの項目へ広げて測らない。
                let skipped = (array.data().len() as u64).div_ceil(16);
                cx.check(Kind::Work, skipped)?;
                reread = reread.saturating_add(skipped);
                break;
            }
            self.reads.source.reserve_tables(4, cx)?;
            let Some(annotation) = dicts.next() else {
                let (bytes, referenced) = match item {
                    MaybeRef::NotRef(raw) => (raw.0.len() as u64, false),
                    MaybeRef::Ref(_) => (self.reads.source.metadata_bound(), true),
                };
                let skipped = bytes.div_ceil(16);
                cx.check(Kind::Work, skipped)?;
                if referenced {
                    reread = reread.saturating_add(skipped);
                }
                break;
            };
            let read = lookup_work(annotation.data());
            cx.check(Kind::Work, read)?;
            reread = reread.saturating_add(read);
            self.ap_entry(&annotation, b"F", 3, cx)?;
            if annotation.get::<u32>(b"F").unwrap_or(0) & 0x22 != 0 {
                continue;
            }
            let Some(stream) = self.ap_stream(&annotation, cx)? else {
                return Ok(false);
            };
            self.ap_entry(stream.dict(), b"BBox", 3, cx)?;
            self.ap_entry(stream.dict(), b"Matrix", 2, cx)?;
            self.ap_entry(&annotation, b"Rect", 2, cx)?;
            cx.check(Kind::Work, AP_WORK)?;
            cx.check(Kind::DecodedBytes, AP_BYTES)?;
            if !placement(&annotation, stream.dict()) {
                return Ok(false);
            }
            if !any {
                // 最初の描ける AP まで、不要な報告済み集合の写しを作らない。
                for (_, value) in &page_walk.reported {
                    cx.check(Kind::Work, 1 + (value.len() as u64).div_ceil(16))?;
                    cx.check(
                        Kind::DecodedBytes,
                        REPORTED_BYTES.saturating_add(value.len() as u64),
                    )?;
                }
                self.reported.clone_from(&page_walk.reported);
            }
            let id = self.own(&stream);
            let annot_source = match item {
                MaybeRef::Ref(annot_ref) => self
                    .objects
                    .get(&key(annot_ref))
                    .cloned()
                    .unwrap_or_else(|| default_source.clone()),
                MaybeRef::NotRef(_) => id
                    .and_then(|ap_id| self.objects.get(&ap_id).cloned())
                    .or_else(|| {
                        page.raw()
                            .get_ref(b"Annots")
                            .and_then(|arr_ref| self.objects.get(&key(arr_ref)).cloned())
                    })
                    .unwrap_or_else(|| default_source.clone()),
            };
            // hayro は注釈自身の OC（12.5.2）を見ない。先に評価し、隠れる場合は解釈器の実行分だけ数えて装置で抑止する。
            let _ = self.ap_scalar_value(&annotation, b"OC", TARGET_DEPTH, cx)?;
            if !self.appearance_valid {
                return Ok(false);
            }
            if let Some(group) = annotation.get::<Dict<'_>>(b"OC") {
                let groups = group
                    .get::<Array<'_>>(b"OCGs")
                    .map_or(0, |groups| lookup_work(groups.data()));
                cx.check(Kind::Work, self.reread_dict(&group).saturating_add(groups))?;
            }
            let annot_oc = self.optional(&annotation, None, &annot_source, cx)?;
            if annot_oc.is_none() && annotation.contains_key(b"OC") {
                return Ok(false);
            }
            let annot_shown = annot_oc.is_none_or(|(shown, _)| shown);
            // FormXObject::new は Subtype を確かめず、選ばれた N を直接 Form として読む。
            // 通常の Do の前段と、注釈にだけある外側の状態・配置の分も、先に数える。
            self.reads.source.reserve_tables(16, cx)?;
            cx.check(Kind::Work, self.reread_stream(&stream))?;
            let mut ap_shown = true;
            if let Some(group) = stream.dict().get::<Dict<'_>>(b"OC") {
                let groups = group
                    .get::<Array<'_>>(b"OCGs")
                    .map_or(0, |groups| lookup_work(groups.data()));
                cx.check(
                    Kind::Work,
                    self.reread_dict(&group)
                        .saturating_add(groups.saturating_mul(2)),
                )?;
                ap_shown = match stream.dict().get_ref(b"OC") {
                    Some(reference) => self.membership(&group, key(reference)).0,
                    None => self.ocmd(&group),
                };
            }
            if let Some((shown, context)) = annot_oc {
                self.optional_content.push(context, cx)?;
                self.visible.push(shown);
            }
            let place = self.record(stream.dict().obj_id());
            let form_result = self.form(
                &stream,
                id,
                &place,
                page.resources(),
                Owner::Page,
                0,
                Graphics::default(),
                cx,
            );
            if annot_oc.is_some() {
                self.end_marked();
            }
            form_result?;
            if !self.appearance_valid || self.refused.is_some() {
                return Ok(false);
            }
            // AP 自身の OC が既定で非表示なら FormXObject::draw は装置を呼ばずに戻る。
            // 可視の AP だけ push_clip_path .. pop_clip の 1 区間を作る。
            if ap_shown {
                cx.check(Kind::Work, 1)?;
                cx.check(Kind::DecodedBytes, STEP_BYTES)?;
                steps.push(AnnotationStep {
                    source: annot_source,
                    visible: annot_shown,
                });
            }
            any = true;
        }
        if any {
            // 採用できたときだけ、解釈と unread がもう一度読む分を先に数える。
            cx.check(Kind::Work, reread.saturating_mul(2))?;
        }
        Ok(any)
    }

    /// interpret::appearance_stream と同じく、N stream、AS の状態、Off の状態の順に選ぶ。
    /// Object として読む代わりに型ごとに読むことで、不正な N の文字列は展開しない。
    fn ap_stream(
        &self,
        annotation: &Dict<'a>,
        cx: &mut Cx<'_>,
    ) -> Result<Option<Stream<'a>>, Stop> {
        self.ap_entry(annotation, b"AP", 2, cx)?;
        let Some(appearances) = annotation.get::<Dict<'_>>(b"AP") else {
            return Ok(None);
        };
        self.ap_entry(&appearances, b"N", 3, cx)?;
        if let Some(stream) = appearances.get::<Stream<'_>>(b"N") {
            return Ok(Some(stream));
        }
        let Some(states) = appearances.get::<Dict<'_>>(b"N") else {
            return Ok(None);
        };
        self.ap_entry(annotation, b"AS", 2, cx)?;
        if annotation.contains_key(b"AS") {
            let bound = match annotation.get_raw::<Raw<'_>>(b"AS") {
                Some(MaybeRef::NotRef(raw)) => raw.0.len() as u64,
                _ => self.reads.source.metadata_bound(),
            };
            self.reads.source.reserve_scalar(bound, cx)?;
        }
        if let Some(state) = annotation.get::<Name<'_>>(b"AS") {
            self.ap_entry(&states, state.as_ref(), 2, cx)?;
            if let Some(stream) = states.get::<Stream<'_>>(state) {
                return Ok(Some(stream));
            }
        }
        self.ap_entry(&states, b"Off", 2, cx)?;
        Ok(states.get::<Stream<'_>>(b"Off"))
    }

    /// 新しい入口の辞書表を、guard が測った最大の表で確保前に予約する。
    /// 参照先は scalar を作らず、型付き辞書・配列の実長で数える。未知の scalar だけファイルか最大 ObjStm を上限にする。
    fn ap_entry(
        &self,
        dict: &Dict<'_>,
        name: &[u8],
        copies: u64,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        cx.check(Kind::Work, copies)?;
        let Some(value) = dict.get_raw::<Raw<'_>>(name) else {
            return Ok(());
        };
        self.reads
            .source
            .reserve_tables(copies.saturating_mul(2), cx)?;
        match value {
            MaybeRef::Ref(id) => {
                cx.check(
                    Kind::Work,
                    self.reads.read(id.into()).0.saturating_mul(copies),
                )?;
                // Rect/BBox/Matrix が参照の配列なら、その中の数の参照も使う前に数える。
                if matches!(name, b"Rect" | b"BBox" | b"Matrix")
                    && let Some(array) = dict.get::<Array<'_>>(name)
                {
                    self.ap_references(array.data(), copies, cx)?;
                }
            }
            MaybeRef::NotRef(raw) => self.ap_references(raw.0, copies, cx)?,
        }
        Ok(())
    }

    fn ap_references(&self, data: &[u8], copies: u64, cx: &mut Cx<'_>) -> Result<(), Stop> {
        cx.check(Kind::Work, lookup_work(data).saturating_mul(copies))?;
        for id in written_references(data) {
            self.reads.source.reserve_tables(4, cx)?;
            cx.check(Kind::Work, self.reads.read(id).0.saturating_mul(copies))?;
        }
        Ok(())
    }

    /// 実解釈に使う値の scalar。計測のためだけに Object にせず、容器と参照を型付きで先にたどる。
    /// 同じ参照が何度も出れば、その回数だけ予約する。深さの先は、関数・色空間の次の入口でも確かめる。
    pub(super) fn ap_scalar_value(
        &mut self,
        dict: &Dict<'_>,
        name: &[u8],
        depth: u32,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        if !self.appearance {
            return Ok(Cost::default());
        }
        let work = cx.budget.used(Kind::Work);
        let bytes = cx.budget.used(Kind::DecodedBytes);
        self.reads.source.reserve_tables(4, cx)?;
        cx.check(Kind::Work, 1)?;
        match dict.get_raw::<Raw<'_>>(name) {
            Some(MaybeRef::Ref(id)) => self.ap_scalar_reference(id.into(), depth, cx)?,
            Some(MaybeRef::NotRef(raw)) => self.ap_scalar_data(raw.0, depth, cx)?,
            None => {}
        }
        Ok(Cost {
            ops: cx.budget.used(Kind::Work).saturating_sub(work),
            bytes: cx.budget.used(Kind::DecodedBytes).saturating_sub(bytes),
        })
    }

    pub(super) fn ap_scalar_object(
        &mut self,
        object: &Object<'_>,
        depth: u32,
        cx: &mut Cx<'_>,
    ) -> Result<Cost, Stop> {
        if !self.appearance {
            return Ok(Cost::default());
        }
        let work = cx.budget.used(Kind::Work);
        let bytes = cx.budget.used(Kind::DecodedBytes);
        self.ap_scalar_data(object_data(object), depth, cx)?;
        Ok(Cost {
            ops: cx.budget.used(Kind::Work).saturating_sub(work),
            bytes: cx.budget.used(Kind::DecodedBytes).saturating_sub(bytes),
        })
    }

    fn ap_scalar_data(&mut self, data: &[u8], depth: u32, cx: &mut Cx<'_>) -> Result<(), Stop> {
        self.reads.source.reserve_scalar(data.len() as u64, cx)?;
        if depth == 0 {
            if written_references(data).next().is_some() {
                self.appearance_valid = false;
            }
            return Ok(());
        }
        for id in written_references(data) {
            self.ap_scalar_reference(id, depth - 1, cx)?;
            if !self.appearance_valid {
                break;
            }
        }
        Ok(())
    }

    fn ap_scalar_reference(
        &mut self,
        id: ObjectIdentifier,
        depth: u32,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        // 辞書を型付きで引く前にも、その表と、失敗して読み飛ばす最長の値を予約する。
        self.reads.source.reserve_tables(4, cx)?;
        let bound = self.reads.source.metadata_bound();
        cx.check(Kind::Work, bound.div_ceil(LOOKUP_BYTES).saturating_mul(3))?;
        if let Some(dict) = self.xref.get::<Dict<'_>>(id) {
            self.ap_scalar_data(dict.data(), depth, cx)
        } else if let Some(array) = self.xref.get::<Array<'_>>(id) {
            self.ap_scalar_data(array.data(), depth, cx)
        } else if self.xref.get::<f64>(id).is_some() || self.xref.get::<bool>(id).is_some() {
            Ok(())
        } else {
            self.reads.source.reserve_scalar(bound, cx)
        }
    }
}

/// interpret_annotation と FormXObject::new の型・丸め・矩形の正規化に合わせる。
/// 分母と写した座標が有限で、ゼロでないときだけ、依存へ配置を渡す。
fn placement(annotation: &Dict<'_>, appearance: &Dict<'_>) -> bool {
    let Some(rect) = annotation.get::<PdfRect>(b"Rect") else {
        return false;
    };
    let Some(bbox) = appearance.get::<[f32; 4]>(b"BBox") else {
        return false;
    };
    let matrix = match appearance.get::<[f64; 6]>(b"Matrix") {
        Some(matrix) => matrix,
        None if !appearance.contains_key(b"Matrix") => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        None => return false,
    };
    if ![rect.x0, rect.y0, rect.x1, rect.y1]
        .iter()
        .chain(matrix.iter())
        .all(|number| number.is_finite())
        || !bbox.iter().all(|number| number.is_finite())
        || rect.width() <= 0.0
        || rect.height() <= 0.0
        || bbox[0] == bbox[2]
        || bbox[1] == bbox[3]
    {
        return false;
    }
    let matrix = Affine::new(matrix);
    if !matrix.determinant().is_finite() || matrix.determinant() == 0.0 {
        return false;
    }
    let transformed = (matrix
        * Rect::new(
            f64::from(bbox[0]),
            f64::from(bbox[1]),
            f64::from(bbox[2]),
            f64::from(bbox[3]),
        )
        .to_path(0.1))
    .bounding_box();
    if ![
        transformed.x0,
        transformed.y0,
        transformed.x1,
        transformed.y1,
        transformed.width(),
        transformed.height(),
    ]
    .iter()
    .all(|number| number.is_finite())
        || transformed.width() <= 0.0
        || transformed.height() <= 0.0
    {
        return false;
    }
    let sx = rect.width() / transformed.width();
    let sy = rect.height() / transformed.height();
    let affine = Affine::new([
        sx,
        0.0,
        0.0,
        sy,
        rect.x0 - sx * transformed.x0,
        rect.y0 - sy * transformed.y0,
    ]);
    sx > 0.0
        && sy > 0.0
        && affine.as_coeffs().iter().all(|number| number.is_finite())
        && (affine * matrix)
            .as_coeffs()
            .iter()
            .all(|number| number.is_finite())
}
