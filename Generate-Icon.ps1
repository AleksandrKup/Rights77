$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$assetDirectory = Join-Path $PSScriptRoot 'assets'
New-Item -ItemType Directory -Path $assetDirectory -Force | Out-Null
$bitmap = [Drawing.Bitmap]::new(512, 512)
$graphics = [Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.TextRenderingHint = [Drawing.Text.TextRenderingHint]::AntiAliasGridFit
$graphics.Clear([Drawing.Color]::Transparent)
$shieldPath = [Drawing.Drawing2D.GraphicsPath]::new()
$shieldPath.AddLines([Drawing.PointF[]]@(
    [Drawing.PointF]::new(256, 20),
    [Drawing.PointF]::new(462, 96),
    [Drawing.PointF]::new(440, 292),
    [Drawing.PointF]::new(374, 405),
    [Drawing.PointF]::new(256, 492),
    [Drawing.PointF]::new(138, 405),
    [Drawing.PointF]::new(72, 292),
    [Drawing.PointF]::new(50, 96)
))
$shieldPath.CloseFigure()
$shieldBrush = [Drawing.Drawing2D.LinearGradientBrush]::new([Drawing.Point]::new(90, 40), [Drawing.Point]::new(420, 470), [Drawing.Color]::FromArgb(36, 74, 108), [Drawing.Color]::FromArgb(12, 26, 45))
$graphics.FillPath($shieldBrush, $shieldPath)
$outlineColor = [Drawing.Color]::FromArgb(83, 218, 217)
$shieldOutlinePen = [Drawing.Pen]::new($outlineColor, 13)
$graphics.DrawPath($shieldOutlinePen, $shieldPath)
$databaseBodyBrush = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(35, 122, 153))
$databaseTopBrush = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(65, 181, 202))
$graphics.FillRectangle($databaseBodyBrush, 128, 131, 224, 143)
$graphics.FillEllipse($databaseBodyBrush, 128, 240, 224, 68)
$graphics.FillEllipse($databaseTopBrush, 128, 97, 224, 68)
$databaseOutlinePen = [Drawing.Pen]::new($outlineColor, 8)
$graphics.DrawArc($databaseOutlinePen, 128, 151, 224, 68, 0, 180)
$graphics.DrawArc($databaseOutlinePen, 128, 202, 224, 68, 0, 180)
$graphics.DrawArc($databaseOutlinePen, 128, 241, 224, 68, 0, 180)
$keyBrush = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(255, 200, 89))
$keyHoleBrush = [Drawing.SolidBrush]::new([Drawing.Color]::FromArgb(18, 40, 59))
$graphics.FillEllipse($keyBrush, 278, 180, 100, 100)
$graphics.FillEllipse($keyHoleBrush, 306, 208, 44, 44)
$graphics.FillRectangle($keyBrush, 348, 215, 94, 30)
$graphics.FillRectangle($keyBrush, 402, 231, 23, 40)
$graphics.FillRectangle($keyBrush, 430, 231, 20, 28)
$font = [Drawing.Font]::new('Segoe UI', 94, [Drawing.FontStyle]::Bold, [Drawing.GraphicsUnit]::Pixel)
$stringFormat = [Drawing.StringFormat]::new()
$stringFormat.Alignment = [Drawing.StringAlignment]::Center
$graphics.DrawString('77', $font, [Drawing.Brushes]::White, [Drawing.RectangleF]::new(130, 310, 252, 135), $stringFormat)
$bitmap.Save((Join-Path $assetDirectory 'rights77.png'), [Drawing.Imaging.ImageFormat]::Png)
$sizes = @(16, 24, 32, 48, 64, 128, 256)
$iconImages = @()
foreach ($size in $sizes) {
    $scaledBitmap = [Drawing.Bitmap]::new($size, $size)
    $scaledGraphics = [Drawing.Graphics]::FromImage($scaledBitmap)
    $scaledGraphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $scaledGraphics.DrawImage($bitmap, 0, 0, $size, $size)
    $imageStream = [IO.MemoryStream]::new()
    $scaledBitmap.Save($imageStream, [Drawing.Imaging.ImageFormat]::Png)
    $iconImages += , @{
        Size = $size
        Bytes = $imageStream.ToArray()
    }
    $imageStream.Dispose()
    $scaledGraphics.Dispose()
    $scaledBitmap.Dispose()
}
$iconStream = [IO.File]::Create((Join-Path $assetDirectory 'rights77.ico'))
$iconWriter = [IO.BinaryWriter]::new($iconStream)
$iconWriter.Write([uint16]0)
$iconWriter.Write([uint16]1)
$iconWriter.Write([uint16]$iconImages.Count)
# ICO header is 6 bytes, followed by a 16-byte directory entry per image.
$offset = 6 + 16 * $iconImages.Count
foreach ($item in $iconImages) {
    # ICO represents a 256-pixel dimension with a zero byte.
    $dimension = if ($item.Size -eq 256) { 0 } else { $item.Size }
    $iconWriter.Write([byte]$dimension)
    $iconWriter.Write([byte]$dimension)
    $iconWriter.Write([byte]0)
    $iconWriter.Write([byte]0)
    $iconWriter.Write([uint16]1)
    $iconWriter.Write([uint16]32)
    $iconWriter.Write([uint32]$item.Bytes.Length)
    $iconWriter.Write([uint32]$offset)
    $offset += $item.Bytes.Length
}
foreach ($item in $iconImages) {
    $iconWriter.Write([byte[]]$item.Bytes)
}
$iconWriter.Dispose()
$iconStream.Dispose()
$font.Dispose()
$stringFormat.Dispose()
$databaseBodyBrush.Dispose()
$databaseTopBrush.Dispose()
$keyBrush.Dispose()
$keyHoleBrush.Dispose()
$databaseOutlinePen.Dispose()
$shieldOutlinePen.Dispose()
$shieldBrush.Dispose()
$shieldPath.Dispose()
$graphics.Dispose()
$bitmap.Dispose()
