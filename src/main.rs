pub mod cpu;
use pretty_hex::*;

use crate::cpu::nibbles_to_bytes;

fn main() {
    // let mut cpu = cpu::CPU::new();
    // let font = include_bytes!("rom.bin");
    // cpu.write_bytes(0, &font.to_vec());

    let mut cpu = cpu::CPU::new();
    cpu.go(0x200);

    let instructions: Vec<u8> = [
        0xa1, 0x23,     // set i to 0x123
    ].to_vec();
    cpu.write_bytes(0x200, &instructions);

    cpu.clock(1);

    println!("PC ended on {}", cpu.get_pc());
    // println!("CPU: {:#x?}", cpu.regs);
    // println!("{}", pretty_hex(&cpu.read_memory()));
}
