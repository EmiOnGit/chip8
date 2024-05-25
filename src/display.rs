use std::sync::{Arc, RwLock};

use crate::chip8::screen::{self};
use crate::display_bus::DisplayEvent;
use crate::io::InputState;
use crate::ui::Framework;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

pub struct AppDisplay {
    input: WinitInputHelper,
    event_loop: EventLoop<DisplayEvent>,
    framework: Framework,
    pixels: Arc<RwLock<Pixels>>,
    window: winit::window::Window,
}
impl AppDisplay {
    pub fn pixel_buffer(&self) -> Arc<RwLock<Pixels>> {
        Arc::clone(&self.pixels)
    }
    pub fn display_bus(&self) -> EventLoopProxy<DisplayEvent> {
        self.event_loop.create_proxy()
    }
    pub fn init() -> Result<AppDisplay, Error> {
        let input = WinitInputHelper::new();
        let event_loop = EventLoopBuilder::<DisplayEvent>::default().build();

        let window = {
            let size = LogicalSize::new(screen::SCREEN_WIDTH as f64, screen::SCREEN_HEIGHT as f64);
            WindowBuilder::new()
                .with_title("Chip8")
                .with_inner_size(size)
                .with_min_inner_size(size)
                .build(&event_loop)
                .unwrap()
        };
        let (pixels, framework) = {
            let window_size = window.inner_size();
            let scale_factor = window.scale_factor() as f32;
            let surface_texture =
                SurfaceTexture::new(window_size.width, window_size.height, &window);
            let pixels = Pixels::new(
                screen::SCREEN_WIDTH as u32,
                screen::SCREEN_HEIGHT as u32,
                surface_texture,
            )?;
            let framework = Framework::new(
                &event_loop,
                window_size.width,
                window_size.height,
                scale_factor,
                &pixels,
            );
            let pixels = Arc::new(RwLock::new(pixels));

            (pixels, framework)
        };
        Ok(AppDisplay {
            input,
            event_loop,
            framework,
            pixels,
            window,
        })
    }
    pub fn run(self) -> Result<(), Error> {
        let AppDisplay {
            mut input,
            event_loop,
            mut framework,
            pixels,
            window,
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
                    if let Ok(mut pixels) = pixels.write() {
                        if let Err(err) = pixels.resize_surface(size.width, size.height) {
                            error!(target: "pixels.resize_surface", "{err}");
                            *control_flow = ControlFlow::Exit;
                            return;
                        }
                    }
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
                    if let Ok(pixels) = pixels.read() {
                        // Render everything together
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
                    };
                }
                Event::UserEvent(display_event) => match display_event {
                    DisplayEvent::Nop => println!("nop"),
                    DisplayEvent::ClearScreen => {
                        let Ok(mut pixels) = pixels.write() else {
                            return;
                        };
                        pixels.frame_mut().fill(0);
                    }

                    DisplayEvent::DrawSprite { sprite, x, y } => {
                        let Ok(mut pixels) = pixels.write() else {
                            return;
                        };
                        let color = [255, 111, 21, 199];
                        for y_delta in 0..16 {
                            screen::set_row(
                                &mut pixels,
                                x as usize,
                                y as usize + y_delta,
                                sprite[y_delta],
                                color,
                            );
                        }
                    }
                },
                _ => (),
            }
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
