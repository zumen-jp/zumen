param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string[]]$Inputs,
    [ValidateRange(3, 99)][int]$Repeats = 3
)

# Windows のプロセスの最大ワーキングセットと、各要求中に観測したワーキングセットを分けて測る。
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'この測定は Windows のプロセス統計を使います' }
$exe = (Resolve-Path -LiteralPath $Executable).Path
$baseline = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'webgpu-results-2026-11-21.json') | ConvertFrom-Json -AsHashtable
$sheets = @{}
foreach ($key in @('city-min', 'city-vertices', 'city-max')) {
    $record = $baseline.runs[$key]
    $sheets[$record.node_worker[0].input_sha256] = $key
}

function Invoke-MeasuredRpc($Child, $Id, $Method, $Params) {
    $request = @{ jsonrpc = '2.0'; id = $Id; method = $Method; params = $Params } | ConvertTo-Json -Compress -Depth 6
    $Child.Refresh()
    $sampled = $Child.WorkingSet64
    $Child.StandardInput.WriteLine($request)
    $Child.StandardInput.Flush()
    $pending = $Child.StandardOutput.ReadLineAsync()
    do {
        $complete = $pending.Wait(5)
        $Child.Refresh()
        $sampled = [Math]::Max($sampled, $Child.WorkingSet64)
    } until ($complete)
    $highWater = $Child.PeakWorkingSet64
    $reply = $pending.GetAwaiter().GetResult() | ConvertFrom-Json -AsHashtable
    if ($reply.id -ne $Id -or $reply.result.status -ne 'ok') { throw "要求 $Method が成功しませんでした" }
    return @{ result = $reply.result; sampled = $sampled; peak = $highWater }
}

function Measure-Scene($File, $ExplicitTolerance) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $exe
    $start.Arguments = 'serve --stdio'
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $child = [Diagnostics.Process]::Start($start)
    try {
        $push = Invoke-MeasuredRpc $child 1 'push' @{ name = 'input'; path = $File }
        $open = Invoke-MeasuredRpc $child 2 'call' @{ request = @{ v = 1; op = 'open'; input = 'input' } }
        $drawing = Invoke-MeasuredRpc $child 3 'call' @{ request = @{ v = 1; op = 'drawing'; doc = 'input' } }
        $request = @{ v = 1; op = 'scene'; doc = 'input'; from = 'space/0' }
        if ($ExplicitTolerance) { $request.tolerance = 0.01 }
        $scene = Invoke-MeasuredRpc $child 4 'call' @{ request = $request }
        $response = $scene.result.response | ConvertFrom-Json -AsHashtable
        $result = $response.result
        # 取得・Base64 化のメモリを scene の生成に入れない。
        $take = Invoke-MeasuredRpc $child 5 'take' @{ blob = $result.blob }
        $bytes = [Convert]::FromBase64String($take.result.base64)
        if ($bytes.Length -ne $response.blobs[0].size) { throw 'シーンの長さが応答と一致しません' }
        return @{
            scene = [ordered]@{
                tolerance = $result.tolerance
                chunks = $result.counts.chunks
                vertices = $result.counts.vertices
                bytes = $bytes.Length
                sha256 = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()
                diagnostics = @($response.diagnostics | ForEach-Object { [ordered]@{code=$_.code; count=$_.count} })
            }
            memory = [ordered]@{
                push_peak_bytes = $push.peak
                open_peak_bytes = $open.peak
                drawing_peak_bytes = $drawing.peak
                scene_peak_bytes = $scene.peak
                open_sampled_bytes = $open.sampled
                drawing_sampled_bytes = $drawing.sampled
                scene_sampled_bytes = $scene.sampled
            }
        }
    } finally {
        $child.StandardInput.Close()
        if (-not $child.WaitForExit(10000)) { $child.Kill(); $child.WaitForExit() }
        $child.Dispose()
    }
}

$rows = foreach ($file in $Inputs) {
    $path = (Resolve-Path -LiteralPath $file).Path
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $sheets.ContainsKey($hash)) { throw '指定した公開3図郭の SHA-256 と一致しません' }
    $key = $sheets[$hash]
    foreach ($explicit in @($false, $true)) {
        $runs = @(for ($i = 0; $i -lt $Repeats; $i++) { Measure-Scene $path $explicit })
        $expected = if ($explicit) {
            ($baseline.runs[$key].browser_results | Where-Object policy -eq 'provisional').scene_sha256
        } else {
            $baseline.runs[$key].node_worker[0].scene_sha256
        }
        foreach ($run in $runs) {
            if ($run.scene.sha256 -ne $expected) { throw '参照するシーンの SHA-256 と一致しません' }
        }
        [ordered]@{
            case = $key
            input_bytes = (Get-Item -LiteralPath $path).Length
            input_sha256 = $hash
            requested_tolerance = if ($explicit) { 0.01 } else { $null }
            scene = $runs[0].scene
            runs = @($runs | ForEach-Object { $_.memory })
        }
    }
}
ConvertTo-Json -InputObject @($rows) -Depth 8
