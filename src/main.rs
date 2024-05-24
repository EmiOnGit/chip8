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
    let display = AppDisplay::init()?;
    let display_bus = display.display_bus();
    std::thread::spawn(move || {
        let chip8 = Chip8::new(display_bus);
        chip8.run();
    });
    display.run()
}
