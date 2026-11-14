//! Light live blur behind the transparent frame, without Acrylic tint/noise.
use std::cell::{Cell, RefCell};
use windows::{
    core::{implement, w, Error, Interface, Result, GUID, HSTRING, PCWSTR},
    Foundation::{IPropertyValue, PropertyValue},
    Graphics::Effects::{
        IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectSource_Impl, IGraphicsEffect_Impl,
    },
    System::{DispatcherQueue, DispatcherQueueController},
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWINDOWATTRIBUTE},
        System::WinRT::{
            Composition::ICompositorDesktopInterop,
            CreateDispatcherQueueController, DispatcherQueueOptions,
            Graphics::Direct2D::{
                IGraphicsEffectD2D1Interop, IGraphicsEffectD2D1Interop_Impl,
                GRAPHICS_EFFECT_PROPERTY_MAPPING,
            },
            RoInitialize, RoUninitialize, DQTAT_COM_NONE, DQTYPE_THREAD_CURRENT,
            RO_INIT_SINGLETHREADED,
        },
        UI::{
            Controls::MARGINS,
            HiDpi::GetDpiForWindow,
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
    UI::Composition::{
        CompositionEffectSourceParameter, CompositionRoundedRectangleGeometry, Compositor,
        Desktop::DesktopWindowTarget, SpriteVisual,
    },
};

const SYSTEM_BACKDROP: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(38);
// A small Gaussian radius softens detail while preserving the clear desktop.
const BLUR_SIGMA: f32 = 4.;
const SUBCLASS: usize = 0x524f5647;

// Backdrop brushes are effect inputs, not stand-alone SpriteVisual brushes.
// Apply only a light Gaussian blur; the shell supplies its own color wash.
#[implement(IGraphicsEffect, IGraphicsEffectSource, IGraphicsEffectD2D1Interop)]
struct BlurEffect {
    source: IGraphicsEffectSource,
    name: RefCell<HSTRING>,
}
impl IGraphicsEffectSource_Impl for BlurEffect_Impl {}
impl IGraphicsEffect_Impl for BlurEffect_Impl {
    fn Name(&self) -> Result<HSTRING> {
        Ok(self.name.borrow().clone())
    }
    fn SetName(&self, name: &HSTRING) -> Result<()> {
        *self.name.borrow_mut() = name.clone();
        Ok(())
    }
}
impl IGraphicsEffectD2D1Interop_Impl for BlurEffect_Impl {
    fn GetEffectId(&self) -> Result<GUID> {
        Ok(GUID::from_u128(0x1feb6d69_2fe6_4ac9_8c58_1d7f93e7a6a5))
    }
    fn GetNamedPropertyMapping(
        &self,
        _: &PCWSTR,
        _: *mut u32,
        _: *mut GRAPHICS_EFFECT_PROPERTY_MAPPING,
    ) -> Result<()> {
        Err(windows::Win32::Foundation::E_INVALIDARG.into())
    }
    fn GetPropertyCount(&self) -> Result<u32> {
        Ok(3)
    }
    fn GetProperty(&self, index: u32) -> Result<IPropertyValue> {
        match index {
            0 => PropertyValue::CreateSingle(BLUR_SIGMA),
            1 => PropertyValue::CreateUInt32(1), // Balanced optimization.
            2 => PropertyValue::CreateUInt32(1), // Clamp to the edge, with no dark fringe.
            _ => Err(windows::Win32::Foundation::E_INVALIDARG.into()),
        }?
        .cast()
    }
    fn GetSource(&self, index: u32) -> Result<IGraphicsEffectSource> {
        if index != 0 {
            return Err(windows::Win32::Foundation::E_INVALIDARG.into());
        }
        Ok(self.source.clone())
    }
    fn GetSourceCount(&self) -> Result<u32> {
        Ok(1)
    }
}

pub fn clear_system_backdrop(hwnd: HWND) {
    unsafe {
        let none: i32 = 1;
        let _ = DwmSetWindowAttribute(hwnd, SYSTEM_BACKDROP, &none as *const _ as _, 4);
        let _ = DwmExtendFrameIntoClientArea(hwnd, &MARGINS::default());
    }
}

struct Apartment;
impl Apartment {
    fn new() -> Result<Self> {
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED)?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}

pub struct Backdrop {
    surface: Box<Surface>,
    visual: SpriteVisual,
    target: DesktopWindowTarget,
    _compositor: Compositor,
    // Drop composition objects before their dispatcher queue and apartment.
    _queue: Option<DispatcherQueueController>,
    _apartment: Apartment,
}
// WebView2's transparent child surfaces skip a visual on their parent HWND.
// A separate click-through sibling is visible through them, like our shadow.
struct Surface {
    root: HWND,
    radius: f64,
    window: HWND,
    enabled: Cell<bool>,
    updating: Cell<bool>,
    geometry: Option<CompositionRoundedRectangleGeometry>,
    visual: Option<SpriteVisual>,
}
impl Surface {
    fn refresh(&self) -> Result<()> {
        if self.updating.replace(true) {
            return Ok(());
        }
        let result = self.sync();
        self.updating.set(false);
        result
    }
    fn sync(&self) -> Result<()> {
        unsafe {
            if !self.enabled.get()
                || !IsWindowVisible(self.root).as_bool()
                || IsIconic(self.root).as_bool()
            {
                let _ = ShowWindow(self.window, SW_HIDE);
                return Ok(());
            }
            let mut bounds = RECT::default();
            GetWindowRect(self.root, &mut bounds)?;
            if let Some(geometry) = &self.geometry {
                let mut size = geometry.Size()?;
                size.X = (bounds.right - bounds.left) as f32;
                size.Y = (bounds.bottom - bounds.top) as f32;
                geometry.SetSize(size)?;
                if let Some(visual) = &self.visual {
                    visual.SetSize(size)?;
                }
                let mut corner = geometry.CornerRadius()?;
                corner.X = if IsZoomed(self.root).as_bool() {
                    0.
                } else {
                    (self.radius * GetDpiForWindow(self.root) as f64 / 96.) as f32
                };
                corner.Y = corner.X;
                geometry.SetCornerRadius(corner)?;
            }
            SetWindowPos(
                self.window,
                Some(self.root),
                bounds.left,
                bounds.top,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
        }
        Ok(())
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.window);
        }
    }
}
unsafe extern "system" fn surface_procedure(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
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
    // Box stays alive until Backdrop removes this root subclass.
    let surface = &*(data as *const Surface);
    let result = DefSubclassProc(hwnd, msg, wparam, lparam);
    match msg {
        WM_WINDOWPOSCHANGED | WM_SHOWWINDOW | WM_SIZE | WM_NCACTIVATE | WM_DPICHANGED => {
            let _ = surface.refresh();
        }
        WM_NCDESTROY => {
            let _ = ShowWindow(surface.window, SW_HIDE);
            let _ = RemoveWindowSubclass(hwnd, Some(procedure), SUBCLASS);
        }
        _ => {}
    }
    result
}
impl Backdrop {
    pub fn new(root: HWND, radius: f64) -> Result<Self> {
        let mut bounds = RECT::default();
        unsafe {
            GetWindowRect(root, &mut bounds)?;
        }
        let hwnd = unsafe {
            let instance = HINSTANCE(GetWindowLongPtrW(root, GWLP_HINSTANCE) as *mut _);
            let _ = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(surface_procedure),
                hInstance: instance,
                lpszClassName: w!("RoveBackdrop"),
                ..Default::default()
            });
            CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                w!("RoveBackdrop"),
                w!(""),
                WS_POPUP,
                bounds.left,
                bounds.top,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                None,
                None,
                Some(instance),
                None,
            )?
        };
        let mut surface = Box::new(Surface {
            root,
            radius,
            window: hwnd,
            enabled: Cell::new(false),
            updating: Cell::new(false),
            geometry: None,
            visual: None,
        });
        let apartment = Apartment::new()?;
        let queue = if DispatcherQueue::GetForCurrentThread().is_err() {
            Some(unsafe {
                CreateDispatcherQueueController(DispatcherQueueOptions {
                    dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                    threadType: DQTYPE_THREAD_CURRENT,
                    apartmentType: DQTAT_COM_NONE,
                })?
            })
        } else {
            None
        };
        let compositor = Compositor::new()?;
        let target = unsafe {
            compositor
                .cast::<ICompositorDesktopInterop>()?
                .CreateDesktopWindowTarget(hwnd, false)?
        };
        let visual = compositor.CreateSpriteVisual()?;
        let source_name = HSTRING::from("desktop");
        let effect: IGraphicsEffect = BlurEffect {
            source: CompositionEffectSourceParameter::Create(&source_name)?.cast()?,
            name: RefCell::new(HSTRING::from("clear")),
        }
        .into();
        let brush = compositor.CreateEffectFactory(&effect)?.CreateBrush()?;
        brush.SetSourceParameter(&source_name, &compositor.CreateBackdropBrush()?)?;
        visual.SetBrush(&brush)?;
        // Keep the blur inside the root's rounded outline without a GDI window region.
        let geometry = compositor.CreateRoundedRectangleGeometry()?;
        visual.SetClip(&compositor.CreateGeometricClipWithGeometry(&geometry)?)?;
        surface.geometry = Some(geometry);
        surface.visual = Some(visual.clone());
        target.SetRoot(&visual)?;
        if !unsafe {
            SetWindowSubclass(
                root,
                Some(procedure),
                SUBCLASS,
                (&*surface as *const Surface) as usize,
            )
        }
        .as_bool()
        {
            let _ = target.Close();
            return Err(Error::from_thread());
        }
        Ok(Self {
            surface,
            visual,
            target,
            _compositor: compositor,
            _queue: queue,
            _apartment: apartment,
        })
    }
    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        self.visual.SetIsVisible(enabled)?;
        self.surface.enabled.set(enabled);
        self.surface.refresh()
    }
}
impl Drop for Backdrop {
    fn drop(&mut self) {
        unsafe {
            let _ = RemoveWindowSubclass(self.surface.root, Some(procedure), SUBCLASS);
        }
        let _ = self.target.Close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestWindow(HWND);
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }

    #[test]
    fn blur_surface_follows_root_bounds_visibility_and_lifetime() -> Result<()> {
        unsafe {
            let root = TestWindow(CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP,
                w!("STATIC"),
                w!(""),
                WS_POPUP,
                30,
                40,
                320,
                240,
                None,
                None,
                None,
                None,
            )?);
            let backdrop = Backdrop::new(root.0, 8.)?;
            let surface = backdrop.surface.window;
            backdrop.set_enabled(true)?;
            assert!(!IsWindowVisible(surface).as_bool());
            let _ = ShowWindow(root.0, SW_SHOWNOACTIVATE);
            assert!(IsWindowVisible(surface).as_bool());
            for (x, y, width, height) in [(50, 60, 400, 280), (-15, 25, 420, 310)] {
                SetWindowPos(
                    root.0,
                    None,
                    x,
                    y,
                    width,
                    height,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                )?;
                let mut bounds = RECT::default();
                GetWindowRect(surface, &mut bounds)?;
                assert_eq!(
                    (bounds.left, bounds.top, bounds.right, bounds.bottom),
                    (x, y, x + width, y + height)
                );
            }
            backdrop.set_enabled(false)?;
            assert!(!IsWindowVisible(surface).as_bool());
            backdrop.set_enabled(true)?;
            assert!(IsWindowVisible(surface).as_bool());
            let _ = ShowWindow(root.0, SW_HIDE);
            assert!(!IsWindowVisible(surface).as_bool());
            drop(backdrop);
            assert!(!IsWindow(Some(surface)).as_bool());
        }
        Ok(())
    }
}
