#[macro_export]
macro_rules! err_illegal_instruction {
    ($inst: expr) => {
        return Err(Exept::illegal_instruction($inst))
    };
}

#[macro_export]
/// extends any signed integer to 64 bit and then converts to u64 (raw bytes)
macro_rules! sign_extend {
    ($from: ty, $value: expr) => {
        $value as $from as i64 as u64
    };
}
