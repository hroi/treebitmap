// Copyright 2016 Hroi Sigurdsson
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

use std::mem;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::cmp::min;

/// Address trait provides methods required for storing in TreeBitmap trie datastructure.
pub trait Address: Copy {
    type Nibbles: AsRef<[u8]>;
    /// Convert to string of nibbles.
    fn nibbles(self) -> Self::Nibbles;
    /// Convert from string of nibbles.
    fn from_nibbles(nibbles: &[u8]) -> Self;
    /// Returns self masked to n bits.
    fn mask(self, masklen: u32) -> Self;
}

impl Address for Ipv4Addr {
    type Nibbles = [u8; 8];

    fn nibbles(self) -> Self::Nibbles {
        let mut ret: Self::Nibbles = unsafe { mem::uninitialized() };
        let bytes: [u8; 4] = unsafe { mem::transmute(self) };
        for (i, byte) in bytes.iter().enumerate() {
            ret[i * 2] = byte >> 4;
            ret[i * 2 + 1] = byte & 0xf;
        }
        ret
    }

    fn from_nibbles(nibbles: &[u8]) -> Self {
        let mut ret: [u8; 4] = [0; 4];
        let lim = min(ret.len() * 2, nibbles.len());
        for (i, nibble) in nibbles.iter().enumerate().take(lim) {
            match i % 2 {
                0 => {
                    ret[i / 2] = *nibble << 4;
                }
                _ => {
                    ret[i / 2] |= *nibble;
                }
            }
        }
        unsafe { mem::transmute(ret) }
    }

    fn mask(self, masklen: u32) -> Self {
        debug_assert!(masklen <= 32);
        let ip = u32::from(self);
        let masked = match masklen {
            0 => 0,
            n => ip & (!0 << (32 - n)),
        };
        Ipv4Addr::from(masked)
    }
}

impl Address for Ipv6Addr {
    type Nibbles = [u8; 32];

    fn nibbles(self) -> Self::Nibbles {
        let mut ret: Self::Nibbles = unsafe { mem::uninitialized() };
        let bytes: [u8; 16] = unsafe { mem::transmute(self) };
        for (i, byte) in bytes.iter().enumerate() {
            ret[i * 2] = byte >> 4;
            ret[i * 2 + 1] = byte & 0xf;
        }
        ret
    }

    fn from_nibbles(nibbles: &[u8]) -> Self {
        let mut ret: [u8; 16] = [0; 16];
        let lim = min(ret.len() * 2, nibbles.len());
        for (i, nibble) in nibbles.iter().enumerate().take(lim) {
            match i % 2 {
                0 => {
                    ret[i / 2] = *nibble << 4;
                }
                _ => {
                    ret[i / 2] |= *nibble;
                }
            }
        }
        unsafe { mem::transmute(ret) }
    }

    fn mask(self, masklen: u32) -> Self {
        debug_assert!(masklen <= 128);
        let (first, last): (u64, u64) = unsafe { mem::transmute(self) };
        if masklen <= 64 {
            let masked = match masklen {
                0 => 0,
                n => first.to_be() & (!0 << (64 - n)),
            };
            unsafe { mem::transmute((masked.to_be(), 0u64)) }
        } else {
            let masked = match masklen {
                64 => 0,
                n => last.to_be() & (!0 << (128 - n)),
            };
            unsafe { mem::transmute((first, masked.to_be())) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::str::FromStr;

    #[test]
    fn address_ipv4_mask() {
        let ip = Ipv4Addr::new(1, 2, 3, 4);
        assert_eq!(ip.mask(24), Ipv4Addr::new(1, 2, 3, 0))
    }

    #[test]
    fn address_ipv6_mask() {
        let ip = Ipv6Addr::from_str("2001:db8:aaaa:bbbb:cccc:dddd:eeee:ffff").unwrap();
        let expected1 = Ipv6Addr::from_str("2001:db8:aaaa::").unwrap();
        let expected2 = Ipv6Addr::from_str("2001:db8:aaaa:bbbb:cccc:dddd::").unwrap();
        assert_eq!(ip.mask(48), expected1);
        assert_eq!(ip.mask(96), expected2);
    }

    #[test]
    fn address_ipv4_nibbles() {
        let ip = Ipv4Addr::from(0x12345678);
        assert_eq!(ip.nibbles(), [1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn address_ipv6_nibbles() {
        let ip = Ipv6Addr::from_str("2001:db8:aaaa:bbbb:cccc:dddd:eeee:ffff").unwrap();
        assert_eq!(
            ip.nibbles(),
            [
                0x2, 0x0, 0x0, 0x1, 0x0, 0xd, 0xb, 0x8, 0xa, 0xa, 0xa, 0xa, 0xb, 0xb, 0xb, 0xb,
                0xc, 0xc, 0xc, 0xc, 0xd, 0xd, 0xd, 0xd, 0xe, 0xe, 0xe, 0xe, 0xf, 0xf, 0xf, 0xf,
            ]
        );
    }

    #[test]
    fn address_ipv4_from_nibbles() {
        let ip: Ipv4Addr = Address::from_nibbles(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(ip.octets(), [0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn address_ipv6_from_nibbles() {
        let ip: Ipv6Addr = Address::from_nibbles(&[
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 15, 14, 13, 12, 11, 10, 9, 8, 7,
            6, 5, 4, 3, 2, 1, 0,
        ]);
        let expected = Ipv6Addr::new(
            0x123,
            0x4567,
            0x89ab,
            0xcdef,
            0xfedc,
            0xba98,
            0x7654,
            0x3210,
        );
        assert_eq!(ip, expected);
    }

}
