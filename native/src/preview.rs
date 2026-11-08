//! Resolve hover targets without changing the selected tab or saved library.
use crate::model::{State, Tab};

pub fn targets(state: &State, id: &str) -> Option<Vec<Tab>> {
    let mut tabs = if let Some(mark) = state.bookmark(id) {
        if mark
            .space_id
            .as_ref()
            .is_some_and(|s| s != &state.active_space)
        {
            return None;
        }
        state
            .pins
            .iter()
            .chain(&state.favorites)
            .filter(|f| f.id == id || Some(&f.id) == mark.split_with.as_ref())
            .map(|f| {
                state
                    .tabs
                    .iter()
                    .find(|t| t.bookmark_id.as_ref() == Some(&f.id))
                    .cloned()
                    .unwrap_or_else(|| Tab {
                        id: f.id.clone(),
                        space_id: f
                            .space_id
                            .clone()
                            .unwrap_or_else(|| state.active_space.clone()),
                        title: f.title.clone(),
                        url: f.url.clone(),
                        pinned: f.space_id.is_some(),
                        bookmark_id: Some(f.id.clone()),
                        loading: false,
                        favicon: f.favicon.clone(),
                        split_with: None,
                    })
            })
            .collect::<Vec<_>>()
    } else {
        let tab = state
            .tabs
            .iter()
            .find(|t| t.id == id && t.space_id == state.active_space)?;
        vec![tab.clone()]
    };
    if tabs.len() == 1 {
        let first = &tabs[0];
        if let Some(other) = state.tabs.iter().find(|t| {
            Some(&t.id) == first.split_with.as_ref() || t.split_with.as_ref() == Some(&first.id)
        }) {
            tabs.push(other.clone());
        }
    }
    if tabs.iter().any(|t| {
        crate::settings::is_settings(&t.url) || t.id == state.active_tab
            || state.active().and_then(|a| a.split_with.as_ref()) == Some(&t.id)
    }) {
        return None;
    }
    Some(tabs)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unloaded_saved_targets_are_transient_and_active_pairs_are_excluded() {
        let mut s = State::default();
        let before = s.tabs.len();
        let mark = s.favorites[0].clone();
        let found = targets(&s, &mark.id).unwrap();
        assert_eq!(found[0].id, mark.id);
        assert_eq!(found[0].url, mark.url);
        assert_eq!(s.tabs.len(), before);
        let active = s.active_tab.clone();
        let other = s.tabs.iter().find(|t| t.id != active).unwrap().id.clone();
        s.tabs
            .iter_mut()
            .find(|t| t.id == active)
            .unwrap()
            .split_with = Some(other.clone());
        assert!(targets(&s, &other).is_none());
    }
}
