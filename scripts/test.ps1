[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'cargo-target.ps1')
$null = Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot
$manifestPath = Join-Path $repoRoot 'Cargo.toml'

& python -B (Join-Path $PSScriptRoot 'check-public-core-boundary.py')
if ($LASTEXITCODE -ne 0) { throw 'Public Core source boundary check failed.' }

& python -B (Join-Path $PSScriptRoot 'test-base-runtime.py')
if ($LASTEXITCODE -ne 0) { throw 'Base Runtime distribution contracts failed.' }

# Native integration tests load these artifacts by filename, so emit the public
# Core DLLs and synthetic test fixtures before enumerating package tests.
& cargo build `
    --manifest-path $manifestPath `
    -p glyphshift-adapter-directwrite-native `
    -p glyphshift-adapter-draw-text-native `
    -p glyphshift-adapter-gdi-native `
    -p glyphshift-adapter-gdi-text-out-native `
    -p glyphshift-adapter-gdiplus-native `
    -p glyphshift-target-runtime `
    -p glyphshift-test-native-adapter `
    -p glyphshift-test-controller-plugin `
    -p glyphshift-test-isolated-worker `
    -p glyphshift-test-acquisition-worker `
    -p glyphshift-windows-runtime-target
if ($LASTEXITCODE -ne 0) {
    throw "Public Core native package build failed with exit code $LASTEXITCODE"
}

# Keep the repository rule of enumerating active packages instead of invoking a
# raw `cargo test --workspace`.
$metadata = (& cargo metadata --manifest-path $manifestPath --format-version 1 --no-deps | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Cannot read Public Core package metadata.' }
$testArguments = @('test', '--no-fail-fast', '--manifest-path', $manifestPath)
foreach ($package in $metadata.packages) {
    if ($metadata.workspace_members -contains $package.id) {
        $testArguments += @('-p', $package.name)
    }
}
& cargo @testArguments
if ($LASTEXITCODE -ne 0) { throw "Public Core package tests failed with exit code $LASTEXITCODE" }

