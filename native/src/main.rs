#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod bookmarks;
mod caption;
mod capture;
mod clipboard;
mod clip;
mod chrome_store;
mod downloads;
mod error_page;
mod extensions;
mod default_browser;
mod favicon;
mod find;
mod frame;
mod glass;
mod host;
mod https;
mod model;
mod new_tab;
mod onboarding;
mod page_output;
mod import;
mod import_store;
mod instance;
mod layout;
mod passwords;
mod permissions;
mod preview;
mod reader;
mod resize_motion;
mod resize_surface;
mod shadow;
mod sidebar;
mod site;
mod settings;
mod session_writer;
mod state_snapshot;
mod sleep;
mod storage;
mod split;
mod suggestions;
mod thumbnails;
mod update_install;
mod updates;
mod web_settings;
fn main() {
    if update_install::run_helper() {
        return;
    }
    if let Err(e) = host::run(update_install::startup_message()) {
        eprintln!("Rove: {e}");
        let _ = std::fs::write(std::env::temp_dir().join("rove-error.log"), e.to_string());
    }
}
