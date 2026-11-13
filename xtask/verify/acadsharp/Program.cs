// 検証キットの DWG を ACadSharp で読み、図形をハンドルごとに書き出す（#503）。
//
// 使い方: dotnet run --project xtask/verify/acadsharp -c Release -- <キットのフォルダー>
//
// キットの manifest.json の DWG を読み、<キット>/acadsharp-dump.json に、ファイルごとの読みの通知・誤り・ヘッダーの点の
// 表し方・画層の名前と色と、図形の値（ACadSharp のまま。角度はラジアン）を書く。値を図面モデルの形に直して zumen の
// 読みと照らすのは check_acadsharp.py。
using System.Globalization;
using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization;
using ACadSharp;
using ACadSharp.Entities;
using ACadSharp.IO;

if (args.Length != 1)
{
    Console.Error.WriteLine("使い方: dotnet run --project xtask/verify/acadsharp -c Release -- <キットのフォルダー>");
    return 2;
}
var folder = args[0];
var manifest = JsonNode.Parse(File.ReadAllText(Path.Combine(folder, "manifest.json")))!;
var files = new JsonObject();
foreach (var entry in manifest["files"]!.AsArray())
{
    // probe は apply の応答だけを残し、ファイルを持たない。当たらなかった組もファイルを持たない
    var status = (string?)entry!["status"];
    if ((string?)entry["format"] != "dwg" || (string?)entry["role"] == "probe" || (status != null && status != "ok"))
    {
        continue;
    }
    var path = (string)entry["path"]!;
    files[path] = Dwg.Dump(Path.Combine(folder, path));
}
var version = typeof(CadDocument).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;
var output = new JsonObject { ["acadsharp"] = version, ["files"] = files };
// 受け取った図面の NaN・無限大は、文字列（"NaN" など）で書く。数でないので、照らすと食い違いになる
var options = new JsonSerializerOptions { WriteIndented = true, NumberHandling = JsonNumberHandling.AllowNamedFloatingPointLiterals };
File.WriteAllText(Path.Combine(folder, "acadsharp-dump.json"), output.ToJsonString(options) + "\n");
Console.WriteLine($"ACadSharp {version} で {files.Count} の DWG を読んだ");
return 0;

static class Dwg
{
    static ulong FileHandleSeed(string path)
    {
        using var reader = new DwgReader(path);
        return reader.ReadHeader().HandleSeed;
    }

    public static JsonObject Dump(string path)
    {
        var notes = new JsonArray();
        var result = new JsonObject { ["notifications"] = notes };
        CadDocument doc;
        try
        {
            using var reader = new DwgReader(path, (_, e) => notes.Add(new JsonObject
            {
                ["type"] = e.NotificationType.ToString(),
                ["message"] = e.Message,
            }));
            doc = reader.Read();
        }
        catch (Exception error)
        {
            // 読めないことそのものを結果にする
            result["error"] = $"{error.GetType().Name}: {error.Message}";
            return result;
        }
        try
        {
            result["version"] = doc.Header.Version.ToString();
            result["header"] = new JsonObject
            {
                ["PDMODE"] = (int)doc.Header.PointDisplayMode,
                ["PDSIZE"] = doc.Header.PointDisplaySize,
                // ファイルに書かれた、次に振るハンドル。足した図形で大きさ（バイト）が増えたときの書き直しを照らす（#701）。
                // doc.Header.HandleSeed は、読むときに登録したオブジェクトと ACadSharp が作ったオブジェクトで
                // 引き上げられるので、ヘッダーだけを読み直した値を使う
                ["HANDSEED"] = FileHandleSeed(path),
                // HANDSEED より後ろに書く変数。HANDSEED の大きさが変わったときに、後ろのビットがずれていないことを照らす
                ["INSUNITS"] = (int)doc.Header.InsUnits,
                ["PLIMMIN"] = new JsonArray(doc.Header.PaperSpaceLimitsMin.X, doc.Header.PaperSpaceLimitsMin.Y),
                ["PLIMMAX"] = new JsonArray(doc.Header.PaperSpaceLimitsMax.X, doc.Header.PaperSpaceLimitsMax.Y),
                ["CLAYER"] = doc.Header.CurrentLayerName,
            };
            var layers = new JsonObject();
            foreach (var layer in doc.Layers)
            {
                layers[layer.Name] = Index(layer.Color);
            }
            // 画層の名前と色（ACI。真の色なら null）
            result["layers"] = layers;
            var entities = new JsonObject();
            foreach (var record in doc.BlockRecords)
            {
                foreach (var entity in record.Entities)
                {
                    Add(entities, entity, record.Name);
                }
            }
            result["entities"] = entities;
        }
        catch (Exception error)
        {
            // 読めた後の値の写しで止まっても、ほかのファイルは書く
            result["error"] = $"{error.GetType().Name}: {error.Message}";
        }
        return result;
    }

    static void Add(JsonObject entities, Entity e, string owner)
    {
        var d = new JsonObject
        {
            ["type"] = e.ObjectName,
            ["layer"] = e.Layer?.Name,
            ["color"] = Index(e.Color),
            ["owner"] = owner,
        };
        switch (e)
        {
            case Line line:
                d["start"] = Xy(line.StartPoint);
                d["end"] = Xy(line.EndPoint);
                break;
            // Arc は Circle を継ぐので先に見る
            case Arc arc:
                d["center"] = Xy(arc.Center);
                d["radius"] = arc.Radius;
                d["start_angle"] = arc.StartAngle;
                d["end_angle"] = arc.EndAngle;
                break;
            case Circle circle:
                d["center"] = Xy(circle.Center);
                d["radius"] = circle.Radius;
                break;
            case Ellipse ellipse:
                d["center"] = Xy(ellipse.Center);
                d["major_axis"] = Xy(ellipse.MajorAxisEndPoint);
                d["ratio"] = ellipse.RadiusRatio;
                d["start_param"] = ellipse.StartParameter;
                d["end_param"] = ellipse.EndParameter;
                break;
            case LwPolyline polyline:
                d["points"] = new JsonArray(polyline.Vertices
                    .Select(v => (JsonNode?)new JsonArray(v.Location.X, v.Location.Y, v.Bulge))
                    .ToArray());
                d["closed"] = polyline.IsClosed;
                d["has_width"] = polyline.ConstantWidth != 0.0
                    || polyline.Vertices.Any(v => v.StartWidth != 0.0 || v.EndWidth != 0.0);
                break;
            case Point point:
                d["location"] = Xy(point.Location);
                d["rotation"] = point.Rotation;
                break;
            case MText mtext:
                d["text"] = mtext.Value;
                d["insert"] = Xy(mtext.InsertPoint);
                d["height"] = mtext.Height;
                // 枠の幅（0 なら枠なし）
                d["width"] = mtext.RectangleWidth;
                d["rotation"] = mtext.Rotation;
                d["attachment"] = (int)mtext.AttachmentPoint;
                d["style"] = mtext.Style?.Name;
                break;
            case TextEntity text:
                d["text"] = text.Value;
                d["insert"] = Xy(text.InsertPoint);
                d["align_point"] = Xy(text.AlignmentPoint);
                d["height"] = text.Height;
                d["rotation"] = text.Rotation;
                d["width_factor"] = text.WidthFactor;
                d["oblique"] = text.ObliqueAngle;
                d["mirror"] = (int)text.Mirror;
                d["halign"] = (int)text.HorizontalAlignment;
                d["valign"] = (int)text.VerticalAlignment;
                d["style"] = text.Style?.Name;
                break;
            case Insert insert:
                d["name"] = insert.Block?.Name;
                d["insert"] = Xy(insert.InsertPoint);
                d["xscale"] = insert.XScale;
                d["yscale"] = insert.YScale;
                d["rotation"] = insert.Rotation;
                d["columns"] = insert.ColumnCount;
                d["rows"] = insert.RowCount;
                d["column_spacing"] = insert.ColumnSpacing;
                d["row_spacing"] = insert.RowSpacing;
                d["attributes"] = new JsonArray(insert.Attributes
                    .Select(a => (JsonNode?)new JsonObject { ["tag"] = a.Tag, ["value"] = a.Value })
                    .ToArray());
                break;
        }
        entities[e.Handle.ToString("X", CultureInfo.InvariantCulture)] = d;
    }

    // ACI の番号（BYLAYER は 256、BYBLOCK は 0）。真の色は番号を持たない
    static JsonNode? Index(Color color) => color.IsTrueColor ? null : JsonValue.Create(color.Index);

    static JsonArray Xy(CSMath.XYZ p) => new(p.X, p.Y);

    static JsonArray Xy(CSMath.XY p) => new(p.X, p.Y);
}
