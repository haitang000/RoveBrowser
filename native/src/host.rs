use crate::{
    clip::WindowClip,
    model::{self, Command, PeekPhase, Point, Rect, State, Tab},
    storage::Storage,
};
use include_dir::{include_dir, Dir};
use serde_json::json;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
};
use webview2_com::{
    take_pwstr, AcceleratorKeyPressedEventHandler, FaviconChangedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2NavigationStartingEventArgs3, ICoreWebView2NewWindowRequestedEventArgs2,
        ICoreWebView2_15, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
        COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN, COREWEBVIEW2_NAVIGATION_KIND_NEW_DOCUMENT,
    },
    NavigationStartingEventHandler, NewWindowRequestedEventHandler, SourceChangedEventHandler,
};
use windows::{
    core::{Interface, BOOL, PWSTR},
    Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::ScreenToClient,
        UI::{
            Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT},
            WindowsAndMessaging::{
                GetCursorPos, SetWindowPos, HWND_BOTTOM, HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE,
                SWP_NOSIZE,
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
    Command(Command),
    Title(String, String),
    Load(String, String, bool),
    Source(String, String),
    Favicon(String, String),
    Shortcut(String, bool),
    Open(String),
    PeekRequest(String, String, Option<Point>),
    PeekBackground(String, usize, Option<String>),
    PeekSurface(String, u64, Option<String>),
    CommandBackground(u64, usize, Option<String>),
    Suggestions(u64, Vec<String>),
    CaptureTabs,
    Thumbnail(String, String, u64, Option<String>),
}
struct Page {
    view: WebView,
    url: String,
    clip: WindowClip,
    thumbnail_version: u64,
    thumbnail_attempt: Option<u64>,
    thumbnail_pending: bool,
    visible: Cell<bool>,
    dark: Cell<bool>,
    navigation: Rc<NavigationPolicy>,
}
struct NavigationPolicy {
    bookmarked: Cell<bool>,
    current: RefCell<String>,
    host_navigation: Cell<bool>,
}
impl Page {
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
    background: [Option<String>; 2],
    background_pending: [bool; 2],
    covering: bool,
    shown: Cell<bool>,
    origin: Option<Point>,
    phase: PeekPhase,
    ticket: u64,
    surface: Option<String>,
    surface_sent: bool,
    opening_capture: bool,
    moving: bool,
}
struct App {
    // Drop child controllers before their parent HWND and shared context.
    pages: HashMap<String, Page>,
    peek: Option<Peek>,
    peek_visible: Rc<Cell<bool>>,
    caption: Option<crate::caption::Caption>,
    thumbnails: crate::thumbnails::Cache,
    ui: Option<WebView>,
    window: Option<Window>,
    window_clip: WindowClip,
    context: WebContext,
    state: State,
    storage: Storage,
    proxy: EventLoopProxy<Event>,
    primary: Rect,
    secondary: Option<Rect>,
    overlay: bool,
    command_backdrop: Option<CommandBackdrop>,
    command_ticket: u64,
    ready: bool,
    last_error: Option<String>,
    ui_data: PathBuf,
    glass: bool,
}
struct CommandBackdrop {
    ticket: u64,
    images: [Option<String>; 3],
    pending: [bool; 3],
    covering: bool,
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
    let storage = Storage::open(&data)?;
    // A malformed session is retained, and startup stops instead of overwriting it.
    let state = storage.load()?;
    let event_loop = EventLoop::<Event>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut app = App {
        pages: HashMap::new(),
        peek: None,
        peek_visible: Rc::new(Cell::new(false)),
        caption: None,
        thumbnails: crate::thumbnails::Cache::default(),
        ui: None,
        window: None,
        window_clip: WindowClip::default(),
        context: WebContext::new(Some(data.join("WebView2"))),
        state,
        storage,
        proxy,
        primary: Rect::default(),
        secondary: None,
        overlay: false,
        command_backdrop: None,
        command_ticket: 0,
        ready: false,
        last_error: None,
        ui_data: data.join("UI"),
        glass: false,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}
impl App {
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
    }
    fn page(&self) -> Option<&WebView> {
        self.peek
            .as_ref()
            .map(|p| &p.page.view)
            .or_else(|| self.pages.get(&self.state.active_tab).map(|p| &p.view))
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
        register_page_events(
            &view,
            tab.id.clone(),
            self.proxy.clone(),
            self.peek_visible.clone(),
            navigation.clone(),
        )?;
        Ok(Page {
            view,
            url: tab.url.clone(),
            clip: WindowClip::default(),
            thumbnail_version: 0,
            thumbnail_attempt: None,
            thumbnail_pending: false,
            visible: Cell::new(false),
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
        self.emit(json!({"type":"peek","peek":self.peek.as_ref().map(|p|&p.tab),"phase":self.peek.as_ref().map(|p|p.phase),"ticket":self.peek.as_ref().map(|p|p.ticket),"origin":self.peek.as_ref().and_then(|p|p.origin),"backgrounds":self.peek.as_ref().map(|p|&p.background),"backgroundReady":self.peek.as_ref().is_some_and(|p|!p.background_pending.contains(&true))}));
    }
    fn open_peek(
        &mut self,
        url: &str,
        origin: Option<Point>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        };
        let id = tab.id.clone();
        let background = self
            .peek
            .as_ref()
            .map(|p| p.background.clone())
            .unwrap_or_default();
        let reuse = self
            .peek
            .as_ref()
            .is_some_and(|p| !p.background_pending.contains(&true));
        let covering = self.peek.as_ref().is_some_and(|p| p.covering);
        let sources = if reuse {
            [None, None]
        } else {
            [
                Some(self.state.active_tab.clone()),
                self.state.active().and_then(|t| t.split_with.clone()),
            ]
        };
        let background_pending = std::array::from_fn(|pane| {
            sources[pane]
                .as_ref()
                .is_some_and(|id| self.pages.contains_key(id))
        });
        let page = self.build_page(&tab)?;
        // Replacing a Peek drops its controller, without stacking temporary tabs.
        self.peek = Some(Peek {
            tab,
            page,
            rect: None,
            background: background.clone(),
            background_pending,
            covering,
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
        for (pane, source) in sources.into_iter().enumerate() {
            if let Some(opener) = source.and_then(|id| self.pages.get(&id)) {
                let proxy = self.proxy.clone();
                let capture_id = id.clone();
                let result = crate::capture::background(&opener.view, move |image| {
                    let _ = proxy.send_event(Event::PeekBackground(capture_id, pane, image));
                });
                if result.is_err() {
                    let _ = self
                        .proxy
                        .send_event(Event::PeekBackground(id.clone(), pane, None));
                }
            }
        }
        if background_pending.contains(&true) {
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                for pane in 0..2 {
                    let _ = proxy.send_event(Event::PeekBackground(id.clone(), pane, None));
                }
            });
        }
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
        // Put the continuously rendered source HWNDs above the UI before removing its snapshots.
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
        let _ = self.storage.visit(&peek.tab.title, &peek.tab.url);
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
    fn publish_command_backdrop(&self) {
        if let Some(backdrop) = &self.command_backdrop {
            self.emit(json!({"type":"commandBackdrop","ticket":backdrop.ticket,
                "backgrounds":backdrop.images,"backgroundReady":!backdrop.pending.contains(&true)}));
        } else {
            self.emit(json!({"type":"commandBackdrop","backgrounds":null}));
        }
    }
    fn prepare_command_backdrop(&mut self) {
        if self.command_backdrop.is_some() {
            return;
        }
        self.command_ticket += 1;
        let ticket = self.command_ticket;
        let views = [
            self.pages.get(&self.state.active_tab).map(|p| &p.view),
            self.state
                .active()
                .and_then(|t| t.split_with.as_ref())
                .and_then(|id| self.pages.get(id))
                .map(|p| &p.view),
            self.peek
                .as_ref()
                .filter(|p| p.shown.get())
                .map(|p| &p.page.view),
        ];
        let pending = views.each_ref().map(|view| view.is_some());
        self.command_backdrop = Some(CommandBackdrop {
            ticket,
            images: [None, None, None],
            pending,
            covering: false,
        });
        for (pane, view) in views.into_iter().enumerate() {
            if let Some(view) = view {
                let proxy = self.proxy.clone();
                if crate::capture::background(view, move |image| {
                    let _ = proxy.send_event(Event::CommandBackground(ticket, pane, image));
                })
                .is_err()
                {
                    let _ = self
                        .proxy
                        .send_event(Event::CommandBackground(ticket, pane, None));
                }
            }
        }
        self.publish_command_backdrop();
        if pending.contains(&true) {
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                for pane in 0..3 {
                    let _ = proxy.send_event(Event::CommandBackground(ticket, pane, None));
                }
            });
        }
    }
    fn fail_command_backdrop(&mut self) {
        self.command_backdrop = None;
        self.overlay = false;
        if let Err(e) = self.sync_layout() {
            self.error(e);
        }
        self.publish_command_backdrop();
        self.emit(json!({"type":"commandDismissed"}));
        self.error("无法准备命令栏的网页背景，请重试");
        if let Some(page) = self.page() {
            let _ = page.focus();
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
        self.pages.retain(|id, _| {
            self.state
                .tabs
                .iter()
                .any(|t| &t.id == id && model::is_web(&t.url))
        });
        self.thumbnails.retain(|id| self.pages.contains_key(id));
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
    fn sync_layout(&self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window) = &self.window else {
            return Ok(());
        };
        let second = self.state.active().and_then(|t| t.split_with.as_ref());
        for (id, page) in &self.pages {
            let r = if id == &self.state.active_tab {
                Some(self.primary)
            } else if second == Some(id) {
                self.secondary
            } else {
                None
            };
            if let Some(r) = r.filter(|r| r.width > 0. && r.height > 0.) {
                page.view.set_bounds(self.bounds(r))?;
                page.clip
                    .apply(page.view.hwnd(), r.corners, window.scale_factor())?;
                // Cover rather than hide: toggling IsVisible discards/resumes native surfaces
                // and exposes a white frame when returning from the screenshot animation.
                page.set_visible(!self.overlay || self.command_backdrop.is_some())?;
            } else {
                page.set_visible(false)?;
            }
        }
        if let Some(peek) = &self.peek {
            if let Some(r) = peek.rect.filter(|r| r.width > 0. && r.height > 0.) {
                let visible = (!self.overlay || self.command_backdrop.is_some())
                    && (peek.phase == PeekPhase::Open
                        || (peek.phase == PeekPhase::Closing && !peek.moving && peek.shown.get()));
                let was_visible = peek.shown.replace(visible);
                peek.page.view.set_bounds(self.bounds(r))?;
                peek.page
                    .clip
                    .apply(peek.page.view.hwnd(), r.corners, window.scale_factor())?;
                peek.page.set_visible(visible)?;
                if !was_visible && visible {
                    peek.page.view.focus()?;
                }
            } else {
                peek.shown.set(false);
                peek.page.set_visible(false)?;
            }
        }
        if let Some(ui) = &self.ui {
            let command_covering = self.command_backdrop.as_ref().is_some_and(|b| b.covering);
            let covering = command_covering || self.peek.as_ref().is_some_and(|p| p.covering);
            unsafe {
                SetWindowPos(
                    ui.hwnd(),
                    Some(if covering { HWND_TOP } else { HWND_BOTTOM }),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
                )?;
                if let Some(p) = self
                    .peek
                    .as_ref()
                    .filter(|p| p.shown.get() && !command_covering)
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
        Ok(())
    }
    fn shortcut(&mut self, key: String, shift: bool) {
        match key.as_str() {
            "address" | "new" | "desk" => {
                // Address/new commands capture the still-visible page after Vue opens
                // the floating panel. Hiding it here would capture an empty surface.
                if key == "desk" {
                    self.overlay = true;
                    let _ = self.sync();
                }
                if let Some(ui) = &self.ui {
                    let _ = ui.focus();
                }
                self.emit(json!({"type":"shortcut","action":key}));
            }
            "cycle" => {
                let ids = self.state.navigable_ids();
                if !ids.is_empty() {
                    let i = ids
                        .iter()
                        .position(|id| id == &self.state.active_tab)
                        .unwrap_or(0);
                    let next = if shift {
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
            "escape" => self.request_peek_close(),
            _ => {}
        }
    }
    fn command(&mut self, c: Command, event_loop: Option<&ActiveEventLoop>) {
        let focus_after_transition = self.peek.is_some()
            && matches!(
                c,
                Command::New { .. }
                    | Command::Activate { .. }
                    | Command::OpenBookmark { .. }
                    | Command::Close { .. }
                    | Command::CloseOthers
                    | Command::CloseBelow
                    | Command::SwitchSpace { .. }
                    | Command::CreateSpace { .. }
                    | Command::Restore
                    | Command::Move { .. }
            );
        match c {
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
                        && !p.background_pending.contains(&true)
                        && (p.phase == PeekPhase::Opening || p.surface_sent)
                }) {
                    p.covering = true;
                    p.moving = true;
                    if let Err(e) = self.sync() {
                        self.error(e)
                    }
                    if let Some(ui) = &self.ui {
                        let _ = ui.focus();
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
                match model::normalize(&url).and_then(|url| {
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
                if let Err(e) = self.sync() {
                    self.error(e);
                }
                return;
            }
            Command::Presented => {
                if self.ready && self.primary.width > 0. && self.primary.height > 0. {
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
            Command::Layout { primary, secondary } => {
                if primary.valid() && secondary.is_none_or(|r| r.valid()) {
                    self.primary = primary;
                    self.secondary = secondary;
                } else {
                    return;
                }
                if let Err(e) = self.sync_layout() {
                    self.error(e);
                }
                return;
            }
            Command::Overlay { visible, command } => {
                let clearing = self.command_backdrop.is_some() && !(visible && command);
                if visible && command {
                    self.prepare_command_backdrop();
                } else {
                    self.command_backdrop = None;
                }
                if visible && !self.overlay && !command {
                    self.refresh_visible_thumbnails();
                }
                self.overlay = visible;
                if let Err(e) = self.sync_layout() {
                    self.error(e);
                }
                // Restore live sibling HWNDs before Vue removes the covering pixels.
                if clearing {
                    self.publish_command_backdrop();
                }
                if visible {
                    if let Some(ui) = &self.ui {
                        let _ = ui.focus();
                    }
                } else if let Some(page) = self.page() {
                    let _ = page.focus();
                }
                return;
            }
            Command::FloatingBackdropReady { ticket } => {
                if let Some(backdrop) = self
                    .command_backdrop
                    .as_mut()
                    .filter(|b| b.ticket == ticket && !b.pending.contains(&true))
                {
                    backdrop.covering = true;
                    if let Err(e) = self.sync_layout() {
                        self.error(e);
                    }
                    if let Some(ui) = &self.ui {
                        let _ = ui.focus();
                    }
                    self.emit(json!({"type":"commandPresented","ticket":ticket}));
                }
                return;
            }
            Command::FloatingBackdropFailed { ticket } => {
                if self
                    .command_backdrop
                    .as_ref()
                    .is_some_and(|b| b.ticket == ticket)
                {
                    self.fail_command_backdrop();
                }
                return;
            }
            Command::Window { action } => {
                if let Some(w) = &self.window {
                    match action.as_str() {
                        "close" => {
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
                        | Command::CloseOthers
                        | Command::CloseBelow
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
            let dark = self.state.theme == "dark";
            let initialization = format!(
                "document.documentElement.dataset.native='true';window.__ERA_NATIVE__=true;window.__ERA_INITIAL_STATE__={};",
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
                    if let Ok(c)=serde_json::from_str::<Command>(req.body()){let _=proxy.send_event(Event::Command(c));}
                }).build_as_child(&window)?;
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
            Event::CommandBackground(ticket, pane, image) => {
                if let Some(backdrop) = self
                    .command_backdrop
                    .as_mut()
                    .filter(|b| b.ticket == ticket && b.pending[pane])
                {
                    backdrop.pending[pane] = false;
                    if let Some(image) = image {
                        backdrop.images[pane] = Some(image);
                        if !backdrop.pending.contains(&true) {
                            self.publish_command_backdrop();
                        }
                    } else {
                        self.fail_command_backdrop();
                    }
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
            Event::PeekBackground(id, pane, background) => {
                if let Some(p) = self
                    .peek
                    .as_mut()
                    .filter(|p| p.tab.id == id && p.background_pending[pane])
                {
                    p.background_pending[pane] = false;
                    if let Some(background) = background {
                        p.background[pane] = Some(background.clone());
                        let ready = !p.background_pending.contains(&true);
                        self.emit(json!({"type":"peekBackground","id":id,"pane":pane,"background":background,"backgroundReady":ready}));
                    } else {
                        // Keep the live page on capture failure; never reveal an empty backdrop.
                        self.close_peek();
                        self.error("无法准备 Peek 背景，请重试");
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
                    if model::is_web(&t.url) {
                        let _ = self.storage.visit(&t.title, &t.url);
                    }
                }
                self.publish();
                self.save();
            }
            Event::Load(id, url, loading) => {
                if let Some(p) = self.peek.as_mut().filter(|p| p.tab.id == id) {
                    p.tab.loading = loading;
                    if model::is_web(&url) {
                        p.tab.url = url.clone();
                        p.page.url = url;
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
                        t.url = url.clone();
                        if let Some(p) = self.pages.get_mut(&id) {
                            p.url = url;
                        }
                    }
                    if !loading {
                        let _ = self.storage.visit(&t.title, &t.url);
                    }
                }
                self.publish();
                if !loading {
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
                        t.url = url.clone();
                    }
                    if let Some(p) = self.pages.get_mut(&id) {
                        p.url = url;
                    }
                    self.publish();
                }
            }
            Event::Favicon(id, url) => {
                if let Some(p) = self.peek.as_mut().filter(|p| p.tab.id == id) {
                    if model::is_web(&url) || url.starts_with("data:image/") {
                        p.tab.favicon = Some(url);
                        self.publish_peek();
                    }
                    return;
                }
                if model::is_web(&url) || url.starts_with("data:image/") {
                    if let Some(t) = self.state.tabs.iter_mut().find(|t| t.id == id) {
                        t.favicon = Some(url)
                    }
                    self.publish();
                }
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.save();
                event_loop.exit()
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Err(e) = self.update_window_frame() {
                    self.error(e);
                }
                if let (Some(ui), Some(w)) = (&self.ui, &self.window) {
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
    peek_visible: Rc<Cell<bool>>,
    navigation: Rc<NavigationPolicy>,
) -> Result<(), Box<dyn std::error::Error>> {
    let p = proxy.clone();
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
                let action = if ctrl && !alt {
                    match key {
                        76 => Some("address"),
                        84 => Some(if shift { "restore" } else { "new" }),
                        87 => Some("close"),
                        9 => Some("cycle"),
                        66 => Some("sidebar"),
                        82 => Some("reload"),
                        220 if shift => Some("desk"),
                        _ => None,
                    }
                } else if alt {
                    match key {
                        37 => Some("back"),
                        39 => Some("forward"),
                        _ => None,
                    }
                } else if key == 27 && !shift && peek_visible.get() {
                    Some("escape")
                } else {
                    None
                };
                if let Some(action) = action {
                    args.SetHandled(true)?;
                    let _ = p.send_event(Event::Shortcut(action.into(), shift));
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
    if let Ok(core) = view.webview().cast::<ICoreWebView2_15>() {
        let icon = FaviconChangedEventHandler::create(Box::new(move |sender, _| {
            if let Some(sender) = sender {
                let core = sender.cast::<ICoreWebView2_15>()?;
                let mut value = PWSTR::null();
                unsafe {
                    core.FaviconUri(&mut value)?;
                }
                let _ = proxy.send_event(Event::Favicon(id.clone(), take_pwstr(value)));
            }
            Ok(())
        }));
        unsafe {
            core.add_FaviconChanged(&icon, &mut token)?;
        }
    }
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
