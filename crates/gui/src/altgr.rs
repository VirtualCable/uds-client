// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! AltGr (RAlt) pairing for Windows targets (client-side input).
//!
//! On Windows, AltGr is `Ctrl+Alt`: the OS emits a fake LCtrl key-down just
//! before the RAlt key-down and the matching releases afterwards. winit's
//! delivery of those fake events is not reliable (the fake LCtrl key-up can
//! be swallowed), which left Ctrl stuck on the remote until the user tapped
//! Ctrl again. This tracker synthesizes the pairing ourselves:
//!
//! - AltGr down: if no real LCtrl is held, press one synthetically; if an
//!   LCtrl down arrived within [`FAKE_WINDOW`] it is Windows' AltGr fake, so
//!   we take ownership of it (to release it ourselves on AltGr up).
//! - A real LCtrl down while AltGr is held transfers ownership back to the
//!   user (we must not release a key they are physically holding).
//! - AltGr up: release the owned Ctrl.
//!
//! The corner case (user taps the real LCtrl and AltGr within 100 ms) makes
//! us release their Ctrl when AltGr ends — a mild surprise, far better than
//! a silently stuck Ctrl.

use std::time::{Duration, Instant};

/// How close an LCtrl down must be to the AltGr down to be considered the
/// OS-generated fake (they share the GetMessage timestamp; 100 ms is far
/// above that and below deliberate human Ctrl→AltGr chords).
const FAKE_WINDOW: Duration = Duration::from_millis(100);

/// What the caller should do with LCtrl after consulting the tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtrlAction {
    /// Leave LCtrl alone (user-owned or already paired).
    Keep,
    /// Press LCtrl synthetically (no Ctrl held when AltGr went down).
    Press,
    /// Release the LCtrl we own (AltGr went up).
    Release,
}

/// AltGr ↔ LCtrl pairing state (see module docs).
#[derive(Debug, Default)]
pub struct AltGr {
    owns_ctrl: bool,
    lctrl_down_at: Option<Instant>,
}

impl AltGr {
    /// An LCtrl key-down was observed (real or OS fake).
    pub fn ctrl_down(&mut self, now: Instant) {
        // A Ctrl press while AltGr owns the Ctrl is a real user press:
        // ownership transfers so AltGr-up will not release it.
        self.owns_ctrl = false;
        self.lctrl_down_at = Some(now);
    }

    /// An LCtrl key-up was observed.
    pub fn ctrl_up(&mut self) {
        self.lctrl_down_at = None;
    }

    /// AltGr (RAlt) went down. `lctrl_held` = LCtrl currently pressed.
    pub fn altgr_down(&mut self, now: Instant, lctrl_held: bool) -> CtrlAction {
        let recent = self
            .lctrl_down_at
            .is_some_and(|t| now.duration_since(t) <= FAKE_WINDOW);
        if lctrl_held && !recent {
            // Real user-held Ctrl: do not touch it, do not release it later.
            self.owns_ctrl = false;
            return CtrlAction::Keep;
        }
        // Either no Ctrl held (we press one) or the just-seen Ctrl is the
        // OS fake (we adopt it to guarantee its release).
        self.owns_ctrl = true;
        if lctrl_held {
            CtrlAction::Keep
        } else {
            CtrlAction::Press
        }
    }

    /// AltGr (RAlt) went up.
    pub fn altgr_up(&mut self) -> CtrlAction {
        if self.owns_ctrl {
            self.owns_ctrl = false;
            CtrlAction::Release
        } else {
            CtrlAction::Keep
        }
    }

    /// Drops all ownership (window focus lost / release-all).
    pub fn reset(&mut self) {
        self.owns_ctrl = false;
        self.lctrl_down_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: u64) -> Instant {
        Instant::now()
            .checked_sub(Duration::from_millis(2000))
            .map(|b| b + Duration::from_millis(ms))
            .unwrap()
    }

    #[test]
    fn bare_altgr_presses_and_releases_ctrl() {
        let mut a = AltGr::default();
        assert_eq!(a.altgr_down(at(0), false), CtrlAction::Press);
        assert_eq!(a.altgr_up(), CtrlAction::Release);
        // And the tracker is clean for the next chord.
        assert_eq!(a.altgr_down(at(500), false), CtrlAction::Press);
    }

    #[test]
    fn os_fake_ctrl_is_adopted_and_released() {
        let mut a = AltGr::default();
        a.ctrl_down(at(1000)); // fake, same tick as the RAlt
        assert_eq!(a.altgr_down(at(1005), true), CtrlAction::Keep);
        assert_eq!(a.altgr_up(), CtrlAction::Release);
    }

    #[test]
    fn real_held_ctrl_is_never_touched() {
        let mut a = AltGr::default();
        a.ctrl_down(at(0));
        assert_eq!(a.altgr_down(at(500), true), CtrlAction::Keep);
        assert_eq!(a.altgr_up(), CtrlAction::Keep);
    }

    #[test]
    fn real_ctrl_during_altgr_takes_ownership_back() {
        let mut a = AltGr::default();
        assert_eq!(a.altgr_down(at(0), false), CtrlAction::Press);
        a.ctrl_down(at(300)); // user really presses Ctrl mid-AltGr
        assert_eq!(a.altgr_up(), CtrlAction::Keep);
    }

    #[test]
    fn reset_drops_ownership() {
        let mut a = AltGr::default();
        assert_eq!(a.altgr_down(at(0), false), CtrlAction::Press);
        a.reset();
        assert_eq!(a.altgr_up(), CtrlAction::Keep);
    }
}
