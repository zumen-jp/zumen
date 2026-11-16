// シーンの blob のリーダー（決定 0213・0196）。配置は schema/scene3d.toml にあり、セクションの定義は Constants.g.cs に生成する。
//
// ヘッダーは ZMS3、メジャーバージョン（u16）、マイナーバージョン（u16）、塊の数（u32）、セクションの数（u32）で、続けてセクションごとに始まりと長さ（u32、
// バイト）を置く。数はリトルエンディアン。知らないセクションは読み飛ばす。基本の 20 セクションがないときは
// 読まない。メジャーバージョンが違う場合も読まない。

#nullable enable

using System;
using System.Collections.Generic;
#if NET
using System.Runtime.InteropServices;
#endif

namespace Zumen
{
    /// <summary>読んだシーン。列は blob のバイト列を共有するので、blob を書き換えない</summary>
    public sealed class Scene3D
    {
        private readonly byte[] blob;
        private readonly Dictionary<string, (int Start, int Length)> columns;

        private Scene3D(byte[] blob, int major, int minor, uint chunks, uint sections, Dictionary<string, (int, int)> columns)
        {
            this.blob = blob;
            Major = major;
            Minor = minor;
            Chunks = chunks;
            Sections = sections;
            this.columns = columns;
        }

        /// <summary>メジャーバージョン</summary>
        public int Major { get; }

        /// <summary>マイナーバージョン</summary>
        public int Minor { get; }

        /// <summary>塊の数</summary>
        public uint Chunks { get; }

        /// <summary>blob にあるセクションの数。このラッパーが知らないセクションも数える</summary>
        public uint Sections { get; }

        /// <summary>シーンの blob を読む</summary>
        public static Scene3D Read(byte[] blob)
        {
            if (blob == null)
            {
                throw new ArgumentNullException(nameof(blob));
            }
            if (blob.Length > 536870912) throw new FormatException("3D シーンのバイト数の上限");
            const int header = 16;
            const int entry = 8;
            if (blob.Length < header || U32(blob, 0) != Scene3DLayout.Magic)
            {
                throw new FormatException("シーンの blob でない");
            }
            int major = U16(blob, 4);
            int minor = U16(blob, 6);
            var chunks = U32(blob, 8);
            var sections = U32(blob, 12);
            if (major != Scene3DLayout.Major)
            {
                throw new FormatException("シーンのメジャーバージョンが " + major + "（このラッパーは " + Scene3DLayout.Major + "）");
            }
            if (header + (ulong)entry * sections > (ulong)blob.Length)
            {
                throw new FormatException("セクションの表が blob に収まらない");
            }
            if (sections < Scene3DLayout.Sections.Count) throw new FormatException("3D シーンの基本列がない");
            ulong previous = header + (ulong)entry * sections;
            for (uint i = 0; i < sections; i++) {
                var start = U32(blob, header + entry * (int)i);
                var length = U32(blob, header + entry * (int)i + 4);
                if (start % 8 != 0 || start < previous || (ulong)start + length > (ulong)blob.Length)
                    throw new FormatException("3D シーンのセクションの範囲");
                previous = (ulong)start + length;
            }
            var columns = new Dictionary<string, (int, int)>();
            foreach (var section in Scene3DLayout.Sections)
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
                    throw new FormatException("セクション " + section.Name + " の始まりか長さが合わない");
                }
                columns[section.Name] = ((int)start, (int)length);
            }
            return new Scene3D(blob, major, minor, chunks, sections, columns);
        }

        private uint Reference(string name, int row, int component = 0) {
            var section = SectionOf(name);
            if (row < 0 || row >= Rows(name) || component < 0 || component >= section.Width)
                throw new IndexOutOfRangeException("3D シーンの行の範囲");
            return U32(blob, columns[name].Start + row * section.Width * 4 + component * 4);
        }
        private uint InstanceReference(string name, string target, int row) {
            uint value = Reference(name, row);
            if (value >= Rows(target)) throw new FormatException("3D シーンの参照");
            return value;
        }
        /// <summary>元の建物の要素 ID</summary>
        public string ElementId(int row) => Text(Reference("element_ids", row), Reference("element_ids", row, 1));
        /// <summary>インスタンスが指すメッシュの行</summary>
        public uint InstanceMesh(int row) => InstanceReference("instance_meshes", "mesh_vertices", row);
        /// <summary>インスタンスが指す要素の行</summary>
        public uint InstanceElement(int row) => InstanceReference("instance_elements", "element_ids", row);
        /// <summary>インスタンスが指す色の行</summary>
        public uint InstanceColor(int row) => InstanceReference("instance_colors", "color_rgba", row);

        /// <summary>セクションの定義を名前で引く</summary>
        public static SceneSection SectionOf(string name)
        {
            foreach (var section in Scene3DLayout.Sections)
            {
                if (section.Name == name)
                {
                    return section;
                }
            }
            throw new KeyNotFoundException("知らないセクション: " + name);
        }

        /// <summary>セクションの行の数</summary>
        public int Rows(string name)
        {
            var section = SectionOf(name);
            return columns[name].Length / (section.Size * section.Width);
        }

        /// <summary>セクションのバイト列（リトルエンディアン）。写さない</summary>
        public ArraySegment<byte> Bytes(string name)
        {
            SectionOf(name);
            var (start, length) = columns[name];
            return new ArraySegment<byte>(blob, start, length);
        }

        /// <summary>セクションの値を配列に写す。T はセクションの値の型（SceneSection.Type）にする</summary>
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
        /// セクションの値を、行ごとに Width 個の値を並べた列で見る。T はセクションの値の型（SceneSection.Type）にする。
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

        /// <summary>strings のセクションから、始まりと長さ（バイト）で文字列を引く</summary>
        public string Text(uint start, uint length)
        {
            var (offset, size) = columns["strings"];
            if ((ulong)start + length > (ulong)size)
            {
                throw new ArgumentOutOfRangeException(nameof(start), "strings のセクションの外を指す");
            }
            return ZumenException.Utf8.GetString(blob, offset + (int)start, (int)length);
        }

        private static SceneSection Check<T>(string name)
        {
            var section = SectionOf(name);
            if (section.Type != typeof(T))
            {
                throw new ArgumentException("セクション " + name + " の値の型は " + section.Type.Name + "（" + typeof(T).Name + " でない）");
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
