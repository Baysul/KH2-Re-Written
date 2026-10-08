# Source read-only. Execute exact pinned draw methods in an observation adapter;
# compare selected originals with an independently verified parent font report.
param(
    [Parameter(Mandatory)][string]$InstallRoot,
    [Parameter(Mandatory)][string]$ProbePath,
    [Parameter(Mandatory)][string]$ParentProbePath,
    [Parameter(Mandatory)][string]$OpenKhRoot
)
$ErrorActionPreference = 'Stop'
$pin = '7a3b945c538d32c6a285128c98aefba093f52ceb'
if ((git -C $OpenKhRoot rev-parse HEAD) -ne $pin) { throw 'Unaudited OpenKH pin' }
$primary = @('OpenKh.Kh2/Constants.cs','OpenKh.Engine/Renders/Kh2MessageRenderer.cs')
if (git -C $OpenKhRoot status --porcelain -- $primary) { throw 'Draw reference is modified' }
$cellReport = Get-Content -LiteralPath $ProbePath -Raw | ConvertFrom-Json
$parentReport = Get-Content -LiteralPath $ParentProbePath -Raw | ConvertFrom-Json
if ($cellReport.schema_version -ne 1 -or
    $cellReport.qualification -cne 'explicit_font_cell_candidate_inspection_only' -or
    $cellReport.source_content_identity -cne $parentReport.source_content_identity -or
    $cellReport.pairs.Count -lt 1 -or $cellReport.pairs.Count -gt 32) { throw 'Invalid cell report/parent binding' }
# Own script scope; compiled unchanged reference types are reusable below.
& (Join-Path $PSScriptRoot 'Verify-OriginalSamples.ps1') -InstallRoot $InstallRoot -ProbePath $ParentProbePath -OpenKhRoot $OpenKhRoot

$renderer = Get-Content -LiteralPath (Join-Path $OpenKhRoot $primary[1]) -Raw
$charMethods = [regex]::Matches($renderer, '(?s)protected void DrawChar\(.*?\r?\n        \}')
$iconIndex = [regex]::Match($renderer, '(?s)private void DrawIcon\(DrawContext context, byte index\).*?\r?\n        \}')
$iconCoordinates = [regex]::Match($renderer, '(?s)protected void DrawIcon\(DrawContext context, int sourceX, int sourceY\).*?;')
if ($charMethods.Count -ne 2 -or !$iconIndex.Success -or !$iconCoordinates.Success) { throw 'Pinned draw methods not found' }
$adapterStart = @'
namespace RewriteFontOracle {
    public interface ISpriteTexture { int Lane { get; } }
    public class Texture : ISpriteTexture { public int Lane {get;set;} }
    public class Size { public int Width; }
    public class Image { public Size Size = new Size(); }
    public class RenderingMessageContext { public Image Font; public int FontWidth, FontHeight, TableHeight; }
    public class DrawContext { public double x, y; }
    public struct ColorF { public ColorF(float r, float g, float b, float a) {} }
    public class Observation { public int Lane, X, Y, Width, Height; }
    public class Projector {
        const int IconWidth = OpenKh.Kh2.Constants.FontIconWidth;
        const int IconHeight = OpenKh.Kh2.Constants.FontIconHeight;
        RenderingMessageContext _msgContext;
        int _charPerRow, _charTableHeight, _tableHeight, _iconPerRow;
        ISpriteTexture _spriteFont = new Texture {Lane=0};
        ISpriteTexture _spriteFont2 = new Texture {Lane=1};
        ISpriteTexture _spriteIcon = new Texture {Lane=0};
        byte[] _iconSpacing;
        bool _icon;
        Observation _result;
        public Projector(string profile, int atlasWidth, byte[] spacing) {
            var context = new RenderingMessageContext { Font = new Image() };
            context.Font.Size.Width = atlasWidth;
            switch (profile) {
                case "international_system":
                    context.FontWidth = OpenKh.Kh2.Constants.FontEuropeanSystemWidth;
                    context.FontHeight = OpenKh.Kh2.Constants.FontEuropeanSystemHeight;
                    context.TableHeight = OpenKh.Kh2.Constants.FontTableSystemHeight; break;
                case "international_event":
                    context.FontWidth = OpenKh.Kh2.Constants.FontEuropeanEventWidth;
                    context.FontHeight = OpenKh.Kh2.Constants.FontEuropeanEventHeight;
                    context.TableHeight = OpenKh.Kh2.Constants.FontTableEventHeight; break;
                case "japanese_system":
                    context.FontWidth = OpenKh.Kh2.Constants.FontJapaneseSystemWidth;
                    context.FontHeight = OpenKh.Kh2.Constants.FontJapaneseSystemHeight;
                    context.TableHeight = OpenKh.Kh2.Constants.FontTableSystemHeight; break;
                case "japanese_event":
                    context.FontWidth = OpenKh.Kh2.Constants.FontJapaneseEventWidth;
                    context.FontHeight = OpenKh.Kh2.Constants.FontJapaneseEventHeight;
                    context.TableHeight = OpenKh.Kh2.Constants.FontTableEventHeight; break;
                case "icon": _icon=true; context.FontWidth=24; context.FontHeight=24; context.TableHeight=160; break;
                default: throw new System.IO.InvalidDataException("Profile");
            }
            _msgContext = context; _iconSpacing = spacing;
            _iconPerRow = atlasWidth / IconWidth;
'@
# Preserve these exact constructor statements as well as the method bodies.
$initializers = foreach ($field in @('_charPerRow','_tableHeight','_charTableHeight')) {
    $match = [regex]::Match($renderer, [regex]::Escape($field) + ' = [^;]+;')
    if (!$match.Success) { throw 'Draw initializer missing' }
    $match.Value
}
$adapterMiddle = @'
        }
        public Observation Project(int index) {
            _result=null;
            if (_icon) DrawIcon(new DrawContext(), checked((byte)index));
            else DrawChar(new DrawContext(),index);
            return _result;
        }
        public int NonAliasedIndexLimit {
            get { return _icon ? 60 : 4*_charPerRow*(_charTableHeight/_msgContext.FontHeight); }
        }
        void DrawImageScale(DrawContext context, ISpriteTexture texture, int x, int y, int width, int height) {
            _result = new Observation {Lane=texture.Lane, X=x, Y=y, Width=width, Height=height};
        }
        void DrawImage(ISpriteTexture texture, double dx, double dy, int x, int y, int width, int height, double sx, double sy, ColorF color) {
            _result = new Observation {Lane=texture.Lane, X=x, Y=y, Width=width, Height=height};
        }
'@
$adapterEnd = @'
    }
    public static class Pixels {
        public static byte[] Expand(byte[] packed, byte[] clut, int pixels, bool icon) {
            var rgba = new byte[checked(pixels*4)];
            for (int p=0; p<pixels; p++) {
                int color = icon ? packed[p] : ((p&1)==0 ? packed[p/2]>>4 : packed[p/2]&15);
                System.Array.Copy(clut, color*4, rgba, p*4, 4);
            }
            return rgba;
        }
        public static string CropHash(byte[] rgba, int atlasWidth, int atlasHeight, Observation cell) {
            if (cell.X<0 || cell.Y<0 || cell.Width<1 || cell.Height<1 ||
                cell.X+cell.Width>atlasWidth || cell.Y+cell.Height>atlasHeight) return null;
            var crop = new byte[checked(cell.Width*cell.Height*4)];
            for (int y=0; y<cell.Height; y++)
                System.Array.Copy(rgba, ((cell.Y+y)*atlasWidth+cell.X)*4, crop, y*cell.Width*4, cell.Width*4);
            return System.Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(crop)).ToLowerInvariant();
        }
    }
}
'@
$constants = Get-Content -LiteralPath (Join-Path $OpenKhRoot $primary[0]) -Raw
Add-Type -TypeDefinition ($constants + $adapterStart + ($initializers -join "`n") + $adapterMiddle +
    (($charMethods | ForEach-Object {$_.Value}) -join "`n") + $iconIndex.Value + $iconCoordinates.Value + $adapterEnd)

function Get-ReferenceCellPayload($Metadata) {
    $selected = $Metadata.selection
    $parents = @($parentReport.samples | Where-Object {$_.index -ceq $selected.index -and $_.ordinal -eq $selected.ordinal})
    if ($parents.Count -ne 1) { throw 'Source selection absent/ambiguous in verified parent' }
    $parent = $parents[0]
    if ([Convert]::ToHexString([byte[]]$parent.name_hash) -cne [Convert]::ToHexString([byte[]]$selected.name_hash) -or
        $Metadata.original_sha256 -cne $parent.sha256 -or $Metadata.original_size -ne $parent.original_size -or
        $Metadata.package_offset -ne $parent.package_offset -or $Metadata.compression_tag -ne $parent.compression_tag -or
        $Metadata.replacement_count -ne $parent.replacement_count -or
        ($Metadata.bar | ConvertTo-Json -Depth 8 -Compress) -cne ($parent.bar | ConvertTo-Json -Depth 8 -Compress)) { throw 'Cell source metadata differs from verified parent' }
    $hed = [IO.BinaryReader]::new([IO.File]::OpenRead((Join-Path $InstallRoot $selected.index)))
    $pkg = [IO.BinaryReader]::new([IO.File]::OpenRead([IO.Path]::ChangeExtension((Join-Path $InstallRoot $selected.index),'pkg')))
    try {
        $hed.BaseStream.Position = 32L * $selected.ordinal
        $hash = $hed.ReadBytes(16); $offset = $hed.ReadInt64(); $indexedSize = $hed.ReadInt32()
        if ($offset -ne $parent.package_offset -or [Convert]::ToHexString($hash) -cne [Convert]::ToHexString([byte[]]$selected.name_hash)) { throw 'HED binding changed' }
        $pkg.BaseStream.Position = $offset
        $seed = $pkg.ReadBytes(16)
        $expected = [BitConverter]::ToInt32($seed,0); $replacements = [BitConverter]::ToInt32($seed,4); $tag = [BitConverter]::ToInt32($seed,8)
        $length = if ($tag -ge 0) {$tag} else {$expected}
        if ($expected -lt 0 -or $expected -gt 33554432 -or $length -lt 0 -or $length -gt 33554432 -or $tag -lt -2 -or
            $replacements -lt 0 -or 16L+48L*$replacements+$length -gt $indexedSize -or $offset+$indexedSize -gt $pkg.BaseStream.Length) { throw 'Original range/limit' }
        $pkg.BaseStream.Position = $offset+16L+48L*$replacements
        $data = $pkg.ReadBytes($length)
        if ($data.Length -ne $length) { throw 'Truncated original' }
        if ($tag -gt -2) {
            $key = [OpenKh.Egs.EgsEncryption]::GenerateKey($seed,10)
            for ($chunk=0; $chunk -lt [Math]::Min($length,256); $chunk+=16) { [OpenKh.Egs.EgsEncryption]::DecryptChunk($key,$data,$chunk,10) }
        }
        if ($tag -ge 0) {
            $packed = [IO.MemoryStream]::new($data,$false); $packed.Position=2
            $deflate = [IO.Compression.DeflateStream]::new($packed,[IO.Compression.CompressionMode]::Decompress)
            $decoded = [IO.MemoryStream]::new()
            try {
                $buffer = [byte[]]::new(65536)
                while (($read=$deflate.Read($buffer,0,$buffer.Length)) -gt 0) {
                    if ($decoded.Length+$read -gt $expected) { throw 'Decoded output exceeds header' }
                    $decoded.Write($buffer,0,$read)
                }
                $data = $decoded.ToArray()
            } finally { $decoded.Dispose(); $deflate.Dispose(); $packed.Dispose() }
        }
        if ($data.Length -ne $expected -or $expected -ne $parent.original_size -or $tag -ne $parent.compression_tag -or
            $replacements -ne $parent.replacement_count -or
            [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($data)) -ne $parent.sha256) { throw 'Independent original differs' }
        $child = $parent.bar.entries[$selected.bar_ordinal]
        if ($null -eq $child -or $child.offset -lt 0) { throw 'Invalid BAR selection' }
        $payload = [byte[]]::new($child.size)
        [Array]::Copy($data,$child.offset,$payload,0,$child.size)
        if ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($payload)) -ne $Metadata.payload_sha256) { throw 'Independent BAR payload differs' }
        return ,$payload
    } finally { $hed.Dispose(); $pkg.Dispose() }
}

$verified=0; $failed=0; $pixelsVerified=0L
foreach ($pair in $cellReport.pairs) {
    $profile = $pair.request.profile
    foreach ($role in @('image','spacing')) {
        $sourceMetadata = $pair."${role}_source"
        if (($pair.request.$role | ConvertTo-Json -Compress) -cne ($sourceMetadata.selection | ConvertTo-Json -Compress)) { throw 'Requested source differs from observed source' }
        $child = $sourceMetadata.bar.entries[$sourceMetadata.selection.bar_ordinal]
        $expectedSlot = if ($profile -ceq 'icon') {'icon'} elseif ($profile.EndsWith('_system')) {'sys'} else {'evt'}
        $expectedType = if ($role -ceq 'image') {36} else {2}
        if ($child.entry_type -ne $expectedType -or [Text.Encoding]::ASCII.GetString([byte[]]$child.name_bytes).TrimEnd([char]0) -cne $expectedSlot) { throw 'Profile/slot/type mismatch' }
    }
    $imageData = Get-ReferenceCellPayload $pair.image_source
    $spacing = Get-ReferenceCellPayload $pair.spacing_source
    if ($pair.spacing_count -ne $spacing.Length -or $pair.cells.Count -ne $spacing.Length) { throw 'Cell denominator differs' }
    $icon = $profile -ceq 'icon'
    $width = if ($icon) {256} else {512}
    $height = if ($icon) {160} elseif ($imageData.Length -lt 131072) {256} elseif ($imageData.Length -lt 262144) {512} else {1024}
    if ($pair.atlas_width -ne $width -or $pair.atlas_height -ne $height) { throw 'Atlas dimensions differ' }
    $planes = @()
    foreach ($lane in $(if ($icon) {@(0)} else {@(0,1)})) {
        $stream = [IO.MemoryStream]::new($imageData,$false)
        try {
            $image = if ($icon) {[OpenKh.Kh2.RawBitmap]::Read8bit($stream,$width,$height)}
                elseif ($lane -eq 0) {[OpenKh.Kh2.RawBitmap]::Read4bitPalette1($stream,$width,$height)}
                else {[OpenKh.Kh2.RawBitmap]::Read4bitPalette2($stream,$width,$height)}
            $planes += ,([RewriteFontOracle.Pixels]::Expand($image.GetData(),$image.GetClut(),$width*$height,$icon))
        } finally { $stream.Dispose() }
    }
    $projector = [RewriteFontOracle.Projector]::new($profile,$width,$spacing)
    $pairFailed=0
    for ($index=0; $index -lt $spacing.Length; $index++) {
        $expectedCell = $pair.cells[$index]
        if ($expectedCell.index -ne $index) { throw 'Cell order differs' }
        # These are our admission guards: upstream masks banks/substitutes icons.
        if ($index -ge $projector.NonAliasedIndexLimit) {
            if ($expectedCell.status -cne 'FAIL') { throw 'Aliased bank/icon not retained as failed' }
            $pairFailed++; $failed++; continue
        }
        $actual = $projector.Project($index)
        $cropHash = [RewriteFontOracle.Pixels]::CropHash($planes[$actual.Lane],$width,$height,$actual)
        if ($null -eq $cropHash) {
            if ($expectedCell.status -cne 'FAIL') { throw 'Out-of-atlas cell not retained as failed' }
            $pairFailed++; $failed++; continue
        }
        $cell = $expectedCell.cell
        if ($expectedCell.status -cne 'PASS' -or $cell.index -ne $index -or $cell.palette_lane -ne $actual.Lane -or
            $cell.x -ne $actual.X -or $cell.y -ne $actual.Y -or $cell.width -ne $actual.Width -or $cell.height -ne $actual.Height -or
            $cell.spacing_byte -ne $spacing[$index] -or $expectedCell.pixels_rgba_sha256 -cne $cropHash) { throw 'Independent draw/crop/spacing differs' }
        $verified++; $pixelsVerified+=$actual.Width*$actual.Height
    }
    if ($pair.failed_cells -ne $pairFailed) { throw 'Failed-cell denominator differs' }
}
# Generated controls establish why bounds differ from permissive public drawing.
$fixtureSpacing = [byte[]]::new(2048)
$syntheticText = [RewriteFontOracle.Projector]::new('japanese_system',512,$fixtureSpacing)
$zero = $syntheticText.Project(0); $alias = $syntheticText.Project(1568)
if ($zero.Lane -ne $alias.Lane -or $zero.X -ne $alias.X -or $zero.Y -ne $alias.Y) { throw 'Pinned bank-alias observation changed' }
$syntheticIcon = [RewriteFontOracle.Projector]::new('icon',256,$fixtureSpacing)
$eleven = $syntheticIcon.Project(11); $replacement = $syntheticIcon.Project(60)
if ($eleven.X -ne $replacement.X -or $eleven.Y -ne $replacement.Y) { throw 'Pinned icon-workaround observation changed' }
"PASS: generated bank alias and icon substitution observed in exact public methods; Rust guards reject these instead of claiming source behavior."
"PASS: $($cellReport.pairs.Count) explicit pairs; $verified cells / $pixelsVerified cropped pixels; $failed failed cells retained. Runtime pairing, glyph meanings and spacing units unqualified."
