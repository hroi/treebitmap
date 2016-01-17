#![allow(dead_code,unused_imports,unused_variables,unused_features)]
#![feature(test)]
#![feature(alloc)]

//!
//! A datastructure for fast IP lookups in software. Based on Tree-bitmap algorithm described by W. Eatherton, Z. Dittia, G. Varghes.
//!

extern crate alloc;
use std::mem;
use std::ptr;
use std::net::Ipv4Addr;

// mod allocator;
mod allocator_raw;
pub use allocator_raw::{Allocator, AllocatorHandle};
//pub use allocator::{Allocator, AllocatorHandle};

mod trie;
pub use trie::{TrieNode, MatchResult};

mod nibbles;
pub use nibbles::Nibbles;

#[derive(Debug)]
pub struct TreeBitmap<T: Sized> {
    //rootnode: TrieNode,
    trienodes: Allocator<TrieNode>,
    results: Allocator<T>,
}

fn bit_position(me: (u16,u16), them: (u16,u16)) -> u32 {
    let me32   = ((me.0   as u32 & !1) << 16) | me.1 as u32;
    let them32 = ((them.0 as u32 & !1) << 16) | them.1 as u32;
    let which_bit = me32.leading_zeros();
    if which_bit > 0 {
        (them32 >> (32 - which_bit)).count_ones()
    } else {
        0
    }
}

impl<T: Sized> TreeBitmap<T> {

    pub fn new() -> Self {
        let mut trieallocator: Allocator<TrieNode> = Allocator::new();
        let mut root_hdl = trieallocator.alloc(0);
        trieallocator.insert(&mut root_hdl, 0, TrieNode::new());

        let mut resultsallocator: Allocator<T> = Allocator::new();
        resultsallocator.alloc(0);
        TreeBitmap {
            //rootnode: rootnode,
            trienodes: trieallocator,
            results: resultsallocator,
        }
    }

    /// Returns handle to root node.
    pub fn root_hdl(&self) -> AllocatorHandle {
        AllocatorHandle::generate(1, 0)
    }

    /// Returns the root node.
    pub fn root_node(&self) -> TrieNode {
        //let hdl = self.root_hdl();
        self.trienodes.get(&self.root_hdl(), 0).clone()
    }

    /// Push down results encoded in the last 16 bits into child trie nodes. Makes ```node``` into a normal node.
    pub fn push_down(&mut self, node: &mut TrieNode) {
        assert!(node.is_endnode(), "push_down: not an endnode");
        assert!(node.child_ptr == 0);
        // count number of internal nodes in the first 15 bits (those that will remain in place).
        let remove_at = (node.internal() & 0xffff0000).count_ones();
        // count how many nodes to push down
        let nodes_to_pushdown = (node.bitmap & 0x0000ffff).count_ones();
        if nodes_to_pushdown > 0 {
            // alloc handle for _this_ node
            //let mut node_hdl = AllocatorHandle::generate(node.result_count(), node.result_ptr);
            let mut result_hdl = node.result_handle();
            let mut child_node_hdl = self.trienodes.alloc(0); // allocate space for the child trie nodes
            //let mut child_node_hdl = match node.child_count() {
            //    0 => self.trienodes.alloc(0),
            //    _ => node.child_handle(),
            //};
            for i in 0..nodes_to_pushdown {
                // allocate space for child result value
                let mut child_result_hdl = self.results.alloc(0);
                // put result value in freshly allocated bucket
                let result_value = self.results.remove(&mut result_hdl, remove_at);
                self.results.insert(&mut child_result_hdl, 0, result_value);
                // create and save child node
                let mut child_node = TrieNode::new();
                child_node.set_internal(1<<31);
                child_node.result_ptr = child_result_hdl.offset;
                // append trienode to collection
                let insert_node_at = child_node_hdl.len;
                self.trienodes.insert(&mut child_node_hdl, insert_node_at, child_node);
            }
            // the result data may have moved to a smaller bucket, update the result pointer
            node.result_ptr = result_hdl.offset;
            node.child_ptr = child_node_hdl.offset;
        }
        // done!
        node.make_normalnode();
        // note: we do not need to touch the external bits, they are correct as are
    }

    /// longest match lookup of ```ip```. Returns matched ip as u32, bits matched as u32, and reference to T.
    pub fn longest_match(&self, ip: Ipv4Addr) -> Option<(Ipv4Addr, u32, &T)> {
        //println!("lookup({})", ip);
        let ip = u32::from(ip);
        let nibbles = ip.nibbles();
        let mut cur_hdl = self.root_hdl();
        let mut cur_index = 0;
        let mut bits_matched = 0;

        for nibble in &nibbles {
            let cur_node = self.trienodes.get(&cur_hdl, cur_index).clone();
            let match_result = cur_node.match_segment(*nibble);
            //println!("{:#?}\n{:?}", cur_node, match_result);
            match match_result {
                MatchResult::Chase(child_hdl, offset) => {
                    bits_matched += 4;
                    cur_hdl = child_hdl;
                    cur_index = offset;
                    continue;
                },
                MatchResult::Match(result_hdl, result_slot, bits) => {
                    unsafe {
                        bits_matched += *trie::BIT_MATCH.get_unchecked(bits as usize); //[bits as usize];
                    }
                    let masked_ip = match bits_matched {
                        0 => 0,
                        32 => ip,
                        _ => ip & (!0 << (32 - bits_matched))
                    };
                    return Some((Ipv4Addr::from(masked_ip), bits_matched, self.results.get(&result_hdl, result_slot)));
                },
                MatchResult::None => return None,
            }
        }
        unreachable!()
    }

    /// returns any existing T set for key
    pub fn insert(&mut self, ip: Ipv4Addr, masklen: u32, value: T) {
        //println!("insert(ip: {}, masklen: {}, value: T)", ip, masklen);
        let nibbles = u32::from(ip).nibbles();
        //println!("\tnibbles: {:?}", &nibbles);
        let mut cur_hdl = self.root_hdl();
        let mut cur_index = 0;
        let mut bits_left = masklen;

        let mut depth = 0;
        for nibble in &nibbles {
            //println!("\tloop {}, cur_hdl: {:?}, cur_index: {}, bits_left: {}", depth, cur_hdl, cur_index, bits_left);
            depth += 1;

            let mut cur_node = self.trienodes.get(&cur_hdl, cur_index).clone();
            let match_result = cur_node.match_segment(*nibble);
            //println!("\tcur_node: {:?}", cur_node);
            //println!("\tmatch_segment({:4b}) -> {:?}", nibble, match_result);

            if let MatchResult::Chase(child_hdl, index) = match_result {
                if bits_left >= 4 {
                    // follow existing branch
                    bits_left -= 4;
                    cur_hdl = child_hdl;
                    cur_index = index;
                    continue;
                }
            }

            let bitmap = trie::gen_bitmap(*nibble, std::cmp::min(4, bits_left));

            if (cur_node.is_endnode() && bits_left <= 4) || bits_left <= 3 {
                // final node reached, insert results
                let mut result_hdl = match cur_node.result_count() {
                    0 => self.results.alloc(0),
                    _ => cur_node.result_handle()
                };
                let result_index = (cur_node.internal() >> (bitmap & trie::END_BIT_MASK).trailing_zeros()).count_ones();
                //println!("\tinserting result at {}", result_index);
                cur_node.set_internal(bitmap & trie::END_BIT_MASK);
                //println!("\tresult_hdl before insert: {:?}", result_hdl);
                self.results.insert(&mut result_hdl, result_index, value); // add result
                //println!("\tresult_hdl after insert:  {:?}", result_hdl);
                cur_node.result_ptr = result_hdl.offset;
                self.trienodes.set(&cur_hdl, cur_index, cur_node.clone()); // save trie node
                return;
            }
            // add a branch

            if cur_node.is_endnode() {
                // move any result pointers out of the way, so we can add branch
                //println!("\tbefore pushdown: {:?}", cur_node);
                self.push_down(&mut cur_node);
                //println!("\tafter pushdown: {:?}", cur_node);
            }
            let mut child_hdl = match cur_node.child_count() {
                0 => self.trienodes.alloc(0),
                _ => cur_node.child_handle()
            };

            //println!("\ttrienodes: {:#?}", self.trienodes);
            let child_index = (cur_node.external() >> bitmap.trailing_zeros()).count_ones();


            if cur_node.external() & (bitmap & trie::END_BIT_MASK) == 0 {
                //println!("\tadding branch");
                cur_node.set_external(bitmap & trie::END_BIT_MASK);
            } else {
                // follow existing branch
                if let MatchResult::Chase(child_hdl, index) = cur_node.match_segment(*nibble) {
                    bits_left -= 4;
                    cur_hdl = child_hdl;
                    cur_index = index;
                    continue;
                }
                unreachable!()
            }
            //println!("\tinsert child {}/{} nibble: {} at slot {}", ip, masklen, nibble, child_index);

            let mut child_node = TrieNode::new();
            child_node.make_endnode();
            //println!("\tchild_hdl before insert: {:?}", child_hdl);
            self.trienodes.insert(&mut child_hdl, child_index, child_node);
            //println!("\tchild_hdl after insert:  {:?}", child_hdl);
            cur_node.child_ptr = child_hdl.offset;
            self.trienodes.set(&cur_hdl, cur_index, cur_node.clone()); // save trie node

            cur_hdl = child_hdl;
            cur_index = child_index;
            bits_left -= 4;
        }
    }
}

#[cfg(test)]
mod test {
    extern crate test;
    use super::*;
    use self::test::{Bencher,black_box};
    use std::net::Ipv4Addr;
    use std::str::FromStr;
    use std::io::prelude::*;
    use std::io::{BufReader, Error};
    use std::fs::File;
    use std::mem;

    #[test]
    fn test_treebitmap_insert() {
        let mut tbm = TreeBitmap::<u32>::new();
        //  second quarter of internet: 64.0.0.0/2
        //tbm.insert(u32::from(Ipv4Addr::new(64,0,0,0)), u32::from(Ipv4Addr::new(192,0,0,0)), 4);

        //// default route
        //println!("{:#?}", tbm);
        tbm.insert(Ipv4Addr::new(0,0,0,0), 0, 100001);
        tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 100002);
        tbm.insert(Ipv4Addr::new(77,66,19,0), 24, 100003);
        tbm.insert(Ipv4Addr::new(77,66,19,0), 28, 100004);
        tbm.insert(Ipv4Addr::new(217,116,224,0), 19, 100005);
        //tbm.insert(u32::from(Ipv4Addr::new(240,0,0,0)), 4, 100003);
        println!("{:#?}", tbm.trienodes);
    }

    #[test]
    fn test_treebitmap_insert_bug() {
        let mut tbm = TreeBitmap::<u32>::new();
        tbm.insert(Ipv4Addr::new(1,48,0,0), 15, 1);
        tbm.insert(Ipv4Addr::new(1,50,0,0), 16, 2);
        tbm.insert(Ipv4Addr::new(1,51,0,0), 16, 3);
        tbm.insert(Ipv4Addr::new(1,51,64,0), 18, 3);
        tbm.insert(Ipv4Addr::new(1,52,0,0), 14, 4);
        tbm.insert(Ipv4Addr::new(1,52,0,0), 20, 4);
        println!("{:#?}", tbm);
    }

    #[test]
    fn test_treebitmap_longest_match() {
        let mut tbm = TreeBitmap::<u32>::new();
        //tbm.insert(Ipv4Addr::new(0,0,0,0), 0, 100001);
        //println!("{:#?}", tbm);
        tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 100002);
        //println!("{:#?}", tbm);
        //return;
        tbm.insert(Ipv4Addr::new(100,64,0,0), 24, 10064024);
        tbm.insert(Ipv4Addr::new(100,64,1,0), 24, 10064124);
        tbm.insert(Ipv4Addr::new(100,64,0,0), 10, 100004);
        //println!("{:#?}", tbm);
        let result = tbm.longest_match(Ipv4Addr::new(10,10,10,10));
        assert_eq!(result, Some((Ipv4Addr::new(10,0,0,0), 8, &100002)));
        //assert_eq!(result, Some((Ipv4Addr::new(10,0,0,0), 8, &100002)));
        let result = tbm.longest_match(Ipv4Addr::new(100,100,100,100));
        assert_eq!(result, Some((Ipv4Addr::new(100,64,0,0), 10, &100004)));
        let result = tbm.longest_match(Ipv4Addr::new(100,64,0,100));
        assert_eq!(result, Some((Ipv4Addr::new(100,64,0,0), 24, &10064024)));
        let result = tbm.longest_match(Ipv4Addr::new(200,200,200,200));
        //assert_eq!(result, Some((Ipv4Addr::new(0,0,0,0), 0, &100001)));
        assert_eq!(result, None);
        println!("{:#?}", tbm);
    }

    fn load_bgp_dump(limit: u32) -> Result<TreeBitmap<u32>, Error> {
        let mut tbm = TreeBitmap::<u32>::new();
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
                tbm.insert(ip, masklen, i);
            }
        }
        Ok(tbm)
    }

    #[test]
    fn test_load_full_bgp() {
        let tbm = load_bgp_dump(0).unwrap();
        let google_dns = Ipv4Addr::new(8,8,8,8);
        let (prefix, mask, val)= tbm.longest_match(google_dns).unwrap();
        println!("tbm trie memory usage: {} Kbytes", tbm.trienodes.mem_usage());
        println!("tbm result memory usage: {} Kbytes", tbm.results.mem_usage());
        println!("tbm.longest_match({}) -> {}/{} => {:?}", google_dns, prefix, mask, val);
        assert_eq!(prefix, Ipv4Addr::new(8,8,8,0));
        assert_eq!(mask, 24);
    }

    #[test]
    fn test_treebitmap_pushdown() {
        let mut tbm = TreeBitmap::<u32>::new();
        let mut result_hdl = tbm.results.alloc(0);
        let root_hdl = AllocatorHandle::generate(1, 0);
        let mut root_node = tbm.root_node();

        root_node.make_endnode();
        let node_count = 16;
        for i in 0..node_count {
            tbm.results.insert(&mut result_hdl, i, 100 + i as u32);
            root_node.set_internal(1<<(node_count - i - 1));
        }

        tbm.trienodes.set(&root_hdl, 0, root_node);
        println!("tbm before: {:#?}", tbm);

        tbm.push_down(&mut root_node);
        tbm.trienodes.set(&root_hdl, 0, root_node);
        println!("tbm after: {:#?}", tbm);
    }

    #[bench]
    fn bench_100k_bgp_lookup(b: &mut Bencher) {
        let tbm = load_bgp_dump(100_000).unwrap();
        let google_dns = Ipv4Addr::new(1,1,1,1);
        b.iter(|| {
            black_box(tbm.longest_match(google_dns));
        })
    }

    #[bench]
    fn bench_full_bgp_lookup(b: &mut Bencher) {
        let tbm = load_bgp_dump(0).unwrap();
        let google_dns = Ipv4Addr::new(8,8,8,8);
        b.iter(|| {
            black_box(tbm.longest_match(google_dns));
        })
    }

    //#[test]
    //fn test_treebitmap_insert() {
    //    let mut tbm = TreeBitmap::<u32>::new();
    //    tbm.insert(u32::from(Ipv4Addr::new(192,168,12,0)), 24);
    //    println!("{:#?}", tbm);
    //}

//    #[bench]
//    fn bench_bitmap(b: &mut Bencher) {
//        b.iter(||{
//            for item in &TEST_DATA {
//                let (mask, prefix) = *item;
//                test::black_box(bitmap(prefix, mask));
//            }
//        });
//    }

}
