use crate::{
    device::uart::Uart,
    dram::Dram,
    exept::Exception,
    param::{DRAM_BASE, DRAM_END, UART_BASE, UART_END},
};

pub struct Bus {
    dram: Dram,
    pub uart: Uart,
}

impl Bus {
    pub fn new(code: Vec<u8>) -> Bus {
        Self {
            dram: Dram::new(code),
            uart: Uart::new(),
        }
    }

    pub fn load(&mut self, addr: u64, size: u64) -> Result<u64, Exception> {
        match &addr {
            DRAM_BASE..DRAM_END => {
                return self.dram.load(addr, size);
            }
            UART_BASE..UART_END => {
                return self.uart.load(addr, size);
            }
            _ => Err(Exception::LoadAccessFault(addr)),
        }
    }

    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exception> {
        match &addr {
            DRAM_BASE..DRAM_END => {
                return self.dram.store(addr, size, value);
            }
            UART_BASE..UART_END => {
                return self.uart.store(addr, size, value);
            }
            _ => Err(Exception::LoadAccessFault(addr)),
        }
    }
}
