# Independent scalar/layout reads plus unchanged pinned public register/CLUT methods.
# Neither the full ModelTexture reader nor pixel swizzle/target code executes.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$textureDestination=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$textureProbePath=Get-ResearchCanonicalPath $ProbePath
$null=Assert-PrivateResearchDirectory (Split-Path -Parent $textureProbePath) $InstallRoot
if((Get-Item -LiteralPath $textureProbePath).Length -gt 33554432){throw 'Texture report exceeds 32 MiB'}
$textureReport=Get-Content -LiteralPath $textureProbePath -Raw|ConvertFrom-Json
if(!$textureReport.model_texture_inspection_requested -or $textureReport.model_texture_profile -ne 'openkh_model_texture_layout_clut_v1'){throw 'Explicit texture inspection required'}
$skeletonDirectory=Join-Path $textureDestination 'skeleton'
$null=New-Item -ItemType Directory $skeletonDirectory
. (Join-Path $PSScriptRoot 'Verify-ModelSkeleton.ps1') -InstallRoot $InstallRoot -ProbePath $textureProbePath -OpenKhRoot $OpenKhRoot -OutputDirectory $skeletonDirectory | Out-Null
$OutputDirectory=$textureDestination;$ProbePath=$textureProbePath
$pin='7a3b945c538d32c6a285128c98aefba093f52ceb'
$files=@('OpenKh.Kh2/ModelTexture.cs','OpenKh.Imaging/Tm2.cs','OpenKh.Kh2/Ps2.Utility.cs','OpenKh.Kh2/TextureFooter/TextureFooterData.cs','OpenKh.Kh2/TextureFooter/UvScroll.cs','OpenKh.Game/Entities/ObjectEntity.cs')
if((git -C $OpenKhRoot rev-parse HEAD) -ne $pin -or (git -C $OpenKhRoot status --porcelain -- $files)){throw 'Texture references differ from pin'}
function Get-TextureBracedSource([string]$Source,[string]$Declaration) {
    $start=$Source.IndexOf($Declaration,[StringComparison]::Ordinal)
    if($start -lt 0){throw "Pinned declaration absent: $Declaration"}
    $brace=$Source.IndexOf('{',$start);$depth=0
    for($cursor=$brace;$cursor -lt $Source.Length;$cursor++){
        if($Source[$cursor] -eq '{'){$depth++}
        if($Source[$cursor] -eq '}'){$depth--;if($depth -eq 0){return $Source.Substring($start,$cursor-$start+1)}}
    }
    throw 'Unclosed public declaration'
}
$modelTextureSource=Get-Content (Join-Path $OpenKhRoot $files[0]) -Raw
$tm2Source=Get-Content (Join-Path $OpenKhRoot $files[1]) -Raw
$textureExtracts=@('public class BITBLTBUF','public class CLAMP','private static byte[] GetClut4','private static byte[] GetClut8','public static int GetClutPointer','private static int GetBits','private static long SetBits')|ForEach-Object{Get-TextureBracedSource $modelTextureSource $_}
$tm2Extracts=@('public enum GsPSM','public enum GsCPSM','public class GsTex','private static int GetBits(','private static long SetBits(')|ForEach-Object{Get-TextureBracedSource $tm2Source $_}
$tm2Expressions=@('private static int GetSizeRegister(','private static bool GetBit(','private static long SetBit(')|ForEach-Object{
    $start=$tm2Source.IndexOf($_,[StringComparison]::Ordinal);if($start -lt 0){throw 'Missing public expression'}
    $end=$tm2Source.IndexOf(';',$start);$tm2Source.Substring($start,$end-$start+1)
}
$compile='using System;using System.IO;using System.Collections.Generic;using System.Security.Cryptography;using Xe.BinaryMapper;using OpenKh.Kh2;'+
    'namespace Xe.BinaryMapper {[AttributeUsage(AttributeTargets.Property)]public class DataAttribute:Attribute{}}'+
    ((Get-Content (Join-Path $OpenKhRoot $files[2]) -Raw)-replace '(?m)^using [^;]+;\r?\n','')+
    'namespace OpenKh.Imaging {public class Tm2 {'+($tm2Extracts -join "`n")+($tm2Expressions -join "`n")+'}}'+
    'public class TexturePublicMethods {'+($textureExtracts -join "`n")+@'
    public static object Fields(ulong tex0,ulong clamp,ulong bitblt) {
        var t=new OpenKh.Imaging.Tm2.GsTex {Data=unchecked((long)tex0)};
        var c=new CLAMP(unchecked((long)clamp));var b=new BITBLTBUF(unchecked((long)bitblt));
        return new {tex0=new[]{t.TBP0,t.TBW,(int)t.PSM,t.TW,t.TH,t.TCC?1:0,t.TFX,t.CBP,(int)t.CPSM,t.CSM?1:0,t.CSA,t.CLD},
            address=new[]{c.WMS,c.WMT,c.MINU,c.MAXU,c.MINV,c.MAXV},destination_base=b.DBP,upload_psm=b.DPSM};
    }
    public static int[] Tex0(ulong data) {
        var t=new OpenKh.Imaging.Tm2.GsTex{Data=unchecked((long)data)};
        return new[]{t.TBP0,t.TBW,(int)t.PSM,t.TW,t.TH,t.TCC?1:0,t.TFX,t.CBP,(int)t.CPSM,t.CSM?1:0,t.CSA,t.CLD};
    }
    public static int[] Address(ulong data) {var c=new CLAMP(unchecked((long)data));return new[]{c.WMS,c.WMT,c.MINU,c.MAXU,c.MINV,c.MAXV};}
    public static int Destination(ulong data)=>new BITBLTBUF(unchecked((long)data)).DBP;
    public static int Upload(ulong data)=>new BITBLTBUF(unchecked((long)data)).DPSM;
    public static byte[] Palette(byte[] input,int cbp,int csa,int depth)=>depth==4?GetClut4(input,cbp,csa):GetClut8(input,cbp,csa);
}
public static class TextureLayoutOracle {
    static byte[] Exact(BinaryReader r,int count){var b=r.ReadBytes(count);if(b.Length!=count)throw new InvalidDataException("truncated texture bytes");return b;}
    static string Hash(byte[] b)=>Convert.ToHexString(SHA256.HashData(b)).ToLowerInvariant();
    static object Span(byte[] payload,int offset,int length){
        if(offset<0 || length<0 || (long)offset+length>payload.Length)throw new InvalidDataException("texture range");
        byte[] b=new byte[length];Array.Copy(payload,offset,b,0,length);return b;
    }
    public static byte[] Payload(byte[] original,int ordinal) {
        using var m=new MemoryStream(original,false);using var r=new BinaryReader(m);
        if(r.ReadUInt32()!=0x01524142)throw new InvalidDataException("BAR magic");int count=r.ReadInt32();
        if(ordinal<0 || ordinal>=count)throw new InvalidDataException("BAR ordinal");m.Position=16+16*ordinal;
        if(r.ReadUInt16()!=7)throw new InvalidDataException("BAR texture type");m.Position+=6;
        int offset=r.ReadInt32(),size=r.ReadInt32();if(size>33554432)throw new InvalidDataException("texture byte bound");
        return (byte[])Span(original,offset,size);
    }
    public static object Read(byte[] payload) {
        using var m=new MemoryStream(payload,false);using var r=new BinaryReader(m);
        int[] h=new int[9];for(int i=0;i<9;i++)h[i]=r.ReadInt32();
        var transfers=new List<object>();var environments=new List<object>();var images=new List<object>();var footer=new List<object>();
        if(h[0]==-1)return new{profile="openkh_model_texture_layout_clut_v1",header_words=h,disabled=true,
            offset_data=Array.Empty<byte>(),transfers,environments,picture_sha256=(string)null,palette_sha256=(string)null,images,footer,pixel_swizzle_decoded=false,runtime_verified=false};
        if(h[0]!=0 || h[1]<0 || h[1]>1048576 || h[2]<=0 || h[2]>4096 || h[3]<=0 || h[3]>4096)throw new InvalidDataException("texture header");
        byte[] mapping=(byte[])Span(payload,h[4],h[3]);byte[] picture=(byte[])Span(payload,h[7],h[8]-h[7]);byte[] palette=(byte[])Span(payload,h[8],checked(h[1]*4));
        var transferWords=new uint[h[2]+1][];var transferBitblt=new ulong[h[2]+1];
        for(int i=0;i<=h[2];i++) {
            m.Position=h[5]+i*144;uint[] w=new uint[36];for(int j=0;j<36;j++)w[j]=r.ReadUInt32();transferWords[i]=w;
            m.Position=h[5]+i*144+32;ulong bitblt=r.ReadUInt64();transferBitblt[i]=bitblt;
            transfers.Add(new{words=w,destination_base=TexturePublicMethods.Destination(bitblt),upload_psm=TexturePublicMethods.Upload(bitblt),data_offset=unchecked((int)w[29]),quad_word_count=w[24]&32767});
        }
        long imageReadBytes=0;
        for(int i=0;i<h[3];i++) {
            m.Position=h[6]+i*160;ulong[] w=new ulong[20];for(int j=0;j<20;j++)w[j]=r.ReadUInt64();
            var t=TexturePublicMethods.Tex0(w[14]);environments.Add(new{words=w,address=TexturePublicMethods.Address(w[16]),tex0=t});
            int transfer=mapping[i];if(transfer>=h[2])throw new InvalidDataException("texture map");
            int width=1<<t[3],height=1<<t[4],depth=t[2]==19?8:t[2]==20?4:throw new InvalidDataException("texture PSM");
            int length=checked(width*height)/(depth==4?2:1),offset=unchecked((int)transferWords[transfer+1][29]);
            imageReadBytes+=length;if(imageReadBytes>134217728)throw new InvalidDataException("cumulative image reads");
            var data=(byte[])Span(payload,offset,length);int cbp=t[7]-TexturePublicMethods.Destination(transferBitblt[0]);
            if(cbp<0)throw new InvalidDataException("negative CLUT relative base");
            var clut=TexturePublicMethods.Palette(palette,cbp,t[10],depth);int colors=depth==4?16:256;
            var rgba=new byte[colors][];int[] indices=new int[colors];
            for(int j=0;j<colors;j++){rgba[j]=new byte[4];Array.Copy(clut,j*4,rgba[j],0,4);indices[j]=TexturePublicMethods.GetClutPointer(j,cbp,t[10]);}
            images.Add(new{environment_ordinal=i,transfer_ordinal=transfer,width,height,indexed_bits_per_pixel=depth,
                upload_psm=TexturePublicMethods.Upload(transferBitblt[transfer+1]),data_offset=offset,data_length=length,
                stored_data_sha256=Hash(data),relative_clut_base=cbp,clut_entry_offset=t[10],clut_source_indices=indices,clut_rgba=rgba});
        }
        m.Position=(long)h[8]+palette.Length;
        while(m.Position<m.Length) {
            if(footer.Count>=4096)throw new InvalidDataException("footer count");long offset=m.Position;var tag=Exact(r,4);
            bool final=tag[0]==95 && tag[1]==75 && tag[2]==78 && tag[3]==53;
            int length=final?checked((int)(m.Length-m.Position)):r.ReadInt32();byte[] data=Exact(r,length);uint[] uv=null;
            if(tag[0]==85 && tag[1]==86 && tag[2]==83 && tag[3]==67){using var u=new BinaryReader(new MemoryStream(data,false));uv=new[]{u.ReadUInt32(),u.ReadUInt32(),u.ReadUInt32()};}
            footer.Add(new{offset,tag,length,payload_sha256=Hash(data),uv_scroll_words=uv});
        }
        return new{profile="openkh_model_texture_layout_clut_v1",header_words=h,disabled=false,offset_data=mapping,
            transfers,environments,picture_sha256=Hash(picture),palette_sha256=Hash(palette),images,footer,pixel_swizzle_decoded=false,runtime_verified=false};
    }
}
'@
Write-NewResearchFile (Join-Path $OutputDirectory 'public-method-adapter.cs') ([Text.Encoding]::UTF8.GetBytes($compile))
$hashes=@(foreach($relative in $files){@{path=$relative;sha256=(Get-FileHash (Join-Path $OpenKhRoot $relative)).Hash.ToLowerInvariant()}})
$extractHashes=@(@($textureExtracts)+@($tm2Extracts)+@($tm2Expressions)|ForEach-Object{@{length=$_.Length;sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($_))).ToLowerInvariant()}})
Write-NewResearchFile (Join-Path $OutputDirectory 'reference-hashes.json') ([Text.Encoding]::UTF8.GetBytes((@{pin=$pin;files=$hashes;unchanged_extracts=$extractHashes}|ConvertTo-Json -Depth 8)))
Add-Type -TypeDefinition $compile
$textureResults=@();$textureSampleNumber=0
foreach($sample in $textureReport.samples){
    $storagePath=Join-Path $skeletonDirectory ('storage-{0:d2}.json' -f $textureSampleNumber)
    $independentOriginal=&{. (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $storagePath -OpenKhRoot $OpenKhRoot|Out-Null;,$data}
    foreach($texture in $sample.model_texture_layouts){
        $payload=[TextureLayoutOracle]::Payload($independentOriginal,[int]$texture.bar_ordinal)
        $independent=[TextureLayoutOracle]::Read($payload)|ConvertTo-Json -Depth 24|ConvertFrom-Json
        Assert-ModelValue $texture.layout $independent "sample/$textureSampleNumber/texture/$($texture.bar_ordinal)"
        $textureResults+=@{sample=$textureSampleNumber;bar_ordinal=$texture.bar_ordinal;layout=$independent}
    }
    $textureSampleNumber++
}
if(!$textureResults.Count){throw 'No texture observations'}
Write-NewResearchFile (Join-Path $OutputDirectory 'independent-model-textures.json') ([Text.Encoding]::UTF8.GetBytes((@{verdict='PASS';pin=$pin;samples=$textureResults;scalar_reader='owned BinaryReader';public_register_clut_methods_executed_unchanged=$true;full_model_texture_reader_executed=$false;pixel_swizzle_decoded=$false;runtime_verified=$false}|ConvertTo-Json -Depth 32)))
"PASS: $($textureResults.Count) model texture layouts, raw registers, image ranges, CLUT addresses/colors and footer spans match independently."
