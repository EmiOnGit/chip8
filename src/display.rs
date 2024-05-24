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

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
const BOX_SIZE: i16 = 64;

pub struct AppDisplay {
    input: WinitInputHelper,
    event_loop: EventLoop<DisplayEvent>,
    framework: Framework,
    pixels: Pixels,
    window: winit::window::Window,
}
impl AppDisplay {
    pub fn display_bus(&self) -> EventLoopProxy<DisplayEvent> {
        self.event_loop.create_proxy()
    }
    pub fn init() -> Result<AppDisplay, Error> {
        let input = WinitInputHelper::new();
        let event_loop = EventLoopBuilder::<DisplayEvent>::default().build();

        let window = {
            let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
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
            let pixels = Pixels::new(WIDTH, HEIGHT, surface_texture)?;
            let framework = Framework::new(
                &event_loop,
                window_size.width,
                window_size.height,
                scale_factor,
                &pixels,
            );

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
            mut pixels,
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
                    if let Err(err) = pixels.resize_surface(size.width, size.height) {
                        error!(target: "pixels.resize_surface", "{err}");
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    framework.resize(size.width, size.height);
                }

                // Update internal state and request a redraw
                world.update();
                window.request_redraw();
            }

            match event {
                Event::WindowEvent { event, .. } => {
                    // Update egui inputs
                    framework.handle_event(&event);
                }
                // Draw the current frame
                Event::RedrawRequested(_) => {
                    // Draw the world
                    world.draw(pixels.frame_mut());

                    // Prepare egui
                    framework.prepare(&window);

                    // Render everything together
                    let render_result = pixels.render_with(|encoder, render_target, context| {
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
                }
                Event::UserEvent(display_event) => println!("recv event: {display_event:?}"),
                _ => (),
            }
        });
    }
}
/// Representation of the application state. In this example, a box will bounce around the screen.
struct World {
    input: InputState,
    box_x: i16,
    box_y: i16,
    velocity_x: i16,
    velocity_y: i16,
}
impl World {
    /// Create a new `World` instance that can draw a moving box.
    fn new() -> Self {
        Self {
            input: InputState::default(),
            box_x: 24,
            box_y: 16,
            velocity_x: 1,
            velocity_y: 1,
        }
    }

    /// Update the `World` internal state; bounce the box around the screen.
    fn update(&mut self) {
        if self.box_x <= 0 || self.box_x + BOX_SIZE > WIDTH as i16 {
            self.velocity_x *= -1;
        }
        if self.box_y <= 0 || self.box_y + BOX_SIZE > HEIGHT as i16 {
            self.velocity_y *= -1;
        }

        self.box_x += self.velocity_x;
        self.box_y += self.velocity_y;
    }

    /// Draw the `World` state to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as i16;
            let y = (i / WIDTH as usize) as i16;

            let inside_the_box = x >= self.box_x
                && x < self.box_x + BOX_SIZE
                && y >= self.box_y
                && y < self.box_y + BOX_SIZE;

            let rgba = if inside_the_box {
                [0x5e, 0x48, 0xe8, 0xff]
            } else {
                [0x48, 0xb2, 0xe8, 0xff]
            };

            pixel.copy_from_slice(&rgba);
        }
    }
}
