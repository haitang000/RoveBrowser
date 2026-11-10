// Behavioral port of Zigzag's Library / TabManager (Apache-2.0), adapted to WebView2.
// Saved pages survive unloading; only Tab records own live browser controllers.
use crate::model::{favorite_url, new_id, Command, Favorite, State, Tab, MAX_FAVORITES};

// Tab.site(of:) and isSignIn, preserving sign-in/redirects in their existing controller.
pub fn leaves_site(here: &str, there: &str) -> bool {
    fn site(url: &url::Url) -> String {
        let host = url.host_str().unwrap_or("").to_lowercase();
        if url
            .host()
            .is_some_and(|h| !matches!(h, url::Host::Domain(_)))
        {
            return host;
        }
        let parts: Vec<_> = host.split('.').collect();
        let keep = if parts.len() > 2
            && parts.last().unwrap().len() == 2
            && [
                "co", "com", "org", "net", "ac", "gov", "edu", "ne", "or", "go",
            ]
            .contains(&parts[parts.len() - 2])
        {
            3
        } else {
            2
        };
        parts[parts.len().saturating_sub(keep)..].join(".")
    }
    let (Ok(here), Ok(there)) = (url::Url::parse(here), url::Url::parse(there)) else {
        return false;
    };
    if !matches!(there.scheme(), "http" | "https") {
        return false;
    }
    let host = there.host_str().unwrap_or("").to_lowercase();
    let path = there.path().to_lowercase();
    let sign_in = [
        "accounts.",
        "login.",
        "auth.",
        "signin.",
        "sso.",
        "id.",
        "appleid.",
    ]
    .iter()
    .any(|prefix| host.starts_with(prefix))
        || ["/login", "/signin", "/sign-in", "/oauth", "/sso"]
            .iter()
            .any(|prefix| path.starts_with(prefix));
    site(&here) != site(&there) && !sign_in
}

pub fn bookmark_title(title: &str, input: &str) -> String {
    let Ok(url) = url::Url::parse(input) else {
        return title.into();
    };
    if !url.path().trim_matches('/').is_empty() || url.query().is_some() {
        return title.into();
    }
    let host = url
        .host_str()
        .unwrap_or("")
        .trim_start_matches("www.")
        .split('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    for separator in [" · ", " | ", " - ", " — ", " – ", ": "] {
        let parts: Vec<_> = title.split(separator).collect();
        if parts.len() > 1 && !host.is_empty() {
            for part in [parts[0], parts[parts.len() - 1]] {
                if part.to_lowercase().replace(' ', "").contains(&host) {
                    return part.into();
                }
            }
        }
    }
    title.into()
}

impl State {
    pub fn navigable_ids(&self) -> Vec<String> {
        self.favorites
            .iter()
            .chain(
                self.pins
                    .iter()
                    .filter(|f| f.space_id.as_ref() == Some(&self.active_space)),
            )
            .filter_map(|f| {
                self.tabs
                    .iter()
                    .find(|t| t.bookmark_id.as_ref() == Some(&f.id))
                    .map(|t| t.id.clone())
            })
            .chain(
                self.tabs
                    .iter()
                    .filter(|t| t.bookmark_id.is_none() && t.space_id == self.active_space)
                    .map(|t| t.id.clone()),
            )
            .collect()
    }
    pub fn bookmark(&self, id: &str) -> Option<&Favorite> {
        self.pins.iter().chain(&self.favorites).find(|f| f.id == id)
    }
    pub fn bookmark_id(&self, id: &str) -> Option<String> {
        self.bookmark(id).map(|f| f.id.clone()).or_else(|| {
            self.tabs
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.bookmark_id.clone())
        })
    }
    fn group(&self, id: &str) -> Vec<String> {
        let other = self.bookmark(id).and_then(|f| f.split_with.as_ref());
        self.pins
            .iter()
            .chain(&self.favorites)
            .filter(|f| f.id == id || Some(&f.id) == other)
            .map(|f| f.id.clone())
            .collect()
    }
    pub fn repair_bookmarks(&mut self) {
        if self.library_version == 0 {
            for tab in &mut self.tabs {
                if tab.pinned {
                    let id = new_id();
                    self.pins.push(Favorite {
                        id: id.clone(),
                        title: tab.title.clone(),
                        url: tab.url.clone(),
                        favicon: tab.favicon.clone(),
                        space_id: Some(tab.space_id.clone()),
                        split_with: None,
                    });
                    tab.bookmark_id = Some(id);
                }
            }
            for tab in &self.tabs {
                if let (Some(id), Some(other)) = (
                    &tab.bookmark_id,
                    tab.split_with
                        .as_ref()
                        .and_then(|id| self.tabs.iter().find(|t| &t.id == id))
                        .and_then(|t| t.bookmark_id.clone()),
                ) {
                    if let Some(f) = self.pins.iter_mut().find(|f| &f.id == id) {
                        f.split_with = Some(other.clone());
                    }
                    if let Some(f) = self.pins.iter_mut().find(|f| f.id == other) {
                        f.split_with = Some(id.clone());
                    }
                }
            }
            self.library_version = 1;
        }
        let active_space = self.active_space.clone();
        // Preserve overflow from older sessions as workspace pins, keeping split pairs together.
        let mut overflow: std::collections::HashSet<_> = self
            .favorites
            .iter()
            .skip(MAX_FAVORITES)
            .map(|f| f.id.clone())
            .collect();
        for f in &self.favorites {
            if overflow.contains(&f.id) {
                if let Some(other) = &f.split_with {
                    overflow.insert(other.clone());
                }
            }
        }
        let mut retained = Vec::new();
        for mut f in self.favorites.drain(..) {
            if overflow.contains(&f.id) {
                f.space_id = Some(active_space.clone());
                self.pins.push(f);
            } else {
                retained.push(f);
            }
        }
        self.favorites = retained;
        for f in &mut self.pins {
            if !self
                .spaces
                .iter()
                .any(|s| Some(&s.id) == f.space_id.as_ref())
            {
                f.space_id = Some(active_space.clone());
            }
        }
        let marks = self
            .pins
            .iter()
            .chain(&self.favorites)
            .map(|f| (f.id.clone(), f.space_id.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        let mut seen = std::collections::HashSet::new();
        for t in &mut self.tabs {
            if let Some(id) = &t.bookmark_id {
                if let Some(space) = marks.get(id).filter(|_| seen.insert(id.clone())) {
                    t.pinned = space.is_some();
                    if let Some(space) = space {
                        t.space_id = space.clone();
                    }
                } else {
                    t.bookmark_id = None;
                    t.pinned = false;
                }
            }
        }
        for f in self.pins.iter_mut().chain(&mut self.favorites) {
            if f.split_with
                .as_ref()
                .is_some_and(|id| id == &f.id || marks.get(id) != Some(&f.space_id))
            {
                f.split_with = None;
            }
        }
    }
    pub fn open_bookmark(&mut self, id: &str) -> Result<(), String> {
        let mark = self.bookmark(id).cloned().ok_or("固定项不存在")?;
        let group = self.group(id);
        let missing = group
            .iter()
            .filter(|id| !self.tabs.iter().any(|t| t.bookmark_id.as_ref() == Some(id)))
            .count();
        if self.tabs.len() + missing > 256 {
            return Err("已达到 256 个标签页上限".into());
        }
        let mut live = vec![];
        for id in &group {
            if let Some(t) = self
                .tabs
                .iter()
                .find(|t| t.bookmark_id.as_ref() == Some(id))
            {
                live.push(t.id.clone());
            } else if let Some(f) = self.bookmark(id).cloned() {
                let tab_id = new_id();
                self.tabs.push(Tab {
                    id: tab_id.clone(),
                    space_id: f.space_id.unwrap_or_else(|| self.active_space.clone()),
                    title: f.title,
                    url: f.url,
                    pinned: self.pins.iter().any(|p| &p.id == id),
                    bookmark_id: Some(id.clone()),
                    loading: false,
                    favicon: f.favicon,
                    split_with: None,
                    muted: false,
                });
                live.push(tab_id);
            }
        }
        if live.len() == 2 {
            for t in &mut self.tabs {
                if t.id == live[0] {
                    t.split_with = Some(live[1].clone());
                } else if t.id == live[1] {
                    t.split_with = Some(live[0].clone());
                }
            }
        }
        let selected = self
            .tabs
            .iter()
            .find(|t| t.bookmark_id.as_deref() == Some(id))
            .unwrap()
            .id
            .clone();
        self.activate(&selected);
        if mark.space_id.is_none() {
            for t in &mut self.tabs {
                if live.contains(&t.id) {
                    t.space_id = self.active_space.clone();
                }
            }
        }
        Ok(())
    }
    pub fn remove_bookmark(&mut self, id: &str) {
        let ids = self.group(id);
        self.pins.retain(|f| !ids.contains(&f.id));
        self.favorites.retain(|f| !ids.contains(&f.id));
        for tab in &mut self.tabs {
            if tab.bookmark_id.as_ref().is_some_and(|id| ids.contains(id)) {
                tab.bookmark_id = None;
                tab.pinned = false;
                tab.space_id = self.active_space.clone();
            }
        }
    }
    fn keep_tab(
        &mut self,
        id: &str,
        favorite: bool,
        space: Option<&str>,
    ) -> Result<String, String> {
        if let Some(mark) = self.bookmark_id(id) {
            let ids = self.group(&mark);
            if favorite
                && self.favorites.len()
                    + ids
                        .iter()
                        .filter(|id| self.pins.iter().any(|f| &f.id == *id))
                        .count()
                    > MAX_FAVORITES
            {
                return Err("最多置顶 8 个网站".into());
            }
            let mut moving = vec![];
            for id in &ids {
                let mut f = self.bookmark(id).cloned().ok_or("固定项不存在")?;
                f.space_id = if favorite {
                    None
                } else {
                    Some(space.unwrap_or(&self.active_space).to_string())
                };
                moving.push(f);
            }
            self.pins.retain(|f| !ids.contains(&f.id));
            self.favorites.retain(|f| !ids.contains(&f.id));
            if favorite {
                self.favorites.extend(moving);
            } else {
                self.pins.extend(moving);
            }
            for t in &mut self.tabs {
                if t.bookmark_id.as_ref().is_some_and(|id| ids.contains(id)) {
                    t.pinned = !favorite;
                    t.space_id = space.unwrap_or(&self.active_space).to_string();
                }
            }
            let live: Vec<_> = self
                .tabs
                .iter()
                .filter(|t| t.bookmark_id.as_ref().is_some_and(|id| ids.contains(id)))
                .map(|t| t.id.clone())
                .collect();
            for t in &mut self.tabs {
                if t.split_with
                    .as_ref()
                    .is_some_and(|id| live.contains(&t.id) != live.contains(id))
                {
                    t.split_with = None;
                }
            }
            return Ok(mark);
        }
        let tab = self
            .tabs
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or("标签页不存在")?;
        let partner = tab.split_with.clone().or_else(|| {
            self.tabs
                .iter()
                .find(|t| t.split_with.as_deref() == Some(id))
                .map(|t| t.id.clone())
        });
        let mut tabs = vec![tab];
        if let Some(other) = partner
            .and_then(|id| self.tabs.iter().find(|t| t.id == id))
            .filter(|t| t.bookmark_id.is_none())
        {
            tabs.push(other.clone());
        }
        if tabs.iter().any(|t| favorite_url(&t.url).is_err()) {
            return Err("只有网页可以固定".into());
        }
        if favorite && self.favorites.len() + tabs.len() > MAX_FAVORITES {
            return Err("最多置顶 8 个网站".into());
        }
        let ids: Vec<_> = tabs.iter().map(|_| new_id()).collect();
        for (i, tab) in tabs.iter().enumerate() {
            let f = Favorite {
                id: ids[i].clone(),
                title: bookmark_title(&tab.title, &tab.url),
                url: tab.url.clone(),
                favicon: tab.favicon.clone(),
                space_id: if favorite {
                    None
                } else {
                    Some(space.unwrap_or(&tab.space_id).to_string())
                },
                split_with: if ids.len() == 2 {
                    Some(ids[1 - i].clone())
                } else {
                    None
                },
            };
            if favorite {
                self.favorites.push(f);
            } else {
                self.pins.push(f);
            }
            let live = self.tabs.iter_mut().find(|t| t.id == tab.id).unwrap();
            live.bookmark_id = Some(ids[i].clone());
            live.pinned = !favorite;
            if let Some(space) = space {
                live.space_id = space.to_string();
            }
        }
        Ok(ids[0].clone())
    }
    fn unpin(&mut self, id: &str, space: Option<&str>) -> Result<(), String> {
        let Some(mark) = self.bookmark_id(id) else {
            return Ok(());
        };
        let group = self.group(&mark);
        let space = space
            .map(str::to_string)
            .or_else(|| self.bookmark(&mark).and_then(|f| f.space_id.clone()))
            .unwrap_or_else(|| self.active_space.clone());
        // Dragging an unloaded item to tabs opens it, as TabManager.unbookmark(toIndex:) does.
        if !self
            .tabs
            .iter()
            .any(|t| t.bookmark_id.as_ref() == Some(&mark))
        {
            self.open_bookmark(&mark)?;
        }
        let live = self
            .tabs
            .iter()
            .filter(|t| t.bookmark_id.as_ref().is_some_and(|id| group.contains(id)))
            .map(|t| t.id.clone())
            .collect::<Vec<_>>();
        self.remove_bookmark(&mark);
        for t in &mut self.tabs {
            if live.contains(&t.id) {
                t.space_id = space.clone();
            }
        }
        // remove_bookmark preserved the controller ids; saved ids and tab ids need not match.
        if let Some(t) = self
            .active()
            .filter(|t| t.space_id != self.active_space)
            .cloned()
        {
            self.active_space = t.space_id;
        }
        Ok(())
    }
    pub fn apply_bookmark_command(&mut self, c: &Command) -> Result<bool, String> {
        match c {
            Command::Split { id, other } => {
                let mut live_ids = vec![];
                for id in [id, other] {
                    if self.bookmark(id).is_some() {
                        self.open_bookmark(id)?;
                        live_ids.push(
                            self.tabs
                                .iter()
                                .find(|t| t.bookmark_id.as_ref() == Some(id))
                                .unwrap()
                                .id
                                .clone(),
                        );
                    } else {
                        live_ids.push(id.clone());
                    }
                }
                let a = self.tabs.iter().find(|t| t.id == live_ids[0]).cloned();
                let b = self.tabs.iter().find(|t| t.id == live_ids[1]).cloned();
                if let (Some(a), Some(b)) = (a, b) {
                    if a.id != b.id && a.space_id == b.space_id {
                        self.tabs
                            .iter_mut()
                            .find(|t| t.id == a.id)
                            .unwrap()
                            .split_with = Some(b.id);
                        self.activate(&a.id);
                    }
                }
            }
            Command::MoveFavorite { url, offset } => {
                if matches!(offset, -1 | 1) {
                    let mut groups: Vec<Vec<Favorite>> = vec![];
                    for f in self.favorites.clone() {
                        if groups
                            .iter()
                            .any(|g| g.iter().any(|p| p.split_with.as_ref() == Some(&f.id)))
                        {
                            groups
                                .iter_mut()
                                .find(|g| g.iter().any(|p| p.split_with.as_ref() == Some(&f.id)))
                                .unwrap()
                                .push(f);
                        } else {
                            groups.push(vec![f]);
                        }
                    }
                    if let Some(i) = groups.iter().position(|g| g.iter().any(|f| &f.url == url)) {
                        let target = i as isize + *offset as isize;
                        if target >= 0 && (target as usize) < groups.len() {
                            groups.swap(i, target as usize);
                            self.favorites = groups.into_iter().flatten().collect();
                        }
                    }
                }
            }
            Command::Unsplit => {
                let active = self.active_tab.clone();
                let other = self.active().and_then(|t| t.split_with.clone());
                for t in &mut self.tabs {
                    if t.id == active
                        || Some(&t.id) == other.as_ref()
                        || t.split_with.as_ref() == Some(&active)
                    {
                        t.split_with = None;
                    }
                }
            }
            Command::OpenBookmark { id } => self.open_bookmark(id)?,
            Command::UnloadBookmark { id } => {
                let ids = self.group(id);
                let live: Vec<_> = self
                    .tabs
                    .iter()
                    .filter(|t| t.bookmark_id.as_ref().is_some_and(|id| ids.contains(id)))
                    .map(|t| t.id.clone())
                    .collect();
                for id in live {
                    self.close(&id);
                }
            }
            Command::ResetBookmark { id } => {
                let f = self.bookmark(id).cloned().ok_or("固定项不存在")?;
                if let Some(t) = self
                    .tabs
                    .iter_mut()
                    .find(|t| t.bookmark_id.as_ref() == Some(id))
                {
                    t.url = f.url;
                    t.favicon = f.favicon;
                }
            }
            Command::SeparateBookmark { id } => {
                if let Some(t) = self
                    .tabs
                    .iter_mut()
                    .find(|t| t.bookmark_id.as_ref() == Some(id))
                {
                    let live = t.id.clone();
                    t.bookmark_id = None;
                    t.pinned = false;
                    t.split_with = None;
                    t.space_id = self.active_space.clone();
                    for other in &mut self.tabs {
                        if other.split_with.as_ref() == Some(&live) {
                            other.split_with = None;
                        }
                    }
                    self.activate(&live);
                }
            }
            Command::UpdateBookmark { id, title, url } => {
                let url = favorite_url(url)?;
                if title.trim().is_empty() || title.chars().count() > 80 {
                    return Err("请输入 1–80 个字符的网站名称".into());
                }
                if let Some(f) = self
                    .pins
                    .iter_mut()
                    .chain(&mut self.favorites)
                    .find(|f| &f.id == id)
                {
                    f.title = title.trim().into();
                    if !crate::favicon::same_origin(&f.url, &url) {
                        f.favicon = None;
                    }
                    f.url = url;
                }
            }
            Command::UnpinBookmark { id, space_id } => self.unpin(id, space_id.as_deref())?,
            Command::PinBookmark { id, space_id } => {
                if space_id
                    .as_ref()
                    .is_some_and(|id| !self.spaces.iter().any(|s| &s.id == id))
                {
                    return Err("工作区不存在".into());
                }
                self.keep_tab(id, false, space_id.as_deref())?;
            }
            Command::FavoriteTab { id, before } => {
                let mark = self.keep_tab(id, true, None)?;
                if let Some(before) = before {
                    move_before(&mut self.favorites, &mark, before);
                }
            }
            Command::Pin { id } => {
                let id = id.as_deref().unwrap_or(&self.active_tab).to_string();
                if self.bookmark_id(&id).is_some() {
                    self.unpin(&id, None)?;
                } else {
                    self.keep_tab(&id, false, None)?;
                }
            }
            Command::Move { id, space_id } if self.bookmark_id(id).is_some() => {
                if !self.spaces.iter().any(|s| &s.id == space_id) {
                    return Err("工作区不存在".into());
                }
                self.keep_tab(id, false, Some(space_id))?;
                if let Some(t) = self.active() {
                    self.active_space = t.space_id.clone();
                }
            }
            Command::Reorder { id, before } => {
                let target = self.bookmark_id(before);
                if let Some(target) = target.filter(|id| self.pins.iter().any(|f| &f.id == id)) {
                    let space = self
                        .bookmark(&target)
                        .and_then(|f| f.space_id.clone())
                        .unwrap();
                    let mark = self.keep_tab(id, false, Some(&space))?;
                    move_before(&mut self.pins, &mark, &target);
                } else {
                    let mark = self.bookmark_id(id);
                    if let Some(mark) = &mark {
                        self.open_bookmark(mark)?;
                    }
                    let source = mark
                        .as_ref()
                        .and_then(|id| {
                            self.tabs
                                .iter()
                                .find(|t| t.bookmark_id.as_ref() == Some(id))
                        })
                        .map(|t| t.id.clone())
                        .unwrap_or_else(|| id.clone());
                    if mark.is_some() {
                        self.unpin(id, Some(&self.active_space.clone()))?;
                    }
                    if source != *before {
                        if let (Some(a), Some(b)) = (
                            self.tabs.iter().position(|t| t.id == source),
                            self.tabs.iter().position(|t| &t.id == before),
                        ) {
                            if self.tabs[a].space_id == self.tabs[b].space_id {
                                let t = self.tabs.remove(a);
                                let at = self.tabs.iter().position(|t| &t.id == before).unwrap();
                                self.tabs.insert(at, t);
                            }
                        }
                    }
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

fn move_before(items: &mut Vec<Favorite>, id: &str, before: &str) {
    if id == before {
        return;
    }
    let partner = items
        .iter()
        .find(|f| f.id == id)
        .and_then(|f| f.split_with.clone());
    let target_partner = items
        .iter()
        .find(|f| f.id == before)
        .and_then(|f| f.split_with.clone());
    if partner.as_deref() == Some(before) {
        return;
    }
    let moving: Vec<_> = items
        .iter()
        .filter(|f| f.id == id || Some(&f.id) == partner.as_ref())
        .cloned()
        .collect();
    if moving.is_empty() || !items.iter().any(|f| f.id == before) {
        return;
    }
    items.retain(|f| !moving.iter().any(|m| m.id == f.id));
    let target = items
        .iter()
        .position(|f| f.id == before || Some(&f.id) == target_partner.as_ref())
        .unwrap();
    items.splice(target..target, moving);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn favorite_limits_are_atomic_for_regular_tabs_pins_and_split_groups() {
        let mut s = State::default();
        assert!(s
            .apply(&Command::SaveFavorite {
                title: "Ninth".into(),
                url: "https://extra.example".into(),
                previous_url: None,
            })
            .unwrap_err()
            .contains('8'));
        s.new_tab("https://example.com/a").unwrap();
        let a = s.active_tab.clone();
        let before = serde_json::to_string(&s).unwrap();
        for id in [&a, &s.pins[0].id.clone()] {
            assert!(s.keep_tab(id, true, None).unwrap_err().contains('8'));
            assert_eq!(serde_json::to_string(&s).unwrap(), before);
        }
        s.favorites.pop();
        s.new_tab("https://example.com/b").unwrap();
        s.apply(&Command::Split {
            id: a.clone(),
            other: s.active_tab.clone(),
        })
        .unwrap();
        let before = serde_json::to_string(&s).unwrap();
        assert!(s.keep_tab(&a, true, None).is_err());
        assert_eq!(serde_json::to_string(&s).unwrap(), before);
        s.favorites.pop();
        let mark = s.keep_tab(&a, true, None).unwrap();
        assert_eq!(s.favorites.len(), MAX_FAVORITES);
        s.keep_tab(&mark, true, None).unwrap();
        assert_eq!(s.favorites.len(), MAX_FAVORITES);
    }

    #[test]
    fn legacy_overflow_preserves_saved_split_pairs_as_workspace_pins() {
        let mut s = State::default();
        let mut extra = s.favorites[7].clone();
        extra.id = "overflow".into();
        extra.split_with = Some(s.favorites[7].id.clone());
        s.favorites[7].split_with = Some(extra.id.clone());
        s.favorites.push(extra);
        s.repair();
        assert_eq!(s.favorites.len(), 7);
        assert_eq!(s.pins.len(), 4);
        assert!(s.pins[2..]
            .iter()
            .all(|f| f.space_id.as_deref() == Some("personal") && f.split_with.is_some()));
        s.repair();
        assert_eq!(s.pins.len(), 4);
    }
    #[test]
    fn unload_retains_pin_and_reopens_its_saved_address() {
        let mut s = State::default();
        let live = s.tabs[0].id.clone();
        let id = s.tabs[0].bookmark_id.clone().unwrap();
        let saved = s.bookmark(&id).unwrap().clone();
        s.tabs[0].url = "https://github.com/explore".into();
        s.close(&live);
        assert!(s.closed.is_empty());
        assert_eq!(s.bookmark(&id).unwrap().url, saved.url);
        s.open_bookmark(&id).unwrap();
        assert_ne!(s.active_tab, live);
        assert_eq!(s.active().unwrap().url, saved.url);
    }
    #[test]
    fn favorite_reuses_one_controller_identity_across_spaces_and_unpinning() {
        let mut s = State::default();
        s.favorites.pop();
        s.new_tab("https://example.com/docs").unwrap();
        let live = s.active_tab.clone();
        s.apply(&Command::FavoriteTab {
            id: live.clone(),
            before: None,
        })
        .unwrap();
        let id = s.active().unwrap().bookmark_id.clone().unwrap();
        s.active_space = "work".into();
        s.open_bookmark(&id).unwrap();
        assert_eq!(s.active_tab, live);
        assert_eq!(s.active_space, "work");
        s.apply(&Command::UnpinBookmark { id, space_id: None })
            .unwrap();
        assert_eq!(s.active_tab, live);
        assert!(s.active().unwrap().bookmark_id.is_none());
        assert_eq!(s.active().unwrap().space_id, "work");
    }
    #[test]
    fn legacy_session_upgrades_without_losing_pins_or_custom_favorites() {
        let original = State::default();
        let mut json = serde_json::to_value(&original).unwrap();
        json.as_object_mut().unwrap().remove("libraryVersion");
        json.as_object_mut().unwrap().remove("pins");
        for t in json["tabs"].as_array_mut().unwrap() {
            t.as_object_mut().unwrap().remove("bookmarkId");
        }
        for f in json["favorites"].as_array_mut().unwrap() {
            f.as_object_mut().unwrap().remove("id");
        }
        let mut restored: State = serde_json::from_value(json).unwrap();
        restored.repair();
        assert_eq!(restored.pins.len(), 2);
        assert_eq!(restored.favorites.len(), original.favorites.len());
        assert_eq!(restored.active_tab, original.active_tab);
        let count = restored.pins.len();
        restored.repair();
        assert_eq!(restored.pins.len(), count);
    }
    #[test]
    fn pinned_split_unloads_both_pages_and_recreates_the_pair() {
        let mut s = State::default();
        s.new_tab("https://example.com/a").unwrap();
        let a = s.active_tab.clone();
        s.new_tab("https://example.com/b").unwrap();
        let b = s.active_tab.clone();
        s.apply(&Command::Split {
            id: a.clone(),
            other: b.clone(),
        })
        .unwrap();
        s.apply(&Command::Pin {
            id: Some(a.clone()),
        })
        .unwrap();
        let id = s.bookmark_id(&a).unwrap();
        s.apply(&Command::UnloadBookmark { id: id.clone() })
            .unwrap();
        assert!(!s.tabs.iter().any(|t| t.id == a || t.id == b));
        s.open_bookmark(&id).unwrap();
        assert!(s.active().unwrap().split_with.is_some());
    }
    #[test]
    fn link_policy_keeps_related_sites_and_sign_in_and_distinguishes_ip_hosts() {
        assert!(!leaves_site(
            "https://mail.google.com",
            "https://accounts.google.com"
        ));
        assert!(!leaves_site(
            "https://bbc.co.uk",
            "https://news.bbc.co.uk/story"
        ));
        assert!(!leaves_site(
            "https://example.com",
            "https://login.other.com/signin"
        ));
        assert!(!leaves_site(
            "https://example.com",
            "https://other.com/oauth/callback"
        ));
        assert!(leaves_site(
            "https://example.com",
            "https://other.com/article"
        ));
        assert!(leaves_site("http://127.0.0.1", "http://127.0.0.2"));
        assert_eq!(
            bookmark_title("Inbox (3) - Gmail", "https://gmail.com/"),
            "Gmail"
        );
    }
}
