//! Render navigation failures in place, keeping the failed URL and native history.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use webview2_com::{
    take_pwstr, ExecuteScriptCompletedHandler, Microsoft::Web::WebView2::Win32::*,
    NavigationCompletedEventHandler, NavigationStartingEventHandler,
};
use windows::core::{Interface, BOOL, HSTRING, PWSTR};
use wry::{WebView, WebViewExtWindows};

#[derive(Default)]
pub struct ErrorPage {
    navigation: Cell<u64>,
    url: RefCell<String>,
    failed: Cell<bool>,
    dark: Cell<bool>,
}

impl ErrorPage {
    pub fn is_failed(&self) -> bool {
        self.failed.get()
    }

    pub fn set_theme(&self, view: &WebView, dark: bool) {
        self.dark.set(dark);
        if self.is_failed() {
            let _ = view.evaluate_script(&format!(
                "if (location.protocol === 'chrome-error:') document.documentElement.dataset.theme = '{}'", if dark { "dark" } else { "light" }
            ));
        }
    }
}

pub fn install(view: &WebView, dark: bool) -> windows::core::Result<Rc<ErrorPage>> {
    let state = Rc::new(ErrorPage::default());
    state.dark.set(dark);
    let starting_state = state.clone();
    let completed_state = state.clone();
    let starting = NavigationStartingEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            unsafe {
                let mut canceled = BOOL::default();
                args.Cancel(&mut canceled)?;
                if canceled.as_bool() {
                    return Ok(());
                }
                let mut id = 0;
                args.NavigationId(&mut id)?;
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                starting_state.navigation.set(id);
                *starting_state.url.borrow_mut() = take_pwstr(uri);
                starting_state.failed.set(false);
            }
        }
        Ok(())
    }));
    let completed = NavigationCompletedEventHandler::create(Box::new(move |sender, args| {
        let (Some(sender), Some(args)) = (sender, args) else {
            return Ok(());
        };
        unsafe {
            let mut id = 0;
            args.NavigationId(&mut id)?;
            if id != completed_state.navigation.get() {
                return Ok(());
            }
            let mut success = BOOL::default();
            args.IsSuccess(&mut success)?;
            if success.as_bool() {
                return Ok(());
            }
            let mut status = COREWEBVIEW2_WEB_ERROR_STATUS_UNKNOWN;
            args.WebErrorStatus(&mut status)?;
            let url = completed_state.url.borrow().clone();
            if status == COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED
                || !crate::model::is_web(&url)
            {
                return Ok(());
            }
            // HTTP errors can carry the site's own useful document. WebView2
            // reports IsSuccess=false for these as well as transport failures.
            if status == COREWEBVIEW2_WEB_ERROR_STATUS_UNKNOWN {
                let mut http_status = 0;
                if args
                    .cast::<ICoreWebView2NavigationCompletedEventArgs2>()
                    .is_ok_and(|args| args.HttpStatusCode(&mut http_status).is_ok())
                    && http_status >= 400
                {
                    return Ok(());
                }
            }
            completed_state.failed.set(true);
            let (description, code) = error_message(status);
            let data = serde_json::json!({
                "url": url, "description": description, "code": code,
                "theme": if completed_state.dark.get() { "dark" } else { "light" },
            });
            let html = serde_json::to_string(include_str!("error-page.html")).unwrap();
            let script = format!(
                r#"(() => {{
                if (location.protocol !== 'chrome-error:') return false;
                const data = {data};
                document.open(); document.write({html}); document.close();
                document.documentElement.dataset.theme = data.theme;
                document.getElementById('description').textContent = data.description;
                document.getElementById('address').textContent = data.url;
                document.getElementById('code').textContent = data.code;
                document.getElementById('retry').addEventListener('click', () => location.reload());
                return true;
            }})()"#
            );
            let handler = ExecuteScriptCompletedHandler::create(Box::new(|result, _| result));
            sender.ExecuteScript(&HSTRING::from(script), &handler)?;
        }
        Ok(())
    }));
    unsafe {
        let core = view.webview();
        core.Settings()?.SetIsBuiltInErrorPageEnabled(false)?;
        let mut token = 0;
        core.add_NavigationStarting(&starting, &mut token)?;
        core.add_NavigationCompleted(&completed, &mut token)?;
    }
    Ok(state)
}

fn error_message(status: COREWEBVIEW2_WEB_ERROR_STATUS) -> (&'static str, &'static str) {
    match status {
        COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED => (
            "找不到这个网站，请检查网址后重试。",
            "HOST_NAME_NOT_RESOLVED",
        ),
        COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED => {
            ("网络连接已断开，请检查网络后重试。", "DISCONNECTED")
        }
        COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT => ("网站响应时间过长，请稍后重试。", "TIMEOUT"),
        COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT
        | COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE => (
            "无法连接到这个网站，请检查网络或稍后重试。",
            "CANNOT_CONNECT",
        ),
        COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_ABORTED
        | COREWEBVIEW2_WEB_ERROR_STATUS_CONNECTION_RESET => {
            ("与网站的连接已中断，请重新加载。", "CONNECTION_INTERRUPTED")
        }
        COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_COMMON_NAME_IS_INCORRECT
        | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_EXPIRED
        | COREWEBVIEW2_WEB_ERROR_STATUS_CLIENT_CERTIFICATE_CONTAINS_ERRORS
        | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_REVOKED
        | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_IS_INVALID => (
            "网站的安全证书有问题，无法建立安全连接。",
            "CERTIFICATE_ERROR",
        ),
        COREWEBVIEW2_WEB_ERROR_STATUS_REDIRECT_FAILED => {
            ("网站重定向失败，请稍后重试。", "REDIRECT_FAILED")
        }
        COREWEBVIEW2_WEB_ERROR_STATUS_VALID_AUTHENTICATION_CREDENTIALS_REQUIRED => (
            "网站需要身份验证，请检查登录信息后重试。",
            "AUTHENTICATION_REQUIRED",
        ),
        COREWEBVIEW2_WEB_ERROR_STATUS_VALID_PROXY_AUTHENTICATION_REQUIRED => (
            "代理服务器需要身份验证，请检查代理设置后重试。",
            "PROXY_AUTHENTICATION_REQUIRED",
        ),
        _ => ("网页暂时无法加载，请稍后重试。", "LOAD_FAILED"),
    }
}
