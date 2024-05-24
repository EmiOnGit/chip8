use std::time::{Duration, Instant};

use winit::event_loop::EventLoopProxy;

use crate::display_bus::DisplayEvent;
pub struct Chip8 {
    display_bus: EventLoopProxy<DisplayEvent>,
    device_timer: DeviceTimer,
}
impl Chip8 {
    pub fn new(display_bus: EventLoopProxy<DisplayEvent>) -> Chip8 {
        Chip8 {
            display_bus,
            device_timer: DeviceTimer::default(),
        }
    }
    pub fn run(mut self) {
        let mut x = 0;
        loop {
            if self.device_timer.counter == 0 {
                x += 1;
                self.display_bus
                    .send_event(DisplayEvent::SwapPixel(x, 1))
                    .unwrap();
            }

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
enum Op {
    None,
}
