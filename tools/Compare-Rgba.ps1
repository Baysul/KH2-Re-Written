# Owned first-divergence diagnostics for private, source-traced image evidence.
param(
    [Parameter(Mandatory)][string]$LeftPath,
    [Parameter(Mandatory)][string]$RightPath,
    [Parameter(Mandatory)][string]$OutputPath,
    [Parameter(Mandatory)][string]$InstallRoot
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
foreach ($path in @($LeftPath,$RightPath,$OutputPath)) {
    $null=Assert-PrivateResearchDirectory -Directory (Split-Path -Parent $path) -SourceRoot $InstallRoot
}
foreach ($path in @($LeftPath,$RightPath)) {
    $length=(Get-Item -LiteralPath $path).Length
    if ($length -lt 12 -or $length -gt 67108876) {throw 'RGBA diagnostic file exceeds bounds'}
}
Add-Type -TypeDefinition @'
using System;
public static class ResearchRgbaDifference {
    public static object Compare(byte[] left,byte[] right) {
        if (left.Length!=right.Length) throw new ArgumentException("RGBA file lengths differ");
        for(int i=0;i<12;i++) if(left[i]!=right[i]) throw new ArgumentException("RGBA headers differ");
        if(left[0]!=82 || left[1]!=71 || left[2]!=66 || left[3]!=65) throw new ArgumentException("RGBA magic");
        uint width=BitConverter.ToUInt32(left,4),height=BitConverter.ToUInt32(left,8);
        if(width==0 || height==0 || width>4096 || height>4096 || (ulong)width*height*4+12!=(ulong)left.Length)
            throw new ArgumentException("RGBA dimensions/length");
        long[] channels=new long[4]; int[] maxima=new int[4];
        long differentPixels=0;int first=-1;
        for(int i=12;i<left.Length;i+=4) {
            bool changed=false;
            for(int c=0;c<4;c++) {
                int difference=Math.Abs((int)left[i+c]-right[i+c]);
                if(difference!=0) {channels[c]++;changed=true;if(first<0) first=i+c-12;}
                if(difference>maxima[c]) maxima[c]=difference;
            }
            if(changed) differentPixels++;
        }
        return new {width,height,pixels=(long)width*height,different_pixels=differentPixels,
            differing_channels_rgba=channels,max_absolute_difference_rgba=maxima,
            first_difference_byte=first,first_left=first<0 ? -1 : left[first+12],
            first_right=first<0 ? -1 : right[first+12],exact=first<0};
    }
}
'@
$result=[ResearchRgbaDifference]::Compare([IO.File]::ReadAllBytes($LeftPath),[IO.File]::ReadAllBytes($RightPath))
Write-NewResearchFile -Path $OutputPath -Bytes ([Text.Encoding]::UTF8.GetBytes(($result|ConvertTo-Json -Depth 4)))
"Compared $($result.pixels) pixels; different=$($result.different_pixels), exact=$($result.exact)."
