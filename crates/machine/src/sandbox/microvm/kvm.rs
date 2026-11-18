//! Probe the KVM device from the same user that will start Firecracker.

use std::fs::OpenOptions;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::raw::{c_int, c_ulong};
use std::path::Path;

unsafe extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

pub(super) fn check(path: &Path) -> io::Result<()> {
    let device = OpenOptions::new().read(true).write(true).open(path)?;
    // SAFETY: these KVM ioctls accept no pointer; CREATE_VM returns an owned fd.
    let api = unsafe { ioctl(device.as_raw_fd(), 0xae00) };
    if api < 0 {
        return Err(io::Error::last_os_error());
    }
    if api != 12 {
        return Err(io::Error::other("the KVM API version is not 12"));
    }
    // SAFETY: VM type 0 selects the default architecture VM type.
    let vm = unsafe { ioctl(device.as_raw_fd(), 0xae01, 0 as c_ulong) };
    if vm < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: KVM returned a new file descriptor on success.
    drop(unsafe { OwnedFd::from_raw_fd(vm) });
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_readable_non_kvm_device_is_unusable() {
        assert!(super::check(std::path::Path::new("/dev/null")).is_err());
    }
}
