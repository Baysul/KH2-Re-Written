# Independently specified literal synthetic encodings, never game data.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'NativeRipReads.ps1')
$checks=0
foreach ($case in @(
    @{hex='488B0509000000';rva=0x1000;image=0x3000;target=0x1010;width=8;kind='memory_read'},
    @{hex='4C8B05F9FFFFFF';rva=0x1000;image=0x3000;target=0x1000;width=8;kind='memory_read'},
    @{hex='448B0529000000';rva=0x1000;image=0x3000;target=0x1030;width=4;kind='memory_read'},
    @{hex='8B05FAFFFFFF';rva=0x1000;image=0x3000;target=0x1000;width=4;kind='memory_read'},
    @{hex='0FBE0519000000';rva=0x1000;image=0x3000;target=0x1020;width=1;kind='memory_read'},
    @{hex='833D0900000001';rva=0x1000;image=0x3000;target=0x1010;width=4;kind='memory_read'},
    @{hex='833D09000000FF';rva=0x1000;image=0x3000;target=0x1010;width=4;kind='memory_read'},
    @{hex='FF15FAFFFFFF';rva=0x1000;image=0x3000;target=0x1000;width=8;kind='indirect_call_slot'},
    @{hex='488B0500000080';rva=0x80001000L;image=0xffffffffL;target=0x1007;width=8;kind='memory_read'})) {
    $got=Get-NativeRipRead ([Convert]::FromHexString($case.hex)) $case.rva $case.image
    if ($got.target_rva -ne $case.target -or $got.width -ne $case.width -or $got.kind -cne $case.kind) {throw 'Literal RIP read differs'}
    $checks++
}
foreach ($hex in @('488D0509000000','48890509000000','488B00','F0488B0509000000','668B0509000000','678B0509000000','FF2509000000')) {
    if (@(Get-NativeRipRead ([Convert]::FromHexString($hex)) 0x1000 0x3000).Count) {throw 'Unsupported read/write/address form admitted'}
    $checks++
}
foreach ($case in @(@{hex='';rva=0x1000;image=0x3000},@{hex='488B0500000000';rva=-1;image=0x3000},
    @{hex='488B0500000000';rva=0x1000;image=0},@{hex='488B0500000000';rva=0x1000;image=0x1006},
    @{hex='488B0500000080';rva=0x1000;image=0x3000},@{hex='488B05FA1F0000';rva=0x1000;image=0x3000},
    @{hex=('90'*16);rva=0x1000;image=0x3000})) {
    $rejected=$false
    try {[void](Get-NativeRipRead ([Convert]::FromHexString($case.hex)) $case.rva $case.image)} catch {$rejected=$true}
    if (!$rejected) {throw 'Malformed RIP instruction/image span admitted'}
    $checks++
}
Write-Output "PASS: $checks generated RIP read/width/next-address/rejection checks."
