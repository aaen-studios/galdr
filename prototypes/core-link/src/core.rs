//! The Calx core, linked in as a static library (see build.rs and core/galdr.calx).
//!
//! Requests go in as JSON; events come back as JSON on the core's own thread, and are passed on
//! through a channel. A panic in the core doesn't take the app down: the hook reports it, and
//! `restart` starts the library again from nothing.

use std::ffi::{CStr, c_char, c_void};
use std::sync::mpsc::Sender;

#[repr(C)]
struct CalxPanic {
    message: *const c_char,
    location: *const c_char,
    trace: *const c_char,
    function: *const c_char,
}

unsafe extern "C" {
    fn calx_init();
    fn calx_shutdown() -> i32;
    fn calx_on_panic(hook: Option<extern "C" fn(*mut c_void, *const CalxPanic)>, ctx: *mut c_void);
    fn galdr_core_start(on_event: extern "C" fn(*mut u8, *const u8, usize), ctx: *mut u8) -> *mut u8;
    fn galdr_core_send(core: *mut u8, request: *const u8, len: usize) -> i32;
    fn galdr_core_stop(core: *mut u8);
}

#[derive(Debug)]
pub enum Message {
    Event(serde_json::Value),
    Panic { message: String, location: String, function: Option<String>, trace: String },
}

pub struct Core {
    ptr: *mut u8,
    /// The channel the callbacks send on. Leaked on purpose: after a panic the old library's
    /// threads may still hold the pointer, so it has to outlive every library start.
    ctx: *mut Sender<Message>,
    alive: bool,
}

extern "C" fn on_event(ctx: *mut u8, text: *const u8, len: usize) {
    let tx = unsafe { &*(ctx as *const Sender<Message>) };
    let bytes = unsafe { std::slice::from_raw_parts(text, len) };
    let message = match serde_json::from_slice(bytes) {
        Ok(v) => Message::Event(v),
        Err(e) => Message::Event(serde_json::json!({ "event": "bad-event", "error": e.to_string() })),
    };
    let _ = tx.send(message);
}

extern "C" fn on_panic(ctx: *mut c_void, p: *const CalxPanic) {
    let tx = unsafe { &*(ctx as *const Sender<Message>) };
    let text = |s: *const c_char| {
        if s.is_null() { None } else { Some(unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()) }
    };
    let p = unsafe { &*p };
    let _ = tx.send(Message::Panic {
        message: text(p.message).unwrap_or_default(),
        location: text(p.location).unwrap_or_default(),
        function: text(p.function),
        trace: text(p.trace).unwrap_or_default(),
    });
}

impl Core {
    pub fn start(events: Sender<Message>) -> Self {
        let ctx = Box::into_raw(Box::new(events));
        unsafe {
            calx_on_panic(Some(on_panic), ctx.cast());
            calx_init();
            let ptr = galdr_core_start(on_event, ctx.cast());
            Self { ptr, ctx, alive: true }
        }
    }

    /// Sends a request. False if the core refused it or has stopped.
    pub fn send(&self, request: &serde_json::Value) -> bool {
        if !self.alive {
            return false;
        }
        let text = request.to_string();
        unsafe { galdr_core_send(self.ptr, text.as_ptr(), text.len()) == 0 }
    }

    /// After a panic: the library refuses every call until it starts again.
    pub fn mark_dead(&mut self) {
        self.alive = false;
    }

    pub fn alive(&self) -> bool {
        self.alive
    }

    /// Starts the library again from nothing. Gives what `calx_shutdown` said: 101 after a panic.
    pub fn restart(&mut self) -> i32 {
        unsafe {
            galdr_core_stop(self.ptr); // returns at once if the library has stopped
            let code = calx_shutdown();
            calx_init();
            self.ptr = galdr_core_start(on_event, self.ctx.cast());
            self.alive = true;
            code
        }
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        unsafe {
            galdr_core_stop(self.ptr);
            calx_shutdown();
        }
    }
}
