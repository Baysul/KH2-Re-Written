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

function Get-NativeStaticPointerTarget([byte[]]$Bytes, $Sections, [ulong]$ImageBase,
    [long]$InstructionRva, [long]$Offset) {
    if ($InstructionRva -lt 0 -or $InstructionRva -gt [uint32]::MaxValue -or
        $Offset -lt 0 -or $Offset + 6 -gt $Bytes.LongLength) {throw 'Indirect instruction span exceeds input'}
    if ($Bytes[$Offset] -ne 0xFF -or $Bytes[$Offset+1] -notin @(0x15,0x25)) {
        throw 'Only unprefixed RIP-relative indirect CALL/JMP leads are admitted'
    }
    $slotRva=$InstructionRva + 6 + [BitConverter]::ToInt32($Bytes,[int]$Offset + 2)
    if ($slotRva -lt 0 -or $slotRva -gt [uint32]::MaxValue) {throw 'Indirect slot exceeds PE RVA range'}
    $slotOffset=Get-NativePeOffset $Sections $slotRva 8 $Bytes.LongLength
    $value=[BitConverter]::ToUInt64($Bytes,[int]$slotOffset)
    if ($value -lt $ImageBase -or $value-$ImageBase -gt [uint32]::MaxValue) {
        throw 'Static pointer is not an in-image PE RVA candidate'
    }
    return [pscustomobject]@{slot_rva=$slotRva;slot_file_offset=$slotOffset;
        slot_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData(
            [byte[]]$Bytes[$slotOffset..($slotOffset+7)])).ToLowerInvariant();
        static_target_rva=[long]($value-$ImageBase);
        qualification='static_file_pointer_only_not_live_destination'}
}
