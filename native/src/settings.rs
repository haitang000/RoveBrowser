//! Zigzag SettingsPage/SearchEngine/Shortcuts port (Zigao Wang, Apache-2.0).
//! The host, rather than a website or renderer, validates and owns preferences.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Shortcut {
    pub key: u32,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}
pub fn default_shortcuts() -> Vec<(&'static str, Shortcut)> {
    [
        ("address", 76, true, false, false),
        ("new", 84, true, false, false),
        ("close", 87, true, false, false),
        ("restore", 84, true, false, true),
        ("cycle", 9, true, false, false),
        ("cyclePrevious", 9, true, false, true),
        ("sidebar", 66, true, false, false),
        ("desk", 220, true, false, true),
        ("settings", 188, true, false, false),
        ("reload", 82, true, false, false),
        ("back", 37, false, true, false),
        ("forward", 39, false, true, false),
        ("find", 70, true, false, false),
    ]
    .into_iter()
    .map(|(id, key, ctrl, alt, shift)| {
        (
            id,
            Shortcut {
                key,
                ctrl,
                alt,
                shift,
            },
        )
    })
    .collect()
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub search_engine: String,
    pub custom_search_template: String,
    pub search_suggestions: bool,
    pub large_space_names: bool,
    pub favorites_per_row: u8,
    pub reduce_motion: bool,
    pub ask_before_quit: bool,
    pub save_passwords: bool,
    pub autofill: bool,
    pub remember_history: bool,
    pub download_folder: String,
    pub shortcuts: HashMap<String, Option<Shortcut>>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            search_engine: "google".into(),
            custom_search_template: String::new(),
            search_suggestions: true,
            large_space_names: true,
            favorites_per_row: 4,
            reduce_motion: false,
            ask_before_quit: true,
            save_passwords: true,
            autofill: true,
            remember_history: true,
            download_folder: String::new(),
            shortcuts: HashMap::new(),
        }
    }
}
pub fn is_settings(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|u| {
        matches!(u.scheme(), "rove" | "era")
            && u.host_str() == Some("settings")
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
    })
}
pub fn normalize_settings_url(value: &str) -> Option<String> {
    if !is_settings(value) {
        return None;
    }
    let mut url = url::Url::parse(value).ok()?;
    url.set_scheme("rove").ok()?;
    Some(url.to_string())
}
pub fn settings_url(pane: Option<&str>) -> String {
    match pane {
        Some(
            p @ ("spaces" | "passwords" | "privacy" | "translation" | "extensions" | "shortcuts"),
        ) => format!("rove://settings/{p}"),
        _ => "rove://settings".into(),
    }
}
pub fn valid_template(template: &str) -> bool {
    template.len() <= 8192
        && template.contains("%s")
        && url::Url::parse(&template.replace("%s", "q")).is_ok_and(|u| {
            matches!(u.scheme(), "http" | "https")
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        })
}
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
impl Settings {
    pub fn repair(&mut self) {
        if !self.download_folder.is_empty() && !std::path::Path::new(&self.download_folder).is_dir()
        {
            self.download_folder.clear();
        }
        if !self.custom_search_template.is_empty() && !valid_template(&self.custom_search_template)
        {
            self.custom_search_template.clear();
        }
        if ![0, 2, 3, 4].contains(&self.favorites_per_row) {
            self.favorites_per_row = 4;
        }
        if self.validate().is_err() {
            self.shortcuts.clear();
        }
    }
    pub fn validate(&mut self) -> Result<(), String> {
        if ![
            "google",
            "duckDuckGo",
            "brave",
            "bing",
            "kagi",
            "ecosia",
            "startpage",
            "perplexity",
            "yahoo",
            "baidu",
            "yandex",
            "custom",
        ]
        .contains(&self.search_engine.as_str())
        {
            self.search_engine = "google".into();
        }
        self.custom_search_template = self.custom_search_template.trim().to_string();
        if self.custom_search_template.chars().count() > 2048
            || (!self.custom_search_template.is_empty()
                && !valid_template(&self.custom_search_template))
        {
            return Err("搜索地址需要包含 %s，并使用有效的 HTTP 或 HTTPS 网址".into());
        }
        if ![0, 2, 3, 4].contains(&self.favorites_per_row) {
            return Err("每行网站数量应为自动、2、3 或 4".into());
        }
        self.download_folder = self.download_folder.trim().to_string();
        if !self.download_folder.is_empty() {
            let path = std::path::Path::new(&self.download_folder);
            if !path.is_absolute() || !path.is_dir() {
                return Err("请选择已存在的绝对下载目录".into());
            }
        }
        let defaults = default_shortcuts();
        if self
            .shortcuts
            .keys()
            .any(|id| !defaults.iter().any(|(key, _)| key == id))
        {
            return Err("未知快捷键操作".into());
        }
        let mut used = vec![];
        for (id, default) in defaults {
            let Some(value) = self.shortcuts.get(id).cloned().unwrap_or(Some(default)) else {
                continue;
            };
            let key = value.key;
            let valid = (65..=90).contains(&key)
                || (48..=57).contains(&key)
                || (112..=123).contains(&key)
                || [
                    9, 32, 33, 34, 35, 36, 37, 38, 39, 40, 186, 187, 188, 189, 190, 191, 192, 219,
                    220, 221, 222,
                ]
                .contains(&key);
            if !valid
                || (!value.ctrl && !value.alt && !(112..=123).contains(&key))
                || (value.ctrl && !value.alt && (49..=57).contains(&key))
                || (value.alt && key == 115)
                || used.contains(&value)
            {
                return Err("快捷键重复或被系统／工作区保留".into());
            }
            used.push(value);
        }
        Ok(())
    }
    pub fn action(&self, value: &Shortcut) -> Option<String> {
        default_shortcuts().into_iter().find_map(|(id, default)| {
            let bound = self.shortcuts.get(id).cloned().unwrap_or(Some(default));
            (bound.as_ref() == Some(value)).then(|| id.into())
        })
    }
    pub fn search_url(&self, query: &str) -> String {
        if self.search_engine == "custom" && valid_template(&self.custom_search_template) {
            return self.custom_search_template.replace("%s", &encode(query));
        }
        let (base, key) = match self.search_engine.as_str() {
            "duckDuckGo" => ("https://duckduckgo.com/", "q"),
            "brave" => ("https://search.brave.com/search", "q"),
            "bing" => ("https://www.bing.com/search", "q"),
            "kagi" => ("https://kagi.com/search", "q"),
            "ecosia" => ("https://www.ecosia.org/search", "q"),
            "startpage" => ("https://www.startpage.com/sp/search", "query"),
            "perplexity" => ("https://www.perplexity.ai/search", "q"),
            "yahoo" => ("https://search.yahoo.com/search", "p"),
            "baidu" => ("https://www.baidu.com/s", "wd"),
            "yandex" => ("https://yandex.com/search/", "text"),
            _ => ("https://www.google.com/search", "q"),
        };
        format!("{base}?{key}={}", encode(query))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Command, State};
    #[test]
    fn search_engines_encode_queries_and_reject_unsafe_templates() {
        let mut s = Settings::default();
        for engine in [
            "google",
            "duckDuckGo",
            "brave",
            "bing",
            "kagi",
            "ecosia",
            "startpage",
            "perplexity",
            "yahoo",
            "baidu",
            "yandex",
        ] {
            s.search_engine = engine.into();
            let u = url::Url::parse(&s.search_url("中文 &+#?")).unwrap();
            assert_eq!(u.query_pairs().next().unwrap().1, "中文 &+#?");
        }
        s.search_engine = "custom".into();
        s.custom_search_template = "https://example.com/find/%s?q=%s".into();
        assert!(s.search_url("a/b&c").contains("a%2Fb%26c"));
        for template in [
            "javascript:%s",
            "file:///C:/%s",
            "https://example.com/no-placeholder",
            "https://a:b@example.com/?q=%s",
        ] {
            assert!(!valid_template(template));
        }
        s.custom_search_template = String::new();
        assert!(s.search_url("abc").starts_with("https://www.google.com/"));
    }
    #[test]
    fn settings_tabs_reuse_across_spaces_and_survive_session_repair() {
        let mut state = State::default();
        state
            .apply(&Command::OpenSettings {
                pane: Some("privacy".into()),
            })
            .unwrap();
        let id = state.active_tab.clone();
        state
            .apply(&Command::SwitchSpace { id: "work".into() })
            .unwrap();
        state.apply(&Command::OpenSettings { pane: None }).unwrap();
        assert_eq!(state.active_tab, id);
        assert_eq!(state.active().unwrap().url, "rove://settings/privacy");
        state.settings.search_engine = "baidu".into();
        state.settings.reduce_motion = true;
        assert_eq!(
            crate::model::normalize("era://settings/privacy").unwrap(),
            "rove://settings/privacy"
        );
        let tab = state.tabs.iter_mut().find(|t| t.id == id).unwrap();
        tab.url = "era://settings/privacy".into();
        let mut closed = tab.clone();
        closed.id = "legacy-closed-settings".into();
        closed.url = "era://settings/shortcuts".into();
        state.closed.push(closed);
        let mut restored: State =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        restored.repair();
        assert_eq!(restored.active().unwrap().url, "rove://settings/privacy");
        assert_eq!(
            restored.closed.last().unwrap().url,
            "rove://settings/shortcuts"
        );
        assert_eq!(restored.settings.search_engine, "baidu");
        assert!(restored.settings.reduce_motion);
    }
    #[test]
    fn shortcut_validation_is_atomic_and_has_one_owner() {
        let mut state = State::default();
        let mut next = state.settings.clone();
        next.shortcuts
            .insert("new".into(), Some(default_shortcuts()[0].1.clone()));
        assert!(state
            .apply(&Command::UpdateSettings { settings: next })
            .is_err());
        assert!(state.settings.shortcuts.is_empty());
        let mut next = state.settings.clone();
        let combo = Shortcut {
            key: 89,
            ctrl: true,
            alt: false,
            shift: true,
        };
        next.shortcuts.insert("new".into(), Some(combo.clone()));
        next.validate().unwrap();
        assert_eq!(next.action(&combo).as_deref(), Some("new"));
        next.shortcuts.insert("new".into(), None);
        assert!(next.action(&combo).is_none());
    }
    #[test]
    fn find_shortcut_defaults_to_ctrl_f_and_keeps_the_table_valid() {
        let settings = Settings::default();
        let ctrl_f = Shortcut {
            key: 70,
            ctrl: true,
            alt: false,
            shift: false,
        };
        assert_eq!(settings.action(&ctrl_f).as_deref(), Some("find"));
        // The defaults must stay a valid, duplicate-free binding set that the
        // settings page can render and the page accelerator can dispatch.
        let mut validated = Settings::default();
        validated.validate().unwrap();
        // A user may unbind it, like any other shortcut.
        let mut unbound = Settings::default();
        unbound.shortcuts.insert("find".into(), None);
        assert!(unbound.action(&ctrl_f).is_none());
        assert_eq!(
            unbound
                .action(&Shortcut {
                    key: 76,
                    ctrl: true,
                    alt: false,
                    shift: false
                })
                .as_deref(),
            Some("address")
        );
    }
    #[test]
    fn deleting_space_keeps_shared_favorites_and_breaks_stale_splits() {
        let mut state = State::default();
        let favorites = state.favorites.len();
        state
            .apply(&Command::OpenSettings {
                pane: Some("spaces".into()),
            })
            .unwrap();
        state
            .apply(&Command::ReorderSpace {
                id: "studio".into(),
                before: "work".into(),
            })
            .unwrap();
        assert_eq!(state.spaces[1].id, "studio");
        state
            .apply(&Command::DeleteSpace {
                id: "personal".into(),
            })
            .unwrap();
        assert_eq!(state.favorites.len(), favorites);
        assert!(state.tabs.iter().all(|t| t.space_id != "personal"));
        assert!(state
            .pins
            .iter()
            .all(|p| p.space_id.as_deref() != Some("personal")));
        assert!(state.active().is_some());
        state
            .apply(&Command::DeleteSpace {
                id: "studio".into(),
            })
            .unwrap();
        assert!(state
            .apply(&Command::DeleteSpace { id: "work".into() })
            .is_err());
    }
}
