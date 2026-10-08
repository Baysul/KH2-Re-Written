# Private, execution-free native research; does not establish source support.
param(
    [Parameter(Mandatory)][string]$ToolDirectory,
    [Parameter(Mandatory)][string]$JavaHome,
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$InputFile,
    [Parameter(Mandatory)][string]$SelectionPath,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$ManagedInventory,
    [string]$ConformanceReceipt,
    [string]$ParentNativeDirectory,
    [string]$SourceManifest,
    [string]$RuntimeModuleInventory,
    [switch]$PreservePrivateProject
)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
. (Join-Path $PSScriptRoot 'NativeReader.ps1')
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
    $inputImageBase=$inputPe.PEHeaders.PEHeader.ImageBase
    $inputSections=$inputPe.PEHeaders.SectionHeaders
    $inputExports=$inputPe.PEHeaders.PEHeader.ExportTableDirectory
    $inputHasClr=$null -ne $inputPe.PEHeaders.CorHeader
} finally {$inputPe.Dispose();$inputMemory.Dispose()}
$sourceRoot=Get-ResearchCanonicalPath $InstallRoot
$privateRoot=Get-ResearchCanonicalPath (Join-Path $env:LOCALAPPDATA 'KH2FinalRewrite/research')
if ($inputPath.StartsWith($sourceRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {
    $expected=Get-ResearchCanonicalPath (Join-Path $sourceRoot 'KINGDOM HEARTS II FINAL MIX.exe')
    $shared=![StringComparer]::OrdinalIgnoreCase.Equals($inputPath,$expected)
    if (!$ConformanceReceipt -or ($shared -and (!$SourceManifest -or !$RuntimeModuleInventory -or $ManagedInventory -or $ParentNativeDirectory)) -or
        (!$shared -and (!$ManagedInventory -or $SourceManifest -or $RuntimeModuleInventory))) {
        throw 'Steam research requires the source-bound KH2 or observed-shared-module prerequisites and native oracle'
    }
    $prerequisites=@($ConformanceReceipt)
    if ($shared) {$prerequisites += @($SourceManifest,$RuntimeModuleInventory)} else {$prerequisites += $ManagedInventory}
    foreach ($path in $prerequisites) {
        if ((Get-Item -LiteralPath $path).Length -gt 1048576) {throw 'Native prerequisite JSON exceeds 1 MiB'}
    }
    if ($shared) {
        foreach ($path in @($SourceManifest,$RuntimeModuleInventory)) {
            [void](Assert-PrivateResearchDirectory (Split-Path -Parent (Get-ResearchCanonicalPath $path)) $InstallRoot)
        }
        $manifest=Get-Content -LiteralPath $SourceManifest -Raw | ConvertFrom-Json
        $runtime=Get-Content -LiteralPath $RuntimeModuleInventory -Raw | ConvertFrom-Json
        $sharedIdentity=Assert-NativeSharedModuleEvidence $manifest $runtime $sourceRoot ([IO.Path]::GetRelativePath($sourceRoot,$inputPath)) `
            $inputBytes.LongLength $inputHash $selection.source_content_identity
        if ($inputHasClr -or $selection.methods.Count -lt 1 -or $selection.methods.Count -gt 8) {throw 'Shared native export selection/profile differs'}
        foreach ($method in $selection.methods) {
            if ($method.kind -cne 'pe_export') {throw 'Shared module admits named PE exports only'}
            $lead=Get-NativeNamedExport $inputBytes $inputSections $inputExports $method.export_name
            if ($lead.rva -ne $method.rva -or $lead.ordinal -ne $method.export_ordinal -or
                (Get-NativePeOffset $inputSections $lead.rva $method.prefix_size $inputBytes.LongLength) -ne $method.file_offset) {
                throw 'Shared native lead differs from current named export'
            }
        }
    } else {$inventory=Get-Content -LiteralPath $ManagedInventory -Raw | ConvertFrom-Json}
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
        $oracleLauncher.loader_profile -ne 'native_pe_no_cli_no_ordinal_lookup_v1' -or
        $oracleLauncher.java_sha256 -ne (Get-FileHash (Join-Path $JavaHome 'bin/java.exe')).Hash) {
        throw 'Native oracle tool/host identity differs'
    }
    if (!$shared) {
        if ($inventory.qualification -ne 'quarantined_static_managed_research_only' -or
            $inventory.executable_sha256 -cne $inputHash -or $selection.source_content_identity -cnotmatch '^[0-9a-f]{64}$' -or
            $selection.source_content_identity -cne $inventory.source_content_identity) {throw 'Native inventory/selection/source binding differs'}
        if (@($selection.methods | Where-Object {$_.kind -in @('direct_branch','static_pointer')}).Count) {
            if (!$ParentNativeDirectory) {throw 'Source-derived selection requires a qualified private parent run'}
            $parent=Assert-PrivateResearchDirectory $ParentNativeDirectory $InstallRoot
            $parentReportPath=Join-Path $parent 'report.json'
            $parentReceiptPath=Join-Path $parent 'launcher-receipt.json'
            if ((Get-Item $parentReportPath).Length -gt 8388608 -or (Get-Item $parentReceiptPath).Length -gt 1048576) {
                throw 'Parent native evidence exceeds JSON bounds'
            }
            $parentReceipt=Get-Content $parentReceiptPath -Raw | ConvertFrom-Json
            $parentReport=Get-Content $parentReportPath -Raw | ConvertFrom-Json
            $parentReportHash=(Get-FileHash $parentReportPath).Hash.ToLowerInvariant()
            if ($parentReceipt.verdict -ne 'PASS' -or $parentReceipt.qualification -ne 'static_native_reader_only' -or
                $parentReceipt.input_sha256 -cne $inputHash -or $parentReceipt.report_sha256 -ine $parentReportHash -or
                $parentReceipt.script_sha256 -ine (Get-FileHash (Join-Path $PSScriptRoot 'NativeSelected.java')).Hash -or
                $parentReceipt.package_sha256 -ne $oracleLauncher.package_sha256 -or
                $parentReceipt.java_sha256 -ine $oracleLauncher.java_sha256 -or
                $parentReceipt.loader_profile -ne $oracleLauncher.loader_profile -or
                $parentReport.qualification -ne 'quarantined_static_native_research_only' -or
                $parentReport.executable_sha256 -cne $inputHash -or $parentReport.ghidra_version -ne '12.1.4' -or
                $parentReport.image_base -cne $expectedImageBase -or $parentReport.language -ne 'x86:LE:64:default' -or
                $parentReport.compiler_spec -ne 'windows') {throw 'Parent native evidence identity/profile differs'}
            foreach ($component in $coreHashes.Keys) {
                if ($parentReceipt.core_components.$component -cne $coreHashes[$component]) {throw 'Parent native engine components differ'}
            }
        }
        foreach ($method in $selection.methods) {
            if (!$method.kind -or $method.kind -eq 'methoddef') {
                $rows=@($inventory.methods | Where-Object {$_.token -ceq $method.token})
                if ($rows.Count -ne 1 -or $rows[0].implementation_kind -ne 'Native' -or $rows[0].rva -ne $method.rva -or
                    $rows[0].method_file_offset -ne $method.file_offset) {throw 'Selected native lead differs from bound MethodDef'}
            } elseif ($method.kind -in @('direct_branch','static_pointer')) {
                if ($method.parent_report_sha256 -cne $parentReportHash) {throw 'Selected branch parent report differs'}
                $findings=@($parentReport.findings | Where-Object rva -eq $method.parent_function_rva)
                $instructions=@($findings.instructions | Where-Object rva -eq $method.instruction_rva)
                $instructionSize=if ($method.kind -eq 'direct_branch') {5} else {6}
                if ($findings.Count -ne 1 -or $instructions.Count -ne 1 -or $instructions[0].size -ne $instructionSize) {
                    throw 'Selected branch instruction is not uniquely recorded in parent body'
                }
                $instruction=$instructions[0]
                $offset=Get-NativePeOffset $inputSections $instruction.rva $instructionSize $inputBytes.LongLength
                $bytes=[byte[]]$inputBytes[$offset..($offset+$instructionSize-1)]
                if ($offset -ne $instruction.file_offset -or
                    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant() -cne $instruction.sha256) {
                    throw 'Parent branch instruction differs from physical source'
                }
                if ($method.kind -eq 'direct_branch') {
                    $target=Get-NativeRelativeBranchTarget $inputBytes $instruction.rva $offset
                } else {
                    $pointer=Get-NativeStaticPointerTarget $inputBytes $inputSections $inputImageBase $instruction.rva $offset
                    if ($method.slot_rva -ne $pointer.slot_rva -or $method.slot_file_offset -ne $pointer.slot_file_offset -or
                        $method.slot_sha256 -cne $pointer.slot_sha256) {throw 'Selected static pointer slot differs from source'}
                    $target=$pointer.static_target_rva
                }
                if ($target -ne $method.rva -or (Get-NativePeOffset $inputSections $target $method.prefix_size $inputBytes.LongLength) -ne $method.file_offset) {
                    throw 'Selected branch target or physical mapping differs'
                }
            } else {throw 'Unknown native selection kind'}
        }
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
    '-import',$inputPath,'-loader','PeLoader','-loader-parseCliHeaders','false','-loader-ordinalLookup','false','-loader-loadLibraries','false',
    '-loader-linkExistingProjectLibraries','false','-processor','x86:LE:64:default','-cspec','windows',
    '-noanalysis','-scriptPath',$PSScriptRoot,'-postScript','NativeSelected.java',
    (Get-ResearchCanonicalPath $SelectionPath),(Join-Path $output 'report.json'),
    '-log',(Join-Path $output 'headless.log'),'-scriptlog',(Join-Path $output 'script.log'))
# A retained project contains derived source bytes and uses the same private
# output/source guards. The default remains a disposable read-only project.
if (!$PreservePrivateProject) {$vmargs += '-readOnly'}
# Direct launch matches the official launcher bootstrap and keeps settings/temp
# private without modifying JAVA_HOME, global Ghidra configuration or plugins.
& $java @vmargs *> (Join-Path $output 'console.log')
if ($LASTEXITCODE -ne 0) {throw 'Headless native engine exited unsuccessfully; inspect private console log'}
if ((Get-FileHash -LiteralPath $inputPath).Hash.ToLowerInvariant() -cne $inputHash) {throw 'Input changed during native analysis'}
if (!(Test-Path -LiteralPath (Join-Path $output 'report.json'))) {throw 'No native report; headless script errors can exit zero. Inspect private console log'}
$report=Get-Content -LiteralPath (Join-Path $output 'report.json') -Raw | ConvertFrom-Json
if ($report.executable_sha256 -cne $inputHash -or $report.ghidra_version -ne '12.1.4' -or $report.image_base -ne $expectedImageBase -or
    $report.findings.Count -ne $selection.methods.Count) {throw 'Native report identity/version/coverage differs'}
if ($PreservePrivateProject -and (!(Test-Path -LiteralPath (Join-Path $output 'projects/SelectedNative.gpr')) -or
    !(Test-Path -LiteralPath (Join-Path $output 'projects/SelectedNative.rep') -PathType Container))) {
    throw 'Requested private project was not saved'
}
$receipt=[ordered]@{verdict='PASS';qualification='static_native_reader_only';input_sha256=$inputHash;
    package_sha256='ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db';
    core_components=$coreHashes;script_sha256=(Get-FileHash -LiteralPath (Join-Path $PSScriptRoot 'NativeSelected.java')).Hash;
    java_sha256=(Get-FileHash -LiteralPath $java).Hash;java_version=$report.java_version;
    selection_sha256=(Get-FileHash -LiteralPath $SelectionPath).Hash;
    report_sha256=(Get-FileHash -LiteralPath (Join-Path $output 'report.json')).Hash;
    private_project_preserved=[bool]$PreservePrivateProject;
    loader_profile='native_pe_no_cli_no_ordinal_lookup_v1';
    parent_report_sha256=if ($ParentNativeDirectory) {$parentReportHash} else {$null};
    branch_helper_sha256=(Get-FileHash (Join-Path $PSScriptRoot 'NativeReader.ps1')).Hash;
    static_pointer_leads=@($selection.methods | Where-Object kind -eq 'static_pointer').Count;
    shared_export_leads=@($selection.methods | Where-Object kind -eq 'pe_export').Count;
    shared_source_content_identity=$sharedIdentity;
    source_manifest_sha256=if ($shared) {(Get-FileHash $SourceManifest).Hash} else {$null};
    runtime_inventory_sha256=if ($shared) {(Get-FileHash $RuntimeModuleInventory).Hash} else {$null};
    limitations='Native PE import with CLR parsing disabled; no auto-analysis, target execution, fresh full source scan or source-support qualification'}
Write-NewResearchFile (Join-Path $output 'launcher-receipt.json') ([Text.Encoding]::UTF8.GetBytes(($receipt | ConvertTo-Json -Depth 5)))
Write-Output "PASS: $($report.findings.Count) selected static native findings; installed input unchanged."
