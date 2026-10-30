// 試験の組を読むための、小さな JSON の読み書き。
//
// .NET Framework 4.8 には System.Text.Json がなく、NuGet の依存を足さないので、試験の中に置く。値は、null・bool・
// string・JsonNumber（書かれたままの数）・List（配列）・JsonObject（欄の順を保つ）で持つ。

#nullable enable

using System;
using System.Collections.Generic;
using System.Globalization;
using System.Text;

namespace Zumen.Conformance
{
    /// <summary>書かれたままの数</summary>
    internal sealed class JsonNumber
    {
        public JsonNumber(string text) => Text = text;

        public string Text { get; }

        /// <summary>数として等しいか。整数は桁を落とさずに比べる</summary>
        public bool SameAs(JsonNumber other)
        {
            if (decimal.TryParse(Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var a)
                && decimal.TryParse(other.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var b))
            {
                return a == b;
            }
            return double.Parse(Text, CultureInfo.InvariantCulture) == double.Parse(other.Text, CultureInfo.InvariantCulture);
        }

        public long ToInt64() => long.Parse(Text, CultureInfo.InvariantCulture);
    }

    /// <summary>欄の順を保つオブジェクト</summary>
    internal sealed class JsonObject : List<KeyValuePair<string, object?>>
    {
        public bool TryGet(string key, out object? value)
        {
            foreach (var pair in this)
            {
                if (pair.Key == key)
                {
                    value = pair.Value;
                    return true;
                }
            }
            value = null;
            return false;
        }

        public bool Has(string key) => TryGet(key, out _);

        public object? this[string key] => TryGet(key, out var value) ? value : throw new KeyNotFoundException(key);
    }

    internal static class Json
    {
        public static object? Parse(string text)
        {
            var reader = new Reader(text);
            reader.Space();
            var value = reader.Value(0);
            reader.Space();
            if (reader.At < text.Length)
            {
                throw new FormatException("JSON の後ろに余りがある");
            }
            return value;
        }

        /// <summary>空白を入れずに書く。文字列は、" と \ と制御文字だけを崩す</summary>
        public static string Write(object? value)
        {
            var out_ = new StringBuilder();
            Write(out_, value);
            return out_.ToString();
        }

        private static void Write(StringBuilder out_, object? value)
        {
            switch (value)
            {
                case null:
                    out_.Append("null");
                    break;
                case bool flag:
                    out_.Append(flag ? "true" : "false");
                    break;
                case JsonNumber number:
                    out_.Append(number.Text);
                    break;
                case string text:
                    Quote(out_, text);
                    break;
                case JsonObject members:
                    out_.Append('{');
                    for (var i = 0; i < members.Count; i++)
                    {
                        if (i > 0)
                        {
                            out_.Append(',');
                        }
                        Quote(out_, members[i].Key);
                        out_.Append(':');
                        Write(out_, members[i].Value);
                    }
                    out_.Append('}');
                    break;
                case List<object?> items:
                    out_.Append('[');
                    for (var i = 0; i < items.Count; i++)
                    {
                        if (i > 0)
                        {
                            out_.Append(',');
                        }
                        Write(out_, items[i]);
                    }
                    out_.Append(']');
                    break;
                default:
                    throw new ArgumentException("JSON の値でない: " + value.GetType());
            }
        }

        private static void Quote(StringBuilder out_, string text)
        {
            out_.Append('"');
            foreach (var c in text)
            {
                switch (c)
                {
                    case '"':
                        out_.Append("\\\"");
                        break;
                    case '\\':
                        out_.Append("\\\\");
                        break;
                    case '\n':
                        out_.Append("\\n");
                        break;
                    case '\r':
                        out_.Append("\\r");
                        break;
                    case '\t':
                        out_.Append("\\t");
                        break;
                    default:
                        if (c < 0x20)
                        {
                            out_.Append("\\u").Append(((int)c).ToString("x4", CultureInfo.InvariantCulture));
                        }
                        else
                        {
                            out_.Append(c);
                        }
                        break;
                }
            }
            out_.Append('"');
        }

        private sealed class Reader
        {
            private readonly string text;

            public Reader(string text) => this.text = text;

            public int At { get; private set; }

            public void Space()
            {
                while (At < text.Length && (text[At] == ' ' || text[At] == '\t' || text[At] == '\n' || text[At] == '\r'))
                {
                    At++;
                }
            }

            public object? Value(int depth)
            {
                if (depth > 128)
                {
                    throw new FormatException("JSON が深すぎる");
                }
                if (At >= text.Length)
                {
                    throw new FormatException("JSON が途中で終わる");
                }
                switch (text[At])
                {
                    case '{':
                        return Object(depth);
                    case '[':
                        return Array(depth);
                    case '"':
                        return String();
                    case 't':
                        return Word("true", true);
                    case 'f':
                        return Word("false", false);
                    case 'n':
                        return Word("null", null);
                    default:
                        return Number();
                }
            }

            private object? Word(string word, object? value)
            {
                if (string.CompareOrdinal(text, At, word, 0, word.Length) != 0)
                {
                    throw new FormatException("JSON として読めない（位置 " + At + "）");
                }
                At += word.Length;
                return value;
            }

            private JsonNumber Number()
            {
                var start = At;
                while (At < text.Length && "+-0123456789.eE".IndexOf(text[At]) >= 0)
                {
                    At++;
                }
                if (start == At)
                {
                    throw new FormatException("JSON として読めない（位置 " + At + "）");
                }
                return new JsonNumber(text.Substring(start, At - start));
            }

            private string String()
            {
                At++;
                var out_ = new StringBuilder();
                while (true)
                {
                    if (At >= text.Length)
                    {
                        throw new FormatException("文字列が閉じない");
                    }
                    var c = text[At++];
                    if (c == '"')
                    {
                        return out_.ToString();
                    }
                    if (c != '\\')
                    {
                        out_.Append(c);
                        continue;
                    }
                    var escape = text[At++];
                    switch (escape)
                    {
                        case 'n':
                            out_.Append('\n');
                            break;
                        case 'r':
                            out_.Append('\r');
                            break;
                        case 't':
                            out_.Append('\t');
                            break;
                        case 'b':
                            out_.Append('\b');
                            break;
                        case 'f':
                            out_.Append('\f');
                            break;
                        case 'u':
                            out_.Append((char)int.Parse(text.Substring(At, 4), NumberStyles.HexNumber, CultureInfo.InvariantCulture));
                            At += 4;
                            break;
                        default:
                            out_.Append(escape);
                            break;
                    }
                }
            }

            private List<object?> Array(int depth)
            {
                At++;
                var items = new List<object?>();
                Space();
                if (text[At] == ']')
                {
                    At++;
                    return items;
                }
                while (true)
                {
                    Space();
                    items.Add(Value(depth + 1));
                    Space();
                    var c = text[At++];
                    if (c == ']')
                    {
                        return items;
                    }
                    if (c != ',')
                    {
                        throw new FormatException("配列の区切りがない（位置 " + At + "）");
                    }
                }
            }

            private JsonObject Object(int depth)
            {
                At++;
                var members = new JsonObject();
                Space();
                if (text[At] == '}')
                {
                    At++;
                    return members;
                }
                while (true)
                {
                    Space();
                    var key = String();
                    Space();
                    if (text[At++] != ':')
                    {
                        throw new FormatException("オブジェクトの : がない（位置 " + At + "）");
                    }
                    Space();
                    members.Add(new KeyValuePair<string, object?>(key, Value(depth + 1)));
                    Space();
                    var c = text[At++];
                    if (c == '}')
                    {
                        return members;
                    }
                    if (c != ',')
                    {
                        throw new FormatException("オブジェクトの区切りがない（位置 " + At + "）");
                    }
                }
            }
        }
    }
}
