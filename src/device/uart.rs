use std::{
    array,
    io::{self, Read},
    sync::{atomic::AtomicBool, Arc, Condvar, Mutex},
    thread,
};

use crate::param::{MASK_UART_LSR_RX, MASK_UART_LSR_TX, UART_LSR, UART_RHR, UART_SIZE};

pub struct Uart {
    // used by multiple threads
    uart: Arc<(Mutex<[u8; UART_SIZE as usize]>, Condvar)>,
    // bit if interrupt happens
    interrupt: Arc<AtomicBool>,
}

impl Uart {
    pub fn new() -> Self {
        let mut array = [0; UART_SIZE as usize];
        // tell LSR that THR is empty, CPU will load next char
        array[UART_LSR as usize] |= MASK_UART_LSR_TX;

        let uart = Arc::new(((Mutex::new(array)), Condvar::new()));
        let interrupt = Arc::new(AtomicBool::new(false));

        // recieve part
        let read_uart = Arc::clone(&uart);
        let read_interrupt = Arc::clone(&interrupt);
        let mut byte = [0];
        thread::spawn(move || loop {
            match io::stdin().read(&mut byte) {
                Ok(_) => {
                    let (uart, cvar) = &*read_uart;
                    let mut array = uart.lock().unwrap();
                    // if data have been received but not yet be transferred.
                    while (array[UART_LSR as usize] & MASK_UART_LSR_RX == 1) {
                        array = cvar.wait(array).unwrap();
                    }
                    // data have been transferred, so receive next one.
                    array[UART_RHR as usize] = byte[0];
                    read_interrupt.store(true, std::sync::atomic::Ordering::Release);
                    array[UART_LSR as usize] |= MASK_UART_LSR_RX;
                }
                Err(e) => {
                    println!("{}", e);
                }
            }
        });

        Self { uart, interrupt }
    }
}
