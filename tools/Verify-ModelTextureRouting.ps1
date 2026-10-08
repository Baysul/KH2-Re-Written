# Independent storage/layout/topology, unchanged public RAW scanner and TEXA
# stride property. This qualifies finite inspection candidates, not runtime.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$routingDestination=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$routingProbePath=Get-ResearchCanonicalPath $ProbePath
$null=Assert-PrivateResearchDirectory (Split-Path -Parent $routingProbePath) $InstallRoot
if((Get-Item -LiteralPath $routingProbePath).Length -gt 33554432){throw 'Routing report exceeds 32 MiB'}
$routingReport=Get-Content $routingProbePath -Raw|ConvertFrom-Json
if(!$routingReport.model_texture_routing_requested -or $routingReport.model_texture_routing_profile -ne 'openkh_egs_raw_bounded_footer_links_v1'){
    throw 'Explicit bounded texture routing required'
}
foreach($stage in @(@{name='geometry';script='Verify-ModelGeometry.ps1'},@{name='textures';script='Verify-ModelTextures.ps1'})){
    $stageDirectory=Join-Path $routingDestination $stage.name
    $null=New-Item -ItemType Directory $stageDirectory
    & pwsh -NoProfile -File (Join-Path $PSScriptRoot $stage.script) -InstallRoot $InstallRoot -ProbePath $routingProbePath -OpenKhRoot $OpenKhRoot -OutputDirectory $stageDirectory *> (Join-Path $routingDestination ($stage.name+'.log'))
    if($LASTEXITCODE -ne 0){throw "Independent $($stage.name) preflight failed"}
}
# Load only the existing recursive comparison helper, without executing its script.
$parseErrors=$null;$parseTokens=$null
$helperAst=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'Verify-ModelSkeleton.ps1'),[ref]$parseTokens,[ref]$parseErrors)
if($parseErrors.Count){throw 'Comparison helper syntax differs'}
$helper=$helperAst.Find({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Assert-ModelValue'},$true)
if(!$helper){throw 'Comparison helper missing'}
. ([scriptblock]::Create($helper.Extent.Text))
$pin='7a3b945c538d32c6a285128c98aefba093f52ceb'
$files=@('OpenKh.Egs/EgsTools.cs','OpenKh.Egs/Helpers.cs','OpenKh.Kh2/TextureFooter/TextureAnimation.cs','OpenKh.Kh2/TextureFooter/TextureFooterData.cs','OpenKh.Game/Entities/ObjectEntity.cs','OpenKh.Engine.MonoGame/MeshLoader.cs','OpenKh.Engine.MonoGame/GraphicsDeviceExtensions.cs','OpenKh.Engine/Parsers/Kkdf2MdlxParser.cs','OpenKh.Engine/Parsers/MdlxParser.cs')
if((git -C $OpenKhRoot rev-parse HEAD) -ne $pin -or (git -C $OpenKhRoot status --porcelain -- $files)){throw 'Routing references differ from pin'}
function Get-RoutingBracedSource([string]$Source,[string]$Declaration){
    $start=$Source.IndexOf($Declaration,[StringComparison]::Ordinal)
    if($start -lt 0){throw "Missing routing declaration: $Declaration"}
    $brace=$Source.IndexOf('{',$start);$depth=0
    for($cursor=$brace;$cursor -lt $Source.Length;$cursor++){
        if($Source[$cursor] -eq '{'){$depth++}
        if($Source[$cursor] -eq '}'){$depth--;if(!$depth){return $Source.Substring($start,$cursor-$start+1)}}
    }
    throw 'Unclosed routing declaration'
}
$raw=Get-RoutingBracedSource (Get-Content (Join-Path $OpenKhRoot $files[0]) -Raw) 'class RAW'
$scan=Get-RoutingBracedSource (Get-Content (Join-Path $OpenKhRoot $files[1]) -Raw) 'public static int IndexOfByteArray('
$animation=Get-RoutingBracedSource (Get-Content (Join-Path $OpenKhRoot $files[2]) -Raw) 'public class TextureAnimation'
$adapter='using System.Linq;using System.Text.Json;using OpenKh.Common;'+(Get-Content (Join-Path $routingDestination 'textures/public-method-adapter.cs') -Raw)+
    'namespace OpenKh.Common { public static class OwnedScalarExtensions {public static int ReadInt32(this Stream s)=>new BinaryReader(s,System.Text.Encoding.UTF8,true).ReadInt32();public static long ReadInt64(this Stream s)=>new BinaryReader(s,System.Text.Encoding.UTF8,true).ReadInt64();public static short ReadInt16(this Stream s)=>new BinaryReader(s,System.Text.Encoding.UTF8,true).ReadInt16();}}'+
    'namespace OpenKh.Egs {public static class Helpers {'+$scan+' public static void ScanPrint(string s){} }'+$raw+' public static class RawSnapshot {public static object Read(byte[] b,int offset){var raw=new RAW(b,offset);return new{offsets=raw.Offsets,count=raw.TextureCount,invalid=raw.Invalid};}}}'+
    'namespace OpenKh.Kh2.TextureFooter {public class TextureFrameGroup {}'+$animation+'}'+@'
public static class TextureRoutingOracle {
    static string Hash(byte[] b)=>Convert.ToHexString(SHA256.HashData(b)).ToLowerInvariant();
    public static int AssetOffset(byte[] original,int ordinal) {
        using var r=new BinaryReader(new MemoryStream(original,false));
        if(r.ReadUInt32()!=0x01524142)throw new InvalidDataException("BAR magic");int count=r.ReadInt32();
        if(ordinal<0 || ordinal>=count)throw new InvalidDataException("BAR ordinal");r.BaseStream.Position=16+ordinal*16;
        if(r.ReadUInt16()!=7)throw new InvalidDataException("BAR type");r.ReadInt16();
        if(!r.ReadBytes(4).SequenceEqual(new byte[]{116,105,109,95}))throw new InvalidDataException("BAR tim_ name");
        return r.ReadInt32();
    }
    public static object Read(byte[] payload,int assetOffset,string replacementJson) {
        using var layoutDoc=JsonDocument.Parse(JsonSerializer.Serialize(TextureLayoutOracle.Read(payload)));
        using var replacementsDoc=JsonDocument.Parse(replacementJson);
        var layout=layoutDoc.RootElement;var replacements=replacementsDoc.RootElement;
        var headers=new List<object>();var animations=new List<(int environment,int footer,long offset)>();
        int footerOrdinal=0;
        foreach(var chunk in layout.GetProperty("footer").EnumerateArray()) {
            if(chunk.GetProperty("tag").GetBytesFromBase64().SequenceEqual(new byte[]{84,69,88,65})) {
                int start=chunk.GetProperty("offset").GetInt32()+8,length=chunk.GetProperty("length").GetInt32();
                if(length<40)throw new InvalidDataException("TEXA header");
                using var r=new BinaryReader(new MemoryStream(payload,start,length,false));
                ushort[] u=new ushort[12];int[] w=new int[4];for(int i=0;i<12;i++)u[i]=r.ReadUInt16();for(int i=0;i<4;i++)w[i]=r.ReadInt32();
                var a=new OpenKh.Kh2.TextureFooter.TextureAnimation{BitsPerPixel=u[3],SpriteWidth=u[10]};
                int spriteLength=checked(a.SpriteStride*u[11]*u[7]);
                if(w[2]<0 || (long)w[2]+spriteLength>length || spriteLength>33554432)throw new InvalidDataException("TEXA span");
                var sprite=new byte[spriteLength];Array.Copy(payload,start+w[2],sprite,0,spriteLength);
                headers.Add(new{footer_ordinal=footerOrdinal,header_u16=u,header_i32=w,sprite_data_offset=start+w[2],sprite_data_length=spriteLength,sprite_data_sha256=Hash(sprite)});
                animations.Add((u[1],footerOrdinal,(long)assetOffset+start+w[2]+u[1]*16+0x20000000));
            }
            footerOrdinal++;
        }
        var images=layout.GetProperty("images").EnumerateArray().ToArray();
        int modifier=images.Count(x=>x.GetProperty("indexed_bits_per_pixel").GetInt32()!=4),pxl4=0,pxl8=0;
        var targets=new List<Dictionary<string,object>>();var matched=new HashSet<int>();var offsets=new List<long>();
        void Target(string kind,int environment,long offset,int footer=-1) {
            int[] matches=replacements.EnumerateArray().Where(x=>x.GetProperty("original_asset_offset").GetInt64()==offset).Select(x=>x.GetProperty("ordinal").GetInt32()).ToArray();
            foreach(int ordinal in matches)matched.Add(ordinal);
            var target=new Dictionary<string,object>{{"kind",kind},{"environment_ordinal",environment},{"candidate_original_asset_offset",offset},{"matching_replacement_ordinals",matches}};
            if(footer>=0)target.Add("footer_ordinal",footer);targets.Add(target);offsets.Add(offset);
        }
        for(int i=0;i<images.Length;i++) {
            int adjustment=images[i].GetProperty("indexed_bits_per_pixel").GetInt32()==4?pxl4+++modifier*16:pxl8++*16;
            Target("base",i,(long)assetOffset+images[i].GetProperty("data_offset").GetInt32()+adjustment+0x20000000);
            foreach(var a in animations.Where(x=>x.environment==i))Target("texa",i,a.offset,a.footer);
        }
        if(animations.Any(x=>x.environment>=images.Length))throw new InvalidDataException("TEXA environment");
        var seen=new HashSet<long>();var duplicates=new HashSet<long>();var duplicateOffsets=new List<long>();
        foreach(long offset in offsets)if(!seen.Add(offset) && duplicates.Add(offset))duplicateOffsets.Add(offset);
        return new{profile="openkh_egs_raw_bounded_footer_links_v1",texa_headers=headers,ordered_targets=targets,
            unmatched_replacement_ordinals=replacements.EnumerateArray().Select(x=>x.GetProperty("ordinal").GetInt32()).Where(x=>!matched.Contains(x)).ToArray(),
            duplicate_candidate_offsets=duplicateOffsets,runtime_verified=false};
    }
}
'@
Write-NewResearchFile (Join-Path $routingDestination 'public-routing-adapter.cs') ([Text.Encoding]::UTF8.GetBytes($adapter))
$hashes=@(foreach($relative in $files){@{path=$relative;sha256=(Get-FileHash (Join-Path $OpenKhRoot $relative)).Hash.ToLowerInvariant()}})
$extracts=@(foreach($source in @($raw,$scan,$animation)){@{length=$source.Length;sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($source))).ToLowerInvariant()}})
Write-NewResearchFile (Join-Path $routingDestination 'routing-reference-hashes.json') ([Text.Encoding]::UTF8.GetBytes((@{pin=$pin;files=$hashes;unchanged_extracts=$extracts}|ConvertTo-Json -Depth 8)))
Add-Type -TypeDefinition $adapter
$independentGeometry=Get-Content (Join-Path $routingDestination 'geometry/independent-model-geometry.json') -Raw|ConvertFrom-Json
$results=@();$sampleNumber=0;$corners=0;$crossPart=0
foreach($sample in $routingReport.samples){
    $storagePath=Join-Path $routingDestination ('textures/skeleton/storage-{0:d2}.json' -f $sampleNumber)
    $independentOriginal=&{. (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $storagePath -OpenKhRoot $OpenKhRoot|Out-Null;,$data}
    $storage=Get-Content $storagePath -Raw|ConvertFrom-Json
    $route=$sample.model_texture_routing
    $payload=[TextureLayoutOracle]::Payload($independentOriginal,[int]$route.texture_bar_ordinal)
    $assetOffset=[TextureRoutingOracle]::AssetOffset($independentOriginal,[int]$route.texture_bar_ordinal)
    $metadataJson=ConvertTo-Json -InputObject @($storage.samples[0].replacement_samples.metadata) -Depth 8
    $links=[TextureRoutingOracle]::Read($payload,$assetOffset,$metadataJson)|ConvertTo-Json -Depth 24|ConvertFrom-Json
    Assert-ModelValue $route.links $links "sample/$sampleNumber/links"
    $publicRaw=[OpenKh.Egs.RawSnapshot]::Read($payload,$assetOffset)|ConvertTo-Json -Depth 8|ConvertFrom-Json
    Assert-ModelValue @($links.ordered_targets.candidate_original_asset_offset) @($publicRaw.offsets) "sample/$sampleNumber/public-raw-offsets"
    if($publicRaw.invalid -or $publicRaw.count -ne $links.ordered_targets.Count){throw 'Public RAW finite target count differs'}
    $models=@(foreach($model in $independentGeometry.samples|Where-Object sample -eq $sampleNumber){
        $pool=[Collections.Generic.List[object]]::new()
        foreach($part in $model.geometry.parts){foreach($uv in $part.uv_bits){$pool.Add($uv)}}
        $partOrdinal=0
        $parts=@(foreach($part in $model.geometry.parts){
            $environment=[int]([uint32]([long]$part.texture_index -band 0xffffffff) -band 0xffff)
            if($environment -ge @($links.ordered_targets|Where-Object kind -eq 'base').Count){throw 'Masked texture index outside independent layout'}
            $expanded=[Collections.Generic.List[object]]::new();$cross=0
            foreach($triangle in $part.triangles){foreach($corner in $triangle){
                $expanded.Add($pool[[int]$corner.uv_index]);$corners++
                if($corner.uv_index -lt $part.uv_base -or $corner.uv_index -ge $part.uv_base+$part.uv_bits.Count){$cross++;$crossPart++}
            }}
            @{part_ordinal=$partOrdinal;raw_texture_index=$part.texture_index;environment_ordinal=$environment;expanded_uv_bits=$expanded.ToArray();cross_part_uv_corners=$cross}
            $partOrdinal++
        })
        @{model_bar_ordinal=$model.bar_ordinal;geometry_profile=$model.geometry.profile;part_routes=$parts}
    })
    $expected=@{texture_bar_ordinal=$route.texture_bar_ordinal;links=$links;models=$models}|ConvertTo-Json -Depth 32|ConvertFrom-Json
    Assert-ModelValue $route $expected "sample/$sampleNumber/routing"
    $results+=@{sample=$sampleNumber;routing=$expected;public_raw=$publicRaw}
    $sampleNumber++
}
Write-NewResearchFile (Join-Path $routingDestination 'independent-model-texture-routing.json') ([Text.Encoding]::UTF8.GetBytes((@{verdict='PASS';pin=$pin;samples=$results;triangle_uv_corners=$corners;cross_part_uv_corners=$crossPart;unchanged_public_raw_and_stride_executed=$true;whole_payload_scanner_general_equivalence=$false;runtime_verified=$false}|ConvertTo-Json -Depth 40)))
"PASS: $sampleNumber finite routing observations, $corners UV corners and $crossPart global cross-part UV corners; installed routing unqualified."
