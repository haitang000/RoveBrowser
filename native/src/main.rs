#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod bookmarks;
mod caption;
mod capture;
mod clip;
mod host;
mod model;
mod shadow;
mod storage;
mod thumbnails;
fn main() {
    if let Err(e) = host::run() {
        eprintln!("Rove: {e}");
        let _ = std::fs::write(std::env::temp_dir().join("rove-error.log"), e.to_string());
    }
}
