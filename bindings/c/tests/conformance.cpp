// 適合テストの C++ の側。ラッパー（include/zumen.hpp）で API の関数を呼び、シーンの blob を zumen::Scene で読む。
// tests/conformance.rs が、bindings/conformance の組の手順をここに渡し、応答のハッシュを hashes.txt と比べる（決定 0198）

#include <cstddef>
#include <cstdint>
#include <exception>
#include <new>

#include "zumen.hpp"

namespace {

// 1 つのテストの組の、セッションと直前の結果
struct Engine {
    zumen::Session session;
    zumen::Bytes output;
};

} // namespace

// シーンをリーダーで読んで数えたもの
struct zumen_cpp_scene_counts {
    std::uint32_t major;
    std::uint32_t sections;
    std::uint32_t chunks;
    std::uint64_t chunk_origins;
    std::uint64_t text_xy;
    std::uint64_t point_xy;
    std::uint64_t glyph_xy;
    std::uint64_t line_and_face_xy;
    // text_strings の行のうち、strings のセクションから引けなかったもの
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

extern "C" std::int32_t zumen_cpp_read_scene3d(const std::uint8_t *blob, std::size_t len, std::uint64_t *out) {
    try {
        const auto scene = zumen::Scene3D::read(blob, len);
        const std::uint32_t sections[] = {ZM_SCENE3D_SECTION_CHUNK_ORIGINS, ZM_SCENE3D_SECTION_MESH_VERTICES, ZM_SCENE3D_SECTION_INSTANCE_MESHES, ZM_SCENE3D_SECTION_ELEMENT_IDS, ZM_SCENE3D_SECTION_COLOR_RGBA, ZM_SCENE3D_SECTION_VERTEX_XYZ, ZM_SCENE3D_SECTION_TRIANGLE_VERTICES, ZM_SCENE3D_SECTION_EDGE_VERTICES};
        if (scene.major() != ZM_SCENE3D_MAJOR || scene.sections() != ZM_SCENE3D_SECTION_COUNT) return 1;
        for (std::size_t column = 0; column < 8; ++column) out[column] = scene.rows(sections[column]);
        if (scene.chunks() != out[0]) return 1;
        for (std::size_t row = 0; row < out[2]; ++row) {
            if (scene.element_id(scene.instance_element(row)).empty()) return 1;
            scene.instance_mesh(row);
            scene.instance_color(row);
        }
        for (std::uint32_t column = 0; column < ZM_SCENE3D_SECTION_COUNT; ++column) {
            const auto *definition = zm_scene3d_section_at(column);
            if (scene.rows(column) == 0) continue;
            if (definition->type == ZM_SCENE3D_TYPE_F64) scene.column<double>(column).at(0);
            if (definition->type == ZM_SCENE3D_TYPE_F32) scene.column<float>(column).at(0);
            if (definition->type == ZM_SCENE3D_TYPE_U32) scene.column<std::uint32_t>(column).at(0);
            if (definition->type == ZM_SCENE3D_TYPE_U8) scene.column<std::uint8_t>(column).at(0);
        }
        try { scene.column<float>(ZM_SCENE3D_SECTION_CHUNK_ORIGINS); return 1; }
        catch (const std::invalid_argument &) {}
        return 0;
    } catch (const std::exception &) { return 1; }
}
