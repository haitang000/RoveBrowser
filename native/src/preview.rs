//! Resolve hover targets without changing the selected tab or saved library.
use crate::model::{State, Tab};
use std::{cell::Cell, rc::Rc};
use webview2_com::{
    take_pwstr, DOMContentLoadedEventHandler, Microsoft::Web::WebView2::Win32::ICoreWebView2_2,
    NavigationCompletedEventHandler, NavigationStartingEventHandler,
};
use windows::core::{Interface, BOOL, PWSTR};
use wry::{WebView, WebViewExtWindows};

#[derive(Debug)]
pub enum Navigation {
    Started,
    DomReady,
    Completed,
}

pub fn observe_navigation(
    view: &WebView,
    notify: impl Fn(Navigation) + 'static,
) -> windows::core::Result<()> {
    let current = Rc::new(Cell::new(None));
    let notify = Rc::new(notify);
    let starting = NavigationStartingEventHandler::create(Box::new({
        let (current, notify) = (current.clone(), notify.clone());
        move |_, args| {
            let Some(args) = args else { return Ok(()) };
            unsafe {
                let mut cancelled = BOOL::default();
                args.Cancel(&mut cancelled)?;
                if cancelled.as_bool() {
                    return Ok(());
                }
                let mut uri = PWSTR::null();
                args.Uri(&mut uri)?;
                if !crate::model::is_web(&take_pwstr(uri)) {
                    return Ok(());
                }
                let mut id = 0;
                args.NavigationId(&mut id)?;
                current.set(Some(id));
            }
            notify(Navigation::Started);
            Ok(())
        }
    }));
    let ready = DOMContentLoadedEventHandler::create(Box::new({
        let (current, notify) = (current.clone(), notify.clone());
        move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut id = 0;
            unsafe {
                args.NavigationId(&mut id)?;
            }
            if current.get() == Some(id) {
                notify(Navigation::DomReady);
            }
            Ok(())
        }
    }));
    let completed = NavigationCompletedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else { return Ok(()) };
        let mut id = 0;
        unsafe {
            args.NavigationId(&mut id)?;
        }
        if current.get() == Some(id) {
            current.set(None);
            notify(Navigation::Completed);
        }
        Ok(())
    }));
    unsafe {
        let web = view.webview();
        web.add_NavigationStarting(&starting, &mut 0)?;
        web.cast::<ICoreWebView2_2>()?
            .add_DOMContentLoaded(&ready, &mut 0)?;
        web.add_NavigationCompleted(&completed, &mut 0)?;
    }
    Ok(())
}

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
                        muted: false,
                        zoom: None,
                        custom_title: None,
                        last_activated: crate::model::now_secs(),
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
