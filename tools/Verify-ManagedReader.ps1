param(
    [Parameter(Mandatory)][string]$ToolDirectory,
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$output = Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$tool = Initialize-ManagedReader $ToolDirectory
$fixturePath = Join-Path $output 'authored-fixture.dll'
$roundTripPath = Join-Path $output 'authored-round-trip.dll'
$sourcePath = Join-Path $PSScriptRoot 'fixtures/ManagedConformance.cs'
foreach ($name in @('authored-fixture.dll','authored-round-trip.dll','authored-decompiled.cs','conformance.json')) {
    if (Test-Path -LiteralPath (Join-Path $output $name)) { throw 'Conformance output already exists' }
}
Add-Type -TypeDefinition (Get-Content -LiteralPath $sourcePath -Raw) -OutputAssembly $fixturePath
$bytes = [IO.File]::ReadAllBytes($fixturePath)
$context = New-StaticDecompiler $bytes (Join-Path $output 'authored-input.dll')
try {
    $text = $context.decompiler.DecompileTypeAsString(
        [ICSharpCode.Decompiler.TypeSystem.FullTypeName]::new('RewriteResearchConformance.BranchFixture'))
} finally {$context.module.Dispose();$context.memory.Dispose()}
Write-NewResearchFile (Join-Path $output 'authored-decompiled.cs') ([Text.Encoding]::UTF8.GetBytes($text))
Add-Type -TypeDefinition ($text.Replace('RewriteResearchConformance','RewriteResearchRoundTrip')) -OutputAssembly $roundTripPath
# Only these two authored fixture assemblies are executed for the oracle.
$original = [Reflection.Assembly]::LoadFrom($fixturePath).GetType('RewriteResearchConformance.BranchFixture')
$recompiled = [Reflection.Assembly]::LoadFrom($roundTripPath).GetType('RewriteResearchRoundTrip.BranchFixture')
$cases = [Collections.Generic.List[object]]::new()
foreach ($value in @([int]::MinValue,-17,-1,0,7,15,16,255,256,1024,[int]::MaxValue)) {
    foreach ($name in @('Classify','Label')) {
        $expected = $original.GetMethod($name).Invoke($null, [object[]]@([int]$value))
        $actual = $recompiled.GetMethod($name).Invoke($null, [object[]]@([int]$value))
        if ($expected -cne $actual) { throw 'Synthetic decompile/recompile behavior differs' }
        $cases.Add([pscustomobject]@{method=$name;input=$value;expected=$expected;actual=$actual})
    }
}
$result = [ordered]@{verdict='PASS';tool=$tool;source_sha256=(Get-FileHash $sourcePath -Algorithm SHA256).Hash;
    fixture_sha256=([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)));
    decompiled_sha256=([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($text))));
    cases=$cases;scope='Two authored methods/22 finite cases only; no target game execution or general decompiler correctness claim'}
Write-NewResearchFile (Join-Path $output 'conformance.json') ([Text.Encoding]::UTF8.GetBytes(($result | ConvertTo-Json -Depth 6)))
Write-Output 'PASS: pinned managed reader decompile/recompile oracle, 22 authored cases.'
