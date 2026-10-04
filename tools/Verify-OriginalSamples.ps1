# Read-only comparison against pinned public OpenKH code and .NET DeflateStream.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot
)
$ErrorActionPreference = 'Stop'
$pin = '7a3b945c538d32c6a285128c98aefba093f52ceb'
if ((git -C $OpenKhRoot rev-parse HEAD) -ne $pin) { throw 'OpenKH checkout is not the audited pin' }
if (git -C $OpenKhRoot status --porcelain -- OpenKh.Egs/EgsEncryption.cs) { throw 'Reference transform is locally modified' }
$referenceText = Get-Content -LiteralPath (Join-Path $OpenKhRoot 'OpenKh.Egs/EgsEncryption.cs') -Raw
# EgsEncryption only depends on this stream extension; compile the unchanged
# transform in memory. This does not run any OpenKH source writer/extractor.
$extension = @'
namespace OpenKh.Common {
    public static class ProbeStreamExtension {
        public static byte[] ReadBytes(this System.IO.Stream stream, int count) {
            byte[] result = new byte[count];
            stream.ReadExactly(result);
            return result;
        }
    }
}
'@
Add-Type -TypeDefinition ($referenceText + $extension)
$report = Get-Content -LiteralPath $ProbePath -Raw | ConvertFrom-Json
if ($report.qualification -ne 'bounded_original_sample_inspection_only') { throw 'Unexpected report qualification' }
$totalBytes = 0L
$replacementBytes = 0L
$replacementVerified = 0
foreach ($sample in $report.samples) {
    $hedPath = Join-Path $InstallRoot $sample.index
    $pkgPath = [IO.Path]::ChangeExtension($hedPath, 'pkg')
    $hed = [IO.BinaryReader]::new([IO.File]::OpenRead($hedPath))
    $pkg = [IO.BinaryReader]::new([IO.File]::OpenRead($pkgPath))
    try {
        $hed.BaseStream.Position = [long]$sample.ordinal * 32
        $nameHash = $hed.ReadBytes(16)
        $offset = $hed.ReadInt64()
        $indexedSize = $hed.ReadInt32()
        $null = $hed.ReadInt32()
        if ($offset -ne $sample.package_offset -or
            [Convert]::ToHexString($nameHash) -ne [Convert]::ToHexString([byte[]]$sample.name_hash)) { throw 'Independent HED identity mismatch' }
        $pkg.BaseStream.Position = $offset
        $seed = $pkg.ReadBytes(16)
        $expected = [BitConverter]::ToInt32($seed, 0)
        $replacementCount = [BitConverter]::ToInt32($seed, 4)
        $tag = [BitConverter]::ToInt32($seed, 8)
        $length = if ($tag -ge 0) { $tag } else { $expected }
        if ($expected -lt 0 -or $expected -gt 33554432 -or $length -lt 0 -or $length -gt 33554432 -or $replacementCount -lt 0 -or
            (16L + 48L * $replacementCount + $length) -gt $indexedSize -or ($offset + $indexedSize) -gt $pkg.BaseStream.Length) { throw 'Invalid independent sample range/limit' }
        $pkg.BaseStream.Position = $offset + 16 + 48L * $replacementCount
        $data = $pkg.ReadBytes($length)
        if ($data.Length -ne $length) { throw 'Truncated sample' }
        if ($tag -gt -2) {
            # Preserve the upstream four-byte loop; an incompatible partial
            # word fails this comparison instead of silently changing its behavior.
            $key = [OpenKh.Egs.EgsEncryption]::GenerateKey($seed, 10)
            for ($chunk = 0; $chunk -lt [Math]::Min($length, 256); $chunk += 16) {
                [OpenKh.Egs.EgsEncryption]::DecryptChunk($key, $data, $chunk, 10)
            }
        }
        if ($tag -ge 0) {
            $packed = [IO.MemoryStream]::new($data, $false)
            $packed.Position = 2
            $deflate = [IO.Compression.DeflateStream]::new($packed, [IO.Compression.CompressionMode]::Decompress)
            $decoded = [IO.MemoryStream]::new()
            try {
                $buffer = [byte[]]::new(65536)
                while (($read = $deflate.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    if ($decoded.Length + $read -gt $expected) { throw 'Independent decoded output exceeds header' }
                    $decoded.Write($buffer, 0, $read)
                }
                $data = $decoded.ToArray()
            } finally { $deflate.Dispose(); $packed.Dispose(); $decoded.Dispose() }
        }
        if ($tag -ne $sample.compression_tag -or $replacementCount -ne $sample.replacement_count -or
            $data.Length -ne $expected -or $data.Length -ne $sample.original_size -or
            [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($data)) -ne $sample.sha256) { throw 'Independent original decode/hash mismatch' }
        $totalBytes += $data.Length
        $replacementStart = $offset + 16 + 48L * $replacementCount + $length
        foreach ($replacement in $sample.replacement_samples) {
            $meta = $replacement.metadata
            $pkg.BaseStream.Position = $offset + 16 + 48L * $meta.ordinal
            $replacementName = $pkg.ReadBytes(32)
            $declaredOffset = $pkg.ReadInt32()
            $originalOffset = $pkg.ReadInt32()
            $replacementExpected = $pkg.ReadInt32()
            $replacementTag = $pkg.ReadInt32()
            $replacementStored = if ($replacementTag -ge 0) {$replacementTag} else {$replacementExpected}
            $replacementPadded = [long]([Math]::Ceiling($replacementStored / 16.0) * 16)
            $replacementStart = [long]([Math]::Ceiling($replacementStart / 16.0) * 16)
            if ($replacementExpected -lt 0 -or $replacementExpected -gt 33554432 -or
                $replacementTag -lt -2 -or $replacementPadded -gt 33554432 -or
                $replacementStart + $replacementPadded -gt $offset + $indexedSize) { throw 'Invalid independent replacement range/limit' }
            if ($replacementStart -ne $meta.package_offset -or $replacementPadded -ne $meta.padded_stored_length -or
                $declaredOffset -ne $meta.declared_offset -or $originalOffset -ne $meta.original_asset_offset -or
                $replacementExpected -ne $meta.declared_decompressed_length -or $replacementTag -ne $meta.compression_tag -or
                [Convert]::ToHexString($replacementName) -ne [Convert]::ToHexString([byte[]]$meta.name_bytes)) { throw 'Independent replacement metadata mismatch' }
            $pkg.BaseStream.Position = $replacementStart
            $replacementData = $pkg.ReadBytes([int]$replacementPadded)
            if ($replacementData.Length -ne $replacementPadded) { throw 'Truncated replacement' }
            if ($replacementTag -gt -2) {
                $key = [OpenKh.Egs.EgsEncryption]::GenerateKey($seed, 10)
                for ($chunk = 0; $chunk -lt [Math]::Min($replacementPadded, 256); $chunk += 16) {
                    [OpenKh.Egs.EgsEncryption]::DecryptChunk($key, $replacementData, $chunk, 10)
                }
            }
            if ($replacementTag -ge 0) {
                $packed = [IO.MemoryStream]::new($replacementData, $false)
                $packed.Position = 2
                $deflate = [IO.Compression.DeflateStream]::new($packed, [IO.Compression.CompressionMode]::Decompress)
                $decoded = [IO.MemoryStream]::new()
                try {
                    $buffer = [byte[]]::new(65536)
                    while (($read = $deflate.Read($buffer, 0, $buffer.Length)) -gt 0) {
                        if ($decoded.Length + $read -gt $replacementExpected) { throw 'Independent replacement output exceeds header' }
                        $decoded.Write($buffer, 0, $read)
                    }
                    $replacementData = $decoded.ToArray()
                } finally { $deflate.Dispose(); $packed.Dispose(); $decoded.Dispose() }
                if ($replacementData.Length -ne $replacementExpected) { throw 'Replacement output length mismatch' }
            }
            if ($replacementData.Length -ne $replacement.decoded_size -or
                [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($replacementData)) -ne $replacement.sha256) { throw 'Independent replacement decode/hash mismatch' }
            $replacementBytes += $replacementData.Length
            $replacementVerified++
            $replacementStart += $replacementPadded
        }
    } finally { $hed.Dispose(); $pkg.Dispose() }
}
"PASS: $($report.samples.Count) originals / $totalBytes bytes and $replacementVerified replacements / $replacementBytes bytes agree with pinned OpenKH transform and .NET deflate; source was only read."
