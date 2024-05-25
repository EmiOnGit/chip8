#[derive(Default, Debug, PartialEq, Eq)]
pub enum DisplayEvent {
    #[default]
    Nop,
    ClearScreen,
    DrawSprite {
        sprite: [u8; 16],
        x: u8,
        y: u8,
    },
}
