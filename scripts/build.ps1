[CmdletBinding()]
param([switch]$SkipTests, [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $taskRoot 'dist' }
. (Join-Path $PSScriptRoot 'toolchain.ps1')
Push-Location $taskRoot
try {
    if (-not $SkipTests) {
        cargo test --locked --target x86_64-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    }
    cargo build --release --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
    $taskOutput = (Resolve-Path -LiteralPath $OutputDirectory).Path
    Copy-Item -LiteralPath (Join-Path $taskRoot 'target\x86_64-pc-windows-msvc\release\battery-alerts.exe') -Destination $taskOutput
    Copy-Item -LiteralPath (Join-Path $taskRoot 'battery-alerts.json') -Destination $taskOutput
    Copy-Item -LiteralPath (Join-Path $taskRoot 'docs\portable-readme.txt') -Destination (Join-Path $taskOutput 'README.txt')
    Copy-Item -LiteralPath (Join-Path $taskRoot 'LICENSE') -Destination $taskOutput
    Copy-Item -LiteralPath (Join-Path $taskRoot 'THIRD_PARTY_NOTICES.md') -Destination $taskOutput
    & (Join-Path $PSScriptRoot 'licenses.ps1') -Destination (Join-Path $taskOutput 'licenses')
    Write-Host "Portable app: $taskOutput\battery-alerts.exe"
} finally { Pop-Location }
