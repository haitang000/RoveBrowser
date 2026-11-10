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
    let mut factor = 1.0;
    // SAFETY: plain out-parameter owned by this call on the UI thread.
    unsafe { view.controller().ZoomFactor(&mut factor)? };
    Ok(factor)
}

/// Applies a stored site zoom without saving a redundant change. A failure
/// (for example an older runtime) leaves the page at its current zoom.
pub fn ensure_zoom(view: &WebView, desired: f64) {
    if !crate::model::valid_zoom(desired) {
        return;
    }
    if let Ok(current) = zoom_factor(view) {
        if (current - desired).abs() > 0.001 {
            let _ = set_zoom_factor(view, desired);
        }
    }
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

pub fn zoom_factor_controller(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
) -> windows::core::Result<f64> {
    let mut factor = 1.0;
    // SAFETY: plain out-parameter owned by this call on the UI thread.
    unsafe { controller.ZoomFactor(&mut factor)? };
    Ok(factor)
}
