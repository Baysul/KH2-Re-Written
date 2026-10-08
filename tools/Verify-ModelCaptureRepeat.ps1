# Full logged input and independently decoded capture comparison for repeat tours.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$FirstLog,
    [Parameter(Mandatory)][string]$SecondLog,
    [Parameter(Mandatory)][string]$FirstFrameDirectory,
    [Parameter(Mandatory)][string]$SecondFrameDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$output=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
foreach($path in @($FirstFrameDirectory,$SecondFrameDirectory)){
    $null=Assert-PrivateResearchDirectory $path $InstallRoot
}
foreach($file in @($FirstLog,$SecondLog)){
    $null=Assert-PrivateResearchDirectory (Split-Path -Parent $file) $InstallRoot
    if((Get-Item $file).Length -gt 134217728){throw 'Repeat log exceeds finite bound'}
}
$errors=$null;$tokens=$null
$ast=[Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'Verify-ModelSkeleton.ps1'),[ref]$tokens,[ref]$errors)
if($errors.Count){throw 'Comparison helper syntax differs'}
$helper=$ast.Find({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Assert-ModelValue'},$true)
if(!$helper){throw 'Comparison helper absent'}
. ([scriptblock]::Create($helper.Extent.Text))
function Read-RepeatEvents([string]$Path){
    $events=@(Get-Content $Path|ForEach-Object{$_|ConvertFrom-Json})
    $admission=@($events|Where-Object event -eq 'admitted')
    $exit=@($events|Where-Object event -eq 'exit')
    $views=@($events|Where-Object event -eq 'view')
    $captures=@($events|Where-Object event -eq 'captured')
    $camera=@($events|Where-Object event -eq 'camera_drawing')
    if($admission.Count -ne 1 -or $exit.Count -ne 1 -or !$exit[0].complete -or !$admission[0].tour -or
        !$admission[0].fresh_full_scan -or $admission[0].gameplay_enabled -or
        $admission[0].qualification -ne 'quarantined_model_research_only' -or
        $admission[0].source_content_identity -notmatch '^[a-f0-9]{64}$' -or
        $views.Count -lt 1 -or $views.Count -gt 1024 -or $captures.Count -ne $views.Count -or
        $admission[0].expected_captures -ne $views.Count -or $exit[0].captures -ne $views.Count -or
        $camera.Count -ne $views.Count -or $admission[0].drawing_profile -notin @('bevy_default_drawing_v1','bevy_no_indirect_drawing_v1')){
        throw 'Unqualified or incomplete repeat tour'
    }
    $direct=$admission[0].drawing_profile -eq 'bevy_no_indirect_drawing_v1'
    for($i=0;$i -lt $views.Count;$i++){
        if($views[$i].step -ne $i -or $captures[$i].step -ne $i -or $camera[$i].step -ne $i -or
            $camera[$i].no_indirect_drawing -ne $direct -or $camera[$i].installed_drawing_mode_verified){
            throw 'Repeat state or actual camera drawing mode differs'
        }
    }
    @{admission=$admission[0];views=$views;captures=$captures}
}
Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Security.Cryptography;
public static class ModelRepeatFrames {
    public static byte[] Read(string path,string expectedHash) {
        var file=new FileInfo(path);
        if(file.Length!=3240012)throw new ArgumentException("complete 900x900 frame length");
        byte[] frame=File.ReadAllBytes(path);
        if(!frame.AsSpan(0,4).SequenceEqual(new byte[]{82,71,66,65}) ||
            BitConverter.ToUInt32(frame,4)!=900 || BitConverter.ToUInt32(frame,8)!=900 ||
            !String.Equals(Convert.ToHexString(SHA256.HashData(frame.AsSpan(12))),expectedHash,StringComparison.OrdinalIgnoreCase))
            throw new ArgumentException("independent frame/native readback identity differs");
        return frame;
    }
    public static object Compare(byte[] a,byte[] b) {
        long pixels=0,coverage=0,channels=0;
        for(int p=12;p<a.Length;p+=4) {
            bool different=false;
            for(int c=0;c<4;c++)if(a[p+c]!=b[p+c]){different=true;channels++;}
            if(different)pixels++;
            bool blackA=a[p]==0&&a[p+1]==0&&a[p+2]==0,blackB=b[p]==0&&b[p+1]==0&&b[p+2]==0;
            if(blackA!=blackB)coverage++;
        }
        return new{different_pixels=pixels,different_channels=channels,coverage_different_pixels=coverage};
    }
}
'@
$first=Read-RepeatEvents $FirstLog;$second=Read-RepeatEvents $SecondLog
if($first.admission.source_content_identity -cne $second.admission.source_content_identity -or
    $first.admission.drawing_profile -cne $second.admission.drawing_profile -or
    $first.views.Count -ne $second.views.Count){throw 'Repeat source/profile/denominator differs'}
$results=@();$different=0L;$coverage=0L
for($i=0;$i -lt $first.views.Count;$i++){
    Assert-ModelValue $first.views[$i] $second.views[$i] "complete repeat view $i"
    $frames=@()
    foreach($pair in @(@{events=$first;directory=$FirstFrameDirectory},@{events=$second;directory=$SecondFrameDirectory})){
        $capture=$pair.events.captures[$i]
        $name='model-{0:000}.png' -f $i
        if($capture.file -cne $name -or $capture.width -ne 900 -or $capture.height -ne 900){throw 'Repeat capture shape/name differs'}
        $path=Join-Path $pair.directory ($name -replace '\.png$','.rgba')
        $canonical=Get-ResearchCanonicalPath $path
        $null=Assert-PrivateResearchDirectory (Split-Path -Parent $canonical) $InstallRoot
        $frames+=,[ModelRepeatFrames]::Read($canonical,$capture.pixels_sha256)
    }
    $comparison=[ModelRepeatFrames]::Compare($frames[0],$frames[1])
    $different+=$comparison.different_pixels;$coverage+=$comparison.coverage_different_pixels
    $results+=@{step=$i;comparison=$comparison}
}
Write-NewResearchFile (Join-Path $output 'model-capture-repeat.json') ([Text.Encoding]::UTF8.GetBytes((@{
    input_verdict='PASS';capture_repeat_verdict=$(if($different){'FAIL'}else{'PASS'});
    source_content_identity=$first.admission.source_content_identity;
    first_log_sha256=(Get-FileHash -LiteralPath $FirstLog).Hash.ToLowerInvariant();
    second_log_sha256=(Get-FileHash -LiteralPath $SecondLog).Hash.ToLowerInvariant();
    drawing_profile=$first.admission.drawing_profile;states=$first.views.Count;
    compared_pixels=$first.views.Count*810000;different_pixels=$different;coverage_different_pixels=$coverage;
    full_independent_frame_readback_identity=$true;tolerance_selected=$false;installed_renderer_verified=$false;samples=$results
}|ConvertTo-Json -Depth 7)))
"Repeat inputs PASS; complete captures $(if($different){'FAIL'}else{'PASS'}): $different differing pixels, no tolerance."
