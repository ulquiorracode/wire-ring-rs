//! Hardware MMU-accelerated Virtual Mirrored Memory storage.
//!
//! Maps a single physical memory allocation of size `C` into two contiguous
//! virtual address regions of size `2C`:
//! - `[0 .. C)` -> Physical storage
//! - `[C .. 2C)` -> Exactly the same physical storage
//!
//! This guarantees that any slice of length $\le C$ starting at any offset $idx \in [0, C)$
//! is ALWAYS contiguous in virtual memory without wrap-around splits.

use crate::storage::StorageBackend;

/// Operating system error during virtual memory mapping.
#[derive(Debug)]
pub enum MemoryError {
    /// Allocation granularity violation or non-power-of-two capacity.
    InvalidCapacity(usize),
    /// OS system call failed with raw OS error code.
    OsError(u32),
    /// Platform is unsupported for hardware MMU mirroring.
    UnsupportedPlatform,
}

impl core::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidCapacity(cap) => write!(
                f,
                "Invalid ring capacity: {cap} (must be multiple of page size)"
            ),
            Self::OsError(code) => write!(f, "OS virtual memory syscall failed with code {code}"),
            Self::UnsupportedPlatform => {
                write!(f, "Platform unsupported for hardware MMU mirroring")
            }
        }
    }
}

impl std::error::Error for MemoryError {}

/// Virtual Mirrored Memory Storage.
pub struct MirroredStorage {
    ptr: *mut u8,
    capacity: usize,
    #[cfg(target_os = "windows")]
    mapping_handle: *mut core::ffi::c_void,
}

// SAFETY: MirroredStorage owns the unique virtual memory mappings and can be transferred across threads.
unsafe impl Send for MirroredStorage {}
unsafe impl Sync for MirroredStorage {}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use core::ffi::c_void;
    use core::ptr;

    const INVALID_HANDLE_VALUE: *mut c_void = -1isize as *mut c_void;
    const PAGE_NOACCESS: u32 = 0x01;
    const PAGE_READWRITE: u32 = 0x04;
    const MEM_RESERVE: u32 = 0x00002000;
    const MEM_RESERVE_PLACEHOLDER: u32 = 0x00040000;
    const MEM_REPLACE_PLACEHOLDER: u32 = 0x00004000;
    const MEM_PRESERVE_PLACEHOLDER: u32 = 0x00000002;
    const MEM_RELEASE: u32 = 0x00008000;

    #[repr(C)]
    struct MemExtendedParameter {
        field1: u64,
        field2: u64,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateFileMappingA(
            h_file: *mut c_void,
            lp_file_mapping_attributes: *mut c_void,
            fl_protect: u32,
            dw_maximum_size_high: u32,
            dw_maximum_size_low: u32,
            lp_name: *const i8,
        ) -> *mut c_void;

        fn CloseHandle(h_object: *mut c_void) -> i32;
        fn UnmapViewOfFile(lp_base_address: *const c_void) -> i32;
        fn VirtualFree(lp_address: *mut c_void, dw_size: usize, dw_free_type: u32) -> i32;
        fn GetCurrentProcess() -> *mut c_void;
        fn GetLastError() -> u32;
    }

    #[link(name = "onecore")]
    unsafe extern "system" {
        fn VirtualAlloc2(
            process: *mut c_void,
            base_address: *mut c_void,
            size: usize,
            allocation_type: u32,
            page_protection: u32,
            extended_parameters: *mut MemExtendedParameter,
            parameter_count: u32,
        ) -> *mut c_void;

        fn MapViewOfFile3(
            file_mapping: *mut c_void,
            process: *mut c_void,
            base_address: *mut c_void,
            offset: u64,
            view_size: usize,
            allocation_type: u32,
            page_protection: u32,
            extended_parameters: *mut MemExtendedParameter,
            parameter_count: u32,
        ) -> *mut c_void;
    }

    pub fn allocate_mirrored(capacity: usize) -> Result<MirroredStorage, MemoryError> {
        // Must be multiple of Windows allocation granularity (64 KiB)
        const ALLOCATION_GRANULARITY: usize = 64 * 1024;
        if capacity == 0 || capacity % ALLOCATION_GRANULARITY != 0 || !capacity.is_power_of_two() {
            return Err(MemoryError::InvalidCapacity(capacity));
        }

        unsafe {
            let process = GetCurrentProcess();

            // 1. Create page-file-backed anonymous file mapping of size C
            let mapping = CreateFileMappingA(
                INVALID_HANDLE_VALUE,
                ptr::null_mut(),
                PAGE_READWRITE,
                0,
                capacity as u32,
                ptr::null(),
            );
            if mapping.is_null() {
                return Err(MemoryError::OsError(GetLastError()));
            }

            // 2. Reserve a contiguous virtual address region of size 2C
            let placeholder = VirtualAlloc2(
                process,
                ptr::null_mut(),
                2 * capacity,
                MEM_RESERVE | MEM_RESERVE_PLACEHOLDER,
                PAGE_NOACCESS,
                ptr::null_mut(),
                0,
            );
            if placeholder.is_null() {
                let err = GetLastError();
                CloseHandle(mapping);
                return Err(MemoryError::OsError(err));
            }

            // 3. Split the placeholder into two halves of size C each
            let split_res = VirtualFree(
                placeholder,
                capacity,
                MEM_RELEASE | MEM_PRESERVE_PLACEHOLDER,
            );
            if split_res == 0 {
                let err = GetLastError();
                CloseHandle(mapping);
                VirtualFree(placeholder, 0, MEM_RELEASE);
                return Err(MemoryError::OsError(err));
            }

            // 4. Map first half [placeholder .. placeholder + C)
            let view1 = MapViewOfFile3(
                mapping,
                process,
                placeholder,
                0,
                capacity,
                MEM_REPLACE_PLACEHOLDER,
                PAGE_READWRITE,
                ptr::null_mut(),
                0,
            );
            if view1.is_null() {
                let err = GetLastError();
                CloseHandle(mapping);
                VirtualFree(placeholder, 0, MEM_RELEASE);
                return Err(MemoryError::OsError(err));
            }

            // 5. Map second half [placeholder + C .. placeholder + 2C) to the SAME mapping
            let target2 = (placeholder as usize + capacity) as *mut c_void;
            let view2 = MapViewOfFile3(
                mapping,
                process,
                target2,
                0,
                capacity,
                MEM_REPLACE_PLACEHOLDER,
                PAGE_READWRITE,
                ptr::null_mut(),
                0,
            );
            if view2.is_null() {
                let err = GetLastError();
                UnmapViewOfFile(view1);
                CloseHandle(mapping);
                VirtualFree(target2, 0, MEM_RELEASE);
                return Err(MemoryError::OsError(err));
            }

            debug_assert_eq!(view2 as usize, view1 as usize + capacity);

            Ok(MirroredStorage {
                ptr: view1 as *mut u8,
                capacity,
                mapping_handle: mapping,
            })
        }
    }

    pub fn deallocate_mirrored(storage: &mut MirroredStorage) {
        unsafe {
            let view1 = storage.ptr as *const c_void;
            let view2 = (storage.ptr as usize + storage.capacity) as *const c_void;
            UnmapViewOfFile(view1);
            UnmapViewOfFile(view2);
            CloseHandle(storage.mapping_handle);
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use core::ffi::c_void;
    use core::ptr;

    // libc constants
    const PROT_READ: i32 = 0x1;
    const PROT_WRITE: i32 = 0x2;
    const PROT_NONE: i32 = 0x0;
    const MAP_SHARED: i32 = 0x01;
    const MAP_PRIVATE: i32 = 0x02;
    const MAP_ANONYMOUS: i32 = 0x20;
    const MAP_FIXED: i32 = 0x10;
    const MAP_FAILED: *mut c_void = -1isize as *mut c_void;
    const MFD_CLOEXEC: u32 = 0x0001;

    unsafe extern "C" {
        fn memfd_create(name: *const i8, flags: u32) -> i32;
        fn ftruncate(fd: i32, length: i64) -> i32;
        fn mmap(
            addr: *mut c_void,
            length: usize,
            prot: i32,
            flags: i32,
            fd: i32,
            offset: i64,
        ) -> *mut c_void;
        fn munmap(addr: *mut c_void, length: usize) -> i32;
        fn close(fd: i32) -> i32;
    }

    pub fn allocate_mirrored(capacity: usize) -> Result<MirroredStorage, MemoryError> {
        // Page size on Linux is typically 4096
        const PAGE_SIZE: usize = 4096;
        if capacity == 0 || capacity % PAGE_SIZE != 0 || !capacity.is_power_of_two() {
            return Err(MemoryError::InvalidCapacity(capacity));
        }

        unsafe {
            let name = c"wire_ring_mirrored".as_ptr();
            let fd = memfd_create(name, MFD_CLOEXEC);
            if fd < 0 {
                return Err(MemoryError::OsError(
                    std::io::Error::last_os_error().raw_os_error().unwrap_or(1) as u32,
                ));
            }

            if ftruncate(fd, capacity as i64) != 0 {
                let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(1) as u32;
                close(fd);
                return Err(MemoryError::OsError(err));
            }

            // Reserve 2C virtual address space
            let anon_addr = mmap(
                ptr::null_mut(),
                2 * capacity,
                PROT_NONE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            );
            if anon_addr == MAP_FAILED {
                let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(1) as u32;
                close(fd);
                return Err(MemoryError::OsError(err));
            }

            // Map first half
            let view1 = mmap(
                anon_addr,
                capacity,
                PROT_READ | PROT_WRITE,
                MAP_SHARED | MAP_FIXED,
                fd,
                0,
            );
            if view1 == MAP_FAILED {
                let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(1) as u32;
                munmap(anon_addr, 2 * capacity);
                close(fd);
                return Err(MemoryError::OsError(err));
            }

            // Map second half
            let target2 = (anon_addr as usize + capacity) as *mut c_void;
            let view2 = mmap(
                target2,
                capacity,
                PROT_READ | PROT_WRITE,
                MAP_SHARED | MAP_FIXED,
                fd,
                0,
            );
            if view2 == MAP_FAILED {
                let err = std::io::Error::last_os_error().raw_os_error().unwrap_or(1) as u32;
                munmap(view1, capacity);
                munmap(anon_addr, 2 * capacity);
                close(fd);
                return Err(MemoryError::OsError(err));
            }

            // The fd can be closed after mappings are established
            close(fd);

            Ok(MirroredStorage {
                ptr: view1 as *mut u8,
                capacity,
            })
        }
    }

    pub fn deallocate_mirrored(storage: &mut MirroredStorage) {
        unsafe {
            munmap(storage.ptr as *mut c_void, 2 * storage.capacity);
        }
    }
}

impl MirroredStorage {
    /// Allocates a new hardware MMU-mirrored storage region.
    pub fn new(capacity: usize) -> Result<Self, MemoryError> {
        #[cfg(target_os = "windows")]
        return windows_impl::allocate_mirrored(capacity);

        #[cfg(target_os = "linux")]
        return linux_impl::allocate_mirrored(capacity);

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        return Err(MemoryError::UnsupportedPlatform);
    }
}

impl Drop for MirroredStorage {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        windows_impl::deallocate_mirrored(self);

        #[cfg(target_os = "linux")]
        linux_impl::deallocate_mirrored(self);
    }
}

impl StorageBackend for MirroredStorage {
    #[inline(always)]
    fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    #[inline(always)]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }

    #[inline(always)]
    fn is_mirrored(&self) -> bool {
        true
    }
}
