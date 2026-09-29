[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$RegistryExecutable,
    [string]$AdapterPackage
)

$ErrorActionPreference = 'Stop'
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'cargo-target.ps1')
$target = Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot
Push-Location $repoRoot
try {
    & cargo fmt -p glyphshift-registry-client -p glyphshift-resource-inspector -- --check
    if ($LASTEXITCODE -ne 0) { throw 'Registry client formatting failed.' }
    & cargo test --locked -p glyphshift-registry-client -p glyphshift-resource-inspector -p glyphshift-dictionary-distribution -p glyphshift-plugin-package
    if ($LASTEXITCODE -ne 0) { throw 'Registry client contracts failed.' }
    & cargo clippy --locked -p glyphshift-registry-client -p glyphshift-resource-inspector --all-targets --no-deps -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Registry client static checks failed.' }
    & cargo build --locked -p glyphshift-registry-client -p glyphshift-resource-inspector
    if ($LASTEXITCODE -ne 0) { throw 'Registry client build failed.' }
    $suffix = if ($IsWindows) { '.exe' } else { '' }
    $arguments = @('-B', (Join-Path $PSScriptRoot 'test-registry-client.py'),
        '--client', (Join-Path $target "debug/glyphshift-registry$suffix"),
        '--inspector', (Join-Path $target "debug/glyphshift-resource-inspector$suffix"),
        '--registry', $RegistryExecutable)
    if ($AdapterPackage) { $arguments += @('--adapter', $AdapterPackage) }
    & python @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Go/Rust Registry HTTPS installation failed.' }
}
finally { Pop-Location }
