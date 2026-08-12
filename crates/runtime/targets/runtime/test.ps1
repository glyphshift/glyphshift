[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$workspaceManifest = Join-Path $PSScriptRoot '..\..\..\..\Cargo.toml'

& cargo build `
    --manifest-path $workspaceManifest `
    -p glyphshift-adapter-gdi-native `
    -p glyphshift-test-native-adapter
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter package build failed with exit code $LASTEXITCODE"
}

& cargo test `
    --manifest-path $workspaceManifest `
    -p glyphshift-target-runtime
if ($LASTEXITCODE -ne 0) {
    throw "target Runtime contract failed with exit code $LASTEXITCODE"
}

& cargo test `
    --manifest-path $workspaceManifest `
    -p glyphshift-target-runtime `
    --test target_runtime_contract `
    trh_004_requests_adapter_refresh_after_each_lifecycle_change `
    -- --ignored --exact
if ($LASTEXITCODE -ne 0) {
    throw "target Runtime refresh contract failed with exit code $LASTEXITCODE"
}
