param([Parameter(Mandatory)][int]$GpuProcessId)
$ErrorActionPreference = 'Stop'

# GPU プロセスの物理アダプターと同じ LUID・物理番号の全体カウンターを、同じ時刻で読む。
$processInstance = "pid_${GpuProcessId}_*"
$prefix = "pid_${GpuProcessId}_"
$paths = foreach ($field in 'Dedicated Usage', 'Shared Usage') {
    "\GPU Process Memory($processInstance)\$field"
    "\GPU Adapter Memory(*)\$field"
}
Get-Counter -Counter $paths -SampleInterval 1 -Continuous | ForEach-Object {
    $instances = @{}
    foreach ($sample in $_.CounterSamples) {
        if ($sample.Status -gt 1) { throw "カウンターの読み取りに失敗しました $($sample.Status)" }
        $name = $sample.InstanceName
        if (!$instances.ContainsKey($name)) { $instances[$name] = @{} }
        $field = $sample.Path.Split('\')[-1]
        $instances[$name][$field] = $sample.RawValue
    }
    $rows = foreach ($name in $instances.Keys) {
        if (!$name.StartsWith($prefix)) { continue }
        $luid = $name.Substring($prefix.Length)
        if (!$instances.ContainsKey($luid)) { throw "同じアダプターのカウンターがありません" }
        $proc = $instances[$name]
        $adapter = $instances[$luid]
        [ordered]@{
            adapter = $luid
            processDedicated = $proc['dedicated usage']
            processShared = $proc['shared usage']
            adapterDedicated = $adapter['dedicated usage']
            adapterShared = $adapter['shared usage']
        }
    }
    if (!$rows) { throw "GPU プロセスのカウンターがありません" }
    [Console]::WriteLine((ConvertTo-Json -Compress -Depth 3 -InputObject @{
        time = ([DateTimeOffset]$_.Timestamp).ToUnixTimeMilliseconds()
        rows = @($rows)
    }))
}
