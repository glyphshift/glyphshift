[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$workspaceManifest = Join-Path $PSScriptRoot '..\..\Cargo.toml'

& cargo build `
    --manifest-path $workspaceManifest `
    -p glyphshift-adapter-gdi-native
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter package build failed with exit code $LASTEXITCODE"
}

& cargo test `
    --manifest-path $workspaceManifest `
    -p glyphshift-target-runtime
if ($LASTEXITCODE -ne 0) {
    throw "target Runtime contract failed with exit code $LASTEXITCODE"
}
