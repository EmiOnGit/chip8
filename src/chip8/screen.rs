use crate::chip8::Pixels;

pub const SCREEN_HEIGHT: usize = 32;
pub const SCREEN_WIDTH: usize = 64;

pub fn pixel_row(pixels: &Pixels, y: usize) -> &[u8] {
    let frame = pixels.frame();
    let pixel_size = 4;
    let width = SCREEN_WIDTH * pixel_size;
    frame.get(y * width..(y + 1) * width).unwrap_or_default()
}
pub fn pixel_row_mut(pixels: &mut Pixels, y: usize) -> &mut [u8] {
    let frame = pixels.frame_mut();
    let pixel_size = 4;
    let width = SCREEN_WIDTH * pixel_size;
    &mut frame[y * width..(y + 1) * width]
}

pub fn set_row(pixels: &mut Pixels, x: usize, y: usize, row: u8, color: [u8; 4]) {
    if row == 0 {
        return;
    }
    pixel_row_mut(pixels, y)
        .chunks_exact_mut(4)
        .skip(x)
        .take(8)
        .enumerate()
        .filter(|(i, _pixel)| row & (1 << (7 - i)) != 0)
        .for_each(|(_i, pixel)| {
            if *pixel == [0, 0, 0, 0] {
                pixel.copy_from_slice(&color);
            } else {
                pixel.fill(0);
            }
        });
}
