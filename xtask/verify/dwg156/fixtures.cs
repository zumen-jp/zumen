// #156 の自作 DWG を ACadSharp で書き、もう一度読んで対象の値を確かめる。
// .NET 10: dotnet run --file fixtures.cs -- <空の出力フォルダー>
#:property PublishAot=false
#:package ACadSharp@3.8.0
using ACadSharp;
using ACadSharp.Entities;
using ACadSharp.IO;
using ACadSharp.Objects;
using ACadSharp.Tables;
using ACadSharp.Types.Units;
using CSMath;

if (args.Length != 1)
    throw new ArgumentException("引数に空の出力フォルダーを指定してください。");
var folder = Directory.CreateDirectory(args[0]);
if (folder.EnumerateFileSystemInfos().Any())
    throw new IOException("出力フォルダーには既にファイルがあります。");

foreach (var version in new[] { ACadVersion.AC1024, ACadVersion.AC1027, ACadVersion.AC1032 })
{
    var doc = NewDocument(version);
    doc.UCSs.Add(new UCS("LOCAL_TEST") { Origin = new XYZ(10, 20, 3), XAxis = XYZ.AxisY, YAxis = -XYZ.AxisX });
    var record = new XRecord("TEST_TEXT");
    record.CreateEntry(1, "sample-α");
    doc.RootDictionary.Add("TEST_TEXT", record);
    var leader = new Leader { ArrowHeadEnabled = true };
    var vertices = new[] { XYZ.Zero, new XYZ(5, 2, 0), new XYZ(8, 2, 0) };
    leader.Vertices.AddRange(vertices);
    doc.Entities.Add(leader);

    var copy = WriteAndRead(doc, "fixture");
    var ucs = copy.UCSs["LOCAL_TEST"];
    var text = copy.RootDictionary.GetEntry<XRecord>("TEST_TEXT").Entries.Single(entry => entry.Code == 1);
    var line = copy.Entities.OfType<Leader>().Single();
    if (ucs.Origin != new XYZ(10, 20, 3) || ucs.XAxis != XYZ.AxisY || ucs.YAxis != -XYZ.AxisX ||
        !Equals(text.Value, "sample-α") || !line.ArrowHeadEnabled || !line.Vertices.SequenceEqual(vertices))
        throw new InvalidDataException("UCS・XRECORD・LEADER の値が合いません。");

    if (version == ACadVersion.AC1032)
        continue;
    var gridDoc = NewDocument(version);
    gridDoc.Header.InsUnits = UnitsType.Millimeters;
    var block = new BlockRecord("GRID");
    block.Entities.Add(new Line(XYZ.Zero, XYZ.AxisX));
    gridDoc.Entities.Add(new Insert(block) { InsertPoint = new XYZ(10, 20, 0), RowCount = 2, ColumnCount = 3,
        RowSpacing = 12, ColumnSpacing = 20 });
    var grid = WriteAndRead(gridDoc, "grid").Entities.OfType<Insert>().Single();
    if (grid.InsertPoint != new XYZ(10, 20, 0) || grid.Block.Name != "GRID" || grid.Attributes.Any() ||
        grid.RowCount != 2 || grid.ColumnCount != 3 || grid.RowSpacing != 12 || grid.ColumnSpacing != 20)
        throw new InvalidDataException("格子の値が合いません。");
}

CadDocument WriteAndRead(CadDocument doc, string kind)
{
    var path = Path.Combine(folder.FullName, $"{kind}-{doc.Header.Version}.dwg");
    DwgWriter.Write(path, doc);
    var copy = DwgReader.Read(path);
    if (copy.Header.Version != doc.Header.Version)
        throw new InvalidDataException("DWG の版が合いません。");
    return copy;
}

static CadDocument NewDocument(ACadVersion version)
{
    var doc = new CadDocument(version);
    var date = new DateTime(2026, 1, 1, 0, 0, 0, DateTimeKind.Utc);
    doc.Header.CreateDateTime = doc.Header.UpdateDateTime = date;
    doc.Header.UniversalCreateDateTime = doc.Header.UniversalUpdateDateTime = date;
    doc.SummaryInfo.CreatedDate = doc.SummaryInfo.ModifiedDate = date;
    doc.Header.LastSavedBy = doc.SummaryInfo.Author = doc.SummaryInfo.LastSavedBy = "";
    return doc;
}
