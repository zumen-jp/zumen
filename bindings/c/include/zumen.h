/* 生成したファイル。手で直さない。
 * 元は schema/boundary.toml で、`cargo xtask codegen` で作り直す。 */

/*
 * zumen の境界（決定 0012）。
 *
 * - 文字列は UTF-8。要求と応答は JSON で、形は schema/messages.json にある
 * - 入力のバッファーは呼び出しの間だけ借りる。コアが返したバッファーは zm_free で返す
 * - 1 つのセッションは、一度に 1 つのスレッドだけが使う。zm_cancel だけは、どのスレッドからも呼べる
 * - 関数・状態コードは足すだけにし、消さず、名前・値・引数・意味を変えない（決定 0015）
 */

#ifndef ZUMEN_H
#define ZUMEN_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* 呼び出し規約。32 bit の Windows では stdcall にする（VBA の Declare のため、決定 0011） */
#if defined(_WIN32) && !defined(_WIN64)
#define ZM_CALL __stdcall
#else
#define ZM_CALL
#endif

/* ABI の大版。zm_abi_version が返す */
#define ZM_ABI_VERSION 1
/* メッセージのスキーマの版。大版は要求と応答の v、小版は capabilities が返す */
#define ZM_SCHEMA_VERSION_MAJOR 1
#define ZM_SCHEMA_VERSION_MINOR 45

/* 状態コード。理由は応答の診断で返す */
typedef int32_t zm_status;
/* 成功した */
#define ZM_OK 0
/* 操作が失敗した。応答に error の診断がある */
#define ZM_FAILED 1
/* 引数が使えない（NULL、長さが合わない、使えない名前など）。応答は書かない */
#define ZM_INVALID 2
/* 別のスレッドが、同じセッションを使っている */
#define ZM_BUSY 3
/* コアの不具合で止まった。セッションは閉じるしかない */
#define ZM_PANICKED 4

/* セッション。中身はコアだけが知る */
typedef struct zm_session zm_session;

/* コアが返すバッファー。zm_free で返す */
typedef struct zm_buf {
    uint8_t *ptr;
    size_t len;
} zm_buf;

/* Arrow の C Data Interface と C Stream Interface。Arrow の文書が勧めるとおりの定義と目印
 * https://arrow.apache.org/docs/format/CDataInterface.html
 * https://arrow.apache.org/docs/format/CStreamInterface.html */
#ifndef ARROW_C_DATA_INTERFACE
#define ARROW_C_DATA_INTERFACE

#define ARROW_FLAG_DICTIONARY_ORDERED 1
#define ARROW_FLAG_NULLABLE 2
#define ARROW_FLAG_MAP_KEYS_SORTED 4

struct ArrowSchema {
    const char *format;
    const char *name;
    const char *metadata;
    int64_t flags;
    int64_t n_children;
    struct ArrowSchema **children;
    struct ArrowSchema *dictionary;
    void (*release)(struct ArrowSchema *);
    void *private_data;
};

struct ArrowArray {
    int64_t length;
    int64_t null_count;
    int64_t offset;
    int64_t n_buffers;
    int64_t n_children;
    const void **buffers;
    struct ArrowArray **children;
    struct ArrowArray *dictionary;
    void (*release)(struct ArrowArray *);
    void *private_data;
};

#endif /* ARROW_C_DATA_INTERFACE */

#ifndef ARROW_C_STREAM_INTERFACE
#define ARROW_C_STREAM_INTERFACE

struct ArrowArrayStream {
    int (*get_schema)(struct ArrowArrayStream *, struct ArrowSchema *out);
    int (*get_next)(struct ArrowArrayStream *, struct ArrowArray *out);
    const char *(*get_last_error)(struct ArrowArrayStream *);
    void (*release)(struct ArrowArrayStream *);
    void *private_data;
};

#endif /* ARROW_C_STREAM_INTERFACE */

/* ABI の大版を返す。この関数の形は変えない */
uint32_t ZM_CALL zm_abi_version(void);

/*
 * セッションを作る。設定は JSON（schema/messages.json の session_options）で渡す
 *
 * options: 設定の JSON（UTF-8）。長さが 0 なら既定の設定
 * options_len: 設定のバイト数
 * session: 作ったセッションを書く所。zm_session_close で閉じる
 */
zm_status ZM_CALL zm_session_new(const uint8_t *options, size_t options_len, zm_session **session);

/*
 * 名前のついた入力に、バイト列を足す。同じ名前で何回か呼ぶと、後ろに足していく
 *
 * session: セッション
 * name: 入力の名前（UTF-8）。/ で区切った相対の名前で、空の部分・.・..・\・:・NUL を含まない
 * name_len: 名前のバイト数
 * data: 足すバイト列。呼び出しの間だけ借りる
 * data_len: 足すバイト数
 */
zm_status ZM_CALL zm_push(zm_session *session, const uint8_t *name, size_t name_len, const uint8_t *data, size_t data_len);

/*
 * 要求のメッセージを渡し、応答のメッセージを受け取る。前の呼び出しのバイナリの結果は、ここで捨てる
 *
 * session: セッション
 * request: 要求の JSON（UTF-8）
 * request_len: 要求のバイト数
 * response: 応答の JSON を書く所。状態が ZM_OK・ZM_FAILED・ZM_PANICKED のときに書く。zm_free で返す
 */
zm_status ZM_CALL zm_call(zm_session *session, const uint8_t *request, size_t request_len, zm_buf *response);

/*
 * 直前の zm_call が作ったバイナリの結果を取り出す。取り出したものは、セッションを閉じたあとも使える
 *
 * session: セッション
 * blob: 応答の blobs にある番号
 * out: 結果を書く所。zm_free で返す
 */
zm_status ZM_CALL zm_take(zm_session *session, uint32_t blob, zm_buf *out);

/*
 * 表の結果（種類が arrow のもの）を、Arrow の C Stream Interface で取り出す
 *
 * session: セッション
 * blob: 応答の blobs にある番号
 * out: 結果を書く所。使い終わったら、その release を呼ぶ
 */
zm_status ZM_CALL zm_take_arrow(zm_session *session, uint32_t blob, struct ArrowArrayStream *out);

/*
 * 動いている呼び出しを止める。どのスレッドからも呼べる。動いている呼び出しがなければ、何もしない
 *
 * session: セッション
 */
void ZM_CALL zm_cancel(zm_session *session);

/*
 * セッションを閉じる。NULL なら何もしない。別のスレッドが使っている途中なら閉じない
 *
 * session: セッション
 */
void ZM_CALL zm_session_close(zm_session *session);

/*
 * コアが返したバッファーを返す。返したあとの buf は空になる。空のバッファーと NULL には何もしない
 *
 * buf: zm_call・zm_take が書いたバッファー
 */
void ZM_CALL zm_free(zm_buf *buf);

#ifdef __cplusplus
}
#endif

#endif /* ZUMEN_H */
