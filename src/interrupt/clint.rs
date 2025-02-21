use crate::{
    exept::Exception,
    param::{CLINT_MTIME, CLINT_MTIMECMP, PLIC_PENDING, PLIC_SCLAIM, PLIC_SENABLE, PLIC_SPRIORITY},
};

pub struct Clint {
    mtime: u64,
    mtimecmp: u64,
}

impl Clint {
    pub fn new() -> Self {
        Self {
            mtime: 0,
            mtimecmp: 0,
        }
    }

    pub fn load(&self, addr: u64, size: u64) -> Result<u64, Exception> {
        if size != 64 {
            return Err(Exception::LoadAccessFault(addr));
        }
        match addr {
            CLINT_MTIME => Ok(self.mtime),
            CLINT_MTIMECMP => Ok(self.mtimecmp),
            _ => Ok(0),
        }
    }

    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exception> {
        if size != 64 {
            return Err(Exception::StoreAMOAccessFault(addr));
        }
        match addr {
            CLINT_MTIME => Ok(self.mtime = value),
            CLINT_MTIMECMP => Ok(self.mtimecmp = value),
            _ => Ok(()),
        }
    }
}
