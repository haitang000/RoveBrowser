use crate::{
    chrome_store,
    clip::WindowClip,
    downloads::{DownloadItem, DownloadState, Downloads},
    extensions,
    model::{self, Command, PeekPhase, Point, Rect, State, Tab},
    permissions,
    storage::Storage,
};
use include_dir::{include_dir, Dir};
use serde_json::json;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    path::{Path as FilePath, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
use webview2_com::{
    take_pwstr, AcceleratorKeyPressedEventHandler, BytesReceivedChangedEventHandler, DOMContentLoadedEventHandler,
    ContainsFullScreenElementChangedEventHandler, DownloadStartingEventHandler, FaviconChangedEventHandler,
    FocusChangedEventHandler, IsDocumentPlayingAudioChangedEventHandler, StateChangedEventHandler,
    TrySuspendCompletedHandler, ZoomFactorChangedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2DownloadOperation, ICoreWebView2NavigationStartingEventArgs3,
        ICoreWebView2NewWindowRequestedEventArgs2, ICoreWebView2NewWindowRequestedEventArgs3,
        ICoreWebView2_15, ICoreWebView2_2, ICoreWebView2_3,
        ICoreWebView2_4, ICoreWebView2_9,
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE,
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED,
        COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED,
        COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED, COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED,
        COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
        COREWEBVIEW2_NAVIGATION_KIND_NEW_DOCUMENT,
    },
    NavigationStartingEventHandler, NewWindowRequestedEventHandler, SourceChangedEventHandler,
};
use windows::{
    core::{Interface, BOOL, PWSTR, HSTRING},
    Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::Gdi::ScreenToClient,
        UI::{
            Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT},
            Shell::ShellExecuteW,
            WindowsAndMessaging::{
                GetCursorPos, GetGUIThreadInfo, IsChild, IsWindowVisible, SetWindowPos,
                GUITHREADINFO, HWND_BOTTOM, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                SWP_NOZORDER, SW_SHOWNORMAL,
            },
        },
    },
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    platform::windows::WindowAttributesExtWindows,
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{Fullscreen, Icon, Window, WindowId},
};
use wry::{
    PageLoadEvent, WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows,
};

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../dist");
// Match theme.css --surface, including the empty frame before a document paints.
fn app_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let image = image::load_from_memory(include_bytes!("../assets/icon.png"))?.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(Icon::from_rgba(image.into_raw(), width, height)?)
}
fn surface_color(dark: bool) -> wry::RGBA {
    if dark {
        (30, 30, 32, 255)
    } else {
        (255, 255, 255, 255)
    }
}
fn browser_args(ui: bool) -> String {
    #[allow(unused_mut)]
    let mut args = String::from("--autoplay-policy=user-gesture-required");
    // Never compile a debugging endpoint into release builds.
    #[cfg(debug_assertions)]
    if let Some(port) = std::env::var("ERA_DEBUG_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|p| *p >= 1024 && *p < 65535)
    {
        args.push_str(&format!(
            " --remote-debugging-address=127.0.0.1 --remote-debugging-port={}",
            port + if ui { 0 } else { 1 }
        ));
    }
    #[cfg(not(debug_assertions))]
    let _ = ui;
    args
}
fn browsing_webview(context: &mut WebContext) -> WebViewBuilder<'_> {
    WebViewBuilder::new_with_web_context(context)
        .with_additional_browser_args(browser_args(false))
        .with_browser_extensions_enabled(true)
}
#[derive(Debug)]
enum Event {
    Agent(crate::agent_bridge::Request),
    ExternalUrls(Vec<String>),
    ImportSources(Vec<crate::import::Source>),
    ImportRead(Vec<crate::import::Batch>),
    ExtensionPrepared(uuid::Uuid, Result<extensions::Prepared, String>),
    ExtensionBackground(uuid::Uuid, Rect, String),
    ExtensionView(uuid::Uuid, crate::extension_view::Event),
    RestoreKeyboardFocus,
    Layout,
    SplitResize,
    SidebarResize,
    SplitExited(u64),
    DataCleared(String, bool),
    HistoryCleared(bool),
    Command(Command),
    Title(String, String),
    Load(String, String, bool),
    Source(String, String),
    PageFullscreen(String, isize, bool),
    Favicon(String, String, String),
    /// Icon links the loaded page declares, for the sharper fetch that follows.
    FaviconLinks(String, String, Vec<crate::favicon::Link>),
    /// The fetched icon of a page's site; `None` when the site had none.
    SiteIcon(String, String, Option<String>),
    /// Site icons fetched for saved addresses: the one-time legacy icon upgrade.
    IconsRefreshed(Vec<(String, Option<String>)>),
    Shortcut(String, bool),
    Open(String),
    PeekRequest(String, String, Option<Point>),
    PeekSurface(String, u64, Option<String>),
    Suggestions(u64, Vec<String>),
    CaptureTabs,
    Thumbnail(String, String, u64, Option<String>),
    PreviewLoad(String, u64, String, bool),
    PreviewThumbnail(String, u64, u64, Option<String>),
    PreviewCapture,
    FindMatches(i32, i32),
    ChoosePageSavePath,
    PageSaveFinished(Result<bool, String>),
    ReaderAvailable(String, String, bool),
    ReaderArticle(String, String, u64, Option<crate::reader::Article>),
    ReaderLink(String, String),
    ReaderDeadline(u64),
    ReaderReady(u64),
    ReaderMotionDone(u64, bool),
    ReaderMotionFailed(u64),
    ReaderResources(u64, u8),
    /// A page's controller reported a new zoom factor.
    Zoom(String, isize, f64),
    /// A page started or stopped playing sound.
    Audio(String, bool),
    /// The download list changed; persist only for starts and state changes,
    /// not for every bytes tick.
    DownloadsChanged {
        persist: bool,
    },
    /// A new download joined the list and needs its initial state published.
    DownloadStarted,
    /// Zigzag `TabSleep`: every tick, background pages nobody has seen for a
    /// while give their memory back.
    SleepTick,
    /// A background page's `TrySuspend` finished; `true` when it slept.
    SleepDone(String, bool),
    StorageWriteError(String),
    PermissionsChanged,
    UpdateChecked(Result<crate::updates::Offer, crate::updates::Report>),
    UpdateProgress(crate::updates::Report),
    UpdatePrepared(Result<(crate::update_install::PreparedUpdate, crate::updates::Report), crate::updates::Report>),
}
struct Page {
    permission_document: Rc<permissions::Document>,
    view: WebView,
    save_path: crate::page_output::SavePath,
    resize_surface: Rc<crate::resize_surface::Surface>,
    url: String,
    clip: Rc<WindowClip>,
    thumbnail_version: u64,
    thumbnail_attempt: Option<u64>,
    thumbnail_pending: bool,
    visible: Cell<bool>,
    bounds: Rc<Cell<Option<[i32; 4]>>>,
    viewport: Rc<Cell<Option<[i32; 2]>>>,
    dark: Cell<bool>,
    transparent: bool,
    zoom_default: Cell<f64>,
    navigation: Rc<NavigationPolicy>,
    error_page: Rc<crate::error_page::ErrorPage>,
    /// The page is suspended (`ICoreWebView2_3::TrySuspend`). Runtime only.
    sleeping: Rc<Cell<bool>>,
    /// A suspension is in flight; do not ask for another.
    sleep_pending: Rc<Cell<bool>>,
    /// A suspension that failed is not retried for `sleep::RETRY_AFTER`.
    sleep_failed_at: Cell<Option<Instant>>,
}
struct NavigationPolicy {
    bookmarked: Cell<bool>,
    current: RefCell<String>,
    host_navigation: Cell<bool>,
}
impl Page {
    fn exit_fullscreen(&self) -> wry::Result<()> {
        self.view.evaluate_script("if (document.fullscreenElement) document.exitFullscreen()")
    }
    fn split_pane(&self) -> crate::split::Pane {
        crate::split::Pane { hwnd: self.view.hwnd(), controller: self.view.controller(),
            surface: self.resize_surface.clone(),
            clip: self.clip.clone(), bounds: self.bounds.clone(), viewport: self.viewport.clone() }
    }
    fn set_window_bounds(&self, bounds: wry::Rect, scale: f64) -> wry::Result<()> {
        let position = bounds.position.to_physical::<i32>(scale);
        let size = bounds.size.to_physical::<i32>(scale);
        let next = [position.x, position.y, size.width, size.height];
        if self.bounds.get() != Some(next) {
            if self.resize_surface.relocating() {
                // Keep the original renderer frozen until its snapshot arrives.
                self.bounds.set(Some(next));
                return Ok(());
            }
            if self.resize_surface.dragging() {
                self.resize_surface.resize(next)?;
                self.bounds.set(Some(next));
                return Ok(());
            }
            self.resize_surface.end();
            // Move the HWND before asking Chromium to reflow. Wry does these in
            // the opposite order, making visible edges wait on renderer work.
            unsafe { SetWindowPos(self.view.hwnd(), None, position.x, position.y,
                size.width, size.height, SWP_NOACTIVATE | SWP_NOZORDER)?; }
            self.bounds.set(Some(next));
        }
        Ok(())
    }
    fn sync_viewport(&self) -> wry::Result<()> {
        if self.resize_surface.dragging() { return Ok(()); }
        if let Some([_, _, width, height]) = self.bounds.get() {
            if self.viewport.get() != Some([width, height]) {
                // SAFETY: controller belongs to this live Page and runs on the
                // same UI thread. Coordinates are validated physical pixels.
                unsafe { self.view.controller().SetBounds(RECT {
                    left: 0, top: 0, right: width, bottom: height,
                })?; }
                self.viewport.set(Some([width, height]));
            }
        }
        Ok(())
    }
    fn set_visible(&self, visible: bool) -> wry::Result<()> {
        if self.visible.get() != visible {
            self.view.set_visible(visible)?;
            self.visible.set(visible);
        }
        Ok(())
    }
    fn set_theme(&self, dark: bool) -> wry::Result<()> {
        if self.dark.get() != dark {
            self.view.set_theme(if dark {
                wry::Theme::Dark
            } else {
                wry::Theme::Light
            })?;
            self.view.set_background_color(if self.transparent { (0, 0, 0, 0) } else { surface_color(dark) })?;
            self.dark.set(dark);
            self.error_page.set_theme(&self.view, dark);
        }
        Ok(())
    }
    fn set_zoom(&self, factor: f64) -> windows::core::Result<()> {
        crate::site::set_zoom_factor(&self.view, factor)?;
        self.zoom_default.set(factor);
        Ok(())
    }
    fn restore_zoom(&self, factor: f64) {
        if (self.zoom_default.get() - factor).abs() > 0.001
            || crate::site::zoom_factor(&self.view)
                .is_ok_and(|current| (current - factor).abs() > 0.001)
        {
            let _ = self.set_zoom(factor);
        }
    }
    /// Shows the page again after it slept, and resumes it explicitly. WebView2
    /// also resumes a suspended page when it becomes visible; doing both keeps
    /// the wake immediate and the state certain.
    fn wake(&self) {
        if !self.sleeping.replace(false) {
            return;
        }
        if let Ok(web) = self.view.webview().cast::<ICoreWebView2_3>() {
            // SAFETY: the controller belongs to this live Page on the UI thread.
            let _ = unsafe { web.Resume() };
        }
    }
}
struct Peek {
    tab: Tab,
    page: Page,
    rect: Option<Rect>,
    shown: Cell<bool>,
    origin: Option<Point>,
    phase: PeekPhase,
    ticket: u64,
    surface: Option<String>,
    surface_sent: bool,
    opening_capture: bool,
    moving: bool,
}
#[derive(Clone, Copy)]
struct PageLayout {
    primary: Rect,
    secondary: Option<Rect>,
    sidebar: Option<Rect>,
    viewport: Option<crate::layout::Viewport>,
    live_resize: bool,
    sidebar_motion: bool,
}
/// The runtime state every page's events share. Bundled so registering a page
/// stays a short argument list as more event families move onto it.
#[derive(Clone)]
struct PageShared {
    peek_visible: Rc<Cell<bool>>,
    find_open: Rc<Cell<bool>>,
    downloads_open: Rc<Cell<bool>>,
    downloads: Rc<RefCell<Downloads>>,
    download_ops: Rc<RefCell<HashMap<String, ICoreWebView2DownloadOperation>>>,
    download_sources: Rc<RefCell<HashMap<String, String>>>,
    save_path: crate::page_output::SavePath,
}
/// Tabs removed together by Clear or Close Others/Below, with the active tab
/// before the batch so the Undo toast can put everything back as it was.
struct UndoBatch {
    entries: Vec<(String, usize, Tab)>,
    previous_active: String,
    /// A blank tab that closing everything auto-created; undo takes it back.
    auto_created: Option<String>,
}
struct ExtensionView {
    ticket: uuid::Uuid,
    view: WebView,
    fit_content: bool,
    ready: bool,
    visible: bool,
    size: Option<[u32; 2]>,
    next_measure: Option<Instant>,
}

struct App {
    _instance: crate::instance::Instance,
    agent_tabs: Option<crate::agent_tabs::Tabs>,
    agent_data: PathBuf,
    updating: bool,
    update_offer: Option<crate::updates::Offer>,
    update_message: Option<String>,
    external_urls: Vec<String>,
    import_sources: Vec<crate::import::Source>,
    import_busy: bool,
    extensions_root: PathBuf,
    extension_candidate: Option<extensions::Prepared>,
    extension_preparation: Option<uuid::Uuid>,
    extension_popup: Option<uuid::Uuid>,
    extension_error: Option<String>,
    extension_view: Option<ExtensionView>,
    // Drop child controllers before their parent HWND and shared context.
    split_handle: Option<crate::split::SplitHandle>,
    sidebar_handle: Option<crate::sidebar::SidebarHandle>,
    resize_appearance: crate::resize_surface::Appearance,
    presented_split: Option<(String, String)>,
    split_entry_pending: bool,
    split_exit: Option<SplitExit>,
    split_exit_ticket: u64,
    pages: HashMap<String, Page>,
    fullscreen_page: Option<(String, isize)>,
    reader: Option<ReaderSession>,
    reader_available: HashMap<String, bool>,
    reader_request: Option<(String, String, u64)>,
    reader_epoch: u64,
    data_clear_view: Option<WebView>,
    live_settings: Rc<RefCell<crate::settings::Settings>>,
    peek: Option<Peek>,
    peek_visible: Rc<Cell<bool>>,
    caption: Option<crate::caption::Caption>,
    glass_backdrop: Option<crate::glass::Backdrop>,
    thumbnails: crate::thumbnails::Cache,
    tab_preview: Option<(String, Rect)>,
    preview_renders: HashMap<String, PreviewRender>,
    preview_epoch: u64,
    find: Option<crate::find::Session>,
    find_open: Rc<Cell<bool>>,
    /// Tabs currently playing sound; Zigzag never treats those as stale.
    playing: std::collections::HashSet<String>,
    undo_batch: Option<UndoBatch>,
    downloads: Rc<RefCell<Downloads>>,
    /// Live WebView2 operations for pause/resume/cancel; dropped on completion.
    download_ops: Rc<RefCell<HashMap<String, ICoreWebView2DownloadOperation>>>,
    download_sources: Rc<RefCell<HashMap<String, String>>>,
    next_download_poll: Option<Instant>,
    downloads_open: Rc<Cell<bool>>,
    download_popup_visible: bool,
    pending_download_review: Option<String>,
    popup: Option<WebView>,
    popup_has_regions: bool,
    page_save: Option<crate::page_output::SaveRequest>,
    popup_state: serde_json::Map<String, serde_json::Value>,
    permissions: permissions::Requests,
    permission_state: serde_json::Value,
    permission_shown_at: Instant,
    ui: Option<WebView>,
    last_focused_view: Rc<Cell<HWND>>,
    window: Option<Window>,
    context: WebContext,
    state: State,
    storage: Storage,
    session_writer: crate::session_writer::SessionWriter,
    next_session_save: Option<Instant>,
    pending_publish: Cell<bool>,
    state_publisher: crate::state_snapshot::Publisher,
    proxy: EventLoopProxy<Event>,
    primary: Rect,
    secondary: Option<Rect>,
    pending_layout: Rc<Cell<Option<PageLayout>>>,
    layout_viewport: Option<crate::layout::Viewport>,
    sidebar_rect: Option<Rect>,
    sidebar_expected_width: Option<f64>,
    live_resize: bool,
    sidebar_motion: bool,
    overlay: bool,
    tab_drag_cutouts: [Option<Rect>; 2],
    quit_pending: bool,
    ready: bool,
    last_error: Option<String>,
    ui_data: PathBuf,
    glass: bool,
    /// Site icons fetched this session, one request per site.
    site_icons: crate::favicon::SiteIcons,
    /// A pre-upgrade session still holds upscaled icons; fetch each site once.
    icon_refresh: bool,
    /// At least one saved icon was actually replaced by that refresh.
    icon_refresh_found: bool,
    /// Zigzag `TabSleep`: pages waiting for a suspension turn, longest unseen
    /// first, and the thresholds this build runs with.
    sleep_queue: VecDeque<String>,
    sleep_thresholds: crate::sleep::Thresholds,
}
struct SplitExit {
    keeper: String,
    departing: String,
    ticket: u64,
    departed: bool,
    layout_ready: bool,
}
struct ReaderSession {
    id: String,
    url: String,
    original: Page,
    ticket: u64,
    ready: bool,
    closing: bool,
    transitioning: bool,
}
struct PreviewRender {
    view: WebView,
    error_page: Rc<crate::error_page::ErrorPage>,
    source: String,
    epoch: u64,
    loading: bool,
    pending: bool,
    version: u64,
}
pub fn run(update_message: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Keep the existing profile location so the rename preserves sessions and logins.
    let data = std::env::var_os("ERA_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            directories::ProjectDirs::from("", "Era", "EraBrowser")
                .map(|d| d.data_local_dir().to_path_buf())
        })
        .ok_or("无法定位用户数据目录")?;
    let external_urls = crate::instance::urls(std::env::args_os().skip(1))?;
    let Some(mut instance) = crate::instance::Instance::claim(&data, &external_urls)? else { return Ok(()); };
    let mut storage = Storage::open(&data)?;
    // A malformed session is retained, and startup stops instead of overwriting it.
    let mut state = storage.load()?;
    // Icons cached by an older build were stretched to 64 pixels and are soft;
    // the one-time refresh after the UI is ready re-fetches each saved site. The
    // marker is only written once that refresh found real icons, so an offline
    // first launch tries again next time.
    let icon_refresh = state.icons_need_upgrade();
    if !icon_refresh && state.icon_version != crate::favicon::ICON_VERSION {
        state.mark_icons_upgraded();
        storage.save(&state)?;
    }
    // Downloads that were running when the app quit become interruptions; the
    // repaired list is written back so a second launch shows the same thing.
    let downloads = Rc::new(RefCell::new(Downloads::load(
        storage.load_downloads().unwrap_or_default(),
    )));
    if let Err(e) = storage.save_downloads(&downloads.borrow().items) {
        eprintln!("Rove downloads: {e}");
    }
    let event_loop = EventLoop::<Event>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let agent_proxy = proxy.clone();
    if let Err(error) = crate::agent_bridge::start(move |request| agent_proxy.send_event(Event::Agent(request)).is_ok()) {
        eprintln!("Rove agent bridge disabled: {error}");
    }
    let save_proxy = proxy.clone();
    let session_writer = crate::session_writer::SessionWriter::new(&data, move |error| {
        let _ = save_proxy.send_event(Event::StorageWriteError(error));
    })?;
    let external_proxy = proxy.clone();
    instance.listen(move |urls| { let _ = external_proxy.send_event(Event::ExternalUrls(urls)); });
    // Zigzag `TabManager.sleepIdleTabs` runs on a timer; the same tick asks the
    // pages that have been out of sight for a while to give their memory back.
    {
        let sleep_proxy = proxy.clone();
        let every = crate::sleep::tick_interval();
        std::thread::spawn(move || loop {
            std::thread::sleep(every);
            if sleep_proxy.send_event(Event::SleepTick).is_err() {
                return;
            }
        });
    }
    let mut app = App {
        _instance: instance,
        agent_tabs: None,
        agent_data: data.join("AgentWebView2"),
        updating: false,
        update_offer: None,
        update_message,
        external_urls,
        import_sources: vec![],
        import_busy: false,
        extensions_root: std::path::absolute(data.join("Extensions"))?,
        extension_candidate: None,
        extension_preparation: None,
        extension_popup: None,
        extension_error: None,
        extension_view: None,
        split_handle: None,
        sidebar_handle: None,
        resize_appearance: crate::resize_surface::Appearance::default(),
        presented_split: None,
        split_entry_pending: false,
        split_exit: None,
        split_exit_ticket: 0,
        live_settings: Rc::new(RefCell::new(state.settings.clone())),
        data_clear_view: None,
        pages: HashMap::new(),
        fullscreen_page: None,
        reader: None,
        reader_available: HashMap::new(),
        reader_request: None,
        reader_epoch: 0,
        peek: None,
        peek_visible: Rc::new(Cell::new(false)),
        caption: None,
        glass_backdrop: None,
        thumbnails: crate::thumbnails::Cache::default(),
        tab_preview: None,
        preview_renders: HashMap::new(),
        preview_epoch: 0,
        find: None,
        find_open: Rc::new(Cell::new(false)),
        playing: std::collections::HashSet::new(),
        undo_batch: None,
        downloads,
        download_ops: Rc::new(RefCell::new(HashMap::new())),
        download_sources: Rc::new(RefCell::new(HashMap::new())),
        next_download_poll: None,
        downloads_open: Rc::new(Cell::new(false)),
        download_popup_visible: false,
        pending_download_review: None,
        popup: None,
        popup_has_regions: false,
        popup_state: serde_json::Map::new(),
        permissions: permissions::Requests::load(&storage)?,
        permission_state: json!({"type":"permissionState", "permissionPrompt":null, "permissionTabs":[]}),
        permission_shown_at: Instant::now(),
        page_save: None,
        ui: None,
        last_focused_view: Rc::default(),
        window: None,
        context: WebContext::new(Some(data.join("WebView2"))),
        state,
        storage,
        session_writer,
        next_session_save: None,
        pending_publish: Cell::new(false),
        state_publisher: crate::state_snapshot::Publisher::default(),
        proxy,
        primary: Rect::default(),
        secondary: None,
        pending_layout: Rc::new(Cell::new(None)),
        layout_viewport: None,
        sidebar_rect: None,
        sidebar_expected_width: None,
        live_resize: false,
        sidebar_motion: false,
        overlay: false,
        tab_drag_cutouts: [None; 2],
        quit_pending: false,
        ready: false,
        last_error: None,
        ui_data: data.join("UI"),
        glass: false,
        site_icons: crate::favicon::SiteIcons::default(),
        icon_refresh,
        icon_refresh_found: false,
        sleep_queue: VecDeque::new(),
        sleep_thresholds: crate::sleep::thresholds(),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
impl App {
    fn permission_pane(&self, request: &permissions::Request) -> Option<Rect> {
        if self.fullscreen_page.is_some() || self.quit_pending || self.overlay || self.downloads_open.get() || self.space_form_open() || !self.state.onboarding.completed {
            return None;
        }
        let (page, rect) = if let Some(peek) = &self.peek {
            if peek.phase != PeekPhase::Open { return None; }
            (&peek.page, peek.rect?)
        } else {
            let rect = if request.tab == self.state.active_tab {
                self.primary
            } else if self.state.active()?.split_with.as_deref() == Some(&request.tab) {
                self.secondary?
            } else {
                return None;
            };
            (self.pages.get(&request.tab)?, rect)
        };
        (rect.valid() && rect.width > 0. && rect.height > 0. && page.visible.get()
            && request.belongs_to(&page.permission_document)).then_some(rect)
    }

    fn publish_permission_rules(&self) {
        self.emit(json!({"type":"permissionRules", "rules": self.permissions.rules()}));
    }

    fn sync_permissions(&mut self) {
        self.permissions.reconcile();
        if self.permissions.waiting.is_empty()
            && self.permission_state["permissionTabs"].as_array().is_some_and(Vec::is_empty) {
            return;
        }
        let tabs: std::collections::BTreeSet<_> = self.permissions.waiting.iter().map(|r| &r.tab).collect();
        let prompt = self.permissions.waiting.iter().find_map(|request| {
            let rect = self.permission_pane(request)?;
            Some(json!({
                "id": request.id, "scope": request.scope, "canRemember": request.scope.can_remember(),
                "remember": request.scope.kind.remembers_by_default(),
                "pane": {"x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height},
                "waiting": self.permissions.waiting.len() - 1,
            }))
        });
        let state = json!({"type":"permissionState", "permissionPrompt": prompt, "permissionTabs": tabs});
        if state["permissionPrompt"]["id"] != self.permission_state["permissionPrompt"]["id"] {
            self.permission_shown_at = Instant::now();
        }
        if state != self.permission_state {
            self.emit(state.clone());
            self.permission_state = state;
        }
    }

    fn update_permission_rule(&mut self, scope: permissions::Scope, allowed: Option<bool>) -> bool {
        if let Err(error) = self.permissions.update(&self.storage, scope.clone(), allowed) {
            self.error(format!("无法保存网站权限（{error}）"));
            self.publish_permission_rules();
            return false;
        }
        for page in self.pages.values()
            .chain(self.peek.iter().map(|peek| &peek.page))
            .chain(self.reader.iter().map(|reader| &reader.original)) {
            page.permission_document.forget(&scope);
        }
        self.publish_permission_rules();
        true
    }

    fn answer_permission(&mut self, id: &str, allowed: Option<bool>, remember: bool) {
        self.sync_permissions();
        if self.permission_state["permissionPrompt"]["id"].as_str() != Some(id)
            || (allowed == Some(true) && self.permission_shown_at.elapsed() < Duration::from_millis(500)) {
            return;
        }
        let Some(index) = self.permissions.waiting.iter().position(|request| request.id == id) else { return };
        let request = self.permissions.waiting.remove(index).unwrap();
        if let Some(allowed) = allowed {
            let remembered = remember && request.scope.can_remember()
                && self.update_permission_rule(request.scope.clone(), Some(allowed));
            request.answer(allowed, !remembered);
        }
        self.sync_permissions();
    }

    fn publish_default_browser(&self) {
        self.emit(json!({"type":"defaultBrowser","defaultBrowser":crate::default_browser::status()}));
    }
    fn open_external_urls(&mut self, event_loop: Option<&ActiveEventLoop>) {
        if !self.state.onboarding.completed || !self.ready { return; }
        for url in std::mem::take(&mut self.external_urls) { self.command(Command::New { url }, event_loop); }
    }
    fn confirm_quit(&mut self) -> bool {
        if self.quit_pending { return false; }
        if !self.state.settings.ask_before_quit { return true; }
        self.close_extension_menu();
        if self.fullscreen_page.is_some() {
            if let Err(error) = self.restore_page_window(true).and_then(|_| self.sync_layout()) {
                self.error(error);
            }
        }
        self.finish_split_resize();
        self.dismiss_tab_preview();
        self.quit_pending = true;
        self.emit(json!({"type":"quitConfirmation","visible":true}));
        false
    }
    fn update_window_frame(&self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window) = &self.window else {
            return Ok(());
        };
        let maximized = window.is_maximized();
        if self.ready {
            if let Some(ui) = &self.ui {
                let radius = if maximized { 0. } else { self.caption.as_ref().map_or(12., |c| c.radius()) };
                let inset = crate::caption::chrome_inset(maximized);
                ui.evaluate_script(&format!(
                    "document.documentElement.style.setProperty('--radius-window', '{radius}px');document.documentElement.style.setProperty('--chrome-inset', '{inset}px')"
                ))?;
            }
        }
        Ok(())
    }

    /// Keep the clear appearance, with a light native blur where supported.
    fn apply_glass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window) = &self.window else {
            return Ok(());
        };
        let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else {
            return Ok(());
        };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        crate::glass::clear_system_backdrop(hwnd);
        if self.glass && self.glass_backdrop.is_none() {
            match crate::glass::Backdrop::new(hwnd, self.caption.as_ref().map_or(12., |c| c.radius())) {
                Ok(backdrop) => self.glass_backdrop = Some(backdrop),
                // Keep real clear transparency when Composition is unavailable.
                Err(error) => eprintln!("Rove background blur: {error}"),
            }
        }
        if let Some(backdrop) = &self.glass_backdrop {
            backdrop.set_enabled(self.glass)?;
        }
        if let Some(ui) = &self.ui {
            let color = if self.glass {
                (0, 0, 0, 0)
            } else {
                surface_color(self.state.theme == "dark")
            };
            ui.set_background_color(color)?;
        }
        Ok(())
    }

    fn emit(&self, value: serde_json::Value) {
        let message = HSTRING::from(value.to_string());
        if value["type"] != "shortcut" {
            if let Some(popup) = &self.popup {
                let _ = unsafe { popup.webview().PostWebMessageAsJson(&message) };
            }
        }
        if let Some(ui) = &self.ui {
            let _ = unsafe { ui.webview().PostWebMessageAsJson(&message) };
        }
    }
    fn error(&mut self, message: impl ToString) {
        let message = message.to_string();
        eprintln!("{message}");
        self.emit(json!({"type":"error","message":message}));
        self.last_error = Some(message)
    }
    fn publish(&self) {
        if self.ready {
            self.pending_publish.set(true);
        }
    }
    fn save(&mut self) {
        // Bound the delay from the first edit; continuous activity cannot defer
        // a save indefinitely. Only clone/serialize once per batch.
        self.next_session_save.get_or_insert_with(|| Instant::now() + Duration::from_millis(150));
    }
    fn flush_session(&mut self) -> bool {
        self.next_session_save = None;
        self.session_writer.save(self.state.clone());
        if let Err(error) = self.session_writer.flush() {
            self.error(format!("保存会话失败：{error}"));
            return false;
        }
        true
    }
    fn publish_update(&self, report: crate::updates::Report) {
        let offer_id = self.update_offer.as_ref().map(|offer| &offer.id);
        self.emit(json!({"type":"updateStatus","phase":report.phase,"channel":report.channel,"version":report.version,"message":report.message,"offerId":offer_id}));
    }
    /// One-time: replace icons an older build enlarged to 64 pixels. Each saved
    /// site is asked for its own icon in the background and the replacements
    /// arrive in small batches, so the sidebar fills in instead of freezing.
    fn refresh_saved_icons(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let mut targets: Vec<(String, String)> = Vec::new();
        let urls = self
            .state
            .tabs
            .iter()
            .map(|tab| &tab.url)
            .chain(self.state.pins.iter().map(|mark| &mark.url))
            .chain(self.state.favorites.iter().map(|mark| &mark.url));
        for url in urls {
            let Some(origin) = crate::favicon::origin_of(url) else {
                continue;
            };
            if !seen.insert(origin.clone()) || !self.site_icons.begin(&origin) {
                continue;
            }
            targets.push((origin, url.clone()));
        }
        if targets.is_empty() {
            return;
        }
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let mut batch = Vec::new();
            for (origin, url) in targets {
                batch.push((origin, crate::favicon::site_icon(&url)));
                if batch.len() >= 4
                    && proxy
                        .send_event(Event::IconsRefreshed(std::mem::take(&mut batch)))
                        .is_err()
                {
                    return;
                }
            }
            if !batch.is_empty() {
                let _ = proxy.send_event(Event::IconsRefreshed(batch));
            }
            // An empty batch ends the refresh; the host then decides whether the
            // session may record the upgrade (offline launches retry next time).
            let _ = proxy.send_event(Event::IconsRefreshed(Vec::new()));
        });
    }
    /// Fetches the icons a loaded page declares at their own resolution; the
    /// WebView2 capture that already arrived stays as the instant fallback.
    fn start_site_icon(&mut self, id: String, page_url: String, links: Vec<crate::favicon::Link>) {
        let Some(origin) = crate::favicon::origin_of(&page_url) else {
            return;
        };
        if links.is_empty() {
            return;
        }
        if let Some(icon) = self.site_icons.known(&origin).cloned() {
            // Another page of this site already produced its icon; reuse it.
            if self.state.cache_page_icon(&id, &page_url, icon) {
                self.save();
                self.publish();
            }
            return;
        }
        if !self.site_icons.begin(&origin) {
            return;
        }
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let icon = crate::favicon::resolve(&links, &page_url);
            let _ = proxy.send_event(Event::SiteIcon(id, page_url, icon));
        });
    }
    fn page(&self) -> Option<&WebView> {
        if let Some((id, hwnd)) = &self.fullscreen_page {
            if let Some(page) = self.page_with_handle(id, *hwnd) {
                return Some(&page.view);
            }
        }
        self.peek
            .as_ref()
            .map(|p| &p.page.view)
            .or_else(|| self.pages.get(&self.state.active_tab).map(|p| &p.view))
    }
    fn page_with_handle(&self, id: &str, hwnd: isize) -> Option<&Page> {
        self.peek.as_ref().filter(|peek| peek.tab.id == id).map(|peek| &peek.page)
            .or_else(|| self.pages.get(id))
            .filter(|page| page.view.hwnd().0 as isize == hwnd)
    }
    fn can_present_fullscreen(&self, id: &str, hwnd: isize) -> bool {
        self.page_with_handle(id, hwnd).is_some_and(|page| page.visible.get())
            && self.state.onboarding.completed && !self.space_form_open()
            && !self.overlay && !self.quit_pending
            && self.peek.as_ref().map_or_else(
                || self.state.visible_tab(id),
                |peek| peek.tab.id == id && peek.phase == PeekPhase::Open,
            )
    }
    fn page_fullscreen_changed(&mut self, id: String, hwnd: isize, entered: bool) -> Result<(), Box<dyn std::error::Error>> {
        let source = (id, hwnd);
        if !entered {
            if self.fullscreen_page.as_ref() == Some(&source) {
                self.restore_page_window(false)?;
                self.sync_layout()?;
            }
            return Ok(());
        }
        if self.fullscreen_page.as_ref() == Some(&source) { return Ok(()); }
        if self.fullscreen_page.is_some() || !self.can_present_fullscreen(&source.0, hwnd) {
            if let Some(page) = self.page_with_handle(&source.0, hwnd) { page.exit_fullscreen()?; }
            return Ok(());
        }
        self.finish_split_resize();
        self.dismiss_tab_preview();
        self.close_extension_menu();
        if let Some(handle) = &self.split_handle { handle.configure(None)?; }
        if let Some(handle) = &self.sidebar_handle { handle.configure(None)?; }
        let Some(window) = &self.window else { return Ok(()); };
        self.fullscreen_page = Some(source);
        if let Some(caption) = &self.caption { caption.set_fullscreen(true); }
        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        self.sync_layout()?;
        if let Some(view) = self.page() { view.focus()?; }
        Ok(())
    }
    fn restore_page_window(&mut self, exit_element: bool) -> Result<(), Box<dyn std::error::Error>> {
        let Some((id, hwnd)) = self.fullscreen_page.take() else { return Ok(()); };
        if exit_element {
            if let Some(page) = self.page_with_handle(&id, hwnd) { let _ = page.exit_fullscreen(); }
        }
        if let Some(caption) = &self.caption { caption.set_fullscreen(false); }
        if let Some(window) = &self.window {
            window.set_fullscreen(None);
            if let Some(caption) = &self.caption {
                caption.restore_maximized_bounds()?;
                caption.refresh(window.scale_factor())?;
            }
        }
        Ok(())
    }
    fn reconcile_page_fullscreen(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.fullscreen_page.as_ref().is_some_and(|(id, hwnd)| !self.can_present_fullscreen(id, *hwnd)) {
            self.restore_page_window(true)?;
        }
        Ok(())
    }
    fn page_output_target(&self, print: bool) -> Option<&Page> {
        if self.quit_pending || self.space_form_open() {
            return None;
        }
        // Like Zigzag, print the visible Peek or Reader, and save the active
        // tab's original document even while its Reader view is open.
        if print {
            if let Some(peek) = self.peek.as_ref().filter(|peek| peek.phase != PeekPhase::Closing) {
                let page = &peek.page;
                return (page.visible.get() && model::is_web(&page.url) && !page.error_page.is_failed())
                    .then_some(page);
            }
        }
        let tab = self.state.active()?;
        let page = self.pages.get(&tab.id)?;
        let reader = self.reader.as_ref().filter(|reader| reader.id == tab.id);
        let page = match reader {
            Some(reader) if !print || !reader.ready => &reader.original,
            _ => page,
        };
        (model::is_web(&page.url) && !page.error_page.is_failed()).then_some(page)
    }
    fn publish_reader(&self) {
        let id = self.reader.as_ref().filter(|r| r.ready).map(|r| r.id.as_str());
        let rect = id.and_then(|id| if id == self.state.active_tab { Some(self.primary) }
            else if self.state.active().and_then(|t| t.split_with.as_deref()) == Some(id) { self.secondary } else { None });
        self.emit(json!({"type":"readerState", "reader": {
            "id":id, "closing":self.reader.as_ref().is_some_and(|r| r.closing), "waiting":self.reader_request.as_ref().map(|r| &r.0),
            "available":self.reader_available.iter().filter(|(_, available)| **available).map(|(id, _)| id).collect::<Vec<_>>(),
            "rect":rect.map(|r| json!({"x":r.x,"y":r.y,"width":r.width,"height":r.height}))
        }}));
    }
    fn probe_reader(&self, id: &str, url: &str) {
        if self.reader.as_ref().is_some_and(|r| r.id == id) { return; }
        let Some(page) = self.pages.get(id).filter(|p| p.url == url) else { return; };
        let proxy = self.proxy.clone();
        let (id, url) = (id.to_string(), url.to_string());
        crate::reader::probe(&page.view.webview(), &url.clone(), move |available| {
            let _ = proxy.send_event(Event::ReaderAvailable(id.clone(), url.clone(), available));
        });
    }
    fn start_reader_extract(&self, id: &str, url: &str, epoch: u64) {
        let Some(page) = self.pages.get(id).filter(|p| p.url == url) else { return; };
        let proxy = self.proxy.clone();
        let (id, url) = (id.to_string(), url.to_string());
        crate::reader::extract(&page.view.webview(), &url.clone(), move |article| {
            let _ = proxy.send_event(Event::ReaderArticle(id.clone(), url.clone(), epoch, article));
        });
    }
    fn toggle_reader(&mut self, id: Option<&str>) {
        if id.is_some_and(|id| !self.state.visible_tab(id)) { return; }
        let current = self.reader.as_ref().map(|reader| reader.id.as_str())
            .or_else(|| self.reader_request.as_ref().map(|request| request.0.as_str()));
        if id.is_some() && current.is_some() && current != id {
            self.close_reader();
        } else if self.reader.as_ref().is_some_and(|r| r.closing) {
            self.animate_reader(true);
            self.focus_page_or_ui();
            return;
        } else if current.is_some() {
            self.dismiss_reader();
            return;
        }
        if self.peek.is_some() { return; }
        let id = id.unwrap_or(&self.state.active_tab);
        let Some(tab) = self.state.tabs.iter().find(|t| t.id == id && model::is_web(&t.url)).cloned() else { return; };
        self.reader_epoch += 1;
        self.reader_request = Some((tab.id.clone(), tab.url.clone(), self.reader_epoch));
        let proxy = self.proxy.clone();
        let epoch = self.reader_epoch;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(10));
            let _ = proxy.send_event(Event::ReaderDeadline(epoch));
        });
        self.publish_reader();
        if tab.loading { self.toast("网页加载完成后打开阅读模式", None); }
        else { self.start_reader_extract(&tab.id, &tab.url, self.reader_epoch); }
    }
    fn dismiss_reader(&mut self) {
        if self.reader.as_ref().is_some_and(|r| r.closing) { return; }
        if self.reader.as_ref().is_some_and(|r| r.ready) {
            self.close_find(true);
            self.animate_reader(false);
        } else {
            self.close_reader();
            if let Err(e) = self.sync() { self.error(e); }
            self.focus_page_or_ui();
        }
    }
    fn animate_reader(&mut self, opening: bool) {
        let Some(reader) = self.reader.as_mut() else { return; };
        self.reader_epoch += 1;
        let ticket = self.reader_epoch;
        reader.ticket = ticket;
        reader.ready = true;
        reader.closing = !opening;
        reader.transitioning = true;
        let script = crate::reader::motion_script(opening, self.resize_appearance.reduce_motion);
        let proxy = self.proxy.clone();
        let result = self.pages.get(&reader.id).map(|page| page.view.evaluate_script_with_callback(&script, move |value| {
            let Ok(duration) = serde_json::from_str::<u64>(&value) else {
                let _ = proxy.send_event(Event::ReaderMotionFailed(ticket));
                return;
            };
            let delay = duration.min(300) + 32;
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(delay));
                let _ = proxy.send_event(Event::ReaderMotionDone(ticket, opening));
            });
        }));
        // A renderer failure must never strand a transparent/inert controller.
        let proxy = self.proxy.clone();
        let fallback = if matches!(result, Some(Ok(()))) { 1000 } else { 0 };
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(fallback));
            let _ = proxy.send_event(Event::ReaderMotionFailed(ticket));
        });
        if opening {
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(450));
                let _ = proxy.send_event(Event::ReaderResources(ticket, 0));
            });
        }
        if let Err(e) = self.sync_layout() { self.error(e); }
    }
    /// Swap only the displayed controller. The original DOM, form and scroll
    /// remain alive; all existing layout, split, find and capture paths use Reader.
    fn close_reader(&mut self) {
        self.reader_request = None;
        self.reader_epoch += 1;
        if let Some(mut reader) = self.reader.take() {
            self.close_find(true);
            self.finish_split_resize();
            if let Some(handle) = &self.split_handle { let _ = handle.configure(None); }
            if let Some(handle) = &self.sidebar_handle { let _ = handle.configure(None); }
            if let Some(page) = self.pages.remove(&reader.id) {
                let _ = page.set_visible(false);
                reader.original.thumbnail_version = page.thumbnail_version + 1;
            }
            self.pages.insert(reader.id.clone(), reader.original);
            self.invalidate_thumbnail(&reader.id);
        }
        self.publish_reader();
        if let Err(e) = self.sync_layout() { self.error(e); }
    }
    fn apply_reader_preferences(&mut self, preferences: crate::reader::Preferences) {
        if !preferences.valid() { self.error("阅读偏好无效"); return; }
        if let Some(reader) = &self.reader {
            if let Some(page) = self.pages.get(&reader.id) { let _ = page.view.evaluate_script(&preferences.script()); }
        }
        self.state.reader_preferences = preferences;
        self.save();
        self.publish();
    }
    fn show_reader(&mut self, id: String, url: String, article: crate::reader::Article) -> Result<(), Box<dyn std::error::Error>> {
        if article.url != url || !model::is_web(&url) { return Err("阅读内容的地址已变化".into()); }
        self.close_find(true);
        self.dismiss_tab_preview();
        self.finish_split_resize();
        if let Some(handle) = &self.split_handle { handle.configure(None)?; }
        if let Some(handle) = &self.sidebar_handle { handle.configure(None)?; }
        let proxy = self.proxy.clone();
        let reader_id = id.clone();
        let initial_document = Cell::new(true);
        let ticket = self.reader_epoch;
        let view = browsing_webview(&mut self.context)
            .with_visible(false).with_focused(false).with_javascript_disabled()
            .with_transparent(true)
            .with_theme(if self.state.theme == "dark" { wry::Theme::Dark } else { wry::Theme::Light })
            .with_navigation_handler(move |target| {
                // wry turns with_html into one data: document on Windows. Only
                // that initial load is allowed; article links never load data.
                if initial_document.get() && (target == "about:blank" || target.starts_with("data:text/html")) {
                    initial_document.set(false);
                    return true;
                }
                if model::is_web(&target) { let _ = proxy.send_event(Event::ReaderLink(reader_id.clone(), target)); }
                false
            })
            .with_new_window_req_handler({
                let proxy = self.proxy.clone();
                move |target, _| {
                    if model::is_web(&target) { let _ = proxy.send_event(Event::Open(target)); }
                    wry::NewWindowResponse::Deny
                }
            })
            .with_on_page_load_handler({
                let proxy = self.proxy.clone();
                move |event, _| {
                    if matches!(event, PageLoadEvent::Finished) { let _ = proxy.send_event(Event::ReaderReady(ticket)); }
                }
            })
            .with_html(crate::reader::html(&article, &self.state.reader_preferences))
            .build_as_child(self.window.as_ref().ok_or("窗口尚未就绪")?)?;
        // Reveal at DOM readiness, without waiting for slow article images.
        let proxy = self.proxy.clone();
        let handler = DOMContentLoadedEventHandler::create(Box::new(move |_, _| {
            let _ = proxy.send_event(Event::ReaderReady(ticket));
            Ok(())
        }));
        unsafe { view.webview().cast::<ICoreWebView2_2>()?.add_DOMContentLoaded(&handler, &mut 0)?; }
        self.track_webview_focus(&view)?;
        register_accelerators(&view, self.proxy.clone(), self.find_open.clone(), self.peek_visible.clone(), self.downloads_open.clone(), self.live_settings.clone(), None)?;
        let error_page = crate::error_page::install(&view, self.state.theme == "dark")?;
        let page = Page {
            permission_document: Default::default(),
            resize_surface: Rc::new(crate::resize_surface::Surface::new(view.hwnd(), Some(view.webview()), self.resize_appearance)),
            view, url: url.clone(), clip: Rc::new(WindowClip::default()), save_path: Default::default(),
            thumbnail_version: 0, thumbnail_attempt: None, thumbnail_pending: false,
            visible: Cell::new(false), bounds: Rc::new(Cell::new(None)), viewport: Rc::new(Cell::new(None)),
            dark: Cell::new(self.state.theme == "dark"), transparent: true, zoom_default: Cell::new(1.0),
            navigation: Rc::new(NavigationPolicy { bookmarked: Cell::new(false), current: RefCell::new(url.clone()), host_navigation: Cell::new(false) }),
            error_page, sleeping: Rc::new(Cell::new(false)), sleep_pending: Rc::new(Cell::new(false)), sleep_failed_at: Cell::new(None),
        };
        if let Some(original) = self.pages.insert(id.clone(), page) {
            if let Some(page) = self.pages.get_mut(&id) { page.thumbnail_version = original.thumbnail_version + 1; }
            self.reader = Some(ReaderSession { id: id.clone(), url, original, ticket, ready: false, closing: false, transitioning: true });
        }
        self.invalidate_thumbnail(&id);
        self.sync()?;
        self.publish_reader();
        self.focus_page_or_ui();
        Ok(())
    }
    /// Closes the find session and (unless another page takes over silently)
    /// tells the trusted UI, so both the shell and the floating layer hide the
    /// bar. Zigzag's `FindBar.hide` also clears the page's highlights, which
    /// `ICoreWebView2Find::Stop` does for us.
    fn close_find(&mut self, notify: bool) {
        if let Some(session) = self.find.take() {
            session.close();
        }
        let was_open = self.find_open.replace(false);
        if notify && was_open {
            self.emit(json!({"type":"findClosed"}));
        }
    }
    /// ⌘F: opens the bar, or puts the caret back in it when it is already up.
    /// Internal pages (`era://settings`, the new-tab surface) have no WebView
    /// to search, so they are left alone, as Zigzag does for its own pages.
    fn find_target_is_web(&self) -> bool {
        match &self.peek {
            Some(peek) => model::is_web(&peek.tab.url),
            None => self
                .state
                .active()
                .is_some_and(|tab| model::is_web(&tab.url)),
        }
    }
    fn open_find(&mut self) {
        if !self.find_target_is_web() {
            return;
        }
        if let Some(popup) = &self.popup {
            let _ = popup.focus();
        }
        if self.find_open.get() {
            self.emit(json!({"type":"findFocus"}));
        } else {
            self.find_open.set(true);
            self.emit(json!({"type":"findOpen"}));
        }
    }
    /// Runs the search in the page the bar is over. Every keystroke arrives as
    /// a fresh session: WebView2 has no "set the term" call, and `FindNext`
    /// without a prior `Start` would search an empty term.
    fn start_find(&mut self, query: &str) {
        let query: String = query.trim().chars().take(crate::find::MAX_QUERY_CHARS).collect();
        if query.is_empty() {
            if let Some(session) = self.find.take() {
                session.close();
            }
            self.emit(json!({"type":"findMatches","count":0,"active":0}));
            return;
        }
        if !self.find_target_is_web() {
            self.close_find(true);
            return;
        }
        // The document this search belongs to. `Page::url` follows in-page
        // navigations (Source events) sooner than the shared tab state does.
        let Some((target_id, target_url)) = self
            .peek
            .as_ref()
            .map(|peek| (peek.tab.id.clone(), peek.page.url.clone()))
            .or_else(|| {
                let tab = self.state.active()?;
                let page_url = self
                    .pages
                    .get(&tab.id)
                    .map(|page| page.url.clone())
                    .unwrap_or_else(|| tab.url.clone());
                Some((tab.id.clone(), page_url))
            })
        else {
            self.close_find(true);
            return;
        };
        let proxy = self.proxy.clone();
        let report: crate::find::Report = Rc::new(move |count, active| {
            let _ = proxy.send_event(Event::FindMatches(count, active));
        });
        let started = match self.page() {
            Some(page) => crate::find::start(page, &query, target_id, target_url, report),
            None => Err("当前页面不支持页内查找".into()),
        };
        match started {
            Ok(session) => {
                if let Some(previous) = self.find.replace(session) {
                    previous.close();
                }
                self.find_open.set(true);
            }
            Err(message) => {
                self.error(message);
                self.close_find(true);
            }
        }
    }
    /// The zoom this site was last left at, from `ZoomLevels`-style storage.
    fn site_zoom_for(&self, url: &str) -> f64 {
        model::site_key(url)
            .and_then(|site| self.state.site_zoom.get(&site).copied())
            .filter(|factor| model::valid_zoom(*factor))
            .unwrap_or(1.0)
    }
    /// Tells the UI the shown page's zoom, so its site menu and toast start in
    /// the right place after a tab, space or split switch.
    fn emit_active_zoom(&self) {
        let factor = self.page().and_then(|view| crate::site::zoom_factor(view).ok()).unwrap_or(1.0);
        self.emit(json!({"type":"zoom","factor":factor}));
    }
    fn page_zoom_changed(&mut self, id: &str, hwnd: isize, factor: f64) {
        if !model::valid_zoom(factor) {
            return;
        }
        let (tab, page, in_peek) = if let Some(peek) = self.peek.as_mut().filter(|peek| peek.tab.id == id) {
            (&mut peek.tab, &peek.page, true)
        } else {
            let Some(page) = self.pages.get(id) else { return };
            let Some(tab) = self.state.tabs.iter_mut().find(|tab| tab.id == id) else { return };
            (tab, page, false)
        };
        let previous = page.zoom_default.get();
        // Navigation can also emit ZoomFactorChanged. Load restores the saved
        // factor first. Discard notifications superseded by that restore or an input.
        if page.view.hwnd().0 as isize != hwnd
            || !page.visible.get()
            || (previous - factor).abs() <= 0.001
            || !crate::site::zoom_factor(&page.view)
                .is_ok_and(|current| (current - factor).abs() <= 0.001)
        {
            return;
        }
        let factor = factor.clamp(model::ZOOM_STEPS[0], *model::ZOOM_STEPS.last().unwrap());
        // Setting the factor makes it the controller's default across navigations.
        if page.set_zoom(factor).is_err() || (previous - factor).abs() <= 0.001 {
            return;
        }
        tab.zoom = Some(factor);
        if !in_peek {
            self.save();
            self.publish();
        }
        if in_peek || (self.peek.is_none() && self.state.active_tab == id) {
            self.emit(json!({"type":"zoom","factor":factor}));
        }
        self.emit(json!({"type":"zoomNotice","id":id,"factor":factor}));
    }
    /// A short-lived message for the floating layer. `action` names the button
    /// ("undoBatch" for the batch Undo), or is absent for a plain notice.
    fn toast(&self, message: &str, action: Option<&str>) {
        self.emit(json!({"type":"toast","message":message,"action":action}));
    }
    fn focus_page_or_ui(&self) {
        if let Some(page) = self.page() {
            let _ = page.focus();
        } else if let Some(ui) = &self.ui {
            let _ = ui.focus();
        }
    }
    fn track_webview_focus(&self, view: &WebView) -> windows::core::Result<()> {
        let last_focused = self.last_focused_view.clone();
        let hwnd = view.hwnd();
        let got_focus = FocusChangedEventHandler::create(Box::new(move |_, _| {
            last_focused.set(hwnd);
            Ok(())
        }));
        unsafe { view.controller().add_GotFocus(&got_focus, &mut 0) }
    }
    fn restore_keyboard_focus(&self) {
        let Some(handle) = self.window.as_ref().and_then(|window| window.window_handle().ok()) else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else { return };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        let mut state = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        // Query the foreground input queue so focus in WebView2's renderer
        // window is also visible here, even when it belongs to another thread.
        unsafe {
            if GetGUIThreadInfo(0, &mut state).is_err() || state.hwndActive != hwnd
                || (IsChild(hwnd, state.hwndFocus).as_bool() && IsWindowVisible(state.hwndFocus).as_bool())
            {
                return;
            }
        }
        let visible = |view: &&WebView| unsafe { IsWindowVisible(view.hwnd()).as_bool() };
        let page = self.page().filter(visible);
        let last = self.last_focused_view.get();
        let previous = self.ui.iter()
            .chain(self.popup.iter().filter(|_| self.popup_has_regions))
            .chain(self.peek.iter().map(|peek| &peek.page.view))
            .chain(self.pages.values().map(|page| &page.view))
            .find(|view| view.hwnd() == last && visible(view));
        let target = if self.fullscreen_page.is_some() { page } else { previous.or(page) };
        if let Some(view) = target.or(self.ui.as_ref()) {
            let _ = view.focus();
        }
    }
    /// Closes a set of tabs as one undoable batch, keeping enough geometry to put
    /// them back exactly where they were. Used by Clear, Close Others and Close Below.
    fn close_batch(&mut self, ids: Vec<String>, faded: bool) {
        if ids.is_empty() {
            return;
        }
        let before: std::collections::HashSet<String> =
            self.state.tabs.iter().map(|tab| tab.id.clone()).collect();
        let previous_active = self.state.active_tab.clone();
        let mut entries = Vec::new();
        for id in ids {
            if let Some(index) = self.state.tabs.iter().position(|tab| tab.id == id) {
                let tab = self.state.tabs[index].clone();
                if tab.bookmark_id.is_some() {
                    continue;
                }
                entries.push((tab.space_id.clone(), index, tab));
            }
        }
        if entries.is_empty() {
            return;
        }
        let count = entries.len();
        for (_, _, tab) in &entries {
            self.state.close(&tab.id);
            self.playing.remove(&tab.id);
        }
        let auto_created = self
            .state
            .tabs
            .iter()
            .find(|tab| !before.contains(&tab.id))
            .map(|tab| tab.id.clone());
        self.undo_batch = Some(UndoBatch {
            entries,
            previous_active,
            auto_created,
        });
        self.publish();
        self.save();
        if let Err(error) = self.sync() {
            self.error(error);
        }
        self.focus_page_or_ui();
        self.toast(
            &format!("已关闭 {count} 个{}标签", if faded { "旧" } else { "" }),
            Some("undoBatch"),
        );
    }
    /// Puts the last batch back at its original indexes and restores selection.
    fn undo_batch(&mut self) {
        let Some(batch) = self.undo_batch.take() else {
            return;
        };
        let count = batch.entries.len();
        if let Some(blank) = batch.auto_created {
            self.state.tabs.retain(|tab| tab.id != blank);
        }
        let mut entries = batch.entries;
        entries.sort_by_key(|(_, index, _)| *index);
        let mut restored = Vec::new();
        for (space_id, index, mut tab) in entries {
            if self.state.tabs.iter().any(|existing| existing.id == tab.id) {
                continue;
            }
            tab.space_id = if self.state.spaces.iter().any(|space| space.id == space_id) {
                space_id
            } else {
                self.state.active_space.clone()
            };
            tab.last_activated = model::now_secs();
            let position = index.min(self.state.tabs.len());
            restored.push(tab.id.clone());
            self.state.tabs.insert(position, tab);
        }
        let target = self
            .state
            .tabs
            .iter()
            .any(|tab| tab.id == batch.previous_active)
            .then_some(batch.previous_active)
            .or_else(|| restored.first().cloned());
        if let Some(id) = target {
            self.state.activate(&id);
        }
        // Undo re-adds the tabs itself, so the closed stack must not offer them again.
        self.state.closed.retain(|tab| !restored.contains(&tab.id));
        self.publish();
        self.save();
        if let Err(error) = self.sync() {
            self.error(error);
        }
        self.focus_page_or_ui();
        self.toast(&format!("已恢复 {count} 个标签"), None);
    }
    fn publish_downloads(&self) {
        self.download_sources.borrow_mut()
            .retain(|id, _| self.download_ops.borrow().contains_key(id));
        let items = self.downloads.borrow_mut().publish_items();
        self.emit(json!({"type": "downloads", "items": items}));
    }
    fn save_downloads(&mut self) {
        let items = self.downloads.borrow().items.clone();
        if let Err(error) = self.storage.save_downloads(&items) {
            self.error(error);
        }
    }
    fn refresh_downloads(&mut self) {
        // COM calls can dispatch callbacks. Never hold a RefCell borrow while
        // querying an operation or releasing its last reference.
        let operations: Vec<_> = self.download_ops.borrow().iter()
            .map(|(id, operation)| (id.clone(), operation.clone()))
            .collect();
        let mut persist = false;
        for (id, operation) in operations {
            if let Ok(changed) = sync_download(&operation, &id, &self.downloads, &self.download_ops) {
                persist |= changed;
            }
        }
        if persist {
            self.save_downloads();
        }
        self.publish_downloads();
    }
    /// A cancelled or failed download leaves a partial file; Zigzag discards it
    /// when the row goes, so Clear never leaves hidden debris behind.
    fn discard_partial(&self, item: &DownloadItem) {
        if matches!(item.state, DownloadState::Failed | DownloadState::Cancelled) {
            let path = std::path::Path::new(&item.path);
            if path.is_file() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    fn open_path(&self, path: &std::path::Path) {
        use windows::core::w;
        let file = HSTRING::from(path.as_os_str());
        unsafe { ShellExecuteW(None, w!("open"), &file, None, None, SW_SHOWNORMAL) };
    }
    fn reveal_path(&self, path: &std::path::Path) {
        use windows::core::w;
        let explorer = HSTRING::from("explorer.exe");
        let arguments = HSTRING::from(format!("/select,\"{}\"", path.display()));
        unsafe { ShellExecuteW(None, w!("open"), &explorer, &arguments, None, SW_SHOWNORMAL) };
    }
    fn download_folder(&self) -> PathBuf {
        if !self.state.settings.download_folder.is_empty() {
            return PathBuf::from(&self.state.settings.download_folder);
        }
        directories::UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(|path| path.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    }
    fn review_download(&mut self, id: String) {
        if self.download_popup_visible {
            // Let the history panel release focus before opening the native dialog.
            self.pending_download_review = Some(id);
            return;
        }
        let source = self.download_sources.borrow().get(&id).cloned();
        let Some(source) = source else {
            self.error("这个下载已结束");
            return;
        };
        if self.pages.contains_key(&source) {
            self.state.activate(&source);
            self.publish();
            self.save();
            if let Err(error) = self.sync() {
                self.error(error);
                return;
            }
        }
        let view = self.pages.get(&source).map(|page| &page.view)
            .or_else(|| self.peek.as_ref().filter(|peek| peek.tab.id == source).map(|peek| &peek.page.view))
            .or_else(|| self.page());
        let Some(view) = view else {
            self.error("下载来源页面已关闭");
            return;
        };
        let _ = view.focus();
        if let Err(error) = unsafe {
            view.webview().cast::<ICoreWebView2_9>().and_then(|view| view.OpenDefaultDownloadDialog())
        } {
            self.error(format!("无法显示下载提示：{error}"));
        }
    }
    fn download_action(&mut self, id: Option<String>, action: &str) {
        if action == "openFolder" {
            let folder = self.download_folder();
            if !folder.is_dir() {
                self.error("下载文件夹不存在");
                return;
            }
            self.open_path(&folder);
            return;
        }
        if action == "clearFinished" {
            let removed = self.downloads.borrow_mut().clear_clearable();
            for item in &removed {
                self.discard_partial(item);
            }
            self.download_ops
                .borrow_mut()
                .retain(|id, _| self.downloads.borrow().get(id).is_some());
            self.save_downloads();
            self.publish_downloads();
            return;
        }
        let Some(id) = id else { return };
        let operation = self.download_ops.borrow().get(&id).cloned();
        match action {
            "review" => self.review_download(id),
            "pause" => {
                if let Some(operation) = &operation {
                    if let Err(error) = unsafe { operation.Pause() } {
                        self.error(format!("无法暂停下载：{error}"));
                        return;
                    }
                }
                if let Some(item) = self.downloads.borrow_mut().get_mut(&id)
                    .filter(|item| item.is_active())
                {
                    item.state = DownloadState::Paused;
                }
                self.save_downloads();
                self.publish_downloads();
            }
            "resume" => {
                let Some(operation) = operation else {
                    self.error("这个下载已经无法继续");
                    return;
                };
                let mut can_resume = BOOL::default();
                let resumable = unsafe { operation.CanResume(&mut can_resume) }.is_ok()
                    && can_resume.as_bool();
                if !resumable {
                    self.error("这个下载已经无法继续");
                    return;
                }
                if let Err(error) = unsafe { operation.Resume() } {
                    self.error(format!("无法继续下载：{error}"));
                    return;
                }
                if let Some(item) = self.downloads.borrow_mut().get_mut(&id)
                    .filter(|item| item.is_active())
                {
                    item.state = DownloadState::InProgress;
                }
                self.save_downloads();
                self.publish_downloads();
            }
            "cancel" => {
                if let Some(operation) = &operation {
                    let _ = unsafe { operation.Cancel() };
                }
                let active = self.downloads.borrow().get(&id).is_some_and(DownloadItem::is_active);
                if active {
                    self.downloads
                        .borrow_mut()
                        .finish(&id, DownloadState::Cancelled, None);
                }
                self.download_ops.borrow_mut().remove(&id);
                self.save_downloads();
                self.publish_downloads();
            }
            "remove" => {
                if let Some(item) = self.downloads.borrow_mut().remove(&id) {
                    self.discard_partial(&item);
                }
                self.download_ops.borrow_mut().remove(&id);
                self.save_downloads();
                self.publish_downloads();
            }
            "open" | "reveal" => {
                let Some(item) = self.downloads.borrow().get(&id)
                    .filter(|item| item.state == DownloadState::Finished).cloned()
                else {
                    return;
                };
                let path = PathBuf::from(&item.path);
                if !path.is_file() {
                    self.error("文件已被移动或删除");
                    return;
                }
                if action == "open" {
                    self.open_path(&path);
                } else {
                    self.reveal_path(&path);
                }
            }
            _ => {}
        }
    }
    fn build_page(&mut self, tab: &Tab) -> Result<Page, Box<dyn std::error::Error>> {
        let navigation = Rc::new(NavigationPolicy {
            bookmarked: Cell::new(tab.bookmark_id.is_some()),
            current: RefCell::new(tab.url.clone()),
            host_navigation: Cell::new(false),
        });
        let p = self.proxy.clone();
        let id = tab.id.clone();
        let p2 = p.clone();
        let id2 = id.clone();
        let view = browsing_webview(&mut self.context)
            .with_visible(false)
            .with_focused(false)
            .with_background_color(surface_color(self.state.theme == "dark"))
            .with_theme(if self.state.theme == "dark" {
                wry::Theme::Dark
            } else {
                wry::Theme::Light
            })
            .with_navigation_handler(|url| model::is_web(&url) || url == "about:blank")
            .with_initialization_script(include_str!("peek-links.js"))
            .with_initialization_script(include_str!("chrome-store-install.js"))
            // Zigzag's SwipeNavigation: WebView2 performs the macOS-style
            // two-finger back/forward swipe itself, including its own animation.
            .with_back_forward_navigation_gestures(true)
            .with_document_title_changed_handler(move |title| {
                let _ = p.send_event(Event::Title(id.clone(), title));
            })
            .with_on_page_load_handler(move |event, url| {
                let _ = p2.send_event(Event::Load(
                    id2.clone(),
                    url,
                    matches!(event, PageLoadEvent::Started),
                ));
            })
            // The page adapters use standard window requests. Websites have no application IPC.
            .build_as_child(self.window.as_ref().unwrap())?;
        self.track_webview_focus(&view)?;
        // Wry's zoom option also enables pinch, which is handled separately by WebView2.
        unsafe { view.webview().Settings()?.SetIsZoomControlEnabled(true)?; }
        crate::web_settings::apply(&view, &self.state.settings)?;
        let shared = PageShared {
            peek_visible: self.peek_visible.clone(),
            find_open: self.find_open.clone(),
            downloads_open: self.downloads_open.clone(),
            downloads: self.downloads.clone(),
            download_ops: self.download_ops.clone(),
            download_sources: self.download_sources.clone(),
            save_path: Default::default(),
        };
        let save_path = shared.save_path.clone();
        register_page_events(
            &view,
            tab.id.clone(),
            self.proxy.clone(),
            shared,
            navigation.clone(),
            self.live_settings.clone(),
        )?;
        let error_page = crate::error_page::install(&view, self.state.theme == "dark")?;
        let _ = crate::site::set_muted(&view, tab.muted);
        let permission_document = Rc::new(permissions::Document::default());
        let proxy = self.proxy.clone();
        permissions::register(&view, tab.id.clone(), &permission_document, self.permissions.inbox.clone(), move || {
            let _ = proxy.send_event(Event::PermissionsChanged);
        })?;
        let page = Page {
            permission_document,
            resize_surface: Rc::new(crate::resize_surface::Surface::new(view.hwnd(), Some(view.webview()), self.resize_appearance)),
            view,
            save_path,
            url: tab.url.clone(),
            clip: Rc::new(WindowClip::default()),
            thumbnail_version: 0,
            thumbnail_attempt: None,
            thumbnail_pending: false,
            visible: Cell::new(false),
            bounds: Rc::new(Cell::new(None)),
            viewport: Rc::new(Cell::new(None)),
            dark: Cell::new(self.state.theme == "dark"),
            transparent: false,
            zoom_default: Cell::new(1.0),
            navigation,
            error_page,
            sleeping: Rc::new(Cell::new(false)),
            sleep_pending: Rc::new(Cell::new(false)),
            sleep_failed_at: Cell::new(None),
        };
        page.restore_zoom(tab.zoom.unwrap_or_else(|| self.site_zoom_for(&tab.url)));
        page.view.load_url(&tab.url)?;
        Ok(page)
    }
    fn invalidate_thumbnail(&mut self, id: &str) {
        if let Some(page) = self.pages.get_mut(id) {
            page.thumbnail_version += 1;
            page.thumbnail_attempt = None;
            page.thumbnail_pending = false;
        }
        self.thumbnails.remove(id);
        self.emit(json!({"type":"thumbnail","id":id,"image":null,"kept":self.thumbnails.images().keys().collect::<Vec<_>>()}));
    }
    fn refresh_visible_thumbnails(&mut self) {
        let ids = [
            Some(self.state.active_tab.clone()),
            self.state.active().and_then(|t| t.split_with.clone()),
        ];
        for id in ids.into_iter().flatten() {
            if let Some(page) = self.pages.get_mut(&id) {
                if !page.thumbnail_pending {
                    page.thumbnail_version += 1;
                    page.thumbnail_attempt = None;
                }
            }
        }
        self.pump_thumbnails();
    }

    /// Saved groups may be partially unloaded. Looking at one must never create a page.
    fn preview_tabs(&self, id: &str) -> Option<Vec<String>> {
        if self.overlay || self.peek.is_some() || self.state.sidebar_collapsed {
            return None;
        }
        let mut tabs = if let Some(mark) = self.state.bookmark(id) {
            if mark
                .space_id
                .as_ref()
                .is_some_and(|space| space != &self.state.active_space)
            {
                return None;
            }
            self.state
                .tabs
                .iter()
                .filter(|t| {
                    t.bookmark_id
                        .as_ref()
                        .is_some_and(|key| key == id || Some(key) == mark.split_with.as_ref())
                })
                .map(|t| t.id.clone())
                .collect::<Vec<_>>()
        } else {
            let tab = self
                .state
                .tabs
                .iter()
                .find(|t| t.id == id && t.space_id == self.state.active_space)?;
            self.state
                .tabs
                .iter()
                .filter(|t| {
                    t.id == tab.id
                        || Some(&t.id) == tab.split_with.as_ref()
                        || t.split_with.as_ref() == Some(&tab.id)
                })
                .map(|t| t.id.clone())
                .collect()
        };
        if tabs.len() == 1 {
            let first = self.state.tabs.iter().find(|t| t.id == tabs[0]).unwrap();
            if let Some(partner) = self.state.tabs.iter().find(|t| {
                Some(&t.id) == first.split_with.as_ref() || t.split_with.as_ref() == Some(&first.id)
            }) {
                tabs.push(partner.id.clone());
            }
        }
        if tabs.iter().any(|id| {
            id == &self.state.active_tab
                || self.state.active().and_then(|t| t.split_with.as_ref()) == Some(id)
        }) {
            return None;
        }
        Some(tabs)
    }

    fn ensure_preview_renders(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let targets = crate::preview::targets(&self.state, id).unwrap_or_default();
        self.preview_renders.retain(|key, p| targets.iter().any(|t| &t.id == key && t.url == p.source));
        for tab in targets {
            if !model::is_web(&tab.url) || self.pages.contains_key(&tab.id) || self.preview_renders.contains_key(&tab.id) || self.thumbnails.has(&tab.id, &tab.url) { continue; }
            self.preview_epoch += 1;
            let epoch = self.preview_epoch;
            let key = tab.id.clone();
            let proxy = self.proxy.clone();
            let view = browsing_webview(&mut self.context)
                .with_visible(false).with_focused(false)
                .with_theme(if self.state.theme == "dark" { wry::Theme::Dark } else { wry::Theme::Light })
                .with_background_color(surface_color(self.state.theme == "dark"))
                .with_navigation_handler(|url| model::is_web(&url) || url == "about:blank")
                .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
                .with_download_started_handler(|_, _| false)
                .with_on_page_load_handler(move |event, url| {
                    let _ = proxy.send_event(Event::PreviewLoad(key.clone(), epoch, url, matches!(event, PageLoadEvent::Started)));
                }).build_as_child(self.window.as_ref().unwrap())?;
            crate::web_settings::apply(&view, &self.state.settings)?;
            permissions::deny_preview_requests(&view)?;
            let error_page = crate::error_page::install(&view, self.state.theme == "dark")?;
            view.set_bounds(self.bounds(self.primary))?;
            view.load_url(&tab.url)?;
            let timed_key = tab.id.clone();
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(12));
                let _ = proxy.send_event(Event::PreviewLoad(timed_key, epoch, String::new(), false));
            });
            self.preview_renders.insert(tab.id, PreviewRender { view, error_page, source: tab.url, epoch, loading: true, pending: false, version: 0 });
        }
        self.sync_layout()?;
        Ok(())
    }
    fn refresh_preview_renders(&mut self) {
        for (key, page) in &mut self.preview_renders {
            if page.loading || page.pending { continue; }
            page.pending = true;
            page.version += 1;
            let version = page.version;
            let epoch = page.epoch;
            let id = key.clone();
            let proxy = self.proxy.clone();
            if crate::capture::thumbnail(&page.view, move |image| {
                let _ = proxy.send_event(Event::PreviewThumbnail(id, epoch, version, image));
            }).is_err() {
                let _ = self.proxy.send_event(Event::PreviewThumbnail(key.clone(), epoch, version, None));
            }
            let id = key.clone();
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = proxy.send_event(Event::PreviewThumbnail(id, epoch, version, None));
            });
        }
    }
    fn dismiss_tab_preview(&mut self) {
        let existed = self.tab_preview.take().is_some();
        self.preview_renders.clear();
        if existed {
            let _ = self.sync_layout();
            self.emit(json!({"type":"tabPreviewDismissed"}));
        }
    }
    /// Zigzag `TabSleep.sleepers`, with what the host knows: a page on screen,
    /// in the split, playing sound, loading or tried lately never sleeps.
    fn sleep_candidates(&self) -> Vec<crate::sleep::Candidate> {
        let favorites: std::collections::HashSet<&str> =
            self.state.favorites.iter().map(|f| f.id.as_str()).collect();
        let second = self.state.active().and_then(|t| t.split_with.as_deref());
        let now = model::now_secs();
        let mut candidates = Vec::new();
        for (id, page) in &self.pages {
            let Some(tab) = self.state.tabs.iter().find(|tab| &tab.id == id) else { continue };
            if !model::is_web(&tab.url) {
                continue;
            }
            if page.sleeping.get() || page.sleep_pending.get() {
                continue;
            }
            if page
                .sleep_failed_at
                .get()
                .is_some_and(|at| at.elapsed() < crate::sleep::RETRY_AFTER)
            {
                continue;
            }
            let favorite = tab
                .bookmark_id
                .as_ref()
                .is_some_and(|mark| favorites.contains(mark.as_str()));
            candidates.push(crate::sleep::Candidate {
                id: id.clone(),
                unseen: Duration::from_secs(now.saturating_sub(tab.last_activated)),
                can_sleep: !page.visible.get()
                    && id != &self.state.active_tab
                    && second != Some(id.as_str())
                    && !self.playing.contains(id)
                    && !tab.loading,
                favorite,
            });
        }
        candidates
    }

    /// One timer tick: ask Zigzag's rule which background pages should give
    /// their memory back, then start suspending them two at a time.
    fn sleep_pass(&mut self) {
        if !self.ready || !self.state.onboarding.completed || !self.state.settings.sleep_tabs {
            return;
        }
        if self.pages.is_empty() {
            return;
        }
        let awake = self
            .pages
            .values()
            .filter(|page| !page.sleeping.get())
            .count();
        let memory = crate::sleep::pressure();
        let ids = crate::sleep::sleepers(
            &self.sleep_candidates(),
            awake,
            memory,
            &self.sleep_thresholds,
        );
        if ids.is_empty() {
            return;
        }
        self.sleep_queue.extend(ids);
        self.pump_sleep();
    }

    /// Zigzag's `StagedQueue`: at most two suspensions in flight, each one
    /// followed by the next through `Event::SleepDone`, so a long list of tabs
    /// never takes down their pages in one turn of the event loop.
    fn pump_sleep(&mut self) {
        if self.sleep_queue.is_empty() {
            return;
        }
        if !self.state.settings.sleep_tabs || !self.ready || !self.state.onboarding.completed {
            self.sleep_queue.clear();
            return;
        }
        let mut in_flight = self
            .pages
            .values()
            .filter(|page| page.sleep_pending.get())
            .count();
        while in_flight < 2 {
            let Some(id) = self.sleep_queue.pop_front() else { return };
            let Some(page) = self.pages.get(&id) else { continue };
            if page.sleeping.get() || page.sleep_pending.get() {
                continue;
            }
            let second = self.state.active().and_then(|t| t.split_with.as_deref());
            if page.visible.get()
                || id == self.state.active_tab
                || second == Some(id.as_str())
                || self.playing.contains(&id)
            {
                continue;
            }
            let Ok(web) = page.view.webview().cast::<ICoreWebView2_3>() else {
                page.sleep_failed_at.set(Some(Instant::now()));
                continue;
            };
            let proxy = self.proxy.clone();
            let sleeping = page.sleeping.clone();
            let pending = page.sleep_pending.clone();
            let callback_id = id.clone();
            let handler = TrySuspendCompletedHandler::create(Box::new(move |result, suspended| {
                pending.set(false);
                let slept = result.is_ok() && suspended;
                if slept {
                    sleeping.set(true);
                }
                let _ = proxy.send_event(Event::SleepDone(callback_id.clone(), slept));
                Ok(())
            }));
            // SAFETY: same live WebView on the UI thread; WebView2 owns the
            // handler until it invokes the completion.
            match unsafe { web.TrySuspend(&handler) } {
                Ok(()) => {
                    page.sleep_pending.set(true);
                    in_flight += 1;
                }
                Err(_) => page.sleep_failed_at.set(Some(Instant::now())),
            }
        }
    }

    /// A suspension finished. If the page came back on screen meanwhile (or the
    /// setting was turned off), wake it; otherwise tell the UI its row is asleep.
    fn sleep_done(&mut self, id: &str, slept: bool) {
        let present = self.pages.contains_key(id);
        let mut stale = false;
        if let Some(page) = self.pages.get(id) {
            page.sleep_pending.set(false);
            if slept {
                let second = self.state.active().and_then(|t| t.split_with.as_deref());
                stale = !self.state.settings.sleep_tabs
                    || page.visible.get()
                    || id == self.state.active_tab
                    || second == Some(id)
                    || self.playing.contains(id);
                if stale {
                    page.wake();
                }
            } else {
                page.sleep_failed_at.set(Some(Instant::now()));
            }
        }
        #[cfg(debug_assertions)]
        if std::env::var_os("ERA_SLEEP_LOG").is_some() {
            eprintln!(
                "Rove sleep: {id} {}",
                if slept { "asleep" } else { "refused" }
            );
        }
        // A tab closed while its suspension was in flight has no row to mark.
        if slept && present {
            self.emit(json!({"type":"sleep","id":id,"asleep":!stale}));
        }
        self.pump_sleep();
    }

    /// Settings › 标签页 turned sleeping off: give every sleeping page back.
    fn wake_all_sleeping(&mut self) {
        self.sleep_queue.clear();
        let ids: Vec<String> = self
            .pages
            .iter()
            .filter(|(_, page)| page.sleeping.get())
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some(page) = self.pages.get(&id) {
                page.wake();
            }
            self.emit(json!({"type":"sleep","id":id,"asleep":false}));
        }
    }

    /// The runtime state the session doesn't hold: which loaded pages are asleep.
    fn publish_sleep_state(&self) {
        let ids: Vec<&str> = self
            .pages
            .iter()
            .filter(|(_, page)| page.sleeping.get())
            .map(|(id, _)| id.as_str())
            .collect();
        self.emit(json!({"type":"sleepState","ids":ids}));
    }

    fn pump_thumbnails(&mut self) {
        // Peek's full-size capture/animation has priority over background cards.
        if self.peek.is_some() {
            return;
        }
        let pending = self.pages.values().filter(|p| p.thumbnail_pending).count();
        let candidates: Vec<_> = self
            .pages
            .iter()
            .filter(|(id, p)| {
                !p.thumbnail_pending
                    && !p.sleeping.get()
                    && p.thumbnail_attempt != Some(p.thumbnail_version)
                    && self.state.tabs.iter().any(|t| &t.id == *id && !t.loading)
            })
            .take(2usize.saturating_sub(pending))
            .map(|(id, _)| id.clone())
            .collect();
        for id in candidates {
            let page = self.pages.get_mut(&id).unwrap();
            page.thumbnail_pending = true;
            page.thumbnail_attempt = Some(page.thumbnail_version);
            let url = page.url.clone();
            let version = page.thumbnail_version;
            let proxy = self.proxy.clone();
            let callback_id = id.clone();
            let callback_url = url.clone();
            let result = crate::capture::thumbnail(&page.view, move |image| {
                let _ =
                    proxy.send_event(Event::Thumbnail(callback_id, callback_url, version, image));
            });
            if result.is_err() {
                let _ =
                    self.proxy
                        .send_event(Event::Thumbnail(id.clone(), url.clone(), version, None));
            }
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = proxy.send_event(Event::Thumbnail(id, url, version, None));
            });
        }
    }
    fn publish_peek(&self) {
        self.emit(json!({"type":"peek","peek":self.peek.as_ref().map(|p|&p.tab),"phase":self.peek.as_ref().map(|p|p.phase),"ticket":self.peek.as_ref().map(|p|p.ticket),"origin":self.peek.as_ref().and_then(|p|p.origin),"backgroundReady":true}));
    }
    fn open_peek(
        &mut self,
        url: &str,
        origin: Option<Point>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.dismiss_tab_preview();
        let url = model::normalize(url)?;
        if !model::is_web(&url) {
            return Err("Peek 仅支持 HTTP 和 HTTPS 网页".into());
        }
        let tab = Tab {
            id: uuid::Uuid::new_v4().to_string(),
            space_id: self.state.active_space.clone(),
            title: url::Url::parse(&url)?.host_str().unwrap_or("Peek").into(),
            url,
            pinned: false,
            bookmark_id: None,
            loading: true,
            favicon: None,
            split_with: None,
            muted: false,
            zoom: None,
            custom_title: None,
            last_activated: model::now_secs(),
        };
        let page = self.build_page(&tab)?;
        // Replacing a Peek drops its controller, without stacking temporary tabs.
        self.peek = Some(Peek {
            tab,
            page,
            rect: None,
            shown: Cell::new(false),
            origin: origin.filter(|p| p.valid()),
            phase: PeekPhase::Opening,
            ticket: 1,
            surface: None,
            surface_sent: false,
            opening_capture: false,
            moving: false,
        });
        self.peek_visible.set(true);
        self.publish_peek();
        self.sync()?;
        Ok(())
    }
    fn request_peek_close(&mut self) {
        let Some(peek) = self.peek.as_mut() else {
            return;
        };
        if peek.phase == PeekPhase::Closing {
            return;
        }
        let capture = peek.phase == PeekPhase::Open && peek.shown.get();
        peek.phase = PeekPhase::Closing;
        peek.ticket += 1;
        peek.surface_sent = false;
        peek.moving = false;
        let id = peek.tab.id.clone();
        let ticket = peek.ticket;
        let proxy = self.proxy.clone();
        if capture {
            let p = proxy.clone();
            let capture_id = id.clone();
            let _ = crate::capture::background(&peek.page.view, move |image| {
                let _ = p.send_event(Event::PeekSurface(capture_id, ticket, image));
            });
            // A busy renderer must not delay dismissing the browser UI indefinitely.
            let timeout_id = id.clone();
            let fallback = proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(750));
                let _ = fallback.send_event(Event::PeekSurface(timeout_id, ticket, None));
            });
        } else {
            let _ = proxy.send_event(Event::PeekSurface(id, ticket, peek.surface.clone()));
        }
        // The live controller remains visible until Vue has decoded the proxy and acknowledges it.
        self.publish_peek();
    }
    fn capture_opening_surface(&mut self) {
        let Some(p) = self.peek.as_mut().filter(|p| {
            p.phase == PeekPhase::Opening
                && p.rect.is_some()
                && !p.tab.loading
                && !p.opening_capture
        }) else {
            return;
        };
        p.opening_capture = true;
        let proxy = self.proxy.clone();
        let id = p.tab.id.clone();
        let ticket = p.ticket;
        let _ = crate::capture::background(&p.page.view, move |image| {
            let _ = proxy.send_event(Event::PeekSurface(id, ticket, image));
        });
    }
    fn peek_origin(&self, source: &str, point: Option<Point>) -> Option<Point> {
        let rect = if let Some(p) = self.peek.as_ref().filter(|p| p.tab.id == source) {
            p.rect
        } else if source == self.state.active_tab {
            Some(self.primary)
        } else {
            self.secondary
        }?;
        if let Some(p) = point {
            return Some(Point {
                x: rect.x + p.x * rect.width,
                y: rect.y + p.y * rect.height,
            });
        }
        let ui = self.ui.as_ref()?;
        let mut cursor = POINT::default();
        // Both calls use physical pixels; convert once into the same logical units as DOM bounds.
        if unsafe { GetCursorPos(&mut cursor) }.is_ok()
            && unsafe { ScreenToClient(ui.hwnd(), &mut cursor) }.as_bool()
        {
            let scale = self.window.as_ref()?.scale_factor();
            let p = Point {
                x: cursor.x as f64 / scale,
                y: cursor.y as f64 / scale,
            };
            if p.x >= rect.x
                && p.x <= rect.x + rect.width
                && p.y >= rect.y
                && p.y <= rect.y + rect.height
            {
                return Some(p);
            }
        }
        None
    }
    fn close_peek(&mut self) {
        if self.peek.take().is_none() {
            return;
        }
        self.peek_visible.set(false);
        if let Err(e) = self.sync() {
            self.error(e)
        }
        // Source pages keep rendering beneath the popup throughout Peek dismissal.
        self.publish_peek();
        self.emit_active_zoom();
        if !self.overlay {
            if let Some(p) = self.page() {
                let _ = p.focus();
            }
        }
    }
    fn promote_peek(&mut self) {
        if self.state.tabs.len() >= 256 {
            self.error("已达到 256 个标签页上限");
            return;
        }
        let Some(peek) = self.peek.take() else { return };
        self.peek_visible.set(false);
        let id = peek.tab.id.clone();
        if self.state.settings.remember_history { self.session_writer.visit(&peek.tab.title, &peek.tab.url); }
        self.state.tabs.push(peek.tab);
        self.state.activate(&id);
        // Transfer the same controller: DOM, scroll, JS state and navigation stack remain intact.
        self.pages.insert(id, peek.page);
        self.publish_peek();
        self.save();
        self.publish();
        if let Err(e) = self.sync() {
            self.error(e)
        }
        if let Some(p) = self.page() {
            let _ = p.focus();
        }
    }
    fn bounds(&self, r: Rect) -> wry::Rect {
        let size = self
            .window
            .as_ref()
            .unwrap()
            .inner_size()
            .to_logical::<f64>(self.window.as_ref().unwrap().scale_factor());
        let x = r.x.max(0.).min(size.width);
        let y = r.y.max(32.).min(size.height);
        wry::Rect {
            position: LogicalPosition::new(x, y).into(),
            size: LogicalSize::new(
                r.width.min((size.width - x).max(0.)),
                r.height.min((size.height - y).max(0.)),
            )
            .into(),
        }
    }
    fn sync(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.window.is_none() {
            return Ok(());
        }
        self.reconcile_page_fullscreen()?;
        if !self.state.onboarding.completed { return self.sync_layout(); }
        if self.reader.as_ref().is_some_and(|r| !self.state.visible_tab(&r.id) || !self.state.tabs.iter().any(|t| t.id == r.id && t.url == r.url)) {
            self.close_reader();
        }
        if self.reader_request.as_ref().is_some_and(|(id, url, _)| !self.state.visible_tab(id) || !self.state.tabs.iter().any(|t| t.id == *id && t.url == *url)) {
            self.reader_request = None; self.reader_epoch += 1; self.publish_reader();
        }
        self.reader_available.retain(|id, _| self.state.tabs.iter().any(|t| &t.id == id));
        // State is already saved/published, but both controllers must outlive
        // the departing snapshot. Its completion event performs the cleanup.
        if self.split_exit.is_some() { return self.sync_layout(); }
        // Release native mouse capture and controller references before a tab
        // switch or close can destroy the pages the handle was resizing.
        self.finish_split_resize();
        self.sidebar_expected_width = None;
        if let Some(handle) = &self.sidebar_handle { handle.configure(None)?; }
        if let Some(handle) = &self.split_handle {
            handle.configure(None)?;
        }
        if self
            .tab_preview
            .as_ref()
            .is_some_and(|(id, _)| self.preview_tabs(id).is_none())
        {
            self.dismiss_tab_preview();
        }
        self.pages.retain(|id, _| {
            self.state
                .tabs
                .iter()
                .any(|t| &t.id == id && model::is_web(&t.url))
        });
        let sources: HashMap<_, _> = self.state.tabs.iter().map(|t| (t.id.as_str(), t.url.as_str()))
            .chain(self.state.pins.iter().chain(&self.state.favorites).map(|f| (f.id.as_str(), f.url.as_str()))).collect();
        self.thumbnails.retain_sources(|id, url| sources.get(id).copied() == Some(url));
        let active = self.state.active().cloned();
        let second = active
            .as_ref()
            .and_then(|t| t.split_with.as_ref())
            .and_then(|id| self.state.tabs.iter().find(|t| &t.id == id))
            .cloned();
        let visible: Vec<_> = active
            .into_iter()
            .chain(second)
            .filter(|t| model::is_web(&t.url))
            .collect();
        // Create on first activation. Previously opened pages stay alive but are hidden.
        for tab in &visible {
            if !self.pages.contains_key(&tab.id) {
                let page = self.build_page(tab)?;
                self.pages.insert(tab.id.clone(), page);
            }
        }
        let changed: Vec<_> = self
            .pages
            .iter()
            .filter(|(id, p)| {
                self.state
                    .tabs
                    .iter()
                    .any(|t| &t.id == *id && t.url != p.url)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in changed {
            self.invalidate_thumbnail(&id);
        }
        for (id, page) in &mut self.pages {
            if let Some(tab) = self.state.tabs.iter().find(|t| &t.id == id) {
                page.navigation.bookmarked.set(tab.bookmark_id.is_some());
                if page.url != tab.url {
                    page.navigation.host_navigation.set(true);
                    page.view.load_url(&tab.url)?;
                    page.url = tab.url.clone();
                }
            }
        }
        for page in self.pages.values().chain(self.peek.iter().map(|p| &p.page)) {
            page.set_theme(self.state.theme == "dark")?;
        }
        for render in self.preview_renders.values() {
            render.error_page.set_theme(&render.view, self.state.theme == "dark");
        }
        self.sync_layout()
    }
    fn space_form_open(&self) -> bool {
        ["spaceCreation", "spaceEditor"].iter().any(|key| {
            self.popup_state.get(*key).is_some_and(|value| !value.is_null())
        })
    }
    // Layout messages arrive every frame during sidebar motion. They must not
    // create/navigate pages, touch profile settings, or repeat ShowWindow calls.
    fn sync_page_bounds(&self) -> Result<bool, Box<dyn std::error::Error>> {
        if let Some((id, hwnd)) = &self.fullscreen_page {
            if let (Some(window), Some(page)) = (&self.window, self.page_with_handle(id, *hwnd)) {
                let size = window.inner_size();
                if size.width > 0 && size.height > 0 {
                    page.clip.clear(page.view.hwnd())?;
                    page.set_window_bounds(wry::Rect {
                        position: PhysicalPosition::new(0, 0).into(),
                        size: size.into(),
                    }, window.scale_factor())?;
                    page.sync_viewport()?;
                }
            }
            return Ok(false);
        }
        if self.split_exit.is_some() { return Ok(false); }
        if self.split_entry_pending && self.overlay { return Ok(false); }
        if self.split_handle.as_ref().is_some_and(|handle| handle.dragging())
            || self.sidebar_handle.as_ref().is_some_and(|handle| handle.dragging()) {
            return Ok(false);
        }
        let Some(window) = &self.window else {
            return Ok(false);
        };
        if !self.state.onboarding.completed || self.space_form_open() {
            let changed = self.pages.values().chain(self.peek.iter().map(|peek| &peek.page)).any(|page| page.visible.get());
            for page in self.pages.values().chain(self.peek.iter().map(|peek| &peek.page)) { page.set_visible(false)?; }
            if let Some(reader) = &self.reader { reader.original.set_visible(false)?; }
            if let Some(handle) = &self.split_handle { handle.configure(None)?; }
            if let Some(handle) = &self.sidebar_handle { handle.configure(None)?; }
            return Ok(changed);
        }
        let mut visibility_changed = false;
        let mut visibility = Vec::new();
        let mut focus_peek = false;
        let second = self.state.active().and_then(|t| t.split_with.as_ref());
        // Expose the guide and dragged row without moving or resizing the live panes.
        let drag_cutouts = |pane: Rect| self.tab_drag_cutouts.map(|cutout| {
            cutout.filter(|r| r.x < pane.x + pane.width && r.x + r.width > pane.x
                && r.y < pane.y + pane.height && r.y + r.height > pane.y)
                .map(|r| Rect { x: r.x - pane.x, y: r.y - pane.y, ..r })
        });
        let watched = self
            .tab_preview
            .as_ref()
            .and_then(|(id, _)| self.preview_tabs(id))
            .unwrap_or_default();
        for (id, page) in &self.pages {
            let r = if id == &self.state.active_tab {
                Some(self.primary)
            } else if second == Some(id) {
                self.secondary
            } else {
                None
            };
            if let Some(r) = r.filter(|r| r.width > 0. && r.height > 0.) {
                if self.sidebar_motion && !self.overlay && self.peek.is_none()
                    && page.visible.get() && !page.resize_surface.dragging()
                    && !page.resize_surface.relocating() && !self.resize_appearance.reduce_motion {
                    if let Some(bounds) = page.bounds.get() {
                        page.clip.clear(page.view.hwnd())?;
                        page.resize_surface.begin_blur(bounds, r.corners, window.scale_factor())?;
                    }
                }
                if self.live_resize { page.clip.clear(page.view.hwnd())?; }
                page.set_window_bounds(self.bounds(r), window.scale_factor())?;
                if !self.live_resize && !page.resize_surface.dragging() {
                    page.clip.apply_with_cutouts(page.view.hwnd(), r.corners, window.scale_factor(), drag_cutouts(r))?;
                }
                visibility_changed |= !page.visible.get();
                visibility.push((Some(id.as_str()), page, true));
            } else {
                // WebView2 stops painting IsVisible=false controllers. Let only the
                // watched pages render behind the trusted UI while their card is up.
                let visible = watched.contains(id);
                visibility_changed |= page.visible.get() != visible;
                visibility.push((Some(id.as_str()), page, visible));
            }
        }
        if let Some(peek) = &self.peek {
            if let Some(r) = peek.rect.filter(|r| r.width > 0. && r.height > 0.) {
                let visible = peek.phase == PeekPhase::Open
                        || (peek.phase == PeekPhase::Closing && !peek.moving && peek.shown.get());
                let was_visible = peek.shown.replace(visible);
                visibility_changed |= was_visible != visible;
                peek.page.set_window_bounds(self.bounds(r), window.scale_factor())?;
                peek.page
                    .clip
                    .apply(peek.page.view.hwnd(), r.corners, window.scale_factor())?;
                visibility.push((None, &peek.page, visible));
                focus_peek = !was_visible && visible;
            } else {
                visibility_changed |= peek.shown.get();
                peek.shown.set(false);
                visibility.push((None, &peek.page, false));
            }
        }
        if let Some(reader) = &self.reader {
            let rect = if reader.id == self.state.active_tab { Some(self.primary) }
                else if second == Some(&reader.id) { self.secondary } else { None };
            if let Some(rect) = rect.filter(|r| r.width > 0. && r.height > 0.) {
                // Keep the preserved page behind the transparent paper through
                // entry/exit, with matching split bounds and popup cutouts.
                let original = &reader.original;
                original.set_window_bounds(self.bounds(rect), window.scale_factor())?;
                original.sync_viewport()?;
                original.clip.apply_with_cutouts(original.view.hwnd(), rect.corners, window.scale_factor(), drag_cutouts(rect))?;
                original.set_visible(reader.transitioning)?;
                if reader.transitioning {
                    if let Some(page) = self.pages.get(&reader.id) {
                        unsafe { SetWindowPos(original.view.hwnd(), Some(page.view.hwnd()), 0, 0, 0, 0,
                            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)?; }
                    }
                }
            } else { reader.original.set_visible(false)?; }
        }
        // Both live edges move together before either renderer reflows.
        for page in self.pages.values().chain(self.peek.iter().map(|p| &p.page)) {
            page.sync_viewport()?;
        }
        // Newly shown pages need their first viewport before becoming visible.
        // A page that was asleep wakes here, and the sidebar's ring goes away.
        for (id, page, visible) in visibility {
            page.set_visible(visible)?;
            if visible && page.sleeping.get() {
                page.wake();
                if let Some(id) = id {
                    self.emit(json!({"type":"sleep","id":id,"asleep":false}));
                }
            }
        }
        if focus_peek {
            if let Some(peek) = &self.peek { peek.page.view.focus()?; }
        }
        if let Some(handle) = &self.split_handle {
            let config = self.secondary.filter(|r| r.width > 0. && r.height > 0.)
                .filter(|_| second.is_some() && !self.overlay && self.peek.is_none())
                .map(|secondary| crate::split::Config {
                    id: self.state.active_tab.clone(),
                    other: second.unwrap().clone(),
                    layout: crate::split::Layout { primary: self.primary, secondary },
                    scale: window.scale_factor(),
                    left: self.pages.get(&self.state.active_tab).map(Page::split_pane),
                    right: second.and_then(|id| self.pages.get(id)).map(Page::split_pane),
                });
            handle.configure(config)?;
        }
        if let Some(handle) = &self.sidebar_handle {
            let config = self.sidebar_rect.filter(|r| r.width > 0. && r.height > 0.)
                .filter(|_| !self.state.sidebar_collapsed && !self.overlay && self.peek.is_none() && self.tab_preview.is_none())
                .zip(self.layout_viewport)
                .map(|(rect, viewport)| crate::sidebar::Config {
                    id: self.state.active_tab.clone(), other: second.cloned(),
                    primary: self.primary, secondary: self.secondary, handle: rect,
                    preferred: self.state.sidebar_width, viewport, scale: window.scale_factor(),
                    left: self.pages.get(&self.state.active_tab).map(Page::split_pane),
                    right: second.and_then(|id| self.pages.get(id)).map(Page::split_pane),
                });
            handle.configure(config)?;
        }
        Ok(visibility_changed)
    }
    fn receive_split_resize(&mut self) {
        let Some(update) = self.split_handle.as_ref().and_then(|handle| handle.take_update()) else { return; };
        // A tab switch, close or a new split can invalidate a queued update.
        // Never apply the previous pair's dimensions to the new page.
        if update.id != self.state.active_tab || self.state.active().and_then(|t| t.split_with.as_ref()) != Some(&update.other) {
            self.live_resize = false;
            return;
        }
        self.primary = update.layout.primary;
        self.secondary = Some(update.layout.secondary);
        self.live_resize = update.resizing;
        // The mouse callback already resized the pages. This only updates the
        // shell headers and drag decoration, without another renderer round trip.
        if let Some(ui) = &self.ui {
            let event = json!({ "type": "splitResize", "ratio": update.layout.ratio(), "resizing": update.resizing });
            let _ = ui.evaluate_script(&format!("window.__ERA_RECEIVE__?.({event})"));
        }
        if let Some(error) = update.error { self.error(error); }
    }
    fn swap_split(&mut self, id: &str, other: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.state.active_tab != id || self.state.active().and_then(|tab| tab.split_with.as_deref()) != Some(other) {
            return Ok(());
        }
        self.dismiss_tab_preview();
        self.finish_split_resize();
        let Some((window, secondary)) = self.window.as_ref().zip(self.secondary) else { return Ok(()); };
        let scale = window.scale_factor();
        let layout = crate::split::Layout { primary: self.primary, secondary };
        let swapped = layout.swapped();
        self.state.apply(&Command::SwapSplit { id: id.into(), other: other.into() })?;
        if self.state.active_tab != other { return Ok(()); }
        self.pending_layout.take();
        self.primary = swapped.primary;
        self.secondary = Some(swapped.secondary);
        self.live_resize = false;
        self.split_entry_pending = false;
        self.presented_split = Some((other.into(), id.into()));

        let animate = (|| -> Result<(), Box<dyn std::error::Error>> {
            let panes: Vec<_> = [(other, layout.secondary, swapped.primary, -1.), (id, layout.primary, swapped.secondary, 1.)]
                .into_iter().filter_map(|(id, from, to, edge)| {
                    let page = self.pages.get(id).filter(|page| page.visible.get())?;
                    Some((page.split_pane(), page.bounds.get()?, from, to, edge))
                }).collect();
            let clock = crate::resize_surface::SwapClock::new(panes.len());
            for (pane, bounds, from, to, edge) in panes {
                let rect = self.bounds(to);
                let position = rect.position.to_physical::<i32>(scale);
                let size = rect.size.to_physical::<i32>(scale);
                let destination = [position.x, position.y, size.width, size.height];
                pane.surface.clone().swap_to(bounds, from.corners, scale, destination, edge, clock.clone(), Box::new(move || {
                    unsafe {
                        SetWindowPos(pane.hwnd, None, destination[0], destination[1], destination[2], destination[3], SWP_NOACTIVATE | SWP_NOZORDER)?;
                        if pane.viewport.get() != Some([destination[2], destination[3]]) {
                            pane.controller.SetBounds(RECT { left: 0, top: 0, right: destination[2], bottom: destination[3] })?;
                            pane.viewport.set(Some([destination[2], destination[3]]));
                        }
                    }
                    pane.bounds.set(Some(destination));
                    pane.clip.apply(pane.hwnd, to.corners, scale)
                }))?;
            }
            Ok(())
        })();
        if let Err(error) = animate {
            self.finish_split_resize();
            self.error(error);
        }
        self.save();
        self.emit(json!({"type":"splitResize", "ratio":swapped.ratio(), "resizing":false}));
        self.publish();
        self.sync_layout()?;
        self.emit_active_zoom();
        Ok(())
    }
    fn animate_split_entry(&mut self) {
        if !self.split_entry_pending || self.live_resize || self.overlay || self.peek.is_some() || self.fullscreen_page.is_some() { return; }
        self.split_entry_pending = false;
        let current = self.state.active().and_then(|tab| tab.split_with.as_ref())
            .map(|other| (self.state.active_tab.clone(), other.clone()));
        if current != self.presented_split { return; }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let Some(window) = &self.window else { return Ok(()); };
            let Some((left, right)) = current.as_ref() else { return Ok(()); };
            let scale = window.scale_factor();
            let incoming = self.pages.get(right).map(|page| page.resize_surface.clone());
            let mut outgoing = None;
            if let Some(page) = self.pages.get(left).filter(|page| page.visible.get()) {
                if let Some(from) = page.bounds.get() {
                    let rect = self.bounds(self.primary);
                    let position = rect.position.to_physical::<i32>(scale);
                    let size = rect.size.to_physical::<i32>(scale);
                    let to = [position.x, position.y, size.width, size.height];
                    if from != to {
                        page.resize_surface.begin(from, self.primary.corners, scale)?;
                        let hwnd = page.view.hwnd();
                        let controller = page.view.controller();
                        let viewport = page.viewport.clone();
                        let clip = page.clip.clone();
                        let corners = self.primary.corners;
                        let incoming = incoming.clone();
                        let cancelled = incoming.clone();
                        page.resize_surface.relocate(to, Box::new(move || {
                            unsafe {
                                SetWindowPos(hwnd, None, to[0], to[1], to[2], to[3], SWP_NOACTIVATE | SWP_NOZORDER)?;
                                controller.SetBounds(RECT { left: 0, top: 0, right: to[2], bottom: to[3] })?;
                            }
                            clip.apply(hwnd, corners, scale)?;
                            viewport.set(Some([to[2], to[3]]));
                            Ok(())
                        }), Box::new(move || {
                            if let Some(surface) = incoming { surface.reveal_entry()?; }
                            Ok(())
                        }), Box::new(move || {
                            if let Some(surface) = cancelled { surface.end(); }
                            Ok(())
                        }));
                        outgoing = Some(page.resize_surface.clone());
                    }
                }
            }
            self.sync_layout()?;
            if let Some((page, rect)) = self.pages.get(right).zip(self.secondary) {
                if let Some(bounds) = page.bounds.get() {
                    if outgoing.is_some() {
                        page.resize_surface.stage_entry(bounds, rect.corners, scale)?;
                    } else {
                        page.resize_surface.enter(bounds, rect.corners, scale, -1.)?;
                    }
                }
            }
            if let Some(surface) = outgoing { surface.raise()?; }
            Ok(())
        })();
        if let Err(error) = result {
            for page in self.pages.values() { page.resize_surface.end(); }
            if let Err(error) = self.sync_layout() { self.error(error); }
            self.error(error);
        }
    }
    fn begin_split_exit(&mut self, command: &Command) -> Result<bool, Box<dyn std::error::Error>> {
        if self.fullscreen_page.is_some() { return Ok(false); }
        let Some(active) = self.state.active() else { return Ok(false); };
        let Some(other) = active.split_with.as_ref() else { return Ok(false); };
        let left = active.id.clone();
        let right = other.clone();
        let (keeper, departing, side) = match command {
            Command::Unsplit => (left, right, 1.),
            Command::Close { id } if id.as_ref().unwrap_or(&left) == &left => (right, left, -1.),
            Command::Close { id: Some(id) } if id == &right => (left, right, 1.),
            _ => return Ok(false),
        };
        let Some(window) = &self.window else { return Ok(false); };
        let scale = window.scale_factor();
        if self.peek.is_some() || self.secondary.is_none()
            || [&keeper, &departing].iter().any(|id| self.pages.get(*id)
                .is_none_or(|page| !page.visible.get() || page.bounds.get().is_none())) {
            return Ok(false);
        }
        if let Some(handle) = &self.split_handle { handle.cancel(); handle.configure(None)?; }
        if let Some(handle) = &self.sidebar_handle { handle.cancel(); handle.configure(None)?; }
        self.receive_split_resize();
        self.receive_sidebar_resize();
        self.pending_layout.take();
        self.sidebar_motion = false;
        self.live_resize = false;
        self.split_entry_pending = false;
        for id in [&keeper, &departing] {
            let page = &self.pages[id];
            let corners = if id == &self.state.active_tab { self.primary.corners }
                else { self.secondary.unwrap().corners };
            let bounds = page.resize_surface.hold(page.bounds.get().unwrap(), corners, scale)?;
            page.bounds.set(Some(bounds));
        }
        self.split_exit_ticket = self.split_exit_ticket.wrapping_add(1);
        let ticket = self.split_exit_ticket;
        let proxy = self.proxy.clone();
        self.pages[&departing].resize_surface.exit(side, Box::new(move || {
            let _ = proxy.send_event(Event::SplitExited(ticket));
            Ok(())
        }))?;
        self.split_exit = Some(SplitExit { keeper, departing, ticket, departed: false, layout_ready: false });
        Ok(true)
    }
    fn recover_split_exit(&mut self) {
        if !self.split_exit.as_ref().is_some_and(|exit| exit.departed && exit.layout_ready) { return; }
        let exit = self.split_exit.take().unwrap();
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let Some(page) = self.pages.get(&exit.keeper) else { return Ok(()); };
            let Some(window) = &self.window else { return Ok(()); };
            let scale = window.scale_factor();
            let rect = self.bounds(self.primary);
            let position = rect.position.to_physical::<i32>(scale);
            let size = rect.size.to_physical::<i32>(scale);
            let to = [position.x, position.y, size.width, size.height];
            let hwnd = page.view.hwnd();
            let controller = page.view.controller();
            let viewport = page.viewport.clone();
            let clip = page.clip.clone();
            let corners = self.primary.corners;
            page.resize_surface.relocate(to, Box::new(move || {
                unsafe {
                    SetWindowPos(hwnd, None, to[0], to[1], to[2], to[3], SWP_NOACTIVATE | SWP_NOZORDER)?;
                    controller.SetBounds(RECT { left: 0, top: 0, right: to[2], bottom: to[3] })?;
                }
                clip.apply(hwnd, corners, scale)?;
                viewport.set(Some([to[2], to[3]]));
                Ok(())
            }), Box::new(|| Ok(())), Box::new(|| Ok(())));
            self.presented_split = None;
            self.pages.retain(|id, _| self.state.tabs.iter().any(|tab| &tab.id == id));
            self.sync_layout()?;
            Ok(())
        })();
        if let Err(error) = result {
            self.finish_split_resize();
            self.error(error);
            if let Err(error) = self.sync() { self.error(error); }
        }
    }
    fn finish_split_resize(&mut self) {
        // Invalidate any queued completion before dropping its frame/controller.
        self.split_exit = None;
        if let Some(handle) = &self.split_handle { handle.cancel(); }
        if let Some(handle) = &self.sidebar_handle { handle.cancel(); }
        // Consume the final dimensions before any ordinary layout can reapply
        // an older frame during a navigation or overlay transition.
        self.receive_split_resize();
        self.receive_sidebar_resize();
        self.sidebar_motion = false;
        let mut errors = vec![];
        for page in self.pages.values() {
            // A cancelled sidebar animation has cached presentation bounds;
            // commit those before dropping its surface or the HWND stays behind.
            if page.resize_surface.dragging() && !page.resize_surface.relocating() {
                if let Err(error) = page.split_pane().end_preview(false) { errors.push(error); }
            }
            page.resize_surface.end();
        }
        for error in errors { self.error(error); }
    }
    fn receive_sidebar_resize(&mut self) {
        let Some(update) = self.sidebar_handle.as_ref().and_then(|handle| handle.take_update()) else { return; };
        let c = update.config;
        if c.id != self.state.active_tab || c.other.as_ref() != self.state.active().and_then(|t| t.split_with.as_ref()) { return; }
        self.primary = c.primary;
        self.secondary = c.secondary;
        self.sidebar_rect = Some(c.handle);
        self.sidebar_expected_width = Some(c.width());
        self.live_resize = update.resizing;
        self.pending_layout.take();
        if let Some(ui) = &self.ui {
            if update.commit {
                // The native strip temporarily owns keyboard focus for Escape.
                // WebView2's old focus can be a foreign-thread child (or NULL);
                // restore the trusted controller and accessible separator.
                let _ = ui.focus();
                let _ = ui.evaluate_script("document.querySelector('.sidebar-resize-handle')?.focus({preventScroll:true})");
            }
        }
        if update.commit && self.state.sidebar_width != c.preferred {
            self.state.sidebar_width = c.preferred;
            self.save(); self.publish();
        }
        if let Some(error) = update.error { self.error(error); }
    }
    fn sync_layout(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.reconcile_page_fullscreen()?;
        if self.reader.is_some() { self.publish_reader(); }
        self.sync_page_bounds()?;
        if let Some((id, hwnd)) = &self.fullscreen_page {
            if let Some(page) = self.page_with_handle(id, *hwnd) {
                unsafe { SetWindowPos(page.view.hwnd(), Some(HWND_TOP), 0, 0, 0, 0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)?; }
            }
            return Ok(());
        }
        // Stacking changes on page/overlay transitions, not on every drag frame.
        let watched = self.tab_preview.as_ref()
            .and_then(|(id, _)| self.preview_tabs(id)).unwrap_or_default();
        if let Some(ui) = &self.ui {
            unsafe {
                SetWindowPos(
                    ui.hwnd(),
                    Some(HWND_BOTTOM),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                )?;
                if let Some(p) = self
                    .peek
                    .as_ref()
                    .filter(|p| p.shown.get())
                {
                    SetWindowPos(
                        p.page.view.hwnd(),
                        Some(HWND_TOP),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                    )?;
                }
            }
        }
        if let Some(popup) = &self.popup {
            unsafe { SetWindowPos(popup.hwnd(), Some(HWND_TOP), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)?; }
        }
        if let Some(extension) = &self.extension_view {
            unsafe { SetWindowPos(extension.view.hwnd(), Some(HWND_TOP), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)?; }
        }
        for id in watched {
            if let Some(page) = self.pages.get(&id) {
                // Below the UI, including its new-tab surface; never over an active page.
                unsafe {
                    SetWindowPos(
                        page.view.hwnd(),
                        Some(HWND_BOTTOM),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                    )?;
                }
            }
        }
        for render in self.preview_renders.values() {
            render.view.set_visible(true)?;
            unsafe { SetWindowPos(render.view.hwnd(), Some(HWND_BOTTOM), 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)?; }
        }
        Ok(())
    }
    fn shortcut(&mut self, key: String, shift: bool) {
        if self.quit_pending { return; }
        if !self.state.onboarding.completed { return; }
        if self.fullscreen_page.is_some() {
            if let Err(error) = self.restore_page_window(true).and_then(|_| self.sync_layout()) {
                self.error(error);
            }
        }
        if let Some(index) = key.strip_prefix("space:").and_then(|i| i.parse::<usize>().ok()) {
            if let Some(space) = self.state.spaces.get(index) { self.command(Command::SwitchSpace { id: space.id.clone() }, None); }
            return;
        }
        match key.as_str() {
            "address" | "new" | "desk" | "settings" | "downloads" | "downloadsEscape" => {
                if let Some(ui) = &self.ui {
                    let _ = ui.focus();
                }
                self.emit(json!({"type":"shortcut","action":key}));
            }
            "cycle" | "cyclePrevious" => {
                let ids = self.state.navigable_ids();
                if !ids.is_empty() {
                    let i = ids
                        .iter()
                        .position(|id| id == &self.state.active_tab)
                        .unwrap_or(0);
                    let next = if key == "cyclePrevious" || shift {
                        (i + ids.len() - 1) % ids.len()
                    } else {
                        (i + 1) % ids.len()
                    };
                    self.command(
                        Command::Activate {
                            id: ids[next].clone(),
                        },
                        None,
                    )
                }
            }
            "close" => self.command(Command::Close { id: None }, None),
            "restore" => self.command(Command::Restore, None),
            "sidebar" => self.command(Command::Sidebar, None),
            "sidebarFocus" => self.command(Command::SidebarFocus, None),
            "reload" => self.command(Command::Reload { id: None }, None),
            "back" => self.command(Command::Back { id: None }, None),
            "forward" => self.command(Command::Forward { id: None }, None),
            "zoomIn" => self.command(Command::Zoom { action: "in".into() }, None),
            "zoomOut" => self.command(Command::Zoom { action: "out".into() }, None),
            "zoomReset" => self.command(Command::Zoom { action: "reset".into() }, None),
            "reader" => self.command(Command::Reader { id: None }, None),
            "readerEscape" => self.command(Command::ReaderClose, None),
            "find" => self.command(Command::FindOpen, None),
            "findEscape" => self.command(Command::FindClose, None),
            "printPage" => self.command(Command::PrintPage, None),
            "savePageAs" => self.command(Command::SavePageAs, None),
            "escape" => self.request_peek_close(),
            _ => {}
        }
    }
    fn publish_extensions(&self, items: Option<Vec<extensions::Item>>) {
        self.emit(json!({
            "type": "extensionsState",
            "extensions": {
                "items": items,
                "review": self.extension_candidate.as_ref().map(|extension| &extension.review),
                "busy": self.extension_preparation.is_some(),
                "popup": self.extension_popup.is_some(),
                "error": self.extension_error,
            }
        }));
    }

    fn close_extension_menu(&mut self) {
        self.extension_view.take();
        if self.popup_state.remove("extensionMenuAnchor").is_some_and(|anchor| !anchor.is_null()) {
            self.emit(json!({"type":"popupState","popup":{"extensionMenuAnchor":null}}));
        }
    }

    fn can_show_extension_view(&self, rect: Rect) -> bool {
        rect.valid() && rect.width > 0. && rect.height > 0.
            && !self.overlay && !self.quit_pending
            && self.fullscreen_page.is_none() && self.extension_popup.is_none()
    }

    fn extension_menu_tab(&self) -> Option<&str> {
        self.popup_state.get("extensionMenuAnchor")?.get("tabId")?.as_str()
    }

    fn open_extension_view(&mut self, id: &str, rect: Rect) -> Result<(), Box<dyn std::error::Error>> {
        self.extension_view.take();
        let page = self.extension_profile(|profile, root| profile.action_page(root, id))?;
        let active_url = self.extension_menu_tab()
            .filter(|id| self.state.visible_tab(id))
            .and_then(|id| self.pages.get(id))
            .map(|page| page.navigation.current.borrow().clone())
            .filter(|url| model::is_web(url));
        let window = self.window.as_ref().ok_or("窗口尚未就绪")?;
        // The initial WebView is hidden while measuring. Lay it out toward the
        // window interior so the toolbar anchor doesn't restrict the probe width.
        let bounds = self.bounds(Rect { x: 0., height: 470., width: 458., ..rect });
        let ticket = uuid::Uuid::new_v4();
        let proxy = self.proxy.clone();
        let view = crate::extension_view::build(browsing_webview(&mut self.context), window, &page.url, active_url.as_deref(), bounds, move |event| {
            let _ = proxy.send_event(Event::ExtensionView(ticket, event));
        })?;
        self.extension_view = Some(ExtensionView {
            ticket, view, fit_content: page.fit_content, ready: false,
            visible: false, size: None, next_measure: None,
        });
        Ok(())
    }

    fn measure_extension_view(&mut self) {
        let Some(extension) = self.extension_view.as_mut() else { return };
        let proxy = self.proxy.clone();
        let ticket = extension.ticket;
        extension.next_measure = None;
        if crate::extension_view::measure(&extension.view, move |event| {
            let _ = proxy.send_event(Event::ExtensionView(ticket, event));
        }).is_err() {
            let _ = self.proxy.send_event(Event::ExtensionView(ticket, crate::extension_view::Event::Measured(None)));
        }
    }

    fn update_extension_size(&mut self, size: Option<[u32; 2]>) {
        let Some(extension) = self.extension_view.as_mut() else { return };
        let size = size.or(extension.size);
        let changed = !extension.ready || extension.size != size;
        extension.size = size;
        extension.ready = true;
        if extension.fit_content {
            extension.next_measure = Some(Instant::now() + Duration::from_millis(200));
        }
        if changed {
            self.emit(json!({"type":"extensionView","phase":"ready","size":size}));
        }
    }

    fn resize_extension_view(&mut self, rect: Rect) {
        if !self.can_show_extension_view(rect) { return }
        let bounds = self.bounds(rect);
        let Some(extension) = self.extension_view.as_mut() else { return };
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            extension.view.set_bounds(bounds)?;
            if extension.ready && !extension.visible {
                unsafe { SetWindowPos(extension.view.hwnd(), Some(HWND_TOP), 0, 0, 0, 0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE)?; }
                extension.view.set_visible(true)?;
                extension.view.focus()?;
                extension.visible = true;
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.extension_view.take();
            self.emit(json!({"type":"extensionView","phase":"error","message":error.to_string()}));
        }
    }

    fn emit_extensions(&mut self, items: Option<Vec<extensions::Item>>, error: Option<String>) {
        self.extension_error = error;
        self.publish_extensions(items);
    }

    fn prepare_extension(
        &mut self,
        prepare: impl FnOnce(&FilePath) -> Result<extensions::Prepared, String> + Send + 'static,
    ) {
        let request = uuid::Uuid::new_v4();
        self.extension_candidate = None;
        self.extension_preparation = Some(request);
        self.emit_extensions(None, None);
        let root = self.extensions_root.clone();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let _ = proxy.send_event(Event::ExtensionPrepared(request, prepare(&root)));
        });
    }

    fn capture_extension_background(&self, popup: uuid::Uuid) {
        let Some(active) = self.state.active() else { return };
        for (id, rect) in std::iter::once((&active.id, self.primary))
            .chain(active.split_with.as_ref().zip(self.secondary))
        {
            let Some(page) = self.pages.get(id).filter(|page| page.visible.get()) else { continue };
            let proxy = self.proxy.clone();
            let _ = crate::capture::thumbnail(&page.view, move |image| {
                if let Some(image) = image {
                    let _ = proxy.send_event(Event::ExtensionBackground(popup, rect, image));
                }
            });
        }
    }

    fn extension_profile<T>(
        &mut self,
        run: impl FnOnce(extensions::Profile, &FilePath) -> Result<T, Box<dyn std::error::Error>>,
    ) -> Result<T, Box<dyn std::error::Error>> {
        if let Some(page) = self.pages.values().next() {
            return run(extensions::Profile::new(&page.view)?, &self.extensions_root);
        }
        // Settings can be the only open tab. Use the browsing profile without
        // keeping an extra WebView that extensions would see as a tab.
        let window = self.window.as_ref().ok_or("窗口尚未就绪")?;
        let view = browsing_webview(&mut self.context)
            .with_url("about:blank")
            .with_visible(false)
            .with_focused(false)
            .build_as_child(window)?;
        run(extensions::Profile::new(&view)?, &self.extensions_root)
    }

    fn extension_command(&mut self, command: Command) {
        if matches!(command, Command::DismissExtensionPopup) {
            if self.extension_popup.take().is_some() {
                self.extension_preparation = None;
                self.extension_candidate = None;
                self.emit_extensions(None, None);
            }
            return;
        }
        if self.extension_preparation.is_some() && !matches!(command, Command::ListExtensions) { return; }
        if matches!(command, Command::EnableExtension { .. } | Command::RemoveExtension { .. }) {
            self.close_extension_menu();
        }
        let result = match command {
            Command::ChooseExtension => {
                let owner = self.window.as_ref().and_then(|window| window.window_handle().ok())
                    .and_then(|handle| match handle.as_raw() {
                        RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut _)),
                        _ => None,
                    });
                let Some(owner) = owner else {
                    self.emit_extensions(None, Some("无法打开扩展目录选择窗口".into()));
                    return;
                };
                self.extension_candidate = None;
                match extensions::choose_folder(owner) {
                    Ok(Some(path)) => {
                        self.prepare_extension(move |root| {
                            extensions::prepare(root, &path)
                                .map_err(|error| format!("无法读取扩展目录：{error}"))
                        });
                    }
                    result => {
                        self.emit_extensions(None, result.err().map(|error| format!("无法选择扩展目录：{error}")));
                    }
                }
                return;
            }
            Command::PrepareChromeStoreExtension { input } => {
                let Some(id) = chrome_store::input_id(&input) else {
                    self.emit_extensions(None, Some("请输入 Chrome 网上应用店的扩展详情链接或扩展 ID".into()));
                    return;
                };
                let runtime = match wry::webview_version() {
                    Ok(version) => version,
                    Err(_) => {
                        self.emit_extensions(None, Some("无法读取 WebView2 Runtime 版本".into()));
                        return;
                    }
                };
                self.prepare_extension(move |root| {
                    extensions::prepare_store(root, &id, &runtime)
                        .map_err(|error| format!("无法准备商店扩展：{error}"))
                });
                return;
            }
            Command::ListExtensions => {
                match self.extension_profile(|profile, root| profile.list(root)) {
                    Ok(items) => self.publish_extensions(Some(items)),
                    Err(error) if self.extension_popup.is_none() => {
                        self.emit_extensions(None, Some(format!("无法读取扩展列表：{error}")));
                    }
                    Err(_) => self.publish_extensions(None),
                }
                return;
            }
            Command::ReviewExtension { ticket, install } => {
                if !self.extension_candidate.as_ref().is_some_and(|candidate| candidate.matches(&ticket)) {
                    self.emit_extensions(None, Some("这次安装确认已失效，请重新选择目录".into()));
                    return;
                }
                let candidate = self.extension_candidate.take().unwrap();
                if !install {
                    drop(candidate);
                    self.extension_popup = None;
                    self.emit_extensions(None, None);
                    return;
                }
                let result = self.extension_profile(|profile, root| {
                    profile.install(root, candidate)?;
                    profile.list(root)
                });
                if result.is_ok() && self.extension_popup.take().is_some() {
                    let message = match self.state.settings.language.as_str() {
                        "en" => "Extension installed",
                        "zh-TW" => "已安裝擴充功能",
                        _ => "扩展已安装",
                    };
                    self.toast(message, None);
                }
                result
            }
            Command::EnableExtension { id, enabled } => self.extension_profile(|profile, root| {
                profile.enable(root, &id, enabled)?;
                profile.list(root)
            }),
            Command::RemoveExtension { id } => {
                let result = self.extension_profile(|profile, root| {
                    let removed = profile.remove(root, &id);
                    Ok((profile.list(root)?, removed.err()))
                });
                match result {
                    Ok((items, error)) => self.emit_extensions(Some(items), error.map(|error| error.to_string())),
                    Err(error) => self.emit_extensions(None, Some(format!("卸载扩展失败：{error}"))),
                }
                return;
            }
            _ => return,
        };
        match result {
            Ok(items) => self.emit_extensions(Some(items), None),
            Err(error) => self.emit_extensions(None, Some(format!("扩展操作失败：{error}"))),
        }
    }

    fn command(&mut self, mut c: Command, event_loop: Option<&ActiveEventLoop>) {
        if let Command::ResizeAppearance { appearance } = &c {
            if appearance.angle.is_finite() {
                self.resize_appearance = *appearance;
                for page in self.pages.values().chain(self.peek.iter().map(|p| &p.page)) {
                    page.resize_surface.set_appearance(*appearance);
                }
                if let Some(reader) = &self.reader {
                    if let Some(page) = self.pages.get(&reader.id) {
                        let _ = page.view.evaluate_script(&format!("document.documentElement.dataset.reduceMotion='{}'", appearance.reduce_motion));
                    }
                }
            }
            return;
        }
        if self.import_busy && matches!(c, Command::OnboardingOpen | Command::OnboardingBack { .. } | Command::OnboardingAdvance { .. }) {
            self.error("正在导入，请稍候"); return;
        }
        if matches!(c, Command::ImportScan | Command::ImportRun { .. } | Command::ImportPasswords) {
            if self.import_busy { self.error("正在导入或检查，请稍候"); return; }
            if !self.state.onboarding.completed && self.state.onboarding.step != crate::onboarding::Step::Bring { return; }
            match c {
                Command::ImportScan => {
                    self.import_busy = true;
                    self.emit(json!({"type":"importState","importBusy":true}));
                    let proxy = self.proxy.clone();
                    std::thread::spawn(move || { let _ = proxy.send_event(Event::ImportSources(crate::import::detect())); });
                }
                Command::ImportRun { request } => {
                    let unique: std::collections::HashSet<_> = request.sources.iter().collect();
                    let sources: Vec<_> = request.sources.iter().filter_map(|id| self.import_sources.iter().find(|s| &s.id == id).cloned()).collect();
                    if sources.is_empty() || sources.len() > 20 || unique.len() != sources.len() || request.sources.len() != sources.len() || (!request.bookmarks && !request.history) { self.error("请选择有效的浏览器和数据类型"); return; }
                    self.import_busy = true;
                    self.emit(json!({"type":"importState","importBusy":true}));
                    let proxy = self.proxy.clone();
                    std::thread::spawn(move || { let batches = sources.iter().map(|source| crate::import::read_browser(source, &request)).collect(); let _ = proxy.send_event(Event::ImportRead(batches)); });
                }
                Command::ImportPasswords => {
                    let owner = self.window.as_ref().and_then(|w| w.window_handle().ok()).and_then(|handle| match handle.as_raw() { RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)), _ => None });
                    match crate::passwords::choose_csv(owner) {
                        Ok(Some(path)) => {
                            self.import_busy = true;
                            self.emit(json!({"type":"importState","importBusy":true}));
                            let proxy = self.proxy.clone();
                            std::thread::spawn(move || { let _ = proxy.send_event(Event::ImportRead(vec![crate::import::read_csv(&path)])); });
                        }
                        Ok(None) => self.emit(json!({"type":"importState","importBusy":false})),
                        Err(e) => self.error(e),
                    }
                }
                _ => {},
            }
            return;
        }
        if matches!(c, Command::OnboardingOpen | Command::OnboardingBack { .. } | Command::OnboardingAdvance { .. }) {
            if matches!(c, Command::OnboardingOpen) {
                self.finish_split_resize(); self.dismiss_tab_preview(); self.close_peek(); self.close_find(true);
                self.overlay = false;
                self.tab_drag_cutouts = [None; 2];
            }
            let make_default = matches!(c, Command::OnboardingAdvance { step: crate::onboarding::Step::Ready, make_default: true, .. });
            if let Err(e) = self.state.apply(&c) { self.error(e); return; }
            if matches!(c, Command::OnboardingAdvance { step: crate::onboarding::Step::Ready, .. }) {
                // Wait for the browser surface to report its new bounds after the greeting.
                self.pending_layout.take();
                self.primary = Rect::default();
                self.secondary = None;
                self.sidebar_rect = None;
            }
            self.open_external_urls(event_loop);
            self.save(); self.publish();
            if let Err(e) = self.sync() { self.error(e); }
            if let Some(ui) = &self.ui { let _ = ui.focus(); }
            // Windows owns the final choice. Opening Settings never claims that
            // Rove has become the default browser.
            if make_default { self.command(Command::SystemSettings, event_loop); }
            return;
        }
        let onboarding_edit = match &c {
            Command::UpdateSpace { id, .. } => self.state.onboarding.step == crate::onboarding::Step::Spaces && id == &self.state.active_space,
            Command::Theme => self.state.onboarding.step == crate::onboarding::Step::Spaces,
            Command::UpdateSettings { .. } => self.state.onboarding.step == crate::onboarding::Step::Search,
            _ => false,
        };
        if !self.state.onboarding.completed && !onboarding_edit && !matches!(c,
            Command::Ready | Command::Presented | Command::Window { .. } | Command::QuitResponse { .. } | Command::WindowChrome { .. }
            | Command::Layout { .. } | Command::Glass { .. } | Command::PopupReady
            | Command::PopupState { .. } | Command::PopupLayer { .. } | Command::Overlay { .. }
        ) { return; }
        if let Command::PrepareChromeStoreTab { id, expected_url } = &c {
            if self.extension_preparation.is_some() || self.extension_popup.is_some() || self.quit_pending {
                return;
            }
            let extension_id = self.state.active()
                .filter(|tab| &tab.id == id || tab.split_with.as_ref() == Some(id))
                .and_then(|_| self.pages.get(id))
                .and_then(|page| {
                    let current = page.navigation.current.borrow();
                    if expected_url.as_ref().is_some_and(|url| url != &*current) {
                        return None;
                    }
                    chrome_store::detail_id(&url::Url::parse(&current).ok()?).map(str::to_owned)
                });
            if let Some(input) = extension_id {
                self.close_extension_menu();
                let popup = uuid::Uuid::new_v4();
                self.extension_popup = Some(popup);
                self.extension_candidate = None;
                self.extension_command(Command::PrepareChromeStoreExtension { input });
                self.capture_extension_background(popup);
            } else if expected_url.is_none() {
                self.error("当前标签页不是 Chrome 网上应用店的扩展详情页");
            }
            return;
        }
        if matches!(c, Command::ListExtensions | Command::ChooseExtension | Command::PrepareChromeStoreExtension { .. } | Command::ReviewExtension { .. } | Command::DismissExtensionPopup
            | Command::EnableExtension { .. } | Command::RemoveExtension { .. }) {
            self.extension_command(c);
            return;
        }
        if let Command::UpdateSettings { settings } = &mut c {
            if self.updating {
                settings.update_channel.clone_from(&self.state.settings.update_channel);
            }
        }
        // A find session lives on one page. Switching, closing, splitting or
        // opening a new tab ends it, so the bar never highlights a hidden page.
        if (self.find.is_some() || self.find_open.get())
            && matches!(
                c,
                Command::New { .. }
                    | Command::Activate { .. }
                    | Command::OpenBookmark { .. }
                    | Command::Navigate { .. }
                    | Command::Close { .. }
                        | Command::CloseOthers { .. }
                        | Command::CloseBelow { .. }
                    | Command::SwitchSpace { .. }
                    | Command::CreateSpace { .. }
                    | Command::Restore
                    | Command::Move { .. }
                    | Command::OpenSettings { .. }
                    | Command::Split { .. }
                    | Command::SwapSplit { .. }
                    | Command::Unsplit
                    | Command::PeekOpen { .. }
                    | Command::PeekClose
                    | Command::PeekPromote
            )
        {
            self.close_find(true);
        }
        if matches!(c, Command::OpenSettings { .. }) { self.close_peek(); }
        if matches!(c,
            Command::New { .. } | Command::Activate { .. } | Command::OpenBookmark { .. } | Command::Navigate { .. }
            | Command::Close { .. } | Command::CloseOthers { .. } | Command::CloseBelow { .. }
            | Command::SwitchSpace { .. } | Command::CreateSpace { .. } | Command::Restore | Command::Move { .. }
            | Command::OpenSettings { .. } | Command::Split { .. } | Command::SwapSplit { .. } | Command::Unsplit
        ) {
            self.close_extension_menu();
        }
        // The page behind a switch may already exist with its own zoom; the
        // UI only learns a factor from an event, so send the shown page's zoom.
        let zoom_after_transition = matches!(
            c,
            Command::Activate { .. }
                | Command::New { .. }
                | Command::Navigate { .. }
                | Command::OpenBookmark { .. }
                | Command::Restore
                | Command::SwitchSpace { .. }
                | Command::Close { .. }
                        | Command::CloseOthers { .. }
                        | Command::CloseBelow { .. }
                | Command::Split { .. }
                | Command::Unsplit
        );
        let focus_after_transition = self.peek.is_some()
            && matches!(
                c,
                Command::New { .. }
                    | Command::Activate { .. }
                    | Command::OpenBookmark { .. }
                    | Command::Close { .. }
                        | Command::CloseOthers { .. }
                        | Command::CloseBelow { .. }
                    | Command::SwitchSpace { .. }
                    | Command::CreateSpace { .. }
                    | Command::Restore
                    | Command::Move { .. }
            );
        match c {
            Command::UpdateCheck => {
                if self.updating { return; }
                self.update_offer = None;
                self.updating = true;
                let channel = crate::updates::Channel::parse(&self.state.settings.update_channel);
                self.publish_update(crate::updates::Report::new(channel, "checking", "正在检查更新…"));
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let _ = proxy.send_event(Event::UpdateChecked(crate::updates::check(channel)));
                });
                return;
            }
            Command::UpdateInstall { offer_id } => {
                if self.updating { return; }
                let channel = crate::updates::Channel::parse(&self.state.settings.update_channel);
                // Consume the exact offer once, before any package is downloaded.
                let Some(offer) = self.update_offer.take_if(|offer| offer.id == offer_id && offer.channel == channel) else {
                    return;
                };
                self.updating = true;
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = offer.prepare(|report| {
                        let _ = proxy.send_event(Event::UpdateProgress(report));
                    });
                    let _ = proxy.send_event(Event::UpdatePrepared(result));
                });
                return;
            }
            Command::SystemSettings => {
                if let Err(e) = crate::default_browser::open_settings() { self.error(e); }
                self.publish_default_browser();
                return;
            }
            Command::DefaultBrowserStatus => {
                self.publish_default_browser();
                return;
            }
            Command::ClearBrowsingData { kind } => {
                if kind == "history" {
                    // Apply the in-memory part now: a close/quit can arrive
                    // before the background DELETE finishes.
                    self.state.closed.clear();
                    self.save();
                    self.publish();
                    let proxy = self.proxy.clone();
                    self.session_writer.clear_history(move |ok| {
                        let _ = proxy.send_event(Event::HistoryCleared(ok));
                    });
                } else if kind == "cache" || kind == "siteData" {
                    if self.data_clear_view.is_some() { self.error("正在清除浏览数据，请稍候"); return; }
                    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                        let view = browsing_webview(&mut self.context).with_url("about:blank").with_visible(false).with_focused(false).build_as_child(self.window.as_ref().ok_or("窗口尚未就绪")?)?;
                        let proxy = self.proxy.clone(); let action = kind.clone();
                        crate::web_settings::clear(&view, &kind, move |ok| { let _ = proxy.send_event(Event::DataCleared(action.clone(), ok)); })?;
                        self.data_clear_view = Some(view);
                        Ok(())
                    })();
                    if let Err(e) = result { self.error(e); self.emit(json!({"type":"dataCleared","message":"清除失败，请重试"})); }
                } else { self.error("未知浏览数据类型"); }
                return;
            }
            Command::TabPreview { id, rect, ticket } => {
                let valid = id.as_ref().zip(rect).filter(|(id, r)| {
                    r.valid()
                        && r.width >= 1.
                        && r.width <= 320.
                        && r.height >= 1.
                        && r.height <= 400.
                        && self.preview_tabs(id).is_some()
                });
                if let Some((id, rect)) = valid {
                    let id = id.clone();
                    self.tab_preview = Some((id.clone(), rect));
                    self.emit(json!({"type":"tabPreviewPresented","ticket":ticket}));
                    if let Err(e) = self.ensure_preview_renders(&id) { self.error(e); self.dismiss_tab_preview(); }
                } else {
                    self.tab_preview = None;
                    self.preview_renders.clear();
                    if let Err(e) = self.sync_layout() { self.error(e); }
                    self.emit(json!({"type":"tabPreviewHidden","ticket":ticket}));
                }
                return;
            }
            Command::TabPreviewRefresh { id } => {
                if self
                    .tab_preview
                    .as_ref()
                    .is_some_and(|(current, _)| current == &id)
                {
                    if let Some(ids) = self.preview_tabs(&id) {
                        for id in ids {
                            if let Some(page) =
                                self.pages.get_mut(&id).filter(|p| !p.thumbnail_pending)
                            {
                                page.thumbnail_version += 1;
                                page.thumbnail_attempt = None;
                            }
                        }
                        self.pump_thumbnails();
                        self.refresh_preview_renders();
                    }
                }
                return;
            }
            Command::Thumbnails => {
                self.emit(json!({"type":"thumbnails","images":self.thumbnails.images()}));
                self.refresh_visible_thumbnails();
                return;
            }
            Command::WindowChrome {
                maximize,
                drag_regions,
                viewport_width,
                anchor_left,
                background,
                hover,
                foreground,
            } => {
                if let (Some(caption), Some(window)) = (&self.caption, &self.window) {
                    caption.set_drag_regions(&drag_regions);
                    if let Err(e) = caption.update(
                        maximize,
                        viewport_width,
                        anchor_left,
                        background,
                        hover,
                        foreground,
                        window.scale_factor(),
                    ) {
                        self.error(e);
                    }
                }
                return;
            }
            Command::Glass { enabled } => {
                self.glass = enabled;
                if let Err(e) = self.apply_glass() {
                    self.error(e);
                }
                return;
            }
            Command::PeekOpen { url, origin } => {
                self.dismiss_tab_preview();
                if let Err(e) = self.open_peek(&url, origin) {
                    self.error(e)
                }
                return;
            }
            Command::PeekClose => {
                self.request_peek_close();
                return;
            }
            Command::PeekMotionStart { id, ticket } => {
                if let Some(p) = self.peek.as_mut().filter(|p| {
                    p.tab.id == id
                        && p.ticket == ticket
                        && (p.phase == PeekPhase::Opening || p.surface_sent)
                }) {
                    p.moving = true;
                    if let Err(e) = self.sync() {
                        self.error(e)
                    }
                    if let Some(popup) = &self.popup {
                        let _ = popup.focus();
                    }
                    self.emit(json!({"type":"peekMoving","id":id,"ticket":ticket}));
                }
                return;
            }
            Command::PeekMotionComplete { id, ticket } => {
                if let Some(p) = self
                    .peek
                    .as_mut()
                    .filter(|p| p.tab.id == id && p.ticket == ticket && p.moving)
                {
                    if p.phase == PeekPhase::Closing {
                        self.close_peek()
                    } else if p.phase == PeekPhase::Opening {
                        p.phase = PeekPhase::Open;
                        p.moving = false;
                        self.publish_peek();
                        if let Err(e) = self.sync() {
                            self.error(e)
                        }
                    }
                }
                return;
            }
            Command::PeekPromote => {
                self.promote_peek();
                return;
            }
            Command::PeekBackdropFailed { id, ticket } => {
                if self
                    .peek
                    .as_ref()
                    .is_some_and(|p| p.tab.id == id && p.ticket == ticket)
                {
                    self.close_peek();
                    self.error("无法解码 Peek 背景，请重试");
                }
                return;
            }
            Command::PeekLayout { id, rect } => {
                if rect.valid() {
                    if let Some(p) = self.peek.as_mut().filter(|p| p.tab.id == id) {
                        p.rect = Some(rect)
                    }
                }
                self.capture_opening_surface();
            }
            Command::Navigate { url, id: None } if self.peek.is_some() => {
                match model::normalize_with(&url, &self.state.settings).and_then(|url| {
                    if model::is_web(&url) {
                        Ok(url)
                    } else {
                        Err("Peek 仅支持网页".into())
                    }
                }) {
                    Ok(url) => {
                        let peek = self.peek.as_mut().unwrap();
                        match peek.page.view.load_url(&url) {
                            Ok(()) => {
                                peek.tab.url = url.clone();
                                peek.page.url = url;
                                peek.tab.loading = true;
                                self.publish_peek()
                            }
                            Err(e) => self.error(e),
                        }
                    }
                    Err(e) => self.error(e),
                }
                return;
            }
            Command::Close { id: None } if self.peek.is_some() => {
                self.request_peek_close();
                return;
            }
            Command::Ready => {
                self.ready = true;
                if let Some(message) = self.update_message.take() {
                    self.toast(&message, None);
                }
                self.open_external_urls(event_loop);
                if let Err(e) = self.update_window_frame() {
                    self.error(e);
                }
                self.publish();
                self.publish_downloads();
                self.publish_default_browser();
                self.publish_sleep_state();
                self.publish_reader();
                self.publish_permission_rules();
                if let Err(e) = self.sync() {
                    self.error(e);
                }
                if self.icon_refresh {
                    self.icon_refresh = false;
                    self.refresh_saved_icons();
                }
                return;
            }
            Command::Presented => {
                if self.ready && (!self.state.onboarding.completed || (self.primary.width > 0. && self.primary.height > 0.)) {
                    if let Some(window) = &self.window {
                        window.set_visible(true);
                        if let Some(caption) = &self.caption {
                            if let Err(e) = caption.refresh(window.scale_factor()) {
                                self.error(e);
                            }
                        }
                    }
                }
                return;
            }
            Command::Layout { primary, secondary, sidebar, viewport, live_resize, sidebar_motion } => {
                self.receive_sidebar_resize();
                if self.split_handle.as_ref().is_some_and(|handle| handle.dragging())
                    || self.sidebar_handle.as_ref().is_some_and(|handle| handle.dragging()) { return; }
                if self.sidebar_expected_width.zip(sidebar).is_some_and(|(width, rect)| (rect.x + rect.width - width).abs() > 1.) { return; }
                let Some(window) = &self.window else { return; };
                let size = window.inner_size().to_logical::<f64>(window.scale_factor());
                let current = crate::layout::Viewport { width: size.width, height: size.height };
                // A renderer can finish an older resize after the native window
                // has moved on. It must not shrink the pages back to that frame.
                if viewport.is_some_and(|viewport| !viewport.matches(current)) { return; }
                let finish_sidebar = self.sidebar_motion && !sidebar_motion;
                if primary.valid() && secondary.is_none_or(|r| r.valid()) {
                    self.primary = primary;
                    self.secondary = secondary;
                    self.layout_viewport = Some(current);
                    self.sidebar_rect = sidebar;
                    self.sidebar_expected_width = None;
                    self.live_resize = live_resize;
                    self.sidebar_motion = sidebar_motion;
                } else {
                    return;
                }
                if let Some(exit) = &mut self.split_exit {
                    // Ignore a stale two-pane report sent before state changed.
                    exit.layout_ready = secondary.is_none();
                    self.recover_split_exit();
                    return;
                }
                // Capture the original full-size page BEFORE committing the
                // split slots. Its frozen presentation moves first.
                let pair = self.secondary.filter(|r| r.width > 0. && r.height > 0.)
                    .and_then(|_| self.state.active()?.split_with.as_ref())
                    .map(|other| (self.state.active_tab.clone(), other.clone()));
                if pair != self.presented_split {
                    self.split_entry_pending = pair.is_some();
                    self.presented_split = pair;
                }
                if self.split_entry_pending {
                    self.animate_split_entry();
                    return;
                }
                let result = self.sync_page_bounds().and_then(|changed| {
                    if finish_sidebar {
                        for page in self.pages.values() {
                            if page.resize_surface.dragging() && !page.resize_surface.relocating() {
                                page.split_pane().end_preview(true)?;
                            }
                        }
                        // Reapply final corners after releasing the frozen plane.
                        self.sync_page_bounds()?;
                    }
                    // Showing a newly created controller can change its z-order.
                    if changed { self.sync_layout() } else { Ok(()) }
                });
                if let Err(e) = result {
                    self.error(e);
                }
                return;
            }
            Command::Overlay { visible, command } => {
                if visible {
                    self.close_extension_menu();
                    self.finish_split_resize();
                    self.dismiss_tab_preview();
                }
                if visible && !self.overlay && !command { self.refresh_visible_thumbnails(); }
                self.overlay = visible;
                if !visible { self.tab_drag_cutouts = [None; 2]; }
                if !visible && self.split_entry_pending {
                    self.animate_split_entry();
                } else if let Err(e) = self.sync_layout() { self.error(e); }
                // PopupLayer requests focus once its input region is installed.
                // Tab dragging also uses Overlay and keeps shell capture.
                if !visible {
                    if let Some(page) = self.page() {
                        let _ = page.focus();
                    }
                }
                return;
            }
            Command::TabDropTarget { rect, preview } => {
                let cutouts = [rect, preview];
                if cutouts.into_iter().flatten().any(|r| {
                    // The row can partly leave the client area while the pointer is captured.
                    let bounds = Rect { x: r.x.abs(), y: r.y.abs(), ..r };
                    !self.overlay || !bounds.valid() || r.width <= 0. || r.height <= 0.
                }) { return; }
                if cutouts.iter().all(Option::is_none) && self.tab_drag_cutouts.iter().all(Option::is_none) { return; }
                self.tab_drag_cutouts = cutouts;
                if let Err(e) = self.sync_page_bounds() { self.error(e); }
                return;
            }
            Command::PopupReady => {
                self.publish_reader();
                self.emit(json!({"type":"state","state":self.state}));
                self.emit(json!({"type":"popupState","popup":self.popup_state}));
                self.emit(json!({"type":"quitConfirmation","visible":self.quit_pending}));
                self.publish_extensions(None);
                self.publish_downloads();
                self.publish_peek();
                self.publish_sleep_state();
                self.sync_permissions();
                self.emit(self.permission_state.clone());
                self.publish_permission_rules();
                return;
            }
            Command::PermissionAnswer { id, allowed, remember } => {
                self.answer_permission(&id, allowed, remember);
                return;
            }
            Command::PermissionRule { scope, allowed } => {
                self.update_permission_rule(scope, allowed);
                return;
            }
            Command::PopupState { popup } => {
                if let Some(patch) = popup.as_object() {
                    let was_open = self.space_form_open();
                    if patch.get("extensionMenuAnchor").is_some() {
                        self.extension_view.take();
                    }
                    for (key, value) in patch { self.popup_state.insert(key.clone(), value.clone()); }
                    self.emit(json!({"type":"popupState","popup":patch}));
                    let form_open = self.space_form_open();
                    if form_open != was_open {
                        self.finish_split_resize();
                        if let Err(e) = self.sync_layout() { self.error(e); }
                        if form_open {
                            if let Some(ui) = &self.ui { let _ = ui.focus(); }
                        }
                    }
                }
                return;
            }
            Command::OpenExtensionView { id, rect } => {
                if !self.popup_state.get("extensionMenuAnchor").is_some_and(|anchor| anchor.is_object()) {
                    return;
                }
                if !self.can_show_extension_view(rect) {
                    self.emit(json!({"type":"extensionView","phase":"error"}));
                } else if let Err(error) = self.open_extension_view(&id, rect) {
                    self.emit(json!({"type":"extensionView","phase":"error","message":error.to_string()}));
                }
                return;
            }
            Command::ExtensionViewBounds { rect } => {
                self.resize_extension_view(rect);
                return;
            }
            Command::CloseExtensionView => {
                self.extension_view.take();
                if let Some(popup) = &self.popup { let _ = popup.focus(); }
                return;
            }
            Command::PopupLayer { regions, cutout, focus, downloads } => {
                if regions.len() > 16 || !regions.iter().all(|r| r.valid()) || cutout.is_some_and(|r| !r.valid()) { return; }
                self.download_popup_visible = downloads;
                if let (Some(popup), Some(window)) = (&self.popup, &self.window) {
                    let result = WindowClip::popup(popup.hwnd(), &regions, cutout, window.scale_factor());
                    if result.is_ok() {
                        self.popup_has_regions = !regions.is_empty();
                        if focus && self.fullscreen_page.is_none() { let _ = popup.focus(); }
                    }
                    if let Err(e) = result { self.error(e); }
                }
                if regions.is_empty() {
                    if self.space_form_open() {
                        if let Some(ui) = &self.ui { let _ = ui.focus(); }
                    }
                    else if let Some(page) = self.page() { let _ = page.focus(); }
                    else if let Some(ui) = &self.ui { let _ = ui.focus(); }
                }
                if !downloads {
                    if let Some(id) = self.pending_download_review.take() {
                        self.review_download(id);
                    }
                }
                return;
            }
            Command::QuitResponse { confirm, always } => {
                if !self.quit_pending { return; }
                self.quit_pending = false;
                if confirm {
                    if always { self.state.settings.ask_before_quit = false; }
                    self.save();
                    if let Some(e) = event_loop { e.exit(); }
                } else {
                    self.emit(json!({"type":"quitConfirmation","visible":false}));
                }
                return;
            }
            Command::Window { action } => {
                if let Some(w) = &self.window {
                    match action.as_str() {
                        "close" => {
                            if !self.confirm_quit() { return; }
                            self.save();
                            if let Some(e) = event_loop {
                                e.exit()
                            }
                        }
                        "minimize" => w.set_minimized(true),
                        "maximize" => w.set_maximized(!w.is_maximized()),
                        _ => {}
                    }
                }
                return;
            }
            Command::SwapSplit { id, other } => {
                if let Err(error) = self.swap_split(&id, &other) { self.error(error); }
                return;
            }
            Command::Suggest { query, ticket } => {
                if !self.state.settings.search_suggestions || self.state.settings.search_engine != "google" { self.emit(json!({"type":"suggestions","ticket":ticket,"suggestions":[]})); return; }
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let _ = proxy.send_event(Event::Suggestions(ticket, crate::suggestions::fetch(&query)));
                });
                return;
            }
            Command::History { query } => {
                match self
                    .storage
                    .search(&query.chars().take(500).collect::<String>())
                {
                    Ok(entries) => self.emit(json!({"type":"history","entries":entries})),
                    Err(e) => self.error(e),
                }
                return;
            }
            Command::Reader { id } => { self.toggle_reader(id.as_deref()); return; }
            Command::ReaderClose => {
                self.dismiss_reader(); return;
            }
            Command::ReaderPreferences { preferences } => { self.apply_reader_preferences(preferences); return; }
            Command::FindOpen => {
                self.open_find();
                return;
            }
            Command::PrintPage => {
                if let Some(page) = self.page_output_target(true).filter(|_| self.page_save.is_none()) {
                    if let Err(error) = crate::page_output::print(&page.view) {
                        self.toast(&error, None);
                    }
                }
                return;
            }
            Command::SavePageAs => {
                if let Some(page) = self.page_output_target(false).filter(|_| self.page_save.is_none()) {
                    let proxy = self.proxy.clone();
                    let choose_proxy = proxy.clone();
                    match crate::page_output::SaveRequest::start(
                        &page.view,
                        page.save_path.clone(),
                        move || choose_proxy.send_event(Event::ChoosePageSavePath).is_ok(),
                        move |result| { let _ = proxy.send_event(Event::PageSaveFinished(result)); },
                    ) {
                        Ok(save) => self.page_save = Some(save),
                        Err(error) => self.toast(&error, None),
                    }
                }
                return;
            }
            Command::FindClose => {
                self.close_find(true);
                self.focus_page_or_ui();
                return;
            }
            Command::FindStart { query } => {
                self.start_find(&query);
                return;
            }
            Command::FindNext { backwards } => {
                let result = self
                    .find
                    .as_ref()
                    .map(|session| session.next(backwards));
                if let Some(Err(e)) = result {
                    self.error(e);
                }
                return;
            }
            Command::Zoom { action } => {
                if self.peek.is_none() && self.reader.as_ref().is_some_and(|r| r.id == self.state.active_tab) {
                    let mut preferences = self.state.reader_preferences.clone();
                    preferences.step(&action);
                    let percent = preferences.size as u32 * 5;
                    self.apply_reader_preferences(preferences);
                    self.toast(&format!("文字 {percent}%"), None);
                    return;
                }
                let (tab, page, in_peek) = if let Some(peek) = self.peek.as_mut() {
                    (&mut peek.tab, &peek.page, true)
                } else {
                    let id = &self.state.active_tab;
                    let Some(page) = self.pages.get(id) else { return };
                    let Some(tab) = self.state.tabs.iter_mut().find(|tab| &tab.id == id) else { return };
                    (tab, page, false)
                };
                if !model::is_web(&tab.url) {
                    return;
                }
                let current = crate::site::zoom_factor(&page.view).unwrap_or(1.0);
                let factor = match action.as_str() {
                    "reset" => 1.0,
                    "out" => model::next_zoom(current, -1),
                    _ => model::next_zoom(current, 1),
                };
                if page.set_zoom(factor).is_err() {
                    return;
                }
                // Zigzag's ZoomLevels: remembered per host; 100% is the absence
                // of an entry. An existing tab override stays local to the tab.
                if tab.zoom.is_some() {
                    tab.zoom = Some(factor);
                    if !in_peek {
                        self.save();
                        self.publish();
                    }
                } else if let Some(site) = model::site_key(&tab.url) {
                    if (factor - 1.0).abs() <= 0.001 {
                        self.state.site_zoom.remove(&site);
                    } else {
                        self.state.site_zoom.insert(site, factor);
                    }
                    self.save();
                }
                self.emit(json!({"type":"zoom","factor":factor}));
                self.emit(json!({"type":"zoomNotice","factor":factor}));
                return;
            }
            Command::Mute { id, muted } => {
                let target = id.or_else(|| Some(self.state.active_tab.clone()));
                let mut changed = None;
                if let Some(target) = target {
                    if let Some(tab) = self.state.tabs.iter_mut().find(|tab| {
                        tab.id == target || tab.bookmark_id.as_deref() == Some(target.as_str())
                    }) {
                        let next = muted.unwrap_or(!tab.muted);
                        if tab.muted != next {
                            tab.muted = next;
                            changed = Some((tab.id.clone(), next));
                        }
                    }
                }
                if let Some((tab_id, next)) = changed {
                    if let Some(page) = self.pages.get(&tab_id) {
                        let _ = crate::site::set_muted(&page.view, next);
                    }
                    self.save();
                    self.publish();
                }
                return;
            }
            Command::ClearStep => {
                // Zigzag's sidebar Clear: old tabs first, then the others, then
                // the tab being looked at and its split partner.
                let plan = self.state.clear_targets(&self.playing, model::now_secs());
                self.close_batch(plan.ids, plan.faded);
                return;
            }
            Command::CloseOthers { id } => {
                let base = id.or_else(|| Some(self.state.active_tab.clone()));
                let targets = self.state.tabs_to_dismiss(false, base.as_deref());
                self.close_batch(targets, false);
                return;
            }
            Command::CloseBelow { id } => {
                let base = id.or_else(|| Some(self.state.active_tab.clone()));
                let targets = self.state.tabs_to_dismiss(true, base.as_deref());
                self.close_batch(targets, false);
                return;
            }
            Command::UndoBatch => {
                self.undo_batch();
                return;
            }
            Command::DownloadAction { id, action } => {
                self.download_action(id, &action);
                return;
            }
            Command::DownloadsPanel { open } => {
                // Only used so the page accelerator knows whether Esc belongs to
                // the panel; the UI owns its visibility.
                self.downloads_open.set(open);
                return;
            }
            Command::CopyText { text } => {
                if text.is_empty() || text.len() > 8192 {
                    self.error("无法复制这个链接");
                    return;
                }
                if let Err(error) = crate::clipboard::write_text(&text) {
                    self.error(format!("复制失败：{error}"));
                }
                return;
            }
            Command::Back { ref id } | Command::Forward { ref id } | Command::Reload { ref id } => {
                if id.as_deref().is_some_and(|id| !self.state.visible_tab(id)) { return; }
                let peek = id.is_none() && self.peek.is_some();
                let on_target = |tab: &str| id.as_deref().is_none_or(|id| id == tab);
                if !peek && (self.reader.as_ref().is_some_and(|r| on_target(&r.id))
                    || self.reader_request.as_ref().is_some_and(|r| on_target(&r.0))) {
                    self.close_reader();
                    if let Err(e) = self.sync() { self.error(e); }
                }
                let page = if let Some(id) = id {
                    self.pages.get(id).map(|page| &page.view)
                } else {
                    self.page()
                };
                if let Some(p) = page {
                    let r = match c {
                        Command::Back { .. } => p.go_back(),
                        Command::Forward { .. } => p.go_forward(),
                        _ => p.reload(),
                    };
                    if let Err(e) = r {
                        self.error(e)
                    }
                }
                return;
            }
            Command::Sidebar | Command::SidebarFocus => {
                // Reversing a shortcut keeps the current compositor plane alive.
                // Ordinary sync() cancels resize surfaces, so only publish state.
                if let Some(handle) = &self.sidebar_handle { handle.cancel(); }
                if let Some(handle) = &self.split_handle { handle.cancel(); }
                self.receive_sidebar_resize();
                self.receive_split_resize();
                self.dismiss_tab_preview();
                if let Err(error) = self.state.apply(&c) { self.error(error); return; }
                self.save();
                self.publish();
                return;
            }
            _ => {
                if self.split_exit.is_some() { self.finish_split_resize(); }
                self.dismiss_tab_preview();
                if matches!(
                    c,
                    Command::Activate { .. }
                        | Command::OpenBookmark { .. }
                        | Command::SwitchSpace { .. }
                        | Command::New { .. }
                ) {
                    self.refresh_visible_thumbnails();
                }
                if matches!(
                    c,
                    Command::New { .. }
                        | Command::Activate { .. }
                        | Command::OpenBookmark { .. }
                        | Command::Close { .. }
                        | Command::CloseOthers { .. }
                        | Command::CloseBelow { .. }
                        | Command::SwitchSpace { .. }
                        | Command::CreateSpace { .. }
                        | Command::Restore
                        | Command::Move { .. }
                ) {
                    self.close_peek();
                }
                if let Err(error) = self.begin_split_exit(&c) {
                    self.finish_split_resize();
                    self.error(error);
                }
                if let Err(e) = self.state.apply(&c) {
                    self.finish_split_resize();
                    self.error(e);
                    return;
                }
                // A tab closed through the generic path stops being "playing".
                self.playing.retain(|id| self.state.tabs.iter().any(|tab| &tab.id == id));
                *self.live_settings.borrow_mut() = self.state.settings.clone();
                if matches!(c, Command::UpdateSettings { .. }) {
                    if self.update_offer.as_ref().is_some_and(|offer| offer.channel.as_str() != self.state.settings.update_channel) {
                        self.update_offer = None;
                    }
                    let mut errors = vec![];
                    for view in self.pages.values().map(|p| &p.view).chain(self.peek.iter().map(|p| &p.page.view)).chain(self.preview_renders.values().map(|p| &p.view)) {
                        if let Err(e) = crate::web_settings::apply(view, &self.state.settings) { errors.push(e); }
                        if !self.state.settings.autofill { let _ = view.evaluate_script("window.dispatchEvent(new Event('__rove_disable_imported_logins'))"); }
                    }
                    for error in errors { self.error(error); }
                    // Zigzag's sleep setting is a switch: off gives every
                    // suspended page back at once.
                    if !self.state.settings.sleep_tabs {
                        self.wake_all_sleeping();
                    }
                }
                self.save();
                self.publish();
            }
        }
        if let Err(e) = self.sync() {
            self.error(e)
        }
        if focus_after_transition && !self.overlay {
            if let Some(p) = self.page() {
                let _ = p.focus();
            } else if let Some(ui) = &self.ui {
                let _ = ui.focus();
            }
        }
        if zoom_after_transition {
            self.emit_active_zoom();
        }
    }
}
impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let window = event_loop.create_window(
                Window::default_attributes()
                    .with_title("Rove")
                    .with_window_icon(Some(app_icon()?))
                    .with_decorations(false)
                    .with_transparent(true)
                    // Let WebView2 alpha reach the desktop/system backdrop.
                    .with_no_redirection_bitmap(true)
                    .with_visible(false)
                    .with_resizable(true)
                    .with_inner_size(LogicalSize::new(1280, 820))
                    .with_min_inner_size(LogicalSize::new(480, 400)),
            )?;
            let proxy = self.proxy.clone();
            let pending_layout = self.pending_layout.clone();
            let dark = self.state.theme == "dark";
            let initialization = format!(
                // Document-created scripts may run before documentElement exists.
                // App.onMounted sets the DOM attribute before the hidden window is shown.
                "window.__ERA_NATIVE__=true;window.__ERA_INITIAL_STATE__={};",
                serde_json::to_string(&self.state)?
            );
            let mut ui_context = WebContext::new(Some(self.ui_data.clone()));
            let ui=WebViewBuilder::new_with_web_context(&mut ui_context)
                .with_custom_protocol("era".into(),|_,req|{
                    let path=req.uri().path().trim_start_matches('/');
                    let file=ASSETS.get_file(if path.is_empty(){"index.html"}else{path});
                    let mime=if path.ends_with(".js"){"text/javascript"}else if path.ends_with(".css"){"text/css"}else if path.ends_with(".woff2"){"font/woff2"}else{"text/html"};
                    wry::http::Response::builder().status(if file.is_some(){200}else{404})
                        .header("Content-Type",mime)
                        .header("Content-Security-Policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' https: data:; connect-src https://github-contributions-api.jogruber.de; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'")
                        .body(Cow::Borrowed(file.map(|f|f.contents()).unwrap_or(b"Not found"))).unwrap()
                }).with_https_scheme(true).with_url("era://ui/index.html")
                .with_additional_browser_args(browser_args(true))
                .with_background_color(surface_color(dark))
                .with_transparent(true)
                .with_theme(if dark { wry::Theme::Dark } else { wry::Theme::Light })
                .with_hotkeys_zoom(false)
                .with_initialization_script(&initialization)
                .with_navigation_handler(|url|url=="https://era.ui/index.html"||url=="era://ui/index.html")
                .with_ipc_handler(move|req|{
                    if req.uri().scheme_str()!=Some("https")||req.uri().host()!=Some("era.ui")||req.uri().path()!="/index.html"||req.body().len()>16384{return}
                    if let Ok(c)=serde_json::from_str::<Command>(req.body()){
                        if let Command::Layout { primary, secondary, sidebar, viewport, live_resize, sidebar_motion } = c {
                            if primary.valid() && secondary.is_none_or(|r| r.valid())
                                && sidebar.is_none_or(|r| r.valid())
                                && viewport.is_none_or(|v| v.valid())
                                && pending_layout.replace(Some(PageLayout { primary, secondary, sidebar, viewport, live_resize, sidebar_motion })).is_none() {
                                let _ = proxy.send_event(Event::Layout);
                            }
                        } else { let _=proxy.send_event(Event::Command(c)); }
                    }
                }).build_as_child(&window)?;
            self.track_webview_focus(&ui)?;
            let proxy = self.proxy.clone();
            let popup=WebViewBuilder::new_with_web_context(&mut ui_context)
                .with_custom_protocol("era".into(),|_,req|{
                    let path=req.uri().path().trim_start_matches('/');
                    let file=ASSETS.get_file(if path.is_empty(){"index.html"}else{path});
                    let mime=if path.ends_with(".js"){"text/javascript"}else if path.ends_with(".css"){"text/css"}else if path.ends_with(".woff2"){"font/woff2"}else{"text/html"};
                    wry::http::Response::builder().status(if file.is_some(){200}else{404})
                        .header("Content-Type",mime)
                        .header("Content-Security-Policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' https: data:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'")
                        .body(Cow::Borrowed(file.map(|f|f.contents()).unwrap_or(b"Not found"))).unwrap()
                }).with_https_scheme(true).with_url("era://ui/index.html?popup")
                .with_additional_browser_args(browser_args(true))
                .with_background_color((0, 0, 0, 0))
                .with_transparent(true)
                .with_theme(if dark { wry::Theme::Dark } else { wry::Theme::Light })
                .with_hotkeys_zoom(false)
                .with_initialization_script(format!("{initialization}window.__ERA_POPUP__=true;"))
                .with_navigation_handler(|url|url=="https://era.ui/index.html?popup"||url=="era://ui/index.html?popup")
                .with_ipc_handler(move|req|{
                    if req.uri().scheme_str()!=Some("https")||req.uri().host()!=Some("era.ui")||req.uri().path()!="/index.html"||req.body().len()>16384{return}
                    if let Ok(c)=serde_json::from_str::<Command>(req.body()){let _=proxy.send_event(Event::Command(c));}
                }).build_as_child(&window)?;
            self.track_webview_focus(&popup)?;
            popup.set_bounds(wry::Rect { position: LogicalPosition::new(0, 0).into(), size: window.inner_size().into() })?;
            WindowClip::popup(popup.hwnd(), &[], None, window.scale_factor())?;
            self.popup = Some(popup);
            ui.set_bounds(wry::Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: window.inner_size().into(),
            })?;
            let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else {
                return Err("无法获取窗口句柄".into());
            };
            let caption = crate::caption::Caption::new(
                HWND(handle.hwnd.get() as *mut _),
                ui.hwnd(),
            )?;
            caption.refresh(window.scale_factor())?;
            self.caption = Some(caption);
            let proxy = self.proxy.clone();
            let pending_layout = self.pending_layout.clone();
            let split_ui = ui.webview();
            self.split_handle = Some(crate::split::SplitHandle::new(
                HWND(handle.hwnd.get() as *mut _),
                self.popup.as_ref().unwrap().hwnd(),
                move || {
                    pending_layout.take();
                    let _ = proxy.send_event(Event::SplitResize);
                },
                move |guide| {
                    let event = match guide {
                        crate::split::Guide::Hover(visible) => json!({ "type": "splitHover", "visible": visible }),
                        crate::split::Guide::Click => json!({ "type": "splitClick" }),
                    };
                    let _ = unsafe { split_ui.PostWebMessageAsJson(&HSTRING::from(event.to_string())) };
                },
            )?);
            self.ui = Some(ui);
            let proxy = self.proxy.clone();
            let pending_layout = self.pending_layout.clone();
            let sidebar_ui = self.ui.as_ref().unwrap().webview();
            self.sidebar_handle = Some(crate::sidebar::SidebarHandle::new(
                HWND(handle.hwnd.get() as *mut _), self.popup.as_ref().unwrap().hwnd(),
                move || { pending_layout.take(); let _ = proxy.send_event(Event::SidebarResize); },
                move |width, resizing, reset| {
                    let event = json!({ "type": "sidebarResize", "width": width, "resizing": resizing, "reset": reset });
                    // Native input -> shell message, without a timer, winit
                    // event hop, ExecuteScript request or returned result.
                    let _ = unsafe { sidebar_ui.PostWebMessageAsJson(&HSTRING::from(event.to_string())) };
                },
            )?);
            self.window = Some(window);
            self.update_window_frame()?;
            Ok(())
        })();
        if let Err(e) = result {
            self.error(e);
            event_loop.exit()
        }
    }
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        match event {
            Event::Agent(request) => {
                use crate::agent_bridge::Response;
                let response = if Instant::now() >= request.deadline {
                    Response::error("timeout")
                } else if let Some(window) = self.window.as_ref().filter(|_| {
                    self.ready && self.state.onboarding.completed && !self.quit_pending && !self.updating
                }) {
                    self.agent_tabs.get_or_insert_with(|| crate::agent_tabs::Tabs::new(self.agent_data.clone()))
                        .run(window, request.command)
                } else {
                    Response::error("unavailable")
                };
                let _ = request.reply.send(response);
            }
            Event::RestoreKeyboardFocus => self.restore_keyboard_focus(),
            Event::ExtensionPrepared(request, result) => {
                if self.extension_preparation != Some(request) { return; }
                self.extension_preparation = None;
                match result {
                    Ok(prepared) => {
                        self.extension_candidate = Some(prepared);
                        self.emit_extensions(None, None);
                    }
                    Err(error) => self.emit_extensions(None, Some(error)),
                }
            }
            Event::ExtensionBackground(popup, rect, image) => {
                if self.extension_popup == Some(popup) {
                    self.emit(json!({
                        "type": "extensionBackground", "image": image,
                        "rect": {"x": rect.x, "y": rect.y, "width": rect.width, "height": rect.height},
                    }));
                }
            }
            Event::ExtensionView(ticket, event) => {
                use crate::extension_view::Event as ViewEvent;
                let Some(extension) = self.extension_view.as_ref().filter(|view| view.ticket == ticket) else { return };
                match event {
                    ViewEvent::Loaded => {
                        if extension.fit_content {
                            self.measure_extension_view();
                        } else {
                            self.update_extension_size(None);
                        }
                    }
                    ViewEvent::Measured(size) => self.update_extension_size(size),
                    ViewEvent::Failed => {
                        self.extension_view.take();
                        self.emit(json!({"type":"extensionView","phase":"error"}));
                    }
                    ViewEvent::Close => self.close_extension_menu(),
                    ViewEvent::Link(url) => self.command(Command::New { url }, Some(event_loop)),
                }
            }
            Event::PermissionsChanged => {}, // Wake the event loop so about_to_wait reconciles once.
            Event::ExternalUrls(urls) => {
                self.external_urls.extend(urls.into_iter().take(32usize.saturating_sub(self.external_urls.len())));
                self.open_external_urls(Some(event_loop));
                if let Some(window) = &self.window { window.set_minimized(false); window.focus_window(); }
            }
            Event::ImportSources(sources) => {
                self.import_sources = sources;
                self.import_busy = false;
                self.emit(json!({"type":"importState","importBusy":false,"importSources":self.import_sources}));
            }
            Event::ImportRead(batches) => {
                // Imports commit their own session in the same transaction.
                // Complete older background snapshots before that transaction.
                self.flush_session();
                for batch in batches {
                    if let Err(e) = self.storage.import_batch(&mut self.state, batch) { self.error(e); }
                }
                self.import_busy = false;
                self.publish();
                self.emit(json!({"type":"importState","importBusy":false}));
            }
            Event::SplitResize => {
                self.receive_split_resize();
            }
            Event::SidebarResize => self.receive_sidebar_resize(),
            Event::SplitExited(ticket) => {
                if let Some(exit) = self.split_exit.as_mut().filter(|exit| exit.ticket == ticket) {
                    exit.departed = true;
                    if let Some(page) = self.pages.get(&exit.departing) {
                        if let Err(error) = page.set_visible(false) { eprintln!("Rove split exit: {error}"); }
                        page.resize_surface.end();
                    }
                    self.recover_split_exit();
                }
            }
            Event::Layout => {
                if let Some(layout) = self.pending_layout.take() {
                    self.command(Command::Layout { primary: layout.primary, secondary: layout.secondary, sidebar: layout.sidebar, viewport: layout.viewport, live_resize: layout.live_resize, sidebar_motion: layout.sidebar_motion }, Some(event_loop));
                }
            }
            Event::PageFullscreen(id, hwnd, entered) => {
                if let Err(error) = self.page_fullscreen_changed(id, hwnd, entered) {
                    let _ = self.restore_page_window(true).and_then(|_| self.sync_layout());
                    self.error(error);
                }
            }
            Event::DataCleared(kind, ok) => {
                self.data_clear_view = None;
                self.emit(json!({"type":"dataCleared","action":kind,"message":if ok { "已清除浏览数据" } else { "清除失败，请重试" }}));
            }
            Event::UpdateChecked(result) => {
                self.updating = false;
                let report = match result {
                    Ok(offer) => {
                        let report = offer.report.clone();
                        self.update_offer = Some(offer);
                        report
                    }
                    Err(report) => report,
                };
                self.publish_update(report);
            }
            Event::UpdateProgress(report) => self.publish_update(report),
            Event::UpdatePrepared(result) => {
                let report = match result {
                    Ok((update, mut report)) => {
                        let started = if self.flush_session() {
                            update.start()
                        } else {
                            Err("无法保存浏览会话，已取消更新。".into())
                        };
                        match started {
                            Ok(()) => {
                                self.publish_update(report);
                                event_loop.exit();
                                return;
                            }
                            Err(message) => {
                                report.phase = "error";
                                report.message = message;
                                report
                            }
                        }
                    }
                    Err(report) => report,
                };
                self.updating = false;
                self.publish_update(report);
            }
            Event::HistoryCleared(ok) => {
                if ok {
                    self.emit(json!({"type":"dataCleared","action":"history","message":"已清除浏览历史"}));
                }
            }
            Event::PreviewLoad(key, epoch, url, loading) => {
                if let Some(render) = self.preview_renders.get_mut(&key).filter(|p| p.epoch == epoch) {
                    if url.is_empty() {
                        if render.loading { self.emit(json!({"type":"tabPreviewUnavailable","id":key})); }
                    } else if model::is_web(&url) {
                        render.loading = loading;
                        if !loading {
                            let proxy = self.proxy.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(140));
                                let _ = proxy.send_event(Event::PreviewCapture);
                            });
                        }
                    }
                }
            }
            Event::PreviewCapture => self.refresh_preview_renders(),
            Event::PreviewThumbnail(key, epoch, version, image) => {
                if let Some(render) = self.preview_renders.get_mut(&key).filter(|p| p.epoch == epoch && p.version == version && p.pending) {
                    render.pending = false;
                    if let Some(image) = image {
                        self.thumbnails.insert(key.clone(), render.source.clone(), image.clone());
                        self.emit(json!({"type":"thumbnail","id":key,"image":image,"kept":self.thumbnails.images().keys().collect::<Vec<_>>()}));
                    } else { self.emit(json!({"type":"tabPreviewUnavailable","id":key})); }
                }
            }
            Event::CaptureTabs => self.pump_thumbnails(),
            Event::Thumbnail(id, url, version, image) => {
                let valid = self
                    .pages
                    .get(&id)
                    .is_some_and(|p| p.url == url && p.thumbnail_version == version);
                if valid {
                    if let Some(page) = self.pages.get_mut(&id) {
                        page.thumbnail_pending = false;
                    }
                    if let Some(image) = image {
                        self.thumbnails.insert(id.clone(), url, image.clone());
                        self.emit(json!({"type":"thumbnail","id":id,"image":image,"kept":self.thumbnails.images().keys().collect::<Vec<_>>()}));
                    }
                }
                self.pump_thumbnails();
            }
            Event::Suggestions(ticket, items) => {
                self.emit(json!({"type":"suggestions","ticket":ticket,"suggestions":items}));
            }
            Event::ReaderAvailable(id, url, available) => {
                if self.state.tabs.iter().any(|t| t.id == id && t.url == url && !t.loading) {
                    self.reader_available.insert(id, available); self.publish_reader();
                }
            }
            Event::ReaderReady(ticket) => {
                if self.reader.as_ref().is_some_and(|r| r.ticket == ticket && !r.ready) {
                    self.animate_reader(true);
                }
            }
            Event::ReaderMotionDone(ticket, opening) => {
                if self.reader.as_ref().is_some_and(|r| r.ticket == ticket && r.transitioning && r.closing != opening) {
                    if opening {
                        if let Some(reader) = self.reader.as_mut() { reader.transitioning = false; }
                        if let Err(e) = self.sync_layout() { self.error(e); }
                    } else {
                        self.close_reader();
                        if let Err(e) = self.sync() { self.error(e); }
                        self.focus_page_or_ui();
                    }
                }
            }
            Event::ReaderMotionFailed(ticket) => {
                if self.reader.as_ref().is_some_and(|r| r.ticket == ticket && r.transitioning) {
                    self.close_reader();
                    if let Err(e) = self.sync() { self.error(e); }
                    self.focus_page_or_ui();
                }
            }
            Event::ReaderResources(ticket, attempt) => {
                if let Some(reader) = self.reader.as_ref().filter(|r| r.ticket == ticket && !r.closing) {
                    if let Some(page) = self.pages.get(&reader.id) {
                        let proxy = self.proxy.clone();
                        let _ = page.view.evaluate_script_with_callback(&crate::reader::resources_script(attempt == 0), move |value| {
                            if attempt < 95 && serde_json::from_str::<u64>(&value).unwrap_or(0) > 0 {
                                let proxy = proxy.clone();
                                std::thread::spawn(move || {
                                    std::thread::sleep(Duration::from_millis(100));
                                    let _ = proxy.send_event(Event::ReaderResources(ticket, attempt + 1));
                                });
                            }
                        });
                    }
                }
            }
            Event::ReaderArticle(id, url, epoch, article) => {
                if self.reader_request.as_ref() != Some(&(id.clone(), url.clone(), epoch)) { return; }
                self.reader_request = None;
                if !self.state.visible_tab(&id) || !self.state.tabs.iter().any(|t| t.id == id && t.url == url) {
                    self.publish_reader(); return;
                }
                match article {
                    Some(article) => if let Err(e) = self.show_reader(id, url, article) { self.error(e); },
                    None => self.toast("此页面没有可提取的正文", None),
                }
                self.publish_reader();
            }
            Event::ReaderDeadline(epoch) => {
                if let Some((id, _, current)) = self.reader_request.clone() {
                    if current == epoch && self.state.visible_tab(&id) {
                        if let Some(page) = self.pages.get(&id) {
                            let url = page.url.clone();
                            self.reader_request = Some((id.clone(), url.clone(), epoch));
                            self.start_reader_extract(&id, &url, epoch);
                        }
                    }
                }
            }
            Event::ReaderLink(id, url) => {
                if self.reader.as_ref().is_some_and(|r| r.id == id) {
                    self.close_reader();
                    self.command(Command::Navigate { url, id: Some(id) }, Some(event_loop));
                }
            }
            Event::FindMatches(count, active) => {
                if self.find_open.get() {
                    self.emit(json!({"type":"findMatches","count":count,"active":active}));
                }
            }
            Event::ChoosePageSavePath => {
                let Some(save) = &self.page_save else { return; };
                let Some(RawWindowHandle::Win32(handle)) = self.window.as_ref()
                    .and_then(|window| window.window_handle().ok()).map(|handle| handle.as_raw()) else {
                    self.page_save.take();
                    return;
                };
                if let Err(error) = save.choose_path(HWND(handle.hwnd.get() as *mut _)) {
                    self.toast(&format!("无法另存网页：{error}"), None);
                }
            }
            Event::PageSaveFinished(result) => {
                self.page_save.take();
                match result {
                    Ok(true) => self.toast("网页已保存", None),
                    Ok(false) => {}
                    Err(error) => self.toast(&error, None),
                }
            }
            Event::Zoom(id, hwnd, factor) => self.page_zoom_changed(&id, hwnd, factor),
            Event::Audio(id, playing) => {
                if playing {
                    self.playing.insert(id.clone());
                } else {
                    self.playing.remove(&id);
                }
                // A page that starts playing sound must not stay suspended.
                if playing {
                    if let Some(page) = self.pages.get(&id).filter(|page| page.sleeping.get()) {
                        page.wake();
                        self.emit(json!({"type":"sleep","id":id,"asleep":false}));
                    }
                }
                self.emit(json!({"type":"audio","id":id,"playing":playing}));
            }
            Event::SleepTick => self.sleep_pass(),
            Event::StorageWriteError(error) => self.error(format!("保存浏览数据失败：{error}")),
            Event::SleepDone(id, slept) => self.sleep_done(&id, slept),
            Event::DownloadsChanged { persist } => {
                if persist {
                    self.save_downloads();
                }
                self.publish_downloads();
            }
            Event::DownloadStarted => {
                // A small/cached download may already be complete by the time
                // its handlers are registered; events do not replay old states.
                self.refresh_downloads();
                self.save_downloads();
            }
            Event::Command(c) => self.command(c, Some(event_loop)),
            Event::PeekRequest(source, url, point) => {
                let eligible = self
                    .state
                    .active()
                    .is_some_and(|t| t.id == source || t.split_with.as_ref() == Some(&source))
                    || self.peek.as_ref().is_some_and(|p| p.tab.id == source);
                if eligible {
                    let origin = self.peek_origin(&source, point);
                    if let Err(e) = self.open_peek(&url, origin) {
                        self.error(e)
                    }
                }
            }
            Event::PeekSurface(id, ticket, image) => {
                if let Some(p) = self
                    .peek
                    .as_mut()
                    .filter(|p| p.tab.id == id && p.ticket == ticket)
                {
                    if image.is_some() {
                        p.surface = image.clone();
                    }
                    if p.phase == PeekPhase::Closing {
                        if p.surface_sent {
                            return;
                        }
                        p.surface_sent = true;
                        let image = p.surface.clone();
                        self.emit(
                            json!({"type":"peekSurface","id":id,"ticket":ticket,"surface":image}),
                        );
                    } else if p.phase == PeekPhase::Opening && image.is_some() {
                        self.emit(
                            json!({"type":"peekSurface","id":id,"ticket":ticket,"surface":image}),
                        );
                    }
                }
            }
            Event::Shortcut(key, shift) => self.shortcut(key, shift),
            Event::Open(url) => self.command(Command::New { url }, Some(event_loop)),
            Event::Title(id, title) => {
                let failed = self.pages.get(&id).is_some_and(|p| p.error_page.is_failed());
                if let Some(p) = self.peek.as_mut().filter(|p| p.tab.id == id) {
                    p.tab.title = title.chars().take(512).collect();
                    self.publish_peek();
                    return;
                }
                if let Some(t) = self.state.tabs.iter_mut().find(|t| t.id == id) {
                    t.title = title.chars().take(512).collect();
                    if !failed && self.state.settings.remember_history && model::is_web(&t.url) {
                        self.session_writer.visit(&t.title, &t.url);
                    }
                }
                self.publish();
                self.save();
            }
            Event::Load(id, url, loading) => {
                if loading {
                    if let Some((pending_id, pending_url, _)) = self.reader_request.as_mut() {
                        if pending_id == &id { *pending_url = url.clone(); }
                    }
                    if self.reader.as_ref().is_some_and(|r| r.id == id) { self.close_reader(); }
                    self.reader_available.remove(&id);
                    self.publish_reader();
                }
                let failed = self.pages.get(&id).is_some_and(|p| p.error_page.is_failed());
                // A different document in the page the bar searched ends the
                // session. The same page's own first load does not: the bar may
                // open while that document is still arriving.
                if loading
                    && self.find.as_ref().is_some_and(|session| {
                        session.target_id == id && session.target_url != url
                    })
                {
                    self.close_find(true);
                }
                let desired_zoom = self.site_zoom_for(&url);
                if let Some(p) = self.peek.as_mut().filter(|p| p.tab.id == id) {
                    p.tab.loading = loading;
                    if model::is_web(&url) {
                        // A page on another site must not keep the last site's icon.
                        if !crate::favicon::same_origin(&p.tab.url, &url) {
                            p.tab.favicon = None;
                        }
                        p.tab.url = url.clone();
                        p.page.url = url;
                        if loading {
                            p.page.restore_zoom(p.tab.zoom.unwrap_or(desired_zoom));
                        }
                    }
                    self.publish_peek();
                    if loading { self.emit_active_zoom(); }
                    self.capture_opening_surface();
                    return;
                }
                if loading {
                    self.invalidate_thumbnail(&id);
                }
                if let Some(t) = self.state.tabs.iter_mut().find(|t| t.id == id) {
                    t.loading = loading;
                    if model::is_web(&url) {
                        // A page on another site must not keep the last site's icon.
                        if !crate::favicon::same_origin(&t.url, &url) {
                            t.favicon = None;
                        }
                        t.url = url.clone();
                        if let Some(p) = self.pages.get_mut(&id) {
                            p.url = url.clone();
                            if loading {
                                p.restore_zoom(t.zoom.unwrap_or(desired_zoom));
                            }
                        }
                    }
                    if !loading && !failed && self.state.settings.remember_history {
                        self.session_writer.visit(&t.title, &t.url);
                    }
                }
                self.publish();
                if loading && self.peek.is_none() && self.state.active_tab == id {
                    self.emit_active_zoom();
                }
                if !loading {
                    if !failed && self.state.settings.autofill {
                        if let Some(page) = self.pages.get(&id) {
                            if let Some(origin) = crate::import::origin(&page.url) {
                                let logins = self.storage.imported_logins(&origin);
                                if !logins.is_empty() { let _ = page.view.evaluate_script(&crate::passwords::fill_script(&origin, &logins, &self.state.settings.language)); }
                            }
                        }
                    }
                    // The page may have started or stopped sound while loading,
                    // before the audio change handler was registered for it.
                    if let Some(page) = self.pages.get(&id) {
                        if let Ok(playing) = crate::site::is_playing_audio(&page.view) {
                            self.emit(json!({"type":"audio","id":id,"playing":playing}));
                        }
                    }
                    self.probe_reader(&id, &url);
                    if let Some((pending_id, pending_url, epoch)) = self.reader_request.clone() {
                        if pending_id == id && pending_url == url { self.start_reader_extract(&id, &url, epoch); }
                    }
                    self.save();
                    let proxy = self.proxy.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(140));
                        let _ = proxy.send_event(Event::CaptureTabs);
                    });
                }
            }
            Event::Source(id, url) => {
                if self.reader.as_ref().is_some_and(|r| r.id == id && r.url != url) { self.close_reader(); }
                let source_changed = self.pages.get(&id).is_some_and(|p| p.url != url);
                if source_changed && self.extension_menu_tab() == Some(id.as_str()) {
                    self.close_extension_menu();
                }
                if source_changed { self.reader_available.remove(&id); self.publish_reader(); }
                if let Some(p) = self.peek.as_mut().filter(|p| p.tab.id == id) {
                    if model::is_web(&url) {
                        if !crate::favicon::same_origin(&p.tab.url, &url) {
                            p.tab.favicon = None;
                        }
                        p.tab.url = url.clone();
                        p.page.url = url;
                        self.publish_peek();
                    }
                    return;
                }
                if model::is_web(&url) {
                    if self.pages.get(&id).is_some_and(|p| p.url != url) {
                        self.invalidate_thumbnail(&id);
                    }
                    if let Some(t) = self.state.tabs.iter_mut().find(|t| t.id == id) {
                        if !crate::favicon::same_origin(&t.url, &url) {
                            t.favicon = None;
                        }
                        t.url = url.clone();
                    }
                    if let Some(p) = self.pages.get_mut(&id) {
                        p.url = url.clone();
                    }
                    if source_changed { self.probe_reader(&id, &url); }
                    self.publish();
                }
            }
            Event::Favicon(id, page_url, icon) => {
                if let Some(p) = self.peek.as_mut().filter(|p| p.tab.id == id) {
                    if p.tab.url == page_url {
                        p.tab.favicon = Some(icon);
                        self.publish_peek();
                    }
                    return;
                }
                if self.state.cache_favicon(&id, &page_url, icon) { self.save(); }
                self.publish();
            }
            Event::FaviconLinks(id, page_url, links) => self.start_site_icon(id, page_url, links),
            Event::SiteIcon(id, page_url, icon) => {
                let Some(origin) = crate::favicon::origin_of(&page_url) else { return };
                let Some(icon) = self.site_icons.finish(&origin, icon) else { return };
                if let Some(p) = self
                    .peek
                    .as_mut()
                    .filter(|p| p.tab.id == id && crate::favicon::same_origin(&p.tab.url, &page_url))
                {
                    p.tab.favicon = Some(icon);
                    self.publish_peek();
                    return;
                }
                if self.state.cache_page_icon(&id, &page_url, icon) {
                    self.save();
                    self.publish();
                }
            }
            Event::IconsRefreshed(items) => {
                if items.is_empty() {
                    // The one-time refresh finished: remembered only when it found
                    // real icons, so an offline first launch tries again next time.
                    if self.icon_refresh_found {
                        self.icon_refresh_found = false;
                        self.state.mark_icons_upgraded();
                        self.save();
                    }
                    return;
                }
                let mut fetched = Vec::new();
                for (origin, icon) in items {
                    if let Some(icon) = self.site_icons.finish(&origin, icon) {
                        fetched.push((origin, icon));
                    }
                }
                // One publish for the whole batch: the state carries every icon.
                let mut changed = false;
                for (origin, icon) in fetched {
                    changed |= self.state.cache_site_icon(&origin, icon);
                }
                self.icon_refresh_found |= changed;
                if changed {
                    self.save();
                    self.publish();
                }
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Focused(true) => {
                // Let Windows finish activating the window and assigning focus
                // before deciding whether a WebView needs to receive it.
                let _ = self.proxy.send_event(Event::RestoreKeyboardFocus);
                self.publish_default_browser();
            }
            WindowEvent::Focused(false) => {
                self.close_extension_menu();
                self.emit(json!({"type":"windowDeactivated"}));
                self.dismiss_tab_preview();
            }
            WindowEvent::CloseRequested => {
                if !self.confirm_quit() { return; }
                self.save();
                event_loop.exit()
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.finish_split_resize();
                self.sidebar_expected_width = None;
                self.dismiss_tab_preview();
                if let Some(w) = &self.window {
                    let size = w.inner_size().to_logical::<f64>(w.scale_factor());
                    let current = crate::layout::Viewport { width: size.width, height: size.height };
                    if current.valid() {
                        if let Some(previous) = self.layout_viewport {
                            (self.primary, self.secondary) = crate::layout::resized(
                                self.primary, self.secondary, previous, current,
                                self.state.sidebar_width, self.state.sidebar_collapsed,
                            );
                        }
                        self.layout_viewport = Some(current);
                        // The pages reflow now, even if the shell's JS is busy.
                        // Its next layout restores the rounded card decoration.
                        self.live_resize = true;
                        if let Err(e) = self.sync_page_bounds() { self.error(e); }
                    }
                }
                if let Err(e) = self.update_window_frame() {
                    self.error(e);
                }
                if let (Some(ui), Some(w)) = (&self.ui, &self.window) {
                    if let Some(popup) = &self.popup { let _ = popup.set_bounds(wry::Rect { position: LogicalPosition::new(0, 0).into(), size: w.inner_size().into() }); }
                    let _ = ui.set_bounds(wry::Rect {
                        position: LogicalPosition::new(0, 0).into(),
                        size: w.inner_size().into(),
                    });
                    if let Some(caption) = &self.caption {
                        if let Err(e) = caption.refresh(w.scale_factor()) {
                            self.error(e);
                        }
                    }
                }
                if let Err(e) = self.sync_layout() {
                    self.error(e)
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.sync_permissions();
        // Publish the latest state once after draining this turn's events.
        if self.pending_publish.replace(false) {
            let message = self.state_publisher.message(&self.state);
            self.emit(message);
        }
        if self.next_session_save.is_some_and(|due| Instant::now() >= due) {
            self.next_session_save = None;
            self.session_writer.save(self.state.clone());
        }
        if !self.download_ops.borrow().is_empty() {
            let now = Instant::now();
            if self.next_download_poll.is_none_or(|due| now >= due) {
                // StateChanged is the fast path; reconcile the actual engine
                // state as well so a missed notification cannot strand a row.
                self.refresh_downloads();
                self.next_download_poll = Some(Instant::now() + Duration::from_secs(1));
            }
        }
        if self.download_ops.borrow().is_empty() {
            self.next_download_poll = None;
        }
        if self.extension_view.as_ref().and_then(|view| view.next_measure).is_some_and(|due| Instant::now() >= due) {
            self.measure_extension_view();
        }
        let due = self.next_download_poll.into_iter().chain(self.next_session_save)
            .chain(self.extension_view.as_ref().and_then(|view| view.next_measure)).min();
        event_loop.set_control_flow(due.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.agent_tabs.take();
        self.permissions.inbox.take();
        self.permissions.waiting.clear();
        self.page_save.take();
        self.glass_backdrop = None;
        self.flush_session();
        self.save_downloads();
    }
}
fn register_page_events(
    view: &WebView,
    id: String,
    proxy: EventLoopProxy<Event>,
    shared: PageShared,
    navigation: Rc<NavigationPolicy>,
    settings: Rc<RefCell<crate::settings::Settings>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let fullscreen = Rc::new(Cell::new(false));
    register_accelerators(view, proxy.clone(), shared.find_open.clone(), shared.peek_visible.clone(), shared.downloads_open.clone(), settings.clone(), Some(fullscreen.clone()))?;
    let mut token = 0;
    let fullscreen_id = id.clone();
    let fullscreen_hwnd = view.hwnd().0 as isize;
    let fullscreen_proxy = proxy.clone();
    let fullscreen_changed = ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _| {
        if let Some(sender) = sender {
            let mut entered = BOOL::default();
            unsafe { sender.ContainsFullScreenElement(&mut entered)?; }
            fullscreen.set(entered.as_bool());
            let _ = fullscreen_proxy.send_event(Event::PageFullscreen(fullscreen_id.clone(), fullscreen_hwnd, entered.as_bool()));
        }
        Ok(())
    }));
    unsafe { view.webview().add_ContainsFullScreenElementChanged(&fullscreen_changed, &mut token)?; }
    let download_downloads = shared.downloads.clone();
    let download_ops = shared.download_ops.clone();
    let download_sources = shared.download_sources.clone();
    let download_source = id.clone();
    let save_path = shared.save_path.clone();
    let download_proxy = proxy.clone();
    let download = DownloadStartingEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            unsafe {
                // Wry sets Handled first. Allow WebView2 to show its own
                // download dialog, including file safety decisions.
                args.SetHandled(false)?;
                let mut suggested = PWSTR::null();
                args.ResultFilePath(&mut suggested)?;
                let mut path = PathBuf::from(take_pwstr(suggested));
                // WebView2's PDF viewer saves through DownloadStarting. Its
                // explicit Save As path takes precedence over the default folder.
                let explicit = save_path.take_matching(&path);
                let folder = settings.borrow().download_folder.clone();
                if !explicit && !folder.is_empty() {
                    let filename = path.file_name().unwrap_or(std::ffi::OsStr::new("download"));
                    let mut destination = PathBuf::from(&folder).join(filename);
                    if !std::path::Path::new(&folder).is_dir() { args.SetCancel(true)?; return Ok(()); }
                    let stem = path.file_stem().unwrap_or(filename).to_string_lossy();
                    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                    let mut index = 1;
                    while destination.exists() { destination = PathBuf::from(&folder).join(format!("{stem} ({index}){ext}")); index += 1; }
                    args.SetResultFilePath(&HSTRING::from(destination.as_os_str()))?;
                    path = destination;
                }
                // Keep the download history in sync with the native operation.
                let operation = args.DownloadOperation()?;
                let mut uri = PWSTR::null();
                operation.Uri(&mut uri)?;
                let url = take_pwstr(uri);
                let mut total = 0i64;
                let _ = operation.TotalBytesToReceive(&mut total);
                let mut received = 0i64;
                let _ = operation.BytesReceived(&mut received);
                let id = uuid::Uuid::new_v4().to_string();
                let filename = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "download".into());
                download_downloads.borrow_mut().insert(DownloadItem {
                    id: id.clone(),
                    url,
                    path: path.to_string_lossy().into_owned(),
                    filename,
                    received: received.max(0) as u64,
                    total: total.max(0) as u64,
                    state: DownloadState::InProgress,
                    failure: None,
                    started: model::now_secs(),
                    finished: None,
                    exists: false,
                });
                download_ops.borrow_mut().insert(id.clone(), operation.clone());
                download_sources.borrow_mut().insert(id.clone(), download_source.clone());
                track_download(
                    &operation,
                    id,
                    download_downloads.clone(),
                    download_ops.clone(),
                    download_proxy.clone(),
                )?;
                let _ = download_proxy.send_event(Event::DownloadStarted);
            }
        }
        Ok(())
    }));
    unsafe { view.webview().cast::<ICoreWebView2_4>()?.add_DownloadStarting(&download, &mut token)?; }
    let p = proxy.clone();
    let source_id = id.clone();
    let popup = NewWindowRequestedEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            unsafe {
                args.SetHandled(true)?;
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                let url = take_pwstr(uri);
                if !model::is_web(&url) {
                    return Ok(());
                }
                let mut name = PWSTR::null();
                let target = if args
                    .cast::<ICoreWebView2NewWindowRequestedEventArgs2>()
                    .is_ok_and(|a| a.Name(&mut name).is_ok())
                {
                    take_pwstr(name)
                } else {
                    String::new()
                };
                if target == "__rove_chrome_store_install__" {
                    let mut user = BOOL::default();
                    args.IsUserInitiated(&mut user)?;
                    if user.as_bool()
                        && url::Url::parse(&url).ok().as_ref().and_then(chrome_store::detail_id).is_some()
                    {
                        // Bind the click to the actual requester's listing. The event loop also
                        // checks the live top-level URL before opening the confirmation page.
                        let frame = args.cast::<ICoreWebView2NewWindowRequestedEventArgs3>()?
                            .OriginalSourceFrameInfo()?;
                        let mut source = PWSTR::null();
                        frame.Source(&mut source)?;
                        if take_pwstr(source) == url {
                            let _ = p.send_event(Event::Command(Command::PrepareChromeStoreTab {
                                id: source_id.clone(),
                                expected_url: Some(url),
                            }));
                        }
                    }
                } else if let Some(point) = peek_target(&target) {
                    let mut user = BOOL::default();
                    args.IsUserInitiated(&mut user)?;
                    if user.as_bool() {
                        let _ = p.send_event(Event::PeekRequest(source_id.clone(), url, point));
                    }
                } else {
                    let _ = p.send_event(Event::Open(url));
                }
            }
        }
        Ok(())
    }));
    unsafe {
        view.webview().add_NewWindowRequested(&popup, &mut token)?;
    }
    let id2 = id.clone();
    let p = proxy.clone();
    let nav = navigation.clone();
    let source = SourceChangedEventHandler::create(Box::new(move |sender, _| {
        if let Some(sender) = sender {
            let mut value = PWSTR::null();
            unsafe {
                sender.Source(&mut value)?;
            }
            let url = take_pwstr(value);
            *nav.current.borrow_mut() = url.clone();
            let _ = p.send_event(Event::Source(id2.clone(), url));
        }
        Ok(())
    }));
    unsafe {
        view.webview().add_SourceChanged(&source, &mut token)?;
    }
    let p = proxy.clone();
    let starting = NavigationStartingEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            unsafe {
                if navigation.host_navigation.replace(false) || !navigation.bookmarked.get() {
                    return Ok(());
                }
                let mut kind = COREWEBVIEW2_NAVIGATION_KIND_NEW_DOCUMENT;
                if args
                    .cast::<ICoreWebView2NavigationStartingEventArgs3>()
                    .is_ok_and(|a| a.NavigationKind(&mut kind).is_ok())
                    && kind != COREWEBVIEW2_NAVIGATION_KIND_NEW_DOCUMENT
                {
                    return Ok(());
                }
                let mut user = BOOL::default();
                let mut redirected = BOOL::default();
                args.IsUserInitiated(&mut user)?;
                args.IsRedirected(&mut redirected)?;
                let mut value = PWSTR::null();
                args.Uri(&mut value)?;
                let url = take_pwstr(value);
                if user.as_bool()
                    && !redirected.as_bool()
                    && crate::bookmarks::leaves_site(&navigation.current.borrow(), &url)
                {
                    args.SetCancel(true)?;
                    let _ = p.send_event(Event::Open(url));
                }
            }
        }
        Ok(())
    }));
    unsafe {
        view.webview()
            .add_NavigationStarting(&starting, &mut token)?;
    }
    let zoom_id = id.clone();
    let zoom_hwnd = view.hwnd().0 as isize;
    let audio_id = id.clone();
    let zoom_proxy = proxy.clone();
    let audio_proxy = proxy.clone();
    if let Ok(core) = view.webview().cast::<ICoreWebView2_15>() {
        let icon = FaviconChangedEventHandler::create(Box::new(move |sender, _| {
            if let Some(sender) = sender {
                let core = sender.cast::<ICoreWebView2_15>()?;
                let mut value = PWSTR::null();
                unsafe {
                    core.Source(&mut value)?;
                }
                let page_url = take_pwstr(value);
                let capture_proxy = proxy.clone();
                let capture_id = id.clone();
                let capture_url = page_url.clone();
                crate::favicon::capture(&core, move |icon| {
                    let _ = capture_proxy.send_event(Event::Favicon(capture_id, capture_url, icon));
                })?;
                // WebView2's favicon store only holds 16 and 32 pixel bitmaps; the
                // page's declared SVG or 64px+ icon is what stays sharp when cached.
                if let Ok(web) = core.cast::<ICoreWebView2>() {
                    let link_proxy = proxy.clone();
                    let link_id = id.clone();
                    let link_url = page_url;
                    let _ = crate::favicon::discover_links(&web, move |links| {
                        if !links.is_empty() {
                            let _ =
                                link_proxy.send_event(Event::FaviconLinks(link_id, link_url, links));
                        }
                    });
                }
            }
            Ok(())
        }));
        unsafe {
            core.add_FaviconChanged(&icon, &mut token)?;
        }
    }
    // WebView2 delivers Ctrl+wheel to its own controller, even inside an iframe.
    let zoom = ZoomFactorChangedEventHandler::create(Box::new(move |sender, _| {
        if let Some(sender) = sender {
            if let Ok(factor) = crate::site::zoom_factor_controller(&sender) {
                let _ = zoom_proxy.send_event(Event::Zoom(zoom_id.clone(), zoom_hwnd, factor));
            }
        }
        Ok(())
    }));
    unsafe {
        view.controller().add_ZoomFactorChanged(&zoom, &mut token)?;
    }
    // Sound: drives the tab row's speaker indicator and the mute label.
    let audio = IsDocumentPlayingAudioChangedEventHandler::create(Box::new(move |sender, _| {
        if let Some(sender) = sender {
            if let Ok(playing) = crate::site::is_playing_audio_core(&sender) {
                let _ = audio_proxy.send_event(Event::Audio(audio_id.clone(), playing));
            }
        }
        Ok(())
    }));
    unsafe {
        view.webview()
            .cast::<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8>()?
            .add_IsDocumentPlayingAudioChanged(&audio, &mut token)?;
    }
    Ok(())
}
/// Reconciles one operation using its current state, both for notifications and
/// the watchdog. Receiving every byte does not imply COMPLETED: WebView2 may
/// still be checking or renaming the file.
fn sync_download(
    operation: &ICoreWebView2DownloadOperation,
    id: &str,
    downloads: &RefCell<Downloads>,
    ops: &RefCell<HashMap<String, ICoreWebView2DownloadOperation>>,
) -> windows::core::Result<bool> {
    let current = downloads.borrow().get(id).cloned();
    let Some(current) = current.filter(DownloadItem::is_active) else {
        return Ok(false);
    };
    unsafe {
        let mut code = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
        operation.State(&mut code)?;
        let mut received = current.received as i64;
        let _ = operation.BytesReceived(&mut received);
        let mut total = current.total as i64;
        let _ = operation.TotalBytesToReceive(&mut total);
        let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE;
        if code == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
            operation.InterruptReason(&mut reason)?;
        }
        let path = if code == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
            let mut pointer = PWSTR::null();
            operation.ResultFilePath(&mut pointer).ok()
                .map(|_| take_pwstr(pointer))
        } else {
            None
        };
        let mut downloads = downloads.borrow_mut();
        // A COM getter above may have dispatched the terminal callback already.
        let Some(current) = downloads.get(id).filter(|item| item.is_active()).map(|item| item.state)
        else {
            return Ok(false);
        };
        downloads.set_progress(id, received.max(0) as u64, total.max(0) as u64);
        let next = if code == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
            if let Some(path) = path.filter(|path| !path.is_empty()) {
                if let Some(item) = downloads.get_mut(id) {
                    item.path = path;
                    if let Some(name) = FilePath::new(&item.path).file_name() {
                        item.filename = name.to_string_lossy().into_owned();
                    }
                }
            }
            downloads.finish(id, DownloadState::Finished, None);
            DownloadState::Finished
        } else if code == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
            if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED {
                downloads.finish(id, DownloadState::Cancelled, None);
                DownloadState::Cancelled
            } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED {
                if let Some(item) = downloads.get_mut(id) {
                    item.state = DownloadState::Paused;
                }
                DownloadState::Paused
            } else {
                let state = if crate::downloads::is_security_interrupt(reason) {
                    DownloadState::Blocked
                } else {
                    DownloadState::Failed
                };
                downloads.finish(
                    id, state,
                    Some(crate::downloads::interrupt_reason_text(reason)),
                );
                state
            }
        } else {
            if let Some(item) = downloads.get_mut(id) {
                item.state = DownloadState::InProgress;
            }
            DownloadState::InProgress
        };
        drop(downloads);
        if !next.is_active() {
            let removed = ops.borrow_mut().remove(id);
            drop(removed);
        }
        Ok(next != current)
    }
}

/// Attaches Zigzag's state machine to one WebView2 download: progress ticks are
/// coalesced for the UI, and the operation is forgotten once it can no longer
/// be paused or resumed.
fn track_download(
    operation: &ICoreWebView2DownloadOperation,
    id: String,
    downloads: Rc<RefCell<Downloads>>,
    ops: Rc<RefCell<HashMap<String, ICoreWebView2DownloadOperation>>>,
    proxy: EventLoopProxy<Event>,
) -> windows::core::Result<()> {
    let state_downloads = downloads.clone();
    let state_ops = ops.clone();
    let state_proxy = proxy.clone();
    let state_id = id.clone();
    let state = StateChangedEventHandler::create(Box::new(move |sender, _| {
        let Some(sender) = sender else { return Ok(()) };
        let persist = sync_download(&sender, &state_id, &state_downloads, &state_ops)?;
        let _ = state_proxy.send_event(Event::DownloadsChanged { persist });
        Ok(())
    }));
    let mut token = 0;
    unsafe { operation.add_StateChanged(&state, &mut token)?; }
    let bytes_downloads = downloads;
    let bytes_proxy = proxy;
    let bytes_id = id;
    let last_update = Rc::new(Cell::new(Instant::now() - Duration::from_secs(1)));
    let bytes = BytesReceivedChangedEventHandler::create(Box::new(move |sender, _| {
        unsafe {
        let Some(sender) = sender else { return Ok(()) };
        let mut received = 0i64;
        sender.BytesReceived(&mut received)?;
        let mut total = 0i64;
        let _ = sender.TotalBytesToReceive(&mut total);
        bytes_downloads
            .borrow_mut()
            .set_progress(&bytes_id, received.max(0) as u64, total.max(0) as u64);
        // WebView2 can tick many times a second; the panel only needs ~8 fps.
        let now = Instant::now();
        if now.duration_since(last_update.get()) >= Duration::from_millis(120) {
            last_update.set(now);
            let _ = bytes_proxy.send_event(Event::DownloadsChanged { persist: false });
        }
        Ok(())
        }
    }));
    unsafe { operation.add_BytesReceivedChanged(&bytes, &mut token)?; }
    Ok(())
}
// Window names are untrusted input. Only finite normalized click coordinates are accepted.
fn peek_target(name: &str) -> Option<Option<Point>> {
    if name == "__era_peek__" {
        return Some(None);
    }
    let value = name.strip_prefix("__era_peek__:")?;
    let point = value.split_once(',').and_then(|(x, y)| {
        let x = x.parse::<f64>().ok()?;
        let y = y.parse::<f64>().ok()?;
        ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then_some(Point { x, y })
    });
    Some(point)
}
#[cfg(test)]
#[path = "download_tests.rs"]
mod download_tests;
#[cfg(test)]
mod motion_tests {
    use super::*;
    #[test]
    fn normalized_link_metadata_is_bounded() {
        let p = peek_target("__era_peek__:0.25,0.75").unwrap().unwrap();
        assert_eq!(p.x, 0.25);
        assert_eq!(p.y, 0.75);
        for name in [
            "__era_peek__:NaN,0.5",
            "__era_peek__:1.1,0",
            "__era_peek__:-1,0",
            "__era_peek__:0,inf",
        ] {
            assert!(peek_target(name).unwrap().is_none())
        }
        assert!(peek_target("_blank").is_none());
    }
}

fn register_accelerators(
    view: &WebView, proxy: EventLoopProxy<Event>, find_open: Rc<Cell<bool>>,
    peek_visible: Rc<Cell<bool>>, downloads_open: Rc<Cell<bool>>,
    settings: Rc<RefCell<crate::settings::Settings>>, page_fullscreen: Option<Rc<Cell<bool>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let p = proxy.clone();
    let shortcut_settings = settings;
    let reader = page_fullscreen.is_none();
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            unsafe {
                let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
                args.KeyEventKind(&mut kind)?;
                if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                    && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                {
                    return Ok(());
                }
                let mut key = 0;
                args.VirtualKey(&mut key)?;
                let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
                let alt = GetKeyState(VK_MENU.0 as i32) < 0;
                let action = if key == 27 && !ctrl && !alt && !shift {
                    // WebView2 handles Escape for its fullscreen element first.
                    if page_fullscreen.as_ref().is_some_and(|fullscreen| fullscreen.get()) { return Ok(()); }
                    // Esc closes the downloads panel first, then the find bar,
                    // then Peek.
                    if downloads_open.get() { Some("downloadsEscape".into()) }
                    else if find_open.get() { Some("findEscape".into()) }
                    else if peek_visible.get() { Some("escape".into()) }
                    else if reader { Some("readerEscape".into()) }
                    else { None }
                }
                    // Ctrl+J opens the downloads panel, like Chrome.
                    else if ctrl && !alt && !shift && key == 74 { Some("downloads".into()) }
                    else if reader && !ctrl && !alt && !shift && key == 116 { Some("reload".into()) }
                    // Browser-standard page zoom: Ctrl+plus/minus/0, also from
                    // the OEM keys, the numpad and Ctrl+Shift+plus.
                    else if ctrl && !alt && (key == 187 || key == 107) { Some("zoomIn".into()) }
                    else if ctrl && !alt && (key == 189 || key == 109) { Some("zoomOut".into()) }
                    else if ctrl && !alt && !shift && (key == 48 || key == 96) { Some("zoomReset".into()) }
                    else if ctrl && !alt && !shift && (49..=57).contains(&key) { Some(format!("space:{}", key - 49)) }
                    else {
                        shortcut_settings.borrow().action(&crate::settings::Shortcut {key, ctrl, alt, shift})
                            .or_else(|| match (ctrl, alt, shift, key) {
                                (true, false, false, 80) => Some("printPage".into()),
                                (true, false, false, 83) => Some("savePageAs".into()),
                                _ => None,
                            })
                    };
                if let Some(action) = action {
                    args.SetHandled(true)?;
                    if matches!(action.as_str(), "printPage" | "savePageAs") {
                        let mut status = Default::default();
                        args.PhysicalKeyStatus(&mut status)?;
                        if status.WasKeyDown.as_bool() { return Ok(()); }
                    }
                    let _ = p.send_event(Event::Shortcut(action, false));
                }
            }
        }
        Ok(())
    }));
    let mut token = 0;
    unsafe {
        view.controller()
            .add_AcceleratorKeyPressed(&handler, &mut token)?;
    }
    Ok(())
}
