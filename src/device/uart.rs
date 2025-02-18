use std::{
    array,
    io::{self, Read, Write},
    ops::Index,
    sync::{atomic::AtomicBool, Arc, Condvar, Mutex},
    thread,
};

use crate::{
    exept::Exception,
    param::{
        MASK_UART_LSR_RX, MASK_UART_LSR_TX, UART_BASE, UART_LSR, UART_RHR, UART_SIZE, UART_THR,
    },
};

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
                    while array[UART_LSR as usize] & MASK_UART_LSR_RX == 1 {
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

    pub fn load(&mut self, addr: u64, size: u64) -> Result<u64, Exception> {
        if size != 8 {
            return Err(Exception::LoadAccessFault(addr));
        }

        let (uart, cvar) = &*self.uart;
        let mut array = uart.lock().unwrap();
        let index = addr - UART_BASE;
        // a read happens
        match index {
            UART_RHR => {
                // waking up cvar.wait
                cvar.notify_one();
                array[UART_LSR as usize] &= !MASK_UART_LSR_RX;
                Ok(array[UART_RHR as usize] as u64)
            }
            _ => Ok(array[index as usize] as u64),
        }
    }

    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exception> {
        if size != 8 {
            return Err(Exception::StoreAMOAccessFault(addr));
        }

        let (uart, cvar) = &*self.uart;
        let mut array = uart.lock().unwrap();
        let index = addr - UART_BASE;
        match index {
            UART_THR => {
                print!("{}", value as u8 as char);
                io::stdout().flush().unwrap();
                return Ok(());
            }
            _ => {
                array[index as usize] = value as u8;
                return Ok(());
            }
        }
    }

    pub fn is_interrupting(&self) -> bool {
        self.interrupt
            .swap(false, std::sync::atomic::Ordering::Acquire)
    }
}
