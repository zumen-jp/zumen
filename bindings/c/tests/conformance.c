/* 適合試験の C の側。生成したヘッダー（include/zumen.h）だけで境界の関数を呼ぶ。tests/conformance.rs が、
 * bindings/conformance の組の手順をここに渡し、応答のハッシュを hashes.txt と比べる（決定 0198） */

#include <stdint.h>
#include <stdlib.h>

#include "zumen.h"

/* 1 つの試験の組の、セッションと直前の結果 */
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
