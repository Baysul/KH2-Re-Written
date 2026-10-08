# Fresh independent storage plus unchanged pinned BC3 decode/pitch branches.
# Complete chain envelopes compare, including every layout span and RGBA byte.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$PixelDirectory,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$PublicOraclePath,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$AuthoredDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$destination=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$pixels=Assert-PrivateResearchDirectory $PixelDirectory $InstallRoot
$probe=Get-ResearchCanonicalPath $ProbePath
$null=Assert-PrivateResearchDirectory (Split-Path -Parent $probe) $InstallRoot
if((Get-Item $probe).Length -gt 33554432){throw 'Mip report exceeds bound'}
$report=Get-Content $probe -Raw|ConvertFrom-Json
if(!$report.hd_image_decoding_requested -or !$report.hd_mip_chain_inspection_requested){throw 'Explicit source-bound mip observations required'}
$oracle=Get-ResearchCanonicalPath $PublicOraclePath
$oracleDirectory=Assert-PrivateResearchDirectory (Split-Path -Parent $oracle) $InstallRoot
$provenance=Get-Content (Join-Path $oracleDirectory 'build-provenance.json') -Raw|ConvertFrom-Json
if($provenance.pin -ne '1acf4ebe6330e06b7eb52e8db63d4f824ad77427' -or $provenance.mode -ne 'complete_bc3_mips' -or
    (Get-FileHash $oracle).Hash -ne $provenance.binary_sha256 -or
    (Get-FileHash (Join-Path $PSScriptRoot 'Bc3Reference.cpp')).Hash -ne $provenance.owned_adapter_sha256 -or
    (Get-FileHash (Join-Path $oracleDirectory 'Bc3ReferenceMethods.h')).Hash -ne $provenance.public_method_header_sha256 -or
    (Get-FileHash (Join-Path $oracleDirectory 'Bc3PitchBranch.h')).Hash -ne $provenance.public_pitch_header_sha256){throw 'Public mip executable/provenance differs'}
$inputs=Join-Path $destination 'inputs';$null=New-Item -ItemType Directory $inputs
& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $probe -OpenKhRoot $OpenKhRoot -ReplacementInspectionDirectory $inputs *> (Join-Path $destination 'storage.log')
if($LASTEXITCODE){throw 'Independent mip storage reconstruction failed'}
Add-Type -TypeDefinition @'
using System;using System.IO;using System.Security.Cryptography;using System.Collections.Generic;
public sealed class Bc3MipObservation {
    public uint ordinal,width,height;public ulong encoded_offset,encoded_length;public int rgba_byte_count;public string pixels_rgba_sha256;
}
public static class Bc3MipComparison {
    public static bool Equal(byte[] a,byte[] b)=>a.AsSpan().SequenceEqual(b);
    public static Bc3MipObservation[] Read(byte[] bytes) {
        using var stream=new MemoryStream(bytes,false);using var r=new BinaryReader(stream);
        if(bytes.Length<8 || r.ReadUInt32()!=0x5350494d) throw new Exception("Invalid mip envelope");
        uint count=r.ReadUInt32();if(count<1 || count>13) throw new Exception("Mip count exceeds bound");
        var levels=new List<Bc3MipObservation>();long total=0;ulong end=128;
        for(uint ordinal=0;ordinal<count;++ordinal) {
            var m=new Bc3MipObservation {ordinal=ordinal,width=r.ReadUInt32(),height=r.ReadUInt32(),encoded_offset=r.ReadUInt64(),encoded_length=r.ReadUInt64()};
            if(m.width<1 || m.height<1 || m.width>4096 || m.height>4096 || m.encoded_offset!=end || m.encoded_length<16 || m.encoded_length>33554432) throw new Exception("Mip shape/span exceeds bound");
            end=checked(end+m.encoded_length);m.rgba_byte_count=checked((int)(m.width*m.height*4));total+=m.rgba_byte_count;
            if(total>67108864 || stream.Length-stream.Position<m.rgba_byte_count) throw new Exception("Mip pixel span exceeds bound");
            m.pixels_rgba_sha256=Convert.ToHexString(SHA256.HashData(bytes.AsSpan((int)stream.Position,m.rgba_byte_count))).ToLowerInvariant();
            stream.Position+=m.rgba_byte_count;levels.Add(m);
        }
        if(stream.Position!=stream.Length) throw new Exception("Extra mip envelope bytes");return levels.ToArray();
    }
}
'@
function Compare-Mips([string]$InputPath,[string]$RustPath,[string]$Stem,$Layout){
    $inputCanonical=Get-ResearchCanonicalPath $InputPath;$rustCanonical=Get-ResearchCanonicalPath $RustPath
    $null=Assert-PrivateResearchDirectory (Split-Path -Parent $inputCanonical) $InstallRoot
    $null=Assert-PrivateResearchDirectory (Split-Path -Parent $rustCanonical) $InstallRoot
    if((Get-Item $inputCanonical).Length -gt 33554432 -or (Get-Item $rustCanonical).Length -gt 67109200 -or
        $Layout.profile -ne 'legacy_dds_bc3_mip_chain_v1' -or $Layout.runtime_verified -or $Layout.header_words.Count -ne 31){throw 'Mip input/output/layout bound differs'}
    $publicPath=Join-Path $destination ($Stem+'.public.mips')
    & $oracle $inputCanonical $publicPath *> (Join-Path $destination ($Stem+'.public.log'))
    if($LASTEXITCODE){throw "Public mip oracle failed: $Stem"}
    if((Get-Item $publicPath).Length -gt 67109200){throw 'Public mip envelope exceeds bound'}
    $rustBytes=[IO.File]::ReadAllBytes($rustCanonical);$publicBytes=[IO.File]::ReadAllBytes($publicPath)
    if(![Bc3MipComparison]::Equal($rustBytes,$publicBytes)){throw "Complete mip layout/pixels differ: $Stem"}
    $levels=[Bc3MipComparison]::Read($publicBytes)
    if($levels.Count -ne $Layout.levels.Count){throw 'Observed mip count differs'}
    $total=0L
    for($i=0;$i -lt $levels.Count;$i++){
        foreach($field in @('ordinal','width','height','encoded_offset','encoded_length')){
            if($levels[$i].$field -ne $Layout.levels[$i].$field){throw "Observed mip field differs: $field"}
        }
        $total+=$levels[$i].rgba_byte_count
    }
    if($Layout.rgba_byte_count -ne $total -or $Layout.encoded_length -ne $levels[-1].encoded_offset+$levels[-1].encoded_length -or
        $Layout.storage_padding -ne (Get-Item $inputCanonical).Length-$Layout.encoded_length){throw 'Mip aggregate length/padding differs'}
    $header=[IO.File]::ReadAllBytes($inputCanonical)
    for($i=0;$i -lt 31;$i++){if([BitConverter]::ToUInt32($header,4+$i*4) -ne $Layout.header_words[$i]){throw 'Raw mip header word differs'}}
    @{id=$Stem;levels=$levels;pixels=$total/4;rgba_bytes=$total;complete_bytes_exact=$true;output_sha256=(Get-FileHash $rustCanonical).Hash.ToLowerInvariant()}
}
$results=@();$sampleNumber=0;$pixelCount=0L
foreach($sample in $report.samples){foreach($replacement in $sample.replacement_samples){
    $image=$replacement.decoded_image
    if(!$image -or !$image.mip_chain){continue}
    $chain=$image.mip_chain
    if($image.format -ne 'dds_bc3' -or $image.pixel_decode_profile -ne 'bc3_normalized_nearest_rgba8_v1' -or
        $chain.pixel_decode_profile -ne $image.pixel_decode_profile -or $chain.runtime_verified -or $image.runtime_verified){throw 'Mip image/profile fields differ'}
    $stem='sample-{0:D2}-replacement-{1:D2}' -f $sampleNumber,[int]$replacement.metadata.ordinal
    if($chain.mip_file -cne $stem+'.mips' -or $image.encoded_length -ne $replacement.metadata.declared_decompressed_length){throw 'Mip file/logical identity differs'}
    $result=Compare-Mips (Join-Path $inputs ($stem+'.image')) (Join-Path $pixels ($stem+'.mips')) $stem $chain.layout
    if($result.levels.Count -ne $chain.levels.Count){throw 'Mip pixel observation count differs'}
    for($i=0;$i -lt $result.levels.Count;$i++){
        foreach($field in @('ordinal','rgba_byte_count','pixels_rgba_sha256')){
            if($result.levels[$i].$field -cne $chain.levels[$i].$field){throw 'Mip pixel observation differs'}
        }
    }
    if($image.pixels_rgba_sha256 -cne $result.levels[0].pixels_rgba_sha256){throw 'Base mip fingerprint differs'}
    $pixelCount+=$result.pixels
    if($pixelCount -gt 33554432 -or $results.Count -ge 64){throw 'Mip aggregate observation bound'}
    $results+=$result
};$sampleNumber++}
if(!$results.Count){throw 'No independently compared mip chains'}
$authoredResults=@()
if($AuthoredDirectory){
    $authored=Assert-PrivateResearchDirectory $AuthoredDirectory $InstallRoot
    if((Get-Item (Join-Path $authored 'cases.json')).Length -gt 1048576){throw 'Authored mip manifest exceeds bound'}
    $manifest=Get-Content (Join-Path $authored 'cases.json') -Raw|ConvertFrom-Json
    if($manifest.schema_version -ne 1 -or $manifest.cases.Count -gt 64){throw 'Authored mip corpus differs'}
    foreach($case in $manifest.cases){
        $stem='case-{0:D2}' -f [int]$case.ordinal
        if($case.input_file -cne $stem+'.dds' -or $case.output_file -cne $stem+'.mips' -or $case.profile -ne 'bc3_normalized_nearest_rgba8_v1'){throw 'Authored mip identity/profile differs'}
        $inputPath=Join-Path $authored $case.input_file;$rustPath=Join-Path $authored $case.output_file
        if((Get-FileHash $inputPath).Hash -ne $case.input_sha256 -or (Get-FileHash $rustPath).Hash -ne $case.output_sha256){throw 'Authored mip fingerprint differs'}
        $authoredResults+=Compare-Mips $inputPath $rustPath $stem $case.layout
    }
}
Write-NewResearchFile (Join-Path $destination 'independent-bc3-mips.json') ([Text.Encoding]::UTF8.GetBytes((@{
    verdict='PASS';pin=$provenance.pin;public_binary_sha256=$provenance.binary_sha256;samples=$results;source_pixels=$pixelCount;
    authored_cases=$authoredResults;unchanged_public_decode_and_pitch_branches_executed=$true;whole_directxtex_importer_executed=$false;runtime_verified=$false
}|ConvertTo-Json -Depth 12)))
"PASS: $($results.Count) source BC3 chains / $pixelCount pixels, $($authoredResults.Count) authored cases, complete layouts/pixels exact."
