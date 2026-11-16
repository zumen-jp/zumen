// BcfToolkit 3.2.0 の公開 API だけを呼び、自作入力の値を記録する。
// https://www.nuget.org/packages/Smino.Bcf.Toolkit/3.2.0
using BcfToolkit;
using BcfToolkit.Converter;
using BcfToolkit.Model;
using BcfToolkit.Model.Interfaces;
using Newtonsoft.Json;
using System.Text.RegularExpressions;

var stage = "arguments";
try
{
    if (args.Length == 1 && args[0] == "metadata")
    {
        var framework = System.Runtime.InteropServices.RuntimeEnvironment.GetRuntimeDirectory();
        Console.WriteLine(JsonConvert.SerializeObject(new
        {
            runtime = Environment.Version.ToString(),
            toolkit = typeof(Worker).Assembly.GetName().ToString(),
            references = typeof(Worker).Assembly.GetReferencedAssemblies().Select(item => item.ToString()),
            regex = typeof(Regex).Assembly.GetName().ToString(),
            regexIsFramework = typeof(Regex).Assembly.Location.StartsWith(framework, StringComparison.Ordinal),
        }));
        return 0;
    }
    if (args.Length is not (3 or 4) || args[0] is not ("2.1" or "3.0"))
        throw new ArgumentException();
    if (File.Exists(args[2]) || (args.Length == 4 && File.Exists(args[3])))
        throw new IOException();
    stage = "read";
    await using var input = File.OpenRead(args[1]);
    IConverter converter;
    IBcf bcf;
    BcfVersionEnum version;
    if (args[0] == "2.1")
    {
        converter = new BcfToolkit.Converter.Bcf21.Converter();
        bcf = await converter.BcfFromStream<BcfToolkit.Model.Bcf21.Bcf>(input);
        version = BcfVersionEnum.Bcf21;
    }
    else
    {
        converter = new BcfToolkit.Converter.Bcf30.Converter();
        bcf = await converter.BcfFromStream<BcfToolkit.Model.Bcf30.Bcf>(input);
        version = BcfVersionEnum.Bcf30;
    }
    stage = "observe";
    await File.WriteAllTextAsync(args[2], JsonConvert.SerializeObject(bcf, Formatting.Indented));
    if (args.Length == 4)
    {
        stage = "write";
        // Stream を返す経路は Windows で一時ファイルを再び開けなかったため、渡した Stream に書く。
        using var data = new MemoryStream();
        converter.ToBcf(bcf, version, data);
        await File.WriteAllBytesAsync(args[3], data.ToArray());
    }
    Console.WriteLine("{\"complete\":true}");
    return 0;
}
catch (Exception error)
{
    // 外部実装の例外の本文とスタックには、端末の道筋が入り得るため記録しない。
    Console.WriteLine(JsonConvert.SerializeObject(new { complete = false, stage, errorType = error.GetType().FullName }));
    return 2;
}
