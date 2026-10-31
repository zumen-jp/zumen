# accoreconsole に台本を流し、時間内に終わらなければ止める。ログは台本と同じ名前の .out（UTF-16）
param([string]$Scr, [int]$Timeout = 120)
$out = [IO.Path]::ChangeExtension($Scr, ".out")
$p = Start-Process "C:\Program Files\Autodesk\AutoCAD 2027\accoreconsole.exe" -ArgumentList "/s `"$Scr`"" -NoNewWindow -PassThru -RedirectStandardOutput $out
if (-not $p.WaitForExit($Timeout * 1000)) { Stop-Process -Id $p.Id -Force; "TIMEOUT" }
