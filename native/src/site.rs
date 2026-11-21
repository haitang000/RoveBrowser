//! Per-page site controls ported from Zigzag: `Tab.setMuted`, the zoom ladder
//! in `BrowserActions.swift` / `ZoomLevels`, and the audio state behind the
//! sidebar's Now Playing control. WebView2 exposes all of them on the page's
//! controller, so this module is the only place that talks to those APIs.
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8;
use windows::core::{Interface, BOOL};
use wry::{WebView, WebViewExtWindows};

/// Zigzag clamps its ladder to 0.5...3; WebView2 allows 0.25...5.
pub fn set_zoom_factor(view: &WebView, factor: f64) -> windows::core::Result<()> {
    if !crate::model::valid_zoom(factor) {
        return Err(windows::core::Error::from(
            windows::Win32::Foundation::E_INVALIDARG,
        ));
    }
    // SAFETY: controller belongs to this live page and runs on the UI thread.
    unsafe { view.controller().SetZoomFactor(factor) }
}

pub fn zoom_factor(view: &WebView) -> windows::core::Result<f64> {
    zoom_factor_controller(&view.controller())
}

pub fn set_muted(view: &WebView, muted: bool) -> windows::core::Result<()> {
    // SAFETY: same UI thread; ICoreWebView2_8 is available on current runtimes.
    unsafe { view.webview().cast::<ICoreWebView2_8>()?.SetIsMuted(muted) }
}

pub fn is_playing_audio(view: &WebView) -> windows::core::Result<bool> {
    // SAFETY: same UI thread; the out-parameter is a plain BOOL.
    is_playing_audio_core(&view.webview())
}

pub fn is_playing_audio_core(
    web: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2,
) -> windows::core::Result<bool> {
    let mut playing = BOOL::default();
    // SAFETY: same UI thread; the out-parameter is a plain BOOL.
    unsafe {
        web.cast::<ICoreWebView2_8>()?
            .IsDocumentPlayingAudio(&mut playing)?
    };
    Ok(playing.as_bool())
}

/// Controls media elements from the sidebar's Now Playing card. Remember the
/// elements paused by the card so Play does not start unrelated paused players.
pub fn control_media(view: &WebView, action: &str) {
    let script = match action {
        "pause" => r#"(() => {
            const media = [...document.querySelectorAll('audio,video')];
            const playing = media.filter(item => !item.paused && !item.ended);
            window.__ROVE_NOW_PLAYING_PAUSED__ = playing;
            for (const item of playing) item.pause();
        })()"#,
        "play" => r#"(() => {
            const media = [...document.querySelectorAll('audio,video')];
            const remembered = Array.isArray(window.__ROVE_NOW_PLAYING_PAUSED__)
                ? window.__ROVE_NOW_PLAYING_PAUSED__ : [];
            const tracked = remembered.filter(item => media.includes(item) && item.paused && !item.ended);
            const targets = tracked.length
                ? tracked : media.filter(item => item.paused && !item.ended).slice(0, 1);
            window.__ROVE_NOW_PLAYING_PAUSED__ = [];
            for (const item of targets) item.play().catch(() => {});
        })()"#,
        _ => return,
    };
    let _ = view.evaluate_script(script);
}

pub fn zoom_factor_controller(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
) -> windows::core::Result<f64> {
    let mut factor = 1.0;
    // SAFETY: plain out-parameter owned by this call on the UI thread.
    unsafe { controller.ZoomFactor(&mut factor)? };
    Ok(factor)
}
