#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod bookmarks;
mod caption;
mod capture;
mod clipboard;
mod clip;
mod downloads;
mod error_page;
mod default_browser;
mod favicon;
mod find;
mod frame;
mod glass;
mod host;
mod model;
mod onboarding;
mod import;
mod import_store;
mod instance;
mod layout;
mod passwords;
mod preview;
mod shadow;
mod site;
mod settings;
mod sleep;
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
