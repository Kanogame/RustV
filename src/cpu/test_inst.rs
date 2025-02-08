use crate::{cpu::test_framework::rv_helper, param::DRAM_BASE};

macro_rules! riscv_test {
        ($code:expr, $name: expr, $clock:expr, $($real:expr => $expect:expr),* ) => {
            match rv_helper($code, $name, $clock) {
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
    riscv_test!(code, "test_addi_1", 1, "x1" => 42);
}

#[test]
fn test_addi_2() {
    let code = "addi x1, x0, -42";
    riscv_test!(code, "test_addi_2", 1, "x1" => (-42_i64 as u64));
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
    let code = "
        addi a1, zero, 42
        jalr a0, -8(a1)
    ";
    riscv_test!(code, "test_jalr", 2, "a0" => DRAM_BASE + 8, "pc" => 34);
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

#[test]
fn test_bne() {
    let code = "addi x20, x20, 8
addi x21, x21, 1
bne x20, x21, -4
addi x31, x0, 1
";
    riscv_test!(code, "test_bne", 20, "x21" => 8, "x31" => 1);
}

#[test]
fn test_blt() {
    let code = "addi x20, x20, 8
addi x21, x21, 1
blt x21, x20, -4 
addi x31, x0, 1
";
    riscv_test!(code, "test_blt", 20, "x21" => 8, "x31" => 1);
}

#[test]
fn test_bge() {
    let code = "addi x20, x20, 8
addi x21, x21, 1
bge x20, x21, -4 
addi x31, x0, 1
";
    riscv_test!(code, "test_bge", 20, "x21" => 9, "x31" => 1);
}

#[test]
fn test_slb() {
    let code = "addi sp, sp, -1
addi x20, x0, 82
sb x20, 0(sp)
lb x22, 0(sp)
";
    riscv_test!(code, "test_slb", 4, "x20" => 82, "x22" => 82);
}

#[test]
fn test_swlbu() {
    let code = "addi sp, sp, -4
addi x20, x0, 247
sw x20, 0(sp)
lbu x22, 0(sp)
";
    riscv_test!(code, "test_swlbu", 4, "x20" => 247, "x22" => 247);
}

#[test]
fn test_max_64() {
    let code = "addi x20, x20, -1
srli x20, x20, 1
";
    riscv_test!(code, "test_max_64", 2, "x20" => 0x7fff_ffff_ffff_ffff as u64);
}

#[test]
fn test_li() {
    let code = "
li x20, 0x12345678
";
    riscv_test!(code, "test_li", 4, "x20" => 0x1234_5678);
}

#[test]
fn test_slti() {
    let code = "addi x20, x20, -12
slti x21, x20, 10
addi x22, x22, -30
slti x23, x22, -200
";
    riscv_test!(code, "test_slti", 4, "x21" => 1, "x23" => 0);
}

#[test]
fn test_sltiu() {
    let code = "addi x20, x20, -12
sltiu x21, x20, 10
addi x22, x22, -30
sltiu x23, x22, -200
";
    riscv_test!(code, "test_sltiu", 4, "x21" => 0, "x23" => 0);
}

#[test]
fn test_xori() {
    let code = "addi x20, x20, 0x482
xori x21, x20, 0x273
";
    riscv_test!(code, "test_xori", 2, "x21" => 0x6f1);
}

#[test]
fn test_andi_ori() {
    let code = "addi x20, x20, 0x482
andi x21, x20, 0x273
ori x22, x20, 0x273
";
    riscv_test!(code, "test_andi_ori", 3, "x21" => 2, "x22" => 0x6f3);
}

#[test]
fn test_slli() {
    let code = "addi x20, x20, 10
slli x21, x20, 2
";
    riscv_test!(code, "test_slli", 3, "x21" => 40);
}

#[test]
fn test_sub() {
    let code = "addi x1, x0, 10
addi x2, x0, 3 
sub x3, x1, x2  
";
    riscv_test!(code, "test_sub", 3, "x3" => 7);
}

#[test]
fn test_sll() {
    let code = "addi x20, x20, 3
addi x21, x21, 1
sll x22, x20, x21
";
    riscv_test!(code, "test_sll", 3, "x22" => 6);
}

#[test]
fn test_srai() {
    let code = "addi x1, x0, -4 
srai x3, x1, 1
";
    riscv_test!(code, "test_srai", 2, "x3" => (-2) as i64 as u64);
}

#[test]
fn test_srli() {
    let code = "addi x3, x3, 0xfff
srli x3, x3, 1
";
    riscv_test!(code, "test_srli", 3, "x3" => 2);
}

#[test]
fn test_simple_c() {
    let code = "addi	sp,sp,-16
            sd	s0,8(sp)
            addi	s0,sp,16
            li	a5,42
            mv	a0,a5
            ld	s0,8(sp)
            addi	sp,sp,16
            jr	ra
        ";
    riscv_test!(code, "test_simple_c", 20, "a0" => 42);
}

#[test]
fn test_store_load1() {
    let code = "
        addi s0, zero, 256
        addi sp, sp, -16
        sd   s0, 8(sp)
        lb   t1, 8(sp)
        lh   t2, 8(sp)
    ";
    riscv_test!(code, "test_store_load1", 10, "t1" => 0, "t2" => 256);
}

#[test]
fn test_fib_c() {
    let code = "
main: 
	addi	sp, sp, -32
	sd	ra, 24(sp)                      # 8-byte Folded Spill
	sd	s0, 16(sp)                      # 8-byte Folded Spill
	addi	s0, sp, 32
	li	a0, 0
	sw	a0, -20(s0)
	li	a0, 5
    addi x31, x0, 1
	call	fib
	ld	ra, 24(sp)                      # 8-byte Folded Reload
	ld	s0, 16(sp)                      # 8-byte Folded Reload
	addi	sp, sp, 32
	ret
fib:
	addi	sp, sp, -32
	sd	ra, 24(sp)                      # 8-byte Folded Spill
	sd	s0, 16(sp)                      # 8-byte Folded Spill
	addi	s0, sp, 32
                                        # kill: def $x11 killed $x10
	sw	a0, -24(s0)
	lw	a0, -24(s0)
	beqz	a0, .LBB1_2
	j	.LBB1_1
.LBB1_1:
	lw	a0, -24(s0)
	li	a1, 1
	bne	a0, a1, .LBB1_3
	j	.LBB1_2
.LBB1_2:
	lw	a0, -24(s0)
	sw	a0, -20(s0)
	j	.LBB1_4
.LBB1_3:
	lw	a0, -24(s0)
	addiw	a0, a0, -1
	call	fib
	sd	a0, -32(s0)                     # 8-byte Folded Spill
	lw	a0, -24(s0)
	addiw	a0, a0, -2
	call	fib
	mv	a1, a0
	ld	a0, -32(s0)                     # 8-byte Folded Reload
	addw	a0, a0, a1
	sw	a0, -20(s0)
	j	.LBB1_4
.LBB1_4:
	lw	a0, -20(s0)
	ld	ra, 24(sp)                      # 8-byte Folded Reload
	ld	s0, 16(sp)                      # 8-byte Folded Reload
	addi	sp, sp, 32
	ret
";

    riscv_test!(code, "test_fib_c", 1000000, "a0" => 8);
}
