# Ask Cargo to resolve environment variables and local/global configuration in its own precedence.
function Get-GlyphshiftCargoTargetDirectory {
    param([Parameter(Mandatory)][string]$RepoRoot, [string]$Override)
    if (-not [string]::IsNullOrWhiteSpace($Override)) {
        return [IO.Path]::GetFullPath($Override)
    }
    Push-Location $RepoRoot
    try {
        $metadata = & cargo metadata --manifest-path (Join-Path $RepoRoot 'Cargo.toml') --no-deps --format-version 1
        if ($LASTEXITCODE -ne 0) { throw 'Cargo target directory resolution failed.' }
        return [IO.Path]::GetFullPath(($metadata | ConvertFrom-Json).target_directory)
    }
    finally { Pop-Location }
}
