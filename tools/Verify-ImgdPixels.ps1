# Pinned unchanged palette method plus independent indexed pixel expansion.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$StructurePath,
    [Parameter(Mandatory)][string]$PreviousProbePath,
    [Parameter(Mandatory)][string]$PixelDirectory,
    [Parameter(Mandatory)][string]$ImgdPixelDirectory,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$null=Assert-PrivateResearchDirectory -Directory $ImgdPixelDirectory -SourceRoot $InstallRoot
$null=Assert-PrivateResearchDirectory -Directory $OutputDirectory -SourceRoot $InstallRoot
$files=@('OpenKh.Kh2/Imgd.cs','OpenKh.Kh2/Ps2.Utility.cs','OpenKh.Kh2/Ps2.Repl.cs')
if((git -C $OpenKhRoot rev-parse HEAD) -ne '7a3b945c538d32c6a285128c98aefba093f52ceb' -or
    (git -C $OpenKhRoot status --porcelain -- $files)){throw 'Unaudited IMGD palette sources'}
$imgdText=Get-Content -LiteralPath (Join-Path $OpenKhRoot $files[0]) -Raw
$method=[regex]::Match($imgdText,'(?s)private byte\[\] GetClut8\(\).*?(?=\r?\n\s*private byte\[\] GetData32bpp)')
if(!$method.Success){throw 'Pinned palette method not found'}
$reference='using System;'+((Get-Content -LiteralPath (Join-Path $OpenKhRoot $files[1]) -Raw) -replace '(?m)^using [^;]+;\r?\n','')+
    (Get-Content -LiteralPath (Join-Path $OpenKhRoot $files[2]) -Raw)+
    'namespace OpenKh.Kh2 { public class ImgdPaletteObservation { public byte[] Clut {get;set;} public byte[] Palette()=>GetClut8();'+$method.Value+'}}'
Add-Type -TypeDefinition $reference
# Independently verify source/profile/header linkage before pixel comparison.
. (Join-Path $PSScriptRoot 'Verify-ImageStructure.ps1') -InstallRoot $InstallRoot -StructurePath $StructurePath -PreviousProbePath $PreviousProbePath -PixelDirectory $PixelDirectory -OpenKhRoot $OpenKhRoot -OutputDirectory $OutputDirectory
$pixelResults=@();$ordinal=0
foreach($internal in $structure.internal_images) {
    $decoded=$internal.decoded_image
    if(!$decoded -or $internal.pixel_decode -ne 'CPU_PROFILE_ONLY' -or
        $decoded.pixel_decode_profile -ne 'imgd_indexed8_linear_ps2_palette_v1' -or
        $internal.header.version -ne 256 -or $internal.header.gs_psm -ne 19 -or
        ($internal.header.flags -band -4) -ne 0 -or $internal.header.clut_length -ne 1024 -or
        $internal.header.bitmap_length -ne [long]$internal.header.width*$internal.header.height){throw 'Unqualified indexed8 pixel request'}
    $subsetPath=Join-Path $OutputDirectory ('internal-{0:D2}-storage.json' -f $ordinal)
    $sourceOriginal = & {
        . (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $subsetPath -OpenKhRoot $OpenKhRoot | Out-Null
        # Isolate the storage script's working variables from pixel metadata.
        ,$data
    }
    # Source bytes come from independent HED/PKG reconstruction, not Rust export.
    $offset=[int]$internal.bar_entry.offset
    $indices=[byte[]]::new($internal.header.bitmap_length)
    $storedClut=[byte[]]::new(1024)
    [Array]::Copy($sourceOriginal,$offset+$internal.header.bitmap_offset,$indices,0,$indices.Length)
    [Array]::Copy($sourceOriginal,$offset+$internal.header.clut_offset,$storedClut,0,1024)
    $paletteReader=[OpenKh.Kh2.ImgdPaletteObservation]::new();$paletteReader.Clut=$storedClut
    $palette=$paletteReader.Palette()
    $rgba=[byte[]]::new($indices.Length*4)
    for($pixel=0;$pixel -lt $indices.Length;$pixel++){[Buffer]::BlockCopy($palette,[int]$indices[$pixel]*4,$rgba,$pixel*4,4)}
    $stem='internal-{0:D3}' -f $ordinal
    $rgbaFile=Join-Path $OutputDirectory ($stem+'.rgba')
    $encoded=[byte[]]::new(12+$rgba.Length)
    [Text.Encoding]::ASCII.GetBytes('RGBA').CopyTo($encoded,0)
    [BitConverter]::GetBytes([uint32]$internal.header.width).CopyTo($encoded,4)
    [BitConverter]::GetBytes([uint32]$internal.header.height).CopyTo($encoded,8)
    $rgba.CopyTo($encoded,12)
    Write-NewResearchFile -Path $rgbaFile -Bytes $encoded
    Write-NewResearchFile -Path (Join-Path $OutputDirectory ($stem+'.indices')) -Bytes $indices
    Write-NewResearchFile -Path (Join-Path $OutputDirectory ($stem+'.clut')) -Bytes $palette
    foreach($part in @(@{field='indices';bytes=$indices},@{field='clut';bytes=$palette},@{field='rgba';bytes=$rgba})) {
        $name=$decoded.($part.field+'_file')
        if($name -cne ($stem+'.'+$part.field)){throw 'Unsafe/mismatched IMGD pixel filename'}
        $path=Join-Path $ImgdPixelDirectory $name
        if((Get-Item -LiteralPath $path).Length -gt 67108876){throw 'IMGD comparison input too large'}
        $actual=[IO.File]::ReadAllBytes($path)
        if($part.field -eq 'rgba') {
            if($actual.Length -ne $encoded.Length -or [Text.Encoding]::ASCII.GetString($actual,0,4) -ne 'RGBA' -or
                [BitConverter]::ToUInt32($actual,4) -ne $internal.header.width -or
                [BitConverter]::ToUInt32($actual,8) -ne $internal.header.height){throw 'IMGD pixel dimensions/header differs'}
            $actual=[byte[]]$actual[12..($actual.Length-1)]
        }
        $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($part.bytes)).ToLowerInvariant()
        $field=switch($part.field){'indices'{'indices_sha256'}'clut'{'clut_rgba_sha256'}'rgba'{'pixels_rgba_sha256'}}
        if($hash -ne $decoded.$field -or ![Linq.Enumerable]::SequenceEqual([byte[]]$part.bytes,$actual)) {throw "Independent IMGD bytes differ: $($part.field)"}
    }
    $channels=[ImageChannelOracle]::Observe($encoded)|ConvertTo-Json -Depth 6|ConvertFrom-Json
    foreach($field in @('pixels','histograms_rgba','bit_one_counts_rgba','equal_rgb_pairs','zero_alpha_pixels','zero_alpha_nonzero_rgb_pixels')) {
        if(($channels.$field|ConvertTo-Json -Depth 6 -Compress) -cne ($decoded.channels.$field|ConvertTo-Json -Depth 6 -Compress)){throw 'IMGD channel statistics differ'}
    }
    $pixelResults+=@{ordinal=$ordinal;width=$internal.header.width;height=$internal.header.height;pixels=$indices.Length;
        palette_colors=256;indices_palette_rgba_exact=$true;channels_exact=$true;installed_semantics='NOT RUN'}
    $ordinal++
}
if(!$ordinal){throw 'No admitted internal IMGD pixels'}
Write-NewResearchFile -Path (Join-Path $OutputDirectory 'independent-imgd-pixels.json') -Bytes ([Text.Encoding]::UTF8.GetBytes(($pixelResults|ConvertTo-Json -Depth 6)))
$pixelResults|ConvertTo-Json -Depth 6
