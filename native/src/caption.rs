//! Native resize borders and caption buttons, including the Windows 11 Snap flyout.
//! WebView2 owns descendant HWNDs on other threads. Instead of subclassing those
//! windows, remove the borders/buttons from the UI HWND region and draw them in an
//! input-transparent child. The root receives WM_NCHITTEST/HTMAXBUTTON and the
//! system supplies its own Snap menu. No synthetic layout flyout.
use crate::model::Rect;
use std::cell::Cell;
use windows::{
    core::{w, Error, Result},
    Win32::{
        Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::*,
        UI::{
            Controls::{
                BeginBufferedPaint, BufferedPaintInit, BufferedPaintUnInit, EndBufferedPaint,
                BPBF_COMPATIBLEBITMAP,
            },
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
// UxTheme's private frame-paint messages are also suppressed by Chromium's
// custom frame (ui/views/win/hwnd_message_handler.cc). USER32 can send these
// while tracking caption hover even when the window has no standard frame.
const WM_NCUAHDRAWCAPTION: u32 = 0x00ae;
const WM_NCUAHDRAWFRAME: u32 = 0x00af;
struct BufferedPainting;
impl BufferedPainting {
    fn new() -> Result<Self> {
        unsafe {
            BufferedPaintInit()?;
        }
        Ok(Self)
    }
}
impl Drop for BufferedPainting {
    fn drop(&mut self) {
        unsafe {
            let _ = BufferedPaintUnInit();
        }
    }
}
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
    background: [u8; 3],
    hover: [u8; 3],
    foreground: [u8; 3],
}
struct State {
    _painting: BufferedPainting,
    shadow: crate::shadow::Shadow,
    surface: Cell<HWND>,
    rect: Cell<RECT>,
    anchor: Cell<Option<Anchor>>,
    palette: Cell<Palette>,
    hot: Cell<Option<Button>>,
    pressed: Cell<Option<Button>>,
    scale: Cell<f64>,
    cut: Cell<Option<[i32; 7]>>,
}
pub struct Caption {
    root: HWND,
    ui: HWND,
    state: Box<State>,
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
fn color(c: [u8; 3]) -> COLORREF {
    COLORREF(c[0] as u32 | ((c[1] as u32) << 8) | ((c[2] as u32) << 16))
}
impl Caption {
    pub fn new(root: HWND, ui: HWND) -> Result<Self> {
        let state = Box::new(State {
            _painting: BufferedPainting::new()?,
            shadow: crate::shadow::Shadow::new(root)?,
            surface: Cell::new(HWND::default()),
            rect: Cell::new(RECT::default()),
            anchor: Cell::new(None),
            palette: Cell::new(Palette {
                background: [212, 228, 251],
                hover: [228, 239, 253],
                foreground: [50, 55, 62],
            }),
            hot: Cell::new(None),
            pressed: Cell::new(None),
            scale: Cell::new(1.),
            cut: Cell::new(None),
        });
        // This same-thread paint surface covers only the borders/buttons. Its
        // HTTRANSPARENT result lets the root receive real non-client input before
        // WebView2 can capture the pointer; no asynchronous drag IPC is needed.
        let surface = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE,
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
        Ok(Self { root, ui, state })
    }
    pub fn update(
        &self,
        rect: Rect,
        viewport_width: f64,
        background: [u8; 3],
        hover: [u8; 3],
        foreground: [u8; 3],
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
            background,
            hover,
            foreground,
        });
        self.refresh(scale)
    }
    pub fn refresh(&self, scale: f64) -> Result<()> {
        self.state
            .shadow
            .refresh(scale, crate::host::WINDOW_RADIUS)?;
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
            // Paint all three controls on the same surface. Leaving minimize/
            // close in WebView2 would clip their hover fill at the resize inset.
            let button = Region::new(button_bounds(rect))?;
            let surface = Region::new(client)?;
            unsafe {
                if CombineRgn(Some(interior.0), Some(interior.0), Some(button.0), RGN_DIFF).0 == 0
                    || CombineRgn(Some(surface.0), Some(surface.0), Some(interior.0), RGN_DIFF).0
                        == 0
                {
                    return Err(Error::from_thread());
                }
            }
            interior.install(self.ui)?;
            surface.install(self.state.surface.get())?;
            self.state.cut.set(Some(key));
        }
        unsafe {
            SetWindowPos(
                self.state.surface.get(),
                Some(HWND_TOP),
                0,
                0,
                client.right,
                client.bottom,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
            let _ = RedrawWindow(
                Some(self.state.surface.get()),
                None,
                None,
                RDW_INVALIDATE | RDW_UPDATENOW,
            );
        }
        Ok(())
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
        WM_NCHITTEST => {
            if hwnd == s.surface.get() {
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
            // Preserve USER32's Snap tracking. Its paint messages are suppressed
            // below, so the system never draws over the custom controls.
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
        WM_MOUSEMOVE if hwnd != s.surface.get() && s.pressed.get().is_some() => {
            let point = POINT {
                x: (lparam.0 as u16 as i16) as i32,
                y: ((lparam.0 >> 16) as u16 as i16) as i32,
            };
            let hot = s
                .pressed
                .get()
                .filter(|button| contains(button.rect(s.rect.get()), point));
            set_hot(s, hot);
            return LRESULT(0);
        }
        WM_LBUTTONUP | WM_NCLBUTTONUP if hwnd != s.surface.get() && s.pressed.get().is_some() => {
            let button = s.pressed.take().unwrap();
            let mut point = POINT::default();
            let inside = GetCursorPos(&mut point).is_ok()
                && ScreenToClient(hwnd, &mut point).as_bool()
                && contains(button.rect(s.rect.get()), point);
            set_hot(s, inside.then_some(button));
            let _ = ReleaseCapture();
            if inside {
                let command = button.command(IsZoomed(hwnd).as_bool());
                let _ = SendMessageW(
                    hwnd,
                    WM_SYSCOMMAND,
                    Some(WPARAM(command as usize)),
                    Some(LPARAM(0)),
                );
            }
            return LRESULT(0);
        }
        WM_CAPTURECHANGED => {
            s.pressed.set(None);
        }
        WM_NCACTIVATE if hwnd != s.surface.get() => {
            // Keep winit's activation/focus handling; -1 asks DefWindowProc to
            // update activation without repainting the standard non-client frame.
            let result = DefSubclassProc(hwnd, msg, wparam, LPARAM(-1));
            let _ = s.shadow.refresh(s.scale.get(), crate::host::WINDOW_RADIUS);
            return result;
        }
        WM_SIZE if hwnd != s.surface.get() => {
            let result = DefSubclassProc(hwnd, msg, wparam, lparam);
            let _ = s.shadow.refresh(s.scale.get(), crate::host::WINDOW_RADIUS);
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
            let _ = s.shadow.refresh(s.scale.get(), crate::host::WINDOW_RADIUS);
            return result;
        }
        WM_PAINT => {
            if hwnd != s.surface.get() {
                return DefSubclassProc(hwnd, msg, wparam, lparam);
            }
            let mut paint = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut r = RECT::default();
            let _ = GetClientRect(hwnd, &mut r);
            paint_buffered(
                dc,
                paint.rcPaint,
                r,
                s,
                IsZoomed(GetAncestor(hwnd, GA_ROOT)).as_bool(),
            );
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
    // DefWindowProc sees real non-client mouse messages and supplies Windows' flyout.
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
unsafe fn set_hot(s: &State, hot: Option<Button>) {
    let previous = s.hot.replace(hot);
    if previous != hot {
        for button in [previous, hot].into_iter().flatten() {
            let rect = button.rect(s.rect.get());
            let _ = InvalidateRect(Some(s.surface.get()), Some(&rect), false);
        }
    }
}
unsafe fn paint_buffered(dc: HDC, dirty: RECT, bounds: RECT, s: &State, maximized: bool) {
    if dirty.right <= dirty.left || dirty.bottom <= dirty.top {
        return;
    }
    let mut buffer_dc = HDC::default();
    let buffer = BeginBufferedPaint(dc, &dirty, BPBF_COMPATIBLEBITMAP, None, &mut buffer_dc);
    if buffer == 0 {
        paint_surface(dc, bounds, s, maximized);
    } else {
        // Present background and glyphs together, never a cleared intermediate frame.
        paint_surface(buffer_dc, bounds, s, maximized);
        let _ = EndBufferedPaint(buffer, true);
    }
}
unsafe fn paint_surface(dc: HDC, bounds: RECT, s: &State, maximized: bool) {
    let brush = CreateSolidBrush(color(s.palette.get().background));
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
        [196, 43, 28]
    } else if hot {
        p.hover
    } else {
        p.background
    };
    let brush = CreateSolidBrush(color(background));
    let _ = FillRect(dc, &r, brush);
    let _ = DeleteObject(brush.into());
    let pen = CreatePen(
        PS_SOLID,
        (s.scale.get().round() as i32).max(1),
        color(if hot && button == Button::Close {
            [255; 3]
        } else {
            p.foreground
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
    fn real_hwnd_hit_testing_and_ui_hole_follow_dpi() {
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
            assert_ne!(GetWindowRgn(ui, initial_region).0, 0);
            assert!(!PtInRegion(initial_region, 3, 300).as_bool());
            assert!(PtInRegion(initial_region, 400, 300).as_bool());
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
                    [212, 228, 251],
                    [228, 239, 253],
                    [40, 40, 40],
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
                assert_ne!(GetWindowRgn(ui, region).0, 0);
                assert!(
                    !PtInRegion(region, (r.left + r.right) / 2, (r.top + r.bottom) / 2).as_bool()
                );
                assert!(PtInRegion(region, 20, 20).as_bool());
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
                assert_ne!(GetWindowRgn(ui, ui_region).0, 0);
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
                    assert!(!PtInRegion(ui_region, x, y).as_bool());
                    assert!(PtInRegion(surface_region, x, y).as_bool());
                }
                assert!(PtInRegion(ui_region, border, 300).as_bool());
                assert!(!PtInRegion(surface_region, border, 300).as_bool());
                for button in Button::ALL {
                    let rect = button.rect(r);
                    let (x, y) = ((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2);
                    assert!(!PtInRegion(ui_region, x, y).as_bool());
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
                assert!(PtInRegion(ui_region, button_bounds(r).left - 1, 20).as_bool());
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
            assert_ne!(GetWindowRgn(ui, region).0, 0);
            assert!(PtInRegion(region, 1, 300).as_bool());
            let _ = SetWindowLongPtrW(root.0, GWL_STYLE, style);
            caption.refresh(2.).unwrap();
            assert_ne!(GetWindowRgn(ui, region).0, 0);
            assert!(!PtInRegion(region, 1, 300).as_bool());
            let _ = DeleteObject(region.into());
            assert_stable_hover(&caption);
        }
    }

    unsafe fn assert_stable_hover(caption: &Caption) {
        unsafe extern "system" fn count_paints(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
            _: usize,
            data: usize,
        ) -> LRESULT {
            if msg == WM_PAINT {
                let paints = &*(data as *const Cell<usize>);
                paints.set(paints.get() + 1);
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        struct PaintCounter {
            hwnd: HWND,
            paints: Box<Cell<usize>>,
        }
        impl Drop for PaintCounter {
            fn drop(&mut self) {
                unsafe {
                    let _ = RemoveWindowSubclass(self.hwnd, Some(count_paints), SUBCLASS + 2);
                }
            }
        }
        let counter = PaintCounter {
            hwnd: caption.state.surface.get(),
            paints: Box::new(Cell::new(0)),
        };
        assert!(SetWindowSubclass(
            counter.hwnd,
            Some(count_paints),
            SUBCLASS + 2,
            (&*counter.paints as *const Cell<usize>) as usize
        )
        .as_bool());
        // A visible off-screen HWND exercises WM_PAINT dispatch, unlike a
        // hidden test window where USER32 can silently discard redraw requests.
        let _ = ShowWindow(caption.root, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(counter.hwnd);
        for button in Button::ALL {
            SendMessageW(
                caption.root,
                WM_NCMOUSEMOVE,
                Some(WPARAM(button.hit() as usize)),
                None,
            );
            let _ = UpdateWindow(counter.hwnd);
            let paints = counter.paints.get();
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
                counter.paints.get(),
                paints,
                "stationary hover must not force repaint: {button:?}"
            );
            assert!(!GetUpdateRect(counter.hwnd, None, false).as_bool());
            // Leaving repaints once to clear hover, then stays idle as well.
            SendMessageW(caption.root, WM_NCMOUSELEAVE, None, None);
            let _ = UpdateWindow(counter.hwnd);
            assert!(counter.paints.get() > paints);
            let paints = counter.paints.get();
            SendMessageW(caption.root, WM_NCMOUSELEAVE, None, None);
            let _ = UpdateWindow(counter.hwnd);
            assert_eq!(counter.paints.get(), paints);
        }
    }

    // Render through the actual native surface's region. The regression was
    // visible only at the top/right resize strips, outside WebView2's region.
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
            paint_buffered(dc, button_bounds(s.rect.get()), bounds, s, false);
            for button in Button::ALL {
                let r = button.rect(s.rect.get());
                let expected = color(if hot == Some(button) {
                    if button == Button::Close {
                        [196, 43, 28]
                    } else {
                        s.palette.get().hover
                    }
                } else {
                    s.palette.get().background
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
