use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use std::env;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=../dist");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon_png = manifest_dir.join("assets").join("icon.png");
    println!("cargo:rerun-if-changed={}", icon_png.display());

    // The executable icon is a Windows resource. Skip it for any other target.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let icon_ico = out_dir.join("icon.ico");
    write_ico(&icon_png, &icon_ico)
        .unwrap_or_else(|error| panic!("failed to build {}: {error}", icon_ico.display()));
    let rc_path = out_dir.join("icon.rc");
    // rc.exe treats backslash as an escape, so the absolute path is doubled.
    let escaped = icon_ico.display().to_string().replace('\\', "\\\\");
    fs::write(&rc_path, format!("1 ICON \"{escaped}\"\n"))
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", rc_path.display()));
    // Icon only. Do not attach an application manifest; that would replace the
    // process DPI settings already established at startup.
    embed_resource::compile(&rc_path, embed_resource::NONE)
        .manifest_required()
        .unwrap_or_else(|error| panic!("failed to embed the application icon: {error}"));
}

fn write_ico(
    png_path: &std::path::Path,
    ico_path: &std::path::Path,
) -> Result<(), image::ImageError> {
    let source = image::open(png_path)?;
    let mut sizes = vec![16u32, 20, 24, 32, 40, 48, 64, 128, 256];
    let native = source.width().min(source.height());
    if native > 0 && native < 256 && !sizes.contains(&native) {
        sizes.push(native);
    }
    sizes.sort_unstable();

    let mut images = Vec::with_capacity(sizes.len());
    for size in sizes {
        let rgba = if source.width() == size && source.height() == size {
            source.to_rgba8()
        } else {
            source
                .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
                .into_rgba8()
        };
        let mut png = Vec::new();
        PngEncoder::new(Cursor::new(&mut png)).write_image(
            rgba.as_raw(),
            size,
            size,
            ExtendedColorType::Rgba8,
        )?;
        images.push((size, png));
    }
    fs::write(ico_path, pack_ico(&images)).map_err(image::ImageError::IoError)?;
    Ok(())
}

fn pack_ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let count = images.len() as u16;
    let mut file = Vec::new();
    file.extend_from_slice(&0u16.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + u32::from(count) * 16;
    for (size, png) in images {
        let dimension = if *size >= 256 { 0 } else { *size as u8 };
        file.push(dimension);
        file.push(dimension);
        file.push(0);
        file.push(0);
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&32u16.to_le_bytes());
        file.extend_from_slice(&(png.len() as u32).to_le_bytes());
        file.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        file.extend_from_slice(png);
    }
    file
}
