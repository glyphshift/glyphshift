[CmdletBinding()]
param(
    [ValidateSet('Debug','Release')][string]$Profile = 'Debug',
    [Parameter(Mandatory)][string]$OutputRoot,
    [string]$CargoTargetDir,
    [switch]$IncludeTestTarget
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'cargo-target.ps1')
$CargoTargetDir = Get-GlyphshiftCargoTargetDirectory -RepoRoot $repoRoot -Override $CargoTargetDir
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
$localPrefix = [IO.Path]::GetFullPath((Join-Path $repoRoot 'local-test')).TrimEnd('\') + '\'
$stage = "$OutputRoot.staging"
foreach ($path in @($OutputRoot,$stage)) {
    if (-not $path.StartsWith($localPrefix,[StringComparison]::OrdinalIgnoreCase)) { throw 'Runtime output must stay below local-test.' }
}
if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
$catalog = Get-Content -Raw (Join-Path $PSScriptRoot 'adapter-distribution.json') | ConvertFrom-Json
if ($catalog.schema -ne 'glyphshift.adapter-distribution/1') { throw 'Unknown adapter distribution schema.' }
$presentations = Get-Content -Raw (Join-Path $PSScriptRoot 'runtime-bundle-adapters.zh-CN.json') | ConvertFrom-Json
$profileDirectory = $Profile.ToLowerInvariant()
$manifestPath = Join-Path $repoRoot 'Cargo.toml'
function Copy-Artifact([string]$Source, [string]$Stem) {
    $hash = (Get-FileHash -LiteralPath $Source -Algorithm SHA256).Hash.ToLowerInvariant()
    $name = "$Stem-$($hash.Substring(0,12))$([IO.Path]::GetExtension($Source))"
    Copy-Item -LiteralPath $Source -Destination (Join-Path $stage $name)
    return [ordered]@{file=$name; sha256=$hash}
}
$groups = @()
foreach ($architecture in @('x86_64','x86')) {
    $arguments = @('build','--locked','--manifest-path',$manifestPath,'--target-dir',$CargoTargetDir,
        '-p','glyphshift-controller-windows','-p','glyphshift-target-runtime')
    foreach ($package in $catalog.coreBuildTargets) { $arguments += @('-p',$package) }
    if ($architecture -eq 'x86') { $arguments += @('--target','i686-pc-windows-msvc') }
    if ($IncludeTestTarget) { $arguments += @('-p','glyphshift-windows-runtime-target') }
    if ($Profile -eq 'Release') { $arguments += '--release' }
    & cargo @arguments
    if ($LASTEXITCODE -ne 0) { throw "Core Runtime build failed: $architecture" }
    $source = if ($architecture -eq 'x86') { Join-Path $CargoTargetDir "i686-pc-windows-msvc/$profileDirectory" } else { Join-Path $CargoTargetDir $profileDirectory }
    $controller = Copy-Artifact (Join-Path $source 'glyphshift-controller-windows.exe') "controller-$architecture"
    $controller.artifact = if ($architecture -eq 'x86') { 'windows-generic-controller-x86' } else { 'windows-generic-controller' }
    $controller.protocol = @(1,0)
    $runtime = Copy-Artifact (Join-Path $source 'glyphshift_target_runtime.dll') "runtime-$architecture"
    $adapters = @()
    foreach ($package in $catalog.coreBuildTargets) {
        $artifact = Copy-Artifact (Join-Path $source ($package.Replace('-','_') + '.dll')) "$package-$architecture"
        $metadata = & (Join-Path $stage $controller.file) --inspect-adapter (Join-Path $stage $artifact.file)
        if ($LASTEXITCODE -ne 0) { throw "Core descriptor inspection failed: $package" }
        $metadata = $metadata | ConvertFrom-Json
        if ($metadata.adapter_id -notin $catalog.core) { throw 'Unexpected core Adapter ID.' }
        $presentation = @($presentations | Where-Object id -eq $metadata.adapter_id)
        if ($presentation.Count -ne 1) { throw 'Missing or duplicate core presentation.' }
        $presentation = $presentation[0]
        foreach ($key in @('name','summary','technology','technicalTarget','documentationUrl')) { $artifact[$key] = $presentation.$key }
        $artifact.native_metadata = $metadata
        $adapters += $artifact
    }
    $actualIds = @($adapters | ForEach-Object { $_.native_metadata.adapter_id })
    if (@(Compare-Object $catalog.core $actualIds).Count -ne 0) { throw 'Incomplete core Adapter set.' }
    if ($IncludeTestTarget) {
        $name = if ($architecture -eq 'x86') { 'test-target-x86.exe' } else { 'test-target.exe' }
        Copy-Item -LiteralPath (Join-Path $source 'glyphshift-windows-runtime-target.exe') -Destination (Join-Path $stage $name)
    }
    $groups += [ordered]@{architecture=$architecture; controller=$controller; runtime=$runtime; adapters=$adapters}
}
$manifest = $groups[0]
$manifest.schema = 'glyphshift.runtime-bundle/4'
$manifest.authority = 'app.glyphshift.runtime.first-party'
$manifest.isolated_workers = @()
$manifest.acquisition_workers = @()
$manifest.additional_architectures = @($groups[1])
[IO.File]::WriteAllText((Join-Path $stage 'runtime-bundle.json'), ($manifest | ConvertTo-Json -Depth 16), [Text.UTF8Encoding]::new($false))
$arguments = @('build','--locked','--manifest-path',$manifestPath,'--target-dir',$CargoTargetDir,
    '-p','glyphshift-desktop-runtime','--bin','glyphshift-runtime-bundle-verify')
if ($Profile -eq 'Release') { $arguments += '--release' }
& cargo @arguments
if ($LASTEXITCODE -ne 0) { throw 'Runtime verifier build failed.' }
& (Join-Path $CargoTargetDir "$profileDirectory/glyphshift-runtime-bundle-verify.exe") $stage
if ($LASTEXITCODE -ne 0) { throw 'Core Runtime failed production loader verification.' }
# Keep immutable old DLLs alive for existing sessions. Fresh release directories
# contain only core files; the manifest controls discovery in reused directories.
New-Item -ItemType Directory -Path $OutputRoot -Force | Out-Null
Get-ChildItem -LiteralPath $stage -File | Where-Object Name -ne 'runtime-bundle.json' | ForEach-Object {
    $destination = Join-Path $OutputRoot $_.Name
    if (-not (Test-Path -LiteralPath $destination)) { Copy-Item -LiteralPath $_.FullName -Destination $destination }
    elseif ($_.Name -in @('test-target.exe','test-target-x86.exe')) {
        Copy-Item -LiteralPath $_.FullName -Destination $destination -Force
    }
    elseif ((Get-FileHash -LiteralPath $destination).Hash -ne (Get-FileHash -LiteralPath $_.FullName).Hash) {
        throw 'Existing Runtime artifact differs; use a fresh output directory.'
    }
}
$next = Join-Path $OutputRoot 'runtime-bundle.json.next'
Copy-Item -LiteralPath (Join-Path $stage 'runtime-bundle.json') -Destination $next -Force
[IO.File]::Move($next,(Join-Path $OutputRoot 'runtime-bundle.json'),$true)
Remove-Item -LiteralPath $stage -Recurse -Force
Write-Output "Core Runtime Bundle ready: $OutputRoot"
