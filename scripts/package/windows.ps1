# Makes the Windows installer, claudius-<version>-x86_64-pc-windows-msvc.msi, from the release's Windows archive, with
# WiX 5 (packaging/windows/claudius.wxs). wix has to be on PATH: `dotnet tool install --global wix --version 5.0.2`.
#
#   pwsh scripts/package/windows.ps1 <claudius-...-x86_64-pc-windows-msvc.zip> <out dir>

param(
    [Parameter(Mandatory)] [string]$Archive,
    [Parameter(Mandatory)] [string]$Out
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 3

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$name = [IO.Path]::GetFileNameWithoutExtension($Archive)
if ($name -notmatch '^claudius-(.+)-x86_64-pc-windows-msvc$') {
    throw "$Archive is not a Windows archive of claudius"
}
$version = $Matches[1]
# an installer's version is numbers only, so a pre-release (0.5.0-rc.1) is 0.5.0 to Windows
$msiVersion = $version -replace '-.*$', ''

$tmp = Join-Path ([IO.Path]::GetTempPath()) "claudius-msi-$([Guid]::NewGuid())"
try {
    Expand-Archive -Path $Archive -DestinationPath $tmp
    $stage = Join-Path $tmp $name
    New-Item -ItemType Directory -Force -Path $Out | Out-Null
    $msi = Join-Path (Resolve-Path $Out).Path "$name.msi"
    wix build (Join-Path $root 'packaging\windows\claudius.wxs') -arch x64 -o $msi `
        -d "Version=$msiVersion" -d "Stage=$stage" -d "Icons=$(Join-Path $root 'packaging\icons')"
    if ($LASTEXITCODE -ne 0) {
        throw "wix build failed with exit code $LASTEXITCODE"
    }
    $hash = (Get-FileHash -Algorithm SHA256 -Path $msi).Hash.ToLowerInvariant()
    # the format sha256sum --check reads, with a Unix line ending
    [IO.File]::WriteAllText("$msi.sha256", "$hash  $name.msi`n")
    Write-Output "windows.ps1: $msi"
} finally {
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $tmp
}
