[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'cargo-target.ps1')
$null = Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot
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
    -p glyphshift-adapter-qt-text-document-native `
    -p glyphshift-adapter-qt-quick-native `
    -p glyphshift-adapter-raylib-native `
    -p glyphshift-adapter-sidefx-cv-paint-buffer-native `
    -p glyphshift-adapter-unity-mono-standard-ui-native `
    -p glyphshift-target-runtime `
    -p glyphshift-test-controller-plugin `
    -p glyphshift-test-isolated-worker `
    -p glyphshift-test-acquisition-worker `
    -p glyphshift-windows-runtime-target
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter package build failed with exit code $LASTEXITCODE"
}

# Enumerate active packages explicitly; archived UIA and its dependants never enter a test build.
$excludedPackages = [System.Collections.Generic.HashSet[string]]::new()
@('glyphshift-adapter-console', 'glyphshift-adapter-console-native', 'glyphshift-adapter-ocr',
  'glyphshift-adapter-uia', 'glyphshift-adapter-uia-worker', 'glyphshift-adapter-ocr-worker') |
    ForEach-Object { $null = $excludedPackages.Add($_) }
$metadata = (& cargo metadata --manifest-path $manifestPath --format-version 1 --no-deps | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Cannot read active package metadata.' }
do {
    $changed = $false
    foreach ($package in $metadata.packages) {
        if (@($package.dependencies | Where-Object { $excludedPackages.Contains($_.name) }).Count -gt 0) {
            $changed = $excludedPackages.Add($package.name) -or $changed
        }
    }
} while ($changed)
$testArguments = @('test', '--no-fail-fast', '--manifest-path', $manifestPath)
foreach ($package in $metadata.packages) {
    if ($metadata.workspace_members -contains $package.id -and -not $excludedPackages.Contains($package.name)) {
        $testArguments += @('-p', $package.name)
    }
}
& cargo @testArguments
if ($LASTEXITCODE -ne 0) { throw "active package tests failed with exit code $LASTEXITCODE" }
