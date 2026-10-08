# Generated synthetic records/bytes only; no installed game or engine required.
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
. (Join-Path $PSScriptRoot 'NativeReader.ps1')
$root=Get-ResearchCanonicalPath (Get-Location).Path
$identity='f766ec05d88a3d61965c8d65c355b0180ec4bc7c5bb8156e7b1a20907ebd562b'
$hash='bbeebd879e1dff6918546dc0c179fdde505f2a21591c9a9c96e36b054ec5af83'
# Independent Python hashlib/struct calculation over thirteen one-byte Z files.
$names=@('KINGDOM HEARTS II FINAL MIX.exe')
foreach ($pair in @('first','second','third','fourth','fifth','sixth')) {
    foreach ($extension in @('hed','pkg')) {$names += "Image/dt/kh2_$pair.$extension"}
}
$manifest=[pscustomobject]@{schema_version=1;qualification='quarantined_unqualified';canonical_source_path=$root;
    content_identity=$identity;files=@($names | ForEach-Object {[pscustomobject]@{relative_path=$_;size=1;sha256=$hash}})}
$runtime=[pscustomobject]@{os_module_metadata_enumerated=$true;process_memory_modified=$false;custom_game_state_or_code_memory_read=$false;
    processes=@([pscustomobject]@{source_root_modules=@([pscustomobject]@{relative_path='steam_api64.dll';size=1;sha256=$hash})})}
if ((Assert-NativeSharedModuleEvidence $manifest $runtime $root 'steam_api64.dll' 1 $hash $identity) -cne $identity) {
    throw 'Synthetic shared-module commitment differs'
}
$checks=1
foreach ($case in @('unsupported module','unobserved module','changed observed hash','changed observed size','incomplete candidate',
    'changed candidate hash','wrong selected identity','supported qualification','unrecorded modules','memory modified','code memory read')) {
    $m=$manifest | ConvertTo-Json -Depth 8 | ConvertFrom-Json
    $r=$runtime | ConvertTo-Json -Depth 8 | ConvertFrom-Json
    $relative='steam_api64.dll';$selected=$identity
    switch ($case) {
        'unsupported module' {$relative='authored-other.exe'}
        'unobserved module' {$relative='KINGDOM HEARTS HD 1.5+2.5 ReMIX.exe'}
        'changed observed hash' {$r.processes[0].source_root_modules[0].sha256='0'*64}
        'changed observed size' {$r.processes[0].source_root_modules[0].size=2}
        'incomplete candidate' {$m.files=@($m.files | Select-Object -Skip 1)}
        'changed candidate hash' {$m.files[0].sha256='0'*64}
        'wrong selected identity' {$selected='0'*64}
        'supported qualification' {$m.qualification='supported'}
        'unrecorded modules' {$r.os_module_metadata_enumerated=$false}
        'memory modified' {$r.process_memory_modified=$true}
        'code memory read' {$r.custom_game_state_or_code_memory_read=$true}
    }
    $rejected=$false
    try {[void](Assert-NativeSharedModuleEvidence $m $r $root $relative 1 $hash $selected)} catch {$rejected=$true}
    if (!$rejected) {throw "Invalid synthetic shared evidence admitted: $case"}
    $checks++
}
$sections=@([pscustomobject]@{VirtualAddress=0x1000;SizeOfRawData=2048;PointerToRawData=0})
$directory=[pscustomobject]@{RelativeVirtualAddress=0x1100;Size=0x100}
$bytes=[byte[]]::new(2048)
function Put-U32([byte[]]$Target,[int]$Offset,[uint32]$Value) {
    [Array]::Copy([BitConverter]::GetBytes($Value),0,$Target,$Offset,4)
}
foreach ($entry in @(@(0x110,1),@(0x114,2),@(0x118,2),@(0x11c,0x1300),@(0x120,0x1320),@(0x124,0x1340),
    @(0x300,0x1560),@(0x304,0x1570),@(0x320,0x1400),@(0x324,0x1410))) {
    Put-U32 $bytes $entry[0] $entry[1]
}
$bytes[0x342]=1
[Array]::Copy([Text.Encoding]::ASCII.GetBytes("Alpha`0"),0,$bytes,0x400,6)
[Array]::Copy([Text.Encoding]::ASCII.GetBytes("Beta`0"),0,$bytes,0x410,5)
$alpha=Get-NativeNamedExport $bytes $sections $directory 'Alpha'
$beta=Get-NativeNamedExport $bytes $sections $directory 'Beta'
if ($alpha.rva -ne 0x1560 -or $alpha.ordinal -ne 1 -or $beta.rva -ne 0x1570 -or $beta.ordinal -ne 2) {
    throw 'Synthetic export literal differs'
}
$checks+=2
foreach ($case in @('absent name','ordinal overflow','forwarder','missing export','name pointer','duplicate name','table budget','nonprintable name','short header','ambiguous sections')) {
    $b=[byte[]]$bytes.Clone();$s=$sections;$d=$directory;$name='Alpha'
    switch ($case) {
        'absent name' {$name='Missing'}
        'ordinal overflow' {$b[0x340]=2}
        'forwarder' {Put-U32 $b 0x300 0x1150}
        'missing export' {Put-U32 $b 0x300 0}
        'name pointer' {Put-U32 $b 0x320 0x1800}
        'duplicate name' {Put-U32 $b 0x324 0x1400}
        'table budget' {Put-U32 $b 0x118 65537}
        'nonprintable name' {$b[0x400]=127}
        'short header' {$d=[pscustomobject]@{RelativeVirtualAddress=0x1100;Size=39}}
        'ambiguous sections' {$s=@($sections+$sections)}
    }
    $rejected=$false
    try {[void](Get-NativeNamedExport $b $s $d $name)} catch {$rejected=$true}
    if (!$rejected) {throw "Invalid synthetic named export admitted: $case"}
    $checks++
}
Write-Output "PASS: $checks synthetic shared-module evidence and named-export checks."
