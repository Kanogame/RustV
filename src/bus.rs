use crate::{
    dram::Dram,
    exept::Exception,
    param::{DRAM_BASE, DRAM_END},
};

pub struct Bus {
    dram: Dram,
}

impl Bus {
    pub fn new(code: Vec<u8>) -> Bus {
        Self {
            dram: Dram::new(code),
        }
    }

    pub fn load(&self, addr: u64, size: u64) -> Result<u64, Exception> {
        match &addr {
            DRAM_BASE..DRAM_END => {
                return self.dram.load(addr, size);
            }
            _ => Err(Exception::LoadAccessFault(addr)),
        }
    }

    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exception> {
        match &addr {
            DRAM_BASE..DRAM_END => {
                return self.dram.store(addr, size, value);
            }
            _ => Err(Exception::LoadAccessFault(addr)),
        }
    }
}
