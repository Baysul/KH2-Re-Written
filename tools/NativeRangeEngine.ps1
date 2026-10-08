# Low-level isolated raw inspection, never source/gameplay admission.
# Callers separately qualify authored controls or current source/process evidence.
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
function Get-NativeRangeEngineIdentity([string]$ToolDirectory,[string]$JavaHome) {
    $package=Join-Path $ToolDirectory 'ghidra_12.1.4_PUBLIC_20260921.zip'
    $packageHash='ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db'
    if ((Get-FileHash $package).Hash.ToLowerInvariant() -cne $packageHash) {throw 'Unqualified Ghidra package'}
    $distribution=Get-ResearchCanonicalPath (Join-Path $ToolDirectory 'distribution/ghidra_12.1.4_PUBLIC')
    $archive=[IO.Compression.ZipFile]::OpenRead($package);$core=[ordered]@{}
    try {
        foreach ($relative in @('Ghidra/application.properties','Ghidra/Framework/Utility/lib/Utility.jar',
            'Ghidra/Framework/SoftwareModeling/lib/SoftwareModeling.jar','Ghidra/Features/Base/lib/Base.jar',
            'Ghidra/Features/Decompiler/lib/Decompiler.jar','Ghidra/Features/Decompiler/os/win_x86_64/decompile.exe',
            'Ghidra/Processors/x86/data/languages/x86-64.sla','Ghidra/Processors/x86/data/languages/x86-64-win.cspec')) {
            $entry=$archive.GetEntry('ghidra_12.1.4_PUBLIC/'+$relative)
            if (!$entry) {throw 'Required engine component absent'}
            $stream=$entry.Open()
            try {$hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream)).ToLowerInvariant()} finally {$stream.Dispose()}
            if ((Get-FileHash (Join-Path $distribution $relative)).Hash.ToLowerInvariant() -cne $hash) {throw 'Installed engine component differs'}
            $core[$relative]=$hash
        }
    } finally {$archive.Dispose()}
    $java=Get-ResearchCanonicalPath (Join-Path $JavaHome 'bin/java.exe')
    if (!(Test-Path (Join-Path $JavaHome 'bin/javac.exe'))) {throw 'Full JDK required'}
    return [ordered]@{package_sha256=$packageHash;core_components=$core;java_sha256=(Get-FileHash $java).Hash;
        script_sha256=(Get-FileHash (Join-Path $PSScriptRoot 'NativeMemorySelected.java')).Hash;
        engine_helper_sha256=(Get-FileHash (Join-Path $PSScriptRoot 'NativeRangeEngine.ps1')).Hash;loader_profile='raw_x64_windows_isolated_range_v1'}
}
function Invoke-NativeRangeEngine([string]$ToolDirectory,[string]$JavaHome,[string]$InstallRoot,
    [string]$Snapshot,[long]$Address,[string]$OutputDirectory) {
    $out=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
    if (@(Get-ChildItem $out -Force).Count) {throw 'Fresh empty raw engine directory required'}
    $input=Get-ResearchCanonicalPath $Snapshot
    [void](Assert-PrivateResearchDirectory (Split-Path $input -Parent) $InstallRoot)
    $length=(Get-Item $input).Length
    if ($length -lt 1 -or $length -gt 4096 -or $Address -le 0 -or $Address -gt [long]::MaxValue-$length) {throw 'Invalid raw range bounds'}
    $identity=Get-NativeRangeEngineIdentity $ToolDirectory $JavaHome
    $hash=(Get-FileHash $input).Hash.ToLowerInvariant()
    Write-NewResearchFile ($out+'/selection.json') ([Text.Encoding]::UTF8.GetBytes((@{address=$Address;snapshot_sha256=$hash}|ConvertTo-Json)))
    foreach ($name in @('home','temp','settings','cache','projects')) {[void](New-Item -ItemType Directory (Join-Path $out $name))}
    $source=Get-ResearchCanonicalPath $InstallRoot
    $root=Get-ResearchCanonicalPath ($env:LOCALAPPDATA+'/KH2FinalRewrite/research')
    $distribution=Get-ResearchCanonicalPath (Join-Path $ToolDirectory 'distribution/ghidra_12.1.4_PUBLIC')
    $vm=@('-Djava.system.class.loader=ghidra.GhidraClassLoader','-Dfile.encoding=UTF8','-Duser.country=US','-Duser.language=en',
        '-Xshare:off','--enable-native-access=ALL-UNNAMED','-Xmx2G',"-Duser.home=$out/home","-Djava.io.tmpdir=$out/temp",
        "-Dapplication.settingsdir=$out/settings","-Dapplication.cachedir=$out/cache","-Dapplication.tempdir=$out/temp",
        "-Drewrite.research.root=$root","-Drewrite.source.root=$source",'-cp',($distribution+'/Ghidra/Framework/Utility/lib/Utility.jar'),
        'ghidra.Ghidra','ghidra.app.util.headless.AnalyzeHeadless',($out+'/projects'),'SelectedMemory',
        '-import',$input,'-loader','BinaryLoader','-loader-baseAddr',$Address.ToString('x'),'-loader-fileOffset','0','-loader-length',$length.ToString('x'),
        '-processor','x86:LE:64:default','-cspec','windows','-noanalysis','-readOnly','-scriptPath',$PSScriptRoot,
        '-postScript','NativeMemorySelected.java',($out+'/selection.json'),($out+'/report.json'),
        '-log',($out+'/headless.log'),'-scriptlog',($out+'/script.log'))
    & (Join-Path $JavaHome 'bin/java.exe') @vm *> ($out+'/console.log')
    if ($LASTEXITCODE -or !(Test-Path ($out+'/report.json'))) {throw 'Raw engine failed or returned no bound report'}
    if ((Get-FileHash $input).Hash.ToLowerInvariant() -cne $hash) {throw 'Snapshot changed during raw inspection'}
    $report=Get-Content ($out+'/report.json') -Raw|ConvertFrom-Json
    if ($report.loader_profile -cne $identity.loader_profile -or $report.ghidra_version -cne '12.1.4' -or
        $report.snapshot_sha256 -cne $hash -or $report.address -ne $Address -or $report.range_size -ne $length -or
        $report.mapped_blocks -ne 1 -or !$report.instructions.Count) {throw 'Raw report identity/profile/coverage differs'}
    $identity.verdict='PASS';$identity.snapshot_sha256=$hash;$identity.address=$Address;$identity.range_size=$length
    $identity.report_sha256=(Get-FileHash ($out+'/report.json')).Hash;$identity.java_version=$report.java_version
    $identity.qualification='isolated_raw_inspection_not_source_support';$identity.auto_analysis=$false
    Write-NewResearchFile ($out+'/engine-receipt.json') ([Text.Encoding]::UTF8.GetBytes(($identity|ConvertTo-Json -Depth 5)))
    return $identity
}
