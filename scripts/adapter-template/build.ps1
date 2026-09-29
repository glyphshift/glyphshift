[CmdletBinding()]
param([ValidateSet('Debug','Release')][string]$Profile = 'Release')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    & python -B scripts/bootstrap-sdk.py
    if ($LASTEXITCODE -ne 0) { throw 'SDK verification failed.' }
    & python -B scripts/test-bootstrap.py
    if ($LASTEXITCODE -ne 0) { throw 'SDK bootstrap contracts failed.' }
    . (Join-Path $PSScriptRoot 'cargo-target.ps1')
    $targetRoot = Get-AdapterCargoTargetDirectory -RepoRoot $repoRoot
    $spec = Get-Content -Raw plugin.json | ConvertFrom-Json
    $profileArguments = @()
    if ($Profile -eq 'Release') { $profileArguments = @('--release') }
    foreach ($architecture in @('x86_64','x86')) {
        $target = if ($architecture -eq 'x86') { 'i686-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }
        $entries = @($spec.adapters | Where-Object { $architecture -in $_.architectures })
        if ($entries.Count -eq 0 -and $architecture -eq 'x86') { continue }
        $packages = @($spec.crates | Where-Object { $architecture -in $_.architectures } | ForEach-Object name)
        if ($packages.Count -gt 0) {
            $packageArguments = @()
            foreach ($package in $packages) { $packageArguments += @('-p',$package) }
            & cargo build --locked --target $target @packageArguments @profileArguments
            if ($LASTEXITCODE -ne 0) { throw "Adapter build failed: $target" }
            & cargo test --locked --target $target @packageArguments @profileArguments
            if ($LASTEXITCODE -ne 0) { throw "Adapter tests failed: $target" }
        }
        $outputRoot = Join-Path $targetRoot "$target/$($Profile.ToLowerInvariant())"
        foreach ($entry in @($entries | Where-Object { $_.build_script })) {
            $arguments = @{ OutputRoot=$outputRoot }
            & (Join-Path $PSScriptRoot $entry.build_script) @arguments
        }
        & cargo build --locked --manifest-path .sdk/Cargo.toml -p glyphshift-adapter-devkit --target $target @profileArguments
        if ($LASTEXITCODE -ne 0) { throw "SDK tool build failed: $target" }
    }
    & python -B scripts/package.py --target-root $targetRoot --profile $Profile.ToLowerInvariant()
    if ($LASTEXITCODE -ne 0) { throw 'GSP packaging failed.' }
}
finally { Pop-Location }
