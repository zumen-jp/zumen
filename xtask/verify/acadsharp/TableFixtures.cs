// 自作の 2 行 2 列の表を ACadSharp（MIT）で DWG にする。製品の付属ファイルは使わない。
// dotnet run --project xtask/verify/acadsharp -c Release -- --make-table-fixtures crates/zumen-acad/tests/fixtures
using ACadSharp;
using ACadSharp.Entities;
using ACadSharp.IO;
using ACadSharp.Tables;
using ACadSharp.Types.Units;
using CSMath;

static class TableFixtures
{
    public static void Write(string folder)
    {
        Directory.CreateDirectory(folder);
        foreach (var (version, name) in new[] {
            (ACadVersion.AC1024, "table-r2010.dwg"),
            (ACadVersion.AC1027, "table-r2013.dwg"),
            (ACadVersion.AC1032, "table-r2018.dwg") })
        {
            var doc = new CadDocument(version);
            var date = new DateTime(2026, 1, 1, 0, 0, 0, DateTimeKind.Utc);
            doc.Header.CreateDateTime = date;
            doc.Header.UpdateDateTime = date;
            doc.Header.UniversalCreateDateTime = date;
            doc.Header.UniversalUpdateDateTime = date;
            doc.Header.LastSavedBy = "";
            doc.Header.InsUnits = UnitsType.Millimeters;
            doc.SummaryInfo.Author = "";
            doc.SummaryInfo.LastSavedBy = "";
            doc.SummaryInfo.CreatedDate = date;
            doc.SummaryInfo.ModifiedDate = date;

            var block = new BlockRecord("*T1");
            foreach (double y in new[] { 0.0, -10.0, -25.0 })
                block.Entities.Add(new Line(new XYZ(0, y, 0), new XYZ(50, y, 0)));
            foreach (double x in new[] { 0.0, 20.0, 50.0 })
                block.Entities.Add(new Line(new XYZ(x, 0, 0), new XYZ(x, -25, 0)));
            var table = new TableEntity(block) { InsertPoint = new XYZ(10, 20, 0), HorizontalDirection = XYZ.AxisX };
            table.Columns.Add(new TableEntity.Column { Width = 20 });
            table.Columns.Add(new TableEntity.Column { Width = 30 });
            for (int r = 0; r < 2; r++)
            {
                var row = new TableEntity.Row { Height = 10 + r * 5 };
                for (int c = 0; c < 2; c++)
                {
                    string text = $"表{r}{c}";
                    var cell = new TableEntity.Cell { Type = TableEntity.CellType.Text };
                    var content = new TableEntity.CellContent { ContentType = TableEntity.TableCellContentType.Value };
                    content.CadValue.SetValue(text, CadValueType.String);
                    content.CadValue.FormattedValue = text;
                    cell.Contents.Add(content);
                    row.Cells.Add(cell);
                    block.Entities.Add(new TextEntity { Value = text, InsertPoint = new XYZ(3 + c * 20, -5 - r * 12.5, 0), Height = 2 });
                }
                table.Rows.Add(row);
            }
            doc.Entities.Add(table);
            doc.Entities.Add(new Line(new XYZ(-20, -20, 0), new XYZ(-10, -10, 0)));
            DwgWriter.Write(Path.Combine(folder, name), doc);
            Console.WriteLine(name);
        }
    }
}
