//! Native split dragging adapted from Zigzag's ResizeHandle, paneFrames and
//! PageCardView.layout (Copyright 2026 Zigao Wang; Apache-2.0), modified for Win32.
//! Mouse input transforms frozen page snapshots; release commits each viewport
//! once and blends both new layouts through the same presentation animation.
use crate::{clip::WindowClip, model::Rect};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
use windows::{
    core::{w, Error, Result},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{BeginPaint, EndPaint, MapWindowPoints, PAINTSTRUCT},
        UI::{
            Controls::WM_MOUSELEAVE,
            Input::KeyboardAndMouse::{
                GetCapture, ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
            },
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
};
const SUBCLASS: usize = 0x53504c54;
const DRAG_THRESHOLD: f64 = 4.;

/// Keep the visible edges and the renderer dimensions in one shared cache.
/// The host's ordinary layout and the native input handler use the same values.
#[derive(Clone)]
pub struct Pane {
    pub hwnd: HWND,
    pub surface: Rc<crate::resize_surface::Surface>,
    pub controller: ICoreWebView2Controller,
    pub clip: Rc<WindowClip>,
    pub bounds: Rc<Cell<Option<[i32; 4]>>>,
    pub viewport: Rc<Cell<Option<[i32; 2]>>>,
}
impl Pane {
    pub(crate) fn begin_preview(&self, rect: Rect, scale: f64) {
        if let Some(bounds) = self.bounds.get() {
            let _ = self.clip.clear(self.hwnd);
            if let Err(error) = self.surface.begin(bounds, rect.corners, scale) {
                eprintln!("Rove resize surface unavailable: {error}");
            }
        }
    }
    pub(crate) fn end_preview(&self, animate: bool) -> Result<()> {
        if self.surface.active() {
            let result = (|| -> Result<()> {
                if let Some([x, y, width, height]) = self.bounds.get() {
                    unsafe {
                        SetWindowPos(
                            self.hwnd,
                            None,
                            x,
                            y,
                            width,
                            height,
                            SWP_NOACTIVATE | SWP_NOZORDER,
                        )?;
                        self.controller.SetBounds(RECT {
                            left: 0,
                            top: 0,
                            right: width,
                            bottom: height,
                        })?;
                    }
                    self.viewport.set(Some([width, height]));
                }
                Ok(())
            })();
            if result.is_ok() && animate {
                self.surface.release();
            } else {
                self.surface.end();
            }
            result?;
        }
        Ok(())
    }
    pub(crate) fn move_window(&self, rect: Rect, scale: f64) -> Result<()> {
        self.clip.clear(self.hwnd)?;
        let next = [
            (rect.x * scale).round() as i32,
            (rect.y * scale).round() as i32,
            (rect.width * scale).round() as i32,
            (rect.height * scale).round() as i32,
        ];
        if self.bounds.get() != Some(next) {
            if self.surface.dragging() {
                self.surface.resize(next)?;
                self.bounds.set(Some(next));
                return Ok(());
            }
            self.surface.end();
            // SAFETY: these are live same-thread child HWNDs. Dimensions derive
            // from the validated page area, including the current monitor DPI.
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    None,
                    next[0],
                    next[1],
                    next[2],
                    next[3],
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )?;
            }
            self.bounds.set(Some(next));
        }
        Ok(())
    }
    pub(crate) fn reflow(&self) -> Result<()> {
        if self.surface.dragging() {
            return Ok(());
        }
        if let Some([_, _, width, height]) = self.bounds.get() {
            if self.viewport.get() != Some([width, height]) {
                unsafe {
                    self.controller.SetBounds(RECT {
                        left: 0,
                        top: 0,
                        right: width,
                        bottom: height,
                    })?;
                }
                self.viewport.set(Some([width, height]));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub primary: Rect,
    pub secondary: Rect,
}
impl Layout {
    fn gap(self) -> f64 {
        self.secondary.x - self.primary.x - self.primary.width
    }
    fn width(self) -> f64 {
        self.primary.width + self.secondary.width
    }
    pub fn ratio(self) -> f64 {
        self.primary.width / self.width().max(1.)
    }
    pub fn swapped(self) -> Self {
        Self {
            primary: Rect { width: self.secondary.width, ..self.primary },
            secondary: Rect {
                x: self.primary.x + self.secondary.width + self.gap(),
                width: self.primary.width,
                ..self.secondary
            },
        }
    }
    /// Zigzag: a 20–80% share, at least 260 points per pane, equal halves when
    /// the available width cannot fit those minima. Round once, share the seam.
    fn resized(self, x: f64) -> (Self, f64) {
        let width = self.width();
        let ratio = crate::layout::clamp_split_ratio((x - self.primary.x) / width.max(1.), width);
        let left_width = (width * ratio).floor();
        let layout = Self {
            primary: Rect {
                width: left_width,
                ..self.primary
            },
            secondary: Rect {
                x: self.primary.x + left_width + self.gap(),
                width: width - left_width,
                ..self.secondary
            },
        };
        (layout, ratio)
    }
}
#[derive(Clone)]
pub struct Config {
    pub id: String,
    pub other: String,
    pub layout: Layout,
    pub scale: f64,
    pub left: Option<Pane>,
    pub right: Option<Pane>,
}
pub struct Update {
    pub id: String,
    pub other: String,
    pub layout: Layout,
    pub resizing: bool,
    pub committed_ratio: Option<f64>,
    pub error: Option<String>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Guide {
    Hover(bool),
    Click,
}
struct State {
    root: HWND,
    popup: HWND,
    hwnd: Cell<HWND>,
    config: RefCell<Option<Config>>,
    pending: RefCell<Option<Update>>,
    notify: Box<dyn Fn()>,
    guide: Box<dyn Fn(Guide)>,
    hovered: Cell<bool>,
    pressed: Cell<Option<POINT>>,
    dragging: Cell<bool>,
    busy: Cell<bool>,
    grab_offset: Cell<f64>,
    changed_ratio: Cell<Option<f64>>,
}
pub struct SplitHandle {
    state: Box<State>,
}
impl SplitHandle {
    pub fn new(
        root: HWND,
        popup: HWND,
        notify: impl Fn() + 'static,
        guide: impl Fn(Guide) + 'static,
    ) -> Result<Self> {
        let state = Box::new(State {
            root,
            popup,
            hwnd: Cell::new(HWND::default()),
            config: RefCell::new(None),
            pending: RefCell::new(None),
            notify: Box::new(notify),
            guide: Box::new(guide),
            hovered: Cell::new(false),
            pressed: Cell::new(None),
            dragging: Cell::new(false),
            busy: Cell::new(false),
            grab_offset: Cell::new(0.),
            changed_ratio: Cell::new(None),
        });
        // The native strip owns pointer input. The shell draws the themed guide.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
                w!("STATIC"),
                w!(""),
                WS_CHILD,
                0,
                0,
                0,
                0,
                Some(root),
                None,
                None,
                None,
            )?
        };
        state.hwnd.set(hwnd);
        if !unsafe {
            SetWindowSubclass(
                hwnd,
                Some(procedure),
                SUBCLASS,
                (&*state as *const State) as usize,
            )
        }
        .as_bool()
        {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return Err(Error::from_thread());
        }
        Ok(Self { state })
    }
    pub fn dragging(&self) -> bool {
        self.state.dragging.get()
    }
    pub fn take_update(&self) -> Option<Update> {
        self.state.pending.borrow_mut().take()
    }
    pub fn configure(&self, config: Option<Config>) -> Result<()> {
        if self.dragging() {
            return Ok(());
        }
        *self.state.config.borrow_mut() = config;
        if let Some(config) = self.state.config.borrow().as_ref() {
            place_handle(&self.state, config)?;
        } else {
            finish(&self.state);
            set_hovered(&self.state, false);
            unsafe {
                let _ = ShowWindow(self.state.hwnd.get(), SW_HIDE);
            }
        }
        Ok(())
    }
    pub fn cancel(&self) {
        finish(&self.state);
    }
}
impl Drop for SplitHandle {
    fn drop(&mut self) {
        self.state.dragging.set(false);
        unsafe {
            if GetCapture() == self.state.hwnd.get() {
                let _ = ReleaseCapture();
            }
            let _ = RemoveWindowSubclass(self.state.hwnd.get(), Some(procedure), SUBCLASS);
            let _ = DestroyWindow(self.state.hwnd.get());
        }
    }
}
fn set_hovered(s: &State, hovered: bool) {
    if s.hovered.replace(hovered) == hovered {
        return;
    }
    if hovered {
        let mut track = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: s.hwnd.get(),
            ..Default::default()
        };
        let _ = unsafe { TrackMouseEvent(&mut track) };
    }
    (s.guide)(Guide::Hover(hovered));
}
fn place_handle(s: &State, config: &Config) -> Result<()> {
    let left = (config.layout.primary.x + config.layout.primary.width) * config.scale;
    let right = config.layout.secondary.x * config.scale;
    unsafe {
        SetWindowPos(
            s.hwnd.get(),
            Some(s.popup),
            left.round() as i32,
            ((config.layout.primary.y - 30.) * config.scale).round() as i32,
            (right - left).round().max(1.) as i32,
            ((config.layout.primary.height + 30.) * config.scale).round() as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )?;
    }
    Ok(())
}
fn report(s: &State, config: &Config, resizing: bool, error: Option<String>) {
    let previous = s.pending.borrow_mut().replace(Update {
        id: config.id.clone(),
        other: config.other.clone(),
        layout: config.layout,
        resizing,
        committed_ratio: if resizing { None } else { s.changed_ratio.take() },
        error,
    });
    if previous.is_none() {
        (s.notify)();
    }
}
fn mouse_point(lparam: LPARAM) -> POINT {
    POINT {
        x: lparam.0 as u16 as i16 as i32,
        y: (lparam.0 >> 16) as u16 as i16 as i32,
    }
}
fn root_point(s: &State, point: POINT) -> POINT {
    let mut point = [point];
    unsafe {
        MapWindowPoints(Some(s.hwnd.get()), Some(s.root), &mut point);
    }
    point[0]
}
fn apply(s: &State, config: &Config) -> Result<()> {
    if let Some(pane) = &config.left {
        pane.move_window(config.layout.primary, config.scale)?;
    }
    if let Some(pane) = &config.right {
        pane.move_window(config.layout.secondary, config.scale)?;
    }
    place_handle(s, config)?;
    if let Some(pane) = &config.left {
        pane.reflow()?;
    }
    if let Some(pane) = &config.right {
        pane.reflow()?;
    }
    Ok(())
}
fn move_to(s: &State, x: f64) {
    if s.busy.replace(true) {
        return;
    }
    let config = s.config.borrow().clone();
    if let Some(mut config) = config {
        let (next, ratio) = config.layout.resized(x - s.grab_offset.get());
        if next.primary.width != config.layout.primary.width {
            s.changed_ratio.set(Some(ratio));
            config.layout = next;
            *s.config.borrow_mut() = Some(config.clone());
            let error = apply(s, &config).err().map(|e| e.to_string());
            report(s, &config, s.dragging.get(), error);
        }
    }
    s.busy.set(false);
}
fn drag(s: &State, point: POINT) {
    let config = s.config.borrow().clone();
    let Some(config) = config else { return; };
    if !s.dragging.get() {
        let Some(pressed) = s.pressed.get() else { return; };
        if ((point.x - pressed.x).abs() as f64) < DRAG_THRESHOLD * config.scale { return; }
        for (pane, rect) in [(&config.left, config.layout.primary), (&config.right, config.layout.secondary)] {
            if let Some(pane) = pane { pane.begin_preview(rect, config.scale); }
        }
        let x = root_point(s, pressed).x as f64 / config.scale;
        s.grab_offset.set(x - config.layout.primary.x - config.layout.primary.width);
        s.changed_ratio.set(None);
        s.dragging.set(true);
        report(s, &config, true, None);
    }
    let point = root_point(s, point);
    for pane in [&config.left, &config.right].into_iter().flatten() {
        pane.surface.pointer(point.x as f32, point.y as f32);
    }
    move_to(s, point.x as f64 / config.scale);
}
fn finish(s: &State) {
    let pressed = s.pressed.take().is_some();
    let dragging = s.dragging.replace(false);
    if !pressed && !dragging { return; }
    unsafe {
        if GetCapture() == s.hwnd.get() {
            let _ = ReleaseCapture();
        }
        let mut point = POINT::default();
        set_hovered(
            s,
            GetCursorPos(&mut point).is_ok() && WindowFromPoint(point) == s.hwnd.get(),
        );
    }
    if !dragging { return; }
    let config = s.config.borrow().clone();
    if let Some(config) = config {
        let restore = || -> Result<()> {
            let left = config
                .left
                .as_ref()
                .map(|pane| pane.end_preview(true))
                .transpose();
            let right = config
                .right
                .as_ref()
                .map(|pane| pane.end_preview(true))
                .transpose();
            left?;
            right?;
            if let Some(pane) = &config.left {
                pane.clip
                    .apply(pane.hwnd, config.layout.primary.corners, config.scale)?;
            }
            if let Some(pane) = &config.right {
                pane.clip
                    .apply(pane.hwnd, config.layout.secondary.corners, config.scale)?;
            }
            Ok(())
        };
        report(s, &config, false, restore().err().map(|e| e.to_string()));
    }
}
unsafe extern "system" fn procedure(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: SplitHandle owns the stable Box until the subclass is removed.
    let s = &*(data as *const State);
    match msg {
        WM_NCHITTEST => return LRESULT(HTCLIENT as isize),
        WM_SETCURSOR => {
            SetCursor(LoadCursorW(None, IDC_SIZEWE).ok());
            return LRESULT(1);
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut paint);
            let _ = EndPaint(hwnd, &paint);
            return LRESULT(0);
        }
        WM_LBUTTONDOWN => {
            if s.config.borrow().is_some() {
                s.pressed.set(Some(mouse_point(lparam)));
                SetCapture(hwnd);
            }
            return LRESULT(0);
        }
        WM_MOUSEMOVE if s.pressed.get().is_some() => {
            drag(s, mouse_point(lparam));
            return LRESULT(0);
        }
        WM_MOUSEMOVE => {
            set_hovered(s, true);
            return LRESULT(0);
        }
        WM_MOUSELEAVE => {
            set_hovered(s, false);
            return LRESULT(0);
        }
        WM_LBUTTONUP => {
            let point = mouse_point(lparam);
            drag(s, point);
            let clicked = !s.dragging.get() && s.pressed.get().is_some_and(|pressed| {
                let threshold = s.config.borrow().as_ref().map_or(0., |c| DRAG_THRESHOLD * c.scale);
                ((point.y - pressed.y).abs() as f64) < threshold
            });
            finish(s);
            if clicked { (s.guide)(Guide::Click); }
            return LRESULT(0);
        }
        WM_CAPTURECHANGED | WM_CANCELMODE => {
            finish(s);
            return LRESULT(0);
        }
        WM_KEYDOWN if wparam.0 == 27 => {
            finish(s);
            return LRESULT(0);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zigzag_ratio_limits_minimum_panes_and_shared_seam() {
        let layout = Layout {
            primary: Rect {
                x: 241.,
                y: 63.,
                width: 511.,
                height: 748.,
                ..Rect::default()
            },
            secondary: Rect {
                x: 760.,
                y: 63.,
                width: 511.,
                height: 748.,
                ..Rect::default()
            },
        };
        for x in [-500., 241., 700.3, 1271., 3000.] {
            let (next, _) = layout.resized(x);
            assert!(next.primary.width >= 260. && next.secondary.width >= 260.);
            assert_eq!(next.gap(), 8.);
            assert_eq!(next.secondary.x + next.secondary.width, 1271.);
            assert_eq!(next.primary.width.fract(), 0.);
        }
        let narrow = Layout {
            primary: Rect {
                width: 100.,
                ..layout.primary
            },
            secondary: Rect {
                x: 349.,
                width: 100.,
                ..layout.secondary
            },
        };
        assert_eq!(narrow.resized(-500.).0.ratio(), 0.5);
        assert_eq!(narrow.resized(3000.).0.ratio(), 0.5);
    }
    #[test]
    fn native_mouse_capture_updates_frames_before_shell_feedback_and_cancels() {
        struct Window(HWND);
        impl Drop for Window {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        let root = Window(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("split test"),
                WS_POPUP,
                0,
                0,
                1280,
                820,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        });
        let popup = Window(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("popup test"),
                WS_CHILD,
                0,
                0,
                1280,
                820,
                Some(root.0),
                None,
                None,
                None,
            )
            .unwrap()
        });
        let notifications = Rc::new(Cell::new(0));
        let count = notifications.clone();
        let hover_events = Rc::new(RefCell::new(Vec::new()));
        let events = hover_events.clone();
        let handle = SplitHandle::new(
            root.0,
            popup.0,
            move || count.set(count.get() + 1),
            move |guide| events.borrow_mut().push(guide),
        )
        .unwrap();
        let config = Config {
            id: "active".into(),
            other: "other".into(),
            scale: 1.,
            left: None,
            right: None,
            layout: Layout {
                primary: Rect {
                    x: 241.,
                    y: 63.,
                    width: 511.,
                    height: 748.,
                    ..Rect::default()
                },
                secondary: Rect {
                    x: 760.,
                    y: 63.,
                    width: 511.,
                    height: 748.,
                    ..Rect::default()
                },
            },
        };
        handle.configure(Some(config.clone())).unwrap();
        let point = |x: i16| LPARAM((100 << 16) | x as u16 as isize);
        let hwnd = handle.state.hwnd.get();
        unsafe {
            SendMessageW(hwnd, WM_MOUSEMOVE, None, Some(point(1)));
            SendMessageW(hwnd, WM_MOUSEMOVE, None, Some(point(2)));
            SendMessageW(hwnd, WM_MOUSELEAVE, None, None);
        }
        assert_eq!(*hover_events.borrow(), [Guide::Hover(true), Guide::Hover(false)]);
        unsafe {
            SendMessageW(hwnd, WM_LBUTTONDOWN, None, Some(point(1)));
            SendMessageW(hwnd, WM_MOUSEMOVE, None, Some(point(2)));
            SendMessageW(hwnd, WM_LBUTTONUP, None, Some(point(2)));
        }
        assert_eq!(hover_events.borrow().last(), Some(&Guide::Click));
        assert_eq!(notifications.get(), 0);
        unsafe {
            SendMessageW(hwnd, WM_LBUTTONDOWN, None, Some(point(1)));
        }
        assert!(!handle.dragging());
        assert_eq!(unsafe { GetCapture() }, handle.state.hwnd.get());
        unsafe {
            SendMessageW(
                handle.state.hwnd.get(),
                WM_MOUSEMOVE,
                None,
                Some(point(100)),
            );
        }
        assert!(handle.dragging());
        let live_update = handle.take_update().unwrap();
        assert!(live_update.resizing);
        assert_eq!(live_update.committed_ratio, None);
        // Config changed inside the mouse callback, before the app handles any
        // notification and without a UI renderer or IPC handler in this test.
        assert_eq!(
            handle
                .state
                .config
                .borrow()
                .as_ref()
                .unwrap()
                .layout
                .primary
                .width,
            610.
        );
        let mut actual = RECT::default();
        unsafe {
            GetWindowRect(handle.state.hwnd.get(), &mut actual).unwrap();
        }
        let mut origin = [POINT {
            x: actual.left,
            y: actual.top,
        }];
        unsafe {
            MapWindowPoints(None, Some(root.0), &mut origin);
        }
        assert_eq!(origin[0].x, 851);
        unsafe {
            SendMessageW(
                handle.state.hwnd.get(),
                WM_MOUSEMOVE,
                None,
                Some(point(-1500)),
            );
        }
        assert_eq!(
            handle
                .state
                .config
                .borrow()
                .as_ref()
                .unwrap()
                .layout
                .primary
                .width,
            260.
        );
        // Multiple native updates coalesce into one shell notification.
        assert_eq!(notifications.get(), 2);
        unsafe {
            SendMessageW(handle.state.hwnd.get(), WM_CANCELMODE, None, None);
        }
        assert!(!handle.dragging());
        assert_ne!(unsafe { GetCapture() }, handle.state.hwnd.get());
        let final_update = handle.take_update().unwrap();
        assert!(!final_update.resizing);
        assert_eq!(final_update.layout.primary.width, 260.);
        assert_eq!(final_update.committed_ratio, Some(260. / 1022.));
        unsafe {
            SendMessageW(hwnd, WM_MOUSEMOVE, None, Some(point(1)));
        }
        handle.configure(None).unwrap();
        assert_eq!(*hover_events.borrow(), [Guide::Hover(true), Guide::Hover(false), Guide::Click, Guide::Hover(true), Guide::Hover(false)]);
        assert!(!unsafe { IsWindowVisible(handle.state.hwnd.get()) }.as_bool());
    }
}
