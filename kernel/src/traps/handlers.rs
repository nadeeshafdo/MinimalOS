//! Interrupt and exception handlers.

use x86_64::structures::idt::InterruptStackFrame;

/// IST index used for the double fault handler.
pub const DOUBLE_FAULT_IST_INDEX: u8 = 1;

/// Breakpoint exception handler (INT 3).
///
/// This is a trap-type exception triggered by the `int3` instruction.
/// It's commonly used for debugging.
pub extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
	klog::info!("[020] Breakpoint exception triggered!");
}

/// Double Fault exception handler (INT 8).
///
/// A double fault occurs when the CPU fails to invoke an exception handler.
/// This is typically caused by a stack overflow or a missing IDT entry.
/// This handler runs on a dedicated IST stack to ensure it can execute
/// even when the kernel stack is corrupted.
///
/// A double fault is an abort - it cannot be recovered from.
pub extern "x86-interrupt" fn double_fault_handler(
	stack_frame: InterruptStackFrame,
	error_code: u64,
) -> ! {
	klog::error!("=== DOUBLE FAULT ===");
	klog::error!("Error code: {}", error_code);
	klog::error!("{:#?}", stack_frame);
	klog::error!("System halted.");

	loop {
		unsafe {
			core::arch::asm!("cli; hlt", options(nomem, nostack));
		}
	}
}

/// Timer interrupt handler (vector 32).
///
/// Fired periodically by the Local APIC Timer. This is the kernel's
/// heartbeat — it drives preemptive scheduling and timekeeping.
/// An EOI must be sent to the APIC at the end of every timer interrupt.
pub extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
	// Send End of Interrupt to the APIC *first*, so the timer can
	// continue firing even if schedule() switches to a different task.
	khal::apic::eoi();

	// [072] Increment the global tick counter.
	crate::task::clock::tick();

	// [064] The Slice — preemptive scheduling on timer tick.
	// Use try_lock to avoid deadlock if the scheduler is already held
	// (e.g. inside sys_yield or sys_spawn).
	if let Some(sched) = crate::task::process::SCHEDULER.try_lock() {
		let count = sched.task_count();
		drop(sched); // Release lock before context switch
		if count > 1 {
			unsafe { crate::task::process::do_schedule() };
		}
	}
}

/// Spurious interrupt handler (vector 0xFF).
///
/// The APIC may deliver spurious interrupts when the interrupt condition
/// disappears before the CPU acknowledges it. These should be silently
/// ignored — no EOI is sent for spurious interrupts.
pub extern "x86-interrupt" fn spurious_handler(_stack_frame: InterruptStackFrame) {
	// Spurious interrupts require NO end-of-interrupt signal.
	// Simply return.
}

/// Page Fault exception handler (INT 14).
///
/// Temporary diagnostic handler to identify page fault causes.
/// Prints the faulting address (CR2) and error code.
pub extern "x86-interrupt" fn page_fault_handler(
	stack_frame: InterruptStackFrame,
	error_code: x86_64::structures::idt::PageFaultErrorCode,
) {
	let cr2: u64;
	unsafe {
		core::arch::asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags));
	}
	klog::error!("=== PAGE FAULT ===");
	klog::error!("Faulting address (CR2): {:#018x}", cr2);
	klog::error!("Error code: {:?}", error_code);
	klog::error!("{:#?}", stack_frame);
	klog::error!("System halted.");

	loop {
		unsafe {
			core::arch::asm!("cli; hlt", options(nomem, nostack));
		}
	}
}

/// Keyboard interrupt handler (IRQ1 = vector 33).
///
/// Reads the scancode from the PS/2 data port, feeds it through the
/// `pc-keyboard` state machine, echoes the character to the console,
/// and pushes it into the kernel input buffer for `sys_read`.
pub extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
	let status = khal::keyboard::read_status();
	if status & 0x01 != 0 {
		let scancode = khal::keyboard::read_scancode();

		// [074] Feed through the upgraded state machine.
		if let Some(event) = khal::keyboard::handle_scancode_event(scancode) {
			// [078] Push structured event into the EventBuffer.
			let ch = match event.key {
				khal::keyboard::KeyKind::Char(c) => c as u32,
				khal::keyboard::KeyKind::Raw(_) => 0,
			};
			let press = event.state == khal::keyboard::KeyState::Pressed;
			crate::task::events::push_key(press, scancode, ch);

			// Legacy: echo printable chars to console + input buffer.
			if press {
				if let khal::keyboard::KeyKind::Char(c) = event.key {
					// Echo to serial (display echo is now a Wasm actor's job).
					if c == '\x08' {
						khal::serial::write_str("\x08 \x08");
					} else {
						let mut utf8 = [0u8; 4];
						let s = c.encode_utf8(&mut utf8);
						khal::serial::write_str(s);
					}
					crate::task::input::push_char(c);
				}
			}
		}
	}

	// Send EOI via Local APIC (I/O APIC routes the interrupt).
	khal::keyboard::send_eoi();
}

/// [075] Mouse interrupt handler (IRQ12 = vector 44).
///
/// Reads the raw byte from the PS/2 data port, feeds it through the
/// 3-byte packet decoder, and moves the software cursor.
pub extern "x86-interrupt" fn mouse_handler(_stack_frame: InterruptStackFrame) {
	if khal::mouse::is_mouse_data() {
		let byte = khal::mouse::read_data();
		if let Some(packet) = khal::mouse::handle_byte(byte) {
			// [078] Push mouse event into EventBuffer.
			// Cursor rendering is now a Wasm actor's responsibility.
			crate::task::events::push_mouse(
				packet.dx, packet.dy, packet.buttons,
				0, 0, // absolute position tracked by UI actor, not kernel
			);
		}
	}

	khal::mouse::send_eoi();
}
