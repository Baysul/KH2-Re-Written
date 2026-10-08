# Own allocated synthetic bytes only; never modifies or reads a game process.
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $PSScriptRoot 'ReadOnlyRangeProbe.cs')))
if (![Environment]::Is64BitProcess -or [Runtime.InteropServices.Marshal]::SizeOf([type][ReadOnlyRangeProbe+Region]) -ne 48) {
    throw 'Windows x64 query ABI required'
}
$checks=1;$buffer=[Runtime.InteropServices.Marshal]::AllocHGlobal(4096)
try {
    $authored=[byte[]]@(0..4095 | ForEach-Object {($_*17+3)%256})
    [Runtime.InteropServices.Marshal]::Copy($authored,0,$buffer,4096)
    foreach ($case in @(@{offset=0;count=4096},@{offset=13;count=17},@{offset=4095;count=1})) {
        $got=[ReadOnlyRangeProbe]::Read([Environment]::ProcessId,$buffer.ToInt64()+$case.offset,$case.count)
        $expected=[byte[]]$authored[$case.offset..($case.offset+$case.count-1)]
        if ([Convert]::ToHexString($got) -cne [Convert]::ToHexString($expected)) {throw 'Authored byte read differs'}
        $checks++
    }
    $a=[ReadOnlyRangeProbe]::Query([Environment]::ProcessId,$buffer.ToInt64())
    $b=[ReadOnlyRangeProbe]::Query([Environment]::ProcessId,$buffer.ToInt64()+13)
    if ($a.BaseAddress -ne $b.BaseAddress -or $a.AllocationBase -ne $b.AllocationBase -or $a.RegionSize -ne $b.RegionSize -or
        $a.State -ne 0x1000 -or $a.Type -ne 0x20000 -or $a.Protect -ne 4 -or
        $buffer.ToInt64() -lt $a.BaseAddress -or $buffer.ToInt64()+4096 -gt $a.BaseAddress+$a.RegionSize) {
        throw 'Own heap memory-query fields differ'
    }
    $checks++
    foreach ($case in @(@{pid=0;address=$buffer.ToInt64();count=1},@{pid=[Environment]::ProcessId;address=0;count=1},
        @{pid=[Environment]::ProcessId;address=1;count=32},@{pid=[Environment]::ProcessId;address=$buffer.ToInt64();count=0},
        @{pid=[Environment]::ProcessId;address=$buffer.ToInt64();count=4097},@{pid=[Environment]::ProcessId;address=[long]::MaxValue;count=1})) {
        $rejected=$false
        try {[void][ReadOnlyRangeProbe]::Read($case.pid,$case.address,$case.count)} catch {$rejected=$true}
        if (!$rejected) {throw 'Invalid read admitted'}
        $checks++
    }
    foreach ($case in @(@{pid=0;address=$buffer.ToInt64()},@{pid=[Environment]::ProcessId;address=0},
        @{pid=[Environment]::ProcessId;address=[long]::MaxValue})) {
        $rejected=$false
        try {[void][ReadOnlyRangeProbe]::Query($case.pid,$case.address)} catch {$rejected=$true}
        if (!$rejected) {throw 'Invalid query admitted'}
        $checks++
    }
} finally {[Runtime.InteropServices.Marshal]::FreeHGlobal($buffer)}
$region=[ReadOnlyRangeProbe+Region]::new()
$region.BaseAddress=0x1000;$region.AllocationBase=0x1000;$region.RegionSize=0x1000
$region.State=0x1000;$region.Type=0x1000000;$region.Protect=0x20
[ReadOnlyRangeProbe]::RequireImageRegion($region,0x1100,64,0x1000,$true);$checks++
foreach ($case in @('private','reserved','guard','no access','not executable','different module','outside region','overflow')) {
    $r=$region
    switch ($case) {
        'private' {$r.Type=0x20000}
        'reserved' {$r.State=0x2000}
        'guard' {$r.Protect=0x120}
        'no access' {$r.Protect=1}
        'not executable' {$r.Protect=4}
        'different module' {$r.AllocationBase=0x2000}
        'outside region' {$r.RegionSize=1}
        'overflow' {$r.RegionSize=[ulong]::MaxValue}
    }
    $rejected=$false
    try {[ReadOnlyRangeProbe]::RequireImageRegion($r,0x1100,64,0x1000,$true)} catch {$rejected=$true}
    if (!$rejected) {throw "Invalid image range admitted: $case"}
    $checks++
}
Write-Output "PASS: $checks authored read/query ABI and image-range admission checks."
