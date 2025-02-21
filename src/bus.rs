use crate::{
    device::uart::Uart,
    dram::Dram,
    exept::Exception,
    interrupt::{clint::Clint, plic::Plic},
    param::*,
};

pub struct Bus {
    dram: Dram,
    clint: Clint,
    plic: Plic,
    pub uart: Uart,
}

impl Bus {
    pub fn new(code: Vec<u8>) -> Bus {
        Self {
            dram: Dram::new(code),
            uart: Uart::new(),
            plic: Plic::new(),
            clint: Clint::new(),
        }
    }

    pub fn load(&mut self, addr: u64, size: u64) -> Result<u64, Exception> {
        match &addr {
            CLINT_BASE..=CLINT_END => self.clint.load(addr, size),
            PLIC_BASE..=PLIC_END => self.plic.load(addr, size),
            DRAM_BASE..DRAM_END => {
                return self.dram.load(addr, size);
            }
            // static values
            0x1000..0xFFFF => {
                return self.dram.load(addr + DRAM_BASE, size);
            }
            UART_BASE..UART_END => {
                return self.uart.load(addr, size);
            }
            _ => Err(Exception::LoadAccessFault(addr)),
        }
    }

    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exception> {
        match &addr {
            CLINT_BASE..=CLINT_END => self.clint.store(addr, size, value),
            PLIC_BASE..=PLIC_END => self.plic.store(addr, size, value),
            DRAM_BASE..DRAM_END => {
                return self.dram.store(addr, size, value);
            }
            // static values
            0x1000..0xFFFF => {
                return self.dram.store(addr + DRAM_BASE, size, value);
            }
            UART_BASE..UART_END => {
                return self.uart.store(addr, size, value);
            }
            _ => Err(Exception::LoadAccessFault(addr)),
        }
    }
}
