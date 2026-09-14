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
. (Join-Path $PSScriptRoot 'cargo-target.ps1')
$CargoTargetDir = Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot -Override $CargoTargetDir
$OutputRoot = [System.IO.Path]::GetFullPath($OutputRoot)
$CargoTargetDir = [System.IO.Path]::GetFullPath($CargoTargetDir)

function Assert-LocalTestPath([string]$Candidate, [string]$Purpose) {
    $prefix = $localTestRoot.TrimEnd('\') + '\'
    if (-not $Candidate.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "$Purpose must stay below the repository local-test root."
    }
}

Assert-LocalTestPath $OutputRoot 'Runtime Bundle output'

$manifestPath = Join-Path $repoRoot 'Cargo.toml'
$cargoArguments = @(
    'build',
    '--manifest-path', $manifestPath,
    '--target-dir', $CargoTargetDir,
    '-p', 'glyphshift-controller-windows',
    '-p', 'glyphshift-target-runtime',
    '-p', 'glyphshift-adapter-draw-text-native',
    '-p', 'glyphshift-adapter-gdi-native',
    '-p', 'glyphshift-adapter-gdi-text-out-native',
    '-p', 'glyphshift-adapter-gdiplus-native',
    '-p', 'glyphshift-adapter-directwrite-native',
    '-p', 'glyphshift-adapter-gtk3-pango-native',
    '-p', 'glyphshift-adapter-qt-painter-native',
    '-p', 'glyphshift-adapter-qt-text-document-native',
    '-p', 'glyphshift-adapter-qt-quick-native',
    '-p', 'glyphshift-adapter-raylib-native',
    '-p', 'glyphshift-adapter-sidefx-cv-paint-buffer-native',
    '-p', 'glyphshift-adapter-unity-mono-standard-ui-native',
    '-p', 'glyphshift-adapter-unity-il2cpp-standard-ui-native'
)
if ($IncludeTestTarget) {
    $cargoArguments += @('-p', 'glyphshift-windows-runtime-target')
}
if ($Profile -eq 'Release') {
    $cargoArguments += '--release'
}

New-Item -ItemType Directory -Path $CargoTargetDir -Force | Out-Null
Write-Output "Building the $Profile target-process Runtime bundle..."
& cargo @cargoArguments
if ($LASTEXITCODE -ne 0) {
    throw "The $Profile target-process Runtime bundle did not build."
}

$verifierArguments = @(
    'build',
    '--manifest-path', $manifestPath,
    '--target-dir', $CargoTargetDir,
    '-p', 'glyphshift-desktop-runtime',
    '--bin', 'glyphshift-runtime-bundle-verify'
)
if ($Profile -eq 'Release') {
    $verifierArguments += '--release'
}
& cargo @verifierArguments
if ($LASTEXITCODE -ne 0) {
    throw "The $Profile Runtime Bundle verifier did not build."
}

$x86Arguments = @('build', '--manifest-path', $manifestPath, '--target-dir', $CargoTargetDir,
    '--target', 'i686-pc-windows-msvc', '-p', 'glyphshift-controller-windows', '-p', 'glyphshift-target-runtime',
    '-p', 'glyphshift-adapter-gdi-native', '-p', 'glyphshift-adapter-gdi-text-out-native', '-p', 'glyphshift-adapter-draw-text-native',
    '-p', 'glyphshift-adapter-gdiplus-native', '-p', 'glyphshift-adapter-directwrite-native',
    '-p', 'glyphshift-adapter-gtk3-pango-native', '-p', 'glyphshift-adapter-raylib-native',
    '-p', 'glyphshift-adapter-qt-painter-native', '-p', 'glyphshift-adapter-unity-mono-standard-ui-native',
    '-p', 'glyphshift-adapter-vgui-localize-native', '-p', 'glyphshift-adapter-catsystem2-native')
if ($IncludeTestTarget) { $x86Arguments += @('-p', 'glyphshift-windows-runtime-target') }
if ($Profile -eq 'Release') { $x86Arguments += '--release' }
& cargo @x86Arguments
if ($LASTEXITCODE -ne 0) { throw 'The x86 Runtime components did not build.' }

$profileDirectory = $Profile.ToLowerInvariant()
$x86ProfileRoot = Join-Path $CargoTargetDir "i686-pc-windows-msvc\$profileDirectory"
& (Join-Path $PSScriptRoot 'build-monogame-native.ps1') -OutputRoot (Join-Path $CargoTargetDir $profileDirectory)
& (Join-Path $PSScriptRoot 'build-monogame-native.ps1') -Architecture x86 -OutputRoot $x86ProfileRoot
$stagingRoot = "$OutputRoot.staging"
Assert-LocalTestPath $stagingRoot 'Runtime Bundle staging output'
if (Test-Path -LiteralPath $stagingRoot) {
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingRoot -Force | Out-Null

function Copy-VersionedBundleArtifact(
    [string]$SourceName,
    [string]$TargetStem,
    [string]$Extension,
    [string]$SourceRoot = (Join-Path $CargoTargetDir $profileDirectory)
) {
    $sourcePath = Join-Path $SourceRoot $SourceName
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
$qtTextDocumentBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_qt_text_document_native.dll' 'adapter-qt-text-document' 'dll'
$qtQuickBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_qt_quick_native.dll' 'adapter-qt-quick' 'dll'
$raylibBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_raylib_native.dll' 'adapter-raylib' 'dll'
$sidefxCvPaintBufferBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_sidefx_cv_paint_buffer_native.dll' 'adapter-sidefx-cv-paint-buffer' 'dll'
$unityMonoStandardUiBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_unity_mono_standard_ui_native.dll' 'adapter-unity-mono-standard-ui' 'dll'
$unityIl2CppStandardUiBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_unity_il2cpp_standard_ui_native.dll' 'adapter-unity-il2cpp-standard-ui' 'dll'
$monoGameBundle = Copy-VersionedBundleArtifact `
    'glyphshift_adapter_monogame_native.dll' 'adapter-monogame' 'dll'

$x86Controller = Copy-VersionedBundleArtifact 'glyphshift-controller-windows.exe' 'controller-x86' 'exe' $x86ProfileRoot
$x86Runtime = Copy-VersionedBundleArtifact 'glyphshift_target_runtime.dll' 'runtime-x86' 'dll' $x86ProfileRoot
$x86Gdi = Copy-VersionedBundleArtifact 'glyphshift_adapter_gdi_native.dll' 'adapter-gdi-x86' 'dll' $x86ProfileRoot
$x86TextOut = Copy-VersionedBundleArtifact 'glyphshift_adapter_gdi_text_out_native.dll' 'adapter-gdi-text-out-x86' 'dll' $x86ProfileRoot
$x86DrawText = Copy-VersionedBundleArtifact 'glyphshift_adapter_draw_text_native.dll' 'adapter-draw-text-x86' 'dll' $x86ProfileRoot
$x86GdiPlus = Copy-VersionedBundleArtifact 'glyphshift_adapter_gdiplus_native.dll' 'adapter-gdiplus-x86' 'dll' $x86ProfileRoot
$x86DirectWrite = Copy-VersionedBundleArtifact 'glyphshift_adapter_directwrite_native.dll' 'adapter-directwrite-x86' 'dll' $x86ProfileRoot
$x86Gtk = Copy-VersionedBundleArtifact 'glyphshift_adapter_gtk3_pango_native.dll' 'adapter-gtk3-pango-x86' 'dll' $x86ProfileRoot
$x86QtPainter = Copy-VersionedBundleArtifact 'glyphshift_adapter_qt_painter_native.dll' 'adapter-qt-painter-x86' 'dll' $x86ProfileRoot
$x86Raylib = Copy-VersionedBundleArtifact 'glyphshift_adapter_raylib_native.dll' 'adapter-raylib-x86' 'dll' $x86ProfileRoot
$x86Unity = Copy-VersionedBundleArtifact 'glyphshift_adapter_unity_mono_standard_ui_native.dll' 'adapter-unity-mono-x86' 'dll' $x86ProfileRoot
$x86MonoGame = Copy-VersionedBundleArtifact 'glyphshift_adapter_monogame_native.dll' 'adapter-monogame-x86' 'dll' $x86ProfileRoot
$x86VguiLocalize = Copy-VersionedBundleArtifact 'glyphshift_adapter_vgui_localize_native.dll' 'adapter-vgui-localize-x86' 'dll' $x86ProfileRoot
$x86CatSystem2 = Copy-VersionedBundleArtifact 'glyphshift_adapter_catsystem2_native.dll' 'adapter-catsystem2-x86' 'dll' $x86ProfileRoot

if ($IncludeTestTarget) {
    $testTarget = Join-Path $CargoTargetDir "$profileDirectory\glyphshift-windows-runtime-target.exe"
    Copy-Item -LiteralPath $testTarget -Destination (Join-Path $stagingRoot 'test-target.exe')
    Copy-Item -LiteralPath (Join-Path $x86ProfileRoot 'glyphshift-windows-runtime-target.exe') -Destination (Join-Path $stagingRoot 'test-target-x86.exe')
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
$qtTextDocumentPresentation = Get-AdapterPresentation 'windows.qt.text-document'
$qtQuickPresentation = Get-AdapterPresentation 'windows.qt.quick-text'
$raylibPresentation = Get-AdapterPresentation 'windows.raylib.draw-text-ex'
$sidefxCvPaintBufferPresentation = Get-AdapterPresentation 'windows.sidefx.cv-paint-buffer-text'
$unityMonoStandardUiPresentation = Get-AdapterPresentation 'windows.unity.mono.standard-ui'
$unityIl2CppStandardUiPresentation = Get-AdapterPresentation 'windows.unity.il2cpp.standard-ui'
$monoGamePresentation = Get-AdapterPresentation 'windows.monogame.sprite-batch-draw-string'
$vguiLocalizePresentation = Get-AdapterPresentation 'windows.vgui.localize-query'
$catSystem2Presentation = Get-AdapterPresentation 'windows.catsystem2.utf8-text'

$runtimeManifest = [ordered]@{
    schema = 'glyphshift.runtime-bundle/4'
    architecture = 'x86_64'
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
            file = $monoGameBundle.file
            sha256 = $monoGameBundle.sha256
            name = $monoGamePresentation.name
            summary = $monoGamePresentation.summary
            technology = $monoGamePresentation.technology
            technicalTarget = $monoGamePresentation.technicalTarget
            documentationUrl = $monoGamePresentation.documentationUrl
            process_resident_after_deactivate = $true
        },
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
            file = $qtTextDocumentBundle.file
            sha256 = $qtTextDocumentBundle.sha256
            name = $qtTextDocumentPresentation.name
            summary = $qtTextDocumentPresentation.summary
            technology = $qtTextDocumentPresentation.technology
            technicalTarget = $qtTextDocumentPresentation.technicalTarget
            documentationUrl = $qtTextDocumentPresentation.documentationUrl
            process_resident_after_deactivate = $true
        },
        [ordered]@{
            file = $qtQuickBundle.file
            sha256 = $qtQuickBundle.sha256
            name = $qtQuickPresentation.name
            summary = $qtQuickPresentation.summary
            technology = $qtQuickPresentation.technology
            technicalTarget = $qtQuickPresentation.technicalTarget
            documentationUrl = $qtQuickPresentation.documentationUrl
            process_resident_after_deactivate = $true
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
            file = $sidefxCvPaintBufferBundle.file
            sha256 = $sidefxCvPaintBufferBundle.sha256
            name = $sidefxCvPaintBufferPresentation.name
            summary = $sidefxCvPaintBufferPresentation.summary
            technology = $sidefxCvPaintBufferPresentation.technology
            technicalTarget = $sidefxCvPaintBufferPresentation.technicalTarget
            documentationUrl = $sidefxCvPaintBufferPresentation.documentationUrl
        },
        [ordered]@{
            file = $unityMonoStandardUiBundle.file
            sha256 = $unityMonoStandardUiBundle.sha256
            name = $unityMonoStandardUiPresentation.name
            summary = $unityMonoStandardUiPresentation.summary
            technology = $unityMonoStandardUiPresentation.technology
            technicalTarget = $unityMonoStandardUiPresentation.technicalTarget
            documentationUrl = $unityMonoStandardUiPresentation.documentationUrl
        },
        [ordered]@{
            file = $unityIl2CppStandardUiBundle.file
            sha256 = $unityIl2CppStandardUiBundle.sha256
            name = $unityIl2CppStandardUiPresentation.name
            summary = $unityIl2CppStandardUiPresentation.summary
            technology = $unityIl2CppStandardUiPresentation.technology
            technicalTarget = $unityIl2CppStandardUiPresentation.technicalTarget
            documentationUrl = $unityIl2CppStandardUiPresentation.documentationUrl
        }
    )
    isolated_workers = @()
    acquisition_workers = @()
}
$x86Adapters = @()
foreach ($pair in @(@($x86Gdi, $extTextOutPresentation), @($x86TextOut, $textOutPresentation), @($x86DrawText, $drawTextPresentation),
    @($x86GdiPlus, $gdiPlusPresentation), @($x86DirectWrite, $directWritePresentation), @($x86Gtk, $gtk3PangoPresentation),
    @($x86QtPainter, $qtPainterPresentation), @($x86Raylib, $raylibPresentation),
    @($x86Unity, $unityMonoStandardUiPresentation), @($x86MonoGame, $monoGamePresentation),
    @($x86VguiLocalize, $vguiLocalizePresentation), @($x86CatSystem2, $catSystem2Presentation))) {
    $artifact = $pair[0]; $presentation = $pair[1]
    $x86Adapters += [ordered]@{ file=$artifact.file; sha256=$artifact.sha256; name=$presentation.name;
        summary=$presentation.summary; technology=$presentation.technology; technicalTarget=$presentation.technicalTarget;
        documentationUrl=$presentation.documentationUrl }
    if ($artifact.file -eq $x86MonoGame.file) { $x86Adapters[-1].process_resident_after_deactivate = $true }
    if ($artifact.file -eq $x86VguiLocalize.file) { $x86Adapters[-1].process_resident_after_deactivate = $true }
    if ($artifact.file -eq $x86CatSystem2.file) { $x86Adapters[-1].process_resident_after_deactivate = $true }
}
$runtimeManifest.additional_architectures = @([ordered]@{
    architecture = 'x86'
    controller = [ordered]@{artifact='windows-generic-controller-x86';file=$x86Controller.file;sha256=$x86Controller.sha256;protocol=@(1,0)}
    runtime = $x86Runtime
    adapters = $x86Adapters
})
# Produce descriptors with an inspector compiled for the same architecture.
# The x64 App reads this bounded metadata; the target loader revalidates the native ABI.
foreach ($group in @(@($runtimeManifest.adapters, $controllerBundle.file), @($x86Adapters, $x86Controller.file))) {
    $inspector = Join-Path $stagingRoot $group[1]
    foreach ($adapter in $group[0]) {
        $metadata = & $inspector --inspect-adapter (Join-Path $stagingRoot $adapter.file)
        if ($LASTEXITCODE -ne 0) { throw "Native descriptor inspection failed: $($adapter.file)" }
        $adapter.native_metadata = $metadata | ConvertFrom-Json
    }
}
$runtimeManifestJson = $runtimeManifest | ConvertTo-Json -Depth 12
$utf8WithoutBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText(
    (Join-Path $stagingRoot 'runtime-bundle.json'),
    $runtimeManifestJson,
    $utf8WithoutBom
)

$declaredArtifacts = @(
    $runtimeManifest.controller,
    $runtimeManifest.runtime
) + @($runtimeManifest.adapters) + @($runtimeManifest.isolated_workers) + @($x86Controller, $x86Runtime) + @($x86Adapters)
$expectedFiles = @('runtime-bundle.json') + @($declaredArtifacts | ForEach-Object { $_.file })
if ($IncludeTestTarget) {
    $expectedFiles += @('test-target.exe', 'test-target-x86.exe')
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

$verifierPath = Join-Path $CargoTargetDir "$profileDirectory\glyphshift-runtime-bundle-verify.exe"
if (-not (Test-Path -LiteralPath $verifierPath -PathType Leaf)) {
    throw 'Runtime Bundle verifier executable is missing.'
}
& $verifierPath $stagingRoot
if ($LASTEXITCODE -ne 0) {
    throw 'Runtime Bundle failed real-loader verification.'
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
    New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
    Get-ChildItem -LiteralPath $stagingRoot -File |
        Where-Object { $_.Name -ne 'runtime-bundle.json' } |
        ForEach-Object {
            $destination = Join-Path $OutputRoot $_.Name
            if (-not (Test-Path -LiteralPath $destination -PathType Leaf)) {
                Copy-Item -LiteralPath $_.FullName -Destination $destination
            }
        }
    $nextManifestPath = Join-Path $OutputRoot 'runtime-bundle.json.next'
    Copy-Item `
        -LiteralPath (Join-Path $stagingRoot 'runtime-bundle.json') `
        -Destination $nextManifestPath `
        -Force
    [System.IO.File]::Move(
        $nextManifestPath,
        (Join-Path $OutputRoot 'runtime-bundle.json'),
        $true
    )
    Remove-Item -LiteralPath $stagingRoot -Recurse -Force
    Write-Output "Runtime Bundle updated: $OutputRoot"
    return
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
