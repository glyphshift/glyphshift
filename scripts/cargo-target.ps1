# Resolve the global Cargo root, then isolate this application's outputs.
function Get-GlyphshiftCargoTargetDirectory {
    param([Parameter(Mandatory)][string]$RepoRoot, [string]$Override)
    if (-not [string]::IsNullOrWhiteSpace($Override)) {
        $env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath($Override)
        $env:GLYPHSHIFT_CARGO_TARGET_DIR = $env:CARGO_TARGET_DIR
        return $env:CARGO_TARGET_DIR
    }
    if ($env:GLYPHSHIFT_CARGO_TARGET_DIR -and $env:CARGO_TARGET_DIR -eq $env:GLYPHSHIFT_CARGO_TARGET_DIR) {
        return $env:CARGO_TARGET_DIR
    }
    Push-Location $RepoRoot
    try {
        $metadata = & cargo metadata --manifest-path (Join-Path $RepoRoot 'Cargo.toml') --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw 'Cargo target directory resolution failed.' }
        $root = [IO.Path]::GetFullPath(($metadata | ConvertFrom-Json).target_directory).TrimEnd([IO.Path]::DirectorySeparatorChar)
        $resolved = if ([IO.Path]::GetFileName($root) -eq 'glyphshift') { $root } else { Join-Path $root 'glyphshift' }
        # Child Cargo invocations and native fixture loaders must resolve the same directory.
        $env:CARGO_TARGET_DIR = $resolved
        $env:GLYPHSHIFT_CARGO_TARGET_DIR = $resolved
        return $resolved
    }
    finally { Pop-Location }
}
