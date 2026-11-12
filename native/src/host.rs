use crate::{
    clip::WindowClip,
    downloads::{DownloadItem, DownloadState, Downloads},
    model::{self, Command, PeekPhase, Point, Rect, State, Tab},
    storage::Storage,
};
use include_dir::{include_dir, Dir};
use serde_json::json;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::HashMap,
    path::{Path as FilePath, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
use webview2_com::{
    take_pwstr, AcceleratorKeyPressedEventHandler, BytesReceivedChangedEventHandler,
    DownloadStartingEventHandler, FaviconChangedEventHandler,
    IsDocumentPlayingAudioChangedEventHandler, StateChangedEventHandler,
    ZoomFactorChangedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2DownloadOperation, ICoreWebView2NavigationStartingEventArgs3,
        ICoreWebView2NewWindowRequestedEventArgs2, ICoreWebView2_15, ICoreWebView2_4,
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
                GetCursorPos, SetWindowPos, HWND_BOTTOM, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE,
                SWP_NOSIZE, SWP_NOZORDER, SW_SHOWNORMAL,
            },
        },
    },
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{Icon, Window, WindowId},
};
use wry::{
    PageLoadEvent, WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows,
};

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../dist");
// Keep the standalone UI preview's --radius-window default in sync.
pub(crate) const WINDOW_RADIUS: f64 = 12.;
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
#[derive(Debug)]
enum Event {
    ImportSources(Vec<crate::import::Source>),
    ImportRead(Vec<crate::import::Batch>),
    Layout,
    SplitResize,
    DataCleared(String, bool),
    Command(Command),
    Title(String, String),
    Load(String, String, bool),
    Source(String, String),
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
    /// A page's controller reported a new zoom factor.
    Zoom(String, f64),
    /// A page started or stopped playing sound.
    Audio(String, bool),
    /// The download list changed; persist only for starts and state changes,
    /// not for every bytes tick.
    DownloadsChanged {
        persist: bool,
    },
    /// A new download joined the list: the UI reveals the panel on it.
    DownloadStarted(String),
}
struct Page {
    view: WebView,
    url: String,
    clip: Rc<WindowClip>,
    thumbnail_version: u64,
    thumbnail_attempt: Option<u64>,
    thumbnail_pending: bool,
    visible: Cell<bool>,
    bounds: Rc<Cell<Option<[i32; 4]>>>,
    viewport: Rc<Cell<Option<[i32; 2]>>>,
    dark: Cell<bool>,
    navigation: Rc<NavigationPolicy>,
}
struct NavigationPolicy {
    bookmarked: Cell<bool>,
    current: RefCell<String>,
    host_navigation: Cell<bool>,
}
impl Page {
    fn split_pane(&self) -> crate::split::Pane {
        crate::split::Pane { hwnd: self.view.hwnd(), controller: self.view.controller(),
            clip: self.clip.clone(), bounds: self.bounds.clone(), viewport: self.viewport.clone() }
    }
    fn set_window_bounds(&self, bounds: wry::Rect, scale: f64) -> wry::Result<()> {
        let position = bounds.position.to_physical::<i32>(scale);
        let size = bounds.size.to_physical::<i32>(scale);
        let next = [position.x, position.y, size.width, size.height];
        if self.bounds.get() != Some(next) {
            // Move the HWND before asking Chromium to reflow. Wry does these in
            // the opposite order, making visible edges wait on renderer work.
            unsafe { SetWindowPos(self.view.hwnd(), None, position.x, position.y,
                size.width, size.height, SWP_NOACTIVATE | SWP_NOZORDER)?; }
            self.bounds.set(Some(next));
        }
        Ok(())
    }
    fn sync_viewport(&self) -> wry::Result<()> {
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
            self.view.set_background_color(surface_color(dark))?;
            self.dark.set(dark);
        }
        Ok(())
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
    live_resize: bool,
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
}
/// Tabs removed together by Clear or Close Others/Below, with the active tab
/// before the batch so the Undo toast can put everything back as it was.
struct UndoBatch {
    entries: Vec<(String, usize, Tab)>,
    previous_active: String,
    /// A blank tab that closing everything auto-created; undo takes it back.
    auto_created: Option<String>,
}
struct App {
    import_sources: Vec<crate::import::Source>,
    import_busy: bool,
    // Drop child controllers before their parent HWND and shared context.
    split_handle: Option<crate::split::SplitHandle>,
    pages: HashMap<String, Page>,
    data_clear_view: Option<WebView>,
    live_settings: Rc<RefCell<crate::settings::Settings>>,
    peek: Option<Peek>,
    peek_visible: Rc<Cell<bool>>,
    caption: Option<crate::caption::Caption>,
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
    downloads_open: Rc<Cell<bool>>,
    popup: Option<WebView>,
    popup_state: serde_json::Map<String, serde_json::Value>,
    ui: Option<WebView>,
    window: Option<Window>,
    window_clip: WindowClip,
    context: WebContext,
    state: State,
    storage: Storage,
    proxy: EventLoopProxy<Event>,
    primary: Rect,
    secondary: Option<Rect>,
    pending_layout: Rc<Cell<Option<PageLayout>>>,
    live_resize: bool,
    overlay: bool,
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
}
struct PreviewRender {
    view: WebView,
    source: String,
    epoch: u64,
    loading: bool,
    pending: bool,
    version: u64,
}
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Keep the existing profile location so the rename preserves sessions and logins.
    let data = std::env::var_os("ERA_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            directories::ProjectDirs::from("", "Era", "EraBrowser")
                .map(|d| d.data_local_dir().to_path_buf())
        })
        .ok_or("无法定位用户数据目录")?;
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
    let mut app = App {
        import_sources: vec![],
        import_busy: false,
        split_handle: None,
        live_settings: Rc::new(RefCell::new(state.settings.clone())),
        data_clear_view: None,
        pages: HashMap::new(),
        peek: None,
        peek_visible: Rc::new(Cell::new(false)),
        caption: None,
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
        downloads_open: Rc::new(Cell::new(false)),
        popup: None,
        popup_state: serde_json::Map::new(),
        ui: None,
        window: None,
        window_clip: WindowClip::default(),
        context: WebContext::new(Some(data.join("WebView2"))),
        state,
        storage,
        proxy,
        primary: Rect::default(),
        secondary: None,
        pending_layout: Rc::new(Cell::new(None)),
        live_resize: false,
        overlay: false,
        ready: false,
        last_error: None,
        ui_data: data.join("UI"),
        glass: false,
        site_icons: crate::favicon::SiteIcons::default(),
        icon_refresh,
        icon_refresh_found: false,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
impl App {
    fn confirm_quit(&self) -> bool {
        if !self.state.settings.ask_before_quit { return true; }
        unsafe {
            use windows::{core::w, Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_YESNO, MB_ICONQUESTION, MB_DEFBUTTON2, IDYES}};
            let parent = self.window.as_ref().and_then(|w| w.window_handle().ok()).and_then(|handle| match handle.as_raw() { RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)), _ => None });
            MessageBoxW(parent, w!("关闭窗口并退出 Rove？标签页与工作区会在下次启动时恢复。"), w!("退出 Rove"), MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2) == IDYES
        }
    }
    fn update_window_frame(&self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window) = &self.window else {
            return Ok(());
        };
        let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else {
            return Err("无法获取 Windows 窗口句柄".into());
        };
        let maximized = window.is_maximized();
        self.window_clip.apply_window(
            HWND(handle.hwnd.get() as *mut _),
            WINDOW_RADIUS,
            window.scale_factor(),
            maximized,
        )?;
        if self.ready {
            if let Some(ui) = &self.ui {
                let radius = if maximized { 0. } else { WINDOW_RADIUS };
                ui.evaluate_script(&format!(
                    "document.documentElement.style.setProperty('--radius-window', '{radius}px')"
                ))?;
            }
        }
        Ok(())
    }

    /// Acrylic under a see-through frame. A failed backdrop leaves the frame solid.
    fn apply_glass(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window) = &self.window else {
            return Ok(());
        };
        let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else {
            return Ok(());
        };
        let wanted = self.glass;
        let shown = crate::glass::apply(HWND(handle.hwnd.get() as *mut _), wanted) && wanted;
        if wanted && !shown {
            self.glass = false;
            self.emit(json!({"type":"glassUnavailable"}));
        }
        if let Some(ui) = &self.ui {
            let color = if shown {
                (0, 0, 0, 0)
            } else {
                surface_color(self.state.theme == "dark")
            };
            ui.set_background_color(color)?;
        }
        Ok(())
    }

    fn emit(&self, value: serde_json::Value) {
        if value["type"] != "shortcut" {
            if let Some(popup) = &self.popup {
                let _ = popup.evaluate_script(&format!("window.__ERA_RECEIVE__?.({value})"));
            }
        }
        if let Some(ui) = &self.ui {
            let _ = ui.evaluate_script(&format!("window.__ERA_RECEIVE__?.({value})"));
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
            self.emit(json!({"type":"state","state":self.state}));
        }
    }
    fn save(&mut self) {
        if let Err(e) = self.storage.save(&self.state) {
            self.error(format!("保存会话失败：{e}"))
        }
        // The downloads list is small; writing it on every state change already,
        // and once more on exit keeps the final byte count of a quit download.
        let items = self.downloads.borrow().items.clone();
        let _ = self.storage.save_downloads(&items);
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
        self.peek
            .as_ref()
            .map(|p| &p.page.view)
            .or_else(|| self.pages.get(&self.state.active_tab).map(|p| &p.view))
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
    /// Applies the tab's mute state and its site's zoom to a fresh controller.
    fn apply_site_controls(&self, view: &WebView, url: &str, muted: bool) {
        let _ = crate::site::set_muted(view, muted);
        crate::site::ensure_zoom(view, self.site_zoom_for(url));
    }
    /// Tells the UI the shown page's zoom, so its site menu and toast start in
    /// the right place after a tab, space or split switch.
    fn emit_active_zoom(&self) {
        let target = self
            .peek
            .as_ref()
            .map(|peek| (peek.tab.id.clone(), &peek.page.view))
            .or_else(|| {
                let tab = self.state.active()?;
                let page = self.pages.get(&tab.id)?;
                Some((tab.id.clone(), &page.view))
            });
        if let Some((id, view)) = target {
            if let Ok(factor) = crate::site::zoom_factor(view) {
                self.emit(json!({"type":"zoom","id":id,"factor":factor}));
            }
        }
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
        let items = self.downloads.borrow_mut().publish_items();
        self.emit(json!({"type":"downloads","items":items}));
    }
    fn save_downloads(&mut self) {
        let items = self.downloads.borrow().items.clone();
        if let Err(error) = self.storage.save_downloads(&items) {
            self.error(error);
        }
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
            "pause" => {
                if let Some(operation) = &operation {
                    if let Err(error) = unsafe { operation.Pause() } {
                        self.error(format!("无法暂停下载：{error}"));
                        return;
                    }
                }
                if let Some(item) = self.downloads.borrow_mut().get_mut(&id) {
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
                if let Some(item) = self.downloads.borrow_mut().get_mut(&id) {
                    item.state = DownloadState::InProgress;
                }
                self.save_downloads();
                self.publish_downloads();
            }
            "cancel" => {
                if let Some(operation) = &operation {
                    let _ = unsafe { operation.Cancel() };
                }
                if self.downloads.borrow_mut().get(&id).is_some() {
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
                let Some(item) = self.downloads.borrow().get(&id).cloned() else {
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
        let view = WebViewBuilder::new_with_web_context(&mut self.context)
            .with_url(&tab.url)
            .with_visible(false)
            .with_focused(false)
            .with_background_color(surface_color(self.state.theme == "dark"))
            .with_additional_browser_args(browser_args(false))
            .with_theme(if self.state.theme == "dark" {
                wry::Theme::Dark
            } else {
                wry::Theme::Light
            })
            .with_navigation_handler(|url| model::is_web(&url) || url == "about:blank")
            .with_initialization_script(include_str!("peek-links.js"))
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
            // The script only adapts standard link opening. There is still no IPC/host API on websites.
            .build_as_child(self.window.as_ref().unwrap())?;
        crate::web_settings::apply(&view, &self.state.settings)?;
        let shared = PageShared {
            peek_visible: self.peek_visible.clone(),
            find_open: self.find_open.clone(),
            downloads_open: self.downloads_open.clone(),
            downloads: self.downloads.clone(),
            download_ops: self.download_ops.clone(),
        };
        register_page_events(
            &view,
            tab.id.clone(),
            self.proxy.clone(),
            shared,
            navigation.clone(),
            self.live_settings.clone(),
        )?;
        self.apply_site_controls(&view, &tab.url, tab.muted);
        Ok(Page {
            view,
            url: tab.url.clone(),
            clip: Rc::new(WindowClip::default()),
            thumbnail_version: 0,
            thumbnail_attempt: None,
            thumbnail_pending: false,
            visible: Cell::new(false),
            bounds: Rc::new(Cell::new(None)),
            viewport: Rc::new(Cell::new(None)),
            dark: Cell::new(self.state.theme == "dark"),
            navigation,
        })
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
            let view = WebViewBuilder::new_with_web_context(&mut self.context)
                .with_url(&tab.url).with_visible(false).with_focused(false)
                .with_theme(if self.state.theme == "dark" { wry::Theme::Dark } else { wry::Theme::Light })
                .with_background_color(surface_color(self.state.theme == "dark"))
                .with_additional_browser_args(browser_args(false))
                .with_navigation_handler(|url| model::is_web(&url) || url == "about:blank")
                .with_new_window_req_handler(|_, _| wry::NewWindowResponse::Deny)
                .with_download_started_handler(|_, _| false)
                .with_on_page_load_handler(move |event, url| {
                    let _ = proxy.send_event(Event::PreviewLoad(key.clone(), epoch, url, matches!(event, PageLoadEvent::Started)));
                }).build_as_child(self.window.as_ref().unwrap())?;
            crate::web_settings::apply(&view, &self.state.settings)?;
            view.set_bounds(self.bounds(self.primary))?;
            let timed_key = tab.id.clone();
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(12));
                let _ = proxy.send_event(Event::PreviewLoad(timed_key, epoch, String::new(), false));
            });
            self.preview_renders.insert(tab.id, PreviewRender { view, source: tab.url, epoch, loading: true, pending: false, version: 0 });
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
        if self.state.settings.remember_history { let _ = self.storage.visit(&peek.tab.title, &peek.tab.url); }
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
        if !self.state.onboarding.completed { return self.sync_layout(); }
        // Release native mouse capture and controller references before a tab
        // switch or close can destroy the pages the handle was resizing.
        self.finish_split_resize();
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
        self.sync_layout()
    }
    // Layout messages arrive every frame during sidebar motion. They must not
    // create/navigate pages, touch profile settings, or repeat ShowWindow calls.
    fn sync_page_bounds(&self) -> Result<bool, Box<dyn std::error::Error>> {
        if self.split_handle.as_ref().is_some_and(|handle| handle.dragging()) {
            return Ok(false);
        }
        let Some(window) = &self.window else {
            return Ok(false);
        };
        if !self.state.onboarding.completed {
            let changed = self.pages.values().any(|page| page.visible.get());
            for page in self.pages.values() { page.set_visible(false)?; }
            if let Some(handle) = &self.split_handle { handle.configure(None)?; }
            return Ok(changed);
        }
        let mut visibility_changed = false;
        let mut visibility = Vec::new();
        let mut focus_peek = false;
        let second = self.state.active().and_then(|t| t.split_with.as_ref());
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
                if self.live_resize { page.clip.clear(page.view.hwnd())?; }
                page.set_window_bounds(self.bounds(r), window.scale_factor())?;
                if !self.live_resize {
                    page.clip.apply(page.view.hwnd(), r.corners, window.scale_factor())?;
                }
                visibility_changed |= !page.visible.get();
                visibility.push((page, true));
            } else {
                // WebView2 stops painting IsVisible=false controllers. Let only the
                // watched pages render behind the trusted UI while their card is up.
                let visible = watched.contains(id);
                visibility_changed |= page.visible.get() != visible;
                visibility.push((page, visible));
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
                visibility.push((&peek.page, visible));
                focus_peek = !was_visible && visible;
            } else {
                visibility_changed |= peek.shown.get();
                peek.shown.set(false);
                visibility.push((&peek.page, false));
            }
        }
        // Both live edges move together before either renderer reflows.
        for page in self.pages.values().chain(self.peek.iter().map(|p| &p.page)) {
            page.sync_viewport()?;
        }
        // Newly shown pages need their first viewport before becoming visible.
        for (page, visible) in visibility { page.set_visible(visible)?; }
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
    fn finish_split_resize(&mut self) {
        if let Some(handle) = &self.split_handle { handle.cancel(); }
        // Consume the final dimensions before any ordinary layout can reapply
        // an older frame during a navigation or overlay transition.
        self.receive_split_resize();
    }
    fn sync_layout(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.sync_page_bounds()?;
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
        if !self.state.onboarding.completed { return; }
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
            "reload" => self.command(Command::Reload, None),
            "back" => self.command(Command::Back, None),
            "forward" => self.command(Command::Forward, None),
            "zoomIn" => self.command(Command::Zoom { action: "in".into() }, None),
            "zoomOut" => self.command(Command::Zoom { action: "out".into() }, None),
            "zoomReset" => self.command(Command::Zoom { action: "reset".into() }, None),
            "find" => self.command(Command::FindOpen, None),
            "findEscape" => self.command(Command::FindClose, None),
            "escape" => self.request_peek_close(),
            _ => {}
        }
    }
    fn command(&mut self, c: Command, event_loop: Option<&ActiveEventLoop>) {
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
            }
            let make_default = matches!(c, Command::OnboardingAdvance { step: crate::onboarding::Step::Ready, make_default: true, .. });
            if let Err(e) = self.state.apply(&c) { self.error(e); return; }
            self.save(); self.publish();
            if let Err(e) = self.sync() { self.error(e); }
            if let Some(ui) = &self.ui { let _ = ui.focus(); }
            // Windows owns the final choice. Opening Settings never claims that
            // Rove has become the default browser.
            if make_default { self.command(Command::SystemSettings, event_loop); }
            return;
        }
        if !self.state.onboarding.completed && !matches!(c,
            Command::Ready | Command::Presented | Command::Window { .. } | Command::WindowChrome { .. }
            | Command::Layout { .. } | Command::Glass { .. } | Command::PopupReady
            | Command::PopupState { .. } | Command::PopupLayer { .. } | Command::Overlay { .. }
        ) { return; }
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
                    | Command::Unsplit
                    | Command::PeekOpen { .. }
                    | Command::PeekClose
                    | Command::PeekPromote
            )
        {
            self.close_find(true);
        }
        if matches!(c, Command::OpenSettings { .. }) { self.close_peek(); }
        // The page behind a switch may already exist with its site's zoom; the
        // UI only learns a factor from an event, so send the shown page's zoom.
        let zoom_after_transition = matches!(
            c,
            Command::Activate { .. }
                | Command::New { .. }
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
            Command::SystemSettings => {
                unsafe {
                    use windows::{core::w, Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL}};
                    let result = ShellExecuteW(None, w!("open"), w!("ms-settings:defaultapps"), None, None, SW_SHOWNORMAL);
                    if result.0 as isize <= 32 { self.error("无法打开 Windows 默认应用设置"); }
                }
                return;
            }
            Command::ClearBrowsingData { kind } => {
                if kind == "history" {
                    match self.storage.clear_history() {
                        Ok(_) => { self.state.closed.clear(); self.save(); self.publish(); self.emit(json!({"type":"dataCleared","action":"history","message":"已清除浏览历史"})); }
                        Err(e) => self.error(e),
                    }
                } else if kind == "cache" || kind == "siteData" {
                    if self.data_clear_view.is_some() { self.error("正在清除浏览数据，请稍候"); return; }
                    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                        let view = WebViewBuilder::new_with_web_context(&mut self.context).with_url("about:blank").with_visible(false).with_focused(false).with_additional_browser_args(browser_args(false)).build_as_child(self.window.as_ref().ok_or("窗口尚未就绪")?)?;
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
                viewport_width,
                background,
                hover,
                foreground,
            } => {
                if let (Some(caption), Some(window)) = (&self.caption, &self.window) {
                    if let Err(e) = caption.update(
                        maximize,
                        viewport_width,
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
            Command::Navigate { url } if self.peek.is_some() => {
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
                if let Err(e) = self.update_window_frame() {
                    self.error(e);
                }
                self.publish();
                self.publish_downloads();
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
            Command::Layout { primary, secondary, live_resize } => {
                if self.split_handle.as_ref().is_some_and(|handle| handle.dragging()) { return; }
                if primary.valid() && secondary.is_none_or(|r| r.valid()) {
                    self.primary = primary;
                    self.secondary = secondary;
                    self.live_resize = live_resize && secondary.is_some();
                } else {
                    return;
                }
                let result = self.sync_page_bounds().and_then(|changed| {
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
                    self.finish_split_resize();
                    self.dismiss_tab_preview();
                }
                if visible && !self.overlay && !command { self.refresh_visible_thumbnails(); }
                self.overlay = visible;
                if let Err(e) = self.sync_layout() { self.error(e); }
                // PopupLayer requests focus once its input region is installed.
                // Tab dragging also uses Overlay and keeps shell capture.
                if !visible {
                    if let Some(page) = self.page() {
                        let _ = page.focus();
                    }
                }
                return;
            }
            Command::PopupReady => {
                self.emit(json!({"type":"state","state":self.state}));
                self.emit(json!({"type":"popupState","popup":self.popup_state}));
                self.publish_peek();
                return;
            }
            Command::PopupState { popup } => {
                if let Some(patch) = popup.as_object() {
                    for (key, value) in patch { self.popup_state.insert(key.clone(), value.clone()); }
                    self.emit(json!({"type":"popupState","popup":patch}));
                }
                return;
            }
            Command::PopupLayer { regions, cutout, focus } => {
                if regions.len() > 16 || !regions.iter().all(|r| r.valid()) || cutout.is_some_and(|r| !r.valid()) { return; }
                if let (Some(popup), Some(window)) = (&self.popup, &self.window) {
                    let result = WindowClip::popup(popup.hwnd(), &regions, cutout, window.scale_factor());
                    if result.is_ok() && focus { let _ = popup.focus(); }
                    if let Err(e) = result { self.error(e); }
                }
                if regions.is_empty() {
                    if let Some(page) = self.page() { let _ = page.focus(); }
                    else if let Some(ui) = &self.ui { let _ = ui.focus(); }
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
            Command::FindOpen => {
                self.open_find();
                return;
            }
            Command::FindClose => {
                self.close_find(true);
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
                let Some(url) = self
                    .peek
                    .as_ref()
                    .map(|peek| peek.tab.url.clone())
                    .or_else(|| self.state.active().map(|tab| tab.url.clone()))
                    .filter(|url| model::is_web(url))
                else {
                    return;
                };
                let factor = match self.page() {
                    Some(view) => {
                        let current = crate::site::zoom_factor(view).unwrap_or(1.0);
                        let next = match action.as_str() {
                            "reset" => 1.0,
                            "out" => model::next_zoom(current, -1),
                            _ => model::next_zoom(current, 1),
                        };
                        if crate::site::set_zoom_factor(view, next).is_err() {
                            return;
                        }
                        next
                    }
                    None => return,
                };
                // Zigzag's ZoomLevels: remembered per host; 100% is the absence
                // of an entry, so a forgotten site stays clean.
                if let Some(site) = model::site_key(&url) {
                    if (factor - 1.0).abs() <= 0.001 {
                        self.state.site_zoom.remove(&site);
                    } else {
                        self.state.site_zoom.insert(site, factor);
                    }
                    self.save();
                }
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
            Command::Back | Command::Forward | Command::Reload => {
                if let Some(p) = self.page() {
                    let r = match c {
                        Command::Back => p.go_back(),
                        Command::Forward => p.go_forward(),
                        _ => p.reload(),
                    };
                    if let Err(e) = r {
                        self.error(e)
                    }
                }
                return;
            }
            _ => {
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
                if let Err(e) = self.state.apply(&c) {
                    self.error(e);
                    return;
                }
                // A tab closed through the generic path stops being "playing".
                self.playing.retain(|id| self.state.tabs.iter().any(|tab| &tab.id == id));
                *self.live_settings.borrow_mut() = self.state.settings.clone();
                if matches!(c, Command::UpdateSettings { .. }) {
                    let mut errors = vec![];
                    for view in self.pages.values().map(|p| &p.view).chain(self.peek.iter().map(|p| &p.page.view)).chain(self.preview_renders.values().map(|p| &p.view)) {
                        if let Err(e) = crate::web_settings::apply(view, &self.state.settings) { errors.push(e); }
                        if !self.state.settings.autofill { let _ = view.evaluate_script("window.dispatchEvent(new Event('__rove_disable_imported_logins'))"); }
                    }
                    for error in errors { self.error(error); }
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
                        .header("Content-Security-Policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' https: data:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'")
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
                        if let Command::Layout { primary, secondary, live_resize } = c {
                            if primary.valid() && secondary.is_none_or(|r| r.valid())
                                && pending_layout.replace(Some(PageLayout { primary, secondary, live_resize })).is_none() {
                                let _ = proxy.send_event(Event::Layout);
                            }
                        } else { let _=proxy.send_event(Event::Command(c)); }
                    }
                }).build_as_child(&window)?;
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
            self.split_handle = Some(crate::split::SplitHandle::new(
                HWND(handle.hwnd.get() as *mut _),
                self.popup.as_ref().unwrap().hwnd(),
                move || {
                    pending_layout.take();
                    let _ = proxy.send_event(Event::SplitResize);
                },
            )?);
            self.ui = Some(ui);
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
            Event::ImportSources(sources) => {
                self.import_sources = sources;
                self.import_busy = false;
                self.emit(json!({"type":"importState","importBusy":false,"importSources":self.import_sources}));
            }
            Event::ImportRead(batches) => {
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
            Event::Layout => {
                if let Some(layout) = self.pending_layout.take() {
                    self.command(Command::Layout { primary: layout.primary, secondary: layout.secondary, live_resize: layout.live_resize }, Some(event_loop));
                }
            }
            Event::DataCleared(kind, ok) => {
                self.data_clear_view = None;
                self.emit(json!({"type":"dataCleared","action":kind,"message":if ok { "已清除浏览数据" } else { "清除失败，请重试" }}));
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
            Event::FindMatches(count, active) => {
                if self.find_open.get() {
                    self.emit(json!({"type":"findMatches","count":count,"active":active}));
                }
            }
            Event::Zoom(id, factor) => {
                if !model::valid_zoom(factor) {
                    return;
                }
                // Remember what the page settled on, whoever changed it: our
                // zoom ladder, WebView2's own Ctrl+wheel/pinch, or a reset.
                if let Some(site) = self
                    .peek
                    .as_ref()
                    .filter(|peek| peek.tab.id == id)
                    .map(|peek| peek.tab.url.clone())
                    .or_else(|| self.state.tabs.iter().find(|tab| tab.id == id).map(|tab| tab.url.clone()))
                    .and_then(|url| model::site_key(&url))
                {
                    let changed = if (factor - 1.0).abs() <= 0.001 {
                        self.state.site_zoom.remove(&site).is_some()
                    } else {
                        self.state
                            .site_zoom
                            .insert(site, factor)
                            .is_none_or(|previous| (previous - factor).abs() > 0.001)
                    };
                    if changed {
                        self.save();
                    }
                }
                self.emit(json!({"type":"zoom","id":id,"factor":factor}));
            }
            Event::Audio(id, playing) => {
                if playing {
                    self.playing.insert(id.clone());
                } else {
                    self.playing.remove(&id);
                }
                self.emit(json!({"type":"audio","id":id,"playing":playing}));
            }
            Event::DownloadsChanged { persist } => {
                if persist {
                    self.save_downloads();
                }
                self.publish_downloads();
            }
            Event::DownloadStarted(id) => {
                self.save_downloads();
                self.publish_downloads();
                self.emit(json!({"type":"downloadStarted","id":id}));
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
                if let Some(p) = self.peek.as_mut().filter(|p| p.tab.id == id) {
                    p.tab.title = title.chars().take(512).collect();
                    self.publish_peek();
                    return;
                }
                if let Some(t) = self.state.tabs.iter_mut().find(|t| t.id == id) {
                    t.title = title.chars().take(512).collect();
                    if self.state.settings.remember_history && model::is_web(&t.url) {
                        let _ = self.storage.visit(&t.title, &t.url);
                    }
                }
                self.publish();
                self.save();
            }
            Event::Load(id, url, loading) => {
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
                            crate::site::ensure_zoom(&p.page.view, desired_zoom);
                        }
                    }
                    self.publish_peek();
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
                            p.url = url;
                            if loading {
                                crate::site::ensure_zoom(&p.view, desired_zoom);
                            }
                        }
                    }
                    if !loading && self.state.settings.remember_history {
                        let _ = self.storage.visit(&t.title, &t.url);
                    }
                }
                self.publish();
                if !loading {
                    if self.state.settings.autofill {
                        if let Some(page) = self.pages.get(&id) {
                            if let Some(origin) = crate::import::origin(&page.url) {
                                let logins = self.storage.imported_logins(&origin);
                                if !logins.is_empty() { let _ = page.view.evaluate_script(&crate::passwords::fill_script(&origin, &logins)); }
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
                    self.save();
                    let proxy = self.proxy.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(140));
                        let _ = proxy.send_event(Event::CaptureTabs);
                    });
                }
            }
            Event::Source(id, url) => {
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
                        p.url = url;
                    }
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
            WindowEvent::Focused(false) => self.dismiss_tab_preview(),
            WindowEvent::CloseRequested => {
                if !self.confirm_quit() { return; }
                self.save();
                event_loop.exit()
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.finish_split_resize();
                self.dismiss_tab_preview();
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
                if let Err(e) = self.sync() {
                    self.error(e)
                }
            }
            _ => {}
        }
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.save();
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
    let p = proxy.clone();
    let shortcut_settings = settings.clone();
    let find_open = shared.find_open.clone();
    let peek_visible = shared.peek_visible.clone();
    let downloads_open = shared.downloads_open.clone();
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
                    // Esc closes the downloads panel first, then the find bar,
                    // then Peek.
                    if downloads_open.get() { Some("downloadsEscape".into()) }
                    else if find_open.get() { Some("findEscape".into()) }
                    else if peek_visible.get() { Some("escape".into()) }
                    else { None }
                }
                    // Ctrl+J opens the downloads panel, like Chrome.
                    else if ctrl && !alt && !shift && key == 74 { Some("downloads".into()) }
                    // Browser-standard page zoom: Ctrl+plus/minus/0, also from
                    // the OEM keys, the numpad and Ctrl+Shift+plus.
                    else if ctrl && !alt && (key == 187 || key == 107) { Some("zoomIn".into()) }
                    else if ctrl && !alt && (key == 189 || key == 109) { Some("zoomOut".into()) }
                    else if ctrl && !alt && !shift && (key == 48 || key == 96) { Some("zoomReset".into()) }
                    else if ctrl && !alt && !shift && (49..=57).contains(&key) { Some(format!("space:{}", key - 49)) }
                    else { shortcut_settings.borrow().action(&crate::settings::Shortcut {key, ctrl, alt, shift}) };
                if let Some(action) = action {
                    args.SetHandled(true)?;
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
    let download_downloads = shared.downloads.clone();
    let download_ops = shared.download_ops.clone();
    let download_proxy = proxy.clone();
    let download = DownloadStartingEventHandler::create(Box::new(move |_, args| {
        if let Some(args) = args {
            unsafe {
                let folder = settings.borrow().download_folder.clone();
                if !folder.is_empty() {
                    let mut suggested = PWSTR::null();
                    args.ResultFilePath(&mut suggested)?;
                    let suggested = PathBuf::from(take_pwstr(suggested));
                    let filename = suggested.file_name().unwrap_or(std::ffi::OsStr::new("download"));
                    let mut destination = PathBuf::from(&folder).join(filename);
                    if !std::path::Path::new(&folder).is_dir() { args.SetCancel(true)?; return Ok(()); }
                    let stem = suggested.file_stem().unwrap_or(filename).to_string_lossy();
                    let ext = suggested.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                    let mut index = 1;
                    while destination.exists() { destination = PathBuf::from(&folder).join(format!("{stem} ({index}){ext}")); index += 1; }
                    args.SetResultFilePath(&HSTRING::from(destination.as_os_str()))?;
                }
                // Track every download Zigzag-style: a row appears at once and
                // the panel is revealed; progress and state arrive by events.
                let mut path_pointer = PWSTR::null();
                args.ResultFilePath(&mut path_pointer)?;
                let path = PathBuf::from(take_pwstr(path_pointer));
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
                track_download(
                    &operation,
                    id.clone(),
                    download_downloads.clone(),
                    download_ops.clone(),
                    download_proxy.clone(),
                )?;
                let _ = download_proxy.send_event(Event::DownloadStarted(id));
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
                if let Some(point) = peek_target(&target) {
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
    // Per-page zoom, whether changed by our ladder, WebView2's own Ctrl+wheel
    // or a pinch: the host remembers it for the site and the UI shows it.
    let zoom = ZoomFactorChangedEventHandler::create(Box::new(move |sender, _| {
        if let Some(sender) = sender {
            if let Ok(factor) = crate::site::zoom_factor_controller(&sender) {
                let _ = zoom_proxy.send_event(Event::Zoom(zoom_id.clone(), factor));
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
        unsafe {
        let Some(sender) = sender else { return Ok(()) };
        let mut code = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
        sender.State(&mut code)?;
        let mut received = 0i64;
        let _ = sender.BytesReceived(&mut received);
        let mut total = 0i64;
        let _ = sender.TotalBytesToReceive(&mut total);
        if let Some(item) = state_downloads.borrow_mut().get_mut(&state_id) {
            item.received = received.max(0) as u64;
            if total > 0 {
                item.total = total as u64;
            }
        }
        if code == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
            let mut path_pointer = PWSTR::null();
            if sender.ResultFilePath(&mut path_pointer).is_ok() {
                let path = take_pwstr(path_pointer);
                if let Some(item) = state_downloads.borrow_mut().get_mut(&state_id) {
                    if !path.is_empty() {
                        item.path = path;
                        if let Some(name) = FilePath::new(&item.path).file_name() {
                            item.filename = name.to_string_lossy().into_owned();
                        }
                    }
                }
            }
            state_downloads
                .borrow_mut()
                .finish(&state_id, DownloadState::Finished, None);
            state_ops.borrow_mut().remove(&state_id);
            let _ = state_proxy.send_event(Event::DownloadsChanged { persist: true });
        } else if code == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
            let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE;
            let _ = sender.InterruptReason(&mut reason);
            let current = state_downloads.borrow().get(&state_id).map(|item| item.state);
            if current == Some(DownloadState::Cancelled) || reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED {
                state_downloads
                    .borrow_mut()
                    .finish(&state_id, DownloadState::Cancelled, None);
                state_ops.borrow_mut().remove(&state_id);
            } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED
                || current == Some(DownloadState::Paused)
            {
                // Pausing interrupts the operation underneath; the row keeps its
                // bytes and turns into "paused" until Resume.
                if let Some(item) = state_downloads.borrow_mut().get_mut(&state_id) {
                    item.state = DownloadState::Paused;
                }
            } else {
                let failure = crate::downloads::interrupt_reason_text(reason);
                state_downloads
                    .borrow_mut()
                    .finish(&state_id, DownloadState::Failed, Some(failure));
                state_ops.borrow_mut().remove(&state_id);
            }
            let _ = state_proxy.send_event(Event::DownloadsChanged { persist: true });
        } else {
            let _ = state_proxy.send_event(Event::DownloadsChanged { persist: false });
        }
        Ok(())
        }
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
