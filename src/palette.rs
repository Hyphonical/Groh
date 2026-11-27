#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rgb(pub(crate) u8, pub(crate) u8, pub(crate) u8);

pub(crate) const FOREGROUND: Rgb = Rgb(0xff, 0xff, 0xd4);
pub(crate) const RED: Rgb = Rgb(0xc0, 0x6d, 0x44);
pub(crate) const GREEN: Rgb = Rgb(0xaf, 0xb9, 0x7a);
pub(crate) const YELLOW: Rgb = Rgb(0xc2, 0xa8, 0x6c);
pub(crate) const BLUE: Rgb = Rgb(0x44, 0x47, 0x4a);
pub(crate) const CYAN: Rgb = Rgb(0x77, 0x83, 0x85);
