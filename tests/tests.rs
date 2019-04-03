// Copyright 2016 Hroi Sigurdsson
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.
//

extern crate treebitmap;

use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use treebitmap::*;

#[test]
fn remove() {
    let mut tbl = IpLookupTable::new();
    tbl.insert(Ipv4Addr::new(10, 0, 0, 0), 8, 1);
    tbl.insert(Ipv4Addr::new(10, 0, 10, 0), 24, 2);
    tbl.insert(Ipv4Addr::new(10, 0, 10, 9), 32, 3);

    {
        let lookup_ip = Ipv4Addr::new(10, 0, 10, 10);
        let expected_ip = Ipv4Addr::new(10, 0, 10, 0);
        let lookup_result = tbl.longest_match(lookup_ip);
        assert_eq!(lookup_result, Some((expected_ip, 24, &2)));

        let lookup_ip = Ipv4Addr::new(10, 0, 10, 9);
        let expected_ip = Ipv4Addr::new(10, 0, 10, 9);
        let lookup_result = tbl.longest_match(lookup_ip);
        assert_eq!(lookup_result, Some((expected_ip, 32, &3)));
    }

    {
        let value = tbl.remove(Ipv4Addr::new(10, 0, 10, 0), 24);
        assert_eq!(value, Some(2));
        let lookup_ip = Ipv4Addr::new(10, 0, 10, 10);
        let expected_ip = Ipv4Addr::new(10, 0, 0, 0);
        let lookup_result = tbl.longest_match(lookup_ip);
        assert_eq!(lookup_result, Some((expected_ip, 8, &1)));
    }
}

#[test]
fn insert() {
    let mut tbm = IpLookupTable::new();
    tbm.insert(Ipv4Addr::new(0, 0, 0, 0), 0, 1);
    tbm.insert(Ipv4Addr::new(10, 0, 0, 0), 8, 1);
}

#[test]
fn insert_dup() {
    let mut tbm = IpLookupTable::new();
    assert_eq!(tbm.insert(Ipv4Addr::new(10, 0, 0, 0), 8, 1), None);
    assert_eq!(tbm.insert(Ipv4Addr::new(10, 0, 0, 0), 8, 2), Some(1));
}

#[test]
fn longest_match6() {
    let mut tbm = IpLookupTable::new();
    let google = Ipv6Addr::from_str("2a00:1450::0").unwrap();
    let ip = Ipv6Addr::from_str("2a00:1450:400f:804::2004").unwrap();
    let ip2 = Ipv6Addr::from_str("2000:1000::f00").unwrap();
    tbm.insert(google, 32, 1);
    let ret = tbm.longest_match(ip);
    println!("{:?}", ret.unwrap());
    assert_eq!(ret.unwrap().0, google);
    let ret = tbm.longest_match(ip2);
    println!("{:?}", ret);
    assert_eq!(ret, None);
}

#[test]
fn longest_match() {
    let mut tbm = IpLookupTable::new();
    tbm.insert(Ipv4Addr::new(10, 0, 0, 0), 8, 100002);
    tbm.insert(Ipv4Addr::new(100, 64, 0, 0), 24, 10064024);
    tbm.insert(Ipv4Addr::new(100, 64, 1, 0), 24, 10064124);
    tbm.insert(Ipv4Addr::new(100, 64, 0, 0), 10, 100004);

    let result = tbm.longest_match(Ipv4Addr::new(10, 10, 10, 10));
    assert_eq!(result, Some((Ipv4Addr::new(10, 0, 0, 0), 8, &100002)));

    let result = tbm.longest_match(Ipv4Addr::new(100, 100, 100, 100));
    assert_eq!(result, Some((Ipv4Addr::new(100, 64, 0, 0), 10, &100004)));

    let result = tbm.longest_match(Ipv4Addr::new(100, 64, 0, 100));
    assert_eq!(result, Some((Ipv4Addr::new(100, 64, 0, 0), 24, &10064024)));

    let result = tbm.longest_match(Ipv4Addr::new(100, 64, 1, 100));
    assert_eq!(result, Some((Ipv4Addr::new(100, 64, 1, 0), 24, &10064124)));

    let result = tbm.longest_match(Ipv4Addr::new(200, 200, 200, 200));
    assert_eq!(result, None);
}

#[test]
fn iter() {
    let mut tbl = IpLookupTable::new();

    let (ip_a, mask_a, value_a) = (Ipv4Addr::new(10, 0, 0, 0), 8, 1);
    let (ip_b, mask_b, value_b) = (Ipv4Addr::new(100, 64, 0, 0), 24, 2);
    let (ip_c, mask_c, value_c) = (Ipv4Addr::new(100, 64, 1, 0), 24, 3);
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
    let mut tbl = IpLookupTable::new();

    let (ip_a, mask_a, value_a) = (Ipv4Addr::new(10, 0, 0, 0), 8, 1);
    let (ip_b, mask_b, value_b) = (Ipv4Addr::new(100, 64, 0, 0), 24, 2);
    let (ip_c, mask_c, value_c) = (Ipv4Addr::new(100, 64, 1, 0), 24, 3);
    tbl.insert(ip_a, mask_a, value_a);
    tbl.insert(ip_b, mask_b, value_b);
    tbl.insert(ip_c, mask_c, value_c);

    for (_ip, _mask, val) in tbl.iter_mut() {
        *val += 10;
    }
    let mut iter = tbl.iter();

    assert_eq!(iter.next(), Some((ip_a, mask_a, &11)));
    assert_eq!(iter.next(), Some((ip_b, mask_b, &12)));
    assert_eq!(iter.next(), Some((ip_c, mask_c, &13)));
    assert_eq!(iter.next(), None);
}

#[test]
fn into_iter() {
    let mut tbl = IpLookupTable::new();

    let (ip_a, mask_a, value_a) = (Ipv4Addr::new(10, 0, 0, 0), 8, 1);
    let (ip_b, mask_b, value_b) = (Ipv4Addr::new(100, 64, 0, 0), 24, 2);
    let (ip_c, mask_c, value_c) = (Ipv4Addr::new(100, 64, 1, 0), 24, 3);
    tbl.insert(ip_a, mask_a, value_a);
    tbl.insert(ip_b, mask_b, value_b);
    tbl.insert(ip_c, mask_c, value_c);

    let mut iter = tbl.into_iter();
    assert_eq!(iter.next(), Some((ip_a, mask_a, value_a)));
    assert_eq!(iter.next(), Some((ip_b, mask_b, value_b)));
    assert_eq!(iter.next(), Some((ip_c, mask_c, value_c)));
    assert_eq!(iter.next(), None);
}

#[test]
fn send() {
    use std::sync::Arc;
    use std::thread;

    let mut tbl = IpLookupTable::new();
    let (ip, mask, value) = (Ipv4Addr::new(10, 0, 0, 0), 8, 1);
    tbl.insert(ip, mask, value);

    let arc = Arc::new(tbl);
    let arc_thread = arc.clone();
    thread::spawn(move || {
        let lookup_result = arc_thread.exact_match(Ipv4Addr::new(10, 0, 0, 0), 8);
        assert_eq!(lookup_result, Some(&1));
    })
    .join()
    .unwrap();
}

// https://github.com/hroi/treebitmap/issues/7
#[test]
fn issue_7() {
    let mut table: IpLookupTable<Ipv4Addr, ()> = IpLookupTable::new();

    println!("len: {}", table.len());

    table.insert("2.93.185.24".parse().unwrap(), 32, ());
    table.insert("2.93.200.133".parse().unwrap(), 32, ());

    println!("len: {}", table.len());

    table.remove("2.93.185.24".parse().unwrap(), 32);
    table.remove("2.93.200.133".parse().unwrap(), 32);

    println!("len: {}", table.len());
}
