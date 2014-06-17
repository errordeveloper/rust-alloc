#![crate_type = "lib"]
#![feature(macro_rules, globs)]
#![allow(unused_unsafe)] // broken with macro expansion

#![no_std]

extern crate core;
extern crate libc;

#[cfg(test)]
extern crate native;
#[cfg(test)]
extern crate std;

use core::prelude::*;
use core::num::Bitwise;
use core::ptr;
use libc::{PROT_READ, PROT_WRITE, MAP_PRIVATE, MAP_FAILED, c_void, mmap, munmap, size_t};
use MAP_ANONYMOUS = libc::consts::os::extra::MAP_ANONONYMOUS;

extern {
    #[link_name = "llvm.expect.i8"]
    pub fn expect(val: u8, expected_val: u8) -> u8;
}

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

struct FreeBlock {
    next: *mut FreeBlock,
    // padding
}

static initial_bucket: uint = 16;
static n_buckets: uint = 17;
static slab_size: uint = initial_bucket << (n_buckets - 1);

static mut buckets: [*mut FreeBlock, ..n_buckets] = [0 as *mut FreeBlock, ..n_buckets];

fn get_size_class(size: u32) -> u32 {
    let pow2 = 1 << (32 - (size - 1).leading_zeros());
    if pow2 < 16 { 16 } else { pow2 }
}

fn size_class_to_bucket(size_class: u32) -> u32 {
    size_class.trailing_zeros() - 4
}

unsafe fn map_memory(size: uint) -> *mut u8 {
    let ptr = mmap(ptr::null(), size as size_t, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if unlikely!(ptr == MAP_FAILED as *mut c_void) {
        ptr::mut_null()
    } else {
        ptr as *mut u8
    }
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

pub unsafe fn allocate(size: uint) -> *mut u8 {
    if likely!(size < slab_size) {
        return allocate_small(size as u32)
    }
    map_memory(size)
}

pub unsafe fn reallocate(ptr: *mut u8, old_size: uint, size: uint) -> *mut u8 {
    let dst = allocate(size);
    if dst.is_null() { return ptr }
    ptr::copy_nonoverlapping_memory(dst, ptr as *u8, old_size);
    deallocate(ptr, old_size);
    dst
}

pub unsafe fn reallocate_inplace(ptr: *mut u8, old_size: uint, size: uint) -> bool {
    false
}

unsafe fn deallocate_small(ptr: *mut u8, size: u32) {
    let size_class = get_size_class(size);
    let bucket = size_class_to_bucket(size_class);
    let block = ptr as *mut FreeBlock;

    (*block).next = buckets[bucket as uint];
    buckets[bucket as uint] = block;
}

pub unsafe fn deallocate(ptr: *mut u8, size: uint) {
    if likely!(size < slab_size) {
        return deallocate_small(ptr, size as u32)
    }
    unmap_memory(ptr, size);
}

#[cfg(test)]
mod tests {
    use core::ptr::RawPtr;
    use super::{allocate, deallocate};

    #[test]
    fn basic() {
        unsafe {
            let ptr = allocate(16);
            if ptr.is_null() {
                unsafe { ::core::intrinsics::abort() }
            }
            deallocate(ptr, 16);
        }
    }
}
