//! Per-page site controls ported from Zigzag: `Tab.setMuted`, the zoom ladder
//! in `BrowserActions.swift` / `ZoomLevels`, and the audio state behind the
//! sidebar's Now Playing control. Playback commands use WebView2's frame APIs.
use crate::model::MediaAction;
use std::{
    cell::{Cell, RefCell},
    cmp::Reverse,
    rc::Rc,
};
use webview2_com::{
    ContentLoadingEventHandler, ExecuteScriptCompletedHandler, FrameChildFrameCreatedEventHandler,
    FrameCreatedEventHandler, FrameDestroyedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Frame, ICoreWebView2Frame2, ICoreWebView2Frame7, ICoreWebView2_4,
        ICoreWebView2_8,
    },
};
use windows::core::{Interface, BOOL, HSTRING};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaOutcome {
    Unavailable,
    Blocked,
    Applied,
    Cancelled,
}

impl MediaOutcome {
    fn from_script(value: &str) -> Self {
        match value {
            "\"applied\"" => Self::Applied,
            "\"blocked\"" => Self::Blocked,
            "\"cancelled\"" => Self::Cancelled,
            _ => Self::Unavailable,
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackDetails {
    duration: Option<f64>,
    position: Option<f64>,
    seekable: bool,
    previous: bool,
    next: bool,
    picture_in_picture: bool,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct NowPlayingDetails {
    #[serde(skip_serializing)]
    priority: u8,
    #[serde(skip_serializing)]
    token: String,
    title: Option<String>,
    colors: Option<[[u8; 3]; 3]>,
    sampling: bool,
    playback: PlaybackDetails,
}

struct MediaTarget {
    frame: Option<ICoreWebView2Frame2>,
    token: String,
}

/// WebView2 executes in each frame directly, including cross-origin players.
/// The stored media elements belong to the frame document that paused them.
#[derive(Default)]
pub struct MediaPlayback {
    frames: RefCell<Vec<ICoreWebView2Frame>>,
    document: Cell<u64>,
    target: RefCell<Option<MediaTarget>>,
}

impl MediaPlayback {
    pub fn install(self: &Rc<Self>, view: &WebView) -> windows::core::Result<()> {
        let web = view.webview();
        let playback = Rc::downgrade(self);
        let loading = ContentLoadingEventHandler::create(Box::new(move |_, _| {
            if let Some(playback) = playback.upgrade() {
                playback.target.borrow_mut().take();
                playback
                    .document
                    .set(playback.document.get().wrapping_add(1));
            }
            Ok(())
        }));
        let playback = Rc::downgrade(self);
        let created = FrameCreatedEventHandler::create(Box::new(move |_, args| {
            if let (Some(playback), Some(args)) = (playback.upgrade(), args) {
                playback.track_frame(unsafe { args.Frame()? })?;
            }
            Ok(())
        }));
        unsafe {
            web.add_ContentLoading(&loading, &mut 0)?;
            web.cast::<ICoreWebView2_4>()?
                .add_FrameCreated(&created, &mut 0)
        }
    }

    fn track_frame(self: &Rc<Self>, frame: ICoreWebView2Frame) -> windows::core::Result<()> {
        let playback = Rc::downgrade(self);
        let destroyed = FrameDestroyedEventHandler::create(Box::new(move |sender, _| {
            if let (Some(playback), Some(sender)) = (playback.upgrade(), sender) {
                playback
                    .frames
                    .borrow_mut()
                    .retain(|frame| frame != &sender);
            }
            Ok(())
        }));
        unsafe {
            frame.add_Destroyed(&destroyed, &mut 0)?;
        }
        self.frames.borrow_mut().push(frame.clone());
        if let Ok(parent) = frame.cast::<ICoreWebView2Frame7>() {
            let playback = Rc::downgrade(self);
            let created = FrameChildFrameCreatedEventHandler::create(Box::new(move |_, args| {
                if let (Some(playback), Some(args)) = (playback.upgrade(), args) {
                    playback.track_frame(unsafe { args.Frame()? })?;
                }
                Ok(())
            }));
            unsafe {
                parent.add_FrameCreated(&created, &mut 0)?;
            }
        }
        Ok(())
    }

    fn script_frames(&self, limit: usize) -> Vec<ICoreWebView2Frame2> {
        self.frames
            .borrow()
            .iter()
            .filter_map(|frame| frame.cast::<ICoreWebView2Frame2>().ok())
            .take(limit)
            .collect()
    }

    pub fn details(
        self: &Rc<Self>,
        view: &WebView,
        report: impl Fn(Option<NowPlayingDetails>) + 'static,
    ) {
        let script = HSTRING::from(include_str!("now-playing-artwork.js"));
        // At most three cards ask for this data. Bound the periodic work on
        // pages with many embedded documents while still checking players in frames.
        let frames = Rc::new(self.script_frames(16));
        let remaining = Cell::new(frames.len() + 1);
        let best = RefCell::new(None::<(usize, NowPlayingDetails)>);
        let document = self.document.get();
        let playback = Rc::downgrade(self);
        let sampled_frames = frames.clone();
        let finish = Rc::new(move |index, candidate: Option<NowPlayingDetails>| {
            if let Some(candidate) = candidate {
                let mut best = best.borrow_mut();
                if best.as_ref().is_none_or(|(prior_index, prior)| {
                    (
                        candidate.priority,
                        candidate.colors.is_some(),
                        Reverse(index),
                    ) > (
                        prior.priority,
                        prior.colors.is_some(),
                        Reverse(*prior_index),
                    )
                }) {
                    *best = Some((index, candidate));
                }
            }
            remaining.set(remaining.get() - 1);
            if remaining.get() == 0 {
                if let Some(playback) = playback.upgrade().filter(|p| p.document.get() == document)
                {
                    let best = best.borrow_mut().take();
                    *playback.target.borrow_mut() =
                        best.as_ref().map(|(index, details)| MediaTarget {
                            frame: index.checked_sub(1).map(|i| sampled_frames[i].clone()),
                            token: details.token.clone(),
                        });
                    report(best.map(|(_, details)| details));
                }
            }
        });
        let handler = |index| {
            let finish = finish.clone();
            ExecuteScriptCompletedHandler::create(Box::new(move |result, value| {
                let candidate = if result.is_ok() && value.len() <= 8192 {
                    serde_json::from_str::<NowPlayingDetails>(&value).ok()
                } else {
                    None
                };
                finish(
                    index,
                    candidate.filter(|item| {
                        item.priority <= 15
                            && item.token.len() <= 32
                            && item
                                .title
                                .as_ref()
                                .is_none_or(|title| title.chars().count() <= 320)
                    }),
                );
                Ok(())
            }))
        };
        if unsafe { view.webview().ExecuteScript(&script, &handler(0)) }.is_err() {
            finish(0, None);
        }
        for (index, frame) in (1..).zip(frames.iter()) {
            if unsafe { frame.ExecuteScript(&script, &handler(index)) }.is_err() {
                finish(index, None);
            }
        }
    }

    pub fn control(
        self: &Rc<Self>,
        view: &WebView,
        action: MediaAction,
        url: &str,
        report: impl Fn(MediaOutcome) + 'static,
    ) {
        if !matches!(action, MediaAction::Play | MediaAction::Pause) {
            self.control_target(view, action, url, report);
            return;
        }
        // The top document also checks the address at execution time. Each child
        // is scoped by WebView2 to its own frame and is removed on destruction.
        let script = include_str!("media-control.js");
        let pause = matches!(action, MediaAction::Pause);
        let top = HSTRING::from(format!("({script})({pause}, {})", serde_json::json!(url)));
        let child = HSTRING::from(format!("({script})({pause})"));
        let frames = self.script_frames(usize::MAX);
        let remaining = Cell::new(frames.len() + 1);
        let outcome = Cell::new(MediaOutcome::Unavailable);
        let document = self.document.get();
        let playback = Rc::downgrade(self);
        let finish = Rc::new(move |result| {
            outcome.set(outcome.get().max(result));
            remaining.set(remaining.get() - 1);
            if remaining.get() == 0 {
                report(
                    if playback
                        .upgrade()
                        .is_some_and(|p| p.document.get() == document)
                    {
                        outcome.get()
                    } else {
                        MediaOutcome::Cancelled
                    },
                );
            }
        });
        // A completed handler is single-use, so each frame gets its own.
        let handler = || {
            let finish = finish.clone();
            ExecuteScriptCompletedHandler::create(Box::new(move |result, value| {
                finish(if result.is_ok() {
                    MediaOutcome::from_script(&value.to_string())
                } else {
                    MediaOutcome::Unavailable
                });
                Ok(())
            }))
        };
        if unsafe { view.webview().ExecuteScript(&top, &handler()) }.is_err() {
            finish(MediaOutcome::Unavailable);
        }
        for frame in frames {
            if unsafe { frame.ExecuteScript(&child, &handler()) }.is_err() {
                finish(MediaOutcome::Unavailable);
            }
        }
    }

    fn control_target(
        &self,
        view: &WebView,
        action: MediaAction,
        url: &str,
        report: impl Fn(MediaOutcome) + 'static,
    ) {
        let target = self.target.borrow();
        let Some(target) = target.as_ref() else {
            report(MediaOutcome::Unavailable);
            return;
        };
        let script = include_str!("now-playing-action.js");
        let action = serde_json::json!(action);
        let token = serde_json::json!(target.token);
        let page = if target.frame.is_none() {
            serde_json::json!(url).to_string()
        } else {
            "undefined".into()
        };
        let script = HSTRING::from(format!("({script})({action}, {token}, {page})"));
        let report = Rc::new(report);
        let finished = report.clone();
        let handler = ExecuteScriptCompletedHandler::create(Box::new(move |result, value| {
            finished(if result.is_ok() {
                MediaOutcome::from_script(&value.to_string())
            } else {
                MediaOutcome::Unavailable
            });
            Ok(())
        }));
        let result = unsafe {
            match &target.frame {
                Some(frame) => frame.ExecuteScript(&script, &handler),
                None => view.webview().ExecuteScript(&script, &handler),
            }
        };
        if result.is_err() {
            report(MediaOutcome::Unavailable);
        }
    }
}

pub fn zoom_factor_controller(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
) -> windows::core::Result<f64> {
    let mut factor = 1.0;
    // SAFETY: plain out-parameter owned by this call on the UI thread.
    unsafe { controller.ZoomFactor(&mut factor)? };
    Ok(factor)
}
