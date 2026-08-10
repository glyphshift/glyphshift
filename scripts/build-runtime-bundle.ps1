[CmdletBinding()]
param(
    [ValidateSet('Debug', 'Release')]
    [string]$Profile = 'Debug',

    [string]$OutputRoot,

    [string]$CargoTargetDir,

    [switch]$IncludeTestTarget,

    [switch]$KeepExistingOutput
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$localTestRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $repoRoot 'local-test')
)
if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
    $OutputRoot = Join-Path $localTestRoot "runtime-bundle\$($Profile.ToLowerInvariant())"
}
if ([string]::IsNullOrWhiteSpace($CargoTargetDir)) {
    $CargoTargetDir = Join-Path $localTestRoot 'runtime-build'
}
$OutputRoot = [System.IO.Path]::GetFullPath($OutputRoot)
$CargoTargetDir = [System.IO.Path]::GetFullPath($CargoTargetDir)

function Assert-LocalTestPath([string]$Candidate, [string]$Purpose) {
    $prefix = $localTestRoot.TrimEnd('\') + '\'
    if (-not $Candidate.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Purpose must stay below the repository local-test root."
    }
}

Assert-LocalTestPath $OutputRoot 'Runtime Bundle output'
Assert-LocalTestPath $CargoTargetDir 'Cargo target directory'

$manifestPath = Join-Path $repoRoot 'Cargo.toml'
$cargoArguments = @(
    'build',
    '--manifest-path', $manifestPath,
    '-p', 'glyphshift-controller-windows',
    '-p', 'glyphshift-target-runtime',
    '-p', 'glyphshift-adapter-draw-text-native',
    '-p', 'glyphshift-adapter-gdi-native',
    '-p', 'glyphshift-adapter-gdi-text-out-native',
    '-p', 'glyphshift-adapter-gdiplus-native',
    '-p', 'glyphshift-adapter-directwrite-native',
    '-p', 'glyphshift-adapter-gtk3-pango-native',
    '-p', 'glyphshift-adapter-qt-painter-native',
    '-p', 'glyphshift-adapter-raylib-native',
    '-p', 'glyphshift-adapter-unity-mono-standard-ui-native'
)
if ($IncludeTestTarget) {
    $cargoArguments += @('-p', 'glyphshift-windows-runtime-target')
}
if ($Profile -eq 'Release') {
    $cargoArguments += '--release'
}

New-Item -ItemType Directory -Path $CargoTargetDir -Force | Out-Null
$env:CARGO_TARGET_DIR = $CargoTargetDir
Write-Output "Building the $Profile target-process Runtime bundle..."
& cargo @cargoArguments
if ($LASTEXITCODE -ne 0) {
    throw "The $Profile target-process Runtime bundle did not build."
}

$profileDirectory = $Profile.ToLowerInvariant()
$stagingRoot = "$OutputRoot.staging"
Assert-LocalTestPath $stagingRoot 'Runtime Bundle staging output'
if (Test-Path -LiteralPath $stagingRoot) {
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingRoot -Force | Out-Null

function Copy-VersionedBundleArtifact(
    [string]$SourceName,
    [string]$TargetStem,
    [string]$Extension
) {
    $sourcePath = Join-Path $CargoTargetDir "$profileDirectory\$SourceName"
    if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
        throw "Missing Runtime Bundle artifact: $SourceName"
    }
    $hash = (Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash.ToLowerInvariant()
    $targetName = "$TargetStem-$($hash.Substring(0, 12)).$Extension"
    Copy-Item -LiteralPath $sourcePath -Destination (Join-Path $stagingRoot $targetName)
    return [ordered]@{ file = $targetName; sha256 = $hash }
}

$controllerBundle = Copy-VersionedBundleArtifact `
    'glyphshift-controller-windows.exe' 'controller' 'exe'
$runtimeBundle = Copy-VersionedBundleArtifact `
    'glyphshift_target_runtime.dll' 'runtime' 'dll'
$gdiBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_gdi_native.dll' 'adapter-gdi' 'dll'
$textOutBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_gdi_text_out_native.dll' 'adapter-gdi-text-out' 'dll'
$drawTextBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_draw_text_native.dll' 'adapter-draw-text' 'dll'
$gdiPlusBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_gdiplus_native.dll' 'adapter-gdiplus' 'dll'
$directWriteBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_directwrite_native.dll' 'adapter-directwrite-text-layout' 'dll'
$gtk3PangoBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_gtk3_pango_native.dll' 'adapter-gtk3-pango' 'dll'
$qtPainterBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_qt_painter_native.dll' 'adapter-qt-painter' 'dll'
$raylibBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_raylib_native.dll' 'adapter-raylib' 'dll'
$unityMonoStandardUiBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_unity_mono_standard_ui_native.dll' 'adapter-unity-mono-standard-ui' 'dll'

if ($IncludeTestTarget) {
    $testTarget = Join-Path $CargoTargetDir "$profileDirectory\glyphshift-windows-runtime-target.exe"
    Copy-Item -LiteralPath $testTarget -Destination (Join-Path $stagingRoot 'test-target.exe')
}

$adapterPresentationPath = Join-Path $PSScriptRoot 'runtime-bundle-adapters.zh-CN.json'
$adapterPresentationJson = [System.IO.File]::ReadAllText(
    $adapterPresentationPath,
    [System.Text.Encoding]::UTF8
)
$adapterPresentation = $adapterPresentationJson | ConvertFrom-Json
function Get-AdapterPresentation([string]$AdapterId) {
    $presentation = $adapterPresentation |
        Where-Object { $_.id -eq $AdapterId } |
        Select-Object -First 1
    if ($null -eq $presentation) {
        throw "Missing Runtime Adapter presentation for $AdapterId"
    }
    try {
        $documentationUri = [System.Uri]$presentation.documentationUrl
    }
    catch {
        throw "Invalid Runtime Adapter documentation URL for $AdapterId"
    }
    if (-not $documentationUri.IsAbsoluteUri -or $documentationUri.Scheme -ne 'https') {
        throw "Runtime Adapter documentation URL must use HTTPS for $AdapterId"
    }
    return $presentation
}

$extTextOutPresentation = Get-AdapterPresentation 'windows.gdi.ext-text-out'
$textOutPresentation = Get-AdapterPresentation 'windows.gdi.text-out'
$drawTextPresentation = Get-AdapterPresentation 'windows.user32.draw-text'
$gdiPlusPresentation = Get-AdapterPresentation 'windows.gdiplus.draw-string'
$directWritePresentation = Get-AdapterPresentation 'windows.directwrite.text-layout'
$gtk3PangoPresentation = Get-AdapterPresentation 'windows.gtk3.pango-render-layout'
$qtPainterPresentation = Get-AdapterPresentation 'windows.qt.painter-draw-text'
$raylibPresentation = Get-AdapterPresentation 'windows.raylib.draw-text-ex'
$unityMonoStandardUiPresentation = Get-AdapterPresentation 'windows.unity.mono.standard-ui'

$runtimeManifest = [ordered]@{
    schema = 'glyphshift.runtime-bundle/3'
    authority = 'app.glyphshift.runtime.first-party'
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
            name = $extTextOutPresentation.name
            summary = $extTextOutPresentation.summary
            technology = $extTextOutPresentation.technology
            technicalTarget = $extTextOutPresentation.technicalTarget
            documentationUrl = $extTextOutPresentation.documentationUrl
        },
        [ordered]@{
            file = $textOutBundle.file
            sha256 = $textOutBundle.sha256
            name = $textOutPresentation.name
            summary = $textOutPresentation.summary
            technology = $textOutPresentation.technology
            technicalTarget = $textOutPresentation.technicalTarget
            documentationUrl = $textOutPresentation.documentationUrl
        },
        [ordered]@{
            file = $drawTextBundle.file
            sha256 = $drawTextBundle.sha256
            name = $drawTextPresentation.name
            summary = $drawTextPresentation.summary
            technology = $drawTextPresentation.technology
            technicalTarget = $drawTextPresentation.technicalTarget
            documentationUrl = $drawTextPresentation.documentationUrl
        },
        [ordered]@{
            file = $gdiPlusBundle.file
            sha256 = $gdiPlusBundle.sha256
            name = $gdiPlusPresentation.name
            summary = $gdiPlusPresentation.summary
            technology = $gdiPlusPresentation.technology
            technicalTarget = $gdiPlusPresentation.technicalTarget
            documentationUrl = $gdiPlusPresentation.documentationUrl
        },
        [ordered]@{
            file = $directWriteBundle.file
            sha256 = $directWriteBundle.sha256
            name = $directWritePresentation.name
            summary = $directWritePresentation.summary
            technology = $directWritePresentation.technology
            technicalTarget = $directWritePresentation.technicalTarget
            documentationUrl = $directWritePresentation.documentationUrl
        },
        [ordered]@{
            file = $gtk3PangoBundle.file
            sha256 = $gtk3PangoBundle.sha256
            name = $gtk3PangoPresentation.name
            summary = $gtk3PangoPresentation.summary
            technology = $gtk3PangoPresentation.technology
            technicalTarget = $gtk3PangoPresentation.technicalTarget
            documentationUrl = $gtk3PangoPresentation.documentationUrl
        },
        [ordered]@{
            file = $qtPainterBundle.file
            sha256 = $qtPainterBundle.sha256
            name = $qtPainterPresentation.name
            summary = $qtPainterPresentation.summary
            technology = $qtPainterPresentation.technology
            technicalTarget = $qtPainterPresentation.technicalTarget
            documentationUrl = $qtPainterPresentation.documentationUrl
        },
        [ordered]@{
            file = $raylibBundle.file
            sha256 = $raylibBundle.sha256
            name = $raylibPresentation.name
            summary = $raylibPresentation.summary
            technology = $raylibPresentation.technology
            technicalTarget = $raylibPresentation.technicalTarget
            documentationUrl = $raylibPresentation.documentationUrl
        },
        [ordered]@{
            file = $unityMonoStandardUiBundle.file
            sha256 = $unityMonoStandardUiBundle.sha256
            name = $unityMonoStandardUiPresentation.name
            summary = $unityMonoStandardUiPresentation.summary
            technology = $unityMonoStandardUiPresentation.technology
            technicalTarget = $unityMonoStandardUiPresentation.technicalTarget
            documentationUrl = $unityMonoStandardUiPresentation.documentationUrl
        }
    )
    isolated_workers = @()
    acquisition_workers = @()
}
$runtimeManifestJson = $runtimeManifest | ConvertTo-Json -Depth 6
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText(
    (Join-Path $stagingRoot 'runtime-bundle.json'),
    $runtimeManifestJson,
    $utf8WithoutBom
)

$declaredArtifacts = @(
    $runtimeManifest.controller,
    $runtimeManifest.runtime
) + @($runtimeManifest.adapters) + @($runtimeManifest.isolated_workers)
$expectedFiles = @('runtime-bundle.json') + @($declaredArtifacts | ForEach-Object { $_.file })
if ($IncludeTestTarget) {
    $expectedFiles += 'test-target.exe'
}
$actualFiles = @(Get-ChildItem -LiteralPath $stagingRoot -File | ForEach-Object { $_.Name })
$unexpectedFiles = @(Compare-Object $expectedFiles $actualFiles)
if ($unexpectedFiles.Count -gt 0) {
    throw 'Runtime Bundle staging does not match its declared artifact set.'
}
foreach ($artifact in $declaredArtifacts) {
    $actualHash = (
        Get-FileHash -LiteralPath (Join-Path $stagingRoot $artifact.file) -Algorithm SHA256
    ).Hash.ToLowerInvariant()
    if ($actualHash -ne $artifact.sha256) {
        throw "Runtime Bundle staging hash mismatch: $($artifact.file)"
    }
}

if ((-not $KeepExistingOutput) -and (Test-Path -LiteralPath $OutputRoot)) {
    $stagedFiles = @(Get-ChildItem -LiteralPath $stagingRoot -File)
    $existingFiles = @(Get-ChildItem -LiteralPath $OutputRoot -File)
    $sameFiles = $stagedFiles.Count -eq $existingFiles.Count
    if ($sameFiles) {
        foreach ($stagedFile in $stagedFiles) {
            $existingFile = Join-Path $OutputRoot $stagedFile.Name
            if (-not (Test-Path -LiteralPath $existingFile -PathType Leaf)) {
                $sameFiles = $false
                break
            }
            $stagedHash = (Get-FileHash -LiteralPath $stagedFile.FullName -Algorithm SHA256).Hash
            $existingHash = (Get-FileHash -LiteralPath $existingFile -Algorithm SHA256).Hash
            if ($stagedHash -ne $existingHash) {
                $sameFiles = $false
                break
            }
        }
    }
    if ($sameFiles) {
        Remove-Item -LiteralPath $stagingRoot -Recurse -Force
        Write-Output "Runtime Bundle unchanged: $OutputRoot"
        return
    }
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force
    throw 'Runtime Bundle output already contains a different generation. Choose a new output directory.'
}

if ($KeepExistingOutput) {
    New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
    Get-ChildItem -LiteralPath $stagingRoot -File | ForEach-Object {
        $destination = Join-Path $OutputRoot $_.Name
        $copyRequired = $true
        if (Test-Path -LiteralPath $destination -PathType Leaf) {
            $stagedHash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
            $existingHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
            if ($stagedHash -eq $existingHash) {
                $copyRequired = $false
            }
        }
        if ($copyRequired) {
            Copy-Item -LiteralPath $_.FullName -Destination $destination -Force
        }
    }
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force
}
else {
    Move-Item -LiteralPath $stagingRoot -Destination $OutputRoot
}
Write-Output "Runtime Bundle ready: $OutputRoot"
