# Narrow byte-derived AMD64 RIP-read profile; unsupported forms stay unknown.
function Get-NativeRipRead([byte[]]$Instruction,[long]$InstructionRva,[long]$ImageSize) {
    $length=$Instruction.Length
    if ($length -lt 1 -or $length -gt 15 -or $InstructionRva -lt 0 -or $ImageSize -le 0 -or
        $ImageSize -gt [uint32]::MaxValue -or $InstructionRva+$length -gt $ImageSize) {throw 'Invalid complete instruction/image span'}
    $width=0;$displacement=0;$kind='memory_read'
    if ($length -eq 7 -and $Instruction[0] -in @(0x48,0x4c,0x44) -and $Instruction[1] -eq 0x8b -and
        ($Instruction[2] -band 0xc7) -eq 5) {
        $width=if ($Instruction[0] -eq 0x44) {4} else {8};$displacement=3
    } elseif ($length -eq 6 -and $Instruction[0] -eq 0x8b -and ($Instruction[1] -band 0xc7) -eq 5) {
        $width=4;$displacement=2
    } elseif ($length -eq 7 -and $Instruction[0] -eq 0x0f -and $Instruction[1] -eq 0xbe -and
        ($Instruction[2] -band 0xc7) -eq 5) {
        $width=1;$displacement=3
    } elseif ($length -eq 7 -and $Instruction[0] -eq 0x83 -and $Instruction[1] -eq 0x3d) {
        $width=4;$displacement=2
    } elseif ($length -eq 6 -and $Instruction[0] -eq 0xff -and $Instruction[1] -eq 0x15) {
        $width=8;$displacement=2;$kind='indirect_call_slot'
    }
    if (!$width) {return}
    # RIP uses the next complete instruction, including any immediate suffix.
    $target=$InstructionRva+$length+[BitConverter]::ToInt32($Instruction,$displacement)
    if ($target -lt 0 -or $target+$width -gt $ImageSize) {throw 'RIP operand exceeds declared image'}
    return [pscustomobject]@{target_rva=$target;width=$width;kind=$kind}
}
