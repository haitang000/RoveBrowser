//! Agent-owned WebViews live outside the browser's tab, focus and layout state.
use crate::agent_bridge::{web_url, BackgroundCommand as Command, Response};
use serde_json::{json, Value};
use std::{cell::Cell, path::PathBuf, rc::Rc};
use webview2_com::{
    take_pwstr, BasicAuthenticationRequestedEventHandler, ClientCertificateRequestedEventHandler,
    DownloadStartingEventHandler, LaunchingExternalUriSchemeEventHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_10, ICoreWebView2_13, ICoreWebView2_18, ICoreWebView2_4, ICoreWebView2_5,
        ICoreWebView2_8, COREWEBVIEW2_PERMISSION_STATE_DENY,
    },
    NavigationCompletedEventHandler, NavigationStartingEventHandler,
    PermissionRequestedEventHandler,
};
use windows::core::{Interface, BOOL, PWSTR};
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    window::Window,
};
use wry::{
    NewWindowResponse, WebContext, WebView, WebViewBuilderExtWindows, WebViewExtWindows,
};

const MAX_TABS: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Load {
    Queued,
    Loading(u64),
    Ready,
    Failed,
}

struct Tab {
    id: String,
    view: WebView,
    load: Rc<Cell<Load>>,
}

pub struct Tabs {
    tabs: Vec<Tab>,
    context: WebContext,
    font_fingerprint_protection: bool,
}

impl Tabs {
    pub fn new(data: PathBuf, font_fingerprint_protection: bool) -> Self {
        Self {
            tabs: Vec::new(),
            context: WebContext::new(Some(data)),
            font_fingerprint_protection,
        }
    }

    pub fn run(&mut self, parent: &Window, command: Command) -> Response {
        match self.execute(parent, command) {
            Ok(result) => Response::ok(result),
            Err(code) => Response::error(code),
        }
    }

    fn execute(&mut self, parent: &Window, command: Command) -> Result<Value, &'static str> {
        match command {
            Command::List {} => {
                Ok(json!({"tabs": self.tabs.iter().map(Tab::snapshot).collect::<Vec<_>>()}))
            }
            Command::Open { url } => {
                let url = web_url(&url).ok_or("invalid_url")?;
                if self.tabs.len() >= MAX_TABS {
                    return Err("tab_limit");
                }
                let mut builder = crate::web_settings::browsing_builder(
                    &mut self.context,
                    "--autoplay-policy=user-gesture-required".into(),
                    self.font_fingerprint_protection,
                )
                    .with_visible(false)
                    .with_focused(false)
                    .with_incognito(true)
                    .with_devtools(false)
                    .with_autoplay(false)
                    .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
                    .with_bounds(wry::Rect {
                        position: LogicalPosition::new(0, 0).into(),
                        size: LogicalSize::new(1024, 768).into(),
                    });
                if let Some(tab) = self.tabs.first() {
                    builder = builder.with_environment(tab.view.environment());
                }
                let view = builder
                    .build_as_child(parent)
                    .map_err(|_| "webview_unavailable")?;
                let load = Rc::new(Cell::new(Load::Queued));
                configure(&view, &load).map_err(|_| "webview_unavailable")?;
                let tab = Tab {
                    id: uuid::Uuid::new_v4().to_string(),
                    view,
                    load,
                };
                tab.navigate(&url)?;
                let result = json!({"id": tab.id});
                self.tabs.push(tab);
                Ok(result)
            }
            Command::Navigate { id, url } => {
                let url = web_url(&url).ok_or("invalid_url")?;
                self.tabs
                    .iter()
                    .find(|tab| tab.id == id)
                    .ok_or("tab_not_found")?
                    .navigate(&url)?;
                Ok(json!({"id": id}))
            }
            Command::Close { id } => {
                let index = self
                    .tabs
                    .iter()
                    .position(|tab| tab.id == id)
                    .ok_or("tab_not_found")?;
                self.tabs.remove(index);
                Ok(json!({"id": id}))
            }
        }
    }
}

impl Tab {
    fn navigate(&self, url: &str) -> Result<(), &'static str> {
        let previous = self.load.get();
        let same_document = previous == Load::Ready
            && self
                .view
                .url()
                .is_ok_and(|current| current.split('#').next() == url.split('#').next());
        // Same-document requests may update Source without navigation events.
        // If WebView2 actually reloads, NavigationStarting still sets Loading.
        if !same_document {
            self.load.set(Load::Queued);
        }
        self.view.load_url(url).map_err(|_| {
            self.load.set(previous);
            "navigation_rejected"
        })
    }

    fn snapshot(&self) -> Value {
        let load = self.load.get();
        let status = match load {
            Load::Queued | Load::Loading(_) => "loading",
            Load::Ready => "ready",
            Load::Failed => "failed",
        };
        // Do not return the last document as the result of a pending or failed navigation.
        let url = (load == Load::Ready)
            .then(|| web_url(&self.view.url().ok()?))
            .flatten();
        let title = url.as_ref().and_then(|_| {
            let mut title = PWSTR::null();
            unsafe {
                self.view.webview().DocumentTitle(&mut title).ok()?;
            }
            Some(take_pwstr(title).chars().take(256).collect::<String>())
        });
        json!({"id": self.id, "load": status, "url": url, "title": title})
    }
}

fn configure(view: &WebView, load: &Rc<Cell<Load>>) -> windows::core::Result<()> {
    let web = view.webview();
    let mut token = 0;
    unsafe {
        let mut private = BOOL::default();
        web.cast::<ICoreWebView2_13>()?
            .Profile()?
            .IsInPrivateModeEnabled(&mut private)?;
        if !private.as_bool() {
            return Err(windows::core::Error::from(
                windows::Win32::Foundation::E_FAIL,
            ));
        }
        let settings = web.Settings()?;
        settings.SetIsWebMessageEnabled(false)?;
        settings.SetAreDefaultScriptDialogsEnabled(false)?;
        web.cast::<ICoreWebView2_8>()?.SetIsMuted(true)?;
        web.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                if let Some(args) = args {
                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
                }
                Ok(())
            })),
            &mut token,
        )?;
        web.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
            &DownloadStartingEventHandler::create(Box::new(|_, args| {
                if let Some(args) = args {
                    args.SetHandled(true)?;
                    args.SetCancel(true)?;
                }
                Ok(())
            })),
            &mut token,
        )?;
        web.cast::<ICoreWebView2_18>()?
            .add_LaunchingExternalUriScheme(
                &LaunchingExternalUriSchemeEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        args.SetCancel(true)?;
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        web.cast::<ICoreWebView2_10>()?
            .add_BasicAuthenticationRequested(
                &BasicAuthenticationRequestedEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        args.SetCancel(true)?;
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        web.cast::<ICoreWebView2_5>()?
            .add_ClientCertificateRequested(
                &ClientCertificateRequestedEventHandler::create(Box::new(|_, args| {
                    if let Some(args) = args {
                        args.SetHandled(true)?;
                        args.SetCancel(true)?;
                    }
                    Ok(())
                })),
                &mut token,
            )?;
        let started = load.clone();
        web.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut id = 0;
                    args.NavigationId(&mut id)?;
                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    if web_url(&take_pwstr(uri)).is_some() {
                        started.set(Load::Loading(id));
                    } else {
                        args.SetCancel(true)?;
                        if started.get() == Load::Loading(id) {
                            started.set(Load::Failed);
                        }
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
        let completed = load.clone();
        web.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                if let Some(args) = args {
                    let mut id = 0;
                    args.NavigationId(&mut id)?;
                    if completed.get() == Load::Loading(id) {
                        let mut success = BOOL::default();
                        args.IsSuccess(&mut success)?;
                        completed.set(if success.as_bool() {
                            Load::Ready
                        } else {
                            Load::Failed
                        });
                    }
                }
                Ok(())
            })),
            &mut token,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_bounded_http_urls_without_credentials() {
        assert_eq!(
            web_url("https://example.com").as_deref(),
            Some("https://example.com/")
        );
        for url in [
            "about:blank",
            "file:///secret",
            "https://user:pass@example.com",
            "https://example.com/\n",
            "javascript:void(0)",
        ] {
            assert!(web_url(url).is_none(), "{url}");
        }
        assert!(web_url(&format!("https://example.com/{}", "a".repeat(8192))).is_none());
    }
}
