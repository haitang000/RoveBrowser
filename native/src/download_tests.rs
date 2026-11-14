//! Query an operation without delivering any StateChanged notifications. This
//! exercises the same reconciliation used after registration and by the timer.
use super::*;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::{implement, Ref, Result};

#[implement(ICoreWebView2DownloadOperation)]
struct Operation {
    state: Rc<Cell<COREWEBVIEW2_DOWNLOAD_STATE>>,
    reason: Rc<Cell<COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON>>,
}

macro_rules! unused_methods {
    ($($name:ident($($arg:ident: $ty:ty),*);)*) => {
        $(fn $name(&self, $(_: $ty),*) -> Result<()> {
            Err(windows::Win32::Foundation::E_NOTIMPL.into())
        })*
    };
}

impl ICoreWebView2DownloadOperation_Impl for Operation_Impl {
    fn State(&self, value: *mut COREWEBVIEW2_DOWNLOAD_STATE) -> Result<()> {
        unsafe {
            *value = self.state.get();
        }
        Ok(())
    }
    fn InterruptReason(&self, value: *mut COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON) -> Result<()> {
        unsafe {
            *value = self.reason.get();
        }
        Ok(())
    }
    fn BytesReceived(&self, value: *mut i64) -> Result<()> {
        unsafe {
            *value = 100;
        }
        Ok(())
    }
    fn TotalBytesToReceive(&self, value: *mut i64) -> Result<()> {
        unsafe {
            *value = 100;
        }
        Ok(())
    }
    unused_methods! {
        add_BytesReceivedChanged(handler: Ref<'_, ICoreWebView2BytesReceivedChangedEventHandler>, token: *mut i64);
        remove_BytesReceivedChanged(token: i64);
        add_EstimatedEndTimeChanged(handler: Ref<'_, ICoreWebView2EstimatedEndTimeChangedEventHandler>, token: *mut i64);
        remove_EstimatedEndTimeChanged(token: i64);
        add_StateChanged(handler: Ref<'_, ICoreWebView2StateChangedEventHandler>, token: *mut i64);
        remove_StateChanged(token: i64);
        Uri(value: *mut PWSTR);
        ContentDisposition(value: *mut PWSTR);
        MimeType(value: *mut PWSTR);
        EstimatedEndTime(value: *mut PWSTR);
        ResultFilePath(value: *mut PWSTR);
        Cancel();
        Pause();
        Resume();
        CanResume(value: *mut BOOL);
    }
}

struct Fixture {
    state: Rc<Cell<COREWEBVIEW2_DOWNLOAD_STATE>>,
    reason: Rc<Cell<COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON>>,
    operation: ICoreWebView2DownloadOperation,
    downloads: RefCell<Downloads>,
    ops: RefCell<HashMap<String, ICoreWebView2DownloadOperation>>,
}

impl Fixture {
    fn new() -> Self {
        let state = Rc::new(Cell::new(COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS));
        let reason = Rc::new(Cell::new(COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE));
        let operation: ICoreWebView2DownloadOperation = Operation {
            state: state.clone(),
            reason: reason.clone(),
        }
        .into();
        let mut downloads = Downloads::default();
        downloads.insert(DownloadItem {
            id: "file".into(),
            url: "https://example.com/file".into(),
            path: "file.bin".into(),
            filename: "file.bin".into(),
            received: 0,
            total: 100,
            state: DownloadState::InProgress,
            failure: None,
            started: 1,
            finished: None,
            exists: false,
        });
        let ops = RefCell::new(HashMap::from([("file".into(), operation.clone())]));
        Self {
            state,
            reason,
            operation,
            downloads: RefCell::new(downloads),
            ops,
        }
    }
    fn sync(&self) -> bool {
        sync_download(&self.operation, "file", &self.downloads, &self.ops).unwrap()
    }
    fn item(&self) -> DownloadItem {
        self.downloads.borrow().get("file").unwrap().clone()
    }
}

#[test]
fn full_bytes_wait_for_engine_completion_then_recover_without_an_event() {
    let fixture = Fixture::new();
    assert!(!fixture.sync());
    assert_eq!(fixture.item().received, 100);
    assert_eq!(fixture.item().state, DownloadState::InProgress);
    assert!(fixture.item().finished.is_none());
    assert_eq!(fixture.ops.borrow().len(), 1);

    fixture.state.set(COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED);
    assert!(fixture.sync());
    assert_eq!(fixture.item().state, DownloadState::Finished);
    assert!(fixture.item().finished.is_some());
    assert!(fixture.ops.borrow().is_empty());
    assert!(!fixture.sync());
}

#[test]
fn already_completed_download_is_reconciled_on_the_first_read() {
    let fixture = Fixture::new();
    fixture.state.set(COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED);
    assert!(fixture.sync());
    assert_eq!(fixture.item().state, DownloadState::Finished);
    assert!(fixture.ops.borrow().is_empty());
}

#[test]
fn polling_preserves_pause_and_detects_resume_and_failure() {
    let fixture = Fixture::new();
    fixture.state.set(COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED);
    fixture
        .reason
        .set(COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED);
    assert!(fixture.sync());
    assert_eq!(fixture.item().state, DownloadState::Paused);
    assert!(!fixture.sync());
    assert_eq!(fixture.ops.borrow().len(), 1);
    fixture.state.set(COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS);
    assert!(fixture.sync());
    assert_eq!(fixture.item().state, DownloadState::InProgress);
    fixture.state.set(COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED);
    fixture
        .reason
        .set(COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NETWORK_FAILED);
    assert!(fixture.sync());
    assert_eq!(fixture.item().state, DownloadState::Failed);
    assert_eq!(fixture.item().failure.as_deref(), Some("网络错误"));
    assert!(fixture.ops.borrow().is_empty());
}
