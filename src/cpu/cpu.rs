use core::panic;
use std::cmp::{max, min};
use std::usize;

use crate::bus::Bus;
use crate::device::virtio::virtqueue::{VirtioBlkRequest, VirtqAvail, VirtqDesc};
use crate::exept::Exception;
use crate::interrupt::interrupt::Interrupt;
use crate::param::{
    DESC_NUM, DRAM_BASE, DRAM_END, PAGE_SIZE, PLIC_SCLAIM, SECTOR_SIZE, UART_IRQ, VIRTIO_BLK_T_IN,
    VIRTIO_BLK_T_OUT, VIRTIO_IRQ,
};
use crate::{bus, csr, sign_extend};
use crate::{csr::*, err_illegal_instruction};

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
    pub fn load(&mut self, addr: u64, size: u64) -> Result<u64, Exception> {
        self.bus.load(addr, size)
    }

    // Store value to dram
    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exception> {
        self.bus.store(addr, size, value)
    }

    pub fn fetch(&mut self) -> Result<u64, Exception> {
        match self.bus.load(self.pc, 32) {
            Ok(inst) => Ok(inst),
            Err(_e) => Err(Exception::InstructionAccessFault(self.pc)),
        }
    }

    pub fn execute(&mut self, inst: u64) -> Result<u64, Exception> {
        let (funct7, rs2, rs1, funct3, rd, opcode) = decode_r(inst as u32);
        //println!("{:x}: {:x} {:x} -> {:x}", opcode, funct3, funct7, inst);
        // by spec x0 is ALWAYS zero
        self.regs[0] = 0;

        // all convertions are nessesary to preserve sign
        // i8 -> i64 (will sign-extend)
        // i8 -> u64 (will zero-extend)
        match opcode {
            0x3 => {
                //I load value from memory to rd
                let addr = self.regs[rs1].wrapping_add(get_i_imm(inst));
                self.regs[rd] = match funct3 {
                    0x0 => sign_extend!(i8, self.load(addr, 8)?),   // lb
                    0x1 => sign_extend!(i16, self.load(addr, 16)?), // lh
                    0x2 => sign_extend!(i32, self.load(addr, 32)?), // lw
                    0x3 => self.load(addr, 64)?,                    //ld
                    0x4 => self.load(addr, 8)?,                     // lbu
                    0x5 => self.load(addr, 16)?,                    // lhu
                    0x6 => self.load(addr, 32)?,                    // lwu
                    _ => err_illegal_instruction!(inst),
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
                            _ => err_illegal_instruction!(inst),
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

                    _ => err_illegal_instruction!(inst),
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
                        self.regs[rd] = sign_extend!(i32, self.regs[rs1].wrapping_add(imm));
                    }
                    0x1 => {
                        //S (without rs2) slliw - rd = rs1 << rs2
                        self.regs[rd] = sign_extend!(i32, (self.regs[rs1].wrapping_shl(shamt)));
                    }
                    0x5 => {
                        match funct7 {
                            0x0 => {
                                //S (without rs2) srliw - rd = rs1 >> rs2
                                self.regs[rd] = sign_extend!(
                                    i32,
                                    ((self.regs[rs1] as u32).wrapping_shr(shamt))
                                );
                            }
                            0x20 => {
                                //S (without rs2) sraiw - rd = rs1 >> rs2 (arithmetic)
                                self.regs[rd] = sign_extend!(
                                    i32,
                                    ((self.regs[rs1] as i32).wrapping_shr(shamt))
                                );
                            }
                            _ => err_illegal_instruction!(inst),
                        }
                    }

                    _ => err_illegal_instruction!(inst),
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
                    _ => err_illegal_instruction!(inst),
                }
            }
            0x2f => {
                let funct5 = funct7 >> 2;
                match (funct3, funct5) {
                    (0x2, 0x0) => {
                        // amoadd.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                        self.store(
                            self.regs[rs1],
                            32,
                            self.regs[rs2].wrapping_add(self.regs[rd]),
                        )?;
                    }
                    (0x2, 0x1) => {
                        // amoswap.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                        self.store(self.regs[rs1], 32, self.regs[rs2])?;
                    }
                    (0x2, 0x2) => {
                        // lr.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                    }
                    (0x2, 0x3) => {
                        // sc.w, no condition
                        self.store(self.regs[rs1], 32, self.regs[rs2])?;
                    }
                    (0x2, 0x4) => {
                        // amoxor.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                        self.store(self.regs[rs1], 32, self.regs[rs2] ^ self.regs[rd])?;
                    }
                    (0x2, 0x8) => {
                        // amoor.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                        self.store(self.regs[rs1], 32, self.regs[rs2] | self.regs[rd])?;
                    }
                    (0x2, 0xc) => {
                        // amoand.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                        self.store(self.regs[rs1], 32, self.regs[rs2] & self.regs[rd])?;
                    }
                    (0x2, 0x10) => {
                        // amomin.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                        self.store(
                            self.regs[rs1],
                            32,
                            min(self.regs[rs2] as i64, self.regs[rd] as i64) as u64,
                        )?;
                    }
                    (0x2, 0x14) => {
                        // amomax.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                        self.store(
                            self.regs[rs1],
                            32,
                            max(self.regs[rs2] as i64, self.regs[rd] as i64) as u64,
                        )?;
                    }
                    (0x2, 0x18) => {
                        // amomax.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                        self.store(self.regs[rs1], 32, min(self.regs[rs2], self.regs[rd]))?;
                    }
                    (0x2, 0x1c) => {
                        // amomaxu.w
                        self.regs[rd] = sign_extend!(i32, self.load(self.regs[rs1], 32)?);
                        self.store(self.regs[rs1], 32, max(self.regs[rs2], self.regs[rd]))?;
                    }
                    (0x3, 0x0) => {
                        // amoadd.d
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                        self.store(
                            self.regs[rs1],
                            64,
                            self.regs[rs2].wrapping_add(self.regs[rd]),
                        )?;
                    }
                    (0x3, 0x1) => {
                        // amoswap.w
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2])?;
                    }
                    (0x3, 0x2) => {
                        // lr.d
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                    }
                    (0x3, 0x3) => {
                        // sc.d, no condition
                        self.store(self.regs[rs1], 64, self.regs[rs2])?;
                    }
                    (0x3, 0x4) => {
                        // amoxor.w
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2] ^ self.regs[rd])?;
                    }
                    (0x3, 0x8) => {
                        // amoor.w
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2] | self.regs[rd])?;
                    }
                    (0x3, 0xc) => {
                        // amoand.w
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2] & self.regs[rd])?;
                    }
                    (0x3, 0x10) => {
                        // amomin.w
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                        self.store(
                            self.regs[rs1],
                            64,
                            min(self.regs[rs2] as i64, self.regs[rd] as i64) as u64,
                        )?;
                    }
                    (0x3, 0x14) => {
                        // amomax.w
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                        self.store(
                            self.regs[rs1],
                            64,
                            max(self.regs[rs2] as i64, self.regs[rd] as i64) as u64,
                        )?;
                    }
                    (0x3, 0x18) => {
                        // amomax.w
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, min(self.regs[rs2], self.regs[rd]))?;
                    }
                    (0x3, 0x1c) => {
                        // amomaxu.w
                        self.regs[rd] = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, max(self.regs[rs2], self.regs[rd]))?;
                    }
                    _ => err_illegal_instruction!(inst),
                }
            }
            0x33 => {
                let shamt = get_shamt_6(self.regs[rs2]);
                match (funct3, funct7) {
                    (0x0, 0x0) => {
                        //R add - add rs1 with rs2, store to rd
                        self.regs[rd] = self.regs[rs1].wrapping_add(self.regs[rs2]);
                    }
                    (0x0, 0x1) => {
                        // R mul - multiply rs1 by rs2, store to rd
                        self.regs[rd] = self.regs[rs1].wrapping_mul(self.regs[rs2]);
                    }
                    (0x0, 0x20) => {
                        //R sub - sub rs1 with rs2, store to rd
                        self.regs[rd] = self.regs[rs1].wrapping_sub(self.regs[rs2]);
                    }
                    (0x1, 0x0) => {
                        //R sll - rd = rs1 << rs2
                        self.regs[rd] = self.regs[rs1].wrapping_shl(shamt);
                    }
                    (0x1, 0x1) => {
                        // R mulh - multiply rs1 by rs2 (both signed) as 128, store to rd upper 64
                        let val: i128 = (self.regs[rs1] as i64 as i128)
                            .wrapping_mul(self.regs[rs2] as i64 as i128);
                        self.regs[rd] = (val >> 64) as u64;
                    }
                    (0x2, 0x0) => {
                        //R slt - if rs1 < rs2, rd = 1, else rd = 0
                        if (self.regs[rs1] as i64) < (self.regs[rs2] as i64) {
                            self.regs[rd] = 1;
                        } else {
                            self.regs[rd] = 0;
                        }
                    }
                    (0x2, 0x1) => {
                        // R mulhsu - multiply rs1(s) by rs2(u) as 128, store to rd upper 64
                        let val: i128 =
                            (self.regs[rs1] as i128).wrapping_mul(self.regs[rs2] as u64 as i128);
                        self.regs[rd] = (val >> 64) as u64;
                    }
                    (0x3, 0x0) => {
                        //R sltu (unsigned) - if rs1 < rs2, rd = 1, else rd = 0
                        if self.regs[rs1] < self.regs[rs2] {
                            self.regs[rd] = 1;
                        } else {
                            self.regs[rd] = 0;
                        }
                    }
                    (0x3, 0x1) => {
                        // R mulhu - multiply rs1 by rs2 (both unsigned) as 128, store to rd upper 64
                        let val: u128 =
                            (self.regs[rs1] as u128).wrapping_mul(self.regs[rs2] as u128);
                        self.regs[rd] = (val >> 64) as u64;
                    }
                    (0x4, 0x0) => {
                        //R xor - rd = rs1 ^ rs2
                        self.regs[rd] = self.regs[rs1] ^ self.regs[rs2];
                    }
                    (0x4, 0x1) => {
                        //R div - divide rs1 by rs2 (both signed), store to rd
                        if self.regs[rs2] == 0 {
                            self.regs[rd] = -1 as i64 as u64;
                        } else {
                            self.regs[rd] =
                                (self.regs[rs1] as i64).wrapping_div(self.regs[rs2] as i64) as u64;
                        }
                    }
                    (0x5, 0x0) => {
                        //R srl (unsigned) - rd = rs1 >> rs2
                        self.regs[rd] = self.regs[rs1].wrapping_shr(shamt);
                    }
                    (0x5, 0x1) => {
                        //R divu - divide rs1 by rs2 (both unsigned), store to rd
                        if self.regs[rs2] == 0 {
                            self.regs[rd] = -1 as i64 as u64;
                        } else {
                            self.regs[rd] = (self.regs[rs1]).wrapping_div(self.regs[rs2]);
                        }
                    }
                    (0x5, 0x20) => {
                        //R sra - rd = rs1 >> rs2
                        self.regs[rd] = ((self.regs[rs1] as i64).wrapping_shr(shamt)) as u64;
                    }
                    (0x6, 0x0) => {
                        //R or - rd = rs1 | rs2
                        self.regs[rd] = self.regs[rs1] | self.regs[rs2];
                    }
                    (0x6, 0x1) => {
                        //R rem - signed remainder of div: rs1 by rs2 (both signed), store to rd
                        if self.regs[rs2] == 0 {
                            self.regs[rd] = self.regs[rs1];
                        } else {
                            self.regs[rd] =
                                (self.regs[rs1] as i64).wrapping_rem(self.regs[rs2] as i64) as u64;
                        }
                    }
                    (0x7, 0x0) => {
                        //R and - rd = rs1 & rs2
                        self.regs[rd] = self.regs[rs1] & self.regs[rs2];
                    }
                    (0x7, 0x1) => {
                        //R remu - unsigned remainder of divu: rs1 by rs2 (both unsigned), store to rd
                        if self.regs[rs2] == 0 {
                            self.regs[rd] = self.regs[rs1];
                        } else {
                            self.regs[rd] = self.regs[rs1].wrapping_rem(self.regs[rs2]);
                        }
                    }
                    _ => err_illegal_instruction!(inst),
                }
            }
            0x37 => {
                //U lui - load imm to register, with << 12
                self.regs[rd] = get_u_imm(inst);
            }
            0x3b => {
                let shamt = get_shamt_5(self.regs[rs2]);
                match (funct3, funct7) {
                    (0x0, 0x0) => {
                        //R addw - add rs1 with rs2, store to rd
                        self.regs[rd] =
                            sign_extend!(i32, self.regs[rs1].wrapping_add(self.regs[rs2]));
                    }
                    (0x0, 0x01) => {
                        //R mulw - multiply rs1 with rs2, store to rd
                        self.regs[rd] = sign_extend!(
                            i32,
                            (self.regs[rs1] as i32).wrapping_mul(self.regs[rs2] as i32)
                        );
                    }
                    (0x0, 0x20) => {
                        //R subw - sub rs1 with rs2, store to rd
                        self.regs[rd] =
                            sign_extend!(i32, self.regs[rs1].wrapping_sub(self.regs[rs2]));
                    }
                    (0x1, 0x0) => {
                        //R sllw - rd = rs1 << rs2
                        self.regs[rd] =
                            sign_extend!(i32, (self.regs[rs1] as u32).wrapping_shl(shamt));
                    }
                    (0x4, 0x01) => {
                        //R divw - divide rs1 with rs2, store to rd
                        if self.regs[rs2] as i32 == 0 {
                            self.regs[rd] = -1 as i64 as u64;
                        } else {
                            self.regs[rd] = sign_extend!(
                                i32,
                                (self.regs[rs1] as i32).wrapping_div(self.regs[rs2] as i32)
                            );
                        }
                    }
                    (0x5, 0x0) => {
                        //R srlw (unsigned) - rd = rs1 >> rs2
                        self.regs[rd] =
                            sign_extend!(i32, (self.regs[rs1] as u32).wrapping_shr(shamt));
                    }
                    (0x5, 0x01) => {
                        //R divuw - divide (unsigned) rs1 with rs2, store to rd
                        if self.regs[rs2] as i32 == 0 {
                            self.regs[rd] = -1 as i64 as u64;
                        } else {
                            self.regs[rd] = sign_extend!(
                                i32,
                                (self.regs[rs1] as u32).wrapping_div(self.regs[rs2] as u32)
                            );
                        }
                    }
                    (0x5, 0x20) => {
                        //R sraw - rd = rs1 >> rs2
                        self.regs[rd] =
                            sign_extend!(i32, ((self.regs[rs1] as i32).wrapping_shr(shamt)));
                    }
                    (0x6, 0x0) => {
                        //R or - rd = rs1 | rs2
                        self.regs[rd] = self.regs[rs1] | self.regs[rs2];
                    }
                    (0x6, 0x1) => {
                        //R remw - reminder of signed divw: rs1 with rs2, store to rd
                        if self.regs[rs2] as i32 == 0 {
                            self.regs[rd] = sign_extend!(i32, self.regs[rs1]);
                        } else {
                            self.regs[rd] = sign_extend!(
                                i32,
                                (self.regs[rs1] as i32).wrapping_rem(self.regs[rs2] as i32)
                            );
                        }
                    }
                    (0x7, 0x0) => {
                        //R and - rd = rs1 & rs2
                        self.regs[rd] = self.regs[rs1] & self.regs[rs2];
                    }
                    (0x7, 0x1) => {
                        //R remuw - reminder of unsigned divw: rs1 with rs2, store to rd
                        if self.regs[rs2] as i32 == 0 {
                            self.regs[rd] = sign_extend!(i32, self.regs[rs1]);
                        } else {
                            self.regs[rd] = sign_extend!(
                                i32,
                                (self.regs[rs1] as u32).wrapping_rem(self.regs[rs2] as u32)
                            );
                        }
                    }
                    _ => err_illegal_instruction!(inst),
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
                    _ => err_illegal_instruction!(inst),
                }
            }
            0x67 => {
                //I jalr - jumps to rs1 + imm12
                // new var cause rd can be equal rs1
                let new_pc = (self.regs[rs1].wrapping_add(get_i_imm(inst))) & !1;

                self.regs[rd] = self.pc + 4;
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
                        _ => err_illegal_instruction!(inst),
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
                    _ => err_illegal_instruction!(inst),
                }
            }
            _ => err_illegal_instruction!(inst),
        }
        Ok(self.pc.wrapping_add(4))
    }

    pub fn handle_exeption(&mut self, e: Exception) {
        let pc = self.pc;
        let mode = self.mode;
        let cause = e.code();

        // if an exception happen in U-mode or S-mode, and the exception is delegated to S-mode.
        let trap_in_s_mode = mode <= Supervisor && self.csr.is_medelegated(cause);
        // selecting mode
        let (STATUS, TVEC, CAUSE, TVAL, EPC, MASK_PIE, pie_i, MASK_IE, ie_i, MASK_PP, pp_i) =
            if trap_in_s_mode {
                self.mode = Supervisor;
                (
                    SSTATUS, STVEC, SCAUSE, STVAL, SEPC, MASK_SPIE, 5, MASK_SIE, 1, MASK_SPP, 8,
                )
            } else {
                self.mode = Machine;
                (
                    MSTATUS, MTVEC, MCAUSE, MTVAL, MEPC, MASK_MPIE, 7, MASK_MIE, 3, MASK_MPP, 11,
                )
            };

        self.pc = self.csr.load(TVEC) & !0b11;
        self.csr.store(EPC, pc);
        self.csr.store(CAUSE, cause);
        self.csr.store(TVAL, e.value());
        let mut status = self.csr.load(STATUS);
        let ie = (status & MASK_IE) >> ie_i;
        // set SPIE = SIE / MPIE = MIE
        status = (status & !MASK_PIE) | (ie << pie_i);
        // set SIE = 0 / MIE = 0
        status &= !MASK_IE;
        // set SPP / MPP = previous mode
        status = (status & !MASK_PP) | (mode << pp_i);
        self.csr.store(STATUS, status);
    }

    pub fn handle_interrupt(&mut self, interrupt: Interrupt) {
        let pc = self.pc;
        let mode = self.mode;
        let cause = interrupt.code();
        let trap_in_s_mode = mode <= Supervisor && self.csr.is_midelegated(cause);

        let (STATUS, TVEC, CAUSE, TVAL, EPC, MASK_PIE, pie_i, MASK_IE, ie_i, MASK_PP, pp_i) =
            if trap_in_s_mode {
                self.mode = Supervisor;
                (
                    SSTATUS, STVEC, SCAUSE, STVAL, SEPC, MASK_SPIE, 5, MASK_SIE, 1, MASK_SPP, 8,
                )
            } else {
                self.mode = Machine;
                (
                    MSTATUS, MTVEC, MCAUSE, MTVAL, MEPC, MASK_MPIE, 7, MASK_MIE, 3, MASK_MPP, 11,
                )
            };

        // trap base address
        let tvec = self.csr.load(TVEC);
        self.pc = match tvec & 0b11 {
            0 => tvec & 0b11,
            1 => (tvec & !0b11) + cause << 2,
            _ => unreachable!(),
        };

        self.csr.store(EPC, pc);
        self.csr.store(CAUSE, cause);
        self.csr.store(TVAL, 0);
        let mut status = self.csr.load(STATUS);
        let ie = (status & MASK_IE) >> ie_i;
        // set SPIE = SIE / MPIE = MIE
        status = (status & !MASK_PIE) | (ie << pie_i);
        // set SIE = 0 / MIE = 0
        status &= !MASK_IE;
        // set SPP / MPP = previous mode
        status = (status & !MASK_PP) | (mode << pp_i);
        self.csr.store(STATUS, status);
    }

    pub fn check_pending_interrupt(&mut self) -> Option<Interrupt> {
        use Interrupt::*;
        // is mie on
        if (self.mode == Machine) && (self.csr.load(MSTATUS) & MASK_MIE) == 0 {
            return None;
        }
        // is sie on
        if (self.mode == Supervisor) && (self.csr.load(SSTATUS) & MASK_SIE) == 0 {
            return None;
        }

        // interrupts for external devices
        if self.bus.uart.is_interrupting() {
            self.bus.store(PLIC_SCLAIM, 32, UART_IRQ).unwrap();
            self.csr.store(MIP, self.csr.load(MIP) | MASK_SEIP);
        } else if self.bus.virtio_blk.is_interrupting() {
            self.disk_access();
            self.bus.store(PLIC_SCLAIM, 32, VIRTIO_IRQ).unwrap();
            self.csr.store(MIP, self.csr.load(MIP) | MASK_SEIP);
        }

        let pending = self.csr.load(MIE) & self.csr.load(MIP);

        for i in [
            MASK_MEIP, MASK_MSIP, MASK_MTIP, MASK_SEIP, MASK_SSIP, MASK_STIP,
        ] {
            if (pending & i) != 0 {
                self.csr.store(MIP, self.csr.load(MIP) & !i);
                return Some(MachineExternalInterrupt);
            }
        }

        return None;
    }

    pub fn disk_access(&mut self) {
        // size of descriptor table el
        const DESC_SIZE: u64 = size_of::<VirtqDesc>() as u64;
        let desc_addr = self.bus.virtio_blk.desc_addr();
        let avail_addr = desc_addr + DESC_NUM as u64 * DESC_SIZE;
        let used_addr = desc_addr + PAGE_SIZE;
        // casting addresses
        let virtq_avail = unsafe { &(*(avail_addr as *const VirtqAvail)) };
        let virtq_used = unsafe { &(*(used_addr as *const VirtqAvail)) };

        // indexing idx to available ring
        let idx = self
            .bus
            .load(&virtq_avail.idx as *const _ as u64, 16)
            .unwrap() as usize;
        let index = self
            .bus
            .load(&virtq_avail.ring[idx % DESC_NUM] as *const _ as u64, 16)
            .unwrap();

        //The first descriptor:
        // which contains the request information and a pointer to the data descriptor.
        let desc_addr0 = desc_addr + DESC_SIZE * index;
        let virtq_desc0 = unsafe { &(*(desc_addr0 as *const VirtqDesc)) };
        let next0 = self
            .bus
            .load(&virtq_desc0.next as *const _ as u64, 16)
            .unwrap();

        let req_addr = self
            .bus
            .load(&virtq_desc0.addr as *const _ as u64, 64)
            .unwrap();
        let virtq_blk_req = unsafe { &(*(req_addr as *const VirtioBlkRequest)) };
        let blk_sector = self
            .bus
            .load(&virtq_blk_req.sector as *const _ as u64, 64)
            .unwrap();
        let iotype = self
            .bus
            .load(&virtq_blk_req.iotype as *const _ as u64, 32)
            .unwrap() as u32;

        // the second descriptor.
        let desc_addr1 = desc_addr + DESC_SIZE * next0;
        let virtq_desc1 = unsafe { &(*(desc_addr1 as *const VirtqDesc)) };
        let addr1 = self
            .bus
            .load(&virtq_desc1.addr as *const _ as u64, 64)
            .unwrap();
        let len1 = self
            .bus
            .load(&virtq_desc1.len as *const _ as u64, 32)
            .unwrap();

        match iotype {
            VIRTIO_BLK_T_OUT => {
                for i in 0..len1 {
                    let data = self.bus.load(addr1 + i, 8).unwrap();
                    self.bus
                        .virtio_blk
                        .write_disk(blk_sector * SECTOR_SIZE + i, data);
                }
            }
            VIRTIO_BLK_T_IN => {
                for i in 0..len1 {
                    let data = self.bus.virtio_blk.read_disk(blk_sector * SECTOR_SIZE + i);
                    self.bus.store(addr1 + i, 8, data as u64).unwrap();
                }
            }
            _ => unreachable!(),
        }

        let new_id = self.bus.virtio_blk.get_new_id();
        self.bus
            .store(&virtq_used.idx as *const _ as u64, 16, new_id % 8)
            .unwrap();
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
