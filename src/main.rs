use std::{
    env,
    fs::File,
    io::{self, Read},
};

use cpu::cpu::Cpu;

mod bus;
mod cpu;
mod dram;
mod exept;
mod param;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("pass the filename");

        return Ok(());
    }

    let mut file = File::open(&args[1])?;
    let mut code = Vec::new();
    file.read_to_end(&mut code)?;

    let mut cpu = Cpu::new(code);

    loop {
        let inst = match cpu.fetch() {
            Ok(inst) => inst,
            Err(e) => {
                panic!("{}", e);
                break;
            }
        };

        match cpu.execute(inst) {
            Ok(next_pc) => cpu.pc = next_pc,
            Err(e) => {
                panic!("{}", e);
                break;
            }
        };
    }

    cpu.dump_registers();

    Ok(())
}
