use std::{
    sync::{mpsc::Receiver, Arc, RwLock},
    time::{Duration, Instant},
};

use egui::Color32;
use pixels::Pixels;
use serde::{Deserialize, Serialize};
use winit::event_loop::EventLoopProxy;

use crate::display_bus::AppEvents;

use self::hardware::Hardware;
pub mod hardware;
pub mod screen;

pub struct Chip8 {
    display_bus: EventLoopProxy<AppEvents>,
    pixels: Arc<RwLock<Pixels>>,
    device_timer: DeviceTimer,
    hardware: Hardware,
    event_bus: Receiver<EmulatorEvents>,
    config: EmulatorConfig,
}

pub struct EmulatorConfig {
    color: Color32,
}
impl EmulatorConfig {
    pub fn new(color: Color32) -> EmulatorConfig {
        Self { color }
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
        event_bus: Receiver<EmulatorEvents>,
        emulator_config: EmulatorConfig,
    ) -> Chip8 {
        let mut hardware = Hardware::default();
        hardware.load_program(include_bytes!("../IBM Logo.ch8"));
        Chip8 {
            event_bus,
            display_bus,
            device_timer: DeviceTimer::default(),
            pixels,
            hardware,
            config: emulator_config,
        }
    }
    pub fn run_hardware_cycle(&mut self) {
        let instr = self.hardware.fetch();
        self.hardware
            .decode(instr, &mut self.display_bus, &self.pixels);
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
        loop {
            self.handle_event();
            self.run_hardware_cycle();
            self.device_timer.next();
        }
    }
}

struct DeviceTimer {
    last: Instant,
    counter: usize,
}
impl DeviceTimer {
    const INSTR_PER_SEC: usize = 700;

    pub fn next(&mut self) {
        self.counter += 1;
        if self.counter >= DeviceTimer::INSTR_PER_SEC {
            let next = self.last + Duration::from_secs(1);
            let time_left = next - Instant::now();
            if time_left.as_secs_f32() < 0.001 {
                println!("CRITICAL: time_left {:?}", time_left);
            }
            std::thread::sleep(time_left);
            self.counter = 0;
            self.last = Instant::now();
        }
    }
}
impl Default for DeviceTimer {
    fn default() -> Self {
        DeviceTimer {
            last: Instant::now(),
            counter: 0,
        }
    }
}
