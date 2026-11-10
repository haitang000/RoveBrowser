//! Behavioral port of Zigzag's `Window/FindBar.swift` (Zigao Wang, Apache-2.0)
//! onto WebView2's own `ICoreWebView2Find`. The original injected a script to
//! count matches because WebKit does not report them; WebView2 reports the
//! match count and the active match index itself, so this module only creates
//! a find session, keeps its two change handlers, and forwards their counts.
use std::rc::Rc;
use webview2_com::{
    FindActiveMatchIndexChangedEventHandler, FindMatchCountChangedEventHandler,
    FindStartCompletedHandler,
    Microsoft::Web::WebView2::Win32::{ICoreWebView2Find, ICoreWebView2Environment15, ICoreWebView2_28},
};
use windows::core::{Interface, HSTRING};
use wry::{WebView, WebViewExtWindows};

/// The query cap the UI applies too. Long enough for a sentence, short enough
/// that a hostile message cannot spend the renderer's time on a huge search.
pub const MAX_QUERY_CHARS: usize = 256;

pub type Report = Rc<dyn Fn(i32, i32)>;

pub struct Session {
    find: ICoreWebView2Find,
    count_token: i64,
    index_token: i64,
    /// The tab and document the search belongs to. A later load of another
    /// document ends the session; the initial load of the same page does not
    /// (the bar may legitimately open while that page is still loading).
    pub target_id: String,
    pub target_url: String,
}

impl Session {
    pub fn next(&self, backwards: bool) -> windows::core::Result<()> {
        // SAFETY: the session lives on the UI thread that owns the WebView, and
        // is closed before its WebView goes away (see App::close_find callers).
        unsafe {
            if backwards {
                self.find.FindPrevious()
            } else {
                self.find.FindNext()
            }
        }
    }

    /// Removes the change handlers and ends the find session, clearing its
    /// highlights. Called before a tab switch, navigation, or closing the bar.
    pub fn close(self) {
        // SAFETY: same UI thread as above; both interfaces belong to this session.
        unsafe {
            let _ = self.find.remove_MatchCountChanged(self.count_token);
            let _ = self.find.remove_ActiveMatchIndexChanged(self.index_token);
            let _ = self.find.Stop();
        }
    }
}

fn reads(find: &ICoreWebView2Find) -> (i32, i32) {
    let (mut count, mut active) = (0i32, 0i32);
    // SAFETY: out-parameters are plain integers owned by this call.
    unsafe {
        let _ = find.MatchCount(&mut count);
        let _ = find.ActiveMatchIndex(&mut active);
    }
    (count, active)
}

/// Starts (or restarts) a find session on `view`. The callback runs on the UI
/// thread with the new match count and active index whenever either changes.
pub fn start(
    view: &WebView,
    query: &str,
    target_id: String,
    target_url: String,
    report: Report,
) -> Result<Session, String> {
    let unavailable = || "当前 WebView2 不支持页内查找".to_string();
    // SAFETY: all interfaces are used on the WebView's own UI thread; every
    // out-parameter is initialized before it is read.
    unsafe {
        let core = view
            .webview()
            .cast::<ICoreWebView2_28>()
            .map_err(|_| unavailable())?;
        let find = core.Find().map_err(|_| unavailable())?;
        let environment = view
            .environment()
            .cast::<ICoreWebView2Environment15>()
            .map_err(|_| unavailable())?;
        let options = environment.CreateFindOptions().map_err(|_| unavailable())?;
        // Zigzag starts a fresh search for each keystroke, case-insensitively,
        // and keeps every match highlighted while the bar is open.
        options
            .SetFindTerm(&HSTRING::from(query))
            .map_err(|e| e.to_string())?;
        options.SetIsCaseSensitive(false).map_err(|e| e.to_string())?;
        options
            .SetShouldHighlightAllMatches(true)
            .map_err(|e| e.to_string())?;
        options
            .SetShouldMatchWord(false)
            .map_err(|e| e.to_string())?;
        // Rove draws its own bar; WebView2 must not add its own find dialog on
        // top of it. (Ctrl+F is also intercepted by the accelerator handler.)
        options
            .SetSuppressDefaultFindDialog(true)
            .map_err(|e| e.to_string())?;

        let mut count_token = 0i64;
        let mut index_token = 0i64;
        let count_report = report.clone();
        let count_handler =
            FindMatchCountChangedEventHandler::create(Box::new(move |sender, _args| {
                if let Some(sender) = sender {
                    let (count, active) = reads(&sender);
                    count_report(count, active);
                }
                Ok(())
            }));
        find.add_MatchCountChanged(&count_handler, &mut count_token)
            .map_err(|e| e.to_string())?;

        let index_report = report.clone();
        let index_handler =
            FindActiveMatchIndexChangedEventHandler::create(Box::new(move |sender, _args| {
                if let Some(sender) = sender {
                    let (count, active) = reads(&sender);
                    index_report(count, active);
                }
                Ok(())
            }));
        find.add_ActiveMatchIndexChanged(&index_handler, &mut index_token)
            .map_err(|e| e.to_string())?;

        let started = find.clone();
        let start_handler = FindStartCompletedHandler::create(Box::new(move |result| {
            if result.is_ok() {
                let (count, active) = reads(&started);
                report(count, active);
            }
            Ok(())
        }));
        if let Err(e) = find.Start(&options, &start_handler) {
            let _ = find.remove_MatchCountChanged(count_token);
            let _ = find.remove_ActiveMatchIndexChanged(index_token);
            return Err(e.to_string());
        }
        Ok(Session {
            find,
            count_token,
            index_token,
            target_id,
            target_url,
        })
    }
}
