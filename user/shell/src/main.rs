//! MinimalOS shell — proper command-line interface.
//!
//! Features:
//! - Argument tokenizer with single-quote, double-quote, and backslash escaping
//! - 15 built-in commands (echo, exit, type, help, history, etc.)
//! - Command history (ring buffer, last 16 commands, `!!` and `!N` recall)
//! - External program execution from ramdisk with arguments
//! - Error messages in standard `cmd: not found` format

#![no_std]
#![no_main]

use core::arch::asm;

// ─────────────────────────────────────────────────────────────
// Syscall numbers (must match kernel/src/arch/syscall.rs)
// ─────────────────────────────────────────────────────────────

const SYS_LOG: u64 = 0;
const SYS_EXIT: u64 = 1;
const SYS_YIELD: u64 = 2;
const SYS_SPAWN: u64 = 3;
const SYS_READ: u64 = 4;
const SYS_PIPE_CREATE: u64 = 5;
const SYS_PIPE_WRITE: u64 = 6;
const SYS_PIPE_READ: u64 = 7;
const SYS_PIPE_CLOSE: u64 = 8;
const SYS_TIME: u64 = 9;
const SYS_SLEEP: u64 = 10;
const SYS_FUTEX: u64 = 11;
const SYS_READ_EVENT: u64 = 12;
const SYS_LIST: u64 = 13;
const SYS_PRINT: u64 = 14;

const FUTEX_WAIT: u64 = 0;
const FUTEX_WAKE: u64 = 1;

const MAX_CMD: usize = 256;
const MAX_ARGS: usize = 16;
const HISTORY_SIZE: usize = 16;

// ─────────────────────────────────────────────────────────────
// Syscall wrappers
// ─────────────────────────────────────────────────────────────

#[inline(always)]
unsafe fn syscall0(nr: u64) -> u64 {
	let ret: u64;
	unsafe {
		asm!(
		"syscall",
		inlateout("rax") nr => ret,
		lateout("rcx") _,
		lateout("r11") _,
		options(nostack),
		);
	}
	ret
}

#[inline(always)]
unsafe fn syscall1(nr: u64, a0: u64) -> u64 {
	let ret: u64;
	unsafe {
		asm!(
		"syscall",
		inlateout("rax") nr => ret,
		in("rdi") a0,
		lateout("rcx") _,
		lateout("r11") _,
		options(nostack),
		);
	}
	ret
}

#[inline(always)]
unsafe fn syscall2(nr: u64, a0: u64, a1: u64) -> u64 {
	let ret: u64;
	unsafe {
		asm!(
		"syscall",
		inlateout("rax") nr => ret,
		in("rdi") a0,
		in("rsi") a1,
		lateout("rcx") _,
		lateout("r11") _,
		options(nostack),
		);
	}
	ret
}

#[inline(always)]
unsafe fn syscall3(nr: u64, a0: u64, a1: u64, a2: u64) -> u64 {
	let ret: u64;
	unsafe {
		asm!(
		"syscall",
		inlateout("rax") nr => ret,
		in("rdi") a0,
		in("rsi") a1,
		in("rdx") a2,
		lateout("rcx") _,
		lateout("r11") _,
		options(nostack),
		);
	}
	ret
}

#[inline(always)]
unsafe fn syscall4(nr: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> u64 {
	let ret: u64;
	unsafe {
		asm!(
		"syscall",
		inlateout("rax") nr => ret,
		in("rdi") a0,
		in("rsi") a1,
		in("rdx") a2,
		in("r10") a3,
		lateout("rcx") _,
		lateout("r11") _,
		options(nostack),
		);
	}
	ret
}

// ─────────────────────────────────────────────────────────────
// High-level syscall wrappers
// ─────────────────────────────────────────────────────────────

/// Log a message to the kernel debug log (serial, with prefix).
fn log(msg: &str) {
	unsafe {
		syscall2(SYS_LOG, msg.as_ptr() as u64, msg.len() as u64);
	}
}

/// Print raw text to the console (serial + framebuffer, no prefix).
fn print(s: &str) {
	unsafe {
		syscall2(SYS_PRINT, s.as_ptr() as u64, s.len() as u64);
	}
}

/// Print text followed by a newline.
fn println(s: &str) {
	print(s);
	print("\n");
}

fn exit(code: u64) -> ! {
	unsafe {
		syscall1(SYS_EXIT, code);
	}
	loop {
		core::hint::spin_loop();
	}
}

fn yield_cpu() {
	unsafe {
		syscall0(SYS_YIELD);
	}
}

/// Spawn a new process from the ramdisk with optional arguments.
fn spawn(path: &str, args: &str) -> u64 {
	unsafe {
		syscall4(
			SYS_SPAWN,
			path.as_ptr() as u64,
			path.len() as u64,
			args.as_ptr() as u64,
			args.len() as u64,
		)
	}
}

/// Read one byte from STDIN. Returns 0 if nothing available.
fn read_char() -> u8 {
	unsafe { syscall1(SYS_READ, 0) as u8 }
}

fn pipe_create() -> u64 {
	unsafe { syscall0(SYS_PIPE_CREATE) }
}

fn pipe_write(id: u64, data: &[u8]) -> u64 {
	unsafe { syscall3(SYS_PIPE_WRITE, id, data.as_ptr() as u64, data.len() as u64) }
}

fn pipe_read(id: u64, buf: &mut [u8]) -> u64 {
	unsafe { syscall3(SYS_PIPE_READ, id, buf.as_mut_ptr() as u64, buf.len() as u64) }
}

fn pipe_close(id: u64) {
	unsafe {
		syscall1(SYS_PIPE_CLOSE, id);
	}
}

fn time() -> u64 {
	unsafe { syscall0(SYS_TIME) }
}

fn sleep(ticks: u64) {
	unsafe {
		syscall1(SYS_SLEEP, ticks);
	}
}

fn futex_wait(addr: *const u64, expected: u64) -> u64 {
	unsafe { syscall3(SYS_FUTEX, addr as u64, FUTEX_WAIT, expected) }
}

fn futex_wake(addr: *const u64, count: u64) -> u64 {
	unsafe { syscall3(SYS_FUTEX, addr as u64, FUTEX_WAKE, count) }
}

fn read_event(buf: &mut [u8; 12]) -> u64 {
	unsafe { syscall1(SYS_READ_EVENT, buf.as_mut_ptr() as u64) }
}

/// List ramdisk files into a buffer (newline-separated).
/// Returns the number of bytes written.
fn list_files(buf: &mut [u8]) -> u64 {
	unsafe { syscall2(SYS_LIST, buf.as_mut_ptr() as u64, buf.len() as u64) }
}

// ─────────────────────────────────────────────────────────────
// Output helpers
// ─────────────────────────────────────────────────────────────

/// Print a u64 value as decimal.
#[allow(dead_code)]
fn print_u64(val: u64) {
	let mut buf = [0u8; 20];
	let n = fmt_u64(val, &mut buf);
	print(unsafe { core::str::from_utf8_unchecked(&buf[..n]) });
}

/// Stack-allocated string builder (no heap needed).
struct OutBuf {
	buf: [u8; 512],
	pos: usize,
}

impl OutBuf {
	fn new() -> Self {
		OutBuf {
			buf: [0u8; 512],
			pos: 0,
		}
	}

	fn push_str(&mut self, s: &str) {
		let bytes = s.as_bytes();
		let n = bytes.len().min(self.buf.len() - self.pos);
		self.buf[self.pos..self.pos + n].copy_from_slice(&bytes[..n]);
		self.pos += n;
	}

	fn push_u64(&mut self, val: u64) {
		self.pos += fmt_u64(val, &mut self.buf[self.pos..]);
	}

	fn as_str(&self) -> &str {
		unsafe { core::str::from_utf8_unchecked(&self.buf[..self.pos]) }
	}

	fn flush(&mut self) {
		print(self.as_str());
		self.pos = 0;
	}
}

// ─────────────────────────────────────────────────────────────
// Tokenizer — splits a command line into arguments with quoting
// ─────────────────────────────────────────────────────────────

/// Parsed arguments from a command line.
struct Args {
	/// Parsed argument data stored contiguously.
	buf: [u8; MAX_CMD],
	/// Start offset of each argument in `buf`.
	starts: [usize; MAX_ARGS],
	/// Length of each argument.
	lens: [usize; MAX_ARGS],
	/// Number of parsed arguments.
	count: usize,
}

impl Args {
	fn new() -> Self {
		Args {
			buf: [0u8; MAX_CMD],
			starts: [0; MAX_ARGS],
			lens: [0; MAX_ARGS],
			count: 0,
		}
	}

	/// Get argument at index as a string slice.
	fn get(&self, idx: usize) -> &str {
		if idx >= self.count {
			return "";
		}
		unsafe {
			core::str::from_utf8_unchecked(
				&self.buf[self.starts[idx]..self.starts[idx] + self.lens[idx]],
			)
		}
	}
}

/// Tokenize a command line into arguments with quoting support.
///
/// - Whitespace separates arguments
/// - `'...'` — single quotes: everything is literal (no escaping)
/// - `"..."` — double quotes: `\\`, `\"`, `\n`, `\t` are escaped
/// - `\x` outside quotes — escapes the next character
/// - Adjacent quoted/unquoted segments merge into one argument
fn tokenize(input: &[u8], args: &mut Args) {
	args.count = 0;
	let mut buf_pos: usize = 0;
	let mut i: usize = 0;
	let len = input.len();

	while i < len && args.count < MAX_ARGS {
		// Skip whitespace
		while i < len && (input[i] == b' ' || input[i] == b'\t') {
			i += 1;
		}
		if i >= len {
			break;
		}

		// Start of a new argument
		let arg_start = buf_pos;

		// Parse until unquoted whitespace
		while i < len && input[i] != b' ' && input[i] != b'\t' {
			match input[i] {
				b'\'' => {
					// Single-quoted string: everything is literal
					i += 1;
					while i < len && input[i] != b'\'' {
						if buf_pos < args.buf.len() {
							args.buf[buf_pos] = input[i];
							buf_pos += 1;
						}
						i += 1;
					}
					if i < len {
						i += 1;
					} // skip closing quote
				}
				b'"' => {
					// Double-quoted string: handle escape sequences
					i += 1;
					while i < len && input[i] != b'"' {
						if input[i] == b'\\' && i + 1 < len {
							i += 1;
							let escaped = match input[i] {
								b'\\' => b'\\',
								b'"' => b'"',
								b'n' => b'\n',
								b't' => b'\t',
								other => other,
							};
							if buf_pos < args.buf.len() {
								args.buf[buf_pos] = escaped;
								buf_pos += 1;
							}
						} else {
							if buf_pos < args.buf.len() {
								args.buf[buf_pos] = input[i];
								buf_pos += 1;
							}
						}
						i += 1;
					}
					if i < len {
						i += 1;
					} // skip closing quote
				}
				b'\\' => {
					// Backslash escape outside quotes
					i += 1;
					if i < len {
						if buf_pos < args.buf.len() {
							args.buf[buf_pos] = input[i];
							buf_pos += 1;
						}
						i += 1;
					}
				}
				_ => {
					// Regular character
					if buf_pos < args.buf.len() {
						args.buf[buf_pos] = input[i];
						buf_pos += 1;
					}
					i += 1;
				}
			}
		}

		let arg_len = buf_pos - arg_start;
		if arg_len > 0 {
			args.starts[args.count] = arg_start;
			args.lens[args.count] = arg_len;
			args.count += 1;
		}
	}
}

// ─────────────────────────────────────────────────────────────
// Command history — ring buffer of last N commands
// ─────────────────────────────────────────────────────────────

struct History {
	entries: [[u8; MAX_CMD]; HISTORY_SIZE],
	lengths: [usize; HISTORY_SIZE],
	count: usize,
	head: usize,
}

impl History {
	const fn new() -> Self {
		History {
			entries: [[0u8; MAX_CMD]; HISTORY_SIZE],
			lengths: [0; HISTORY_SIZE],
			count: 0,
			head: 0,
		}
	}

	/// Add a command to history.
	fn push(&mut self, cmd: &[u8]) {
		let len = cmd.len().min(MAX_CMD);
		self.entries[self.head][..len].copy_from_slice(&cmd[..len]);
		self.lengths[self.head] = len;
		self.head = (self.head + 1) % HISTORY_SIZE;
		if self.count < HISTORY_SIZE {
			self.count += 1;
		}
	}

	/// Get command at index (0 = oldest, count-1 = newest).
	fn get(&self, idx: usize) -> Option<&str> {
		if idx >= self.count {
			return None;
		}
		let actual = if self.count < HISTORY_SIZE {
			idx
		} else {
			(self.head + idx) % HISTORY_SIZE
		};
		let len = self.lengths[actual];
		Some(unsafe { core::str::from_utf8_unchecked(&self.entries[actual][..len]) })
	}

	/// Number of commands stored.
	fn len(&self) -> usize {
		self.count
	}

	/// Get the most recent command.
	fn last(&self) -> Option<&str> {
		if self.count == 0 {
			None
		} else {
			self.get(self.count - 1)
		}
	}
}

// ─────────────────────────────────────────────────────────────
// Built-in commands
// ─────────────────────────────────────────────────────────────

const BUILTINS: &[&str] = &[
	"echo", "exit", "type", "help", "history", "hello", "spawn", "files", "pipe", "time", "sleep",
	"uptime", "events", "futex",
];

fn is_builtin(name: &str) -> bool {
	for &b in BUILTINS {
		if str_eq(name, b) {
			return true;
		}
	}
	false
}

/// echo [args...] — print arguments separated by spaces
fn cmd_echo(args: &Args) {
	for i in 1..args.count {
		if i > 1 {
			print(" ");
		}
		print(args.get(i));
	}
	print("\n");
}

/// exit [code] — exit the shell
fn cmd_exit(args: &Args) -> ! {
	let code = if args.count > 1 {
		parse_u64(args.get(1)).unwrap_or(0)
	} else {
		0
	};
	println("Goodbye.");
	exit(code);
}

/// type <command> — show whether command is a builtin or program
fn cmd_type(args: &Args) {
	if args.count < 2 {
		println("type: missing argument");
		return;
	}
	for i in 1..args.count {
		let name = args.get(i);
		if is_builtin(name) {
			let mut out = OutBuf::new();
			out.push_str(name);
			out.push_str(" is a shell builtin\n");
			out.flush();
		} else {
			// Check ramdisk for the file or file.elf
			let mut found = false;
			if is_ramdisk_file(name) {
				let mut out = OutBuf::new();
				out.push_str(name);
				out.push_str(" is /ramdisk/");
				out.push_str(name);
				out.push_str("\n");
				out.flush();
				found = true;
			} else if is_ramdisk_file_elf(name) {
				let mut out = OutBuf::new();
				out.push_str(name);
				out.push_str(" is /ramdisk/");
				out.push_str(name);
				out.push_str(".elf\n");
				out.flush();
				found = true;
			}
			if !found {
				let mut out = OutBuf::new();
				out.push_str(name);
				out.push_str(": not found\n");
				out.flush();
			}
		}
	}
}

/// help — show available commands
fn cmd_help() {
	println("MinimalOS Shell - Built-in Commands:");
	println("");
	println("  echo [args...]     Print arguments to console");
	println("  exit [code]        Exit the shell (default code: 0)");
	println("  type <command>     Show command type (builtin/program)");
	println("  help               Show this help message");
	println("  history            Show command history");
	println("  hello              Print a greeting");
	println("  spawn <file> [a..] Launch a ramdisk program");
	println("  files              List ramdisk files");
	println("  pipe               Test IPC pipe round-trip");
	println("  time               Show kernel tick count");
	println("  sleep [ticks]      Sleep (default: 500 ticks)");
	println("  uptime             Show system uptime");
	println("  events [seconds]   Read input events (default: 5s)");
	println("  futex              Test futex synchronization");
	println("");
	println("Quoting: 'literal'  \"with \\\"escapes\\\"\"  back\\\\slash");
	println("History: !! (repeat last)  !N (repeat Nth command)");
}

/// history — show command history
fn cmd_history(history: &History) {
	if history.len() == 0 {
		println("  (no history)");
		return;
	}
	for i in 0..history.len() {
		if let Some(cmd) = history.get(i) {
			let mut out = OutBuf::new();
			out.push_str("  ");
			out.push_u64((i + 1) as u64);
			out.push_str("  ");
			out.push_str(cmd);
			out.push_str("\n");
			out.flush();
		}
	}
}

/// hello — print a greeting
fn cmd_hello() {
	println("Hello from MinimalOS shell!");
}

/// spawn <file> [args...] — launch a ramdisk program
fn cmd_spawn(args: &Args) {
	if args.count < 2 {
		println("spawn: missing filename");
		println("Usage: spawn <file.elf> [args...]");
		return;
	}
	let path = args.get(1);

	// Build args string from remaining arguments
	let mut args_buf = [0u8; MAX_CMD];
	let mut args_pos = 0;
	for i in 2..args.count {
		if i > 2 && args_pos < args_buf.len() {
			args_buf[args_pos] = b' ';
			args_pos += 1;
		}
		let a = args.get(i);
		let bytes = a.as_bytes();
		let n = bytes.len().min(args_buf.len() - args_pos);
		args_buf[args_pos..args_pos + n].copy_from_slice(&bytes[..n]);
		args_pos += n;
	}
	let spawn_args = unsafe { core::str::from_utf8_unchecked(&args_buf[..args_pos]) };

	let pid = spawn(path, spawn_args);
	if pid != u64::MAX {
		let mut out = OutBuf::new();
		out.push_str("Spawned ");
		out.push_str(path);
		out.push_str(" (pid ");
		out.push_u64(pid);
		out.push_str(")\n");
		out.flush();
	} else {
		let mut out = OutBuf::new();
		out.push_str("spawn: ");
		out.push_str(path);
		out.push_str(": file not found in ramdisk\n");
		out.flush();
	}
}

/// files — list ramdisk files
fn cmd_files() {
	let mut buf = [0u8; 1024];
	let n = list_files(&mut buf);
	if n == u64::MAX || n == 0 {
		println("  (no files)");
		return;
	}
	let listing = unsafe { core::str::from_utf8_unchecked(&buf[..n as usize]) };
	for line in listing.split('\n') {
		if !line.is_empty() {
			print("  ");
			println(line);
		}
	}
}

/// pipe — test IPC pipe round-trip
fn cmd_pipe() {
	let id = pipe_create();
	if id == u64::MAX {
		println("pipe: failed to create pipe");
		return;
	}
	let msg = b"Hello from pipe!";
	let written = pipe_write(id, msg);
	let mut rbuf = [0u8; 64];
	let nread = pipe_read(id, &mut rbuf);
	pipe_close(id);

	let mut out = OutBuf::new();
	out.push_str("pipe: wrote ");
	out.push_u64(written);
	out.push_str(", read ");
	out.push_u64(nread);
	out.push_str(" bytes: ");
	let copy_len = (nread as usize).min(out.buf.len() - out.pos);
	out.buf[out.pos..out.pos + copy_len].copy_from_slice(&rbuf[..copy_len]);
	out.pos += copy_len;
	out.push_str("\n");
	out.flush();
}

/// time — show kernel tick count
fn cmd_time() {
	let t = time();
	let mut out = OutBuf::new();
	out.push_str("Ticks: ");
	out.push_u64(t);
	out.push_str("\n");
	out.flush();
}

/// sleep [ticks] — sleep for N ticks (default 500)
fn cmd_sleep(args: &Args) {
	let ticks = if args.count > 1 {
		parse_u64(args.get(1)).unwrap_or(500)
	} else {
		500
	};
	let t0 = time();
	let mut out = OutBuf::new();
	out.push_str("Sleeping for ");
	out.push_u64(ticks);
	out.push_str(" ticks...\n");
	out.flush();
	sleep(ticks);
	let t1 = time();
	let elapsed = t1 - t0;
	let mut out2 = OutBuf::new();
	out2.push_str("Awake! Elapsed: ");
	out2.push_u64(elapsed);
	out2.push_str(" ticks\n");
	out2.flush();
}

/// uptime — show system uptime in ticks and approximate seconds
fn cmd_uptime() {
	let ticks = time();
	let secs = ticks / 100; // assuming ~100 Hz timer
	let mut out = OutBuf::new();
	out.push_str("Uptime: ");
	out.push_u64(ticks);
	out.push_str(" ticks (~");
	out.push_u64(secs);
	out.push_str("s)\n");
	out.flush();
}

/// events [seconds] — read input events for N seconds (default 5)
fn cmd_events(args: &Args) {
	let duration = if args.count > 1 {
		parse_u64(args.get(1)).unwrap_or(5)
	} else {
		5
	};
	let ticks = duration * 100; // ~100 Hz timer
	let mut out = OutBuf::new();
	out.push_str("Reading events for ");
	out.push_u64(duration);
	out.push_str("s... move mouse or press keys.\n");
	out.flush();

	let t0 = time();
	let mut count: u64 = 0;
	loop {
		let now = time();
		if now - t0 > ticks {
			break;
		}
		let mut buf = [0u8; 12];
		let n = read_event(&mut buf);
		if n == 12 {
			count += 1;
			if count <= 5 {
				let kind = buf[0];
				let mut ev = OutBuf::new();
				ev.push_str("  event: kind=");
				ev.push_u64(kind as u64);
				ev.push_str(" code=");
				ev.push_u64(buf[1] as u64);
				ev.push_str("\n");
				ev.flush();
			}
		} else {
			yield_cpu();
		}
	}
	let mut out2 = OutBuf::new();
	out2.push_str("Total events: ");
	out2.push_u64(count);
	out2.push_str("\n");
	out2.flush();
}

/// futex — test futex synchronization
fn cmd_futex() {
	static mut FUTEX_VAR: u64 = 0;
	let addr = &raw const FUTEX_VAR;

	// WAIT with mismatched value — should NOT block
	unsafe { core::ptr::write_volatile(addr as *mut u64, 42) };
	let ret = futex_wait(addr, 0);
	let mut out = OutBuf::new();
	out.push_str("futex: WAIT(42!=0) = ");
	out.push_u64(ret);
	out.push_str(" (expected MAX)\n");
	out.flush();

	// WAKE with no waiters — should return 0
	let woken = futex_wake(addr, 1);
	let mut out2 = OutBuf::new();
	out2.push_str("futex: WAKE(no waiters) = ");
	out2.push_u64(woken);
	out2.push_str(" (expected 0)\n");
	out2.flush();

	println("futex: Synchronisation primitives OK");
}

// ─────────────────────────────────────────────────────────────
// Command dispatch
// ─────────────────────────────────────────────────────────────

fn dispatch(args: &Args, history: &History) {
	if args.count == 0 {
		return;
	}
	let cmd = args.get(0);
	match cmd {
		"echo" => cmd_echo(args),
		"exit" => cmd_exit(args),
		"type" => cmd_type(args),
		"help" => cmd_help(),
		"history" => cmd_history(history),
		"hello" => cmd_hello(),
		"spawn" => cmd_spawn(args),
		"files" => cmd_files(),
		"pipe" => cmd_pipe(),
		"time" => cmd_time(),
		"sleep" => cmd_sleep(args),
		"uptime" => cmd_uptime(),
		"events" => cmd_events(args),
		"futex" => cmd_futex(),
		_ => try_external(args),
	}
}

/// Try to run an external program from the ramdisk.
/// Attempts the command name directly, then with ".elf" suffix.
fn try_external(args: &Args) {
	let cmd = args.get(0);

	// Build args string from remaining arguments
	let mut args_buf = [0u8; MAX_CMD];
	let mut args_pos = 0;
	for i in 1..args.count {
		if i > 1 && args_pos < args_buf.len() {
			args_buf[args_pos] = b' ';
			args_pos += 1;
		}
		let a = args.get(i);
		let bytes = a.as_bytes();
		let n = bytes.len().min(args_buf.len() - args_pos);
		args_buf[args_pos..args_pos + n].copy_from_slice(&bytes[..n]);
		args_pos += n;
	}
	let spawn_args = unsafe { core::str::from_utf8_unchecked(&args_buf[..args_pos]) };

	// Try the command name directly (e.g., "init.elf")
	let pid = spawn(cmd, spawn_args);
	if pid != u64::MAX {
		return;
	}

	// Try with ".elf" suffix (e.g., "init" -> "init.elf")
	let mut elf_name = [0u8; 64];
	let cmd_bytes = cmd.as_bytes();
	if cmd_bytes.len() + 4 <= elf_name.len() {
		elf_name[..cmd_bytes.len()].copy_from_slice(cmd_bytes);
		elf_name[cmd_bytes.len()..cmd_bytes.len() + 4].copy_from_slice(b".elf");
		let name = unsafe { core::str::from_utf8_unchecked(&elf_name[..cmd_bytes.len() + 4]) };
		let pid2 = spawn(name, spawn_args);
		if pid2 != u64::MAX {
			return;
		}
	}

	// Command not found
	let mut out = OutBuf::new();
	out.push_str(cmd);
	out.push_str(": command not found\n");
	out.flush();
}

// ─────────────────────────────────────────────────────────────
// Ramdisk file lookup
// ─────────────────────────────────────────────────────────────

/// Check if a file exists in the ramdisk by exact name.
fn is_ramdisk_file(name: &str) -> bool {
	let mut buf = [0u8; 1024];
	let n = list_files(&mut buf);
	if n == u64::MAX || n == 0 {
		return false;
	}
	let listing = unsafe { core::str::from_utf8_unchecked(&buf[..n as usize]) };
	for line in listing.split('\n') {
		if !line.is_empty() && str_eq(line, name) {
			return true;
		}
	}
	false
}

/// Check if `name.elf` exists in the ramdisk.
fn is_ramdisk_file_elf(name: &str) -> bool {
	let mut elf_name = [0u8; 64];
	let name_bytes = name.as_bytes();
	if name_bytes.len() + 4 > elf_name.len() {
		return false;
	}
	elf_name[..name_bytes.len()].copy_from_slice(name_bytes);
	elf_name[name_bytes.len()..name_bytes.len() + 4].copy_from_slice(b".elf");
	let elf = unsafe { core::str::from_utf8_unchecked(&elf_name[..name_bytes.len() + 4]) };
	is_ramdisk_file(elf)
}

// ─────────────────────────────────────────────────────────────
// Utility functions
// ─────────────────────────────────────────────────────────────

/// Format a u64 as decimal ASCII into `buf`. Returns number of bytes written.
fn fmt_u64(mut val: u64, buf: &mut [u8]) -> usize {
	if val == 0 {
		if !buf.is_empty() {
			buf[0] = b'0';
		}
		return 1;
	}
	let mut tmp = [0u8; 20]; // u64 max is 20 digits
	let mut i = 0;
	while val > 0 {
		tmp[i] = b'0' + (val % 10) as u8;
		val /= 10;
		i += 1;
	}
	let len = i.min(buf.len());
	for j in 0..len {
		buf[j] = tmp[i - 1 - j];
	}
	len
}

/// Parse a decimal u64 from a string.
fn parse_u64(s: &str) -> Option<u64> {
	let bytes = s.as_bytes();
	if bytes.is_empty() {
		return None;
	}
	let mut val: u64 = 0;
	for &b in bytes {
		if b < b'0' || b > b'9' {
			return None;
		}
		val = val.checked_mul(10)?.checked_add((b - b'0') as u64)?;
	}
	Some(val)
}

/// Compare two string slices for equality (byte-by-byte).
fn str_eq(a: &str, b: &str) -> bool {
	let ab = a.as_bytes();
	let bb = b.as_bytes();
	if ab.len() != bb.len() {
		return false;
	}
	for i in 0..ab.len() {
		if ab[i] != bb[i] {
			return false;
		}
	}
	true
}

// ─────────────────────────────────────────────────────────────
// Entry point — REPL
// ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn _start(args_ptr: u64, args_len: u64) -> ! {
	log("[069] MinimalOS shell started");

	// Display startup arguments if any.
	if args_ptr != 0 && args_len > 0 && args_len <= 256 {
		let args = unsafe {
			let slice = core::slice::from_raw_parts(args_ptr as *const u8, args_len as usize);
			core::str::from_utf8_unchecked(slice)
		};
		let mut msg = OutBuf::new();
		msg.push_str("[shell] started with args: ");
		msg.push_str(args);
		log(msg.as_str());
	}

	// Banner
	print("\n");
	print("  MinimalOS Shell v2.0\n");
	print("  Type 'help' for commands.\n");
	print("\n");

	let mut line_buf = [0u8; MAX_CMD];
	let mut line_pos: usize = 0;
	let mut history = History::new();

	// Print initial prompt
	print("$ ");

	loop {
		let ch = read_char();
		if ch == 0 {
			yield_cpu();
			continue;
		}

		match ch {
			// Enter / newline
			b'\n' | b'\r' => {
				// Newline is echoed by keyboard handler to both
				// framebuffer and serial — no need to echo here.

				if line_pos > 0 {
					let raw = &line_buf[..line_pos];

					// ── History expansion ────────────────────
					let mut expanded = [0u8; MAX_CMD];
					let mut exp_len: usize = 0;
					let mut did_expand = false;

					if raw.len() >= 2 && raw[0] == b'!' && raw[1] == b'!' {
						// !! — repeat last command
						if let Some(last) = history.last() {
							let bytes = last.as_bytes();
							exp_len = bytes.len().min(MAX_CMD);
							expanded[..exp_len].copy_from_slice(&bytes[..exp_len]);
							did_expand = true;
							println(last);
						} else {
							println("!!: no history");
							line_pos = 0;
							print("$ ");
							continue;
						}
					} else if raw.len() >= 2 && raw[0] == b'!' && raw[1] >= b'0' && raw[1] <= b'9' {
						// !N — repeat command N from history
						let num_str = unsafe { core::str::from_utf8_unchecked(&raw[1..line_pos]) };
						if let Some(n) = parse_u64(num_str) {
							let idx = (n as usize).wrapping_sub(1);
							if let Some(cmd) = history.get(idx) {
								let bytes = cmd.as_bytes();
								exp_len = bytes.len().min(MAX_CMD);
								expanded[..exp_len].copy_from_slice(&bytes[..exp_len]);
								did_expand = true;
								println(cmd);
							} else {
								let mut out = OutBuf::new();
								out.push_str("!");
								out.push_str(num_str);
								out.push_str(": event not found\n");
								out.flush();
								line_pos = 0;
								print("$ ");
								continue;
							}
						} else {
							println("!: invalid number");
							line_pos = 0;
							print("$ ");
							continue;
						}
					}

					let cmd_bytes = if did_expand {
						&expanded[..exp_len]
					} else {
						raw
					};

					// Save to history
					history.push(cmd_bytes);

					// Tokenize and dispatch
					let mut args = Args::new();
					tokenize(cmd_bytes, &mut args);
					dispatch(&args, &history);

					line_pos = 0;
				}

				// Print prompt for next command
				print("$ ");
			}

			// Backspace
			0x08 | 0x7F => {
				if line_pos > 0 {
					line_pos -= 1;
					// Visual backspace is handled by the keyboard handler
					// (framebuffer erase + serial \x08-space-\x08).
				}
			}

			// Tab — reserved for future tab completion
			b'\t' => {}

			// Printable ASCII
			0x20..=0x7E => {
				if line_pos < MAX_CMD - 1 {
					line_buf[line_pos] = ch;
					line_pos += 1;
					// Echo is handled by the keyboard handler
					// (framebuffer + serial).
				}
			}

			_ => {}
		}
	}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
	log("PANIC in shell!");
	exit(1);
}
