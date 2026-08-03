fn main() {
    tauri_build::build();
    let _ = generate_icons();
}

#[derive(Clone, Copy)]
struct Color(u8, u8, u8, u8);

const MINT: Color = Color(127, 255, 212, 255);       // #7fffd4 body base
const MINT_LIGHT: Color = Color(184, 255, 232, 255); // #b8ffe8 highlight
const MINT_DARK: Color = Color(45, 168, 138, 255);   // #2da88a shadow
const PINK: Color = Color(255, 110, 180, 255);       // #ff6eb4 lens ring base
const PINK_LIGHT: Color = Color(255, 157, 207, 255); // #ff9dcf highlight
const PINK_DARK: Color = Color(184, 61, 122, 255);   // #b83d7a shadow
const YELLOW: Color = Color(255, 238, 120, 255);     // #ffee78 lens inner base
const YELLOW_LIGHT: Color = Color(255, 246, 176, 255); // #fff6b0 highlight
const YELLOW_DARK: Color = Color(197, 168, 32, 255); // #c5a820 shadow
const WHITE: Color = Color(255, 255, 255, 255);      // #ffffff center highlight
const RED: Color = Color(255, 77, 109, 255);         // #ff4d6d shutter base
const RED_LIGHT: Color = Color(255, 143, 165, 255);  // #ff8fa5 highlight

/// Generate the base 32x32 pixel art matching the hero camera logo SVG exactly
fn generate_camera_pixels_32() -> Vec<u8> {
    let w = 32usize;
    let h = 32usize;
    let mut buf = vec![0u8; w * h * 4];

    fn set(buf: &mut [u8], w: usize, x: usize, y: usize, c: Color) {
        if x < w && y < 32 {
            let idx = (y * w + x) * 4;
            buf[idx] = c.0;
            buf[idx + 1] = c.1;
            buf[idx + 2] = c.2;
            buf[idx + 3] = c.3;
        }
    }

    fn rect(buf: &mut [u8], w: usize, x: usize, y: usize, rw: usize, rh: usize, c: Color) {
        for dy in 0..rh {
            for dx in 0..rw {
                set(buf, w, x + dx, y + dy, c);
            }
        }
    }

    // Draw in exact SVG order (later draws on top)

    // 1. Viewfinder bump base
    rect(&mut buf, w, 10, 4, 10, 3, MINT);
    // 2. Viewfinder top highlight
    rect(&mut buf, w, 10, 4, 10, 1, MINT_LIGHT);
    // 3. Viewfinder left highlight
    rect(&mut buf, w, 10, 6, 1, 1, MINT_LIGHT);
    // 4. Viewfinder right shadow
    rect(&mut buf, w, 19, 6, 1, 1, MINT_DARK);

    // 5. Main body base
    rect(&mut buf, w, 2, 7, 28, 20, MINT);
    // 6. Main body top highlight
    rect(&mut buf, w, 2, 7, 28, 1, MINT_LIGHT);
    // 7. Main body bottom shadow
    rect(&mut buf, w, 2, 26, 28, 1, MINT_DARK);
    // 8. Main body left highlight
    rect(&mut buf, w, 2, 8, 1, 18, MINT_LIGHT);
    // 9. Main body right shadow
    rect(&mut buf, w, 29, 8, 1, 18, MINT_DARK);

    // 10. Right grip base
    rect(&mut buf, w, 27, 10, 3, 8, MINT);
    // 11. Right grip top-left highlight
    rect(&mut buf, w, 27, 10, 1, 1, MINT_LIGHT);
    // 12. Right grip bottom-right shadow
    rect(&mut buf, w, 29, 17, 1, 1, MINT_DARK);

    // 13. Lens outer ring (pink) base
    rect(&mut buf, w, 9, 11, 14, 12, PINK);
    // 14. Lens outer top highlight
    rect(&mut buf, w, 9, 11, 14, 1, PINK_LIGHT);
    // 15. Lens outer bottom shadow
    rect(&mut buf, w, 9, 22, 14, 1, PINK_DARK);
    // 16. Lens outer left highlight
    rect(&mut buf, w, 9, 12, 1, 10, PINK_LIGHT);
    // 17. Lens outer right shadow
    rect(&mut buf, w, 22, 12, 1, 10, PINK_DARK);

    // 18. Lens inner (yellow) base
    rect(&mut buf, w, 12, 14, 8, 6, YELLOW);
    // 19. Lens inner top highlight
    rect(&mut buf, w, 12, 14, 8, 1, YELLOW_LIGHT);
    // 20. Lens inner bottom shadow
    rect(&mut buf, w, 12, 19, 8, 1, YELLOW_DARK);
    // 21. Lens inner left highlight
    rect(&mut buf, w, 12, 15, 1, 4, YELLOW_LIGHT);
    // 22. Lens inner right shadow
    rect(&mut buf, w, 19, 15, 1, 4, YELLOW_DARK);

    // 23. Lens center highlight (white)
    rect(&mut buf, w, 14, 15, 3, 3, WHITE);

    // 24. Shutter button (red) base
    rect(&mut buf, w, 24, 12, 2, 2, RED);
    // 25. Shutter button highlight
    rect(&mut buf, w, 24, 12, 1, 1, RED_LIGHT);

    // 26. Feet (dark mint)
    rect(&mut buf, w, 5, 27, 4, 2, MINT_DARK);
    rect(&mut buf, w, 23, 27, 4, 2, MINT_DARK);

    buf
}

/// Scale pixel art to a larger size using nearest-neighbor (pixel-perfect)
fn scale_pixel_art(src: &[u8], src_w: u32, src_h: u32, scale: u32) -> Vec<u8> {
    let dst_w = src_w * scale;
    let dst_h = src_h * scale;
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    for sy in 0..src_h {
        for sx in 0..src_w {
            let src_idx = ((sy * src_w + sx) * 4) as usize;
            let r = src[src_idx];
            let g = src[src_idx + 1];
            let b = src[src_idx + 2];
            let a = src[src_idx + 3];
            for dy in 0..scale {
                for dx in 0..scale {
                    let dst_x = sx * scale + dx;
                    let dst_y = sy * scale + dy;
                    let dst_idx = ((dst_y * dst_w + dst_x) * 4) as usize;
                    dst[dst_idx] = r;
                    dst[dst_idx + 1] = g;
                    dst[dst_idx + 2] = b;
                    dst[dst_idx + 3] = a;
                }
            }
        }
    }
    dst
}

fn generate_icons() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from("icons");
    std::fs::create_dir_all(&out_dir)?;

    let base32 = generate_camera_pixels_32();

    // Generate individual PNGs at each size
    let sizes: Vec<(u32, &str, u32)> = vec![
        (16, "16x16.png", 1),   // 16 = 32/2 (downscale by taking every other pixel)
        (32, "32x32.png", 1),   // base size
        (64, "64x64.png", 2),   // 2x scale
        (128, "128x128.png", 4),// 4x scale
        (256, "256x256.png", 8),// 8x scale
    ];

    let mut images: Vec<image::DynamicImage> = Vec::new();

    for (size, filename, scale) in &sizes {
        let pixels = if *scale == 1 && *size == 32 {
            base32.clone()
        } else if *scale == 1 && *size < 32 {
            // Downscale from 32 to 16 by sampling every other pixel
            let mut down = vec![0u8; (*size * *size * 4) as usize];
            let step = 32 / *size;
            for dy in 0..*size {
                for dx in 0..*size {
                    let src_x = dx * step;
                    let src_y = dy * step;
                    let src_idx = ((src_y * 32 + src_x) * 4) as usize;
                    let dst_idx = ((dy * *size + dx) * 4) as usize;
                    down[dst_idx] = base32[src_idx];
                    down[dst_idx + 1] = base32[src_idx + 1];
                    down[dst_idx + 2] = base32[src_idx + 2];
                    down[dst_idx + 3] = base32[src_idx + 3];
                }
            }
            down
        } else {
            scale_pixel_art(&base32, 32, 32, *scale)
        };

        if let Some(img) = image::RgbaImage::from_raw(*size, *size, pixels.clone()) {
            let dyn_img = image::DynamicImage::ImageRgba8(img);
            let _ = dyn_img.save(out_dir.join(filename));
            images.push(dyn_img);
        }
    }

    // Also save 128x128@2x.png as alias for 256x256 (Tauri convention)
    if let Some(img) = image::RgbaImage::from_raw(256, 256, scale_pixel_art(&base32, 32, 32, 8)) {
        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let _ = dyn_img.save(out_dir.join("128x128@2x.png"));
    }

    // Also save 128x128.png
    if let Some(img) = image::RgbaImage::from_raw(128, 128, scale_pixel_art(&base32, 32, 32, 4)) {
        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let _ = dyn_img.save(out_dir.join("128x128.png"));
    }

    // Save icon.png (1024x1024 as Tauri recommends for the master icon source)
    // We'll scale the 32x32 pixel art to 1024x1024 (32x scale) for best quality
    let pixels1024 = scale_pixel_art(&base32, 32, 32, 32);
    if let Some(img) = image::RgbaImage::from_raw(1024, 1024, pixels1024) {
        let dyn_img = image::DynamicImage::ImageRgba8(img);
        let _ = dyn_img.save(out_dir.join("icon.png"));
    }

    // Build multi-size ICO file
    use image::codecs::ico::{IcoEncoder, IcoFrame};
    use std::fs::File;
    use std::io::BufWriter;

    let ico_path = out_dir.join("icon.ico");
    let file = File::create(&ico_path)?;
    let writer = BufWriter::new(file);
    let encoder = IcoEncoder::new(writer);

    // Encode images at sizes: 16, 32, 48, 64, 128, 256
    let ico_sizes: Vec<(u32, u32)> = vec![
        (16, 1),
        (32, 1),
        (48, 1),  // 48 = special, need to generate it
        (64, 2),
        (128, 4),
        (256, 8),
    ];

    let mut frames = Vec::new();
    for (size, scale) in &ico_sizes {
        let pixels = if *scale == 1 && *size == 32 {
            base32.clone()
        } else if *scale == 1 && *size == 16 {
            let mut down = vec![0u8; (16 * 16 * 4) as usize];
            for dy in 0..16 {
                for dx in 0..16 {
                    let src_x = dx * 2;
                    let src_y = dy * 2;
                    let src_idx = ((src_y * 32 + src_x) * 4) as usize;
                    let dst_idx = ((dy * 16 + dx) * 4) as usize;
                    down[dst_idx] = base32[src_idx];
                    down[dst_idx + 1] = base32[src_idx + 1];
                    down[dst_idx + 2] = base32[src_idx + 2];
                    down[dst_idx + 3] = base32[src_idx + 3];
                }
            }
            down
        } else if *scale == 1 && *size == 48 {
            // 48 is 1.5x of 32, use nearest neighbor from a 2x scale then downsample
            // Actually, 48 doesn't divide evenly. Let's scale 32->96 (3x) then take every other pixel?
            // Better: scale 32->96 then downscale by taking every 2nd pixel from a 3x scale gives us 48
            let scale3 = scale_pixel_art(&base32, 32, 32, 3); // 96x96
            let mut down = vec![0u8; (48 * 48 * 4) as usize];
            for dy in 0..48 {
                for dx in 0..48 {
                    // Map 48->96: take every 2nd pixel
                    let src_x = (dx * 2).min(95);
                    let src_y = (dy * 2).min(95);
                    let src_idx = ((src_y * 96 + src_x) * 4) as usize;
                    let dst_idx = ((dy * 48 + dx) * 4) as usize;
                    down[dst_idx] = scale3[src_idx];
                    down[dst_idx + 1] = scale3[src_idx + 1];
                    down[dst_idx + 2] = scale3[src_idx + 2];
                    down[dst_idx + 3] = scale3[src_idx + 3];
                }
            }
            down
        } else {
            scale_pixel_art(&base32, 32, 32, *scale)
        };

        if let Some(rgba) = image::RgbaImage::from_raw(*size, *size, pixels) {
            // For sizes <= 256, ICO can store them; PNG encoding is used automatically for larger sizes by the encoder
            let frame = IcoFrame::as_png(rgba.as_raw(), *size, *size, image::ExtendedColorType::Rgba8)?;
            frames.push(frame);
        }
    }

    encoder.encode_images(frames.as_slice())?;

    eprintln!("[BUILD] Generated icons in {:?}", out_dir);
    Ok(())
}
