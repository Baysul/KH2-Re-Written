# Generated synthetic bytes only; no source executable or engine is required.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'NativeReader.ps1')
$checks=0
foreach ($case in @(
    @{opcode=0xE8;rva=100;displacement=20;target=125},
    @{opcode=0xE9;rva=100;displacement=-30;target=75},
    @{opcode=0xE8;rva=100;displacement=-105;target=0},
    @{opcode=0xE9;rva=4294967290;displacement=0;target=4294967295}
)) {
    $bytes=[byte[]](@($case.opcode)+[BitConverter]::GetBytes([int]$case.displacement))
    if ((Get-NativeRelativeBranchTarget $bytes $case.rva 0) -ne $case.target) {throw 'Synthetic relative target differs'}
    $checks++
}
foreach ($case in @(
    @{bytes=[byte[]]@(0xFF,0,0,0,0);rva=100;offset=0},
    @{bytes=[byte[]]@(0xE8,0,0,0);rva=100;offset=0},
    @{bytes=[byte[]]@(0xE9,0xFA,0xFF,0xFF,0xFF);rva=0;offset=0},
    @{bytes=[byte[]]@(0xE8,1,0,0,0);rva=4294967295;offset=0},
    @{bytes=[byte[]]@(0xE8,0,0,0,0);rva=100;offset=-1}
)) {
    $rejected=$false
    try { [void](Get-NativeRelativeBranchTarget $case.bytes $case.rva $case.offset) } catch {$rejected=$true}
    if (!$rejected) {throw 'Invalid synthetic relative branch accepted'}
    $checks++
}
$sections=@([pscustomobject]@{VirtualAddress=100;SizeOfRawData=20;PointerToRawData=40})
if ((Get-NativePeOffset $sections 105 5 60) -ne 45) {throw 'Synthetic PE mapping differs'}
$checks++
foreach ($case in @(
    @{sections=$sections;rva=119;count=2;size=60},
    @{sections=@($sections+$sections);rva=105;count=5;size=60},
    @{sections=$sections;rva=105;count=5;size=44},
    @{sections=$sections;rva=105;count=65;size=200}
)) {
    $rejected=$false
    try {[void](Get-NativePeOffset $case.sections $case.rva $case.count $case.size)} catch {$rejected=$true}
    if (!$rejected) {throw 'Invalid synthetic PE span accepted'}
    $checks++
}
$pointerSections=@([pscustomobject]@{VirtualAddress=0;SizeOfRawData=128;PointerToRawData=0})
foreach ($case in @(@{opcode=0x15;pc=16;slot=64},@{opcode=0x25;pc=16;slot=64},@{opcode=0x15;pc=96;slot=32})) {
    $bytes=[byte[]]::new(128)
    $bytes[$case.pc]=0xFF; $bytes[$case.pc+1]=$case.opcode
    [Array]::Copy([BitConverter]::GetBytes([int]($case.slot-$case.pc-6)),0,$bytes,$case.pc+2,4)
    [Array]::Copy([BitConverter]::GetBytes([ulong]0x100050),0,$bytes,$case.slot,8)
    $pointer=Get-NativeStaticPointerTarget $bytes $pointerSections 0x100000 $case.pc $case.pc
    if ($pointer.slot_rva -ne $case.slot -or $pointer.slot_file_offset -ne $case.slot -or
        $pointer.static_target_rva -ne 80 -or $pointer.qualification -ne 'static_file_pointer_only_not_live_destination') {
        throw 'Synthetic static pointer observation differs'
    }
    $checks++
}
foreach ($case in @(
    @{opcode=0xD0;slot=64;value=0x100050;length=128},
    @{opcode=0x15;slot=-1;value=0x100050;length=128},
    @{opcode=0x15;slot=124;value=0x100050;length=128},
    @{opcode=0x15;slot=64;value=0;length=128},
    @{opcode=0x15;slot=64;value=0x100100000;length=128},
    @{opcode=0x15;slot=64;value=0x100050;length=20}
)) {
    $bytes=[byte[]]::new($case.length)
    $bytes[16]=0xFF; $bytes[17]=$case.opcode
    if ($bytes.Length -ge 22) {[Array]::Copy([BitConverter]::GetBytes([int]($case.slot-22)),0,$bytes,18,4)}
    if ($case.slot -ge 0 -and $case.slot+8 -le $bytes.Length) {
        [Array]::Copy([BitConverter]::GetBytes([ulong]$case.value),0,$bytes,$case.slot,8)
    }
    $rejected=$false
    try {[void](Get-NativeStaticPointerTarget $bytes $pointerSections 0x100000 16 16)} catch {$rejected=$true}
    if (!$rejected) {throw 'Invalid synthetic indirect observation accepted'}
    $checks++
}
Write-Output "PASS: $checks synthetic direct/indirect branch and PE mapping checks."
