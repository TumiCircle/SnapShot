# Generate PixelSnap camera icons
Add-Type -AssemblyName System.Drawing

function New-CameraIcon {
    param([int]$Size)

    # 32x32 pixel art - matches approved SVG logo exactly
    # C=mint body H=mint highlight D=mint shadow
    # M=pink lens P=pink hl Q=pink shadow
    # Y=yellow lens L=yellow hl K=yellow shadow
    # W=white highlight, R=red shutter
    $cam = @(
        "................................",
        "................................",
        "................................",
        "................................",
        "..........CHHHHHHHHHDC..........",
        ".........CHCCCCCCCCCCH..........",
        "........CC............DC........",
        ".......CHCMMMMMMMMMMMMCDC.......",
        "......CCCHMPPPPPPPPPPMCCDC......",
        ".....CCCMMM..........MMMCDC.....",
        "....CCCMMMYYYYYYYYYYYMMMCDC.....",
        "....CCCMMMYLLLLLLLLKYYMMMCDC....",
        "....CCCMMMYLWWWWWLKYYMMMCDC....",
        "....CCCMMMYKKKKKKKKKYYMMMCDC....",
        "....CCCMMMYYYYYYYYYYYMMMCDC.....",
        ".....CCCMMM..........MMMCDC.....",
        "......CCCHMPPPPPPPPPPMCCDC......",
        ".......CHCMMMMMMMMMMMMCDC.......",
        "........CCRR............DC......",
        "........CCCCCCCCCCCCCCCC........",
        "........CDDDDDDDDDDDDDDD........",
        ".......CCDDDD.......DDDDCC......",
        "......CCDDDDD.......DDDDDCC.....",
        "................................",
        "................................",
        "................................",
        "................................",
        "................................",
        "................................",
        "................................",
        "................................",
        "................................"
    )

    # Verify all lines are 32 chars (disabled for speed)
    # for ($i = 0; $i -lt 32; $i++) {
    #     if ($cam[$i].Length -ne 32) { Write-Host "Line $i length: $($cam[$i].Length)" }
    # }

    $bmp = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
    $g.Clear([System.Drawing.Color]::Transparent)

    $pixelSize = [math]::Floor($Size / 32)
    $offX = [math]::Floor(($Size - $pixelSize * 32) / 2)
    $offY = [math]::Floor(($Size - $pixelSize * 32) / 2)

    $colors = @{
        'C' = [System.Drawing.Color]::FromArgb(255,127,255,212)
        'H' = [System.Drawing.Color]::FromArgb(255,184,255,232)
        'D' = [System.Drawing.Color]::FromArgb(255,45,168,138)
        'M' = [System.Drawing.Color]::FromArgb(255,255,110,180)
        'P' = [System.Drawing.Color]::FromArgb(255,255,157,207)
        'Y' = [System.Drawing.Color]::FromArgb(255,255,238,120)
        'L' = [System.Drawing.Color]::FromArgb(255,255,246,176)
        'K' = [System.Drawing.Color]::FromArgb(255,197,168,32)
        'W' = [System.Drawing.Color]::FromArgb(255,255,255,255)
        'R' = [System.Drawing.Color]::FromArgb(255,255,77,109)
    }
    $brushes = @{}
    foreach ($k in $colors.Keys) { $brushes[$k] = New-Object System.Drawing.SolidBrush($colors[$k]) }

    for ($y = 0; $y -lt 32; $y++) {
        $line = $cam[$y]
        for ($x = 0; $x -lt 32; $x++) {
            $ch = [string]$line[$x]
            if ($ch -eq '.') { continue }
            if ($brushes.ContainsKey($ch)) {
                $g.FillRectangle($brushes[$ch], $offX + $x*$pixelSize, $offY + $y*$pixelSize, $pixelSize, $pixelSize)
            }
        }
    }
    foreach ($b in $brushes.Values) { $b.Dispose() }
    $g.Dispose()
    return $bmp
}

$iconsDir = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "icons"

# Generate PNGs
foreach ($sz in @(32,128,256)) {
    $bmp = New-CameraIcon $sz
    $name = if ($sz -eq 256) { "128x128@2x.png" } else { "${sz}x${sz}.png" }
    $bmp.Save((Join-Path $iconsDir $name), [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Host "Saved $name"
    $bmp.Dispose()
}

# Generate ICO with embedded PNGs
$iconPath = Join-Path $iconsDir "icon.ico"
$fs = [System.IO.File]::Create($iconPath)
$bw = New-Object System.IO.BinaryWriter($fs)

$icoSizes = @(16,32,48,64,128,256)
$pngChunks = @{}
foreach ($sz in $icoSizes) {
    $bmp = New-CameraIcon $sz
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $pngChunks[$sz] = $ms.ToArray()
    $ms.Dispose(); $bmp.Dispose()
}

$bw.Write([uint16]0)
$bw.Write([uint16]1)
$bw.Write([uint16]$icoSizes.Count)

$offset = 6 + 16 * $icoSizes.Count
foreach ($sz in $icoSizes) {
    $d = $pngChunks[$sz]
    $bw.Write([byte]$(if($sz -ge 256){0}else{$sz}))
    $bw.Write([byte]$(if($sz -ge 256){0}else{$sz}))
    $bw.Write([byte]0)
    $bw.Write([byte]0)
    $bw.Write([uint16]1)
    $bw.Write([uint16]32)
    $bw.Write([uint32]$d.Length)
    $bw.Write([uint32]$offset)
    $offset += $d.Length
}
foreach ($sz in $icoSizes) { $bw.Write($pngChunks[$sz]) }

$bw.Dispose(); $fs.Dispose()
Write-Host "Saved icon.ico"
