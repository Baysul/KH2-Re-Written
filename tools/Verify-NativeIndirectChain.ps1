# Authored operands, synthetic regions and own allocated bytes only.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'NativeIndirectChain.ps1')
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $PSScriptRoot 'ReadOnlyRangeProbe.cs')))
$checks=0
foreach ($case in @(
    @{hex='488B01';register='RCX';displacement=0;kind='load_RAX'},
    @{hex='48FF6018';register='RAX';displacement=24;kind='indirect_jump_slot'},
    @{hex='48FF6080';register='RAX';displacement=-128;kind='indirect_jump_slot'},
    @{hex='48FF607F';register='RAX';displacement=127;kind='indirect_jump_slot'})) {
    $got=Get-NativeRegisterMemoryOperand ([Convert]::FromHexString($case.hex))
    if ($got.base_register -cne $case.register -or $got.displacement -ne $case.displacement -or
        $got.width -ne 8 -or $got.kind -cne $case.kind) {throw 'Authored operand differs'}
    $checks++
}
foreach ($hex in @('488B00','488901','FF6018','48FF5018','48FF2018','488B0100','488B','48FF60','90')) {
    if ($null -ne (Get-NativeRegisterMemoryOperand ([Convert]::FromHexString($hex)))) {throw 'Unsupported encoding admitted'}
    $checks++
}
foreach ($case in @(@{word=0x2000;offset=24;expected=0x2018},@{word=0x2000;offset=-128;expected=0x1f80},
    @{word=0x2000;offset=127;expected=0x207f})) {
    if ((Get-NativeIndirectReadAddress $case.word $case.offset) -ne $case.expected) {throw 'Authored address differs'}
    $checks++
}
foreach ($case in @(@{word=0;offset=24},@{word=[uint64]::MaxValue;offset=0},@{word=1;offset=-1},
    @{word=[long]::MaxValue;offset=0},@{word=0x2000;offset=128},@{word=0x2000;offset=-129})) {
    $rejected=$false
    try {[void](Get-NativeIndirectReadAddress $case.word $case.offset)} catch {$rejected=$true}
    if (!$rejected) {throw 'Unbounded indirect address admitted'}
    $checks++
}
$region=[ReadOnlyRangeProbe+Region]::new()
$region.BaseAddress=0x1000;$region.AllocationBase=0x1000;$region.RegionSize=0x1000
$region.State=0x1000;$region.Type=0x20000;$region.Protect=4
foreach ($type in @(0x20000,0x1000000)) {
    $r=$region;$r.Type=$type
    Assert-NativeReadableSpan $r 0x1100 8 $type 0x1000
    Assert-NativeReadableSpan $r 0x1ff8 8 $type 0x1000
    $checks+=2
}
foreach ($case in @('mapped','reserved','guard','no access','execute only','different allocation','outside region',
    'empty region','overflow','type mismatch','allocation above region','zero address','too large','zero count','address overflow')) {
    $r=$region;$address=0x1100;$count=8
    switch ($case) {
        'mapped' {$r.Type=0x40000}
        'reserved' {$r.State=0x2000}
        'guard' {$r.Protect=0x104}
        'no access' {$r.Protect=1}
        'execute only' {$r.Protect=0x10}
        'different allocation' {$r.AllocationBase=0x800}
        'outside region' {$address=0x1ff9}
        'empty region' {$r.RegionSize=0}
        'overflow' {$r.RegionSize=[uint64]::MaxValue}
        'type mismatch' {$r.Type=0x1000000}
        'allocation above region' {$r.BaseAddress=0x800}
        'zero address' {$address=0}
        'too large' {$count=65}
        'zero count' {$count=0}
        'address overflow' {$address=[long]::MaxValue}
    }
    $rejected=$false
    try {Assert-NativeReadableSpan $r $address $count 0x20000 0x1000} catch {$rejected=$true}
    if (!$rejected) {throw "Invalid region admitted: $case"}
    $checks++
}
$buffer=[Runtime.InteropServices.Marshal]::AllocHGlobal(64)
try {
    $bytes=[byte[]]@(0..63 | ForEach-Object {($_*23+7)%256})
    [Runtime.InteropServices.Marshal]::Copy($bytes,0,$buffer,64)
    $address=$buffer.ToInt64();$r=[ReadOnlyRangeProbe]::Query([Environment]::ProcessId,$address)
    Assert-NativeReadableSpan $r $address 64 0x20000 $r.AllocationBase
    foreach ($offset in @(0,24,56)) {
        $readAddress=Get-NativeIndirectReadAddress ([uint64]$address) $offset
        Assert-NativeReadableSpan $r $readAddress 8 0x20000 $r.AllocationBase
        $got=[ReadOnlyRangeProbe]::Read([Environment]::ProcessId,$readAddress,8)
        if ([Convert]::ToHexString($got) -cne [Convert]::ToHexString([byte[]]$bytes[$offset..($offset+7)])) {
            throw 'Own allocated indirect span differs'
        }
        $checks++
    }
} finally {[Runtime.InteropServices.Marshal]::FreeHGlobal($buffer)}
Write-Output "PASS: $checks authored operand/address/region and own-memory checks."
