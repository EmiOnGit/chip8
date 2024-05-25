use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, RwLock,
    },
    thread,
    time::Duration,
};

use pixels::{Pixels, SurfaceTexture};
use winit::window::Window;

use crate::chip8::{screen, EmulatorEvents};

pub enum EmulatorViewMode {
    Host(HostView),
    Client(ClientView),
    OffView(OffView),
}
const ADDR: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 4214));
pub type PixelRef = Arc<RwLock<Pixels>>;
pub struct EmulatorView {
    pixels: PixelRef,
    pub mode: EmulatorViewMode,
}
impl EmulatorView {
    pub fn send(&self, event: EmulatorEvents) {
        match &self.mode {
            EmulatorViewMode::Host(host) => host.sender.send(event).unwrap(),
            EmulatorViewMode::Client(_) => {}
            EmulatorViewMode::OffView(_) => {}
        }
    }
    pub fn new(window: &Window) -> Result<Self, pixels::Error> {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new(
            screen::SCREEN_WIDTH as u32,
            screen::SCREEN_HEIGHT as u32,
            surface_texture,
        )?;
        Ok(EmulatorView {
            pixels: Arc::new(RwLock::new(pixels)),
            mode: EmulatorViewMode::OffView(OffView {}),
        })
    }
    pub fn to_client(&mut self) -> TcpStream {
        match self.mode {
            EmulatorViewMode::Host(_) => panic!(),
            EmulatorViewMode::Client(_) => panic!("already client"),
            EmulatorViewMode::OffView(_) => {
                let connection = TcpStream::connect(ADDR).unwrap();
                println!("CLIENT connected with {connection:?}");
                self.mode = EmulatorViewMode::Client(ClientView);
                thread::sleep(Duration::from_secs_f32(0.05));
                connection
            }
        }
    }
    pub fn to_host(&mut self) -> Option<Receiver<EmulatorEvents>> {
        match self.mode {
            EmulatorViewMode::Host(_) => {
                println!("already host");
            }
            EmulatorViewMode::Client(_) => panic!(),
            EmulatorViewMode::OffView(_) => {
                let connection = {
                    let listener = TcpListener::bind(ADDR).unwrap();
                    println!("start searching");
                    let connection = listener.incoming().next().unwrap();
                    match connection {
                        Ok(connection) => {
                            println!("connection was successful");
                            thread::sleep(Duration::from_secs_f32(0.05));
                            Some(connection)
                        }
                        Err(e) => {
                            println!("failed connecting with: {e}");
                            None
                        }
                    }
                };
                let (sender, recv) = mpsc::channel();
                self.mode = EmulatorViewMode::Host(HostView {
                    sender,
                    tcp: connection,
                });
                return Some(recv);
            }
        };
        return None;
    }
    pub fn on_pixels<T>(&self, f: impl FnOnce(&Pixels) -> T) -> Option<T> {
        self.pixels.read().ok().map(|p| f(&p))
    }
    pub fn on_pixels_mut(&mut self, f: impl FnOnce(&mut Pixels)) {
        let mut pixels = self.pixels.write().expect("pixel RWlock is broken");
        f(&mut pixels)
    }

    pub(crate) fn clone_pixel_buffer(&self) -> PixelRef {
        Arc::clone(&self.pixels)
    }
}

pub struct OffView {}
pub struct HostView {
    sender: Sender<EmulatorEvents>,
    pub tcp: Option<TcpStream>,
}
pub struct ClientView;
