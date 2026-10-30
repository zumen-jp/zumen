// 適合試験の C++ の側。包み（include/zumen.hpp）で境界の関数を呼び、シーンの blob を zumen::Scene で読む。
// tests/conformance.rs が、bindings/conformance の組の手順をここに渡し、応答のハッシュを hashes.txt と比べる（決定 0198）

#include <cstddef>
#include <cstdint>
#include <exception>
#include <new>

#include "zumen.hpp"

namespace {

// 1 つの試験の組の、セッションと直前の結果
struct Engine {
    zumen::Session session;
    zumen::Bytes output;
};

} // namespace

// シーンを読み手で読んで数えたもの
struct zumen_cpp_scene_counts {
    std::uint32_t major;
    std::uint32_t sections;
    std::uint32_t chunks;
    std::uint64_t chunk_origins;
    std::uint64_t text_xy;
    std::uint64_t point_xy;
    std::uint64_t glyph_xy;
    std::uint64_t line_and_face_xy;
    // text_strings の行のうち、strings の区画から引けなかったもの
    std::uint64_t bad_texts;
};

extern "C" void *zumen_cpp_engine_new(void) { return new (std::nothrow) Engine(); }

extern "C" void zumen_cpp_engine_free(void *engine) { delete static_cast<Engine *>(engine); }

extern "C" std::int32_t zumen_cpp_engine_session(void *engine, const std::uint8_t *options, std::size_t options_len) {
    std::string_view text(reinterpret_cast<const char *>(options), options_len);
    return static_cast<Engine *>(engine)->session.try_open(text);
}

extern "C" std::int32_t zumen_cpp_engine_push(void *engine, const std::uint8_t *name, std::size_t name_len,
                                              const std::uint8_t *data, std::size_t data_len) {
    std::string_view text(reinterpret_cast<const char *>(name), name_len);
    return static_cast<Engine *>(engine)->session.try_push(text, data, data_len);
}

extern "C" std::int32_t zumen_cpp_engine_call(void *engine, const std::uint8_t *request, std::size_t request_len) {
    Engine *self = static_cast<Engine *>(engine);
    zumen::Reply reply = self->session.try_call(std::string_view(reinterpret_cast<const char *>(request), request_len));
    self->output = std::move(reply.bytes);
    return reply.status;
}

extern "C" std::int32_t zumen_cpp_engine_take(void *engine, std::uint32_t blob) {
    Engine *self = static_cast<Engine *>(engine);
    zumen::Reply reply = self->session.try_take(blob);
    self->output = std::move(reply.bytes);
    return reply.status;
}

// 直前の zumen_cpp_engine_call・zumen_cpp_engine_take が書いたバッファー。書かれていなければ ptr は NULL
extern "C" void zumen_cpp_engine_output(const void *engine, const std::uint8_t **ptr, std::size_t *len) {
    const zumen::Bytes &output = static_cast<const Engine *>(engine)->output;
    *ptr = output.data();
    *len = output.size();
}

// シーンの blob を読んで数える。読めたら 0、読めなければ 1 を返す
extern "C" std::int32_t zumen_cpp_read_scene(const std::uint8_t *blob, std::size_t len, zumen_cpp_scene_counts *out) {
    try {
        zumen::Scene scene = zumen::Scene::read(blob, len);
        out->major = scene.major();
        out->sections = scene.sections();
        out->chunks = scene.chunks();
        out->chunk_origins = scene.rows(ZM_SCENE_SECTION_CHUNK_ORIGINS);
        out->text_xy = scene.rows(ZM_SCENE_SECTION_TEXT_XY);
        out->point_xy = scene.rows(ZM_SCENE_SECTION_POINT_XY);
        out->glyph_xy = scene.rows(ZM_SCENE_SECTION_GLYPH_XY);
        out->line_and_face_xy = scene.rows(ZM_SCENE_SECTION_LINE_XY) + scene.rows(ZM_SCENE_SECTION_FACE_XY);
        out->bad_texts = 0;
        zumen::Column<std::uint32_t> strings = scene.column<std::uint32_t>(ZM_SCENE_SECTION_TEXT_STRINGS);
        for (std::size_t row = 0; row < strings.rows(); ++row) {
            try {
                scene.text(strings.at(row, 0), strings.at(row, 1));
            } catch (const std::out_of_range &) {
                out->bad_texts += 1;
            }
        }
        // 型の違う列を引くと投げる
        try {
            scene.column<float>(ZM_SCENE_SECTION_CHUNK_ORIGINS);
            return 1;
        } catch (const std::invalid_argument &) {
        }
        return 0;
    } catch (const std::exception &) {
        return 1;
    }
}
