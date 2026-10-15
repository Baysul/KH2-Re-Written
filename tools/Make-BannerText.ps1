# Renders the "Final Re:WRITE" banner text (source/visual/assets/
# final-rewrite-text.png) from a Windows system font in the gold bevel
# colours measured on the title logo banner. Reads no game data; the
# ornaments and lines around it come from the install at runtime
# (main::banner). Downloads/installs nothing.
param(
    [string]$Font = "Bahnschrift",
    [string]$Text = "Final Re:WRITE",
    [int]$Width = 306,
    [Parameter(Mandatory)][string]$Out
)
Add-Type -AssemblyName System.Drawing
$D = [System.Drawing.Drawing2D.Matrix]
$H = 32            # canvas height
$capTop = 2        # first row of glyph fill
$capH = 26         # glyph fill height (cap height)
$W = $Width + 8
$S = 4             # supersampling
$bmp = New-Object System.Drawing.Bitmap ($W * $S), ($H * $S), ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.Clear([System.Drawing.Color]::Transparent)
$fam = New-Object System.Drawing.FontFamily $Font
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$path.AddString($Text, $fam, [int][System.Drawing.FontStyle]::Bold, 100, (New-Object System.Drawing.PointF 0, 0), [System.Drawing.StringFormat]::GenericTypographic)
# Cap height from the "F"
$pf = New-Object System.Drawing.Drawing2D.GraphicsPath
$pf.AddString("F", $fam, [int][System.Drawing.FontStyle]::Bold, 100, (New-Object System.Drawing.PointF 0, 0), [System.Drawing.StringFormat]::GenericTypographic)
$fb = $pf.GetBounds()
$b = $path.GetBounds()
$sy = ($capH * $S) / $fb.Height
$sx = ($Width * $S) / $b.Width
$m = New-Object System.Drawing.Drawing2D.Matrix
$m.Translate(4 * $S, $capTop * $S)
$m.Scale($sx, $sy)
$m.Translate(-$b.X, -$fb.Y)
$path.Transform($m)
$path.FillMode = [System.Drawing.Drawing2D.FillMode]::Winding; $pb = $path.GetBounds()
# halo, outline, then gradient fill
$halo = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(150, 0, 0, 0)), (3.2 * $S)
$halo.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
$g.DrawPath($halo, $path)
$edge = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 30, 28, 21)), (2.0 * $S)
$edge.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
$g.DrawPath($edge, $path)
$top = ($capTop * $S)
$rect = New-Object System.Drawing.RectangleF 0, $top, ($W * $S), ($capH * $S)
$brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, ([System.Drawing.Color]::Black), ([System.Drawing.Color]::White), 90.0
$blend = New-Object System.Drawing.Drawing2D.ColorBlend 7
$blend.Colors = @(
    [System.Drawing.Color]::FromArgb(255, 87, 79, 56),
    [System.Drawing.Color]::FromArgb(255, 115, 105, 73),
    [System.Drawing.Color]::FromArgb(255, 149, 141, 107),
    [System.Drawing.Color]::FromArgb(255, 200, 195, 155),
    [System.Drawing.Color]::FromArgb(255, 197, 192, 153),
    [System.Drawing.Color]::FromArgb(255, 211, 206, 168),
    [System.Drawing.Color]::FromArgb(255, 207, 202, 162))
$blend.Positions = [single[]]@(0.0, 0.12, 0.38, 0.76, 0.84, 0.92, 1.0)
$brush.InterpolationColors = $blend
$g.FillPath($brush, $path)
$g.Dispose()
$small = New-Object System.Drawing.Bitmap $W, $H, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g2 = [System.Drawing.Graphics]::FromImage($small)
$g2.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g2.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
$g2.Clear([System.Drawing.Color]::Transparent)
$g2.DrawImage($bmp, (New-Object System.Drawing.Rectangle 0, 0, $W, $H))
$g2.Dispose()
$small.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
"wrote $Out ${W}x$H"
