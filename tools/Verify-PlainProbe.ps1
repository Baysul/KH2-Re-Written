# Requires PowerShell 7. Independent scalar reader; no source/output files are written.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath
)
$ErrorActionPreference = 'Stop'
$taskProbe = Get-Content -LiteralPath $ProbePath -Raw | ConvertFrom-Json -AsHashtable
$taskExpected = @{}
foreach ($taskSample in $taskProbe.samples) {
    $taskKey = $taskSample.index + ':' + $taskSample.ordinal
    if ($taskExpected.ContainsKey($taskKey)) { throw 'Duplicate probe identity' }
    $taskExpected[$taskKey] = $taskSample
}
$taskVerified = 0
$taskBars = 0
$taskBarRecords = 0
$taskBytesTotal = 0L
foreach ($taskName in @('first','second','third','fourth','fifth','sixth')) {
    $taskRelative = "Image/dt/kh2_$taskName.hed"
    $taskHed = [IO.BinaryReader]::new([IO.File]::OpenRead((Join-Path $InstallRoot $taskRelative)))
    $taskPkg = [IO.BinaryReader]::new([IO.File]::OpenRead((Join-Path $InstallRoot ($taskRelative -replace '\.hed$','.pkg'))))
    try {
        if ($taskHed.BaseStream.Length % 32 -ne 0) { throw 'Truncated HED index' }
        $taskOrdinal = 0
        while ($taskHed.BaseStream.Position -lt $taskHed.BaseStream.Length) {
            $taskNameHash = $taskHed.ReadBytes(16)
            $taskOffset = $taskHed.ReadInt64()
            $taskStored = $taskHed.ReadInt32()
            $taskOriginal = $taskHed.ReadInt32()
            if ($taskOffset -lt 0 -or $taskStored -lt 16 -or $taskOriginal -lt 0 -or $taskOffset + $taskStored -gt $taskPkg.BaseStream.Length) { throw 'Invalid index range' }
            $taskPkg.BaseStream.Position = $taskOffset
            $taskSize = $taskPkg.ReadInt32()
            $taskCount = $taskPkg.ReadInt32()
            $taskTag = $taskPkg.ReadInt32()
            $taskDate = $taskPkg.ReadInt32()
            if ($taskTag -eq -2) {
                if ($taskSize -lt 0 -or $taskSize -gt 33554432 -or $taskCount -lt 0 -or 16L + 48L * $taskCount + $taskSize -gt $taskStored) { throw 'Invalid plain range' }
                $taskPkg.BaseStream.Position = $taskOffset + 16L + 48L * $taskCount
                $taskBytes = $taskPkg.ReadBytes($taskSize)
                if ($taskBytes.Length -ne $taskSize) { throw 'Truncated original' }
                $taskHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($taskBytes)).ToLowerInvariant()
                $taskKey = $taskRelative + ':' + $taskOrdinal
                if (-not $taskExpected.ContainsKey($taskKey)) { throw 'Original missing from probe' }
                $taskSample = $taskExpected[$taskKey]
                if ($taskHash -ne $taskSample.sha256 -or $taskSize -ne $taskSample.original_size -or $taskCount -ne $taskSample.replacement_count -or $taskOffset -ne $taskSample.package_offset -or ($taskNameHash -join ',') -ne ($taskSample.name_hash -join ',')) { throw 'Original fingerprint/identity differs' }
                if ($taskSample.format_hint -eq 'BAR') {
                    $taskBar = [IO.BinaryReader]::new([IO.MemoryStream]::new($taskBytes))
                    try {
                        if ($taskBar.ReadUInt32() -ne 0x01524142) { throw 'BAR magic differs' }
                        $taskEntryCount = $taskBar.ReadInt32()
                        $taskField8 = $taskBar.ReadInt32()
                        $taskField12 = $taskBar.ReadInt32()
                        if ($taskEntryCount -ne $taskSample.bar.entries.Count -or $taskField8 -ne $taskSample.bar.header_field_8 -or $taskField12 -ne $taskSample.bar.header_field_12) { throw 'BAR header differs' }
                        for ($taskI = 0; $taskI -lt $taskEntryCount; $taskI++) {
                            $taskEntry = @{entry_type=$taskBar.ReadUInt16();index=$taskBar.ReadInt16();name_bytes=$taskBar.ReadBytes(4);offset=$taskBar.ReadInt32();size=$taskBar.ReadInt32()}
                            $taskReference = $taskSample.bar.entries[$taskI]
                            foreach ($taskField in @('entry_type','index','offset','size')) { if ($taskEntry[$taskField] -ne $taskReference[$taskField]) { throw 'BAR entry differs' } }
                            if (($taskEntry.name_bytes -join ',') -ne ($taskReference.name_bytes -join ',')) { throw 'BAR names differ' }
                        }
                        $taskBars++
                        $taskBarRecords += $taskEntryCount
                    } finally { $taskBar.Dispose() }
                }
                $taskVerified++
                $taskBytesTotal += $taskSize
            }
            $taskOrdinal++
        }
    } finally { $taskHed.Dispose(); $taskPkg.Dispose() }
}
if ($taskVerified -ne $taskExpected.Count) { throw 'Probe contains unmatched originals' }
Write-Output "PASS: $taskVerified original fingerprints, $taskBytesTotal original bytes, $taskBars BAR containers and $taskBarRecords BAR records agree; source was only read."
