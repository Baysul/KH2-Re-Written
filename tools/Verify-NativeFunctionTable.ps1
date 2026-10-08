# Generated metadata only; no game file/process reads.
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition ([IO.File]::ReadAllText((Join-Path $PSScriptRoot 'NativeFunctionTable.cs')))
function New-Table($Rows) {
    $stream=[IO.MemoryStream]::new();$writer=[IO.BinaryWriter]::new($stream)
    try {foreach($row in $Rows){foreach($word in $row){$writer.Write([uint32]$word)}};return ,$stream.ToArray()}
    finally {$writer.Dispose();$stream.Dispose()}
}
$table=New-Table @(@(0x1000,0x1020,0x3000),@(0x1040,0x1080,0x3004),@(0x2000,0x2008,0x3008))
$checks=0
foreach($case in @(@{target=0x1000;index=0;exact=$true},@{target=0x101f;index=0;exact=$false},
    @{target=0x1020;index=-1;exact=$false},@{target=0x103f;index=-1;exact=$false},
    @{target=0x1040;index=1;exact=$true},@{target=0x107f;index=1;exact=$false},
    @{target=0x2000;index=2;exact=$true},@{target=0x2008;index=-1;exact=$false})) {
    $got=[NativeFunctionTable]::Select($table,$case.target,0x4000)
    if($got.RecordCount -ne 3 -or $got.Index -ne $case.index -or $got.ExactEntry -ne $case.exact){throw 'Authored selection differs'}
    if($case.index -ge 0 -and ($got.Begin -ne [BitConverter]::ToUInt32($table,$case.index*12) -or
        $got.End -ne [BitConverter]::ToUInt32($table,$case.index*12+4) -or
        $got.Unwind -ne [BitConverter]::ToUInt32($table,$case.index*12+8))){throw 'Selected record differs'}
    $checks++
}
$badRows=@(
    @(@(0,0x1020,0x3000)),@(@(0x1000,0x1000,0x3000)),@(@(0x1020,0x1000,0x3000)),
    @(@(0x1000,0x4001,0x3000)),@(@(0x1000,0x1020,0)),@(@(0x1000,0x1020,0x3001)),
    @(@(0x1000,0x1020,0x4000)),@(@(0x1000,0x1020,0x3000),@(0x1000,0x1080,0x3004)),
    @(@(0x1040,0x1080,0x3004),@(0x1000,0x1020,0x3000)),
    @(@(0x1000,0x1080,0x3000),@(0x1040,0x1080,0x3004)),
    @(@(0x1000,0x1020,0x3000),@(0x2000,0x2000,0x3004)))
foreach($rows in $badRows){
    $rejected=$false
    try {[void][NativeFunctionTable]::Select((New-Table $rows),0x1040,0x4000)}catch{$rejected=$true}
    if(!$rejected){throw 'Invalid or ambiguous table admitted'}
    $checks++
}
foreach($case in @(@{bytes=[byte[]]@();target=0x1000;image=0x4000},@{bytes=[byte[]]@(1);target=0x1000;image=0x4000},
    @{bytes=[byte[]]::new(131073*12);target=0x1000;image=0x4000},@{bytes=$table;target=0;image=0x4000},
    @{bytes=$table;target=0x4000;image=0x4000},@{bytes=$table;target=0x1000;image=0})) {
    $rejected=$false
    try {[void][NativeFunctionTable]::Select($case.bytes,$case.target,$case.image)}catch{$rejected=$true}
    if(!$rejected){throw 'Unbounded table/target admitted'}
    $checks++
}
# Exercise the actual large-table path and the highest image-relative fields.
$rows=@(for($i=0;$i -lt 131072;$i++){
    ,([uint32[]]@([uint32](0x1000+$i*16),[uint32](0x1008+$i*16),[uint32]0x300000))
})
$large=New-Table $rows
$last=[NativeFunctionTable]::Select($large,0x200ff0,0x400000)
if($last.RecordCount -ne 131072 -or $last.Index -ne 131071 -or !$last.ExactEntry -or $last.Begin -ne 0x200ff0){throw 'Large authored table selection differs'}
$checks++
$high=New-Table @(@([uint32]4294967040,[uint32]4294967056,[uint32]4294967040))
$last=[NativeFunctionTable]::Select($high,[uint32]4294967040,[uint32]::MaxValue)
if($last.Index -ne 0 -or !$last.ExactEntry){throw 'Unsigned high RVA differs'}
$checks++
Write-Output "PASS: $checks generated range-selection/table-boundary controls."
