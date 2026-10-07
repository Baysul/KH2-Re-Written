# Authored helpers for source-bound static native inspection.
function Get-NativePeOffset($Sections, [long]$Rva, [int]$Count, [long]$FileSize) {
    if ($Rva -lt 0 -or $Count -lt 1 -or $Count -gt 64) {throw 'Invalid bounded native PE span'}
    $rows=@($Sections | Where-Object {$Rva -ge $_.VirtualAddress -and
        $Rva + $Count -le [long]$_.VirtualAddress + $_.SizeOfRawData})
    if ($rows.Count -ne 1) {throw 'Native PE span is unmapped or ambiguous'}
    $offset=[long]$rows[0].PointerToRawData + $Rva - $rows[0].VirtualAddress
    if ($offset -lt 0 -or $offset + $Count -gt $FileSize) {throw 'Native PE span exceeds input'}
    return $offset
}

function Get-NativeRelativeBranchTarget([byte[]]$Bytes, [long]$InstructionRva, [long]$Offset) {
    if ($InstructionRva -lt 0 -or $InstructionRva -gt [uint32]::MaxValue -or
        $Offset -lt 0 -or $Offset + 5 -gt $Bytes.LongLength) {throw 'Relative branch span exceeds input'}
    if ($Bytes[$Offset] -notin @(0xE8,0xE9)) {throw 'Only direct rel32 CALL/JMP leads are admitted'}
    $target=$InstructionRva + 5 + [BitConverter]::ToInt32($Bytes,[int]$Offset + 1)
    if ($target -lt 0 -or $target -gt [uint32]::MaxValue) {throw 'Relative branch target exceeds PE RVA range'}
    return $target
}
