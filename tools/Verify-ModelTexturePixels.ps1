# Independent source reconstruction/layouts, unchanged public GetData and pure
# memory rearrangement methods, followed by owned indexed RGBA expansion.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$PixelDirectory,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$AuthoredDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$pixelDestination=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$pixelInputDirectory=Assert-PrivateResearchDirectory $PixelDirectory $InstallRoot
$pixelProbePath=Get-ResearchCanonicalPath $ProbePath
$null=Assert-PrivateResearchDirectory (Split-Path -Parent $pixelProbePath) $InstallRoot
if((Get-Item -LiteralPath $pixelProbePath).Length -gt 33554432){throw 'Pixel report exceeds 32 MiB'}
$pixelReport=Get-Content -LiteralPath $pixelProbePath -Raw|ConvertFrom-Json
if(!$pixelReport.model_texture_pixel_inspection_requested -or !$pixelReport.model_texture_pixel_files_exported -or
    $pixelReport.model_texture_pixel_profile -ne 'openkh_model_indexed_getdata_v1'){throw 'Explicit indexed pixel inspection required'}
$layoutDirectory=Join-Path $pixelDestination 'layout'
$null=New-Item -ItemType Directory $layoutDirectory
# Separate process keeps partial Ps2 declarations in a single later assembly.
& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Verify-ModelTextures.ps1') -InstallRoot $InstallRoot -ProbePath $pixelProbePath -OpenKhRoot $OpenKhRoot -OutputDirectory $layoutDirectory *> (Join-Path $pixelDestination 'layout.log')
if($LASTEXITCODE -ne 0){throw 'Independent texture layout preflight failed'}
$pixelPin='7a3b945c538d32c6a285128c98aefba093f52ceb'
$pixelFiles=@('OpenKh.Kh2/ModelTexture.cs','OpenKh.Kh2/Ps2.Reform32.cs','OpenKh.Kh2/Ps2.Reform8.cs','OpenKh.Kh2/Ps2.Reform4.cs','OpenKh.Imaging/PixelFormat.cs')
if((git -C $OpenKhRoot rev-parse HEAD) -ne $pixelPin -or (git -C $OpenKhRoot status --porcelain -- $pixelFiles)){throw 'Pixel sources differ from audited pin'}
function Get-PixelBracedSource([string]$Source,[string]$Declaration){
    $start=$Source.IndexOf($Declaration,[StringComparison]::Ordinal);if($start -lt 0){throw 'Missing pixel declaration'}
    $brace=$Source.IndexOf('{',$start);$depth=0
    for($cursor=$brace;$cursor -lt $Source.Length;$cursor++){
        if($Source[$cursor] -eq '{'){$depth++}
        if($Source[$cursor] -eq '}'){$depth--;if(!$depth){return $Source.Substring($start,$cursor-$start+1)}}
    }
    throw 'Unclosed pixel declaration'
}
$pixelSource=Get-Content (Join-Path $OpenKhRoot $pixelFiles[0]) -Raw
$getData=Get-PixelBracedSource $pixelSource 'public byte[] GetData()'
$getData32=Get-PixelBracedSource $pixelSource 'private static byte[] GetData32bpp('
$pixelAdapter='using OpenKh.Imaging;'+(Get-Content (Join-Path $layoutDirectory 'public-method-adapter.cs') -Raw)
foreach($relative in $pixelFiles[1..4]){$pixelAdapter+=[Environment]::NewLine+(Get-Content (Join-Path $OpenKhRoot $relative) -Raw)}
$pixelAdapter+=@'
namespace OpenKh.Kh2 {
public class IndexedPublicTexture {
    readonly byte[] _data;
    readonly OpenKh.Imaging.Tm2.GsPSM _uploadPixelFormat;
    public struct OwnedSize {public int Width,Height;}
    public OwnedSize Size;
    public OpenKh.Imaging.PixelFormat PixelFormat;
    public IndexedPublicTexture(byte[] data,int width,int height,int depth,int upload) {
        _data=data;Size=new OwnedSize{Width=width,Height=height};
        PixelFormat=depth==8?OpenKh.Imaging.PixelFormat.Indexed8:OpenKh.Imaging.PixelFormat.Indexed4;
        _uploadPixelFormat=(OpenKh.Imaging.Tm2.GsPSM)upload;
    }
'@+$getData+[Environment]::NewLine+$getData32+@'
}}
public static class IndexedPixelExpansion {
    public static byte[] Stored(byte[] payload,int offset,int length) {
        if(offset<0 || length<0 || (long)offset+length>payload.Length)throw new InvalidDataException("stored pixel span");
        byte[] bytes=new byte[length];Array.Copy(payload,offset,bytes,0,length);return bytes;
    }
    public static byte[] Expand(byte[] packed,byte[] clut,int width,int height,int depth) {
        if(width<=0 || height<=0 || width>4096 || height>4096 || (depth!=4 && depth!=8))throw new InvalidDataException("pixel dimensions/depth");
        int count=checked(width*height);if(count*4>67108864 || packed.Length!=count/(8/depth) || (depth==4 && count%2!=0))throw new InvalidDataException("pixel allocation/length");
        if(clut.Length!=(depth==8?1024:64))throw new InvalidDataException("pixel palette length");
        byte[] rgba=new byte[checked(count*4)];
        for(int pixel=0;pixel<count;pixel++) {
            int index=depth==8?packed[pixel]:(packed[pixel/2]>>(pixel%2==0?4:0))&15;
            Array.Copy(clut,index*4,rgba,pixel*4,4);
        }
        return rgba;
    }
}
'@
Write-NewResearchFile (Join-Path $pixelDestination 'public-pixel-adapter.cs') ([Text.Encoding]::UTF8.GetBytes($pixelAdapter))
$pixelHashes=@(foreach($relative in $pixelFiles){@{path=$relative;sha256=(Get-FileHash (Join-Path $OpenKhRoot $relative)).Hash.ToLowerInvariant()}})
Write-NewResearchFile (Join-Path $pixelDestination 'pixel-reference-hashes.json') ([Text.Encoding]::UTF8.GetBytes((@{pin=$pixelPin;files=$pixelHashes;get_data_unchanged_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($getData))).ToLowerInvariant();get_data32_unchanged_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($getData32))).ToLowerInvariant()}|ConvertTo-Json -Depth 8)))
Add-Type -TypeDefinition $pixelAdapter
if($AuthoredDirectory){
    $authoredRoot=Assert-PrivateResearchDirectory $AuthoredDirectory $InstallRoot
    $authoredReportPath=Join-Path $authoredRoot 'cases.json'
    if((Get-Item -LiteralPath $authoredReportPath).Length -gt 1048576){throw 'Authored report too large'}
    $authoredReport=Get-Content -LiteralPath $authoredReportPath -Raw|ConvertFrom-Json
    if($authoredReport.schema_version -ne 1 -or $authoredReport.profile -ne 'openkh_model_indexed_getdata_v1' -or
        $authoredReport.input_provenance -ne 'authored xorshift32 bytes' -or @($authoredReport.cases).Count -ne 16){throw 'Authored case declaration differs'}
    $authoredResults=@();$authoredBytes=0L
    foreach($case in $authoredReport.cases){
        $ordinal=$authoredResults.Count
        if($case.ordinal -ne $ordinal -or $case.input_file -cne ('case-{0:d2}.stored' -f $ordinal) -or $case.output_file -cne ('case-{0:d2}.indices' -f $ordinal)){throw 'Authored case identity/path differs'}
        if($case.width -lt 1 -or $case.width -gt 512 -or $case.height -lt 1 -or $case.height -gt 512 -or
            $case.depth -notin @(4,8) -or $case.upload_psm -lt 0 -or $case.upload_psm -gt 63){throw 'Authored case bounds'}
        if($case.upload_psm -eq 0 -and ($case.width%128 -ne 0 -or $case.height%$(if($case.depth -eq 8){64}else{128}) -ne 0)){throw 'Authored partial pages'}
        $inputPath=Join-Path $authoredRoot $case.input_file;$outputPath=Join-Path $authoredRoot $case.output_file
        if((Get-Item -LiteralPath $inputPath).Length -gt 262144 -or (Get-Item -LiteralPath $outputPath).Length -gt 262144){throw 'Authored input bytes bound'}
        $inputBytes=[IO.File]::ReadAllBytes($inputPath);$actualBytes=[IO.File]::ReadAllBytes($outputPath)
        if($inputBytes.Length -ne [long]$case.width*$case.height/(8/$case.depth) -or $case.bytes -ne $inputBytes.Length){throw 'Authored input length'}
        $expectedBytes=[OpenKh.Kh2.IndexedPublicTexture]::new($inputBytes,$case.width,$case.height,$case.depth,$case.upload_psm).GetData()
        if(![Linq.Enumerable]::SequenceEqual($expectedBytes,$actualBytes)){throw 'Authored indexed bytes differ from public GetData'}
        foreach($part in @(@{bytes=$inputBytes;hash=$case.input_sha256},@{bytes=$actualBytes;hash=$case.output_sha256})){
            if([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([byte[]]$part.bytes)).ToLowerInvariant() -cne $part.hash){throw 'Authored case fingerprint differs'}
        }
        $authoredBytes+=$actualBytes.Length
        $authoredResults+=@{ordinal=$ordinal;bytes=$actualBytes.Length;exact=$true}
    }
    Write-NewResearchFile (Join-Path $pixelDestination 'authored-boundary-comparison.json') ([Text.Encoding]::UTF8.GetBytes((@{verdict='PASS';cases=$authoredResults;packed_bytes=$authoredBytes;installed_coverage=$false}|ConvertTo-Json -Depth 6)))
}
$independentLayouts=Get-Content (Join-Path $layoutDirectory 'independent-model-textures.json') -Raw|ConvertFrom-Json
$pixelResults=@();$totalPixelRgba=0L;$totalPixelCount=0L
for($sampleNumber=0;$sampleNumber -lt $pixelReport.samples.Count;$sampleNumber++) {
    $pixelSample=$pixelReport.samples[$sampleNumber]
    $storagePath=Join-Path $layoutDirectory ('skeleton/storage-{0:d2}.json' -f $sampleNumber)
    $sourceOriginal=&{. (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $storagePath -OpenKhRoot $OpenKhRoot|Out-Null;,$data}
    $expectedPixelCount=0
    foreach($layoutRecord in @($independentLayouts.samples|Where-Object{$_.sample -eq $sampleNumber})) {
        $payload=[TextureLayoutOracle]::Payload($sourceOriginal,[int]$layoutRecord.bar_ordinal)
        foreach($image in $layoutRecord.layout.images) {
            $expectedPixelCount++
            $observations=@($pixelSample.model_texture_pixels|Where-Object{$_.bar_ordinal -eq $layoutRecord.bar_ordinal -and $_.environment_ordinal -eq $image.environment_ordinal})
            if($observations.Count -ne 1){throw 'Missing/duplicate pixel observation'}
            $observation=$observations[0]
            foreach($field in @('width','height','indexed_bits_per_pixel','upload_psm')){if([string]$observation.$field -cne [string]$image.$field){throw "Pixel metadata differs: $field"}}
            if($observation.pixel_decode_profile -ne 'openkh_model_indexed_getdata_v1' -or $observation.runtime_verified -ne $false){throw 'Pixel profile/runtime assertion differs'}
            if($image.upload_psm -eq 0 -and ($image.width%128 -ne 0 -or $image.height%$(if($image.indexed_bits_per_pixel -eq 8){64}else{128}) -ne 0)){throw 'Partial public swizzle pages outside profile'}
            $stored=[IndexedPixelExpansion]::Stored($payload,[int]$image.data_offset,[int]$image.data_length)
            $publicTexture=[OpenKh.Kh2.IndexedPublicTexture]::new($stored,$image.width,$image.height,$image.indexed_bits_per_pixel,$image.upload_psm)
            $indices=$publicTexture.GetData();$clut=[byte[]]@($image.clut_rgba|ForEach-Object{$_})
            $rgba=[IndexedPixelExpansion]::Expand($indices,$clut,$image.width,$image.height,$image.indexed_bits_per_pixel)
            $totalPixelRgba+=$rgba.Length;$totalPixelCount+=[long]$image.width*$image.height
            if($totalPixelRgba -gt 134217728 -or $pixelResults.Count -ge 64){throw 'Pixel aggregate bound'}
            if($observation.rgba_byte_count -ne $rgba.Length){throw 'RGBA byte count differs'}
            $encoded=[byte[]]::new(12+$rgba.Length);[Text.Encoding]::ASCII.GetBytes('RGBA').CopyTo($encoded,0)
            [BitConverter]::GetBytes([uint32]$image.width).CopyTo($encoded,4);[BitConverter]::GetBytes([uint32]$image.height).CopyTo($encoded,8);$rgba.CopyTo($encoded,12)
            $stem='model-texture-{0:d2}-{1:d2}-{2:d3}' -f $sampleNumber,$layoutRecord.bar_ordinal,$image.environment_ordinal
            foreach($part in @(@{extension='indices';bytes=$indices;hash_field='packed_indices_sha256'},@{extension='clut';bytes=$clut;hash_field='clut_rgba_sha256'},@{extension='rgba';bytes=$encoded;hash_bytes=$rgba;hash_field='pixels_rgba_sha256'})) {
                $name=$stem+'.'+$part.extension
                if($observation.($part.extension+'_file') -cne $name){throw 'Pixel filename differs'}
                $path=Join-Path $pixelInputDirectory $name
                if((Get-Item -LiteralPath $path).Length -gt 67108876){throw 'Pixel comparison input too large'}
                $actual=[IO.File]::ReadAllBytes($path)
                $hashBytes=if($part.ContainsKey('hash_bytes')){$part.hash_bytes}else{$part.bytes}
                $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([byte[]]$hashBytes)).ToLowerInvariant()
                if($hash -ne $observation.($part.hash_field) -or ![Linq.Enumerable]::SequenceEqual([byte[]]$part.bytes,$actual)){throw "Independent model texture bytes differ: $name"}
                Write-NewResearchFile (Join-Path $pixelDestination $name) ([byte[]]$part.bytes)
            }
            $pixelResults+=@{sample=$sampleNumber;bar_ordinal=$layoutRecord.bar_ordinal;environment_ordinal=$image.environment_ordinal;width=$image.width;height=$image.height;indexed_bits_per_pixel=$image.indexed_bits_per_pixel;upload_psm=$image.upload_psm;pixels=[long]$image.width*$image.height;packed_indices_palette_rgba_exact=$true}
        }
    }
    if($expectedPixelCount -ne @($pixelSample.model_texture_pixels).Count){throw 'Extra/missing model pixel outputs'}
}
if(!$pixelResults.Count){throw 'No qualified model pixels'}
Write-NewResearchFile (Join-Path $pixelDestination 'independent-model-texture-pixels.json') ([Text.Encoding]::UTF8.GetBytes((@{verdict='PASS';pin=$pixelPin;profile='openkh_model_indexed_getdata_v1';images=$pixelResults;pixels=$totalPixelCount;rgba_bytes=$totalPixelRgba;get_data_and_reform_methods_executed_unchanged=$true;rgba_expansion='owned high-first indexed CPU';full_model_texture_reader_executed=$false;runtime_verified=$false}|ConvertTo-Json -Depth 10)))
"PASS: $($pixelResults.Count) model texture images / $totalPixelCount pixels; indices, palettes and complete RGBA agree independently."
