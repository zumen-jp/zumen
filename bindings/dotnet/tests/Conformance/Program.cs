// 適合試験を、.NET の包み（bindings/dotnet）を通して流す（決定 0006・0196）。
//
// 使い方: dotnet run --project bindings/dotnet/tests/Conformance -- <ネイティブのライブラリ>
//
// bindings/conformance/cases/*.json を、包みのセッションのメソッドで流し、状態と応答の値が期待どおりで、ハッシュの
// 一覧が hashes.txt と一致することを確かめる。シーンの blob は、包みの読み手で読み、応答の数と合うかを確かめる。
// 手順の読み方と応答の確かめ方は、Rust の側（bindings/conformance/src/lib.rs）と、Python の側
// （bindings/python/tests/conformance.py）と同じ規則で書く。

#nullable enable

using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using Zumen.Provisional;
#if NET
using System.Text.Json;
using Zumen.Provisional.Messages;
#endif

namespace Zumen.Conformance
{
    internal static class Program
    {
        /// <summary>手順の種類。手順は、どれか 1 つの欄を持つ</summary>
        private static readonly string[] Actions = { "session", "push", "call", "call_text", "call_hex", "take" };

        /// <summary>JSON Pointer で引けなかった</summary>
        private static readonly object Missing = new object();

        private static readonly Encoding Utf8 = new UTF8Encoding(false);

        private static int Main(string[] args)
        {
            // Windows の端末の既定（cp1252・cp932 など）に任せず、UTF-8 で書く
            Console.SetOut(new StreamWriter(Console.OpenStandardOutput(), Utf8) { AutoFlush = true });
            Console.SetError(new StreamWriter(Console.OpenStandardError(), Utf8) { AutoFlush = true });
            if (args.Length != 1)
            {
                Console.Error.WriteLine("使い方: dotnet run --project bindings/dotnet/tests/Conformance -- <ネイティブのライブラリ>");
                return 2;
            }
            return Run(args[0]);
        }

        private static string FindRepository()
        {
            foreach (var start in new[] { Directory.GetCurrentDirectory(), AppContext.BaseDirectory })
            {
                for (var dir = new DirectoryInfo(start); dir != null; dir = dir.Parent)
                {
                    if (File.Exists(Path.Combine(dir.FullName, "bindings", "conformance", "hashes.txt")))
                    {
                        return dir.FullName;
                    }
                }
            }
            throw new DirectoryNotFoundException("リポジトリの根（bindings/conformance/hashes.txt）が見つからない");
        }

        private static byte[] DecodeHex(string text)
        {
            var digits = Regex.Replace(text, @"\s+", "");
            if (!Regex.IsMatch(digits, "^([0-9a-fA-F]{2})*$"))
            {
                throw new FormatException("16 進として読めない: " + text);
            }
            var bytes = new byte[digits.Length / 2];
            for (var i = 0; i < bytes.Length; i++)
            {
                bytes[i] = Convert.ToByte(digits.Substring(i * 2, 2), 16);
            }
            return bytes;
        }

        /// <summary>JSON Pointer で値を引く。なければ Missing</summary>
        private static object? Pointer(object? value, string path)
        {
            if (path.Length == 0)
            {
                return value;
            }
            var current = value;
            foreach (var raw in path.Substring(1).Split('/'))
            {
                var token = raw.Replace("~1", "/").Replace("~0", "~");
                if (current is List<object?> items)
                {
                    if (!Regex.IsMatch(token, "^(0|[1-9][0-9]*)$") || !int.TryParse(token, out var index) || index >= items.Count)
                    {
                        return Missing;
                    }
                    current = items[index];
                }
                else if (current is JsonObject members && members.TryGet(token, out var next))
                {
                    current = next;
                }
                else
                {
                    return Missing;
                }
            }
            return current;
        }

        /// <summary>JSON の値として等しいか。真偽値と数は別にする</summary>
        private static bool Same(object? a, object? b)
        {
            switch (a)
            {
                case null:
                    return b == null;
                case bool flag:
                    return b is bool otherFlag && flag == otherFlag;
                case string text:
                    return b is string otherText && text == otherText;
                case JsonNumber number:
                    return b is JsonNumber otherNumber && number.SameAs(otherNumber);
                case JsonObject members:
                    if (!(b is JsonObject otherMembers) || members.Count != otherMembers.Count)
                    {
                        return false;
                    }
                    return members.All(pair => otherMembers.TryGet(pair.Key, out var other) && Same(pair.Value, other));
                case List<object?> items:
                    return b is List<object?> otherItems && items.Count == otherItems.Count && items.Zip(otherItems, Same).All(same => same);
                default:
                    return false;
            }
        }

        /// <summary>session の設定か、call の要求のバイト列。値は JSON に直し、文字列はそのまま渡す</summary>
        private static byte[] BytesOf(object? input, string valueKey, string textKey)
        {
            if (input is JsonObject members)
            {
                if (members.TryGet(valueKey, out var value))
                {
                    return Utf8.GetBytes(Json.Write(value));
                }
                if (members.TryGet(textKey, out var text))
                {
                    return Utf8.GetBytes((string)text!);
                }
            }
            return Array.Empty<byte>();
        }

        /// <summary>公開サンプル（決定 0171）を読む。道筋はリポジトリの根から書き、samples/ の中に限る</summary>
        private static byte[] ReadSample(string repository, string file)
        {
            var parts = file.Split('/');
            if (parts[0] != "samples" || parts.Length < 2 || parts.Skip(1).Any(p => p.Length == 0 || p == "." || p == ".." || p.Contains("\\")))
            {
                throw new ArgumentException("push の file は samples/ の中のファイルにする: " + file);
            }
            return File.ReadAllBytes(Path.Combine(new[] { repository }.Concat(parts).ToArray()));
        }

        /// <summary>push の手順の、入力の名前と中身</summary>
        private static (byte[] Name, byte[] Data) PushOf(string repository, JsonObject input)
        {
            var name = input.TryGet("name_hex", out var hex) ? DecodeHex((string)hex!) : Utf8.GetBytes((string)input["name"]!);
            byte[] data;
            if (input.TryGet("hex", out var dataHex))
            {
                data = DecodeHex((string)dataHex!);
            }
            else if (input.TryGet("file", out var file))
            {
                data = ReadSample(repository, (string)file!);
            }
            else
            {
                data = Utf8.GetBytes(input.TryGet("text", out var text) ? (string)text! : "");
            }
            return (name, data);
        }

        /// <summary>call・call_text・call_hex の手順の、要求のバイト列</summary>
        private static byte[] RequestOf(string action, object? input)
        {
            switch (action)
            {
                case "call_text":
                    return Utf8.GetBytes((string)input!);
                case "call_hex":
                    return DecodeHex((string)input!);
                default:
                    return Utf8.GetBytes(Json.Write(input));
            }
        }

        private static void CheckResponse(string at, JsonObject step, object? response, List<string> failures)
        {
            if (step.TryGet("expect", out var expect) && expect is JsonObject expected)
            {
                foreach (var pair in expected)
                {
                    var found = Pointer(response, pair.Key);
                    if (found == Missing)
                    {
                        failures.Add(at + ": " + pair.Key + " がない");
                    }
                    else if (!Same(found, pair.Value))
                    {
                        failures.Add(at + ": " + pair.Key + " が " + Json.Write(found) + "（期待は " + Json.Write(pair.Value) + "）");
                    }
                }
            }
            if (step.TryGet("absent", out var absent) && absent is List<object?> paths)
            {
                foreach (string path in paths.Cast<string>())
                {
                    if (Pointer(response, path) != Missing)
                    {
                        failures.Add(at + ": " + path + " があってはいけない");
                    }
                }
            }
        }

        /// <summary>境界の関数を呼び、状態の名前と、書かれたバイト列を返す</summary>
        private static (string Status, T? Value) Attempt<T>(Func<T> call)
            where T : class
        {
            try
            {
                return ("ok", call());
            }
            catch (ZumenException error)
            {
                return (error.StatusName, error.ResponseBytes as T);
            }
        }

        private static string Sha256(byte[] data)
        {
            using (var sha = SHA256.Create())
            {
                return string.Concat(sha.ComputeHash(data).Select(b => b.ToString("x2")));
            }
        }

        private static long Count(object? counts, string name) => ((JsonNumber)((JsonObject)counts!)[name]!).ToInt64();

        /// <summary>シーンの blob を読み手で読み、応答の数と合うかを確かめる</summary>
        private static void CheckScene(string at, byte[] blob, JsonObject result, List<string> failures)
        {
            Scene scene;
            try
            {
                scene = Scene.Read(blob);
            }
            catch (FormatException error)
            {
                failures.Add(at + ": シーンを読めない: " + error.Message);
                return;
            }
            var counts = result["counts"];
            var checks = new (string Label, long Actual, long Expected)[]
            {
                ("大版", scene.Major, SceneLayout.Major),
                ("区画の数", scene.Sections, SceneLayout.Sections.Count),
                ("塊の数", scene.Chunks, Count(counts, "chunks")),
                ("chunk_origins の行", scene.Rows("chunk_origins"), Count(counts, "chunks")),
                ("text_xy の行", scene.Rows("text_xy"), Count(counts, "texts")),
                ("point_xy の行", scene.Rows("point_xy"), Count(counts, "points")),
                ("glyph_xy の行", scene.Rows("glyph_xy"), Count(counts, "glyphs")),
                ("line_xy と face_xy の行", scene.Rows("line_xy") + scene.Rows("face_xy"), Count(counts, "vertices")),
            };
            // 文字の文字列は、strings の区画から引ける
            var strings = scene.ToArray<uint>("text_strings");
#if NET
            if (!scene.Column<uint>("text_strings").SequenceEqual(strings))
            {
                failures.Add(at + ": text_strings の Column と ToArray が合わない");
            }
#endif
            for (var row = 0; row + 1 < strings.Length; row += 2)
            {
                try
                {
                    scene.Text(strings[row], strings[row + 1]);
                }
                catch (Exception error) when (error is ArgumentException || error is DecoderFallbackException)
                {
                    failures.Add(at + ": text_strings の行 " + row / 2 + " を引けない: " + error.Message);
                }
            }
            foreach (var (label, actual, expected) in checks)
            {
                if (actual != expected)
                {
                    failures.Add(at + ": シーンの" + label + "が " + actual + "（期待は " + expected + "）");
                }
            }
        }

        private static int Run(string libraryPath)
        {
            var repository = FindRepository();
            var conformance = Path.Combine(repository, "bindings", "conformance");
            var zumen = ZumenLibrary.Load(libraryPath);
            var failures = new List<string>();
            var hashes = new List<string>();
            var scenes = 0;
            var files = Directory.GetFiles(Path.Combine(conformance, "cases"), "*.json")
                .OrderBy(path => Path.GetFileNameWithoutExtension(path), StringComparer.Ordinal)
                .ToList();
            foreach (var file in files)
            {
                var name = Path.GetFileNameWithoutExtension(file);
                var steps = (List<object?>)((JsonObject)Json.Parse(File.ReadAllText(file, Utf8))!)["steps"]!;
                Session? session = null;
                // 直前の応答が返したシーンの blob の番号と結果
                (long Blob, JsonObject Result)? lastScene = null;
                for (var index = 0; index < steps.Count; index++)
                {
                    var step = (JsonObject)steps[index]!;
                    var at = name + " の手順 " + index;
                    var action = Actions.First(step.Has);
                    var expected = step.TryGet("status", out var status) ? (string)status! : "ok";
                    var input = step[action];
                    if (action != "session" && session == null)
                    {
                        session = zumen.Open();
                    }
                    string actual;
                    (string Kind, byte[] Data)? output = null;
                    if (action == "session")
                    {
                        session?.Dispose();
                        Session? opened;
                        (actual, opened) = Attempt(() => zumen.Open(BytesOf(input, "options", "options_text")));
                        if (actual != "ok" || opened == null)
                        {
                            // 作れなかったら、閉じたセッションで続ける。境界は、空のセッションへの呼び出しを invalid にする
                            opened = zumen.Open();
                            opened.Dispose();
                        }
                        session = opened;
                    }
                    else if (action == "push")
                    {
                        var (pushName, pushData) = PushOf(repository, (JsonObject)input!);
                        (actual, _) = Attempt(() =>
                        {
                            session!.Push(pushName, pushData);
                            return string.Empty;
                        });
                    }
                    else if (action == "take")
                    {
                        var blob = (uint)((JsonNumber)input!).ToInt64();
                        byte[]? data;
                        (actual, data) = Attempt(() => session!.Take(blob));
                        output = data != null ? ("blob", data) : ((string, byte[])?)null;
                        if (actual == "ok" && data != null && lastScene is { } last && last.Blob == blob)
                        {
                            CheckScene(at, data, last.Result, failures);
                            scenes++;
                        }
                    }
                    else
                    {
                        var request = RequestOf(action, input);
                        byte[]? data;
                        (actual, data) = Attempt(() => session!.CallBytes(request));
                        output = data != null ? ("response", data) : ((string, byte[])?)null;
                    }
                    if (actual != expected)
                    {
                        failures.Add(at + ": 状態が " + actual + "（期待は " + expected + "）");
                    }
                    if (output is not { } written)
                    {
                        continue;
                    }
                    hashes.Add(name + " " + index + " " + written.Kind + " " + Sha256(written.Data));
                    if (written.Kind != "response")
                    {
                        continue;
                    }
                    var response = Json.Parse(Utf8.GetString(written.Data));
                    CheckResponse(at, step, response, failures);
                    lastScene = null;
                    if (response is JsonObject members
                        && members.TryGet("blobs", out var blobs)
                        && members.TryGet("result", out var result)
                        && result is JsonObject resultMembers)
                    {
                        var scene = ((List<object?>)blobs!).OfType<JsonObject>()
                            .FirstOrDefault(entry => entry.TryGet("kind", out var kind) && kind as string == "scene");
                        if (scene != null)
                        {
                            lastScene = (((JsonNumber)scene["id"]!).ToInt64(), resultMembers);
                        }
                    }
                }
                // 動いている呼び出しがないので、何もしない
                session?.Cancel();
                session?.Dispose();
            }

            CompareHashes(conformance, hashes, failures);
            CheckSession(zumen, failures);

            var summary = "包みを通した試験の組 " + files.Count + " 件、ハッシュ " + hashes.Count + " 件、読んだシーン " + scenes + " 件";
            Console.WriteLine(summary);
            var stepSummary = Environment.GetEnvironmentVariable("GITHUB_STEP_SUMMARY");
            if (!string.IsNullOrEmpty(stepSummary))
            {
                var runtime = System.Runtime.InteropServices.RuntimeInformation.FrameworkDescription;
                var bits = IntPtr.Size * 8;
                File.AppendAllText(stepSummary, "### .NET の包みの適合試験（" + runtime + "、" + bits + " bit）\n\n" + summary + "\n", Utf8);
            }
            if (scenes == 0)
            {
                failures.Add("シーンの blob を 1 つも読んでいない");
            }
            if (failures.Count > 0)
            {
                Console.Error.WriteLine("期待と違った:\n" + string.Join("\n", failures));
                return 1;
            }
            Console.WriteLine("ネイティブと同じ一覧になった");
            return 0;
        }

        /// <summary>ハッシュの一覧が hashes.txt と一致するかを確かめる</summary>
        private static void CompareHashes(string conformance, List<string> hashes, List<string> failures)
        {
            var expected = File.ReadAllText(Path.Combine(conformance, "hashes.txt"), Utf8)
                .Split('\n')
                .Where(line => line.Length > 0 && !line.StartsWith("#", StringComparison.Ordinal))
                .ToList();
            if (!hashes.SequenceEqual(expected))
            {
                var missing = expected.Where(line => !hashes.Contains(line)).Select(line => "- " + line);
                var extra = hashes.Where(line => !expected.Contains(line)).Select(line => "+ " + line);
                failures.Add("ハッシュの一覧が hashes.txt と合わない:\n" + string.Join("\n", missing.Concat(extra)));
            }
        }

        /// <summary>文字列を渡す call と、失敗した call と、閉じたあとの呼び出し</summary>
        private static void CheckSession(ZumenLibrary zumen, List<string> failures)
        {
            var major = Versions.SchemaMajor;
            var session = zumen.Open("{\"v\":" + major + "}");
            using (session)
            {
                var response = Json.Parse(session.Call("{\"v\":" + major + ",\"op\":\"capabilities\"}"));
                if (!(Pointer(response, "/result/package") is string) || !(Pointer(response, "/v") is JsonNumber v) || v.ToInt64() != major)
                {
                    failures.Add("call の応答が capabilities の形でない: " + Json.Write(response));
                }
                try
                {
                    session.Call("{\"v\":" + major + ",\"op\":\"open\",\"input\":\"none.dxf\"}");
                    failures.Add("ない入力を開く call が失敗しない");
                }
                catch (ZumenException error)
                {
                    var diagnostics = error.ResponseText == null ? null : Pointer(Json.Parse(error.ResponseText), "/diagnostics") as List<object?>;
                    if (error.Status != Status.Failed || diagnostics == null || diagnostics.Count == 0)
                    {
                        failures.Add("ない入力を開く call の失敗が、診断のある failed でない: " + error.Message);
                    }
                }
#if NET
                // 型つきの要求と応答（System.Text.Json）
                var typed = session.Call<CapabilitiesResult>(new CapabilitiesRequest { V = major });
                if (typed.V != major || string.IsNullOrEmpty(typed.Result?.Package) || typed.Diagnostics == null)
                {
                    failures.Add("型つきの capabilities の応答を読めない");
                }
                try
                {
                    session.Call<OpenResult>(new OpenRequest { V = major, Input = "none.dxf" });
                    failures.Add("型つきの、ない入力を開く call が失敗しない");
                }
                catch (ZumenException error)
                {
                    if (error.Response is not { } failed || failed.Diagnostics.Count == 0 || failed.Diagnostics[0].Severity != Severity.Error)
                    {
                        failures.Add("型つきの失敗の応答に、error の診断がない");
                    }
                }
#endif
                // ほかのスレッドから Cancel を呼べる（動いている呼び出しがなければ、何もしない）
                var thread = new Thread(session.Cancel);
                thread.Start();
                thread.Join();
            }
            var (status, _) = Attempt(() => session.Call("{\"v\":" + major + ",\"op\":\"capabilities\"}"));
            if (!session.IsClosed || status != "invalid")
            {
                failures.Add("閉じたあとの call が " + status + "（期待は invalid）");
            }
        }
    }
}
