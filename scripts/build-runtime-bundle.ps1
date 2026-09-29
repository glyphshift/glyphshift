[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Profile = 'Debug',

    [string]$OutputRoot,

    [string]$CargoTargetDir,

    [switch]$IncludeTestTarget,

    [switch]$KeepExistingOutput
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$localTestRoot = Join-Path $repoRoot 'local-test'
if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
    $OutputRoot = Join-Path $localTestRoot "runtime-bundle\$($Profile.ToLowerInvariant())"
}

# Public Core only builds the five first-party Windows adapters declared by
# adapter-distribution.json. Official engine/framework adapters are produced by
# their private adapter repositories and installed as GSP plugins.
& (Join-Path $PSScriptRoot 'build-core-runtime.ps1') `
    -Profile $Profile `
    -OutputRoot $OutputRoot `
    -CargoTargetDir $CargoTargetDir `
    -IncludeTestTarget:$IncludeTestTarget
if ($LASTEXITCODE -ne 0) {
    throw "Core Runtime Bundle build failed with exit code $LASTEXITCODE"
}

