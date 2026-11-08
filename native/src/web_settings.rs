//! WebView2 equivalents of Zigzag's WebKit preferences and browsing-data actions.
use crate::settings::Settings;
use webview2_com::{
    ClearBrowsingDataCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Profile2, ICoreWebView2Profile6, ICoreWebView2Settings4, ICoreWebView2_13,
        COREWEBVIEW2_BROWSING_DATA_KINDS_ALL_DOM_STORAGE, COREWEBVIEW2_BROWSING_DATA_KINDS_COOKIES,
        COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
    },
};
use windows::core::{Interface, HSTRING};
use wry::{WebView, WebViewExtWindows};

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
    kind: &str,
    callback: impl Fn(bool) + 'static,
) -> windows::core::Result<()> {
    let kinds = match kind {
        "cache" => COREWEBVIEW2_BROWSING_DATA_KINDS_DISK_CACHE,
        "siteData" => {
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
