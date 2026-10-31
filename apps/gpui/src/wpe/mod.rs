//! WPE WebKit, which paints a message's page off the screen, on the CPU, into memory the app
//! draws from. WebKit runs on a thread of its own, in GLib's default loop; a page's calls are
//! posted there in order, and what it paints comes back in bands of rows.

use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_void};
use std::sync::{Arc, Mutex, Once};

use gpui::{CursorStyle, RenderImage};

/// The rows of a page in one of its pictures: a change redraws only the bands it touches.
pub const BAND: u32 = 256;

/// What a page tells the app.
pub enum Said {
    Height(f32),
    Frame(Frame),
    Cursor(CursorStyle),
    Answer(u64, String),
    Failed,
}

/// A picture of the part of the page WebKit holds, `top` down from the page's top, with the
/// bands that changed since the last; on a new size, every band.
pub struct Frame {
    pub top: f32,
    pub width: u32,
    pub height: u32,
    pub bands: Vec<(usize, Arc<RenderImage>)>,
}

#[derive(Clone, Copy)]
pub enum Pointer {
    Move = 0,
    Down = 1,
    Up = 2,
    Leave = 3,
}

pub const MODIFIER_CONTROL: u32 = 1 << 0;
pub const MODIFIER_SHIFT: u32 = 1 << 1;
pub const MODIFIER_ALT: u32 = 1 << 2;
pub const MODIFIER_META: u32 = 1 << 3;
pub const MODIFIER_BUTTON1: u32 = 1 << 8;

#[repr(C)]
struct WonnetPage {
    _private: [u8; 0],
}

#[repr(C)]
struct Rect {
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
}

#[repr(C)]
struct Callbacks {
    frame: unsafe extern "C" fn(*mut c_void, *const u8, c_uint, c_int, c_int, *const Rect, c_uint),
    said: unsafe extern "C" fn(*mut c_void, *const c_char),
    open: unsafe extern "C" fn(*mut c_void, *const c_char),
    cursor: unsafe extern "C" fn(*mut c_void, *const c_char),
    answer: unsafe extern "C" fn(*mut c_void, u64, *const c_char),
    failed: unsafe extern "C" fn(*mut c_void),
}

unsafe extern "C" {
    fn wonnet_wpe_run(callbacks: *const Callbacks);
    fn wonnet_page_new(
        sink: *mut c_void,
        script: *const c_char,
        width: c_int,
        height: c_int,
        scale: f64,
    ) -> *mut WonnetPage;
    fn wonnet_page_free(page: *mut WonnetPage);
    fn wonnet_page_load(page: *mut WonnetPage, html: *const c_char, red: f64, green: f64, blue: f64);
    fn wonnet_page_resize(page: *mut WonnetPage, width: c_int, height: c_int, scale: f64);
    fn wonnet_page_run(page: *mut WonnetPage, script: *const c_char, question: u64);
    fn wonnet_page_pointer(
        page: *mut WonnetPage,
        kind: c_int,
        button: c_uint,
        x: f64,
        y: f64,
        modifiers: c_uint,
        count: c_uint,
    );
    fn g_idle_add(function: unsafe extern "C" fn(*mut c_void) -> c_int, data: *mut c_void) -> c_uint;
}

static CALLBACKS: Callbacks = Callbacks { frame, said, open, cursor, answer, failed };

type Job = Box<dyn FnOnce() + Send>;

static JOBS: Mutex<Vec<Job>> = Mutex::new(Vec::new());

/// Runs `job` on WebKit's thread, after the jobs posted before it.
fn post(job: impl FnOnce() + Send + 'static) {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        let started = std::thread::Builder::new().name("wpe".into()).spawn(|| unsafe { wonnet_wpe_run(&CALLBACKS) });
        if let Err(error) = started {
            eprintln!("WebKit's thread didn't start: {error}");
        }
    });
    let mut jobs = JOBS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let idle = jobs.is_empty();
    jobs.push(Box::new(job));
    drop(jobs);
    if idle {
        unsafe { g_idle_add(run_jobs, std::ptr::null_mut()) };
    }
}

unsafe extern "C" fn run_jobs(_: *mut c_void) -> c_int {
    let jobs = std::mem::take(&mut *JOBS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()));
    for job in jobs {
        job();
    }
    0
}

/// A page's state on WebKit's thread, which alone reads and writes it.
struct Sink {
    page: *mut WonnetPage,
    said: async_channel::Sender<Said>,
    size: (u32, u32),
    top: f32,
}

#[derive(Clone, Copy)]
struct Shared(*mut Sink);

// SAFETY: the pointer is only followed on WebKit's thread, by jobs posted in order.
unsafe impl Send for Shared {}

impl Shared {
    fn with(self, job: impl FnOnce(&mut Sink) + Send + 'static) {
        post(move || {
            let shared = self;
            job(unsafe { &mut *shared.0 })
        });
    }
}

/// A message's page in WebKit. Dropped, it goes with its web view.
pub struct Page {
    sink: Shared,
}

impl Page {
    pub fn new(script: &str, width: i32, height: i32, scale: f32, said: async_channel::Sender<Said>) -> Self {
        let sink = Shared(Box::into_raw(Box::new(Sink { page: std::ptr::null_mut(), said, size: (0, 0), top: 0. })));
        let script = text(script);
        sink.with(move |state| {
            let raw = state as *mut Sink;
            state.page = unsafe { wonnet_page_new(raw.cast(), script.as_ptr(), width, height, scale as f64) };
        });
        Self { sink }
    }

    pub fn load(&self, html: &str, background: gpui::Rgba) {
        let html = text(html);
        self.sink.with(move |state| unsafe {
            wonnet_page_load(state.page, html.as_ptr(), background.r as f64, background.g as f64, background.b as f64)
        });
    }

    pub fn resize(&self, width: i32, height: i32, scale: f32) {
        self.sink.with(move |state| unsafe { wonnet_page_resize(state.page, width, height, scale as f64) });
    }

    /// Runs `script` in the page; its value comes back as `Said::Answer` when `question` isn't 0.
    pub fn run(&self, script: &str, question: u64) {
        let script = text(script);
        self.sink.with(move |state| unsafe { wonnet_page_run(state.page, script.as_ptr(), question) });
    }

    /// The pointer at `x`, `y` in the page's part WebKit holds.
    pub fn pointer(&self, kind: Pointer, button: u32, x: f32, y: f32, modifiers: u32, count: u32) {
        self.sink.with(move |state| unsafe {
            wonnet_page_pointer(state.page, kind as c_int, button, x as f64, y as f64, modifiers, count)
        });
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        let sink = self.sink;
        post(move || {
            let shared = sink;
            let state = unsafe { Box::from_raw(shared.0) };
            unsafe { wonnet_page_free(state.page) };
        });
    }
}

fn text(value: &str) -> CString {
    CString::new(value.replace('\0', "")).unwrap_or_default()
}

unsafe fn sink<'a>(raw: *mut c_void) -> &'a mut Sink {
    unsafe { &mut *raw.cast::<Sink>() }
}

unsafe fn string(raw: *const c_char) -> String {
    unsafe { CStr::from_ptr(raw) }.to_string_lossy().into_owned()
}

unsafe extern "C" fn frame(
    raw: *mut c_void,
    pixels: *const u8,
    stride: c_uint,
    width: c_int,
    height: c_int,
    damage: *const Rect,
    damaged: c_uint,
) {
    let sink = unsafe { sink(raw) };
    let (width, height, stride) = (width.max(0) as u32, height.max(0) as u32, stride as usize);
    let pixels = unsafe { std::slice::from_raw_parts(pixels, stride * height as usize) };
    let damage =
        if damage.is_null() { &[][..] } else { unsafe { std::slice::from_raw_parts(damage, damaged as usize) } };
    let resized = sink.size != (width, height);
    sink.size = (width, height);
    let bands = touched(height, damage, resized)
        .iter()
        .enumerate()
        .filter(|(_, touched)| **touched)
        .filter_map(|(index, _)| band(pixels, stride, width, height, index).map(|image| (index, image)))
        .collect();
    let _ = sink.said.try_send(Said::Frame(Frame { top: sink.top, width, height, bands }));
}

/// Which bands of a picture `height` rows tall the damage touches: all of them when there's
/// none, or the picture has a new size.
fn touched(height: u32, damage: &[Rect], resized: bool) -> Vec<bool> {
    let mut touched = vec![resized || damage.is_empty(); height.div_ceil(BAND) as usize];
    for rect in damage {
        let first = rect.y.clamp(0, height as c_int) as u32 / BAND;
        let end = (rect.y + rect.height).clamp(0, height as c_int) as u32;
        for band in first..end.div_ceil(BAND) {
            touched[band as usize] = true;
        }
    }
    touched
}

/// The rows of band `index`, as GPUI draws them: blue, green, red and alpha, the page opaque.
fn band(pixels: &[u8], stride: usize, width: u32, height: u32, index: usize) -> Option<Arc<RenderImage>> {
    let top = index as u32 * BAND;
    let rows = BAND.min(height.checked_sub(top)?);
    let row = width as usize * 4;
    let mut data = Vec::with_capacity(row * rows as usize);
    for y in top..top + rows {
        let start = y as usize * stride;
        data.extend_from_slice(pixels.get(start..start + row)?);
    }
    let image = image::RgbaImage::from_raw(width, rows, data)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(image)])))
}

unsafe extern "C" fn said(raw: *mut c_void, message: *const c_char) {
    let sink = unsafe { sink(raw) };
    let message = unsafe { string(message) };
    if let Some(top) = message.strip_prefix("scrolled:").and_then(|top| top.parse().ok()) {
        sink.top = top;
        return;
    }
    if let Some(height) = message.strip_prefix("height:").and_then(|height| height.parse().ok()) {
        let _ = sink.said.try_send(Said::Height(height));
    }
}

unsafe extern "C" fn open(_: *mut c_void, uri: *const c_char) {
    let uri = unsafe { string(uri) };
    let _ = crate::platform::open(&uri);
}

unsafe extern "C" fn cursor(raw: *mut c_void, name: *const c_char) {
    let sink = unsafe { sink(raw) };
    let style = match unsafe { CStr::from_ptr(name) }.to_bytes() {
        b"pointer" => CursorStyle::PointingHand,
        b"text" => CursorStyle::IBeam,
        b"vertical-text" => CursorStyle::IBeamCursorForVerticalLayout,
        b"crosshair" => CursorStyle::Crosshair,
        b"not-allowed" | b"no-drop" => CursorStyle::OperationNotAllowed,
        b"grab" => CursorStyle::OpenHand,
        b"grabbing" => CursorStyle::ClosedHand,
        b"ew-resize" | b"col-resize" => CursorStyle::ResizeLeftRight,
        b"ns-resize" | b"row-resize" => CursorStyle::ResizeUpDown,
        _ => CursorStyle::Arrow,
    };
    let _ = sink.said.try_send(Said::Cursor(style));
}

unsafe extern "C" fn answer(raw: *mut c_void, question: u64, value: *const c_char) {
    let sink = unsafe { sink(raw) };
    let value = unsafe { string(value) };
    let _ = sink.said.try_send(Said::Answer(question, value));
}

unsafe extern "C" fn failed(raw: *mut c_void) {
    let sink = unsafe { sink(raw) };
    let _ = sink.said.try_send(Said::Failed);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(y: c_int, height: c_int) -> Rect {
        Rect { x: 0, y, width: 10, height }
    }

    #[test]
    fn damage_touches_the_bands_it_crosses() {
        assert_eq!(touched(600, &[rect(10, 20)], false), [true, false, false]);
        assert_eq!(touched(600, &[rect(250, 10)], false), [true, true, false]);
        assert_eq!(touched(600, &[rect(512, 400)], false), [false, false, true]);
        assert_eq!(touched(512, &[rect(600, 10)], false), [false, false]);
    }

    #[test]
    fn a_new_size_or_no_damage_touches_every_band() {
        assert_eq!(touched(600, &[], false), [true, true, true]);
        assert_eq!(touched(600, &[rect(10, 20)], true), [true, true, true]);
    }

    #[test]
    fn a_band_leaves_out_the_rows_padding() {
        let (width, height, stride) = (2, 300, 12);
        let pixels: Vec<u8> = (0..stride * height).map(|byte| (byte % 251) as u8).collect();
        let last = band(&pixels, stride, width as u32, height as u32, 1).unwrap();
        let size = last.size(0);
        assert_eq!((size.width.0, size.height.0), (2, 300 - BAND as i32));
        let first_row = &pixels[BAND as usize * stride..BAND as usize * stride + width * 4];
        assert_eq!(&last.as_bytes(0).unwrap()[..width * 4], first_row);
    }
}
