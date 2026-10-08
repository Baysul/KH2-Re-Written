# Independent histogram-derived bit counts and source-traced IMGD scalars.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$StructurePath,
    [Parameter(Mandatory)][string]$PreviousProbePath,
    [Parameter(Mandatory)][string]$PixelDirectory,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
foreach($directory in @((Split-Path -Parent $StructurePath),(Split-Path -Parent $PreviousProbePath),$PixelDirectory,$OutputDirectory)) {
    $null=Assert-PrivateResearchDirectory -Directory $directory -SourceRoot $InstallRoot
}
if((git -C $OpenKhRoot rev-parse HEAD) -ne '7a3b945c538d32c6a285128c98aefba093f52ceb' -or
    (git -C $OpenKhRoot status --porcelain -- OpenKh.Kh2/Imgd.cs OpenKh.Egs/EgsTools.cs)){throw 'Unaudited IMGD/layout reference'}
foreach($path in @($StructurePath,$PreviousProbePath)) {
    if((Get-Item -LiteralPath $path).Length -gt 33554432){throw 'Structure metadata too large'}
}
Add-Type -TypeDefinition @'
using System;
public static class ImageChannelOracle {
    public static object Observe(byte[] bytes) {
        if(bytes.Length<16 || bytes.Length>67108876 || bytes[0]!=82 || bytes[1]!=71 || bytes[2]!=66 || bytes[3]!=65)
            throw new ArgumentException("RGBA bounds/magic");
        int w=BitConverter.ToInt32(bytes,4),h=BitConverter.ToInt32(bytes,8);
        if(w<=0 || h<=0 || w>4096 || h>4096 || 12+(long)w*h*4!=bytes.Length) throw new ArgumentException("RGBA dimensions");
        long[][] hist=new long[4][],bits=new long[4][];long[] equal=new long[3];long zero=0,hidden=0;
        for(int c=0;c<4;c++){hist[c]=new long[256];bits[c]=new long[8];}
        for(int i=12;i<bytes.Length;i+=4) {
            for(int c=0;c<4;c++) hist[c][bytes[i+c]]++;
            if(bytes[i]==bytes[i+1]) equal[0]++;
            if(bytes[i]==bytes[i+2]) equal[1]++;
            if(bytes[i+1]==bytes[i+2]) equal[2]++;
            if(bytes[i+3]==0){zero++;if((bytes[i]|bytes[i+1]|bytes[i+2])!=0)hidden++;}
        }
        // Derive bits from completed distributions, independently of the Rust per-pixel loop.
        for(int c=0;c<4;c++) for(int bit=0;bit<8;bit++) for(int value=0;value<256;value++)
            if((value & (1<<bit))!=0) bits[c][bit]+=hist[c][value];
        return new{pixels=(long)w*h,histograms_rgba=hist,bit_one_counts_rgba=bits,equal_rgb_pairs=equal,
            zero_alpha_pixels=zero,zero_alpha_nonzero_rgb_pixels=hidden};
    }
}
'@
$structure=Get-Content -LiteralPath $StructurePath -Raw|ConvertFrom-Json
$previous=Get-Content -LiteralPath $PreviousProbePath -Raw|ConvertFrom-Json
if(!$structure.fresh_full_scan -or $structure.source_content_identity -ne $previous.source_content_identity -or
    $structure.qualification -ne 'quarantined_structural_observation_only'){throw 'Unbound structure evidence'}
$expected=@($previous.samples|ForEach-Object{$_.replacement_samples}|Where-Object{$_.decoded_image})
if(@($structure.images).Count -ne $expected.Count){throw 'Channel denominator differs'}
$seen=@{};$pixelCount=0L
foreach($image in $structure.images) {
    $key="$($image.index)|$($image.ordinal)|$($image.replacement_ordinal)"
    if($seen.ContainsKey($key)){throw 'Duplicate source image'};$seen[$key]=$true
    $sample=@($previous.samples|Where-Object{$_.index -eq $image.index -and $_.ordinal -eq $image.ordinal})
    $replacement=@($sample.replacement_samples|Where-Object{$_.metadata.ordinal -eq $image.replacement_ordinal})
    if($sample.Count -ne 1 -or $replacement.Count -ne 1){throw 'Absent/ambiguous channel source'}
    $decoded=$replacement[0].decoded_image
    if($decoded.pixels_rgba_sha256 -ne $image.pixels_rgba_sha256 -or $decoded.pixel_decode_profile -ne $image.pixel_decode_profile -or
        $decoded.width -ne $image.width -or $decoded.height -ne $image.height){throw 'Channel pixel/profile identity mismatch'}
    if($decoded.preview_file -notmatch '^sample-[0-9]{2}-replacement-[0-9]{2}\.bmp$'){throw 'Unsafe pixel filename'}
    $path=Join-Path $PixelDirectory ($decoded.preview_file -replace '\.bmp$','.rgba')
    if((Get-Item -LiteralPath $path).Length -gt 67108876){throw 'Pixel input too large'}
    $bytes=[IO.File]::ReadAllBytes($path)
    if($bytes.Length -lt 16 -or [BitConverter]::ToUInt32($bytes,4) -ne $image.width -or
        [BitConverter]::ToUInt32($bytes,8) -ne $image.height){throw 'Retained pixel dimensions differ'}
    $stream=[IO.MemoryStream]::new($bytes,12,$bytes.Length-12,$false)
    try{$hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)).ToLowerInvariant()}finally{$stream.Dispose()}
    if($hash -ne $image.pixels_rgba_sha256){throw 'Retained pixels changed'}
    $observed=[ImageChannelOracle]::Observe($bytes)|ConvertTo-Json -Depth 6|ConvertFrom-Json
    foreach($field in @('pixels','histograms_rgba','bit_one_counts_rgba','equal_rgb_pairs','zero_alpha_pixels','zero_alpha_nonzero_rgb_pixels')) {
        if(($observed.$field|ConvertTo-Json -Depth 6 -Compress) -cne ($image.channels.$field|ConvertTo-Json -Depth 6 -Compress)) {
            throw "Independent channel field differs: $field"
        }
    }
    $pixelCount+=$observed.pixels
}
# Reuse the unchanged owned source/storage verifier; it independently reads
# HED/PKG, executes only pinned public transform code and leaves originals in memory.
$internalCount=0
foreach($internal in $structure.internal_images) {
    $original=@($previous.samples|Where-Object{$_.index -eq $internal.index -and $_.ordinal -eq $internal.ordinal})
    if($original.Count -ne 1 -or $original[0].sha256 -ne $internal.original_sha256){throw 'Internal source identity mismatch'}
    $subset=$previous.PSObject.Copy();$subset.samples=$original
    $subsetPath=Join-Path $OutputDirectory ('internal-{0:D2}-storage.json' -f $internalCount)
    Write-NewResearchFile -Path $subsetPath -Bytes ([Text.Encoding]::UTF8.GetBytes(($subset|ConvertTo-Json -Depth 15)))
    . (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $subsetPath -OpenKhRoot $OpenKhRoot
    # $data is the independently reconstructed original, not a Rust export.
    $record=$original[0].bar.entries[$internal.bar_ordinal]
    if(($record|ConvertTo-Json -Compress) -ne ($internal.bar_entry|ConvertTo-Json -Compress)){throw 'Internal BAR coordinates differ'}
    $memory=[IO.MemoryStream]::new($data,[int]$record.offset,[int]$record.size,$false)
    $reader=[IO.BinaryReader]::new($memory)
    try {
        if($reader.ReadUInt32() -ne 0x44474d49){throw 'Internal image is not IMGD'}
        $values=@{version=$reader.ReadInt32();bitmap_offset=$reader.ReadInt32();bitmap_length=$reader.ReadInt32();clut_offset=$reader.ReadInt32();clut_length=$reader.ReadInt32()}
        $memory.Position=28;$values.width=$reader.ReadInt16();$values.height=$reader.ReadInt16()
        $memory.Position=38;$values.gs_psm=$reader.ReadUInt16();$memory.Position=60;$values.flags=$reader.ReadInt32()
        foreach($field in $values.Keys){if($values[$field] -ne $internal.header.$field){throw "Independent IMGD scalar differs: $field"}}
        $values.swizzled_flag=($values.flags -band 4) -ne 0
        $end=[Math]::Max(64,[Math]::Max($values.bitmap_offset+$values.bitmap_length,$values.clut_offset+$values.clut_length))
        if($values.swizzled_flag -ne $internal.header.swizzled_flag -or $record.size-$end -ne $internal.header.bytes_after_declared_regions){throw 'IMGD range/flag derivation differs'}
        foreach($region in @(@{name='header';offset=0;length=64},@{name='bitmap';offset=$values.bitmap_offset;length=$values.bitmap_length},@{name='clut';offset=$values.clut_offset;length=$values.clut_length})) {
            $memory.Position=$region.offset;$regionBytes=$reader.ReadBytes($region.length)
            $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($regionBytes)).ToLowerInvariant()
            if($hash -ne $internal.($region.name+'_sha256')){throw 'IMGD region fingerprint differs'}
        }
        $flagged=([long]$record.offset+$values.bitmap_offset) -bor 0x20000000
        $matches=@($original[0].replacement_samples|Where-Object{$_.metadata.original_asset_offset -eq $flagged}|ForEach-Object{$_.metadata.ordinal})
        if($flagged -ne $internal.flagged_bitmap_offset -or ($matches|ConvertTo-Json -Compress) -ne ($internal.matching_replacement_ordinals|ConvertTo-Json -Compress)){throw 'IMGD/HD linkage differs'}
    }finally{$reader.Dispose();$memory.Dispose()}
    $internalCount++
}
$result=@{images=@($structure.images).Count;pixels=$pixelCount;internal_imgd_headers=$internalCount;channels_exact=$true;imgd_scalars_ranges_hashes_and_links_exact=$true;installed_semantics='NOT RUN'}
Write-NewResearchFile -Path (Join-Path $OutputDirectory 'independent-structure.json') -Bytes ([Text.Encoding]::UTF8.GetBytes(($result|ConvertTo-Json)))
$result|ConvertTo-Json
