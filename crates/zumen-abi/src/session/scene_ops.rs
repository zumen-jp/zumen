//! セッションのシーンの生成と描画。

use super::*;

impl Session {
    /// 起点から配置をたどった 2D のシーンを、blob で返す（決定 0172）
    pub(super) fn scene(
        &mut self,
        request: &SceneRequest,
        cx: &mut Cx<'_>,
    ) -> Result<SceneResult, Failed> {
        let (mut result, bytes) = self.make_scene(request.into(), cx)?;
        result.blob = self.blobs.len() as u64;
        self.blobs.push(Some(Blob {
            kind: BlobKind::Scene,
            bytes,
        }));
        Ok(result)
    }

    /// 既存の scene と同じ生成経路を共有する。予算は描画まで同じ Cx で数える。
    pub(super) fn make_scene(
        &self,
        request: scene::View<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<(SceneResult, Vec<u8>), Failed> {
        scene::check(request, cx)?;
        let palette = self.palette(request.palette, cx)?;
        let fonts = self.fonts(request, cx)?;
        let drawing = self.lift(request.doc, cx)?;
        scene::scene(&drawing, request, palette, &fonts, cx)
    }

    /// シーンを作って描画し、成功した画像だけを登録する（決定 0275）。
    #[cfg(feature = "drawing")]
    pub(super) fn render(
        &mut self,
        request: &messages::RenderRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::RenderResult, Failed> {
        let options = crate::render::options(request, cx)?;
        let (scene, bytes) = self.make_scene(request.into(), cx)?;
        let (mut result, bytes) = crate::render::render(request, &options, scene, &bytes, cx)?;
        result.blob = self.blobs.len() as u64;
        self.blobs.push(Some(Blob {
            kind: BlobKind::Image,
            bytes,
        }));
        Ok(result)
    }

    /// シーンの文字を置くフォントの入力を読む（決定 0188）
    pub(super) fn fonts(
        &self,
        request: scene::View<'_>,
        cx: &mut Cx<'_>,
    ) -> Result<zumen_scene::FontSet<'_>, Failed> {
        let mut set = zumen_scene::FontSet::default();
        for (index, font) in request.fonts.into_iter().flatten().enumerate() {
            let pointer = format!("/fonts/{index}/input");
            if !name::is_safe(&font.input) {
                return Err(fail(cx, invalid_value(pointer)));
            }
            let Some(bytes) = self.inputs.get(&font.input) else {
                return Err(fail(cx, input_missing(&font.input)));
            };
            let number = u32::try_from(font.index.unwrap_or(0)).unwrap_or(u32::MAX);
            let Ok(face) = zumen_text::font::Font::parse(bytes, number) else {
                return Err(fail(cx, invalid_value(pointer)));
            };
            if !set.add(font.name.as_deref(), face) {
                let pointer = match font.name {
                    Some(_) => format!("/fonts/{index}/name"),
                    None => format!("/fonts/{index}"),
                };
                return Err(fail(cx, invalid_value(pointer)));
            }
        }
        for (key, reference) in request.heights.into_iter().flatten() {
            set.declare(key, scene::reference(*reference));
        }
        Ok(set)
    }

    /// 色の表の入力を読む。名前がなければ空の表
    pub(super) fn palette(
        &self,
        name: Option<&str>,
        cx: &mut Cx<'_>,
    ) -> Result<zumen_scene::Palette, Failed> {
        let Some(name) = name else {
            return Ok(zumen_scene::Palette::default());
        };
        if !name::is_safe(name) {
            return Err(fail(cx, invalid_value("/palette")));
        }
        let Some(bytes) = self.inputs.get(name) else {
            return Err(fail(cx, input_missing(name)));
        };
        scene::palette(bytes).ok_or_else(|| fail(cx, invalid_value("/palette")))
    }
}
