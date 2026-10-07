# Execution-free inspection of the bound KH2FM candidate only. Proprietary
# metadata/pseudocode stays in a new private output directory, outside Git/source.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ManifestPath,
    [Parameter(Mandatory)][string]$ToolDirectory,
    [Parameter(Mandatory)][string]$ConformanceReceipt,
    [Parameter(Mandatory)][string]$SelectionPath,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
function Read-BoundedJson([string]$Path) {
    $file = [IO.File]::OpenRead($Path)
    try {
        if ($file.Length -gt 1048576) { throw 'Research JSON exceeds 1 MiB' }
        $reader = [IO.StreamReader]::new($file)
        try { return $reader.ReadToEnd() | ConvertFrom-Json } finally {$reader.Dispose()}
    } finally {$file.Dispose()}
}
function Normalize-SourcePath([string]$Path) {
    return Get-ResearchCanonicalPath $Path
}
$manifest = Read-BoundedJson $ManifestPath
$root = Normalize-SourcePath $InstallRoot
if (![StringComparer]::OrdinalIgnoreCase.Equals($root,(Normalize-SourcePath $manifest.canonical_source_path)) -or
    $manifest.schema_version -ne 1 -or $manifest.qualification -ne 'quarantined_unqualified') {
    throw 'Manifest path/schema/qualification inconsistent'
}
$required = [Collections.Generic.List[string]]::new()
$required.Add('KINGDOM HEARTS II FINAL MIX.exe')
foreach ($pair in @('first','second','third','fourth','fifth','sixth')) {
    foreach ($extension in @('hed','pkg')) {$required.Add("Image/dt/kh2_$pair.$extension")}
}
$required.Sort([StringComparer]::Ordinal)
if ($manifest.files.Count -ne $required.Count) {throw 'Incomplete candidate manifest'}
$identityBytes = [IO.MemoryStream]::new()
$writer = [IO.BinaryWriter]::new($identityBytes)
try {
    $writer.Write([Text.Encoding]::UTF8.GetBytes("kh2-source-identity-v1`0"))
    foreach ($name in $required) {
        $rows = @($manifest.files | Where-Object {$_.relative_path -ceq $name})
        if ($rows.Count -ne 1 -or $rows[0].size -lt 0 -or $rows[0].sha256 -cnotmatch '^[0-9a-f]{64}$') {
            throw 'Missing/duplicate/malformed candidate identity'
        }
        $pathBytes=[Text.Encoding]::UTF8.GetBytes($name)
        $writer.Write([uint64]$pathBytes.Length);$writer.Write($pathBytes)
        $writer.Write([uint64]$rows[0].size);$writer.Write([Text.Encoding]::ASCII.GetBytes($rows[0].sha256))
    }
    $digest=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($identityBytes.ToArray())).ToLowerInvariant()
    if ($digest -cne $manifest.content_identity) {throw 'Candidate commitment mismatch'}
} finally {$writer.Dispose();$identityBytes.Dispose()}
$output = Assert-PrivateResearchDirectory $OutputDirectory $root
$tool = Initialize-ManagedReader $ToolDirectory
$conformance = Read-BoundedJson $ConformanceReceipt
if ($conformance.verdict -ne 'PASS' -or $conformance.tool.dll_sha256 -ne $tool.dll_sha256 -or
    $conformance.tool.package_sha256 -ne $tool.package_sha256 -or $conformance.cases.Count -ne 22 -or
    $conformance.source_sha256 -ne (Get-FileHash (Join-Path $PSScriptRoot 'fixtures/ManagedConformance.cs') -Algorithm SHA256).Hash -or
    @($conformance.cases | Where-Object {$_.expected -cne $_.actual}).Count -ne 0) {
    throw 'Current authored managed-reader oracle is not qualified'
}
$selection = Read-BoundedJson $SelectionPath
$expected = $manifest.files | Where-Object relative_path -ceq 'KINGDOM HEARTS II FINAL MIX.exe'
if ($selection.schema_version -ne 1 -or $selection.source_content_identity -cne $manifest.content_identity -or
    $selection.executable_sha256 -cne $expected.sha256 -or $selection.method_tokens.Count -lt 1 -or $selection.method_tokens.Count -gt 8) {
    throw 'Selection is not bound to this candidate executable'
}
$tokens = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($token in $selection.method_tokens) {
    if ($token -cnotmatch '^0x060[0-9a-f]{5}$' -or !$tokens.Add($token)) {throw 'Invalid/duplicate MethodDef selection'}
}
foreach ($name in @('inventory.json') + @($tokens | ForEach-Object {"$_.cs"})) {
    if (Test-Path -LiteralPath (Join-Path $output $name)) {throw 'Research output already exists'}
}
$exe = Get-Item -LiteralPath (Join-Path $root 'KINGDOM HEARTS II FINAL MIX.exe')
for ($item=$exe; $null -ne $item; $item=if ($item.PSIsContainer) {$item.Parent} else {$item.Directory}) {
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {throw 'Executable research path may not traverse links'}
}
if ($exe.Length -ne $expected.size -or $exe.Length -gt 134217728) {throw 'Executable size mismatch or >128 MiB'}
$snapshot=[IO.File]::ReadAllBytes($exe.FullName)
$snapshotHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($snapshot)).ToLowerInvariant()
if ($snapshotHash -cne $expected.sha256) {throw 'Executable differs from candidate commitment'}
$context=New-StaticDecompiler $snapshot (Join-Path $output 'input-not-written.exe')
$rows=[Collections.Generic.List[object]]::new()
$selected=[Collections.Generic.List[object]]::new()
$pendingFiles=[Collections.Generic.List[object]]::new()
$deadline=[Threading.CancellationTokenSource]::new(60000)
function Get-PhysicalMethodOffset([int]$Rva) {
    if ($Rva -eq 0) {return $null}
    $matches=@($context.module.Reader.PEHeaders.SectionHeaders | Where-Object {
        $Rva -ge $_.VirtualAddress -and [long]$Rva -lt [long]$_.VirtualAddress+$_.SizeOfRawData
    })
    if ($matches.Count -ne 1) {throw 'Method RVA has no unique physical PE section mapping'}
    $offset=[long]$matches[0].PointerToRawData+$Rva-$matches[0].VirtualAddress
    if ($offset -lt 0 -or $offset -ge $snapshot.Length) {throw 'Mapped method offset outside snapshot'}
    return $offset
}
try {
    $context.decompiler.CancellationToken=$deadline.Token
    $metadata=$context.module.Metadata
    foreach ($handle in $metadata.MethodDefinitions) {
        $method=$metadata.GetMethodDefinition($handle)
        $type=$metadata.GetTypeDefinition($method.GetDeclaringType())
        $token='0x{0:x8}' -f [Reflection.Metadata.Ecma335.MetadataTokens]::GetToken([Reflection.Metadata.EntityHandle]$handle)
        $kind=@('IL','Native','OPTIL','Runtime')[[int]$method.ImplAttributes -band 3]
        $row=[ordered]@{token=$token;name=$metadata.GetString($method.Name);declaring_type=$metadata.GetString($type.Name);
            declaring_namespace=$metadata.GetString($type.Namespace);implementation_flags=[int]$method.ImplAttributes;
            implementation_kind=$kind;rva=$method.RelativeVirtualAddress;
            method_file_offset=(Get-PhysicalMethodOffset $method.RelativeVirtualAddress);
            raw_cil_file_offset=$null;raw_cil_sha256=$null;raw_cil_size=$null}
        if ($kind -eq 'IL' -and $method.RelativeVirtualAddress -ne 0) {
            $body=[Reflection.Metadata.PEReaderExtensions]::GetMethodBody($context.module.Reader,$method.RelativeVirtualAddress).GetILBytes()
            if ($body.Length -gt 1048576) {throw 'CIL method exceeds 1 MiB inspection limit'}
            $row.raw_cil_size=$body.Length
            $row.raw_cil_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($body)).ToLowerInvariant()
            $start=$row.method_file_offset
            $headerSize=if (($snapshot[$start] -band 3) -eq 2) {1}
                elseif (($snapshot[$start] -band 3) -eq 3) {([BitConverter]::ToUInt16($snapshot,$start) -shr 12)*4}
                else {throw 'Invalid CIL header kind'}
            $row.raw_cil_file_offset=$start+$headerSize
            if ($headerSize -lt 1 -or $row.raw_cil_file_offset+$body.Length -gt $snapshot.Length) {throw 'CIL source span outside snapshot'}
            $span=[byte[]]::new($body.Length)
            [Array]::Copy($snapshot,$row.raw_cil_file_offset,$span,0,$body.Length)
            if ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($span)).ToLowerInvariant() -cne $row.raw_cil_sha256) {
                throw 'Mapped source span differs from metadata reader CIL'
            }
        }
        $rows.Add([pscustomobject]$row)
        if ($tokens.Contains($token)) {
            if ($kind -eq 'IL' -and $method.RelativeVirtualAddress -ne 0) {
                $text=$context.decompiler.DecompileAsString([Reflection.Metadata.EntityHandle[]]@([Reflection.Metadata.EntityHandle]$handle))
                $bytes=[Text.Encoding]::UTF8.GetBytes($text)
                if ($bytes.Length -gt 1048576) {throw 'Selected pseudocode exceeds 1 MiB'}
                $pendingFiles.Add([pscustomobject]@{path=(Join-Path $output "$token.cs");bytes=$bytes})
                $selected.Add([pscustomobject]@{token=$token;status='managed_pseudocode_only';sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()})
            } else {
                $selected.Add([pscustomobject]@{token=$token;status='declaration_only_native_or_body_unavailable';sha256=$null})
            }
        }
    }
    if ($selected.Count -ne $tokens.Count) {throw 'Selected MethodDef absent from bound metadata'}
    $cor=$context.module.Reader.PEHeaders.CorHeader
    $headers=[ordered]@{machine=$context.module.Reader.PEHeaders.CoffHeader.Machine.ToString();clr_flags=[int]$cor.Flags;
        clr_entry_token_or_rva=$cor.EntryPointTokenOrRelativeVirtualAddress;metadata_version=$metadata.MetadataVersion;
        type_definitions=$metadata.TypeDefinitions.Count;method_definitions=$metadata.MethodDefinitions.Count;
        assembly_references=$metadata.AssemblyReferences.Count}
} finally {$deadline.Dispose();$context.module.Dispose();$context.memory.Dispose()}
if ((Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash -ne $snapshotHash) {throw 'Executable changed during inspection'}
$result=[ordered]@{schema_version=1;qualification='quarantined_static_managed_research_only';source_content_identity=$manifest.content_identity;
    executable_sha256=$snapshotHash;tool=$tool;headers=$headers;methods=$rows;selected=$selected;
    limitations=@('Metadata and pseudocode, not original source or runtime behavior','Native method declarations are not native-body recovery',
        'Missing reference symbols tolerated; resolver searches host/private locations, not Steam siblings','Snapshot hash proves only selected executable; full source closure remains unqualified')}
foreach ($file in $pendingFiles) {Write-NewResearchFile $file.path $file.bytes}
Write-NewResearchFile (Join-Path $output 'inventory.json') ([Text.Encoding]::UTF8.GetBytes(($result | ConvertTo-Json -Depth 8)))
Write-Output "Inspected $($rows.Count) method declarations; $($pendingFiles.Count) selected private CIL pseudocode files. Target not executed."
