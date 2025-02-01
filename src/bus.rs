use crate::{
    dram::Dram,
    exept::Exept,
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

    pub fn load(&self, addr: u64, size: u64) -> Result<u64, Exept> {
        match &addr {
            DRAM_BASE..DRAM_END => {
                return self.dram.load(addr, size);
            }
            _ => Err(Exept::load_access_fault(addr)),
        }
    }

    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exept> {
        match &addr {
            DRAM_BASE..DRAM_END => {
                return self.dram.store(addr, size, value);
            }
            _ => Err(Exept::load_access_fault(addr)),
        }
    }
}
