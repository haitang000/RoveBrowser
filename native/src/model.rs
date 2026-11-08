use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub color: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Favorite {
    #[serde(default = "new_id")]
    pub id: String,
    pub title: String,
    pub url: String,
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
    pub spaces: Vec<Space>,
    pub tabs: Vec<Tab>,
    pub active_space: String,
    pub active_tab: String,
    pub favorites: Vec<Favorite>,
    #[serde(default)]
    pub pins: Vec<Favorite>,
    #[serde(default)]
    pub library_version: u8,
    pub closed: Vec<Tab>,
    pub theme: String,
    pub sidebar_collapsed: bool,
}
#[derive(Clone, Copy, Deserialize, Debug, Default)]
pub struct Rect {
    pub x: f64,
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
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "presented")]
    Presented,
    #[serde(rename = "thumbnails")]
    Thumbnails,
    #[serde(rename = "windowChrome")]
    WindowChrome {
        maximize: Rect,
        #[serde(rename = "viewportWidth")]
        viewport_width: f64,
        background: [u8; 3],
        hover: [u8; 3],
        foreground: [u8; 3],
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
    CreateSpace { name: String, color: String },
    #[serde(rename = "spaceUpdate")]
    UpdateSpace {
        id: String,
        name: String,
        color: String,
    },
    #[serde(rename = "theme")]
    Theme,
    #[serde(rename = "sidebar")]
    Sidebar,
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
    },
    #[serde(rename = "overlay")]
    Overlay {
        visible: bool,
        #[serde(default)]
        command: bool,
    },
    #[serde(rename = "commandBackdropReady")]
    FloatingBackdropReady { ticket: u64 },
    #[serde(rename = "commandBackdropFailed")]
    FloatingBackdropFailed { ticket: u64 },
    #[serde(rename = "window")]
    Window { action: String },
    #[serde(rename = "history")]
    History { query: String },
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
    let text = input.trim();
    if text.is_empty() || text == "era://newtab" {
        return Ok("era://newtab".into());
    }
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
        let mut u = url::Url::parse("https://www.google.com/search").unwrap();
        u.query_pairs_mut().append_pair("q", text);
        return Ok(u.to_string());
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
        })
        .collect();
        let mut s = Self {
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
            })
            .collect(),
            tabs: vec![],
            active_space: "personal".into(),
            active_tab: String::new(),
            favorites,
            pins: vec![],
            library_version: 0,
            closed: vec![],
            theme: "light".into(),
            sidebar_collapsed: false,
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
        if let Some(t) = self.tabs.iter().find(|t| t.id == id) {
            self.active_tab = t.id.clone();
            self.active_space = t.space_id.clone()
        }
    }
    pub fn new_tab(&mut self, input: &str) -> Result<(), String> {
        if self.tabs.len() >= 256 {
            return Err("已达到 256 个标签页上限".into());
        }
        let url = normalize(input)?;
        let title = url::Url::parse(&url)
            .ok()
            .and_then(|u| u.host_str().map(str::to_string))
            .filter(|_| url != "era://newtab")
            .unwrap_or("New Tab".into());
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
        });
        self.active_tab = id;
        Ok(())
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
                }
            }
        }
    }
    pub fn repair(&mut self) {
        if self.spaces.is_empty() {
            *self = Self::default();
            return;
        }
        if !self.spaces.iter().any(|s| s.id == self.active_space) {
            self.active_space = self.spaces[0].id.clone()
        }
        self.repair_bookmarks();
        let ids: Vec<_> = self.tabs.iter().map(|t| t.id.clone()).collect();
        for t in &mut self.tabs {
            t.loading = false;
            if !self.spaces.iter().any(|s| s.id == t.space_id) {
                t.space_id = self.active_space.clone()
            }
            if !is_web(&t.url) {
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
    }
    pub fn apply(&mut self, c: &Command) -> Result<(), String> {
        if self.apply_bookmark_command(c)? {
            return Ok(());
        }
        match c {
            Command::New { url } => self.new_tab(url)?,
            Command::Navigate { url } => {
                let u = normalize(url)?;
                if let Some(t) = self.tabs.iter_mut().find(|t| t.id == self.active_tab) {
                    t.url = u;
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
            Command::Restore => {
                if let Some(mut t) = self.closed.pop() {
                    t.split_with = None;
                    t.pinned = false;
                    t.bookmark_id = None;
                    let id = t.id.clone();
                    if !self.spaces.iter().any(|s| s.id == t.space_id) {
                        t.space_id = self.active_space.clone()
                    }
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
                    url,
                    space_id: None,
                    split_with: index.and_then(|i| self.favorites[i].split_with.clone()),
                };
                if let Some(i) = index {
                    self.favorites[i] = favorite;
                } else {
                    if self.favorites.len() >= 32 {
                        return Err("最多添加 32 个常用网站".into());
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
                    self.active_space = id.clone();
                    self.active_tab = self
                        .tabs
                        .iter()
                        .find(|t| &t.space_id == id)
                        .map(|t| t.id.clone())
                        .unwrap_or_default();
                    if self.active_tab.is_empty() {
                        self.new_tab("")?
                    }
                }
            }
            Command::CreateSpace { name, color } => {
                if !valid_color(color) {
                    return Err("未知工作区颜色".into());
                }
                if self.spaces.len() >= 16 {
                    return Err("最多创建 16 个工作区".into());
                }
                let id = Uuid::new_v4().to_string();
                self.spaces.push(Space {
                    id: id.clone(),
                    name: name.chars().take(40).collect(),
                    color: color.clone(),
                });
                self.active_space = id;
                self.new_tab("")?
            }
            Command::UpdateSpace { id, name, color } => {
                if !valid_color(color) {
                    return Err("未知工作区颜色".into());
                }
                if let Some(s) = self.spaces.iter_mut().find(|s| &s.id == id) {
                    if !name.trim().is_empty() {
                        s.name = name.chars().take(40).collect()
                    }
                    s.color = color.clone()
                }
            }
            Command::Theme => {
                self.theme = if self.theme == "dark" {
                    "light"
                } else {
                    "dark"
                }
                .into()
            }
            Command::Sidebar => self.sidebar_collapsed = !self.sidebar_collapsed,
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
}
