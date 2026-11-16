/* ZMS3 の C の読み手。blob の寿命と変更しないことは呼び出し側で保つ。 */
#ifndef ZUMEN_SCENE3D_READER_H
#define ZUMEN_SCENE3D_READER_H
#include "zumen_scene3d.h"
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct zm_scene3d_part {
    const uint8_t *data;
    uint32_t length;
} zm_scene3d_part;

typedef struct zm_scene3d_reader {
    uint16_t major, minor;
    uint32_t chunks, sections;
    zm_scene3d_part parts[ZM_SCENE3D_SECTION_COUNT];
} zm_scene3d_reader;

typedef struct zm_scene3d_instance {
    uint32_t mesh, element, color;
    float transform[12];
} zm_scene3d_instance;

static inline uint16_t zm_scene3d_u16_le(const uint8_t *p) {
    return (uint16_t)((uint16_t)p[0] | (uint16_t)((uint16_t)p[1] << 8));
}
static inline uint32_t zm_scene3d_u32_le(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

/* 正常なら 1。失敗では out を変えない。基本列・範囲・幅を検査する。点のない 1.0 は 20 列、点を持つ 1.1 以降は全列を持つ。 */
static inline int zm_scene3d_read(const uint8_t *data, size_t size, zm_scene3d_reader *out) {
    zm_scene3d_reader result;
    uint32_t i;
    uint64_t previous;
    if (data == NULL || out == NULL || size < 16 || size > UINT64_C(536870912) || memcmp(data, ZM_SCENE3D_MAGIC, 4) != 0) return 0;
    memset(&result, 0, sizeof result);
    result.major = zm_scene3d_u16_le(data + 4);
    result.minor = zm_scene3d_u16_le(data + 6);
    result.chunks = zm_scene3d_u32_le(data + 8);
    result.sections = zm_scene3d_u32_le(data + 12);
    previous = 16 + (uint64_t)result.sections * 8;
    if (result.major != ZM_SCENE3D_MAJOR || (result.sections != ZM_SCENE3D_SECTION_STRINGS + 1 && result.sections < ZM_SCENE3D_SECTION_COUNT) || previous > size) return 0;
    for (i = 0; i < result.sections; ++i) {
        uint32_t start = zm_scene3d_u32_le(data + 16 + (size_t)i * 8);
        uint32_t length = zm_scene3d_u32_le(data + 20 + (size_t)i * 8);
        if (start % 8 != 0 || start < previous || (uint64_t)start + length > size) return 0;
        previous = (uint64_t)start + length;
        if (i < ZM_SCENE3D_SECTION_COUNT) {
            const zm_scene3d_section *section = zm_scene3d_section_at(i);
            uint32_t row = zm_scene3d_type_size(section->type) * section->width;
            if (row == 0 || length % row != 0) return 0;
            result.parts[i].data = data + start;
            result.parts[i].length = length;
        }
    }
    *out = result;
    return 1;
}

/* 行数。知らないセクションなら 0。 */
static inline uint32_t zm_scene3d_rows(const zm_scene3d_reader *scene, uint32_t section) {
    const zm_scene3d_section *definition = zm_scene3d_section_at(section);
    if (scene == NULL || definition == NULL) return 0;
    return scene->parts[section].length / (zm_scene3d_type_size(definition->type) * definition->width);
}

static inline const uint8_t *zm_scene3d_value_at(const zm_scene3d_reader *scene, uint32_t section, uint32_t row, uint32_t component, int32_t type) {
    const zm_scene3d_section *definition = zm_scene3d_section_at(section);
    if (scene == NULL || definition == NULL || definition->type != type || row >= zm_scene3d_rows(scene, section) || component >= definition->width) return NULL;
    return scene->parts[section].data + ((size_t)row * definition->width + component) * zm_scene3d_type_size(type);
}

static inline int zm_scene3d_u32_at(const zm_scene3d_reader *scene, uint32_t section, uint32_t row, uint32_t component, uint32_t *out) {
    const uint8_t *p = zm_scene3d_value_at(scene, section, row, component, ZM_SCENE3D_TYPE_U32);
    if (p == NULL || out == NULL) return 0;
    *out = zm_scene3d_u32_le(p);
    return 1;
}

static inline int zm_scene3d_u16_at(const zm_scene3d_reader *scene, uint32_t section, uint32_t row, uint32_t component, uint16_t *out) {
    const uint8_t *p = zm_scene3d_value_at(scene, section, row, component, ZM_SCENE3D_TYPE_U16);
    if (p == NULL || out == NULL) return 0;
    *out = zm_scene3d_u16_le(p);
    return 1;
}

static inline int zm_scene3d_f32_at(const zm_scene3d_reader *scene, uint32_t section, uint32_t row, uint32_t component, float *out) {
    const uint8_t *p = zm_scene3d_value_at(scene, section, row, component, ZM_SCENE3D_TYPE_F32);
    uint32_t bits;
    if (p == NULL || out == NULL || sizeof(float) != 4) return 0;
    bits = zm_scene3d_u32_le(p);
    memcpy(out, &bits, sizeof bits);
    return 1;
}

static inline int zm_scene3d_f64_at(const zm_scene3d_reader *scene, uint32_t section, uint32_t row, uint32_t component, double *out) {
    const uint8_t *p = zm_scene3d_value_at(scene, section, row, component, ZM_SCENE3D_TYPE_F64);
    uint64_t bits;
    if (p == NULL || out == NULL || sizeof(double) != 8) return 0;
    bits = (uint64_t)zm_scene3d_u32_le(p) | ((uint64_t)zm_scene3d_u32_le(p + 4) << 32);
    memcpy(out, &bits, sizeof bits);
    return 1;
}

/* UTF-8 の要素 ID。終端 NUL は含めず、data と length で返す。 */
static inline int zm_scene3d_element_id(const zm_scene3d_reader *scene, uint32_t row, zm_scene3d_part *out) {
    uint32_t start, length;
    const zm_scene3d_part *strings;
    if (out == NULL || !zm_scene3d_u32_at(scene, ZM_SCENE3D_SECTION_ELEMENT_IDS, row, 0, &start)
        || !zm_scene3d_u32_at(scene, ZM_SCENE3D_SECTION_ELEMENT_IDS, row, 1, &length)) return 0;
    strings = &scene->parts[ZM_SCENE3D_SECTION_STRINGS];
    if ((uint64_t)start + length > strings->length) return 0;
    out->data = strings->data + start;
    out->length = length;
    return 1;
}

static inline int zm_scene3d_instance_at(const zm_scene3d_reader *scene, uint32_t row, zm_scene3d_instance *out) {
    zm_scene3d_instance value;
    uint32_t i;
    if (out == NULL || !zm_scene3d_u32_at(scene, ZM_SCENE3D_SECTION_INSTANCE_MESHES, row, 0, &value.mesh)
        || !zm_scene3d_u32_at(scene, ZM_SCENE3D_SECTION_INSTANCE_ELEMENTS, row, 0, &value.element)
        || !zm_scene3d_u32_at(scene, ZM_SCENE3D_SECTION_INSTANCE_COLORS, row, 0, &value.color)) return 0;
    if (value.mesh >= zm_scene3d_rows(scene, ZM_SCENE3D_SECTION_MESH_VERTICES)
        || value.element >= zm_scene3d_rows(scene, ZM_SCENE3D_SECTION_ELEMENT_IDS)
        || value.color >= zm_scene3d_rows(scene, ZM_SCENE3D_SECTION_COLOR_RGBA)) return 0;
    for (i = 0; i < 12; ++i) if (!zm_scene3d_f32_at(scene, ZM_SCENE3D_SECTION_INSTANCE_TRANSFORMS, row, i, &value.transform[i])) return 0;
    *out = value;
    return 1;
}
#ifdef __cplusplus
}
#endif
#endif
