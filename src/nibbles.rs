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

#[cfg(test)]
mod tests {
    extern crate test;
    extern crate rand;
    use super::*;
    use self::test::{Bencher,black_box};
    use self::rand::Rng;

    #[test]
    fn test_nibbles() {
        let n = 0x12345678;
        assert_eq!([1,2,3,4,5,6,7,8], n.nibbles());
    }

    #[bench]
    fn bench_nibbles_u32(b: &mut Bencher) {
        let mut rng = rand::weak_rng();
        let n: u32 = rng.gen();
        b.iter(|| {
            for i in n..n+80 {
                //black_box(i.nibbles());
                black_box(i);
            }
        });
    }
} 
