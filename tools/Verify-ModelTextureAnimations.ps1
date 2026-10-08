# Independent current storage/layout and unchanged public TEXA constructor/getters.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$AuthoredDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$destination=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$probe=Get-ResearchCanonicalPath $ProbePath
$null=Assert-PrivateResearchDirectory (Split-Path -Parent $probe) $InstallRoot
if((Get-Item $probe).Length -gt 33554432){throw 'TEXA probe exceeds bound'}
$report=Get-Content $probe -Raw|ConvertFrom-Json
if(!$report.model_texture_animation_records_requested -or $report.model_texture_animation_profile -ne 'openkh_texa_bounded_frame_graph_v1'){
    throw 'Explicit TEXA record inspection required'
}
if($AuthoredDirectory){$AuthoredDirectory=Assert-PrivateResearchDirectory $AuthoredDirectory $InstallRoot}
$layoutDirectory=Join-Path $destination 'layout'
New-Item -ItemType Directory $layoutDirectory -ErrorAction Stop|Out-Null
& pwsh -NoProfile -File (Join-Path $PSScriptRoot 'Verify-ModelTextures.ps1') -InstallRoot $InstallRoot -ProbePath $probe -OpenKhRoot $OpenKhRoot -OutputDirectory $layoutDirectory *> (Join-Path $destination 'layout.log')
if($LASTEXITCODE){throw 'Independent texture/storage preflight failed'}
$errors=$null;$tokens=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'Verify-ModelSkeleton.ps1'),[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Comparison helper syntax differs'}
$helper=$ast.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Assert-ModelValue'},$true)
if(!$helper){throw 'Comparison helper absent'}
. ([scriptblock]::Create($helper.Extent.Text))
$pin='7a3b945c538d32c6a285128c98aefba093f52ceb'
$files=@('OpenKh.Kh2/TextureFooter/TextureFooterData.cs','OpenKh.Kh2/TextureFooter/TextureAnimation.cs','OpenKh.Kh2/TextureFooter/TextureFrame.cs','OpenKh.Kh2/TextureFooter/TextureFrameGroup.cs','OpenKh.Kh2/TextureFooter/TextureFrameControl.cs','OpenKh.Kh2/TextureFooter/UvScroll.cs','OpenKh.Common/Utils/BitsUtil.cs')
if((git -C $OpenKhRoot rev-parse HEAD) -ne $pin -or (git -C $OpenKhRoot status --porcelain -- $files)){throw 'TEXA primary sources differ from pin'}
function Get-TexaConstructor([string]$Source){
    $start=$Source.IndexOf('public TextureFooterData(Stream stream)',[StringComparison]::Ordinal)
    if($start -lt 0){throw 'Public TEXA constructor absent'}
    $brace=$Source.IndexOf('{',$start);$depth=0
    for($i=$brace;$i -lt $Source.Length;$i++){
        if($Source[$i] -eq '{'){$depth++}
        if($Source[$i] -eq '}'){$depth--;if(!$depth){return $Source.Substring($start,$i-$start+1)}}
    }
    throw 'Public TEXA constructor incomplete'
}
$constructor=Get-TexaConstructor (Get-Content (Join-Path $OpenKhRoot $files[0]) -Raw)
$classes=@($files[1..6]|ForEach-Object{(Get-Content (Join-Path $OpenKhRoot $_) -Raw)-replace '(?m)^using [^;]+;\r?\n',''})
$adapter='using System.Linq;using System.Text;using System.Text.Json;using OpenKh.Common;using OpenKh.Common.Utils;'+
    (Get-Content (Join-Path $layoutDirectory 'public-method-adapter.cs') -Raw)+($classes -join "`n")+
    'namespace OpenKh.Kh2.TextureFooter {public class TextureFooterData {public List<UvScroll> UvscList{get;}=new();public List<TextureAnimation> TextureAnimationList{get;}=new();public byte[] UnkFooter{get;set;}public bool ShouldEmitDMYAtFirst{get;set;}public bool ShouldEmitKN5{get;set;}'+$constructor+'}}'+@'
namespace OpenKh.Common {
 public static class TexaScalarExtensions {
  public static byte[] ReadBytes(this Stream s,int count) {if(count<0||count>33554432)throw new InvalidDataException("byte bound");var b=new byte[count];s.ReadExactly(b);return b;}
  public static byte[] ReadBytes(this Stream s)=>s.ReadBytes(checked((int)(s.Length-s.Position)));
  public static int ReadInt32(this Stream s)=>new BinaryReader(s,Encoding.UTF8,true).ReadInt32();
  public static short ReadInt16(this Stream s)=>new BinaryReader(s,Encoding.UTF8,true).ReadInt16();
  public static uint ReadUInt32(this Stream s)=>new BinaryReader(s,Encoding.UTF8,true).ReadUInt32();
  public static T SetPosition<T>(this T s,long p)where T:Stream{s.Seek(p,SeekOrigin.Begin);return s;}
 }
}
namespace Xe.BinaryMapper {
 public static class BinaryMapping {
  public static int FrameReads;
  public static T ReadObject<T>(Stream s) {
   var r=new BinaryReader(s,Encoding.UTF8,true);object value;
   if(typeof(T)==typeof(OpenKh.Kh2.TextureFooter.TextureAnimation)){
    value=new OpenKh.Kh2.TextureFooter.TextureAnimation{Unk1=r.ReadUInt16(),TextureIndex=r.ReadUInt16(),FrameStride=r.ReadUInt16(),BitsPerPixel=r.ReadUInt16(),BaseSlotIndex=r.ReadUInt16(),MaximumSlotIndex=r.ReadUInt16(),NumAnimations=r.ReadUInt16(),NumSpritesInImageData=r.ReadUInt16(),UOffsetInBaseImage=r.ReadUInt16(),VOffsetInBaseImage=r.ReadUInt16(),SpriteWidth=r.ReadUInt16(),SpriteHeight=r.ReadUInt16(),OffsetSlotTable=r.ReadInt32(),OffsetAnimationTable=r.ReadInt32(),OffsetSpriteImage=r.ReadInt32(),DefaultAnimationIndex=r.ReadInt32()};
   }else if(typeof(T)==typeof(OpenKh.Kh2.TextureFooter.TextureFrame)){
    if(++FrameReads>65536)throw new InvalidDataException("owned public execution frame budget");
    value=new OpenKh.Kh2.TextureFooter.TextureFrame{Data=r.ReadUInt16(),MinimumLength=r.ReadUInt16(),MaximumLength=r.ReadUInt16(),SpriteImageIndex=r.ReadUInt16()};
   }else if(typeof(T)==typeof(OpenKh.Kh2.TextureFooter.UvScroll)){
    value=new OpenKh.Kh2.TextureFooter.UvScroll{TextureIndex=r.ReadInt32(),UScrollSpeed=r.ReadSingle(),VScrollSpeed=r.ReadSingle()};
   }else throw new InvalidDataException("owned scalar mapping type");
   return (T)value;
  }
 }
}
public static class TexaRecordOracle {
 static string Hash(byte[] b)=>Convert.ToHexString(SHA256.HashData(b)).ToLowerInvariant();
 public static object Read(byte[] payload){
  if(payload.Length>33554432)throw new InvalidDataException("payload bound");
  using var doc=JsonDocument.Parse(JsonSerializer.Serialize(TextureLayoutOracle.Read(payload)));
  var chunks=doc.RootElement.GetProperty("footer").EnumerateArray().ToArray();
  int footerStart=chunks.Length==0?payload.Length:chunks[0].GetProperty("offset").GetInt32();
  BinaryMapping.FrameReads=0;
  using var stream=new MemoryStream(payload,footerStart,payload.Length-footerStart,false);
  var parsed=new OpenKh.Kh2.TextureFooter.TextureFooterData(stream);
  var result=new List<object>();int animationOrdinal=0,footerOrdinal=0;
  foreach(var chunk in chunks){
   if(chunk.GetProperty("tag").GetBytesFromBase64().SequenceEqual(new byte[]{84,69,88,65})){
    var a=parsed.TextureAnimationList[animationOrdinal++];
    int start=chunk.GetProperty("offset").GetInt32()+8,length=chunk.GetProperty("length").GetInt32();
    ushort[] u={a.Unk1,a.TextureIndex,a.FrameStride,a.BitsPerPixel,a.BaseSlotIndex,a.MaximumSlotIndex,a.NumAnimations,a.NumSpritesInImageData,a.UOffsetInBaseImage,a.VOffsetInBaseImage,a.SpriteWidth,a.SpriteHeight};
    int[] h={a.OffsetSlotTable,a.OffsetAnimationTable,a.OffsetSpriteImage,a.DefaultAnimationIndex};
    using var r=new BinaryReader(new MemoryStream(payload,start,length,false));r.BaseStream.Position=a.OffsetAnimationTable;
    uint[] offsets=Enumerable.Range(0,a.NumAnimations).Select(_=>r.ReadUInt32()).ToArray();
    var groups=new List<object>();
    for(int g=0;g<a.FrameGroupList.Length;g++){
     var records=new List<object>();int repeated=0;
     foreach(var pair in a.FrameGroupList[g].IndexedFrameList){
      var f=pair.Value;int control=(int)f.FrameControl,delta=f.FrameIndexDelta;
      int next=checked(pair.Key+((control==2||control==3)?delta:1));
      long offset=(long)offsets[g]+pair.Key*8L;
      if(offset<0||offset+8>length)throw new InvalidDataException("owned frame body bound");
      records.Add(new{index=pair.Key,body_offset=offset,words=new ushort[]{f.Data,f.MinimumLength,f.MaximumLength,f.SpriteImageIndex},control,frame_index_delta=delta,next_index=next});repeated=next;
     }
     groups.Add(new{first_position=offsets[g],records,repeated_index=repeated});
    }
    result.Add(new{footer_ordinal=footerOrdinal,header_u16=u,header_i32=h,sprite_data_offset=start+a.OffsetSpriteImage,sprite_data_length=a.SpriteImage.Length,sprite_data_sha256=Hash(a.SpriteImage),slot_table=a.SlotTable,frame_group_offsets=offsets,frame_groups=groups});
   }
   footerOrdinal++;
  }
  if(animationOrdinal!=parsed.TextureAnimationList.Count)throw new InvalidDataException("constructor TEXA denominator");
  return new{profile="openkh_texa_bounded_frame_graph_v1",fixed_frame_size=8,animations=result,runtime_verified=false};
 }
}
'@
Write-NewResearchFile (Join-Path $destination 'public-constructor-adapter.cs') ([Text.Encoding]::UTF8.GetBytes($adapter))
Write-NewResearchFile (Join-Path $destination 'reference-hashes.json') ([Text.Encoding]::UTF8.GetBytes((@{
    pin=$pin;files=@($files|ForEach-Object{@{path=$_;sha256=(Get-FileHash (Join-Path $OpenKhRoot $_)).Hash.ToLowerInvariant()}});
    unchanged_constructor_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($constructor))).ToLowerInvariant();
    owned_adapter_sha256=(Get-FileHash (Join-Path $destination 'public-constructor-adapter.cs')).Hash.ToLowerInvariant()
}|ConvertTo-Json -Depth 6)))
Add-Type -TypeDefinition $adapter
$results=@();$sampleNumber=0;$animations=0;$slots=0;$groups=0;$frames=0
foreach($sample in $report.samples){
    $storage=Join-Path $layoutDirectory ('skeleton/storage-{0:d2}.json' -f $sampleNumber)
    $original=&{. (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $storage -OpenKhRoot $OpenKhRoot|Out-Null;,$data}
    # The preflight independently validates the complete BAR table. Keep every
    # type-7 entry in the denominator, including payloads without any TEXA.
    $expectedOrdinals=@(for($i=0;$i -lt $sample.bar.entries.Count;$i++){
        if($sample.bar.entries[$i].entry_type -eq 7){$i}
    })
    $observedOrdinals=@($sample.model_texture_animation_records|ForEach-Object{$_.bar_ordinal})
    if(!$expectedOrdinals.Count -or ($expectedOrdinals -join ',') -cne ($observedOrdinals -join ',')){
        throw 'TEXA type-7 denominator differs'
    }
    foreach($entry in $sample.model_texture_animation_records){
        $payload=[TextureLayoutOracle]::Payload($original,[int]$entry.bar_ordinal)
        $reference=[TexaRecordOracle]::Read($payload)|ConvertTo-Json -Depth 32|ConvertFrom-Json
        Assert-ModelValue $entry.records $reference "sample/$sampleNumber/TEXA/$($entry.bar_ordinal)"
        $animations+=$reference.animations.Count
        foreach($a in $reference.animations){$slots+=$a.slot_table.Count;$groups+=$a.frame_groups.Count;foreach($g in $a.frame_groups){$frames+=$g.records.Count}}
        $results+=@{sample=$sampleNumber;bar_ordinal=$entry.bar_ordinal;records=$reference}
    }
    $sampleNumber++
}
if(!$results.Count -or !$animations){throw 'No finite source TEXA observations'}
$authored=@()
if($AuthoredDirectory){
    $manifestPath=Join-Path $AuthoredDirectory 'cases.json'
    if((Get-Item $manifestPath).Length -gt 33554432){throw 'Authored manifest exceeds bound'}
    $manifest=Get-Content $manifestPath -Raw|ConvertFrom-Json
    if($manifest.provenance -ne 'generated_synthetic_bytes' -or $manifest.cases.Count -lt 1 -or $manifest.cases.Count -gt 32){throw 'Authored TEXA provenance/count differs'}
    foreach($case in $manifest.cases){
        if($case.file -notmatch '^case-[0-9]{2}\.texture$'){throw 'Authored filename differs'}
        $path=Join-Path $AuthoredDirectory $case.file
        if((Get-Item $path).Length -gt 33554432){throw 'Authored payload exceeds bound'}
        $reference=[TexaRecordOracle]::Read([IO.File]::ReadAllBytes($path))|ConvertTo-Json -Depth 32|ConvertFrom-Json
        Assert-ModelValue $case.records $reference "authored/$($case.file)"
        $authored+=@{file=$case.file;records=$reference}
    }
}
Write-NewResearchFile (Join-Path $destination 'independent-model-texture-animations.json') ([Text.Encoding]::UTF8.GetBytes((@{
    verdict='PASS';pin=$pin;samples=$results;source_animations=$animations;source_slots=$slots;source_groups=$groups;source_frame_visits=$frames;
    authored=$authored;unchanged_footer_constructor_and_frame_getters_executed=$true;full_binary_mapper_executed=$false;full_model_reader_executed=$false;runtime_verified=$false
}|ConvertTo-Json -Depth 40)))
"PASS TEXA records: $animations source headers, $slots slots, $groups groups, $frames frame visits; runtime unqualified."
