// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

//! The room left on a disk, so fetching older mail never fills it.

use std::path::Path;

/// Bytes this user may still write on the disk holding `path`, and the
/// disk's size; None when the system does not say.
pub fn space(path: &Path) -> Option<(u64, u64)> {
    // The folder may not exist yet: its nearest parent that does.
    let existing = path.ancestors().find(|p| p.exists())?;
    platform(existing)
}

#[cfg(unix)]
fn platform(path: &Path) -> Option<(u64, u64)> {
    let stat = rustix::fs::statvfs(path).ok()?;
    let block = stat.f_frsize;
    Some((stat.f_bavail.saturating_mul(block), stat.f_blocks.saturating_mul(block)))
}

#[cfg(windows)]
fn platform(path: &Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: a nul-terminated path, and two outputs that live through the call.
    let done = unsafe { windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(windows::core::PCWSTR(wide.as_ptr()), Some(&mut free), Some(&mut total), None) };
    done.ok().map(|()| (free, total))
}

#[cfg(not(any(unix, windows)))]
fn platform(_path: &Path) -> Option<(u64, u64)> {
    None
}

/// What stays free whatever comes: 5 GB, or a twentieth of the disk if more.
pub fn reserve(total: u64) -> u64 {
    (5u64 << 30).max(total / 20)
}

/// Bytes older mail may take now on the disk holding `path`: what is free
/// beyond the reserve; everything when the system does not say.
pub fn room(path: &Path) -> u64 {
    space(path).map_or(u64::MAX, |(free, total)| free.saturating_sub(reserve(total)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reserve_grows_with_the_disk() {
        assert_eq!(reserve(64 << 30), 5 << 30);
        assert_eq!(reserve(1000 << 30), 50 << 30);
        let (free, total) = space(&std::env::temp_dir().join("sioul-not-there/at-all")).expect("the system says");
        assert!(total >= free && total > 0);
    }
}
