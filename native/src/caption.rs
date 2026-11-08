//! Real non-client maximize button for the Windows 11 Snap Layout flyout.
//! WebView2 owns descendant HWNDs on other threads. Instead of subclassing those
//! windows, remove this button from the UI HWND region and draw it in a tiny
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
    surface: Cell<HWND>,
    rect: Cell<RECT>,
    anchor: Cell<Option<Anchor>>,
    palette: Cell<Palette>,
    hot: Cell<bool>,
    pressed: Cell<bool>,
    scale: Cell<f64>,
    cut: Cell<Option<[i32; 6]>>,
}
pub struct Caption {
    root: HWND,
    ui: HWND,
    state: Box<State>,
}
fn contains(r: RECT, p: POINT) -> bool {
    p.x >= r.left && p.x < r.right && p.y >= r.top && p.y < r.bottom
}
fn color(c: [u8; 3]) -> COLORREF {
    COLORREF(c[0] as u32 | ((c[1] as u32) << 8) | ((c[2] as u32) << 16))
}
impl Caption {
    pub fn new(root: HWND, ui: HWND) -> Result<Self> {
        let state = Box::new(State {
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
        // A same-thread, input-transparent native paint surface covers the region
        // hole. It lets the root receive NC input without WebView2 intercepting it.
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
        let Some(a) = self.state.anchor.get() else {
            return Ok(());
        };
        let mut client = RECT::default();
        unsafe {
            GetClientRect(self.ui, &mut client)?;
        }
        let width = (a.width * scale).round() as i32;
        let right = client.right - (a.right * scale).round() as i32;
        let rect = RECT {
            left: right - width,
            right,
            top: (a.top * scale).round() as i32,
            bottom: ((a.top + a.height) * scale).round() as i32,
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
        ];
        if self.state.cut.get() != Some(key) {
            // GDI temporaries are owned here until SetWindowRgn transfers ownership.
            unsafe {
                let whole = CreateRectRgn(0, 0, client.right, client.bottom);
                let hole = CreateRectRgn(rect.left, rect.top, rect.right, rect.bottom);
                if whole.0.is_null() || hole.0.is_null() {
                    if !whole.0.is_null() {
                        let _ = DeleteObject(whole.into());
                    }
                    if !hole.0.is_null() {
                        let _ = DeleteObject(hole.into());
                    }
                    return Err(Error::from_thread());
                }
                let result = CombineRgn(Some(whole), Some(whole), Some(hole), RGN_DIFF);
                let _ = DeleteObject(hole.into());
                if result.0 == 0 || SetWindowRgn(self.ui, Some(whole), true) == 0 {
                    let _ = DeleteObject(whole.into());
                    return Err(Error::from_thread());
                }
            }
            self.state.cut.set(Some(key));
        }
        unsafe {
            SetWindowPos(
                self.state.surface.get(),
                Some(HWND_TOP),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
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
            if ScreenToClient(hwnd, &mut p).as_bool() && contains(s.rect.get(), p) {
                return LRESULT(HTMAXBUTTON as isize);
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
            let _ = RedrawWindow(
                Some(s.surface.get()),
                None,
                None,
                RDW_INVALIDATE | RDW_UPDATENOW,
            );
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
            let brush = CreateSolidBrush(color(if s.hot.get() { p.hover } else { p.background }));
            let _ = FillRect(dc, &r, brush);
            let _ = DeleteObject(brush.into());
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
        }
    }
}
