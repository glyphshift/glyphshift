[CmdletBinding()]
param(
    [ValidateSet('Debug','Release')][string]$Profile = 'Debug',
    [string]$CargoTargetDir
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'cargo-target.ps1')
$CargoTargetDir = Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot -Override $CargoTargetDir
$profileDirectory = $Profile.ToLowerInvariant()
$extracted = Join-Path $repoRoot "local-test/cache/base-adapter-test-artifacts/$profileDirectory"

& python -B (Join-Path $PSScriptRoot 'fetch-base-adapters.py') --output $extracted
if ($LASTEXITCODE -ne 0) {
    throw "Base Adapter fetch failed with exit code $LASTEXITCODE"
}

$destination = Join-Path $CargoTargetDir $profileDirectory
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Get-ChildItem -LiteralPath (Join-Path $extracted 'x86_64') -Filter '*.dll' -File | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $destination $_.Name) -Force
}
Write-Output "Pinned base Adapter test artifacts ready: $destination"
