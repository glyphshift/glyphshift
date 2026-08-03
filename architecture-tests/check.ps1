[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$workspaceRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $workspaceRoot 'Cargo.toml'

$metadataJson = & cargo metadata `
    --manifest-path $manifestPath `
    --format-version 1 `
    --no-deps
if ($LASTEXITCODE -ne 0) {
    throw "cargo metadata failed with exit code $LASTEXITCODE"
}

$metadata = $metadataJson | ConvertFrom-Json

function Assert-Dependencies {
    param(
        [Parameter(Mandatory)]
        [string] $PackageName,

        [Parameter(Mandatory)]
        [AllowEmptyCollection()]
        [string[]] $Expected
    )

    $package = $metadata.packages |
        Where-Object { $_.name -eq $PackageName } |
        Select-Object -First 1
    if ($null -eq $package) {
        throw "Missing workspace package: $PackageName"
    }

    $actual = @(
        $package.dependencies |
            Where-Object { $null -eq $_.kind -or $_.kind -eq 'normal' } |
            ForEach-Object { $_.name } |
            Sort-Object -Unique
    )
    $expectedSorted = @($Expected | Sort-Object -Unique)

    $difference = Compare-Object -ReferenceObject $expectedSorted -DifferenceObject $actual
    if ($difference) {
        $rendered = $difference |
            ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }
        throw "$PackageName dependency contract failed: $($rendered -join ', ')"
    }
}

Assert-Dependencies -PackageName 'glyphshift-domain' -Expected @()
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-registry' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-sdk' `
    -Expected @('glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-native-abi' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-native-host' `
    -Expected @(
        'glyphshift-adapter-native-abi',
        'glyphshift-adapter-sdk',
        'glyphshift-domain',
        'libloading'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-gdi' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-gdi-native' `
    -Expected @(
        'glyphshift-adapter-gdi',
        'glyphshift-adapter-gdi-native-support',
        'glyphshift-adapter-native-abi',
        'retour',
        'windows'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-gdi-native-support' `
    -Expected @('glyphshift-adapter-native-abi', 'windows')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-gdi-text-out-native' `
    -Expected @(
        'glyphshift-adapter-gdi',
        'glyphshift-adapter-gdi-native-support',
        'glyphshift-adapter-native-abi',
        'retour',
        'windows'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-draw-text-native' `
    -Expected @(
        'glyphshift-adapter-gdi',
        'glyphshift-adapter-gdi-native-support',
        'glyphshift-adapter-native-abi',
        'retour',
        'windows'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-gdiplus' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-gdiplus-native' `
    -Expected @('glyphshift-adapter-gdiplus', 'glyphshift-adapter-native-abi', 'retour', 'windows')
Assert-Dependencies `
    -PackageName 'glyphshift-controller-sdk' `
    -Expected @('serde', 'serde_json')
Assert-Dependencies `
    -PackageName 'glyphshift-controller-host' `
    -Expected @(
        'glyphshift-adapter-registry',
        'glyphshift-controller-sdk',
        'glyphshift-domain',
        'glyphshift-extension',
        'glyphshift-protocol',
        'serde_json',
        'sha2'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-controller-windows' `
    -Expected @(
        'glyphshift-controller-sdk',
        'glyphshift-runtime-contract',
        'glyphshift-target-runtime-contract',
        'sha2',
        'windows',
        'windows-sys'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-translation' `
    -Expected @('glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-decision' `
    -Expected @('glyphshift-domain', 'glyphshift-translation')
Assert-Dependencies `
    -PackageName 'glyphshift-extension' `
    -Expected @('glyphshift-adapter-registry', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-protocol' `
    -Expected @('glyphshift-adapter-registry', 'glyphshift-domain', 'glyphshift-extension')
Assert-Dependencies `
    -PackageName 'glyphshift-runtime-contract' `
    -Expected @('glyphshift-domain', 'glyphshift-translation', 'serde', 'serde_json', 'sha2')
Assert-Dependencies `
    -PackageName 'glyphshift-runtime-kernel' `
    -Expected @(
        'glyphshift-adapter-registry',
        'glyphshift-decision',
        'glyphshift-domain',
        'glyphshift-runtime-contract',
        'glyphshift-translation'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-target-runtime-contract' `
    -Expected @(
        'glyphshift-adapter-registry',
        'glyphshift-adapter-sdk',
        'glyphshift-capture',
        'glyphshift-decision',
        'glyphshift-domain',
        'glyphshift-runtime-contract',
        'serde',
        'serde_json'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-target-runtime' `
    -Expected @(
        'glyphshift-adapter-native-abi',
        'glyphshift-adapter-native-host',
        'glyphshift-adapter-registry',
        'glyphshift-capture',
        'glyphshift-domain',
        'glyphshift-runtime-contract',
        'glyphshift-runtime-kernel',
        'glyphshift-target-runtime-contract',
        'sha2',
        'windows-sys'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-target-process-host' `
    -Expected @(
        'glyphshift-adapter-registry',
        'glyphshift-capture',
        'glyphshift-domain',
        'glyphshift-protocol',
        'glyphshift-runtime-contract',
        'glyphshift-session',
        'glyphshift-target-runtime-contract'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-session' `
    -Expected @('glyphshift-adapter-registry', 'glyphshift-domain', 'glyphshift-runtime-contract')
Assert-Dependencies `
    -PackageName 'glyphshift-service' `
    -Expected @(
        'glyphshift-adapter-registry',
        'glyphshift-domain',
        'glyphshift-extension',
        'glyphshift-protocol',
        'glyphshift-runtime-contract',
        'glyphshift-session'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-reference-adapters' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-windows-host' `
    -Expected @(
        'glyphshift-adapter-gdi',
        'glyphshift-adapter-gdiplus',
        'glyphshift-adapter-sdk',
        'glyphshift-domain',
        'windows-sys'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-test-controller-plugin' `
    -Expected @('glyphshift-controller-sdk')
Assert-Dependencies `
    -PackageName 'glyphshift-dictionary-package' `
    -Expected @('serde', 'serde_json')
Assert-Dependencies `
    -PackageName 'glyphshift-dictionary-distribution' `
    -Expected @(
        'glyphshift-dictionary-package',
        'semver',
        'serde',
        'serde_json',
        'sha2',
        'subtle',
        'tempfile',
        'url'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-capture' `
    -Expected @('serde', 'serde_json')
Assert-Dependencies `
    -PackageName 'glyphshift-desktop-backend' `
    -Expected @(
        'glyphshift-adapter-registry',
        'glyphshift-dictionary-distribution',
        'glyphshift-dictionary-package',
        'glyphshift-domain',
        'glyphshift-runtime-contract',
        'glyphshift-translation',
        'glyphshift-workflow',
        'serde',
        'serde_json',
        'tempfile'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-desktop-runtime' `
    -Expected @(
        'glyphshift-adapter-native-host',
        'glyphshift-adapter-registry',
        'glyphshift-capture',
        'glyphshift-controller-host',
        'glyphshift-desktop-backend',
        'glyphshift-domain',
        'glyphshift-extension',
        'glyphshift-protocol',
        'glyphshift-runtime-contract',
        'glyphshift-session',
        'glyphshift-target-process-host',
        'serde',
        'serde_json',
        'sha2'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-desktop-shell' `
    -Expected @(
        'glyphshift-capture',
        'glyphshift-desktop-backend',
        'glyphshift-desktop-runtime',
        'glyphshift-dictionary-distribution',
        'glyphshift-domain',
        'glyphshift-runtime-contract',
        'glyphshift-translation',
        'glyphshift-workflow',
        'serde',
        'serde_json',
        'tauri',
        'tauri-plugin-dialog',
        'tempfile',
        'winreg'
    )

$productionSourceRoots = @(
    'crates/glyphshift-adapter-registry/src',
    'crates/glyphshift-adapter-sdk/src',
    'crates/glyphshift-adapter-native-abi/src',
    'crates/glyphshift-adapter-native-host/src',
    'crates/glyphshift-controller-sdk/src',
    'crates/glyphshift-controller-host/src',
    'crates/glyphshift-controller-windows/src',
    'crates/glyphshift-capture/src',
    'crates/glyphshift-desktop-backend/src',
    'crates/glyphshift-desktop-runtime/src',
    'crates/glyphshift-domain/src',
    'crates/glyphshift-extension/src',
    'crates/glyphshift-translation/src',
    'crates/glyphshift-decision/src',
    'crates/glyphshift-session/src',
    'crates/glyphshift-runtime-contract/src',
    'crates/glyphshift-runtime-kernel/src',
    'crates/glyphshift-target-runtime-contract/src',
    'crates/glyphshift-target-process-host/src',
    'crates/glyphshift-target-runtime/src',
    'apps/glyphshift-service/src',
    'apps/glyphshift-desktop/src',
    'apps/glyphshift-desktop/src-tauri/src'
)

$forbiddenTokens = @(
    'After Effects',
    'AfterFX',
    'Premiere',
    'QQ',
    'after-effects-legacy',
    'runtime.driver',
    'HostProfile',
    'HostBridge',
    'archive/dictionary-sources',
    'archive\dictionary-sources',
    'schema2.json',
    'windows.gdi.ext-text-out',
    'windows.gdiplus.draw-string'
)

foreach ($relativeRoot in $productionSourceRoots) {
    $sourceRoot = Join-Path $workspaceRoot $relativeRoot
    if (-not (Test-Path -LiteralPath $sourceRoot -PathType Container)) {
        continue
    }

    $sourceFiles = Get-ChildItem -LiteralPath $sourceRoot -Recurse -File |
        Where-Object { $_.Extension -in @('.rs', '.ts', '.vue', '.css') }

    foreach ($sourceFile in $sourceFiles) {
        $content = [System.IO.File]::ReadAllText($sourceFile.FullName)

        foreach ($token in $forbiddenTokens) {
            if ($content.IndexOf($token, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
                $relativeFile = [System.IO.Path]::GetRelativePath(
                    $workspaceRoot,
                    $sourceFile.FullName
                )
                throw "Forbidden token '$token' found in $relativeFile"
            }
        }

        if ($content -match '(?<![A-Za-z0-9])[A-Za-z]:[\\/]') {
            $relativeFile = [System.IO.Path]::GetRelativePath(
                $workspaceRoot,
                $sourceFile.FullName
            )
            throw "Machine-specific absolute path found in $relativeFile"
        }
    }
}

Write-Output 'Glyphshift architecture checks passed.'
