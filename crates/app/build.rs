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
            let resized_256 = img.resize_exact(256, 256, image::imageops::FilterType::Lanczos3);
            let _ = resized_256.save_with_format(&public_ico_path, image::ImageFormat::Ico);
        }

        if public_ico_path.exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon(public_ico_path.to_str().unwrap_or("public/app_icon.ico"));
            res.set("ProductName", "rDM");
            res.set("FileDescription", "rDM - Internet Download Manager");
            res.set("LegalCopyright", "Copyright (c) 2026 moiCR");
            let _ = res.compile();
        }
    }
}
