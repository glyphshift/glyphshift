[CmdletBinding()]
param(
    [switch]$Detached
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = Split-Path -Parent $PSScriptRoot
$desktopRoot = Join-Path $repoRoot 'apps\glyphshift-desktop'
$localTaskRoot = Join-Path $repoRoot 'target\local-test\evidence\desktop-dev'
$cargoTargetDir = Join-Path $repoRoot 'target\local-test\tauri-build'
$runtimeBundleRoot = Join-Path $repoRoot 'target\local-test\runtime-bundle'

if ($Detached) {
    New-Item -ItemType Directory -Path $localTaskRoot -Force | Out-Null

    $stdoutPath = Join-Path $localTaskRoot 'stdout.log'
    $stderrPath = Join-Path $localTaskRoot 'stderr.log'
    $taskPath = Join-Path $localTaskRoot 'task.pid'
    $powershellPath = (Get-Process -Id $PID).Path

    $task = Start-Process `
        -FilePath $powershellPath `
        -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $PSCommandPath) `
        -WorkingDirectory $repoRoot `
        -WindowStyle Hidden `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath `
        -PassThru

    Set-Content -LiteralPath $taskPath -Value $task.Id
    Write-Output 'Glyphshift desktop task started.'
    exit 0
}

$dependencySentinels = @(
    (Join-Path $desktopRoot 'node_modules\.bin\vite.cmd'),
    (Join-Path $desktopRoot 'node_modules\.bin\vue-tsc.cmd'),
    (Join-Path $desktopRoot 'node_modules\@nuxt\ui\package.json'),
    (Join-Path $desktopRoot 'node_modules\@nuxt\ui\dist\runtime\components\App.vue'),
    (Join-Path $desktopRoot 'node_modules\@nuxt\ui\dist\runtime\components\Checkbox.vue'),
    (Join-Path $desktopRoot 'node_modules\@nuxt\ui\dist\runtime\components\Modal.vue'),
    (Join-Path $desktopRoot 'node_modules\@nuxt\ui\dist\runtime\components\Select.vue'),
    (Join-Path $desktopRoot 'node_modules\@nuxt\ui\dist\runtime\components\Switch.vue'),
    (Join-Path $desktopRoot 'node_modules\@tauri-apps\cli\package.json')
)
$dependenciesNeedRepair = @(
    $dependencySentinels | Where-Object { -not (Test-Path -LiteralPath $_) }
).Count -gt 0

$developmentAppPath = [System.IO.Path]::GetFullPath(
    (Join-Path $cargoTargetDir 'debug\glyphshift-desktop-shell.exe')
)
$developmentApps = @(Get-CimInstance Win32_Process | Where-Object {
    $_.Name -eq 'glyphshift-desktop-shell.exe' `
        -and $_.ExecutablePath `
        -and [System.IO.Path]::GetFullPath($_.ExecutablePath).Equals(
            $developmentAppPath,
            [System.StringComparison]::OrdinalIgnoreCase
        )
})
if ($developmentApps.Count -gt 0) {
    Write-Output 'Stopping the stale Glyphshift development window...'
    $developmentApps | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
}

$listener = $null
for ($attempt = 0; $attempt -lt 20; $attempt++) {
    $listener = Get-NetTCPConnection -LocalPort 1430 -State Listen -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($null -eq $listener) {
        break
    }
    Start-Sleep -Milliseconds 250
}

if ($null -ne $listener) {
    $listenerProcess = Get-CimInstance Win32_Process -Filter "ProcessId = $($listener.OwningProcess)"
    $commandLine = [string]$listenerProcess.CommandLine
    $isRepoVite = $commandLine.IndexOf(
        $desktopRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    ) -ge 0 -and $commandLine.IndexOf(
        'vite',
        [System.StringComparison]::OrdinalIgnoreCase
    ) -ge 0

    if (-not $isRepoVite) {
        throw 'Port 1430 is occupied by another process.'
    }

    Write-Output 'Stopping the stale Glyphshift development server...'
    Stop-Process -Id $listener.OwningProcess -Force
}

if ($dependenciesNeedRepair) {
    Write-Output 'Repairing the V2 desktop dependency tree...'
    Push-Location $desktopRoot
    try {
        & npm.cmd ci
        if ($LASTEXITCODE -ne 0) {
            throw 'The V2 desktop dependency repair failed.'
        }
    }
    finally {
        Pop-Location
    }
}

New-Item -ItemType Directory -Path $cargoTargetDir -Force | Out-Null
$env:CARGO_TARGET_DIR = $cargoTargetDir

Write-Output 'Building the local target-process Runtime bundle...'
& cargo build `
    --manifest-path (Join-Path $repoRoot 'Cargo.toml') `
    -p glyphshift-controller-windows `
    -p glyphshift-target-runtime `
    -p glyphshift-adapter-gdi-native `
    -p glyphshift-adapter-gdiplus-native `
    -p glyphshift-windows-runtime-target
if ($LASTEXITCODE -ne 0) {
    throw 'The local target-process Runtime bundle did not build.'
}

New-Item -ItemType Directory -Path $runtimeBundleRoot -Force | Out-Null
function Copy-VersionedBundleArtifact(
    [string]$SourceName,
    [string]$TargetStem,
    [string]$Extension
) {
    $sourcePath = Join-Path $cargoTargetDir "debug\$SourceName"
    $hash = (Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash.ToLowerInvariant()
    $targetName = "$TargetStem-$($hash.Substring(0, 12)).$Extension"
    $targetPath = Join-Path $runtimeBundleRoot $targetName
    if (Test-Path -LiteralPath $targetPath) {
        $existingHash = (Get-FileHash -LiteralPath $targetPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($existingHash -eq $hash) {
            return [ordered]@{ file = $targetName; sha256 = $hash }
        }
    }
    Copy-Item `
        -LiteralPath $sourcePath `
        -Destination $targetPath `
        -Force
    return [ordered]@{ file = $targetName; sha256 = $hash }
}

$controllerBundle = Copy-VersionedBundleArtifact `
    'glyphshift-controller-windows.exe' 'controller' 'exe'
$runtimeBundle = Copy-VersionedBundleArtifact `
    'glyphshift_target_runtime.dll' 'runtime' 'dll'
$gdiBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_gdi_native.dll' 'adapter-gdi' 'dll'
$gdiPlusBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_gdiplus_native.dll' 'adapter-gdiplus' 'dll'
Copy-Item `
    -LiteralPath (Join-Path $cargoTargetDir 'debug\glyphshift-windows-runtime-target.exe') `
    -Destination (Join-Path $runtimeBundleRoot 'test-target.exe') `
    -Force

$runtimeManifest = [ordered]@{
    schema = 'glyphshift.runtime-bundle/1'
    signer = 'glyphshift.first-party.local'
    controller = [ordered]@{
        artifact = 'windows-generic-controller'
        file = $controllerBundle.file
        sha256 = $controllerBundle.sha256
        protocol = @(1, 0)
    }
    runtime = [ordered]@{
        file = $runtimeBundle.file
        sha256 = $runtimeBundle.sha256
    }
    adapters = @(
        [ordered]@{
            file = $gdiBundle.file
            sha256 = $gdiBundle.sha256
            label = 'GDI'
        },
        [ordered]@{
            file = $gdiPlusBundle.file
            sha256 = $gdiPlusBundle.sha256
            label = 'GDI+'
        }
    )
}
$runtimeManifestJson = $runtimeManifest | ConvertTo-Json -Depth 6
$runtimeManifestPath = Join-Path $runtimeBundleRoot 'runtime-bundle.json'
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($runtimeManifestPath, $runtimeManifestJson, $utf8WithoutBom)
$env:GLYPHSHIFT_RUNTIME_ROOT = $runtimeBundleRoot

Write-Output 'Verifying the desktop Runtime against isolated target processes...'
& cargo test `
    --manifest-path (Join-Path $repoRoot 'Cargo.toml') `
    -p glyphshift-target-runtime `
    --test target_runtime_contract `
    trh_001_runs_a_real_native_adapter_from_publication_through_update_and_stop `
    -- `
    --ignored `
    --exact
if ($LASTEXITCODE -ne 0) {
    throw 'The target-process Runtime contract failed.'
}
$runtimeContracts = @(
    'desktop_runtime_changes_pixels_updates_and_restores_pass_through',
    'desktop_runtime_pool_keeps_two_software_active_and_isolates_stop',
    'desktop_runtime_refresh_reconnects_requested_features_after_target_restart'
)
foreach ($runtimeContract in $runtimeContracts) {
    & cargo test `
        --manifest-path (Join-Path $repoRoot 'Cargo.toml') `
        -p glyphshift-desktop-runtime `
        --test windows_runtime_contract `
        $runtimeContract `
        -- `
        --ignored `
        --exact
    if ($LASTEXITCODE -ne 0) {
        throw "The desktop Runtime contract failed: $runtimeContract"
    }
}

Push-Location $desktopRoot
try {
    & npm.cmd run tauri -- dev
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
