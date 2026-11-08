//! Native resize borders and maximize button for the Windows 11 Snap flyout.
//! WebView2 owns descendant HWNDs on other threads. Instead of subclassing those
//! windows, remove the borders/button from the UI HWND region and draw them in an
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
    shadow: crate::shadow::Shadow,
    surface: Cell<HWND>,
    rect: Cell<RECT>,
    anchor: Cell<Option<Anchor>>,
    palette: Cell<Palette>,
    hot: Cell<bool>,
    pressed: Cell<bool>,
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
            shadow: crate::shadow::Shadow::new(root)?,
            surface: Cell::new(HWND::default()),
            rect: Cell::new(RECT::default()),
            anchor: Cell::new(None),
            palette: Cell::new(Palette {
                background: [212, 228, 251],
                hover: [228, 239, 253],
                foreground: [50, 55, 62],
            }),
            hot: Cell::new(false),
            pressed: Cell::new(false),
            scale: Cell::new(1.),
            cut: Cell::new(None),
        });
        // This same-thread paint surface covers only the borders/button. Its
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
        self.state.shadow.refresh(scale, crate::host::WINDOW_RADIUS)?;
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
            let button = Region::new(rect)?;
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
                if contains(s.rect.get(), p) {
                    return LRESULT(HTMAXBUTTON as isize);
                }
            }
        }
        WM_NCMOUSEMOVE if wparam.0 == HTMAXBUTTON as usize => {
            if !s.hot.replace(true) {
                let _ = InvalidateRect(Some(s.surface.get()), None, false);
            }
            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE | TME_NONCLIENT,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut track);
        }
        WM_NCMOUSELEAVE => {
            s.hot.set(false);
            let _ = InvalidateRect(Some(s.surface.get()), None, false);
        }
        WM_NCLBUTTONDOWN if hwnd != s.surface.get() && wparam.0 == HTMAXBUTTON as usize => {
            s.pressed.set(true);
            let _ = SetCapture(hwnd);
            return LRESULT(0);
        }
        WM_LBUTTONUP | WM_NCLBUTTONUP if hwnd != s.surface.get() && s.pressed.replace(false) => {
            let mut point = POINT::default();
            let inside = GetCursorPos(&mut point).is_ok()
                && ScreenToClient(hwnd, &mut point).as_bool()
                && contains(s.rect.get(), point);
            let _ = ReleaseCapture();
            if inside {
                let command = if IsZoomed(hwnd).as_bool() {
                    SC_RESTORE
                } else {
                    SC_MAXIMIZE
                };
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
            s.pressed.set(false);
        }
        WM_NCACTIVATE | WM_SIZE if hwnd != s.surface.get() => {
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
            let p = s.palette.get();
            let brush = CreateSolidBrush(color(p.background));
            let _ = FillRect(dc, &r, brush);
            let _ = DeleteObject(brush.into());
            let r = s.rect.get();
            if s.hot.get() {
                let brush = CreateSolidBrush(color(p.hover));
                let _ = FillRect(dc, &r, brush);
                let _ = DeleteObject(brush.into());
            }
            let pen = CreatePen(
                PS_SOLID,
                (s.scale.get().round() as i32).max(1),
                color(p.foreground),
            );
            let old_pen = SelectObject(dc, pen.into());
            let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
            let side = (9. * s.scale.get()).round() as i32;
            let x = (r.left + r.right - side) / 2;
            let y = (r.top + r.bottom - side) / 2;
            if IsZoomed(GetAncestor(hwnd, GA_ROOT)).as_bool() {
                let offset = (2. * s.scale.get()).round() as i32;
                let _ = Rectangle(dc, x + offset, y, x + side + offset, y + side);
                let fill =
                    CreateSolidBrush(color(if s.hot.get() { p.hover } else { p.background }));
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
            let _ = EndPaint(hwnd, &paint);
            return LRESULT(0);
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(procedure), subclass);
        }
        _ => {}
    }
    // DefWindowProc sees real non-client mouse messages and supplies Windows' flyout.
    DefSubclassProc(hwnd, msg, wparam, lparam)
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
            for scale in [1., 1.5, 2.] {
                caption.refresh(scale).unwrap();
                let r = caption.state.rect.get();
                assert_eq!(r.right - r.left, (46. * scale) as i32);
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
                // while leaving content and adjacent caption buttons in WebView2.
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
                for x in [r.left - 10, r.right + 10] {
                    let y = (r.top + r.bottom) / 2;
                    assert!(PtInRegion(ui_region, x, y).as_bool());
                    assert!(!PtInRegion(surface_region, x, y).as_bool());
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
        }
    }
}
