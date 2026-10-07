# Private, execution-free native research; does not establish source support.
param(
    [Parameter(Mandatory)][string]$ToolDirectory,
    [Parameter(Mandatory)][string]$JavaHome,
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$InputFile,
    [Parameter(Mandatory)][string]$SelectionPath,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$ManagedInventory,
    [string]$ConformanceReceipt
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
$output=Assert-PrivateResearchDirectory $OutputDirectory $InstallRoot
$inputPath=Get-ResearchCanonicalPath $InputFile
$inputItem=Get-Item -LiteralPath $inputPath
if ($inputItem.PSIsContainer -or $inputItem.Length -gt 134217728) {throw 'Input must be a file <=128 MiB'}
for ($item=$inputItem; $null -ne $item; $item=if ($item.PSIsContainer) {$item.Parent} else {$item.Directory}) {
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {throw 'Native input may not traverse links'}
}
$package=Join-Path $ToolDirectory 'ghidra_12.1.4_PUBLIC_20260921.zip'
if ((Get-FileHash -LiteralPath $package).Hash -ne 'DDAC49F903DA9D5BAC833E5CC79395098B9C33CFD3279BE5F31BD00387D2D4DB') {
    throw 'Ghidra package differs from published release digest'
}
$distribution=Get-ResearchCanonicalPath (Join-Path $ToolDirectory 'distribution/ghidra_12.1.4_PUBLIC')
$archive=[IO.Compression.ZipFile]::OpenRead($package)
$coreHashes=[ordered]@{}
try {
    foreach ($relative in @('Ghidra/application.properties','Ghidra/Framework/Utility/lib/Utility.jar',
        'Ghidra/Framework/SoftwareModeling/lib/SoftwareModeling.jar','Ghidra/Features/Base/lib/Base.jar',
        'Ghidra/Features/Decompiler/lib/Decompiler.jar','Ghidra/Features/Decompiler/os/win_x86_64/decompile.exe',
        'Ghidra/Processors/x86/data/languages/x86-64.sla','Ghidra/Processors/x86/data/languages/x86-64-win.cspec')) {
        $entry=$archive.GetEntry("ghidra_12.1.4_PUBLIC/$relative")
        if (!$entry) {throw 'Required native engine component absent from archive'}
        $stream=$entry.Open()
        try {$hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($stream))} finally {$stream.Dispose()}
        if ((Get-FileHash -LiteralPath (Join-Path $distribution $relative)).Hash -ne $hash) {throw 'Installed native component differs from pinned archive'}
        $coreHashes[$relative]=$hash.ToLowerInvariant()
    }
} finally {$archive.Dispose()}
foreach ($name in @('home','temp','settings','cache','projects','report.json','launcher-receipt.json','headless.log','script.log','console.log')) {
    if (Test-Path -LiteralPath (Join-Path $output $name)) {throw 'Native run output already exists'}
}
$selectionItem=Get-Item -LiteralPath $SelectionPath
if ($selectionItem.Length -gt 1048576) {throw 'Selection exceeds 1 MiB'}
$selection=Get-Content -LiteralPath $SelectionPath -Raw | ConvertFrom-Json
$inputHash=(Get-FileHash -LiteralPath $inputPath).Hash.ToLowerInvariant()
if ($selection.executable_sha256 -cne $inputHash) {throw 'Native selection/input identity differs'}
$inputBytes=[IO.File]::ReadAllBytes($inputPath)
if ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($inputBytes)).ToLowerInvariant() -cne $inputHash) {
    throw 'Native input changed before header validation'
}
$inputMemory=[IO.MemoryStream]::new($inputBytes,$false)
$inputPe=[Reflection.PortableExecutable.PEReader]::new($inputMemory)
try {
    if ($inputPe.PEHeaders.CoffHeader.Machine -ne [Reflection.PortableExecutable.Machine]::Amd64 -or
        $inputPe.PEHeaders.PEHeader.Magic -ne [Reflection.PortableExecutable.PEMagic]::PE32Plus) {throw 'Native reader requires observed x64 PE32+'}
    $expectedImageBase=$inputPe.PEHeaders.PEHeader.ImageBase.ToString('x')
} finally {$inputPe.Dispose();$inputMemory.Dispose()}
$sourceRoot=Get-ResearchCanonicalPath $InstallRoot
$privateRoot=Get-ResearchCanonicalPath (Join-Path $env:LOCALAPPDATA 'KH2FinalRewrite/research')
if ($inputPath.StartsWith($sourceRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {
    $expected=Get-ResearchCanonicalPath (Join-Path $sourceRoot 'KINGDOM HEARTS II FINAL MIX.exe')
    if (![StringComparer]::OrdinalIgnoreCase.Equals($inputPath,$expected) -or !$ManagedInventory -or !$ConformanceReceipt) {
        throw 'Steam research requires KH2FM only, source-bound managed inventory and native oracle'
    }
    foreach ($path in @($ManagedInventory,$ConformanceReceipt)) {
        if ((Get-Item -LiteralPath $path).Length -gt 1048576) {throw 'Native prerequisite JSON exceeds 1 MiB'}
    }
    $inventory=Get-Content -LiteralPath $ManagedInventory -Raw | ConvertFrom-Json
    $oracle=Get-Content -LiteralPath $ConformanceReceipt -Raw | ConvertFrom-Json
    if ($oracle.verdict -ne 'PASS' -or $oracle.cases.Count -ne 22 -or
        $oracle.fixture_source_sha256 -ne (Get-FileHash (Join-Path $PSScriptRoot 'fixtures/NativeConformance.c')).Hash -or
        $oracle.reader_script_sha256 -ne (Get-FileHash (Join-Path $PSScriptRoot 'NativeSelected.java')).Hash -or
        @($oracle.cases | Where-Object {$_.expected -ne $_.original -or $_.expected -ne $_.recompiled}).Count) {
        throw 'Native authored-fixture oracle is not qualified for the current reader'
    }
    $oracleDirectory=Split-Path -Parent $ConformanceReceipt
    $oracleReportPath=Join-Path $oracleDirectory 'reader/report.json'
    $oracleLauncherPath=Join-Path $oracleDirectory 'reader/launcher-receipt.json'
    if ((Get-FileHash $oracleReportPath).Hash -ne $oracle.reader_report_sha256) {throw 'Native oracle report changed'}
    $oracleLauncher=Get-Content $oracleLauncherPath -Raw | ConvertFrom-Json
    if ($oracleLauncher.verdict -ne 'PASS' -or $oracleLauncher.package_sha256 -ne
        'ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db' -or
        $oracleLauncher.java_sha256 -ne (Get-FileHash (Join-Path $JavaHome 'bin/java.exe')).Hash) {
        throw 'Native oracle tool/host identity differs'
    }
    if ($inventory.qualification -ne 'quarantined_static_managed_research_only' -or
        $inventory.executable_sha256 -cne $inputHash -or $selection.source_content_identity -cnotmatch '^[0-9a-f]{64}$' -or
        $selection.source_content_identity -cne $inventory.source_content_identity) {throw 'Native inventory/selection/source binding differs'}
    foreach ($method in $selection.methods) {
        $rows=@($inventory.methods | Where-Object {$_.token -ceq $method.token})
        if ($rows.Count -ne 1 -or $rows[0].implementation_kind -ne 'Native' -or $rows[0].rva -ne $method.rva -or
            $rows[0].method_file_offset -ne $method.file_offset) {throw 'Selected native lead differs from bound MethodDef'}
    }
} elseif (!$inputPath.StartsWith($privateRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {
    throw 'Native input must be selected KH2FM or a private authored fixture'
}
foreach ($dir in @('home','temp','settings','cache','projects')) {[void](New-Item -ItemType Directory -Path (Join-Path $output $dir))}
$java=Join-Path $JavaHome 'bin/java.exe'
$javac=Join-Path $JavaHome 'bin/javac.exe'
if (!(Test-Path -LiteralPath $javac)) {throw 'Full JDK required'}
$vmargs=@('-Djava.system.class.loader=ghidra.GhidraClassLoader','-Dfile.encoding=UTF8',
    '-Duser.country=US','-Duser.language=en','-Xshare:off','--enable-native-access=ALL-UNNAMED','-Xmx2G',
    "-Duser.home=$(Join-Path $output 'home')","-Djava.io.tmpdir=$(Join-Path $output 'temp')",
    "-Dapplication.settingsdir=$(Join-Path $output 'settings')","-Dapplication.cachedir=$(Join-Path $output 'cache')",
    "-Dapplication.tempdir=$(Join-Path $output 'temp')",
    "-Drewrite.research.root=$privateRoot","-Drewrite.source.root=$sourceRoot",
    "-Drewrite.input.imagebase=$expectedImageBase",
    '-cp',(Join-Path $distribution 'Ghidra/Framework/Utility/lib/Utility.jar'),'ghidra.Ghidra',
    'ghidra.app.util.headless.AnalyzeHeadless',(Join-Path $output 'projects'),'SelectedNative',
    '-import',$inputPath,'-loader','PeLoader','-loader-parseCliHeaders','false','-loader-loadLibraries','false',
    '-loader-linkExistingProjectLibraries','false','-processor','x86:LE:64:default','-cspec','windows',
    '-noanalysis','-readOnly','-scriptPath',$PSScriptRoot,'-postScript','NativeSelected.java',
    (Get-ResearchCanonicalPath $SelectionPath),(Join-Path $output 'report.json'),
    '-log',(Join-Path $output 'headless.log'),'-scriptlog',(Join-Path $output 'script.log'))
# Direct launch matches the official launcher bootstrap and keeps settings/temp
# private without modifying JAVA_HOME, global Ghidra configuration or plugins.
& $java @vmargs *> (Join-Path $output 'console.log')
if ($LASTEXITCODE -ne 0) {throw 'Headless native engine exited unsuccessfully; inspect private console log'}
if ((Get-FileHash -LiteralPath $inputPath).Hash.ToLowerInvariant() -cne $inputHash) {throw 'Input changed during native analysis'}
if (!(Test-Path -LiteralPath (Join-Path $output 'report.json'))) {throw 'No native report; headless script errors can exit zero. Inspect private console log'}
$report=Get-Content -LiteralPath (Join-Path $output 'report.json') -Raw | ConvertFrom-Json
if ($report.executable_sha256 -cne $inputHash -or $report.ghidra_version -ne '12.1.4' -or $report.image_base -ne $expectedImageBase -or
    $report.findings.Count -ne $selection.methods.Count) {throw 'Native report identity/version/coverage differs'}
$receipt=[ordered]@{verdict='PASS';qualification='static_native_reader_only';input_sha256=$inputHash;
    package_sha256='ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db';
    core_components=$coreHashes;script_sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'NativeSelected.java')).Hash;
    java_sha256=(Get-FileHash -LiteralPath $java).Hash;java_version=$report.java_version;
    selection_sha256=(Get-FileHash -LiteralPath $SelectionPath).Hash;
    report_sha256=(Get-FileHash -LiteralPath (Join-Path $output 'report.json')).Hash;
    limitations='Native PE import with CLR parsing disabled; no auto-analysis, target execution, fresh full source scan or source-support qualification'}
Write-NewResearchFile (Join-Path $output 'launcher-receipt.json') ([Text.Encoding]::UTF8.GetBytes(($receipt | ConvertTo-Json -Depth 5)))
Write-Output "PASS: $($report.findings.Count) selected static native findings; installed input unchanged."
