use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, SendError, Sender},
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

use super::EmulatorSpawnError;

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
    pub fn send(&mut self, event: EmulatorEvents) -> Result<(), SendError<EmulatorEvents>> {
        match &self.mode {
            EmulatorViewMode::Host(host) => {
                host.sender.send(event)?;
            }
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
            EmulatorViewMode::Single(single) => {
                single.sender.send(event)?;
            }
        }
        Ok(())
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
    pub fn client(
        pixels: PixelRef,
        host_addr: SocketAddr,
    ) -> Result<(Self, TcpStream), EmulatorSpawnError> {
        let connection = TcpStream::connect(host_addr)?;
        println!("CLIENT connected with {connection:?}");
        let view = EmulatorView {
            pixels,
            mode: EmulatorViewMode::Client(ClientView {
                tcp: connection.try_clone()?,
            }),
        };
        thread::sleep(Duration::from_secs_f32(0.05));
        Ok((view, connection))
    }
    pub fn single(pixels: PixelRef) -> (Self, Receiver<EmulatorEvents>) {
        let (sender, recv) = mpsc::channel();
        let view = EmulatorView {
            pixels,
            mode: EmulatorViewMode::Single(SingleView { sender }),
        };
        return (view, recv);
    }
    pub fn host(
        pixels: PixelRef,
        addr: SocketAddr,
    ) -> Result<(Self, Receiver<EmulatorEvents>, TcpStream), EmulatorSpawnError> {
        let connection = {
            let listener = TcpListener::bind(addr)?;
            println!("start searching");
            let (connection, addr) = listener.accept()?;
            println!("connection was successful with: {}", addr);
            thread::sleep(Duration::from_secs_f32(0.05));
            connection
        };
        let (sender, recv) = mpsc::channel();
        let connection2 = connection.try_clone()?;
        let view = EmulatorView {
            mode: EmulatorViewMode::Host(HostView {
                sender,
                tcp: connection,
            }),
            pixels,
        };
        return Ok((view, recv, connection2));
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
    pub tcp: TcpStream,
}
impl HostView {}
pub struct ClientView {
    pub tcp: TcpStream,
}
pub fn send_over_tcp(tcp: &mut TcpStream, event: &AppEvents) {
    let bytes = bincode::serialize(event);
    let Ok(mut bytes) = bytes else { return };
    let mut buffer = bytes.len().to_be_bytes().to_vec();
    buffer.append(&mut bytes);

    tcp.write_all(&buffer).unwrap();
    tcp.flush().unwrap();
}
pub fn receive_event_over_tcp(tcp: &mut TcpStream) -> Option<AppEvents> {
    let mut length_bytes = 0usize.to_be_bytes();
    if let Err(e) = tcp.read_exact(&mut length_bytes) {
        println!("failed reading with: {e}");
        return None;
    };
    let length = usize::from_be_bytes(length_bytes);
    let mut message = vec![0; length];
    if let Err(e) = tcp.read_exact(&mut message) {
        println!("failed reading with: {e}");
        return None;
    };
    let message: AppEvents = bincode::deserialize(&message).unwrap();
    Some(message)
}
