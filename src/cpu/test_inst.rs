use crate::cpu::test_framework::rv_helper;

macro_rules! riscv_test {
        ($code:expr, $name: expr, $clock:expr, $($real:expr => $expect:expr),* ) => {
            match rv_helper($code, $name, $clock) {
                Ok(cpu) => {
                    $(assert_eq!(cpu.reg($real), $expect);)*
                }
                Err(e) => {
                    println!("error: {}", e);
                     assert!(false);
                    }
                }
            }
        }

#[test]
fn test_addi() {
    let code = "addi x31, x0, 42";
    riscv_test!(code, "test_addi", 1, "x31" => 42);
}
