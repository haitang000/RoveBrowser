//! Native sidebar input. Drag only frozen compositor snapshots. Commit one
//! renderer size on release, then restore perspective/crossfade/scale to 1:1.
use crate::{layout::Viewport, model::Rect, split::Pane};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use windows::{
    core::{w, Error, Result},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Gdi::{BeginPaint, EndPaint, MapWindowPoints, PAINTSTRUCT},
        UI::{
            Input::KeyboardAndMouse::{GetCapture, GetFocus, ReleaseCapture, SetCapture, SetFocus},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
};
const SUBCLASS: usize = 0x53494445;
const FRAME_TIMER: usize = 1;

#[derive(Clone)]
pub struct Config {
    pub id: String,
    pub other: Option<String>,
    pub primary: Rect,
    pub secondary: Option<Rect>,
    pub handle: Rect,
    pub preferred: Option<f64>,
    pub split_ratio: f64,
    pub viewport: Viewport,
    pub scale: f64,
    pub left: Option<Pane>,
    pub right: Option<Pane>,
}
impl Config {
    pub fn width(&self) -> f64 {
        self.handle.x + self.handle.width
    }
    fn resized(&self, width: f64) -> Self {
        let width = width
            .round()
            .clamp(192., (self.viewport.width - 248.).clamp(192., 480.));
        let dx = width - self.width();
        let mut next = self.clone();
        next.handle.x += dx;
        next.primary.x += dx;
        if let Some(right) = self.secondary {
            let total = self.primary.width + right.width;
            let available = (total - dx).max(0.);
            let gap = right.x - self.primary.x - self.primary.width;
            next.primary.width = crate::layout::split_left_width(self.split_ratio, available);
            next.secondary = Some(Rect {
                x: next.primary.x + next.primary.width + gap,
                width: available - next.primary.width,
                ..right
            });
        } else {
            next.primary.width = (self.primary.width - dx).max(0.);
        }
        next
    }
}
pub struct Update {
    pub config: Config,
    pub resizing: bool,
    pub commit: bool,
    pub error: Option<String>,
}
struct State {
    root: HWND,
    popup: HWND,
    hwnd: Cell<HWND>,
    config: RefCell<Option<Config>>,
    original: RefCell<Option<Config>>,
    pending: RefCell<Option<Update>>,
    notify: Box<dyn Fn()>,
    present: Box<dyn Fn(f64, bool, bool)>,
    presented: Cell<Option<(f64, bool, bool)>>,
    dragging: Cell<bool>,
    busy: Cell<bool>,
    timer: Cell<bool>,
    dirty: Cell<bool>,
    grab_offset: Cell<f64>,
    previous_focus: Cell<HWND>,
}
pub struct SidebarHandle {
    state: Rc<State>,
}
impl SidebarHandle {
    pub fn new(
        root: HWND,
        popup: HWND,
        notify: impl Fn() + 'static,
        present: impl Fn(f64, bool, bool) + 'static,
    ) -> Result<Self> {
        let state = Rc::new(State {
            root,
            popup,
            hwnd: Cell::new(HWND::default()),
            config: RefCell::new(None),
            original: RefCell::new(None),
            pending: RefCell::new(None),
            notify: Box::new(notify),
            present: Box::new(present),
            presented: Cell::new(None),
            dragging: Cell::new(false),
            busy: Cell::new(false),
            timer: Cell::new(false),
            dirty: Cell::new(false),
            grab_offset: Cell::new(0.),
            previous_focus: Cell::new(HWND::default()),
        });
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
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
        state.hwnd.set(hwnd);
        if !unsafe {
            SetWindowSubclass(hwnd, Some(procedure), SUBCLASS, Rc::as_ptr(&state) as usize)
        }
        .as_bool()
        {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return Err(Error::from_thread());
        }
        Ok(Self { state })
    }
    pub fn dragging(&self) -> bool {
        self.state.dragging.get()
    }
    pub fn take_update(&self) -> Option<Update> {
        self.state.pending.borrow_mut().take()
    }
    pub fn configure(&self, config: Option<Config>) -> Result<()> {
        if self.dragging() {
            return Ok(());
        }
        *self.state.config.borrow_mut() = config.clone();
        if let Some(config) = config {
            place(&self.state, &config)
        } else {
            unsafe {
                let _ = ShowWindow(self.state.hwnd.get(), SW_HIDE);
            }
            Ok(())
        }
    }
    pub fn cancel(&self) {
        finish(&self.state, false);
    }
}
impl Drop for SidebarHandle {
    fn drop(&mut self) {
        self.state.dragging.set(false);
        unsafe {
            let _ = KillTimer(Some(self.state.hwnd.get()), FRAME_TIMER);
            if GetCapture() == self.state.hwnd.get() {
                let _ = ReleaseCapture();
            }
            let _ = RemoveWindowSubclass(self.state.hwnd.get(), Some(procedure), SUBCLASS);
            let _ = DestroyWindow(self.state.hwnd.get());
        }
    }
}
fn place(s: &State, c: &Config) -> Result<()> {
    unsafe {
        SetWindowPos(
            s.hwnd.get(),
            Some(s.popup),
            (c.handle.x * c.scale).round() as i32,
            (c.handle.y * c.scale).round() as i32,
            (c.handle.width * c.scale).round().max(1.) as i32,
            (c.handle.height * c.scale).round().max(1.) as i32,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    }
}
fn move_windows(s: &State, c: &Config) -> Result<()> {
    if let Some(pane) = &c.left {
        pane.move_window(c.primary, c.scale)?;
    }
    if let Some((pane, rect)) = c.right.as_ref().zip(c.secondary) {
        pane.move_window(rect, c.scale)?;
    }
    place(s, c)
}
fn reflow(c: &Config) -> Result<()> {
    if let Some(pane) = &c.left {
        pane.reflow()?;
    }
    if let Some(pane) = &c.right {
        pane.reflow()?;
    }
    Ok(())
}
fn present(s: &State, c: &Config, resizing: bool) {
    let value = (c.width(), resizing, c.preferred.is_none());
    if s.presented.replace(Some(value)) != Some(value) {
        (s.present)(value.0, value.1, value.2);
    }
}
fn report(s: &State, c: Config, resizing: bool, commit: bool, error: Option<String>) {
    present(s, &c, resizing);
    let previous = s.pending.borrow_mut().replace(Update {
        config: c,
        resizing,
        commit,
        error,
    });
    if previous.is_none() {
        (s.notify)();
    }
}
fn flush(s: &State) {
    if !s.dirty.replace(false) {
        return;
    }
    let config = s.config.borrow().clone();
    if let Some(c) = config {
        let error = reflow(&c).err().map(|e| e.to_string());
        report(s, c, true, false, error);
    }
}
fn cursor_x(s: &State, lparam: LPARAM, scale: f64) -> f64 {
    let mut point = [POINT {
        x: lparam.0 as u16 as i16 as i32,
        y: (lparam.0 >> 16) as u16 as i16 as i32,
    }];
    unsafe {
        MapWindowPoints(Some(s.hwnd.get()), Some(s.root), &mut point);
    }
    point[0].x as f64 / scale
}
fn move_to(s: &State, x: f64) {
    if s.busy.replace(true) {
        return;
    }
    let config = s.config.borrow().clone();
    if let Some(c) = config {
        let mut next = c.resized(x - s.grab_offset.get());
        if next.width() != c.width() {
            next.preferred = Some(next.width());
            *s.config.borrow_mut() = Some(next.clone());
            // Paint feedback directly from input, before compositor work. The
            // model/save notification still coalesces on the existing timer.
            present(s, &next, true);
            if let Err(error) = move_windows(s, &next) {
                report(s, next, true, false, Some(error.to_string()));
            }
            s.dirty.set(true);
            if !s.timer.get() {
                flush(s);
            }
        }
    }
    s.busy.set(false);
}
fn point_previews(s: &State, lparam: LPARAM) {
    let mut point = [POINT {
        x: lparam.0 as u16 as i16 as i32,
        y: (lparam.0 >> 16) as u16 as i16 as i32,
    }];
    unsafe {
        MapWindowPoints(Some(s.hwnd.get()), Some(s.root), &mut point);
    }
    if let Some(c) = s.config.borrow().as_ref() {
        for pane in [&c.left, &c.right].into_iter().flatten() {
            pane.surface.pointer(point[0].x as f32, point[0].y as f32);
        }
    }
}
fn finish(s: &State, cancelled: bool) {
    if !s.dragging.replace(false) {
        return;
    }
    unsafe {
        let _ = KillTimer(Some(s.hwnd.get()), FRAME_TIMER);
        if GetCapture() == s.hwnd.get() {
            let _ = ReleaseCapture();
        }
        if GetFocus() == s.hwnd.get() {
            let _ = SetFocus(Some(s.previous_focus.get()));
        }
    }
    s.timer.set(false);
    s.dirty.set(false);
    let original = s.original.borrow_mut().take();
    if cancelled {
        *s.config.borrow_mut() = original;
    }
    let config = s.config.borrow().clone();
    if let Some(c) = config {
        let restore = || -> Result<()> {
            let moved = move_windows(s, &c);
            // A failure on one controller must still release the other preview.
            let left = c
                .left
                .as_ref()
                .map(|pane| pane.end_preview(!cancelled))
                .transpose();
            let right = c
                .right
                .as_ref()
                .map(|pane| pane.end_preview(!cancelled))
                .transpose();
            moved?;
            left?;
            right?;
            reflow(&c)?;
            if let Some(pane) = &c.left {
                pane.clip.apply(pane.hwnd, c.primary.corners, c.scale)?;
            }
            if let Some((pane, rect)) = c.right.as_ref().zip(c.secondary) {
                pane.clip.apply(pane.hwnd, rect.corners, c.scale)?;
            }
            Ok(())
        };
        let error = restore().err().map(|e| e.to_string());
        report(s, c, false, true, error);
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
    // SidebarHandle owns this stable Rc until removing the subclass.
    let s = &*(data as *const State);
    match msg {
        WM_NCHITTEST => return LRESULT(HTCLIENT as isize),
        WM_SETCURSOR => {
            SetCursor(LoadCursorW(None, IDC_SIZEWE).ok());
            return LRESULT(1);
        }
        WM_ERASEBKGND => return LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut paint);
            let _ = EndPaint(hwnd, &paint);
            return LRESULT(0);
        }
        WM_LBUTTONDOWN => {
            let config = s.config.borrow().clone();
            if let Some(c) = config {
                *s.original.borrow_mut() = Some(c.clone());
                s.grab_offset.set(cursor_x(s, lparam, c.scale) - c.width());
                s.dragging.set(true);
                s.previous_focus.set(GetFocus());
                let _ = SetFocus(Some(hwnd));
                SetCapture(hwnd);
                // Capture and show the resizing state before creating GPU
                // devices or requesting either page's asynchronous snapshot.
                report(s, c.clone(), true, false, None);
                if let Some(pane) = &c.left {
                    pane.begin_preview(c.primary, c.scale);
                }
                if let Some((pane, rect)) = c.right.as_ref().zip(c.secondary) {
                    pane.begin_preview(rect, c.scale);
                }
                point_previews(s, lparam);
                let error = move_windows(s, &c).err().map(|e| e.to_string());
                s.timer
                    .set(SetTimer(Some(hwnd), FRAME_TIMER, 16, None) != 0);
                report(s, c, true, false, error);
            }
            return LRESULT(0);
        }
        WM_MOUSEMOVE if s.dragging.get() => {
            point_previews(s, lparam);
            let scale = s.config.borrow().as_ref().map(|c| c.scale);
            if let Some(scale) = scale {
                move_to(s, cursor_x(s, lparam, scale));
            }
            return LRESULT(0);
        }
        WM_TIMER if wparam.0 == FRAME_TIMER && s.dragging.get() => {
            flush(s);
            return LRESULT(0);
        }
        WM_KEYDOWN if wparam.0 == 27 && s.dragging.get() => {
            finish(s, true);
            return LRESULT(0);
        }
        WM_LBUTTONUP => {
            let scale = s.config.borrow().as_ref().map(|c| c.scale);
            if s.dragging.get() {
                if let Some(scale) = scale {
                    move_to(s, cursor_x(s, lparam, scale));
                }
            }
            finish(s, false);
            return LRESULT(0);
        }
        WM_CAPTURECHANGED | WM_CANCELMODE | WM_KILLFOCUS => {
            finish(s, false);
            return LRESULT(0);
        }
        WM_LBUTTONDBLCLK => {
            finish(s, false);
            let config = s.config.borrow().clone();
            if let Some(c) = config {
                let mut next = c.resized(if c.viewport.width <= 740. { 216. } else { 240. });
                next.preferred = None;
                *s.config.borrow_mut() = Some(next.clone());
                let error = move_windows(s, &next)
                    .and_then(|_| reflow(&next))
                    .err()
                    .map(|e| e.to_string());
                report(s, next, false, true, error);
            }
            return LRESULT(0);
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Window(HWND);
    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    fn window(parent: Option<HWND>) -> Window {
        Window(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("sidebar test"),
                if parent.is_some() { WS_CHILD } else { WS_POPUP },
                0,
                0,
                1920,
                1230,
                parent,
                None,
                None,
                None,
            )
            .unwrap()
        })
    }
    fn config(scale: f64) -> Config {
        Config {
            id: "left".into(),
            other: Some("right".into()),
            scale,
            preferred: None,
            split_ratio: 0.5,
            viewport: Viewport {
                width: 1280.,
                height: 820.,
            },
            handle: Rect {
                x: 234.,
                y: 38.,
                width: 6.,
                height: 782.,
                ..Rect::default()
            },
            primary: Rect {
                x: 241.,
                y: 69.,
                width: 511.,
                height: 742.,
                ..Rect::default()
            },
            secondary: Some(Rect {
                x: 760.,
                y: 69.,
                width: 511.,
                height: 742.,
                ..Rect::default()
            }),
            left: None,
            right: None,
        }
    }
    fn pointer(handle: &SidebarHandle, x: f64) -> LPARAM {
        let scale = handle.state.config.borrow().as_ref().unwrap().scale;
        let mut point = [POINT {
            x: (x * scale).round() as i32,
            y: (150. * scale).round() as i32,
        }];
        unsafe {
            MapWindowPoints(
                Some(handle.state.root),
                Some(handle.state.hwnd.get()),
                &mut point,
            );
        }
        LPARAM(((point[0].y as u16 as usize) << 16 | point[0].x as u16 as usize) as isize)
    }
    fn send(handle: &SidebarHandle, message: u32, x: f64) {
        unsafe {
            SendMessageW(
                handle.state.hwnd.get(),
                message,
                None,
                Some(pointer(handle, x)),
            );
        }
    }

    #[test]
    fn native_input_moves_immediately_and_batches_renderer_feedback_before_commit() {
        let root = window(None);
        let popup = window(Some(root.0));
        let reports = Rc::new(Cell::new(0));
        let counter = reports.clone();
        let painted = Rc::new(RefCell::new(Vec::new()));
        let paints = painted.clone();
        let handle = SidebarHandle::new(
            root.0,
            popup.0,
            move || counter.set(counter.get() + 1),
            move |width, resizing, reset| paints.borrow_mut().push((width, resizing, reset)),
        )
        .unwrap();
        handle.configure(Some(config(1.5))).unwrap();
        assert_ne!(
            unsafe { GetClassLongPtrW(handle.state.hwnd.get(), GCL_STYLE) } & CS_DBLCLKS.0 as usize,
            0
        );
        send(&handle, WM_LBUTTONDOWN, 235.);
        assert!(handle.dragging());
        assert_eq!(unsafe { GetCapture() }, handle.state.hwnd.get());
        assert!(handle.take_update().unwrap().resizing);
        let start_reports = reports.get();
        painted.borrow_mut().clear();
        for x in 236..=435 {
            send(&handle, WM_MOUSEMOVE, x as f64);
        }
        // Neither shell JS nor an app-event consumer ran between these inputs.
        let current = handle.state.config.borrow().clone().unwrap();
        assert_eq!(current.width(), 440.);
        assert_eq!(
            painted.borrow().len(),
            200,
            "visual feedback cannot wait for a timer or app event"
        );
        assert_eq!(painted.borrow().last().copied(), Some((440., true, false)));
        assert_eq!(current.primary.x, 441.);
        assert_eq!(
            current.primary.width + current.secondary.unwrap().width,
            822.
        );
        assert_eq!(
            reports.get(),
            start_reports,
            "high-rate mouse input must not enqueue a renderer reflow or shell event per pixel"
        );
        unsafe {
            SendMessageW(
                handle.state.hwnd.get(),
                WM_TIMER,
                Some(WPARAM(FRAME_TIMER)),
                None,
            );
        }
        let update = handle.take_update().unwrap();
        assert_eq!(update.config.width(), 440.);
        assert!(!update.commit);
        assert_eq!(reports.get(), start_reports + 1);
        // Release can carry a newer cursor position than the last mousemove.
        send(&handle, WM_LBUTTONUP, 455.);
        let final_update = handle.take_update().unwrap();
        assert!(!handle.dragging());
        assert!(!final_update.resizing);
        assert!(final_update.commit);
        assert_eq!(final_update.config.preferred, Some(460.));
        assert_eq!(final_update.config.width(), 460.);
        assert_eq!(painted.borrow().last().copied(), Some((460., false, false)));
        assert_ne!(unsafe { GetCapture() }, handle.state.hwnd.get());
    }

    #[test]
    fn escape_restores_the_start_and_double_click_resets_the_preference() {
        let root = window(None);
        let popup = window(Some(root.0));
        let handle = SidebarHandle::new(root.0, popup.0, || {}, |_, _, _| {}).unwrap();
        let mut c = config(1.);
        c.preferred = Some(336.);
        c = c.resized(336.);
        handle.configure(Some(c)).unwrap();
        send(&handle, WM_LBUTTONDOWN, 335.);
        send(&handle, WM_MOUSEMOVE, 455.);
        unsafe {
            SendMessageW(handle.state.hwnd.get(), WM_KEYDOWN, Some(WPARAM(27)), None);
        }
        let update = handle.take_update().unwrap();
        assert!(!update.resizing);
        assert_eq!(update.config.width(), 336.);
        assert_eq!(update.config.preferred, Some(336.));
        send(&handle, WM_LBUTTONDBLCLK, 335.);
        let update = handle.take_update().unwrap();
        assert!(update.commit);
        assert_eq!(update.config.width(), 240.);
        assert_eq!(update.config.preferred, None);
        handle.configure(None).unwrap();
        assert!(!unsafe { IsWindowVisible(handle.state.hwnd.get()) }.as_bool());
    }

    #[test]
    fn clamps_to_narrow_windows_and_keeps_the_split_seam_and_right_edge() {
        let mut c = config(1.);
        c.viewport.width = 480.;
        let next = c.resized(2000.);
        assert_eq!(next.width(), 232.);
        let right = next.secondary.unwrap();
        assert_eq!(right.x - next.primary.x - next.primary.width, 8.);
        assert_eq!(right.x + right.width, 1271.);
        assert_eq!(next.resized(-2000.).width(), 192.);
    }
}
