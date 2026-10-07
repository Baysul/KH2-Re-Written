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
Write-Output "PASS: $checks synthetic direct-branch and PE mapping checks."
