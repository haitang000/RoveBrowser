#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod bookmarks;
mod caption;
mod capture;
mod clip;
mod favicon;
mod find;
mod glass;
mod host;
mod model;
mod preview;
mod shadow;
mod settings;
mod storage;
mod split;
mod suggestions;
mod thumbnails;
mod web_settings;
fn main() {
    if let Err(e) = host::run() {
        eprintln!("Rove: {e}");
        let _ = std::fs::write(std::env::temp_dir().join("rove-error.log"), e.to_string());
    }
}
