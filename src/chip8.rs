use std::{
    sync::{mpsc::Receiver, Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use egui::Color32;
use pixels::Pixels;
use serde::{Deserialize, Serialize};
use winit::event_loop::EventLoopProxy;

use crate::{display_bus::AppEvents, io::InputState};

use self::hardware::{Generation, Hardware};
pub mod hardware;
pub mod screen;

pub struct Chip8 {
    display_bus: EventLoopProxy<AppEvents>,
    pixels: Arc<RwLock<Pixels>>,
    input: Arc<RwLock<InputState>>,
    hardware: Hardware,
    event_bus: Receiver<EmulatorEvents>,
    config: EmulatorConfig,
}

pub struct EmulatorConfig {
    color: Color32,
    generation: Generation,
}
impl EmulatorConfig {
    pub fn new(color: Color32, generation: Generation) -> EmulatorConfig {
        Self { color, generation }
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum EmulatorEvents {
    ChangeColor(Color32),
}
impl Chip8 {
    pub fn new(
        display_bus: EventLoopProxy<AppEvents>,
        pixels: Arc<RwLock<Pixels>>,
        input: Arc<RwLock<InputState>>,
        event_bus: Receiver<EmulatorEvents>,
        emulator_config: EmulatorConfig,
    ) -> Chip8 {
        let mut hardware = Hardware::default();
        hardware.set_generation(emulator_config.generation);
        hardware.load_program(include_bytes!("../tetris.ch8"));
        // hardware.load_program(include_bytes!("../1dcell.ch8"));
        Chip8 {
            event_bus,
            display_bus,
            pixels,
            hardware,
            input,
            config: emulator_config,
        }
    }
    pub fn run_hardware_cycle(&mut self) {
        let instr = self.hardware.fetch();
        self.hardware
            .decode(instr, &mut self.display_bus, &self.pixels, &self.input);
    }
    pub fn handle_event(&mut self) {
        if let Ok(event) = self.event_bus.try_recv() {
            match event {
                EmulatorEvents::ChangeColor(c) => {
                    println!("got event");
                    self.config.color = c;
                    if let Ok(mut pixels) = self.pixels.write() {
                        pixels
                            .frame_mut()
                            .chunks_exact_mut(4)
                            .filter(|c| c != &[0, 0, 0, 0])
                            .for_each(|c| c.copy_from_slice(&self.config.color.to_array()));
                    }
                }
            }
        }
    }
    pub fn run(mut self) {
        let mut last_sec = Instant::now();
        let fps = 60;
        loop {
            for _ in 0..fps {
                self.handle_event();
                for _ in 0..15 {
                    self.run_hardware_cycle();
                }
                self.hardware.tick_cpu_clock();
            }
            thread::sleep(remaining_sec(last_sec));
            last_sec = Instant::now();
        }
    }
}
fn remaining_sec(instant: Instant) -> Duration {
    Duration::from_secs_f32(1.) - instant.elapsed()
}
