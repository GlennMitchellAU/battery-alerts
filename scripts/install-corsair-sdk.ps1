[CmdletBinding()]
param([string]$AppDirectory)
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
if (-not $AppDirectory) { $AppDirectory = Join-Path $taskRoot 'dist' }
if (-not (Test-Path -LiteralPath (Join-Path $AppDirectory 'battery-alerts.exe'))) { throw 'Build or extract the app first, then specify its directory.' }
$taskVendor = Join-Path $taskRoot 'vendor\icue-4.0.84'
$taskSdk = Join-Path $taskVendor 'iCUESDK\redist\x64\iCUESDK.x64_2019.dll'
if (-not (Test-Path -LiteralPath $taskSdk)) {
    New-Item -ItemType Directory -Path $taskVendor -Force | Out-Null
    $taskArchive = Join-Path $taskVendor 'sdk.zip'
    Invoke-WebRequest -Uri 'https://github.com/CorsairOfficial/cue-sdk/releases/download/v4.0.84/iCUESDK_4.0.84.zip' -OutFile $taskArchive
    if ((Get-FileHash -LiteralPath $taskArchive -Algorithm SHA256).Hash -ne 'EB4414CF505145F3E507DC839AD587BF7CA684D64BE3D9A834F416A136736D5D') { throw 'Corsair SDK archive hash mismatch.' }
    Expand-Archive -LiteralPath $taskArchive -DestinationPath $taskVendor -Force
}
if ((Get-FileHash -LiteralPath $taskSdk -Algorithm SHA256).Hash -ne 'D72FD819B91FD1D3B0C3DB2EA17A47DCFE3B38E26269AA967ECFEE10D2E93884') { throw 'Corsair SDK DLL hash mismatch.' }
Copy-Item -LiteralPath $taskSdk -Destination $AppDirectory
Copy-Item -LiteralPath (Join-Path $taskVendor 'iCUESDK\doc\iCUE SDK - Overview and Reference.pdf') -Destination $AppDirectory
Write-Host 'Optional SDK installed locally. Its proprietary terms are in the included Corsair reference PDF.'
