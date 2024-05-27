use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    app::EmulatorKind,
    chip8::{hardware::Generation, EmulatorEvents},
};

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
        kind: EmulatorKind,
        generation: Generation,
        debugger: bool,
        path: Option<PathBuf>,
        fps: u32,
    },
    DebugEmulatorState(DebugState),
    ClientMessage(ClientMessage),
}
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    KeyInput(u16),
}
#[derive(Default, Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct DebugState {
    pub pc: u16,
    pub i: u16,
    pub reg: [u8; 16],
    pub op: u16,
}
