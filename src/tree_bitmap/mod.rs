// Copyright 2016 Hroi Sigurdsson
//
// Licensed under the MIT license <LICENSE-MIT or http://opensource.org/licenses/MIT>.
// This file may not be copied, modified, or distributed except according to those terms.

use std::cmp;

mod node;
mod allocator;

use self::node::{Node, MatchResult};
use self::allocator::{Allocator, AllocatorHandle};

#[derive(Debug)]
pub struct TreeBitmap<T: Sized> {
    trienodes: Allocator<Node>,
    results: Allocator<T>,
}

impl<T: Sized> TreeBitmap<T> {

    /// Returns ````TreeBitmap ```` with 0 start capacity.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Returns ```TreeBitmap``` with pre-allocated buffers of size n.
    pub fn with_capacity(n: usize) -> Self {
        let mut trieallocator: Allocator<Node> = Allocator::with_capacity(n);
        let mut root_hdl = trieallocator.alloc(0);
        trieallocator.insert(&mut root_hdl, 0, Node::new());

        TreeBitmap {
            trienodes: trieallocator,
            results: Allocator::with_capacity(n),
        }
    }

    /// Returns handle to root node.
    fn root_handle(&self) -> AllocatorHandle {
        AllocatorHandle::generate(1, 0)
    }

    /// Returns the root node.
    #[cfg(test)]
    #[allow(dead_code)]
    fn root_node(&self) -> Node {
        self.trienodes.get(&self.root_handle(), 0).clone()
    }

    /// Push down results encoded in the last 16 bits into child trie nodes. Makes ```node``` into a normal node.
    fn push_down(&mut self, node: &mut Node) {
        debug_assert!(node.is_endnode(), "push_down: not an endnode");
        debug_assert!(node.child_ptr == 0);
        // count number of internal nodes in the first 15 bits (those that will remain in place).
        let internal_node_count = (node.internal() & 0xffff0000).count_ones();
        let remove_at = internal_node_count;
        // count how many nodes to push down
        //let nodes_to_pushdown = (node.bitmap & 0x0000ffff).count_ones();
        let nodes_to_pushdown = (node.internal() & 0x0000ffff).count_ones();
        if nodes_to_pushdown > 0 {
            let mut result_hdl = node.result_handle();
            let mut child_node_hdl = self.trienodes.alloc(0);

            for _ in 0..nodes_to_pushdown {
                // allocate space for child result value
                let mut child_result_hdl = self.results.alloc(0);
                // put result value in freshly allocated bucket
                let result_value = self.results.remove(&mut result_hdl, remove_at);
                self.results.insert(&mut child_result_hdl, 0, result_value);
                // create and save child node
                let mut child_node = Node::new();
                child_node.set_internal(1<<31);
                child_node.result_ptr = child_result_hdl.offset;
                // append trienode to collection
                let insert_node_at = child_node_hdl.len;
                self.trienodes.insert(&mut child_node_hdl, insert_node_at, child_node);
            }
            // the result data may have moved to a smaller bucket, update the result pointer
            node.result_ptr = result_hdl.offset;
            node.child_ptr = child_node_hdl.offset;
            // no results from this node remain, free the result slot
            if internal_node_count == 0 && nodes_to_pushdown > 0 {
                self.results.free(&mut result_hdl);
                node.result_ptr = 0;
            }
        }
        node.make_normalnode();
        // note: we do not need to touch the external bits, they are correct as are
    }

    /// longest match lookup of ```nibbles```. Returns bits matched as u32, and reference to T.
    pub fn longest_match(&self, nibbles: &[u8]) -> Option<(u32, &T)> {
        let mut cur_hdl = self.root_handle();
        let mut cur_index = 0;
        let mut bits_matched = 0;
        let mut bits_searched = 0;
        let mut best_match : Option<(AllocatorHandle, u32)> = None; // result handle + index

        for nibble in nibbles {
            let cur_node = self.trienodes.get(&cur_hdl, cur_index).clone();
            let match_mask = unsafe {*node::MATCH_MASKS.get_unchecked(*nibble as usize)};

            match cur_node.match_internal(match_mask) {
                MatchResult::Match(result_hdl, result_index, matching_bit_index) => {
                    bits_matched = bits_searched;
                    unsafe {
                        bits_matched += *node::BIT_MATCH.get_unchecked(matching_bit_index as usize);
                    }
                    best_match = Some((result_hdl, result_index));
                },
                _ => ()
            }

            if cur_node.is_endnode() {
                break;
            }
            match cur_node.match_external(match_mask) {
                MatchResult::Chase(child_hdl, child_index) => {
                    bits_searched += 4;
                    cur_hdl = child_hdl;
                    cur_index = child_index;
                    continue;
                },
                MatchResult::None => {
                    break;
                },
                _ => unreachable!()
            }
        }

        match best_match {
            Some((result_hdl, result_index)) => {
                //debug_assert!(bits_matched <= 32, format!("matched {} bits?", bits_matched));
                return Some((bits_matched, self.results.get(&result_hdl, result_index)));
            },
            None => return None,
        }
    }


    pub fn insert(&mut self, nibbles: &[u8], masklen: u32, value: T) -> Option<T> {
        let mut cur_hdl = self.root_handle();
        let mut cur_index = 0;
        let mut bits_left = masklen;
        let mut ret = None;

        let mut loop_count = 0;
        loop {
            let nibble = if loop_count < nibbles.len() {
                nibbles[loop_count]
            } else {
                0
            };
            loop_count += 1;
            let nibble = &nibble;

            let mut cur_node = self.trienodes.get(&cur_hdl, cur_index).clone();
            let match_result = cur_node.match_segment(*nibble);

            if let MatchResult::Chase(child_hdl, index) = match_result {
                if bits_left >= 4 {
                    // follow existing branch
                    bits_left -= 4;
                    cur_hdl = child_hdl;
                    cur_index = index;
                    continue;
                }
            }

            let bitmap = node::gen_bitmap(*nibble, cmp::min(4, bits_left));

            if (cur_node.is_endnode() && bits_left <= 4) || bits_left <= 3 {
                // final node reached, insert results
                let mut result_hdl = match cur_node.result_count() {
                    0 => self.results.alloc(0),
                    _ => cur_node.result_handle()
                };
                let result_index = (cur_node.internal() >> (bitmap & node::END_BIT_MASK).trailing_zeros()).count_ones();

                if cur_node.internal() & (bitmap & node::END_BIT_MASK) > 0 {
                    // key already exists!
                    ret = Some(self.results.replace(&mut result_hdl, result_index - 1, value));
                } else {
                    cur_node.set_internal(bitmap & node::END_BIT_MASK);
                    self.results.insert(&mut result_hdl, result_index, value); // add result
                }
                cur_node.result_ptr = result_hdl.offset;
                self.trienodes.set(&cur_hdl, cur_index, cur_node.clone()); // save trie node
                return ret;
            }
            // add a branch

            if cur_node.is_endnode() {
                // move any result pointers out of the way, so we can add branch
                self.push_down(&mut cur_node);
            }
            let mut child_hdl = match cur_node.child_count() {
                0 => self.trienodes.alloc(0),
                _ => cur_node.child_handle()
            };

            let child_index = (cur_node.external() >> bitmap.trailing_zeros()).count_ones();

            if cur_node.external() & (bitmap & node::END_BIT_MASK) == 0 {
                // no existing branch; create it
                cur_node.set_external(bitmap & node::END_BIT_MASK);
            } else {
                // follow existing branch
                if let MatchResult::Chase(child_hdl, index) = cur_node.match_segment(*nibble) {
                    self.trienodes.set(&cur_hdl, cur_index, cur_node.clone()); // save trie node
                    bits_left -= 4;
                    cur_hdl = child_hdl;
                    cur_index = index;
                    continue;
                }
                unreachable!()
            }

            // prepare a child node
            let mut child_node = Node::new();
            child_node.make_endnode();
            self.trienodes.insert(&mut child_hdl, child_index, child_node); // save child
            cur_node.child_ptr = child_hdl.offset;
            self.trienodes.set(&cur_hdl, cur_index, cur_node.clone()); // save trie node

            bits_left -= 4;
            cur_hdl = child_hdl;
            cur_index = child_index;
        }
    }

    pub fn mem_usage(&self) -> (usize,usize) {
        let node_bytes = self.trienodes.mem_usage();
        let result_bytes = self.results.mem_usage();
        (node_bytes, result_bytes)
    }

    pub fn exact_match(&self, nibbles: &[u8], masklen: u32) -> Option<&T> {
        let mut cur_hdl = self.root_handle();
        let mut cur_index = 0;
        let mut bits_left = masklen;

        for nibble in nibbles {
            println!("nibble: {}, bits_left: {}", nibble, bits_left);
            let cur_node = self.trienodes.get(&cur_hdl, cur_index).clone();
            let bitmap = node::gen_bitmap(*nibble, cmp::min(bits_left, 4)) & node::END_BIT_MASK;
            let reached_final_node = bits_left < 4 || (cur_node.is_endnode() && bits_left == 4);

            if reached_final_node {
                match cur_node.match_internal(bitmap) {
                    MatchResult::Match(mut result_hdl, result_index, _) => {
                        return Some(self.results.get(&mut result_hdl, result_index));
                    },
                    _ => return None
                }
            }

            match cur_node.match_external(bitmap) {
                MatchResult::Chase(child_hdl, child_index) => {
                    cur_hdl = child_hdl;
                    cur_index = child_index;
                    bits_left -= 4;
                },
                _ => return None
            }
        }
        None
    }

    /// Remove prefix. Returns existing value if the prefix previously existed.
    pub fn remove(&mut self, nibbles: &[u8], masklen: u32) -> Option<T> {
        debug_assert!(nibbles.len() >= (masklen/4) as usize);
        let mut root_hdl = self.root_handle();
        let mut root_node = self.trienodes.get(&root_hdl, 0).clone();
        let ret = self.remove_child(&mut root_node, nibbles, masklen);
        self.trienodes.set(&mut root_hdl, 0, root_node.clone());
        ret
    }

    // remove child and result from node
    fn remove_child(&mut self, node: &mut Node, nibbles: &[u8], masklen: u32) -> Option<T>{
        let nibble = nibbles[0];
        let bitmap = node::gen_bitmap(nibble, cmp::min(masklen, 4)) & node::END_BIT_MASK;
        let reached_final_node = masklen < 4 || (node.is_endnode() && masklen == 4);

        if reached_final_node {
            match node.match_internal(bitmap) {
                MatchResult::Match(mut result_hdl, result_index, _) => {
                    node.unset_internal(bitmap);
                    let ret = self.results.remove(&mut result_hdl, result_index);
                    if node.result_count() == 0 {
                        self.results.free(&mut result_hdl);
                    }
                    node.result_ptr = result_hdl.offset;
                    return Some(ret);
                },
                _ => return None
            }
        }

        match node.match_external(bitmap) {
            MatchResult::Chase(mut child_node_hdl, index) => {
                let mut child_node = self.trienodes.get(&child_node_hdl, index).clone();
                let ret = self.remove_child(&mut child_node, &nibbles[1..], masklen - 4);

                if child_node.child_count() == 0 && !child_node.is_endnode() {
                    child_node.make_endnode();
                }
                if child_node.is_empty() {
                    self.trienodes.remove(&mut child_node_hdl, 0);
                    self.trienodes.free(&mut child_node_hdl);
                    self.trienodes.set(&mut child_node_hdl, 0, Node::new());
                    node.unset_external(bitmap);
                    node.child_ptr = 0;
                } else {
                    node.child_ptr = child_node_hdl.offset;
                    self.trienodes.set(&mut child_node_hdl, index, child_node.clone());
                }
                return ret;
            },
            _ => return None
        }
    }

}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn test_treebitmap_remove() {
        let mut tbm: TreeBitmap<&str> = TreeBitmap::new();
        let (nibbles_a, mask_a) = (&[0, 10, 0,  0, 0,  0, 0, 0], 8);
        let (nibbles_b, mask_b) = (&[0, 10, 0, 10, 0, 10, 0, 0], 24);
        tbm.insert(nibbles_a, mask_a, "foo");
        tbm.insert(nibbles_b, mask_b, "bar");
        {
            let value = tbm.remove(nibbles_b, mask_b);
            assert_eq!(value, Some("bar"));
            let lookup_result = tbm.longest_match(nibbles_b);
            assert_eq!(lookup_result, Some((mask_a, &"foo")));
        }
        // foo should not exist, and therefore return None
        let value = tbm.remove(nibbles_b, mask_b);
        assert_eq!(value, None);
    }
}
