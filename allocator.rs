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
use core::mem;
use core::kinds::marker;
use core::num::Int;
use core::ptr;
use libc::{c_int, c_void};

mod macros;
mod memory;

#[allow(non_camel_case_types)]
type pthread_key_t = libc::c_uint;

extern {
    fn pthread_key_create(key: *mut pthread_key_t,
                          dtor: unsafe extern "C" fn(*mut c_void)) -> c_int;
    fn pthread_getspecific(key: pthread_key_t) -> *mut c_void;
    fn pthread_setspecific(key: pthread_key_t, value: *mut c_void) -> c_int;
}

pub trait Allocator {
    unsafe fn allocate(&mut self, size: uint) -> *mut u8;
    unsafe fn reallocate(&mut self, ptr: *mut u8, old_size: uint, new_size: uint) -> *mut u8;
    unsafe fn reallocate_inplace(&mut self, ptr: *mut u8, old_size: uint, new_size: uint) -> bool;
    unsafe fn deallocate(&mut self, ptr: *mut u8, size: uint);
}

pub struct LocalAlloc {
    no_send: marker::NoSend
}

static mut key: pthread_key_t = 0;

impl LocalAlloc {
    pub unsafe fn init() {
        unsafe extern fn local_free(chunk: *mut c_void) {
            let mut iter = chunk as *mut LocalChunk;
            while iter.is_not_null() {
                let current = iter;
                iter = (*iter).next;
                memory::unmap(current as *mut u8, mem::size_of::<LocalChunk>());
            }
        }
        pthread_key_create(&mut key, local_free);
    }
}

pub static mut local_alloc: LocalAlloc = LocalAlloc { no_send: marker::NoSend };

impl Allocator for LocalAlloc {
    unsafe fn allocate(&mut self, size: uint) -> *mut u8 {
        if likely!(size < slab_size) {
            return allocate_small(size as u32)
        }
        memory::map(size)
    }

    unsafe fn reallocate(&mut self, ptr: *mut u8, old_size: uint, new_size: uint) -> *mut u8 {
        if unlikely!(old_size > slab_size && new_size > slab_size) {
            return memory::remap(ptr, old_size, new_size)
        }

        let dst = local_alloc.allocate(new_size);
        if dst.is_null() { return ptr }
        ptr::copy_nonoverlapping_memory(dst, ptr as *const u8, old_size);
        local_alloc.deallocate(ptr, old_size);
        dst
    }

    unsafe fn reallocate_inplace(&mut self, ptr: *mut u8, old_size: uint, new_size: uint) -> bool {
        if unlikely!(old_size > slab_size && new_size > slab_size) {
            return memory::remap_inplace(ptr, old_size, new_size)
        }
        false
    }

    unsafe fn deallocate(&mut self, ptr: *mut u8, size: uint) {
        if likely!(size < slab_size) {
            return deallocate_small(ptr, size as u32)
        }
        memory::unmap(ptr, size);
    }
}

struct LocalChunk {
    data: [u8, ..slab_size],
    next: *mut LocalChunk
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
    let pow2 = 1 << (32 - (size - 1).leading_zeros()) as uint;
    if pow2 < 16 { 16 } else { pow2 }
}

fn size_class_to_bucket(size_class: u32) -> u32 {
    size_class.trailing_zeros() - 4
}

unsafe fn allocate_small(size: u32) -> *mut u8 {
    let size_class = get_size_class(size);
    let bucket = size_class_to_bucket(size_class);

    if likely!(buckets[bucket as uint].is_not_null()) {
        let current = buckets[bucket as uint];
        buckets[bucket as uint] = (*current).next;
        return current as *mut u8;
    }

    let chunk = memory::map(mem::size_of::<LocalChunk>()) as *mut LocalChunk;
    (*chunk).next = pthread_getspecific(key) as *mut LocalChunk;
    pthread_setspecific(key, chunk as *mut c_void);

    let ptr: *mut u8 = (*chunk).data.as_mut_ptr();

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
