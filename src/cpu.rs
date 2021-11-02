use std::cmp;
use std::convert::TryInto;
use std::iter::Iterator;

#[derive(Debug, PartialEq, Eq)]
pub struct Registers {
    v_regs: [u8; 16],
    i: u16,
    pc: u16,
    sp: u16, // this can be 8 bit?
}

#[derive(Debug)]
pub struct CPU {
    regs: Registers, 
    memory: Vec<u8>,
    // "pseudo registers"
    dt: u8, // delay timer
    st: u8, // sound timer,
    timedelta_error: u32, // the number of ms not used in the previous cycle(s)
    prng_val: u32,
}

fn bytes_to_nibbles<'a>(bytes: impl Iterator<Item = &'a u8>) -> Vec<u8> {
    let mut v = Vec::new();
    for b in bytes {
        v.push((b & 0xf0) >> 4);
        v.push(b & 0x0f);
    }
    v
}

pub fn nibbles_to_bytes<'a>(nibbles: impl Iterator<Item = &'a u8>) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::new();
    let mut first_half = true;

    for b in nibbles {
        if first_half {
            v.push(*b);
        } else {
            let idx = v.len() - 1;
            v[idx] = v[idx] << 4 | *b;
        }
        first_half = !first_half;
    }

    v
}

fn nibbles3_to_u16(insn_nibbles: Vec<u8>) -> u16 {
    u16::from_be_bytes(
        nibbles_to_bytes([0u8].iter().chain(insn_nibbles[1..].iter()))
    .try_into().unwrap())
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
            prng_val: 0x0badf00d
        }
    }

    pub fn go(&mut self, addr: u16) {
        self.regs.pc = addr;
    }

    pub fn get_pc(&self) -> u16 {
        self.regs.pc
    }

    pub fn push_word(&mut self, data: u16) {
        self.regs.sp -= 2;
        self.write_word(self.regs.sp, data);
    }

    pub fn pop_word(&mut self) -> u16 {
        let data =self.read_word(self.regs.sp);
        self.regs.sp += 2;
        data
    }

    pub fn write_byte(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }

    pub fn write_word(&mut self, addr: u16, data: u16) {
        let bytes = data.to_be_bytes();
        self.write_byte(addr, bytes[0]);
        self.write_byte(addr + 1, bytes[1]);
    }

    pub fn read_byte(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    pub fn read_word(&self, addr: u16) -> u16 {
        let bytes : [u8; 2] = self.memory[addr as usize..addr as usize+2].try_into().expect("unable to read at address");
        u16::from_be_bytes(bytes)
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
        self.regs.pc = self.pop_word();
    }

    fn do_call(&mut self, addr: u16) {
        // TODO: push to stack
        self.push_word(self.regs.pc + 2);
        self.regs.pc = addr;
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

    fn skip_cond(&mut self, cond: bool) {
        if cond {
            self.regs.pc += 2;
        }
    }

    fn random(&mut self) -> u8 {
        // pg.4, https://doi.org/10.18637%2Fjss.v008.i14
        // we just ignore the high 3 bytes as we only need a u8
        self.prng_val ^= self.prng_val << 13;
        self.prng_val ^= self.prng_val >> 17;
        self.prng_val ^= self.prng_val << 5;
        self.prng_val as u8
    }

    // store `nr` (must be <= 8) registers from `addr`
    fn store_regs(&mut self, addr: u16, nr: u8) {
        assert!(nr <= 8);

        for i in 0..nr {
            self.write_byte(addr + i as u16, self.regs.v_regs[i as usize]);
        }
    }

    // load `nr` (must be <= 8) registers from `addr`
    fn load_regs(&mut self, addr: u16, nr: u8) {
        assert!(nr <= 8);

        for i in 0..nr {
            self.regs.v_regs[i as usize] = self.read_byte(addr + i as u16);
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

        println!("executing: {:04x} (bytes: {:02x?}, nibbles: {:x?})", insn, insn_bytes, insn_nibbles);

        match insn {
            0x00e0 => self.clear_screen(),
            0x00ee => self.do_ret(),
            // JP (absolute jump)
            0x1000..=0x1fff => self.regs.pc = insn & 0xfff,
            // CALL (subroutine call)
            0x2000..=0x2fff => self.do_call(insn & 0xfff),
            // SE (skip if equal Vx immediate)
            0x3000..=0x3fff => self.skip_cond(self.regs.v_regs[insn_nibbles[1] as usize] == insn_bytes[1]),
            // SNE (skip if not equal Vx immediate)
            0x4000..=0x4fff => self.skip_cond(self.regs.v_regs[insn_nibbles[1] as usize] != insn_bytes[1]),
            // SE (skip if equal Vx Vy)
            0x5000..=0x5fff => self.skip_cond(self.regs.v_regs[insn_nibbles[1] as usize] == self.regs.v_regs[insn_nibbles[2] as usize]),
            0x6000..=0x6fff => self.regs.v_regs[insn_nibbles[1] as usize] = insn_bytes[1],
            0x7000..=0x7fff => self.regs.v_regs[insn_nibbles[1] as usize] = self.regs.v_regs[insn_nibbles[1] as usize] + insn_bytes[1],
            0x8000..=0x8fff => self.binary_reg_op(insn_nibbles[1], insn_nibbles[2], insn_nibbles[3]),           
            // register SNE (skip if not equal Vx Vy)
            0x9000..=0x9fff => self.skip_cond(self.regs.v_regs[insn_nibbles[1] as usize] != self.regs.v_regs[insn_nibbles[2] as usize]),
            // set I to immediate value
            0xa000..=0xafff => self.regs.i = nibbles3_to_u16(insn_nibbles),
            // jump to immediate offset by V0
            0xb000..=0xbfff => self.regs.pc = nibbles3_to_u16(insn_nibbles) + self.regs.v_regs[0] as u16,
            // generate a random byte and store its AND with immediate value in Vx
            0xc000..=0xcfff => self.regs.v_regs[insn_nibbles[1] as usize] = self.random() & insn_bytes[1],
            // Draw N-byte sprite
            // 0xd000..=0xdfff => ,
            // Skip if key Vx is pressed
            // 0xe09e..=0xef9e => ,
            // Skip if key Vx is not pressed
            // 0xe0a1..=0xefa1 => ,
            // Load Vx with the delay timer value
            0xf007..=0xff07 => self.regs.v_regs[insn_nibbles[1] as usize] = self.dt,
            // Wait for a keypress and store the key in Vx
            // 0xf00a..=0xff0a => ,
            // Set delay timer value to Vx
            0xf015..=0xff15 => self.dt = self.regs.v_regs[insn_nibbles[1] as usize],
            // Set sound timer to Vx
            0xf018..=0xff18 => self.st = self.regs.v_regs[insn_nibbles[1] as usize],
            // set I = I + vx
            0xf01e..=0xff1e => self.regs.i = self.regs.v_regs[insn_nibbles[1] as usize] as u16 + self.regs.i,
            // set I to the location of sprite for digit Vx
            // 0xf029..=0xff29 => ,
            // store the BCD representation of Vx in I..I+2
            // 0xf033..=0xff33 => 
            // store V0 through Vx from I..I+x
            0xf055..=0xff55 => self.store_regs(self.regs.i, insn_nibbles[1]),
            // load V0 through Vx from I..I+x
            0xf065..=0xff65 => self.load_regs(self.regs.i, insn_nibbles[1]),

            // Undefined opcode
            _ => panic!("undefined opcode"),
        }
    }
}
