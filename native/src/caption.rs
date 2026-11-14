//! Native resize borders and caption buttons, including the Windows 11 Snap flyout.
//! WebView2 owns descendant HWNDs on other threads. Instead of subclassing those
//! windows, cover the borders/buttons with an almost transparent input surface.
//! The UI paints the entire background. The root receives WM_NCHITTEST/HTMAXBUTTON and the
//! system supplies its own Snap menu. The close button is the exception: its hit stays on
//! this surface. Falling through to the WebView underneath swallows the button release, so
//! the window never sees WM_CLOSE. No synthetic layout flyout.
use crate::model::Rect;
use std::cell::{Cell, RefCell};
use windows::{
    core::{w, Error, Result},
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
        Graphics::Gdi::*,
        UI::{
            Controls::WM_MOUSELEAVE,
            Input::KeyboardAndMouse::{
                ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TME_NONCLIENT,
                TRACKMOUSEEVENT,
            },
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
};
const SUBCLASS: usize = 0x524f5645;
const SURFACE_SUBCLASS: usize = SUBCLASS + 1;
const RESIZE_BORDER: f64 = 6.;
const RESIZE_CORNER: f64 = 12.;
/// Logical pixels the restored frame reserves for resizing. Maximized windows
/// have no strip, so the toolbar can sit flush with the top.
pub(crate) fn chrome_inset(maximized: bool) -> f64 {
    if maximized { 0. } else { RESIZE_BORDER }
}
// UxTheme's private frame-paint messages are also suppressed by Chromium's
// custom frame (ui/views/win/hwnd_message_handler.cc). USER32 can send these
// while tracking caption hover even when the window has no standard frame.
const WM_NCUAHDRAWCAPTION: u32 = 0x00ae;
const WM_NCUAHDRAWFRAME: u32 = 0x00af;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Minimize,
    Maximize,
    Close,
}
impl Button {
    const ALL: [Self; 3] = [Self::Minimize, Self::Maximize, Self::Close];
    fn rect(self, maximize: RECT) -> RECT {
        // The DOM reports the middle of three equally sized, adjacent buttons.
        let offset = match self {
            Self::Minimize => maximize.left - maximize.right,
            Self::Maximize => 0,
            Self::Close => maximize.right - maximize.left,
        };
        RECT {
            left: maximize.left + offset,
            right: maximize.right + offset,
            ..maximize
        }
    }
    fn hit(self) -> u32 {
        match self {
            Self::Minimize => HTMINBUTTON,
            Self::Maximize => HTMAXBUTTON,
            Self::Close => HTCLOSE,
        }
    }
    fn command(self, maximized: bool) -> u32 {
        match self {
            Self::Minimize => SC_MINIMIZE,
            Self::Maximize if maximized => SC_RESTORE,
            Self::Maximize => SC_MAXIMIZE,
            Self::Close => SC_CLOSE,
        }
    }
}
fn button_at(maximize: RECT, point: POINT) -> Option<Button> {
    Button::ALL
        .into_iter()
        .find(|button| contains(button.rect(maximize), point))
}
fn button_bounds(maximize: RECT) -> RECT {
    RECT {
        left: Button::Minimize.rect(maximize).left,
        right: Button::Close.rect(maximize).right,
        ..maximize
    }
}
#[derive(Clone, Copy)]
struct Anchor {
    right: f64,
    top: f64,
    width: f64,
    height: f64,
}
#[derive(Clone, Copy)]
struct Palette {
    input: [u8; 4],
    hover: [u8; 4],
    foreground: [u8; 4],
}
struct State {
    frame: crate::frame::WindowFrame,
    surface: Cell<HWND>,
    rect: Cell<RECT>,
    anchor: Cell<Option<Anchor>>,
    palette: Cell<Palette>,
    hot: Cell<Option<Button>>,
    pressed: Cell<Option<Button>>,
    scale: Cell<f64>,
    cut: Cell<Option<[i32; 7]>>,
    layer: RefCell<Option<CaptionBitmap>>,
    #[cfg(test)]
    paint_count: Cell<usize>,
    #[cfg(test)]
    close_count: Cell<usize>,
}
pub struct Caption {
    root: HWND,
    ui: HWND,
    state: Box<State>,
}
fn message_point(lparam: LPARAM) -> POINT {
    POINT {
        x: (lparam.0 as u16 as i16) as i32,
        y: ((lparam.0 >> 16) as u16 as i16) as i32,
    }
}
fn contains(r: RECT, p: POINT) -> bool {
    p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}
fn resize_hit(bounds: RECT, p: POINT, scale: f64) -> Option<u32> {
    if !contains(bounds, p) {
        return None;
    }
    let border = (RESIZE_BORDER * scale).round() as i32;
    let corner = (RESIZE_CORNER * scale).round() as i32;
    let left = p.x < bounds.left + border;
    let right = p.x >= bounds.right - border;
    let top = p.y < bounds.top + border;
    let bottom = p.y >= bounds.bottom - border;
    if !left && !right && !top && !bottom {
        return None;
    }
    Some(
        match (
            p.x < bounds.left + corner,
            p.x >= bounds.right - corner,
            p.y < bounds.top + corner,
            p.y >= bounds.bottom - corner,
        ) {
            (true, _, true, _) => HTTOPLEFT,
            (_, true, true, _) => HTTOPRIGHT,
            (true, _, _, true) => HTBOTTOMLEFT,
            (_, true, _, true) => HTBOTTOMRIGHT,
            _ if left => HTLEFT,
            _ if right => HTRIGHT,
            _ if top => HTTOP,
            _ => HTBOTTOM,
        },
    )
}

// Regions remain ours until SetWindowRgn successfully transfers ownership.
struct Region(HRGN);
impl Region {
    fn new(bounds: RECT) -> Result<Self> {
        let region = unsafe { CreateRectRgn(bounds.left, bounds.top, bounds.right, bounds.bottom) };
        if region.0.is_null() {
            Err(Error::from_thread())
        } else {
            Ok(Self(region))
        }
    }
    fn install(mut self, hwnd: HWND) -> Result<()> {
        if unsafe { SetWindowRgn(hwnd, Some(self.0), true) } == 0 {
            return Err(Error::from_thread());
        }
        self.0 = HRGN::default();
        Ok(())
    }
}
impl Drop for Region {
    fn drop(&mut self) {
        if !self.0 .0.is_null() {
            unsafe {
                let _ = DeleteObject(self.0.into());
            }
        }
    }
}
fn color(c: [u8; 4]) -> COLORREF {
    COLORREF(c[0] as u32 | ((c[1] as u32) << 8) | ((c[2] as u32) << 16))
}
impl Caption {
    pub fn new(root: HWND, ui: HWND) -> Result<Self> {
        let state = Box::new(State {
            frame: crate::frame::WindowFrame::new(root)?,
            surface: Cell::new(HWND::default()),
            rect: Cell::new(RECT::default()),
            anchor: Cell::new(None),
            palette: Cell::new(Palette {
                input: input_marker([255, 255, 255, 98], [0, 0, 0, 158]),
                hover: [255, 255, 255, 98],
                foreground: [0, 0, 0, 158],
            }),
            hot: Cell::new(None),
            pressed: Cell::new(None),
            scale: Cell::new(1.),
            cut: Cell::new(None),
            layer: RefCell::new(None),
            #[cfg(test)]
            paint_count: Cell::new(0),
            #[cfg(test)]
            close_count: Cell::new(0),
        });
        // This same-thread owned surface covers only the borders/buttons. Its
        // HTTRANSPARENT result lets the root receive real non-client input before
        // WebView2 can capture the pointer; no asynchronous drag IPC is needed.
        // A top-level layered surface remains visible over a non-redirected root.
        let surface = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!(""),
                WS_POPUP,
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
        state.surface.set(surface);
        // SAFETY: Box address is stable; it outlives both subclasses and HWNDs.
        if !unsafe {
            SetWindowSubclass(
                root,
                Some(procedure),
                SUBCLASS,
                (&*state as *const State) as usize,
            )
        }
        .as_bool()
            || !unsafe {
                SetWindowSubclass(
                    surface,
                    Some(procedure),
                    SURFACE_SUBCLASS,
                    (&*state as *const State) as usize,
                )
            }
            .as_bool()
        {
            unsafe {
                let _ = RemoveWindowSubclass(root, Some(procedure), SUBCLASS);
                let _ = DestroyWindow(surface);
            }
            return Err(Error::from_thread());
        }
        let caption = Self { root, ui, state };
        unsafe {
            // winit keeps WS_CAPTION even with decorations(false), extending the
            // client area over it via WM_NCCALCSIZE. USER32 can still draw that
            // hidden caption directly on activation/title/style changes, without
            // sending WM_NCPAINT. Remove the source of those legacy buttons while
            // retaining WS_THICKFRAME, WS_SYSMENU and the minimize/maximize bits.
            let style = GetWindowLongPtrW(root, GWL_STYLE);
            if style & WS_CAPTION.0 as isize != 0 {
                if SetWindowLongPtrW(root, GWL_STYLE, style & !(WS_CAPTION.0 as isize)) == 0 {
                    return Err(Error::from_thread());
                }
                SetWindowPos(
                    root,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                )?;
            }
        }
        Ok(caption)
    }
    pub fn update(
        &self,
        rect: Rect,
        viewport_width: f64,
        _background: [u8; 4],
        hover: [u8; 4],
        foreground: [u8; 4],
        scale: f64,
    ) -> Result<()> {
        if !rect.valid()
            || !viewport_width.is_finite()
            || rect.width < 20.
            || rect.width > 100.
            || rect.height < 16.
            || rect.height > 64.
            || rect.y > 16.
            || rect.x + rect.width > viewport_width + 1.
        {
            return Ok(());
        }
        self.state.anchor.set(Some(Anchor {
            right: (viewport_width - rect.x - rect.width).max(0.),
            top: rect.y,
            width: rect.width,
            height: rect.height,
        }));
        self.state.palette.set(Palette {
            // A unique RGB marker distinguishes untouched input pixels from
            // glyphs/hover ink after GDI (which does not preserve alpha).
            input: input_marker(hover, foreground),
            hover,
            foreground,
        });
        self.refresh(scale)
    }
    pub fn refresh(&self, scale: f64) -> Result<()> {
        self.state.frame.refresh(self.root, scale)?;
        let mut client = RECT::default();
        unsafe {
            GetClientRect(self.ui, &mut client)?;
        }
        let rect = self
            .state
            .anchor
            .get()
            .map(|a| {
                let width = (a.width * scale).round() as i32;
                let right = client.right - (a.right * scale).round() as i32;
                RECT {
                    left: right - width,
                    right,
                    top: (a.top * scale).round() as i32,
                    bottom: ((a.top + a.height) * scale).round() as i32,
                }
            })
            .unwrap_or_default();
        let border = if unsafe { IsZoomed(self.root) }.as_bool() {
            0
        } else {
            (RESIZE_BORDER * scale).round() as i32
        };
        self.state.rect.set(rect);
        self.state.scale.set(scale);
        let key = [
            client.right,
            client.bottom,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            border,
        ];
        if self.state.cut.get() != Some(key) {
            let interior = Region::new(RECT {
                left: border,
                top: border,
                right: (client.right - border).max(border),
                bottom: (client.bottom - border).max(border),
            })?;
            // Include all three controls in the input/ink surface, including
            // the portions that overlap the resize strip.
            let button = Region::new(button_bounds(rect))?;
            let surface = Region::new(client)?;
            unsafe {
                if CombineRgn(Some(interior.0), Some(interior.0), Some(button.0), RGN_DIFF).0 == 0
                    || CombineRgn(Some(surface.0), Some(surface.0), Some(interior.0), RGN_DIFF).0
                        == 0
                {
                    return Err(Error::from_thread());
                }
                // This owned overlay is not clipped by its owner's DWM frame.
                // Limit hover fills to the same outline, also on the fallback.
                let radius = if border == 0 { 0 } else { (self.radius() * scale).round() as i32 };
                let outline = Region(CreateRoundRectRgn(0, 0, client.right + 1, client.bottom + 1, radius * 2, radius * 2));
                if outline.0.0.is_null() || CombineRgn(Some(surface.0), Some(surface.0), Some(outline.0), RGN_AND).0 == 0 {
                    return Err(Error::from_thread());
                }
            }
            // Keep the WebView full size so gradients/grain continue all the
            // way to the frame and underneath the transparent caption ink.
            surface.install(self.state.surface.get())?;
            self.state.cut.set(Some(key));
        }
        unsafe {
            let mut window_bounds = RECT::default();
            GetWindowRect(self.root, &mut window_bounds)?;
            SetWindowPos(
                self.state.surface.get(),
                None,
                window_bounds.left,
                window_bounds.top,
                client.right,
                client.bottom,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )?;
            paint_layered(
                self.state.surface.get(),
                client,
                &self.state,
                IsZoomed(self.root).as_bool(),
            )?;
            position_surface(self.root, &self.state);
            let _ = RedrawWindow(
                Some(self.state.surface.get()),
                None,
                None,
                RDW_INVALIDATE | RDW_UPDATENOW,
            );
        }
        Ok(())
    }
    pub fn radius(&self) -> f64 {
        self.state.frame.radius()
    }
}
impl Drop for Caption {
    fn drop(&mut self) {
        unsafe {
            let _ = RemoveWindowSubclass(self.root, Some(procedure), SUBCLASS);
            let _ =
                RemoveWindowSubclass(self.state.surface.get(), Some(procedure), SURFACE_SUBCLASS);
            let _ = DestroyWindow(self.state.surface.get());
            let _ = SetWindowRgn(self.ui, None, true);
        }
    }
}
fn resize_owns_close(s: &State, point: POINT) -> bool {
    let Ok(root) = (unsafe { GetParent(s.surface.get()) }) else {
        return false;
    };
    if unsafe { IsZoomed(root).as_bool() } {
        return false;
    }
    let mut bounds = RECT::default();
    unsafe { GetClientRect(s.surface.get(), &mut bounds) }.is_ok()
        && resize_hit(bounds, point, s.scale.get()).is_some()
}
unsafe fn command_root(s: &State, hwnd: HWND) -> HWND {
    if hwnd == s.surface.get() {
        GetParent(hwnd).unwrap_or(hwnd)
    } else {
        hwnd
    }
}
unsafe fn issue_button(root: HWND, button: Button) {
    if button == Button::Close {
        // winit turns WM_CLOSE into CloseRequested, the same path Alt+F4 uses.
        let _ = PostMessageW(Some(root), WM_CLOSE, WPARAM(0), LPARAM(0));
        return;
    }
    let command = button.command(IsZoomed(root).as_bool());
    let _ = PostMessageW(Some(root), WM_SYSCOMMAND, WPARAM(command as usize), LPARAM(0));
}
unsafe extern "system" fn procedure(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    subclass: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: this pointer comes from Caption::new and is removed before Box drop.
    let s = &*(data as *const State);
    match msg {
        WM_GETMINMAXINFO if hwnd != s.surface.get() && lparam.0 != 0 => {
            // Keep winit's minimum/maximum tracking sizes, then constrain the
            // outer HWND as well as its client area. Without WS_CAPTION,
            // USER32 otherwise maximizes over the taskbar; winit only adjusts
            // the client rectangle in WM_NCCALCSIZE, leaving a black frame there.
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            let mut monitor = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            )
            .as_bool()
            {
                let limits = &mut *(lparam.0 as *mut MINMAXINFO);
                limits.ptMaxPosition = POINT {
                    x: monitor.rcWork.left - monitor.rcMonitor.left,
                    y: monitor.rcWork.top - monitor.rcMonitor.top,
                };
                limits.ptMaxSize = POINT {
                    x: monitor.rcWork.right - monitor.rcWork.left,
                    y: monitor.rcWork.bottom - monitor.rcWork.top,
                };
            }
            return result;
        }
        WM_STYLECHANGING if hwnd != s.surface.get() && wparam.0 as i32 == GWL_STYLE.0 => {
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            if lparam.0 != 0 {
                // Showing, maximizing and changing resize/button availability
                // rebuild the style from winit's cached flags, including
                // WS_CAPTION. Filter every such update before Windows applies it.
                let styles = &mut *(lparam.0 as *mut STYLESTRUCT);
                styles.styleNew &= !WS_CAPTION.0;
            }
            return result;
        }
        WM_NCHITTEST => {
            if hwnd == s.surface.get() {
                let mut point = message_point(lparam);
                // WebView2 covers the caption and takes the mouse release when this
                // hit falls through. Keep the close click on the surface. Resize
                // edges still fall through so the root can return HTTOP/HTRIGHT.
                if ScreenToClient(hwnd, &mut point).as_bool()
                    && !resize_owns_close(s, point)
                    && button_at(s.rect.get(), point) == Some(Button::Close)
                {
                    return LRESULT(HTCLIENT as isize);
                }
                return LRESULT(HTTRANSPARENT as isize);
            }
            // Sign extension is required for monitors left/above the primary screen.
            let mut p = POINT {
                x: (lparam.0 as u16 as i16) as i32,
                y: ((lparam.0 >> 16) as u16 as i16) as i32,
            };
            if ScreenToClient(hwnd, &mut p).as_bool() {
                if !IsZoomed(hwnd).as_bool() {
                    let mut bounds = RECT::default();
                    if GetClientRect(hwnd, &mut bounds).is_ok() {
                        if let Some(hit) = resize_hit(bounds, p, s.scale.get()) {
                            return LRESULT(hit as isize);
                        }
                    }
                }
                if let Some(button) = button_at(s.rect.get(), p) {
                    return LRESULT(button.hit() as isize);
                }
            }
        }
        WM_NCMOUSEMOVE if hwnd != s.surface.get() => {
            let hot = Button::ALL
                .into_iter()
                .find(|button| button.hit() as usize == wparam.0);
            set_hot(s, hot);
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE | TME_NONCLIENT,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut track);
            // Keep the system's Snap tracking. The root has no WS_CAPTION, so
            // default processing cannot draw system buttons over our surface.
            return DefSubclassProc(hwnd, msg, wparam, lparam);
        }
        WM_NCMOUSELEAVE if hwnd != s.surface.get() => {
            set_hot(s, None);
            return DefSubclassProc(hwnd, msg, wparam, lparam);
        }
        WM_NCPAINT | WM_NCUAHDRAWCAPTION | WM_NCUAHDRAWFRAME if hwnd != s.surface.get() => {
            return LRESULT(0);
        }
        WM_NCLBUTTONDOWN if hwnd != s.surface.get() => {
            if let Some(button) = Button::ALL
                .into_iter()
                .find(|button| button.hit() as usize == wparam.0)
            {
                s.pressed.set(Some(button));
                let _ = SetCapture(hwnd);
                return LRESULT(0);
            }
        }
        WM_MOUSEMOVE if hwnd == s.surface.get() => {
            let point = message_point(lparam);
            let hot = if s.pressed.get().is_some() {
                s.pressed
                    .get()
                    .filter(|button| contains(button.rect(s.rect.get()), point))
            } else if !resize_owns_close(s, point) && button_at(s.rect.get(), point) == Some(Button::Close) {
                Some(Button::Close)
            } else {
                None
            };
            set_hot(s, hot);
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut track);
            return LRESULT(0);
        }
        WM_MOUSELEAVE if hwnd == s.surface.get() && s.pressed.get().is_none() => {
            set_hot(s, None);
            return LRESULT(0);
        }
        WM_LBUTTONDOWN if hwnd == s.surface.get() => {
            let point = message_point(lparam);
            if button_at(s.rect.get(), point) == Some(Button::Close) {
                s.pressed.set(Some(Button::Close));
                set_hot(s, Some(Button::Close));
                let _ = SetCapture(hwnd);
                return LRESULT(0);
            }
        }
        WM_MOUSEMOVE if hwnd != s.surface.get() && s.pressed.get().is_some() => {
            let point = message_point(lparam);
            let hot = s
                .pressed
                .get()
                .filter(|button| contains(button.rect(s.rect.get()), point));
            set_hot(s, hot);
            return LRESULT(0);
        }
        WM_LBUTTONUP | WM_NCLBUTTONUP
            if s.pressed.get().is_some() && (hwnd != s.surface.get() || msg == WM_LBUTTONUP) =>
        {
            let button = s.pressed.take().unwrap();
            let mut point = message_point(lparam);
            if msg == WM_NCLBUTTONUP && !ScreenToClient(hwnd, &mut point).as_bool() {
                point = POINT { x: i32::MIN, y: i32::MIN };
            }
            let inside = contains(button.rect(s.rect.get()), point);
            set_hot(s, inside.then_some(button));
            let _ = ReleaseCapture();
            if inside {
                issue_button(command_root(s, hwnd), button);
            }
            return LRESULT(0);
        }
        WM_CAPTURECHANGED => {
            s.pressed.set(None);
        }
        #[cfg(test)]
        WM_CLOSE if hwnd != s.surface.get() => {
            s.close_count.set(s.close_count.get() + 1);
            return LRESULT(0);
        }
        WM_NCACTIVATE if hwnd != s.surface.get() => {
            // Keep winit's activation/focus handling; -1 asks DefWindowProc to
            // update activation without repainting the standard non-client frame.
            let result = DefSubclassProc(hwnd, msg, wparam, LPARAM(-1));
            let _ = s.frame.refresh(hwnd, s.scale.get());
            return result;
        }
        WM_SIZE if hwnd != s.surface.get() => {
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            let _ = s.frame.refresh(hwnd, s.scale.get());
            let _ = RedrawWindow(
                Some(s.surface.get()),
                None,
                None,
                RDW_INVALIDATE | RDW_UPDATENOW,
            );
            return result;
        }
        WM_WINDOWPOSCHANGED | WM_SHOWWINDOW if hwnd != s.surface.get() => {
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            let _ = s.frame.refresh(hwnd, s.scale.get());
            position_surface(hwnd, s);
            return result;
        }
        WM_PAINT => {
            if hwnd != s.surface.get() {
                return DefSubclassProc(hwnd, msg, wparam, lparam);
            }
            let mut paint = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut paint);
            let mut r = RECT::default();
            let _ = GetClientRect(hwnd, &mut r);
            // A non-redirected root has no GDI backing store. Present this
            // owned surface explicitly, preserving the shell's alpha and hits.
            if let Err(error) = paint_layered(
                hwnd,
                r,
                s,
                GetParent(hwnd).is_ok_and(|root| IsZoomed(root).as_bool()),
            ) {
                eprintln!("Rove caption: {error}");
            }
            let _ = EndPaint(hwnd, &paint);
            return LRESULT(0);
        }
        WM_ERASEBKGND if hwnd == s.surface.get() => {
            // WM_PAINT supplies every pixel, including the resize strips.
            return LRESULT(1);
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(procedure), subclass);
        }
        _ => {}
    }
    // Other non-client messages retain the window's normal handling.
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
unsafe fn set_hot(s: &State, hot: Option<Button>) {
    let previous = s.hot.replace(hot);
    if previous != hot {
        let mut bounds = RECT::default();
        if GetClientRect(s.surface.get(), &mut bounds).is_ok() {
            let maximized = GetParent(s.surface.get()).is_ok_and(|root| IsZoomed(root).as_bool());
            if let Err(error) = paint_layered(s.surface.get(), bounds, s, maximized) {
                eprintln!("Rove caption: {error}");
            }
        }
    }
}
// GDI writes RGB without usable alpha. Keep a reusable top-down DIB for the
// native borders/buttons, then supply premultiplied pixels to the compositor.
struct CaptionBitmap {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u32,
    size: SIZE,
}
impl CaptionBitmap {
    unsafe fn new(size: SIZE) -> Result<Self> {
        let dc = CreateCompatibleDC(None);
        if dc.0.is_null() {
            return Err(Error::from_thread());
        }
        let mut result = Self {
            dc,
            bitmap: HBITMAP::default(),
            previous: HGDIOBJ::default(),
            bits: std::ptr::null_mut(),
            size,
        };
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size.cx,
                biHeight: -size.cy,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        result.bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
        result.previous = SelectObject(dc, result.bitmap.into());
        result.bits = bits.cast();
        Ok(result)
    }
}
impl Drop for CaptionBitmap {
    fn drop(&mut self) {
        unsafe {
            if !self.bitmap.0.is_null() {
                let _ = SelectObject(self.dc, self.previous);
                let _ = DeleteObject(self.bitmap.into());
            }
            let _ = DeleteDC(self.dc);
        }
    }
}
fn input_marker(hover: [u8; 4], foreground: [u8; 4]) -> [u8; 4] {
    // Neither real color can equal all three candidates.
    (1..=3).map(|r| [r, 0, 0, 1])
        .find(|c| c[..3] != hover[..3] && c[..3] != foreground[..3]).unwrap()
}
fn glyph_marker(palette: Palette) -> [u8; 4] {
    (4..=6).map(|r| [r, 0, 0, 255])
        .find(|c| c[..3] != palette.hover[..3]).unwrap()
}
fn bitmap_rgb(c: [u8; 4]) -> u32 {
    (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32
}
fn premultiply(pixel: u32, palette: Palette) -> u32 {
    let mut rgb = pixel & 0x00ff_ffff;
    if rgb == bitmap_rgb(palette.input) {
        // Alpha zero makes layered HWNDs pass input through to WebView2. One
        // alpha unit keeps native hits without painting a solid frame over it.
        return 0x0100_0000;
    }
    // Dark mode uses white for both hover and glyphs at different opacities.
    // A separate glyph marker preserves those independent alpha values.
    let alpha = if rgb == bitmap_rgb(glyph_marker(palette)) {
        rgb = bitmap_rgb(palette.foreground);
        palette.foreground[3] as u32
    } else if rgb == bitmap_rgb(palette.hover) {
        palette.hover[3] as u32
    } else { 255 };
    let channel = |shift: u32| (((rgb >> shift) & 255u32) * alpha + 127) / 255;
    alpha << 24 | channel(16) << 16 | channel(8) << 8 | channel(0)
}
unsafe fn paint_layered(hwnd: HWND, bounds: RECT, s: &State, maximized: bool) -> Result<()> {
    let size = SIZE {
        cx: bounds.right,
        cy: bounds.bottom,
    };
    if size.cx <= 0 || size.cy <= 0 {
        return Ok(());
    }
    let mut layer = s.layer.borrow_mut();
    if layer
        .as_ref()
        .is_none_or(|b| b.size.cx != size.cx || b.size.cy != size.cy)
    {
        *layer = Some(CaptionBitmap::new(size)?);
    }
    let bitmap = layer.as_ref().unwrap();
    paint_surface(bitmap.dc, bounds, s, maximized);
    let _ = GdiFlush();
    let pixels = std::slice::from_raw_parts_mut(bitmap.bits, size.cx as usize * size.cy as usize);
    let border = s.cut.get().map_or(0, |key| key[6]);
    let buttons = button_bounds(s.rect.get());
    let opaque_close =
        (s.hot.get() == Some(Button::Close)).then(|| Button::Close.rect(s.rect.get()));
    let palette = s.palette.get();
    // Only the perimeter and caption buttons belong to this HWND's region.
    // Leave interior pixels at alpha zero; hover work scales with the perimeter.
    for y in 0..size.cy {
        let mut ranges = if y < border || y >= size.cy - border {
            [0..size.cx, 0..0, 0..0]
        } else {
            [0..border, size.cx - border..size.cx, 0..0]
        };
        if y >= border && y < size.cy - border && y >= buttons.top && y < buttons.bottom {
            // Exclude the two edge strips already covered above.
            ranges[2] = buttons.left.max(border)..buttons.right.min(size.cx - border);
        }
        for x in ranges.into_iter().flatten() {
            let pixel = &mut pixels[(y * size.cx + x) as usize];
            *pixel = if opaque_close.is_some_and(|r| contains(r, POINT { x, y })) {
                0xff00_0000 | (*pixel & 0x00ff_ffff)
            } else {
                premultiply(*pixel, palette)
            };
        }
    }
    let mut root_bounds = RECT::default();
    GetWindowRect(GetParent(hwnd)?, &mut root_bounds)?;
    let position = POINT {
        x: root_bounds.left,
        y: root_bounds.top,
    };
    UpdateLayeredWindow(
        hwnd,
        None,
        Some(&position),
        Some(&size),
        Some(bitmap.dc),
        Some(&POINT::default()),
        COLORREF(0),
        Some(&BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        }),
        ULW_ALPHA,
    )?;
    #[cfg(test)]
    s.paint_count.set(s.paint_count.get() + 1);
    Ok(())
}
unsafe fn position_surface(root: HWND, s: &State) {
    if !IsWindowVisible(root).as_bool() || IsIconic(root).as_bool() {
        let _ = ShowWindow(s.surface.get(), SW_HIDE);
    } else {
        let mut bounds = RECT::default();
        if GetWindowRect(root, &mut bounds).is_ok() {
            let _ = SetWindowPos(
                s.surface.get(),
                None,
                bounds.left,
                bounds.top,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
        }
    }
}
unsafe fn paint_surface(dc: HDC, bounds: RECT, s: &State, maximized: bool) {
    let brush = CreateSolidBrush(color(s.palette.get().input));
    let _ = FillRect(dc, &bounds, brush);
    let _ = DeleteObject(brush.into());
    for button in Button::ALL {
        paint_button(dc, button, s, maximized);
    }
}
unsafe fn paint_button(dc: HDC, button: Button, s: &State, maximized: bool) {
    let r = button.rect(s.rect.get());
    let p = s.palette.get();
    let hot = s.hot.get() == Some(button);
    let background = if hot && button == Button::Close {
        [196, 43, 28, 255]
    } else if hot {
        p.hover
    } else {
        p.input
    };
    let brush = CreateSolidBrush(color(background));
    let _ = FillRect(dc, &r, brush);
    let _ = DeleteObject(brush.into());
    let pen = CreatePen(
        PS_SOLID,
        (s.scale.get().round() as i32).max(1),
        color(if hot && button == Button::Close {
            [255; 4]
        } else {
            glyph_marker(p)
        }),
    );
    let old_pen = SelectObject(dc, pen.into());
    let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
    let side = ((if button == Button::Maximize { 9. } else { 7.5 }) * s.scale.get()).round() as i32;
    let x = (r.left + r.right - side) / 2;
    let y = (r.top + r.bottom - side) / 2;
    if button == Button::Minimize {
        let y = (r.top + r.bottom) / 2;
        let _ = MoveToEx(dc, x, y, None);
        let _ = LineTo(dc, x + side, y);
    } else if button == Button::Close {
        let _ = MoveToEx(dc, x, y, None);
        let _ = LineTo(dc, x + side, y + side);
        let _ = MoveToEx(dc, x + side, y, None);
        let _ = LineTo(dc, x, y + side);
    } else if maximized {
        let offset = (2. * s.scale.get()).round() as i32;
        let _ = Rectangle(dc, x + offset, y, x + side + offset, y + side);
        let fill = CreateSolidBrush(color(background));
        let inner = RECT {
            left: x,
            top: y + offset,
            right: x + side,
            bottom: y + side + offset,
        };
        let _ = FillRect(dc, &inner, fill);
        let _ = DeleteObject(fill.into());
        let _ = Rectangle(dc, x, y + offset, x + side, y + side + offset);
    } else {
        let _ = Rectangle(dc, x, y, x + side, y + side);
    }
    let _ = SelectObject(dc, old_pen);
    let _ = SelectObject(dc, old_brush);
    let _ = DeleteObject(pen.into());
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_ink_colors_preserve_distinct_hover_glyph_and_input_alpha() {
        let palette = Palette {
            input: input_marker([255, 255, 255, 13], [255, 255, 255, 168]),
            hover: [255, 255, 255, 13],
            foreground: [255, 255, 255, 168],
        };
        assert_eq!(premultiply(bitmap_rgb(palette.input), palette), 0x0100_0000);
        assert_eq!(premultiply(bitmap_rgb(palette.hover), palette), 0x0d0d_0d0d);
        assert_eq!(premultiply(bitmap_rgb(glyph_marker(palette)), palette), 0xa8a8_a8a8);
    }
    #[test]
    fn maximized_windows_drop_the_resize_strip() {
        assert_eq!(chrome_inset(false), RESIZE_BORDER);
        assert_eq!(chrome_inset(true), 0.);
    }
    #[test]
    #[allow(deprecated)]
    fn winit_state_changes_do_not_restore_the_system_caption() {
        use winit::{
            dpi::PhysicalPosition,
            event_loop::EventLoop,
            platform::windows::EventLoopBuilderExtWindows,
            raw_window_handle::{HasWindowHandle, RawWindowHandle},
            window::{Window, WindowButtons},
        };

        let event_loop = EventLoop::builder().with_any_thread(true).build().unwrap();
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_decorations(false)
                    .with_visible(false)
                    .with_position(PhysicalPosition::new(-10000, -10000)),
            )
            .unwrap();
        let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("expected a Windows HWND");
        };
        let root = HWND(handle.hwnd.get() as *mut _);
        unsafe {
            let ui = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("UI"),
                WS_CHILD,
                0,
                0,
                800,
                600,
                Some(root),
                None,
                None,
                None,
            )
            .unwrap();
            let _caption = Caption::new(root, ui).unwrap();
            let assert_custom_frame = || {
                let style = GetWindowLongPtrW(root, GWL_STYLE) as u32;
                assert_eq!(style & WS_CAPTION.0, 0, "system caption must stay disabled");
                assert_ne!(style & WS_SYSMENU.0, 0, "keep the system window menu");
            };
            assert_custom_frame();
            for _ in 0..3 {
                // These APIs rebuild GWL_STYLE from winit's cached window flags.
                window.set_visible(true);
                window.set_title("Updated caption");
                assert_custom_frame();
                window.set_resizable(false);
                assert_custom_frame();
                window.set_resizable(true);
                window.set_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MAXIMIZE);
                assert_custom_frame();
                window.set_enabled_buttons(WindowButtons::all());
                window.set_maximized(true);
                assert_custom_frame();
                let mut monitor = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                assert!(GetMonitorInfoW(
                    MonitorFromWindow(root, MONITOR_DEFAULTTONEAREST),
                    &mut monitor,
                )
                .as_bool());
                let mut outer = RECT::default();
                GetWindowRect(root, &mut outer).unwrap();
                assert_eq!(
                    outer, monitor.rcWork,
                    "maximized outer bounds must leave the taskbar uncovered"
                );
                window.set_maximized(false);
                assert_custom_frame();
                window.set_minimized(true);
                assert_custom_frame();
                window.set_minimized(false);
                window.set_visible(false);
                assert_custom_frame();
                let style = GetWindowLongPtrW(root, GWL_STYLE) as u32;
                for flag in [WS_THICKFRAME, WS_MINIMIZEBOX, WS_MAXIMIZEBOX] {
                    assert_ne!(style & flag.0, 0, "keep native resize/minimize/maximize");
                }
            }
        }
    }
    #[test]
    fn hit_region_excludes_adjacent_caption_controls() {
        let r = RECT {
            left: 1188,
            top: 0,
            right: 1234,
            bottom: 32,
        };
        assert!(contains(r, POINT { x: 1210, y: 12 }));
        assert!(!contains(r, POINT { x: 1234, y: 12 }));
        assert!(!contains(r, POINT { x: 1187, y: 12 }));
        assert_eq!(
            button_at(r, POINT { x: 1187, y: 12 }),
            Some(Button::Minimize)
        );
        assert_eq!(
            button_at(r, POINT { x: 1188, y: 12 }),
            Some(Button::Maximize)
        );
        assert_eq!(button_at(r, POINT { x: 1234, y: 12 }), Some(Button::Close));
        assert_eq!(button_at(r, POINT { x: 1280, y: 12 }), None);
        assert_eq!(button_at(r, POINT { x: 1210, y: 32 }), None);
    }
    #[test]
    fn surface_close_click_posts_wm_close_without_falling_through() {
        unsafe {
            let root = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Close click"),
                WS_POPUP,
                -1600,
                80,
                800,
                600,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let ui = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("UI"),
                WS_CHILD,
                0,
                0,
                800,
                600,
                Some(root),
                None,
                None,
                None,
            )
            .unwrap();
            let caption = Caption::new(root, ui).unwrap();
            caption
                .update(
                    Rect {
                        x: 708.,
                        y: 0.,
                        width: 46.,
                        height: 32.,
                        ..Rect::default()
                    },
                    800.,
                    [212, 228, 251, 255],
                    [228, 239, 253, 255],
                    [40, 40, 40, 255],
                    1.,
                )
                .unwrap();
            let close = Button::Close.rect(caption.state.rect.get());
            let center = POINT {
                x: (close.left + close.right) / 2,
                y: (close.top + close.bottom) / 2,
            };
            let mut screen = center;
            assert!(ClientToScreen(caption.state.surface.get(), &mut screen).as_bool());
            let coords = LPARAM((((screen.y as u16 as u32) << 16) | screen.x as u16 as u32) as isize);
            assert_eq!(
                SendMessageW(caption.state.surface.get(), WM_NCHITTEST, None, Some(coords)).0,
                HTCLIENT as isize,
                "close clicks must stay on the caption surface"
            );
            let maximize = Button::Maximize.rect(caption.state.rect.get());
            let mut max_screen = POINT {
                x: (maximize.left + maximize.right) / 2,
                y: (maximize.top + maximize.bottom) / 2,
            };
            assert!(ClientToScreen(caption.state.surface.get(), &mut max_screen).as_bool());
            let max_coords =
                LPARAM((((max_screen.y as u16 as u32) << 16) | max_screen.x as u16 as u32) as isize);
            assert_eq!(
                SendMessageW(caption.state.surface.get(), WM_NCHITTEST, None, Some(max_coords)).0,
                HTTRANSPARENT as isize,
                "maximize still falls through so Windows can show Snap layouts"
            );
            let client = LPARAM((((center.y as u16 as u32) << 16) | center.x as u16 as u32) as isize);
            SendMessageW(caption.state.surface.get(), WM_LBUTTONDOWN, None, Some(client));
            SendMessageW(caption.state.surface.get(), WM_LBUTTONUP, None, Some(LPARAM(0)));
            let mut message = MSG::default();
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            assert_eq!(caption.state.close_count.get(), 0, "releasing outside cancels close");
            SendMessageW(caption.state.surface.get(), WM_LBUTTONDOWN, None, Some(client));
            SendMessageW(caption.state.surface.get(), WM_LBUTTONUP, None, Some(client));
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            assert_eq!(caption.state.close_count.get(), 1);
            let _ = DestroyWindow(ui);
            let _ = DestroyWindow(root);
        }
    }
    #[test]
    fn real_hwnd_hit_testing_preserves_the_full_ui_background_at_each_dpi() {
        struct TestWindow(HWND);
        impl Drop for TestWindow {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        unsafe {
            let root = TestWindow(
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("STATIC"),
                    w!("Caption test"),
                    WS_POPUP,
                    -1500,
                    100,
                    800,
                    600,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap(),
            );
            let ui = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("UI"),
                WS_CHILD,
                0,
                0,
                800,
                600,
                Some(root.0),
                None,
                None,
                None,
            )
            .unwrap();
            let caption = Caption::new(root.0, ui).unwrap();
            caption.refresh(1.).unwrap();
            let initial_region = CreateRectRgn(0, 0, 0, 0);
            assert_eq!(GetWindowRgn(ui, initial_region).0, 0, "keep the gradient underneath the input surface");
            let _ = DeleteObject(initial_region.into());
            caption
                .update(
                    Rect {
                        x: 708.,
                        y: 0.,
                        width: 46.,
                        height: 32.,
                        ..Rect::default()
                    },
                    800.,
                    [212, 228, 251, 255],
                    [228, 239, 253, 255],
                    [40, 40, 40, 255],
                    1.,
                )
                .unwrap();
            for scale in [1., 1.25, 1.5, 2.] {
                caption.refresh(scale).unwrap();
                let r = caption.state.rect.get();
                assert_eq!(r.right - r.left, (46. * scale).round() as i32);
                let mut point = POINT {
                    x: (r.left + r.right) / 2,
                    y: (r.top + r.bottom) / 2,
                };
                let _ = ClientToScreen(root.0, &mut point);
                let coords =
                    LPARAM((((point.y as u16 as u32) << 16) | (point.x as u16 as u32)) as isize);
                assert_eq!(
                    SendMessageW(root.0, WM_NCHITTEST, None, Some(coords)).0,
                    HTMAXBUTTON as isize
                );
                let region = CreateRectRgn(0, 0, 0, 0);
                assert_eq!(GetWindowRgn(ui, region).0, 0);
                let _ = DeleteObject(region.into());
                // Real HWND routing must expose every edge/corner to USER32,
                // while all three caption controls share the native paint surface.
                let border = (RESIZE_BORDER * scale).round() as i32;
                let near = (3. * scale).round() as i32;
                let corner = (9. * scale).round() as i32;
                let cases = [
                    (near, 300, HTLEFT),
                    (800 - near, 300, HTRIGHT),
                    (400, near, HTTOP),
                    (400, 600 - near, HTBOTTOM),
                    (near, corner, HTTOPLEFT),
                    (800 - near, corner, HTTOPRIGHT),
                    (corner, 600 - near, HTBOTTOMLEFT),
                    (800 - corner, 600 - near, HTBOTTOMRIGHT),
                ];
                let ui_region = CreateRectRgn(0, 0, 0, 0);
                let surface_region = CreateRectRgn(0, 0, 0, 0);
                assert_eq!(GetWindowRgn(ui, ui_region).0, 0);
                assert_ne!(
                    GetWindowRgn(caption.state.surface.get(), surface_region).0,
                    0
                );
                for (x, y, hit) in cases {
                    let mut point = POINT { x, y };
                    let _ = ClientToScreen(root.0, &mut point);
                    let coords =
                        LPARAM((((point.y as u16 as u32) << 16) | point.x as u16 as u32) as isize);
                    assert_eq!(
                        SendMessageW(root.0, WM_NCHITTEST, None, Some(coords)).0,
                        hit as isize,
                        "scale={scale}, x={x}, y={y}"
                    );
                    assert!(PtInRegion(surface_region, x, y).as_bool());
                }
                assert!(!PtInRegion(surface_region, border, 300).as_bool());
                for button in Button::ALL {
                    let rect = button.rect(r);
                    let (x, y) = ((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2);
                    assert!(PtInRegion(surface_region, x, y).as_bool());
                    let mut point = POINT { x, y };
                    assert!(ClientToScreen(root.0, &mut point).as_bool());
                    let coords =
                        LPARAM((((point.y as u16 as u32) << 16) | point.x as u16 as u32) as isize);
                    assert_eq!(
                        SendMessageW(root.0, WM_NCHITTEST, None, Some(coords)).0,
                        button.hit() as isize
                    );
                }
                assert_hover_pixels(&caption, surface_region);
                for button in Button::ALL {
                    SendMessageW(
                        root.0,
                        WM_NCLBUTTONDOWN,
                        Some(WPARAM(button.hit() as usize)),
                        None,
                    );
                    let rect = button.rect(r);
                    let (x, y) = ((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2);
                    SendMessageW(
                        root.0,
                        WM_MOUSEMOVE,
                        None,
                        Some(LPARAM(((y << 16) | x) as isize)),
                    );
                    assert_eq!(caption.state.hot.get(), Some(button));
                    SendMessageW(root.0, WM_MOUSEMOVE, None, Some(LPARAM((100 << 16) | 400)));
                    assert_eq!(caption.state.hot.get(), None);
                    // Losing capture must cancel the pending system command.
                    SendMessageW(root.0, WM_CAPTURECHANGED, None, None);
                    assert_eq!(caption.state.pressed.get(), None);
                    let _ = ReleaseCapture();
                }
                let _ = DeleteObject(ui_region.into());
                let _ = DeleteObject(surface_region.into());
                assert_eq!(
                    SendMessageW(
                        caption.state.surface.get(),
                        WM_NCHITTEST,
                        None,
                        Some(coords)
                    )
                    .0,
                    HTTRANSPARENT as isize
                );
            }
            // Maximizing at the same dimensions must remove the resize inset;
            // restoring must install it again, even without a DOM report.
            let style = GetWindowLongPtrW(root.0, GWL_STYLE);
            let _ = SetWindowLongPtrW(root.0, GWL_STYLE, style | WS_MAXIMIZE.0 as isize);
            assert!(IsZoomed(root.0).as_bool());
            caption.refresh(2.).unwrap();
            let region = CreateRectRgn(0, 0, 0, 0);
            assert_eq!(GetWindowRgn(ui, region).0, 0);
            let _ = SetWindowLongPtrW(root.0, GWL_STYLE, style);
            caption.refresh(2.).unwrap();
            assert_eq!(GetWindowRgn(ui, region).0, 0);
            let _ = DeleteObject(region.into());
            assert_stable_hover(&caption);
            assert_translucent_pixels(&caption);
        }
    }

    unsafe fn assert_stable_hover(caption: &Caption) {
        let counter = &caption.state.paint_count;
        for button in Button::ALL {
            SendMessageW(
                caption.root,
                WM_NCMOUSEMOVE,
                Some(WPARAM(button.hit() as usize)),
                None,
            );
            let paints = counter.get();
            assert!(paints > 0);
            for _ in 0..100 {
                SendMessageW(
                    caption.root,
                    WM_NCMOUSEMOVE,
                    Some(WPARAM(button.hit() as usize)),
                    None,
                );
            }
            assert_eq!(
                counter.get(),
                paints,
                "stationary hover must not force repaint: {button:?}"
            );
            // Leaving repaints once to clear hover, then stays idle as well.
            SendMessageW(caption.root, WM_NCMOUSELEAVE, None, None);
            assert!(counter.get() > paints);
            let paints = counter.get();
            SendMessageW(caption.root, WM_NCMOUSELEAVE, None, None);
            assert_eq!(counter.get(), paints);
        }
    }

    unsafe fn assert_translucent_pixels(caption: &Caption) {
        let s = &caption.state;
        for (alpha, expected) in [(64, 0x4032190a), (128, 0x80643214), (255, 0xffc86428)] {
            s.palette.set(Palette {
                input: input_marker([200, 100, 40, alpha], [40, 40, 40, 255]),
                hover: [200, 100, 40, alpha],
                foreground: [40, 40, 40, 255],
            });
            caption.refresh(2.).unwrap();
            let read = |x: i32, y: i32| {
                let layer = s.layer.borrow();
                let bitmap = layer.as_ref().unwrap();
                *bitmap.bits.add((y * bitmap.size.cx + x) as usize)
            };
            let r = Button::Maximize.rect(s.rect.get());
            assert_eq!(read((r.left + r.right) / 2, 1), 0x0100_0000);
            assert_eq!(read(1, 100), 0x0100_0000);
            // Changing hover reuses the backing bitmap and preserves per-pixel
            // alpha, while the close affordance stays fully opaque red.
            let bits = s.layer.borrow().as_ref().unwrap().bits;
            set_hot(s, Some(Button::Maximize));
            assert_eq!(read((r.left + r.right) / 2, 1), expected);
            assert_eq!(s.layer.borrow().as_ref().unwrap().bits, bits);
            set_hot(s, Some(Button::Close));
            let close = Button::Close.rect(s.rect.get());
            assert_eq!(read(close.right - 1, 1), 0xffc42b1c);
            set_hot(s, None);
        }
        // A white glass tint must not thin the close button's white glyph.
        s.palette.set(Palette {
            input: input_marker([255, 255, 255, 128], [40, 40, 40, 255]),
            hover: [255, 255, 255, 128],
            foreground: [40, 40, 40, 255],
        });
        set_hot(s, Some(Button::Close));
        let close = Button::Close.rect(s.rect.get());
        let side = (7.5 * s.scale.get()).round() as i32;
        let x = (close.left + close.right - side) / 2;
        let y = (close.top + close.bottom - side) / 2;
        {
            let layer = s.layer.borrow();
            let bitmap = layer.as_ref().unwrap();
            assert_eq!(
                *bitmap.bits.add((y * bitmap.size.cx + x) as usize),
                0xffff_ffff
            );
        }
        set_hot(s, None);
        // The owned surface follows moves without relying on a DOM resize.
        SetWindowPos(
            caption.root,
            None,
            -1200,
            120,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        )
        .unwrap();
        let _ = ShowWindow(caption.root, SW_SHOWNOACTIVATE);
        let mut root = RECT::default();
        let mut surface = RECT::default();
        GetWindowRect(caption.root, &mut root).unwrap();
        GetWindowRect(s.surface.get(), &mut surface).unwrap();
        assert_eq!(surface, root);
        assert!(IsWindowVisible(s.surface.get()).as_bool());
        let _ = ShowWindow(caption.root, SW_HIDE);
        assert!(!IsWindowVisible(s.surface.get()).as_bool());
    }

    // Render through the actual native surface's region, including the top and
    // right resize strips. Its input pixels must preserve the WebView below.
    unsafe fn assert_hover_pixels(caption: &Caption, region: HRGN) {
        let dc = CreateCompatibleDC(None);
        assert!(!dc.0.is_null());
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 800,
                biHeight: -100,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
        let previous = SelectObject(dc, bitmap.into());
        assert_ne!(SelectClipRgn(dc, Some(region)).0, 0);
        let s = &caption.state;
        let bounds = RECT {
            left: 0,
            top: 0,
            right: 800,
            bottom: 100,
        };
        for hot in Button::ALL.into_iter().map(Some).chain([None]) {
            if let Some(button) = hot {
                SendMessageW(
                    caption.root,
                    WM_NCMOUSEMOVE,
                    Some(WPARAM(button.hit() as usize)),
                    None,
                );
            } else {
                SendMessageW(caption.root, WM_NCMOUSELEAVE, None, None);
            }
            paint_surface(dc, bounds, s, false);
            for button in Button::ALL {
                let r = button.rect(s.rect.get());
                let expected = color(if hot == Some(button) {
                    if button == Button::Close {
                        [196, 43, 28, 255]
                    } else {
                        s.palette.get().hover
                    }
                } else {
                    s.palette.get().input
                });
                // Both samples lie in the portions formerly cut off by the 6px border.
                assert_eq!(
                    GetPixel(dc, (r.left + r.right) / 2, 1),
                    expected,
                    "top strip: {button:?}, hot={hot:?}"
                );
                if button == Button::Close {
                    assert_eq!(
                        GetPixel(dc, r.right - 1, (r.top + r.bottom) / 2),
                        expected,
                        "right strip, hot={hot:?}"
                    );
                }
            }
        }
        SelectObject(dc, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
    }
}
