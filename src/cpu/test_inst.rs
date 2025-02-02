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
    let code = "addi x1, x0, 42";
    riscv_test!(code, "test_addi", 1, "x1" => 42);
}

#[test]
fn test_add() {
    let code = "addi x29, x0, 2
addi x30, x0, 10
add  x31, x30, x29";
    riscv_test!(code, "test_add", 3, "x31" => 12);
}

#[test]
fn test_lui() {
    let code = "lui x31, 20";
    riscv_test!(code, "test_lui", 1, "x31" => 81920);
}

#[test]
fn test_auipc_1() {
    let code = "auipc x31, 42";
    riscv_test!(code, "test_auipc_1", 1, "x31" => 172032);
}

#[test]
fn test_auipc_2() {
    let code = "addi x20, x21, 0
auipc x31, 1";
    riscv_test!(code, "test_auipc_2", 2, "x31" => 4100);
}

#[test]
fn test_jal() {
    let code = "addi x20, x20, 1
jal x1, 8
addi x20, x20, 1
addi x20, x20, 1";
    riscv_test!(code, "test_jal", 4, "x20" => 2);
}

#[test]
fn test_jalr() {
    let code = "addi x20, x20, 8
addi x21, x21, 7
addi x20, x20, 4
jalr x1, x20, 8
addi x21, x21, 1
addi x21, x21, 1";
    riscv_test!(code, "test_jalr", 6, "x21" => 8);
}

#[test]
fn test_beq() {
    let code = "addi x20, x20, 8
addi x21, x21, 8
beq x20, x21, -4 
addi x31, x0, 1
";
    riscv_test!(code, "test_beq", 6, "x21" => 16, "x31" => 1);
}

//addi x20, x20, 8
//addi x21, x21, 1
//bne x20, x21, -4
//addi x31, x0, 1
