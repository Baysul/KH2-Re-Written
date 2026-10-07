# Research-only comparison of competing public bind routes. No installed route
# is selected. Caller records full selected-source scans before/after inspection.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$OutputDirectory=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$bindDestination=$OutputDirectory
$bindProbePath=$ProbePath
$geometryDirectory=Join-Path $OutputDirectory 'geometry'
New-Item -ItemType Directory $geometryDirectory | Out-Null
. (Join-Path $PSScriptRoot 'Verify-ModelGeometry.ps1') -InstallRoot $InstallRoot -ProbePath $ProbePath -OpenKhRoot $OpenKhRoot -OutputDirectory $geometryDirectory | Out-Null
$OutputDirectory=$bindDestination
$ProbePath=$bindProbePath
$pin='7a3b945c538d32c6a285128c98aefba093f52ceb'
$files=@('OpenKh.Engine/Parsers/MdlxParser.cs','OpenKh.Kh2/Models/ModelCommon.cs','OpenKh.Kh2/Mdlx.Model.cs','OpenKh.Engine/Parsers/Kkdf2MdlxParser.cs')
if((git -C $OpenKhRoot rev-parse HEAD) -ne $pin -or (git -C $OpenKhRoot status --porcelain -- $files)){throw 'Bind reference differs from pin'}
$legacy=Get-BracedSource (Get-Content (Join-Path $OpenKhRoot $files[0]) -Raw) 'private static Matrix4x4[] BuildTPoseMatrices(Mdlx.SubModel model, Matrix4x4 initialMatrix)'
$modern=Get-BracedSource (Get-Content (Join-Path $OpenKhRoot $files[1]) -Raw) 'public static Matrix4x4[] GetBoneMatrices(List<Bone> boneList)'
$hashes=@(foreach($relative in $files){@{path=$relative;sha256=(Get-FileHash (Join-Path $OpenKhRoot $relative) -Algorithm SHA256).Hash.ToLowerInvariant()}})
$extracts=@(foreach($s in @($legacy,$modern)){@{sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($s))).ToLowerInvariant();length=$s.Length}})
Write-NewResearchFile (Join-Path $OutputDirectory 'reference-hashes.json') ([Text.Encoding]::UTF8.GetBytes((@{pin=$pin;files=$hashes;unchanged_bind_methods=$extracts} | ConvertTo-Json -Depth 8)))
if(!('Kh2BindRoutesOracleV1' -as [type])){
$owned=@'
using System;
using System.Collections.Generic;
using System.Linq;
using System.Numerics;
using OpenKh.Kh2;
namespace OpenKh.Kh2 {
    // Owned shape boundary; no public reader/constructor or target code loads.
    public class Mdlx {
        public class Bone {
            public int Parent;
            public float ScaleX,ScaleY,ScaleZ,RotationX,RotationY,RotationZ,TranslationX,TranslationY,TranslationZ;
        }
        public class SubModel {public List<Bone> Bones;}
    }
}
public class Kh2BindRoutesOracleV1 {
    public class Bone : Mdlx.Bone {public int ParentIndex;}
    public class InputBone {public int[] header_i32;public short[] header_i16;public uint[] transform_bits;}
    public class Binding {public int matrix_index,vertex_index;}
    public class InputPart {public uint[][] vertex_bits;public Binding[][] assignments;}
    public class Result {
        public uint[][] legacy_matrix_bits,modern_matrix_bits,legacy_position_bits,modern_position_bits;
        public int matrix_records_different,matrix_words_different,position_records_different,position_words_different;
        public double maximum_matrix_absolute_difference,maximum_position_absolute_difference;
    }
    static float Float(uint bits) {return BitConverter.Int32BitsToSingle(unchecked((int)bits));}
    static Bone Read(InputBone input,int ordinal) {
        if(input.header_i32.Length!=4 || input.header_i16.Length!=8 || input.transform_bits.Length!=12)throw new ArgumentException("Bad bone shape");
        foreach(uint b in input.transform_bits)if(!float.IsFinite(Float(b)))throw new ArgumentException("Nonfinite bone input");
        int legacy=input.header_i32[1],modern=input.header_i16[2];
        // These public algorithms consume parents as array positions, in order.
        // Reject unqualified forward/cyclic parent dependencies before execution.
        if(legacy>=ordinal || modern>=ordinal)throw new ArgumentException("Parent not before child");
        var t=input.transform_bits;
        return new Bone {Parent=legacy,ParentIndex=modern,
            ScaleX=Float(t[0]),ScaleY=Float(t[1]),ScaleZ=Float(t[2]),
            RotationX=Float(t[4]),RotationY=Float(t[5]),RotationZ=Float(t[6]),
            TranslationX=Float(t[8]),TranslationY=Float(t[9]),TranslationZ=Float(t[10])};
    }
    static uint[] Bits(Matrix4x4 m) {
        float[] words={m.M11,m.M12,m.M13,m.M14,m.M21,m.M22,m.M23,m.M24,m.M31,m.M32,m.M33,m.M34,m.M41,m.M42,m.M43,m.M44};
        if(words.Any(x=>!float.IsFinite(x)))throw new ArgumentException("Nonfinite bind output");
        return words.Select(BitConverter.SingleToUInt32Bits).ToArray();
    }
    static uint[][] Skin(InputPart[] parts,Matrix4x4[] matrices) {
        var result=new List<uint[]>();
        int vertices=0,bindings=0;
        foreach(var part in parts) foreach(var assignment in part.assignments) {
            bindings+=assignment.Length;
            if(assignment.Length==0 || assignment.Length>32 || ++vertices>262144 || bindings>262144)throw new ArgumentException("Bad assignment count");
            Vector3 position=Vector3.Zero;
            foreach(var b in assignment) {
                if(b.matrix_index<0 || b.matrix_index>=matrices.Length || b.vertex_index<0 || b.vertex_index>=part.vertex_bits.Length)throw new ArgumentException("Binding out of range");
                var words=part.vertex_bits[b.vertex_index];
                if(words.Length!=4 || words.Any(w=>!float.IsFinite(Float(w))))throw new ArgumentException("Bad coordinate input");
                var vertex=new Vector4(Float(words[0]),Float(words[1]),Float(words[2]),Float(words[3]));
                // Owned adapter follows Kkdf2's distinct single/multi expressions.
                if(assignment.Length==1)position=Vector3.Transform(new Vector3(vertex.X,vertex.Y,vertex.Z),matrices[b.matrix_index]);
                else {var p=Vector4.Transform(vertex,matrices[b.matrix_index]);position+=new Vector3(p.X,p.Y,p.Z);}
            }
            if(!float.IsFinite(position.X) || !float.IsFinite(position.Y) || !float.IsFinite(position.Z))throw new ArgumentException("Nonfinite position output");
            result.Add(new uint[]{BitConverter.SingleToUInt32Bits(position.X),BitConverter.SingleToUInt32Bits(position.Y),BitConverter.SingleToUInt32Bits(position.Z)});
        }
        return result.ToArray();
    }
    static void Compare(uint[][] a,uint[][] b,out int records,out int words,out double max) {
        if(a.Length!=b.Length)throw new ArgumentException("Output shapes differ");
        records=0;words=0;max=0;
        for(int i=0;i<a.Length;i++) {
            bool different=false;
            for(int j=0;j<a[i].Length;j++) {
                if(a[i][j]!=b[i][j]) {words++;different=true;}
                max=Math.Max(max,Math.Abs((double)Float(a[i][j])-Float(b[i][j])));
            }
            if(different)records++;
        }
    }
    public static Result Run(InputBone[] input,InputPart[] parts) {
        if(input.Length>2048 || parts.Length>4096)throw new ArgumentException("Bind input exceeds bounds");
        var bones=input.Select((b,i)=>Read(b,i)).ToList();
        var legacy=BuildTPoseMatrices(new Mdlx.SubModel {Bones=bones.Cast<Mdlx.Bone>().ToList()},Matrix4x4.Identity);
        var modern=GetBoneMatrices(bones);
        var result=new Result {legacy_matrix_bits=legacy.Select(Bits).ToArray(),modern_matrix_bits=modern.Select(Bits).ToArray(),legacy_position_bits=Skin(parts,legacy),modern_position_bits=Skin(parts,modern)};
        Compare(result.legacy_matrix_bits,result.modern_matrix_bits,out result.matrix_records_different,out result.matrix_words_different,out result.maximum_matrix_absolute_difference);
        Compare(result.legacy_position_bits,result.modern_position_bits,out result.position_records_different,out result.position_words_different,out result.maximum_position_absolute_difference);
        return result;
    }
    public static void SyntheticControls() {
        uint one=BitConverter.SingleToUInt32Bits(1),two=BitConverter.SingleToUInt32Bits(2),ten=BitConverter.SingleToUInt32Bits(10);
        var root=new InputBone {header_i32=new int[]{0,-1,0,0},header_i16=new short[]{0,0,-1,0,0,0,0,0},transform_bits=new uint[]{two,two,two,0,0,0,0,0,ten,0,0,0}};
        var child=new InputBone {header_i32=new int[]{1,0,0,0},header_i16=new short[]{1,0,0,0,0,0,0,0},transform_bits=new uint[]{one,one,one,0,0,0,0,0,one,0,0,0}};
        var part=new InputPart {vertex_bits=new uint[][]{new uint[]{two,0,0,0}},assignments=new Binding[][]{new Binding[]{new Binding {matrix_index=1,vertex_index=0}}}};
        var result=Run(new InputBone[]{root,child},new InputPart[]{part});
        if(Float(result.legacy_matrix_bits[1][12])!=11 || Float(result.modern_matrix_bits[1][12])!=12 || Float(result.legacy_position_bits[0][0])!=13 || Float(result.modern_position_bits[0][0])!=16)throw new Exception("Authored scale/parent/single-weight control failed");
        uint half=BitConverter.SingleToUInt32Bits(0.5f);
        var multi=new InputPart {vertex_bits=new uint[][]{new uint[]{one,0,0,half},new uint[]{two,0,0,half}},assignments=new Binding[][]{new Binding[]{new Binding {matrix_index=0,vertex_index=0},new Binding {matrix_index=1,vertex_index=1}}}};
        var weighted=Run(new InputBone[]{root,child},new InputPart[]{multi});
        if(Float(weighted.legacy_position_bits[0][0])!=13.5f || Float(weighted.modern_position_bits[0][0])!=17)throw new Exception("Authored weighted-translation control failed");
        child.header_i32[1]=1;
        bool rejected=false;try{Run(new InputBone[]{root,child},new InputPart[]{part});}catch(ArgumentException){rejected=true;}
        if(!rejected)throw new Exception("Forward/cyclic parent control accepted");
        child.header_i32[1]=0;part.assignments[0][0].matrix_index=2;
        rejected=false;try{Run(new InputBone[]{root,child},new InputPart[]{part});}catch(ArgumentException){rejected=true;}
        if(!rejected)throw new Exception("Out of range matrix control accepted");
        part.assignments[0][0].matrix_index=1;root.transform_bits[0]=0x7fc01234;
        rejected=false;try{Run(new InputBone[]{root,child},new InputPart[]{part});}catch(ArgumentException){rejected=true;}
        if(!rejected)throw new Exception("Nonfinite transform control accepted");
    }
'@
Add-Type -TypeDefinition ($owned+$legacy+$modern+'}')
}
[Kh2BindRoutesOracleV1]::SyntheticControls()
$geometryEvidence=Get-Content (Join-Path $geometryDirectory 'independent-model-geometry.json') -Raw | ConvertFrom-Json
$fieldEvidence=Get-Content (Join-Path $geometryDirectory 'packets/skeleton/independent-model-fields.json') -Raw | ConvertFrom-Json
$packetEvidence=Get-Content (Join-Path $geometryDirectory 'packets/independent-model-packets.json') -Raw | ConvertFrom-Json
$results=@()
foreach($geometrySample in $geometryEvidence.samples){
    $fields=@($fieldEvidence.samples | Where-Object {$_.sample -eq $geometrySample.sample -and $_.bar_ordinal -eq $geometrySample.bar_ordinal})
    $packets=@($packetEvidence.samples | Where-Object {$_.sample -eq $geometrySample.sample -and $_.bar_ordinal -eq $geometrySample.bar_ordinal})
    if($fields.Count -ne 1 -or $packets.Count -ne 1){throw 'Ambiguous independent bind source'}
    $inputBones=@(foreach($bone in $fields[0].blocks[0].bones){
        $input=[Kh2BindRoutesOracleV1+InputBone]::new()
        $input.header_i32=$bone.header_i32;$input.header_i16=$bone.header_i16;$input.transform_bits=$bone.transform_bits
        $input
    })
    $inputParts=@(foreach($part in $geometrySample.geometry.parts){
        $group=@($packets[0].groups | Where-Object {$_.block_offset -eq 144 -and $_.group_ordinal -eq $part.group_ordinal})
        if($group.Count -ne 1){throw 'Ambiguous independent bind packet'}
        $input=[Kh2BindRoutesOracleV1+InputPart]::new()
        $input.vertex_bits=$group[0].packets[$part.packet_ordinal].vpu_data.vertex_bits
        $input.assignments=@(foreach($assignment in $part.assignments){
            ,([Kh2BindRoutesOracleV1+Binding[]]@(foreach($b in $assignment){$binding=[Kh2BindRoutesOracleV1+Binding]::new();$binding.matrix_index=$b.matrix_index;$binding.vertex_index=$b.vertex_index;$binding}))
        })
        $input
    })
    $result=[Kh2BindRoutesOracleV1]::Run([Kh2BindRoutesOracleV1+InputBone[]]$inputBones,[Kh2BindRoutesOracleV1+InputPart[]]$inputParts)
    $results+=@{sample=$geometrySample.sample;bar_ordinal=$geometrySample.bar_ordinal;comparison=$result}
}
$output=@{verdict='OBSERVATIONS_ONLY';pin=$pin;public_bind_methods_executed_unchanged=$true;skinning='owned adapter following Kkdf2 single/multi expressions with each competing bind matrix set';synthetic_controls='PASS';legacy_initial_matrix='identity';installed_route_selected=$false;runtime_verified=$false;source_check='independent selected original reconstruction; caller full before/after rescan required';samples=$results}
Write-NewResearchFile (Join-Path $OutputDirectory 'bind-route-comparison.json') ([Text.Encoding]::UTF8.GetBytes(($output | ConvertTo-Json -Depth 32)))
Write-Output "Observed competing public bind routes for $($results.Count) models; no installed transform profile selected."
