// zumen の C++ の包み（決定 0011・0198）。ヘッダーだけで、C++17 で使える。
//
// - zumen.h の境界の関数（決定 0012）を、RAII のクラスで包む。セッションは Session、コアが返したバッファーは Bytes が持ち、
//   壊すときに zm_session_close・zm_free で返す
// - 要求と応答は JSON の文字列のまま渡す。形は schema/messages.json にある。JSON の読み書きには、好きなライブラリを使う
// - 状態が ZM_OK でなければ、zumen::Error を投げる。投げない try_ の関数もある。failed の応答は、Error::response で読める
// - シーンの blob は zumen::Scene で読む。区画の定義は zumen_scene.h に生成する
//
// このファイルは手で書く。生成した定数は zumen.h と zumen_scene.h にある。

#ifndef ZUMEN_HPP
#define ZUMEN_HPP

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>

#include "zumen.h"
#include "zumen_scene.h"

namespace zumen {

/// 状態コードの名前（ok・failed など）。知らない値なら空
inline std::string_view status_name(zm_status status) noexcept {
    switch (status) {
    case ZM_OK:
        return "ok";
    case ZM_FAILED:
        return "failed";
    case ZM_INVALID:
        return "invalid";
    case ZM_BUSY:
        return "busy";
    case ZM_PANICKED:
        return "panicked";
    default:
        return {};
    }
}

/// コアが返したバッファー。壊すときに zm_free で返す。写さずに見る
class Bytes {
public:
    Bytes() noexcept = default;
    Bytes(const Bytes &) = delete;
    Bytes &operator=(const Bytes &) = delete;
    Bytes(Bytes &&other) noexcept : buf_(other.buf_) { other.buf_ = zm_buf{nullptr, 0}; }
    Bytes &operator=(Bytes &&other) noexcept {
        if (this != &other) {
            reset();
            buf_ = other.buf_;
            other.buf_ = zm_buf{nullptr, 0};
        }
        return *this;
    }
    ~Bytes() { reset(); }

    /// 先頭の番地。空なら nullptr のことがある
    const std::uint8_t *data() const noexcept { return buf_.ptr; }
    /// バイト数
    std::size_t size() const noexcept { return buf_.len; }
    /// 空か
    bool empty() const noexcept { return buf_.len == 0; }
    /// 文字列として見る（応答の JSON は UTF-8）
    std::string_view text() const noexcept {
        return buf_.ptr == nullptr ? std::string_view{} : std::string_view(reinterpret_cast<const char *>(buf_.ptr), buf_.len);
    }
    /// zm_call・zm_take に渡す書き込み先。前の中身は返す
    zm_buf *out() noexcept {
        reset();
        return &buf_;
    }
    /// 中身を zm_free で返し、空にする
    void reset() noexcept { zm_free(&buf_); }

private:
    zm_buf buf_{nullptr, 0};
};

/// 状態が ZM_OK でなかった
class Error : public std::runtime_error {
public:
    Error(zm_status status, std::string response)
        : std::runtime_error("zumen: " + std::string(status_name(status).empty() ? "unknown" : status_name(status))),
          status_(status), response_(std::move(response)) {}

    /// 状態コード
    zm_status status() const noexcept { return status_; }
    /// 応答の JSON。境界が応答を書かなかった（invalid・busy）なら空
    const std::string &response() const noexcept { return response_; }

private:
    zm_status status_;
    std::string response_;
};

/// 状態と、書かれたバッファー
struct Reply {
    /// 状態コード
    zm_status status = ZM_INVALID;
    /// 応答の JSON か、取り出した結果。状態によっては空
    Bytes bytes;
};

namespace detail {

inline const std::uint8_t *bytes_of(std::string_view text) noexcept {
    // 長さ 0 でも、NULL でない番地を渡す
    static const std::uint8_t empty = 0;
    return text.empty() ? &empty : reinterpret_cast<const std::uint8_t *>(text.data());
}

inline Bytes check(Reply reply) {
    if (reply.status != ZM_OK) {
        throw Error(reply.status, std::string(reply.bytes.text()));
    }
    return std::move(reply.bytes);
}

} // namespace detail

/// セッション（決定 0012）。壊すときに閉じる。1 つのセッションは、一度に 1 つのスレッドだけが使う
class Session {
public:
    /// 閉じたセッション。呼び出しは NULL のまま境界に渡り、invalid になる
    Session() noexcept = default;
    Session(const Session &) = delete;
    Session &operator=(const Session &) = delete;
    Session(Session &&other) noexcept : handle_(std::exchange(other.handle_, nullptr)) {}
    Session &operator=(Session &&other) noexcept {
        if (this != &other) {
            close();
            handle_ = std::exchange(other.handle_, nullptr);
        }
        return *this;
    }
    ~Session() { close(); }

    /// セッションを作る。設定は JSON（session_options）で、空なら既定の設定
    static Session open(std::string_view options = {}) {
        Session session;
        zm_status status = session.try_open(options);
        if (status != ZM_OK) {
            throw Error(status, std::string());
        }
        return session;
    }

    /// 前のセッションを閉じて、作り直す。投げない
    zm_status try_open(std::string_view options = {}) noexcept {
        close();
        return zm_session_new(detail::bytes_of(options), options.size(), &handle_);
    }

    /// 名前のついた入力に、バイト列を足す
    void push(std::string_view name, const void *data, std::size_t size) {
        zm_status status = try_push(name, data, size);
        if (status != ZM_OK) {
            throw Error(status, std::string());
        }
    }
    /// 名前のついた入力に、文字列のバイト列を足す
    void push(std::string_view name, std::string_view data) { push(name, data.data(), data.size()); }
    /// push の投げない形
    zm_status try_push(std::string_view name, const void *data, std::size_t size) noexcept {
        const std::uint8_t *bytes = size == 0 ? detail::bytes_of({}) : static_cast<const std::uint8_t *>(data);
        return zm_push(handle_, detail::bytes_of(name), name.size(), bytes, size);
    }

    /// 要求の JSON を渡し、応答の JSON を受け取る
    Bytes call(std::string_view request) { return detail::check(try_call(request)); }
    /// call の投げない形。failed の応答も受け取る
    Reply try_call(std::string_view request) noexcept {
        Reply reply;
        reply.status = zm_call(handle_, detail::bytes_of(request), request.size(), reply.bytes.out());
        return reply;
    }

    /// 直前の call が作ったバイナリの結果を、応答の blobs の番号で取り出す
    Bytes take(std::uint32_t blob) { return detail::check(try_take(blob)); }
    /// take の投げない形
    Reply try_take(std::uint32_t blob) noexcept {
        Reply reply;
        reply.status = zm_take(handle_, blob, reply.bytes.out());
        return reply;
    }

    /// 動いている呼び出しを止める。どのスレッドからも呼べる
    void cancel() noexcept { zm_cancel(handle_); }
    /// 閉じる。閉じたあとの呼び出しは invalid になる
    void close() noexcept {
        zm_session_close(handle_);
        handle_ = nullptr;
    }
    /// 開いているか
    bool is_open() const noexcept { return handle_ != nullptr; }
    /// zumen.h の関数に渡す番地
    zm_session *handle() const noexcept { return handle_; }

private:
    zm_session *handle_ = nullptr;
};

/// シーンの blob が読めない
class SceneError : public std::runtime_error {
public:
    using std::runtime_error::runtime_error;
};

namespace detail {

inline std::uint32_t u32_le(const std::uint8_t *p) noexcept {
    return static_cast<std::uint32_t>(p[0]) | (static_cast<std::uint32_t>(p[1]) << 8) |
           (static_cast<std::uint32_t>(p[2]) << 16) | (static_cast<std::uint32_t>(p[3]) << 24);
}

inline std::uint16_t u16_le(const std::uint8_t *p) noexcept {
    return static_cast<std::uint16_t>(p[0] | (p[1] << 8));
}

template <class T> struct SceneType;
template <> struct SceneType<std::uint8_t> {
    static constexpr std::int32_t code = ZM_SCENE_TYPE_U8;
    static std::uint8_t read(const std::uint8_t *p) noexcept { return *p; }
};
template <> struct SceneType<std::uint32_t> {
    static constexpr std::int32_t code = ZM_SCENE_TYPE_U32;
    static std::uint32_t read(const std::uint8_t *p) noexcept { return u32_le(p); }
};
template <> struct SceneType<float> {
    static constexpr std::int32_t code = ZM_SCENE_TYPE_F32;
    static float read(const std::uint8_t *p) noexcept {
        static_assert(sizeof(float) == 4, "float は 4 バイトの IEEE 754 にする");
        std::uint32_t bits = u32_le(p);
        float value;
        std::memcpy(&value, &bits, sizeof value);
        return value;
    }
};
template <> struct SceneType<double> {
    static constexpr std::int32_t code = ZM_SCENE_TYPE_F64;
    static double read(const std::uint8_t *p) noexcept {
        static_assert(sizeof(double) == 8, "double は 8 バイトの IEEE 754 にする");
        std::uint64_t bits = static_cast<std::uint64_t>(u32_le(p)) | (static_cast<std::uint64_t>(u32_le(p + 4)) << 32);
        double value;
        std::memcpy(&value, &bits, sizeof value);
        return value;
    }
};

} // namespace detail

/// シーンの 1 つの区画（列）。blob を写さずに見て、値はリトルエンディアンから環境の並びに直して返す
template <class T> class Column {
public:
    Column() noexcept = default;
    Column(const std::uint8_t *data, std::size_t values, std::uint32_t width) noexcept
        : data_(data), values_(values), width_(width) {}

    /// 値の数
    std::size_t size() const noexcept { return values_; }
    /// 行の数（値の数を幅で割ったもの）
    std::size_t rows() const noexcept { return width_ == 0 ? 0 : values_ / width_; }
    /// 行ごとの値の数
    std::uint32_t width() const noexcept { return width_; }
    /// i 番目の値。範囲の外なら投げる
    T at(std::size_t i) const {
        if (i >= values_) {
            throw std::out_of_range("zumen: 区画の外を指す");
        }
        return (*this)[i];
    }
    /// i 番目の値。範囲を確かめない
    T operator[](std::size_t i) const noexcept { return detail::SceneType<T>::read(data_ + i * sizeof(T)); }
    /// row 行の column 番目の値
    T at(std::size_t row, std::uint32_t column) const {
        if (column >= width_) {
            throw std::out_of_range("zumen: 行の幅の外を指す");
        }
        return at(row * width_ + column);
    }
    /// リトルエンディアンのバイト列の先頭
    const std::uint8_t *bytes() const noexcept { return data_; }

private:
    const std::uint8_t *data_ = nullptr;
    std::size_t values_ = 0;
    std::uint32_t width_ = 0;
};

/// 読んだシーン（決定 0137・0178）。blob を写さずに見るので、blob を生かしておき、書き換えない
class Scene {
public:
    /// シーンの blob を読む。知らない区画は読み飛ばし、ない区画は空の列にする。大版が違えば読まない
    static Scene read(const std::uint8_t *data, std::size_t size) {
        constexpr std::size_t header = 16;
        if (data == nullptr || size < header || std::memcmp(data, ZM_SCENE_MAGIC, 4) != 0) {
            throw SceneError("zumen: シーンの blob でない");
        }
        Scene scene;
        scene.major_ = detail::u16_le(data + 4);
        scene.minor_ = detail::u16_le(data + 6);
        scene.chunks_ = detail::u32_le(data + 8);
        scene.sections_ = detail::u32_le(data + 12);
        if (scene.major_ != ZM_SCENE_MAJOR) {
            throw SceneError("zumen: シーンの大版が違う");
        }
        if (static_cast<std::uint64_t>(scene.sections_) * 8 > size - header) {
            throw SceneError("zumen: 区画の表が blob に収まらない");
        }
        for (std::uint32_t index = 0; index < ZM_SCENE_SECTION_COUNT && index < scene.sections_; ++index) {
            const zm_scene_section *section = zm_scene_section_at(index);
            std::uint32_t start = detail::u32_le(data + header + 8 * index);
            std::uint32_t length = detail::u32_le(data + header + 8 * index + 4);
            std::uint64_t row = static_cast<std::uint64_t>(zm_scene_type_size(section->type)) * section->width;
            if (static_cast<std::uint64_t>(start) + length > size || row == 0 || length % row != 0) {
                throw SceneError(std::string("zumen: 区画 ") + section->name + " の始まりか長さが合わない");
            }
            scene.parts_[index] = Part{data + start, length};
        }
        return scene;
    }
    /// Bytes から読む。bytes を生かしておく
    static Scene read(const Bytes &bytes) { return read(bytes.data(), bytes.size()); }

    /// 大版
    std::uint16_t major() const noexcept { return major_; }
    /// 小版
    std::uint16_t minor() const noexcept { return minor_; }
    /// 塊の数
    std::uint32_t chunks() const noexcept { return chunks_; }
    /// blob にある区画の数。この包みが知らない区画も数える
    std::uint32_t sections() const noexcept { return sections_; }

    /// 区画（ZM_SCENE_SECTION_*）の列。T は区画の型（std::uint8_t・std::uint32_t・float・double）にする
    template <class T> Column<T> column(std::uint32_t section) const {
        const zm_scene_section *definition = zm_scene_section_at(section);
        if (definition == nullptr) {
            throw std::out_of_range("zumen: 知らない区画");
        }
        if (definition->type != detail::SceneType<T>::code) {
            throw std::invalid_argument(std::string("zumen: 区画 ") + definition->name + " の型と合わない");
        }
        const Part &part = parts_[section];
        return Column<T>(part.data, part.length / sizeof(T), definition->width);
    }

    /// 区画の行の数
    std::size_t rows(std::uint32_t section) const {
        const zm_scene_section *definition = zm_scene_section_at(section);
        if (definition == nullptr) {
            throw std::out_of_range("zumen: 知らない区画");
        }
        return parts_[section].length / (zm_scene_type_size(definition->type) * definition->width);
    }

    /// strings の区画から、始まりと長さ（バイト）で文字列（UTF-8）を引く
    std::string_view text(std::uint32_t start, std::uint32_t length) const {
        const Part &strings = parts_[ZM_SCENE_SECTION_STRINGS];
        if (static_cast<std::uint64_t>(start) + length > strings.length) {
            throw std::out_of_range("zumen: strings の区画の外を指す");
        }
        return std::string_view(reinterpret_cast<const char *>(strings.data) + start, length);
    }

private:
    struct Part {
        const std::uint8_t *data = nullptr;
        std::size_t length = 0;
    };

    std::uint16_t major_ = 0;
    std::uint16_t minor_ = 0;
    std::uint32_t chunks_ = 0;
    std::uint32_t sections_ = 0;
    Part parts_[ZM_SCENE_SECTION_COUNT];
};

} // namespace zumen

#endif // ZUMEN_HPP
