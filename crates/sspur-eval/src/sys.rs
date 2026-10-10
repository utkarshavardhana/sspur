//! The few OS calls the interpreter makes directly, matching the native runtime: POSIX descriptors on Unix, CRT descriptors on Windows, and the in-memory host in the browser.
use std::ffi::c_void;

#[cfg(unix)]
mod imp {
    use std::ffi::c_void;

    unsafe extern "C" {
        fn read(fd: i32, buf: *mut c_void, n: usize) -> isize;
        fn pread(fd: i32, buf: *mut c_void, n: usize, off: i64) -> isize;
        fn write(fd: i32, buf: *const c_void, n: usize) -> isize;
        fn lseek(fd: i32, off: i64, whence: i32) -> i64;
        fn close(fd: i32) -> i32;
        fn clock_gettime(clk: i32, tp: *mut [i64; 2]) -> i32;
    }

    #[cfg(target_os = "macos")]
    const CLOCK_MONOTONIC: i32 = 6;
    #[cfg(not(target_os = "macos"))]
    const CLOCK_MONOTONIC: i32 = 1;

    pub fn mono_ns() -> i64 {
        let mut t = [0i64; 2];
        unsafe { clock_gettime(CLOCK_MONOTONIC, &mut t) };
        t[0].saturating_mul(1_000_000_000).saturating_add(t[1])
    }

    pub unsafe fn fd_read(fd: i32, buf: *mut c_void, n: usize) -> isize {
        unsafe { read(fd, buf, n) }
    }
    pub unsafe fn fd_pread(fd: i32, buf: *mut c_void, n: usize, off: i64) -> isize {
        unsafe { pread(fd, buf, n, off) }
    }
    pub unsafe fn fd_write(fd: i32, buf: *const c_void, n: usize) -> isize {
        unsafe { write(fd, buf, n) }
    }
    pub unsafe fn fd_seek(fd: i32, off: i64, whence: i32) -> i64 {
        unsafe { lseek(fd, off, whence) }
    }
    pub unsafe fn fd_close(fd: i32) -> i32 {
        unsafe { close(fd) }
    }

    pub fn errno() -> i32 {
        std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
    }

    pub fn errno_reason(e: i32) -> String {
        crate::stdlib::os_reason(&std::io::Error::from_raw_os_error(e))
    }

    pub fn into_fd(f: std::fs::File) -> i32 {
        std::os::fd::IntoRawFd::into_raw_fd(f)
    }

    pub fn identity(fd: i32) -> Option<(i64, i64)> {
        use std::os::fd::FromRawFd;
        use std::os::unix::fs::MetadataExt;
        let f = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) });
        f.metadata().ok().map(|m| (m.dev() as i64, m.ino() as i64))
    }

    pub fn fd_len(fd: i32) -> std::io::Result<u64> {
        use std::os::fd::FromRawFd;
        let f = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) });
        f.metadata().map(|m| m.len())
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct FileInfo {
        attrs: u32,
        times: [u32; 6],
        volume: u32,
        size_hi: u32,
        size_lo: u32,
        links: u32,
        index_hi: u32,
        index_lo: u32,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn QueryPerformanceCounter(c: *mut i64) -> i32;
        fn QueryPerformanceFrequency(f: *mut i64) -> i32;
        fn GetFileInformationByHandle(h: *mut c_void, info: *mut FileInfo) -> i32;
    }

    unsafe extern "C" {
        fn _read(fd: i32, buf: *mut c_void, n: u32) -> i32;
        fn _write(fd: i32, buf: *const c_void, n: u32) -> i32;
        fn _lseeki64(fd: i32, off: i64, whence: i32) -> i64;
        fn _close(fd: i32) -> i32;
        fn _errno() -> *mut i32;
        fn _open_osfhandle(h: isize, flags: i32) -> i32;
        fn _get_osfhandle(fd: i32) -> isize;
        fn _set_invalid_parameter_handler(h: Option<IpHandler>) -> Option<IpHandler>;
    }

    type IpHandler = unsafe extern "C" fn(*const u16, *const u16, *const u16, u32, usize);
    unsafe extern "C" fn ignore_ip(_: *const u16, _: *const u16, _: *const u16, _: u32, _: usize) {}

    // The CRT aborts on a bad fd by default; POSIX returns EBADF, so make it do that.
    fn crt_quiet() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| unsafe {
            _set_invalid_parameter_handler(Some(ignore_ip));
        });
    }

    pub fn mono_ns() -> i64 {
        let (mut c, mut f) = (0i64, 1i64);
        unsafe {
            QueryPerformanceCounter(&mut c);
            QueryPerformanceFrequency(&mut f);
        }
        (c / f).saturating_mul(1_000_000_000).saturating_add((c % f) * 1_000_000_000 / f)
    }

    pub unsafe fn fd_read(fd: i32, buf: *mut c_void, n: usize) -> isize {
        crt_quiet();
        unsafe { _read(fd, buf, n.min(i32::MAX as usize) as u32) as isize }
    }
    pub unsafe fn fd_pread(fd: i32, buf: *mut c_void, n: usize, off: i64) -> isize {
        crt_quiet();
        unsafe {
            let cur = _lseeki64(fd, 0, 1);
            if cur < 0 || _lseeki64(fd, off, 0) < 0 {
                return -1;
            }
            let k = fd_read(fd, buf, n);
            let e = errno();
            _lseeki64(fd, cur, 0);
            *_errno() = e;
            k
        }
    }
    pub unsafe fn fd_write(fd: i32, buf: *const c_void, n: usize) -> isize {
        crt_quiet();
        unsafe { _write(fd, buf, n.min(i32::MAX as usize) as u32) as isize }
    }
    pub unsafe fn fd_seek(fd: i32, off: i64, whence: i32) -> i64 {
        crt_quiet();
        unsafe { _lseeki64(fd, off, whence) }
    }
    pub unsafe fn fd_close(fd: i32) -> i32 {
        crt_quiet();
        unsafe { _close(fd) }
    }

    pub fn errno() -> i32 {
        unsafe { *_errno() }
    }

    pub fn errno_reason(e: i32) -> String {
        match e {
            2 => "not found".into(),
            1 | 13 => "permission denied".into(),
            21 => "is a directory".into(),
            20 => "not a directory".into(),
            17 => "already exists".into(),
            41 => "directory not empty".into(),
            n => format!("os error {n}"),
        }
    }

    pub fn into_fd(f: std::fs::File) -> i32 {
        let h = std::os::windows::io::IntoRawHandle::into_raw_handle(f);
        unsafe { _open_osfhandle(h as isize, 0x8000) }
    }

    fn info(fd: i32) -> Option<FileInfo> {
        let h = unsafe { _get_osfhandle(fd) };
        if h == -1 || h == 0 {
            return None;
        }
        let mut i = FileInfo::default();
        (unsafe { GetFileInformationByHandle(h as *mut c_void, &mut i) } != 0).then_some(i)
    }

    pub fn identity(fd: i32) -> Option<(i64, i64)> {
        info(fd).map(|i| (i64::from(i.volume), ((u64::from(i.index_hi) << 32) | u64::from(i.index_lo)) as i64))
    }

    pub fn fd_len(fd: i32) -> std::io::Result<u64> {
        info(fd).map(|i| (u64::from(i.size_hi) << 32) | u64::from(i.size_lo)).ok_or_else(|| std::io::Error::from_raw_os_error(6))
    }
}

#[cfg(target_family = "wasm")]
mod imp {
    pub use crate::web::{errno, errno_reason, fd_close, fd_len, fd_pread, fd_read, fd_seek, fd_write, identity, mono_ns, sleep_ms, wall_ms};
}

#[cfg(not(target_family = "wasm"))]
pub fn wall_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

#[cfg(not(target_family = "wasm"))]
pub fn sleep_ms(ms: i64) {
    std::thread::sleep(std::time::Duration::from_millis(ms.max(0) as u64));
}

pub use imp::*;

pub fn read_byte(fd: i32) -> Option<u8> {
    let mut b = 0u8;
    (unsafe { fd_read(fd, (&raw mut b).cast::<c_void>(), 1) } > 0).then_some(b)
}
