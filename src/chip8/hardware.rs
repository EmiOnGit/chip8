use std::sync::{Arc, RwLock};

use pixels::Pixels;
use winit::event_loop::EventLoopProxy;

use crate::{
    chip8::screen::{SCREEN_HEIGHT, SCREEN_WIDTH},
    display_bus::AppEvents,
};

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
    memory: [u8; 4096],  // 4kb of RAM
    stack: [u16; 32],    // The stack offers a max depth of 32 with 2 bytes per stack frame
    stack_frame: i8,     // Current stack frame. Starts at -1 and is set to 0 on first use
    i: u16,              // Represents the 16-bit Index register
    registers: [u8; 16], // Represents the 16 registers
    pc: u16,             // Program counter, set it to the initial memory offset
    delay_timer: u8,     // Represents the delay timer that's decremented at 60hz if > 0
    sound_timer: u8,     // The sound timer that's decremented at 60hz and plays a beep if > 0
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
            stack_frame: -1,
            i: 0,
            registers: [0; 16],
            pc: 0x200,
            delay_timer: 0,
            sound_timer: 0,
        }
    }
}
impl Hardware {
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
    pub fn decode(
        &mut self,
        instr: u16,
        bus: &mut EventLoopProxy<AppEvents>,
        pixel_buffer: &Arc<RwLock<Pixels>>,
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
                self.pc = self.stack[self.stack_frame as usize];
                self.stack_frame -= 1;
            }
            // Jump
            (0x1, _, _, _) => self.pc = nnn,
            // Push subroutine
            (0x2, _, _, _) => {
                self.stack_frame += 1;
                self.stack[self.stack_frame as usize] = self.pc;
                self.pc = nnn;
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
            // Set index register I
            (0xa, _, _, _) => {
                self.i = nnn;
            }
            // display/draw
            (0xd, reg_x, reg_y, sprite_height) => {
                let x = self.registers[reg_x] % SCREEN_WIDTH as u8;
                let y = self.registers[reg_y] % SCREEN_HEIGHT as u8;
                // set flag register to 0
                self.registers[15] = 0;
                let i = self.i;
                let mut sprite: [u8; 16] = [0; 16];
                for n in 0..sprite_height {
                    let row_start = i + n as u16;
                    let row = self.memory[row_start as usize];
                    sprite[n as usize] = row;
                }
                let mut bytebuffer: Vec<u8> = Vec::new();
                if let Ok(pixel_buffer) = pixel_buffer.read() {
                    bytebuffer = pixel_buffer.frame().into_iter().map(|u| *u).collect();
                }
                let set_pixels: Vec<bool> = bytebuffer
                    .chunks_exact(4)
                    .map(|color| color != &[0, 0, 0, 0])
                    .collect();
                let first_pixel = x as usize + y as usize * SCREEN_WIDTH as usize;
                'check_flag: for (i_row, sprite_row) in sprite.into_iter().enumerate() {
                    let start = first_pixel + i_row * SCREEN_WIDTH as usize;
                    for b in 0..8 {
                        if set_pixels[start + b] && sprite_row & (1 << (7 - b)) != 0 {
                            self.registers[15] = 1;
                            break 'check_flag;
                        }
                    }
                }
                bus.send_event(AppEvents::DrawSprite { sprite, x, y })
                    .unwrap();
            }
            _ => {
                panic!()
            }
        }
    }
}
impl Hardware {
    fn pc(&self) -> usize {
        self.pc as usize
    }
}
