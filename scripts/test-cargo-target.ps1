[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'cargo-target.ps1')
$previous = $env:CARGO_TARGET_DIR
$previousResolved = $env:GLYPHSHIFT_CARGO_TARGET_DIR
try {
    $base = Join-Path $repoRoot 'local-test/cache/cargo-target-contract/global-root'
    $env:CARGO_TARGET_DIR = $base
    $env:GLYPHSHIFT_CARGO_TARGET_DIR = ''
    $expected = Join-Path $base 'glyphshift'
    if ((Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot) -ne $expected) { throw 'Application isolation failed.' }
    if ((Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot) -ne $expected) { throw 'Repeated resolution changed the path.' }
    $metadata = & cargo metadata --manifest-path (Join-Path $repoRoot 'Cargo.toml') --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0 -or ($metadata | ConvertFrom-Json).target_directory -ne $expected) { throw 'Child Cargo did not inherit the resolved path.' }
    $override = Join-Path $repoRoot 'local-test/cache/cargo-target-contract/explicit output'
    if ((Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot -Override $override) -ne $override) { throw 'Override was rewritten.' }
    if ((Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot) -ne $override) { throw 'Nested build lost its override.' }
    $env:CARGO_TARGET_DIR = $expected
    $env:GLYPHSHIFT_CARGO_TARGET_DIR = ''
    if ((Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot) -ne $expected) { throw 'Existing application suffix was duplicated.' }
    'Cargo target isolation, inheritance, idempotence and explicit override: passed.'
}
finally {
    $env:CARGO_TARGET_DIR = $previous
    $env:GLYPHSHIFT_CARGO_TARGET_DIR = $previousResolved
}
