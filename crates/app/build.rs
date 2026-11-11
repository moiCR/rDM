#[cfg(windows)]
use std::path::PathBuf;

fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=../../icon.png");
        println!("cargo:rerun-if-changed=build.rs");

        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
        let icon_png_path = manifest_dir.join("../../icon.png");
        let public_ico_path = manifest_dir.join("public/app_icon.ico");

        if icon_png_path.exists()
            && let Ok(img) = image::open(&icon_png_path)
        {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            let mut min_x = width;
            let mut max_x = 0;
            let mut min_y = height;
            let mut max_y = 0;

            for (x, y, pixel) in rgba.enumerate_pixels() {
                if pixel[3] > 5 {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }

            let cropped = if min_x <= max_x && min_y <= max_y {
                let content_w = max_x - min_x + 1;
                let content_h = max_y - min_y + 1;
                let side = content_w.max(content_h);
                let padded_side = ((side as f32 * 1.10).ceil() as u32).min(width.min(height));
                let center_x = (min_x + max_x) as f32 / 2.0;
                let center_y = (min_y + max_y) as f32 / 2.0;
                let crop_x = ((center_x - padded_side as f32 / 2.0).round().max(0.0) as u32)
                    .min(width.saturating_sub(padded_side));
                let crop_y = ((center_y - padded_side as f32 / 2.0).round().max(0.0) as u32)
                    .min(height.saturating_sub(padded_side));
                img.crop_imm(crop_x, crop_y, padded_side, padded_side)
            } else {
                img
            };

            let resized_256 = cropped.resize_exact(256, 256, image::imageops::FilterType::Lanczos3);
            let _ = resized_256.save_with_format(&public_ico_path, image::ImageFormat::Ico);
        }

        if public_ico_path.exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon(public_ico_path.to_str().unwrap_or("public/app_icon.ico"));
            res.set("ProductName", "rDM");
            res.set("FileDescription", "rDM");
            res.set("LegalCopyright", "Copyright (c) 2026 moiCR");
            let _ = res.compile();
        }
    }
}
