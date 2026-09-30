[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$workspaceManifest = Join-Path $PSScriptRoot '..\..\..\..\Cargo.toml'

& (Join-Path $PSScriptRoot '..\..\..\..\scripts\prepare-base-adapter-test-artifacts.ps1')

& cargo test `
    --manifest-path $workspaceManifest `
    -p glyphshift-adapter-native-host
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter host contract failed with exit code $LASTEXITCODE"
}
