// Copyright 2016 Hroi Sigurdsson
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

//! # Fast IP lookup table for IPv4/IPv6 prefixes
//!
//! This crate provides a datastructure for fast IP address lookups.
//! It aims at fast lookup times, and a small memory footprint.
//! A full IPv4 BGP table of more than 600k entries fits in less than 5 MB. A
//! full IPv6 BGP table of more than 25k entries fits in less than 1 MB.
//!
//! Longest match lookups on full BGP IP tables take on the order of 100ns.
//!
//! The internal datastructure is based on the Tree-bitmap algorithm described
//! by W. Eatherton, Z. Dittia, G. Varghes.
//!

use std::net::{Ipv4Addr, Ipv6Addr};
use std::marker::PhantomData;

mod tree_bitmap;
use tree_bitmap::TreeBitmap;

mod address;
use address::Address;

/// The operations defined on the lookup table.
pub trait IpLookupTableOps<Addr, T> {
    /// Insert a value for the prefix designated by ip and masklen. If prefix
    /// existed previously, the old value is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use treebitmap::{IpLookupTable, IpLookupTableOps};
    /// use std::net::Ipv6Addr;
    ///
    /// let mut table: IpLookupTable<Ipv6Addr,&str> = IpLookupTable::new();
    /// let prefix = Ipv6Addr::new(0x2001, 0xdb8, 0xdead, 0xbeef, 0, 0, 0, 0);
    /// let masklen = 32;
    ///
    /// assert_eq!(table.insert(prefix, masklen, "foo"), None);
    /// // Insert duplicate
    /// assert_eq!(table.insert(prefix, masklen, "bar"), Some("foo"));
    /// ```
    fn insert(&mut self, ip: Addr, masklen: u32, value: T) -> Option<T>;

    /// Remove an entry from the lookup table. If the prefix existed previously,
    /// the value is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use treebitmap::{IpLookupTable, IpLookupTableOps};
    /// use std::net::Ipv6Addr;
    ///
    /// let mut table: IpLookupTable<Ipv6Addr,&str> = IpLookupTable::new();
    /// let prefix = Ipv6Addr::new(0x2001, 0xdb8, 0xdead, 0xbeef, 0, 0, 0, 0);
    /// let masklen = 32;
    /// table.insert(prefix, masklen, "foo");
    ///
    /// assert_eq!(table.remove(prefix, masklen), Some("foo"));
    /// // Remove non-existant
    /// assert_eq!(table.remove(prefix, masklen), None);
    /// ```
    fn remove(&mut self, ip: Addr, masklen: u32) -> Option<T>;

    /// Perform exact match lookup of `ip`/`masklen` and return the
    /// value.
    ///
    /// # Examples
    ///
    /// ```
    /// use treebitmap::{IpLookupTable, IpLookupTableOps};
    /// use std::net::Ipv6Addr;
    ///
    /// let mut table: IpLookupTable<Ipv6Addr,&str> = IpLookupTable::new();
    /// let prefix = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0);
    /// let masklen = 32;
    /// table.insert(prefix, masklen, "foo");
    ///
    /// assert_eq!(table.exact_match(prefix, masklen), Some(&"foo"));
    /// // differing mask
    /// assert_eq!(table.exact_match(prefix, 48), None);
    /// ```
    fn exact_match(&self, ip: Addr, masklen: u32) -> Option<&T>;

    /// Perform longest match lookup of `ip` and return the best matching
    /// prefix, designated by ip, masklen, along with its value.
    ///
    /// # Example
    ///
    /// ```
    /// use treebitmap::{IpLookupTable, IpLookupTableOps};
    /// use std::net::Ipv6Addr;
    ///
    /// let mut table: IpLookupTable<Ipv6Addr,&str> = IpLookupTable::new();
    /// let less_specific = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0);
    /// let more_specific = Ipv6Addr::new(0x2001, 0xdb8, 0xdead, 0, 0, 0, 0, 0);
    /// table.insert(less_specific, 32, "foo");
    /// table.insert(more_specific, 48, "bar");
    ///
    /// let lookupip = Ipv6Addr::new(0x2001, 0xdb8, 0xdead, 0xbeef,
    ///                              0xcafe, 0xbabe, 0, 1);
    /// let result = table.longest_match(lookupip);
    /// assert_eq!(result, Some((more_specific, 48, &"bar")));
    ///
    /// let lookupip = Ipv6Addr::new(0x2001, 0xdb8, 0xcafe, 0xf00,
    ///                              0xf00, 0xf00, 0, 1);
    /// let result = table.longest_match(lookupip);
    /// assert_eq!(result, Some((less_specific, 32, &"foo")));
    /// ```
    fn longest_match(&self, ip: Addr) -> Option<(Addr, u32, &T)>;

    /// Returns iterator over prefixes and values.
    ///
    /// # Examples
    ///
    /// ```
    /// use treebitmap::{IpLookupTable, IpLookupTableOps};
    /// use std::net::Ipv6Addr;
    ///
    /// let mut table: IpLookupTable<Ipv6Addr,&str> = IpLookupTable::new();
    /// let less_specific = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0);
    /// let more_specific = Ipv6Addr::new(0x2001, 0xdb8, 0xdead, 0, 0, 0, 0, 0);
    /// table.insert(less_specific, 32, "foo");
    /// table.insert(more_specific, 48, "bar");
    ///
    /// let mut iter = table.iter();
    /// assert_eq!(iter.next(), Some((less_specific, 32, &"foo")));
    /// assert_eq!(iter.next(), Some((more_specific, 48, &"bar")));
    /// assert_eq!(iter.next(), None);
    /// ```
    fn iter(&self) -> Iter<Addr, T>;

    /// Mutable version of iter()
    fn iter_mut(&self) -> IterMut<Addr, T>;
}

/// A fast, compressed IP lookup table.
pub struct IpLookupTable<A, T> {
    inner: TreeBitmap<T>,
    _addrtype: PhantomData<A>,
}

impl<A, T> IpLookupTable<A, T> {
    /// Initialize an empty lookup table with no preallocation.
    pub fn new() -> Self {
        IpLookupTable {
            inner: TreeBitmap::new(),
            _addrtype: PhantomData,
        }
    }

    /// Initialize an empty lookup table with pre-allocated buffers.
    pub fn with_capacity(n: usize) -> Self {
        IpLookupTable {
            inner: TreeBitmap::with_capacity(n),
            _addrtype: PhantomData,
        }
    }

    /// Return the bytes used by nodes and results.
    pub fn mem_usage(&self) -> (usize, usize) {
        self.inner.mem_usage()
    }

    /// Return number of items inside table.
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

/// Iterator over prefixes and associated values. The prefixes are returned in
/// "tree"-order.
pub struct Iter<'a, A, T: 'a> {
    inner: tree_bitmap::Iter<'a, T>,
    _addrtype: PhantomData<A>,
}

/// Mutable iterator over prefixes and associated values. The prefixes are
/// returned in "tree"-order.
pub struct IterMut<'a, A, T: 'a> {
    inner: tree_bitmap::IterMut<'a, T>,
    _addrtype: PhantomData<A>,
}

/// Converts ```IpLookupTable``` into an iterator. The prefixes are returned in
/// "tree"-order.
pub struct IntoIter<A, T> {
    inner: tree_bitmap::IntoIter<T>,
    _addrtype: PhantomData<A>,
}

macro_rules! impl_ops {
    ($addr_type:ty) => {
        impl<T: Sized> IpLookupTableOps<$addr_type, T> for IpLookupTable<$addr_type, T> {

            fn insert(&mut self, ip: $addr_type, masklen: u32, value: T) -> Option<T>{
                self.inner.insert(&ip.nibbles(), masklen, value)
            }

            fn remove(&mut self, ip: $addr_type, masklen: u32) -> Option<T>{
                self.inner.remove(&ip.nibbles(), masklen)
            }

            fn exact_match(&self, ip: $addr_type, masklen: u32) -> Option<&T> {
                self.inner.exact_match(&ip.nibbles(), masklen)
            }

            fn longest_match(&self, ip: $addr_type) -> Option<($addr_type, u32, &T)> {
                match self.inner.longest_match(&ip.nibbles()) {
                    Some((bits_matched,value)) => Some((ip.mask(bits_matched),
                                                        bits_matched, value)),
                    None => None
                }
            }

            fn iter(&self) -> Iter<$addr_type,T> {
                Iter{
                    inner: self.inner.iter(),
                    _addrtype: PhantomData,
                }
            }

            fn iter_mut(&self) -> IterMut<$addr_type,T> {
                IterMut{
                    inner: self.inner.iter_mut(),
                    _addrtype: PhantomData,
                }
            }
        }

        impl<'a, T: 'a> Iterator for Iter<'a, $addr_type, T> {
            type Item = ($addr_type, u32, &'a T);

            fn next(&mut self) -> Option<Self::Item> {
                match self.inner.next() {
                    Some((nibbles, masklen, value)) => {
                        Some((Address::from_nibbles(&nibbles[..]), masklen, value))
                    },
                    None => None,
                }
            }
        }

        impl<'a, T: 'a> Iterator for IterMut<'a, $addr_type, T> {
            type Item = ($addr_type, u32, &'a mut T);

            fn next(&mut self) -> Option<Self::Item> {
                match self.inner.next() {
                    Some((nibbles, masklen, value)) => {
                        Some((Address::from_nibbles(&nibbles[..]), masklen, value))
                    },
                    None => None,
                }
            }
        }

        impl<'a, T: 'a> Iterator for IntoIter<$addr_type, T> {
            type Item = ($addr_type, u32, T);

            fn next(&mut self) -> Option<Self::Item> {
                match self.inner.next() {
                    Some((nibbles, masklen, value)) => {
                        Some((Address::from_nibbles(&nibbles[..]), masklen, value))
                    },
                    None => None,
                }
            }
        }

        impl<T> IntoIterator for IpLookupTable<$addr_type, T> {
            type Item = ($addr_type, u32, T);
            type IntoIter = IntoIter<$addr_type,T>;

            fn into_iter(self) -> IntoIter<$addr_type,T> {
                IntoIter {
                    inner: self.inner.into_iter(),
                    _addrtype: PhantomData,
                }
            }
        }
    }
}

impl_ops!(Ipv4Addr);
impl_ops!(Ipv6Addr);

#[cfg(test)]
mod tests;
#[cfg(all(test, feature = "full-bgp-tests"))]
mod full_bgp_tests;
