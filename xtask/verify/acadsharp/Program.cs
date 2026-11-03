// 確認の束の DWG を ACadSharp で読み、図形をハンドルごとに書き出す（#503）。
//
// 使い方: dotnet run --project xtask/verify/acadsharp -c Release -- <束のフォルダー>
//
// 束の manifest.json の DWG を読み、<束>/acadsharp-dump.json に、ファイルごとの読みの通知・誤り・見出しの点の
// 表し方・画層の名前と色と、図形の値（ACadSharp のまま。角度はラジアン）を書く。値を図面モデルの形に直して zumen の
// 読みと照らすのは check_acadsharp.py。
using System.Globalization;
using System.Reflection;
using System.Text.Json;
using System.Text.Json.Nodes;
using ACadSharp;
using ACadSharp.Entities;
using ACadSharp.IO;

if (args.Length != 1)
{
    Console.Error.WriteLine("使い方: dotnet run --project xtask/verify/acadsharp -c Release -- <束のフォルダー>");
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
File.WriteAllText(Path.Combine(folder, "acadsharp-dump.json"), output.ToJsonString(new JsonSerializerOptions { WriteIndented = true }) + "\n");
Console.WriteLine($"ACadSharp {version} で {files.Count} の DWG を読んだ");
return 0;

static class Dwg
{
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
        result["version"] = doc.Header.Version.ToString();
        result["header"] = new JsonObject
        {
            ["PDMODE"] = (int)doc.Header.PointDisplayMode,
            ["PDSIZE"] = doc.Header.PointDisplaySize,
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
