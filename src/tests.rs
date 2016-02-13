// Copyright 2016 Hroi Sigurdsson
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

extern crate rand;

use self::rand::{Rng,SeedableRng};
use self::rand::distributions::{Weighted, WeightedChoice, IndependentSample};

use super::*;
use super::address::Address;
use test::{Bencher,black_box};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[test]
fn remove() {
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
fn insert() {
    let mut tbm = IpLookupTable::<Ipv4Addr,u32>::new();
    tbm.insert(Ipv4Addr::new(0,0,0,0), 0, 1);
    tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 1);
}

#[test]
fn insert_dup() {
    let mut tbm = IpLookupTable::<Ipv4Addr,u32>::new();
    assert_eq!(tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 1), None);
    assert_eq!(tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 2), Some(1));
}

#[test]
fn longest_match6() {
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
fn longest_match() {
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

#[test]
fn iter() {

    let mut tbl = IpLookupTable::<Ipv4Addr,u32>::new();

    let (ip_a, mask_a, value_a) = (Ipv4Addr::new( 10, 0,0,0),  8, 1);
    let (ip_b, mask_b, value_b) = (Ipv4Addr::new(100,64,0,0), 24, 2);
    let (ip_c, mask_c, value_c) = (Ipv4Addr::new(100,64,1,0), 24, 3);
    tbl.insert(ip_a, mask_a, value_a);
    tbl.insert(ip_b, mask_b, value_b);
    tbl.insert(ip_c, mask_c, value_c);

    let mut iter = tbl.iter();
    assert_eq!(iter.next(), Some((ip_a, mask_a, &value_a)));
    assert_eq!(iter.next(), Some((ip_b, mask_b, &value_b)));
    assert_eq!(iter.next(), Some((ip_c, mask_c, &value_c)));
    assert_eq!(iter.next(), None);
}

#[test]
fn iter_mut() {

    let mut tbl = IpLookupTable::<Ipv4Addr,u32>::new();

    let (ip_a, mask_a, mut value_a) = (Ipv4Addr::new( 10, 0,0,0),  8, 1);
    let (ip_b, mask_b, mut value_b) = (Ipv4Addr::new(100,64,0,0), 24, 2);
    let (ip_c, mask_c, mut value_c) = (Ipv4Addr::new(100,64,1,0), 24, 3);
    tbl.insert(ip_a, mask_a, value_a);
    tbl.insert(ip_b, mask_b, value_b);
    tbl.insert(ip_c, mask_c, value_c);

    let mut iter = tbl.iter_mut();

    assert_eq!(iter.next(), Some((ip_a, mask_a, &mut value_a)));
    assert_eq!(iter.next(), Some((ip_b, mask_b, &mut value_b)));
    assert_eq!(iter.next(), Some((ip_c, mask_c, &mut value_c)));
    assert_eq!(iter.next(), None);
}

#[test]
fn into_iter() {

    let mut tbl = IpLookupTable::<Ipv4Addr,u32>::new();

    let (ip_a, mask_a, value_a) = (Ipv4Addr::new( 10, 0,0,0),  8, 1);
    let (ip_b, mask_b, value_b) = (Ipv4Addr::new(100,64,0,0), 24, 2);
    let (ip_c, mask_c, value_c) = (Ipv4Addr::new(100,64,1,0), 24, 3);
    tbl.insert(ip_a, mask_a, value_a);
    tbl.insert(ip_b, mask_b, value_b);
    tbl.insert(ip_c, mask_c, value_c);

    let mut iter = tbl.into_iter();
    assert_eq!(iter.next(), Some((ip_a, mask_a, value_a)));
    assert_eq!(iter.next(), Some((ip_b, mask_b, value_b)));
    assert_eq!(iter.next(), Some((ip_c, mask_c, value_c)));
    assert_eq!(iter.next(), None);
}

// Simulate a full Internet table.
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
