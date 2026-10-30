// シーンの blob の読み手（決定 0137・0196）。配置は schema/scene.toml にあり、区画の定義は Constants.g.cs に生成する。
//
// 見出しは ZMSC、大版（u16）、小版（u16）、塊の数（u32）、区画の数（u32）で、続けて区画ごとに始まりと長さ（u32、
// バイト）を置く。数はリトルエンディアン。知らない区画（新しい小版で足したもの）は読み飛ばし、ない区画（古い小版）は
// 空の列にする。大版が違えば読まない。

#nullable enable

using System;
using System.Collections.Generic;
#if NET
using System.Runtime.InteropServices;
#endif

namespace Zumen.Provisional
{
    /// <summary>シーンの区画の定義</summary>
    public sealed class SceneSection
    {
        internal SceneSection(int index, string name, Type type, int width)
        {
            Index = index;
            Name = name;
            Type = type;
            Width = width;
        }

        /// <summary>区画の表での番号</summary>
        public int Index { get; }

        /// <summary>名前</summary>
        public string Name { get; }

        /// <summary>値の型（byte・uint・float・double）</summary>
        public Type Type { get; }

        /// <summary>行ごとの値の数</summary>
        public int Width { get; }

        /// <summary>値 1 つのバイト数</summary>
        public int Size =>
            Type == typeof(byte) ? 1 : Type == typeof(double) ? 8 : 4;
    }

    /// <summary>読んだシーン。列は blob のバイト列を共有するので、blob を書き換えない</summary>
    public sealed class Scene
    {
        private readonly byte[] blob;
        private readonly Dictionary<string, (int Start, int Length)> columns;

        private Scene(byte[] blob, int major, int minor, uint chunks, uint sections, Dictionary<string, (int, int)> columns)
        {
            this.blob = blob;
            Major = major;
            Minor = minor;
            Chunks = chunks;
            Sections = sections;
            this.columns = columns;
        }

        /// <summary>大版</summary>
        public int Major { get; }

        /// <summary>小版</summary>
        public int Minor { get; }

        /// <summary>塊の数</summary>
        public uint Chunks { get; }

        /// <summary>blob にある区画の数。この包みが知らない区画も数える</summary>
        public uint Sections { get; }

        /// <summary>シーンの blob を読む</summary>
        public static Scene Read(byte[] blob)
        {
            if (blob == null)
            {
                throw new ArgumentNullException(nameof(blob));
            }
            const int header = 16;
            const int entry = 8;
            if (blob.Length < header || U32(blob, 0) != SceneLayout.Magic)
            {
                throw new FormatException("シーンの blob でない");
            }
            int major = U16(blob, 4);
            int minor = U16(blob, 6);
            var chunks = U32(blob, 8);
            var sections = U32(blob, 12);
            if (major != SceneLayout.Major)
            {
                throw new FormatException("シーンの大版が " + major + "（この包みは " + SceneLayout.Major + "）");
            }
            if (header + (ulong)entry * sections > (ulong)blob.Length)
            {
                throw new FormatException("区画の表が blob に収まらない");
            }
            var columns = new Dictionary<string, (int, int)>();
            foreach (var section in SceneLayout.Sections)
            {
                if (section.Index >= sections)
                {
                    columns[section.Name] = (0, 0);
                    continue;
                }
                var start = U32(blob, header + entry * section.Index);
                var length = U32(blob, header + entry * section.Index + 4);
                var row = (uint)(section.Size * section.Width);
                if ((ulong)start + length > (ulong)blob.Length || length % row != 0)
                {
                    throw new FormatException("区画 " + section.Name + " の始まりか長さが合わない");
                }
                columns[section.Name] = ((int)start, (int)length);
            }
            return new Scene(blob, major, minor, chunks, sections, columns);
        }

        /// <summary>区画の定義を名前で引く</summary>
        public static SceneSection SectionOf(string name)
        {
            foreach (var section in SceneLayout.Sections)
            {
                if (section.Name == name)
                {
                    return section;
                }
            }
            throw new KeyNotFoundException("知らない区画: " + name);
        }

        /// <summary>区画の行の数</summary>
        public int Rows(string name)
        {
            var section = SectionOf(name);
            return columns[name].Length / (section.Size * section.Width);
        }

        /// <summary>区画のバイト列（リトルエンディアン）。写さない</summary>
        public ArraySegment<byte> Bytes(string name)
        {
            SectionOf(name);
            var (start, length) = columns[name];
            return new ArraySegment<byte>(blob, start, length);
        }

        /// <summary>区画の値を配列に写す。T は区画の値の型（SceneSection.Type）にする</summary>
        public T[] ToArray<T>(string name)
            where T : struct
        {
            var section = Check<T>(name);
            var (start, length) = columns[name];
            var values = new T[length / section.Size];
            Buffer.BlockCopy(blob, start, values, 0, length);
            if (!BitConverter.IsLittleEndian && section.Size > 1)
            {
                Swap(values, section.Size);
            }
            return values;
        }

#if NET
        /// <summary>
        /// 区画の値を、行ごとに Width 個の値を並べた列で見る。T は区画の値の型（SceneSection.Type）にする。
        /// リトルエンディアンの環境では blob を写さない
        /// </summary>
        public ReadOnlySpan<T> Column<T>(string name)
            where T : unmanaged
        {
            Check<T>(name);
            if (!BitConverter.IsLittleEndian)
            {
                return ToArray<T>(name);
            }
            var (start, length) = columns[name];
            return MemoryMarshal.Cast<byte, T>(new ReadOnlySpan<byte>(blob, start, length));
        }
#endif

        /// <summary>strings の区画から、始まりと長さ（バイト）で文字列を引く</summary>
        public string Text(uint start, uint length)
        {
            var (offset, size) = columns["strings"];
            if ((ulong)start + length > (ulong)size)
            {
                throw new ArgumentOutOfRangeException(nameof(start), "strings の区画の外を指す");
            }
            return ZumenException.Utf8.GetString(blob, offset + (int)start, (int)length);
        }

        private static SceneSection Check<T>(string name)
        {
            var section = SectionOf(name);
            if (section.Type != typeof(T))
            {
                throw new ArgumentException("区画 " + name + " の値の型は " + section.Type.Name + "（" + typeof(T).Name + " でない）");
            }
            return section;
        }

        private static void Swap<T>(T[] values, int size)
            where T : struct
        {
            var bytes = new byte[values.Length * size];
            Buffer.BlockCopy(values, 0, bytes, 0, bytes.Length);
            for (var i = 0; i < bytes.Length; i += size)
            {
                Array.Reverse(bytes, i, size);
            }
            Buffer.BlockCopy(bytes, 0, values, 0, bytes.Length);
        }

        private static ushort U16(byte[] data, int at) => (ushort)(data[at] | data[at + 1] << 8);

        private static uint U32(byte[] data, int at) =>
            (uint)(data[at] | data[at + 1] << 8 | data[at + 2] << 16 | data[at + 3] << 24);
    }
}
