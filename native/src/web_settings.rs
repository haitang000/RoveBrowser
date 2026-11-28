//! WebView2 equivalents of Zigzag's WebKit preferences and browsing-data actions.
use crate::settings::{Settings, TrackingProtection};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs,
    io::{self, Write},
    path::Path,
    time::{Duration, Instant},
};
use webview2_com::{
    ClearBrowsingDataCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Profile2, ICoreWebView2Profile3, ICoreWebView2Profile6, ICoreWebView2Settings4,
        ICoreWebView2_13,
        COREWEBVIEW2_BROWSING_DATA_KINDS_ALL_DOM_STORAGE, COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES,
        COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE, COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BASIC,
        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BALANCED, COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT,
    },
};
use windows::core::{Interface, HSTRING};
use wry::{WebView, WebViewExtWindows};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowsingDataKind {
    History,
    Cache,
    SiteData,
    Downloads,
}

pub fn set_tracking_protection(
    view: &WebView,
    protection: TrackingProtection,
) -> windows::core::Result<()> {
    let level = match protection {
        TrackingProtection::Basic => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BASIC,
        TrackingProtection::Balanced => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_BALANCED,
        TrackingProtection::Strict => COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_STRICT,
    };
    let mut applied = Default::default();
    unsafe {
        let profile: ICoreWebView2Profile3 = view.webview().cast::<ICoreWebView2_13>()?.Profile()?.cast()?;
        profile.PreferredTrackingPreventionLevel(&mut applied)?;
        if applied == level {
            return Ok(());
        }
        profile.SetPreferredTrackingPreventionLevel(level)?;
        profile.PreferredTrackingPreventionLevel(&mut applied)?;
    }
    if applied != level {
        return Err(windows::core::Error::from(windows::Win32::Foundation::E_FAIL));
    }
    Ok(())
}

#[derive(Default)]
pub struct StartupPreferenceFailures {
    pub secure_dns: bool,
    pub web_rtc: bool,
}

/// WebView2 reads these preferences at startup. Wait for the previous browser
/// process to finish writing before updating its user data.
pub fn sync_startup_preferences(
    ui: &Path,
    browsing: &Path,
    agent: &Path,
    settings: &Settings,
) -> StartupPreferenceFailures {
    let mut failures = StartupPreferenceFailures::default();
    // A WebView2 process can outlive Rove. All profiles share one startup deadline.
    let deadline = Instant::now() + Duration::from_secs(15);
    for (profile, uses_webrtc) in [(ui, false), (browsing, true), (agent, true)] {
        let browser_data = profile.join("EBWebView");
        if let Err(error) = wait_for_profile(&browser_data, deadline) {
            eprintln!("Rove WebView2 ({}): {error}", profile.display());
            failures.secure_dns = true;
            failures.web_rtc |= uses_webrtc;
            continue;
        }
        if let Err(error) = sync_secure_dns(&browser_data, settings.secure_dns) {
            eprintln!("Rove Secure DNS ({}): {error}", profile.display());
            failures.secure_dns = true;
        }
        if uses_webrtc {
            if let Err(error) = sync_webrtc_address_protection(
                &browser_data.join("Default"),
                settings.web_rtc_address_protection,
            ) {
                eprintln!("Rove WebRTC ({}): {error}", profile.display());
                failures.web_rtc = true;
            }
        }
    }
    failures
}

fn wait_for_profile(browser_data: &Path, deadline: Instant) -> io::Result<()> {
    let lock = browser_data.join("lockfile");
    while lock.try_exists()? {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "WebView2 profile is still open",
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

fn sync_secure_dns(browser_data: &Path, enabled: bool) -> io::Result<()> {
    // Automatic mode upgrades compatible system resolvers to DoH.
    let mode = if enabled { "automatic" } else { "off" };
    update_preference_file(browser_data, "Local State", |prefs| {
        let dns = prefs
            .entry("dns_over_https")
            .or_insert_with(|| Value::Object(Map::new()));
        let dns = dns.as_object_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid WebView2 DNS preferences")
        })?;
        if dns.get("mode").and_then(Value::as_str) == Some(mode) {
            return Ok(false);
        }
        dns.insert("mode".into(), Value::String(mode.into()));
        Ok(true)
    })
}

fn sync_webrtc_address_protection(profile: &Path, enabled: bool) -> io::Result<()> {
    const POLICY: &str = "disable_non_proxied_udp";
    update_preference_file(profile, "Preferences", |prefs| {
        let protected = prefs
            .get("webrtc")
            .and_then(|rtc| rtc.get("ip_handling_policy"))
            .and_then(Value::as_str) == Some(POLICY);
        if protected == enabled {
            return Ok(false);
        }
        let rtc = prefs
            .entry("webrtc")
            .or_insert_with(|| Value::Object(Map::new()));
        let rtc = rtc.as_object_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Invalid WebView2 WebRTC preferences")
        })?;
        if enabled {
            rtc.insert("ip_handling_policy".into(), Value::String(POLICY.into()));
        } else {
            rtc.remove("ip_handling_policy");
        }
        Ok(true)
    })
}

fn update_preference_file(
    directory: &Path,
    filename: &str,
    update: impl FnOnce(&mut Map<String, Value>) -> io::Result<bool>,
) -> io::Result<()> {
    let path = directory.join(filename);
    let mut prefs: Map<String, Value> = match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Map::new(),
        Err(error) => return Err(error),
    };
    if !update(&mut prefs)? {
        return Ok(());
    }

    fs::create_dir_all(directory)?;
    let json = serde_json::to_vec(&prefs).map_err(io::Error::other)?;
    let temporary = directory.join(format!("{filename}.rove-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::File::create_new(&temporary)?;
        file.write_all(&json)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn apply(view: &WebView, settings: &Settings) -> windows::core::Result<()> {
    unsafe {
        let web = view.webview();
        let controls: ICoreWebView2Settings4 = web.Settings()?.cast()?;
        controls.SetIsPasswordAutosaveEnabled(settings.save_passwords)?;
        controls.SetIsGeneralAutofillEnabled(settings.autofill)?;
        let profile = web.cast::<ICoreWebView2_13>()?.Profile()?;
        let prefs: ICoreWebView2Profile6 = profile.cast()?;
        prefs.SetIsPasswordAutosaveEnabled(settings.save_passwords)?;
        prefs.SetIsGeneralAutofillEnabled(settings.autofill)?;
        let folder = if settings.download_folder.is_empty() {
            directories::UserDirs::new().and_then(|d| d.download_dir().map(|p| p.to_path_buf()))
        } else {
            Some(settings.download_folder.clone().into())
        };
        if let Some(folder) = folder {
            profile.SetDefaultDownloadFolderPath(&HSTRING::from(folder.as_os_str()))?;
        }
    }
    Ok(())
}
pub fn clear(
    view: &WebView,
    kind: BrowsingDataKind,
    callback: impl Fn(bool) + 'static,
) -> windows::core::Result<()> {
    let kinds = match kind {
        BrowsingDataKind::Cache => COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
        BrowsingDataKind::SiteData => {
            COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES
                | COREWEBVIEW2_BROWSING_DATA_KINDS_ALL_DOM_STORAGE
        }
        _ => {
            return Err(windows::core::Error::from(
                windows::Win32::Foundation::E_INVALIDARG,
            ))
        }
    };
    unsafe {
        let profile: ICoreWebView2Profile2 = view
            .webview()
            .cast::<ICoreWebView2_13>()?
            .Profile()?
            .cast()?;
        profile.ClearBrowsingData(
            kinds,
            &ClearBrowsingDataCompletedHandler::create(Box::new(move |result| {
                callback(result.is_ok());
                Ok(())
            })),
        )?;
    }
    Ok(())
}
