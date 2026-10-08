# Generated AMD64 metadata only. No installed bytes or engine required.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'NativeReader.ps1')
$bytes=[byte[]]::new(512)
$sections=@([pscustomobject]@{VirtualAddress=0x1000;SizeOfRawData=256;PointerToRawData=0;SectionCharacteristics=0x20000000},
    [pscustomobject]@{VirtualAddress=0x2000;SizeOfRawData=256;PointerToRawData=256;SectionCharacteristics=0})
$directory=[pscustomobject]@{RelativeVirtualAddress=0x2000;Size=24}
function Put-U32([byte[]]$Target,[int]$Offset,[uint32]$Value) {
    [Array]::Copy([BitConverter]::GetBytes($Value),0,$Target,$Offset,4)
}
foreach ($pair in @(@(256,0x1010),@(260,0x1020),@(264,0x2040),@(268,0x1030),@(272,0x1040),@(276,0x2044))) {
    Put-U32 $bytes $pair[0] $pair[1]
}
$rows=@(Get-NativeRuntimeFunctionRecords $bytes $sections $directory 0x3000)
if ($rows.Count -ne 2 -or $rows[0].begin_rva -ne 0x1010 -or $rows[0].end_rva -ne 0x1020 -or
    $rows[0].unwind_rva -ne 0x2040 -or $rows[0].record_file_offset -ne 256 -or $rows[0].code_file_offset -ne 16 -or
    $rows[1].begin_rva -ne 0x1030 -or $rows[1].end_rva -ne 0x1040 -or $rows[1].unwind_rva -ne 0x2044) {
    throw 'Literal authored runtime triples differ'
}
$checks=1
if (@(Get-NativeRuntimeFunctionRecords $bytes $sections ([pscustomobject]@{RelativeVirtualAddress=0;Size=0}) 0x3000).Count) {
    throw 'Absent directory differs'
}
$checks++
foreach ($case in @('unaligned table','partial triple','table budget','zero table','image overflow','unsorted','duplicate begin',
    'empty range','unmapped code','nonexecutable','ambiguous code','truncated raw','unaligned unwind','unmapped unwind','image size')) {
    $b=[byte[]]$bytes.Clone();$s=$sections;$d=$directory;$image=0x3000
    switch ($case) {
        'unaligned table' {$d=[pscustomobject]@{RelativeVirtualAddress=0x2001;Size=24}}
        'partial triple' {$d=[pscustomobject]@{RelativeVirtualAddress=0x2000;Size=23}}
        'table budget' {$d=[pscustomobject]@{RelativeVirtualAddress=0x2000;Size=4097*12};$image=0x100000}
        'zero table' {$d=[pscustomobject]@{RelativeVirtualAddress=0;Size=24}}
        'image overflow' {$image=0x200c}
        'unsorted' {Put-U32 $b 268 0x1001}
        'duplicate begin' {Put-U32 $b 268 0x1010}
        'empty range' {Put-U32 $b 260 0x1010}
        'unmapped code' {Put-U32 $b 260 0x1200}
        'nonexecutable' {$s=@([pscustomobject]@{VirtualAddress=0x1000;SizeOfRawData=256;PointerToRawData=0;SectionCharacteristics=0},$sections[1])}
        'ambiguous code' {$s=@($sections+$sections[0])}
        'truncated raw' {$b=[byte[]]$b[0..269]}
        'unaligned unwind' {Put-U32 $b 264 0x2041}
        'unmapped unwind' {Put-U32 $b 264 0x2200}
        'image size' {$image=0}
    }
    $rejected=$false
    try {[void]@(Get-NativeRuntimeFunctionRecords $b $s $d $image)} catch {$rejected=$true}
    if (!$rejected) {throw "Invalid authored runtime records admitted: $case"}
    $checks++
}
Write-Output "PASS: $checks synthetic AMD64 runtime-function metadata checks."
