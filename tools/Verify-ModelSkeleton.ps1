# Independent scalar reads, guided by the pinned legacy layout. This does not
# execute the full OpenKH model reader or qualify mesh/transform/runtime behavior.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$OutputDirectory = Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$ProbePath = Get-ResearchCanonicalPath $ProbePath
$null = Assert-PrivateResearchDirectory (Split-Path -Parent $ProbePath) $InstallRoot
if ((Get-Item -LiteralPath $ProbePath).Length -gt 32*1024*1024) {throw 'Model report exceeds 32 MiB limit'}
$pin = '7a3b945c538d32c6a285128c98aefba093f52ceb'
$referenceFiles = @('OpenKh.Kh2/Bar.cs','OpenKh.Kh2/Mdlx.cs','OpenKh.Kh2/Mdlx.Model.cs','OpenKh.Kh2/Models/ModelCommon.cs')
if ((git -C $OpenKhRoot rev-parse HEAD) -ne $pin -or
    (git -C $OpenKhRoot status --porcelain -- $referenceFiles)) {throw 'Model references differ from the audited pin'}
$modelReport = Get-Content -LiteralPath $ProbePath -Raw | ConvertFrom-Json
if (!$modelReport.model_skeleton_inspection_requested -or $modelReport.schema_version -ne 1 -or
    $modelReport.qualification -ne 'bounded_original_sample_inspection_only' -or
    @($modelReport.samples).Count -lt 1 -or @($modelReport.samples).Count -gt 64) {
    throw 'Explicit bounded model inspection report required'
}
if (!('Kh2ModelFieldOracleV1' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Collections.Generic;
using System.Security.Cryptography;
public static class Kh2ModelFieldOracleV1 {
    public class Bone {
        public int[] header_i32;
        public short[] header_i16;
        public uint[] transform_bits;
    }
    public class Hierarchy {
        public int duplicate_indices, roots, missing_parent_indices, parents_at_or_after_child;
        public int? cyclic_parent_chains;
    }
    public class Block {
        public long offset;
        public int[] header_words;
        public short declared_bone_count, header_word_16_high_i16;
        public List<int[]> group_headers = new List<int[]>();
        public string unknown_data_sha256;
        public List<Bone> bones = new List<Bone>();
        public Hierarchy legacy_hierarchy;
    }
    static byte[] Exact(BinaryReader reader, int count) {
        var bytes = reader.ReadBytes(count);
        if (bytes.Length != count) throw new InvalidDataException("Truncated model fields");
        return bytes;
    }
    static int[] Words(BinaryReader reader, int count) {
        var words = new int[count];
        for (int i = 0; i < count; i++) words[i] = reader.ReadInt32();
        return words;
    }
    static Hierarchy Graph(List<Bone> bones) {
        var result = new Hierarchy { cyclic_parent_chains = 0 };
        var indices = new Dictionary<int,int>();
        for (int i = 0; i < bones.Count; i++) {
            if (indices.ContainsKey(bones[i].header_i32[0])) result.duplicate_indices++;
            indices[bones[i].header_i32[0]] = i;
        }
        if (result.duplicate_indices != 0) result.cyclic_parent_chains = null;
        for (int i = 0; i < bones.Count; i++) {
            int parent = bones[i].header_i32[1];
            int position;
            if (parent == -1) result.roots++;
            else if (!indices.TryGetValue(parent, out position)) result.missing_parent_indices++;
            else if (position >= i) result.parents_at_or_after_child++;
            if (result.duplicate_indices != 0) continue;
            var visited = new HashSet<int>();
            int cursor = i;
            while (true) {
                if (!visited.Add(cursor)) { result.cyclic_parent_chains++; break; }
                int next = bones[cursor].header_i32[1];
                if (next == -1 || !indices.TryGetValue(next, out cursor)) break;
            }
        }
        return result;
    }
    public static List<Block> Read(byte[] original, int ordinal) {
        using (var memory = new MemoryStream(original, false))
        using (var reader = new BinaryReader(memory)) {
            if (reader.ReadUInt32() != 0x01524142) throw new InvalidDataException("Not BAR");
            int count = reader.ReadInt32();
            if (ordinal < 0 || ordinal >= count) throw new InvalidDataException("Bad BAR ordinal");
            memory.Position = 16 + ordinal * 16;
            if (reader.ReadUInt16() != 4) throw new InvalidDataException("Not a model entry");
            memory.Position += 6;
            int start = reader.ReadInt32(), length = reader.ReadInt32();
            if (start < 0 || length < 0 || (long)start + length > original.Length || length > 32*1024*1024)
                throw new InvalidDataException("Bad BAR model range");
            memory.Position = start;
            byte[] payload = Exact(reader, length);
            using (var model = new MemoryStream(payload, false))
            using (var fields = new BinaryReader(model)) {
                var blocks = new List<Block>();
                long baseOffset = 0x90;
                while (true) {
                    if (blocks.Count == 16) throw new InvalidDataException("Too many blocks");
                    model.Position = baseOffset;
                    var block = new Block { offset = baseOffset, header_words = Words(fields, 8) };
                    model.Position = baseOffset + 16;
                    block.declared_bone_count = fields.ReadInt16();
                    block.header_word_16_high_i16 = fields.ReadInt16();
                    if (block.declared_bone_count < 0 || block.declared_bone_count > 2048 ||
                        block.header_words[7] < 0 || block.header_words[7] > 4096)
                        throw new InvalidDataException("Bad counts");
                    model.Position = baseOffset + 32;
                    for (int group = 0; group < block.header_words[7]; group++) block.group_headers.Add(Words(fields, 8));
                    if (block.header_words[6] < 0 || block.header_words[5] < 0) throw new InvalidDataException("Negative offset");
                    model.Position = baseOffset + block.header_words[6];
                    block.unknown_data_sha256 = Convert.ToHexString(SHA256.HashData(Exact(fields, 0x120))).ToLowerInvariant();
                    if (block.header_words[5] != 0) {
                        model.Position = baseOffset + block.header_words[5];
                        for (int bone = 0; bone < block.declared_bone_count; bone++) {
                            byte[] raw = Exact(fields, 64);
                            var record = new Bone { header_i32 = new int[4], header_i16 = new short[8], transform_bits = new uint[12] };
                            using (var m = new MemoryStream(raw, false))
                            using (var b = new BinaryReader(m)) {
                                record.header_i32 = Words(b, 4);
                                m.Position = 0;
                                for (int i = 0; i < 8; i++) record.header_i16[i] = b.ReadInt16();
                                for (int i = 0; i < 12; i++) record.transform_bits[i] = b.ReadUInt32();
                            }
                            block.bones.Add(record);
                        }
                    }
                    block.legacy_hierarchy = Graph(block.bones);
                    blocks.Add(block);
                    if (block.header_words[3] == 0) return blocks;
                    if (block.header_words[3] < 32) throw new InvalidDataException("Bad link");
                    baseOffset += block.header_words[3];
                }
            }
        }
    }
}
'@
}
function Write-ModelJson([string]$Name, $Value) {
    Write-NewResearchFile (Join-Path $OutputDirectory $Name) ([Text.Encoding]::UTF8.GetBytes(($Value | ConvertTo-Json -Depth 32)))
}
function Assert-ModelValue($Expected, $Observed, [string]$Location) {
    if ($null -eq $Expected -or $null -eq $Observed) {
        if (($null -eq $Expected) -ne ($null -eq $Observed)) {throw "Model null mismatch: $Location"}
    } elseif ($Expected -is [array]) {
        if (@($Observed).Count -ne $Expected.Count) {throw "Model array count mismatch: $Location"}
        for ($n=0; $n -lt $Expected.Count; $n++) {Assert-ModelValue $Expected[$n] $Observed[$n] "$Location/$n"}
    } elseif ($Expected -is [pscustomobject]) {
        $keys=@($Expected.psobject.Properties.Name | Sort-Object)
        $observedKeys=@($Observed.psobject.Properties.Name | Sort-Object)
        if (($keys -join '|') -ne ($observedKeys -join '|')) {throw "Model property mismatch: $Location"}
        foreach ($key in $keys) {Assert-ModelValue $Expected.$key $Observed.$key "$Location/$key"}
    } elseif ([string]$Expected -cne [string]$Observed) {throw "Model scalar mismatch: $Location"}
}
$results=@()
$sampleOrdinal=0
foreach ($sample in $modelReport.samples) {
    if (@($sample.model_skeletons).Count -lt 1) {throw 'Selected sample has no model observations'}
    $storageReport=Get-Content -LiteralPath $ProbePath -Raw | ConvertFrom-Json
    $storageReport.samples=@($sample)
    $storagePath=Join-Path $OutputDirectory ('storage-{0:d2}.json' -f $sampleOrdinal)
    Write-NewResearchFile $storagePath ([Text.Encoding]::UTF8.GetBytes(($storageReport | ConvertTo-Json -Depth 32)))
    $independentOriginal = & {
        . (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $storagePath -OpenKhRoot $OpenKhRoot | Out-Null
        ,$data
    }
    $barMemory=[IO.MemoryStream]::new([byte[]]$independentOriginal,$false)
    $barFields=[IO.BinaryReader]::new($barMemory)
    try {
        if ($barFields.ReadUInt32() -ne 0x01524142 -or $barFields.ReadInt32() -ne @($sample.bar.entries).Count -or
            $barFields.ReadInt32() -ne $sample.bar.header_field_8 -or $barFields.ReadInt32() -ne $sample.bar.header_field_12) {
            throw 'Independent model BAR header differs'
        }
        foreach ($entry in $sample.bar.entries) {
            if ($barFields.ReadUInt16() -ne $entry.entry_type -or $barFields.ReadInt16() -ne $entry.index -or
                ([Convert]::ToHexString($barFields.ReadBytes(4))) -cne ([Convert]::ToHexString([byte[]]$entry.name_bytes)) -or
                $barFields.ReadInt32() -ne $entry.offset -or $barFields.ReadInt32() -ne $entry.size) {
                throw 'Independent model BAR table differs'
            }
        }
    } finally {$barFields.Dispose();$barMemory.Dispose()}
    foreach ($modelObservation in $sample.model_skeletons) {
        if ($modelObservation.structure.profile -ne 'mdlx_legacy_3_4_structure_v1' -or $modelObservation.structure.runtime_verified) {
            throw 'Model profile/runtime qualification differs'
        }
        $blocks=[Kh2ModelFieldOracleV1]::Read($independentOriginal,[int]$modelObservation.bar_ordinal)
        # JSON normalizes public .NET field objects into arrays/scalars, retaining uint bits.
        $independentBlocks=ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject @($blocks.ToArray()) -Depth 32)
        Assert-ModelValue $modelObservation.structure.blocks $independentBlocks "sample/$sampleOrdinal/blocks"
        $results+=@{sample=$sampleOrdinal;bar_ordinal=$modelObservation.bar_ordinal;blocks=$independentBlocks}
    }
    $sampleOrdinal++
}
Write-ModelJson 'independent-model-fields.json' @{verdict='PASS';pin=$pin;reader='owned BinaryReader scalar adapter; full OpenKH model reader not executed';samples=$results;runtime_verified=$false}
Write-Output "PASS: $sampleOrdinal original model samples; raw headers, groups, bones, bit patterns and legacy hierarchy observations match independently."
