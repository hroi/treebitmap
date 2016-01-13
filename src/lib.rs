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
        self.trienodes.get(&self.root_hdl(), 0)
    }

    /// Push down results encoded in the last 16 bits into child trie nodes.
    pub fn push_down(&mut self, node: &mut TrieNode) {
        assert!(node.is_endnode(), "push_down: not an endnode");
        // count number of internal nodes in the first 15 bits (those that will remain in place).
        let remove_at = (node.internal() & 0xffff0000).count_ones();
        // count how many nodes to push down
        let nodes_to_pushdown = (node.bitmap & 0x0000ffff).count_ones();
        if nodes_to_pushdown > 0 {
            // alloc handle for _this_ node
            let mut node_hdl = AllocatorHandle::generate(node.result_count(), node.result_ptr);
            let mut child_node_hdl = self.trienodes.alloc(0); // allocate space for the child trie nodes
            for i in 0..nodes_to_pushdown {
                // allocate space for child result value
                let mut child_result_hdl = self.results.alloc(0);
                // put result value in freshly allocated bucket
                println!("{} node.result_count(): {}, remove_at: {}", i, node.result_count(), remove_at);
                let result_value = self.results.remove(&mut node_hdl, remove_at);
                self.results.insert(&mut child_result_hdl, 0, result_value);
                // create and save child node
                let mut child_node = TrieNode::new();
                child_node.set_internal(1<<31);
                child_node.result_ptr = child_result_hdl.offset as u32;
                // append trienode to collection
                let insert_node_at = child_node_hdl.len;
                self.trienodes.insert(&mut child_node_hdl, insert_node_at, child_node);
            }
            node.result_ptr = node_hdl.offset as u32;
        }
        // the result data may have moved to a smaller bucket, update the result pointer
        // done!
        node.make_normalnode();
        // note: we do not need to touch the external bits, they are correct as are
    }

    /// search trie, return the last trienode in the path and it's depth
    //pub fn search(&self, ip: u32) -> (TrieNode, usize) {
    //pub fn search(&self, ip: u32) -> (AllocatorHandle, u32, usize) {
    //    let nibbles   = ip.nibbles();
    //    let mut cur_node = self.root_node();
    //    let mut depth = 0;
    //    for nibble in &nibbles {
    //        match cur_node.match_segment(*nibble) {
    //            MatchResult::Chase(child_hdl, offset) => {
    //                cur_node = self.trienodes.get(&child_hdl, offset);
    //            },
    //            MatchResult::Match(result_hdl, offset) => {
    //                return (cur_node, depth);
    //            },
    //            MatchResult::None =>  {
    //                return (cur_node, depth);
    //            },
    //        }
    //        depth += 1;
    //    }
    //    (cur_node, depth)
    //}

    /// returns any existing T set for key
    pub fn insert(&mut self, ip: u32, masklen: u32, value: T) {
        let nibbles = ip.nibbles();
        println!("insert(ip: {}, masklen: {})", ip, masklen);
        // Actions:
        // find insertion point.
        // if needed, pushdown internal nodes
        //

        let mut cur_hdl = self.root_hdl();
        let mut cur_slot = 0;
        //let mut cur_node = self.root_node();
        //let mut cur_nibble = 0;
        let mut bits_left = masklen;
        //let mut mask: u32 = 0;
        for nibble in &nibbles {
            let mut cur_node = self.trienodes.get(&cur_hdl, cur_slot);
            let match_result = cur_node.match_segment(*nibble);

            println!("cur_node: #{:#?}", cur_node);
            println!("match_result: #{:?}", match_result);
            match match_result {
                MatchResult::Chase(child_hdl, offset) => {
                    bits_left -= 4;
                    cur_hdl = child_hdl;
                    cur_slot = offset;
                    continue;
                },
                MatchResult::Match(mut result_hdl, offset, bits) => {
                    bits_left -= bits;
                    if bits_left == 0 { // exact match
                        println!("exact match");
                        self.results.set(&result_hdl, offset, value);
                        return;
                    } else { // less specific match
                        println!("less specific match");
                        let bitmap = trie::gen_bitmap(*nibble, 4);
                        cur_node.set_internal(bitmap & trie::END_BIT_MASK);
                        self.results.insert(&mut result_hdl, offset, value); // add result
                        cur_node.result_ptr == result_hdl.offset;
                        self.trienodes.set(&cur_hdl, cur_slot, cur_node); // save trie node
                        return;
                    }
                },
                MatchResult::None => {
                    println!("\t{} more bits to insert.", bits_left);
                    let bitmap = trie::gen_bitmap(*nibble, std::cmp::min(4, bits_left));
                    //println!("bitmap: {:032b}", bitmap);

                    if cur_node.is_endnode() && bits_left > 4 {
                        println!("insert: pushing down nodes");
                        self.push_down(&mut cur_node);
                    }

                    if (cur_node.is_endnode() && bits_left <= 4) || bits_left <= 3 { // final node reached
                        let mut result_hdl = cur_node.result_handle();
                        cur_node.set_internal(bitmap & trie::END_BIT_MASK);
                        println!("result_hdl before insert: {:?}", result_hdl);
                        self.results.insert(&mut result_hdl, cur_slot, value); // add result
                        println!("result_hdl after insert: {:?}", result_hdl);
                        cur_node.result_ptr = result_hdl.offset;
                        if cur_node.child_count() == 0 && !cur_node.is_endnode() {
                            cur_node.make_endnode();
                        }
                        self.trienodes.set(&cur_hdl, cur_slot, cur_node); // save trie node
                        return;
                    } else {
                        let mut child_hdl = match cur_node.child_count() {
                            0 => self.trienodes.alloc(0),
                            _ => cur_node.child_handle()
                        };
                        cur_node.set_external(bitmap & trie::END_BIT_MASK);
                        let mut child_node = TrieNode::new();
                        //let mut child_hdl = self.trienodes.alloc(0);
                        child_node.make_endnode();
                        self.trienodes.insert(&mut child_hdl, 0, child_node);
                        //cur_node.set_external(bitmap);
                        cur_node.child_ptr = child_hdl.offset;
                        self.trienodes.set(&cur_hdl, cur_slot, cur_node); // save trie node

                        cur_hdl = child_hdl;
                        cur_slot = 0;
                        bits_left -= 4;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    extern crate test;
    use super::*;
    use self::test::{Bencher,black_box};
    use std::net::Ipv4Addr;

    #[test]
    fn test_treebitmap_insert() {
        let mut tbm = TreeBitmap::<u32>::new();
        //  second quarter of internet: 64.0.0.0/2
        //tbm.insert(u32::from(Ipv4Addr::new(64,0,0,0)), u32::from(Ipv4Addr::new(192,0,0,0)), 4);

        //// default route
        println!("{:#?}", tbm);
        //tbm.insert(u32::from(Ipv4Addr::new(0,0,0,0)), 0, 100001);
        //tbm.insert(u32::from(Ipv4Addr::new(10,0,0,0)), 8, 100002);
        tbm.insert(u32::from(Ipv4Addr::new(77,66,19,0)), 24, 100003);
        tbm.insert(u32::from(Ipv4Addr::new(217,116,224,0)), 19, 100004);
        tbm.insert(u32::from(Ipv4Addr::new(240,0,0,0)), 4, 100003);
        println!("{:#?}", tbm);

        //// last half of internet: 128.0.0.0/1
        //tbm.insert(u32::from(Ipv4Addr::new(128,0,0,0)), u32::from(Ipv4Addr::new(128,0,0,0)), 3);

        //// first half of internet: 0.0.0.0/1
        //tbm.insert(u32::from(Ipv4Addr::new(0,0,0,0)), u32::from(Ipv4Addr::new(128,0,0,0)), 2);

        ////  class E: 240.0.0.0/4

        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,0)), u32::from(Ipv4Addr::new(255,255,255,254)), 88880);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,2)), u32::from(Ipv4Addr::new(255,255,255,254)), 88882);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,4)), u32::from(Ipv4Addr::new(255,255,255,254)), 88884);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,8)), u32::from(Ipv4Addr::new(255,255,255,254)), 88888);

        //println!("{:#?}", tbm);

        // conflicting
        //assert_eq!(tbm.insert(u32::from(Ipv4Addr::new(0,0,0,0)), u32::from(Ipv4Addr::new(0,0,0,0)), 7).unwrap(), 1);
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
