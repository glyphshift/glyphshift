[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $repoRoot 'Cargo.toml'

# Native integration tests load real DLL and executable artifacts by filename.
# `cargo test --workspace` does not guarantee that those artifacts are emitted first.
& cargo build `
    --manifest-path $manifestPath `
    -p glyphshift-adapter-draw-text-native `
    -p glyphshift-adapter-gdi-native `
    -p glyphshift-adapter-gdi-text-out-native `
    -p glyphshift-adapter-gdiplus-native `
    -p glyphshift-target-runtime `
    -p glyphshift-test-controller-plugin `
    -p glyphshift-windows-runtime-target
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter package build failed with exit code $LASTEXITCODE"
}

& cargo test --manifest-path $manifestPath --workspace
if ($LASTEXITCODE -ne 0) {
    throw "workspace tests failed with exit code $LASTEXITCODE"
}
