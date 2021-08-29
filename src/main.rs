pub mod cpu;
use pretty_hex::*;

fn main() {
    let mut cpu = cpu::CPU::new();
    let font = include_bytes!("rom.bin");
    cpu.write_bytes(0, &font.to_vec());

    println!("{}", pretty_hex(&cpu.read_memory()));
}
