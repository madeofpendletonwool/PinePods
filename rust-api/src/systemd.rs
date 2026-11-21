use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sd_notify::NotifyState;
use tokio::time::MissedTickBehavior;
use tracing::warn;

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

pub fn ready(endpoint: &str) {
    notify(&[NotifyState::Ready, NotifyState::Status(&format!("Listening on {endpoint}"))]);
    if let Some(timeout) = sd_notify::watchdog_enabled().filter(|timeout| !timeout.is_zero()) {
        tokio::spawn(keep_alive(timeout / 2));
    }
}

pub fn stopping() {
    notify(&[NotifyState::Stopping]);
}

async fn keep_alive(period: Duration) {
    let mut ticks = tokio::time::interval(period);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        ticks.tick().await;
        notify(&[NotifyState::Watchdog]);
    }
}

fn notify(state: &[NotifyState]) {
    if let Err(e) = sd_notify::notify(state) {
        warn!("Failed to notify systemd: {e}");
    }
}
