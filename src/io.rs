// use std::sync::mpsc::{self, Receiver};

use serde::{Deserialize, Serialize};
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Copy, Default)]
pub struct InputState {
    pub quit: bool,
    keys: u16,
    client: u16,
}
pub const KEY_MAP: [VirtualKeyCode; 16] = [
    VirtualKeyCode::X,
    VirtualKeyCode::Key1,
    VirtualKeyCode::Key2,
    VirtualKeyCode::Key3,
    VirtualKeyCode::Q,
    VirtualKeyCode::W,
    VirtualKeyCode::E,
    VirtualKeyCode::A,
    VirtualKeyCode::S,
    VirtualKeyCode::D,
    VirtualKeyCode::Z,
    VirtualKeyCode::C,
    VirtualKeyCode::Key4,
    VirtualKeyCode::R,
    VirtualKeyCode::F,
    VirtualKeyCode::V,
];
impl InputState {
    pub const fn pressed(self) -> u16 {
        self.keys | self.client
    }
    pub fn update(&mut self, input: &WinitInputHelper) {
        for (i, key) in KEY_MAP.into_iter().enumerate() {
            if input.key_pressed(key) {
                self.keys |= 1 << i;
            }
            if input.key_released(key) {
                self.keys &= !(1 << i);
            }
        }
    }
    pub fn set_client_keys(&mut self, other: u16) {
        self.client = other;
    }
}
