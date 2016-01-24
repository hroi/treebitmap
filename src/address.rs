// Copyright 2016 Hroi Sigurdsson
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

use std::mem;
use std::net::{Ipv4Addr, Ipv6Addr};

/// Address trait provides methods required for storing in TreeBitmap trie datastructure.
pub trait Address {
    type Nibbles;
    /// Return a string of nibbles (4-bit words).
    fn nibbles(self) -> Self::Nibbles;
    /// Returns self masked to n bits.
    fn mask(self, masklen: u32) -> Self;
}

impl Address for Ipv4Addr {
    type Nibbles = [u8; 8];

    fn nibbles(self) -> Self::Nibbles {
        let mut ret: Self::Nibbles = unsafe{mem::uninitialized()};
        let bytes: [u8;4] = unsafe {mem::transmute(self)};
        for i in 0..bytes.len() {
            ret[i*2]   = bytes[i] >> 4;
            ret[i*2+1] = bytes[i] & 0xf;
        }
        ret
    }

    fn mask(self, masklen: u32) -> Self {
        debug_assert!(masklen <= 32);
        let ip = u32::from(self);
        let masked = match masklen {
            0 => 0,
            n => ip & (!0 << (32 - n))
        };
        Ipv4Addr::from(masked)
    }
}

impl Address for Ipv6Addr {
    type Nibbles = [u8; 32];

    fn nibbles(self) -> Self::Nibbles {
        let mut ret: Self::Nibbles = unsafe{mem::uninitialized()};
        let bytes: [u8;16] = unsafe {mem::transmute(self)};
        for i in 0..bytes.len() {
            ret[i*2]   = bytes[i] >> 4;
            ret[i*2+1] = bytes[i] & 0xf;
        }
        ret
    }

    fn mask(self, masklen: u32) -> Self {
        debug_assert!(masklen <= 128);
        let (first, last): (u64, u64) = unsafe {mem::transmute(self)};
        if masklen <= 64 {
            let masked = match masklen {
                0 => 0,
                n => first.to_be() & (!0 << (64 - n))
            };
            unsafe{ mem::transmute((masked.to_be(), 0u64)) }
        } else {
            let masked = match masklen {
                64 => 0,
                n => last.to_be() & (!0 << (128 - n))
            };
            unsafe{ mem::transmute((first, masked.to_be())) }
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
        let ip = Ipv4Addr::new(1,2,3,4);
        assert_eq!(ip.mask(24), Ipv4Addr::new(1,2,3,0))
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
        assert_eq!(ip.nibbles(), [1,2,3,4,5,6,7,8]);
    }

    #[test]
    fn address_ipv6_nibbles() {
        let ip = Ipv6Addr::from_str("2001:db8:aaaa:bbbb:cccc:dddd:eeee:ffff").unwrap();
        assert_eq!(ip.nibbles(), [0x2, 0x0, 0x0, 0x1, 0x0, 0xd, 0xb, 0x8,
                                  0xa, 0xa, 0xa, 0xa, 0xb, 0xb, 0xb, 0xb,
                                  0xc, 0xc, 0xc, 0xc, 0xd, 0xd, 0xd, 0xd,
                                  0xe, 0xe, 0xe, 0xe, 0xf, 0xf, 0xf, 0xf]);
    }
}
