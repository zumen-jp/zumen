package dev.zumen;

import com.sun.jna.Pointer;
import java.util.Map;

/**
 * セッション。入力と、直前の呼び出しのバイナリの結果を持つ。使い終わったら {@link #close()} で閉じる。
 *
 * <p>try-with-resources の文で使うと、抜けるときに閉じる。1 つのセッションは、一度に 1 つのスレッドだけが使う。
 * {@link #cancel()} だけは、どのスレッドからも呼べる。
 */
public final class Session implements AutoCloseable {
    private final Zumen zumen;
    private volatile Pointer handle;

    Session(Zumen zumen, Pointer handle) {
        this.zumen = zumen;
        this.handle = handle;
    }

    /** 閉じたか */
    public boolean isClosed() {
        return handle == null;
    }

    /** 名前のついた入力に、バイト列を足す。同じ名前で何回か呼ぶと、後ろに足していく */
    public void push(String name, byte[] data) {
        push(Zumen.utf8(name), data);
    }

    /** 名前（UTF-8 のバイト列）のついた入力に、バイト列を足す */
    public void push(byte[] name, byte[] data) {
        int code = zumen.core().zm_push(
                handle,
                Zumen.nonEmpty(name),
                new Zumen.SizeT(name.length),
                Zumen.nonEmpty(data),
                new Zumen.SizeT(data.length));
        if (code != Constants.Status.OK) {
            throw new ZumenException("zm_push", code, null);
        }
    }

    /** 要求の JSON（UTF-8 のバイト列）を渡し、応答の JSON のバイト列を受け取る */
    public byte[] callBytes(final byte[] request) {
        final Pointer session = handle;
        Zumen.Written written = zumen.withBuf(new Zumen.WithBuf() {
            @Override
            public int call(Zumen.Buf buf) {
                return zumen.core().zm_call(
                        session, Zumen.nonEmpty(request), new Zumen.SizeT(request.length), buf);
            }
        });
        if (written.code != Constants.Status.OK) {
            throw new ZumenException("zm_call", written.code, written.data);
        }
        return written.data;
    }

    /** 要求の JSON の文字列を渡し、応答の JSON の文字列を受け取る */
    public String callText(String request) {
        return Json.decode(callBytes(Zumen.utf8(request)));
    }

    /** 要求を渡し、応答を受け取る。失敗した応答は、{@link ZumenException#getResponse()} にある */
    public Messages.Response call(Messages.Message request) {
        return call(request.json());
    }

    /** 要求を JSON のオブジェクト（{@link Map}）で渡し、応答を受け取る */
    public Messages.Response call(Map<String, Object> request) {
        Messages.Response response = Messages.Response.of(Json.parse(callBytes(Json.bytes(request))));
        if (response == null) {
            throw new IllegalStateException("応答が JSON のオブジェクトでない");
        }
        return response;
    }

    /** 直前の call が作ったバイナリの結果を、応答の blobs にある番号で取り出す */
    public byte[] take(final int blob) {
        final Pointer session = handle;
        Zumen.Written written = zumen.withBuf(new Zumen.WithBuf() {
            @Override
            public int call(Zumen.Buf buf) {
                return zumen.core().zm_take(session, blob, buf);
            }
        });
        if (written.code != Constants.Status.OK) {
            throw new ZumenException("zm_take", written.code, null);
        }
        return written.data;
    }

    /** 動いている呼び出しを止める。動いている呼び出しがなければ、何もしない。どのスレッドからも呼べる */
    public void cancel() {
        Pointer session = handle;
        if (session != null) {
            zumen.core().zm_cancel(session);
        }
    }

    /** セッションを閉じる。閉じたあとの呼び出しは、invalid の {@link ZumenException} になる */
    @Override
    public void close() {
        Pointer session;
        synchronized (this) {
            session = handle;
            handle = null;
        }
        if (session != null) {
            zumen.core().zm_session_close(session);
        }
    }
}
