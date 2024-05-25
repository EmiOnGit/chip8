use app::App;
use pixels::Error;
mod app;
mod chip8;
mod display_bus;
mod io;

fn main() -> Result<(), Error> {
    env_logger::init();
    let app = App::init()?;
    app.run()
}
