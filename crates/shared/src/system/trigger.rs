// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::sync::{Arc, Condvar, Mutex};

use tokio::sync::Notify;

#[derive(Clone, Debug)]
pub struct Trigger {
    state: Arc<(Mutex<bool>, Condvar, Notify)>,
}

impl Trigger {
    pub fn new() -> Self {
        Trigger {
            state: Arc::new((Mutex::new(false), Condvar::new(), Notify::new())),
        }
    }

    pub fn trigger(&self) {
        let (lock, cvar, notify) = &*self.state;
        let mut guard = lock.lock().unwrap();
        *guard = true;
        cvar.notify_all();
        notify.notify_waiters();
    }

    pub fn is_triggered(&self) -> bool {
        let (lock, _, _) = &*self.state;
        *lock.lock().unwrap()
    }

    pub fn wait(&self) {
        let (lock, cvar, _) = &*self.state;
        let mut guard = lock.lock().unwrap();
        while !*guard {
            guard = cvar.wait(guard).unwrap();
        }
    }

    pub fn wait_timeout(&self, timeout: std::time::Duration) -> bool {
        let (lock, cvar, _) = &*self.state;
        let triggered = lock.lock().unwrap();
        let (guard, _result) = cvar
            .wait_timeout_while(triggered, timeout, |t| !*t)
            .unwrap();
        *guard
    }

    pub async fn wait_async(&self) {
        let (lock, _, notify) = &*self.state;
        {
            let guard = lock.lock().unwrap();
            if *guard {
                return;
            }
        }
        notify.notified().await;
    }

    pub async fn wait_timeout_async(&self, timeout: std::time::Duration) -> bool {
        let (lock, _, notify) = &*self.state;
        {
            let guard = lock.lock().unwrap();
            if *guard {
                return true;
            }
        }
        tokio::select! {
            _ = notify.notified() => true,
            _ = tokio::time::sleep(timeout) => false,
        }
    }
}

impl Default for Trigger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn trigger_wait_blocks_until_set() {
        let trigger = Trigger::new();
        let trigger_clone = trigger.clone();
        let handle = thread::spawn(move || {
            // Wait 100ms and then set the trigger
            thread::sleep(Duration::from_millis(100));
            trigger_clone.trigger();
        });
        handle.join().unwrap();
        trigger.wait();
    }

    #[test]
    fn trigger_wait_timeout() {
        let trigger = Trigger::new();
        let result = trigger.wait_timeout(Duration::from_millis(100));
        assert!(!result);
    }

    #[test]
    fn trigger_is_set() {
        let trigger = Trigger::new();
        assert!(!trigger.is_triggered());
        trigger.trigger();
        assert!(trigger.is_triggered());
        trigger.wait(); // Should return immediately
        assert!(trigger.is_triggered());
    }
}
