pub struct Exept {
    message: String,
    pub value: u64,
}

impl Exept {
    pub fn load_access_fault(addr: u64) -> Self {
        Exept {
            message: "LoadAccessFault".to_string(),
            value: addr,
        }
    }

    pub fn store_amo_access_fault(addr: u64) -> Self {
        Exept {
            message: "StoreAMOAccessFault".to_string(),
            value: addr,
        }
    }

    pub fn illegal_instruction(addr: u64) -> Self {
        Exept {
            message: "IllegalInstruction".to_string(),
            value: addr,
        }
    }
}

impl std::fmt::Display for Exept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{} {}", self.message, self.value);
    }
}
