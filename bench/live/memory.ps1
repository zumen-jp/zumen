# core-live を新しいプロセスで開き、常駐文書だけを持つ時点のメモリを測る。
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$InputPath,
    [ValidateRange(1, 256)][int]$Threads = 1,
    [ValidateRange(1, 65536)][int]$DecodedMiB = 16384,
    [ValidateRange(512, 65536)][int]$MaxMemoryMiB = 8192,
    [ValidateRange(1, 3600)][int]$TimeoutSeconds = 180
)
$ErrorActionPreference = 'Stop'
$info = [System.Diagnostics.ProcessStartInfo]::new()
$info.FileName = (Resolve-Path -LiteralPath $Executable).Path
$info.UseShellExecute = $false
$info.CreateNoWindow = $true
$info.RedirectStandardInput = $true
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
foreach ($argument in @('open', (Resolve-Path -LiteralPath $InputPath).Path, '--threads', "$Threads", '--decoded-mib', "$DecodedMiB")) {
    $info.ArgumentList.Add($argument)
}
$process = [System.Diagnostics.Process]::new()
$process.StartInfo = $info
$started = $false
try {
    if (!$process.Start()) { throw '測定するプロセスを起動できない' }
    $started = $true
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    $errors = $process.StandardError.ReadToEndAsync()
    $pending = $process.StandardOutput.ReadLineAsync()
    $ready = $false
    while (!$ready -and !$process.HasExited) {
        $process.Refresh()
        if ($process.WorkingSet64 -gt ([long]$MaxMemoryMiB * 1MB)) {
            throw '測定用のメモリ上限で停止した。入力と詳細は公開せずに扱う'
        }
        if ($timer.Elapsed.TotalSeconds -gt $TimeoutSeconds) {
            throw '測定用の時間上限で停止した。入力と詳細は公開せずに扱う'
        }
        if ($pending.IsCompleted) {
            $line = $pending.GetAwaiter().GetResult()
            if ($null -eq $line) { break }
            $ready = $line.StartsWith("ready`t")
            if (!$ready) { $pending = $process.StandardOutput.ReadLineAsync() }
        } else {
            Start-Sleep -Milliseconds 40
        }
    }
    if (!$ready) {
        if (!$process.WaitForExit(10000)) { throw '出力が終わったプロセスが終了しない' }
        throw "文書を開けない: $($errors.GetAwaiter().GetResult())"
    }
    $process.Refresh()
    $name = [System.IO.Path]::GetFileName($InputPath)
    Write-Output "input`tthreads`tworking_set_bytes`tprivate_bytes`tpeak_working_set_bytes`tprocess_open_ms"
    Write-Output "$name`t$Threads`t$($process.WorkingSet64)`t$($process.PrivateMemorySize64)`t$($process.PeakWorkingSet64)`t$($timer.Elapsed.TotalMilliseconds.ToString('F3', [System.Globalization.CultureInfo]::InvariantCulture))"
    $process.StandardInput.WriteLine()
    $process.StandardInput.Flush()
    if (!$process.WaitForExit(10000)) { throw '測定後のプロセスが終了しない' }
    if ($process.ExitCode -ne 0) { throw "測定後の終了コード: $($process.ExitCode)" }
} finally {
    if ($started -and !$process.HasExited) { $process.Kill($true); $process.WaitForExit() }
    $process.Dispose()
}
