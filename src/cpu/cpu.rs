use core::panic;
use std::cmp::{max, min};
use std::thread::AccessError;
use std::usize;

use crate::bus::Bus;
use crate::device::virtio::virtqueue::{VirtioBlkRequest, VirtqAvail, VirtqDesc, VirtqUsed};
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

pub enum AccessType {
    Instruction,
    Load,
    Store,
}

pub struct Cpu {
    //RISC-V has 32 registers
    pub regs: [u64; 32],
    // pc register contains the memory address of the next instruction
    pub pc: u64,
    pub mode: Mode,
    pub bus: bus::Bus,
    pub csr: csr::Csr,
    pub enable_paging: bool,
    pub page_table: u64,
}

impl Cpu {
    pub fn new(code: Vec<u8>, disk_image: Vec<u8>) -> Self {
        let mut regs = [0; 32];
        //sp - stack pointer
        regs[2] = DRAM_END;
        Self {
            regs,
            pc: DRAM_BASE,
            bus: Bus::new(code, disk_image),
            csr: Csr::new(),
            mode: Machine,
            page_table: 0,
            enable_paging: false,
        }
    }

    // Load value from dram
    pub fn load(&mut self, addr: u64, size: u64) -> Result<u64, Exception> {
        let p_addr = self.translate(addr, AccessType::Load)?;
        self.bus.load(p_addr, size)
    }

    // Store value to dram
    pub fn store(&mut self, addr: u64, size: u64, value: u64) -> Result<(), Exception> {
        let p_addr = self.translate(addr, AccessType::Store)?;
        self.bus.store(p_addr, size, value)
    }

    pub fn fetch(&mut self) -> Result<u64, Exception> {
        let p_pc = self.translate(self.pc, AccessType::Instruction)?;
        match self.bus.load(p_pc, 32) {
            Ok(inst) => Ok(inst),
            Err(_e) => Err(Exception::InstructionAccessFault(self.pc)),
        }
    }

    pub fn execute(&mut self, inst: u64) -> Result<u64, Exception> {
        let (funct7, rs2, rs1, funct3, rd, opcode) = decode_r(inst as u32);
        // by spec x0 is ALWAYS zero
        self.regs[0] = 0;

        // for debug
        //println!("{:x}: {:x} {:x} -> {:x}", opcode, funct3, funct7, inst);

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
            0x0f => {
                // A fence instruction does nothing because this emulator executes an instruction sequentially on a single thread.
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
                        self.regs[rd] = self.regs[rs1] << shamt;
                    }
                    0x2 => {
                        //I slti - 1 to rd if signed rs1 < signed imm, else 0
                        if (self.regs[rs1] as i64) < (imm as i64) {
                            self.regs[rd] = 1;
                        } else {
                            self.regs[rd] = 0;
                        }
                    }
                    0x3 => {
                        //I sltiu - 1 to rd if usigned rs1 < usigned imm, else 0
                        if self.regs[rs1] < imm {
                            self.regs[rd] = 1;
                        } else {
                            self.regs[rd] = 0;
                        }
                    }
                    0x4 => {
                        //I xori - bitwise XOR on rs1 and signed imm
                        self.regs[rd] = self.regs[rs1] ^ imm;
                    }
                    0x5 => {
                        match funct7 >> 1 {
                            0x0 => {
                                //S (without rs2) srli - rd = rs1 >> rs2
                                self.regs[rd] = self.regs[rs1].wrapping_shr(shamt);
                            }
                            0x10 => {
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
                                self.regs[rd] =
                                    (self.regs[rs1] as i32).wrapping_shr(shamt) as i64 as u64;
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
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, t.wrapping_add(self.regs[rs2]))?;
                        self.regs[rd] = t;
                    }
                    (0x2, 0x1) => {
                        // amoswap.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, self.regs[rs2])?;
                        self.regs[rd] = t;
                    }
                    (0x2, 0x2) => {
                        // lr.w
                        self.regs[rd] = self.load(self.regs[rs1], 32)?;
                    }
                    (0x2, 0x3) => {
                        // sc.w, no condition
                        self.store(self.regs[rs1], 32, self.regs[rs2])?;
                    }
                    (0x2, 0x4) => {
                        // amoxor.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, self.regs[rs2] ^ t)?;
                        self.regs[rd] = t;
                    }
                    (0x2, 0x8) => {
                        // amoor.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, self.regs[rs2] | t)?;
                        self.regs[rd] = t;
                    }
                    (0x2, 0xc) => {
                        // amoand.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, self.regs[rs2] & t)?;
                        self.regs[rd] = t;
                    }
                    (0x2, 0x10) => {
                        // amomin.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(
                            self.regs[rs1],
                            32,
                            min(self.regs[rs2] as i32, t as i32) as u64,
                        )?;
                        self.regs[rd] = t;
                    }
                    (0x2, 0x14) => {
                        // amomax.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(
                            self.regs[rs1],
                            32,
                            max(self.regs[rs2] as i32, t as i32) as u64,
                        )?;
                        self.regs[rd] = t;
                    }
                    (0x2, 0x18) => {
                        // amomax.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, min(self.regs[rs2], t))?;
                        self.regs[rd] = t;
                    }
                    (0x2, 0x1c) => {
                        // amomaxu.w
                        let t = self.load(self.regs[rs1], 32)?;
                        self.store(self.regs[rs1], 32, max(self.regs[rs2], t))?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0x0) => {
                        // amoadd.d
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, t.wrapping_add(self.regs[rs2]))?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0x1) => {
                        // amoswap.d
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2])?;
                        self.regs[rd] = t;
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
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2] ^ t)?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0x8) => {
                        // amoor.w
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2] | t)?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0xc) => {
                        // amoand.w
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, self.regs[rs2] & t)?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0x10) => {
                        // amomin.w
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(
                            self.regs[rs1],
                            64,
                            min(self.regs[rs2] as i64, t as i64) as u64,
                        )?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0x14) => {
                        // amomax.w
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(
                            self.regs[rs1],
                            64,
                            max(self.regs[rs2] as i64, t as i64) as u64,
                        )?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0x18) => {
                        // amomax.w
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, min(self.regs[rs2], t))?;
                        self.regs[rd] = t;
                    }
                    (0x3, 0x1c) => {
                        // amomaxu.w
                        let t = self.load(self.regs[rs1], 64)?;
                        self.store(self.regs[rs1], 64, max(self.regs[rs2], t))?;
                        self.regs[rd] = t;
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
                            ((self.regs[rs1].wrapping_sub(self.regs[rs2])) as i32) as u64;
                    }
                    (0x1, 0x0) => {
                        //R sllw - rd = rs1 << rs2
                        self.regs[rd] = (self.regs[rs1] as u32).wrapping_shl(shamt) as i32 as u64;
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
                        self.regs[rd] = (self.regs[rs1] as u32).wrapping_shr(shamt) as i32 as u64;
                    }
                    (0x5, 0x01) => {
                        //R divuw - divide (unsigned) rs1 with rs2, store to rd
                        self.regs[rd] = match self.regs[rs2] {
                            0 => 0xffffffff_ffffffff,
                            _ => {
                                let dividend = self.regs[rs1];
                                let divisor = self.regs[rs2];
                                dividend.wrapping_div(divisor)
                            }
                        };
                    }
                    (0x5, 0x20) => {
                        //R sraw - rd = rs1 >> rs2
                        self.regs[rd] = ((self.regs[rs1] as i32) >> (shamt as i32)) as u64;
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
                    (0x7, 0x1) => {
                        // remuw
                        self.regs[rd] = match self.regs[rs2] {
                            0 => self.regs[rs1],
                            _ => {
                                let dividend = self.regs[rs1] as u32;
                                let divisor = self.regs[rs2] as u32;
                                dividend.wrapping_rem(divisor) as i32 as u64
                            }
                        };
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
                let t = self.pc + 4;
                let new_pc = (self.regs[rs1].wrapping_add(get_i_imm(inst))) & !1;

                self.regs[rd] = t;
                return Ok(new_pc);
            }
            0x6f => {
                //J jal - jumps to pc + imm20 << 1
                self.regs[rd] = self.pc + 4;

                // imm[20|10:1|11|19:12] = inst[31|30:21|20|19:12]
                let imm = get_j_imm(inst);
                return Ok(self.pc.wrapping_add(imm));
            }
            0x73 => {
                let csr_addr = ((inst & 0xfff00000) >> 20) as usize;
                match funct3 {
                    0x0 => {
                        match (rs2, funct7) {
                            // ECALL and EBREAK cause the receiving privilege mode’s epc register to be set to the address of
                            // the ECALL or EBREAK instruction itself, not the address of the following instruction.
                            (0x0, 0x0) => {
                                // ecall
                                // Makes a request of the execution environment by raising an environment call exception.
                                return match self.mode {
                                    User => Err(Exception::EnvironmentCallFromUMode(self.pc)),
                                    Supervisor => Err(Exception::EnvironmentCallFromSMode(self.pc)),
                                    Machine => Err(Exception::EnvironmentCallFromMMode(self.pc)),
                                    _ => unreachable!(),
                                };
                            }
                            (0x1, 0x0) => {
                                // ebreak
                                // Makes a request of the debugger bu raising a Breakpoint exception.
                                return Err(Exception::Breakpoint(self.pc));
                            }
                            (0x2, 0x8) => {
                                // sret
                                // When the SRET instruction is executed to return from the trap
                                // handler, the privilege level is set to user mode if the SPP
                                // bit is 0, or supervisor mode if the SPP bit is 1. The SPP bit
                                // is SSTATUS[8].
                                let mut sstatus = self.csr.load(SSTATUS);
                                self.mode = (sstatus & MASK_SPP) >> 8;
                                // The SPIE bit is SSTATUS[5] and the SIE bit is the SSTATUS[1]
                                let spie = (sstatus & MASK_SPIE) >> 5;
                                // set SIE = SPIE
                                sstatus = (sstatus & !MASK_SIE) | (spie << 1);
                                // set SPIE = 1
                                sstatus |= MASK_SPIE;
                                // set SPP the least privilege mode (u-mode)
                                sstatus &= !MASK_SPP;
                                self.csr.store(SSTATUS, sstatus);
                                // set the pc to CSRs[sepc].
                                // whenever IALIGN=32, bit sepc[1] is masked on reads so that it appears to be 0. This
                                // masking occurs also for the implicit read by the SRET instruction.
                                let new_pc = self.csr.load(SEPC) & !0b11;
                                return Ok(new_pc);
                            }
                            (0x2, 0x18) => {
                                // mret
                                let mut mstatus = self.csr.load(MSTATUS);
                                // MPP is two bits wide at MSTATUS[12:11]
                                self.mode = (mstatus & MASK_MPP) >> 11;
                                // The MPIE bit is MSTATUS[7] and the MIE bit is the MSTATUS[3].
                                let mpie = (mstatus & MASK_MPIE) >> 7;
                                // set MIE = MPIE
                                mstatus = (mstatus & !MASK_MIE) | (mpie << 3);
                                // set MPIE = 1
                                mstatus |= MASK_MPIE;
                                // set MPP the least privilege mode (u-mode)
                                mstatus &= !MASK_MPP;
                                // If MPP != M, sets MPRV=0
                                mstatus &= !MASK_MPRV;
                                self.csr.store(MSTATUS, mstatus);
                                // set the pc to CSRs[mepc].
                                let new_pc = self.csr.load(MEPC) & !0b11;
                                return Ok(new_pc);
                            }
                            (_, 0x9) => {
                                // sfence.vma
                                // Do nothing.
                            }
                            _ => err_illegal_instruction!(inst),
                        }
                    }
                    0x1 => {
                        // csrrw
                        let t = self.csr.load(csr_addr);
                        self.csr.store(csr_addr, self.regs[rs1]);
                        self.regs[rd] = t;

                        self.update_paging(csr_addr);
                    }
                    0x2 => {
                        // csrrs
                        let t = self.csr.load(csr_addr);
                        self.csr.store(csr_addr, t | self.regs[rs1]);
                        self.regs[rd] = t;

                        self.update_paging(csr_addr);
                    }
                    0x3 => {
                        // csrrc
                        let t = self.csr.load(csr_addr);
                        self.csr.store(csr_addr, t & (!self.regs[rs1]));
                        self.regs[rd] = t;

                        self.update_paging(csr_addr);
                    }
                    0x5 => {
                        // csrrwi
                        let zimm = rs1 as u64;
                        self.regs[rd] = self.csr.load(csr_addr);
                        self.csr.store(csr_addr, zimm);

                        self.update_paging(csr_addr);
                    }
                    0x6 => {
                        // csrrsi
                        let zimm = rs1 as u64;
                        let t = self.csr.load(csr_addr);
                        self.csr.store(csr_addr, t | zimm);
                        self.regs[rd] = t;

                        self.update_paging(csr_addr);
                    }
                    0x7 => {
                        // csrrci
                        let zimm = rs1 as u64;
                        let t = self.csr.load(csr_addr);
                        self.csr.store(csr_addr, t & (!zimm));
                        self.regs[rd] = t;

                        self.update_paging(csr_addr);
                    }
                    _ => err_illegal_instruction!(inst),
                }
            }

            _ => err_illegal_instruction!(inst),
        }
        Ok(self.pc.wrapping_add(4))
    }

    pub fn handle_exception(&mut self, e: Exception) {
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
        let tvec_mode = tvec & 0b11;
        let tvec_base = tvec & !0b11;
        match tvec_mode {
            // DIrect
            0 => self.pc = tvec_base,
            1 => self.pc = tvec_base + cause << 2,
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

        for (m, i) in [
            (MASK_MEIP, MachineExternalInterrupt),
            (MASK_MSIP, MachineSoftwareInterrupt),
            (MASK_MTIP, MachineTimerInterrupt),
            (MASK_SEIP, SupervisorExternalInterrupt),
            (MASK_SSIP, SupervisorSoftwareInterrupt),
            (MASK_STIP, SupervisorTimerInterrupt),
        ] {
            if (pending & m) != 0 {
                self.csr.store(MIP, self.csr.load(MIP) & !m);
                return Some(i);
            }
        }

        return None;
    }

    fn update_paging(&mut self, csr_addr: usize) {
        if csr_addr != SATP {
            return;
        }

        let satp = self.csr.load(SATP);
        self.page_table = (satp & MASK_PPN) * PAGE_SIZE;

        let mode = satp >> 60;
        self.enable_paging = mode == 8; // Sv39
    }

    pub fn translate(&mut self, addr: u64, access_type: AccessType) -> Result<u64, Exception> {
        if !self.enable_paging {
            return Ok(addr);
        }

        let levels = 3;
        let vpn = [
            (addr >> 12) & 0x1ff, //L0
            (addr >> 21) & 0x1ff, //L1
            (addr >> 30) & 0x1ff, //L2
        ];

        let mut a = self.page_table;
        let mut i: i64 = levels - 1;
        let mut pte;
        loop {
            pte = self.bus.load(a + vpn[i as usize] * 8, 64)?;

            let v = pte & 1;
            let r = (pte >> 1) & 1;
            let w = (pte >> 2) & 1;
            let x = (pte >> 3) & 1;

            // If pte.v = 0, or if pte.r = 0 and pte.w = 1, stop and raise a page-fault
            // exception corresponding to the original access type.
            if v == 0 || (r == 0 && w == 1) {
                match access_type {
                    AccessType::Instruction => return Err(Exception::InstructionPageFault(addr)),
                    AccessType::Load => return Err(Exception::LoadPageFault(addr)),
                    AccessType::Store => return Err(Exception::StoreAMOPageFault(addr)),
                }
            }

            // leaf pte
            if r == 1 || x == 1 {
                break;
            }

            // text page
            i -= 1;
            let ppn = (pte >> 10) & 0x0fff_ffff_ffff;
            a = ppn * PAGE_SIZE;
            if i < 0 {
                match access_type {
                    AccessType::Instruction => return Err(Exception::InstructionPageFault(addr)),
                    AccessType::Load => return Err(Exception::LoadPageFault(addr)),
                    AccessType::Store => return Err(Exception::StoreAMOPageFault(addr)),
                }
            }
        }

        let ppn = [
            (pte >> 10) & 0x1ff,
            (pte >> 19) & 0x1ff,
            (pte >> 28) & 0x03ff_ffff,
        ];

        let offset = addr & 0xfff;
        match i {
            0 => {
                let ppn = (pte >> 10) & 0x0fff_ffff_ffff;
                Ok((ppn << 12) | offset)
            }
            1 => {
                // Superpage translation. 2 MiB
                Ok((ppn[2] << 30) | (ppn[1] << 21) | (vpn[0] << 12) | offset)
            }
            2 => {
                // Superpage translation. 1 GiB
                Ok((ppn[2] << 30) | (vpn[1] << 21) | (vpn[0] << 12) | offset)
            }
            _ => match access_type {
                AccessType::Instruction => return Err(Exception::InstructionPageFault(addr)),
                AccessType::Load => return Err(Exception::LoadPageFault(addr)),
                AccessType::Store => return Err(Exception::StoreAMOPageFault(addr)),
            },
        }
    }

    pub fn disk_access(&mut self) {
        // size of descriptor table el
        const DESC_SIZE: u64 = size_of::<VirtqDesc>() as u64;
        let desc_addr = self.bus.virtio_blk.desc_addr();
        let avail_addr = desc_addr + DESC_NUM as u64 * DESC_SIZE;
        let used_addr = desc_addr + PAGE_SIZE;
        // casting addresses
        let virtq_avail = unsafe { &(*(avail_addr as *const VirtqAvail)) };
        let virtq_used = unsafe { &(*(used_addr as *const VirtqUsed)) };

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
        // The addr field points to a virtio block request. We need the sector number stored
        // in the sector field. The iotype tells us whether to read or write.
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
        // The next field points to the second descriptor. (data descriptor)
        let next0 = self
            .bus
            .load(&virtq_desc0.next as *const _ as u64, 16)
            .unwrap();

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
    return ((inst & 0x80000000) as i32 as i64 >> 11) as u64
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
