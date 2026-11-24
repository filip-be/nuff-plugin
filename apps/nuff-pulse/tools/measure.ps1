<#
Measures the widget against its budget: private memory, CPU while idle and
exe size. Starts the release build (stopping a running widget first), waits
for it to settle, then samples it.

  pwsh tools/measure.ps1                 # bar only
  pwsh tools/measure.ps1 -App            # with the app window open
  pwsh tools/measure.ps1 -Seconds 120    # longer CPU sample
  pwsh tools/measure.ps1 -Idle           # no sessions: empty home and settings
                                         # folders (no extra roots); running
                                         # WSL distros are still read: stop
                                         # their agents for a no-session figure
#>
param(
    [string]$Exe = "$PSScriptRoot\..\target\release\nuff-pulse.exe",
    [int]$Settle = 15,
    [int]$Seconds = 60,
    [switch]$App,
    [switch]$Idle,
    [switch]$Keep
)
$ErrorActionPreference = 'Stop'
$Exe = (Resolve-Path $Exe).Path
Get-Process nuff-pulse -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500
$saved = @{ USERPROFILE = $env:USERPROFILE; APPDATA = $env:APPDATA; CLAUDE_CONFIG_DIR = $env:CLAUDE_CONFIG_DIR; COPILOT_HOME = $env:COPILOT_HOME }
if ($Idle) {
    $empty = Join-Path ([IO.Path]::GetTempPath()) 'nuff-pulse-measure-home'
    New-Item -ItemType Directory -Force $empty | Out-Null
    $env:USERPROFILE = $empty; $env:APPDATA = $empty; $env:CLAUDE_CONFIG_DIR = ''; $env:COPILOT_HOME = ''
}
try {
    $p = Start-Process $Exe -ArgumentList @($(if ($App) { '--app' })) -PassThru
} finally {
    foreach ($k in $saved.Keys) {
        if ($saved[$k]) { Set-Item "env:$k" $saved[$k] } else { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
    }
}
Start-Sleep $Settle
$p.Refresh()
$cpu0 = $p.TotalProcessorTime
$t0 = Get-Date
$peak = 0
for ($i = 0; $i -lt $Seconds; $i++) {
    Start-Sleep 1
    $p.Refresh()
    $peak = [math]::Max($peak, $p.PrivateMemorySize64)
}
$p.Refresh()
$cpu = ($p.TotalProcessorTime - $cpu0).TotalSeconds / ((Get-Date) - $t0).TotalSeconds * 100
[pscustomobject]@{
    PrivateMB     = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
    PeakPrivateMB = [math]::Round($peak / 1MB, 1)
    WorkingSetMB  = [math]::Round($p.WorkingSet64 / 1MB, 1)
    CpuPctOfCore  = [math]::Round($cpu, 2)
    Threads       = $p.Threads.Count
    ExeMB         = [math]::Round((Get-Item $Exe).Length / 1MB, 2)
}
if (-not $Keep) { Stop-Process $p.Id -Force }
