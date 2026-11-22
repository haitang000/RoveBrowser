use crate::model::Point;
use serde::Serialize;
use std::{cell::RefCell, rc::Rc};
use webview2_com::{
    take_pwstr, ContextMenuRequestedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2ContextMenuItemCollection, ICoreWebView2ContextMenuRequestedEventArgs,
        ICoreWebView2Deferral, ICoreWebView2_11, COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND,
        COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_CHECK_BOX, COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_RADIO,
        COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR, COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SUBMENU,
    },
};
use windows::{
    core::{Interface, BOOL, PWSTR},
    Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::MapWindowPoints,
    },
};
use wry::{WebView, WebViewExtWindows};

pub const READ_ALOUD_SELECTION: i32 = i32::MIN;

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Item {
    Separator,
    Submenu {
        label: String,
        enabled: bool,
        items: Vec<Item>,
    },
    Command {
        id: i32,
        label: String,
        enabled: bool,
        shortcut: String,
        checked: Option<bool>,
        radio: bool,
    },
}

impl Item {
    fn allows(&self, id: i32) -> bool {
        match self {
            Self::Command {
                id: command,
                enabled,
                ..
            } => *enabled && *command == id,
            Self::Submenu { enabled, items, .. } => {
                *enabled && items.iter().any(|item| item.allows(id))
            }
            Self::Separator => false,
        }
    }
}

fn read_items(items: ICoreWebView2ContextMenuItemCollection) -> windows::core::Result<Vec<Item>> {
    unsafe {
        let mut count = 0;
        items.Count(&mut count)?;
        (0..count)
            .map(|index| {
                let item = items.GetValueAtIndex(index)?;
                let mut kind = COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND::default();
                item.Kind(&mut kind)?;
                if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR {
                    return Ok(Item::Separator);
                }
                let mut label = PWSTR::null();
                item.Label(&mut label)?;
                let label = take_pwstr(label);
                let mut enabled = BOOL(0);
                item.IsEnabled(&mut enabled)?;
                let enabled = enabled.as_bool();
                if kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SUBMENU {
                    return Ok(Item::Submenu {
                        label,
                        enabled,
                        items: read_items(item.Children()?)?,
                    });
                }
                let radio = kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_RADIO;
                let checked = if radio || kind == COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_CHECK_BOX {
                    let mut checked = BOOL(0);
                    item.IsChecked(&mut checked)?;
                    Some(checked.as_bool())
                } else {
                    None
                };
                let mut id = -1;
                item.CommandId(&mut id)?;
                let mut shortcut = PWSTR::null();
                item.ShortcutKeyDescription(&mut shortcut)?;
                Ok(Item::Command {
                    id,
                    label,
                    enabled,
                    shortcut: take_pwstr(shortcut),
                    checked,
                    radio,
                })
            })
            .collect()
    }
}

pub struct Request {
    pub source: HWND,
    location: POINT,
    pub items: Vec<Item>,
    pub selection: Option<String>,
    args: ICoreWebView2ContextMenuRequestedEventArgs,
    deferral: ICoreWebView2Deferral,
}

impl Request {
    pub fn allow_read_aloud(&mut self) {
        if self.selection.is_none() { return; }
        self.items.insert(0, Item::Separator);
        self.items.insert(0, Item::Command {
            id: READ_ALOUD_SELECTION,
            label: "朗读所选文本".into(),
            enabled: true,
            shortcut: String::new(),
            checked: None,
            radio: false,
        });
    }

    pub fn take_read_aloud_selection(&mut self) -> Option<String> {
        if self.items.iter().any(|item| item.allows(READ_ALOUD_SELECTION)) {
            self.selection.take()
        } else {
            None
        }
    }

    pub fn anchor(&self, popup: HWND, scale: f64) -> Point {
        let mut origin = POINT::default();
        unsafe {
            MapWindowPoints(
                Some(self.source),
                Some(popup),
                std::slice::from_mut(&mut origin),
            );
        }
        // The request supplies WebView-local DIPs. Only the mapped HWND origin is in physical pixels.
        Point {
            x: f64::from(origin.x) / scale + f64::from(self.location.x),
            y: f64::from(origin.y) / scale + f64::from(self.location.y),
        }
    }

    pub fn select(&self, id: i32) {
        if id >= 0 && self.items.iter().any(|item| item.allows(id)) {
            let _ = unsafe { self.args.SetSelectedCommandId(id) };
        }
    }

    pub fn use_default(&self) {
        let _ = unsafe { self.args.SetHandled(false) };
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        let _ = unsafe { self.deferral.Complete() };
    }
}

pub type Inbox = Rc<RefCell<Option<Request>>>;

fn selected_text(args: &ICoreWebView2ContextMenuRequestedEventArgs) -> windows::core::Result<Option<String>> {
    unsafe {
        let target = args.ContextMenuTarget()?;
        let mut editable = BOOL::default();
        let mut selected = BOOL::default();
        target.IsEditable(&mut editable)?;
        target.HasSelection(&mut selected)?;
        if editable.as_bool() || !selected.as_bool() { return Ok(None); }
        let mut text = PWSTR::null();
        target.SelectionText(&mut text)?;
        let text = take_pwstr(text);
        let text: String = text.trim().chars().take(crate::read_aloud::TEXT_LIMIT).collect();
        Ok((!text.is_empty()).then_some(text))
    }
}

pub fn register(
    view: &WebView,
    inbox: Inbox,
    read_aloud_enabled: impl Fn() -> bool + 'static,
    wake: impl Fn() -> bool + 'static,
) -> windows::core::Result<()> {
    let source = view.hwnd();
    let handler = ContextMenuRequestedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };
        unsafe {
            let items = read_items(args.MenuItems()?)?;
            if items.is_empty() {
                return Ok(());
            }
            let mut location = POINT::default();
            args.Location(&mut location)?;
            let deferral = args.GetDeferral()?;
            let request = Request {
                source,
                location,
                items,
                selection: if read_aloud_enabled() { selected_text(&args).ok().flatten() } else { None },
                args,
                deferral,
            };
            request.args.SetHandled(true)?;
            let previous = inbox.replace(Some(request));
            drop(previous);
            if !wake() {
                if let Some(request) = inbox.take() {
                    request.use_default();
                }
            }
        }
        Ok(())
    }));
    unsafe {
        view.webview()
            .cast::<ICoreWebView2_11>()?
            .add_ContextMenuRequested(&handler, &mut 0)
    }
}
