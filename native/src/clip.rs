//! Clip native windows, including the root frame and wry/WebView2 descendants.
//! Regions use physical window coordinates; CSS only describes the desired shape.
use std::cell::Cell;
use windows::{
    core::{Error, Result},
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::Gdi::{
            CombineRgn, CreateEllipticRgn, CreateRectRgn, DeleteObject, SetWindowRgn, HRGN,
            RGN_DIFF,
        },
        UI::WindowsAndMessaging::{GetClientRect, GetWindowRect},
    },
};

/// Before SetWindowRgn succeeds, we own the region; afterward USER32 owns it.
struct Region(Option<HRGN>);
impl Region {
    fn new(handle: HRGN) -> Result<Self> {
        if handle.0.is_null() {
            Err(Error::from_thread())
        } else {
            Ok(Self(Some(handle)))
        }
    }
    fn handle(&self) -> HRGN {
        self.0.unwrap()
    }
}
impl Drop for Region {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            // SAFETY: this handle is owned here and has not been given to USER32.
            unsafe {
                let _ = DeleteObject(handle.into());
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shape {
    width: i32,
    height: i32,
    radii: [i32; 4],
}
impl Shape {
    fn new(width: i32, height: i32, corners: [f64; 4], scale: f64) -> Self {
        let limit = width.min(height).max(0) / 2;
        Self {
            width,
            height,
            radii: corners.map(|r| ((r * scale).round() as i32).clamp(0, limit)),
        }
    }
    fn region(self) -> Result<Region> {
        // SAFETY: all coordinates come from bounded dimensions/radii; temporary
        // GDI handles are cleaned by Region even when an operation fails.
        unsafe {
            let result = Region::new(CreateRectRgn(0, 0, self.width, self.height))?;
            for (corner, r) in self.radii.into_iter().enumerate().filter(|(_, r)| *r > 0) {
                let right = corner == 1 || corner == 2;
                let bottom = corner >= 2;
                let x = if right { self.width - r } else { 0 };
                let y = if bottom { self.height - r } else { 0 };
                let ex = if right { self.width - r * 2 } else { 0 };
                let ey = if bottom { self.height - r * 2 } else { 0 };
                let cut = Region::new(CreateRectRgn(x, y, x + r, y + r))?;
                let circle = Region::new(CreateEllipticRgn(ex, ey, ex + r * 2, ey + r * 2))?;
                // Remove only the part of each corner square outside its circle.
                if CombineRgn(
                    Some(cut.handle()),
                    Some(cut.handle()),
                    Some(circle.handle()),
                    RGN_DIFF,
                )
                .0 == 0
                    || CombineRgn(
                        Some(result.handle()),
                        Some(result.handle()),
                        Some(cut.handle()),
                        RGN_DIFF,
                    )
                    .0 == 0
                {
                    return Err(Error::from_thread());
                }
            }
            Ok(result)
        }
    }
}

#[derive(Default)]
pub struct WindowClip {
    last: Cell<Option<Shape>>,
}
impl WindowClip {
    /// Root window regions include non-client bounds. Clear the custom region
    /// while maximized so Windows can manage the monitor/work-area edges.
    pub fn apply_window(&self, hwnd: HWND, radius: f64, scale: f64, maximized: bool) -> Result<()> {
        if maximized {
            if self.last.get().is_some() {
                // SAFETY: hwnd belongs to the live winit window; USER32 releases
                // the previous region when it is replaced with NULL.
                if unsafe { SetWindowRgn(hwnd, None, true) } == 0 {
                    return Err(Error::from_thread());
                }
                self.last.set(None);
            }
            return Ok(());
        }
        let mut bounds = RECT::default();
        // SAFETY: the caller supplies its live top-level window HWND.
        unsafe {
            GetWindowRect(hwnd, &mut bounds)?;
        }
        self.apply_shape(
            hwnd,
            Shape::new(
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                [radius; 4],
                scale,
            ),
        )
    }

    pub fn apply(&self, hwnd: HWND, corners: [f64; 4], scale: f64) -> Result<()> {
        let mut bounds = RECT::default();
        // SAFETY: hwnd is the live borderless child returned by wry, not the root
        // window or a guessed WebView2 implementation detail.
        unsafe {
            GetClientRect(hwnd, &mut bounds)?;
        }
        let shape = Shape::new(bounds.right, bounds.bottom, corners, scale);
        self.apply_shape(hwnd, shape)
    }

    fn apply_shape(&self, hwnd: HWND, shape: Shape) -> Result<()> {
        if shape.width <= 0 || shape.height <= 0 || self.last.get() == Some(shape) {
            return Ok(());
        }
        let mut region = shape.region()?;
        // SAFETY: ownership transfers only on success. On failure the RAII guard
        // still deletes it; never delete a region installed on a window.
        if unsafe { SetWindowRgn(hwnd, Some(region.handle()), true) } == 0 {
            return Err(Error::from_thread());
        }
        let _ = region.0.take();
        self.last.set(Some(shape));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::PtInRegion;
    fn contains(region: &Region, x: i32, y: i32) -> bool {
        unsafe { PtInRegion(region.handle(), x, y).as_bool() }
    }
    #[test]
    fn single_page_clips_all_four_real_gdi_corners() {
        let r = Shape::new(100, 60, [7.; 4], 1.).region().unwrap();
        for (x, y) in [(0, 0), (99, 0), (99, 59), (0, 59)] {
            assert!(!contains(&r, x, y));
        }
        for (x, y) in [(7, 0), (0, 7), (50, 30), (99, 30), (50, 59)] {
            assert!(contains(&r, x, y));
        }
    }
    #[test]
    fn split_headers_and_inner_seams_stay_square() {
        let left = Shape::new(100, 60, [0., 0., 0., 7.], 1.).region().unwrap();
        let right = Shape::new(100, 60, [0., 0., 7., 0.], 1.).region().unwrap();
        assert!(contains(&left, 0, 0) && contains(&left, 99, 59));
        assert!(!contains(&left, 0, 59));
        assert!(contains(&right, 0, 59) && contains(&right, 99, 0));
        assert!(!contains(&right, 99, 59));
    }
    #[test]
    fn scales_for_dpi_and_clamps_small_windows() {
        assert_eq!(Shape::new(100, 60, [7.; 4], 1.5).radii, [11; 4]);
        assert_eq!(Shape::new(8, 6, [7.; 4], 2.).radii, [3; 4]);
    }
    #[test]
    fn installs_and_updates_the_region_on_a_real_hwnd() {
        use windows::{
            core::w,
            Win32::{
                Graphics::Gdi::{EqualRgn, GetWindowRgn},
                UI::WindowsAndMessaging::{
                    CreateWindowExW, DestroyWindow, SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER,
                    WINDOW_EX_STYLE, WS_POPUP,
                },
            },
        };
        struct TestWindow(HWND);
        impl Drop for TestWindow {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }
        // A hidden system STATIC window: no desktop input or borrowed app HWNDs.
        let window = TestWindow(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Era clip test"),
                WS_POPUP,
                0,
                0,
                100,
                60,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        });
        let clip = WindowClip::default();
        for (width, height, corners, scale) in [
            (100, 60, [7.; 4], 1.),
            (160, 80, [0., 0., 0., 7.], 1.5),
            (70, 40, [0.; 4], 1.),
        ] {
            unsafe {
                SetWindowPos(
                    window.0,
                    None,
                    0,
                    0,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )
                .unwrap();
            }
            clip.apply(window.0, corners, scale).unwrap();
            let actual = Region::new(unsafe { CreateRectRgn(0, 0, 0, 0) }).unwrap();
            assert_ne!(unsafe { GetWindowRgn(window.0, actual.handle()) }.0, 0);
            let expected = Shape::new(width, height, corners, scale).region().unwrap();
            assert!(unsafe { EqualRgn(actual.handle(), expected.handle()) }.as_bool());
        }
        // The root frame must release its region on maximize and reinstall it
        // on restore, even when the restored dimensions match the cached shape.
        for (scale, maximized) in [
            (1., false),
            (1.5, false),
            (1.5, true),
            (1.5, true),
            (1.5, false),
        ] {
            clip.apply_window(window.0, 12., scale, maximized).unwrap();
            let actual = Region::new(unsafe { CreateRectRgn(0, 0, 0, 0) }).unwrap();
            let kind = unsafe { GetWindowRgn(window.0, actual.handle()) };
            if maximized {
                assert_eq!(kind.0, 0);
            } else {
                assert_ne!(kind.0, 0);
                let expected = Shape::new(70, 40, [12.; 4], scale).region().unwrap();
                assert!(unsafe { EqualRgn(actual.handle(), expected.handle()) }.as_bool());
            }
        }
    }
}
