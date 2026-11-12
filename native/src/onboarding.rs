// Flow adapted from Zigzag Import/Onboarding.swift, Copyright 2026 Zigao Wang,
// Apache-2.0. The native host owns first-run state and validates all choices.
use crate::model::{new_id, Favorite, State, MAX_FAVORITES};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Hello,
    Bring,
    Sites,
    Ads,
    Ready,
}
impl Step {
    fn next(self) -> Self {
        match self {
            Self::Hello => Self::Bring,
            Self::Bring => Self::Sites,
            Self::Sites => Self::Ads,
            _ => Self::Ready,
        }
    }
    fn back(self) -> Self {
        match self {
            Self::Sites => Self::Bring,
            Self::Ads => Self::Sites,
            Self::Ready => Self::Ads,
            _ => Self::Hello,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Onboarding {
    pub step: Step,
    pub completed: bool,
    #[serde(default)]
    pub imports: Vec<crate::import::Report>,
}
impl Default for Onboarding {
    // Profiles written before onboarding must continue to restore normally.
    fn default() -> Self {
        Self {
            step: Step::Hello,
            completed: true,
            imports: vec![],
        }
    }
}
// Keep in sync with src/lib/onboarding.ts; IPC accepts IDs, never caller-supplied URLs.
const SITES: &[(&str, &str, &str)] = &[
    ("gmail", "Gmail", "https://mail.google.com"),
    ("calendar", "Calendar", "https://calendar.google.com"),
    ("youtube", "YouTube", "https://www.youtube.com"),
    ("drive", "Drive", "https://drive.google.com"),
    ("notion", "Notion", "https://www.notion.so"),
    ("chatgpt", "ChatGPT", "https://chatgpt.com"),
    ("claude", "Claude", "https://claude.ai"),
    ("spotify", "Spotify", "https://open.spotify.com"),
    ("slack", "Slack", "https://app.slack.com"),
    ("figma", "Figma", "https://www.figma.com"),
    ("github", "GitHub", "https://github.com"),
    ("discord", "Discord", "https://discord.com/app"),
];
fn same_url(a: &str, b: &str) -> bool {
    url::Url::parse(a)
        .ok()
        .zip(url::Url::parse(b).ok())
        .is_some_and(|(a, b)| a == b)
}
impl State {
    pub fn first_run() -> Self {
        let mut state = Self::default();
        state.favorites.clear();
        state.pins.clear();
        state.tabs.clear();
        state.closed.clear();
        state.active_tab.clear();
        for space in &mut state.spaces {
            space.active_tab = None;
        }
        state.new_tab("").expect("blank page is valid");
        state.onboarding.completed = false;
        state
    }
    pub fn advance_onboarding(
        &mut self,
        step: Step,
        skip: bool,
        sites: &[String],
    ) -> Result<(), String> {
        self.check_onboarding(step)?;
        if step == Step::Sites && !skip {
            self.onboarding_sites(sites)?;
        }
        if step == Step::Ready {
            self.onboarding.completed = true;
        } else {
            self.onboarding.step = step.next();
        }
        Ok(())
    }
    pub fn back_onboarding(&mut self, step: Step) -> Result<(), String> {
        self.check_onboarding(step)?;
        self.onboarding.step = step.back();
        Ok(())
    }
    fn check_onboarding(&self, step: Step) -> Result<(), String> {
        if self.onboarding.completed || self.onboarding.step != step {
            Err("引导步骤已更新，请重试".into())
        } else {
            Ok(())
        }
    }
    fn onboarding_sites(&mut self, ids: &[String]) -> Result<(), String> {
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        if unique.len() != ids.len() || ids.iter().any(|id| !SITES.iter().any(|s| s.0 == id)) {
            return Err("无效的常用网站选项".into());
        }
        let other = self
            .favorites
            .iter()
            .filter(|f| !SITES.iter().any(|s| same_url(&f.url, s.2)))
            .count();
        if other + ids.len() > MAX_FAVORITES {
            return Err("最多置顶 8 个网站".into());
        }
        let removed: Vec<_> = self
            .favorites
            .iter()
            .filter(|f| {
                SITES
                    .iter()
                    .any(|s| same_url(&f.url, s.2) && !ids.iter().any(|id| id == s.0))
            })
            .map(|f| f.id.clone())
            .collect();
        for tab in &mut self.tabs {
            if tab
                .bookmark_id
                .as_ref()
                .is_some_and(|id| removed.contains(id))
            {
                tab.bookmark_id = None;
                tab.pinned = false;
            }
        }
        self.favorites.retain(|f| !removed.contains(&f.id));
        for f in &mut self.favorites {
            if f.split_with.as_ref().is_some_and(|id| removed.contains(id)) {
                f.split_with = None;
            }
        }
        for &(id, title, address) in SITES {
            if ids.iter().any(|picked| picked == id)
                && !self.favorites.iter().any(|f| same_url(&f.url, address))
            {
                self.favorites.push(Favorite {
                    id: new_id(),
                    title: title.into(),
                    url: address.into(),
                    favicon: None,
                    space_id: None,
                    split_with: None,
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_run_and_existing_profiles_are_distinct() {
        let fresh = State::first_run();
        assert!(!fresh.onboarding.completed);
        assert!(fresh.favorites.is_empty());
        assert_eq!(fresh.tabs.len(), 1);
        let mut legacy = serde_json::to_value(State::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("onboarding");
        let restored: State = serde_json::from_value(legacy).unwrap();
        assert!(restored.onboarding.completed);
        assert_eq!(restored.favorites.len(), 8);
    }
    #[test]
    fn choices_are_atomic_bounded_and_persist_across_restart() {
        let mut s = State::first_run();
        s.advance_onboarding(Step::Hello, false, &[]).unwrap();
        assert!(s.advance_onboarding(Step::Hello, false, &[]).is_err());
        s.advance_onboarding(Step::Bring, true, &[]).unwrap();
        let ids: Vec<_> = SITES.iter().take(9).map(|s| s.0.into()).collect();
        assert!(s.advance_onboarding(Step::Sites, false, &ids).is_err());
        assert!(s.favorites.is_empty());
        assert_eq!(s.onboarding.step, Step::Sites);
        assert!(s
            .advance_onboarding(Step::Sites, false, &["unknown".into()])
            .is_err());
        s.advance_onboarding(Step::Sites, false, &["github".into(), "gmail".into()])
            .unwrap();
        let mut s: State = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        s.repair();
        assert_eq!(s.onboarding.step, Step::Ads);
        assert_eq!(s.favorites.len(), 2);
        s.back_onboarding(Step::Ads).unwrap();
        let id = s.favorites[0].id.clone();
        s.advance_onboarding(Step::Sites, false, &["github".into(), "gmail".into()])
            .unwrap();
        assert_eq!(s.favorites[0].id, id);
        s.advance_onboarding(Step::Ads, true, &[]).unwrap();
        s.advance_onboarding(Step::Ready, false, &[]).unwrap();
        assert!(s.onboarding.completed);
    }
}
