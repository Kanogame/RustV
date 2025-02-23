use std::{
    env,
    fs::File,
    io::{self, Read},
};

use cpu::{cpu::Cpu, test_framework::run_cpu};

mod bus;
mod cpu;
mod csr;
mod device;
mod dram;
mod exept;
mod interrupt;
mod param;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("pass the filename");

        return Ok(());
    }

    let mut file = File::open(&args[1])?;
    let mut code = Vec::new();
    file.read_to_end(&mut code)?;

    let mut disk_image = Vec::new();
    if args.len() == 3 {
        let mut file = File::open(&args[2])?;
        file.read_to_end(&mut disk_image)?;
    }

    run_cpu(code, disk_image, -1);
    Ok(())
}
