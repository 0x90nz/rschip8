use std::cmp;
use std::convert::TryInto;
use std::iter::Iterator;

pub struct Registers {
    v_regs: [u8; 16],
    i: u16,
    pc: u16,
    sp: u16, // this can be 8 bit?
}

pub struct CPU {
    regs: Registers, 
    memory: Vec<u8>,
    // "pseudo registers"
    dt: u8, // delay timer
    st: u8, // sound timer,
    timedelta_error: u32, // the number of ms not used in the previous cycle(s)
}

fn bytes_to_nibbles<'a>(bytes: impl Iterator<Item = &'a u8>) -> Vec<u8> {
    let mut v = Vec::new();
    for b in bytes {
        v.push((b & 0xf0) >> 4);
        v.push(b & 0x0f);
    }
    v
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            regs: Registers {
                v_regs: [0; 16],
                i: 0,
                pc: 0,
                sp: 0x200, // end of interpreter space,
            },
            memory: vec![0; 4096], // spoilt! a whole 4k!
            dt: 0,
            st: 0,
            timedelta_error: 0,
        }
    }

    pub fn write_byte(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    pub fn read_word(&self, addr: u16) -> u16 {
        let bytes : [u8; 2] = self.memory[addr as usize..addr as usize+2].try_into().expect("unable to read at address");
        u16::from_le_bytes(bytes)
    }

    pub fn write_bytes(&mut self, addr: u16, data: &Vec<u8>) {
        self.memory.splice(addr as usize..addr as usize+data.len(), data.iter().cloned());
    }

    pub fn read_bytes(&mut self, addr: u16, size: usize) -> Vec<u8> {
        let mut v : Vec<u8> = Vec::new();
        v.clone_from_slice(&self.memory[addr as usize .. addr as usize + size]);
        v
    }

    pub fn read_memory(&self) -> Vec<u8> {
        self.memory.clone()
    }

    fn do_ret(&mut self) {

    }

    fn do_call(&mut self, addr: u16) {
        // TODO: push to stack
        self.regs.pc = addr
    }

    fn clear_screen(&mut self) {

    }

    fn set_vf(&mut self) {
        self.regs.v_regs[0xf] = 1;
    }

    fn set_vf_cond(&mut self, cond: bool) {
        self.regs.v_regs[0xf] = if cond { 1 } else { 0 }
    }

    fn clear_vf(&mut self) {
        self.regs.v_regs[0xf] = 0;
    }

    fn binary_reg_op(&mut self, dest: u8, src: u8, op: u8) {
        let x = self.regs.v_regs[dest as usize];
        let y = self.regs.v_regs[src as usize];
        self.regs.v_regs[dest as usize] = match op {
            // LD
            0x0 => y,
            // OR
            0x1 => x | y,
            // AND
            0x2 => x & y,
            // XOR
            0x3 => x ^ y,
            // ADD with carry
            0x4 => { self.set_vf_cond(x as usize + y as usize > 255); x + y },
            // SUB with borrow
            0x5 => { self.set_vf_cond(x > y); x - y },
            // SHR, sets VF to LSB of Vx
            0x6 => { self.regs.v_regs[0xf] = x & 1; x >> 1 },
            // SUB with NOT borrow
            0x7 => { self.set_vf_cond(y > x); y - x },
            // SHL, sets VF to MSB of Vx
            0xe => { self.regs.v_regs[0xf] = x >> 7; x << 1 },
            _ => 0, // TODO better?
        }
    }

    pub fn clock(&mut self, ms_delta: u32) {
        // decrement timers
        let timer_ticks = (ms_delta + self.timedelta_error) / 16;
        self.timedelta_error = (ms_delta + self.timedelta_error) % 16;
        self.st = cmp::min(0, self.st - timer_ticks as u8);
        self.dt = cmp::min(0, self.dt - timer_ticks as u8);

        let insn = self.read_word(self.regs.pc);
        let insn_bytes = insn.to_be_bytes();
        let insn_nibbles = bytes_to_nibbles(insn_bytes.iter());
        self.regs.pc += 2;

        match insn {
            0x00e0 => self.clear_screen(),
            0x00ee => self.do_ret(),
            // JP (absolute jump)
            0x1000..=0x1fff => self.regs.pc = insn & 0xfff,
            // CALL (subroutine call)
            0x2000..=0x2fff => self.do_call(insn & 0xfff),
            // SE (skip if equal)
            0x3000..=0x3fff => if self.regs.v_regs[insn_nibbles[1] as usize] == insn_bytes[1] { self.regs.pc += 2 },
            // SNE (skip if not equal)
            0x4000..=0x4fff => if self.regs.v_regs[insn_nibbles[1] as usize] != insn_bytes[1] { self.regs.pc += 2 },
            0x5000..=0x5fff => if self.regs.v_regs[insn_nibbles[1] as usize] == self.regs.v_regs[insn_nibbles[2] as usize] { self.regs.pc += 2 },
            0x6000..=0x6fff => self.regs.v_regs[insn_nibbles[1] as usize] = insn_bytes[1],
            0x7000..=0x7fff => self.regs.v_regs[insn_nibbles[1] as usize] = self.regs.v_regs[insn_nibbles[1] as usize] + insn_bytes[1],
            0x8000..=0x8fff => self.binary_reg_op(insn_nibbles[1], insn_nibbles[2], insn_nibbles[3]),           
            
            // Undefined opcode
            _ => (),
        }
    }
}
