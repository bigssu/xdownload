# Drives the real window through Windows UI Automation (what a screen reader uses) and checks
# that a click on the Download button really downloads a file.
#
# Uses its own APPDATA so the user's settings and history are untouched; the already cached
# yt-dlp / ffmpeg are copied in to skip the 200 MB first-run download.
# Usage:  pwsh packaging\ui_smoke.ps1 [-Exe dist\xdownload2.exe]
param([string] $Exe = 'dist\xdownload2.exe')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

$sandbox = Join-Path $env:TEMP "xdownload2_ui_smoke_$PID"
$data = Join-Path $sandbox 'Xdownload2'
$out = Join-Path $sandbox 'out'
New-Item -ItemType Directory -Force $data, $out | Out-Null
$cached = Join-Path $env:APPDATA 'Xdownload2\bin'
if (Test-Path $cached) { Copy-Item $cached $data -Recurse }
@{ download_dir = $out; format = 1; auto_paste = $false; last_update_day = (Get-Date -Format 'yyyy-MM-dd') } |
    ConvertTo-Json | Set-Content (Join-Path $data 'settings.json') -Encoding utf8NoBOM

$real = $env:APPDATA
$env:APPDATA = $sandbox
$process = Start-Process (Resolve-Path $Exe) -ArgumentList 'https://www.youtube.com/watch?v=jNQXAC9IVRw' -PassThru
$env:APPDATA = $real

# The accessibility tree appears a moment after the window, so the root is fetched anew for
# every search instead of being kept.
function Get-Window {
    $process.Refresh()
    [System.Windows.Automation.AutomationElement]::FromHandle($process.MainWindowHandle)
}

function Find-Element([string] $Name, [System.Windows.Automation.ControlType] $Type = $null, [int] $Seconds = 20) {
    $condition = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $Name)
    $deadline = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $deadline) {
        $found = (Get-Window).FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition) |
            Where-Object { -not $Type -or $_.Current.ControlType -eq $Type } | Select-Object -First 1
        if ($found) { return $found }
        Start-Sleep -Milliseconds 300
    }
    $names = (Get-Window).FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
        ForEach-Object { "$($_.Current.ControlType.ProgrammaticName -replace 'ControlType.', ''):$($_.Current.Name)" }
    throw "no element named '$Name' (type $Type). Present: $($names -join ' | ')"
}

# Buttons are invoked; tabs and other radio choices are selected.
function Invoke-Element([System.Windows.Automation.AutomationElement] $Element) {
    $pattern = $null
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref] $pattern)) { $pattern.Invoke(); return }
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref] $pattern)) { $pattern.Select(); return }
    throw "element '$($Element.Current.Name)' can be neither invoked nor selected"
}

$failed = $null
try {
    $deadline = (Get-Date).AddSeconds(20)
    while ($process.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200; $process.Refresh() }
    if ($process.MainWindowHandle -eq 0) { throw 'the window did not open' }
    # The first handle can be a helper window; give the real one a moment to appear.
    Start-Sleep -Seconds 3

    # Every tab can be reached by name, then back to the first one.
    foreach ($tab in '변환', '기록', '설정', '다운로드') {
        Invoke-Element (Find-Element $tab ([System.Windows.Automation.ControlType]::RadioButton))
        Start-Sleep -Milliseconds 400
    }
    Write-Host 'tabs: ok'

    # The link came in as a command-line argument; the button queues and downloads it.
    Invoke-Element (Find-Element '다운로드 · Enter' ([System.Windows.Automation.ControlType]::Button))
    Write-Host 'download button: clicked'

    $history = Join-Path $data 'history.json'
    $deadline = (Get-Date).AddSeconds(120)
    while (-not (Test-Path $history) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
    if (-not (Test-Path $history)) { throw 'nothing was recorded in the history within 2 minutes' }
    $entry = (Get-Content $history -Raw | ConvertFrom-Json)[0]
    if (-not (Test-Path -LiteralPath $entry.file)) { throw "history points to a missing file: $($entry.file)" }
    Write-Host "downloaded: $($entry.title).$($entry.format) ($((Get-Item -LiteralPath $entry.file).Length) bytes)"

    # The finished item offers "open folder"; the queue header can clear it.
    Find-Element '폴더 열기' $null 10 | Out-Null
    Invoke-Element (Find-Element '끝난 항목 지우기' $null 10)
    Write-Host 'queue actions: ok'

    # Closing through the window's own button ends the process.
    Invoke-Element (Find-Element '닫기')
    if (-not $process.WaitForExit(10000)) { throw 'the app did not exit after Close' }
    $settings = Get-Content (Join-Path $data 'settings.json') -Raw | ConvertFrom-Json
    if ($settings.format -ne 1 -or $settings.download_dir -ne $out) { throw 'settings were not kept' }
    Write-Host 'close + settings: ok'
} catch {
    $failed = $_
} finally {
    if (-not $process.HasExited) { $process.Kill() }
    Start-Sleep -Milliseconds 500
    Remove-Item $sandbox -Recurse -Force -ErrorAction SilentlyContinue
}
if ($failed) { throw $failed }
Write-Host 'UI SMOKE PASSED'
