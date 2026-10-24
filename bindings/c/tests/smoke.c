/* 生成したヘッダー（include/zumen.h）を C から使う試験。tests/smoke.rs が呼ぶ */

#include <string.h>

#include "zumen.h"

/* 満たさなければ、その行の番号を返す。MSVC の /W4 が do { } while (0) に警告を出すことがあるので、if だけにする */
#define EXPECT(condition) \
    if (!(condition))     \
    return __LINE__

/* バッファが、text を含むか */
static int contains(const zm_buf *buf, const char *text) {
    size_t length = strlen(text);
    size_t i;
    if (buf->ptr == NULL || buf->len < length) {
        return 0;
    }
    for (i = 0; i + length <= buf->len; i++) {
        if (memcmp(buf->ptr + i, text, length) == 0) {
            return 1;
        }
    }
    return 0;
}

int zumen_c_smoke(void) {
    static const char capabilities[] = "{\"v\":1,\"op\":\"capabilities\"}";
    static const char unknown[] = "{\"v\":1,\"op\":\"draw\"}";
    static const char options[] = "{\"v\":1,\"engine\":\"native\"}";
    zm_session *session = NULL;
    zm_buf response = {NULL, 0};
    struct ArrowArrayStream stream;

    EXPECT(zm_abi_version() == (uint32_t)ZM_ABI_VERSION);
    EXPECT(zm_session_new((const uint8_t *)options, sizeof options - 1, &session) == ZM_OK);
    EXPECT(session != NULL);

    EXPECT(zm_call(session, (const uint8_t *)capabilities, sizeof capabilities - 1, &response) == ZM_OK);
    EXPECT(contains(&response, "\"abi\":1"));
    zm_free(&response);
    EXPECT(response.ptr == NULL && response.len == 0);

    EXPECT(zm_call(session, (const uint8_t *)unknown, sizeof unknown - 1, &response) == ZM_FAILED);
    EXPECT(contains(&response, "\"code\":\"call.op.unsupported\""));
    zm_free(&response);

    EXPECT(zm_push(session, (const uint8_t *)"a.txt", 5, (const uint8_t *)"x", 1) == ZM_OK);
    EXPECT(zm_push(session, (const uint8_t *)"../a", 4, NULL, 0) == ZM_INVALID);
    EXPECT(zm_call(session, NULL, 1, &response) == ZM_INVALID);
    EXPECT(zm_take(session, 0, &response) == ZM_INVALID);
    memset(&stream, 0, sizeof stream);
    EXPECT(zm_take_arrow(session, 0, &stream) == ZM_INVALID);

    zm_cancel(session);
    zm_session_close(session);
    zm_session_close(NULL);
    zm_free(NULL);
    return 0;
}
