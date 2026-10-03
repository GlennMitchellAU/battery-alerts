[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$taskRoot = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'toolchain.ps1')
Push-Location $taskRoot
try {
    cargo fmt --all --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting check failed.' }
    cargo clippy --locked --all-targets --target x86_64-pc-windows-msvc -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Clippy check failed.' }
    cargo test --locked --target x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    & (Join-Path $PSScriptRoot 'build.ps1') -SkipTests -OutputDirectory (Join-Path $taskRoot 'artifacts\verified')
    $taskExecutable = Join-Path $taskRoot 'artifacts\verified\battery-alerts.exe'
    $taskInfo = [Diagnostics.ProcessStartInfo]::new()
    $taskInfo.FileName = $taskExecutable
    $taskInfo.Arguments = '--version'
    $taskInfo.UseShellExecute = $false
    $taskInfo.CreateNoWindow = $true
    $taskInfo.RedirectStandardOutput = $true
    $taskProcess = [Diagnostics.Process]::Start($taskInfo)
    $taskVersion = $taskProcess.StandardOutput.ReadToEnd().Trim()
    $taskProcess.WaitForExit()
    if ($taskProcess.ExitCode -ne 0 -or $taskVersion -notmatch '^Battery Alerts \d+\.\d+\.\d+$') { throw 'Version smoke check failed.' }
    Write-Host "Verified: $taskVersion"
} finally { Pop-Location }
