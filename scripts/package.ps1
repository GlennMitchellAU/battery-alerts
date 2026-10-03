[CmdletBinding()]
param([switch]$SkipVerification)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
$taskStage = $null
try {
    if (-not $SkipVerification) { & (Join-Path $PSScriptRoot 'verify.ps1') }
    $taskVersion = [regex]::Match((Get-Content -LiteralPath 'Cargo.toml' -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
    if ($taskVersion -notmatch '^\d+\.\d+\.\d+$') { throw 'Expected a numeric package version.' }
    $taskArtifactRoot = Join-Path $taskRoot 'artifacts'
    New-Item -ItemType Directory -Path $taskArtifactRoot -Force | Out-Null
    $taskArtifactRoot = (Resolve-Path -LiteralPath $taskArtifactRoot).Path
    $taskStage = Join-Path $taskArtifactRoot ('stage-' + [guid]::NewGuid().ToString('N'))
    & (Join-Path $PSScriptRoot 'build.ps1') -SkipTests -OutputDirectory $taskStage
    $taskZipName = "BatteryAlerts-$taskVersion-windows-x64.zip"
    $taskZip = Join-Path $taskArtifactRoot $taskZipName
    Compress-Archive -Path (Join-Path $taskStage '*') -DestinationPath $taskZip -Force
    $taskHash = (Get-FileHash -LiteralPath $taskZip -Algorithm SHA256).Hash.ToLowerInvariant()
    "$taskHash  $taskZipName" | Set-Content -LiteralPath "$taskZip.sha256" -Encoding ascii
    & (Join-Path $PSScriptRoot 'audit-package.ps1') -ZipPath $taskZip
    Write-Host "Package: $taskZip"
} finally {
    if ($taskStage -and (Test-Path -LiteralPath $taskStage)) {
        $taskResolvedStage = (Resolve-Path -LiteralPath $taskStage).Path
        if (-not $taskResolvedStage.StartsWith($taskArtifactRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Refusing to remove a staging directory outside artifacts.' }
        Remove-Item -LiteralPath $taskResolvedStage -Recurse -Force
    }
    Pop-Location
}
