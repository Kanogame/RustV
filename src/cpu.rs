
use crate::bus;
use crate::bus::Bus;
use crate::exept::Exept;
use crate::param::{DRAM_BASE, DRAM_END};

// fancy names for registers
const RVABI: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6",
];

pub struct Cpu {
    //RISC-V has 32 registers
    pub regs: [u64; 32],
    // pc register contains the memory address of the next instruction
    pub pc: u64,
    pub bus: bus::Bus,
}

impl Cpu {
    pub fn new(code: Vec<u8>) -> Self {
        let mut regs = [0; 32];
        //sp - stack pointer
        regs[2] = DRAM_END;
        Self {
            regs,
            pc: DRAM_BASE,
            bus: Bus::new(code),
        }
    }

    // Load value from dram
    pub fn load(&self, addr: u64, size: u64) -> Result<u64, Exept> {
        self.bus.load(addr, size)
    }

    // Store value to dram
    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exept> {
        self.bus.store(addr, size, value)
    }


    pub fn fetch(&mut self) -> Result<u64, Exept> {
        return self.bus.load(self.pc, 32);
    }

    pub fn execute(&mut self, inst: u64) -> Result<u64, Exept> {
        let (funct7, rs2, rs1, funct3, rd, opcode) = decode_r(inst as u32);

        // by spec x0 is ALWAYS zero
        self.regs[0] = 0;

        match opcode {
            0x13 => {
                //addi - add rs1 with immediate, store to rd
                let imm = ((inst & 0xfff0_0000) as i64 >> 20) as u64;
                self.regs[rd] = self.regs[rs1].wrapping_add(imm);
            }
            0x33 => {
                //add - add rs1 with rs2, store to rd
                self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
            }
            _ => {
                return Err(Exept::illegal_instruction(opcode as u64));
            }
        }
        Ok(self.pc + 4)
    }

    pub fn dump_registers(&mut self) {
        println!("{:-^80}", "registers");
        let mut output = String::new();
        self.regs[0] = 0;

        for i in (0..32).step_by(4) {
            let i0 = format!("x{}", i);
            let i1 = format!("x{}", i + 1); 
            let i2 = format!("x{}", i + 2);
            let i3 = format!("x{}", i + 3); 
            let line = format!(
                "{:3}({:^4}) = {:<#18x} {:3}({:^4}) = {:<#18x} {:3}({:^4}) = {:<#18x} {:3}({:^4}) = {:<#18x}\n",
                i0, RVABI[i], self.regs[i], 
                i1, RVABI[i + 1], self.regs[i + 1], 
                i2, RVABI[i + 2], self.regs[i + 2], 
                i3, RVABI[i + 3], self.regs[i + 3],
            );
            output = output + &line;
        }
        println!("{}", output);
    }

}

// decode type R
fn decode_r(inst: u32) -> (u32, usize, usize, u32, usize, u32) {
    return (
        (inst >> 25) & 0x7f,
        ((inst >> 20) & 0x1f) as usize,
        ((inst >> 15) & 0x1f) as usize,
        (inst >> 12) & 0x7,
        ((inst >> 7) & 0x1f) as usize,
        inst & 0x7f,
    );
}