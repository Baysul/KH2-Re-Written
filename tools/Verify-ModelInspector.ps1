# Independent mesh-input and complete screen comparison for the finite axis tour.
# Public pose/topology results are supplied separately from the viewer log.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$InspectorLog,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$BindReference,
    [Parameter(Mandatory)][string]$GeometryReference,
    [Parameter(Mandatory)][string]$CaptureDirectory,
    [Parameter(Mandatory)][string]$OraclePath,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$OutputDirectory=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
foreach($path in @($InspectorLog,$ProbePath,$BindReference,$GeometryReference,$OraclePath)){
    $null=Assert-PrivateResearchDirectory (Split-Path -Parent $path) $InstallRoot
}
$null=Assert-PrivateResearchDirectory $CaptureDirectory $InstallRoot
foreach($path in @($ProbePath,$BindReference,$GeometryReference)){if((Get-Item $path).Length -gt 33554432){throw 'Reference exceeds 32 MiB'}}
if((Get-Item $InspectorLog).Length -gt 67108864){throw 'Inspector log exceeds 64 MiB'}
Add-Type -TypeDefinition @'
using System;
public static class ModelInspectorReference {
    public static uint[][] Normalize(uint[][] source,double[] center,double radius) {
        if(source.Length>262144 || center.Length!=3 || !double.IsFinite(radius) || radius<=0) throw new ArgumentException("normalization bounds");
        var result=new uint[source.Length][];
        for(int i=0;i<source.Length;i++) {
            if(source[i].Length!=3) throw new ArgumentException("position width");
            result[i]=new uint[3];
            for(int a=0;a<3;a++) {
                float value=BitConverter.UInt32BitsToSingle(source[i][a]);
                if(!float.IsFinite(value) || !double.IsFinite(center[a])) throw new ArgumentException("nonfinite position/frame");
                result[i][a]=BitConverter.SingleToUInt32Bits((float)(((double)value-center[a])/radius));
            }
        }
        return result;
    }
    static double Edge(double ax,double ay,double bx,double by,double x,double y) {
        return (bx-ax)*(y-ay)-(by-ay)*(x-ax);
    }
    public static object Compare(uint[][][] parts,byte[][] colors,int axis,byte[] frame) {
        if(frame.Length<12 || frame[0]!=82 || frame[1]!=71 || frame[2]!=66 || frame[3]!=65) throw new ArgumentException("RGBA magic");
        int width=BitConverter.ToInt32(frame,4),height=BitConverter.ToInt32(frame,8);
        if(width!=900 || height!=900 || frame.Length!=12+width*height*4 || axis<0 || axis>2 || parts.Length!=colors.Length) throw new ArgumentException("finite tour dimensions/axis");
        byte[] expected=new byte[width*height*4];double[] depth=new double[width*height];
        Array.Fill(depth,double.NegativeInfinity);
        for(int i=3;i<expected.Length;i+=4)expected[i]=255;
        double scale=height/(double)2.6f;
        long triangles=0;
        for(int p=0;p<parts.Length;p++) {
            if(parts[p].Length%3!=0 || colors[p].Length!=3) throw new ArgumentException("triangle/color shape");
            var coords=new double[parts[p].Length][];
            for(int v=0;v<coords.Length;v++) {
                var words=parts[p][v];if(words.Length!=3)throw new ArgumentException("vertex shape");
                double x=BitConverter.UInt32BitsToSingle(words[0]),y=BitConverter.UInt32BitsToSingle(words[1]),z=BitConverter.UInt32BitsToSingle(words[2]);
                double sx=axis==2?-z:x,sy=axis==1?z:y,d=axis==0?z:axis==1?-y:x;
                coords[v]=new[]{width/2.0+sx*scale,height/2.0-sy*scale,d};
            }
            for(int t=0;t<coords.Length;t+=3) {
                var a=coords[t];var b=coords[t+1];var c=coords[t+2];triangles++;
                double area=Edge(a[0],a[1],b[0],b[1],c[0],c[1]);if(area==0)continue;
                int x0=Math.Max(0,(int)Math.Floor(Math.Min(a[0],Math.Min(b[0],c[0]))));
                int x1=Math.Min(width-1,(int)Math.Ceiling(Math.Max(a[0],Math.Max(b[0],c[0]))));
                int y0=Math.Max(0,(int)Math.Floor(Math.Min(a[1],Math.Min(b[1],c[1]))));
                int y1=Math.Min(height-1,(int)Math.Ceiling(Math.Max(a[1],Math.Max(b[1],c[1]))));
                for(int y=y0;y<=y1;y++)for(int x=x0;x<=x1;x++) {
                    double u=Edge(b[0],b[1],c[0],c[1],x+.5,y+.5)/area;
                    double v=Edge(c[0],c[1],a[0],a[1],x+.5,y+.5)/area;
                    double w=1-u-v;if(u<0 || v<0 || w<0)continue;
                    double d=u*a[2]+v*b[2]+w*c[2];int i=y*width+x;
                    if(d<=depth[i])continue;depth[i]=d;
                    for(int channel=0;channel<3;channel++)expected[i*4+channel]=colors[p][channel];
                }
            }
        }
        long different=0,foreground=0,expectedForeground=0,coverageDifferent=0;object first=null;
        for(int i=0;i<width*height;i++) {
            bool actualCoverage=frame[12+i*4]!=0 || frame[13+i*4]!=0 || frame[14+i*4]!=0;
            bool expectedCoverage=expected[i*4]!=0 || expected[i*4+1]!=0 || expected[i*4+2]!=0;
            if(actualCoverage)foreground++;if(expectedCoverage)expectedForeground++;
            if(actualCoverage!=expectedCoverage)coverageDifferent++;
            bool changed=false;
            for(int channel=0;channel<4;channel++)if(expected[i*4+channel]!=frame[12+i*4+channel]){
                changed=true;if(first==null)first=new{x=i%width,y=i/width,channel,expected=(int)expected[i*4+channel],observed=(int)frame[12+i*4+channel]};
            }
            if(changed)different++;
        }
        return new{pixels=width*height,triangles,foreground_pixels=foreground,expected_foreground_pixels=expectedForeground,
            different_pixels=different,coverage_different_pixels=coverageDifferent,first_difference=first,exact=different==0,
            raster_profile="owned_double_center_pixel_depth_candidate_v1",installed_renderer_verified=false};
    }
}
'@
function Assert-ExactWords($a,$b,[string]$Field){
    if(($a -join ',') -ne ($b -join ',')){throw "Mesh input disagreement: $Field"}
}
$probe=Get-Content $ProbePath -Raw|ConvertFrom-Json
$bind=Get-Content $BindReference -Raw|ConvertFrom-Json
$geometry=Get-Content $GeometryReference -Raw|ConvertFrom-Json
if($bind.verdict -ne 'OBSERVATIONS_ONLY' -or !$bind.public_bind_methods_executed_unchanged -or $bind.runtime_verified){throw 'Bind research reference qualification differs'}
if($geometry.verdict -ne 'PASS'){throw 'Geometry reference is unqualified'}
$events=@(Get-Content $InspectorLog|ForEach-Object{$_|ConvertFrom-Json})
$admission=@($events|Where-Object event -eq 'admitted');$exits=@($events|Where-Object event -eq 'exit')
if($admission.Count -ne 1 -or $admission[0].source_content_identity -ne $probe.source_content_identity -or
    !$admission[0].fresh_full_scan -or !$admission[0].tour -or $admission[0].gameplay_enabled -or
    $admission[0].qualification -ne 'quarantined_model_research_only' -or $admission[0].models -ne $bind.samples.Count -or
    $exits.Count -ne 1 -or !$exits[0].complete){throw 'Unbound/incomplete research tour'}
$expected=$bind.samples.Count*2*4
if($admission[0].expected_captures -ne $expected -or $exits[0].captures -ne $expected){throw 'Tour denominator differs'}
$seen=@{};$states=@{};$results=@();$view=$null;$positionWords=0;$cornerWords=0;$crossCorners=0
foreach($event in $events){
    if($event.event -eq 'view'){$view=$event;continue}
    if($event.event -ne 'captured'){continue}
    if(!$view -or $view.step -ne $event.step -or $view.zoom -ne 1 -or $event.file -notmatch '^model-[0-9]{3,5}\.png$' -or $seen.ContainsKey([string]$event.step)){throw 'Capture/view state differs'}
    $seen[[string]$event.step]=$true
    $sampleNumber=@(0..($probe.samples.Count-1)|Where-Object{$probe.samples[$_].index -eq $view.index -and $probe.samples[$_].ordinal -eq $view.ordinal})
    if($sampleNumber.Count -ne 1){throw 'Ambiguous model identity'}
    $model=@($bind.samples|Where-Object{$_.sample -eq $sampleNumber[0] -and $_.bar_ordinal -eq $view.bar_ordinal})
    $topology=@($geometry.samples|Where-Object{$_.sample -eq $sampleNumber[0] -and $_.bar_ordinal -eq $view.bar_ordinal})
    if($model.Count -ne 1 -or $topology.Count -ne 1){throw 'Model oracle identity differs'}
    $route=switch($view.pose_profile){'openkh_legacy_bind_fma_v1'{'legacy'};'openkh_modern_bind_fma_v1'{'modern'};default{throw 'Unknown pose profile'}}
    if($view.arithmetic_profile -ne 'dotnet10_fma_order_std_sincos_candidate_v1' -or $view.runtime_verified){throw 'Pose qualification differs'}
    $stateKey="$($view.index)|$($view.ordinal)|$($view.bar_ordinal)|$route|$([int]$event.step%4)"
    if($states.ContainsKey($stateKey)){throw 'Duplicate model/profile/axis tour state'};$states[$stateKey]=$true
    $positions=$model[0].comparison."${route}_position_bits"
    if($view.source_position_bits.Count -ne $positions.Count){throw 'Position count differs'}
    for($i=0;$i -lt $positions.Count;$i++){Assert-ExactWords $view.source_position_bits[$i] $positions[$i] "source position $i";$positionWords+=3}
    # Recompute common fit from both independent routes, never from log values.
    $low=@([double]::PositiveInfinity,[double]::PositiveInfinity,[double]::PositiveInfinity)
    $high=@([double]::NegativeInfinity,[double]::NegativeInfinity,[double]::NegativeInfinity)
    foreach($candidate in @($model[0].comparison.legacy_position_bits,$model[0].comparison.modern_position_bits)){
        foreach($position in $candidate){for($a=0;$a -lt 3;$a++){
            $v=[double][BitConverter]::UInt32BitsToSingle([uint32]$position[$a]);$low[$a]=[Math]::Min($low[$a],$v);$high[$a]=[Math]::Max($high[$a],$v)
        }}
    }
    $center=@();$radius=0.0
    for($a=0;$a -lt 3;$a++){$center+=($low[$a]+$high[$a])*0.5;$radius=[Math]::Max($radius,($high[$a]-$low[$a])*0.5)}
    if($radius -le 0){$radius=[double]::Parse('2.2250738585072014E-308',[Globalization.CultureInfo]::InvariantCulture)}
    if($view.radius -ne $radius){throw 'Common radius differs'}
    for($a=0;$a -lt 3;$a++){if($view.center[$a] -ne $center[$a]){throw 'Common center differs'}}
    $normalized=[ModelInspectorReference]::Normalize([uint32[][]]$positions,[double[]]$center,$radius)
    $expectedParts=@(0..($topology[0].geometry.parts.Count-1)|Where-Object{
        $topology[0].geometry.parts[$_].triangles.Count -gt 0 -and ($null -eq $view.part -or $_ -eq $view.part)})
    if($view.rendered.Count -ne $expectedParts.Count){throw 'Rendered part denominator differs'}
    $screenParts=@();$screenColors=@()
    for($j=0;$j -lt $expectedParts.Count;$j++){
        $p=$expectedParts[$j];$part=$topology[0].geometry.parts[$p];$render=$view.rendered[$j]
        $indices=@($part.triangles|ForEach-Object{$_}|ForEach-Object{$_.vertex_index})
        if($render.part -ne $p){throw 'Part ordinal differs'}
        Assert-ExactWords $render.source.indices $indices "part $p indices"
        foreach($field in @('group_ordinal','packet_ordinal','texture_index')){if($render.source.$field -ne $part.$field){throw "Part $field differs"}}
        if($render.source.source_is_opaque -ne $part.is_opaque){throw 'Source opacity metadata differs'}
        $cross=0;foreach($index in $indices){if($index -lt $part.vertex_base -or $index -ge $part.vertex_base+$part.assignments.Count){$cross++}}
        if($render.source.cross_part_corners -ne $cross){throw 'Cross-part references differ'};$crossCorners+=$cross
        $rgb=@(53,97,29|ForEach-Object{64+$p*$_%128});Assert-ExactWords $render.rgb $rgb "part $p color"
        if($render.mesh_position_bits.Count -ne $indices.Count){throw 'Mesh corner count differs'}
        $corners=@();for($i=0;$i -lt $indices.Count;$i++){
            $words=$normalized[$indices[$i]];Assert-ExactWords $render.mesh_position_bits[$i] $words "part $p mesh corner $i"
            $corners+=,@($words);$cornerWords+=3
        }
        $screenParts+=,[uint32[][]]$corners;$screenColors+=,[byte[]]$rgb
    }
    $axis=[int]$event.step%4;if($axis -eq 3){$axis=0}
    $directions=@(@(0,0,1),@(0,-1,0),@(1,0,0));$ups=@(@(0,1,0),@(0,0,1),@(0,1,0))
    Assert-ExactWords $view.direction $directions[$axis] 'camera direction';Assert-ExactWords $view.up $ups[$axis] 'camera up'
    $framePath=Join-Path $OutputDirectory ($event.file -replace '\.png$','.rgba')
    & $OraclePath (Join-Path $CaptureDirectory $event.file) $framePath
    if($LASTEXITCODE){throw 'WIC capture decode failed'}
    if((Get-Item $framePath).Length -gt 67108876){throw 'Frame bounds exceeded'}
    $bytes=[IO.File]::ReadAllBytes($framePath)
    $stream=[IO.MemoryStream]::new($bytes,12,$bytes.Length-12,$false)
    try{$hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)).ToLowerInvariant()}finally{$stream.Dispose()}
    if($hash -ne $event.pixels_sha256 -or $event.width -ne 900 -or $event.height -ne 900){throw 'GPU readback/PNG identity differs'}
    $comparison=[ModelInspectorReference]::Compare([uint32[][][]]$screenParts,[byte[][]]$screenColors,$axis,$bytes)
    $results+=@{step=$event.step;item=$view.item;profile=$view.pose_profile;part=$view.part;file=$event.file;comparison=$comparison}
}
if($seen.Count -ne $expected){throw 'Capture count differs'}
$different=($results|ForEach-Object{$_.comparison.different_pixels}|Measure-Object -Sum).Sum
$coverageDifferent=($results|ForEach-Object{$_.comparison.coverage_different_pixels}|Measure-Object -Sum).Sum
$output=@{mesh_inputs_verdict='PASS';source_position_words=$positionWords;mesh_corner_words=$cornerWords;
    cross_part_corners=$crossCorners;states=$seen.Count;screen_verdict=$(if($different -eq 0){'PASS'}else{'FAIL'});
    compared_screen_pixels=$seen.Count*900*900;different_screen_pixels=$different;coverage_different_screen_pixels=$coverageDifferent;
    tolerance_selected=$false;installed_renderer_verified=$false;samples=$results}
Write-NewResearchFile (Join-Path $OutputDirectory 'model-inspector-comparison.json') ([Text.Encoding]::UTF8.GetBytes(($output|ConvertTo-Json -Depth 10)))
Write-Output "Mesh inputs PASS; screen $($output.screen_verdict), $different differing pixels, no tolerance."
