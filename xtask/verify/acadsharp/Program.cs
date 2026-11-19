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

if (args.Length == 2 && args[0] == "--make-table-fixtures")
{
    TableFixtures.Write(args[1]);
    return 0;
}
if (args.Length == 2 && args[0] == "--make-dim-dstyle-fixtures")
{
    DimensionFixtures.Write(args[1]);
    return 0;
}
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
            result["blocks"] = new JsonArray(doc.BlockRecords.Select(r => (JsonNode?)r.Name).ToArray());
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
            // 線種の名前（画層に従うなら ByLayer、記号の配置に従うなら ByBlock）
            ["line_type"] = e.LineType?.Name,
            ["owner"] = owner,
        };
        switch (e)
        {
            case Hatch hatch:
                d["hatch"] = HatchGeometry(hatch);
                break;
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
            case TableEntity table:
                d["table_geometry"] = TableGeometry(table);
                d["table_part"] = new JsonObject { ["block"] = table.Block?.Name,
                    ["position"] = Xy(table.InsertPoint), ["direction"] = Xy(table.HorizontalDirection) };
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
            case Dimension dim:
                d["block"] = dim.Block?.Name;
                d["measurement"] = dim.Measurement;
                d["text_midpoint"] = Xy(dim.TextMiddlePoint);
                d["text"] = dim.Text ?? "";
                if (dim is DimensionArc arcDimension)
                    d["arc_dimension"] = new JsonObject {
                        ["center"] = Xy(arcDimension.Center), ["first"] = Xy(arcDimension.FirstPoint),
                        ["second"] = Xy(arcDimension.SecondPoint), ["arc"] = Xy(arcDimension.DefinitionPoint),
                        ["partial"] = arcDimension.IsPartial, ["has_leader"] = arcDimension.HasLeader,
                        ["start_angle"] = arcDimension.StartAngle, ["end_angle"] = arcDimension.EndAngle,
                        ["leader_first"] = Xy(arcDimension.LeaderPoint1), ["leader_second"] = Xy(arcDimension.LeaderPoint2)
                    };
                break;
        }
        entities[e.Handle.ToString("X", CultureInfo.InvariantCulture)] = d;
    }

    // 平面の表の位置・大きさと、結合していない文字列のセルを照らす。
    static JsonNode? TableGeometry(TableEntity table)
    {
        if (table.InsertPoint.Z != 0 || table.HorizontalDirection.Z != 0 || table.MergedCellRanges.Count != 0) return null;
        var cells = new JsonArray();
        foreach (var row in table.Rows)
            foreach (var cell in row.Cells)
            {
                if (cell.Contents.Count != 1) return null;
                // 旧版の読み手は、文字の値を String として返し、内容の種類を Unknown のままにする。
                bool legacyText = table.Document != null && table.Document.Header.Version < ACadVersion.AC1024
                    && cell.Type == TableEntity.CellType.Text && cell.Content.ContentType == TableEntity.TableCellContentType.Unknown;
                if ((!legacyText && cell.Content.ContentType != TableEntity.TableCellContentType.Value)
                    || cell.Content.CadValue.ValueType != CadValueType.String) return null;
                var value = cell.Content.CadValue;
                string text = string.IsNullOrEmpty(value.FormattedValue) ? value.Value?.ToString() ?? "" : value.FormattedValue;
                cells.Add(text.Length == 0 ? new JsonObject { ["content"] = "empty" }
                    : new JsonObject { ["content"] = "text", ["text"] = text });
            }
        return new JsonObject { ["shape"] = "table", ["position"] = Xy(table.InsertPoint), ["direction"] = Xy(table.HorizontalDirection),
            ["row_heights"] = new JsonArray(table.Rows.Select(row => (JsonNode?)row.Height).ToArray()),
            ["column_widths"] = new JsonArray(table.Columns.Select(column => (JsonNode?)column.Width).ToArray()), ["cells"] = cells };
    }

    // ハッチングのうち API で表せる境界と模様。角度はここでも ACadSharp のラジアンのまま。
    static JsonNode? HatchGeometry(Hatch hatch)
    {
        if (hatch.IsAssociative || hatch.Paths.Count == 0 || hatch.GradientColor.Colors.Count != 0) return null;
        var loops = new JsonArray();
        foreach (var path in hatch.Paths)
        {
            int flags = (int)path.Flags;
            if ((flags & 12) != 0 || path.Entities.Count != 0 || path.Edges.Count != 1) return null;
            JsonObject boundary;
            if (path.Edges[0] is Hatch.BoundaryPath.Polyline polyline)
            {
                if (!polyline.IsClosed || polyline.Vertices.Count < 2) return null;
                boundary = new JsonObject { ["shape"] = "polyline", ["closed"] = true,
                    ["points"] = new JsonArray(polyline.Vertices.Select(p => (JsonNode?)Xy(p)).ToArray()) };
                if (polyline.Vertices.Any(p => p.Z != 0)) boundary["bulges"] = new JsonArray(polyline.Vertices.Select(p => (JsonNode?)p.Z).ToArray());
            }
            else if (path.Edges[0] is Hatch.BoundaryPath.Arc arc && arc.StartAngle == 0 && arc.EndAngle == Math.Tau && arc.CounterClockWise)
                boundary = new JsonObject { ["shape"] = "circle", ["center"] = Xy(arc.Center), ["radius"] = arc.Radius };
            else return null;
            loops.Add(new JsonObject { ["boundary"] = boundary, ["external"] = (flags & 1) != 0, ["outermost"] = (flags & 16) != 0 });
        }
        var result = new JsonObject { ["shape"] = "hatch", ["loops"] = loops,
            ["islands"] = new[] { "nested", "outer", "ignore" }[(int)hatch.Style], ["fill"] = hatch.IsSolid ? "solid" : "pattern" };
        if (!hatch.IsSolid)
            result["pattern"] = new JsonObject { ["name"] = hatch.Pattern.Name,
                ["origin"] = new[] { "user_defined", "predefined", "custom" }[(int)hatch.PatternType],
                ["angle"] = hatch.PatternAngle, ["scale"] = hatch.PatternScale, ["double"] = hatch.IsDouble,
                ["lines"] = new JsonArray(hatch.Pattern.Lines.Select(line => (JsonNode?)new JsonObject {
                    ["angle"] = line.Angle, ["base"] = Xy(line.BasePoint), ["offset"] = Xy(line.Offset),
                    ["dashes"] = new JsonArray(line.DashLengths.Select(value => (JsonNode?)value).ToArray()) }).ToArray()) };
        if (hatch.SeedPoints.Count != 0) result["seeds"] = new JsonArray(hatch.SeedPoints.Select(point => (JsonNode?)Xy(point)).ToArray());
        return result;
    }

    // ACI の番号（BYLAYER は 256、BYBLOCK は 0）。真の色は番号を持たない
    static JsonNode? Index(Color color) => color.IsTrueColor ? null : JsonValue.Create(color.Index);

    static JsonArray Xy(CSMath.XYZ p) => new(p.X, p.Y);

    static JsonArray Xy(CSMath.XY p) => new(p.X, p.Y);
}
