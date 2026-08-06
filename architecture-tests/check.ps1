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
$publishablePackages = @(
    $metadata.packages |
        Where-Object { $null -eq $_.publish } |
        ForEach-Object { $_.name } |
        Sort-Object
)
if ($publishablePackages.Count -gt 0) {
    throw "Workspace packages must opt out of registry publishing: $($publishablePackages -join ', ')"
}

$dependencyContractPackages = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::Ordinal
)

function Assert-Dependencies {
    param(
        [Parameter(Mandatory)]
        [string] $PackageName,

        [Parameter(Mandatory)]
        [AllowEmptyCollection()]
        [string[]] $Expected
    )

    if (-not $script:dependencyContractPackages.Add($PackageName)) {
        throw "Duplicate dependency contract: $PackageName"
    }

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
Assert-Dependencies -PackageName 'glyphshift-acquisition' -Expected @()
Assert-Dependencies `
    -PackageName 'glyphshift-interactive-translation' `
    -Expected @('glyphshift-acquisition')
Assert-Dependencies `
    -PackageName 'glyphshift-acquisition-worker-sdk' `
    -Expected @('glyphshift-acquisition', 'serde', 'serde_json')
Assert-Dependencies `
    -PackageName 'glyphshift-acquisition-worker-host' `
    -Expected @('glyphshift-acquisition', 'glyphshift-acquisition-worker-sdk')
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
    -PackageName 'glyphshift-adapter-console' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-console-native' `
    -Expected @(
        'glyphshift-adapter-console',
        'glyphshift-adapter-native-abi',
        'retour',
        'windows'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-direct2d' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-direct2d-native' `
    -Expected @(
        'glyphshift-adapter-direct2d',
        'glyphshift-adapter-native-abi',
        'retour',
        'windows'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-directwrite' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-directwrite-native' `
    -Expected @(
        'glyphshift-adapter-directwrite',
        'glyphshift-adapter-native-abi',
        'retour',
        'windows'
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
    -PackageName 'glyphshift-adapter-gtk3-pango' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-gtk3-pango-native' `
    -Expected @('glyphshift-adapter-gtk3-pango', 'glyphshift-adapter-native-abi', 'retour', 'windows')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-ocr' `
    -Expected @('glyphshift-acquisition')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-qt-painter' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-qt-painter-native' `
    -Expected @('glyphshift-adapter-native-abi', 'glyphshift-adapter-qt-painter', 'retour', 'windows')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-raylib' `
    -Expected @('glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-raylib-native' `
    -Expected @('glyphshift-adapter-native-abi', 'glyphshift-adapter-raylib', 'retour', 'windows')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-uia' `
    -Expected @('glyphshift-acquisition', 'glyphshift-adapter-sdk', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-adapter-uia-worker' `
    -Expected @(
        'glyphshift-acquisition',
        'glyphshift-acquisition-worker-sdk',
        'glyphshift-adapter-uia',
        'glyphshift-capture',
        'glyphshift-isolated-worker-sdk',
        'windows',
        'windows-core',
        'windows-sys'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-controller-sdk' `
    -Expected @('serde', 'serde_json')
Assert-Dependencies `
    -PackageName 'glyphshift-controller-host' `
    -Expected @(
        'glyphshift-adapter-registry',
        'glyphshift-capture',
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
        'glyphshift-capture',
        'glyphshift-controller-sdk',
        'glyphshift-runtime-contract',
        'glyphshift-target-runtime-contract',
        'sha2',
        'windows',
        'windows-sys'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-isolated-worker-sdk' `
    -Expected @('serde', 'serde_json')
Assert-Dependencies `
    -PackageName 'glyphshift-isolated-worker-host' `
    -Expected @(
        'glyphshift-adapter-registry',
        'glyphshift-capture',
        'glyphshift-domain',
        'glyphshift-isolated-worker-sdk',
        'glyphshift-protocol',
        'glyphshift-runtime-contract',
        'glyphshift-session',
        'glyphshift-target-process-host',
        'serde_json'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-translation' `
    -Expected @('glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-workflow' `
    -Expected @('glyphshift-domain', 'glyphshift-translation')
Assert-Dependencies `
    -PackageName 'glyphshift-decision' `
    -Expected @('glyphshift-domain', 'glyphshift-translation')
Assert-Dependencies `
    -PackageName 'glyphshift-extension' `
    -Expected @('glyphshift-adapter-registry', 'glyphshift-domain')
Assert-Dependencies `
    -PackageName 'glyphshift-protocol' `
    -Expected @(
        'glyphshift-adapter-registry',
        'glyphshift-capture',
        'glyphshift-domain',
        'glyphshift-extension'
    )
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
        'windows',
        'windows-sys'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-test-controller-plugin' `
    -Expected @('glyphshift-controller-sdk')
Assert-Dependencies `
    -PackageName 'glyphshift-test-isolated-worker' `
    -Expected @(
        'glyphshift-adapter-uia',
        'glyphshift-capture',
        'glyphshift-isolated-worker-sdk'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-test-acquisition-worker' `
    -Expected @('glyphshift-acquisition', 'glyphshift-acquisition-worker-sdk')
Assert-Dependencies `
    -PackageName 'glyphshift-windows-runtime-target' `
    -Expected @('glyphshift-windows-host')
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
        'glyphshift-acquisition',
        'glyphshift-acquisition-worker-host',
        'glyphshift-adapter-native-host',
        'glyphshift-adapter-registry',
        'glyphshift-capture',
        'glyphshift-controller-host',
        'glyphshift-desktop-backend',
        'glyphshift-domain',
        'glyphshift-extension',
        'glyphshift-isolated-worker-host',
        'glyphshift-protocol',
        'glyphshift-runtime-contract',
        'glyphshift-session',
        'glyphshift-target-process-host',
        'serde',
        'serde_json',
        'sha2',
        'url'
    )
Assert-Dependencies `
    -PackageName 'glyphshift-desktop-shell' `
    -Expected @(
        'glyphshift-capture',
        'glyphshift-controller-windows',
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
        'tauri-plugin-global-shortcut',
        'tauri-plugin-opener',
        'tempfile',
        'winreg'
    )

function Assert-PackageSet {
    param(
        [Parameter(Mandatory)]
        [string] $SetName,

        [Parameter(Mandatory)]
        [string[]] $Actual
    )

    $workspacePackages = @(
        $metadata.packages |
            Where-Object { $metadata.workspace_members -contains $_.id } |
            ForEach-Object { $_.name } |
            Sort-Object -Unique
    )
    $actualSorted = @($Actual | Sort-Object -Unique)
    $difference = Compare-Object -ReferenceObject $workspacePackages -DifferenceObject $actualSorted
    if ($difference) {
        $rendered = $difference |
            ForEach-Object { "$($_.SideIndicator) $($_.InputObject)" }
        throw "$SetName workspace coverage failed: $($rendered -join ', ')"
    }
}

function Assert-PackagePartition {
    param(
        [Parameter(Mandatory)]
        [string] $PartitionName,

        [Parameter(Mandatory)]
        [System.Collections.IDictionary] $Groups
    )

    $assigned = @(
        foreach ($group in $Groups.Keys) {
            foreach ($packageName in @($Groups[$group])) {
                $packageName
            }
        }
    )
    $duplicates = @($assigned | Group-Object | Where-Object { $_.Count -ne 1 })
    if ($duplicates) {
        $rendered = $duplicates | ForEach-Object { "$($_.Name) x$($_.Count)" }
        throw "$PartitionName contains duplicate packages: $($rendered -join ', ')"
    }
    Assert-PackageSet -SetName $PartitionName -Actual $assigned
}

$adapterImplementationPackages = @(
    'glyphshift-adapter-console',
    'glyphshift-adapter-console-native',
    'glyphshift-adapter-direct2d',
    'glyphshift-adapter-direct2d-native',
    'glyphshift-adapter-directwrite',
    'glyphshift-adapter-directwrite-native',
    'glyphshift-adapter-draw-text-native',
    'glyphshift-adapter-gdi',
    'glyphshift-adapter-gdi-native',
    'glyphshift-adapter-gdi-native-support',
    'glyphshift-adapter-gdi-text-out-native',
    'glyphshift-adapter-gdiplus',
    'glyphshift-adapter-gdiplus-native',
    'glyphshift-adapter-gtk3-pango',
    'glyphshift-adapter-gtk3-pango-native',
    'glyphshift-adapter-ocr',
    'glyphshift-adapter-qt-painter',
    'glyphshift-adapter-qt-painter-native',
    'glyphshift-adapter-raylib',
    'glyphshift-adapter-raylib-native',
    'glyphshift-adapter-uia',
    'glyphshift-adapter-uia-worker'
)
$testSupportPackages = @(
    'glyphshift-reference-adapters',
    'glyphshift-test-acquisition-worker',
    'glyphshift-test-controller-plugin',
    'glyphshift-test-isolated-worker',
    'glyphshift-windows-host',
    'glyphshift-windows-runtime-target'
)

$packageFamilies = @{
    Core = @(
        'glyphshift-capture',
        'glyphshift-decision',
        'glyphshift-domain',
        'glyphshift-extension',
        'glyphshift-translation',
        'glyphshift-workflow'
    )
    Dictionary = @(
        'glyphshift-dictionary-distribution',
        'glyphshift-dictionary-package'
    )
    AdapterPlatform = @(
        'glyphshift-adapter-native-abi',
        'glyphshift-adapter-native-host',
        'glyphshift-adapter-registry',
        'glyphshift-adapter-sdk'
    )
    AdapterImplementation = $adapterImplementationPackages
    Runtime = @(
        'glyphshift-acquisition',
        'glyphshift-acquisition-worker-host',
        'glyphshift-acquisition-worker-sdk',
        'glyphshift-controller-host',
        'glyphshift-controller-sdk',
        'glyphshift-controller-windows',
        'glyphshift-desktop-runtime',
        'glyphshift-isolated-worker-host',
        'glyphshift-isolated-worker-sdk',
        'glyphshift-protocol',
        'glyphshift-runtime-contract',
        'glyphshift-runtime-kernel',
        'glyphshift-session',
        'glyphshift-target-process-host',
        'glyphshift-target-runtime',
        'glyphshift-target-runtime-contract'
    )
    Product = @(
        'glyphshift-desktop-backend',
        'glyphshift-interactive-translation'
    )
    Application = @('glyphshift-desktop-shell', 'glyphshift-service')
    TestSupport = $testSupportPackages
}

$dependencyLayers = @{
    L0 = @(
        'glyphshift-acquisition',
        'glyphshift-acquisition-worker-sdk',
        'glyphshift-adapter-sdk',
        'glyphshift-capture',
        'glyphshift-controller-sdk',
        'glyphshift-dictionary-package',
        'glyphshift-domain',
        'glyphshift-isolated-worker-sdk',
        'glyphshift-translation'
    )
    L1 = @(
        'glyphshift-adapter-registry',
        'glyphshift-decision',
        'glyphshift-dictionary-distribution',
        'glyphshift-extension',
        'glyphshift-interactive-translation',
        'glyphshift-runtime-contract',
        'glyphshift-workflow'
    )
    L2 = @(
        'glyphshift-adapter-native-abi',
        'glyphshift-protocol',
        'glyphshift-target-runtime-contract'
    )
    Adapter = $adapterImplementationPackages
    L3 = @(
        'glyphshift-acquisition-worker-host',
        'glyphshift-adapter-native-host',
        'glyphshift-controller-host',
        'glyphshift-controller-windows',
        'glyphshift-isolated-worker-host',
        'glyphshift-runtime-kernel',
        'glyphshift-session',
        'glyphshift-target-process-host',
        'glyphshift-target-runtime'
    )
    L4 = @(
        'glyphshift-desktop-backend',
        'glyphshift-desktop-runtime',
        'glyphshift-service'
    )
    L5 = @('glyphshift-desktop-shell')
    Test = $testSupportPackages
}

Assert-PackageSet `
    -SetName 'Exact dependency contracts' `
    -Actual @($dependencyContractPackages)
Assert-PackagePartition -PartitionName 'Package families' -Groups $packageFamilies
Assert-PackagePartition -PartitionName 'Dependency layers' -Groups $dependencyLayers

$allowedDependencyLayers = @{
    L0 = @('L0')
    L1 = @('L0', 'L1')
    L2 = @('L0', 'L1', 'L2')
    Adapter = @('L0', 'L1', 'L2', 'Adapter')
    L3 = @('L0', 'L1', 'L2', 'L3')
    L4 = @('L0', 'L1', 'L2', 'L3', 'L4')
    L5 = @('L0', 'L1', 'L2', 'L3', 'L4', 'L5')
    Test = @('L0', 'L1', 'L2', 'Adapter', 'L3', 'L4', 'L5', 'Test')
}

function Assert-DependencyDirection {
    param(
        [Parameter(Mandatory)]
        [string] $PackageName,

        [Parameter(Mandatory)]
        [string] $PackageLayer,

        [Parameter(Mandatory)]
        [string] $DependencyName,

        [Parameter(Mandatory)]
        [string] $DependencyLayer
    )

    if ($DependencyLayer -notin $allowedDependencyLayers[$PackageLayer]) {
        throw "Forbidden dependency direction: $PackageName ($PackageLayer) -> $DependencyName ($DependencyLayer)"
    }
}

$layerByPackage = @{}
foreach ($layer in $dependencyLayers.Keys) {
    foreach ($packageName in @($dependencyLayers[$layer])) {
        $layerByPackage[$packageName] = $layer
    }
}

foreach ($package in $metadata.packages | Where-Object { $metadata.workspace_members -contains $_.id }) {
    $packageLayer = $layerByPackage[$package.name]
    $internalDependencies = @(
        $package.dependencies |
            Where-Object {
                ($null -eq $_.kind -or $_.kind -eq 'normal') -and
                $layerByPackage.ContainsKey($_.name)
            }
    )
    foreach ($dependency in $internalDependencies) {
        Assert-DependencyDirection `
            -PackageName $package.name `
            -PackageLayer $packageLayer `
            -DependencyName $dependency.name `
            -DependencyLayer $layerByPackage[$dependency.name]
    }
}

foreach ($validCase in @(
    @('policy', 'L1', 'foundation', 'L0'),
    @('adapter', 'Adapter', 'contract', 'L2'),
    @('test', 'Test', 'shell', 'L5')
)) {
    Assert-DependencyDirection `
        -PackageName $validCase[0] `
        -PackageLayer $validCase[1] `
        -DependencyName $validCase[2] `
        -DependencyLayer $validCase[3]
}

foreach ($forbiddenCase in @(
    @('foundation', 'L0', 'host', 'L3'),
    @('host', 'L3', 'concrete-adapter', 'Adapter'),
    @('product', 'L4', 'test-support', 'Test')
)) {
    $rejected = $false
    try {
        Assert-DependencyDirection `
            -PackageName $forbiddenCase[0] `
            -PackageLayer $forbiddenCase[1] `
            -DependencyName $forbiddenCase[2] `
            -DependencyLayer $forbiddenCase[3]
    }
    catch {
        if ($_.Exception.Message -notlike 'Forbidden dependency direction:*') {
            throw
        }
        $rejected = $true
    }
    if (-not $rejected) {
        throw "Architecture self-test accepted a forbidden dependency: $($forbiddenCase -join ' ')"
    }
}

$productionScanPackageNames = @(
    'glyphshift-acquisition',
    'glyphshift-acquisition-worker-host',
    'glyphshift-acquisition-worker-sdk',
    'glyphshift-adapter-registry',
    'glyphshift-adapter-sdk',
    'glyphshift-adapter-native-abi',
    'glyphshift-adapter-native-host',
    'glyphshift-adapter-ocr',
    'glyphshift-adapter-uia',
    'glyphshift-adapter-uia-worker',
    'glyphshift-controller-sdk',
    'glyphshift-controller-host',
    'glyphshift-controller-windows',
    'glyphshift-capture',
    'glyphshift-desktop-backend',
    'glyphshift-desktop-runtime',
    'glyphshift-domain',
    'glyphshift-extension',
    'glyphshift-isolated-worker-host',
    'glyphshift-isolated-worker-sdk',
    'glyphshift-interactive-translation',
    'glyphshift-translation',
    'glyphshift-decision',
    'glyphshift-session',
    'glyphshift-runtime-contract',
    'glyphshift-runtime-kernel',
    'glyphshift-target-runtime-contract',
    'glyphshift-target-process-host',
    'glyphshift-target-runtime',
    'glyphshift-service',
    'glyphshift-desktop-shell'
)

$workspacePackagesByName = @{}
foreach ($package in $metadata.packages | Where-Object { $metadata.workspace_members -contains $_.id }) {
    $workspacePackagesByName[$package.name] = $package
}

$productionSourceRoots = @(
    foreach ($packageName in $productionScanPackageNames) {
        if (-not $workspacePackagesByName.ContainsKey($packageName)) {
            throw "Production source scan package is missing from the workspace: $packageName"
        }
        $packageRoot = Split-Path -Parent $workspacePackagesByName[$packageName].manifest_path
        [System.IO.Path]::GetRelativePath($workspaceRoot, (Join-Path $packageRoot 'src'))
    }
)
$productionSourceRoots += 'apps/glyphshift-desktop/src'

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
