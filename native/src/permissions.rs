//! Site permission prompts adapted from Zigzag's `SitePermissions.swift`.
use crate::storage::Storage;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::{Rc, Weak},
};
use webview2_com::{
    take_pwstr, ContentLoadingEventHandler, FrameChildFrameCreatedEventHandler,
    FrameContentLoadingEventHandler, FrameCreatedEventHandler, FrameNavigationStartingEventHandler,
    FramePermissionRequestedEventHandler, Microsoft::Web::WebView2::Win32::*,
    PermissionRequestedEventHandler, ScreenCaptureStartingEventHandler,
};
use windows::core::{Interface, PWSTR};
use wry::{WebView, WebViewExtWindows};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Camera,
    Microphone,
    Location,
    Notifications,
    Screen,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DefaultAction {
    #[default]
    Ask,
    Block,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Defaults {
    camera: DefaultAction,
    microphone: DefaultAction,
    location: DefaultAction,
    notifications: DefaultAction,
    screen: DefaultAction,
}

impl Defaults {
    fn blocks(&self, kind: Kind) -> bool {
        let action = match kind {
            Kind::Camera => self.camera,
            Kind::Microphone => self.microphone,
            Kind::Location => self.location,
            Kind::Notifications => self.notifications,
            Kind::Screen => self.screen,
        };
        action == DefaultAction::Block
    }
}

impl Kind {
    fn from_webview(kind: COREWEBVIEW2_PERMISSION_KIND) -> Option<Self> {
        match kind {
            COREWEBVIEW2_PERMISSION_KIND_CAMERA => Some(Self::Camera),
            COREWEBVIEW2_PERMISSION_KIND_MICROPHONE => Some(Self::Microphone),
            COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION => Some(Self::Location),
            COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS => Some(Self::Notifications),
            _ => None,
        }
    }

    pub fn remembers_by_default(self) -> bool {
        self != Self::Screen
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Scope {
    pub page: String,
    pub requester: String,
    pub kind: Kind,
}

impl Scope {
    fn new(page: &str, requester: &str, kind: Kind) -> Option<Self> {
        Some(Self {
            page: origin(page)?,
            requester: origin(requester)?,
            kind,
        })
    }

    pub fn can_remember(&self) -> bool {
        self.kind != Kind::Screen
            || (self.page == self.requester && self.page.starts_with("https://"))
    }
}

fn origin(value: &str) -> Option<String> {
    use url::Origin;
    let origin = url::Url::parse(value).ok()?.origin();
    match &origin {
        Origin::Tuple(scheme, _, _) if matches!(scheme.as_str(), "http" | "https") => {
            Some(origin.ascii_serialization())
        }
        _ => None,
    }
}

#[derive(Default)]
pub struct Document {
    generation: Cell<u64>,
    answers: RefCell<HashMap<Scope, bool>>,
}

impl Document {
    fn reset(&self) {
        self.generation.set(self.generation.get() + 1);
        self.answers.borrow_mut().clear();
    }

    pub fn forget(&self, scope: &Scope) {
        self.answers.borrow_mut().remove(scope);
    }
}

// Dropping requests on navigation, tab close or shutdown completes the native deferral.
struct Completion(Option<Box<dyn FnOnce(bool)>>);

impl Completion {
    fn finish(mut self, allowed: bool) {
        if let Some(callback) = self.0.take() {
            callback(allowed);
        }
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            callback(false);
        }
    }
}

pub struct Request {
    pub id: String,
    pub tab: String,
    pub scope: Scope,
    document: Weak<Document>,
    generation: u64,
    completions: Vec<Completion>,
}

impl Request {
    fn live_document(&self) -> Option<Rc<Document>> {
        self.document
            .upgrade()
            .filter(|doc| doc.generation.get() == self.generation)
    }

    pub fn belongs_to(&self, document: &Rc<Document>) -> bool {
        self.document.as_ptr() == Rc::as_ptr(document)
    }

    pub fn answer(self, allowed: bool, for_document: bool) {
        let document = self.live_document();
        if let Some(document) = &document {
            if for_document {
                document.answers.borrow_mut().insert(self.scope, allowed);
            }
        }
        for completion in self.completions {
            completion.finish(allowed && document.is_some());
        }
    }
}

#[derive(Clone, Default)]
pub struct Inbox(Rc<RefCell<VecDeque<Request>>>);

impl Inbox {
    fn push(&self, tab: &str, document: &Rc<Document>, scope: Scope, completion: Completion) {
        let mut queue = self.0.borrow_mut();
        let generation = document.generation.get();
        if let Some(existing) = queue
            .iter_mut()
            .find(|r| r.belongs_to(document) && r.generation == generation && r.scope == scope)
        {
            if existing.completions.len() < 16 {
                existing.completions.push(completion);
            }
        } else if queue.len() < 64 {
            queue.push_back(Request {
                id: uuid::Uuid::new_v4().to_string(),
                tab: tab.into(),
                scope,
                document: Rc::downgrade(document),
                generation,
                completions: vec![completion],
            });
        }
    }

    pub fn take(&self) -> VecDeque<Request> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

#[derive(Default)]
pub struct Requests {
    pub inbox: Inbox,
    pub waiting: VecDeque<Request>,
    rules: HashMap<Scope, bool>,
}

impl Requests {
    pub fn load(storage: &Storage) -> rusqlite::Result<Self> {
        let mut query = storage
            .conn
            .prepare("SELECT scope, allowed FROM site_permissions")?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
        })?;
        let mut result = Self::default();
        for row in rows {
            let (key, allowed) = row?;
            if let Ok(scope) = serde_json::from_str::<Scope>(&key) {
                result.rules.insert(scope, allowed);
            }
        }
        Ok(result)
    }

    pub fn update(
        &mut self,
        storage: &Storage,
        scope: Scope,
        allowed: Option<bool>,
    ) -> rusqlite::Result<()> {
        if !scope.can_remember()
            || Scope::new(&scope.page, &scope.requester, scope.kind).as_ref() != Some(&scope)
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let key = serde_json::to_string(&scope).expect("scope contains only strings and an enum");
        if let Some(allowed) = allowed {
            storage.conn.execute("INSERT INTO site_permissions(scope,allowed) VALUES(?1,?2) ON CONFLICT(scope) DO UPDATE SET allowed=excluded.allowed", params![key, allowed])?;
            self.rules.insert(scope, allowed);
        } else {
            storage
                .conn
                .execute("DELETE FROM site_permissions WHERE scope=?1", [key])?;
            self.rules.remove(&scope);
        }
        Ok(())
    }

    pub fn rules(&self) -> Vec<(&Scope, bool)> {
        let mut rows: Vec<_> = self
            .rules
            .iter()
            .map(|(scope, allowed)| (scope, *allowed))
            .collect();
        rows.sort_by_key(|(scope, _)| *scope);
        rows
    }

    pub fn reconcile(&mut self, defaults: &Defaults) {
        for incoming in self.inbox.take() {
            if let Some(existing) = self.waiting.iter_mut().find(|r| {
                r.document.ptr_eq(&incoming.document)
                    && r.generation == incoming.generation
                    && r.scope == incoming.scope
            }) {
                let remaining = 16usize.saturating_sub(existing.completions.len());
                existing
                    .completions
                    .extend(incoming.completions.into_iter().take(remaining));
            } else if self.waiting.len() < 64 {
                self.waiting.push_back(incoming);
            }
        }
        self.waiting = std::mem::take(&mut self.waiting)
            .into_iter()
            .filter_map(|request| {
                let document = request.live_document()?;
                let known = self
                    .rules
                    .get(&request.scope)
                    .copied()
                    .or_else(|| defaults.blocks(request.scope.kind).then_some(false))
                    .or_else(|| document.answers.borrow().get(&request.scope).copied());
                if let Some(allowed) = known {
                    request.answer(allowed, false);
                    None
                } else {
                    Some(request)
                }
            })
            .collect();
    }
}

fn web_source(sender: &ICoreWebView2) -> windows::core::Result<String> {
    let mut value = PWSTR::null();
    unsafe {
        sender.Source(&mut value)?;
    }
    Ok(take_pwstr(value))
}

fn capture_request(
    args: ICoreWebView2PermissionRequestedEventArgs,
    page: &str,
    frame: Option<&str>,
    enqueue: &impl Fn(&str, &str, Kind, Completion),
) -> windows::core::Result<()> {
    let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
    unsafe {
        args.PermissionKind(&mut kind)?;
    }
    let Some(kind) = Kind::from_webview(kind) else {
        return Ok(());
    };
    unsafe {
        if frame.is_some() {
            args.cast::<ICoreWebView2PermissionRequestedEventArgs2>()?
                .SetHandled(true)?;
        }
        // The profile store keys by requester alone. Keep our decisions separately.
        let Ok(profile) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() else {
            return Ok(());
        };
        profile.SetSavesInProfile(false)?;
        args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
        let mut requester = PWSTR::null();
        args.Uri(&mut requester)?;
        let requester = take_pwstr(requester);
        // Chromium can describe an iframe's delegated request using the top origin.
        let requester = if origin(&requester) == origin(page) {
            frame.unwrap_or(&requester)
        } else {
            &requester
        };
        let deferral = args.GetDeferral()?;
        enqueue(
            page,
            requester,
            kind,
            Completion(Some(Box::new(move |allow| {
                if allow {
                    let _ = args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW);
                }
                let _ = deferral.Complete();
            }))),
        );
    }
    Ok(())
}

fn register_frame(
    frame: ICoreWebView2Frame,
    page_source: Rc<RefCell<String>>,
    enqueue: impl Fn(&str, &str, Kind, Completion) + Clone + 'static,
) -> windows::core::Result<()> {
    let navigation = Rc::new(RefCell::new(None));
    let frame_source = Rc::new(RefCell::new(String::new()));
    let starting = FrameNavigationStartingEventHandler::create(Box::new({
        let navigation = navigation.clone();
        move |_, args| {
            if let Some(args) = args {
                let mut uri = PWSTR::null();
                let mut id = 0;
                unsafe {
                    args.NavigationId(&mut id)?;
                    args.Uri(&mut uri)?;
                }
                *navigation.borrow_mut() = Some((id, take_pwstr(uri)));
            }
            Ok(())
        }
    }));
    let loading = FrameContentLoadingEventHandler::create(Box::new({
        let frame_source = frame_source.clone();
        move |_, args| {
            if let Some(args) = args {
                let mut id = 0;
                unsafe {
                    args.NavigationId(&mut id)?;
                }
                let mut navigation = navigation.borrow_mut();
                *frame_source.borrow_mut() = navigation
                    .take_if(|(expected, _)| *expected == id)
                    .map(|(_, uri)| uri)
                    .unwrap_or_default();
            }
            Ok(())
        }
    }));
    let requested = FramePermissionRequestedEventHandler::create(Box::new({
        let page_source = page_source.clone();
        let enqueue = enqueue.clone();
        move |_, args| {
            let Some(args) = args else { return Ok(()) };
            capture_request(
                args.cast()?,
                &page_source.borrow(),
                Some(&frame_source.borrow()),
                &enqueue,
            )
        }
    }));
    unsafe {
        let events = frame.cast::<ICoreWebView2Frame2>()?;
        events.add_NavigationStarting(&starting, &mut 0)?;
        events.add_ContentLoading(&loading, &mut 0)?;
        frame
            .cast::<ICoreWebView2Frame3>()?
            .add_PermissionRequested(&requested, &mut 0)?;
    }
    if let Ok(nested) = frame.cast::<ICoreWebView2Frame7>() {
        let created = FrameChildFrameCreatedEventHandler::create(Box::new(move |_, args| {
            if let Some(args) = args {
                register_frame(
                    unsafe { args.Frame()? },
                    page_source.clone(),
                    enqueue.clone(),
                )?;
            }
            Ok(())
        }));
        unsafe {
            nested.add_FrameCreated(&created, &mut 0)?;
        }
    }
    Ok(())
}

pub fn register(
    view: &WebView,
    tab: String,
    document: &Rc<Document>,
    inbox: Inbox,
    notify: impl Fn() + Clone + 'static,
) -> windows::core::Result<()> {
    let web = view.webview();
    let document = Rc::downgrade(document);
    let page_source = Rc::new(RefCell::new(String::new()));
    let loading = ContentLoadingEventHandler::create(Box::new({
        let document = document.clone();
        let notify = notify.clone();
        let page_source = page_source.clone();
        move |sender, _| {
            if let Some(document) = document.upgrade() {
                document.reset();
            }
            *page_source.borrow_mut() = sender
                .as_ref()
                .and_then(|web| web_source(web).ok())
                .unwrap_or_default();
            notify();
            Ok(())
        }
    }));
    let enqueue = move |page: &str, requester: &str, kind, completion| {
        if let (Some(document), Some(scope)) =
            (document.upgrade(), Scope::new(page, requester, kind))
        {
            inbox.push(&tab, &document, scope, completion);
            notify();
        }
    };
    let requested = PermissionRequestedEventHandler::create(Box::new({
        let enqueue = enqueue.clone();
        move |sender, args| {
            let (Some(sender), Some(args)) = (sender, args) else {
                return Ok(());
            };
            capture_request(args, &web_source(&sender)?, None, &enqueue)?;
            Ok(())
        }
    }));
    let frames = FrameCreatedEventHandler::create(Box::new({
        let enqueue = enqueue.clone();
        move |sender, args| {
            if let (Some(sender), Some(args)) = (sender, args) {
                *page_source.borrow_mut() = web_source(&sender)?;
                register_frame(
                    unsafe { args.Frame()? },
                    page_source.clone(),
                    enqueue.clone(),
                )?;
            }
            Ok(())
        }
    }));
    unsafe {
        web.add_ContentLoading(&loading, &mut 0)?;
        web.add_PermissionRequested(&requested, &mut 0)?;
        web.cast::<ICoreWebView2_4>()?
            .add_FrameCreated(&frames, &mut 0)?;
    }
    if let Ok(screen) = web.cast::<ICoreWebView2_27>() {
        let starting = ScreenCaptureStartingEventHandler::create(Box::new(move |sender, args| {
            let (Some(sender), Some(args)) = (sender, args) else {
                return Ok(());
            };
            unsafe {
                args.SetCancel(true)?;
                let page = web_source(&sender)?;
                let mut requester = PWSTR::null();
                args.OriginalSourceFrameInfo()?.Source(&mut requester)?;
                let requester = take_pwstr(requester);
                let deferral = args.GetDeferral()?;
                enqueue(
                    &page,
                    &requester,
                    Kind::Screen,
                    Completion(Some(Box::new(move |allow| {
                        if allow {
                            let _ = args.SetCancel(false);
                        }
                        let _ = deferral.Complete();
                    }))),
                );
            }
            Ok(())
        }));
        unsafe {
            screen.add_ScreenCaptureStarting(&starting, &mut 0)?;
        }
    }
    Ok(())
}

pub fn deny_preview_requests(view: &WebView) -> windows::core::Result<()> {
    let web = view.webview();
    let denied = PermissionRequestedEventHandler::create(Box::new(|_, args| {
        if let Some(args) = args {
            if let Ok(profile) = args.cast::<ICoreWebView2PermissionRequestedEventArgs3>() {
                unsafe {
                    profile.SetSavesInProfile(false)?;
                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
                }
            }
        }
        Ok(())
    }));
    unsafe {
        web.add_PermissionRequested(&denied, &mut 0)?;
    }
    if let Ok(screen) = web.cast::<ICoreWebView2_27>() {
        let denied = ScreenCaptureStartingEventHandler::create(Box::new(|_, args| {
            if let Some(args) = args {
                unsafe {
                    args.SetCancel(true)?;
                }
            }
            Ok(())
        }));
        unsafe {
            screen.add_ScreenCaptureStarting(&denied, &mut 0)?;
        }
    }
    Ok(())
}
