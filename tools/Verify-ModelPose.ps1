# Full finite bit comparison, retaining all discrepancies without tolerance.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$OutputDirectory=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$poseDestination=$OutputDirectory
$poseProbe=$ProbePath
$bindDirectory=Join-Path $OutputDirectory 'bind'
New-Item -ItemType Directory $bindDirectory | Out-Null
. (Join-Path $PSScriptRoot 'Inspect-ModelBindRoutes.ps1') -InstallRoot $InstallRoot -ProbePath $ProbePath -OpenKhRoot $OpenKhRoot -OutputDirectory $bindDirectory | Out-Null
$OutputDirectory=$poseDestination
$ProbePath=$poseProbe
$poseReport=Get-Content $ProbePath -Raw | ConvertFrom-Json
$route=switch($poseReport.model_pose_profile){
    'openkh_legacy_bind_fma_v1' {'legacy'}
    'openkh_modern_bind_fma_v1' {'modern'}
    default {throw 'Explicit supported inspection pose profile required'}
}
$reference=Get-Content (Join-Path $bindDirectory 'bind-route-comparison.json') -Raw | ConvertFrom-Json
function Compare-PoseBits($Observed,$Expected,[int]$Width) {
    if($Observed.Count -ne $Expected.Count){throw 'Pose record counts differ'}
    $words=0;$records=0;$maximum=0.0;$first=$null
    for($i=0;$i -lt $Expected.Count;$i++){
        if($Observed[$i].Count -ne $Width -or $Expected[$i].Count -ne $Width){throw 'Pose row width differs'}
        $changed=$false
        for($j=0;$j -lt $Width;$j++){
            $a=[uint32]$Observed[$i][$j];$b=[uint32]$Expected[$i][$j]
            if(($a -band 2139095040) -eq 2139095040 -or ($b -band 2139095040) -eq 2139095040){throw 'Nonfinite pose evidence'}
            if($a -ne $b){
                $changed=$true;$words++
                $x=[BitConverter]::ToSingle([BitConverter]::GetBytes($a),0)
                $y=[BitConverter]::ToSingle([BitConverter]::GetBytes($b),0)
                $maximum=[Math]::Max($maximum,[Math]::Abs([double]$x-[double]$y))
                if($null -eq $first){$first=@{record=$i;word=$j;observed_bits=$a;expected_bits=$b}}
            }
        }
        if($changed){$records++}
    }
    @{records=$Expected.Count;words=$Expected.Count*$Width;records_different=$records;words_different=$words;maximum_absolute_difference=$maximum;first_difference=$first}
}
$results=@();$different=0
foreach($sample in $reference.samples){
    $models=@($poseReport.samples[$sample.sample].model_skeletons | Where-Object bar_ordinal -eq $sample.bar_ordinal)
    if($models.Count -ne 1){throw 'Ambiguous pose model'}
    $pose=$models[0].pose
    if($pose.profile -ne $poseReport.model_pose_profile -or $pose.arithmetic_profile -ne 'dotnet10_fma_order_std_sincos_candidate_v1' -or $pose.runtime_verified){throw 'Pose profile/runtime qualification differs'}
    $matrices=Compare-PoseBits $pose.matrix_bits $sample.comparison."${route}_matrix_bits" 16
    $positions=Compare-PoseBits $pose.position_bits $sample.comparison."${route}_position_bits" 3
    $different+=$matrices.words_different+$positions.words_different
    $results+=@{sample=$sample.sample;bar_ordinal=$sample.bar_ordinal;matrices=$matrices;positions=$positions}
}
$environment=@{framework=[Runtime.InteropServices.RuntimeInformation]::FrameworkDescription;runtime_informational_version=[System.Numerics.Matrix4x4].Assembly.GetCustomAttributes([Reflection.AssemblyInformationalVersionAttribute],$false)[0].InformationalVersion;vector_accelerated=[System.Numerics.Vector]::IsHardwareAccelerated;fma_supported=[System.Runtime.Intrinsics.X86.Fma]::IsSupported}
$output=@{verdict=$(if($different -eq 0){'PASS'}else{'FAIL'});profile=$poseReport.model_pose_profile;comparison='exact finite public-route output bits';tolerance_selected=$false;runtime_verified=$false;public_reference=$reference.pin;numeric_environment=$environment;words_different=$different;samples=$results}
Write-NewResearchFile (Join-Path $OutputDirectory 'pose-comparison.json') ([Text.Encoding]::UTF8.GetBytes(($output | ConvertTo-Json -Depth 16)))
Write-Output "$($output.verdict): pose profile $($output.profile); $different differing words; no tolerance selected."
