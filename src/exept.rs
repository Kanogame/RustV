#[derive(Debug, Copy, Clone)]
pub enum Exception {
    InstructionAddrMisaligned(u64),
    InstructionAccessFault(u64),
    IllegalInstruction(u64),
    Breakpoint(u64),
    LoadAccessMisaligned(u64),
    LoadAccessFault(u64),
    StoreAMOAddrMisaligned(u64),
    StoreAMOAccessFault(u64),
    EnvironmentCallFromUMode(u64),
    EnvironmentCallFromSMode(u64),
    EnvironmentCallFromMMode(u64),
    InstructionPageFault(u64),
    LoadPageFault(u64),
    StoreAMOPageFault(u64),
}

use core::fmt;

use Exception::*;
impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstructionAddrMisaligned(addr) => write!(f, "InstructionAddrMisaligned {:#x}", addr),
            InstructionAccessFault(addr) => write!(f, "InstructionAccessFault {:#x}", addr),
            IllegalInstruction(inst) => write!(f, "IllegalInstruction {:#x}", inst),
            Breakpoint(pc) => write!(f, "Breakpoint {:#x}", pc),
            LoadAccessMisaligned(addr) => write!(f, "LoadAccessMisaligned {:#x}", addr),
            LoadAccessFault(addr) => write!(f, "LoadAccessFault {:#x}", addr),
            StoreAMOAddrMisaligned(addr) => write!(f, "StoreAMOAddrMisaligned {:#x}", addr),
            StoreAMOAccessFault(addr) => write!(f, "StoreAMOAccessFault {:#x}", addr),
            EnvironmentCallFromUMode(pc) => write!(f, "EnvironmentCallFromUMode {:#x}", pc),
            EnvironmentCallFromSMode(pc) => write!(f, "EnvironmentCallFromSMode {:#x}", pc),
            EnvironmentCallFromMMode(pc) => write!(f, "EnvironmentCallFromMMode {:#x}", pc),
            InstructionPageFault(addr) => write!(f, "InstructionPageFault {:#x}", addr),
            LoadPageFault(addr) => write!(f, "LoadPageFault {:#x}", addr),
            StoreAMOPageFault(addr) => write!(f, "StoreAMOPageFault {:#x}", addr),
        }
    }
}

impl Exception {
    pub fn value(self) -> u64 {
        match self {
            InstructionAddrMisaligned(addr) => addr,
            InstructionAccessFault(addr) => addr,
            IllegalInstruction(inst) => inst,
            Breakpoint(pc) => pc,
            LoadAccessMisaligned(addr) => addr,
            LoadAccessFault(addr) => addr,
            StoreAMOAddrMisaligned(addr) => addr,
            StoreAMOAccessFault(addr) => addr,
            EnvironmentCallFromUMode(pc) => pc,
            EnvironmentCallFromSMode(pc) => pc,
            EnvironmentCallFromMMode(pc) => pc,
            InstructionPageFault(addr) => addr,
            LoadPageFault(addr) => addr,
            StoreAMOPageFault(addr) => addr,
        }
    }

    pub fn code(self) -> u64 {
        match self {
            InstructionAddrMisaligned(_) => 0,
            InstructionAccessFault(_) => 1,
            IllegalInstruction(_) => 2,
            Breakpoint(_) => 3,
            LoadAccessMisaligned(_) => 4,
            LoadAccessFault(_) => 5,
            StoreAMOAddrMisaligned(_) => 6,
            StoreAMOAccessFault(_) => 7,
            EnvironmentCallFromUMode(_) => 8,
            EnvironmentCallFromSMode(_) => 9,
            EnvironmentCallFromMMode(_) => 11,
            InstructionPageFault(_) => 12,
            LoadPageFault(_) => 13,
            StoreAMOPageFault(_) => 14,
        }
    }

    pub fn is_fatal(self) -> bool {
        match self {
            InstructionAddrMisaligned(_)
            | InstructionAccessFault(_)
            | LoadAccessFault(_)
            | StoreAMOAddrMisaligned(_)
            | StoreAMOAccessFault(_)
            | IllegalInstruction(_) => true,
            _else => false,
        }
    }
}
