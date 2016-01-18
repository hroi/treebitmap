use std::fmt;
use std::ptr;
use std::mem;
use std::slice;
use std::cmp;

use alloc::raw_vec::RawVec;

pub struct BucketVec<T> {
    buf: RawVec<T>,
    freelist: Vec<u32>,
    len: u32,
    spacing: u32,
}

impl<T: fmt::Debug> fmt::Debug for BucketVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        f.debug_struct("BucketVec")
            .field("spacing", &self.spacing)
            .field("freelist", &self.freelist)
            .field("len", &self.len)
            .field("cap", &self.buf.cap())
            .field("buf", unsafe {&slice::from_raw_parts(self.buf.ptr(), self.len as usize)})
            .finish()
    }
}

impl<T: Sized> BucketVec<T> {

    pub fn with_capacity(spacing: u32, capacity: usize) -> BucketVec<T> {
        BucketVec {
            buf: RawVec::with_capacity(capacity),
            freelist: Vec::with_capacity(32),
            len: 0,
            spacing: spacing,
        }
    }

    #[allow(dead_code)]
    pub fn new(spacing: u32) -> BucketVec<T> {
        Self::with_capacity(spacing, 0)
    }

    pub fn alloc_slot(&mut self) -> u32 {
        match self.freelist.pop() {
            Some(n) => n,
            None => {
                self.buf.reserve(self.len as usize, self.spacing as usize);
                if cfg!(debug_assertions) {
                    unsafe{
                        ptr::write_bytes(self.buf.ptr().offset(self.len as isize), 0, self.spacing as usize);
                    }
                }
                let slot = self.len;
                self.len += self.spacing;
                slot
            }
        }
    }

    /// Adds slot to the freelist.
    pub fn free_slot(&mut self, slot: u32) {
        self.freelist.push(slot)
    }

    pub fn get_slot_entry(&self, slot: u32, index: u32) -> &T {
        let offset = slot + index;
        unsafe {
            let src_ptr = self.buf.ptr().offset(offset as isize);
            &*src_ptr
        }
    }

    pub fn set_slot_entry(&mut self, slot: u32, index: u32, value: T) {
        debug_assert!(index < self.spacing);
        let offset = slot + index;
        unsafe {
            let dst_ptr = self.buf.ptr().offset(offset as isize);
            ptr::write(dst_ptr, value);
        }
    }

    /// Insert ```value``` into ```slot``` at ```index```. Values to the right of ```index``` will be moved.
    /// If all values have been set the last value will be lost.
    pub fn insert_slot_entry(&mut self, slot: u32, index: u32, value: T) {
        let offset = slot + index;
        unsafe {
            let dst_ptr = self.buf.ptr().offset(offset as isize);
            ptr::copy(dst_ptr, dst_ptr.offset(1), (self.spacing - index - 1) as usize);
            ptr::write(dst_ptr, value);
        }
    }

    pub fn remove_slot_entry(&mut self, slot: u32, index: u32) -> T {
        let offset = slot + index;
        let ret: T;
        unsafe {
            ret = ptr::read(self.buf.ptr().offset(offset as isize));
            let dst_ptr = self.buf.ptr().offset(offset as isize);
            ptr::copy(dst_ptr.offset(1), dst_ptr, (self.spacing - index - 1) as usize);
            if cfg!(debug_assertions) {
                ptr::write(dst_ptr.offset((self.spacing - index - 1) as isize), mem::zeroed());
            }
        }
        ret
    }

    /// move contents from one bucket to another. Returns the offset of the new location.
    fn move_slot(&mut self, slot: u32, dst: &mut BucketVec<T>) -> u32 {
        let nitems = cmp::min(self.spacing, dst.spacing);

        //debug_assert!(self.spacing != dst.spacing);
        debug_assert!(nitems > 0);
        debug_assert!(nitems <= dst.spacing);
        debug_assert!(slot < self.len);
        debug_assert!(nitems <= self.spacing);
        debug_assert!(nitems <= dst.spacing);

        let dst_slot = dst.alloc_slot();

        unsafe {
            let src_ptr = self.buf.ptr().offset(slot as isize);
            let dst_ptr = dst.buf.ptr().offset(dst_slot as isize);
            ptr::copy_nonoverlapping(src_ptr, dst_ptr, nitems as usize);
            if cfg!(debug_assertions) {
                ptr::write_bytes(src_ptr, 0, nitems as usize);
            }
        }

        self.free_slot(slot);

        dst_slot
    }

    fn shrink_to_fit(&mut self) {
        self.buf.shrink_to_fit(self.len as usize);
    }
}

#[inline]
pub fn choose_bucket(len: u32) -> u32 {
    match len {
        0 => 0,
        _ => (32 - (len - 1).leading_zeros())
    }
}

/// ```Allocator``` stores items in exponentially sized buckets (using ```BucketVec```s for backing).
///
/// All interaction is done with an ```AllocatorHandle```used for tracking the collection size an location.
/// The location of data is computed based on the collection sized and base pointer (stored in handle).
/// When a bucket becomes full, the contents are moved to a larger bucket. In this case the allocator will update the caller's pointer.
#[derive(Debug)]
pub struct Allocator<T: Sized> {
    buckets: [BucketVec<T>; 6],
}

/// Tracks the size and location of the referenced collection.
#[derive(Debug)]
pub struct AllocatorHandle {
    /// The current length of the collection
    pub len: u32,
    /// Basepointer
    pub offset: u32,
}

impl AllocatorHandle {
    pub fn generate(len: u32, offset: u32) -> AllocatorHandle {
        AllocatorHandle{
            len: len,
            offset: offset,
        }
    }
}

impl<T: Sized> Allocator<T> {
    /// Initialize a new allocator with default capacity.
    #[allow(dead_code)]
    pub fn new() -> Allocator<T> {
        Allocator {
            buckets: [BucketVec::new(1), BucketVec::new(2), BucketVec::new(4),
                      BucketVec::new(8), BucketVec::new(16), BucketVec::new(32)]
        }
    }

    /// Initialize a new ```Allocator``` with specified capacity.
    pub fn with_capacity(cap: usize) -> Allocator<T> {
        Allocator {
            buckets: [BucketVec::with_capacity(1, cap), BucketVec::with_capacity( 2, cap), BucketVec::with_capacity(4, cap),
                      BucketVec::with_capacity(8, cap), BucketVec::with_capacity(16, cap), BucketVec::with_capacity(32, cap)]
        }
    }

    #[allow(dead_code)]
    /// Returns the amount of memory allocated, and the amount of memory allocated but not used.
    pub fn mem_usage(&self) -> (usize, usize) {
        let mut total = 0;
        let mut wasted = 0;
        for buckvec in &self.buckets {
            total += buckvec.buf.cap() * mem::size_of::<T>();
            total += buckvec.freelist.capacity() * mem::size_of::<u32>();
            wasted += buckvec.freelist.len() * mem::size_of::<T>();
        }
        (total, wasted)
    }

    pub fn shrink_to_fit(&mut self) {
        for buckvec in &mut self.buckets {
            buckvec.shrink_to_fit();
        }
    }

    pub fn alloc(&mut self, count: u32) -> AllocatorHandle {
        let bucket_index = choose_bucket(count) as usize;
        let slot = self.buckets[bucket_index].alloc_slot();
        AllocatorHandle{
            len: count,
            offset: slot
        }
    }

    pub fn set(&mut self, hdl: &AllocatorHandle, index: u32, value: T) {
        let bucket_index = choose_bucket(hdl.len) as usize;
        self.buckets[bucket_index].set_slot_entry(hdl.offset, index, value)
    }

    pub fn get(&self, hdl: &AllocatorHandle, index: u32) -> &T {
        let bucket_index = choose_bucket(hdl.len) as usize;
        self.buckets[bucket_index].get_slot_entry(hdl.offset, index)
    }

    pub fn insert(&mut self, hdl: &mut AllocatorHandle, index: u32, value: T) {
        let mut bucket_index = choose_bucket(hdl.len) as usize;
        let next_bucket_index = choose_bucket(hdl.len + 1) as usize;
        let mut slot = hdl.offset;

        debug_assert!(self.buckets[bucket_index].len >= hdl.offset);

        if bucket_index != next_bucket_index {
            // move to bigger bucket
            debug_assert!(next_bucket_index > 0);
            let ptr: *mut BucketVec<T> = &mut self.buckets[0];
            let dst: &mut BucketVec<T> = unsafe {
                mem::transmute(ptr.offset(next_bucket_index as isize))
            };
            slot = self.buckets[bucket_index].move_slot(slot, dst);
            bucket_index = next_bucket_index;
        }

        hdl.offset = slot;
        hdl.len += 1;

        self.buckets[bucket_index].insert_slot_entry(slot, index, value)
    }

    pub fn remove(&mut self, hdl: &mut AllocatorHandle, index: u32) -> T {
        let bucket_index = choose_bucket(hdl.len) as usize;
        let next_bucket_index = choose_bucket(hdl.len - 1) as usize;
        let mut slot = hdl.offset;

        let ret = self.buckets[bucket_index].remove_slot_entry(slot, index);

        if bucket_index != next_bucket_index {
            // move to smaller bucket
            debug_assert!(next_bucket_index < 5);
            let buckets_base_ptr: *mut BucketVec<T> = &mut self.buckets[0];
            let dst: &mut BucketVec<T> = unsafe {
                mem::transmute(buckets_base_ptr.offset(next_bucket_index as isize))
            };
            slot = self.buckets[bucket_index].move_slot(slot, dst);
        }

        hdl.offset = slot;
        hdl.len -= 1;
        ret

    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::{Bencher,black_box};

    #[test]
    fn test_ralloc_bucketvec_move_to() {
        let spacing = 32;
        let mut a: BucketVec<u32> = BucketVec::new(spacing);
        let mut b: BucketVec<u32> = BucketVec::new(spacing);
        let slot_offset = a.alloc_slot();
        for i in 0..spacing {
            a.set_slot_entry(slot_offset,i,1000 + i);
        }
        let _ = a.move_slot(0, &mut b);
    }

    #[test]
    fn test_ralloc_bucketvec_get_slot_entry() {
        let spacing = 16;
        let mut bucket: BucketVec<u32> = BucketVec::new(spacing);
        let slot = bucket.alloc_slot();
        for i in 0..spacing {
            bucket.set_slot_entry(slot, i, 1000 + i);
        }
        for i in 0..spacing {
            assert_eq!(*bucket.get_slot_entry(slot, i), 1000 + i);
        }
    }

    #[test]
    fn test_ralloc_bucketvec_insert_slot_entry() {
        let spacing = 16;
        let mut bucket: BucketVec<u32> = BucketVec::new(spacing);
        let slot = bucket.alloc_slot();
        for i in 0..spacing {
            bucket.insert_slot_entry(slot, 0, i);
        }
        bucket.insert_slot_entry(slot, 0, 123456);
        assert_eq!(*bucket.get_slot_entry(slot, 0), 123456);
        assert_eq!(*bucket.get_slot_entry(slot, spacing - 1), 1);
    }

    #[test]
    fn test_ralloc_allocator_new() {
        Allocator::<u32>::new();
    }

    #[test]
    fn test_ralloc_allocator_alloc1() {
        let mut alloc = Allocator::<u32>::new();
        let _ = alloc.alloc(1);
    }

    #[test]
    fn test_ralloc_allocator_fill() {
        let mut alloc = Allocator::<u32>::new();
        let mut hdl = alloc.alloc(0);
        for i in 0..32 {
            alloc.insert(&mut hdl, 0, 1000 + i);
        }
        let mut hdl = alloc.alloc(0);
        for i in 0..32 {
            alloc.insert(&mut hdl, 0, 2000 + i);
        }
        println!("{:?}", hdl);
        println!("{:#?}", alloc);
    }

    #[test]
    fn test_ralloc_allocator_drain() {
        let mut alloc = Allocator::<u64>::new();
        let mut hdl = alloc.alloc(0);
        assert!(hdl.len == 0);
        let n = 32;
        for i in 0..n {
            alloc.insert(&mut hdl, 0, 1000 + i);
        }
        assert!(hdl.len == 32);
        for i in 0..n {
            let item = alloc.remove(&mut hdl, 0);
            assert!(item == 1031 - i);
        }
        assert!(hdl.len == 0);
    }

    #[test]
    fn test_ralloc_allocator_set() {
        let mut alloc = Allocator::<u32>::new();
        let hdl = alloc.alloc(32);
        for i in 0..32 {
            alloc.set(&hdl, i, 1000+i);
        }

        for i in 0..32 {
            assert_eq!(*alloc.get(&hdl, i), 1000+i);
        }
    }

    #[bench]
    fn bench_ralloc_choose_bucket_32(b: &mut Bencher) {
        for i in 0..32 {
            b.iter(|| {
                black_box(choose_bucket(i));
            })
        }
    }

    #[bench]
    fn bench_ralloc_alloc_set(b: &mut Bencher) {
        let mut alloc: Allocator<u32> = Allocator::new();
        let hdl = alloc.alloc(4);
        b.iter(|| {
            black_box(alloc.set(&hdl, 1, 100));
        });
    }

    #[bench]
    fn bench_ralloc_alloc_and_insert_01(b: &mut Bencher) {
        let mut alloc = Allocator::<u32>::new();
        b.iter(|| {
            let mut hdl = alloc.alloc(0);
            black_box(alloc.insert(&mut hdl, 0, 123123123));
        });
    }

    #[bench]
    fn bench_ralloc_alloc_and_insert_02(b: &mut Bencher) {
        let mut alloc = Allocator::<u32>::new();
        b.iter(|| {
            let mut hdl = alloc.alloc(1);
            black_box(alloc.insert(&mut hdl, 0, 123123123));
        });
    }

    #[bench]
    fn bench_ralloc_alloc_and_insert_04(b: &mut Bencher) {
        let mut alloc = Allocator::<u32>::new();
        b.iter(|| {
            let mut hdl = alloc.alloc(2);
            black_box(alloc.insert(&mut hdl, 0, 123123123));
        });
    }

    #[bench]
    fn bench_ralloc_alloc_and_insert_08(b: &mut Bencher) {
        let mut alloc = Allocator::<u32>::new();
        b.iter(|| {
            let mut hdl = alloc.alloc(4);
            black_box(alloc.insert(&mut hdl, 0, 123123123));
        });
    }

    #[bench]
    fn bench_ralloc_alloc_and_insert_16(b: &mut Bencher) {
        let mut alloc = Allocator::<u32>::new();
        b.iter(|| {
            let mut hdl = alloc.alloc(8);
            black_box(alloc.insert(&mut hdl, 0, 123123123));
        });
    }

    #[bench]
    fn bench_ralloc_alloc_and_insert_32(b: &mut Bencher) {
        let mut alloc = Allocator::<u32>::new();
        b.iter(|| {
            let mut hdl = alloc.alloc(16);
            black_box(alloc.insert(&mut hdl, 0, 123123123));
        });
    }
}
