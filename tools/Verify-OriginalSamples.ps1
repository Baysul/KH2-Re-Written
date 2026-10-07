# Read-only comparison against pinned public OpenKH code and .NET DeflateStream.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot,
    [string]$ReplacementPreviewDirectory
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'ManagedReader.ps1')
if ($ReplacementPreviewDirectory) {
    $ReplacementPreviewDirectory = Assert-PrivateResearchDirectory -Directory $ReplacementPreviewDirectory -SourceRoot $InstallRoot
}
$pngPreviews = 0
$previewPixels = 0L
$pin = '7a3b945c538d32c6a285128c98aefba093f52ceb'
if ((git -C $OpenKhRoot rev-parse HEAD) -ne $pin) { throw 'OpenKH checkout is not the audited pin' }
if (git -C $OpenKhRoot status --porcelain -- OpenKh.Egs/EgsEncryption.cs) { throw 'Reference transform is locally modified' }
$referenceText = Get-Content -LiteralPath (Join-Path $OpenKhRoot 'OpenKh.Egs/EgsEncryption.cs') -Raw
$report = Get-Content -LiteralPath $ProbePath -Raw | ConvertFrom-Json
$messageMode = $report.qualification -eq 'message_font_structure_inspection_only'
$tokenMode = $false
$reencodingMode = $false
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
    $tokenMode = @($report.samples.message_tables | Where-Object {$_.system_decoder_tokens}).Count -gt 0
    $reencodingMode = @($report.samples.message_tables | Where-Object {$_.international_reencoding.requested}).Count -gt 0
    if ($tokenMode) {
        $tokenFiles = @('MessageCommand.cs','MessageCommandModel.cs','IMessageDecode.cs',
            'Internals/BaseCmdModel.cs','Internals/DataCmdModel.cs','Internals/SingleDataCmdModel.cs',
            'Internals/SimpleCmdModel.cs','Internals/TextCmdModel.cs','Internals/TableCmdModel.cs',
            'Internals/UnsupportedCmdModel.cs','Internals/BaseMessageDecoder.cs','Internals/InternationalSystemDecode.cs') |
            ForEach-Object {"OpenKh.Kh2/Messages/$_"}
        if (git -C $OpenKhRoot status --porcelain -- $tokenFiles) {throw 'Reference system decoder sources are modified'}
        $referenceText = "using System.Text;`n" + $referenceText
        foreach ($path in $tokenFiles) {
            $referenceText += (Get-Content -LiteralPath (Join-Path $OpenKhRoot $path) -Raw) -replace '(?m)^using [^;]+;\r?\n',''
        }
        if ($reencodingMode) {
            $encoderFiles = @('OpenKh.Kh2/Messages/IMessageEncode.cs',
                'OpenKh.Kh2/Messages/Internals/InternationalSystemEncode.cs',
                'OpenKh.Common/Exceptions/ParseException.cs')
            if (git -C $OpenKhRoot status --porcelain -- $encoderFiles) {throw 'Reference encoder sources are modified'}
            $referenceText = "using OpenKh.Common.Exceptions;`n" + $referenceText
            foreach ($path in $encoderFiles) {
                $referenceText += (Get-Content -LiteralPath (Join-Path $OpenKhRoot $path) -Raw) -replace '(?m)^using [^;]+;\r?\n',''
            }
            $referenceText += @'
namespace OpenKh.Kh2.Messages.Internals {
    public static class ResearchReencoding {
        public static string Fingerprint(byte[] data, out int count, out int bytes,
            out int changed, out int textChanged, out int unsupported, out int unmapped,
            out string wholeHash, out bool wholeExact) {
            int tokens; long[] opcodes;
            ResearchSystemTokens.Fingerprint(data,out tokens,out opcodes);
            count=0; bytes=0; changed=0; textChanged=0; unsupported=0; unmapped=0;
            wholeHash=null; wholeExact=false;
            for (int opcode=0x19; opcode<=0x1f; opcode++) unmapped+=(int)opcodes[opcode];
            // Do not qualify the public generic question-mark fallback.
            if (unmapped!=0) return null;
            var spans = new List<KeyValuePair<int,int>>();
            int pending=-1;
            for (int cursor=0; cursor<data.Length;) {
                int start=cursor; byte opcode=data[cursor++];
                cursor+=ResearchSystemTokens.ParameterLength(opcode);
                var model=InternationalSystemDecode._table[opcode];
                if (model.Command==OpenKh.Kh2.Messages.MessageCommand.PrintText) {
                    if (pending<0) pending=start;
                } else {
                    if (pending>=0) {spans.Add(new KeyValuePair<int,int>(pending,start-pending));pending=-1;}
                    spans.Add(new KeyValuePair<int,int>(start,cursor-start));
                }
            }
            if (pending>=0) spans.Add(new KeyValuePair<int,int>(pending,data.Length-pending));
            var decoded=new InternationalSystemDecode().Decode(data);
            if (decoded.Count!=spans.Count) throw new System.IO.InvalidDataException("Segment spans differ");
            var encoder=new InternationalSystemEncode();
            using (var fingerprint=new System.IO.MemoryStream())
            using (var whole=new System.IO.MemoryStream()) {
                using (var writer=new System.IO.BinaryWriter(fingerprint,System.Text.Encoding.UTF8,true)) {
                    for (int i=0; i<decoded.Count; i++) {
                        var entry=decoded[i];
                        byte tag=entry.Command==OpenKh.Kh2.Messages.MessageCommand.PrintText ? (byte)1 :
                            entry.Command==OpenKh.Kh2.Messages.MessageCommand.PrintComplex ? (byte)2 :
                            entry.Command==OpenKh.Kh2.Messages.MessageCommand.Unsupported ? (byte)4 : (byte)3;
                        var encoded=encoder.Encode(new List<OpenKh.Kh2.Messages.MessageCommandModel>{entry});
                        var span=spans[i];
                        bool exact=encoded.Length==span.Value;
                        if (exact) for (int j=0; j<encoded.Length; j++) if (encoded[j]!=data[span.Key+j]) {exact=false;break;}
                        if (!exact) {changed++;if(tag==1 || tag==2) textChanged++;}
                        if (tag==4) unsupported++;
                        writer.Write(tag); writer.Write(encoded.Length); writer.Write(encoded);
                        whole.Write(encoded); bytes+=encoded.Length; count++;
                    }
                    writer.Flush();
                }
                var all=whole.ToArray();
                wholeExact=all.SequenceEqual(data);
                wholeHash=Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(all)).ToLowerInvariant();
                return Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(fingerprint.ToArray())).ToLowerInvariant();
            }
        }
    }
}
'@
        }
        # Own observation adapter: derive lengths from unchanged upstream model objects.
        # No text is substituted for unknown entries and no rendering rule is inferred.
        $referenceText += @'
namespace OpenKh.Kh2.Messages.Internals {
    public static class ResearchSystemTokens {
        public static int ParameterLength(byte opcode) {
            var model = InternationalSystemDecode._table[opcode];
            return model is TableCmdModel ? 1 : model.Length;
        }
        public static string Fingerprint(byte[] data, out int count, out long[] opcodes) {
            if (data.Length > 1048576) throw new System.IO.InvalidDataException("Limit");
            using (var boundaries = new System.IO.MemoryStream()) {
                using (var writer = new System.IO.BinaryWriter(boundaries, System.Text.Encoding.UTF8, true)) {
                    count = 0; opcodes = new long[256];
                    for (int cursor = 0; cursor < data.Length;) {
                        int start = cursor; byte opcode = data[cursor++];
                        int length = ParameterLength(opcode);
                        if (cursor + length > data.Length) throw new System.IO.InvalidDataException("Truncated");
                        cursor += length;
                        writer.Write(start); writer.Write(opcode); writer.Write(length);
                        count++; opcodes[opcode]++;
                        if (opcode == 0) {
                            if (cursor != data.Length) throw new System.IO.InvalidDataException("Trailing");
                            writer.Flush();
                            return Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(boundaries.ToArray())).ToLowerInvariant();
                        }
                    }
                }
            }
            throw new System.IO.InvalidDataException("Unterminated");
        }
        public static string TextFingerprint(byte[] data, out int count, out int characters,
            out int complex, out int unsupported, out int unmapped) {
            int tokenCount; long[] opcodes;
            Fingerprint(data, out tokenCount, out opcodes);
            count = 0; characters = 0; complex = 0; unsupported = 0; unmapped = 0;
            for (int opcode = 0x19; opcode <= 0x1f; opcode++) unmapped += (int)opcodes[opcode];
            // The upstream generic '?' is intentionally not compatibility evidence.
            if (unmapped != 0) return null;
            var decoded = new InternationalSystemDecode().Decode(data);
            using (var buffer = new System.IO.MemoryStream()) {
                using (var writer = new System.IO.BinaryWriter(buffer, System.Text.Encoding.UTF8, true)) {
                    foreach (var entry in decoded) {
                        byte tag; byte[] content;
                        if (entry.Command == OpenKh.Kh2.Messages.MessageCommand.PrintText ||
                            entry.Command == OpenKh.Kh2.Messages.MessageCommand.PrintComplex) {
                            tag = (byte)(entry.Command == OpenKh.Kh2.Messages.MessageCommand.PrintText ? 1 : 2);
                            content = System.Text.Encoding.UTF8.GetBytes(entry.Text);
                            characters += entry.Text.Length; // All characters in this pinned profile are BMP.
                            if (tag == 2) complex++;
                        } else if (entry.Command == OpenKh.Kh2.Messages.MessageCommand.Unsupported) {
                            tag = 4; content = entry.Data; unsupported++;
                        } else {
                            tag = 3;
                            byte opcode = InternationalSystemDecode._table.Single(x => x.Value.Command == entry.Command).Key;
                            content = new byte[1 + entry.Data.Length]; content[0] = opcode;
                            Array.Copy(entry.Data, 0, content, 1, entry.Data.Length);
                        }
                        writer.Write(tag); writer.Write(content.Length); writer.Write(content); count++;
                    }
                    writer.Flush();
                }
                return Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(buffer.ToArray())).ToLowerInvariant();
            }
        }
    }
}
'@
    }
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
$tokenRecordsVerified = 0
$tokenFailuresVerified = 0
$systemTokensVerified = 0L
$textRecordsVerified = 0
$unicodeCharactersVerified = 0L
$complexSymbolsVerified = 0L
$unsupportedTextSegmentsVerified = 0L
$unmappedTextRecords = 0
$reencodedRecordsVerified = 0
$reencodedBytesVerified = 0L
$reencodedChangedSegments = 0
$reencodedTextChanged = 0
$reencodedUnsupported = 0
$reencodedWholeExact = 0
$reencodedUnmappedRecords = 0
if ($tokenMode) {
    $arities = @(0,0,0,0,1,6,1,4,3,1,1,1,1,0,1,5,0,4,2,4,2,2,1,2,2,1,1,1,1,1,1,1)
    foreach ($opcode in 0..255) {
        $expectedArity = if ($opcode -lt 32) {$arities[$opcode]} else {0}
        if ([OpenKh.Kh2.Messages.Internals.ResearchSystemTokens]::ParameterLength([byte]$opcode) -ne $expectedArity) {
            throw 'Generated system opcode arity oracle differs'
        }
    }
}
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
                    $tokenCounts = [long[]]::new(256)
                    $tableTokenFailures = 0
                    if ($table.system_decoder_tokens -and $table.system_decoder_tokens.records.Count -ne $decodedMessages.Count) {
                        throw 'Reference token record count mismatch'
                    }
                    if ($table.international_text.requested -and $table.international_text.records.Count -ne $decodedMessages.Count) {
                        throw 'International text record count mismatch'
                    }
                    if ($table.international_reencoding.requested -and $table.international_reencoding.records.Count -ne $decodedMessages.Count) {
                        throw 'Reencoding record count mismatch'
                    }
                    for ($recordIndex = 0; $recordIndex -lt $decodedMessages.Count; $recordIndex++) {
                        $actual = $decodedMessages[$recordIndex]
                        $expectedMessage = $table.inventory.entries[$recordIndex]
                        $sourceOffset = [BitConverter]::ToInt32($messageData,12+8*$recordIndex)
                        if ($actual.Id -ne $expectedMessage.id -or $sourceOffset -ne $expectedMessage.source_offset -or
                            $actual.Data.Length -ne $expectedMessage.encoded_length -or
                            [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($actual.Data)) -ne $expectedMessage.sha256) { throw 'Reference message id/span/hash mismatch' }
                        $messageRecordsVerified++
                        $messageEncodedBytes += $actual.Data.Length
                        if ($table.system_decoder_tokens) {
                            $observation = $table.system_decoder_tokens.records[$recordIndex]
                            if ($observation.id -ne $actual.Id -or $observation.source_offset -ne $sourceOffset) {
                                throw 'Reference token identity mismatch'
                            }
                            $tokenCount=0; $opcodes=[long[]]::new(256); $fingerprint=$null
                            try {
                                $fingerprint=[OpenKh.Kh2.Messages.Internals.ResearchSystemTokens]::Fingerprint($actual.Data,[ref]$tokenCount,[ref]$opcodes)
                            } catch [System.Management.Automation.MethodInvocationException] {
                                if ($_.Exception.InnerException -isnot [IO.InvalidDataException]) {throw}
                            }
                            if ($null -eq $fingerprint) {
                                if ($observation.observation.status -cne 'FAIL') {throw 'Expected tokenization failure not reported'}
                                $tokenFailuresVerified++; $tableTokenFailures++
                            } else {
                                if ($observation.observation.status -cne 'PASS' -or
                                    $observation.observation.token_count -ne $tokenCount -or
                                    $observation.observation.boundaries_sha256 -cne $fingerprint) {throw 'Independent token boundary mismatch'}
                                for ($opcode=0; $opcode -lt 256; $opcode++) {$tokenCounts[$opcode]+=$opcodes[$opcode]}
                                $tokenRecordsVerified++; $systemTokensVerified+=$tokenCount
                            }
                        }
                        if ($table.international_text.requested) {
                            $textRecord = $table.international_text.records[$recordIndex]
                            if ($textRecord.id -ne $actual.Id -or $textRecord.source_offset -ne $sourceOffset) {
                                throw 'International text source identity differs'
                            }
                            $segments=0; $characters=0; $complex=0; $unsupported=0; $unmapped=0
                            $fingerprint=[OpenKh.Kh2.Messages.Internals.ResearchSystemTokens]::TextFingerprint(
                                $actual.Data,[ref]$segments,[ref]$characters,[ref]$complex,[ref]$unsupported,[ref]$unmapped)
                            $textObservation=$textRecord.observation
                            if ($textObservation.status -cne 'PASS' -or $textObservation.unmapped_tables -ne $unmapped) {
                                throw 'International decode status/unmapped count differs'
                            }
                            if ($unmapped) {
                                $unmappedTextRecords++
                            } else {
                                if ($textObservation.content_sha256 -cne $fingerprint -or
                                    $textObservation.segment_count -ne $segments -or
                                    $textObservation.unicode_scalar_count -ne $characters -or
                                    $textObservation.complex_symbols -ne $complex -or
                                    $textObservation.unsupported_segments -ne $unsupported) {throw 'International decoded content differs'}
                                $textRecordsVerified++; $unicodeCharactersVerified+=$characters
                                $complexSymbolsVerified+=$complex; $unsupportedTextSegmentsVerified+=$unsupported
                            }
                        }
                        if ($table.international_reencoding.requested) {
                            $entry = $table.international_reencoding.records[$recordIndex]
                            if ($entry.id -ne $actual.Id -or $entry.source_offset -ne $sourceOffset) {throw 'Reencoding source identity differs'}
                            $count=0; $encodedBytes=0; $changed=0; $textChanged=0; $unsupported=0; $unmapped=0; $wholeHash=$null; $wholeExact=$false
                            $encodingHash = [OpenKh.Kh2.Messages.Internals.ResearchReencoding]::Fingerprint(
                                $actual.Data,[ref]$count,[ref]$encodedBytes,[ref]$changed,[ref]$textChanged,
                                [ref]$unsupported,[ref]$unmapped,[ref]$wholeHash,[ref]$wholeExact)
                            $observation = $entry.observation
                            if ($observation.status -cne 'PASS' -or $observation.unmapped_tables -ne $unmapped) {throw 'Reencoding status/unmapped mismatch'}
                            if ($unmapped) {
                                if ($observation.canonical_sha256 -or $observation.whole_record_exact) {throw 'Unmapped encoding incorrectly qualified'}
                                $reencodedUnmappedRecords++
                            } else {
                                if ($observation.segment_count -ne $count -or $observation.canonical_byte_count -ne $encodedBytes -or
                                    $observation.changed_segments -ne $changed -or $observation.text_route_changed_segments -ne $textChanged -or
                                    $observation.unsupported_segments -ne $unsupported -or $observation.canonical_sha256 -cne $wholeHash -or
                                    $observation.whole_record_exact -ne $wholeExact -or $observation.ordered_encoding_sha256 -cne $encodingHash) {throw 'Independent reencoding differs'}
                                $reencodedRecordsVerified++; $reencodedBytesVerified+=$encodedBytes; $reencodedChangedSegments+=$changed
                                $reencodedTextChanged+=$textChanged; $reencodedUnsupported+=$unsupported
                                if ($wholeExact) {$reencodedWholeExact++}
                            }
                        }
                    }
                    if ($table.system_decoder_tokens) {
                        if ($tableTokenFailures -ne $table.system_decoder_tokens.failed_records) {throw 'Token failure count mismatch'}
                        for ($opcode=0; $opcode -lt 256; $opcode++) {
                            if ($tokenCounts[$opcode] -ne $table.system_decoder_tokens.opcode_counts_for_successful_records[$opcode]) {
                                throw 'Independent opcode count mismatch'
                            }
                        }
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
        if ($report.all_declared_replacements_requested -and @($sample.replacement_samples).Count -ne $replacementCount) {
            throw 'Complete replacement observation omits declared entries'
        }
        $replacementOrdinal = 0
        foreach ($replacement in $sample.replacement_samples) {
            $meta = $replacement.metadata
            if ($meta.ordinal -ne $replacementOrdinal) {throw 'Replacement observations are not in contiguous source order'}
            $replacementOrdinal++
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
            if ($replacement.inspection_prefix_hex -and
                [Convert]::ToHexString([byte[]]$replacementData[0..([Math]::Min(128,$replacementData.Length)-1)]) -ne $replacement.inspection_prefix_hex) {
                throw 'Independent replacement inspection prefix mismatch'
            }
            if ($ReplacementPreviewDirectory -and $replacementData.Length -ge 33 -and
                [Convert]::ToHexString([byte[]]$replacementData[0..7]) -eq '89504E470D0A1A0A') {
                # Windows' existing PNG decoder creates a derived preview only.
                # This is independent asset inspection, not a Rust/renderer oracle.
                if ([Convert]::ToHexString([byte[]]$replacementData[8..15]) -ne '0000000D49484452') {throw 'Invalid PNG IHDR prefix'}
                $width = [uint32]::Parse([Convert]::ToHexString([byte[]]$replacementData[16..19]),[Globalization.NumberStyles]::HexNumber)
                $height = [uint32]::Parse([Convert]::ToHexString([byte[]]$replacementData[20..23]),[Globalization.NumberStyles]::HexNumber)
                if ($width -eq 0 -or $height -eq 0 -or $width -gt 4096 -or $height -gt 4096 -or
                    $previewPixels + [long]$width * $height -gt 16777216 -or $pngPreviews -ge 64) {throw 'PNG preview dimensions/count exceed inspection limit'}
                $previewPixels += [long]$width * $height
                $pngStream = [IO.MemoryStream]::new($replacementData,$false)
                $bitmap = $null
                $composite = $null
                $graphics = $null
                $previewFile = $null
                try {
                    $bitmap = [Drawing.Bitmap]::new($pngStream)
                    if ($bitmap.Width -ne $width -or $bitmap.Height -ne $height) {throw 'PNG decoded/header dimensions differ'}
                    # BMP viewers can ignore source alpha. Composite explicitly
                    # against black to inspect the decoded PNG's visible content.
                    $composite = [Drawing.Bitmap]::new([int]$width,[int]$height,[Drawing.Imaging.PixelFormat]::Format24bppRgb)
                    $graphics = [Drawing.Graphics]::FromImage($composite)
                    $graphics.Clear([Drawing.Color]::Black)
                    $graphics.DrawImageUnscaled($bitmap,0,0)
                    # Numeric IDs prevent source names becoming output paths.
                    $previewName = 'sample-{0:D2}-replacement-{1:D2}.bmp' -f $report.samples.IndexOf($sample),[int]$meta.ordinal
                    $previewFile = [IO.File]::Open((Join-Path $ReplacementPreviewDirectory $previewName),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
                    $composite.Save($previewFile,[Drawing.Imaging.ImageFormat]::Bmp)
                    $previewFile.Flush($true)
                    $pngPreviews++
                } finally {
                    if ($previewFile) {$previewFile.Dispose()}
                    if ($graphics) {$graphics.Dispose()}
                    if ($composite) {$composite.Dispose()}
                    if ($bitmap) {$bitmap.Dispose()}
                    $pngStream.Dispose()
                }
            }
            $replacementBytes += $replacementData.Length
            $replacementVerified++
            $replacementStart += $replacementPadded
        }
    } finally { $hed.Dispose(); $pkg.Dispose() }
}
"PASS: $($report.samples.Count) originals / $totalBytes bytes and $replacementVerified replacements / $replacementBytes bytes agree with pinned OpenKH transform and .NET deflate; source was only read."
if ($ReplacementPreviewDirectory) {"PASS: $pngPreviews independently decoded PNG previews / $previewPixels pixels saved privately; DDS pixels and installed rendering are unqualified."}
if ($messageMode) { "PASS: $barRecordsVerified BAR records and $messageRecordsVerified message records / $messageEncodedBytes encoded bytes match independent scalars and pinned OpenKH Msg.Read; no glyph/Unicode mapping asserted." }
if ($messageMode) { "PASS: $fontImagesVerified font palette planes / $fontPixelsVerified pixels match pinned RawBitmap packed pixels and palettes plus independent RGBA expansion; $spacingTablesVerified spacing byte tables match. Runtime glyph mapping/rendering remains unverified." }
if ($tokenMode) { "PASS: 256 generated opcode arities; $tokenRecordsVerified records / $systemTokensVerified system tokens match pinned decoder-model boundaries and opcode counts; $tokenFailuresVerified record failures agree. No Unicode/runtime semantics asserted." }
if ($textRecordsVerified -or $unmappedTextRecords) { "PASS: $textRecordsVerified explicit International-profile records / $unicodeCharactersVerified Unicode scalars / $complexSymbolsVerified complex symbols / $unsupportedTextSegmentsVerified retained unsupported segments match unchanged decoder content. NOT QUALIFIED: $unmappedTextRecords generic-table records; runtime language/rendering remains unverified." }
if ($reencodingMode) { "PASS: $reencodedRecordsVerified records / $reencodedBytesVerified canonical bytes / $reencodedWholeExact exact whole records match unchanged public encoding; $reencodedChangedSegments changed segments ($reencodedTextChanged text-route changes), $reencodedUnsupported unsupported slots retained. NOT QUALIFIED: $reencodedUnmappedRecords unmapped records; installed rendering/command semantics remain unverified." }
