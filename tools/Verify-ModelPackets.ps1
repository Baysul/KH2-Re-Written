# Read-only packet reconstruction with unchanged pinned public VIF execution.
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
if((Get-Item -LiteralPath $ProbePath).Length -gt 32*1024*1024){throw 'Packet report exceeds limit'}
$pin='7a3b945c538d32c6a285128c98aefba093f52ceb'
$files=@('OpenKh.Kh2/Mdlx.Model.cs','OpenKh.Ps2/VifUnpacker.cs','OpenKh.Common/Helpers.cs','OpenKh.Ps2/VpuPacket.cs','OpenKh.Engine/Parsers/Kkdf2MdlxParser.cs')
if((git -C $OpenKhRoot rev-parse HEAD) -ne $pin -or (git -C $OpenKhRoot status --porcelain -- $files)){throw 'Packet reference differs from pin'}
$hashes=@(foreach($relative in $files){@{path=$relative;sha256=(Get-FileHash (Join-Path $OpenKhRoot $relative) -Algorithm SHA256).Hash.ToLowerInvariant()}})
Write-NewResearchFile (Join-Path $OutputDirectory 'reference-hashes.json') ([Text.Encoding]::UTF8.GetBytes(($hashes | ConvertTo-Json -Depth 6)))
# Execute the whole unchanged VifUnpacker class. Supply only its actual helper,
# extracted unchanged from the pinned file; unrelated YAML/file helpers stay out.
if(!('OpenKh.Ps2.VifUnpacker' -as [type])){
    $vif=Get-Content (Join-Path $OpenKhRoot 'OpenKh.Ps2/VifUnpacker.cs') -Raw
    $helpers=Get-Content (Join-Path $OpenKhRoot 'OpenKh.Common/Helpers.cs') -Raw
    $align=[regex]::Match($helpers,'(?s)public static int Align\(int offset, int alignment\)\s*\{.*?\n\s*\}')
    if(!$align.Success){throw 'Pinned alignment helper not found'}
    Add-Type -TypeDefinition ($vif+'namespace OpenKh.Common {public static class Helpers {'+$align.Value+'}}')
}
if(!('Kh2DmaPacketOracleV1' -as [type])){
Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Collections.Generic;
using System.Security.Cryptography;
public static class Kh2DmaPacketOracleV1 {
    public class Tag { public int offset; public uint[] words; }
    public class Packet {
        public int offset, code_offset, code_length, base_address;
        public string code_sha256;
        public List<Tag> tags = new List<Tag>();
        public byte[] code;
    }
    public class Group {
        public int block_offset, group_ordinal;
        public int[] matrix_entries;
        public List<Packet> packets = new List<Packet>();
    }
    public class VpuData {
        public int[] normal_words, matrix_counts, weight_group_counts;
        public int[][] strips, colors;
        public uint[][] vertex_bits, normal_bits;
        public List<List<int[]>> weighted_indices = new List<List<int[]>>();
    }
    static int[] Words(byte[] memory, int offset, int count) {
        if(offset<0 || count<0 || (long)offset+4L*count>memory.Length) throw new InvalidDataException("VU range exceeds memory");
        using(var stream=new MemoryStream(memory,false)) using(var reader=new BinaryReader(stream)) {
            stream.Position=offset;
            var result=new int[count];
            for(int i=0;i<count;i++)result[i]=reader.ReadInt32();
            return result;
        }
    }
    static int[] Area(byte[] memory,int quad,int count,int width) {
        if(quad<0 || count<0 || (long)quad*16>int.MaxValue || (long)count*width>int.MaxValue) throw new InvalidDataException("Bad VU range");
        return Words(memory,quad*16,count*width);
    }
    static int[][] Rows(int[] words) {
        var rows=new int[words.Length/4][];
        for(int i=0;i<rows.Length;i++) {rows[i]=new int[4];Array.Copy(words,i*4,rows[i],0,4);}
        return rows;
    }
    static uint[][] Bits(int[] words) {
        var rows=new uint[words.Length/4][];
        for(int i=0;i<rows.Length;i++) {rows[i]=new uint[4];for(int j=0;j<4;j++)rows[i][j]=unchecked((uint)words[i*4+j]);}
        return rows;
    }
    public static VpuData ReadVpu(byte[] memory) {
        int[] h=Words(memory,0,16);
        if(h[0]!=1)return null;
        var data=new VpuData();
        bool normals=(long)h[13]!=(long)h[5]+h[4]+h[8];
        data.normal_words=normals?Words(memory,64,2):null;
        data.normal_bits=normals?Bits(Area(memory,data.normal_words[1],data.normal_words[0],4)):new uint[0][];
        data.matrix_counts=Area(memory,h[6],h[15],1);
        data.strips=Rows(Area(memory,h[5],h[4],4));
        data.colors=Rows(Area(memory,h[9],h[8],4));
        data.vertex_bits=Bits(Area(memory,h[13],h[12],4));
        data.weight_group_counts=Area(memory,h[11],h[10],1);
        if(data.weight_group_counts.Length>32)throw new InvalidDataException("Too many weight groups");
        int cursor=h[11]*16+data.weight_group_counts.Length*4;
        for(int group=0;group<data.weight_group_counts.Length;group++) {
            int count=data.weight_group_counts[group],width=group+1;
            if(count<0 || count>4096)throw new InvalidDataException("Bad weight count");
            cursor=(cursor+15)&~15;
            int[] values=Words(memory,cursor,count*width);
            cursor+=values.Length*4;
            var entries=new List<int[]>();
            for(int entry=0;entry<count;entry++) {var indices=new int[width];Array.Copy(values,entry*width,indices,0,width);entries.Add(indices);}
            data.weighted_indices.Add(entries);
        }
        return data;
    }
    public static List<Group> Read(byte[] original, int barOrdinal) {
        using(var source=new MemoryStream(original,false)) using(var r=new BinaryReader(source)) {
            if(r.ReadUInt32()!=0x01524142) throw new InvalidDataException("Not BAR");
            int count=r.ReadInt32();
            if(barOrdinal<0 || barOrdinal>=count) throw new InvalidDataException("Bad ordinal");
            source.Position=16+16*barOrdinal;
            if(r.ReadUInt16()!=4) throw new InvalidDataException("Not model");
            source.Position+=6;
            int start=r.ReadInt32(), length=r.ReadInt32();
            if(start<0 || length<0 || (long)start+length>original.Length || length>32*1024*1024) throw new InvalidDataException("Bad model range");
            source.Position=start;
            byte[] model=r.ReadBytes(length);
            using(var stream=new MemoryStream(model,false)) using(var f=new BinaryReader(stream)) {
                var groups=new List<Group>();
                int totalPackets=0,totalTags=0,totalCodeBytes=0;
                int block=0x90;
                for(int iteration=0;iteration<16;iteration++) {
                    stream.Position=block+12;
                    int next=f.ReadInt32();
                    stream.Position=block+28;
                    int groupCount=f.ReadInt32();
                    if(groupCount<0 || groupCount>4096) throw new InvalidDataException("Bad groups");
                    for(int g=0;g<groupCount;g++) {
                        stream.Position=block+32+g*32+16;
                        int dma=f.ReadInt32(), matrices=f.ReadInt32();
                        if(dma<0 || matrices<0) throw new InvalidDataException("Negative offsets");
                        stream.Position=block+matrices;
                        int matrixCount=f.ReadInt32();
                        if(matrixCount<0 || matrixCount>4096) throw new InvalidDataException("Bad matrix count");
                        var group=new Group {block_offset=block,group_ordinal=g,matrix_entries=new int[matrixCount]};
                        var starts=new List<int>{dma};
                        int cursor=dma+16;
                        for(int m=0;m<matrixCount;m++) {
                            int entry=f.ReadInt32(); group.matrix_entries[m]=entry;
                            if(entry==-1) {starts.Add(cursor+16);cursor+=32;} else cursor+=16;
                        }
                        foreach(int offset in starts) {
                            if(++totalPackets>4096)throw new InvalidDataException("Too many cumulative packets");
                            var packet=new Packet {offset=offset};
                            stream.Position=block+offset;
                            bool closed=false;
                            for(int t=0;t<4096;t++) {
                                if(++totalTags>65536)throw new InvalidDataException("Too many cumulative tags");
                                var tag=new Tag {offset=(int)stream.Position-block,words=new uint[4]};
                                for(int w=0;w<4;w++) tag.words[w]=f.ReadUInt32();
                                packet.tags.Add(tag);
                                if((tag.words[0]&0xffff)==0){closed=true;break;}
                            }
                            if(!closed) throw new InvalidDataException("Unterminated DMA tags");
                            uint[] first=packet.tags[0].words;
                            packet.code_offset=(int)(first[1]&0x7fffffff);
                            packet.code_length=(int)(first[0]&0xffff)*16;
                            totalCodeBytes+=packet.code_length;
                            if(totalCodeBytes>32*1024*1024)throw new InvalidDataException("Too many cumulative VIF bytes");
                            packet.base_address=packet.code_length==0?0:unchecked((int)packet.tags[1].words[3]);
                            if((long)block+packet.code_offset+packet.code_length>model.Length) throw new InvalidDataException("Bad VIF range");
                            stream.Position=block+packet.code_offset;
                            packet.code=f.ReadBytes(packet.code_length);
                            packet.code_sha256=Convert.ToHexString(SHA256.HashData(packet.code)).ToLowerInvariant();
                            group.packets.Add(packet);
                        }
                        groups.Add(group);
                    }
                    if(next==0)return groups;
                    if(next<32)throw new InvalidDataException("Bad block link");
                    block+=next;
                }
                throw new InvalidDataException("Too many blocks");
            }
        }
    }
}
'@
}
# Skeleton verifier checks BAR/structure and produces one private storage binding
# per sample. Retain the destination while exposing its comparison helper;
# read the packet report afterward to avoid working-variable collisions.
$skeletonDir=Join-Path $OutputDirectory 'skeleton'
New-Item -ItemType Directory $skeletonDir | Out-Null
$packetOutputDirectory=$OutputDirectory
. (Join-Path $PSScriptRoot 'Verify-ModelSkeleton.ps1') -InstallRoot $InstallRoot -ProbePath $ProbePath -OpenKhRoot $OpenKhRoot -OutputDirectory $skeletonDir | Out-Null
$OutputDirectory=$packetOutputDirectory
$packetReport=Get-Content $ProbePath -Raw | ConvertFrom-Json
if(!$Preflight -and (!$packetReport.model_packet_inspection_requested -or
    $packetReport.model_packet_profile -ne 'openkh_legacy_dma_vif_first_run_v1' -or
    $packetReport.vif_cpu_profile -ne 'openkh_vif_first_run_zero_tops_v1')){throw 'Explicit packet inspection/profile required'}
$results=@();$sampleNumber=0
foreach($sample in $packetReport.samples){
    $storagePath=Join-Path $skeletonDir ('storage-{0:d2}.json' -f $sampleNumber)
    $original=& {
        . (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $storagePath -OpenKhRoot $OpenKhRoot | Out-Null
        ,$data
    }
    foreach($modelObservation in $sample.model_skeletons){
        $groups=[Kh2DmaPacketOracleV1]::Read($original,[int]$modelObservation.bar_ordinal)
        $normalized=@(foreach($group in $groups){
            $packets=@(foreach($packet in $group.packets){
                $unpacker=[OpenKh.Ps2.VifUnpacker]::new($packet.code)
                $state=$unpacker.Run()
                $counter=[OpenKh.Ps2.VifUnpacker].GetField('_programCounter',[Reflection.BindingFlags]'Instance,NonPublic').GetValue($unpacker)
                $header=@(0..15 | ForEach-Object {[BitConverter]::ToInt32($unpacker.Memory,4*$_)})
                @{offset=$packet.offset;code_offset=$packet.code_offset;code_length=$packet.code_length;base_address=$packet.base_address;
                code_sha256=$packet.code_sha256;tags=@($packet.tags.ToArray());
                memory_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($unpacker.Memory)).ToLowerInvariant();
                program_counter=$counter;stop_state=$state.ToString().ToLowerInvariant();vpu_header_words=$header;
                vpu_data=[Kh2DmaPacketOracleV1]::ReadVpu($unpacker.Memory)}
            })
            @{block_offset=$group.block_offset;group_ordinal=$group.group_ordinal;matrix_entries=@($group.matrix_entries);packets=$packets}
        })
        $normalized=ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $normalized -Depth 32)
        if(!$Preflight){Assert-ModelValue $modelObservation.packet_groups $normalized "sample/$sampleNumber/packet_groups"}
        $results+=@{sample=$sampleNumber;bar_ordinal=$modelObservation.bar_ordinal;groups=$normalized}
    }
    $sampleNumber++
}
$output=@{verdict= $(if($Preflight){'OBSERVATIONS_ONLY'}else{'PASS'});pin=$pin;public_vif_unpacker_executed_unchanged=$true;tops=0;samples=$results;runtime_verified=$false}
Write-NewResearchFile (Join-Path $OutputDirectory 'independent-model-packets.json') ([Text.Encoding]::UTF8.GetBytes(($output | ConvertTo-Json -Depth 32)))
Write-Output "Collected packet observations for $sampleNumber models; preflight=$Preflight; runtime unqualified."
