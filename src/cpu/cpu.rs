use std::usize;

use crate::bus::Bus;
use crate::csr::*;
use crate::exept::Exept;
use crate::param::{DRAM_BASE, DRAM_END};
use crate::{bus, csr};

const I_IMMEDIATE: u64 = 0xfff0_0000;
const U_IMMEDIATE: u64 = 0xffff_f000;

// fancy names for registers
const RVABI: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6",
];

//riscV privilege mode
type Mode = u64;
const User: Mode = 0b00;
const Supervisor: Mode = 0b01;
const Machine: Mode = 0b11;

pub struct Cpu {
    //RISC-V has 32 registers
    pub regs: [u64; 32],
    // pc register contains the memory address of the next instruction
    pub pc: u64,
    pub mode: Mode,
    pub bus: bus::Bus,
    pub csr: csr::Csr,
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
            csr: Csr::new(),
            mode: Machine,
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

        // all convertions are nessesary to preserve sign
        // i8 -> i64 (will sign-extend)
        // i8 -> u64 (will zero-extend)
        println!("{:x}: {:x} {:x} -> {:x}", opcode, funct3, funct7, inst);
        match opcode {
            0x3 => {
                //I load value from memory to rd
                let addr = self.regs[rs1].wrapping_add(get_i_imm(inst));
                self.regs[rd] = match funct3 {
                    0x0 => self.load(addr, 8)? as i8 as i64 as u64, // lb
                    0x1 => self.load(addr, 16)? as i16 as i64 as u64, // lh
                    0x2 => self.load(addr, 32)? as i32 as i64 as u64, // lw
                    0x3 => self.load(addr, 64)?,                    //ld
                    0x4 => self.load(addr, 8)?,                     // lbu
                    0x5 => self.load(addr, 16)?,                    // lhu
                    0x6 => self.load(addr, 32)?,                    // lwu
                    _ => return Err(Exept::illegal_instruction(opcode as u64)),
                }
            }
            0x13 => {
                // I
                let imm = get_i_imm(inst);
                let shamt = get_shamt_6(imm);
                match funct3 {
                    0x0 => {
                        //I addi - add rs1 with immediate, store to rd
                        self.regs[rd] = self.regs[rs1].wrapping_add(imm);
                    }
                    0x1 => {
                        //S (without rs2) slli - rd = rs1 << rs2
                        self.regs[rd] = self.regs[rs1].wrapping_shl(shamt);
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
                                self.regs[rd] = (self.regs[rs1] as i64).wrapping_shr(shamt) as u64;
                            }
                            _ => return Err(Exept::illegal_instruction(opcode as u64)),
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

                    _ => return Err(Exept::illegal_instruction(opcode as u64)),
                }
            }
            0x17 => {
                //U auipc - add imm(with << 12) to pc and store to rd
                self.regs[rd] = self.pc.wrapping_add(get_u_imm(inst));
            }
            0x1b => {
                // I
                let imm = get_i_imm(inst);
                let shamt = get_shamt_5(imm);
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
                                self.regs[rd] = ((self.regs[rs1] as u32).wrapping_shr(shamt)) as i32
                                    as i64 as u64;
                            }
                            0x20 => {
                                //S (without rs2) sraiw - rd = rs1 >> rs2 (arithmetic)
                                self.regs[rd] =
                                    ((self.regs[rs1] as i32).wrapping_shr(shamt)) as i64 as u64;
                            }
                            _ => return Err(Exept::illegal_instruction(opcode as u64)),
                        }
                    }

                    _ => return Err(Exept::illegal_instruction(opcode as u64)),
                }
            }
            0x23 => {
                // S store value to memory
                let addr = self.regs[rs1].wrapping_add(get_s_imm(inst));
                match funct3 {
                    0x0 => self.store(addr, 8, self.regs[rs2])?,  // sb
                    0x1 => self.store(addr, 16, self.regs[rs2])?, // sh
                    0x2 => self.store(addr, 32, self.regs[rs2])?, // sw
                    0x3 => self.store(addr, 64, self.regs[rs2])?, // sd
                    _ => return Err(Exept::illegal_instruction(opcode as u64)),
                }
            }
            0x33 => {
                let shamt = get_shamt_6(self.regs[rs2]);
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
                            _ => return Err(Exept::illegal_instruction(opcode as u64)),
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
                            _ => return Err(Exept::illegal_instruction(opcode as u64)),
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
                    _ => return Err(Exept::illegal_instruction(opcode as u64)),
                }
            }
            0x37 => {
                //U lui - load imm to register, with << 12
                self.regs[rd] = get_u_imm(inst);
            }
            0x3b => {
                let shamt = get_shamt_5(self.regs[rs2]);
                match funct3 {
                    0x0 => {
                        match funct7 {
                            0x0 => {
                                //R addw - add rs1 with rs2, store to rd
                                self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]) as i32
                                    as i64 as u64;
                            }
                            0x01 => {
                                //R mulw - multiply rs1 with rs2, store to rd
                                self.regs[rd] = self.regs[rs1].wrapping_mul(self.regs[rs2]) as i32
                                    as i64 as u64;
                            }
                            0x20 => {
                                //R subw - sub rs1 with rs2, store to rd
                                self.regs[rd] =
                                    (self.regs[rs1].wrapping_sub(self.regs[rs2]) as i32) as u64;
                            }
                            _ => return Err(Exept::illegal_instruction(opcode as u64)),
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
                            _ => return Err(Exept::illegal_instruction(opcode as u64)),
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
                    _ => return Err(Exept::illegal_instruction(opcode as u64)),
                }
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
                        if (self.regs[rs1] as i64) < (self.regs[rs2] as i64) {
                            return Ok(self.pc.wrapping_add(imm));
                        }
                    }
                    0x5 =>
                    // bge
                    {
                        if (self.regs[rs1] as i64) >= (self.regs[rs2] as i64) {
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
                    _ => return Err(Exept::illegal_instruction(opcode as u64)),
                }
            }
            0x67 => {
                //I jalr - jumps to rs1 + imm12
                let t = self.pc + 4;
                let new_pc = (self.regs[rs1].wrapping_add(get_i_imm(inst))) & !1;

                self.regs[rd] = t;
                return Ok(new_pc);
            }
            0x6f => {
                //J jal - jumps to pc + imm20 << 1
                self.regs[rd] = self.pc + 4;
                return Ok(self.pc.wrapping_add(get_j_imm(inst)));
            }
            0x73 => {
                let csr = get_i_imm(inst) as usize;
                let zimm = rs1 as u64;
                match funct3 {
                    0x0 => match (rs2, funct7) {
                        (0x2, 0x8) => {
                            // sret
                            let mut sstatus = self.csr.load(SSTATUS);
                            self.mode = (sstatus & MASK_SPP) >> 8;
                            let spie = (sstatus & MASK_SPIE) >> 5;
                            //sie = spie
                            sstatus = (sstatus & !MASK_SIE) | (spie << 1);
                            // spie = 1
                            sstatus = sstatus | MASK_SPIE;
                            // SPP = 0b00, => U
                            sstatus &= !MASK_SPP;
                            self.csr.store(SSTATUS, sstatus);
                            let new_pc = self.csr.load(SEPC) & !0b11;
                            return Ok(new_pc);
                        }
                        (0x2, 0x18) => {
                            // mret
                            let mut mstatus = self.csr.load(MSTATUS);
                            self.mode = (mstatus & MASK_MPP) >> 11;
                            let mpie = (mstatus & MASK_MPIE) >> 7;
                            //mie = mpie
                            mstatus = (mstatus & !MASK_MIE) | (mpie << 3);
                            // mpie = 1
                            mstatus = mstatus | MASK_MPIE;
                            // MPP = 0b00, => U
                            mstatus &= !MASK_MPP;
                            // If MPP != M, sets MPRV=0
                            mstatus &= !MASK_MPRV;
                            self.csr.store(SSTATUS, mstatus);
                            let new_pc = self.csr.load(MEPC) & !0b11;
                            return Ok(new_pc);
                        }
                        _ => return Err(Exept::illegal_instruction(opcode as u64)),
                    },
                    0x1 => {
                        // csrrw
                        if rs1 != 0 {
                            self.regs[rd] = self.csr.load(csr);
                            self.csr.store(csr, self.regs[rs1]);
                        }
                    }
                    0x2 => {
                        // csrrs
                        if rs1 != 0 {
                            let t = self.csr.load(csr);
                            self.csr.store(csr, t | self.regs[rs1]);
                            self.regs[rd] = t;
                        }
                    }
                    0x3 => {
                        // csrrc
                        if rs1 != 0 {
                            let t = self.csr.load(csr);
                            self.csr.store(csr, t & self.regs[rs1]);
                            self.regs[rd] = t;
                        }
                    }
                    0x5 => {
                        // csrrwi
                        if rs1 != 0 {
                            self.regs[rd] = self.csr.load(csr);
                            self.csr.store(csr, zimm);
                        }
                    }
                    0x6 => {
                        // csrrsi
                        if rs1 != 0 {
                            let t = self.csr.load(csr);
                            self.csr.store(csr, t | zimm);
                            self.regs[rd] = t;
                        }
                    }
                    0x7 => {
                        // csrrci
                        if rs1 != 0 {
                            let t = self.csr.load(csr);
                            self.csr.store(csr, t & zimm);
                            self.regs[rd] = t;
                        }
                    }
                    _ => return Err(Exept::illegal_instruction(opcode as u64)),
                }
            }
            _ => return Err(Exept::illegal_instruction(opcode as u64)),
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
            "mhartid" => self.csr.load(MHARTID),
            "mstatus" => self.csr.load(MSTATUS),
            "mtvec" => self.csr.load(MTVEC),
            "mepc" => self.csr.load(MEPC),
            "mcause" => self.csr.load(MCAUSE),
            "mtval" => self.csr.load(MTVAL),
            "medeleg" => self.csr.load(MEDELEG),
            "mscratch" => self.csr.load(MSCRATCH),
            "MIP" => self.csr.load(MIP),
            "mcounteren" => self.csr.load(MCOUNTEREN),
            "sstatus" => self.csr.load(SSTATUS),
            "stvec" => self.csr.load(STVEC),
            "sepc" => self.csr.load(SEPC),
            "scause" => self.csr.load(SCAUSE),
            "stval" => self.csr.load(STVAL),
            "sscratch" => self.csr.load(SSCRATCH),
            "SIP" => self.csr.load(SIP),
            "SATP" => self.csr.load(SATP),
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

// SHift AMounT - 5 bytes
fn get_shamt_5(imm: u64) -> u32 {
    return (imm & 0x1f) as u32;
}

// SHift AMounT - 6 bytes
fn get_shamt_6(imm: u64) -> u32 {
    return (imm & 0x3f) as u32;
}

fn get_u_imm(inst: u64) -> u64 {
    return (inst & U_IMMEDIATE) as i32 as i64 as u64;
}

fn get_i_imm(inst: u64) -> u64 {
    return ((((inst & I_IMMEDIATE) as i32) as i64) >> 20) as u64;
}

fn get_j_imm(inst: u64) -> u64 {
    return ((inst & 0x8000_0000) as i32 as i64 >> 11) as u64
        | (inst & 0xff000)
        | ((inst >> 9) & 0x800)
        | (inst >> 20) & 0x7fe;
}

fn get_b_imm(inst: u64) -> u64 {
    return (((inst & 0x8000_0000) as i32 as i64 >> 19) as u64)
        | ((inst & 0x80) << 4)
        | ((inst >> 20) & 0x7e0)
        | ((inst >> 7) & 0x1e);
}

fn get_s_imm(inst: u64) -> u64 {
    return (((inst & 0xfe000000) as i32 as i64 >> 20) as u64) | ((inst >> 7) & 0x1f);
}
