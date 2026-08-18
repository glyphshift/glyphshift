[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Profile = 'Release',

    [string]$DataRoot,

    [switch]$BuildOnly
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$desktopRoot = Join-Path $repoRoot 'apps\glyphshift-desktop'
$localTestRoot = Join-Path $repoRoot 'local-test'
$buildId = (Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmssfff')
$reviewRoot = Join-Path $localTestRoot "evidence\desktop-review\$buildId"
$runtimeRoot = Join-Path $reviewRoot 'runtime'
$cargoTargetDir = Join-Path $localTestRoot 'desktop-review\cargo-target'
$profileDirectory = $Profile.ToLowerInvariant()

if ([string]::IsNullOrWhiteSpace($DataRoot)) {
    $DataRoot = Join-Path $localTestRoot 'desktop-review-data'
}
$DataRoot = [System.IO.Path]::GetFullPath($DataRoot)

function Assert-LocalTestPath([string]$Candidate, [string]$Purpose) {
    $prefix = [System.IO.Path]::GetFullPath($localTestRoot).TrimEnd('\') + '\'
    if (-not $Candidate.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Purpose must stay below the repository local-test root."
    }
}

Assert-LocalTestPath $DataRoot 'Desktop review data'

$requiredCommands = @(
    (Join-Path $desktopRoot 'node_modules\.bin\vite.cmd'),
    (Join-Path $desktopRoot 'node_modules\.bin\vue-tsc.cmd')
)
if (@($requiredCommands | Where-Object { -not (Test-Path -LiteralPath $_) }).Count -gt 0) {
    throw 'Desktop dependencies are missing. Run npm ci in the desktop application first.'
}

$repoPrefix = $repoRoot.TrimEnd('\') + '\'
$repoDesktopProcesses = @(Get-CimInstance Win32_Process | Where-Object {
    $_.Name -eq 'glyphshift-desktop-shell.exe' -and
    -not [string]::IsNullOrWhiteSpace($_.ExecutablePath) -and
    [System.IO.Path]::GetFullPath($_.ExecutablePath).StartsWith(
        $repoPrefix,
        [System.StringComparison]::OrdinalIgnoreCase
    )
})
foreach ($process in $repoDesktopProcesses) {
    Stop-Process -Id $process.ProcessId -Force
}

New-Item -ItemType Directory -Path $reviewRoot -Force | Out-Null
New-Item -ItemType Directory -Path $DataRoot -Force | Out-Null
New-Item -ItemType Directory -Path $cargoTargetDir -Force | Out-Null

& (Join-Path $PSScriptRoot 'build-runtime-bundle.ps1') `
    -Profile $Profile `
    -OutputRoot $runtimeRoot `
    -CargoTargetDir $cargoTargetDir

Push-Location $desktopRoot
try {
    & npm.cmd run build
    if ($LASTEXITCODE -ne 0) {
        throw 'The Glyphshift desktop frontend did not build.'
    }
}
finally {
    Pop-Location
}

$env:CARGO_TARGET_DIR = $cargoTargetDir
$desktopBuildArguments = @(
    'build',
    '--manifest-path', (Join-Path $repoRoot 'Cargo.toml'),
    '-p', 'glyphshift-desktop-shell'
)
$desktopBuildArguments += @('--features', 'custom-protocol')
if ($Profile -eq 'Release') {
    $desktopBuildArguments += '--release'
}
& cargo @desktopBuildArguments
if ($LASTEXITCODE -ne 0) {
    throw "The $Profile Glyphshift desktop shell did not build."
}

$verifierPath = Join-Path $cargoTargetDir "$profileDirectory\glyphshift-runtime-bundle-verify.exe"
& $verifierPath $runtimeRoot
if ($LASTEXITCODE -ne 0) {
    throw 'The synchronized desktop Runtime Bundle failed verification.'
}

$desktopExecutable = Join-Path $cargoTargetDir "$profileDirectory\glyphshift-desktop-shell.exe"
if (-not (Test-Path -LiteralPath $desktopExecutable -PathType Leaf)) {
    throw 'The synchronized desktop executable is missing.'
}

if ($BuildOnly) {
    Write-Output "Synchronized desktop review build ready: $reviewRoot"
    exit 0
}

$env:GLYPHSHIFT_DATA_ROOT = $DataRoot
$env:GLYPHSHIFT_RUNTIME_ROOT = $runtimeRoot
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9223 --remote-allow-origins=*'
$process = Start-Process `
    -FilePath $desktopExecutable `
    -WorkingDirectory $repoRoot `
    -RedirectStandardOutput (Join-Path $reviewRoot 'app.stdout.log') `
    -RedirectStandardError (Join-Path $reviewRoot 'app.stderr.log') `
    -PassThru

$reviewDescriptor = [ordered]@{
    schema = 'glyphshift.desktop-review/1'
    processId = $process.Id
    executableSha256 = (Get-FileHash -LiteralPath $desktopExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
    runtimeManifestSha256 = (
        Get-FileHash -LiteralPath (Join-Path $runtimeRoot 'runtime-bundle.json') -Algorithm SHA256
    ).Hash.ToLowerInvariant()
}
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText(
    (Join-Path $reviewRoot 'review.json'),
    ($reviewDescriptor | ConvertTo-Json -Depth 3),
    $utf8WithoutBom
)

Write-Output "Synchronized desktop review started: $reviewRoot"
