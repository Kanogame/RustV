use std::{
    fs::File,
    io::{Read, Write},
    process::Command,
    str::Bytes,
};

use crate::cpu::cpu::Cpu;
const test_folder: &str = "tests/";
const binary_folder: &str = "tests/target/";

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
pub fn rv_asm_helper(code: &str, testname: &str, n_clock: usize) -> Result<Cpu, std::io::Error> {
    let asm_path = test_folder.to_owned() + testname + ".s";
    let mut file = File::create(&asm_path)?;

    let binary_path = binary_folder.to_owned() + testname;
    let final_path = binary_folder.to_owned() + testname + ".bin";
    file.write(&code.as_bytes())?;
    generate_rv_obj(&asm_path, &binary_path);
    generate_rv_bin(&binary_path, &final_path);

    let mut file_bin = File::open(final_path)?;
    let mut code = Vec::new();
    file_bin.read_to_end(&mut code)?;
    run_cpu(code, n_clock)
}

// generate riscv binary from C, run it for n_clocks
pub fn rv_c_helper(path: &str, testname: &str, n_clock: usize) -> Result<Cpu, std::io::Error> {
    let c_path = path;

    let asm_path = test_folder.to_owned() + testname + ".s";
    generate_rv_assembly(c_path, &asm_path);

    let binary_path = binary_folder.to_owned() + testname;
    let final_path = binary_folder.to_owned() + testname + ".bin";

    generate_rv_obj(&asm_path, &binary_path);
    generate_rv_bin(&binary_path, &final_path);

    let mut file_bin = File::open(final_path)?;
    let mut code = Vec::new();
    file_bin.read_to_end(&mut code)?;
    run_cpu(code, n_clock)
}

fn run_cpu(code: Vec<u8>, n_clock: usize) -> Result<Cpu, std::io::Error> {
    let mut cpu = Cpu::new(code);

    for _ in 0..n_clock {
        let inst = match cpu.fetch() {
            Ok(inst) => inst,
            Err(er) => match er.value {
                0 => {
                    println!("program finished its execution and/or jumped to 0");
                    break;
                }
                _ => {
                    panic!("{}", er);
                }
            },
        };

        match cpu.execute(inst) {
            Ok(pc) => cpu.pc = pc,
            Err(er) => {
                println!("e{}", er);
                break;
            }
        }
    }

    Ok(cpu)
}
