// 生成したヘッダー（include/zumen.h）を C++ から使う試験。tests/smoke.rs が呼ぶ。
// extern "C" の囲みと型が、C++17 で通ることを確かめる

#include <cstdint>

#include "zumen.h"

extern "C" int zumen_cpp_smoke(void) {
    zm_session *session = nullptr;
    if (zm_session_new(nullptr, 0, &session) != ZM_OK || session == nullptr) {
        return __LINE__;
    }
    const char request[] = R"({"v":1,"op":"capabilities"})";
    zm_buf response{nullptr, 0};
    if (zm_call(session, reinterpret_cast<const std::uint8_t *>(request), sizeof request - 1, &response) != ZM_OK) {
        return __LINE__;
    }
    if (response.ptr == nullptr || response.len == 0) {
        return __LINE__;
    }
    zm_free(&response);
    zm_session_close(session);
    return 0;
}
