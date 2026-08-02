[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$workspaceManifest = Join-Path $PSScriptRoot '..\..\Cargo.toml'

& cargo build `
    --manifest-path $workspaceManifest `
    -p glyphshift-test-controller-plugin
if ($LASTEXITCODE -ne 0) {
    throw "Controller test plugin build failed with exit code $LASTEXITCODE"
}

& cargo test `
    --manifest-path $workspaceManifest `
    -p glyphshift-controller-host
if ($LASTEXITCODE -ne 0) {
    throw "Controller process contract failed with exit code $LASTEXITCODE"
}
