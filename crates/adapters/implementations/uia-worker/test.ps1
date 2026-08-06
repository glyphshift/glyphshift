[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..\..\..')).Path
& cargo build --manifest-path (Join-Path $repoRoot 'Cargo.toml') `
    -p glyphshift-windows-runtime-target `
    -p glyphshift-adapter-uia-worker
if ($LASTEXITCODE -ne 0) {
    throw "UIA worker fixture build failed with exit code $LASTEXITCODE"
}

& cargo test --manifest-path (Join-Path $PSScriptRoot 'Cargo.toml')
if ($LASTEXITCODE -ne 0) {
    throw "UIA worker contract failed with exit code $LASTEXITCODE"
}
