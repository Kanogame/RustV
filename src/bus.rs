use crate::{
    device::{uart::Uart, virtio::virtio::VirtioBlock},
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
    pub virtio_blk: VirtioBlock,
}

impl Bus {
    pub fn new(code: Vec<u8>, disk_image: Vec<u8>) -> Bus {
        Self {
            dram: Dram::new(code),
            uart: Uart::new(),
            plic: Plic::new(),
            clint: Clint::new(),
            virtio_blk: VirtioBlock::new(disk_image),
        }
    }

    pub fn load(&mut self, addr: u64, size: u64) -> Result<u64, Exception> {
        match &addr {
            CLINT_BASE..=CLINT_END => self.clint.load(addr, size),
            PLIC_BASE..=PLIC_END => self.plic.load(addr, size),
            VIRTIO_BASE..=VIRTIO_END => self.virtio_blk.load(addr, size),
            DRAM_BASE..DRAM_END => self.dram.load(addr, size),
            UART_BASE..UART_END => self.uart.load(addr, size),
            // static values
            0x1000..0xFFFF => self.dram.load(addr + DRAM_BASE, size),
            _ => Err(Exception::LoadAccessFault(addr)),
        }
    }

    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exception> {
        match &addr {
            CLINT_BASE..=CLINT_END => self.clint.store(addr, size, value),
            PLIC_BASE..=PLIC_END => self.plic.store(addr, size, value),
            VIRTIO_BASE..=VIRTIO_END => self.virtio_blk.store(addr, size, value),
            DRAM_BASE..DRAM_END => self.dram.store(addr, size, value),
            UART_BASE..UART_END => self.uart.store(addr, size, value),
            // static values
            0x1000..0xFFFF => self.dram.store(addr + DRAM_BASE, size, value),
            _ => Err(Exception::StoreAMOAccessFault(addr)),
        }
    }
}
