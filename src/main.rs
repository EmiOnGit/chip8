use chip8::Chip8;
use display::AppDisplay;
use pixels::Error;
mod chip8;
mod display;
mod display_bus;
mod io;
mod ui;

fn main() -> Result<(), Error> {
    env_logger::init();
    let (display, recv) = AppDisplay::init()?;
    let display_bus = display.display_bus();
    let pixel_buffer = display.pixel_buffer();
    std::thread::spawn(move || {
        let chip8 = Chip8::new(display_bus, pixel_buffer, recv);
        chip8.run();
    });
    display.run()
}
