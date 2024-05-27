use std::sync::{Arc, RwLock};

use pixels::Pixels;
use serde::{Deserialize, Serialize};
use winit::event_loop::EventLoopProxy;

use crate::{
    chip8::screen::{SCREEN_HEIGHT, SCREEN_WIDTH},
    display_bus::AppEvents,
    io::InputState,
};

use super::screen;

const FONT: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];
pub struct Hardware {
    pub memory: [u8; 4096],         // 4kb of RAM
    stack: [u16; 32], // The stack offers a max depth of 32 with 2 bytes per stack frame
    stack_frame: i8,  // Current stack frame. Starts at -1 and is set to 0 on first use
    pub(crate) i: u16, // Represents the 16-bit Index register
    pub(crate) registers: [u8; 16], // Represents the 16 registers
    pub(crate) pc: u16, // Program counter, set it to the initial memory offset
    delay_timer: u8,  // Represents the delay timer that's decremented at 60hz if > 0
    sound_timer: u8,  // The sound timer that's decremented at 60hz and plays a beep if > 0
    generation: Generation,
    pub(crate) display_sync: bool,
}
#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Generation {
    COSMAC,
    #[default]
    Super,
}
impl Default for Hardware {
    fn default() -> Self {
        let mut memory = [0; 4096];
        for i in 0..FONT.len() {
            memory[i] = FONT[i];
        }
        Hardware {
            memory,
            stack: [0; 32],
            stack_frame: 0,
            i: 0,
            registers: [0; 16],
            pc: 0x200,
            delay_timer: 0,
            sound_timer: 0,
            generation: Generation::default(),
            display_sync: true,
        }
    }
}
impl Hardware {
    pub fn set_generation(&mut self, generation: Generation) {
        self.generation = generation;
    }
    pub fn load_program(&mut self, program: &[u8]) {
        let len = program.len();
        self.memory[0x200..0x200 + len].copy_from_slice(program);
    }
    pub fn fetch(&mut self) -> u16 {
        let instr = ((self.memory[self.pc()] as u16) << 8) | self.memory[self.pc() + 1] as u16;
        // convert the 2-bytes into a u16.
        self.pc += 2;
        instr
    }
    pub fn set_flag(&mut self, is_set: bool) {
        if is_set {
            self.registers[15] = 1;
        } else {
            self.registers[15] = 0;
        }
    }
    pub fn decode(
        &mut self,
        instr: u16,
        bus: &mut EventLoopProxy<AppEvents>,
        pixel_buffer: &Arc<RwLock<Pixels>>,
        input: &Arc<RwLock<InputState>>,
    ) {
        let b0 = (instr & 0xFF00) >> 8 as u8; // To get first byte, & the 8 leftmost bits which removes the 8 rightmost, then shift by 8 to the right to make the u8 conversion contain the bits originally on the left.
                                              // println!("instr: {instr:x}, pc: {pc:x}", pc = self.pc);
        let b1 = (instr & 0x00FF) as u8; // To get the second byte, just & the 8 rightmost bits, which removes the leftmost bits. The remaining bits are already at the rightmost position so no need to shift before converting to u8.

        let op = (b0 & 0xF0) >> 4 as u8; // first nibble, the instruction. Keep 4 leftmost bits, then shift them to the right-hand side.
        let x = (b0 & 0x0F) as usize; // second nibble, register lookup! Only keep rightmost bits.
        let y = ((b1 & 0xF0) >> 4) as usize; // third nibble, register lookup! Keep leftmost bits, shift 4 to left.
        let n = b1 & 0x0F; // fourth nibble, 4 bit number
        let nn = b1; // NN = second byte
        let nnn = (instr & 0x0FFF) as u16; // NNN = second, third and fourth nibbles, obtained by ANDing by b00001111 11111111 masking away the first nibble.
        match (op, x, y, n) {
            // Clear screen
            (0x0, 0x0, 0xe, 0x0) => bus.send_event(AppEvents::ClearScreen).unwrap(),
            // Return from subroutine
            (0x0, 0x0, 0xe, 0xe) => {
                self.stack_frame -= 1;
                self.pc = self.stack[self.stack_frame as usize];
            }
            // Jump
            (0x1, _, _, _) => self.pc = nnn,
            // Push subroutine
            (0x2, _, _, _) => {
                self.stack[self.stack_frame as usize] = self.pc;
                self.stack_frame += 1;
                self.pc = nnn;
            }
            (0x3, _, _, _) => {
                if self.registers[x] == nn {
                    self.pc += 2;
                }
            }
            (0x4, _, _, _) => {
                if self.registers[x] != nn {
                    self.pc += 2;
                }
            }
            (0x5, _, _, 0) => {
                if self.registers[x] == self.registers[y] {
                    self.pc += 2;
                }
            }
            // Set register
            (0x6, _, _, _) => {
                self.registers[x] = nn;
            }
            // Add value to register
            (0x7, _, _, _) => {
                let current = self.registers[x];
                self.registers[x] = current.wrapping_add(nn);
            }
            (0x8, _, _, 0) => {
                self.registers[x] = self.registers[y];
            }
            (0x8, _, _, 1) => {
                self.registers[x] = self.registers[x] | self.registers[y];
            }
            (0x8, _, _, 2) => {
                self.registers[x] = self.registers[x] & self.registers[y];
            }
            (0x8, _, _, 3) => {
                self.registers[x] = self.registers[x] ^ self.registers[y];
            }
            (0x8, _, _, 4) => {
                let overflow = self.registers[x].checked_add(self.registers[y]).is_none();
                self.registers[x] = self.registers[x].wrapping_add(self.registers[y]);
                self.set_flag(overflow);
            }
            (0x8, _, _, 5) => {
                let flag = self.registers[x] >= self.registers[y];
                self.registers[x] = self.registers[x].wrapping_sub(self.registers[y]);
                self.set_flag(flag);
            }
            (0x8, _, _, 6) => {
                match self.generation {
                    Generation::COSMAC => {
                        self.registers[x] = self.registers[y];
                    }
                    Generation::Super => {}
                }
                let flag = self.registers[x] & 1 == 1;
                self.registers[x] = self.registers[x] >> 1;
                self.set_flag(flag);
            }
            (0x8, _, _, 7) => {
                let flag = self.registers[x] <= self.registers[y];
                self.registers[x] = self.registers[y].wrapping_sub(self.registers[x]);
                self.set_flag(flag);
            }
            (0x8, _, _, 0xe) => {
                if matches!(self.generation, Generation::COSMAC) {
                    self.registers[x] = self.registers[y];
                }
                let flag = (self.registers[x] >> 7) == 1;
                self.registers[x] = self.registers[x] << 1;
                self.set_flag(flag);
            }

            (0x9, _, _, 0) => {
                if self.registers[x] != self.registers[y] {
                    self.pc += 2;
                }
            }
            // Set index register I
            (0xa, _, _, _) => {
                self.i = nnn;
            }
            (0xb, _, _, _) => match self.generation {
                Generation::COSMAC => self.pc = self.registers[0] as u16 + nnn,
                Generation::Super => {
                    self.pc = self.registers[x] as u16 + nnn;
                }
            },
            (0xc, _, _, _) => {
                let number = fastrand::u8(..);
                self.registers[x] = number & nn;
            }
            // display/draw
            (0xd, reg_x, reg_y, sprite_height) => {
                if !self.display_sync {
                    self.pc -= 2;
                    return;
                }
                self.display_sync = false;
                let x = self.registers[reg_x] % SCREEN_WIDTH as u8;
                let y = self.registers[reg_y] % SCREEN_HEIGHT as u8;
                // set flag register to 0
                let i = self.i;
                let mut sprite: [u8; 16] = [0; 16];
                for n in 0..sprite_height {
                    let row_start = i + n as u16;
                    let row = self.memory[row_start as usize];
                    sprite[n as usize] = row;
                }
                let mut flip = false;
                if let Ok(pixel_buffer) = pixel_buffer.read() {
                    bus.send_event(AppEvents::DrawSprite { sprite, x, y })
                        .unwrap();
                    for n in 0..16 {
                        let row_i = y as usize + n as usize;
                        let sprite_row = sprite[n as usize];
                        if sprite_row == 0 {
                            continue;
                        }
                        let screen_row = screen::pixel_row(&pixel_buffer, row_i);
                        flip = screen_row
                            .chunks_exact(4)
                            .skip(x as usize)
                            .take(8)
                            .enumerate()
                            .filter(|(i, _pixel)| sprite_row & (1 << (7 - i)) != 0)
                            .any(|(_i, c)| *c != [0, 0, 0, 0]);
                        if flip {
                            self.set_flag(true);
                            break;
                        }
                    }
                    if !flip {
                        self.set_flag(false);
                    }
                }
            }
            (0xe, _, 9, 0xe) => {
                let key = self.registers[x] % 16;
                if let Ok(input) = input.read() {
                    if input.keys & (1 << key) == 1 {
                        self.pc += 2;
                    }
                }
            }
            (0xe, _, 0xa, 1) => {
                let key = self.registers[x] % 16;
                if let Ok(input) = input.read() {
                    if input.keys & (1 << key) != 1 {
                        self.pc += 2;
                    }
                }
            }
            (0xf, _, 0, 7) => {
                self.registers[x] = self.delay_timer;
            }
            (0xf, _, 1, 5) => {
                self.delay_timer = self.registers[x];
            }
            (0xf, _, 1, 8) => {
                self.sound_timer = self.registers[x];
            }
            (0xf, _, 1, 0xe) => self.i = self.i.wrapping_add(self.registers[x] as u16),
            (0xf, _, 0, 0xa) => {
                if let Ok(input) = input.try_read() {
                    // if any key is pressed
                    if input.keys != 0 {
                        self.registers[x] = input.keys.leading_zeros() as u8;
                    } else {
                        self.pc -= 2;
                    }
                }
            }
            (0xf, _, 2, 9) => {
                let char = self.registers[x];
                // each char is 5 bytes
                self.i = 5 * char as u16;
            }
            (0xf, _, 3, 3) => {
                let number = self.registers[x];
                self.memory[self.i as usize] = number / 100;
                self.memory[self.i as usize + 1] = (number % 100) / 10;
                self.memory[self.i as usize + 2] = number % 10;
            }
            (0xf, _, 5, 5) => {
                for i in 0..=x {
                    self.memory[self.i as usize + i] = self.registers[i];
                }
                if matches!(self.generation, Generation::COSMAC) {
                    self.i = self.i.wrapping_add(x as u16 + 1)
                }
            }
            (0xf, _, 6, 5) => {
                for i in 0..=x {
                    self.registers[i] = self.memory[self.i as usize + i];
                }
                if matches!(self.generation, Generation::COSMAC) {
                    self.i = self.i.wrapping_add(x as u16 + 1)
                }
            }

            _ => {
                panic!()
            }
        }
    }

    pub fn tick_cpu_clock(&mut self) {
        self.delay_timer = self.delay_timer.saturating_sub(1);
        self.sound_timer = self.sound_timer.saturating_sub(1);
    }
}
impl Hardware {
    fn pc(&self) -> usize {
        self.pc as usize
    }
}
