//! [078] Kernel EventBuffer — unified input event queue.
//!
//! A fixed-size ring buffer that collects keyboard and mouse events
//! from their respective IRQ handlers.  User processes consume events
//! via `sys_read_event` ([079]).
//!
//! Events are stored as compact 16-byte structs so the buffer can
//! hold a generous number of events without excessive memory use.

use spin::Mutex;

// ── Event types ─────────────────────────────────────────────────

/// Discriminant for the kind of input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EventKind {
	/// A key was pressed.
	KeyPress = 1,
	/// A key was released.
	KeyRelease = 2,
	/// The mouse moved and/or a button state changed.
	Mouse = 3,
}

/// A unified input event.
///
/// Layout (16 bytes, repr(C)):
///   kind:	u8   — EventKind discriminant
///   _pad:	[u8; 3]
///   data:	[u8; 12] — payload interpreted per kind
///
/// Keyboard payload (KeyPress / KeyRelease):
///   data[0]	 = scancode
///   data[1..5]  = Unicode char as u32 (0 if non-printable)
///
/// Mouse payload:
///   data[0..2]  = dx as i16 (little-endian)
///   data[2..4]  = dy as i16 (little-endian)
///   data[4]	 = button state (BTN_LEFT | BTN_RIGHT | BTN_MIDDLE)
///   data[5..9]  = x as i32 (absolute cursor x, little-endian)
///   data[9..13] is unused (padding) — actually data is only 12 bytes
///			   so data[5..9] = abs_x, data[9..12] are the high bytes
///			   of abs_y.  Let's simplify:
///
/// Actually, let's keep it simpler:
///   Mouse payload in data[0..12]:
///	 [0..2] = dx  (i16 LE)
///	 [2..4] = dy  (i16 LE)
///	 [4]	= buttons
///	 [5..9] = abs_x (i32 LE)
///	 [9..12] = first 3 bytes of abs_y — nah, let's just use 8 bytes:
///
/// Simplified layout using named fields instead:
#[derive(Clone, Copy)]
#[repr(C)]
pub struct InputEvent {
	/// Event kind discriminant.
	pub kind: u8,
	/// Scancode (keyboard) or button state (mouse).
	pub code: u8,
	/// Padding / flags.
	pub flags: u8,
	/// Reserved.
	pub _pad: u8,
	/// For keyboard: Unicode codepoint (0 if non-printable).
	/// For mouse: dx as i16 in low 16 bits, dy as i16 in high 16 bits.
	pub value: u32,
	/// For mouse: absolute X position.
	pub abs_x: i16,
	/// For mouse: absolute Y position.
	pub abs_y: i16,
}

// Compile-time size check.
const _: () = assert!(core::mem::size_of::<InputEvent>() == 12);

impl InputEvent {
	/// Create a keyboard event.
	pub fn key(press: bool, scancode: u8, ch: u32) -> Self {
		Self {
			kind: if press { EventKind::KeyPress as u8 } else { EventKind::KeyRelease as u8 },
			code: scancode,
			flags: 0,
			_pad: 0,
			value: ch,
			abs_x: 0,
			abs_y: 0,
		}
	}

	/// Create a mouse event.
	pub fn mouse(dx: i16, dy: i16, buttons: u8, abs_x: i16, abs_y: i16) -> Self {
		let value = (dx as u16 as u32) | ((dy as u16 as u32) << 16);
		Self {
			kind: EventKind::Mouse as u8,
			code: buttons,
			flags: 0,
			_pad: 0,
			value,
			abs_x,
			abs_y,
		}
	}
}

// ── Ring buffer ─────────────────────────────────────────────────

const EVENT_BUF_SIZE: usize = 256;

struct EventRing {
	buf: [InputEvent; EVENT_BUF_SIZE],
	read: usize,
	write: usize,
	count: usize,
}

impl EventRing {
	const fn new() -> Self {
		let zero = InputEvent {
			kind: 0, code: 0, flags: 0, _pad: 0, value: 0, abs_x: 0, abs_y: 0,
		};
		Self {
			buf: [zero; EVENT_BUF_SIZE],
			read: 0,
			write: 0,
			count: 0,
		}
	}

	fn push(&mut self, event: InputEvent) {
		if self.count >= EVENT_BUF_SIZE {
			return; // full — drop oldest would be better, but simple drop is fine
		}
		self.buf[self.write] = event;
		self.write = (self.write + 1) % EVENT_BUF_SIZE;
		self.count += 1;
	}

	fn pop(&mut self) -> Option<InputEvent> {
		if self.count == 0 {
			return None;
		}
		let event = self.buf[self.read];
		self.read = (self.read + 1) % EVENT_BUF_SIZE;
		self.count -= 1;
		Some(event)
	}

	#[allow(dead_code)]
	fn len(&self) -> usize {
		self.count
	}
}

static EVENTS: Mutex<EventRing> = Mutex::new(EventRing::new());

/// Push a keyboard event into the event buffer.
///
/// Called from the keyboard IRQ handler.
pub fn push_key(press: bool, scancode: u8, ch: u32) {
	EVENTS.lock().push(InputEvent::key(press, scancode, ch));
}

/// Push a mouse event into the event buffer.
///
/// Called from the mouse IRQ handler.
pub fn push_mouse(dx: i16, dy: i16, buttons: u8, abs_x: i16, abs_y: i16) {
	EVENTS.lock().push(InputEvent::mouse(dx, dy, buttons, abs_x, abs_y));
}

/// Pop the next event from the buffer.
///
/// Returns `None` if no events are available (non-blocking).
pub fn pop_event() -> Option<InputEvent> {
	EVENTS.lock().pop()
}

/// Copy the next event into a user-space buffer.
///
/// Returns the size of the event (12 bytes) on success, or 0 if no
/// event is available.
///
/// # Safety
/// `buf_ptr` must be a valid pointer to at least 12 bytes of writable
/// memory in the calling process's address space.
pub unsafe fn read_event_to_user(buf_ptr: *mut u8) -> usize {
	if let Some(event) = pop_event() {
		let src = &event as *const InputEvent as *const u8;
		unsafe {
			core::ptr::copy_nonoverlapping(src, buf_ptr, 12);
		}
		12
	} else {
		0
	}
}
