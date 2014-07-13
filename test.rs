extern crate allocator;
extern crate core;

use allocator::{Allocator, LocalAlloc, local_alloc};
use core::intrinsics::abort;
use core::mem::size_of;

static n: uint = 1000000;

#[start]
fn main(_: int, _: *const *const u8) -> int {
    unsafe {
        LocalAlloc::init();

        let allocations = local_alloc.allocate(n * size_of::<*mut u32>()) as *mut *mut u32;
        if allocations.is_null() {
            abort();
        }

        for _ in range(0u, 100) {
            for j in range(0, n) {
                let ptr = allocations.offset(j as int);
                *ptr = local_alloc.allocate(size_of::<u32>()) as *mut u32;
                if (*ptr).is_null() { abort() };
                **ptr = 0xdeadbeef;
            }

            for j in range(0, n) {
                let ptr = allocations.offset(j as int);
                if **ptr != 0xdeadbeef {
                    abort()
                }
                local_alloc.deallocate(*ptr as *mut u8, size_of::<u32>())
            }
        }
        local_alloc.deallocate(allocations as *mut u8, n * size_of::<u32>())
    }
    0
}
