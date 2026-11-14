use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const MAX_FAVORITES: usize = 8;
/// Same ladder as Zigzag's `BrowserActions.zoomSteps`.
pub const ZOOM_STEPS: [f64; 13] = [
    0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0,
];
/// Zigzag's `Tokens.Fade.start`: a regular tab starts fading after this long.
pub const TAB_FADE_AFTER_SECS: u64 = 6 * 60 * 60;

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: String,
    pub space_id: String,
    pub title: String,
    pub url: String,
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookmark_id: Option<String>,
    #[serde(default)]
    pub loading: bool,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub split_with: Option<String>,
    /// The tab's sound is muted, as Chrome and Zigzag's `setMuted`.
    #[serde(default)]
    pub muted: bool,
    /// A rename from the tab's context menu; the page title stays in `title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    /// Unix seconds the tab was last shown. Drives fading and Clear; an older
    /// session without it is refreshed by `repair` rather than treated as ancient.
    #[serde(default)]
    pub last_activated: u64,
}
/// A custom space color, from Zigzag's space editor. Absent fields keep the named color.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SpacePaint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hue2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grain: Option<f64>,
}
impl SpacePaint {
    fn cleaned(&self) -> Self {
        let unit = |value: Option<f64>| value.filter(|n| n.is_finite()).map(|n| n.clamp(0.0, 1.0));
        let tint = |value: Option<f64>| unit(value).filter(|n| *n <= 0.995);
        Self {
            hue: unit(self.hue),
            tint: tint(self.tint),
            brightness: unit(self.brightness),
            hue2: unit(self.hue2),
            tint2: tint(self.tint2),
            brightness2: unit(self.brightness2),
            angle: self.angle.filter(|n| n.is_finite()).map(|n| n.rem_euclid(360.0)),
            grain: unit(self.grain).filter(|n| *n > 0.01),
        }
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub color: String,
    /// How much of the desktop shows through the frame, 0 or absent when solid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translucency: Option<f64>,
    #[serde(flatten, default)]
    pub paint: SpacePaint,
    /// The tab this space was showing when the user left it. Switching back
    /// restores it instead of the space's first tab; absent in an older session.
    #[serde(default, rename = "activeTab", skip_serializing_if = "Option::is_none")]
    pub active_tab: Option<String>,
    /// One emoji, shown beside the name and in the switcher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    #[serde(default = "new_id")]
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub favicon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_with: Option<String>,
}
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct State {
    #[serde(default)]
    pub onboarding: crate::onboarding::Onboarding,
    pub spaces: Vec<Space>,
    pub tabs: Vec<Tab>,
    pub active_space: String,
    pub active_tab: String,
    pub favorites: Vec<Favorite>,
    #[serde(default)]
    pub pins: Vec<Favorite>,
    #[serde(default)]
    pub library_version: u8,
    /// Bumped when the cached-icon pipeline changes; older icons are fetched
    /// again once so a previously blurred cache is replaced (see `favicon`).
    #[serde(default)]
    pub icon_version: u8,
    pub closed: Vec<Tab>,
    pub theme: String,
    pub sidebar_collapsed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar_width: Option<f64>,
    #[serde(default)]
    pub settings: crate::settings::Settings,
    /// The page meets the window edges. Absent in an older session means on.
    #[serde(default = "default_borderless")]
    pub borderless: bool,
    /// Per-site page zoom, keyed by `site_key`. Absent in an older session.
    #[serde(default)]
    pub site_zoom: HashMap<String, f64>,
}
fn default_borderless() -> bool {
    true
}
/// What the sidebar's Clear button would remove right now.
#[derive(Clone, Debug, Default)]
pub struct ClearPlan {
    pub ids: Vec<String>,
    /// At least one target is stale, so the UI says "Clear old".
    pub faded: bool,
}
#[derive(Clone, Copy, Deserialize, Debug, Default)]
pub struct Rect {    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// CSS inner radii, clockwise: top-left, top-right, bottom-right, bottom-left.
    #[serde(default)]
    pub corners: [f64; 4],
}
#[derive(Clone, Copy, Deserialize, Serialize, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub fn valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && (0.0..32000.0).contains(&self.x)
            && (0.0..32000.0).contains(&self.y)
    }
}
#[derive(Clone, Copy, Serialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PeekPhase {
    Opening,
    Open,
    Closing,
}
impl Rect {
    pub fn valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|n| n.is_finite() && *n >= 0.0 && *n < 32000.0)
            && self
                .corners
                .iter()
                .all(|r| r.is_finite() && *r >= 0.0 && *r <= 256.0)
    }
}
#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Command {
    #[serde(rename = "importScan")]
    ImportScan,
    #[serde(rename = "importRun")]
    ImportRun { request: crate::import::Request },
    #[serde(rename = "importPasswords")]
    ImportPasswords,
    #[serde(rename = "onboardingOpen")]
    OnboardingOpen,
    #[serde(rename = "onboardingBack")]
    OnboardingBack { step: crate::onboarding::Step },
    #[serde(rename = "onboardingAdvance")]
    OnboardingAdvance {
        step: crate::onboarding::Step,
        #[serde(default)] skip: bool,
        #[serde(default)] sites: Vec<String>,
        #[serde(default, rename = "makeDefault")] make_default: bool,
    },
    #[serde(rename = "settingsOpen")]
    OpenSettings { pane: Option<String> },
    #[serde(rename = "settingsUpdate")]
    UpdateSettings { settings: crate::settings::Settings },
    #[serde(rename = "settingsPane")]
    SettingsPane { id: String, pane: String },
    #[serde(rename = "spaceDelete")]
    DeleteSpace { id: String },
    #[serde(rename = "spaceReorder")]
    ReorderSpace { id: String, before: String },
    #[serde(rename = "clearBrowsingData")]
    ClearBrowsingData { kind: String },
    #[serde(rename = "systemSettings")]
    SystemSettings,
    #[serde(rename = "defaultBrowserStatus")]
    DefaultBrowserStatus,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "presented")]
    Presented,
    #[serde(rename = "thumbnails")]
    Thumbnails,
    #[serde(rename = "tabPreview")]
    TabPreview {
        id: Option<String>,
        rect: Option<Rect>,
        ticket: u64,
    },
    #[serde(rename = "tabPreviewRefresh")]
    TabPreviewRefresh { id: String },
    #[serde(rename = "windowChrome")]
    WindowChrome {
        maximize: Rect,
        #[serde(rename = "viewportWidth")]
        viewport_width: f64,
        background: [u8; 4],
        hover: [u8; 4],
        foreground: [u8; 4],
    },
    #[serde(rename = "new")]
    New {
        #[serde(default)]
        url: String,
    },
    #[serde(rename = "navigate")]
    Navigate { url: String },
    #[serde(rename = "activate")]
    Activate { id: String },
    #[serde(rename = "close")]
    Close { id: Option<String> },
    #[serde(rename = "restore")]
    Restore,
    #[serde(rename = "pin")]
    Pin { id: Option<String> },
    #[serde(rename = "bookmarkOpen")]
    OpenBookmark { id: String },
    #[serde(rename = "bookmarkUnload")]
    UnloadBookmark { id: String },
    #[serde(rename = "bookmarkReset")]
    ResetBookmark { id: String },
    #[serde(rename = "bookmarkSeparate")]
    SeparateBookmark { id: String },
    #[serde(rename = "bookmarkUpdate")]
    UpdateBookmark {
        id: String,
        title: String,
        url: String,
    },
    #[serde(rename = "bookmarkUnpin")]
    UnpinBookmark {
        id: String,
        #[serde(rename = "spaceId")]
        space_id: Option<String>,
    },
    #[serde(rename = "bookmarkPin")]
    PinBookmark {
        id: String,
        #[serde(rename = "spaceId")]
        space_id: Option<String>,
    },
    #[serde(rename = "favoriteTab")]
    FavoriteTab { id: String, before: Option<String> },
    #[serde(rename = "favoriteSave")]
    SaveFavorite {
        title: String,
        url: String,
        #[serde(rename = "previousUrl")]
        previous_url: Option<String>,
    },
    #[serde(rename = "favoriteRemove")]
    RemoveFavorite { url: String },
    #[serde(rename = "favoriteMove")]
    MoveFavorite { url: String, offset: i8 },
    #[serde(rename = "move")]
    Move {
        id: String,
        #[serde(rename = "spaceId")]
        space_id: String,
    },
    #[serde(rename = "reorder")]
    Reorder { id: String, before: String },
    #[serde(rename = "space")]
    SwitchSpace { id: String },
    #[serde(rename = "spaceCreate")]
    CreateSpace {
        name: String,
        color: String,
        #[serde(default)] background: bool,
        #[serde(default)] translucency: f64,
        #[serde(default)] icon: Option<String>,
        #[serde(flatten, default)]
        paint: SpacePaint,
    },
    #[serde(rename = "spaceUpdate")]
    UpdateSpace {
        id: String,
        name: String,
        color: String,
        #[serde(default)]
        translucency: f64,
        #[serde(default)]
        icon: Option<String>,
        #[serde(flatten, default)]
        paint: SpacePaint,
    },
    #[serde(rename = "borderless")]
    Borderless,
    #[serde(rename = "glass")]
    Glass { enabled: bool },
    #[serde(rename = "theme")]
    Theme,
    #[serde(rename = "sidebar")]
    Sidebar,
    #[serde(rename = "sidebarWidth")]
    SidebarWidth { width: Option<f64> },
    #[serde(rename = "split")]
    Split { id: String, other: String },
    #[serde(rename = "unsplit")]
    Unsplit,
    #[serde(rename = "back")]
    Back,
    #[serde(rename = "forward")]
    Forward,
    #[serde(rename = "reload")]
    Reload,
    #[serde(rename = "layout")]
    Layout {
        primary: Rect,
        secondary: Option<Rect>,
        #[serde(default)]
        viewport: Option<crate::layout::Viewport>,
        #[serde(default, rename = "liveResize")]
        live_resize: bool,
    },
    #[serde(rename = "overlay")]
    Overlay {
        visible: bool,
        #[serde(default)]
        command: bool,
    },
    #[serde(rename = "popupReady")]
    PopupReady,
    #[serde(rename = "popupState")]
    PopupState { popup: serde_json::Value },
    #[serde(rename = "popupLayer")]
    PopupLayer {
        regions: Vec<Rect>, cutout: Option<Rect>, focus: bool,
        #[serde(default)]
        downloads: bool,
    },
    #[serde(rename = "window")]
    Window { action: String },
    #[serde(rename = "history")]
    History { query: String },
    #[serde(rename = "suggest")]
    Suggest { query: String, ticket: u64 },
    #[serde(rename = "closeOthers")]
    CloseOthers {
        #[serde(default)]
        id: Option<String>,
    },
    #[serde(rename = "closeBelow")]
    CloseBelow {
        #[serde(default)]
        id: Option<String>,
    },
    /// Rename a regular tab; an empty title restores the page title.
    #[serde(rename = "renameTab")]
    RenameTab { id: String, title: Option<String> },
    /// Open a new tab with the same URL right after the source tab.
    #[serde(rename = "duplicate")]
    Duplicate { id: String },
    /// Write trusted UI text (a tab URL) to the Windows clipboard.
    #[serde(rename = "copyText")]
    CopyText { text: String },
    /// Zigzag's sidebar Clear: close faded tabs, else others, else this pair.
    #[serde(rename = "clearStep")]
    ClearStep,
    /// Restore the batch the last Clear/Close Others/Close Below removed.
    #[serde(rename = "undoBatch")]
    UndoBatch,
    /// Downloads panel row action: review, pause, resume, cancel, open, reveal, remove,
    /// clearFinished or openFolder.
    #[serde(rename = "downloadAction")]
    DownloadAction {
        #[serde(default)]
        id: Option<String>,
        action: String,
    },
    /// The trusted UI tells the host whether the downloads panel is open so the
    /// page accelerator can route Esc to it.
    #[serde(rename = "downloadsPanel")]
    DownloadsPanel { open: bool },
    #[serde(rename = "findOpen")]
    FindOpen,
    #[serde(rename = "findClose")]
    FindClose,
    #[serde(rename = "findStart")]
    FindStart { query: String },
    #[serde(rename = "findNext")]
    FindNext { backwards: bool },
    /// Page zoom on the current page: "in", "out", or "reset".
    #[serde(rename = "zoom")]
    Zoom { action: String },
    /// Mute or unmute a tab (`id` is a tab id or a pinned page's bookmark id).
    #[serde(rename = "mute")]
    Mute {
        id: Option<String>,
        muted: Option<bool>,
    },
    #[serde(rename = "peekOpen")]
    PeekOpen { url: String, origin: Option<Point> },
    #[serde(rename = "peekClose")]
    PeekClose,
    #[serde(rename = "peekPromote")]
    PeekPromote,
    #[serde(rename = "peekLayout")]
    PeekLayout { id: String, rect: Rect },
    #[serde(rename = "peekMotionStart")]
    PeekMotionStart { id: String, ticket: u64 },
    #[serde(rename = "peekMotionComplete")]
    PeekMotionComplete { id: String, ticket: u64 },
    #[serde(rename = "peekBackdropFailed")]
    PeekBackdropFailed { id: String, ticket: u64 },
}
pub fn normalize(input: &str) -> Result<String, String> {
    normalize_with(input, &crate::settings::Settings::default())
}
pub fn normalize_with(input: &str, settings: &crate::settings::Settings) -> Result<String, String> {
    let text = input.trim();
    if text.is_empty() || text == "era://newtab" {
        return Ok("era://newtab".into());
    }
    if let Some(url) = crate::settings::normalize_settings_url(text) { return Ok(url); }
    let candidate = if text.starts_with("http://") || text.starts_with("https://") {
        text.to_string()
    } else if !text.contains(char::is_whitespace)
        && (text.contains('.') || text.starts_with("localhost"))
        && !text.contains("://")
        && !text.starts_with("javascript:")
    {
        format!("https://{text}")
    } else if text.contains(':') {
        return Err("仅支持 HTTP 和 HTTPS 网页".into());
    } else {
        return Ok(settings.search_url(text));
    };
    let u = url::Url::parse(&candidate).map_err(|_| "地址格式不正确")?;
    if !matches!(u.scheme(), "http" | "https")
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
    {
        return Err("网页地址不受支持".into());
    }
    Ok(u.to_string())
}
pub fn is_web(url: &str) -> bool {
    url::Url::parse(url)
        .is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
}
/// The site a page belongs to for per-site settings: the lowercased host with
/// a leading `www.` dropped, as Zigzag's `HistoryStore.normalizedHost`. IP
/// hosts and other non-domain hosts stay as they are.
pub fn site_key(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.to_lowercase();
    let site = host.strip_prefix("www.").unwrap_or(&host);
    (!site.is_empty() && site.len() <= 253).then(|| site.to_string())
}
pub fn valid_zoom(factor: f64) -> bool {
    factor.is_finite() && (0.25..=5.0).contains(&factor)
}
/// Zigzag's zoom ladder: one step up or down, or back to 100%.
pub fn next_zoom(current: f64, direction: i8) -> f64 {
    match direction {
        0 => 1.0,
        d if d > 0 => ZOOM_STEPS
            .iter()
            .find(|step| **step > current + 0.001)
            .copied()
            .unwrap_or(*ZOOM_STEPS.last().unwrap()),
        _ => ZOOM_STEPS
            .iter()
            .rev()
            .find(|step| **step < current - 0.001)
            .copied()
            .unwrap_or(ZOOM_STEPS[0]),
    }
}
pub(crate) fn favorite_url(input: &str) -> Result<String, String> {
    let text = input.trim();
    if text.is_empty() || text.chars().count() > 2048 {
        return Err("请输入有效网址（最多 2048 个字符）".into());
    }
    let candidate = if text.contains("://") {
        text.to_string()
    } else {
        format!("https://{text}")
    };
    let url =
        url::Url::parse(&candidate).map_err(|_| "请输入 HTTP 或 HTTPS 网址，例如 example.com")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("请输入 HTTP 或 HTTPS 网址，例如 example.com".into());
    }
    Ok(url.to_string())
}
fn clean_icon(icon: &Option<String>) -> Result<Option<String>, String> {
    let Some(icon) = icon else { return Ok(None) };
    let icon = icon.trim();
    if icon.is_empty() { return Ok(None) }
    if icon.chars().count() > 48 || icon.chars().any(|c| c.is_control()) {
        return Err("图标无效".into());
    }
    Ok(Some(icon.to_string()))
}
pub fn valid_color(c: &str) -> bool {
    matches!(
        c,
        "sky" | "sage" | "sand" | "rose" | "lavender" | "mint" | "peach" | "graphite"
    )
}
impl Default for State {
    fn default() -> Self {
        let favorites = [
            ("GitHub", "https://github.com"),
            ("Figma", "https://figma.com"),
            ("Notion", "https://notion.so"),
            ("Claude", "https://claude.ai"),
            ("Gmail", "https://mail.google.com"),
            ("YouTube", "https://youtube.com"),
            ("Spotify", "https://open.spotify.com"),
            ("Linear", "https://linear.app"),
        ]
        .iter()
        .map(|(t, u)| Favorite {
            id: new_id(),
            space_id: None,
            split_with: None,
            title: t.to_string(),
            url: u.to_string(),
            favicon: None,
        })
        .collect();
        let mut s = Self {
            onboarding: crate::onboarding::Onboarding::default(),
            spaces: [
                ("personal", "Personal", "sky"),
                ("work", "Work", "sage"),
                ("studio", "Studio", "lavender"),
            ]
            .iter()
            .map(|(id, n, c)| Space {
                id: id.to_string(),
                name: n.to_string(),
                color: c.to_string(),
                translucency: None,
                paint: SpacePaint::default(),
                active_tab: None,
                icon: None,
            })
            .collect(),
            tabs: vec![],
            active_space: "personal".into(),
            active_tab: String::new(),
            favorites,
            pins: vec![],
            library_version: 0,
            icon_version: 0,
            closed: vec![],
            theme: "light".into(),
            sidebar_collapsed: false,
            sidebar_width: None,
            settings: crate::settings::Settings::default(),
            borderless: true,
            site_zoom: HashMap::new(),
        };
        for (title, url) in [
            ("GitHub", "https://github.com"),
            ("Wikipedia", "https://en.wikipedia.org"),
        ] {
            s.new_tab(url).unwrap();
            let t = s.tabs.last_mut().unwrap();
            t.title = title.into();
            t.pinned = true;
        }
        s.new_tab("").unwrap();
        s.repair_bookmarks();
        s
    }
}
impl State {
    pub fn active(&self) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == self.active_tab)
    }
    pub fn activate(&mut self, id: &str) {
        let favorite = self
            .tabs
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| t.bookmark_id.as_ref())
            .is_some_and(|id| self.favorites.iter().any(|f| &f.id == id));
        if favorite {
            if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
                t.space_id = self.active_space.clone();
            }
        }
        if let Some(t) = self.tabs.iter_mut().find(|t| t.id == id) {
            self.active_tab = t.id.clone();
            self.active_space = t.space_id.clone();
            t.last_activated = now_secs();
        }
        self.remember_active();
    }
    pub fn new_tab(&mut self, input: &str) -> Result<(), String> {
        if self.tabs.len() >= 256 {
            return Err("已达到 256 个标签页上限".into());
        }
        let url = normalize_with(input, &self.settings)?;
        let title = if crate::settings::is_settings(&url) { "设置".into() } else { url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .filter(|_| url != "era://newtab")
            .unwrap_or("New Tab".into()) };
        let id = Uuid::new_v4().to_string();
        self.tabs.push(Tab {
            id: id.clone(),
            space_id: self.active_space.clone(),
            title,
            url,
            pinned: false,
            bookmark_id: None,
            loading: false,
            favicon: None,
            split_with: None,
            muted: false,
            custom_title: None,
            last_activated: now_secs(),
        });
        self.active_tab = id;
        self.remember_active();
        Ok(())
    }
    /// Remembers the tab the current space is showing, so a later switch back
    /// restores that tab rather than the space's first one.
    fn remember_active(&mut self) {
        let space_id = self.active_space.clone();
        let tab_id = self.active_tab.clone();
        if !self.tabs.iter().any(|t| t.id == tab_id) {
            return;
        }
        if let Some(space) = self.spaces.iter_mut().find(|s| s.id == space_id) {
            space.active_tab = Some(tab_id);
        }
    }
    /// The tab to show when entering a space: the one it was showing when the
    /// user left it, else its most recently shown tab.
    fn space_tab(&self, space_id: &str) -> Option<String> {
        if let Some(id) = self
            .spaces
            .iter()
            .find(|s| s.id == space_id)
            .and_then(|s| s.active_tab.clone())
        {
            // A favorite is shared by every space, so its live tab may currently
            // be assigned to another one; activating it brings it back here.
            let alive = self.tabs.iter().any(|t| {
                t.id == id
                    && (t.space_id == space_id
                        || t.bookmark_id.as_ref().is_some_and(|mark| {
                            self.favorites.iter().any(|f| &f.id == mark)
                        }))
            });
            if alive {
                return Some(id);
            }
        }
        self.most_recent_tab(space_id)
    }
    fn most_recent_tab(&self, space_id: &str) -> Option<String> {
        let mut best: Option<&Tab> = None;
        for tab in self.tabs.iter().filter(|t| t.space_id == space_id) {
            if best.is_none_or(|seen| tab.last_activated > seen.last_activated) {
                best = Some(tab);
            }
        }
        best.map(|tab| tab.id.clone())
    }
    /// Switches to a space and restores the tab it was showing, opening a blank
    /// page only when the space has no tabs left.
    fn enter_space(&mut self, space_id: &str) -> Result<(), String> {
        self.active_space = space_id.to_string();
        match self.space_tab(space_id) {
            Some(id) => {
                self.activate(&id);
                Ok(())
            }
            None => self.new_tab(""),
        }
    }
    fn partner_of(&self, id: &str) -> Option<String> {
        let tab = self.tabs.iter().find(|tab| tab.id == id)?;
        tab.split_with.clone().or_else(|| {
            self.tabs
                .iter()
                .find(|tab| tab.split_with.as_deref() == Some(id))
                .map(|tab| tab.id.clone())
        })
    }
    fn regular_pages_in(&self, space_id: &str) -> Vec<String> {
        self.tabs
            .iter()
            .filter(|tab| tab.space_id == space_id && tab.bookmark_id.is_none())
            .map(|tab| tab.id.clone())
            .collect()
    }
    /// `below` closes regular tabs under the base tab (the active one when no id
    /// is given). Otherwise it closes the other regular tabs. A pinned page
    /// treats every regular tab as both other and below. A split partner stays.
    pub fn tabs_to_dismiss(&self, below: bool, base: Option<&str>) -> Vec<String> {
        let anchor = base
            .and_then(|id| self.tabs.iter().find(|tab| tab.id == id))
            .or_else(|| self.active());
        let Some(anchor) = anchor else { return vec![] };
        let pages = self.regular_pages_in(&anchor.space_id);
        let partner = self.partner_of(&anchor.id);
        if anchor.bookmark_id.is_some() {
            return if below {
                pages
            } else {
                pages.into_iter().filter(|id| Some(id) != partner.as_ref()).collect()
            };
        }
        if !below {
            return pages
                .into_iter()
                .filter(|id| id != &anchor.id && Some(id) != partner.as_ref())
                .collect();
        }
        let last = pages
            .iter()
            .rposition(|id| id == &anchor.id || Some(id) == partner.as_ref());
        pages.into_iter().skip(last.map(|index| index + 1).unwrap_or(0)).collect()
    }
    /// Zigzag's three-step sidebar Clear. Faded regular tabs go first; while
    /// sound is playing the tab is never considered stale, as in Zigzag.
    pub fn clear_targets(&self, playing: &std::collections::HashSet<String>, now: u64) -> ClearPlan {
        let pages: Vec<&Tab> = self
            .tabs
            .iter()
            .filter(|tab| tab.space_id == self.active_space && tab.bookmark_id.is_none())
            .collect();
        let current = self.active().filter(|tab| tab.bookmark_id.is_none());
        let current_id = current.map(|tab| tab.id.clone());
        let partner = current.and_then(|tab| self.partner_of(&tab.id));
        let others: Vec<&Tab> = pages
            .iter()
            .copied()
            .filter(|tab| Some(&tab.id) != current_id.as_ref() && Some(&tab.id) != partner.as_ref())
            .collect();
        let faded: Vec<String> = others
            .iter()
            .filter(|tab| {
                now.saturating_sub(tab.last_activated) > TAB_FADE_AFTER_SECS
                    && !playing.contains(&tab.id)
            })
            .map(|tab| tab.id.clone())
            .collect();
        if !faded.is_empty() {
            return ClearPlan { ids: faded, faded: true };
        }
        if !others.is_empty() {
            return ClearPlan {
                ids: others.iter().map(|tab| tab.id.clone()).collect(),
                faded: false,
            };
        }
        // Last step: the tab being looked at and its split partner.
        let ids = current_id
            .into_iter()
            .chain(partner)
            .filter(|id| self.tabs.iter().any(|tab| &tab.id == id))
            .collect();
        ClearPlan { ids, faded: false }
    }
    pub fn close(&mut self, id: &str) {
        if let Some(i) = self.tabs.iter().position(|t| t.id == id) {
            let mut t = self.tabs.remove(i);
            t.loading = false;
            let partner = t.split_with.clone().or_else(|| {
                self.tabs
                    .iter()
                    .find(|x| x.split_with.as_deref() == Some(id))
                    .map(|x| x.id.clone())
            });
            if t.bookmark_id.is_none() {
                self.closed.push(t);
                if self.closed.len() > 30 {
                    self.closed.remove(0);
                }
            }
            for t in &mut self.tabs {
                if t.split_with.as_deref() == Some(id) {
                    t.split_with = None
                }
            }
            if self.active_tab == id {
                self.active_tab = partner
                    .filter(|id| self.tabs.iter().any(|t| &t.id == id))
                    .or_else(|| {
                        self.tabs
                            .iter()
                            .rfind(|t| t.space_id == self.active_space)
                            .map(|t| t.id.clone())
                    })
                    .unwrap_or_default();
                if self.active_tab.is_empty() {
                    let _ = self.new_tab("");
                } else if let Some(t) = self.tabs.iter_mut().find(|t| t.id == self.active_tab) {
                    // The tab that takes over the empty pane is being looked at.
                    t.last_activated = now_secs();
                }
            }
        }
        self.remember_active();
    }
    pub fn repair(&mut self) {
        // A missing removable download folder must not reset unrelated preferences.
        self.settings.repair();
        self.sidebar_width = self.sidebar_width.filter(|width| width.is_finite())
            .map(|width| width.clamp(192., 480.).round());
        // Zoom is only meaningful inside WebView2's range; a session written by
        // an older build or a damaged file must not wedge the page at 0 or 99x.
        self.site_zoom.retain(|site, zoom| {
            !site.is_empty() && site.len() <= 253 && valid_zoom(*zoom) && (*zoom - 1.0).abs() > 0.001
        });
        for tab in self.tabs.iter_mut().chain(self.closed.iter_mut()) {
            if let Some(url) = crate::settings::normalize_settings_url(&tab.url) {
                tab.url = url;
            }
        }
        if self.spaces.is_empty() {
            *self = Self::default();
            return;
        }
        if !self.spaces.iter().any(|s| s.id == self.active_space) {
            self.active_space = self.spaces[0].id.clone()
        }
        self.repair_bookmarks();
        let ids: Vec<_> = self.tabs.iter().map(|t| t.id.clone()).collect();
        let now = now_secs();
        for t in &mut self.tabs {
            t.loading = false;
            // Sessions written before tab aging have no timestamp; treat them as
            // fresh so switching to this build never sweeps real tabs away.
            if t.last_activated == 0 || t.last_activated > now + 86_400 {
                t.last_activated = now;
            }
            if !self.spaces.iter().any(|s| s.id == t.space_id) {
                t.space_id = self.active_space.clone()
            }
            if !is_web(&t.url) && !crate::settings::is_settings(&t.url) {
                t.url = "era://newtab".into()
            }
            if t.split_with
                .as_ref()
                .is_some_and(|id| id == &t.id || !ids.contains(id))
            {
                t.split_with = None
            }
        }
        if let Some(t) = self.active().cloned() {
            self.active_space = t.space_id
        } else {
            let _ = self.new_tab("");
        }
        self.remember_active();
    }
    pub fn apply(&mut self, c: &Command) -> Result<(), String> {
        if self.apply_bookmark_command(c)? {
            self.remember_active();
            return Ok(());
        }
        match c {
            Command::OnboardingOpen => { self.onboarding = crate::onboarding::Onboarding { step: crate::onboarding::Step::Hello, completed: false, imports: self.onboarding.imports.clone() }; }
            Command::OnboardingBack { step } => self.back_onboarding(*step)?,
            Command::OnboardingAdvance { step, skip, sites, .. } => self.advance_onboarding(*step, *skip, sites)?,
            Command::OpenSettings { pane } => {
                let existing = self.tabs.iter().find(|t| t.space_id == self.active_space && crate::settings::is_settings(&t.url))
                    .or_else(|| self.tabs.iter().find(|t| crate::settings::is_settings(&t.url))).map(|t| t.id.clone());
                if let Some(id) = existing {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
                        tab.url = if let Some(pane) = pane { crate::settings::settings_url(Some(pane)) }
                            else { crate::settings::normalize_settings_url(&tab.url).unwrap() };
                    }
                    self.activate(&id);
                } else { self.new_tab(&crate::settings::settings_url(pane.as_deref()))?; }
            }
            Command::SettingsPane { id, pane } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| &t.id == id && crate::settings::is_settings(&t.url)) {
                    tab.url = crate::settings::settings_url(Some(pane));
                }
            }
            Command::UpdateSettings { settings } => {
                let mut next = settings.clone(); next.validate()?; self.settings = next;
            }
            Command::ReorderSpace { id, before } => {
                if id != before && self.spaces.iter().any(|s| &s.id == before) {
                    if let Some(index) = self.spaces.iter().position(|s| &s.id == id) {
                        let space = self.spaces.remove(index);
                        let target = self.spaces.iter().position(|s| &s.id == before).unwrap();
                        self.spaces.insert(target, space);
                    }
                }
            }
            Command::DeleteSpace { id } => {
                if self.spaces.len() <= 1 { return Err("至少保留一个工作区".into()); }
                if self.spaces.iter().any(|s| &s.id == id) {
                    let removed: Vec<_> = self.tabs.iter().filter(|t| &t.space_id == id && !self.favorites.iter().any(|f| Some(&f.id) == t.bookmark_id.as_ref())).map(|t| t.id.clone()).collect();
                    self.spaces.retain(|s| &s.id != id);
                    self.pins.retain(|p| p.space_id.as_ref() != Some(id));
                    if &self.active_space == id { self.active_space = self.spaces[0].id.clone(); }
                    for t in &mut self.tabs {
                        if &t.space_id == id {
                            t.space_id = self.active_space.clone();
                        }
                        if t.split_with.as_ref().is_some_and(|id| removed.contains(id)) {
                            t.split_with = None;
                        }
                    }
                    self.tabs.retain(|t| !removed.contains(&t.id));
                    self.closed.retain(|t| &t.space_id != id);
                    if !self.tabs.iter().any(|t| t.id == self.active_tab) {
                        // Deleting the current space lands in the first remaining
                        // one, which restores the tab it was showing.
                        self.active_tab.clear();
                        let space = self.active_space.clone();
                        self.enter_space(&space)?;
                    }
                }
            }
            Command::New { url } => self.new_tab(url)?,
            Command::Navigate { url } => {
                let u = normalize_with(url, &self.settings)?;
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == self.active_tab) {
                    t.url = u;
                    if crate::settings::is_settings(&t.url) { t.title = "设置".into(); }
                    t.favicon = None
                }
            }
            Command::Activate { id } => {
                if self.bookmark(id).is_some() {
                    self.open_bookmark(id)?;
                } else {
                    self.activate(id);
                }
            }
            Command::Close { id } => self.close(&id.clone().unwrap_or(self.active_tab.clone())),
            Command::CloseOthers { id } => {
                let base = id.clone().or_else(|| Some(self.active_tab.clone()));
                let targets = self.tabs_to_dismiss(false, base.as_deref());
                for id in targets { self.close(&id); }
            }
            Command::CloseBelow { id } => {
                let base = id.clone().or_else(|| Some(self.active_tab.clone()));
                let targets = self.tabs_to_dismiss(true, base.as_deref());
                for id in targets { self.close(&id); }
            }
            Command::RenameTab { id, title } => {
                let value = title.clone().unwrap_or_default().trim().to_string();
                if value.chars().count() > 80 { return Err("标签名称最多 80 个字符".into()); }
                if let Some(tab) = self.tabs.iter_mut().find(|tab| &tab.id == id && tab.bookmark_id.is_none()) {
                    tab.custom_title = (!value.is_empty()).then_some(value);
                }
            }
            Command::Duplicate { id } => {
                let Some(source) = self.tabs.iter().find(|tab| &tab.id == id && tab.bookmark_id.is_none()).cloned() else { return Ok(()); };
                let index = self.tabs.iter().position(|tab| tab.id == source.id).unwrap_or(0);
                let space = source.space_id.clone();
                self.new_tab(&source.url)?;
                let Some(mut duplicate) = self.tabs.pop() else { return Ok(()); };
                duplicate.space_id = space.clone();
                duplicate.title = source.title;
                duplicate.custom_title = source.custom_title;
                duplicate.muted = source.muted;
                duplicate.last_activated = now_secs();
                let position = (index + 1).min(self.tabs.len());
                self.tabs.insert(position, duplicate.clone());
                self.active_space = space;
                self.active_tab = duplicate.id;
            }
            Command::Restore => {
                if let Some(mut t) = self.closed.pop() {
                    t.split_with = None;
                    t.pinned = false;
                    t.bookmark_id = None;
                    let id = t.id.clone();
                    if !self.spaces.iter().any(|s| s.id == t.space_id) {
                        t.space_id = self.active_space.clone()
                    }
                    t.last_activated = now_secs();
                    self.tabs.push(t);
                    self.activate(&id)
                }
            }
            Command::SaveFavorite {
                title,
                url,
                previous_url,
            } => {
                let url = favorite_url(url)?;
                let title = if title.trim().is_empty() {
                    url::Url::parse(&url)
                        .unwrap()
                        .host_str()
                        .unwrap()
                        .to_string()
                } else {
                    title.trim().to_string()
                };
                if title.chars().count() > 80 {
                    return Err("网站名称最多 80 个字符".into());
                }
                let index = if let Some(previous) = previous_url {
                    Some(
                        self.favorites
                            .iter()
                            .position(|f| &f.url == previous)
                            .ok_or("该常用网站已被移除")?,
                    )
                } else {
                    None
                };
                if self
                    .favorites
                    .iter()
                    .enumerate()
                    .any(|(i, f)| Some(i) != index && favorite_url(&f.url).is_ok_and(|u| u == url))
                {
                    return Err("该网址已在顶部常用网站中".into());
                }
                let favorite = Favorite {
                    id: index
                        .map(|i| self.favorites[i].id.clone())
                        .unwrap_or_else(new_id),
                    title,
                    favicon: index.and_then(|i| {
                        crate::favicon::same_origin(&self.favorites[i].url, &url)
                            .then(|| self.favorites[i].favicon.clone())
                            .flatten()
                    }),
                    url,
                    space_id: None,
                    split_with: index.and_then(|i| self.favorites[i].split_with.clone()),
                };
                if let Some(i) = index {
                    self.favorites[i] = favorite;
                } else {
                    if self.favorites.len() >= MAX_FAVORITES {
                        return Err("最多置顶 8 个网站".into());
                    }
                    self.favorites.push(favorite);
                }
            }
            Command::RemoveFavorite { url } => {
                if let Some(f) = self.favorites.iter().find(|f| &f.url == url).cloned() {
                    self.remove_bookmark(&f.id);
                }
            }
            Command::Move { id, space_id } => {
                if self.spaces.iter().any(|s| &s.id == space_id) {
                    for t in &mut self.tabs {
                        if &t.id == id {
                            t.space_id = space_id.clone();
                            t.split_with = None
                        }
                        if t.split_with.as_ref() == Some(id) {
                            t.split_with = None
                        }
                    }
                    if &self.active_tab == id {
                        self.active_space = space_id.clone()
                    }
                }
            }
            Command::SwitchSpace { id } => {
                if self.spaces.iter().any(|s| &s.id == id) {
                    // Keep what the space we are leaving is showing, then restore
                    // what the target space was showing when it was left.
                    self.remember_active();
                    self.enter_space(id)?;
                }
            }
            Command::CreateSpace { name, color, background, translucency, icon, paint } => {
                if name.trim().is_empty() { return Err("请输入工作区名称".into()); }
                if !valid_color(color) {
                    return Err("未知工作区颜色".into());
                }
                if self.spaces.len() >= 16 {
                    return Err("最多创建 16 个工作区".into());
                }
                let id = Uuid::new_v4().to_string();
                self.spaces.push(Space {
                    id: id.clone(),
                    name: name.trim().chars().take(40).collect(),
                    color: color.clone(),
                    translucency: (0.005..=1.0).contains(translucency).then_some(*translucency),
                    paint: paint.cleaned(),
                    active_tab: None,
                    icon: clean_icon(icon)?,
                });
                if !background { self.active_space = id; self.new_tab("")?; }
            }
            Command::UpdateSpace {
                id,
                name,
                color,
                translucency,
                icon,
                paint,
            } => {
                if !valid_color(color) {
                    return Err("未知工作区颜色".into());
                }
                let icon = clean_icon(icon)?;
                if let Some(s) = self.spaces.iter_mut().find(|s| &s.id == id) {
                    if !name.trim().is_empty() {
                        s.name = name.chars().take(40).collect()
                    }
                    s.color = color.clone();
                    s.translucency = (0.005..=1.0).contains(translucency).then_some(*translucency);
                    s.paint = paint.cleaned();
                    s.icon = icon;
                }
            }
            Command::Borderless => self.borderless = !self.borderless,
            Command::Theme => {
                self.theme = if self.theme == "dark" {
                    "light"
                } else {
                    "dark"
                }
                .into()
            }
            Command::Sidebar => self.sidebar_collapsed = !self.sidebar_collapsed,
            Command::SidebarWidth { width } => {
                if width.is_some_and(|width| !width.is_finite()) {
                    return Err("侧栏宽度无效".into());
                }
                self.sidebar_width = width.map(|width| width.clamp(192., 480.).round());
            }
            Command::Split { id, other } => {
                if id != other
                    && self
                        .tabs
                        .iter()
                        .find(|t| &t.id == id)
                        .zip(self.tabs.iter().find(|t| &t.id == other))
                        .is_some_and(|(a, b)| a.space_id == b.space_id)
                {
                    if let Some(t) = self.tabs.iter_mut().find(|t| &t.id == id) {
                        t.split_with = Some(other.clone())
                    }
                }
            }
            Command::Unsplit => {
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == self.active_tab) {
                    t.split_with = None
                }
            }
            _ => {}
        }
        self.remember_active();
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restrict_schemes() {
        for u in [
            "javascript:alert(1)",
            "file:///C:/Windows",
            "era://ui",
            "https://user:pass@example.com",
        ] {
            assert!(normalize(u).is_err(), "{u}")
        }
        assert!(normalize("example.com")
            .unwrap()
            .starts_with("https://example.com"));
    }
    #[test]
    fn favorites_validate_and_update_atomically() {
        let mut s = State::default();
        let before = serde_json::to_string(&s.favorites).unwrap();
        for url in [
            "",
            "search terms",
            "file:///C:/Windows",
            "javascript:alert(1)",
            "era://newtab",
            "https://user:pass@example.com",
            "https://GITHUB.com:443/",
        ] {
            assert!(
                s.apply(&Command::SaveFavorite {
                    title: "Invalid".into(),
                    url: url.into(),
                    previous_url: None,
                })
                .is_err(),
                "{url}"
            );
        }
        assert_eq!(serde_json::to_string(&s.favorites).unwrap(), before);
        let previous_url = s.favorites[0].url.clone();
        s.apply(&Command::SaveFavorite {
            title: " 文档 ".into(),
            url: "example.com/docs".into(),
            previous_url: Some(previous_url),
        })
        .unwrap();
        assert_eq!(s.favorites[0].title, "文档");
        assert_eq!(s.favorites[0].url, "https://example.com/docs");
        s.apply(&Command::MoveFavorite {
            url: "https://example.com/docs".into(),
            offset: 1,
        })
        .unwrap();
        assert_eq!(s.favorites[1].title, "文档");
        s.apply(&Command::RemoveFavorite {
            url: "https://example.com/docs".into(),
        })
        .unwrap();
        assert_eq!(s.favorites.len(), 7);
    }
    #[test]
    fn split_close_restore() {
        let mut s = State::default();
        s.new_tab("https://example.com/first").unwrap();
        let a = s.active_tab.clone();
        s.new_tab("https://example.com/second").unwrap();
        let b = s.active_tab.clone();
        s.apply(&Command::Split {
            id: a.clone(),
            other: b.clone(),
        })
        .unwrap();
        s.close(&b);
        assert!(s
            .tabs
            .iter()
            .find(|t| t.id == a)
            .unwrap()
            .split_with
            .is_none());
        s.apply(&Command::Restore).unwrap();
        assert_eq!(s.active_tab, b);
    }
    #[test]
    fn moving_breaks_split() {
        let mut s = State::default();
        let a = s.tabs[0].id.clone();
        let b = s.tabs[1].id.clone();
        s.apply(&Command::Split {
            id: a.clone(),
            other: b.clone(),
        })
        .unwrap();
        s.apply(&Command::Move {
            id: b,
            space_id: "work".into(),
        })
        .unwrap();
        assert!(s
            .tabs
            .iter()
            .find(|t| t.id == a)
            .unwrap()
            .split_with
            .is_none());
    }
    #[test]
    fn switching_spaces_restores_the_tab_each_space_was_showing() {
        let mut s = State::default();
        s.new_tab("https://example.com/docs").unwrap();
        let personal = s.active_tab.clone();
        s.apply(&Command::SwitchSpace { id: "work".into() }).unwrap();
        let work = s.active_tab.clone();
        s.new_tab("https://example.com/work-notes").unwrap();
        let notes = s.active_tab.clone();
        // Leaving a space remembers the tab it was showing, not its first tab.
        s.apply(&Command::SwitchSpace { id: "personal".into() }).unwrap();
        assert_eq!(s.active_tab, personal);
        s.apply(&Command::SwitchSpace { id: "work".into() }).unwrap();
        assert_eq!(s.active_tab, notes);
        // A remembered tab that no longer belongs to the space falls back to the
        // most recently shown one instead of the first tab.
        s.apply(&Command::SwitchSpace { id: "personal".into() }).unwrap();
        s.apply(&Command::Move {
            id: notes,
            space_id: "studio".into(),
        })
        .unwrap();
        s.apply(&Command::SwitchSpace { id: "work".into() }).unwrap();
        assert_eq!(s.active_tab, work);
        // Favorites are shared, so a space still restores one that was opened
        // elsewhere after it was left here.
        let favorite = s.favorites[0].id.clone();
        s.apply(&Command::Activate {
            id: favorite.clone(),
        })
        .unwrap();
        let favorite_tab = s.active_tab.clone();
        s.apply(&Command::SwitchSpace { id: "personal".into() }).unwrap();
        assert_ne!(s.active_tab, favorite_tab);
        s.apply(&Command::Activate {
            id: favorite.clone(),
        })
        .unwrap();
        s.apply(&Command::SwitchSpace { id: "work".into() }).unwrap();
        assert_eq!(s.active_tab, favorite_tab);
        assert_eq!(s.active_space, "work");
    }
    #[test]
    fn spaces_without_a_remembered_tab_fall_back_to_the_most_recent_page() {
        // Sessions written before spaces remembered their tab still restore the
        // most recently shown page rather than the first one.
        let mut s = State::default();
        s.tabs.push(tab_for_test("studio-old", "studio", 10));
        s.tabs.push(tab_for_test("studio-recent", "studio", 20));
        s.apply(&Command::SwitchSpace { id: "studio".into() }).unwrap();
        assert_eq!(s.active_tab, "studio-recent");
    }
    #[test]
    fn close_last_creates_blank() {
        let mut s = State::default();
        let ids: Vec<_> = s.tabs.iter().map(|t| t.id.clone()).collect();
        for id in ids {
            s.close(&id)
        }
        assert_eq!(s.tabs.len(), 1);
        assert_eq!(s.active().unwrap().url, "era://newtab");
    }
    #[test]
    fn ipc_is_typed() {
        assert!(serde_json::from_str::<Command>(r#"{"type":"exec","cmd":"anything"}"#).is_err());
        assert!(!Rect {
            x: f64::NAN,
            ..Rect::default()
        }
        .valid());
        assert!(!Rect {
            corners: [f64::NAN, 0., 0., 0.],
            ..Rect::default()
        }
        .valid());
    }
    #[test]
    fn close_others_and_below_keep_pins_and_the_current_page() {
        let mut s = State::default();
        s.new_tab("https://alpha.example").unwrap();
        s.new_tab("https://beta.example").unwrap();
        let beta = s.active_tab.clone();
        let alpha = s.tabs.iter().find(|tab| tab.url.contains("alpha")).unwrap().id.clone();
        s.activate(&alpha);
        s.apply(&Command::CloseBelow { id: None }).unwrap();
        assert!(s.tabs.iter().all(|tab| tab.id != beta));
        assert!(s.tabs.iter().any(|tab| tab.id == alpha));
        assert!(s.tabs.iter().any(|tab| tab.bookmark_id.is_some()));
        s.new_tab("https://gamma.example").unwrap();
        s.activate(&alpha);
        s.apply(&Command::CloseOthers { id: None }).unwrap();
        assert_eq!(s.active_tab, alpha);
        assert!(s.tabs.iter().filter(|tab| tab.bookmark_id.is_none()).all(|tab| tab.id == alpha));
    }
    #[test]
    fn zoom_sites_are_normalized_and_use_zigzags_ladder() {
        assert_eq!(site_key("https://www.Example.COM/a?b=1").as_deref(), Some("example.com"));
        assert_eq!(site_key("http://example.com:8080/").as_deref(), Some("example.com"));
        assert_eq!(site_key("http://127.0.0.1:8080/").as_deref(), Some("127.0.0.1"));
        assert_eq!(site_key("https://sub.example.com/").as_deref(), Some("sub.example.com"));
        assert_eq!(site_key("era://newtab"), None);
        assert_eq!(site_key("not a url"), None);
        assert_eq!(next_zoom(1.0, 1), 1.1);
        assert_eq!(next_zoom(1.0, -1), 0.9);
        assert_eq!(next_zoom(1.1, -1), 1.0);
        assert_eq!(next_zoom(0.67, 1), 0.75);
        assert_eq!(next_zoom(3.0, 1), 3.0);
        assert_eq!(next_zoom(0.5, -1), 0.5);
        assert_eq!(next_zoom(2.5, 0), 1.0);
        assert!(valid_zoom(3.0) && !valid_zoom(0.2) && !valid_zoom(f64::NAN));
    }
    #[test]
    fn repaired_sessions_drop_invalid_zooms_and_keep_mute_state() {
        let mut state = State::default();
        state.site_zoom.insert("example.com".into(), 1.25);
        state.site_zoom.insert("one.example".into(), 1.0);
        state.site_zoom.insert("huge.example".into(), 9.0);
        state.site_zoom.insert("tiny.example".into(), 0.01);
        state.site_zoom.insert("".into(), 1.5);
        state.site_zoom.insert("nan.example".into(), f64::NAN);
        state.tabs[0].muted = true;
        state.repair();
        assert_eq!(state.site_zoom.len(), 1);
        assert_eq!(state.site_zoom.get("example.com"), Some(&1.25));
        assert!(state.tabs[0].muted);
        let mut restored: State =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        restored.repair();
        assert_eq!(restored.site_zoom.len(), 1);
        assert!(restored.tabs[0].muted);
        // A session written before mute and zoom existed still opens.
        let mut value: serde_json::Value = serde_json::to_value(&restored).unwrap();
        value["tabs"][0].as_object_mut().unwrap().remove("muted");
        value.as_object_mut().unwrap().remove("siteZoom");
        let old: State = serde_json::from_value(value).unwrap();
        assert!(!old.tabs[0].muted);
        assert!(old.site_zoom.is_empty());
    }
    fn tab_for_test(id: &str, space: &str, last: u64) -> Tab {
        Tab {
            id: id.into(),
            space_id: space.into(),
            title: id.into(),
            url: format!("https://{id}.example/"),
            pinned: false,
            bookmark_id: None,
            loading: false,
            favicon: None,
            split_with: None,
            muted: false,
            custom_title: None,
            last_activated: last,
        }
    }
    #[test]
    fn clear_ladder_prefers_faded_tabs_and_never_stale_audio() {
        let now = 1_000_000;
        let old = now - TAB_FADE_AFTER_SECS - 5;
        let mut state = State::default();
        state.tabs.clear();
        state.active_space = "personal".into();
        state.active_tab = "current".into();
        state.tabs.push(tab_for_test("current", "personal", now));
        state.tabs.push(tab_for_test("old1", "personal", old));
        state.tabs.push(tab_for_test("old2", "personal", old));
        state.tabs.push(tab_for_test("fresh", "personal", now));
        // A tab playing sound is never swept away, even when it is old.
        let mut playing: std::collections::HashSet<String> = ["old2".to_string()].into();
        let plan = state.clear_targets(&playing, now);
        assert_eq!(plan.ids, vec!["old1"]);
        assert!(plan.faded);
        playing.clear();
        assert_eq!(state.clear_targets(&playing, now).ids, vec!["old1", "old2"]);
        // Nothing faded: Clear moves on to the other tabs.
        for tab in &mut state.tabs[1..] {
            tab.last_activated = now;
        }
        let plan = state.clear_targets(&playing, now);
        assert!(!plan.faded);
        assert_eq!(plan.ids, vec!["old1", "old2", "fresh"]);
        // The split partner stays with the current tab until the last step.
        state.tabs[1].split_with = Some("current".into());
        state.tabs[0].split_with = Some("old1".into());
        assert_eq!(state.clear_targets(&playing, now).ids, vec!["old2", "fresh"]);
        state.tabs.retain(|tab| tab.id == "current" || tab.id == "old1");
        let plan = state.clear_targets(&playing, now);
        assert_eq!(plan.ids, vec!["current", "old1"]);
        assert!(!plan.faded);
    }
    #[test]
    fn renaming_duplicating_and_explicit_close_targets_work() {
        let mut state = State::default();
        state.tabs.clear();
        state.active_space = "personal".into();
        state.active_tab = "a".into();
        state.tabs.push(tab_for_test("a", "personal", 1));
        state.tabs.push(tab_for_test("b", "personal", 1));
        state
            .apply(&Command::RenameTab {
                id: "a".into(),
                title: Some("  我的标签  ".into()),
            })
            .unwrap();
        assert_eq!(state.tabs[0].custom_title.as_deref(), Some("我的标签"));
        assert!(state
            .apply(&Command::RenameTab {
                id: "a".into(),
                title: Some("x".repeat(81)),
            })
            .is_err());
        state
            .apply(&Command::RenameTab {
                id: "a".into(),
                title: Some("  ".into()),
            })
            .unwrap();
        assert_eq!(state.tabs[0].custom_title, None);
        state.apply(&Command::Duplicate { id: "a".into() }).unwrap();
        assert_eq!(state.tabs.len(), 3);
        let duplicate = state.tabs[1].clone();
        assert_eq!(duplicate.url, state.tabs[0].url);
        assert_eq!(state.active_tab, duplicate.id);
        assert_eq!(state.tabs[2].id, "b");
        // An explicit base tab lets the menu target a background tab.
        state.apply(&Command::CloseOthers { id: Some("b".into()) }).unwrap();
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.tabs[0].id, "b");
        state.tabs.push(tab_for_test("c", "personal", 1));
        state.tabs.push(tab_for_test("d", "personal", 1));
        state.active_tab = "d".into();
        state.apply(&Command::CloseBelow { id: Some("b".into()) }).unwrap();
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.tabs[0].id, "b");
    }
    #[test]
    fn legacy_tabs_get_a_fresh_age_and_activation_touches_it() {
        let mut state = State::default();
        state.tabs[0].last_activated = 0;
        state.repair();
        let now = now_secs();
        assert!(state.tabs[0].last_activated > 0);
        assert!((now.saturating_sub(state.tabs[0].last_activated)) < 60);
        state.new_tab("https://example.com").unwrap();
        let created = state.tabs.last().unwrap();
        assert!(now.saturating_sub(created.last_activated) < 60);
        let id = created.id.clone();
        state.tabs.iter_mut().find(|tab| tab.id == id).unwrap().last_activated = 1;
        state.activate(&id);
        let activated = state.tabs.iter().find(|tab| tab.id == id).unwrap();
        assert!(now.saturating_sub(activated.last_activated) < 60);
    }
    #[test]
    fn borderless_defaults_on_and_glass_survives_a_session() {
        let mut state = State::default();
        assert!(state.borderless);
        state.borderless = false;
        state.spaces[0].translucency = Some(0.7);
        let mut json = serde_json::to_value(&state).unwrap();
        json["spaces"][0]["clearGlass"] = serde_json::json!(false);
        let back: State = serde_json::from_value(json.clone()).unwrap();
        assert!(!back.borderless);
        assert_eq!(back.spaces[0].translucency, Some(0.7));
        assert!(serde_json::to_value(&back).unwrap()["spaces"][0].get("clearGlass").is_none());
        let mut legacy = json;
        legacy.as_object_mut().unwrap().remove("borderless");
        let restored: State = serde_json::from_value(legacy).unwrap();
        assert!(restored.borderless);
        state.apply(&Command::Borderless).unwrap();
        assert!(state.borderless);
        state
            .apply(&Command::UpdateSpace {
                id: state.spaces[0].id.clone(),
                name: state.spaces[0].name.clone(),
                color: state.spaces[0].color.clone(),
                translucency: 0.0,
                icon: Some("🚀".into()),
                paint: SpacePaint::default(),
            })
            .unwrap();
        assert_eq!(state.spaces[0].translucency, None);
        assert_eq!(state.spaces[0].icon.as_deref(), Some("🚀"));
        let saved = serde_json::to_value(&state).unwrap();
        assert_eq!(saved["spaces"][0]["icon"], "🚀");
        let restored: State = serde_json::from_value(saved).unwrap();
        assert_eq!(restored.spaces[0].icon.as_deref(), Some("🚀"));
        state
            .apply(&Command::UpdateSpace {
                id: state.spaces[0].id.clone(),
                name: state.spaces[0].name.clone(),
                color: state.spaces[0].color.clone(),
                translucency: 0.0,
                icon: Some("  ".into()),
                paint: SpacePaint::default(),
            })
            .unwrap();
        assert!(state.spaces[0].icon.is_none());
        assert!(serde_json::to_value(&state).unwrap()["spaces"][0].get("icon").is_none());
        assert!(state
            .apply(&Command::UpdateSpace {
                id: state.spaces[0].id.clone(),
                name: state.spaces[0].name.clone(),
                color: state.spaces[0].color.clone(),
                translucency: 0.0,
                icon: Some("a\nb".into()),
                paint: SpacePaint::default(),
            })
            .is_err());
    }
    #[test]
    fn space_paint_roundtrips_and_clears() {
        let mut state = State::default();
        let id = state.spaces[0].id.clone();
        state
            .apply(&Command::UpdateSpace {
                id: id.clone(),
                name: state.spaces[0].name.clone(),
                color: "sky".into(),
                translucency: 0.4,
                icon: None,
                paint: SpacePaint {
                    hue: Some(0.2),
                    tint: Some(1.0),
                    brightness: Some(0.3),
                    hue2: Some(0.5),
                    tint2: None,
                    brightness2: Some(0.5),
                    angle: Some(90.0),
                    grain: Some(0.004),
                },
            })
            .unwrap();
        assert_eq!(state.spaces[0].paint.hue, Some(0.2));
        assert!(state.spaces[0].paint.tint.is_none());
        assert!(state.spaces[0].paint.grain.is_none());
        assert_eq!(state.spaces[0].paint.angle, Some(90.0));
        let saved = serde_json::to_value(&state).unwrap();
        assert_eq!(saved["spaces"][0]["hue"], 0.2);
        assert!(saved["spaces"][0].get("grain").is_none());
        let restored: State = serde_json::from_value(saved).unwrap();
        assert_eq!(restored.spaces[0].paint.brightness, Some(0.3));
        state
            .apply(&Command::UpdateSpace {
                id,
                name: state.spaces[0].name.clone(),
                color: "sky".into(),
                translucency: 0.0,
                icon: None,
                paint: SpacePaint::default(),
            })
            .unwrap();
        assert!(state.spaces[0].paint.hue.is_none());
        assert!(state.spaces[0].translucency.is_none());
    }
}
