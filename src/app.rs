pub mod emulator_view;
mod ui;

use std::fmt::Display;
use std::io::Read;
use std::sync::{Arc, RwLock};
use std::thread;

use crate::app::emulator_view::EmulatorViewMode;
use crate::chip8::screen::{self};
use crate::chip8::{Chip8, EmulatorConfig, EmulatorEvents};
use crate::display_bus::AppEvents;
use crate::io::InputState;
use log::error;
use pixels::Error;
use serde::{Deserialize, Serialize};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

use self::emulator_view::EmulatorView;
use self::ui::Framework;

pub struct App {
    input: WinitInputHelper,
    event_loop: EventLoop<AppEvents>,
    framework: Framework,
    emulator_view: EmulatorView,
    window: winit::window::Window,
    input_state: InputStateRef,
}
pub type InputStateRef = Arc<RwLock<InputState>>;
impl App {
    pub fn _display_bus(&self) -> EventLoopProxy<AppEvents> {
        self.event_loop.create_proxy()
    }
    pub fn init() -> Result<App, Error> {
        let input = WinitInputHelper::new();
        let event_loop = EventLoopBuilder::<AppEvents>::default().build();

        let window = {
            let size = LogicalSize::new(screen::SCREEN_WIDTH as f64, screen::SCREEN_HEIGHT as f64);
            WindowBuilder::new()
                .with_title("Chip8")
                .with_inner_size(size)
                .with_min_inner_size(size)
                .build(&event_loop)
                .unwrap()
        };
        let emulator_view = EmulatorView::new(&window)?;
        let framework = {
            let window_size = window.inner_size();
            let scale_factor = window.scale_factor() as f32;
            let framework = Framework::new(
                &event_loop,
                window_size.width,
                window_size.height,
                scale_factor,
                &emulator_view,
            );

            framework
        };
        let input_state = Arc::new(RwLock::new(InputState::default()));
        Ok(App {
            input,
            event_loop,
            framework,
            window,
            emulator_view,
            input_state,
        })
    }
    pub fn run(self) -> Result<(), Error> {
        let App {
            mut input,
            event_loop,
            mut framework,
            window,
            mut emulator_view,
            input_state,
        } = self;
        event_loop.run(move |event, _, control_flow| {
            // Handle input events
            if input.update(&event) {
                // Close events
                if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                if let Ok(mut input_state) = input_state.write() {
                    input_state.update(&input);
                }

                // Update the scale factor
                if let Some(scale_factor) = input.scale_factor() {
                    framework.scale_factor(scale_factor);
                }

                // Resize the window
                if let Some(size) = input.window_resized() {
                    emulator_view.on_pixels_mut(|pixels| {
                        if let Err(err) = pixels.resize_surface(size.width, size.height) {
                            error!(target: "pixels.resize_surface", "{err}");
                            *control_flow = ControlFlow::Exit;
                            return;
                        }
                    });
                    framework.resize(size.width, size.height);
                }

                window.request_redraw();
            }

            match event {
                Event::WindowEvent { event, .. } => {
                    // Update egui inputs
                    framework.handle_event(&event);
                }
                // Draw the current frame
                Event::RedrawRequested(_) => {
                    // Prepare egui
                    framework.prepare(&window);
                    emulator_view.on_pixels(|pixels| {
                        let render_result =
                            pixels.render_with(|encoder, render_target, context| {
                                // Render the world texture
                                context.scaling_renderer.render(encoder, render_target);

                                // Render egui
                                framework.render(encoder, render_target, context);

                                Ok(())
                            });

                        // Basic error handling
                        if let Err(err) = render_result {
                            error!(target: "pixels.render", "{err}");
                            *control_flow = ControlFlow::Exit;
                        }
                    });
                }
                Event::UserEvent(app_event) => {
                    if let EmulatorViewMode::Host(host_view) = &mut emulator_view.mode {
                        host_view.send_over_tcp(&app_event);
                    }
                    match app_event {
                        AppEvents::Nop => println!("nop"),
                        AppEvents::ClearScreen => {
                            emulator_view.on_pixels_mut(|pixels| {
                                pixels.frame_mut().fill(0);
                            });
                        }

                        AppEvents::DrawSprite { sprite, x, y } => {
                            emulator_view.on_pixels_mut(|pixels| {
                                let color = framework.gui.color.to_array();
                                for y_delta in 0..16 {
                                    screen::set_row(
                                        pixels,
                                        x as usize,
                                        y as usize + y_delta,
                                        sprite[y_delta],
                                        color,
                                    );
                                }
                            });
                            emulator_view.send(EmulatorEvents::DisplaySynced);
                        }
                        AppEvents::SpawnEmulator {
                            kind,
                            generation,
                            debugger,
                            path,
                            fps,
                        } => {
                            let config = EmulatorConfig::new(
                                framework.gui.color,
                                generation,
                                debugger,
                                path,
                                fps,
                            );
                            let event_bus = framework.gui.event_bus.clone();
                            spawn_emulator(
                                &mut emulator_view,
                                config,
                                Arc::clone(&input_state),
                                event_bus,
                                kind,
                            );
                        }
                        AppEvents::EmulatorEvent(event) => {
                            emulator_view.send(event);
                        }
                        AppEvents::DebugEmulatorState(state) => {
                            framework.gui.update_debugger(state);
                        }
                    }
                }
                _ => (),
            }
        });
    }
}
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub enum HostIp {
    Empty,
    NotFound,
    Ip(String),
}
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub enum EmulatorKind {
    Single,
    Server { ip: HostIp },
    Client { host_ip: String },
}
impl Display for EmulatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmulatorKind::Single => write!(f, "Singleplayer"),
            EmulatorKind::Server { ip: _ } => write!(f, "Server"),
            EmulatorKind::Client { host_ip: _ } => write!(f, "Client"),
        }
    }
}
fn spawn_emulator(
    emulator_view: &mut EmulatorView,
    config: EmulatorConfig,
    input_state: InputStateRef,
    event_bus: EventLoopProxy<AppEvents>,
    kind: EmulatorKind,
) {
    let pixels = emulator_view.clone_pixel_buffer();
    // we close all emulators that may already be running
    emulator_view.send(EmulatorEvents::QuitEmulator);
    event_bus.send_event(AppEvents::ClearScreen).unwrap();
    match kind {
        EmulatorKind::Single => {
            let (view, recv) = EmulatorView::single(Arc::clone(&pixels));
            *emulator_view = view;
            thread::spawn(move || {
                let chip8 = Chip8::new(event_bus, pixels, input_state, recv, config);
                chip8.run();
            });
        }
        EmulatorKind::Server { ip } => {
            let (view, recv) = EmulatorView::host(Arc::clone(&pixels));
            *emulator_view = view;
            thread::spawn(move || {
                let chip8 = Chip8::new(event_bus, pixels, input_state, recv, config);
                chip8.run();
            });
        }
        EmulatorKind::Client { host_ip } => {
            let (client, mut tcp) = EmulatorView::client(pixels);
            *emulator_view = client;
            thread::spawn(move || loop {
                let mut length_bytes = 0usize.to_be_bytes();
                if let Err(e) = tcp.read_exact(&mut length_bytes) {
                    println!("failed reading with: {e}");
                    return;
                };
                let length = usize::from_be_bytes(length_bytes);
                let mut message = vec![0; length];
                if let Err(e) = tcp.read_exact(&mut message) {
                    println!("failed reading with: {e}");
                    return;
                };
                let message: AppEvents = bincode::deserialize(&message).unwrap();
                event_bus.send_event(message).unwrap();
            });
        }
    }
}
pub fn fetch_global_ip() -> Option<String> {
    let mut ip = String::new();
    let _resp = reqwest::blocking::get("https://api6.ipify.org")
        .ok()?
        .read_to_string(&mut ip)
        .ok()?;
    println!("fetch");
    Some(ip)
}
