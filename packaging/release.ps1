# Builds the files to upload to the portal into dist\:
#   xdownload2.exe         the app
#   xdownload2.exe.sha256  its SHA-256 (sha256sum format)
#   xdownload2.json        what the app's update check reads: { version, url, sha256 }
#
# Usage:  pwsh packaging\release.ps1 [-SkipTests] [-CertThumbprint <thumbprint>]
# With -CertThumbprint (a code-signing certificate in the user's store) the exe is signed
# before the hash is taken. Without it the exe is unsigned and Windows SmartScreen warns.
param(
    [switch] $SkipTests,
    [string] $CertThumbprint = '',
    [string] $BaseUrl = 'https://xtool.duckdns.org/dl'
)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot)
$env:PATH = "$HOME\.cargo\bin;$env:PATH"

function Invoke-Checked([string] $What, [scriptblock] $Command) {
    & $Command
    if ($LASTEXITCODE -ne 0) { throw "$What failed (exit $LASTEXITCODE)" }
}

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
if (-not (Select-String -Path CHANGELOG.md -Pattern "^## $([regex]::Escape($version))" -Quiet)) {
    throw "CHANGELOG.md has no entry for $version"
}
if (-not (Select-String -Path assets\xdownload2.manifest -Pattern "version=`"$([regex]::Escape($version)).0`"" -Quiet)) {
    throw "assets\xdownload2.manifest does not carry version $version.0"
}

# A stale exe must never be shipped after a failed build. The target is fixed in
# .cargo\config.toml; target\release is where older builds used to land.
$target = 'target\x86_64-pc-windows-msvc\release\xdownload2.exe'
Remove-Item $target, 'target\release\xdownload2.exe', 'target\debug\xdownload2.exe' -ErrorAction SilentlyContinue
if (Test-Path dist) {
    try { Remove-Item dist -Recurse -Force }
    catch { throw 'dist\ cannot be replaced: close the running xdownload2.exe (it was started from dist\) and run this again' }
}

if (-not $SkipTests) {
    Invoke-Checked 'cargo test' { cargo test --release }
}
Invoke-Checked 'cargo build' { cargo build --release }
if (-not (Test-Path $target)) { throw "build produced no $target" }

New-Item -ItemType Directory dist | Out-Null
$exe = 'dist\xdownload2.exe'
Copy-Item $target $exe

if ($CertThumbprint) {
    $signtool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe' | Sort-Object FullName | Select-Object -Last 1
    if (-not $signtool) { throw 'signtool.exe not found (install the Windows SDK)' }
    Invoke-Checked 'signtool' { & $signtool.FullName sign /sha1 $CertThumbprint /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $exe }
}

# The hash is taken last, from the exact file that will be uploaded.
$hash = (Get-FileHash $exe -Algorithm SHA256).Hash.ToLower()
"$hash  xdownload2.exe" | Set-Content dist\xdownload2.exe.sha256 -Encoding ascii
[ordered]@{ version = $version; url = "$BaseUrl/xdownload2.exe"; sha256 = $hash } |
    ConvertTo-Json | Set-Content dist\xdownload2.json -Encoding utf8

$fileVersion = (Get-Item $exe).VersionInfo.ProductVersion
if ($fileVersion -ne $version) { throw "exe reports version $fileVersion, expected $version" }
$signed = if ($CertThumbprint) { 'signed' } else { 'UNSIGNED' }
Write-Host "dist\ ready: xdownload2.exe $version ($signed, $([math]::Round((Get-Item $exe).Length / 1MB, 1)) MB)"
Write-Host "Upload the three files in dist\ to $BaseUrl/"
