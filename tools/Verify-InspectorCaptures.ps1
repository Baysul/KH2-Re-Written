# Independent visible-region comparison: WIC screen pixels against the
# previously source-traced RGBA corpus. Only native 1:1 centered views admitted.
param(
    [Parameter(Mandatory)][string]$InspectorLog,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$SourceImagesDirectory,
    [Parameter(Mandatory)][string]$CaptureDirectory,
    [Parameter(Mandatory)][string]$OraclePath,
    [Parameter(Mandatory)][string]$OracleDirectory,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$InstallRoot
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
foreach($directory in @((Split-Path -Parent $InspectorLog),(Split-Path -Parent $ProbePath),
    $SourceImagesDirectory,$CaptureDirectory,(Split-Path -Parent $OraclePath),$OracleDirectory,(Split-Path -Parent $OutputPath))) {
    $null=Assert-PrivateResearchDirectory -Directory $directory -SourceRoot $InstallRoot
}
if((Get-Item -LiteralPath $InspectorLog).Length -gt 10485760 -or (Get-Item -LiteralPath $ProbePath).Length -gt 33554432){throw 'Inspection metadata exceeds bounds'}
Add-Type -TypeDefinition @'
using System;
public static class InspectorRegionComparison {
    static int[] Size(byte[] b) {
        if(b.Length<12 || b.Length>67108876 || b[0]!=82 || b[1]!=71 || b[2]!=66 || b[3]!=65)
            throw new ArgumentException("RGBA bounds/magic");
        int w=BitConverter.ToInt32(b,4),h=BitConverter.ToInt32(b,8);
        if(w<=0 || h<=0 || w>4096 || h>4096 || (long)w*h*4+12!=b.Length) throw new ArgumentException("RGBA dimensions");
        return new[]{w,h};
    }
    public static object Compare(byte[] source,byte[] frame,string mode) {
        var s=Size(source);var f=Size(frame);
        if((f[0]-s[0])%2!=0 || (f[1]-s[1])%2!=0) throw new ArgumentException("Half-pixel placement is not admitted");
        int channel=mode=="red"?0:mode=="green"?1:mode=="blue"?2:mode=="alpha"?3:-1;
        if(channel<0 && mode!="stored_rgb" && mode!="composite") throw new ArgumentException("Unknown projection");
        long pixels=0,different=0;long[] channels=new long[4];int[] max=new int[4];object first=null;
        int ox=(f[0]-s[0])/2,oy=(f[1]-s[1])/2;
        for(int y=0;y<f[1];y++) for(int x=0;x<f[0];x++) {
            int sx=x-ox,sy=y-oy;
            if(sx<0 || sy<0 || sx>=s[0] || sy>=s[1]) continue;
            int si=12+(sy*s[0]+sx)*4,fi=12+(y*f[0]+x)*4;bool changed=false;pixels++;
            for(int c=0;c<4;c++) {
                int expected=c==3?255:channel>=0?source[si+channel]:source[si+c];
                if(c<3 && mode=="composite") {
                    int bg=((sx/8+sy/8)%2==0)?24:40,a=source[si+3];
                    expected=(source[si+c]*a+bg*(255-a)+127)/255;
                }
                int difference=Math.Abs(expected-frame[fi+c]);
                if(difference>0) {
                    channels[c]++;changed=true;
                    if(first==null) first=new{screen_x=x,screen_y=y,source_x=sx,source_y=sy,channel=c,expected,observed=(int)frame[fi+c]};
                }
                max[c]=Math.Max(max[c],difference);
            }
            if(changed) different++;
        }
        if(pixels==0) throw new ArgumentException("No visible source pixels");
        return new{source_width=s[0],source_height=s[1],frame_width=f[0],frame_height=f[1],
            visible_pixels=pixels,different_pixels=different,differing_channels_rgba=channels,
            max_absolute_difference_rgba=max,first_difference=first,exact=first==null};
    }
}
'@
$report=Get-Content -LiteralPath $ProbePath -Raw|ConvertFrom-Json
$events=@(Get-Content -LiteralPath $InspectorLog|ForEach-Object{$_|ConvertFrom-Json})
$admitted=@($events|Where-Object{$_.event -eq 'admitted'})
$exit=@($events|Where-Object{$_.event -eq 'exit'})
if($admitted.Count -ne 1 -or $admitted[0].source_content_identity -ne $report.source_content_identity -or
    -not $admitted[0].fresh_full_scan -or -not $admitted[0].tour -or $exit.Count -ne 1 -or -not $exit[0].complete){throw 'Incomplete/unbound inspection tour'}
$views=@{};$seen=@{};$results=@();$count=0
foreach($event in $events) {
    if($event.event -eq 'view'){$views[[string]$event.item]=$event;continue}
    if($event.event -ne 'captured'){continue}
    $view=$views[[string]$event.item]
    if(!$view -or $view.mode -ne $event.mode -or $view.fit -or $view.zoom -ne 1 -or
        $event.file -notmatch '^view-[0-9]{3,5}\.png$'){throw 'Capture/view identity, native scale or filename differs'}
    $key="$($view.index)|$($view.ordinal)|$($view.replacement_ordinal)|$($view.mode)"
    if($seen.ContainsKey($key)){throw 'Duplicate tour state'};$seen[$key]=$true
    $sample=@($report.samples|Where-Object{$_.index -eq $view.index -and $_.ordinal -eq $view.ordinal})
    if($sample.Count -ne 1){throw 'Source record is absent/ambiguous'}
    $replacement=@($sample[0].replacement_samples|Where-Object{$_.metadata.ordinal -eq $view.replacement_ordinal})
    if($replacement.Count -ne 1 -or $replacement[0].decoded_image.pixels_rgba_sha256 -ne $view.stored_pixels_sha256 -or
        $replacement[0].decoded_image.pixel_decode_profile -ne $view.pixel_decode_profile){throw 'Source pixel/profile identity differs'}
    $name=$replacement[0].decoded_image.preview_file
    if($name -notmatch '^sample-[0-9]{2}-replacement-[0-9]{2}\.bmp$'){throw 'Source pixel filename is not admitted'}
    $sourcePath=Join-Path $SourceImagesDirectory ($name -replace '\.bmp$','.rgba')
    if((Get-Item -LiteralPath $sourcePath).Length -gt 67108876){throw 'Source RGBA too large'}
    $source=[IO.File]::ReadAllBytes($sourcePath)
    $stream=[IO.MemoryStream]::new($source,12,$source.Length-12,$false)
    try{$hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)).ToLowerInvariant()}finally{$stream.Dispose()}
    if($hash -ne $view.stored_pixels_sha256){throw 'Retained source pixels changed'}
    $framePath=Join-Path $OracleDirectory ($event.file -replace '\.png$','.rgba')
    & $OraclePath (Join-Path $CaptureDirectory $event.file) $framePath
    if($LASTEXITCODE){throw 'Native screen PNG decoding failed'}
    if((Get-Item -LiteralPath $framePath).Length -gt 67108876){throw 'Frame RGBA too large'}
    $frame=[IO.File]::ReadAllBytes($framePath)
    $stream=[IO.MemoryStream]::new($frame,12,$frame.Length-12,$false)
    try{$frameHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)).ToLowerInvariant()}finally{$stream.Dispose()}
    if($frameHash -ne $event.pixels_sha256 -or [BitConverter]::ToUInt32($frame,4) -ne $event.width -or
        [BitConverter]::ToUInt32($frame,8) -ne $event.height){throw 'Encoded capture differs from logged GPU readback'}
    $comparison=[InspectorRegionComparison]::Compare($source,$frame,$view.mode)
    $results+=@{item=$event.item;mode=$view.mode;file=$event.file;comparison=$comparison}
    $count++
}
$expected=@($report.samples|ForEach-Object{$_.replacement_samples}|Where-Object{$_.decoded_image}).Count*6
if($count -ne $expected -or $admitted[0].expected_captures -ne $expected){throw 'Tour denominator differs from source report'}
Write-NewResearchFile -Path $OutputPath -Bytes ([Text.Encoding]::UTF8.GetBytes(($results|ConvertTo-Json -Depth 8)))
$failures=@($results|Where-Object{-not $_.comparison.exact}).Count
"Compared $count visible image/channel states; mismatches=$failures."
if($failures){throw 'Inspector rendered-region comparisons differ; no tolerance accepted'}
