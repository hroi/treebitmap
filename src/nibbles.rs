use std::mem;
/// Chops a number into a string of nibbles.
pub trait Nibbles {
    type Output;
    /// Return a string of nibbles (4-bit bytes). Each nibble is encoded as a ```u8```.
    fn nibbles(self) -> Self::Output;
    //fn nibbles2(self) -> Self::Output;
    //fn nibbles3(self) -> Self::Output;
}

static BYTE2NIBBLES: [[u8;2];256] = [
    [0, 0], [0, 1], [0, 2], [0, 3], [0, 4], [0, 5], [0, 6], [0, 7],
    [0, 8], [0, 9], [0, 10], [0, 11], [0, 12], [0, 13], [0, 14], [0, 15],
    [1, 0], [1, 1], [1, 2], [1, 3], [1, 4], [1, 5], [1, 6], [1, 7],
    [1, 8], [1, 9], [1, 10], [1, 11], [1, 12], [1, 13], [1, 14], [1, 15],
    [2, 0], [2, 1], [2, 2], [2, 3], [2, 4], [2, 5], [2, 6], [2, 7],
    [2, 8], [2, 9], [2, 10], [2, 11], [2, 12], [2, 13], [2, 14], [2, 15],
    [3, 0], [3, 1], [3, 2], [3, 3], [3, 4], [3, 5], [3, 6], [3, 7],
    [3, 8], [3, 9], [3, 10], [3, 11], [3, 12], [3, 13], [3, 14], [3, 15],
    [4, 0], [4, 1], [4, 2], [4, 3], [4, 4], [4, 5], [4, 6], [4, 7],
    [4, 8], [4, 9], [4, 10], [4, 11], [4, 12], [4, 13], [4, 14], [4, 15],
    [5, 0], [5, 1], [5, 2], [5, 3], [5, 4], [5, 5], [5, 6], [5, 7],
    [5, 8], [5, 9], [5, 10], [5, 11], [5, 12], [5, 13], [5, 14], [5, 15],
    [6, 0], [6, 1], [6, 2], [6, 3], [6, 4], [6, 5], [6, 6], [6, 7],
    [6, 8], [6, 9], [6, 10], [6, 11], [6, 12], [6, 13], [6, 14], [6, 15],
    [7, 0], [7, 1], [7, 2], [7, 3], [7, 4], [7, 5], [7, 6], [7, 7],
    [7, 8], [7, 9], [7, 10], [7, 11], [7, 12], [7, 13], [7, 14], [7, 15],
    [8, 0], [8, 1], [8, 2], [8, 3], [8, 4], [8, 5], [8, 6], [8, 7],
    [8, 8], [8, 9], [8, 10], [8, 11], [8, 12], [8, 13], [8, 14], [8, 15],
    [9, 0], [9, 1], [9, 2], [9, 3], [9, 4], [9, 5], [9, 6], [9, 7],
    [9, 8], [9, 9], [9, 10], [9, 11], [9, 12], [9, 13], [9, 14], [9, 15],
    [10, 0], [10, 1], [10, 2], [10, 3], [10, 4], [10, 5], [10, 6], [10, 7],
    [10, 8], [10, 9], [10, 10], [10, 11], [10, 12], [10, 13], [10, 14], [10, 15],
    [11, 0], [11, 1], [11, 2], [11, 3], [11, 4], [11, 5], [11, 6], [11, 7],
    [11, 8], [11, 9], [11, 10], [11, 11], [11, 12], [11, 13], [11, 14], [11, 15],
    [12, 0], [12, 1], [12, 2], [12, 3], [12, 4], [12, 5], [12, 6], [12, 7],
    [12, 8], [12, 9], [12, 10], [12, 11], [12, 12], [12, 13], [12, 14], [12, 15],
    [13, 0], [13, 1], [13, 2], [13, 3], [13, 4], [13, 5], [13, 6], [13, 7],
    [13, 8], [13, 9], [13, 10], [13, 11], [13, 12], [13, 13], [13, 14], [13, 15],
    [14, 0], [14, 1], [14, 2], [14, 3], [14, 4], [14, 5], [14, 6], [14, 7],
    [14, 8], [14, 9], [14, 10], [14, 11], [14, 12], [14, 13], [14, 14], [14, 15],
    [15, 0], [15, 1], [15, 2], [15, 3], [15, 4], [15, 5], [15, 6], [15, 7],
    [15, 8], [15, 9], [15, 10], [15, 11], [15, 12], [15, 13], [15, 14], [15, 15],
];

impl Nibbles for u16 {
    type Output = [u8; 4];
    #[inline]
    fn nibbles(self) -> [u8; 4] {
        let input:  [u8; 2] = unsafe { mem::transmute(self.to_be()) };
        let mut output: [u8; 4] = unsafe { mem::uninitialized() };
        for i in 0..input.len() {
            let nibs = unsafe { *BYTE2NIBBLES.get_unchecked(*input.get_unchecked(i) as usize) };
            output[i*2] = nibs[0];
            output[i*2+1] = nibs[1];
        }
        output
    }
}

impl Nibbles for u32 {
    type Output = [u8; 8];

//    fn nibbles2(self) -> [u8; 8] {
//        [((self >> 28) & 0xf) as u8,
//         ((self >> 24) & 0xf) as u8,
//         ((self >> 20) & 0xf) as u8,
//         ((self >> 16) & 0xf) as u8,
//         ((self >> 12) & 0xf) as u8,
//         ((self >>  8) & 0xf) as u8,
//         ((self >>  4) & 0xf) as u8,
//         ((self >>  0) & 0xf) as u8,
//        ]
//    }

    #[inline]
    fn nibbles(self) -> [u8; 8] {
        let input:  [u8; 4] = unsafe { mem::transmute(self.to_be()) };
        let mut output: [u8; 8] = unsafe { mem::uninitialized() };
        for i in 0..input.len() {
            let nibs = unsafe { *BYTE2NIBBLES.get_unchecked(*input.get_unchecked(i) as usize) };
            output[i*2] = nibs[0];
            output[i*2+1] = nibs[1];
        }
        output
    }

//    fn nibbles3(self) -> [u8; 8] {
//        let mut output = self as u64;
//        output |= (self as u64) << 28;
//        let out: [u8;8] = unsafe {mem::transmute((output & 0x0f0f_0f0f_0f0f_0f0f).to_be())};
//        [out[0], out[4], out[1], out[5], out[2], out[6], out[3], out[7]]
//    }
}

#[cfg(test)]
mod tests {
    extern crate test;
    extern crate rand;
    use super::*;
    use self::test::{Bencher,black_box};
    use self::rand::Rng;

    #[test]
    fn test_nibbles_u16() {
        let n: u16 = 0x1234;
        assert_eq!([1,2,3,4], n.nibbles());
    }

    #[test]
    fn test_nibbles_u32() {
        let n: u32 = 0x12345678;
        assert_eq!([1,2,3,4,5,6,7,8], n.nibbles());
    }

    #[bench]
    fn bench_nibbles_u16(b: &mut Bencher) {
        let mut rng = rand::weak_rng();
        let n: u16 = rng.gen();
        b.iter(|| {
            for i in n..n+80 {
                black_box(i.nibbles());
            }
        });
    }

    #[bench]
    fn bench_nibbles_u32(b: &mut Bencher) {
        let mut rng = rand::weak_rng();
        let n: u32 = rng.gen();
        b.iter(|| {
            for i in n..n+80 {
                black_box(i.nibbles());
            }
        });
    }

}
