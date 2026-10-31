package dev.zumen;

import com.sun.jna.IntegerType;
import com.sun.jna.Library;
import com.sun.jna.Native;
import com.sun.jna.NativeLibrary;
import com.sun.jna.Platform;
import com.sun.jna.Pointer;
import com.sun.jna.Structure;
import com.sun.jna.ptr.PointerByReference;
import com.sun.jna.win32.StdCallLibrary;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * zumen の C ABI を JNA で呼ぶ薄い包み（決定 0011・0224）。
 *
 * <p>ネイティブのライブラリ（zumen_abi1.dll・libzumen_abi1.so・libzumen_abi1.dylib）を {@link #load(String)} で読み込み、
 * 境界の関数（schema/boundary.toml）を、{@link Session} のメソッドにして出す。要求と応答の型は {@link Messages} にある。
 * Kotlin からも、このクラスをそのまま使う。
 */
public final class Zumen {
    /** ネイティブのライブラリが出す関数（schema/boundary.toml） */
    interface Core extends Library {
        int zm_abi_version();

        int zm_session_new(byte[] options, SizeT optionsLen, PointerByReference session);

        int zm_push(Pointer session, byte[] name, SizeT nameLen, byte[] data, SizeT dataLen);

        int zm_call(Pointer session, byte[] request, SizeT requestLen, Buf response);

        int zm_take(Pointer session, int blob, Buf out);

        void zm_cancel(Pointer session);

        void zm_session_close(Pointer session);

        void zm_free(Buf buf);
    }

    /** C の size_t */
    public static final class SizeT extends IntegerType {
        private static final long serialVersionUID = 1L;

        public SizeT() {
            this(0);
        }

        public SizeT(long value) {
            super(Native.SIZE_T_SIZE, value, true);
        }
    }

    /** コアが返すバッファー（zm_buf）。zm_free で返す */
    @Structure.FieldOrder({"ptr", "len"})
    public static final class Buf extends Structure {
        public Pointer ptr;
        public SizeT len = new SizeT();
    }

    /** ネイティブのライブラリが出すはずの関数 */
    private static final String[] FUNCTIONS = {
        "zm_abi_version",
        "zm_session_new",
        "zm_push",
        "zm_call",
        "zm_take",
        "zm_cancel",
        "zm_session_close",
        "zm_free",
    };

    /** 長さ 0 の入力にも、NULL でない番地を渡すための 1 バイト */
    private static final byte[] EMPTY = new byte[1];

    private final Core core;

    private Zumen(Core core) {
        this.core = core;
    }

    /**
     * ネイティブのライブラリを、ファイルの道筋から読み込み、関数がそろうことと ABI の大版を確かめる。
     *
     * <p>32 bit の Windows では、関数を stdcall で呼ぶ（決定 0011）。読み込んだライブラリは外さない。
     *
     * @throws UnsatisfiedLinkError 読み込めないか、zumen のライブラリでないか、ABI の大版が違うとき
     */
    public static Zumen load(String path) {
        File file = new File(path).getAbsoluteFile();
        if (!file.isFile()) {
            throw new UnsatisfiedLinkError("zumen のネイティブのライブラリがない: " + file);
        }
        Map<String, Object> options = new HashMap<String, Object>();
        if (Platform.isWindows() && !Platform.is64Bit()) {
            options.put(Library.OPTION_CALLING_CONVENTION, StdCallLibrary.STDCALL_CONVENTION);
        }
        NativeLibrary library = NativeLibrary.getInstance(file.getPath(), options);
        List<String> missing = new ArrayList<String>();
        for (String name : FUNCTIONS) {
            try {
                library.getFunction(name);
            } catch (UnsatisfiedLinkError e) {
                missing.add(name);
            }
        }
        if (!missing.isEmpty()) {
            // String.join は Android 8.0 から
            StringBuilder names = new StringBuilder();
            for (String name : missing) {
                names.append(names.length() == 0 ? "" : ", ").append(name);
            }
            throw new UnsatisfiedLinkError("zumen のライブラリでない（ない関数: " + names + "）");
        }
        Core core = Native.load(file.getPath(), Core.class, options);
        int version = core.zm_abi_version();
        if (version != Constants.ABI_VERSION) {
            throw new UnsatisfiedLinkError(
                    "ABI の大版が " + version + "（この包みは " + Constants.ABI_VERSION + "）");
        }
        return new Zumen(core);
    }

    /** ABI の大版 */
    public int abiVersion() {
        return core.zm_abi_version();
    }

    /** 既定の設定でセッションを作る */
    public Session open() {
        return open(new byte[0]);
    }

    /** 設定（session_options）を値で渡して、セッションを作る */
    public Session open(Messages.SessionOptions options) {
        return open(Json.bytes(options));
    }

    /** 設定（session_options）を JSON の UTF-8 のバイト列で渡して、セッションを作る。長さが 0 なら既定の設定 */
    public Session open(byte[] options) {
        PointerByReference handle = new PointerByReference();
        int code = core.zm_session_new(nonEmpty(options), new SizeT(options.length), handle);
        if (code != Constants.Status.OK) {
            throw new ZumenException("zm_session_new", code, null);
        }
        return new Session(this, handle.getValue());
    }

    static byte[] nonEmpty(byte[] data) {
        return data.length == 0 ? EMPTY : data;
    }

    static byte[] utf8(String text) {
        return text.getBytes(StandardCharsets.UTF_8);
    }

    Core core() {
        return core;
    }

    /** zm_buf を用意して呼び、書かれたバイト列を写して返す。書かれなかったときは null */
    interface WithBuf {
        int call(Buf buf);
    }

    /** 呼び出しの状態と、書かれたバイト列 */
    static final class Written {
        final int code;
        final byte[] data;

        Written(int code, byte[] data) {
            this.code = code;
            this.data = data;
        }
    }

    Written withBuf(WithBuf call) {
        Buf buf = new Buf();
        int code = call.call(buf);
        try {
            buf.read();
            byte[] data;
            if (buf.ptr != null) {
                long length = buf.len.longValue();
                if (length > Integer.MAX_VALUE) {
                    throw new OutOfMemoryError("コアが返したバッファーが 2 GiB を超える");
                }
                data = buf.ptr.getByteArray(0, (int) length);
            } else {
                data = code == Constants.Status.OK ? new byte[0] : null;
            }
            return new Written(code, data);
        } finally {
            core.zm_free(buf);
        }
    }
}
