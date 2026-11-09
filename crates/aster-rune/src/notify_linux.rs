// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ad-hoc Linux notifications until `rune-notify` gains a backend
//! (https://github.com/anomalyco/rune/issues/334).
//!
//! Uses the desktop's `notify-send` so it works under any session without a
//! D-Bus client dependency. The call is fire-and-forget and never blocks the UI.

use std::sync::atomic::{AtomicU32, Ordering};

static LAST_UNREAD: AtomicU32 = AtomicU32::new(u32::MAX);

pub fn notify(title: &str, body: &str) {
    let _ = std::process::Command::new("notify-send")
        .arg("--app-name")
        .arg("Aster")
        .arg("--icon")
        .arg("mail-message-new")
        .arg(title)
        .arg(body)
        .spawn();
}

/// Notify once when the unread count grows. Skips the first observation so
/// opening the app does not announce the backlog.
pub fn check_new_mail(unread: u32) {
    let previous = LAST_UNREAD.swap(unread, Ordering::Relaxed);
    if previous != u32::MAX && unread > previous {
        notify("New mail", &format!("{unread} unread in Inbox"));
    }
}
