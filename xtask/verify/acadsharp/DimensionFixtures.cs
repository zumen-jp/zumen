// 自作の個別 DSTYLE 上書き・拡張辞書 XRECORD・匿名ブロックを持つ平行寸法を ACadSharp（MIT）で DWG にする（#905）。
// dotnet run --project xtask/verify/acadsharp -c Release -- --make-dim-dstyle-fixtures crates/zumen-acad/tests/fixtures
using ACadSharp;
using ACadSharp.Entities;
using ACadSharp.IO;
using ACadSharp.Objects;
using ACadSharp.Tables;
using ACadSharp.XData;
using CSMath;

static class DimensionFixtures
{
    public static void Write(string folder)
    {
        Directory.CreateDirectory(folder);
        foreach (var (version, name) in new[]
        {
            (ACadVersion.AC1024, "dim-dstyle-r2010.dwg"),
            (ACadVersion.AC1027, "dim-dstyle-r2013.dwg"),
            (ACadVersion.AC1032, "dim-dstyle-r2018.dwg"),
        })
        {
            var document = new CadDocument(version);
            document.Header.LastSavedBy = "";
            document.SummaryInfo.Author = "";
            document.SummaryInfo.LastSavedBy = "";
            var date = new DateTime(2026, 1, 1, 0, 0, 0, DateTimeKind.Utc);
            document.Header.CreateDateTime = date;
            document.Header.UpdateDateTime = date;
            document.Header.UniversalCreateDateTime = date;
            document.Header.UniversalUpdateDateTime = date;
            document.SummaryInfo.CreatedDate = date;
            document.SummaryInfo.ModifiedDate = date;

            var style = new DimensionStyle("CaseBase")
            {
                TextHeight = 2.5,
                ArrowSize = 1.25,
                DecimalPlaces = 2,
            };
            document.DimensionStyles.Add(style);

            var dimension = new DimensionAligned(new XYZ(3, 7, 0), new XYZ(33, 47, 0))
            {
                Style = style,
                Offset = 7,
                AttachmentPoint = AttachmentPointType.MiddleCenter,
                LineSpacingStyle = LineSpacingStyleType.AtLeast,
                LineSpacingFactor = 1,
            };
            document.Entities.Add(dimension);

            var overrides = new ExtendedDataRecord[]
            {
                new ExtendedDataString("DSTYLE"),
                ExtendedDataControlString.Open,
                new ExtendedDataInteger16(41),
                new ExtendedDataReal(3.625),
                new ExtendedDataInteger16(140),
                new ExtendedDataReal(4.75),
                new ExtendedDataInteger16(271),
                new ExtendedDataInteger16(3),
                ExtendedDataControlString.Close,
            };
            dimension.ExtendedData.Add("ACAD", new ExtendedData(overrides));

            var record = new XRecord("VERIFY_VALUES");
            record.CreateEntry(1, "A-19");
            record.CreateEntry(70, (short)321);
            record.CreateEntry(40, 6.125);
            dimension.CreateExtendedDictionary().Add("VERIFY_VALUES", record);

            dimension.UpdateBlock();
            var text = dimension.Block.Entities.OfType<MText>().Single();
            text.LineSpacingStyle = LineSpacingStyleType.AtLeast;
            text.LineSpacing = 1;

            DwgWriter.Write(Path.Combine(folder, name), document);
        }
    }
}
