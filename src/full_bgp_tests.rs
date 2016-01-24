// Copyright 2016 Hroi Sigurdsson
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

//!
//! To use these tests and benchmarks, make sure test/ contains bgp4-dump.txt and
//! bgp6-dump.txt each containing a full dump of the current internet routing
//! table, one prefix per line in CIDR notation.
//!

extern crate rand;

use self::rand::{Rng};

use super::*;
use test::{Bencher,black_box};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::io::prelude::*;
use std::io::{BufReader, Error};
use std::fs::File;

lazy_static! {
    static ref FULL_BGP_TABLE_IDENT: IpLookupTable<Ipv4Addr,(Ipv4Addr, u32)> = {load_bgp_dump(0).unwrap()};
    static ref FULL_BGP_TABLE_UNIT: IpLookupTable<Ipv4Addr,()> = {load_bgp_dump_light(0).unwrap()};
    //static ref FULL_BGP6_TABLE_IDENT: Ipv6LookupTable<(Ipv6Addr, u32)> = {load_bgp6_dump(0).unwrap()};
    static ref FULL_BGP6_TABLE_UNIT: IpLookupTable<Ipv6Addr,()> = {load_bgp6_dump_light(0).unwrap()};
}


/// We store the the prefix in the value, so we can later compare it and check that it is the correct value for the key.
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
    let f = try!(File::open("test/bgp4-dump.txt"));
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
    let f = try!(File::open("test/bgp4-dump.txt"));
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
fn loadv4() {
    let tbm = load_bgp_dump_light(0).unwrap();
    let google_dns = Ipv4Addr::new(8,8,8,8);
    let (prefix, mask, _)= tbm.longest_match(google_dns).unwrap();
    assert_eq!(prefix, Ipv4Addr::new(8,8,8,0));
    assert_eq!(mask, 24);
}

#[bench]
fn iterv4(b: &mut Bencher) {
    b.iter(|| {
        for (prefix, masklen, value) in FULL_BGP_TABLE_IDENT.iter() {
            assert_eq!((prefix, masklen), *value);
        }
    });
}

#[test]
fn iterv4_print() {
    for (prefix, masklen, _) in FULL_BGP_TABLE_UNIT.iter() {
        println!("{}/{}", prefix, masklen);
    }
}

#[test]
fn loadv6() {
    let _ = load_bgp6_dump_light(0).unwrap();
}

#[test]
// check that the values returned match what was in the key
fn lookup_random_id_check() {
    let ref tbm = FULL_BGP_TABLE_IDENT;
    let mut rng = rand::weak_rng();
    for _ in 0..10000 {
        let ip = Ipv4Addr::from(rng.gen_range(1<<24, 224<<24));
        let result = tbm.longest_match(ip);
        println!("lookup({}) -> {:?}", ip, result);
        if let Some((prefix, masklen, val)) = result {
            let (orig_prefix, orig_masklen) = *val;
            assert_eq!((prefix, masklen), (orig_prefix, orig_masklen));
        }
    }
}

#[bench]
fn lookup_apple(b: &mut Bencher) {
    let ip = Ipv4Addr::new(17,151,0,151);
    b.iter(|| {
        black_box(FULL_BGP_TABLE_UNIT.longest_match(ip));
    })
}

#[bench]
fn lookup_comcast6(b: &mut Bencher) {
    let ip = Ipv6Addr::from_str("2001:6c8:180:1::c3f9:1b20").unwrap();
    b.iter(|| {
        black_box(FULL_BGP6_TABLE_UNIT.longest_match(ip));
    })
}


#[bench]
fn lookup_netgroup(b: &mut Bencher) {
    let ip = Ipv4Addr::new(77,66,88,50);
    b.iter(|| {
        black_box(FULL_BGP_TABLE_UNIT.longest_match(ip));
    })
}

#[bench]
fn lookup_googledns(b: &mut Bencher) {
    let ip = Ipv4Addr::new(8,8,8,8);
    b.iter(|| {
        black_box(FULL_BGP_TABLE_UNIT.longest_match(ip));
    })
}

#[bench]
fn lookup_googledns6(b: &mut Bencher) {
    let ip = Ipv6Addr::from_str("2001:4860:4860::8888").unwrap();
    b.iter(|| {
        black_box(FULL_BGP6_TABLE_UNIT.longest_match(ip));
    })
}

#[bench]
fn localhost(b: &mut Bencher) {
    let ip = Ipv4Addr::new(127,0,0,1);
    b.iter(|| {
        black_box(FULL_BGP_TABLE_UNIT.longest_match(ip));
    })
}

#[bench]
fn localhost6(b: &mut Bencher) {
    let ip = Ipv6Addr::from_str("::1").unwrap();
    b.iter(|| {
        black_box(FULL_BGP6_TABLE_UNIT.longest_match(ip));
    })
}

#[bench]
fn lookup_random_sample(b: &mut Bencher) {
    let mut rng = rand::weak_rng();
    let r: u32 = rng.gen_range(1<<24, 224<<24);
    let ip = Ipv4Addr::from(r);
    b.iter(||{
        black_box(FULL_BGP_TABLE_UNIT.longest_match(ip));
    });
}

#[bench]
fn lookup_random_every(b: &mut Bencher) {
    let mut rng = rand::weak_rng();
    b.iter(||{
        let r: u32 = rng.gen_range(1<<24, 224<<24);
        let ip = Ipv4Addr::from(r);
        black_box(FULL_BGP_TABLE_UNIT.longest_match(ip));
    });
}
