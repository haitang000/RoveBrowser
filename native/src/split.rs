//! Native split dragging adapted from Zigzag's ResizeHandle, paneFrames and
//! PageCardView.layout (Copyright 2026 Zigao Wang; Apache-2.0), modified for Win32.
//! Mouse input changes both real WebView2 viewports before notifying the shell.
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
            Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
};
const SUBCLASS: usize = 0x53504c54;
pub const MINIMUM_PANE_WIDTH: f64 = 260.;

/// Keep the visible edges and the renderer dimensions in one shared cache.
/// The host's ordinary layout and the native input handler use the same values.
#[derive(Clone)]
pub struct Pane {
    pub hwnd: HWND,
    pub controller: ICoreWebView2Controller,
    pub clip: Rc<WindowClip>,
    pub bounds: Rc<Cell<Option<[i32; 4]>>>,
    pub viewport: Rc<Cell<Option<[i32; 2]>>>,
}
impl Pane {
    fn move_window(&self, rect: Rect, scale: f64) -> Result<()> {
        self.clip.clear(self.hwnd)?;
        let next = [
            (rect.x * scale).round() as i32,
            (rect.y * scale).round() as i32,
            (rect.width * scale).round() as i32,
            (rect.height * scale).round() as i32,
        ];
        if self.bounds.get() != Some(next) {
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
    fn reflow(&self) -> Result<()> {
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
    /// Zigzag: a 20–80% share, at least 260 points per pane, equal halves when
    /// the available width cannot fit those minima. Round once, share the seam.
    fn resized(self, x: f64) -> Self {
        let width = self.width();
        let floor = 0.2_f64.max(MINIMUM_PANE_WIDTH / width.max(1.));
        let ceiling = 0.8_f64.min(1. - floor);
        let ratio = if floor < ceiling {
            ((x - self.primary.x) / width).clamp(floor, ceiling)
        } else {
            0.5
        };
        let left_width = (width * ratio).floor();
        Self {
            primary: Rect {
                width: left_width,
                ..self.primary
            },
            secondary: Rect {
                x: self.primary.x + left_width + self.gap(),
                width: width - left_width,
                ..self.secondary
            },
        }
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
    pub error: Option<String>,
}
struct State {
    root: HWND,
    popup: HWND,
    hwnd: Cell<HWND>,
    config: RefCell<Option<Config>>,
    pending: RefCell<Option<Update>>,
    notify: Box<dyn Fn()>,
    dragging: Cell<bool>,
    busy: Cell<bool>,
    grab_offset: Cell<f64>,
}
pub struct SplitHandle {
    state: Box<State>,
}
impl SplitHandle {
    pub fn new(root: HWND, popup: HWND, notify: impl Fn() + 'static) -> Result<Self> {
        let state = Box::new(State {
            root,
            popup,
            hwnd: Cell::new(HWND::default()),
            config: RefCell::new(None),
            pending: RefCell::new(None),
            notify: Box::new(notify),
            dragging: Cell::new(false),
            busy: Cell::new(false),
            grab_offset: Cell::new(0.),
        });
        // An invisible native input strip, like Zigzag's NSView ResizeHandle.
        // It draws no background and never hides or snapshots either page.
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
        error,
    });
    if previous.is_none() {
        (s.notify)();
    }
}
fn cursor_x(s: &State, lparam: LPARAM, scale: f64) -> f64 {
    let mut point = [POINT {
        x: lparam.0 as u16 as i16 as i32,
        y: (lparam.0 >> 16) as u16 as i16 as i32,
    }];
    unsafe {
        MapWindowPoints(Some(s.hwnd.get()), Some(s.root), &mut point);
    }
    point[0].x as f64 / scale
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
        let next = config.layout.resized(x - s.grab_offset.get());
        if next.primary.width != config.layout.primary.width {
            config.layout = next;
            *s.config.borrow_mut() = Some(config.clone());
            let error = apply(s, &config).err().map(|e| e.to_string());
            report(s, &config, s.dragging.get(), error);
        }
    }
    s.busy.set(false);
}
fn finish(s: &State) {
    if !s.dragging.replace(false) {
        return;
    }
    unsafe {
        if GetCapture() == s.hwnd.get() {
            let _ = ReleaseCapture();
        }
    }
    let config = s.config.borrow().clone();
    if let Some(config) = config {
        let restore = || -> Result<()> {
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
            let config = s.config.borrow().clone();
            if let Some(config) = config {
                let x = cursor_x(s, lparam, config.scale);
                s.grab_offset
                    .set(x - config.layout.primary.x - config.layout.primary.width);
                s.dragging.set(true);
                SetCapture(hwnd);
                let error = apply(s, &config).err().map(|e| e.to_string());
                report(s, &config, s.dragging.get(), error);
            }
            return LRESULT(0);
        }
        WM_MOUSEMOVE if s.dragging.get() => {
            let scale = s.config.borrow().as_ref().map(|c| c.scale);
            if let Some(scale) = scale {
                move_to(s, cursor_x(s, lparam, scale));
            }
            return LRESULT(0);
        }
        WM_LBUTTONUP | WM_CAPTURECHANGED | WM_CANCELMODE => {
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
            let next = layout.resized(x);
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
        assert_eq!(narrow.resized(-500.).ratio(), 0.5);
        assert_eq!(narrow.resized(3000.).ratio(), 0.5);
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
        let handle = SplitHandle::new(root.0, popup.0, move || count.set(count.get() + 1)).unwrap();
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
        unsafe {
            SendMessageW(
                handle.state.hwnd.get(),
                WM_LBUTTONDOWN,
                None,
                Some(point(1)),
            );
        }
        assert!(handle.dragging());
        assert_eq!(unsafe { GetCapture() }, handle.state.hwnd.get());
        handle.take_update();
        unsafe {
            SendMessageW(
                handle.state.hwnd.get(),
                WM_MOUSEMOVE,
                None,
                Some(point(100)),
            );
        }
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
        handle.configure(None).unwrap();
        assert!(!unsafe { IsWindowVisible(handle.state.hwnd.get()) }.as_bool());
    }
}
