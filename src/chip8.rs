use std::{
    fs,
    path::PathBuf,
    sync::{mpsc::Receiver, Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use egui::Color32;
use pixels::Pixels;
use serde::{Deserialize, Serialize};
use winit::event_loop::EventLoopProxy;

use crate::{
    display_bus::{AppEvents, DebugState},
    io::InputState,
};

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
    runner: Chip8Runner,
    path: Option<PathBuf>,
    fps: u32,
}
impl EmulatorConfig {
    pub fn new(
        color: Color32,
        generation: Generation,
        debug: bool,
        path: Option<PathBuf>,
        fps: u32,
    ) -> EmulatorConfig {
        Self {
            color,
            generation,
            runner: Chip8Runner::new(debug),
            path,
            fps,
        }
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum EmulatorEvents {
    ChangeColor(Color32),
    FpsChange(u32),
    NextDebugCycle(usize),
    SetDebug(bool),
    QuitEmulator,
    DisplaySynced,
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
        let program = emulator_config
            .path
            .as_ref()
            .map(|path| fs::read(path).ok())
            .flatten()
            .unwrap_or(include_bytes!("../tetris.ch8").to_vec());
        hardware.load_program(&program);
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
    pub fn handle_event(&mut self) -> Quit {
        if let Ok(event) = self.event_bus.try_recv() {
            match event {
                EmulatorEvents::ChangeColor(c) => {
                    self.config.color = c;
                    if let Ok(mut pixels) = self.pixels.write() {
                        pixels
                            .frame_mut()
                            .chunks_exact_mut(4)
                            .filter(|c| c != &[0, 0, 0, 0])
                            .for_each(|c| c.copy_from_slice(&self.config.color.to_array()));
                    }
                }
                EmulatorEvents::NextDebugCycle(count) => {
                    if let Chip8RunnerKind::DebugRunner { cycles_to_run } =
                        &mut self.config.runner.kind
                    {
                        *cycles_to_run += count;
                    }
                }
                EmulatorEvents::QuitEmulator => return Quit::True,
                EmulatorEvents::DisplaySynced => self.hardware.display_sync = true,
                EmulatorEvents::FpsChange(fps) => self.config.fps = fps,
                EmulatorEvents::SetDebug(debug) => {
                    if debug && self.config.runner.is_debug() {
                        return Quit::False;
                    }
                    self.config.runner = Chip8Runner::new(debug);
                }
            }
        }
        Quit::False
    }
    fn send_debug_state(&self) {
        let instr = ((self.hardware.memory[self.hardware.pc as usize] as u16) << 8)
            | self.hardware.memory[self.hardware.pc as usize + 1] as u16;
        let debug_state = DebugState {
            pc: self.hardware.pc,
            i: self.hardware.i,
            reg: self.hardware.registers.clone(),
            op: instr,
        };
        self.display_bus
            .send_event(AppEvents::DebugEmulatorState(debug_state))
            .unwrap();
    }
    pub fn run(mut self) {
        loop {
            let now = Instant::now();
            let quit = self.handle_event();
            if matches!(quit, Quit::True) {
                return;
            }
            if self.runner().can_run() {
                self.config.runner.advance();
                if self.runner().is_debug() {
                    self.run_hardware_cycle();
                    if self.runner().hardware_clock_tick() {
                        self.hardware.tick_cpu_clock();
                    }
                    self.send_debug_state();
                } else {
                    let frame_time = Duration::from_secs_f32(1. / self.config.fps as f32);
                    self.run_hardware_cycle();
                    if self.runner().hardware_clock_tick() {
                        self.hardware.tick_cpu_clock();
                        let delta = frame_time.saturating_sub(now.elapsed());
                        thread::sleep(delta);
                    }
                }
            }
        }
    }
    fn runner(&self) -> &Chip8Runner {
        &self.config.runner
    }
}
pub struct Chip8Runner {
    kind: Chip8RunnerKind,
    cycles: u32,
}
impl Chip8Runner {
    pub fn new(debug: bool) -> Chip8Runner {
        Chip8Runner {
            kind: Chip8RunnerKind::new(debug),
            cycles: 0,
        }
    }
    pub fn is_debug(&self) -> bool {
        matches!(self.kind, Chip8RunnerKind::DebugRunner { cycles_to_run: _ })
    }
    pub fn hardware_clock_tick(&self) -> bool {
        let hardware_cycles_per_clock_tick = 18;
        self.cycles % hardware_cycles_per_clock_tick == 0
    }
    pub fn advance(&mut self) {
        self.kind.advance();
        self.cycles += 1;
    }
    pub fn can_run(&self) -> bool {
        self.kind.can_run()
    }
}
pub enum Chip8RunnerKind {
    DebugRunner { cycles_to_run: usize },
    NormalRunner,
}
impl Chip8RunnerKind {
    pub fn new(debug: bool) -> Chip8RunnerKind {
        if debug {
            Chip8RunnerKind::DebugRunner { cycles_to_run: 0 }
        } else {
            Chip8RunnerKind::NormalRunner
        }
    }
    pub fn advance(&mut self) {
        if let Chip8RunnerKind::DebugRunner { cycles_to_run } = self {
            *cycles_to_run -= 1;
        }
    }
    pub fn can_run(&self) -> bool {
        match self {
            Chip8RunnerKind::DebugRunner { cycles_to_run } => *cycles_to_run > 0,
            Chip8RunnerKind::NormalRunner => true,
        }
    }
}
pub enum Quit {
    True,
    False,
}
