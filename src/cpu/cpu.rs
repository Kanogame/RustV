use std::usize;

use crate::bus;
use crate::bus::Bus;
use crate::exept::Exept;
use crate::param::{DRAM_BASE, DRAM_END};

const I_IMMEDIATE: u64 = 0xfff0_0000;
const U_IMMEDIATE: u64 = 0xffff_f000;

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
        let base_pc = self.pc - DRAM_BASE;
        
        match opcode {
            0x13 => {
                //I addi - add rs1 with immediate, store to rd
                let imm = ((inst & I_IMMEDIATE) as i64 >> 20) as u64;
                self.regs[rd] = self.regs[rs1].wrapping_add(imm);
            }
            0x17 => {
                //U auipc - add imm(with << 12) to pc and store to rd
                self.regs[rd] = base_pc.wrapping_add(inst & U_IMMEDIATE);
            }
            0x33 => {
                //R add - add rs1 with rs2, store to rd
                self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
            }
            0x37 => {
                //U lui - load imm to register, with << 12
                self.regs[rd] = inst & U_IMMEDIATE;
            }
            0x67 => {
                //I jalr - jumps to rs1 + imm12
                self.regs[rd] = self.pc + 4;
                println!("HIHI{}", self.regs[rs1] + (inst >> 20));
                return Ok(self.regs[rs1] + (inst >> 20) + DRAM_BASE);
            }
            0x6f => {
                //J jal - jumps to pc + imm20 << 1
                // imm reordering, check wiki for J order
                let imm = ((inst & 0x8000_0000) >> 11) 
                | (inst & 0xff000) 
                | ((inst >> 9) & 0x800) 
                | (inst >> 20) & 0x7fe;
                self.regs[rd] = self.pc + 4;
                return Ok(self.pc + imm);
            }
            _ => {
                return Err(Exept::illegal_instruction(opcode as u64));
            }
        }
        Ok(self.pc + 4)
    }

    pub fn reg(&self, r: &str) -> u64 {
        for (i, val) in RVABI.iter().enumerate() {
            if (*val).eq(r) {
                return self.regs[i];
            }
        }

        self.dump_registers();

        match r {
            "pc" => self.pc,
            "fp" => self.reg("s0"),
            r if r.starts_with("x") => {
                if let Ok(i) = r[1..].parse::<usize>() {
                    if i <= 31 { return self.regs[i]; }
                    panic!("Invalid register {}", r);
                }
                panic!("Invalid register {}", r);
            }
            _ => panic!("Invalid register {}", r),
        }
    }

    pub fn dump_registers(&self) {
        println!("{:-^80}", "registers");
        let mut output = String::new();
        //self.regs[0] = 0;

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
