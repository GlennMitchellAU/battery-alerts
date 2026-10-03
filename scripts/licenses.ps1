[CmdletBinding()]
param([Parameter(Mandatory)][string]$Destination)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
try {
    $taskMetadataJson = cargo metadata --locked --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Could not read locked dependency metadata.' }
    $taskMetadata = $taskMetadataJson | ConvertFrom-Json
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    $taskIndex = @('Third-party licenses for Battery Alerts', '')
    foreach ($taskPackage in $taskMetadata.packages | Where-Object name -NE 'battery-alerts' | Sort-Object name) {
        $taskLicenseFiles = @(Get-ChildItem -LiteralPath (Split-Path -Parent $taskPackage.manifest_path) -File | Where-Object Name -Match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)')
        if ($taskLicenseFiles.Count -eq 0) { throw "No license text found for $($taskPackage.name)." }
        $taskPackageDir = Join-Path $Destination "$($taskPackage.name)-$($taskPackage.version)"
        New-Item -ItemType Directory -Path $taskPackageDir -Force | Out-Null
        foreach ($taskLicense in $taskLicenseFiles) { Copy-Item -LiteralPath $taskLicense.FullName -Destination $taskPackageDir }
        $taskIndex += "$($taskPackage.name) $($taskPackage.version): $($taskPackage.license)"
    }
    $taskSysroot = rustc --print sysroot
    if ($LASTEXITCODE -ne 0) { throw 'Could not find Rust runtime license texts.' }
    $taskRustDocs = Join-Path $taskSysroot 'share\doc\rust'
    $taskRustLicenses = Join-Path $Destination 'rust-runtime'
    New-Item -ItemType Directory -Path $taskRustLicenses -Force | Out-Null
    foreach ($taskRustFile in @('COPYRIGHT-library.html', 'licenses\MIT.txt', 'licenses\Apache-2.0.txt')) {
        Copy-Item -LiteralPath (Join-Path $taskRustDocs $taskRustFile) -Destination $taskRustLicenses
    }
    Copy-Item -LiteralPath (Join-Path $taskRustDocs 'licenses') -Destination $taskRustLicenses -Recurse -Force
    $taskIndex += 'Rust standard library: see rust-runtime/COPYRIGHT-library.html and accompanying license texts.'
    $taskIndex | Set-Content -LiteralPath (Join-Path $Destination 'INDEX.txt') -Encoding utf8
} finally { Pop-Location }
