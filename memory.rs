use core::ptr::{mut_null, null};
use libc::{PROT_READ, PROT_WRITE, MAP_ANON, MAP_PRIVATE, MAP_FAILED, c_int, c_void, mmap, munmap,
           size_t};

extern {
    fn mremap(old_address: *mut c_void, old_size: size_t, new_size: size_t, flags: c_int,
              ... /* new_address: *mut c_void */) -> *mut c_void;
}

static MREMAP_MAYMOVE: c_int = 1;

pub unsafe fn map(size: uint) -> *mut u8 {
    let ptr = mmap(null(), size as size_t, PROT_READ | PROT_WRITE, MAP_ANON | MAP_PRIVATE, -1, 0);
    if unlikely!(ptr == MAP_FAILED as *mut c_void) { return mut_null() }
    ptr as *mut u8
}

pub unsafe fn remap(ptr: *mut u8, old_size: uint, new_size: uint) -> *mut u8 {
    let ptr = mremap(ptr as *mut c_void, old_size as size_t, new_size as size_t, MREMAP_MAYMOVE);
    if unlikely!(ptr == MAP_FAILED as *mut c_void) { return mut_null() }
    ptr as *mut u8
}

pub unsafe fn remap_inplace(ptr: *mut u8, old_size: uint, new_size: uint) -> bool {
    let ptr = mremap(ptr as *mut c_void, old_size as size_t, new_size as size_t, 0);
    likely!(ptr != MAP_FAILED as *mut c_void)
}

pub unsafe fn unmap(ptr: *mut u8, size: uint) {
    munmap(ptr as *c_void, size as size_t);
}
