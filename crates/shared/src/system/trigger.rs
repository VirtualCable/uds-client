// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
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

    pub fn wait_timeout(&self, timeout: std::time::Duration) -> Result<(), std::io::Error> {
        let (lock, cvar, _) = &*self.state;
        let triggered = lock.lock().unwrap();
        let (guard, _result) = cvar
            .wait_timeout_while(triggered, timeout, |t| !*t)
            .unwrap();
        if *guard {
            Ok(())
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout"))
        }
    }

    pub async fn wait_async(&self) {
        let (lock, _, notify) = &*self.state;
        // Arm the waiter BEFORE reading the flag. `notify_waiters()` stores
        // no permit, and `notified()` snapshots its broadcast counter at
        // creation, so a `trigger()` landing between the flag check and the
        // creation of the `Notified` future is invisible to the waiter and
        // this wait would hang forever. Creating the future first closes
        // that window; `enable()` additionally registers it on the notify
        // list before the check, and the flag check still short-circuits the
        // common case.
        let notified = notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if *lock.lock().unwrap() {
            return;
        }
        notified.await;
    }

    pub async fn wait_timeout_async(
        &self,
        timeout: std::time::Duration,
    ) -> Result<(), std::io::Error> {
        let (lock, _, notify) = &*self.state;
        // Same arm-before-check ordering as `wait_async`.
        let notified = notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if *lock.lock().unwrap() {
            return Ok(());
        }
        tokio::select! {
            _ = notified => Ok(()),
            _ = tokio::time::sleep(timeout) => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout")),
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
        assert!(result.is_err());
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

    #[test]
    fn trigger_default_is_unset() {
        let t = Trigger::default();
        assert!(!t.is_triggered());
    }

    #[test]
    fn trigger_multiple_calls_idempotent() {
        let t = Trigger::new();
        t.trigger();
        t.trigger();
        assert!(t.is_triggered());
        t.wait();
    }

    #[test]
    fn trigger_wait_timeout_zero_unset() {
        let t = Trigger::new();
        assert!(t.wait_timeout(Duration::ZERO).is_err());
    }

    #[test]
    fn trigger_wait_timeout_zero_set() {
        let t = Trigger::new();
        t.trigger();
        assert!(t.wait_timeout(Duration::ZERO).is_ok());
    }

    #[tokio::test]
    async fn trigger_wait_async_already_set() {
        let t = Trigger::new();
        t.trigger();
        t.wait_async().await; // Should not block
    }

    #[tokio::test]
    async fn trigger_wait_timeout_async_timeout() {
        let t = Trigger::new();
        assert!(
            t.wait_timeout_async(Duration::from_millis(10))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn trigger_wait_timeout_async_already_set() {
        let t = Trigger::new();
        t.trigger();
        assert!(t.wait_timeout_async(Duration::ZERO).await.is_ok());
    }

    /// The fix's ordering, pinned deterministically: arm the waiter
    /// (create + `enable()`), THEN read the flag, and let the trigger land
    /// between the check and the final await. An armed waiter sits in
    /// `Notify`'s wait list, so `notify_waiters()` wakes it and the wait
    /// completes.
    #[tokio::test]
    async fn armed_waiter_observes_a_trigger_landing_after_the_flag_check() {
        use std::sync::{Arc, Mutex};
        use tokio::sync::Notify;

        let notify = Arc::new(Notify::new());
        let flag = Arc::new(Mutex::new(false));

        let mut notified = Box::pin(notify.notified());
        notified.as_mut().enable(); // arm BEFORE the check
        assert!(!*flag.lock().unwrap(), "not triggered yet -> would await");
        // trigger() runs while the waiter is between check and await:
        *flag.lock().unwrap() = true;
        notify.notify_waiters();
        let woke = tokio::time::timeout(Duration::from_millis(200), notified)
            .await
            .is_ok();
        assert!(woke, "an armed waiter must observe the notification");
    }

    /// Deterministic reproduction of the defect: the old ordering read the
    /// flag first and created the `Notified` afterwards. A `trigger()` that
    /// lands between the flag check and the future creation is invisible to
    /// the waiter — `notify_waiters()` stores no permit, and the future's
    /// broadcast counter is snapshotted at creation, after the notification
    /// already happened — so the await hangs forever. This pins the window
    /// the arm-before-check ordering in `wait_async` closes.
    #[tokio::test]
    async fn check_before_create_ordering_loses_a_trigger_landing_in_the_gap() {
        use std::sync::{Arc, Mutex};
        use tokio::sync::Notify;

        let notify = Arc::new(Notify::new());
        let flag = Arc::new(Mutex::new(false));

        // 1. old buggy ordering: check the flag first...
        let triggered = { *flag.lock().unwrap() };
        assert!(!triggered, "not triggered yet -> would await");
        // 2. trigger() lands in the gap (flag set, broadcast fired)
        //    before the waiter creates its future...
        *flag.lock().unwrap() = true;
        notify.notify_waiters();
        // 3. ...and only now the waiter creates the Notified and awaits it.
        //    The creation snapshot misses the broadcast: lost wakeup.
        let notified = notify.notified();
        let woke = tokio::time::timeout(Duration::from_millis(200), notified)
            .await
            .is_ok();
        assert!(
            !woke,
            "a waiter created after the broadcast must miss it (defect control)"
        );
    }

    /// `wait_async` / `wait_timeout_async` must never lose a `trigger()` that
    /// races the flag check; a lost wakeup would hang a stopping session.
    #[test]
    fn async_wakeup_never_lost_under_racing_triggers() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let mut lost = 0usize;
            for i in 0..5000u64 {
                let t = Trigger::new();
                let waiter = {
                    let t = t.clone();
                    tokio::spawn(async move {
                        if i % 2 == 0 {
                            t.wait_async().await;
                        } else {
                            t.wait_timeout_async(Duration::from_secs(4)).await.unwrap();
                        }
                    })
                };
                // Vary the interleaving: sometimes trigger immediately,
                // sometimes after a yield, sometimes from a separate task.
                match i % 3 {
                    0 => t.trigger(),
                    1 => {
                        tokio::task::yield_now().await;
                        t.trigger();
                    }
                    _ => {
                        let t2 = t.clone();
                        tokio::spawn(async move { t2.trigger() });
                    }
                }
                if tokio::time::timeout(Duration::from_secs(10), waiter)
                    .await
                    .is_err()
                {
                    lost += 1;
                }
            }
            assert_eq!(lost, 0, "{lost} async waits never observed the trigger");
        });
    }
}
