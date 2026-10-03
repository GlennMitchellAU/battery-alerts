# Configure x64 library discovery in this process only, including installations
# that put the C runtime import libraries under lib/onecore/x64.
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'Battery Alerts builds require Windows.' }
$taskVsWhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$taskVsRoots = @()
if (Test-Path -LiteralPath $taskVsWhere) {
    $taskVsRoots = @(& $taskVsWhere -products '*' -property installationPath)
    if ($LASTEXITCODE -ne 0) { throw 'Visual Studio discovery failed.' }
}
$taskVcLib = $null
foreach ($taskVsRoot in $taskVsRoots) {
    $taskVersions = Get-ChildItem -LiteralPath (Join-Path $taskVsRoot 'VC\Tools\MSVC') -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending
    foreach ($taskVersion in $taskVersions) {
        foreach ($taskSuffix in @('lib\x64', 'lib\onecore\x64')) {
            $taskCandidate = Join-Path $taskVersion.FullName $taskSuffix
            if (Test-Path -LiteralPath (Join-Path $taskCandidate 'msvcrt.lib')) { $taskVcLib = $taskCandidate; break }
        }
        if ($taskVcLib) { break }
    }
    if ($taskVcLib) { break }
}
$taskSdkRoot = (Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots' -ErrorAction SilentlyContinue).KitsRoot10
if (-not $taskSdkRoot) { $taskSdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10' }
$taskSdkVersion = Get-ChildItem -LiteralPath (Join-Path $taskSdkRoot 'Lib') -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Where-Object {
    (Test-Path -LiteralPath (Join-Path $_.FullName 'ucrt\x64\ucrt.lib')) -and (Test-Path -LiteralPath (Join-Path $_.FullName 'um\x64\kernel32.lib'))
} | Select-Object -First 1
if ($taskVcLib -and $taskSdkVersion) {
    $env:LIB = @($taskVcLib, (Join-Path $taskSdkVersion.FullName 'ucrt\x64'), (Join-Path $taskSdkVersion.FullName 'um\x64'), $env:LIB) -join ';'
} elseif (-not $env:LIB) {
    throw 'Install Visual Studio C++ Build Tools and the Windows SDK, or run from an x64 Developer PowerShell.'
}

# Keep contributor paths and PDB records out of distributable binaries. Encoded
# flags preserve paths containing spaces and avoid shell parsing of arguments.
$taskSeparator = [char]0x1f
$taskFlags = [Collections.Generic.List[string]]::new()
if ($env:CARGO_ENCODED_RUSTFLAGS) {
    foreach ($taskFlag in $env:CARGO_ENCODED_RUSTFLAGS.Split($taskSeparator)) { $taskFlags.Add($taskFlag) }
} elseif ($env:RUSTFLAGS) {
    throw 'Use CARGO_ENCODED_RUSTFLAGS instead of RUSTFLAGS when supplying custom build flags.'
}
$taskRemapFlags = @("--remap-path-prefix=$env:USERPROFILE=user")
if ($env:CARGO_HOME) { $taskRemapFlags += "--remap-path-prefix=$env:CARGO_HOME=cargo" }
$taskRemapFlags += "--remap-path-prefix=$(Split-Path -Parent $PSScriptRoot)=."
$taskRemapFlags += '-Clink-arg=/DEBUG:NONE'
foreach ($taskFlag in $taskRemapFlags) { if (-not $taskFlags.Contains($taskFlag)) { $taskFlags.Add($taskFlag) } }
$env:CARGO_ENCODED_RUSTFLAGS = $taskFlags -join $taskSeparator
