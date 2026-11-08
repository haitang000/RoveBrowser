//! Behind-window blur for a see-through space. Zigzag uses NSVisualEffectView
//! (.sidebar, behind the window) and, where available, NSGlassEffectView.
//! Windows 11's acrylic backdrop is the same idea: the desktop is blurred, and
//! the frame color washes over it. Clear and frosted differ by that wash.
use windows::Win32::{
    Foundation::HWND,
    Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWINDOWATTRIBUTE},
    UI::Controls::MARGINS,
};

const SYSTEM_BACKDROP: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(38);
const BACKDROP_NONE: i32 = 1;
const BACKDROP_ACRYLIC: i32 = 3;

pub fn apply(hwnd: HWND, acrylic: bool) -> bool {
    unsafe {
        let backdrop = if acrylic { BACKDROP_ACRYLIC } else { BACKDROP_NONE };
        if DwmSetWindowAttribute(
            hwnd,
            SYSTEM_BACKDROP,
            &backdrop as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        )
        .is_err()
        {
            return false;
        }
        if acrylic {
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            if DwmExtendFrameIntoClientArea(hwnd, &margins).is_err() {
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    SYSTEM_BACKDROP,
                    &BACKDROP_NONE as *const i32 as *const _,
                    std::mem::size_of::<i32>() as u32,
                );
                return false;
            }
        }
        true
    }
}
