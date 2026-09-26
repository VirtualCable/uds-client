// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

/// Size of the anti-replay window, in bits (i.e. in tracked sequence numbers).
///
/// Reference points: DTLS 1.2 uses a 64-bit window, IPsec implementations use
/// 64-1024. With RDPUDP2 datagrams of up to 1232 bytes, a 1024-entry window
/// covers ~1.2 MiB of in-flight traffic; reordering beyond that is
/// pathological and RDPUDP already treats it as loss.
pub const REPLAY_WINDOW_BITS: u64 = 1024;
const WINDOW_WORDS: usize = (REPLAY_WINDOW_BITS / 64) as usize;

/// Sliding-window anti-replay tracker for datagram transports (IPsec/DTLS style).
///
/// Unlike the stream `Crypt`, which requires strictly increasing sequence
/// numbers (valid over TCP, which cannot reorder), datagrams may arrive
/// reordered, duplicated or lost. This window accepts any unseen sequence
/// number within the last `REPLAY_WINDOW_BITS` values and rejects duplicates
/// and anything too old.
///
/// Sequence numbers start at `INITIAL_SEQ + 1` (senders pre-increment,
/// matching the `Crypt` convention); `seq == 0` is always rejected.
#[derive(Debug, Clone)]
pub struct ReplayWindow {
    /// Highest sequence number accepted so far. 0 means "nothing seen yet"
    /// (and is safe because seq 0 is never valid).
    max_seq: u64,
    /// Bit i of the bitmap tracks seq (max_seq - i). Bit 0 of word 0 is max_seq itself.
    bitmap: [u64; WINDOW_WORDS],
}

impl ReplayWindow {
    pub fn new() -> Self {
        ReplayWindow {
            max_seq: 0,
            bitmap: [0; WINDOW_WORDS],
        }
    }

    /// Returns true and marks the seq as seen if it is acceptable (new, and
    /// inside the window). Returns false for duplicates, seq 0, and seqs that
    /// fell off the window.
    pub fn check_and_mark(&mut self, seq: u64) -> bool {
        if seq == 0 {
            return false;
        }

        if seq > self.max_seq {
            let shift = seq - self.max_seq;
            if shift >= REPLAY_WINDOW_BITS {
                self.bitmap = [0; WINDOW_WORDS];
            } else {
                let word_shift = (shift / 64) as usize;
                let bit_shift = (shift % 64) as u32;
                let mut new_bitmap = [0u64; WINDOW_WORDS];
                for i in (0..WINDOW_WORDS).rev() {
                    let mut v = if i >= word_shift {
                        self.bitmap[i - word_shift] << bit_shift
                    } else {
                        0
                    };
                    if bit_shift != 0 && i > word_shift {
                        v |= self.bitmap[i - word_shift - 1] >> (64 - bit_shift);
                    }
                    new_bitmap[i] = v;
                }
                self.bitmap = new_bitmap;
            }
            self.max_seq = seq;
            self.bitmap[0] |= 1;
            return true;
        }

        let diff = self.max_seq - seq;
        if diff >= REPLAY_WINDOW_BITS {
            return false; // Too old, fell off the window
        }
        let word = (diff / 64) as usize;
        let mask = 1u64 << (diff % 64);
        if self.bitmap[word] & mask != 0 {
            return false; // Duplicate
        }
        self.bitmap[word] |= mask;
        true
    }
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order_seqs_are_accepted() {
        let mut w = ReplayWindow::new();
        for seq in 1..=2000u64 {
            assert!(w.check_and_mark(seq), "seq {} should be accepted", seq);
        }
    }

    #[test]
    fn seq_zero_is_always_rejected() {
        let mut w = ReplayWindow::new();
        assert!(!w.check_and_mark(0));
        assert!(w.check_and_mark(1));
        assert!(!w.check_and_mark(0));
    }

    #[test]
    fn duplicates_are_rejected() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_mark(1));
        assert!(!w.check_and_mark(1));
        assert!(w.check_and_mark(100));
        assert!(!w.check_and_mark(100));
        assert!(!w.check_and_mark(1));
    }

    #[test]
    fn reordering_inside_window_is_accepted() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_mark(1));
        assert!(w.check_and_mark(3));
        assert!(w.check_and_mark(2)); // late arrival, inside window
        assert!(!w.check_and_mark(2)); // but only once
        // Jump forward, then receive stragglers from before the jump
        assert!(w.check_and_mark(1000));
        assert!(w.check_and_mark(4));
        assert!(w.check_and_mark(999));
        assert!(w.check_and_mark(500));
    }

    #[test]
    fn seqs_older_than_window_are_rejected() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_mark(1));
        assert!(w.check_and_mark(1 + REPLAY_WINDOW_BITS));
        // seq 1 is now exactly REPLAY_WINDOW_BITS behind max: outside
        assert!(!w.check_and_mark(1));
        // The oldest in-window seq (2) is still fine
        assert!(w.check_and_mark(2));
    }

    #[test]
    fn huge_jump_resets_window() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_mark(1));
        assert!(w.check_and_mark(10_000_000));
        // Everything from before the jump is far outside the window now
        assert!(!w.check_and_mark(1));
        assert!(!w.check_and_mark(9_000_000));
        assert!(w.check_and_mark(10_000_001));
    }

    #[test]
    fn boundary_bits_of_each_word() {
        let mut w = ReplayWindow::new();
        // Put max_seq at a word boundary and probe around it
        assert!(w.check_and_mark(64 * 5));
        assert!(w.check_and_mark(64 * 5 - 63)); // last bit of word 0
        assert!(w.check_and_mark(64 * 5 - 64)); // first bit of word 1
        assert!(!w.check_and_mark(64 * 5 - 64)); // duplicate at word edge
    }
}
