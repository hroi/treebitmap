/// Chops a number into a string of nibbles.
pub trait Nibbles {
    type Output;
    /// Return a string of nibbles (4-bit bytes). Each nibble is encoded as a ```u8```.
    fn nibbles(self) -> Self::Output;
}

impl Nibbles for u32 {
    type Output = [u8; 8];

    fn nibbles(self) -> [u8; 8] {
        [((self >> 28) & 0xf) as u8,
         ((self >> 24) & 0xf) as u8,
         ((self >> 20) & 0xf) as u8,
         ((self >> 16) & 0xf) as u8,
         ((self >> 12) & 0xf) as u8,
         ((self >>  8) & 0xf) as u8,
         ((self >>  4) & 0xf) as u8,
         ((self >>  0) & 0xf) as u8,
        ]
    }
}
