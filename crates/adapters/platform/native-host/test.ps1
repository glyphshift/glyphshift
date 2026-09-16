[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$workspaceManifest = Join-Path $PSScriptRoot '..\..\..\..\Cargo.toml'

& cargo build `
    --manifest-path $workspaceManifest `
    -p glyphshift-adapter-console-native `
    -p glyphshift-adapter-direct2d-native `
    -p glyphshift-adapter-directwrite-native `
    -p glyphshift-adapter-draw-text-native `
    -p glyphshift-adapter-gdi-native `
    -p glyphshift-adapter-gdi-text-out-native `
    -p glyphshift-adapter-gdiplus-native `
    -p glyphshift-adapter-gtk3-pango-native `
    -p glyphshift-adapter-qt-painter-native `
    -p glyphshift-adapter-qt-translation-native `
    -p glyphshift-adapter-qt-text-document-native `
    -p glyphshift-adapter-raylib-native `
    -p glyphshift-adapter-sidefx-cv-paint-buffer-native `
    -p glyphshift-adapter-unity-mono-standard-ui-native
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter package build failed with exit code $LASTEXITCODE"
}

& cargo test `
    --manifest-path $workspaceManifest `
    -p glyphshift-adapter-native-host
if ($LASTEXITCODE -ne 0) {
    throw "native Adapter host contract failed with exit code $LASTEXITCODE"
}
