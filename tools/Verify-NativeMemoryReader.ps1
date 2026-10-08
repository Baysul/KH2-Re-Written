# Authored fixture ranges only. Executes only authored/recompiled fixture DLLs.
param([Parameter(Mandatory)][string]$ToolDirectory,[Parameter(Mandatory)][string]$JavaHome,
    [Parameter(Mandatory)][string]$CompilerDirectory,[Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$PeOracleDirectory,[Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'NativeRangeEngine.ps1')
. (Join-Path $PSScriptRoot 'NativeReader.ps1')
$out=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$prior=Assert-PrivateResearchDirectory $PeOracleDirectory $InstallRoot
if (@(Get-ChildItem $out -Force).Count) {throw 'Fresh memory oracle directory required'}
$qualification=Get-Content ($prior+'/conformance.json') -Raw|ConvertFrom-Json
$fixture=Join-Path $prior 'authored.dll';$source=Join-Path $PSScriptRoot 'fixtures/NativeConformance.c'
if ($qualification.verdict -cne 'PASS' -or $qualification.cases.Count -ne 22 -or
    $qualification.fixture_sha256 -cne (Get-FileHash $fixture).Hash -or
    $qualification.fixture_source_sha256 -cne (Get-FileHash $source).Hash) {throw 'Authored fixture provenance differs'}
$report=Get-Content ($prior+'/reader/report.json') -Raw|ConvertFrom-Json
if ((Get-FileHash ($prior+'/reader/report.json')).Hash -cne $qualification.reader_report_sha256) {throw 'Fixture body report differs'}
$row=@($report.findings|Where-Object function_name -CEQ 'FixtureTransform')
if ($row.Count -ne 1) {throw 'Need one authored transform'}
$bytes=[IO.File]::ReadAllBytes($fixture);$memory=[IO.MemoryStream]::new($bytes,$false);$pe=[Reflection.PortableExecutable.PEReader]::new($memory)
try {
    $begin=[long]$row[0].rva
    $end=($row[0].instructions|ForEach-Object {$_.rva+$_.size}|Measure-Object -Maximum).Maximum
    $offset=Get-NativePeOffset $pe.PEHeaders.SectionHeaders $begin 1 $bytes.LongLength
    $last=Get-NativePeOffset $pe.PEHeaders.SectionHeaders ($end-1) 1 $bytes.LongLength
    if ($end-$begin -lt 1 -or $end-$begin -gt 4096 -or $last-$offset -ne $end-$begin-1) {throw 'Authored code span differs'}
    $base=[long]$pe.PEHeaders.PEHeader.ImageBase
} finally {$pe.Dispose();$memory.Dispose()}
$snapshot=Join-Path $out 'authored-transform.bin'
Write-NewResearchFile $snapshot ([byte[]]$bytes[$offset..$last])
$cl=Join-Path $CompilerDirectory 'cl.exe';$link=Join-Path $CompilerDirectory 'link.exe'
if ((Get-FileHash $cl).Hash -cne $qualification.compiler_sha256 -or
    (Get-FileHash $link).Hash -cne $qualification.linker_sha256) {throw 'Fixture compiler identity differs'}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class AuthoredMemoryOracle {
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] public delegate uint Transform(uint value);
    public static uint[][] Compare(string originalPath,string recoveredPath,uint[] values) {
        IntPtr original=NativeLibrary.Load(originalPath),recovered=IntPtr.Zero;
        try {
            recovered=NativeLibrary.Load(recoveredPath);
            var a=Marshal.GetDelegateForFunctionPointer<Transform>(NativeLibrary.GetExport(original,"FixtureTransform"));
            var b=Marshal.GetDelegateForFunctionPointer<Transform>(NativeLibrary.GetExport(recovered,"SelectedRange"));
            var rows=new uint[values.Length][];
            for(int i=0;i<values.Length;i++) rows[i]=new uint[]{values[i],a(values[i]),b(values[i])};
            return rows;
        } finally {if(recovered!=IntPtr.Zero) NativeLibrary.Free(recovered);NativeLibrary.Free(original);}
    }
}
'@
$cases=[Collections.Generic.List[object]]::new();$engineReceipts=[Collections.Generic.List[object]]::new()
$index=0;$firstAddress=$base+$begin
foreach ($address in @($firstAddress,0x180000001000L)) {
    $dir=Join-Path $out "branch-$index";[void](New-Item -ItemType Directory $dir)
    $identity=Invoke-NativeRangeEngine $ToolDirectory $JavaHome $InstallRoot $snapshot $address $dir
    $engineReceipts.Add($identity)
    $found=Get-Content ($dir+'/report.json') -Raw|ConvertFrom-Json
    if ($found.uninterpreted_bytes -ne 0) {throw 'Authored transform contains uninterpreted bytes'}
    $code="typedef unsigned int uint; typedef unsigned int undefined4;`n"+
        ($found.pseudocode -replace '(?m)^(uint|int|undefined4) SelectedRange','__declspec(dllexport) $1 SelectedRange')
    $path=Join-Path $out "recovered-$index.c"
    Write-NewResearchFile $path ([Text.Encoding]::UTF8.GetBytes($code))
    & $cl /nologo /O2 /GS- /c $path "/Fo$out/recovered-$index.obj" *> ($out+"/recovered-$index-compile.log")
    if ($LASTEXITCODE) {throw 'Authored recovered compilation failed'}
    & $link /NOLOGO /DLL /NOENTRY /NODEFAULTLIB /INCREMENTAL:NO "/OUT:$out/recovered-$index.dll" ($out+"/recovered-$index.obj") *> ($out+"/recovered-$index-link.log")
    if ($LASTEXITCODE) {throw 'Authored recovered linking failed'}
    $values=[uint32[]]@(0,7,15,16,255,256,1024,2147483647,2147483648,4294967294,4294967295)
    foreach ($v in [AuthoredMemoryOracle]::Compare($fixture,($out+"/recovered-$index.dll"),$values)) {
        $expected=if ($v[0] -lt 16) {($v[0] -bxor 90)*3} else {($v[0] -band 255)+7}
        if ($v[1] -ne $expected -or $v[2] -ne $expected) {throw 'Authored raw round-trip behavior differs'}
        $cases.Add([pscustomobject]@{relocation=$index;input=$v[0];expected=$expected;original=$v[1];recovered=$v[2]})
    }
    $index++
}
$controls=[Collections.Generic.List[object]]::new()
foreach ($control in @(@{name='rip-read';hex='8B05FA000000C3';type='READ';target=0x10000100L},
    @{name='outside-call';hex='E8FB000000C3';type='UNCONDITIONAL_CALL';target=0x10000100L})) {
    $input=Join-Path $out ($control.name+'.bin');Write-NewResearchFile $input ([Convert]::FromHexString($control.hex))
    $dir=Join-Path $out $control.name;[void](New-Item -ItemType Directory $dir)
    $identity=Invoke-NativeRangeEngine $ToolDirectory $JavaHome $InstallRoot $input 0x10000000 $dir
    $engineReceipts.Add($identity)
    $found=Get-Content ($dir+'/report.json') -Raw|ConvertFrom-Json
    $references=@($found.instructions[0].references|Where-Object { [Convert]::ToInt64($_.address,16) -eq $control.target })
    if ($found.uninterpreted_bytes -ne 0 -or $found.instructions.Count -ne 2 -or $references.Count -ne 1 -or
        $references[0].type -cne $control.type) {throw 'Authored outside reference differs'}
    $controls.Add([pscustomobject]@{name=$control.name;verdict='PASS';outside_memory_remains_unmapped=$true})
}
$bad=Join-Path $out 'truncated.bin';Write-NewResearchFile $bad ([byte[]]@(0x48,0xb8,1))
$dir=Join-Path $out 'truncated';[void](New-Item -ItemType Directory $dir)
$rejected=$false
try {[void](Invoke-NativeRangeEngine $ToolDirectory $JavaHome $InstallRoot $bad 0x10000000 $dir)} catch {$rejected=$true}
if (!$rejected -or (Test-Path ($dir+'/engine-receipt.json')) -or (Test-Path ($dir+'/report.json'))) {throw 'Truncated instruction admitted'}
$controls.Add([pscustomobject]@{name='truncated-entry';verdict='PASS rejection'})
$current=Get-NativeRangeEngineIdentity $ToolDirectory $JavaHome
$receipt=[ordered]@{verdict='PASS';loader_profile=$current.loader_profile;engine_identity=$current;cases=$cases;controls=$controls;
    fixture_source_sha256=(Get-FileHash $source).Hash;fixture_sha256=(Get-FileHash $fixture).Hash;
    compiler_sha256=(Get-FileHash $cl).Hash;linker_sha256=(Get-FileHash $link).Hash;engine_runs=$engineReceipts;
    scope='22 finite authored cases at two addresses; two outside-reference controls; truncated-entry rejection; no game bytes executed or general equivalence claim'}
Write-NewResearchFile ($out+'/conformance.json') ([Text.Encoding]::UTF8.GetBytes(($receipt|ConvertTo-Json -Depth 8)))
Write-Output 'PASS 22 raw decompile/recompile cases, two outside references and truncated-entry rejection.'
