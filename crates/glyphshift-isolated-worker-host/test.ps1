[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$workspaceManifest = Join-Path $PSScriptRoot '..\..\Cargo.toml'

& cargo build `
    --manifest-path $workspaceManifest `
    -p glyphshift-test-isolated-worker
if ($LASTEXITCODE -ne 0) {
    throw "Isolated Worker fixture build failed with exit code $LASTEXITCODE"
}

& cargo test `
    --manifest-path $workspaceManifest `
    -p glyphshift-isolated-worker-host
if ($LASTEXITCODE -ne 0) {
    throw "Isolated Worker process contract failed with exit code $LASTEXITCODE"
}
