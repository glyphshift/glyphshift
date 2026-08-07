[CmdletBinding()]
param(
    [string]$OcrSupportRoot = $env:GLYPHSHIFT_OCR_SUPPORT_ROOT
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$desktopRoot = Join-Path $repoRoot 'apps\glyphshift-desktop'
$localTestRoot = Join-Path $repoRoot 'target\local-test'
$buildId = (Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmssfff')
$buildRoot = Join-Path $localTestRoot "evidence\desktop-release\$buildId"
$runtimeRoot = Join-Path $buildRoot 'runtime'
$candidateRoot = Join-Path $buildRoot 'candidate'
$cargoTargetDir = Join-Path $localTestRoot 'desktop-release\tauri-build'
$localConfigPath = Join-Path $buildRoot 'tauri.release.local.conf.json'

$requiredCommands = @(
    (Join-Path $desktopRoot 'node_modules\.bin\tauri.cmd'),
    (Join-Path $desktopRoot 'node_modules\.bin\vite.cmd'),
    (Join-Path $desktopRoot 'node_modules\.bin\vue-tsc.cmd')
)
if (@($requiredCommands | Where-Object { -not (Test-Path -LiteralPath $_) }).Count -gt 0) {
    throw 'Desktop dependencies are missing. Run npm ci in the desktop application first.'
}

New-Item -ItemType Directory -Path $buildRoot -Force | Out-Null
New-Item -ItemType Directory -Path $candidateRoot -Force | Out-Null
New-Item -ItemType Directory -Path $cargoTargetDir -Force | Out-Null

$runtimeBundleArguments = @{
    Profile = 'Release'
    OutputRoot = $runtimeRoot
    CargoTargetDir = $cargoTargetDir
}
if (-not [string]::IsNullOrWhiteSpace($OcrSupportRoot)) {
    $runtimeBundleArguments.OcrSupportRoot = $OcrSupportRoot
}
& (Join-Path $PSScriptRoot 'build-runtime-bundle.ps1') @runtimeBundleArguments

$runtimeManifestPath = Join-Path $runtimeRoot 'runtime-bundle.json'
$runtimeManifest = Get-Content -Raw -LiteralPath $runtimeManifestPath | ConvertFrom-Json
if ($runtimeManifest.schema -ne 'glyphshift.runtime-bundle/3') {
    throw 'Desktop Release requires Runtime Bundle /3.'
}
if (Test-Path -LiteralPath (Join-Path $runtimeRoot 'test-target.exe')) {
    throw 'Desktop Release Runtime must not include the synthetic test target.'
}

$resources = [ordered]@{}
$resources[$runtimeRoot] = 'runtime'
$localConfig = [ordered]@{
    bundle = [ordered]@{
        active = $true
        targets = @('nsis')
        resources = $resources
    }
}
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText(
    $localConfigPath,
    ($localConfig | ConvertTo-Json -Depth 6),
    $utf8WithoutBom
)

$env:CARGO_TARGET_DIR = $cargoTargetDir
$buildStartedAt = Get-Date
Push-Location $desktopRoot
try {
    & npm.cmd run tauri -- build `
        --config $localConfigPath `
        --bundles nsis `
        --no-sign `
        --ci
    if ($LASTEXITCODE -ne 0) {
        throw 'The Glyphshift desktop Release candidate did not build.'
    }
}
finally {
    Pop-Location
}

$releaseExecutable = Join-Path $cargoTargetDir 'release\glyphshift-desktop-shell.exe'
if (-not (Test-Path -LiteralPath $releaseExecutable -PathType Leaf)) {
    throw 'The unpacked Glyphshift Release executable is missing.'
}
$installer = Get-ChildItem -LiteralPath (Join-Path $cargoTargetDir 'release\bundle\nsis') `
    -Filter '*.exe' `
    -File |
    Where-Object { $_.LastWriteTime -ge $buildStartedAt.AddSeconds(-2) } |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if ($null -eq $installer) {
    throw 'The Glyphshift NSIS candidate is missing.'
}

$candidateExecutable = Join-Path $candidateRoot 'glyphshift-desktop-shell.exe'
$candidateInstaller = Join-Path $candidateRoot $installer.Name
Copy-Item -LiteralPath $releaseExecutable -Destination $candidateExecutable
Copy-Item -LiteralPath $installer.FullName -Destination $candidateInstaller

$candidateManifest = [ordered]@{
    schema = 'glyphshift.desktop-candidate/1'
    signed = $false
    executable = [ordered]@{
        file = [System.IO.Path]::GetFileName($candidateExecutable)
        sha256 = (Get-FileHash -LiteralPath $candidateExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    installer = [ordered]@{
        file = [System.IO.Path]::GetFileName($candidateInstaller)
        sha256 = (Get-FileHash -LiteralPath $candidateInstaller -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    runtimeManifestSha256 = (
        Get-FileHash -LiteralPath $runtimeManifestPath -Algorithm SHA256
    ).Hash.ToLowerInvariant()
}
[System.IO.File]::WriteAllText(
    (Join-Path $candidateRoot 'candidate-manifest.json'),
    ($candidateManifest | ConvertTo-Json -Depth 5),
    $utf8WithoutBom
)

Write-Output "Desktop Release candidate ready: $buildRoot"
