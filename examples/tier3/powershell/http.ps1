# zumen を、PowerShell から CLI の `serve --http`（HTTP の JSON-RPC）で呼ぶサンプル（#404、決定 0006・0200）。
#
# 使い方: pwsh examples/tier3/powershell/http.ps1 <zumen の実行ファイル> <図面のファイル>
#
# サーバーを 127.0.0.1 の空いたポートで起こし、図面を開き、形式と図形の数を書く。入力は本文のままで送る。

param(
    [Parameter(Mandatory = $true)][string]$Zumen,
    [Parameter(Mandatory = $true)][string]$Drawing
)
$ErrorActionPreference = 'Stop'

$start = [System.Diagnostics.ProcessStartInfo]::new($Zumen)
foreach ($arg in 'serve', '--http', '--listen', '127.0.0.1:0') {
    $start.ArgumentList.Add($arg)
}
$start.RedirectStandardOutput = $true
$start.UseShellExecute = $false
$server = [System.Diagnostics.Process]::Start($start)
try {
    # 1 行目に、待ち受けたアドレスを書く
    $base = $server.StandardOutput.ReadLine().Trim().TrimEnd('/')

    function Invoke-Rpc([string]$Method, [hashtable]$Params) {
        $body = @{ jsonrpc = '2.0'; id = 1; method = $Method; params = $Params } | ConvertTo-Json -Depth 10 -Compress
        $reply = Invoke-RestMethod -Method Post -Uri "$base/" -ContentType 'application/json' -Body $body
        if ($null -ne $reply.error) {
            throw "$Method が誤りを返した: $($reply.error | ConvertTo-Json -Compress)"
        }
        $reply.result
    }

    function Invoke-Zumen([string]$Session, [hashtable]$Request) {
        $result = Invoke-Rpc 'call' @{ session = $Session; request = $Request }
        if ($result.status -ne 'ok') {
            throw "$($Request.op) が $($result.status) を返した: $($result.response)"
        }
        $result.response | ConvertFrom-Json
    }

    $session = (Invoke-Rpc 'session_new' @{}).session
    $name = [System.IO.Path]::GetFileName($Drawing)
    $pushed = Invoke-RestMethod -Method Post -Uri "$base/sessions/$session/inputs/$name" -ContentType 'application/octet-stream' -InFile $Drawing
    if ($pushed.status -ne 'ok') {
        throw "push が $($pushed.status) を返した"
    }
    $opened = Invoke-Zumen $session @{ v = 1; op = 'open'; input = $name }
    $entities = Invoke-Zumen $session @{ v = 1; op = 'entities'; doc = $name }
    Write-Output "format $($opened.result.format)"
    Write-Output "entities $($entities.result.total)"
    Invoke-Rpc 'session_close' @{ session = $session } | Out-Null
}
finally {
    $server.Kill()
}
