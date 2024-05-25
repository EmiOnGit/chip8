pub mod emulator_view;
mod ui;

use std::io::{Read, Write};
use std::thread;

use crate::app::emulator_view::EmulatorViewMode;
use crate::chip8::screen::{self};
use crate::chip8::{Chip8, EmulatorConfig};
use crate::display_bus::AppEvents;
use crate::io::InputState;
use log::error;
use pixels::Error;
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
}
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
        Ok(App {
            input,
            event_loop,
            framework,
            window,
            emulator_view,
        })
    }
    pub fn run(self) -> Result<(), Error> {
        let App {
            mut input,
            event_loop,
            mut framework,
            window,
            mut emulator_view,
        } = self;
        let mut world = World::new();
        event_loop.run(move |event, _, control_flow| {
            // Handle input events
            if input.update(&event) {
                // Close events
                if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                world.input.update(&input);

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
                Event::UserEvent(display_event) => {
                    if let EmulatorViewMode::Host(host_view) = &mut emulator_view.mode {
                        if let Some(tcp) = &mut host_view.tcp {
                            let bytes = bincode::serialize(&display_event);
                            if let Ok(mut bytes) = bytes {
                                let mut buffer = bytes.len().to_be_bytes().to_vec();
                                buffer.append(&mut bytes);

                                tcp.write_all(&buffer).unwrap();
                            }
                            // println!("send mess");
                            // let mess = String::from("Hallo from server\n");
                            // tcp.write(&mess.into_bytes()).unwrap();
                            tcp.flush().unwrap();
                        }
                    }
                    match display_event {
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
                        }
                        AppEvents::SpawnEmulator { client } => {
                            let config = EmulatorConfig::new(framework.gui.color);
                            let event_bus = framework.gui.event_bus.clone();
                            spawn_emulator(&mut emulator_view, config, event_bus, client);
                        }
                        AppEvents::EmulatorEvent(event) => {
                            emulator_view.send(event);
                        }
                    }
                }
                _ => (),
            }
        });
    }
}
fn spawn_emulator(
    emulator_view: &mut EmulatorView,
    config: EmulatorConfig,
    event_bus: EventLoopProxy<AppEvents>,
    is_client: bool,
) {
    if is_client {
        let mut tcp = emulator_view.to_client();
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
    } else {
        let recv = emulator_view.to_host();
        let Some(recv) = recv else {
            return;
        };
        let pixel_buffer = emulator_view.clone_pixel_buffer();
        thread::spawn(move || {
            let chip8 = Chip8::new(event_bus, pixel_buffer, recv, config);
            chip8.run();
        });
    }
}
/// Representation of the application state. In this example, a box will bounce around the screen.
struct World {
    input: InputState,
}
impl World {
    /// Create a new `World` instance that can draw a moving box.
    fn new() -> Self {
        Self {
            input: InputState::default(),
        }
    }
}