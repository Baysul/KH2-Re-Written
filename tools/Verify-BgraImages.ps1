# Fresh independent replacement storage plus unchanged pinned public scanline
# branches. Complete private RGBA buffers compare; installed rendering is open.
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
if((Get-Item $probe).Length -gt 33554432){throw 'BGRA report exceeds bound'}
$report=Get-Content $probe -Raw|ConvertFrom-Json
if(!$report.hd_image_decoding_requested){throw 'Explicit source-bound HD pixel observations required'}
$oracle=Get-ResearchCanonicalPath $PublicOraclePath
$oracleDirectory=Assert-PrivateResearchDirectory (Split-Path -Parent $oracle) $InstallRoot
$provenance=Get-Content (Join-Path $oracleDirectory 'build-provenance.json') -Raw|ConvertFrom-Json
if($provenance.pin -ne '1acf4ebe6330e06b7eb52e8db63d4f824ad77427' -or
    (Get-FileHash $oracle).Hash -ne $provenance.binary_sha256 -or
    (Get-FileHash (Join-Path $PSScriptRoot 'BgraReference.cpp')).Hash -ne $provenance.owned_adapter_sha256 -or
    (Get-FileHash (Join-Path $oracleDirectory 'BgraReferenceBranches.h')).Hash -ne $provenance.public_branch_header_sha256){
    throw 'Public BGRA executable/provenance differs'
}
$inputs=Join-Path $destination 'inputs';$null=New-Item -ItemType Directory $inputs
& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $probe -OpenKhRoot $OpenKhRoot -ReplacementInspectionDirectory $inputs *> (Join-Path $destination 'storage.log')
if($LASTEXITCODE){throw 'Independent BGRA storage reconstruction failed'}
Add-Type -TypeDefinition 'using System;public static class BgraBufferComparison {public static bool Equal(byte[] a,byte[] b)=>a.AsSpan().SequenceEqual(b);}'
function Compare-BgraBuffer([string]$InputPath,[string]$RustPath,[string]$Stem,[long]$Width,[long]$Height){
    if($Width -lt 1 -or $Height -lt 1 -or $Width -gt 4096 -or $Height -gt 4096){throw 'BGRA dimensions outside bound'}
    $inputCanonical=Get-ResearchCanonicalPath $InputPath;$rustCanonical=Get-ResearchCanonicalPath $RustPath
    $null=Assert-PrivateResearchDirectory (Split-Path -Parent $inputCanonical) $InstallRoot
    $null=Assert-PrivateResearchDirectory (Split-Path -Parent $rustCanonical) $InstallRoot
    if((Get-Item $inputCanonical).Length -gt 33554432 -or (Get-Item $rustCanonical).Length -ne 12+$Width*$Height*4){throw 'BGRA input/output size differs'}
    $publicPath=Join-Path $destination ($Stem+'.public.rgba')
    & $oracle $inputCanonical $publicPath *> (Join-Path $destination ($Stem+'.public.log'))
    if($LASTEXITCODE){throw "Public BGRA branch oracle failed: $Stem"}
    if((Get-Item $publicPath).Length -ne 12+$Width*$Height*4){throw 'Public RGBA size differs'}
    $rustBytes=[IO.File]::ReadAllBytes($rustCanonical);$publicBytes=[IO.File]::ReadAllBytes($publicPath)
    if([Text.Encoding]::ASCII.GetString($publicBytes,0,4) -ne 'RGBA' -or
        [BitConverter]::ToUInt32($publicBytes,4) -ne $Width -or [BitConverter]::ToUInt32($publicBytes,8) -ne $Height -or
        ![BgraBufferComparison]::Equal($rustBytes,$publicBytes)){throw "Complete BGRA/RGBA bytes differ: $Stem"}
    $pixelStream=[IO.MemoryStream]::new($rustBytes,12,$rustBytes.Length-12,$false)
    try{$hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($pixelStream)).ToLowerInvariant()}
    finally{$pixelStream.Dispose()}
    @{id=$Stem;width=$Width;height=$Height;pixels=$Width*$Height;rgba_sha256=$hash;complete_bytes_exact=$true}
}
$results=@();$sampleNumber=0;$pixelCount=0L
foreach($sample in $report.samples){foreach($replacement in $sample.replacement_samples){
    $image=$replacement.decoded_image
    if(!$image -or $image.format -ne 'dds_bgra8' -or $image.pixel_decode_profile -ne 'legacy_dds_bgra8_straight_rgba8_v1' -or
        $image.runtime_verified -or $image.color_profile_transform_applied -or $image.rgba_byte_count -ne [long]$image.width*$image.height*4){throw 'BGRA image profile/fields differ'}
    $pixelCount+=[long]$image.width*$image.height
    if($pixelCount -gt 33554432 -or $results.Count -ge 64){throw 'BGRA aggregate observation bound'}
    $stem='sample-{0:D2}-replacement-{1:D2}' -f $sampleNumber,[int]$replacement.metadata.ordinal
    if($image.preview_file -cne $stem+'.bmp'){throw 'BGRA preview identity differs'}
    $inputPath=Join-Path $inputs ($stem+'.image')
    if($image.encoded_length -ne $replacement.metadata.declared_decompressed_length -or
        $image.storage_padding -ne (Get-Item $inputPath).Length-$image.encoded_length){throw 'BGRA logical length/padding differs'}
    $result=Compare-BgraBuffer $inputPath (Join-Path $pixels ($stem+'.rgba')) $stem $image.width $image.height
    if($result.rgba_sha256 -cne $image.pixels_rgba_sha256){throw 'BGRA probe pixel fingerprint differs'}
    $results+=$result
};$sampleNumber++}
if(!$results.Count){throw 'No independently compared BGRA observations'}
$authoredResults=@()
if($AuthoredDirectory){
    $authored=Assert-PrivateResearchDirectory $AuthoredDirectory $InstallRoot
    if((Get-Item (Join-Path $authored 'cases.json')).Length -gt 1048576){throw 'Authored BGRA manifest exceeds bound'}
    $manifest=Get-Content (Join-Path $authored 'cases.json') -Raw|ConvertFrom-Json
    if($manifest.schema_version -ne 1 -or $manifest.cases.Count -gt 64){throw 'Authored BGRA corpus differs'}
    foreach($case in $manifest.cases){
        $stem='case-{0:D2}' -f [int]$case.ordinal
        if($case.input_file -cne $stem+'.dds' -or $case.output_file -cne $stem+'.rgba' -or $case.profile -ne 'legacy_dds_bgra8_straight_rgba8_v1'){throw 'Authored BGRA file/profile differs'}
        $inputPath=Join-Path $authored $case.input_file;$rustPath=Join-Path $authored $case.output_file
        if((Get-FileHash $inputPath).Hash -ne $case.input_sha256 -or (Get-FileHash $rustPath).Hash -ne $case.output_sha256){throw 'Authored BGRA input/evidence fingerprint differs'}
        $authoredResults+=Compare-BgraBuffer $inputPath $rustPath $stem $case.width $case.height
    }
}
Write-NewResearchFile (Join-Path $destination 'independent-bgra-images.json') ([Text.Encoding]::UTF8.GetBytes((@{
    verdict='PASS';pin=$provenance.pin;public_binary_sha256=$provenance.binary_sha256;samples=$results;source_pixels=$pixelCount;
    authored_cases=$authoredResults;unchanged_public_scanline_branches_executed=$true;whole_directxtex_importer_executed=$false;runtime_verified=$false
}|ConvertTo-Json -Depth 12)))
"PASS: $($results.Count) BGRA source images / $pixelCount pixels, $($authoredResults.Count) authored cases, complete RGBA buffers exact."
