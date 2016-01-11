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

//#[derive(Clone,Copy,Debug)]
//pub struct TrieNode_ {
//    internal: u16,
//    // the external bitmap encodes the lower branches from this node. if bit N is set, there is a branch down for value N.
//    external: u16,
//    child_ptr: u32,
//    result_ptr: u32,
//}

// meanings of each bit in internal bitmap
// position  0    1    2    3     4     5     6     7      8       9     10     11     12     13     14      15
// meaning | * | 0* | 1* | 00* | 01* | 10* | 11* | 000* | 001* | 010* | 011* | 100* | 101* | 110* | 111* | is endnode |
//
// optimisation: if the endnode bit is set the *external* bitmap becomes an extension of the internal bitmap:
// position    0       1       2       3       4       5       6       7       8       9       10      11      12      13     14      15
// meaning | 0000* | 0001* | 0010* | 0011* | 0100* | 0101* | 0110* | 0111* | 1000* | 1001* | 1010* | 1011* | 1100* | 1101* | 1110* | 1111*|

//pub fn gen_bitmaps(prefix: u8, mask: u8) -> (u16,u16) {
//    assert!(prefix < 16); // only nibbles allowed
//    assert!(mask < 16);
//    assert_eq!(prefix & mask, prefix); // ensure no bits set past mask
//    let res = INTERNAL_LOOKUP_TABLE[mask.count_ones() as usize][prefix as usize];
//    ((res >> 16) as u16, (res & 0xffff) as u16)
//    //unsafe {
//    //    let res = *INTERNAL_LOOKUP_TABLE.get_unchecked(mask.count_ones() as usize).get_unchecked(prefix as usize);
//    //    //((res >> 16) as u16, (res & 0xffff) as u16)
//    //    assert!(res > 0); // invalid input was given
//    //    (res.wrapping_shr(16) as u16, (res & 0xffff) as u16)
//    //}
//}

//impl TrieNode {
//    fn new() -> TrieNode {
//        TrieNode{
//            internal: 0,
//            external: 0,
//            child_ptr: 0,
//            result_ptr: 0,
//        }
//    }
//
//    // if rightmost internal bit is set, it means that the external bitmap becomes part of the internal bitmap
//    fn is_endnode(self) -> bool {
//        self.internal & 1 > 0
//    }
//
//    // panics if called on a node that is already an endnode
//    fn make_endnode(&mut self) {
//        assert_eq!(self.external, 0); // make sure there are no external pointers
//        self.internal |= 1;
//    }
//
//    // first 16 bits are for normal nodes
//    // last 16 bits are for end nodes
//    fn set_internal_bit(&mut self, bits: u32) {
//        // are we required to be an endnode?
//        if (bits >> 17) > 0 {
//            assert!(self.is_endnode());
//        }
//    }
//}

#[derive(Debug)]
pub struct TreeBitmap<T: Sized> {
    //rootnode: TrieNode,
    trienodes: Allocator<TrieNode>,
    results: Allocator<T>,
}

// convert prefix nibble p and mask nibble into (internal, external) bitmasks
//pub fn bitmap(prefix: u8, mask: u8) -> (u16,u16) {
//    assert!(prefix < 16 && mask < 16); // must be nibbles
//    assert_eq!(prefix & mask, prefix);      // not bits set in p past mask
//    let (internal, external) = match mask {
//        0b0000 => (0x8000                  , 0),
//        0b1000 => (0x8000 >> ((prefix >> 3) + 1), 0),
//        0b1100 => (0x8000 >> ((prefix >> 2) + 3), 0),
//        0b1110 => (0x8000 >> ((prefix >> 1) + 7), 0),
//        0b1111 => (0                       , 0x8000 >> prefix),
//        _ => panic!("invalid mask"),
//    };
//    (internal, external)
//}

// me: u16 with only one bit set
// them: u16 with any bits set
// result: how many bits are set to the left of my u16 in their u16
//fn bit_position(me: u16, them: u16) -> u32 {
//    assert_eq!(me.count_ones(), 1);
//    let which_bit = me.leading_zeros();
//    let bitmask = !(!0 >> which_bit);
//    (them & bitmask).count_ones()
//}

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
        //let mut root_node =
        TreeBitmap {
            //rootnode: rootnode,
            trienodes: trieallocator,
            results: Allocator::new(),
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
        // alloc handle for _this_ node
        let mut node_hdl = AllocatorHandle::generate(node.result_count(), node.result_ptr);
        // go to work
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
        // the result data may have moved to a smaller bucket, update the result pointer
        node.result_ptr = node_hdl.offset as u32;
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

        let mut cur_node = self.root_node();
        let mut bits_matched = 0;
        let mut mask: u32 = 0;
        'nibbles:
        for nibble in &nibbles {
            mask = match bits_matched {
                0 => 0,
                _ => (!0 << 32 - bits_matched)
            };
            println!("\tat {}/{}", Ipv4Addr::from(ip & mask), bits_matched);
            match cur_node.match_segment(*nibble) {
                MatchResult::Chase(child_hdl, offset) => {
                    bits_matched += 4;
                    cur_node = self.trienodes.get(&child_hdl, offset);
                },
                MatchResult::Match(result_hdl, offset, bits) => {
                    bits_matched += bits;
                    break 'nibbles;
                    //return (cur_node, depth);
                    // insert nodes from here
                },
                MatchResult::None => {
                    break 'nibbles;
                    //return (cur_node, depth);
                }
            }
        }
        println!("\tmatch {}/{}", Ipv4Addr::from(ip & mask), bits_matched);
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
        tbm.insert(u32::from(Ipv4Addr::new(0,0,0,0)), u32::from(Ipv4Addr::new(0,0,0,0)), 1);

        //// last half of internet: 128.0.0.0/1
        //tbm.insert(u32::from(Ipv4Addr::new(128,0,0,0)), u32::from(Ipv4Addr::new(128,0,0,0)), 3);

        //// first half of internet: 0.0.0.0/1
        //tbm.insert(u32::from(Ipv4Addr::new(0,0,0,0)), u32::from(Ipv4Addr::new(128,0,0,0)), 2);

        ////  class E: 240.0.0.0/4
        //tbm.insert(u32::from(Ipv4Addr::new(240,0,0,0)), u32::from(Ipv4Addr::new(240,0,0,0)), 5);

        //tbm.insert(u32::from(Ipv4Addr::new(10,10,10,12)), u32::from(Ipv4Addr::new(255,255,255,252)), 10101012);
        //tbm.insert(u32::from(Ipv4Addr::new(10,10,10,0)), u32::from(Ipv4Addr::new(255,255,255,252)), 10101000);
        //tbm.insert(u32::from(Ipv4Addr::new(10,10,10,4)), u32::from(Ipv4Addr::new(255,255,255,252)), 10101004);
        //tbm.insert(u32::from(Ipv4Addr::new(10,10,10,8)), u32::from(Ipv4Addr::new(255,255,255,252)), 10101008);

        //tbm.insert(u32::from(Ipv4Addr::new(77,66,19,0)), u32::from(Ipv4Addr::new(255,255,255,224)), 27);
        //tbm.insert(u32::from(Ipv4Addr::new(77,66,19,0)), u32::from(Ipv4Addr::new(255,255,255,248)), 2900);
        //tbm.insert(u32::from(Ipv4Addr::new(77,66,19,4)), u32::from(Ipv4Addr::new(255,255,255,252)), 2904);
        //tbm.insert(u32::from(Ipv4Addr::new(77,66,19,8)), u32::from(Ipv4Addr::new(255,255,255,248)), 2908);
        //tbm.insert(u32::from(Ipv4Addr::new(77,66,19,16)), u32::from(Ipv4Addr::new(255,255,255,240)), 28);
        //tbm.insert(u32::from(Ipv4Addr::new(217,116,224,0)), u32::from(Ipv4Addr::new(255,255,240,0)), 19);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,0,0)), u32::from(Ipv4Addr::new(255,255,0,0)), 8800);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,0)), u32::from(Ipv4Addr::new(255,255,255,0)), 8880);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,4,0)), u32::from(Ipv4Addr::new(255,255,255,0)), 8840);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,8)), u32::from(Ipv4Addr::new(255,255,255,254)), 8888);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,0)), u32::from(Ipv4Addr::new(255,255,255,255)), 8880);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,1)), u32::from(Ipv4Addr::new(255,255,255,255)), 8881);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,2)), u32::from(Ipv4Addr::new(255,255,255,255)), 8882);
        //tbm.insert(u32::from(Ipv4Addr::new(8,8,8,3)), u32::from(Ipv4Addr::new(255,255,255,255)), 8883);

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
