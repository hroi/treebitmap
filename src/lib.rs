#![feature(test)]
#![feature(alloc)]

//!
//! A datastructure for fast IP lookups in software. Based on Tree-bitmap algorithm described by W. Eatherton, Z. Dittia, G. Varghes.
//!

extern crate alloc; // for RawVec
extern crate test;
use std::net::Ipv4Addr;

mod allocator_raw;
use allocator_raw::{Allocator, AllocatorHandle};

mod trie;
use trie::{TrieNode, MatchResult};

mod nibbles;
use nibbles::Nibbles;

pub struct TreeBitmap<T: Sized> {
    trienodes: Allocator<TrieNode>,
    results: Allocator<T>,
}

impl<T: Sized> TreeBitmap<T> {

    /// Returns ````TreeBitmap ```` with 0 start capacity.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Returns ```TreeBitmap``` with pre-allocated buffers of size n.
    pub fn with_capacity(n: usize) -> Self {
        let mut trieallocator: Allocator<TrieNode> = Allocator::with_capacity(n);
        let mut root_hdl = trieallocator.alloc(0);
        trieallocator.insert(&mut root_hdl, 0, TrieNode::new());

        let mut resultsallocator: Allocator<T> = Allocator::with_capacity(n);
        resultsallocator.alloc(0);
        TreeBitmap {
            trienodes: trieallocator,
            results: resultsallocator,
        }
    }

    /// Returns handle to root node.
    fn root_hdl(&self) -> AllocatorHandle {
        AllocatorHandle::generate(1, 0)
    }

    /// Returns the root node.
    #[cfg(test)]
    fn root_node(&self) -> TrieNode {
        self.trienodes.get(&self.root_hdl(), 0).clone()
    }

    /// Push down results encoded in the last 16 bits into child trie nodes. Makes ```node``` into a normal node.
    fn push_down(&mut self, node: &mut TrieNode) {
        debug_assert!(node.is_endnode(), "push_down: not an endnode");
        debug_assert!(node.child_ptr == 0);
        // count number of internal nodes in the first 15 bits (those that will remain in place).
        let remove_at = (node.internal() & 0xffff0000).count_ones();
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

    /// longest match lookup of ```ip```. Returns matched ip as Ipv4Addr, bits matched as u32, and reference to T.
    pub fn longest_match(&self, ip: Ipv4Addr) -> Option<(Ipv4Addr, u32, &T)> {
        //println!("longest_match(ip: {})", ip);
        let ip = u32::from(ip);
        let nibbles = ip.nibbles();
        let mut cur_hdl = self.root_hdl();
        let mut cur_index = 0;
        let mut bits_matched = 0;
        let mut bits_searched = 0;
        let mut best_match : Option<(AllocatorHandle, u32)> = None; // result handle + index
        for nibble in &nibbles {
            let cur_node = self.trienodes.get(&cur_hdl, cur_index).clone();
            let match_mask = unsafe {*trie::MATCH_MASKS.get_unchecked(*nibble as usize)};
            //println!("  nibble: {} - {:04b}   cur_hld: {:?}", nibble, nibble, cur_hdl);
            //if cur_node.is_endnode() {
            //    //println!("  internal:  {:016b} {:016b} (endnode)", cur_node.internal() >> 16,
            //    //         cur_node.internal() & 0x0000ffff);
            //} else {
            //    //println!("  int/ext:   {:015b}- {:016b}", cur_node.internal() >> 17,
            //    //         cur_node.external() & 0x0000ffff );
            //}
            //println!("  matchmask: {:016b} {:016b}", match_mask >> 16, match_mask & 0x0000ffff);
            match cur_node.match_internal(match_mask) {
                MatchResult::Match(result_hdl, result_index, matching_bit_index) => {
                    //println!("  internal match at bit index {}, the {}th set bit. result_ptr: {}", matching_bit_index, result_index, cur_node.result_ptr);
                    bits_matched = bits_searched;
                    unsafe {
                        bits_matched += *trie::BIT_MATCH.get_unchecked(matching_bit_index as usize);
                    }
                    best_match = Some((result_hdl, result_index));
                },
                _ => ()
            }

            match cur_node.match_external(match_mask) {
                MatchResult::Chase(child_hdl, child_index) => {
                    //println!("  child found at {} from the left. child_ptr: {}", child_index, cur_node.child_ptr);
                    bits_searched += 4;
                    cur_hdl = child_hdl;
                    cur_index = child_index;
                    //println!("");
                    continue;
                },
                MatchResult::None => {
                    match best_match {
                        Some((result_hdl, result_index)) => {
                            debug_assert!(bits_matched <= 32, format!("{} matched {} bits?", Ipv4Addr::from(ip), bits_matched));
                            //println!("");
                            let masked_ip = match bits_matched {
                                0 => 0,
                                32 => ip,
                                _ => ip & (!0 << (32 - bits_matched))
                            };
                            return Some((Ipv4Addr::from(masked_ip), bits_matched,
                                         self.results.get(&result_hdl, result_index)));
                        },
                        None => return None,
                    }
                },
                _ => unreachable!()
            }
        }
        unreachable!()
    }

    /// returns any existing T set for key
    pub fn insert(&mut self, ip: Ipv4Addr, masklen: u32, value: T) {
        let nibbles = u32::from(ip).nibbles();
        let mut cur_hdl = self.root_hdl();
        let mut cur_index = 0;
        let mut bits_left = masklen;

        for nibble in &nibbles {

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

            let bitmap = trie::gen_bitmap(*nibble, std::cmp::min(4, bits_left));

            if (cur_node.is_endnode() && bits_left <= 4) || bits_left <= 3 {
                // final node reached, insert results
                let mut result_hdl = match cur_node.result_count() {
                    0 => self.results.alloc(0),
                    _ => cur_node.result_handle()
                };
                let result_index = (cur_node.internal() >> (bitmap & trie::END_BIT_MASK).trailing_zeros()).count_ones();

                cur_node.set_internal(bitmap & trie::END_BIT_MASK);
                self.results.insert(&mut result_hdl, result_index, value); // add result
                cur_node.result_ptr = result_hdl.offset;
                self.trienodes.set(&cur_hdl, cur_index, cur_node.clone()); // save trie node
                return;
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

            if cur_node.external() & (bitmap & trie::END_BIT_MASK) == 0 {
                // no existing branch; create it
                cur_node.set_external(bitmap & trie::END_BIT_MASK);
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
            let mut child_node = TrieNode::new();
            child_node.make_endnode();
            self.trienodes.insert(&mut child_hdl, child_index, child_node); // save child
            cur_node.child_ptr = child_hdl.offset;
            self.trienodes.set(&cur_hdl, cur_index, cur_node.clone()); // save trie node

            bits_left -= 4;
            cur_hdl = child_hdl;
            cur_index = child_index;
        }
    }

    pub fn shrink_to_fit(&mut self) {
        self.trienodes.shrink_to_fit();
        self.results.shrink_to_fit();
    }
}

#[cfg(test)]
mod tests {
    extern crate rand;
    use self::rand::{Rng};

    use super::*;
    use test::{Bencher,black_box};
    use std::net::Ipv4Addr;
    use std::str::FromStr;
    use std::io::prelude::*;
    use std::io::{BufReader, Error};
    use std::fs::File;

    #[test]
    fn test_treebitmap_insert() {
        let mut tbm = TreeBitmap::<u32>::new();
        tbm.insert(Ipv4Addr::new(0,0,0,0), 0, 100001);
        tbm.insert(Ipv4Addr::new(10,0,0,0), 8, 100002);
        tbm.insert(Ipv4Addr::new(77,66,19,0), 24, 100003);
        tbm.insert(Ipv4Addr::new(77,66,19,0), 28, 100004);
        tbm.insert(Ipv4Addr::new(217,116,224,0), 19, 100005);
    }

    #[test]
    fn test_treebitmap_longest_match() {
        let mut tbm = TreeBitmap::<u32>::new();
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

    fn load_bgp_dump_light(limit: u32) -> Result<TreeBitmap<()>, Error> {
        let mut tbm = TreeBitmap::<()>::with_capacity(512);
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
                tbm.insert(ip, masklen, ());
            }
        }
        tbm.shrink_to_fit();
        Ok(tbm)
    }

    fn load_bgp_dump(limit: u32) -> Result<TreeBitmap<(Ipv4Addr, u32)>, Error> {
        let mut tbm = TreeBitmap::<(Ipv4Addr,u32)>::with_capacity(512);
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
        tbm.shrink_to_fit();
        Ok(tbm)
    }

    #[test]
    fn test_load_full_bgp() {
        let tbm = load_bgp_dump_light(0).unwrap();
        let google_dns = Ipv4Addr::new(8,8,8,8);
        let (prefix, mask, val)= tbm.longest_match(google_dns).unwrap();
        let (allocated, wasted) = tbm.trienodes.mem_usage();
        println!("Tree-bitmap node memory: {} bytes allocated, {} bytes wasted", allocated, wasted);
        let (allocated, wasted) = tbm.results.mem_usage();
        println!("Tree-bitmap result memory: {} bytes allocated, {} bytes wasted", allocated, wasted);
        println!("tbm.longest_match({}) -> {}/{} => {:?}", google_dns, prefix, mask, val);
        assert_eq!(prefix, Ipv4Addr::new(8,8,8,0));
        assert_eq!(mask, 24);
    }

    #[test]
    fn test_treebitmap_lookup_all_the_things() {
        let tbm = load_bgp_dump(0).unwrap();
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
    fn test_treebitmap_pushdown() {
        let mut tbm = TreeBitmap::<u32>::new();
        let mut result_hdl = tbm.results.alloc(0);
        let root_hdl = tbm.root_hdl();
        let mut root_node = tbm.root_node();

        root_node.make_endnode();
        let node_count = 16;
        for i in 0..node_count {
            tbm.results.insert(&mut result_hdl, i, 100 + i as u32);
            root_node.set_internal(1<<(node_count - i - 1));
        }

        tbm.trienodes.set(&root_hdl, 0, root_node);
        tbm.push_down(&mut root_node);
        tbm.trienodes.set(&root_hdl, 0, root_node);
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_apple(b: &mut Bencher) {
        let tbm = load_bgp_dump_light(0).unwrap();
        let ip = Ipv4Addr::new(17,151,0,151);
        b.iter(|| {
            black_box(tbm.longest_match(ip));
        })
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_netgroup(b: &mut Bencher) {
        let tbm = load_bgp_dump_light(0).unwrap();
        let ip = Ipv4Addr::new(77,66,88,50);
        b.iter(|| {
            black_box(tbm.longest_match(ip));
        })
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_googledns(b: &mut Bencher) {
        let tbm = load_bgp_dump_light(0).unwrap();
        let ip = Ipv4Addr::new(8,8,8,8);
        b.iter(|| {
            black_box(tbm.longest_match(ip));
        })
    }

    #[bench]
    fn bench_treebitmap_bgp_lookup_random(b: &mut Bencher) {
        let tbm = load_bgp_dump_light(0).unwrap();
        let mut rng = rand::weak_rng();
        b.iter(||{
            let r: u32 = rng.gen_range(1<<24, 224<<24);
            let ip = Ipv4Addr::from(r);
            black_box(tbm.longest_match(ip));
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
