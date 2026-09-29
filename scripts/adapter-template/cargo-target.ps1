function Get-AdapterCargoTargetDirectory {
    param([Parameter(Mandatory)][string]$RepoRoot)
    $spec = Get-Content -Raw (Join-Path $RepoRoot 'plugin.json') | ConvertFrom-Json
    if (-not $env:GLYPHSHIFT_ADAPTER_CACHE_ROOT) {
        if (-not $env:CARGO_TARGET_DIR) {
            # CI has no machine-level Cargo cache configuration. Keep all build
            # output outside the immutable SDK, in ignored local storage.
            $env:CARGO_TARGET_DIR = Join-Path $RepoRoot 'local-test/cache/cargo'
        }
        $metadata = & cargo metadata --manifest-path (Join-Path $RepoRoot '.sdk/Cargo.toml') --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve Cargo cache.' }
        $root = [IO.Path]::GetFullPath(($metadata | ConvertFrom-Json).target_directory).TrimEnd('\')
        if ([IO.Path]::GetFileName($root) -eq 'glyphshift') { $root = Split-Path $root -Parent }
        $env:GLYPHSHIFT_ADAPTER_CACHE_ROOT = $root
    }
    $env:CARGO_TARGET_DIR = Join-Path $env:GLYPHSHIFT_ADAPTER_CACHE_ROOT $spec.package_id
    return $env:CARGO_TARGET_DIR
}
