#![feature(test)]
#![feature(alloc)]

//! # Fast IP lookup table for IPv4/IPv6 prefixes
//!
//! This crate provides a datastructure for fast IP address lookups.
//! It aims at fast lookup times, and a small memory footprint.
//! A full IPv4 BGP table of more than 600k entries fits in less than 5 MB. A full IPv6 BGP table of more than 25k entries fits in less than 1 MB.
//!
//! Longest match lookups on full BGP IP tables take on the order of 100ns.
//!
//! The internal datastructure is based on the Tree-bitmap algorithm described by W. Eatherton, Z. Dittia, G. Varghes.
//!

#[macro_use]
#[cfg(test)]
extern crate lazy_static;
extern crate alloc; // for RawVec
extern crate test;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::marker::PhantomData;

mod tree_bitmap;
use tree_bitmap::TreeBitmap;

mod address;
use address::Address;

///The operations defined on the lookup table.
pub trait IpLookupTableOps<Addr, T> {
    /// Insert a value for the prefix designated by ip and masklen. If prefix existed previously, the old value is returned.
    fn insert(&mut self, ip: Addr, masklen: u32, value: T) -> Option<T>;
    /// Remove an entry from the lookup table. If the prefix existed previously, the value is returned.
    fn remove(&mut self, ip: Addr, masklen: u32) -> Option<T>;
    /// Perform exact match lookup of ```ip```/```masklen``` and return the value.
    fn exact_match(&self, ip: Addr, masklen: u32) -> Option<&T>;
    /// Perform longest match lookup of ```ip``` and return the best matching prefix, designated by ip, masklen, along with its value.
    fn longest_match(&self, ip: Addr) -> Option<(Addr, u32, &T)>;
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
    fn mem_usage(&self) -> (usize, usize) {
        self.inner.mem_usage()
    }
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
                    Some((bits_matched,value)) => Some((ip.mask(bits_matched), bits_matched, value)),
                    None => None
                }
            }
        }
    }
}

impl_ops!(Ipv4Addr);
impl_ops!(Ipv6Addr);

#[cfg(test)]
mod tests {
    extern crate rand;

    use self::rand::{Rng,SeedableRng};
    use self::rand::distributions::{Weighted, WeightedChoice, IndependentSample};

    lazy_static! {
        static ref FULL_BGP_TABLE_IDENT: IpLookupTable<Ipv4Addr,(Ipv4Addr, u32)> = {load_bgp_dump(0).unwrap()};
        static ref FULL_BGP_TABLE_LIGHT: IpLookupTable<Ipv4Addr,()> = {load_bgp_dump_light(0).unwrap()};
        //static ref FULL_BGP6_TABLE_IDENT: Ipv6LookupTable<(Ipv6Addr, u32)> = {load_bgp6_dump(0).unwrap()};
        static ref FULL_BGP6_TABLE_LIGHT: IpLookupTable<Ipv6Addr,()> = {load_bgp6_dump_light(0).unwrap()};
    }

    use super::*;
    use super::address::Address;
    use test::{Bencher,black_box};
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::str::FromStr;
    use std::io::prelude::*;
    use std::io::{BufReader, Error};
    use std::fs::File;

    #[test]
    fn test_treebitmap_remove() {
        let mut tbl = IpLookupTable::<Ipv4Addr,u32>::new();
        tbl.insert(Ipv4Addr::new(10,0,0,0), 8, 1);
        tbl.insert(Ipv4Addr::new(10,0,10,0), 24, 2);
        let value = tbl.remove(Ipv4Addr::new(10,0,10,0), 24);
        assert_eq!(value, Some(2));
        let lookup_ip = Ipv4Addr::new(10,10,10,10);
        let expected_ip = Ipv4Addr::new(10,0,0,0);
        let lookup_result = tbl.longest_match(lookup_ip);
        assert_eq!(lookup_result, Some((expected_ip, 8, &1)));

    }

    #[test]
    fn test_treebitmap_insert() {
        let mut tbm = IpLookupTable::<Ipv4Addr,u32>::new();
        tbm.insert(Ipv4Addr::new(0,0,0,0), 0, 100001);
        tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 100002);
        tbm.insert(Ipv4Addr::new(77,66,19,0), 24, 100003);
        tbm.insert(Ipv4Addr::new(77,66,19,0), 28, 100004);
        tbm.insert(Ipv4Addr::new(217,116,224,0), 19, 100005);
    }

    #[test]
    fn test_treebitmap_insert_dup() {
        let mut tbm = IpLookupTable::<Ipv4Addr,u32>::new();
        assert_eq!(tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 1), None);
        assert_eq!(tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 2), Some(1));
    }

    #[test]
    fn test_treebitmap_longest_match6() {
        let mut tbm = IpLookupTable::<Ipv6Addr,u32>::new();
        let google = Ipv6Addr::from_str("2a00:1450::0").unwrap();
        let ip = Ipv6Addr::from_str("2a00:1450:400f:804::2004").unwrap();
        let ip2 = Ipv6Addr::from_str("2000:1000::f00").unwrap();
        tbm.insert(google, 32, 1);
        let ret = tbm.longest_match(ip);
        println!("{:?}", ret.unwrap());
        assert_eq!(ret.unwrap().0, google);
        let ret = tbm.longest_match(ip2);
        println!("{:?}", ret);

    }

    #[test]
    fn test_treebitmap_longest_match() {
        let mut tbm = IpLookupTable::<Ipv4Addr,u32>::new();
        tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 100002);
        tbm.insert(Ipv4Addr::new(100,64,0,0), 24, 10064024);
        tbm.insert(Ipv4Addr::new(100,64,1,0), 24, 10064124);
        tbm.insert(Ipv4Addr::new(100,64,0,0), 10, 100004);

        let result = tbm.longest_match(Ipv4Addr::new(10,10,10,10));
        assert_eq!(result, Some((Ipv4Addr::new(10,0,0,0), 8, &100002)));

        let result = tbm.longest_match(Ipv4Addr::new(100,100,100,100));
        assert_eq!(result, Some((Ipv4Addr::new(100,64,0,0), 10, &100004)));

        let result = tbm.longest_match(Ipv4Addr::new(100,64,0,100));
        assert_eq!(result, Some((Ipv4Addr::new(100,64,0,0), 24, &10064024)));

        let result = tbm.longest_match(Ipv4Addr::new(200,200,200,200));
        assert_eq!(result, None);
    }

    fn load_bgp6_dump_light(limit: u32) -> Result<IpLookupTable<Ipv6Addr,()>, Error> {
        let mut tbm = IpLookupTable::<Ipv6Addr,()>::with_capacity(512);
        let f = try!(File::open("test/bgp6-dump.txt"));
        let r = BufReader::new(f);
        let mut i = 0;
        for line in r.lines() {
            let line = line.unwrap();
            if let Some(slash_offset) = line.find('/') {
                i += 1;
                if limit > 0 && i > limit {
                    break;
                }
                let ip = Ipv6Addr::from_str(&line[..slash_offset]).unwrap();
                let masklen = u32::from_str(&line[slash_offset+1..]).unwrap();
                assert!(masklen <= 128);
                tbm.insert(ip, masklen, ());
            }
        }
        Ok(tbm)
    }

    fn load_bgp_dump_light(limit: u32) -> Result<IpLookupTable<Ipv4Addr,()>, Error> {
        let mut tbl = IpLookupTable::<Ipv4Addr,()>::with_capacity(512);
        let f = try!(File::open("test/bgp-dump.txt"));
        let r = BufReader::new(f);
        let mut i = 0;
        for line in r.lines() {
            let line = line.unwrap();
            if let Some(slash_offset) = line.find('/') {
                i += 1;
                if limit > 0 && i > limit {
                    break;
                }
                let ip = Ipv4Addr::from_str(&line[..slash_offset]).unwrap();
                let masklen = u32::from_str(&line[slash_offset+1..]).unwrap();
                assert!(masklen <= 32);
                tbl.insert(ip, masklen, ());
            }
        }
        let (node_bytes, result_bytes) = tbl.mem_usage();
        println!("load_bgp_dump_light: nodes: {} bytes, results: {} bytes", node_bytes, result_bytes);
        Ok(tbl)
    }

    #[allow(dead_code)]
    fn load_bgp6_dump(limit: u32) -> Result<IpLookupTable<Ipv6Addr, (Ipv6Addr, u32)>, Error> {
        let mut tbm = IpLookupTable::<Ipv6Addr,(Ipv6Addr,u32)>::with_capacity(512);
        let f = try!(File::open("test/bgp6-dump.txt"));
        let r = BufReader::new(f);
        let mut i = 0;
        for line in r.lines() {
            let line = line.unwrap();
            if let Some(slash_offset) = line.find('/') {
                i += 1;
                if limit > 0 && i > limit {
                    break;
                }
                let ip = Ipv6Addr::from_str(&line[..slash_offset]).unwrap();
                let masklen = u32::from_str(&line[slash_offset+1..]).unwrap();
                assert!(masklen <= 128);
                tbm.insert(ip, masklen, (ip, masklen));
            }
        }
        //tbm.shrink_to_fit();
        Ok(tbm)
    }

    fn load_bgp_dump(limit: u32) -> Result<IpLookupTable<Ipv4Addr, (Ipv4Addr, u32)>, Error> {
        let mut tbm = IpLookupTable::<Ipv4Addr,(Ipv4Addr,u32)>::with_capacity(512);
        let f = try!(File::open("test/bgp-dump.txt"));
        let r = BufReader::new(f);
        let mut i = 0;
        for line in r.lines() {
            let line = line.unwrap();
            if let Some(slash_offset) = line.find('/') {
                i += 1;
                if limit > 0 && i > limit {
                    break;
                }
                let ip = Ipv4Addr::from_str(&line[..slash_offset]).unwrap();
                let masklen = u32::from_str(&line[slash_offset+1..]).unwrap();
                assert!(masklen <= 32);
                tbm.insert(ip, masklen, (ip, masklen));
            }
        }
        //tbm.shrink_to_fit();
        Ok(tbm)
    }

    #[test]
    fn test_load_full_bgp() {
        let tbm = load_bgp_dump_light(0).unwrap();
        let google_dns = Ipv4Addr::new(8,8,8,8);
        let (prefix, mask, _)= tbm.longest_match(google_dns).unwrap();
        assert_eq!(prefix, Ipv4Addr::new(8,8,8,0));
        assert_eq!(mask, 24);
    }

    #[test]
    fn test_load_full_bgp6() {
        let _ = load_bgp6_dump_light(0).unwrap();
    }

    #[test]
    fn test_treebitmap_lookup_all_the_things() {
        let ref tbm = FULL_BGP_TABLE_IDENT;
        let mut rng = rand::weak_rng();
        for _ in 0..1000 {
            let ip = Ipv4Addr::from(rng.gen_range(1<<24, 224<<24));
            let result = tbm.longest_match(ip);
            println!("lookup({}) -> {:?}", ip, result);
            if let Some((prefix, masklen, val)) = result {
                let (orig_prefix, orig_masklen) = *val;
                assert_eq!((prefix, masklen), (orig_prefix, orig_masklen));
            }
        }
    }

    #[test]
    fn test_treebitmap_lookup_host() {
        let ip = Ipv4Addr::new(217,199,218,175);
        let ret = FULL_BGP_TABLE_IDENT.longest_match(ip);
        assert_eq!(ret, Some((ip, 32, &(ip, 32))));
    }

    fn synth_internet_table(n: usize) -> IpLookupTable<Ipv4Addr, Ipv4Addr> {
        let mut tbl: IpLookupTable<Ipv4Addr,Ipv4Addr> = IpLookupTable::new();
        let mut rng = rand::XorShiftRng::from_seed([1,2,3,4]);
        // http://bgp.potaroo.net/as6447/ - Root Prefix Length Distributions
        let mut masklen_distribution = vec![
            Weighted {item:  8, weight: 1},
            Weighted {item:  9, weight: 0},
            Weighted {item: 10, weight: 1},
            Weighted {item: 11, weight: 3},
            Weighted {item: 12, weight: 9},
            Weighted {item: 13, weight: 15},
            Weighted {item: 14, weight: 32},
            Weighted {item: 15, weight: 52},
            Weighted {item: 16, weight: 355},
            Weighted {item: 17, weight: 154},
            Weighted {item: 18, weight: 245},
            Weighted {item: 19, weight: 533},
            Weighted {item: 20, weight: 676},
            Weighted {item: 21, weight: 735},
            Weighted {item: 22, weight: 1261},
            Weighted {item: 23, weight: 807},
            Weighted {item: 24, weight: 5117},
            Weighted {item: 25, weight: 1},
            Weighted {item: 26, weight: 0},
            Weighted {item: 27, weight: 0},
            Weighted {item: 28, weight: 0},
            Weighted {item: 29, weight: 0},
            Weighted {item: 30, weight: 1},
            Weighted {item: 31, weight: 0},
            Weighted {item: 32, weight: 1},
        ];
        let wc = WeightedChoice::new(&mut masklen_distribution);
        for _ in 0..n {
            let ipu: u32 = rng.gen_range(1<<24, 224<<24);
            let ip = Ipv4Addr::from(ipu);
            let masklen = wc.ind_sample(&mut rng);
            tbl.insert(ip.mask(masklen), masklen, ip);
        }
        let (node_bytes, result_bytes) = tbl.mem_usage();
        println!("nodes: {} bytes, results: {} bytes", node_bytes, result_bytes);
        tbl
    }

    fn gen_random_table(n: usize) -> IpLookupTable<Ipv4Addr, Ipv4Addr> {
        let mut tbl = IpLookupTable::<Ipv4Addr,Ipv4Addr>::new();
        let mut rng = rand::weak_rng();
        for _ in 0..n {
            let ipu: u32 = rng.gen_range(1<<24, 224<<24);
            let ip = Ipv4Addr::from(ipu);
            let masklen: u32 = rng.gen_range(12, 32);
            tbl.insert(ip.mask(masklen), masklen, ip);
        }
        tbl
    }

    #[test]
    fn test_treebitmap_synthtable_rand_lookup() {
        let tbl = synth_internet_table(500_000);
        let mut rng = rand::weak_rng();
        for _ in 0..100 {
            let ipu: u32 = rng.gen_range(1<<24, 224<<24);
            let ip = Ipv4Addr::from(ipu);
            let result = tbl.longest_match(ip);
            println!("{} -> {:?}", ip, result);
        }
    }

    #[test]
    fn test_treebitmap_rand_lookup() {
        let tbl = gen_random_table(100_000);
        let mut rng = rand::weak_rng();
        for _ in 0..100 {
            let ipu: u32 = rng.gen_range(1<<24, 224<<24);
            let ip = Ipv4Addr::from(ipu);
            let result = tbl.longest_match(ip);
            println!("{} -> {:?}", ip, result);
        }
    }

    #[bench]
    fn bench_treebitmap_gen_random_table_1k(b: &mut Bencher) {
        b.iter(|| {
            black_box(gen_random_table(1_000));
        });
    }

    #[bench]
    fn bench_treebitmap_gen_random_table_10k(b: &mut Bencher) {
        b.iter(|| {
            black_box(gen_random_table(10_000));
        });
    }

    #[bench]
    fn bench_treebitmap_gen_random_table_100k(b: &mut Bencher) {
        b.iter(|| {
            black_box(gen_random_table(100_000));
        });
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_apple(b: &mut Bencher) {
        let ip = Ipv4Addr::new(17,151,0,151);
        b.iter(|| {
            black_box(FULL_BGP_TABLE_LIGHT.longest_match(ip));
        })
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_comcast6(b: &mut Bencher) {
        let ip = Ipv6Addr::from_str("2001:6c8:180:1::c3f9:1b20").unwrap();
        b.iter(|| {
            black_box(FULL_BGP6_TABLE_LIGHT.longest_match(ip));
        })
    }


    #[bench]
    fn bench_treebitmap_bgp_lookup_netgroup(b: &mut Bencher) {
        let ip = Ipv4Addr::new(77,66,88,50);
        b.iter(|| {
            black_box(FULL_BGP_TABLE_LIGHT.longest_match(ip));
        })
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_googledns(b: &mut Bencher) {
        let ip = Ipv4Addr::new(8,8,8,8);
        b.iter(|| {
            black_box(FULL_BGP_TABLE_LIGHT.longest_match(ip));
        })
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_googledns6(b: &mut Bencher) {
        let ip = Ipv6Addr::from_str("2001:4860:4860::8888").unwrap();
        b.iter(|| {
            black_box(FULL_BGP6_TABLE_LIGHT.longest_match(ip));
        })
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_localhost(b: &mut Bencher) {
        let ip = Ipv4Addr::new(127,0,0,1);
        b.iter(|| {
            black_box(FULL_BGP_TABLE_LIGHT.longest_match(ip));
        })
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_localhost6(b: &mut Bencher) {
        let ip = Ipv6Addr::from_str("::1").unwrap();
        b.iter(|| {
            black_box(FULL_BGP6_TABLE_LIGHT.longest_match(ip));
        })
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_random_sample(b: &mut Bencher) {
        let mut rng = rand::weak_rng();
        let r: u32 = rng.gen_range(1<<24, 224<<24);
        let ip = Ipv4Addr::from(r);
        b.iter(||{
            black_box(FULL_BGP_TABLE_LIGHT.longest_match(ip));
        });
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_random_every(b: &mut Bencher) {
        let mut rng = rand::weak_rng();
        b.iter(||{
            let r: u32 = rng.gen_range(1<<24, 224<<24);
            let ip = Ipv4Addr::from(r);
            black_box(FULL_BGP_TABLE_LIGHT.longest_match(ip));
        });
    }

    #[bench]
    fn bench_weak_rng(b: &mut Bencher) {
        let mut rng = rand::weak_rng();
        b.iter(||{
            let r: u32 = rng.gen_range(1<<24, 224<<24);
            black_box(r);
        });
    }
}
