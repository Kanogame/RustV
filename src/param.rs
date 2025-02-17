// DRAM
pub const DRAM_SIZE: u64 = 1024 * 1024 * 128;
pub const DRAM_BASE: u64 = 0x8000_0000;
pub const DRAM_END: u64 = DRAM_SIZE + DRAM_BASE - 1;

// UART
pub const UART_BASE: u64 = 0x1000_0000;
pub const UART_SIZE: u64 = 0x100;
pub const UART_END: u64 = UART_BASE + UART_SIZE - 1;
// uart interrupt request
pub const UART_IRQ: u64 = 10;
// Receive holding register (for input bytes).
pub const UART_RHR: u64 = 0;
// Transmit holding register (for output bytes).
pub const UART_THR: u64 = 0;
// Line control register.
pub const UART_LCR: u64 = 3;
// Line status register.
// LSR BIT 0:
//     0 = no data in receive holding register or FIFO.
//     1 = data has been receive and saved in the receive holding register or FIFO.
// LSR BIT 5:
//     0 = transmit holding register is full. 16550 will not accept any data for transmission.
//     1 = transmitter hold register (or FIFO) is empty. CPU can load the next character.
pub const UART_LSR: u64 = 5;
// The receiver (RX) bit MASK.
pub const MASK_UART_LSR_RX: u8 = 1;
// The transmitter (TX) bit MASK.
pub const MASK_UART_LSR_TX: u8 = 1 << 5;
