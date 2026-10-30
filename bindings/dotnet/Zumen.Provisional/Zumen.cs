// zumen の C ABI を呼ぶ薄い包み（決定 0011・0196）。
//
// ネイティブのライブラリ（zumen_abi1.dll・libzumen_abi1.so・libzumen_abi1.dylib）を利用者が渡した道筋から読み込み、
// 境界の関数（schema/boundary.toml）を、セッションのメソッドにして出す。関数は、生成した委譲の型（Constants.g.cs）で
// 呼ぶ。委譲は Winapi の呼び方なので、32 bit の Windows では stdcall、ほかでは cdecl になる。

#nullable enable

using System;
using System.Runtime.InteropServices;
using System.Threading;
#if NET
using System.Text.Json;
using System.Text.Json.Serialization;
using Zumen.Provisional.Messages;
#endif

namespace Zumen.Provisional
{
    /// <summary>境界の関数が ok でない状態を返した</summary>
    public sealed class ZumenException : Exception
    {
        internal ZumenException(string operation, int code, byte[]? data)
            : base(operation + " が " + StatusNames.Of(code) + " を返した")
        {
            Operation = operation;
            Code = code;
            ResponseBytes = data;
        }

        /// <summary>呼んだ境界の関数</summary>
        public string Operation { get; }

        /// <summary>状態コード</summary>
        public int Code { get; }

        /// <summary>状態コード（知らない値もそのまま持つ）</summary>
        public Status Status => (Status)Code;

        /// <summary>状態コードの名前</summary>
        public string StatusName => StatusNames.Of(Code);

        /// <summary>応答の JSON のバイト列。書かれなかったときは null</summary>
        public byte[]? ResponseBytes { get; }

        /// <summary>応答の JSON。書かれなかったときは null</summary>
        public string? ResponseText => ResponseBytes == null ? null : Utf8.GetString(ResponseBytes);

#if NET
        /// <summary>応答。書かれなかったときは null。failed の応答は診断を持つ</summary>
        public Response<JsonElement>? Response =>
            ResponseBytes == null
                ? null
                : JsonSerializer.Deserialize<Response<JsonElement>>(ResponseBytes, ZumenLibrary.JsonOptions);
#endif

        internal static readonly System.Text.Encoding Utf8 = new System.Text.UTF8Encoding(false, true);
    }

    /// <summary>読み込んだネイティブのライブラリ</summary>
    public sealed unsafe class ZumenLibrary
    {
        /// <summary>長さ 0 のバイト列にも、NULL でない道筋を渡すための置き場</summary>
        private static readonly byte[] Empty = new byte[1];

        internal readonly NativeFunctions.ZmAbiVersion AbiVersionFn;
        internal readonly NativeFunctions.ZmSessionNew SessionNew;
        internal readonly NativeFunctions.ZmPush Push;
        internal readonly NativeFunctions.ZmCall Call;
        internal readonly NativeFunctions.ZmTake Take;
        internal readonly NativeFunctions.ZmCancel Cancel;
        internal readonly NativeFunctions.ZmSessionClose SessionClose;
        internal readonly NativeFunctions.ZmFree Free;

#if NET
        /// <summary>型つきの要求と応答を読み書きする設定。値のない欄は書かない</summary>
        public static readonly JsonSerializerOptions JsonOptions = new JsonSerializerOptions
        {
            DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        };
#endif

        private ZumenLibrary(IntPtr handle)
        {
            var missing = new System.Collections.Generic.List<string>();
            foreach (var name in NativeFunctions.Names)
            {
                if (Loader.Symbol(handle, name) == IntPtr.Zero)
                {
                    missing.Add(name);
                }
            }
            if (missing.Count > 0)
            {
                throw new DllNotFoundException("zumen のライブラリでない（ない関数: " + string.Join(", ", missing) + "）");
            }
            AbiVersionFn = Get<NativeFunctions.ZmAbiVersion>(handle, "zm_abi_version");
            SessionNew = Get<NativeFunctions.ZmSessionNew>(handle, "zm_session_new");
            Push = Get<NativeFunctions.ZmPush>(handle, "zm_push");
            Call = Get<NativeFunctions.ZmCall>(handle, "zm_call");
            Take = Get<NativeFunctions.ZmTake>(handle, "zm_take");
            Cancel = Get<NativeFunctions.ZmCancel>(handle, "zm_cancel");
            SessionClose = Get<NativeFunctions.ZmSessionClose>(handle, "zm_session_close");
            Free = Get<NativeFunctions.ZmFree>(handle, "zm_free");
            var version = AbiVersionFn();
            if (version != Versions.Abi)
            {
                throw new DllNotFoundException("ABI の大版が " + version + "（この包みは " + Versions.Abi + "）");
            }
        }

        private static T Get<T>(IntPtr handle, string name)
            where T : Delegate =>
            Marshal.GetDelegateForFunctionPointer<T>(Loader.Symbol(handle, name));

        /// <summary>
        /// ネイティブのライブラリを、ファイルの道筋から読み込み、関数がそろうことと ABI の大版を確かめる。
        /// 読み込んだライブラリは、プロセスが終わるまで外さない。
        /// </summary>
        public static ZumenLibrary Load(string path) =>
            new ZumenLibrary(Loader.Open(System.IO.Path.GetFullPath(path)));

        /// <summary>ABI の大版</summary>
        public uint AbiVersion => AbiVersionFn();

        /// <summary>セッションを作る。設定（session_options）は JSON のバイト列で渡す。null か空なら既定の設定</summary>
        public Session Open(byte[]? options = null)
        {
            options ??= Array.Empty<byte>();
            IntPtr handle;
            int code;
            fixed (byte* data = Pin(options))
            {
                code = SessionNew(data, (nuint)options.Length, &handle);
            }
            if (code != 0)
            {
                throw new ZumenException("zm_session_new", code, null);
            }
            return new Session(this, handle);
        }

        /// <summary>セッションを作る。設定（session_options）は JSON の文字列で渡す</summary>
        public Session Open(string options) => Open(ZumenException.Utf8.GetBytes(options));

#if NET
        /// <summary>セッションを作る。設定は SessionOptions などの値で渡す</summary>
        public Session Open<TOptions>(TOptions options) =>
            Open(JsonSerializer.SerializeToUtf8Bytes(options, ZumenLibrary.JsonOptions));
#endif

        /// <summary>fixed で NULL にならないように、空の列を置き換える</summary>
        internal static byte[] Pin(byte[] data) => data.Length == 0 ? Empty : data;

        /// <summary>zm_buf を用意して呼び、書かれたバイト列を写して zm_free で返す</summary>
        internal int WithBuf(Func<IntPtr, int> call, out byte[]? data)
        {
            var buf = default(Buf);
            var code = call((IntPtr)(&buf));
            try
            {
                if (buf.Ptr != null)
                {
                    if (buf.Len > int.MaxValue)
                    {
                        throw new OutOfMemoryException("コアが返したバッファーが、.NET の配列に収まらない");
                    }
                    data = new byte[(int)buf.Len];
                    Marshal.Copy((IntPtr)buf.Ptr, data, 0, data.Length);
                }
                else
                {
                    data = code == 0 ? Array.Empty<byte>() : null;
                }
            }
            finally
            {
                Free(&buf);
            }
            return code;
        }
    }

    /// <summary>
    /// セッション。入力と、直前の呼び出しのバイナリの結果を持つ。使い終わったら Dispose で閉じる。
    /// 1 つのセッションは、一度に 1 つのスレッドだけが使う。Cancel だけは、どのスレッドからも呼べる。
    /// </summary>
    public sealed unsafe class Session : IDisposable
    {
        private readonly ZumenLibrary library;
        private IntPtr handle;

        internal Session(ZumenLibrary library, IntPtr handle)
        {
            this.library = library;
            this.handle = handle;
        }

        /// <summary>閉じたか</summary>
        public bool IsClosed => handle == IntPtr.Zero;

        /// <summary>名前のついた入力に、バイト列を足す。同じ名前で何回か呼ぶと、後ろに足していく</summary>
        public void Push(byte[] name, byte[] data)
        {
            int code;
            fixed (byte* n = ZumenLibrary.Pin(name))
            fixed (byte* d = ZumenLibrary.Pin(data))
            {
                code = library.Push(handle, n, (nuint)name.Length, d, (nuint)data.Length);
            }
            if (code != 0)
            {
                throw new ZumenException("zm_push", code, null);
            }
        }

        /// <summary>名前のついた入力に、バイト列を足す。名前は UTF-8 で渡す</summary>
        public void Push(string name, byte[] data) => Push(ZumenException.Utf8.GetBytes(name), data);

        /// <summary>要求の JSON のバイト列を渡し、応答の JSON のバイト列を受け取る</summary>
        public byte[] CallBytes(byte[] request)
        {
            var session = handle;
            var code = library.WithBuf(
                buf =>
                {
                    fixed (byte* r = ZumenLibrary.Pin(request))
                    {
                        return library.Call(session, r, (nuint)request.Length, (Buf*)buf);
                    }
                },
                out var data);
            if (code != 0)
            {
                throw new ZumenException("zm_call", code, data);
            }
            return data!;
        }

        /// <summary>要求の JSON の文字列を渡し、応答の JSON の文字列を受け取る</summary>
        public string Call(string request) =>
            ZumenException.Utf8.GetString(CallBytes(ZumenException.Utf8.GetBytes(request)));

#if NET
        /// <summary>
        /// 要求（OpenRequest などの値）を渡し、応答を受け取る。結果の型は Operations.Types が op から引く型。
        /// 失敗した応答は、ZumenException の Response にある
        /// </summary>
        public Response<TResult> Call<TResult>(object request)
        {
            var bytes = JsonSerializer.SerializeToUtf8Bytes(request, request.GetType(), ZumenLibrary.JsonOptions);
            return JsonSerializer.Deserialize<Response<TResult>>(CallBytes(bytes), ZumenLibrary.JsonOptions)!;
        }
#endif

        /// <summary>直前の Call が作ったバイナリの結果を、応答の blobs にある番号で取り出す</summary>
        public byte[] Take(uint blob)
        {
            var session = handle;
            var code = library.WithBuf(buf => library.Take(session, blob, (Buf*)buf), out var data);
            if (code != 0)
            {
                throw new ZumenException("zm_take", code, null);
            }
            return data!;
        }

        /// <summary>動いている呼び出しを止める。動いている呼び出しがなければ、何もしない</summary>
        public void Cancel() => library.Cancel(handle);

        /// <summary>セッションを閉じる。閉じたあとの呼び出しは、invalid の ZumenException になる</summary>
        public void Dispose()
        {
            var old = Interlocked.Exchange(ref handle, IntPtr.Zero);
            if (old != IntPtr.Zero)
            {
                library.SessionClose(old);
            }
            GC.SuppressFinalize(this);
        }

        /// <summary>閉じ忘れたセッションを閉じる</summary>
        ~Session()
        {
            var old = Interlocked.Exchange(ref handle, IntPtr.Zero);
            if (old != IntPtr.Zero)
            {
                library.SessionClose(old);
            }
        }
    }

    /// <summary>ネイティブのライブラリを読み込み、関数の番地を引く</summary>
    internal static class Loader
    {
        internal static IntPtr Open(string path)
        {
#if NET
            return NativeLibrary.Load(path);
#else
            IntPtr handle;
            if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            {
                handle = Windows.LoadLibraryW(path);
            }
            else if (RuntimeInformation.IsOSPlatform(OSPlatform.OSX))
            {
                handle = Mac.dlopen(path, Unix.Now);
            }
            else
            {
                handle = Linux.dlopen(path, Unix.Now);
            }
            if (handle == IntPtr.Zero)
            {
                throw new DllNotFoundException("ネイティブのライブラリを読み込めない: " + path);
            }
            return handle;
#endif
        }

        internal static IntPtr Symbol(IntPtr handle, string name)
        {
#if NET
            return NativeLibrary.TryGetExport(handle, name, out var address) ? address : IntPtr.Zero;
#else
            if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
            {
                return Windows.GetProcAddress(handle, name);
            }
            return RuntimeInformation.IsOSPlatform(OSPlatform.OSX) ? Mac.dlsym(handle, name) : Linux.dlsym(handle, name);
#endif
        }

#if !NET
        // netstandard2.0 には NativeLibrary がないので、OS の関数で読み込む

        private static class Windows
        {
            [DllImport("kernel32", CharSet = CharSet.Unicode, SetLastError = true)]
            internal static extern IntPtr LoadLibraryW(string path);

            [DllImport("kernel32", CharSet = CharSet.Ansi, BestFitMapping = false, ExactSpelling = true)]
            internal static extern IntPtr GetProcAddress(IntPtr module, string name);
        }

        private static class Unix
        {
            internal const int Now = 2;
        }

        private static class Linux
        {
            [DllImport("libdl.so.2", CharSet = CharSet.Ansi, BestFitMapping = false)]
            internal static extern IntPtr dlopen(string path, int flags);

            [DllImport("libdl.so.2", CharSet = CharSet.Ansi, BestFitMapping = false)]
            internal static extern IntPtr dlsym(IntPtr handle, string name);
        }

        private static class Mac
        {
            [DllImport("/usr/lib/libSystem.dylib", CharSet = CharSet.Ansi, BestFitMapping = false)]
            internal static extern IntPtr dlopen(string path, int flags);

            [DllImport("/usr/lib/libSystem.dylib", CharSet = CharSet.Ansi, BestFitMapping = false)]
            internal static extern IntPtr dlsym(IntPtr handle, string name);
        }
#endif
    }
}
