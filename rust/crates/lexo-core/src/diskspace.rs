//! How much room a disk has: the preparation of a game checks it before it copies anything. The one place of the core that asks the operating system a thing the standard library cannot
//! (free space), so the two small calls are here and nowhere else.
use std::path::Path;

/// The bytes a user may still write on the disk that holds `path` (the nearest folder that exists); `None` when the system does not say.
pub fn free_bytes(path: &Path) -> Option<u64> {
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    imp(p)
}

#[cfg(windows)]
fn imp(p: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(dir: *const u16, available: *mut u64, total: *mut u64, free: *mut u64) -> i32;
    }
    let wide: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut available, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: `wide` is a NUL-terminated wide string that lives through the call, and the three pointers are to live integers.
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut free) };
    (ok != 0).then_some(available)
}

#[cfg(unix)]
fn imp(p: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
    // SAFETY: `c` is a NUL-terminated path that lives through the call, and `s` is a zeroed structure of the type the call fills.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    let ok = unsafe { libc::statvfs(c.as_ptr(), &mut s) };
    (ok == 0).then(|| (s.f_bavail as u64).saturating_mul(s.f_frsize as u64))
}

#[cfg(not(any(windows, unix)))]
fn imp(_p: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_temporary_folder_has_some_room_and_a_folder_that_is_not_there_is_asked_through_its_parent() {
        let tmp = std::env::temp_dir();
        let here = free_bytes(&tmp).expect("the system says");
        assert!(here > 0);
        assert!(free_bytes(&tmp.join("not").join("there").join("yet")).is_some());
    }
}
