use std::{
    fs::File,
    io::{Read, Write},
    process::Command,
};

use crate::cpu::cpu::Cpu;
const test_folder: &str = "tests/";
const binary_folder: &str = "tests/target/";

//clang -S simple.c -nostdlib -march=rv64i -mabi=lp64 -mno-relax
//add support for folders
fn generate_rv_assembly(c_src: &str) {
    let cc = "clang";
    let output = Command::new(cc)
        .arg("-S")
        .arg(c_src)
        .arg("-nostdlib")
        .arg("-match=rv64g")
        .arg("-mabi=lp64")
        .arg("--target=riscv64")
        .arg("-mno-relax")
        .output()
        .expect("Error while generating assembly");
    println!("{}", String::from_utf8_lossy(&output.stderr));
}

//clang -Wl,-Ttext=0x0 -nostdlib -march=rv64i -mabi=lp64 -mno-relax -o simple simple.s
fn generate_rv_obj(testname: &str) {
    let cc = "clang";
    let output = Command::new(cc)
        .arg("-Wl,-Ttext=0x0")
        .arg("-nostdlib")
        .arg("-march=rv64g")
        .arg("-mabi=lp64")
        .arg("--target=riscv64")
        .arg("-mno-relax")
        .arg("-o")
        .arg(binary_folder.to_owned() + testname)
        .arg(test_folder.to_owned() + testname + ".s")
        .output()
        .expect("Error while generating ELF object");
    println!("{}", String::from_utf8_lossy(&output.stderr));
}

//llvm-objcopy -O binary simple simple.bin
fn generate_rv_bin(obj: &str) {
    let objcopy = "llvm-objcopy";
    let output = Command::new(objcopy)
        .arg("-O")
        .arg("binary")
        .arg(binary_folder.to_owned() + obj)
        .arg(binary_folder.to_owned() + obj + ".bin")
        .output()
        .expect("Error while generating headless binary");
    println!("{}", String::from_utf8_lossy(&output.stderr));
}

// generate riscv binary from code, run it for n_clocks
pub fn rv_helper(code: &str, testname: &str, n_clock: usize) -> Result<Cpu, std::io::Error> {
    let filename = test_folder.to_owned() + testname + ".s";
    let mut file = File::create(&filename)?;
    file.write(&code.as_bytes())?;
    generate_rv_obj(testname);
    generate_rv_bin(testname);

    let mut file_bin = File::open(binary_folder.to_owned() + testname + ".bin")?;
    let mut code = Vec::new();
    file_bin.read_to_end(&mut code)?;
    let mut cpu = Cpu::new(code);

    for _ in 0..n_clock {
        let inst = match cpu.fetch() {
            Ok(inst) => inst,
            Err(er) => {
                println!("f{}", er);
                break;
            }
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
