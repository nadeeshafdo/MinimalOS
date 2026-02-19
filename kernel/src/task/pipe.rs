//! IPC Pipes — [070].
//!
//! Fixed-size kernel ring buffers for inter-process communication.
//! One process writes, another reads.  Non-blocking — reads and
//! writes return immediately with whatever could be transferred.

use spin::Mutex;

/// Size of each pipe's internal buffer (4 KiB).
const PIPE_BUF_SIZE: usize = 4096;

/// Maximum number of simultaneously open pipes.
const MAX_PIPES: usize = 16;

/// A single IPC pipe backed by a ring buffer.
#[derive(Clone, Copy)]
struct Pipe {
	buf: [u8; PIPE_BUF_SIZE],
	read_pos: usize,
	write_pos: usize,
	count: usize,
}

impl Pipe {
	/// Create a new, empty pipe.
	const fn new() -> Self {
		Self {
			buf: [0; PIPE_BUF_SIZE],
			read_pos: 0,
			write_pos: 0,
			count: 0,
		}
	}

	/// Write `data` into the pipe.  Returns the number of bytes written
	/// (may be less than `data.len()` if the buffer fills up).
	fn write(&mut self, data: &[u8]) -> usize {
		let mut written = 0;
		for &byte in data {
			if self.count >= PIPE_BUF_SIZE {
				break;
			}
			self.buf[self.write_pos] = byte;
			self.write_pos = (self.write_pos + 1) % PIPE_BUF_SIZE;
			self.count += 1;
			written += 1;
		}
		written
	}

	/// Read up to `buf.len()` bytes from the pipe.  Returns the number
	/// of bytes actually read (0 if the pipe is empty).
	fn read(&mut self, buf: &mut [u8]) -> usize {
		let mut nread = 0;
		for slot in buf.iter_mut() {
			if self.count == 0 {
				break;
			}
			*slot = self.buf[self.read_pos];
			self.read_pos = (self.read_pos + 1) % PIPE_BUF_SIZE;
			self.count -= 1;
			nread += 1;
		}
		nread
	}
}

/// Global pipe table, protected by a spinlock.
static PIPES: Mutex<[Option<Pipe>; MAX_PIPES]> = {
	const NONE: Option<Pipe> = None;
	Mutex::new([NONE; MAX_PIPES])
};

/// Create a new pipe.  Returns the pipe ID (0‥MAX_PIPES−1),
/// or `None` if the table is full.
pub fn create() -> Option<usize> {
	let mut pipes = PIPES.lock();
	for (id, slot) in pipes.iter_mut().enumerate() {
		if slot.is_none() {
			*slot = Some(Pipe::new());
			return Some(id);
		}
	}
	None
}

/// Write `data` into pipe `id`.  Returns bytes written, or 0 on error.
pub fn write(id: usize, data: &[u8]) -> usize {
	let mut pipes = PIPES.lock();
	if id >= MAX_PIPES {
		return 0;
	}
	match pipes[id].as_mut() {
		Some(pipe) => pipe.write(data),
		None => 0,
	}
}

/// Read up to `buf.len()` bytes from pipe `id`.
/// Returns bytes read, or 0 if empty / invalid.
pub fn read(id: usize, buf: &mut [u8]) -> usize {
	let mut pipes = PIPES.lock();
	if id >= MAX_PIPES {
		return 0;
	}
	match pipes[id].as_mut() {
		Some(pipe) => pipe.read(buf),
		None => 0,
	}
}

/// Close (destroy) a pipe, freeing its slot.
pub fn close(id: usize) {
	let mut pipes = PIPES.lock();
	if id < MAX_PIPES {
		pipes[id] = None;
	}
}
