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
        foreach (var (version, name, withStyles) in new[] {
            (ACadVersion.AC1015, "table-r2000.dwg", false),
            (ACadVersion.AC1018, "table-r2004.dwg", false),
            (ACadVersion.AC1018, "table-add-r2004.dwg", true),
            (ACadVersion.AC1024, "table-r2010.dwg", false),
            (ACadVersion.AC1027, "table-r2013.dwg", false),
            (ACadVersion.AC1032, "table-r2018.dwg", false),
            (ACadVersion.AC1024, "table-styled-r2010.dwg", true),
            (ACadVersion.AC1027, "table-styled-r2013.dwg", true),
            (ACadVersion.AC1032, "table-styled-r2018.dwg", true),
            (ACadVersion.AC1032, "table-partial-standard-r2018.dwg", true),
            (ACadVersion.AC1032, "table-title-standard-r2018.dwg", true),
            (ACadVersion.AC1032, "table-title-custom-border-r2018.dwg", true) })
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

            // 既定の表スタイルは文書に登録されたものを使い、各セルスタイルの文字スタイルも結ぶ。
            var tableStyle = doc.TableStyles!["Standard"];
            if (withStyles)
                foreach (var style in new[] { tableStyle.TableCellStyle, tableStyle.DataCellStyle, tableStyle.HeaderCellStyle, tableStyle.TitleCellStyle })
                    style.TextStyle = doc.TextStyles["Standard"];
            // 表全体の色を指定する。表示用の新規ブロックでは、まだこの書式を扱わない。
            if (name == "table-partial-standard-r2018.dwg")
            {
                tableStyle.TableCellStyle.HasData = true;
                tableStyle.TableCellStyle.TextColor = new Color(3);
                tableStyle.TableCellStyle.PropertyOverrideFlags = ACadSharp.Objects.TableStyle.CellStylePropertyFlags.ContentColor;
            }
            // 四方 1.5 の余白、表題の結合、表示する単線、表題・見出し・データそれぞれの位置合わせ。
            if (name is "table-title-standard-r2018.dwg" or "table-title-custom-border-r2018.dwg")
            {
                doc.TextStyles["Standard"].Filename = "arial.ttf";
                foreach (var (style, height, alignment) in new[]{
                    (tableStyle.TableCellStyle, 0.18, 1), (tableStyle.TitleCellStyle, 6.0, 5),
                    (tableStyle.HeaderCellStyle, 4.5, 5), (tableStyle.DataCellStyle, 4.5, 2)})
                {
                    style.HasData = true;
                    style.Type = ACadSharp.Objects.TableStyle.CellStyleType.Table;
                    style.ContentLayoutFlags = ACadSharp.Objects.TableStyle.CellContentLayoutFlags.Flow;
                    style.TextHeight = height;
                    style.Alignment = alignment;
                    style.Scale = 1;
                    style.ValueDataType = 4;
                    style.ValueUnitType = 0;
                    style.MarginOverrideFlags = ACadSharp.Objects.TableStyle.MarginFlags.Override;
                    style.HorizontalMargin = style.VerticalMargin = style.RightMargin = style.BottomMargin = 1.5;
                    style.PropertyOverrideFlags = ACadSharp.Objects.TableStyle.CellStylePropertyFlags.None;
                    style.TableCellStylePropertyFlags = style == tableStyle.TitleCellStyle
                        ? ACadSharp.Objects.TableStyle.CellStylePropertyFlags.MergeAll
                        : ACadSharp.Objects.TableStyle.CellStylePropertyFlags.None;
                    style.TopBorder = new(ACadSharp.Objects.TableStyle.CellEdgeFlags.Top);
                    style.RightBorder = new(ACadSharp.Objects.TableStyle.CellEdgeFlags.Right);
                    style.BottomBorder = new(ACadSharp.Objects.TableStyle.CellEdgeFlags.Bottom);
                    style.LeftBorder = new(ACadSharp.Objects.TableStyle.CellEdgeFlags.Left);
                    foreach (var border in new[]{style.TopBorder,style.RightBorder,style.BottomBorder,style.LeftBorder,style.HorizontalInsideBorder,style.VerticalInsideBorder})
                    {
                        border.ApplyBorder = style != tableStyle.TableCellStyle;
                        border.LineWeight = LineWeightType.ByBlock;
                        border.Color = Color.ByBlock;
                        border.PropertyOverrideFlags = 0;
                        border.Type = ACadSharp.Objects.TableStyle.BorderType.Single;
                    }
                }
            }
            if (name == "table-title-custom-border-r2018.dwg")
                tableStyle.HeaderCellStyle.TopBorder.Color = new Color(3);
            if (withStyles && version == ACadVersion.AC1018)
                foreach (var style in new[] { tableStyle.DataCellStyle, tableStyle.HeaderCellStyle, tableStyle.TitleCellStyle })
                {
                    style.TextHeight = 2.5;
                    style.CellAlignment = ACadSharp.Objects.TableStyle.CellAlignmentType.MiddleCenter;
                }
            var block = new BlockRecord("*T1");
            foreach (double y in new[] { 0.0, -10.0, -25.0 })
                block.Entities.Add(new Line(new XYZ(0, y, 0), new XYZ(50, y, 0)));
            foreach (double x in new[] { 0.0, 20.0, 50.0 })
                block.Entities.Add(new Line(new XYZ(x, 0, 0), new XYZ(x, -25, 0)));
            var table = new TableEntity(block) { InsertPoint = new XYZ(10, 20, 0), HorizontalDirection = XYZ.AxisX };
            if (withStyles)
                table.Style = tableStyle;
            table.Columns.Add(new TableEntity.Column { Width = 20 });
            table.Columns.Add(new TableEntity.Column { Width = 30 });
            for (int r = 0; r < 2; r++)
            {
                var row = new TableEntity.Row { Height = 10 + r * 5 };
                for (int c = 0; c < 2; c++)
                {
                    string text = version < ACadVersion.AC1024 ? $"A{r}{c}" : $"表{r}{c}";
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
            Line? placeholder = null;
            if (version < ACadVersion.AC1024)
            {
                // ACadSharp 3.8.0 は旧版の表を書かない。自作の Rust の生成器が、この線を表のレコードに置き換える。
                placeholder = new Line(new XYZ(10, 20, 0), new XYZ(11, 20, 0));
                doc.Entities.Add(placeholder);
            }
            doc.Entities.Add(new Line(new XYZ(-20, -20, 0), new XYZ(-10, -10, 0)));
            var path = Path.Combine(folder, name);
            DwgWriter.Write(path, doc);
            if (withStyles && version >= ACadVersion.AC1024)
                ValidateReferences(path);
            if (placeholder != null)
            {
                File.WriteAllLines(Path.Combine(folder, name + ".meta"), new[] {
                    doc.Classes.GetByName("ACAD_TABLE").ClassNumber.ToString(),
                    placeholder.Handle.ToString(), block.Handle.ToString(),
                    doc.Layers["0"].Handle.ToString() });
            }
            Console.WriteLine(name);
        }
    }

    static void ValidateReferences(string path)
    {
        var notes = new List<string>();
        using var reader = new DwgReader(path, (_, e) => notes.Add(e.Message));
        var doc = reader.Read();
        var table = doc.Entities.OfType<TableEntity>().Single();
        if (notes.Count != 0 || table.Style.Handle == 0 || !ReferenceEquals(table.Style, doc.TableStyles!["Standard"]))
            throw new InvalidOperationException($"表のスタイル参照を読み戻せない: {string.Join("; ", notes)}");
        foreach (var style in new[] { table.Style.TableCellStyle, table.Style.DataCellStyle, table.Style.HeaderCellStyle, table.Style.TitleCellStyle })
            if (style.TextStyle.Handle == 0 || !ReferenceEquals(style.TextStyle, doc.TextStyles["Standard"]))
                throw new InvalidOperationException("セルスタイルの文字スタイル参照を読み戻せない");
    }
}
