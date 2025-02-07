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

        // all convertions are nessesary to preserve sign
        // i8 -> i64 (will sign-extend) -> u64 (just bytes)
        // i8 -> u64 (will zero-extend)
        match opcode {
            0x3 => {
                //I load value from memory to rd
                let addr = self.regs[rs1].wrapping_add(get_i_imm(inst));
                match funct3 {
                    0x0 => {
                        // lb
                        self.regs[rd] = self.load(addr, 8)? as i8 as i64 as u64;
                    }
                    0x1 => {
                        // lh
                        self.regs[rd] = self.load(addr, 16)? as i8 as i64 as u64;
                    }
                    0x2 => {
                        // lw
                        self.regs[rd] = self.load(addr, 32)? as i8 as i64 as u64;
                    }
                    0x3 => {
                        // ld
                        self.regs[rd] = self.load(addr, 64)?;
                    }
                    0x4 => {
                        // lbu
                        self.regs[rd] = self.load(addr, 8)?;
                    }
                    0x5 => {
                        // lhu
                        self.regs[rd] = self.load(addr, 16)?;
                    }
                    0x6 => {
                        // lwu
                        self.regs[rd] = self.load(addr, 32)?;
                    }
                    _ => {}
                }
            }
            0x13 => {
                // I
                let imm = get_i_imm(inst);
                let shamt = (imm & 0x3f) as u32;
                match funct3 {
                    0x0 => {
                        //I addi - add rs1 with immediate, store to rd
                        self.regs[rd] = self.regs[rs1].wrapping_add(imm);
                    }
                    0x1 => {
                        //S (without rs2) slli - rd = rs1 << rs2
                        self.regs[rd] = (self.regs[rs1].wrapping_shl(shamt)) as i32 as i64 as u64;
                    }
                    0x2 => {
                        //I slti - 1 to rd if signed rs1 < signed imm, else 0
                        if (self.regs[rs1] as i64) < (imm as i64) {
                            self.regs[rd] = 1
                        } else {
                            self.regs[rd] = 0
                        }
                    }
                    0x3 => {
                        //I sltiu - 1 to rd if usigned rs1 < usigned imm, else 0
                        if self.regs[rs1] < imm {
                            self.regs[rd] = 1
                        } else {
                            self.regs[rd] = 0
                        }
                    }
                    0x4 => {
                        //I xori - bitwise XOR on rs1 and signed imm
                        self.regs[rd] = self.regs[rs1] ^ imm;
                    }
                    0x5 => {
                        match funct7 {
                            0x0 => {
                                //S (without rs2) srli - rd = rs1 >> rs2
                                self.regs[rd] = self.regs[rs1].wrapping_shr(shamt);
                            }
                            0x20 => {
                                //S (without rs2) srai - rd = rs1 >> rs2 (arithmetic)
                                self.regs[rd] =
                                    ((self.regs[rs1] as i64).wrapping_shr(shamt)) as u64;
                            }
                            _ => {}
                        }
                    }
                    0x6 => {
                        //I ori - bitwise OR on rs1 and signed imm
                        self.regs[rd] = self.regs[rs1] | imm;
                    }
                    0x7 => {
                        //I andi - bitwise ANDI on rs1 and signed imm
                        self.regs[rd] = self.regs[rs1] & imm;
                    }

                    _ => {}
                }
            }
            0x17 => {
                //U auipc - add imm(with << 12) to pc and store to rd
                self.regs[rd] = base_pc.wrapping_add(inst & U_IMMEDIATE);
            }
            0x1b => {
                // I
                let imm = get_i_imm(inst);
                let shamt = (imm & 0x1f) as u32;
                match funct3 {
                    0x0 => {
                        //I addiw - add rs1 with immediate, store to rd
                        self.regs[rd] = self.regs[rs1].wrapping_add(imm) as i32 as i64 as u64;
                    }
                    0x1 => {
                        //S (without rs2) slliw - rd = rs1 << rs2
                        self.regs[rd] = (self.regs[rs1].wrapping_shl(shamt)) as i32 as i64 as u64;
                    }
                    0x5 => {
                        match funct7 {
                            0x0 => {
                                //S (without rs2) srliw - rd = rs1 >> rs2
                                self.regs[rd] =
                                    (self.regs[rs1].wrapping_shr(shamt)) as i32 as i64 as u64;
                            }
                            0x20 => {
                                //S (without rs2) sraiw - rd = rs1 >> rs2 (arithmetic)
                                self.regs[rd] =
                                    ((self.regs[rs1] as i64).wrapping_shr(shamt)) as u64;
                            }
                            _ => {}
                        }
                    }

                    _ => {}
                }
            }
            0x23 => {
                // S store value to memory
                let addr = self.regs[rs1].wrapping_add(get_s_imm(inst));
                match funct3 {
                    0x0 => {
                        // sb
                        self.store(addr, 8, self.regs[rs2])?;
                    }
                    0x1 => {
                        // sh
                        self.store(addr, 16, self.regs[rs2])?;
                    }
                    0x2 => {
                        // sw
                        self.store(addr, 32, self.regs[rs2])?;
                    }
                    0x3 => {
                        // sd
                        self.store(addr, 64, self.regs[rs2])?;
                    }
                    _ => {}
                }
            }
            0x2a => {
                let shamt = (self.regs[rs2] & 0x1f) as u32;
                match funct3 {
                    0x0 => {
                        match funct7 {
                            0x0 => {
                                //R addw - add rs1 with rs2, store to rd
                                self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]) as i32
                                    as i64 as u64;
                            }
                            0x20 => {
                                //R subw - sub rs1 with rs2, store to rd
                                self.regs[rd] = self.regs[rs1].wrapping_sub(self.regs[rs2]) as i32
                                    as i64 as u64;
                            }
                            _ => {}
                        }
                    }
                    0x1 => {
                        //R sllw - rd = rs1 << rs2
                        self.regs[rd] = (self.regs[rs1] as u32).wrapping_shl(shamt) as i32 as u64;
                    }
                    0x5 => {
                        match funct7 {
                            0x0 => {
                                //R srlw (unsigned) - rd = rs1 >> rs2
                                self.regs[rd] =
                                    (self.regs[rs1] as u32).wrapping_shr(shamt) as i32 as u64;
                            }
                            0x20 => {
                                //R sraw - rd = rs1 >> rs2
                                self.regs[rd] =
                                    ((self.regs[rs1] as i32).wrapping_shr(shamt)) as u64;
                            }
                            _ => {}
                        }
                    }
                    0x6 => {
                        //R or - rd = rs1 | rs2
                        self.regs[rd] = self.regs[rs1] | self.regs[rs2];
                    }
                    0x7 => {
                        //R and - rd = rs1 & rs2
                        self.regs[rd] = self.regs[rs1] & self.regs[rs2];
                    }
                    _ => {}
                }
            }
            0x33 => {
                let shamt = (self.regs[rs2] & 0x3f) as u32;
                match funct3 {
                    0x0 => {
                        match funct7 {
                            0x0 => {
                                //R add - add rs1 with rs2, store to rd
                                self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
                            }
                            0x20 => {
                                //R sub - sub rs1 with rs2, store to rd
                                self.regs[rd] = self.regs[rs1].wrapping_sub(self.regs[rs2]);
                            }
                            _ => {}
                        }
                    }
                    0x1 => {
                        //R sll - rd = rs1 << rs2
                        self.regs[rd] = self.regs[rs1].wrapping_shl(shamt);
                    }
                    0x2 => {
                        //R slt - if rs1 < rs2, rd = 1, else rd = 0
                        if (self.regs[rs1] as i64) < (self.regs[rs2] as i64) {
                            self.regs[rd] = 1;
                        } else {
                            self.regs[rd] = 0;
                        }
                    }
                    0x3 => {
                        //R sltu (unsigned) - if rs1 < rs2, rd = 1, else rd = 0
                        if self.regs[rs1] < self.regs[rs2] {
                            self.regs[rd] = 1;
                        } else {
                            self.regs[rd] = 0;
                        }
                    }
                    0x4 => {
                        //R xor - rd = rs1 ^ rs2
                        self.regs[rd] = self.regs[rs1] ^ self.regs[rs2];
                    }
                    0x5 => {
                        match funct7 {
                            0x0 => {
                                //R srl (unsigned) - rd = rs1 >> rs2
                                self.regs[rd] = self.regs[rs1].wrapping_shr(shamt);
                            }
                            0x20 => {
                                //R sra - rd = rs1 >> rs2
                                self.regs[rd] =
                                    ((self.regs[rs1] as i64).wrapping_shr(shamt)) as u64;
                            }
                            _ => {}
                        }
                    }
                    0x6 => {
                        //R or - rd = rs1 | rs2
                        self.regs[rd] = self.regs[rs1] | self.regs[rs2];
                    }
                    0x7 => {
                        //R and - rd = rs1 & rs2
                        self.regs[rd] = self.regs[rs1] & self.regs[rs2];
                    }
                    _ => {}
                }
            }
            0x37 => {
                //U lui - load imm to register, with << 12
                self.regs[rd] = inst & U_IMMEDIATE;
            }
            0x63 => {
                // S - add imm12 to pc if
                let imm = get_b_imm(inst);
                match funct3 {
                    0x0 =>
                    // beq
                    {
                        if self.regs[rs1] == self.regs[rs2] {
                            return Ok(self.pc.wrapping_add(imm));
                        }
                    }
                    0x1 =>
                    // bne
                    {
                        if self.regs[rs1] != self.regs[rs2] {
                            return Ok(self.pc.wrapping_add(imm));
                        }
                    }
                    0x4 =>
                    // blt
                    {
                        if (self.regs[rs1] as i64) < self.regs[rs2] as i64 {
                            return Ok(self.pc.wrapping_add(imm));
                        }
                    }
                    0x5 =>
                    // bge
                    {
                        if (self.regs[rs1] as i64) >= self.regs[rs2] as i64 {
                            return Ok(self.pc.wrapping_add(imm));
                        }
                    }
                    0x6 =>
                    // bltu
                    {
                        if self.regs[rs1] < self.regs[rs2] {
                            return Ok(self.pc.wrapping_add(imm));
                        }
                    }
                    0x7 =>
                    // bgeu
                    {
                        if self.regs[rs1] >= self.regs[rs2] {
                            return Ok(self.pc.wrapping_add(imm));
                        }
                    }
                    _ => {}
                }
            }
            0x67 => {
                //I jalr - jumps to rs1 + imm12
                self.regs[rd] = self.pc + 4;
                return Ok(self.regs[rs1].wrapping_add(get_i_imm(inst) + DRAM_BASE));
            }
            0x6f => {
                //J jal - jumps to pc + imm20 << 1
                // imm reordering, check wiki for J order
                self.regs[rd] = self.pc + 4;
                return Ok(self.pc.wrapping_add(get_j_imm(inst)));
            }
            _ => {
                return Err(Exept::illegal_instruction(opcode as u64));
            }
        }
        Ok(self.pc.wrapping_add(4))
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
                    if i <= 31 {
                        return self.regs[i];
                    }
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

fn get_i_imm(inst: u64) -> u64 {
    return ((inst as i32 as i64) >> 20) as u64;
}

fn get_j_imm(inst: u64) -> u64 {
    return ((inst & 0x8000_0000) as i32 as i64 >> 11) as u64
        | (inst & 0xff000)
        | ((inst >> 9) & 0x800)
        | (inst >> 20) & 0x7fe;
}

fn get_b_imm(inst: u64) -> u64 {
    return (((inst & 0x80000000) as i32 as i64 >> 19) as u64)
        | ((inst & 0x80) << 4)
        | ((inst >> 20) & 0x7e0)
        | ((inst >> 7) & 0x1e);
}

fn get_s_imm(inst: u64) -> u64 {
    return (((inst & 0xfe000000) as i32 as i64 >> 20) as u64) | ((inst >> 7) & 0x1f);
}
