# Only authored fixture binaries are compiled/loaded/executed by this oracle.
param(
    [Parameter(Mandatory)][string]$ToolDirectory,
    [Parameter(Mandatory)][string]$JavaHome,
    [Parameter(Mandatory)][string]$CompilerDirectory,
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$output=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
if (@(Get-ChildItem -LiteralPath $output -Force).Count) {throw 'Native oracle requires a fresh empty private directory'}
$source=Join-Path $PSScriptRoot 'fixtures/NativeConformance.c'
$cl=Join-Path $CompilerDirectory 'cl.exe'
$link=Join-Path $CompilerDirectory 'link.exe'
function Compile-Fixture([string]$SourcePath,[string]$Stem) {
    & $cl /nologo /O2 /GS- /c $SourcePath "/Fo$(Join-Path $output "$Stem.obj")" *> (Join-Path $output "$Stem-compile.log")
    if ($LASTEXITCODE) {throw 'Authored fixture compilation failed'}
    & $link /NOLOGO /DLL /NOENTRY /NODEFAULTLIB /INCREMENTAL:NO "/OUT:$(Join-Path $output "$Stem.dll")" (Join-Path $output "$Stem.obj") *> (Join-Path $output "$Stem-link.log")
    if ($LASTEXITCODE) {throw 'Authored fixture link failed'}
}
Compile-Fixture $source 'authored'
$fixture=Join-Path $output 'authored.dll'
$bytes=[IO.File]::ReadAllBytes($fixture)
$memory=[IO.MemoryStream]::new($bytes,$false)
$pe=[Reflection.PortableExecutable.PEReader]::new($memory)
function Map-FixtureRva([uint32]$Rva) {
    $sections=@($pe.PEHeaders.SectionHeaders | Where-Object {$Rva -ge $_.VirtualAddress -and [long]$Rva -lt [long]$_.VirtualAddress+$_.SizeOfRawData})
    if ($sections.Count -ne 1) {throw 'Unmapped authored fixture RVA'}
    return [int]($sections[0].PointerToRawData+$Rva-$sections[0].VirtualAddress)
}
try {
    $export=Map-FixtureRva $pe.PEHeaders.PEHeader.ExportTableDirectory.RelativeVirtualAddress
    $nameCount=[BitConverter]::ToUInt32($bytes,$export+24)
    if ($nameCount -ne 2) {throw 'Unexpected authored fixture export count'}
    $functions=Map-FixtureRva ([BitConverter]::ToUInt32($bytes,$export+28))
    $names=Map-FixtureRva ([BitConverter]::ToUInt32($bytes,$export+32))
    $ordinals=Map-FixtureRva ([BitConverter]::ToUInt32($bytes,$export+36))
    $rows=@(for ($i=0;$i -lt $nameCount;$i++) {
        $nameOffset=Map-FixtureRva ([BitConverter]::ToUInt32($bytes,$names+4*$i))
        $length=0;while ($bytes[$nameOffset+$length]) {$length++}
        $name=[Text.Encoding]::ASCII.GetString($bytes,$nameOffset,$length)
        if ($name -notin @('FixtureBias','FixtureTransform')) {throw 'Unexpected fixture export'}
        $ordinal=[BitConverter]::ToUInt16($bytes,$ordinals+2*$i)
        $rva=[BitConverter]::ToUInt32($bytes,$functions+4*$ordinal)
        $offset=Map-FixtureRva $rva
        $prefix=[byte[]]::new(32);[Array]::Copy($bytes,$offset,$prefix,0,32)
        [pscustomobject]@{name=$name;rva=$rva;file_offset=$offset;prefix_size=32;
            prefix_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($prefix)).ToLowerInvariant()}
    })
} finally {$pe.Dispose();$memory.Dispose()}
$selection=[ordered]@{executable_sha256=(Get-FileHash $fixture).Hash.ToLowerInvariant();methods=$rows}
Write-NewResearchFile (Join-Path $output 'selection.json') ([Text.Encoding]::UTF8.GetBytes(($selection | ConvertTo-Json -Depth 5)))
$readerOutput=Join-Path $output 'reader'
[void](New-Item -ItemType Directory -Path $readerOutput)
& (Join-Path $PSScriptRoot 'Invoke-NativeReader.ps1') -ToolDirectory $ToolDirectory -JavaHome $JavaHome -InstallRoot $InstallRoot -InputFile $fixture -SelectionPath (Join-Path $output 'selection.json') -OutputDirectory $readerOutput
$report=Get-Content (Join-Path $readerOutput 'report.json') -Raw | ConvertFrom-Json
$bias=@($report.findings | Where-Object function_name -eq 'FixtureBias')
$transform=@($report.findings | Where-Object function_name -eq 'FixtureTransform')
if ($bias.Count -ne 1 -or $transform.Count -ne 1) {throw 'Authored export was not recovered'}
$globalNames=@([regex]::Matches($bias[0].pseudocode,'\b_DAT_[0-9a-fA-F]+\b').Value | Select-Object -Unique)
if ($globalNames.Count -ne 1) {throw 'Expected one authored getter data symbol'}
# Recompilation supplies the fixture's known global initial value and scalar
# typedefs; these are explicit harness context, not decompiler inference.
$roundTrip="typedef unsigned int uint; typedef unsigned int undefined4;`nstatic volatile uint $($globalNames[0]) = 23u;`n"+
    ($bias[0].pseudocode -replace 'undefined4 FixtureBias','__declspec(dllexport) undefined4 FixtureBias')+
    ($transform[0].pseudocode -replace 'int FixtureTransform','__declspec(dllexport) int FixtureTransform')
Write-NewResearchFile (Join-Path $output 'authored-recompiled.c') ([Text.Encoding]::UTF8.GetBytes($roundTrip))
Compile-Fixture (Join-Path $output 'authored-recompiled.c') 'round-trip'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class NativeFixtureOracle {
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] public delegate uint Transform(uint value);
    [UnmanagedFunctionPointer(CallingConvention.Cdecl)] public delegate uint Bias();
    public static uint[][] Compare(string originalPath, string recoveredPath, uint[] values) {
        IntPtr original = NativeLibrary.Load(originalPath);
        IntPtr recovered = IntPtr.Zero;
        try {
            recovered = NativeLibrary.Load(recoveredPath);
            var ot = Marshal.GetDelegateForFunctionPointer<Transform>(NativeLibrary.GetExport(original,"FixtureTransform"));
            var rt = Marshal.GetDelegateForFunctionPointer<Transform>(NativeLibrary.GetExport(recovered,"FixtureTransform"));
            var ob = Marshal.GetDelegateForFunctionPointer<Bias>(NativeLibrary.GetExport(original,"FixtureBias"));
            var rb = Marshal.GetDelegateForFunctionPointer<Bias>(NativeLibrary.GetExport(recovered,"FixtureBias"));
            var rows = new uint[values.Length][];
            for (int i=0; i<values.Length; i++) rows[i]=new uint[]{values[i],ot(values[i]),rt(values[i]),ob(),rb()};
            return rows;
        } finally {
            if (recovered != IntPtr.Zero) NativeLibrary.Free(recovered);
            NativeLibrary.Free(original);
        }
    }
}
'@
$values=[uint32[]]@(0,7,15,16,255,256,1024,2147483647,2147483648,4294967294,4294967295)
$cases=@(foreach ($row in [NativeFixtureOracle]::Compare($fixture,(Join-Path $output 'round-trip.dll'),$values)) {
    $expected=if ($row[0] -lt 16) {($row[0] -bxor 90)*3} else {($row[0] -band 255)+7}
    if ($row[1] -ne $expected -or $row[2] -ne $expected -or $row[3] -ne 23 -or $row[4] -ne 23) {throw 'Native fixture behavior differs'}
    [pscustomobject]@{method='FixtureTransform';input=$row[0];expected=$expected;original=$row[1];recompiled=$row[2]}
    [pscustomobject]@{method='FixtureBias';input=$null;expected=23;original=$row[3];recompiled=$row[4]}
})
$receipt=[ordered]@{verdict='PASS';cases=$cases;fixture_source_sha256=(Get-FileHash $source).Hash;
    fixture_sha256=(Get-FileHash $fixture).Hash;reader_report_sha256=(Get-FileHash (Join-Path $readerOutput 'report.json')).Hash;
    reader_script_sha256=(Get-FileHash (Join-Path $PSScriptRoot 'NativeSelected.java')).Hash;
    compiler_sha256=(Get-FileHash $cl).Hash;linker_sha256=(Get-FileHash $link).Hash;
    scope='22 finite cases; known authored global initializer/typedef context supplied; no game code executed or general native-decompiler correctness claim'}
Write-NewResearchFile (Join-Path $output 'conformance.json') ([Text.Encoding]::UTF8.GetBytes(($receipt | ConvertTo-Json -Depth 6)))
Write-Output 'PASS: native decompile/recompile oracle, 22 authored cases.'
