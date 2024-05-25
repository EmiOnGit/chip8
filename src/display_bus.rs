use serde::{Deserialize, Serialize};

use crate::chip8::EmulatorEvents;

#[derive(Default, Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum AppEvents {
    #[default]
    Nop,
    EmulatorEvent(EmulatorEvents),
    ClearScreen,
    DrawSprite {
        sprite: [u8; 16],
        x: u8,
        y: u8,
    },
    SpawnEmulator {
        client: bool,
    },
}
