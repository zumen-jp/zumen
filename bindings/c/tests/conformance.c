/* 適合テストの C の側。生成したヘッダー（include/zumen.h）だけで API の関数を呼ぶ。tests/conformance.rs が、
 * bindings/conformance の組の手順をここに渡し、応答のハッシュを hashes.txt と比べる（決定 0198） */

#include <stdint.h>
#include <stdlib.h>

#include "zumen.h"

/* 1 つのテストの組の、セッションと直前の結果 */
typedef struct zumen_c_engine {
    zm_session *session;
    zm_buf output;
} zumen_c_engine;

zumen_c_engine *zumen_c_engine_new(void) {
    return (zumen_c_engine *)calloc(1, sizeof(zumen_c_engine));
}

void zumen_c_engine_free(zumen_c_engine *engine) {
    if (engine == NULL) {
        return;
    }
    zm_free(&engine->output);
    zm_session_close(engine->session);
    free(engine);
}

int32_t zumen_c_engine_session(zumen_c_engine *engine, const uint8_t *options, size_t options_len) {
    zm_session_close(engine->session);
    engine->session = NULL;
    return zm_session_new(options, options_len, &engine->session);
}

int32_t zumen_c_engine_push(zumen_c_engine *engine, const uint8_t *name, size_t name_len, const uint8_t *data,
                            size_t data_len) {
    return zm_push(engine->session, name, name_len, data, data_len);
}

int32_t zumen_c_engine_call(zumen_c_engine *engine, const uint8_t *request, size_t request_len) {
    zm_free(&engine->output);
    return zm_call(engine->session, request, request_len, &engine->output);
}

int32_t zumen_c_engine_take(zumen_c_engine *engine, uint32_t blob) {
    zm_free(&engine->output);
    return zm_take(engine->session, blob, &engine->output);
}

/* 直前の zumen_c_engine_call・zumen_c_engine_take が書いたバッファー。書かれていなければ ptr は NULL */
void zumen_c_engine_output(const zumen_c_engine *engine, const uint8_t **ptr, size_t *len) {
    *ptr = engine->output.ptr;
    *len = engine->output.len;
}

#include "zumen_scene3d_reader.h"

/* 共有の 8 種の数に加え、列の型・インスタンス・要素 ID を確認する。 */
int32_t zumen_c_read_scene3d(const uint8_t *blob, size_t len, uint64_t *out) {
    zm_scene3d_reader scene;
    const uint32_t sections[] = {ZM_SCENE3D_SECTION_CHUNK_ORIGINS, ZM_SCENE3D_SECTION_MESH_VERTICES, ZM_SCENE3D_SECTION_INSTANCE_MESHES, ZM_SCENE3D_SECTION_ELEMENT_IDS, ZM_SCENE3D_SECTION_COLOR_RGBA, ZM_SCENE3D_SECTION_VERTEX_XYZ, ZM_SCENE3D_SECTION_TRIANGLE_VERTICES, ZM_SCENE3D_SECTION_EDGE_VERTICES};
    uint32_t row, column;
    if (!zm_scene3d_read(blob, len, &scene)) return 1;
    if (scene.major != ZM_SCENE3D_MAJOR || scene.sections != ZM_SCENE3D_SECTION_COUNT) return 1;
    for (column = 0; column < 8; ++column) out[column] = zm_scene3d_rows(&scene, sections[column]);
    if (scene.chunks != out[0]) return 1;
    for (row = 0; row < out[2]; ++row) {
        zm_scene3d_instance instance;
        zm_scene3d_part id;
        if (!zm_scene3d_instance_at(&scene, row, &instance)
            || !zm_scene3d_element_id(&scene, instance.element, &id) || id.length == 0) return 1;
    }
    for (column = 0; column < ZM_SCENE3D_SECTION_COUNT; ++column) {
        const zm_scene3d_section *definition = zm_scene3d_section_at(column);
        if (zm_scene3d_rows(&scene, column) > 0) {
            double f64;
            float f32;
            uint32_t u32;
            if (definition->type == ZM_SCENE3D_TYPE_F64 && !zm_scene3d_f64_at(&scene, column, 0, 0, &f64)) return 1;
            if (definition->type == ZM_SCENE3D_TYPE_F32 && !zm_scene3d_f32_at(&scene, column, 0, 0, &f32)) return 1;
            if (definition->type == ZM_SCENE3D_TYPE_U32 && !zm_scene3d_u32_at(&scene, column, 0, 0, &u32)) return 1;
        }
    }
    return 0;
}
