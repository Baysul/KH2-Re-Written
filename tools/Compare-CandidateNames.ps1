# Candidate-only metadata research, following pinned OpenKH EgsTools name generation.
# No extractor/writer is invoked and no dictionary is copied into the runtime.
param(
    [Parameter(Mandatory)][string]$ManifestPath,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [Parameter(Mandatory)][string]$OutputPath
)
$ErrorActionPreference = 'Stop'
$pin = '7a3b945c538d32c6a285128c98aefba093f52ceb'
if ((git -C $OpenKhRoot rev-parse HEAD) -ne $pin) { throw 'Unaudited OpenKH pin' }
$referenceFiles = @('OpenKh.Kh2/resources/kh2idx.txt','OpenKh.Kh2/Constants.cs','OpenKh.Egs/EgsTools.cs')
if (git -C $OpenKhRoot status --porcelain -- $referenceFiles) { throw 'Reference name sources modified' }
$manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
if ($manifest.schema_version -ne 1 -or $manifest.qualification -ne 'quarantined_unqualified') { throw 'Unexpected source manifest schema/qualification' }
$root = (Get-Item -LiteralPath $manifest.canonical_source_path).FullName
$output = [IO.Path]::GetFullPath($OutputPath)
$parent = Get-Item -LiteralPath ([IO.Path]::GetDirectoryName($output))
if ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Output parent may not be a link' }
if ($output.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Output inside source' }
for ($ancestor = $parent; $null -ne $ancestor; $ancestor = $ancestor.Parent) {
    if ($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Output ancestor may not be a link' }
    if (Test-Path -LiteralPath (Join-Path $ancestor.FullName '.git')) { throw 'Output inside Git checkout' }
}
if (Test-Path -LiteralPath $output) { throw 'Output exists' }
$names = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
$baseNames = Get-Content -LiteralPath (Join-Path $OpenKhRoot $referenceFiles[0])
foreach ($name in $baseNames) {
    $null = $names.Add($name)
    if ($name.Contains('anm/')) {
        foreach ($region in @('jp','us','fm')) { $null = $names.Add($name.Replace('anm/',"anm/$region/")) }
    }
    if ($name.StartsWith('bgm/')) { $null = $names.Add($name.Replace('.bgm','.win32.scd')) }
    if ($name.StartsWith('se/') -or $name.StartsWith('gumibattle/se/')) { $null = $names.Add($name.Replace('.seb','.win32.scd')) }
    if ($name.StartsWith('vagstream/')) { $null = $names.Add($name.Replace('.vas','.win32.scd')) }
    if ($name.StartsWith('voice/')) { $null = $names.Add($name.Replace('.vag','.win32.scd').Replace('.vsb','.win32.scd')) }
}
$constants = Get-Content -LiteralPath (Join-Path $OpenKhRoot $referenceFiles[1]) -Raw
function Read-ReferenceArray([string]$Name) {
    $array = [regex]::Match($constants, 'public static readonly string\[\] ' + $Name + ' = [^{]+\{([^}]+)\}')
    if (!$array.Success) { throw 'Pinned reference array not found' }
    return @([regex]::Matches($array.Groups[1].Value, '"([^"]*)"') | ForEach-Object {$_.Groups[1].Value})
}
foreach ($language in (Read-ReferenceArray 'Languages' | Select-Object -Unique)) {
    foreach ($world in (Read-ReferenceArray 'WorldIds')) {
        foreach ($number in 0..63) {
            foreach ($extension in @('ard','map','bar')) {
                $kind = if ($extension -eq 'ard') {'ard'} else {'map'}
                $prefix = if ($language -eq '') {$kind} else {"$kind/$language"}
                $null = $names.Add(('{0}/{1}{2:D2}.{3}' -f $prefix,$world,$number,$extension))
            }
        }
    }
}
foreach ($extra in @('item-011.imd','KH2.IDX','ICON/ICON0.PNG','ICON/ICON0_EN.png')) { $null = $names.Add($extra) }
$byHash = @{}
foreach ($name in $names) {
    $hash = [Convert]::ToHexString([Security.Cryptography.MD5]::HashData([Text.Encoding]::UTF8.GetBytes($name)))
    if ($byHash.ContainsKey($hash) -and $byHash[$hash] -cne $name) { throw 'Candidate MD5 collision; names ambiguous' }
    $byHash[$hash] = $name
}
$rows = [Collections.Generic.List[object]]::new()
$summaries = [Collections.Generic.List[object]]::new()
foreach ($pair in @('first','second','third','fourth','fifth','sixth')) {
    $relative = "Image/dt/kh2_$pair.hed"
    $expected = @($manifest.files | Where-Object relative_path -eq $relative)
    if ($expected.Count -ne 1) { throw 'Missing/duplicate bound HED identity' }
    $hedPath = Join-Path $root $relative
    if ((Get-FileHash -LiteralPath $hedPath -Algorithm SHA256).Hash -ne $expected[0].sha256) { throw 'HED identity mismatch' }
    $hed = [IO.BinaryReader]::new([IO.File]::OpenRead($hedPath))
    $ordinal = 0
    $matched = 0
    try {
        if ($hed.BaseStream.Length -ne $expected[0].size -or $hed.BaseStream.Length % 32 -ne 0) { throw 'Invalid HED size' }
        while ($hed.BaseStream.Position -lt $hed.BaseStream.Length) {
            $hash = [Convert]::ToHexString($hed.ReadBytes(16))
            $offset = $hed.ReadInt64()
            $stored = $hed.ReadInt32()
            $original = $hed.ReadInt32()
            $candidate = $byHash[$hash]
            if ($null -ne $candidate) { $matched++ }
            $rows.Add([pscustomobject]@{index=$relative;ordinal=$ordinal;name_hash=$hash;package_offset=$offset;stored_length=$stored;original_length=$original;candidate_name=$candidate;semantic_identity_verified=$false})
            $ordinal++
        }
    } finally { $hed.Dispose() }
    $summaries.Add([pscustomobject]@{index=$relative;records=$ordinal;candidate_matches=$matched;unknown=$ordinal-$matched})
}
$referenceReceipts = @($referenceFiles | ForEach-Object {[pscustomobject]@{path=$_;sha256=(Get-FileHash -LiteralPath (Join-Path $OpenKhRoot $_) -Algorithm SHA256).Hash}})
$report = [ordered]@{schema_version=1;qualification='candidate_names_only_unqualified';source_content_identity=$manifest.content_identity;openkh_pin=$pin;reference_files=$referenceReceipts;candidate_count=$names.Count;candidate_rule='MD5 of exact UTF-8 candidate name, following KH2-only pinned EgsTools generation';summaries=$summaries;records=$rows;limits=@('name hash match is a candidate, not semantic/runtime identity','shared packages and language completeness not established','no gameplay-supported source profile')}
$bytes = [Text.Encoding]::UTF8.GetBytes(($report | ConvertTo-Json -Depth 8))
$file = [IO.File]::Open($output,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try { $file.Write($bytes); $file.Flush($true) } finally { $file.Dispose() }
$summaries | Select-Object index,records,candidate_matches,unknown
