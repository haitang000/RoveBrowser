//! Prefer the compositor's antialiased frame; keep regions/shadows for Windows 10.
use crate::{clip::WindowClip, shadow::Shadow};
use std::cell::Cell;
use windows::{
    core::Result,
    Win32::{
        Foundation::HWND,
        Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        },
        UI::WindowsAndMessaging::IsZoomed,
    },
};

pub struct WindowFrame {
    fallback: Option<(WindowClip, Shadow)>,
    updating: Cell<bool>,
}
impl WindowFrame {
    pub fn new(hwnd: HWND) -> Result<Self> {
        // A region on the root disables DWM rounding. Only install one when the
        // Windows 11 opt-in is unavailable; never put a layered style on the root.
        let native = unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                &DWMWA_COLOR_NONE as *const _ as _,
                4,
            );
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &DWMWCP_ROUND as *const _ as _,
                4,
            )
            .is_ok()
        };
        Ok(Self {
            fallback: if native {
                None
            } else {
                Some((WindowClip::default(), Shadow::new(hwnd)?))
            },
            updating: Cell::new(false),
        })
    }

    pub fn radius(&self) -> f64 {
        if self.fallback.is_none() {
            8.
        } else {
            12.
        }
    }

    pub fn refresh(&self, hwnd: HWND, scale: f64) -> Result<()> {
        // SetWindowRgn and the fallback shadow can synchronously notify the
        // root's subclass again before their geometry caches have been updated.
        if self.updating.replace(true) {
            return Ok(());
        }
        let result = (|| {
            if let Some((clip, shadow)) = &self.fallback {
                clip.apply_window(hwnd, self.radius(), scale, unsafe {
                    IsZoomed(hwnd).as_bool()
                })?;
                shadow.refresh(scale, self.radius())?;
            }
            Ok(())
        })();
        self.updating.set(false);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::{
        core::w,
        Win32::{
            Graphics::Gdi::{CreateRectRgn, DeleteObject, GetWindowRgn},
            UI::WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, GetWindowLongPtrW, SetWindowLongPtrW, GWL_STYLE,
                WINDOW_EX_STYLE, WS_MAXIMIZE, WS_POPUP,
            },
        },
    };

    #[test]
    fn native_frame_avoids_root_regions_and_fallback_clears_on_maximize() -> Result<()> {
        unsafe {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!(""),
                WS_POPUP,
                0,
                0,
                160,
                100,
                None,
                None,
                None,
                None,
            )?;
            let frame = WindowFrame::new(hwnd)?;
            let region = CreateRectRgn(0, 0, 0, 0);
            frame.refresh(hwnd, 1.5)?;
            if frame.fallback.is_none() {
                assert_eq!(
                    GetWindowRgn(hwnd, region).0,
                    0,
                    "DWM must own the root outline"
                );
                // BORDER_COLOR is a setter-only DWM attribute.
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_BORDER_COLOR,
                    &DWMWA_COLOR_NONE as *const _ as _,
                    4,
                )?;
            }
            drop(frame);
            // Exercise Windows 10's actual GDI path even on a Windows 11 runner.
            let fallback = WindowFrame {
                fallback: Some((WindowClip::default(), Shadow::new(hwnd)?)),
                updating: Cell::new(false),
            };
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            for maximized in [false, true, false] {
                SetWindowLongPtrW(
                    hwnd,
                    GWL_STYLE,
                    if maximized {
                        style | WS_MAXIMIZE.0 as isize
                    } else {
                        style
                    },
                );
                fallback.refresh(hwnd, 1.5)?;
                assert_eq!(GetWindowRgn(hwnd, region).0 != 0, !maximized);
            }
            drop(fallback);
            let _ = DeleteObject(region.into());
            DestroyWindow(hwnd)?;
        }
        Ok(())
    }
}
