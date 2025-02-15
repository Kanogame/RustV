use crate::{
    cpu::test_framework::rv_asm_helper, cpu::test_framework::rv_c_helper, param::DRAM_BASE,
};

macro_rules! riscv_asm_test {
        ($code:expr, $name: expr, $clock:expr, $($real:expr => $expect:expr),* ) => {
            match rv_asm_helper($code, $name, $clock) {
                Ok(cpu) => {
                    $(if cpu.reg($real)!= $expect {
                        cpu.dump_registers();
                        panic!("left {}, right {}", cpu.reg($real), $expect);
                    })*
                }
                Err(e) => {
                    println!("error: {}", e);
                     assert!(false);
                    }
                }
            }
        }

macro_rules! riscv_c_test {
            ($code:expr, $path: expr, $clock:expr, $($real:expr => $expect:expr),* ) => {
                match rv_c_helper($code, $path, $clock) {
                    Ok(cpu) => {
                        $(if cpu.reg($real)!= $expect {
                            cpu.dump_registers();
                            panic!("left {}, right {}", cpu.reg($real), $expect);
                        })*
                    }
                    Err(e) => {
                        println!("error: {}", e);
                         assert!(false);
                        }
                    }
                }
            }

#[test]
fn test_addi_1() {
    let code = "addi x1, x0, 42";
    riscv_asm_test!(code, "test_addi_1", 1, "x1" => 42);
}

#[test]
fn test_addi_2() {
    let code = "addi x1, x0, -42";
    riscv_asm_test!(code, "test_addi_2", 1, "x1" => (-42_i64 as u64));
}

#[test]
fn test_add() {
    let code = "addi x29, x0, 2
addi x30, x0, 10
add  x31, x30, x29";
    riscv_asm_test!(code, "test_add", 3, "x31" => 12);
}

#[test]
fn test_lui() {
    let code = "lui x31, 20";
    riscv_asm_test!(code, "test_lui", 1, "x31" => 81920);
}

#[test]
fn test_auipc_1() {
    let code = "auipc x31, 42";
    riscv_asm_test!(code, "test_auipc_1", 1, "x31" => (42 << 12) + DRAM_BASE);
}

#[test]
fn test_auipc_2() {
    let code = "addi x20, x21, 0
auipc x31, 1";
    riscv_asm_test!(code, "test_auipc_2", 2, "x31" => (1 << 12) + DRAM_BASE + 4);
}

#[test]
fn test_jal() {
    let code = "addi x20, x20, 1
jal x1, 8
addi x20, x20, 1
addi x20, x20, 1";
    riscv_asm_test!(code, "test_jal", 4, "x20" => 2);
}

#[test]
fn test_jalr() {
    let code = "
        addi a1, zero, 42
        jalr a0, -8(a1)
    ";
    riscv_asm_test!(code, "test_jalr", 2, "a0" => DRAM_BASE + 8, "pc" => 34);
}

#[test]
fn test_beq() {
    let code = "addi x20, x20, 8
addi x21, x21, 8
beq x20, x21, -4 
addi x31, x0, 1
";
    riscv_asm_test!(code, "test_beq", 6, "x21" => 16, "x31" => 1);
}

#[test]
fn test_bne() {
    let code = "addi x20, x20, 8
addi x21, x21, 1
bne x20, x21, -4
addi x31, x0, 1
";
    riscv_asm_test!(code, "test_bne", 20, "x21" => 8, "x31" => 1);
}

#[test]
fn test_blt() {
    let code = "addi x20, x20, 8
addi x21, x21, 1
blt x21, x20, -4 
addi x31, x0, 1
";
    riscv_asm_test!(code, "test_blt", 20, "x21" => 8, "x31" => 1);
}

#[test]
fn test_bge() {
    let code = "addi x20, x20, 8
addi x21, x21, 1
bge x20, x21, -4 
addi x31, x0, 1
";
    riscv_asm_test!(code, "test_bge", 20, "x21" => 9, "x31" => 1);
}

#[test]
fn test_slb() {
    let code = "addi sp, sp, -1
addi x20, x0, 82
sb x20, 0(sp)
lb x22, 0(sp)
";
    riscv_asm_test!(code, "test_slb", 4, "x20" => 82, "x22" => 82);
}

#[test]
fn test_swlbu() {
    let code = "addi sp, sp, -4
addi x20, x0, 247
sw x20, 0(sp)
lbu x22, 0(sp)
";
    riscv_asm_test!(code, "test_swlbu", 4, "x20" => 247, "x22" => 247);
}

#[test]
fn test_max_64() {
    let code = "addi x20, x20, -1
srli x20, x20, 1
";
    riscv_asm_test!(code, "test_max_64", 2, "x20" => 0x7fff_ffff_ffff_ffff as u64);
}

#[test]
fn test_li() {
    let code = "
li x20, 0x12345678
";
    riscv_asm_test!(code, "test_li", 4, "x20" => 0x1234_5678);
}

#[test]
fn test_slti() {
    let code = "addi x20, x20, -12
slti x21, x20, 10
addi x22, x22, -30
slti x23, x22, -200
";
    riscv_asm_test!(code, "test_slti", 4, "x21" => 1, "x23" => 0);
}

#[test]
fn test_sltiu() {
    let code = "addi x20, x20, -12
sltiu x21, x20, 10
addi x22, x22, -30
sltiu x23, x22, -200
";
    riscv_asm_test!(code, "test_sltiu", 4, "x21" => 0, "x23" => 0);
}

#[test]
fn test_xori() {
    let code = "addi x20, x20, 0x482
xori x21, x20, 0x273
";
    riscv_asm_test!(code, "test_xori", 2, "x21" => 0x6f1);
}

#[test]
fn test_andi_ori() {
    let code = "addi x20, x20, 0x482
andi x21, x20, 0x273
ori x22, x20, 0x273
";
    riscv_asm_test!(code, "test_andi_ori", 3, "x21" => 2, "x22" => 0x6f3);
}

#[test]
fn test_slli() {
    let code = "addi x20, x20, 10
slli x21, x20, 2
";
    riscv_asm_test!(code, "test_slli", 3, "x21" => 40);
}

#[test]
fn test_sub() {
    let code = "addi x1, x0, 10
addi x2, x0, 3 
sub x3, x1, x2  
";
    riscv_asm_test!(code, "test_sub", 3, "x3" => 7);
}

#[test]
fn test_sll() {
    let code = "addi x20, x20, 3
addi x21, x21, 1
sll x22, x20, x21
";
    riscv_asm_test!(code, "test_sll", 3, "x22" => 6);
}

#[test]
fn test_srai() {
    let code = "addi x1, x0, -4 
srai x3, x1, 1
";
    riscv_asm_test!(code, "test_srai", 2, "x3" => (-2) as i64 as u64);
}

#[test]
fn test_srli() {
    let code = "addi x3, x3, 4
srli x3, x3, 1
";
    riscv_asm_test!(code, "test_srli", 3, "x3" => 2);
}

#[test]
fn test_store_load1() {
    let code = "
        addi s0, zero, 256
        addi sp, sp, -16
        sd   s0, 8(sp)
        lb   t1, 8(sp)
        lh   t2, 8(sp)
        ret
    ";
    riscv_asm_test!(code, "test_store_load1", 10, "t1" => 0, "t2" => 256);
}

#[test]
fn test_func() {
    let code = "
main:
    addi	sp, sp, -8
	sd	ra, 0(sp)
    call is_secret_value
    mv x30, a2
    xor a2, a2, a2
    li a0, 0x69
    call is_secret_value
    mv x31, a2
    xor a2, a2, a2

    ld	ra, 0(sp)
    addi	sp, sp, 8
    ret
is_secret_value:
    addi sp, sp, -8
    sd ra, 0(sp)

    li a1, 0x69
    beq a0, a1, .get_sec
    li a2, 0x3
    j .ret
.get_sec: #get_sec
    li a2, 0x7
    j .ret
.ret: #ret
    ld ra, 0(sp)
    addi sp, sp, 8
    ret
";
    riscv_asm_test!(code, "test_func", 100, "x30" => 3, "x31" => 7);
}

#[test]
fn test_csrs1() {
    let code = "
        addi t0, zero, 1
        addi t1, zero, 2
        addi t2, zero, 3
        csrrw zero, mstatus, t0
        csrrs zero, mtvec, t1
        csrrw zero, mepc, t2
        csrrc t2, mepc, zero
        csrrwi zero, sstatus, 4
        csrrsi zero, stvec, 5
        csrrwi zero, sepc, 6
        csrrci zero, sepc, 0 
        ret
    ";
    riscv_asm_test!(code, "test_csrs1", 20, "mstatus" => 1, "mtvec" => 2, "mepc" => 3,
                                        "sstatus" => 0, "stvec" => 5, "sepc" => 6);
}

#[test]
fn test_simple_c() {
    riscv_c_test!("./m_tests/simple.c", "test_simple_c", 10000, "a0" => 42);
}

#[test]
fn test_fib() {
    riscv_c_test!("./m_tests/fib.c", "test_fib_c", 10000, "a0" => 55);
}

#[test]
fn test_sort_c() {
    riscv_c_test!("./m_tests/sorting.c", "test_sorting_c", 10000, "a0" => 20);
}

#[test]
fn test_amoswap_w() {
    let code = "li a0, 0x10
addi sp, sp, -8
sd a0, 0(sp)
li a1, 0x20
amoswap.w a2, a1, (sp)
ld a0, 0(sp)";

    riscv_asm_test!(code, "test_amoswap_w", 10, "a2" => 0x10, "a0" => 0x20);
}

#[test]
fn test_amoadd() {
    let code = "li a0, 0x10
addi sp, sp, -8
sd a0, 0(sp)
li a1, 0x5
amoadd.d a2, a1, 0(sp)
ld a0, 0(sp)";
    riscv_asm_test!(code, "test_amoadd", 10, "a2" => 0x10, "a0" => 0x15);
}

#[test]
fn test_amoand() {
    let code = "li a0, 0x10
addi sp, sp, -8
sd a0, 0(sp)
li a1, 0x5
amoand.d a2, a1, 0(sp)
ld a0, 0(sp)";
    riscv_asm_test!(code, "test_amoand", 10, "a2" => 0x10, "a0" => 0x10 & 0x5);
}

#[test]
fn test_amoor() {
    let code = "li a0, 0x10
addi sp, sp, -8
sd a0, 0(sp)
li a1, 0x5
amoor.w a2, a1, 0(sp)
ld a0, 0(sp)";

    riscv_asm_test!(code, "test_amoor", 10, "a0" => 0x10 | 0x5);
}

#[test]
fn test_amoxor() {
    let code = "li a0, 0x10
addi sp, sp, -8
sd a0, 0(sp)
li a1, 0x5
amoor.w a2, a1, 0(sp)
ld a0, 0(sp)";

    riscv_asm_test!(code, "test_amoxor", 10, "a0" => 0x10 ^ 0x5);
}

#[test]
fn test_mulhu() {
    let code = "li a0, 0x7fffffffffffffff
li a1, 0x3e8
mulhu a2, a1, a0
";

    riscv_asm_test!(code, "test_mulhu", 10, "a2" => 499);
}

#[test]
fn test_mulh() {
    let code = "li a0, -0x1
li a1, 0x3e8
mulh a2, a1, a0
";

    riscv_asm_test!(code, "test_mulh", 10, "a2" => (-1 as i64 as u64));
}

#[test]
fn test_rem_positive() {
    let code = "addi a0, x0, 10
        addi a1, x0, 3
        rem a2, a0, a1
    ";
    riscv_asm_test!(code, "test_rem_positive", 10, "a2" => 1);
}

#[test]
fn test_rem_negative() {
    let code = "addi a0, x0, -10
addi a1, x0, 3
rem a2, a0, a1
    ";
    riscv_asm_test!(code, "test_rem_negative", 10, "a2" => (-1 as i64 as u64));
}

#[test]
fn test_remu() {
    let code = "addi a0, x0, 10
addi a1, x0, 3
remu a2, a0, a1    
    ";
    riscv_asm_test!(code, "test_remu", 10, "a2" => 1);
}

#[test]
fn test_div_positive() {
    let code = "li a0, 10
li a1, 3
div a2, a0, a1
    ";
    riscv_asm_test!(code, "test_div_positive", 10, "a2" => 3);
}

#[test]
fn test_div_negative() {
    let code = "li a0, -10
li a1, 3
div a2, a0, a1
    ";
    riscv_asm_test!(code, "test_div_negative", 10, "a2" => -3 as i64 as u64);
}

#[test]
fn test_divu() {
    let code = "li a0, 10
li a1, 3
divu a2, a0, a1
    ";
    riscv_asm_test!(code, "test_divu", 10, "a2" => 3);
}

#[test]
fn test_divw_divisor_zero() {
    let code = "li a0, 123
li a1, 0
divw a2, a0, a1
";
    riscv_asm_test!(code, "test_divw_divisor_zero", 10, "a2" => -1 as i64 as u64);
}

#[test]
fn test_divw_overflow() {
    let code = "li a0, 0x80000000
li a1, -1
divw a2, a0, a1
";
    riscv_asm_test!(code, "test_divw_overflow", 10, "a2" => 0x80000000 as u32 as i32 as i64 as u64);
}

#[test]
fn test_mulw_minus_one() {
    let code = "li a0, -1
li a1, -1
mulw a2, a0, a1
";
    riscv_asm_test!(code, "test_mulw_minus_one", 10, "a2" => 1);
}

#[test]
fn test_remw_divisor_zero() {
    let code = "li a0, 0x80000000
li a1, 0
remw a2, a0, a1
";
    riscv_asm_test!(code, "test_remw_divisor_zero", 10, "a2" => 0x80000000 as u32 as i32 as i64 as u64);
}

#[test]
fn test_remw_overflow_division() {
    let code = "li a0, 0x80000000
li a1, -1
remw a2, a0, a1
";
    riscv_asm_test!(code, "test_remw_overflow_division", 10, "a2" => 0);
}
