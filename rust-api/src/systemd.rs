use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};

pub fn take_listen_fds() -> io::Result<Vec<OwnedFd>> {
    static TAKEN: AtomicBool = AtomicBool::new(false);
    if TAKEN.swap(true, Ordering::Relaxed) {
        return Err(io::Error::other("the sockets systemd passed were already taken"));
    }
    Ok(sd_notify::listen_fds()?
        // SAFETY: systemd passes these descriptors to this process to own, and TAKEN ensures they are taken only once.
        .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) })
        .collect())
}
