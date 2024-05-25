use std::{
    sync::{
        mpsc::{self, Receiver},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use pixels::Pixels;
use winit::event_loop::EventLoopProxy;

use crate::display_bus::DisplayEvent;

use self::hardware::Hardware;
pub mod hardware;
pub mod screen;

pub struct Chip8 {
    display_bus: EventLoopProxy<DisplayEvent>,
    pixels: Arc<RwLock<Pixels>>,
    device_timer: DeviceTimer,
    hardware: Hardware,
    event_bus: Receiver<Events>,
}
pub enum Events {}
impl Chip8 {
    pub fn new(
        display_bus: EventLoopProxy<DisplayEvent>,
        pixels: Arc<RwLock<Pixels>>,
        event_bus: Receiver<Events>,
    ) -> Chip8 {
        let (r, w) = mpsc::channel::<Events>();
        let mut hardware = Hardware::default();
        hardware.load_program(include_bytes!("../IBM Logo.ch8"));
        Chip8 {
            event_bus,
            display_bus,
            device_timer: DeviceTimer::default(),
            pixels,
            hardware,
        }
    }
    pub fn run(mut self) {
        loop {
            let instr = self.hardware.fetch();
            self.hardware
                .decode(instr, &mut self.display_bus, &self.pixels);

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
