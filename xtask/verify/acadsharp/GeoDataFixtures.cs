// 自作の GEODATA（EPSG:6677・mm・北を反時計回りに 30 度回転）を持つ DWG を ACadSharp（MIT）で作る（#685）。
// dotnet run --project xtask/verify/acadsharp -c Release -- --make-geodata-fixtures crates/zumen-acad/tests/fixtures
using System.Text;
using ACadSharp;
using ACadSharp.IO;
using ACadSharp.Objects;
using ACadSharp.Types.Units;
using CSMath;

static class GeoDataFixtures
{
    const string DefinitionXml = """
        <?xml version="1.0" encoding="UTF-16" standalone="no" ?>
        <Dictionary version="1.0" xmlns="http://www.osgeo.org/mapguide/coordinatesystem">
        <ProjectedCoordinateSystem id="ZUMEN-TEST-IX">
        <Name>ZUMEN-TEST-IX</Name>
        <Axis uom="Meter">
        <CoordinateSystemAxis><AxisOrder>1</AxisOrder><AxisName>Easting</AxisName><AxisAbbreviation>E</AxisAbbreviation><AxisDirection>East</AxisDirection></CoordinateSystemAxis>
        <CoordinateSystemAxis><AxisOrder>2</AxisOrder><AxisName>Northing</AxisName><AxisAbbreviation>N</AxisAbbreviation><AxisDirection>North</AxisDirection></CoordinateSystemAxis>
        </Axis>
        </ProjectedCoordinateSystem>
        <Alias id="6677" type="CoordinateSystem">
        <ObjectId>ZUMEN-TEST-IX</ObjectId>
        <Namespace>EPSG Code</Namespace>
        </Alias>
        </Dictionary>
        """;

    const string MinimalDxf = """
        0
        SECTION
        2
        HEADER
        9
        $ACADVER
        1
        AC1032
        9
        $INSUNITS
        70
        4
        9
        $HANDSEED
        5
        200
        0
        ENDSEC
        0
        SECTION
        2
        CLASSES
        0
        CLASS
        1
        GEODATA
        2
        AcDbGeoData
        3
        ObjectDBX Classes
        90
        4095
        91
        1
        280
        0
        281
        0
        0
        ENDSEC
        0
        SECTION
        2
        TABLES
        0
        TABLE
        2
        LTYPE
        5
        5
        330
        0
        100
        AcDbSymbolTable
        70
        1
        0
        LTYPE
        5
        14
        330
        5
        100
        AcDbSymbolTableRecord
        100
        AcDbLinetypeTableRecord
        2
        Continuous
        70
        0
        3
        Solid line
        72
        65
        73
        0
        40
        0.0
        0
        ENDTAB
        0
        TABLE
        2
        LAYER
        5
        2
        330
        0
        100
        AcDbSymbolTable
        70
        1
        0
        LAYER
        5
        10
        330
        2
        100
        AcDbSymbolTableRecord
        100
        AcDbLayerTableRecord
        2
        0
        70
        0
        62
        7
        6
        Continuous
        0
        ENDTAB
        0
        TABLE
        2
        STYLE
        5
        3
        330
        0
        100
        AcDbSymbolTable
        70
        1
        0
        STYLE
        5
        11
        330
        3
        100
        AcDbSymbolTableRecord
        100
        AcDbTextStyleTableRecord
        2
        Standard
        70
        0
        40
        0.0
        41
        1.0
        50
        0.0
        71
        0
        42
        2.5
        3
        txt
        4

        0
        ENDTAB
        0
        TABLE
        2
        BLOCK_RECORD
        5
        1
        330
        0
        100
        AcDbSymbolTable
        70
        2
        0
        BLOCK_RECORD
        5
        1F
        102
        {ACAD_XDICTIONARY
        360
        90
        102
        }
        330
        1
        100
        AcDbSymbolTableRecord
        100
        AcDbBlockTableRecord
        2
        *Model_Space
        0
        BLOCK_RECORD
        5
        1E
        330
        1
        100
        AcDbSymbolTableRecord
        100
        AcDbBlockTableRecord
        2
        *Paper_Space
        0
        ENDTAB
        0
        ENDSEC
        0
        SECTION
        2
        BLOCKS
        0
        BLOCK
        5
        20
        330
        1F
        100
        AcDbEntity
        8
        0
        100
        AcDbBlockBegin
        2
        *Model_Space
        70
        0
        10
        0.0
        20
        0.0
        30
        0.0
        3
        *Model_Space
        1

        0
        ENDBLK
        5
        21
        330
        1F
        100
        AcDbEntity
        8
        0
        100
        AcDbBlockEnd
        0
        BLOCK
        5
        1C
        330
        1E
        100
        AcDbEntity
        8
        0
        100
        AcDbBlockBegin
        2
        *Paper_Space
        70
        0
        10
        0.0
        20
        0.0
        30
        0.0
        3
        *Paper_Space
        1

        0
        ENDBLK
        5
        1D
        330
        1E
        100
        AcDbEntity
        8
        0
        100
        AcDbBlockEnd
        0
        ENDSEC
        0
        SECTION
        2
        ENTITIES
        0
        LINE
        5
        80
        330
        1F
        100
        AcDbEntity
        8
        0
        100
        AcDbLine
        10
        10000.0
        20
        20000.0
        30
        0.0
        11
        20000.0
        21
        20000.0
        31
        0.0
        0
        ENDSEC
        0
        SECTION
        2
        OBJECTS
        0
        DICTIONARY
        5
        C
        330
        0
        100
        AcDbDictionary
        281
        1
        0
        DICTIONARY
        5
        90
        330
        1F
        100
        AcDbDictionary
        280
        1
        281
        1
        3
        ACAD_GEOGRAPHICDATA
        360
        91
        0
        GEODATA
        5
        91
        330
        90
        100
        AcDbGeoData
        90
        3
        330
        1F
        70
        1
        10
        10000.0
        20
        20000.0
        30
        0.0
        11
        23456.789
        21
        -12345.678
        31
        12.34
        40
        0.001
        91
        4
        41
        0.001
        92
        4
        210
        0.0
        220
        0.0
        230
        1.0
        12
        -0.5
        22
        0.8660254037844386
        95
        1
        141
        1.0
        294
        0
        142
        0.0
        143
        6378137.0
        301
        placeholder
        93
        0
        96
        0
        0
        ENDSEC
        0
        EOF
        """;

    static CadDocument BuildDocument(ACadVersion cadVersion, GeoDataVersion geoVersion)
    {
        using var inStream = new MemoryStream(Encoding.UTF8.GetBytes(MinimalDxf.Replace("\r\n", "\n") + "\n"));
        var doc = DxfReader.Read(inStream);
        doc.CreateDefaults();
        doc.Header.Version = cadVersion;
        doc.Header.InsUnits = UnitsType.Millimeters;
        doc.Header.LastSavedBy = "";
        doc.SummaryInfo.Author = "";
        doc.SummaryInfo.LastSavedBy = "";
        var date = new DateTime(2026, 1, 1, 0, 0, 0, DateTimeKind.Utc);
        doc.Header.CreateDateTime = date;
        doc.Header.UpdateDateTime = date;
        doc.Header.UniversalCreateDateTime = date;
        doc.Header.UniversalUpdateDateTime = date;
        doc.SummaryInfo.CreatedDate = date;
        doc.SummaryInfo.ModifiedDate = date;

        if (doc.TableStyles != null)
        {
            foreach (var tableStyle in doc.TableStyles)
            {
                foreach (var style in new[]
                {
                    tableStyle.TableCellStyle,
                    tableStyle.DataCellStyle,
                    tableStyle.HeaderCellStyle,
                    tableStyle.TitleCellStyle,
                })
                {
                    style.TextStyle = doc.TextStyles["Standard"];
                }
            }
        }

        var geo = doc.ModelSpace.XDictionary?.OfType<GeoData>().SingleOrDefault()
            ?? throw new InvalidOperationException("DxfReader did not attach GeoData to ModelSpace.XDictionary");
        geo.Version = geoVersion;
        geo.HostBlock = doc.ModelSpace;
        geo.CoordinatesType = DesignCoordinatesType.LocalGrid;
        geo.DesignPoint = new XYZ(1200, 3400, 0);
        geo.ReferencePoint = new XYZ(-12345.678, 23456.789, 12.5);
        geo.HorizontalUnitScale = 0.001;
        geo.HorizontalUnits = UnitsType.Millimeters;
        geo.VerticalUnitScale = 0.001;
        geo.VerticalUnits = UnitsType.Millimeters;
        geo.UpDirection = new XYZ(0, 0, 1);
        geo.NorthDirection = new XY(0.5, 0.8660254037844386);
        geo.ScaleEstimationMethod = ScaleEstimationType.None;
        geo.UserSpecifiedScaleFactor = 1.0;
        geo.EnableSeaLevelCorrection = false;
        geo.SeaLevelElevation = 0.0;
        geo.CoordinateProjectionRadius = 0.0;
        geo.CoordinateSystemDefinition = DefinitionXml.Replace("\r\n", "\n") + "\n";
        return doc;
    }

    public static void Write(string folder)
    {
        Directory.CreateDirectory(folder);
        var specs = new[]
        {
            (ACadVersion.AC1018, GeoDataVersion.R2009, "geodata-r2004-v1.dwg"),
            (ACadVersion.AC1024, GeoDataVersion.R2010, "geodata-r2010.dwg"),
            (ACadVersion.AC1027, GeoDataVersion.R2013, "geodata-r2013.dwg"),
            (ACadVersion.AC1032, GeoDataVersion.R2013, "geodata-r2018.dwg"),
        };
        foreach (var (cadVersion, geoVersion, name) in specs)
        {
            var document = BuildDocument(cadVersion, geoVersion);
            var path = Path.Combine(folder, name);
            DwgWriter.Write(path, document);

            var notes = new List<string>();
            using var reader = new DwgReader(path, (_, e) => notes.Add($"{e.NotificationType}:{e.Message}"));
            var reread = reader.Read();
            if (notes.Count != 0)
            {
                throw new InvalidOperationException($"{name}: unexpected notifications: {string.Join("; ", notes)}");
            }
            var rereadGeo = reread.ModelSpace.XDictionary?.OfType<GeoData>().SingleOrDefault()
                ?? throw new InvalidOperationException($"{name}: missing GeoData on reread");
            if (rereadGeo.Version != geoVersion || rereadGeo.CoordinateSystemDefinition.Length < 400)
            {
                throw new InvalidOperationException($"{name}: mismatch on reread");
            }
        }
    }
}
