//! Adapts Zigzag's PagePrinting.swift and BrowserActions.swift (Apache-2.0).
use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
use webview2_com::{
    take_pwstr,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Deferral, ICoreWebView2SaveAsUIShowingEventArgs, ICoreWebView2_16,
        ICoreWebView2_25, COREWEBVIEW2_PRINT_DIALOG_KIND_BROWSER,
        COREWEBVIEW2_SAVE_AS_KIND_DEFAULT, COREWEBVIEW2_SAVE_AS_KIND_SINGLE_FILE,
        COREWEBVIEW2_SAVE_AS_UI_RESULT_CANCELLED, COREWEBVIEW2_SAVE_AS_UI_RESULT_SUCCESS,
    },
    SaveAsUIShowingEventHandler, ShowSaveAsUICompletedHandler,
};
use windows::{
    core::{w, Interface, HSTRING, PWSTR},
    Win32::{
        Foundation::{ERROR_CANCELLED, HWND},
        System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
        UI::Shell::{
            FileSaveDialog, IFileSaveDialog, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR,
            FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
        },
    },
};
use wry::{WebView, WebViewExtWindows};

pub fn print(view: &WebView) -> Result<(), String> {
    let web = view
        .webview()
        .cast::<ICoreWebView2_16>()
        .map_err(|_| "请更新 WebView2 运行时以打印网页".to_owned())?;
    // The preview owns pagination, print styles, PDF output and dialog lifetime.
    unsafe { web.ShowPrintUI(COREWEBVIEW2_PRINT_DIALOG_KIND_BROWSER) }
        .map_err(|error| format!("无法打开打印预览：{error}"))
}

type SaveArgs = (ICoreWebView2SaveAsUIShowingEventArgs, ICoreWebView2Deferral);

#[derive(Clone, Default)]
/// A short-lived, single-use destination for this controller's download event.
pub struct SavePath(Rc<RefCell<Option<(PathBuf, Instant)>>>);

impl SavePath {
    pub fn take_matching(&self, path: &Path) -> bool {
        let mut selected = self.0.borrow_mut();
        let Some((destination, confirmed)) = selected.as_ref() else {
            return false;
        };
        if confirmed.elapsed() >= Duration::from_secs(30) {
            selected.take();
            return false;
        }
        if destination == path {
            selected.take();
            return true;
        }
        false
    }
}

pub struct SaveRequest {
    web: ICoreWebView2_25,
    token: i64,
    dialog: Rc<RefCell<Option<SaveArgs>>>,
    // PDF viewer saves report progress through the downloads panel.
    notify_saved: Rc<Cell<Option<bool>>>,
    path: SavePath,
}

impl SaveRequest {
    pub fn start(
        view: &WebView,
        path: SavePath,
        choose: impl Fn() -> bool + 'static,
        done: impl FnOnce(Result<bool, String>) + 'static,
    ) -> Result<Self, String> {
        let web = view
            .webview()
            .cast::<ICoreWebView2_25>()
            .map_err(|_| "请更新 WebView2 运行时以另存网页".to_owned())?;
        let dialog = Rc::new(RefCell::new(None));
        let notify_saved = Rc::new(Cell::new(None));
        let slot = dialog.clone();
        let showing = SaveAsUIShowingEventHandler::create(Box::new(move |_, args| {
            if let Some(args) = args {
                unsafe {
                    if slot.borrow().is_some() {
                        args.SetCancel(true)?;
                    } else {
                        let deferral = args.GetDeferral()?;
                        *slot.borrow_mut() = Some((args, deferral));
                        // A modal Windows dialog must run outside the COM callback.
                        if !choose() {
                            if let Some((args, deferral)) = slot.borrow_mut().take() {
                                let _ = args.SetCancel(true);
                                deferral.Complete()?;
                            }
                        }
                    }
                }
            }
            Ok(())
        }));
        let selected = notify_saved.clone();
        let save_path = path.clone();
        let completed = ShowSaveAsUICompletedHandler::create(Box::new(move |status, result| {
            let report = selected.get();
            let saved = match (status, result, report) {
                (Err(error), _, _) => Err(format!("无法另存网页：{error}")),
                (Ok(()), COREWEBVIEW2_SAVE_AS_UI_RESULT_CANCELLED, _) => Ok(false),
                (Ok(()), COREWEBVIEW2_SAVE_AS_UI_RESULT_SUCCESS, Some(notify)) => Ok(notify),
                (Ok(()), _, _) => Err("无法另存网页，请检查文件名和保存位置".into()),
            };
            if report.is_some()
                && (saved.is_err() || result == COREWEBVIEW2_SAVE_AS_UI_RESULT_CANCELLED)
            {
                save_path.0.borrow_mut().take();
            }
            done(saved);
            Ok(())
        }));
        let mut token = 0;
        unsafe { web.add_SaveAsUIShowing(&showing, &mut token) }
            .map_err(|error| format!("无法准备另存网页：{error}"))?;
        let request = Self {
            web,
            token,
            dialog,
            notify_saved,
            path,
        };
        unsafe { request.web.ShowSaveAsUI(&completed) }
            .map_err(|error| format!("无法开始另存网页：{error}"))?;
        Ok(request)
    }

    pub fn choose_path(&self, owner: HWND) -> windows::core::Result<()> {
        let Some((args, deferral)) = self.dialog.borrow_mut().take() else {
            return Ok(());
        };
        let result = self.pick_and_set_path(owner, &args);
        unsafe {
            if !matches!(result, Ok(true)) {
                let _ = args.SetCancel(true);
            }
            let completed = deferral.Complete();
            result.and(completed)
        }
    }

    fn pick_and_set_path(
        &self,
        owner: HWND,
        args: &ICoreWebView2SaveAsUIShowingEventArgs,
    ) -> windows::core::Result<bool> {
        unsafe {
            let mut mime = PWSTR::null();
            args.ContentMimeType(&mut mime)?;
            let mime = take_pwstr(mime);
            let archive = matches!(mime.as_str(), "text/html" | "application/xhtml+xml");
            let mut suggested = PWSTR::null();
            args.SaveAsFilePath(&mut suggested)?;
            let suggested = take_pwstr(suggested);
            let name = Path::new(&suggested)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("网页");
            let name = if archive {
                let mut title = PWSTR::null();
                self.web.DocumentTitle(&mut title)?;
                let title: String = take_pwstr(title)
                    .chars()
                    .take(140)
                    .map(|c| {
                        if c.is_control() || "<>:\"/\\|?*".contains(c) {
                            '-'
                        } else {
                            c
                        }
                    })
                    .collect();
                let title = title.trim_matches([' ', '.']);
                format!("{}.mhtml", if title.is_empty() { "网页" } else { title })
            } else {
                name.to_owned()
            };
            let dialog: IFileSaveDialog =
                CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetOptions(
                dialog.GetOptions()?
                    | FOS_FORCEFILESYSTEM
                    | FOS_NOCHANGEDIR
                    | FOS_OVERWRITEPROMPT
                    | FOS_PATHMUSTEXIST,
            )?;
            dialog.SetTitle(w!("另存网页"))?;
            dialog.SetFileName(&HSTRING::from(&name))?;
            if let Some(extension) = Path::new(&name).extension() {
                dialog.SetDefaultExtension(&HSTRING::from(extension))?;
            }
            match dialog.Show(Some(owner)) {
                Err(error) if error.code() == ERROR_CANCELLED.to_hresult() => return Ok(false),
                result => result?,
            }
            let path = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = take_pwstr(path);
            args.SetKind(if archive {
                COREWEBVIEW2_SAVE_AS_KIND_SINGLE_FILE
            } else {
                COREWEBVIEW2_SAVE_AS_KIND_DEFAULT
            })?;
            args.SetSaveAsFilePath(&HSTRING::from(&path))?;
            args.SetAllowReplace(true)?;
            args.SetSuppressDefaultDialog(true)?;
            // DownloadStarting for a PDF can arrive after ShowSaveAsUI completes.
            *self.path.0.borrow_mut() = Some((PathBuf::from(path), Instant::now()));
            self.notify_saved.set(Some(mime != "application/pdf"));
            Ok(true)
        }
    }
}

impl Drop for SaveRequest {
    fn drop(&mut self) {
        unsafe {
            let _ = self.web.remove_SaveAsUIShowing(self.token);
            if let Some((args, deferral)) = self.dialog.borrow_mut().take() {
                let _ = args.SetCancel(true);
                let _ = deferral.Complete();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chosen_download_path_matches_once_and_expires() {
        let pending = SavePath::default();
        let chosen = Path::new(r"C:\Export\receipt.pdf");
        *pending.0.borrow_mut() = Some((chosen.to_path_buf(), Instant::now()));
        assert!(!pending.take_matching(Path::new(r"C:\Downloads\receipt.pdf")));
        assert!(pending.take_matching(chosen));
        assert!(!pending.take_matching(chosen));

        let expired = Instant::now() - Duration::from_secs(30);
        *pending.0.borrow_mut() = Some((chosen.to_path_buf(), expired));
        assert!(!pending.take_matching(chosen));
    }
}
