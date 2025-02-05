use crate::exept::Exept;
use crate::param::{DRAM_BASE, DRAM_SIZE};

pub struct Dram {
    pub dram: Vec<u8>,
}

impl Dram {
    pub fn new(code: Vec<u8>) -> Self {
        let mut dram = vec![0; DRAM_SIZE as usize];
        dram.splice(..code.len(), code.into_iter());
        Self { dram }
    }

    pub fn load(&self, addr: u64, size: u64) -> Result<u64, Exept> {
        if ![8, 16, 24, 32].contains(&size) {
            return Err(Exept::load_access_fault(size));
        }

        return Ok(self.load_little_endian((addr - DRAM_BASE) as usize, (size / 8) as usize));
    }

    fn load_little_endian(&self, index: usize, bytes: usize) -> u64 {
        let mut code = self.dram[index] as u64;
        for i in 1..bytes {
            code |= (self.dram[index + i] as u64) << (i * 8);
        }
        code
    }

    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exept> {
        if ![8, 16, 24, 32].contains(&size) {
            return Err(Exept::store_amo_access_fault(size));
        }

        self.store_little_endian((addr - DRAM_BASE) as usize, (size / 8) as usize, value);

        Ok(())
    }

    fn store_little_endian(&mut self, index: usize, bytes: usize, value: u64) {
        for i in 0..bytes {
            self.dram[index + i] = (value >> i * 8) as u8;
        }
    }
}
