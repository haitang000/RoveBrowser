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
    args: ICoreWebView2ContextMenuRequestedEventArgs,
    deferral: ICoreWebView2Deferral,
}

impl Request {
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

pub fn register(
    view: &WebView,
    inbox: Inbox,
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
