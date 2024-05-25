// use std::sync::mpsc::{self, Receiver};

use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct InputState {
    pub quit: bool,
    pub keys: u16,
}
pub const KEY_MAP: [VirtualKeyCode; 16] = [
    // row 1
    VirtualKeyCode::Key1,
    VirtualKeyCode::Key2,
    VirtualKeyCode::Key3,
    VirtualKeyCode::Key4,
    // row 2
    VirtualKeyCode::Q,
    VirtualKeyCode::W,
    VirtualKeyCode::E,
    VirtualKeyCode::R,
    // row 3
    VirtualKeyCode::A,
    VirtualKeyCode::S,
    VirtualKeyCode::D,
    VirtualKeyCode::F,
    // row 4
    VirtualKeyCode::Z,
    VirtualKeyCode::X,
    VirtualKeyCode::C,
    VirtualKeyCode::V,
];
impl InputState {
    pub fn update(&mut self, input: &WinitInputHelper) {
        for (i, key) in KEY_MAP.into_iter().enumerate() {
            if input.key_pressed(key) {
                self.keys = self.keys | (1 << i);
            }
            if input.key_released(key) {
                self.keys = self.keys & !(1 << i);
            }
        }
    }
}
