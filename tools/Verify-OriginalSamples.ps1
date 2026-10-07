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
$report = Get-Content -LiteralPath $ProbePath -Raw | ConvertFrom-Json
$messageMode = $report.qualification -eq 'message_font_structure_inspection_only'
if ($messageMode) {
    if (git -C $OpenKhRoot status --porcelain -- OpenKh.Kh2/Msg.cs) { throw 'Reference message reader is modified' }
    $msgText = Get-Content -LiteralPath (Join-Path $OpenKhRoot 'OpenKh.Kh2/Msg.cs') -Raw
    # Move using directives only; the upstream Msg method bodies are unchanged.
    $referenceText = "using System.Collections.Generic;`nusing System.Linq;`n" + $referenceText + ($msgText -replace '(?m)^using [^;]+;\r?\n','')
    $fontFiles = @('OpenKh.Kh2/RawBitmap.cs','OpenKh.Kh2/Ps2.Utility.cs','OpenKh.Kh2/Ps2.Repl.cs',
        'OpenKh.Imaging/IImage.cs','OpenKh.Imaging/IImageRead.cs','OpenKh.Imaging/PixelFormat.cs','OpenKh.Imaging/ImageDataHelpers.cs')
    if (git -C $OpenKhRoot status --porcelain -- $fontFiles) { throw 'Reference font sources are modified' }
    $referenceText = "using System.Drawing;`nusing OpenKh.Imaging;`n" + $referenceText
    foreach ($path in $fontFiles | Where-Object {$_ -ne 'OpenKh.Imaging/ImageDataHelpers.cs'}) {
        $referenceText += (Get-Content -LiteralPath (Join-Path $OpenKhRoot $path) -Raw) -replace '(?m)^using [^;]+;\r?\n',''
    }
    # Use the exact upstream nibble-swap method body, without unrelated image helpers.
    $helpers = Get-Content -LiteralPath (Join-Path $OpenKhRoot 'OpenKh.Imaging/ImageDataHelpers.cs') -Raw
    $swap = [regex]::Match($helpers, '(?s)public static void SwapEndianIndexed4\(byte\[\] data\)\s*\{.*?\n\s*\}')
    if (!$swap.Success) { throw 'Pinned nibble-swap method not found' }
    $referenceText += 'namespace OpenKh.Imaging { public static class ImageDataHelpers {' + $swap.Value + '}}'
}
# EgsEncryption only depends on this stream extension; compile the unchanged
# transform in memory. This does not run any OpenKH source writer/extractor.
$extension = @'
namespace OpenKh.Common {
    public static class ProbeStreamExtension {
        public static uint PeekUInt32(this System.IO.Stream stream) {
            long position = stream.Position;
            try { return new System.IO.BinaryReader(stream).ReadUInt32(); }
            finally { stream.Position = position; }
        }
        public static byte[] ReadBytes(this System.IO.Stream stream, int count) {
            byte[] result = new byte[count];
            stream.ReadExactly(result);
            return result;
        }
    }
}
'@
Add-Type -TypeDefinition ($referenceText + $extension)
if (!$messageMode -and $report.qualification -ne 'bounded_original_sample_inspection_only') { throw 'Unexpected report qualification' }
$totalBytes = 0L
$replacementBytes = 0L
$replacementVerified = 0
$messageRecordsVerified = 0
$messageEncodedBytes = 0L
$barRecordsVerified = 0
$fontImagesVerified = 0
$fontPixelsVerified = 0L
$spacingTablesVerified = 0
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
        if ($messageMode) {
            # Independently read all BAR scalars before selecting message payloads.
            $barStream = [IO.MemoryStream]::new($data,$false)
            $barReader = [IO.BinaryReader]::new($barStream)
            try {
                if ($barReader.ReadUInt32() -ne 0x01524142 -or $barReader.ReadInt32() -ne $sample.bar.entries.Count -or
                    $barReader.ReadInt32() -ne $sample.bar.header_field_8 -or $barReader.ReadInt32() -ne $sample.bar.header_field_12) { throw 'Independent BAR header mismatch' }
                foreach ($child in $sample.bar.entries) {
                    if ($barReader.ReadUInt16() -ne $child.entry_type -or $barReader.ReadInt16() -ne $child.index -or
                        [Convert]::ToHexString($barReader.ReadBytes(4)) -ne [Convert]::ToHexString([byte[]]$child.name_bytes) -or
                        $barReader.ReadInt32() -ne $child.offset -or $barReader.ReadInt32() -ne $child.size) { throw 'Independent BAR table mismatch' }
                    $barRecordsVerified++
                }
                foreach ($table in $sample.message_tables) {
                    $child = $sample.bar.entries[$table.bar_ordinal]
                    $messageData = [byte[]]::new($child.size)
                    [Array]::Copy($data,$child.offset,$messageData,0,$child.size)
                    $messageStream = [IO.MemoryStream]::new($messageData,$false)
                    try { $decodedMessages = [OpenKh.Kh2.Msg]::Read($messageStream) } finally { $messageStream.Dispose() }
                    if ($decodedMessages.Count -ne $table.inventory.entries.Count) { throw 'Reference message count mismatch' }
                    for ($recordIndex = 0; $recordIndex -lt $decodedMessages.Count; $recordIndex++) {
                        $actual = $decodedMessages[$recordIndex]
                        $expectedMessage = $table.inventory.entries[$recordIndex]
                        $sourceOffset = [BitConverter]::ToInt32($messageData,12+8*$recordIndex)
                        if ($actual.Id -ne $expectedMessage.id -or $sourceOffset -ne $expectedMessage.source_offset -or
                            $actual.Data.Length -ne $expectedMessage.encoded_length -or
                            [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($actual.Data)) -ne $expectedMessage.sha256) { throw 'Reference message id/span/hash mismatch' }
                        $messageRecordsVerified++
                        $messageEncodedBytes += $actual.Data.Length
                    }
                }
                foreach ($spacing in $sample.spacing_tables) {
                    $child = $sample.bar.entries[$spacing.bar_ordinal]
                    $spacingBytes = [byte[]]::new($child.size)
                    [Array]::Copy($data,$child.offset,$spacingBytes,0,$child.size)
                    if ($child.entry_type -ne 2 -or $spacingBytes.Length -ne $spacing.byte_count -or
                        [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($spacingBytes)) -ne $spacing.sha256) {throw 'Independent spacing bytes mismatch'}
                    $spacingTablesVerified++
                }
                foreach ($font in $sample.font_images) {
                    $child = $sample.bar.entries[$font.bar_ordinal]
                    if ($child.entry_type -ne 36) {throw 'Font payload is not a RawBitmap record'}
                    $fontBytes = [byte[]]::new($child.size)
                    [Array]::Copy($data,$child.offset,$fontBytes,0,$child.size)
                    $fontStream = [IO.MemoryStream]::new($fontBytes,$false)
                    try {
                        $slot = [Text.Encoding]::ASCII.GetString([byte[]]$child.name_bytes).TrimEnd([char]0)
                        if ($slot -eq 'icon') {
                            $width=256; $height=160; $bpp=8
                            if ($font.palette_lane -ne 0) {throw 'Unexpected icon palette lane'}
                            $image=[OpenKh.Kh2.RawBitmap]::Read8bit($fontStream,$width,$height)
                        } elseif ($slot -in @('sys','evt')) {
                            $width=512; $bpp=4
                            $height = if ($fontBytes.Length -lt 131072) {256} elseif ($fontBytes.Length -lt 262144) {512} elseif ($fontBytes.Length -lt 524288) {1024} else {throw 'Unsupported reference font size'}
                            $image=if ($font.palette_lane -eq 0) {[OpenKh.Kh2.RawBitmap]::Read4bitPalette1($fontStream,$width,$height)}
                                elseif ($font.palette_lane -eq 1) {[OpenKh.Kh2.RawBitmap]::Read4bitPalette2($fontStream,$width,$height)}
                                else {throw 'Unexpected font palette lane'}
                        } else {throw 'Unrecognized font slot'}
                        $packed=$image.GetData(); $clut=$image.GetClut()
                        $embedded=($fontBytes.Length -gt ($width*$height*$bpp/8))
                        if ($image.Size.Width -ne $font.width -or $image.Size.Height -ne $font.height -or $bpp -ne $font.bits_per_pixel -or
                            $embedded -ne $font.embedded_palette -or
                            [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($packed)) -ne $font.packed_pixel_sha256 -or
                            [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($clut)) -ne $font.clut_rgba_sha256) {throw 'Pinned font dimensions/packed pixels/palette mismatch'}
                        $rgba=[byte[]]::new($width*$height*4)
                        for ($pixel=0; $pixel -lt $width*$height; $pixel++) {
                            $color=if ($bpp -eq 8) {[int]$packed[$pixel]} elseif (($pixel -band 1) -eq 0) {$packed[[int][Math]::Floor($pixel/2)] -shr 4} else {$packed[[int][Math]::Floor($pixel/2)] -band 15}
                            [Array]::Copy($clut,4*$color,$rgba,4*$pixel,4)
                        }
                        if ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($rgba)) -ne $font.pixels_rgba_sha256) {throw 'Expanded straight RGBA font pixels mismatch'}
                        $fontImagesVerified++
                        $fontPixelsVerified += $width*$height
                    } finally {$fontStream.Dispose()}
                }
            } finally { $barReader.Dispose(); $barStream.Dispose() }
        }
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
if ($messageMode) { "PASS: $barRecordsVerified BAR records and $messageRecordsVerified message records / $messageEncodedBytes encoded bytes match independent scalars and pinned OpenKH Msg.Read; no glyph/Unicode mapping asserted." }
if ($messageMode) { "PASS: $fontImagesVerified font palette planes / $fontPixelsVerified pixels match pinned RawBitmap packed pixels and palettes plus independent RGBA expansion; $spacingTablesVerified spacing byte tables match. Runtime glyph mapping/rendering remains unverified." }
