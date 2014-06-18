#![crate_type = "lib"]
#![feature(macro_rules, globs, thread_local)]
#![allow(unused_unsafe)] // broken with macro expansion

#![no_std]

extern crate core;
extern crate libc;

#[cfg(test)]
extern crate native;
#[cfg(test)]
extern crate std;

use core::prelude::*;
use core::kinds::marker;
use core::num::Bitwise;
use core::ptr;
use libc::{PROT_READ, PROT_WRITE, MAP_ANON, MAP_PRIVATE, MAP_FAILED, c_int, c_void, mmap, munmap,
           size_t};

extern {
    #[link_name = "llvm.expect.i8"]
    fn expect(val: u8, expected_val: u8) -> u8;

    fn mremap(old_address: *mut c_void, old_size: size_t, new_size: size_t, flags: c_int,
              ... /* new_address: *mut c_void */) -> *mut c_void;
}

static MREMAP_MAYMOVE: c_int = 1;

#[macro_export]
macro_rules! likely(
    ($val:expr) => {
        {
            let x: bool = $val;
            unsafe { expect(x as u8, 1) != 0 }
        }
    }
)

#[macro_export]
macro_rules! unlikely(
    ($val:expr) => {
        {
            let x: bool = $val;
            unsafe { expect(x as u8, 0) != 0 }
        }
    }
)

pub trait Allocator {
    unsafe fn allocate(&mut self, size: uint) -> *mut u8;
    unsafe fn reallocate(&mut self, ptr: *mut u8, old_size: uint, new_size: uint) -> *mut u8;
    unsafe fn reallocate_inplace(&mut self, ptr: *mut u8, old_size: uint, new_size: uint) -> bool;
    unsafe fn deallocate(&mut self, ptr: *mut u8, size: uint);
}

pub struct LocalAlloc {
    no_send: marker::NoSend
}

pub static mut local_alloc: LocalAlloc = LocalAlloc { no_send: marker::NoSend };

impl Allocator for LocalAlloc {
    unsafe fn allocate(&mut self, size: uint) -> *mut u8 {
        if likely!(size < slab_size) {
            return allocate_small(size as u32)
        }
        map_memory(size)
    }

    unsafe fn reallocate(&mut self, ptr: *mut u8, old_size: uint, new_size: uint) -> *mut u8 {
        if unlikely!(old_size > slab_size && new_size > slab_size) {
            return remap_memory(ptr, old_size, new_size, MREMAP_MAYMOVE)
        }

        let dst = local_alloc.allocate(new_size);
        if dst.is_null() { return ptr }
        ptr::copy_nonoverlapping_memory(dst, ptr as *u8, old_size);
        local_alloc.deallocate(ptr, old_size);
        dst
    }

    unsafe fn reallocate_inplace(&mut self, ptr: *mut u8, old_size: uint, new_size: uint) -> bool {
        if unlikely!(old_size > slab_size && new_size > slab_size) {
            return remap_memory(ptr, old_size, new_size, 0).is_null()
        }
        false
    }

    unsafe fn deallocate(&mut self, ptr: *mut u8, size: uint) {
        if likely!(size < slab_size) {
            return deallocate_small(ptr, size as u32)
        }
        unmap_memory(ptr, size);
    }
}

struct FreeBlock {
    next: *mut FreeBlock,
    // padding
}

static initial_bucket: uint = 16;
static n_buckets: uint = 17;
static slab_size: uint = initial_bucket << (n_buckets - 1);

#[thread_local]
static mut buckets: [*mut FreeBlock, ..n_buckets] = [0 as *mut FreeBlock, ..n_buckets];

fn get_size_class(size: u32) -> u32 {
    let pow2 = 1 << (32 - (size - 1).leading_zeros());
    if pow2 < 16 { 16 } else { pow2 }
}

fn size_class_to_bucket(size_class: u32) -> u32 {
    size_class.trailing_zeros() - 4
}

unsafe fn map_memory(size: uint) -> *mut u8 {
    let ptr = mmap(ptr::null(), size as size_t, PROT_READ | PROT_WRITE, MAP_ANON | MAP_PRIVATE, -1, 0);
    if unlikely!(ptr == MAP_FAILED as *mut c_void) { return ptr::mut_null() }
    ptr as *mut u8
}

unsafe fn remap_memory(ptr: *mut u8, old_size: uint, new_size: uint, flags: c_int) -> *mut u8 {
    let ptr = mremap(ptr as *mut c_void, old_size as size_t, new_size as size_t, flags);
    if unlikely!(ptr == MAP_FAILED as *mut c_void) { return ptr::mut_null() }
    ptr as *mut u8
}

unsafe fn unmap_memory(ptr: *mut u8, size: uint) {
    munmap(ptr as *c_void, size as size_t);
}

unsafe fn allocate_small(size: u32) -> *mut u8 {
    let size_class = get_size_class(size);
    let bucket = size_class_to_bucket(size_class);

    if likely!(buckets[bucket as uint].is_not_null()) {
        let current = buckets[bucket as uint];
        buckets[bucket as uint] = (*current).next;
        return current as *mut u8;
    }

    let ptr = map_memory(slab_size);
    if unlikely!(ptr.is_null()) { return ptr }

    for offset in core::iter::range_step(size_class, slab_size as u32, size_class) {
        let block: *mut FreeBlock = ptr.offset(offset as int) as *mut FreeBlock;
        (*block).next = buckets[bucket as uint];
        buckets[bucket as uint] = block;
    }

    ptr
}

unsafe fn deallocate_small(ptr: *mut u8, size: u32) {
    let size_class = get_size_class(size);
    let bucket = size_class_to_bucket(size_class);
    let block = ptr as *mut FreeBlock;

    (*block).next = buckets[bucket as uint];
    buckets[bucket as uint] = block;
}
