// 自作の R2010・R2013・R2018 の表スタイル・セル書式・色見本帳の検証用 DWG/DXF を ACadSharp（MIT）で作る（#906）。
// dotnet run --project xtask/verify/acadsharp -c Release -- --make-table-format-fixtures crates/zumen-acad/tests/fixtures
using System.Globalization;
using System.Text;
using ACadSharp;
using ACadSharp.Entities;
using ACadSharp.IO;
using ACadSharp.Objects;
using ACadSharp.Tables;
using ACadSharp.Types.Units;
using CSMath;

static class TableFormatFixtures
{
    public static void Write(string folder)
    {
        Directory.CreateDirectory(folder);
        foreach (var (version, tag) in new[]
        {
            (ACadVersion.AC1024, "r2010"),
            (ACadVersion.AC1027, "r2013"),
            (ACadVersion.AC1032, "r2018"),
        })
        {
            var doc = CreateBaseDocument(version, setMillimeters: false);

            var book = new BookColor
            {
                BookName = "SamplePocket",
                ColorName = "Harbor",
                Color = new Color(13, 97, 184),
            };
            doc.Colors.Add(book);

            var circle = new Circle
            {
                Center = new XYZ(5, 6, 0),
                Radius = 2,
                BookColor = book,
            };
            doc.Entities.Add(circle);

            var style = new TableStyle("SampleFormat");
            style.DataCellStyle.TextColor = new Color(2);
            style.DataCellStyle.TextHeight = 1.75;
            style.TitleCellStyle.TextHeight = style.HeaderCellStyle.TextHeight = 1.75;
            style.TitleCellStyle.Alignment = style.HeaderCellStyle.Alignment = style.DataCellStyle.Alignment = 5;
            style.TitleCellStyle.TextStyle = style.HeaderCellStyle.TextStyle = style.DataCellStyle.TextStyle = doc.TextStyles["Standard"];
            style.DataCellStyle.HasData = true;
            style.DataCellStyle.Color = new Color(2);
            style.DataCellStyle.PropertyOverrideFlags = style.DataCellStyle.TableCellStylePropertyFlags =
                TableStyle.CellStylePropertyFlags.ContentColor | TableStyle.CellStylePropertyFlags.TextHeight;
            doc.TableStyles.Add(style);

            var symbol = new BlockRecord("*T1");
            doc.BlockRecords.Add(symbol);
            var table = new TableEntity(symbol) { Style = style };
            table.Columns.Add(new TableEntity.Column { Width = 15 });
            table.Columns.Add(new TableEntity.Column { Width = 18 });
            var row = new TableEntity.Row { Height = 8 };
            for (int k = 0; k < 2; k++)
            {
                var cell = new TableEntity.Cell();
                var content = new TableEntity.CellContent
                {
                    ContentType = TableEntity.TableCellContentType.Value,
                };
                string text = $"cell{k}";
                content.CadValue.SetValue(text, CadValueType.String);
                content.CadValue.FormattedValue = text;
                cell.Contents.Add(content);
                cell.StyleOverride.TextColor = new Color((short)(3 + k));
                cell.StyleOverride.TextHeight = 2.5 + k;
                cell.StyleOverride.TableCellStylePropertyFlags =
                    TableStyle.CellStylePropertyFlags.ContentColor | TableStyle.CellStylePropertyFlags.TextHeight;
                cell.StyleOverride.HasData = true;
                cell.StyleOverride.TextStyle = doc.TextStyles["Standard"];
                cell.StyleOverride.PropertyOverrideFlags = cell.StyleOverride.TableCellStylePropertyFlags;
                cell.StyleOverride.Color = new Color((short)(3 + k));
                row.Cells.Add(cell);
            }
            table.Rows.Add(row);
            doc.Entities.Add(table);

            string dwgPath = Path.Combine(folder, $"table-style-{tag}.dwg");
            string dxfPath = Path.Combine(folder, $"table-style-{tag}.dxf");
            DwgWriter.Write(dwgPath, doc);
            DxfWriter.Write(dxfPath, doc);
            CompleteControlDxf(dxfPath);
            VerifyStylePair(dwgPath, isDwg: true);
            VerifyStylePair(dxfPath, isDwg: false);
            Console.WriteLine($"table-style-{tag}.{{dwg,dxf}}");
        }

        WriteCleanAndPartialCases(Path.Combine(folder, "table-format-cases-r2018.dwg"));
        Console.WriteLine("table-format-cases-r2018.dwg");
    }

    static CadDocument CreateBaseDocument(ACadVersion version, bool setMillimeters)
    {
        var doc = new CadDocument(version);
        var date = new DateTime(2026, 1, 1, 0, 0, 0, DateTimeKind.Utc);
        doc.Header.CreateDateTime = date;
        doc.Header.UpdateDateTime = date;
        doc.Header.UniversalCreateDateTime = date;
        doc.Header.UniversalUpdateDateTime = date;
        doc.Header.LastSavedBy = "";
        if (setMillimeters)
        {
            doc.Header.InsUnits = UnitsType.Millimeters;
        }
        doc.SummaryInfo.Author = "";
        doc.SummaryInfo.LastSavedBy = "";
        doc.SummaryInfo.CreatedDate = date;
        doc.SummaryInfo.ModifiedDate = date;
        return doc;
    }

    static void CompleteControlDxf(string path)
    {
        var lines = File.ReadAllLines(path, new UTF8Encoding(encoderShouldEmitUTF8Identifier: true));
        var tags = new List<(int Code, string Value)>(lines.Length / 2);
        for (int i = 0; i + 1 < lines.Length; i += 2)
        {
            tags.Add((int.Parse(lines[i].Trim(), CultureInfo.InvariantCulture), lines[i + 1]));
        }
        if (tags.Any(t => t.Code == 0 && t.Value == "ACAD_TABLE"))
        {
            throw new InvalidOperationException($"{path} は既に表を含んでいます。");
        }

        var starts = new List<int>();
        for (int i = 0; i < tags.Count; i++)
        {
            if (tags[i].Code == 0)
            {
                starts.Add(i);
            }
        }
        starts.Add(tags.Count);

        var output = new List<(int Code, string Value)>();
        for (int s = 0; s + 1 < starts.Count; s++)
        {
            var rec = tags.GetRange(starts[s], starts[s + 1] - starts[s]);
            string kind = rec[0].Value;
            if (kind == "DBCOLOR" && rec.Contains((5, "48")))
            {
                rec = rec
                    .Select(t => t.Code == 430 ? (t.Code, "SamplePocket$Harbor") : t)
                    .ToList();
            }
            if (kind == "TABLESTYLE" && rec.Contains((5, "4A")))
            {
                int rows = 0;
                var rec2 = new List<(int Code, string Value)>(rec.Count);
                foreach (var (code, val) in rec)
                {
                    string next = val;
                    if (code == 7)
                    {
                        rows++;
                    }
                    if (code == 140)
                    {
                        next = "1.75";
                    }
                    if (code == 170)
                    {
                        next = "5";
                    }
                    if (code == 62 && rows == 1)
                    {
                        next = "2";
                    }
                    rec2.Add((code, next));
                }
                rec = rec2;
            }
            output.AddRange(rec);
            if (kind == "CIRCLE" && rec.Contains((5, "49")))
            {
                output.AddRange(new (int, string)[]
                {
                    (0, "ACAD_TABLE"), (5, "4E"), (330, "40"), (100, "AcDbEntity"), (8, "0"),
                    (100, "AcDbBlockReference"), (2, "*T1"), (10, "0"), (20, "0"), (30, "0"),
                    (100, "AcDbTable"), (280, "0"), (342, "4A"), (343, "4B"),
                    (11, "1"), (21, "0"), (31, "0"),
                    (90, "0"), (91, "1"), (92, "2"), (93, "0"), (94, "0"), (95, "0"), (96, "0"),
                    (141, "8"), (142, "15"), (142, "18"),
                });
                for (int k = 0; k < 2; k++)
                {
                    string text = $"cell{k}";
                    output.AddRange(new (int, string)[]
                    {
                        (171, "1"), (172, "0"), (173, "0"), (174, "0"), (175, "1"), (176, "1"),
                        (91, "0"), (178, "0"), (145, "0"),
                        (140, (2.5 + k).ToString(CultureInfo.InvariantCulture)),
                        (64, (3 + k).ToString(CultureInfo.InvariantCulture)),
                        (301, "CELL_VALUE"), (90, "4"), (93, "0"), (1, text),
                        (94, "0"), (300, ""), (302, text), (304, "ACVALUE_END"),
                    });
                }
            }
        }

        var sb = new StringBuilder();
        foreach (var (code, val) in output)
        {
            sb.AppendFormat(CultureInfo.InvariantCulture, "{0,3}\r\n{1}\r\n", code, val);
        }
        File.WriteAllText(path, sb.ToString(), new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
    }

    static void VerifyStylePair(string path, bool isDwg)
    {
        var drawing = isDwg ? DwgReader.Read(path) : DxfReader.Read(path);
        var circle = drawing.Entities.OfType<Circle>().Single();
        var table = drawing.Entities.OfType<TableEntity>().Single();
        var book = drawing.Colors.Single();
        var style = drawing.TableStyles["SampleFormat"];
        if (circle.Handle != 0x49
            || book.Handle != 0x48
            || circle.BookColor != book
            || book.Name != "SamplePocket$Harbor"
            || (book.Color.R, book.Color.G, book.Color.B) != (13, 97, 184)
            || table.Handle != 0x4E
            || style.Handle != 0x4A
            || table.Style != style
            || table.Rows.Count != 1
            || table.Columns.Count != 2)
        {
            throw new InvalidDataException(path);
        }
        if (isDwg && (style.DataCellStyle.Color.Index != 2 || style.DataCellStyle.TextHeight != 1.75))
        {
            throw new InvalidDataException("DWGのデータ行 " + path);
        }
        for (int k = 0; k < 2; k++)
        {
            var cell = table.Rows[0].Cells[k];
            if (isDwg && (cell.StyleOverride.Color.Index != 3 + k || cell.StyleOverride.TextHeight != 2.5 + k))
            {
                throw new InvalidDataException("DWGのセル " + path);
            }
        }
    }

    static void WriteCleanAndPartialCases(string path)
    {
        var doc = CreateBaseDocument(ACadVersion.AC1032, setMillimeters: true);

        // 対応済みの欄（行の文字スタイル・文字高・ACI 色）だけを持つ TABLESTYLE
        var clean = new TableStyle("CleanFormat");
        ConfigureRow(clean.TitleCellStyle, doc.TextStyles["Standard"], height: 4.0, color: new Color(1));
        ConfigureRow(clean.HeaderCellStyle, doc.TextStyles["Standard"], height: 3.0, color: new Color(6));
        ConfigureRow(clean.DataCellStyle, doc.TextStyles["Standard"], height: 2.25, color: new Color(5));
        clean.TableCellStyle.HasData = false;
        doc.TableStyles.Add(clean);

        // 表 1: 未設定・高さのみ上書き・色のみ上書き（ByBlock）・内容書式で高さと色（ByLayer）を上書き
        var block1 = new BlockRecord("*T1");
        doc.BlockRecords.Add(block1);
        var table1 = new TableEntity(block1)
        {
            Style = clean,
            InsertPoint = new XYZ(10, 20, 0),
        };
        table1.Columns.Add(new TableEntity.Column { Width = 12 });
        table1.Columns.Add(new TableEntity.Column { Width = 16 });
        for (int r = 0; r < 2; r++)
        {
            var row = new TableEntity.Row { Height = 6 };
            for (int c = 0; c < 2; c++)
            {
                var cell = new TableEntity.Cell { Type = TableEntity.CellType.Text };
                var content = new TableEntity.CellContent
                {
                    ContentType = TableEntity.TableCellContentType.Value,
                };
                string text = $"c{r}{c}";
                content.CadValue.SetValue(text, CadValueType.String);
                content.CadValue.FormattedValue = text;
                if (r == 0 && c == 0)
                {
                    cell.StyleOverride.HasData = false;
                }
                else if (r == 0 && c == 1)
                {
                    cell.StyleOverride.HasData = true;
                    cell.StyleOverride.TextHeight = 3.25;
                    cell.StyleOverride.PropertyOverrideFlags = TableStyle.CellStylePropertyFlags.TextHeight;
                }
                else if (r == 1 && c == 0)
                {
                    cell.StyleOverride.HasData = true;
                    cell.StyleOverride.Color = Color.ByBlock;
                    cell.StyleOverride.TextColor = Color.ByBlock;
                    cell.StyleOverride.PropertyOverrideFlags = TableStyle.CellStylePropertyFlags.ContentColor;
                }
                else
                {
                    cell.StyleOverride.HasData = false;
                    content.Format.HasData = true;
                    content.Format.TextHeight = 4.5;
                    content.Format.Color = Color.ByLayer;
                    content.Format.PropertyOverrideFlags =
                        TableStyle.CellStylePropertyFlags.ContentColor | TableStyle.CellStylePropertyFlags.TextHeight;
                }
                cell.Contents.Add(content);
                row.Cells.Add(cell);
            }
            table1.Rows.Add(row);
        }
        doc.Entities.Add(table1);

        // 表 2: スタイルは対応済みだが、セルの真の色（RGB）上書きにより表 2 だけ未対応診断になる
        var block2 = new BlockRecord("*T2");
        doc.BlockRecords.Add(block2);
        var table2 = new TableEntity(block2)
        {
            Style = clean,
            InsertPoint = new XYZ(60, 20, 0),
        };
        table2.Columns.Add(new TableEntity.Column { Width = 20 });
        var row2 = new TableEntity.Row { Height = 8 };
        var cell2 = new TableEntity.Cell { Type = TableEntity.CellType.Text };
        var content2 = new TableEntity.CellContent
        {
            ContentType = TableEntity.TableCellContentType.Value,
        };
        content2.CadValue.SetValue("rgb", CadValueType.String);
        content2.CadValue.FormattedValue = "rgb";
        cell2.StyleOverride.HasData = true;
        cell2.StyleOverride.Color = new Color(10, 20, 30);
        cell2.StyleOverride.TextColor = new Color(10, 20, 30);
        cell2.StyleOverride.PropertyOverrideFlags = TableStyle.CellStylePropertyFlags.ContentColor;
        cell2.StyleOverride.TableCellStylePropertyFlags = TableStyle.CellStylePropertyFlags.ContentColor;
        cell2.Contents.Add(content2);
        row2.Cells.Add(cell2);
        table2.Rows.Add(row2);
        doc.Entities.Add(table2);

        DwgWriter.Write(path, doc);
    }

    static void ConfigureRow(TableStyle.CellStyle row, TextStyle textStyle, double height, Color color)
    {
        row.HasData = true;
        row.Alignment = 0;
        row.TextStyle = textStyle;
        row.TextHeight = height;
        row.Color = color;
        row.TextColor = color;
        row.PropertyOverrideFlags =
            TableStyle.CellStylePropertyFlags.ContentColor | TableStyle.CellStylePropertyFlags.TextHeight;
    }
}
