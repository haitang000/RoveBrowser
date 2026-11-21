//! A toolbar extension page runs in the browsing profile, outside Rove's UI renderer.
use std::rc::Rc;
use url::Url;
use webview2_com::{
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED,
    },
    take_pwstr, AcceleratorKeyPressedEventHandler, NavigationCompletedEventHandler,
    NavigationStartingEventHandler, NewWindowRequestedEventHandler, WindowCloseRequestedEventHandler,
};
use windows::core::{BOOL, PWSTR};
use wry::{WebView, WebViewBuilder, WebViewExtWindows};

#[derive(Debug)]
pub enum Event {
    Loaded,
    Measured(Option<[u32; 2]>),
    Failed,
    Close,
    Link(String),
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
    active_url: Option<&str>,
    bounds: wry::Rect,
    event: impl Fn(Event) + 'static,
) -> Result<WebView, Box<dyn std::error::Error>> {
    // url::Url treats chrome-extension as an opaque origin. The Chromium URL
    // origin includes the extension ID.
    let extension_id = page.host_str().ok_or("扩展页面地址无效")?.to_owned();
    let script = active_url.map(|url| {
        format!(
            "({})({}, {});",
            include_str!("extension-view.js"),
            serde_json::json!(format!("chrome-extension://{extension_id}")),
            serde_json::json!(url),
        )
    });
    let builder = match script {
        Some(script) => builder.with_initialization_script(script),
        None => builder,
    };
    let view = builder
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
        web.add_NavigationStarting(&NavigationStartingEventHandler::create(Box::new({
            let event = event.clone();
            move |_, args| {
                if let Some(args) = args {
                    let mut uri = PWSTR::null();
                    args.Uri(&mut uri)?;
                    let target = take_pwstr(uri);
                    let same_extension = Url::parse(&target).is_ok_and(|url| {
                        url.scheme() == "chrome-extension"
                            && url.host_str() == Some(extension_id.as_str())
                            && url.username().is_empty()
                            && url.password().is_none()
                            && url.port().is_none()
                    });
                    if !same_extension {
                        args.SetCancel(true)?;
                        let mut explicit = BOOL::default();
                        args.IsUserInitiated(&mut explicit)?;
                        if explicit.as_bool() && crate::model::is_web(&target) {
                            event(Event::Link(target));
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
            let event = event.clone();
            move |_, args| {
                if let Some(args) = args {
                    args.SetHandled(true)?;
                    let mut explicit = BOOL::default();
                    args.IsUserInitiated(&mut explicit)?;
                    if explicit.as_bool() {
                        let mut uri = PWSTR::null();
                        args.Uri(&mut uri)?;
                        let url = take_pwstr(uri);
                        if crate::model::is_web(&url) {
                            event(Event::Link(url));
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
