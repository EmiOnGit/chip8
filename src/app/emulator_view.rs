use std::{
    io::Write,
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, RwLock,
    },
    thread,
    time::Duration,
};

use pixels::{Pixels, SurfaceTexture};
use winit::window::Window;

use crate::{
    chip8::{screen, EmulatorEvents},
    display_bus::AppEvents,
};

pub enum EmulatorViewMode {
    Host(HostView),
    Client(ClientView),
    Single(SingleView),
    OffView(OffView),
}
pub const PORT: u16 = 4442;

pub type PixelRef = Arc<RwLock<Pixels>>;
pub struct EmulatorView {
    pixels: PixelRef,
    pub mode: EmulatorViewMode,
}
impl EmulatorView {
    pub fn send(&mut self, event: EmulatorEvents) {
        match &self.mode {
            EmulatorViewMode::Host(host) => host.sender.send(event).unwrap(),
            EmulatorViewMode::Client(_) => match event {
                EmulatorEvents::ChangeColor(new_color) => self.on_pixels_mut(|pixels| {
                    pixels
                        .frame_mut()
                        .chunks_mut(4)
                        .filter(|c| *c != [0, 0, 0, 0])
                        .for_each(|c| c.clone_from_slice(&new_color.to_array()))
                }),
                _ => {}
            },
            EmulatorViewMode::OffView(_) => {}
            EmulatorViewMode::Single(single) => single.sender.send(event).unwrap(),
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
    pub fn client(pixels: PixelRef, host_addr: SocketAddr) -> (Self, TcpStream) {
        let connection = TcpStream::connect(host_addr).unwrap();
        println!("CLIENT connected with {connection:?}");
        let view = EmulatorView {
            pixels,
            mode: EmulatorViewMode::Client(ClientView),
        };
        thread::sleep(Duration::from_secs_f32(0.05));
        (view, connection)
    }
    pub fn single(pixels: PixelRef) -> (Self, Receiver<EmulatorEvents>) {
        let (sender, recv) = mpsc::channel();
        let view = EmulatorView {
            pixels,
            mode: EmulatorViewMode::Single(SingleView { sender }),
        };
        return (view, recv);
    }
    pub fn host(pixels: PixelRef, addr: SocketAddr) -> (Self, Receiver<EmulatorEvents>) {
        let connection = {
            let listener = TcpListener::bind(addr).unwrap();
            println!("start searching");
            let connection = listener.accept();
            match connection {
                Ok(connection) => {
                    println!("connection was successful");
                    thread::sleep(Duration::from_secs_f32(0.05));
                    Some(connection.0)
                }
                Err(e) => {
                    println!("failed connecting with: {e}");
                    None
                }
            }
        };
        let (sender, recv) = mpsc::channel();
        let view = EmulatorView {
            mode: EmulatorViewMode::Host(HostView {
                sender,
                tcp: connection,
            }),
            pixels,
        };
        return (view, recv);
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
pub struct SingleView {
    sender: Sender<EmulatorEvents>,
}
pub struct HostView {
    sender: Sender<EmulatorEvents>,
    pub tcp: Option<TcpStream>,
}
impl HostView {
    pub fn send_over_tcp(&mut self, event: &AppEvents) {
        let Some(tcp) = &mut self.tcp else { return };
        let bytes = bincode::serialize(event);
        let Ok(mut bytes) = bytes else { return };
        let mut buffer = bytes.len().to_be_bytes().to_vec();
        buffer.append(&mut bytes);

        tcp.write_all(&buffer).unwrap();
        tcp.flush().unwrap();
    }
}
pub struct ClientView;
