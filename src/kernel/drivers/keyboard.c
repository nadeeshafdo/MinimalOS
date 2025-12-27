#include "keyboard.h"
#include "../arch/x86_64/idt.h"
#include "../lib/printk.h"
#include "../process/scheduler.h"
#include "../process/process.h"

#define KB_DATA_PORT    0x60
#define KB_STATUS_PORT  0x64
#define KB_BUFFER_SIZE  256

// Circular buffer for keyboard input
static char kb_buffer[KB_BUFFER_SIZE];
static volatile size_t kb_read_pos = 0;
static volatile size_t kb_write_pos = 0;
static volatile size_t kb_count = 0;

// Keyboard state
static bool shift_pressed = false;
static bool caps_lock = false;

// Scancode to ASCII translation table (US QWERTY, scancode set 1)
static const char scancode_to_ascii[] = {
    0,  27, '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\b',
    '\t', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p', '[', ']', '\n',
    0,  // Left Ctrl
    'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', ';', '\'', '`',
    0,  // Left Shift
    '\\', 'z', 'x', 'c', 'v', 'b', 'n', 'm', ',', '.', '/', 
    0,  // Right Shift
    '*',
    0,  // Left Alt
    ' ',  // Space
    0,  // Caps Lock
};

// Shifted characters
static const char scancode_to_ascii_shift[] = {
    0,  27, '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '_', '+', '\b',
    '\t', 'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', '{', '}', '\n',
    0,  // Left Ctrl
    'A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', ':', '"', '~',
    0,  // Left Shift
    '|', 'Z', 'X', 'C', 'V', 'B', 'N', 'M', '<', '>', '?',
    0,  // Right Shift
    '*',
    0,  // Left Alt
    ' ',  // Space
    0,  // Caps Lock
};

static inline u8 inb(u16 port) {
    u8 ret;
    __asm__ volatile("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}

static inline void outb(u16 port, u8 value) {
    __asm__ volatile("outb %0, %1" : : "a"(value), "Nd"(port));
}

// Add character to keyboard buffer
static void kb_buffer_put(char c) {
    if (kb_count < KB_BUFFER_SIZE) {
        kb_buffer[kb_write_pos] = c;
        kb_write_pos = (kb_write_pos + 1) % KB_BUFFER_SIZE;
        kb_count++;
    }
}

// Get character from keyboard buffer (non-blocking)
static char kb_buffer_get(void) {
    if (kb_count == 0) {
        return 0;
    }
    
    char c = kb_buffer[kb_read_pos];
    kb_read_pos = (kb_read_pos + 1) % KB_BUFFER_SIZE;
    kb_count--;
    return c;
}

// Keyboard interrupt handler (IRQ1)
static void keyboard_interrupt_handler(struct registers* regs) {
    (void)regs;
    
    u8 scancode = inb(KB_DATA_PORT);
    
    // Handle key release (bit 7 set)
    if (scancode & 0x80) {
        scancode &= 0x7F;  // Clear release bit
        
        // Handle shift key release
        if (scancode == 0x2A || scancode == 0x36) {  // Left/Right Shift
            shift_pressed = false;
        }
        return;
    }
    
    // Handle special keys
    if (scancode == 0x2A || scancode == 0x36) {  // Left/Right Shift
        shift_pressed = true;
        return;
    }
    
    if (scancode == 0x3A) {  // Caps Lock
        caps_lock = !caps_lock;
        return;
    }
    
    // Translate scancode to ASCII
    char ascii = 0;
    if (scancode < sizeof(scancode_to_ascii)) {
        if (shift_pressed) {
            ascii = scancode_to_ascii_shift[scancode];
        } else {
            ascii = scancode_to_ascii[scancode];
        }
        
        // Apply caps lock to letters only
        if (caps_lock && ascii >= 'a' && ascii <= 'z') {
            ascii -= 32;  // Convert to uppercase
        } else if (caps_lock && ascii >= 'A' && ascii <= 'Z' && shift_pressed) {
            ascii += 32;  // Convert to lowercase (caps+shift = lowercase)
        }
    }
    
    // Add to buffer if printable or special character
    if (ascii != 0) {
        kb_buffer_put(ascii);
    }
}

void keyboard_init(void) {
    printk("[KEYBOARD] Initializing PS/2 keyboard driver...\n");
    
    // Initialize buffer state
    kb_read_pos = 0;
    kb_write_pos = 0;
    kb_count = 0;
    
    // Clear buffer
    for (int i = 0; i < KB_BUFFER_SIZE; i++) {
        kb_buffer[i] = 0;
    }
    
    // Register IRQ1 interrupt handler (INT 33)
    register_interrupt_handler(33, keyboard_interrupt_handler);
    
    // Enable IRQ1 in PIC (unmask bit 1)
    u8 mask = inb(0x21);
    mask &= ~0x02;  // Unmask IRQ1
    outb(0x21, mask);
    
    printk("[KEYBOARD] Initialization complete!\n");
}

bool keyboard_has_char(void) {
    return kb_count > 0;
}

char keyboard_getchar(void) {
    // Check if character already available (non-blocking fast path)
    if (keyboard_has_char()) {
        __asm__ volatile("cli");
        char c = kb_buffer_get();
        __asm__ volatile("sti");
        return c;
    }
    
    // No input available - block the current process
    process_t* current = process_get_current();
    if (current != NULL) {
        // Block this process - it will be woken by keyboard interrupt
        scheduler_block_on_keyboard(current);
        
        // Trigger a reschedule to run another process
        // The process will be resumed when keyboard input arrives
        schedule();
        
        // When we get here, we've been woken up and there should be input
        if (keyboard_has_char()) {
            __asm__ volatile("cli");
            char c = kb_buffer_get();
            __asm__ volatile("sti");
            return c;
        }
    }
    
    // Fallback: just return 0 if no current process or no input
    return 0;
}
