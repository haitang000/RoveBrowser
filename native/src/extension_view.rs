//! A toolbar extension page runs in the browsing profile, outside Rove's UI renderer.
use std::{cell::Cell, rc::Rc};
use url::Url;
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2NewWindowRequestedEventArgs, ICoreWebView2NewWindowRequestedEventArgs3,
        COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED,
    },
    take_pwstr, AcceleratorKeyPressedEventHandler, NavigationCompletedEventHandler,
    NavigationStartingEventHandler, NewWindowRequestedEventHandler, WindowCloseRequestedEventHandler,
};
use windows::core::{Interface, BOOL, PWSTR};
use wry::{WebView, WebViewBuilder, WebViewExtWindows};

#[derive(Debug)]
pub enum Event {
    Loaded,
    Measured(Option<[u32; 2]>),
    Failed,
    Close,
    Link(String),
}

// Local pages may use their existing controller for same-window navigation.
// Any new tab requested by an extension is limited to its own origin or the web.
fn can_open_link(url: &Url, owner: &str) -> bool {
    crate::model::extension_id(url) == Some(owner)
        || (matches!(url.scheme(), "https" | "http")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none())
}

pub fn requested_by_extension(args: &ICoreWebView2NewWindowRequestedEventArgs, owner: &str) -> bool {
    let mut source = PWSTR::null();
    let result = unsafe {
        args.cast::<ICoreWebView2NewWindowRequestedEventArgs3>()
            .and_then(|args| args.OriginalSourceFrameInfo())
            .and_then(|frame| frame.Source(&mut source))
    };
    result.is_ok() && Url::parse(&take_pwstr(source))
        .is_ok_and(|url| crate::model::extension_id(&url) == Some(owner))
}

pub fn measure(view: &WebView, done: impl Fn(Event) + Send + 'static) -> wry::Result<()> {
    let zoom = crate::site::zoom_factor(view).unwrap_or(1.);
    view.evaluate_script_with_callback(include_str!("extension-size.js"), move |json| {
        let size = serde_json::from_str::<[f64; 2]>(&json).ok()
            .filter(|size| size.iter().all(|n| n.is_finite() && *n > 0.))
            .map(|[width, height]| [
                (width * zoom).ceil().clamp(25., 800.) as u32,
                (height * zoom).ceil().clamp(25., 600.) as u32,
            ]);
        done(Event::Measured(size));
    })
}

pub fn build(
    builder: WebViewBuilder<'_>,
    parent: &winit::window::Window,
    page: &Url,
    options_page: Option<&Url>,
    active_url: Option<&str>,
    bounds: wry::Rect,
    event: impl Fn(Event) + 'static,
) -> Result<WebView, Box<dyn std::error::Error>> {
    // url::Url treats chrome-extension as an opaque origin. The Chromium URL
    // origin includes the extension ID.
    let extension_id = page.host_str().ok_or("扩展页面地址无效")?.to_owned();
    let script = format!(
        "({})({}, {}, {});",
        include_str!("extension-view.js"),
        serde_json::json!(format!("chrome-extension://{extension_id}")),
        serde_json::json!(active_url),
        serde_json::json!(options_page.map(Url::as_str)),
    );
    let view = builder
        .with_initialization_script(script)
        .with_bounds(bounds)
        .with_visible(false)
        .with_focused(false)
        .with_hotkeys_zoom(false)
        .build_as_child(parent)?;
    let web = view.webview();
    let event = Rc::new(event);
    unsafe {
        let settings = web.Settings()?;
        settings.SetIsWebMessageEnabled(false)?;
        settings.SetAreDefaultContextMenusEnabled(false)?;
        // A requested tab can outlive the popup document's immediate event
        // handlers. Accept one destination before the host closes the view.
        let opened = Rc::new(Cell::new(false));
        let open_link = Rc::new({
            let extension_id = extension_id.clone();
            let event = event.clone();
            move |target: String| {
                if Url::parse(&target).is_ok_and(|url| can_open_link(&url, &extension_id))
                    && !opened.replace(true)
                {
                    event(Event::Link(target));
                }
            }
        });
        web.add_NavigationStarting(&NavigationStartingEventHandler::create(Box::new({
            let open_link = open_link.clone();
            let extension_id = extension_id.clone();
            move |_, args| {
                if let Some(args) = args {
                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let target = take_pwstr(uri);
                    let same_extension = Url::parse(&target)
                        .is_ok_and(|url| crate::model::extension_id(&url) == Some(extension_id.as_str()));
                    if !same_extension {
                        args.SetCancel(true)?;
                        let mut explicit = BOOL::default();
                        args.IsUserInitiated(&mut explicit)?;
                        if explicit.as_bool() {
                            open_link(target);
                        }
                    }
                }
                Ok(())
            }
        })), &mut 0)?;
        web.add_NavigationCompleted(&NavigationCompletedEventHandler::create(Box::new({
            let event = event.clone();
            move |_, args| {
                if let Some(args) = args {
                    let mut success = BOOL::default();
                    args.IsSuccess(&mut success)?;
                    if success.as_bool() {
                        event(Event::Loaded);
                    } else {
                        let mut status = Default::default();
                        args.WebErrorStatus(&mut status)?;
                        if status != COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED {
                            event(Event::Failed);
                        }
                    }
                }
                Ok(())
            }
        })), &mut 0)?;
        web.add_NewWindowRequested(&NewWindowRequestedEventHandler::create(Box::new({
            move |_, args| {
                if let Some(args) = args {
                    args.SetHandled(true)?;
                    let mut explicit = BOOL::default();
                    args.IsUserInitiated(&mut explicit)?;
                    if explicit.as_bool() {
                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        let target = take_pwstr(uri);
                        if !crate::model::is_extension_page(&target)
                            || requested_by_extension(&args, &extension_id)
                        {
                            open_link(target);
                        }
                    }
                }
                Ok(())
            }
        })), &mut 0)?;
        web.add_WindowCloseRequested(&WindowCloseRequestedEventHandler::create(Box::new({
            let event = event.clone();
            move |_, _| {
                event(Event::Close);
                Ok(())
            }
        })), &mut 0)?;
        view.controller().add_AcceleratorKeyPressed(&AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
            if let Some(args) = args {
                let mut kind = Default::default();
                let mut key = 0;
                args.KeyEventKind(&mut kind)?;
                args.VirtualKey(&mut key)?;
                if kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN && key == 27 {
                    args.SetHandled(true)?;
                    event(Event::Close);
                }
            }
            Ok(())
        })), &mut 0)?;
    }
    view.load_url(page.as_str())?;
    Ok(view)
}
