# Read-only topology oracle. Public methods are extracted unchanged at the pin;
# owned input shapes bridge independently reconstructed packet/scalar data.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [switch]$Preflight
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$OutputDirectory=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$ProbePath=Get-ResearchCanonicalPath $ProbePath
$null=Assert-PrivateResearchDirectory (Split-Path -Parent $ProbePath) $InstallRoot
if((Get-Item -LiteralPath $ProbePath).Length -gt 32*1024*1024){throw 'Geometry report exceeds limit'}
$geometryDestination=$OutputDirectory
$geometryPreflight=$Preflight
$packetDirectory=Join-Path $OutputDirectory 'packets'
New-Item -ItemType Directory $packetDirectory | Out-Null
# Packet verification freshly reconstructs source storage and VIF memories.
. (Join-Path $PSScriptRoot 'Verify-ModelPackets.ps1') -InstallRoot $InstallRoot -ProbePath $ProbePath -OpenKhRoot $OpenKhRoot -OutputDirectory $packetDirectory | Out-Null
$OutputDirectory=$geometryDestination
$Preflight=$geometryPreflight
$pin='7a3b945c538d32c6a285128c98aefba093f52ceb'
$files=@('OpenKh.Ps2/VpuPacketExtensions.cs','OpenKh.Ps2/VpuPacket.cs','OpenKh.Engine/Parsers/Kkdf2MdlxParser.cs')
if((git -C $OpenKhRoot rev-parse HEAD) -ne $pin -or (git -C $OpenKhRoot status --porcelain -- $files)){throw 'Topology reference differs from pin'}
$hashes=@(foreach($relative in $files){@{path=$relative;sha256=(Get-FileHash (Join-Path $OpenKhRoot $relative) -Algorithm SHA256).Hash.ToLowerInvariant()}})
function Get-BracedSource([string]$Source,[string]$Declaration) {
    $start=$Source.IndexOf($Declaration,[StringComparison]::Ordinal)
    if($start -lt 0){throw "Pinned declaration absent: $Declaration"}
    $brace=$Source.IndexOf('{',$start);$depth=0
    for($cursor=$brace;$cursor -lt $Source.Length;$cursor++){
        if($Source[$cursor] -eq '{'){$depth++}
        if($Source[$cursor] -eq '}'){$depth--;if($depth -eq 0){return $Source.Substring($start,$cursor-$start+1)}}
    }
    throw 'Unclosed pinned declaration'
}
$engine=Get-Content (Join-Path $OpenKhRoot $files[2]) -Raw
$method=Get-BracedSource $engine 'private ExportedMesh PreProcessVerticesAndBuildModel()'
$support=@('private class VertexRef','private class TriangleRef','private class ExportedMesh') | ForEach-Object {Get-BracedSource $engine $_}
$enum=Get-BracedSource (Get-Content (Join-Path $OpenKhRoot $files[1]) -Raw) 'public enum VertexFunction'
$extracts=@(foreach($s in @($method)+$support+@($enum)){@{sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($s))).ToLowerInvariant();length=$s.Length}})
Write-NewResearchFile (Join-Path $OutputDirectory 'reference-hashes.json') ([Text.Encoding]::UTF8.GetBytes((@{pin=$pin;files=$hashes;unchanged_extracts=$extracts} | ConvertTo-Json -Depth 8)))
if(!('Kh2TopologyOracleV1' -as [type])){
$shape=@'
using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using OpenKh.Ps2;
#pragma warning disable 0649
namespace OpenKh.Ps2 {
    // Owned adapter: no public VpuPacket.Read or full MDLX reader executes.
    public class VpuPacket {
        public class VertexIndex {public int U,V,Index; public VertexFunction Function;}
        public class VertexCoord {public float X,Y,Z,W;}
        public int[] VertexRange;
        public int[][][] VertexWeightedIndices;
        public int VertexWeightedCount;
        public VertexIndex[] Indices;
        public VertexCoord[] Vertices;
'@
$shape += $enum + '}}'
$wrapper=@'
public class Kh2TopologyOracleV1 {
    public class InputPacket {
        public int[][] strips;
        public uint[][] vertex_bits;
        public int[] matrix_counts, matrices;
        public int[][][] weighted_indices;
    }
    public class InputGroup {public int texture_index,render_flags; public InputPacket[] packets;}
    private class DmaVif {public int[] Alaxi;}
    private class DmaChain {public DmaVif[] DmaVifs;}
    private class ImmutableMesh {
        public int TextureIndex;
        public bool IsOpaque;
        public DmaChain DmaChain;
        public List<VpuPacket> VpuPackets;
    }
    private List<ImmutableMesh> immultableMeshList;
    public class Binding {public int matrix_index,vertex_index;}
    public class Ref {public int vertex_index,uv_index;}
    public class Part {
        public int group_ordinal,packet_ordinal,texture_index,vertex_base,uv_base;
        public bool is_opaque;
        public Binding[][] assignments;
        public uint[][] uv_bits;
        public Ref[][] triangles;
    }
    public Part[] Run(InputGroup[] groups) {
        immultableMeshList=groups.Select(g=>new ImmutableMesh {
            TextureIndex=g.texture_index, IsOpaque=(g.render_flags&1)==0,
            DmaChain=new DmaChain {DmaVifs=g.packets.Select(p=>new DmaVif {Alaxi=p.matrices}).ToArray()},
            VpuPackets=g.packets.Select(p=>new VpuPacket {
                VertexRange=p.matrix_counts, VertexWeightedIndices=p.weighted_indices,
                VertexWeightedCount=p.weighted_indices.Sum(slice=>slice.Length),
                Indices=p.strips.Select(s=>new VpuPacket.VertexIndex {U=s[0],V=s[1],Index=s[2],Function=(VpuPacket.VertexFunction)s[3]}).ToArray(),
                Vertices=p.vertex_bits.Select(b=>new VpuPacket.VertexCoord {X=BitConverter.Int32BitsToSingle(unchecked((int)b[0])),Y=BitConverter.Int32BitsToSingle(unchecked((int)b[1])),Z=BitConverter.Int32BitsToSingle(unchecked((int)b[2])),W=BitConverter.Int32BitsToSingle(unchecked((int)b[3]))}).ToArray()
            }).ToList()
        }).ToList();
        var mesh=PreProcessVerticesAndBuildModel();
        var result=new List<Part>();
        int flat=0,vertices=0,uv=0;
        for(int g=0;g<groups.Length;g++) for(int p=0;p<groups[g].packets.Length;p++,flat++) {
            var part=mesh.partList[flat];
            int uvCount=groups[g].packets[p].strips.Length;
            result.Add(new Part {
                group_ordinal=g,packet_ordinal=p,texture_index=part.TextureIndex,is_opaque=part.IsOpaque,
                vertex_base=vertices,uv_base=uv,
                assignments=mesh.vertexAssignments[flat].Select(a=>a.Select(b=>new Binding {matrix_index=b.MatrixIndex,vertex_index=b.VertexIndex}).ToArray()).ToArray(),
                uv_bits=mesh.uvList.Skip(uv).Take(uvCount).Select(v=>new uint[]{BitConverter.SingleToUInt32Bits(v.X),BitConverter.SingleToUInt32Bits(v.Y)}).ToArray(),
                triangles=part.triangleRefList.Select(t=>t.list.Select(r=>new Ref {vertex_index=r.vertexIndex,uv_index=r.uvIndex}).ToArray()).ToArray()
            });
            vertices+=mesh.vertexAssignments[flat].Length;uv+=uvCount;
        }
        return result.ToArray();
    }
'@
$extensions=Get-Content (Join-Path $OpenKhRoot $files[0]) -Raw
# Keep all using clauses at the front, before owned/public namespaces.
$extensions=$extensions.Replace('using System.Linq;','')
Add-Type -TypeDefinition ($shape+$extensions+$wrapper+($support -join "`n")+$method+'}')
}
$geometryReport=Get-Content $ProbePath -Raw | ConvertFrom-Json
if(!$Preflight -and (!$geometryReport.model_geometry_inspection_requested -or $geometryReport.model_geometry_profile -ne 'openkh_kkdf2_type1_topology_v1')){throw 'Explicit geometry inspection/profile required'}
$independentPackets=Get-Content (Join-Path $packetDirectory 'independent-model-packets.json') -Raw | ConvertFrom-Json
$independentFields=Get-Content (Join-Path $packetDirectory 'skeleton/independent-model-fields.json') -Raw | ConvertFrom-Json
$results=@()
foreach($packetSample in $independentPackets.samples){
    $fieldSample=@($independentFields.samples | Where-Object {$_.sample -eq $packetSample.sample -and $_.bar_ordinal -eq $packetSample.bar_ordinal})
    if($fieldSample.Count -ne 1){throw 'Ambiguous independent model binding'}
    $root=$fieldSample[0].blocks[0]
    if($root.offset -ne 0x90 -or $root.header_words[0] -ne 3){throw 'Unsupported public topology root'}
    $inputGroups=@(foreach($group in $packetSample.groups | Where-Object block_offset -eq 0x90){
        $header=$root.group_headers[$group.group_ordinal]
        $matrixLists=[Collections.Generic.List[object]]::new();$current=[Collections.Generic.List[int]]::new()
        foreach($entry in $group.matrix_entries){
            if($entry -eq -1){$matrixLists.Add($current.ToArray());$current=[Collections.Generic.List[int]]::new()}else{$current.Add($entry)}
        }
        $matrixLists.Add($current.ToArray())
        if($matrixLists.Count -ne $group.packets.Count){throw 'Independent matrix packet shape differs'}
        $inputPackets=@(for($p=0;$p -lt $group.packets.Count;$p++){
            $data=$group.packets[$p].vpu_data
            if(!$data){throw 'Root packet lacks independently reconstructed type-1 data'}
            $input=[Kh2TopologyOracleV1+InputPacket]::new()
            $input.strips=$data.strips;$input.vertex_bits=$data.vertex_bits;$input.matrix_counts=$data.matrix_counts
            $input.matrices=$matrixLists[$p];$input.weighted_indices=$data.weighted_indices
            $input
        })
        $inputGroup=[Kh2TopologyOracleV1+InputGroup]::new()
        $inputGroup.texture_index=$header[1];$inputGroup.render_flags=$header[0];$inputGroup.packets=$inputPackets
        $inputGroup
    })
    $parts=[Kh2TopologyOracleV1]::new().Run([Kh2TopologyOracleV1+InputGroup[]]$inputGroups)
    $geometry=@{profile='openkh_kkdf2_type1_topology_v1';block_offset=144;excluded_linked_groups=@($packetSample.groups | Where-Object block_offset -ne 144).Count;parts=@($parts);runtime_verified=$false}
    $geometry=ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $geometry -Depth 32)
    if(!$Preflight){
        $model=@($geometryReport.samples[$packetSample.sample].model_skeletons | Where-Object bar_ordinal -eq $packetSample.bar_ordinal)
        if($model.Count -ne 1){throw 'Ambiguous geometry report model'}
        Assert-ModelValue $model[0].geometry $geometry "sample/$($packetSample.sample)/geometry"
    }
    $results+=@{sample=$packetSample.sample;bar_ordinal=$packetSample.bar_ordinal;geometry=$geometry}
}
$output=@{verdict=$(if($Preflight){'OBSERVATIONS_ONLY'}else{'PASS'});pin=$pin;public_extensions_and_preprocess_executed_unchanged=$true;full_model_or_vpu_reader_executed=$false;numerical_skinning_executed=$false;runtime_verified=$false;samples=$results}
Write-NewResearchFile (Join-Path $OutputDirectory 'independent-model-geometry.json') ([Text.Encoding]::UTF8.GetBytes(($output | ConvertTo-Json -Depth 32)))
Write-Output "Topology comparison collected for $($results.Count) models; preflight=$Preflight; runtime unqualified."
