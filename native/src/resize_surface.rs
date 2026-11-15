//! Frozen WebView snapshots, projected and blended by DirectComposition.
//! Chromium only receives the final size; no layout/paint runs on our drag clock.
use crate::resize_motion::{projection, Motion, PAINT_DELAY, PAINT_LIMIT};
use std::{
    cell::{Cell, RefCell},
    sync::mpsc,
    time::Instant,
};
use webview2_com::{
    CapturePreviewCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
    },
};
use windows::{
    core::{w, Interface, Result, BOOL},
    Win32::{
        Foundation::{COLORREF, HGLOBAL, HMODULE, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::{
            Direct2D::{
                Common::{
                    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_BORDER_MODE_HARD, D2D1_COLOR_F,
                    D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
                },
                D2D1CreateDevice, ID2D1DeviceContext, D2D1_ANTIALIAS_MODE_ALIASED,
                D2D1_BITMAP_PROPERTIES1, D2D1_INTERPOLATION_MODE_LINEAR,
            },
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
            Direct3D11::{
                D3D11CreateDevice, ID3D11Device, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                D3D11_SDK_VERSION,
            },
            DirectComposition::{
                DCompositionCreateDevice3, IDCompositionDevice, IDCompositionDevice3,
                IDCompositionEffectGroup, IDCompositionGaussianBlurEffect,
                IDCompositionMatrixTransform3D, IDCompositionRectangleClip,
                IDCompositionScaleTransform, IDCompositionSurface, IDCompositionTarget,
                IDCompositionVisual,
            },
            Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK},
            Dxgi::{
                Common::{DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM},
                IDXGIAdapter, IDXGIDevice,
            },
            Imaging::{
                CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICImagingFactory,
                IWICPalette, WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom,
                WICDecodeMetadataCacheOnLoad,
            },
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize,
            StructuredStorage::CreateStreamOnHGlobal, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
        },
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SHCreateMemStream, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
};
const SUBCLASS: usize = 0x52535641;
const TIMER: usize = 1;
thread_local! {
    // All page previews belong to the UI thread. Reuse its device rather than
    // initializing D3D/D2D synchronously for each pane on every mouse press.
    static DEVICE: RefCell<Option<IDCompositionDevice>> = const { RefCell::new(None) };
}
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub start: [u8; 3],
    pub end: [u8; 3],
    pub angle: f64,
    pub dark: bool,
    pub reduce_motion: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            start: [66, 80, 107],
            end: [27, 35, 53],
            angle: 135.,
            dark: false,
            reduce_motion: false,
        }
    }
}
fn trace_enabled() -> bool {
    #[cfg(debug_assertions)]
    {
        std::env::var_os("ERA_RESIZE_TRACE").is_some()
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}
fn trace(value: serde_json::Value) {
    if trace_enabled() {
        eprintln!("Rove resize: {value}");
    }
}
pub struct Surface {
    source: HWND,
    core: Option<ICoreWebView2>,
    appearance: Cell<Appearance>,
    frame: RefCell<Option<Box<Frame>>>,
}
struct Pixels {
    width: u32,
    height: u32,
    bgra: Vec<u8>,
}
#[derive(Clone, Copy, PartialEq)]
enum Capture {
    Before,
    After,
}
struct Layer {
    visual: IDCompositionVisual,
    effect: IDCompositionEffectGroup,
    scale: IDCompositionScaleTransform,
    size: Cell<[i32; 2]>,
}
struct Frame {
    source: HWND,
    root: HWND,
    old_style: isize,
    preview: HWND,
    device: IDCompositionDevice,
    _target: IDCompositionTarget,
    _root_visual: IDCompositionVisual,
    projection: IDCompositionMatrixTransform3D,
    blur: IDCompositionGaussianBlurEffect,
    card_clip: IDCompositionRectangleClip,
    viewport_clip: IDCompositionRectangleClip,
    background_scale: IDCompositionScaleTransform,
    background: FluidBackground,
    corners: [f32; 4],
    scale_factor: f32,
    old: Layer,
    new: Layer,
    core: Option<ICoreWebView2>,
    pixels: mpsc::Receiver<(Capture, Option<Pixels>)>,
    sender: mpsc::Sender<(Capture, Option<Pixels>)>,
    bounds: Cell<[i32; 4]>,
    frozen: Cell<bool>,
    paint_requested: Cell<bool>,
    paint_ready: Cell<bool>,
    restored: Cell<bool>,
    started: Instant,
    previous_tick: Cell<f32>,
    motion: RefCell<Motion>,
}
impl Surface {
    pub fn new(source: HWND, core: Option<ICoreWebView2>, appearance: Appearance) -> Self {
        // Warm it while creating the page, before the user starts a drag.
        let _ = unsafe { composition_device() };
        Self {
            source,
            core,
            appearance: Cell::new(appearance),
            frame: RefCell::new(None),
        }
    }
    pub fn set_appearance(&self, appearance: Appearance) {
        self.appearance.set(appearance);
    }
    pub fn active(&self) -> bool {
        self.frame
            .borrow()
            .as_ref()
            .is_some_and(|f| !f.restored.get())
    }
    pub fn dragging(&self) -> bool {
        self.frame
            .borrow()
            .as_ref()
            .is_some_and(|f| !f.restored.get() && f.motion.borrow().dragging())
    }
    pub fn begin(&self, bounds: [i32; 4], corners: [f64; 4], scale: f64) -> Result<()> {
        let pose = self
            .frame
            .borrow()
            .as_ref()
            .filter(|f| !f.restored.get())
            .map(|f| f.motion.borrow_mut().sample(f.now(), 0.).pose);
        self.end();
        let frame = unsafe {
            Frame::new(
                self.source,
                self.core.clone(),
                bounds,
                self.appearance.get(),
                pose,
                corners.map(|r| (r * scale) as f32),
                scale as f32,
            )?
        };
        self.frame.replace(Some(frame));
        Ok(())
    }
    pub fn resize(&self, bounds: [i32; 4]) -> Result<()> {
        if let Some(f) = self.frame.borrow().as_ref().filter(|f| !f.restored.get()) {
            unsafe {
                f.resize(bounds)?;
            }
        }
        Ok(())
    }
    pub fn pointer(&self, x: f32, y: f32) {
        if let Some(f) = self.frame.borrow().as_ref().filter(|f| !f.restored.get()) {
            let [left, top, width, height] = f.bounds.get();
            f.motion.borrow_mut().point(
                2. * (x - left as f32) / width.max(1) as f32 - 1.,
                2. * (y - top as f32) / height.max(1) as f32 - 1.,
            );
        }
    }
    /// Called AFTER SetBounds has committed exactly one final renderer size.
    pub fn release(&self) {
        if let Some(f) = self.frame.borrow().as_ref().filter(|f| !f.restored.get()) {
            f.motion.borrow_mut().release(f.now());
            // The opaque preview now covers the FINAL page rectangle. Uncloak
            // the renderer underneath it so WebView2 can paint the new bounds;
            // leaving it cloaked can defer CapturePreview until reveal.
            unsafe {
                let cloak = BOOL(0);
                let _ = DwmSetWindowAttribute(f.source, DWMWA_CLOAK, &cloak as *const _ as _, 4);
            }
            trace(
                serde_json::json!({"phase":"release", "frozen":f.frozen.get(), "bounds":f.bounds.get()}),
            );
        }
    }
    pub fn end(&self) {
        self.frame.take();
    }
}
const GRADIENT_SIZE: u32 = 128;
const LIFTED_RADIUS: f32 = 18.;
struct FluidBlob {
    visual: IDCompositionVisual,
    scale: IDCompositionScaleTransform,
}
struct FluidBackground {
    scale: IDCompositionScaleTransform,
    blobs: Vec<FluidBlob>,
}
// Upload soft colour fields once; every animation frame only changes their
// compositor transforms. No texture redraw, browser paint or layout is needed.
fn radial(color: [u8; 3]) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((GRADIENT_SIZE * GRADIENT_SIZE * 4) as usize);
    for y in 0..GRADIENT_SIZE {
        for x in 0..GRADIENT_SIZE {
            let dx = 2. * x as f32 / (GRADIENT_SIZE - 1) as f32 - 1.;
            let dy = 2. * y as f32 / (GRADIENT_SIZE - 1) as f32 - 1.;
            let t = (1. - dx.hypot(dy)).clamp(0., 1.);
            let alpha = 0.94 * t * t * (3. - 2. * t);
            for c in [2, 1, 0] {
                pixels.push((color[c] as f32 * alpha).round() as u8);
            }
            pixels.push((255. * alpha).round() as u8);
        }
    }
    pixels
}
fn fluid_pose(index: usize, now: f32) -> [f32; 4] {
    let phase = index as f32 * 2.1;
    let t = now * 0.8;
    let cx = 0.5 + 0.52 * (t + phase).cos();
    let cy = 0.5 + 0.55 * (t * 0.83 + phase + 0.7).sin();
    let sx = 1.55 + 0.22 * (t * 0.67 + phase).sin();
    let sy = 1.65 + 0.20 * (t * 0.71 + phase).cos();
    [cx - sx / 2., cy - sy / 2., sx, sy]
}
impl FluidBackground {
    unsafe fn new(
        device: &IDCompositionDevice,
        appearance: Appearance,
    ) -> Result<(IDCompositionVisual, Self)> {
        let visual = device.CreateVisual()?;
        let scale = device.CreateScaleTransform()?;
        visual.SetTransform(&scale)?;
        visual.SetContent(&bitmap(
            device,
            GRADIENT_SIZE,
            GRADIENT_SIZE,
            Some(&gradient(appearance)),
            None,
        )?)?;
        let mut blobs = Vec::with_capacity(3);
        for color in [appearance.start, appearance.end, appearance.start] {
            let blob = device.CreateVisual()?;
            let transform = device.CreateScaleTransform()?;
            blob.SetTransform(&transform)?;
            blob.SetContent(&bitmap(
                device,
                GRADIENT_SIZE,
                GRADIENT_SIZE,
                Some(&radial(color)),
                None,
            )?)?;
            visual.AddVisual(&blob, true, None::<&IDCompositionVisual>)?;
            blobs.push(FluidBlob {
                visual: blob,
                scale: transform,
            });
        }
        Ok((visual, Self { scale, blobs }))
    }
    unsafe fn present(&self, width: f32, height: f32, now: f32) -> Result<()> {
        self.scale
            .SetScaleX2(width.max(1.) / GRADIENT_SIZE as f32)?;
        self.scale
            .SetScaleY2(height.max(1.) / GRADIENT_SIZE as f32)?;
        for (index, blob) in self.blobs.iter().enumerate() {
            let [x, y, sx, sy] = fluid_pose(index, now);
            blob.scale.SetScaleX2(sx)?;
            blob.scale.SetScaleY2(sy)?;
            blob.visual.SetOffsetX2(x * GRADIENT_SIZE as f32)?;
            blob.visual.SetOffsetY2(y * GRADIENT_SIZE as f32)?;
        }
        Ok(())
    }
}
fn gradient(appearance: Appearance) -> Vec<u8> {
    let from = appearance.start.map(f32::from);
    let to = appearance.end.map(f32::from);
    // CSS angles: zero points up, 90 degrees points right.
    let angle = (appearance.angle as f32).to_radians();
    let (dx, dy) = (angle.sin(), -angle.cos());
    let span = dx.abs() + dy.abs();
    let mut pixels = Vec::with_capacity((GRADIENT_SIZE * GRADIENT_SIZE * 4) as usize);
    for y in 0..GRADIENT_SIZE {
        for x in 0..GRADIENT_SIZE {
            let t = (0.5
                + ((x as f32 / (GRADIENT_SIZE - 1) as f32 - 0.5) * dx
                    + (y as f32 / (GRADIENT_SIZE - 1) as f32 - 0.5) * dy)
                    / span)
                .clamp(0., 1.);
            for c in [2, 1, 0] {
                pixels.push((from[c] + (to[c] - from[c]) * t).round() as u8);
            }
            pixels.push(255);
        }
    }
    pixels
}
unsafe fn rounded(
    clip: &IDCompositionRectangleClip,
    width: f32,
    height: f32,
    radii: [f32; 4],
) -> Result<()> {
    let [tl, tr, br, bl] = radii.map(|r| r.clamp(0., width.min(height).max(0.) / 2.));
    clip.SetLeft2(0.)?;
    clip.SetTop2(0.)?;
    clip.SetRight2(width)?;
    clip.SetBottom2(height)?;
    clip.SetTopLeftRadiusX2(tl)?;
    clip.SetTopLeftRadiusY2(tl)?;
    clip.SetTopRightRadiusX2(tr)?;
    clip.SetTopRightRadiusY2(tr)?;
    clip.SetBottomRightRadiusX2(br)?;
    clip.SetBottomRightRadiusY2(br)?;
    clip.SetBottomLeftRadiusX2(bl)?;
    clip.SetBottomLeftRadiusY2(bl)
}
impl Layer {
    unsafe fn new(device: &IDCompositionDevice, size: [i32; 2]) -> Result<Self> {
        let visual = device.CreateVisual()?;
        let effect = device.CreateEffectGroup()?;
        let scale = device.CreateScaleTransform()?;
        visual.SetEffect(&effect)?;
        visual.SetTransform(&scale)?;
        Ok(Self {
            visual,
            effect,
            scale,
            size: Cell::new(size),
        })
    }
    unsafe fn fit(&self, size: [i32; 2]) -> Result<()> {
        self.scale
            .SetScaleX2(size[0].max(1) as f32 / self.size.get()[0].max(1) as f32)?;
        self.scale
            .SetScaleY2(size[1].max(1) as f32 / self.size.get()[1].max(1) as f32)
    }
    unsafe fn bitmap(&self, device: &IDCompositionDevice, pixels: Pixels) -> Result<()> {
        let surface = bitmap(
            device,
            pixels.width,
            pixels.height,
            Some(&pixels.bgra),
            None,
        )?;
        self.visual.SetContent(&surface)?;
        self.size.set([pixels.width as i32, pixels.height as i32]);
        Ok(())
    }
}
unsafe fn composition_device() -> Result<IDCompositionDevice> {
    if let Some(device) = DEVICE.with(|cached| cached.borrow().clone()) {
        return Ok(device);
    }
    let mut d3d: Option<ID3D11Device> = None;
    D3D11CreateDevice(
        None::<&IDXGIAdapter>,
        D3D_DRIVER_TYPE_HARDWARE,
        HMODULE::default(),
        D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        None,
        D3D11_SDK_VERSION,
        Some(&mut d3d),
        None,
        None,
    )
    .or_else(|_| {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d),
            None,
            None,
        )
    })?;
    let dxgi: IDXGIDevice = d3d.unwrap().cast()?;
    let d2d = D2D1CreateDevice(&dxgi, None)?;
    let device: IDCompositionDevice = DCompositionCreateDevice3(&d2d)?;
    DEVICE.with(|cached| cached.replace(Some(device.clone())));
    Ok(device)
}
unsafe fn bitmap(
    device: &IDCompositionDevice,
    width: u32,
    height: u32,
    pixels: Option<&[u8]>,
    color: Option<D2D1_COLOR_F>,
) -> Result<IDCompositionSurface> {
    let surface = device.CreateSurface(
        width,
        height,
        DXGI_FORMAT_B8G8R8A8_UNORM,
        DXGI_ALPHA_MODE_PREMULTIPLIED,
    )?;
    let mut offset = POINT::default();
    let dc: ID2D1DeviceContext = surface.BeginDraw(None, &mut offset)?;
    let draw = paint_bitmap(&dc, offset, width, height, pixels, color);
    let end = surface.EndDraw();
    draw?;
    end?;
    Ok(surface)
}
unsafe fn paint_bitmap(
    dc: &ID2D1DeviceContext,
    offset: POINT,
    width: u32,
    height: u32,
    pixels: Option<&[u8]>,
    color: Option<D2D1_COLOR_F>,
) -> Result<()> {
    dc.SetDpi(96., 96.);
    let image = if let Some(pixels) = pixels {
        Some(dc.CreateBitmap(
            D2D_SIZE_U { width, height },
            Some(pixels.as_ptr().cast()),
            width * 4,
            &D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.,
                dpiY: 96.,
                ..Default::default()
            },
        )?)
    } else {
        None
    };
    let dest = D2D_RECT_F {
        left: offset.x as f32,
        top: offset.y as f32,
        right: (offset.x + width as i32) as f32,
        bottom: (offset.y + height as i32) as f32,
    };
    // BeginDraw may return reused atlas pixels, including old page snapshots.
    // Source-over cannot replace those beneath transparent blob edges. Clear
    // OUR update rectangle first, without touching neighbouring atlas entries.
    dc.PushAxisAlignedClip(&dest, D2D1_ANTIALIAS_MODE_ALIASED);
    let clear = if image.is_some() { None } else { color };
    dc.Clear(clear.as_ref().map(|v| v as *const _));
    if let Some(image) = image {
        dc.DrawBitmap(
            &image,
            Some(&dest),
            1.,
            D2D1_INTERPOLATION_MODE_LINEAR,
            None,
            None,
        );
    }
    dc.PopAxisAlignedClip();
    Ok(())
}
impl Frame {
    fn now(&self) -> f32 {
        self.started.elapsed().as_secs_f32()
    }
    unsafe fn new(
        source: HWND,
        core: Option<ICoreWebView2>,
        bounds: [i32; 4],
        appearance: Appearance,
        pose: Option<crate::resize_motion::Pose>,
        corners: [f32; 4],
        scale_factor: f32,
    ) -> Result<Box<Self>> {
        let device = composition_device()?;
        let root = GetParent(source)?;
        let preview = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW,
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
        )?;
        let built = (|| -> Result<_> {
            let target = device.CreateTargetForHwnd(preview, true)?;
            let root_visual = device.CreateVisual()?;
            let (background_visual, background) = FluidBackground::new(&device, appearance)?;
            let card = device.CreateVisual()?;
            let content = device.CreateVisual()?;
            let blurred = device.CreateVisual()?;
            let blur = device
                .cast::<IDCompositionDevice3>()?
                .CreateGaussianBlurEffect()?;
            blur.SetStandardDeviation2(0.)?;
            // Extend the page's edge colours into the filter so blur never
            // makes a translucent fringe. Its parent clips AFTER filtering.
            blur.SetBorderMode(D2D1_BORDER_MODE_HARD)?;
            blurred.SetEffect(&blur)?;
            let card_background = device.CreateVisual()?;
            let background_scale = device.CreateScaleTransform()?;
            card_background.SetTransform(&background_scale)?;
            let color = if appearance.dark {
                D2D1_COLOR_F {
                    r: 30. / 255.,
                    g: 30. / 255.,
                    b: 32. / 255.,
                    a: 1.,
                }
            } else {
                D2D1_COLOR_F {
                    r: 1.,
                    g: 1.,
                    b: 1.,
                    a: 1.,
                }
            };
            let fill = bitmap(&device, 1, 1, None, Some(color))?;
            card_background.SetContent(&fill)?;
            let card_clip = device.CreateRectangleClip()?;
            let viewport_clip = device.CreateRectangleClip()?;
            // Clip the snapshot subtree BEFORE the parent's 3D projection.
            // A clip on the projected visual cuts the destination rectangle,
            // missing the inset/perspective corners of the actual snapshot.
            content.SetClip(&card_clip)?;
            root_visual.SetClip(&viewport_clip)?;
            let effect = device.CreateEffectGroup()?;
            let projection = device.CreateMatrixTransform3D()?;
            effect.SetTransform3D(&projection)?;
            card.SetEffect(&effect)?;
            let size = [bounds[2].max(1), bounds[3].max(1)];
            let old = Layer::new(&device, size)?;
            let new = Layer::new(&device, size)?;
            new.effect.SetOpacity2(1.)?;
            root_visual.AddVisual(&background_visual, false, None::<&IDCompositionVisual>)?;
            root_visual.AddVisual(&card, true, &background_visual)?;
            card.AddVisual(&content, false, None::<&IDCompositionVisual>)?;
            content.AddVisual(&blurred, false, None::<&IDCompositionVisual>)?;
            blurred.AddVisual(&card_background, false, None::<&IDCompositionVisual>)?;
            blurred.AddVisual(&new.visual, true, &card_background)?;
            blurred.AddVisual(&old.visual, true, &new.visual)?;
            target.SetRoot(&root_visual)?;
            Ok((
                target,
                root_visual,
                projection,
                blur,
                card_clip,
                viewport_clip,
                background_scale,
                background,
                old,
                new,
            ))
        })();
        let (
            target,
            root_visual,
            projection,
            blur,
            card_clip,
            viewport_clip,
            background_scale,
            background,
            old,
            new,
        ) = match built {
            Ok(parts) => parts,
            Err(e) => {
                let _ = DestroyWindow(preview);
                return Err(e);
            }
        };
        let (sender, pixels) = mpsc::channel();
        let mut enabled = BOOL(1);
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(&mut enabled as *mut _ as _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        let mut motion = Motion::new(appearance.reduce_motion || !enabled.as_bool());
        if let Some(pose) = pose {
            motion.resume(pose);
        }
        motion.point(-1., 0.);
        let frame = Box::new(Self {
            source,
            root,
            old_style: GetWindowLongPtrW(source, GWL_EXSTYLE),
            preview,
            device,
            _target: target,
            _root_visual: root_visual,
            projection,
            blur,
            card_clip,
            viewport_clip,
            background_scale,
            background,
            corners,
            scale_factor,
            old,
            new,
            core,
            pixels,
            sender,
            bounds: Cell::new(bounds),
            frozen: Cell::new(false),
            paint_requested: Cell::new(false),
            paint_ready: Cell::new(false),
            restored: Cell::new(false),
            started: Instant::now(),
            previous_tick: Cell::new(0.),
            motion: RefCell::new(motion),
        });
        SetWindowLongPtrW(
            source,
            GWL_EXSTYLE,
            frame.old_style | WS_EX_LAYERED.0 as isize,
        );
        SetLayeredWindowAttributes(source, COLORREF(0), 255, LWA_ALPHA)?;
        // Immediate GPU feedback while the single asynchronous capture is in
        // flight; its wrapper is replaced by immutable pixels as soon as ready.
        let wrapper = frame.device.CreateSurfaceFromHwnd(source)?;
        frame.old.visual.SetContent(&wrapper)?;
        frame.resize(bounds)?;
        let cloak = BOOL(1);
        DwmSetWindowAttribute(source, DWMWA_CLOAK, &cloak as *const _ as _, 4)?;
        if !SetWindowSubclass(
            preview,
            Some(procedure),
            SUBCLASS,
            &*frame as *const Frame as usize,
        )
        .as_bool()
        {
            return Err(windows::core::Error::from_thread());
        }
        if SetTimer(Some(preview), TIMER, 16, None) == 0 {
            return Err(windows::core::Error::from_thread());
        }
        frame.capture(Capture::Before);
        Ok(frame)
    }
    unsafe fn resize(&self, bounds: [i32; 4]) -> Result<()> {
        self.bounds.set(bounds);
        let mut origin = POINT {
            x: bounds[0],
            y: bounds[1],
        };
        windows::Win32::Graphics::Gdi::ClientToScreen(self.root, &mut origin).ok()?;
        SetWindowPos(
            self.preview,
            Some(HWND_TOP),
            origin.x,
            origin.y,
            bounds[2],
            bounds[3],
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )?;
        self.tick()
    }
    unsafe fn present(&self, sample: crate::resize_motion::Sample) -> Result<()> {
        let [_, _, width, height] = self.bounds.get();
        let size = [width, height];
        if trace_enabled() && !self.motion.borrow().dragging() {
            trace(
                serde_json::json!({"phase":"settling", "time":self.now(), "scale":sample.pose.scale,
                "depth":[sample.pose.x,sample.pose.y], "blend":sample.blend}),
            );
        }
        self.background_scale.SetScaleX2(width.max(1) as f32)?;
        self.background_scale.SetScaleY2(height.max(1) as f32)?;
        let time = if self.motion.borrow().reduced {
            0.
        } else {
            self.now()
        };
        self.background.present(width as f32, height as f32, time)?;
        rounded(
            &self.viewport_clip,
            width as f32,
            height as f32,
            self.corners,
        )?;
        // 18 CSS px while lifted. Morph back to each pane's real corners as
        // the final scale reaches 1:1, so the handoff never changes its outline.
        let lift = ((1. - sample.pose.scale) / 0.032).clamp(0., 1.);
        // Reuse the spring/ease-out lift: blur enters with the lifted snapshot,
        // remains through the layout crossfade, and clears as it returns 1:1.
        // Only this inner subtree is filtered; the outline and background stay sharp.
        self.blur
            .SetStandardDeviation2(3. * self.scale_factor * lift)?;
        rounded(
            &self.card_clip,
            width as f32,
            height as f32,
            self.corners
                .map(|r| r + (LIFTED_RADIUS * self.scale_factor - r) * lift),
        )?;
        self.old.fit(size)?;
        self.new.fit(size)?;
        for (row, values) in projection(sample.pose, width as f32, height as f32)
            .iter()
            .enumerate()
        {
            for (col, value) in values.iter().enumerate() {
                self.projection
                    .SetMatrixElement2(row as i32, col as i32, *value)?;
            }
        }
        // The new image is opaque underneath; fading ONLY the old layer avoids
        // the background leaking through (and flashing) halfway through a fade.
        self.old.effect.SetOpacity2(1. - sample.blend)?;
        self.device.Commit()
    }
    fn capture(&self, kind: Capture) {
        let Some(core) = &self.core else {
            return;
        };
        let sender = self.sender.clone();
        let request = (|| -> Result<()> {
            let stream = unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true)? };
            let output = stream.clone();
            let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
                let bytes = result.and_then(|_| crate::capture::bytes(&output));
                std::thread::spawn(move || {
                    let start = Instant::now();
                    let pixels = bytes.ok().and_then(decode);
                    trace(
                        serde_json::json!({"phase":"decoded", "after":kind == Capture::After,
                        "ms":start.elapsed().as_millis(), "size":pixels.as_ref().map(|p| [p.width,p.height])}),
                    );
                    let _ = sender.send((kind, pixels));
                });
                Ok(())
            }));
            unsafe {
                core.CapturePreview(
                    COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                    &stream,
                    &handler,
                )
            }
        })();
        if let Err(error) = request {
            eprintln!("Rove resize snapshot: {error}");
            let _ = self.sender.send((kind, None));
        }
    }
    unsafe fn tick(&self) -> Result<()> {
        if self.restored.get() {
            return Ok(());
        }
        let now = self.now();
        let dt = now - self.previous_tick.replace(now);
        for (kind, pixels) in self.pixels.try_iter() {
            match (kind, pixels) {
                (Capture::Before, Some(pixels)) if !self.paint_ready.get() => {
                    self.old.bitmap(&self.device, pixels)?;
                    self.frozen.set(true);
                    trace(serde_json::json!({"phase":"frozen", "size":self.old.size.get()}));
                }
                (Capture::After, Some(pixels))
                    if !self.motion.borrow().dragging() && !self.paint_ready.get() =>
                {
                    let [_, _, w, h] = self.bounds.get();
                    trace(
                        serde_json::json!({"phase":"capture-result", "actual":[pixels.width,pixels.height], "expected":[w,h]}),
                    );
                    if pixels.width as i32 == w && pixels.height as i32 == h {
                        self.new.bitmap(&self.device, pixels)?;
                        self.paint_ready.set(true);
                        self.motion.borrow_mut().paint_ready(now);
                        trace(serde_json::json!({"phase":"painted", "size":self.new.size.get()}));
                    }
                }
                _ => {}
            }
        }
        let elapsed = self.motion.borrow().elapsed(now);
        if elapsed.is_some_and(|t| t >= PAINT_DELAY) && !self.paint_requested.replace(true) {
            self.capture(Capture::After);
        }
        if elapsed.is_some_and(|t| t >= PAINT_LIMIT) && !self.paint_ready.replace(true) {
            self.new
                .visual
                .SetContent(&self.device.CreateSurfaceFromHwnd(self.source)?)?;
            let [_, _, w, h] = self.bounds.get();
            self.new.size.set([w, h]);
        }
        let sample = self.motion.borrow_mut().sample(now, dt);
        if sample.finished {
            self.restore();
            return Ok(());
        }
        self.present(sample)
    }
    unsafe fn restore(&self) {
        if self.restored.replace(true) {
            return;
        }
        trace(serde_json::json!({"phase":"restored", "bounds":self.bounds.get()}));
        let _ = KillTimer(Some(self.preview), TIMER);
        let cloak = BOOL(0);
        let _ = DwmSetWindowAttribute(self.source, DWMWA_CLOAK, &cloak as *const _ as _, 4);
        SetWindowLongPtrW(self.source, GWL_EXSTYLE, self.old_style);
        let _ = ShowWindow(self.preview, SW_HIDE);
        let _ = windows::Win32::Graphics::Gdi::RedrawWindow(
            Some(self.source),
            None,
            None,
            windows::Win32::Graphics::Gdi::RDW_INVALIDATE
                | windows::Win32::Graphics::Gdi::RDW_FRAME
                | windows::Win32::Graphics::Gdi::RDW_ALLCHILDREN,
        );
        let _ = RemoveWindowSubclass(self.preview, Some(procedure), SUBCLASS);
        let _ = DestroyWindow(self.preview);
    }
}
impl Drop for Frame {
    fn drop(&mut self) {
        unsafe {
            self.restore();
        }
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
    if msg == WM_TIMER && wparam.0 == TIMER {
        let frame = &*(data as *const Frame);
        if let Err(error) = frame.tick() {
            eprintln!("Rove resize animation: {error}");
            frame.restore();
        }
        return LRESULT(0);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
fn decode(png: Vec<u8>) -> Option<Pixels> {
    // WIC decodes AND premultiplies in native code. A debug Rust pixel loop on
    // a Retina-sized page could otherwise outlast the whole release animation.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok().ok()?;
        let result = (|| -> Option<Pixels> {
            let factory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
            let stream = SHCreateMemStream(Some(&png))?;
            let decoder = factory
                .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnLoad)
                .ok()?;
            let frame = decoder.GetFrame(0).ok()?;
            let (mut width, mut height) = (0u32, 0u32);
            frame.GetSize(&mut width, &mut height).ok()?;
            let length = width.checked_mul(height)?.checked_mul(4)?;
            if width == 0
                || height == 0
                || width > 16384
                || height > 16384
                || length > 128 * 1024 * 1024
            {
                return None;
            }
            let converter = factory.CreateFormatConverter().ok()?;
            converter
                .Initialize(
                    &frame,
                    &GUID_WICPixelFormat32bppPBGRA,
                    WICBitmapDitherTypeNone,
                    None::<&IWICPalette>,
                    0.,
                    WICBitmapPaletteTypeCustom,
                )
                .ok()?;
            let mut bgra = vec![0; length as usize];
            converter
                .CopyPixels(std::ptr::null(), width * 4, &mut bgra)
                .ok()?;
            Some(Pixels {
                width,
                height,
                bgra,
            })
        })();
        CoUninitialize();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::{
        Foundation::RECT,
        Graphics::Direct2D::{
            D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_CPU_READ,
            D2D1_BITMAP_OPTIONS_TARGET, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_MAP_OPTIONS_READ,
        },
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
        System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
    };
    #[test]
    fn texture_upload_replaces_stale_pixels_without_clearing_neighbours() -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            let device = composition_device()?;
            let seed = device.CreateSurface(
                1,
                1,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_ALPHA_MODE_PREMULTIPLIED,
            )?;
            let mut offset = POINT::default();
            let seed_dc: ID2D1DeviceContext = seed.BeginDraw(None, &mut offset)?;
            let dc = seed_dc
                .GetDevice()?
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            seed.EndDraw()?;
            let mut properties = D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.,
                dpiY: 96.,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                ..Default::default()
            };
            let size = D2D_SIZE_U {
                width: 160,
                height: 160,
            };
            let target = dc.CreateBitmap(size, None, 0, &properties)?;
            properties.bitmapOptions =
                D2D1_BITMAP_OPTIONS_CPU_READ | D2D1_BITMAP_OPTIONS_CANNOT_DRAW;
            let readback = dc.CreateBitmap(size, None, 0, &properties)?;
            dc.SetTarget(&target);
            dc.SetDpi(96., 96.);
            let field = radial([200, 80, 40]);
            // Seed the entire shared target with conspicuous old content.
            let stale = D2D1_COLOR_F {
                r: 1.,
                g: 0.,
                b: 1.,
                a: 1.,
            };
            let fill = D2D1_COLOR_F {
                r: 0.,
                g: 1.,
                b: 0.,
                a: 1.,
            };
            let offset = POINT { x: 13, y: 17 };
            for pixels in [Some(field.as_slice()), None] {
                dc.BeginDraw();
                dc.Clear(Some(&stale));
                paint_bitmap(
                    &dc,
                    offset,
                    GRADIENT_SIZE,
                    GRADIENT_SIZE,
                    pixels,
                    Some(fill),
                )?;
                dc.EndDraw(None, None)?;
                readback.CopyFromBitmap(None, &target, None)?;
                let mapped = readback.Map(D2D1_MAP_OPTIONS_READ)?;
                let mut actual = Vec::new();
                for y in 0..160 {
                    actual.extend_from_slice(std::slice::from_raw_parts(
                        mapped.bits.add(y * mapped.pitch as usize),
                        160 * 4,
                    ));
                }
                readback.Unmap()?;
                for y in 0..160 {
                    for x in 0..160 {
                        let expected = if (13..141).contains(&x) && (17..145).contains(&y) {
                            match pixels {
                                Some(data) => &data[((y - 17) * 128 + x - 13) * 4..][..4],
                                None => &[0, 255, 0, 255],
                            }
                        } else {
                            &[255, 0, 255, 255]
                        };
                        assert_eq!(
                            &actual[(y * 160 + x) * 4..][..4],
                            expected,
                            "GPU pixel at ({x}, {y}), bitmap={}",
                            pixels.is_some()
                        );
                    }
                }
            }
            drop(dc);
            CoUninitialize();
        }
        Ok(())
    }
    #[test]
    fn trusted_space_palette_deserializes_and_gradient_uses_its_direction() {
        let command: crate::model::Command = serde_json::from_str(r#"{"type":"resizeAppearance","start":[210,30,20],"end":[10,180,40],"angle":90,"dark":true,"reduceMotion":false}"#).unwrap();
        let crate::model::Command::ResizeAppearance { appearance } = command else {
            panic!("wrong command")
        };
        let pixels = gradient(appearance);
        assert_eq!(&pixels[..4], &[20, 30, 210, 255]);
        assert_eq!(
            &pixels[(GRADIENT_SIZE as usize - 1) * 4..GRADIENT_SIZE as usize * 4],
            &[40, 180, 10, 255]
        );
        assert_eq!(
            &pixels[..GRADIENT_SIZE as usize * 4],
            &pixels[(GRADIENT_SIZE as usize - 1) * GRADIENT_SIZE as usize * 4..]
        );
        // Transparent field edges must stay premultiplied, otherwise scaled
        // blobs introduce dark/coloured fringes where they overlap.
        let field = radial(appearance.start);
        assert_eq!(&field[..4], &[0, 0, 0, 0]);
        assert!(field
            .chunks_exact(4)
            .all(|p| p[..3].iter().all(|&c| c <= p[3])));
        let centre = ((GRADIENT_SIZE / 2 * GRADIENT_SIZE + GRADIENT_SIZE / 2) * 4) as usize;
        assert!(field[centre + 3] > 230);
    }
    #[test]
    fn compositor_keeps_renderer_geometry_and_always_restores_styles() -> Result<()> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            let root = CreateWindowExW(
                WS_EX_NOREDIRECTIONBITMAP,
                w!("STATIC"),
                w!("test"),
                WS_POPUP,
                0,
                0,
                640,
                480,
                None,
                None,
                None,
                None,
            )?;
            let source = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("source"),
                WS_CHILD,
                20,
                20,
                200,
                150,
                Some(root),
                None,
                None,
                None,
            )?;
            let style = GetWindowLongPtrW(source, GWL_EXSTYLE);
            let surface = Surface::new(source, None, Appearance::default());
            for _ in 0..3 {
                surface.begin([20, 20, 200, 150], [8.; 4], 1.)?;
                surface.resize([60, 20, 360, 150])?;
                let mut rect = RECT::default();
                GetClientRect(source, &mut rect)?;
                assert_eq!((rect.right, rect.bottom), (200, 150));
                let mut cloaked = 0u32;
                DwmGetWindowAttribute(source, DWMWA_CLOAKED, &mut cloaked as *mut _ as _, 4)?;
                assert_ne!(cloaked, 0);
                surface.release();
                // Even when a capture never completes, the timer must finish
                // the handoff and release every native visibility/style flag.
                for _ in 0..80 {
                    if !surface.active() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    surface.frame.borrow().as_ref().unwrap().tick()?;
                }
                assert!(!surface.active(), "missing capture cannot retain a preview");
                assert_eq!(GetWindowLongPtrW(source, GWL_EXSTYLE), style);
                DwmGetWindowAttribute(source, DWMWA_CLOAKED, &mut cloaked as *mut _ as _, 4)?;
                assert_eq!(cloaked, 0);
            }
            drop(surface);
            DestroyWindow(source)?;
            DestroyWindow(root)?;
            CoUninitialize();
        }
        Ok(())
    }
    #[test]
    fn png_is_decoded_as_bounded_premultiplied_bgra() {
        let source = image::RgbaImage::from_pixel(2, 3, image::Rgba([200, 100, 50, 128]));
        let mut png = std::io::Cursor::new(Vec::new());
        source.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let pixels = decode(png.into_inner()).unwrap();
        assert_eq!((pixels.width, pixels.height), (2, 3));
        assert_eq!(&pixels.bgra[..4], &[25, 50, 100, 128]);
        assert!(decode(b"invalid".to_vec()).is_none());
    }
}
