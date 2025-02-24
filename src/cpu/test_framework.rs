use std::{
    fs::File,
    io::{Read, Write},
    process::Command,
};

use crate::cpu::cpu::Cpu;
const TEST_FOLDER: &str = "tests/";
const BINARY_FOLDER: &str = "tests/target/";

//clang -S source -nostdlib -march=rv64i -mabi=lp64 -mno-relax
//add support for folders
fn generate_rv_assembly(source: &str, dest: &str) {
    let cc = "clang";
    let output = Command::new(cc)
        .arg("-S")
        .arg(source)
        .arg("-nostdlib")
        .arg("-march=rv64g")
        .arg("-mabi=lp64")
        .arg("--target=riscv64")
        .arg("-mno-relax")
        .arg("-o")
        .arg(dest)
        .output()
        .expect("Error while generating assembly");
    println!("{}", String::from_utf8_lossy(&output.stderr));
}

//clang -Wl,-Ttext=0x0 -nostdlib -march=rv64i -mabi=lp64 -mno-relax -o source dest
fn generate_rv_obj(source: &str, dest: &str) {
    let cc = "clang";
    let output = Command::new(cc)
        .arg("-Wl,-Ttext=0x0")
        .arg("-nostdlib")
        .arg("-march=rv64g")
        .arg("-mabi=lp64")
        .arg("--target=riscv64")
        .arg("-mno-relax")
        .arg("-o")
        .arg(dest)
        .arg(source)
        .output()
        .expect("Error while generating ELF object");
    println!("{}", String::from_utf8_lossy(&output.stderr));
}

//llvm-objcopy -O binary source dest
fn generate_rv_bin(source: &str, dest: &str) {
    let objcopy = "llvm-objcopy";
    let output = Command::new(objcopy)
        .arg("-O")
        .arg("binary")
        .arg(source)
        .arg(dest)
        .output()
        .expect("Error while generating headless binary");
    println!("{}", String::from_utf8_lossy(&output.stderr));
}

// generate riscv binary from asm, run it for n_clocks
pub fn rv_asm_helper(code: &str, testname: &str, n_clock: i64) -> Result<Cpu, std::io::Error> {
    let asm_path = TEST_FOLDER.to_owned() + testname + ".s";
    let mut file = File::create(&asm_path)?;

    let binary_path = BINARY_FOLDER.to_owned() + testname;
    let final_path = BINARY_FOLDER.to_owned() + testname + ".bin";
    file.write(&code.as_bytes())?;
    generate_rv_obj(&asm_path, &binary_path);
    generate_rv_bin(&binary_path, &final_path);

    let mut file_bin = File::open(final_path)?;
    let mut code = Vec::new();
    file_bin.read_to_end(&mut code)?;
    run_cpu(code, vec![0], n_clock)
}

// generate riscv binary from C, run it for n_clocks
pub fn rv_c_helper(path: &str, testname: &str, n_clock: i64) -> Result<Cpu, std::io::Error> {
    let c_path = path;

    let asm_path = TEST_FOLDER.to_owned() + testname + ".s";
    generate_rv_assembly(c_path, &asm_path);

    let binary_path = BINARY_FOLDER.to_owned() + testname;
    let final_path = BINARY_FOLDER.to_owned() + testname + ".bin";

    generate_rv_obj(&asm_path, &binary_path);
    generate_rv_bin(&binary_path, &final_path);

    let mut file_bin = File::open(final_path)?;
    let mut code = Vec::new();
    file_bin.read_to_end(&mut code)?;
    run_cpu(code, vec![0], n_clock)
}

pub fn run_cpu(code: Vec<u8>, disk_image: Vec<u8>, n_clock: i64) -> Result<Cpu, std::io::Error> {
    let mut cpu = Cpu::new(code, disk_image);
    let mut n_clock = n_clock;

    while n_clock != 0 || n_clock == -1 {
        let inst = match cpu.fetch() {
            Ok(0) => break,
            //Ok(0xfee79ce3) => break,
            Ok(inst) => inst,
            Err(e) => {
                cpu.handle_exception(e);
                if e.is_fatal() {
                    println!("{}", e);
                    break;
                }
                continue;
            }
        };

        match cpu.execute(inst) {
            Ok(pc) => cpu.pc = pc,
            Err(e) => {
                cpu.handle_exception(e);
                if e.is_fatal() {
                    println!("{}", e);
                    break;
                }
            }
        }

        match cpu.check_pending_interrupt() {
            Some(interrupt) => cpu.handle_interrupt(interrupt),
            None => (),
        }

        if n_clock != -1 {
            n_clock -= 1;
        }
    }

    Ok(cpu)
}
