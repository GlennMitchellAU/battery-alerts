[CmdletBinding()]
param([Parameter(Mandatory)][string]$ZipPath)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem
$taskZip = [IO.Compression.ZipFile]::OpenRead((Resolve-Path -LiteralPath $ZipPath).Path)
try {
    $taskExpectedRoot = @('battery-alerts.exe', 'battery-alerts.json', 'README.txt', 'LICENSE', 'THIRD_PARTY_NOTICES.md')
    $taskNames = @($taskZip.Entries | Select-Object -ExpandProperty FullName)
    foreach ($taskExpected in $taskExpectedRoot) {
        if ($taskNames -notcontains $taskExpected) { throw "Package is missing $taskExpected." }
    }
    if ($taskNames -notcontains 'licenses/INDEX.txt') { throw 'Package is missing third-party license notices.' }
    foreach ($taskName in $taskNames) {
        if ($taskName -match '(^/|\\|(^|/)\.\.(/|$))' -or ($taskExpectedRoot -notcontains $taskName -and $taskName -notmatch '^licenses/')) { throw "Unexpected package entry: $taskName" }
        if ($taskName -match '(?i)\.(dll|pdb|log)$') { throw "Unexpected binary or log: $taskName" }
    }
    $taskMemory = [IO.MemoryStream]::new()
    $taskStream = $taskZip.GetEntry('battery-alerts.exe').Open()
    try { $taskStream.CopyTo($taskMemory); $taskBytes = $taskMemory.ToArray() } finally { $taskStream.Dispose(); $taskMemory.Dispose() }
    $taskPatterns = '(?i)[A-Z]:[\\/]+Users[\\/]|github_pat_[A-Za-z0-9_]+|gh[pousr]_[A-Za-z0-9]{30}|BEGIN (RSA |OPENSSH |EC )?PRIVATE KEY'
    foreach ($taskEncoding in @([Text.Encoding]::ASCII, [Text.Encoding]::Unicode)) {
        if ($taskEncoding.GetString($taskBytes) -match $taskPatterns) { throw 'Package binary contains a personal path or potential secret.' }
    }
    $taskPe = [BitConverter]::ToInt32($taskBytes, 60)
    if ([BitConverter]::ToUInt16($taskBytes, $taskPe + 4) -ne 0x8664 -or [BitConverter]::ToUInt16($taskBytes, $taskPe + 24) -ne 0x20b) { throw 'Expected an x64 PE executable.' }
    $taskDebugRva = [BitConverter]::ToUInt32($taskBytes, $taskPe + 24 + 112 + 6 * 8)
    $taskDebugSize = [BitConverter]::ToUInt32($taskBytes, $taskPe + 24 + 112 + 6 * 8 + 4)
    if ($taskDebugRva -ne 0 -or $taskDebugSize -ne 0) {
        $taskSectionCount = [BitConverter]::ToUInt16($taskBytes, $taskPe + 6)
        $taskSections = $taskPe + 24 + [BitConverter]::ToUInt16($taskBytes, $taskPe + 20)
        $taskDebugOffset = $null
        for ($taskI = 0; $taskI -lt $taskSectionCount; $taskI++) {
            $taskSection = $taskSections + 40 * $taskI
            $taskVirtualSize = [BitConverter]::ToUInt32($taskBytes, $taskSection + 8)
            $taskVirtualAddress = [BitConverter]::ToUInt32($taskBytes, $taskSection + 12)
            $taskRawSize = [BitConverter]::ToUInt32($taskBytes, $taskSection + 16)
            if ($taskDebugRva -ge $taskVirtualAddress -and $taskDebugRva -lt $taskVirtualAddress + [Math]::Max($taskVirtualSize, $taskRawSize)) {
                $taskDebugOffset = [BitConverter]::ToUInt32($taskBytes, $taskSection + 20) + $taskDebugRva - $taskVirtualAddress
                break
            }
        }
        if ($null -eq $taskDebugOffset -or $taskDebugSize % 28 -ne 0) { throw 'Invalid PE debug directory.' }
        for ($taskI = 0; $taskI -lt $taskDebugSize; $taskI += 28) {
            $taskDebugType = [BitConverter]::ToUInt32($taskBytes, $taskDebugOffset + $taskI + 12)
            # Linkers can retain non-symbol metadata: VC features (12), PGO (13),
            # reproducibility (16) and extended DLL characteristics (20).
            if ($taskDebugType -notin @(12, 13, 16, 20)) { throw "PE contains symbol/debug metadata (type $taskDebugType)." }
        }
    }
    $taskExpectedHash = (Get-Content -LiteralPath "$ZipPath.sha256" -Raw).Split(' ')[0].Trim()
    if ((Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $taskExpectedHash) { throw 'Package checksum mismatch.' }
    Write-Host "Package audit passed ($($taskNames.Count) entries, x64, no symbol records or user paths)."
} finally { $taskZip.Dispose() }
