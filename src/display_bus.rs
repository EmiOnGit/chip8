#[derive(Default, Debug, PartialEq, Eq)]
pub enum DisplayEvent {
    #[default]
    Nop,
    SwapPixel(usize, usize),
}
