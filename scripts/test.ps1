[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $repoRoot 'Cargo.toml'

# Native integration tests load real DLL and executable artifacts by filename.
# `cargo test --workspace` does not guarantee that those artifacts are emitted first.
& cargo build `
    --manifest-path $manifestPath `
    -p glyphshift-adapter-direct2d-native `
    -p glyphshift-adapter-directwrite-native `
    -p glyphshift-adapter-draw-text-native `
    -p glyphshift-adapter-gdi-native `
    -p glyphshift-adapter-gdi-text-out-native `
    -p glyphshift-adapter-gdiplus-native `
    -p glyphshift-adapter-gtk3-pango-native `
    -p glyphshift-adapter-qt-painter-native `
    -p glyphshift-adapter-qt-quick-native `
    -p glyphshift-adapter-raylib-native `
    -p glyphshift-adapter-unity-mono-standard-ui-native `
    -p glyphshift-target-runtime `
    -p glyphshift-test-controller-plugin `
    -p glyphshift-test-isolated-worker `
    -p glyphshift-test-acquisition-worker `
    -p glyphshift-windows-runtime-target
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter package build failed with exit code $LASTEXITCODE"
}

$pausedAdapterPackages = @(
    'glyphshift-adapter-console',
    'glyphshift-adapter-console-native',
    'glyphshift-adapter-ocr'
)
$testArguments = @('test', '--manifest-path', $manifestPath, '--workspace')
foreach ($pausedAdapterPackage in $pausedAdapterPackages) {
    $testArguments += @('--exclude', $pausedAdapterPackage)
}
& cargo @testArguments
if ($LASTEXITCODE -ne 0) {
    throw "workspace tests failed with exit code $LASTEXITCODE"
}
