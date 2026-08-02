[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$workspaceManifest = Join-Path $PSScriptRoot '..\..\Cargo.toml'

& cargo build `
    --manifest-path $workspaceManifest `
    -p glyphshift-adapter-gdi-native `
    -p glyphshift-adapter-gdiplus-native
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter package build failed with exit code $LASTEXITCODE"
}

& cargo test `
    --manifest-path $workspaceManifest `
    -p glyphshift-adapter-native-host
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter host contract failed with exit code $LASTEXITCODE"
}
