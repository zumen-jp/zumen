// 自作の 2 次有理スプラインを DWG から直接読む。weighted_spline.py が用意した原本と出力を使う。
#:property PublishAot=false
#:package ACadSharp@3.8.0
using ACadSharp.Entities;
using ACadSharp.IO;
using CSMath;

foreach (var version in new[] { "2013", "2018" })
{
    ulong? original = null;
    foreach (var name in new[] { "base", "move", "add" })
    {
        using var reader = new DwgReader(Path.Combine(args.Single(), version, name + ".dwg"));
        var curves = reader.Read().Entities.OfType<Spline>().OrderBy(s => s.Handle).ToArray();
        if (curves.Length != (name == "add" ? 2 : 1))
            throw new InvalidDataException("曲線の数が合いません。");
        original ??= curves[0].Handle;
        if (curves[0].Handle != original)
            throw new InvalidDataException("元の曲線のハンドルが変わりました。");
        foreach (var s in curves)
        {
            var offset = name == "move" ? new XYZ(10, 20, 0)
                : s.Handle == original ? XYZ.Zero : new XYZ(30, 40, 0);
            var points = new[] { new XYZ(12, 0, 0) + offset, new XYZ(12, 12, 0) + offset,
                new XYZ(0, 12, 0) + offset };
            if (s.Degree != 2 || ((int)s.Flags & 7) != 4 || s.FitPoints.Count != 0 ||
                !s.Knots.SequenceEqual(new double[] { 0, 0, 0, 1, 1, 1 }) ||
                !s.Weights.SequenceEqual(new[] { 1.0, Math.Sqrt(0.5), 1.0 }) ||
                !s.ControlPoints.SequenceEqual(points))
                throw new InvalidDataException($"{version}/{name}: 制御点・重み・ノットかフラグが合いません。");
        }
    }
}
Console.WriteLine("ACadSharp 3.8.0: 6 DWG の 8 曲線が一致");
