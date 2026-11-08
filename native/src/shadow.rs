//! Click-through shadow behind the region-clipped root HWND. A normal DWM
//! shadow is suppressed by SetWindowRgn; this layered tool window preserves the
//! custom rounded outline. Caption owns its lifetime and keeps it behind root.
use std::cell::Cell;
use windows::{
    core::{w, Error, Result},
    Win32::{
        Foundation::{COLORREF, HWND, POINT, RECT, SIZE},
        Graphics::Gdi::*,
        UI::WindowsAndMessaging::*,
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Shape {
    width: i32,
    height: i32,
    scale: f64,
    radius: f64,
}
impl Shape {
    fn padding(self) -> i32 {
        (48. * self.scale).ceil() as i32
    }
    fn size(self) -> SIZE {
        SIZE {
            cx: self.width + 2 * self.padding(),
            cy: self.height + 2 * self.padding(),
        }
    }
    fn distance(self, x: f64, y: f64, offset: f64) -> f64 {
        let (hw, hh) = (self.width as f64 / 2., self.height as f64 / 2.);
        let r = (self.radius * self.scale).min(hw.min(hh));
        let x = (x - self.padding() as f64 - hw).abs() - hw + r;
        let y = (y - self.padding() as f64 - hh - offset).abs() - hh + r;
        x.max(0.).hypot(y.max(0.)) + x.max(y).min(0.) - r
    }
    fn paint(self, pixels: &mut [u32]) {
        let size = self.size();
        pixels.fill(0);
        // Only shade the perimeter. Moving reuses USER32's cached bitmap.
        let inset = self.padding() + (self.radius * self.scale).ceil() as i32;
        for y in 0..size.cy {
            let ranges = if y >= inset && y < size.cy - inset {
                [0..inset, size.cx - inset..size.cx]
            } else {
                [0..size.cx, 0..0]
            };
            for x in ranges.into_iter().flatten() {
                let d = self.distance(x as f64 + 0.5, y as f64 + 0.5, 0.);
                if d <= 0. {
                    continue;
                }
                let ambient_d = self
                    .distance(x as f64 + 0.5, y as f64 + 0.5, 6. * self.scale)
                    .max(0.);
                let ambient = 0.18 * (-0.5 * (ambient_d / (12. * self.scale)).powi(2)).exp();
                let contact = 0.14 * (-d / (2. * self.scale)).exp();
                let alpha = ((1. - (1. - ambient) * (1. - contact)) * 255.).round() as u32;
                // Premultiplied BGRA: black has zero RGB at every alpha.
                pixels[(y * size.cx + x) as usize] = alpha << 24;
            }
        }
    }
}

struct Bitmap {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Bitmap {
    fn new(shape: Shape) -> Result<Self> {
        let size = shape.size();
        let dc = unsafe { CreateCompatibleDC(None) };
        if dc.0.is_null() {
            return Err(Error::from_thread());
        }
        let mut bitmap = Self {
            dc,
            bitmap: HBITMAP::default(),
            previous: HGDIOBJ::default(),
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
        // SAFETY: this top-down 32-bit DIB contains width * height pixels and
        // its returned storage remains valid until Bitmap drops.
        unsafe {
            bitmap.bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
            bitmap.previous = SelectObject(dc, bitmap.bitmap.into());
            shape.paint(std::slice::from_raw_parts_mut(
                bits.cast(),
                size.cx as usize * size.cy as usize,
            ));
        }
        Ok(bitmap)
    }
}
impl Drop for Bitmap {
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

pub struct Shadow {
    root: HWND,
    window: HWND,
    last: Cell<Option<Shape>>,
    updating: Cell<bool>,
}
impl Shadow {
    pub fn new(root: HWND) -> Result<Self> {
        let window = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!(""),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )?
        };
        Ok(Self {
            root,
            window,
            last: Cell::new(None),
            updating: Cell::new(false),
        })
    }
    pub fn refresh(&self, scale: f64, radius: f64) -> Result<()> {
        // Positioning the sibling can synchronously notify the root again.
        if self.updating.replace(true) {
            return Ok(());
        }
        let result = self.sync(scale, radius);
        self.updating.set(false);
        result
    }
    fn sync(&self, scale: f64, radius: f64) -> Result<()> {
        unsafe {
            if !IsWindowVisible(self.root).as_bool()
                || IsIconic(self.root).as_bool()
                || IsZoomed(self.root).as_bool()
            {
                let _ = ShowWindow(self.window, SW_HIDE);
                return Ok(());
            }
            let mut bounds = RECT::default();
            GetWindowRect(self.root, &mut bounds)?;
            let shape = Shape {
                width: bounds.right - bounds.left,
                height: bounds.bottom - bounds.top,
                scale,
                radius,
            };
            if shape.width <= 0 || shape.height <= 0 {
                return Ok(());
            }
            let position = POINT {
                x: bounds.left - shape.padding(),
                y: bounds.top - shape.padding(),
            };
            if self.last.get() != Some(shape) {
                let bitmap = Bitmap::new(shape)?;
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                UpdateLayeredWindow(
                    self.window,
                    None,
                    Some(&position),
                    Some(&shape.size()),
                    Some(bitmap.dc),
                    Some(&POINT::default()),
                    COLORREF(0),
                    Some(&blend),
                    ULW_ALPHA,
                )?;
                self.last.set(Some(shape));
            }
            // Unowned tool windows may sit behind root. Owned popups would be
            // forced above it and could cover content or menus.
            SetWindowPos(
                self.window,
                Some(self.root),
                position.x,
                position.y,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
        }
        Ok(())
    }
}
impl Drop for Shadow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shadow_fades_outside_all_rounded_edges_at_each_dpi() {
        for scale in [1., 1.5, 2.] {
            let shape = Shape {
                width: (160. * scale) as i32,
                height: (100. * scale) as i32,
                scale,
                radius: 12.,
            };
            let size = shape.size();
            let mut pixels = vec![0; (size.cx * size.cy) as usize];
            shape.paint(&mut pixels);
            let alpha = |x: i32, y: i32| pixels[(y * size.cx + x) as usize] >> 24;
            let p = shape.padding();
            let (cx, cy) = (size.cx / 2, size.cy / 2);
            let (near, far) = ((2. * scale) as i32, (24. * scale) as i32);
            for (a, b) in [
                ((p - near, cy), (p - far, cy)),
                ((size.cx - p + near, cy), (size.cx - p + far, cy)),
                ((cx, p - near), (cx, p - far)),
                ((cx, size.cy - p + near), (cx, size.cy - p + far)),
            ] {
                assert!(alpha(a.0, a.1) > alpha(b.0, b.1));
                assert!(alpha(b.0, b.1) > 0);
            }
            assert_eq!(alpha(cx, cy), 0);
            assert!(alpha(p, p) > 0, "shadow follows the rounded corner");
            assert_eq!(alpha(0, cy), 0);
            assert_eq!(alpha(cx, size.cy - 1), 0);
            assert!(pixels.iter().all(|pixel| pixel & 0x00ff_ffff == 0));
        }
    }
}
